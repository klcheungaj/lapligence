#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd -- "$SCRIPT_DIR/../.." && pwd)
CORPUS_DIR="$REPO_ROOT/perf/corpus"

if [[ ${1:-} == --cmake-build ]]; then
    shift
    cmake_program=$1
    source_dir=$2
    build_dir=$3
    compiler=$4
    cflags=$5
    "$cmake_program" -S "$source_dir" -B "$build_dir" \
        -DCMAKE_BUILD_TYPE=Release \
        -DCMAKE_C_COMPILER="$compiler" \
        -DCMAKE_C_FLAGS_RELEASE:STRING="$cflags -DNDEBUG"
    "$cmake_program" --build "$build_dir" --config Release
    exit 0
fi

usage() {
    cat <<'EOF'
Usage: perf/scripts/corpus.sh [options]

Build and measure the stackless-coroutine performance corpus.

Options:
  --sim-bin PATH       llg binary to use (required)
  --output-dir PATH    TSV and per-run logs (required)
  --scratch-dir PATH   Parent for generated models (default: $TMPDIR or /tmp)
  --runs N             Repetitions per configuration and mode (default: 3)
  --config NAME        Select one named configuration; repeatable
  --size SET           Select smoke or standard configurations (default: standard)
  --many-size N        Add both many-process variants at an exact size, 10k..1M
  --mode MODE          default, no-opt, or both (default: default)
  --cc PATH            C compiler for generated models (default: $CC or cc)
  --cmake PATH         CMake executable (default: cmake)
  --cflags FLAGS       Generated-C flags (default: -O3 -Wall -Wno-unused-function)
  --keep-scratch       Retain generated models and print their location
  --list-configs       List named configurations and exit
  -h, --help           Show this help

The report is <output-dir>/results.tsv. Each configuration is generated and
built once; every simulation run records wall/RSS measurements and a SHA-256
of stdout. Generation and build measurements are repeated on the run rows so
each TSV row remains self-contained.
EOF
}

die() {
    printf 'corpus: %s\n' "$*" >&2
    exit 2
}

declare -A CONFIG_FILE CONFIG_TOP CONFIG_DEFINES CONFIG_SET

add_config() {
    local name=$1 file=$2 top=$3 defines=$4 sets=$5
    CONFIG_FILE[$name]=$file
    CONFIG_TOP[$name]=$top
    CONFIG_DEFINES[$name]=$defines
    CONFIG_SET[$name]=$sets
}

add_config many-registers-smoke many_processes.sv many_processes_registers_config \
    'LLG_CORPUS_N=100 LLG_CORPUS_EDGES=4' smoke
add_config many-masked-smoke many_processes.sv many_processes_masked_config \
    'LLG_CORPUS_N=100 LLG_CORPUS_EDGES=4' smoke
add_config tasks-smoke testbench_tasks.sv testbench_tasks \
    'LLG_CORPUS_N=16 LLG_CORPUS_ITERS=1' smoke
add_config zero-delay-smoke zero_delay_churn.sv zero_delay_churn \
    'LLG_CORPUS_N=100 LLG_CORPUS_ROUNDS=10' smoke
add_config wide-values-smoke wide_values.sv wide_values \
    'LLG_CORPUS_N=16 LLG_CORPUS_WIDTH=256 LLG_CORPUS_ROUNDS=2' smoke

add_config many-registers-20k many_processes.sv many_processes_registers_config \
    'LLG_CORPUS_N=20000 LLG_CORPUS_EDGES=20' standard
add_config many-masked-20k many_processes.sv many_processes_masked_config \
    'LLG_CORPUS_N=20000 LLG_CORPUS_EDGES=20' standard
add_config tasks-default testbench_tasks.sv testbench_tasks \
    'LLG_CORPUS_N=128 LLG_CORPUS_ITERS=2000' standard
