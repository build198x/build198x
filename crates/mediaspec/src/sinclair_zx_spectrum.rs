//! Sinclair ZX Spectrum graphics capabilities.
//!
//! Authored from `syntheses/zx-spectrum/screen-and-attribute-memory.md`
//! (whose silicon canon is Chris Smith, *The ZX Spectrum ULA*, Chs 12 + 15):
//!
//! - Paper bitmap is 256×192 pixels, 1 bit per pixel (§ 1).
//! - The attribute table covers 32×24 cells of 8×8 pixels, one byte per
//!   cell (§ 6).
//! - Attribute byte `FBPPPIII`: FLASH, BRIGHT, 3-bit PAPER, 3-bit INK (§ 7).
//! - BRIGHT applies to INK and PAPER together — one brightness per cell,
//!   never mixed (§ 7, "Why BRIGHT applies to both INK and PAPER").
//! - Black is shared across the brightness halves: 8 colours × 2 levels
//!   gives 15 distinct colours rendered from 16 indices (§ 7).
//!
//! **Out of converter scope, noted not modelled:** the FLASH bit (attribute
//! bit 7, swaps INK/PAPER every 16 frames — § 7 "FLASH timing"). A converted
//! image always ships FLASH=0.
//!
//! **Border:** the synthesis documents border *timing* (T-state budgets for
//! border-time work, § 8) but no paper-relative border pixel geometry, so
//! [`MachineGraphics::border`]-level data is `None` here.

use crate::{
    CellGrid, ConstraintRule, MachineGraphics, NamedPalette, PaletteModel, Ratio, Rgb, ScreenMode,
    rgb,
};

/// `emu198x-v1`: transcription of Emu198x's Spectrum palette — the charter's
/// labelled emulator-table exception (`198x/decisions/shared-media-spec.md`
/// § 4). Source: `Emu198x/crates/common-sinclair-zx-spectrum/src/palette.rs`
/// (`SPECTRUM_PALETTE`), which derives normal = 0xC2 / bright = 0xFF per
/// active primary from Smith Table 16-1 emitter currents. Indices 0–7
/// normal, 8–15 bright; indices 0 and 8 are both black.
const EMU198X_V1: &[Rgb] = &[
    rgb(0x00, 0x00, 0x00), // 0: black
    rgb(0x00, 0x00, 0xC2), // 1: blue
    rgb(0xC2, 0x00, 0x00), // 2: red
    rgb(0xC2, 0x00, 0xC2), // 3: magenta
    rgb(0x00, 0xC2, 0x00), // 4: green
    rgb(0x00, 0xC2, 0xC2), // 5: cyan
    rgb(0xC2, 0xC2, 0x00), // 6: yellow
    rgb(0xC2, 0xC2, 0xC2), // 7: white
    rgb(0x00, 0x00, 0x00), // 8: bright black (== black)
    rgb(0x00, 0x00, 0xFF), // 9: bright blue
    rgb(0xFF, 0x00, 0x00), // 10: bright red
    rgb(0xFF, 0x00, 0xFF), // 11: bright magenta
    rgb(0x00, 0xFF, 0x00), // 12: bright green
    rgb(0x00, 0xFF, 0xFF), // 13: bright cyan
    rgb(0xFF, 0xFF, 0x00), // 14: bright yellow
    rgb(0xFF, 0xFF, 0xFF), // 15: bright white
];

/// `fuse-v1`: the Fuse emulator's level table — normal = 192, bright = 255
/// per active primary. Transcribed from the `rgb_colours[16][3]` table in
/// `emulators/zx-spectrum/fuse-emulator-fuse/ui/gtk/gtkdisplay.c`.
const FUSE_V1: &[Rgb] = &[
    rgb(0, 0, 0),       // 0: black
    rgb(0, 0, 192),     // 1: blue
    rgb(192, 0, 0),     // 2: red
    rgb(192, 0, 192),   // 3: magenta
    rgb(0, 192, 0),     // 4: green
    rgb(0, 192, 192),   // 5: cyan
    rgb(192, 192, 0),   // 6: yellow
    rgb(192, 192, 192), // 7: white
    rgb(0, 0, 0),       // 8: bright black (== black)
    rgb(0, 0, 255),     // 9: bright blue
    rgb(255, 0, 0),     // 10: bright red
    rgb(255, 0, 255),   // 11: bright magenta
    rgb(0, 255, 0),     // 12: bright green
    rgb(0, 255, 255),   // 13: bright cyan
    rgb(255, 255, 0),   // 14: bright yellow
    rgb(255, 255, 255), // 15: bright white
];

/// The ZX Spectrum graphics description.
pub const MACHINE: MachineGraphics = MachineGraphics {
    id: "sinclair-zx-spectrum",
    name: "Sinclair ZX Spectrum",
    modes: &[ScreenMode {
        name: "standard",
        // 256×192 paper, 32×24 cells of 8×8:
        // syntheses/zx-spectrum/screen-and-attribute-memory.md §§ 1, 6.
        paper_width: 256,
        paper_height: 192,
        // The Spectrum has a single pixel shape (no double-wide mode), so
        // its pixel is the machine baseline 1:1 — see the mode-relative
        // pixel-aspect convention on `ScreenMode::pixel_aspect`.
        pixel_aspect: Ratio::SQUARE,
        // 8×8 cell, INK + PAPER = 2 free colours per cell (brightness
        // coupling lives in the constraint rule).
        cell: Some(CellGrid {
            width: 8,
            height: 8,
            free_colours: 2,
        }),
        constraint: ConstraintRule::SpectrumAttr,
        // TODO(emu198x-harness): the synthesis documents border timing, not
        // paper-relative border pixel geometry; refine from the Emu198x
        // framebuffer once the smoke harness lands.
        border: None,
    }],
    palette: PaletteModel::Fixed(&[
        NamedPalette {
            name: "emu198x-v1",
            source: "Emu198x/crates/common-sinclair-zx-spectrum/src/palette.rs",
            colours: EMU198X_V1,
        },
        NamedPalette {
            name: "fuse-v1",
            source: "emulators/zx-spectrum/fuse-emulator-fuse/ui/gtk/gtkdisplay.c",
            colours: FUSE_V1,
        },
    ]),
    default_interpretation: Some("emu198x-v1"),
    notes: "FLASH (attribute bit 7) exists but is out of converter scope; \
            converted images ship FLASH=0. Source: \
            syntheses/zx-spectrum/screen-and-attribute-memory.md \u{a7} 7.",
};
