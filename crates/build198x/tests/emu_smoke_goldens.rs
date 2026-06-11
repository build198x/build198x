//! Enforcement tests for the Emu198x smoke fixtures: converting
//! `tests/fixtures/emu-smoke/source.png` with **exactly the regen script's
//! flags** (`scripts/regen-emu-fixtures.sh`) must reproduce the committed
//! `smoke.{scr,koa,art,iff}` byte-for-byte. The Emu198x harness loads those
//! bytes, so a silent drift here would feed the separate Emu198x session
//! unreproducible fixtures.
//!
//! The pipeline is driven through the library (fast — no binary spawn), but
//! flag parity with the script is the load-bearing property: the script
//! passes no conversion flags beyond `--mode lores-pal` for ILBM, so the
//! CLI defaults apply — and [`Options::new`] pins those same defaults
//! (pinned default interpretation, OKLab metric, 8×8 Bayer at strength 32,
//! black matte, heuristic background). The CLI encodes ILBM with ByteRun1;
//! so does this test. A failure is a determinism bug or an unexplained
//! pipeline change, not a regen chore (`decisions/determinism-contract.md`).

mod common;

use build198x::convert::pipeline::{Conversion, Options, convert};
use build198x::format::{art_studio, ilbm, koala, scr};

/// Convert the committed smoke source with the regen script's effective
/// options for `machine`/`mode` (all defaults — see the module docs).
fn convert_source(machine: &str, mode: &str) -> Conversion {
    let path = common::fixtures().join("emu-smoke").join("source.png");
    let img = image::open(&path).expect("open emu-smoke source.png");
    let opts = Options::new(machine, mode);
    convert(&img, &opts).expect("smoke conversion succeeds")
}

/// Assert `actual` equals the committed fixture byte-for-byte (without
/// dumping whole buffers on failure).
fn assert_matches_fixture(name: &str, actual: &[u8]) {
    let path = common::fixtures().join("emu-smoke").join(name);
    let expected = std::fs::read(&path).expect("read committed smoke fixture");
    let first_diff = expected
        .iter()
        .zip(actual.iter())
        .position(|(a, b)| a != b)
        .unwrap_or_else(|| expected.len().min(actual.len()));
    assert!(
        expected == actual,
        "{name} drifted from the committed fixture (expected {} bytes, got {}, first difference \
         at offset {first_diff}): determinism bug or unexplained pipeline change — goldens move \
         only on an explicit version bump (decisions/determinism-contract.md)",
        expected.len(),
        actual.len()
    );
}

#[test]
fn smoke_scr_matches_committed_fixture() {
    let conv = convert_source("sinclair-zx-spectrum", "standard");
    let bytes = scr::encode(&conv.to_scr().expect("scr bridge")).expect("scr encode");
    assert_matches_fixture("smoke.scr", &bytes);
}

#[test]
fn smoke_koala_matches_committed_fixture() {
    let conv = convert_source("commodore-c64", "multicolour-bitmap");
    let bytes = koala::encode(&conv.to_koala().expect("koala bridge")).expect("koala encode");
    assert_matches_fixture("smoke.koa", &bytes);
}

#[test]
fn smoke_art_studio_matches_committed_fixture() {
    let conv = convert_source("commodore-c64", "hires-bitmap");
    let bytes = art_studio::encode(&conv.to_art_studio().expect("art-studio bridge"))
        .expect("art-studio encode");
    assert_matches_fixture("smoke.art", &bytes);
}

#[test]
fn smoke_ilbm_matches_committed_fixture() {
    // The script passes `--mode lores-pal`; the CLI encodes with ByteRun1.
    let conv = convert_source("commodore-amiga-ocs", "lores-pal");
    let bytes = ilbm::encode(
        &conv.to_ilbm().expect("ilbm bridge"),
        ilbm::Compression::ByteRun1,
    )
    .expect("ilbm encode");
    assert_matches_fixture("smoke.iff", &bytes);
}
