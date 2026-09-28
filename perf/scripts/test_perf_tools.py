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


if __name__ == "__main__":
    unittest.main()
