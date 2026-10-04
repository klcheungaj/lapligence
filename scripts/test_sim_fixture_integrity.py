"""Unit tests for the static fixture-index gate (no compiler required)."""
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from check_sim_fixture_integrity import Reference, feature_completion_errors, missing, source_references


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

    def test_explicit_verilog_extension_and_output_oracle(self) -> None:
        source = 'sim_cli::run_case("feature_completion/rtl_001", "copy.v", "ok", "", &[]);'
        source += 'include_str!("../fixtures/sim/feature_completion/rtl_001/copy.out");'
        refs = source_references(source, "tests/sim_feature_completion/rtl_001.rs")
        self.assertEqual({ref.path for ref in refs}, {
            "tests/fixtures/sim/feature_completion/rtl_001/copy.v",
            "tests/fixtures/sim/feature_completion/rtl_001/copy.out",
        })

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

    def test_undefined_behavior_fixture_macro(self) -> None:
        source = 'p.join("tests/fixtures/sim/undefined_behavior");\n'
        source += 'fixture!(q02_short_hex, "v");\nfixture!(q03_control, "sv");'
        self.assertEqual(self.paths(source), {
            "tests/fixtures/sim/undefined_behavior/q02_short_hex.v",
            "tests/fixtures/sim/undefined_behavior/q03_control.sv",
        })

    def test_companion_inputs_are_references(self) -> None:
        source = ('const SUITE: &str = "feature_completion/rtl_018";\n'
                  'const MAP: &str = "root.map";\n'
                  'sim_cli::run_case_with_inputs(SUITE, "top", &["lib/rtl.sv", MAP], "ok", "", &[], &[]);\n'
                  'sim_cli::reject_case_with_inputs(SUITE, "bad.v", &["cfg.sv"], "error", &[]);')
        self.assertEqual(self.paths(source), {
            "tests/fixtures/sim/feature_completion/rtl_018/top.sv",
            "tests/fixtures/sim/feature_completion/rtl_018/lib/rtl.sv",
            "tests/fixtures/sim/feature_completion/rtl_018/root.map",
            "tests/fixtures/sim/feature_completion/rtl_018/bad.v",
            "tests/fixtures/sim/feature_completion/rtl_018/cfg.sv",
        })

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


