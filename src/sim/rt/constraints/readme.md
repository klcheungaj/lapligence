# Constraint solver (`llg_constraint.c`)

The solver behind `randomize()` (IEEE 1800-2009 clause 18). The facade
`../llg_constraint.c` includes `common.c`, `eval.c`, `aig.c`, `bdd.c`,
`sample.c`, `cdcl.c` and `solve.c` in that order; `rt::constraint_sources` embeds the same order as one
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

SIM-030 adds `dist` constraints (`llg_cr_dist_t`: the distributed
expression, the guard of enclosing implications and `if`s, and per item a
1-bit match node, the bounds and the weight node), `solve...before` orders
(`llg_cr_order_t`, per constraint block), `randc` variables (`cyclic`) with
their cycle state passed in and out through `llg_cr_set_cyclic` /
`llg_cr_cyclic`, the `**` operator and the `DIST` node.

## Solving

1. **Classification** (`eval.c`, 18.5.12). Every node reachable from an
   enabled block or an active variable's domain becomes K (known before
   solving), R (depends on an unsolved random variable) or E (evaluation
   error: null handle, X/Z input (S29-D1), division by zero, select index out
   of range, a negative `dist` weight (S30-D8)). `&&`, `||`, `!` and
   implication follow Figure 18-3; every other operator is E if any operand
   is E, else R if any operand is R. A root that is E fails with
   `LLG_CR_ERROR`; a known false root is `LLG_CR_UNSAT`. A random divisor,
   random select index or random `**` operand is R: the bit-blaster gives
   each node an error literal next to its value (division by zero, index out
   of range, `0 ** negative`), combined with the Figure 18-3 absorption, and
   a conjunct holds only where it is true without error (S30-D4).
2. **Function order** (18.5.11). Variables used as arguments of a function
   whose arguments are still random are solved first, in layers; their
   values then become state and the function is called through the generated
   thunk. A cycle is `LLG_CR_ERROR` (reason `ORDER`); a circular
   `solve...before` among enabled blocks is reason `SOLVE_BEFORE`.
3. **Components.** Each layer splits into independent components (union-find
   over conjunct supports), solved in order of their smallest variable.
   Variables in no constraint are drawn directly.
4. **Portfolio per component.**
   - *Exact (BDD)*: bit-blast into a structurally hashed AIG (`aig.c`), build
     an ROBDD (bit positions most significant first, variables interleaved),
     count models exactly with multi-word integers and draw one model by
     unranking a uniform integer below the count (`bdd.c`). This is uniform
     over all legal combinations (18.5.9). An empty BDD proves UNSAT.
   - *Layered weighted exact sampling* (`sample.c`) replaces the uniform
     draw when a component has `randc` variables, live `solve...before`
     orders or `dist` constraints. Layers: each `randc` variable first (in
     index order), then the solve-before heights (longest order path to a
     sink over the whole problem), largest first; unordered variables join
     the last set (S30-D2). Layer i is drawn from Q_i = exists(later
     layers).F with the earlier layers fixed, so its values are weighted by
     their own layer only. Each `dist` adds one-hot selector levels below the
     variable bits: one for "the guard fails" and one per nonzero-weight
     item. Their 1-literals carry integer weights proportional to 1 and to
     |S| * w(v) / W respectively (the density of the weights relative to
     uniform over the support S of the items; w is the item's per-value
     weight, W the total), so a combination where the guard holds weighs its
     density and scaling all weights changes nothing; a draw counts weighted
     models per node and walks down with fresh uniform integers, so it is
     exact (S30-D1). A `randc` layer draws until
     it hits an unused value (64 tries), then enumerates the unused feasible
     values; with none left a new cycle starts (S30-D3).
   - *Rejection*: uniform draws of every component variable, accepted only
     when every conjunct holds; also uniform over the solutions.
   - *Search (CDCL, `cdcl.c`)*: only when both engines above give up. The
     same AIG cone is Tseitin-encoded and searched with two watched
     literals, first-UIP learning, integer VSIDS, phase saving and Luby
     restarts; initial phases, input priorities and bits outside the cone
     come from the random stream. It finds *a* solution, not a uniformly
     distributed one (S29-D10); exhausting the search proves UNSAT.
   - The fallbacks ignore `dist` weights and orders; a component with a
     `randc` variable that does not fit the exact engine is
     `LLG_CR_RESOURCE` (S30-D9).
   - Otherwise `LLG_CR_RESOURCE` names the budget to raise: the conflict
     budget when the search ran out (or is 0), the node budget when the AIG
     did not fit.
5. **Verification.** The complete assignment is re-classified with the
   word-level evaluator, which shares no code with the bit-blasted engines.
   A mismatch aborts as an internal error; it is never returned.

## Budgets and caching

Named tunables, read once from the environment: `LLG_RAND_BDD_NODES`
(default 2^20 nodes per component; also bounds the AIG at 8x and the apply
steps at 32x), `LLG_RAND_BDD_BITS` (512 variable bits per exact component,
which keeps the recursive BDD operations shallow on coroutine-hosted stacks),
`LLG_RAND_REJECT_TRIES` (4096), `LLG_RAND_CONFLICTS` (100000 conflicts per
component search; 0 disables the search) and `LLG_RAND_STATS=1` (one stderr
line per solve: engines, components, BDD nodes, rejection tries, conflicts).
The AIG bound (8x `LLG_RAND_BDD_NODES`, at least 4096) also bounds the CDCL
encoding. Changing a budget may change the values drawn.

Each call site keeps one session for the run. A session caches the exact
model of its eight most recent components, keyed exactly by the conjuncts,
the variables and every known operand value, so a loop that randomizes with
unchanged state samples without rebuilding. Budget overflows are cached the
same way so a repeated overflow goes straight to the fallbacks. Layered
entries also key the variables' layers and keep the projections and weights. The CDCL
search keeps nothing between calls: it re-encodes the component each time.

## Determinism

No floating point, pointer ordering or host word size affects a decision.
The result is a pure function of the tables, the inputs, the budgets and the
random stream, so all platforms draw the same values from the same seed. A
failed solve still consumes the draws it made (S29-D6).

## randc state

A `randc` variable's cycle state is one "used in this cycle" bit per value
plus a 64-bit structural signature of its component's conjuncts (operators,
widths, constants, variable, state and call names; never state values),
`LLG_CR_CYCLIC_WORDS(width)` words, at most 16 bits wide. The caller passes
the object's state in before solving and stores `llg_cr_cyclic` back only
after `LLG_CR_OK`, so a failed randomize leaves it unchanged. A different
signature (constraint blocks enabled or disabled, other constraints) starts a
new cycle; so does a cycle with no unused feasible value left (S30-D3).
