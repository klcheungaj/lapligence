# RTL-012 strength resolution and unconnected drives

IEEE 1364-2001 §§3.4, 3.7.4, 6.1.4, 7.1.2, 7.4, 7.9–7.13, 17.1.1.5 and 19.9
and IEEE 1800-2009 §§10.3.4, 21.2.1.5, 22.9 and 28.11–28.15 supply the
oracles. `scalar_matrix.v` and `wide_composition.sv` are checked against the
exhaustive outcome oracle in `tests/sim_feature_completion/rtl_012/oracle.rs`
(every combination of each source's strength levels, hull of the outcomes).
The other `.out` files are hand-derived. Positive sources run through the
public CLI in both optimizer modes and on the legacy and compact value
backends.

- `scalar_matrix.v`: 55 driver pairs on wire, wand, wor, tri0 and tri1 scalar
  nets, from default, supply, pull, weak, mixed and highz-sided continuous
  strengths and `bufif1` gates with an unknown enable (L/H), over all sixteen
  0/1/x/z value pairs. It was produced by a generator kept with the task
  evidence; the source order is `SOURCES` in `rtl_012.rs`. Runs in both
  editions.
- `wide_composition.sv`: 70-bit nets (two limbs) combine an omitted
  `unconnected_drive pull1` input with a `bufif1 (strong0, weak1)` instance
  array, an omitted wand input with `pulldown (weak0)` and `buf (pull0, pull1)`
  arrays, and a plain wire with two default-strength drivers.
- `tristate_lh.v`: unknown enables give StL/StH/WeH/PuL, a highz-sided `and`
  gives StH, and L/H combine with pull, weak and net-array-cell drivers
  (650, 651, 630, 36X).
- `strength_wakeups.sv`: Pu1/St1 and Pu0/St0 strength-only changes reach a
  `%v` monitor while `@` waiters stay asleep; an unchanged reassignment wakes
  nobody.
- `directive_lifetime.sv` with `directive_pull0.svh`: an included
  `unconnected_drive pull0`, `resetall`, `nounconnected_drive`, wire, tri0,
  tri1, wand, supply1 and variable formals, an internal strong driver, a
  parameterized instance array and a generated instance.
- `units_pull.sv` + `units_main.sv`: separate compilation units do not inherit
  an open directive; merged units do.
- `unconnected_array.sv` adopts FND-002's L-F11-05-01 witness;
  `unconnected_aggregates.sv` pulls omitted net-array formals per cell beside
  internal drivers and resolves a connected 2-D net-array input per cell.
- Negatives: strengths on supply nets (continuous and declaration forms), an
  explicit strength on a bit of a vector net, `(highz1, highz0)` and a
  `pullup (strong0)`.
