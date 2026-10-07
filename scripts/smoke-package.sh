#!/usr/bin/env bash
set -euo pipefail
package=$1
package_bin=$2
fixture_dir=$3/$package-smoke
mkdir -p "$fixture_dir"
case "$package" in
  build198x)
    printf '10 PRINT "HELLO"\n' > "$fixture_dir/hello.bas"
    "$package_bin/build198x" basic "$fixture_dir/hello.bas" --machine commodore-c64 -o "$fixture_dir/hello.prg"
    # Load address $0801, next line $080f, line 10, PRINT token $99.
    printf '\001\010\017\010\012\000\231\040\042HELLO\042\000\000\000' > "$fixture_dir/expected.prg"
    cmp "$fixture_dir/expected.prg" "$fixture_dir/hello.prg"
    ;;
  build198x-adf)
    printf 'HELLO\n' > "$fixture_dir/readme.txt"
    "$package_bin/build198x-adf" create "$fixture_dir/data.adf" --label Smoke --add "$fixture_dir/readme.txt=readme.txt"
    "$package_bin/build198x-adf" verify "$fixture_dir/data.adf"
    "$package_bin/build198x-adf" info "$fixture_dir/data.adf" > "$fixture_dir/info.txt"
    grep -F 'readme.txt' "$fixture_dir/info.txt"
    [ "$(wc -c < "$fixture_dir/data.adf" | tr -d ' ')" = 901120 ]
    ;;
  *) echo "Unexpected package: $package" >&2; exit 1 ;;
esac
