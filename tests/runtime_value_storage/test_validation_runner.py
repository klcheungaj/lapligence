import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from validation_runner import BASE_TESTS, Runner, find_product, verify_inventory, verify_measurement


class InventoryTests(unittest.TestCase):
    def capabilities(self, **values):
        return dict(schema="llg.owner-probes/v1", waveforms=False, scheduler=False,
                    coroutines=False, sanitizers=False, **values)

    def inventory(self):
        return {"tests": [{"name": name} for name in sorted(BASE_TESTS)]}

    def test_exact_portable_inventory(self):
        self.assertEqual(verify_inventory(self.inventory(), self.capabilities()), sorted(BASE_TESTS))

    def test_original_fixtures_follow_enabled_runtime_components(self):
        scheduler = {"coroutine_chunk_cache", "port_net_collapse_values", "port_net_collapse_publication", "vpi_ownership", "scheduler_ownership", "generated_scope_patterns",
                     "scope_address_index", "scheduler_lists", "wait_inline_storage", "clocking_history_index", "runtime_value_vectors", "event_array_selection",
                     "frame_value_cells", "frame_value_cells_reject_escape",
                     "file_input_isolation", "file_output_isolation", "memory_image", "native_value_scopes", "native_reference_scopes", "review_native_index_and_reference_bits",
                     "packed_selection_nba", "packed_selection_input", "packed_formal_owner_contracts"}
        scheduler.update({"stackless_runtime_boundaries", "coroutine_scale_release", "coroutine_scale_debug",
                          "coroutine_debug_fresh_poison", "coroutine_debug_cancelled_arena_reuse",
                          "coroutine_mixed_mode_0_0", "coroutine_mixed_mode_0_1",
                          "coroutine_mixed_mode_1_0", "coroutine_mixed_mode_1_1"})
        scheduler.update(f"coroutine_debug_reject_{mode}" for mode in
                         ("root-tag", "static-enter", "static-tag", "anchor-enter", "anchor-tag", "anchored-root-tag",
                          "invalid-state", "poison-state", "finished", "reentrant", "exiting-service"))
        scheduler.update(f"patch_review_{mode}" for mode in
                         ("forced_real_nba", "force_overlap", "force_partial_release", "force_concat_release",
                          "force_slot_reuse", "event_cleanup", "fork_self_disable", "fork_ancestor_disable",
                          "reactive_fixed_point", "pre_nba_reentry", "preponed_once", "finish_pending",
                          "monitor_reenable", "event_readonly", "early_spawn", "callback_nba"))
        coroutines = {"coroutine_ownership", "generated_coroutine_patterns", "callback_finish_ownership",
                      "runtime_original_selftest", "runtime_region", "runtime_stop-resume",
                      "runtime_budget-finite", "event_array_waits", "nextest_control_ownership", "native_input_callbacks",
                      "native_mailbox_stream_callbacks", "review_real_coroutine_storage"}
        for waveforms in (False, True):
            for has_scheduler in (False, True):
                for has_coroutines in (False, True):
                    if has_coroutines and not has_scheduler:
                        continue
                    caps = self.capabilities()
                    caps.update(waveforms=waveforms, scheduler=has_scheduler, coroutines=has_coroutines)
                    names = set(BASE_TESTS)
                    if waveforms:
                        names.add("waveform_snapshot_lifecycle")
                    if has_scheduler:
                        names.update(scheduler)
                        if waveforms:
                            names.add("waveform_original_selftest")
                    if has_coroutines:
                        names.update(coroutines)
                    inventory = {"tests": [{"name": name} for name in sorted(names)]}
                    self.assertEqual(verify_inventory(inventory, caps), sorted(names))
                    caps["sanitizers"] = True
                    self.assertEqual(verify_inventory(inventory, caps), sorted(names))

    def test_no_tests_is_not_a_pass(self):
        with self.assertRaises(ValueError):
            verify_inventory({"tests": []}, self.capabilities())

    def test_missing_and_extra_tests_are_errors(self):
        inventory = self.inventory()
        inventory["tests"].pop()
        with self.assertRaises(ValueError):
            verify_inventory(inventory, self.capabilities())
        inventory = self.inventory()
        inventory["tests"].append({"name": "unexpected"})
        with self.assertRaises(ValueError):
            verify_inventory(inventory, self.capabilities())

    def test_missing_manifest_cannot_hide_scheduler_tests(self):
        with self.assertRaises(ValueError):
            verify_inventory(self.inventory(), {})

    def test_disabled_test_is_not_a_pass(self):
        inventory = self.inventory()
        inventory["tests"][0]["properties"] = [{"name": "DISABLED", "value": True}]
        with self.assertRaises(ValueError):
            verify_inventory(inventory, self.capabilities())

    def test_duplicate_test_is_not_a_pass(self):
        inventory = self.inventory()
        inventory["tests"].append(inventory["tests"][0])
        with self.assertRaises(ValueError):
            verify_inventory(inventory, self.capabilities())

    def test_coroutines_require_scheduler(self):
        caps = self.capabilities()
        caps.update(scheduler=False, coroutines=True, sanitizers=True)
        with self.assertRaises(ValueError):
            verify_inventory(self.inventory(), caps)


