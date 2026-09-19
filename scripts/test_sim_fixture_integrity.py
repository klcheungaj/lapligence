"""Unit tests for the static fixture-index gate (no compiler required)."""
from pathlib import Path
import tempfile
import unittest

from check_sim_fixture_integrity import Reference, missing, source_references


class FixtureIntegrityTests(unittest.TestCase):
    def paths(self, source: str) -> set[str]:
        return {r.path for r in source_references(source, "tests/sample.rs")}

    def test_shared_cli_literal(self) -> None:
        self.assertEqual(self.paths('sim_cli::run_case("gates", "matrix", "ok", "", &[]);'),
                         {"tests/fixtures/sim/gates/matrix.sv"})

    def test_shared_cli_suite_constant(self) -> None:
        self.assertEqual(self.paths('const SUITE: &str = "group1_repairs";\n'
                                    'sim_cli::invoke_with_env(SUITE, "member", true, &[], &[], &[]);'),
                         {"tests/fixtures/sim/group1_repairs/member.sv"})

    def test_fixture_helper_and_datatype_macro(self) -> None:
        source = 'fn root() { p.join("tests/fixtures/sim/data_types_next"); }\n'
        source += 'run_fixture("union.sv", "union");\n'
        source += 'datatype_case!(members, "members.sv", "members");'
        self.assertEqual(self.paths(source), {
            "tests/fixtures/sim/data_types_next/union.sv",
            "tests/fixtures/sim/data_types_next/members.sv",
        })

    def test_explicit_path(self) -> None:
        self.assertEqual(self.paths('p.join("tests/fixtures/sim/net_resolution/example.sv")'),
                         {"tests/fixtures/sim/net_resolution/example.sv"})

    def test_comments_are_not_discovered_and_lines_are_preserved(self) -> None:
        source = '// sim_cli::run_case("absent", "one", "", "", &[]);\n'
        source += '/* nested /* comment */ sim_cli::run_case("absent", "two", "", "", &[]); */\n'
        source += 'sim_cli::run_case("actual", "three", "", "", &[]);'
        refs = source_references(source, "tests/sample.rs")
        self.assertEqual(refs, {Reference("tests/fixtures/sim/actual/three.sv", "tests/sample.rs", 3)})

    def test_missing_file_is_an_error(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            refs = [Reference("tests/fixtures/sim/a.sv", "tests/sample.rs", 3)]
            self.assertEqual(len(missing(root, refs)), 1)
            target = root / refs[0].path
            target.parent.mkdir(parents=True)
            target.write_text("module tb; endmodule\n", encoding="ascii")
            self.assertEqual(missing(root, refs), [])

    def test_untracked_file_cannot_mask_an_incomplete_patch(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "case.sv"
            target.write_text("module tb; endmodule\n", encoding="ascii")
            refs = [Reference("case.sv", "tests/sample.rs", 1)]
            self.assertIn("untracked case.sv", missing(root, refs, set())[0])
            self.assertEqual(missing(root, refs, {"case.sv"}), [])

    def test_path_escape_is_rejected(self) -> None:
        with self.assertRaises(ValueError):
            self.paths('sim_cli::run_case("../outside", "case", "", "", &[]);')


if __name__ == "__main__":
    unittest.main()
