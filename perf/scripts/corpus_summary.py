#!/usr/bin/env python3

import argparse
import csv
import statistics
from collections import defaultdict
from pathlib import Path


NUMERIC_FIELDS = (
    "generation_ms",
    "generation_rss_kib",
    "build_ms",
    "build_rss_kib",
    "simulation_ms",
    "simulation_rss_kib",
    "executable_bytes",
)


def summarize(rows):
    grouped = defaultdict(list)
    for row in rows:
        grouped[(row["config"], row["mode"])].append(row)

    summaries = []
    for (config, mode), group in sorted(grouped.items()):
        hashes = {row["stdout_sha256"] for row in group}
        summary = {
            "config": config,
            "mode": mode,
            "runs": len(group),
            "all_status_zero": int(
                all(
                    row[field] == "0"
                    for row in group
                    for field in (
                        "generation_status",
                        "build_status",
                        "simulation_status",
                    )
                )
            ),
        }
        for field in NUMERIC_FIELDS:
            summary[f"median_{field}"] = statistics.median(
                float(row[field]) for row in group
            )
        summary["stdout_sha256"] = hashes.pop() if len(hashes) == 1 else "MIXED"
        summaries.append(summary)
    return summaries


def format_number(value):
    if float(value).is_integer():
        return str(int(value))
    return f"{value:.3f}"


def write_summary(input_path, output_path):
    with input_path.open(newline="", encoding="utf-8") as source:
        rows = list(csv.DictReader(source, delimiter="\t"))
    summaries = summarize(rows)
    fields = [
        "config",
        "mode",
        "runs",
        "all_status_zero",
        *(f"median_{field}" for field in NUMERIC_FIELDS),
        "stdout_sha256",
    ]
    with output_path.open("w", newline="", encoding="utf-8") as destination:
        writer = csv.DictWriter(destination, fieldnames=fields, delimiter="\t")
        writer.writeheader()
        for summary in summaries:
            writer.writerow(
                {
                    field: format_number(value)
                    if field.startswith("median_")
                    else value
                    for field, value in summary.items()
                }
            )


def main():
    parser = argparse.ArgumentParser(
        description="write per-configuration medians from corpus results.tsv"
    )
    parser.add_argument("input", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    write_summary(args.input, args.output)


if __name__ == "__main__":
    main()
