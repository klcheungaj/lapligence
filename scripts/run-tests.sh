#!/usr/bin/env bash
# Parallel test flow for the llg workspace.
#
# Usage:
#   scripts/run-tests.sh                          # run the whole suite
#   scripts/run-tests.sh --test sim_counter       # one suite of a grouped binary
#   scripts/run-tests.sh --test model_tests --test elab_resolve
#   scripts/run-tests.sh --test-work-dir /build --test sim_counter
#   scripts/run-tests.sh --cargo-profile quick --test sim_counter
#
# Uses cargo-nextest: every test runs in its own process and the grouped
# integration-test binaries execute concurrently (see .config/nextest.toml).
# `--test <suite>` for a suite compiled into a group binary becomes that binary
# plus a `test(/^<suite>::/)` filterset (tests/readme.md#test-binaries).
#
# Install nextest with: cargo install cargo-nextest --locked
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
test_work_root=
nextest_args=()

while (($# > 0)); do
    case $1 in
        --sccache)
            export LLG_SCCACHE=1
            shift
            ;;
        --mold)
            export LLG_MOLD=1
            shift
            ;;
        --test-work-dir)
            if (($# < 2)) || [[ -z $2 || $2 == --* ]]; then
                echo 'error: --test-work-dir requires an existing directory' >&2
                exit 2
            fi
            test_work_root=$2
            shift 2
            ;;
        --test-work-dir=*)
            test_work_root=${1#*=}
            if [[ -z $test_work_root ]]; then
                echo 'error: --test-work-dir requires an existing directory' >&2
                exit 2
            fi
            shift
            ;;
        -h|--help)
            cat <<'EOF'
Usage: scripts/run-tests.sh [--test-work-dir PATH] [--sccache] [--mold] [nextest run options]

--sccache            Use sccache for Rust unless RUSTC_WRAPPER is already set.
--mold               Use mold for the Linux GNU host linker, preserving rustflags.
                     These also accept LLG_SCCACHE=1 and LLG_MOLD=1. Missing
                     requested tools fail before Cargo starts. See dev-env.sh
                     for plain Cargo and tests/readme.md for native launchers.

--test-work-dir PATH  Root for temporary test files, generated simulator builds
                      and a shared runtime cache. Must exist and permit execution.
                      Each worktree/run gets isolated scratch space; Cargo outputs
                      stay in its target/. Relative paths use the caller's directory.

Without --test-work-dir, existing environment settings and storage defaults apply.
Test builds use the optimized Cargo test profile by default. Pass --cargo-profile
quick for shorter edit-test rebuilds (artifacts in target/quick/). Nextest's
--profile selects runner settings, independently of the Cargo build profile.
--test SUITE selects one tests/SUITE.rs even though it is a module of a grouped
binary; other nextest options pass through, and -E filtersets narrow the selection.
Nextest concurrency is unchanged (8 tests by default). Use cargo nextest run --help
for nextest help. See tests/readme.md#parallel-worktrees for layout and cleanup rules.
EOF
            exit 0
            ;;
        --)
            nextest_args+=("$@")
            break
            ;;
        *)
            nextest_args+=("$1")
            shift
            ;;
    esac
done

