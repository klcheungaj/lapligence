import contextlib
import importlib.util
import io
import os
import tempfile
import time
import unittest
from pathlib import Path

SPEC = importlib.util.spec_from_file_location(
    "prune_target", Path(__file__).resolve().parent / "prune-target.py")
prune_target = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(prune_target)

DAY = 86400


class PruneTargetTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix="llg-prune-tests-")
        self.addCleanup(directory.cleanup)
        self.target = Path(directory.name) / "target"
        self.now = time.time()

    def unit(self, profile, package, unit_hash, kind, outputs, age_days):
        """Create one fingerprinted unit with its outputs, all `age_days` old."""
        base = self.target / profile
        stamp = self.now - age_days * DAY
        paths = [base / ".fingerprint" / f"{package}-{unit_hash}"]
        paths[0].mkdir(parents=True)
        (paths[0] / kind).write_text("fingerprint")
        (paths[0] / f"{kind}.json").write_text("{}")
        for output in outputs:
            path = base / output.format(hash=unit_hash)
            path.parent.mkdir(parents=True, exist_ok=True)
            if output.endswith("/"):
                path.mkdir(exist_ok=True)
                (path / "out").write_text("build script output")
            else:
                path.write_text("artifact")
            paths.append(path)
        for path in paths:
            for root, dirs, files in os.walk(path) if path.is_dir() else [(path.parent, [], [path.name])]:
                for name in dirs + files:
                    os.utime(os.path.join(root, name), (stamp, stamp))
            os.utime(path, (stamp, stamp))
        return paths

    def prune(self, *args):
        with contextlib.redirect_stdout(io.StringIO()) as output, \
                contextlib.redirect_stderr(io.StringIO()):
            status = prune_target.main([
                "--target-dir", str(self.target),
                "--manifest-path", str(self.target / "missing" / "Cargo.toml"), *args,
            ])
        self.assertEqual(status, 0)
        return output.getvalue()

    def test_old_superseded_units_are_removed_and_newest_kept(self):
        old = self.unit("quick", "llg", "a" * 16, "test-integration-test-sim_a_m",
                        ["deps/sim_a_m-{hash}", "deps/sim_a_m-{hash}.d"], age_days=5)
        new = self.unit("quick", "llg", "b" * 16, "test-integration-test-sim_a_m",
                        ["deps/sim_a_m-{hash}"], age_days=4)
        report = self.prune()
        self.assertTrue(all(not path.exists() for path in old), report)
        self.assertTrue(all(path.exists() for path in new))

    def test_recent_superseded_units_survive_the_keep_window(self):
        older = self.unit("quick", "llg", "a" * 16, "lib-llg",
                          ["deps/libllg-{hash}.rlib"], age_days=0.5)
        self.unit("quick", "llg", "b" * 16, "lib-llg", ["deps/libllg-{hash}.rlib"], age_days=0)
        self.prune()
        self.assertTrue(all(path.exists() for path in older))
        self.prune("--keep-days", "0")
        self.assertFalse(older[1].exists())

    def test_different_target_kinds_never_displace_each_other(self):
        script = self.unit("quick", "llg", "c" * 16, "build-script-build-script-build",
                           ["build/llg-{hash}/"], age_days=30)
        run = self.unit("quick", "llg", "d" * 16, "run-build-script-build-script-build",
                        ["build/llg-{hash}/"], age_days=1)
        lib = self.unit("quick", "llg", "e" * 16, "lib-llg", ["deps/libllg-{hash}.rlib"], age_days=0)
        self.prune("--keep-days", "0")
        for path in script + run + lib:
            self.assertTrue(path.exists(), path)

    def test_old_incremental_sessions_of_the_same_crate_are_removed(self):
        incremental = self.target / "quick" / "incremental"
        old, new = incremental / "llg-0abcdefghijkl", incremental / "llg-1abcdefghijkl"
        for path, age in [(old, 9), (new, 0)]:
            (path / "s-session").mkdir(parents=True)
            stamp = self.now - age * DAY
            os.utime(path / "s-session", (stamp, stamp))
            os.utime(path, (stamp, stamp))
        (self.target / "quick" / ".fingerprint").mkdir()
        self.prune()
        self.assertFalse(old.exists())
        self.assertTrue(new.exists())

    def test_dry_run_reports_without_removing(self):
        old = self.unit("release", "serde", "a" * 16, "lib-serde",
                        ["deps/libserde-{hash}.rlib"], age_days=10)
        self.unit("release", "serde", "b" * 16, "lib-serde", ["deps/libserde-{hash}.rlib"], age_days=0)
        report = self.prune("--dry-run")
        self.assertIn("would remove release/serde-" + "a" * 16, report)
        self.assertTrue(all(path.exists() for path in old))

    def test_cross_target_profiles_are_pruned(self):
        old = self.unit("x86_64-unknown-linux-musl/release", "llg", "a" * 16, "bin-llg",
                        ["deps/llg-{hash}"], age_days=10)
        self.unit("x86_64-unknown-linux-musl/release", "llg", "b" * 16, "bin-llg",
                  ["deps/llg-{hash}"], age_days=0)
        self.prune()
        self.assertFalse(old[1].exists())

    def test_units_of_removed_workspace_targets_are_orphans(self):
        targets = {"llg": {("test", "general"), ("bin", "llg"), ("lib", "llg")}}
        orphan = prune_target.is_orphan
        self.assertTrue(orphan("llg", ("test-integration-test-sim_force",), targets))
        self.assertFalse(orphan("llg", ("test-integration-test-general",), targets))
        self.assertTrue(orphan("llg", ("bin-elab_check",), targets))
        self.assertFalse(orphan("llg", ("test-bin-llg",), targets))
        self.assertFalse(orphan("llg", ("lib-llg",), targets))
        self.assertFalse(orphan("llg", ("run-build-script-build-script-build",), targets))
        self.assertFalse(orphan("serde", ("lib-serde",), targets))
        self.assertFalse(orphan("llg", ("test-integration-test-sim_force",), None))

    def test_old_orphaned_units_are_removed_without_a_newer_copy(self):
        targets = {"llg": {("test", "general")}}
        old = self.unit("quick", "llg", "a" * 16, "test-integration-test-sim_force",
                        ["deps/sim_force-{hash}"], age_days=3)
        recent = self.unit("quick", "llg", "b" * 16, "test-integration-test-sim_wait",
                           ["deps/sim_wait-{hash}"], age_days=0)
        live = self.unit("quick", "llg", "c" * 16, "test-integration-test-general",
                         ["deps/general-{hash}"], age_days=9)
        original = prune_target.workspace_targets
        prune_target.workspace_targets = lambda _manifest: targets
        self.addCleanup(setattr, prune_target, "workspace_targets", original)
        self.prune()
        self.assertFalse(any(path.exists() for path in old))
        self.assertTrue(all(path.exists() for path in recent + live))


if __name__ == "__main__":
    unittest.main()
