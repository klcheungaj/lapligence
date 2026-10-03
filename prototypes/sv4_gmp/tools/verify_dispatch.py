import argparse
import hashlib
import json
import shutil
import subprocess
import tempfile
from pathlib import Path

parser = argparse.ArgumentParser(description="Compare facade/direct .text and relocation tables with ELF GCC/Clang tools")
parser.add_argument("--compiler", default="cc")
parser.add_argument("--gmp-root", type=Path, required=True)
parser.add_argument("--output", type=Path, required=True)
args = parser.parse_args()
root = Path(__file__).resolve().parents[1]
for tool in (args.compiler, "objcopy", "objdump"):
    if shutil.which(tool) is None:
        parser.error(f"Missing required tool: {tool}")

def run(command):
    return subprocess.run(command, capture_output=True, text=True, check=True)

def relocation_table(path):
    lines = run(["objdump", "-r", str(path)]).stdout.splitlines()
    return "\n".join(line for line in lines if "file format" not in line).strip()

records = []
with tempfile.TemporaryDirectory(prefix="sv4-dispatch-") as temp:
    directory = Path(temp)
    for backend in (0, 1):
        for optimization in ("-O0", "-O2"):
            bodies = {}
            for mode in ("facade", "direct"):
                if mode == "facade":
                    header = '#include "sv4.h"\n' if backend else '#include "llg_value.h"\n'
                    typename, prefix = "sv4_t", "sv4_"
                elif backend:
                    header = '#include "gmp4.h"\n'
                    typename, prefix = "gmp4_t", "gmp4_"
                else:
                    header = '#include "llg_value.h"\n'
                    typename, prefix = "sv4_t", "sv4_"
                source = directory / f"{mode}.c"
                source.write_text(header + f"""
{typename} probe_add({typename} a, {typename} b) {{ return {prefix}add(a, b); }}
{typename} probe_and({typename} a, {typename} b) {{ return {prefix}and(a, b); }}
void probe_copy({typename} *a, const {typename} *b) {{ {prefix}copy(a, b); }}
""")
                if optimization == "-O2":
                    if mode == "facade":
                        extra = f"uint32_t probe_width({typename} v) {{ return llg_sv4_width(v); }}\n"
                        extra += f"uint64_t probe_word({typename} v, size_t w) {{ return llg_sv4_word(v,w,0); }}\n"
                    else:
                        extra = f"uint32_t probe_width({typename} v) {{ return v.width; }}\n"
                        if backend:
                            extra += f"uint64_t probe_word({typename} v, size_t w) {{ return gmp4_word(v,w,0); }}\n"
                        else:
                            extra += f"uint64_t probe_word({typename} v, size_t w) {{ return w >= ((size_t)v.width+63u)/64u ? 0 : v.bits[w]; }}\n"
                    source.write_text(source.read_text() + extra)
                obj = source.with_suffix(".o")
                run([args.compiler, "-std=c11", optimization, "-fno-ident",
                     f"-DLLG_SV4_USE_GMP={backend}", "-I" + str(root / "include"),
                     "-I" + str(root.parents[1] / "src/sim/rt"), "-I" + str(args.gmp_root / "include"),
                     "-c", str(source), "-o", str(obj)])
                binary = directory / f"{mode}.text"
                run(["objcopy", "--dump-section", f".text={binary}", str(obj)])
                bodies[mode] = (binary.read_bytes(), relocation_table(obj))
            if bodies["facade"] != bodies["direct"]:
                raise SystemExit(f"FAIL: facade adds a difference, backend={backend} {optimization}")
            records.append({"backend": "gmp" if backend else "legacy", "optimization": optimization,
                            "text_bytes": len(bodies["facade"][0]),
                            "text_sha256": hashlib.sha256(bodies["facade"][0]).hexdigest(),
                            "relocations": bodies["facade"][1], "identical": True})
report = {"compiler": run([args.compiler, "--version"]).stdout.splitlines()[0],
          "scope": "Production legacy/prototype GMP add, and, copy at O0/O2; neutral width/word at O2. Not a proof of every operation's cost.",
          "records": records}
args.output.write_text(json.dumps(report, indent=2) + "\n")
print(f"PASS: {len(records)} backend/optimization pairs, identical .text and relocations")
