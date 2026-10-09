# Owned semantic database

`Db::from_slang` validates and imports the flat owned Slang snapshot, which the
FFI receiver builds from the wrapper's capture stream (node names and kind
spellings arrive interned as shared `SemanticText`). The arena
retains declaration identity, source text/ranges, types, constants, dimensions,
lifetimes, initialization, bindings, timing and unsupported-node metadata.
A validated builder supports frontend-independent tests.

Import validates the dense semantic ID domain and resolves IDs by checked arena
index without retaining an identity hash map.
Source positions use lazy per-file byte checkpoints and a bounded repeated-offset
cache. Queries scan at most one checkpoint interval and preserve byte columns
and UTF-8 boundary validation, with memory bounded by source bytes plus a small
fixed cache per queried file.

Per-node storage is kept proportional to distinct metadata: nodes share one
type descriptor per frontend type record, one native detail spelling per
distinct value, one file name per admitted file and, for unnamed nodes, their
enclosing scope's hierarchical name. Rare large node payloads are boxed;
`node_records_stay_compact` bounds the per-node record size.

Each node keeps its physical file, line and column, which remain the
diagnostic identity. `SourceMap` keeps the `` `line`` mappings separately,
per physical file name, built once from the snapshot's sorted records with one
pass over each mapped file; `Db::logical_position` derives a node's logical
file and line on demand. Files without a directive cost nothing per node.

`Db::legacy_directives` lists the active `` `default_decay_time``,
`` `default_trireg_strength`` and `` `delay_mode_*`` directives with their file,
line and column. Slang consumes them without effect, and an inactive
conditional branch contributes nothing, so this table is the only owned record
that lets the simulator reject them instead of accepting and ignoring them.

Projection normalizes implicit instance bodies and expands concrete instance-array
entries without losing source indices or explicit statement scopes. Packed ranges
use declaration IDs rather than names. Assignment-pattern nodes also retain their immediate packed element descriptor,
including canonical identity and ranges, for type-key matching after snapshot
destruction. Each `std::mailbox #(T)` specialization retains the descriptor of
its `T`, resolved in the specializing scope, keyed by the class type identity.
Modport expression ports `.p(expr)` keep their port expression in a side
table (`Db::modport_port_expression`); references still target the port.
Typed references distinguish subroutine
bodies, indexed pattern keys, event qualifiers and ordered conditional clauses.
Concrete loop-generate blocks use the array name and source index as one
hierarchical segment (`rows[0]`), so bound children retain standard paths after
the native snapshot is released.

The DB is the common semantic source for model/lint/LSP projections and simulator
lowering. Invalid IDs, table windows, ranges, cycles or relationship metadata fail
construction. See the [shared core](../readme.md) and
[source map](../../../docs/source_layout.md) for module ownership.
