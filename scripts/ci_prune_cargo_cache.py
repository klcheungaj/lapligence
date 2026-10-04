#!/usr/bin/env python3
"""Reduce a Cargo target directory to compiled third-party dependencies.

CI saves `target/` so registry dependencies are not recompiled. Nothing built
from this repository may be saved: the workspace crate and its build-script
output (including the native Slang/fmt/wrapper tree that build.rs places in
`target/slang`, outside Cargo's layout), test executables, nextest archives and
generated-model runtime caches. Swatinem/rust-cache prunes workspace members
itself, but it does not know about `target/slang`, and the musl container job
has no such action, so this runs in every job just before the cache is saved.

An artifact is kept only if its crate name is a package of Cargo.lock that has
a registry or git `source`. Path packages (the workspace crate) have none, so
everything compiled from them is removed. Run it as the last step of a job.
"""

import argparse
import os
import re
import shutil
import stat
import sys
from pathlib import Path

PROFILE_DIRS = ("build", ".fingerprint", "deps")
# Native trees outside Cargo's layout: the Slang/fmt/wrapper CMake build that
# build.rs keeps in target/slang, and the generated-model runtime archive cache.
NATIVE_TREES = ("slang", "llg-runtime-cache")
ARTIFACT_NAME = re.compile(r"^(?P<name>.+?)-(?P<hash>[0-9a-f]{16})(?:\..*)?$")


def normalize(name):
    return name.replace("-", "_")


def dependency_names(lock_text):
    """Normalized names of Cargo.lock packages that come from a registry or git."""
    names = set()
    for block in lock_text.split("[[package]]")[1:]:
        fields = {}
        for line in block.splitlines():
            if line.startswith("["):
                break
            key, sep, value = line.partition(" = ")
            if sep and key in ("name", "source"):
                fields[key] = value.strip().strip('"')
        if "name" in fields and "source" in fields:
            names.add(normalize(fields["name"]))
    return names


def is_dependency_artifact(entry_name, names):
    """Whether a deps/build/.fingerprint entry belongs to a dependency package."""
    match = ARTIFACT_NAME.match(entry_name)
    # Cargo's nested build-dir layout names the directory by package alone.
    stem = match.group("name") if match else entry_name
    candidates = {normalize(stem)}
    if stem.startswith("lib"):
        candidates.add(normalize(stem[3:]))
    return bool(candidates & names)


def remove(path):
    def clear_readonly(function, target, _error):
        os.chmod(target, stat.S_IWRITE | stat.S_IREAD | stat.S_IEXEC)
        function(target)

    if path.is_dir() and not path.is_symlink():
        shutil.rmtree(path, onerror=clear_readonly)
    else:
        try:
            path.unlink()
        except PermissionError:
            os.chmod(path, stat.S_IWRITE | stat.S_IREAD)
            path.unlink()


def is_profile_dir(path):
    return any((path / name).is_dir() for name in PROFILE_DIRS)


def has_file(path):
    return any(files for _root, _dirs, files in os.walk(path))


def prune_profile(profile, names, removed):
    for entry in sorted(profile.iterdir()):
        if entry.name in PROFILE_DIRS and entry.is_dir():
            for artifact in sorted(entry.iterdir()):
                if not is_dependency_artifact(artifact.name, names):
                    remove(artifact)
                    removed.append(artifact)
        else:
            remove(entry)
            removed.append(entry)


def prune_tree(directory, names, removed):
    """Keep only dependency artifacts of profile directories below `directory`."""
    for entry in sorted(directory.iterdir()):
        if entry.is_symlink() or not entry.is_dir():
            if entry.name != "CACHEDIR.TAG":
                remove(entry)
                removed.append(entry)
        elif entry.name in NATIVE_TREES:
            remove(entry)
            removed.append(entry)
        elif is_profile_dir(entry):
            prune_profile(entry, names, removed)
            if not has_file(entry):
                remove(entry)
        else:
            # Target-triple directories hold profiles; anything else (slang,
            # llg-runtime-cache, nextest output, evidence) is not a dependency.
            prune_tree(entry, names, removed)
            if not any(entry.iterdir()):
                remove(entry)


def directory_size(path):
    total = 0
    for root, _dirs, files in os.walk(path):
        for name in files:
            try:
                total += os.lstat(os.path.join(root, name)).st_size
            except OSError:
                pass
    return total


def prune(target, lock_path):
    names = dependency_names(Path(lock_path).read_text(encoding="utf-8"))
    if not names:
        raise RuntimeError(f"{lock_path} lists no registry or git packages")
    removed = []
    target = Path(target)
    if target.is_dir():
        prune_tree(target, names, removed)
    return removed


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target-dir", default="target")
    parser.add_argument("--lock", default="Cargo.lock")
    args = parser.parse_args(argv)
    removed = prune(args.target_dir, args.lock)
    top = sorted({Path(item).relative_to(args.target_dir).parts[0] for item in removed})
    print(f"pruned {len(removed)} entries; removed top-level: {', '.join(top) or 'none'}")
    if Path(args.target_dir).is_dir():
        print(f"kept {directory_size(args.target_dir) / 2**20:.0f} MiB of dependency artifacts")
    return 0


if __name__ == "__main__":
    sys.exit(main())
