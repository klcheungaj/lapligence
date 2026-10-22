# Constraint solver (`llg_constraint.c`)

The solver behind `randomize()` (IEEE 1800-2009 clause 18). The facade
`../llg_constraint.c` includes `common.c`, `eval.c`, `aig.c`, `bdd.c`,
`sample.c`, `cdcl.c`, `solve.c` and `expand.c` in that order;
`rt::constraint_sources` embeds the same order as one translation unit of the `llg_runtime` archive. The interface is
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

### Objects and arrays (`expand.c`)

A problem with units (`llg_cr_unit_t`) is a template. Each unit is one class
layout: it lists its variables, arrays (`llg_cr_array_t`: fixed, dynamic or
queue, packed elements) and handle properties (`llg_cr_slot_t`, scalar or
array), and owns a range of blocks and leaves. Variables, arrays and slots
are problem-wide (one per property), so an inherited property has one index
in every derived unit; variables listed by no unit are global (static
properties). Template nodes `REF`, `HVAR`, `AELEM`, `ASIZE`, `ITER`,
`FOREACH`, `REDUCE` and `ISNULL` name another object's variables, array
elements and sizes, loop indices and loops; `VAR` and `STATE` name the
expanding instance's own.

Generated code registers the objects of one call as instances: instance 0 is
the randomized object, and `llg_cr_link` registers each object a handle
names (deduplicated by handle, so an object reached twice, or the root
reached again, is one instance). `llg_cr_next_inst` walks the instances in
depth-first preorder over the links (S30-D7); `llg_cr_inst_pre` says when to
call an instance's `pre_randomize`, and `llg_cr_visit` lists the random
instances in that order for `post_randomize`. An object reached only through non-random links is state;
reached later through a random link it is returned again to be fed as
random.

Each solve expands the template over the instances into an ordinary
problem: loops become conjunctions or operator chains (an empty `sum`,
`product` or logical fold is the operation's identity; an empty `and`, `or`
or `xor` is an object-less leaf whose input is `LLG_CR_INPUT_EMPTY`, an
evaluation error a guard can eliminate, S30-D6), constant indices select an element
variable directly, and other indices become `ELEM` nodes over a table of
the array's elements, whose out-of-range or null selections are evaluation
errors the engines exclude (S30-D11). A value read through a null handle is
a null leaf, an error a guard can eliminate (18.5.12). Expansion
hash-conses nodes and folds constants, so `a[i+1]` with a known `i` is a
direct element. With random dynamic arrays or queues, a first expansion
solves the sizes before every other variable and drops the conjuncts that
need elements of arrays of unknown size; the second fixes those sizes
(S30-D5). A size no kept conjunct of an enabled block names keeps its value.
Expanded problems and their sessions are cached by structure (sixteen per
call site, least recently used replaced first), so repeated randomization of
the same graph reuses the exact-engine caches. Random sizes give one graph
per size combination, which is why this cache is larger than one.

## Solving

1. **Classification** (`eval.c`, 18.5.12). Every node reachable from an
   enabled block or an active variable's domain becomes K (known before
   solving), R (depends on an unsolved random variable) or E (evaluation
   error: null handle, X/Z input (S29-D1), division by zero, select index out
   of range, an empty `and`/`or`/`xor`
   reduction (S30-D6)). `&&`, `||`, `!` and
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
     `LLG_CR_RESOURCE` (S30-D9). A component with a `dist` or a user
     `solve...before` order that reaches them prints one stderr warning per
     call site (`cr_warn_fallback_measure`: the site's session owns the flag,
     an expanded problem's session shares its template's), naming the budget
     that sent it there as the variable to raise.
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
component search; 0 disables the search), `LLG_RAND_ARRAY_ELEMENTS` (2^20
random array elements per solve), `LLG_RAND_EXPAND_NODES` (2^22 nodes per
expansion), `LLG_RAND_OVERFLOW_MEMO` (1; 0 turns off the foreseen overflows
below), `LLG_RAND_SEARCH_CACHE` (1; 0 re-encodes every search; neither switch
changes a drawn value) and `LLG_RAND_STATS=1` (one stderr line per solve:
engines, components, BDD nodes, foreseen overflows, rejection tries,
conflicts, reused search encodings).
The AIG bound (8x `LLG_RAND_BDD_NODES`, at least 4096) also bounds the CDCL
encoding. Changing a budget may change the values drawn.

