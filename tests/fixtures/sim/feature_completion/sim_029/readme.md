# SIM-029 fixtures: constrained randomization

Plain IEEE 1800-2009 SystemVerilog with top `tb`, run by
`tests/sim_feature_completion/sim_029.rs` in both optimizer modes and, for
the positive cases, on every value backend. Line numbers refer to
`SystemVerilog-1800-2009.txt` (`pdftotext -layout` extraction, see
`docs/lrm_decisions.md`).

Positive fixtures print relations that every legal solution satisfies
(`bad=0`, `holds=1`) or values the clause fixes (values left unchanged,
values forced by constraints, mode queries), never the drawn values
themselves, so the expected output is independent of the solver's choices.

| Fixture | Checks | Clause |
| --- | --- | --- |
| `class_constraints` | `z == x + y` in every solution; implication, `if`/`else`, `inside`, an external constraint body; a derived block replaces the base block of the same name and randomize() through a base handle uses the object's class; a static random variable is shared | 18.6.1 L30399-30400 "generates random values for all the active random variables in the object, subject to the active constraints"; 18.5.2 L29567-29568 "Any constraint in a derived class having the same name as a constraint in its superclass shall replace the inherited constraint"; 18.5.2 L29563-29564 "The randomize() method is virtual and therefore honors constraints of the object on which it was called"; 18.6.3 L30468 static variables "are shared by all instances" |
| `modes` | an inactive variable keeps its value and is state; disabled blocks are ignored; whole-object forms reach the dynamic class; static members share one mode; a mode set through a base handle addresses the block in effect | 18.8 L30648-30650 "Inactive variables are not randomized by the randomize() method, and their values are treated as state variables by the solver"; 18.9 L30749-30750 "When a constraint is inactive, it is not considered by the randomize() method"; 18.5.10 L30117-30118 "if a static constraint is set to OFF, it is off for all instances" |
| `unsat_and_hooks` | a failed call returns 0 and leaves every variable (instance, static, `std::randomize` argument) unchanged; `pre_randomize` runs before modes and state are read, also when the call fails; `post_randomize` runs only after a commit | 18.6.3 L30471-30473 "If randomize() fails, the constraints are infeasible, and the random variables retain their previous values. If randomize() fails, post_randomize() is not called."; 18.6.2 L30445-30447 |
| `inline_and_lists` | `with` constraints over the object and the caller's scope; argument lists designate the complete set of random variables; `randomize(null)` and `std::randomize()` without arguments only check | 18.7 L30493-30494 "These additional constraints are applied along with the object constraints"; 18.11 L30838-30840 "those arguments designate the complete set of random variables within that object; all other variables in the object are considered state variables"; 18.11.1 L30887-30889; 18.12 L30918 |
| `signed_ranges` | signed boundaries `-128`, `127` and negative ranges are reached exactly; a mixed-sign comparison is unsigned | 18.5.3 L29617; 11.8.1 (expression signedness) |
| `functions_and_guards` | a function argument is solved before the constraint that calls the function; a null handle guarded by `if`, `->` or `\|\|` does not fail the call | 18.5.11 L30175-30176 "Random variables used as function arguments shall establish an implicit variable ordering"; 18.5.12 L30237 "Both implication ( –>) and if…else can be used as guards", L30250-30263 |
| `search_fallback` | products whose BDD exceeds the node budget and whose solutions rejection cannot find in its budget: the CDCL fallback finds members of the solution set and proves a prime product infeasible, leaving the variables unchanged; its distribution is not uniform (S29-D10) | 18.6.1 L30399-30400; 18.6.3 L30471-30472 "If randomize() fails, the constraints are infeasible, and the random variables retain their previous values" |

Negative fixtures (`neg_*`) are located compile errors for constructs outside
SIM-029: structures whose members need a value domain, and a value-form
`randomize()` where it could run a different number of times than the source
evaluates it (a short-circuit operand, a loop condition). SIM-030 implements
`randc`, `dist`, `solve...before`, random divisors, random select indices
(superseding S29-D7), `**` on random variables, `foreach`, random arrays and
random object members; their cases are in `../sim_030/`.

Run-time error fixtures (`rt_*`) exit with status 1:

| Fixture | Clause |
| --- | --- |
| `rt_null_receiver` | S29-D9: `randomize()` through a null handle is an error at the call |
| `rt_unguarded_null` | 18.5.12 L30257 "If the guard evaluates to ERROR, then an error is generated and randomize fails" |
| `rt_circular_order` | 18.5.11 L30193 "Circular dependencies created by the implicit variable ordering shall result in an error" |
| `rt_resource` | A03: with `LLG_RAND_BDD_NODES=1 LLG_RAND_REJECT_TRIES=0 LLG_RAND_CONFLICTS=0` a satisfiable problem ends with a solver-budget error naming the budget, while an unsatisfiable one only warns (S29-D5) |

`enum_elements` draws random arrays of enum and packed-structure elements and
packed structures with enum members (18.4, 6.19): only named values appear, all
of them do, and an array under `rand_mode(0)` keeps an illegal value.
`tagged_member` holds a tagged union inside a `rand` packed structure (18.4
keeps the union itself out of `rand`): random bits keep a declared tag and a
legal member value.

`scope_fixed_arrays` runs `std::randomize` (18.12) over fixed arrays of a module
and of a subroutine, of one or several dimensions.

`randomize_inherited_list` names inherited properties in `randomize(list)`
(18.11), which needs `slang-randomize-inherited-property.patch`.

`randomize_value_forms` uses `randomize()` as a value where other operands decide
whether it runs, and in loop conditions. Its output is the reference run: the
counters of `pre_randomize` show which calls ran. 11.3.5 L14434-14435: "When short
circuiting occurs, any side effects or runtime errors that would have occurred
due to evaluation of the short circuited operand expression shall not occur."
11.4.7 L14898-14899: "For &&, if the first operand value is logically false then
the second operand shall not be evaluated." and "For ||, if the first operand
value is logically true then the second operand shall not be evaluated." 11.4.11:
"If cond_predicate evaluates to an ambiguous value (x or z), then both the first
expression and the second expression shall be evaluated". A null receiver in a
skipped operand is never read. `neg_randomize_in_wait` stays rejected: a wait would
run the call at every wake.
