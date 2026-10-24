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

    def test_template_outcomes_are_grouped(self):
        lines = LINES + [
            "llg-build\tresult=ok\ttotal_ms=300\tseed_ms=1\tseed=applied\ttemplate_ms=4\ttemplate=applied\tbuild_ms=290",
            "llg-build\tresult=ok\ttotal_ms=1300\tseed_ms=1\tseed=applied\tconfigure_ms=900\ttemplate_ms=6\ttemplate=captured",
        ]
        text = timings.summarize(timings.parse(lines))
        self.assertIn("template: -=3, applied=1, captured=1", text)
        self.assertIn("template=applied: builds=1 configure median=none total median=300ms", text)
        self.assertIn("template=captured: builds=1 configure median=900ms total median=1300ms", text)
        template = next(line for line in text.splitlines() if line.startswith("template "))
        self.assertEqual(template.split()[1:3], ["2", "0.0"])

    def test_template_rejection_reasons_are_listed(self):
        lines = LINES + [
            "llg-build\tresult=ok\ttotal_ms=1300\ttemplate=rejected\ttemplate_reason=template differs from a real configure: CMakeCache.txt: line 3: `a` vs `b`\tdir=C:\\x",
        ]
        text = timings.summarize(timings.parse(lines))
        self.assertIn("template: -=3, rejected=1", text)
        self.assertIn("template rejected (1): template differs from a real configure: CMakeCache.txt: line 3: `a` vs `b`", text)

    def test_generate_sub_phases_and_slowest_builds_are_reported(self):
        lines = LINES + [
            "llg-build\tresult=ok\ttotal_ms=900\tend_unix_ms=1760000120000\tgenerate_ms=700\tgen_check_ms=5"
            "\tgen_write_ms=690\tgen_tree=new\tgen_files=140\tgen_bytes=4000000\tgen_model_bytes=90000"
            "\tgen_dir_calls=9\tdir=C:\\t\\big",
            "llg-build\tresult=ok\ttotal_ms=300\tend_unix_ms=1760000000000\tgenerate_ms=40\tgen_check_ms=1"
            "\tgen_write_ms=30\tgen_prune_ms=9\tgen_tree=existing\tgen_files=139\tdir=/tmp/small",
        ]
        text = timings.summarize(timings.parse(lines))
        self.assertIn("gen_tree: -=3, existing=1, new=1", text)
        write = next(line for line in text.splitlines() if line.startswith("gen_write "))
        self.assertEqual(write.split()[1:3], ["2", "0.7"])
        prune = next(line for line in text.splitlines() if line.startswith("gen_prune "))
        self.assertEqual(prune.split()[1], "1")
        rows = text.splitlines()
        first = rows.index("slowest generate phases (of 5):")
        self.assertEqual(rows[first + 2].split(),
                         ["700", "5", "690", "-", "140", "4000000", "90000", "new", "+2m", "C:\\t\\big"])
        self.assertEqual(rows[first + 3].split()[:4], ["40", "1", "30", "9"])
        self.assertEqual(rows[first + 3].split()[-2:], ["+0m", "/tmp/small"])
        self.assertEqual(rows[first + 4].split()[0], "10")
        self.assertEqual(timings.slowest_generate([{"total_ms": "5"}]), [])

    def test_windows_show_phase_medians_by_build_end_time(self):
        start = 1760000000000
        lines = [
            f"llg-build\tresult=ok\ttotal_ms={total}\tend_unix_ms={start + offset}\tconfigure_ms={configure}"
            for offset, total, configure in (
                (0, 1000, 300), (60000, 1200, 500), (14 * 60000, 1100, 400),
                (16 * 60000, 7000, 3000), (40 * 60000, 9000, 2900),
            )
        ] + LINES
        text = timings.windows(timings.parse(lines), 15)
        rows = text.splitlines()
        self.assertEqual(rows[0], "phase medians (ms) per 15-minute window of build end time:")
        self.assertEqual(rows[2].split(), ["+0m", "3", "-", "-", "-", "400", "-", "1100"])
        self.assertEqual(rows[3].split(), ["+15m", "1", "-", "-", "-", "3000", "-", "7000"])
        self.assertEqual(rows[4].split(), ["+30m", "1", "-", "-", "-", "2900", "-", "9000"])
        self.assertEqual(len(rows), 5)
        self.assertEqual(timings.windows(timings.parse(LINES)), "")

    def test_main_prints_windows_only_for_stamped_records(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "t.tsv"
            path.write_text("\n".join(LINES + ["llg-build\tresult=ok\ttotal_ms=5\tend_unix_ms=10"]), encoding="utf-8")
            output = io.StringIO()
            with redirect_stdout(output):
                self.assertEqual(timings.main([str(path), "--window-minutes", "5"]), 0)
            self.assertIn("per 5-minute window", output.getvalue())

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
