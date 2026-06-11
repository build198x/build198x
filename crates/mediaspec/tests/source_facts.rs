//! Spot checks of authored values against their cited sources.
//!
//! Each assertion restates a fact from the primary source the spec module
//! cites, so a drift in the spec data is caught against the documented
//! value, not against itself.

use mediaspec::{ConstraintRule, PaletteModel, Ratio, machine, rgb};

/// Spectrum paper is 256×192 with 32×24 cells of 8×8
/// (`syntheses/zx-spectrum/screen-and-attribute-memory.md` §§ 1, 6).
#[test]
fn spectrum_standard_mode_geometry() {
    let m = machine("sinclair-zx-spectrum").expect("machine");
    let mode = m.mode("standard").expect("mode");
    assert_eq!((mode.paper_width, mode.paper_height), (256, 192));
    let cell = mode.cell.expect("cell grid");
    assert_eq!((cell.width, cell.height), (8, 8));
    assert_eq!(mode.paper_width / u16::from(cell.width), 32);
    assert_eq!(mode.paper_height / u16::from(cell.height), 24);
    assert_eq!(cell.free_colours, 2); // INK + PAPER
    assert_eq!(mode.constraint, ConstraintRule::SpectrumAttr);
}

/// Spectrum `emu198x-v1` matches the transcribed table in
/// `Emu198x/crates/common-sinclair-zx-spectrum/src/palette.rs`
/// (normal primaries 0xC2, bright 0xFF, black shared across halves).
#[test]
fn spectrum_emu198x_v1_matches_transcription() {
    let m = machine("sinclair-zx-spectrum").expect("machine");
    let p = m.interpretation("emu198x-v1").expect("interpretation");
    assert_eq!(p.colours.len(), 16);
    assert_eq!(p.colours[1], rgb(0x00, 0x00, 0xC2)); // normal blue
    assert_eq!(p.colours[7], rgb(0xC2, 0xC2, 0xC2)); // normal white
    assert_eq!(p.colours[9], rgb(0x00, 0x00, 0xFF)); // bright blue
    assert_eq!(p.colours[15], rgb(0xFF, 0xFF, 0xFF)); // bright white
    // Black is shared across the brightness halves.
    assert_eq!(p.colours[0], p.colours[8]);
    assert_eq!(p.colours[0], rgb(0x00, 0x00, 0x00));
}

/// C64 multicolour bitmap is 160×200 in double-wide (2:1) pixels with 4×8
/// cells, three free per-cell colours, and a 16-colour palette
/// (`syntheses/commodore-c64/vic-ii-screen-memory-modes-sprites.md` § 5).
#[test]
fn c64_multicolour_mode_geometry() {
    let m = machine("commodore-c64").expect("machine");
    let mode = m.mode("multicolour-bitmap").expect("mode");
    assert_eq!((mode.paper_width, mode.paper_height), (160, 200));
    assert_eq!(mode.pixel_aspect, Ratio::new(2, 1));
    let cell = mode.cell.expect("cell grid");
    assert_eq!((cell.width, cell.height), (4, 8));
    assert_eq!(cell.free_colours, 3);
    assert_eq!(mode.constraint, ConstraintRule::C64Multicolour);
    // 16 hardware colours in every interpretation.
    let PaletteModel::Fixed(palettes) = m.palette else {
        panic!("C64 should have a fixed palette");
    };
    assert!(palettes.iter().all(|p| p.colours.len() == 16));
}

/// C64 hires bitmap is 320×200, square against the machine baseline, 8×8
/// cells with two free colours (same synthesis, § 5 "Standard bitmap mode").
#[test]
fn c64_hires_mode_geometry() {
    let m = machine("commodore-c64").expect("machine");
    let mode = m.mode("hires-bitmap").expect("mode");
    assert_eq!((mode.paper_width, mode.paper_height), (320, 200));
    assert_eq!(mode.pixel_aspect, Ratio::SQUARE);
    let cell = mode.cell.expect("cell grid");
    assert_eq!((cell.width, cell.height, cell.free_colours), (8, 8, 2));
    assert_eq!(mode.constraint, ConstraintRule::C64Hires);
}

