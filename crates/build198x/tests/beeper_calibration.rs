//! The beeper converter's acceptance test, as required by
//! `decisions/demand-gate-beeper-phrases.md` § Fidelity rule: Gloaming's
//! three hand-authored phrases, transcribed into notation
//! (`fixtures/gloaming-phrases.bpr`), must regenerate the prototype's
//! B/C constants exactly. The tool is trustworthy when this round trip
//! matches, and not before.
//!
//! Expected constants are copied verbatim from
//! `code-samples/sinclair-zx-spectrum/assembly/gloaming/prototype/gloaming.asm`
//! (`chime_dusk`, `fanfare_held`, `sting_nightfall`).

use build198x::beeper::{asm, notation, wav};

const FIXTURE: &str = include_str!("fixtures/gloaming-phrases.bpr");

/// The prototype's (B, C) loads, in playing order. Rests are (frames, None).
struct ExpectedPhrase {
    name: &'static str,
    /// (b, Some(c)) for a beep load, (frames, None) for a rest.
    loads: &'static [(u8, Option<u8>)],
}

const EXPECTED: &[ExpectedPhrase] = &[
    ExpectedPhrase {
        name: "chime_dusk",
        loads: &[(0xC6, Some(0xA4)), (8, None), (0xD1, Some(0xCF))],
    },
    ExpectedPhrase {
        name: "fanfare_held",
        loads: &[
            (0x83, Some(0xCF)),
            (3, None),
            (0xA5, Some(0xA4)),
            (3, None),
            (0xC4, Some(0x8A)),
            (3, None),
            (0xFF, Some(0x67)),
            (0xFF, Some(0x67)), // "so hold it by sounding it twice"
        ],
    },
    ExpectedPhrase {
        name: "sting_nightfall",
        loads: &[
            (0x53, Some(0xA4)),
            (4, None),
            (0x49, Some(0xB8)),
            (4, None),
            (0xDC, Some(0xF7)),
        ],
    },
];

/// Pull the (B, C-or-rest) loads back out of an emitted block.
fn loads_of(block: &str) -> Vec<(u8, Option<u8>)> {
    let mut loads: Vec<(u8, Option<u8>)> = Vec::new();
    let mut pending_b: Option<(u8, bool)> = None; // (value, was_hex)
    for line in block.lines() {
        let body = line.split(';').next().unwrap_or("").trim();
        if let Some(operands) = body.strip_prefix("ld") {
            let operands = operands.trim();
            if let Some(value) = operands.strip_prefix("b,") {
                let value = value.trim();
                let (parsed, was_hex) = match value.strip_prefix('$') {
                    Some(hex) => (u8::from_str_radix(hex, 16), true),
                    None => (value.parse(), false),
                };
                let parsed = parsed.unwrap_or_else(|_| panic!("bad B operand `{value}`"));
                // A decimal B is a rest frame count; flush it now.
                if was_hex {
                    pending_b = Some((parsed, true));
                } else {
                    loads.push((parsed, None));
                }
            } else if let Some(value) = operands.strip_prefix("c,") {
                let value = value.trim().strip_prefix('$').expect("C loads are hex");
                let c = u8::from_str_radix(value, 16).expect("valid C hex");
                let (b, _) = pending_b.take().expect("C load follows a B load");
                loads.push((b, Some(c)));
            }
        }
    }
    loads
}

#[test]
fn gloaming_phrases_regenerate_their_prototype_constants() {
    let phrases = notation::parse(FIXTURE).expect("fixture parses");
    assert_eq!(phrases.len(), EXPECTED.len(), "three phrases");
    for (phrase, expected) in phrases.iter().zip(EXPECTED) {
        assert_eq!(phrase.name, expected.name);
        let block = asm::emit(phrase).expect("emits");
        assert_eq!(
            loads_of(&block),
            expected.loads,
            "{}: regenerated constants must match the prototype exactly",
            expected.name
        );
    }
}

#[test]
fn every_fixture_phrase_renders_a_preview() {
    let phrases = notation::parse(FIXTURE).expect("fixture parses");
    for phrase in &phrases {
        let bytes = wav::render(phrase).expect("renders");
        assert!(bytes.len() > 44, "{}: non-empty PCM", phrase.name);
        assert_eq!(&bytes[0..4], b"RIFF");
    }
}

#[test]
fn phrase_blocks_never_contain_the_routines() {
    // The record's hardest fence: phrases only, never beep/rest themselves.
    let phrases = notation::parse(FIXTURE).expect("fixture parses");
    let all = asm::emit_all(&phrases).expect("emits");
    assert!(
        !all.contains("out"),
        "no port writes — the routine stays hand-written"
    );
    assert!(
        !all.contains("djnz"),
        "no loop code — the routine stays hand-written"
    );
    assert!(
        !all.contains("halt"),
        "no halt code — the routine stays hand-written"
    );
}
