#!/usr/bin/env python3
import argparse
import hashlib
import json
import os
import platform
import re
import selectors
import shutil
import signal
import statistics
import subprocess
import sys
import time
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
VALUES_DIR = SCRIPT_DIR.parent / "values"

BACKENDS = {
    "legacy": {"LLG_VALUE_BACKEND": "legacy", "LLG_COMPACT_KERNELS": "portable"},
    "compact-portable": {"LLG_VALUE_BACKEND": "compact", "LLG_COMPACT_KERNELS": "portable"},
    "compact-gmp": {"LLG_VALUE_BACKEND": "compact", "LLG_COMPACT_KERNELS": "gmp"},
}

WORKLOADS = {
    "rtl-narrow": {
        "source": "rtl_datapath.sv",
        "top": "rtl_narrow",
        "class": "rtl-narrow",
        "sizes": {"smoke": {"LLG_VB_CYCLES": 200}, "standard": {"LLG_VB_CYCLES": 5000}},
    },
    "rtl-wide": {
        "source": "rtl_datapath.sv",
        "top": "rtl_wide",
        "class": "rtl-wide",
        "sizes": {"smoke": {"LLG_VB_CYCLES": 200}, "standard": {"LLG_VB_CYCLES": 1500}},
    },
    "scheduler": {
        "source": "scheduler.sv",
        "top": "scheduler",
        "class": "scheduler",
        "sizes": {"smoke": {"LLG_VB_ROUNDS": 40}, "standard": {"LLG_VB_ROUNDS": 600}},
    },
    "containers": {
        "source": "containers.sv",
        "top": "containers",
        "class": "containers",
        "sizes": {"smoke": {"LLG_VB_ROUNDS": 2000}, "standard": {"LLG_VB_ROUNDS": 200000}},
    },
    "assertions": {
        "source": "assertions.sv",
        "top": "assertions",
        "class": "assertions",
        "sizes": {"smoke": {"LLG_VB_CYCLES": 200}, "standard": {"LLG_VB_CYCLES": 1500}},
    },
    "mul65": {
        "source": "mul65.sv",
        "top": "mul65",
        "class": "mul65-witness",
        "sizes": {"smoke": {"LLG_VB_ROUNDS": 2000}, "standard": {"LLG_VB_ROUNDS": 1000000}},
    },
}

RUN_COLUMNS = [
    "workload", "backend", "round", "order", "status", "wall_s", "user_s", "sys_s",
    "cpu_s", "max_rss_kib", "load1_before", "stdout_sha256",
]

SUMMARY_METRICS = ["wall_s", "cpu_s", "max_rss_kib"]

HEAPTRACK_FIELDS = {
    "calls to allocation functions": "alloc_calls",
    "temporary memory allocations": "temporary_allocs",
    "peak heap memory consumption": "peak_heap_bytes",
    "peak RSS (including heaptrack overhead)": "peak_rss_heaptrack_bytes",
    "total memory leaked": "leaked_bytes",
}

UNIT_SCALE = {"": 1, "B": 1, "K": 1000, "M": 1000**2, "G": 1000**3, "T": 1000**4}


def die(message):
    print(f"value_backends.py: {message}", file=sys.stderr)
    raise SystemExit(2)


def parse_human_bytes(text):
    match = re.fullmatch(r"\s*([0-9]+(?:\.[0-9]+)?)\s*([KMGT]?)B?\s*", text)
    if not match:
        raise ValueError(f"unrecognized byte quantity: {text!r}")
    return round(float(match.group(1)) * UNIT_SCALE[match.group(2)])


def parse_heaptrack_summary(text):
    result = {}
    for line in text.splitlines():
        if ":" not in line:
            continue
        key, _, value = line.partition(":")
        field = HEAPTRACK_FIELDS.get(key.strip())
        if field is None:
            continue
        value = value.strip()
        if field in ("alloc_calls", "temporary_allocs"):
            match = re.match(r"([0-9]+)", value)
            if match:
                result[field] = int(match.group(1))
        else:
            try:
                result[field] = parse_human_bytes(value)
            except ValueError:
                pass
    return result


