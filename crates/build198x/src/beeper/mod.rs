//! Spectrum beeper-phrase converter — one timing model, two renderings.
//!
//! Converts a textual phrase notation (see [`notation`]) into a preview WAV
//! ([`wav`]) and the phrase in Gloaming's table-free assembly idiom
//! ([`asm`]). Opened by `decisions/demand-gate-beeper-phrases.md`; the
//! concrete consumer is Gloaming's audio pass.
//!
//! # The timing model
//!
//! The target routine is the Gloaming prototype's `beep`
//! (Code198x `code-samples/sinclair-zx-spectrum/assembly/gloaming/prototype/gloaming.asm`):
//! interrupts off, toggle the port-`$FE` speaker bit with a `DEC A / JR NZ`
//! delay loop of `C` iterations per half-period, `B` full cycles per note.
//! Instruction T-states per the Zilog Z80 CPU User Manual give, at the 48K
//! Spectrum's 3.5 MHz:
//!
//! - first half-period (`LD A,SPEAKER / OUT / LD A,C` + loop): `16·C + 17` T
//! - second half-period (`XOR A / OUT / LD A,C` + loop + `DJNZ`): `16·C + 27` T
//! - full period: **`32·C + 44` T-states**
//!
//! `rest` is `B` frames of `HALT` (69,888 T per 48K frame).
//!
//! The model is calibrated against the prototype's three hand-authored
//! phrases: with truncation (not rounding) in [`c_for_frequency`], all six
//! note constants regenerate exactly (E5→$A4, C5→$CF, G5→$8A, C6→$67,
//! D5→$B8, A4→$F7) — the calibration test in
//! `tests/beeper_calibration.rs` holds that invariant.
//!
//! # Determinism
//!
//! Everything here obeys `decisions/determinism-contract.md`'s basic-ops
//! rule: note frequencies are `f64` literals scaled by exact powers of two
//! (no `powf`), constant derivation is divide/subtract/truncate, and WAV
//! synthesis walks T-states with integer arithmetic only.

pub mod asm;
pub mod notation;
pub mod wav;

/// Z80 clock of the 48K Spectrum, in T-states per second.
pub const TSTATES_PER_SECOND: u64 = 3_500_000;
/// T-states per 48K Spectrum frame (the `HALT` quantum of `rest`).
pub const TSTATES_PER_FRAME: u64 = 69_888;
/// Fixed T-state overhead of one full `beep` period beyond the delay loops.
pub const PERIOD_OVERHEAD_T: u64 = 44;
/// T-states per delay-loop iteration (`DEC A` 4 + `JR NZ` taken 12), doubled
/// across the two half-periods: full period = `32·C + 44`.
pub const TSTATES_PER_LOOP_PAIR: u64 = 32;
/// Extra T-states in the first half-period beyond its delay loop.
pub const FIRST_HALF_OVERHEAD_T: u64 = 17;

/// One phrase: a named sequence of notes and rests.
#[derive(Clone, Debug, PartialEq)]
pub struct Phrase {
    /// Label the emitted block carries.
    pub name: String,
    /// Comment on the `phrase` line, carried to the label line.
    pub comment: Option<String>,
    /// The events, in playing order.
    pub events: Vec<Event>,
}

/// One notation event.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// A tone: `B` cycles of the square wave at delay constant `C`.
    Note {
        /// Pitch specification as written (note name or raw Hz).
        pitch: Pitch,
        /// Requested duration.
        duration: Duration,
        /// Comment after the event, carried to the emitted `ld b` line.
        comment: Option<String>,
    },
    /// Silence: `frames` frames of `HALT`.
    Rest {
        /// Number of 50 Hz frames.
        frames: u8,
        /// Comment after the event.
        comment: Option<String>,
    },
}

/// A pitch, as authored.
#[derive(Clone, Debug, PartialEq)]
pub enum Pitch {
    /// A 12-TET note: semitone 0–11 (C=0 … B=11) and octave 0–8.
    Note {
        /// Semitone within the octave, C=0 … B=11.
        semitone: u8,
        /// Scientific-pitch octave, 0–8.
        octave: u8,
        /// The spelling as written (`E5`, `C#4`…), kept for comments/reports.
        spelling: String,
    },
    /// A raw frequency in Hz (for unpitched blips).
    Hz(f64),
}

