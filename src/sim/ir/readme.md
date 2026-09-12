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
helpers, spawn identity, process kind, program identity, and typed write
dependencies because functions, storage, and call references share their
checked index tables.
Concurrent assertion instances stay in a separate `IrModel::assertions` table;
their clock/disable sources, sampled predicate expressions, overlap mode,
labels, and Reactive action identities are validated without becoming ordinary
procedural blocks.

Variable aliases identify canonical storage explicitly. Event-evaluation helpers,
typed evaluator contexts, read dependencies, delayed NBA operations and real math
functions are validated before optimization/emission; deferred updates remain
distinct from suspension. Evaluator contexts carry activation-owned storage
identities rather than transient C addresses.
True-net aliases retain bit-level bindings to canonical resolved net groups so
optimized storage pruning cannot disconnect alias reads, dependencies, force/
release descriptors, or waveform observations.
`IrInitialization` keeps declaration identity, `StorageLifetime`, source origin,
and the Verilog/SystemVerilog execution phase attached to scalar static
initializers. Automatic declaration values remain activation-local operations;
static local storage is never initialized lazily by a first subprogram call.
Procedural delays retain either constant ticks or a typed runtime expression
with module-unit and precision scales. Validation, effect analysis, optimization
and stack sizing traverse that expression like other statement operands.
Inertial driver operations capture packed values for cancelable active-region
updates; their validation requires persistent whole-driver storage.
