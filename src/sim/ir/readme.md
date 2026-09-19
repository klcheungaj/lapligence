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
typed evaluator contexts, read dependencies, delayed NBA operations, deferred
immediate-assertion action frames and real math functions are validated before
optimization/emission; deferred updates remain distinct from suspension.
Evaluator contexts and assertion actions carry activation-owned storage
identities rather than transient C addresses.
Packed bit writes through subroutine references retain a typed index on the
reference lvalue. Validation, operand traversal, optimization and stack sizing
include that index. Unpacked member/element references can forward selected
views; frontend-illegal packed bit/part reference actuals remain rejected.
True-net aliases retain bit-level bindings to canonical resolved net groups so
optimized storage pruning cannot disconnect alias reads, dependencies, force/
release descriptors, or waveform observations.
`IrInitialization` keeps declaration identity, `StorageLifetime`, source origin,
and the Verilog/SystemVerilog execution phase attached to scalar and fixed
composite static initializers. Fixed targets retain checked persistent lvalues. Automatic declaration values remain activation-local operations;
static local storage is never initialized lazily by a first subprogram call.
Procedural delays retain either constant ticks or a typed runtime expression
with module-unit and precision scales. Validation, effect analysis, optimization
and stack sizing traverse that expression like other statement operands.
Inertial driver operations capture packed values for cancelable active-region
updates; their validation requires persistent whole-driver storage.

## Source organization

`ir.rs` retains `IrModel` and re-exports the existing operation API from domain
files for expressions, lvalues, calls, statements, events, assertions,
processes, functions, initialization, storage and VPI. Existing container and
object domains remain separate. `validate.rs` owns the validation context;
its `validate/` children check individual domains against that shared context.

See [the source map](../../../docs/source_layout.md).

## Bounded packed selection chains

`IrElemSel::PackedChain(Vec<IrPackedSelect>)` describes successive packed slices
inside a fixed-array element. Each step contains a typed integral physical-LSB
base and a positive result width. Its base is relative to the immediately
preceding value; its width includes any remaining element stride. Bases may be
negative, unknown, or outside the selected value. These are language-level
X/no-write cases, not invalid IR.

Validation rejects empty chains, real selectors/elements, zero step widths and
read results whose unsigned width disagrees with the final step. It also checks
all nested expression references and includes their widths in capacity accounting.
The `IrElemSel::expressions` and `expressions_mut` visitors cover every step;
optimization, effects/dependency discovery, address snapshots and stack sizing
must retain this traversal. Folding a base expression must not erase intermediate
bounds or merge adjacent steps. Runtime clipping is defined by the selected value
at each step, even if the root storage has further accessible bits.

Fixed formals retain recursive shape metadata and an exact-width payload. Union
shapes use their maximum member width; struct/array shapes use declaration
order. Default constants preserve each unpacked leaf's state domain and explicit
member initializer. Net arrays bind cells to canonical resolved signal/alias
storage; validators check the cell bounds and electrical target width.
