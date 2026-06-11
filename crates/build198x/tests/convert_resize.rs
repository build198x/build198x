//! Resize/gamma correctness: linear-light box resampling and identity
//! passthrough.

use build198x::convert::colour::SRGB_TO_LINEAR;
use build198x::convert::resize::{box_resample, fit_rect, letterbox, to_linear};
use build198x::convert::{LinearImage, Rgb8Image};

#[test]
fn checkerboard_downsample_averages_in_linear_light() {
    // A 2×2 black/white checkerboard box-filtered to 1×1 must average the
    // *linear* values — (0 + 1 + 1 + 0) / 4 = 0.5 — not the sRGB codes.
    let src = LinearImage {
        width: 2,
        height: 2,
        pixels: vec![
            [0.0, 0.0, 0.0],
            [1.0, 1.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 0.0, 0.0],
        ],
    };
    let out = box_resample(&src, 1, 1);
    for ch in 0..3 {
        assert!(
            (out.pixels[0][ch] - 0.5).abs() < 1e-6,
            "channel {ch} = {} not ~0.5",
            out.pixels[0][ch]
        );
    }
}

#[test]
fn srgb_128_decodes_to_known_linear_value() {
    // The canonical "mid grey is not 0.5" check through the public LUT.
    assert!((SRGB_TO_LINEAR[128] - 0.2158).abs() < 1e-4);
}

#[test]
fn at_target_dims_resample_is_byte_identical() {
    // Already-at-target input must skip resampling entirely: the f32
    // buffer equals the LUT-mapped input bit for bit.
    let img = Rgb8Image {
        width: 4,
        height: 3,
        pixels: (0..12u8).map(|i| [i * 20, 255 - i * 20, i]).collect(),
    };
    let linear = to_linear(&img);
    let out = box_resample(&linear, 4, 3);
    assert_eq!(out, linear);
}

#[test]
fn at_paper_dims_letterbox_is_byte_identical() {
    let mode = mediaspec::machine("sinclair-zx-spectrum")
        .expect("machine")
        .mode("standard")
        .expect("mode");
    let img = Rgb8Image {
        width: 256,
        height: 192,
        pixels: (0..256u32 * 192)
            .map(|i| {
                let v = u8::try_from(i % 251).expect("fits");
                [v, v.wrapping_mul(3), v.wrapping_add(7)]
            })
            .collect(),
    };
    let linear = to_linear(&img);
    let out = letterbox(&linear, mode, [0.0, 0.0, 0.0]);
    assert_eq!(out, linear);
}

#[test]
fn fit_rect_honours_c64_multicolour_pixel_aspect() {
    // C64 multicolour pixels are 2:1, so the 160×200 paper displays as
    // 320×200. A square 100×100 source must fill the height and take
    // 100·200/(100·2) = 100 mode pixels of width, centred at x = 30.
    let mode = mediaspec::machine("commodore-c64")
        .expect("machine")
        .mode("multicolour-bitmap")
        .expect("mode");
    let (w, h, ox, oy) = fit_rect(100, 100, mode);
    assert_eq!((w, h, ox, oy), (100, 200, 30, 0));
}

#[test]
fn fit_rect_letterboxes_a_wide_source() {
    // 512×100 onto the Spectrum's square-pixel 256×192: width binds,
    // height = 100·256/512 = 50, centred at y = (192−50)/2 = 71.
    let mode = mediaspec::machine("sinclair-zx-spectrum")
        .expect("machine")
        .mode("standard")
        .expect("mode");
    let (w, h, ox, oy) = fit_rect(512, 100, mode);
    assert_eq!((w, h, ox, oy), (256, 50, 0, 71));
}

#[test]
fn upscale_uses_the_box_filter_without_panic() {
    // A 2×1 source upscaled to 5×1: fractional coverage on every dest
    // pixel; weights must sum cleanly and interpolate the boundary pixel.
    let src = LinearImage {
        width: 2,
        height: 1,
        pixels: vec![[0.0, 0.0, 0.0], [1.0, 1.0, 1.0]],
    };
    let out = box_resample(&src, 5, 1);
    assert_eq!(out.pixels.len(), 5);
    assert_eq!(out.pixels[0], [0.0, 0.0, 0.0]);
    assert_eq!(out.pixels[4], [1.0, 1.0, 1.0]);
    // Middle dest pixel spans the source boundary: exact 50/50 mix.
    for ch in 0..3 {
        assert!((out.pixels[2][ch] - 0.5).abs() < 1e-6);
    }
}
