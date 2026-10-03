# Simulator lowering

`codegen.rs` lowers a validated `SemanticModel` into typed operations and an
`ExecutionModel`, returning `GeneratedModel` or a typed `CodegenError`.
It performs no native frontend calls, unsafe operations or C emission.

| Component | Responsibility |
| --- | --- |
| `lowering.rs`, `lowering/` | Shared lowering state and collection, statement, expression, container and object domains. |
| `timescale.rs` | Slang-resolved units/precision and checked femtosecond ticks across 1fs–100s. |
| `lowering/collection/fixed_*` | Checked layouts, defaults, patterns, casts and activation/member projections. |
| `lowering/expressions/conditionals.rs` | Array/structure merge plans before flattening loses immediate boundaries. |
| `lowering/collection/packed_elements.rs` | Dimension-relative packed selection chains over fixed-array elements. |
| `net_collapse.rs`, `port_net_types.rs`, `nets.rs`, `net_arrays.rs` | Directional net-type selection, canonical electrical identities, drivers and publication. |

Oversized whole integral array copies and supported streams lower to descriptor
operations instead of coordinate expansion. `collection/fixed_calls.rs` creates
lexical array activations for automatic nonrecursive oversized array-returning
functions; packed signatures keep their existing transport.

Collection consumes the semantic coverage ledger so reachable unknown executable
nodes reject with source locations. Initialization keeps declaration identity,
lifetime, origin and edition-specific phase. Process, assertion, clocking, event
and reference metadata remain typed through emission. Multiple optimizer variants
can reuse one DB through `generate_from_db_with_opts`.

`GeneratedModel::sources()` supplies the C model and its deterministic debugging
symbol map to the builder or source export. The
[emitter](../emit_c/AGENTS.md#model-lifecycle-and-checks) owns their naming and
sidecar format.

Subroutine collection traverses generated instance scopes at every pass (names,
signatures and bodies). Fork branches capture input event formals as event object
identities; nested branches resolve that capture instead of a callee formal.

See [lowering domains](lowering/readme.md),
[feature status](../../../docs/sim_features.md) and
[source layout](../../../docs/source_layout.md).