def census(model_text):
    calls = {}
    for match in re.finditer(r"\b((?:llg_gmp_)?sv4_[A-Za-z0-9_]+|llg_sv4_[A-Za-z0-9_]+)\s*\(", model_text):
        name = match.group(1)
        calls[name] = calls.get(name, 0) + 1
    destination = sum(count for name, count in calls.items() if name.endswith(("_to", "_into", "_from")))
    returning = sum(count for name, count in calls.items() if name == "sv4_replace")
    destroys = calls.get("sv4_destroy", 0)
    widths = {}
    for match in re.finditer(r"\bsv4_(?:zero|x|from_masks|from_u64|fill)(?:_to)?\s*\(([^;]*?)\)\s*;", model_text):
        arguments = [part.strip() for part in match.group(1).split(",")]
        numeric = [int(part) for part in arguments if re.fullmatch(r"[0-9]+", part)]
        if len(numeric) >= 2:
            width = numeric[-2]
            widths[width_class(width)] = widths.get(width_class(width), 0) + 1
    return {
        "sites": calls,
        "destination_sites": destination,
        "replace_sites": returning,
        "destroy_sites": destroys,
        "constructor_width_classes": widths,
    }


def width_class(width):
    if width <= 64:
        return "<=64"
    if width <= 128:
        return "65-128"
    if width <= 1024:
        return "129-1024"
    return ">1024"


def median_range(values):
    values = [value for value in values if value is not None]
    if not values:
        return None
    return {"median": statistics.median(values), "min": min(values), "max": max(values), "n": len(values)}


def summarize(rows, baseline="legacy"):
    groups = {}
    for row in rows:
        if row["status"] != 0:
            continue
        groups.setdefault((row["workload"], row["backend"]), []).append(row)
    summary = {}
    for (workload, backend), items in groups.items():
        entry = {metric: median_range([item[metric] for item in items]) for metric in SUMMARY_METRICS}
        summary[(workload, backend)] = entry
    for (workload, backend), entry in summary.items():
        base = summary.get((workload, baseline))
        if base is None or backend == baseline:
            continue
        for metric in SUMMARY_METRICS:
            if entry[metric] and base[metric] and base[metric]["median"]:
                entry[metric]["ratio"] = entry[metric]["median"] / base[metric]["median"]
        paired = paired_ratios(rows, workload, backend, baseline, "cpu_s")
        entry["cpu_s"]["paired_ratio"] = median_range(paired)
    return summary


def paired_ratios(rows, workload, backend, baseline, metric):
    by_round = {}
    for row in rows:
        if row["workload"] != workload or row["status"] != 0:
            continue
        by_round.setdefault(row["round"], {})[row["backend"]] = row[metric]
    ratios = []
    for values in by_round.values():
        if backend in values and baseline in values and values[baseline]:
            ratios.append(values[backend] / values[baseline])
    return ratios


def evaluate_budgets(summary, budgets, heap):
    findings = []
    for (workload, backend), entry in sorted(summary.items()):
        if backend == "legacy":
            continue
        workload_class = WORKLOADS.get(workload, {}).get("class", workload)
        budget = budgets.get("classes", {}).get(workload_class, budgets.get("default", {}))
        for metric, limit_key in (("cpu_s", "cpu_ratio_max"), ("wall_s", "wall_ratio_max"),
                                  ("max_rss_kib", "rss_ratio_max")):
            limit = budget.get(limit_key)
            data = entry.get(metric)
            if limit is None or not data or "ratio" not in data:
                continue
            ratio = data["ratio"]
            findings.append({
                "workload": workload, "backend": backend, "metric": metric,
                "ratio": ratio, "limit": limit, "within": ratio <= limit,
            })
        limit = budget.get("peak_heap_ratio_max")
        mine = heap.get((workload, backend), {}).get("peak_heap_bytes")
        base = heap.get((workload, "legacy"), {}).get("peak_heap_bytes")
        if limit is not None and mine is not None and base:
            ratio = mine / base
            findings.append({
                "workload": workload, "backend": backend, "metric": "peak_heap_bytes",
                "ratio": ratio, "limit": limit, "within": ratio <= limit,
            })
        limit = budget.get("alloc_calls_ratio_max")
        mine = heap.get((workload, backend), {}).get("alloc_calls")
        base = heap.get((workload, "legacy"), {}).get("alloc_calls")
        if limit is not None and mine is not None and base:
            ratio = mine / base
            findings.append({
                "workload": workload, "backend": backend, "metric": "alloc_calls",
                "ratio": ratio, "limit": limit, "within": ratio <= limit,
            })
    return findings


