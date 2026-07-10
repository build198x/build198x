# ADF read verbs: `verify` and `info`

**Status:** design approved, not yet implemented.
**Scope:** both ADF twins — the standalone `build198x-adf` binary and the
`build198x adf` pipeline subcommand — kept in lockstep, as they already are for
`master`.

## Purpose

The ADF tools only *write* today: they master a hunk executable into a bootable
`.adf`. The `format-commodore-amiga-adf` library already exposes a complete,
panic-free read side (`Disk::open`, `label`, `filesystem`, `list`, `read`,
`verify`). This spec adds a *read* surface over it so the tool can answer "is
this disk sound?" and "what's on it?" — turning a build-only utility into a
proper little ADF tool, without any new library work or dependencies.

Two verbs now: `verify` (integrity/CI) and `info` (quick inspection). `list`
(arbitrary path, recursive) and file extraction are a deliberate fast-follow,
built only if reached for.

## Command surface

Both tools grow an explicit verb in front of the operation. The bare,
verb-less form is preserved as shorthand for `master` so nothing that calls the
0.2.0 interface — or the `build198x adf <exe> -o <adf>` form `capture.py` uses —
breaks.

```
build198x-adf <exe> -o <out.adf> [flags]          # implicit master (shorthand, unchanged)
build198x-adf master <exe> -o <out.adf> [flags]   # explicit master, identical
build198x-adf verify <disk.adf> [--format json]
build198x-adf info   <disk.adf> [--format json]
```

Dispatch: if the first argument is `master`, `verify`, `info`, `-h`, or
`--help`, run that verb; otherwise treat the whole argument list as an implicit
`master`. Top-level `--help` lists the verbs and notes the shorthand. The same
surface applies to `build198x adf …` (nested one level under the pipeline
tool).

## Output: human by default, JSON on demand

Kubernetes-style. Every verb prints **human-readable text by default** and
machine JSON only when asked. The selector is `--format <text|json>`
(default `text`) on every verb.

The selector is `--format`, not kubectl's literal `-o json`, because `-o` is
already `master`'s output-*file* flag (and `capture.py` depends on it). A single
unambiguous `--format` on every verb beats overloading `-o` to mean "file" for
one verb and "format" for another.

`--format json` reproduces the machine output; failures always go to stderr
with a non-zero exit regardless of format.

### `master`

- **text** (new default): `flock.adf: 880K OFS disk "Flock", flock (51234 bytes)`
- **json**: the current one-line object, unchanged —
  `{"tool":"adf","output":"...","volume":"...","file":"...","filesystem":"ofs","bytes":901120,"exe_bytes":51234}`

Nothing parses the current JSON line (the sole caller, `capture.py`, uses
`stdout=DEVNULL` + `check=True`), so flipping the default is safe. The JSON is
preserved verbatim behind `--format json`.

### `verify`

`Disk::open` then `Disk::verify()` — the deep checksum + structural pass.

- **success**: exit 0.
  - text: `disk.adf: OK — 880K OFS "MyGame"`
  - json: `{"tool":"adf","command":"verify","input":"disk.adf","filesystem":"ofs","label":"MyGame","result":"ok"}`
- **corrupt / not an ADF**: exit 1, reason on stderr —
  `build198x-adf: <reason>` (e.g. `boot checksum`,
  `image size (not an 880K DD floppy)`). The exit code is the machine contract.

### `info`

`Disk::open` + `label()` + `filesystem()` + `list("")` (root only — cheap, no
recursion). Entries sorted by name for stable output.

- **text**:
  ```
  disk.adf: 880K OFS "MyGame"
    NAME     KIND  SIZE
    c        dir      -
    mygame   file  51234
    s        dir      -
  ```
- **json**: `{"tool":"adf","command":"info","input":"disk.adf","filesystem":"ofs","label":"MyGame","bytes":901120,"entries":[{"name":"c","kind":"dir","size":0},{"name":"mygame","kind":"file","size":51234},{"name":"s","kind":"dir","size":0}]}`

A corrupt root surfaces as exit 1 + reason, same as `verify`.

## Exit codes

Unchanged from `master`, applied to every verb: `0` ok, `1` runtime/verify
failure, `2` usage error (bad or missing arguments; prints per-verb usage).

## Code shape

Refactor each tool's single-operation entry point (`run` in the standalone,
`adf_command` in the pipeline) into a small verb dispatch over
`cmd_master` / `cmd_verify` / `cmd_info`, leaving the existing master logic
intact inside `cmd_master`. A shared `--format` parse + a small text/JSON
formatter per verb.

The two twins keep duplicating the thin CLI glue exactly as they already
duplicate `master` — no shared CLI crate. That matches the established twin
precedent; a shared crate would be scope creep for three short functions.

## Testing

Unit tests in both crates:

- `verify` on a known-good image (round-trip `Volume` → bytes → `verify`) → exit 0.
- `verify` on a byte-flipped image → exit 1.
- `info` label / filesystem / entry-list / name-sort assertions.
- `--format json` output shape for each verb; `text` is the default.
- Usage-error exit codes (`2`) for missing/extra arguments.
- Dispatch: implicit `master` vs explicit `master`/`verify`/`info`.

Existing tests asserting `master`'s stdout are updated for the new text default
(and a `--format json` case added where JSON is asserted).

## Deferred (YAGNI)

- `list [<path>] [--recursive]` — directory listing; shallow by default (the
  same root view `info` gives), `--recursive` walks the whole tree. The verb
  surface and `--format` plumbing built here leave a clean home for it.
- file extraction (`read <disk> <path> [-o file]`).
- `--format yaml`, `-o wide`-style variants.

Built only when reached for.
