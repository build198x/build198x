# Build198x

The build-tools pipeline for the [198x family](https://github.com/build198x) — everything between authored source and a runnable artifact that isn't assembly or emulation.

One binary, one subcommand per tool:

```
build198x image  <input.png> --machine <id> --format <f>   # images → native screen formats
build198x beeper <input.bpr> [--repeat <n>]                # phrase notation → audition WAV + Spectrum beeper asm
build198x adf    create <out.adf> [flags]                  # files + directories → Amiga OFS/FFS volume
build198x adf    <program> -o <out.adf>                    # hunk executable → bootable Amiga disk
```

## The tools

**`image`** — converts modern images to native screen formats: Spectrum `.scr`, C64 Koala and Art Studio, Amiga IFF/ILBM. Spec-driven (the [`mediaspec198x`](crates/mediaspec198x) capability layer describes each machine's constraints; the pipeline searches within them), deterministic byte-for-byte across platforms for PNG input, and emitting a machine-readable JSON report. First consumer: Code198x curriculum art.

**`beeper`** — turns a textual phrase notation (notes, durations, rests) into two renderings of one timing model: a square-wave WAV for fast audition by ear, and the phrase as ZX Spectrum assembly in the table-free `beep`/`rest` idiom the Code198x curriculum teaches. Calibrated by regenerating Gloaming's hand-authored phrase constants exactly. First consumer: Gloaming's audio pass. The tool emits phrases, never the playback routines — those stay hand-written curriculum content.

**`adf`** — creates, masters, verifies, and inspects Commodore Amiga ADF floppy images. `master` (and the shorthand form shown above) packages one hunk executable into a bootable disk; `create` authors a general OFS or FFS volume from files and directories. `verify` performs a deep structural and checksum check, while `info` reports the filesystem and directory contents. The command delegates the disk layout to the standalone [`format198x-commodore-amiga-adf`](https://crates.io/crates/format198x-commodore-amiga-adf) library owned by Format198x; Build198x owns the mastering workflow and CLI.

Each tool opened on a named concrete need (the demand gate): see [`decisions/demand-gate-opening.md`](decisions/demand-gate-opening.md), [`decisions/demand-gate-beeper-phrases.md`](decisions/demand-gate-beeper-phrases.md), and the completed [`decisions/demand-gate-adf-master.md`](decisions/demand-gate-adf-master.md).

A third lane is pending a boundary decision: the Spectrum **tape master** (`.tap`: BASIC loader + SCREEN$ + CODE). Its demand gate is recorded in [`decisions/demand-gate-tape-master.md`](decisions/demand-gate-tape-master.md); implementation waits until the Build198x/Asm198x ownership call is settled.

## Install

Prebuilt binaries for each release are on the [Releases page](https://github.com/build198x/build198x/releases) (built by cargo-dist). The full `build198x` binary is distributed there. The lean ADF-only binary is also published to crates.io:

```sh
cargo install build198x-adf
```

Its command surface mirrors `build198x adf` and adds a `manifest` verb for canonical provenance JSON:

```sh
build198x-adf mygame -o mygame.adf
build198x-adf mygame -o mygame.adf --protect e
build198x-adf create data.adf --label Data --add readme.txt --mkdir docs
build198x-adf create game.adf --add mygame=c/mygame --protect-file c/mygame=e
build198x-adf verify data.adf
build198x-adf info --recursive data.adf
build198x-adf manifest data.adf
```

## The roster and the gate

Three bands: **asset conversion** (graphics, audio, palettes), **data packing** (crunchers, level/tilemap packers), and **media mastering** (disk and tape images, bootable media — ADF, D64, TAP, and friends). It meets [Asm198x](https://github.com/asm198x) at the program-framing handoff: Asm198x emits the program, Build198x masters it onto media.

The wider roster stays demand-gated: each tool starts when its own concrete need fires, never speculatively. A **membership test** keeps the scope honest — a tool belongs here only if it *converts, packs, or masters build inputs into machine-ready data or media*. Assembly is Asm198x; emulation is Emu198x; cataloguing is Cat198x; playback of existing media is Play198x.

## Two rules, inherited from Asm198x

**Native output only, never a bespoke format.** And every tool is validated against reality: format encoders round-trip against reference tools and golden fixtures ([`decisions/validation-tiers.md`](decisions/validation-tiers.md)), contracted conversions are byte-identical across platforms ([`decisions/determinism-contract.md`](decisions/determinism-contract.md)), and the beeper's timing model is proven against hand-authored constants from shipped game code.

Build198x is the build-tools sibling of the 198x family, alongside Code198x, Emu198x, Asm198x, Cat198x, Forge198x, and Play198x.
