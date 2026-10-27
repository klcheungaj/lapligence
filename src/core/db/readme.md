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
Type records are canonical and carry no typedef names; `Db::declared_typedef`
(variables, nets, parameters and typedefs, from `LLG_SLANG_EDGE_TYPEDEF`),
`Db::member_typedef` (aggregate members) and `Db::typedef_renames_builtin`
keep the typedef each declared type names, with the number of dimensions
declared above it, for VPI typespec names (IEEE 1800-2009 37.23).
`Db::instance_array_range` keeps the declared `(left, right)` of every
instance array level (`NodeKind::InstanceArray`), for VPI instance array
objects (37.11).
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
kind), `Db::specparam`/`Db::specparams`, which mark specparams (and
`PATHPULSE$` ones) apart from elaboration parameters, and `Db::path_pulses`
(`PATHPULSE$` reject/error limit values and the input/output terminal
declarations a `PATHPULSE$in$out` name resolves to, `Module` for the
module-wide one, `Unresolved` otherwise; single-value forms included), and
`Db::timing_checks` (each check's events in signature order with their
terminal, `TimingEdges` transition mask and `&&&` condition, its limit
arguments with empty positions, its notifier and its argument shape). A
specparam's value is its parameter constant; its node, each path node and
each timing check node are the stable source identities annotation keys on.
Modport expression ports `.p(expr)` keep their port expression in a side
table (`Db::modport_port_expression`); references still target the port.
Module, interface and program port expressions (IEEE 1800-2009 23.2.2.1,
23.2.2.2) are side tables too: a select or explicitly named ANSI port has no
whole `low` signal and keeps its child-side expression
(`Db::port_internal_expression`); a concatenated port is a `MultiPort` port
whose component ports, MSB first, each carry one part and its slice of the
actual (`Db::multi_port_components`, `Db::multi_port_of`).
`Db::is_const_cast` marks `const'(expr)` casts (6.24.1), which are otherwise
ordinary `ExprKind::Cast` nodes. `Db::is_parenthesized_assertion` marks
assertion expressions written in parentheses, which the frontend binds away;
clock flow stops at them (16.14.3).
A checker instance (IEEE 1800-2009 17.3) is a `ModuleInst` whose body
flattens like a module's; `Db::checker_instance` records procedural, bound
and nested instantiation, a `$unit` time scale and the instantiation
context's default disable, and `Db::checker_actual` marks the body
expressions a formal reference expanded to (its actual or default), so
lowering can read their sampled values. A formal passed as an argument of a
let, sequence or property instance, declared in the checker or outside it,
is marked where the instance's formal expands to it.
Typed references distinguish subroutine
bodies, indexed pattern keys, event qualifiers and ordered conditional clauses.
Concrete loop-generate blocks use the array name and source index as one
hierarchical segment (`rows[0]`), so bound children retain standard paths after
the native snapshot is released.

## Import and projection contracts

- Node IDs are stable arena indices. Parent/structural-child links are acyclic;
  typed resolved references may cross hierarchies. `InstanceBody` is normalized
  so instances directly own declarations, processes, subroutines and children;
  instance-array containers and entries are kept, with concrete entries in the
  enclosing instance's child list. Explicit statement blocks are kept.
- Implicit conversions and source- versus context-determined widths survive
  import and projection, as do complete delays.
- Kept metadata: native kinds for unsupported-node reachability, program origin
  (anonymous-program members flatten into their package or `$unit`), explicit
  storage lifetime and source qualification, enum declaration order and every
  `foreach` iterator slot (including omitted and packed dimensions). Event
  metadata keeps `iff`, event-array identities, nonblocking trigger mode and
  timing.
- Subroutine bodies use explicit references. Index keys fold as expressions, not
  spelling. Recursive instance-array expansion rejects cycles and repeated
  entries.
- Conditional clauses have dense, unique ordered indices; each
  `ConditionPattern` pairs with its condition, and `if` branches are explicit,
  not child-position heuristics. Tagged pattern member IDs and relationships are
  validated.
- A member access whose base is a function call (possibly through further member
  accesses) is `ExprKind::MemberSelect` with the computed base, and an element
  select of it keeps that node as its base; other non-path member accesses stay
  `Other`.
- Frontend-rejected input (editor analysis only; the simulator stops on the
  Slang error first): an instance of an unresolved definition (an unknown
  module, a black box) is `Other` with its parameter and connection expressions
  as children, and a continuous assignment Slang replaced with an invalid
  expression is `Other` instead of failing construction.

The DB is the common semantic source for model/lint/LSP projections and simulator
lowering. Invalid IDs, table windows, ranges, cycles or relationship metadata fail
construction. See the [shared core](../readme.md) and
[source map](../../../docs/source_layout.md) for module ownership.
