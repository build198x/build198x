//! The `build198x` library — the build-tools pipeline's reusable layers.
//!
//! Wave 1 ships the [`format`] codec tree: Spectrum SCR, C64 Koala, C64 Art
//! Studio (hires), and Amiga IFF/ILBM. The conversion pipeline
//! (quantise/dither/constrain) and the CLI wiring arrive in later units; the
//! binary target stays a stub until then.
//!
//! Module layout and dependency discipline follow
//! `decisions/module-and-crate-naming.md`: codec modules mirror the crate
//! names they would become if a second consumer makes a split real, and they
//! depend on nothing but `core`/`std`.

pub mod format;
