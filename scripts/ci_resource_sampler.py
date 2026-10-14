#!/usr/bin/env python3
import argparse
import json
import os
import shutil
import subprocess
import sys
import time
from collections import defaultdict
from pathlib import Path

WATCHED = ("sim", "llg", "cmake", "ninja", "cl", "link", "lib", "ccache", "cargo-nextest", "msmpeng", "conhost")
TOP = 8

WINDOWS_QUERY = r"""
$ErrorActionPreference = 'SilentlyContinue'
Get-CimInstance Win32_Process | ForEach-Object {
    "P|{0}|{1}|{2}|{3}|{4}" -f $_.ProcessId, $_.ParentProcessId, $_.Name, ([uint64]$_.KernelModeTime + [uint64]$_.UserModeTime), $_.WorkingSetSize
}
$os = Get-CimInstance Win32_OperatingSystem
"M|{0}|{1}|{2}|{3}" -f $os.TotalVisibleMemorySize, $os.FreePhysicalMemory, $os.TotalVirtualMemorySize, $os.FreeVirtualMemory
Get-CimInstance Win32_PageFileUsage | ForEach-Object { "F|{0}|{1}|{2}" -f $_.Name, $_.AllocatedBaseSize, $_.CurrentUsage }
"""


def base_name(name):
    name = name.strip().lower()
    return name[:-4] if name.endswith(".exe") else name


def parse_windows(text):
    processes, memory, pagefiles = [], {}, []
    for line in text.splitlines():
        fields = line.strip().split("|")
        try:
            if fields[0] == "P" and len(fields) == 6:
                processes.append({"pid": int(fields[1]), "ppid": int(fields[2]), "name": base_name(fields[3]),
                                  "cpu_s": int(fields[4] or 0) / 1e7, "rss_mb": int(fields[5] or 0) / 1048576})
            elif fields[0] == "M" and len(fields) == 5:
                total, free, commit_limit, commit_free = (int(value) / 1024 for value in fields[1:])
                memory = {"total_mb": round(total), "available_mb": round(free),
                          "commit_mb": round(commit_limit - commit_free), "commit_limit_mb": round(commit_limit)}
            elif fields[0] == "F" and len(fields) == 4:
                pagefiles.append({"name": fields[1], "allocated_mb": int(fields[2] or 0),
                                  "used_mb": int(fields[3] or 0)})
        except ValueError:
            continue
    return processes, memory, pagefiles


def cpu_seconds(text):
    days, _, clock = text.strip().rpartition("-")
    parts = [float(part) for part in clock.split(":")]
    while len(parts) < 3:
        parts.insert(0, 0.0)
    return (int(days) if days else 0) * 86400 + parts[0] * 3600 + parts[1] * 60 + parts[2]


def parse_ps(text):
    processes = []
    for line in text.splitlines():
        fields = line.split(None, 4)
        if len(fields) != 5:
            continue
        pid, ppid, cpu, rss, command = fields
        try:
            processes.append({"pid": int(pid), "ppid": int(ppid), "name": base_name(os.path.basename(command)),
                              "cpu_s": cpu_seconds(cpu), "rss_mb": int(rss) / 1024})
        except ValueError:
            continue
    return processes


def parse_meminfo(text):
    values = {}
    for line in text.splitlines():
        name, _, rest = line.partition(":")
        if rest.split():
            values[name] = int(rest.split()[0]) / 1024
    if "MemTotal" not in values:
        return {}
    swap = values.get("SwapTotal", 0) - values.get("SwapFree", 0)
    return {"total_mb": round(values["MemTotal"]), "available_mb": round(values.get("MemAvailable", 0)),
            "commit_mb": round(values.get("Committed_AS", 0)), "commit_limit_mb": round(values.get("CommitLimit", 0)),
            "swap_used_mb": round(swap)}


def run(command):
    try:
        return subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True,
                              errors="replace", timeout=60).stdout
    except (OSError, subprocess.SubprocessError):
        return ""


