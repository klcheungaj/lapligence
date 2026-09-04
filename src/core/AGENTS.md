# core — shared processing layer (LSP + simulator)

## Purpose

The common processing core used by both the LSP server (`src/bin/llg_ls`) and
the simulator (`src/sim/`):

- `compile.rs` — unified Surelog pipeline: raw `compile` returns partial
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
- `elab.rs` — 4-state `Value` math + parameter/expression resolver
  (`Resolver`), used by the db build (and tests).
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
  per compiled file resolves in-source `` `define``/`` `undef``/
  `` `undefineall`` and the `` `ifdef`` family positionally (last definition
  wins; comments, strings, and dead branches skipped; usages inside define
  bodies resolve at their definition point).  Documented approximations:
  includes are not followed and definitions do not leak across files — a
  lost value is possible, an invented one is not.  Pure Rust, no FFI, no
  LSP dependencies; built once per analysis commit (see
  `src/bin/llg_ls/features.rs::analyze_inner`), never inside a request.
- `lint/` — shared rule engine over `db` + `model`: `LintRule`/`LintCtx`/
  `LintDiag` and a registry of 24 default rules (see
  `src/core/lint/rules/mod.rs::default_rules` for the authoritative list).
  Consumed by the LSP (lint diagnostics with source `llg-lint`) and
  `llg --lint`.
- `tokens.rs`, `vobject_types.rs` — VPI/parse-tree object collection and the
  node-type taxonomy for semantic highlighting.

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
