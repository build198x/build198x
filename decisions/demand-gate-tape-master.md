# Decision: the tape master — Gloaming's cassette packaging books the media-mastering lane

**Status:** Booked, **placement settled: Build198x** (2026-07-08, umbrella
[`tape-framing-vs-mastering.md`](../../../decisions/tape-framing-vs-mastering.md)).
Demand fired 2026-07-03. Tool not yet started — this record is the gate
opening, not the build — but the build is now unblocked.

**Date:** 2026-07-03. Placement resolved 2026-07-08.

## The decision

Build198x's third tool is a **Spectrum tape master**: a program binary plus a
loading screen in, a `.tap` (and later `.tzx`) out — the standard commercial
cassette shape of 1983–84:

1. a **BASIC loader** block (auto-running: border/paper set, `LOAD "" SCREEN$`,
   `LOAD "" CODE`, `RANDOMIZE USR start`),
2. a **SCREEN$ loading screen** block (6912 bytes — `build198x image` already
   emits this format), and
3. the **CODE block** — the game binary at its org.

This is media *mastering* — the charter's third lane, first exercised (**on
the working assumption that tape mastering lands here and not in Asm198x —
itself unsettled; see § "Open: which sibling owns this?"**). Under that
assumption the membership test passes: it converts build inputs (a binary, a
screen, loader parameters) into a machine-ready medium; it is not playback
(Play198x), not emulation. The one boundary in genuine doubt is the Asm198x
program-framing seam — precisely the open question below.

## The concrete need (the gate)

Per [`demand-gate-opening.md`](demand-gate-opening.md), each tool starts when
its own concrete need fires. The need: **Gloaming's tape master** — the
commercial-shape review (Code198x,
`docs/platforms/sinclair-zx-spectrum/games/gloaming/per-unit-plan.md`
§Commercial-shape considerations, 2026-07-03) found the finished two-module
game clears the 1983–84 design bar, with the cassette packaging as one of two
items between "passes the bar" and "shippable tape": loader, SCREEN$ loading
screen, and the **verified 16K build** (org 24576 + explicit SP; booted and
played 2026-07-03) as the payload option. The other item (Kempston) closed as
a curriculum try-this the same day.

No tool exists for this today: `asm198x` emits `.sna`; the retired Docker
image's `pasmonext --tapbas` made loader+code tapes but no loading screen and
is the toolchain we deliberately walked away from; hand-rolling TAP block
maths in a project script is exactly the ad-hoc drift this org exists to
prevent.

Named future consumers, *not* opened by this record: every other Spectrum
curriculum game's tape (same shape), TZX with turbo/custom loaders (a real
1984+ topic), and the wider mastering roster (ADF/D64/DSK/TRD/MDR) — each
fires its own gate when real.

## Scope fence

In: TAP container maths (headers, checksums, block framing), the stock-ROM
loader BASIC program, SCREEN$ + CODE payloads, 48K and 16K orgs. Out (until
their own gates fire): TZX, turbo loaders, multiload, copy protection, other
machines' tape formats.

## Open: which sibling owns this?

Steve flagged (2026-07-03) that the sibling boundary is not settled: a tape is
the *framing of a program for a machine to load and run*, which is arguably an
**Asm198x** concern (it already emits `.sna`, and `.tap`/`.tzx` are the same
"here is a program, ready to run" job in a different container) rather than a
Build198x **media-mastering** one. The umbrella CLAUDE.md's current line — "Asm
emits the program; Build masters the media" — puts the cut at the program /
media seam, but a bootable tape sits *on* that seam: it is media whose entire
content is one framed program. The two readings:

- **Build198x** (this record's working assumption): a `.tap` is a container
  format like ADF/D64 — mastering payload bytes into a medium, the third lane
  of the charter. The loader BASIC and checksums are container plumbing.
- **Asm198x**: a `.tap` is just another output container for an assembled
  program, next to `.sna`/`.prg`. The retired Docker `pasmonext --tapbas` made
  tapes *from the assembler*, which is precedent for this reading.

This is an **umbrella-level** question (it binds Asm198x and Build198x and the
program/media seam between them), TBC in a later conversation. Until it
resolves, no tape code is written in either sibling — the loading-screen art
(the other dependency) is done regardless, so nothing is blocked by parking
this.

**Resolved 2026-07-08 — both readings were half-right; the seam splits by
composition.** Umbrella record:
[`tape-framing-vs-mastering.md`](../../../decisions/tape-framing-vs-mastering.md).
A tape whose content is the assembled program and nothing else (including the
pasmo `--tapbas`-parity minimal stub) is an Asm198x *framing*; the moment a
second artifact joins — a loading screen, an authored loader, another program
— it is *mastering*, owned here. Gloaming's loader + SCREEN$ + CODE tape is
mastering, so this tool proceeds in Build198x, ingesting a raw binary + org
(no dependency on Asm198x's not-yet-built `.tap` serialiser). The authored
BASIC loader reuses Emu198x's `format-sinclair-zx-spectrum-bas` tokeniser
rather than reimplementing (publish path per Emu198x's
`crate-licensing-split.md`). Steve confirmed loading screens ship on
curriculum tapes, so screen support stays in initial scope alongside the
test-card default.

## Open questions for the build session

- Loading-screen art for Gloaming does not exist yet — the tool should land
  with a test-card screen; the art is a Code198x deliverable.
- Whether the 16K build ships as the tape's payload ("for any Spectrum") or as
  a second side/tape is inlay copy, not tool design.
