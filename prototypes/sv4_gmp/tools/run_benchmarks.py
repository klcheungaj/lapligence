import json
import platform
import statistics
import subprocess
import sys
from pathlib import Path

executable, output = map(Path, sys.argv[1:])
process = subprocess.run([str(executable.resolve())], capture_output=True, text=True,
                         check=True, timeout=300)
records = [json.loads(line) for line in process.stdout.splitlines() if line.strip()]
for record in records:
    if record["kind"] == "timing":
        if "ns" in record:
            record["median_ns"] = statistics.median(record["ns"])
        else:
            record["old_median_ns"] = statistics.median(record["old_ns"])
            record["new_median_ns"] = statistics.median(record["new_ns"])
            record["old_over_new"] = record["old_median_ns"] / record["new_median_ns"]
report = {"system": platform.platform(), "machine": platform.machine(),
          "method": "C clock process CPU time; 5 repeated warm-cache measurements; no LTO; selected benchmark measures only its backend; differential reuse cases compare the new reusable API against legacy fresh-result API",
          "memory_scope": "descriptor and requested persistent payload only; no allocator metadata/RSS or GMP internal scratch",
          "stderr": process.stderr.strip(), "records": records}
output.write_text(json.dumps(report, indent=2) + "\n")
print(f"Wrote {len(records)} records to {output}")
