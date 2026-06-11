//! Sinclair ZX Spectrum `.scr` screen-dump codec.
//!
//! An SCR file is a raw dump of the Spectrum's 6,912-byte display file:
//! 6,144 bytes of bitmap followed by 768 attribute bytes, in the machine's
//! native memory order ($4000–$5AFF on hardware ⇒ file offset 0–$1AFF here).
//!
//! Layout facts are authored from
//! `syntheses/zx-spectrum/screen-and-attribute-memory.md` (silicon canon:
//! Chris Smith, *The ZX Spectrum ULA*, Chs 12 + 15):
//!
//! - The bitmap is 256×192 pixels, 1 bit per pixel, MSB leftmost: the pixel
//!   within a byte is bit `7 - (x & 7)` (§ 3).
//! - Bitmap bytes are **interleaved**: the file offset of pixel row `y`,
//!   byte column `c` is
//!   `((y & 0xC0) << 5) | ((y & 0x07) << 8) | ((y & 0x38) << 2) | c`
//!   (§ 3, Smith Figure 15-5). This yields three 2 KB bands of 64 pixel rows
//!   ($0000/$0800/$1000), with consecutive pixel rows of one character cell
//!   256 bytes apart (the `INC H` stride, § 2).
//! - The attribute table is **linear**: 24 rows × 32 cells, one byte per
//!   8×8 cell, at file offset 6,144 = $1800 (§ 6).
//! - Attribute byte: bits 2–0 INK, bits 5–3 PAPER, bit 6 BRIGHT, bit 7
//!   FLASH (§ 7).
//!
//! The decoded [`Screen`] stores the bitmap **de-interleaved** into linear
//! row-major order (pixel row 0 first, 32 bytes per row); [`encode`]
//! re-applies the interleave. The mapping is a pure permutation of the 6,144
//! bitmap offsets, so encode and decode are lossless in both directions.

use super::{DecodeError, EncodeError};

/// Bitmap section length in bytes (256×192 ÷ 8).
pub const BITMAP_LEN: usize = 6144;
/// Attribute section length in bytes (32×24 cells).
pub const ATTRIBUTES_LEN: usize = 768;
/// Total SCR file length in bytes.
pub const FILE_LEN: usize = BITMAP_LEN + ATTRIBUTES_LEN;
/// Screen width in pixels.
pub const WIDTH: usize = 256;
/// Screen height in pixels.
pub const HEIGHT: usize = 192;
/// Byte columns per pixel row (and attribute cells per cell row).
pub const COLUMNS: usize = 32;
/// Attribute cell rows.
pub const ATTRIBUTE_ROWS: usize = 24;

/// File offset of the bitmap byte for pixel row `y` (0..192), byte column
/// `column` (0..32) — the Smith Figure 15-5 interleave with the $4000 base
/// removed. Returns `None` when either coordinate is out of range.
///
/// Deliberately public beyond this crate's own needs: this module mirrors
/// the standalone codec crate it would become for the anticipated Play198x
/// consumer (`decisions/module-and-crate-naming.md`), and random-access
/// offset lookup is part of that codec surface.
#[must_use]
pub fn bitmap_file_offset(y: usize, column: usize) -> Option<usize> {
    if y >= HEIGHT || column >= COLUMNS {
        return None;
    }
    Some(interleave_offset(y, column))
}

/// The Smith interleave for in-range coordinates — infallible, for the
/// encode/decode loops that iterate the full grid by construction.
fn interleave_offset(y: usize, column: usize) -> usize {
    ((y & 0xC0) << 5) | ((y & 0x07) << 8) | ((y & 0x38) << 2) | column
}

/// File offset of the attribute byte for cell row `row` (0..24), cell column
/// `column` (0..32). The attribute table is linear at offset $1800
/// (synthesis § 6). Returns `None` when either coordinate is out of range.
///
/// Deliberately public beyond this crate's own needs: this module mirrors
/// the standalone codec crate it would become for the anticipated Play198x
/// consumer (`decisions/module-and-crate-naming.md`), and random-access
/// offset lookup is part of that codec surface.
#[must_use]
pub fn attribute_file_offset(row: usize, column: usize) -> Option<usize> {
    if row >= ATTRIBUTE_ROWS || column >= COLUMNS {
        return None;
    }
    Some(BITMAP_LEN + row * COLUMNS + column)
}

/// A decoded Spectrum screen: de-interleaved bitmap plus linear attributes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Screen {
    /// Bitmap in **linear** row-major order: 192 pixel rows × 32 bytes,
    /// MSB = leftmost pixel of each byte. (The file order is interleaved;
    /// see the module docs.)
    pub bitmap: Vec<u8>,
    /// Attribute bytes, row-major 24×32, one per 8×8 cell.
    /// Bit layout `FBPPPIII`: FLASH, BRIGHT, PAPER 5–3, INK 2–0.
    pub attributes: Vec<u8>,
}

impl Screen {
    /// A blank screen: all pixels off, all attributes zero.
    #[must_use]
    pub fn blank() -> Self {
        Self {
            bitmap: vec![0; BITMAP_LEN],
            attributes: vec![0; ATTRIBUTES_LEN],
        }
    }

    /// The pixel at `(x, y)`, or `None` when out of range.
    #[must_use]
    pub fn pixel(&self, x: usize, y: usize) -> Option<bool> {
        if x >= WIDTH || y >= HEIGHT {
            return None;
        }
        let byte = self.bitmap.get(y * COLUMNS + x / 8)?;
        Some(byte & (0x80 >> (x & 7)) != 0)
    }
}

/// Encode a [`Screen`] into the 6,912-byte SCR file layout, applying the
/// bitmap interleave.
///
/// # Errors
///
/// [`EncodeError::WrongLength`] when `bitmap` is not 6,144 bytes or
/// `attributes` is not 768 bytes.
pub fn encode(screen: &Screen) -> Result<Vec<u8>, EncodeError> {
    if screen.bitmap.len() != BITMAP_LEN {
        return Err(EncodeError::WrongLength {
            what: "SCR bitmap",
            expected: BITMAP_LEN,
            actual: screen.bitmap.len(),
        });
    }
    if screen.attributes.len() != ATTRIBUTES_LEN {
        return Err(EncodeError::WrongLength {
            what: "SCR attributes",
            expected: ATTRIBUTES_LEN,
            actual: screen.attributes.len(),
        });
    }

    let mut out = vec![0u8; FILE_LEN];
    for y in 0..HEIGHT {
        for column in 0..COLUMNS {
            out[interleave_offset(y, column)] = screen.bitmap[y * COLUMNS + column];
        }
    }
    out[BITMAP_LEN..].copy_from_slice(&screen.attributes);
    Ok(out)
}

/// Decode a 6,912-byte SCR file, de-interleaving the bitmap into linear
/// row-major order.
///
/// # Errors
///
/// [`DecodeError::WrongLength`] when the input is not exactly 6,912 bytes.
pub fn decode(bytes: &[u8]) -> Result<Screen, DecodeError> {
    if bytes.len() != FILE_LEN {
        return Err(DecodeError::WrongLength {
            what: "SCR file",
            expected: "exactly 6912",
            actual: bytes.len(),
        });
    }

    let mut screen = Screen::blank();
    for y in 0..HEIGHT {
        for column in 0..COLUMNS {
            screen.bitmap[y * COLUMNS + column] = bytes[interleave_offset(y, column)];
        }
    }
    screen.attributes.copy_from_slice(&bytes[BITMAP_LEN..]);
    Ok(screen)
}
