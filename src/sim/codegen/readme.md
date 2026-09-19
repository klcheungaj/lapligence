# Simulator lowering

- **Purpose:** lower the validated `sim::semantic::SemanticModel` into typed
  executable operations and `sim::execution::ExecutionModel` scheduling.
- **Facade:** `codegen.rs` exposes the public generation API and typed
  `CodegenError` failures.
- **Implementation:** `lowering/` coordinates collection, statement lowering,
  expression lowering, and shared state; `timescale.rs` converts
  Slang-resolved module time scales and typed delay values into checked
  femtosecond scheduler ticks across the complete 1fs–100s range.
- **Boundary:** this layer performs no frontend API access, `unsafe`
  operations, or C source emission. Before collection, it consumes the
  semantic coverage ledger so reachable unknown executable nodes fail with
  source-located diagnostics; declaration initialization is lowered into
  typed operations with owned declaration identity, storage lifetime, source
  origin, and edition-specific scheduling phase; the C backend consumes only
  the resulting execution model.
- **Reuse:** `generate_from_db_with_opts` supports multiple optimization
  variants from one owned database.
- **Enum methods:** scalar enum first/last/next/prev/num/name calls capture
  declaration-order values and owned names from the database; navigation keeps
  the enum base shape and applies the specified wrap and default-value rules.
- **Real subset:** Scalar `real`/`shortreal` ports, combinational reads,
  level-sensitive `wait`, any-change event controls, ordinary real `case`, and
  blocking/NBA writes lower through typed double storage. Shortreal writes use
  IEEE single-precision rounding. Arrays, continuous real assignments, and
  real subprogram storage remain explicit boundaries.
- **Process contracts:** Lowering carries the exact `always`, `always_comb`,
  `always_latch`, and `always_ff` kind into typed IR with stable typed write
  dependencies. Combinational sensitivity follows called-function reads while
  excluding written storage; plain `@*` keeps Verilog call-site behavior.
  Single-writer, timing, and flip-flop event or assignment violations fail
  before emission, independently of lint.
- **Program blocks:** Slang-owned program identity is retained through the
  database and process IR. Initial processes launch in Reactive, while
  prohibited always/continuous/primitive/generate/nested-instance members
  fail before lowering; `$exit` is admitted only from a program process.
- **Concurrent assertions:** Packed `|->`/`|=>` properties and sequence
  assertions lower to dedicated IR assertion instances. A shared NFA
  retains `##` fixed/ranged delays, consecutive/nonconsecutive/goto repetition
  (including unbounded endpoints), `or`, direct one-cycle `and`/`intersect`,
  `throughout`/`within`, and `first_match` endpoint selection. Named
  sequence/property instances reuse Slang's owned actual/default expansion;
  one-cycle property `not`/`and`/`or`/`iff`/`implies` forms and compatible
  clock/disable metadata are composed without re-parsing source. Sequence-local
  packed values use per-attempt/per-thread snapshots; top-level typed local
  input formals (including declaration defaults) are initialized at attempt
  entry, and
  ordered whole-local assignments, increments, and void subroutine calls are
  evaluated at match endpoints. Legal multiclocked `##0`/`##1` sequence
  boundaries retain each segment's owned clock and edge, while default clocking
  metadata supplies an omitted property clock. Predicates use immutable sampled
  values; asynchronous `disable iff`, bounded `accept_on`/`reject_on` controls,
  synchronous variants, overlap mode, labels, and action callbacks remain
  explicit. Output/inout/ref formal copy-out, selected-local lvalues, repeated
  match-item bodies, conflicting clock or disable metadata, unsupported
  cross-clock delay/combinator forms, and temporal property operators outside
  this bounded subset remain source-located fail-closed boundaries.
- **Evaluated events:** Explicit event expressions retain only their expression
  and qualifier dependencies. Read-only input/`const ref` function calls are
  checked transitively for disallowed effects, and automatic locals/formals are
  copied into typed evaluator frames before suspension.
- **Clocking controls:** Owned clocking metadata drives input/inout sample
  storage, captured output/inout Re-NBA writes with constant skews, selected
  packed targets, and default-clock-bound `##N` event repetitions. A drive
  issued away from its event retains its captured value until the next event.
  Dynamic skews, sequence/property cycle timing, and frontend-rejected concatenated
  clockvar lvalues remain explicit boundaries.
- **Reference arguments:** Formal modes and `const ref` qualification are
  preserved in `IrFormal`; lowering rejects non-lvalues and incompatible
  packed/state shapes, and emits canonical alias descriptors without
  copy-in/copy-out temporaries.
  Packed bit writes within ref formals retain typed, range-translated indices
  and write through the original descriptor, including retained queue aliases.
- **True-net aliases:** Legal packed `alias` declarations are flattened into
  bit-level canonical net groups; structural drivers, packed links, force/
  release, dependencies, and waveform registration use the shared resolved
  identities while dynamic selects, aggregate forms, and switch-level paths
  remain explicit boundaries.
- **Deferred assertions:** `assert`/`assume`/`cover #0` conditions are lowered
  as issue-time samples with owned value captures and Reactive action
  callbacks. Actions remain within Slang's single-call contract; automatic or
  dynamic `ref` actuals, timing/control actions, and unsupported opaque values
  fail closed with source-linked diagnostics.
- **Mailboxes:** Typed and untyped mailbox declarations lower to runtime-owned
  FIFO handles with optional bounds, copy/identity-aware packed, real, string,
  and handle values, blocking and nonblocking methods, writable `get` targets,
  and process-local/static handle storage. Waiter suspension and cancellation
  are delegated to the runtime mailbox queues.

See [`docs/sim_features.md`](../../../docs/sim_features.md) for the supported
feature surface and rejection boundaries.

## Source organization

`lowering.rs` owns `Codegen` and coordinates domain modules in `lowering/`.
Collection, statements, expressions, containers and objects each have a small
facade with responsibility-named children. Assertion/clocking context,
references, initialization, delays and selections are separate lowering
concerns, not backend text-generation helpers.

See [lowering domains](lowering/readme.md) and
[the source map](../../../docs/source_layout.md).

## Packed selections of fixed-array elements

`lowering/collection/packed_elements.rs` resolves a fully indexed fixed unpacked
array root and records each following packed selection separately. Logical
indices are converted using that dimension's declared direction and right bound;
part-select counts include the complete remaining packed-element stride. The
result is `IrElemSel::PackedChain`, shared by reads and writable targets.

Coordinate arithmetic is widened before subtraction and multiplication. Index
operands are self-determined, including unbased fill literals; an unsigned high
bit must not wrap into a valid lane. Width admission remains checked against the
existing packed limit. Each chain step is relative to the preceding selected
value, not the root allocation, so invalid or partially invalid intermediate
selections retain their X/no-write positions. Do not replace the chain with one
summed offset without retaining all intermediate bounds.

Ordinary assignments, compound updates and NBA issue capture use the same target
recipe. Whole-array values and the existing packed formal/reference restrictions
are separate contracts. This source repair does not close the Group 1 release
gate; public HDL regressions are in `tests/sim_group1_repairs.rs`.
