# C11 model emission

The emitter consumes validated execution IR and renders standalone models with
ordered setup, evaluation and cleanup. It has no frontend dependency.

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

Packed arithmetic borrows signal/local operands when subsequent evaluation cannot
write or suspend, pools wide constants for each model lifetime, and skips
matching width/sign conversions. One-limb literals
use the existing initializer directly. General expression results and data retained
through selectors, calls or suspension stay owned. Arithmetic results reuse an
owned operand slot; add/subtract can reuse its same-width payload, while aliased
multiplication keeps an independent result. Borrowed inputs and retained results
follow the [runtime ownership contract](../rt/value/ownership.md).


## Contracts

| Topic | Document |
| --- | --- |
| Coroutine frames, scopes, ownership of values, constants, UDP/path/timing/SDF tables, muxes, dense cells | [evaluation.md](evaluation.md) |
| Storage, references, publication, native boundaries, containers, streams, fork/event captures | [storage.md](storage.md) |
| Model lifecycle, frame guards, net tables, C names and `model.symbols.tsv`, validation layers, instance body sharing, generation memory | [model/readme.md](model/readme.md) |
| Owned-emitter components | [owned/readme.md](owned/readme.md) |

See [runtime](../rt/readme.md),
[feature status](../../../docs/sim_features.md) and
[ownership validation](../../../tests/readme.md#dynamic-ownership-validation).
