//! `build198x basic lint`: the checks a listing must pass before it is built.
//!
//! A listing is written as the machine's LIST shows it (Code198x
//! `docs/specifications/unit.md`), and every rule here comes from a mistake
//! found in Code198x's samples. The rules read the dialect crates' positioned
//! pieces, so each finding names the column it is about.
//!
//! | Rule | Machine | Catches |
//! |---|---|---|
//! | `listing-form` | both | a line that differs from what LIST prints for it |
//! | `stored-space` | Spectrum | a space the ROM stores and lists (`LET n = 1`) |
//! | `string-var-name` | Spectrum | a string variable longer than one letter |
//! | `keyword-var-name` | Spectrum | a variable named like a keyword (`ink`) |
//! | `statement-keyword` | Spectrum | a statement that does not start with a command keyword (`GOTO 10`) |
//! | `keyword-in-name` | C64 | a keyword with name letters on both sides (`SCORE`) |
//! | `var-name-clash` | C64 | two names BASIC V2 reads as one variable |
//! | `line-order` | both | a duplicate or descending line number |
//!
//! [`fix`] mends the first two by rewriting each line to its listed form. It
//! leaves a line with a `statement-keyword` finding as written: its listed
//! form is a line the ROM refuses too, so there is nothing to mend by
//! spacing.

use std::collections::{HashMap, HashSet};

use format198x_commodore_c64_bas as c64;
use format198x_sinclair_zx_spectrum_bas as zx;

use super::{Error, Machine};

/// One problem in a listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// 1-based source line.
    pub line: usize,
    /// 1-based column within the source line.
    pub column: usize,
    /// The rule's name, such as `listing-form`.
    pub rule: &'static str,
    /// What is wrong, without the position or rule.
    pub message: String,
}

/// Spectrum keyword tokens after which the ROM reads a variable name:
/// LET, FOR, NEXT, INPUT, READ and DIM.
const ZX_NAME_TAKERS: [u8; 6] = [0xF1, 0xEB, 0xF3, 0xEE, 0xE3, 0xE9];

/// The Spectrum's THEN token, after which a new statement starts, and the
/// first command token (DEF FN). The ROM's statement loop (STMT-L-1, `1B29`)
/// subtracts `0xCE` from a statement's first code and gives report C,
/// Nonsense in BASIC, if it goes below zero; IF jumps back to STMT-L-1 after
/// THEN (IF-1, `1D00`) (Logan and O'Hara, *The Complete Spectrum ROM
/// Disassembly*, 1983).
const ZX_THEN: u8 = 0xCB;
const ZX_FIRST_COMMAND: u8 = 0xCE;

/// The C64's DATA and FN tokens.
const C64_DATA: u8 = 0x83;
const C64_FN: u8 = 0xA5;

/// Every finding for one listing, sorted by line then column.
///
/// # Errors
/// Returns the tokeniser's error for a line it cannot read, naming that
/// source line.
pub fn check(machine: Machine, source: &str) -> Result<Vec<Finding>, Error> {
    let mut findings = listing_form(machine, source)?;
    let mut numbers = Vec::new();
    match machine {
        Machine::SinclairZxSpectrum => {
            for (index, raw) in numbered_lines(source) {
                let lexed = zx::lex_line(raw).map_err(|e| at_line(index, e.message))?;
                numbers.push((index, lexed.number));
                let starts = zx_statement_keyword(index + 1, &lexed);
                if !starts.is_empty() {
                    // The listed form of this line is refused too, so a
                    // listing-form finding would point at a line the ROM
                    // will not take; report only what is wrong.
                    findings.retain(|f| !(f.line == index + 1 && f.rule == "listing-form"));
                    findings.extend(starts);
                }
                zx_line(index + 1, &lexed, &mut findings);
            }
        }
        Machine::CommodoreC64 => {
            let mut names = C64Names::default();
            for (index, raw) in numbered_lines(source) {
                let lexed = c64::lex_line(raw).map_err(|e| at_line(index, e.message))?;
                numbers.push((index, lexed.number));
                c64_line(index + 1, &lexed, &mut names, &mut findings);
            }
        }
    }
    line_order(&numbers, &mut findings);
    findings.sort_by_key(|f| (f.line, f.column));
    Ok(findings)
}