/// A duration, as authored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Duration {
    /// Milliseconds; converted via the period length.
    Millis(u32),
    /// Exact full periods of the square wave (`B` counts, may exceed 255 —
    /// the emitter chunks).
    Periods(u32),
}

/// A note resolved against the timing model: the constants the routine
/// consumes and the exact T-state cost.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResolvedNote {
    /// Delay constant `C` (1–255).
    pub c: u8,
    /// Total full periods (chunked into ≤255 `B` loads by the emitter).
    pub periods: u32,
    /// T-states of one full period (`32·C + 44`).
    pub period_t: u64,
    /// T-states of the first (speaker-high) half-period (`16·C + 17`).
    pub first_half_t: u64,
}

/// Twelve-tone equal temperament, octave 4, C4 … B4, in Hz. Literals so the
/// determinism contract's basic-ops rule holds — octave shifts multiply by
/// exact powers of two.
const OCTAVE4_HZ: [f64; 12] = [
    261.625_565_300_598_6,  // C4
    277.182_630_976_872_1,  // C#4
    293.664_767_917_407_6,  // D4
    311.126_983_722_080_9,  // D#4
    329.627_556_912_869_9,  // E4
    349.228_231_433_003_9,  // F4
    369.994_422_711_634_4,  // F#4
    391.995_435_981_749_27, // G4
    415.304_697_579_945_1,  // G#4
    440.0,                  // A4
    466.163_761_518_089_9,  // A#4
    493.883_301_256_124_1,  // B4
];

/// Errors resolving a notation event against the model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolveError {
    /// The frequency needs a delay constant outside 1–255.
    PitchOutOfRange {
        /// The spelling or Hz value as written.
        pitch: String,
    },
    /// The duration resolves to zero periods.
    DurationTooShort,
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PitchOutOfRange { pitch } => {
                write!(
                    f,
                    "pitch {pitch} is outside the routine's range (C must be 1-255)"
                )
            }
            Self::DurationTooShort => write!(f, "duration resolves to zero periods"),
        }
    }
}

impl Pitch {
    /// The frequency in Hz. Note names scale the octave-4 literal by an
    /// exact power of two; `Hz` passes through.
    #[must_use]
    pub fn frequency(&self) -> f64 {
        match self {
            Self::Hz(hz) => *hz,
            Self::Note {
                semitone, octave, ..
            } => {
                let mut f = OCTAVE4_HZ[usize::from(*semitone % 12)];
                let oct = i32::from(*octave);
                if oct >= 4 {
                    for _ in 4..oct {
                        f *= 2.0;
                    }
                } else {
                    for _ in oct..4 {
                        f /= 2.0;
                    }
                }
                f
            }
        }
    }

    /// The spelling for comments and error messages.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Note { spelling, .. } => spelling.clone(),
            Self::Hz(hz) => format!("{hz}hz"),
        }
    }
}

