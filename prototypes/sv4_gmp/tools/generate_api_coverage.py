import argparse
import json
import re
from pathlib import Path

parser = argparse.ArgumentParser(description="Inventory legacy sv4 function names and implemented facade aliases")
parser.add_argument("output", type=Path)
args = parser.parse_args()
root = Path(__file__).resolve().parents[1]
header = (root / "golden/llg_value.h").read_text()
header = re.sub(r"/\*.*?\*/|//[^\n]*", "", header, flags=re.S)
legacy = set(re.findall(r"\b(sv4_[a-zA-Z0-9_]+)\s*\(", header))
facade = (root / "include/sv4.h").read_text()
aliases = dict(re.findall(r"^#define\s+(sv4_\w+)\s+(gmp4_\w+)\s*$", facade, re.M))
records = [{"legacy_name": name, "status": "implemented" if name in aliases else "not_implemented",
            "new_symbol": aliases.get(name)} for name in sorted(legacy)]
report = {"scope": "Legacy sv4_* function-name inventory only; not all llg_* helpers, type declarations or language features",
          "implemented": sum(row["status"] == "implemented" for row in records),
          "total": len(records), "functions": records,
          "separate_prototype_helpers": ["gmp4_get_bit", "gmp4_set_bit", "gmp4_word", "gmp4_compact",
              "gmp4_add_into", "gmp4_mul_into", "gmp4_workspace_destroy", "gmp4_workspace_bytes",
              "gmp4_resolve_wire", "llg_sv4_cell_*", "llg_sv4_wire_resolve"]}
args.output.write_text(json.dumps(report, indent=2) + "\n")
print(f"{report['implemented']} of {report['total']} legacy sv4 function names mapped")
