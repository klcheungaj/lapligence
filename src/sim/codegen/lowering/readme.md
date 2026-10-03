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

Electrical storage uses maximal adjacent runs whose member mappings, driver
ranges, force targets and effective type/delay agree. The partitioner in
`collection/net_partition.rs` serves arrays and scalar/vector alias networks;
whole vector nets keep their existing wide storage. Declared shapes remain
separate bit bindings, and contributions select contiguous RHS ranges before
resolution. New net topology belongs in the partitioner's source inventory.

Packed record member selections use the recursive descriptor and guarded typed
selection steps, including runtime indexed part-selects. Fixed record net arrays
map member paths and declared coordinates to canonical electrical bits before
range partitioning and driver planning; member selection materializes the selected
interval rather than the complete cell. Net admission excludes recursively nested
unpacked unions in the SystemVerilog-2009 profile. These paths reuse the existing
storage, validation, dependency and owned-emission operations.

Selected module references normalize to canonical storage plus ordered packed
selection steps. Composing bit, part and indexed selections preserves each
intermediate bound, including fixed-array element/member roots. The same typed
projection supplies reads, mutations, NBA capture and call copy-out; module
connections require constant selectors and keep subroutine ref-actual legality
separate.

See [lowering overview](../readme.md),
[execution model](../../execution/readme.md),
[feature boundaries](../../../../docs/sim_features.md) and
[source layout](../../../../docs/source_layout.md).
