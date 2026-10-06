#!/usr/bin/env python3
"""Generate or check the GMP tables bundled with compact GMP kernels.

GMP derives a few tables from its own generator programs (`gen-*.c` in
`vendor/gmp`). They depend only on the limb and nail sizes, which the compact
kernels fix at 64 and 0, so the outputs are checked in under
`src/sim/rt/gmp/generated/` and the generated CMake builds never run a
generator. Run this script after moving the `vendor/gmp` submodule; `--check`
fails when the checked-in tables differ from the generators' output.
"""

import argparse
import os
import shlex
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
GMP = ROOT / "vendor/gmp"
OUTPUT = ROOT / "src/sim/rt/gmp/generated"
LIMB_BITS = "64"
NAIL_BITS = "0"

# (generator, arguments, output file)
TABLES = (
    ("gen-bases", ("header", LIMB_BITS, NAIL_BITS), "mp_bases.h"),
    ("gen-bases", ("table", LIMB_BITS, NAIL_BITS), "mp_bases.c"),
    ("gen-fac", (LIMB_BITS, NAIL_BITS), "fac_table.h"),
    ("gen-fib", ("header", LIMB_BITS, NAIL_BITS), "fib_table.h"),
    ("gen-sieve", (LIMB_BITS,), "sieve_table.h"),
)


def compiler():
    value = os.environ.get("CC") or shutil.which("cc") or shutil.which("gcc") or shutil.which("clang")
    if not value:
        raise RuntimeError("no C compiler: set CC")
    return shlex.split(value)


def generate(scratch):
    cc = compiler()
    built = {}
    tables = {}
    for program, arguments, output in TABLES:
        if program not in built:
            executable = Path(scratch) / program
            subprocess.run(
                [*cc, "-O1", f"-I{GMP}", str(GMP / f"{program}.c"), "-o", str(executable), "-lm"],
                check=True,
            )
            built[program] = executable
        result = subprocess.run(
            [str(built[program]), *arguments], check=True, capture_output=True
        )
        tables[output] = result.stdout
    return tables


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--check", action="store_true", help="compare instead of writing")
    args = parser.parse_args(argv)
    if not (GMP / "gmp-h.in").is_file():
        print("gmp_tables: vendor/gmp is not checked out (git submodule update --init)", file=sys.stderr)
        return 1
    with tempfile.TemporaryDirectory(prefix="llg-gmp-tables-") as scratch:
        tables = generate(scratch)
    stale = []
    for name, data in tables.items():
        path = OUTPUT / name
        if args.check:
            if not path.is_file() or path.read_bytes() != data:
                stale.append(name)
        else:
            OUTPUT.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
    if stale:
        print(
            "gmp_tables: stale tables " + ", ".join(stale) + "; run scripts/gmp_tables.py",
            file=sys.stderr,
        )
        return 1
    print("gmp_tables: " + ("tables are current" if args.check else f"wrote {len(tables)} tables"))
    return 0


if __name__ == "__main__":
    sys.exit(main())