/// C64 `emu198x-v1` matches the transcribed table in
/// `Emu198x/crates/mos-vic-ii/src/palette.rs` — first entries verbatim.
#[test]
fn c64_emu198x_v1_matches_transcription() {
    let m = machine("commodore-c64").expect("machine");
    let p = m.interpretation("emu198x-v1").expect("interpretation");
    assert_eq!(p.colours.len(), 16);
    assert_eq!(p.colours[0], rgb(0x00, 0x00, 0x00)); // 0xFF00_0000 black
    assert_eq!(p.colours[1], rgb(0xFF, 0xFF, 0xFF)); // 0xFFFF_FFFF white
    assert_eq!(p.colours[2], rgb(0x88, 0x39, 0x32)); // 0xFF88_3932 red
    assert_eq!(p.colours[3], rgb(0x67, 0xB6, 0xBD)); // 0xFF67_B6BD cyan
    assert_eq!(p.colours[6], rgb(0x40, 0x31, 0x8D)); // 0xFF40_318D blue
    assert_eq!(p.source, "Emu198x/crates/mos-vic-ii/src/palette.rs");
}

/// C64 community interpretations match the VICE palette files they cite
/// (`assets/vice/vice/data/C64/pepto-pal.vpl`, `…/colodore.vpl`) — spot
/// entries verbatim.
#[test]
fn c64_community_interpretations_match_vpl_files() {
    let m = machine("commodore-c64").expect("machine");
    let pepto = m.interpretation("pepto-v1").expect("pepto-v1");
    assert_eq!(pepto.colours[2], rgb(0x68, 0x37, 0x2B)); // "Red: 68 37 2b"
    assert_eq!(pepto.colours[14], rgb(0x6C, 0x5E, 0xB5)); // "Light Blue: 6c 5e b5"
    let colodore = m.interpretation("colodore-v1").expect("colodore-v1");
    assert_eq!(colodore.colours[2], rgb(0x96, 0x28, 0x2E)); // "Red: 96 28 2e"
    assert_eq!(colodore.colours[14], rgb(0x68, 0x64, 0xFF)); // "Light Blue: 68 64 ff"
}

/// Amiga OCS lores PAL is 320×256 with at most 5 bitplanes; hires halves
/// the pixel width and caps at 4 planes; the colour registers are 4 bits
/// per gun = a 4096-colour gamut
/// (`syntheses/commodore-amiga/amiga-graphics-display.md` §§ 2.2–2.4, 6,
/// 11.1).
#[test]
fn amiga_ocs_modes_and_gamut() {
    let m = machine("commodore-amiga-ocs").expect("machine");

    let lores_pal = m.mode("lores-pal").expect("mode");
    assert_eq!((lores_pal.paper_width, lores_pal.paper_height), (320, 256));
    assert_eq!(lores_pal.planes(), Some(5));
    assert_eq!(lores_pal.pixel_aspect, Ratio::SQUARE);
    assert!(lores_pal.cell.is_none());

    let lores_ntsc = m.mode("lores-ntsc").expect("mode");
    assert_eq!(
        (lores_ntsc.paper_width, lores_ntsc.paper_height),
        (320, 200)
    );

    let hires_pal = m.mode("hires-pal").expect("mode");
    assert_eq!((hires_pal.paper_width, hires_pal.paper_height), (640, 256));
    assert_eq!(hires_pal.planes(), Some(4));
    assert_eq!(hires_pal.pixel_aspect, Ratio::new(1, 2));

    assert_eq!(m.palette.gamut_size(), Some(4096));
    assert!(m.interpretation("emu198x-v1").is_none());
}

/// The pinned defaults: `emu198x-v1` on both fixed-palette machines, none
/// on the gamut machine.
#[test]
fn default_interpretations_pinned_to_emu198x_v1() {
    let pinned = |id: &str| machine(id).and_then(|m| m.default_interpretation);
    assert_eq!(pinned("sinclair-zx-spectrum"), Some("emu198x-v1"));
    assert_eq!(pinned("commodore-c64"), Some("emu198x-v1"));
    assert_eq!(pinned("commodore-amiga-ocs"), None);
    assert_eq!(pinned("no-such-machine"), None);
}
