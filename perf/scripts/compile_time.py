#!/usr/bin/env python3
import argparse
import hashlib
import json
import os
import re
import shlex
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent.parent
CORPUS_DIR = REPO_ROOT / "perf" / "corpus"

BASE_CFLAGS = ""
SETS = ("smoke", "ladder", "large")
FRONT_END_PHASES = ("phase setup", "phase parsing", "phase lang. deferred")
MODEL_SOURCE_PREFIX = "model"
TOP_PASSES = 25

COLUMNS = (
    "design",
    "sim",
    "repeat",
    "generation_status",
    "generation_ms",
    "generation_rss_kib",
    "model_tus",
    "model_source_bytes",
    "preprocessed_bytes",
    "build_status",
    "build_jobs",
    "build_ms",
    "build_rss_kib",
    "model_object_bytes",
    "executable_bytes",
    "tu_compile_ms",
    "tu_compile_rss_kib",
    "front_end_s",
    "optimize_s",
    "ipa_s",
    "compiler_total_s",
    "compiler_mem_kib",
    "run_status",
    "run_ms",
    "run_rss_kib",
    "stdout_sha256",
)
SUMMARY_COLUMNS = (
    "generation_ms",
    "model_source_bytes",
    "preprocessed_bytes",
    "build_ms",
    "build_rss_kib",
    "model_object_bytes",
    "tu_compile_ms",
    "tu_compile_rss_kib",
    "front_end_s",
    "optimize_s",
    "compiler_total_s",
    "run_ms",
)


@dataclass(frozen=True)
class Design:
    file: str
    top: str
    defines: tuple
    level: str


def design_catalog():
    designs = {}
    for count, level in ((512, "smoke"), (1024, "ladder"), (2048, "ladder"), (4100, "ladder")):
        designs[f"pca-{count}"] = Design(
            "pca_sites.sv", "pca_sites", (f"LLG_CORPUS_N={count}",), level
        )
    for count, level in ((16, "smoke"), (64, "ladder"), (128, "ladder")):
        designs[f"tasks-{count}"] = Design(
            "testbench_tasks.sv",
            "testbench_tasks",
            (f"LLG_CORPUS_N={count}", "LLG_CORPUS_ITERS=1"),
            level,
        )
    for count, label, level in ((10000, "10k", "ladder"), (20000, "20k", "ladder"), (100000, "100k", "large")):
        designs[f"many-registers-{label}"] = Design(
            "many_processes.sv",
            "many_processes_registers_config",
            (f"LLG_CORPUS_N={count}", "LLG_CORPUS_EDGES=2"),
            level,
        )
    return designs


def designs_in_set(designs, name):
    limit = SETS.index(name)
    return [key for key, design in designs.items() if SETS.index(design.level) <= limit]


@dataclass
class Measurement:
    status: int
    ms: float
    rss_kib: int


def measure(command, cwd=None, stdout=None, stderr=None, env=None):
    start = time.monotonic_ns()
    try:
        process = subprocess.Popen(command, cwd=cwd, stdout=stdout, stderr=stderr, env=env)
    except OSError as error:
        if stderr is not None and hasattr(stderr, "write"):
            stderr.write(f"{command[0]}: {error}\n".encode())
        return Measurement(127, 0.0, 0)
    _, wait_status, usage = os.wait4(process.pid, 0)
    elapsed_ms = (time.monotonic_ns() - start) / 1e6
    process.returncode = os.waitstatus_to_exitcode(wait_status)
    return Measurement(process.returncode, elapsed_ms, usage.ru_maxrss)


TIME_ROW = re.compile(
    r"^\s*(?P<name>\S.*?)\s*:"
    r"\s*(?P<usr>\d+\.\d+)\s*(?:\(\s*\d+%\))?"
    r"\s*(?P<sys>\d+\.\d+)\s*(?:\(\s*\d+%\))?"
    r"\s*(?P<wall>\d+\.\d+)\s*(?:\(\s*\d+%\))?"
    r"\s*(?P<mem>\d+(?:\.\d+)?[kMG]?)"
)
MEMORY_UNITS = {"": 1 / 1024, "k": 1, "M": 1024, "G": 1024 * 1024}


