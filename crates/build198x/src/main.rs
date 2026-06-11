//! The `build198x` CLI — the agent-native front-end over the image
//! conversion pipeline: `build198x image <inputs...> --machine <id>
//! --format <f> [options]` drives [`build198x::convert::pipeline`] and the
//! [`build198x::format`] codecs, and emits a machine-readable JSON report
//! (stdout by default, `--report <path>` for a file).
//!
//! Dependency stance follows the Asm198x CLI precedent exactly: hand-rolled
//! argument parsing and a hand-rolled JSON emitter — no clap, no serde
//! (Asm198x uses neither, and the report surface here is a small flat
//! object with known keys, so fixed key order falls out for free).
//!
//! # Exit codes
//!
//! | code | meaning |
//! |------|---------|
//! | 0 | success — every input converted |
//! | 2 | usage / argument error (usage on stderr, no report) |
//! | 3 | input decode failure (all inputs failed to read or decode) |
//! | 4 | conversion / constraint failure |
//! | 5 | output IO failure, including refusing to overwrite without `--force` |
//! | 6 | partial batch failure (some inputs succeeded, some failed) |
//!
//! Single-input runs use the specific code. A batch where every input
//! failed uses 3 when every failure was a decode failure, otherwise 5 if
//! any failure was IO, otherwise 4; a mixed batch uses 6.
//!
//! # Behaviour notes
//!
//! - Outputs are written **atomically**: bytes go to a temp file in the
//!   destination directory, then rename — a failed run never leaves a
//!   truncated output.
//! - Existing outputs are **never overwritten without `--force`**, and the
//!   check happens before any conversion work.
//! - Animated GIFs convert their **first frame** (the `image` crate's
//!   default frame for non-animated decode) with a warning in the report.
//!
//! # JSON report schema
//!
//! Top-level keys are **always present**, in fixed order:
//! `converter_version`, `mediaspec_version`, `machine`, `mode`, `format`,
//! `palette`, `options`, `files`, `summary`.
//!
//! - `palette` reflects the **first successful conversion**: named palettes
//!   echo its interpretation name and colours (falling back to the spec's
//!   pinned default when every input failed); generated (gamut) palettes
//!   echo the first successful conversion's colours, empty when none
//!   succeeded.
//!
//! Per entry in `files`:
//!
//! - **Always present**: `input`, `status` (`"ok"`/`"error"`), `outputs`
//!   (paths actually written, possibly empty), `warnings` (always present
//!   but may be empty).
//! - **Conditional**: `error` (`kind` + `message`) appears only on failure
//!   — absent on success; `quality` (`mean_error`,
//!   `cells_over_threshold`) appears only once conversion ran — absent
//!   when the entry failed before the pipeline stage (no-clobber check,
//!   read, or decode).

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, Ordering};

use build198x::convert::colour::Metric;
use build198x::convert::dither::DitherMode;
use build198x::convert::pipeline::{Conversion, Options, convert};
use build198x::format::{art_studio, ilbm, koala, scr};
use mediaspec::{ConstraintRule, PaletteModel, Rgb};

/// Monotonic counter making temp-file names unique within the process.
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.split_first() {
        None => {
            eprintln!("{}", top_usage());
            ExitCode::from(2)
        }
        Some((cmd, rest)) => match cmd.as_str() {
            "image" => image_command(rest),
            "--version" | "-V" => {
                println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
                ExitCode::SUCCESS
            }
            "--help" | "-h" | "help" => {
                println!("{}", top_usage());
                ExitCode::SUCCESS
            }
            other => {
                eprintln!("build198x: unknown command `{other}`\n\n{}", top_usage());
                ExitCode::from(2)
            }
        },
    }
}

/// The output format requested with `--format`. Each format pins one
/// machine, and all but ILBM pin one screen mode.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Format {
    /// Spectrum screen dump (`sinclair-zx-spectrum`, mode `standard`).
    Scr,
    /// C64 Koala Painter (`commodore-c64`, mode `multicolour-bitmap`).
    Koala,
    /// C64 OCP Art Studio (`commodore-c64`, mode `hires-bitmap`).
    ArtStudio,
    /// Amiga IFF/ILBM (`commodore-amiga-ocs`, mode from `--mode`).
    Ilbm,
}

impl Format {
    fn parse(token: &str) -> Option<Self> {
        match token {
            "scr" => Some(Self::Scr),
            "koala" => Some(Self::Koala),
            "art-studio" => Some(Self::ArtStudio),
            "ilbm" => Some(Self::Ilbm),
            _ => None,
        }
    }

