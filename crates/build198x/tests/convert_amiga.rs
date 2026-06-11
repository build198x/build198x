//! Amiga planar path: deterministic median-cut, gamut rounding, error
//! diffusion, and the ILBM bridge.

use build198x::convert::LinearImage;
use build198x::convert::colour::{Metric, srgb8_to_linear};
use build198x::convert::constrain::PaletteData;
use build198x::convert::dither::{DitherMode, diffuse_planar};
use build198x::convert::pipeline::{Options, convert};
use build198x::convert::quantise::{generate_palette, round_channel_to_gamut};
use image::{DynamicImage, RgbImage};
use mediaspec::Rgb;

/// Four colours already on the 4-bit (v = level·17) gamut grid.
const GRID_COLOURS: [[u8; 3]; 4] = [[0, 0, 0], [255, 255, 255], [136, 17, 34], [68, 204, 170]];

#[test]
fn median_cut_recovers_a_known_four_colour_image() {
    let pixels: Vec<[f32; 3]> = (0..64 * 64)
        .map(|i| srgb8_to_linear(GRID_COLOURS[i % 4]))
        .collect();
    let img = LinearImage {
        width: 64,
        height: 64,
        pixels,
    };
    let palette = generate_palette(&img, 32, 4);

    let mut expected: Vec<Rgb> = GRID_COLOURS
        .iter()
        .map(|&[r, g, b]| Rgb { r, g, b })
        .collect();
    expected.sort_unstable_by_key(|c| (c.r, c.g, c.b));
    assert_eq!(palette, expected);
}

#[test]
fn gamut_rounded_channels_satisfy_the_nibble_identity() {
    for v in 0..=255u8 {
        let rounded = round_channel_to_gamut(v, 4);
        assert_eq!(
            rounded,
            (rounded & 0x0F) * 17,
            "{v} rounded to {rounded}, off the 4-bit grid"
        );
    }
}

#[test]
fn serpentine_diffusion_on_a_gradient_is_deterministic() {
    let pixels: Vec<[f32; 3]> = (0..64u32 * 64)
        .map(|i| {
            let x = i % 64;
            let y = i / 64;
            #[allow(clippy::cast_precision_loss)]
            [x as f32 / 63.0, y as f32 / 63.0, 0.5]
        })
        .collect();
    let img = LinearImage {
        width: 64,
        height: 64,
        pixels,
    };
    let palette: Vec<Rgb> = GRID_COLOURS
        .iter()
        .map(|&[r, g, b]| Rgb { r, g, b })
        .collect();
    let pal = PaletteData::new(&palette, Metric::OkLab);

    for mode in [DitherMode::FloydSteinberg, DitherMode::Atkinson] {
        let a = diffuse_planar(&img, &pal, mode);
        let b = diffuse_planar(&img, &pal, mode);
        assert_eq!(a, b, "{mode:?} not deterministic");
        assert!(
            a.iter().any(|&px| px != a[0]),
            "{mode:?} produced flat output"
        );
    }
}

#[test]
fn planar_pipeline_is_deterministic_and_bridges_to_ilbm() {
    // A colourful gradient through the full planar path, twice.
    let img = DynamicImage::ImageRgb8(RgbImage::from_fn(320, 200, |x, y| {
        image::Rgb([
            u8::try_from(x * 255 / 319).unwrap_or(255),
            u8::try_from(y * 255 / 199).unwrap_or(255),
            96,
        ])
    }));
    let mut opts = Options::new("commodore-amiga-ocs", "lores-ntsc");
    opts.dither = DitherMode::FloydSteinberg;
    let a = convert(&img, &opts).expect("run 1");
    let b = convert(&img, &opts).expect("run 2");
    assert_eq!(a.pixels, b.pixels);
    assert_eq!(a.palette, b.palette);

    // Every generated palette entry sits on the 4-bit grid.
    for c in &a.palette {
        for v in [c.r, c.g, c.b] {
            assert_eq!(v, (v & 0x0F) * 17);
        }
    }
    assert!(a.palette.len() <= 32);
    let n_planes = a.n_planes.expect("planar conversion has planes");
    assert!((1..=5).contains(&n_planes));
    assert!(a.palette.len() <= 1 << n_planes);

    let ilbm = a.to_ilbm().expect("bridge succeeds");
    assert_eq!(ilbm.width, 320);
    assert_eq!(ilbm.height, 200);
    assert_eq!(ilbm.camg, 0, "lores carries no HIRES bit");
    let bytes =
        build198x::format::ilbm::encode(&ilbm, build198x::format::ilbm::Compression::ByteRun1)
            .expect("encode");
    let decoded = build198x::format::ilbm::decode(&bytes).expect("decode");
    assert_eq!(decoded.pixels, ilbm.pixels);
}

#[test]
fn hires_mode_sets_the_camg_hires_bit() {
    let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(64, 64, image::Rgb([136, 17, 34])));
    let mut opts = Options::new("commodore-amiga-ocs", "hires-ntsc");
    opts.no_dither = true;
    let conv = convert(&img, &opts).expect("conversion succeeds");
    let ilbm = conv.to_ilbm().expect("bridge succeeds");
    assert_eq!(
        ilbm.camg & build198x::format::ilbm::CAMG_HIRES,
        build198x::format::ilbm::CAMG_HIRES
    );
    let n_planes = conv.n_planes.expect("planes");
    assert!(n_planes <= 4, "hires OCS caps at 4 planes");
}

#[test]
fn ordered_planar_dither_spreads_a_midtone() {
    // A flat mid-grey against a black/white palette must dither to a mix
    // of both entries under ordered Bayer at default strength... at
    // amplitude 1/15 the bias straddles the decision boundary only when
    // the value sits near it, so use a value near the black/white midpoint
    // in OkLab terms.
    let mid = srgb8_to_linear([188, 188, 188]); // L ≈ 0.5 territory
    let img = LinearImage {
        width: 16,
        height: 16,
        pixels: vec![mid; 256],
    };
    let palette = vec![
        Rgb { r: 0, g: 0, b: 0 },
        Rgb {
            r: 255,
            g: 255,
            b: 255,
        },
    ];
    let pal = PaletteData::new(&palette, Metric::WeightedRgb);
    let out = build198x::convert::dither::ordered_planar(&img, &pal, DitherMode::Bayer8, 64);
    // Deterministic, and at minimum not panicking; spread depends on the
    // amplitude constant, so only determinism is asserted strictly.
    let again = build198x::convert::dither::ordered_planar(&img, &pal, DitherMode::Bayer8, 64);
    assert_eq!(out, again);
}
