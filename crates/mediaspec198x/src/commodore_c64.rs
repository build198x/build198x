//! Commodore 64 (VIC-II) graphics capabilities.
//!
//! Authored from
//! `syntheses/commodore-c64/vic-ii-screen-memory-modes-sprites.md`
//! (whose primary sources are the MOS 6567 datasheet and the C64
//! Programmer's Reference Guide):
//!
//! - **Standard ("hires") bitmap mode** (§ 5): 320×200, 1 bit per pixel,
//!   8×8 cells with two colours per cell — both freely chosen from the
//!   16-colour palette via the cell's screen-RAM nybbles.
//! - **Multicolour bitmap mode** (§ 5): effectively 160×200 in double-wide
//!   pixels, 2 bits per pixel; per 4×8 cell, bit-pairs `01`/`10`/`11` select
//!   three per-cell colours (screen-RAM upper/lower nybble, colour-RAM
//!   nybble) and `00` selects the single global background (`$D021`).
//! - The palette has 16 fixed hardware colours (colour nybbles throughout
//!   §§ 4–5).
//!
//! **Border:** § 7 documents the display window's raster/pixel placement
//! (e.g. pixels 24–343, rasters 51–250 at 40×25) but not a paper-relative
//! visible-border rectangle, so border geometry is `None` here.

use crate::{
    CellGrid, ConstraintRule, MachineGraphics, NamedPalette, PaletteModel, Ratio, Rgb, ScreenMode,
    rgb,
};

/// `emu198x-v1`: transcription of Emu198x's VIC-II palette — the charter's
/// labelled emulator-table exception (`198x/decisions/shared-media-spec.md`
/// § 4). Source: `Emu198x/crates/mos-vic-ii/src/palette.rs` (`PALETTE`,
/// ARGB32 with the alpha byte dropped). Hardware index order 0–15.
const EMU198X_V1: &[Rgb] = &[
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

/// `pepto-v1`: Philip "Pepto" Timmermann's calculated PAL palette, as
/// shipped by VICE. Transcribed from
/// `assets/vice/vice/data/C64/pepto-pal.vpl`.
const PEPTO_V1: &[Rgb] = &[
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

/// `colodore-v1`: Pepto's later colodore.com PAL palette, as shipped by
/// VICE. Transcribed from `assets/vice/vice/data/C64/colodore.vpl`.
const COLODORE_V1: &[Rgb] = &[
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

/// The Commodore 64 graphics description.
pub const MACHINE: MachineGraphics = MachineGraphics {
    id: "commodore-c64",
    name: "Commodore 64",
    modes: &[
        ScreenMode {
            name: "hires-bitmap",
            // 320×200, 2 colours per 8×8 block:
            // syntheses/commodore-c64/vic-ii-screen-memory-modes-sprites.md
            // § 5 "Standard bitmap mode".
            paper_width: 320,
            paper_height: 200,
            // Single-width pixels — the machine baseline 1:1 (multicolour
            // pixels are defined as double these).
            pixel_aspect: Ratio::SQUARE,
            cell: Some(CellGrid {
                width: 8,
                height: 8,
                free_colours: 2,
            }),
            constraint: ConstraintRule::C64Hires,
            // TODO(emu198x-harness): § 7 documents display-window placement,
            // not a visible-border rectangle; refine from the Emu198x
            // framebuffer once the smoke harness lands.
            border: None,
        },
        ScreenMode {
            name: "multicolour-bitmap",
            // "Resolution is effectively 160×200 with 4 colours per 8×8
            // block", rendered as "4 double-wide pixels" per cell byte row:
            // syntheses/commodore-c64/vic-ii-screen-memory-modes-sprites.md
            // § 5 "Multicolour bitmap mode" (+ the multicolour-text bit-pair
            // table for the double-wide rendering).
            paper_width: 160,
            paper_height: 200,
            // Double-wide pixels: 2:1 against the hires baseline (same
            // synthesis section).
            pixel_aspect: Ratio::new(2, 1),
            // 4×8 cell in mode pixels (8×8 hires pixels); 3 free per-cell
            // colours — the global background is in the constraint rule.
            cell: Some(CellGrid {
                width: 4,
                height: 8,
                free_colours: 3,
            }),
            constraint: ConstraintRule::C64Multicolour,
            // TODO(emu198x-harness): see hires-bitmap border note.
            border: None,
        },
    ],
    palette: PaletteModel::Fixed(&[
        NamedPalette {
            name: "emu198x-v1",
            source: "Emu198x/crates/mos-vic-ii/src/palette.rs",
            colours: EMU198X_V1,
        },
        NamedPalette {
            name: "pepto-v1",
            source: "assets/vice/vice/data/C64/pepto-pal.vpl",
            colours: PEPTO_V1,
        },
        NamedPalette {
            name: "colodore-v1",
            source: "assets/vice/vice/data/C64/colodore.vpl",
            colours: COLODORE_V1,
        },
    ]),
    default_interpretation: Some("emu198x-v1"),
    notes: "Text modes (standard, multicolour, ECM) and sprites exist but are \
            out of wave-1 converter scope; only the two bitmap modes are \
            modelled. Source: \
            syntheses/commodore-c64/vic-ii-screen-memory-modes-sprites.md \
            \u{a7}\u{a7} 5\u{2013}6.",
};