    fn token(self) -> &'static str {
        match self {
            Self::Scr => "scr",
            Self::Koala => "koala",
            Self::ArtStudio => "art-studio",
            Self::Ilbm => "ilbm",
        }
    }

    /// The machine id this format belongs to.
    fn machine_id(self) -> &'static str {
        match self {
            Self::Scr => "sinclair-zx-spectrum",
            Self::Koala | Self::ArtStudio => "commodore-c64",
            Self::Ilbm => "commodore-amiga-ocs",
        }
    }

    /// The screen mode this format implies (`None` for ILBM, which takes
    /// `--mode`).
    fn implied_mode(self) -> Option<&'static str> {
        match self {
            Self::Scr => Some("standard"),
            Self::Koala => Some("multicolour-bitmap"),
            Self::ArtStudio => Some("hires-bitmap"),
            Self::Ilbm => None,
        }
    }

    /// Default output file extension.
    fn extension(self) -> &'static str {
        match self {
            Self::Scr => "scr",
            Self::Koala => "koa",
            Self::ArtStudio => "art",
            Self::Ilbm => "iff",
        }
    }
}

/// Fully validated `image` subcommand arguments.
struct ImageArgs {
    inputs: Vec<String>,
    machine: String,
    format: Format,
    mode: String,
    /// Palette interpretation name (fixed-palette machines only).
    palette: Option<String>,
    metric: Metric,
    dither: DitherMode,
    no_dither: bool,
    strength: u8,
    matte: [u8; 3],
    exhaustive_background: bool,
    output: Option<String>,
    preview: Option<String>,
    force: bool,
    report: Option<String>,
}

/// What `parse_image_args` resolved to.
enum ImageParse {
    /// `-h`/`--help` was present.
    Help,
    /// A validated run.
    Run(Box<ImageArgs>),
}

fn image_command(args: &[String]) -> ExitCode {
    match parse_image_args(args) {
        Ok(ImageParse::Help) => {
            println!("{}", image_usage());
            ExitCode::SUCCESS
        }
        Ok(ImageParse::Run(parsed)) => run_image(&parsed),
        Err(msg) => {
            eprintln!("build198x image: {msg}\n\n{}", image_usage());
            ExitCode::from(2)
        }
    }
}

/// Fetch the value following a flag.
fn flag_value(args: &[String], i: usize, flag: &str) -> Result<String, String> {
    args.get(i)
        .cloned()
        .ok_or_else(|| format!("`{flag}` needs a value"))
}

