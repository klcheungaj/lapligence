import importlib.util
import unittest
from pathlib import Path


SCRIPT_DIR = Path(__file__).resolve().parent


def load_module(name):
    spec = importlib.util.spec_from_file_location(name, SCRIPT_DIR / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


vb = load_module("value_backends")


def run_row(workload, backend, round_index, cpu, wall=None, rss=1000, status=0):
    return {
        "workload": workload, "backend": backend, "round": round_index, "status": status,
        "cpu_s": cpu, "wall_s": cpu if wall is None else wall, "max_rss_kib": rss,
    }


class ParserTests(unittest.TestCase):
    def test_heaptrack_summary_uses_exact_counts_and_decimal_units(self):
        text = """total runtime: 0.06s.
calls to allocation functions: 54051 (857952/s)
temporary memory allocations: 2004 (31809/s)
peak heap memory consumption: 81.99K
peak RSS (including heaptrack overhead): 4.72M
total memory leaked: 0B
"""
        self.assertEqual(vb.parse_heaptrack_summary(text), {
            "alloc_calls": 54051,
            "temporary_allocs": 2004,
            "peak_heap_bytes": 81990,
            "peak_rss_heaptrack_bytes": 4720000,
            "leaked_bytes": 0,
        })

    def test_rejects_unknown_byte_quantity(self):
        with self.assertRaises(ValueError):
            vb.parse_human_bytes("12 parsecs")

    def test_layout_and_bench_records(self):
        layout = vb.parse_layout("layout\tdescriptor_bytes\t24\npayload\t65\t16\t32\n")
        self.assertEqual(layout["descriptor_bytes"], 24)
        self.assertEqual(layout["payload"][65], {"known": 16, "xz": 32})
        rows = vb.parse_bench_timing("timing\t65\tknown\tmul\tfresh\t64\t3.0\t1.0\t2.0\n", "legacy", 4)
        self.assertEqual(rows, [{"backend": "legacy", "round": 4, "width": 65, "state": "known",
                                 "op": "mul", "mode": "fresh", "ns": 2.0}])

    def test_measure_record_fields(self):
        self.assertEqual(vb.parse_measure_record("run\t0\t1500000000\t2048\n"),
                         {"status": 0, "wall_ns": 1500000000, "max_rss_kib": 2048})

    def test_census_counts_destination_forms_and_constructor_widths(self):
        model = """sv4_mul_to(&t[0], &a, &b);
sv4_replace(&x, sv4_add(a, b));
sv4_destroy(&t[0]);
sv4_from_masks_to(&t[0], 1000ULL, 0ULL, 0ULL, 32, 1);
sv4_zero_to(&g, 65, 0);
"""
        result = vb.census(model)
        self.assertEqual(result["sites"]["sv4_mul_to"], 1)
        self.assertEqual(result["destination_sites"], 3)
        self.assertEqual(result["replace_sites"], 1)
        self.assertEqual(result["destroy_sites"], 1)
        self.assertEqual(result["constructor_width_classes"], {"<=64": 1, "65-128": 1})

    def test_compile_definitions_come_from_generated_project(self):
        text = "add_compile_definitions(LLG_SV4_USE_GMP=1 LLG_SV4_GMP_KERNELS=0)\n"
        self.assertEqual(vb.compile_definitions(text),
                         ["-DLLG_SV4_USE_GMP=1", "-DLLG_SV4_GMP_KERNELS=0"])


class OrderTests(unittest.TestCase):
    def test_every_backend_takes_every_position(self):
        backends = ["legacy", "compact-portable", "compact-gmp"]
        positions = {backend: set() for backend in backends}
        for index in range(6):
            order = vb.round_order(backends, index)
            self.assertEqual(sorted(order), sorted(backends))
            for position, backend in enumerate(order):
                positions[backend].add(position)
        self.assertTrue(all(found == {0, 1, 2} for found in positions.values()))


class SummaryTests(unittest.TestCase):
    def test_ratios_use_medians_and_pairs_and_skip_failures(self):
        rows = [
            run_row("w", "legacy", 0, 2.0), run_row("w", "compact-gmp", 0, 1.0),
            run_row("w", "legacy", 1, 4.0), run_row("w", "compact-gmp", 1, 1.0),
            run_row("w", "legacy", 2, 3.0), run_row("w", "compact-gmp", 2, 3.0),
            run_row("w", "compact-gmp", 3, 0.1, status=1),
        ]
        summary = vb.summarize(rows)
        compact = summary[("w", "compact-gmp")]
        self.assertEqual(compact["cpu_s"]["median"], 1.0)
        self.assertEqual(compact["cpu_s"]["n"], 3)
        self.assertAlmostEqual(compact["cpu_s"]["ratio"], 1.0 / 3.0)
        self.assertEqual(compact["cpu_s"]["paired_ratio"]["median"], 0.5)
        self.assertEqual(compact["cpu_s"]["paired_ratio"]["max"], 1.0)

    def test_budget_findings_flag_only_ratios_above_limit(self):
        rows = [run_row("mul65", "legacy", 0, 2.0, rss=100), run_row("mul65", "compact-gmp", 0, 2.2, rss=100)]
        summary = vb.summarize(rows)
        budgets = {"default": {"cpu_ratio_max": 1.5},
                   "classes": {"mul65-witness": {"cpu_ratio_max": 1.0, "rss_ratio_max": 1.02,
                                                 "alloc_calls_ratio_max": 1.0}}}
        heap = {("mul65", "legacy"): {"alloc_calls": 10}, ("mul65", "compact-gmp"): {"alloc_calls": 5}}
        findings = {item["metric"]: item for item in vb.evaluate_budgets(summary, budgets, heap)}
        self.assertFalse(findings["cpu_s"]["within"])
        self.assertTrue(findings["max_rss_kib"]["within"])
        self.assertTrue(findings["alloc_calls"]["within"])
        self.assertNotIn("wall_s", findings)


if __name__ == "__main__":
    unittest.main()
