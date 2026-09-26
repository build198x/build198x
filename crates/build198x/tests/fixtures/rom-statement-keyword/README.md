# What the 48K ROM does with a statement that starts without a keyword

`results.tsv` records what the genuine Spectrum 48K ROM does with each line of
`cases.bas`. There is one row per line: the editor's verdict, the report after
RUN, and the line itself. `tests/basic_lint.rs` reads it, so the
`statement-keyword` rule is held to what the ROM did, not to a hand-written list.
The rule must flag a line exactly when RUN stops with `C Nonsense in BASIC`.

Never edit `results.tsv` by hand. Change `cases.bas` and capture again.

## Capture

| | |
|---|---|
| ROM | `48.rom`, SHA1 `5ea7c2b824672e914525d1d5c419d71b84a426a2` (the script refuses any other) |
| Emulator | `@emu198x/zx-spectrum` 0.4.0, headless |
| Tokeniser | `format198x-sinclair-zx-spectrum-bas` 0.1.2, `examples/tokenise_listing` (build it at the tag `format198x-sinclair-zx-spectrum-bas-v0.1.2`) |
| Captured | 2026-09-26 |

```bash
EMU198X_ZX_SPECTRUM_PKG='<path to the @emu198x/zx-spectrum package>' \
SPECTRUM_48K_ROM='<path to the 48K ROM>' \
TOKENISE_LISTING='<path to tokenise_listing>' \
node crates/build198x/tests/fixtures/rom-statement-keyword/capture.mjs
```

For each line, the script tokenises it with the same tokeniser build198x uses
and wraps the bytes in a TAP. Then it runs two tests on a fresh machine each time:

- **editor**: `LOAD ""` with no auto-run, then EDIT (Caps Shift+1) and ENTER.
  ENTER makes the ROM syntax-check the line, exactly as it checks a typed one. A
  refused line stays in the edit area, which the script reads from `E_LINE` to
  `WORKSP`.
- **run**: the same bytes auto-run from their line number. The column holds the
  bottom row after 3 s, or `(running)` if the program is still going.

## Reading it

The ROM's statement loop (STMT-L-1, `1B29`) skips a `:` and stops at the end of
the line. Otherwise it gives report C when a statement's first code is below
`0xCE` (DEF FN). IF-1 (`1D00`) sends the rest of the line after THEN back
there. The source is Logan and O'Hara, *The Complete Spectrum ROM Disassembly*.

`10 5` is the one line the editor cannot refuse. EDIT shows it as `105`, which
the editor reads as "delete line 105". It still stops with report C on RUN, so
the run column is the one the test holds the rule to.