def round_order(backends, index):
    rotation = index % len(backends)
    order = backends[rotation:] + backends[:rotation]
    return order if index % 2 == 0 else list(reversed(order))


def parse_measure_record(text):
    fields = text.strip().split("\t")
    return {"status": int(fields[1]), "wall_ns": int(fields[2]), "max_rss_kib": int(fields[3])}


def timed_child(command, cwd, timeout, helper, record):
    load = os.getloadavg()[0]
    start = time.monotonic()
    wrapped = [str(helper), "--label", "run", "--record", str(record), "--"] + command
    with open(os.devnull, "rb") as stdin:
        process = subprocess.Popen(wrapped, cwd=cwd, stdin=stdin, stdout=subprocess.PIPE,
                                   stderr=subprocess.PIPE, start_new_session=True)
    stdout = bytearray()
    stderr = bytearray()
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ, stdout)
    selector.register(process.stderr, selectors.EVENT_READ, stderr)
    open_streams = 2
    timed_out = False
    while open_streams:
        remaining = None if timeout is None else timeout - (time.monotonic() - start)
        if remaining is not None and remaining <= 0:
            os.killpg(process.pid, signal.SIGKILL)
            timed_out = True
            break
        for key, _ in selector.select(remaining):
            chunk = os.read(key.fileobj.fileno(), 65536)
            if chunk:
                key.data.extend(chunk)
            else:
                selector.unregister(key.fileobj)
                open_streams -= 1
    selector.close()
    _, wait_status, usage = os.wait4(process.pid, 0)
    process.returncode = os.waitstatus_to_exitcode(wait_status)
    process.stdout.close()
    process.stderr.close()
    status = 124 if timed_out else process.returncode
    if status < 0:
        status = 128 - status
    measured = parse_measure_record(Path(record).read_text()) if Path(record).is_file() else None
    return {
        "status": status,
        "stdout": bytes(stdout),
        "stderr": bytes(stderr),
        "wall_s": measured["wall_ns"] / 1e9 if measured else time.monotonic() - start,
        "user_s": usage.ru_utime,
        "sys_s": usage.ru_stime,
        "cpu_s": usage.ru_utime + usage.ru_stime,
        "max_rss_kib": measured["max_rss_kib"] if measured else None,
        "load1_before": load,
    }


def command_text(command):
    return " ".join(subprocess.list2cmdline([part]) for part in command)


def backend_env(backend, gmp_root):
    env = dict(os.environ)
    env.update(BACKENDS[backend])
    if backend == "compact-gmp":
        env["GMP_ROOT"] = str(gmp_root)
    else:
        env.pop("GMP_ROOT", None)
    return env


def compile_definitions(cmake_text):
    match = re.search(r"add_compile_definitions\(([^)]*)\)", cmake_text)
    return ["-D" + item for item in match.group(1).split()] if match else []


def section_sizes(executable):
    try:
        output = subprocess.run(["size", "-A", str(executable)], capture_output=True, text=True,
                                check=True).stdout
    except (OSError, subprocess.CalledProcessError):
        return {}
    sizes = {}
    for line in output.splitlines():
        parts = line.split()
        if len(parts) >= 2 and parts[0] in (".text", ".data", ".bss", ".rodata"):
            sizes[parts[0].lstrip(".")] = int(parts[1])
    return sizes


