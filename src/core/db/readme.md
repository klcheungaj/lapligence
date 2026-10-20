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
Constrained randomization (IEEE 1800-2009 clause 18) is captured as typed
data: `NodeKind::ConstraintBlock` (qualifiers and the root constraint) and
`NodeKind::Constraint` (list, expression, implication, if-else, solve-before,
foreach with its loop-variable slots, and the later-edition
unique/disable-soft/soft forms the 2009 profile rejects); `ExprKind::Dist` keeps a
`dist` expression's items and `:=`/`:/` weights. `Db::rand_qualifier` gives a class property's `rand`/`randc`, and
`Db::randomize_call` identifies a class or scope `randomize` call and its inline
`with` constraints (`constraints.rs`).
`randsequence` (18.17) is `StmtKind::RandSequence` (productions and the item
that generates the first one) over `NodeKind::Production` nodes: declarations
(formals as automatic variables, a synthesized result variable for a non-void
production, rules), rules (elements, `:=` weight, weight code block, implicit
result variables of 18.17.7, `rand join` ratio) and items, code blocks, `if`,
`repeat` and `case` elements. Formal bindings and result copies are ordinary
assignment expressions on the item, and the result and implicit variables
carry reset assignments from a never-written variable of the same type, so
each generation starts from its type's default (`randsequence.rs`). Only the
randsequence statement lists the productions as children; they are members of
its block scope but not that scope's children, so a tree walk visits each code
block once.
Specify blocks (IEEE 1364-2001 14, IEEE 1800-2009 30) keep their nodes as
`NodeKind::Other` for identity and source range, with typed side tables
(`specify.rs`): `Db::specify_blocks`, `Db::module_paths` (connection,
polarity, edge, condition, ordered input/output terminal and delay
expressions, one record per elaborated instance), `Db::specify_items`
(pulse-style declarations with their terminals, timing checks with their
kind) and `Db::specparam`/`Db::specparams`, which mark specparams (and
`PATHPULSE$` ones) apart from elaboration parameters. A specparam's value is
its parameter constant; its node and each path node are the stable source
identities later annotation and timing-check work keys on.
Modport expression ports `.p(expr)` keep their port expression in a side
table (`Db::modport_port_expression`); references still target the port.
`Db::is_const_cast` marks `const'(expr)` casts (6.24.1), which are otherwise
ordinary `ExprKind::Cast` nodes.
Typed references distinguish subroutine
bodies, indexed pattern keys, event qualifiers and ordered conditional clauses.
Concrete loop-generate blocks use the array name and source index as one
hierarchical segment (`rows[0]`), so bound children retain standard paths after
the native snapshot is released.

The DB is the common semantic source for model/lint/LSP projections and simulator
lowering. Invalid IDs, table windows, ranges, cycles or relationship metadata fail
construction. See the [shared core](../readme.md) and
[source map](../../../docs/source_layout.md) for module ownership.
