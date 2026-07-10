//! Amiga ADF disk-image writer (OFS and FFS).
//!
//! Two entry points. [`Volume`] builds an arbitrary file/directory tree onto a
//! DD floppy image (880 KB) — `add_file`/`add_dir`, then `build`. [`master`]
//! (and [`master_fs`]) is the common special case: a Kickstart-1.x hunk
//! executable plus a `startup-sequence` that runs it, the disk an Amiga boots
//! straight into. This is the mastering half of the Amiga-assembly build
//! (Asm198x emits the hunk exe; this writes the disk), per
//! `Build198x/build198x/decisions/demand-gate-adf-master.md`.
//!
//! Both floppy filesystems are supported ([`FileSystem`]): **OFS** (`DOS\0`,
//! the bare A500/KS1.3 default) and **FFS** (`DOS\1`, denser and faster, but
//! bootable only on KS2.0+). They differ only in their data blocks; the volume
//! structure is identical.
//!
//! **Deterministic** (the determinism contract): every date field is zeroed and
//! block allocation is fixed, so the same exe + names always produce identical
//! bytes — unlike xdftool, which stamps creation dates. That makes the committed
//! `.adf` deliverables byte-reproducible.
//!
//! **General within the DD-floppy shape** — any tree of files and directories,
//! bootable or a plain data disk. It is correct for *any* input: a file of any
//! size chains into extension blocks (not just the 72 that fit a header), names
//! that hash to the same slot chain through the hash table, nested directories
//! to any depth, and a tree too large for an 880 KB disk is a typed error
//! rather than a corrupt image. The International/Dir-Cache variants, hard-disk
//! (RDB) layouts, multi-disk sets, and the read side are the remaining
//! generality frontier — each its own later scope.
//!
//! Layout facts were taken as ground truth from a known-good `xdftool` disk and
//! cross-checked against ADFlib (adflib/ADFlib) and gadf (sphair/gadf, public
//! domain). The block structures used:
//!
//! - **Boot block** (sectors 0–1): the DOS-type byte (`DOS\0` OFS / `DOS\1`
//!   FFS) + the fixed KS1.2+ boot code + `dos.library`, with an add-with-carry
//!   boot checksum. The bootstrap is a constant, volume-independent blob.
//! - **Root block** (block 880): volume name, a 72-slot name-hash table of
//!   top-level entries, the bitmap pointer, dates, and a block checksum.
//! - **Bitmap block** (block 881): one bit per block (1 = free), checksum at
//!   offset 0.
//! - **Directory / file headers**: type `T_HEADER` (2); a directory's 72-slot
//!   table holds child headers hashed by name, a file's holds its data-block
//!   pointers in reverse; secondary type `ST_USERDIR` (2) or `ST_FILE` (−3).
//! - **Data blocks**: OFS wraps each in a 24-byte header (`T_DATA`, header-key,
//!   1-based sequence, data size, next block, checksum) then up to 488 payload
//!   bytes; FFS stores a raw 512-byte sector and relies on the pointer tables.
//!
//! Pure byte-layout (`core`/`std` only), per `decisions/module-and-crate-naming.md`.

/// Why an ADF operation failed.
///
/// The write path validates its inputs rather than panicking. Marked
/// `#[non_exhaustive]` because the read side (forthcoming) will add
/// parse-failure variants without a breaking change.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// A file or volume name is empty, longer than 30 bytes, or not
    /// AmigaDOS-legal (ASCII only).
    InvalidName {
        /// Which name was rejected — e.g. `"file name"`, `"volume name"`.
        what: &'static str,
        /// The length supplied.
        len: usize,
    },
    /// The content does not fit on a double-density floppy. Counts are in
    /// 512-byte blocks.
    DiskFull {
        /// Blocks the content requires.
        needed: u32,
        /// Blocks a DD floppy leaves free for the file tree.
        available: u32,
    },
    /// A path passed to [`Volume`] could not be used — it is empty, already
    /// exists, or routes a directory through a file.
    BadPath {
        /// The offending path.
        path: String,
        /// Why it was rejected.
        reason: &'static str,
    },
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidName { what, len } => {
                write!(f, "{what}: must be 1..=30 ASCII bytes (got {len})")
            }
            Self::DiskFull { needed, available } => write!(
                f,
                "disk full: {needed} blocks needed, {available} free on an 880K floppy"
            ),
            Self::BadPath { path, reason } => write!(f, "bad path {path:?}: {reason}"),
        }
    }
}

impl std::error::Error for Error {}

/// Which Amiga filesystem to write.
///
/// The two differ only in their data blocks: [`Ofs`](Self::Ofs) wraps each in a
/// 24-byte header (type/key/sequence/size/next/checksum), so a block holds 488
/// payload bytes and the file is a self-describing chain; [`Ffs`](Self::Ffs)
/// stores raw 512-byte sectors and relies entirely on the header/extension
/// pointer tables. The root, bitmap, directory, and file-header blocks are
/// identical between them.
///
/// **Boot compatibility:** an FFS floppy boots only on Kickstart 2.0+ — the 1.3
/// ROM's floppy filesystem is OFS-only. Target OFS for a bare A500/KS1.3; FFS
/// is for KS2.0+ machines (and is faster and denser there).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum FileSystem {
    /// Old File System (`DOS\0`) — headered data blocks. Boots on KS1.3+.
    #[default]
    Ofs,
    /// Fast File System (`DOS\1`) — raw data sectors. Boots on KS2.0+.
    Ffs,
}

