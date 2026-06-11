//! The public conversion entry point: [`Options`] + a decoded image in,
//! [`Conversion`] out — indexed pixels, the resolved palette, per-cell
//! choices, a quality report, and bridges to the [`crate::format`] codec
//! input structs.

use mediaspec::{ConstraintRule, MachineGraphics, PaletteModel, Rgb, ScreenMode};

use super::colour::{Metric, srgb8_to_linear};
use super::constrain::{CellSearcher, HiresChoice, MultiChoice, PaletteData, SpectrumChoice};
use super::dither::{self, DitherMode};
use super::normalise;
use super::quantise;
use super::resize;
use super::{ConvertError, LinearImage, Rgb8Image};
use crate::format::{art_studio, ilbm, koala, scr};

/// Per-cell mean-error threshold for the report's over-threshold count.
/// Squared metric distance per pixel — tuned for the OKLab scale (where
/// 0.01 ≈ a clearly visible residual); diagnostic only, not a gate.
pub const CELL_ERROR_THRESHOLD: f32 = 0.01;

/// Conversion options. Build with [`Options::new`] and override fields as
/// needed.
#[derive(Debug, Clone)]
pub struct Options {
    /// Target machine id, e.g. `"commodore-c64"`.
    pub machine: String,
    /// Target screen-mode name, e.g. `"multicolour-bitmap"`.
    pub mode: String,
    /// Palette interpretation name; `None` uses the machine's pinned
    /// default. Must be `None` for gamut machines.
    pub interpretation: Option<String>,
    /// Colour-distance metric (default OKLab).
    pub metric: Metric,
    /// Dither algorithm (default 8×8 Bayer). Error-diffusion modes are
    /// free-palette (planar) targets only.
    pub dither: DitherMode,
    /// Ordered-dither strength, 0..=64 (default 32). 0 means
    /// nearest-colour.
    pub strength: u8,
    /// Matte colour (sRGB) composited under any alpha and used for
    /// letterbox padding. Default black.
    pub matte: [u8; 3],
    /// C64 multicolour only: try every palette entry as the global
    /// background instead of the histogram heuristic (16× the search
    /// cost).
    pub exhaustive_background: bool,
    /// Disable dithering entirely (equivalent to strength 0 for ordered
    /// modes; overrides error-diffusion selection).
    pub no_dither: bool,
}

impl Options {
    /// Defaults: pinned default palette, OKLab metric, 8×8 Bayer at
    /// strength 32, black matte.
    #[must_use]
    pub fn new(machine: &str, mode: &str) -> Self {
        Self {
            machine: machine.to_owned(),
            mode: mode.to_owned(),
            interpretation: None,
            metric: Metric::OkLab,
            dither: DitherMode::Bayer8,
            strength: 32,
            matte: [0, 0, 0],
            exhaustive_background: false,
            no_dither: false,
        }
    }
}

/// Quality report for a conversion.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    /// Mean per-pixel squared metric error, modelled at search time (cell
    /// modes: the winning candidates' mixing-aware error; planar: distance
    /// to the nearest generated palette entry, pre-dither).
    pub mean_error: f32,
    /// Cells whose mean per-pixel error exceeds
    /// [`CELL_ERROR_THRESHOLD`] — content the hardware rule genuinely
    /// cannot hold. Always 0 for planar modes (no cells).
    pub cells_over_threshold: usize,
    /// Total cells scored (0 for planar modes).
    pub total_cells: usize,
    /// True when a pre-pass found the input already exactly on the target:
    /// dimensions match the paper, every pixel sits exactly on a palette
    /// colour (for planar: on the gamut grid, within budget), and every
    /// cell already satisfies its constraint rule.
    pub already_constrained: bool,
}

/// One cell's colour decision, exposed for the format bridges and for
/// inspection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellChoice {
    /// ZX Spectrum INK/PAPER/BRIGHT.
    Spectrum(SpectrumChoice),
    /// C64 hires colour pair.
    Hires(HiresChoice),
    /// C64 multicolour background + free colours.
    Multi(MultiChoice),
}

