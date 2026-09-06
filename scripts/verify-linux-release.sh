#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 2 ]]; then
  echo "usage: $0 {x86_64|arm64} BINARY..." >&2
  exit 2
fi

architecture=$1
shift

case "$architecture" in
  x86_64) machine_pattern='ELF 64-bit LSB.*x86-64' ;;
  arm64) machine_pattern='ELF 64-bit LSB.*ARM aarch64' ;;
  *) echo "error: unsupported Linux architecture: $architecture" >&2; exit 2 ;;
esac

for tool in file readelf; do
  command -v "$tool" >/dev/null || {
    echo "error: required verification tool is unavailable: $tool" >&2
    exit 1
  }
done

for binary in "$@"; do
  [[ -x "$binary" ]] || {
    echo "error: Linux executable is missing: $binary" >&2
    exit 1
  }
  file "$binary" | grep -Eq "$machine_pattern" || {
    echo "error: $binary is not an ELF $architecture executable" >&2
    exit 1
  }
  if readelf --wide --program-headers "$binary" | grep -Eq '^[[:space:]]*INTERP[[:space:]]'; then
    echo "error: $binary contains a dynamic interpreter" >&2
    exit 1
  fi
  if readelf --wide --dynamic "$binary" 2>/dev/null | grep -q '(NEEDED)'; then
    echo "error: $binary contains a dynamic-library dependency" >&2
    exit 1
  fi
done

echo "verified fully static Linux $architecture executables: $*"