impl FileSystem {
    /// The lowercase short name — `"ofs"` or `"ffs"`. Handy for CLI output and
    /// logging without matching a `#[non_exhaustive]` enum.
    pub fn name(self) -> &'static str {
        match self {
            Self::Ofs => "ofs",
            Self::Ffs => "ffs",
        }
    }

    /// The boot-block DOS-type byte (offset 3): 0 for OFS, 1 for FFS.
    fn dos_type(self) -> u8 {
        match self {
            Self::Ofs => 0,
            Self::Ffs => 1,
        }
    }

    /// Payload bytes per data block: OFS reserves 24 for the block header.
    fn data_capacity(self) -> usize {
        match self {
            Self::Ofs => OFS_DATA,
            Self::Ffs => BSIZE,
        }
    }
}

/// Bytes per disk block (sector).
const BSIZE: usize = 512;
/// Blocks on a DD floppy: 80 cylinders × 2 heads × 11 sectors.
const BLOCKS: u32 = 1760;
/// The root block sits at the middle of a DD disk.
const ROOT_BLK: u32 = 880;
/// The bitmap block, immediately after the root.
const BITMAP_BLK: u32 = 881;
/// Hash-table / data-pointer slots per header block.
const HT_SIZE: usize = 72;
/// Payload bytes per OFS data block (512 − the 24-byte OFS data header).
const OFS_DATA: usize = BSIZE - 24;
/// File/dir/data blocks are allocated upward from here (deterministic).
const FIRST_FREE: u32 = 882;

/// Primary block type for headers.
const T_HEADER: u32 = 2;
/// Primary block type for OFS data blocks.
const T_DATA: u32 = 8;
/// Primary block type for file-extension lists (data pointers beyond a header's
/// 72 slots).
const T_LIST: u32 = 16;
/// Secondary type: root.
const ST_ROOT: u32 = 1;
/// Secondary type: user directory.
const ST_USERDIR: u32 = 2;
/// Secondary type: file (−3 as a two's-complement u32).
const ST_FILE: u32 = (-3i32) as u32;

/// AmigaDOS name length limit.
const MAX_NAME: usize = 30;

/// Protection bits for the executable. The low nibble is the RWED set, stored
/// **active-low** — a set bit *revokes* that permission — so `0x00` grants read,
/// write, execute, and delete: a normal, runnable file. The executable must be
/// readable and executable, because the CLI `LoadSeg`s the command named in
/// `startup-sequence`; revoking read breaks that on any Kickstart that enforces
/// protection.
///
/// An earlier `0x0d` (read/write/delete revoked) was copied from an xdftool
/// disk and *looked* fine because KS1.3 ignores protection on LoadSeg — but it
/// fails on KS2.0+ with "file is read protected". See the demand-gate-adf-master
/// decision log (2026-07-10). Fixing it also makes the OFS disks portable to
/// KS2.0+, not just KS1.3.
const EXE_PROTECT: u32 = 0x00;

/// The standard KS1.2+ OFS boot block: `DOS\0`, its checksum, the boot code,
/// and `dos.library`. 49 nonzero bytes; the rest of the 1024-byte boot area is
/// zero. Volume-independent — verified to boot on A500/KS1.3.
const BOOT_PREFIX: [u8; 49] = [
    0x44, 0x4f, 0x53, 0x00, 0xc0, 0x20, 0x0f, 0x19, 0x00, 0x00, 0x03, 0x70, 0x43, 0xfa, 0x00, 0x18,
    0x4e, 0xae, 0xff, 0xa0, 0x4a, 0x80, 0x67, 0x0a, 0x20, 0x40, 0x20, 0x68, 0x00, 0x16, 0x70, 0x00,
    0x4e, 0x75, 0x70, 0xff, 0x60, 0xfa, 0x64, 0x6f, 0x73, 0x2e, 0x6c, 0x69, 0x62, 0x72, 0x61, 0x72,
    0x79,
];

fn put_u32(buf: &mut [u8], off: usize, v: u32) {
    buf[off..off + 4].copy_from_slice(&v.to_be_bytes());
}

fn read_u32(buf: &[u8], off: usize) -> u32 {
    u32::from_be_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}

/// The 512-byte slice for block `n` within the disk image.
fn block_mut(img: &mut [u8], n: u32) -> &mut [u8] {
    let off = n as usize * BSIZE;
    &mut img[off..off + BSIZE]
}

/// The AmigaDOS block checksum: the value that makes the sum of all 128
/// longwords come to zero, with the checksum field (`chk_off`) taken as zero.
/// Headers and data blocks put it at offset 20; the bitmap block at offset 0.
fn checksum(block: &[u8], chk_off: usize) -> u32 {
    let mut sum: u32 = 0;
    let mut i = 0;
    while i < BSIZE {
        if i != chk_off {
            sum = sum.wrapping_add(read_u32(block, i));
        }
        i += 4;
    }
    sum.wrapping_neg()
}

/// The boot-block checksum over the 1024-byte boot area: add every longword
/// with end-around carry, then complement. Distinct from [`checksum`] — the
/// bootstrap ROM verifies the boot block with this add-with-carry variant.
/// The caller zeroes the checksum field (offset 4) before calling.
fn boot_checksum(boot: &[u8]) -> u32 {
    let mut sum: u32 = 0;
    let mut i = 0;
    while i < 1024 {
        let (s, carried) = sum.overflowing_add(read_u32(boot, i));
        sum = if carried { s.wrapping_add(1) } else { s };
        i += 4;
    }
    !sum
}

/// Write the boot block (sectors 0–1): the DOS-type marker, the filesystem's
/// type byte, and a freshly computed boot checksum. A `bootable` disk also
/// carries the fixed bootstrap blob (reproduced byte-for-byte for OFS); a data
/// disk gets only `DOS` + type + checksum — mountable, but the ROM finds no
/// bootstrap to run.
fn write_boot_block(img: &mut [u8], fs: FileSystem, bootable: bool) {
    if bootable {
        img[..BOOT_PREFIX.len()].copy_from_slice(&BOOT_PREFIX);
    } else {
        img[0..4].copy_from_slice(b"DOS\0");
    }
    img[3] = fs.dos_type();
    put_u32(img, 4, 0); // zero the checksum field before computing
    let c = boot_checksum(&img[..1024]);
    put_u32(img, 4, c);
}

