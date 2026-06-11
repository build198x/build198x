//! Wild-file decode fixtures: every file under
//! `tests/fixtures/wild/{scr,koala,art-studio,ilbm}/` must decode without
//! error. Empty or absent directories pass with an `eprintln` note — the
//! curated TOSEC pulls are a tracked follow-up.

mod common;

use build198x::format::{art_studio, ilbm, koala, scr};

#[test]
fn wild_scr_files_decode() {
    common::decode_wild_dir("scr", |bytes| scr::decode(bytes).map(|_| ()));
}

#[test]
fn wild_koala_files_decode() {
    common::decode_wild_dir("koala", |bytes| koala::decode(bytes).map(|_| ()));
}

#[test]
fn wild_art_studio_files_decode() {
    common::decode_wild_dir("art-studio", |bytes| art_studio::decode(bytes).map(|_| ()));
}

#[test]
fn wild_ilbm_files_decode() {
    common::decode_wild_dir("ilbm", |bytes| ilbm::decode(bytes).map(|_| ()));
}
