# Decision: workspace crates, and when a format splits out

**Status:** Active, binding for workspace structure and naming.

**Date:** 2026-06-11.

## The workspace

Three crates:

- **`mediaspec198x`** — zero-dep spec data. Separate from day one because Emu198x
  consumes it by pinned git rev.
- **`build198x`** — the conversion pipeline and its CLI, a lib + bin crate.
- **`build198x-adf`** — a standalone ADF tool, a thin front-end over
  `format198x-commodore-amiga-adf` for users who want disk images without the
  pipeline.

Wave 1 shipped two of these, revising the plan's seven-crate structure under the
family's split-when-a-second-consumer-makes-it-real rule.

## Formats live in Format198x

`build198x` holds no byte-layout code. The screen codecs and the ADF writer are
published crates in [`format198x/format198x`](https://github.com/format198x/format198x),
consumed from crates.io like any external user would:

| Module alias | Crate |
|---|---|
| `format::scr` | `format198x-sinclair-zx-spectrum-scr` |
| `format::koala` | `format198x-commodore-c64-koala` |
| `format::art_studio` | `format198x-commodore-c64-art-studio` |
| `format::ilbm` | `format198x-commodore-amiga-ilbm` |
| — | `format198x-commodore-amiga-adf` |

The `format::*` module paths are kept as re-export aliases so call sites read
unchanged. There is no shared `DecodeError`/`EncodeError`: Format198x crates are
dependency-free and cannot share a type, so call sites convert with
`.to_string()` via `Display`.

## What triggers a split

A **second consumer that is real**, which is either:

- another sibling project consuming the codec (Play198x, which
  fired the screen-codec split); or
- a **committed** public crates.io audience. The tools exist in their own right
  and should be usable by anyone — see
  [`../../../decisions/family-tools-are-general.md`](../../../decisions/family-tools-are-general.md).
  The bar is a committed audience with a plausible consumer, not a hypothetical
  one; the same bar the licensing-split record sets for publishing.

Neither licenses a pre-emptive split.

**A format crate joins `format198x/format198x` directly, never `build198x`.**
Fitting an independently-versioned published library into a binary workspace
took four workarounds — an independent version bolted onto lockstep,
`git_tag_enable=false` to dodge cargo-dist, a publish guard for the shared
release tag, and an unproven git-only bump. That friction is the reason the
library workspace exists: per-package versioning, no cargo-dist, OIDC publish.

**Naming:** `format198x-{manufacturer}-{system}-{format}`. Formats are always
namespaced by system because retro extensions collide across machines (ADF, DSK,
TAP). The org prefix is bound by
[`../../../decisions/crate-naming.md`](../../../decisions/crate-naming.md) and is
*added* rather than replacing a category word — an Emu198x format crate becomes
`emu198x-format-*`, not `emu198x-*`.

A disk-image library is not a pixel codec and need not match the codecs' surface:
`format198x-commodore-amiga-adf` carries its own `Error` type and a multi-file
API. The naming discipline still binds.

## Module dependency discipline

`format::*` re-exports depend on nothing but `core`/`std` — not on
`mediaspec198x`, not on the pipeline. If a codec wants spec data, the layering is
wrong.

## Drift triggers

- **"The codec needs a peek at the spec/pipeline"** — no; codecs take
  already-constrained indexed data.
- **"Split a codec crate out pre-emptively"** — wait for a real consumer, either
  a sibling or a committed public audience.
- **"Keep the new format crate here and move it later"** — no; it goes straight
  to `format198x/format198x`. Keeping it here is the case that cost four
  workarounds.
- **"Name the split crate after the file extension alone"** — no; formats are
  namespaced by system and carry their org prefix.