/// AmigaDOS filename hash → slot in a 72-entry table. `h = len; for each byte
/// h = (h*13 + toupper(c)) & 0x7ff; slot = h % 72`.
fn name_hash(name: &str) -> usize {
    let mut h = name.len() as u32;
    for c in name.bytes() {
        h = h
            .wrapping_mul(13)
            .wrapping_add(c.to_ascii_uppercase() as u32)
            & 0x7ff;
    }
    (h as usize) % HT_SIZE
}

/// Write a `name_len`-prefixed AmigaDOS name into `block` ending at its tail
/// (the name field ends 80 bytes from the block end: len byte at `BSIZE-80`).
fn put_name(block: &mut [u8], name: &str) {
    block[BSIZE - 80] = name.len() as u8;
    block[BSIZE - 79..BSIZE - 79 + name.len()].copy_from_slice(name.as_bytes());
}

fn validate_name(name: &str, what: &'static str) -> Result<(), Error> {
    if name.is_empty() || name.len() > MAX_NAME || !name.is_ascii() {
        return Err(Error::InvalidName {
            what,
            len: name.len(),
        });
    }
    Ok(())
}

/// The immutable 512-byte slice for block `n`.
fn block(img: &[u8], n: u32) -> &[u8] {
    let off = n as usize * BSIZE;
    &img[off..off + BSIZE]
}

/// Insert `child` into `parent`'s hash table under `name`, chaining on a slot
/// collision via the sibling chain (`hash_chain` at `BSIZE-16`). This makes the
/// writer correct for *any* set of names, not only ones that happen not to
/// collide. Does not checksum — the caller checksums headers after all inserts
/// (an insert may set a header's `hash_chain`).
fn dir_insert(img: &mut [u8], parent: u32, child: u32, name: &str) {
    let slot = 24 + 4 * name_hash(name);
    let head = read_u32(block(img, parent), slot);
    if head == 0 {
        put_u32(block_mut(img, parent), slot, child);
    } else {
        let mut cur = head;
        loop {
            let next = read_u32(block(img, cur), BSIZE - 16);
            if next == 0 {
                break;
            }
            cur = next;
        }
        put_u32(block_mut(img, cur), BSIZE - 16, child);
    }
}

/// Extension blocks a file of `data_n` data blocks needs beyond its header's 72
/// pointer slots.
fn ext_count(data_n: usize) -> usize {
    data_n.saturating_sub(1) / HT_SIZE
}

/// Write a file's data blocks — any length. OFS wraps each block in a 24-byte
/// header (type/key/sequence/size/next/checksum) and chains them; FFS writes
/// raw 512-byte sectors, relying on the header/extension pointer tables for
/// order.
fn write_file_data(
    img: &mut [u8],
    fs: FileSystem,
    header_key: u32,
    data_blocks: &[u32],
    payload: &[u8],
) {
    let cap = fs.data_capacity();
    for (i, &blk) in data_blocks.iter().enumerate() {
        let start = i * cap;
        let chunk = &payload[start..(start + cap).min(payload.len())];
        match fs {
            FileSystem::Ffs => {
                block_mut(img, blk)[..chunk.len()].copy_from_slice(chunk);
            }
            FileSystem::Ofs => {
                let next = data_blocks.get(i + 1).copied().unwrap_or(0);
                let b = block_mut(img, blk);
                put_u32(b, 0, T_DATA);
                put_u32(b, 4, header_key);
                put_u32(b, 8, i as u32 + 1); // 1-based sequence
                put_u32(b, 12, chunk.len() as u32);
                put_u32(b, 16, next);
                b[24..24 + chunk.len()].copy_from_slice(chunk);
                let c = checksum(b, 20);
                put_u32(block_mut(img, blk), 20, c);
            }
        }
    }
}

/// Fill a header/ext block's data-pointer table (slots from the top down).
fn put_ptr_table(img: &mut [u8], blk: u32, ptrs: &[u32]) {
    let b = block_mut(img, blk);
    for (i, &d) in ptrs.iter().enumerate() {
        put_u32(b, 24 + 4 * (HT_SIZE - 1 - i), d);
    }
}

/// Write a file header plus any extension blocks holding its data-block
/// pointers (72 per block) — a file of any size, up to the disk. The header is
/// left unchecksummed (a directory insert may set its `hash_chain`); the ext
/// blocks, which inserts never touch, are checksummed here.
#[allow(clippy::too_many_arguments)]
fn write_file_header(
    img: &mut [u8],
    hdr: u32,
    ext: &[u32],
    name: &str,
    parent: u32,
    data_blocks: &[u32],
    byte_size: u32,
    protect: u32,
) {
    let first = &data_blocks[..data_blocks.len().min(HT_SIZE)];
    {
        let b = block_mut(img, hdr);
        put_u32(b, 0, T_HEADER);
        put_u32(b, 4, hdr); // own block number
        put_u32(b, 8, first.len() as u32); // high_seq: pointers in this block
        put_u32(b, 16, data_blocks.first().copied().unwrap_or(0)); // first_data (0 if empty)
        put_u32(b, BSIZE - 192, protect);
        put_u32(b, BSIZE - 188, byte_size);
        put_name(b, name);
        put_u32(b, BSIZE - 12, parent);
        put_u32(b, BSIZE - 8, ext.first().copied().unwrap_or(0)); // extension
        put_u32(b, BSIZE - 4, ST_FILE);
    }
    put_ptr_table(img, hdr, first);
    for (k, &e) in ext.iter().enumerate() {
        let start = HT_SIZE * (k + 1);
        let these = &data_blocks[start..(start + HT_SIZE).min(data_blocks.len())];
        {
            let b = block_mut(img, e);
            put_u32(b, 0, T_LIST);
            put_u32(b, 4, e); // own block number
            put_u32(b, 8, these.len() as u32);
            put_u32(b, BSIZE - 12, hdr); // parent: the file header
            put_u32(b, BSIZE - 8, ext.get(k + 1).copied().unwrap_or(0)); // next ext
            put_u32(b, BSIZE - 4, ST_FILE);
        }
        put_ptr_table(img, e, these);
        let c = checksum(block(img, e), 20);
        put_u32(block_mut(img, e), 20, c);
    }
}

