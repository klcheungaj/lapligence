#!/usr/bin/env bash
set -euo pipefail
if ! command -v mold >/dev/null; then
    echo 'error: mold linker wrapper requires mold on PATH' >&2
    exit 2
fi
# Cargo/rustc may supply its own -fuse-ld flag; the explicit opt-in wins.
if [[ -n ${LLG_MOLD_THREADS:-} ]]; then
    if [[ ! $LLG_MOLD_THREADS =~ ^[1-9][0-9]*$ ]]; then
        echo 'error: LLG_MOLD_THREADS must be a positive integer' >&2
        exit 2
    fi
    exec "${LLG_MOLD_CC:-cc}" "$@" "-Wl,--threads=$LLG_MOLD_THREADS" -fuse-ld=mold
fi
exec "${LLG_MOLD_CC:-cc}" "$@" -fuse-ld=mold
