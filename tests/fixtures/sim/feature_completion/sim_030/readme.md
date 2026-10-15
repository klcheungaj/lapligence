# SIM-030 fixtures: dist, solve...before, randc and random operators

Plain IEEE 1800-2009 SystemVerilog with top `tb`, run by
`tests/sim_feature_completion/sim_030.rs` in both optimizer modes and, for
the positive cases, on every value backend. Line numbers refer to
`SystemVerilog-1800-2009.txt` (`pdftotext -layout` extraction, see
`docs/lrm_decisions.md`).

Positive fixtures print relations every legal solution satisfies (`bad=0`,
permutation bitmaps, values the constraints force) and frequencies that the
fixture itself compares with bounds fixed before the run: `near(hits, n,
num, den)` is 1 when `hits / n` lies within five standard deviations of
`num / den`. The expected output therefore does not depend on the values
drawn, only on the distribution.

| Fixture | Checks | Clause |
| --- | --- | --- |
| `dist_weights` | `:=` and `:/` weights (1:3 and 3 spread over a range), a weight read from state, a zero weight acting as a constraint, another constraint removing an item while the other weights keep their ratio, a dist under `if` reweighting only where the guard holds (the guard variable stays uniform, S30-D1), overlapping items adding their weights, signed ranges | 18.5.4 L29675-29678 "Absent any other constraints, the probability that the expression matches any value in the list is proportional to its specified weight. If there are constraints on some expressions that cause the distribution weights on these expressions to be not satisfiable, implementations are only required to satisfy the constraints. An exception to this rule is a weight of zero, which is treated as a constraint."; L29684-29687 "The := operator assigns the specified weight to the item or, if the item is a range, to every value in the range. The :/ operator assigns the specified weight to the item or, if the item is a range, to the range as a whole. If there are n values in the range, the weight of each value is range_weight / n." |
| `solve_before` | the LRM example (`s` true half the time, the solution space unchanged), an order in a disabled block ignored, orders in inline constraints and `std::randomize`, a three-variable chain solved layer by layer (S30-D2) | 18.5.9 L30053-30054 "the order constraint instructs the solver to solve for s before solving for d. The effect is that s is now chosen true with 50% probability, and then d is chosen subject to the value of s"; L30058-30059 "a “solve...before...” constraint does not change the solution space"; L30079-30080 "Variables that are not explicitly ordered shall be solved with the last set of ordered variables." |
| `randc` | interleaved objects keep separate cycles; every cycle is a permutation of the legal values; changing the constraints (`constraint_mode`) starts a new cycle; an inactive variable and a failed call leave the cycle unchanged; an inherited and an own randc variable cycle independently; a static randc variable has one cycle for all instances; 256 draws of an 8-bit randc are a permutation (S30-D3) | 18.4.2 L29428-29429 "no value is repeated within an iteration. When the iteration finishes, a new iteration automatically starts"; L29442-29443 "The permutation sequence for any given randc variable is recomputed whenever the constraints change on that variable or when none of the remaining values in the permutation can satisfy the constraints."; L29453-29454 "If a random variable is declared as static, the randc state of the variable shall also be static." |
| `random_ops` | random divisors, random bit and indexed part-select indices, `**` with random operands (including a negative exponent): assignments whose value would be x are not solutions, and an implication guard absorbs a zero divisor (S30-D4) | 11.4.2 L14532-14533 "For the division or modulus operators, if the second operand is a zero, then the entire result value shall be x."; 11.5.1 L15825 "If the value of addr is out of bounds, then vect[addr] returns x."; 11.4.3 L14542-14543 "The result value is 'x if the first operand is zero and the second operand is negative." and Table 11-4 |
| `shadowing` | a derived property hiding a base property of the same name: each class's constraint names its own field; in an inline constraint a name resolves in the object's class first, other names in the caller's scope; an inherited randc variable constrained by the derived class | 18.7 L30539-30552 "The scope for resolution of variable names referenced in an unrestricted constraint block begins with the randomize() with object class ... Then, if a name fails to resolve within the randomize() with object class, the name is resolved normally starting in the scope containing the inline constraint."; L30583 "x is a member of class C1 and hides the x in class C2" |

Negative fixtures (`neg_*`) are located compile errors: a randc variable
wider than 16 bits (18.4.2 L29446-29447 "implementations may impose a limit on
the maximum size of a randc variable, but it shall be no less than 8 bits"),
a dist weight or item bound that reads a random variable (S30-D10), and the
later-edition `default` dist item rejected under sv2009 (A03).

Run-time error fixtures (`rt_*`) exit with status 1:

| Fixture | Clause |
| --- | --- |
| `rt_negative_weight` | S30-D8: a negative dist weight is an error; the call returns 0 and the variable keeps its value |
| `rt_circular_solve_before` | 18.5.9 L30077-30078 "There shall be no circular dependencies in the ordering"; the cycle is checked over the enabled blocks, so disabling one block makes the call succeed |
