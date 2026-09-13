# Semantic simulation model

`SemanticModel` is a validated view over the frontend-neutral owned database.
It is the only input to executable lowering and contains no scheduler regions,
coroutine layout, runtime calls, or backend naming policy.

Every captured node has a revision-local `OriginId`. Origins preserve physical
file coordinates when the frontend supplied them and explicitly identify
synthetic elaborated objects otherwise. `ExtensionRef` is an opaque namespace
and key reserved for future formal, tracing, and coverage consumers; it has no
runtime behavior today.

`validate_synthesizable(PortableRtl)` returns `SynthDesignView` only when all
reachable nodes and their semantic target dependencies fit the initial
conservative profile. Timing controls, named
events, dynamic processes and containers, procedural drivers, runtime object
types and services, initial/final processes, and unknown constructs return
structured `SynthesisIssue` values with origins. This classification is
separate from simulation support and does not discard testbench behavior.

The initial profile accepts resolved, fixed-width packed declarations,
structural combinational gates and ordinary arithmetic/control operations. A
single scalar edge or a list of plain signal-change controls can describe a
clocked or combinational process; nested waits remain simulation timing.
Multiple edge controls, including asynchronous-reset patterns, require a future
clock/reset proof and are rejected by this initial profile. Procedural loops
and subprogram calls require termination and effect proofs; they are rejected
until those proofs exist. Elaborated generate scopes are structural objects
and are unaffected. Source genvars remain elaboration-only declarations,
distinct from runtime variables and concrete iteration parameters. Port types
are checked even when there is no connected internal storage. Variable and
array declaration initialization requires a
target-specific profile and is excluded from portable RTL. Missing widths,
unresolved references, unknown expressions, non-static arrays, resolved-net
classes outside ordinary wire/logic/reg/uwire, delayed primitives, and
unsupported operations fail closed. Reachability follows every embedded
expression, lvalue, timing, call, port, and initializer reference, including
references that are absent from structural child lists. Later target profiles can refine memory,
initialization, X/Z, clock and reset policy without changing semantic capture.

Operation operand arity is validated at this boundary and reused by executable
lowering. A malformed frontend projection therefore returns a source-linked
error before constant evaluation or runtime lowering can index a missing
operand.

## Simulation coverage ledger

`simulation_coverage()` walks only the owned graph reachable from elaborated
top instances, including embedded references and declaration initializers. Each
node is classified as executable, declaration-only, elaboration-consumed,
intentionally unreachable, or unsupported. Native semantic kind and detail are
retained by `core::db` when a frontend-neutral node has no direct variant, so a
reachable unknown statement or expression produces a source-located error
instead of becoming `NodeKind::Other` and disappearing. Inactive generate
branches and declaration-only frontend records remain in the ledger without
being treated as executable obligations.
