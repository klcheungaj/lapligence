# Constraint solver (`llg_constraint.c`)

The solver behind `randomize()` (IEEE 1800-2009 clause 18). The facade
`../llg_constraint.c` includes `common.c`, `eval.c`, `aig.c`, `bdd.c` and
`solve.c` in that order; `rt::constraint_sources` embeds the same order as one
translation unit of the `llg_runtime` archive. The interface is
`../llg_constraint.h`; the scheduler glue (`../scheduler/randomize.c`) converts
packed values, picks the random stream and reports outcomes.

The solver depends on no other runtime module. Values cross the interface as
exact 2-state little-endian words and draws come through a callback, so
`tests/runtime_constraints.rs` builds it standalone.

## Problems

Generated models emit one `llg_cr_problem_t` per randomize problem: expression
nodes in post-order (every operand index is smaller than its node), constant
words, variables (with an optional domain node, used for enum values),
constraint blocks (each a root node), state leaves and function-call sites.
Lowering sizes every node (IEEE 1800-2009 11.6/11.8) and inserts explicit
`RESIZE` nodes; the solver never re-derives expression types.

## Solving

1. **Classification** (`eval.c`, 18.5.12). Every node reachable from an
   enabled block or an active variable's domain becomes K (known before
   solving), R (depends on an unsolved random variable) or E (evaluation
   error: null handle, X/Z input (S29-D1), division by zero, select index out
   of range). `&&`, `||`, `!` and implication follow Figure 18-3; every other
   operator is E if any operand is E, else R if any operand is R. A root that
   is E fails with `LLG_CR_ERROR`; a known false root is `LLG_CR_UNSAT`.
2. **Function order** (18.5.11). Variables used as arguments of a function
   whose arguments are still random are solved first, in layers; their
   values then become state and the function is called through the generated
   thunk. A cycle is `LLG_CR_ERROR` (reason `ORDER`).
3. **Components.** Each layer splits into independent components (union-find
   over conjunct supports), solved in order of their smallest variable.
   Variables in no constraint are drawn directly.
4. **Portfolio per component.**
   - *Exact (BDD)*: bit-blast into a structurally hashed AIG (`aig.c`), build
     an ROBDD (bit positions most significant first, variables interleaved),
     count models exactly with multi-word integers and draw one model by
     unranking a uniform integer below the count (`bdd.c`). This is uniform
     over all legal combinations (18.5.9). An empty BDD proves UNSAT.
   - *Rejection*: uniform draws of every component variable, accepted only
     when every conjunct holds; also uniform over the solutions.
   - Otherwise `LLG_CR_RESOURCE` names the exhausted budget.
5. **Verification.** The complete assignment is re-classified with the
   word-level evaluator, which shares no code with the bit-blasted engines.
   A mismatch aborts as an internal error; it is never returned.

## Budgets and caching

Named tunables, read once from the environment: `LLG_RAND_BDD_NODES`
(default 2^20 nodes per component; also bounds the AIG at 8x and the apply
steps at 32x), `LLG_RAND_BDD_BITS` (512 variable bits per exact component,
which keeps the recursive BDD operations shallow on coroutine-hosted stacks),
`LLG_RAND_REJECT_TRIES` (4096), `LLG_RAND_CONFLICTS` (search fallback) and
`LLG_RAND_STATS=1` (one stderr line per solve: engines, components, BDD nodes,
rejection tries). Changing a budget may change the values drawn.

Each call site keeps one session for the run. A session caches the exact
model of its eight most recent components, keyed exactly by the conjuncts,
the variables and every known operand value, so a loop that randomizes with
unchanged state samples without rebuilding. Budget overflows are cached the
same way so a repeated overflow goes straight to the fallbacks.

## Determinism

No floating point, pointer ordering or host word size affects a decision.
The result is a pure function of the tables, the inputs, the budgets and the
random stream, so all platforms draw the same values from the same seed. A
failed solve still consumes the draws it made (S29-D6).
