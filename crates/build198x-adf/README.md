# build198x-adf

A small command-line tool to **read and write Commodore Amiga `.adf` floppy disk
images**: master a hunk executable into a bootable disk, build arbitrary volumes,
verify a disk's integrity, and inspect its contents.

It's a thin front-end over the [`format-commodore-amiga-adf`] library:
deterministic, dependency-light, OFS and FFS.

## Install

```sh
cargo install build198x-adf
```

## Use

```sh
# master a hunk executable into a bootable disk (OFS boots on a bare A500/KS1.3):
build198x-adf mygame -o mygame.adf
build198x-adf mygame -o mygame.adf --ffs      # FFS: denser, needs Kickstart 2.0+

# build an arbitrary volume — files, directories, bootable or not:
build198x-adf create data.adf --label Data --add readme.txt --add logo.iff=art/logo.iff --mkdir docs
build198x-adf create game.adf --add mygame=c/mygame --startup mygame   # bootable

# check integrity (exit 0 sound, 1 corrupt) and inspect contents:
build198x-adf verify mygame.adf
build198x-adf info mygame.adf
```

Output is human-readable by default; pass `--format json` on any verb for a
single machine-readable line. Disks are written atomically.

## Verbs

- **`master <exe> -o <out.adf>`** — one hunk executable → a bootable disk that
  runs straight into your program (a generated `s/startup-sequence`). The bare
  `build198x-adf <exe> -o <out.adf>` is shorthand for this.
- **`create <out.adf>`** — assemble a volume from `--add host[=dest]` files and
  `--mkdir` directories; `--bootable` / `--startup <cmd>` for a boot disk.
- **`verify <disk.adf>`** — deep checksum + structure check.
- **`info <disk.adf>`** — label, filesystem, and root listing.

## Notes

- **OFS boots on any Kickstart** (including 1.3); **FFS needs 2.0+** — the 1.3
  ROM's floppy filesystem is OFS-only.
- **Deterministic** — the same inputs always produce identical bytes.
- Assembling a hunk executable is a separate step (any Amiga assembler, e.g.
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
