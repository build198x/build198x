//! `adf extract` copies one file's exact bytes out of an ADF and only ever reads it.
use format198x_commodore_amiga_adf::{Disk, FileSystem, Volume};
use std::path::PathBuf;
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_build198x");
const PREFIX: &[&str] = &["adf"];

/// Larger than one header's 72 data-block pointers on both filesystems, so the
/// file needs extension blocks.
fn big() -> Vec<u8> {
    (0..100_000u32).map(|i| (i * 7 % 256) as u8).collect()
}

fn disk(fs: FileSystem) -> Vec<u8> {
    let mut vol = Volume::new("Extract", fs);
    vol.add_file("Docs/Guides/ReadMe", b"Read me first.\n")
        .expect("fixture disk");
    vol.add_file("Empty", b"").expect("fixture disk");
    vol.add_file("c/Big", &big()).expect("fixture disk");
    vol.add_dir("Docs/Empty-Dir").expect("fixture disk");
    vol.build().expect("fixture disk")
}

struct Fixture(PathBuf);
impl Fixture {
    fn new(tag: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("build198x-extract-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir(&path).expect("unique fixture directory");
        Self(path)
    }
    fn write(&self, name: &str, bytes: &[u8]) {
        std::fs::write(self.0.join(name), bytes).expect("fixture file");
    }
    fn read(&self, name: &str) -> Vec<u8> {
        std::fs::read(self.0.join(name)).expect("output file")
    }
    fn exists(&self, name: &str) -> bool {
        self.0.join(name).exists()
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(BIN)
            .current_dir(&self.0)
            .args(PREFIX)
            .arg("extract")
            .args(args)
            .output()
            .expect("run CLI")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}
fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn extracts_exact_bytes_on_both_filesystems() {
    for fs in [FileSystem::Ofs, FileSystem::Ffs] {
        let f = Fixture::new(fs.name());
        let img = disk(fs);
        f.write("d.adf", &img);
        let library = Disk::open(&img).unwrap();
        for (path, out) in [
            ("Docs/Guides/ReadMe", "readme"),
            ("Empty", "empty"),
            ("c/Big", "big"),
        ] {
            let run = f.run(&["d.adf", path, "-o", out]);
            assert!(run.status.success(), "{fs:?} {path}: {run:?}");
            assert_eq!(f.read(out), library.read(path).unwrap(), "{fs:?} {path}");
        }
        assert_eq!(f.read("big"), big());
        assert!(f.read("empty").is_empty());
        assert_eq!(f.read("d.adf"), img, "the source disk is never modified");
    }
}

#[test]
fn matches_names_as_amigados_does_and_reports_the_stored_spelling() {
    let f = Fixture::new("case");
    f.write("d.adf", &disk(FileSystem::Ofs));
    let run = f.run(&[
        "d.adf",
        "DOCS/guides/readme",
        "-o",
        "out",
        "--format",
        "json",
    ]);
    assert!(run.status.success(), "{run:?}");
    assert_eq!(
        stdout(&run).trim(),
        "{\"tool\":\"adf\",\"command\":\"extract\",\"input\":\"d.adf\",\"path\":\"DOCS/guides/readme\",\"resolved\":\"Docs/Guides/ReadMe\",\"output\":\"out\",\"filesystem\":\"ofs\",\"bytes\":15}"
    );
    let text = f.run(&["d.adf", "docs/guides/README", "-o", "out2"]);
    assert_eq!(
        stdout(&text).trim(),
        "out2: 15 bytes from d.adf:Docs/Guides/ReadMe (OFS)"
    );
}

#[test]
fn refuses_what_it_cannot_extract_without_writing_anything() {
    let f = Fixture::new("refuse");
    f.write("d.adf", &disk(FileSystem::Ffs));
    f.write("junk.adf", b"not a disk");
    for (args, needle) in [
        (vec!["d.adf", "Docs/Missing", "-o", "out"], "Docs/Missing"),
        (
            vec!["d.adf", "Docs/Guides", "-o", "out"],
            "is a directory, not a file",
        ),
        (vec!["d.adf", "Empty/inside", "-o", "out"], "Empty/inside"),
        (vec!["d.adf", "", "-o", "out"], "is a directory, not a file"),
        (vec!["junk.adf", "Empty", "-o", "out"], "junk.adf"),
        (vec!["absent.adf", "Empty", "-o", "out"], "absent.adf"),
        (
            vec!["d.adf", "Empty", "-o", "no-such-dir/out"],
            "no-such-dir/out",
        ),
    ] {
        let run = f.run(&args);
        assert_eq!(run.status.code(), Some(1), "{args:?}: {run:?}");
        assert!(stderr(&run).contains(needle), "{args:?}: {}", stderr(&run));
        assert!(!f.exists("out"), "{args:?} left an output behind");
    }
}

#[test]
fn keeps_an_existing_destination_unless_forced() {
    let f = Fixture::new("force");
    let img = disk(FileSystem::Ofs);
    f.write("d.adf", &img);
    f.write("out", b"keep me");

    let refused = f.run(&["d.adf", "Docs/Guides/ReadMe", "-o", "out"]);
    assert_eq!(refused.status.code(), Some(1), "{refused:?}");
    assert!(stderr(&refused).contains("--force"), "{}", stderr(&refused));
    assert_eq!(f.read("out"), b"keep me");

    let forced = f.run(&["d.adf", "Docs/Guides/ReadMe", "-o", "out", "--force"]);
    assert!(forced.status.success(), "{forced:?}");
    assert_eq!(f.read("out"), b"Read me first.\n");

    let onto_source = f.run(&["d.adf", "Docs/Guides/ReadMe", "-o", "./d.adf", "--force"]);
    assert_eq!(onto_source.status.code(), Some(1), "{onto_source:?}");
    assert_eq!(
        f.read("d.adf"),
        img,
        "even --force never writes over the disk"
    );
}

#[test]
fn usage_errors_exit_2() {
    let f = Fixture::new("usage");
    for args in [
        vec!["d.adf", "Empty"],
        vec!["d.adf", "-o", "out"],
        vec!["d.adf", "Empty", "extra", "-o", "out"],
        vec!["d.adf", "Empty", "-o", "out", "--bogus"],
    ] {
        assert_eq!(f.run(&args).status.code(), Some(2), "{args:?}");
    }
}
