# Overridden nonblocking assignments (FIN-001 O2)

Lowering drops a nonblocking update that a later update of the same constant
target overrides in the same process turn
([nba_merge.rs](../../../../src/sim/codegen/lowering/statements/nba_merge.rs)).
These cases check that the merge changes no output. Both run in both
optimizer modes through `tests/sim_nba_merge.rs`; the
IR shapes the merge produces, and the cases it must leave alone, are unit
tests next to the pass.

The expected outputs are the industry-standard results for the same sources,
and they follow from the text:

- IEEE 1800-2009 4.6 b) (`SystemVerilog-1800-2009.txt` L3442-3443):
  "Nonblocking assignment updates shall be performed in the order the
  statements were executed (see 10.4.2)."
- 10.4.2 (L13170-13173): "The order of the execution of distinct nonblocking
  assignments to a given variable shall be preserved. In other words, if there
  is clear ordering of the execution of a set of nonblocking assignments, then
  the order of the resulting updates of the destination of the nonblocking
  assignments shall be the same as the ordering of the execution (see 4.6)."
  So the last update of a variable in one process turn gives its value.
- 21.2.3 (L36516-36517): `$monitor` displays "each time a variable or an
  expression in the argument list changes value"; L36531-36532: "If two or
  more arguments change value at the same time, only one display is produced
  that shows the new values."

`overridden_updates.sv`: a default with a conditional override (`s`), an
update of another variable between two updates of `q` that reads `q` before
either commits (`r`), a variable that returns to its value within one batch
(`t`, whose `@(t)` counter `n_t` only counts the time-0 assignment), adjacent
updates (`w`), nested branches (`w2`), a `case` without a default arm (`w3`),
a loop with a run-time bit index (`v`, not merged) and a partial update
between two whole updates (`w4`). Each line is printed at the falling clock
edge, after the batch of the rising edge.

`observed_updates.sv`: `$monitor`, `$strobe` and event controls around
overridden updates. A monitored variable keeps each update (its report is
marked at each write); the others merge. `q` and `k` return to their values
within each batch, so their event controls wake only at time 0 and no report
shows the dropped values. Under T1-D11 (`docs/lrm_decisions.md`) the waiters
see the value at the check either way.
