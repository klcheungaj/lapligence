#!/usr/bin/env python3
import argparse
import hashlib
import os
import platform
import shutil
import subprocess
import sys
import tarfile
import tempfile
import urllib.request
from pathlib import Path

VERSION = "6.3.0"
TARBALL = f"gmp-{VERSION}.tar.xz"
SHA256 = "a3c2b80201b89e68616f4ad30bc66aee4927c3ce50e33929ca819d5c43538898"
URLS = (
    f"https://gmplib.org/download/gmp/{TARBALL}",
    f"https://ftp.gnu.org/gnu/gmp/{TARBALL}",
)
RECIPE = "gmp-recipe-v1"
LIBRARIES = ("lib/libgmp.a", "lib/gmp.lib", "lib/libgmp.lib")
LICENSES = ("COPYING", "COPYING.LESSERv3", "COPYINGv2", "COPYINGv3")
VCPKG_TRIPLETS = {"x86_64": "x64-windows-static-md", "aarch64": "arm64-windows-static-md"}

IDENTITY_C = r"""
#include <gmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
int main(void) {
    char header[64];
    snprintf(header, sizeof header, "%d.%d.%d", __GNU_MP_VERSION, __GNU_MP_VERSION_MINOR,
             __GNU_MP_VERSION_PATCHLEVEL);
    printf("gmp_header_version=%s\n", header);
    printf("gmp_library_version=%s\n", gmp_version);
    printf("gmp_limb_bits=%d\n", (int)GMP_LIMB_BITS);
    printf("gmp_nail_bits=%d\n", (int)GMP_NAIL_BITS);
    printf("sizeof_mp_limb_t=%u\n", (unsigned)sizeof(mp_limb_t));
    printf("mp_limb_t_is_uint64_t=%d\n", _Generic((mp_limb_t*)0, uint64_t*: 1, default: 0));
    printf("gmp_cc=%s\n", __GMP_CC);
    printf("gmp_cflags=%s\n", __GMP_CFLAGS);
    printf("c_compiler=%s %s\n", LLG_C_COMPILER_ID, LLG_C_COMPILER_VERSION);
    mp_limb_t a[2] = {7, 0}, b[1] = {3}, q[2], r[1];
    mpn_tdiv_qr(q, r, 0, a, 1, b, 1);
    int ok = strcmp(header, gmp_version) == 0 && GMP_LIMB_BITS == 64 && GMP_NAIL_BITS == 0 &&
             sizeof(mp_limb_t) == 8 && q[0] == 2 && r[0] == 1;
    printf("compatible=%d\n", ok);
    return ok ? 0 : 1;
}
"""

IDENTITY_CMAKE = """cmake_minimum_required(VERSION 3.16)
project(llg_gmp_identity C)
set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)
add_executable(identity identity.c)
target_compile_definitions(identity PRIVATE
  LLG_C_COMPILER_ID="${CMAKE_C_COMPILER_ID}" LLG_C_COMPILER_VERSION="${CMAKE_C_COMPILER_VERSION}")
target_include_directories(identity PRIVATE "${GMP_ROOT}/include")
target_link_libraries(identity PRIVATE "${GMP_LIBRARY}")
"""


def log(message):
    print(f"ci_gmp: {message}", flush=True)


