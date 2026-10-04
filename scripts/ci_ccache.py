#!/usr/bin/env python3
"""Install and activate ccache for generated-model C compiles in CI.

The launcher is exported only as LLG_C_LAUNCHER, which sim::build forwards to
CMAKE_C_COMPILER_LAUNCHER for generated models and their runtime archive. It is
never put on PATH as a compiler masquerade and never reaches the root build.rs
Slang/fmt/wrapper build (that one reads LLG_CCACHE, which CI leaves unset).

`activate` installs a pinned, checksum-verified ccache, then proves it works in
this environment by building a tiny CMake project twice from different
directories (the way tests build models) and requiring a cache hit. If any step
fails the launcher is simply not exported and tests run uncached.
"""

import argparse
import hashlib
import os
import platform
import shlex
import shutil
import subprocess
import sys
import tarfile
import tempfile
import urllib.request
import zipfile
from pathlib import Path

VERSION = "4.14.1"
RELEASE_BASE_URL = f"https://github.com/ccache/ccache/releases/download/v{VERSION}"

# SHA-256 of the official release archives (static musl builds on Linux).
ASSETS = {
    ("linux", "x86_64"): (
        f"ccache-{VERSION}-linux-x86_64-musl-static.tar.gz",
        "8f684cf82a7acd2f1daddbefcda505c499afe8245cfbfe78f751ce389cd6df52",
    ),
    ("linux", "aarch64"): (
        f"ccache-{VERSION}-linux-aarch64-musl-static.tar.gz",
        "dd0b8516cd796aa4a260c543441653cc12bfa7ac34864970c068fc4ff04932fb",
    ),
    ("darwin", "arm64"): (
        f"ccache-{VERSION}-darwin.tar.gz",
        "c279fa81e2e806b4d9e64d4a06cb569a84ef54151feef796e5d4f4e95a464563",
    ),
    ("darwin", "x86_64"): (
        f"ccache-{VERSION}-darwin.tar.gz",
        "c279fa81e2e806b4d9e64d4a06cb569a84ef54151feef796e5d4f4e95a464563",
    ),
    ("windows", "x86_64"): (
        f"ccache-{VERSION}-windows-x86_64.zip",
        "6219f3865ca59aec41ee4b678df171d5d35855ecb2b6dbbbd20690b3a68af7b4",
    ),
    ("windows", "aarch64"): (
        f"ccache-{VERSION}-windows-aarch64.zip",
        "28121851c8c2c8b76214cfc5e119d769fbc39e93d27862e4e3a556905ebd23b2",
    ),
}

CHECK_C = "int llg_ccache_check(int x) { return x * 3 + 1; }\n"
CHECK_CMAKE = (
    "cmake_minimum_required(VERSION 3.16)\n"
    "project(llg_ccache_check C)\n"
    "add_library(check STATIC check.c)\n"
)


def host_key(system=None, machine=None):
    system = (system or platform.system()).lower()
    machine = (machine or platform.machine()).lower()
    machine = {"amd64": "x86_64", "x64": "x86_64", "arm64": "arm64", "aarch64": "aarch64"}.get(
        machine, machine
    )
    if system != "darwin" and machine == "arm64":
        machine = "aarch64"
    return system, machine


def select_asset(system=None, machine=None):
    key = host_key(system, machine)
    if key not in ASSETS:
        raise RuntimeError(f"no pinned ccache build for {key[0]} {key[1]}")
    return ASSETS[key]


def executable_name(system=None):
    return "ccache.exe" if (system or platform.system()).lower() == "windows" else "ccache"


def sha256_of(path):
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def extract_binary(archive, asset, install_dir, system=None):
    """Write only the ccache executable from `archive` into `install_dir`."""
    name = executable_name(system)
    stem = asset
    for suffix in (".tar.gz", ".zip"):
        if stem.endswith(suffix):
            stem = stem[: -len(suffix)]
    member = f"{stem}/{name}"
    install_dir = Path(install_dir)
    install_dir.mkdir(parents=True, exist_ok=True)
    target = install_dir / name
    if asset.endswith(".zip"):
        with zipfile.ZipFile(archive) as bundle:
            data = bundle.read(member)
    else:
        with tarfile.open(archive) as bundle:
            handle = bundle.extractfile(member)
            if handle is None:
                raise RuntimeError(f"{member} is not a regular file in {asset}")
            data = handle.read()
    target.write_bytes(data)
    target.chmod(0o755)
    return target


def install(install_dir, base_url=None, system=None, machine=None):
    """Download, verify and unpack the pinned ccache; return the executable."""
    asset, expected = select_asset(system, machine)
    url = f"{base_url or RELEASE_BASE_URL}/{asset}"
    with tempfile.TemporaryDirectory(prefix="llg-ccache-dl-") as scratch:
        archive = Path(scratch) / asset
        with urllib.request.urlopen(url, timeout=120) as response, open(archive, "wb") as out:
            shutil.copyfileobj(response, out)
        actual = sha256_of(archive)
        if actual != expected:
            raise RuntimeError(f"{asset}: sha256 {actual} does not match the pinned {expected}")
        return extract_binary(archive, asset, install_dir, system)


def parse_print_stats(text):
    stats = {}
    for line in text.splitlines():
        key, _, value = line.partition("\t")
        if value.strip().isdigit():
            stats[key] = int(value)
    return stats


def cache_hits(stats):
    return stats.get("direct_cache_hit", 0) + stats.get("preprocessed_cache_hit", 0)


def ccache_environment(cache_dir, base_dir, max_size):
    return {
        "CCACHE_DIR": str(cache_dir),
        "CCACHE_BASEDIR": str(base_dir),
        # Model directories are unique per test; neither the build directory
        # nor the (rewritten) absolute source path may enter the hash.
        "CCACHE_NOHASHDIR": "1",
        "CCACHE_MAXSIZE": max_size,
    }


