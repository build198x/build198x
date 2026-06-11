#!/usr/bin/env bash
# Regenerates the Emu198x smoke-test fixtures from the deterministic source
# image. The per-format CLI reports are the version manifest: they record
# converter_version, mediaspec_version, the palette interpretation, and every
# option used — the Emu198x harness asserts those against its pinned mediaspec
# rev before comparing pixels (skip-with-reason on mismatch, never a silent
# stale-golden pass).
#
# Determinism contract: same source + versions + flags => byte-identical
# fixtures on every platform. An unexplained diff here is a determinism bug,
# not a regen chore (decisions/determinism-contract.md).
set -euo pipefail

cd "$(dirname "$0")/.."
OUT="crates/build198x/tests/fixtures/emu-smoke"
SRC="$OUT/source.png"

cargo build --release -p build198x
BIN="target/release/build198x"

mkdir -p "$OUT"
cargo run --release -p build198x --example gen_smoke_source -- "$SRC"

run() { # run <format> <machine> <ext> [extra flags...]
  local format="$1" machine="$2" ext="$3"
  shift 3
  "$BIN" image "$SRC" --machine "$machine" --format "$format" \
    -o "$OUT/smoke.$ext" --report "$OUT/report-$format.json" --force "$@"
}

run scr sinclair-zx-spectrum scr
run koala commodore-c64 koa
run art-studio commodore-c64 art
run ilbm commodore-amiga-ocs iff --mode lores-pal

(cd "$OUT" && shasum -a 256 smoke.* source.png > SHA256SUMS)

echo "--- fixtures regenerated:"
cat "$OUT/SHA256SUMS"