def build_case(args, workload, backend, logs, commands):
    spec = WORKLOADS[workload]
    case_dir = args.scratch_dir / f"{workload}.{backend}"
    out_root = case_dir / "generated"
    case_dir.mkdir(parents=True)
    defines = []
    for name, value in spec["sizes"][args.size].items():
        defines += ["--define", f"{name}={value}"]
    generation = [str(args.sim_bin), "--gen-only", "--out-dir", str(out_root), "--top", spec["top"]]
    if args.no_opt:
        generation.append("--no-opt")
    generation += defines + [str(VALUES_DIR / spec["source"])]
    env = backend_env(backend, args.gmp_root)
    commands.append(f"{' '.join(f'{k}={v}' for k, v in BACKENDS[backend].items())} {command_text(generation)}")
    result = subprocess.run(generation, env=env, capture_output=True, text=True)
    (logs / f"{workload}.{backend}.generation.log").write_text(result.stdout + result.stderr)
    if result.returncode != 0:
        raise RuntimeError(f"{workload}/{backend}: generation failed with {result.returncode}")
    model_dir = Path(result.stdout.strip().splitlines()[-1])
    configure = [args.cmake, "-S", str(model_dir), "-B", str(model_dir / "build"),
                 f"-DCMAKE_C_COMPILER={args.cc}"]
    build = [args.cmake, "--build", str(model_dir / "build"), "--config", "Release",
             "--parallel", str(args.jobs)]
    commands.append(command_text(configure))
    commands.append(command_text(build))
    build_log = logs / f"{workload}.{backend}.build.log"
    with build_log.open("w") as handle:
        for command in (configure, build):
            status = subprocess.run(command, stdout=handle, stderr=subprocess.STDOUT).returncode
            if status != 0:
                raise RuntimeError(f"{workload}/{backend}: build failed, see {build_log}")
    executable = model_dir / "build" / "bin" / "sim"
    if not executable.is_file():
        raise RuntimeError(f"{workload}/{backend}: missing {executable}")
    model_text = "".join(path.read_text() for path in sorted(model_dir.glob("model*.c")))
    static = census(model_text)
    static["executable_bytes"] = executable.stat().st_size
    static["sections"] = section_sizes(executable)
    return {"model_dir": model_dir, "executable": executable, "static": static}


def build_bench(args, backend, model_dir, logs, commands):
    output = args.scratch_dir / f"value_ops_bench.{backend}"
    definitions = compile_definitions((model_dir / "CMakeLists.txt").read_text())
    command = [args.cc, "-std=c11", "-O3", "-DNDEBUG", "-Wall"] + definitions + ["-I", str(model_dir)]
    link = [str(model_dir / "build" / "libllg_runtime.a")]
    if backend == "compact-gmp":
        command += ["-I", str(args.gmp_root / "include")]
        link.append(str(args.gmp_root / "lib" / "libgmp.a"))
    command += [str(VALUES_DIR / "value_ops_bench.c")] + link + ["-lm", "-ldl", "-o", str(output)]
    commands.append(command_text(command))
    result = subprocess.run(command, capture_output=True, text=True)
    (logs / f"value_ops_bench.{backend}.build.log").write_text(result.stdout + result.stderr)
    if result.returncode != 0:
        raise RuntimeError(f"value_ops_bench/{backend}: build failed")
    return output


def parse_layout(text):
    layout = {"payload": {}}
    for line in text.splitlines():
        parts = line.split("\t")
        if parts[0] == "layout" and len(parts) == 3:
            layout[parts[1]] = int(parts[2])
        elif parts[0] == "payload" and len(parts) == 4:
            layout["payload"][int(parts[1])] = {"known": int(parts[2]), "xz": int(parts[3])}
    return layout


def parse_bench_timing(text, backend, round_index):
    rows = []
    for line in text.splitlines():
        parts = line.split("\t")
        if parts[0] != "timing" or len(parts) < 7:
            continue
        samples = [float(value) for value in parts[6:]]
        rows.append({
            "backend": backend, "round": round_index, "width": int(parts[1]), "state": parts[2],
            "op": parts[3], "mode": parts[4], "ns": statistics.median(samples),
        })
    return rows


def heaptrack_case(args, workload, backend, executable, cwd, heap_dir, commands):
    prefix = heap_dir / f"{workload}.{backend}"
    env = dict(os.environ)
    if args.tool_library_path:
        env["LD_LIBRARY_PATH"] = args.tool_library_path
    record = [str(args.heaptrack), "-o", str(prefix), str(executable)]
    commands.append(command_text(record))
    result = subprocess.run(record, cwd=cwd, env=env, capture_output=True, text=True)
    (heap_dir / f"{workload}.{backend}.record.log").write_text(result.stdout + result.stderr)
    data = sorted(heap_dir.glob(f"{workload}.{backend}.*[zg]*"))
    data = [path for path in data if not path.name.endswith(".log") and not path.name.endswith(".txt")]
    if result.returncode != 0 or not data:
        return {"error": f"heaptrack status {result.returncode}"}
    printer = args.heaptrack_print or args.heaptrack.with_name("heaptrack_print")
    print_command = [str(printer), "-f", str(data[0]), "-n", "8"]
    commands.append(command_text(print_command))
    printed = subprocess.run(print_command, env=env, capture_output=True, text=True)
    (heap_dir / f"{workload}.{backend}.txt").write_text(printed.stdout + printed.stderr)
    summary = parse_heaptrack_summary(printed.stdout)
    summary["file"] = str(data[0])
    return summary


