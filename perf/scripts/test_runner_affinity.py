import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


SCRIPTS = Path(__file__).resolve().parent


class RunnerAffinityTests(unittest.TestCase):
    def test_corpus_rejects_invalid_cpu_before_generating(self):
        result = subprocess.run(
            [str(SCRIPTS / "corpus.sh"), "--sim-bin", "/missing", "--output-dir", "/missing",
             "--cpu", "-1"], capture_output=True, text=True,
        )
        self.assertEqual(result.returncode, 2)
        self.assertIn("--cpu must be a nonnegative integer", result.stderr)

    def test_legacy_runner_rejects_invalid_cpu_before_building(self):
        result = subprocess.run(
            [str(SCRIPTS / "perf_baseline.sh"), "--cpu", "invalid"],
            capture_output=True, text=True,
        )
        self.assertEqual(result.returncode, 2)
        self.assertIn("--cpu must be a nonnegative integer", result.stderr)

    def test_run_wrapper_pins_only_the_child(self):
        affinity = os.sched_getaffinity(0)
        cpu = min(affinity)
        with tempfile.TemporaryDirectory() as directory:
            metric = Path(directory) / "run.tsv"
            result = subprocess.run(
                [str(SCRIPTS / "perf_baseline.sh"), "--run-wrapper", "--record", str(metric),
                 "--", sys.executable, "-c", "import os; print(sorted(os.sched_getaffinity(0)))"],
                capture_output=True, text=True,
                env=dict(os.environ, PERF_BASELINE_CPU=str(cpu)),
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout.strip(), f"[{cpu}]")
            self.assertEqual(metric.read_text().split("\t")[:2], ["run", "0"])
        self.assertEqual(os.sched_getaffinity(0), affinity)


if __name__ == "__main__":
    unittest.main()
