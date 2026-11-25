# FIN-002 conditional operator chains and deep nesting (step 4)

Chains of the conditional operator, 4 to 32 levels deep, run by
[`tests/sim_feature_completion/fin_002e.rs`](../../../../sim_feature_completion/fin_002e.rs).
A chain used to emit C that doubled with every level; the suite also bounds
the size of each fixture's `model.c`. Every fixture drives its predicates to
0, 1, X and Z at each position of each chain.

The expected values rest on IEEE 1800-2009 11.4.11
(`SystemVerilog-1800-2009.txt`):

> If cond_predicate is true, the operator returns the value of the first expression without evaluating the second
> expression; if false, it returns the value of the second expression without evaluating the first expression. If
> cond_predicate evaluates to an ambiguous value (x or z), then both the first expression and the second
> expression shall be evaluated, and their results shall be combined bit by bit using Table 11-20 to calculate
> the final result unless either the first or second expression is real, in which case the result shall be 0. The first
> and second expressions are extended to the same width, as described in 11.6.1 and 11.8.2.

(L15147-15152; Table 11-20, L15156-15166, keeps a bit that is 0 in both or
1 in both and gives X for every other pair), and for the last fixture

> For nonintegral and aggregate expressions, if cond_predicate evaluates to an ambiguous value, then:
> — If the first expression and the second expression are of a class data type and if the conditional opera-
> tion is legal, then the resulting type is determined as defined above and the result is null.
> — Otherwise, both the first expression and second expression shall be evaluated, and their results shall
> be combined element by element. If the elements match, the element is returned. If they do not
> match, then the default-uninitialized value for that element’s type shall be returned.

(L15211-15216).

| Fixture | Contents | Expected output |
| --- | --- | --- |
| `cond_chain_packed.sv` | 32-bit priority chains of 4, 8, 16 and 32 levels as a continuous assignment, in a procedure and in a function, and a two-state chain; one predicate true, X or Z at every position, X followed by a true or a Z predicate, none, all X, and all alternatives equal. | industry-standard behaviour |
| `cond_chain_effects.sv` | The same depths with predicates and alternatives that are function calls recording their order; short-circuit operators as predicates. | industry-standard behaviour |
| `cond_chain_types.sv` | Chains of 8 over 33-, 64-, 65-, 128- and 200-bit vectors, signed and mixed-sign operands of different widths, two-state and mixed-state operands, reals, a real among integers, strings, an enumeration and a packed structure; `'0`, `'1`, `'x` and `'z` alternatives at 65 and 128 bits. | industry-standard behaviour |
| `cond_chain_shapes.sv` | Conditionals nested in the first alternative (4 to 32 levels), in both alternatives (a 32-leaf tree), in the predicate, and alternatives of different widths and signs along one chain. | industry-standard behaviour |
| `cond_chain_clause.sv` | Chains of 8 over unpacked arrays, an unpacked structure, class handles and reals, and increments in alternatives. | the clause text, below |

### `cond_chain_clause.out`

Each line follows from the two quotations and from the decisions
[R05-D1 and S11-D7](../../../../../docs/lrm_decisions.md): for an ambiguous
predicate an array element or structure member that is known equal in both
alternatives keeps its value and any other takes its default (`xx` for
`logic [7:0]`, 0 for `int`); a class result is null (printed as -1); a real
result is 0, also when both alternatives are 2.5; and an increment in an
alternative runs only when that alternative is evaluated (`inc u=1` leaves
`j` and `k` unchanged). Some simulators differ on these rows: they combine
differing packed elements bit by bit, return one of the class operands, keep
equal reals, or run the increments of an alternative that is not selected.
The transcript was derived line by line from these rules with a separate
model of the chain, not from the simulator under test.

## Deep nesting

`deep_nesting.sv` nests every construct tens to hundreds of levels deep: above
the 60 to 70 levels that used to overflow the stack of an unoptimized `llg`,
and below what a C compiler accepts, because nested statements become nested
C blocks (MSVC stops at 128). Tests of the limit itself
(`llg::stack::MAX_NESTING_DEPTH`) generate their sources in
[`tests/deep_nesting.rs`](../../../../deep_nesting.rs) and build nothing.

Every line of `deep_nesting.out` is a count or a selection that does not
depend on the depth:

| Lines | Value |
| --- | --- |
| `blocks` | one `initial` in the innermost block of the `if`, `case` and loop nests, and the arm selected from the 150-arm `else` chain (149) |
| `instances` | `a` plus one for each of 100 instance levels |
| `binary`, `calls`, `let`, `macro`, `index`, `members`, `string` | the number of operands or levels, plus the innermost value |
| `parentheses`, `concatenations`, `streams`, `target`, `casts` | the innermost value: a concatenation of one operand is that operand, an even number of bit reversals restores it, and a cast to `byte` truncates 301 to 45 |
| `unary` | an odd number of complements of 1 |
| `conditional` | the first, the middle and, past the last condition, the default alternative |
| `inside` | an even number of memberships of 0 in the set of the previous result; of 1, always 1 |
| `begin` to `class`, `constraint` | the assignment in the innermost statement, one unit of time per nested `fork` or delay |
| `property` | an even number of negations of a condition that holds at the first clock edge and fails at the second |

## Recursive specializations

`recursive_specializations.sv`: classes that name themselves with other
parameter values until the values stop changing, two classes that name each
other, a class with two branches per level, eleven nested specializations of
one class, and a module that ends in a generate block. Endless ones are refused
([`tests/deep_nesting.rs`](../../../../deep_nesting.rs)).

- 8.24 L10756-10757: "The combination of a generic class and the actual
  parameter values is called a specialization. Each specialization of a class
  has a separate set of static member variables".

| Line | Value |
| --- | --- |
| `sum`, `chain` | 100 + 99 + ... + 0, over the specializations and over objects of them |
| `visit`, `again` | each specialization counts its own visits from its own `N`: 4 + 3 + 2 + 1, then 5 + 4 + 3 + 2 |
| `even`, `odd` | the number of levels above 0 |
| `leaves` | f(n) = f(n - 1) + f(n - 2) with f(0) = 1 and f(1) = 2 |
| `ladder` | 60 steps and the last one |
| `nested` | the fifth handle was never constructed |

## Call diamonds

`call_diamonds.sv`: thirty functions that each call the next one at two places,
so the first reaches the last along 2^30 call paths. Lowering visits a body once
for each way its `ref` and handle formals are bound, and an expression whose
calls would expand to more than 256 inlined bodies is evaluated by its process
instead of a callback
([`scaling_tests.rs`](../../../../../src/sim/codegen/lowering/scaling_tests.rs)
counts the lookups). Only the argument 200 takes the paths, and no process
passes it.

- 9.2.2.2.2 L11201-11202: "always_comb is sensitive to changes within the
  contents of a function, whereas always @* is only sensitive to changes to the
  arguments of a function".

| Line | Value |
| --- | --- |
| `n`, `y`, `event`, `monitor`, `force` | `d + e`, one time unit later for `n` |
| `edge` | `d + e + 1` when its least significant bit rises |
| `f` | `r` after it changes |
| `handle`, `wait` | the time at which the waited value appears |