def run(command, env, cwd=None):
    return subprocess.run(
        command, env=env, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True
    )


def self_check(ccache, base_dir, compiler, generator=None):
    """Return None if two builds from different directories hit, else the reason."""
    base_dir = Path(base_dir)
    base_dir.mkdir(parents=True, exist_ok=True)
    work = Path(tempfile.mkdtemp(prefix="llg-ccache-check-", dir=base_dir))
    try:
        env = dict(os.environ)
        env.update(ccache_environment(work / "cache", base_dir, "50M"))
        env.pop("CMAKE_C_COMPILER_LAUNCHER", None)
        launcher = str(ccache).replace("\\", "/")
        for name in ("a", "b"):
            project = work / name
            project.mkdir()
            (project / "check.c").write_text(CHECK_C)
            (project / "CMakeLists.txt").write_text(CHECK_CMAKE)
            configure = ["cmake", "-S", str(project), "-B", str(project / "build")]
            if generator:
                configure += ["-G", generator]
            configure += ["-DCMAKE_BUILD_TYPE=Release", f"-DCMAKE_C_COMPILER_LAUNCHER={launcher}"]
            if compiler:
                configure.append(f"-DCMAKE_C_COMPILER={compiler}")
            for step in (configure, ["cmake", "--build", str(project / "build")]):
                result = run(step, env)
                if result.returncode != 0:
                    return f"{' '.join(step)} failed:\n{result.stdout[-2000:]}"
        stats = run([str(ccache), "--print-stats"], env)
        if stats.returncode != 0:
            return f"ccache --print-stats failed:\n{stats.stdout[-1000:]}"
        if cache_hits(parse_print_stats(stats.stdout)) < 1:
            return f"the second identical build was not a cache hit:\n{stats.stdout[-1000:]}"
        return None
    finally:
        shutil.rmtree(work, ignore_errors=True)


def write_env(entries, path, fmt):
    lines = []
    for key, value in entries.items():
        lines.append(f"{key}={value}" if fmt == "github" else f"{key}={shlex.quote(value)}")
    text = "".join(line + "\n" for line in lines)
    if path:
        with open(path, "a", encoding="utf-8", newline="\n") as handle:
            handle.write(text)
    sys.stdout.write(text)


def warn(message):
    print(f"::warning title=ccache::{message}", flush=True)


def default_path(name, fallback):
    if name:
        return Path(name)
    return Path(os.environ.get("RUNNER_TEMP") or tempfile.gettempdir()) / fallback


def activate(args):
    cache_dir = default_path(args.ccache_dir, "ccache")
    base_dir = Path(args.base_dir or tempfile.gettempdir())
    # The same resolution sim::build applies when LLG_CC and CC are both unset.
    compiler = args.compiler or os.environ.get("LLG_CC") or os.environ.get("CC") or "cc"
    try:
        if args.ccache:
            ccache = Path(args.ccache)
        else:
            ccache = install(default_path(args.install_dir, "ccache-bin"))
        if args.install_only:
            print(f"installed {ccache}")
            return 0
        cache_dir.mkdir(parents=True, exist_ok=True)
        version = run([str(ccache), "--version"], dict(os.environ))
        if version.returncode != 0:
            raise RuntimeError(f"{ccache} --version failed: {version.stdout}")
        print(version.stdout.splitlines()[0])
        failure = self_check(ccache, base_dir, compiler, os.environ.get("CMAKE_GENERATOR"))
        if failure:
            raise RuntimeError(f"self-check failed: {failure}")
        env = ccache_environment(cache_dir, base_dir, args.max_size)
        zero = run([str(ccache), "-z"], {**os.environ, **env})
        if zero.returncode != 0:
            raise RuntimeError(f"ccache -z failed: {zero.stdout}")
        write_env(
            {"LLG_C_LAUNCHER": str(ccache).replace("\\", "/"), **env}, args.env_file, args.env_format
        )
        return 0
    except Exception as error:  # noqa: BLE001 - any failure must degrade to uncached
        if args.strict:
            raise
        warn(f"generated models are built without a compiler cache: {error}")
        return 0


def stats(args):
    candidate = args.ccache or os.environ.get("LLG_C_LAUNCHER")
    if not candidate:
        print("ccache was not activated in this job")
        return 0
    result = run([candidate, "-sv"], dict(os.environ))
    print(result.stdout)
    return 0


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    act = commands.add_parser("activate")
    act.add_argument("--ccache", help="use this executable instead of downloading one")
    act.add_argument("--install-dir", help="download target (default $RUNNER_TEMP/ccache-bin)")
    act.add_argument("--ccache-dir", help="cache directory (default $RUNNER_TEMP/ccache)")
    act.add_argument("--max-size", default="500M")
    act.add_argument("--base-dir", help="common parent of model directories (default: temp dir)")
    act.add_argument("--compiler", help="C compiler for the self-check (default LLG_CC, CC, cc)")
    act.add_argument("--env-file", help="append NAME=VALUE lines here (for GITHUB_ENV)")
    act.add_argument("--env-format", choices=("github", "shell"), default="github")
    act.add_argument("--install-only", action="store_true", help="download and verify only")
    act.add_argument("--strict", action="store_true", help="raise instead of degrading to uncached")
    act.set_defaults(handler=activate)
    stat = commands.add_parser("stats")
    stat.add_argument("--ccache")
    stat.set_defaults(handler=stats)
    args = parser.parse_args(argv)
    return args.handler(args)


if __name__ == "__main__":
    sys.exit(main())