/// Delay constant `C` for a frequency, by the calibrated model:
/// `C = trunc((3_500_000/f − 44) / 32)`. Truncation, not rounding — that is
/// what regenerates all six of the prototype's hand-authored constants.
///
/// # Errors
///
/// [`ResolveError::PitchOutOfRange`] when the result leaves 1–255.
pub fn c_for_frequency(hz: f64, label: &str) -> Result<u8, ResolveError> {
    if !(hz.is_finite()) || hz <= 0.0 {
        return Err(ResolveError::PitchOutOfRange {
            pitch: label.to_owned(),
        });
    }
    #[allow(clippy::cast_precision_loss)] // 3.5e6 is exact in f64
    let ideal =
        (TSTATES_PER_SECOND as f64 / hz - PERIOD_OVERHEAD_T as f64) / TSTATES_PER_LOOP_PAIR as f64;
    if !(1.0..256.0).contains(&ideal) {
        return Err(ResolveError::PitchOutOfRange {
            pitch: label.to_owned(),
        });
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // range-checked above
    Ok(ideal as u8)
}

/// T-states of one full period for a delay constant.
#[must_use]
pub fn period_tstates(c: u8) -> u64 {
    TSTATES_PER_LOOP_PAIR * u64::from(c) + PERIOD_OVERHEAD_T
}

/// Resolve a note event against the model.
///
/// # Errors
///
/// [`ResolveError`] for out-of-range pitch or a zero-period duration.
pub fn resolve_note(pitch: &Pitch, duration: Duration) -> Result<ResolvedNote, ResolveError> {
    let c = c_for_frequency(pitch.frequency(), &pitch.label())?;
    let period_t = period_tstates(c);
    let periods = match duration {
        Duration::Periods(p) => p,
        Duration::Millis(ms) => {
            // rounded integer division: periods = round(ms · 3500 / period_t)
            let t = u64::from(ms) * (TSTATES_PER_SECOND / 1000);
            u32::try_from((t + period_t / 2) / period_t).unwrap_or(u32::MAX)
        }
    };
    if periods == 0 {
        return Err(ResolveError::DurationTooShort);
    }
    Ok(ResolvedNote {
        c,
        periods,
        period_t,
        first_half_t: (TSTATES_PER_LOOP_PAIR / 2) * u64::from(c) + FIRST_HALF_OVERHEAD_T,
    })
}

/// Split a period count into the routine's `B` loads (1–255 each, greedy).
#[must_use]
pub fn chunk_periods(periods: u32) -> Vec<u8> {
    let mut chunks = Vec::new();
    let mut left = periods;
    while left > 255 {
        chunks.push(255);
        left -= 255;
    }
    if left > 0 {
        #[allow(clippy::cast_possible_truncation)] // ≤255 by the loop above
        chunks.push(left as u8);
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The prototype's six note constants regenerate exactly (the module-doc
    /// calibration claim; the full-phrase version lives in
    /// `tests/beeper_calibration.rs`).
    #[test]
    fn prototype_constants_regenerate() {
        let cases: [(&str, u8, u8, u8); 6] = [
            ("E5", 4, 5, 0xA4),
            ("C5", 0, 5, 0xCF),
            ("G5", 7, 5, 0x8A),
            ("C6", 0, 6, 0x67),
            ("D5", 2, 5, 0xB8),
            ("A4", 9, 4, 0xF7),
        ];
        for (spelling, semitone, octave, expected_c) in cases {
            let pitch = Pitch::Note {
                semitone,
                octave,
                spelling: spelling.to_owned(),
            };
            let c = c_for_frequency(pitch.frequency(), spelling);
            assert_eq!(c, Ok(expected_c), "{spelling}");
        }
    }

    #[test]
    fn period_model_matches_hand_arithmetic() {
        // E5's C=$A4=164: 32·164 + 44 = 5292 T ⇒ ~661 Hz on a 3.5 MHz clock.
        assert_eq!(period_tstates(0xA4), 5292);
    }

    #[test]
    fn millis_round_to_nearest_period() {
        let pitch = Pitch::Note {
            semitone: 4,
            octave: 5,
            spelling: "E5".to_owned(),
        };
        let note = resolve_note(&pitch, Duration::Millis(299)).expect("E5 resolves");
        assert_eq!(note.periods, 198); // chime_dusk's opening strike, B=$C6
    }

    #[test]
    fn long_notes_chunk_at_255() {
        assert_eq!(chunk_periods(510), vec![255, 255]); // fanfare's held C6
        assert_eq!(chunk_periods(198), vec![198]);
        assert_eq!(chunk_periods(256), vec![255, 1]);
    }

    #[test]
    fn out_of_range_pitch_is_an_error() {
        // Above ~53 kHz the constant would drop below 1.
        assert!(c_for_frequency(60_000.0, "60000hz").is_err());
        // C=255 puts the floor at 32·255+44 = 8204 T ⇒ ~427 Hz; A3 (220 Hz)
        // needs C≈494. The routine genuinely cannot play it — honest error.
        assert!(c_for_frequency(220.0, "A3").is_err());
    }
}
