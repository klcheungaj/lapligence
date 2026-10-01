#!/usr/bin/env python3

import argparse
import csv
import hashlib
import json
import os
import signal
import statistics
import subprocess
import time
from pathlib import Path

from rss_attribution import parse_smaps, summarize


ROOT = Path(__file__).resolve().parents[2]
FIELDS = ("workload", "processes", "binary", "run", "status", "seconds",
          "peak_rss_kib", "steady_rss_kib", "peak_bytes_per_process",
          "steady_bytes_per_process", "stdout_sha256")


def execute(command, log, options):
    started = time.monotonic()
    with log.open("wb") as stream:
        process = subprocess.Popen(command, stdout=stream, stderr=subprocess.STDOUT,
                                   env={**os.environ, **options.get("env", {})}, start_new_session=True)
        try:
            status = process.wait(timeout=options["timeout"])
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait()
            status = 124
    return status, time.monotonic() - started


def measure(args, model, run):
    workload = model["workload"]
    count = model["processes"]
    label = model["binary"]
    binary = model["executable"]
    directory = args.output / workload / str(count) / label
    directory.mkdir(parents=True, exist_ok=True)
    prefix = directory / str(run)
    record = prefix.with_suffix(".metrics.tsv")
    snapshot = prefix.with_suffix(".smaps")
    stdout = prefix.with_suffix(".stdout")
    command = [str(args.work_dir / "perf_measure"), "--label", label, "--record", str(record),
               "--", "taskset", "-c", str(args.cpu), str(binary)]
    status, seconds = execute(command, stdout,
                              dict(timeout=args.run_timeout, env={"LLG_SCALE_SNAPSHOT": str(snapshot)}))
    row = dict(workload=workload, processes=count, binary=label, run=run,
               status=status, seconds=seconds)
    if status == 0:
        fields = record.read_text().strip().split("\t")
        row.update(seconds=int(fields[2]) / 1e9, peak_rss_kib=int(fields[3]),
                   stdout_sha256=hashlib.sha256(stdout.read_bytes()).hexdigest())
        if workload == "spawner":
            expected = f"process_scale n={count} parked\n"
            if stdout.read_text() != expected:
                row["status"] = "wrong-output"
            if not snapshot.exists():
                row["status"] = "missing-snapshot"
            else:
                totals = summarize(parse_smaps(snapshot.read_text()))
                row["steady_rss_kib"] = sum(item["rss_kib"] for item in totals.values())
                prefix.with_suffix(".attribution.json").write_text(json.dumps(totals, indent=2))
        for phase in ("peak", "steady"):
            if f"{phase}_rss_kib" in row:
                row[f"{phase}_bytes_per_process"] = row[f"{phase}_rss_kib"] * 1024 / count
    return row


