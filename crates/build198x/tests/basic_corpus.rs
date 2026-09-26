//! The C64 corpus check against VICE's petcat, and its ROM-authority policy.
//!
//! The ROM is the authority over tools: where build198x and petcat disagree
//! because build198x follows a documented ROM behaviour petcat does not
//! reproduce, that disagreement belongs in `ROM_JUSTIFIED_DIVERGENCES` below,
//! cited to the ROM routine, not read as petcat being right. For example the
//! C64 tokeniser stores `?` as the PRINT token (petcat does not); none of the
//! 86 samples currently use `?`, so the list starts empty.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use build198x::basic::{Machine, build};

static DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A unique, self-cleaning temp directory.
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "build198x-basic-corpus-{tag}-{}-{}",
            std::process::id(),
            DIR_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).expect("create temp dir");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Every `*.bas` file under `root`, walked recursively, in a stable order.
fn bas_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries {
            let path = entry.expect("read code-samples dir entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "bas") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// ROM-justified divergences from petcat: each entry is a source path
/// fragment that is allowed to disagree, with the reason and the ROM routine
/// that makes build198x's reading authoritative recorded here rather than in
/// the mismatch list. Empty: none of the 86 samples need one yet.
const ROM_JUSTIFIED_DIVERGENCES: &[(&str, &str)] = &[];

/// Every C64 listing in code-samples must tokenise byte-identically to VICE's
/// petcat. petcat reads uppercase as shifted graphics, so it gets the
/// listing lowercased. Needs CODE_SAMPLES_PATH and petcat on PATH.
#[test]
#[ignore = "needs CODE_SAMPLES_PATH and petcat"]
fn c64_listings_match_petcat() {
    let code_samples = std::env::var("CODE_SAMPLES_PATH").expect("CODE_SAMPLES_PATH must be set");
    let root = Path::new(&code_samples).join("commodore-64/basic");
    let files = bas_files(&root);
    assert!(
        !files.is_empty(),
        "no *.bas files found under {}",
        root.display()
    );

    let dir = TempDir::new("petcat");
    let mut mismatches = Vec::new();

    for path in &files {
        let display = path.display().to_string();
        if let Some((_, reason)) = ROM_JUSTIFIED_DIVERGENCES
            .iter()
            .find(|(fragment, _)| display.contains(fragment))
        {
            eprintln!("{display}: skipped, ROM-justified divergence ({reason})");
            continue;
        }

        let source = std::fs::read_to_string(path).expect("read listing");

        let built = match build(Machine::CommodoreC64, &source, "corpus", false) {
            Ok(built) => built,
            Err(e) => {
                mismatches.push(format!("{display}: build198x failed to tokenise: {e}"));
                continue;
            }
        };

        // petcat reads uppercase source text as shifted graphics rather than
        // plain letters, so the listing is lowercased first to read as the
        // ROM would read it typed on a real keyboard.
        let lowered_path = dir.path().join("lowered.bas");
        std::fs::write(&lowered_path, source.to_ascii_lowercase()).expect("write lowered listing");
        let out_path = dir.path().join("out.prg");
        let _ = std::fs::remove_file(&out_path);

        let output = Command::new("petcat")
            .arg("-w2")
            .arg("-o")
            .arg(&out_path)
            .arg("--")
            .arg(&lowered_path)
            .output()
            .expect("run petcat (is it on PATH?)");
        if !output.status.success() {
            mismatches.push(format!(
                "{display}: petcat failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
            continue;
        }

        let petcat_bytes = std::fs::read(&out_path).expect("read petcat output");
        if built.bytes != petcat_bytes {
            mismatches.push(format!(
                "{display}: build198x ({} bytes) and petcat ({} bytes) tokenise differently",
                built.bytes.len(),
                petcat_bytes.len()
            ));
        }
    }

    assert!(mismatches.is_empty(), "{mismatches:#?}");
}
