# Build198x

The build-tools pipeline for the [198x family](https://github.com/build198x) — everything between authored source and a runnable artifact that isn't assembly or emulation.

One binary, one subcommand per tool:

```
build198x image  <input.png> --machine <id> --format <f>   # images → native screen formats
build198x beeper <input.bpr> [--repeat <n>]                # phrase notation → audition WAV + Spectrum beeper asm
build198x adf    create <out.adf> [flags]                  # files + directories → Amiga OFS/FFS volume
build198x adf    <program> -o <out.adf>                    # hunk executable → bootable Amiga disk
build198x basic  <in.bas> --machine <id> -o <out>          # numbered BASIC listing → tape or PRG
build198x basic  lint <in.bas>... --machine <id> [--fix]   # check (and mend) a listing against its listed form
```

## The tools

**`image`** — converts modern images to native screen formats: Spectrum `.scr`, C64 Koala and Art Studio, Amiga IFF/ILBM. Spec-driven (the [`mediaspec198x`](crates/mediaspec198x) capability layer describes each machine's constraints; the pipeline searches within them), deterministic byte-for-byte across platforms for PNG input, and emitting a machine-readable JSON report. First consumer: Code198x curriculum art.

**`beeper`** — turns a textual phrase notation (notes, durations, rests) into two renderings of one timing model: a square-wave WAV for fast audition by ear, and the phrase as ZX Spectrum assembly in the table-free `beep`/`rest` idiom the Code198x curriculum teaches. Calibrated by regenerating Gloaming's hand-authored phrase constants exactly. First consumer: Gloaming's audio pass. The tool emits phrases, never the playback routines — those stay hand-written curriculum content.

**`adf`** — creates, masters, verifies, and inspects Commodore Amiga ADF floppy images. `master` (and the shorthand form shown above) packages one hunk executable into a bootable disk; `create` authors a general OFS or FFS volume from files and directories. `verify` performs a deep structural and checksum check, while `info` reports the filesystem and directory contents. The command delegates the disk layout to the standalone [`format198x-commodore-amiga-adf`](https://crates.io/crates/format198x-commodore-amiga-adf) library owned by Format198x; Build198x owns the mastering workflow and CLI.

**`basic`** — turns a numbered BASIC listing into the file its machine loads: a self-starting ZX Spectrum `.tap`, or a Commodore 64 `.prg` at `$0801`. `basic lint` checks (and, with `--fix`, mends) a listing against the *listed form* — see [BASIC listings](#basic-listings) below. First consumer: Code198x's ~170 BASIC lesson pages, which had a listing but nothing to run.

Each tool opened on a named concrete need (the demand gate): see [`decisions/demand-gate-opening.md`](decisions/demand-gate-opening.md), [`decisions/demand-gate-beeper-phrases.md`](decisions/demand-gate-beeper-phrases.md), [`decisions/demand-gate-adf-master.md`](decisions/demand-gate-adf-master.md), and [`decisions/demand-gate-basic.md`](decisions/demand-gate-basic.md).

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

## BASIC listings

```sh
build198x basic hello.bas --machine sinclair-zx-spectrum -o hello.tap
build198x basic hello.bas --machine commodore-c64 -o hello.prg
build198x basic lint hello.bas --machine sinclair-zx-spectrum --fix
```

`--machine` picks the machine (`sinclair-zx-spectrum` or `commodore-c64`); the
BASIC dialect and the output container follow from it — a `.tap` (ROM header
plus data block, auto-running from the program's first line) for the
Spectrum, a `.prg` at `$0801` for the C64.

**The listed form.** A source file is written exactly as the machine's own
`LIST` command would display it, line by line. That is the binding rule (see
Code198x `docs/specifications/unit.md`): the reader sees the same program on
the page and in the emulator, and it also pins down every space and blank
line, which is what the lint rules below check. On the Spectrum, for example,
a space typed around `=` is stored and `LIST` prints it, so it costs a byte
and shows on screen; the house style types none there, and the
`stored-space` rule flags one. `LIST` also prints a space after `CHR$`
before its argument, so the listed form has one:

```
before: 10 LET n = n + 1
after:    10 LET n=n+1

before: PRINT CHR$(147)
after:  PRINT CHR$ (147)
```

`build198x basic` runs every lint rule before it writes anything, so a
Makefile cannot build a listing that fails lint. `build198x basic lint`
reports each finding as `file:line:column: rule: message`:

| Rule | Machine | Catches |
|---|---|---|
| `listing-form` | both | a line that differs from its listed form (including a blank line — `LIST` never shows one); `--fix` rewrites it |
| `stored-space` | Spectrum | a space outside strings and `REM` that the ROM stores and lists, such as `LET n = n + 1`; `--fix` removes it. A space inside a numeric variable name (`my score`), which the ROM allows and ignores, is left alone |
| `string-var-name` | Spectrum | string variables longer than one letter (`name$`), which the ROM rejects |
| `keyword-var-name` | Spectrum | a variable named like a keyword (`ink`), which the tokeniser can turn into a token |
| `statement-keyword` | Spectrum | a statement that does not start with a command keyword, such as C64-style `GOTO 10` or `x=1` (the Spectrum needs `GO TO` and `LET`). The ROM's editor refuses the line and a tape built from it stops with C Nonsense in BASIC. A statement starts at the start of the line, after a `:` outside strings and `REM`, and after `THEN`; an empty statement (`::`, a trailing `:`) is fine |
| `keyword-in-name` | C64 | a keyword with name letters on both sides (`SCORE` holds `OR`), which BASIC V2 turns into a token, so it is not one variable. One-sided contact (`FORI`, `PRINTA`) is ordinary unspaced BASIC V2 and is not flagged |
| `var-name-clash` | C64 | variables that share their first two characters and type (`SCORE`, `SCALE`), which BASIC V2 treats as one |
| `line-order` | both | duplicate or descending line numbers |

`lint --fix` rewrites each listing to its listed form in place — mending
`listing-form` and `stored-space`, and dropping any blank line — then reports
what remains (a rule like `string-var-name` names a real syntax problem, not
a formatting one, so `--fix` leaves it for you to fix by hand). A line with a
`statement-keyword` finding is left exactly as written, since its listed
form would be refused too. `listing-form` and `stored-space` are not
reported on that line, because `--fix` will not apply them there; any other
finding on it still is. A file
already in listed form is left untouched, so a Makefile can run `lint --fix`
on every build without rewriting files that don't need it.

Where a tool and a machine's ROM disagree, **the ROM is the authority.** The
C64 tokeniser, for instance, stores `?` as the `PRINT` token the way the real
machine does; VICE's `petcat` does not, and build198x's corpus test
(`crates/build198x/tests/basic_corpus.rs`) documents that kind of divergence
explicitly rather than treating petcat as the reference.

## The roster and the gate

Three bands: **asset conversion** (graphics, audio, palettes), **data packing** (crunchers, level/tilemap packers), and **media mastering** (disk and tape images, bootable media — ADF, D64, TAP, and friends). It meets [Asm198x](https://github.com/asm198x) at the program-framing handoff: Asm198x emits the program, Build198x masters it onto media.

The wider roster stays demand-gated: each tool starts when its own concrete need fires, never speculatively. A **membership test** keeps the scope honest — a tool belongs here only if it *converts, packs, or masters build inputs into machine-ready data or media*. Assembly is Asm198x; emulation is Emu198x; cataloguing is Cat198x; playback of existing media is Play198x.

## Two rules, inherited from Asm198x

**Native output only, never a bespoke format.** And every tool is validated against reality: format encoders round-trip against reference tools and golden fixtures ([`decisions/validation-tiers.md`](decisions/validation-tiers.md)), contracted conversions are byte-identical across platforms ([`decisions/determinism-contract.md`](decisions/determinism-contract.md)), and the beeper's timing model is proven against hand-authored constants from shipped game code.

Build198x is the build-tools sibling of the 198x family, alongside Code198x, Emu198x, Asm198x, Cat198x, Forge198x, and Play198x.