class FeatureCompletionTests(unittest.TestCase):
    def setUp(self) -> None:
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.facade = self.root / "tests/sim_feature_completion.rs"
        self.module = self.root / "tests/sim_feature_completion/rtl_001.rs"
        self.fixture = self.root / "tests/fixtures/sim/feature_completion/rtl_001/copy.v"
        self.module.parent.mkdir(parents=True)
        self.fixture.parent.mkdir(parents=True)
        self.facade.write_text('#[path = "sim_feature_completion/rtl_001.rs"] mod rtl_001;')
        self.fixture.write_text("module tb; endmodule\n", encoding="ascii")
        self.active = ('const SUITE: &str = "feature_completion/rtl_001";\n'
                       '#[test]\nfn copy() {\n'
                       'sim_cli::run_case(SUITE, "copy.v", "ok\\n", "", &[]);\n}\n')
        self.module.write_text(self.active)

    def errors(self) -> list[str]:
        return feature_completion_errors(self.root)

    def test_declared_fixture_passes(self) -> None:
        self.assertEqual(self.errors(), [])

    def test_missing_suite_and_module_fail(self) -> None:
        self.module.unlink()
        self.assertTrue(self.errors())
        self.facade.unlink()
        self.assertIn("missing declared feature suite", self.errors()[0])

    def test_missing_declaration_and_directory_fail(self) -> None:
        self.facade.write_text("")
        self.assertTrue(self.errors())
        self.facade.write_text('#[path = "sim_feature_completion/rtl_001.rs"] mod rtl_001;')
        self.fixture.unlink()
        self.fixture.parent.rmdir()
        self.assertIn("has no fixture directory", self.errors()[0])

    def test_companion_input_counts_as_feature_reference(self) -> None:
        library = self.fixture.with_name("library.sv")
        library.write_text("module leaf; endmodule\n", encoding="ascii")
        self.assertIn("not referenced by a declared feature test", self.errors()[0])
        self.module.write_text(self.active.replace(
            'sim_cli::run_case(SUITE, "copy.v", "ok\\n", "", &[]);',
            'sim_cli::run_case_with_inputs(SUITE, "copy.v", &["library.sv"], "ok\\n", "", &[], &[]);'))
        self.assertEqual(self.errors(), [])

    def test_missing_and_unreferenced_inputs_fail(self) -> None:
        self.fixture.unlink()
        refs = source_references(self.active, "tests/sim_feature_completion/rtl_001.rs")
        self.assertTrue(missing(self.root, refs))
        self.assertIn("zero HDL fixtures", self.errors()[0])
        self.fixture.write_text("module tb; endmodule\n")
        self.fixture.with_name("unused.sv").write_text("module tb; endmodule\n")
        self.assertIn("not referenced by a declared feature test", self.errors()[0])

    def test_ignored_and_conditional_tests_fail(self) -> None:
        for attribute in ['#[ignore]', '#[cfg(unix)]', '#[cfg_attr(unix, ignore)]']:
            with self.subTest(attribute=attribute):
                self.module.write_text(self.active.replace('#[test]', attribute + '\n#[test]'))
                self.assertIn("must not be disabled", self.errors()[0])
        self.facade.write_text('#[cfg(unix)]\n#[path = "sim_feature_completion/rtl_001.rs"] mod rtl_001;')
        self.assertTrue(self.errors())

    def test_zero_tests_and_helper_only_references_fail(self) -> None:
        self.module.write_text(self.active.replace('#[test]', ''))
        self.assertIn("zero declared feature tests", self.errors()[0])
        self.module.write_text(self.active.replace('#[test]', '') + '#[test]\nfn unrelated() {}\n')
        self.assertIn("not referenced by a declared feature test", self.errors()[0])

    def test_component_or_owned_helper_alone_cannot_accept_a_feature(self) -> None:
        self.module.write_text(self.active.replace("run_case(", "run_case_after_db_drop("))
        self.assertIn("through public sim_cli", self.errors()[0])
        self.module.write_text('#[test]\nfn component() {\n'
                               'p.join("tests/fixtures/sim/feature_completion/rtl_001/copy.v");\n}\n')
        self.assertIn("through public sim_cli", self.errors()[0])

    def test_commented_test_and_string_with_braces_do_not_hide_failures(self) -> None:
        self.module.write_text('/*' + self.active + '*/')
        self.assertTrue(self.errors())
        self.module.write_text(self.active.replace('"ok\\n"', '"} {\\n"'))
        self.assertEqual(self.errors(), [])

    def test_untracked_suite_or_module_fail(self) -> None:
        errors = feature_completion_errors(self.root, {self.fixture.relative_to(self.root).as_posix()})
        self.assertEqual(len(errors), 2)
        self.assertTrue(all("untracked" in error for error in errors))

    def test_command_fails_for_missing_disabled_and_zero_tests(self) -> None:
        checker = Path(__file__).with_name("check_sim_fixture_integrity.py")
        for source in [self.active, self.active.replace('#[test]', '#[ignore]\n#[test]'),
                       self.active.replace('#[test]', '')]:
            with self.subTest(source=source):
                self.module.write_text(source)
                result = subprocess.run([sys.executable, str(checker), "--root", str(self.root)],
                                        capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, 0 if source == self.active else 1,
                                 result.stdout + result.stderr)
        self.module.write_text(self.active)
        self.fixture.unlink()
        result = subprocess.run([sys.executable, str(checker), "--root", str(self.root)],
                                capture_output=True, text=True, check=False)
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)

    def test_legacy_directories_keep_their_existing_owners(self) -> None:
        self.facade.unlink()
        self.fixture.unlink()
        self.fixture.parent.rmdir()
        (self.fixture.parent.parent / "g1_12").mkdir()
        self.assertEqual(self.errors(), [])


if __name__ == "__main__":
    unittest.main()
