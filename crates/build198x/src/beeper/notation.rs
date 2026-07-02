//! The phrase notation parser.
//!
//! Line-based, comment-preserving. The grammar is exactly what one ear and
//! one routine need (the demand-gate record's fence):
//!
//! ```text
//! ; file-level comment (ignored)
//! phrase chime_dusk        ; the title — two bell notes, high then low
//!   E5  300ms              ; a bright strike
//!   rest 8
//!   C5  340ms              ; C5 answers, a shade longer
//! ```
//!
//! - `phrase <name>` opens a phrase; the name must be a valid assembly
//!   label (`[A-Za-z_][A-Za-z0-9_]*`).
//! - A note is `<pitch> <duration>`: pitch is a note name (`C4`…`B8`,
//!   `#`/`b` accidentals) or `<number>hz`; duration is `<int>ms` or
//!   `<int>p` (exact periods).
//! - `rest <frames>` is silence in 50 Hz frames (1–255).
//! - `;` starts a comment; on event and phrase lines it is preserved into
//!   the emitted assembly.

use super::{Duration, Event, Phrase, Pitch};

/// A parse failure, pointing at its line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    /// 1-based line number.
    pub line: usize,
    /// What went wrong.
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

/// Parse a notation file into phrases.
///
/// # Errors
///
/// [`ParseError`] on the first malformed line.
pub fn parse(source: &str) -> Result<Vec<Phrase>, ParseError> {
    let mut phrases: Vec<Phrase> = Vec::new();
    for (index, raw) in source.lines().enumerate() {
        let line_no = index + 1;
        let (body, comment) = split_comment(raw);
        let body = body.trim();
        if body.is_empty() {
            continue; // blank or comment-only line
        }
        let mut words = body.split_whitespace();
        let head = words.next().unwrap_or_default();
        if head == "phrase" {
            let name = words.next().ok_or_else(|| ParseError {
                line: line_no,
                message: "phrase needs a name".to_owned(),
            })?;
            if words.next().is_some() {
                return Err(ParseError {
                    line: line_no,
                    message: "phrase takes exactly one name".to_owned(),
                });
            }
            if !is_label(name) {
                return Err(ParseError {
                    line: line_no,
                    message: format!("`{name}` is not a valid assembly label"),
                });
            }
            phrases.push(Phrase {
                name: name.to_owned(),
                comment,
                events: Vec::new(),
            });
            continue;
        }
        let phrase = phrases.last_mut().ok_or_else(|| ParseError {
            line: line_no,
            message: "events must follow a `phrase` line".to_owned(),
        })?;
        if head == "rest" {
            let frames_word = words.next().ok_or_else(|| ParseError {
                line: line_no,
                message: "rest needs a frame count".to_owned(),
            })?;
            let frames: u8 = frames_word.parse().map_err(|_| ParseError {
                line: line_no,
                message: format!("`{frames_word}` is not a frame count (1-255)"),
            })?;
            if frames == 0 || words.next().is_some() {
                return Err(ParseError {
                    line: line_no,
                    message: "rest takes one frame count, 1-255".to_owned(),
                });
            }
            phrase.events.push(Event::Rest { frames, comment });
            continue;
        }
        // A note line: <pitch> <duration>
        let pitch = parse_pitch(head).ok_or_else(|| ParseError {
            line: line_no,
            message: format!("`{head}` is not a note name or `<number>hz`"),
        })?;
        let duration_word = words.next().ok_or_else(|| ParseError {
            line: line_no,
            message: format!("note {head} needs a duration (`<int>ms` or `<int>p`)"),
        })?;
        let duration = parse_duration(duration_word).ok_or_else(|| ParseError {
            line: line_no,
            message: format!("`{duration_word}` is not a duration (`<int>ms` or `<int>p`)"),
        })?;
        if words.next().is_some() {
            return Err(ParseError {
                line: line_no,
                message: "a note line is `<pitch> <duration>` only".to_owned(),
            });
        }
        phrase.events.push(Event::Note {
            pitch,
            duration,
            comment,
        });
    }
    Ok(phrases)
}