#[allow(clippy::too_many_lines)] // One linear validation pass; splitting it hides the flow.
fn parse_image_args(args: &[String]) -> Result<ImageParse, String> {
    if args.iter().any(|a| a == "-h" || a == "--help") {
        return Ok(ImageParse::Help);
    }

    let mut inputs: Vec<String> = Vec::new();
    let mut machine: Option<String> = None;
    let mut format_token: Option<String> = None;
    let mut mode_arg: Option<String> = None;
    let mut palette: Option<String> = None;
    let mut metric_arg: Option<String> = None;
    let mut dither_arg: Option<String> = None;
    let mut strength_arg: Option<String> = None;
    let mut matte_arg: Option<String> = None;
    let mut exhaustive_background = false;
    let mut output: Option<String> = None;
    let mut preview: Option<String> = None;
    let mut force = false;
    let mut report: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--machine" => {
                i += 1;
                machine = Some(flag_value(args, i, "--machine")?);
            }
            "--format" => {
                i += 1;
                format_token = Some(flag_value(args, i, "--format")?);
            }
            "--mode" => {
                i += 1;
                mode_arg = Some(flag_value(args, i, "--mode")?);
            }
            "--palette" => {
                i += 1;
                palette = Some(flag_value(args, i, "--palette")?);
            }
            "--metric" => {
                i += 1;
                metric_arg = Some(flag_value(args, i, "--metric")?);
            }
            "--dither" => {
                i += 1;
                dither_arg = Some(flag_value(args, i, "--dither")?);
            }
            "--dither-strength" => {
                i += 1;
                strength_arg = Some(flag_value(args, i, "--dither-strength")?);
            }
            "--matte" => {
                i += 1;
                matte_arg = Some(flag_value(args, i, "--matte")?);
            }
            "--exhaustive-background" => exhaustive_background = true,
            "-o" | "--output" => {
                i += 1;
                output = Some(flag_value(args, i, "--output")?);
            }
            "--preview" => {
                i += 1;
                preview = Some(flag_value(args, i, "--preview")?);
            }
            "--force" => force = true,
            "--report" => {
                i += 1;
                report = Some(flag_value(args, i, "--report")?);
            }
            flag if flag.starts_with('-') && flag.len() > 1 => {
                return Err(format!("unknown flag `{flag}`"));
            }
            path => inputs.push(path.to_owned()),
        }
        i += 1;
    }

    if inputs.is_empty() {
        return Err("no input files given".to_owned());
    }

    let machine = machine.ok_or("`--machine` is required")?;
    let machine_ids: Vec<&str> = mediaspec::machines().iter().map(|m| m.id).collect();
    let spec = mediaspec::machine(&machine)
        .ok_or_else(|| format!("unknown machine `{machine}` ({})", machine_ids.join(", ")))?;

    let format_token = format_token.ok_or("`--format` is required")?;
    let format = Format::parse(&format_token)
        .ok_or_else(|| format!("unknown format `{format_token}` (scr, koala, art-studio, ilbm)"))?;

    if format.machine_id() != spec.id {
        return Err(format!(
            "format `{}` targets machine `{}`, not `{}`",
            format.token(),
            format.machine_id(),
            spec.id
        ));
    }

    let mode = match (format.implied_mode(), mode_arg) {
        (None, None) => "lores-pal".to_owned(),
        (None, Some(m)) => m,
        (Some(implied), None) => implied.to_owned(),
        (Some(implied), Some(_)) => {
            return Err(format!(
                "--mode applies to --format ilbm only (format `{}` implies mode `{implied}`)",
                format.token()
            ));
        }
    };
    let mode_spec = spec.mode(&mode).ok_or_else(|| {
        let names: Vec<&str> = spec.modes.iter().map(|md| md.name).collect();
        format!(
            "unknown mode `{mode}` for {} ({})",
            spec.id,
            names.join(", ")
        )
    })?;

    if let Some(name) = &palette {
        // PaletteModel is non_exhaustive; only Fixed palettes have named
        // interpretations, so anything else (Gamut today) rejects --palette.
        if let PaletteModel::Fixed(list) = spec.palette {
            if !list.iter().any(|p| p.name == name) {
                let names: Vec<&str> = list.iter().map(|p| p.name).collect();
                return Err(format!(
                    "unknown palette interpretation `{name}` for {} ({})",
                    spec.id,
                    names.join(", ")
                ));
            }
        } else {
            return Err(format!(
                "--palette is not available for `{}`: its palette is a generated gamut, not a named interpretation",
                spec.id
            ));
        }
    }

    let metric = match metric_arg.as_deref() {
        None | Some("oklab") => Metric::OkLab,
        Some("weighted-rgb") => Metric::WeightedRgb,
        Some("yuv") => Metric::Yuv,
        Some(other) => {
            return Err(format!(
                "unknown metric `{other}` (oklab, weighted-rgb, yuv)"
            ));
        }
    };

    let (dither, no_dither) = match dither_arg.as_deref() {
        None | Some("ordered8") => (DitherMode::Bayer8, false),
        Some("ordered4") => (DitherMode::Bayer4, false),
        Some("fs") => (DitherMode::FloydSteinberg, false),
        Some("atkinson") => (DitherMode::Atkinson, false),
        Some("none") => (DitherMode::Bayer8, true),
        Some(other) => {
            return Err(format!(
                "unknown dither `{other}` (ordered4, ordered8, fs, atkinson, none)"
            ));
        }
    };
    // Error diffusion needs a free-palette target: keyed on the resolved
    // mode's constraint rule, not the format token.
    if !dither.is_ordered() && !matches!(mode_spec.constraint, ConstraintRule::Planar { .. }) {
        return Err(format!(
            "--dither {} needs a free-palette (planar) target; cell-constrained formats take ordered4, ordered8, or none",
            dither_token(dither, no_dither)
        ));
    }

    let strength = match strength_arg {
        None => 32,
        Some(s) => s
            .parse::<u8>()
            .ok()
            .filter(|v| *v <= 64)
            .ok_or_else(|| format!("--dither-strength must be an integer 0..=64, got `{s}`"))?,
    };

    let matte = match matte_arg {
        None => [0, 0, 0],
        Some(s) => parse_matte(&s)?,
    };

    // Exhaustive background search exists only where a global background
    // does: keyed on the resolved mode's constraint rule.
    if exhaustive_background && mode_spec.constraint != ConstraintRule::C64Multicolour {
        return Err(
            "--exhaustive-background applies to C64 multicolour (--format koala) only".to_owned(),
        );
    }

    if inputs.len() > 1 {
        if output.is_some() {
            return Err("`-o/--output` takes a single input (batch outputs default to <stem>.<ext> in the cwd)".to_owned());
        }
        if preview.is_some() {
            return Err("`--preview` takes a single input".to_owned());
        }
    }

    Ok(ImageParse::Run(Box::new(ImageArgs {
        inputs,
        machine,
        format,
        mode,
        palette,
        metric,
        dither,
        no_dither,
        strength,
        matte,
        exhaustive_background,
        output,
        preview,
        force,
        report,
    })))
}

/// The CLI token for a metric (the report's `options.metric` echoes it).
fn metric_token(metric: Metric) -> &'static str {
    match metric {
        Metric::OkLab => "oklab",
        Metric::WeightedRgb => "weighted-rgb",
        Metric::Yuv => "yuv",
    }
}

