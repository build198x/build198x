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
