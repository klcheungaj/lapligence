from __future__ import annotations

import argparse
import csv
from pathlib import Path
import re
import sys


SECTION = re.compile(r"^\| \*\*B\.(\d+) ")
PRODUCTION = re.compile(r"`([a-z][a-z_0-9]*(?:\([^`]*\))?)`")
PDF_PRODUCTION = re.compile(r"\b([a-z][a-z_0-9]*(?:,\d+)*)\s*::=")


def table_rows(document: str, prefix: str) -> list[list[str]]:
    return [
        [cell.strip() for cell in line.strip("|").split("|")]
        for line in document.splitlines()
        if line.startswith(prefix)
    ]


def direct_owners(rows: list[list[str]], column: int) -> dict[str, set[str]]:
    owners: dict[str, set[str]] = {}
    for row in rows:
        for name in PRODUCTION.findall(row[column]):
            owners.setdefault(name, set()).add(row[0])
    return owners


def inventory(source: str, ledger: str) -> list[tuple[str, str, str, str, str]]:
    checklist = source.split("## Section B — Annex A production checklist", 1)[1]
    selected = table_rows(ledger, "| SYN038-CORE-")
    direct_core = direct_owners([row for row in selected if row[5] == "PASS" and row[0] != "SYN038-CORE-PI-03"], 2)
    direct_boundary = direct_owners([row for row in selected if row[5] != "PASS" or row[0] == "SYN038-CORE-PI-03"], 2)
    direct_excluded = direct_owners(table_rows(ledger, "| SYN038-EX-"), 2)
    result = []
    section = ""
    seen = set()
    for line in checklist.splitlines():
        match = SECTION.match(line)
        if match:
            section = f"B.{match[1]}"
            continue
        if not section or not line.startswith("| `"):
            continue
        for name in PRODUCTION.findall(line.split("|", 2)[1]):
            key = (section, name)
            if key in seen:
                continue
            seen.add(key)
            result.append((
                section,
                name,
                ",".join(sorted(direct_core.get(name, ()))),
                ",".join(sorted(direct_boundary.get(name, ()))),
                ",".join(sorted(direct_excluded.get(name, ()))),
            ))
    return result


def pdf_productions(path: Path, first_page: int, last_page: int) -> set[str]:
    from pypdf import PdfReader

    reader = PdfReader(path)
    annex = "\n".join(reader.pages[index].extract_text() or "" for index in range(first_page, last_page + 1))
    annex = annex.split("Annex B", 1)[0]
    names = PDF_PRODUCTION.findall(annex)
    if len(names) != annex.count("::="):
        raise ValueError(f"unrecognized Annex A production left-hand side in {path}")
    return {re.sub(r"\d+(?:,\d+)*$", "", name) for name in names}


def pdf_inventory(root: Path, ledger: str) -> list[tuple[str, str, str, str, str]]:
    v2001 = pdf_productions(root / "Verilog-1364-2001.pdf", 783, 809)
    sv2009 = pdf_productions(root / "SystemVerilog-1800-2009.pdf", 1095, 1144)
    selected = table_rows(ledger, "| SYN038-CORE-")
    direct_core = direct_owners([row for row in selected if row[5] == "PASS" and row[0] != "SYN038-CORE-PI-03"], 2)
    direct_boundary = direct_owners([row for row in selected if row[5] != "PASS" or row[0] == "SYN038-CORE-PI-03"], 2)
    direct_excluded = direct_owners(table_rows(ledger, "| SYN038-EX-"), 2)
    return [
        (
            "V2001/SV2009" if name in v2001 and name in sv2009 else "V2001" if name in v2001 else "SV2009",
            name,
            ",".join(sorted(direct_core.get(name, ()))),
            ",".join(sorted(direct_boundary.get(name, ()))),
            ",".join(sorted(direct_excluded.get(name, ()))),
        )
        for name in sorted(v2001 | sv2009)
    ]


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, default=Path("docs/specification/spec-reference-annex-a.md"))
    parser.add_argument("--ledger", type=Path, default=Path("tests/syn038_coverage_ledger.md"))
    parser.add_argument("--pdf-root", type=Path)
    args = parser.parse_args()
    rows = pdf_inventory(args.pdf_root, args.ledger.read_text()) if args.pdf_root else inventory(args.source.read_text(), args.ledger.read_text())
    writer = csv.writer(sys.stdout, delimiter="\t", lineterminator="\n")
    writer.writerow(("edition_or_section", "production", "core_rows", "boundary_rows", "exclusion_rows"))
    writer.writerows(rows)


if __name__ == "__main__":
    main()
