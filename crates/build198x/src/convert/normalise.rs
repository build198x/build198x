//! Input normalisation: any decoded [`image::DynamicImage`] → an 8-bit
//! sRGB [`Rgb8Image`] with alpha composited over a matte.
//!
//! Scope notes (documented limitations, per the plan):
//!
//! - **Colour management:** the source is assumed to be sRGB. Embedded ICC
//!   profiles are ignored — the `image` crate decodes pixel data but does
//!   not apply profiles, and profile application would drag transcendental
//!   maths into a contracted path.
//! - **Animated inputs** (GIF) are out of this module's sight: the CLI
//!   decodes and passes the **first frame** only, already flattened into a
//!   `DynamicImage`.
//! - **Bit depth:** 16-bit channels narrow and grey/indexed inputs expand
//!   via `DynamicImage::to_rgba8` — integer-only, deterministic
//!   conversions.
//! - **Alpha compositing** happens in sRGB space with round-half-up integer
//!   arithmetic (`(c·a + m·(255−a) + 127) / 255`). Compositing in linear
//!   would be more physically correct, but the sRGB-space integer form is
//!   fully deterministic, matches what most tooling does, and the matte is
//!   black by default where the two agree exactly.

use super::{ConvertError, Rgb8Image};

/// Sanity cap on input width and height, in pixels — a per-axis bound on
/// degenerate geometry. It does **not** bound the total pixel count
/// (16384² is 268 megapixels); [`MAX_PIXELS`] does that. Both are checked
/// before any allocation this module performs.
pub const MAX_DIMENSION: u32 = 16384;

/// Sanity cap on the total pixel count (`width × height`). 64 megapixels
/// is generous for any plausible source photo while bounding the working
/// buffers this pipeline allocates. The CLI also enforces this cap from
/// the container header **before** the full decode; the check here is the
/// defensive backstop for library callers.
pub const MAX_PIXELS: u64 = 64_000_000;

/// Normalise a decoded image: dimension sanity checks, expand/narrow to
/// 8-bit RGBA, composite alpha over `matte` (sRGB, integer round-half-up).
///
/// # Errors
///
/// - [`ConvertError::EmptyImage`] when either dimension is zero.
/// - [`ConvertError::DimensionsTooLarge`] when either dimension exceeds
///   [`MAX_DIMENSION`] — returned before this module allocates anything.
/// - [`ConvertError::TooManyPixels`] when `width × height` exceeds
///   [`MAX_PIXELS`] — likewise returned before any allocation here.
pub fn normalise(img: &image::DynamicImage, matte: [u8; 3]) -> Result<Rgb8Image, ConvertError> {
    let (width, height) = (img.width(), img.height());
    if width == 0 || height == 0 {
        return Err(ConvertError::EmptyImage);
    }
    if width > MAX_DIMENSION || height > MAX_DIMENSION {
        return Err(ConvertError::DimensionsTooLarge {
            width,
            height,
            max: MAX_DIMENSION,
        });
    }
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(ConvertError::TooManyPixels {
            width,
            height,
            max_pixels: MAX_PIXELS,
        });
    }

    let rgba = img.to_rgba8();
    let pixels = rgba
        .pixels()
        .map(|p| {
            let [r, g, b, a] = p.0;
            [
                composite(r, matte[0], a),
                composite(g, matte[1], a),
                composite(b, matte[2], a),
            ]
        })
        .collect();

    Ok(Rgb8Image {
        width,
        height,
        pixels,
    })
}

/// `src` over `matte` at coverage `alpha`, sRGB-space integer
/// round-half-up. `alpha = 255` returns `src` exactly; `alpha = 0` returns
/// `matte` exactly.
fn composite(src: u8, matte: u8, alpha: u8) -> u8 {
    let a = u32::from(alpha);
    let blended = u32::from(src) * a + u32::from(matte) * (255 - a);
    u8::try_from((blended + 127) / 255).unwrap_or(u8::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composite_endpoints_are_exact() {
        for v in [0u8, 1, 127, 128, 254, 255] {
            assert_eq!(composite(v, 99, 255), v);
            assert_eq!(composite(v, 99, 0), 99);
        }
    }

    #[test]
    fn composite_half_alpha_rounds_half_up() {
        // 255·128/255 + 0·127/255 = 128 exactly at a = 128 over black.
        assert_eq!(composite(255, 0, 128), 128);
    }
}
