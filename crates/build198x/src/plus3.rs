//! Compose an authored BASIC loader and assembled CODE onto a standard +3 disk.
//! Byte layouts belong to Format198x; this module selects files and metadata.

use crate::basic::{Error, Machine, lint};
use format198x_amstrad_cpc_dsk::plus3::{self, File};

/// A mastered disk and its program metadata.
#[derive(Debug)]
pub struct Built {
    /// Standard CPCEMU DSK bytes, containing `DISK` and `DIAG.BIN`.
    pub bytes: Vec<u8>,
    /// First stored BASIC line, used for autorun.
    pub autorun: u16,
    /// Tokenised BASIC bytes, excluding the +3DOS header.
    pub basic_length: usize,
}

/// Master a fresh 180 KiB, single-sided disk with no executable boot sector.
///
/// The authored loader is responsible for loading `DIAG.BIN` and calling its
/// entry address. This function does not rewrite its statements.
///
/// # Errors
/// Rejects lint/tokenisation errors, empty CODE, ROM placement and overflow.
pub fn build(source: &str, code: &[u8], load_address: u16) -> Result<Built, Error> {
    let error = |message: String| Error { line: 0, message };
    let findings = lint::check(Machine::SinclairZxSpectrum, source)?;
    if let Some(first) = findings.first() {
        return Err(Error {
            line: first.line,
            message: format!("{}: {}", first.rule, first.message),
        });
    }
    let program = format198x_sinclair_zx_spectrum_bas::tokenise_listing(source)?;
    let autorun = match program.bytes.as_slice() {
        [hi, lo, ..] => u16::from_be_bytes([*hi, *lo]),
        _ => return Err(error("BASIC loader is empty".into())),
    };
    let loader = plus3::basic(&program.bytes, autorun).map_err(error)?;
    let code = plus3::code(code, load_address).map_err(error)?;
    let bytes = plus3::master(&[
        File {
            name: "DISK",
            bytes: &loader,
        },
        File {
            name: "DIAG.BIN",
            bytes: &code,
        },
    ])
    .map_err(error)?;
    Ok(Built {
        bytes,
        autorun,
        basic_length: program.bytes.len(),
    })
}
