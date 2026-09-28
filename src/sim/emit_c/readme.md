# C11 model emission

The emitter consumes validated execution IR and renders standalone models with
ordered setup, evaluation and cleanup. It has no frontend dependency.

`owned/` handles expressions, typed lvalues, calls, captures, native services and
registered temporary/local scopes. Its model layer emits persistent storage,
initialization, procedures and the start/advance/close embedding API. A suspended
model remains live until resumed or closed; the process-global runtime supports
one model at a time. Legacy string-only fragment APIs remain fail-closed when
ownership cannot be represented.

Suspendable functions, processes and fork branches already use explicit
`llg_co_frame_t`-headed POD frames while libaco still provides execution. The
layout builder owns typed fields, per-block call-site unions, descriptors and
LP64 frame upper bounds. It records the generated C block tree exactly, merges
single storage-bearing child chains into their parent struct, drops empty blocks,
and emits `union { struct { ... } bK; ... } uN` only for storage-bearing siblings.
Sibling blocks therefore share storage without making ordinary nested scopes add
declarator depth. Finalized member paths also drive descriptor offsets and the
emitted-C scope lint. Fields read at the first post-suspension
statement boundary are packed first at each level with stable declaration-order
ties. Polled/anchored callees are caller-owned; recursion and callees over the
configurable embed limit use the process arena. Plain functions and finals retain
their C-stack ABI.

See [owned-emitter components](owned/readme.md), [runtime](../rt/readme.md),
[feature status](../../../docs/sim_features.md) and
[ownership validation](../../../tests/readme.md#dynamic-ownership-validation).
