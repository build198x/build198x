//! Declarative graphics-capability specifications for the 198x family machines.
//!
//! This crate is the **authored media capability spec** — the executable
//! single source of truth for what each machine's display hardware can show:
//! screen modes (paper geometry, pixel aspect, cell grids, bitplane counts),
//! the per-cell constraint rules a converter must satisfy, and the palette
//! model (fixed named interpretations, or a parametric gamut). Build198x's
//! converter consumes it to constrain images; Emu198x validates its renderers
//! against it. The spec is *authored* from the family's primary reference
//! library, not extracted from any emulator — with one labelled exception:
//! `emu198x-*` palette interpretations transcribe Emu198x's actual tables by
//! design, citing the emulator source file as provenance. See the binding
//! decision at `198x/decisions/shared-media-spec.md`.
//!
//! Source citations in this crate are file paths relative to the `198x/`
//! umbrella root (e.g. `syntheses/zx-spectrum/screen-and-attribute-memory.md`).
//!
//! Palette interpretation names are **content-versioned and frozen**: a
//! published name (`emu198x-v1`, `pepto-v1`) never changes its values. A
//! corrected table gets a new name (`emu198x-v2`), never an edit — goldens
//! depend on the freeze.
//!
//! Everything is `&'static` data so a whole machine description is a
//! compile-time constant: zero dependencies, no allocation, diffable in
//! review (the same stance as the `isa` crate in Asm198x).

/// The complete graphics description of one machine.
pub struct MachineGraphics {
    /// Stable machine id, kebab-case `manufacturer-system` (Emu198x's naming
    /// discipline), e.g. `"commodore-c64"`.
    pub id: &'static str,
    /// Human display name, e.g. `"Commodore 64"`.
    pub name: &'static str,
    /// Every screen mode this spec models for the machine. Wave-1 scope is
    /// the converter-relevant bitmap modes, not the full hardware mode set.
    pub modes: &'static [ScreenMode],
    /// How colour works on this machine: fixed hardware palette with named
    /// RGB interpretations, or a parametric gamut.
    pub palette: PaletteModel,
    /// The pinned default interpretation name for [`PaletteModel::Fixed`]
    /// machines — `emu198x-v1` where present, so converter output and the
    /// Emu198x smoke harness agree exactly. `None` for gamut machines.
    pub default_interpretation: Option<&'static str>,
    /// Free-form scope notes: hardware features that exist but are
    /// deliberately not modelled (FLASH, EHB, HAM, …), with their sources.
    pub notes: &'static str,
}

impl MachineGraphics {
    /// Find a screen mode by name.
    #[must_use]
    pub fn mode(&self, name: &str) -> Option<&ScreenMode> {
        self.modes.iter().find(|m| m.name == name)
    }

    /// Find a named palette interpretation. Always `None` for
    /// [`PaletteModel::Gamut`] machines (their palettes are generated
    /// per image, not named).
    #[must_use]
    pub fn interpretation(&self, name: &str) -> Option<&NamedPalette> {
        match self.palette {
            PaletteModel::Fixed(palettes) => palettes.iter().find(|p| p.name == name),
            PaletteModel::Gamut { .. } => None,
        }
    }

    /// The pinned default palette interpretation, resolved.
    #[must_use]
    pub fn default_palette(&self) -> Option<&NamedPalette> {
        self.interpretation(self.default_interpretation?)
    }
}

/// One screen mode: the geometry and constraint shape a converter targets.
pub struct ScreenMode {
    /// Mode name, unique within the machine, e.g. `"multicolour-bitmap"`.
    pub name: &'static str,
    /// Paper (active bitmap area) width in mode pixels.
    pub paper_width: u16,
    /// Paper height in mode pixels.
    pub paper_height: u16,
    /// Pixel aspect ratio as a small integer ratio, width : height of one
    /// mode pixel **relative to the machine's single-width pixel taken as
    /// 1:1**. This is a mode-relative shape, not a calibrated TV aspect:
    /// C64 multicolour pixels are double-wide (2:1), Amiga hires pixels are
    /// half-width (1:2). Each mode documents its value's source.
    pub pixel_aspect: Ratio,
    /// The attribute cell grid, where the mode has one. `None` for planar
    /// modes (colour constraints are per-screen, not per-cell).
    pub cell: Option<CellGrid>,
    /// The constraint rule the converter must satisfy for this mode.
    pub constraint: ConstraintRule,
    /// Border geometry around the paper, where the primary sources document
    /// it in converter-usable pixel terms. `None` until the Emu198x harness
    /// refines it (the syntheses document border *timing* and display-window
    /// *placement*, not a paper-relative border rectangle).
    pub border: Option<BorderGeometry>,
}

impl ScreenMode {
    /// Bitplane count for planar modes (the maximum the mode supports);
    /// `None` for cell/attribute modes.
    #[must_use]
    pub const fn planes(&self) -> Option<u8> {
        match self.constraint {
            ConstraintRule::Planar { max_planes } => Some(max_planes),
            _ => None,
        }
    }
}

/// A small integer ratio (used for pixel aspect). Components are nonzero.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Ratio {
    pub horizontal: u8,
    pub vertical: u8,
}

impl Ratio {
    /// The machine-relative square pixel.
    pub const SQUARE: Ratio = Ratio::new(1, 1);

    /// Build a ratio.
    #[must_use]
    pub const fn new(horizontal: u8, vertical: u8) -> Self {
        Self {
            horizontal,
            vertical,
        }
    }
}

