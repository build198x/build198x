//! `build198x-adf` — master, verify, and inspect bootable Amiga ADF floppies.
//! The standalone twin of `build198x adf`: the same operations over the same
//! [`format_commodore_amiga_adf`] library, packaged as a lean ADF-only tool
//! with no pipeline dependencies.
//!
//! ```text
//! build198x-adf <exe> -o <out.adf> [--volume <label>] [--name <file>] [--ffs]  # master (shorthand)
//! build198x-adf master <exe> -o <out.adf> [flags]                              # master, explicit
//! build198x-adf create <out.adf> [--add host=dest]... [--mkdir dir]... [flags]  # general volume builder
//! build198x-adf verify <disk.adf>                                              # integrity check
//! build198x-adf info   <disk.adf>                                              # label, fs, contents
//! ```
//!
//! Output is human-readable by default; pass `--format json` on any verb for a
//! single machine-readable JSON line. Exit codes: `0` ok, `1` runtime/verify
//! failure, `2` usage error. On failure a diagnostic goes to stderr. A mastered
//! `.adf` is written atomically (temp file, then rename).

use format_commodore_amiga_adf::{Disk, Entry, EntryKind, FileSystem, Volume};
use std::path::Path;
use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, Ordering};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run(&args)
}

/// Top-level verb dispatch. A leading `master`/`create`/`verify`/`info` selects
/// the verb; anything else is an implicit `master` (the shorthand
/// `build198x-adf <exe> -o <out.adf>`), preserving the pre-verb interface.
fn run(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("--help" | "-h") => {
            println!("{}", top_usage());
            ExitCode::SUCCESS
        }
        Some("master") => cmd_master(&args[1..]),
        Some("create") => cmd_create(&args[1..]),
        Some("verify") => cmd_verify(&args[1..]),
        Some("info") => cmd_info(&args[1..]),
        _ => cmd_master(args),
    }
}

/// The output format shared by every verb: human-readable text (default) or a
/// single JSON line (`--format json`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Text,
    Json,
}

/// Pull `--format <text|json>` out of an argument list, returning the chosen
/// format and the remaining arguments. Defaults to text. Errors on a missing or
/// unrecognised value.
fn take_format(args: &[String]) -> Result<(Format, Vec<String>), String> {
    let mut fmt = Format::Text;
    let mut rest = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--format" {
            i += 1;
            match args.get(i).map(String::as_str) {
                Some("text") => fmt = Format::Text,
                Some("json") => fmt = Format::Json,
                Some(other) => {
                    return Err(format!("unknown format `{other}` (use text or json)"));
                }
                None => return Err("--format needs a value (text or json)".to_owned()),
            }
        } else {
            rest.push(args[i].clone());
        }
        i += 1;
    }
    Ok((fmt, rest))
}

// ---------------------------------------------------------------------------
// master
// ---------------------------------------------------------------------------