/// The CLI token for a dither selection (the report's `options.dither`
/// echoes it).
fn dither_token(dither: DitherMode, no_dither: bool) -> &'static str {
    if no_dither {
        return "none";
    }
    match dither {
        DitherMode::Bayer4 => "ordered4",
        DitherMode::Bayer8 => "ordered8",
        DitherMode::FloydSteinberg => "fs",
        DitherMode::Atkinson => "atkinson",
    }
}

/// Parse `rrggbb` (optional leading `#`) into an RGB triple.
fn parse_matte(s: &str) -> Result<[u8; 3], String> {
    let hex = s.strip_prefix('#').unwrap_or(s);
    let err = || format!("--matte must be six hex digits (rrggbb), got `{s}`");
    if hex.len() != 6 || !hex.is_ascii() {
        return Err(err());
    }
    let channel = |range: core::ops::Range<usize>| {
        hex.get(range)
            .and_then(|pair| u8::from_str_radix(pair, 16).ok())
    };
    match (channel(0..2), channel(2..4), channel(4..6)) {
        (Some(r), Some(g), Some(b)) => Ok([r, g, b]),
        _ => Err(err()),
    }
}

/// One input's outcome, as it appears in the report's `files` array.
struct FileEntry {
    input: String,
    ok: bool,
    /// Paths actually written, in write order (native, then preview).
    outputs: Vec<String>,
    /// `(kind, message)`: kind is `decode`, `convert`, or `io`.
    error: Option<(&'static str, String)>,
    warnings: Vec<String>,
    /// `(mean_error, cells_over_threshold)`, present once conversion ran.
    quality: Option<(f32, usize)>,
}

impl FileEntry {
    fn new(input: &str) -> Self {
        Self {
            input: input.to_owned(),
            ok: false,
            outputs: Vec::new(),
            error: None,
            warnings: Vec::new(),
            quality: None,
        }
    }
}

/// The report's top-level `palette` object.
enum PaletteSection {
    Named { name: String, colours: Vec<Rgb> },
    Generated { gamut_bits: u8, colours: Vec<Rgb> },
}

/// A conversion's resolved palette: the interpretation name (`None` for
/// generated gamut palettes) and the colours.
type ResolvedPalette = (Option<String>, Vec<Rgb>);

fn run_image(a: &ImageArgs) -> ExitCode {
    let mut entries: Vec<FileEntry> = Vec::with_capacity(a.inputs.len());
    let mut first_palette: Option<ResolvedPalette> = None;

    for input in &a.inputs {
        let native = a
            .output
            .clone()
            .map_or_else(|| default_output(input, a.format), PathBuf::from);
        let preview = a.preview.clone().map(PathBuf::from);
        let (entry, palette) = process_file(input, &native, preview.as_deref(), a);
        if first_palette.is_none() {
            first_palette = palette;
        }
        entries.push(entry);
    }

    let palette_section = palette_section(a, first_palette);
    let report = render_report(a, &palette_section, &entries);

    let ok = entries.iter().filter(|e| e.ok).count();
    eprintln!("build198x image: {ok} ok, {} failed", entries.len() - ok);

    if let Some(path) = &a.report {
        if let Err(msg) = write_atomic(Path::new(path), report.as_bytes()) {
            eprintln!("build198x image: report: {msg}");
            return ExitCode::from(5);
        }
    } else {
        print!("{report}");
    }

    ExitCode::from(exit_code(&entries))
}

/// Default output path: input stem + the format's extension, in the cwd.
fn default_output(input: &str, format: Format) -> PathBuf {
    let stem = Path::new(input)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "out".to_owned());
    PathBuf::from(format!("{stem}.{}", format.extension()))
}

/// Map the per-file outcomes to the documented exit codes (see the module
/// doc): 0 all ok; 6 mixed; all-failed picks 3 (all decode), else 5 (any
/// IO), else 4.
fn exit_code(entries: &[FileEntry]) -> u8 {
    let ok = entries.iter().filter(|e| e.ok).count();
    let failed = entries.len() - ok;
    if failed == 0 {
        return 0;
    }
    if ok > 0 {
        return 6;
    }
    let kinds: Vec<&str> = entries
        .iter()
        .filter_map(|e| e.error.as_ref().map(|(kind, _)| *kind))
        .collect();
    if !kinds.is_empty() && kinds.iter().all(|k| *k == "decode") {
        3
    } else if kinds.contains(&"io") {
        5
    } else {
        4
    }
}

