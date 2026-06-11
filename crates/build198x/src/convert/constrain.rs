//! Per-cell exhaustive, mixing-aware constraint search.
//!
//! The heart of the converter: for each attribute cell, enumerate the colour
//! sets the hardware rule allows and score each against the **original
//! pixels** (metric-projected once by the pipeline) with **mixing-aware**
//! error — a candidate set's error for a pixel is the minimum metric
//! distance over every ordered-dither mix the set can achieve
//! (Yliluoma-style: for a colour pair `(a, b)`, mix levels `k/8` for `k` in
//! `0..=8`, mixed in linear space, then projected). Scoring
//! against nearest-single-colour would kill dithering in out-of-gamut
//! regions: a uniform orange cell on the Spectrum must prefer a red+yellow
//! pair (whose k≈3 mix lands on orange) over any flat single colour.
//!
//! Determinism (`decisions/determinism-contract.md`): enumeration orders are
//! documented on each search function and stable; candidate selection uses
//! strict `<` so the **first candidate in enumeration order wins ties**;
//! nearest-colour helpers scan ascending palette index with strict `<` so
//! the **lowest palette index wins on equal distance**. All arithmetic is
//! basic IEEE ops.

use mediaspec::Rgb;

use super::colour::{Metric, srgb8_to_linear};

/// Number of mix steps between a colour pair: mix fractions are `k /
/// MIX_LEVELS` for `k` in `0..=MIX_LEVELS`. 8 gives nine levels — fine
/// enough that an ordered 8×8 Bayer render can actually hit each level,
/// coarse enough to keep the exhaustive searches fast in debug builds.
pub const MIX_LEVELS: usize = 8;

/// C64 multicolour pruning width: per cell, the top `K` palette colours by
/// the deterministic frequency/error-fit ranking (see
/// [`CellSearcher::c64_multi`]) are searched as C(K, 3) free-colour
/// triples. 8 keeps the per-cell search at 56 triples while comfortably
/// covering the colours a 4×8 cell can use.
pub const MULTI_PRUNE_K: usize = 8;

/// The minimum metric distance from `p` to any projection in `row`
/// (strict `<` scan in row order — the earliest minimum wins, per the
/// contract's tie-break rules).
fn min_dist_sq(metric: Metric, p: [f32; 3], row: &[[f32; 3]]) -> f32 {
    let mut best = f32::INFINITY;
    for &m in row {
        let d = metric.distance_sq(p, m);
        if d < best {
            best = d;
        }
    }
    best
}

/// Flat index of the unordered pair `(i, j)`, `i <= j < n`, in the
/// upper-triangle-with-diagonal layout. Row `i` starts after rows `0..i`,
/// which hold `n, n-1, …` entries: offset = `i·(2n − i + 1)/2` (always an
/// even product, no underflow).
fn tri_index(n: usize, i: usize, j: usize) -> usize {
    debug_assert!(i <= j && j < n);
    i * (2 * n - i + 1) / 2 + (j - i)
}

/// A resolved palette with its linear and metric-projected forms
/// precomputed.
#[derive(Debug, Clone)]
pub struct PaletteData {
    /// The metric every distance in this palette context uses.
    pub metric: Metric,
    /// Resolved 8-bit sRGB entries, hardware/palette index order.
    pub srgb: Vec<Rgb>,
    /// Linear-light form of each entry (via the const LUT).
    pub linear: Vec<[f32; 3]>,
    /// Metric projection of each entry.
    pub proj: Vec<[f32; 3]>,
}

impl PaletteData {
    /// Precompute linear and projected forms for a palette.
    #[must_use]
    pub fn new(colours: &[Rgb], metric: Metric) -> Self {
        let linear: Vec<[f32; 3]> = colours
            .iter()
            .map(|c| srgb8_to_linear([c.r, c.g, c.b]))
            .collect();
        let proj = linear.iter().map(|&l| metric.project(l)).collect();
        Self {
            metric,
            srgb: colours.to_vec(),
            linear,
            proj,
        }
    }

