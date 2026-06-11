//! The spec-driven image-conversion pipeline: decoded image in, constrained
//! indexed pixels out, ready for the [`crate::format`] codecs.
//!
//! Stages, one module each:
//!
//! 1. [`normalise`] — alpha compositing over a matte, 8-bit sRGB widening,
//!    dimension sanity cap.
//! 2. [`colour`] — deterministic colour maths: the const sRGB→linear LUT,
//!    the hand-rolled `cbrt`, OKLab, and the [`colour::Metric`] distance
//!    functions.
//! 3. [`resize`] — letterboxing onto the target mode's paper grid honouring
//!    pixel aspect ratio, with hand-rolled linear-light box resampling.
//! 4. [`constrain`] — the per-cell exhaustive, mixing-aware constraint
//!    search (Spectrum attributes, C64 hires/multicolour).
//!    [`quantise`] — deterministic median-cut palette generation for
//!    free-palette (Amiga planar) targets. [`dither`] — ordered Bayer
//!    rendering restricted to each cell's chosen colours, plus serpentine
//!    error diffusion for free-palette targets.
//! 5. [`pipeline`] — the public entry: [`pipeline::Options`] in,
//!    [`pipeline::Conversion`] out, with bridges to the `format::*` input
//!    structs.
//!
//! **Binding rules.** This tree is a contracted path under
//! `decisions/determinism-contract.md`: byte-identical output across runs
//! and platforms for PNG input. Concretely: basic IEEE float ops only (no
//! libm transcendentals — enforced by a source-grep test), const-LUT sRGB
//! decoding, hand-rolled deterministic `cbrt`, box-filter resampling,
//! single-threaded pixel passes, and documented tie-breaks (lowest palette
//! index on equal distance; first candidate in enumeration order on equal
//! cell score). Validation tier (`decisions/validation-tiers.md`):
//! determinism goldens plus emulator-load — there is no external reference
//! implementation to byte-diff against.
//!
//! Layering (`decisions/module-and-crate-naming.md`): `convert::*` may
//! depend on `mediaspec` and `image`; `format::*` stays dependency-free and
//! receives already-constrained indexed data from here.

pub mod colour;
pub mod constrain;
pub mod dither;
pub mod normalise;
pub mod pipeline;
pub mod quantise;
pub mod resize;

/// An 8-bit-per-channel sRGB image with alpha already composited — the
/// output of [`normalise`] and the input to the linear-light stages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rgb8Image {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Row-major `[r, g, b]` triples, `width × height` entries.
    pub pixels: Vec<[u8; 3]>,
}

/// A linear-light RGB image (channel values 0.0–1.0, decoded through
/// [`colour::SRGB_TO_LINEAR`]). All resampling, quantisation, and mixing
/// arithmetic happens in this space.
#[derive(Debug, Clone, PartialEq)]
pub struct LinearImage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Row-major linear `[r, g, b]` triples, `width × height` entries.
    pub pixels: Vec<[f32; 3]>,
}

/// Why a conversion was rejected. Every invalid input or option maps to one
/// of these variants; the pipeline never panics on user input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConvertError {
    /// Input dimensions exceed [`normalise::MAX_DIMENSION`]; checked before
    /// any pipeline allocation.
    DimensionsTooLarge {
        /// Declared width in pixels.
        width: u32,
        /// Declared height in pixels.
        height: u32,
        /// The cap that was exceeded.
        max: u32,
    },
    /// Input pixel count (`width × height`) exceeds
    /// [`normalise::MAX_PIXELS`]; checked before any pipeline allocation.
    TooManyPixels {
        /// Declared width in pixels.
        width: u32,
        /// Declared height in pixels.
        height: u32,
        /// The total-pixel cap that was exceeded.
        max_pixels: u64,
    },
    /// The input has zero width or height.
    EmptyImage,
    /// No machine in the spec carries this id.
    UnknownMachine {
        /// The id requested.
        machine: String,
    },
    /// The machine exists but has no mode of this name.
    UnknownMode {
        /// The machine id.
        machine: String,
        /// The mode name requested.
        mode: String,
    },
    /// The machine has no palette interpretation of this name (always the
    /// case for gamut machines, whose palettes are generated per image).
    UnknownInterpretation {
        /// The machine id.
        machine: String,
        /// The interpretation name requested.
        name: String,
    },
    /// Dither strength outside the supported 0..=64 range.
    InvalidStrength {
        /// The strength requested.
        strength: u8,
    },
    /// Error diffusion requested for a cell-constrained mode — serpentine
    /// diffusion is supported for free-palette (planar) targets only,
    /// because diffused error cannot respect attribute-cell boundaries.
    DiffusionNeedsFreePalette,
    /// A `format::*` bridge was called on a conversion for a different
    /// machine or mode.
    WrongTarget {
        /// The bridge that was called.
        bridge: &'static str,
        /// The machine/mode the bridge requires.
        expected: &'static str,
        /// The conversion's actual machine/mode.
        actual: String,
    },
    /// An internal invariant failed — a bug in this pipeline, not bad input.
    Internal {
        /// What went wrong.
        what: &'static str,
    },
}

impl core::fmt::Display for ConvertError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::DimensionsTooLarge { width, height, max } => {
                write!(
                    f,
                    "input dimensions {width}x{height} exceed the {max} sanity cap"
                )
            }
            Self::TooManyPixels {
                width,
                height,
                max_pixels,
            } => {
                write!(
                    f,
                    "input dimensions {width}x{height} exceed the {max_pixels} total-pixel cap"
                )
            }
            Self::EmptyImage => write!(f, "input image has zero width or height"),
            Self::UnknownMachine { machine } => write!(f, "unknown machine id: {machine}"),
            Self::UnknownMode { machine, mode } => {
                write!(f, "machine {machine} has no mode named {mode}")
            }
            Self::UnknownInterpretation { machine, name } => {
                write!(
                    f,
                    "machine {machine} has no palette interpretation named {name}"
                )
            }
            Self::InvalidStrength { strength } => {
                write!(f, "dither strength {strength} outside 0..=64")
            }
            Self::DiffusionNeedsFreePalette => {
                write!(
                    f,
                    "error diffusion is only supported for free-palette (planar) targets"
                )
            }
            Self::WrongTarget {
                bridge,
                expected,
                actual,
            } => {
                write!(f, "{bridge} requires a {expected} conversion, got {actual}")
            }
            Self::Internal { what } => write!(f, "internal pipeline error: {what}"),
        }
    }
}

impl std::error::Error for ConvertError {}
