#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
SCRIPT_PATH="$SCRIPT_DIR/$(basename -- "${BASH_SOURCE[0]}")"
REPO_ROOT=$(cd -- "$SCRIPT_DIR/../.." && pwd)

quote_sh() {
    local value=$1
    printf '%q' "$value"
}

now_ns() {
    date +%s%N
}

measure_child() {
    local label=
    local record=

    while (($# > 0)); do
        case $1 in
            --label)
                label=$2
                shift 2
                ;;
            --record)
                record=$2
                shift 2
                ;;
            --)
                shift
                break
                ;;
            *)
                printf 'perf_baseline: measure: unexpected argument: %s\n' "$1" >&2
                return 2
                ;;
        esac
    done

    if [[ -z $label || -z $record || $# -eq 0 ]]; then
        printf 'perf_baseline: measure: --label, --record, and a command are required\n' >&2
        return 2
    fi

    if [[ -n ${PERF_MEASURE_HELPER:-} ]]; then
        "$PERF_MEASURE_HELPER" --label "$label" --record "$record" -- "$@"
        return $?
    fi

    mkdir -p -- "$(dirname -- "$record")"

    local start_ns end_ns status=0 peak_rss=0 rss state
    local pid status_file stat_file
    start_ns=$(now_ns)
    "$@" &
    pid=$!
    status_file="/proc/$pid/status"
    stat_file="/proc/$pid/stat"

    # VmHWM is the process high-water RSS, so sampling it does not require
    # catching the exact allocation peak. This is intentionally Linux-only;
    # the main mode checks /proc before starting a baseline.
    while [[ -r $status_file ]]; do
        rss=$(awk '$1 == "VmHWM:" { print $2; found = 1 } END { if (!found) print 0 }' "$status_file" 2>/dev/null || printf '0')
        if [[ $rss =~ ^[0-9]+$ ]] && ((rss > peak_rss)); then
            peak_rss=$rss
        fi
        state=$(awk '{ print $3 }' "$stat_file" 2>/dev/null || true)
        [[ $state == Z ]] && break
        sleep 0.01
    done

    if wait "$pid"; then
        status=0
    else
        status=$?
    fi
    end_ns=$(now_ns)

    local temporary_record="${record}.tmp.$$"
    printf '%s\t%s\t%s\t%s\n' "$label" "$status" "$((end_ns - start_ns))" "$peak_rss" >"$temporary_record"
    mv -- "$temporary_record" "$record"
    return "$status"
}

run_wrapper() {
    local record=
    while (($# > 0)); do
        case $1 in
            --record)
                record=$2
                shift 2
                ;;
            --)
                shift
                break
                ;;
            *)
                printf 'perf_baseline: run wrapper: unexpected argument: %s\n' "$1" >&2
                return 2
                ;;
        esac
    done

    if [[ -z $record || $# -eq 0 ]]; then
        printf 'perf_baseline: run wrapper: --record and a command are required\n' >&2
        return 2
    fi
    measure_child --label run --record "$record" -- "$@"
}

cc_wrapper() {
    local -a compiler_args=("$@")
    local output=
    local output_index=-1
    local i

    for ((i = 0; i < ${#compiler_args[@]}; i++)); do
        if [[ ${compiler_args[i]} == -o ]] && ((i + 1 < ${#compiler_args[@]})); then
            output=${compiler_args[i + 1]}
            output_index=$((i + 1))
        fi
    done

    if [[ -z $output || $output_index -lt 0 ]]; then
        printf 'perf_baseline: compiler wrapper could not find the -o output path\n' >&2
        return 2
    fi
    if [[ -z ${PERF_METRICS_DIR:-} || -z ${PERF_BASELINE_SCRIPT:-} ]]; then
        printf 'perf_baseline: compiler wrapper is missing its measurement environment\n' >&2
        return 2
    fi

    local real_cc=${PERF_BASELINE_CC:-${CC:-cc}}
    local real_output="${output}.real"
    compiler_args[output_index]=$real_output

    if measure_child --label cc --record "$PERF_METRICS_DIR/cc.tsv" -- "$real_cc" "${compiler_args[@]}"; then
        :
    else
        return $?
    fi
    if [[ ! -x $real_output ]]; then
        printf 'perf_baseline: compiler did not create executable %s\n' "$real_output" >&2
        return 1
    fi

    local run_record="$PERF_METRICS_DIR/run.tsv"
    {
        printf '#!/usr/bin/env bash\n'
        printf 'exec '
        quote_sh "$PERF_BASELINE_SCRIPT"
        printf ' --run-wrapper --record '
        quote_sh "$run_record"
        printf ' -- '
        quote_sh "$real_output"
        printf ' "\$@"\n'
    } >"$output"
    chmod +x -- "$output"
}

if [[ ${1:-} == --measure ]]; then
    shift
    measure_child "$@"
    exit $?
fi

if [[ ${1:-} == --run-wrapper ]]; then
    shift
    run_wrapper "$@"
    exit $?
fi

if [[ ${1:-} == --cc-wrapper ]]; then
    shift
    cc_wrapper "$@"
    exit $?
fi

# Command::new receives LLG_CC as one executable path, so the compiler
# wrapper is selected with an inherited marker rather than an argv suffix.
if [[ ${PERF_BASELINE_CC_WRAPPER:-0} == 1 ]]; then
    cc_wrapper "$@"
    exit $?
fi

usage() {
    cat <<'EOF'
Usage: perf/scripts/perf_baseline.sh [options]

Measure the simulator pipeline on the default performance-design suite.

Options:
  --design PATH       Measure one design; may be repeated.
  --runs N            Repetitions per design (default: 3).
  --top MODULE        Pass --top MODULE to llg.
  --sim-bin PATH      Release llg binary (default: target/release/llg).
  --cc PATH           C compiler used for generated models (default: CC or cc).
  --build             Build target/release/llg before measuring (unmeasured).
  --output PATH       Write the TSV report to PATH instead of stdout.
  --keep              Keep the per-run temporary directories and logs.
  -h, --help          Show this help.
EOF
}

die() {
    printf 'perf_baseline: %s\n' "$*" >&2
    exit 2
}

if [[ ! -r /proc/self/status ]]; then
    die 'Linux /proc is required for RSS measurement'
fi
if ! command -v awk >/dev/null || ! command -v date >/dev/null || ! command -v mktemp >/dev/null; then
    die 'awk, date, and mktemp are required'
fi
if [[ $(now_ns) =~ [^0-9] ]]; then
    die 'date +%s%N is required for nanosecond wall-time measurement'
fi

declare -a designs=()
runs=3
top=
sim_bin="$REPO_ROOT/target/release/llg"
actual_cc=${LLG_BASELINE_CC:-${CC:-cc}}
output_path=-
build_sim=0
keep_work=0
base_cwd=$PWD

while (($# > 0)); do
    case $1 in
        --design)
            (($# >= 2)) || die '--design requires a path'
            designs+=("$2")
            shift 2
            ;;
        --runs)
            (($# >= 2)) || die '--runs requires a positive integer'
            runs=$2
            shift 2
            ;;
        --top)
            (($# >= 2)) || die '--top requires a module name'
            top=$2
            shift 2
            ;;
        --sim-bin)
            (($# >= 2)) || die '--sim-bin requires a path'
            sim_bin=$2
            shift 2
            ;;
        --cc)
            (($# >= 2)) || die '--cc requires a compiler executable'
            actual_cc=$2
            shift 2
            ;;
        --build)
            build_sim=1
            shift
            ;;
        --output)
            (($# >= 2)) || die '--output requires a path'
            output_path=$2
            shift 2
            ;;
        --keep)
            keep_work=1
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

if ! [[ $runs =~ ^[1-9][0-9]*$ ]]; then
    die '--runs must be a positive integer'
fi

if ((build_sim)); then
    (cd -- "$REPO_ROOT" && cargo build --release --no-default-features --bin llg)
fi

if [[ $sim_bin != /* ]]; then
    sim_bin="$base_cwd/$sim_bin"
fi
if [[ ! -x $sim_bin ]]; then
    die "simulator binary is not executable: $sim_bin (run cargo build --release --no-default-features --bin llg)"
fi
if [[ $actual_cc == */* && $actual_cc != /* ]]; then
    actual_cc="$base_cwd/$actual_cc"
fi
if [[ $actual_cc == */* && ! -x $actual_cc ]]; then
    die "C compiler is not executable: $actual_cc"
fi
if [[ $actual_cc != */* ]] && ! command -v "$actual_cc" >/dev/null; then
    die "C compiler not found: $actual_cc"
fi

if ((${#designs[@]} == 0)); then
    designs=(
        "$REPO_ROOT/perf/designs/comb.sv"
        "$REPO_ROOT/perf/designs/cpu.sv"
        "$REPO_ROOT/perf/designs/fifo.sv"
        "$REPO_ROOT/perf/designs/gen16_proto.sv"
        "$REPO_ROOT/perf/designs/uart.sv"
    )
fi

declare -a design_paths=()
for design in "${designs[@]}"; do
    if [[ $design != /* ]]; then
        design="$base_cwd/$design"
    fi
    [[ -f $design ]] || die "design does not exist: $design"
    design_paths+=("$design")
done

work_dir=$(mktemp -d "${TMPDIR:-/tmp}/llg-perf.XXXXXX")
work_dir=$(cd -- "$work_dir" && pwd -P)
cleanup() {
    if ((keep_work)); then
        printf 'perf_baseline: kept work directory: %s\n' "$work_dir" >&2
    else
        rm -rf -- "$work_dir"
    fi
}
trap cleanup EXIT

measure_helper="$work_dir/perf_measure"
if "$actual_cc" -std=c11 -O2 "$SCRIPT_DIR/perf_measure.c" -o "$measure_helper" >"$work_dir/measure-build.log" 2>&1; then
    :
else
    cat "$work_dir/measure-build.log" >&2
    die 'failed to build perf/scripts/perf_measure.c'
fi

report="$work_dir/report.tsv"
printf 'design\trun\tphase\tstatus\twall_ms\trss_kib\n' >"$report"

emit_row() {
    local design=$1
    local run=$2
    local phase=$3
    local status=$4
    local wall_ns=$5
    local rss_kib=$6
    local wall_ms
    wall_ms=$(awk -v ns="$wall_ns" 'BEGIN { printf "%.3f", ns / 1000000 }')
    printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$design" "$run" "$phase" "$status" "$wall_ms" "$rss_kib" >>"$report"
}

design_number=0
for design in "${design_paths[@]}"; do
    design_number=$((design_number + 1))
    design_name=$(basename -- "$design")
    design_name=${design_name%.sv}

    for ((run = 1; run <= runs; run++)); do
        case_dir="$work_dir/design${design_number}-run${run}"
        metrics_dir="$case_dir/metrics"
        mkdir -p -- "$metrics_dir"
        stdout_log="$case_dir/sim.stdout"
        stderr_log="$case_dir/sim.stderr"
        sim_status=0

        sim_args=()
        if [[ -n $top ]]; then
            sim_args+=(--top "$top")
        fi
        sim_args+=("$design")

        if (
            cd -- "$case_dir"
            export PERF_BASELINE_SCRIPT="$SCRIPT_PATH"
            export PERF_BASELINE_CC="$actual_cc"
            export PERF_BASELINE_CC_WRAPPER=1
            export PERF_MEASURE_HELPER="$measure_helper"
            export PERF_METRICS_DIR="$metrics_dir"
            export LLG_CC="$SCRIPT_PATH"
            "$SCRIPT_PATH" --measure --label total --record "$metrics_dir/total.tsv" -- "$sim_bin" "${sim_args[@]}"
        ) >"$stdout_log" 2>"$stderr_log"; then
            sim_status=0
        else
            sim_status=$?
        fi

        if [[ ! -s $metrics_dir/total.tsv ]]; then
            printf 'perf_baseline: no total measurement for %s run %d\n' "$design_name" "$run" >&2
            sed -n '1,120p' "$stderr_log" >&2 || true
            exit 1
        fi
        if [[ ! -s $metrics_dir/cc.tsv || ! -s $metrics_dir/run.tsv ]]; then
            printf 'perf_baseline: missing cc/run measurement for %s run %d\n' "$design_name" "$run" >&2
            sed -n '1,120p' "$stderr_log" >&2 || true
            exit 1
        fi

        IFS=$'\t' read -r total_label total_status total_wall total_rss <"$metrics_dir/total.tsv"
        IFS=$'\t' read -r cc_label cc_status cc_wall cc_rss <"$metrics_dir/cc.tsv"
        IFS=$'\t' read -r run_label run_status run_wall run_rss <"$metrics_dir/run.tsv"
        frontend_wall=$((total_wall - cc_wall - run_wall))
        if ((frontend_wall < 0)); then
            frontend_wall=0
        fi

        emit_row "$design_name" "$run" compile_codegen "$total_status" "$frontend_wall" "$total_rss"
        emit_row "$design_name" "$run" cc "$cc_status" "$cc_wall" "$cc_rss"
        emit_row "$design_name" "$run" run "$run_status" "$run_wall" "$run_rss"
        emit_row "$design_name" "$run" total "$total_status" "$total_wall" "$total_rss"

        if ((sim_status != 0)); then
            printf 'perf_baseline: %s run %d failed with exit %d\n' "$design_name" "$run" "$sim_status" >&2
            sed -n '1,120p' "$stderr_log" >&2 || true
            sed -n '1,120p' "$stdout_log" >&2 || true
            exit 1
        fi
    done
done

if [[ $output_path == - ]]; then
    cat -- "$report"
else
    if [[ $output_path != /* ]]; then
        output_path="$base_cwd/$output_path"
    fi
    mkdir -p -- "$(dirname -- "$output_path")"
    mv -- "$report" "$output_path"
    printf 'perf_baseline: report: %s\n' "$output_path"
fi
