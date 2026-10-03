import math
import random
import struct
import subprocess
import sys

if not __debug__:
    raise RuntimeError("oracle assertions must be enabled")
sys.set_int_max_str_digits(0)
widths = [0, 1, 2, 7, 8, 31, 32, 33, 63, 64, 65, 127, 128, 129, 255, 256, 257,
          1023, 1024, 4096, 8191, 8192, 8193, 16384, (1 << 20) - 1]
rng = random.Random(0xC065789)
requests = []
expected = []


def signed(bits, width):
    return bits - (1 << width) if width and bits >> (width - 1) else bits


def round_integer(value):
    numerator, denominator = value.as_integer_ratio()
    magnitude = (abs(numerator) * 2 + denominator) // (2 * denominator)
    return -magnitude if numerator < 0 else magnitude


def from_real(value, width, sign, rtoi=False):
    requests.append(f"rtoi {value.hex()}" if rtoi else f"from {width} {sign} {value.hex()}")
    unknown = not math.isfinite(value) and width != 0
    number = 0 if not math.isfinite(value) else (int(value) if rtoi else round_integer(value)) & ((1 << width) - 1)
    expected.append(("from", (width, sign, int(unknown), number)))


def groups(bits, x, z, width, size, limit):
    result = []
    for bit in range(((width + size - 1) // size - 1) * size, -1, -size):
        mask = ((1 << size) - 1) << bit
        result.append("x" if x & mask else "z" if z & mask else format((bits & mask) >> bit, "x"))
        if len(result) >= limit:
            break
    return "".join(result)


def inspect(bits, x, z, width, sign, cap):
    mask = (1 << width) - 1
    x &= mask
    z &= mask & ~x
    bits &= mask & ~(x | z)
    requests.append(f"inspect {width} {sign} {cap} {bits:x} {x:x} {z:x}")
    integer = signed(bits, width) if sign else bits
    low_width = min(width, 64)
    low_i64 = signed(bits & ((1 << low_width) - 1), low_width)
    fits = not (x | z) and -(1 << 63) <= integer < (1 << 63)
    index = integer if not (x | z) and 0 <= integer < (1 << 64) else (1 << 64) - 1
    try:
        real = float(integer)
    except OverflowError:
        real = -math.inf if integer < 0 else math.inf
    bitreal = struct.unpack("=d", struct.pack("=Q", bits & ((1 << 64) - 1)))[0]
    shortreal = float(struct.unpack("=f", struct.pack("=I", bits & ((1 << 32) - 1)))[0])
    decimal = "x" if x | z else str(integer)
    binary = "".join("x" if (x >> i) & 1 else "z" if (z >> i) & 1 else str((bits >> i) & 1)
                     for i in range(width - 1, max(-1, width - 1 - max(1, cap)), -1))
    texts = [decimal, groups(bits, x, z, width, 4, max(1, cap)), binary, groups(bits, x, z, width, 3, max(1, cap))]
    texts = [text[:max(0, cap - 1)] or "#" for text in texts]
    expected.append(("inspect", (low_i64, int(fits), index, int(fits), integer if fits else 1234567,
                                  real, bitreal, shortreal, texts)))


values = [0.0, -0.0, 0.5, -0.5, 1.5, -1.5, 2.5, -2.5, float("inf"), -float("inf"), float("nan")]
values += [math.ldexp(rng.random() * 2 - 1, rng.randrange(-1074, 1024)) for _ in range(300)]
for width in widths:
    for sign in (0, 1):
        for value in values if width < 20000 else values[:11] + [0x1FFFFFFFFFFFFF * 2.0 ** 971]:
            from_real(value, width, sign)
for value in values:
    from_real(value, 32, 1, True)
for width in widths:
    mask = (1 << width) - 1
    samples = [0, 1 & mask, mask, 1 << (width - 1) if width else 0, (1 << 63) & mask,
               ((1 << 63) - 1) & mask, ((1 << 64) - 1) & mask]
    if width > 20000:
        samples = [0, 1, mask, (1 << 64) - 1]
    else:
        samples += [rng.getrandbits(width) for _ in range(16)]
    for sign in (0, 1):
        for sample in samples:
            if width > 20000 and sample == mask and not sign:
                continue
            for cap in (0, 1, 2, 9, 80):
                inspect(sample, 0, 0, width, sign, cap)
        for _ in range(8 if width < 20000 else 2):
            x, z, bits = rng.getrandbits(width), rng.getrandbits(width), rng.getrandbits(width)
            inspect(bits, x, z, width, sign, 80)
    if width <= 4096:
        inspect(mask, 0, 0, width, 0, width + 32)
        inspect(rng.getrandbits(width), rng.getrandbits(width), rng.getrandbits(width), width, 1, width + 32)


def close_real(actual, oracle):
    if math.isnan(oracle):
        return math.isnan(actual)
    if math.isinf(oracle):
        return actual == oracle
    if actual == oracle:
        return True
    return math.isfinite(actual) and abs(actual - oracle) <= math.ulp(oracle)


payload = "\n".join(requests) + "\n"
for executable in sys.argv[1:]:
    run = subprocess.run([executable], input=payload, text=True, capture_output=True, timeout=180)
    assert run.returncode == 0, (executable, run.stderr)
    lines = run.stdout.splitlines()
    assert len(lines) == len(expected), (executable, len(lines), len(expected))
    for index, ((kind, oracle), line) in enumerate(zip(expected, lines)):
        fields = line.split()
        if kind == "from":
            actual = tuple(map(int, fields[:3])) + (int(fields[3], 16),)
            assert actual == oracle, (executable, index, requests[index], actual, oracle)
        else:
            actual = tuple(map(int, fields[:5]))
            assert actual == oracle[:5], (executable, index, requests[index][:200], actual, oracle[:5])
            assert close_real(float.fromhex(fields[5]), oracle[5]), (
                executable, index, requests[index][:200], fields[5], oracle[5])
            for i in (6, 7):
                actual_real = float.fromhex(fields[i])
                assert (math.isnan(actual_real) and math.isnan(oracle[i])) or (
                    struct.pack("=d", actual_real) == struct.pack("=d", oracle[i])), (
                        executable, index, fields[i], oracle[i])
            assert fields[8:] == oracle[8], (executable, index, requests[index][:200], fields[8:], oracle[8])
    print(f"{executable}: {len(lines)} independent real/integer/text vectors passed")