def sha256_file(path):
    digest = hashlib.sha256()
    with open(path, "rb") as stream:
        for chunk in iter(lambda: stream.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def verify_sha256(path, expected):
    actual = sha256_file(path)
    if actual != expected:
        raise RuntimeError(f"{path}: SHA-256 {actual} does not match pinned {expected}")


def find_library(prefix):
    for name in LIBRARIES:
        if (Path(prefix) / name).is_file():
            return Path(prefix) / name
    return None


def host_arch(machine=None):
    machine = (machine or platform.machine()).lower()
    return {"amd64": "x86_64", "x64": "x86_64", "arm64": "aarch64"}.get(machine, machine)


def default_route(system=None):
    return "vcpkg" if (system or platform.system()) == "Windows" else "source"


def vcpkg_triplet(machine=None):
    arch = host_arch(machine)
    if arch not in VCPKG_TRIPLETS:
        raise RuntimeError(f"no vcpkg triplet for Windows {arch}")
    return VCPKG_TRIPLETS[arch]


def configure_arguments(prefix, build_triple):
    return [
        f"--prefix={prefix}",
        f"--build={build_triple}",
        "ABI=64",
        "--enable-static",
        "--disable-shared",
        "--with-pic",
    ]


def cache_key(target, compiler_text, extra=()):
    digest = hashlib.sha256()
    for part in (RECIPE, VERSION, SHA256, target, compiler_text, *extra):
        digest.update(part.encode())
        digest.update(b"\0")
    digest.update(Path(__file__).read_bytes())
    return f"gmp-{VERSION}-{target}-{digest.hexdigest()[:20]}"


def compiler_text(command):
    if not command:
        return ""
    for argument in ("--version", "/Bv", ""):
        args = [command] + ([argument] if argument else [])
        try:
            result = subprocess.run(args, capture_output=True, text=True, errors="replace")
        except OSError:
            continue
        text = (result.stdout + result.stderr).strip()
        if text:
            return text.splitlines()[0] if argument == "" else text
    raise RuntimeError(f"cannot query compiler {command}")


def run(args, cwd=None, env=None):
    log("$ " + " ".join(str(arg) for arg in args))
    subprocess.run([str(arg) for arg in args], cwd=cwd, env=env, check=True)


def download(work):
    tarball = Path(work) / TARBALL
    if tarball.is_file():
        try:
            verify_sha256(tarball, SHA256)
            return tarball
        except RuntimeError:
            tarball.unlink()
    errors = []
    for url in URLS:
        try:
            log(f"downloading {url}")
            with urllib.request.urlopen(url, timeout=120) as response, open(tarball, "wb") as out:
                shutil.copyfileobj(response, out)
            verify_sha256(tarball, SHA256)
            return tarball
        except (OSError, RuntimeError) as error:
            errors.append(f"{url}: {error}")
            tarball.unlink(missing_ok=True)
    raise RuntimeError("GMP download failed: " + "; ".join(errors))


def build_source(prefix, work, jobs, check):
    tarball = download(work)
    source = Path(work) / f"gmp-{VERSION}"
    if source.exists():
        shutil.rmtree(source)
    with tarfile.open(tarball) as archive:
        archive.extractall(work)
    triple = subprocess.run(
        ["sh", "configfsf.guess"], cwd=source, capture_output=True, text=True, check=True
    ).stdout.strip()
    log(f"generic build triple {triple} (no host-CPU tuning, so the cached library is portable)")
    run(["sh", "configure", *configure_arguments(prefix, triple)], cwd=source)
    run(["make", f"-j{jobs}"], cwd=source)
    if check:
        run(["make", f"-j{jobs}", "check"], cwd=source)
    run(["make", "install"], cwd=source)
    licenses = Path(prefix) / "share/licenses/gmp"
    licenses.mkdir(parents=True, exist_ok=True)
    for name in LICENSES:
        shutil.copy2(source / name, licenses / name)


def vcpkg_root(work):
    for variable in ("VCPKG_INSTALLATION_ROOT", "VCPKG_ROOT"):
        value = os.environ.get(variable)
        if value and (Path(value) / "vcpkg.exe").is_file():
            return Path(value)
    root = Path(work) / "vcpkg"
    if not (root / "vcpkg.exe").is_file():
        if not root.exists():
            run(["git", "clone", "--depth", "1", "https://github.com/microsoft/vcpkg", root])
        run(["cmd", "/c", str(root / "bootstrap-vcpkg.bat"), "-disableMetrics"], cwd=root)
    return root


def build_vcpkg(prefix, work, triplet):
    root = vcpkg_root(work)
    installed = Path(work) / "vcpkg-installed"
    run([root / "vcpkg.exe", "version"])
    run(
        [
            root / "vcpkg.exe",
            "install",
            f"gmp:{triplet}",
            f"--x-install-root={installed}",
        ],
        cwd=work,
    )
    tree = installed / triplet
    library = find_library(tree)
    if library is None:
        raise RuntimeError(f"vcpkg installed no GMP library under {tree / 'lib'}")
    prefix = Path(prefix)
    (prefix / "include").mkdir(parents=True, exist_ok=True)
    (prefix / "lib").mkdir(parents=True, exist_ok=True)
    shutil.copy2(tree / "include/gmp.h", prefix / "include/gmp.h")
    shutil.copy2(library, prefix / library.relative_to(tree))
    licenses = prefix / "share/licenses/gmp"
    licenses.mkdir(parents=True, exist_ok=True)
    shutil.copy2(tree / "share/gmp/copyright", licenses / "copyright")
    port = root / "ports/gmp/vcpkg.json"
    if port.is_file():
        shutil.copy2(port, licenses / "vcpkg-port.json")


def identity(prefix, cmake="cmake"):
    prefix = Path(prefix).resolve()
    header = prefix / "include/gmp.h"
    library = find_library(prefix)
    if not header.is_file() or library is None:
        raise RuntimeError(f"{prefix} lacks include/gmp.h or a static GMP library")
    lines = [
        f"host={platform.platform()}",
        f"machine={platform.machine()}",
        f"gmp_root={prefix}",
        f"gmp_header_sha256={sha256_file(header)}",
        f"gmp_library={library.relative_to(prefix).as_posix()}",
        f"gmp_library_sha256={sha256_file(library)}",
    ]
    with tempfile.TemporaryDirectory(prefix="llg-gmp-identity-") as scratch:
        scratch = Path(scratch)
        (scratch / "identity.c").write_text(IDENTITY_C, encoding="utf-8")
        (scratch / "CMakeLists.txt").write_text(IDENTITY_CMAKE, encoding="utf-8")
        build = scratch / "build"
        run(
            [
                cmake,
                "-S",
                scratch,
                "-B",
                build,
                f"-DGMP_ROOT={prefix.as_posix()}",
                f"-DGMP_LIBRARY={library.as_posix()}",
                "-DCMAKE_BUILD_TYPE=Release",
            ]
        )
        run([cmake, "--build", build, "--config", "Release"])
        executable = next(
            (
                path
                for name in ("identity", "identity.exe", "Release/identity.exe")
                for path in [build / name]
                if path.is_file()
            ),
            None,
        )
        if executable is None:
            raise RuntimeError("identity probe executable missing")
        result = subprocess.run([str(executable)], capture_output=True, text=True)
        lines.extend(result.stdout.splitlines())
        if result.returncode != 0:
            print("\n".join(lines))
            raise RuntimeError("GMP installation is not 64-bit nail-free or headers/library differ")
    return lines


def clear_directory(path):
    path.mkdir(parents=True, exist_ok=True)
    for child in path.iterdir():
        if child.is_dir() and not child.is_symlink():
            shutil.rmtree(child)
        else:
            child.unlink()


def command_build(args):
    prefix = Path(args.prefix).resolve()
    work = Path(args.work or tempfile.mkdtemp(prefix="llg-gmp-build-")).resolve()
    work.mkdir(parents=True, exist_ok=True)
    if args.reuse and find_library(prefix) is not None:
        try:
            for line in identity(prefix, args.cmake):
                print(line)
            log(f"reusing cached GMP at {prefix}")
            return 0
        except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
            log(f"cached GMP rejected ({error}); rebuilding")
    clear_directory(prefix)
    route = args.route if args.route != "auto" else default_route()
    if route == "source":
        build_source(prefix, work, args.jobs, args.check)
    else:
        build_vcpkg(prefix, work, args.triplet or vcpkg_triplet())
    for line in identity(prefix, args.cmake):
        print(line)
    return 0


def command_fetch(args):
    work = Path(args.work).resolve()
    work.mkdir(parents=True, exist_ok=True)
    print(download(work))
    return 0


def command_identity(args):
    lines = identity(args.prefix, args.cmake)
    text = "\n".join(lines) + "\n"
    print(text, end="")
    if args.output:
        Path(args.output).write_text(text, encoding="utf-8")
    return 0


def command_cache_key(args):
    key = cache_key(args.target, compiler_text(args.compiler), args.extra)
    print(key)
    if args.output:
        with open(args.output, "a", encoding="utf-8") as stream:
            stream.write(f"key={key}\n")
    return 0


def main(argv=None):
    parser = argparse.ArgumentParser(description="Build and identify GMP for CI qualification")
    commands = parser.add_subparsers(dest="command", required=True)
    build = commands.add_parser("build")
    build.add_argument("--prefix", required=True)
    build.add_argument("--work")
    build.add_argument("--route", choices=("auto", "source", "vcpkg"), default="auto")
    build.add_argument("--triplet")
    build.add_argument("--jobs", type=int, default=os.cpu_count() or 2)
    build.add_argument("--check", action="store_true")
    build.add_argument("--reuse", action="store_true")
    build.add_argument("--cmake", default="cmake")
    build.set_defaults(handler=command_build)
    fetch = commands.add_parser("fetch")
    fetch.add_argument("--work", required=True)
    fetch.set_defaults(handler=command_fetch)
    show = commands.add_parser("identity")
    show.add_argument("--prefix", required=True)
    show.add_argument("--output")
    show.add_argument("--cmake", default="cmake")
    show.set_defaults(handler=command_identity)
    key = commands.add_parser("cache-key")
    key.add_argument("--target", required=True)
    key.add_argument("--compiler")
    key.add_argument("--extra", action="append", default=[])
    key.add_argument("--output")
    key.set_defaults(handler=command_cache_key)
    args = parser.parse_args(argv)
    try:
        return args.handler(args)
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"ci_gmp: error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
