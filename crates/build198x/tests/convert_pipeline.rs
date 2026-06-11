//! Full-pipeline integration tests: identity, mixing-aware regression,
//! pathological degradation, determinism, options validation, and the
//! format bridges.

use build198x::convert::ConvertError;
use build198x::convert::colour::Metric;
use build198x::convert::dither::DitherMode;
use build198x::convert::pipeline::{CellChoice, Options, convert, default_dither};
use image::{DynamicImage, RgbImage};
use mediaspec::Rgb;

mod common;
use common::palette_of;

/// Build an image by mapping a per-pixel palette index to its exact RGB.
fn image_of_indices(
    palette: &[Rgb],
    width: u32,
    height: u32,
    index_at: impl Fn(u32, u32) -> u8,
) -> DynamicImage {
    DynamicImage::ImageRgb8(RgbImage::from_fn(width, height, |x, y| {
        let c = palette[usize::from(index_at(x, y))];
        image::Rgb([c.r, c.g, c.b])
    }))
}

#[test]
fn identity_spectrum_image_passes_through_unchanged() {
    let pal = palette_of("sinclair-zx-spectrum");
    // Every 8×8 cell: black + one normal colour (cycling 1..=7), in a
    // checker pattern — already exactly constraint-satisfying.
    let index_at = |x: u32, y: u32| -> u8 {
        let cell = (y / 8) * 32 + x / 8;
        let colour = u8::try_from(cell % 7).expect("fits") + 1;
        if (x + y).is_multiple_of(2) { colour } else { 0 }
    };
    let img = image_of_indices(pal, 256, 192, index_at);

    let mut opts = Options::new("sinclair-zx-spectrum", "standard");
    opts.strength = 0;
    let conv = convert(&img, &opts).expect("conversion succeeds");

    assert!(conv.report.already_constrained, "pre-pass must detect it");
    assert!(conv.report.mean_error < 1e-12);
    assert_eq!(conv.report.cells_over_threshold, 0);
    for y in 0..192u32 {
        for x in 0..256u32 {
            assert_eq!(
                conv.pixels[(y * 256 + x) as usize],
                index_at(x, y),
                "pixel ({x}, {y}) changed"
            );
        }
    }
}

#[test]
fn mixing_aware_search_dithers_an_out_of_gamut_cell() {
    // The plan review's killer case: a uniform orange image on the
    // Spectrum. Nearest-single-colour scoring would paint it flat; the
    // mixing-aware search must pick a mixable pair (red+yellow territory)
    // and the dithered cell must contain both colours.
    let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(256, 192, image::Rgb([255, 165, 0])));
    let opts = Options::new("sinclair-zx-spectrum", "standard");
    let conv = convert(&img, &opts).expect("conversion succeeds");

    let CellChoice::Spectrum(choice) = conv.cells[0] else {
        panic!("expected a Spectrum cell choice");
    };
    assert_ne!(choice.ink, choice.paper, "must choose a two-colour mix");

    let mut distinct: Vec<u8> = Vec::new();
    for y in 0..8usize {
        for x in 0..8usize {
            let px = conv.pixels[y * 256 + x];
            if !distinct.contains(&px) {
                distinct.push(px);
            }
        }
    }
    assert!(
        distinct.len() >= 2,
        "cell dithered flat ({distinct:?}) — mixing lost"
    );
}

#[test]
fn pathological_cell_degrades_deterministically_and_is_flagged() {
    // First cell: eight distinct saturated colours, one per row — more
    // than any Spectrum attribute cell can hold.
    let wild: [[u8; 3]; 8] = [
        [255, 0, 0],
        [0, 255, 0],
        [0, 0, 255],
        [255, 255, 0],
        [255, 0, 255],
        [0, 255, 255],
        [255, 128, 0],
        [128, 0, 255],
    ];
    let img = DynamicImage::ImageRgb8(RgbImage::from_fn(256, 192, |x, y| {
        if x < 8 && y < 8 {
            image::Rgb(wild[y as usize])
        } else {
            image::Rgb([0, 0, 0])
        }
    }));
    let opts = Options::new("sinclair-zx-spectrum", "standard");

    let first = convert(&img, &opts).expect("first run");
    let second = convert(&img, &opts).expect("second run");
    assert_eq!(first.pixels, second.pixels, "runs must be identical");
    assert_eq!(first.report, second.report);
    assert!(
        first.report.cells_over_threshold >= 1,
        "the impossible cell must be flagged (report: {:?})",
        first.report
    );
    assert!(!first.report.already_constrained);
}