/// Convert one input end to end. Returns the report entry plus the
/// conversion's resolved palette — its interpretation name (`None` for
/// generated) and colours — for the report's palette section.
fn process_file(
    input: &str,
    native_out: &Path,
    preview_out: Option<&Path>,
    a: &ImageArgs,
) -> (FileEntry, Option<ResolvedPalette>) {
    let mut entry = FileEntry::new(input);

    // No-clobber check first, before any conversion work.
    if !a.force {
        let mut targets: Vec<&Path> = vec![native_out];
        if let Some(p) = preview_out {
            targets.push(p);
        }
        for target in targets {
            if target.exists() {
                entry.error = Some((
                    "io",
                    format!(
                        "output {} exists (pass --force to overwrite)",
                        target.display()
                    ),
                ));
                return (entry, None);
            }
        }
    }

    let bytes = match std::fs::read(input) {
        Ok(b) => b,
        Err(e) => {
            entry.error = Some(("decode", format!("cannot read {input}: {e}")));
            return (entry, None);
        }
    };
    let img = match image::load_from_memory(&bytes) {
        Ok(i) => i,
        Err(e) => {
            entry.error = Some(("decode", format!("cannot decode {input}: {e}")));
            return (entry, None);
        }
    };
    if gif_is_animated(&bytes) {
        entry
            .warnings
            .push("animated input: first frame used".to_owned());
    }

    let opts = Options {
        machine: a.machine.clone(),
        mode: a.mode.clone(),
        interpretation: a.palette.clone(),
        metric: a.metric,
        dither: a.dither,
        strength: a.strength,
        matte: a.matte,
        exhaustive_background: a.exhaustive_background,
        no_dither: a.no_dither,
    };
    let conv = match convert(&img, &opts) {
        Ok(c) => c,
        Err(e) => {
            entry.error = Some(("convert", e.to_string()));
            return (entry, None);
        }
    };
    if conv.report.already_constrained {
        entry
            .warnings
            .push("input appears already constrained".to_owned());
    }
    entry.quality = Some((conv.report.mean_error, conv.report.cells_over_threshold));
    let palette = Some((conv.interpretation.clone(), conv.palette.clone()));

    let native_bytes = match encode_native(&conv, a.format) {
        Ok(b) => b,
        Err(msg) => {
            entry.error = Some(("convert", msg));
            return (entry, palette);
        }
    };
    if let Err(msg) = write_atomic(native_out, &native_bytes) {
        entry.error = Some(("io", msg));
        return (entry, palette);
    }
    entry.outputs.push(native_out.display().to_string());

    if let Some(preview_path) = preview_out {
        let png = match render_preview_png(&conv) {
            Ok(p) => p,
            Err(msg) => {
                entry.error = Some(("convert", msg));
                return (entry, palette);
            }
        };
        // A preview failure after a successful native write is a partial
        // (multi-output) failure: the native path stays in `outputs`, the
        // entry carries the IO error, and the run exits 5/6.
        if let Err(msg) = write_atomic(preview_path, &png) {
            entry.error = Some(("io", msg));
            return (entry, palette);
        }
        entry.outputs.push(preview_path.display().to_string());
    }

    entry.ok = true;
    (entry, palette)
}

/// Encode the conversion into its native on-disk format.
fn encode_native(conv: &Conversion, format: Format) -> Result<Vec<u8>, String> {
    match format {
        Format::Scr => {
            scr::encode(&conv.to_scr().map_err(|e| e.to_string())?).map_err(|e| e.to_string())
        }
        Format::Koala => {
            koala::encode(&conv.to_koala().map_err(|e| e.to_string())?).map_err(|e| e.to_string())
        }
        Format::ArtStudio => art_studio::encode(&conv.to_art_studio().map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string()),
        Format::Ilbm => ilbm::encode(
            &conv.to_ilbm().map_err(|e| e.to_string())?,
            ilbm::Compression::ByteRun1,
        )
        .map_err(|e| e.to_string()),
    }
}

/// Render the converted result through its resolved palette to PNG bytes —
/// a diagnostic of this conversion, per `decisions/play198x-boundary.md`.
///
/// The preview is **PAR-corrected to display proportions**: each mode pixel
/// is duplicated by the mode's `pixel_aspect` from mediaspec — pure integer
/// duplication, no resampling. C64 multicolour (2:1) doubles each pixel
/// horizontally (160×200 mode pixels → 320×200 PNG); Amiga hires (1:2)
/// doubles each row (640×256 → 640×512); square-pixel modes emit 1:1.
fn render_preview_png(conv: &Conversion) -> Result<Vec<u8>, String> {
    let mode = mediaspec::machine(&conv.machine_id)
        .and_then(|m| m.mode(&conv.mode_name))
        .ok_or_else(|| {
            format!(
                "cannot render preview: unknown machine/mode `{}`/`{}`",
                conv.machine_id, conv.mode_name
            )
        })?;
    let sx = u32::from(mode.pixel_aspect.horizontal);
    let sy = u32::from(mode.pixel_aspect.vertical);
    let out_w = conv.width * sx;
    let out_h = conv.height * sy;

    let row_len = conv.width as usize;
    let mut rgb = Vec::with_capacity(out_w as usize * out_h as usize * 3);
    for row in conv.pixels.chunks(row_len.max(1)) {
        let mut line = Vec::with_capacity(out_w as usize * 3);
        for &idx in row {
            let c = conv
                .palette
                .get(usize::from(idx))
                .copied()
                .unwrap_or(Rgb { r: 0, g: 0, b: 0 });
            for _ in 0..sx {
                line.extend_from_slice(&[c.r, c.g, c.b]);
            }
        }
        for _ in 0..sy {
            rgb.extend_from_slice(&line);
        }
    }
    let mut out = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut out);
    image::ImageEncoder::write_image(encoder, &rgb, out_w, out_h, image::ExtendedColorType::Rgb8)
        .map_err(|e| format!("cannot encode preview PNG: {e}"))?;
    Ok(out)
}

