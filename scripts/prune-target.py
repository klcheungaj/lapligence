#!/usr/bin/env python3
"""Remove stale Cargo build units from a target directory.

Cargo never deletes outputs whose hash changed (new dependency versions,
features, flags or toolchain), so `target/<profile>/` keeps every old copy of
the library and test binaries. A unit is one fingerprinted `<package>-<hash>` with its `deps/` and `build/`
outputs; incremental sessions are `incremental/<crate>-<id>`.

A unit is removed when none of its files changed within --keep-days
(default 1) AND either:
  * a newer unit of the same package and target kind exists in the same
    profile directory, or
  * it builds a workspace target (integration test, binary, example or
    benchmark) that `cargo metadata` no longer lists, e.g. after suites were
    regrouped or a binary was renamed.
So artifacts that alternate between configurations (for example `clippy
--all-features` and the default test build) survive while in use, and the
newest copy of every crate is always kept. Removing a unit never breaks a
build: Cargo rebuilds anything it still needs.

Usage: scripts/prune-target.py [--target-dir DIR] [--keep-days N] [--dry-run]
"""

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path

UNIT = re.compile(r"^(?P<crate>.+?)-(?P<hash>[0-9a-f]{16})(?P<rest>(?:\..*)?)$")
SESSION = re.compile(r"^(?P<crate>.+)-(?P<hash>[0-9a-z]{13})$")


def newest_mtime(path):
    """Newest modification time of `path` or anything below it."""
    try:
        newest = path.lstat().st_mtime
    except FileNotFoundError:
        return 0.0
    if path.is_dir() and not path.is_symlink():
        for root, dirs, files in os.walk(path):
            for name in dirs + files:
                try:
                    newest = max(newest, os.lstat(os.path.join(root, name)).st_mtime)
                except FileNotFoundError:
                    pass
    return newest


def size_of(path):
    if path.is_symlink() or not path.is_dir():
        try:
            return path.lstat().st_size
        except FileNotFoundError:
            return 0
    total = 0
    for root, _, files in os.walk(path):
        for name in files:
            try:
                total += os.lstat(os.path.join(root, name)).st_size
            except FileNotFoundError:
                pass
    return total


def profile_dirs(target):
    """Directories that hold Cargo units: target/<profile>/ and target/<triple>/<profile>/."""
    found = []
    for candidate in sorted(target.glob("*")) + sorted(target.glob("*/*")):
        if candidate.is_dir() and (candidate / ".fingerprint").is_dir():
            found.append(candidate)
    return found


def collect_units(profile):
    """Map unit identity -> {hash: [paths]} for one profile directory.

    Identity is the package plus the target kind recorded in its fingerprint
    (`lib-llg`, `test-integration-test-sim_force`, `run-build-script-...`), so
    the library, its tests, binaries and build-script units never displace each
    other. Every `deps/` and `build/` entry carrying a fingerprinted hash belongs
    to that unit. Incremental sessions are keyed by crate name alone.
    """
    units = {}
    by_hash = {}
    fingerprints = profile / ".fingerprint"
    for entry in fingerprints.iterdir():
        match = UNIT.match(entry.name)
        if not match or not entry.is_dir():
            continue
        kinds = tuple(sorted(
            name for name in os.listdir(entry)
            if not name.startswith("dep-") and not name.endswith(".json")
            and name != "invoked.timestamp" and name != "output"
        ))
        key = (match.group("crate"), kinds)
        paths = units.setdefault(key, {}).setdefault(match.group("hash"), [entry])
        by_hash[match.group("hash")] = paths
    for part in ("deps", "build"):
        directory = profile / part
        if not directory.is_dir():
            continue
        for entry in directory.iterdir():
            match = UNIT.match(entry.name)
            if match and match.group("hash") in by_hash:
                by_hash[match.group("hash")].append(entry)
    incremental = profile / "incremental"
    if incremental.is_dir():
        for entry in incremental.iterdir():
            match = SESSION.match(entry.name)
            if match:
                key = (match.group("crate"), ("incremental",))
                units.setdefault(key, {}).setdefault(match.group("hash"), []).append(entry)
    return units


# Fingerprint kind-file prefixes for targets that `cargo metadata` lists by kind.
TARGET_KIND_PREFIXES = (
    ("test-integration-test-", "test"),
    ("test-bin-", "bin"),
    ("bin-", "bin"),
    ("example-", "example"),
    ("bench-", "bench"),
)


def workspace_targets(manifest_path):
    """{package: {(kind, target name)}} for workspace members, or None."""
    try:
        output = subprocess.run(
            ["cargo", "metadata", "--format-version", "1", "--no-deps", "--offline",
             "--manifest-path", str(manifest_path)],
            check=True, capture_output=True, text=True,
        ).stdout
    except (OSError, subprocess.CalledProcessError):
        return None
    targets = {}
    for package in json.loads(output)["packages"]:
        names = targets.setdefault(package["name"], set())
        for target in package["targets"]:
            for kind in target["kind"]:
                names.add((kind, target["name"]))
    return targets


def is_orphan(package, kinds, targets):
    """Whether a fingerprinted unit builds a workspace target that no longer exists."""
    if targets is None or package not in targets:
        return False
    for kind_file in kinds:
        for prefix, kind in TARGET_KIND_PREFIXES:
            if kind_file.startswith(prefix):
                return (kind, kind_file[len(prefix):]) not in targets[package]
    return False


def stale_paths(profile, keep_seconds, now, targets=None):
    for (crate, kinds), by_hash in sorted(collect_units(profile).items()):
        orphan = is_orphan(crate, kinds, targets)
        if len(by_hash) < 2 and not orphan:
            continue
        ages = {h: max(newest_mtime(p) for p in paths) for h, paths in by_hash.items()}
        newest = max(ages.values())
        for unit_hash, paths in sorted(by_hash.items()):
            superseded = orphan or ages[unit_hash] < newest
            if superseded and now - ages[unit_hash] > keep_seconds:
                yield crate, unit_hash, paths


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--target-dir", type=Path,
                        default=Path(os.environ.get("CARGO_TARGET_DIR", "target")))
    parser.add_argument("--keep-days", type=float, default=1.0)
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--manifest-path", type=Path,
                        default=Path(__file__).resolve().parent.parent / "Cargo.toml",
                        help="workspace whose current targets decide orphaned units")
    args = parser.parse_args(argv)

    target = args.target_dir
    if not target.is_dir():
        print(f"error: target directory not found: {target}", file=sys.stderr)
        return 2
    targets = workspace_targets(args.manifest_path)
    if targets is None:
        print("note: cargo metadata failed; only superseded units are pruned", file=sys.stderr)
    now = time.time()
    keep_seconds = args.keep_days * 86400
    removed_units = 0
    removed_bytes = 0
    for profile in profile_dirs(target):
        for crate, unit_hash, paths in stale_paths(profile, keep_seconds, now, targets):
            removed_units += 1
            for path in paths:
                removed_bytes += size_of(path)
                if args.dry_run:
                    continue
                if path.is_dir() and not path.is_symlink():
                    shutil.rmtree(path, ignore_errors=True)
                else:
                    path.unlink(missing_ok=True)
            if args.dry_run:
                print(f"would remove {profile.relative_to(target)}/{crate}-{unit_hash}")
    verb = "would free" if args.dry_run else "freed"
    print(f"{removed_units} stale units; {verb} {removed_bytes / 2**30:.2f} GiB in {target}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
