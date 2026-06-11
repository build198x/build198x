//! Transcendental ban (`decisions/determinism-contract.md`): contracted
//! paths in `src/convert/` must not call libm transcendentals or fused
//! multiply-add. This is a crude source grep, by design — it scans each
//! module's non-test source (everything before the first `#[cfg(test)]`)
//! for call-shaped substrings. Test modules may use `powf`/`cbrt` freely to
//! validate the deterministic replacements.

use std::fs;
use std::path::Path;

/// Call-shaped substrings of the banned operations. The leading `.`/`(`
/// shapes avoid false positives: `.exp(` does not match `.expect(`, and
/// `.cbrt(` does not match the hand-rolled `cbrt_det(`.
const BANNED: &[&str] = &[
    ".powf(",
    ".powi(",
    ".cbrt(",
    ".sin(",
    ".cos(",
    ".tan(",
    ".exp(",
    ".ln(",
    ".log(",
    ".log2(",
    ".log10(",
    ".mul_add(",
];

#[test]
fn convert_modules_use_no_transcendentals_outside_tests() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/convert");
    let mut checked = 0usize;
    let entries = fs::read_dir(&dir).expect("src/convert exists");
    for entry in entries {
        let path = entry.expect("readable dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let source = fs::read_to_string(&path).expect("readable source");
        // Contracted code is everything before the first test module.
        let contracted = source
            .split("#[cfg(test)]")
            .next()
            .expect("split always yields one piece");
        for banned in BANNED {
            assert!(
                !contracted.contains(banned),
                "{} contains banned operation {banned:?} outside #[cfg(test)]",
                path.display()
            );
        }
        checked += 1;
    }
    assert!(
        checked >= 7,
        "expected to scan the convert modules, saw {checked}"
    );
}