add_config zero-delay-default zero_delay_churn.sv zero_delay_churn \
    'LLG_CORPUS_N=10000 LLG_CORPUS_ROUNDS=200' standard
add_config wide-values-default wide_values.sv wide_values \
    'LLG_CORPUS_N=128 LLG_CORPUS_WIDTH=4096 LLG_CORPUS_ROUNDS=2000' standard

list_configs() {
    printf 'name\tset\ttop\tdefines\n'
    local name
    while IFS= read -r name; do
        printf '%s\t%s\t%s\t%s\n' "$name" "${CONFIG_SET[$name]}" \
            "${CONFIG_TOP[$name]}" "${CONFIG_DEFINES[$name]}"
    done < <(printf '%s\n' "${!CONFIG_FILE[@]}" | LC_ALL=C sort)
}

sim_bin=
output_dir=
scratch_parent=${TMPDIR:-/tmp}
runs=3
size_set=standard
mode=default
cc=${CC:-cc}
cmake_program=${LLG_CMAKE:-cmake}
cflags='-O3 -Wall -Wno-unused-function'
keep_scratch=0
many_size=
declare -a requested_configs=()

while (($# > 0)); do
    case $1 in
        --sim-bin) (($# >= 2)) || die '--sim-bin requires a path'; sim_bin=$2; shift 2 ;;
        --output-dir) (($# >= 2)) || die '--output-dir requires a path'; output_dir=$2; shift 2 ;;
        --scratch-dir) (($# >= 2)) || die '--scratch-dir requires a path'; scratch_parent=$2; shift 2 ;;
        --runs) (($# >= 2)) || die '--runs requires a value'; runs=$2; shift 2 ;;
        --config) (($# >= 2)) || die '--config requires a name'; requested_configs+=("$2"); shift 2 ;;
        --size) (($# >= 2)) || die '--size requires smoke or standard'; size_set=$2; shift 2 ;;
        --many-size) (($# >= 2)) || die '--many-size requires a value'; many_size=$2; shift 2 ;;
        --mode) (($# >= 2)) || die '--mode requires a value'; mode=$2; shift 2 ;;
        --cc) (($# >= 2)) || die '--cc requires a path'; cc=$2; shift 2 ;;
        --cmake) (($# >= 2)) || die '--cmake requires a path'; cmake_program=$2; shift 2 ;;
        --cflags) (($# >= 2)) || die '--cflags requires a value'; cflags=$2; shift 2 ;;
        --keep-scratch) keep_scratch=1; shift ;;
        --list-configs) list_configs; exit 0 ;;
        -h|--help) usage; exit 0 ;;
        *) die "unknown option: $1" ;;
    esac
done

[[ -n $sim_bin ]] || die '--sim-bin is required'
[[ -n $output_dir ]] || die '--output-dir is required'
[[ $runs =~ ^[1-9][0-9]*$ ]] || die '--runs must be a positive integer'
[[ $size_set == smoke || $size_set == standard ]] || die '--size must be smoke or standard'
[[ $mode == default || $mode == no-opt || $mode == both ]] || \
    die '--mode must be default, no-opt, or both'
if [[ -n $many_size ]]; then
    [[ $many_size =~ ^[0-9]+$ ]] || die '--many-size must be an integer'
    ((many_size >= 10000 && many_size <= 1000000)) || \
        die '--many-size must be between 10000 and 1000000'
fi

base_cwd=$PWD
for variable in sim_bin output_dir scratch_parent; do
    value=${!variable}
    if [[ $value != /* ]]; then
        printf -v "$variable" '%s/%s' "$base_cwd" "$value"
    fi
done
[[ -x $sim_bin ]] || die "simulator binary is not executable: $sim_bin"
command -v "$cc" >/dev/null || [[ -x $cc ]] || die "C compiler not found: $cc"
command -v "$cmake_program" >/dev/null || [[ -x $cmake_program ]] || \
    die "CMake not found: $cmake_program"
for command_name in awk sha256sum mktemp; do
    command -v "$command_name" >/dev/null || die "$command_name is required"
done
[[ -r /proc/self/status ]] || die 'Linux /proc is required for RSS measurement'

if ((${#requested_configs[@]} == 0)) && [[ -z $many_size ]]; then
    while IFS= read -r name; do
        [[ ${CONFIG_SET[$name]} == "$size_set" ]] && requested_configs+=("$name")
    done < <(printf '%s\n' "${!CONFIG_FILE[@]}" | LC_ALL=C sort)
fi
if [[ -n $many_size ]]; then
    add_many_configs=0
    ((${#requested_configs[@]} == 0)) && add_many_configs=1
    for variant in registers masked; do
        name="many-${variant}-${many_size}"
        CONFIG_FILE[$name]=many_processes.sv
        CONFIG_TOP[$name]="many_processes_${variant}_config"
        CONFIG_DEFINES[$name]="LLG_CORPUS_N=${many_size} LLG_CORPUS_EDGES=20"
        CONFIG_SET[$name]=custom
        ((add_many_configs)) && requested_configs+=("$name")
    done
fi
for name in "${requested_configs[@]}"; do
    [[ -n ${CONFIG_FILE[$name]+present} ]] || die "unknown configuration: $name"
done

mkdir -p -- "$output_dir" "$scratch_parent"
scratch=$(mktemp -d "$scratch_parent/llg-corpus.XXXXXX")
scratch=$(cd -- "$scratch" && pwd -P)
cleanup() {
    if ((keep_scratch)); then
        printf 'corpus: scratch retained at %s\n' "$scratch" >&2
    else
        rm -rf -- "$scratch"
    fi
}
trap cleanup EXIT

measure_helper="$scratch/perf_measure"
"$cc" -std=c11 -O2 -Wall -Wextra "$SCRIPT_DIR/perf_measure.c" -o "$measure_helper"

report="$output_dir/results.tsv"
printf 'config\tmode\trun\tgeneration_status\tgeneration_ms\tgeneration_rss_kib\tbuild_status\tbuild_ms\tbuild_rss_kib\tsimulation_status\tsimulation_ms\tsimulation_rss_kib\texecutable_bytes\tstdout_sha256\n' >"$report"
{
    printf 'sim_bin\t%s\n' "$sim_bin"
    printf 'sim_version\t'; "$sim_bin" --version
    printf 'cc\t%s\n' "$cc"
    printf 'cc_version\t'; "$cc" --version | head -1
    printf 'cmake\t%s\n' "$cmake_program"
    printf 'cflags\t%s\n' "$cflags"
    printf 'uname\t'; uname -a
    printf 'uptime\t'; uptime
    printf 'scratch\t%s\n' "$scratch"
} >"$output_dir/metadata.tsv"

read_metric() {
    local record=$1
    IFS=$'\t' read -r metric_label metric_status metric_ns metric_rss <"$record"
    metric_ms=$(awk -v ns="$metric_ns" 'BEGIN { printf "%.3f", ns / 1000000 }')
}

declare -a modes
if [[ $mode == both ]]; then
    modes=(default no-opt)
else
    modes=("$mode")
fi

for config in "${requested_configs[@]}"; do
    source_file="$CORPUS_DIR/${CONFIG_FILE[$config]}"
    top=${CONFIG_TOP[$config]}
    read -r -a defines <<<"${CONFIG_DEFINES[$config]}"
    define_args=()
    for define in "${defines[@]}"; do
        define_args+=(--define "$define")
    done

    for optimization in "${modes[@]}"; do
        optimization_args=()
        [[ $optimization == no-opt ]] && optimization_args+=(--no-opt)
        build_name="$config.$optimization.build"
        build_log_dir="$output_dir/logs/$build_name"
        case_dir="$scratch/$build_name"
        out_root="$case_dir/generated"
        mkdir -p -- "$build_log_dir" "$case_dir"

        generation_record="$case_dir/generation.tsv"
        generation_status=0
        if "$measure_helper" --label generation --record "$generation_record" -- \
            "$sim_bin" --gen-only --out-dir "$out_root" --top "$top" \
            "${optimization_args[@]}" "${define_args[@]}" "$source_file" \
            >"$build_log_dir/generation.stdout" \
            2>"$build_log_dir/generation.stderr"; then
            generation_status=0
        else
            generation_status=$?
        fi
        read_metric "$generation_record"
        generation_ms=$metric_ms
        generation_rss=$metric_rss
        model_dir=$(awk 'NF { line=$0 } END { print line }' \
            "$build_log_dir/generation.stdout")

        build_status=1 build_ms=0 build_rss=0 executable_bytes=0
        executable=
        if ((generation_status == 0)) && [[ -f $model_dir/CMakeLists.txt ]]; then
            build_record="$case_dir/build.tsv"
            if "$measure_helper" --label build --record "$build_record" -- \
                "$SCRIPT_DIR/corpus.sh" --cmake-build "$cmake_program" "$model_dir" \
                "$model_dir/build" "$cc" "$cflags" \
                >"$build_log_dir/build.stdout" \
                2>"$build_log_dir/build.stderr"; then
                build_status=0
            else
                build_status=$?
            fi
            read_metric "$build_record"
            build_ms=$metric_ms
            build_rss=$metric_rss
            executable="$model_dir/build/bin/sim"
            if ((build_status == 0)) && [[ -x $executable ]]; then
                executable_bytes=$(wc -c <"$executable")
                printf '%s\n' "$executable" >"$build_log_dir/executable.path"
            fi
        fi

        if ((generation_status != 0 || build_status != 0)); then
            printf 'corpus: %s failed (generation=%d build=%d); see %s\n' \
                "$build_name" "$generation_status" "$build_status" \
                "$build_log_dir" >&2
            exit 1
        fi

        for ((run = 1; run <= runs; run++)); do
            case_name="$config.$optimization.run$run"
            log_dir="$output_dir/logs/$case_name"
            mkdir -p -- "$log_dir"
            simulation_status=1 simulation_ms=0 simulation_rss=0 stdout_hash=-
            run_record="$case_dir/simulation.run$run.tsv"
            if LLG_SIM_OUT_DIR="$log_dir" \
                "$measure_helper" --label simulation --record "$run_record" -- \
                "$executable" >"$log_dir/simulation.stdout" \
                2>"$log_dir/simulation.stderr"; then
                simulation_status=0
            else
                simulation_status=$?
            fi
            read_metric "$run_record"
            simulation_ms=$metric_ms
            simulation_rss=$metric_rss
            stdout_hash=$(sha256sum "$log_dir/simulation.stdout" | awk '{print $1}')
            printf '%s\n' "$executable" >"$log_dir/executable.path"

            printf '%s\t%s\t%d\t%d\t%s\t%s\t%d\t%s\t%s\t%d\t%s\t%s\t%s\t%s\n' \
                "$config" "$optimization" "$run" \
                "$generation_status" "$generation_ms" "$generation_rss" \
                "$build_status" "$build_ms" "$build_rss" \
                "$simulation_status" "$simulation_ms" "$simulation_rss" \
                "$executable_bytes" "$stdout_hash" >>"$report"

            if ((simulation_status != 0)); then
                printf 'corpus: %s failed (simulation=%d); see %s\n' \
                    "$case_name" "$simulation_status" "$log_dir" >&2
                exit 1
            fi
        done
        if ((keep_scratch == 0)); then
            rm -rf -- "$case_dir"
        fi
    done
done

"$SCRIPT_DIR/corpus_summary.py" "$report" "$output_dir/medians.tsv"
printf 'corpus: report: %s\n' "$report"
printf 'corpus: medians: %s\n' "$output_dir/medians.tsv"
