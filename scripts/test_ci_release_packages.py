import contextlib
import hashlib
import io
import json
import sys
import tarfile
import tempfile
import unittest
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import ci_release_packages


class ReleasePackagesTests(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory()
        self.root = Path(self.scratch.name)
        self.manifest = self.root / "Cargo.toml"
        self.manifest.write_text(
            '[package]\nname = "lapligence"\nversion = "1.2.3"\n\n'
            '[dependencies]\nother = { version = "9.9.9" }\n',
            encoding="utf-8",
        )
        self.dist = self.root / "dist"
        self.dist.mkdir()

    def tearDown(self):
        self.scratch.cleanup()

    def run_main(self, *arguments, stdin=""):
        stdout = io.StringIO()
        stderr = io.StringIO()
        previous = sys.stdin
        sys.stdin = io.StringIO(stdin)
        try:
            with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
                status = ci_release_packages.main(["--manifest", str(self.manifest), *arguments])
        finally:
            sys.stdin = previous
        return status, stdout.getvalue(), stderr.getvalue()

    def write_package(self, platform, members=None):
        archive = self.dist / ci_release_packages.archive_name("1.2.3", platform)
        members = sorted(members or ci_release_packages.expected_members(platform))
        if archive.name.endswith(".zip"):
            with zipfile.ZipFile(archive, "w") as handle:
                for member in members:
                    handle.writestr(member, member)
        else:
            source = self.root / "members"
            source.mkdir(exist_ok=True)
            with tarfile.open(archive, "w:gz") as handle:
                for member in members:
                    (source / member).write_text(member, encoding="utf-8")
                    handle.add(source / member, arcname=member)
        checksum = hashlib.sha256(archive.read_bytes()).hexdigest()
        (self.dist / f"{archive.name}.sha256").write_text(
            f"{checksum}  {archive.name}\n", encoding="ascii"
        )
        return archive

    def write_all_packages(self):
        return [self.write_package(platform) for platform in ci_release_packages.PLATFORMS]

    def test_version_reads_the_package_table(self):
        self.assertEqual(self.run_main("version"), (0, "1.2.3\n", ""))

    def test_version_allows_a_trailing_comment_and_ignores_other_tables(self):
        self.manifest.write_text(
            '[workspace]\nversion = "0.0.1"\n\n[package]\nname = "lapligence"\n'
            'version = "2.0.0"  # bumped\nedition = "2024"\n',
            encoding="utf-8",
        )
        self.assertEqual(self.run_main("version"), (0, "2.0.0\n", ""))

    def test_version_rejects_a_package_without_a_plain_version(self):
        self.manifest.write_text('[package]\nname = "lapligence"\n', encoding="utf-8")
        status, stdout, stderr = self.run_main("version")
        self.assertEqual((status, stdout), (1, ""))
        self.assertIn("[package] has no version", stderr)
        self.manifest.write_text("[package]\nversion.workspace = true\n", encoding="utf-8")
        status, _, stderr = self.run_main("version")
        self.assertEqual(status, 1)
        self.assertIn("not a quoted string", stderr)

    def test_version_accepts_only_the_matching_tag(self):
        self.assertEqual(self.run_main("version", "--tag", "v1.2.3"), (0, "1.2.3\n", ""))
        status, stdout, stderr = self.run_main("version", "--tag", "v1.2.4")
        self.assertEqual((status, stdout), (1, ""))
        self.assertIn("expected v1.2.3", stderr)
        status, _, stderr = self.run_main("version", "--tag", "1.2.3")
        self.assertEqual(status, 1)
        self.assertIn("expected v1.2.3", stderr)

    def test_archive_names_follow_the_platform(self):
        self.assertEqual(
            self.run_main("archive", "linux-arm64"),
            (0, "lapligence-1.2.3-linux-arm64.tar.gz\n", ""),
        )
        self.assertEqual(
            self.run_main("archive", "windows-x86_64"),
            (0, "lapligence-1.2.3-windows-x86_64.zip\n", ""),
        )
        with contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaises(SystemExit):
                self.run_main("archive", "linux-riscv64")

    def test_artifacts_name_every_platform(self):
        status, stdout, _ = self.run_main("artifacts")
        self.assertEqual(status, 0)
        self.assertEqual(
            stdout.split(),
            [
                "lapligence-linux-x86_64",
                "lapligence-linux-arm64",
                "lapligence-windows-x86_64",
                "lapligence-windows-arm64",
                "lapligence-macos-arm64",
            ],
        )

    def test_windows_arm64_packages_only_the_language_server(self):
        self.assertEqual(
            ci_release_packages.expected_members("windows-arm64"),
            {"llg_ls.exe", "LICENSE", "readme.md"},
        )
        self.assertEqual(
            ci_release_packages.expected_members("macos-arm64"),
            {"llg", "llg_ls", "LICENSE", "readme.md"},
        )

    def test_verify_accepts_the_complete_package_set(self):
        self.write_all_packages()
        self.assertEqual(self.run_main("verify", str(self.dist)), (0, "", ""))

    def test_verify_rejects_a_missing_package(self):
        self.write_all_packages()
        (self.dist / "lapligence-1.2.3-macos-arm64.tar.gz").unlink()
        status, _, stderr = self.run_main("verify", str(self.dist))
        self.assertEqual(status, 1)
        self.assertIn("expected", stderr)

    def test_verify_rejects_a_package_of_another_version(self):
        self.write_all_packages()
        archive = self.dist / "lapligence-1.2.3-linux-x86_64.tar.gz"
        archive.rename(self.dist / "lapligence-1.2.2-linux-x86_64.tar.gz")
        self.assertEqual(self.run_main("verify", str(self.dist))[0], 1)

    def test_verify_rejects_a_wrong_checksum(self):
        self.write_all_packages()
        checksum = self.dist / "lapligence-1.2.3-windows-x86_64.zip.sha256"
        checksum.write_text("0" * 64 + "  lapligence-1.2.3-windows-x86_64.zip\n", encoding="ascii")
        status, _, stderr = self.run_main("verify", str(self.dist))
        self.assertEqual(status, 1)
        self.assertIn("windows-x86_64.zip.sha256 records", stderr)

    def test_verify_rejects_wrong_archive_members(self):
        self.write_all_packages()
        self.write_package("windows-arm64", members={"llg_ls.exe", "llg.exe", "LICENSE", "readme.md"})
        status, _, stderr = self.run_main("verify", str(self.dist))
        self.assertEqual(status, 1)
        self.assertIn("windows-arm64.zip holds", stderr)

    def test_assets_require_every_file_uploaded_with_its_size(self):
        archive = self.write_package("linux-x86_64")
        checksum = self.dist / f"{archive.name}.sha256"
        uploaded = [
            {"name": archive.name, "size": archive.stat().st_size, "state": "uploaded"},
            {"name": checksum.name, "size": checksum.stat().st_size, "state": "uploaded"},
            {"name": "notes.txt", "size": 5, "state": "uploaded"},
        ]
        lines = "".join(json.dumps(asset) + "\n" for asset in uploaded)
        self.assertEqual(self.run_main("assets", str(self.dist), stdin=lines), (0, "", ""))

        status, _, stderr = self.run_main(
            "assets", str(self.dist), stdin=json.dumps(uploaded[0]) + "\n"
        )
        self.assertEqual(status, 1)
        self.assertIn(f"{checksum.name}: not attached", stderr)

        short = dict(uploaded[0], size=1)
        status, _, stderr = self.run_main(
            "assets", str(self.dist), stdin=json.dumps(short) + "\n" + json.dumps(uploaded[1])
        )
        self.assertEqual(status, 1)
        self.assertIn("holds 1 bytes", stderr)

        pending = dict(uploaded[1], state="new")
        status, _, stderr = self.run_main(
            "assets", str(self.dist), stdin=json.dumps(uploaded[0]) + "\n" + json.dumps(pending)
        )
        self.assertEqual(status, 1)
        self.assertIn("state is new", stderr)


if __name__ == "__main__":
    unittest.main()