#[test]
fn pipeline_is_deterministic_and_metric_sensitive() {
    // A colour gradient, letterboxed and searched twice per metric.
    let img = DynamicImage::ImageRgb8(RgbImage::from_fn(64, 64, |x, y| {
        image::Rgb([
            u8::try_from(x * 4).unwrap_or(255),
            u8::try_from(y * 4).unwrap_or(255),
            128,
        ])
    }));
    let opts = Options::new("sinclair-zx-spectrum", "standard");

    let a = convert(&img, &opts).expect("run 1");
    let b = convert(&img, &opts).expect("run 2");
    assert_eq!(a.pixels, b.pixels);
    assert_eq!(a.report, b.report);

    let mut weighted = opts.clone();
    weighted.metric = Metric::WeightedRgb;
    let c = convert(&img, &weighted).expect("weighted run 1");
    let d = convert(&img, &weighted).expect("weighted run 2");
    assert_eq!(c.pixels, d.pixels, "weighted metric must self-agree");
    assert_ne!(a.pixels, c.pixels, "metric swap should change the output");
}

#[test]
fn option_validation_rejects_bad_inputs() {
    let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(8, 8, image::Rgb([0, 0, 0])));

    let bad_machine = Options::new("acorn-electron", "standard");
    assert!(matches!(
        convert(&img, &bad_machine),
        Err(ConvertError::UnknownMachine { .. })
    ));

    let bad_mode = Options::new("commodore-c64", "ham8");
    assert!(matches!(
        convert(&img, &bad_mode),
        Err(ConvertError::UnknownMode { .. })
    ));

    let mut bad_strength = Options::new("commodore-c64", "hires-bitmap");
    bad_strength.strength = 65;
    assert!(matches!(
        convert(&img, &bad_strength),
        Err(ConvertError::InvalidStrength { strength: 65 })
    ));

    let mut bad_interp = Options::new("sinclair-zx-spectrum", "standard");
    bad_interp.interpretation = Some("not-a-palette".to_owned());
    assert!(matches!(
        convert(&img, &bad_interp),
        Err(ConvertError::UnknownInterpretation { .. })
    ));

    let mut diffusion_on_cells = Options::new("sinclair-zx-spectrum", "standard");
    diffusion_on_cells.dither = DitherMode::FloydSteinberg;
    assert!(matches!(
        convert(&img, &diffusion_on_cells),
        Err(ConvertError::DiffusionNeedsFreePalette)
    ));
}

#[test]
fn scr_bridge_packs_attributes_and_round_trips() {
    let pal = palette_of("sinclair-zx-spectrum");
    // Cell 0: black paper, bright red ink halves; rest black.
    let img = image_of_indices(pal, 256, 192, |x, y| {
        if x < 8 && y < 8 && (x + y) % 2 == 0 {
            10 // bright red
        } else {
            0
        }
    });
    let mut opts = Options::new("sinclair-zx-spectrum", "standard");
    opts.strength = 0;
    let conv = convert(&img, &opts).expect("conversion succeeds");

    let screen = conv.to_scr().expect("bridge succeeds");
    // Cell 0 attribute: bright set, ink or paper red (2) and black (0),
    // FLASH clear.
    let attr = screen.attributes[0];
    assert_eq!(attr & 0x80, 0, "FLASH must be 0");
    assert_eq!(attr & 0x40, 0x40, "BRIGHT must be set");
    let ink = attr & 7;
    let paper = (attr >> 3) & 7;
    let mut pair = [ink, paper];
    pair.sort_unstable();
    assert_eq!(pair, [0, 2]);

    // The encoded file decodes back to the same screen.
    let bytes = build198x::format::scr::encode(&screen).expect("encode");
    let decoded = build198x::format::scr::decode(&bytes).expect("decode");
    assert_eq!(decoded, screen);

    // Wrong-target guard.
    assert!(matches!(
        conv.to_koala(),
        Err(ConvertError::WrongTarget { .. })
    ));
}

#[test]
fn art_studio_bridge_agrees_with_indexed_pixels() {
    let pal = palette_of("commodore-c64");
    // Cells of two colours each, varying by cell.
    let img = image_of_indices(pal, 320, 200, |x, y| {
        let cell = (y / 8) * 40 + x / 8;
        let a = u8::try_from(cell % 15).expect("fits") + 1;
        if (x / 2 + y / 2) % 2 == 0 { a } else { 0 }
    });
    let mut opts = Options::new("commodore-c64", "hires-bitmap");
    opts.strength = 0;
    let conv = convert(&img, &opts).expect("conversion succeeds");
    assert!(conv.report.already_constrained);

    let art = conv.to_art_studio().expect("bridge succeeds");
    for y in 0..200usize {
        for x in 0..320usize {
            assert_eq!(
                art.color_index(x, y),
                Some(conv.pixels[y * 320 + x]),
                "decoded colour disagrees at ({x}, {y})"
            );
        }
    }
}