/// The result of a conversion: indexed pixels against `palette`, per-cell
/// choices, and the quality report.
#[derive(Debug, Clone)]
pub struct Conversion {
    /// The machine converted for.
    pub machine_id: String,
    /// The screen mode converted for.
    pub mode_name: String,
    /// Output width in mode pixels (the mode's paper width).
    pub width: u32,
    /// Output height in mode pixels.
    pub height: u32,
    /// Row-major palette indices, `width × height` entries.
    pub pixels: Vec<u8>,
    /// The resolved palette the indices refer to: the named interpretation
    /// for fixed-palette machines, or the generated gamut-rounded palette
    /// for planar targets.
    pub palette: Vec<Rgb>,
    /// Per-cell choices, row-major cell order. Empty for planar modes.
    pub cells: Vec<CellChoice>,
    /// The global background palette index (C64 multicolour only).
    pub background: Option<u8>,
    /// Bitplane count for planar targets (`None` for cell modes).
    pub n_planes: Option<u8>,
    /// Quality report.
    pub report: Report,
}

/// Run the full pipeline: normalise → linear light → PAR-corrected
/// letterbox → constraint search / quantisation → dither → indexed output.
///
/// # Errors
///
/// Any [`ConvertError`]: unknown machine/mode/interpretation, dimensions
/// over the sanity cap, invalid strength, or error diffusion requested for
/// a cell-constrained mode.
pub fn convert(img: &image::DynamicImage, opts: &Options) -> Result<Conversion, ConvertError> {
    if opts.strength > 64 {
        return Err(ConvertError::InvalidStrength {
            strength: opts.strength,
        });
    }
    let machine =
        mediaspec::machine(&opts.machine).ok_or_else(|| ConvertError::UnknownMachine {
            machine: opts.machine.clone(),
        })?;
    let mode = machine
        .mode(&opts.mode)
        .ok_or_else(|| ConvertError::UnknownMode {
            machine: opts.machine.clone(),
            mode: opts.mode.clone(),
        })?;

    let normalised = normalise::normalise(img, opts.matte)?;
    let linear = resize::to_linear(&normalised);
    let target = resize::letterbox(&linear, mode, srgb8_to_linear(opts.matte));

    match mode.constraint {
        ConstraintRule::Planar { max_planes } => {
            convert_planar(machine, mode, opts, &normalised, &target, max_planes)
        }
        _ => convert_cells(machine, mode, opts, &normalised, &target),
    }
}

/// Cell-constrained path: Spectrum attributes, C64 hires, C64 multicolour.
fn convert_cells(
    machine: &MachineGraphics,
    mode: &ScreenMode,
    opts: &Options,
    normalised: &Rgb8Image,
    target: &LinearImage,
) -> Result<Conversion, ConvertError> {
    if !opts.dither.is_ordered() && !opts.no_dither {
        return Err(ConvertError::DiffusionNeedsFreePalette);
    }
    let palette = resolve_palette(machine, opts)?;
    let cell = mode.cell.ok_or(ConvertError::Internal {
        what: "cell-constrained mode without a cell grid",
    })?;
    let (cell_w, cell_h) = (usize::from(cell.width), usize::from(cell.height));

    let searcher = CellSearcher::new(PaletteData::new(palette, opts.metric));
    let width = target.width as usize;
    let cells_x = width / cell_w;
    let cells_y = target.height as usize / cell_h;
    let pixels_per_cell = cell_w * cell_h;

    // Per-cell search.
    let (outcome, background) = match mode.constraint {
        ConstraintRule::SpectrumAttr => {
            let outcome = search_all_cells(target, cell_w, cell_h, |cell_px| {
                let (choice, score) = searcher.spectrum(cell_px);
                (
                    CellChoice::Spectrum(choice),
                    pair_allowed(choice.ink, choice.paper),
                    score,
                )
            });
            (outcome, None)
        }
        ConstraintRule::C64Hires => {
            let outcome = search_all_cells(target, cell_w, cell_h, |cell_px| {
                let (choice, score) = searcher.c64_hires(cell_px);
                (
                    CellChoice::Hires(choice),
                    pair_allowed(choice.fg, choice.bg),
                    score,
                )
            });
            (outcome, None)
        }
        ConstraintRule::C64Multicolour => {
            let backgrounds: Vec<u8> = if opts.exhaustive_background {
                (0..u8::try_from(searcher.pal.len()).unwrap_or(u8::MAX)).collect()
            } else {
                vec![super::constrain::choose_background(target, &searcher.pal)]
            };
            // Lowest total score wins; ascending enumeration + strict <
            // gives the lowest background index on ties.
            let mut best: Option<(CellOutcome, u8, f32)> = None;
            for bg in backgrounds {
                let outcome = search_all_cells(target, cell_w, cell_h, |cell_px| {
                    let (choice, score) = searcher.c64_multi(cell_px, bg);
                    let mut colours = choice.colours.to_vec();
                    colours.sort_unstable();
                    colours.dedup();
                    (CellChoice::Multi(choice), colours, score)
                });
                let total: f32 = outcome.scores.iter().sum();
                if best.as_ref().is_none_or(|(_, _, t)| total < *t) {
                    best = Some((outcome, bg, total));
                }
            }
            let (outcome, bg, _) = best.ok_or(ConvertError::Internal {
                what: "no background candidate evaluated",
            })?;
            (outcome, Some(bg))
        }
        ConstraintRule::Planar { .. } => {
            return Err(ConvertError::Internal {
                what: "planar rule reached the cell path",
            });
        }
    };

    let strength = if opts.no_dither { 0 } else { opts.strength };
    let pixels = dither::render_cells(
        target,
        &searcher,
        &outcome.allowed,
        cell_w,
        cell_h,
        opts.dither,
        strength,
    );

    #[allow(clippy::cast_precision_loss)]
    let mean_error =
        outcome.scores.iter().sum::<f32>() / (pixels_per_cell as f32 * (cells_x * cells_y) as f32);
    #[allow(clippy::cast_precision_loss)]
    let cells_over_threshold = outcome
        .scores
        .iter()
        .filter(|&&s| s / pixels_per_cell as f32 > CELL_ERROR_THRESHOLD)
        .count();

    let already_constrained =
        cell_input_already_constrained(normalised, mode, palette, cell_w, cell_h);

    Ok(Conversion {
        machine_id: machine.id.to_owned(),
        mode_name: mode.name.to_owned(),
        width: target.width,
        height: target.height,
        pixels,
        palette: palette.to_vec(),
        cells: outcome.choices,
        background,
        n_planes: None,
        report: Report {
            mean_error,
            cells_over_threshold,
            total_cells: cells_x * cells_y,
            already_constrained,
        },
    })
}