/// Split a line at its `;`, trimming the comment. Returns the comment only
/// when non-empty.
fn split_comment(line: &str) -> (&str, Option<String>) {
    match line.split_once(';') {
        Some((body, comment)) => {
            let comment = comment.trim();
            (body, (!comment.is_empty()).then(|| comment.to_owned()))
        }
        None => (line, None),
    }
}

fn is_label(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// `C4`…`B8` with `#`/`b`, or `<number>hz`.
fn parse_pitch(word: &str) -> Option<Pitch> {
    if let Some(hz) = word.strip_suffix("hz") {
        let value: f64 = hz.parse().ok()?;
        return Some(Pitch::Hz(value));
    }
    let mut chars = word.chars();
    let letter = chars.next()?.to_ascii_uppercase();
    let base: i8 = match letter {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let rest: String = chars.collect();
    let (accidental, octave_str) = match rest.strip_prefix(['#', 'b']) {
        Some(oct) if rest.starts_with('#') => (1i8, oct),
        Some(oct) => (-1i8, oct),
        None => (0i8, rest.as_str()),
    };
    let octave: u8 = octave_str.parse().ok()?;
    if octave > 8 {
        return None;
    }
    let semitone = base + accidental;
    // Cb and B# spill across the octave boundary; keep v1 strict instead of
    // silently re-octaving.
    if !(0..=11).contains(&semitone) {
        return None;
    }
    #[allow(clippy::cast_sign_loss)] // range-checked above
    Some(Pitch::Note {
        semitone: semitone as u8,
        octave,
        spelling: word.to_owned(),
    })
}

fn parse_duration(word: &str) -> Option<Duration> {
    if let Some(ms) = word.strip_suffix("ms") {
        return ms.parse().ok().map(Duration::Millis);
    }
    if let Some(periods) = word.strip_suffix('p') {
        return periods.parse().ok().map(Duration::Periods);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_phrase_with_comments() {
        let src =
            "; file note\nphrase chime ; two bells\n  E5 300ms ; strike\n  rest 8\n  C5 340ms\n";
        let phrases = parse(src).expect("parses");
        assert_eq!(phrases.len(), 1);
        let p = &phrases[0];
        assert_eq!(p.name, "chime");
        assert_eq!(p.comment.as_deref(), Some("two bells"));
        assert_eq!(p.events.len(), 3);
        match &p.events[0] {
            Event::Note {
                pitch,
                duration,
                comment,
            } => {
                assert_eq!(pitch.label(), "E5");
                assert_eq!(*duration, Duration::Millis(300));
                assert_eq!(comment.as_deref(), Some("strike"));
            }
            other => panic!("expected note, got {other:?}"),
        }
    }

    #[test]
    fn accidentals_and_hz_parse() {
        assert!(matches!(
            parse_pitch("F#5"),
            Some(Pitch::Note {
                semitone: 6,
                octave: 5,
                ..
            })
        ));
        assert!(matches!(
            parse_pitch("Bb4"),
            Some(Pitch::Note {
                semitone: 10,
                octave: 4,
                ..
            })
        ));
        assert!(matches!(parse_pitch("4310hz"), Some(Pitch::Hz(_))));
        assert_eq!(parse_pitch("H5"), None);
        assert_eq!(parse_pitch("Cb4"), None); // strict: no octave spill
    }

    #[test]
    fn events_before_a_phrase_are_an_error() {
        let err = parse("E5 300ms\n").expect_err("must fail");
        assert_eq!(err.line, 1);
    }

    #[test]
    fn period_durations_parse() {
        assert_eq!(parse_duration("198p"), Some(Duration::Periods(198)));
        assert_eq!(parse_duration("300ms"), Some(Duration::Millis(300)));
        assert_eq!(parse_duration("300"), None);
    }
}