def query():
    if os.name == "nt":
        exe = shutil.which("powershell") or shutil.which("pwsh")
        if not exe:
            return [], {}, []
        return parse_windows(run([exe, "-NoProfile", "-NonInteractive", "-Command", WINDOWS_QUERY]))
    processes = parse_ps(run(["ps", "-A", "-o", "pid=,ppid=,time=,rss=,comm="]))
    try:
        memory = parse_meminfo(Path("/proc/meminfo").read_text())
    except OSError:
        memory = {}
    return processes, memory, []


def cpu_top(previous, current, elapsed, cpus, top=TOP):
    before = {(p["pid"], p["name"]): p["cpu_s"] for p in previous}
    by_name = defaultdict(float)
    for process in current:
        delta = process["cpu_s"] - before.get((process["pid"], process["name"]), 0.0)
        if delta > 0:
            by_name[process["name"]] += delta
    scale = 100.0 / max(elapsed * max(cpus, 1), 1e-9)
    ranked = sorted(by_name.items(), key=lambda item: item[1], reverse=True)[:top]
    return [[name, round(seconds * scale, 1)] for name, seconds in ranked]


def process_counts(processes):
    alive = {p["pid"] for p in processes}
    counts, orphans = defaultdict(int), defaultdict(int)
    for process in processes:
        if process["name"] in WATCHED:
            counts[process["name"]] += 1
            if process["ppid"] not in alive:
                orphans[process["name"]] += 1
    return {"total": len(processes), "watched": dict(sorted(counts.items())), "orphaned": dict(sorted(orphans.items()))}


def tree_stats(path):
    files = size = 0
    for root, _, names in os.walk(path, onerror=lambda error: None):
        for name in names:
            try:
                size += os.lstat(os.path.join(root, name)).st_size
                files += 1
            except OSError:
                continue
    return {"mb": round(size / 1048576, 1), "files": files}


def disk_free(paths):
    free = {}
    for path in paths:
        anchor = Path(path).anchor or "/"
        if anchor not in free:
            try:
                free[anchor] = round(shutil.disk_usage(anchor).free / 1073741824, 1)
            except OSError:
                continue
    return free


def sample(previous, elapsed, trees, with_trees):
    processes, memory, pagefiles = query()
    record = {"t": time.strftime("%H:%M:%S", time.gmtime()),
              "cpu_top": cpu_top(previous, processes, elapsed, os.cpu_count() or 1) if previous else [],
              "memory": memory, "pagefiles": pagefiles, "processes": process_counts(processes),
              "disk_free_gb": disk_free(path for path in trees.values())}
    if with_trees:
        record["trees"] = {name: tree_stats(path) for name, path in trees.items()}
    return processes, record


def parse_trees(items):
    trees = {}
    for item in items or []:
        name, sep, path = item.partition("=")
        if sep and name and path:
            trees[name] = path
    return trees


def loop(args):
    log = Path(args.log)
    stop = log.with_name(log.name + ".stop")
    trees = parse_trees(args.tree)
    previous, last, index = [], time.monotonic(), 0
    while not stop.exists():
        now = time.monotonic()
        try:
            previous, record = sample(previous, now - last, trees, index % max(args.tree_every, 1) == 0)
            with log.open("a", encoding="utf-8") as handle:
                handle.write(json.dumps(record, sort_keys=True) + "\n")
        except Exception as error:
            try:
                with log.open("a", encoding="utf-8") as handle:
                    handle.write(json.dumps({"t": time.strftime("%H:%M:%S", time.gmtime()), "error": str(error)}) + "\n")
            except OSError:
                pass
        last, index = now, index + 1
        deadline = time.monotonic() + args.interval
        while time.monotonic() < deadline and not stop.exists():
            time.sleep(min(1.0, args.interval))
    return 0


