//! Integration tests for the C64 Art Studio hires codec: hires bit
//! semantics against the synthesis § 5 table, the trailing-pad policy,
//! round-trips, a golden fixture, and typed error paths.

mod common;

use build198x::format::{DecodeError, EncodeError, art_studio};

/// A deterministic full-coverage image.
fn patterned_art_studio() -> art_studio::ArtStudio {
    let mut image = art_studio::ArtStudio::blank();
    for (i, b) in image.bitmap.iter_mut().enumerate() {
        *b = ((i * 3) % 256) as u8;
    }
    for (i, b) in image.screen_ram.iter_mut().enumerate() {
        *b = (255 - (i % 256)) as u8;
    }
    image
}

/// Hires semantics from
/// `syntheses/commodore-c64/vic-ii-screen-memory-modes-sprites.md` § 5
/// ("Standard bitmap mode"): bit 1 -> screen upper nybble, bit 0 -> lower.
#[test]
fn pixels_resolve_to_screen_ram_nybbles() {
    let mut image = art_studio::ArtStudio::blank();
    image.bitmap[0] = 0b1000_0000;
    image.screen_ram[0] = 0x5A;
    assert_eq!(image.pixel(0, 0), Some(true));
    assert_eq!(image.pixel(1, 0), Some(false));
    assert_eq!(image.color_index(0, 0), Some(0x5)); // set -> upper nybble
    assert_eq!(image.color_index(1, 0), Some(0xA)); // clear -> lower nybble
    assert_eq!(image.pixel(320, 0), None);
    assert_eq!(image.pixel(0, 200), None);
}

/// Cell-major bitmap addressing, same fetch order as Koala.
#[test]
fn bitmap_addressing_is_cell_major() {
    assert_eq!(art_studio::bitmap_offset(0, 0), Some(0));
    assert_eq!(art_studio::bitmap_offset(0, 1), Some(1));
    assert_eq!(art_studio::bitmap_offset(8, 0), Some(8)); // next cell right
    assert_eq!(art_studio::bitmap_offset(0, 8), Some(320)); // next cell row
    assert_eq!(art_studio::bitmap_offset(319, 199), Some(7999));
    assert_eq!(art_studio::bitmap_offset(320, 0), None);
}

#[test]
fn encode_emits_canonical_9009_bytes_with_zero_pad() {
    let bytes = art_studio::encode(&patterned_art_studio()).expect("encode");
    assert_eq!(bytes.len(), art_studio::FILE_LEN);
    assert_eq!(&bytes[0..2], &[0x00, 0x20]); // $2000 little-endian
    assert_eq!(&bytes[art_studio::MIN_FILE_LEN..], &[0u8; 7]);
}

#[test]
fn decode_accepts_padless_and_canonical_lengths_and_ignores_the_pad() {
    let image = patterned_art_studio();
    let canonical = art_studio::encode(&image).expect("encode");

    // Canonical 9,009 bytes.
    assert_eq!(art_studio::decode(&canonical).expect("decode"), image);

    // Pad absent: 9,002 bytes.
    let padless = &canonical[..art_studio::MIN_FILE_LEN];
    assert_eq!(art_studio::decode(padless).expect("decode"), image);

    // Non-zero pad bytes are ignored, not preserved.
    let mut noisy = canonical.clone();
    for b in &mut noisy[art_studio::MIN_FILE_LEN..] {
        *b = 0xFF;
    }
    assert_eq!(art_studio::decode(&noisy).expect("decode"), image);
}

#[test]
fn round_trip_is_lossless() {
    let image = patterned_art_studio();
    let bytes = art_studio::encode(&image).expect("encode");
    let decoded = art_studio::decode(&bytes).expect("decode");
    assert_eq!(decoded, image);
    assert_eq!(art_studio::encode(&decoded).expect("re-encode"), bytes);
}

#[test]
fn golden_bytes_are_frozen() {
    let bytes = art_studio::encode(&patterned_art_studio()).expect("encode");
    common::assert_golden("pattern.art", &bytes);
}

#[test]
fn decode_rejects_wrong_lengths() {
    for len in [0, art_studio::MIN_FILE_LEN - 1, art_studio::FILE_LEN + 1] {
        let err = art_studio::decode(&vec![0u8; len]).expect_err("must reject");
        assert!(
            matches!(err, DecodeError::WrongLength { actual, .. } if actual == len),
            "unexpected error for length {len}: {err:?}"
        );
    }
}

#[test]
fn decode_rejects_wrong_load_address() {
    let mut bytes = art_studio::encode(&art_studio::ArtStudio::blank()).expect("encode");
    bytes[1] = 0x60; // $6000 (Koala's address) instead of $2000
    assert!(matches!(
        art_studio::decode(&bytes),
        Err(DecodeError::BadMagic { .. })
    ));
}

#[test]
fn encode_rejects_wrong_section_lengths() {
    let mut image = art_studio::ArtStudio::blank();
    image.bitmap.push(0);
    assert!(matches!(
        art_studio::encode(&image),
        Err(EncodeError::WrongLength {
            what: "Art Studio bitmap",
            ..
        })
    ));

    let mut image = art_studio::ArtStudio::blank();
    image.screen_ram.truncate(999);
    assert!(matches!(
        art_studio::encode(&image),
        Err(EncodeError::WrongLength {
            what: "Art Studio screen RAM",
            ..
        })
    ));
}
