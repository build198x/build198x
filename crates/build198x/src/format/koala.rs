//! Commodore 64 Koala Painter (`.koa`) multicolour-bitmap codec.
//!
//! File layout — 10,003 bytes total:
//!
//! | offset | length | section |
//! |-------:|-------:|---------|
//! | 0      | 2      | load address, little-endian, always $6000 |
//! | 2      | 8,000  | bitmap (multicolour, cell-major) |
//! | 8,002  | 1,000  | screen RAM (colour sources %01 / %10) |
//! | 9,002  | 1,000  | colour RAM (colour source %11, low nybble) |
//! | 10,002 | 1      | background colour (colour source %00) |
//!
//! The multicolour bit-pair semantics are authored from
//! `syntheses/commodore-c64/vic-ii-screen-memory-modes-sprites.md` § 5
//! ("Multicolour bitmap mode"):
//!
//! | bit pair | colour source |
//! |----------|---------------|
//! | `%00`    | background colour ($D021) — the file's background byte |
//! | `%01`    | screen RAM **upper** nybble |
//! | `%10`    | screen RAM **lower** nybble |
//! | `%11`    | colour RAM nybble |
//!
//! Each bitmap byte holds four double-wide pixels, leftmost pair in bits
//! 7–6. The bitmap is stored exactly as the VIC-II fetches it: cell-major —
//! 8 consecutive bytes per 8×8 cell, 40 cells per cell row, so one cell row
//! spans 320 bytes and the byte for multicolour pixel `(x, y)` (x in 0..160)
//! sits at `(y / 8) * 320 + (x / 4) * 8 + (y % 8)`.
//!
//! **Provenance note:** the *container* layout (the $6000 load address,
//! section order, and trailing background byte totalling 10,003 bytes) is
//! community knowledge — Koala Painter's native save format, widely
//! documented in the C64 file-format literature. A `reference/` provenance
//! entry for it is a tracked follow-up. The bit-pair semantics above are
//! reference-backed via the synthesis. The cell-major fetch order is the
//! VIC-II's g-access addressing (`bitmap_base + VC×8 + RC`), part of the
//! same tracked provenance follow-up.
//!
//! Colour RAM on real hardware has only 4 significant bits (the upper
//! nybble floats); this codec **preserves bytes verbatim** in both
//! directions rather than masking, so wild files round-trip losslessly.

use super::{DecodeError, EncodeError};

/// The load address every Koala file declares, little-endian, at offset 0.
pub const LOAD_ADDRESS: u16 = 0x6000;
/// Bitmap section length in bytes.
pub const BITMAP_LEN: usize = 8000;
/// Screen RAM section length in bytes.
pub const SCREEN_RAM_LEN: usize = 1000;
/// Colour RAM section length in bytes.
pub const COLOR_RAM_LEN: usize = 1000;
/// Total Koala file length in bytes.
pub const FILE_LEN: usize = 2 + BITMAP_LEN + SCREEN_RAM_LEN + COLOR_RAM_LEN + 1;
/// Multicolour pixels per row (double-wide pixels).
pub const WIDTH: usize = 160;
/// Pixel rows.
pub const HEIGHT: usize = 200;
/// Cells per cell row.
pub const CELL_COLUMNS: usize = 40;

/// Offset *within the bitmap section* of the byte holding multicolour pixel
/// `(x, y)`, `x` in 0..160, `y` in 0..200. Cell-major VIC-II fetch order
/// (see module docs). Returns `None` when out of range.
#[must_use]
pub fn bitmap_offset(x: usize, y: usize) -> Option<usize> {
    if x >= WIDTH || y >= HEIGHT {
        return None;
    }
    Some((y / 8) * (CELL_COLUMNS * 8) + (x / 4) * 8 + (y % 8))
}

/// A decoded Koala image: the four sections, verbatim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Koala {
    /// 8,000 bitmap bytes, cell-major (see module docs).
    pub bitmap: Vec<u8>,
    /// 1,000 screen RAM bytes: upper nybble = colour for `%01`, lower
    /// nybble = colour for `%10`, per cell in row-major cell order.
    pub screen_ram: Vec<u8>,
    /// 1,000 colour RAM bytes: low nybble = colour for `%11`. The upper
    /// nybble is hardware-insignificant and preserved verbatim.
    pub color_ram: Vec<u8>,
    /// Background colour byte (colour for `%00`); only the low nybble is
    /// significant on hardware, preserved verbatim.
    pub background: u8,
}

