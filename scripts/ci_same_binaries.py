#!/usr/bin/env python3
import argparse
import hashlib
import sys
from pathlib import Path


def digest(path):
    hasher = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            hasher.update(block)
    return hasher.hexdigest()


def compare(tested, directories, names):
    lines = []
    problems = []
    for name in names:
        reference = Path(tested) / name
        if not reference.is_file():
            problems.append(f"{reference}: missing from the tested executables")
            continue
        expected = digest(reference)
        lines.append(f"{expected}  {reference}")
        for directory in directories:
            candidate = Path(directory) / name
            if not candidate.is_file():
                problems.append(f"{candidate}: missing")
                continue
            actual = digest(candidate)
            lines.append(f"{actual}  {candidate}")
            if actual != expected:
                problems.append(
                    f"{candidate}: SHA-256 {actual} differs from the tested {expected}"
                )
    return lines, problems


def main(argv=None):
    parser = argparse.ArgumentParser(
        description="Fail unless every named executable in each directory is "
        "byte-identical to the copy in --tested."
    )
    parser.add_argument("--tested", required=True)
    parser.add_argument("--name", action="append", required=True)
    parser.add_argument("directories", nargs="+")
    args = parser.parse_args(argv)
    lines, problems = compare(args.tested, args.directories, args.name)
    for line in lines:
        print(line)
    for problem in problems:
        print(f"error: {problem}", file=sys.stderr)
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
