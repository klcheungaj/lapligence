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

    def test_profiles_rank_commands_by_self_time(self):
        events = [
            {"cat": "project", "name": "configure", "ph": "B", "ts": 0},
            {"cat": "script", "name": "project", "ph": "B", "ts": 10,
             "args": {"location": "C:\\a\\m1\\CMakeLists.txt:2"}},
            {"cat": "script", "name": "execute_process", "ph": "B", "ts": 20,
             "args": {"location": "C:/Program Files/CMake/Modules/X.cmake:7"}},
            {"ph": "E", "ts": 1020},
            {"ph": "E", "ts": 1510},
            {"ph": "E", "ts": 2000},
            {"cat": "project", "name": "generate", "ph": "B", "ts": 2000},
            {"ph": "E", "ts": 5000},
        ]
        text = timings.summarize_profiles([events, events])
        self.assertIn("cmake configure profiles: 2", text)
        self.assertIn("configure: median=2ms", text)
        self.assertIn("generate: median=3ms", text)
        rows = [line.split() for line in text.splitlines()[4:]]
        self.assertEqual(rows[0][3:], ["execute_process", "Modules/X.cmake:7"])
        self.assertEqual(rows[0][:3], ["2.0", "2", "1.0"])
        self.assertEqual(rows[1][3:], ["project", "m1/CMakeLists.txt:2"])
        self.assertEqual(rows[1][2], "0.5")

    def test_profile_directory_skips_unreadable_files(self):
        with tempfile.TemporaryDirectory() as directory:
            Path(directory, "a.json").write_text('[{"cat": "project", "name": "configure", "ph": "B", "ts": 0}, '
                                                 '{"ph": "E", "ts": 3000}]', encoding="utf-8")
            Path(directory, "b.json").write_text("[{", encoding="utf-8")
            output = io.StringIO()
            with redirect_stdout(output):
                self.assertEqual(timings.main([str(Path(directory) / "none.tsv"),
                                               "--cmake-profiles", directory]), 0)
            text = output.getvalue()
            self.assertIn("cmake configure profiles: 1", text)
            self.assertIn("configure: median=3ms", text)
            self.assertIn("unreadable profiles: 1", text)


if __name__ == "__main__":
    unittest.main()
