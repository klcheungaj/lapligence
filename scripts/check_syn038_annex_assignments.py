from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import importlib.util
import json
from pathlib import Path
import re


ROOT = Path(__file__).resolve().parents[1]
SOURCE_LABELS = ("V2001_PDF", "SV2009_PDF", "ADDENDUM")
DISPOSITIONS = {
    "CORE", "BOUNDARY", "EXTENDED", "OUTSIDE_CORE_IMPLEMENTED",
    "EXCLUDED", "ALIAS/HELPER", "OPEN",
}
SELECTED_UDP = {
    "combinational_body", "combinational_entry", "input_value",
    "level_input_list", "level_symbol", "list_of_udp_port_identifiers",
    "name_of_udp_instance", "output_symbol", "output_value",
    "udp_ansi_declaration", "udp_body", "udp_declaration",
    "udp_declaration_port_list", "udp_identifier", "udp_input_declaration",
    "udp_instance", "udp_instance_identifier", "udp_instantiation",
    "udp_nonansi_declaration", "udp_output_declaration",
    "udp_port_declaration", "udp_port_list",
}
UNSUPPORTED_UDP = {
    "current_state", "edge_indicator", "edge_input_list", "edge_symbol",
    "init_val", "next_state", "seq_input_list", "sequential_body",
    "sequential_entry", "udp_initial_statement", "udp_reg_declaration",
}
SELECTED_TAGGED_PATTERN = {
    "tagged_union_expression": "SYN-024",
    "cond_pattern": "SYN-024",
    "expression_or_cond_pattern": "SYN-024",
    "pattern": "SYN-024",
    "case_pattern_item": "SYN-025",
}
SELECTED_METHOD = {
    "array_manipulation_call", "array_method_call", "array_method_name",
    "built_in_method_call", "method_call", "method_call_body",
    "method_call_root",
}
OUTSIDE_IMPLEMENTED = {
    "event_declaration", "event_identifier", "list_of_event_identifiers",
    "event_trigger", "hierarchical_event_identifier",
    "procedural_continuous_assignment",
    "procedural_continuous_assignments",
    "procedural_continuous_assignment(s)",
    "virtual_interface_declaration", "list_of_virtual_interface_decl",
    "class_declaration", "program_declaration", "clocking_declaration",
    "assertion_item", "dynamic_array_variable_identifier",
    "dynamic_array_new", "associative_dimension", "queue_dimension",
}


def fail(message: str) -> None:
    raise SystemExit(f"SYN-038 Annex assignment error: {message}")


