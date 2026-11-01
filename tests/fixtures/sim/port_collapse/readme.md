# Port collapse fixtures

Run by `tests/sim_port_collapse.rs` in both optimizer modes. Structural
checks (link processes, resolved nets) are in the same file; the decision
cases are `A21-D1`..`A21-D3` in `tests/fixtures/sim/lrm_decisions/`.

Clauses (SystemVerilog-1800-2009.txt):

- 23.3.3 L41452-41454: "Each port connection shall be a continuous assignment
  of source to sink, where one connected item shall be a signal source and the
  other shall be a signal sink. The assignment shall be a continuous assignment
  from source to sink for input or output ports."
- 23.3.3.1 L41461-41462: "A port that is declared as input (output) but used as
  an output (input) or inout may be coerced to inout. If not coerced to inout, a
  warning shall be issued."
- 23.3.3.7 L41596-41598: "It is permissible to merge the dominating and
  dominated nets into a single net, whose type shall be that of the dominating
  net. The resulting net is called the simulated net, and the dominated net is
  called a collapsed net."
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
- With `--vpi` every port stays a continuous assignment: `u.d=0 u.l.d=0` and
  `tb.r=x` (the old values, one update later); the `#0` line is unchanged.

## `fallbacks.sv`

Ports that keep their continuous assignment (A21-D2 exceptions); the output
is the same as before net ports merged.

- `coerced: c=1 u0.n=x`: `coerced` drives its input net (a port 23.3.3.1 may
  coerce): the formal resolves the port's 1 with its own 0; the actual keeps
  its single driver.
- `second driver: m=x u1.o=1`: `m` has its own `assign m = 1'b0` besides the
  output port; the formal keeps only its own driver.
- `types: z=z u2.t=0 wo=1 u2.a=1 u2.w=11`: the `tri0` formal pulls its
  undriven copy to 0 (the actual stays z); the `wand` formal and the 2-bit
  formal of a 4-bit actual are copies.
- `delay: dl=1 u3.n=x` then `delay later: dl=1 u3.n=1`: the formal's own
  `#2` net delay applies to the copy.

Some simulators also merge the coerced, multiply driven and `tri0` cases
(both names then show `x`, `x` and `0`); llg keeps those ports as continuous
assignments (see `docs/known_issues.md`).
