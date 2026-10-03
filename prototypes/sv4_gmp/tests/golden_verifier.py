import json
import subprocess
import sys
import tempfile
from pathlib import Path

root = Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix="sv4-golden-test-") as temporary:
    directory = Path(temporary)
    prototype = directory / "prototype"
    repository = directory / "repository"
    prototype.mkdir()
    repository.mkdir()
    entry = json.loads((root / "golden_manifest.json").read_text())["files"][0]
    (prototype / "golden_manifest.json").write_text(json.dumps({"files": [entry]}))
    snapshot = prototype / entry["snapshot"]
    production = repository / entry["production"]
    snapshot.parent.mkdir(parents=True)
    production.parent.mkdir(parents=True)
    snapshot.write_bytes((root / entry["snapshot"]).read_bytes())
    production.write_bytes(snapshot.read_bytes())
    output = directory / "report.json"
    command = [sys.executable, str(root / "tools/verify_golden.py"),
               "--prototype-root", str(prototype), "--repository-root", str(repository),
               "--output", str(output)]
    cases = [("identical", 0), ("drift", 0), ("missing", 0), ("corrupt", 1)]
    for case, expected in cases:
        if case == "drift":
            production.write_bytes(b"live backend changed\n")
        elif case == "missing":
            production.unlink()
        elif case == "corrupt":
            snapshot.write_bytes(b"oracle changed\n")
        result = subprocess.run(command, capture_output=True, text=True)
        report = json.loads(output.read_text())
        status = "missing" if case == "corrupt" else case
        if (result.returncode != expected or report["files"][0]["production_status"] != status
                or report["golden_valid"] != (case != "corrupt")):
            raise SystemExit(f"FAIL: {case}: {result.stdout}{result.stderr}")
    output.unlink()
    snapshot.unlink()
    result = subprocess.run(command, capture_output=True, text=True)
    if result.returncode != 1 or json.loads(output.read_text())["golden_valid"]:
        raise SystemExit("FAIL: missing oracle accepted")
print("PASS: 5 integrity/drift cases")
