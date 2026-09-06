# sim — Verilog/SV → C11 simulator

## Purpose

Compiles elaborated designs into C11 models that run as standalone
executables:

- `codegen.rs` — db → IR lowering (pure lowering; no string emission left).
  `generate(design)` builds the owned db (`core::db`) and lowers it into an
  `IrModel` (`generate_with_opts(design, &OptConfig::default())` delegates to
  the optimizer + backend); the lower_expr/lower_stmt/lower_lhs families and
  the process/link/function/init-step builders live here.  `GeneratedModel`
  carries a `pub design_name: String` plus the emitted `model_c` and warnings.
  The behavioral contracts below (inout nets, arrays, timescale, force/
  release, interface bodies, …) are decided here, at lowering time.
- `ir.rs` — the typed IR: signals/arrays/net-groups/functions/processes
  (`IrShape` = `RunOnce | Loop | SensLoop`) + init steps/spawns and
  `IrExpr`/`IrStmt` trees.  Sensitivity and read sets are computed at
  lowering and carried in the IR; the optimizer never recomputes wake
  behavior.
- `opt.rs` — conservative optimization passes over the IR, driven by
  `OptConfig { fold_constants, identities, prune_branches, unused_storage }`
  with `default()`/`none()` and per-pass toggling for bisection.  Constant
  folding reuses `core::elab::Value` math (X/Z-correct; shortreal casts fold
  through `f32` like the runtime; div/mod/pow preserve model-sized known
  operands); identity simplifications are shape-guarded; branch/case pruning
  follows strict provability rules (never prunes past non-const items, and to
  default only when ALL items are proven unmatched); unused-storage
  elimination uses an omit flag (no index remapping) with a read-collector
  covering processes, funcs, init steps, spawns, monitor eval fns, links,
  force targets, display args, wait sensitivity lists, and task-call temps.
- `emit_c.rs` — the C11 backend, consuming ONLY IR types; decoupled from
  `core::db`/`ffi`/`vpi` (enforced by `tests/emit_decoupling.rs` greps, same
  spirit as the repo's `unsafe`-confinement rule).  Naming conventions
  (G_/p_/D_ prefixes) and the `model.c` first-line header are unchanged.
- `build.rs` — the CMake-only model builder (`build_model_cmake`), the only
  supported build path, invoked automatically right after C emission.  It
  writes sources via the shared helper, generates a `CMakeLists.txt`
  (C11, Release default, exe under `<build>/bin/`, links `m`), runs
  `<cmake> -S <out_dir> -B <out_dir>/build [-G <generator>]
  -DCMAKE_C_COMPILER=<LLG_CC|$CC|cc> -DCMAKE_C_FLAGS:STRING="-O2 -Wall
  -Wno-unused-function [$LLG_CFLAGS]"`, then `cmake --build --config Release`.
  Generator selection: `CmakeBuildOpts.generator` (driver
  `--generator <backend>` via `build_model_cmake_with_opts`) >
  `$CMAKE_GENERATOR` passthrough > cmake's host default.
  `generate_model_sources` writes sources + `CMakeLists.txt` only (driver
  `--gen-only`).  Env vars: `LLG_CMAKE` (cmake program), `LLG_CC`/`CC`
  compiler chain, `LLG_CFLAGS` appended.  Missing cmake → actionable error
  naming install; flags containing double quotes are rejected;
  `cmake_available()` probes for a usable cmake once per process.
- `rt/` — embedded C runtime (`include_str!`): standalone `sv4_t` value
  types/operations/conversions (`llg_value.h`/`llg_value.c`) and event
  scheduler (`llg_rt.h`/`llg_rt.c`), plus libaco (`aco.c`/`acosw.S`)
  and the C self-test.  The self-test carries a deterministic vector table
  (`VECTORS[]` in `llg_rt_selftest.c`) whose expected values are generated
  from `core::elab::Value` by `tests/property_elab.rs` (`gen_c_vectors`,
  ignored test; regenerate with `cargo test --test property_elab
  gen_c_vectors -- --ignored --nocapture > /tmp/vectors.inc`), so the C
  `sv4_*` ops are cross-checked against the Rust 4-state math on identical
  inputs.
- `mod.rs` — the shared source-write helper `write_sim_sources` (writes
  runtime + libaco + extra sources into a build dir; consumed by the
  `build` module).

The IR staging tables in `IrModelParts` are untrusted until
`IrModel::from_parts` validates all table references, storage shapes, process
registrations, and nested nodes. `IrModel::validate` and detached-node
validation protect later optimization and emission indexing. The C emitter
derives `LLG_MODEL_STACK_VALUES` from the largest validated function/process
frames using `(max_function_frame * recursion_depth_256 +
max_process_frame) * 8`, retaining the historical minimum for small models.
Frame accounting includes typed expression storage across sequential statements
and lexical control-flow arms because generated C compilers, especially with
sanitizer instrumentation, can retain return-by-value temporaries for the
whole function lifetime. Checked sizing errors stop emission. The generated
CMake `sim` target defines both `LLG_MODEL_MAX_WIDTH` and
`LLG_MODEL_STACK_VALUES` for every translation unit, including the standalone
value runtime; runtime checks remain defensive at that ABI boundary.

Selected assignment targets retain typed index trees and their elaborated
indexed-part extent. Capacity discovery includes intermediate index values even
when they are wider than every stored signal. Indexed reads and writes use that
static extent during C emission; a width expression's integer storage width is
not the selected data width. Named packed-member writes preserve the member's
two-state conversion independently of the enclosing storage type.

Driver: `src/bin/llg.rs` (compile → codegen(lowering → IR → opt → emit)
→ build → run).

Packed-width capacity is selected per generated model. The codegen/backend
emits `LLG_MODEL_MAX_WIDTH` from the completed design and rejects widths at the
exclusive `1 << 20` backend limit; the IR itself has no fixed 1024/64-bit
semantic cap. Runtime `sv4_t` widths are `uint32_t`, with defensive checks for
the model capacity. Division, modulo, and power therefore use the model's
wide limb capacity rather than a separate 64-bit operand limit. See
[docs/sim_data_semantics.md](../../docs/sim_data_semantics.md) for the
standard width/signedness and X/Z rules.

## Requirements

- **No `unsafe`** (all UHDM access is through `core::db`, which is safe).
- **No direct VPI calls** outside the `generate` entry's db build.
- Multi-variant consumers should build `core::db::Db` once and call
  `generate_from_db_with_opts`; do not traverse the same live VPI design once
  per optimizer configuration.
- libaco is **not** a Rust dependency: it is compiled together with the
  generated C model at model-build time.
- CMake is the only supported model-build path. Source output is pruned to the
  current model, incompatible or partial CMake build trees are discarded, and
  a failed configure receives one clean retry. Generator selection is explicit
  option, then `$CMAKE_GENERATOR`, then CMake's host default.
- Read [codegen/AGENTS.md](codegen/AGENTS.md) for initialization, sensitivity,
  ports/interfaces, inout nets, tasks/forks, force/release, real values,
  timescale, arrays, supported forms and explicit rejection boundaries.
- Read [rt/AGENTS.md](rt/AGENTS.md) for value/scheduler/waveform ownership.
- The runtime is pure C, emitted into `target/sim/<design>/` with the model.
  `core::compile`, `core::db`, and `core::elab` supply frontend data and values.
