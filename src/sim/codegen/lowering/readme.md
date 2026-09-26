# Lowering domains

`lowering.rs` owns `Codegen` state; responsibility-named child modules lower owned
semantic nodes into typed IR. C generation belongs to `emit_c`, not these modules.

| Domain | Responsibility |
| --- | --- |
| `collection/` | Instance/storage discovery, fixed layouts/defaults, initialization, ports, canonical nets and selected projections. |
| `statements/` | Assignments, source-loop control, process bodies, subroutine calls and procedural drivers. |
| `expressions/` | Typed values/conversions, conditionals, ordered predicates, patterns, `inside` and fixed reductions. |
| `containers/`, `objects/` | Admitted container/object operations and typed native boundaries. |
| Assertion, clocking and event helpers | Sampling/evaluator contexts, sequence plans, qualifiers, captures and regions. |

Fixed-value collection preserves immediate rows/members, declaration order and
state domains. Assignment plans distinguish frozen positional targets from
streaming selectors that can observe earlier unpacked fields. Packed selections
retain every intermediate bound; memory views distinguish HDL addresses from
physical strides. Subroutine lowering retains activation-relative storage,
reference identity and static/automatic initialization rather than inventing
synthetic global values.

See [lowering overview](../readme.md),
[execution model](../../execution/readme.md),
[feature boundaries](../../../../docs/sim_features.md) and
[source layout](../../../../docs/source_layout.md).
