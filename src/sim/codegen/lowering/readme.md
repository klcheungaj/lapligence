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
reference identity and static/automatic initialization. Static fixed-array
locals use one persistent signal per declaration and instance, shared by
subroutine writes, delayed NBAs and hierarchical fixed selections.
`initialization/order.rs` orders all static declaration initializers by the
static declarations they read (through called functions too), with collection
declaration order as the tie-break.

Electrical storage uses maximal adjacent runs whose member mappings, driver
ranges, force targets and effective type/delay agree. The partitioner in
`collection/net_partition.rs` serves arrays and scalar/vector alias networks;
whole vector nets keep their existing wide storage. Declared shapes remain
separate bit bindings, and contributions select contiguous RHS ranges before
resolution. New net topology belongs in the partitioner's source inventory.
Net-array cell sources are indexed once per lowering, so whole-array drivers
stay linear in the cell count. `collection/net_cells.rs` runs first: cells that
no driver, alias, selected inout connection or force target reaches (with their
whole-array inout peers) become typed constant runs (`IrNetArray`) and never
enter the bit-level graph; one representative per class shape enters the type
plan.

Continuous positional patterns publish each leaf as its own contribution,
including leaves that name alias views, net-array cells or their selected bits.
`collection/continuous_feedback.rs` repeats a zero-delay driver in place when
its own write can change one of its operands; other drivers keep the plain
sensitivity loop.
`collection/strength_views.rs` maps a `%v` operand to the resolved group bits
it reads and creates, on first use, a two-state strength-view signal for each
of those groups; nets that no `%v` reads have none.

Packed record member selections use the recursive descriptor and guarded typed
selection steps, including runtime indexed part-selects. Fixed record net arrays
map member paths and declared coordinates to canonical electrical bits before
range partitioning and driver planning; member selection materializes the selected
interval rather than the complete cell. Net admission excludes recursively nested
unpacked unions in the SystemVerilog-2009 profile. These paths reuse the existing
storage, validation, dependency and owned-emission operations.

Selected module references normalize to canonical storage plus ordered packed
selection steps. Composing bit, part and indexed selections preserves each
intermediate bound, including fixed-array element/member roots. Selections of a
whole multidimensional packed value, or of a packed array of records, address
whole elements through `packed_geometry.rs`; an in-range constant chain lowers
to one part-select of its root and one runtime step to an indexed part-select.
A member of such an element (`ps[i].hi`) is one more constant step of the
chain; a runtime element LSB absorbs the member offset, so it stays one indexed
part-select. Members of unpacked-array elements of packed arrays and of
packed-array structure members project through `fixed_projections.rs`. The same typed
projection supplies reads, mutations, NBA capture and call copy-out; module
connections require constant selectors and keep subroutine ref-actual legality
separate.

Overloaded updates (IEEE 1800-2009 11.11) whose value is used or whose target
selector has side effects arrive as `OverloadUpdate`/`OverloadPostUpdate`
assignments: the target, and the bound function's call whose `OverloadCurrent`
operand reads the target's current value. They lower to the ordinary
`IrExprKind::Mutation` (the call is its value, the operand its
`_llg_mut_current` capture), so the target resolves once; an unpacked record's
leaf `Stream` is admitted as a mutation target only here. Targets above the
packed value limit or with native members are rejected with a specific error.

See [lowering overview](../readme.md),
[execution model](../../execution/readme.md),
[feature boundaries](../../../../docs/sim_features.md) and
[source layout](../../../../docs/source_layout.md).

Descriptor-backed fixed patterns construct a lexical snapshot before publication.
Scalar defaults and uniform replications retain sparse defaults; resolved shared
positions become a default plus exceptions, and nonuniform replications use typed
loops. Value captures precede all scratch writes; blocking/NBA copies reuse the
fixed descriptor lifecycle and per-cell publication. Array-valued items requiring
oversized selected views remain subject to the descriptor transport limits.
Packed pattern keys use the captured immediate element type; equivalent
non-nominal integral types match structurally while enums retain nominal identity.
