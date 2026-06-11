//! Checks of authored values against their cited sources.
//!
//! Each assertion restates a fact from the primary source the spec module
//! cites, so a drift in the spec data is caught against the documented
//! value, not against itself. Palette interpretations are asserted as
//! **full tables**, not spot entries: interpretation names are
//! content-versioned and frozen (a published name never changes its
//! values), so transcribing every entry here freezes each table
//! mechanically — any edit to a shipped table fails the test and forces a
//! new `…-v2` name instead.

use mediaspec::{ConstraintRule, PaletteModel, Ratio, Rgb, machine, rgb};

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
/// (normal primaries 0xC2, bright 0xFF, black shared across halves) —
/// full table, freezing the content-versioned name.
#[test]
fn spectrum_emu198x_v1_matches_transcription() {
    let m = machine("sinclair-zx-spectrum").expect("machine");
    let p = m.interpretation("emu198x-v1").expect("interpretation");
    let expected: &[Rgb] = &[
        rgb(0x00, 0x00, 0x00), // 0: black
        rgb(0x00, 0x00, 0xC2), // 1: blue
        rgb(0xC2, 0x00, 0x00), // 2: red
        rgb(0xC2, 0x00, 0xC2), // 3: magenta
        rgb(0x00, 0xC2, 0x00), // 4: green
        rgb(0x00, 0xC2, 0xC2), // 5: cyan
        rgb(0xC2, 0xC2, 0x00), // 6: yellow
        rgb(0xC2, 0xC2, 0xC2), // 7: white
        rgb(0x00, 0x00, 0x00), // 8: bright black (== black, shared)
        rgb(0x00, 0x00, 0xFF), // 9: bright blue
        rgb(0xFF, 0x00, 0x00), // 10: bright red
        rgb(0xFF, 0x00, 0xFF), // 11: bright magenta
        rgb(0x00, 0xFF, 0x00), // 12: bright green
        rgb(0x00, 0xFF, 0xFF), // 13: bright cyan
        rgb(0xFF, 0xFF, 0x00), // 14: bright yellow
        rgb(0xFF, 0xFF, 0xFF), // 15: bright white
    ];
    assert_eq!(p.colours, expected);
    assert_eq!(
        p.source,
        "Emu198x/crates/common-sinclair-zx-spectrum/src/palette.rs"
    );
}