def memory_kib(text):
    match = re.fullmatch(r"(\d+(?:\.\d+)?)([kMG]?)", text)
    if not match:
        return 0
    return int(float(match.group(1)) * MEMORY_UNITS[match.group(2)])


def parse_gcc_time_report(text):
    rows = {}
    for line in text.splitlines():
        match = TIME_ROW.match(line)
        if not match:
            continue
        name = match.group("name").lstrip("|").strip()
        wall, mem = rows.get(name, (0.0, 0))
        rows[name] = (wall + float(match.group("wall")), max(mem, memory_kib(match.group("mem"))))
    total_wall, total_mem = rows.get("TOTAL", (0.0, 0))
    passes = sorted(
        ((wall, name, mem) for name, (wall, mem) in rows.items()
         if name != "TOTAL" and not name.startswith("phase ")),
        reverse=True,
    )
    return {
        "front_end_s": sum(rows.get(phase, (0.0, 0))[0] for phase in FRONT_END_PHASES),
        "optimize_s": rows.get("phase opt and generate", (0.0, 0))[0],
        "ipa_s": rows.get("callgraph ipa passes", (0.0, 0))[0],
        "compiler_total_s": total_wall,
        "compiler_mem_kib": total_mem,
        "passes": [(name, wall, mem) for wall, name, mem in passes],
    }


def parse_clang_time_trace(data):
    totals = {}
    for event in data.get("traceEvents", []):
        name = event.get("name", "")
        if name.startswith("Total ") and "dur" in event:
            key = name[len("Total "):]
            totals[key] = totals.get(key, 0.0) + event["dur"] / 1e6
    passes = sorted(((wall, name) for name, wall in totals.items()), reverse=True)
    return {
        "front_end_s": totals.get("Frontend", 0.0),
        "optimize_s": totals.get("Backend", 0.0),
        "ipa_s": 0.0,
        "compiler_total_s": totals.get("ExecuteCompiler", 0.0),
        "compiler_mem_kib": 0,
        "passes": [(name, wall, 0) for wall, name in passes],
    }


def compile_arguments(entry):
    if "arguments" in entry:
        return list(entry["arguments"])
    return shlex.split(entry["command"])


def retarget(arguments, output, extra, preprocess=False):
    result = []
    skip = False
    for argument in arguments:
        if skip:
            skip = False
            continue
        if argument == "-o":
            skip = True
            continue
        if argument.startswith("-o") and len(argument) > 2:
            continue
        if preprocess and argument == "-c":
            continue
        result.append(argument)
    compiler, rest = result[0], result[1:]
    mode = ["-E"] if preprocess else []
    return [compiler, *mode, *extra, *rest, "-o", str(output)]


def object_path(entry):
    if "output" in entry:
        path = Path(entry["output"])
    else:
        arguments = compile_arguments(entry)
        path = None
        for index, argument in enumerate(arguments):
            if argument == "-o" and index + 1 < len(arguments):
                path = Path(arguments[index + 1])
            elif argument.startswith("-o") and len(argument) > 2:
                path = Path(argument[2:])
        if path is None:
            return None
    return path if path.is_absolute() else Path(entry["directory"]) / path


def model_entries(compile_commands):
    entries = []
    for entry in compile_commands:
        source = Path(entry["file"])
        if source.name.startswith(MODEL_SOURCE_PREFIX) and source.suffix == ".c":
            entries.append(entry)
    return sorted(entries, key=lambda entry: entry["file"])


def compiler_kind(cc):
    try:
        text = subprocess.run([cc, "--version"], capture_output=True, text=True, check=False).stdout
    except OSError:
        return "unknown"
    lowered = text.lower()
    if "clang" in lowered:
        return "clang"
    if "gcc" in lowered or "free software foundation" in lowered:
        return "gcc"
    return "unknown"


