import io
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import ci_build_timings as timings

LINES = [
    "llg-build\tresult=ok\ttotal_ms=4000\tgenerate_ms=10\tprobe_ms=5\truntime_ms=20\truntime=hit\tseed_ms=1\tseed=applied\tconfigure_retry=0\tconfigure_ms=900\tbuild_ms=3000\tdir=C:\\a b",
    "llg-build\tresult=ok\ttotal_ms=9000\tgenerate_ms=10\tprobe_ms=5\truntime_ms=20\truntime=hit\tseed_ms=1\tseed=unseeded\tconfigure_retry=0\tconfigure_ms=5000\tbuild_ms=3900\tdir=/tmp/x",
    "llg-build\tresult=error\ttotal_ms=100\tgenerate_ms=10\tdir=/tmp/y",
    "unrelated line",
    "",
]


class BuildTimingsTest(unittest.TestCase):
    def test_parse_keeps_only_build_records(self):
        records = timings.parse(LINES)
        self.assertEqual(len(records), 3)
        self.assertEqual(records[0]["dir"], "C:\\a b")
        self.assertEqual(records[1]["seed"], "unseeded")

    def test_summary_counts_outcomes_and_phases(self):
        text = timings.summarize(timings.parse(LINES))
        self.assertIn("model builds: 3", text)
        self.assertIn("seed: -=1, applied=1, unseeded=1", text)
        self.assertIn("result: error=1, ok=2", text)
        self.assertIn("seed=applied: builds=1 configure median=900ms total median=4000ms", text)
        self.assertIn("seed=unseeded: builds=1 configure median=5000ms", text)
        configure = next(line for line in text.splitlines() if line.startswith("configure "))
        self.assertEqual(configure.split()[1:3], ["2", "5.9"])

    def test_missing_file_is_not_an_error(self):
        with tempfile.TemporaryDirectory() as directory:
            output = io.StringIO()
            with redirect_stdout(output):
                self.assertEqual(timings.main([str(Path(directory) / "none.tsv")]), 0)
            self.assertIn("no model-build timings", output.getvalue())
            path = Path(directory) / "t.tsv"
            path.write_text("\n".join(LINES), encoding="utf-8")
            output = io.StringIO()
            with redirect_stdout(output):
                self.assertEqual(timings.main([str(path)]), 0)
            self.assertIn("model builds: 3", output.getvalue())


if __name__ == "__main__":
    unittest.main()
