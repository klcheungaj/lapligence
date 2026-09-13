# sim — Verilog/SV to C11 simulator

## Pipeline and ownership

The driver `src/bin/llg.rs` runs compile → lower → IR → optimize → emit →
CMake build → execute. Read [codegen/AGENTS.md](codegen/AGENTS.md) for
initialization, sensitivity, ports/interfaces, inout nets, tasks/forks,
force/release, reals, timescales, arrays, supported forms and rejection
boundaries; [rt/AGENTS.md](rt/AGENTS.md) owns value/scheduler/waveform contracts.
The [source map](../../docs/source_layout.md) locates responsibility modules.

- `codegen.rs` exposes `generate(&db)` and `generate_with_opts(&db, ...)` over
  an owned DB. `codegen/lowering/` decides behavior and builds expressions,
  statements/LHSs, processes, links, functions and initialization through
  `SemanticModel` and `ExecutionModel`, then delegates optimization/emission.
  `GeneratedModel` retains `pub design_name: String`, `model_c` and warnings.
- `semantic::SemanticModel` wraps the frontend-neutral DB and owns synthesis
  classification: return checked `SynthDesignView` or origin-linked reasons.
- `ir.rs` and `ir/` stage validated typed signals, arrays, nets, functions,
  processes, initialization and `IrExpr`/`IrStmt`. `execution::ExecutionModel`
  owns executable operations/blocks, effects, suspend/resume plans and regions.
  Optimization and whole-model emission consume only that model. Emit block
  terminators directly, including distinct resume blocks; body-controlled
  suspension must contain a validated waiting operation.
- `opt.rs` uses `OptConfig { fold_constants, identities, prune_branches,
  unused_storage }`, `default()`/`none()` and per-pass bisection. Constant
  folding reuses X/Z-correct `core::elab::Value`; shortreal rounds through `f32`,
  div/mod/pow retain model-sized known operands. Shape-guard identities; prune
  only proven branches/cases, never past nonconstant items or to default unless
  ALL items are proven unmatched. Omit unused storage without remapping indices.
  Read collection covers processes, functions, init, spawns, monitor evaluators,
  links, force targets, display, waits, trigger plans and task-call temporaries.
- `emit_c.rs`/`emit_c/` consumes ONLY IR: no `core::db`/`ffi`/`vpi` dependency.
  Preserve G_/p_/D_ naming and the `model.c` first-line header.
  `tests/emit_decoupling.rs` enforces this like the repository's
  `unsafe`-confinement rule.
- `mod.rs::write_sim_sources` writes runtime/libaco/extra sources for `build`.
  `rt/` embeds pure C with `include_str!` into `target/sim/<design>/`: independent
  `llg_value.h`/`llg_value.c`, `llg_random.h`/`llg_random.c`, scheduler
  `llg_rt.h`/`llg_rt.c`, libaco `aco.c`/`acosw.S`, and C self-tests. Scheduler
  and container domain fragments assemble into their existing flat sources.
  Normal builds compile the runtime/libaco sources into a compatible cached
  archive; source-only projects remain self-contained. They are never linked
  into Rust; `core::compile`, `core::db` and `core::value` supply frontend data/values.

## Validation and capacity

Treat `IrModelParts` as untrusted until `IrModel::from_parts` validates table
references, storage shapes, registrations and nested nodes. Keep
`IrModel::validate` and detached-node validation before optimizer/emitter
indexing. Semantic and executable representations remain separately owned.

Derive `LLG_MODEL_STACK_VALUES` from validated frames:
`(max_function_frame * recursion_depth_256 + max_process_frame) * 8`, retaining
the historical minimum. Account for typed expression storage across sequential
statements and lexical arms: C compilers, especially sanitizers, may retain
return-by-value temporaries for the whole function. Checked sizing failure
stops emission. Define `LLG_MODEL_MAX_WIDTH` on the cached runtime and generated
model. Pass `LLG_MODEL_STACK_VALUES` from generated `main` through
`llg_rt_init_with_args_precision_and_stack`, keeping stack headroom outside the
compiled runtime ABI and retaining defensive runtime checks.

Select packed capacity from the completed model; reject the exclusive `1 << 20`
backend limit, not a fixed 1024-/64-bit IR semantic cap. Runtime `sv4_t` widths are
`uint32_t`; div/mod/pow use model limbs. Preserve typed selected-index trees,
elaborated indexed-part extents and capacity for intermediate indices wider
than stored signals. Emit static extents, not a width expression's integer
storage width. Named packed-member writes preserve member-specific two-state
conversion. See [data semantics](../../docs/sim_data_semantics.md) for standard
width/sign/X/Z rules.

`llg_rt_selftest.c::VECTORS[]` cross-checks C `sv4_*` against identical
`core::elab::Value` inputs in `tests/property_elab.rs`. Regenerate with the
ignored Rust generator:

```sh
cargo test --test property_elab gen_c_vectors -- --ignored --nocapture > /tmp/vectors.inc
```

## Build contract

CMake is the only model builder, automatically invoked after emission through
`build_model_cmake`. It writes C11 sources and `CMakeLists.txt`, defaults to
Release, emits the executable under `<build>/bin/`, and links `m`:

```sh
<cmake> -S <out_dir> -B <out_dir>/build [-G <generator>] [-DCMAKE_C_COMPILER_LAUNCHER=<launcher>] -DCMAKE_C_COMPILER=<LLG_CC|$CC|cc> -DCMAKE_C_FLAGS:STRING="-O2 -Wall -Wno-unused-function [$LLG_CFLAGS]" -DLLG_RUNTIME_LIBRARY=<cache>
cmake --build --config Release
```

Generator precedence: `CmakeBuildOpts.generator` (`--generator <backend>` via
`build_model_cmake_with_opts`) > `$CMAKE_GENERATOR` > host default.
`CmakeBuildOpts.launcher` (`--launcher <program>`) forwards
`CMAKE_C_COMPILER_LAUNCHER` without choosing a default.
`CmakeBuildOpts.dpi_libraries` (`--dpi-lib <path>`, repeatable) accepts validated
explicit link files; copy `svdpi.h` into generated trees.
`generate_model_sources` (`--gen-only`) writes sources and CMake without building.
`LLG_CMAKE` selects CMake; `LLG_CC`/`CC` selects the compiler; append `LLG_CFLAGS`.
Reject double quotes in flags; missing CMake must name installation guidance.
`cmake_available()` probes once per process. Cache runtime archives by packed
width, source content, toolchain, flags, generator, launcher, platform and
waveform support; `LLG_RUNTIME_CACHE_DIR` overrides the platform cache root.
Prune stale model sources, discard incompatible/partial build trees, and retry
a failed configure once cleanly.

Root `build.rs` applies repository Slang/libaco `patches/` before native source
consumption. Keep upstream-base gitlinks; the portable preparer accepts clean
or fully-applied states and rejects partial/mismatched edits.

## Required boundaries

No `unsafe`, direct frontend or FFI calls in simulator Rust: use owned safe
`core::db`. Library optimizer comparisons build one `core::db::Db` and call
`generate_from_db_with_opts`; end-to-end comparisons use checked-in HDL through
both `llg` CLI modes, with independent frontend-to-executable runs. libaco is
not a Rust dependency and compiles only with generated C models.
