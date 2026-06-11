# Decision: validation tiers — encoders are reference-backed, the pipeline is emulator-validated

**Status:** Active, binding for every codec and pipeline stage in this workspace.
Carries [`Asm198x/asm198x/decisions/assemble-io-model.md`](../../../Asm198x/asm198x/decisions/assemble-io-model.md)'s
correctness tiering into Build198x, as
[`decisions/build198x-build-tools.md`](../../../decisions/build198x-build-tools.md)
(umbrella) point 6 requires.

**Date:** 2026-06-11.

## The decision

Every output path carries a named correctness tier:

1. **Format encoders are reference-backed.** Golden byte fixtures frozen in the
   repo; round-trip (encode→decode→pixel-equal) tests; and cross-checks against a
   runnable reference tool where one exists — for ILBM, netpbm in **both
   directions** (`ppmtoilbm` output decoded by our decoder, *and* our encoder's
   output decoded by `ilbmtoppm`). Where packers legitimately differ (ByteRun1
   break-even choices), the diff is on the **decoded form**, not raw bytes.
   Reference tools are validation-time dependencies only: those tests are
   `#[ignore]`d and skip gracefully when the tool is absent.

2. **The conversion pipeline (quantise/dither/constrain) is algorithm-defined** —
   there is no external reference to byte-diff against. Its tier is: determinism
   goldens (same input + flags ⇒ same bytes, cross-platform, enforced in CI) plus
   **emulator-load validation** — converter output loaded and rendered by Emu198x,
   compared losslessly (see the harness issues on `emu198x/emu198x`).

## Why

Round-trip self-consistency cannot catch real-tool incompatibility — Asm198x's
`ASL A` lesson (`spec-conformance-and-fuzzing.md`): output that round-trips
through our own decoder can still be rejected by the tool that matters. Splitting
the tiers keeps "byte-identical against a reference" honest where a reference
exists, instead of quietly dropping it everywhere because it can't apply to the
dither pass.

## Drift triggers

- **"Round-trips fine, ship it"** — round-trip is one layer; a real reader
  (netpbm, the emulator, period tooling) must also accept the bytes.
- **"The cross-check only runs one direction"** — the encoder is the product;
  `ilbmtoppm`-decodes-our-output is the direction that guards it.
- **"Make the reference tool a build dependency"** — no; validation-time only,
  skip gracefully.
