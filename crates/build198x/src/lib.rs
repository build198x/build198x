//! The `build198x` library — the build-tools pipeline's reusable layers.
//!
//! Wave 1 ships the [`format`] codec tree (Spectrum SCR, C64 Koala, C64 Art
//! Studio hires, Amiga IFF/ILBM) and the [`convert`] pipeline (normalise →
//! linear-light resize → quantise → per-cell constraint search → dither →
//! indexed output, bridged into the codecs). The CLI wiring arrives in a
//! later unit; the binary target stays a stub until then.
//!
//! Module layout and dependency discipline follow
//! `decisions/module-and-crate-naming.md`: codec modules mirror the crate
//! names they would become if a second consumer makes a split real, and
//! they depend on nothing but `core`/`std`; `convert::*` may depend on
//! `mediaspec` and `image`.

pub mod convert;
pub mod format;
