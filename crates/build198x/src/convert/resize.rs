//! Linear-light letterboxing onto a target mode's paper grid.
//!
//! The fit honours the mode's **pixel aspect ratio** from `mediaspec`: the
//! source (assumed square-pixel) is scaled to the largest PAR-corrected fit
//! inside the paper's display rectangle, centred, and padded with the matte
//! colour. Geometry is computed in **pure integer arithmetic** (u64
//! rationals with round-half-up), so the fit is trivially deterministic.
//!
//! Resampling is a hand-rolled **area-average (box) filter** operating on
//! linear-light f32 buffers — pure accumulation arithmetic per
//! `decisions/determinism-contract.md` (no Lanczos/sinc). Fractional pixel
//! coverage is handled with exact integer overlap weights (dest pixel `j`
//! spans `[j·sh, (j+1)·sh]` in `1/dh` source units, so every weight is an
//! integer); accumulation order is fixed (source rows ascending, columns
//! ascending, f64 accumulators). A source already at the target dimensions
//! short-circuits to a byte-identical copy. Upscaling uses the same box
//! filter.

use mediaspec::ScreenMode;

use super::colour::srgb8_to_linear;
use super::{LinearImage, Rgb8Image};

/// Decode an 8-bit sRGB image to linear light through the const LUT.
#[must_use]
pub fn to_linear(img: &Rgb8Image) -> LinearImage {
    LinearImage {
        width: img.width,
        height: img.height,
        pixels: img.pixels.iter().map(|&p| srgb8_to_linear(p)).collect(),
    }
}

/// The PAR-corrected best fit of a `src_w × src_h` square-pixel source on
/// `mode`'s paper grid: returns `(dest_w, dest_h, offset_x, offset_y)` in
/// mode pixels.
///
/// Derivation: the paper's display rectangle is `paper_w·par_h ×
/// paper_h·par_v` display units (display aspect = `paper_w·par_h /
/// (paper_h·par_v)`). The source fills whichever display axis binds;
/// the other axis scales as `round(num/den)` (round-half-up, integer) and
/// clamps to the paper. Offsets centre the rect with floor division (the
/// odd spare pixel goes right/bottom).
#[must_use]
pub fn fit_rect(src_w: u32, src_h: u32, mode: &ScreenMode) -> (u32, u32, u32, u32) {
    let pw = u64::from(mode.paper_width);
    let ph = u64::from(mode.paper_height);
    let par_h = u64::from(mode.pixel_aspect.horizontal);
    let par_v = u64::from(mode.pixel_aspect.vertical);
    let sw = u64::from(src_w);
    let sh = u64::from(src_h);

    // Display-unit extents of the paper.
    let disp_w = pw * par_h;
    let disp_h = ph * par_v;

    // Width-bound when disp_w/sw <= disp_h/sh, i.e. disp_w·sh <= disp_h·sw.
    let (dest_w, dest_h) = if disp_w * sh <= disp_h * sw {
        // Source spans the full paper width; height scales by the same
        // display factor: dest_h = sh·disp_w / (sw·par_v) mode pixels.
        let h = round_div(sh * disp_w, sw * par_v).clamp(1, ph);
        (pw, h)
    } else {
        let w = round_div(sw * disp_h, sh * par_h).clamp(1, pw);
        (w, ph)
    };

    let ox = (pw - dest_w) / 2;
    let oy = (ph - dest_h) / 2;
    #[allow(clippy::cast_possible_truncation)] // all values clamped <= paper dims (u16)
    (dest_w as u32, dest_h as u32, ox as u32, oy as u32)
}

/// Integer round-half-up division.
fn round_div(num: u64, den: u64) -> u64 {
    (2 * num + den) / (2 * den)
}

/// Box-filter (area-average) resample of a linear-light image to
/// `dw × dh`. Identity dimensions short-circuit to a clone — a
/// byte-identical no-op.
#[must_use]
pub fn box_resample(src: &LinearImage, dw: u32, dh: u32) -> LinearImage {
    if dw == src.width && dh == src.height {
        return src.clone();
    }

    let sw = u64::from(src.width);
    let sh = u64::from(src.height);
    let dwu = u64::from(dw);
    let dhu = u64::from(dh);
    let src_row = usize::try_from(src.width).unwrap_or(0);

    let mut pixels = Vec::with_capacity((dw as usize) * (dh as usize));
    for j in 0..dhu {
        // Dest row j covers [j·sh, (j+1)·sh] in 1/dh source units.
        let y_lo = j * sh;
        let y_hi = (j + 1) * sh;
        let sy_first = y_lo / dhu;
        let sy_last = (y_hi - 1) / dhu; // inclusive
        for i in 0..dwu {
            let x_lo = i * sw;
            let x_hi = (i + 1) * sw;
            let sx_first = x_lo / dwu;
            let sx_last = (x_hi - 1) / dwu; // inclusive

            let mut acc = [0.0f64; 3];
            let mut weight_sum = 0.0f64;
            for sy in sy_first..=sy_last {
                // Overlap of source row sy with the dest span, in integer
                // 1/dh units — exact, no floor/ceil on floats.
                let wy = (y_hi.min((sy + 1) * dhu) - y_lo.max(sy * dhu)) as f64;
                let row_base = usize::try_from(sy).unwrap_or(0) * src_row;
                for sx in sx_first..=sx_last {
                    let wx = (x_hi.min((sx + 1) * dwu) - x_lo.max(sx * dwu)) as f64;
                    let w = wy * wx;
                    let p = src.pixels[row_base + usize::try_from(sx).unwrap_or(0)];
                    acc[0] += w * f64::from(p[0]);
                    acc[1] += w * f64::from(p[1]);
                    acc[2] += w * f64::from(p[2]);
                    weight_sum += w;
                }
            }
            #[allow(clippy::cast_possible_truncation)]
            pixels.push([
                (acc[0] / weight_sum) as f32,
                (acc[1] / weight_sum) as f32,
                (acc[2] / weight_sum) as f32,
            ]);
        }
    }

    LinearImage {
        width: dw,
        height: dh,
        pixels,
    }
}

/// Letterbox a linear-light source onto `mode`'s paper grid: PAR-corrected
/// best fit ([`fit_rect`]), box resample, centre, pad with the linear
/// `matte` colour. When the source already equals the paper dimensions the
/// fit is the identity and the pixel data passes through byte-identically.
#[must_use]
pub fn letterbox(src: &LinearImage, mode: &ScreenMode, matte: [f32; 3]) -> LinearImage {
    let (dw, dh, ox, oy) = fit_rect(src.width, src.height, mode);
    let scaled = box_resample(src, dw, dh);

    let paper_w = u32::from(mode.paper_width);
    let paper_h = u32::from(mode.paper_height);
    if dw == paper_w && dh == paper_h {
        return scaled; // Full-paper fit: nothing to pad.
    }

    let pw = paper_w as usize;
    let mut pixels = vec![matte; pw * paper_h as usize];
    for row in 0..dh as usize {
        let dst_base = (oy as usize + row) * pw + ox as usize;
        let src_base = row * dw as usize;
        pixels[dst_base..dst_base + dw as usize]
            .copy_from_slice(&scaled.pixels[src_base..src_base + dw as usize]);
    }

    LinearImage {
        width: paper_w,
        height: paper_h,
        pixels,
    }
}
