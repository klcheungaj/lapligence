# Range selects of multidimensional packed values

IEEE 1800-2009 §7.4.5 and §11.5.1 supply the oracle: a part-select or indexed
part-select of a packed array addresses whole elements of the dimension it is
applied to, and a select of a packed array of structures or unions addresses
whole structures. §11.5.1 also gives X for read bits that are out of range or
addressed by an X/Z index, and no write for those bits. §7.4.1 makes an element
of a named signed type signed, and a part-select unsigned. Expected values are
derived by hand below; none were captured from `llg`.

- `procedural_selects`: `w = 32'h44332211` is `logic [3:0][7:0]`, so `w[3]`
  is `44` and `w[0]` is `11`.
  - Constant ranges: `w[3:2] = 4433`, `w[1:0] = 2211`, `w[2 +: 2] = w[3:2]`,
    `w[2 -: 2] = w[2:1] = 3322` and `w[0 +: 1] = 11`.
  - Ascending `a [0:3]` holds `a[0] = 44` to `a[3] = 11`. That gives
    `a[0:1] = 4433` and `a[2:3] = 2211`; both `a[1 +: 2]` and `a[2 -: 2]`
    are `a[1:2] = 3322`.
  - `n [4:1][3:0] = 16'h4321` has `n[4] = 4` to `n[1] = 1`. So `n[3:2] = 32`,
    `n[4 -: 2] = 43` and `n[1 +: 3] = n[3:1] = 321`.
  - `x [2:0][1:0][3:0] = 24'h654321` has `x[2] = 65`. So `x[2:1] = 6543`,
    `x[1][0] = 3`, `x[2][1:0] = 65`, and `x[0][1] = 2` gives `[3:2] = 0`.
  - Runtime indices (`i = 1`, `j = 2`) give the same elements as the
    constant forms.
  - Out of range: `w[3 +: 2]` covers elements 4 and 3, so it reads `xx44`,
    and `w[4]` reads `xx`. `w[-1 +: 2]` reads `11xx`. `a[-1 +: 2]` is
    `a[-1:0]`, so it reads `xx44`.
  - An X index reads all X.
  - Signed elements: `se [1:0]` of `logic signed [3:0]` with value `8'h8f`
    gives `se[1] = -8`, `se[0] = -1`, and the unsigned part-select
    `se[1:0] = 143`.
  - Records: `ps [3:0]` of 8-bit structures holds `ps[2:1] = 3322`,
    `ps[3] = 44` and `ps[0] = 11`. `us [1:0]` of 8-bit unions holds
    `us[1:1] = 12` and `us[0] = 34`. Two-state `tw` selects like `w`.
  - Writes, applied in order:
    - `w[2:1] = bbaa` gives `44bbaa11`.
    - `w[3 -: 2] = ddcc` gives `ddccaa11`.
    - `w[0 +: 2] = ffee` gives `ddccffee`.
    - `w[1][5:2] = 0` clears bits 5..2 of `ff`, giving `c3` and `ddccc3ee`.
    - `w[3 +: 2] = 1234` writes only element 3 (`34`).
    - `w[-1 +: 2] = 5678` writes only element 0 (`56`).
    - An X index writes nothing, so the result is `34ccc356`.
  - Ascending writes: `a[1:2] = 0102`, then `a[2 -: 2] = 0a0b`, giving
    `440a0b11`.
  - Nested writes on `x = 654321`: `x[1] = ab` gives `65ab21`, `x[0][1] = f`
    gives `65abf1`, `x[2][0] = 0` gives `60abf1`, and `x[2][0 +: 2] = 9c`
    gives `9cabf1`.
  - Record writes: `ps[1:0] = bbaa`, then `ps[2] = 0f`, giving `440fbbaa`.
  - NBAs from `w = 0`: `w[2:1] <= beef` and `w[3 -: 1] <= 5a` give
    `5abeef00`.
- `continuous_ports`: `w = 44332211`, `src = 6655` and `i = 3`.
  - `v[1:0]` is the inverter output `~w[3:2] = bbcc`, and
    `assign v[3:2] = w[1:0]` gives `v = 2211bbcc`.
  - Two continuous drivers of disjoint element ranges give
    `nw = {src, w[1:0]} = 66552211`, and `mid` aliases
    `nw[2:1] = 5522`.
  - `na[1] = w`, so `slot = na[1][3:2] = 4433`.
  - `always_comb` gives `mon = w[2:1] = 3322` and `pick = w[3] = 44`.
  - After `w[2:1] = 7788` and `i = 1`, `w` is `44778811`:
    - `v = {8811, ~4477} = 8811bb88` and `nw = 66558811`.
    - `mid = 5588` and `slot = 4477`.
    - `mon = 7788` and `pick = w[1] = 88`.
  - Forcing `nw[2:1] = f00d` gives `66f00d11` (and `mid = f00d`).
    Releasing it restores the drivers.
- `modport_ranges`: `.p(w[3:2])` is numbered `[3:2]` (§25.5.4) and
  `.lane(w[2])` is one element. From `w = 44332211`, the writer applies, in
  order:
  - `p = beef` gives `beef2211`.
  - `p[2] = 11` gives `be112211`.
  - `p[3] = 22`, with a runtime index, gives `22112211`.
  - `p[3 -: 2] = 3344` gives `33442211`.

  The `always @(b.p)` reader reports each change; the samples at odd times
  avoid racing the writer.
- `subroutine_views`: `w[1] = 665544332211`, and the module `ref` ports pass
  `w[1][4:0]`, then `[3:0]`, so `leaf` sees `44332211`.
  - `value[3:2] = bbaa`, then `value[0 +: 2] = ddcc`, then
    `value[1][3:0] = 0` give `bbaad0cc`. The reads are `value[2:1] = aad0`
    and `value[0] = cc`, so `w[1] = 6655bbaad0cc`.
  - `pick` copies `[3:2]` into `[1:0]` of an automatic local
    (`44334433`), so both `[3 -: 2]` and `[1 -: 2]` are `4433`.
  - `poke` writes `v[1 +: 2] = 0102` and `v[3] = ee` through a `ref` formal,
    giving `ee010211`.
- `neg_reversed_range`: `[2:3]` of a descending dimension names its bounds in
  the wrong order (§11.5.1).
- `neg_runtime_width`: the width of an indexed part-select must be constant
  (§11.5.1).
