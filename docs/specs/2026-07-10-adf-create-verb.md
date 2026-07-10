# ADF `create` verb: the general volume builder

**Status:** implemented (both twins, 2026-07-10).
**Scope:** both ADF twins — standalone `build198x-adf` and the `build198x adf`
subcommand — in lockstep, as with `master`/`verify`/`info`.
**Follows:** [`2026-07-10-adf-read-verbs.md`](2026-07-10-adf-read-verbs.md).

## Purpose

`master` is the convenience path: one hunk executable → a bootable game disk
with a generated `s/startup-sequence`. `create` is the general path — it exposes
the library's full `Volume` builder so anyone can assemble an arbitrary ADF:
multiple files, directories, a chosen label, OFS or FFS, bootable or not. No new
library capability (`Volume::add_file`/`add_dir`/`set_bootable`/`build` already
exist); this is the CLI surface over them.

`master` stays a distinct verb (curriculum's 90% path, and `capture.py` depends
on it). `create` is *not* an alias for it — different operation.

## Command surface

```
build198x-adf create <out.adf> [options]
  --label <name>           volume label (default: the output filename stem, capitalised)
  --add <host>[=<dest>]    add a host file (repeatable). <dest> is its on-disk path;
                           defaults to the host basename at the root. A <dest> ending
                           in `/` keeps the host basename inside that directory.
  --mkdir <dest>           create an empty directory (repeatable)
  --bootable               write a boot block (default: not bootable)
  --startup <cmd>          write s/startup-sequence running <cmd> (implies --bootable)
  --ofs | --ffs            filesystem (default --ofs)
  --format <text|json>     output format (default text)
```

`<out.adf>` is the sole positional (there is no input positional to confuse it
with). Order-independent with the flags.

### The `=` mapping

`--add <host>=<dest>` maps a host file to a disk path. `=` (not `:`) is the
separator so Windows drive-letter host paths (`C:\game.exe=c/boot`) work. Split
on the **first** `=`. Without `=`, `<dest>` is the host basename at the root. A
`<dest>` ending in `/` appends the host basename (`--add logo.iff=art/` →
`art/logo.iff`). Intermediate directories are created by the library
(`add_file("c/x", …)` creates `c`).

### Bootable / startup

`--bootable` writes the boot block (`set_bootable(true)`). `--startup <cmd>`
writes `<cmd>\n` to `s/startup-sequence` and implies `--bootable`. The user is
responsible for placing the referenced program (e.g. `--add mygame=c/mygame`);
`create` does not synthesise it — that is `master`'s job.

### Defaults & edge cases

- **Empty disk is valid:** `create blank.adf --label Blank` yields a formatted,
  empty, non-bootable volume. Useful; not an error.
- **Label default:** the output filename stem, first letter capitalised
  (`create game.adf` → `Game`), matching how `master` derives its volume label.
  Falls back to `Disk` if the stem is empty.
- **A disk-full or bad-path input** surfaces as the library's `Error`
  (`exit 1` + stderr), same as `master`.

## Output

Human by default, JSON via `--format json`, consistent with the other verbs.

- **text:** `game.adf: created — 880K OFS "Game", 3 files, 2 dirs, bootable`
  (the `, bootable` clause only when bootable).
- **json:** `{"tool":"adf","command":"create","output":"game.adf","filesystem":"ofs","label":"Game","bootable":true,"files":3,"dirs":2,"bytes":901120}`

`files` counts `--add` entries plus the generated `s/startup-sequence` (when
`--startup` is given); `dirs` counts `--mkdir` entries. These report what the
user asked for, not a recursive census.

## Exit codes

Unchanged: `0` ok, `1` runtime failure (unreadable input, disk full, bad dest,
write failure), `2` usage error.

## Code shape

A new `cmd_create` (standalone) / `adf_create` (pipeline) alongside the existing
verbs, dispatched by the leading `create` token. Argument parsing collects the
`--add`/`--mkdir` lists, then drives a `Volume`: `new(label, fs)`, optional
`set_bootable`, each `add_dir`/`add_file`, optional startup, `build()`. A pure
`create_text`/`create_json` formatter pair keeps output testable without files,
matching the verify/info pattern. The two twins duplicate the thin glue, as they
already do.

## Testing

Both crates:

- `create` a multi-file disk in memory, then `Disk::open` + `verify` it → sound;
  `list("")` shows the expected root entries.
- `--add host=dest` places a file at `dest`; a bare `--add host` lands at the
  basename; a `dest` ending `/` keeps the basename.
- `--startup` implies bootable and writes `s/startup-sequence`.
- Empty `create --label X` builds a valid empty volume.
- `create_json` / `create_text` output shape (incl. the bootable clause).
- Usage errors (`2`): missing `<out.adf>`, malformed `--add`.
- Determinism: the same inputs build byte-identical images.

## Deferred (YAGNI)

- `--protect <bits>` per-file protection (library has
  `add_file_with_protection`; default normal/readable is fine until needed).
- Reading a directory tree off the host in one flag (`--add-tree <dir>`).
- `list [<path>] [--recursive]` and file extraction (from the read-verbs spec).
