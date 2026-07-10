//! Build a bootable multi-directory `.adf` with the [`Volume`] builder: a
//! command in `c/`, launched from `s/startup-sequence` — the general shape
//! `master_fs` specialises.
//!
//! ```text
//! cargo run --example multi_file_disk -- <command-exe> <out.adf>
//! ```
//!
//! The executable is placed at `c/<name>` (its basename) and run by a
//! `startup-sequence` of `c/<name>`. Boots on a bare A500/KS1.3.

use format_commodore_amiga_adf::{FileSystem, Volume};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let [_, exe_path, out_path] = args.as_slice() else {
        eprintln!("usage: multi_file_disk <command-exe> <out.adf>");
        std::process::exit(2);
    };

    let bytes = std::fs::read(exe_path)?;
    let name = Path::new(exe_path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "program".to_owned());

    let mut vol = Volume::new("Demo", FileSystem::Ofs);
    vol.add_file(&format!("c/{name}"), &bytes)?;
    vol.add_file("s/startup-sequence", format!("c/{name}\n").as_bytes())?;
    vol.set_bootable(true);

    std::fs::write(out_path, vol.build()?)?;
    println!("wrote {out_path}: a bootable disk that runs c/{name}");
    Ok(())
}
