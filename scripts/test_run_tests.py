import concurrent.futures
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


RUNNER = Path(__file__).resolve().parent / "run-tests.sh"
FAKE_CARGO = '''#!/usr/bin/env python3
import json
import os
from pathlib import Path
import sys
import time

if sys.argv[1:] == ["nextest", "--version"]:
    sys.exit(0)
keys = ["TMPDIR", "LLG_TEST_BUILD_DIR", "LLG_RUNTIME_CACHE_DIR",
        "CARGO_TARGET_DIR", "CARGO_BUILD_BUILD_DIR", "RUSTC_WRAPPER",
        "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER", "RUSTFLAGS"]
report = {key: os.environ.get(key) for key in keys}
report["args"] = sys.argv[1:]
report["cwd"] = os.getcwd()
for key in ["TMPDIR", "LLG_TEST_BUILD_DIR", "LLG_RUNTIME_CACHE_DIR"]:
    if report[key]:
        directory = Path(report[key])
        directory.mkdir(parents=True, exist_ok=True)
        (directory / "artifact").write_text("keep until run completes", encoding="utf-8")
Path(os.environ["LLG_RUNNER_REPORT"]).write_text(json.dumps(report), encoding="utf-8")
if "LLG_RUNNER_BARRIER" in os.environ:
    barrier = Path(os.environ["LLG_RUNNER_BARRIER"])
    (barrier / str(os.getpid())).touch()
    deadline = time.monotonic() + 10
    while len(list(barrier.iterdir())) < 5:
        if time.monotonic() > deadline:
            sys.exit(99)
        time.sleep(0.01)
sys.exit(int(os.environ.get("LLG_RUNNER_STATUS", "0")))
'''


class RunTestsTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="llg-runner-tests-")
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name).resolve()
        self.storage = self.root / "ram scratch"
        self.storage.mkdir()
        self.bin = self.root / "bin"
        self.bin.mkdir()
        cargo = self.bin / "cargo"
        cargo.write_text(FAKE_CARGO, encoding="utf-8")
        cargo.chmod(0o755)
        self.env = os.environ.copy()
        for key in ["TMPDIR", "LLG_TEST_BUILD_DIR", "LLG_RUNTIME_CACHE_DIR",
                    "CARGO_TARGET_DIR", "CARGO_BUILD_BUILD_DIR", "LLG_SCCACHE",
                    "LLG_MOLD", "LLG_CCACHE", "RUSTC_WRAPPER",
                    "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER"]:
            self.env.pop(key, None)
        self.env["PATH"] = str(self.bin) + os.pathsep + self.env["PATH"]

    def worktree(self, name):
        root = self.root / name
        (root / "scripts").mkdir(parents=True)
        for name in ["run-tests.sh", "dev-env.sh", "sccache.sh", "mold-linker.sh"]:
            shutil.copy2(RUNNER.parent / name, root / "scripts" / name)
        return root

    def invoke(self, worktree, name, args, extra_env=None):
        report = self.root / f"{name}.json"
        env = dict(self.env, LLG_RUNNER_REPORT=str(report))
        env.update(extra_env or {})
        result = subprocess.run(
            ["bash", str(worktree / "scripts/run-tests.sh"), *args],
            cwd=self.root, env=env, capture_output=True, text=True, timeout=20,
        )
        return result, json.loads(report.read_text(encoding="utf-8")) if report.exists() else None

    def test_default_preserves_environment_and_arguments(self):
        worktree = self.worktree("default")
        settings = {key: str(self.root / key) for key in [
            "TMPDIR", "LLG_TEST_BUILD_DIR", "LLG_RUNTIME_CACHE_DIR",
            "CARGO_TARGET_DIR", "CARGO_BUILD_BUILD_DIR",
        ]}
        args = ["--test", "sim_counter", "-E", "test(a) or test(b)",
                "--test-threads", "8", "--", "--test-work-dir"]
        result, report = self.invoke(worktree, "default", args, settings)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(report["args"], ["nextest", "run", "--locked", *args])
        for key, value in settings.items():
            self.assertEqual(report[key], value)
        self.assertIsNone(report["RUSTC_WRAPPER"])
        self.assertIsNone(report["CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER"])
        self.assertEqual(list(self.storage.iterdir()), [])

    def test_grouped_suite_selects_group_binary_and_name_prefix(self):
        worktree = self.worktree("grouped")
        (worktree / "tests").mkdir()
        (worktree / "tests/sim_n_z.rs").write_text("mod sim_counter;\nmod sim_force;\n", encoding="utf-8")
        # A suite file may declare an inner module of its own name.
        (worktree / "tests/sim_counter.rs").write_text("mod sim_counter;\n", encoding="utf-8")
        args = ["--test", "sim_counter", "--test=sim_force", "--test", "sim_feature_completion",
                "--lib", "-E", "test(a)", "--test-threads", "8", "--", "--exact"]
        result, report = self.invoke(worktree, "grouped", args)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(report["args"], [
            "nextest", "run", "--locked", "-E",
            "(test(/^sim_counter::/) | test(/^sim_force::/) | binary(=sim_feature_completion)"
            " | kind(lib)) & (test(a))",
            "--test", "sim_n_z", "--test", "sim_feature_completion", "--lib",
            "--test-threads", "8", "--", "--exact",
        ])

    def test_grouped_suites_intersect_every_user_filterset(self):
        worktree = self.worktree("filtersets")
        (worktree / "tests").mkdir()
        (worktree / "tests/general.rs").write_text("mod lsp_stdio;\n", encoding="utf-8")
        args = ["-E", "test(a)", "--test", "lsp_stdio", "--filterset=test(b)"]
        result, report = self.invoke(worktree, "filtersets", args)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(report["args"], [
            "nextest", "run", "--locked", "-E",
            "(test(/^lsp_stdio::/)) & ((test(a)) | (test(b)))", "--test", "general",
        ])

    def test_accelerators_set_tools_and_consume_only_runner_flags(self):
        worktree = self.worktree("accelerators")
        for name, body in {
            "sccache": "exit 0", "mold": "exit 0",
            "rustc": "echo 'host: x86_64-unknown-linux-gnu'",
        }.items():
            tool = self.bin / name
            tool.write_text("#!/bin/sh\n" + body + "\n", encoding="utf-8")
            tool.chmod(0o755)
        args = ["--test", "sim_function", "--sccache", "--mold"]
        result, report = self.invoke(worktree, "accelerators", args, {"RUSTFLAGS": "keep-me"})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(report["args"], ["nextest", "run", "--locked", "--test", "sim_function"])
        self.assertEqual(report["RUSTC_WRAPPER"], str(worktree / "scripts/sccache.sh"))
        self.assertEqual(report["CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER"],
                         str(worktree / "scripts/mold-linker.sh"))
        self.assertEqual(report["RUSTFLAGS"], "keep-me")

    def test_parallel_worktrees_share_only_runtime_cache(self):
        worktrees = [self.worktree(f"agent {index}") for index in range(4)]
        worktrees.append(worktrees[0])
        barrier = self.root / "barrier"
        barrier.mkdir()
        sentinel = self.storage / "unrelated"
        sentinel.write_text("preserve", encoding="utf-8")
        args = ["--test-work-dir", "ram scratch", "--test-threads", "8"]
        with concurrent.futures.ThreadPoolExecutor(max_workers=5) as executor:
            tasks = [executor.submit(self.invoke, worktree, f"parallel-{index}", args, {
                "LLG_RUNNER_BARRIER": str(barrier),
                "CARGO_TARGET_DIR": "/unwanted/shared/target",
                "CARGO_BUILD_BUILD_DIR": "/unwanted/shared/build",
            }) for index, worktree in enumerate(worktrees)]
            results = [task.result() for task in tasks]
        scratch = []
        caches = set()
        for worktree, (result, report) in zip(worktrees, results):
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(report["cwd"], str(worktree))
            self.assertEqual(report["CARGO_TARGET_DIR"], str(worktree / "target"))
            self.assertEqual(report["CARGO_BUILD_BUILD_DIR"], str(worktree / "target"))
            self.assertEqual(report["args"], ["nextest", "run", "--locked", "--test-threads", "8"])
            run = Path(report["LLG_TEST_BUILD_DIR"]).parent
            self.assertEqual(Path(report["TMPDIR"]).parent, run)
            self.assertFalse(run.exists(), "successful scratch must be removed")
            scratch.append(run)
            caches.add(report["LLG_RUNTIME_CACHE_DIR"])
        self.assertEqual(len(set(scratch)), 5)
        self.assertEqual(len({run.parent for run in scratch}), 4)
        self.assertEqual(scratch[0].parent, scratch[4].parent)
        self.assertEqual(caches, {str(self.storage / "lapligence/runtime-cache")})
        self.assertTrue((Path(caches.pop()) / "artifact").is_file())
        self.assertEqual(sentinel.read_text(encoding="utf-8"), "preserve")

    def test_failure_retains_only_its_run_and_preserves_exit_status(self):
        worktree = self.worktree("failure")
        result, report = self.invoke(worktree, "failure", [f"--test-work-dir={self.storage}"],
                                     {"LLG_RUNNER_STATUS": "7"})
        self.assertEqual(result.returncode, 7, result.stderr)
        self.assertTrue((Path(report["TMPDIR"]) / "artifact").is_file())
        self.assertIn("retained scratch directory:", result.stderr)

    def test_invalid_work_directory_fails_before_cargo(self):
        worktree = self.worktree("invalid")
        for index, args in enumerate([
            ["--test-work-dir"], ["--test-work-dir="], ["--test-work-dir", ""],
            ["--test-work-dir", str(self.root / "missing")],
            ["--test-work-dir", str(worktree / "scripts/run-tests.sh")],
        ]):
            with self.subTest(args=args):
                result, report = self.invoke(worktree, f"invalid-{index}", args)
                self.assertEqual(result.returncode, 2)
                self.assertIsNone(report)


if __name__ == "__main__":
    unittest.main()
