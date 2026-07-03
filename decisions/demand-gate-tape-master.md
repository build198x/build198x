# Decision: the tape master — Gloaming's cassette packaging books the media-mastering lane

**Status:** Booked (demand fired 2026-07-03; Steve directed the closeout from
the Code198x session). Tool not yet started — this record is the gate opening,
not the build.

**Date:** 2026-07-03.

## The decision

Build198x's third tool is a **Spectrum tape master**: a program binary plus a
loading screen in, a `.tap` (and later `.tzx`) out — the standard commercial
cassette shape of 1983–84:

1. a **BASIC loader** block (auto-running: border/paper set, `LOAD "" SCREEN$`,
   `LOAD "" CODE`, `RANDOMIZE USR start`),
2. a **SCREEN$ loading screen** block (6912 bytes — `build198x image` already
   emits this format), and
3. the **CODE block** — the game binary at its org.

This is media *mastering* — the charter's third lane, first exercised. The
membership test passes: it converts build inputs (a binary, a screen, loader
parameters) into a machine-ready medium; it is not assembly (Asm198x emits the
program — the handoff is the program-framing seam the umbrella decision
names), not playback (Play198x), not emulation.

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

## Open questions for the build session

- Loading-screen art for Gloaming does not exist yet — the tool should land
  with a test-card screen; the art is a Code198x deliverable.
- Whether the 16K build ships as the tape's payload ("for any Spectrum") or as
  a second side/tape is inlay copy, not tool design.
