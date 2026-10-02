#!/usr/bin/env python3
import argparse
import concurrent.futures
import csv
import os
from pathlib import Path
import shlex
import statistics
import subprocess

ROOT = Path(__file__).resolve().parents[2]
RT = ROOT / "src/sim/rt"
POLL_LIMITS = (0, 1, 2, 3, 4, 6, 8)
EMBED_LIMITS = (0, 4096, 16384, 65536, 131072)
CHUNK_SIZES = (256, 512, 1024, 4096, 16384)
CACHE_CAPS = (0, 65536, 262144, 1048576, 4194304, 16777216)


def call_source(depth, poll_limit, frame_bytes, embed_limit, caller_bytes=0):
    parts = []
    distance = 0
    anchors = set()
    for level in range(depth):
        distance += 1
        if distance > poll_limit:
            anchors.add(level)
            distance = 0
    for level in range(depth, -1, -1):
        if level == depth:
            fields = f"uint64_t remaining; unsigned char payload[{frame_bytes}];"
            body = """
        F->remaining = yields_per_call;
        memset(F->payload, 1, sizeof(F->payload));
        while (F->remaining--) {
            checksum += F->payload[0] + F->payload[sizeof(F->payload) - 1];
            LLG_CO_SUSPEND(co, ch, 1);
        }
        return LLG_CO_DONE;
"""
        else:
            child = level + 1
            if depth == 1 and frame_bytes + 16 > embed_limit:
                fields = "llg_co_anchor_t* child;"
                call = f"""LLG_CO_ARENA_ENTER(ch, &desc_{child}, F->child);
            LLG_CO_CALL_ARENA(co, ch, 1, &desc_{child}, F->child);"""
            elif level in anchors:
                fields = f"LLG_CO_ANCHORED(frame_{child}) child;"
                call = f"LLG_CO_CALL_ANCHOR(co, ch, 1, &desc_{child}, &F->child.an);"
            else:
                fields = f"frame_{child} child;"
                call = f"LLG_CO_CALL(co, ch, 1, fn_{child}, &F->child.co);"
            if caller_bytes:
                fields = f"unsigned char caller_payload[{caller_bytes}]; " + fields
            if level == 0:
                body = f"for (;;) {{ {call} }}"
            else:
                body = f"{call}\nreturn LLG_CO_DONE;"
        parts.append(f"""
    typedef struct {{ llg_co_frame_t co; {fields} }} frame_{level};
    LLG_CO_ROOT_FRAME_OK(frame_{level});
    LLG_CO_ANCHORED_OK(frame_{level});
    static llg_co_status_t fn_{level}(llg_co_frame_t* co, llg_co_chain_t* ch) {{
        frame_{level}* F = (frame_{level}*)co;
        LLG_CO_DISPATCH_BEGIN(co)
        LLG_CO_RESUME_CASE(1)
        LLG_CO_DISPATCH_END(co)
        {body}
    }}
    static const llg_co_desc_t desc_{level} = {{
        fn_{level}, "level {level}", sizeof(frame_{level}), NULL, 2, 0
    }};
""")
    return "\n".join(parts)


