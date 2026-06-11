//! Integration tests for the Spectrum SCR codec: round-trips, the Smith
//! Figure 15-5 interleave spot checks, a golden byte fixture, and typed
//! error paths.

mod common;

use build198x::format::{DecodeError, EncodeError, scr};

/// A deterministic full-coverage screen: every linear bitmap byte and
/// attribute byte derived from its position.
fn patterned_screen() -> scr::Screen {
    let mut screen = scr::Screen::blank();
    for y in 0..scr::HEIGHT {
        for c in 0..scr::COLUMNS {
            screen.bitmap[y * scr::COLUMNS + c] = ((y ^ (c * 7)) & 0xFF) as u8;
        }
    }
    for (i, attr) in screen.attributes.iter_mut().enumerate() {
        *attr = (i % 256) as u8;
    }
    screen
}

#[test]
fn round_trip_two_cell_screen() {
    let mut screen = scr::Screen::blank();
    // Cell (0,0): solid ink. Cell (1,0): vertical stripes.
    for y in 0..8 {
        screen.bitmap[y * scr::COLUMNS] = 0xFF;
        screen.bitmap[y * scr::COLUMNS + 1] = 0xAA;
    }
    screen.attributes[0] = 0x47; // BRIGHT, white INK, black PAPER
    screen.attributes[1] = 0x16; // PAPER red, INK cyan

    let bytes = scr::encode(&screen).expect("encode");
    assert_eq!(bytes.len(), scr::FILE_LEN);
    let decoded = scr::decode(&bytes).expect("decode");
    assert_eq!(decoded, screen);
}

#[test]
fn round_trip_is_lossless_in_both_directions() {
    let screen = patterned_screen();
    let bytes = scr::encode(&screen).expect("encode");
    let decoded = scr::decode(&bytes).expect("decode");
    assert_eq!(decoded, screen);
    // The interleave is a permutation, so byte-level round-trip holds too.
    assert_eq!(scr::encode(&decoded).expect("re-encode"), bytes);
}

/// Interleave spot checks from
/// `syntheses/zx-spectrum/screen-and-attribute-memory.md` § 3:
/// offset = ((y & 0xC0) << 5) | ((y & 0x07) << 8) | ((y & 0x38) << 2) | c.
#[test]
fn bitmap_interleave_matches_documented_offsets() {
    // Row 0 starts the file.
    assert_eq!(scr::bitmap_file_offset(0, 0), Some(0x0000));
    // Next pixel row within a character cell is 256 bytes on (the INC H
    // stride, synthesis § 2).
    assert_eq!(scr::bitmap_file_offset(1, 0), Some(0x0100));
    // Next character row is 32 bytes on.
    assert_eq!(scr::bitmap_file_offset(8, 0), Some(0x0020));
    // The second 2 KB band starts at pixel row 64 (synthesis § 11, band
    // table: $4800 - $4000 = $0800).
    assert_eq!(scr::bitmap_file_offset(64, 0), Some(0x0800));
    // The last bitmap byte: row 191, column 31.
    assert_eq!(scr::bitmap_file_offset(191, 31), Some(0x17FF));
    // Out of range coordinates are rejected, not wrapped.
    assert_eq!(scr::bitmap_file_offset(192, 0), None);
    assert_eq!(scr::bitmap_file_offset(0, 32), None);
}

#[test]
fn encoded_bytes_land_at_interleaved_offsets() {
    let mut screen = scr::Screen::blank();
    screen.bitmap[scr::COLUMNS] = 0x5A; // linear row 1, column 0
    let bytes = scr::encode(&screen).expect("encode");
    assert_eq!(bytes[0x0100], 0x5A);
    assert_eq!(bytes[0x0000], 0x00);
}

/// The attribute table is linear at offset $1800 (synthesis § 6).
#[test]
fn attributes_are_linear_after_the_bitmap() {
    assert_eq!(scr::attribute_file_offset(0, 0), Some(0x1800));
    assert_eq!(scr::attribute_file_offset(23, 31), Some(0x1AFF));
    assert_eq!(scr::attribute_file_offset(24, 0), None);

    let mut screen = scr::Screen::blank();
    screen.attributes[scr::ATTRIBUTES_LEN - 1] = 0x38;
    let bytes = scr::encode(&screen).expect("encode");
    assert_eq!(bytes[scr::FILE_LEN - 1], 0x38);
}

#[test]
fn pixel_accessor_reads_msb_first() {
    let mut screen = scr::Screen::blank();
    screen.bitmap[0] = 0b1000_0001;
    assert_eq!(screen.pixel(0, 0), Some(true));
    assert_eq!(screen.pixel(1, 0), Some(false));
    assert_eq!(screen.pixel(7, 0), Some(true));
    assert_eq!(screen.pixel(256, 0), None);
    assert_eq!(screen.pixel(0, 192), None);
}

#[test]
fn golden_bytes_are_frozen() {
    let bytes = scr::encode(&patterned_screen()).expect("encode");
    common::assert_golden("pattern.scr", &bytes);
}

#[test]
fn decode_rejects_wrong_lengths() {
    for len in [0, scr::FILE_LEN - 1, scr::FILE_LEN + 1] {
        let err = scr::decode(&vec![0u8; len]).expect_err("must reject");
        assert!(
            matches!(err, DecodeError::WrongLength { actual, .. } if actual == len),
            "unexpected error for length {len}: {err:?}"
        );
    }
}

#[test]
fn encode_rejects_wrong_section_lengths() {
    let mut screen = scr::Screen::blank();
    screen.bitmap.pop();
    assert!(matches!(
        scr::encode(&screen),
        Err(EncodeError::WrongLength {
            what: "SCR bitmap",
            ..
        })
    ));

    let mut screen = scr::Screen::blank();
    screen.attributes.push(0);
    assert!(matches!(
        scr::encode(&screen),
        Err(EncodeError::WrongLength {
            what: "SCR attributes",
            ..
        })
    ));
}
