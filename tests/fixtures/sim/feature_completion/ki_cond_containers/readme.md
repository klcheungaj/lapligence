# Conditionals over containers and native records

Conditional operators whose result is a queue, a dynamic or associative array
or a structure with a string, handle or container member, nested 1 to 32
levels deep, run by
[`tests/sim_feature_completion/ki_cond_containers.rs`](../../../../sim_feature_completion/ki_cond_containers.rs).
Nested conditionals of these types used to lower to code that doubled with
every level; the suite also bounds the size of each fixture's `model.c`.

The expected values rest on IEEE 1800-2009 11.4.11
(`SystemVerilog-1800-2009.txt`):

> If cond_predicate is true, the operator returns the value of the first expression without evaluating the second
> expression; if false, it returns the value of the second expression without evaluating the first expression. If

(L15147-15148), and

> For nonintegral and aggregate expressions, if cond_predicate evaluates to an ambiguous value, then:

> — Otherwise, both the first expression and second expression shall be evaluated, and their results shall
> be combined element by element. If the elements match, the element is returned. If they do not
> match, then the default-uninitialized value for that element’s type shall be returned.

(L15211, L15214-15216), with the decisions
[R05-D1 and R05-D2](../../../../../docs/lrm_decisions.md): members and
elements combine leaf by leaf; a container result has the larger size, and an
element or key only one operand has counts as its default in the other.

| Fixture | Contents | Expected output |
| --- | --- | --- |
| `chain_containers.sv` | Chains of 4, 8, 16 and 32 over queues, bounded queues, dynamic arrays, associative arrays with integral and string keys, queues of 200-bit and of two-state elements; one predicate true, X or Z at every position, X followed by a true or a Z predicate, none and all X; the target as one of its alternatives. | industry-standard behaviour; the lines `w` (200-bit elements) with an ambiguous predicate are the clause text |
| `chain_effects.sv` | Chains of 4 to 32 whose predicates and alternatives are calls that record their order. | industry-standard behaviour |
| `chain_shapes.sv` | Conditionals nested in the first alternative (4 to 32 levels), in both (a 32-leaf tree) and around concatenations. | industry-standard behaviour; the lines `conv` (concatenations) with an ambiguous predicate are the clause text |
| `chain_records_known.sv` | Chains of 4 over records with string, real, handle, container, nested and array members, with a queue element and a function result as targets, and of 8 over a small record, under known predicates. | industry-standard behaviour |
| `chain_records.sv` | Chains of 4 (the same records) and of 8 and 32 (small records) under every predicate, with a nonblocking and a continuous target, a tagged union and conditionals in call arguments. | the clause text, below |
| `chain_arguments.sv` | Conditionals over records nested 5 to 8 deep through the arguments of calls in their last, first, alternating or both alternatives, after a chain and beside another call; callees that write the variables and the target; a target that is one of its alternatives, a subroutine variable and a nonblocking one. | the clause text, below |
| `chain_late_reads.sv` | Later alternatives that write a variable an earlier one names. | industry-standard behaviour |
| `chain_predicates.sv` | Vector, signed, real, string, constant and fill predicates, over one level and five. | the clause text, below |
| `chain_patterns.sv` | `matches` predicates whose identifiers the alternatives use, over one level and five. | the clause text, below |

Some simulators differ on the clause-text rows: they combine differing
integral elements bit by bit, empty a string member although both operands
hold the same text, take one operand for a constant unknown predicate below
the first level, or reject tagged unions and real predicates over containers.
Those transcripts were derived from the rules above with separate models of
the conditionals, not from the simulator under test.

`chain_late_reads.sv` fixes one order the standard leaves open: a record
variable is read when the rest of its conditional has been evaluated, a
container when it is reached.
