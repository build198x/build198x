# Master an authored loader and CODE onto a standard +3 disk

Accepted 2026-10-14 for the cross-stack Spectrum qualification consumer
([umbrella #84](https://github.com/stevehill1981/198x/issues/84)). This opens the
disk scope deferred by `demand-gate-basic.md`; it does not expand that command.

`build198x plus3` takes an authored BASIC listing and raw assembled CODE with an
explicit decimal load address. It writes a standard 180 KiB +3DOS filesystem in
a CPCEMU DSK: `DISK` autoruns its first stored BASIC line and `DIAG.BIN` carries
the CODE. The loader must name and call that CODE; the tool does not rewrite
the user's BASIC or infer an entry address from raw bytes.

Reuse the current BASIC lint/tokeniser and the newly graduated
`format198x-amstrad-cpc-dsk` library. The user approved this family-owned
dependency; no third-party dependency is added. Byte layouts stay in Format198x
under `module-and-crate-naming.md`.

The CLI supports text/JSON success reports and atomic output. It refuses an
existing destination without `--force`, refuses input aliases, and leaves
existing output intact on invalid input. Deterministic native bytes contain no
host metadata. Qualification must check them independently and load them via
the real +3 ROM and FDC in Emu and MAME.

Other geometries, filenames, boot-sector code, EDSK writing, disk editing and
general-purpose mastering remain outside this first consumer.
