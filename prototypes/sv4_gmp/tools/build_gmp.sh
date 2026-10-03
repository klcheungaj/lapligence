#!/bin/sh
set -eu
if [ "$#" -lt 3 ] || [ "$#" -gt 4 ]; then
    echo "usage: sh build_gmp.sh GMP_SOURCE BUILD_DIRECTORY INSTALL_PREFIX [JOBS]" >&2
    exit 2
fi
source_dir=$(cd "$1" && pwd)
mkdir -p "$2" "$3"
build_dir=$(cd "$2" && pwd)
prefix=$(cd "$3" && pwd)
jobs=${4:-2}
case "$jobs" in ''|*[!0-9]*|0) echo "JOBS must be positive" >&2; exit 2;; esac
if [ "$source_dir" = "$build_dir" ]; then
    echo "Use a separate GMP build directory" >&2; exit 2
fi
cd "$build_dir"
"$source_dir/configure" --prefix="$prefix" --disable-shared --enable-static
make -j"$jobs"
make check -j"$jobs"
make install