/// Free-palette planar path (Amiga OCS): median-cut, gamut rounding,
/// ordered dither or serpentine error diffusion.
fn convert_planar(
    machine: &MachineGraphics,
    mode: &ScreenMode,
    opts: &Options,
    normalised: &Rgb8Image,
    target: &LinearImage,
    max_planes: u8,
) -> Result<Conversion, ConvertError> {
    let PaletteModel::Gamut { bits_per_gun } = machine.palette else {
        return Err(ConvertError::Internal {
            what: "planar mode on a fixed-palette machine",
        });
    };
    if let Some(name) = &opts.interpretation {
        return Err(ConvertError::UnknownInterpretation {
            machine: machine.id.to_owned(),
            name: name.clone(),
        });
    }

    let budget = 1usize << max_planes;
    let palette = quantise::generate_palette(target, budget, bits_per_gun);
    let pal = PaletteData::new(&palette, opts.metric);

    let strength = if opts.no_dither { 0 } else { opts.strength };
    let pixels = if opts.no_dither || opts.dither.is_ordered() {
        dither::ordered_planar(target, &pal, opts.dither, strength)
    } else {
        dither::diffuse_planar(target, &pal, opts.dither)
    };

    // Modelled error: nearest-palette distance, pre-dither.
    let mut error_sum = 0.0f64;
    for &p in &target.pixels {
        let proj = pal.metric.project(p);
        let nearest = pal.nearest(proj);
        error_sum += f64::from(pal.metric.distance_sq(proj, pal.proj[usize::from(nearest)]));
    }
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    let mean_error = (error_sum / target.pixels.len() as f64) as f32;

    let already_constrained =
        planar_input_already_constrained(normalised, mode, bits_per_gun, budget);

    Ok(Conversion {
        machine_id: machine.id.to_owned(),
        mode_name: mode.name.to_owned(),
        width: target.width,
        height: target.height,
        pixels,
        n_planes: Some(needed_planes(palette.len(), max_planes)),
        palette,
        cells: Vec::new(),
        background: None,
        report: Report {
            mean_error,
            cells_over_threshold: 0,
            total_cells: 0,
            already_constrained,
        },
    })
}

/// Resolve a fixed-palette machine's interpretation (requested name or the
/// pinned default).
fn resolve_palette<'m>(
    machine: &'m MachineGraphics,
    opts: &Options,
) -> Result<&'m [Rgb], ConvertError> {
    let name = opts
        .interpretation
        .as_deref()
        .or(machine.default_interpretation)
        .ok_or_else(|| ConvertError::UnknownInterpretation {
            machine: machine.id.to_owned(),
            name: opts.interpretation.clone().unwrap_or_default(),
        })?;
    machine
        .interpretation(name)
        .map(|p| p.colours)
        .ok_or_else(|| ConvertError::UnknownInterpretation {
            machine: machine.id.to_owned(),
            name: name.to_owned(),
        })
}