def command_version(command):
    try:
        output = subprocess.run(command, capture_output=True, text=True, check=False)
    except OSError as error:
        return str(error)
    lines = (output.stdout or output.stderr).strip().splitlines()
    return lines[0] if lines else ""


def parse_sim(value):
    label, separator, path = value.partition("=")
    if not separator:
        path = value
        label = Path(value).name
    if not re.fullmatch(r"[A-Za-z0-9_.-]+", label):
        raise argparse.ArgumentTypeError(f"invalid simulator label: {label}")
    return label, Path(path).resolve()


def file_bytes(path):
    try:
        return path.stat().st_size
    except OSError:
        return 0


def last_nonempty_line(path):
    lines = [line for line in path.read_text(errors="replace").splitlines() if line.strip()]
    return lines[-1].strip() if lines else ""


def phase_reports(entries, kind, work, logs):
    totals = {
        "tu_compile_ms": 0.0,
        "tu_compile_rss_kib": 0,
        "front_end_s": 0.0,
        "optimize_s": 0.0,
        "ipa_s": 0.0,
        "compiler_total_s": 0.0,
        "compiler_mem_kib": 0,
        "preprocessed_bytes": 0,
    }
    passes = []
    status = 0
    for index, entry in enumerate(entries):
        arguments = compile_arguments(entry)
        directory = entry["directory"]
        name = Path(entry["file"]).name
        preprocessed = work / f"tu{index}.i"
        with open(logs / f"{name}.preprocess.stderr", "wb") as stderr:
            measured = measure(retarget(arguments, preprocessed, [], preprocess=True), cwd=directory, stderr=stderr)
        status = status or measured.status
        totals["preprocessed_bytes"] += file_bytes(preprocessed)
        preprocessed.unlink(missing_ok=True)

        output = work / f"tu{index}.o"
        extra = ["-ftime-trace"] if kind == "clang" else ["-ftime-report"] if kind == "gcc" else []
        report_path = logs / f"{name}.time-report"
        with open(report_path, "wb") as stderr:
            measured = measure(retarget(arguments, output, extra), cwd=directory, stderr=stderr)
        status = status or measured.status
        totals["tu_compile_ms"] += measured.ms
        totals["tu_compile_rss_kib"] = max(totals["tu_compile_rss_kib"], measured.rss_kib)
        report = None
        if kind == "gcc":
            report = parse_gcc_time_report(report_path.read_text(errors="replace"))
        elif kind == "clang":
            trace = output.with_suffix(".json")
            if trace.is_file():
                report = parse_clang_time_trace(json.loads(trace.read_text()))
                shutil.copyfile(trace, logs / f"{name}.time-trace.json")
                trace.unlink()
        output.unlink(missing_ok=True)
        if report is None:
            continue
        for key in ("front_end_s", "optimize_s", "ipa_s", "compiler_total_s"):
            totals[key] += report[key]
        totals["compiler_mem_kib"] = max(totals["compiler_mem_kib"], report["compiler_mem_kib"])
        passes.extend((name, pass_name, wall, mem) for pass_name, wall, mem in report["passes"][:TOP_PASSES])
    return status, totals, passes


