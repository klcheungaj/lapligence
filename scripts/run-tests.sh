#!/usr/bin/env bash
# Parallel test flow for the llg workspace.
#
# Usage:
#   scripts/run-tests.sh                          # run the whole suite
#   scripts/run-tests.sh --test sim_counter       # subset; args pass through
#   scripts/run-tests.sh --test model_tests --test elab_resolve
#
# Uses cargo-nextest when installed: every test runs in its own process and
# the many integration-test binaries execute concurrently (see
# .config/nextest.toml for the profile).  Falls back to plain `cargo test`
# otherwise, which still parallelizes tests within each binary but runs one
# binary after another.
#
# Install nextest with: cargo install cargo-nextest --locked
set -euo pipefail

cd "$(dirname "$0")/.."

if cargo nextest --version >/dev/null 2>&1; then
    exec cargo nextest run "$@"
else
    echo "note: cargo-nextest not found; falling back to 'cargo test' (binaries run serially)" >&2
    echo "      install it with: cargo install cargo-nextest --locked" >&2
    exec cargo test "$@"
fi
