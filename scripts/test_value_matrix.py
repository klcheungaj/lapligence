import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parent / "value_matrix.py"
FAKE_RUNNER = '''#!/usr/bin/env python3
import json
import os
from pathlib import Path
import sys

cell = os.environ["LLG_TEST_MATRIX_CELL"]
with open(os.environ["FAKE_REPORT"], "a", encoding="utf-8") as report:
    report.write(json.dumps({
        "cell": cell,
        "backend": os.environ.get("LLG_DEV_VALUE_BACKEND"),
        "kernel": os.environ.get("LLG_DEV_COMPACT_KERNELS"),
        "gmp_root": os.environ.get("GMP_ROOT"),
        "args": sys.argv[1:],
    }) + "\\n")
mode = os.environ.get("FAKE_MODE", "pass")
if mode == "no-junit":
    sys.exit(101)
witness = "sim_value_backends::matrix::matrix_cell_selection_reaches_the_built_model"
cases = [("llg::sim_n_z", "sim_counter::counts", "")]
if mode != "no-witness":
    cases.append(("llg::sim_n_z", witness, ""))
if mode == "fail-legacy" and cell.startswith("legacy/"):
    cases.append(("llg::sim_a_m", "sim_force::forces", "<failure message='x'/>"))
suites = {}
for binary, name, body in cases:
    suites.setdefault(binary, []).append(f"<testcase name='{name}' classname='{binary}'>{body}</testcase>")
xml = "<testsuites>" + "".join(
    f"<testsuite name='{binary}'>{''.join(items)}</testsuite>" for binary, items in suites.items()
) + "</testsuites>"
junit = Path(os.environ["CARGO_TARGET_DIR"]) / "nextest" / "value-matrix" / "junit.xml"
junit.parent.mkdir(parents=True, exist_ok=True)
junit.write_text(xml, encoding="utf-8")
print("     Summary [   1.000s] 2 tests run: 2 passed, 7 skipped")
sys.exit(100 if mode == "fail-legacy" and cell.startswith("legacy/") else 0)
'''


class ValueMatrixTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.runner = self.root / "runner.py"
        self.runner.write_text(FAKE_RUNNER, encoding="utf-8")
        self.runner.chmod(0o755)
        self.report = self.root / "report.jsonl"

    def tearDown(self):
        self.temp.cleanup()

    def run_matrix(self, *extra, mode="pass", env_extra=None):
        env = {key: value for key, value in os.environ.items()
               if key not in ("GMP_ROOT", "LLG_TEST_GMP_ROOT", "LLG_DEV_VALUE_BACKEND",
                              "LLG_DEV_COMPACT_KERNELS", "LLG_TEST_MATRIX_CELL")}
        env.update({"CARGO_TARGET_DIR": str(self.root / "target"), "FAKE_REPORT": str(self.report),
                    "FAKE_MODE": mode})
        env.update(env_extra or {})
        out = self.root / "out"
        completed = subprocess.run(
            [sys.executable, str(SCRIPT), "--output-dir", str(out), "--runner", str(self.runner), *extra],
            env=env, capture_output=True, text=True,
        )
        return completed, out

    def calls(self):
        if not self.report.exists():
            return []
        return [json.loads(line) for line in self.report.read_text(encoding="utf-8").splitlines()]

    def summary(self, out):
        lines = (out / "summary.tsv").read_text(encoding="utf-8").splitlines()
        header = lines[0].split("\t")
        return [dict(zip(header, line.split("\t"))) for line in lines[1:]]

    def test_default_cells_select_each_lane_and_mode(self):
        completed, out = self.run_matrix("--", "--cargo-profile", "quick")
        self.assertEqual(completed.returncode, 0, completed.stderr)
        calls = self.calls()
        self.assertEqual([call["cell"] for call in calls], [
            "compact/gmp/normal", "legacy/portable/normal", "legacy/portable/no-opt", "compact/gmp/no-opt",
        ])
        for call in calls:
            backend, kernel, _ = call["cell"].split("/")
            self.assertEqual((call["backend"], call["kernel"], call["gmp_root"]), (backend, kernel, None))
            self.assertEqual(call["args"][:6], ["--cargo-profile", "quick", "--profile", "value-matrix",
                                                "--no-fail-fast", "--ignore-default-filter"])
        witness = "test(=sim_value_backends::matrix::matrix_cell_selection_reaches_the_built_model)"
        self.assertEqual(calls[0]["args"][-1], f"(all()) | {witness}")
        self.assertEqual(calls[1]["args"][-1], f"(not default()) | {witness}")
        rows = self.summary(out)
        self.assertEqual([row["status"] for row in rows], ["ok"] * 4)
        self.assertEqual([row["scope"] for row in rows], ["all"] + ["backend-dependent"] * 3)
        self.assertEqual(rows[0]["passed"], "2")
        self.assertEqual(rows[0]["nextest_skipped"], "7")
        self.assertTrue((out / "legacy.no-opt.junit.xml").is_file())
        groups = (out / "compact-gmp.normal.groups.tsv").read_text(encoding="utf-8").splitlines()
        self.assertIn("llg::sim_n_z\tsim_counter\t1\t0\t0\t0", groups)
        metadata = (out / "metadata.tsv").read_text(encoding="utf-8")
        self.assertIn("lanes\tlegacy,compact-gmp", metadata)

    def test_filter_narrows_every_cell_but_keeps_the_witness(self):
        completed, _ = self.run_matrix("--lanes", "compact-portable", "--modes", "no-opt",
                                       "--filter", "test(/^sim_counter::/)")
        self.assertEqual(completed.returncode, 0, completed.stderr)
        [call] = self.calls()
        self.assertEqual(call["cell"], "compact/portable/no-opt")
        self.assertTrue(call["args"][-1].startswith("((not default()) & (test(/^sim_counter::/))) | test(="))

    def test_failed_test_fails_its_cell_and_the_run(self):
        completed, out = self.run_matrix(mode="fail-legacy")
        self.assertEqual(completed.returncode, 1)
        rows = {row["cell"]: row for row in self.summary(out)}
        self.assertEqual(rows["legacy.normal"]["status"], "FAILED")
        self.assertEqual(rows["legacy.normal"]["failed"], "1")
        self.assertEqual(rows["compact-gmp.normal"]["status"], "ok")
        self.assertEqual((out / "legacy.normal.failed.txt").read_text(encoding="utf-8"),
                         "llg::sim_a_m sim_force::forces\n")

    def test_missing_witness_fails_the_cell(self):
        completed, out = self.run_matrix("--lanes", "legacy", "--modes", "normal", mode="no-witness")
        self.assertEqual(completed.returncode, 1)
        [row] = self.summary(out)
        self.assertIn("backend witness", row["problems"])

    def test_cell_without_report_counts_as_not_run(self):
        completed, out = self.run_matrix("--lanes", "legacy", "--modes", "normal", mode="no-junit")
        self.assertEqual(completed.returncode, 1)
        [row] = self.summary(out)
        self.assertIn("did not run", row["problems"])

    def test_inherited_value_selection_is_refused(self):
        for name in ("GMP_ROOT", "LLG_TEST_GMP_ROOT", "LLG_DEV_VALUE_BACKEND"):
            completed, _ = self.run_matrix(env_extra={name: "x"})
            self.assertEqual(completed.returncode, 2, name)
            self.assertIn(name, completed.stderr)
        self.assertEqual(self.calls(), [])

    def test_unknown_lane_is_rejected(self):
        completed, _ = self.run_matrix("--lanes", "legacy,gmp")
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn("invalid lanes", completed.stderr)


if __name__ == "__main__":
    unittest.main()
