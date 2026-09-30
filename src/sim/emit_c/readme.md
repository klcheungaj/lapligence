# C11 model emission

The emitter consumes validated execution IR and renders standalone models with
ordered setup, evaluation and cleanup. It has no frontend dependency.

`owned/` handles expressions, typed lvalues, calls, captures, native services and
registered temporary/local scopes. Its model layer emits persistent storage,
initialization, procedures and the start/advance/close embedding API. A suspended
model remains live until resumed or closed; the process-global runtime supports
one model at a time. Legacy string-only fragment APIs remain fail-closed when
ownership cannot be represented.

Fork activation frames store input event captures in opaque slots. Joined
branches borrow the live parent's handle, preserving sibling rebinding; detached
branches snapshot the object identity and bind a private handle that survives the
enclosing timing task's return.

Suspendable functions, processes and fork branches use explicit
`llg_co_frame_t`-headed POD frames and `llg_co_fn` entry points. The
layout builder owns typed fields, per-block call-site unions, descriptors and
LP64 frame upper bounds. It records the generated C block tree exactly, merges
single storage-bearing child chains into their parent struct, drops empty blocks,
and emits `union { struct { ... } bK; ... } uN` only for storage-bearing siblings.
Declarations in a block with no resume point in that block or any descendant stay
as ordinary C locals; an unshared process with no resume points consequently has a
header-only frame. Resume-bearing scopes alone participate in overlays, upper
bounds and descriptor offsets. Dispatch labels target only those scopes, while
cancellation and structured-control gotos leave scopes, so no dispatch can skip a
narrowed local's initialization; strict GCC jump-initialization checks enforce the
same constraint on generated C.
Sibling blocks therefore share storage without making ordinary nested scopes add
declarator depth. Finalized member paths also drive descriptor offsets and the
emitted-C scope lint. Fields read at the first post-suspension
statement boundary are packed first at each level with stable declaration-order
ties. Polled/anchored callees are caller-owned; recursion and callees over the
configurable embed limit use the chain arena. Plain functions retain their typed
C ABI and finals remain plain `void` calls. Every suspension is an exact numbered
`LLG_CO_AWAIT` or `LLG_CO_CALL*` site, and descriptors carry the real entry.
Identical frame layouts (every instance of one process or task) share a single
`llg_shared_frame_<k>_t` typedef instead of one struct per coroutine.

Large designs stay compact at model level too: plain static storage is
initialized and destroyed through per-representation pointer tables and loops.
Net contributions use one cell array per electrical group, with descriptor loops
for defaults, index resets, alias binding and teardown. Net resolution and per-bit
grouping are unchanged. Consecutive spawns, finals and waveform registrations run from constant
argument tables in their original order.
Consecutive compatible procedural `assign` statements also use immutable row
tables and one model-local typed helper per shape. Each row reads its source at
issue time and completes publication/cleanup before the next row. Shared bodies
load their own table from their instance record; the minimum run length and
eligibility rules are in the [emitter guide](AGENTS.md#procedural-continuous-assignment-batches).

See [owned-emitter components](owned/readme.md), [runtime](../rt/readme.md),
[feature status](../../../docs/sim_features.md) and
[ownership validation](../../../tests/readme.md#dynamic-ownership-validation).

Repeated compatible source processes and subroutines also share their bodies.
Per-instance addresses, paths and runtime identities live in immutable records;
shared roots receive the record through startup tables, while subroutine and fork
entries forward it through a hidden argument. Descriptors and visible storage
remain per instance. `LLG_SHARE_MIN_INSTANCES` selects the minimum class size
(default 4); set it to `unlimited` to debug with independent bodies. Bodies with
unrepresented differences or local static driver storage remain separate.
The default leaves pairs and triples independent, reserving record and hidden-call
overhead for classes with a larger reduction in repeated code.
