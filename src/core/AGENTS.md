# core — shared processing layer (LSP + simulator)

## Purpose

The common processing core used by both the LSP server (`src/bin/llg_ls`) and
the simulator (`src/sim/`):

- `compile.rs` — unified Surelog pipeline (`CompileOpts`, `CompileOut`,
  `CompileError`, `Diag`): raw `compile` returns partial
  frontend results plus `CompileOut` diagnostics for the LSP, while
  `compile_checked` returns `CompileError` and withholds failed sessions from
  execution/elaboration consumers. A single-source `parse_only` path returns
  owned parse-tree tokens without preprocessing, compilation, elaboration,
  builtins, or cache reuse.
- `tokens.rs` — UHDM and parse-tree token collection. Isolated parse-only
  results are supplemented from the supplied source buffer for literal module
  boundaries when unresolved macros damage Surelog's tree. The source-local
  scanner uses UTF-16 columns and must ignore comments, strings, escaped
  identifiers, includes, and continued preprocessor directives.
- `db.rs` — **the owned UHDM database**: one VPI walk at build time
  (`Db::build`) captures the whole elaborated design as an arena of `Node`s;
  consumers read owned Rust data instead of calling VPI again.  Includes
  structural primitives (`vpiPrimitive` + `vpiPrimitiveArray` per
  instance/gen scope, captured as `NodeKind::Gate` with folded `GateTerm`s;
  switches/UDPs/arrays carry a class marker so the simulator can reject them
  at lowering time), interface instances (actuals and per-port copies),
  their modports/io_decls,
  and interface-port connections (`IfaceConn` children), so `sim` can emit
  interface link processes. Ordinary ports retain both the historical direct
  `high` target and an owned `high_expr` tree for expression-valued actuals,
  so consumers can recover parent-side reads after the Surelog session ends.
  Packages (`uhdmallPackages`) carry their items as
  children: parameters (values resolved via `elab::Resolver`), enum constants
  of the package's enum typedefs, and package functions/tasks
  (`NodeKind::FuncTask`).  Classes (`uhdmallClasses`, `Db::classes`) are
  per-file definitions with `NodeKind::ClassDef` children: data members
  (`vpiVariables` — ordinary variables, so `walk_var` is reused) and methods
  (`vpiMethod` — the class's task_func relationship, NOT `vpiTaskFunc`; the
  constructor is a function named `new`).
  Delay controls retain raw ticks or source spelling where Surelog lacks an
  expression relationship; supported evaluation forms are defined in the
  [simulator lowering guide](../sim/codegen/AGENTS.md).
- `elab.rs` — 4-state `Value` math + parameter/expression resolver
  (`Resolver`), used by the db build (and tests).
  Wildcard equality, bit-vector queries, and real/integer/IEEE-bit conversions
  are shared with simulator constant evaluation so parameter and runtime
  paths agree on the supported forms.
  Property tests in `tests/property_elab.rs` pin the `Value` math against
  per-bit references (deterministic fixed-seed proptest): X-propagation,
  resize low-bit preservation, concat/split round-trip, casez/casex wildcard
  truth tables and signed-compare consistency.  The same expected values feed
  the deterministic C cross-check vector table in
  `src/sim/rt/llg_rt_selftest.c`.
- `model.rs` — owned `DesignModel` (instances/ports/signals/params,
  `PackageDef` with params + enum constants, `ClassDef` with `methods` +
  `fields`), a projection of `db` via `DesignModel::from_db`.
