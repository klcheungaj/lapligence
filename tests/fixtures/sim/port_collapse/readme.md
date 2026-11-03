# Port collapse fixtures

Run by `tests/sim_port_collapse.rs` in both optimizer modes. Structural
checks (link processes, resolved nets) are in the same file; the decision
cases are `A21-D1`..`A21-D5` in `tests/fixtures/sim/lrm_decisions/`.

Clauses (SystemVerilog-1800-2009.txt):

- 23.3.3 L41452-41454: "Each port connection shall be a continuous assignment
  of source to sink, where one connected item shall be a signal source and the
  other shall be a signal sink. The assignment shall be a continuous assignment
  from source to sink for input or output ports."
- 23.3.3.1 L41461-41462: "A port that is declared as input (output) but used as
  an output (input) or inout may be coerced to inout. If not coerced to inout, a
  warning shall be issued."
- 23.3.3.7 L41592-41598: "When different net types are connected through a
  module port, the nets on both sides of the port can take on the same type.
  The resulting net type can be determined as shown in Table 23-1. [...] It is
  permissible to merge the dominating and dominated nets into a single net,
  whose type shall be that of the dominating net. The resulting net is called
  the simulated net, and the dominated net is called a collapsed net."
- Table 23-1 L41613-41649: the rows of internal `tri0`, `tri1`, `supply0` and
  `wor` read "internal" under an external `wire`; the `wire` row reads
  "external" in every column; the `wor` row reads "external, warn" under an
  external `wand`; "external = The external net type shall be used."
- 10.6.2 L13397-13399: "A force procedural statement on a net shall override
  all drivers of the net-gate outputs, module outputs, and continuous
  assignments-until a release procedural statement is executed on the net."
