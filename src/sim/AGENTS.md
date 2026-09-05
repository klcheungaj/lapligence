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
  through `f32` like the runtime; div/mod/pow only for known ≤64-bit
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
- `rt/` — embedded C runtime (`include_str!`): `sv4_t` 4-state value ops +
  event scheduler (`llg_rt.c`), plus the libaco sources (`aco.c`/`acosw.S`)
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

Driver: `src/bin/llg.rs` (compile → codegen(lowering → IR → opt → emit)
→ build → run).

## Requirements

- **No `unsafe`** (all UHDM access is through `core::db`, which is safe).
- **No direct VPI calls** outside the `generate` entry's db build.
- Multi-variant consumers should build `core::db::Db` once and call
  `generate_from_db_with_opts`; do not traverse the same live VPI design once
  per optimizer configuration.
- libaco is **not** a Rust dependency: it is compiled together with the
  generated C model at model-build time.
- Read [codegen/AGENTS.md](codegen/AGENTS.md) for initialization, sensitivity,
  ports/interfaces, inout nets, tasks/forks, force/release, real values,
  timescale, arrays, supported forms and explicit rejection boundaries.
- Read [rt/AGENTS.md](rt/AGENTS.md) for value/scheduler/waveform ownership.
- The runtime is pure C, emitted into `target/sim/<design>/` with the model.
  `core::compile`, `core::db`, and `core::elab` supply frontend data and values.
