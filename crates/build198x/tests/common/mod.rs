//! Shared helpers for the codec integration tests: fixture paths, golden
//! byte-fixture comparison, and the wild-file discovery loop.

#![allow(dead_code)] // Each test target uses a subset of these helpers.

use std::path::PathBuf;

/// Root of the test fixture tree.
pub fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Assert `actual` matches the frozen golden fixture `name` byte-for-byte.
///
/// Run with `UPDATE_GOLDEN=1` to (re)write the fixture instead of comparing
/// — only for deliberate, reviewed regeneration.
pub fn assert_golden(name: &str, actual: &[u8]) {
    let path = fixtures().join("golden").join(name);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        let parent = path.parent().expect("golden fixture path has a parent");
        std::fs::create_dir_all(parent).expect("create golden fixture dir");
        std::fs::write(&path, actual).expect("write golden fixture");
        eprintln!("UPDATE_GOLDEN: wrote {}", path.display());
        return;
    }
    let expected = std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "missing golden fixture {}: {e}; run with UPDATE_GOLDEN=1 to create it",
            path.display()
        )
    });
    if expected != actual {
        let first_diff = expected
            .iter()
            .zip(actual.iter())
            .position(|(a, b)| a != b)
            .unwrap_or_else(|| expected.len().min(actual.len()));
        panic!(
            "golden mismatch for {name}: expected {} bytes, got {} bytes, first difference at offset {first_diff}",
            expected.len(),
            actual.len()
        );
    }
}

/// Decode every file in `tests/fixtures/wild/{subdir}/` with `decode`,
/// panicking on the first failure. An absent or empty directory passes with
/// a note — the curated TOSEC pulls are a tracked follow-up.
pub fn decode_wild_dir<E: std::fmt::Debug>(subdir: &str, decode: impl Fn(&[u8]) -> Result<(), E>) {
    let dir = fixtures().join("wild").join(subdir);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        eprintln!(
            "wild fixture dir {} absent; passing with a note (curated pulls are a tracked follow-up)",
            dir.display()
        );
        return;
    };
    let mut decoded = 0usize;
    for entry in entries {
        let path = entry.expect("read wild fixture dir entry").path();
        let hidden = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_none_or(|n| n.starts_with('.'));
        if hidden || !path.is_file() {
            continue;
        }
        let bytes = std::fs::read(&path).expect("read wild fixture");
        if let Err(e) = decode(&bytes) {
            panic!("wild fixture {} failed to decode: {e:?}", path.display());
        }
        decoded += 1;
    }
    if decoded == 0 {
        eprintln!(
            "no wild fixtures in {}; passing with a note (curated pulls are a tracked follow-up)",
            dir.display()
        );
    } else {
        eprintln!("decoded {decoded} wild fixture(s) from {}", dir.display());
    }
}
