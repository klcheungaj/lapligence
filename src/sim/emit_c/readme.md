# C emitter

## Structured ownership path

Whole-model rendering consumes a validated `ExecutionModel`; it does not access
frontend or FFI objects. `model.rs` emits static descriptors/prototypes and
coordinates `owned/`, which emits ordered setup, tracked packed results,
borrowing runtime calls, and explicit cleanup. Operands are evaluated once;
short-circuit and conditional branches retain their separate setup paths.
Temporary slots are reused, and lexical packed cells have distinct registered
scopes when a pending write might outlive the declaration.

The active numeric path covers ordinary packed/real model storage, fixed arrays,
numeric procedures, loops, basic waits, assignments, and typed numeric output.
Further source paths cover assertion/sampling callbacks, aliases, clocking and
qualified branching; their bounded contracts and unverified acceptance status are
recorded in `owned/readme.md`.
It is an incremental migration, not full HDL support. Unsupported feature
families return an error without legacy fallback. The legacy expression and
statement fragment APIs remain gated. Read the exact boundary in
[`owned/readme.md`](owned/readme.md).

## Model lifetime and ABI

Packed globals, static returns/locals, net drivers and array elements are empty
file-scope descriptors, constructed at model startup and destroyed at close.
`llg_model_start`, `llg_model_advance` and `llg_model_close` provide an embeddable
lifetime. A suspended advance returns 2 without destroying queues or owners;
close explicitly cancels a suspended run. Define `LLG_MODEL_NO_MAIN` when a host
provides `main`. One model instance may be live at a time; this is not a new
thread-safe or multi-instance runtime.

The generated `LLG_MODEL_VALUE_ABI` must equal `LLG_VALUE_ABI_VERSION` (3).
Model capacity is not an allocation size or a build/cache dimension. The width
scan enforces only the exclusive backend limit. `LLG_MODEL_STACK_VALUES` remains
a checked conservative descriptor/stack-headroom estimate. CMake requests C11
with extensions disabled and hashes the ownership ABI into runtime-cache keys.

## Source organization and verification

`model/` retains static metadata and prototype helpers; its old procedure and
initialization implementations are not a fallback for `owned/`. Existing
`expressions/` and `statements/` fragment renderers are migration references and
legacy test subjects. New ownership responsibilities are split into the modules
listed in `owned/readme.md`.

The Rust structural and numeric-model execution tests are checked in
but were not executable in the delivery environment (no Rust toolchain).
Standalone C ownership tests exercise hand-authored output patterns; they do
not establish that the Rust renderer builds or emits compiling models. Full
frontend-to-executable and native-platform verification remain acceptance gates.

See [`docs/sim_data_semantics.md`](../../../docs/sim_data_semantics.md) for width
and conversion semantics and [the source map](../../../docs/source_layout.md).
