import random
import sys
from pathlib import Path

rng = random.Random(0x46F75BD9)
widths = [0, 1, 2, 7, 8, 15, 31, 32, 33, 63, 64, 65, 127, 128, 129,
          255, 256, 257, 1023, 1024, 4096, 16384]
lines = ["/* Generated mathematical oracle; regenerate, do not edit. */"]
records = []

def mask(width):
    return (1 << width) - 1

def words(value, width):
    return ",".join(f"UINT64_C(0x{(value >> i) & mask(64):016x})"
                    for i in range(0, max(width, 1), 64))

def extend(value, width, out_width, signed):
    if signed and width and value & (1 << (width - 1)):
        value -= 1 << width
    return value & mask(out_width)

for case in range(400):
    wa = widths[case % len(widths)] if case < 88 else rng.randrange(1, 2049)
    wb = wa if case % 3 == 0 else rng.choice(widths[:-1])
    sa, sb = (case >> 1) & 1, case & 1
    a, b = rng.getrandbits(wa), rng.getrandbits(wb)
    if case % 7 == 0:
        a, b = mask(wa), int(wb > 0)
    if case % 11 == 0:
        a = 1 << (wa - 1) if wa else 0
    w = max(wa, wb)
    x, y = extend(a, wa, w, sa and sb), extend(b, wb, w, sa and sb)
    payloads = {"a": (a, wa), "b": (b, wb), "add": ((x+y)&mask(w), w),
                "sub": ((x-y)&mask(w), w), "mul": ((x*y)&mask(w), w)}
    for name, (value, width) in payloads.items():
        lines.append(f"static const uint64_t v{case}_{name}[] = {{{words(value,width)}}};")
    records.append(f"{{{wa},{wb},{sa},{sb},v{case}_a,v{case}_b,v{case}_add,v{case}_sub,v{case}_mul}}")
lines.append("static const oracle_case_t oracle_cases[] = {" + ",\n".join(records) + "};")
Path(sys.argv[1]).write_text("\n".join(lines) + "\n", encoding="ascii")
