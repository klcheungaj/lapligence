# SIM-031 randcase weighted procedural choice

IEEE 1800-2009 18.16 is the oracle. Line numbers refer to the `pdftotext
-layout` extraction `SystemVerilog-1800-2009.txt` (SV). The suite
`tests/sim_feature_completion/sim_031.rs` runs every positive fixture in both
optimizer modes on the legacy, compact/portable and compact/GMP value backends.
The decisions this task made are S31-D1 to S31-D7 in
[lrm_decisions](../../../../../docs/lrm_decisions.md#weighted-procedural-choice-sim-031),
with portable cases under `tests/fixtures/sim/lrm_decisions/`.

The generator behind `$urandom_range` and the thread streams is implementation
dependent, so no fixture holds a captured random value. Expected output is
either exact (a single nonzero weight, all-zero, side effects, chosen-body
control flow) or a relation (replay equality, frequency inside a statistical
bound, thread independence).

## Clauses relied on

- SV 18.16 L31310-31311: "The keyword randcase introduces a case statement
  that randomly selects one of its branches. The randcase_item expressions are
  non-negative integral values that constitute the branch weights."
- SV 18.16 L31311-31312, L31329-31330: "An item's weight divided by the sum of
  all weights gives the probability of taking that branch." (3, 1, 4 give
  0.375, 0.125, 0.5.) The frequency fixture bounds the observed counts by this
  rule.
- SV 18.16 L31332-31333: "If a branch specifies a zero weight, then that
  branch is not taken. If all randcase_items specify zero weights, then no
  branch is taken and a warning can be issued." `single_weight`, `all_zero`.
- SV 18.16 L31335-31336: "The randcase weights can be arbitrary expressions,
  not just constants."
- SV 18.16 L31346-31351: "The precision of each weight expression is
  self-determined. The sum of the weights is computed using standard addition
  semantics (maximum precision of all weights), where each summand is
  unsigned. Each weight expression is evaluated at most once (implementations
  can cache identical expressions) in an unspecified order." Decisions S31-D1
  (exact sum), S31-D2 (self-determined), S31-D6 (order).
- SV 18.16 L31353-31355: "Each call to randcase retrieves one random number in
  the range of 0 to the sum of the weights. The weights are then selected in
  declaration order: smaller random numbers correspond to the first (top)
  weight statements."
- SV 18.16 L31357-31359: "Randcase statements exhibit thread stability. The
  random numbers are obtained from $urandom_range(); thus, random values drawn
  are independent of thread execution order. This can result in multiple calls
  to $urandom_range() to handle greater than 32 bits." `thread_stability`,
  `seed_replay`, S31-D7.
- SV 18.16 L31306: "randcase_item ::= expression : statement_or_null". The
  body of a branch is an ordinary statement, so waits, forks, `return`,
  `disable`, `break` and `continue` in it have their ordinary meaning
  (`body_wait_fork`, `body_control`).

## What each fixture checks

| Fixture | Expected result | Basis |
| --- | --- | --- |
| `single_weight` | One nonzero weight (variable, constant, 32-bit maximum, signed short, single item, null body) is chosen in all 64 executions and no other body runs. | L31332 |
| `all_zero` | Zero weights from tasks, constants, all-X and X-masked operands run no body and leave execution going; 8 executions give 8 stderr warnings naming the statement. | L31332-31333 |
| `wide_weights` | Single nonzero weights of 40, 64 and 100 bits fix the choice; equal 63-, 64- and 100-bit weights both occur; two 1048574-bit weights (a 1048575-bit exact sum, the widest legal) still select. | L31311, L31346-31351 |
| `weight_side_effects` | Every weight, chosen or not, runs once per execution in source order (`123`); counter-based weights add 111; a weight sees the write of the weight above it. | L31347-31349 (llg chooses source order, S31-D6) |
| `frequency` | Five fixed seeds times five sets (3:1:0:4, 1:99, 40-bit 3:1, 100-bit 3:1, 3-bit 7 against 70-bit 14); Pearson chi-square below the p=0.001 critical value (13.82 for 2 degrees of freedom, 10.83 for 1) and no zero-weight hit. | L31311-31312 |
| `seed_replay` | The same seed gives the same 96 picks (also with 100-bit weights), another seed differs; the checksum equals across all six mode/backend runs. | L31357-31359 |
| `thread_stability` | Selections of two earlier threads and a forked child equal with and without a later thread that draws `$urandom`, `$urandom_range` and randcase. | L31357-31358 |
| `body_wait_fork` | A chosen body waits on a delay and an event, forks and joins children, calls a timed task and keeps a local across those waits; unchosen bodies never run. | statement_or_null |
| `body_control` | `return` from a function and a task body, `disable` of the enclosing named block, `break`/`continue` in a loop, a process killed by `disable` and by `disable fork` while its chosen body waits. | statement_or_null |
| `neg_real_weight`, `neg_string_weight` | Compile error `expression type 'real'`/`'string' is not integral`. | L31311 |
| `neg_sum_overflow` | Source-addressed lowering error: two 1048575-bit weights need a 1048576-bit exact sum, over the packed-value limit. | llg resource bound |

## Statistical tolerance

`frequency` draws 40000 (small, skewed), 20000 (40-bit) or 8000 (100-bit,
mixed) selections per seed. A correct implementation exceeds a p=0.001
critical value once in a thousand sets; the 25 sets use fixed seeds, so the
outcome is reproducible and a failure indicates a changed generator or a bias
(a shift of 1.5 percentage points in the 3:1:0:4 probabilities already exceeds the bound), not
noise. If the generator is replaced and a set fails, check the printed
statistic first: values just above the bound over many sets mean a different
seed is needed, a large value means a bias.