def start(args):
    log = Path(args.log)
    log.parent.mkdir(parents=True, exist_ok=True)
    log.with_name(log.name + ".stop").unlink(missing_ok=True)
    command = [sys.executable, str(Path(__file__).resolve()), "run", "--log", str(log),
               "--interval", str(args.interval), "--tree-every", str(args.tree_every)]
    for item in args.tree or []:
        command += ["--tree", item]
    options = {"stdin": subprocess.DEVNULL, "stdout": subprocess.DEVNULL, "stderr": subprocess.DEVNULL,
               "close_fds": True}
    if os.name == "nt":
        detached = subprocess.DETACHED_PROCESS | subprocess.CREATE_NEW_PROCESS_GROUP
        attempts = [detached | 0x01000000, detached]
    else:
        attempts = [None]
    for flags in attempts:
        try:
            if flags is None:
                process = subprocess.Popen(command, start_new_session=True, **options)
            else:
                process = subprocess.Popen(command, creationflags=flags, **options)
            print(f"resource sampler started (pid {process.pid}), every {args.interval} s, log {log}")
            return 0
        except OSError as error:
            print(f"resource sampler start failed: {error}")
    return 0


def summary_line(record):
    if "error" in record:
        return f"{record.get('t', '?')} sample error: {record['error']}"
    memory = record.get("memory") or {}
    parts = [record.get("t", "?")]
    if memory:
        parts.append(f"avail={memory.get('available_mb', '?')}/{memory.get('total_mb', '?')}MB "
                     f"commit={memory.get('commit_mb', '?')}/{memory.get('commit_limit_mb', '?')}MB")
    pagefiles = record.get("pagefiles") or []
    if pagefiles:
        parts.append("pagefile=" + "+".join(str(p.get("used_mb", 0)) for p in pagefiles) + "MB")
    if record.get("disk_free_gb"):
        parts.append("free=" + ",".join(f"{k}{v}G" for k, v in sorted(record["disk_free_gb"].items())))
    processes = record.get("processes") or {}
    parts.append(f"procs={processes.get('total', '?')}")
    if processes.get("watched"):
        parts.append(" ".join(f"{k}={v}" for k, v in processes["watched"].items()))
    if processes.get("orphaned"):
        parts.append("orphaned: " + " ".join(f"{k}={v}" for k, v in processes["orphaned"].items()))
    if record.get("cpu_top"):
        parts.append("cpu% " + " ".join(f"{name}={pct}" for name, pct in record["cpu_top"]))
    if record.get("trees"):
        parts.append("trees " + " ".join(f"{k}={v['mb']}MB/{v['files']}f" for k, v in sorted(record["trees"].items())))
    return " | ".join(parts)


def report(args):
    log = Path(args.log)
    log.with_name(log.name + ".stop").touch()
    if not log.is_file():
        print(f"no resource samples at {log}")
        return 0
    lines = log.read_text(encoding="utf-8", errors="replace").splitlines()
    print(f"resource samples: {len(lines)} ({log})")
    for line in lines:
        try:
            print(summary_line(json.loads(line)))
        except ValueError:
            continue
    return 0


def main(argv=None):
    parser = argparse.ArgumentParser()
    commands = parser.add_subparsers(dest="command", required=True)
    for name, handler in (("start", start), ("run", loop)):
        sub = commands.add_parser(name)
        sub.add_argument("--log", required=True)
        sub.add_argument("--interval", type=float, default=30.0)
        sub.add_argument("--tree", action="append", help="NAME=DIR whose size and file count are sampled")
        sub.add_argument("--tree-every", type=int, default=10)
        sub.set_defaults(handler=handler)
    rep = commands.add_parser("report")
    rep.add_argument("--log", required=True)
    rep.set_defaults(handler=report)
    args = parser.parse_args(argv)
    try:
        return args.handler(args)
    except Exception as error:
        print(f"resource sampler {args.command} failed: {error}")
        return 0


if __name__ == "__main__":
    sys.exit(main())
