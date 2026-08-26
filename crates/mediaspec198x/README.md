# mediaspec198x

The 198x family's **authored media capability spec**: a declarative, per-machine
description of what retro display hardware can show. Screen modes (paper
geometry, pixel aspect, cell grids, bitplane counts), the per-cell constraint
rules a converter has to satisfy, and the palette model.

It is *authored* from the family's primary reference library — manuals,
datasheets, service guides — not extracted from an emulator. The one labelled
exception is the `emu198x-*` palette interpretations, which transcribe
Emu198x's own tables by design and cite the emulator source as provenance.

## Zero dependencies, const Rust

Everything is `&'static` data, so a whole machine description is a compile-time
constant: no dependencies, no allocation, diffable in review. That is
load-bearing, not tidiness — Emu198x consumes this crate to validate its
renderers and must never inherit an image or palette dependency graph.

## Palettes

Fixed-palette machines (Spectrum, C64) carry **named interpretations** that are
content-versioned and frozen: a published name such as `emu198x-v1` or
`pepto-v1` never changes its values. A corrected table gets a new name
(`emu198x-v2`), never an edit — goldens depend on the freeze. Free-palette
machines (Amiga OCS) carry a parametric gamut instead.

```rust
let spectrum = mediaspec198x::machine("sinclair-zx-spectrum").unwrap();
let mode = spectrum.mode("standard").unwrap();
let palette = spectrum.default_palette().unwrap();
```

Format crates expose palette *indices*; combining an index with an
interpretation from this crate to produce RGBA is the consumer's job.

## The binding decision

`198x/decisions/shared-media-spec.md` in the 198x umbrella governs this crate:
what belongs in it, the zero-dependency rule, and the palette freeze.
