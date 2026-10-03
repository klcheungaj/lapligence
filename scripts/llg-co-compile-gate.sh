#!/usr/bin/env bash
# Phase 1 llg_co compile gate: build llg_co.c, its include-only consumer and
# the runtime with each named C compiler under strict C11 warnings.
#
#   scripts/llg-co-compile-gate.sh gcc clang
set -euo pipefail

if [[ $# -eq 0 ]]; then
  echo "usage: $0 <compiler>..." >&2
  exit 2
fi

gate_dir=$(mktemp -d)
trap 'rm -rf "$gate_dir"' EXIT

for compiler in "$@"; do
  command -v "$compiler"
  jump_flag=()
  if [[ "$compiler" == gcc ]]; then
    jump_flag=(-Werror=jump-misses-init)
  fi
  for variant in plain debug host debug-host; do
    definitions=()
    case "$variant" in
      debug) definitions=(-DLLG_CO_DEBUG) ;;
      host) definitions=(-DLLG_CO_HOST_ALLOC) ;;
      debug-host) definitions=(-DLLG_CO_DEBUG -DLLG_CO_HOST_ALLOC) ;;
    esac
    "$compiler" -std=c11 -Wall -Wextra -Wpedantic -Werror -Wunused-function \
      "${jump_flag[@]}" "${definitions[@]}" -Isrc/sim/rt -c \
      src/sim/rt/llg_co.c -o "$gate_dir/$compiler-$variant-library.o"
    "$compiler" -std=c11 -Wall -Wextra -Wpedantic -Werror -Wunused-function \
      "${jump_flag[@]}" "${definitions[@]}" -Isrc/sim/rt -c \
      tests/runtime_value_storage/llg_co_include_only.c \
      -o "$gate_dir/$compiler-$variant-include.o"
  done
  "$compiler" -std=c11 -O2 -Wall -Wno-unused-function \
    -Isrc/sim/rt -c src/sim/rt/llg_rt.c \
    -o "$gate_dir/$compiler-runtime.o"
done
