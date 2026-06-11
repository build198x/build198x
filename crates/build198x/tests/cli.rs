//! Integration tests driving the `build198x` binary end to end: exit codes,
//! no-clobber + atomic writes, batch behaviour, the JSON report (shape,
//! golden, escaping), and one conversion smoke per format.
//!
//! Each test gets its own unique temp dir under `std::env::temp_dir()`
//! (cleaned on drop) and runs the binary with that dir as cwd, so reports
//! carry relative paths and tests never collide.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use image::{Rgb, RgbImage, Rgba, RgbaImage};

static DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A unique, self-cleaning temp directory.
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "build198x-cli-{tag}-{}-{}",
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

/// Write a deterministic gradient PNG.
fn write_gradient_png(path: &Path, w: u32, h: u32) {
    let img = RgbImage::from_fn(w, h, |x, y| {
        Rgb([
            u8::try_from(x * 255 / w.max(1)).unwrap_or(255),
            u8::try_from(y * 255 / h.max(1)).unwrap_or(255),
            u8::try_from((x + y) % 256).unwrap_or(0),
        ])
    });
    img.save(path).expect("write gradient png");
}

/// Write a two-frame animated GIF.
fn write_two_frame_gif(path: &Path) {
    let f1 = RgbaImage::from_pixel(64, 48, Rgba([200, 30, 30, 255]));
    let f2 = RgbaImage::from_pixel(64, 48, Rgba([30, 200, 30, 255]));
    let file = std::fs::File::create(path).expect("create gif");
    let mut encoder = image::codecs::gif::GifEncoder::new(file);
    encoder
        .encode_frames(vec![image::Frame::new(f1), image::Frame::new(f2)])
        .expect("encode animated gif");
}

// --- a tiny recursive-descent JSON validator -------------------------------

/// Assert `s` is one complete, valid JSON value. Minimal recursive-descent
/// parser — structure validation only, no value model.
fn assert_valid_json(s: &str) {
    let bytes = s.as_bytes();
    let mut pos = 0usize;
    skip_ws(bytes, &mut pos);
    parse_value(bytes, &mut pos);
    skip_ws(bytes, &mut pos);
    assert_eq!(pos, bytes.len(), "trailing bytes after the JSON value");
}

fn skip_ws(b: &[u8], pos: &mut usize) {
    while *pos < b.len() && matches!(b[*pos], b' ' | b'\t' | b'\n' | b'\r') {
        *pos += 1;
    }
}

fn parse_value(b: &[u8], pos: &mut usize) {
    skip_ws(b, pos);
    match b.get(*pos) {
        Some(b'{') => parse_object(b, pos),
        Some(b'[') => parse_array(b, pos),
        Some(b'"') => parse_string(b, pos),
        Some(b't') => parse_literal(b, pos, b"true"),
        Some(b'f') => parse_literal(b, pos, b"false"),
        Some(b'n') => parse_literal(b, pos, b"null"),
        Some(c) if c.is_ascii_digit() || *c == b'-' => parse_number(b, pos),
        other => panic!("unexpected JSON byte {other:?} at offset {pos}"),
    }
}

fn parse_object(b: &[u8], pos: &mut usize) {
    *pos += 1; // {
    skip_ws(b, pos);
    if b.get(*pos) == Some(&b'}') {
        *pos += 1;
        return;
    }
    loop {
        skip_ws(b, pos);
        assert_eq!(b.get(*pos), Some(&b'"'), "object key must be a string");
        parse_string(b, pos);
        skip_ws(b, pos);
        assert_eq!(b.get(*pos), Some(&b':'), "missing `:` in object");
        *pos += 1;
        parse_value(b, pos);
        skip_ws(b, pos);
        match b.get(*pos) {
            Some(b',') => *pos += 1,
            Some(b'}') => {
                *pos += 1;
                return;
            }
            other => panic!("expected `,` or `}}` in object, got {other:?}"),
        }
    }
}

fn parse_array(b: &[u8], pos: &mut usize) {
    *pos += 1; // [
    skip_ws(b, pos);
    if b.get(*pos) == Some(&b']') {
        *pos += 1;
        return;
    }
    loop {
        parse_value(b, pos);
        skip_ws(b, pos);
        match b.get(*pos) {
            Some(b',') => *pos += 1,
            Some(b']') => {
                *pos += 1;
                return;
            }
            other => panic!("expected `,` or `]` in array, got {other:?}"),
        }
    }
}