/// The source with every numbered line replaced by its listed form (for the
/// Spectrum, after removing the spaces `stored-space` flags), lines joined
/// with `\n` and ending with one. Blank lines are dropped: LIST never shows
/// one, and a listing reads exactly as LIST shows it.
///
/// Returns `None` when that equals the source normalised the same way, so a
/// listing that differs only in line endings is left alone.
///
/// # Errors
/// Returns the tokeniser's error for a line it cannot read.
pub fn fix(machine: Machine, source: &str) -> Result<Option<String>, Error> {
    let mut fixed = String::with_capacity(source.len());
    let mut normalised = String::with_capacity(source.len());
    for (index, raw) in source.lines().enumerate() {
        let raw = raw.trim_end();
        normalised.push_str(raw);
        normalised.push('\n');
        if raw.trim().is_empty() {
            continue;
        }
        let listed = match machine {
            Machine::SinclairZxSpectrum => {
                let lexed = zx::lex_line(raw).map_err(|e| at_line(index, e.message))?;
                if !zx_statement_keyword(index + 1, &lexed).is_empty() {
                    // Left as written: see `statement-keyword`.
                    fixed.push_str(raw);
                    fixed.push('\n');
                    continue;
                }
                let body: Vec<u8> = lexed
                    .pieces
                    .iter()
                    .enumerate()
                    .filter(|&(i, _)| !zx_stored_space(&lexed.pieces, i))
                    .flat_map(|(_, p)| p.bytes.iter().copied())
                    .collect();
                zx::list_line(lexed.number, &body)
            }
            Machine::CommodoreC64 => {
                let lexed = c64::lex_line(raw).map_err(|e| at_line(index, e.message))?;
                let body: Vec<u8> = lexed.pieces.into_iter().flat_map(|p| p.bytes).collect();
                c64::list_line(lexed.number, &body)
            }
        };
        fixed.push_str(listed.trim_end());
        fixed.push('\n');
    }
    Ok((fixed != normalised).then_some(fixed))
}

/// The non-blank source lines with their 0-based indexes, as the tokenisers
/// read them.
fn numbered_lines(source: &str) -> impl Iterator<Item = (usize, &str)> {
    source
        .lines()
        .enumerate()
        .filter(|(_, raw)| !raw.trim().is_empty())
}

fn at_line(index: usize, message: String) -> Error {
    Error {
        line: index + 1,
        message,
    }
}

/// `listing-form`: a line that is not what LIST prints for it. The whole
/// line is compared, so the finding is at column 1. A blank line is flagged
/// here too: the dialect crates' `listed_form` skips blank lines (there is
/// no line number to list), but LIST never shows one either, so a source
/// file with one is not in the listed form.
fn listing_form(machine: Machine, source: &str) -> Result<Vec<Finding>, Error> {
    let listed = match machine {
        Machine::SinclairZxSpectrum => zx::listed_form(source)?,
        Machine::CommodoreC64 => c64::listed_form(source)?,
    };
    let lines: Vec<&str> = source.lines().collect();
    let mut findings: Vec<Finding> = listed
        .into_iter()
        .filter_map(|(index, listed)| {
            // The Spectrum lists a space after a keyword that ends the line;
            // a source line keeps no trailing space.
            let listed = listed.trim_end();
            let written = lines.get(index).map_or("", |l| l.trim_end());
            (written != listed).then(|| Finding {
                line: index + 1,
                column: 1,
                rule: "listing-form",
                message: format!("LIST shows `{listed}`"),
            })
        })
        .collect();
    findings.extend(
        lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.trim().is_empty())
            .map(|(index, _)| Finding {
                line: index + 1,
                column: 1,
                rule: "listing-form",
                message: "a blank line; LIST never shows one".to_owned(),
            }),
    );
    Ok(findings)
}