def main():
    parser = argparse.ArgumentParser(description="Linux process-count RSS and runtime qualification")
    parser.add_argument("--binary", action="append", required=True, metavar="LABEL=PATH")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--work-dir", type=Path, required=True)
    parser.add_argument("--sizes", nargs="+", type=int, default=[100000, 300000, 1000000])
    parser.add_argument("--repeat", type=int, default=7)
    parser.add_argument("--cpu", type=int, default=min(os.sched_getaffinity(0)))
    parser.add_argument("--jobs", type=int, default=10)
    parser.add_argument("--generation-timeout", type=float, default=600)
    parser.add_argument("--build-timeout", type=float, default=600)
    parser.add_argument("--run-timeout", type=float, default=3600)
    parser.add_argument("--clocked", action="store_true")
    args = parser.parse_args()
    if min(args.sizes) <= 0 or args.repeat <= 0 or args.jobs <= 0 or min(
            args.generation_timeout, args.build_timeout, args.run_timeout) <= 0:
        parser.error("sizes, repeat, jobs and timeouts must be positive")
    binaries = []
    for spec in args.binary:
        label, separator, path = spec.partition("=")
        if not separator or not label or label in (".", "..") or Path(label).name != label:
            parser.error("each binary must be LABEL=PATH with a simple label")
        binaries.append((label, Path(path).resolve()))
    if len({label for label, _ in binaries}) != len(binaries):
        parser.error("binary labels must be unique")
    if args.output.exists() or args.work_dir.exists():
        parser.error("output and work-dir must be new directories")
    output = args.output.resolve()
    products = args.work_dir.resolve()
    args.output = output
    args.work_dir = products
    output.mkdir(parents=True)
    products.mkdir(parents=True)
    cc = os.environ.get("CC", "gcc")
    subprocess.run([cc, "-O2", "-std=c11", str(ROOT / "perf/scripts/perf_measure.c"),
                    "-o", str(args.work_dir / "perf_measure")], check=True)
    library = products / "libprocess_scale_snapshot.so"
    subprocess.run([cc, "-O2", "-std=c11", "-fPIC", "-shared",
                    str(ROOT / "perf/scripts/process_scale_snapshot.c"),
                    "-o", str(library)], check=True)
    metadata = dict(machine=dict(zip(("system", "node", "release", "version", "machine"), os.uname())), cpu=args.cpu, jobs=args.jobs,
                    binaries=[(label, str(path)) for label, path in binaries],
                    arguments=vars(args) | {"output": str(output), "work_dir": str(products)},
                    commands=[])
    rows = []
    builds = []
    workloads = [("spawner", n) for n in args.sizes]
    if args.clocked:
        workloads.append(("clocked", 100000))
    for workload, count in workloads:
        available = []
        for label, cli in binaries:
            directory = products / workload / str(count) / label
            directory.mkdir(parents=True)
            if workload == "spawner":
                top = "process_scale"
                source = ROOT / "perf/corpus/process_scale.sv"
                defines = ["-D", f"LLG_SCALE_N={count}"]
            else:
                top = "many_processes_registers_config"
                source = ROOT / "perf/corpus/many_processes.sv"
                defines = ["-D", f"LLG_CORPUS_N={count}", "-D", "LLG_CORPUS_EDGES=20"]
            generate = [str(cli), "--gen-only", str(source), "--top", top,
                        "--out-dir", str(directory), "--dpi-lib", str(library), *defines]
            model = directory / "sim" / top
            configure = ["cmake", "-S", str(model), "-B", str(model / "build"),
                         "-DCMAKE_BUILD_TYPE=Release", f"-DCMAKE_C_COMPILER={cc}"]
            build = ["cmake", "--build", str(model / "build"), "--parallel", str(args.jobs)]
            stages = [("generate", generate, args.generation_timeout),
                      ("configure", configure, 60), ("build", build, args.build_timeout)]
            success = True
            for stage, command, timeout in stages:
                metadata["commands"].append(command)
                status, seconds = execute(command, output / f"{workload}-{count}-{label}-{stage}.log", dict(timeout=timeout))
                builds.append(dict(workload=workload, processes=count, binary=label,
                                   stage=stage, status=status, seconds=seconds))
                print(f"{workload} {count} {label} {stage}: status={status} seconds={seconds:.3f}", flush=True)
                if status:
                    success = False
                    break
            if success:
                available.append((label, model / "build/bin/sim"))
        failed = set()
        reference = None
        for run in range(-1, args.repeat):
            for label, binary in available if run % 2 else reversed(available):
                if label in failed:
                    continue
                model = dict(workload=workload, processes=count, binary=label, executable=binary)
                row = measure(args, model, run)
                if row["status"] == 0:
                    reference = reference or row["stdout_sha256"]
                    if row["stdout_sha256"] != reference:
                        row["status"] = "hash-mismatch"
                if row["status"] != 0:
                    failed.add(label)
                rows.append(row)
                with (output / "results.tsv").open("w") as stream:
                    writer = csv.DictWriter(stream, fieldnames=FIELDS, delimiter="\t")
                    writer.writeheader()
                    writer.writerows(rows)
                (output / "builds.json").write_text(json.dumps(builds, indent=2))
                print(json.dumps(row), flush=True)
        (output / "builds.json").write_text(json.dumps(builds, indent=2))
        (output / "metadata.json").write_text(json.dumps(metadata, indent=2))
    summary = []
    for workload, count in workloads:
        for label, _ in binaries:
            group = [row for row in rows if row["workload"] == workload and
                     row["processes"] == count and row["binary"] == label and
                     row["run"] >= 0 and row["status"] == 0]
            item = dict(workload=workload, processes=count, binary=label, samples=len(group))
            for field in FIELDS[5:-1]:
                values = [row[field] for row in group if field in row]
                if values:
                    item[field] = dict(median=statistics.median(values), min=min(values), max=max(values))
            summary.append(item)
    (output / "summary.json").write_text(json.dumps(summary, indent=2))
    print(json.dumps(summary, indent=2))
    return 0 if all(item["samples"] == args.repeat for item in summary) else 1


if __name__ == "__main__":
    raise SystemExit(main())
