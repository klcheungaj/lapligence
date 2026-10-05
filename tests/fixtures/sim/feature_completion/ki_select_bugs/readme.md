# Packed-array element members, member-target selectors and procedural `$sampled`

IEEE 1800-2009 supplies the oracles: §7.2.1 and §7.4.1 for packed structure
and union members (a structure with any four-state member is four-state; a
two-state member reads as if cast), §7.4.5 and §11.5.1 for element selects
(an out-of-range or X element reads X and is not written), §9.2.2.2.1 for
`always_comb` sensitivity, §10.3.2 and §23.3.3.2 for continuous assignments and
variable output ports, and §16.9.3 with §16.5.1 for `$sampled`. Every `.out`
file is derived by hand below; none were captured from `llg`.

- `element_members`: `pair_t` is `{hi, lo}`, 4 bits each.
  - `ps = 87654321` holds `ps[3] = 87` down to `ps[0] = 21`, so `ps[1].hi = 4`,
    `ps[2].lo = 5`, and with `i = 3` `ps[i] = 87` gives `8 7`.
  - `ps[0].hi = a` and `ps[i].lo = b` give `8b6543a1`. `ps[2].hi[3] = 0`
    leaves `6` unchanged and `ps[1].lo[1:0] = 00` makes `43` into `40`:
    `8b6540a1`. `ps[3].hi[3]` of `8` is `1`, `ps[1].hi[2:1]` of `4` is `10`.
  - Ascending `asc [0:3] = 01234567` has `asc[0] = 01`, `asc[1] = 23` and
    `asc[3] = 67`: `0 7 2`. `nz [5:2] = 89abcdef` has `nz[5] = 89`,
    `nz[3] = cd` and `nz[2] = ef`: `8 f d`.
  - `nz[7]`, `nz[0]` and `nz[x]` are outside `[5:2]` or unknown: each member
    reads `x`, and writes through `nz[9]`, `nz[0]` and `nz[x]` change nothing.
  - `w [1:0][1:0] = 87654321` has `w[1][0] = 65` and `w[0][1] = 43`: `6 3 6`.
    `w[1][1].hi = a` and `w[1][0].lo = b` give `a76b4321`.
  - `rec_t` is `{signed s[3:0], pair_t p}` (12 bits). `r[1].s = -3` (`d`),
    `r[0].p.lo = 5` and `r[0].p.hi = c` give `d000c5`; `r[1].s` prints `-3`
    because the member is signed.
  - Union `u = 1234`: `u[1].p.hi = 1`, `u[0].b = 34`, `u[0].p.lo = 4`.
  - Nested `ns[1].b.hi = 7` and `ns[0].a.lo = 3` give `0070` and `0300`, so
    `00700300`, and `ns[1].b = 70`.
  - `mixed_t` is `{logic f[3:0], bit t[3:0]}`. From `xx_x5`, `mx[0].t = 5` and
    `mx[0].f = x`. Writing `4'bx01z` to the two-state `mx[1].t` reads back as
    `0010`; `mx[1].f` stays `x`.
  - The automatic local `l = 1234` gives `2 3`, then `l[1].hi = 9` gives
    `9234`. The parameter `P = a5c3` gives `P[1].hi = a`, `P[0].lo = 3`.
  - The unpacked array `q` of `pair_t [1:0]`: `q[1][0] = 44` gives `4`,
    `q[0][1] = 11` gives `1`, and `q[1][1].lo = f` gives `3f44`.
  - `holder_t` is `{pair_t [1:0] arr, tag[3:0]}`: `h.arr[1].hi = 7` and
    `h.arr[0].lo = 2` give `arr = 7002`, so `h = 70020`, and `h.arr[1].hi = 7`.
  - `pick(g, 2)` reads `g[2] = 23`, so `2`; `poke(g, 1)` writes
    `g[1].lo = e` through a `ref` formal: `01234e67`.
  - NBAs: `u[1].p.lo <= f` gives `1f34`; `w[1][0].hi <= 0` makes `6b` into
    `0b`: `a70b4321`.