    /// Number of palette entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.srgb.len()
    }

    /// Whether the palette is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.srgb.is_empty()
    }

    /// Index of the entry nearest to a projected coordinate. Ascending scan
    /// with strict `<`: the lowest palette index wins on equal distance.
    #[must_use]
    pub fn nearest(&self, proj: [f32; 3]) -> u8 {
        let mut best = 0usize;
        let mut best_d = f32::INFINITY;
        for (i, &p) in self.proj.iter().enumerate() {
            let d = self.metric.distance_sq(proj, p);
            if d < best_d {
                best_d = d;
                best = i;
            }
        }
        u8::try_from(best).unwrap_or(u8::MAX)
    }
}

/// A Spectrum cell's chosen attribute, in **global palette indices** (0–15
/// of the 16-entry interpretation; black is always index 0, even in a
/// bright cell — the lowest-index rule for the shared black).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpectrumChoice {
    /// Palette index of the INK colour.
    pub ink: u8,
    /// Palette index of the PAPER colour.
    pub paper: u8,
    /// The cell's BRIGHT bit (applies to ink and paper together).
    pub bright: bool,
}

impl SpectrumChoice {
    /// Hardware INK value 0–7 (palette index masked to the colour number).
    #[must_use]
    pub fn ink_value(self) -> u8 {
        self.ink & 7
    }

    /// Hardware PAPER value 0–7.
    #[must_use]
    pub fn paper_value(self) -> u8 {
        self.paper & 7
    }
}

/// A C64 hires cell's chosen colour pair (palette indices 0–15).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HiresChoice {
    /// Colour of set bitmap bits (screen RAM upper nybble).
    pub fg: u8,
    /// Colour of clear bitmap bits (screen RAM lower nybble).
    pub bg: u8,
}

/// A C64 multicolour cell's colour set: `[background, %01, %10, %11]`
/// (palette indices 0–15; slot 0 is the global background, slots 1–3 the
/// per-cell free colours sorted ascending by palette index).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MultiChoice {
    /// `[background, c01, c10, c11]`.
    pub colours: [u8; 4],
}

/// The per-image search context: palette plus the precomputed projections
/// of every pair mix, shared by all cells (and by the dither pass).
#[derive(Debug, Clone)]
pub struct CellSearcher {
    /// The palette this searcher scores against.
    pub pal: PaletteData,
    /// `mixes[pair_index(i, j)][k]` = metric projection of the linear-space
    /// mix `lerp(linear[i], linear[j], k / MIX_LEVELS)`.
    mixes: Vec<[[f32; 3]; MIX_LEVELS + 1]>,
}

impl CellSearcher {
    /// Precompute the mix-projection table for every unordered pair
    /// (including the degenerate `(i, i)` solids).
    #[must_use]
    pub fn new(pal: PaletteData) -> Self {
        let n = pal.len();
        let mut mixes = Vec::with_capacity(n * (n + 1) / 2);
        for i in 0..n {
            for j in i..n {
                let a = pal.linear[i];
                let b = pal.linear[j];
                let mut row = [[0.0f32; 3]; MIX_LEVELS + 1];
                for (k, slot) in row.iter_mut().enumerate() {
                    #[allow(clippy::cast_precision_loss)]
                    let t = k as f32 / MIX_LEVELS as f32;
                    let mixed = [
                        a[0] + (b[0] - a[0]) * t,
                        a[1] + (b[1] - a[1]) * t,
                        a[2] + (b[2] - a[2]) * t,
                    ];
                    *slot = pal.metric.project(mixed);
                }
                mixes.push(row);
            }
        }
        Self { pal, mixes }
    }

    /// Flat index of the unordered pair `(i, j)`, `i <= j`, in the
    /// upper-triangle-with-diagonal layout.
    #[must_use]
    pub fn pair_index(&self, i: usize, j: usize) -> usize {
        tri_index(self.pal.len(), i, j)
    }

    /// The precomputed mix projections for pair `(i, j)`, `i <= j`.
    #[must_use]
    pub fn mix_projections(&self, i: u8, j: u8) -> &[[f32; 3]; MIX_LEVELS + 1] {
        &self.mixes[self.pair_index(usize::from(i), usize::from(j))]
    }