fn parse_string(b: &[u8], pos: &mut usize) {
    *pos += 1; // opening quote
    while let Some(&c) = b.get(*pos) {
        match c {
            b'"' => {
                *pos += 1;
                return;
            }
            b'\\' => {
                *pos += 1;
                match b.get(*pos) {
                    Some(b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't') => *pos += 1,
                    Some(b'u') => {
                        for k in 1..=4 {
                            assert!(
                                b.get(*pos + k).is_some_and(u8::is_ascii_hexdigit),
                                "bad \\u escape"
                            );
                        }
                        *pos += 5;
                    }
                    other => panic!("bad escape {other:?}"),
                }
            }
            0x00..=0x1f => panic!("raw control byte {c:#04x} inside JSON string"),
            _ => *pos += 1,
        }
    }
    panic!("unterminated JSON string");
}

fn parse_number(b: &[u8], pos: &mut usize) {
    let start = *pos;
    while b
        .get(*pos)
        .is_some_and(|c| c.is_ascii_digit() || matches!(c, b'-' | b'+' | b'.' | b'e' | b'E'))
    {
        *pos += 1;
    }
    assert!(*pos > start, "empty number");
}

fn parse_literal(b: &[u8], pos: &mut usize, lit: &[u8]) {
    assert!(
        b[*pos..].starts_with(lit),
        "bad literal at offset {pos}, expected {}",
        String::from_utf8_lossy(lit)
    );
    *pos += lit.len();
}

// --- AE1: no-clobber + --force ---------------------------------------------

#[test]
fn ae1_existing_output_refused_without_force_and_replaced_with_force() {
    let td = TempDir::new("ae1");
    write_gradient_png(&td.path().join("in.png"), 256, 192);
    let sentinel = b"sentinel: do not overwrite".to_vec();
    std::fs::write(td.path().join("out.scr"), &sentinel).expect("write sentinel");

    let (code, stdout, _) = run_in(
        td.path(),
        &[
            "image",
            "in.png",
            "--machine",
            "sinclair-zx-spectrum",
            "--format",
            "scr",
            "-o",
            "out.scr",
        ],
    );
    assert_eq!(code, 5, "no-clobber refusal is exit 5");
    assert_eq!(
        std::fs::read(td.path().join("out.scr")).expect("read sentinel back"),
        sentinel,
        "the existing file must be untouched"
    );
    assert_valid_json(&stdout);
    assert!(
        stdout.contains("out.scr") && stdout.contains("exists"),
        "report names the conflict: {stdout}"
    );
    assert!(stdout.contains("\"kind\": \"io\""));

    let (code, stdout, _) = run_in(
        td.path(),
        &[
            "image",
            "in.png",
            "--machine",
            "sinclair-zx-spectrum",
            "--format",
            "scr",
            "-o",
            "out.scr",
            "--force",
        ],
    );
    assert_eq!(code, 0, "--force overwrites: {stdout}");
    let replaced = std::fs::read(td.path().join("out.scr")).expect("read replaced output");
    assert_eq!(replaced.len(), build198x::format::scr::FILE_LEN);
    assert_ne!(replaced, sentinel);
}

// --- AE2: animated GIF -------------------------------------------------------

#[test]
fn ae2_animated_gif_converts_first_frame_with_warning() {
    let td = TempDir::new("ae2");
    write_two_frame_gif(&td.path().join("anim.gif"));

    let (code, stdout, _) = run_in(
        td.path(),
        &[
            "image",
            "anim.gif",
            "--machine",
            "sinclair-zx-spectrum",
            "--format",
            "scr",
        ],
    );
    assert_eq!(code, 0, "animated gif converts: {stdout}");
    assert_valid_json(&stdout);
    assert!(
        stdout.contains("animated input: first frame used"),
        "warning present in report: {stdout}"
    );
    assert!(td.path().join("anim.scr").exists(), "output written");
}

// --- AE3: batch continues past errors ---------------------------------------

