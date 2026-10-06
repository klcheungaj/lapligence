import contextlib
import io
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import ci_gmp


class Sha256Tests(unittest.TestCase):
    def test_pinned_digest_is_verified(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "file"
            path.write_bytes(b"gmp")
            digest = ci_gmp.sha256_file(path)
            ci_gmp.verify_sha256(path, digest)
            with self.assertRaises(RuntimeError):
                ci_gmp.verify_sha256(path, "0" * 64)

    def test_pinned_release_is_the_audited_tarball(self):
        self.assertEqual(ci_gmp.VERSION, "6.3.0")
        self.assertEqual(len(ci_gmp.SHA256), 64)
        self.assertTrue(all(url.endswith(ci_gmp.TARBALL) for url in ci_gmp.URLS))
        self.assertTrue(all(url.startswith("https://") for url in ci_gmp.URLS))


class LayoutTests(unittest.TestCase):
    def test_library_names_match_the_build_lookup(self):
        lookup = (Path(__file__).resolve().parent.parent / "src/sim/build/value.rs").read_text(
            encoding="utf-8"
        )
        for name in ci_gmp.LIBRARIES:
            self.assertIn(f'"{name}"', lookup)

    def test_find_library_prefers_static_archives(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.assertIsNone(ci_gmp.find_library(root))
            (root / "lib").mkdir()
            (root / "lib/gmp.lib").write_text("x")
            self.assertEqual(ci_gmp.find_library(root), root / "lib/gmp.lib")
            (root / "lib/libgmp.a").write_text("x")
            self.assertEqual(ci_gmp.find_library(root), root / "lib/libgmp.a")


class ClearTests(unittest.TestCase):
    def test_clear_keeps_the_mounted_prefix_directory(self):
        with tempfile.TemporaryDirectory() as directory:
            prefix = Path(directory) / "gmp"
            (prefix / "lib").mkdir(parents=True)
            (prefix / "lib/libgmp.a").write_text("stale")
            (prefix / "stamp").write_text("stale")
            ci_gmp.clear_directory(prefix)
            self.assertTrue(prefix.is_dir())
            self.assertEqual(list(prefix.iterdir()), [])


class RouteTests(unittest.TestCase):
    def test_windows_uses_vcpkg_static_libraries_with_the_dynamic_crt(self):
        self.assertEqual(ci_gmp.default_route("Windows"), "vcpkg")
        self.assertEqual(ci_gmp.default_route("Linux"), "source")
        self.assertEqual(ci_gmp.default_route("Darwin"), "source")
        self.assertEqual(ci_gmp.vcpkg_triplet("AMD64"), "x64-windows-static-md")
        self.assertEqual(ci_gmp.vcpkg_triplet("ARM64"), "arm64-windows-static-md")
        with self.assertRaises(RuntimeError):
            ci_gmp.vcpkg_triplet("x86")

    def test_source_configuration_is_generic_static_and_64_bit(self):
        arguments = ci_gmp.configure_arguments("/opt/gmp", "aarch64-unknown-linux-gnu")
        self.assertIn("--build=aarch64-unknown-linux-gnu", arguments)
        self.assertIn("ABI=64", arguments)
        self.assertIn("--disable-shared", arguments)
        self.assertIn("--enable-static", arguments)
        self.assertIn("--prefix=/opt/gmp", arguments)
        self.assertFalse(any("march" in argument for argument in arguments))


class CacheKeyTests(unittest.TestCase):
    def test_key_separates_targets_compilers_and_images(self):
        base = ci_gmp.cache_key("x86_64-pc-windows-msvc", "cl 19.44")
        self.assertTrue(base.startswith("gmp-6.3.0-x86_64-pc-windows-msvc-"))
        self.assertEqual(base, ci_gmp.cache_key("x86_64-pc-windows-msvc", "cl 19.44"))
        self.assertNotEqual(base, ci_gmp.cache_key("aarch64-pc-windows-msvc", "cl 19.44"))
        self.assertNotEqual(base, ci_gmp.cache_key("x86_64-pc-windows-msvc", "cl 19.45"))
        self.assertNotEqual(
            base, ci_gmp.cache_key("x86_64-pc-windows-msvc", "cl 19.44", ["rockylinux:9"])
        )

    def test_cache_key_command_appends_github_output(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "out"
            with contextlib.redirect_stdout(io.StringIO()):
                status = ci_gmp.main(
                    ["cache-key", "--target", "t", "--extra", "image", "--output", str(output)]
                )
            self.assertEqual(status, 0)
            self.assertEqual(
                output.read_text(), f"key={ci_gmp.cache_key('t', '', ['image'])}\n"
            )

    def test_missing_installation_is_an_error(self):
        with tempfile.TemporaryDirectory() as directory:
            stderr = io.StringIO()
            with contextlib.redirect_stderr(stderr):
                status = ci_gmp.main(["identity", "--prefix", directory])
            self.assertEqual(status, 1)
            self.assertIn("lacks include/gmp.h", stderr.getvalue())


if __name__ == "__main__":
    unittest.main()
