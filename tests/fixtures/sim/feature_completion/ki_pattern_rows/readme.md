# Positional pattern rows and for-step overloaded updates

IEEE 1800-2009 supplies the oracles: §10.10 for positional pattern lvalues
(the source's elements are taken in declaration order, from the left bound,
and a target that is itself an unpacked row receives a whole row), §10.9.1
for `default:` items (a scalar default fills every nested element), §10.3
with Table 10-1 for continuous pattern targets (constant selects), §12.7.1
for `for` steps (their values are discarded) and §11.11 for overloaded
operators. Every `.out` file is derived by hand below; none were captured
from `llg`.

## `dense_rows` (small dense sources)

`w[i][j] = 16i + j`, so `w[0] = 00 01 02 03` and `w[1] = 10 11 12 13`.

- `'{c, d} = w`: `c = w[0]`, `d = w[1]`.
- `wd [1:0][3:0]` holds the same values. Its first row in declaration order
  is `wd[1]`, read `wd[1][3]` first. So `cd [0:3]` gets `cd[0] = 13` and
  `cd[3] = 10`; `dd [3:0]` gets `wd[0]` with `dd[3] = 03` and `dd[0] = 00`.
- `'{c, d} = two_t'{d, c}` reads the source first: `c = 10..13`,
  `d = 00..03`.
- `pw[i][j] = 32i + j + 1` (packed `[3:0][1:0]` elements): `pa = 01 02 03`,
  `pb = 21 22 23`.
- `w3[i][j][l] = 64i + 16j + l`. `'{'{a, b}, '{e, f}}` takes
  `a = w3[0][0]` (`00`, `02`), `b = w3[0][1]` (`10`, `12`), `e = w3[1][0]`
  (`40`, `42`), `f = w3[1][1]` (`50`, `52`).
- `'{'{a, '{h0, h1, h2}}, g}` mixes a row, packed leaves and a 2-D row:
  `a = w3[0][0]`, `h = 10 11 12`, `g[0][0] = w3[1][0][0] = 40`,
  `g[1][2] = w3[1][1][2] = 52`.
- Typed pattern `'{1, 2, 3, 4}, '{5, 6, 7, 8}`: `c[0] = 01`, `c[3] = 04`,
  `d[0] = 05`, `d[3] = 08`. Items `'{q, p}` give `c = b0..b3`, `d = a0..a3`.
- `mk(32)` returns `32 + 16i + j`: `c = 20..23`, `d = 30..33`.
- `two_t'{default: 8'h5a}` fills every cell with `5a`.
- The unpacked record `s = '{r: p, x: 9}` scatters into `c = a0..a3` and
  `x = 9`.
- `w[0][1] = xxxx0101` (`x5`), then `'{c, d} <= w`: before the NBA,
  `c[1] = a1` (record step) and `d[1] = 5a` (default step); after it
  `c[1] = x5`, `d[1] = 11`, `d[3] = 13`.
- `'{w[next_k()], c} = wd` with `k = 0`: the selector runs once (`k = 1`),
  `w[1]` takes `wd[1]` in declaration order (`w[1][0] = 13`,
  `w[1][3] = 10`) and `c[0] = wd[0][3] = 03`.

## `descriptor_rows` (more than 4,096 cells on one side)

`big [2000][4]` (8,000 cells), `src [2][2][3000]` (12,000 cells) and
`rows [8][3000]` (24,000 cells) are descriptor storage.

- `'{big[k], d} = w` with `k = 5`: `big[5] = 00 01 02 03`, `d = 10..13`.
- `'{big[next_k()], '{h0, h1, h2, h3}} = w`: `k = 6` once, `big[6] = w[0]`
  (`00`, `03`), `h = 10 11 12 13`.
- `'{'{h3, h2, h1, h0}, big[k]} <= w`: before the NBA `h0 = 10` and
  `big[6][3] = 03`; after it `h0 = 03`, `h1 = 02`, `h2 = 01`, `h3 = 00`,
  `big[6] = w[1]` (`10`, `13`).
