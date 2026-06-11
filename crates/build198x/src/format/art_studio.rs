//! Commodore 64 OCP Art Studio hires (`.art`) codec.
//!
//! File layout — canonical files are 9,009 bytes total:
//!
//! | offset | length | section |
//! |-------:|-------:|---------|
//! | 0      | 2      | load address, little-endian, always $2000 |
//! | 2      | 8,000  | bitmap (hires, cell-major) |
//! | 8,002  | 1,000  | screen RAM (per-cell colour pair) |
//! | 9,002  | 7      | trailing pad (filler, not image data) |
//!
//! The hires (standard bitmap mode) bit semantics are authored from
//! `syntheses/commodore-c64/vic-ii-screen-memory-modes-sprites.md` § 5
//! ("Standard bitmap mode"):
//!
//! | bitmap bit | colour source |
//! |------------|---------------|
//! | `0`        | screen RAM **lower** nybble |
//! | `1`        | screen RAM **upper** nybble |
//!
//! The bitmap is cell-major exactly as in [`super::koala`]: 8 consecutive
//! bytes per 8×8 cell, 40 cells (320 bytes) per cell row; the byte for pixel
//! `(x, y)` (x in 0..320) sits at `(y / 8) * 320 + (x / 8) * 8 + (y % 8)`,
//! and the pixel is bit `7 - (x % 8)` (MSB leftmost).
//!
//! **Provenance note:** the container layout — the $2000 load address and
//! the 9,009-byte canonical length (7 trailing bytes after screen RAM) — is
//! community knowledge from the C64 file-format literature; no local
//! `reference/` corroboration was found, so a provenance entry is a tracked
//! follow-up alongside the Koala one. **Trailing-pad policy:** [`encode`]
//! always emits the canonical 9,009 bytes, zero-padding the tail (the pad
//! carries no image data, so zeros are the deterministic choice); [`decode`]
//! accepts any length from 9,002 (pad absent) to 9,009 (canonical) and
//! **ignores** the trailing bytes rather than preserving them — they are
//! filler, and preserving them would make decode→encode round-trips depend
//! on bytes with no meaning.

use super::{DecodeError, EncodeError};

/// The load address every Art Studio hires file declares at offset 0.
pub const LOAD_ADDRESS: u16 = 0x2000;
/// Bitmap section length in bytes.
pub const BITMAP_LEN: usize = 8000;
/// Screen RAM section length in bytes.
pub const SCREEN_RAM_LEN: usize = 1000;
/// Length of the trailing pad in a canonical file.
pub const TRAILING_PAD_LEN: usize = 7;
/// Minimum decodable file length (load address + bitmap + screen RAM).
pub const MIN_FILE_LEN: usize = 2 + BITMAP_LEN + SCREEN_RAM_LEN;
/// Canonical file length (with the 7-byte trailing pad).
pub const FILE_LEN: usize = MIN_FILE_LEN + TRAILING_PAD_LEN;
/// Pixels per row.
pub const WIDTH: usize = 320;
/// Pixel rows.
pub const HEIGHT: usize = 200;
/// Cells per cell row.
pub const CELL_COLUMNS: usize = 40;

/// Offset *within the bitmap section* of the byte holding pixel `(x, y)`,
/// `x` in 0..320, `y` in 0..200. Cell-major VIC-II fetch order (see module
/// docs). Returns `None` when out of range.
#[must_use]
pub fn bitmap_offset(x: usize, y: usize) -> Option<usize> {
    if x >= WIDTH || y >= HEIGHT {
        return None;
    }
    Some((y / 8) * (CELL_COLUMNS * 8) + (x / 8) * 8 + (y % 8))
}

/// A decoded Art Studio hires image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtStudio {
    /// 8,000 bitmap bytes, cell-major, MSB = leftmost pixel.
    pub bitmap: Vec<u8>,
    /// 1,000 screen RAM bytes: upper nybble = colour of set pixels, lower
    /// nybble = colour of clear pixels, per cell in row-major cell order.
    pub screen_ram: Vec<u8>,
}

impl ArtStudio {
    /// A blank image: bitmap and screen RAM zeroed.
    #[must_use]
    pub fn blank() -> Self {
        Self {
            bitmap: vec![0; BITMAP_LEN],
            screen_ram: vec![0; SCREEN_RAM_LEN],
        }
    }

    /// The pixel at `(x, y)`, `x` in 0..320, `y` in 0..200, or `None` when
    /// out of range.
    #[must_use]
    pub fn pixel(&self, x: usize, y: usize) -> Option<bool> {
        let byte = self.bitmap.get(bitmap_offset(x, y)?)?;
        Some(byte & (0x80 >> (x % 8)) != 0)
    }

    /// The resolved 4-bit colour index for pixel `(x, y)`: screen RAM upper
    /// nybble when the bitmap bit is 1, lower nybble when 0 (synthesis § 5,
    /// standard bitmap mode). `None` when out of range.
    #[must_use]
    pub fn color_index(&self, x: usize, y: usize) -> Option<u8> {
        let set = self.pixel(x, y)?;
        let cell = self.screen_ram.get((y / 8) * CELL_COLUMNS + x / 8)?;
        Some(if set { cell >> 4 } else { cell & 0x0F })
    }
}

/// Encode an [`ArtStudio`] image into the canonical 9,009-byte file layout:
/// $2000 load address, bitmap, screen RAM, then 7 zero pad bytes.
///
/// # Errors
///
/// [`EncodeError::WrongLength`] when either section buffer is not its
/// required length.
pub fn encode(image: &ArtStudio) -> Result<Vec<u8>, EncodeError> {
    if image.bitmap.len() != BITMAP_LEN {
        return Err(EncodeError::WrongLength {
            what: "Art Studio bitmap",
            expected: BITMAP_LEN,
            actual: image.bitmap.len(),
        });
    }
    if image.screen_ram.len() != SCREEN_RAM_LEN {
        return Err(EncodeError::WrongLength {
            what: "Art Studio screen RAM",
            expected: SCREEN_RAM_LEN,
            actual: image.screen_ram.len(),
        });
    }

    let mut out = Vec::with_capacity(FILE_LEN);
    out.extend_from_slice(&LOAD_ADDRESS.to_le_bytes());
    out.extend_from_slice(&image.bitmap);
    out.extend_from_slice(&image.screen_ram);
    out.resize(FILE_LEN, 0);
    Ok(out)
}

/// Decode an Art Studio hires file. Accepts lengths 9,002 (no trailing pad)
/// through 9,009 (canonical); trailing bytes beyond the screen RAM are
/// ignored (see the module docs for the policy).
///
/// # Errors
///
/// - [`DecodeError::WrongLength`] when the input is outside 9,002..=9,009
///   bytes.
/// - [`DecodeError::BadMagic`] when the load address is not $2000.
pub fn decode(bytes: &[u8]) -> Result<ArtStudio, DecodeError> {
    if bytes.len() < MIN_FILE_LEN || bytes.len() > FILE_LEN {
        return Err(DecodeError::WrongLength {
            what: "Art Studio file",
            expected: "9002 to 9009",
            actual: bytes.len(),
        });
    }
    if bytes[0..2] != LOAD_ADDRESS.to_le_bytes() {
        return Err(DecodeError::BadMagic {
            what: "Art Studio load address ($2000 little-endian)",
        });
    }

    let bitmap_end = 2 + BITMAP_LEN;
    Ok(ArtStudio {
        bitmap: bytes[2..bitmap_end].to_vec(),
        screen_ram: bytes[bitmap_end..bitmap_end + SCREEN_RAM_LEN].to_vec(),
    })
}
