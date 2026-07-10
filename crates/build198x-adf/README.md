# build198x-adf

A tiny command-line tool that masters a Commodore Amiga **bootable `.adf`
floppy** from a Kickstart-1.x hunk executable — the disk an Amiga boots straight
into your program.

It's a thin front-end over the [`format-commodore-amiga-adf`] library:
deterministic, dependency-light, OFS and FFS.

## Install

```sh
cargo install build198x-adf
```

## Use

```sh
# OFS (boots on a bare A500 / KS1.3) — the default:
build198x-adf mygame.exe -o mygame.adf

# FFS (denser; needs Kickstart 2.0+):
build198x-adf mygame.exe -o mygame.adf --ffs
```

The on-disk file name defaults to the executable's basename, and the volume
label to that name capitalised; override with `--name` and `--volume`. Output is
a one-line JSON summary; the `.adf` is written atomically.

```
build198x-adf <exe> -o <out.adf> [--volume <label>] [--name <file>] [--ofs|--ffs]
```

## Notes

- **OFS boots on any Kickstart** (including 1.3); **FFS needs 2.0+** — the 1.3
  ROM's floppy filesystem is OFS-only.
- **Deterministic** — the same inputs always produce identical bytes.
- Assembling the hunk executable is a separate step (any Amiga assembler, e.g.
  `vasm -Fhunkexe`); this tool takes the executable and writes the disk.

## Part of the 198x family

`build198x-adf` is the standalone twin of the `build198x adf` subcommand in
[Build198x], the 198x family's build-tools pipeline. The disk-format logic lives
in [`format-commodore-amiga-adf`]; install this if you just want the ADF tool
without the rest of the pipeline.

## Licence

GPL-2.0-or-later.

[`format-commodore-amiga-adf`]: https://crates.io/crates/format-commodore-amiga-adf
[Build198x]: https://github.com/build198x/build198x
