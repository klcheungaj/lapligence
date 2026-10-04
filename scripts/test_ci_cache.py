import contextlib
import io
import os
import shutil
import sys
import tarfile
import tempfile
import unittest
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import ci_ccache
import ci_prune_cargo_cache as prune

ROOT = Path(__file__).resolve().parent.parent
HASH = "0123456789abcdef"

LOCK = """
[[package]]
name = "llg"
version = "0.1.0"
dependencies = ["serde"]

[[package]]
name = "serde"
version = "1.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "windows-sys"
version = "0.61.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "libc"
version = "0.2.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "git-dep"
version = "1.0.0"
source = "git+https://example.invalid/repo#abc"
"""


def touch(path, text="x"):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)


class PruneTests(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="llg-prune-test-"))
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)
        self.lock = self.tmp / "Cargo.lock"
        self.lock.write_text(LOCK)
        self.target = self.tmp / "target"

    def profile(self, root):
        for name in (
            f"deps/libserde-{HASH}.rlib",
            f"deps/serde-{HASH}.d",
            f"deps/serde-{HASH}.serde.abc-cgu.0.rcgu.dwo",
            f"deps/liblibc-{HASH}.rlib",
            f"deps/libwindows_sys-{HASH}.rmeta",
            f"deps/libgit_dep-{HASH}.rlib",
            f"build/serde-{HASH}/out/generated.rs",
            f"build/windows-sys-{HASH}/output",
            f".fingerprint/serde-{HASH}/lib-serde",
            # Everything below is compiled from this repository.
            f"deps/libllg-{HASH}.rlib",
            f"deps/llg-{HASH}.d",
            f"deps/sim_counter-{HASH}",
            f"deps/sim_counter-{HASH}.d",
            f"build/llg-{HASH}/out/libllg_slang_wrapper.a",
            f"build/llg-{HASH}/build-script-build",
            f".fingerprint/llg-{HASH}/lib-llg",
            "incremental/llg-abc/s-1",
            "examples/demo",
            "llg",
            "llg.d",
            ".cargo-lock",
        ):
            touch(root / name)

    def test_dependency_names_exclude_path_packages(self):
        names = prune.dependency_names(LOCK)
        self.assertEqual(names, {"serde", "windows_sys", "libc", "git_dep"})
        self.assertNotIn("llg", names)

    def test_keeps_only_dependency_artifacts(self):
        self.profile(self.target / "debug")
        self.profile(self.target / "x86_64-unknown-linux-musl" / "release")
        touch(self.target / "slang/x86_64-unknown-linux-gnu/debug/0123/build/CMakeCache.txt")
        touch(self.target / "slang/x86_64-unknown-linux-gnu/debug/0123/lib/libsvlang.a")
        touch(self.target / "llg-runtime-cache/owned-v1/build/lib.a")
        touch(self.target / "native-cmake-tree/key/build/CMakeCache.txt")
        touch(self.target / "native-cmake-tree/key/lib/libsvlang.a")
        touch(self.target / "nextest-archive.tar.zst")
        touch(self.target / "nextest-bin/cargo-nextest")
        touch(self.target / "p07-full-host/logs/a.log")
        touch(self.target / "CACHEDIR.TAG")
        removed = prune.prune(self.target, self.lock)
        self.assertTrue(removed)
        left = sorted(
            str(path.relative_to(self.target)).replace(os.sep, "/")
            for path in self.target.rglob("*")
            if path.is_file()
        )
        for profile in ("debug", "x86_64-unknown-linux-musl/release"):
            self.assertEqual(
                [name for name in left if name.startswith(profile + "/")],
                sorted(
                    f"{profile}/{name}"
                    for name in (
                        f".fingerprint/serde-{HASH}/lib-serde",
                        f"build/serde-{HASH}/out/generated.rs",
                        f"build/windows-sys-{HASH}/output",
                        f"deps/liblibc-{HASH}.rlib",
                        f"deps/libgit_dep-{HASH}.rlib",
                        f"deps/libserde-{HASH}.rlib",
                        f"deps/libwindows_sys-{HASH}.rmeta",
                        f"deps/serde-{HASH}.d",
                        f"deps/serde-{HASH}.serde.abc-cgu.0.rcgu.dwo",
                    )
                ),
            )
        self.assertEqual(
            [name for name in left if "/" not in name or name.split("/")[0] not in ("debug", "x86_64-unknown-linux-musl")],
            ["CACHEDIR.TAG"],
        )
        for name in ("slang", "llg-runtime-cache", "nextest-bin", "p07-full-host", "native-cmake-tree"):
            self.assertFalse((self.target / name).exists(), name)
        self.assertFalse(any("llg" in name or "sim_counter" in name for name in left))

    def test_nested_build_dir_layout_keeps_dependency_by_package_name(self):
        touch(self.target / "debug/build/serde/abcd/out/x")
        touch(self.target / "debug/build/llg/abcd/out/y")
        touch(self.target / "debug/deps" / ("placeholder-" + "0" * 16))
        prune.prune(self.target, self.lock)
        self.assertTrue((self.target / "debug/build/serde/abcd/out/x").is_file())
        self.assertFalse((self.target / "debug/build/llg").exists())

    def test_missing_target_is_fine_and_empty_lock_is_an_error(self):
        self.assertEqual(prune.prune(self.tmp / "absent", self.lock), [])
        (self.tmp / "empty.lock").write_text('[[package]]\nname = "llg"\nversion = "0"\n')
        with self.assertRaises(RuntimeError):
            prune.prune(self.target, self.tmp / "empty.lock")

    def test_repository_lock_has_dependencies_and_no_workspace_crate(self):
        names = prune.dependency_names((ROOT / "Cargo.lock").read_text(encoding="utf-8"))
        self.assertIn("serde", names)
        self.assertNotIn("llg", names)


