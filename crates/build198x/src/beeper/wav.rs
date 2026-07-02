//! Square-wave preview synthesis — the audition half of the converter.
//!
//! Renders a phrase to mono 16-bit 44.1 kHz PCM by walking the same T-state
//! model the assembly emitter uses: each note is `periods` full cycles of
//! `32·C + 44` T-states (speaker high for the first `16·C + 17`), each rest
//! is `frames × 69,888` T-states of silence. The routine runs under `DI`,
//! so elapsed preview time is elapsed game-freeze time — the preview is
//! honest about cost, not just pitch.
//!
//! Per `decisions/play198x-boundary.md`, this renders *what was just
//! converted* — it is a converter diagnostic, not a media player. Synthesis
//! is integer-only (T-states scaled by the sample counter), so output is
//! byte-identical across platforms per the determinism contract.

use super::{Event, Phrase, ResolveError, TSTATES_PER_FRAME, TSTATES_PER_SECOND, resolve_note};

/// Preview sample rate in Hz.
pub const SAMPLE_RATE: u32 = 44_100;
/// Square-wave amplitude (±) in 16-bit PCM.
const AMPLITUDE: i16 = 11_468; // ~0.35 full scale

/// One resolved timeline segment.
enum Segment {
    /// Tone: full-period and first-half lengths in T-states.
    Tone {
        /// T-states of one full period.
        period_t: u64,
        /// T-states of the speaker-high first half.
        first_half_t: u64,
    },
    /// Silence.
    Silence,
}

/// Render a phrase to WAV bytes.
///
/// # Errors
///
/// [`ResolveError`] if any event is outside the routine's range.
pub fn render(phrase: &Phrase) -> Result<Vec<u8>, ResolveError> {
    render_repeated(phrase, 1)
}

/// Render a phrase played `repeats` times back to back — the loop-point
/// audition (`--repeat`). Preview-only: the emitted assembly is always the
/// single phrase; looping is the game code's job (call the cell, poll the
/// keyboard, call it again).
///
/// # Errors
///
/// [`ResolveError`] if any event is outside the routine's range.
pub fn render_repeated(phrase: &Phrase, repeats: u32) -> Result<Vec<u8>, ResolveError> {
    // Resolve events into (start_t, end_t, segment) spans.
    let mut segments: Vec<(u64, u64, Segment)> = Vec::new();
    let mut t = 0u64;
    let repeated =
        std::iter::repeat_n(&phrase.events, usize::try_from(repeats).unwrap_or(1)).flatten();
    for event in repeated {
        match event {
            Event::Note {
                pitch, duration, ..
            } => {
                let note = resolve_note(pitch, *duration)?;
                let len = note.period_t * u64::from(note.periods);
                segments.push((
                    t,
                    t + len,
                    Segment::Tone {
                        period_t: note.period_t,
                        first_half_t: note.first_half_t,
                    },
                ));
                t += len;
            }
            Event::Rest { frames, .. } => {
                let len = TSTATES_PER_FRAME * u64::from(*frames);
                segments.push((t, t + len, Segment::Silence));
                t += len;
            }
        }
    }
    let total_t = t;
    // ceil(total_t · rate / clock) samples
    let total_samples = (total_t * u64::from(SAMPLE_RATE)).div_ceil(TSTATES_PER_SECOND);

    let mut pcm: Vec<i16> = Vec::with_capacity(usize::try_from(total_samples).unwrap_or(0));
    let mut current = 0usize;
    for i in 0..total_samples {
        let sample_t = i * TSTATES_PER_SECOND / u64::from(SAMPLE_RATE);
        while current < segments.len() && sample_t >= segments[current].1 {
            current += 1;
        }
        let value = match segments.get(current) {
            Some((
                start,
                _,
                Segment::Tone {
                    period_t,
                    first_half_t,
                },
            )) => {
                let phase = (sample_t - start) % period_t;
                if phase < *first_half_t {
                    AMPLITUDE
                } else {
                    -AMPLITUDE
                }
            }
            _ => 0,
        };
        pcm.push(value);
    }
    Ok(encode_wav(&pcm))
}

/// Wrap PCM samples in a minimal RIFF/WAVE container (PCM, mono, 16-bit).
fn encode_wav(samples: &[i16]) -> Vec<u8> {
    let data_len = u32::try_from(samples.len() * 2).unwrap_or(u32::MAX);
    let mut out = Vec::with_capacity(44 + samples.len() * 2);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    out.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes()); // byte rate
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::{Duration, Pitch};
    use super::*;

    fn e5_phrase() -> Phrase {
        Phrase {
            name: "test".to_owned(),
            comment: None,
            events: vec![
                Event::Note {
                    pitch: Pitch::Note {
                        semitone: 4,
                        octave: 5,
                        spelling: "E5".to_owned(),
                    },
                    duration: Duration::Periods(198),
                    comment: None,
                },
                Event::Rest {
                    frames: 8,
                    comment: None,
                },
            ],
        }
    }

    #[test]
    fn wav_has_correct_header_and_length() {
        let bytes = render(&e5_phrase()).expect("renders");
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        // 198 periods · 5292 T + 8 · 69,888 T = 1,606,920 T ⇒ ~0.459 s
        let total_t = 198 * 5292 + 8 * 69_888;
        let expected_samples = (total_t * 44_100u64).div_ceil(3_500_000);
        assert_eq!(bytes.len() as u64, 44 + expected_samples * 2);
    }

    #[test]
    fn tone_toggles_and_rest_is_silent() {
        let bytes = render(&e5_phrase()).expect("renders");
        let pcm: Vec<i16> = bytes[44..]
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect();
        // The tone section contains both polarities…
        let tone_end = (198 * 5292 * 44_100u64 / 3_500_000) as usize;
        assert!(pcm[..tone_end].contains(&AMPLITUDE));
        assert!(pcm[..tone_end].contains(&-AMPLITUDE));
        // …and the rest tail is silent.
        assert!(pcm[tone_end + 2..].iter().all(|&s| s == 0));
    }

    #[test]
    fn output_is_deterministic() {
        let a = render(&e5_phrase()).expect("renders");
        let b = render(&e5_phrase()).expect("renders");
        assert_eq!(a, b);
    }

    #[test]
    fn repeated_render_is_proportionally_longer() {
        let once = render_repeated(&e5_phrase(), 1).expect("renders");
        let thrice = render_repeated(&e5_phrase(), 3).expect("renders");
        let once_pcm = (once.len() - 44) as u64;
        let thrice_pcm = (thrice.len() - 44) as u64;
        // Within a couple of samples of exactly 3× — the single pass ceils
        // its tail sample, so 3·ceil(x) can exceed ceil(3x) slightly.
        assert!((once_pcm * 3).abs_diff(thrice_pcm) <= 6);
    }
}