#[test]
fn ae3_batch_of_three_with_one_bad_input_is_partial_failure() {
    let td = TempDir::new("ae3");
    write_gradient_png(&td.path().join("a.png"), 320, 200);
    std::fs::write(td.path().join("b.png"), []).expect("write zero-byte file");
    write_gradient_png(&td.path().join("c.png"), 320, 200);

    let (code, stdout, _) = run_in(
        td.path(),
        &[
            "image",
            "a.png",
            "b.png",
            "c.png",
            "--machine",
            "commodore-c64",
            "--format",
            "koala",
        ],
    );
    assert_eq!(code, 6, "mixed batch is exit 6: {stdout}");
    assert_valid_json(&stdout);
    assert!(td.path().join("a.koa").exists());
    assert!(!td.path().join("b.koa").exists());
    assert!(td.path().join("c.koa").exists());
    assert_eq!(stdout.matches("\"status\": \"ok\"").count(), 2);
    assert_eq!(stdout.matches("\"status\": \"error\"").count(), 1);
    assert!(stdout.contains("\"ok\": 2"), "summary ok count: {stdout}");
    assert!(
        stdout.contains("\"failed\": 1"),
        "summary failed count: {stdout}"
    );
    // Statuses appear in input order: ok, error, ok.
    let first_error = stdout.find("\"status\": \"error\"").expect("error status");
    let first_ok = stdout.find("\"status\": \"ok\"").expect("ok status");
    let last_ok = stdout.rfind("\"status\": \"ok\"").expect("ok status");
    assert!(first_ok < first_error && first_error < last_ok);
}

// --- AE4: determinism (in-process leg) ---------------------------------------

#[test]
fn ae4_same_input_and_flags_twice_yield_identical_outputs() {
    let td1 = TempDir::new("ae4a");
    let td2 = TempDir::new("ae4b");
    for td in [&td1, &td2] {
        write_gradient_png(&td.path().join("in.png"), 320, 200);
        let (code, _, stderr) = run_in(
            td.path(),
            &[
                "image",
                "in.png",
                "--machine",
                "commodore-c64",
                "--format",
                "koala",
                "--preview",
                "prev.png",
            ],
        );
        assert_eq!(code, 0, "conversion succeeds: {stderr}");
    }
    let native1 = std::fs::read(td1.path().join("in.koa")).expect("read first output");
    let native2 = std::fs::read(td2.path().join("in.koa")).expect("read second output");
    assert_eq!(native1, native2, "native outputs must be byte-identical");
    // Preview PNGs compare by decoded pixels per the determinism contract.
    let prev1 = image::open(td1.path().join("prev.png")).expect("open preview 1");
    let prev2 = image::open(td2.path().join("prev.png")).expect("open preview 2");
    assert_eq!(prev1.to_rgb8().into_raw(), prev2.to_rgb8().into_raw());
}

// --- usage and decode failures ------------------------------------------------

#[test]
fn wrong_machine_format_pairing_is_a_usage_error() {
    let td = TempDir::new("pairing");
    write_gradient_png(&td.path().join("in.png"), 64, 64);
    let (code, stdout, stderr) = run_in(
        td.path(),
        &[
            "image",
            "in.png",
            "--machine",
            "commodore-c64",
            "--format",
            "scr",
        ],
    );
    assert_eq!(code, 2);
    assert!(stdout.is_empty(), "no report on usage errors: {stdout}");
    assert!(stderr.contains("usage:"), "usage on stderr: {stderr}");
    assert!(!td.path().join("in.scr").exists(), "nothing written");
}

#[test]
fn unknown_flag_and_unknown_subcommand_exit_2_with_usage_on_stderr() {
    let td = TempDir::new("unknown");
    let (code, stdout, stderr) = run_in(td.path(), &["image", "x.png", "--bogus"]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("--bogus") && stderr.contains("usage:"));

    let (code, stdout, stderr) = run_in(td.path(), &["bogus-subcommand"]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("usage:"));
}

#[test]
fn version_and_help_report_on_stdout() {
    let td = TempDir::new("version");
    let (code, stdout, _) = run_in(td.path(), &["--version"]);
    assert_eq!(code, 0);
    assert_eq!(
        stdout.trim(),
        format!("build198x {}", env!("CARGO_PKG_VERSION"))
    );

    let (code, stdout, _) = run_in(td.path(), &["image", "--help"]);
    assert_eq!(code, 0);
    for needle in ["exit codes", "--machine", "--format", "--force", "--report"] {
        assert!(stdout.contains(needle), "help mentions {needle}: {stdout}");
    }
}

#[test]
fn zero_byte_single_input_is_a_decode_failure() {
    let td = TempDir::new("zerobyte");
    std::fs::write(td.path().join("empty.png"), []).expect("write zero-byte file");
    let (code, stdout, _) = run_in(
        td.path(),
        &[
            "image",
            "empty.png",
            "--machine",
            "sinclair-zx-spectrum",
            "--format",
            "scr",
        ],
    );
    assert_eq!(code, 3, "all-inputs decode failure is exit 3: {stdout}");
    assert_valid_json(&stdout);
    assert!(stdout.contains("\"kind\": \"decode\""));
}

// --- output IO failures ----------------------------------------------------

