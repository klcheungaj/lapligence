import argparse
import json
import os
import platform
import re
import math
import sys
from datetime import datetime, timezone
from pathlib import Path

from validation_runner import Runner, find_product, verify_inventory, verify_measurement


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]


def audit_capacity() -> dict:
    runtime = ROOT / "src/sim/rt"
    excluded = {"llg_rt_selftest.c", "llg_wave_selftest.c"}
    obsolete = re.compile(r"\b(?:LLG_MODEL_MAX_WIDTH|LLG_MAX_WIDTH|LLG_LIMBS)\b")
    limit_array = re.compile(r"\[[^\]\n]*\bLLG_SUPPORTED_WIDTH_LIMIT\b")
    files, findings = [], []
    for path in sorted(runtime.rglob("*")):
        if path.suffix not in (".c", ".h") or path.name in excluded or "gtkwave" in path.parts:
            continue
        files.append(str(path.relative_to(ROOT)))
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            if obsolete.search(line) or limit_array.search(line):
                findings.append(f"{path.relative_to(ROOT)}:{number}: {line.strip()}")
    if not files or findings:
        raise ValueError("active-runtime capacity audit failed: " + "; ".join(findings))
    return {"files_scanned": files, "excluded": sorted(excluded) + ["gtkwave (third party)"],
            "limitation": "Lexical check only; not a proof of ownership or arbitrary array bounds."}


def status(records: list[dict]) -> str:
    if any(record["status"] == "failed" for record in records):
        return "failed"
    if any(record["status"] == "blocked" for record in records):
        return "blocked"
    return "passed"


def component(runner: Runner, args, label: str, compiler: str, configuration: str,
              sanitize: bool) -> dict | None:
    build = runner.output / label
    environment = {}
    if sanitize:
        environment = dict(ASAN_OPTIONS="detect_leaks=1:halt_on_error=1",
                           UBSAN_OPTIONS="halt_on_error=1:print_stacktrace=1")
    configure = [args.cmake, "-S", str(HERE), "-B", str(build),
                 f"-DCMAKE_C_COMPILER={compiler}", f"-DCMAKE_BUILD_TYPE={configuration}",
                 "-DBUILD_TESTING=ON", f"-DLLG_STORAGE_TEST_SANITIZERS={'ON' if sanitize else 'OFF'}",
                 f"-DLLG_STORAGE_TEST_ORACLE_LIBRARY={'OFF' if sanitize else 'ON'}",
                 f"-DLLG_STORAGE_TEST_WAVEFORMS={'OFF' if args.without_waveforms else 'ON'}",
                 f"-DLLG_STORAGE_TEST_SCHEDULER={'OFF' if args.without_scheduler else 'ON'}"]
    if runner.run(label + "-configure", configure)["status"] != "passed":
        return None
    if runner.run(label + "-build", [args.cmake, "--build", str(build), "--config", configuration,
                                     "--parallel", str(args.jobs)])["status"] != "passed":
        return None
    listing = runner.run(label + "-inventory", [args.ctest, "--test-dir", str(build),
                         "--build-config", configuration, "--show-only=json-v1"])
    if listing["status"] != "passed":
        return None
    capabilities = json.loads((build / "ownership-capabilities.json").read_text(encoding="utf-8"))
    tests = verify_inventory(runner.read_json(listing), capabilities)
    runner.note(label + "-coverage", "passed", f"verified {len(tests)} required tests",
                tests=tests, capabilities=capabilities)
    for capability, reason in (("waveforms", "waveforms explicitly disabled"),
                               ("scheduler", "no native libaco scheduler tests on this configuration"),
                               ("coroutines", "real stack-switch tests not covered by this configuration")):
        if not capabilities[capability]:
            runner.note(label + "-" + capability, "excluded", reason)
    runner.run(label + "-ctest", [args.ctest, "--test-dir", str(build), "--build-config", configuration,
                                  "--verbose", "--output-on-failure"], env=environment)
    binary = find_product(build, configuration, ["value_lifetime_benchmark" + (".exe" if os.name == "nt" else "")])
    measured = runner.run(label + "-memory", [str(binary), "--slots", "8192", "--rounds", "64"], env=environment)
    if measured["status"] == "passed":
        measured["measurement"] = runner.read_json(measured)
        verify_measurement(measured["measurement"])
    if not sanitize:
        library = find_product(build, configuration, ["value_oracle.dll", "libvalue_oracle.so", "libvalue_oracle.dylib"])
        runner.run(label + "-integer-oracle", [sys.executable, str(HERE / "value_oracle.py"), "--dynamic", str(library)])
    return capabilities