- `macros.rs` — preprocessor macro tables for macro-usage hover.  Surelog
  carves `` `define `` directives out of every frontend artifact reachable
  over FFI, so the LSP recovers a conservative table instead: config
  `[compile] defines` (`-D` args) seed every analyzed file, then ONE scan
  per compiled file resolves simple/function-like defines (joining backslash
  continuations for display) and in-source `` `define``/`` `undef``/
  `` `undefineall`` and the `` `ifdef`` family positionally (last definition
  wins; comments, strings, and dead branches skipped; usages inside define
  bodies resolve at their definition point). An in-source redefine overrides
  config from that position onward; `undef` also removes config-seeded names.
  Evaluate ifdef/ifndef/elsif/else/endif against the evolving table.
  Documented approximations:
  includes are not followed (header-only macros read as undefined) and
  definitions do not leak across files — a
  lost value is possible, an invented one is not.  Pure Rust, no FFI, no
  LSP dependencies; built once per analysis commit (see
  `src/bin/llg_ls/features.rs::analyze_inner`), never inside a request.
- `lint/` — shared rule engine over `db` + `model`: `LintRule`/`LintCtx`/
  `LintDiag` and a registry of 24 default rules (see
  `src/core/lint/rules/mod.rs::default_rules` for the authoritative list).
  Consumed by the LSP (lint diagnostics with source `llg-lint`) and
  `llg --lint`.
- `vobject_types.rs` — Surelog parse-tree node taxonomy for semantic highlighting.

## Requirements

- **No `unsafe`** — all FFI access goes through `ffi`'s safe APIs
  (`vpi::read_value`, `iterate`/`handle`, `surelog::SessionBuilder`).
- **No LSP-only dependencies** (tower-lsp/tokio/dashmap stay in
  `src/bin/llg_ls`).  Core token collection is silent so the LSP binary can
  preserve stdout for framed JSON-RPC.
- No panics in library code; `Result`/`Option` throughout.
- The db is the **single VPI traversal point** — prefer extending `db` over
  adding new VPI walks.
- Missing bodies and `vpiNullStmt` lower to `StmtKind::Empty`; every unknown
  executable statement is retained as `StmtKind::Unsupported { vpi_type }`
  so simulator codegen rejects it instead of silently emitting a no-op.

## Interactions

- Below: `src/ffi/` (sessions, VPI).
- Above: `src/sim/` (codegen consumes `db` + `model`), `src/bin/llg_ls/`
  (`features::analyze` builds `db` → `model` + tokens and runs `core::lint`),
  `src/bin/llg.rs` (`--lint` gate runs `core::lint` before codegen),
  `src/bin/elab_check` (verifies elaboration via `compile_checked` + `ffi`).

## Elaborated model contract

The mandatory frontend flow is **parse + compile + elaborate + `-elabuhdm`** —
set all of: `set_parse()`, `set_write_pp_output()`, `set_compile()`,
`set_elaborate()`, `set_elab_uhdm()`. Without `-elabuhdm` the refs in the
UHDM point at **definition-level** objects instead of per-instance ones.

With the full flow, Surelog's UHDM output is already **elaborated** for
simulation purposes (verified empirically, 100% `vpiActual` ref binding on the
`tests/elaboration/` designs):

- Instance tree: `vpi_iterate(uhdmtopModules, design)` → `module_inst` tree,
  children via `vpiModule`.
- Per-instance parameters: `vpiParamAssign` on each instance
  (`vpiLhs` = parameter, `vpiRhs` = value expression, `vpiOverriden` flag).
  **Quirk**: when overridden, the `parameter` object itself keeps the stale
  *default* value — the real value only lives in the param_assign RHS.
  `core::elab::Resolver::scope_params` handles this (and evaluates unfolded
  expressions like `localparam W2 = W + 1`).
- Port binding: `vpiHighConn` → parent-scope ref, `vpiLowConn` → child ref.
- Processes (always/initial) and continuous assignments are cloned per
  instance with all refs re-bound via `vpiActual`.
- Generate loops → `gen_scope_array`/`gen_scope` with concrete genvar
  parameter values; conditional generate keeps only the taken branch.
  `GenScopeModel` retains each concrete scope's `full_name` and direct
  module/interface-instance children; nested generate boundaries are not
  flattened for hierarchy consumers.
- Ranges/types are folded to constants per instance (`[WIDTH-1:0]` → `[7:0]`,
  `$clog2()` evaluated).  `core::db::Db` also owns the ordered packed ranges
  captured for each elaborated instance/object (unpacked dimensions stay in
  array metadata), so consumers can render parameter-dependent widths after
  the Surelog session is gone; unknown bounds remain explicitly unresolved.

Residual elaboration work left to the consumer (documented in `core::elab` /
`core::model`): stale parameter objects (see above), always_comb / `@(*)`
sensitivity synthesis (event control has no condition), interface/modport
wiring (child interface view vs actual interface instance), and some
function-local typespec ranges.

For handle ownership and VPI relationship/type gotchas, read
[../ffi/AGENTS.md](../ffi/AGENTS.md) before extending DB capture.
