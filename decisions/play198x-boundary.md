# Decision: the Play198x boundary — preview is a diagnostic, decode is scoped

**Status:** Active. Interprets the umbrella records
[`build198x-build-tools.md`](../../../decisions/build198x-build-tools.md) and
[`play198x-media-player.md`](../../../decisions/play198x-media-player.md) for this
workspace.

**Date:** 2026-06-11.

## The decision

1. **Preview output is a converter diagnostic, not a viewer.** The CLI may render
   a PNG preview *of its own conversion result* so a human (or agent) can inspect
   what was just produced. General-purpose rendering of existing Koala/SCR/ILBM
   files is Play198x's verb and stays out of this workspace — the drift trigger in
   the Build198x charter ("not a media player") is read exactly this way.

2. **Codecs carry decode as well as encode — deliberately.** Round-trip tests need
   decode anyway, and the decode halves are the natural shared layer Play198x
   later consumes (origin brainstorm R6: shared layers throughout). This is
   recorded sharing, not scope drift.

3. **Decode scope is bounded:** what our encoder emits, plus curated wild fixtures
   (period-produced files committed as decode-only test data with provenance).
   Robust arbitrary-wild-file decoding (truncated variants, packer quirks beyond
   the spec'd format) is Play198x's problem space, not this workspace's.

## Drift triggers

- **"Add a `view` subcommand"** — no; that's Play198x. Preview renders *the file
  just produced*, in the same invocation or from the report's output path.
- **"Harden the decoder for every wild file in TOSEC"** — out of scope; curated
  fixtures only. File gaps as Play198x roster notes instead.
