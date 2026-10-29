import json
import sys
import unittest
from unittest import mock
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import ci_dependency_report as report


class VersionTest(unittest.TestCase):
    def test_parse_version_accepts_releases_only(self):
        self.assertEqual(report.parse_version("v1.3.2"), (1, 3, 2))
        self.assertEqual(report.parse_version("12.0"), (12, 0))
        for name in ("v12.0rc1", "1.0.0-beta.1", "gmp-6.1.0", "nextest", "rel-5.1.0"):
            self.assertIsNone(report.parse_version(name))

    def test_latest_version_compares_numerically(self):
        self.assertEqual(report.latest_version(["v1.9", "v1.10", "v1.10rc1", "main"]), "v1.10")
        self.assertIsNone(report.latest_version(["nightly"]))

    def test_semver_compatible_follows_cargo_caret_rules(self):
        self.assertTrue(report.semver_compatible((1, 2, 60), (1, 10, 0)))
        self.assertFalse(report.semver_compatible((1, 2, 0), (2, 0, 0)))
        self.assertTrue(report.semver_compatible((0, 8, 1), (0, 8, 23)))
        self.assertFalse(report.semver_compatible((0, 8, 23), (0, 9, 0)))
        self.assertFalse(report.semver_compatible((0, 0, 3), (0, 0, 4)))


class CargoTest(unittest.TestCase):
    def test_direct_dependencies_follow_renames_and_skip_local_sources(self):
        manifest = {
            "dependencies": {"serde": "1", "alias": {"package": "real", "version": "2"},
                             "local": {"path": "crates/local"}, "forked": {"git": "https://example.invalid/x"}},
            "dev-dependencies": {"serde": "1"},
            "target": {"cfg(unix)": {"dependencies": {"libc": "0.2"}}},
        }
        self.assertEqual(report.direct_dependencies(manifest), {
            "serde": ["dependencies", "dev-dependencies"],
            "real": ["dependencies"],
            "libc": ["dependencies (cfg(unix))"],
        })

    def test_locked_versions_keep_highest_registry_version(self):
        registry = "registry+https://github.com/rust-lang/crates.io-index"
        lockfile = {"package": [
            {"name": "toml", "version": "0.5.11", "source": registry},
            {"name": "toml", "version": "0.8.23", "source": registry},
            {"name": "lapligence", "version": "0.1.0"},
        ]}
        self.assertEqual(report.locked_versions(lockfile), {"toml": (0, 8, 23)})

    def test_index_path_follows_sparse_index_layout(self):
        self.assertEqual(report.index_path("cc"), "2/cc")
        self.assertEqual(report.index_path("a"), "1/a")
        self.assertEqual(report.index_path("Toml"), "to/ml/toml")
        self.assertEqual(report.index_path("syn"), "3/s/syn")
        self.assertEqual(report.index_path("serde_json"), "se/rd/serde_json")

    def test_latest_index_version_skips_yanked_and_prereleases(self):
        lines = [{"vers": "1.0.0", "yanked": False}, {"vers": "1.2.0", "yanked": True},
                 {"vers": "1.1.0", "yanked": False}, {"vers": "2.0.0-alpha.1", "yanked": False}]
        text = "\n".join(json.dumps(line) for line in lines) + "\n"
        self.assertEqual(report.latest_index_version(text), "1.1.0")

    def test_outdated_crates_choose_update_or_bump(self):
        direct = {"cc": ["build-dependencies"], "toml": ["dependencies"], "serde": ["dependencies"]}
        locked = {"cc": (1, 2, 60), "toml": (0, 8, 23), "serde": (1, 0, 229)}
        latest = {"cc": "1.10.0", "toml": "0.9.8", "serde": "1.0.229"}
        self.assertEqual(report.outdated_crates(direct, locked, latest), [
            ("cc", "1.2.60", "1.10.0", "`cargo update`", "build-dependencies"),
            ("toml", "0.8.23", "0.9.8", "bump Cargo.toml", "dependencies"),
        ])


