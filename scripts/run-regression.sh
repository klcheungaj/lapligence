#!/usr/bin/env bash
# Capture the repeatable U05 regression gate and its build provenance.
#
# Usage:
#   scripts/run-regression.sh --label before --output-dir persistence/u05/before
#   scripts/run-regression.sh --label after --output-dir persistence/u05/after
#   diff -u persistence/u05/before/summary.tsv persistence/u05/after/summary.tsv
#
# The default gate is intentionally serialized. Each phase has a durable log,
# while metadata.tsv and summary.tsv remain easy to consume from shell tooling.
# A clean tracked checkout and clean submodules are required unless the caller
# opts into an explicitly marked local run with the allow-dirty flags.

set -Eeuo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd -- "$SCRIPT_DIR/.." && pwd)

label=baseline
output_dir=
allow_dirty_root=0
allow_dirty_submodules=0

usage() {
    cat <<'EOF'
Usage: scripts/run-regression.sh [options]

Run the serialized U05 regression gate and record provenance under persistence/.

Options:
  --label NAME                  Label in metadata (default: baseline).
  --output-dir PATH             Evidence directory (default: persistence/u05/<label>-<UTC>).
  --allow-dirty-root            Permit tracked root edits; marks the run non-reproducible.
  --allow-dirty-submodules      Permit dirty vendor checkouts; marks the run non-reproducible.
  -h, --help                    Show this help.

The command records the root commit, submodule gitlinks and checked-out heads,
Cargo.lock/manifests hashes, tool versions, and every phase status. It runs:
  cargo metadata --locked
  cargo test --locked --all-features -- --list
  cargo test --locked --jobs 2 --all-features -- --test-threads=1
  cargo test --locked --jobs 2 --test sim_opt_differential -- --test-threads=1
  the CI generated-runtime GCC ASan/UBSan matrix
  source-location diagnostic tests (compile_errors, slang_frontend, model_tests)

Run this command once before and once after a patch, using separate output
directories. Compare the two summary.tsv files; test oracles remain owned by
the Rust tests and are never updated by this script.
EOF
}

die() {
    printf 'run-regression: %s\n' "$*" >&2
    exit 2
}

