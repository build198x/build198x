//! Amiga bootable-floppy master (OFS and FFS).
//!
//! Masters a Kickstart-1.x hunk executable into a bootable DD floppy image
//! (880 KB) — the disk an Amiga boots straight into, running the program from
//! `s/startup-sequence`. This is the mastering half of the Amiga-assembly build
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
//! **General within the DD-floppy shape** — the standard boot block, an
//! `s/startup-sequence` that runs the program, and the executable. Within that
//! shape it is correct for *any* input: a file of any size chains into
//! extension blocks (not just the 72 that fit a header), names that hash to the
//! same slot chain through the hash table, and a program too large for an
//! 880 KB disk is a typed error rather than a corrupt image. The
//! International/Dir-Cache variants, hard-disk (RDB) layouts, multi-disk sets,
//! and the read side are the remaining generality frontier — each its own later
//! scope.
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

/// Write the boot block (sectors 0–1): the fixed bootstrap blob, the
/// filesystem's DOS-type byte, and a freshly computed boot checksum. For OFS
/// this reproduces the known-good blob byte-for-byte; for FFS it flips the type
/// byte and recomputes.
fn write_boot_block(img: &mut [u8], fs: FileSystem) {
    img[..BOOT_PREFIX.len()].copy_from_slice(&BOOT_PREFIX);
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
        put_u32(b, 16, data_blocks[0]); // first_data
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
pub fn master_fs(exe: &[u8], name: &str, volume: &str, fs: FileSystem) -> Result<Vec<u8>, Error> {
    validate_name(name, "file name")?;
    validate_name(volume, "volume name")?;

    let startup = format!("{name}\n");
    let startup_bytes = startup.as_bytes();
    let cap = fs.data_capacity();
    let exe_data_n = exe.len().div_ceil(cap).max(1);
    let startup_data_n = startup_bytes.len().div_ceil(cap).max(1);

    // Deterministic block allocation, upward from FIRST_FREE: each file is a
    // header, its extension blocks, then its data blocks.
    let mut next = FIRST_FREE;
    let mut alloc = |count: u32| {
        let base = next;
        next += count;
        base
    };
    let s_dir_blk = alloc(1);
    let startup_hdr_blk = alloc(1);
    let startup_ext: Vec<u32> = (0..ext_count(startup_data_n)).map(|_| alloc(1)).collect();
    let startup_data: Vec<u32> = (0..startup_data_n).map(|_| alloc(1)).collect();
    let exe_hdr_blk = alloc(1);
    let exe_ext: Vec<u32> = (0..ext_count(exe_data_n)).map(|_| alloc(1)).collect();
    let exe_data: Vec<u32> = (0..exe_data_n).map(|_| alloc(1)).collect();
    let used_end = next; // FIRST_FREE..used_end are the file-tree blocks

    if used_end > BLOCKS {
        // The program plus its filesystem overhead doesn't fit on an 880K disk.
        return Err(Error::DiskFull {
            needed: used_end - FIRST_FREE,
            available: BLOCKS - FIRST_FREE,
        });
    }

    let mut img = vec![0u8; BLOCKS as usize * BSIZE];

    // Boot block (sectors 0-1): bootstrap blob + filesystem type + checksum.
    write_boot_block(&mut img, fs);

    // Data blocks, then file headers (+ extension blocks) — headers unchecksummed.
    write_file_data(&mut img, fs, startup_hdr_blk, &startup_data, startup_bytes);
    write_file_data(&mut img, fs, exe_hdr_blk, &exe_data, exe);
    write_file_header(
        &mut img,
        startup_hdr_blk,
        &startup_ext,
        "startup-sequence",
        s_dir_blk,
        &startup_data,
        startup_bytes.len() as u32,
        0,
    );
    write_file_header(
        &mut img,
        exe_hdr_blk,
        &exe_ext,
        name,
        ROOT_BLK,
        &exe_data,
        exe.len() as u32,
        EXE_PROTECT,
    );

    // `s/` directory header (structure only; the entry is inserted below).
    {
        let b = block_mut(&mut img, s_dir_blk);
        put_u32(b, 0, T_HEADER);
        put_u32(b, 4, s_dir_blk); // own block
        put_name(b, "s");
        put_u32(b, BSIZE - 12, ROOT_BLK); // parent
        put_u32(b, BSIZE - 4, ST_USERDIR);
    }

    // Root block (structure only; entries inserted below).
    {
        let b = block_mut(&mut img, ROOT_BLK);
        put_u32(b, 0, T_HEADER);
        put_u32(b, 12, HT_SIZE as u32); // hash-table size (root only)
        put_u32(b, BSIZE - 200, 0xffff_ffff); // bitmap flag: valid
        put_u32(b, BSIZE - 196, BITMAP_BLK); // bm_pages[0]
        put_name(b, volume);
        put_u32(b, BSIZE - 4, ST_ROOT);
    }

    // Insert entries into their parents (chaining on any hash collision), then
    // checksum every header — an insert can set a header's `hash_chain`.
    dir_insert(&mut img, s_dir_blk, startup_hdr_blk, "startup-sequence");
    dir_insert(&mut img, ROOT_BLK, s_dir_blk, "s");
    dir_insert(&mut img, ROOT_BLK, exe_hdr_blk, name);
    for blk in [ROOT_BLK, s_dir_blk, startup_hdr_blk, exe_hdr_blk] {
        let c = checksum(block(&img, blk), 20);
        put_u32(block_mut(&mut img, blk), 20, c);
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

    /// Read a top-level file by walking the header + extension pointer tables —
    /// the way FFS (which has no per-data-block chain) and disk validators
    /// navigate. Works for both filesystems.
    fn read_file_via_ptrs(img: &[u8], name: &str, fs: FileSystem) -> Vec<u8> {
        let hdr_blk = read_u32(block(img, ROOT_BLK), 24 + 4 * name_hash(name));
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
        write_boot_block(&mut img, FileSystem::Ofs);
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
    fn rejects_disk_full_and_bad_names() {
        // Larger than an 880K disk can hold: a typed disk-full error, not a
        // panic or a corrupt image.
        let too_big = vec![0u8; BSIZE * BLOCKS as usize];
        assert!(master(&too_big, "x", "X").is_err());
        assert!(master(b"z", "", "V").is_err());
        assert!(master(b"z", &"n".repeat(31), "V").is_err());
    }
}