def run_case(args, design_name, design, sim_label, sim_path, repeat, kind):
    row = {column: "-" for column in COLUMNS}
    row.update(design=design_name, sim=sim_label, repeat=repeat)
    case_name = f"{design_name}.{sim_label}.{repeat}"
    case_dir = args.scratch / case_name
    logs = args.output_dir / "logs" / case_name
    shutil.rmtree(case_dir, ignore_errors=True)
    case_dir.mkdir(parents=True)
    logs.mkdir(parents=True, exist_ok=True)

    command = [str(sim_path), "--gen-only", "--out-dir", str(case_dir / "generated"), "--top", design.top]
    if args.no_opt:
        command.append("--no-opt")
    for define in design.defines:
        command += ["--define", define]
    command.append(str(CORPUS_DIR / design.file))
    with open(logs / "generation.stdout", "wb") as stdout, open(logs / "generation.stderr", "wb") as stderr:
        generation = measure(command, stdout=stdout, stderr=stderr)
    row.update(
        generation_status=generation.status,
        generation_ms=f"{generation.ms:.1f}",
        generation_rss_kib=generation.rss_kib,
    )
    model_dir = Path(last_nonempty_line(logs / "generation.stdout") or case_dir)
    if generation.status != 0 or not (model_dir / "CMakeLists.txt").is_file():
        return row, [], False

    build_dir = model_dir / "build"
    configure = [
        args.cmake,
        "-S", str(model_dir),
        "-B", str(build_dir),
        "-DCMAKE_BUILD_TYPE=Release",
        f"-DCMAKE_C_COMPILER={args.cc}",
        f"-DCMAKE_C_FLAGS:STRING={args.cflags}",
        "-DCMAKE_EXPORT_COMPILE_COMMANDS=ON",
    ]
    with open(logs / "configure.stdout", "wb") as stdout, open(logs / "configure.stderr", "wb") as stderr:
        configured = measure(configure, stdout=stdout, stderr=stderr)
    build = Measurement(configured.status, 0.0, 0)
    if configured.status == 0:
        command = [args.cmake, "--build", str(build_dir), "--config", "Release", "--parallel", str(args.jobs)]
        with open(logs / "build.stdout", "wb") as stdout, open(logs / "build.stderr", "wb") as stderr:
            build = measure(command, stdout=stdout, stderr=stderr)
    row.update(build_status=build.status, build_jobs=args.jobs, build_ms=f"{build.ms:.1f}", build_rss_kib=build.rss_kib)

    commands_path = build_dir / "compile_commands.json"
    entries = model_entries(json.loads(commands_path.read_text())) if commands_path.is_file() else []
    row.update(
        model_tus=len(entries),
        model_source_bytes=sum(file_bytes(Path(entry["file"])) for entry in entries),
        model_object_bytes=sum(file_bytes(path) for path in map(object_path, entries) if path is not None),
    )
    executable = build_dir / "bin" / "sim"
    row["executable_bytes"] = file_bytes(executable)
    ok = build.status == 0 and executable.is_file()

    passes = []
    if not args.no_phases and entries:
        work = case_dir / "phases"
        work.mkdir()
        status, totals, passes = phase_reports(entries, kind, work, logs)
        row.update(
            tu_compile_ms=f"{totals['tu_compile_ms']:.1f}",
            tu_compile_rss_kib=totals["tu_compile_rss_kib"],
            preprocessed_bytes=totals["preprocessed_bytes"],
        )
        if kind in ("gcc", "clang"):
            row.update(
                front_end_s=f"{totals['front_end_s']:.2f}",
                optimize_s=f"{totals['optimize_s']:.2f}",
                ipa_s=f"{totals['ipa_s']:.2f}",
                compiler_total_s=f"{totals['compiler_total_s']:.2f}",
                compiler_mem_kib=totals["compiler_mem_kib"],
            )
        ok = ok and status == 0

    if args.run and ok:
        env = dict(os.environ, LLG_SIM_OUT_DIR=str(logs))
        with open(logs / "simulation.stdout", "wb") as stdout, open(logs / "simulation.stderr", "wb") as stderr:
            run = measure([str(executable)], cwd=case_dir, stdout=stdout, stderr=stderr, env=env)
        digest = hashlib.sha256((logs / "simulation.stdout").read_bytes()).hexdigest()
        row.update(run_status=run.status, run_ms=f"{run.ms:.1f}", run_rss_kib=run.rss_kib, stdout_sha256=digest)
        ok = ok and run.status == 0

    if not args.keep_scratch:
        shutil.rmtree(case_dir, ignore_errors=True)
    return row, passes, ok


def numeric(value):
    try:
        return float(value)
    except (TypeError, ValueError):
        return None


