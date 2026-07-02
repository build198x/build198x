//! Assembly emission — the shipping half of the converter.
//!
//! Emits phrases in Gloaming's table-free idiom (its prototype's own
//! formatting, byte-for-byte conventions): a label line, then straight-line
//! `ld b / ld c / call beep` per note and `ld b / call rest` per rest, with
//! the final event tail-calling (`jp`) exactly as the hand-written phrases
//! do. Notes longer than 255 periods chunk into consecutive `beep` calls —
//! the prototype's own "hold it by sounding it twice" move.
//!
//! The `beep`/`rest` routines are **never** emitted — they are curriculum
//! content (the demand-gate record's hardest fence).

use super::{Event, Phrase, ResolveError, chunk_periods, resolve_note};

/// Column where comments start (matches the Gloaming prototype).
const COMMENT_COLUMN: usize = 36;
/// Instruction indent (matches the prototype).
const INDENT: &str = "            ";

/// Emit one phrase as an assembly block.
///
/// # Errors
///
/// [`ResolveError`] if any event is outside the routine's range.
pub fn emit(phrase: &Phrase) -> Result<String, ResolveError> {
    let mut lines: Vec<String> = Vec::new();
    lines.push(with_comment(
        format!("{}:", phrase.name),
        phrase.comment.as_deref(),
    ));

    // Flatten events into (b, c-or-rest, comment) steps so the tail-call
    // decision sees chunks, not events.
    enum Step {
        Beep {
            b: u8,
            c: u8,
            comment: Option<String>,
        },
        Rest {
            frames: u8,
            comment: Option<String>,
        },
    }
    let mut steps: Vec<Step> = Vec::new();
    for event in &phrase.events {
        match event {
            Event::Note {
                pitch,
                duration,
                comment,
            } => {
                let note = resolve_note(pitch, *duration)?;
                let chunks = chunk_periods(note.periods);
                let held = chunks.len() > 1;
                for (i, b) in chunks.iter().enumerate() {
                    let comment = if i == 0 {
                        let label = pitch.label();
                        Some(match comment {
                            // Keep the author's text verbatim when it already
                            // names the note (the prototype's own style:
                            // "C5 answers, a shade longer").
                            Some(text) if text.contains(label.as_str()) => text.clone(),
                            Some(text) => format!("{label}, {text}"),
                            None if held => format!("{label}, held"),
                            None => label,
                        })
                    } else {
                        Some("held — B counts to 255, so sound it again".to_owned())
                    };
                    steps.push(Step::Beep {
                        b: *b,
                        c: note.c,
                        comment,
                    });
                }
            }
            Event::Rest { frames, comment } => {
                steps.push(Step::Rest {
                    frames: *frames,
                    comment: comment.clone(),
                });
            }
        }
    }

    let last = steps.len().saturating_sub(1);
    for (i, step) in steps.iter().enumerate() {
        let tail = i == last;
        match step {
            Step::Beep { b, c, comment } => {
                lines.push(with_comment(
                    format!("{INDENT}ld      b, ${b:02X}"),
                    comment.as_deref(),
                ));
                lines.push(format!("{INDENT}ld      c, ${c:02X}"));
                lines.push(format!(
                    "{INDENT}{}    beep",
                    if tail { "jp  " } else { "call" }
                ));
            }
            Step::Rest { frames, comment } => {
                lines.push(with_comment(
                    format!("{INDENT}ld      b, {frames}"),
                    comment.as_deref(),
                ));
                lines.push(format!(
                    "{INDENT}{}    rest",
                    if tail { "jp  " } else { "call" }
                ));
            }
        }
    }
    lines.push(String::new());
    Ok(lines.join("\n"))
}

/// Emit every phrase in file order, separated by blank lines.
///
/// # Errors
///
/// [`ResolveError`] from the first failing phrase.
pub fn emit_all(phrases: &[Phrase]) -> Result<String, ResolveError> {
    let blocks: Result<Vec<String>, ResolveError> = phrases.iter().map(emit).collect();
    Ok(blocks?.join("\n"))
}

/// Pad `text` to the comment column and append `; comment` when present.
fn with_comment(text: String, comment: Option<&str>) -> String {
    match comment {
        None => text,
        Some(c) => {
            let mut line = text;
            while line.chars().count() < COMMENT_COLUMN {
                line.push(' ');
            }
            line.push_str("; ");
            line.push_str(c);
            line
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Duration, Pitch};
    use super::*;

    fn note(
        spelling: &str,
        semitone: u8,
        octave: u8,
        periods: u32,
        comment: Option<&str>,
    ) -> Event {
        Event::Note {
            pitch: Pitch::Note {
                semitone,
                octave,
                spelling: spelling.to_owned(),
            },
            duration: Duration::Periods(periods),
            comment: comment.map(str::to_owned),
        }
    }

    /// chime_dusk regenerates in the prototype's exact shape.
    #[test]
    fn chime_dusk_block_matches_the_prototype_shape() {
        let phrase = Phrase {
            name: "chime_dusk".to_owned(),
            comment: Some("the title — two bell notes, high then low".to_owned()),
            events: vec![
                note("E5", 4, 5, 198, Some("a bright strike")),
                Event::Rest {
                    frames: 8,
                    comment: None,
                },
                note("C5", 0, 5, 209, Some("C5 answers, a shade longer")),
            ],
        };
        let block = emit(&phrase).expect("emits");
        let expected = "\
chime_dusk:                         ; the title — two bell notes, high then low
            ld      b, $C6          ; E5, a bright strike
            ld      c, $A4
            call    beep
            ld      b, 8
            call    rest
            ld      b, $D1          ; C5 answers, a shade longer
            ld      c, $CF
            jp      beep
";
        assert_eq!(block, expected);
    }

    /// A held note chunks into consecutive beeps, final chunk tail-calls.
    #[test]
    fn held_note_chunks_like_the_fanfare() {
        let phrase = Phrase {
            name: "hold".to_owned(),
            comment: None,
            events: vec![note("C6", 0, 6, 510, None)],
        };
        let block = emit(&phrase).expect("emits");
        assert!(block.contains("ld      b, $FF          ; C6, held"));
        assert!(block.contains("call    beep"));
        assert!(block.ends_with("jp      beep\n"));
        assert_eq!(block.matches("ld      c, $67").count(), 2);
    }
}
