# Decision: the BASIC build — Code198x's BASIC lessons book a listing-to-media verb

**Status:** Active. Built 2026-09-26. The tokenisers live in Format198x as
[`format198x-sinclair-zx-spectrum-bas`](https://github.com/format198x/format198x)
and `format198x-commodore-c64-bas`; `build198x basic` consumes them from
crates.io.

**Date:** 2026-09-26.

## The decision

Build198x turns a numbered BASIC listing into the file its machine loads:

- **ZX Spectrum:** a ROM header and data block pair in a `.tap`, auto-running
  from the program's first line unless `--no-autorun` is given.
- **Commodore 64:** a `.prg` at `$0801`, which the tokeniser already emits.

`--machine` picks the machine, and the BASIC and the container follow from it.
The design is in
[`docs/specs/2026-09-26-basic-verb.md`](https://github.com/build198x/docs/blob/main/specs/2026-09-26-basic-verb.md).

## The gate

Code198x's BASIC lessons have no runnable build. About 170 Spectrum BASIC
lesson pages show a listing but give the reader nothing to run. The assembly
lessons build in their code-samples Makefiles and the website runs what those
Makefiles produce; no family tool did the same for BASIC:

- asm198x only wraps machine code in a BASIC loader;
- Emu198x's tokenisers were private crates, reachable only from inside the
  emulator;
- the retired `zmakebas`/`bas2tap` path is outside the family.

A Makefile now calls `build198x basic` the way the assembly Makefiles call
asm198x. That need was present, not speculative, so the gate opens.

## Why it lands here

The umbrella rule in
[`tape-framing-vs-mastering.md`](../../../decisions/tape-framing-vs-mastering.md)
gives Asm198x a tape whose content is the *assembled* program and nothing
else: that is framing. The same record separates Asm198x's fixed auto-run stub
from authored BASIC, which it places in Build198x.

A BASIC listing is authored BASIC, and nothing here is assembled. Tokenising
the listing is conversion, the charter's asset-conversion lane. The TAP or PRG is the
container that conversion writes, the same way `image` writes a `.scr`. A
one-program tape is not mastering: the rule keeps that word for composing more
than one artifact, such as a loader, a loading screen and code. The tokenisers themselves
moved to Format198x under
[`formats-graduate-to-their-own-projects.md`](../../../decisions/formats-graduate-to-their-own-projects.md),
because Build198x is a consumer that is not their producer.

## Scope fence

**In:** stock-ROM tokenisation; a self-starting Spectrum TAP; a C64 PRG at
`$0801`; lint, which runs before every build so a Makefile cannot build a
listing that fails it.

**Out until their own need arrives:** TZX, disk images, 128K-only keywords,
loading screens for BASIC tapes, and every other machine. A machine with more
than one BASIC adds a `--dialect` override when it is added; nothing here needs
one yet.

## Next

The parser and language server are the next sub-project, with their own spec:
a lossless syntax tree per dialect and LSP diagnostics built on the same
positioned token stream the lint rules use. Where that lives is for its spec to
decide, not this record.
