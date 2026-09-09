#!/bin/sh
# clean.sh — reclaim disk space from llg build/cache artifacts.
#
# What accumulates and why: cargo keys the Slang CMake build under
# target/slang/<triple>/<profile>/<workspace-path-key>/ so switching triples,
# profiles, or container mount paths can accumulate full copies (gigabytes
# each); simulator model outputs pile up one directory per design under
# target/sim/. None of these are garbage-collected by cargo.
#
# Removes, by default:
#   (a) target/slang/<triple>/<profile>/     for every triple/profile EXCEPT the
#       currently selected one ($CARGO_BUILD_TARGET, else the uncommented
#       `[build] target` from .cargo/config.toml, else `rustc -vV` host
#       triple; $CARGO_BUILD_PROFILE, else "debug")
#   (b) target/sim/<design>/                generated simulator model trees
# Options:
#   --dry-run   list what would be removed (with sizes); delete nothing
#   --all       also remove the currently selected target/slang tree
#               (forces a full Slang rebuild on next build)
#   --logs      additionally remove stale *.log files at depth <= 2 under the
#               repo root and target/
#   --help      this text
#
# Never touched: cargo's own profile trees (target/debug, target/release,
# target/<triple>/...), vendor/, tests/, docs/.  Every path handed to rm is
# derived from this script's location plus fixed names and must resolve inside
# the repository; no user-supplied paths ever reach rm.
#
# Usage: scripts/clean.sh [--dry-run] [--all] [--logs]

set -eu

repo=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
target=$repo/target

dry=0
all=0
logs=0
usage() {
    sed -n '2,/^$/p' "$0" | sed 's/^# \{0,1\}//'
    exit "${1:-0}"
}
while [ $# -gt 0 ]; do
    case $1 in
        --dry-run) dry=1 ;;
        --all) all=1 ;;
        --logs) logs=1 ;;
        --help|-h) usage 0 ;;
        *) printf 'clean.sh: unknown option: %s (try --help)\n' "$1" >&2; exit 2 ;;
    esac
    shift
done

removed_bytes_before=$(du -sh "$target" 2>/dev/null | cut -f1 || true)

# Remove one path if it is a non-empty relative-to-repo fixed artifact.
remove() {
    path=$1
    label=${2:-}
    case $path in
        "$repo"/*) ;;                      # only artifacts inside the repo
        *) printf 'clean.sh: refusing path outside repo: %s\n' "$path" >&2; return 0 ;;
    esac
    [ -e "$path" ] || return 0
    size=$(du -sh "$path" 2>/dev/null | cut -f1)
    if [ "$dry" = 1 ]; then
        printf 'would remove  %8s  %s%s\n' "$size" "${path#"$repo"/}" "${label:+  ($label)}"
    else
        printf 'removing      %8s  %s%s\n' "$size" "${path#"$repo"/}" "${label:+  ($label)}"
        rm -rf -- "$path"
    fi
}

printf 'repository: %s\n' "$repo"
[ "$dry" = 1 ] && printf 'mode: DRY RUN (nothing deleted)\n'

# ---- (a) target/slang: keep only the selected triple/profile unless --all
if [ -d "$target/slang" ]; then
    cur_triple=${CARGO_BUILD_TARGET:-}
    if [ -z "$cur_triple" ]; then
        # Honor a configured default target so its tree is never deleted as
        # "unselected": first uncommented target = "…" under [build].
        for cfg in "$repo/.cargo/config.toml" "$repo/.cargo/config"; do
            [ -f "$cfg" ] || continue
            cur_triple=$(sed -n '/^\[build\][[:space:]]*$/,/^\[/s/^[[:space:]]*target[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$cfg" 2>/dev/null | head -n 1)
            if [ -n "$cur_triple" ]; then break; fi
        done
    fi
    if [ -z "$cur_triple" ]; then
        cur_triple=$(rustc -vV 2>/dev/null | sed -n 's/^host: //p')
    fi
    cur_profile=${CARGO_BUILD_PROFILE:-debug}
    if [ "$all" != 1 ] && [ -z "$cur_triple" ]; then
        printf 'clean.sh: cannot determine the host triple (no rustc?)\n' >&2
        printf 'clean.sh: set CARGO_BUILD_TARGET or pass --all to remove everything\n' >&2
        exit 1
    fi
    for tdir in "$target/slang"/*/; do
        [ -d "$tdir" ] || continue
        triple=${tdir#"$target/slang/"}
        triple=${triple%/}
        for pdir in "$tdir"*/; do
            [ -d "$pdir" ] || continue
            profile=${pdir%"${pdir##*[!/]}"}   # strip trailing slash(es)
            profile=${profile##*/}
            rel=$triple/$profile
            if [ "$all" = 1 ]; then
                remove "${pdir%/}" "slang build $rel (forced by --all)"
            elif [ "$rel" = "$cur_triple/$cur_profile" ]; then
                printf 'keeping       %8s  slang/%s (selected)\n' \
                    "$(du -sh "$pdir" 2>/dev/null | cut -f1)" "$rel"
            else
                remove "${pdir%/}" "slang build $rel (selected: $cur_triple/$cur_profile)"
            fi
        done
    done
fi

# ---- (b) target/sim: generated model sources + CMake trees (dir itself stays)
if [ -d "$target/sim" ]; then
    for dir in "$target/sim"/*/; do
        [ -d "$dir" ] || continue
        remove "${dir%/}" "simulator model output"
    done
fi

# ---- (c) optional: stale logs near the root
if [ "$logs" = 1 ]; then
    find "$repo" -maxdepth 2 -name '*.log' -type f 2>/dev/null | while IFS= read -r f; do
        remove "$f" "stale log"
    done
fi

if [ "$dry" != 1 ]; then
    printf 'done. target size: %s -> %s\n' \
        "${removed_bytes_before:-?}" "$(du -sh "$target" 2>/dev/null | cut -f1 || echo '?')"
fi
