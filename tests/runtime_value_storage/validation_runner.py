import json
import os
import signal
import subprocess
import time
from pathlib import Path


BASE_TESTS = {
    "storage_lifecycle", "storage_reject_limit", "storage_reject_uint32-max",
    "storage_reject_oom", "storage_reject_oom-copy", "value_ownership",
    "container_ownership", "four_state", "value_allocation_plateau",
    "value_isolation", "container_isolation",
}


def verify_inventory(inventory: dict, capabilities: dict) -> list[str]:
    if capabilities.get("schema") != "llg.owner-probes/v1":
        raise ValueError("missing or unsupported capability manifest")
    for key in ("waveforms", "scheduler", "coroutines", "sanitizers"):
        if type(capabilities.get(key)) is not bool:
            raise ValueError(f"capability {key} must be an explicit boolean")
    if capabilities["coroutines"] and (not capabilities["scheduler"] or capabilities["sanitizers"]):
        raise ValueError("inconsistent coroutine/scheduler/sanitizer capabilities")
    expected = set(BASE_TESTS)
    if capabilities["waveforms"]:
        expected.add("waveform_snapshot_lifecycle")
    if capabilities["scheduler"]:
        expected.update(("vpi_ownership", "scheduler_ownership", "generated_scope_patterns", "scope_address_index", "runtime_value_vectors", "event_array_selection", "file_input_isolation", "file_output_isolation", "native_value_scopes"))
        if capabilities["waveforms"]:
            expected.add("waveform_original_selftest")
    if capabilities["coroutines"]:
        expected.update(("coroutine_ownership", "generated_coroutine_patterns", "callback_finish_ownership",
                         "runtime_original_selftest", "runtime_region",
                         "runtime_stop-resume", "runtime_budget-finite", "event_array_waits", "nextest_control_ownership", "native_input_callbacks"))
    tests = inventory.get("tests", [])
    actual = {test["name"] for test in tests}
    if not actual or actual != expected or len(actual) != len(tests):
        raise ValueError(f"CTest inventory mismatch; missing={sorted(expected - actual)}, "
                         f"unexpected={sorted(actual - expected)}, entries={len(tests)}")
    for test in tests:
        for prop in test.get("properties", []):
            if prop["name"] == "DISABLED" and prop["value"]:
                raise ValueError(f"required test is disabled: {test['name']}")
    return sorted(actual)


def verify_measurement(measurement: dict) -> None:
    if measurement.get("schema") != "llg.value-lifetime/v1":
        raise ValueError("missing or unsupported value measurement schema")
    keys = ("slots", "rounds", "steady_payload_bytes", "peak_payload_bytes",
            "steady_live_allocations", "peak_live_allocations", "allocations_during_cycles",
            "ending_live_allocations", "ending_payload_bytes")
    for key in keys:
        if type(measurement.get(key)) is not int or measurement[key] < 0:
            raise ValueError(f"measurement {key} must be a nonnegative integer")
    if not measurement["slots"] or not measurement["rounds"]:
        raise ValueError("measurement did not exercise a workload")
    if measurement["ending_live_allocations"] or measurement["ending_payload_bytes"]:
        raise ValueError("value allocations survived benchmark teardown")
    if measurement["allocations_during_cycles"] != measurement["rounds"] * measurement["steady_live_allocations"]:
        raise ValueError("measurement allocation count does not match the workload")
    if measurement["peak_payload_bytes"] < measurement["steady_payload_bytes"] or not (
            measurement["steady_live_allocations"] <= measurement["peak_live_allocations"] <=
            measurement["steady_live_allocations"] + 1):
        raise ValueError("measurement has an inconsistent or unbounded live allocation peak")


def find_product(directory: Path, configuration: str, filenames: list[str]) -> Path:
    matches = [parent / name for parent in (directory / configuration, directory)
               for name in filenames if (parent / name).is_file()]
    if len(matches) != 1:
        raise ValueError(f"expected exactly one built product {filenames} in {directory}; found {matches}")
    return matches[0]


class Runner:
    def __init__(self, root: Path, output: Path, timeout: float = 900):
        self.root = root
        self.output = output
        self.timeout = timeout
        self.records: list[dict] = []
        (output / "logs").mkdir(parents=True, exist_ok=True)

    def note(self, name: str, status: str, reason: str, **details) -> dict:
        record = dict(name=name, status=status, reason=reason, **details)
        self.records.append(record)
        print(f"{status.upper()}: {name}: {reason}", flush=True)
        return record

    def run(self, name: str, command: list[str], env: dict[str, str] | None = None,
            timeout: float | None = None) -> dict:
        log = self.output / "logs" / f"{len(self.records):03d}-{name}.log"
        started = time.perf_counter()
        record = dict(name=name, command=command, cwd=str(self.root), log=str(log),
                      status="blocked", returncode=None)
        with log.open("w", encoding="utf-8") as stream:
            try:
                process = subprocess.Popen(command, cwd=self.root, stdout=stream,
                                           stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL,
                                           env={**os.environ, **(env or {})},
                                           start_new_session=os.name == "posix")
            except OSError as error:
                record["reason"] = str(error)
                stream.write(str(error) + "\n")
            else:
                try:
                    record["returncode"] = process.wait(timeout=self.timeout if timeout is None else timeout)
                    record["status"] = "passed" if record["returncode"] == 0 else "failed"
                except subprocess.TimeoutExpired:
                    record["status"] = "failed"
                    record["reason"] = "command timed out"
                    try:
                        if os.name == "posix":
                            os.killpg(process.pid, signal.SIGKILL)
                        else:
                            subprocess.run(["taskkill", "/PID", str(process.pid), "/T", "/F"],
                                           stdout=stream, stderr=subprocess.STDOUT, timeout=15,
                                           check=True)
                    except (OSError, subprocess.SubprocessError) as error:
                        record["termination_error"] = str(error)
                        process.kill()
                    process.wait(timeout=15)
        record["seconds"] = time.perf_counter() - started
        self.records.append(record)
        print(f"{record['status'].upper()}: {name} ({record['seconds']:.2f}s)", flush=True)
        return record

    def read_json(self, record: dict) -> dict:
        return json.loads(Path(record["log"]).read_text(encoding="utf-8", errors="replace"))

    def save(self, metadata: dict) -> Path:
        report = self.output / "report.json"
        content = dict(schema="llg.dynamic-validation/v1", **metadata, checks=self.records)
        temporary = report.with_suffix(".tmp")
        temporary.write_text(json.dumps(content, indent=2) + "\n", encoding="utf-8")
        temporary.replace(report)
        return report
