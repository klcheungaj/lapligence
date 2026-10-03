#!/usr/bin/env bash
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
# CMake can set CC to this wrapper during compiler detection.
exec "$root/sccache.sh" "${LLG_SCCACHE_CC:-cc}" "$@"
