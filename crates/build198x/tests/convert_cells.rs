//! Hand-computable constraint-search cell tests — written before the search
//! implementations, per the plan's test-first execution note. Each cell is
//! small enough to reason the correct answer by hand.

use build198x::convert::colour::{Metric, srgb8_to_linear};
use build198x::convert::constrain::{CellSearcher, PaletteData, choose_background};
mod common;
use common::palette_of;

fn searcher_for(machine: &str) -> CellSearcher {
    CellSearcher::new(PaletteData::new(palette_of(machine), Metric::OkLab))
}

/// A cell filled by cycling the given palette indices, metric-projected the
/// way the pipeline projects pixels before searching.
fn cell_of(searcher: &CellSearcher, indices: &[u8], len: usize) -> Vec<[f32; 3]> {
    (0..len)
        .map(|i| {
            let c = searcher.pal.srgb[usize::from(indices[i % indices.len()])];
            searcher
                .pal
                .metric
                .project(srgb8_to_linear([c.r, c.g, c.b]))
        })
        .collect()
}

/// Per-pixel nearest palette indices for a projected cell.
fn nearest_map(searcher: &CellSearcher, projs: &[[f32; 3]]) -> Vec<u8> {
    projs.iter().map(|&p| searcher.pal.nearest(p)).collect()
}

#[test]
fn spectrum_black_white_cell_is_exact() {
    let searcher = searcher_for("sinclair-zx-spectrum");
    // Half black (index 0), half normal white (index 7).
    let cell = cell_of(&searcher, &[0, 7], 64);

    let (choice, score) = searcher.spectrum(&cell);
    let mut pair = [choice.ink, choice.paper];
    pair.sort_unstable();
    assert_eq!(pair, [0, 7]);
    assert!(!choice.bright);
    assert!(score < 1e-12, "expected zero error, got {score}");
}

#[test]
fn spectrum_bright_pair_selects_bright_state() {
    let searcher = searcher_for("sinclair-zx-spectrum");
    // Bright red (10) and bright cyan (13) — distinct from their normal
    // counterparts, so zero error exists only in the bright state.
    let cell = cell_of(&searcher, &[10, 13], 64);

    let (choice, score) = searcher.spectrum(&cell);
    let mut pair = [choice.ink, choice.paper];
    pair.sort_unstable();
    assert_eq!(pair, [10, 13]);
    assert!(choice.bright);
    assert!(score < 1e-12, "expected zero error, got {score}");
}

#[test]
fn spectrum_black_is_allowed_in_a_bright_cell() {
    let searcher = searcher_for("sinclair-zx-spectrum");
    // Black + bright yellow (14): black is shared across the brightness
    // halves, so the bright state must be able to pair it with yellow.
    let cell = cell_of(&searcher, &[0, 14], 64);

    let (choice, score) = searcher.spectrum(&cell);
    let mut pair = [choice.ink, choice.paper];
    pair.sort_unstable();
    assert_eq!(pair, [0, 14]);
    assert!(choice.bright);
    assert!(score < 1e-12, "expected zero error, got {score}");
}

#[test]
fn c64_hires_two_colour_cell_is_exact() {
    let searcher = searcher_for("commodore-c64");
    // Black (0) and white (1).
    let cell = cell_of(&searcher, &[0, 1], 64);

    let (choice, score) = searcher.c64_hires(&cell);
    let mut pair = [choice.fg, choice.bg];
    pair.sort_unstable();
    assert_eq!(pair, [0, 1]);
    assert!(score < 1e-12, "expected zero error, got {score}");
}

#[test]
fn c64_multi_background_slot_is_honoured() {
    let searcher = searcher_for("commodore-c64");
    // A 4×8 multicolour cell using the background (blue, 6) plus white (1)
    // and red (2).
    let cell = cell_of(&searcher, &[6, 1, 2, 6], 32);

    let (choice, score) = searcher.c64_multi(&cell, &nearest_map(&searcher, &cell), 6);
    assert_eq!(choice.colours[0], 6, "background must sit in slot 0");
    assert!(choice.colours[1..].contains(&1));
    assert!(choice.colours[1..].contains(&2));
    assert!(score < 1e-12, "expected zero error, got {score}");
}

#[test]
fn c64_multi_cell_without_the_background_colour_still_renders() {
    let searcher = searcher_for("commodore-c64");
    // Background is black (0) but the cell uses cyan (3), purple (4),
    // green (5): the three free slots must cover it exactly.
    let cell = cell_of(&searcher, &[3, 4, 5, 3], 32);

    let (choice, score) = searcher.c64_multi(&cell, &nearest_map(&searcher, &cell), 0);
    assert_eq!(choice.colours[0], 0);
    let mut free = [choice.colours[1], choice.colours[2], choice.colours[3]];
    free.sort_unstable();
    assert_eq!(free, [3, 4, 5]);
    assert!(score < 1e-12, "expected zero error, got {score}");
}

#[test]
fn background_histogram_picks_most_frequent_colour() {
    let searcher = searcher_for("commodore-c64");
    // 6 blue pixels (index 6), 2 white (index 1).
    let projs = cell_of(&searcher, &[6, 6, 6, 1, 6, 6, 1, 6], 8);
    let nearest = nearest_map(&searcher, &projs);
    assert_eq!(choose_background(&nearest, searcher.pal.len()), 6);
}