/// Does this byte stream hold an animated (multi-frame) GIF?
fn gif_is_animated(bytes: &[u8]) -> bool {
    use image::AnimationDecoder;
    if !bytes.starts_with(b"GIF8") {
        return false;
    }
    let Ok(decoder) = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(bytes)) else {
        return false;
    };
    decoder.into_frames().take(2).filter_map(Result::ok).count() > 1
}

/// Write `bytes` to `path` atomically: temp file in the destination
/// directory, then rename. A failed run never leaves a truncated output.
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
        // The temp file may exist part-written (create succeeded, write
        // failed); remove it so a failed run leaves nothing behind.
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("cannot write {}: {e}", path.display()));
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("cannot write {}: {e}", path.display()));
    }
    Ok(())
}

/// Resolve the report's top-level palette object from the first
/// conversion's resolved palette: named palettes echo its interpretation
/// name and colours (falling back to the spec's pinned default when no
/// conversion ran); generated palettes are per image, so the colours echo
/// the first successful conversion (empty when none succeeded).
fn palette_section(a: &ImageArgs, conv: Option<ResolvedPalette>) -> PaletteSection {
    let spec = mediaspec::machine(&a.machine);
    match spec.map(|m| &m.palette) {
        Some(PaletteModel::Gamut { bits_per_gun }) => PaletteSection::Generated {
            gamut_bits: *bits_per_gun,
            colours: conv.map(|(_, colours)| colours).unwrap_or_default(),
        },
        // Fixed palettes (and unknown machines) take the named path;
        // PaletteModel is non_exhaustive, so this arm is the catch-all.
        _ => {
            if let Some((Some(name), colours)) = conv {
                return PaletteSection::Named { name, colours };
            }
            // No conversion resolved a palette (every input failed first):
            // derive the same name and colours from the spec.
            let name = a
                .palette
                .clone()
                .or_else(|| {
                    spec.and_then(|m| m.default_interpretation)
                        .map(str::to_owned)
                })
                .unwrap_or_default();
            let colours = spec
                .and_then(|m| m.interpretation(&name))
                .map(|p| p.colours.to_vec())
                .unwrap_or_default();
            PaletteSection::Named { name, colours }
        }
    }
}

// --- JSON emission (hand-rolled; keys in fixed, documented order) --------

/// Escape a string for a JSON string literal.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

/// Render a one-line JSON array of strings.
fn json_string_array(items: &[String]) -> String {
    let inner: Vec<String> = items
        .iter()
        .map(|i| format!("\"{}\"", json_escape(i)))
        .collect();
    format!("[{}]", inner.join(", "))
}

