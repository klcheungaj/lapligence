# `sim::ir`

- **Purpose:** define the typed intermediate representation shared by lowering,
  optimization, and C emission.
- **Validation:** check table references, widths, constants, array shapes, and
  process registrations at lowering and optimization boundaries.
- **Ownership:** IR tables and representation fields are implementation
  details; public constructors and accessors enforce local invariants.
- **Cross-table checks:** `IrModel::validate` and detached-node validation run
  before backend table indexing.

Recursive validation covers each IR variant and owns new cross-structure
invariants.
