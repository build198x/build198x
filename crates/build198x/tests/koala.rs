//! Integration tests for the C64 Koala codec: bit-pair extraction against
//! the synthesis § 5 table, cell-major addressing, round-trips, a golden
//! fixture, and typed error paths.

mod common;

use build198x::format::{DecodeError, EncodeError, koala};

/// A deterministic full-coverage image.
fn patterned_koala() -> koala::Koala {
    let mut image = koala::Koala::blank();
    for (i, b) in image.bitmap.iter_mut().enumerate() {
        *b = ((i * 7) % 256) as u8;
    }
    for (i, b) in image.screen_ram.iter_mut().enumerate() {
        *b = (i % 256) as u8;
    }
    for (i, b) in image.color_ram.iter_mut().enumerate() {
        *b = (i % 16) as u8;
    }
    image.background = 6;
    image
}

/// Bit-pair extraction: leftmost pair in bits 7-6
/// (`syntheses/commodore-c64/vic-ii-screen-memory-modes-sprites.md` § 5).
#[test]
fn bit_pairs_extract_msb_first() {
    let mut image = koala::Koala::blank();
    image.bitmap[0] = 0b00_01_10_11;
    assert_eq!(image.bit_pair(0, 0), Some(0b00));
    assert_eq!(image.bit_pair(1, 0), Some(0b01));
    assert_eq!(image.bit_pair(2, 0), Some(0b10));
    assert_eq!(image.bit_pair(3, 0), Some(0b11));
    assert_eq!(image.bit_pair(160, 0), None);
    assert_eq!(image.bit_pair(0, 200), None);
}

/// Colour resolution per the synthesis § 5 multicolour-bitmap table:
/// %00 background, %01 screen upper nybble, %10 screen lower nybble,
/// %11 colour RAM nybble.
#[test]
fn color_indices_follow_the_bit_pair_table() {
    let mut image = koala::Koala::blank();
    image.bitmap[0] = 0b00_01_10_11;
    image.screen_ram[0] = 0xAB;
    image.color_ram[0] = 0x5C; // upper nybble floats on hardware
    image.background = 0x06;
    assert_eq!(image.color_index(0, 0), Some(0x6)); // %00 -> background
    assert_eq!(image.color_index(1, 0), Some(0xA)); // %01 -> screen upper
    assert_eq!(image.color_index(2, 0), Some(0xB)); // %10 -> screen lower
    assert_eq!(image.color_index(3, 0), Some(0xC)); // %11 -> colour RAM low
}

/// Cell-major bitmap addressing: 8 bytes per cell, 320 bytes per cell row.
#[test]
fn bitmap_addressing_is_cell_major() {
    assert_eq!(koala::bitmap_offset(0, 0), Some(0));
    assert_eq!(koala::bitmap_offset(0, 1), Some(1)); // next line, same cell
    assert_eq!(koala::bitmap_offset(4, 0), Some(8)); // next cell right
    assert_eq!(koala::bitmap_offset(0, 8), Some(320)); // next cell row
    assert_eq!(koala::bitmap_offset(159, 199), Some(7999)); // last byte
    assert_eq!(koala::bitmap_offset(160, 0), None);
}

#[test]
fn round_trip_known_bit_pairs() {
    let mut image = koala::Koala::blank();
    image.bitmap[0] = 0b11_10_01_00;
    image.bitmap[7999] = 0xE4;
    image.screen_ram[0] = 0x12;
    image.color_ram[999] = 0xF7; // verbatim high nybble must survive
    image.background = 0x0E;

    let bytes = koala::encode(&image).expect("encode");
    assert_eq!(bytes.len(), koala::FILE_LEN);
    assert_eq!(&bytes[0..2], &[0x00, 0x60]); // $6000 little-endian
    let decoded = koala::decode(&bytes).expect("decode");
    assert_eq!(decoded, image);
    assert_eq!(koala::encode(&decoded).expect("re-encode"), bytes);
}

#[test]
fn round_trip_patterned_image() {
    let image = patterned_koala();
    let decoded = koala::decode(&koala::encode(&image).expect("encode")).expect("decode");
    assert_eq!(decoded, image);
}

#[test]
fn golden_bytes_are_frozen() {
    let bytes = koala::encode(&patterned_koala()).expect("encode");
    common::assert_golden("pattern.koa", &bytes);
}

#[test]
fn decode_rejects_wrong_lengths() {
    for len in [0, koala::FILE_LEN - 1, koala::FILE_LEN + 1] {
        let err = koala::decode(&vec![0u8; len]).expect_err("must reject");
        assert!(
            matches!(err, DecodeError::WrongLength { actual, .. } if actual == len),
            "unexpected error for length {len}: {err:?}"
        );
    }
}

#[test]
fn decode_rejects_wrong_load_address() {
    let mut bytes = koala::encode(&koala::Koala::blank()).expect("encode");
    bytes[1] = 0xA0; // $A000 instead of $6000
    assert!(matches!(
        koala::decode(&bytes),
        Err(DecodeError::BadMagic { .. })
    ));
}

#[test]
fn encode_rejects_wrong_section_lengths() {
    let mut image = koala::Koala::blank();
    image.bitmap.truncate(7999);
    assert!(matches!(
        koala::encode(&image),
        Err(EncodeError::WrongLength {
            what: "Koala bitmap",
            ..
        })
    ));

    let mut image = koala::Koala::blank();
    image.screen_ram.push(0);
    assert!(matches!(
        koala::encode(&image),
        Err(EncodeError::WrongLength {
            what: "Koala screen RAM",
            ..
        })
    ));

    let mut image = koala::Koala::blank();
    image.color_ram.truncate(999);
    assert!(matches!(
        koala::encode(&image),
        Err(EncodeError::WrongLength {
            what: "Koala colour RAM",
            ..
        })
    ));
}
