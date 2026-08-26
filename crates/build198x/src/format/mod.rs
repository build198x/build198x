//! Retro screen-format codecs — re-exported from the Format198x crates.
//!
//! These codecs lived here until Play198x became a second consumer and made
//! the split real, exactly as the previous version of this doc predicted.
//! They now live in `format198x/format198x/crates/`, are independently
//! versioned, and are published for use outside the family.
//!
//! The module paths are kept as aliases so call sites read unchanged:
//! `crate::format::scr::encode(..)` still resolves.
//!
//! **There is no longer a shared `DecodeError`/`EncodeError`.** Each crate
//! carries its own, because Format198x crates are dependency-free and cannot
//! share a type. Call sites convert with `.to_string()`, which works on any
//! of them via `Display`.

pub use format_commodore_amiga_ilbm as ilbm;
pub use format_commodore_c64_art_studio as art_studio;
pub use format_commodore_c64_koala as koala;
pub use format_sinclair_zx_spectrum_scr as scr;