impl Koala {
    /// A blank image: all sections zeroed.
    #[must_use]
    pub fn blank() -> Self {
        Self {
            bitmap: vec![0; BITMAP_LEN],
            screen_ram: vec![0; SCREEN_RAM_LEN],
            color_ram: vec![0; COLOR_RAM_LEN],
            background: 0,
        }
    }

    /// The bit pair (0–3) for multicolour pixel `(x, y)`, `x` in 0..160,
    /// `y` in 0..200; leftmost pair is bits 7–6 of its byte. `None` when
    /// out of range.
    #[must_use]
    pub fn bit_pair(&self, x: usize, y: usize) -> Option<u8> {
        let byte = self.bitmap.get(bitmap_offset(x, y)?)?;
        let shift = 6 - 2 * (x % 4);
        Some((byte >> shift) & 0b11)
    }

    /// The resolved 4-bit colour index for multicolour pixel `(x, y)`,
    /// applying the synthesis § 5 bit-pair table. `None` when out of range.
    #[must_use]
    pub fn color_index(&self, x: usize, y: usize) -> Option<u8> {
        let pair = self.bit_pair(x, y)?;
        let cell = (y / 8) * CELL_COLUMNS + x / 4;
        Some(match pair {
            0b00 => self.background & 0x0F,
            0b01 => self.screen_ram.get(cell)? >> 4,
            0b10 => self.screen_ram.get(cell)? & 0x0F,
            _ => self.color_ram.get(cell)? & 0x0F,
        })
    }
}

/// Encode a [`Koala`] into the 10,003-byte file layout, prefixing the $6000
/// load address.
///
/// # Errors
///
/// [`EncodeError::WrongLength`] when any section buffer is not its required
/// length.
pub fn encode(image: &Koala) -> Result<Vec<u8>, EncodeError> {
    if image.bitmap.len() != BITMAP_LEN {
        return Err(EncodeError::WrongLength {
            what: "Koala bitmap",
            expected: BITMAP_LEN,
            actual: image.bitmap.len(),
        });
    }
    if image.screen_ram.len() != SCREEN_RAM_LEN {
        return Err(EncodeError::WrongLength {
            what: "Koala screen RAM",
            expected: SCREEN_RAM_LEN,
            actual: image.screen_ram.len(),
        });
    }
    if image.color_ram.len() != COLOR_RAM_LEN {
        return Err(EncodeError::WrongLength {
            what: "Koala colour RAM",
            expected: COLOR_RAM_LEN,
            actual: image.color_ram.len(),
        });
    }

    let mut out = Vec::with_capacity(FILE_LEN);
    out.extend_from_slice(&LOAD_ADDRESS.to_le_bytes());
    out.extend_from_slice(&image.bitmap);
    out.extend_from_slice(&image.screen_ram);
    out.extend_from_slice(&image.color_ram);
    out.push(image.background);
    Ok(out)
}

/// Decode a 10,003-byte Koala file.
///
/// # Errors
///
/// - [`DecodeError::WrongLength`] when the input is not exactly 10,003
///   bytes.
/// - [`DecodeError::BadMagic`] when the load address is not $6000 — the
///   only signature a Koala file carries.
pub fn decode(bytes: &[u8]) -> Result<Koala, DecodeError> {
    if bytes.len() != FILE_LEN {
        return Err(DecodeError::WrongLength {
            what: "Koala file",
            expected: "exactly 10003",
            actual: bytes.len(),
        });
    }
    if bytes[0..2] != LOAD_ADDRESS.to_le_bytes() {
        return Err(DecodeError::BadMagic {
            what: "Koala load address ($6000 little-endian)",
        });
    }

    let bitmap_end = 2 + BITMAP_LEN;
    let screen_end = bitmap_end + SCREEN_RAM_LEN;
    let color_end = screen_end + COLOR_RAM_LEN;
    Ok(Koala {
        bitmap: bytes[2..bitmap_end].to_vec(),
        screen_ram: bytes[bitmap_end..screen_end].to_vec(),
        color_ram: bytes[screen_end..color_end].to_vec(),
        background: bytes[color_end],
    })
}
