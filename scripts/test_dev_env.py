from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


SCRIPTS = Path(__file__).resolve().parent
LINKER = "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER"
KEYS = ["RUSTC_WRAPPER", LINKER, "RUSTFLAGS", "TMPDIR", "CARGO_BUILD_TARGET"]


class DevEnvTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix="llg-dev-env-")
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        self.env = {"PATH": str(self.bin), "REPORT": str(self.root / "report")}
        for tool in ["dirname", "bash", "tr"]:
            (self.bin / tool).symlink_to(shutil.which(tool))
        self.tool("rustc", "printf 'host: x86_64-unknown-linux-gnu\\n'")
        self.tool("cc", "exit 0")

    def tool(self, name, body):
        path = self.bin / name
        path.write_text("#!/bin/sh\n" + body + "\n", encoding="utf-8")
        path.chmod(0o755)

    def source(self, settings=None):
        dump = "\n".join(f'printf \'%s\\n\' "${{{key}-<unset>}}"' for key in KEYS)
        return subprocess.run(
            ["/bin/bash", "-eu", "-c", 'source "$1" || exit $?; ' + dump,
             "test", str(SCRIPTS / "dev-env.sh")],
            env=dict(self.env, **(settings or {})), capture_output=True, text=True,
            timeout=10,
        )

    def test_default_needs_no_accelerators_or_rustc(self):
        (self.bin / "rustc").unlink()
        result = self.source()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.splitlines(), ["<unset>"] * len(KEYS))

    def test_opt_ins_preserve_flags_and_cross_target(self):
        self.tool("sccache", "exit 0")
        self.tool("mold", "exit 0")
        result = self.source({"LLG_SCCACHE": "1", "LLG_MOLD": "1",
                              "RUSTFLAGS": "existing flags", "TMPDIR": "long/scratch",
                              "CARGO_BUILD_TARGET": "aarch64-unknown-linux-musl"})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.splitlines(), [str(SCRIPTS / "sccache.sh"),
                         str(SCRIPTS / "mold-linker.sh"), "existing flags",
                         "long/scratch", "aarch64-unknown-linux-musl"])

    def test_existing_rust_wrapper_wins(self):
        self.tool("sccache", "exit 0")
        result = self.source({"LLG_SCCACHE": "1", "RUSTC_WRAPPER": "/user/wrapper"})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.splitlines()[0], "/user/wrapper")

    def test_missing_tools_fail(self):
        for key, value, tool in [("LLG_SCCACHE", "1", "sccache"),
                                 ("LLG_MOLD", "1", "mold"),
                                 ("LLG_CCACHE", "1", "ccache"),
                                 ("LLG_CCACHE", "sccache", "sccache")]:
            with self.subTest(key=key, value=value):
                result = self.source({key: value})
                self.assertEqual(result.returncode, 2)
                self.assertIn(tool, result.stderr)
                self.assertIn("PATH", result.stderr)

    def test_invalid_opt_ins_fail(self):
        for key in ["LLG_SCCACHE", "LLG_MOLD", "LLG_CCACHE"]:
            with self.subTest(key=key):
                result = self.source({key: "invalid"})
                self.assertEqual(result.returncode, 2)
                self.assertIn(key, result.stderr)

    def test_runner_fails_before_cargo_when_requested_tool_is_missing(self):
        for flag, name in [("--sccache", "sccache"), ("--mold", "mold")]:
            with self.subTest(flag=flag):
                result = subprocess.run(
                    ["/bin/bash", str(SCRIPTS / "run-tests.sh"), flag],
                    env=self.env, capture_output=True, text=True, timeout=10,
                )
                self.assertEqual(result.returncode, 2)
                self.assertIn(name, result.stderr)
                self.assertNotIn("cargo-nextest", result.stderr)

    def test_wrappers_fail_when_tools_disappear(self):
        for script, name in [("sccache.sh", "sccache"), ("mold-linker.sh", "mold")]:
            with self.subTest(script=script):
                result = subprocess.run(
                    [str(SCRIPTS / script), "argument"], env=self.env,
                    capture_output=True, text=True, timeout=10,
                )
                self.assertEqual(result.returncode, 2)
                self.assertIn(name, result.stderr)

    def test_non_gnu_hosts_fail_without_setting_linker(self):
        self.tool("mold", "exit 0")
        for host in ["x86_64-unknown-linux-musl", "aarch64-apple-darwin",
                     "x86_64-pc-windows-msvc"]:
            with self.subTest(host=host):
                self.tool("rustc", f"echo 'host: {host}'")
                result = self.source({"LLG_MOLD": "1"})
                self.assertEqual(result.returncode, 2)
                self.assertIn("Linux GNU", result.stderr)

    def test_existing_linker_conflict_and_idempotence(self):
        self.tool("mold", "exit 0")
        result = self.source({"LLG_MOLD": "1", LINKER: "/user/linker"})
        self.assertEqual(result.returncode, 2)
        self.assertIn("conflicts", result.stderr)
        result = self.source({"LLG_MOLD": "1", LINKER: str(SCRIPTS / "mold-linker.sh")})
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_wrapper_resets_tmpdir_and_preserves_arguments(self):
        self.tool("sccache", 'printf "%s\\n" "$TMPDIR" "$@"')
        result = subprocess.run(
            [str(SCRIPTS / "sccache.sh"), "/rust compiler", "arg with spaces"],
            env=dict(self.env, TMPDIR="/very/long/scratch"),
            capture_output=True, text=True, timeout=10,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.splitlines(), ["/tmp", "/rust compiler", "arg with spaces"])

    def test_native_wrapper_selects_compiler(self):
        self.tool("sccache", 'printf "%s\\n" "$TMPDIR" "$@"')
        result = subprocess.run(
            [str(SCRIPTS / "sccache-cc.sh"), "-c", "source file.c"],
            env=dict(self.env, CC=str(SCRIPTS / "sccache-cc.sh"), LLG_SCCACHE_CC="clang"),
            capture_output=True, text=True, timeout=10,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.splitlines(), ["/tmp", "clang", "-c", "source file.c"])

    def test_native_wrapper_default_ignores_cmake_cc_self_reference(self):
        self.tool("sccache", 'printf "%s\\n" "$TMPDIR" "$@"')
        result = subprocess.run(
            [str(SCRIPTS / "sccache-cc.sh"), "--version"],
            env=dict(self.env, CC=str(SCRIPTS / "sccache-cc.sh")),
            capture_output=True, text=True, timeout=10,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.splitlines(), ["/tmp", "cc", "--version"])

    def test_mold_wrapper_passes_link_flags(self):
        self.tool("mold", "exit 0")
        self.tool("clang", 'printf "%s\\n" "$@"')
        result = subprocess.run(
            [str(SCRIPTS / "mold-linker.sh"), "-fuse-ld=lld", "-o", "output file"],
            env=dict(self.env, LLG_MOLD_CC="clang", LLG_MOLD_THREADS="4"),
            capture_output=True, text=True, timeout=10,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.splitlines(),
                         ["-fuse-ld=lld", "-o", "output file", "-Wl,--threads=4", "-fuse-ld=mold"])

    def test_invalid_mold_thread_count_fails_early(self):
        for value in ["0", "-1", "many"]:
            with self.subTest(value=value):
                result = self.source({"LLG_MOLD": "1", "LLG_MOLD_THREADS": value})
                self.assertEqual(result.returncode, 2)
                self.assertIn("LLG_MOLD_THREADS", result.stderr)


if __name__ == "__main__":
    unittest.main()
