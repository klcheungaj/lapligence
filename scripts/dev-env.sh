#!/usr/bin/env bash
# Source after exporting LLG_SCCACHE=1 and/or LLG_MOLD=1 for plain Cargo.

llg_dev_env() {
    local root linker host linker_variable native_cache native_setting
    root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P) || return
    case ${LLG_SCCACHE:-0} in
        1)
            command -v sccache >/dev/null || {
                echo 'error: LLG_SCCACHE=1 requires sccache on PATH' >&2
                return 2
            }
            ;;
        0|'') ;;
        *) echo 'error: LLG_SCCACHE must be 0 or 1' >&2; return 2 ;;
    esac
    native_setting=${LLG_CCACHE:-}
    if [[ -n $native_setting ]]; then
        native_setting=$(printf '%s' "$native_setting" | tr '[:upper:]' '[:lower:]') || return
    fi
    case $native_setting in
        1|on|true|ccache) native_cache=ccache ;;
        sccache) native_cache=sccache ;;
        0|off|false|'') native_cache= ;;
        *) echo 'error: LLG_CCACHE must be 0, 1, ccache or sccache' >&2; return 2 ;;
    esac
    if [[ -n $native_cache ]] && ! command -v "$native_cache" >/dev/null; then
        echo "error: LLG_CCACHE requires $native_cache on PATH" >&2
        return 2
    fi
    case ${LLG_MOLD:-0} in
        1)
            if [[ -n ${LLG_MOLD_THREADS:-} && ! $LLG_MOLD_THREADS =~ ^[1-9][0-9]*$ ]]; then
                echo 'error: LLG_MOLD_THREADS must be a positive integer' >&2
                return 2
            fi
            if ! command -v mold >/dev/null; then
                echo 'error: LLG_MOLD=1 requires mold on PATH' >&2
                return 2
            fi
            if ! command -v "${LLG_MOLD_CC:-cc}" >/dev/null; then
                echo 'error: LLG_MOLD=1 requires cc (or LLG_MOLD_CC) on PATH' >&2
                return 2
            fi
            host=$(rustc -vV) || return
            host=${host#*host: }
            host=${host%%$'\n'*}
            case $host in
                *-linux-gnu) ;;
                *) echo 'error: LLG_MOLD=1 supports only a Linux GNU Rust host' >&2; return 2 ;;
            esac
            linker_variable=CARGO_TARGET_$(printf '%s' "$host" | tr '[:lower:]-' '[:upper:]_')_LINKER
            linker=$root/scripts/mold-linker.sh
            if [[ -n ${!linker_variable:-} && ${!linker_variable} != "$linker" ]]; then
                echo "error: LLG_MOLD=1 conflicts with $linker_variable; unset it first" >&2
                return 2
            fi
            ;;
        0|'') ;;
        *) echo 'error: LLG_MOLD must be 0 or 1' >&2; return 2 ;;
    esac
    # Commit environment changes only after every requested tool was checked.
    if [[ ${LLG_SCCACHE:-0} == 1 && -z ${RUSTC_WRAPPER:-} ]]; then
        export RUSTC_WRAPPER="$root/scripts/sccache.sh"
    fi
    if [[ ${LLG_MOLD:-0} == 1 ]]; then
        export "$linker_variable=$linker"
    fi
}

# Do not change the caller's shell options or exit their interactive shell.
if llg_dev_env; then
    unset -f llg_dev_env
else
    unset -f llg_dev_env
    return 2 2>/dev/null || exit 2
fi
