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
- Node positions are physical. `` `line`` mappings stay in the separate
  `SourceMap`; consumers append, never substitute, the logical position.
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
A member access whose base is not a declaration path but a function call
(possibly through further member accesses) is `ExprKind::MemberSelect` with
the computed base, and an element select of it keeps that node as its base;
other non-path member accesses stay `Other`.

## Admission, libraries and editions

Library-map resolution ranks explicit filenames above wildcard filenames above
trailing-directory matches. Wildcards in parent directories do not demote the
filename rank. Resolve ties only at the final rank, deduplicate same-library
matches and preserve explicit library overrides. Disk and logical inputs share
one bounded resolver; capture metadata once, retaining original buffers/handles
as source truth, with no native I/O.

`library_configs.rs` identifies configuration spans for bounded map admission.
Configuration-bearing maps are parsed by Slang's map preprocessor on the original
buffer, retaining macro-use locations and source text. Rust expands declarations
with bounded work before path admission; command-line defines seed each map,
while map-local defines stay in that map's source order and scope.
Admit unquoted per-library `-incdir` directories with the containing map as
relative base. Keep declaration order and library scope through Rust include
preflight and cache-only native lookup. Missing or inadmissible directories
reject at map admission; quoted `"-incdir"` remains a literal file pattern. In-memory
maps select logical directories that contain admitted buffers or are listed in
`logical_directories`, the representation of an empty logical directory.
Library sources follow the compilation-unit mode: separate gives each file its
own preprocessor; merged shares one per library, in admission order, in Rust
include preflight and in the wrapper alike. `Db::source_library` retains each
instance's bound library (and each package's/unit's) for `%l`.

Path-mode include directories are also definition search directories
(`compile/definition_search.rs`). After given sources, library files and their
includes are admitted, and only when an include directory holds candidates, the
native parse-only metadata request names what the admitted units reference but
none declares (instantiated definitions, imported or `::`-scoped
packages/classes, interface port types). Every `.v`/`.sv` file directly inside
each directory (non-recursive, not already admitted by canonical identity) is
read with bounded handle-checked reads and scanned once, in one batch, without
its own includes. A file declaring a missing name at its outermost level becomes
a default-library unit (never an implicit top), its includes are admitted like a
library source's, and its own missing names repeat the search. A missing name
declared by several candidates is a startup error naming every file; one
declared nowhere stays Slang's unknown-definition diagnostic. In-memory compiles
never search.

Edition checks use classified tokens and owned semantic edges shared by execution
and navigation. The 2001 profile also gates keyword-free later grammar: type
shapes (queue/dynamic/associative, multiple packed ranges), subroutine formals
and bodies, unnamed-block declarations and procedural initializers from semantic
nodes; labels, `.name`, `edge`, casts, time literals, inline genvars, `[size]`,
`localparam` ports and empty `()` from token sequences; generate constructs from
`generate` regions. Sequence rules skip macro-expanded tokens, whose shared
use-site range carries no order; the wrapper's closed syntax profile
(`edition_findings`) covers the grammar on macro-expanded trees and is merged
after these rules, dropping a finding at an already reported position.
Semantic rules also reject variables driven by continuous assignments, gate
outputs and output/inout ports, and variable input/inout ports. The 2009 profile rejects 2012 covergroup bins
forms. Body rules need executable capture; navigation snapshots skip them. Explicit `system_subroutines` prototypes distinguish extensions
from standards. Directive bodies are ignored until expanded; executable for-loop
header edges differ from navigation-only capture. Memory-storage exemptions are
argument-specific (`$fread` and legacy PLA personality argument 0;
`$readmem*` argument 1), never exemptions for unrelated whole-array uses of the
same declaration. PLA syntax admission does not select a simulator PLA target.

Frontend `Diag`s keep their physical `file`/`line`/`col` and carry the
`` `line``-mapped `logical` position, filled once per compile from the
snapshot's directives (`attach_logical_positions`); lint fills `LintDiag::logical`
from the Db source map. Printers use `Diag::location()` so every consumer shows
the same `` path:line:col (`line file:line)`` form.

See [FFI](../ffi/AGENTS.md), [simulator](../sim/AGENTS.md),
[LSP](../bin/llg_ls/AGENTS.md) and [lint](lint/AGENTS.md) for their contracts.