# Integration tests are grouped into a few binaries (tests/readme.md#test-binaries).
# Translate `--test <suite>` for a suite that is a module of a group into the
# group binary plus a test-name filterset, so per-suite selection keeps working.
# Binaries named directly (and --lib/--bins/--bin selections) stay selected
# whole; user filtersets still apply on top of the selection.
translate_suite_args() {
    local -a original=("${nextest_args[@]}") out=() user_filters=() selected=()
    local -A groups=()
    local translated=0 arg name group
    while ((${#nextest_args[@]} > 0)); do
        arg=${nextest_args[0]}
        nextest_args=("${nextest_args[@]:1}")
        case $arg in
            --)
                out+=("$arg" "${nextest_args[@]}")
                nextest_args=()
                break
                ;;
            -E|--filterset|--filter-expr)
                user_filters+=("${nextest_args[0]}")
                nextest_args=("${nextest_args[@]:1}")
                continue
                ;;
            -E=*|--filterset=*|--filter-expr=*)
                user_filters+=("${arg#*=}")
                continue
                ;;
            --lib)
                selected+=("kind(lib)")
                ;;
            --bins)
                selected+=("kind(bin)")
                ;;
            --bin)
                selected+=("binary(=${nextest_args[0]})")
                out+=("$arg" "${nextest_args[0]}")
                nextest_args=("${nextest_args[@]:1}")
                continue
                ;;
            --tests|--all-targets)
                selected+=("all()")
                ;;
            --test|--test=*)
                if [[ $arg == --test ]]; then
                    name=${nextest_args[0]}
                    nextest_args=("${nextest_args[@]:1}")
                else
                    name=${arg#--test=}
                fi
                # The suite's own file may declare a same-named inner module.
                group=$(grep -lx "mod $name;" "$repo_root"/tests/*.rs 2>/dev/null \
                    | grep -vx "$repo_root/tests/$name.rs" | head -n 1 || true)
                if [[ -n $group ]]; then
                    group=$(basename "$group" .rs)
                    translated=1
                    selected+=("test(/^$name::/)")
                else
                    group=$name
                    selected+=("binary(=$name)")
                fi
                if [[ -z ${groups[$group]:-} ]]; then
                    groups[$group]=1
                    out+=(--test "$group")
                fi
                continue
                ;;
        esac
        out+=("$arg")
    done
    if ((!translated)); then
        nextest_args=("${original[@]}")
        return
    fi
    local expr users
    expr=$(printf '%s | ' "${selected[@]}")
    expr=${expr% | }
    if ((${#user_filters[@]} == 1)); then
        expr="($expr) & (${user_filters[0]})"
    elif ((${#user_filters[@]} > 1)); then
        users=$(printf '(%s) | ' "${user_filters[@]}")
        expr="($expr) & (${users% | })"
    fi
    nextest_args=(-E "$expr" "${out[@]}")
}

translate_suite_args

if [[ -n $test_work_root ]]; then
    if [[ ! -d $test_work_root ]]; then
        echo "error: test work directory does not exist: $test_work_root" >&2
        exit 2
    fi
    test_work_root=$(cd -- "$test_work_root" && pwd -P)
fi

cd -- "$repo_root"
source "$repo_root/scripts/dev-env.sh"

if ! cargo nextest --version >/dev/null 2>&1; then
    echo "error: cargo-nextest is required; install it with:" >&2
    echo "       cargo install cargo-nextest --locked" >&2
    exit 2
fi

if [[ -z $test_work_root ]]; then
    exec cargo nextest run --locked "${nextest_args[@]}"
fi

if command -v sha256sum >/dev/null 2>&1; then
    worktree_key=$(printf '%s' "$repo_root" | sha256sum)
elif command -v shasum >/dev/null 2>&1; then
    worktree_key=$(printf '%s' "$repo_root" | shasum -a 256)
else
    echo 'error: --test-work-dir requires sha256sum or shasum' >&2
    exit 2
fi
worktree_key=${worktree_key%% *}
scratch_root="$test_work_root/lapligence/worktrees/$worktree_key"
mkdir -p -- "$scratch_root"
run_dir=$(mktemp -d "$scratch_root/run.XXXXXXXX")

cleanup() {
    local status=$?
    if ((status == 0)); then
        rm -rf -- "$run_dir"
    else
        printf 'run-tests: retained scratch directory: %s\n' "$run_dir" >&2
    fi
}
trap cleanup EXIT

mkdir -- "$run_dir/tests" "$run_dir/tmp"
# Execute a probe directly so a noexec mount fails before Cargo starts building.
printf '#!/bin/sh\nexit 0\n' >"$run_dir/exec-probe"
chmod u+x "$run_dir/exec-probe"
if ! "$run_dir/exec-probe"; then
    echo 'error: test work directory must permit execution; check the noexec mount option' >&2
    exit 2
fi
rm -- "$run_dir/exec-probe"

export LLG_TEST_BUILD_DIR="$run_dir/tests"
export TMPDIR="$run_dir/tmp"
export LLG_RUNTIME_CACHE_DIR="$test_work_root/lapligence/runtime-cache"
export CARGO_TARGET_DIR="$repo_root/target"
export CARGO_BUILD_BUILD_DIR="$repo_root/target"
printf 'run-tests: scratch=%s runtime-cache=%s\n' "$run_dir" "$LLG_RUNTIME_CACHE_DIR" >&2
cargo nextest run --locked "${nextest_args[@]}"
