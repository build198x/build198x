//! Final indexed rendering: ordered Bayer dithering (cell-restricted and
//! free-palette), plus serpentine error diffusion for free-palette targets.
//!
//! **Cell modes** ([`render_cells`]): each pixel is restricted to its
//! cell's chosen colours. The mix is computed gamma-correctly — the best
//! `(pair, k)` mix is found in linear space via the same mixing-aware
//! machinery as the constraint search, then thresholded by the Bayer
//! matrix. The strength knob `s` (0..=64, default 32) interpolates the
//! threshold between flat 0.5 (no dithering) and the full matrix:
//! `θ = 0.5 + (bayer − 0.5)·(s/64)`, choose the pair's high colour when the
//! mix fraction `t = k/8` exceeds `θ`. Strength 0 short-circuits to
//! nearest-colour within the cell (lowest palette index on ties).
//!
//! **Free-palette ordered dither** ([`ordered_planar`]): classic bias
//! injection — `p′ = p + (bayer − 0.5)·A·(s/64)` per linear channel, then
//! nearest palette entry. The amplitude `A = 1/15` (one 4-bit gamut step,
//! taken in linear units) is a documented constant.
//!
//! **Error diffusion** ([`diffuse_planar`]): serpentine Floyd–Steinberg or
//! Atkinson, fixed traversal (even rows left→right, odd rows right→left,
//! kernel mirrored), f32 linear accumulation, basic ops — free-palette
//! targets only, because diffused error cannot respect attribute-cell
//! boundaries.
//!
//! Determinism: single-threaded fixed-order passes, strict `<` selections
//! throughout (`decisions/determinism-contract.md`).

use super::LinearImage;
use super::constrain::{CellSearcher, MIX_LEVELS, PaletteData};

/// The 4×4 ordered-dither matrix (values 0..=15).
const BAYER4: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

/// The 8×8 ordered-dither matrix (values 0..=63).
const BAYER8: [[u8; 8]; 8] = [
    [0, 32, 8, 40, 2, 34, 10, 42],
    [48, 16, 56, 24, 50, 18, 58, 26],
    [12, 44, 4, 36, 14, 46, 6, 38],
    [60, 28, 52, 20, 62, 30, 54, 22],
    [3, 35, 11, 43, 1, 33, 9, 41],
    [51, 19, 59, 27, 49, 17, 57, 25],
    [15, 47, 7, 39, 13, 45, 5, 37],
    [63, 31, 55, 23, 61, 29, 53, 21],
];

/// Free-palette ordered-dither bias amplitude: one 4-bit gamut step, taken
/// in linear units (documented constant; see the module docs).
const ORDERED_AMPLITUDE: f32 = 1.0 / 15.0;

/// Dither algorithm selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DitherMode {
    /// Ordered, 4×4 Bayer matrix.
    Bayer4,
    /// Ordered, 8×8 Bayer matrix (the default for cell-constrained
    /// targets — see `pipeline::default_dither`).
    #[default]
    Bayer8,
    /// Serpentine Floyd–Steinberg error diffusion (free-palette only; the
    /// default for planar targets — see `pipeline::default_dither`).
    FloydSteinberg,
    /// Serpentine Atkinson error diffusion (free-palette only).
    Atkinson,
}

impl DitherMode {
    /// Whether this mode is an ordered (Bayer) dither.
    #[must_use]
    pub fn is_ordered(self) -> bool {
        matches!(self, Self::Bayer4 | Self::Bayer8)
    }
}

/// The normalised Bayer threshold at `(x, y)`: `(m + 0.5) / n²`, in (0, 1).
/// `Bayer8` is used for the error-diffusion variants too (callers reject
/// those before thresholding matters).
#[must_use]
fn bayer_threshold(mode: DitherMode, x: usize, y: usize) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    match mode {
        DitherMode::Bayer4 => (f32::from(BAYER4[y % 4][x % 4]) + 0.5) / 16.0,
        _ => (f32::from(BAYER8[y % 8][x % 8]) + 0.5) / 64.0,
    }
}

/// Render a cell-constrained image to palette indices: each pixel draws
/// only from its cell's `allowed` colour list (sorted ascending palette
/// index). `projected` holds the image's pixels already metric-projected,
/// row-major `width × height`. `strength` 0 = nearest colour within the
/// cell; otherwise the best linear-space mix is thresholded against the
/// Bayer matrix (see the module docs for the strength mapping).
#[must_use]
#[allow(clippy::too_many_arguments)] // One knob per documented dither input.
pub fn render_cells(
    projected: &[[f32; 3]],
    width: usize,
    height: usize,
    searcher: &CellSearcher,
    allowed: &[Vec<u8>],
    cell_w: usize,
    cell_h: usize,
    mode: DitherMode,
    strength: u8,
) -> Vec<u8> {
    let cells_per_row = width / cell_w;
    let strength_t = f32::from(strength) / 64.0;

    let mut out = vec![0u8; width * height];
    for y in 0..height {
        for x in 0..width {
            let cell = (y / cell_h) * cells_per_row + x / cell_w;
            let colours = &allowed[cell];
            let proj = projected[y * width + x];
            out[y * width + x] = if strength == 0 {
                nearest_of(&searcher.pal, proj, colours)
            } else {
                let (lo, hi, k) = searcher.best_mix(proj, colours);
                #[allow(clippy::cast_precision_loss)]
                let t = k as f32 / MIX_LEVELS as f32;
                let theta = 0.5 + (bayer_threshold(mode, x, y) - 0.5) * strength_t;
                if t > theta { hi } else { lo }
            };
        }
    }
    out
}

