# Agent guide: src/sim

The `llg` simulator: owned DB → semantic model → typed IR → `ExecutionModel` →
optimization → C11 → CMake-built model linked with the embedded runtime.
Component map and pipeline contracts: [readme](readme.md).

## Core abstractions

- **`SemanticModel`** (`semantic.rs`) — frontend-neutral model, origins,
  synthesis classification and executable-node coverage.
- **`Codegen`** (`codegen/`) — typed lowering of owned DB data into IR.
- **`IrModel`** (`ir.rs`, `ir/`) — typed storage/value/statement tables,
  validated by `IrModel::from_parts`.
- **`ExecutionModel`** (`execution/`) — executable blocks, effects, regions,
  suspend/resume plans; the only input to optimization and emission.
- **`OptConfig`** (`opt/`) — the four optimization passes.
- **`GeneratedModel`** — design name, `model.c`, symbol map and warnings.
- **`CmakeBuildOpts`** (`build.rs`) — model and runtime-archive builds.
- **`ValueConfig`** (`value_backend.rs`) — packed-value backend selection.

## Directory map

| Path | Guide or readme |
| --- | --- |
| `semantic/`, `semantic.rs` | [readme](semantic/readme.md) |
| `codegen/`, `codegen.rs` | [guide](codegen/AGENTS.md) |
| `ir/`, `ir.rs` | [readme](ir/readme.md) |
| `execution/`, `execution.rs` | [readme](execution/readme.md) |
| `opt/`, `opt.rs` | [readme](opt/readme.md) |
| `emit_c/`, `emit_c.rs` | [guide](emit_c/AGENTS.md) |
| `rt/` | [guide](rt/AGENTS.md) |
| `build/`, `build.rs` | [readme](build/readme.md) |
| `sdf/` | module docs in `sdf/mod.rs` |
| `legacy_unsupported.rs`, `value_backend.rs`, `mod.rs` | module docs |

## Conventions and pitfalls

- Simulator Rust has no `unsafe`, native or FFI calls; it reads only owned DB
  data.
- Unsupported executable forms fail with source-linked errors before C
  compilation; by-design rejections reuse `legacy_unsupported::diagnostic`.
- Treat `IrModelParts` as untrusted until validated; validate before any
  optimizer or emitter indexing.
- Generation stays linear in the instance count (gate-level netlists hold
  10^5+ cells).
- Packed-width limit: `emit_c.rs` width constants; value backend policy:
  `value_backend.rs` module docs (no fallback).
- The runtime ships only with generated models; never link it into Rust
  binaries.
- CMake is the only model builder; never bake build-machine paths into runtime
  defaults.

## Commands

```sh
cargo nextest run --lib sim::
cargo nextest run --test general emit_decoupling opt_storage_differential
cargo nextest run --test sim_cmake --test sim_scale --test sim_opt_differential
```

Full-suite and parallel-worktree runs: [tests](../../tests/readme.md).

## Pointers

- [Feature status](../../docs/sim_features.md) owns supported type/context
  inventories; [LRM decisions](../../docs/lrm_decisions.md) own decision IDs.
- [Data semantics](../../docs/sim_data_semantics.md),
  [cross-platform guide](../../docs/cross_platform.md),
  [model build](build/readme.md).