/// `line-order`: walking the lines in source order, a number already seen or
/// lower than the highest so far. Both machines store lines in number order
/// whatever order they are typed in, and a repeated number replaces the
/// earlier line, so the listing would not be the program.
fn line_order(numbers: &[(usize, u16)], findings: &mut Vec<Finding>) {
    let mut seen = HashSet::new();
    let mut highest = 0;
    for &(index, number) in numbers {
        let repeated = !seen.insert(number);
        if repeated || number < highest {
            let message = if repeated {
                format!("line {number} appears more than once; the later line replaces the earlier")
            } else {
                format!(
                    "line {number} comes after line {highest}; the machine stores lines in number order"
                )
            };
            findings.push(Finding {
                line: index + 1,
                column: 1,
                rule: "line-order",
                message,
            });
        }
        highest = highest.max(number);
    }
}

// --- Spectrum ------------------------------------------------------------

/// Whether piece `i` is a space `stored-space` flags: any stored space except
/// one inside a numeric variable name, that is, with name pieces as its
/// nearest non-space neighbours on both sides. The ROM allows spaces in a
/// numeric variable's name and ignores them when it looks the name up
/// (`LET now we=6: PRINT nowwe` prints 6; Vickers, *ZX Spectrum BASIC
/// Programming*, 1983, chapters 7 and 24).
fn zx_stored_space(pieces: &[zx::Piece], i: usize) -> bool {
    if pieces[i].kind != zx::PieceKind::Space {
        return false;
    }
    let is_name = |p: Option<&zx::Piece>| p.is_some_and(|p| p.kind == zx::PieceKind::Name);
    let before = pieces[..i]
        .iter()
        .rev()
        .find(|p| p.kind != zx::PieceKind::Space);
    let after = pieces[i + 1..]
        .iter()
        .find(|p| p.kind != zx::PieceKind::Space);
    !(is_name(before) && is_name(after))
}

/// `statement-keyword`: a statement whose first piece is not a command
/// keyword (DEF FN, `0xCE`, to COPY, `0xFF`). A statement starts at the
/// start of the body, after a `:` outside strings and REM, and after THEN.
/// An empty statement (`::`, a trailing `:`, nothing after THEN) is fine:
/// the ROM's statement loop skips a `:` and stops at the end of the line
/// before it looks for a command. Anything else there, such as a name
/// (`GOTO 10`, `x=1`), a number, a string, a function or `TO`, is refused
/// by the ROM's editor, and a tape built from it stops with report C,
/// Nonsense in BASIC, when the statement runs.
fn zx_statement_keyword(line: usize, lexed: &zx::LexLine) -> Vec<Finding> {
    let pieces = &lexed.pieces;
    let mut findings = Vec::new();
    let mut at_start = true;
    for (i, piece) in pieces.iter().enumerate() {
        match piece.kind {
            zx::PieceKind::Space => continue,
            zx::PieceKind::Punct if piece.text == ":" => {
                at_start = true;
                continue;
            }
            zx::PieceKind::Keyword(ZX_THEN) => {
                if !at_start {
                    at_start = true;
                    continue;
                }
            }
            zx::PieceKind::Keyword(token) if at_start && token >= ZX_FIRST_COMMAND => {
                at_start = false;
                continue;
            }
            _ => {}
        }
        if at_start {
            findings.push(Finding {
                line,
                column: lexed.body_column + piece.column + 1,
                rule: "statement-keyword",
                message: zx_statement_message(pieces, i),
            });
        }
        at_start = false;
    }
    findings
}

