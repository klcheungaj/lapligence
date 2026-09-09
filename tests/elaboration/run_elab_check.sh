#!/usr/bin/env bash
# run_elab_check.sh — verify Slang elaboration output on the test designs.
#
# For each design, runs `elab_check` and prints the owned semantic summary:
# instance tree, resolved parameters, database size, and ref-binding ratio.
#
# Usage: run_elab_check.sh [path-to-elab_check-binary]

set -eu

ELAB_CHECK="${1:-$(dirname "$0")/../../target/debug/elab_check}"
DIR="$(cd "$(dirname "$0")" && pwd)"

if [[ ! -x "$ELAB_CHECK" ]]; then
    echo "elab_check binary not found at $ELAB_CHECK" >&2
    echo "build it with: cargo build --bin elab_check" >&2
    exit 1
fi

run() {
    local top="$1" file="$2"
    echo "===================== $top ($file) ====================="
    "$ELAB_CHECK" --top "$top" "$DIR/$file" 2>/dev/null
    echo
}

run top   top.sv
run top2  top2.sv
run tb    top3.sv
run top4  top4.sv
run top5  top5.sv
run param_top params.sv
