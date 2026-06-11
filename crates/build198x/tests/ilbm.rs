//! Integration tests for the Amiga IFF/ILBM codec: round-trips under both
//! compression settings, row/chunk padding spot checks, golden fixtures for
//! compression on and off, and typed error paths.

mod common;

use build198x::format::{DecodeError, EncodeError, ilbm};
use ilbm::{Compression, Ilbm};

/// A deterministic 17×5, 3-plane image with a 5-entry palette: the odd
/// width exercises word-boundary row padding, the 15-byte CMAP exercises
/// odd-length chunk padding.
fn patterned_ilbm() -> Ilbm {
    let width = 17u16;
    let height = 5u16;
    let pixels = (0..usize::from(width) * usize::from(height))
        .map(|i| ((i * 3 + i / 17) % 8) as u8)
        .collect();
    Ilbm {
        width,
        height,
        n_planes: 3,
        palette: vec![
            [0x00, 0x00, 0x00],
            [0xFF, 0xFF, 0xFF],
            [0xC2, 0x00, 0x00],
            [0x00, 0xC2, 0x00],
            [0x12, 0x34, 0x56],
        ],
        pixels,
        camg: 0,
        // The lores PAL aspect the encoder hardcoded before the fields
        // existed — keeps the frozen golden bytes valid.
        x_aspect: 10,
        y_aspect: 11,
    }
}

/// Find the payload of the first `id` chunk inside the FORM, returning
/// `(payload_offset, size)`.
fn find_chunk(bytes: &[u8], id: &[u8; 4]) -> Option<(usize, usize)> {
    let mut pos = 12;
    while pos + 8 <= bytes.len() {
        let size = u32::from_be_bytes([
            bytes[pos + 4],
            bytes[pos + 5],
            bytes[pos + 6],
            bytes[pos + 7],
        ]) as usize;
        if &bytes[pos..pos + 4] == id {
            return Some((pos + 8, size));
        }
        pos += 8 + size + (size & 1);
    }
    None
}

#[test]
fn round_trip_uncompressed() {
    let image = patterned_ilbm();
    let bytes = ilbm::encode(&image, Compression::None).expect("encode");
    let decoded = ilbm::decode(&bytes).expect("decode");
    assert_eq!(decoded, image);
}

#[test]
fn round_trip_byte_run1() {
    let image = patterned_ilbm();
    let bytes = ilbm::encode(&image, Compression::ByteRun1).expect("encode");
    let decoded = ilbm::decode(&bytes).expect("decode");
    assert_eq!(decoded, image);
}

#[test]
fn round_trip_preserves_camg_mode_bits() {
    let mut image = patterned_ilbm();
    image.camg = ilbm::CAMG_HIRES | ilbm::CAMG_LACE;
    let bytes = ilbm::encode(&image, Compression::None).expect("encode");
    let decoded = ilbm::decode(&bytes).expect("decode");
    assert_eq!(decoded.camg, ilbm::CAMG_HIRES | ilbm::CAMG_LACE);
}

/// BMHD `xAspect`/`yAspect` are written by encode and populated by decode
/// — non-default values survive the trip and land at BMHD bytes 14/15.
#[test]
fn round_trip_preserves_bmhd_pixel_aspect() {
    let mut image = patterned_ilbm();
    image.x_aspect = 5; // hires PAL
    image.y_aspect = 11;
    let bytes = ilbm::encode(&image, Compression::ByteRun1).expect("encode");
    let (bmhd, _) = find_chunk(&bytes, b"BMHD").expect("BMHD present");
    assert_eq!((bytes[bmhd + 14], bytes[bmhd + 15]), (5, 11));
    let decoded = ilbm::decode(&bytes).expect("decode");
    assert_eq!((decoded.x_aspect, decoded.y_aspect), (5, 11));
    assert_eq!(decoded, image);
}