/// What to say about a statement that starts with piece `i`, with a hint
/// for the two ways other BASICs lead a learner there.
fn zx_statement_message(pieces: &[zx::Piece], i: usize) -> String {
    let text = pieces[i].text.trim_end();
    let upper = text.to_ascii_uppercase();
    let next = pieces[i + 1..]
        .iter()
        .find(|p| p.kind != zx::PieceKind::Space);
    let hint = if upper.starts_with("GOTO") {
        " (the Spectrum's keyword is `GO TO`)".to_owned()
    } else if upper.starts_with("GOSUB") {
        " (the Spectrum's keyword is `GO SUB`)".to_owned()
    } else if pieces[i].kind == zx::PieceKind::Name
        && next.is_some_and(|p| p.text == "=" || p.text == "(")
    {
        format!(" (an assignment needs `LET`: `LET {text}=…`)")
    } else {
        String::new()
    };
    format!(
        "a statement starts with `{text}`, not a keyword; the ROM refuses the line (C Nonsense in BASIC){hint}"
    )
}

fn zx_line(line: usize, lexed: &zx::LexLine, findings: &mut Vec<Finding>) {
    let column = |p: &zx::Piece| lexed.body_column + p.column + 1;
    let pieces = &lexed.pieces;
    // The keyword before the current piece, ignoring spaces.
    let mut last_keyword: Option<u8> = None;
    for (i, piece) in pieces.iter().enumerate() {
        if zx_stored_space(pieces, i) {
            findings.push(Finding {
                line,
                column: column(piece),
                rule: "stored-space",
                message: "a space here is stored and listed; the Spectrum's own display spacing needs none".to_owned(),
            });
        }
        if piece.kind == zx::PieceKind::Name {
            let text = &piece.text;
            // The ROM's string variables are one letter and `$` (Vickers,
            // chapter 7), so a longer name ending in `$` is a syntax error.
            if text.ends_with('$') && text.len() > 2 {
                findings.push(Finding {
                    line,
                    column: column(piece),
                    rule: "string-var-name",
                    message: format!(
                        "string variables are one letter and $ (`a$`); the ROM rejects `{text}`"
                    ),
                });
            }
            let upper = text.to_ascii_uppercase();
            if last_keyword.is_some_and(|k| ZX_NAME_TAKERS.contains(&k))
                && zx::KEYWORD_NAMES.contains(&upper.as_str())
            {
                findings.push(Finding {
                    line,
                    column: column(piece),
                    rule: "keyword-var-name",
                    message: format!(
                        "`{text}` is also a keyword; elsewhere in the program the tokeniser may store it as one"
                    ),
                });
            }
        }
        match piece.kind {
            zx::PieceKind::Space => {}
            zx::PieceKind::Keyword(token) => last_keyword = Some(token),
            _ => last_keyword = None,
        }
    }
}

// --- Commodore 64 ----------------------------------------------------------

/// The variables seen so far in a C64 listing, keyed as BASIC V2 tells them
/// apart, with the first name spelt each way.
#[derive(Default)]
struct C64Names {
    /// Key to the first name seen with it.
    first: HashMap<String, String>,
    /// Names already reported, so each is reported once.
    reported: HashSet<String>,
}

fn c64_line(line: usize, lexed: &c64::LexLine, names: &mut C64Names, findings: &mut Vec<Finding>) {
    let column = |p: &c64::Piece| lexed.body_column + p.column + 1;
    let pieces = &lexed.pieces;
    let mut in_data = false;
    for (i, piece) in pieces.iter().enumerate() {
        match piece.kind {
            c64::PieceKind::Keyword(C64_DATA) => in_data = true,
            // Only `:` ends a DATA statement's values, as in the ROM's
            // cruncher (CRUNCH, $A57C); the lexer keeps them as text.
            c64::PieceKind::Punct if piece.text == ":" => in_data = false,
            c64::PieceKind::Keyword(_) => {
                if let Some(finding) = c64_keyword_in_name(pieces, i) {
                    findings.push(Finding {
                        line,
                        column: column(&pieces[i - 1]),
                        ..finding
                    });
                }
            }
            c64::PieceKind::Name if !in_data => {
                if let Some(message) = c64_clash(pieces, i, names) {
                    findings.push(Finding {
                        line,
                        column: column(piece),
                        rule: "var-name-clash",
                        message,
                    });
                }
            }
            _ => {}
        }
    }
}

