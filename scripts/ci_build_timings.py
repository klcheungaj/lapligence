#!/usr/bin/env python3
import argparse
import sys
from collections import Counter, defaultdict
from pathlib import Path

PHASES = ("generate", "probe", "runtime", "seed", "configure", "build", "total")
OUTCOMES = ("result", "runtime", "seed", "configure_retry")


def parse(lines):
    records = []
    for line in lines:
        fields = line.rstrip("\r\n").split("\t")
        if not fields or fields[0] != "llg-build":
            continue
        record = {}
        for field in fields[1:]:
            name, sep, value = field.partition("=")
            if sep:
                record[name] = value
        records.append(record)
    return records


def percentile(values, fraction):
    ordered = sorted(values)
    if not ordered:
        return 0
    index = min(len(ordered) - 1, int(round(fraction * (len(ordered) - 1))))
    return ordered[index]


def phase_rows(records):
    rows = []
    for phase in PHASES:
        values = [int(r[phase + "_ms"]) for r in records if (r.get(phase + "_ms") or "").isdigit()]
        if values:
            rows.append((phase, len(values), sum(values), percentile(values, 0.5), percentile(values, 0.9), max(values)))
    return rows


def summarize(records):
    out = [f"model builds: {len(records)}"]
    for outcome in OUTCOMES:
        counts = Counter(r.get(outcome, "-") for r in records)
        out.append(f"{outcome}: " + ", ".join(f"{k}={v}" for k, v in sorted(counts.items())))
    out.append("phase       count    sum_s  median_ms   p90_ms   max_ms")
    for phase, count, total, median, p90, peak in phase_rows(records):
        out.append(f"{phase:<10} {count:>6} {total / 1000:>8.1f} {median:>10} {p90:>8} {peak:>8}")
    groups = defaultdict(list)
    for r in records:
        groups[r.get("seed", "-")].append(r)
    for seed, group in sorted(groups.items()):
        rows = {row[0]: row for row in phase_rows(group)}
        configure = rows.get("configure")
        total = rows.get("total")
        if configure and total:
            out.append(f"seed={seed}: builds={len(group)} configure median={configure[3]}ms total median={total[3]}ms")
    return "\n".join(out)


def main(argv=None):
    parser = argparse.ArgumentParser()
    parser.add_argument("file", type=Path)
    args = parser.parse_args(argv)
    if not args.file.is_file():
        print(f"no model-build timings at {args.file}")
        return 0
    text = args.file.read_text(encoding="utf-8", errors="replace")
    print(summarize(parse(text.splitlines())))
    return 0


if __name__ == "__main__":
    sys.exit(main())
