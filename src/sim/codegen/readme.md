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
- **Evaluated events:** Explicit event expressions retain only their expression
  and qualifier dependencies. Read-only input/`const ref` function calls are
  checked transitively for disallowed effects, and automatic locals/formals are
  copied into typed evaluator frames before suspension.
- **Reference arguments:** Formal modes and `const ref` qualification are
  preserved in `IrFormal`; lowering rejects non-lvalues and incompatible
  packed/state shapes, and emits canonical alias descriptors without
  copy-in/copy-out temporaries.

See [`docs/sim_features.md`](../../../docs/sim_features.md) for the supported
feature surface and rejection boundaries.
