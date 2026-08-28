# Decision: the ADF master — Amiga assembly's bootable-disk packaging books the media-mastering lane

**Status:** Active. Built 2026-07-10. The library lives in Format198x as
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

## The ingest contract

Raw hunk-exe + volume name + the `startup-sequence` template, with no dependency
on how the exe was produced — the same raw-binary-in shape as the tape master.
This is the part that binds Build198x: the pipeline must not grow a dependency on
Asm198x's output format beyond "a hunk executable".

## The library's own design

How the disk is written — the OFS block structures, determinism, protection bits,
panic-free reads, and why the crate is from-scratch rather than wrapping an
existing one — belongs to the crate, and lives with it in
[`format198x/format198x/decisions/commodore-amiga-adf.md`](https://github.com/format198x/format198x/blob/main/decisions/commodore-amiga-adf.md).

D64, DSK, TRD, MDR and other machines' disk formats each fire their own gate when
a real need appears.
