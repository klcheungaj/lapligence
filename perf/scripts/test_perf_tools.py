import importlib.util
import struct
import tempfile
import unittest
from pathlib import Path
from unittest import mock


SCRIPT_DIR = Path(__file__).resolve().parent


def load_module(name):
    spec = importlib.util.spec_from_file_location(name, SCRIPT_DIR / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


profile = load_module("profile_symbolize")
rss = load_module("rss_attribution")
summary = load_module("corpus_summary")
compile_time = load_module("compile_time")


class ProfileParserTests(unittest.TestCase):
    def test_reads_fixed_size_samples_and_rejects_partial_record(self):
        header = profile.HEADER.pack(profile.MAGIC, 1, 8, 2, 0)
        record = struct.Struct("=HHI2Q").pack(2, 0, 7, 0x1234, 0x5678)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "samples.raw"
            path.write_bytes(header + record)
            self.assertEqual(profile.read_samples(path), [[0x1234, 0x5678]])
            path.write_bytes(header + record[:-1])
            with self.assertRaisesRegex(ValueError, "partial sample"):
                profile.read_samples(path)

    def test_symbolizes_unique_addresses_in_one_batch(self):
        completed = mock.Mock(stdout="first\nfirst.c:1\nsecond\nsecond.c:2\n")
        with mock.patch.object(
            profile, "locate", side_effect=lambda address, _: ("/fake/model", address)
        ), mock.patch.object(profile.subprocess, "run", return_value=completed) as run:
            result = profile.symbolize([[1, 2], [2]], [], "addr2line")
        self.assertEqual(result, [["first", "second"], ["second"]])
        run.assert_called_once()


class RssClassifierTests(unittest.TestCase):
    def test_classifies_sparse_large_anonymous_mapping_as_save_stacks(self):
        mapping = {
            "start": 0x1000,
            "end": 0x401000,
            "perms": "rw-p",
            "path": "",
            "Size": 1048576,
            "Rss": 4096,
        }
        self.assertEqual(rss.classify_mapping(mapping), "libaco_save_stacks")

    def test_parses_and_summarizes_heap_mapping(self):
        text = """1000-3000 rw-p 00000000 00:00 0 [heap]
Size:                  8 kB
Rss:                   4 kB
Pss:                   4 kB
Private_Dirty:         4 kB
VmFlags: rd wr mr mw me ac sd
"""
        mappings = rss.parse_smaps(text)
        totals = rss.summarize(mappings)
        self.assertEqual(totals["heap"]["size_kib"], 8)
        self.assertEqual(totals["heap"]["rss_kib"], 4)


class CorpusSummaryTests(unittest.TestCase):
    def test_computes_medians_and_rejects_mixed_output_hashes(self):
        rows = []
        for run, elapsed, digest in ((1, "3", "same"), (2, "1", "different")):
            rows.append(
                {
                    "config": "example",
                    "mode": "default",
                    "run": str(run),
                    "generation_status": "0",
                    "generation_ms": "2",
                    "generation_rss_kib": "10",
                    "build_status": "0",
                    "build_ms": "4",
                    "build_rss_kib": "20",
                    "simulation_status": "0",
                    "simulation_ms": elapsed,
                    "simulation_rss_kib": "30",
                    "executable_bytes": "40",
                    "stdout_sha256": digest,
                }
            )
        result = summary.summarize(rows)[0]
        self.assertEqual(result["median_simulation_ms"], 2)
        self.assertEqual(result["all_status_zero"], 1)
        self.assertEqual(result["stdout_sha256"], "MIXED")


class CompileTimeTests(unittest.TestCase):
    GCC_REPORT = """Time variable                                   usr           sys          wall           GGC
 phase setup                        :   0.00 (  0%)   0.00 (  0%)   0.10 (  0%)  1841k ( 86%)
 phase parsing                      :   1.00 ( 10%)   0.50 ( 50%)   2.00 ( 20%)   167M (  8%)
 phase lang. deferred               :   0.00 (  0%)   0.00 (  0%)   0.40 (  4%)   140k (  7%)
 phase opt and generate             :   6.00 ( 60%)   0.50 ( 50%)   7.00 ( 70%)   140M (  7%)
 callgraph ipa passes               :   0.50 (  5%)   0.00 (  0%)   0.60 (  6%)    12k (  1%)
 |name lookup                       :   0.10 (  1%)   0.00 (  0%)   0.20 (  2%)  9480  (  0%)
 tree PTA                           :   3.00 ( 30%)   0.00 (  0%)   3.50 ( 35%)     1G (  0%)
 TOTAL                              :  10.00          1.00         10.20         2149M
"""

    def test_parses_gcc_phases_passes_and_memory(self):
        report = compile_time.parse_gcc_time_report(self.GCC_REPORT)
        self.assertAlmostEqual(report["front_end_s"], 2.5)
        self.assertAlmostEqual(report["optimize_s"], 7.0)
        self.assertAlmostEqual(report["ipa_s"], 0.6)
        self.assertAlmostEqual(report["compiler_total_s"], 10.2)
        self.assertEqual(report["compiler_mem_kib"], 2149 * 1024)
        self.assertEqual(report["passes"][0], ("tree PTA", 3.5, 1024 * 1024))
        self.assertIn(("name lookup", 0.2, 9), report["passes"])
        self.assertFalse(any(name.startswith("phase ") for name, _, _ in report["passes"]))

    def test_parses_clang_time_trace_totals(self):
        trace = {
            "traceEvents": [
                {"name": "Total Frontend", "dur": 2_000_000},
                {"name": "Total Backend", "dur": 5_000_000},
                {"name": "Total ExecuteCompiler", "dur": 7_500_000},
                {"name": "Total InstCombinePass", "dur": 1_000_000},
                {"name": "InstCombinePass", "dur": 9_000_000},
            ]
        }
        report = compile_time.parse_clang_time_trace(trace)
        self.assertEqual(report["front_end_s"], 2.0)
        self.assertEqual(report["optimize_s"], 5.0)
        self.assertEqual(report["compiler_total_s"], 7.5)
        self.assertEqual([name for name, _, _ in report["passes"]][:2], ["ExecuteCompiler", "Backend"])

    def test_retargets_compile_command_for_reports_and_preprocessing(self):
        entry = {
            "directory": "/b",
            "command": "/usr/bin/cc -O2 -Wall -o CMakeFiles/sim.dir/model.c.o -c /s/model.c",
            "file": "/s/model.c",
        }
        arguments = compile_time.compile_arguments(entry)
        self.assertEqual(
            compile_time.retarget(arguments, "/w/t.o", ["-ftime-report"]),
            ["/usr/bin/cc", "-ftime-report", "-O2", "-Wall", "-c", "/s/model.c", "-o", "/w/t.o"],
        )
        self.assertEqual(
            compile_time.retarget(arguments, "/w/t.i", [], preprocess=True),
            ["/usr/bin/cc", "-E", "-O2", "-Wall", "/s/model.c", "-o", "/w/t.i"],
        )
        self.assertEqual(compile_time.object_path(entry), Path("/b/CMakeFiles/sim.dir/model.c.o"))

    def test_selects_model_translation_units_and_design_sets(self):
        entries = [{"file": f"/s/{name}", "directory": "/b", "command": "cc"}
                   for name in ("llg_rt.c", "model_part_1.c", "model.c", "llg_co.c")]
        self.assertEqual(
            [Path(entry["file"]).name for entry in compile_time.model_entries(entries)],
            ["model.c", "model_part_1.c"],
        )
        designs = compile_time.design_catalog()
        smoke = compile_time.designs_in_set(designs, "smoke")
        ladder = compile_time.designs_in_set(designs, "ladder")
        self.assertEqual(sorted(smoke), ["pca-512", "tasks-16"])
        self.assertTrue(set(smoke) < set(ladder) < set(compile_time.designs_in_set(designs, "large")))
        for design in designs.values():
            self.assertTrue((compile_time.CORPUS_DIR / design.file).is_file())


if __name__ == "__main__":
    unittest.main()
