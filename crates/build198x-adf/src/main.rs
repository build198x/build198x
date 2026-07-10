//! `build198x-adf` — master a Kickstart-1.x hunk executable into a bootable
//! Amiga ADF floppy. The standalone twin of `build198x adf`: the same
//! operation over the same [`format_commodore_amiga_adf`] library, packaged as
//! a lean ADF-only tool with no pipeline dependencies.
//!
//! ```text
//! build198x-adf <exe> -o <out.adf> [--volume <label>] [--name <file>]
//! ```
//!
//! The on-disk file name defaults to the executable's basename; the volume
//! label defaults to that name with its first letter capitalised. Output is a
//! single JSON line on success; a diagnostic on stderr and a non-zero exit on
//! failure. The `.adf` is written atomically (temp file, then rename).

use std::path::Path;
use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, Ordering};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run(&args)
}

fn run(args: &[String]) -> ExitCode {
    let mut exe_path: Option<&String> = None;
    let mut out_path: Option<&String> = None;
    let mut volume: Option<String> = None;
    let mut name: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => {
                println!("{}", usage());
                return ExitCode::SUCCESS;
            }
            "-o" | "--output" => {
                i += 1;
                match args.get(i) {
                    Some(v) => out_path = Some(v),
                    None => return arg_error("-o needs a path"),
                }
            }
            "--volume" => {
                i += 1;
                match args.get(i) {
                    Some(v) => volume = Some(v.clone()),
                    None => return arg_error("--volume needs a label"),
                }
            }
            "--name" => {
                i += 1;
                match args.get(i) {
                    Some(v) => name = Some(v.clone()),
                    None => return arg_error("--name needs a value"),
                }
            }
            other if other.starts_with('-') => {
                return arg_error(&format!("unknown flag `{other}`"));
            }
            _ => {
                if exe_path.is_some() {
                    return arg_error("more than one executable given");
                }
                exe_path = Some(&args[i]);
            }
        }
        i += 1;
    }

    let Some(exe_path) = exe_path else {
        return arg_error("no executable given");
    };
    let Some(out_path) = out_path else {
        return arg_error("no output path given (-o <out.adf>)");
    };

    let exe = match std::fs::read(exe_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("build198x-adf: cannot read {exe_path}: {e}");
            return ExitCode::from(1);
        }
    };

    // Defaults: on-disk file name is the exe's basename; volume is that name
    // with its first letter capitalised (matching `build198x adf`).
    let name = name.unwrap_or_else(|| {
        Path::new(exe_path)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "program".to_owned())
    });
    let volume = volume.unwrap_or_else(|| {
        let mut c = name.chars();
        match c.next() {
            Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
            None => name.clone(),
        }
    });

    let img = match format_commodore_amiga_adf::master(&exe, &name, &volume) {
        Ok(img) => img,
        Err(e) => {
            eprintln!("build198x-adf: {e}");
            return ExitCode::from(1);
        }
    };

    if let Err(e) = write_atomic(Path::new(out_path), &img) {
        eprintln!("build198x-adf: {e}");
        return ExitCode::from(1);
    }

    println!(
        "{{\"tool\":\"adf\",\"output\":\"{}\",\"volume\":\"{}\",\"file\":\"{}\",\"bytes\":{},\"exe_bytes\":{}}}",
        json_escape(out_path),
        json_escape(&volume),
        json_escape(&name),
        img.len(),
        exe.len()
    );
    ExitCode::SUCCESS
}

fn arg_error(msg: &str) -> ExitCode {
    eprintln!("build198x-adf: {msg}\n\n{}", usage());
    ExitCode::from(2)
}

fn usage() -> String {
    "build198x-adf — master a hunk executable into a bootable OFS floppy\n\n\
     Usage:\n\
     \x20 build198x-adf <exe> -o <out.adf> [--volume <label>] [--name <file>]\n\n\
     Writes an 880K OFS DD `.adf` that boots on a bare A500/KS1.3 straight\n\
     into the program.\n\n\
     Options:\n\
     \x20 -o, --output <path>   the .adf to write (required)\n\
     \x20     --volume <label>  disk volume label (default: capitalised name)\n\
     \x20     --name <file>     on-disk file + startup-sequence command\n\
     \x20                       (default: the executable's basename)\n\
     \x20 -h, --help            show this help"
        .to_owned()
}

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Write `bytes` to `path` atomically: a temp file in the same directory, then
/// a rename. A failed write leaves nothing behind.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let dir = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "out".to_owned());
    let tmp = dir.join(format!(
        ".{file_name}.tmp.{}.{}",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    if let Err(e) = std::fs::write(&tmp, bytes) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("cannot write {}: {e}", path.display()));
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("cannot write {}: {e}", path.display()));
    }
    Ok(())
}

/// Minimal JSON string escaping for the one-line result object.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_escape_matches_the_pipeline_tool() {
        assert_eq!(json_escape("plain 123"), "plain 123");
        assert_eq!(json_escape(r#"a"b\c"#), r#"a\"b\\c"#);
        assert_eq!(json_escape("a\nb\tc\u{1}d"), "a\\nb\\tc\\u0001d");
    }

    #[test]
    fn missing_args_are_rejected() {
        assert_eq!(run(&[]), ExitCode::from(2));
        assert_eq!(run(&["x".to_owned()]), ExitCode::from(2));
    }
}
