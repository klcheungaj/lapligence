# RTL-102 modport port expressions

IEEE 1800-2009 §25.5.4 supplies the oracle: a modport expression port
`.p(expr)` names its port expression in the connected interface instance, has
the expression's self-determined type (so `.p(r[7:4])` is numbered `[7:4]`),
and is a valid target only when the expression is an lvalue. §6.5 and §6.6.1
govern writers of the underlying storage. Expected values are derived by hand
below; none were captured from `llg`.

- `probe_p02`: the RTL-099 audit probe. `data[3:0] = 5` procedurally and
  `assign b.hi = 4'ha` through `.hi(data[7:4])` give `y = 5`, `data = a5`.
- `continuous_views`: continuous reads/writes over variables (`data`, `lane`)
  and a net (`bus`, two modules each drive one half) through child-port
  actuals, an interface-array element, a forwarding module, a generate-scoped
  instance and the hierarchical `r.b.nib`. Element 1: `hi = v = 9`, so
  `data = 97`; `nib = 7`; `lane[1] = 3`; `top = v ^ f = 6`, `bot = u = 5`, so
  `bus = 65`; `lane[0] = ~9 = 6`. Element 0 (generate): `hi = 1`, `data = 12`,
  `top = e`, `bot = 6`. After `data[3:0] = e`, `v = 0`, `u = a`: `data = 0e`,
  `top = f`, `lane[0] = f`, `bus = fa`.
- `procedural_views`: `always_comb`, `@(b.bit2)`, NBA and blocking writes
  through expression ports. `.sum(data[3:0] + data[7:4])` is 4 bits, so
  `s = 8'(1 + 2) + K = 06`, then `4 + 2 + 3 = 09`, then `4 + b + 3 = 12`.
  `.ascp(asc[2:5])` of `0011_1100` is `1111` and `ascp[3]` is `asc[3] = 1`.
  `b.wd[i]` selects 8-bit elements at run time (`22`, then `44`, then `be`).
  The posedge NBAs give `data = b4` and `{a, m, c} = 1011`. The negedge
  writes: `asc[0:3] = 1010` then `asc[1] = 1` (`1110_1100`); words `be`,
  `[2][3:0] = 5` (`35`), runtime `[j=0] = 91` then `[0][0 +: 4] = 3` (`93`);
  `hi[5]` (port range `[7:4]`) clears `data[5]` (`b4 -> 94`); `mem[1] = 77`;
  `cat[1]` is `m[0]` (`1001`).
- `inout_views`: `.io(pins[1:0])` is one more driver of interface net bits.
  Enabled with `v = 10`: `pins = 0z10` (`pins[3] = 0`, `pins[2]` undriven,
  `pins[0]` released by `tb`). Disabled: `io` floats and `tb` drives
  `pins[0] = 1`, so `pins = 0zz1` and `io` reads `z1`.
- `neg_output_not_lvalue`, `neg_input_write`, `neg_runtime_selector`: the
  frontend rejects a non-lvalue output expression, a write through an input
  port and a non-constant selector in a port expression.
- `neg_continuous_and_procedural`, `neg_two_continuous`,
  `neg_concat_select_conflict`: writers through expression ports overlap
  another writer of the same variable bits (§6.5); bit 1 of
  `.p({a[1:0], c})` is `a[0]`.
- `neg_virtual_interface_expression_port`: deferred; reading an expression port
  through a virtual interface handle reports a clear diagnostic.
