# Test-card tape

`border.tap` is what `build198x tape` makes when there is no loading-screen
art yet:

```sh
build198x tape --machine sinclair-zx-spectrum --code border.bin --load-address 32768 -o border.tap
```

`border.bin` is six bytes of Z80, `LD A,2 / OUT ($FE),A / JR $`: it turns the
border red and loops. Load the tape on a 48K Spectrum with `LOAD ""` and you
see the built-in test card (eight colour bars, dim above and bright below)
appear while the CODE loads, then the border turn red.

`tests/tape_cli.rs` rebuilds the tape and compares it byte for byte, so this
file changes only when the tape master's output changes on purpose.
