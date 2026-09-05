# Simulator module

This directory implements the simulator-specific half of LLG. `codegen.rs`,
`emit_c.rs`, and `opt.rs` are narrow public facades: their implementation
modules live in the matching directories and separate lowering, time-scale,
naming, expression, statement, model, and pass concerns. `codegen` lowers the
owned `core::db` design into typed IR, `opt` applies conservative IR passes,
`emit_c` renders the IR as a C11 model, and `build.rs` builds that model with
the embedded runtime and libaco sources from `rt/`. Public lowering and
rendering failures use `CodegenError` and `EmitError`, so callers do not need
to classify module failures by parsing strings. Model rebuilds
are deterministic: stale entries in the output directory are pruned and an
incompatible CMake build tree (missing cache or generator mismatch) is
discarded so cmake reconfigures cleanly instead of failing.

The module must remain free of `unsafe` and direct VPI access. Extend
`core::db` when lowering needs additional UHDM data; the C emitter consumes IR
only. CMake is the sole model builder, and runtime behavior must stay aligned
with `core::elab::Value` through the property vectors and C runtime self-test.
IR table and representation fields are private to the simulator implementation. Public
constructors validate local invariants, immutable accessors expose phase
outputs, and `IrModel::validate` plus its detached-node validation helpers
guard lowering, optimization, and rendering boundaries. External tools can
assemble a nonempty model in `IrModelParts` and pass it to
`IrModel::from_parts`; the parts are explicitly untrusted staging data and a
model is returned only after complete cross-table validation succeeds.

End-to-end behavior is covered by `tests/sim_*.rs`, scheduling by
`tests/region_conformance.rs`, optimizer equivalence by
`tests/sim_opt_differential.rs`, and the emitter boundary by
`tests/emit_decoupling.rs`. `docs/sim_features.md` is the detailed support
matrix; unsupported accepted syntax must fail clearly or be documented as an
explicit approximation.

Waveform controls lower into IR and enable an otherwise-absent generated-model
component. `$dumpfile` chooses VCD or FST by extension; `$dumpvars`, `$dumpon`,
`$dumpoff`, `$dumpall`, `$dumpflush`, and `$dumplimit` feed one bounded,
lossless SPSC ring from the single simulation OS thread to a dedicated writer
thread. The writer exclusively owns the file and GTKWave libfst context, so
simulation coroutines never race an encoder or perform file I/O. Normal models
without waveform tasks neither compile libfst nor create a writer thread.
Generated waveform builds require CMake Threads and zlib; bundled libfst,
FastLZ, and LZ4 sources are emitted locally without network downloads.

The thread abstraction has POSIX and Win32 implementations, but this does not
remove the simulator's existing native-Windows blockers in libaco/generated C;
Windows FST remains an experimental MinGW-oriented path. `$dumpvars` selection
arguments currently produce one warning and dump all registered user storage.
Unpacked arrays are exposed as flattened `[linear_index]` variables.
Hierarchy component boundaries use an internal non-source control delimiter,
so dots inside escaped identifiers never become false scopes. Punctuation in
serialized VCD/FST component names is encoded reversibly and without name
collisions. VCD and FST metadata use a fixed date field for reproducible
output.

True-net declaration assignments (`wire w = expr;`) are continuous drivers,
not startup initializers. They reuse the ordinary event-driven continuous-
assignment IR: constant RHS expressions run once and dynamic RHS expressions
wait on a precomputed, deduplicated sensitivity set. Variable declaration
initializers remain startup writes. Unknown executable UHDM statements must
reach codegen as `StmtKind::Unsupported` and fail with source context; only
`StmtKind::Empty` may lower to a no-op.

Use `generate_from_db_with_opts` when emitting more than one optimization
variant from the same elaborated design. It reuses the single owned `Db`,
avoiding repeated VPI walks and frontend iterator-lifetime assumptions.