/// Master `exe` (a KS1.x hunk executable) into a bootable OFS DD `.adf` — the
/// bare A500/KS1.3 shape. Convenience for [`master_fs`] with
/// [`FileSystem::Ofs`].
pub fn master(exe: &[u8], name: &str, volume: &str) -> Result<Vec<u8>, Error> {
    master_fs(exe, name, volume, FileSystem::Ofs)
}

/// Master `exe` (a KS1.x hunk executable) into a bootable DD `.adf` that runs
/// it, on the chosen [`FileSystem`]. `name` is the file's on-disk name and the
/// `startup-sequence` command; `volume` is the disk label. Returns the
/// 901,120-byte image. (FFS boots on KS2.0+ only — see [`FileSystem`].)
///
/// A convenience over [`Volume`] for the one-executable bootable disk; use
/// `Volume` directly for arbitrary file/directory trees.
pub fn master_fs(exe: &[u8], name: &str, volume: &str, fs: FileSystem) -> Result<Vec<u8>, Error> {
    validate_name(name, "file name")?;
    let mut vol = Volume::new(volume, fs);
    // `s/startup-sequence` is added before the executable so the block layout
    // matches the historical single-exe master exactly — byte-stable output.
    vol.add_file("s/startup-sequence", format!("{name}\n").as_bytes())?;
    vol.add_file(name, exe)?;
    vol.set_bootable(true);
    vol.build()
}

/// A directory in the volume tree: named children in insertion order.
#[derive(Default)]
struct DirNode {
    entries: Vec<(String, Child)>,
}

enum Child {
    File { bytes: Vec<u8>, protect: u32 },
    Dir(DirNode),
}

impl DirNode {
    /// Get-or-create a child directory named `name`; error if a *file* already
    /// occupies that name.
    fn dir_child(&mut self, name: &str, path: &str) -> Result<&mut DirNode, Error> {
        if let Some(i) = self.entries.iter().position(|(n, _)| n == name) {
            match &mut self.entries[i].1 {
                Child::Dir(d) => Ok(d),
                Child::File { .. } => Err(Error::BadPath {
                    path: path.to_owned(),
                    reason: "a path component is a file, not a directory",
                }),
            }
        } else {
            self.entries
                .push((name.to_owned(), Child::Dir(DirNode::default())));
            match &mut self.entries.last_mut().unwrap().1 {
                Child::Dir(d) => Ok(d),
                _ => unreachable!(),
            }
        }
    }

    fn has(&self, name: &str) -> bool {
        self.entries.iter().any(|(n, _)| n == name)
    }
}

/// Split a slash-separated path into validated, non-empty components.
fn split_path(path: &str) -> Result<Vec<String>, Error> {
    let parts: Vec<String> = path
        .split('/')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    if parts.is_empty() {
        return Err(Error::BadPath {
            path: path.to_owned(),
            reason: "empty path",
        });
    }
    for p in &parts {
        validate_name(p, "path component")?;
    }
    Ok(parts)
}

/// A double-density Amiga floppy volume you fill with files and directories,
/// then [`build`](Volume::build) into a deterministic 880 KB `.adf` image.
///
/// ```
/// use format_commodore_amiga_adf::{FileSystem, Volume};
/// let mut vol = Volume::new("MyDisk", FileSystem::Ofs);
/// vol.add_file("c/hello", b"...").unwrap();
/// vol.add_file("s/startup-sequence", b"c/hello\n").unwrap();
/// vol.set_bootable(true);
/// let adf = vol.build().unwrap();
/// assert_eq!(adf.len(), 901_120);
/// ```
pub struct Volume {
    label: String,
    fs: FileSystem,
    bootable: bool,
    root: DirNode,
}

impl Volume {
    /// A new, empty volume with the given label and filesystem. Not bootable
    /// until [`set_bootable(true)`](Volume::set_bootable).
    pub fn new(label: &str, fs: FileSystem) -> Self {
        Volume {
            label: label.to_owned(),
            fs,
            bootable: false,
            root: DirNode::default(),
        }
    }

    /// Set whether the disk carries the boot bootstrap. A bootable disk runs
    /// `s/startup-sequence`; a data disk is mountable but does not boot.
    pub fn set_bootable(&mut self, bootable: bool) -> &mut Self {
        self.bootable = bootable;
        self
    }

    /// Add a file at `path` (slash-separated, e.g. `"s/startup-sequence"`),
    /// creating any intermediate directories. Protection defaults to a normal
    /// readable/executable file; use [`add_file_with_protection`] to override.
    ///
    /// [`add_file_with_protection`]: Volume::add_file_with_protection
    pub fn add_file(&mut self, path: &str, bytes: &[u8]) -> Result<&mut Self, Error> {
        self.add_file_with_protection(path, bytes, EXE_PROTECT)
    }

