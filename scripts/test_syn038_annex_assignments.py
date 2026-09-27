import copy
import json
from pathlib import Path
import re
import unittest

import check_syn038_annex_assignments as checker


ROOT = Path(__file__).resolve().parents[1]


class AnnexAssignmentCheckerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.manifest = json.loads((ROOT / "tests/syn038_annex_assignments.json").read_text())
        cls.ledger = (ROOT / "tests/syn038_coverage_ledger.md").read_text()
        cls.inventory = checker.load_inventory_module()

    def check_rejects(self, manifest, fragment):
        with self.assertRaises(SystemExit) as failure:
            checker.check_manifest(manifest, self.ledger, self.inventory, None)
        self.assertIn(fragment, str(failure.exception))

    def test_missing_extracted_production_fails(self):
        manifest = copy.deepcopy(self.manifest)
        manifest["assignments"] = [entry for entry in manifest["assignments"] if entry["production"] != "genvar_function_call"]
        self.check_rejects(manifest, "unassigned extracted names")

    def test_comma_separated_pdf_footnote_is_retained(self):
        source = "dpi_function_proto21,22 ::= function_prototype"
        matches = self.inventory.PDF_PRODUCTION.findall(source)
        self.assertEqual([re.sub(r"\d+(?:,\d+)*$", "", name) for name in matches], ["dpi_function_proto"])
        self.assertIn("dpi_function_proto", self.manifest["source_names"]["SV2009_PDF"])

    def test_unknown_core_ledger_row_fails(self):
        manifest = copy.deepcopy(self.manifest)
        entry = next(entry for entry in manifest["assignments"] if entry["production"] == "genvar_function_call")
        entry["ledger_rows"] = ["SYN038-CORE-HY-99"]
        self.check_rejects(manifest, "nonexistent ledger row")

    def test_unrelated_positive_row_cannot_cover_production(self):
        manifest = copy.deepcopy(self.manifest)
        entry = next(entry for entry in manifest["assignments"] if entry["production"] == "genvar_function_call")
        entry["ledger_rows"] = ["SYN038-CORE-LX-01"]
        self.check_rejects(manifest, "without a named production witness")

    def test_rejected_control_cannot_count_as_positive_core(self):
        manifest = copy.deepcopy(self.manifest)
        entry = next(entry for entry in manifest["assignments"] if entry["production"] == "genvar_function_call")
        entry["ledger_rows"] = ["SYN038-CORE-AS-06"]
        self.check_rejects(manifest, "counts rejected source")

    def test_core_row_requires_real_fixture(self):
        ledger = self.ledger.replace(
            "`tests/fixtures/sim/review_bundle/r12_genvar_function_call.sv`",
            "`tests/fixtures/sim/review_bundle/missing_genvar_fixture.sv`",
        )
        with self.assertRaises(SystemExit) as failure:
            checker.check_manifest(self.manifest, ledger, self.inventory, None)
        self.assertIn("without real fixtures", str(failure.exception))

    def test_unassigned_alias_parent_fails(self):
        manifest = copy.deepcopy(self.manifest)
        entry = next(entry for entry in manifest["assignments"] if entry["disposition"] == "ALIAS/HELPER")
        entry["parents"] = ["not_an_annex_production"]
        self.check_rejects(manifest, "parent not_an_annex_production is unassigned")

    def test_selected_extended_cannot_be_called_excluded(self):
        manifest = copy.deepcopy(self.manifest)
        entry = next(entry for entry in manifest["assignments"] if entry["production"] == "combinational_entry")
        entry["disposition"] = "EXCLUDED"
        entry["reason"] = "SYN-031 outside Core"
        self.check_rejects(manifest, "must map to selected Extended SYN-031 evidence")

    def test_sequential_udp_cannot_borrow_combinational_evidence(self):
        manifest = copy.deepcopy(self.manifest)
        selected = next(entry for entry in manifest["assignments"] if entry["production"] == "combinational_entry")
        entry = next(entry for entry in manifest["assignments"] if entry["production"] == "sequential_entry")
        entry["disposition"] = "EXTENDED"
        entry["evidence"] = copy.deepcopy(selected["evidence"])
        self.check_rejects(manifest, "must remain outside selected combinational UDP")

    def test_timing_zero_or_one_cannot_borrow_udp_evidence(self):
        manifest = copy.deepcopy(self.manifest)
        selected = next(entry for entry in manifest["assignments"] if entry["production"] == "combinational_entry")
        entry = next(entry for entry in manifest["assignments"] if entry["production"] == "zero_or_one")
        entry["disposition"] = "EXTENDED"
        entry["evidence"] = copy.deepcopy(selected["evidence"])
        self.check_rejects(manifest, "timing descriptor, not a UDP table symbol")

    def test_extended_owner_must_exist(self):
        manifest = copy.deepcopy(self.manifest)
        entry = next(entry for entry in manifest["assignments"] if entry["production"] == "combinational_entry")
        entry["evidence"]["owner"] = "tests/sim_udp.rs::missing_udp_test"
        self.check_rejects(manifest, "has no real evidence test function")

    def test_implemented_event_cannot_be_called_unsupported(self):
        manifest = copy.deepcopy(self.manifest)
        entry = next(entry for entry in manifest["assignments"] if entry["production"] == "event_declaration")
        entry["disposition"] = "EXCLUDED"
        entry["reason"] = "SYN-034 unsupported event"
        self.check_rejects(manifest, "must retain its implemented outside-Core evidence")

    def test_outside_core_evidence_requires_product_row(self):
        manifest = copy.deepcopy(self.manifest)
        entry = next(entry for entry in manifest["assignments"] if entry["production"] == "virtual_interface_declaration")
        entry["evidence"]["docs_heading"] = "**Absent virtual interfaces**"
        self.check_rejects(manifest, "lacks its product documentation row")


if __name__ == "__main__":
    unittest.main()
