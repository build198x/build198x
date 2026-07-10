# Decision: the ADF master — Amiga assembly's bootable-disk packaging books the media-mastering lane

**Status:** Active — **built 2026-07-10** (`build198x adf` / `format::adf`) and
wired into the Amiga-assembly capture path, retiring the last `commodore-amiga`
Docker image. Bounded from-scratch (the Rust ADF-write crates stop short), no
deps, deterministic. The placement follows the resolved tape/framing seam
([`tape-framing-vs-mastering.md`](../../../decisions/tape-framing-vs-mastering.md)).

**Date:** 2026-07-09.

## The decision

Build198x masters the Amiga **bootable `.adf`**: a Kickstart-1.x hunk executable
plus a boot block and an authored `startup-sequence` in, a bootable OFS disk
image out — the shape a bare A500/KS1.3 boots straight into the game. This is
the mastering half of the Amiga-assembly build; Asm198x owns the other half.

Two halves, one seam:

1. **Assemble** (`.asm` → KS1.x hunk-exe) → **Asm198x** (`--dialect vasm --exe`;
   the curriculum's `-Fhunkexe` target).
2. **Master** (hunk-exe + boot block + `startup-sequence` → bootable `.adf`) →
   **Build198x** (this record). Today `code-samples/_capture/capture.py`'s
   `ensure_amiga_adf` does this with `xdftool` via the `commodore-amiga` Docker
   image (create OFS + `boot install` + write `s/startup-sequence` + write the
   exe); that image is the last build-image holdout of
   [`code198x-dev-tooling-migration.md`](../../../decisions/code198x-dev-tooling-migration.md).

## Why the seam falls here

By the resolved framing/mastering rule
([`tape-framing-vs-mastering.md`](../../../decisions/tape-framing-vs-mastering.md)):
a container whose content is the assembled program *and nothing else* is an
Asm198x framing; the moment a second artifact joins, it is mastering. A
bootable ADF is never just the program — it carries a boot block and an authored
`startup-sequence` that launches it, on an OFS filesystem. Program + loader +
filesystem = mastering, the same shape as Gloaming's loader+SCREEN$+CODE tape
([`demand-gate-tape-master.md`](demand-gate-tape-master.md), which already named
"ADF/D64/DSK/TRD/MDR — each fires its own gate when real"). It passes the
membership test: it converts build inputs into a machine-ready medium; it is not
assembly, not emulation, not playback.

## The concrete need (the gate)

The Amiga-assembly capture pipeline masters ADFs *today* — exodus, flock,
signal, meet-the-machine all boot from `ensure_amiga_adf`-built disks. The
migration retires the `commodore-amiga` Docker image; that cannot complete while
the mastering step still shells out to `xdftool` in that image. So the need is
concrete and present, not speculative: a family-owned ADF master is what lets
the last build image retire.

**Assemble-half status (2026-07-09).** Asm198x `--exe` is byte-identical to vasm
for exodus and signal, but the corpus gate is **not yet met** — flock units
08–18 assemble 20 bytes shorter (an encoding differential), and
`meet-the-machine/unit-16/blitter.asm` fails on an unsupported `!` expression
operator. Both are logged as Asm198x compatibility bugs (see the migration
record's log). The assemble cutover waits on those; this mastering record is
independent of them.

## Scope fence

In: OFS disk image creation, boot-block install (`boot1x`), `s/startup-sequence`
authoring, writing + protecting the hunk-exe, KS1.x. Out (until their own gates
fire): FFS, multi-file/multi-program disks, D64/DSK/TRD/MDR and other machines'
disk formats, copy protection, custom bootblocks/trackloaders.

## Open questions for the build session

- Reuse vs reimplement the `xdftool` OFS/bootblock maths — whether Build198x
  wraps a vetted library or masters the disk bytes directly (parallels the
  tape master's "container maths in, medium out").
- The `startup-sequence` is authored plumbing (like the tape's BASIC loader):
  does it stay a fixed template, or take parameters as the roster grows?
- Ingest contract: raw hunk-exe + org/boot params in (no dependency on how the
  exe was produced), matching the tape master's raw-binary ingest.

## Mastering scope (scoped 2026-07-10)

What the mastering step does today (xdftool, per unit): create an 880K DD image
(1760×512 = 901120 bytes), format **OFS** with a volume name, install the
standard **1.x boot block**, make an `s/` dir, write `s/startup-sequence` (one
line: the exe name), write the exe with the **execute** protection bit. The
disk boots on a bare A500/KS1.3 straight into the program.

**The OFS structures a writer must emit** (dissected from a built disk):
- **Boot block** (sectors 0–1, 1024 B): `DOS\0` + the fixed 1.x boot code +
  boot checksum. The boot code is a constant blob — embed it, don't author it.
- **Root block** (sector 880): volume name, 72-slot hash table (top-level
  entries), bitmap pointer(s), dates, block checksum.
- **Bitmap block**: free/used sector map (one block suffices for DD).
- **Dir header** (`s/`): like a file header, sec_type 2, its own 72-slot table.
- **File headers** (`startup-sequence`, exe): name hashed into the parent's
  table, size, protection bits (exe gets `e`), data-block list, checksum.
- **OFS data blocks**: 24-byte header (type/header-key/seq/data-size/next/
  checksum) + up to 488 B data, chained per file.
Plus the AmigaDOS filename hash and the OFS block checksum — both small, fully
specified algorithms.

**Bounded scope** (the ca65-linker precedent): exactly one disk shape — bootable
OFS DD, 1.x boot block, `s/startup-sequence` + one exe. Out: FFS, HD, multi-file
or multi-disk sets, custom bootblocks/trackloaders (own gates when real).

**Rust landscape (evaluate before from-scratch).** `gadf` does precisely this
job (executable → bootable OFS ADF, AmigaDOS 1.2+); `adflib` (vschwaberow) is a
Rust read/**write** ADF library; `affs-read` is read-only; `fstool` builds
Amiga OFS/FFS images. Path A: wrap/port one of these. Path B: a bounded
from-scratch OFS writer (~a few hundred lines, no deps, format fully
documented). Decide by maturity + determinism (below).

**Determinism is a requirement and an improvement.** xdftool stamps creation
dates, so its `.adf` bytes aren't reproducible — a from-scratch writer (or a
patched/ configured crate) can zero/fix the dates and emit **byte-stable**
disks, making the committed `.adf` deliverables reproducible. Whichever path is
chosen must produce deterministic output.

**Validation:** not a byte-compare against xdftool (it stamps dates). The bar is
functional — the mastered `.adf` boots in emu198x-amiga to the same verified
screenshot (the migration trigger), confirmed 2026-07-10 for the *assemble*
half. A structural read-back (adflib/xdftool) is a useful secondary check.

**Ingest contract:** raw hunk-exe + volume name (+ the fixed `startup-sequence`
template) → bootable `.adf`; no dependency on how the exe was produced — the
same raw-binary-in shape as the tape master.

## Built (2026-07-10)

Path B (from-scratch) was chosen: the Rust ADF-write ecosystem stops short —
adflib's create is unimplemented, fstool is a heavy multi-format dependency,
gadf is Go. `format::adf` (dependency-free, `core`/`std` only) emits the boot
block (constant KS1.2+ blob), root/bitmap/dir/file headers, OFS data-block
chaining, the AmigaDOS name hash and block checksums; `build198x adf` is the
CLI. Layout was taken as ground truth from a known-good disk, cross-checked
against ADFlib and gadf. **Deterministic** — dates zeroed, byte-stable across
runs (an improvement over xdftool). Verified: exodus (1 block) and flock
unit-18 (26 KB / 55 blocks) master and boot to correct renders in
emu198x-amiga; round-trip + checksum + determinism tests pass. Wired into
`capture.py`'s `ensure_amiga_adf` — the Amiga-assembly build is now fully
family-tooled (Asm198x + Build198x), Docker retired. Open scope questions
(§ above) resolved by the build; the `startup-sequence` is a fixed `<name>\n`
template, ingest is raw hunk-exe + name + volume.

## Generalised beyond the curriculum shape (2026-07-10)

The first build was bounded to the curriculum's inputs: one exe of ≤72 data
blocks (~35 KB), and names assumed not to collide in the hash table. That is
the wrong bar for a family tool — Asm198x, Build198x, and Emu198x are
general tools that should be usable by anyone, with the curriculum merely the
first consumer (see the umbrella principle
[`../../../decisions/family-tools-are-general.md`](../../../decisions/family-tools-are-general.md)).
So the writer was made correct for *any* input within the OFS-DD shape:

- **Any file size.** Data-pointer overflow beyond a header's 72 slots chains
  into `T_LIST` extension blocks. The old block ceiling became a disk-capacity
  check: a program too large for an 880 KB disk is a typed error, not a
  corrupt image.
- **Any name set.** Directory inserts chain through the `hash_chain` field on a
  slot collision instead of clobbering, so any set of names is correct. Header
  checksums are deferred until after all inserts (an insert can set a header's
  `hash_chain`).
- **Protection bits** were left at the xdftool-copied `0x0d` here, documented as
  "cosmetic under KS1.3" — **that was wrong, corrected 2026-07-10 below.**

Verified by re-mastering + booting flock unit-18 and by unit tests for an
extension-block file and a hash-collision insert.

## Extracted, FFS added, protection bug fixed (2026-07-10)

Three follow-on changes made the master a standalone, portable, general tool:

1. **Extracted to its own crate** `format-commodore-amiga-adf` (dependency-free,
   GPL-2.0-or-later) so it can be consumed on its own — Emu198x's floppy read
   path in time, and crates.io once the read side lands. `build198x adf` and a
   new standalone `build198x-adf` binary both delegate to it. See
   [`module-and-crate-naming.md`](module-and-crate-naming.md) (amended: an
   external audience is a split-triggering consumer) and
   [`../../../decisions/family-tools-are-general.md`](../../../decisions/family-tools-are-general.md).

2. **FFS (`DOS\1`) added** alongside OFS, selected by a `FileSystem` argument
   (and `--ffs` on both CLIs). FFS data blocks are raw 512-byte sectors with no
   per-block header/chain, navigated entirely by the header/extension pointer
   tables; the volume structure is identical to OFS. **FFS floppies boot only on
   KS2.0+** — the 1.3 ROM's floppy filesystem is OFS-only — so the curriculum
   stays OFS; FFS is a general-tool capability for KS2.0+ users.

3. **Protection-bit bug fixed: `0x0d` → `0x00`.** Booting an FFS disk on KS2.04
   surfaced `flock: file is read protected` — the RWED bits are active-low, and
   `0x0d` revokes read, so the CLI could not `LoadSeg` the command. KS1.3 never
   enforced this, which is why the OFS disks "worked" and the value looked
   cosmetic. `0x00` (a normal readable/executable file) fixes it and makes the
   **OFS disks portable to KS2.0+ too**, not just KS1.3. Verified: flock unit-18
   boots to its title as OFS on KS1.3, and as both OFS and FFS on KS2.04.

## General multi-file/directory API (2026-07-10)

The single-exe master was generalised into a `Volume` builder: `add_file(path,
bytes)` / `add_dir(path)` create arbitrary nested trees (any depth,
auto-created intermediate directories, per-file protection), `set_bootable`
chooses a bootable vs data disk, and `build` emits the deterministic image.
`master`/`master_fs` are now thin conveniences over it — verified **byte-
identical** to the previous single-exe output, so nothing regressed. Empty
files, duplicate/File-through-path errors (`Error::BadPath`), and non-bootable
data disks are handled. Verified end-to-end: a two-directory disk (a command in
`c/`, run from `s/startup-sequence`) boots to its title on KS1.3; unit tests
cover nested trees, data disks, and empty files; an `examples/multi_file_disk`
doubles as a crates.io example.

## Read side (2026-07-10)

Added `Disk`: `open` (validate the boot block + root), `filesystem`, `label`,
`list`, `read` (any path, OFS or FFS), and `verify` (every checksum — boot,
root, bitmap, headers, extension and OFS data blocks — plus structural sanity).
It is **panic-free on malformed input**: every block pointer is range-checked
and every chain is loop-bounded, so a corrupt image yields `Error::Corrupt`, not
a crash. Round-trip tests assert `Disk::open(Volume::build())` reproduces every
file for both filesystems. This makes the crate a genuine read+write ADF
library — which Rust's `adflib` is not (its write is unimplemented) — and clears
the last capability gate before the crates.io publish.

What remains out — the International/Dir-Cache variants, hard-disk (RDB)
layouts, and multi-disk sets — is the general-tool roadmap, each its own later
scope.
