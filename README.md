# Build198x

The build-tools pipeline for the [198x family](https://github.com/build198x) — everything between authored source and a runnable artifact that isn't assembly or emulation.

Three bands: **asset conversion** (graphics → sprites/charsets/bitplanes, audio → SID/AY/samples), **data packing** (crunchers, level/tilemap packers), and **media mastering** (disk and tape images, bootable media — ADF, D64, TAP, and friends). It meets [Asm198x](https://github.com/asm198x) at the program-framing handoff: Asm198x emits the program, Build198x masters it onto media.

## Status — decided, not yet started

This repository is a placeholder. Build198x is demand-gated: nothing in the family's near-term work depends on it — the October launch ships Asm198x program framings (`.prg`/`.sna`), not Build198x volumes — so the decision to pursue it is made and the work waits for a concrete need.

Two rules, inherited from Asm198x: **native output only, never a bespoke format**, and every tool validated byte-for-byte against a real reference tool where one exists. A **membership test** keeps the scope honest — a tool belongs here only if it converts, packs, or masters build inputs into machine-ready data or media.

Sixth sibling of the 198x family, alongside Code198x, Emu198x, Asm198x, Cat198x, Forge198x, and Play198x.
