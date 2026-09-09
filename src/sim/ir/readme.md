# Typed executable operations

- **Purpose:** define typed storage, expressions, lvalues and operations used by
  executable lowering. Process scheduling ownership lives in
  `sim::execution`; frontend meaning and synthesis classification live in
  `sim::semantic`.
- **Validation:** check table references, widths, constants, array shapes, and
  process registrations at lowering and optimization boundaries.
- **Ownership:** IR tables and representation fields are implementation
  details; public constructors and accessors enforce local invariants.
- **Cross-table checks:** `IrModel::validate` and detached-node validation run
  before backend table indexing.

Recursive validation covers each IR variant and owns new cross-structure
invariants.

`IrModel` is the staging owner used while converting semantic database nodes.
`ExecutionModel::lower` moves every process body out of that staging table and
into executable basic blocks. The staging process entries retain names,
helpers, and spawn identity because functions, storage, and call references
share their checked index tables.
