import subprocess
import sys

ABI_LINE = "LLG_VALUE_ABI_VERSION|"
TEST = "facade_compare_${mode}"


def run(path):
    completed = subprocess.run([path], capture_output=True, text=True, timeout=240)
    assert completed.returncode == 0, (path, completed.returncode, completed.stderr[-2000:])
    return completed.stdout.splitlines()


def covered_by_bridge(manifest):
    rows = {}
    for line in open(manifest, encoding="utf-8").read().splitlines():
        if line and not line.startswith("#"):
            fields = line.split("\t")
            rows.setdefault(fields[1], set()).add(fields[0])
    return rows.get(TEST, set())


def main():
    manifest, legacy, *others = sys.argv[1:]
    assert others, "a compact backend executable is required"
    reference = run(legacy)
    names = {line.split("|", 1)[0] for line in reference}
    missing = sorted(covered_by_bridge(manifest) - names)
    assert not missing, ("operations without comparison output", missing)
    assert len(reference) > 1000, len(reference)
    for path in others:
        lines = run(path)
        differences = []
        for number, (old, new) in enumerate(zip(reference, lines)):
            if old.startswith(ABI_LINE):
                assert (old, new) == (ABI_LINE + " 4", ABI_LINE + " 5"), (old, new)
            elif old != new:
                differences.append((number, old, new))
        assert len(lines) == len(reference), (path, len(lines), len(reference))
        if differences:
            for number, old, new in differences[:5]:
                print(f"line {number}\nlegacy : {old}\nbackend: {new}", file=sys.stderr)
            raise SystemExit(f"{path}: {len(differences)} of {len(reference)} lines differ from legacy")
        print(f"{path}: {len(lines)} lines, {len(names)} operations identical to legacy")


if __name__ == "__main__":
    main()