    /// The mix-projection row for pair `(a, b)`, `a <= b`, narrowed to a
    /// single entry for solid pairs: every mix of `(a, a)` is `a` itself
    /// (bitwise — `a + (a − a)·t` is exactly `a`), so one distance stands
    /// in for the min over nine identical ones.
    fn mix_row(&self, a: u8, b: u8) -> &[[f32; 3]] {
        let row = self.mix_projections(a, b);
        if a == b { &row[..1] } else { &row[..] }
    }

    /// A whole cell's mixing-aware error against pair `(a, b)` (`a <= b`),
    /// aborting with `None` as soon as the running sum strictly exceeds
    /// `abort_above` (a deterministic prune: it depends only on the data
    /// and the fixed enumeration order, never on timing).
    fn pair_cell_score(&self, projs: &[[f32; 3]], a: u8, b: u8, abort_above: f32) -> Option<f32> {
        let row = self.mix_row(a, b);
        let mut sum = 0.0f32;
        for &p in projs {
            sum += min_dist_sq(self.pal.metric, p, row);
            if sum > abort_above {
                return None;
            }
        }
        Some(sum)
    }

    /// Scan every unordered pair (singles included) from `set` in
    /// lexicographic position order, mixing-aware, pruning against and
    /// updating `best_score` with strict `<` (the first candidate in
    /// enumeration order wins ties — and an incumbent score from an earlier
    /// set survives equal challengers). Returns the winning pair when any
    /// candidate improved on the incoming score.
    fn best_pair(&self, projs: &[[f32; 3]], set: &[u8], best_score: &mut f32) -> Option<(u8, u8)> {
        let mut best = None;
        for (i, &a) in set.iter().enumerate() {
            for &b in &set[i..] {
                if let Some(score) = self.pair_cell_score(projs, a, b, *best_score)
                    && score < *best_score
                {
                    *best_score = score;
                    best = Some((a, b));
                }
            }
        }
        best
    }

    /// Split a winning pair into (majority, minority) roles: the colour
    /// nearest (single-colour distance, ties to the lower index since
    /// `a <= b`) to at least half the pixels is the majority. Used to
    /// assign PAPER (Spectrum) and the clear-bit colour (C64 hires).
    fn majority_roles(&self, projs: &[[f32; 3]], a: u8, b: u8) -> (u8, u8) {
        if a == b {
            return (a, b);
        }
        let pa = self.pal.proj[usize::from(a)];
        let pb = self.pal.proj[usize::from(b)];
        let votes_a = projs
            .iter()
            .filter(|&&p| self.pal.metric.distance_sq(p, pa) <= self.pal.metric.distance_sq(p, pb))
            .count();
        if votes_a * 2 >= projs.len() {
            (a, b)
        } else {
            (b, a)
        }
    }

    /// Exhaustive Spectrum attribute search for one 8×8 cell (64
    /// metric-projected pixels, row-major within the cell).
    ///
    /// Enumeration order (stable, documented per the contract): the normal
    /// brightness state first, then bright. Within a state, all unordered
    /// pairs from that state's 8-colour candidate list in lexicographic
    /// position order, singles included. Candidate lists in global palette
    /// indices: normal `[0..=7]`, bright `[0, 9..=15]` (black is shared —
    /// index 0 represents it in both states, the lowest-index rule).
    /// Strict `<` selection: the first candidate in this order wins ties.
    /// PAPER takes the cell's majority colour (ties to the lower index);
    /// INK the other. Returns the winning choice and its summed cell error.
    #[must_use]
    pub fn spectrum(&self, projs: &[[f32; 3]]) -> (SpectrumChoice, f32) {
        const NORMAL: [u8; 8] = [0, 1, 2, 3, 4, 5, 6, 7];
        const BRIGHT: [u8; 8] = [0, 9, 10, 11, 12, 13, 14, 15];

        let mut best = (0u8, 0u8, false);
        let mut best_score = f32::INFINITY;
        for (bright, set) in [(false, NORMAL), (true, BRIGHT)] {
            if let Some((a, b)) = self.best_pair(projs, &set, &mut best_score) {
                best = (a, b, bright);
            }
        }
        let (a, b, bright) = best;
        let (paper, ink) = self.majority_roles(projs, a, b);
        (SpectrumChoice { ink, paper, bright }, best_score)
    }

