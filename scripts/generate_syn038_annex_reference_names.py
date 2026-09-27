import hashlib
import json
from pathlib import Path

from syn038_annex_inventory import inventory


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "docs/specification/spec-reference-annex-a.md"
LEDGER = ROOT / "tests/syn038_coverage_ledger.md"
OUTPUT = ROOT / "tests/syn038_annex_reference_names.json"


def main() -> None:
    content = SOURCE.read_bytes()
    names = {}
    for section, name, *_ in inventory(content.decode(), LEDGER.read_text()):
        names.setdefault(name, set()).add(section)
    snapshot = {
        "schema": "syn038-annex-reference-names/v1",
        "source": "docs/specification/spec-reference-annex-a.md",
        "source_sha256": hashlib.sha256(content).hexdigest(),
        "generator": "python3 scripts/generate_syn038_annex_reference_names.py",
        "sections_by_name": {name: sorted(sections) for name, sections in sorted(names.items())},
    }
    OUTPUT.write_text(json.dumps(snapshot, indent=2) + "\n")


if __name__ == "__main__":
    main()