Each call site keeps one session for the run. A session caches the exact
model of its 32 most recently used components (an expanded problem has a
component per independent element or object, so the cache must hold all of
one call's components or each call would rebuild the evicted ones), keyed exactly by the conjuncts,
the variables and every known operand value, so a loop that randomizes with
unchanged state samples without rebuilding. Budget overflows are cached the
same way so a repeated overflow goes straight to the fallbacks.

State that changes on every call never repeats a key, so a component whose
exact model overflows would rebuild until the overflow each time (about
0.4 to 0.9 s for a 16-bit product at the default budget). A session remembers
the shapes (conjuncts and variables) whose build overflowed, eight per
session. The second build of such a shape first tries a proof
(`cr_overflow_foreseen`): it converts only the cones of the *state-free*
bits the root needs, the bits of nodes whose cone holds only constants,
operators and the component's random variables. The manager never frees
nodes and an apply step creates only nodes of its result, so a build ends
with exactly the union of the BDDs of the functions it computes and exceeds
the node bound exactly when that union does; a state-free cone computes the
same functions under every state. When the proof alone exceeds the node
bound, every later build of the shape whose root still needs those bits
would overflow too, and is skipped (it fails as the build would, with the
same draws). A state that drops those bits (a guard, a `?:` select, an
inactive variable) still gets the exact engine, so the exact engine runs
whenever it would have succeeded. An overflow that needs the state-dependent
part is still rebuilt per new state; its proof is tried once per shape. Layered
entries also key the variables' layers and keep the projections and weights.
Components without dist, randc or solve-before skip the layer plan, so
SIM-029 problems pay nothing for the layered engine.

The search keeps its encoding (the Tseitin clauses and watch lists of the
AIG cone, `cr_cnf_t`) for the four most recently used keys of the exact
cache, cached on a key's second search (the session remembers the hashes
of the last eight uncached keys, so state that never repeats pays no copy).
A run copies the cached encoding, so it searches exactly as a fresh one
would; learned clauses are not kept. A changed state value is a new key:
encoding the state as parameters fixed by unit clauses instead was measured
and rejected, because the unfolded instance made long searches slower than
the encoding it saved. Components with dist selectors are not cached.

A solve's working memory (reachability and cone marks, conjunct and support
lists, union-find and bucket arrays, the cache key, sampling temporaries)
lives in the session and is reused by every call, so a call that hits the
exact cache allocates nothing. Without calls to constraint functions the
18.5.11 pass is skipped and each conjunct's support is computed once. The
BDD manager's buffers (`bdd.c`) are pooled across builds, up to
`CR_BDD_POOL_KEEP_BYTES` (2 MiB); larger ones are freed. A rebuild on every
call (state that changes per call) would otherwise allocate and free about
200 KiB each time, which the C library may return to the system and fault in
again. None of this changes which values are drawn.

## Determinism

No floating point, pointer ordering or host word size affects a decision.
The result is a pure function of the tables, the inputs, the budgets and the
random stream, so all platforms draw the same values from the same seed. A
failed solve still consumes the draws it made (S29-D6).

## randc state

A `randc` variable's cycle state is one "used in this cycle" bit per value
plus a 64-bit signature of its component's conjuncts (operators, widths,
variable and call names, and the values of the known operands they read:
state variables, inline-constraint operands and constants; never the source
positions in state leaf names, so the same constraint at another call site
matches), `LLG_CR_CYCLIC_WORDS(width)` words, at most 16 bits wide. The caller passes
the object's state in before solving and stores `llg_cr_cyclic` back only
after `LLG_CR_OK`, so a failed randomize leaves it unchanged. A different
signature (constraint blocks enabled or disabled, other constraints, a state
value read by the component that changed since the last call) starts a new
cycle; so does a cycle with no unused feasible value left (S30-D3).