/// One full pass of per-cell search results.
struct CellOutcome {
    /// Each cell's colour decision, row-major cell order.
    choices: Vec<CellChoice>,
    /// Each cell's allowed colour list (sorted ascending, deduplicated).
    allowed: Vec<Vec<u8>>,
    /// Each cell's summed search error.
    scores: Vec<f32>,
}

/// Run `search` over every cell, row-major.
fn search_all_cells(
    target: &LinearImage,
    cell_w: usize,
    cell_h: usize,
    mut search: impl FnMut(&[[f32; 3]]) -> (CellChoice, Vec<u8>, f32),
) -> CellOutcome {
    let width = target.width as usize;
    let cells_x = width / cell_w;
    let cells_y = target.height as usize / cell_h;

    let mut outcome = CellOutcome {
        choices: Vec::with_capacity(cells_x * cells_y),
        allowed: Vec::with_capacity(cells_x * cells_y),
        scores: Vec::with_capacity(cells_x * cells_y),
    };
    let mut cell_px = vec![[0.0f32; 3]; cell_w * cell_h];

    for cy in 0..cells_y {
        for cx in 0..cells_x {
            for row in 0..cell_h {
                let base = (cy * cell_h + row) * width + cx * cell_w;
                cell_px[row * cell_w..(row + 1) * cell_w]
                    .copy_from_slice(&target.pixels[base..base + cell_w]);
            }
            let (choice, list, score) = search(&cell_px);
            outcome.choices.push(choice);
            outcome.allowed.push(list);
            outcome.scores.push(score);
        }
    }
    outcome
}

/// Sorted, deduplicated two-colour allowed list.
fn pair_allowed(a: u8, b: u8) -> Vec<u8> {
    if a == b {
        vec![a]
    } else {
        vec![a.min(b), a.max(b)]
    }
}

/// Minimal plane count for a palette size, clamped to the mode's budget.
fn needed_planes(palette_len: usize, max_planes: u8) -> u8 {
    let mut planes = 1u8;
    while (1usize << planes) < palette_len && planes < max_planes {
        planes += 1;
    }
    planes
}

/// Pre-pass: is the (un-resized) input already exactly on a cell-mode
/// target? Dimensions must match the paper; every pixel must sit exactly on
/// a palette colour (lowest matching index canonicalises the shared
/// Spectrum black); every cell's distinct colour set must satisfy the rule.
fn cell_input_already_constrained(
    normalised: &Rgb8Image,
    mode: &ScreenMode,
    palette: &[Rgb],
    cell_w: usize,
    cell_h: usize,
) -> bool {
    if normalised.width != u32::from(mode.paper_width)
        || normalised.height != u32::from(mode.paper_height)
    {
        return false;
    }
    let Some(indices) = exact_indices(normalised, palette) else {
        return false;
    };

    let width = normalised.width as usize;
    let cells_x = width / cell_w;
    let cells_y = normalised.height as usize / cell_h;
    let mut four_distinct_sets: Vec<Vec<u8>> = Vec::new();

    for cy in 0..cells_y {
        for cx in 0..cells_x {
            let mut distinct: Vec<u8> = Vec::new();
            for row in 0..cell_h {
                let base = (cy * cell_h + row) * width + cx * cell_w;
                for &idx in &indices[base..base + cell_w] {
                    if !distinct.contains(&idx) {
                        distinct.push(idx);
                    }
                }
            }
            distinct.sort_unstable();
            match mode.constraint {
                ConstraintRule::SpectrumAttr => {
                    let normal_ok = distinct.iter().all(|&c| c <= 7);
                    let bright_ok = distinct.iter().all(|&c| c == 0 || (9..=15).contains(&c));
                    if distinct.len() > 2 || (!normal_ok && !bright_ok) {
                        return false;
                    }
                }
                ConstraintRule::C64Hires => {
                    if distinct.len() > 2 {
                        return false;
                    }
                }
                ConstraintRule::C64Multicolour => {
                    if distinct.len() > 4 {
                        return false;
                    }
                    if distinct.len() == 4 {
                        four_distinct_sets.push(distinct);
                    }
                }
                ConstraintRule::Planar { .. } => return false,
            }
        }
    }

    // Multicolour: cells with four distinct colours must share at least
    // one colour that can serve as the global background.
    if let Some(first) = four_distinct_sets.first() {
        return first
            .iter()
            .any(|c| four_distinct_sets.iter().all(|set| set.contains(c)));
    }
    true
}