fn cmd_master(args: &[String]) -> ExitCode {
    let (fmt, rest) = match take_format(args) {
        Ok(v) => v,
        Err(e) => return arg_error(&e),
    };

    let mut exe_path: Option<&String> = None;
    let mut out_path: Option<&String> = None;
    let mut volume: Option<String> = None;
    let mut name: Option<String> = None;
    let mut fs = FileSystem::Ofs;

    let mut i = 0;
    while i < rest.len() {
        match rest[i].as_str() {
            "--help" | "-h" => {
                println!("{}", usage());
                return ExitCode::SUCCESS;
            }
            "--ffs" => fs = FileSystem::Ffs,
            "--ofs" => fs = FileSystem::Ofs,
            "-o" | "--output" => {
                i += 1;
                match rest.get(i) {
                    Some(v) => out_path = Some(v),
                    None => return arg_error("-o needs a path"),
                }
            }
            "--volume" => {
                i += 1;
                match rest.get(i) {
                    Some(v) => volume = Some(v.clone()),
                    None => return arg_error("--volume needs a label"),
                }
            }
            "--name" => {
                i += 1;
                match rest.get(i) {
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
                exe_path = Some(&rest[i]);
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

    let exe = match read_bytes(exe_path) {
        Ok(b) => b,
        Err(code) => return code,
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

    let img = match format_commodore_amiga_adf::master_fs(&exe, &name, &volume, fs) {
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

    let line = match fmt {
        Format::Text => master_text(out_path, img.len(), &volume, &name, fs, exe.len()),
        Format::Json => master_json(out_path, img.len(), &volume, &name, fs, exe.len()),
    };
    println!("{line}");
    ExitCode::SUCCESS
}

fn master_text(
    out_path: &str,
    img_len: usize,
    volume: &str,
    name: &str,
    fs: FileSystem,
    exe_len: usize,
) -> String {
    format!(
        "{out_path}: {}K {} disk \"{volume}\", {name} ({exe_len} bytes)",
        img_len / 1024,
        fs.name().to_uppercase(),
    )
}

fn master_json(
    out_path: &str,
    img_len: usize,
    volume: &str,
    name: &str,
    fs: FileSystem,
    exe_len: usize,
) -> String {
    format!(
        "{{\"tool\":\"adf\",\"output\":\"{}\",\"volume\":\"{}\",\"file\":\"{}\",\"filesystem\":\"{}\",\"bytes\":{},\"exe_bytes\":{}}}",
        json_escape(out_path),
        json_escape(volume),
        json_escape(name),
        fs.name(),
        img_len,
        exe_len
    )
}

// ---------------------------------------------------------------------------
// verify
// ---------------------------------------------------------------------------

fn cmd_verify(args: &[String]) -> ExitCode {
    let (fmt, rest) = match take_format(args) {
        Ok(v) => v,
        Err(e) => return verb_arg_error("verify", &e),
    };
    let path = match single_disk_arg("verify", &rest) {
        Ok(p) => p,
        Err(code) => return code,
    };
    let img = match read_bytes(&path) {
        Ok(b) => b,
        Err(code) => return code,
    };
    run_verify(&img, &path, fmt)
}

/// Open and deep-verify an in-memory image, printing the verdict. Split from
/// `cmd_verify` so the open/verify/error paths are testable without a file.
fn run_verify(img: &[u8], path: &str, fmt: Format) -> ExitCode {
    let disk = match Disk::open(img) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("build198x-adf: {e}");
            return ExitCode::from(1);
        }
    };
    if let Err(e) = disk.verify() {
        eprintln!("build198x-adf: {e}");
        return ExitCode::from(1);
    }
    let line = match fmt {
        Format::Text => verify_text(path, img.len(), disk.filesystem(), &disk.label()),
        Format::Json => verify_json(path, disk.filesystem(), &disk.label()),
    };
    println!("{line}");
    ExitCode::SUCCESS
}

fn verify_text(path: &str, img_len: usize, fs: FileSystem, label: &str) -> String {
    format!(
        "{path}: OK — {}K {} \"{label}\"",
        img_len / 1024,
        fs.name().to_uppercase(),
    )
}

fn verify_json(path: &str, fs: FileSystem, label: &str) -> String {
    format!(
        "{{\"tool\":\"adf\",\"command\":\"verify\",\"input\":\"{}\",\"filesystem\":\"{}\",\"label\":\"{}\",\"result\":\"ok\"}}",
        json_escape(path),
        fs.name(),
        json_escape(label)
    )
}

// ---------------------------------------------------------------------------
// info
// ---------------------------------------------------------------------------

fn cmd_info(args: &[String]) -> ExitCode {
    let (fmt, rest) = match take_format(args) {
        Ok(v) => v,
        Err(e) => return verb_arg_error("info", &e),
    };
    let path = match single_disk_arg("info", &rest) {
        Ok(p) => p,
        Err(code) => return code,
    };
    let img = match read_bytes(&path) {
        Ok(b) => b,
        Err(code) => return code,
    };
    run_info(&img, &path, fmt)
}

/// Open an in-memory image and print its label, filesystem, and root listing.
/// Split from `cmd_info` for the same file-free testability as `run_verify`.
fn run_info(img: &[u8], path: &str, fmt: Format) -> ExitCode {
    let disk = match Disk::open(img) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("build198x-adf: {e}");
            return ExitCode::from(1);
        }
    };
    let entries = match disk.list("") {
        Ok(e) => sorted_entries(e),
        Err(e) => {
            eprintln!("build198x-adf: {e}");
            return ExitCode::from(1);
        }
    };
    let line = match fmt {
        Format::Text => info_text(path, img.len(), disk.filesystem(), &disk.label(), &entries),
        Format::Json => info_json(path, img.len(), disk.filesystem(), &disk.label(), &entries),
    };
    println!("{line}");
    ExitCode::SUCCESS
}

/// Entries sorted by name — the stable order `info` presents.
fn sorted_entries(mut entries: Vec<Entry>) -> Vec<Entry> {
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
}

fn info_text(path: &str, img_len: usize, fs: FileSystem, label: &str, entries: &[Entry]) -> String {
    let mut out = format!(
        "{path}: {}K {} \"{label}\"",
        img_len / 1024,
        fs.name().to_uppercase(),
    );
    if entries.is_empty() {
        out.push_str("\n  (empty)");
        return out;
    }
    let rows: Vec<(&str, &str, String)> = entries
        .iter()
        .map(|e| {
            let (kind, size) = match e.kind {
                EntryKind::File => ("file", e.size.to_string()),
                EntryKind::Directory => ("dir", "-".to_owned()),
            };
            (e.name.as_str(), kind, size)
        })
        .collect();
    let name_w = rows
        .iter()
        .map(|(n, ..)| n.chars().count())
        .chain(std::iter::once(4)) // "NAME"
        .max()
        .unwrap_or(4);
    let size_w = rows
        .iter()
        .map(|(.., s)| s.len())
        .chain(std::iter::once(4)) // "SIZE"
        .max()
        .unwrap_or(4);
    out.push_str(&format!(
        "\n  {:<name_w$}  {:<4}  {:>size_w$}",
        "NAME", "KIND", "SIZE"
    ));
    for (name, kind, size) in &rows {
        out.push_str(&format!("\n  {name:<name_w$}  {kind:<4}  {size:>size_w$}"));
    }
    out
}

fn info_json(path: &str, img_len: usize, fs: FileSystem, label: &str, entries: &[Entry]) -> String {
    let mut out = format!(
        "{{\"tool\":\"adf\",\"command\":\"info\",\"input\":\"{}\",\"filesystem\":\"{}\",\"label\":\"{}\",\"bytes\":{},\"entries\":[",
        json_escape(path),
        fs.name(),
        json_escape(label),
        img_len
    );
    for (i, e) in entries.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let kind = match e.kind {
            EntryKind::File => "file",
            EntryKind::Directory => "dir",
        };
        out.push_str(&format!(
            "{{\"name\":\"{}\",\"kind\":\"{}\",\"size\":{}}}",
            json_escape(&e.name),
            kind,
            e.size
        ));
    }
    out.push_str("]}");
    out
}

// ---------------------------------------------------------------------------
// create — the general Volume builder
// ---------------------------------------------------------------------------

fn cmd_create(args: &[String]) -> ExitCode {
    let (fmt, rest) = match take_format(args) {
        Ok(v) => v,
        Err(e) => return verb_arg_error("create", &e),
    };

    let mut out_path: Option<String> = None;
    let mut label: Option<String> = None;
    let mut adds: Vec<(String, String)> = Vec::new();
    let mut mkdirs: Vec<String> = Vec::new();
    let mut bootable = false;
    let mut startup: Option<String> = None;
    let mut fs = FileSystem::Ofs;

    let mut i = 0;
    while i < rest.len() {
        match rest[i].as_str() {
            "--help" | "-h" => {
                println!("{}", verb_usage("create"));
                return ExitCode::SUCCESS;
            }
            "--ffs" => fs = FileSystem::Ffs,
            "--ofs" => fs = FileSystem::Ofs,
            "--bootable" => bootable = true,
            "--label" => {
                i += 1;
                match rest.get(i) {
                    Some(v) => label = Some(v.clone()),
                    None => return verb_arg_error("create", "--label needs a value"),
                }
            }
            "--startup" => {
                i += 1;
                match rest.get(i) {
                    Some(v) => startup = Some(v.clone()),
                    None => return verb_arg_error("create", "--startup needs a command"),
                }
            }
            "--mkdir" => {
                i += 1;
                match rest.get(i) {
                    Some(v) => mkdirs.push(v.clone()),
                    None => return verb_arg_error("create", "--mkdir needs a path"),
                }
            }
            "--add" => {
                i += 1;
                match rest.get(i) {
                    Some(v) => match parse_add(v) {
                        Ok(pair) => adds.push(pair),
                        Err(e) => return verb_arg_error("create", &e),
                    },
                    None => return verb_arg_error("create", "--add needs a host file"),
                }
            }
            other if other.starts_with('-') => {
                return verb_arg_error("create", &format!("unknown flag `{other}`"));
            }
            _ => {
                if out_path.is_some() {
                    return verb_arg_error("create", "more than one output path given");
                }
                out_path = Some(rest[i].clone());
            }
        }
        i += 1;
    }

    let Some(out_path) = out_path else {
        return verb_arg_error("create", "no output path given (<out.adf>)");
    };
    let label = label.unwrap_or_else(|| default_label(&out_path));
    if startup.is_some() {
        bootable = true; // a startup-sequence only runs on a bootable disk
    }

    let img = match build_volume(&label, fs, bootable, &mkdirs, &adds, startup.as_deref()) {
        Ok(img) => img,
        Err(code) => return code,
    };

    if let Err(e) = write_atomic(Path::new(&out_path), &img) {
        eprintln!("build198x-adf: {e}");
        return ExitCode::from(1);
    }

    let files = adds.len() + usize::from(startup.is_some());
    let dirs = mkdirs.len();
    let line = match fmt {
        Format::Text => create_text(&out_path, img.len(), fs, &label, bootable, files, dirs),
        Format::Json => create_json(&out_path, img.len(), fs, &label, bootable, files, dirs),
    };
    println!("{line}");
    ExitCode::SUCCESS
}

/// Parse an `--add` spec `host[=dest]` into `(host, dest)`. Without `=`, the
/// destination is the host basename at the root. A `dest` ending in `/` keeps
/// the host basename inside that directory. Split on the first `=` so Windows
/// drive-letter host paths (`C:\x=c/x`) survive.
fn parse_add(spec: &str) -> Result<(String, String), String> {
    match spec.split_once('=') {
        Some((host, dest)) => {
            if host.is_empty() {
                return Err(format!("--add {spec}: empty host path"));
            }
            if dest.is_empty() {
                return Err(format!("--add {spec}: empty destination"));
            }
            let dest = match dest.strip_suffix('/') {
                Some(dir) => {
                    let base = host_basename(host);
                    if dir.is_empty() {
                        base
                    } else {
                        format!("{dir}/{base}")
                    }
                }
                None => dest.to_owned(),
            };
            Ok((host.to_owned(), dest))
        }
        None => Ok((spec.to_owned(), host_basename(spec))),
    }
}

/// The final path component of a host path, honouring the host OS separators.
fn host_basename(host: &str) -> String {
    Path::new(host)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| host.to_owned())
}

