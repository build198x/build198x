# Decision: the determinism contract

**Status:** Active, binding for the conversion pipeline and CLI.

**Date:** 2026-06-11.

## The decision

1. **Byte-identical output across runs *and* platforms, for PNG input.** Same
   input file, flags, and converter version ⇒ identical native-format bytes on
   every supported target, enforced by a cross-platform golden CI job. JPEG and
   GIF inputs are accepted but **outside the byte-identical contract** — their
   decoders may take architecture-dependent paths; the report flags lossy/animated
   input as best-effort.

2. **Contracted code paths use deterministic maths only.** Basic IEEE float ops
   (`+ - * /`, comparisons) are bit-deterministic across platforms; libm
   transcendentals (`powf`, `cbrt`, `sin`) are not. Therefore:
   - sRGB→linear uses a **const 256-entry lookup table** (compile-time constants).
   - OKLab uses a **hand-rolled deterministic `cbrt`** (bit-pattern seed + Newton
     iterations — basic ops only), never `f32::cbrt`.
   - Resampling uses a **basic-arithmetic filter** (triangle/box); no
     Lanczos/sinc in contracted paths.
   - No `fast-math`, no FMA contraction (no `mul_add` in contracted paths), no
     parallelism in pixel-order-dependent passes (v1 is single-threaded there).

3. **Defined tie-breaks.** Equal colour distance ⇒ lowest palette index wins.
   Equal cell-candidate score ⇒ first candidate in enumeration order wins.
   Enumeration orders are documented in code and stable.

4. **Goldens move only on an explicit version bump.** Palette interpretations are
   content-versioned and frozen (see the umbrella `shared-media-spec.md` record);
   the default interpretation per machine is pinned per converter minor version.
   Preview PNGs are compared by **decoded pixels**, never by PNG bytes.

## Why

Golden tests in two repos (here and the Emu198x harness) hang off this contract;
an agent retry loop feeding the emulator nondeterministic bytes produces
unreproducible "failures" that burn the separate Emu198x session's time. Scoping
to PNG and banning transcendentals makes the contract *achievable* rather than
aspirational — adversarial plan review showed the original "all inputs, fix the
resampler" framing could not survive libm and SIMD-decode divergence.

## Drift triggers

- **"Use `powf`/`cbrt`/Lanczos here, it's prettier"** — not in a contracted path.
  LUT it, hand-roll it, or keep it out of the contract (preview-only).
- **"Parallelise the dither loop"** — only with an ordering proof and a golden
  re-verification on all targets.
- **"The golden changed slightly, just regenerate"** — goldens move on explicit
  version bumps only; an unexplained diff is a determinism bug.