while (($# > 0)); do
    case $1 in
        --label)
            (($# >= 2)) || die '--label requires a value'
            label=$2
            shift 2
            ;;
        --output-dir)
            (($# >= 2)) || die '--output-dir requires a path'
            output_dir=$2
            shift 2
            ;;
        --allow-dirty-root)
            allow_dirty_root=1
            shift
            ;;
        --allow-dirty-submodules)
            allow_dirty_submodules=1
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            die "unknown option: $1"
            ;;
    esac
done

[[ $label =~ ^[[:alnum:]_.-]+$ ]] || die '--label must contain only letters, digits, dot, underscore, or dash'

if [[ -z $output_dir ]]; then
    output_dir="$REPO_ROOT/persistence/u05/$label-$(date -u +%Y%m%dT%H%M%SZ)"
elif [[ $output_dir != /* ]]; then
    output_dir="$REPO_ROOT/$output_dir"
fi

mkdir -p -- "$output_dir"
[[ ! -e "$output_dir/summary.tsv" ]] || die "evidence directory already contains summary.tsv: $output_dir"

cd -- "$REPO_ROOT"

metadata_file="$output_dir/metadata.tsv"
commands_file="$output_dir/commands.tsv"
summary_file="$output_dir/summary.tsv"
timings_file="$output_dir/timings.tsv"

printf 'key\tvalue\n' >"$metadata_file"
printf 'phase\tcommand\n' >"$commands_file"
printf 'phase\tstatus\n' >"$summary_file"
printf 'phase\tseconds\n' >"$timings_file"

record() {
    local key=$1
    local value=${2:-}
    value=${value//$'\t'/\\t}
    value=${value//$'\n'/\\n}
    printf '%s\t%s\n' "$key" "$value" >>"$metadata_file"
}

record_command() {
    local phase=$1
    shift
    local rendered
    printf -v rendered '%q ' "$@"
    rendered=${rendered% }
    printf '%s\t%s\n' "$phase" "$rendered" >>"$commands_file"
}

command_output() {
    local output
    output=$("$@" 2>&1 || true)
    output=${output//$'\n'/'; '}
    output=${output//$'\t'/\\t}
    printf '%s' "$output"
}

record_tool() {
    local name=$1
    shift
    if command -v "$name" >/dev/null 2>&1; then
        record "tool.$name" "$(command_output "$@")"
    else
        record "tool.$name" MISSING
    fi
}

record_hashes() {
    local -a hash_tool
    if command -v sha256sum >/dev/null 2>&1; then
        hash_tool=(sha256sum)
    elif command -v shasum >/dev/null 2>&1; then
        hash_tool=(shasum -a 256)
    else
        die 'sha256sum or shasum is required to record input hashes'
    fi
    local path
    for path in Cargo.toml Cargo.lock .gitmodules rust-toolchain.toml .cargo/config.toml \
        patches/slang/slang-cache-only-source-reads.patch; do
        [[ -f $path ]] || die "required reproducibility input is missing: $path"
        record "sha256.$path" "$("${hash_tool[@]}" "$path")"
    done
}

record_git_state() {
    local root_commit
    root_commit=$(git rev-parse HEAD)
    record root.commit "$root_commit"
    record root.branch "$(git branch --show-current)"
    record root.status "$(git status --porcelain=v1 --untracked-files=all)"
    record root.submodule_status "$(git submodule status --recursive)"

    local tracked_root_changes
    tracked_root_changes=$(git diff --name-status --ignore-submodules=all)
    tracked_root_changes+=$'\n'
    tracked_root_changes+=$(git diff --cached --name-status --ignore-submodules=all)
    if [[ -n ${tracked_root_changes//$'\n'/} ]]; then
        record root.tracked_changes "$tracked_root_changes"
        if ((allow_dirty_root == 0)); then
            die 'tracked root changes found; use --allow-dirty-root only for a marked local run'
        fi
        reproducible=no
    else
        record root.tracked_changes none
    fi

    local path expected actual dirty
    for path in vendor/libaco vendor/slang; do
        expected=$(git ls-tree HEAD -- "$path" | awk '{print $3}')
        [[ $expected =~ ^[0-9a-f]{40}$ ]] || die "HEAD has no gitlink for $path"
        actual=$(git -C "$path" rev-parse HEAD)
        record "gitlink.$path" "$expected"
        record "checkout.$path" "$actual"
        [[ $actual == "$expected" ]] || die "$path is at $actual, expected gitlink $expected"
        dirty=$(git -C "$path" status --porcelain=v1 --untracked-files=all)
        if [[ -n $dirty ]]; then
            record "dirty.$path" "$dirty"
            if ((allow_dirty_submodules == 0)); then
                die "$path has local modifications; use --allow-dirty-submodules only for a marked local run"
            fi
            reproducible=no
        else
            record "dirty.$path" none
        fi
    done
}

record_toolchain() {
    [[ -f rust-toolchain.toml ]] || die 'rust-toolchain.toml is missing'
    local channel
    channel=$(sed -n 's/^[[:space:]]*channel[[:space:]]*=[[:space:]]*"\([0-9][0-9]*\.[0-9][0-9]*\.[0-9][0-9]*\)"[[:space:]]*$/\1/p' rust-toolchain.toml | head -n 1)
    [[ $channel =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die 'rust-toolchain.toml must pin an exact x.y.z channel'
    record toolchain.channel "$channel"
    record toolchain.active "$(command_output rustup show active-toolchain)"
    record toolchain.rustc "$(command_output rustc --version --verbose)"
    record toolchain.cargo "$(command_output cargo --version --verbose)"
}

reproducible=yes
record_git_state
record_hashes
record_toolchain
record_tool git git --version
record_tool cmake cmake --version
record_tool cc cc --version
record_tool python3 python3 --version
record env.CC "${CC-}"
record env.CXX "${CXX-}"
record env.LLG_CC "${LLG_CC-}"
record env.LLG_CFLAGS "${LLG_CFLAGS-}"
record run.label "$label"
record run.output_dir "$output_dir"
record run.started_utc "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
record reproducible "$reproducible"

overall_status=0

run_phase() {
    local phase=$1
    shift
    local log_file="$output_dir/$phase.log"
    local start end elapsed status
    record_command "$phase" "$@"
    printf '[run-regression] %s\n' "$phase"
    start=$(date +%s)
    set +e
    "$@" >"$log_file" 2>&1
    status=$?
    set -e
    end=$(date +%s)
    elapsed=$((end - start))
    printf '%s\t%s\n' "$phase" "$status" >>"$summary_file"
    printf '%s\t%s\n' "$phase" "$elapsed" >>"$timings_file"
    record "phase.$phase.status" "$status"
    record "phase.$phase.seconds" "$elapsed"
    if ((status != 0)); then
        overall_status=1
        printf '[run-regression] %s failed; see %s\n' "$phase" "$log_file" >&2
    else
        printf '[run-regression] %s passed (%ss)\n' "$phase" "$elapsed"
    fi
}

run_phase metadata \
    cargo metadata --locked --format-version 1
run_phase test-inventory \
    cargo test --locked --jobs 2 --all-features -- --list
run_phase full-suite \
    cargo test --locked --jobs 2 --all-features -- --test-threads=1
run_phase optimizer-differential \
    cargo test --locked --jobs 2 --test sim_opt_differential -- --test-threads=1
run_phase generated-runtime-sanitizers \
    env \
    LLG_CC=gcc \
    LLG_CFLAGS='-DACO_USE_ASAN -fsanitize=address,undefined -fno-omit-frame-pointer -fno-sanitize-recover=all' \
    ASAN_OPTIONS='detect_leaks=1:strict_string_checks=1' \
    UBSAN_OPTIONS='print_stacktrace=1:halt_on_error=1' \
    cargo test --locked --jobs 2 --all-features \
    --test runtime_values \
    --test runtime_boundaries \
    --test sim_counter \
    --test sim_data_types \
    --test sim_data_types_next \
    --test sim_data_types_completion \
    --test sim_type_conformance \
    --test sim_partial_features \
    --test sim_net_resolution \
    --test sim_net_defaults \
    --test sim_function \
    --test sim_loops \
    --test sim_procedural_assign \
    -- --test-threads=1
run_phase source-location-diagnostics \
    cargo test --locked --jobs 2 \
    --test compile_errors \
    --test slang_frontend \
    --test model_tests \
    -- --test-threads=1

record run.finished_utc "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
record run.status "$overall_status"
if ((overall_status != 0)); then
    printf '[run-regression] failed; evidence: %s\n' "$output_dir" >&2
    exit "$overall_status"
fi
printf '[run-regression] all phases passed; evidence: %s\n' "$output_dir"
