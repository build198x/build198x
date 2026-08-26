//! Commodore Amiga (OCS chipset) graphics capabilities.
//!
//! Authored from `syntheses/commodore-amiga/amiga-graphics-display.md`
//! (whose primary source is the Amiga Hardware Reference Manual, 3rd ed.):
//!
//! - Display data is **planar bitplanes**: up to 5 planes in lores single
//!   playfield (5-bit colour index → COLOR00..COLOR31, § 2.3), up to 4
//!   planes in hires (§ 2.2, HRM p.9027 — 6-plane hires is impossible on
//!   OCS because bitplane DMA would exhaust the memory slots).
//! - **Lores** is 320 pixels per line; **hires** is 640 (§ 2.2). Visible
//!   lines non-interlaced: 200 NTSC / 256 PAL (§ 11.1, HRM Table 3-13).
//! - Colour registers COLOR00..COLOR31 hold 4 bits per gun (R4 G4 B4),
//!   a 4096-colour gamut (§ 6, HRM p.4491) — so the palette model is a
//!   parametric [`PaletteModel::Gamut`], not a fixed table: per-image
//!   palettes are generated, then rounded to 4 bits per gun.
//!
//! **Out of wave-1 scope, noted not modelled:** EHB / Extra-Halfbrite
//! (BPU=6, DBLPF=0, HOMOD=0: bitplane 6 halves the intensity of the colour
//! selected by planes 1–5, giving 32 + 32 colours — § 2.3, HRM p.4650);
//! HAM (§ 2.3); dual playfield (§ 2.3); interlace (§ 1.7) and ECS
//! super-hires (§ 2.2).
//!
//! **Border:** § 1.3 and § 11.1 document display-window placement
//! (DIWSTRT/DIWSTOP defaults) and maximum visible extents, but no
//! paper-relative border rectangle, so border geometry is `None` here.

use crate::{ConstraintRule, MachineGraphics, PaletteModel, Ratio, ScreenMode};

/// Build one Amiga planar mode (the four modes differ only in name,
/// geometry, pixel shape, and plane budget).
const fn planar_mode(
    name: &'static str,
    paper_width: u16,
    paper_height: u16,
    pixel_aspect: Ratio,
    max_planes: u8,
) -> ScreenMode {
    ScreenMode {
        name,
        paper_width,
        paper_height,
        pixel_aspect,
        // Planar modes have no attribute cell grid: the palette constraint
        // is per screen, not per cell.
        cell: None,
        constraint: ConstraintRule::Planar { max_planes },
        // TODO(emu198x-harness): the synthesis documents DIWSTRT/DIWSTOP
        // window placement (§ 1.3) and max visible extents (§ 11.1), not a
        // paper-relative border rectangle; refine from the Emu198x
        // framebuffer once the smoke harness lands.
        border: None,
    }
}

/// Hires pixels are half the width of lores pixels: one colour clock spans
/// 2 lores or 4 hires pixels over the same line timing
/// (`syntheses/commodore-amiga/amiga-graphics-display.md` § 2.4), so against
/// the lores baseline of 1:1 a hires pixel is 1:2.
const HIRES_ASPECT: Ratio = Ratio::new(1, 2);

/// The Amiga OCS graphics description.
///
/// Geometry sources: 320 lores / 640 hires pixels per line — synthesis
/// § 2.2; 256 PAL / 200 NTSC non-interlaced visible lines — § 11.1 (HRM
/// Table 3-13); plane budgets (5 lores, 4 hires) — §§ 2.2–2.3.
pub const MACHINE: MachineGraphics = MachineGraphics {
    id: "commodore-amiga-ocs",
    name: "Commodore Amiga (OCS)",
    modes: &[
        planar_mode("lores-pal", 320, 256, Ratio::SQUARE, 5),
        planar_mode("lores-ntsc", 320, 200, Ratio::SQUARE, 5),
        planar_mode("hires-pal", 640, 256, HIRES_ASPECT, 4),
        planar_mode("hires-ntsc", 640, 200, HIRES_ASPECT, 4),
    ],
    // 4 bits per gun (R4 G4 B4 in COLOR00..COLOR31) = 4096 colours:
    // syntheses/commodore-amiga/amiga-graphics-display.md § 6 (HRM p.4491).
    palette: PaletteModel::Gamut { bits_per_gun: 4 },
    // Gamut machines have no named interpretations to default to.
    default_interpretation: None,
    notes: "EHB, HAM, dual playfield, interlace, and ECS super-hires exist \
            but are out of wave-1 converter scope (see module docs). Source: \
            syntheses/commodore-amiga/amiga-graphics-display.md \
            \u{a7}\u{a7} 1.7, 2.2\u{2013}2.3.",
};
