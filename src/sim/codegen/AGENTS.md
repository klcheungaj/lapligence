# Agent guide: src/sim/codegen

Lowers a validated `SemanticModel` and the owned DB into typed IR and an
`ExecutionModel`, returning `GeneratedModel` or a source-located
`CodegenError`. No native calls, no `unsafe`, no C text. Applies to
`codegen.rs` and children. Module overview: [readme](readme.md).

## Core abstractions

- **`Codegen`** (`lowering.rs`) — shared lowering state; children are
  responsibility-named domains.
- **`lower_expr` / `lower_stmt` / `lower_lhs`** — produce typed IR only.
- **Collection** (`lowering/collection/`) — instances, storage, layouts,
  ports, canonical nets and the one-pass lookup indexes.
- **Typed selections** — `PackedChain` steps, `packed_geometry.rs` label-to-bit
  mapping, fixed projections.
- **Fixed values** — `IrFixedValue` descriptor operands for aggregates beyond
  packed capacity (frozen contract: [IR readme](../ir/readme.md)).
- **Initialization schedule** (`lowering/initialization/order.rs`).
- **`CodegenOptions`** — tunables shared with the driver (timing checks,
  pulses, SDF, value config).

## Directory map

- `lowering.rs`, `lowering/` — domains mapped in
  [lowering/readme.md](lowering/readme.md): `collection/`, `statements/`,
  `expressions/`, `containers/`, `objects/`, `randomization/`, `coverage/`,
  `assertions/`, `initialization/`, `net_delay_layout/`, `packed_geometry/`.
- `timescale.rs` — checked femtosecond ticks.
- `error.rs` — `CodegenError`.

## Conventions and pitfalls

- Keep shared state in `lowering.rs` and the smallest existing visibility;
  add code to the responsibility-named child that owns it.
- [Feature status](../../../docs/sim_features.md) owns supported type/context
  inventories; never broaden a guard because another context accepts the
  syntax.
- Take selectors, extents and delays from owned typed fields, never source
  names or frontend numeric codes; missing metadata rejects.
- Map packed labels to bits only through `packed_geometry.rs`; never flatten a
  `PackedChain` or sum offsets.
- Never expand descriptor-backed or oversized aggregates per cell or flatten
  them into one packed value.
- A read no wait can observe rejects with its location; a missed dependency is
  a hang.
- Exact-width values only; never truncate silently and never add detached
  C/`Verbatim` workarounds.
- Generation stays linear in instances: use the one-pass indexes, never
  design or signal scans.
- New topology, driver or writer forms must be added to the `net_cells.rs`
  inventory and register cell intervals for the single-writer rule.
- Release the DB after typed lowering and collection state before execution
  lowering on the consuming path.

## Commands

```sh
cargo nextest run --lib sim::codegen
cargo nextest run --test sim_scale --test sim_semantic --test sim_lrm_decisions
cargo nextest run --test sim_feature_completion --test sim_partial_features
```

## Contracts

- [Values and selections](values.md)
- [Processes, dependencies and initialization](processes.md)
- [Nets and procedural drivers](nets.md)
- [Calls, loops and callbacks](calls.md)
- [Events, predicates, aggregates and streams](events.md)
- [Randomization, coverage, time and native services](services.md)
- [Names, labels and scale](readme.md#names-labels-and-scale)
- Downstream: [emitter](../emit_c/AGENTS.md), [runtime](../rt/AGENTS.md).