/// The default volume label: the output filename stem, first letter capitalised
/// (`game.adf` -> `Game`), matching how `master` derives its volume label.
fn default_label(out_path: &str) -> String {
    let stem = Path::new(out_path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut c = stem.chars();
    match c.next() {
        Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
        None => "Disk".to_owned(),
    }
}

/// Read each host file and assemble the volume, mapping any failure to `exit 1`
/// with a stderr diagnostic. Reading is split from [`assemble_volume`] so the
/// build logic stays testable with in-memory bytes.
fn build_volume(
    label: &str,
    fs: FileSystem,
    bootable: bool,
    mkdirs: &[String],
    adds: &[(String, String)],
    startup: Option<&str>,
) -> Result<Vec<u8>, ExitCode> {
    let mut files = Vec::with_capacity(adds.len());
    for (host, dest) in adds {
        match std::fs::read(host) {
            Ok(bytes) => files.push((dest.clone(), bytes)),
            Err(e) => {
                eprintln!("build198x-adf: cannot read {host}: {e}");
                return Err(ExitCode::from(1));
            }
        }
    }
    match assemble_volume(label, fs, bootable, mkdirs, &files, startup) {
        Ok(img) => Ok(img),
        Err(e) => {
            eprintln!("build198x-adf: {e}");
            Err(ExitCode::from(1))
        }
    }
}

/// Build an ADF image from in-memory contents: directories, then files (each a
/// `(dest, bytes)`), then an optional `s/startup-sequence`. Pure over the
/// library `Volume`, so it is unit-testable without touching the disk.
fn assemble_volume(
    label: &str,
    fs: FileSystem,
    bootable: bool,
    mkdirs: &[String],
    files: &[(String, Vec<u8>)],
    startup: Option<&str>,
) -> Result<Vec<u8>, format_commodore_amiga_adf::Error> {
    let mut vol = Volume::new(label, fs);
    vol.set_bootable(bootable);
    for d in mkdirs {
        vol.add_dir(d)?;
    }
    for (dest, bytes) in files {
        vol.add_file(dest, bytes)?;
    }
    if let Some(cmd) = startup {
        vol.add_file("s/startup-sequence", format!("{cmd}\n").as_bytes())?;
    }
    vol.build()
}

fn create_text(
    out_path: &str,
    img_len: usize,
    fs: FileSystem,
    label: &str,
    bootable: bool,
    files: usize,
    dirs: usize,
) -> String {
    let boot = if bootable { ", bootable" } else { "" };
    format!(
        "{out_path}: created — {}K {} \"{label}\", {}, {}{boot}",
        img_len / 1024,
        fs.name().to_uppercase(),
        plural(files, "file"),
        plural(dirs, "dir"),
    )
}

fn create_json(
    out_path: &str,
    img_len: usize,
    fs: FileSystem,
    label: &str,
    bootable: bool,
    files: usize,
    dirs: usize,
) -> String {
    format!(
        "{{\"tool\":\"adf\",\"command\":\"create\",\"output\":\"{}\",\"filesystem\":\"{}\",\"label\":\"{}\",\"bootable\":{},\"files\":{},\"dirs\":{},\"bytes\":{}}}",
        json_escape(out_path),
        fs.name(),
        json_escape(label),
        bootable,
        files,
        dirs,
        img_len
    )
}

/// `1 file` / `3 files` — singular for one, plural otherwise.
fn plural(n: usize, word: &str) -> String {
    if n == 1 {
        format!("1 {word}")
    } else {
        format!("{n} {word}s")
    }
}

// ---------------------------------------------------------------------------
// shared helpers
// ---------------------------------------------------------------------------

/// Read a file, reporting a read failure as `exit 1` with a stderr diagnostic.
fn read_bytes(path: &str) -> Result<Vec<u8>, ExitCode> {
    match std::fs::read(path) {
        Ok(b) => Ok(b),
        Err(e) => {
            eprintln!("build198x-adf: cannot read {path}: {e}");
            Err(ExitCode::from(1))
        }
    }
}

/// Parse a read verb's single `<disk.adf>` positional (plus `-h`/`--help`).
/// `Err` carries the exit code: `SUCCESS` for help, `2` for a usage error.
fn single_disk_arg(verb: &str, rest: &[String]) -> Result<String, ExitCode> {
    let mut path: Option<&String> = None;
    for a in rest {
        match a.as_str() {
            "--help" | "-h" => {
                println!("{}", verb_usage(verb));
                return Err(ExitCode::SUCCESS);
            }
            other if other.starts_with('-') => {
                return Err(verb_arg_error(verb, &format!("unknown flag `{other}`")));
            }
            _ => {
                if path.is_some() {
                    return Err(verb_arg_error(verb, "more than one .adf given"));
                }
                path = Some(a);
            }
        }
    }
    match path {
        Some(p) => Ok(p.clone()),
        None => Err(verb_arg_error(verb, "no .adf given")),
    }
}

fn arg_error(msg: &str) -> ExitCode {
    eprintln!("build198x-adf: {msg}\n\n{}", usage());
    ExitCode::from(2)
}

fn verb_arg_error(verb: &str, msg: &str) -> ExitCode {
    eprintln!("build198x-adf {verb}: {msg}\n\n{}", verb_usage(verb));
    ExitCode::from(2)
}

fn top_usage() -> String {
    "build198x-adf — master, create, verify, and inspect bootable Amiga ADF floppies\n\n\
     Usage:\n\
     \x20 build198x-adf <exe> -o <out.adf> [flags]         master (shorthand)\n\
     \x20 build198x-adf master <exe> -o <out.adf> [flags]  master a hunk exe into a bootable disk\n\
     \x20 build198x-adf create <out.adf> [flags]           build a volume from files and dirs\n\
     \x20 build198x-adf verify <disk.adf>                  check an ADF's integrity\n\
     \x20 build198x-adf info <disk.adf>                     show label, filesystem, and contents\n\n\
     Every verb takes --format text|json (default text). Run `<verb> --help`\n\
     for the verb's own options."
        .to_owned()
}

fn verb_usage(verb: &str) -> String {
    match verb {
        "create" => "build198x-adf create — build an ADF volume from files and directories\n\n\
             Usage:\n\
             \x20 build198x-adf create <out.adf> [options]\n\n\
             Options:\n\
             \x20     --label <name>     volume label (default: capitalised output stem)\n\
             \x20     --add <host>[=<dest>]  add a host file at <dest> (repeatable;\n\
             \x20                       dest defaults to the basename; a trailing / keeps it)\n\
             \x20     --mkdir <dest>    create an empty directory (repeatable)\n\
             \x20     --bootable        write a boot block (default: not bootable)\n\
             \x20     --startup <cmd>   write s/startup-sequence running <cmd> (implies --bootable)\n\
             \x20     --ofs | --ffs     filesystem (default: --ofs; --ffs needs KS2.0+)\n\
             \x20     --format <fmt>    output format: text (default) or json"
            .to_owned(),
        "verify" => "build198x-adf verify — check an ADF's integrity (checksums + structure)\n\n\
             Usage:\n\
             \x20 build198x-adf verify <disk.adf> [--format text|json]\n\n\
             Exits 0 if the disk is sound, 1 if it is corrupt or not an ADF."
            .to_owned(),
        "info" => "build198x-adf info — show an ADF's label, filesystem, and root listing\n\n\
             Usage:\n\
             \x20 build198x-adf info <disk.adf> [--format text|json]"
            .to_owned(),
        _ => usage(),
    }
}

fn usage() -> String {
    "build198x-adf master — master a hunk executable into a bootable Amiga floppy\n\n\
     Usage:\n\
     \x20 build198x-adf [master] <exe> -o <out.adf> [--volume <label>] [--name <file>] [--ffs]\n\n\
     Writes an 880K DD `.adf` that boots straight into the program. OFS is the\n\
     default and boots on a bare A500/KS1.3; --ffs is denser but needs KS2.0+.\n\n\
     Options:\n\
     \x20 -o, --output <path>   the .adf to write (required)\n\
     \x20     --volume <label>  disk volume label (default: capitalised name)\n\
     \x20     --name <file>     on-disk file + startup-sequence command\n\
     \x20                       (default: the executable's basename)\n\
     \x20     --ofs | --ffs     filesystem (default: --ofs; --ffs needs KS2.0+)\n\
     \x20     --format <fmt>    output format: text (default) or json\n\
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

/// Minimal JSON string escaping for the one-line result objects.
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
    /// A well-formed OFS disk: a file plus two dirs at the root.
    fn good_adf() -> Vec<u8> {
        let mut v = Volume::new("MyGame", FileSystem::Ofs);
        v.add_file("mygame", &[0u8; 2048]).unwrap();
        v.add_dir("c").unwrap();
        v.add_dir("s").unwrap();
        v.build().unwrap()
    }

    #[test]
    fn parse_add_maps_host_to_dest() {
        assert_eq!(
            parse_add("mygame").unwrap(),
            ("mygame".to_owned(), "mygame".to_owned())
        );
        assert_eq!(
            parse_add("art/logo.iff").unwrap(),
            ("art/logo.iff".to_owned(), "logo.iff".to_owned())
        );
        assert_eq!(
            parse_add("boot.exe=c/boot").unwrap(),
            ("boot.exe".to_owned(), "c/boot".to_owned())
        );
        assert_eq!(
            parse_add("logo.iff=art/").unwrap(),
            ("logo.iff".to_owned(), "art/logo.iff".to_owned())
        );
        assert!(parse_add("=dest").is_err());
        assert!(parse_add("host=").is_err());
    }

    #[test]
    fn default_label_capitalises_the_stem() {
        assert_eq!(default_label("game.adf"), "Game");
        assert_eq!(default_label("path/to/cool.adf"), "Cool");
    }

    #[test]
    fn assemble_builds_a_verifiable_disk() {
        let img = assemble_volume(
            "MyGame",
            FileSystem::Ofs,
            true,
            &["data".to_owned()],
            &[
                ("mygame".to_owned(), vec![0u8; 1024]),
                ("readme".to_owned(), b"hi".to_vec()),
            ],
            Some("mygame"),
        )
        .unwrap();
        let disk = Disk::open(&img).unwrap();
        disk.verify().unwrap();
        let names: Vec<String> = disk.list("").unwrap().into_iter().map(|e| e.name).collect();
        assert!(names.contains(&"mygame".to_owned()));
        assert!(names.contains(&"readme".to_owned()));
        assert!(names.contains(&"data".to_owned()));
        assert!(names.contains(&"s".to_owned())); // from --startup
        assert_eq!(disk.read("s/startup-sequence").unwrap(), b"mygame\n");
    }

    #[test]
    fn assemble_empty_disk_is_valid() {
        let img = assemble_volume("Blank", FileSystem::Ofs, false, &[], &[], None).unwrap();
        let disk = Disk::open(&img).unwrap();
        disk.verify().unwrap();
        assert_eq!(disk.label(), "Blank");
        assert!(disk.list("").unwrap().is_empty());
    }

    #[test]
    fn assemble_is_deterministic() {
        let build = || {
            assemble_volume(
                "D",
                FileSystem::Ffs,
                false,
                &["c".to_owned()],
                &[("x".to_owned(), vec![1, 2, 3])],
                None,
            )
            .unwrap()
        };
        assert_eq!(build(), build());
    }

    #[test]
    fn create_needs_an_output_path() {
        assert_eq!(run(&["create".to_owned()]), ExitCode::from(2));
    }

    #[test]
    fn create_output_shapes() {
        assert_eq!(
            create_json("game.adf", 901120, FileSystem::Ofs, "Game", true, 3, 2),
            "{\"tool\":\"adf\",\"command\":\"create\",\"output\":\"game.adf\",\"filesystem\":\"ofs\",\"label\":\"Game\",\"bootable\":true,\"files\":3,\"dirs\":2,\"bytes\":901120}"
        );
        assert_eq!(
            create_text("game.adf", 901120, FileSystem::Ofs, "Game", true, 3, 2),
            "game.adf: created — 880K OFS \"Game\", 3 files, 2 dirs, bootable"
        );
        assert_eq!(
            create_text("d.adf", 901120, FileSystem::Ffs, "D", false, 1, 0),
            "d.adf: created — 880K FFS \"D\", 1 file, 0 dirs"
        );
    }

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

    #[test]
    fn read_verbs_need_a_disk() {
        assert_eq!(run(&["verify".to_owned()]), ExitCode::from(2));
        assert_eq!(run(&["info".to_owned()]), ExitCode::from(2));
    }

    #[test]
    fn explicit_master_with_no_exe_is_rejected() {
        assert_eq!(run(&["master".to_owned()]), ExitCode::from(2));
    }

    #[test]
    fn bad_format_value_is_a_usage_error() {
        assert_eq!(
            run(&[
                "verify".to_owned(),
                "d.adf".to_owned(),
                "--format".to_owned(),
                "yaml".to_owned()
            ]),
            ExitCode::from(2)
        );
    }

    #[test]
    fn verify_accepts_a_good_disk() {
        assert_eq!(
            run_verify(&good_adf(), "good.adf", Format::Text),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_verify(&good_adf(), "good.adf", Format::Json),
            ExitCode::SUCCESS
        );
    }

    #[test]
    fn verify_rejects_a_corrupt_disk() {
        let mut img = good_adf();
        img[500] ^= 0xff; // flip a boot-block byte -> boot checksum fails
        assert_eq!(run_verify(&img, "bad.adf", Format::Text), ExitCode::from(1));
    }

    #[test]
    fn verify_rejects_a_non_adf() {
        assert_eq!(
            run_verify(&[0u8; 16], "nope.bin", Format::Text),
            ExitCode::from(1)
        );
    }

    #[test]
    fn info_reads_a_good_disk() {
        assert_eq!(
            run_info(&good_adf(), "good.adf", Format::Text),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_info(&good_adf(), "good.adf", Format::Json),
            ExitCode::SUCCESS
        );
    }

    #[test]
    fn entries_sort_by_name() {
        let e = |name: &str| Entry {
            name: name.to_owned(),
            kind: EntryKind::File,
            size: 0,
        };
        let sorted = sorted_entries(vec![e("s"), e("c"), e("mygame")]);
        let names: Vec<&str> = sorted.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, ["c", "mygame", "s"]);
    }

    #[test]
    fn info_json_shape() {
        let entries = vec![
            Entry {
                name: "c".to_owned(),
                kind: EntryKind::Directory,
                size: 0,
            },
            Entry {
                name: "mygame".to_owned(),
                kind: EntryKind::File,
                size: 51234,
            },
        ];
        let json = info_json("d.adf", 901120, FileSystem::Ofs, "MyGame", &entries);
        assert_eq!(
            json,
            "{\"tool\":\"adf\",\"command\":\"info\",\"input\":\"d.adf\",\"filesystem\":\"ofs\",\"label\":\"MyGame\",\"bytes\":901120,\"entries\":[{\"name\":\"c\",\"kind\":\"dir\",\"size\":0},{\"name\":\"mygame\",\"kind\":\"file\",\"size\":51234}]}"
        );
    }

    #[test]
    fn info_text_lists_and_labels() {
        let entries = vec![
            Entry {
                name: "c".to_owned(),
                kind: EntryKind::Directory,
                size: 0,
            },
            Entry {
                name: "mygame".to_owned(),
                kind: EntryKind::File,
                size: 51234,
            },
        ];
        let text = info_text("d.adf", 901120, FileSystem::Ofs, "MyGame", &entries);
        assert!(text.starts_with("d.adf: 880K OFS \"MyGame\""));
        assert!(text.contains("mygame"));
        assert!(text.contains("51234"));
        assert!(text.contains("dir"));
    }

    #[test]
    fn verify_json_shape() {
        assert_eq!(
            verify_json("d.adf", FileSystem::Ffs, "MyGame"),
            "{\"tool\":\"adf\",\"command\":\"verify\",\"input\":\"d.adf\",\"filesystem\":\"ffs\",\"label\":\"MyGame\",\"result\":\"ok\"}"
        );
    }

    #[test]
    fn master_text_is_human_readable() {
        let line = master_text(
            "flock.adf",
            901120,
            "Flock",
            "flock",
            FileSystem::Ofs,
            51234,
        );
        assert_eq!(
            line,
            "flock.adf: 880K OFS disk \"Flock\", flock (51234 bytes)"
        );
    }

    #[test]
    fn master_json_is_unchanged() {
        let line = master_json(
            "flock.adf",
            901120,
            "Flock",
            "flock",
            FileSystem::Ofs,
            51234,
        );
        assert_eq!(
            line,
            "{\"tool\":\"adf\",\"output\":\"flock.adf\",\"volume\":\"Flock\",\"file\":\"flock\",\"filesystem\":\"ofs\",\"bytes\":901120,\"exe_bytes\":51234}"
        );
    }
}
