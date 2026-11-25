# FIN-002 conditional operator chains (step 4)

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