class MeasurementTests(unittest.TestCase):
    def measurement(self):
        return dict(schema="llg.value-lifetime/v1", slots=10, rounds=20,
                    steady_payload_bytes=240, peak_payload_bytes=264,
                    steady_live_allocations=10, peak_live_allocations=11,
                    allocations_during_cycles=200, ending_live_allocations=0,
                    ending_payload_bytes=0)

    def test_exact_plateau(self):
        verify_measurement(self.measurement())

    def test_leak_is_not_a_pass(self):
        result = self.measurement()
        result["ending_payload_bytes"] = 24
        with self.assertRaises(ValueError):
            verify_measurement(result)

    def test_empty_workload_is_not_a_pass(self):
        result = self.measurement()
        result["rounds"] = 0
        with self.assertRaises(ValueError):
            verify_measurement(result)


class ExecutionTests(unittest.TestCase):
    def test_results_and_atomic_report(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            runner = Runner(root, root / "evidence", timeout=10)
            ok = runner.run("ok", [sys.executable, "-c", "print('measured')"])
            failure = runner.run("failure", [sys.executable, "-c", "raise SystemExit(7)"])
            missing = runner.run("missing", [str(root / "absent-command")])
            self.assertEqual(ok["status"], "passed")
            self.assertEqual(failure["returncode"], 7)
            self.assertEqual(failure["status"], "failed")
            self.assertEqual(missing["status"], "blocked")
            report = runner.save({"scope": "unit-test"})
            self.assertEqual(len(json.loads(report.read_text())["checks"]), 3)
            self.assertFalse(report.with_suffix(".tmp").exists())

    def test_timeout_is_not_a_pass(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            record = Runner(root, root / "evidence", timeout=0.1).run(
                "timeout", [sys.executable, "-c", "import time; time.sleep(60)"])
            self.assertEqual(record["status"], "failed")
            self.assertEqual(record["reason"], "command timed out")

    def test_optimized_python_cannot_bypass_oracle(self):
        result = subprocess.run(
            [sys.executable, "-O", str(Path(__file__).with_name("value_oracle.py")),
             "--dynamic", "unused-library"],
            stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=10,
            check=False,
        )
        self.assertEqual(result.returncode, 2)
        self.assertIn("integer oracle requires assertions", result.stderr)
        self.assertNotIn("checks passed", result.stdout)

    def test_products_require_exact_single_match(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with self.assertRaises(ValueError):
                find_product(root, "Release", ["probe"])
            (root / "probe").write_text("single")
            self.assertEqual(find_product(root, "Release", ["probe"]), root / "probe")
            (root / "Release").mkdir()
            (root / "Release/probe").write_text("stale")
            with self.assertRaises(ValueError):
                find_product(root, "Release", ["probe"])


if __name__ == "__main__":
    unittest.main()
