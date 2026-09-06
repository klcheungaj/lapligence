# C emitter

`emit_c.rs` is the public facade. The modules in this directory render
validated IR expressions, statements, functions, and whole models; `names.rs`
owns C identifier spelling and `error.rs` owns the typed public failure.

The emitter depends only on `sim::ir`. Public detached-node render entry points
validate their input against the supplied model before table indexing.

`model.rs` derives the packed capacity from the completed IR and emits
`LLG_MODEL_MAX_WIDTH` for every C translation unit. The capacity is strictly
below the backend's `1 << 20` exclusive limit; the IR itself has no fixed
1024-bit or 64-bit arithmetic ceiling. `stack.rs` emits a conservative per-
model stack estimate from the largest function frame and process frame:
`(max_function_frame * recursion_depth_256 + max_process_frame) * 8`, with
the historical minimum retained for small models. Each frame accumulates the
typed expression storage across sequential statements and lexical control-flow
arms because the C compiler, particularly with sanitizer instrumentation, may
reserve return-by-value temporaries for the lifetime of the generated function.
All size arithmetic is checked before emission. The model's CMake `sim`
target defines both `LLG_MODEL_MAX_WIDTH` and `LLG_MODEL_STACK_VALUES` for all
translation units, including the standalone value runtime. Runtime checks remain
defensive if a malformed value
crosses that ABI boundary. See
[`docs/sim_data_semantics.md`](../../../docs/sim_data_semantics.md) for the
language-level width and conversion rules.

Selected assignment targets retain typed index trees and their elaborated
indexed-part extent. Capacity discovery includes intermediate index values
even when they are wider than every stored signal. Indexed reads and writes
use that static extent during C emission; a width expression's integer storage
width is not the selected data width. Named packed-member writes preserve the
member's two-state conversion independently of the enclosing storage type.
