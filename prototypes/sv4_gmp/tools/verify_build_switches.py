import argparse
import json
import re
import shutil
import subprocess
import tempfile
from pathlib import Path
from generate_api_coverage import audit

parser = argparse.ArgumentParser(description="Unix GCC/Clang build-selection and fail-closed checks")
parser.add_argument("--compiler", default="cc")
parser.add_argument("--gmp-root", type=Path, required=True)
parser.add_argument("--output", type=Path, required=True)
parser.add_argument("--jobs", type=int, default=8)
args = parser.parse_args()
if args.jobs < 1:
    parser.error("--jobs must be positive")
root = Path(__file__).resolve().parents[1]
gmp_root = args.gmp_root.resolve()
for tool in ("cmake", "ctest", "nm", args.compiler):
    if not shutil.which(tool):
        parser.error(f"Missing required tool: {tool}")
records = []
surface = audit(root)

def check(name, command, success=True, needle=None):
    result = subprocess.run(command, capture_output=True, text=True)
    text = result.stdout + result.stderr
    okay = (result.returncode == 0) == success and (needle is None or needle in text)
    records.append({"name": name, "expected": "success" if success else "failure",
                    "exit_code": result.returncode, "pass": okay, "output": text})
    if not okay:
        args.output.write_text(json.dumps(records, indent=2) + "\n")
        raise SystemExit(f"FAIL: {name}\n{text}")
    return text

with tempfile.TemporaryDirectory(prefix="sv4-switches-") as temp:
    directory = Path(temp)
    for backend in (0, 1):
        build = directory / str(backend)
        check(f"configure_{backend}", ["cmake", "-S", str(root), "-B", str(build),
              "-DCMAKE_C_COMPILER=" + args.compiler,
              "-DLLG_SV4_USE_GMP=" + ("ON" if backend else "OFF"),
              "-DLLG_SV4_BUILD_DIFFERENTIAL=OFF",
              "-DGMP_ROOT=" + str(gmp_root if backend else directory / "nonexistent")])
        check(f"build_{backend}", ["cmake", "--build", str(build), "--parallel", str(args.jobs)])
        check(f"ctest_{backend}", ["ctest", "--test-dir", str(build), "--parallel", str(args.jobs), "--output-on-failure"])
        symbols = check(f"symbols_{backend}", ["nm", str(build / "sv4_sample")])
        legacy_names = [row["legacy_name"] for row in surface["functions"]]
        legacy_names += list(surface["llg_helpers"])
        forbidden = (r"\b(?:" + "|".join(map(re.escape, legacy_names)) + r")\b"
                     if backend else r"\b(__gmp\w*|gmp4_\w*)\b")
        if re.search(forbidden, symbols):
            raise SystemExit(f"FAIL: backend {backend} contains unselected backend symbols")
    check("reject_missing_gmp", ["cmake", "-S", str(root), "-B", str(directory / "bad"),
          "-DLLG_SV4_USE_GMP=ON", "-DGMP_ROOT=" + str(directory / "missing")],
          False, "GMP was not found")
    source = directory / "client.c"
    source.write_text('#include "sv4.h"\nint main(void) {sv4_t v=sv4_zero(1,0);sv4_destroy(&v);return 0;}\n')
    base = [args.compiler, "-std=c11", "-I" + str(root / "include"), "-I" + str(gmp_root / "include")]
    check("reject_invalid_selector", base + ["-DLLG_SV4_USE_GMP=2", "-c", str(source), "-o", str(directory / "bad.o")],
          False, "must be 0 or 1")
    for selector in ("-1", "unknown", "", "1+0"):
        check("reject_prototype_selector_" + selector, base + ["-DLLG_SV4_USE_GMP=" + selector,
              "-c", str(source), "-o", str(directory / "bad.o")], False)
    production = root.parents[1] / "src/sim/rt"
    production_base = [args.compiler, "-std=c11", "-I" + str(production)]
    source.write_text('#include "llg_value.h"\nint main(void) {return 0;}\n')
    check("production_default_legacy", production_base + ["-c", str(source), "-o", str(directory / "prod.o")])
    for selector in ("2", "-1", "unknown", "", "1+0"):
        check("reject_production_selector_" + selector, production_base + ["-DLLG_SV4_USE_GMP=" + selector,
              "-c", str(source), "-o", str(directory / "bad.o")], False)
    check("production_gmp_unavailable", production_base + ["-DLLG_SV4_USE_GMP=1",
          "-c", str(source), "-o", str(directory / "bad.o")], False, "Production GMP backend awaits")
    for backend in (0, 1):
        declarations = [row["declaration"] for row in surface["functions"]
                        if backend == 0 or row["status"] == "implemented"]
        if backend == 0:
            declarations += list(surface["llg_helpers"].values())
        pointers = []
        for declaration in declarations:
            match = re.search(r"\b((?:sv4_|llg_)\w+)\s*\(", declaration)
            name = match[1]
            pointers.append(declaration[:match.start()] + "(*probe_" + name + ")" +
                            declaration[match.end()-1:] + " = &" + name + ";")
        header = 'sv4.h' if backend else 'llg_value.h'
        source.write_text('#include "' + header + '"\n' + "\n".join(pointers) + "\n")
        command_base = base if backend else production_base
        check(f"unchanged_signatures_{backend}", command_base + ["-Werror=incompatible-pointer-types",
              f"-DLLG_SV4_USE_GMP={backend}", "-c", str(source), "-o", str(directory / "signatures.o")])
    source.write_text('#include "sv4.h"\nint main(void) {sv4_t v=sv4_zero(1,0);sv4_destroy(&v);return 0;}\n')
    check("reject_new_client_old_library", base + ["-DLLG_SV4_USE_GMP=1", str(source),
          str(directory / "0/libsv4_golden.a"), "-lm", "-o", str(directory / "wrong")], False, "gmp4_zero")
    gmp_library = next((path for folder in ("lib", "lib64") for name in ("libgmp.a", "libgmp.so", "libgmp.dylib")
                        if (path := gmp_root / folder / name).exists()), None)
    if gmp_library is None:
        raise SystemExit("GMP library not found below prefix")
    check("reject_old_client_new_library", base + ["-DLLG_SV4_USE_GMP=0", str(source),
          str(directory / "1/libsv4_gmp.a"), str(gmp_library), "-o", str(directory / "wrong")], False, "sv4_zero")
    source.write_text('#include "sv4.h"\nint main(void) {sv4_t v=SV4_EMPTY;sv4_t r=sv4_div(v,v);sv4_destroy(&r);return 0;}\n')
    check("reject_unimplemented_api", base + ["-Werror=implicit-function-declaration", "-DLLG_SV4_USE_GMP=1",
          "-c", str(source), "-o", str(directory / "missing.o")], False, "sv4_div")
    missing = [row["legacy_name"] for row in surface["functions"] if row["status"] != "implemented"]
    missing += list(surface["llg_helpers"])
    for name in missing:
        source.write_text('#include "sv4.h"\nvoid probe(void) {(void)&' + name + ';}\n')
        check("unmapped_" + name, base + ["-DLLG_SV4_USE_GMP=1", "-c", str(source),
              "-o", str(directory / "missing.o")], False, name)
args.output.write_text(json.dumps(records, indent=2) + "\n")
print(f"PASS: {len(records)} build, test, symbol and expected-failure checks")
