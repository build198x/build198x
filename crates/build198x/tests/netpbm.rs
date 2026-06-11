//! Netpbm cross-checks for the ILBM codec, in both directions, per
//! `decisions/validation-tiers.md`:
//!
//! 1. `ppmtoilbm` output decoded by **our decoder** (their encoder, our
//!    decoder).
//! 2. **Our encoder's** output decoded by `ilbmtoppm` (our encoder, their
//!    decoder — the direction that guards the product).
//!
//! Diffs are on the **decoded pixel form**, not raw bytes, because packers
//! legitimately differ in ByteRun1 break-even choices.
//!
//! The netpbm tools are validation-time dependencies only: these tests are
//! `#[ignore]`d and skip gracefully (eprintln + return) when the tools are
//! not on PATH. Run with `cargo test -p build198x -- --ignored`.

use std::path::PathBuf;
use std::process::Command;

use build198x::format::ilbm::{self, Compression, Ilbm};

/// True when `tool` resolves on PATH (checked with `which`).
fn tool_available(tool: &str) -> bool {
    Command::new("which")
        .arg(tool)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// A scratch file path that is unique per test.
fn scratch(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("build198x-netpbm-{}-{name}", std::process::id()))
}

/// Minimal binary PPM (P6, maxval 255) parser: returns (width, height,
/// RGB triples).
#[allow(clippy::unwrap_used)] // test-only parser; panics are the failure signal
fn parse_ppm(bytes: &[u8]) -> (usize, usize, Vec<[u8; 3]>) {
    assert_eq!(&bytes[0..2], b"P6", "expected a raw PPM");
    let mut pos = 2;
    let mut fields = [0usize; 3];
    for field in &mut fields {
        // Skip whitespace and comments.
        loop {
            match bytes[pos] {
                b'#' => {
                    while bytes[pos] != b'\n' {
                        pos += 1;
                    }
                }
                b' ' | b'\t' | b'\r' | b'\n' => pos += 1,
                _ => break,
            }
        }
        let mut value = 0usize;
        while bytes[pos].is_ascii_digit() {
            value = value * 10 + usize::from(bytes[pos] - b'0');
            pos += 1;
        }
        *field = value;
    }
    let [width, height, maxval] = fields;
    assert_eq!(maxval, 255, "test PPMs use maxval 255");
    pos += 1; // single whitespace byte after maxval
    let data = &bytes[pos..pos + width * height * 3];
    let pixels = data.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();
    (width, height, pixels)
}

/// Serialise a binary PPM (P6, maxval 255).
fn write_ppm(width: usize, height: usize, pixels: &[[u8; 3]]) -> Vec<u8> {
    let mut out = format!("P6\n{width} {height}\n255\n").into_bytes();
    for rgb in pixels {
        out.extend_from_slice(rgb);
    }
    out
}

/// Expand a decoded ILBM to RGB via its palette.
fn ilbm_to_rgb(image: &Ilbm) -> Vec<[u8; 3]> {
    image
        .pixels
        .iter()
        .map(|&i| {
            *image
                .palette
                .get(usize::from(i))
                .unwrap_or_else(|| panic!("pixel index {i} outside palette"))
        })
        .collect()
}

/// The shared test image: 16x4, four colours. Palette values keep nonzero
/// low nibbles so ilbmtoppm's 4-bit-CMAP heuristic never rescales them.
fn test_image() -> Ilbm {
    let palette = vec![
        [0x00, 0x00, 0x00],
        [0xFF, 0x00, 0x7F],
        [0x12, 0x34, 0x56],
        [0xC3, 0x9A, 0x44],
    ];
    let pixels = (0..16 * 4).map(|i| ((i + i / 16) % 4) as u8).collect();
    Ilbm {
        width: 16,
        height: 4,
        n_planes: 2,
        palette,
        pixels,
        camg: 0,
    }
}

/// Direction 1: their encoder, our decoder.
#[test]
#[ignore = "needs netpbm on PATH; validation-time dependency only"]
fn ppmtoilbm_output_decodes_with_our_decoder() {
    if !tool_available("ppmtoilbm") {
        eprintln!("ppmtoilbm not on PATH; skipping the netpbm cross-check");
        return;
    }

    let reference = test_image();
    let rgb = ilbm_to_rgb(&reference);
    let ppm_path = scratch("in.ppm");
    std::fs::write(&ppm_path, write_ppm(16, 4, &rgb)).expect("write scratch PPM");

    let output = Command::new("ppmtoilbm")
        .arg(&ppm_path)
        .output()
        .expect("run ppmtoilbm");
    assert!(
        output.status.success(),
        "ppmtoilbm failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let decoded = ilbm::decode(&output.stdout).expect("our decoder reads ppmtoilbm output");
    assert_eq!(usize::from(decoded.width), 16);
    assert_eq!(usize::from(decoded.height), 4);
    assert_eq!(
        ilbm_to_rgb(&decoded),
        rgb,
        "pixel RGB must survive ppmtoilbm -> our decoder"
    );

    let _ = std::fs::remove_file(&ppm_path);
}

/// Direction 2: our encoder, their decoder — the direction that guards the
/// product.
#[test]
#[ignore = "needs netpbm on PATH; validation-time dependency only"]
fn ilbmtoppm_reads_our_encoder_output() {
    if !tool_available("ilbmtoppm") {
        eprintln!("ilbmtoppm not on PATH; skipping the netpbm cross-check");
        return;
    }

    let image = test_image();
    let expected_rgb = ilbm_to_rgb(&image);

    for (label, compression) in [
        ("uncompressed", Compression::None),
        ("byterun1", Compression::ByteRun1),
    ] {
        let iff_path = scratch(&format!("out-{label}.iff"));
        let bytes = ilbm::encode(&image, compression).expect("encode");
        std::fs::write(&iff_path, &bytes).expect("write scratch IFF");

        let output = Command::new("ilbmtoppm")
            .arg(&iff_path)
            .output()
            .expect("run ilbmtoppm");
        assert!(
            output.status.success(),
            "ilbmtoppm failed on {label}: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let (width, height, pixels) = parse_ppm(&output.stdout);
        assert_eq!((width, height), (16, 4), "{label}: dimensions");
        assert_eq!(
            pixels, expected_rgb,
            "{label}: pixel RGB must survive our encoder -> ilbmtoppm"
        );

        let _ = std::fs::remove_file(&iff_path);
    }
}
