//! Native output, metadata, repeatability and failure preservation at the CLI.
use std::path::PathBuf;
use std::process::{Command, Output};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("build198x-plus3-{}", std::process::id()));
        std::fs::create_dir(&path).expect("unique fixture directory");
        std::fs::write(
            path.join("loader.bas"),
            "  10 CLEAR 32767\n  20 LOAD \"DIAG.BIN\"CODE\n  30 RANDOMIZE USR 32768\n",
        )
        .expect("loader");
        std::fs::write(path.join("code.bin"), [0xc9]).expect("CODE");
        Self(path)
    }
    fn run(&self, extra: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_build198x"))
            .current_dir(&self.0)
            .args([
                "plus3",
                "--basic",
                "loader.bas",
                "--code",
                "code.bin",
                "--load-address",
                "32768",
                "-o",
                "out.dsk",
            ])
            .args(extra)
            .output()
            .expect("run CLI")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn disk_build_and_failures_preserve_inputs_and_outputs() {
    let fixture = Fixture::new();
    let result = fixture.run(&["--format", "json"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report = String::from_utf8(result.stdout).expect("UTF-8");
    assert!(report.contains("\"autorun\":10"));
    let original = std::fs::read(fixture.0.join("out.dsk")).expect("DSK");
    let parsed = format198x_amstrad_cpc_dsk::parse(&original).expect("DSK parse");
    // Resolve the directory allocation pointer, rather than assuming the writer's
    // placement for CODE. Then check its +3DOS header and the authored payload.
    let directory = &parsed.sector(1, 0, 1).expect("directory").data;
    let entry = &directory[32..64];
    assert_eq!(&entry[1..12], b"DIAG    BIN");
    let physical_sector = 9 + usize::from(entry[16]) * 2;
    let file = &parsed
        .sector(
            (physical_sector / 9) as u8,
            0,
            (physical_sector % 9 + 1) as u8,
        )
        .expect("allocated sector")
        .data;
    assert_eq!(&file[..9], b"PLUS3DOS\x1a");
    assert_eq!(&file[15..20], &[3, 1, 0, 0, 128]);
    assert_eq!(file[128], 0xc9);
    assert!(!fixture.run(&[]).status.success());
    assert!(fixture.run(&["--force"]).status.success());
    assert_eq!(
        original,
        std::fs::read(fixture.0.join("out.dsk")).expect("repeat")
    );
    std::fs::write(fixture.0.join("code.bin"), []).expect("empty CODE");
    assert!(!fixture.run(&["--force"]).status.success());
    assert_eq!(
        original,
        std::fs::read(fixture.0.join("out.dsk")).expect("preserved")
    );
    std::fs::remove_file(fixture.0.join("code.bin")).expect("remove prerequisite");
    assert_eq!(fixture.run(&["--force"]).status.code(), Some(3));
    assert_eq!(
        original,
        std::fs::read(fixture.0.join("out.dsk")).expect("preserved")
    );
}
