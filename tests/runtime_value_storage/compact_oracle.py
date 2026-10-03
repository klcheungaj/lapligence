import subprocess
import random
import sys

WIDTHS = [1, 2, 7, 8, 31, 32, 33, 63, 64, 65, 127, 128, 129, 255, 256, 257, 1023, 1024, 4096, 8191, 8192, 8193, 16384]


def load(path):
    process = subprocess.Popen([path], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, bufsize=1)

    def evaluate(op, av, aw, sa, bv, bw, sb):
        data = [str(op), str(aw), str(sa), str(bw), str(sb)]
        data += [format((av >> i) & ((1 << 64) - 1), "x") for i in range(0, aw, 64)]
        data += [format((bv >> i) & ((1 << 64) - 1), "x") for i in range(0, bw, 64)]
        process.stdin.write(" ".join(data) + "\n")
        process.stdin.flush()
        result = process.stdout.readline().split()
        assert result, (path, process.poll())
        w, s, size = map(int, result[:3])
        values = tuple(sum(int(result[3 + 3 * i + plane], 16) << (64 * i) for i in range((w + 63) // 64)) for plane in range(3))
        return values, w, s, size

    return process, evaluate


def call(fn, op, av, aw, sa, bv, bw, sb):
    return fn(op, av, aw, sa, bv, bw, sb)


def signed(value, width, sign):
    return value - (1 << width) if sign and width and value >> (width - 1) else value


def oracle(op, av, aw, sa, bv, bw, sb):
    av &= (1 << aw) - 1
    bv &= (1 << bw) - 1
    w, s = max(aw, bw), int(sa and sb)
    a = signed(av, aw, s)
    b = signed(bv, bw, s)
    unknown = False
    if op == 0:
        r = a + b
    elif op == 1:
        r = a - b
    elif op == 2:
        r = a * b
    elif op in (3, 4):
        if b == 0:
            r, unknown = 0, True
        else:
            q = abs(a) // abs(b) * (-1 if (a < 0) != (b < 0) else 1)
            r = q if op == 3 else a - q * b
    elif op == 5:
        w, s = aw, sa
        b = signed(bv, bw, sb)
        a = signed(av, aw, sa)
        if b < 0:
            if a == 0:
                r, unknown = 0, True
            elif a == -1:
                r = -1 if b & 1 else 1
            else:
                r = 1 if a == 1 else 0
        else:
            r = pow(a, b, 1 << w)
    elif op == 6:
        w, s, r = aw, sa, -av
    elif op == 7:
        w, s, r = 32, 0, (av - 1).bit_length() if av else 0
    elif op in (17, 18, 19, 20):
        r = int((a < b, a <= b, a > b, a >= b)[op - 17])
        w, s = 1, 0
    elif op in (25, 26):
        w, s = bw, sb
        r = signed(av, aw, sa if op == 25 else sb)
    else:
        raise ValueError(op)
    mask = (1 << w) - 1
    return ((0, mask, 0) if unknown else (r & mask, 0, 0)), w, s


def main():
    libs = [load(p) for p in sys.argv[1:]]
    rng = random.Random(0x4839712)
    cases = 0
    for aw in WIDTHS:
        for bw in sorted({aw, 1, max(1, aw - 1), min(16384, aw + 1)}):
            patterns = [(0, 0), ((1 << aw) - 1, 1), (1 << (aw - 1), (1 << bw) - 1), (1, 1 << (bw - 1))]
            patterns += [(rng.getrandbits(aw), rng.getrandbits(bw)) for _ in range(6)]
            for sa in (0, 1):
                for sb in (0, 1):
                    for av, bv in patterns:
                        for op in (0, 1, 2, 3, 4, 6, 7, 17, 18, 19, 20, 25, 26):
                            expected = oracle(op, av, aw, sa, bv, bw, sb)
                            for index, (_, fn) in enumerate(libs):
                                got = call(fn, op, av, aw, sa, bv, bw, sb)
                                assert got[:3] == expected, (index, op, aw, bw, sa, sb, av, bv, expected, got)
                                if index == 0:
                                    wanted_bytes = 0 if expected[1] <= 64 else 8 * ((expected[1] + 63) // 64) * (2 if expected[0][1] else 1)
                                    assert got[3] == wanted_bytes
                            cases += 1
        for sa in (0, 1):
            for av in (0, 1, 2, 3, (1 << aw) - 1, 1 << (aw - 1), rng.getrandbits(aw)):
                for exponent in (-3, -2, -1, 0, 1, 2, 3, 7, 16, 65, (1 << 65) + 1):
                    bw = 67
                    bv = exponent & ((1 << bw) - 1)
                    expected = oracle(5, av, aw, sa, bv, bw, 1)
                    for index, (_, fn) in enumerate(libs):
                        got = call(fn, 5, av, aw, sa, bv, bw, 1)
                        assert got[:3] == expected, (index, "pow", aw, sa, av, exponent, expected, got)
                    cases += 1
    aw = (1 << 20) - 1
    for op in (0, 1, 3, 4, 6, 7, 25, 26):
        av, bv, bw = (1 << (aw - 1)) | 7, 3, 8
        expected = oracle(op, av, aw, 1, bv, bw, 1)
        for _, fn in libs:
            assert call(fn, op, av, aw, 1, bv, bw, 1)[:3] == expected
        cases += 1
    for process, _ in libs:
        process.stdin.close()
        assert process.wait() == 0
    print(f"Python big-int oracle: {cases} vectors; {len(libs)} backends; passed")


if __name__ == "__main__":
    main()