    /// Exhaustive C64 hires search for one 8×8 cell (64 metric-projected
    /// pixels): all 136 unordered pairs (singles included) from the
    /// 16-colour palette, in lexicographic `(i, j)` order, mixing-aware,
    /// strict `<` selection (first candidate wins ties). The clear-bit
    /// colour (`bg`) takes the cell's majority colour. Returns the winning
    /// choice and its summed cell error.
    #[must_use]
    pub fn c64_hires(&self, projs: &[[f32; 3]]) -> (HiresChoice, f32) {
        let n = u8::try_from(self.pal.len()).unwrap_or(u8::MAX);
        let all: Vec<u8> = (0..n).collect();
        let mut best_score = f32::INFINITY;
        let (a, b) = self
            .best_pair(projs, &all, &mut best_score)
            .unwrap_or((0, 0));
        let (bg, fg) = self.majority_roles(projs, a, b);
        (HiresChoice { fg, bg }, best_score)
    }

    /// C64 multicolour search for one 4×8 cell (32 double-wide
    /// metric-projected pixels, row-major, with each pixel's nearest
    /// palette index alongside) given the global `background`.
    ///
    /// Deterministic pruning (documented per the plan, "single-colour
    /// frequency/error fit"): rank the 15 non-background colours by, in
    /// order, (1) **frequency** — how many cell pixels have this colour as
    /// their nearest palette entry (descending; guarantees every exactly
    /// used colour outranks unused ones), (2) **pair-fit cost** — for
    /// colour `c`, `Σ_pixels min(d(p, c), d(p, background))`, i.e. how well
    /// `{bg, c}` alone would cover the cell (ascending, `f32::total_cmp`),
    /// (3) lower palette index. Take the top [`MULTI_PRUNE_K`] and search
    /// all C(K, 3) free triples in lexicographic ranked-position order,
    /// each scored mixing-aware over the 4-colour set `{bg} ∪ triple` (all
    /// 10 unordered pairs, singles included), strict `<` selection.
    /// Returns the winning choice (free colours sorted ascending by
    /// palette index into the %01/%10/%11 slots) and its summed cell
    /// error.
    #[must_use]
    pub fn c64_multi(
        &self,
        projs: &[[f32; 3]],
        nearest: &[u8],
        background: u8,
    ) -> (MultiChoice, f32) {
        let n = self.pal.len();
        let bg_dist: Vec<f32> = projs
            .iter()
            .map(|&p| {
                self.pal
                    .metric
                    .distance_sq(p, self.pal.proj[usize::from(background)])
            })
            .collect();

        // Per-colour nearest-pixel frequency (lowest index wins a
        // nearest-distance tie, via the precomputed nearest-index map).
        let mut freq = vec![0usize; n];
        for &c in nearest {
            freq[usize::from(c)] += 1;
        }

        // Rank non-background colours: frequency desc, fit cost asc, index.
        let mut ranked: Vec<(usize, f32, u8)> = (0..n)
            .filter_map(|c| {
                let ci = u8::try_from(c).ok()?;
                if ci == background {
                    return None;
                }
                let pc = self.pal.proj[c];
                let cost: f32 = projs
                    .iter()
                    .zip(&bg_dist)
                    .map(|(&p, &db)| {
                        let dc = self.pal.metric.distance_sq(p, pc);
                        if dc < db { dc } else { db }
                    })
                    .sum();
                Some((freq[c], cost, ci))
            })
            .collect();
        ranked.sort_unstable_by(|a, b| b.0.cmp(&a.0).then(a.1.total_cmp(&b.1)).then(a.2.cmp(&b.2)));
        let cands: Vec<u8> = ranked
            .iter()
            .take(MULTI_PRUNE_K)
            .map(|&(_, _, c)| c)
            .collect();

        // Cache per-pixel mixing-aware error for every pair among
        // {background} ∪ candidates (positions: 0 = background).
        let involved: Vec<u8> = std::iter::once(background)
            .chain(cands.iter().copied())
            .collect();
        let m = involved.len();
        let mut cache: Vec<Vec<f32>> = Vec::with_capacity(m * (m + 1) / 2);
        for p in 0..m {
            for q in p..m {
                let a = involved[p].min(involved[q]);
                let b = involved[p].max(involved[q]);
                let row = self.mix_row(a, b);
                cache.push(
                    projs
                        .iter()
                        .map(|&px| min_dist_sq(self.pal.metric, px, row))
                        .collect(),
                );
            }
        }

        // Enumerate triples in lexicographic ranked-position order.
        let k = cands.len();
        let mut best_triple = [0usize; 3];
        let mut best_score = f32::INFINITY;
        let mut found = false;
        for x in 0..k {
            for y in (x + 1)..k {
                for z in (y + 1)..k {
                    let positions = [0, x + 1, y + 1, z + 1];
                    // 4 positions ⇒ exactly 10 unordered pairs.
                    let mut pair_rows: [&[f32]; 10] = [&[]; 10];
                    let mut slot = 0;
                    for (pi, &p) in positions.iter().enumerate() {
                        for &q in &positions[pi..] {
                            pair_rows[slot] = &cache[tri_index(m, p, q)];
                            slot += 1;
                        }
                    }
                    let mut sum = 0.0f32;
                    let mut aborted = false;
                    for px in 0..projs.len() {
                        let mut best_px = f32::INFINITY;
                        for row in &pair_rows {
                            let d = row[px];
                            if d < best_px {
                                best_px = d;
                            }
                        }
                        sum += best_px;
                        if sum > best_score {
                            aborted = true;
                            break;
                        }
                    }
                    if !aborted && sum < best_score {
                        best_score = sum;
                        best_triple = [x, y, z];
                        found = true;
                    }
                }
            }
        }

        // Fewer than three candidates (tiny palettes only): pad the free
        // slots with the background.
        let mut free: Vec<u8> = if found {
            best_triple.iter().map(|&i| cands[i]).collect()
        } else {
            let mut f = cands.clone();
            while f.len() < 3 {
                f.push(background);
            }
            // Score the padded set honestly: it degenerates to {bg} ∪ cands,
            // i.e. the per-pixel minimum over every cached pair row.
            let mut per_pixel = vec![f32::INFINITY; projs.len()];
            for row in &cache {
                for (slot, &d) in per_pixel.iter_mut().zip(row) {
                    if d < *slot {
                        *slot = d;
                    }
                }
            }
            best_score = per_pixel.iter().sum();
            f
        };
        free.sort_unstable();
        (
            MultiChoice {
                colours: [background, free[0], free[1], free[2]],
            },
            best_score,
        )
    }