/// Plane scanlines pad to a word boundary: 17 px -> 3 bytes -> 4; 9 px ->
/// 2 bytes -> 2; 8 px -> 1 byte -> 2.
#[test]
fn rows_pad_to_word_boundaries() {
    assert_eq!(ilbm::row_bytes(8), 2);
    assert_eq!(ilbm::row_bytes(9), 2);
    assert_eq!(ilbm::row_bytes(16), 2);
    assert_eq!(ilbm::row_bytes(17), 4);

    // An uncompressed BODY is exactly height x planes x row_bytes.
    let image = patterned_ilbm(); // 17 wide -> 4 bytes per plane scanline
    let bytes = ilbm::encode(&image, Compression::None).expect("encode");
    let (_, body_size) = find_chunk(&bytes, b"BODY").expect("BODY present");
    assert_eq!(body_size, 5 * 3 * 4);
}

/// Odd-length chunks carry a zero pad byte and the next chunk starts at an
/// even offset (synthesis § 10.1).
#[test]
fn odd_length_chunks_are_padded_to_even() {
    let image = patterned_ilbm(); // 5-entry palette -> 15-byte CMAP
    let bytes = ilbm::encode(&image, Compression::None).expect("encode");
    let (cmap_start, cmap_size) = find_chunk(&bytes, b"CMAP").expect("CMAP present");
    assert_eq!(cmap_size, 15);
    assert_eq!(bytes[cmap_start + 15], 0, "pad byte must be zero");
    assert_eq!(&bytes[cmap_start + 16..cmap_start + 20], b"CAMG");
    // The FORM size covers the whole stream after the 8-byte FORM header.
    let form_size = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
    assert_eq!(8 + form_size, bytes.len());
}

#[test]
fn byte_run1_actually_compresses_runs() {
    let image = Ilbm {
        width: 320,
        height: 16,
        n_planes: 2,
        palette: vec![[0, 0, 0], [0xFF, 0xFF, 0xFF]],
        pixels: vec![1; 320 * 16],
        camg: 0,
        x_aspect: 10,
        y_aspect: 11,
    };
    let raw = ilbm::encode(&image, Compression::None).expect("encode raw");
    let packed = ilbm::encode(&image, Compression::ByteRun1).expect("encode packed");
    assert!(
        packed.len() < raw.len() / 4,
        "constant image should pack well: raw {} vs packed {}",
        raw.len(),
        packed.len()
    );
    assert_eq!(ilbm::decode(&packed).expect("decode"), image);
}

#[test]
fn empty_palette_omits_the_cmap_chunk() {
    let mut image = patterned_ilbm();
    image.palette.clear();
    let bytes = ilbm::encode(&image, Compression::None).expect("encode");
    assert!(find_chunk(&bytes, b"CMAP").is_none());
    let decoded = ilbm::decode(&bytes).expect("decode");
    assert!(decoded.palette.is_empty());
}

#[test]
fn unknown_chunks_are_skipped() {
    let image = patterned_ilbm();
    let bytes = ilbm::encode(&image, Compression::ByteRun1).expect("encode");
    // Splice an ANNO chunk (odd-length, so with pad) right after "ILBM".
    let mut spliced = bytes[..12].to_vec();
    spliced.extend_from_slice(b"ANNO");
    spliced.extend_from_slice(&5u32.to_be_bytes());
    spliced.extend_from_slice(b"hello\0"); // 5 payload bytes + pad
    spliced.extend_from_slice(&bytes[12..]);
    let form_size = (spliced.len() - 8) as u32;
    spliced[4..8].copy_from_slice(&form_size.to_be_bytes());
    assert_eq!(ilbm::decode(&spliced).expect("decode"), image);
}

#[test]
fn trailing_bytes_after_the_form_are_ignored() {
    let image = patterned_ilbm();
    let mut bytes = ilbm::encode(&image, Compression::None).expect("encode");
    bytes.extend_from_slice(b"junk after the FORM");
    assert_eq!(ilbm::decode(&bytes).expect("decode"), image);
}