def load_inventory_module():
    spec = importlib.util.spec_from_file_location("syn038_annex_inventory", ROOT / "scripts/syn038_annex_inventory.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def load_reference_names(path: Path) -> dict:
    if not path.is_file():
        fail(f"required frozen reference-name snapshot is missing: {path}")
    return json.loads(path.read_text())


def ledger_rows(ledger: str) -> dict[str, list[str]]:
    selected = ledger.split("#### SYN-038 audited evidence map", 1)[0]
    rows = {}
    for line in selected.splitlines():
        if not line.startswith("| SYN038-CORE-"):
            continue
        cells = [cell.strip() for cell in line.strip("|").split("|")]
        if len(cells) != 7:
            fail(f"malformed Core row: {line}")
        if cells[0] in rows:
            fail(f"duplicate Core row {cells[0]}")
        rows[cells[0]] = cells
    return rows


def check_row(name: str, row_id: str, rows: dict[str, list[str]], disposition: str) -> None:
    if row_id not in rows:
        fail(f"{name} points to nonexistent ledger row {row_id}")
    row = rows[row_id]
    if row[5] not in {"PASS", "REJECT"}:
        fail(f"{name} points to {row_id} without a PASS/REJECT oracle")
    if disposition == "CORE" and row[5] != "PASS":
        fail(f"{name} counts rejected source {row_id} as positive Core")
    if name not in re.findall(r"`([a-z][a-z_0-9]*)`", row[2]):
        fail(f"{name} points to {row_id} without a named production witness")
    fixtures = re.findall(r"`(tests/fixtures/sim/[^`]+)`", row[4])
    if not fixtures or any(not (ROOT / fixture).is_file() for fixture in fixtures):
        fail(f"{name} points to {row_id} without real fixtures")
    if "tests/" not in row[6] or "::" not in row[6]:
        fail(f"{name} points to {row_id} without a named owner")


def check_evidence(name: str, evidence: object, disposition: str, ledger: str, product_docs: str) -> None:
    if not isinstance(evidence, dict):
        fail(f"{name} lacks {disposition} evidence")
    owner = evidence.get("owner", "")
    if not isinstance(owner, str) or "::" not in owner:
        fail(f"{name} lacks a named evidence owner")
    file, function = owner.split("::", 1)
    source_path = ROOT / file
    if not file.startswith("tests/") or not source_path.is_file():
        fail(f"{name} has no real evidence test file")
    source = source_path.read_text()
    marker = f"fn {function}("
    if marker in source:
        body = source.split(marker, 1)[1].split("\n#[test]", 1)[0]
    else:
        invocation = re.search(rf"datatype_case!\(\s*{re.escape(function)},([\s\S]*?)\);", source)
        if invocation is None:
            fail(f"{name} has no real evidence test function {owner}")
        body = invocation.group(0)
    fixture = evidence.get("fixture")
    if fixture is not None:
        if not isinstance(fixture, str) or not fixture.startswith("tests/fixtures/sim/") or not (ROOT / fixture).is_file():
            fail(f"{name} has no real evidence fixture")
        if Path(fixture).stem not in body:
            fail(f"{name} evidence owner does not invoke {fixture}")
    elif disposition == "EXTENDED" or evidence.get("in_memory_source") is not True:
        fail(f"{name} has no fixture or in-memory source")
    elif "event " not in body:
        fail(f"{name} does not exercise the recorded in-memory event source")
    if disposition == "EXTENDED":
        task = evidence.get("task")
        if not isinstance(task, str) or not re.fullmatch(r"SYN-\d+", task):
            fail(f"{name} has no owning Extended task")
        link = next((line for line in ledger.splitlines() if line.startswith(f"| SYN038-LINK-{task} |")), "")
        if not link or not link.split("|")[3].strip().startswith("PASS"):
            fail(f"{name} has no positive {task} ledger link")
        if not evidence.get("scope"):
            fail(f"{name} has no bounded Extended scope")
    else:
        heading = evidence.get("docs_heading", "")
        if not isinstance(heading, str) or heading not in product_docs:
            fail(f"{name} lacks its product documentation row")
        profile = evidence.get("profile", "")
        if not isinstance(profile, str) or not ("outside Core" in profile or "SYN-034" in profile):
            fail(f"{name} has no outside-Core profile reason")


def check_manifest(manifest: dict, ledger: str, inventory, pdf_root: Path | None,
                   addendum_path: Path | None = None) -> Counter:
    if manifest.get("schema") != "syn038-annex-assignments/v2":
        fail("unknown manifest schema")
    source_names = manifest.get("source_names")
    if not isinstance(source_names, dict) or set(source_names) != set(SOURCE_LABELS):
        fail("missing source-name snapshots")
    for label in SOURCE_LABELS:
        names = source_names[label]
        if not isinstance(names, list) or names != sorted(set(names)):
            fail(f"{label} names are not sorted and unique")

    snapshot = load_reference_names(ROOT / "tests/syn038_annex_reference_names.json")
    if snapshot.get("schema") != "syn038-annex-reference-names/v1":
        fail("unknown reference-name snapshot schema")
    sections = snapshot.get("sections_by_name")
    if not isinstance(sections, dict) or any(
        not isinstance(name, str) or not isinstance(ids, list) or
        ids != sorted(set(ids)) or not ids or
        any(not re.fullmatch(r"B\.\d+", section) for section in ids)
        for name, ids in sections.items()
    ):
        fail("reference-name snapshot has invalid sections")
    product_docs = (ROOT / "docs/sim_features.md").read_text()
    addendum_names = set(sections)
    if addendum_names != set(source_names["ADDENDUM"]):
        fail("reference-name snapshot differs from assignment source names")
    if addendum_path is not None:
        if not addendum_path.is_file():
            fail(f"optional reference addendum is missing: {addendum_path}")
        content = addendum_path.read_bytes()
        if hashlib.sha256(content).hexdigest() != snapshot.get("source_sha256"):
            fail("reference addendum source hash differs from frozen snapshot")
        observed = {}
        for section, name, *_ in inventory.inventory(content.decode(), ledger):
            observed.setdefault(name, set()).add(section)
        if {name: sorted(ids) for name, ids in observed.items()} != sections:
            fail("reference addendum names or sections differ from frozen snapshot")
    if pdf_root is not None:
        for label, filename, first, last in (
            ("V2001_PDF", "Verilog-1364-2001.pdf", 783, 809),
            ("SV2009_PDF", "SystemVerilog-1800-2009.pdf", 1095, 1144),
        ):
            path = pdf_root / filename
            if not path.is_file():
                fail(f"optional Annex PDF is missing: {path}")
            names = inventory.pdf_productions(path, first, last)
            if names != set(source_names[label]):
                fail(f"{label} PDF extraction differs from the checked-in snapshot")

    source_union = set().union(*(set(source_names[label]) for label in SOURCE_LABELS))
    items = manifest.get("assignments")
    if not isinstance(items, list):
        fail("assignments must be a list")
    assigned = {}
    rows = ledger_rows(ledger)
    counts = Counter()
    for item in items:
        if not isinstance(item, dict):
            fail("assignment is not an object")
        name = item.get("production")
        if not isinstance(name, str) or name in assigned:
            fail(f"duplicate or invalid production {name!r}")
        assigned[name] = item
        disposition = item.get("disposition")
        if disposition not in DISPOSITIONS:
            fail(f"{name} lacks exactly one recognized disposition")
        expected_task = None
        if name in SELECTED_UDP:
            expected_task = "SYN-031"
        elif "B.27" in item.get("reference_families", []) or name == "include_statement":
            expected_task = "SYN-032"
        elif name.startswith("bind_"):
            expected_task = "SYN-033"
        elif name in SELECTED_TAGGED_PATTERN:
            expected_task = SELECTED_TAGGED_PATTERN[name]
        elif name in SELECTED_METHOD:
            expected_task = "SYN-026"
        elif name == "load_memory_tasks":
            expected_task = "SYN-029"
        elif name == "writemem_tasks":
            expected_task = "SYN-030"
        if expected_task and (disposition != "EXTENDED" or item.get("evidence", {}).get("task") != expected_task):
            fail(f"{name} must map to selected Extended {expected_task} evidence")
        if name in UNSUPPORTED_UDP and disposition != "EXCLUDED":
            fail(f"{name} must remain outside selected combinational UDP")
        if name == "zero_or_one" and disposition != "EXCLUDED":
            fail("zero_or_one is an Annex A.7.5.3 timing descriptor, not a UDP table symbol")
        if name in OUTSIDE_IMPLEMENTED and disposition != "OUTSIDE_CORE_IMPLEMENTED":
            fail(f"{name} must retain its implemented outside-Core evidence")
        counts[disposition] += 1
        sources = sorted(label for label in SOURCE_LABELS if name in source_names[label])
        if item.get("sources") != sources:
            fail(f"{name} source reconciliation differs from the snapshots")
        editions = item.get("editions")
        if not isinstance(editions, list) or not editions or len(editions) != len(set(editions)) or not set(editions) <= {"V2001", "SV2009"}:
            fail(f"{name} has no precise edition field")
        for label, edition in (("V2001_PDF", "V2001"), ("SV2009_PDF", "SV2009")):
            if label in sources and edition not in editions:
                fail(f"{name} omits its {edition} PDF edition")
        if "ADDENDUM" in sources and not item.get("reference_families"):
            fail(f"{name} has no addendum B family")
        if "ADDENDUM" in sources and set(item.get("reference_families", [])) != set(sections[name]):
            fail(f"{name} addendum section IDs differ from frozen snapshot")
        if any(label.endswith("_PDF") for label in sources) and not item.get("annex_sections"):
            fail(f"{name} has no Annex A section")
        if sources == ["ADDENDUM"] and "edition_basis" not in item:
            fail(f"{name} reference-only edition is not identified as an inference")
        if disposition in {"CORE", "BOUNDARY"}:
            row_ids = item.get("ledger_rows")
            if not isinstance(row_ids, list) or not row_ids or row_ids != sorted(set(row_ids)):
                fail(f"{name} has no stable ledger row assignment")
            for row_id in row_ids:
                check_row(name, row_id, rows, disposition)
            if not any(set(editions) & set(rows[row_id][1].split("/")) for row_id in row_ids):
                fail(f"{name} has no witness in any recorded source edition")
        elif disposition in {"EXTENDED", "OUTSIDE_CORE_IMPLEMENTED"}:
            check_evidence(name, item.get("evidence"), disposition, ledger, product_docs)
        elif disposition == "EXCLUDED":
            reason = item.get("reason", "")
            if not re.search(r"SYN-\d+|SYN038-EX-\d+|Core|host|VCD|SDF|simulator|verification|foreign|dynamic", reason, re.IGNORECASE):
                fail(f"{name} has no named profile/exclusion reason")
        elif disposition == "ALIAS/HELPER":
            parents = item.get("parents")
            if not isinstance(parents, list) or not parents or parents != sorted(set(parents)):
                fail(f"{name} has no unique, ordered parent production")
        elif disposition == "OPEN":
            child = item.get("child", "")
            if not re.fullmatch(r"SYN-038-N\d+", child) or child not in ledger:
                fail(f"{name} has no scoped child recorded in the ledger")

    if set(assigned) != source_union:
        missing = sorted(source_union - set(assigned))
        extra = sorted(set(assigned) - source_union)
        fail(f"unassigned extracted names {missing[:8]}, extra names {extra[:8]}")

    visited = set()

    def visit(name: str, path: set[str]) -> None:
        if name in path:
            fail(f"cyclic ALIAS/HELPER assignment through {name}")
        if name in visited:
            return
        item = assigned.get(name)
        if item is None:
            fail(f"ALIAS/HELPER parent {name} is unassigned")
        if item["disposition"] == "ALIAS/HELPER":
            for parent in item["parents"]:
                visit(parent, path | {name})
        elif item["disposition"] == "OPEN":
            fail(f"ALIAS/HELPER chain terminates in OPEN {name}")
        visited.add(name)

    for name in assigned:
        visit(name, set())

    extraction = manifest.get("extraction", {})
    pdf_union = set(source_names["V2001_PDF"]) | set(source_names["SV2009_PDF"])
    expected = {
        "pdf_distinct": len(pdf_union),
        "addendum_distinct": len(addendum_names),
        "pdf_only": len(pdf_union - addendum_names),
        "addendum_only": len(addendum_names - pdf_union),
        "union_distinct": len(source_union),
    }
    if extraction != expected:
        fail(f"source reconciliation metadata changed: expected {expected}")
    return counts


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--pdf-root", type=Path)
    parser.add_argument("--reference-addendum", type=Path)
    args = parser.parse_args()
    manifest = json.loads((ROOT / "tests/syn038_annex_assignments.json").read_text())
    ledger = (ROOT / "tests/syn038_coverage_ledger.md").read_text()
    counts = check_manifest(manifest, ledger, load_inventory_module(), args.pdf_root,
                            args.reference_addendum)
    print(f"SYN-038 Annex assignments: {sum(counts.values())} names, " + ", ".join(f"{name}={counts[name]}" for name in sorted(counts)))


if __name__ == "__main__":
    main()