def write_tsv(path, columns, rows):
    with path.open("w") as handle:
        handle.write("\t".join(columns) + "\n")
        for row in rows:
            handle.write("\t".join(format_cell(row.get(column)) for column in columns) + "\n")


def format_cell(value):
    if value is None:
        return ""
    if isinstance(value, float):
        return f"{value:.6g}"
    return str(value)


def host_metadata(args):
    def version(command):
        try:
            result = subprocess.run(command, capture_output=True, text=True, timeout=60)
            return (result.stdout or result.stderr).strip().splitlines()[0]
        except (OSError, IndexError, subprocess.TimeoutExpired):
            return "unavailable"

    cpu = "unknown"
    try:
        for line in Path("/proc/cpuinfo").read_text().splitlines():
            if line.startswith("model name"):
                cpu = line.split(":", 1)[1].strip()
                break
    except OSError:
        pass
    return {
        "platform": platform.platform(),
        "machine": platform.machine(),
        "cpu": cpu,
        "logical_cpus": os.cpu_count(),
        "loadavg_start": os.getloadavg(),
        "cc": version([args.cc, "--version"]),
        "cmake": version([args.cmake, "--version"]),
        "sim_version": version([str(args.sim_bin), "--version"]),
        "size": args.size,
        "runs": args.runs,
        "no_opt": args.no_opt,
        "gmp_root": str(args.gmp_root) if args.gmp_root else None,
        "pinned_cpu": args.cpu,
    }


def pinned(command, cpu):
    if cpu is None:
        return command
    return ["taskset", "-c", str(cpu)] + command


def parse_args(argv):
    parser = argparse.ArgumentParser(
        description="Measure generated-model workloads on the legacy and compact value backends.")
    parser.add_argument("--sim-bin", type=Path)
    parser.add_argument("--output-dir", type=Path)
    parser.add_argument("--scratch-dir", type=Path)
    parser.add_argument("--backend", action="append", choices=sorted(BACKENDS))
    parser.add_argument("--workload", action="append", choices=sorted(WORKLOADS))
    parser.add_argument("--list-workloads", action="store_true")
    parser.add_argument("--size", choices=["smoke", "standard"], default="standard")
    parser.add_argument("--runs", type=int, default=5)
    parser.add_argument("--warmups", type=int, default=1)
    parser.add_argument("--no-opt", action="store_true")
    parser.add_argument("--gmp-root", type=Path, default=os.environ.get("GMP_ROOT"))
    parser.add_argument("--cc", default=os.environ.get("CC", "cc"))
    parser.add_argument("--cmake", default=os.environ.get("LLG_CMAKE", "cmake"))
    parser.add_argument("--jobs", type=int, default=os.cpu_count() or 1)
    parser.add_argument("--cpu", type=int)
    parser.add_argument("--run-timeout", type=float, default=1800.0)
    parser.add_argument("--heaptrack", type=Path)
    parser.add_argument("--heaptrack-print", type=Path)
    parser.add_argument("--tool-library-path")
    parser.add_argument("--no-bench", action="store_true")
    parser.add_argument("--bench-seconds", type=float, default=0.005)
    parser.add_argument("--budgets", type=Path, default=VALUES_DIR / "budgets.json")
    parser.add_argument("--keep-scratch", action="store_true")
    args = parser.parse_args(argv)
    if args.list_workloads:
        return args
    for name in ("sim_bin", "output_dir", "scratch_dir"):
        if getattr(args, name) is None:
            parser.error(f"--{name.replace('_', '-')} is required")
    args.backends = args.backend or list(BACKENDS)
    args.workloads = args.workload or list(WORKLOADS)
    if "compact-gmp" in args.backends:
        if not args.gmp_root:
            parser.error("compact-gmp requires --gmp-root or GMP_ROOT")
        args.gmp_root = Path(args.gmp_root).resolve()
    if args.runs < 1 or args.warmups < 0:
        parser.error("--runs must be positive and --warmups non-negative")
    args.sim_bin = args.sim_bin.resolve()
    for name in ("output_dir", "scratch_dir"):
        path = getattr(args, name).resolve()
        if path.exists():
            parser.error(f"--{name.replace('_', '-')} must name a new directory: {path}")
        setattr(args, name, path)
    if args.heaptrack:
        args.heaptrack = args.heaptrack.resolve()
    return args