#[test]
fn golden_bytes_are_frozen_for_both_compressions() {
    let image = patterned_ilbm();
    let raw = ilbm::encode(&image, Compression::None).expect("encode raw");
    common::assert_golden("pattern-uncompressed.iff", &raw);
    let packed = ilbm::encode(&image, Compression::ByteRun1).expect("encode packed");
    common::assert_golden("pattern-byterun1.iff", &packed);
}

// --- error paths -----------------------------------------------------------

#[test]
fn decode_rejects_truncated_and_bad_magic_input() {
    assert!(matches!(
        ilbm::decode(&[]),
        Err(DecodeError::Truncated { .. })
    ));
    assert!(matches!(
        ilbm::decode(b"FORM"),
        Err(DecodeError::Truncated { .. })
    ));
    assert!(matches!(
        ilbm::decode(b"RIFF\0\0\0\x04WAVE"),
        Err(DecodeError::BadMagic { what: "FORM" })
    ));
    // A FORM whose declared size exceeds the available bytes.
    assert!(matches!(
        ilbm::decode(b"FORM\0\0\xFF\xFFILBM"),
        Err(DecodeError::Truncated { .. })
    ));
    // A well-formed FORM of the wrong type.
    assert!(matches!(
        ilbm::decode(b"FORM\0\0\0\x048SVX"),
        Err(DecodeError::BadMagic { what: "ILBM" })
    ));
}

#[test]
fn decode_rejects_bad_bmhd_fields() {
    let image = patterned_ilbm();
    let bytes = ilbm::encode(&image, Compression::None).expect("encode");
    let (bmhd, _) = find_chunk(&bytes, b"BMHD").expect("BMHD present");

    // nPlanes > 8.
    let mut bad = bytes.clone();
    bad[bmhd + 8] = 9;
    assert!(matches!(
        ilbm::decode(&bad),
        Err(DecodeError::Unsupported {
            what: "BMHD nPlanes",
            value: 9
        })
    ));

    // Zero width.
    let mut bad = bytes.clone();
    bad[bmhd..bmhd + 2].copy_from_slice(&0u16.to_be_bytes());
    assert!(matches!(
        ilbm::decode(&bad),
        Err(DecodeError::Unsupported {
            what: "BMHD width",
            ..
        })
    ));

    // Dimensions above the sanity cap.
    let mut bad = bytes.clone();
    bad[bmhd..bmhd + 2].copy_from_slice(&(ilbm::MAX_DIMENSION + 1).to_be_bytes());
    assert!(matches!(
        ilbm::decode(&bad),
        Err(DecodeError::DimensionsTooLarge { .. })
    ));

    // Unknown compression scheme.
    let mut bad = bytes.clone();
    bad[bmhd + 10] = 2;
    assert!(matches!(
        ilbm::decode(&bad),
        Err(DecodeError::Unsupported {
            what: "BMHD compression",
            value: 2
        })
    ));
}

#[test]
fn decode_rejects_truncated_body() {
    let image = patterned_ilbm();
    let bytes = ilbm::encode(&image, Compression::None).expect("encode");
    let (body, size) = find_chunk(&bytes, b"BODY").expect("BODY present");
    // Shrink the BODY payload by 2 bytes (keeping the chunk well-formed by
    // shrinking its declared size and the FORM size to match).
    let mut bad = bytes.clone();
    bad.truncate(body + size - 2);
    bad[body - 4..body].copy_from_slice(&u32::try_from(size - 2).expect("fits").to_be_bytes());
    let form_size = u32::try_from(bad.len() - 8).expect("fits");
    bad[4..8].copy_from_slice(&form_size.to_be_bytes());
    assert!(matches!(
        ilbm::decode(&bad),
        Err(DecodeError::Truncated { .. })
    ));
}

