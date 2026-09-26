//! `build198x basic`: a numbered BASIC listing to the file its machine loads
//! (opened by `decisions/demand-gate-basic.md`).
//!
//! - **ZX Spectrum:** a ROM header and data block pair (TAP), auto-running
//!   from the program's first line unless asked not to.
//! - **Commodore 64:** a PRG at `$0801`, which the tokeniser already emits.
//!
//! The tokenisers are the Format198x dialect crates; this module only picks
//! the dialect and the container for a machine. [`Machine`] is the one list
//! a new machine is added to.

use format198x_sinclair_zx_spectrum_tap::{Header, HeaderKind, TapBlock, encode};

/// The auto-start line the Spectrum ROM reads as "do not run": any value of
/// 32768 or more.
const NO_AUTORUN: u16 = 32768;

/// A machine the verb builds for. Each has one BASIC and one container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Machine {
    /// 48K Spectrum BASIC, written as a `.tap`.
    SinclairZxSpectrum,
    /// Commodore BASIC V2, written as a `.prg`.
    CommodoreC64,
}

impl Machine {
    /// The machine a mediaspec198x id names, or `None` for an id this verb
    /// does not build for.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "sinclair-zx-spectrum" => Some(Self::SinclairZxSpectrum),
            "commodore-c64" => Some(Self::CommodoreC64),
            _ => None,
        }
    }

    /// The machine's mediaspec198x id.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::SinclairZxSpectrum => "sinclair-zx-spectrum",
            Self::CommodoreC64 => "commodore-c64",
        }
    }

    /// The extension of the file the machine loads, without the dot.
    #[must_use]
    pub fn extension(self) -> &'static str {
        match self {
            Self::SinclairZxSpectrum => "tap",
            Self::CommodoreC64 => "prg",
        }
    }
}

/// A built program, ready to write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Built {
    /// The output file's bytes.
    pub bytes: Vec<u8>,
    /// Numbered lines in the listing (blank lines are not counted).
    pub lines: usize,
    /// The tokenised program's length, without any container framing (the
    /// C64 load address, the TAP header and block framing).
    pub program_length: usize,
    /// The line the program runs from when loaded, or `None` when it does not
    /// run by itself.
    pub autorun: Option<u16>,
}

/// Why a listing could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    /// 1-based source line, or 0 when the error is not about one line.
    pub line: usize,
    /// What is wrong, without the line number.
    pub message: String,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.line == 0 {
            f.write_str(&self.message)
        } else {
            write!(f, "line {}: {}", self.line, self.message)
        }
    }
}

impl std::error::Error for Error {}

impl From<format198x_sinclair_zx_spectrum_bas::ListingError> for Error {
    fn from(e: format198x_sinclair_zx_spectrum_bas::ListingError) -> Self {
        Self {
            line: e.line,
            message: e.message,
        }
    }
}

impl From<format198x_commodore_c64_bas::ListingError> for Error {
    fn from(e: format198x_commodore_c64_bas::ListingError) -> Self {
        Self {
            line: e.line,
            message: e.message,
        }
    }
}

/// Tokenise one listing and package it for `machine`. `name` is the tape
/// header name (Spectrum; passed through [`tape_name`]). `autorun`
/// makes a Spectrum tape run from the program's first line once loaded.
///
/// # Errors
/// Returns the tokeniser's error, which names the source line when there is
/// one.
pub fn build(machine: Machine, source: &str, name: &str, autorun: bool) -> Result<Built, Error> {
    let lines = source.lines().filter(|l| !l.trim().is_empty()).count();
    match machine {
        Machine::SinclairZxSpectrum => {
            let program = format198x_sinclair_zx_spectrum_bas::tokenise_listing(source)?;
            // The tokeniser caps a program at 36 KiB, so this cannot fail;
            // it is checked rather than cast all the same.
            let length = u16::try_from(program.bytes.len()).map_err(|_| Error {
                line: 0,
                message: "BASIC program too large for a tape header".to_owned(),
            })?;
            // Each stored line starts with its number, big-endian; the
            // tokeniser refuses an empty program, so the first line exists.
            let first = match program.bytes.as_slice() {
                [hi, lo, ..] => u16::from_be_bytes([*hi, *lo]),
                _ => {
                    return Err(Error {
                        line: 0,
                        message: "Enter at least one numbered BASIC line".to_owned(),
                    });
                }
            };
            let start = if autorun { first } else { NO_AUTORUN };
            let bytes = encode(&[
                Header::new(HeaderKind::Program, &tape_name(name), length, start, length).block(),
                TapBlock::data(program.bytes),
            ]);
            Ok(Built {
                bytes,
                lines,
                program_length: usize::from(length),
                autorun: autorun.then_some(first),
            })
        }
        Machine::CommodoreC64 => {
            let program = format198x_commodore_c64_bas::tokenise(source)?;
            let program_length = program.bytes.len().saturating_sub(2);
            Ok(Built {
                bytes: program.bytes,
                lines,
                program_length,
                autorun: None,
            })
        }
    }
}

/// A name the Spectrum's ten-byte tape header can hold and the ROM prints as
/// written: its first ten characters, each outside printable ASCII
/// (`0x20..=0x7E`) replaced by `?`. The ROM prints header bytes from its own
/// character set, where bytes from `0x80` are block graphics, UDGs and
/// keywords and bytes below `0x20` are control codes, so a UTF-8 byte would
/// show as neither the character nor a clean truncation.
#[must_use]
pub fn tape_name(name: &str) -> String {
    name.chars()
        .take(10)
        .map(|c| if (' '..='~').contains(&c) { c } else { '?' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn machine_ids_round_trip() {
        for m in [Machine::SinclairZxSpectrum, Machine::CommodoreC64] {
            assert_eq!(Machine::from_id(m.id()), Some(m));
        }
        assert_eq!(Machine::from_id("bbc-micro"), None);
    }

    #[test]
    fn spectrum_autorun_is_the_first_stored_line_not_the_first_written() {
        let built = build(
            Machine::SinclairZxSpectrum,
            "  20 STOP\n  10 GO TO 20\n",
            "t",
            true,
        )
        .expect("builds");
        assert_eq!(built.autorun, Some(10));
        assert_eq!(built.lines, 2);
    }

    #[test]
    fn tape_names_are_ten_printable_ascii_characters() {
        assert_eq!(tape_name("hello"), "hello");
        assert_eq!(tape_name("shadowkeep-unit-01"), "shadowkeep");
        assert_eq!(tape_name("caf\u{e9}"), "caf?");
        assert_eq!(tape_name("\u{65e5}\u{672c}\tx"), "???x");
        assert_eq!(
            tape_name("\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}"),
            "??????????"
        );
    }

    #[test]
    fn errors_keep_the_source_line() {
        let err = build(Machine::CommodoreC64, "10 PRINT\nX\n", "t", true).expect_err("fails");
        assert_eq!(err.line, 2);
        let err = build(Machine::SinclairZxSpectrum, "\n", "t", true).expect_err("fails");
        assert_eq!(err.line, 0);
    }
}
