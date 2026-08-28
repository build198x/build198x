# Decision: the ADF master — Amiga assembly's bootable-disk packaging books the media-mastering lane

**Status:** Active. Built 2026-07-10. The library now lives in Format198x as
[`format198x-commodore-amiga-adf`](https://github.com/format198x/format198x);
`build198x adf` and the standalone `build198x-adf` binary both consume it from
crates.io.

**Date:** 2026-07-06. Built 2026-07-10.

## The decision

Build198x masters the Amiga bootable `.adf`: a Kickstart-1.x hunk executable plus
a boot block and an authored `startup-sequence` in, a bootable OFS disk image out
— the shape a bare A500/KS1.3 boots straight into the game. This is the mastering
half of the Amiga-assembly build; Asm198x owns the other half.

Two halves, one seam:

1. **Assemble** (`.asm` → KS1.x hunk-exe) → **Asm198x** (`--dialect vasm --exe`).
2. **Master** (hunk-exe + boot block + `startup-sequence` → bootable `.adf`) →
   **Build198x** (this record).

## Why the seam falls here

By the resolved framing/mastering rule
([`tape-framing-vs-mastering.md`](../../../decisions/tape-framing-vs-mastering.md)):
a container whose content is the assembled program *and nothing else* is an
Asm198x framing; the moment a second artifact joins, it is mastering. A bootable
ADF is never just the program — it carries a boot block and an authored
`startup-sequence` that launches it, on an OFS filesystem. Program + loader +
filesystem = mastering, the same shape as Gloaming's loader+SCREEN$+CODE tape
([`demand-gate-tape-master.md`](demand-gate-tape-master.md)). It passes the
membership test: it converts build inputs into a machine-ready medium; it is not
assembly, not emulation, not playback.

## The gate

The Amiga-assembly capture pipeline mastered ADFs with `xdftool` inside the
`commodore-amiga` Docker image, the last build-image holdout of
[`code198x-dev-tooling-migration.md`](../../../decisions/code198x-dev-tooling-migration.md).
A family-owned ADF master is what let that image retire, so the need was present
rather than speculative.

## What the library does

A dependency-free (`core`/`std` only) read+write ADF library:

- **Write.** A `Volume` builder — `add_file(path, bytes)`, `add_dir(path)`,
  `set_bootable`, `build` — creating arbitrary nested trees at any depth with
  auto-created intermediate directories and per-file protection. `master` and
  `master_fs` are thin conveniences over it.
- **Read.** `Disk::open` validates boot block and root; `filesystem`, `label`,
  `list`, `read` for any path; `verify` checks every checksum — boot, root,
  bitmap, headers, extension and OFS data blocks — plus structural sanity.
- **OFS and FFS.** FFS (`DOS\1`) data blocks are raw 512-byte sectors navigated
  entirely by the header/extension pointer tables; the volume structure is
  identical to OFS.

## Constraints that bind

**Deterministic output.** Dates are zeroed and images are byte-stable across
runs. `xdftool` stamps creation dates, so its output is not reproducible; the
committed `.adf` deliverables are.

**Panic-free on malformed input.** Every block pointer is range-checked and every
chain loop-bounded, so a corrupt image yields `Error::Corrupt` rather than a
crash.

**Protection bits are `0x00`.** The RWED bits are **active-low**, so `0x0d`
revokes read and the CLI cannot `LoadSeg` the file. KS1.3 never enforced it,
which is why a wrong value can look cosmetic; KS2.04 reports `file is read
protected`. `0x00` is a normal readable/executable file and makes OFS disks
portable to KS2.0+ as well as KS1.3.

**FFS floppies boot only on KS2.0+.** The 1.3 ROM's floppy filesystem is
OFS-only, so the curriculum stays OFS; FFS is a general-tool capability.

**Validation is functional, not a byte-compare.** The bar is that the mastered
`.adf` boots in emu198x-amiga to the same verified screenshot. A structural
read-back is a useful secondary check. A byte-compare against `xdftool` is
meaningless because it stamps dates.

**Ingest contract:** raw hunk-exe + volume name + the `startup-sequence`
template, with no dependency on how the exe was produced — the same
raw-binary-in shape as the tape master.

**Correct for any input within the disk shape, not just the curriculum's.**
Data-pointer overflow beyond a header's 72 slots chains into `T_LIST` extension
blocks, and a program too large for an 880 KB disk is a typed error rather than a
corrupt image. Directory inserts chain through `hash_chain` on a slot collision
instead of clobbering, so any set of names is correct; header checksums are
deferred until after all inserts, since an insert can set a header's
`hash_chain`. The curriculum is the first consumer, not the bar — see
[`../../../decisions/family-tools-are-general.md`](../../../decisions/family-tools-are-general.md).

## The OFS structures a writer must emit

Dissected from a known-good disk, cross-checked against ADFlib and gadf:

- **Boot block** (sectors 0–1, 1024 B): `DOS\0` + the fixed 1.x boot code + boot
  checksum. The boot code is a constant blob — embed it, don't author it.
- **Root block** (sector 880): volume name, 72-slot hash table, bitmap
  pointer(s), dates, block checksum.
- **Bitmap block**: free/used sector map; one block suffices for DD.
- **Dir header**: like a file header, sec_type 2, its own 72-slot table.
- **File headers**: name hashed into the parent's table, size, protection bits,
  data-block list, checksum.
- **OFS data blocks**: 24-byte header (type/header-key/seq/data-size/next/
  checksum) + up to 488 B data, chained per file.

Plus the AmigaDOS filename hash and the OFS block checksum, both small and fully
specified. An 880K DD image is 1760×512 = 901,120 bytes.

## Why from scratch

The Rust ADF-write ecosystem stops short: `adflib`'s create is unimplemented,
`fstool` is a heavy multi-format dependency, and `gadf` — which does precisely
this job — is Go. A bounded from-scratch writer is a few hundred lines against a
fully documented format, and it is the only path that guarantees determinism.

## Out of scope

The International and Dir-Cache variants, hard-disk (RDB) layouts, multi-disk
sets, copy protection, and custom bootblocks or trackloaders. Each is its own
later scope on the general-tool roadmap.

D64, DSK, TRD, MDR and other machines' disk formats each fire their own gate when
a real need appears.
