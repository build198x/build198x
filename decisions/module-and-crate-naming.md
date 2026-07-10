# Decision: two crates now; modules mirror the future crate names

**Status:** Active, binding for workspace structure and naming.

**Date:** 2026-06-11.

## The decision

1. **Two crates in wave 1:** `mediaspec` (zero-dep spec data — separate because
   Emu198x consumes it by pinned git rev from day 1) and `build198x` (everything
   else: codecs, pipeline, CLI — a lib + bin crate). This applies the family's
   split-when-a-second-consumer-makes-it-real rule (the `isa` crate's own
   deferral) and was confirmed by Steve on 2026-06-11, revising the wave-1 plan's
   seven-crate Output Structure.

2. **Codec modules mirror the crate names they would become.** Inside
   `build198x`: `format::scr`, `format::koala`, `format::art_studio`,
   `format::ilbm`. If/when Play198x consumes a codec, it splits out as
   `format-{manufacturer}-{system}-{format}` (`format-sinclair-zx-spectrum-scr`,
   `format-commodore-c64-koala`, `format-commodore-c64-art-studio`,
   `format-commodore-amiga-ilbm`) — adopting Emu198x's naming discipline
   (`Emu198x/knowledge/decisions/crate-naming.md`): retro extensions collide
   (DSK, TAP), so formats are always namespaced by system.

3. **Module dependency discipline holds by convention until crates enforce it:**
   `format::*` modules depend on nothing but `core`/`std` (not on `mediaspec`,
   not on the pipeline) — they are pure byte-layout code, exactly as they'd be as
   crates.

## Drift triggers

- **"The codec needs a peek at the spec/pipeline"** — no; codecs take
  already-constrained indexed data. If a codec wants spec data, the layering is
  wrong.
- **"Split a codec crate out pre-emptively"** — wait for the real consumer.
- **"Name the split crate after the file extension alone"** — no;
  `format-{manufacturer}-{system}-{format}`, always.

## Amendment (2026-07-10): an external audience is a real consumer

The "split when a second consumer makes it real" rule was written with *internal*
consumers in mind (Play198x pulling a codec). Steve extended it: **a public
crates.io audience the family commits to counts as that real consumer** — the
split need not wait for a second *sibling*. This follows from
[`../../../decisions/family-tools-are-general.md`](../../../decisions/family-tools-are-general.md)
(the tools exist in their own right and should be usable by anyone). It is not a
licence to split pre-emptively: the trigger is a *committed* audience with a
plausible consumer, not a hypothetical one — the same bar the licensing-split
record sets for publishing (`Emu198x/.../crate-licensing-split.md`, "publish
where there's a plausible consumer").

**First application — `format-commodore-amiga-adf`** (2026-07-10). The Amiga ADF
writer split out of `format::adf` under this amendment, keeping the convention
name. It is not a pixel codec but a disk-image/filesystem library (OFS now; FFS,
a general multi-file API, and the read side to follow), so its public surface is
richer than the codecs' encode/decode — it carries its own `Error` type rather
than the shared `format::EncodeError`. The naming discipline still binds
(system-namespaced: ADF/DSK/TAP collide across systems).

**The Emu198x tie this creates.** Writing an ADF is Build198x's domain; reading
one is more Emu198x's (it mounts floppies). Once the crate holds the read side,
Emu198x is its natural second consumer — consuming it by pinned git rev exactly
as it already consumes `mediaspec` and Asm198x's `isa-disasm`.

**The neutral home: `format198x` (reserved 2026-07-10).** The `format198x`
GitHub org was grabbed as the eventual home for the `format-{manufacturer}-
{system}-{format}` crate family — the direct analog of the reserved `isa198x`
org for the ISA/CPU-spec crates. A domain org (not a catch-all `lib198x`, which
would be the junk drawer the family's membership tests guard against) keeps the
grain: `format198x` alongside `isa198x`, each scoped. When it fills it will be a
workspace repo (`format198x/format198x`), mirroring `build198x/build198x`.

**Migration is deferred — reserve now, move when real.** Per the split-when-real
rule, `format-commodore-amiga-adf` **stays in the `build198x` workspace for
now** and publishes to crates.io from there; the org sits empty like `isa198x`.
The move to `format198x/format198x` fires when a *second* format crate (a D64,
TAP, or a split-out codec) makes the standalone workspace worth standing up, or
when Emu198x adopting the read side makes the neutral home concrete — whichever
comes first. Moving one crate today would be the pre-emptive split this rule
forbids. The crates.io name is independent of the org, so the first publish's
`repository` pointing at `build198x` is a cosmetic detail, corrected in a later
version at migration.
