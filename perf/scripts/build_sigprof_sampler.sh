#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
output="$SCRIPT_DIR/llg_sigprof.so"
cc=${CC:-cc}

while (($# > 0)); do
    case $1 in
        --output) output=$2; shift 2 ;;
        --cc) cc=$2; shift 2 ;;
        -h|--help)
            printf 'usage: %s [--output PATH] [--cc COMPILER]\n' "$0"
            exit 0
            ;;
        *) printf 'build_sigprof_sampler: unknown option: %s\n' "$1" >&2; exit 2 ;;
    esac
done

mkdir -p -- "$(dirname -- "$output")"
"$cc" -std=c11 -O2 -Wall -Wextra -Wpedantic -fPIC -shared \
    "$SCRIPT_DIR/sigprof_sampler.c" -o "$output"
printf '%s\n' "$output"