/// Pre-pass for planar targets: dimensions match, every channel sits on
/// the gamut grid, and the distinct colour count fits the budget.
fn planar_input_already_constrained(
    normalised: &Rgb8Image,
    mode: &ScreenMode,
    bits_per_gun: u8,
    budget: usize,
) -> bool {
    if normalised.width != u32::from(mode.paper_width)
        || normalised.height != u32::from(mode.paper_height)
    {
        return false;
    }
    let mut distinct: Vec<[u8; 3]> = Vec::new();
    for &p in &normalised.pixels {
        if p.iter()
            .any(|&v| quantise::round_channel_to_gamut(v, bits_per_gun) != v)
        {
            return false;
        }
        if !distinct.contains(&p) {
            if distinct.len() >= budget {
                return false;
            }
            distinct.push(p);
        }
    }
    true
}

/// Map every pixel to the lowest palette index with an exactly equal RGB,
/// or `None` if any pixel misses the palette.
fn exact_indices(img: &Rgb8Image, palette: &[Rgb]) -> Option<Vec<u8>> {
    img.pixels
        .iter()
        .map(|&[r, g, b]| {
            palette
                .iter()
                .position(|c| c.r == r && c.g == g && c.b == b)
                .and_then(|i| u8::try_from(i).ok())
        })
        .collect()
}

impl Conversion {
    /// Bridge to the Spectrum SCR codec input.
    ///
    /// # Errors
    ///
    /// [`ConvertError::WrongTarget`] unless this conversion targeted the
    /// Spectrum `standard` mode; [`ConvertError::Internal`] if a pixel
    /// escaped its cell's colours (a pipeline bug).
    pub fn to_scr(&self) -> Result<scr::Screen, ConvertError> {
        self.expect_target("to_scr", "sinclair-zx-spectrum", "standard")?;
        let mut screen = scr::Screen::blank();
        let width = self.width as usize;

        for (cell_idx, choice) in self.cells.iter().enumerate() {
            let CellChoice::Spectrum(attr) = choice else {
                return Err(ConvertError::Internal {
                    what: "non-Spectrum cell choice in a Spectrum conversion",
                });
            };
            let flash = 0u8;
            screen.attributes[cell_idx] = flash << 7
                | u8::from(attr.bright) << 6
                | attr.paper_value() << 3
                | attr.ink_value();

            let cy = cell_idx / scr::COLUMNS;
            let cx = cell_idx % scr::COLUMNS;
            for row in 0..8 {
                let y = cy * 8 + row;
                let mut byte = 0u8;
                for bit in 0..8 {
                    let px = self.pixels[y * width + cx * 8 + bit];
                    let set = if px == attr.paper {
                        false // Paper wins when ink == paper.
                    } else if px == attr.ink {
                        true
                    } else {
                        return Err(ConvertError::Internal {
                            what: "pixel outside its Spectrum cell colours",
                        });
                    };
                    byte |= u8::from(set) << (7 - bit);
                }
                screen.bitmap[y * scr::COLUMNS + cx] = byte;
            }
        }
        Ok(screen)
    }

    /// Bridge to the C64 Art Studio (hires) codec input.
    ///
    /// # Errors
    ///
    /// [`ConvertError::WrongTarget`] unless this conversion targeted the
    /// C64 `hires-bitmap` mode; [`ConvertError::Internal`] on a pipeline
    /// bug.
    pub fn to_art_studio(&self) -> Result<art_studio::ArtStudio, ConvertError> {
        self.expect_target("to_art_studio", "commodore-c64", "hires-bitmap")?;
        let mut img = art_studio::ArtStudio::blank();
        let width = self.width as usize;

        for (cell_idx, choice) in self.cells.iter().enumerate() {
            let CellChoice::Hires(pair) = choice else {
                return Err(ConvertError::Internal {
                    what: "non-hires cell choice in a hires conversion",
                });
            };
            img.screen_ram[cell_idx] = pair.fg << 4 | pair.bg;

            let cy = cell_idx / art_studio::CELL_COLUMNS;
            let cx = cell_idx % art_studio::CELL_COLUMNS;
            for row in 0..8 {
                let y = cy * 8 + row;
                let mut byte = 0u8;
                for bit in 0..8 {
                    let px = self.pixels[y * width + cx * 8 + bit];
                    let set = if px == pair.bg {
                        false // Background wins when fg == bg.
                    } else if px == pair.fg {
                        true
                    } else {
                        return Err(ConvertError::Internal {
                            what: "pixel outside its hires cell colours",
                        });
                    };
                    byte |= u8::from(set) << (7 - bit);
                }
                let offset = (cy * art_studio::CELL_COLUMNS + cx) * 8 + row;
                img.bitmap[offset] = byte;
            }
        }
        Ok(img)
    }