#[test]
fn decode_rejects_byte_run1_scanline_overrun() {
    // Hand-built minimal FORM: 8x1, 1 plane, ByteRun1 BODY whose single
    // control byte declares a 128-byte literal into a 2-byte scanline.
    let mut bmhd = vec![0u8; 20];
    bmhd[0..2].copy_from_slice(&8u16.to_be_bytes());
    bmhd[2..4].copy_from_slice(&1u16.to_be_bytes());
    bmhd[8] = 1; // nPlanes
    bmhd[10] = 1; // ByteRun1

    let mut form = b"ILBM".to_vec();
    form.extend_from_slice(b"BMHD");
    form.extend_from_slice(&20u32.to_be_bytes());
    form.extend_from_slice(&bmhd);
    form.extend_from_slice(b"BODY");
    form.extend_from_slice(&2u32.to_be_bytes());
    form.extend_from_slice(&[0x7F, 0xAA]); // 128-byte literal, then EOF

    let mut bytes = b"FORM".to_vec();
    bytes.extend_from_slice(&u32::try_from(form.len()).expect("fits").to_be_bytes());
    bytes.extend_from_slice(&form);

    assert!(matches!(
        ilbm::decode(&bytes),
        Err(DecodeError::Corrupt { .. })
    ));
}

#[test]
fn decode_requires_bmhd_and_body() {
    // BODY with no preceding BMHD.
    let mut form = b"ILBM".to_vec();
    form.extend_from_slice(b"BODY");
    form.extend_from_slice(&0u32.to_be_bytes());
    let mut bytes = b"FORM".to_vec();
    bytes.extend_from_slice(&u32::try_from(form.len()).expect("fits").to_be_bytes());
    bytes.extend_from_slice(&form);
    assert!(matches!(
        ilbm::decode(&bytes),
        Err(DecodeError::MissingChunk { id: "BMHD" })
    ));

    // BMHD but no BODY.
    let image = patterned_ilbm();
    let full = ilbm::encode(&image, Compression::None).expect("encode");
    let (body, _) = find_chunk(&full, b"BODY").expect("BODY present");
    let mut bytes = full[..body - 8].to_vec();
    let form_size = u32::try_from(bytes.len() - 8).expect("fits");
    bytes[4..8].copy_from_slice(&form_size.to_be_bytes());
    assert!(matches!(
        ilbm::decode(&bytes),
        Err(DecodeError::MissingChunk { id: "BODY" })
    ));
}

#[test]
fn encode_rejects_invalid_inputs() {
    let good = patterned_ilbm();

    let mut bad = good.clone();
    bad.width = 0;
    assert!(matches!(
        ilbm::encode(&bad, Compression::None),
        Err(EncodeError::ValueOutOfRange {
            what: "ILBM width",
            ..
        })
    ));

    let mut bad = good.clone();
    bad.height = ilbm::MAX_DIMENSION + 1;
    assert!(matches!(
        ilbm::encode(&bad, Compression::None),
        Err(EncodeError::ValueOutOfRange {
            what: "ILBM height",
            ..
        })
    ));

    let mut bad = good.clone();
    bad.n_planes = 9;
    assert!(matches!(
        ilbm::encode(&bad, Compression::None),
        Err(EncodeError::ValueOutOfRange {
            what: "ILBM nPlanes",
            ..
        })
    ));

    let mut bad = good.clone();
    bad.pixels.pop();
    assert!(matches!(
        ilbm::encode(&bad, Compression::None),
        Err(EncodeError::WrongLength {
            what: "ILBM pixels",
            ..
        })
    ));

    // A pixel index that needs more bits than n_planes provides.
    let mut bad = good.clone();
    bad.pixels[0] = 8; // n_planes = 3 -> max index 7
    assert!(matches!(
        ilbm::encode(&bad, Compression::None),
        Err(EncodeError::ValueOutOfRange {
            what: "ILBM pixel index",
            value: 8,
            ..
        })
    ));
}
