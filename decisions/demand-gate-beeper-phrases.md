# Decision: the beeper-phrase converter — Gloaming's audio pass opens the audio lane

**Status:** Active (blessed by Steve 2026-07-02).

**Date:** 2026-07-02.

## The decision

Build198x's second tool is a **beeper-phrase converter**: textual note-event
notation in, two outputs out —

1. a **preview WAV** of the phrase, so a human ear can audition it in seconds,
   and
2. an **assembly data table** for the established Spectrum port-`$FE` beeper
   routine, so the bytes shipped are the bytes auditioned.

Same input, two renderings. The membership test passes cleanly: this
*converts a build input (a phrase description) into machine-ready data* — it is
not assembly (Asm198x), not playback of existing media (Play198x), not
emulation, not cataloguing.

## The concrete need (the gate)

Per [`demand-gate-opening.md`](demand-gate-opening.md), each tool starts when
its own concrete need fires, citing that record's pattern. The need:

**Gloaming's audio pass** — the last unbuilt tier before its capstone run
(prototype log, 2026-07-02: "the audio pass (chime redo + a dawn phrase — needs
Steve's ear)"). The phrase "needs Steve's ear" is the design constraint: no
tool composes the dawn phrase. What the tool changes is the iteration loop
around the ear. Today one audition costs edit-asm → assemble → boot →
`save_audio_capture` → listen — minutes per attempt. With the converter it is
edit-notation → play WAV — seconds — and the accepted phrase emits its data
table without retyping.

Named future consumers, *not* opened by this record: Shadowkeep's Spectrum
audio (same routine family), the C64 track's SID work, AY/PT3 for 128K
Spectrum. Each fires its own gate when real.

## Scope fence

- **Notation:** a plain-text event list — note (or rest), duration, and the
  small effect vocabulary the port-`$FE` routine actually implements (e.g.
  pitch slide for the "rising blip"). No general music notation, no MIDI
  import, no tracker format.
- **WAV preview:** square-wave synthesis of the same event list, mirroring the
  routine's timing model closely enough to trust the audition. This is a
  converter diagnostic in the sense of
  [`play198x-boundary.md`](play198x-boundary.md) — it renders *what was just
  converted*, never arbitrary existing audio files.
- **Asm output:** the phrase in the game's own idiom. Gloaming's taught style
  is deliberately table-free ("no tables, no driver: each phrase is
  straight-line code"), so the emitted artefact is the straight-line
  `ld b / ld c / call beep` block, house-formatted, source comments carried
  through. A data-table target is a later option for a game that grows a
  driver. The `beep`/`rest` routines themselves stay hand-written curriculum
  content — the tool emits phrases, never the routines.
- **Size discipline:** version one is a small tool, not a product — parse,
  synthesise, emit. If it cannot audition a phrase for Steve's ear within a
  day or two of starting, it has grown past its demand: stop and re-read this
  fence.
- **No audio capability layer.** `mediaspec` earned its existence with Emu198x
  as a second consumer from day 1. An `audiospec` analogue waits for a second
  audio tool with a real shared-data need (likely the SID or AY gate); one
  square-wave phrase tool does not justify a spec layer.

## Naming

Module lives inside the `build198x` crate per
[`module-and-crate-naming.md`](module-and-crate-naming.md); provisional CLI
shape `build198x beeper` alongside `build198x image`. If Play198x ever consumes
the phrase-data codec it splits out under the system-namespaced pattern.

## Fidelity rule

The WAV preview approximates; the emulator remains the verifier. An accepted
phrase still gets one `save_audio_capture` pass in Emu198x before a unit ships
it — the preview collapses iteration, it does not replace verification.

**Calibration is the acceptance test.** The Gloaming prototype already carries
three hand-authored phrases (`chime_dusk`, `fanfare_held`, `sting_nightfall`)
whose constants are note-annotated and consistent with the 16-T-state inner
loop model (C=$A4 → ~659 Hz = the E5 the comment claims). Transcribe all three
into notation, regenerate, and diff against the prototype's constants — the
tool is trustworthy when the round trip matches, and not before.

## Drift triggers

Stop and re-read this record if you are about to:

- **"Add tracker-file import"** — that is the AY/PT3 gate's business, not this
  tool's.
- **"Generalise the synthesis into an audio engine"** — the preview mirrors one
  routine's timing model; a second routine means a second data target, not an
  engine.
- **"Emit the player routine too"** — never; `beep`/`rest` are curriculum
  content, taught hand-written. Phrases only.
- **"Spend a week on the notation design"** — the notation serves one ear and
  one routine; ship the smallest thing Steve can audition with.