/// Nearest colour among `allowed` (sorted ascending): strict `<` scan, so
/// the lowest palette index wins on equal distance.
fn nearest_of(pal: &PaletteData, proj: [f32; 3], allowed: &[u8]) -> u8 {
    let mut best = allowed[0];
    let mut best_d = f32::INFINITY;
    for &c in allowed {
        let d = pal.metric.distance_sq(proj, pal.proj[usize::from(c)]);
        if d < best_d {
            best_d = d;
            best = c;
        }
    }
    best
}

/// Ordered dither against a free (whole-image) palette: per-channel linear
/// bias injection, then nearest palette entry (lowest index on ties).
/// `strength` 0 degenerates to plain nearest-colour mapping.
#[must_use]
pub fn ordered_planar(
    img: &LinearImage,
    pal: &PaletteData,
    mode: DitherMode,
    strength: u8,
) -> Vec<u8> {
    let width = img.width as usize;
    let height = img.height as usize;
    let strength_t = f32::from(strength) / 64.0;

    let mut out = vec![0u8; width * height];
    for y in 0..height {
        for x in 0..width {
            let p = img.pixels[y * width + x];
            let bias = (bayer_threshold(mode, x, y) - 0.5) * ORDERED_AMPLITUDE * strength_t;
            let biased = [
                clamp01(p[0] + bias),
                clamp01(p[1] + bias),
                clamp01(p[2] + bias),
            ];
            out[y * width + x] = pal.nearest(pal.metric.project(biased));
        }
    }
    out
}

/// Serpentine error diffusion against a free palette. Fixed traversal:
/// even rows left→right, odd rows right→left with the kernel mirrored.
/// The working value is clamped to 0..=1 before the nearest-colour search
/// and the diffused error is `clamped − chosen` (documented policy). Basic
/// ops, f32 accumulation, single-threaded.
///
/// Kernels (error fractions pushed to neighbours, `→` = scan direction):
///
/// - Floyd–Steinberg: 7/16 ahead; 3/16, 5/16, 1/16 to the next row
///   (behind, below, ahead).
/// - Atkinson: 1/8 to each of: one ahead, two ahead, next row behind /
///   below / ahead, and two rows below (only 6/8 of the error diffuses).
#[must_use]
pub fn diffuse_planar(img: &LinearImage, pal: &PaletteData, mode: DitherMode) -> Vec<u8> {
    let width = img.width as usize;
    let height = img.height as usize;
    let mut work = img.pixels.clone();
    let mut out = vec![0u8; width * height];

    // (dx, dy, weight) with dx in scan direction; mirrored on odd rows.
    let fs: &[(isize, isize, f32)] = &[
        (1, 0, 7.0 / 16.0),
        (-1, 1, 3.0 / 16.0),
        (0, 1, 5.0 / 16.0),
        (1, 1, 1.0 / 16.0),
    ];
    let atkinson: &[(isize, isize, f32)] = &[
        (1, 0, 1.0 / 8.0),
        (2, 0, 1.0 / 8.0),
        (-1, 1, 1.0 / 8.0),
        (0, 1, 1.0 / 8.0),
        (1, 1, 1.0 / 8.0),
        (0, 2, 1.0 / 8.0),
    ];
    let kernel = match mode {
        DitherMode::Atkinson => atkinson,
        _ => fs,
    };

    for y in 0..height {
        let serpentine = !y.is_multiple_of(2);
        for step in 0..width {
            let x = if serpentine { width - 1 - step } else { step };
            let idx = y * width + x;
            let value = [
                clamp01(work[idx][0]),
                clamp01(work[idx][1]),
                clamp01(work[idx][2]),
            ];
            let chosen = pal.nearest(pal.metric.project(value));
            out[idx] = chosen;
            let target = pal.linear[usize::from(chosen)];
            let err = [
                value[0] - target[0],
                value[1] - target[1],
                value[2] - target[2],
            ];
            for &(dx, dy, w) in kernel {
                let dx = if serpentine { -dx } else { dx };
                let nx = x as isize + dx;
                let ny = y as isize + dy;
                if nx < 0 || nx >= width as isize || ny >= height as isize {
                    continue;
                }
                #[allow(clippy::cast_sign_loss)]
                let n = ny as usize * width + nx as usize;
                work[n][0] += err[0] * w;
                work[n][1] += err[1] * w;
                work[n][2] += err[2] * w;
            }
        }
    }
    out
}

fn clamp01(v: f32) -> f32 {
    // f32::clamp is comparisons only — deterministic basic ops.
    v.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bayer_matrices_are_permutations() {
        let mut seen4 = [false; 16];
        for row in &BAYER4 {
            for &v in row {
                seen4[usize::from(v)] = true;
            }
        }
        assert!(seen4.iter().all(|&s| s));

        let mut seen8 = [false; 64];
        for row in &BAYER8 {
            for &v in row {
                seen8[usize::from(v)] = true;
            }
        }
        assert!(seen8.iter().all(|&s| s));
    }

    #[test]
    fn thresholds_stay_inside_open_unit_interval() {
        for y in 0..8 {
            for x in 0..8 {
                for mode in [DitherMode::Bayer4, DitherMode::Bayer8] {
                    let t = bayer_threshold(mode, x, y);
                    assert!(t > 0.0 && t < 1.0);
                }
            }
        }
    }
}
