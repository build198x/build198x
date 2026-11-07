//! `--hd` writes a 1760K high-density disk; the default stays an 880K DD one.
use format198x_commodore_amiga_adf::{DD, Disk, HD};
use std::path::PathBuf;
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_build198x-adf");
const PREFIX: &[&str] = &[];

struct Fixture(PathBuf);
impl Fixture {
    fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "build198x-adf-density-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir(&path).expect("unique fixture directory");
        Self(path)
    }
    fn write(&self, name: &str, bytes: &[u8]) {
        std::fs::write(self.0.join(name), bytes).expect("fixture file");
    }
    fn read(&self, name: &str) -> Vec<u8> {
        std::fs::read(self.0.join(name)).expect("output disk")
    }
    fn exists(&self, name: &str) -> bool {
        self.0.join(name).exists()
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(BIN)
            .current_dir(&self.0)
            .args(PREFIX)
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

#[test]
fn master_writes_dd_by_default_and_hd_on_request() {
    let f = Fixture::new("master");
    f.write("game", b"not really a hunk, but the disk does not care");

    let dd = f.run(&["master", "game", "-o", "dd.adf"]);
    assert!(dd.status.success(), "{dd:?}");
    assert!(stdout(&dd).contains("880K OFS"), "{}", stdout(&dd));
    let img = f.read("dd.adf");
    assert_eq!(img.len(), DD.len());
    assert_eq!(Disk::open(&img).expect("opens").geometry(), DD);

    let hd = f.run(&["master", "game", "-o", "hd.adf", "--hd"]);
    assert!(hd.status.success(), "{hd:?}");
    assert!(stdout(&hd).contains("1760K OFS"), "{}", stdout(&hd));
    let img = f.read("hd.adf");
    assert_eq!(img.len(), HD.len());
    let disk = Disk::open(&img).expect("opens");
    assert_eq!(disk.geometry(), HD);
    disk.verify().expect("an HD master verifies");
    assert_eq!(disk.read("game").expect("file"), f.read("game"));
    assert_eq!(disk.read("s/startup-sequence").expect("startup"), b"game\n");
}

#[test]
fn create_fits_on_hd_what_does_not_fit_on_dd() {
    let f = Fixture::new("create");
    // Larger than a DD disk's ~865K of file space, well within an HD disk's.
    let payload: Vec<u8> = (0..1_200_000u32).map(|i| (i % 251) as u8).collect();
    f.write("big.bin", &payload);

    let dd = f.run(&["create", "dd.adf", "--ffs", "--add", "big.bin=data/big"]);
    assert_eq!(dd.status.code(), Some(1), "{dd:?}");
    assert!(!f.exists("dd.adf"), "a failed create must not leave a disk");

    let hd = f.run(&[
        "create",
        "hd.adf",
        "--ffs",
        "--hd",
        "--add",
        "big.bin=data/big",
        "--format",
        "json",
    ]);
    assert!(hd.status.success(), "{hd:?}");
    assert!(
        stdout(&hd).contains(&format!("\"bytes\":{}", HD.len())),
        "{}",
        stdout(&hd)
    );
    let img = f.read("hd.adf");
    let disk = Disk::open(&img).expect("opens");
    assert_eq!(disk.geometry(), HD);
    disk.verify().expect("an HD volume verifies");
    assert_eq!(disk.read("data/big").expect("file"), payload);
}

#[test]
fn the_last_density_flag_wins() {
    let f = Fixture::new("last");
    f.write("game", b"x");
    let out = f.run(&["master", "game", "-o", "d.adf", "--hd", "--dd"]);
    assert!(out.status.success(), "{out:?}");
    assert_eq!(f.read("d.adf").len(), DD.len());
}
