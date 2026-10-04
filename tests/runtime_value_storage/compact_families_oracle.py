import random
import subprocess
import sys

WIDTHS = [1, 2, 7, 8, 31, 32, 33, 63, 64, 65, 127, 128, 129, 255, 256, 257, 1023, 1024, 4096, 16384, (1 << 20) - 1]


def value(bits, x, z, width, sign):
    mask = (1 << width) - 1
    x &= mask
    z &= mask & ~x
    return bits & mask & ~(x | z), x, z, width, sign


def extend(v, width, sign):
    bits, x, z, w, _ = v
    padding = ((1 << width) - 1) ^ ((1 << w) - 1)
    if sign and w:
        bits |= padding if bits >> (w - 1) else 0
        x |= padding if x >> (w - 1) else 0
        z |= padding if z >> (w - 1) else 0
    return bits, x, z


def truth(v):
    return 1 if v[0] else 2 if v[1] or v[2] else 0


def predicate(state):
    return value(state == 1, state == 2, 0, 1, 0)


def relation(a, b, ge):
    if a[1] or a[2] or b[1] or b[2]:
        return 2
    x, y = a[0], b[0]
    if a[4] and b[4]:
        x -= (1 << a[3]) if a[3] and x >> (a[3] - 1) else 0
        y -= (1 << b[3]) if b[3] and y >> (b[3] - 1) else 0
    return int(x >= y if ge else x <= y)


def oracle(op, a, b, c):
    bits, x, z, w, s = a
    mask = (1 << w) - 1
    if op < 4:
        if b[1] or b[2]:
            return value(0, mask, 0, w, s)
        count = b[0]
        right = op in (1, 3)
        if count >= w:
            planes = tuple(mask if right and op == 3 and s and w and plane >> (w - 1) else 0 for plane in a[:3])
        elif right:
            padding = mask ^ ((1 << (w - count)) - 1)
            planes = tuple((plane >> count) | (padding if op == 3 and s and plane >> (w - 1) else 0) for plane in a[:3])
        else:
            planes = tuple((plane << count) & mask for plane in a[:3])
        return value(*planes, w, s)
    if op <= 12:
        ones = bin(bits).count("1")
        zeros = w - ones - bin(x | z).count("1")
        if op == 10:
            return value(ones, 0, 0, 32, 1)
        if op in (11, 12):
            return predicate(int(ones == 1 or op == 12 and ones == 0))
        if op in (4, 5):
            t = 0 if zeros else 2 if x or z else 1
        elif op in (6, 7):
            t = 1 if ones else 2 if x or z else 0
        else:
            t = 2 if x or z else ones % 2
        return predicate(1 - t if op % 2 and t != 2 else t)
    if op <= 16:
        width = max(a[3], b[3])
        aa, ax, az = extend(a, width, op >= 15 and a[4] and b[4])
        bb, bx, bz = extend(b, width, op >= 15 and a[4] and b[4])
        care = (1 << width) - 1
        if op == 13:
            care &= ~(ax | az | bx | bz)
            t = int(not ((aa ^ bb) & care))
        elif op == 14:
            care &= ~(az | bz)
            t = int(not (((aa ^ bb) | (ax ^ bx)) & care))
        else:
            care &= ~(bx | bz)
            if (aa ^ bb) & care & ~(ax | az):
                t = 0
            else:
                t = 2 if (ax | az) & care else 1
            if op == 16 and t != 2:
                t = 1 - t
        return predicate(t)
    if op in (17, 18):
        ta, tb = truth(a), truth(b)
        if op == 17:
            t = 1 if ta == 0 or tb == 1 else 0 if ta == 1 and tb == 0 else 2
        else:
            t = 2 if ta == 2 or tb == 2 else int(ta == tb)
        return predicate(t)
    ge, le = relation(a, b, True), relation(a, c, False)
    return predicate(0 if ge == 0 or le == 0 else 1 if ge == 1 and le == 1 else 2)