    /// Bridge to the C64 Koala (multicolour) codec input.
    ///
    /// # Errors
    ///
    /// [`ConvertError::WrongTarget`] unless this conversion targeted the
    /// C64 `multicolour-bitmap` mode; [`ConvertError::Internal`] on a
    /// pipeline bug.
    pub fn to_koala(&self) -> Result<koala::Koala, ConvertError> {
        self.expect_target("to_koala", "commodore-c64", "multicolour-bitmap")?;
        let background = self.background.ok_or(ConvertError::Internal {
            what: "multicolour conversion without a background",
        })?;
        let mut img = koala::Koala::blank();
        img.background = background;
        let width = self.width as usize;

        for (cell_idx, choice) in self.cells.iter().enumerate() {
            let CellChoice::Multi(multi) = choice else {
                return Err(ConvertError::Internal {
                    what: "non-multicolour cell choice in a multicolour conversion",
                });
            };
            let [bg, c01, c10, c11] = multi.colours;
            img.screen_ram[cell_idx] = c01 << 4 | c10;
            img.color_ram[cell_idx] = c11;

            let cy = cell_idx / koala::CELL_COLUMNS;
            let cx = cell_idx % koala::CELL_COLUMNS;
            for row in 0..8 {
                let y = cy * 8 + row;
                let mut byte = 0u8;
                for pos in 0..4 {
                    let px = self.pixels[y * width + cx * 4 + pos];
                    // Background first, then the free slots in order.
                    let pair = if px == bg {
                        0b00
                    } else if px == c01 {
                        0b01
                    } else if px == c10 {
                        0b10
                    } else if px == c11 {
                        0b11
                    } else {
                        return Err(ConvertError::Internal {
                            what: "pixel outside its multicolour cell colours",
                        });
                    };
                    byte |= pair << (6 - 2 * pos);
                }
                let offset = (cy * koala::CELL_COLUMNS + cx) * 8 + row;
                img.bitmap[offset] = byte;
            }
        }
        Ok(img)
    }

    /// Bridge to the Amiga ILBM codec input. The palette is emitted as
    /// 8-bit-per-gun triples already scaled from the 4-bit gamut
    /// (`v = level·17`, produced by the quantiser); CAMG carries the HIRES
    /// bit for hires modes and 0 for lores.
    ///
    /// # Errors
    ///
    /// [`ConvertError::WrongTarget`] unless this conversion targeted an
    /// Amiga planar mode.
    pub fn to_ilbm(&self) -> Result<ilbm::Ilbm, ConvertError> {
        if self.machine_id != "commodore-amiga-ocs" {
            return Err(ConvertError::WrongTarget {
                bridge: "to_ilbm",
                expected: "commodore-amiga-ocs planar",
                actual: format!("{} {}", self.machine_id, self.mode_name),
            });
        }
        let n_planes = self.n_planes.ok_or(ConvertError::Internal {
            what: "planar conversion without a plane count",
        })?;
        let camg = if self.mode_name.starts_with("hires") {
            ilbm::CAMG_HIRES
        } else {
            0
        };
        Ok(ilbm::Ilbm {
            width: u16::try_from(self.width).unwrap_or(u16::MAX),
            height: u16::try_from(self.height).unwrap_or(u16::MAX),
            n_planes,
            palette: self.palette.iter().map(|c| [c.r, c.g, c.b]).collect(),
            pixels: self.pixels.clone(),
            camg,
        })
    }

    fn expect_target(
        &self,
        bridge: &'static str,
        machine: &'static str,
        mode: &'static str,
    ) -> Result<(), ConvertError> {
        if self.machine_id == machine && self.mode_name == mode {
            Ok(())
        } else {
            Err(ConvertError::WrongTarget {
                bridge,
                expected: machine,
                actual: format!("{} {}", self.machine_id, self.mode_name),
            })
        }
    }
}
