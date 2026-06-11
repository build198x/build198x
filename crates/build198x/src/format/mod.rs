//! Retro screen-format codecs — pure byte-layout encode/decode.
//!
//! Each submodule is one on-disk format: [`scr`] (Spectrum screen dump),
//! [`koala`] (C64 Koala Painter multicolour), [`art_studio`] (C64 OCP Art
//! Studio hires), and [`ilbm`] (Amiga EA-IFF-85 ILBM).
//!
//! Two binding rules from `decisions/module-and-crate-naming.md` shape this
//! tree:
//!
//! - **Modules mirror future crate names.** If a second consumer (Play198x)
//!   makes a split real, `format::scr` becomes
//!   `format-sinclair-zx-spectrum-scr`, and so on.
//! - **Codecs depend on nothing but `core`/`std`** — not `mediaspec`, not the
//!   pipeline. They take already-constrained indexed pixel data and
//!   produce/parse bytes. If a codec wants spec data, the layering is wrong.
//!
//! Validation tier (`decisions/validation-tiers.md`): encoders are
//! reference-backed — golden byte fixtures frozen under
//! `tests/fixtures/golden/`, encode→decode round-trips, and (for ILBM)
//! netpbm cross-checks in both directions as `#[ignore]`d validation-time
//! tests.
//!
//! Malformed input never panics: every decode failure is a typed
//! [`DecodeError`], every rejected encode input a typed [`EncodeError`].

pub mod art_studio;
pub mod ilbm;
pub mod koala;
pub mod scr;

/// Why a byte stream failed to decode.
///
/// Every malformed input maps to one of these variants; codecs never panic
/// on input bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// The input is not a size the fixed-layout format allows.
    WrongLength {
        /// Which format/section was being decoded.
        what: &'static str,
        /// Human-readable statement of the allowed size(s).
        expected: &'static str,
        /// The size actually supplied.
        actual: usize,
    },
    /// The input ended before the named structure was complete.
    Truncated {
        /// The structure that ran past the end of the input.
        what: &'static str,
    },
    /// A magic number / signature did not match the format.
    BadMagic {
        /// The signature that failed to match.
        what: &'static str,
    },
    /// A header field holds a value this codec does not support.
    Unsupported {
        /// The field in question.
        what: &'static str,
        /// The value found.
        value: u32,
    },
    /// Declared dimensions exceed the sanity cap ([`ilbm::MAX_DIMENSION`]).
    DimensionsTooLarge {
        /// Declared width in pixels.
        width: u16,
        /// Declared height in pixels.
        height: u16,
    },
    /// A chunk the format requires never appeared.
    MissingChunk {
        /// The four-character chunk ID, as ASCII.
        id: &'static str,
    },
    /// The input is structurally inconsistent — e.g. a compressed run that
    /// overruns its scanline, or a chunk whose payload contradicts its size.
    Corrupt {
        /// What was inconsistent.
        what: &'static str,
    },
}

impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::WrongLength {
                what,
                expected,
                actual,
            } => {
                write!(f, "{what}: expected {expected} bytes, got {actual}")
            }
            Self::Truncated { what } => write!(f, "input truncated inside {what}"),
            Self::BadMagic { what } => write!(f, "bad magic: {what} did not match"),
            Self::Unsupported { what, value } => {
                write!(f, "unsupported {what}: {value}")
            }
            Self::DimensionsTooLarge { width, height } => {
                write!(
                    f,
                    "declared dimensions {width}x{height} exceed the sanity cap"
                )
            }
            Self::MissingChunk { id } => write!(f, "required chunk {id} missing"),
            Self::Corrupt { what } => write!(f, "corrupt input: {what}"),
        }
    }
}

impl std::error::Error for DecodeError {}

/// Why an encode input was rejected.
///
/// Encoders validate their input structs instead of panicking or silently
/// masking out-of-range values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodeError {
    /// A buffer in the input struct is not the length the format requires.
    WrongLength {
        /// Which buffer was the wrong length.
        what: &'static str,
        /// The required length.
        expected: usize,
        /// The length actually supplied.
        actual: usize,
    },
    /// A field or pixel value is outside the format's representable range.
    ValueOutOfRange {
        /// The field or value in question.
        what: &'static str,
        /// The offending value.
        value: u32,
        /// The smallest allowed value.
        min: u32,
        /// The largest allowed value.
        max: u32,
    },
}

impl core::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::WrongLength {
                what,
                expected,
                actual,
            } => {
                write!(f, "{what}: expected {expected} bytes, got {actual}")
            }
            Self::ValueOutOfRange {
                what,
                value,
                min,
                max,
            } => {
                write!(f, "{what} = {value} outside allowed range {min}..={max}")
            }
        }
    }
}

impl std::error::Error for EncodeError {}
