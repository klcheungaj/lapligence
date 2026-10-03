import argparse
import hashlib
import json
from pathlib import Path


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest() if path.is_file() else None


def verify(root, repository):
    manifest = json.loads((root / "golden_manifest.json").read_text())
    records = []
    for entry in manifest["files"]:
        snapshot_hash = digest(root / entry["snapshot"])
        production_hash = digest(repository / entry["production"])
        records.append({
            **entry,
            "snapshot_sha256": snapshot_hash,
            "snapshot_matches": snapshot_hash == entry["sha256"],
            "production_sha256": production_hash,
            "production_status": "missing" if production_hash is None else
                "identical" if production_hash == entry["sha256"] else "drift",
        })
    return {
        "repository": str(repository),
        "golden_valid": all(row["snapshot_matches"] for row in records),
        "production_drift": sum(row["production_status"] == "drift" for row in records),
        "production_missing": sum(row["production_status"] == "missing" for row in records),
        "files": records,
    }


def main():
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description="Verify frozen bytes and report live production drift separately")
    parser.add_argument("--prototype-root", type=Path, default=root)
    parser.add_argument("--repository-root", type=Path, default=root.parents[1])
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    try:
        report = verify(args.prototype_root.resolve(), args.repository_root.resolve())
    except (OSError, ValueError, KeyError) as error:
        parser.exit(1, f"golden verification failed: {error}\n")
    if args.output:
        args.output.write_text(json.dumps(report, indent=2) + "\n")
    for row in report["files"]:
        if not row["snapshot_matches"]:
            print(f"golden changed or missing: {row['snapshot']}")
        print(f"production {row['production_status']}: {row['production']} "
              f"sha256={row['production_sha256']}")
    print(f"golden: {len(report['files'])} files, valid={report['golden_valid']}; "
          f"production: {report['production_drift']} drift, {report['production_missing']} missing")
    return 0 if report["golden_valid"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
