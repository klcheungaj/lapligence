import io
import json
import os
import sys
import tempfile
import time
import unittest
from contextlib import redirect_stdout
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import ci_resource_sampler as sampler

WINDOWS_TEXT = "\n".join([
    "P|4|0|System|50000000|1048576",
    "P|100|4|MsMpEng.exe|300000000|209715200",
    "P|200|150|cmake.exe|20000000|10485760",
    "P|201|200|ninja.exe|10000000|5242880",
    "P|202|999|cl.exe|40000000|52428800",
    "P|bad|line",
    "M|16777216|4194304|20971520|8388608",
    "F|C:\\pagefile.sys|4096|512",
    "",
])


class ResourceSamplerTest(unittest.TestCase):
    def test_windows_query_output_is_parsed(self):
        processes, memory, pagefiles = sampler.parse_windows(WINDOWS_TEXT)
        self.assertEqual([p["name"] for p in processes], ["system", "msmpeng", "cmake", "ninja", "cl"])
        self.assertEqual(processes[1]["cpu_s"], 30.0)
        self.assertEqual(processes[1]["rss_mb"], 200.0)
        self.assertEqual(memory, {"total_mb": 16384, "available_mb": 4096, "commit_mb": 12288,
                                  "commit_limit_mb": 20480})
        self.assertEqual(pagefiles, [{"name": "C:\\pagefile.sys", "allocated_mb": 4096, "used_mb": 512}])

    def test_cpu_top_ranks_names_by_cpu_used_in_the_interval(self):
        previous, _, _ = sampler.parse_windows(WINDOWS_TEXT)
        current = [dict(p) for p in previous]
        for process in current:
            if process["name"] == "msmpeng":
                process["cpu_s"] += 60.0
            if process["name"] == "cl":
                process["cpu_s"] += 30.0
        current.append({"pid": 300, "ppid": 4, "name": "cl", "cpu_s": 15.0, "rss_mb": 1.0})
        top = sampler.cpu_top(previous, current, 30.0, 4)
        self.assertEqual(top, [["msmpeng", 50.0], ["cl", 37.5]])

    def test_counts_name_watched_and_orphaned_processes(self):
        processes, _, _ = sampler.parse_windows(WINDOWS_TEXT)
        counts = sampler.process_counts(processes)
        self.assertEqual(counts["total"], 5)
        self.assertEqual(counts["watched"], {"cl": 1, "cmake": 1, "msmpeng": 1, "ninja": 1})
        self.assertEqual(counts["orphaned"], {"cl": 1, "cmake": 1})

    def test_ps_and_meminfo_are_parsed(self):
        processes = sampler.parse_ps("  10     1 01:02:03  2048 /usr/bin/cmake\n  11    10 1-00:00:01 1024 ninja\nbad\n")
        self.assertEqual([(p["name"], p["cpu_s"], p["rss_mb"]) for p in processes],
                         [("cmake", 3723.0, 2.0), ("ninja", 86401.0, 1.0)])
        self.assertEqual(sampler.cpu_seconds("00:05.50"), 5.5)
        memory = sampler.parse_meminfo("MemTotal: 2048 kB\nMemAvailable: 1024 kB\nCommitted_AS: 512 kB\n"
                                       "CommitLimit: 4096 kB\nSwapTotal: 1024 kB\nSwapFree: 512 kB\n")
        self.assertEqual(memory, {"total_mb": 2, "available_mb": 1, "commit_mb": 0, "commit_limit_mb": 4,
                                  "swap_used_mb": 0})
        self.assertEqual(sampler.parse_meminfo(""), {})

    def test_trees_report_size_and_file_count(self):
        with tempfile.TemporaryDirectory() as directory:
            Path(directory, "a").mkdir()
            Path(directory, "a", "x").write_bytes(b"1" * 1048576)
            Path(directory, "y").write_bytes(b"")
            self.assertEqual(sampler.tree_stats(directory), {"mb": 1.0, "files": 2})
            self.assertEqual(sampler.tree_stats(os.path.join(directory, "absent")), {"mb": 0.0, "files": 0})
            self.assertEqual(sampler.parse_trees(["scratch=" + directory, "bad", "=x"]), {"scratch": directory})

    def test_summary_lines_name_every_measure(self):
        line = sampler.summary_line({
            "t": "10:00:00", "memory": {"available_mb": 1, "total_mb": 2, "commit_mb": 3, "commit_limit_mb": 4},
            "pagefiles": [{"used_mb": 5}], "disk_free_gb": {"C:\\": 6.0},
            "processes": {"total": 7, "watched": {"cl": 2}, "orphaned": {"cl": 1}},
            "cpu_top": [["msmpeng", 80.0]], "trees": {"temp": {"mb": 8.0, "files": 9}}})
        self.assertEqual(line, "10:00:00 | avail=1/2MB commit=3/4MB | pagefile=5MB | free=C:\\6.0G | procs=7 | "
                               "cl=2 | orphaned: cl=1 | cpu% msmpeng=80.0 | trees temp=8.0MB/9f")
        self.assertEqual(sampler.summary_line({"t": "1", "error": "x"}), "1 sample error: x")

    def test_started_sampler_records_until_the_report_stops_it(self):
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory, "samples.jsonl")
            output = io.StringIO()
            with redirect_stdout(output):
                self.assertEqual(sampler.main(["start", "--log", str(log), "--interval", "0.2",
                                               "--tree", "work=" + directory, "--tree-every", "1"]), 0)
            self.assertIn("resource sampler started", output.getvalue())
            deadline = time.monotonic() + 30
            while time.monotonic() < deadline and (not log.exists() or len(log.read_text().splitlines()) < 2):
                time.sleep(0.1)
            output = io.StringIO()
            with redirect_stdout(output):
                self.assertEqual(sampler.main(["report", "--log", str(log)]), 0)
            records = [json.loads(line) for line in log.read_text().splitlines()]
            self.assertGreaterEqual(len(records), 2)
            self.assertIn("work", records[0]["trees"])
            self.assertIn("procs=", output.getvalue())
            time.sleep(1.5)
            count = len(log.read_text().splitlines())
            time.sleep(1.0)
            self.assertEqual(len(log.read_text().splitlines()), count)

    def test_report_without_samples_is_not_an_error(self):
        with tempfile.TemporaryDirectory() as directory:
            output = io.StringIO()
            with redirect_stdout(output):
                self.assertEqual(sampler.main(["report", "--log", str(Path(directory, "none.jsonl"))]), 0)
            self.assertIn("no resource samples", output.getvalue())


if __name__ == "__main__":
    unittest.main()