    /// Add a file with explicit AmigaDOS protection bits (active-low RWED; see
    /// the crate docs). Otherwise like [`add_file`](Volume::add_file).
    pub fn add_file_with_protection(
        &mut self,
        path: &str,
        bytes: &[u8],
        protect: u32,
    ) -> Result<&mut Self, Error> {
        let parts = split_path(path)?;
        let (dirs, leaf) = parts.split_at(parts.len() - 1);
        let leaf = &leaf[0];
        let mut cur = &mut self.root;
        for d in dirs {
            cur = cur.dir_child(d, path)?;
        }
        if cur.has(leaf) {
            return Err(Error::BadPath {
                path: path.to_owned(),
                reason: "already exists",
            });
        }
        cur.entries.push((
            leaf.clone(),
            Child::File {
                bytes: bytes.to_vec(),
                protect,
            },
        ));
        Ok(self)
    }

    /// Add an explicit (possibly empty) directory at `path`, creating any
    /// intermediate directories. Idempotent for an existing directory; errors
    /// if a file already occupies the path.
    pub fn add_dir(&mut self, path: &str) -> Result<&mut Self, Error> {
        let parts = split_path(path)?;
        let mut cur = &mut self.root;
        for p in &parts {
            cur = cur.dir_child(p, path)?;
        }
        Ok(self)
    }

    /// Build the deterministic `.adf` image (901,120 bytes). Errors only if the
    /// tree does not fit on an 880 KB disk or the volume label is invalid.
    pub fn build(&self) -> Result<Vec<u8>, Error> {
        validate_name(&self.label, "volume name")?;

        // Plan: assign blocks to every directory header, file header, file
        // extension block, and data block, in a deterministic pre-order walk.
        let mut planned: Vec<Planned> = Vec::new();
        let mut next = FIRST_FREE;
        plan_dir(&self.root, ROOT_BLK, self.fs, &mut next, &mut planned);
        let used_end = next; // FIRST_FREE..used_end are the file-tree blocks

        if used_end > BLOCKS {
            return Err(Error::DiskFull {
                needed: used_end - FIRST_FREE,
                available: BLOCKS - FIRST_FREE,
            });
        }

        let mut img = vec![0u8; BLOCKS as usize * BSIZE];
        write_boot_block(&mut img, self.fs, self.bootable);

        // Data blocks + headers (headers unchecksummed; an insert may set a
        // header's hash_chain).
        for p in &planned {
            match p {
                Planned::File {
                    hdr,
                    ext,
                    data,
                    parent,
                    name,
                    bytes,
                    protect,
                } => {
                    write_file_data(&mut img, self.fs, *hdr, data, bytes);
                    write_file_header(
                        &mut img,
                        *hdr,
                        ext,
                        name,
                        *parent,
                        data,
                        bytes.len() as u32,
                        *protect,
                    );
                }
                Planned::Dir { hdr, parent, name } => {
                    let b = block_mut(&mut img, *hdr);
                    put_u32(b, 0, T_HEADER);
                    put_u32(b, 4, *hdr); // own block
                    put_name(b, name);
                    put_u32(b, BSIZE - 12, *parent);
                    put_u32(b, BSIZE - 4, ST_USERDIR);
                }
            }
        }

        // Root block (structure only; entries inserted below).
        {
            let b = block_mut(&mut img, ROOT_BLK);
            put_u32(b, 0, T_HEADER);
            put_u32(b, 12, HT_SIZE as u32); // hash-table size (root only)
            put_u32(b, BSIZE - 200, 0xffff_ffff); // bitmap flag: valid
            put_u32(b, BSIZE - 196, BITMAP_BLK); // bm_pages[0]
            put_name(b, &self.label);
            put_u32(b, BSIZE - 4, ST_ROOT);
        }

        // Insert every entry into its parent (in pre-order, so sibling chain
        // order on a hash collision is deterministic), then checksum all
        // headers — an insert can set a header's hash_chain.
        for p in &planned {
            let (parent, hdr, name) = p.link();
            dir_insert(&mut img, parent, hdr, name);
        }
        let c = checksum(block(&img, ROOT_BLK), 20);
        put_u32(block_mut(&mut img, ROOT_BLK), 20, c);
        for p in &planned {
            let (_, hdr, _) = p.link();
            let c = checksum(block(&img, hdr), 20);
            put_u32(block_mut(&mut img, hdr), 20, c);
        }

        // Bitmap block: 1 = free. Mark the used blocks used.
        {
            let words = ((BLOCKS - 2) as usize).div_ceil(32); // blocks 2..BLOCKS
            let mut map = vec![0xffff_ffffu32; words];
            let mut mark_used = |n: u32| {
                let i = (n - 2) as usize;
                map[i / 32] &= !(1u32 << (i % 32));
            };
            mark_used(ROOT_BLK);
            mark_used(BITMAP_BLK);
            for n in FIRST_FREE..used_end {
                mark_used(n);
            }
            // Bits past the last real block (BLOCKS-1) don't exist: mark used.
            for n in BLOCKS..(2 + words as u32 * 32) {
                mark_used(n);
            }
            let b = block_mut(&mut img, BITMAP_BLK);
            for (i, w) in map.iter().enumerate() {
                put_u32(b, 4 + 4 * i, *w);
            }
            let c = checksum(b, 0);
            put_u32(block_mut(&mut img, BITMAP_BLK), 0, c);
        }

        Ok(img)
    }
}

/// A tree node with its assigned blocks, ready to write.
enum Planned<'a> {
    File {
        hdr: u32,
        ext: Vec<u32>,
        data: Vec<u32>,
        parent: u32,
        name: &'a str,
        bytes: &'a [u8],
        protect: u32,
    },
    Dir {
        hdr: u32,
        parent: u32,
        name: &'a str,
    },
}

