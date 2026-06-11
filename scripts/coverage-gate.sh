#!/usr/bin/env bash
#
# Fail if total line coverage drops below the floor. Reads the JSON
# summary produced by scripts/coverage.sh. The floor is an anti-regression
# ratchet, not a target — raise COVERAGE_GATE_THRESHOLD (and the default
# here) as the suite grows.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

threshold="${COVERAGE_GATE_THRESHOLD:-60}"
summary="target/llvm-cov/coverage-summary.json"

if [ ! -f "${summary}" ]; then
    echo "ERROR: ${summary} not found. Run scripts/coverage.sh first." >&2
    exit 2
fi

python3 - "${summary}" "${threshold}" <<'PY'
import json
import sys
from pathlib import Path

summary = json.loads(Path(sys.argv[1]).read_text())
threshold = float(sys.argv[2])

lines = summary["data"][0]["totals"]["lines"]
pct = float(lines["percent"])
covered, count = lines["covered"], lines["count"]

print(f"Total line coverage: {pct:.2f}% ({covered}/{count} lines)")
print(f"Floor: {threshold:.1f}%")

if pct < threshold:
    print(f"FAIL: line coverage {pct:.2f}% is below the {threshold:.1f}% floor.")
    sys.exit(1)

print("OK: coverage meets the floor.")
PY