class AuditTest(unittest.TestCase):
    def test_audit_findings_split_vulnerabilities_and_warnings(self):
        audit = {
            "vulnerabilities": {"found": True, "count": 1, "list": [{
                "advisory": {"id": "RUSTSEC-2099-0001", "title": "Overflow"},
                "package": {"name": "foo", "version": "1.0.0"},
                "versions": {"patched": [">=1.0.1"]},
            }]},
            "warnings": {
                "yanked": [{"kind": "yanked", "advisory": None, "package": {"name": "bar", "version": "0.3.1"}}],
                "unmaintained": [{"kind": "unmaintained", "advisory": {"id": "RUSTSEC-2099-0002", "title": "Gone"},
                                  "package": {"name": "baz", "version": "2.0.0"}}],
            },
        }
        vulnerabilities, warnings = report.audit_findings(audit)
        self.assertEqual(vulnerabilities, [("RUSTSEC-2099-0001", "foo", "1.0.0", "Overflow", ">=1.0.1")])
        self.assertEqual(warnings, [
            ("unmaintained", "RUSTSEC-2099-0002", "baz", "2.0.0", "Gone"),
            ("yanked", "-", "bar", "0.3.1", ""),
        ])


class RemoteTest(unittest.TestCase):
    LS_REMOTE = "\n".join([
        "1111111111111111111111111111111111111111\trefs/tags/v11.0",
        "7ddf4059f79eff508dd486eb42fd650cdf320d52\trefs/tags/v11.0^{}",
        "2222222222222222222222222222222222222222\trefs/tags/v12.0rc1",
        "3333333333333333333333333333333333333333\trefs/tags/v12.1",
        "4444444444444444444444444444444444444444\trefs/heads/master",
    ])

    def test_parse_ls_remote_peels_annotated_tags(self):
        tags = report.parse_ls_remote(self.LS_REMOTE)
        self.assertEqual(tags["v11.0"], "7ddf4059f79eff508dd486eb42fd650cdf320d52")
        self.assertEqual(set(tags), {"v11.0", "v12.0rc1", "v12.1"})

    def test_submodule_status_names_pinned_tag(self):
        tags = report.parse_ls_remote(self.LS_REMOTE)
        self.assertEqual(report.submodule_status("7ddf4059f79eff508dd486eb42fd650cdf320d52", tags),
                         ("v11.0", "v12.1", True))
        self.assertEqual(report.submodule_status("3333333333333333333333333333333333333333", tags),
                         ("v12.1", "v12.1", False))
        self.assertEqual(report.submodule_status("5555555555555555555555555555555555555555", tags),
                         ("555555555555", "v12.1", True))

    def test_action_references_keep_versioned_remote_actions(self):
        workflow = "\n".join([
            "      - uses: actions/checkout@v7",
            "        uses: actions/cache/restore@v6",
            "      - uses: dtolnay/rust-toolchain@1.98.0",
            "      - uses: taiki-e/install-action@nextest",
            "        uses: ./.github/actions/ccache-setup",
            "      - uses: 'actions/upload-artifact@v4' # quoted",
        ])
        self.assertEqual(report.action_references([workflow]), {
            "actions/checkout": {"v7"},
            "actions/cache": {"v6"},
            "actions/upload-artifact": {"v4"},
        })

    def test_outdated_actions_compare_major_versions(self):
        references = {"actions/checkout": {"v7"}, "actions/upload-artifact": {"v4"}, "actions/cache": {"v6"}}
        latest = {"actions/checkout": "v7.0.1", "actions/upload-artifact": "v7.0.2"}
        self.assertEqual(report.outdated_actions(references, latest),
                         [("actions/upload-artifact", "v4", "v7.0.2")])


class RenderTest(unittest.TestCase):
    def test_render_report_lists_rows_errors_and_empty_sections(self):
        sections = [("Crates", "note", ("Crate", "Title"), [("foo", "a|b")]), ("Actions", "unused", ("Action",), [])]
        text = report.render_report(sections, ["action x: timeout"], "https://example.invalid/run/1")
        self.assertIn("Generated by https://example.invalid/run/1.", text)
        self.assertIn("| foo | a\\|b |", text)
        self.assertIn("## Actions\n\nNothing to report.", text)
        self.assertNotIn("unused", text)
        self.assertIn("- action x: timeout", text)


class TomlParserTest(unittest.TestCase):
    def test_module_imports_without_tomllib_and_main_names_the_requirement(self):
        with mock.patch.object(report, "tomllib", None):
            with self.assertRaises(SystemExit) as raised:
                report.main(["--audit-json", "audit.json", "--output", "report.md"])
        self.assertIn("tomllib", str(raised.exception))

    def test_toml_parser_reads_cargo_tables_when_available(self):
        if report.tomllib is None:
            self.skipTest("neither tomllib nor tomli is installed")
        manifest = report.tomllib.loads('[dependencies]\nserde = { version = "1", features = ["derive"] }\n')
        self.assertEqual(report.direct_dependencies(manifest), {"serde": ["dependencies"]})


if __name__ == "__main__":
    unittest.main()