/// `keyword-in-name`: the keyword at `i` has a name piece directly on both
/// sides, as `SCORE` lexes to `SC`, OR, `E`. BASIC V2's cruncher tokenises a
/// keyword wherever it finds one outside strings, REM and DATA (CRUNCH,
/// $A57C, in *The Anatomy of the Commodore 64*), so this is not one
/// variable; the *Programmer's Reference Guide* (p. 6) warns that a keyword
/// inside a name gives `?SYNTAX ERROR`. Contact on one side only (`FORI`, `PRINTA`) is ordinary
/// unspaced BASIC V2, which the stored bytes cannot tell from a name, so it
/// is left alone. Operators are keyword tokens too (`A+B`); only word
/// keywords count. The returned finding's line and column are filled in by
/// the caller.
fn c64_keyword_in_name(pieces: &[c64::Piece], i: usize) -> Option<Finding> {
    let keyword = &pieces[i];
    if !keyword
        .text
        .bytes()
        .next()
        .is_some_and(|b| b.is_ascii_alphabetic())
    {
        return None;
    }
    let left = pieces.get(i.checked_sub(1)?)?;
    let right = pieces.get(i + 1)?;
    if left.kind != c64::PieceKind::Name || right.kind != c64::PieceKind::Name {
        return None;
    }
    let (left, word, right) = (
        left.text.to_ascii_uppercase(),
        keyword.text.to_ascii_uppercase(),
        right.text.to_ascii_uppercase(),
    );
    Some(Finding {
        line: 0,
        column: 0,
        rule: "keyword-in-name",
        message: format!(
            "`{left}{word}{right}` has the keyword {word} inside it; BASIC V2 stores it as a token, so this is not one variable (space it if you meant {left} {word} {right})"
        ),
    })
}

/// `var-name-clash`: BASIC V2 keeps only a name's first two characters and
/// its type (`$` string, `%` integer, none for real) (*Commodore 64
/// Programmer's Reference Guide*, 1982, chapter 1, "Integer, floating-point
/// and string variables", pp. 6-7). Arrays live in their own table (from
/// ARYTAB, $2F) and an `FN` name is flagged apart from a variable, so each is
/// keyed separately. The first name seen for a key stands; the first use of
/// each different name with the same key is reported.
fn c64_clash(pieces: &[c64::Piece], i: usize, names: &mut C64Names) -> Option<String> {
    let piece = &pieces[i];
    let name = piece.text.to_ascii_uppercase();
    let next = pieces.get(i + 1);
    let integer = next.is_some_and(|p| p.kind == c64::PieceKind::Punct && p.text == "%");
    let suffix = if name.ends_with('$') {
        "$"
    } else if integer {
        "%"
    } else {
        ""
    };
    // Spaces between a name and its `(` are skipped by CHRGET, so the next
    // non-space piece decides whether this is an array.
    let after_type = if integer { i + 2 } else { i + 1 };
    let array = pieces[after_type.min(pieces.len())..]
        .iter()
        .find(|p| p.kind != c64::PieceKind::Space)
        .is_some_and(|p| p.text == "(");
    let function = pieces[..i]
        .iter()
        .rev()
        .find(|p| p.kind != c64::PieceKind::Space)
        .is_some_and(|p| p.kind == c64::PieceKind::Keyword(C64_FN));
    let stem: String = name.trim_end_matches('$').chars().take(2).collect();
    let kind = match (function, array) {
        (true, _) => "fn",
        (false, true) => "array",
        (false, false) => "",
    };
    let key = format!("{stem}{suffix}{kind}");
    let spelt = format!("{name}{}", if integer { "%" } else { "" });
    let key_for_report = key.clone();
    let first = names.first.entry(key).or_insert_with(|| spelt.clone());
    if *first == spelt || !names.reported.insert(format!("{key_for_report}{spelt}")) {
        return None;
    }
    Some(format!(
        "`{spelt}` is the same variable as `{first}`; BASIC V2 reads only the first two characters"
    ))
}
