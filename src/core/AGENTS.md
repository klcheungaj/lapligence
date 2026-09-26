# Shared semantic layer

`core` owns frontend-neutral data for simulation, lint and LSP. `compile` admits
in-memory buffers and returns owned diagnostics plus a `ffi::slang::Snapshot`.
`Db::from_slang` is the only semantic import; downstream consumers use the owned
arena or validated test builder, never native pointers or source rereads.

## Data contracts

- Node IDs are stable arena indices. Parent/structural-child links are acyclic;
  typed resolved references may cross hierarchies. Normalize `InstanceBody` so
  instances directly own declarations, processes, subroutines and children.
  Retain instance-array containers/entries and expose concrete entries in the
  enclosing instance's child list. Preserve explicit statement blocks.
- Key elaborated ranges by declaration `NodeId`, not display paths: unnamed
  sibling scopes may contain identical names with different ranges.
- Import types, constants, dimensions, aggregate members, array categories, port
  bindings, definition kinds, source ranges and time scales from typed ABI fields.
  Never infer categories from display text or raw Slang enum values. Preserve
  implicit conversions and source- versus context-determined widths.
- Preserve every ordered `DriverDelay` expression, including rise/fall/turn-off;
  validate all references. Never substitute the first expression for the list.
- `Db::source_text` owns admitted bounded buffers. Reject invalid ranges, IDs,
  windows, cycles and cross-table references during construction. Unsupported
  legal nodes stay typed `Unsupported`; executable uses must reject, not become
  empty statements or fabricated defaults.
- Four-state `core::value` words are little-significance-first, with
  `(unknown, value)` planes: X=(1,0), Z=(1,1). Preserve arbitrary string bytes.
  `elab` supplies pure operations; `model`, `tokens`, `macros` and `lint` project
  owned data. No core `unsafe` or LSP-only dependencies; extend capture/DB for
  shared facts and keep semantic/executable IR independently testable.
- Source-byte failures, including preflight and isolated parsing, use
  `StartupErrorKind::LimitExceeded` for caller resource guidance.

## Database projections

Keep native-kind metadata for unsupported-node reachability, program origin,
explicit storage lifetime/source qualification, enum declaration order and every
`foreach` iterator slot (including omitted and packed dimensions). Subroutine
bodies use explicit references. Fold index keys as expressions, not spelling.
Recursive instance-array expansion rejects cycles and repeated entries.
Event metadata retains `iff`, event-array identities, nonblocking trigger mode
and timing; implicit conversions and complete delays survive projection.

Conditional clauses have dense, unique ordered indices; each `ConditionPattern`
pairs with its condition. `if` true/false branches are explicit, not child-position
heuristics. One-clause conditional operations and multi-clause
`ExprKind::Conditional` remain distinct. Consumers traverse every potentially
reached clause; an earlier conditional write is not necessarily definite.
Validate tagged pattern member IDs and their relationships.

## Admission, libraries and editions

Library-map resolution ranks explicit filenames above wildcard filenames above
trailing-directory matches. Wildcards in parent directories do not demote the
filename rank. Resolve ties only at the final rank, deduplicate same-library
matches and preserve explicit library overrides. Disk and logical inputs share
one bounded resolver; capture metadata once, retaining original buffers/handles
as source truth, with no native I/O.

`library_configs.rs` projects literal configuration text without shifting byte
positions and restores original map text after capture for UTF-16 presentation.
Reject unquoted per-library `-incdir` clauses at admission until bounded include
search can preserve authorization and source ordering; do not discard their tokens.

Edition checks use classified tokens and owned semantic edges shared by execution
and navigation. Explicit `system_subroutines` prototypes distinguish extensions
from standards. Directive bodies are ignored until expanded; executable for-loop
header edges differ from navigation-only capture. Memory-storage exemptions are
argument-specific (`$fread` argument 0; `$readmem*` argument 1), never exemptions
for unrelated whole-array uses of the same declaration.

See [FFI](../ffi/AGENTS.md), [simulator](../sim/AGENTS.md),
[LSP](../bin/llg_ls/AGENTS.md) and [lint](lint/AGENTS.md) for their contracts.