- `'{big[1999], big[0]} = two_t'{default: 8'h5a}` fills both rows with `5a`;
  `big[1][0]` was never written and reads `xx`.
- `'{d, big[1999]} = two_t'{'{1, 2, 3, 4}, d}` evaluates the source before
  any write: `d = 01..04`, `big[1999] = 10..13` (old `d`).
- `src[i][j][l] = (64i + 16j + l) mod 256`. `'{'{a, b}, '{e, f}} = src`:
  `a[0] = 00`, `a[2999] = 2999 mod 256 = b7`; `b[1] = 11`,
  `b[2999] = 3015 mod 256 = c7`; `e[2] = 42`, `e[2999] = 3063 mod 256 = f7`;
  `f[3] = 53`, `f[2999] = 3079 mod 256 = 07`.
- Descriptor rows of `rows` take the same source rows in the same order.
- The nested NBA gives `a[4] = 04`, `rows[1][5] = 16 + 5 = 15`,
  `rows[2][6] = 64 + 6 = 46`, `f[7] = 80 + 7 = 57`.

## `continuous_rows`

The same `w` and `src`. Variable rows (`vc`, `vd`), net rows (`nc`, `nd`),
net leaves (`n0..n3`), a descriptor row (`big[7]`, `big[8]`), dense rows of a
descriptor source and a descriptor row (`rows[6] = src[1][1]`) each drive one
row. A typed item source `two_t'{w[1], w[0]}` swaps the rows into `nd` and
`big[8]`. After `w[0][1] = dd`, `w[1][2] = ee`, `src[1][1][5] = aa` and
`src[0][0][0] = bb`, every driver re-evaluates: `vc[1] = nc[1] = big[7][1] =
big[8][1] = dd`, `vd[2] = n2 = h2 = nd[2] = ee`, `a[0] = bb`,
`rows[6][5] = aa`.

## `for_step_updates`

`inc` adds `(1, 2)` to `(a, b)`, `add` adds the records, `ninc`/`ndec`
append `+`/`-` and add `+1`/`-1`, `nadd` appends `*` and adds its integer,
`vinc` adds 1 to every element and `vadd` adds its integer.

- Packed: three `x++` steps from `(1, 10)` give `(4, 16)` with `i = 3`; two
  `x += (5, 1)` steps give `(14, 18)`; two `y = x++` steps (a value form)
  give `x = (16, 22)`, `y = (15, 20)`; two `sa[1]++` steps from `(7, 3)` give
  `(9, 7)`.
- Native: three `n++` steps from `("a", 1)` give `("a+++", 4)`; two
  `n += 5` steps give `("a+++**", 14)`; two `--n` steps give
  `("a+++**--", 12)`.
- Descriptor: `c[k] = k`; three `c++` steps give `c[0] = 3`,
  `c[65536] = 65539`; two `c += 10` steps give `c[1] = 24`,
  `c[65536] = 65559`.
- Expression statements under `if`, `case`, `begin`, `fork` and a task:
  `n++`, `n += 2`, `n++`, `n++` append `+*++` and add 5 (`17`); `c++` and
  `c += 1` give `c[0] = 25`, `c[65536] = 65561`.

## Negatives and limits

- `neg_row_shape`: a 3-element row target for a 4-element source row is
  rejected by the frontend.
- `neg_row_runtime_continuous`: a runtime-selected continuous row target is
  illegal (Table 10-1), also for a descriptor row.
- `neg_row_automatic_nba`: an automatic row cannot take a nonblocking write
  (§6.21).
- `limit_descriptor_step_selector`: a for-step update whose target selector
  has side effects binds its target once, which needs a packed-capacity
  target; a descriptor-sized row keeps the documented limit, as in an
  expression statement. (Arrays of records with native members are not
  admitted at all, so a native element cannot reach this form.)