def load(path):
    process = subprocess.Popen([path], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, bufsize=1)

    def evaluate(op, a, b, c):
        fields = [str(op)]
        for v in (a, b, c):
            fields += [str(v[3]), str(v[4])]
            for plane in v[:3]:
                raw = plane.to_bytes(8 * ((v[3] + 63) // 64), 'little')
                fields += [format(int.from_bytes(raw[i:i + 8], 'little'), 'x') for i in range(0, len(raw), 8)]
        process.stdin.write(' '.join(fields) + '\n')
        process.stdin.flush()
        result = process.stdout.readline().split()
        assert result, (path, process.poll())
        w, s, size = map(int, result[:3])
        n = (w + 63) // 64
        planes = tuple(int(''.join(f'{int(word, 16):016x}' for word in reversed(result[3 + p * n:3 + (p + 1) * n])), 16) if n else 0 for p in range(3))
        return (*planes, w, s), size

    return process, evaluate


def main():
    if not __debug__:
        raise SystemExit('compact family oracle requires assertions enabled')
    libraries = [load(path) for path in sys.argv[1:]]
    rng = random.Random(0x3FA585A)
    cases = 0

    def check(op, a, b, c):
        nonlocal cases
        wanted = oracle(op, a, b, c)
        for i, (_, fn) in enumerate(libraries):
            got, size = fn(op, a, b, c)
            assert got == wanted, (i, op, a[3:], b[3:], c[3:], wanted, got)
            if i == 0:
                expected_size = 0 if wanted[3] <= 64 else 8 * ((wanted[3] + 63) // 64) * (2 if wanted[1] or wanted[2] else 1)
                assert size == expected_size, (size, expected_size)
        cases += 1

    empty = value(0, 0, 0, 0, 0)
    for w in WIDTHS:
        mask = (1 << w) - 1
        bw = max(1, w - 1)
        patterns = [(0, 0, 0), (mask, 0, 0), (1 << (w - 1), 0, 0), (mask // 3, 0, 0), (0, mask, 0), (0, 0, mask)]
        if w < 20000:
            patterns += [(rng.getrandbits(w), rng.getrandbits(w), rng.getrandbits(w)) for _ in range(6)]
        if w > 20000:
            for bits, x, z in patterns:
                for sa in (0, 1):
                    a = value(bits, x, z, w, sa)
                    b = value(65, 0, 0, 129, 1)
                    c = value(mask, 0, 0, w, sa)
                    for op in range(20):
                        check(op, a, b, c)
            continue
        for bits, x, z in patterns:
            for sa in (0, 1):
                a = value(bits, x, z, w, sa)
                for op in range(4, 13):
                    check(op, a, empty, empty)
                for count in (0, 1, 2, 63, 64, 65, w - 1, w, w + 1, (1 << 64) - 1, (1 << 128) + 1):
                    for sb in (0, 1):
                        b = value(count, 0, 0, 129, sb)
                        for op in range(4):
                            check(op, a, b, empty)
                for sb in (0, 1):
                    for plane in (1, 2):
                        b = value(1 << 128, 1 << 127 if plane == 1 else 0, 1 << 127 if plane == 2 else 0, 129, sb)
                        for op in range(4):
                            check(op, a, b, empty)
                    b = value(bits ^ mask, z, x, bw, sb)
                    c = value(mask, x, z, w + 1, 1 - sb)
                    for op in range(13, 20):
                        check(op, a, b, c)
                        check(op, b, a, c)
                    for low, high in ((0, mask), (mask, 0), (1 << (w - 1), (1 << (w - 1)) - 1)):
                        check(19, a, value(low, 0, 0, w, sb), value(high, 0, 0, w, sb))
    for _, fn in libraries:
        for op in range(20):
            assert fn(op, empty, empty, empty)[0] == oracle(op, empty, empty, empty)
    for process, _ in libraries:
        process.stdin.close()
        assert process.wait(timeout=10) == 0
    print(f'families Python: {cases} vectors per backend, {len(libraries)} backends')


if __name__ == '__main__':
    main()
