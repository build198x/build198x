//! Deterministic palette generation for free-palette (Amiga planar)
//! targets: median-cut in linear space, then rounding to the machine
//! gamut's bits-per-gun.
//!
//! Median-cut rules (documented per the plan, all deterministic):
//!
//! - Buckets hold pixel indices. Splitting repeats until the colour budget
//!   is reached or no bucket is splittable (zero range in every channel).
//! - **Bucket selection:** the bucket with the widest single-channel range;
//!   ties break to the lowest bucket index.
//! - **Channel selection:** the widest channel within that bucket; ties
//!   break R before G before B.
//! - **Median:** the bucket's pixels are sorted by `(channel value, pixel
//!   index)` — `f32::total_cmp` plus the stable index tie-break — and split
//!   at `len / 2`; the lower half keeps the bucket's slot, the upper half
//!   is appended at the end of the bucket list.
//! - **Averaging:** f64 accumulation in each bucket's stored (sorted)
//!   order.
//!
//! Gamut rounding: a linear average is encoded back to 8-bit sRGB by
//! nearest-LUT-entry search ([`linear_to_srgb8`] — the LUT is strictly
//! monotonic, so this is the deterministic inverse; lower code wins ties),
//! then quantised per channel as `level = (v·levels + 127) / 255`,
//! `display = (level·255 + levels/2) / levels` — for the 4-bit Amiga gamut
//! that is exactly `level = (v·15 + 127)/255`, `display = level·17`.

use mediaspec::Rgb;

use super::LinearImage;
use super::colour::SRGB_TO_LINEAR;

/// Deterministic median-cut of linear-light pixels down to at most
/// `budget` representative linear colours (see the module docs for the
/// split/tie rules).
#[must_use]
pub fn median_cut(pixels: &[[f32; 3]], budget: usize) -> Vec<[f32; 3]> {
    if pixels.is_empty() || budget == 0 {
        return Vec::new();
    }

    // Buckets of pixel indices; start with everything in one bucket.
    let mut buckets: Vec<Vec<u32>> = vec![(0..pixels.len() as u32).collect()];

    while buckets.len() < budget {
        // Widest single-channel range across buckets; lowest bucket index
        // wins ties (strict > on the comparison).
        let mut pick: Option<(usize, usize)> = None; // (bucket, channel)
        let mut widest = 0.0f32;
        for (bi, bucket) in buckets.iter().enumerate() {
            let mut lo = [f32::INFINITY; 3];
            let mut hi = [f32::NEG_INFINITY; 3];
            for &pi in bucket {
                for ((l, h), &v) in lo.iter_mut().zip(hi.iter_mut()).zip(&pixels[pi as usize]) {
                    if v < *l {
                        *l = v;
                    }
                    if v > *h {
                        *h = v;
                    }
                }
            }
            for (ch, (&h, &l)) in hi.iter().zip(&lo).enumerate() {
                let range = h - l;
                if range > widest {
                    widest = range;
                    pick = Some((bi, ch));
                }
            }
        }
        let Some((bi, ch)) = pick else {
            break; // Every bucket is a single colour: unsplittable.
        };

        buckets[bi].sort_unstable_by(|&a, &b| {
            pixels[a as usize][ch]
                .total_cmp(&pixels[b as usize][ch])
                .then(a.cmp(&b))
        });
        let mid = buckets[bi].len() / 2;
        let upper = buckets[bi].split_off(mid);
        buckets.push(upper);
    }

    buckets
        .iter()
        .map(|bucket| {
            let mut acc = [0.0f64; 3];
            for &pi in bucket {
                let p = pixels[pi as usize];
                acc[0] += f64::from(p[0]);
                acc[1] += f64::from(p[1]);
                acc[2] += f64::from(p[2]);
            }
            #[allow(clippy::cast_precision_loss)]
            let n = bucket.len() as f64;
            #[allow(clippy::cast_possible_truncation)]
            [
                (acc[0] / n) as f32,
                (acc[1] / n) as f32,
                (acc[2] / n) as f32,
            ]
        })
        .collect()
}

/// Encode a linear value back to the nearest 8-bit sRGB code by searching
/// the strictly monotonic [`SRGB_TO_LINEAR`] LUT — the deterministic
/// inverse of the decode (binary search + neighbour compare; the lower
/// code wins ties).
#[must_use]
pub fn linear_to_srgb8(v: f32) -> u8 {
    if v <= 0.0 {
        return 0;
    }
    if v >= 1.0 {
        return 255;
    }
    // First code whose linear value exceeds v.
    let above = SRGB_TO_LINEAR.partition_point(|&e| e <= v);
    let below = above - 1; // safe: SRGB_TO_LINEAR[0] = 0.0 <= v
    if above > 255 {
        return 255;
    }
    let d_below = v - SRGB_TO_LINEAR[below];
    let d_above = SRGB_TO_LINEAR[above] - v;
    // Strict <: the lower code wins an exact tie.
    #[allow(clippy::cast_possible_truncation)]
    if d_above < d_below {
        above as u8
    } else {
        below as u8
    }
}

/// Round one 8-bit channel to a `bits_per_gun` gamut and scale back to
/// 8-bit display form: `level = (v·levels + 127) / 255`, `display =
/// (level·255 + levels/2) / levels`. For 4 bits: `display = level·17`.
#[must_use]
pub fn round_channel_to_gamut(v: u8, bits_per_gun: u8) -> u8 {
    let levels = (1u32 << bits_per_gun) - 1;
    let level = (u32::from(v) * levels + 127) / 255;
    u8::try_from((level * 255 + levels / 2) / levels).unwrap_or(u8::MAX)
}

/// Generate a free palette for a linear image: median-cut to `budget`
/// entries, encode to 8-bit sRGB, round each gun to the gamut depth, then
/// sort ascending by `(r, g, b)` and deduplicate (rounding can merge
/// adjacent buckets).
#[must_use]
pub fn generate_palette(img: &LinearImage, budget: usize, bits_per_gun: u8) -> Vec<Rgb> {
    let mut entries: Vec<Rgb> = median_cut(&img.pixels, budget)
        .iter()
        .map(|&lin| Rgb {
            r: round_channel_to_gamut(linear_to_srgb8(lin[0]), bits_per_gun),
            g: round_channel_to_gamut(linear_to_srgb8(lin[1]), bits_per_gun),
            b: round_channel_to_gamut(linear_to_srgb8(lin[2]), bits_per_gun),
        })
        .collect();
    entries.sort_unstable_by_key(|c| (c.r, c.g, c.b));
    entries.dedup();
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_to_srgb8_round_trips_every_code() {
        for code in 0..=255u8 {
            assert_eq!(linear_to_srgb8(SRGB_TO_LINEAR[usize::from(code)]), code);
        }
    }

    #[test]
    fn gamut_round_is_idempotent_on_grid_values() {
        for level in 0..16u8 {
            let v = level * 17;
            assert_eq!(round_channel_to_gamut(v, 4), v);
        }
    }
}
