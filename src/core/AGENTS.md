# Agent guide: src/core

Frontend-neutral data shared by the simulator, lint and LSP. `compile` admits
in-memory buffers and returns owned diagnostics plus a `ffi::slang::Snapshot`;
`Db::from_slang` is the only semantic import. Contracts: [readme](readme.md),
[db/readme](db/readme.md).

## Directory map

- `compile.rs`, `compile/`: admission, library mapping and configurations
  (`library_mapping.rs`, `library_configs.rs`), definition search
  (`definition_search.rs`), edition checks (`editions.rs`), `` `line`` positions.
  Each file's module docs state its rules.
- `db.rs`, `db/`: validated arena, import, test builder, `SourceMap`.
- `value.rs`, `elab.rs`: four-state values (`value.rs` docs give the planes) and
  pure value operations.
- `model.rs`, `tokens.rs`, `macros.rs`: editor/hierarchy projections, exact
  bindings, macros.
- `lint/`: shared rules ([guide](lint/AGENTS.md)).

## Rules

- Consumers use the owned arena or the validated test builder, never native
  pointers or source rereads. Extend capture/DB for shared facts; no new native
  traversals.
- No `unsafe` and no LSP-only dependencies in core.
- Import categories, widths, bindings, ranges and time scales from typed ABI
  fields; never infer them from display text or raw Slang enum values.
- Key elaborated ranges by declaration `NodeId`, never display paths: unnamed
  sibling scopes may hold identical names with different ranges.
- Preserve every ordered `DriverDelay` expression (rise/fall/turn-off); never
  substitute the first one for the list.
- Reject invalid ranges, IDs, windows, cycles and cross-table references during
  construction. Unsupported legal nodes stay typed `Unsupported`; executable uses
  reject, never become empty statements or fabricated defaults.
- Node positions are physical; consumers append the `SourceMap` logical position,
  never substitute it. Printers use `Diag::location()`.
- Source-byte failures, including preflight and isolated parsing, use
  `StartupErrorKind::LimitExceeded`.
- Keep semantic and executable IR independently testable.

## Pitfalls

- An earlier conditional write is not necessarily definite: traverse every
  potentially reached clause. One-clause conditional operations and
  multi-clause `ExprKind::Conditional` stay distinct.
- Memory-storage edition exemptions are argument-specific, never exemptions for
  unrelated whole-array uses of the declaration
  ([edition checks](readme.md#edition-checks)).
- In-memory compiles never run the definition search.

## Pointers

- Projection and import contracts: [db/readme](db/readme.md#import-and-projection-contracts).
- Library maps, `-incdir` and editions: [readme](readme.md).
- ABI and capture: [FFI](../ffi/AGENTS.md), [wrapper](../wrapper/AGENTS.md).
- Consumers: [simulator](../sim/AGENTS.md), [LSP](../bin/llg_ls/AGENTS.md).