class CcacheTests(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="llg-ccache-test-"))
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)

    def test_every_supported_host_has_a_pinned_checksummed_asset(self):
        for system, machine, suffix in [
            ("Linux", "x86_64", "linux-x86_64-musl-static.tar.gz"),
            ("Linux", "aarch64", "linux-aarch64-musl-static.tar.gz"),
            ("Darwin", "arm64", "darwin.tar.gz"),
            ("Windows", "AMD64", "windows-x86_64.zip"),
            ("Windows", "ARM64", "windows-aarch64.zip"),
        ]:
            asset, digest = ci_ccache.select_asset(system, machine)
            self.assertTrue(asset.endswith(suffix), (system, machine, asset))
            self.assertRegex(digest, r"^[0-9a-f]{64}$")
        with self.assertRaises(RuntimeError):
            ci_ccache.select_asset("Linux", "riscv64")

    def test_extracts_only_the_executable_from_either_archive_kind(self):
        tar_path = self.tmp / "ccache-9-linux.tar.gz"
        member = self.tmp / "ccache"
        member.write_bytes(b"#!/bin/sh\n")
        with tarfile.open(tar_path, "w:gz") as bundle:
            bundle.add(member, arcname="ccache-9-linux/ccache")
            bundle.add(member, arcname="ccache-9-linux/NEWS.md")
        binary = ci_ccache.extract_binary(tar_path, "ccache-9-linux.tar.gz", self.tmp / "bin", "Linux")
        self.assertEqual(binary.name, "ccache")
        self.assertEqual(sorted(path.name for path in (self.tmp / "bin").iterdir()), ["ccache"])
        self.assertTrue(os.access(binary, os.X_OK) or os.name == "nt")
        zip_path = self.tmp / "ccache-9-windows.zip"
        with zipfile.ZipFile(zip_path, "w") as bundle:
            bundle.writestr("ccache-9-windows/ccache.exe", b"MZ")
            bundle.writestr("ccache-9-windows/MANUAL.md", b"x")
        binary = ci_ccache.extract_binary(zip_path, "ccache-9-windows.zip", self.tmp / "win", "Windows")
        self.assertEqual(binary.name, "ccache.exe")

    def test_checksum_mismatch_is_rejected(self):
        asset, _ = ci_ccache.select_asset("Linux", "x86_64")
        served = self.tmp / "served"
        served.mkdir()
        (served / asset).write_bytes(b"not the pinned archive")
        with self.assertRaises(RuntimeError) as context:
            ci_ccache.install(self.tmp / "bin", served.as_uri(), "Linux", "x86_64")
        self.assertIn("does not match the pinned", str(context.exception))
        self.assertFalse((self.tmp / "bin" / "ccache").exists())

    def test_print_stats_hits_and_environment_formats(self):
        stats = ci_ccache.parse_print_stats("direct_cache_hit\t2\npreprocessed_cache_hit\t1\nstats_updated_timestamp\tx\n")
        self.assertEqual(ci_ccache.cache_hits(stats), 3)
        self.assertEqual(ci_ccache.cache_hits(ci_ccache.parse_print_stats("cache_miss\t4\n")), 0)
        env_file = self.tmp / "env"
        with contextlib.redirect_stdout(io.StringIO()):
            ci_ccache.write_env({"A": "x y"}, env_file, "github")
            ci_ccache.write_env({"B": "x y"}, env_file, "shell")
        self.assertEqual(env_file.read_text(), "A=x y\nB='x y'\n")

    def test_failure_degrades_to_an_uncached_run_and_exports_nothing(self):
        env_file = self.tmp / "env"
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            status = ci_ccache.main(
                ["activate", "--ccache", str(self.tmp / "missing-ccache"), "--ccache-dir", str(self.tmp / "c"),
                 "--env-file", str(env_file)]
            )
        self.assertEqual(status, 0)
        self.assertIn("::warning", output.getvalue())
        self.assertFalse(env_file.exists())
        with self.assertRaises(Exception):
            ci_ccache.main(["activate", "--strict", "--ccache", str(self.tmp / "missing-ccache"),
                            "--ccache-dir", str(self.tmp / "c")])

    @unittest.skipUnless(
        shutil.which("ccache") and shutil.which("cmake") and shutil.which("cc"),
        "needs ccache, cmake and a C compiler",
    )
    def test_activation_self_check_proves_a_cache_hit_across_directories(self):
        env_file = self.tmp / "env"
        base = self.tmp / "base"
        with contextlib.redirect_stdout(io.StringIO()):
            ci_ccache.main(
                ["activate", "--strict", "--ccache", shutil.which("ccache"), "--ccache-dir", str(self.tmp / "cache"),
                 "--base-dir", str(base), "--compiler", "cc", "--env-file", str(env_file), "--max-size", "20M"]
            )
        text = env_file.read_text()
        self.assertIn("LLG_C_LAUNCHER=", text)
        self.assertIn("CCACHE_NOHASHDIR=1", text)
        self.assertIn(f"CCACHE_BASEDIR={base}", text)
        self.assertFalse(list(base.iterdir()), "the self-check removes its scratch directory")

    @unittest.skipUnless(shutil.which("ccache") and shutil.which("cmake") and shutil.which("cc"), "needs ccache")
    def test_self_check_fails_without_base_dir_rewriting(self):
        base = self.tmp / "base"
        original = ci_ccache.ccache_environment
        ci_ccache.ccache_environment = lambda cache_dir, base_dir, max_size: {
            "CCACHE_DIR": str(cache_dir),
            "CCACHE_MAXSIZE": max_size,
        }
        try:
            failure = ci_ccache.self_check(shutil.which("ccache"), base, "cc")
        finally:
            ci_ccache.ccache_environment = original
        self.assertIsNotNone(failure)
        self.assertIn("not a cache hit", failure)


if __name__ == "__main__":
    unittest.main()