def main(argv):
    args = parse_args(argv)
    if args.list_workloads:
        for name, spec in WORKLOADS.items():
            sizes = "; ".join(f"{size}: {values}" for size, values in spec["sizes"].items())
            print(f"{name}\t{spec['class']}\t{spec['source']}:{spec['top']}\t{sizes}")
        return 0
    args.output_dir.mkdir(parents=True)
    args.scratch_dir.mkdir(parents=True)
    logs = args.output_dir / "logs"
    logs.mkdir()
    commands = [command_text([sys.executable] + [str(Path(__file__).resolve())] + argv)]
    metadata = host_metadata(args)

    cases = {}
    benches = {}
    layouts = {}
    for workload in args.workloads:
        for backend in args.backends:
            print(f"build {workload} {backend}", flush=True)
            cases[(workload, backend)] = build_case(args, workload, backend, logs, commands)
            if not args.no_bench and backend not in benches:
                benches[backend] = build_bench(args, backend, cases[(workload, backend)]["model_dir"],
                                               logs, commands)
                layout = subprocess.run([str(benches[backend]), "layout"], capture_output=True,
                                        text=True, check=True)
                layouts[backend] = parse_layout(layout.stdout)

    helper = args.scratch_dir / "perf_measure"
    subprocess.run([args.cc, "-std=c11", "-O2", str(SCRIPT_DIR / "perf_measure.c"), "-o", str(helper)],
                   check=True)
    rows = []
    failures = []
    hashes = {}
    for workload in args.workloads:
        for round_index in range(-args.warmups, args.runs):
            order = round_order(args.backends, round_index + args.warmups)
            for position, backend in enumerate(order):
                case = cases[(workload, backend)]
                command = pinned([str(case["executable"])], args.cpu)
                record = args.scratch_dir / "measure.tsv"
                if record.exists():
                    record.unlink()
                result = timed_child(command, case["model_dir"], args.run_timeout, helper, record)
                digest = hashlib.sha256(result["stdout"]).hexdigest()
                if round_index < 0:
                    (logs / f"{workload}.{backend}.stdout").write_bytes(result["stdout"])
                    (logs / f"{workload}.{backend}.stderr").write_bytes(result["stderr"])
                    hashes.setdefault(workload, {})[backend] = digest
                    continue
                row = {key: result[key] for key in ("status", "wall_s", "user_s", "sys_s", "cpu_s",
                                                    "max_rss_kib", "load1_before")}
                row.update({"workload": workload, "backend": backend, "round": round_index,
                            "order": position, "stdout_sha256": digest})
                rows.append(row)
                if result["status"] != 0:
                    failures.append(f"{workload}/{backend} round {round_index}: status {result['status']}")
                print(f"run {workload} {backend} r{round_index} cpu={result['cpu_s']:.3f}s "
                      f"wall={result['wall_s']:.3f}s rss={result['max_rss_kib']}KiB", flush=True)
            commands.append(f"# {workload}: {args.runs} measured rounds, order rotates per round")
    for row in rows:
        reference = hashes.get(row["workload"], {}).get("legacy")
        if reference and row["stdout_sha256"] != reference:
            failures.append(f"{row['workload']}/{row['backend']} round {row['round']}: stdout differs from legacy")
    write_tsv(args.output_dir / "runs.tsv", RUN_COLUMNS, rows)

    heap = {}
    if args.heaptrack:
        heap_dir = args.output_dir / "heaptrack"
        heap_dir.mkdir()
        for workload in args.workloads:
            for backend in args.backends:
                case = cases[(workload, backend)]
                print(f"heaptrack {workload} {backend}", flush=True)
                heap[(workload, backend)] = heaptrack_case(args, workload, backend, case["executable"],
                                                           case["model_dir"], heap_dir, commands)

    bench_rows = []
    if benches:
        for round_index in range(args.runs):
            for backend in round_order(list(benches), round_index):
                print(f"bench {backend} r{round_index}", flush=True)
                command = pinned([str(benches[backend]), "timing", str(args.bench_seconds), "3"], args.cpu)
                if round_index == 0:
                    commands.append(command_text(command))
                result = subprocess.run(command, capture_output=True, text=True, check=True)
                bench_rows += parse_bench_timing(result.stdout, backend, round_index)
        write_tsv(args.output_dir / "bench_runs.tsv",
                  ["backend", "round", "width", "state", "op", "mode", "ns"], bench_rows)

    summary = summarize(rows)
    budgets = json.loads(args.budgets.read_text()) if args.budgets and args.budgets.is_file() else {}
    findings = evaluate_budgets(summary, budgets, heap) if budgets else []
    summary_rows = []
    for (workload, backend), entry in sorted(summary.items()):
        row = {"workload": workload, "backend": backend}
        for metric in SUMMARY_METRICS:
            data = entry[metric]
            row[f"{metric}_median"] = data["median"]
            row[f"{metric}_min"] = data["min"]
            row[f"{metric}_max"] = data["max"]
            row[f"{metric}_ratio"] = data.get("ratio")
        paired = entry["cpu_s"].get("paired_ratio")
        if paired:
            row["cpu_paired_ratio_median"] = paired["median"]
            row["cpu_paired_ratio_min"] = paired["min"]
            row["cpu_paired_ratio_max"] = paired["max"]
        for key, value in heap.get((workload, backend), {}).items():
            if key != "file":
                row[key] = value
        static = cases[(workload, backend)]["static"]
        row["executable_bytes"] = static["executable_bytes"]
        row["bss_bytes"] = static["sections"].get("bss")
        row["data_bytes"] = static["sections"].get("data")
        summary_rows.append(row)
    summary_columns = ["workload", "backend"]
    for metric in SUMMARY_METRICS:
        summary_columns += [f"{metric}_median", f"{metric}_min", f"{metric}_max", f"{metric}_ratio"]
    summary_columns += ["cpu_paired_ratio_median", "cpu_paired_ratio_min", "cpu_paired_ratio_max"]
    summary_columns += list(HEAPTRACK_FIELDS.values()) + ["executable_bytes", "bss_bytes", "data_bytes"]
    write_tsv(args.output_dir / "summary.tsv", summary_columns, summary_rows)
    write_tsv(args.output_dir / "budget.tsv", ["workload", "backend", "metric", "ratio", "limit", "within"],
              findings)
    bench_summary = {}
    for row in bench_rows:
        key = (row["backend"], row["width"], row["state"], row["op"], row["mode"])
        bench_summary.setdefault(key, []).append(row["ns"])
    write_tsv(args.output_dir / "bench_summary.tsv",
              ["backend", "width", "state", "op", "mode", "median_ns", "min_ns", "max_ns"],
              [{"backend": key[0], "width": key[1], "state": key[2], "op": key[3], "mode": key[4],
                "median_ns": statistics.median(values), "min_ns": min(values), "max_ns": max(values)}
               for key, values in sorted(bench_summary.items())])
    metadata["loadavg_end"] = os.getloadavg()
    metadata["stdout_sha256"] = hashes
    metadata["layouts"] = {backend: layout for backend, layout in layouts.items()}
    metadata["static"] = {f"{workload}.{backend}": case["static"] for (workload, backend), case in cases.items()}
    metadata["heaptrack"] = {f"{workload}.{backend}": value for (workload, backend), value in heap.items()}
    metadata["failures"] = failures
    (args.output_dir / "metadata.json").write_text(json.dumps(metadata, indent=2, sort_keys=True) + "\n")
    (args.output_dir / "commands.txt").write_text("\n".join(commands) + "\n")
    if not args.keep_scratch:
        shutil.rmtree(args.scratch_dir)
    for failure in failures:
        print(f"FAIL {failure}", file=sys.stderr)
    outside = [finding for finding in findings if not finding["within"]]
    for finding in outside:
        print(f"OUTSIDE BUDGET {finding['workload']}/{finding['backend']} {finding['metric']} "
              f"{finding['ratio']:.3f} > {finding['limit']}", file=sys.stderr)
    print(f"results in {args.output_dir}")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
