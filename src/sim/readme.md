# Simulator module

This directory implements the simulator-specific half of LLG. `codegen.rs`
lowers the owned `core::db` design into typed IR, `opt.rs` applies conservative
IR passes, `emit_c.rs` renders the IR as a C11 model, and `build.rs` builds that
model with the embedded runtime and libaco sources from `rt/`.  Model rebuilds
are deterministic: stale entries in the output directory are pruned and an
incompatible CMake build tree (missing cache or generator mismatch) is
discarded so cmake reconfigures cleanly instead of failing.

The module must remain free of `unsafe` and direct VPI access. Extend
`core::db` when lowering needs additional UHDM data; the C emitter consumes IR
only. CMake is the sole model builder, and runtime behavior must stay aligned
with `core::elab::Value` through the property vectors and C runtime self-test.

End-to-end behavior is covered by `tests/sim_*.rs`, scheduling by
`tests/region_conformance.rs`, optimizer equivalence by
`tests/sim_opt_differential.rs`, and the emitter boundary by
`tests/emit_decoupling.rs`. `docs/sim_features.md` is the detailed support
matrix; unsupported accepted syntax must fail clearly or be documented as an
explicit approximation.