#[test]
fn koala_bridge_agrees_with_indexed_pixels_and_background() {
    let pal = palette_of("commodore-c64");
    // Multicolour pixels are double-wide (2:1 PAR), so the square-pixel
    // source for a full 160×200 multicolour screen is 320×200 with each
    // mode pixel drawn twice. Background blue (6) covers half; each cell
    // adds up to two extra colours.
    let img = image_of_indices(pal, 320, 200, |x, y| {
        let xm = x / 2; // mode-pixel column
        let cell = (y / 8) * 40 + xm / 4;
        match (xm + y) % 4 {
            0 => u8::try_from(cell % 15).expect("fits") + 1,
            1 => u8::try_from((cell + 7) % 15).expect("fits") + 1,
            _ => 6,
        }
    });
    let mut opts = Options::new("commodore-c64", "multicolour-bitmap");
    opts.strength = 0;
    let conv = convert(&img, &opts).expect("conversion succeeds");

    assert_eq!(conv.background, Some(6), "blue dominates the histogram");

    let k = conv.to_koala().expect("bridge succeeds");
    assert_eq!(k.background, 6);
    for y in 0..200usize {
        for x in 0..160usize {
            assert_eq!(
                k.color_index(x, y),
                Some(conv.pixels[y * 160 + x]),
                "decoded colour disagrees at ({x}, {y})"
            );
        }
    }
}

#[test]
fn exhaustive_background_is_deterministic_and_tie_breaks_low() {
    // Mostly dark grey with one extra colour per cell. The histogram
    // heuristic picks the dominant grey (11). Every background can render
    // this image exactly (each cell needs only two free colours), so the
    // exhaustive loop sees a 16-way zero-error tie and the contract's
    // tie-break applies: ascending enumeration + strict `<` keeps the
    // lowest background index, 0.
    let pal = palette_of("commodore-c64");
    let img = image_of_indices(pal, 320, 200, |x, y| {
        let xm = x / 2;
        if (xm + y) % 8 == 0 {
            u8::try_from(1 + (xm / 4 + y / 8) % 15).expect("fits")
        } else {
            11 // dark grey dominates
        }
    });
    let mut heuristic = Options::new("commodore-c64", "multicolour-bitmap");
    heuristic.strength = 0;
    let mut exhaustive = heuristic.clone();
    exhaustive.exhaustive_background = true;

    let h = convert(&img, &heuristic).expect("heuristic run");
    let e1 = convert(&img, &exhaustive).expect("exhaustive run 1");
    let e2 = convert(&img, &exhaustive).expect("exhaustive run 2");

    assert_eq!(h.background, Some(11), "histogram picks the dominant grey");
    assert!(h.report.mean_error < 1e-12);
    assert_eq!(e1.background, Some(0), "zero-error tie breaks to index 0");
    assert!(e1.report.mean_error < 1e-12);
    assert_eq!(e1.pixels, e2.pixels);
    assert_eq!(e1.report, e2.report);
}

#[test]
fn dither_default_resolves_per_target() {
    // The shared resolver: planar (free-palette) targets default to
    // serpentine Floyd–Steinberg, cell-constrained targets to 8×8 Bayer.
    use mediaspec::ConstraintRule;
    assert_eq!(
        default_dither(ConstraintRule::Planar { max_planes: 5 }),
        DitherMode::FloydSteinberg
    );
    for rule in [
        ConstraintRule::SpectrumAttr,
        ConstraintRule::C64Hires,
        ConstraintRule::C64Multicolour,
    ] {
        assert_eq!(default_dither(rule), DitherMode::Bayer8, "{rule:?}");
    }

    // Options::new inherits the resolver, so library consumers and the
    // emu-smoke goldens share the CLI's defaults.
    assert_eq!(
        Options::new("commodore-amiga-ocs", "lores-pal").dither,
        DitherMode::FloydSteinberg
    );
    assert_eq!(
        Options::new("commodore-amiga-ocs", "hires-ntsc").dither,
        DitherMode::FloydSteinberg
    );
    for (machine, mode) in [
        ("sinclair-zx-spectrum", "standard"),
        ("commodore-c64", "hires-bitmap"),
        ("commodore-c64", "multicolour-bitmap"),
    ] {
        assert_eq!(
            Options::new(machine, mode).dither,
            DitherMode::Bayer8,
            "{machine} {mode}"
        );
    }
    // Unknown targets fall back to the ordered default; convert() rejects
    // them before the dither choice matters.
    assert_eq!(
        Options::new("acorn-electron", "standard").dither,
        DitherMode::Bayer8
    );
}
