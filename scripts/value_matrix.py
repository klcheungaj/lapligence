#!/usr/bin/env python3
import argparse
import os
import re
import shutil
import subprocess
import sys
import time
import xml.etree.ElementTree as ElementTree
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
PROFILE = "value-matrix"
CELL_ENV = "LLG_TEST_MATRIX_CELL"
LANES = {
    "legacy": ("legacy", "portable"),
    "compact-portable": ("compact", "portable"),
    "compact-gmp": ("compact", "gmp"),
}
MODES = ("normal", "no-opt")
DEFAULT_LANES = ("legacy", "compact-gmp")
PRODUCTION_CELL = ("compact-gmp", "normal")
WITNESS_BINARY = "llg::sim_n_z"
WITNESS_TEST = "sim_value_backends::matrix::matrix_cell_selection_reaches_the_built_model"
FORBIDDEN_ENV = ("GMP_ROOT", "LLG_TEST_GMP_ROOT", "LLG_DEV_VALUE_BACKEND", "LLG_DEV_COMPACT_KERNELS", CELL_ENV)


def split_list(text, allowed, what):
    names = [name for name in text.split(",") if name]
    unknown = [name for name in names if name not in allowed]
    if unknown or not names or len(set(names)) != len(names):
        raise SystemExit(f"value_matrix: invalid {what} {text!r}; choose from {', '.join(allowed)}")
    return names


def cells(lanes, modes):
    ordered = [(lane, mode) for lane in lanes for mode in modes]
    ordered.sort(key=lambda cell: cell != PRODUCTION_CELL)
    return ordered


def cell_name(lane, mode):
    return f"{lane}.{mode}"


def scope_filter(lane, mode, extra):
    whole = (lane, mode) == PRODUCTION_CELL
    expression = "all()" if whole else "not default()"
    if extra:
        expression = f"({expression}) & ({extra})"
    return ("all" if whole else "backend-dependent"), f"({expression}) | test(={WITNESS_TEST})"


def cell_env(base, lane, mode):
    backend, kernel = LANES[lane]
    env = dict(base)
    env["LLG_DEV_VALUE_BACKEND"] = backend
    env["LLG_DEV_COMPACT_KERNELS"] = kernel
    env[CELL_ENV] = f"{backend}/{kernel}/{mode}"
    return env


def junit_candidates(env):
    roots = [REPO_ROOT / "target"]
    if env.get("CARGO_TARGET_DIR"):
        roots.insert(0, Path(env["CARGO_TARGET_DIR"]))
    return [root / "nextest" / PROFILE / "junit.xml" for root in roots]


def outcome(testcase):
    tags = {child.tag for child in testcase}
    if tags & {"failure", "error"}:
        return "failed"
    if "skipped" in tags:
        return "skipped"
    if tags & {"flakyFailure", "flakyError", "rerunFailure", "rerunError"}:
        return "flaky"
    return "passed"


def read_junit(path):
    root = ElementTree.parse(path).getroot()
    suites = [root] if root.tag == "testsuite" else root.findall("testsuite")
    results = []
    for suite in suites:
        binary = suite.get("name", "")
        for testcase in suite.findall("testcase"):
            results.append((binary, testcase.get("name", ""), outcome(testcase)))
    return results


def tally(results, key):
    counts = {}
    for result in results:
        row = counts.setdefault(key(result), {"passed": 0, "failed": 0, "skipped": 0, "flaky": 0})
        row[result[2]] += 1
    return counts


def nextest_skipped(log_path):
    found = re.findall(r"Summary \[.*\] \d+ tests? run: .*?(\d+) skipped", log_path.read_text(encoding="utf-8", errors="replace"))
    return found[-1] if found else "0"


def git_text(*args):
    completed = subprocess.run(["git", *args], cwd=REPO_ROOT, capture_output=True, text=True)
    return completed.stdout.strip() if completed.returncode == 0 else "unknown"


