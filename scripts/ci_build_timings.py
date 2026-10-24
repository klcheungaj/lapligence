#!/usr/bin/env python3
import argparse
import json
import sys
from collections import Counter, defaultdict
from pathlib import Path

PHASES = ("generate", "gen_check", "gen_write", "gen_prune", "probe", "runtime", "seed", "template", "configure",
          "build", "total")
OUTCOMES = ("result", "runtime", "seed", "template", "configure_retry", "gen_tree")
SLOWEST_GENERATE = 10


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
    templates = defaultdict(list)
    for r in records:
        if "template" in r:
            templates[r["template"]].append(r)
    for template, group in sorted(templates.items()):
        rows = {row[0]: row for row in phase_rows(group)}
        total = rows.get("total")
        if total:
            configure = rows.get("configure")
            configure_text = f"{configure[3]}ms" if configure else "none"
            out.append(f"template={template}: builds={len(group)} configure median={configure_text} "
                       f"total median={total[3]}ms")
    reasons = Counter(r["template_reason"] for r in records if r.get("template_reason"))
    for reason, count in sorted(reasons.items()):
        out.append(f"template rejected ({count}): {reason}")
    out.extend(slowest_generate(records))
    return "\n".join(out)


def number(record, name):
    value = record.get(name) or ""
    return int(value) if value.isdigit() else None


def slowest_generate(records, count=SLOWEST_GENERATE):
    timed = [r for r in records if number(r, "generate_ms") is not None]
    if not timed:
        return []
    stamps = [number(r, "end_unix_ms") for r in records if number(r, "end_unix_ms") is not None]
    start = min(stamps) if stamps else None
    out = [f"slowest generate phases (of {len(timed)}):",
           "  gen_ms  check  write  prune  files  bytes  model_bytes  tree      end  dir"]
    for r in sorted(timed, key=lambda r: number(r, "generate_ms"), reverse=True)[:count]:
        def cell(name):
            value = number(r, name)
            return "-" if value is None else str(value)
        end = number(r, "end_unix_ms")
        offset = f"+{(end - start) // 60000}m" if end is not None and start is not None else "-"
        out.append(f"{cell('generate_ms'):>8} {cell('gen_check_ms'):>6} {cell('gen_write_ms'):>6} "
                   f"{cell('gen_prune_ms'):>6} {cell('gen_files'):>6} {cell('gen_bytes'):>6} "
                   f"{cell('gen_model_bytes'):>12}  {r.get('gen_tree', '-'):<8} {offset:>4}  {r.get('dir', '')}")
    return out


WINDOW_PHASES = ("generate", "runtime", "seed", "configure", "build", "total")


def windows(records, minutes=15):
    stamped = [(int(r["end_unix_ms"]), r) for r in records if (r.get("end_unix_ms") or "").isdigit()]
    if not stamped:
        return ""
    start = min(stamp for stamp, _ in stamped)
    width = max(1, minutes) * 60000
    buckets = defaultdict(list)
    for stamp, record in stamped:
        buckets[(stamp - start) // width].append(record)
    out = [f"phase medians (ms) per {minutes}-minute window of build end time:",
           "window  builds " + " ".join(f"{phase:>9}" for phase in WINDOW_PHASES)]
    for index in sorted(buckets):
        group = buckets[index]
        medians = []
        for phase in WINDOW_PHASES:
            values = [int(r[phase + "_ms"]) for r in group if (r.get(phase + "_ms") or "").isdigit()]
            medians.append(f"{percentile(values, 0.5):>9}" if values else f"{'-':>9}")
        label = f"+{index * minutes}m"
        out.append(f"{label:>6} {len(group):>7} " + " ".join(medians))
    return "\n".join(out)


def short_location(location):
    path, sep, line = location.rpartition(":")
    if not sep or not line.isdigit():
        path, line = location, ""
    parts = [part for part in path.replace("\\", "/").split("/") if part]
    short = "/".join(parts[-2:])
    return f"{short}:{line}" if line else short


def profile_spans(events):
    spans = []
    stack = []
    for event in events:
        if not isinstance(event, dict):
            continue
        phase = event.get("ph")
        if phase == "B":
            stack.append([event, 0])
        elif phase == "E" and stack:
            begin, children = stack.pop()
            duration = max(0, int(event.get("ts", 0)) - int(begin.get("ts", 0)))
            location = (begin.get("args") or {}).get("location", "")
            spans.append((begin.get("cat", ""), begin.get("name", ""), short_location(location),
                          duration, max(0, duration - children)))
            if stack:
                stack[-1][1] += duration
    return spans


def summarize_profiles(profiles, top=15):
    project = defaultdict(list)
    commands = defaultdict(lambda: [0, 0])
    for events in profiles:
        for category, name, location, duration, own in profile_spans(events):
            if category == "project":
                project[name].append(duration // 1000)
            else:
                entry = commands[(name, location)]
                entry[0] += own
                entry[1] += 1
    out = [f"cmake configure profiles: {len(profiles)}"]
    for name, values in sorted(project.items()):
        out.append(f"{name}: median={percentile(values, 0.5)}ms p90={percentile(values, 0.9)}ms "
                   f"max={max(values)}ms")
    out.append("  self_ms  count  avg_ms  command  location")
    ranked = sorted(commands.items(), key=lambda item: item[1][0], reverse=True)[:top]
    for (name, location), (own, count) in ranked:
        out.append(f"{own / 1000:>9.1f} {count:>6} {own / 1000 / count:>7.1f}  {name}  {location}")
    return "\n".join(out)


def load_profiles(directory):
    profiles, unreadable = [], 0
    for path in sorted(Path(directory).glob("*.json")):
        try:
            events = json.loads(path.read_text(encoding="utf-8", errors="replace"))
        except (OSError, ValueError):
            unreadable += 1
            continue
        if isinstance(events, list):
            profiles.append(events)
        else:
            unreadable += 1
    return profiles, unreadable


def main(argv=None):
    parser = argparse.ArgumentParser()
    parser.add_argument("file", type=Path)
    parser.add_argument("--cmake-profiles", type=Path,
                        help="directory of LLG_CMAKE_PROFILE_DIR google-trace files to summarize")
    parser.add_argument("--window-minutes", type=int, default=15,
                        help="width of the time windows of the per-window phase medians")
    args = parser.parse_args(argv)
    if not args.file.is_file():
        print(f"no model-build timings at {args.file}")
    else:
        text = args.file.read_text(encoding="utf-8", errors="replace")
        records = parse(text.splitlines())
        print(summarize(records))
        windowed = windows(records, args.window_minutes)
        if windowed:
            print(windowed)
    if args.cmake_profiles:
        profiles, unreadable = load_profiles(args.cmake_profiles)
        print(summarize_profiles(profiles))
        if unreadable:
            print(f"unreadable profiles: {unreadable}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