def medians(rows):
    groups = {}
    for row in rows:
        groups.setdefault((row["design"], row["sim"]), []).append(row)
    result = []
    for (design, sim), members in groups.items():
        summary = {"design": design, "sim": sim, "repeats": len(members)}
        for column in SUMMARY_COLUMNS:
            values = [value for value in (numeric(member[column]) for member in members) if value is not None]
            summary[column] = statistics.median(values) if values else None
        result.append(summary)
    return result


def format_value(value):
    if value is None:
        return "-"
    if float(value).is_integer():
        return str(int(value))
    return f"{value:.2f}"


def comparison_table(summaries, labels):
    lines = ["design\tsim\tbuild_s\ttu_compile_s\tfront_end_s\toptimize_s\tmodel_MB\tobject_MB\tbuild_vs_first"]
    by_design = {}
    for summary in summaries:
        by_design.setdefault(summary["design"], {})[summary["sim"]] = summary
    for design, sims in by_design.items():
        first = sims.get(labels[0])
        for label in labels:
            summary = sims.get(label)
            if summary is None:
                continue
            ratio = "-"
            if first and first["build_ms"] and summary["build_ms"] is not None:
                ratio = f"{summary['build_ms'] / first['build_ms']:.3f}"

            def scaled(column, factor):
                value = summary[column]
                return "-" if value is None else f"{value / factor:.2f}"

            lines.append("\t".join((
                design,
                label,
                scaled("build_ms", 1000),
                scaled("tu_compile_ms", 1000),
                scaled("front_end_s", 1),
                scaled("optimize_s", 1),
                scaled("model_source_bytes", 1e6),
                scaled("model_object_bytes", 1e6),
                ratio,
            )))
    return "\n".join(lines) + "\n"


def write_metadata(args, kind):
    lines = [
        ("compiler", f"{args.cc} ({kind})"),
        ("compiler_version", command_version([args.cc, "--version"])),
        ("cmake_version", command_version([args.cmake, "--version"])),
        ("cflags", args.cflags),
        ("jobs", str(args.jobs)),
        ("no_opt", str(args.no_opt)),
        ("uname", " ".join(os.uname())),
        ("loadavg", " ".join(f"{value:.2f}" for value in os.getloadavg())),
    ]
    for label, path in args.sim_bin:
        lines.append((f"sim.{label}", str(path)))
        lines.append((f"sim.{label}.version", command_version([str(path), "--version"])))
    (args.output_dir / "metadata.tsv").write_text("".join(f"{key}\t{value}\n" for key, value in lines))


def parse_args(argv):
    designs = design_catalog()
    parser = argparse.ArgumentParser(
        description="Measure generated-model compile time for one or more llg binaries (A/B)."
    )
    parser.add_argument("--sim-bin", action="append", type=parse_sim, default=[],
                        metavar="[LABEL=]PATH", help="llg binary; repeat for A/B comparisons")
    parser.add_argument("--output-dir", type=Path, help="results, logs and time reports")
    parser.add_argument("--scratch-dir", type=Path, default=Path(os.environ.get("TMPDIR", "/tmp")),
                        help="parent for generated models (default: $TMPDIR or /tmp)")
    parser.add_argument("--design", action="append", default=[], choices=sorted(designs),
                        help="design to measure; repeatable")
    parser.add_argument("--set", choices=SETS, default="smoke",
                        help="design set when no --design is given (default: smoke)")
    parser.add_argument("--repeat", type=int, default=1, help="repetitions per design and binary")
    parser.add_argument("--jobs", type=int, default=os.cpu_count() or 1,
                        help="cmake --build parallelism (default: all cores)")
    parser.add_argument("--cc", default=os.environ.get("CC", "cc"), help="C compiler (default: $CC or cc)")
    parser.add_argument("--cmake", default=os.environ.get("LLG_CMAKE", "cmake"), help="CMake executable")
    parser.add_argument("--cflags", default=BASE_CFLAGS,
                        help="Extra CMAKE_C_FLAGS (default: generated project flags)")
    parser.add_argument("--no-opt", action="store_true", help="generate with llg --no-opt")
    parser.add_argument("--no-phases", action="store_true",
                        help="skip the per-TU preprocess and compiler time-report compiles")
    parser.add_argument("--run", action="store_true", help="also run each model once")
    parser.add_argument("--keep-scratch", action="store_true", help="retain generated models")
    parser.add_argument("--list-designs", action="store_true", help="list designs and exit")
    args = parser.parse_args(argv)
    if args.list_designs:
        return args, designs
    if not args.sim_bin:
        parser.error("--sim-bin is required")
    if args.output_dir is None:
        parser.error("--output-dir is required")
    labels = [label for label, _ in args.sim_bin]
    if len(set(labels)) != len(labels):
        parser.error("simulator labels must be unique")
    for _, path in args.sim_bin:
        if not os.access(path, os.X_OK):
            parser.error(f"simulator binary is not executable: {path}")
    if args.repeat < 1 or args.jobs < 1:
        parser.error("--repeat and --jobs must be positive")
    return args, designs


