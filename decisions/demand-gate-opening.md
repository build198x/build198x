# Decision: the demand gate is open — curriculum production is the triggering need

**Status:** Active. Satisfies the demand-gate requirement in
[`build198x-build-tools.md`](../../../decisions/build198x-build-tools.md) ("the
project starts when a concrete need appears, not before").

**Date:** 2026-06-11.

## The decision

Build198x starts now, on a named concrete need: **curriculum production still
uses external tools for graphics conversion.** Emu198x's screenshot/video capture
removed the capture-side pressure; the conversion-side gap (modern image → native
screen formats for unit assets: title screens, loading screens, in-game graphics)
has no family-owned tool. The 2026-06-11 tool-roster brainstorm
(`docs/brainstorms/2026-06-11-198x-tool-roster-requirements.md`) ranked the
image→native converter as the wave-1 lead — "the converter every machine track
touches, and the first real consumer of the capability-spec layer" — and the
wave-1 plan (`docs/plans/2026-06-11-001-feat-build198x-wave1-image-converter-plan.md`)
opens the gate deliberately on that basis.

Wave-1 scope: the `mediaspec` layer + image converter for the curriculum three
(Spectrum, C64, Amiga OCS), with Atari ST as a fast-follow gated on its format
documentation entering `reference/` with provenance (tracked as a follow-up
issue, not silent).

## What this does not open

The wider Build198x roster (sprite/charset/tilemap converters, crunchers, disk
mastering) stays demand-gated per the charter — each tool starts when its own
concrete need fires, citing this record's pattern.
