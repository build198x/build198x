//! The `build198x` library — the build-tools pipeline's reusable layers.
//!
//! Wave 1 ships the [`format`] codec tree (Spectrum SCR, C64 Koala, C64 Art
//! Studio hires, Amiga IFF/ILBM) and the [`convert`] pipeline (normalise →
//! linear-light resize → quantise → per-cell constraint search → dither →
//! indexed output, bridged into the codecs). The CLI wiring arrives in a
//! later unit; the binary target stays a stub until then.
//!
//! The [`beeper`] module is the audio lane's first tool (opened by
//! `decisions/demand-gate-beeper-phrases.md`): Spectrum beeper-phrase
//! notation in, audition WAV + phrase assembly out.
//!
//! The [`basic`] module (opened by `decisions/demand-gate-basic.md`) builds a
//! numbered BASIC listing into the file its machine loads, using the
//! Format198x tokenisers: a Spectrum `.tap` or a C64 `.prg`.
//!
//! Module layout and dependency discipline follow
//! `decisions/module-and-crate-naming.md`: codec modules mirror the crate
//! names they would become if a second consumer makes a split real, and
//! they depend on nothing but `core`/`std`; `convert::*` may depend on
//! `mediaspec198x` and `image`; `beeper::*` is `std`-only.

pub mod basic;
pub mod beeper;
pub mod convert;
pub mod format;
