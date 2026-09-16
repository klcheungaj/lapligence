import argparse
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


def run(command: list[str], expect_success: bool = True) -> None:
    result = subprocess.run(command, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    if (result.returncode == 0) != expect_success:
        raise RuntimeError(f"Unexpected compiler result: {' '.join(command)}\n{result.stdout}")


def main() -> int:
    parser = argparse.ArgumentParser(description="Compile flat runtime embeddings, not the Rust emitter.")
    parser.add_argument("--compiler", action="append", required=True,
                        help="GCC/Clang-style compiler executable; repeat to test both")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    runtime = root / "src" / "sim" / "rt"
    embedding = (runtime / "mod.rs").read_text(encoding="utf-8")
    with tempfile.TemporaryDirectory(prefix="llg-flat-runtime-") as directory:
        output = Path(directory)
        for header in runtime.glob("*.h"):
            shutil.copy2(header, output / header.name)
        shutil.copy2(root / "vendor" / "libaco" / "aco.h", output / "aco.h")
        for name, function in (("llg_rt.c", "runtime_sources"),
                               ("llg_value.c", "value_sources"),
                               ("llg_container.c", "container_sources")):
            body = embedding.split(f"pub fn {function}()", 1)[1].split("\n}\n", 1)[0]
            fragments = re.findall(r'include_str!\("([^"\n]+\.c)"\)', body)
            facade = re.findall(r'^#include "([^"\n]+\.c)"',
                                (runtime / name).read_text(encoding="utf-8"), re.MULTILINE)
            if not fragments or fragments != facade:
                raise RuntimeError(f"Facade/embedding order mismatch for {name}")
            (output / name).write_text("".join((runtime / item).read_text(encoding="utf-8")
                                              for item in fragments), encoding="utf-8")
            print(f"{name}: {len(fragments)} fragments match embedding order")
        probe = ('#include "llg_value.h"\n#define LLG_MODEL_VALUE_ABI 3\n'
                 '_Static_assert(LLG_MODEL_VALUE_ABI == LLG_VALUE_ABI_VERSION, "ABI mismatch");\n')
        (output / "abi_probe.c").write_text(probe, encoding="utf-8")
        (output / "stale_abi.c").write_text(probe.replace("ABI 3", "ABI 2"), encoding="utf-8")
        for compiler in args.compiler:
            common = [compiler, "-std=c11", "-Wall", "-Wextra", "-Werror", "-pedantic-errors",
                      "-I", str(output)]
            for name in ("llg_value.c", "llg_container.c", "llg_rt.c"):
                run(common + ["-c", str(output / name), "-o", str(output / "probe.o")])
                print(f"{compiler}: {name} PASS")
            run(common + ["-fsyntax-only", str(output / "abi_probe.c")])
            run(common + ["-fsyntax-only", str(output / "stale_abi.c")], expect_success=False)
            print(f"{compiler}: accepted ABI 3 and rejected stale ABI 2")
    print("Runtime/ABI probes only; no Rust compilation or generated-HDL execution.")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, IndexError, RuntimeError) as error:
        print(f"flat runtime check: {error}", file=sys.stderr)
        sys.exit(1)