    /// The best ordered-dither mix of `allowed` colours for one pixel:
    /// returns `(lo, hi, k)` — palette indices with `lo <= hi` and the mix
    /// level `k` (fraction of `hi` = `k / MIX_LEVELS`) minimising the metric
    /// distance. `allowed` must be sorted ascending. Enumeration: pairs in
    /// lexicographic position order, `k` ascending; strict `<` keeps the
    /// first candidate on ties (lowest pair, then lowest `k`).
    #[must_use]
    pub fn best_mix(&self, pixel_proj: [f32; 3], allowed: &[u8]) -> (u8, u8, usize) {
        let mut best = (allowed[0], allowed[0], 0usize);
        let mut best_d = f32::INFINITY;
        for (ai, &a) in allowed.iter().enumerate() {
            for &b in &allowed[ai..] {
                let row = self.mix_row(a, b);
                for (k, &m) in row.iter().enumerate() {
                    let d = self.pal.metric.distance_sq(pixel_proj, m);
                    if d < best_d {
                        best_d = d;
                        best = (a, b, k);
                    }
                }
            }
        }
        best
    }
}

/// Choose the global C64 multicolour background by the deterministic
/// histogram heuristic over a precomputed per-pixel nearest-palette-index
/// map (each entry already the lowest index on equal distance, via
/// [`PaletteData::nearest`]): count, and return the most frequent index —
/// lowest index on equal counts.
#[must_use]
pub fn choose_background(nearest: &[u8], n_colours: usize) -> u8 {
    let mut counts = vec![0usize; n_colours];
    for &c in nearest {
        counts[usize::from(c)] += 1;
    }
    let mut best = 0usize;
    let mut best_count = 0usize;
    for (i, &count) in counts.iter().enumerate() {
        if count > best_count {
            best_count = count;
            best = i;
        }
    }
    u8::try_from(best).unwrap_or(u8::MAX)
}
