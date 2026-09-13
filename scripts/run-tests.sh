#!/usr/bin/env bash
# Parallel test flow for the llg workspace.
#
# Usage:
#   scripts/run-tests.sh                          # run the whole suite
#   scripts/run-tests.sh --test sim_counter       # subset; args pass through
#   scripts/run-tests.sh --test model_tests --test elab_resolve
#
# Uses cargo-nextest: every test runs in its own process and the many
# integration-test binaries execute concurrently (see .config/nextest.toml).
#
# Install nextest with: cargo install cargo-nextest --locked
set -euo pipefail

cd "$(dirname "$0")/.."

if ! cargo nextest --version >/dev/null 2>&1; then
    echo "error: cargo-nextest is required; install it with:" >&2
    echo "       cargo install cargo-nextest --locked" >&2
    exit 2
fi

exec cargo nextest run --locked "$@"
