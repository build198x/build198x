//! Integration tests for `build198x basic`: a listing in, the file its machine
//! loads out, checked by decoding the output with the container crate.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use format198x_sinclair_zx_spectrum_tap::{Header, HeaderKind, decode};

static DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A unique, self-cleaning temp directory.
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "build198x-basic-{tag}-{}-{}",
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

/// Run the binary in `dir`, returning (exit code, stdout, stderr).
fn run_in(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_build198x"))
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run build198x binary");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// The program header of a Spectrum tape: its first block.
fn tap_header(path: &Path) -> Header {
    let tap = std::fs::read(path).expect("read tap");
    let blocks = decode(&tap).expect("decode tap");
    assert!(blocks[0].is_header(), "first block is a header");
    Header::from_payload(&blocks[0].data).expect("parse header")
}

#[test]
fn spectrum_listing_builds_an_autorunning_tap() {
    let dir = TempDir::new("zx");
    std::fs::write(
        dir.path().join("hello.bas"),
        "  10 PRINT \"HELLO\"\n  20 GO TO 10\n",
    )
    .expect("write listing");
    let (code, _, err) = run_in(
        dir.path(),
        &[
            "basic",
            "hello.bas",
            "--machine",
            "sinclair-zx-spectrum",
            "-o",
            "hello.tap",
        ],
    );
    assert_eq!(code, 0, "{err}");
    let tap = std::fs::read(dir.path().join("hello.tap")).expect("read tap");
    let blocks = decode(&tap).expect("decode tap");
    assert_eq!(blocks.len(), 2);
    let header = Header::from_payload(&blocks[0].data).expect("parse header");
    assert_eq!(header.kind, HeaderKind::Program);
    assert_eq!(header.name, "hello"); // from the -o stem
    assert_eq!(header.param1, 10); // autorun line
    // Length and program length (no variables) both name the data block.
    let program_length = u16::try_from(blocks[1].data.len()).expect("fits u16");
    assert_eq!(header.length, program_length);
    assert_eq!(header.param2, program_length);
    assert!(!blocks[1].is_header());
}

#[test]
fn c64_listing_builds_a_prg_at_0801() {
    let dir = TempDir::new("c64");
    std::fs::write(dir.path().join("a.bas"), "10 PRINT CHR$(147)\n").expect("write listing");
    let (code, _, err) = run_in(
        dir.path(),
        &[
            "basic",
            "a.bas",
            "--machine",
            "commodore-c64",
            "-o",
            "a.prg",
        ],
    );
    assert_eq!(code, 0, "{err}");
    let prg = std::fs::read(dir.path().join("a.prg")).expect("read prg");
    assert_eq!(&prg[..2], &[0x01, 0x08]);
}

#[test]
fn wrong_extension_and_bad_lines_fail_and_write_nothing() {
    let dir = TempDir::new("err");
    std::fs::write(dir.path().join("a.bas"), "  10 PRINT 1\nPRINT 2\n").expect("write listing");
    let (code, _, err) = run_in(
        dir.path(),
        &[
            "basic",
            "a.bas",
            "--machine",
            "commodore-c64",
            "-o",
            "a.tap",
        ],
    );
    assert_ne!(code, 0);
    assert!(err.contains(".prg"), "{err}");
    let (code, _, err) = run_in(
        dir.path(),
        &[
            "basic",
            "a.bas",
            "--machine",
            "sinclair-zx-spectrum",
            "-o",
            "a.tap",
        ],
    );
    assert_ne!(code, 0);
    assert!(err.contains("a.bas:2:"), "{err}");
    assert!(!dir.path().join("a.tap").exists());
}

#[test]
fn no_autorun_writes_line_32768() {
    let dir = TempDir::new("noauto");
    std::fs::write(dir.path().join("a.bas"), "  10 STOP\n").expect("write listing");
    let (code, _, err) = run_in(
        dir.path(),
        &[
            "basic",
            "a.bas",
            "--machine",
            "sinclair-zx-spectrum",
            "-o",
            "a.tap",
            "--no-autorun",
        ],
    );
    assert_eq!(code, 0, "{err}");
    assert_eq!(tap_header(&dir.path().join("a.tap")).param1, 32768);
}

#[test]
fn name_defaults_to_ten_characters_of_the_stem_and_can_be_set() {
    let dir = TempDir::new("name");
    std::fs::write(dir.path().join("a.bas"), "  10 STOP\n").expect("write listing");
    let (code, _, err) = run_in(
        dir.path(),
        &[
            "basic",
            "a.bas",
            "--machine",
            "sinclair-zx-spectrum",
            "-o",
            "shadowkeep-unit-01.tap",
        ],
    );
    assert_eq!(code, 0, "{err}");
    assert_eq!(
        tap_header(&dir.path().join("shadowkeep-unit-01.tap")).name,
        "shadowkeep"
    );
    let (code, _, err) = run_in(
        dir.path(),
        &[
            "basic",
            "a.bas",
            "--machine",
            "sinclair-zx-spectrum",
            "-o",
            "b.tap",
            "--name",
            "Game",
        ],
    );
    assert_eq!(code, 0, "{err}");
    assert_eq!(tap_header(&dir.path().join("b.tap")).name, "Game");
}

#[test]
fn json_report_names_the_build() {
    let dir = TempDir::new("json");
    std::fs::write(dir.path().join("a.bas"), "10 PRINT 1\n\n20 GOTO 10\n").expect("write listing");
    let (code, out, err) = run_in(
        dir.path(),
        &[
            "basic",
            "a.bas",
            "--machine",
            "commodore-c64",
            "-o",
            "a.prg",
            "--format",
            "json",
        ],
    );
    assert_eq!(code, 0, "{err}");
    let prg_len = std::fs::read(dir.path().join("a.prg"))
        .expect("read prg")
        .len();
    let expected = format!(
        "{{\"tool_version\":\"{}\",\"machine\":\"commodore-c64\",\"input\":\"a.bas\",\"output\":\"a.prg\",\"lines\":2,\"program_length\":{},\"autorun\":null}}\n",
        env!("CARGO_PKG_VERSION"),
        prg_len - 2
    );
    assert_eq!(out, expected);
}

#[test]
fn unknown_machine_and_missing_output_are_usage_errors() {
    let dir = TempDir::new("usage");
    std::fs::write(dir.path().join("a.bas"), "10 STOP\n").expect("write listing");
    let (code, _, _) = run_in(
        dir.path(),
        &["basic", "a.bas", "--machine", "bbc-micro", "-o", "a.prg"],
    );
    assert_eq!(code, 2);
    let (code, _, _) = run_in(
        dir.path(),
        &["basic", "a.bas", "--machine", "commodore-c64"],
    );
    assert_eq!(code, 2);
}