def run(command, log):
    with log.open("w") as output:
        subprocess.run(command, stdout=output, stderr=subprocess.STDOUT, check=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--work-dir", type=Path, required=True)
    parser.add_argument("--cpu", type=int, required=True)
    parser.add_argument("--jobs", type=int, default=24)
    parser.add_argument("--runs", type=int, default=7)
    parser.add_argument("--cc", default="cc")
    parser.add_argument("--cflags", default="")
    parser.add_argument("--large-chains", type=int, default=65536)
    parser.add_argument("--caller-bytes", type=int, choices=(0, 64, 256, 1024), default=0)
    parser.add_argument("--smoke", action="store_true")
    parser.add_argument("--kind", choices=("poll", "embed", "chunk", "cache"))
    parser.add_argument("--value", type=int)
    args = parser.parse_args()
    if min(args.jobs, args.runs) < 1 or args.cpu < 0:
        parser.error("jobs/runs must be positive and cpu nonnegative")
    if not 256 <= args.large_chains <= 1000000:
        parser.error("--large-chains must be between 256 and 1000000")
    if args.value is not None and args.kind is None:
        parser.error("--value requires --kind")
    subprocess.run(["taskset", "-c", str(args.cpu), "true"], check=True)
    args.output.mkdir(parents=True, exist_ok=False)
    args.work_dir.mkdir(parents=True, exist_ok=False)
    flags = [args.cc, "-std=c11", "-O3", "-DNDEBUG", "-Wall", "-Wextra",
             "-Werror", "-Wno-unused-const-variable", "-ffunction-sections",
             "-fdata-sections", "-I", str(RT), *shlex.split(args.cflags)]
    (args.output / "metadata.txt").write_text(
        f"command={shlex.join(os.sys.argv)}\nflags={shlex.join(flags)}\n"
        + subprocess.check_output([args.cc, "--version"], text=True)
        + subprocess.check_output(["uname", "-a"], text=True)
        + subprocess.check_output(["uptime"], text=True)
    )
    variants = {}
    cases = []
    def add(kind, value, depth, size, poll, embed, chunk, cap, count, rounds, yields, active, shape):
        key = (depth, size, poll, embed, chunk, cap)
        name = f"variant-{len(variants)}" if key not in variants else variants[key]
        variants[key] = name
        cases.append((kind, value, shape, name, count, rounds, yields, active))
    for limit in ((0, 3) if args.smoke else POLL_LIMITS):
        for depth in ((4,) if args.smoke else (1, 2, 3, 4, 8, 16)):
            for count in ((256,) if args.smoke else sorted({256, args.large_chains})):
                for yields in ((1,) if args.smoke else (1, 16)):
                    add("poll", limit, depth, 64, limit, 65536, 1024, 1048576,
                        count, max(8, 4000000 // count), yields, 1, f"depth{depth}-n{count}-yield{yields}")
    for limit in ((0, 16384) if args.smoke else EMBED_LIMITS):
        for size in ((4096,) if args.smoke else (256, 4096, 16384, 65536)):
            for active in ((1,) if args.smoke else (1, 16)):
                add("embed", limit, 1, size, 3, limit, 1024, 1048576, 4096,
                    max(8, 200000 * active // 4096), 1, active, f"bytes{size}-active1of{active}")
    for kind, values in (("chunk", CHUNK_SIZES), ("cache", CACHE_CAPS)):
        for value in ((values[0], values[-1]) if args.smoke else values):
            for mix in ((1,) if args.smoke else (1, 2, 3)):
                for count in ((32,) if args.smoke else (1, 64)):
                    add(kind, value, 1, 64, 3, 65536,
                        value if kind == "chunk" else 1024,
                        value if kind == "cache" else 1048576,
                        count, max(8, (20000 if mix == 3 else 100000) // count),
                        1, mix, f"mix{mix}-n{count}")
    objects = {}
    def build_runtime(key):
        chunk, cap = key
        directory = args.work_dir / f"runtime-{chunk}-{cap}"
        directory.mkdir()
        defines = [f"-DLLG_CO_ARENA_MIN_CHUNK={chunk}", f"-DLLG_CO_CHUNK_CACHE_MAX_BYTES={cap}"]
        rt_object, co_object = directory / "rt.o", directory / "co.o"
        run([*flags, *defines, "-c", str(RT / "llg_rt.c"), "-o", str(rt_object)], directory / "rt.log")
        run([*flags, *defines, "-DLLG_CO_HOST_ALLOC", "-c", str(RT / "llg_co.c"), "-o", str(co_object)], directory / "co.log")
        return key, (rt_object, co_object)
    if args.kind is not None:
        cases = [case for case in cases if case[0] == args.kind
                 and (args.value is None or case[1] == args.value)]
        if not cases:
            parser.error("no configurations match --kind/--value")
        names = {case[3] for case in cases}
        variants = {key: name for key, name in variants.items() if name in names}
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
        for key, compiled in pool.map(build_runtime, sorted({key[-2:] for key in variants})):
            objects[key] = compiled
    def build_variant(item):
        key, name = item
        depth, size, poll, embed, chunk, cap = key
        directory = args.work_dir / name
        directory.mkdir()
        (directory / "benchmark_calls.inc").write_text(call_source(depth, poll, size, embed, args.caller_bytes))
        run([*flags, f"-DLLG_CO_ARENA_MIN_CHUNK={chunk}", f"-DLLG_CO_CHUNK_CACHE_MAX_BYTES={cap}",
             "-I", str(directory), str(ROOT / "perf/designs/coroutine_tuning.c"),
             *map(str, objects[(chunk, cap)]), "-Wl,--gc-sections", "-lm", "-o", str(directory / "bench")], directory / "build.log")
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
        list(pool.map(build_variant, variants.items()))
    helper = args.work_dir / "perf_measure"
    run([args.cc, "-std=c11", "-O2", str(ROOT / "perf/scripts/perf_measure.c"), "-o", str(helper)], args.work_dir / "measure.log")
    rows = []
    for kind, value, shape, name, count, rounds, yields, active in cases:
        mode = "arena" if kind in ("chunk", "cache") else "calls"
        command = ["taskset", "-c", str(args.cpu), str(args.work_dir / name / "bench"), mode,
                   str(count), str(rounds if not args.smoke else 8), str(yields), str(active)]
        run(command, args.output / f"{kind}-{value}-{shape}-warm.log")
    for repeat in range(1, args.runs + 1):
        order = cases if repeat % 2 else list(reversed(cases))
        for kind, value, shape, name, count, rounds, yields, active in order:
            label = f"{kind}-{value}-{shape}-{repeat}"
            mode = "arena" if kind in ("chunk", "cache") else "calls"
            metric = args.output / f"{label}.tsv"
            command = [str(helper), "--label", label, "--record", str(metric), "--", "taskset", "-c", str(args.cpu),
                       str(args.work_dir / name / "bench"), mode, str(count),
                       str(rounds if not args.smoke else 8), str(yields), str(active)]
            run(command, args.output / f"{label}.log")
            _, status, wall_ns, rss = metric.read_text().strip().split("\t")
            metrics = dict(field.split("=") for field in (args.output / f"{label}.log").read_text().split())
            rows.append(dict(kind=kind, value=value, shape=shape, run=repeat, status=status,
                             wall_ns=wall_ns, rss_kib=rss, **metrics))
        print(f"tune_coroutines: repetition {repeat}/{args.runs}", flush=True)
    columns = sorted({key for row in rows for key in row})
    with (args.output / "results.tsv").open("w") as output:
        writer = csv.DictWriter(output, columns, delimiter="\t", restval="0")
        writer.writeheader()
        writer.writerows(rows)
    with (args.output / "medians.tsv").open("w") as output:
        columns = ["kind", "value", "shape", "runs", "ns_per_op", "min_ns_per_op", "max_ns_per_op",
                   "rss_kib", "root_bytes", "live_chunk_bytes", "cached_bytes", "system_allocations", "cache_hits"]
        writer = csv.DictWriter(output, columns, delimiter="\t", restval="0")
        writer.writeheader()
        for kind, value, shape, *_ in cases:
            group = [row for row in rows if (row["kind"], row["value"], row["shape"]) == (kind, value, shape)]
            durations = [int(row["elapsed_ns"]) / int(row["operations"]) for row in group]
            summary = dict(kind=kind, value=value, shape=shape, runs=len(group),
                           ns_per_op=statistics.median(durations), min_ns_per_op=min(durations), max_ns_per_op=max(durations))
            for column in columns[7:]:
                summary[column] = statistics.median(int(row.get(column, 0)) for row in group)
            writer.writerow(summary)


if __name__ == "__main__":
    main()