- 6.6.1 / 28.16 (net delays) are not restated: `delayed` shows the formal's own
  net delay, which a merge would drop (23.3.3.7 gives the simulated net the
  dominating net's delay).

## `merged.sv`

Sinks that merge with their source (A21-D1 input variable, A21-D2 net sinks,
A21-D3 output variable).

- `input chain: u.d=1 u.l.d=1`: `d` drives the implicit-net input of `mid`,
  which drives the `wire` input of `leaf`; both formals are merged with `d`
  and show its new value in the writing process (A21-D2).
- `output net: u.l.q=1 q=1`: the `q` nets of `mid` and `tb` are driven only by
  the `q` outputs below them and merge with `leaf.q`, whose continuous
  assignment has run by the `#0`.
- `output variable: r=1 tb.r=1`: `tb.r` shares `leaf.r` through `mid.r`
  (A21-D3), so the child's own write is visible through `tb.r` at once.
- With `--vpi` a port whose source is a variable stays a continuous
  assignment, since a `vpi_put_value` on the sink must not reach the variable
  (A21-D6): `u.d=0 u.l.d=0` (`u.d` follows `d` one update later and `u.l.d`
  shares the net `u.d`) and `tb.r=x`; the net ports of the `#0` line merge
  as without VPI, so it is unchanged.

## `merged_nets.sv`

Net ports whose sink has drivers or a type of its own merge with their source
into one simulated net (A21-D4, 23.3.3.1 and 23.3.3.7); `--vpi` gives the same
output, since a model that keeps the VPI catalog merges these ports too.

- `coerced: c=x u0.n=x`: the parent drives `c` with 1 and the child drives its
  input `n` with 0; the merged net resolves both to x under both names.
- `weak child driver: cw=1 St1 u1.n=1 St1`: the child's weak 0 loses to the
  parent's strong 1 on the merged net.
- `vector: vec=11xx u2.v=11xx`: the child's constant part-select driver of
  bits 1:0 meets the parent's 1s there; bits 3:2 keep the parent's value.
- `output on wand actual: wn=0 u3.o=0`: the external `wand` dominates the
  `wire` output (Table 23-1 "external"), so the child's 1 and the parent's 0
  give 0 on both names.
- `wor output on wire actual: nq=1 St1 u4.o=1`: the internal `wor` dominates
  the external `wire` ("internal"); 1 or 0 is 1.
- `wire output on tri0 actual: et0=0 Pu0 u5.o=0 Pu0`: the undriven output
  takes the external `tri0` pull-down.
- `table over a wor formal: wa=0 u6.a=0`: Table 23-1 reads "external, warn"
  for an internal `wor` under an external `wand`, so the merged net is a
  `wand` of the parent's 1 and 0 and the child's 1. Some simulators resolve
  this pair as `wor` (both names 1); llg follows the table.
- The wait on `u0.n` wakes at 1, 2 and 3 on changes of the parent's driver
  `x`, the force of `c` and its release; `forced actual: c=1 u0.n=1` shows
  the force through the parent's name reaching the child's name.
- `x=0: c=0 u0.n=0 vec=0000 u2.v=0000`: both sides now drive 0.

## `merged_selects.sv`

Net ports whose actual is a constant bit or part select, a net-array element
(or a part of one) or a concatenation of nets merge with their formal bit by
bit, MSB to MSB, where a copy would show something else (A21-D5, 23.3.3 and
23.3.3.7: the connected bits become one simulated net). The output is the same with `--vpi` and in both optimizer
modes.

- `bit select: v=1xx1 u0.n=x StX`: the child drives its input `n` with 0, the
  parent drives `v[1]` with 1; the merged bit is x under both names. Bit 2 is
  x too because `u14.g` drives the input below `u14.m` (`chain` line).
- `disjoint and overlapping: o=x1xx u1.n=x u2.n=x u3.n=1x`: `u1` merges with
  `o[0]` and `u2` with `o[3]`, each driving 0 against the parent's 1; `u3`
  merges with `o[2:1]` and drives only its bit 0 (`o[1]`), so `o[2]` keeps the
  parent's 1.
- `pulled bit: t=11 u4.n=1 Pu1`: the formal of the undriven `tri1` bit shows
  the pull strength, which a copy would turn into a strong 1.
- `tri0 formal: d=z0zz Pu0 u5.n=Pu0`: the internal `tri0` dominates the
  external `wire` bit (Table 23-1 "internal"), so `d[2]` is pulled down.
- `array element: w0=1 w1=x u6.n=x`: the child's 0 on the formal of `w[1]`
  meets the parent's 1; `w[0]` is unaffected.
- `array element part: wa0=111x u7.n=1x`: bit 0 of `wa[0]` meets the child's
  0 on `n[0]`.
- `concatenation: c=1 e=x u8.n=1x`: `{c, e}` pairs `c` with `n[1]` and `e`
  with `n[0]`, which the child drives.
- `output bits: a=z11z St1 We1`: the weak child driver of `a[2]` merges and
  stays weak; `a[1]`, driven strong by its port alone, merges too and shows
  the child's strong 1.
- `second driver: b=zzxz u11.o=x`: the parent's 0 on `b[1]` and the child's 1
  resolve to x under both names.
- `output concatenation: g=1 h=0 u12.o=10`: the child's `2'b10` splits MSB to
  MSB.
- `output element: w2=St1 u13.o=1`: the child drives net-array cell `w[2]`;
  the port selects the whole cell, so it merges although a copy would be
  exact.
- `chain: v=1xx1 u14.m=x u14.g.n=x`: the whole port of `mid` into `coerced`
  joins the merged bit, so the grandchild's 0 reaches `v[2]`.
- `forced formal: f=1101 u15.m=0 u15.k=0`: a force of the formal overrides
  every driver of the merged net (10.6.2), so `f[1]` shows it too; after the
  release `f=1111 u15.m=1`.
- `x=0: ...`: every merged bit follows the parent's 0 once both sides agree.

## `plain_selects.sv`

Plain selected input and output ports, whose copy would show the same values
one update later, merge with their formal too (A21-D5): the connected bits are
one simulated net (23.3.3.7 L41596-41598, "It is permissible to merge the
dominating and dominated nets into a single net"), so the formal changes in
the same update as the actual. The output is the same with `--vpi` and in both
optimizer modes, and matches the reference simulator. Each time step has one
printing process, and time 0 prints nothing, because the order of processes
woken in one step is open (4.7 L3465-3466: "active events can be taken off the
Active or Reactive event region and processed in any order").

- `1 v=0110 u.a=1 u.p=01 u.q=00`: `u.a` is `v[1]`, `u.p` is `v[3:2]` and
  `u.q` is `{v[0], v[3]}`; the parent's process that wakes on `v` already
  sees the formals' new values (a copy showed `u.a=0 u.p=00`).
- `strength: St1 St1 St0 St0`: every name of a strong driver's bit shows
  strong; the child drives `w[2]` through `o` with 0.
- `3 w=z1z1 u.o=1`: the child's driver of `o` reaches `w[2]`; `w[0]` keeps the
  parent's 1 and the undriven bits stay z.
- `4 v=1001 u.a=0 u.p=10 u.q=11`: a force of the actual `v` overrides its
  driver (10.6.2 L13397-13399) and every formal shows the forced bits.
- `5 ...`, `6 ...`: after the release the driver's `0110` returns, then
  `1000` follows.
- `wakes=4 ...`: `u.a` changed at 1, 4, 5 and 6.

## `fallbacks.sv`

Ports that keep their continuous assignment because a merge would drop or
change something (A21-D2, A21-D4 and A21-D5 exceptions).

- `width: wide=1111 u2.w=11`: the 2-bit formal of a 4-bit actual is a
  converting copy.
- `variable actual: vr=1 u4.n=x`: a variable is no net to merge with; the
  formal resolves the port's 1 with the child's 0.
- `delay: dl=1 u3.n=x` then `delay later: dl=1 u3.n=1`: the formal's own
  `#2` net delay applies to the copy.
