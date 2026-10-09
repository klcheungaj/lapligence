# C11 model emission

The emitter consumes validated execution IR and renders standalone models with
ordered setup, evaluation and cleanup. It has no frontend dependency.

Model generation passes the execution model by value: each process's operations
are released once its body is rendered, and a changed arena-callee set is
reanalyzed in place. The borrowed entry points used by tests copy the model only
when reanalysis changes that set. Exact body sharing groups candidates as they
arrive, retaining one normalized body and one comparison key per group while
preserving member and group order; operand text is interned, not copied per
member. Assembly moves rendered bodies into the model text and shortens
identifiers in that buffer instead of building a second copy.

`owned/` handles expressions, typed lvalues, calls, captures, native services and
registered temporary/local scopes. Its model layer emits persistent storage,
initialization, procedures and the start/advance/close embedding API. A suspended
model remains live until resumed or closed; the process-global runtime supports
one model at a time. Legacy string-only fragment APIs remain fail-closed when
ownership cannot be represented.

Large integral variable arrays emit one lazy storage descriptor. Selected reads
borrow an existing cell or its default; writes and selected registrations obtain
stable descriptors. Whole copies, equality, fills and admitted streams call typed
runtime operations. Lexical array activations use registered object scopes; waveform
registration uses loops instead of per-element generated tables.

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
header-only frame. Debug builds poison dead sibling overlays after owner cleanup
and fresh callee payloads before argument transfer; release emission stays
unchanged. Resume-bearing scopes alone participate in overlays, upper bounds
and descriptor offsets. Dispatch labels target only those scopes, while
cancellation and structured-control gotos leave scopes, so no dispatch can skip a
narrowed local's initialization; strict GCC jump-initialization checks enforce the
same constraint on generated C.
Sibling blocks therefore share storage without making ordinary nested scopes add
declarator depth. Finalized member paths also drive descriptor offsets and the
emitted-C scope lint. Fields read at the first post-suspension
statement boundary are packed first at each level with stable declaration-order
ties. Polled/anchored callees are caller-owned; recursion and callees over the
configurable embed limit use the chain arena. Plain functions retain their typed
C ABI and finals remain plain `void` calls. A recursive synchronous subprogram
`<fn>` is emitted as coroutine `<fn>_co` plus a plain entry `<fn>` with the
unchanged ABI that runs it on an `llg_co_sync` driver; calls into its own
component are arena calls with a `_llg_result` destination, and virtual or
virtual-interface calls enter through `llg_class_co_enter_<slot>` /
`llg_vif_co_enter_<i>_<m>` helpers. Class virtual and virtual-interface calls
whose selected implementation may suspend (timed tasks) use the same helpers
from any coroutine; the plain dispatchers fail for such implementations, and a
virtual-interface call checks its receiver at the call site
(`llg_vif_require`). Every suspension is an exact numbered
`LLG_CO_AWAIT` or `LLG_CO_CALL*` site, and descriptors carry the real entry.
Identical frame layouts (every instance of one process or task) share a single
`llg_shared_frame_<k>_t` typedef instead of one struct per coroutine.

Large designs stay compact at model level too: plain static storage is
initialized and destroyed through per-representation pointer tables and loops.
Electrical groups can span maximal identically connected bit ranges while
waveform/VPI views retain declared shapes. Remaining structural contributions use
ordered descriptor loops for captured RHS part selects, including distinct
inertial handles for delayed rows. Every row retains its width/sign cast;
ordinary expression elision does not assume that all rows share a shape.
Short or disconnected gathers retain ordinary emission. Net contributions use one cell array per electrical group, with descriptor loops
for defaults, index resets, alias binding and teardown. Net resolution preserves
electrical bit correspondence and driver semantics. Consecutive spawns, finals
and waveform registrations run from constant
argument tables in their original order.

See [owned-emitter components](owned/readme.md), [runtime](../rt/readme.md),
[feature status](../../../docs/sim_features.md) and
[ownership validation](../../../tests/readme.md#dynamic-ownership-validation).

Complete internal C identifiers are capped at 32 by `names::MAX_C_IDENTIFIER_LEN`
after all derived names exist. Shortened symbols keep their namespace, a leading
readable stem, a base-36 registry index and frame/descriptor suffixes. A sorted
per-model registry reserves occupied and external names; runtime API names and
foreign DPI symbols retain their exact spelling. User-visible hierarchy names,
waveforms and diagnostics remain source-derived.

For debugging, `GeneratedModel::sources()` carries `model.symbols.tsv` beside
`model.c`, including in `llg --gen-only` exports. Each row is a shortened name,
a tab and its full original C identifier, sorted by shortened name. The file is
empty without renames, is replaced on regeneration and is never compiled. See
the [emitter guide](AGENTS.md#model-lifecycle-and-checks) for the full naming and
generated-file contract.

Repeated compatible source processes and subroutines also share their bodies.
Per-instance addresses, paths and runtime identities live in immutable records;
shared roots receive the record through startup tables, while subroutine and fork
entries forward it through a hidden argument. Descriptors and visible storage
remain per instance. `LLG_SHARE_MIN_INSTANCES` selects the minimum class size
(default 4); set it to `unlimited` to debug with independent bodies. Bodies with
unrepresented differences or local static driver storage remain separate.
The default leaves pairs and triples independent, reserving record and hidden-call
overhead for classes with a larger reduction in repeated code.

Packed arithmetic borrows signal/local operands when subsequent evaluation cannot
write or suspend, pools wide constants for each model lifetime, and skips
matching width/sign conversions. One-limb literals
use the existing initializer directly. General expression results and data retained
through selectors, calls or suspension stay owned. Arithmetic results reuse an
owned operand slot; add/subtract can reuse its same-width payload, while aliased
multiplication keeps an independent result. Borrowed inputs and retained results
follow the [runtime ownership contract](../rt/value/ownership.md).