def main(argv):
    args, designs = parse_args(argv)
    if args.list_designs:
        sys.stdout.write("name\tset\ttop\tdefines\n")
        for name, design in designs.items():
            sys.stdout.write(f"{name}\t{design.level}\t{design.top}\t{' '.join(design.defines)}\n")
        return 0
    selected = args.design or designs_in_set(designs, args.set)
    args.output_dir = args.output_dir.resolve()
    args.output_dir.mkdir(parents=True, exist_ok=True)
    args.scratch_dir.mkdir(parents=True, exist_ok=True)
    args.scratch = Path(tempfile.mkdtemp(prefix="llg-compile-time.", dir=args.scratch_dir))
    kind = compiler_kind(args.cc)
    write_metadata(args, kind)

    rows = []
    failures = []
    passes_dir = args.output_dir / "passes"
    passes_dir.mkdir(exist_ok=True)
    try:
        for design_name in selected:
            for repeat in range(1, args.repeat + 1):
                for label, path in args.sim_bin:
                    print(f"compile_time: {design_name} {label} #{repeat}", file=sys.stderr, flush=True)
                    row, passes, ok = run_case(args, design_name, designs[design_name], label, path, repeat, kind)
                    rows.append(row)
                    if not ok:
                        failures.append(f"{design_name}.{label}.{repeat}")
                    with open(passes_dir / f"{design_name}.{label}.{repeat}.tsv", "w") as out:
                        out.write("tu\tpass\twall_s\tmem_kib\n")
                        for tu, pass_name, wall, mem in passes:
                            out.write(f"{tu}\t{pass_name}\t{wall:.2f}\t{mem}\n")
                    with open(args.output_dir / "results.tsv", "w") as out:
                        out.write("\t".join(COLUMNS) + "\n")
                        for written in rows:
                            out.write("\t".join(str(written[column]) for column in COLUMNS) + "\n")
    finally:
        if args.keep_scratch:
            print(f"compile_time: scratch retained at {args.scratch}", file=sys.stderr)
        else:
            shutil.rmtree(args.scratch, ignore_errors=True)

    summaries = medians(rows)
    with open(args.output_dir / "medians.tsv", "w") as out:
        out.write("\t".join(("design", "sim", "repeats", *SUMMARY_COLUMNS)) + "\n")
        for summary in summaries:
            values = [summary["design"], summary["sim"], str(summary["repeats"])]
            values += [format_value(summary[column]) for column in SUMMARY_COLUMNS]
            out.write("\t".join(values) + "\n")
    table = comparison_table(summaries, [label for label, _ in args.sim_bin])
    (args.output_dir / "comparison.tsv").write_text(table)
    sys.stdout.write(table)
    if failures:
        print(f"compile_time: failed cases: {' '.join(failures)} (see {args.output_dir / 'logs'})", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