def rust_acceptance(runner: Runner, args) -> None:
    commands = [
        ("rust-format", ["fmt", "--check"], False),
        ("rust-all-targets", ["check", "--locked", "--all-targets", "--all-features"], False),
        ("rust-owned-structural", ["test", "--locked", "--lib", "--no-default-features",
                                   "sim::emit_c::owned::tests"], True),
        ("rust-abi-cache", ["test", "--locked", "--lib", "--no-default-features", "sim::build::tests"], True),
        ("rust-emitted-c", ["test", "--locked", "--lib", "--no-default-features",
                            "structured_owned_model_", "--", "--ignored"], True),
        ("hdl-ownership", ["test", "--locked", "--no-default-features", "--test", "sim_dynamic_ownership",
                            "--", "--test-threads=1"], True),
        ("repository-suite", ["test", "--locked", "--all-features", "--", "--test-threads=1"], True),
    ]
    tool = runner.run("rust-toolchain", [args.cargo, "--version"])
    if tool["status"] != "passed":
        for name, _, _ in commands:
            runner.note(name, "blocked", "Cargo/Rust prerequisite unavailable; not executed")
        return
    environment = {"LLG_CC": args.compiler[0], "LLG_CMAKE": args.cmake,
                   "LLG_RUNTIME_CACHE_DIR": str(runner.output / "runtime-cache")}
    for name, command, require_tests in commands:
        record = runner.run(name, [args.cargo, *command], env=environment)
        if require_tests and record["status"] == "passed":
            text = Path(record["log"]).read_text(encoding="utf-8", errors="replace")
            count = sum(int(item) for item in re.findall(r"test result: ok\. (\d+) passed", text))
            if not count:
                record.update(status="failed", reason="Cargo succeeded without executing a passing test")
            record["passed_test_count"] = count


def main() -> int:
    parser = argparse.ArgumentParser(description="Host-scoped P07 evidence. Component success is not full simulator acceptance.")
    parser.add_argument("--compiler", action="append", help="Native C compiler; repeat for multiple drivers")
    parser.add_argument("--configuration", action="append", choices=("Debug", "Release"))
    parser.add_argument("--sanitizers", action="store_true", help="Also run Clang/GCC address and undefined sanitizers")
    parser.add_argument("--sanitizer-compiler", default="clang")
    parser.add_argument("--without-waveforms", action="store_true")
    parser.add_argument("--without-scheduler", action="store_true")
    parser.add_argument("--full", action="store_true", help="Require Rust, emitted C, HDL in both optimizer modes, and repository tests")
    parser.add_argument("--cmake", default="cmake")
    parser.add_argument("--ctest", default="ctest")
    parser.add_argument("--cargo", default="cargo")
    parser.add_argument("--jobs", type=int, default=4)
    parser.add_argument("--timeout", type=float, default=1800)
    parser.add_argument("--output", type=Path, help="New/empty evidence directory; existing evidence is never overwritten")
    args = parser.parse_args()
    if args.jobs <= 0 or args.timeout <= 0 or not math.isfinite(args.timeout):
        parser.error("jobs and timeout must be positive")
    args.compiler = args.compiler or [os.environ.get("CC") or ("cl" if os.name == "nt" else "cc")]
    configurations = list(dict.fromkeys(args.configuration or ["Debug", "Release"]))
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    output = (args.output or ROOT / "target" / "p07" / f"{stamp}-{os.getpid()}").resolve()
    if output.exists() and (not output.is_dir() or any(output.iterdir())):
        parser.error(f"output is not an empty directory: {output}")
    output.mkdir(parents=True, exist_ok=True)
    runner = Runner(ROOT, output, args.timeout)
    metadata = dict(timestamp_utc=stamp, platform=platform.platform(), machine=platform.machine(),
                    python=sys.version, scope="full-host" if args.full else "runtime-components",
                    requested_compilers=args.compiler, configurations=configurations,
                    arbitrary_coroutine_sanitizers=False, frontend_tests_requested=args.full,
                    global_p07_acceptance="not established by a single host report")
    try:
        runner.note("capacity-audit", "passed", "active project runtime has no old capacity token", **audit_capacity())
        runner.run("runner-tests", [sys.executable, "-m", "unittest", "discover", "-s", str(HERE),
                                    "-p", "test_validation_runner.py", "-v"])
        runner.run("cmake-version", [args.cmake, "--version"])
        runner.run("ctest-version", [args.ctest, "--version"])
        for index, compiler in enumerate(args.compiler):
            capabilities = None
            for configuration in configurations:
                label = f"native-{index}-{configuration.lower()}"
                result = component(runner, args, label, compiler, configuration, False)
                capabilities = result or capabilities
            if capabilities is not None:
                flat = [sys.executable, str(HERE / "check_flat_runtime.py"), "--compiler", compiler]
                if not capabilities["scheduler"]:
                    flat.append("--without-scheduler")
                runner.run(f"flat-{index}", flat)
                if args.full and not all(capabilities[key] for key in ("waveforms", "scheduler", "coroutines")):
                    runner.note(f"native-{index}-full-coverage", "blocked", "full host gate requires all native runtime components")
        if args.sanitizers:
            component(runner, args, "sanitizers", args.sanitizer_compiler, "Debug", True)
        metadata["component_status"] = status(runner.records)
        if args.full:
            rust_acceptance(runner, args)
        else:
            runner.note("rust-generated-hdl", "excluded", "not requested; use --full to make Rust/HDL prerequisites mandatory")
    except (OSError, ValueError, KeyError, TypeError, RuntimeError) as error:
        runner.note("validation-driver", "failed", str(error))
    finally:
        metadata["status"] = status(runner.records)
        report = runner.save(metadata)
        print(f"Report: {report}", flush=True)
    return {"passed": 0, "failed": 1, "blocked": 2}[metadata["status"]]


if __name__ == "__main__":
    sys.exit(main())
