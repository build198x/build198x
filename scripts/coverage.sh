#!/usr/bin/env bash
#
# Run the test suite under llvm-cov instrumentation and emit coverage
# reports to target/llvm-cov/. This is also a full test run — pass/fail
# here is the regression signal. Extra args are forwarded to cargo
# llvm-cov (e.g. `--include-ignored`).

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

mkdir -p target/llvm-cov

# No target selectors: plain `cargo test --workspace` semantics instrument
# every test target — lib, bin, and integration alike. (Asm198x's `--lib
# --tests` errors here: the workspace's only crate is a binary, so there is
# no library target to select.)
# `--no-fail-fast` lets every crate's tests run so the report is complete
# even if one fails. `--no-report` defers report generation to the
# explicit invocations below, whose stdout is the formatted summary.
test_status=0
cargo llvm-cov --workspace --no-fail-fast --no-report "$@" || test_status=$?

if [ "${test_status}" -ne 0 ]; then
    echo
    echo "WARNING: tests exited with status ${test_status} — coverage data is" \
         "still complete (--no-fail-fast), but review the failures above."
    echo
fi

cargo llvm-cov report | tee target/llvm-cov/coverage-summary.txt
cargo llvm-cov report --json --summary-only \
    --output-path target/llvm-cov/coverage-summary.json
cargo llvm-cov report --lcov \
    --output-path target/llvm-cov/lcov.info
cargo llvm-cov report --html --output-dir target/llvm-cov

echo
echo "Coverage total:"
grep '^TOTAL' target/llvm-cov/coverage-summary.txt | tail -n 1

# Propagate a genuine test failure to the caller.
exit "${test_status}"
