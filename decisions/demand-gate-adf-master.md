# Decision: the ADF master — Amiga assembly's bootable-disk packaging books the media-mastering lane

**Status:** Active (blessed by Steve 2026-07-10). Gate opened; the tool is not
yet started — this record is the placement and reasoning, and the build is now
unblocked. The placement follows the resolved tape/framing seam
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
