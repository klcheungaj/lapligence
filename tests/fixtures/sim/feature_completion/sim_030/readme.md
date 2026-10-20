# SIM-030 fixtures: dist, solve...before, randc, random operators, arrays and objects

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
| `rand_arrays` | the LRM sorted-array example (sizes 1 to 10 equally likely because sizes are solved first, S30-D5), `data.size == len`, fixed arrays with ascending and descending declared indices, a queue and a four-state dynamic array, an unconstrained size kept, `rand_mode(0)` making an array state | 18.4 L29350-29352 "The size of a dynamic array or queue declared as rand or randc can also be constrained. In that case, the array shall be resized according to the size constraint, and then all the array elements shall be randomized."; L29370-29371 "If a dynamic array's size is not constrained, then the array shall not be resized and all the array elements shall be randomized."; 18.5.7.1 L29929-29931 "If an array is constrained by both size constraints and iterative constraints, the size constraints are solved first, and the iterative constraints next." |
| `array_methods` | `sum` with and without a `with` clause (the element type wraps), `product`, `and`, `or`, `xor`, `item.index`, `sum` and `product` of an empty array, 0 and 1 (S30-D6), `inside` a state array, a random index that selects only existing elements (S30-D11) | 18.5.7.2 L29939-29942 "In the context of a constraint, an array reduction method is treated as an expression iterated over each element of the array, joined by the relevant operand for each method. The result returns a single value of the same type as the array element type or, if specified, the type of the expression in the with clause."; 11.4.13 L15366-15367 "If an expression in the list is an unpacked array, its elements are traversed by descending into the array until reaching a singular value." |
| `rand_objects` | the LRM heap example over two levels and a derived node, a null rand handle with a guard, hook order (S30-D7), a non-rand handle's object as state whose own constraints do not apply, inactive variables of a member object, an object reached twice, an inline constraint on a member, per-object randc cycles, a list and a cycle back to the first object | 18.4 L29372-29374 "An object handle can be declared rand, in which case all of that object's variables and constraints are solved concurrently with the variables and constraints of the object that contains the handle. Randomization shall not modify the actual object handle."; 18.5.8 L30000-30001 "Starting with the object that invoked the randomize() method, add all objects that are contained within it, are declared rand, and are active"; L30005-30006 "select all of the active constraints from the set of active random objects"; L30008-30009 "All other variable references are treated as state variables, whose current value is used as a constant."; 18.6.2 L30445-30447 "it first invokes pre_randomize() on obj and also all of its random object members that are enabled" |
| `scope_arrays` | `std::randomize` of a dynamic array and a queue with size, `foreach`, `inside` and `sum` constraints, and of a local array in a function | 18.12 L30907-30908 "The scope randomize function, std::randomize(), enables users to randomize data in the current scope" |
| `randomize_only_targets` | a scalar, a dynamic array and a queue written only by `std::randomize`, and state read only in its constraints, keep their storage with the optimizer on | 18.12 L30907-30908 "The scope randomize function, std::randomize(), enables users to randomize data in the current scope" |
| `productions` | A03: an executable witness of every constraint production of A.1.10 (`static` and plain constraint declarations, `pure`, `extern` and unqualified prototypes, extern declarations, `solve...before` with `this.`, class-scope and plain primaries, expressions, dist items with `:=` and `:/`, implication, `if`/`else` and `foreach` with constraint sets) and of the randomize_call forms (a variable list, `null`, `with ( identifier_list )`, `std::randomize() with`) | A.1.10 L67277-67301 (`constraint_declaration ::= [ static ] constraint constraint_identifier constraint_block` to `identifier_list ::= identifier { , identifier }`); A.8.2 L68967-68970 "randomize_call ::= randomize { attribute_instance } [ ( [ variable_identifier_list \| null ] ) ] [ with [ ( [ identifier_list ] ) ] constraint_block ]" |
| `shadowing` | a derived property hiding a base property of the same name: each class's constraint names its own field; in an inline constraint a name resolves in the object's class first, other names in the caller's scope; an inherited randc variable constrained by the derived class | 18.7 L30539-30552 "The scope for resolution of variable names referenced in an unrestricted constraint block begins with the randomize() with object class ... Then, if a name fails to resolve within the randomize() with object class, the name is resolved normally starting in the scope containing the inline constraint."; L30583 "x is a member of class C1 and hides the x in class C2" |

Negative fixtures (`neg_*`) are located compile errors: a randc variable
wider than 16 bits (18.4.2 L29446-29447 "implementations may impose a limit on
the maximum size of a randc variable, but it shall be no less than 8 bits"),
a dist weight or item bound that reads a random variable (S30-D10), the
later-edition `default` dist item, `soft` and uniqueness constraints rejected
under sv2009 (A03), and the array
and object forms outside SIM-030: a random array read whole, an array of
class handles, a `foreach` over two dimensions and a constraint function
argument that reads an array element.

`dist_fallback_warning` (S30-D9) runs with `LLG_RAND_BDD_NODES=1` and
`LLG_RAND_BDD_BITS=1`, which send every component to the fallback solvers: a
class with `dist` and `solve...before`, one with only a plain constraint and
one with a `foreach` dist over a dynamic array. Both weighted call sites print
one warning naming the call site and the budget however often they run, the
plain one prints none, and the constraints still hold (`bad=0`). With the
default budgets the fixture prints no warning. The register keeps S30-D9 as an
implementation limit without a portable case.

Run-time error fixtures (`rt_*`) exit with status 1:

| Fixture | Clause |
| --- | --- |
| `rt_negative_weight` | S30-D8: a negative dist weight is an error; the call returns 0 and the variable keeps its value |
| `rt_foreach_index` | S30-D11: an unguarded `A[k + 1]` selects outside the array for the last `k` (18.5.7.1 L29926-29927 "Invalid or out or bound array indices are not automatically eliminated; users must explicitly exclude these indices using predicates."); with the block disabled the call succeeds |
| `rt_circular_solve_before` | 18.5.9 L30077-30078 "There shall be no circular dependencies in the ordering"; the cycle is checked over the enabled blocks, so disabling one block makes the call succeed |
