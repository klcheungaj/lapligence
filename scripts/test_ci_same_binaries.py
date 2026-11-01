import contextlib
import io
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import ci_same_binaries


class SameBinariesTests(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory()
        self.root = Path(self.scratch.name)
        self.tested = self.root / "tested"
        self.built = self.root / "built"
        self.package = self.root / "package"
        for directory in (self.tested, self.built, self.package):
            directory.mkdir()

    def tearDown(self):
        self.scratch.cleanup()

    def write(self, directory, name, data):
        (directory / name).write_bytes(data)

    def run_main(self, *arguments):
        stdout = io.StringIO()
        stderr = io.StringIO()
        with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            status = ci_same_binaries.main(list(arguments))
        return status, stdout.getvalue(), stderr.getvalue()

    def test_identical_copies_pass_and_print_digests(self):
        for directory in (self.tested, self.built, self.package):
            self.write(directory, "llg", b"driver")
            self.write(directory, "llg_ls", b"server")
        status, stdout, stderr = self.run_main(
            "--tested", str(self.tested), "--name", "llg", "--name", "llg_ls",
            str(self.built), str(self.package),
        )
        self.assertEqual(status, 0, stderr)
        self.assertEqual(stderr, "")
        self.assertEqual(len(stdout.splitlines()), 6)
        self.assertIn(ci_same_binaries.digest(self.tested / "llg"), stdout)

    def test_one_differing_byte_fails(self):
        self.write(self.tested, "llg", b"driver")
        self.write(self.built, "llg", b"driver")
        self.write(self.package, "llg", b"drivers")
        status, _, stderr = self.run_main(
            "--tested", str(self.tested), "--name", "llg",
            str(self.built), str(self.package),
        )
        self.assertEqual(status, 1)
        self.assertIn(str(self.package / "llg"), stderr)
        self.assertNotIn(str(self.built / "llg"), stderr)

    def test_missing_copies_fail(self):
        self.write(self.built, "llg", b"driver")
        status, _, stderr = self.run_main(
            "--tested", str(self.tested), "--name", "llg", str(self.built),
        )
        self.assertEqual(status, 1)
        self.assertIn("missing from the tested executables", stderr)

        self.write(self.tested, "llg", b"driver")
        status, _, stderr = self.run_main(
            "--tested", str(self.tested), "--name", "llg", str(self.package),
        )
        self.assertEqual(status, 1)
        self.assertIn(f"{self.package / 'llg'}: missing", stderr)


if __name__ == "__main__":
    unittest.main()