#[test]
fn unwritable_output_directory_is_an_io_failure() {
    let td = TempDir::new("unwritable");
    write_gradient_png(&td.path().join("in.png"), 256, 192);
    std::fs::write(td.path().join("blocker"), b"a file, not a dir").expect("write blocker");

    let (code, stdout, _) = run_in(
        td.path(),
        &[
            "image",
            "in.png",
            "--machine",
            "sinclair-zx-spectrum",
            "--format",
            "scr",
            "-o",
            "blocker/out.scr",
        ],
    );
    assert_eq!(code, 5, "unwritable output dir is exit 5: {stdout}");
    assert_valid_json(&stdout);
    assert!(stdout.contains("\"kind\": \"io\""));
}

#[test]
fn unwritable_preview_with_successful_native_output_is_a_partial_io_failure() {
    let td = TempDir::new("previewsplit");
    write_gradient_png(&td.path().join("in.png"), 256, 192);
    std::fs::write(td.path().join("blocker"), b"a file, not a dir").expect("write blocker");

    let (code, stdout, _) = run_in(
        td.path(),
        &[
            "image",
            "in.png",
            "--machine",
            "sinclair-zx-spectrum",
            "--format",
            "scr",
            "-o",
            "out.scr",
            "--preview",
            "blocker/prev.png",
        ],
    );
    assert_eq!(code, 5, "preview IO failure is exit 5: {stdout}");
    assert_valid_json(&stdout);
    // The split is visible per file: the native output is listed as written,
    // the entry still carries the IO error.
    assert!(stdout.contains("\"outputs\": [\"out.scr\"]"), "{stdout}");
    assert!(stdout.contains("\"status\": \"error\""));
    assert!(stdout.contains("\"kind\": \"io\""));
    let native = std::fs::read(td.path().join("out.scr")).expect("native output intact");
    assert_eq!(native.len(), build198x::format::scr::FILE_LEN);
}

// --- report shape -------------------------------------------------------------

