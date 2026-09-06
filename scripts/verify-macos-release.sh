#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "usage: $0 BINARY..." >&2
  exit 2
fi

for tool in file otool; do
  command -v "$tool" >/dev/null || {
    echo "error: required verification tool is unavailable: $tool" >&2
    exit 1
  }
done

for binary in "$@"; do
  [[ -x "$binary" ]] || {
    echo "error: macOS executable is missing: $binary" >&2
    exit 1
  }
  file "$binary" | grep -q 'Mach-O 64-bit executable arm64' || {
    echo "error: $binary is not a macOS arm64 executable" >&2
    exit 1
  }
  while IFS= read -r dependency; do
    [[ -z "$dependency" ]] && continue
    case "$dependency" in
      /usr/lib/* | /System/Library/*) ;;
      *)
        echo "error: $binary has a non-system dynamic dependency: $dependency" >&2
        exit 1
        ;;
    esac
  done < <(otool -L "$binary" | tail -n +2 | awk '{print $1}')
done

echo "verified macOS arm64 executables with only system dynamic libraries: $*"
