//! `build198x tape`: the committed example, the report, and every refusal
//! leaving inputs and any existing output untouched.
mod common;

use std::path::PathBuf;
use std::process::{Command, Output};

struct Fixture(PathBuf);
impl Fixture {
    fn new(tag: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("build198x-tape-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir(&path).expect("unique fixture directory");
        std::fs::write(path.join("code.bin"), [0x3e, 0x02, 0xd3, 0xfe, 0x18, 0xfe]).expect("CODE");
        Self(path)
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_build198x"))
            .current_dir(&self.0)
            .arg("tape")
            .args(args)
            .output()
            .expect("run CLI")
    }
    /// The standard invocation plus `extra`.
    fn master(&self, extra: &[&str]) -> Output {
        let mut args = vec![
            "--machine",
            "sinclair-zx-spectrum",
            "--code",
            "code.bin",
            "--load-address",
            "32768",
            "-o",
            "out.tap",
        ];
        args.extend_from_slice(extra);
        self.run(&args)
    }
    fn read(&self, name: &str) -> Vec<u8> {
        std::fs::read(self.0.join(name)).expect("file")
    }
    fn exists(&self, name: &str) -> bool {
        self.0.join(name).exists()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn rebuilds_the_committed_test_card_example() {
    let example = common::fixtures().join("tape").join("test-card");
    let f = Fixture::new("example");
    std::fs::copy(example.join("border.bin"), f.0.join("border.bin")).expect("copy");
    let out = f.run(&[
        "--machine",
        "sinclair-zx-spectrum",
        "--code",
        "border.bin",
        "--load-address",
        "32768",
        "-o",
        "border.tap",
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(
        f.read("border.tap"),
        std::fs::read(example.join("border.tap")).expect("committed tape"),
        "the tape master's output changed; regenerate the example only if that was intended"
    );
}

#[test]
fn reports_what_went_onto_the_tape() {
    let f = Fixture::new("report");
    let out = f.master(&["--entry", "32770", "--format", "json"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let report = String::from_utf8(out.stdout).expect("UTF-8");
    for field in [
        "\"machine\":\"sinclair-zx-spectrum\"",
        "\"model\":\"48k\"",
        "\"loader\":\"generated\"",
        "\"clear\":32767",
        "\"screen\":\"test-card\"",
        "\"code_length\":6",
        "\"entry\":32770",
    ] {
        assert!(report.contains(field), "{field} missing from {report}");
    }
}

#[test]
fn an_authored_loader_and_a_supplied_screen_are_used() {
    let f = Fixture::new("authored");
    std::fs::write(
        f.0.join("loader.bas"),
        "  10 CLEAR 32767\n  20 LOAD \"\"SCREEN$ \n  30 LOAD \"\"CODE \n  40 RANDOMIZE USR 32768\n",
    )
    .expect("loader");
    std::fs::write(f.0.join("title.scr"), [0x55u8; 6912]).expect("screen");
    let out = f.master(&["--basic", "loader.bas", "--screen", "title.scr"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let tape = f.read("out.tap");
    assert!(
        tape.windows(6912).any(|w| w.iter().all(|&b| b == 0x55)),
        "the supplied screen is on the tape"
    );
}

#[test]
fn usage_errors_exit_2_and_write_nothing() {
    let f = Fixture::new("usage");
    let no_machine = f.run(&[
        "--code",
        "code.bin",
        "--load-address",
        "32768",
        "-o",
        "out.tap",
    ]);
    assert_eq!(no_machine.status.code(), Some(2));
    assert!(stderr(&no_machine).contains("--machine"));

    let c64 = f.run(&[
        "--machine",
        "commodore-c64",
        "--code",
        "code.bin",
        "--load-address",
        "32768",
        "-o",
        "out.tap",
    ]);
    assert_eq!(c64.status.code(), Some(2));
    assert!(stderr(&c64).contains("no tape master for `commodore-c64`"));

    std::fs::write(f.0.join("loader.bas"), "  10 STOP\n").expect("loader");
    for extra in [
        vec!["--basic", "loader.bas", "--entry", "32768"],
        vec!["--basic", "loader.bas", "--border", "1"],
        vec!["--border", "8"],
        vec!["--model", "128k"],
        vec!["--load-address", "1"],
    ] {
        assert_eq!(f.master(&extra).status.code(), Some(2), "{extra:?}");
    }
    let wrong_extension = f.run(&[
        "--machine",
        "sinclair-zx-spectrum",
        "--code",
        "code.bin",
        "--load-address",
        "32768",
        "-o",
        "out.bin",
    ]);
    assert_eq!(wrong_extension.status.code(), Some(2));
    assert!(!f.exists("out.tap") && !f.exists("out.bin"));
}

#[test]
fn refusals_keep_inputs_and_any_existing_output() {
    let f = Fixture::new("refuse");
    assert!(f.master(&[]).status.success());
    let original = f.read("out.tap");

    let again = f.master(&[]);
    assert_eq!(
        again.status.code(),
        Some(5),
        "an existing output needs --force"
    );
    assert!(f.master(&["--force"]).status.success());
    assert_eq!(
        f.read("out.tap"),
        original,
        "the same inputs give the same tape"
    );

    // Content refusals (exit 4) leave the existing tape alone.
    for extra in [
        vec!["--force", "--entry", "40000"],
        vec!["--force", "--model", "16k"],
    ] {
        let out = f.master(&extra);
        assert_eq!(out.status.code(), Some(4), "{extra:?}: {}", stderr(&out));
        assert_eq!(f.read("out.tap"), original, "{extra:?}");
    }
    std::fs::write(f.0.join("short.scr"), [0u8; 100]).expect("screen");
    assert_eq!(
        f.master(&["--force", "--screen", "short.scr"])
            .status
            .code(),
        Some(4)
    );

    // An unreadable input is exit 3.
    assert_eq!(
        f.master(&["--force", "--screen", "absent.scr"])
            .status
            .code(),
        Some(3)
    );

    // The output may not be one of the inputs.
    let alias = f.run(&[
        "--machine",
        "sinclair-zx-spectrum",
        "--code",
        "out.tap",
        "--load-address",
        "32768",
        "-o",
        "out.tap",
        "--force",
    ]);
    assert_eq!(alias.status.code(), Some(5));
    assert_eq!(f.read("out.tap"), original);
    assert_eq!(f.read("code.bin"), [0x3e, 0x02, 0xd3, 0xfe, 0x18, 0xfe]);
}