/// Normalise the volatile report fields (versions, mean_error) so the
/// golden pins structure, key order, and static values only. Paths are
/// already relative (the test runs the binary with cwd = temp dir).
fn normalise_report(report: &str) -> String {
    let mut out = String::with_capacity(report.len());
    for line in report.lines() {
        let trimmed = line.trim_start();
        let indent = &line[..line.len() - trimmed.len()];
        if trimmed.starts_with("\"converter_version\"") {
            out.push_str(&format!("{indent}\"converter_version\": \"NORM\",\n"));
        } else if trimmed.starts_with("\"mediaspec_version\"") {
            out.push_str(&format!("{indent}\"mediaspec_version\": \"NORM\",\n"));
        } else if trimmed.starts_with("\"mean_error\"") {
            out.push_str(&format!("{indent}\"mean_error\": 0.0,\n"));
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

#[test]
fn report_golden_matches_committed_fixture() {
    let td = TempDir::new("golden");
    // A fixed synthetic input: 4 flat vertical bands, fully deterministic.
    let img = RgbImage::from_fn(320, 200, |x, _| match x / 80 {
        0 => Rgb([0, 0, 0]),
        1 => Rgb([200, 40, 40]),
        2 => Rgb([40, 200, 40]),
        _ => Rgb([240, 240, 240]),
    });
    img.save(td.path().join("bands.png"))
        .expect("write bands png");

    let (code, _, stderr) = run_in(
        td.path(),
        &[
            "image",
            "bands.png",
            "--machine",
            "commodore-c64",
            "--format",
            "koala",
            "--report",
            "report.json",
        ],
    );
    assert_eq!(code, 0, "golden conversion succeeds: {stderr}");
    let raw = std::fs::read_to_string(td.path().join("report.json")).expect("read report file");
    assert_valid_json(&raw);
    common::assert_golden("cli-report.json", normalise_report(&raw).as_bytes());
}

#[cfg(unix)]
#[test]
fn report_escapes_quotes_and_backslashes_in_filenames() {
    let td = TempDir::new("escaping");
    let weird = "we\"ird\\name.png";
    write_gradient_png(&td.path().join(weird), 256, 192);

    let (code, stdout, _) = run_in(
        td.path(),
        &[
            "image",
            weird,
            "--machine",
            "sinclair-zx-spectrum",
            "--format",
            "scr",
            "-o",
            "out.scr",
        ],
    );
    assert_eq!(code, 0, "weird filename converts: {stdout}");
    assert_valid_json(&stdout);
    assert!(
        stdout.contains(r#""input": "we\"ird\\name.png""#),
        "escaped filename round-trips: {stdout}"
    );
}

// --- per-format conversion smoke ------------------------------------------------

#[test]
fn smoke_scr_output_decodes_and_preview_is_valid_png() {
    let td = TempDir::new("smoke-scr");
    write_gradient_png(&td.path().join("in.png"), 256, 192);
    let (code, stdout, _) = run_in(
        td.path(),
        &[
            "image",
            "in.png",
            "--machine",
            "sinclair-zx-spectrum",
            "--format",
            "scr",
            "--preview",
            "prev.png",
        ],
    );
    assert_eq!(code, 0, "{stdout}");
    let bytes = std::fs::read(td.path().join("in.scr")).expect("read scr");
    build198x::format::scr::decode(&bytes).expect("scr output decodes");
    let preview = image::open(td.path().join("prev.png")).expect("preview is a valid png");
    assert_eq!((preview.width(), preview.height()), (256, 192));
}

#[test]
fn smoke_koala_output_decodes_and_preview_is_valid_png() {
    let td = TempDir::new("smoke-koala");
    write_gradient_png(&td.path().join("in.png"), 320, 200);
    let (code, stdout, _) = run_in(
        td.path(),
        &[
            "image",
            "in.png",
            "--machine",
            "commodore-c64",
            "--format",
            "koala",
            "--preview",
            "prev.png",
        ],
    );
    assert_eq!(code, 0, "{stdout}");
    let bytes = std::fs::read(td.path().join("in.koa")).expect("read koala");
    build198x::format::koala::decode(&bytes).expect("koala output decodes");
    let preview = image::open(td.path().join("prev.png")).expect("preview is a valid png");
    assert_eq!((preview.width(), preview.height()), (160, 200));
}

#[test]
fn smoke_art_studio_output_decodes_and_preview_is_valid_png() {
    let td = TempDir::new("smoke-art");
    write_gradient_png(&td.path().join("in.png"), 320, 200);
    let (code, stdout, _) = run_in(
        td.path(),
        &[
            "image",
            "in.png",
            "--machine",
            "commodore-c64",
            "--format",
            "art-studio",
            "--preview",
            "prev.png",
        ],
    );
    assert_eq!(code, 0, "{stdout}");
    let bytes = std::fs::read(td.path().join("in.art")).expect("read art studio");
    build198x::format::art_studio::decode(&bytes).expect("art studio output decodes");
    let preview = image::open(td.path().join("prev.png")).expect("preview is a valid png");
    assert_eq!((preview.width(), preview.height()), (320, 200));
}

#[test]
fn smoke_ilbm_output_decodes_and_preview_is_valid_png() {
    let td = TempDir::new("smoke-ilbm");
    write_gradient_png(&td.path().join("in.png"), 320, 256);
    let (code, stdout, _) = run_in(
        td.path(),
        &[
            "image",
            "in.png",
            "--machine",
            "commodore-amiga-ocs",
            "--format",
            "ilbm",
            "--preview",
            "prev.png",
        ],
    );
    assert_eq!(code, 0, "{stdout}");
    assert!(
        stdout.contains("\"mode\": \"lores-pal\""),
        "default ilbm mode"
    );
    assert!(stdout.contains("\"kind\": \"generated\""));
    assert!(stdout.contains("\"gamut_bits\": 4"));
    let bytes = std::fs::read(td.path().join("in.iff")).expect("read ilbm");
    let decoded = build198x::format::ilbm::decode(&bytes).expect("ilbm output decodes");
    assert_eq!((decoded.width, decoded.height), (320, 256));
    let preview = image::open(td.path().join("prev.png")).expect("preview is a valid png");
    assert_eq!((preview.width(), preview.height()), (320, 256));
}

#[test]
fn ilbm_rejects_palette_flag_and_accepts_explicit_mode() {
    let td = TempDir::new("ilbm-flags");
    write_gradient_png(&td.path().join("in.png"), 640, 256);

    let (code, stdout, stderr) = run_in(
        td.path(),
        &[
            "image",
            "in.png",
            "--machine",
            "commodore-amiga-ocs",
            "--format",
            "ilbm",
            "--palette",
            "emu198x-v1",
        ],
    );
    assert_eq!(code, 2, "--palette on a gamut machine is a usage error");
    assert!(stdout.is_empty());
    assert!(stderr.contains("gamut"));

    let (code, stdout, _) = run_in(
        td.path(),
        &[
            "image",
            "in.png",
            "--machine",
            "commodore-amiga-ocs",
            "--format",
            "ilbm",
            "--mode",
            "hires-pal",
        ],
    );
    assert_eq!(code, 0, "{stdout}");
    assert!(stdout.contains("\"mode\": \"hires-pal\""));
}
