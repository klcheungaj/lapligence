#!/usr/bin/env bash
set -euo pipefail
if ! command -v sccache >/dev/null; then
    echo 'error: sccache wrapper requires sccache on PATH' >&2
    exit 2
fi
# The per-run test TMPDIR can exceed the Unix socket path limit for the server.
export TMPDIR=/tmp
exec sccache "$@"