impl Planned<'_> {
    /// The (parent, own-header, name) triple every node inserts into its parent.
    fn link(&self) -> (u32, u32, &str) {
        match self {
            Planned::File {
                parent, hdr, name, ..
            } => (*parent, *hdr, name),
            Planned::Dir { parent, hdr, name } => (*parent, *hdr, name),
        }
    }
}

/// Take `n` consecutive blocks from the allocation cursor, returning the first.
fn take_blocks(next: &mut u32, n: u32) -> u32 {
    let base = *next;
    *next += n;
    base
}

/// Assign blocks to `dir`'s subtree, pre-order, appending to `out`.
fn plan_dir<'a>(
    dir: &'a DirNode,
    parent: u32,
    fs: FileSystem,
    next: &mut u32,
    out: &mut Vec<Planned<'a>>,
) {
    for (name, child) in &dir.entries {
        match child {
            Child::File { bytes, protect } => {
                let hdr = take_blocks(next, 1);
                let data_n = if bytes.is_empty() {
                    0
                } else {
                    bytes.len().div_ceil(fs.data_capacity())
                };
                let ext: Vec<u32> = (0..ext_count(data_n))
                    .map(|_| take_blocks(next, 1))
                    .collect();
                let data: Vec<u32> = (0..data_n).map(|_| take_blocks(next, 1)).collect();
                out.push(Planned::File {
                    hdr,
                    ext,
                    data,
                    parent,
                    name,
                    bytes,
                    protect: *protect,
                });
            }
            Child::Dir(sub) => {
                let hdr = take_blocks(next, 1);
                out.push(Planned::Dir { hdr, parent, name });
                plan_dir(sub, hdr, fs, next, out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Follow the on-disk structure to read a top-level file's bytes back —
    /// validating the root hash table, file header, data-pointer table, and
    /// OFS data-block chain the way a real filesystem would.
    fn read_file(img: &[u8], name: &str) -> Vec<u8> {
        let root = &img[ROOT_BLK as usize * BSIZE..][..BSIZE];
        let hdr_blk = read_u32(root, 24 + 4 * name_hash(name));
        let hdr = &img[hdr_blk as usize * BSIZE..][..BSIZE];
        let size = read_u32(hdr, BSIZE - 188) as usize;
        let mut blk = read_u32(hdr, 16); // first_data
        let mut out = Vec::new();
        while blk != 0 {
            let d = &img[blk as usize * BSIZE..][..BSIZE];
            let n = read_u32(d, 12) as usize;
            out.extend_from_slice(&d[24..24 + n]);
            blk = read_u32(d, 16);
        }
        assert_eq!(out.len(), size, "declared size vs chained data for {name}");
        out
    }

    /// Collect a header/extension block's data-block pointers, in file order
    /// (the reverse-filled table, `high_seq` of them).
    fn collect_ptrs(img: &[u8], blk: u32, out: &mut Vec<u32>) {
        let b = block(img, blk);
        let n = read_u32(b, 8) as usize;
        for i in 0..n {
            out.push(read_u32(b, 24 + 4 * (HT_SIZE - 1 - i)));
        }
    }

    /// A header/dir block's AmigaDOS name.
    fn header_name(img: &[u8], blk: u32) -> String {
        let b = block(img, blk);
        let len = b[BSIZE - 80] as usize;
        String::from_utf8_lossy(&b[BSIZE - 79..BSIZE - 79 + len]).into_owned()
    }

    /// Find `name` in directory `dir`, following the hash chain on a collision.
    fn lookup(img: &[u8], dir: u32, name: &str) -> u32 {
        let mut e = read_u32(block(img, dir), 24 + 4 * name_hash(name));
        while e != 0 {
            if header_name(img, e) == name {
                return e;
            }
            e = read_u32(block(img, e), BSIZE - 16); // hash_chain
        }
        0
    }

    /// Resolve a slash-separated path to its header block — a miniature read
    /// side, walking directory hash tables and hash chains.
    fn resolve(img: &[u8], path: &str) -> u32 {
        let mut blk = ROOT_BLK;
        for comp in path.split('/').filter(|s| !s.is_empty()) {
            blk = lookup(img, blk, comp);
            assert!(blk != 0, "path component {comp:?} not found");
        }
        blk
    }

    /// Read a file at `path` by walking the header + extension pointer tables —
    /// the way FFS (which has no per-data-block chain) and disk validators
    /// navigate. Works for both filesystems and any depth.
    fn read_file_via_ptrs(img: &[u8], path: &str, fs: FileSystem) -> Vec<u8> {
        let hdr_blk = resolve(img, path);
        let size = read_u32(block(img, hdr_blk), BSIZE - 188) as usize;
        let mut blocks = Vec::new();
        collect_ptrs(img, hdr_blk, &mut blocks);
        let mut ext = read_u32(block(img, hdr_blk), BSIZE - 8);
        while ext != 0 {
            collect_ptrs(img, ext, &mut blocks);
            ext = read_u32(block(img, ext), BSIZE - 8);
        }
        let mut out = Vec::new();
        for &b in &blocks {
            match fs {
                FileSystem::Ffs => out.extend_from_slice(block(img, b)),
                FileSystem::Ofs => {
                    let d = block(img, b);
                    let n = read_u32(d, 12) as usize;
                    out.extend_from_slice(&d[24..24 + n]);
                }
            }
        }
        out.truncate(size); // FFS's final sector is zero-padded to 512
        out
    }

    fn assert_checksums(img: &[u8], name: &str) {
        // Every header/data block used carries a valid offset-20 checksum;
        // the bitmap an offset-0 one.
        let check = |blk: u32, off: usize| {
            let b = &img[blk as usize * BSIZE..][..BSIZE];
            assert_eq!(
                read_u32(b, off),
                checksum(b, off),
                "checksum block {blk} of {name}"
            );
        };
        check(ROOT_BLK, 20);
        check(BITMAP_BLK, 0);
        // Walk root entries and their data.
        let root = &img[ROOT_BLK as usize * BSIZE..][..BSIZE];
        for slot in 0..HT_SIZE {
            let e = read_u32(root, 24 + 4 * slot);
            if e != 0 {
                check(e, 20);
            }
        }
    }

    #[test]
    fn masters_a_bootable_shape() {
        let exe = b"\x00\x00\x03\xf3 fake hunk exe payload".to_vec();
        let img = master(&exe, "game", "Game").unwrap();
        assert_eq!(img.len(), BLOCKS as usize * BSIZE);
        assert_eq!(&img[0..4], b"DOS\0");
        assert_eq!(
            read_u32(&img[ROOT_BLK as usize * BSIZE..], BSIZE - 4),
            ST_ROOT
        );
        assert_checksums(&img, "game");
        // The exe reads back intact, and the `s` directory is reachable from
        // the root (an empty read: a directory has no file data).
        assert_eq!(read_file(&img, "game"), exe);
        assert!(read_file(&img, "s").is_empty());
    }

    #[test]
    fn round_trips_a_multi_block_file() {
        // 3 data blocks: forces chaining, seq numbers, reverse pointer table.
        let exe: Vec<u8> = (0..OFS_DATA * 2 + 100).map(|i| (i % 251) as u8).collect();
        let img = master(&exe, "big", "Big").unwrap();
        assert_checksums(&img, "big");
        assert_eq!(read_file(&img, "big"), exe);
    }

    #[test]
    fn boot_checksum_reproduces_the_ofs_reference() {
        // Recomputing the OFS boot block must reproduce BOOT_PREFIX byte-for-byte
        // — including the embedded reference checksum at offset 4. This both
        // validates the add-with-carry algorithm and guards OFS byte-identity now
        // that the boot block is built dynamically.
        let mut img = vec![0u8; 1024];
        write_boot_block(&mut img, FileSystem::Ofs, true);
        assert_eq!(&img[..BOOT_PREFIX.len()], &BOOT_PREFIX[..]);
        assert!(img[BOOT_PREFIX.len()..].iter().all(|&b| b == 0));
    }

    #[test]
    fn round_trips_a_multi_block_ffs_file() {
        // FFS: raw 512-byte sectors, no data-block header/chain — navigated
        // entirely by the header/extension pointer tables. Force several blocks
        // and a partial final sector.
        let exe: Vec<u8> = (0..BSIZE * 3 + 137).map(|i| (i % 251) as u8).collect();
        let img = master_fs(&exe, "ffsgame", "FfsGame", FileSystem::Ffs).unwrap();
        assert_eq!(&img[0..4], b"DOS\x01", "FFS boot type");
        // Volume structure (root/bitmap/headers) is identical to OFS and still
        // checksums; the data blocks carry no checksum, by design.
        assert_checksums(&img, "ffsgame");
        assert_eq!(read_file_via_ptrs(&img, "ffsgame", FileSystem::Ffs), exe);
        // The OFS reader (pointer tables) agrees with the chain reader on OFS,
        // confirming the two navigation paths are consistent.
        let ofs = master_fs(&exe, "ffsgame", "FfsGame", FileSystem::Ofs).unwrap();
        assert_eq!(
            read_file_via_ptrs(&ofs, "ffsgame", FileSystem::Ofs),
            read_file(&ofs, "ffsgame")
        );
    }

    #[test]
    fn ffs_is_deterministic_and_denser_than_ofs() {
        let exe = vec![0x5au8; 4000];
        assert_eq!(
            master_fs(&exe, "d", "D", FileSystem::Ffs).unwrap(),
            master_fs(&exe, "d", "D", FileSystem::Ffs).unwrap()
        );
        // 4000 bytes: OFS needs ceil(4000/488)=9 data blocks, FFS ceil(4000/512)
        // =8 — so the FFS image marks fewer blocks used. Compare bitmap free bits.
        let free = |img: &[u8]| -> u32 {
            (0..((BLOCKS - 2) as usize).div_ceil(32))
                .map(|i| read_u32(block(img, BITMAP_BLK), 4 + 4 * i).count_ones())
                .sum()
        };
        assert!(
            free(&master_fs(&exe, "d", "D", FileSystem::Ffs).unwrap())
                > free(&master(&exe, "d", "D").unwrap()),
            "FFS should leave more blocks free than OFS for the same file"
        );
    }

    #[test]
    fn round_trips_a_file_needing_extension_blocks() {
        // >72 data blocks: the header's 72 pointer slots overflow into at least
        // one extension block. A general writer must handle files of any size.
        let exe: Vec<u8> = (0..OFS_DATA * 80 + 7).map(|i| (i % 251) as u8).collect();
        let img = master(&exe, "huge", "Huge").unwrap();
        assert_checksums(&img, "huge");
        assert_eq!(read_file(&img, "huge"), exe);

        // The extension chain is well-formed and self-checksummed.
        let hdr = read_u32(block(&img, ROOT_BLK), 24 + 4 * name_hash("huge"));
        let mut ext = read_u32(block(&img, hdr), BSIZE - 8);
        let mut ext_seen = 0;
        while ext != 0 {
            let b = block(&img, ext);
            assert_eq!(read_u32(b, 0), T_LIST, "ext block {ext} type");
            assert_eq!(read_u32(b, 20), checksum(b, 20), "ext checksum {ext}");
            ext = read_u32(b, BSIZE - 8);
            ext_seen += 1;
        }
        assert!(ext_seen >= 1, "expected at least one extension block");
    }

    #[test]
    fn dir_insert_chains_on_hash_collision() {
        // Two distinct names that hash to the same slot must both stay
        // reachable: the first in the slot, the second on its hash_chain. This
        // is what makes the writer correct for *any* set of names.
        let mut seen: Vec<(usize, String)> = Vec::new();
        let (mut first, mut second) = (None, None);
        for i in 0..4000u32 {
            let n = format!("f{i}");
            let slot = name_hash(&n);
            if let Some((_, prev)) = seen.iter().find(|(s, _)| *s == slot) {
                first = Some(prev.clone());
                second = Some(n);
                break;
            }
            seen.push((slot, n));
        }
        let (first, second) = (first.unwrap(), second.unwrap());
        assert_eq!(name_hash(&first), name_hash(&second));

        let mut img = vec![0u8; BLOCKS as usize * BSIZE];
        let parent = ROOT_BLK;
        dir_insert(&mut img, parent, 100, &first);
        dir_insert(&mut img, parent, 101, &second);
        let slot = 24 + 4 * name_hash(&first);
        assert_eq!(read_u32(block(&img, parent), slot), 100, "slot holds first");
        assert_eq!(
            read_u32(block(&img, 100), BSIZE - 16),
            101,
            "second chains off first"
        );
        assert_eq!(
            read_u32(block(&img, 101), BSIZE - 16),
            0,
            "chain terminates"
        );
    }

    #[test]
    fn deterministic() {
        let exe = vec![0xa5u8; 5000];
        assert_eq!(
            master(&exe, "d", "D").unwrap(),
            master(&exe, "d", "D").unwrap()
        );
    }

    #[test]
    fn volume_writes_a_nested_multi_file_tree() {
        let mut vol = Volume::new("Tree", FileSystem::Ofs);
        vol.add_file("readme", b"top-level file\n").unwrap();
        vol.add_file("c/list", &vec![0x42u8; 2000]).unwrap(); // multi-block, nested
        vol.add_file("c/util/deep", b"deeply nested\n").unwrap(); // two dirs down
        vol.add_file("s/startup-sequence", b"c/list\n").unwrap();
        vol.set_bootable(true);
        let img = vol.build().unwrap();

        assert_eq!(
            read_file_via_ptrs(&img, "readme", FileSystem::Ofs),
            b"top-level file\n"
        );
        assert_eq!(
            read_file_via_ptrs(&img, "c/list", FileSystem::Ofs),
            vec![0x42u8; 2000]
        );
        assert_eq!(
            read_file_via_ptrs(&img, "c/util/deep", FileSystem::Ofs),
            b"deeply nested\n"
        );
        assert_eq!(
            read_file_via_ptrs(&img, "s/startup-sequence", FileSystem::Ofs),
            b"c/list\n"
        );
        // Intermediate paths are directories.
        assert_eq!(
            read_u32(block(&img, resolve(&img, "c")), BSIZE - 4),
            ST_USERDIR
        );
        assert_eq!(
            read_u32(block(&img, resolve(&img, "c/util")), BSIZE - 4),
            ST_USERDIR
        );
        assert!(vol.build().unwrap() == img, "deterministic");
    }

    #[test]
    fn volume_rejects_bad_paths() {
        let mut vol = Volume::new("V", FileSystem::Ofs);
        vol.add_file("a", b"1").unwrap();
        assert!(matches!(
            vol.add_file("a", b"2"),
            Err(Error::BadPath { .. })
        )); // duplicate
        assert!(matches!(
            vol.add_file("a/b", b"3"),
            Err(Error::BadPath { .. })
        )); // through a file
        assert!(matches!(vol.add_file("", b"x"), Err(Error::BadPath { .. }))); // empty
        assert!(
            vol.add_file(&format!("{}/x", "n".repeat(31)), b"y")
                .is_err()
        ); // bad component
    }

    #[test]
    fn data_disk_is_mountable_but_not_bootable() {
        let mut vol = Volume::new("Data", FileSystem::Ofs);
        vol.add_file("notes", b"hello\n").unwrap();
        let img = vol.build().unwrap(); // bootable defaults to false
        assert_eq!(&img[0..4], b"DOS\0");
        // The boot checksum is valid, but there is no bootstrap to run.
        let mut probe = img[..1024].to_vec();
        put_u32(&mut probe, 4, 0);
        assert_eq!(
            boot_checksum(&probe),
            read_u32(&img, 4),
            "data-disk boot checksum"
        );
        assert!(
            img[8..1024].iter().all(|&b| b == 0),
            "no bootstrap on a data disk"
        );
        assert_eq!(
            read_file_via_ptrs(&img, "notes", FileSystem::Ofs),
            b"hello\n"
        );
    }

    #[test]
    fn volume_handles_empty_files() {
        let mut vol = Volume::new("E", FileSystem::Ofs);
        vol.add_file("empty", b"").unwrap();
        let img = vol.build().unwrap();
        let hdr = resolve(&img, "empty");
        assert_eq!(read_u32(block(&img, hdr), BSIZE - 188), 0, "byte_size 0");
        assert_eq!(read_u32(block(&img, hdr), 16), 0, "first_data 0");
        assert_eq!(read_u32(block(&img, hdr), 8), 0, "high_seq 0");
        assert!(read_file_via_ptrs(&img, "empty", FileSystem::Ofs).is_empty());
    }

    #[test]
    fn rejects_disk_full_and_bad_names() {
        // Larger than an 880K disk can hold: a typed disk-full error, not a
        // panic or a corrupt image.
        let too_big = vec![0u8; BSIZE * BLOCKS as usize];
        assert!(master(&too_big, "x", "X").is_err());
        assert!(master(b"z", "", "V").is_err());
        assert!(master(b"z", &"n".repeat(31), "V").is_err());
    }
}