- `element_member_views`: `src = 89abcdef`, `h = 12345` (`arr = 1234`).
  - t1 (`i = k = 0`): `y0 = ~src[1].lo = ~d = 2`; `assign cv[k].lo = y0`
    writes `cv[0]`: `xxx2`. `out[1].hi = ~3 = c` and
    `out[0].lo = ~src[0].hi = ~e = 1`: `cxx1`. `a = h.arr[0].hi = 3`,
    `c = src[1].hi = c`, `pw[0].hi = 5` (`xxxxxx5x`), and the net drivers of
    `wn[1].hi`, `wn[1].lo` and `wn[0]` give `9876`.
  - t2 (`i = 1`, `k = 2`): `y0 = ~src[2].lo = ~b = 4`; `cv[2]` is out of range,
    so `cv` keeps `xxx2`. `a = h.arr[1].hi = 1`, `c = src[2].hi = a`, and
    `pw[2].hi = 5` leaves `pw[0]`: `xx5xxx5x`.
  - t3 (`k = 1`, `x = 6`): `cv[1].lo = 4` gives `x4x2`; `pw[1].hi = 6` gives
    `xx5x6x5x`.
  - t4: forcing `wn[1].lo = f` gives `9f76`. The write of `src[1].hi` does
    not wake `@(src[i].lo)`, armed at time 2 with `i = 1`.
  - At time 4 `src[1].lo = 4` wakes it (`event src[1].lo 4`), and releasing
    `wn[1].lo` restores `9876`; `y0` still reads `src[2]`, so `4`.
- `selector_retarget`: `pass` copies `x` to its output.
  - t1 (`i = j = k = 0`, `x = 5`, `y = 7`): `s[0].lo = 5` (`x5 xx`),
    `ps[0].hi = 5` (`xx5x`), `c[0].lo = 7` (`x7 xx`). `outer_t` is
    `{pair_t p, t[1:0]}` (10 bits); `m[0][0].p.hi = 7` is `0111 xxxx xx`,
    printed `1Xx`.
  - t2 (`i = k = 1`): the port, `always_comb c[i].lo` and
    `always_comb m[j][k].p.hi` re-run for the selector alone: `s[1].lo = 5`,
    `ps[1].hi = 5` (`5x5x`), `c[1].lo = 7` and `m[0][1] = 1Xx`; the elements
    selected before keep their values.
  - t3 (`j = 1`, `x = 6`): `s[1].lo = 6`, `ps[1].hi = 6` (`6x5x`),
    `m[1][1] = 1Xx`.
  - t4 (`i = 2`, `y = 3`): `s[2]` and `c[2]` are out of range, so nothing
    changes there; `m[1][1].p.hi = 3` is `0011 xxxx xx`, printed `0Xx`.
- `procedural_sampled`: no assertion or clock exists.
  - Time 0 (Preponed, before any process): `x` is X, `d` holds its
    initializer `33`, the net `w` is Z before its continuous assignment runs,
    and `x[3:0]` is X.
  - Time 1: `x = 5a`, `w = 5b`, `s = -2`, `r > 1.0` is `1`, and
    `ps[i].hi` with `i = 0` is `ps[0].hi = 3`. Writes later in the same slot do
    not change any sampled value (`d` stays `33`, `i` stays `0`), while the
    live `x` is `11`; the next delta (`#0`) is the same slot.
  - Time 2: `x = 11`, `d = 44`, `w = 12`, `s = 3`, `r > 1.0` is `0`, and
    `ps[1].hi = 1`.
- Negatives (rejected by the frontend): a member of a slice, a member of a
  vector element, a runtime select in a net lvalue, and `$sampled` with two
  arguments.
- `unsupported_tagged_element_member`: a tagged-union member of a packed-array
  element is legal (§7.3.2) but needs the element's tag check; `llg` rejects
  it explicitly rather than reading it unchecked.
