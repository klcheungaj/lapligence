from __future__ import annotations

from copy import deepcopy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

import check_syn038_pairwise_manifest as checker
from syn038_pairwise_source import canonical_sha256, expand_source, format_source, read_source


class PairwiseSourceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.source = read_source(checker.DEFAULT_MANIFEST)
        cls.expanded = expand_source(cls.source)
        cls.catalog = checker.generated_catalog(cls.expanded)

    def source_copy(self):
        return deepcopy(self.source)

    def run_checker(self, source, *args):
        with tempfile.TemporaryDirectory(prefix="syn038-source-") as directory:
            path = Path(directory) / "source.json"
            before = format_source(source).encode("utf-8")
            path.write_bytes(before)
            result = subprocess.run(
                [sys.executable, str(Path(checker.__file__)), "--manifest", str(path), *args],
                capture_output=True, text=True, timeout=120, cwd=directory,
            )
            after = path.read_bytes()
            self.assertEqual({item.name for item in Path(directory).iterdir()}, {"source.json"})
            return result, before, after

    def test_reviewed_coverage_and_evidence_are_preserved(self):
        counts = checker.validate_manifest(self.expanded, self.catalog)
        self.assertEqual(len(self.source["factors"]), 13)
        self.assertEqual(len(self.expanded["evidence"]), 122)
        self.assertEqual(len(self.expanded["observations"]), 676)
        self.assertEqual(len(self.source["qualifier_sets"]), 104)
        self.assertEqual(len(self.catalog), 2247)
        self.assertEqual(dict(counts), {
            "selected_core": 1849, "impossible": 296, "outside_profile": 102,
            "covered": 1849, "planned_legal_gap": 0,
        })
        self.assertEqual(self.source["catalog_sha256"], canonical_sha256(self.catalog))
        self.assertNotIn("pair_catalog", self.source)
        self.assertNotIn("pair_catalog", self.expanded)

    def test_text_round_trip_is_stable(self):
        text = format_source(self.source)
        self.assertEqual(text, format_source(json.loads(text)))
        self.assertEqual(expand_source(json.loads(text)), self.expanded)
        self.assertTrue(text.isascii())
        self.assertLess(len(text.encode("utf-8")), 700_000)

    def test_crlf_source_uses_the_same_digest(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "source.json"
            path.write_bytes(format_source(self.source).replace("\n", "\r\n").encode("utf-8"))
            self.assertEqual(expand_source(read_source(path)), self.expanded)
        self.assertEqual(canonical_sha256({"b": 2, "a": "\u00e9"}),
                         canonical_sha256({"a": "\u00e9", "b": 2}))

    def test_expansion_does_not_alias_shared_values(self):
        source = self.source_copy()
        expanded = expand_source(source)
        expanded["observations"][0]["qualifiers"]["edition"].append("changed")
        profile = source["observations"][0]["qualifiers"]
        self.assertNotIn("changed", source["qualifier_sets"][profile]["edition"])
        expanded["evidence"][0]["pipeline_mode"].append("changed")
        self.assertNotIn("changed", expanded["evidence"][1]["pipeline_mode"])
        self.assertEqual(source, self.source)

    def test_null_omits_a_factor_but_none_is_a_real_level(self):
        source = self.source_copy()
        row = source["observations"][0]
        row["levels"]["CO"] = None
        row["levels"]["FM"] = "none"
        expanded = expand_source(source)["observations"][0]
        self.assertNotIn("CO", expanded["levels"])
        self.assertEqual(expanded["missing_factors"], ["CO"])
        self.assertEqual(expanded["levels"]["FM"], "none")
        row["levels"]["FM"] = None
        expanded = expand_source(source)["observations"][0]
        self.assertEqual(expanded["missing_factors"], ["CO", "FM"])

    def test_sparse_axes_require_a_basis(self):
        source = self.source_copy()
        source["observations"][0].pop("sparse_basis")
        with self.assertRaisesRegex(ValueError, "sparse factor vector"):
            expand_source(source)

    def test_full_axes_reject_stale_sparse_metadata(self):
        source = self.source_copy()
        source["observations"][0]["levels"]["CO"] = "assignment_rhs"
        with self.assertRaisesRegex(ValueError, "full factor vector"):
            expand_source(source)
        source["observations"][0].pop("sparse_basis")
        expanded = expand_source(source)["observations"][0]
        self.assertNotIn("missing_factors", expanded)
        self.assertEqual(expanded["levels"]["CO"], "assignment_rhs")

    def test_missing_factor_lists_are_not_stored(self):
        source = self.source_copy()
        source["observations"][0]["missing_factors"] = ["CO"]
        with self.assertRaisesRegex(ValueError, "missing_factors is derived"):
            expand_source(source)

    def test_unknown_levels_and_factors_are_rejected(self):
        for key, value in (("UNKNOWN", "none"), ("TY", "not_a_type"), ("LV", []), ("PC", False)):
            with self.subTest(key=key, value=value):
                source = self.source_copy()
                source["observations"][0]["levels"][key] = value
                with self.assertRaisesRegex(ValueError, "unknown"):
                    expand_source(source)

    def test_level_defaults_are_explicit_and_valid(self):
        source = self.source_copy()
        source["level_defaults"].pop("CO")
        with self.assertRaisesRegex(ValueError, "every factor"):
            expand_source(source)
        source = self.source_copy()
        source["level_defaults"]["OP"] = "invalid"
        with self.assertRaisesRegex(ValueError, "unknown OP"):
            expand_source(source)

    def test_unknown_qualifier_profiles_are_rejected(self):
        for value in ("missing", {}, None):
            with self.subTest(value=value):
                source = self.source_copy()
                source["observations"][0]["qualifiers"] = value
                with self.assertRaisesRegex(ValueError, "unknown qualifier set"):
                    expand_source(source)

    def test_invalid_qualifier_values_are_still_checked(self):
        manifest = deepcopy(self.expanded)
        manifest["observations"][0]["qualifiers"]["width"] = ["invalid"]
        with self.assertRaisesRegex(ValueError, "invalid qualifier"):
            checker.observation_pairs(manifest)

    def test_inherited_evidence_fields_cannot_be_missing_or_identifiers(self):
        for mode in ("missing", "extra"):
            with self.subTest(mode=mode):
                source = self.source_copy()
                if mode == "missing":
                    source["evidence_defaults"].pop("pipeline_mode")
                else:
                    source["evidence_defaults"]["id"] = "W01"
                with self.assertRaisesRegex(ValueError, "five shared"):
                    expand_source(source)

    def test_catalog_and_redundant_provenance_cannot_be_stored(self):
        for key in ("pair_catalog", "core_path_basis", "structural_rule_basis", "outside_rule_basis"):
            with self.subTest(key=key):
                source = self.source_copy()
                source[key] = []
                with self.assertRaisesRegex(ValueError, "unexpected"):
                    expand_source(source)

    def test_invalid_container_types_are_reported(self):
        with self.assertRaisesRegex(ValueError, "must be an object"):
            expand_source([])
        for key in ("evidence", "observations"):
            source = self.source_copy()
            source[key] = [None]
            with self.assertRaisesRegex(ValueError, "array of objects"):
                expand_source(source)
        source = self.source_copy()
        source["observations"][0]["levels"] = []
        with self.assertRaisesRegex(ValueError, "must be an object"):
            expand_source(source)

    def test_format_and_digests_are_required(self):
        source = self.source_copy()
        source["storage_format"] = "unknown"
        with self.assertRaisesRegex(ValueError, "storage_format"):
            expand_source(source)
        for key in ("catalog_sha256", "rule_basis_sha256"):
            source = self.source_copy()
            source[key] = "invalid"
            with self.assertRaisesRegex(ValueError, "SHA-256"):
                expand_source(source)
        source = self.source_copy()
        source.pop("catalog_sha256")
        with self.assertRaisesRegex(ValueError, "missing="):
            expand_source(source)

    def test_duplicate_json_keys_and_non_json_constants_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "source.json"
            for text in ('{"a": 1, "a": 2}', '{"a": {"id": 1, "id": 2}}'):
                path.write_text(text, encoding="utf-8")
                with self.assertRaisesRegex(ValueError, "duplicate JSON key"):
                    read_source(path)
            for value in ("NaN", "Infinity", "-Infinity"):
                path.write_text('{"a": ' + value + '}', encoding="utf-8")
                with self.assertRaisesRegex(ValueError, "non-JSON"):
                    read_source(path)

    def test_duplicate_evidence_and_observation_ids_are_still_rejected(self):
        for field, message in (("evidence", "duplicate evidence"), ("observations", "duplicate observation")):
            manifest = deepcopy(self.expanded)
            manifest[field].insert(0, deepcopy(manifest[field][0]))
            with self.assertRaisesRegex(ValueError, message):
                checker.observation_pairs(manifest)

    def test_missing_source_anchors_are_still_rejected(self):
        manifest = deepcopy(self.expanded)
        manifest["observations"][0]["source_anchors"] = ["not in this HDL fixture"]
        with self.assertRaisesRegex(ValueError, "source anchors"):
            checker.observation_pairs(manifest)

    def test_missing_oracle_lines_are_still_rejected(self):
        manifest = deepcopy(self.expanded)
        manifest["evidence"][0]["stdout"] = "not the independent oracle\n"
        with self.assertRaisesRegex(ValueError, "exact stdout"):
            checker.validate_manifest(manifest, self.catalog)

    def test_rule_and_catalog_fingerprints_are_enforced(self):
        for key, message in (("rule_basis_sha256", "rule provenance"), ("catalog_sha256", "derived pair catalog")):
            manifest = deepcopy(self.expanded)
            manifest[key] = "0" * 64
            with self.assertRaisesRegex(ValueError, message):
                checker.validate_manifest(manifest, self.catalog)

    def test_same_counts_cannot_hide_changed_observation_links(self):
        catalog = deepcopy(self.catalog)
        row = next(item for item in catalog if item.get("observation_ids"))
        row["observation_ids"][0] = "O-changed"
        self.assertEqual(len(catalog), len(self.catalog))
        with self.assertRaisesRegex(ValueError, "derived pair catalog"):
            checker.validate_manifest(self.expanded, catalog)

    def test_zero_gap_policy_cannot_be_disabled(self):
        manifest = deepcopy(self.expanded)
        manifest["required_zero_legal_gaps"] = False
        with self.assertRaisesRegex(ValueError, "zero selected legal gaps"):
            checker.validate_manifest(manifest, self.catalog)

    def test_cli_emit_cells_reconstructs_the_catalog_without_writing(self):
        result, before, after = self.run_checker(self.source, "--emit-cells")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stderr, "")
        self.assertEqual(before, after)
        lines = result.stdout.splitlines()
        expected = []
        for row in self.catalog:
            detail = row.get("gap_id") or ",".join(row.get("observation_ids", row.get("reason_rule_ids", [])))
            expected.append(f"{row['id']}\t{row['applicability']}\t{row.get('evidence_status', '')}\t{detail}")
        self.assertEqual(lines[:-1], expected)
        self.assertIn("0 legal gaps", lines[-1])

    def test_cli_refresh_only_updates_baselines(self):
        source = self.source_copy()
        source["catalog_sha256"] = "0" * 64
        source["rule_basis_sha256"] = "0" * 64
        source["baseline_counts"] = {}
        result, _, after = self.run_checker(source, "--refresh-baseline")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(after), self.source)
        self.assertNotIn("pair_catalog", json.loads(after))

    def test_cli_refresh_cannot_bless_gaps_or_modify_a_failed_source(self):
        source = self.source_copy()
        source["observations"] = []
        result, before, after = self.run_checker(source, "--refresh-baseline")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("1849 selected legal gaps remain", result.stderr)
        self.assertEqual(before, after)

    def test_cli_refresh_cannot_bless_a_broken_oracle(self):
        source = self.source_copy()
        source["evidence"][0]["stdout"] = "not the independent oracle\n"
        result, before, after = self.run_checker(source, "--refresh-baseline")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("exact stdout", result.stderr)
        self.assertEqual(before, after)

    def test_cli_missing_source_fails_without_fallback(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "missing.json"
            result = subprocess.run(
                [sys.executable, str(Path(checker.__file__)), "--manifest", str(path)],
                capture_output=True, text=True, timeout=120, cwd=directory,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("SYN-038 pairwise manifest:", result.stderr)
            self.assertEqual(list(Path(directory).iterdir()), [])


if __name__ == "__main__":
    unittest.main()
