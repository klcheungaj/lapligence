#!/usr/bin/env python3

import argparse
import subprocess
import sys
import time
from pathlib import Path


def parse_smaps(text):
    mappings = []
    current = None
    for line in text.splitlines():
        first = line.split(maxsplit=1)[0] if line else ""
        if "-" in first and first.replace("-", "", 1).isalnum():
            fields = line.split(maxsplit=5)
            start, end = fields[0].split("-", 1)
            current = {
                "start": int(start, 16),
                "end": int(end, 16),
                "perms": fields[1],
                "path": fields[5] if len(fields) == 6 else "",
            }
            mappings.append(current)
        elif current is not None and ":" in line:
            key, value = line.split(":", 1)
            fields = value.split()
            if fields and fields[0].isdigit():
                current[key] = int(fields[0])
    return mappings


def classify_mapping(mapping):
    path = mapping["path"]
    perms = mapping["perms"]
    size = mapping.get("Size", (mapping["end"] - mapping["start"]) // 1024)
    rss = mapping.get("Rss", 0)
    anonymous = path == "" or path.startswith("[anon:")
    private_writable = len(perms) >= 4 and perms[1] == "w" and perms[3] == "p"

    if path == "[heap]":
        return "heap"
    if "x" in perms or (path.startswith("/") and not private_writable):
        return "binary_text"
    if anonymous and private_writable:
        sparse = rss * 5 <= max(size, 1)
        near_save_size = any(
            abs(size - expected) <= 128 for expected in (256, 1024, 1028, 1032)
        )
        if (size >= 262144 and sparse) or (near_save_size and rss <= 128):
            return "libaco_save_stacks"
        if 3072 <= size <= 65536:
            return "shared_stack"
        return "heap"
    return "other"


def summarize(mappings):
    totals = {}
    for mapping in mappings:
        category = classify_mapping(mapping)
        total = totals.setdefault(
            category,
            {"mappings": 0, "size_kib": 0, "rss_kib": 0, "pss_kib": 0, "private_dirty_kib": 0},
        )
        total["mappings"] += 1
        total["size_kib"] += mapping.get("Size", 0)
        total["rss_kib"] += mapping.get("Rss", 0)
        total["pss_kib"] += mapping.get("Pss", 0)
        total["private_dirty_kib"] += mapping.get("Private_Dirty", 0)
    return totals


def read_rss(pid):
    try:
        status = Path(f"/proc/{pid}/status").read_text()
    except OSError:
        return None
    for line in status.splitlines():
        if line.startswith("VmRSS:"):
            return int(line.split()[1])
    return 0


def snapshot(pid):
    return Path(f"/proc/{pid}/smaps").read_text(encoding="utf-8", errors="replace")


def monitor(pid, interval, is_running):
    peak_rss = -1
    peak_smaps = ""
    while is_running():
        rss = read_rss(pid)
        if rss is not None and rss > peak_rss:
            try:
                candidate = snapshot(pid)
            except OSError:
                candidate = ""
            if candidate:
                peak_rss = rss
                peak_smaps = candidate
        time.sleep(interval)
    return max(peak_rss, 0), peak_smaps


def write_report(path, pid, peak_rss, smaps):
    mappings = parse_smaps(smaps)
    totals = summarize(mappings)
    with open(path, "w", encoding="utf-8") as stream:
        stream.write(f"pid\t{pid}\n")
        stream.write(f"sampled_peak_rss_kib\t{peak_rss}\n")
        stream.write("category\tmappings\tvirtual_kib\trss_kib\tpss_kib\tprivate_dirty_kib\n")
        for category in (
            "libaco_save_stacks",
            "heap",
            "shared_stack",
            "binary_text",
            "other",
        ):
            total = totals.get(category, {})
            stream.write(
                f"{category}\t{total.get('mappings', 0)}\t{total.get('size_kib', 0)}\t"
                f"{total.get('rss_kib', 0)}\t{total.get('pss_kib', 0)}\t"
                f"{total.get('private_dirty_kib', 0)}\n"
            )
        stream.write("\n# mapping detail\n")
        stream.write("category\taddress\tperms\tvirtual_kib\trss_kib\tpath\n")
        for mapping in mappings:
            stream.write(
                f"{classify_mapping(mapping)}\t{mapping['start']:x}-{mapping['end']:x}\t"
                f"{mapping['perms']}\t{mapping.get('Size', 0)}\t{mapping.get('Rss', 0)}\t"
                f"{mapping['path']}\n"
            )


def main():
    parser = argparse.ArgumentParser(description="Snapshot and classify Linux smaps near peak RSS")
    parser.add_argument("--output", required=True)
    parser.add_argument("--interval-ms", type=int, default=20)
    parser.add_argument("--pid", type=int, help="attach to an existing process")
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    if args.interval_ms < 1:
        parser.error("--interval-ms must be positive")
    interval = args.interval_ms / 1000.0

    child = None
    if args.pid is not None:
        if args.command:
            parser.error("a command cannot be combined with --pid")
        pid = args.pid
        is_running = lambda: Path(f"/proc/{pid}").exists()
    else:
        command = args.command
        if command and command[0] == "--":
            command = command[1:]
        if not command:
            parser.error("provide --pid or a command after --")
        child = subprocess.Popen(command)
        pid = child.pid
        is_running = lambda: child.poll() is None

    peak_rss, smaps = monitor(pid, interval, is_running)
    status = child.wait() if child is not None else 0
    if not smaps:
        print("rss_attribution: no readable smaps snapshot", file=sys.stderr)
        return 1 if status == 0 else status
    write_report(args.output, pid, peak_rss, smaps)
    print(f"rss_attribution: peak={peak_rss} KiB report={args.output}")
    return status


if __name__ == "__main__":
    sys.exit(main())