def run_cell(lane, mode, args, base_env, output_dir):
    name = cell_name(lane, mode)
    scope, expression = scope_filter(lane, mode, args.filter)
    env = cell_env(base_env, lane, mode)
    junits = junit_candidates(env)
    for junit in junits:
        if junit.exists():
            junit.unlink()
    command = [args.runner, *args.runner_args, "--profile", PROFILE, "--no-fail-fast", "--ignore-default-filter", "-E", expression]
    log_path = output_dir / f"{name}.log"
    print(f"value_matrix: {name}: {env[CELL_ENV]} ({scope})", flush=True)
    start = time.monotonic()
    with log_path.open("w", encoding="utf-8") as log:
        log.write(f"$ {CELL_ENV}={env[CELL_ENV]} {' '.join(command)}\n")
        log.flush()
        status = subprocess.run(command, cwd=REPO_ROOT, env=env, stdout=log, stderr=subprocess.STDOUT).returncode
    seconds = time.monotonic() - start
    junit = next((path for path in junits if path.exists()), None)
    problems = []
    results = []
    if junit is None:
        problems.append("no JUnit report: the cell did not run")
    else:
        shutil.copyfile(junit, output_dir / f"{name}.junit.xml")
        results = read_junit(junit)
    counts = tally(results, lambda result: "all").get("all", {"passed": 0, "failed": 0, "skipped": 0, "flaky": 0})
    if status != 0:
        problems.append(f"runner exited with {status}")
    if junit is not None and not results:
        problems.append("no test ran")
    if counts["failed"] or counts["flaky"]:
        problems.append(f"{counts['failed']} failed, {counts['flaky']} flaky")
    witness = [result[2] for result in results if result[:2] == (WITNESS_BINARY, WITNESS_TEST)]
    if witness != ["passed"]:
        problems.append(f"backend witness {WITNESS_TEST}: {witness or 'not run'}")
    with (output_dir / f"{name}.groups.tsv").open("w", encoding="utf-8") as groups:
        groups.write("binary\tsuite\tpassed\tfailed\tskipped\tflaky\n")
        by_suite = tally(results, lambda result: (result[0], result[1].split("::", 1)[0]))
        for (binary, suite), row in sorted(by_suite.items()):
            groups.write(f"{binary}\t{suite}\t{row['passed']}\t{row['failed']}\t{row['skipped']}\t{row['flaky']}\n")
    failed = sorted(f"{result[0]} {result[1]}" for result in results if result[2] in ("failed", "flaky"))
    (output_dir / f"{name}.failed.txt").write_text("".join(f"{line}\n" for line in failed), encoding="utf-8")
    return {
        "cell": name,
        "selection": env[CELL_ENV],
        "scope": scope,
        "status": "ok" if not problems else "FAILED",
        "exit": status,
        "tests": len(results),
        "seconds": f"{seconds:.0f}",
        "nextest_skipped": nextest_skipped(log_path),
        "problems": "; ".join(problems),
        **counts,
    }


COLUMNS = ("cell", "selection", "scope", "status", "exit", "tests", "passed", "failed", "skipped", "flaky", "nextest_skipped", "seconds", "problems")


def main(argv=None):
    parser = argparse.ArgumentParser(
        description="Run the FIN-002 value/optimizer matrix: every requested value lane in "
        "normal and --no-opt HDL modes on the current revision, one nextest run per cell.",
        epilog="Arguments after -- go to the runner, for example "
        "-- --test-work-dir /build --cargo-profile quick.",
    )
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--lanes", default=",".join(DEFAULT_LANES), help=f"comma list of {', '.join(LANES)}")
    parser.add_argument("--modes", default=",".join(MODES), help="comma list of normal, no-opt")
    parser.add_argument("--runner", default=str(REPO_ROOT / "scripts" / "run-tests.sh"))
    parser.add_argument("--filter", default="", help="nextest filterset that narrows every cell")
    parser.add_argument("runner_args", nargs=argparse.REMAINDER)
    args = parser.parse_args(argv)
    if args.runner_args[:1] == ["--"]:
        args.runner_args = args.runner_args[1:]
    lanes = split_list(args.lanes, LANES, "lanes")
    modes = split_list(args.modes, MODES, "modes")
    present = [name for name in FORBIDDEN_ENV if os.environ.get(name)]
    if present:
        print(
            f"value_matrix: unset {', '.join(present)}: each cell sets its own value selection "
            "and qualifies the bundled GMP",
            file=sys.stderr,
        )
        return 2
    output_dir = args.output_dir.resolve()
    output_dir.mkdir(parents=True, exist_ok=True)
    revision = git_text("rev-parse", "HEAD")
    dirty = git_text("status", "--porcelain", "--untracked-files=no", "--", ".", ":(exclude)vendor")
    with (output_dir / "metadata.tsv").open("w", encoding="utf-8") as metadata:
        metadata.write(f"revision\t{revision}\n")
        metadata.write(f"tracked_changes_outside_vendor\t{'yes' if dirty else 'no'}\n")
        metadata.write(f"lanes\t{','.join(lanes)}\nmodes\t{','.join(modes)}\n")
        metadata.write(f"filter\t{args.filter}\nrunner\t{args.runner} {' '.join(args.runner_args)}\n")
    rows = []
    with (output_dir / "summary.tsv").open("w", encoding="utf-8") as summary:
        summary.write("\t".join(COLUMNS) + "\n")
        for lane, mode in cells(lanes, modes):
            row = run_cell(lane, mode, args, os.environ, output_dir)
            rows.append(row)
            summary.write("\t".join(str(row[column]) for column in COLUMNS) + "\n")
            summary.flush()
            print(f"value_matrix: {row['cell']}: {row['status']} {row['passed']} passed, {row['failed']} failed, "
                  f"{row['skipped']} skipped, {row['flaky']} flaky in {row['seconds']} s {row['problems']}", flush=True)
    bad = [row["cell"] for row in rows if row["status"] != "ok"]
    if bad:
        print(f"value_matrix: failed cells: {', '.join(bad)} (see {output_dir})", file=sys.stderr)
        return 1
    print(f"value_matrix: all {len(rows)} cells passed on {revision} (see {output_dir})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