/// The attribute cell grid of a cell-constrained mode.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CellGrid {
    /// Cell width in mode pixels.
    pub width: u8,
    /// Cell height in mode pixels.
    pub height: u8,
    /// Colours freely choosable *per cell* (globals such as the C64
    /// multicolour shared background are not counted here — they live in
    /// the [`ConstraintRule`]).
    pub free_colours: u8,
}

/// The colour-constraint rule a converter must interpret for a mode. Each
/// variant names a concrete hardware rule; the geometric numbers live in the
/// mode's [`CellGrid`] so they are stated once.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConstraintRule {
    /// ZX Spectrum attribute rule: per 8×8 cell, one INK and one PAPER
    /// colour (each 0–7), both drawn from the *same* brightness half as
    /// selected by the cell's BRIGHT bit — BRIGHT applies to ink and paper
    /// together, never separately. Black is shared across the two halves
    /// (the bright bit has no effect when no primaries are active).
    /// Source: `syntheses/zx-spectrum/screen-and-attribute-memory.md` § 7
    /// (attribute byte format; "Why BRIGHT applies to both INK and PAPER").
    SpectrumAttr,
    /// C64 standard ("hires") bitmap rule: per 8×8 cell, two freely chosen
    /// colours from the 16-colour palette (the cell's screen-RAM byte
    /// carries both nybbles). Source:
    /// `syntheses/commodore-c64/vic-ii-screen-memory-modes-sprites.md` § 5
    /// ("Standard bitmap mode").
    C64Hires,
    /// C64 multicolour bitmap rule: per 4×8 cell of double-wide pixels,
    /// three freely chosen per-cell colours (screen-RAM upper and lower
    /// nybbles plus the colour-RAM nybble) plus **one global background
    /// colour** (`$D021`) shared by every cell on screen. Source:
    /// `syntheses/commodore-c64/vic-ii-screen-memory-modes-sprites.md` § 5
    /// ("Multicolour bitmap mode").
    C64Multicolour,
    /// Planar bitplane rule: up to `max_planes` bitplanes give a per-screen
    /// indexed palette of `2^planes` entries; the palette itself is
    /// generated per image and rounded to the machine's gamut depth. No
    /// per-cell constraint.
    Planar {
        /// Maximum bitplane count the mode supports.
        max_planes: u8,
    },
}

/// Border geometry around the paper, in mode pixels.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BorderGeometry {
    pub left: u16,
    pub right: u16,
    pub top: u16,
    pub bottom: u16,
}

/// How colour works on a machine.
pub enum PaletteModel {
    /// A fixed hardware palette whose RGB rendering is a matter of
    /// *interpretation* — each [`NamedPalette`] is one content-versioned,
    /// frozen interpretation of the same hardware colours.
    Fixed(&'static [NamedPalette]),
    /// A parametric gamut: the hardware has programmable colour registers
    /// with `bits_per_gun` bits per RGB channel; per-image palettes are
    /// generated, then rounded to this depth.
    Gamut {
        /// Bits per RGB gun, e.g. 4 on Amiga OCS (4096-colour gamut).
        bits_per_gun: u8,
    },
}

impl PaletteModel {
    /// Total colours the gamut can express (`None` for fixed palettes —
    /// read the interpretation's `colours` length instead).
    #[must_use]
    pub const fn gamut_size(&self) -> Option<u32> {
        match self {
            PaletteModel::Fixed(_) => None,
            PaletteModel::Gamut { bits_per_gun } => Some(1 << (3 * *bits_per_gun as u32)),
        }
    }
}

/// One content-versioned RGB interpretation of a fixed hardware palette.
///
/// **Frozen once published**: the values behind a name never change; a
/// corrected table is a new name (`…-v2`).
pub struct NamedPalette {
    /// Content-versioned name, e.g. `"emu198x-v1"`, `"pepto-v1"`.
    pub name: &'static str,
    /// Provenance: the file (umbrella-relative path) these values were
    /// transcribed from.
    pub source: &'static str,
    /// RGB triple per hardware colour index, in hardware index order.
    pub colours: &'static [Rgb],
}

/// An 8-bit-per-channel RGB triple.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// Build an [`Rgb`] (const-context ergonomics for palette tables).
#[must_use]
pub const fn rgb(r: u8, g: u8, b: u8) -> Rgb {
    Rgb { r, g, b }
}

pub mod commodore_amiga_ocs;
pub mod commodore_c64;
pub mod sinclair_zx_spectrum;

/// Every machine this spec describes.
pub const MACHINES: &[&MachineGraphics] = &[
    &sinclair_zx_spectrum::MACHINE,
    &commodore_c64::MACHINE,
    &commodore_amiga_ocs::MACHINE,
];

/// All machines.
#[must_use]
pub const fn machines() -> &'static [&'static MachineGraphics] {
    MACHINES
}

/// Find a machine by id (e.g. `"commodore-c64"`).
#[must_use]
pub fn machine(id: &str) -> Option<&'static MachineGraphics> {
    MACHINES.iter().copied().find(|m| m.id == id)
}

/// The pinned default palette-interpretation name for a machine, where one
/// exists (`emu198x-v1` on every fixed-palette machine shipped so far;
/// `None` for gamut machines and unknown ids).
#[must_use]
pub fn default_interpretation(machine_id: &str) -> Option<&'static str> {
    machine(machine_id)?.default_interpretation
}