/// `rrggbb` for a palette colour.
fn hex_colour(c: Rgb) -> String {
    format!("{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

/// Render the full JSON report. Key order is fixed and golden-tested:
/// `converter_version`, `mediaspec_version`, `machine`, `mode`, `format`,
/// `palette`, `options`, `files`, `summary` — all always present.
///
/// Conditional keys (see the module docs § JSON report schema): per file,
/// `error` appears only on failure and `quality` only once conversion ran;
/// `warnings` is always present but may be empty. The top-level `palette`
/// reflects the first successful conversion (spec-default fallback for
/// named palettes, empty colours for generated ones).
fn render_report(a: &ImageArgs, palette: &PaletteSection, entries: &[FileEntry]) -> String {
    let mut s = String::with_capacity(2048);
    s.push_str("{\n");
    s.push_str(&format!(
        "  \"converter_version\": \"{}\",\n",
        json_escape(env!("CARGO_PKG_VERSION"))
    ));
    s.push_str(&format!(
        "  \"mediaspec_version\": \"{}\",\n",
        json_escape(mediaspec::VERSION)
    ));
    s.push_str(&format!(
        "  \"machine\": \"{}\",\n",
        json_escape(&a.machine)
    ));
    s.push_str(&format!("  \"mode\": \"{}\",\n", json_escape(&a.mode)));
    s.push_str(&format!("  \"format\": \"{}\",\n", a.format.token()));

    s.push_str("  \"palette\": {\n");
    match palette {
        PaletteSection::Named { name, colours } => {
            s.push_str("    \"kind\": \"named\",\n");
            s.push_str(&format!("    \"name\": \"{}\",\n", json_escape(name)));
            let hex: Vec<String> = colours.iter().map(|&c| hex_colour(c)).collect();
            s.push_str(&format!("    \"colours\": {}\n", json_string_array(&hex)));
        }
        PaletteSection::Generated {
            gamut_bits,
            colours,
        } => {
            s.push_str("    \"kind\": \"generated\",\n");
            s.push_str(&format!("    \"gamut_bits\": {gamut_bits},\n"));
            let hex: Vec<String> = colours.iter().map(|&c| hex_colour(c)).collect();
            s.push_str(&format!("    \"colours\": {}\n", json_string_array(&hex)));
        }
    }
    s.push_str("  },\n");

    s.push_str("  \"options\": {\n");
    s.push_str(&format!(
        "    \"metric\": \"{}\",\n",
        metric_token(a.metric)
    ));
    s.push_str(&format!(
        "    \"dither\": \"{}\",\n",
        dither_token(a.dither, a.no_dither)
    ));
    s.push_str(&format!("    \"strength\": {},\n", a.strength));
    s.push_str(&format!(
        "    \"matte\": \"{}\",\n",
        hex_colour(Rgb {
            r: a.matte[0],
            g: a.matte[1],
            b: a.matte[2]
        })
    ));
    s.push_str(&format!("    \"force\": {}\n", a.force));
    s.push_str("  },\n");

    s.push_str("  \"files\": [\n");
    for (i, e) in entries.iter().enumerate() {
        s.push_str("    {\n");
        s.push_str(&format!(
            "      \"input\": \"{}\",\n",
            json_escape(&e.input)
        ));
        s.push_str(&format!(
            "      \"status\": \"{}\",\n",
            if e.ok { "ok" } else { "error" }
        ));
        s.push_str(&format!(
            "      \"outputs\": {},\n",
            json_string_array(&e.outputs)
        ));
        if let Some((kind, message)) = &e.error {
            s.push_str("      \"error\": {\n");
            s.push_str(&format!("        \"kind\": \"{}\",\n", json_escape(kind)));
            s.push_str(&format!(
                "        \"message\": \"{}\"\n",
                json_escape(message)
            ));
            s.push_str("      },\n");
        }
        s.push_str(&format!(
            "      \"warnings\": {}",
            json_string_array(&e.warnings)
        ));
        if let Some((mean_error, over)) = e.quality {
            s.push_str(",\n      \"quality\": {\n");
            s.push_str(&format!("        \"mean_error\": {mean_error:.6},\n"));
            s.push_str(&format!("        \"cells_over_threshold\": {over}\n"));
            s.push_str("      }\n");
        } else {
            s.push('\n');
        }
        s.push_str(if i + 1 == entries.len() {
            "    }\n"
        } else {
            "    },\n"
        });
    }
    s.push_str("  ],\n");

    let ok = entries.iter().filter(|e| e.ok).count();
    s.push_str("  \"summary\": {\n");
    s.push_str(&format!("    \"ok\": {ok},\n"));
    s.push_str(&format!("    \"failed\": {}\n", entries.len() - ok));
    s.push_str("  }\n}\n");
    s
}

// --- usage ----------------------------------------------------------------

fn top_usage() -> String {
    format!(
        "{name} — the 198x build-tools pipeline\n\n\
         usage:\n\
         \x20 {name} image <input.png> [more inputs...] --machine <id> --format <f> [options]\n\
         \x20 {name} --version\n\
         \x20 {name} --help\n\n\
         run `{name} image --help` for the image converter's flags and exit codes.",
        name = env!("CARGO_PKG_NAME")
    )
}

fn image_usage() -> &'static str {
    "build198x image — convert images to retro native screen formats\n\
     \n\
     usage:\n\
     \x20 build198x image <input.png> [more inputs...] --machine <id> --format <f> [options]\n\
     \n\
     machine / format pairings:\n\
     \x20 sinclair-zx-spectrum   scr          (mode: standard)\n\
     \x20 commodore-c64          koala        (mode: multicolour-bitmap)\n\
     \x20 commodore-c64          art-studio   (mode: hires-bitmap)\n\
     \x20 commodore-amiga-ocs    ilbm         (--mode lores-pal|lores-ntsc|hires-pal|hires-ntsc,\n\
     \x20                                      default lores-pal)\n\
     \n\
     options:\n\
     \x20 --machine <id>             target machine (required)\n\
     \x20 --format <f>               scr | koala | art-studio | ilbm (required)\n\
     \x20 --mode <m>                 ilbm only: Amiga screen mode (default lores-pal)\n\
     \x20 --palette <name>           palette interpretation (fixed-palette machines only;\n\
     \x20                            default: the spec's pinned default, emu198x-v1)\n\
     \x20 --metric <m>               oklab | weighted-rgb | yuv (default oklab)\n\
     \x20 --dither <d>               ordered4 | ordered8 | fs | atkinson | none\n\
     \x20                            (default ordered8; fs/atkinson are ilbm-only)\n\
     \x20 --dither-strength <0..64>  ordered-dither strength (default 32)\n\
     \x20 --matte <rrggbb>           matte under alpha + letterbox colour (default 000000)\n\
     \x20 --exhaustive-background    koala only: try every background colour (16x cost)\n\
     \x20 -o, --output <path>        output path (single input only;\n\
     \x20                            default: <input stem>.<ext> in the cwd)\n\
     \x20 --preview <path.png>       also render the converted result to a PNG preview\n\
     \x20                            at display proportions (mode pixels duplicated by\n\
     \x20                            the mode's pixel aspect; single input only)\n\
     \x20 --force                    overwrite existing outputs\n\
     \x20 --report <path>            write the JSON report to a file instead of stdout\n\
     \n\
     Outputs are written atomically (temp file + rename) and never overwrite\n\
     an existing file without --force. Animated GIFs convert their first\n\
     frame (the image crate's default) with a warning in the report. Batch\n\
     runs continue past per-file errors and report per-file status.\n\
     The JSON report's schema (always-present vs conditional keys) is\n\
     documented in the CLI module docs (crates/build198x/src/main.rs).\n\
     \n\
     exit codes:\n\
     \x20 0  success\n\
     \x20 2  usage / argument error\n\
     \x20 3  input decode failure (all inputs failed to read or decode)\n\
     \x20 4  conversion / constraint failure\n\
     \x20 5  output IO failure (including refusing to overwrite without --force)\n\
     \x20 6  partial batch failure (some inputs succeeded, some failed)"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_escape_passes_plain_text_through() {
        assert_eq!(json_escape("plain text 123"), "plain text 123");
    }

    #[test]
    fn json_escape_handles_quotes_and_backslashes() {
        assert_eq!(json_escape(r#"a"b\c"#), r#"a\"b\\c"#);
    }

    #[test]
    fn json_escape_handles_control_characters() {
        assert_eq!(json_escape("a\nb\tc\rd\u{1}e"), "a\\nb\\tc\\rd\\u0001e");
    }

    #[test]
    fn json_escape_preserves_non_ascii() {
        assert_eq!(json_escape("café 198×"), "café 198×");
    }

    #[test]
    fn matte_parses_hex_with_and_without_hash() {
        assert_eq!(parse_matte("ff8001"), Ok([0xff, 0x80, 0x01]));
        assert_eq!(parse_matte("#102030"), Ok([0x10, 0x20, 0x30]));
        assert!(parse_matte("ff80").is_err());
        assert!(parse_matte("zzzzzz").is_err());
        assert!(parse_matte("ff80011").is_err());
    }

    #[test]
    fn default_output_uses_stem_and_format_extension() {
        assert_eq!(
            default_output("art/in.png", Format::Koala),
            PathBuf::from("in.koa")
        );
        assert_eq!(default_output("x.png", Format::Scr), PathBuf::from("x.scr"));
        assert_eq!(
            default_output("x.png", Format::ArtStudio),
            PathBuf::from("x.art")
        );
        assert_eq!(
            default_output("x.png", Format::Ilbm),
            PathBuf::from("x.iff")
        );
    }

    #[test]
    fn exit_codes_follow_the_documented_map() {
        let entry = |ok: bool, kind: Option<&'static str>| FileEntry {
            input: "i".to_owned(),
            ok,
            outputs: Vec::new(),
            error: kind.map(|k| (k, "m".to_owned())),
            warnings: Vec::new(),
            quality: None,
        };
        assert_eq!(exit_code(&[entry(true, None)]), 0);
        assert_eq!(exit_code(&[entry(false, Some("decode"))]), 3);
        assert_eq!(exit_code(&[entry(false, Some("convert"))]), 4);
        assert_eq!(exit_code(&[entry(false, Some("io"))]), 5);
        assert_eq!(
            exit_code(&[entry(true, None), entry(false, Some("decode"))]),
            6
        );
        // All-failed mixed kinds: IO wins over convert; decode-only is 3.
        assert_eq!(
            exit_code(&[entry(false, Some("decode")), entry(false, Some("io"))]),
            5
        );
        assert_eq!(
            exit_code(&[entry(false, Some("decode")), entry(false, Some("convert"))]),
            4
        );
    }
}