/// Spectrum `fuse-v1` matches the `rgb_colours` table in
/// `emulators/zx-spectrum/fuse-emulator-fuse/ui/gtk/gtkdisplay.c`
/// (normal 192, bright 255 per active primary) — full table, freezing the
/// content-versioned name.
#[test]
fn spectrum_fuse_v1_matches_transcription() {
    let m = machine("sinclair-zx-spectrum").expect("machine");
    let p = m.interpretation("fuse-v1").expect("interpretation");
    let expected: &[Rgb] = &[
        rgb(0, 0, 0),       // 0: black
        rgb(0, 0, 192),     // 1: blue
        rgb(192, 0, 0),     // 2: red
        rgb(192, 0, 192),   // 3: magenta
        rgb(0, 192, 0),     // 4: green
        rgb(0, 192, 192),   // 5: cyan
        rgb(192, 192, 0),   // 6: yellow
        rgb(192, 192, 192), // 7: white
        rgb(0, 0, 0),       // 8: bright black (== black, shared)
        rgb(0, 0, 255),     // 9: bright blue
        rgb(255, 0, 0),     // 10: bright red
        rgb(255, 0, 255),   // 11: bright magenta
        rgb(0, 255, 0),     // 12: bright green
        rgb(0, 255, 255),   // 13: bright cyan
        rgb(255, 255, 0),   // 14: bright yellow
        rgb(255, 255, 255), // 15: bright white
    ];
    assert_eq!(p.colours, expected);
    assert_eq!(
        p.source,
        "emulators/zx-spectrum/fuse-emulator-fuse/ui/gtk/gtkdisplay.c"
    );
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
/// `Emu198x/crates/mos-vic-ii/src/palette.rs` (ARGB32 with the alpha byte
/// dropped) — full table, freezing the content-versioned name.
#[test]
fn c64_emu198x_v1_matches_transcription() {
    let m = machine("commodore-c64").expect("machine");
    let p = m.interpretation("emu198x-v1").expect("interpretation");
    let expected: &[Rgb] = &[
        rgb(0x00, 0x00, 0x00), // 0: black
        rgb(0xFF, 0xFF, 0xFF), // 1: white
        rgb(0x88, 0x39, 0x32), // 2: red
        rgb(0x67, 0xB6, 0xBD), // 3: cyan
        rgb(0x8B, 0x3F, 0x96), // 4: purple
        rgb(0x55, 0xA0, 0x49), // 5: green
        rgb(0x40, 0x31, 0x8D), // 6: blue
        rgb(0xBF, 0xCE, 0x72), // 7: yellow
        rgb(0x8B, 0x54, 0x29), // 8: orange
        rgb(0x57, 0x42, 0x00), // 9: brown
        rgb(0xB8, 0x69, 0x62), // 10: light red
        rgb(0x50, 0x50, 0x50), // 11: dark grey
        rgb(0x78, 0x78, 0x78), // 12: medium grey
        rgb(0x94, 0xE0, 0x89), // 13: light green
        rgb(0x78, 0x68, 0xC0), // 14: light blue
        rgb(0x9F, 0x9F, 0x9F), // 15: light grey
    ];
    assert_eq!(p.colours, expected);
    assert_eq!(p.source, "Emu198x/crates/mos-vic-ii/src/palette.rs");
}

/// C64 community interpretations match the VICE palette files they cite
/// (`assets/vice/vice/data/C64/pepto-pal.vpl`, `…/colodore.vpl`) — full
/// tables, freezing the content-versioned names.
#[test]
fn c64_community_interpretations_match_vpl_files() {
    let m = machine("commodore-c64").expect("machine");

    let pepto = m.interpretation("pepto-v1").expect("pepto-v1");
    let pepto_expected: &[Rgb] = &[
        rgb(0x00, 0x00, 0x00), // 0: black
        rgb(0xFF, 0xFF, 0xFF), // 1: white
        rgb(0x68, 0x37, 0x2B), // 2: red
        rgb(0x70, 0xA4, 0xB2), // 3: cyan
        rgb(0x6F, 0x3D, 0x86), // 4: purple
        rgb(0x58, 0x8D, 0x43), // 5: green
        rgb(0x35, 0x28, 0x79), // 6: blue
        rgb(0xB8, 0xC7, 0x6F), // 7: yellow
        rgb(0x6F, 0x4F, 0x25), // 8: orange
        rgb(0x43, 0x39, 0x00), // 9: brown
        rgb(0x9A, 0x67, 0x59), // 10: light red
        rgb(0x44, 0x44, 0x44), // 11: dark grey
        rgb(0x6C, 0x6C, 0x6C), // 12: medium grey
        rgb(0x9A, 0xD2, 0x84), // 13: light green
        rgb(0x6C, 0x5E, 0xB5), // 14: light blue
        rgb(0x95, 0x95, 0x95), // 15: light grey
    ];
    assert_eq!(pepto.colours, pepto_expected);
    assert_eq!(pepto.source, "assets/vice/vice/data/C64/pepto-pal.vpl");

    let colodore = m.interpretation("colodore-v1").expect("colodore-v1");
    let colodore_expected: &[Rgb] = &[
        rgb(0x00, 0x00, 0x00), // 0: black
        rgb(0xFF, 0xFF, 0xFF), // 1: white
        rgb(0x96, 0x28, 0x2E), // 2: red
        rgb(0x5B, 0xD6, 0xCE), // 3: cyan
        rgb(0x9F, 0x2D, 0xAD), // 4: purple
        rgb(0x41, 0xB9, 0x36), // 5: green
        rgb(0x27, 0x24, 0xC4), // 6: blue
        rgb(0xEF, 0xF3, 0x47), // 7: yellow
        rgb(0x9F, 0x48, 0x15), // 8: orange
        rgb(0x5E, 0x35, 0x00), // 9: brown
        rgb(0xDA, 0x5F, 0x66), // 10: light red
        rgb(0x47, 0x47, 0x47), // 11: dark grey
        rgb(0x78, 0x78, 0x78), // 12: medium grey
        rgb(0x91, 0xFF, 0x84), // 13: light green
        rgb(0x68, 0x64, 0xFF), // 14: light blue
        rgb(0xAE, 0xAE, 0xAE), // 15: light grey
    ];
    assert_eq!(colodore.colours, colodore_expected);
    assert_eq!(colodore.source, "assets/vice/vice/data/C64/colodore.vpl");
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
