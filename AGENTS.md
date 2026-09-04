# AGENTS.md

## Lapligence (llg) Project Overview

This repository implements a **Verilog/SystemVerilog simulator** and a
**Verilog/SystemVerilog Language Server (LSP)**, both built on Surelog + UHDM
and sharing a common processing layer.

- **Simulator flow**: Verilog/SV source → Surelog parse/compile/elaborate →
  UHDM model → owned `core::db` nodes → `IrModel` (`llg::sim::ir`) →
  optimization passes (`llg::sim::opt`) → C11 emission (`llg::sim::emit_c`)
  → C executable (runtime + libaco coroutines) that runs the simulation.
- **LSP flow**: tower-lsp server that drives the same Surelog pipeline for
  diagnostics, semantic tokens, hover, goto-definition, document symbols,
  completion, references, rename (`textDocument/rename` +
  `textDocument/prepareRename`), and a custom read-only module explorer
  snapshot.
- **Frontend**: the vendored Surelog v1.87 (`vendor/Surelog`) parses Verilog/SV
  and produces an elaborated UHDM database, accessed through a small C wrapper
  (`src/wrapper/surelog_c_api.{h,cpp}`) and safe Rust FFI/VPI modules.
- **References**: `vendor/synlig` (Surelog→Yosys frontend, same
  parse+elaborate flow), `vendor/yosys`, `vendor/verilator` (codegen +
  scheduling reference: V3EmitC, V3Sched, verilated_timing), `vendor/libaco`
  (C coroutines: `aco_create`/`aco_resume`/`aco_yield`/`aco_exit`, shared
  stack + per-coroutine save stacks). The Verilog/SystemVerilog LRMs live in
  `docs/specification/` as PDFs.

Rust and C++ interoperate via **FFI** (C ABI), not direct C++ name-mangled
interfaces. When in doubt: prefer **Rust** for new features; modify **C++**
only when required for performance, ABI stability, or legacy reasons.

---

## Repository Layout

```
src/
  lib.rs                        — crate root: `pub mod ffi; pub mod core; pub mod sim; pub mod memory_limit;`
  memory_limit.rs               — shared process-memory safeguard (watchdog + optional native limit;
                                  LLG_MEMORY_LIMIT_MB / LLG_MEMORY_WARNING_PERCENT / LLG_MEMORY_POLL_MS /
                                  LLG_MEMORY_ADDRESS_SPACE_LIMIT); used by both frontends
  ffi/                          — shared FFI layer (Rust ↔ C++); the ONLY module allowed `unsafe`
    surelog.rs                  — Surelog session mgmt, SessionBuilder, structured Diag, owned parse-tree nodes
    vpi.rs                      — safe VPI wrapper + `read_value()` → owned `ValueData`
    process_memory.rs           — platform-specific physical-footprint sampler (Linux/macOS/Windows)
  core/                         — shared processing layer (used by LSP AND simulator); unsafe-free
    compile.rs                  — unified compile pipeline (raw + checked contracts;
                                  CompileOpts/CompileOut/CompileError/Diag)
    db.rs                       — OWNED UHDM node database (single VPI walk; arena of Nodes)
    elab.rs                     — 4-state Value math + parameter/expr resolver (Resolver)
    model.rs                    — owned DesignModel, projected from db via `from_db`;
                                  generated scopes retain concrete identities and direct
                                  children, and signals retain net kinds
    lint/                       — shared rule engine + 24 default rules (LintRule/LintCtx/
                                  LintDiag/registry/LintConfig)
    tokens.rs                   — VPI + parse-tree object collection (semantic tokens)
    macros.rs                   — preprocessor macro tables for macro-usage hover
                                  (config `[compile] defines` seed + one conservative
                                  per-file `` `define``/`` `undef``/conditional scan;
                                  documented approximations, no FFI/LSP deps)
    vobject_types.rs            — Surelog parse-tree node taxonomy (VObjectType)
  sim/                          — simulator: Verilog/SV → IR → opt → C11 emission + runtime; unsafe-free
    codegen.rs                  — db → IrModel lowering (lower_expr/lower_stmt/lower_lhs; generate()/generate_with_opts)
    ir.rs                       — typed IR (signals/arrays/net-groups/functions/processes + IrExpr/IrStmt; sensitivity sets computed at lowering)
    opt.rs                      — conservative optimization passes (OptConfig: fold_constants/identities/prune_branches/unused_storage)
    emit_c.rs                   — C11 backend consuming only IR types (decoupled from core/db/ffi/vpi)
    build.rs                    — CMake-only model builder (build_model_cmake[_with_opts], CmakeBuildOpts,
                                  generate_model_sources, cmake_available; LLG_CMAKE/CMAKE_GENERATOR/LLG_CC/LLG_CFLAGS)
    rt/                         — C runtime sources embedded via include_str!
      llg_rt.h / llg_rt.c     — sv4_t 4-state ops + libaco event scheduler
      mod.rs                    — runtime_sources() / libaco_sources() / selftest / write_sim_sources
  bin/
    llg_ls/                      — language-server binary (tower-lsp): main, lsp, features, workspace, logging, semantic_tokens
    llg.rs                       — simulator driver: compile → codegen → build → run (+ --lint / --lint-json)
    elab_check.rs               — elaboration verifier tool
    hellouhdm.rs, helloworld.rs, llg_demo.rs — raw-API demos
  wrapper/                      — C wrapper (surelog_c_api.h/.cpp, mimalloc_shim.c)
  */readme.md                   — every module dir has a brief readme (purpose/requirements/interactions)
tests/
  elab_resolve.rs               — integration tests for core::elab
  model_tests.rs                — integration tests for core::compile + core::model
  property_elab.rs              — proptest properties (elab::Value) + C vector-table generator
  region_conformance.rs         — IEEE 1800 §4 scheduling-region conformance suite
  lsp_stdio.rs                  — framed stdio acceptance tests for multi-root LSP behavior
                                  behavior, parse-backed enum navigation, and the module explorer
  fixtures/lsp/module-explorer/ — module-explorer source-graph and declaration-fallback fixture
  config_effect.rs              — llg.toml compile-input end-to-end tests: `-D` defines
                                  select `` `ifdef ``/`` `elsif `` branches and `-P`
                                  param_overrides drive generate-branch selection,
                                  observed through the owned DesignModel
  sim_*.rs                      — end-to-end simulator suites (counter, function, fork, memory,
                                  interface, interface_body, casez, monitor, timescale, stress,
                                  geninit, varinit, wait, force, hier, inout)
  elaboration/                  — test designs + run_elab_check.sh regression suite
```

The lib crate contains **no** LSP-only dependencies (tower-lsp/tokio/dashmap
stay in the `llg_ls` bin). Those deps are optional behind the `lsp` cargo
feature (default-on), and the `llg_ls` bin has `required-features = ["lsp"]`;
`cargo build --lib --no-default-features` must compile without them. Bins
consume the lib via `use llg::core::…` / `use llg::ffi::…` — do not
reintroduce `#[path]` module includes in bins.
libaco is **not** linked into the Rust binaries: its sources are embedded
(`sim::rt::libaco_sources`) and compiled together with the generated C model
at model-build time.

CI lives in `.github/workflows/`: `build-binaries.yml` produces multi-platform
release binaries on tag push / manual dispatch — its Windows and macOS legs
are PLACEHOLDERS/untested because build.rs's native pipeline is validated
only on x86_64-linux-musl (see docs/platforms.md) — while `ci.yml` is a
lightweight fmt/check/clippy gate on ubuntu.  `docs/ROADMAP.md` remains the
plan of record for the remaining work.

## Module Rules (architecture invariants)

- **`unsafe` is confined to `src/ffi/`.** Enforced check:
  `grep -rn "unsafe" src --include=*.rs | grep -v src/ffi` must be empty.
  `ffi` exposes stable safe APIs (`vpi::iterate`/`handle`/`get`/`read_value`,
  `surelog::SessionBuilder`); everything else consumes those.
- **`core::db` is the single VPI traversal point.** `Db::build(design)` walks
  the elaborated design once into an owned arena; consumers (`sim::codegen`,
  `core::model::from_db`, `core::lint`, the LSP `analyze`) read owned `Node`
  data and never call VPI themselves. Prefer extending `db` over adding new
  VPI walks.
- **Read object values via `vpi::read_value` → `ValueData`** (owned enum), not
  the raw `VpiValueData` union (that union is read in exactly one place, inside
  `ffi/vpi.rs`).
- Keep the per-module `readme.md` files accurate when the module's
  purpose/requirements/interactions change.

---

## Tooling & Build

- **Build system**: Rust `cargo` (with `build.rs` driving a `cmake` build of
  Surelog v1.87 + UHDM + ANTLR), C++ via cmake.
- Static musl builds are the norm (`x86_64-unknown-linux-musl`); Linux and
  macOS are supported, Windows is not.
- The C/C++ world is linked with mimalloc: binaries set the `#[global_allocator]`
  at their final link point (see `src/bin/llg_ls/main.rs` and
  `src/bin/helloworld.rs`), and `build.rs` + `mimalloc_shim.c` redirect C
  malloc/free via `--wrap`.
- The native libs are linked via `#[link(name = "surelog_c_wrapper", kind =
  "static")]` attributes inside `src/ffi/surelog.rs` and `src/ffi/vpi.rs`.
  Do not remove them: cargo applies build-script `link-lib` output to the lib
  target, and bins receive the native libs transitively through the lib crate
  only because of these attributes.
- Editing `src/wrapper/*` triggers a wrapper rebuild (fast); editing
  `vendor/Surelog` or its CMake config triggers a full Surelog rebuild (slow).
- Build targets: `cargo build --bin llg_ls` (language server), `--bin llg`
  (simulator driver), `--bin elab_check` (elaboration verifier), plus the
  demo bins.
- LSP-only dependencies (`tower-lsp`/`tokio`/`dashmap`) are optional,
  gated behind the default-on `lsp` feature. They are used only by the
  `llg_ls` bin, which declares `required-features = ["lsp"]`; lib-only builds
  can disable them with `--no-default-features`.
- `sim::build` (the only model builder) honors `LLG_CC` (compiler program;
  falls back to `$CC`, then `cc`) and `LLG_CFLAGS` (extra flags appended to
  `-DCMAKE_C_FLAGS`) — useful for sanitizer-instrumented model runs.
- Both frontends can enforce a process-wide physical-memory budget via the
  shared `memory_limit` safeguard (`LLG_MEMORY_LIMIT_MB`, plus
  `LLG_MEMORY_WARNING_PERCENT`/`LLG_MEMORY_POLL_MS`/
  `LLG_MEMORY_ADDRESS_SPACE_LIMIT`): the LSP also wires a memory sampler into
  lifecycle logging, `llg` reports status to stderr.  See
  `docs/lsp_safeguards.md`.

---

## The Elaboration Pipeline (verified against Surelog v1.87)

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

---

## UHDM/VPI Field Notes (gotchas learned the hard way)

- **Handle lifetimes**: handles from `vpi::iterate` are borrowed; handles from
  `vpi::handle(...)` return `vpi::OwnedHandle` which frees the wrapper on drop
  — keep the OwnedHandle alive in a local and use `.raw()` for nested calls.
  Never store the raw pointer of a dropped OwnedHandle (use-after-free).
- **1-to-1 vs 1-to-many**: `vpi_iterate` returns null for single-object
  relationships (`vpiRhs`, `vpiLhs`, `vpiStmt` on a single stmt, …) — use
  `vpi_handle` for those. Prefer the `iter`/`child_handle`/`each_child` helper
  pattern from `src/bin/elab_check.rs`.
- **No `vpiValue` string property** in this UHDM build: read constants and
  parameter values via the safe `vpi::read_value` → `ValueData` (formats
  `vpiBinStrVal`…, `vpiIntVal`, `vpiUIntVal`, `vpiStringVal`). The raw
  `VpiValueData` union is read in exactly one place (inside `ffi/vpi.rs`).
  `vpiSize == -1` means an unsized literal (`'1`, `'x`): fill-on-resize
  semantics (see `elab::Value::fill`).
- **Operation op-types** use UHDM's numbering (`vpiAddOp`=24, `vpiSubOp`=11,
  `vpiEqOp`=14, …) — use the `vpi.rs` constants, never magic numbers.
- Top-level `module_inst` objects have **no `vpiFullName`** (use `vpiName`).
- `vpi_get(vpiType, …)` returns VPI-mapped constants (e.g. `vpiRefTypespec`),
  not the raw `uhdm*` discriminants.
- Port typespecs live under `vpiTypedef`; nets/vars/params under
  `vpiTypespec`.
- `gen_scope` is reached with `vpi_iterate(vpiGenScope, gen_scope_array)`
  (`vpi_handle` returns null there).
- `initial` processes report `vpiAlwaysType` = 1 (same as `always`) —
  distinguish by object type (`vpiInitial`).
- Concat operands may be reversed (Surelog sets `vpiReordered`); respect it.
- `indexed_part_select` uses `vpiBaseExpr`/`vpiWidthExpr`, not `vpiIndex`/`vpiSize`.
- Surelog prefixes top design-unit names with the library, e.g.
  `work@param_top` — strip that known top-level `lib@` qualifier for display
  and source matching. Do not apply this rule to arbitrary `vpiName` values:
  an escaped SystemVerilog source identifier can legally contain `@`.
- `-nowarning` **removes** warnings from the error container at add-time (not
  just at print time); `-noinfo` still leaks one `CM0023` info diagnostic.
- Driving flags via setters requires `set_write_pp_output()`; without it the
  design comes out empty.
- `vpi_iterate(vpiParamAssign, …)`/`vpiParameter` work on gen_scope objects too.
- The parse-tree C ABI (`SL_VObjectInfo` → `surelog::ParseNode`) carries
  `parent_index`, `child_index`, and `sibling_index`; zero is Surelog's
  invalid-node sentinel.  These are owned links used by the LSP's source
  graph and enum scanners.  Parse-node positions remain 1-based and may be
  zero/unknown, so convert with `saturating_sub` when forming 0-based keys.

---

## LSP Architecture Notes

- Surelog is blocking and uses C++ global singletons: all Surelog work runs in
  `tokio::task::spawn_blocking` behind a process-wide `Mutex`.
- `SurelogSession` is **not** `Send`: compile + db build + token collection
  must all happen inside one blocking closure; the session is dropped there.
  `Analysis` (db-derived model, tokens, diagnostics) is fully owned (`Send`)
  and crosses the thread boundary. `features::analyze` builds
  `core::db::Db::build(design)` then `model::DesignModel::from_db(&db)`.
- LSP position conventions: LSP wire positions are 0-based; `Diag` from
  Surelog is 1-based; convert once in `features::lsp_diagnostics`.
- Token collection is silent.  The LSP logger is implemented in the binary,
  writes only to stderr or the configured `LLG_LOG_FILE`, and never writes to
  stdout because stdout carries framed JSON-RPC.
- The Backend debounces root re-analyses (~300 ms trailing edge) with
  latest-wins coalescing (`scheduler.rs`): bursts collapse into one run per
  root, triggers landing during a running analysis never queue another job —
  they mark the root dirty and completion arms exactly one follow-up over the
  newest inputs; stale results are dropped via a generation counter.
  Diagnostics are published project-wide after each
  compile — every analyzed file gets a publication, open or closed (empty
  lists clear stale errors, and identical payloads are suppressed via per-
  root digests).  `didClose` does not clear diagnostics: the post-close
  recompile refreshes the URI from on-disk state.
- Request memoization: read-only navigation results (definition/hover/
  references) and open-buffer isolated token streams are memoized in a small
  bounded LRU (`bin/llg_ls/request_cache.rs`) keyed on the exact inputs —
  request kind/parameters/position plus an analysis epoch that `commit_job`
  re-stamps whenever the served snapshot is replaced or cleared (buffer text
  and `-D` defines hash into the token-stream key).  Any edit, save or config
  hot reload therefore invalidates structurally at the next commit; identical
  repeats are answered from memory with presentation mapping applied per
  request.  Identical full-text `didChange`s are not rescheduled.
- Module explorer (`src/bin/llg_ls/module_explorer.rs`): the custom
  `llg/moduleExplorer` request reads only committed per-root `Analysis`
  snapshots.  An optional `workspaceUri` (also accepted as `rootUri`) filters
  one root; an empty object or omitted params returns a deterministic merged
  multi-root snapshot.  It never parses, reads files, or touches live VPI
  handles during the request.  The analysis-time source module graph preserves
  definitions and instance edges omitted by configured elaboration, while
  the owned DB supplies per-instance packed ranges for concrete types.
  Responses contain `modules` and recursive `roots` with stable source- and
  hierarchy-based IDs, typed ports/parameters/signals, declaration locations,
  elaborated or declaration-only content, and generate-scope boundaries
  (including direct children and nested scopes).  Duplicate definitions,
  cycles, and the bounded response budget are represented with marker flags.
  Response accounting reserves useful prefixes for both hierarchy roots and
  module definitions before optional port/parameter/signal contents consume
  the remaining budget, so a large hierarchy or declaration cannot empty the
  other half of a valid snapshot. Multi-workspace requests partition both
  root and module-catalog capacity deterministically, with unused catalog
  capacity rolling forward, so an oversized earlier workspace cannot starve
  later roots. The explorer must not guess a definition or expand without a
  safe identity.
  A servable committed model/source-graph/top change or a cleared snapshot
  sends the empty-params `llg/moduleExplorerChanged` notification so clients
  refetch.  Fatal or diagnostics-only commits that retain the served snapshot
  do not send it.
- Feature gating: navigation features (hover/definition/references/symbols/
  completion/semantic tokens) serve best-effort from the latest per-root
  analysis that carries feature data (`Analysis::has_feature_data` — any
  non-Fatal outcome with servable index/model/token data, including Parse/
  Compile outcomes over the surviving files).  A project with a SYNTAX error
  (Surelog skips its whole compile/UHDM stage) serves DECLARATION-LEVEL
  features via the parse-tree fallback (`parse_tree_feature_parts` in
  `features.rs`): parse-tree tokens plus a modules-only model (name +
  declaration position per parsed module header), so module/class/function
  declarations navigate while instance-level data and cross-reference/
  use-site resolution stay unavailable until the project compiles.  Only
  Fatal analyses (no or poisoned UHDM, include-isolation preflight aborts)
  — and analyses with no servable data at all (empty projects) — are
  feature-less; an unservable analysis never replaces the retained snapshot.
  Diagnostics always publish.
- Workspace roots are independent.  Each root loads its effective `llg.toml`
  (v1 schema; the client may pass per-root config-file overrides in
  `initializationOptions`).  Discovery indexes only `.v`/`.sv` compilation
  units; `.vh`/`.svh` and arbitrary-extension files enter analysis only
  through include resolution.  Root-relative include/exclude patterns use
  exclude-wins semantics, and longest-root ownership over configured include
  directories remains authoritative even when a nested root's discovery
  filters reject a file.  Standard watched-file notifications are used; the
  server does not run an internal filesystem watcher.
- Compile inputs from config: `[compile] top` selects the elaboration top,
  `[compile] defines` (`NAME`/`NAME=VALUE`) become `-D` preprocessor defines
  for every analyzed source of the root (`` `ifdef ``/`` `ifndef ``/``
  `elsif `` evaluation and macro expansion), and `[compile.param_overrides]`
  (`NAME` = string or integer) becomes a Surelog `-PNAME=VALUE` top-level
  parameter override — the equivalent of `top -GNAME=value`, driving
  parameter-conditioned generate selection during elaboration.  Overrides
  apply to the top-level instances only (explicit `top` or auto-detected);
  there is no per-module scoping, and an override no top module declares is
  reported as an error diagnostic.  Invalid define entries or invalid/empty
  override keys/values fail soft: dropped with a warning against the TOML
  URI.  These edits hot-reload WITHOUT a server restart (the config file is
  watched and any parsed-config change reschedules the root's analysis); a
  restart happens only when the client's VS Code settings change
  (`hdlsupport.llg.path`/`.configPath`/`.logLevel`).
- Workspace indexing: `Analysis` carries a `SymbolIndex` (decls + refs with
  positions, built from the model + token collection). Declarations come from
  the model; reference sites from `vpiRefObj`-typed tokens. Module/instance
  decl positions are refined to the identifier token (Surelog points module
  defs at the `module` keyword and instances at the type name). Named port
  connections resolve each side to its OWN declaration: the label
  (`.clk`, `via=label`) navigates to the child module's port declaration,
  while the connected signal (`via=connection`) stays on its own declaration
  in the instantiating (parent) scope — selected by enclosing-module-span
  containment of the instantiation line with a nearest-above tie-break and
  fallback; with no parent-scope candidate the actual stays unbound.
  Named PARAMETER overrides (`child #(.W(4)) u0 (...)`) follow the same
  shape: the label navigates to the child module's parameter declaration and
  the override RHS expression stays in the instantiating scope.  This holds
  including multi-line instantiations (port labels via a position-based
  heuristic, see `PORT_LABEL_MAX_SPAN`; parameter labels via the exact
  parse-tree pairing, since the override list precedes the instance name and
  defeats same-line heuristics; pairing — positions AND identifier text —
  comes from the parse-tree scan `scan_named_port_connections`).  The same
  semantics hold in parse-fallback mode, where labels are resolved against
  the recorded port/parameter declarations by exact name match and the
  actual/RHS against the recorded parent-scope declarations.  Unresolvable
  override labels yield NO definition instead of a wrong same-name jump.
  Parent-side nets stay reachable through find-references and hover.
  Parameter/localparam hovers (declaration sites and binding-precise use
  sites alike) append an elaborated-value line (`value = <const>`) sourced
  ONLY from the committed analysis snapshot: the declaration's enclosing
  module span collects its instance family's same-named direct/gen-scope
  parameters, unanimity renders that one constant (so `[compile.param_overrides]`
   / Surelog `-P` overrides show wherever they are committed), and ambiguity
   or an unresolved value omits the line rather than guessing — nothing is
   parsed or elaborated inside the hover request.
   MACRO-USAGE hovers resolve `` `NAME `` references the same committed-data
   way (`core::macros::MacroTable`, attached to `Analysis` once per commit;
   Surelog exposes no macro table over FFI — definitions are carved out of
   UHDM and the parse tree during preprocessing): the root's
   `[compile] defines` seed every analyzed file as command-line defines and
   one conservative scan per compiled file then resolves in-source
   `` `define``/`` `undef``/`` `undefineall`` positionally (last definition
   wins; `` `ifdef``-family conditionals evaluated against the evolving
   table; comments/strings/dead branches skipped), so an in-source redefine
   overrides the config value only from its point onward.  A usage shows
   `macro WIDTH = 8` (house style), a body-less macro just its name, and an
   UNDEFINED usage an explicit not-defined message naming the effective
   config file — never a wrong value.  Documented approximations: include
   files are not followed (header-only macros read as undefined) and
   definitions do not leak across files.
   Inner-scope SHADOWING is binding-exact wherever UHDM captured the use:
  the token walk descends into function/task bodies, named begin/fork blocks
  and generate-block scopes (the latter only via the elaborated instance
  tree), so references there bind to the innermost visible declaration;
  hover consults those bindings first and renders position-accurate
  declaration snippets (`decl_details`) instead of the first same-named
  model object, find-references splits conflated reference sets per bound
  target, and goto-definition on a shadowing DECLARATION resolves to itself.
  Parse-backed enum facts fill a Surelog gap: package/class-qualified enum
  members, imported bare members, and bare members with exactly one visible
  declaration are bound at the exact member position in both full and
  syntax-fallback analyses.  Synthetic enum tokens keep those uses indexed;
  ambiguous bare/qualified members are recorded as unresolved so definition,
  references, and rename do not fall through to a misleading same-name match.
- Lint: `Analysis` also carries `core::lint` findings over the db + model;
  they are merged into the published diagnostics with `source: "llg-lint"`
  (rule id as the diagnostic code; severity Error → `ERROR`). The lint
  configuration comes from each root's `llg.toml` `[lint]` table (see
  `config::translate_lint`); `llg-lint.toml` is not read by the LSP.
- Unsaved buffers: `did_open`/`did_change` stage buffer text into a private
  per-process shadow tree (`llg-{pid}-{rand}` under the OS temp dir,
  mirroring absolute paths) and the compile runs on the shadow paths.
  Literal include dependencies are staged transitively, including unopened
  disk files and open non-HDL or filtered buffers; open text wins over disk
  text.  Diagnostics/tokens/definitions are mapped back to the real URIs
  (`shadow_path`/`real_path` helpers).  Include targets are authorized under
  the owning root's configured source/include directories (which may be
  external); lexical, canonical, and symlink checks reject targets that
  escape every configured directory.  The server never writes into project
  or external trees.
  Semantic-token requests for an open document use a separate request-local
  staged copy and Surelog parse-only mode. Inactive conditional-compilation
  branches plus non-lexical compiler-directive lines (and directive
  continuations) are replaced with position-preserving spaces because
  `-parseonly` otherwise misdiagnoses valid directives such as `` `include``;
  project units and include contents are never consumed. This path does not
  publish diagnostics or update the retained project analysis; unopened
  documents use cached project tokens.  Any syntax diagnostic for the current
  buffer returns an authoritative empty token stream, preventing unstable
  partial highlighting while the user edits incomplete syntax. Only
  staging/session/task failures fall back to cached tokens; valid unsaved
  buffers still use their isolated parse result, and a successful empty stream
  is authoritative.
  Identical open-buffer misses are single-flighted on `(uri, text, defines)`
  with a bounded in-flight table; a captured revision that is no longer the
  current document snapshot is rejected BEFORE cache serving, flight
  admission, and any parse-only Surelog work (re-checked inside the blocking
  parse), so a stale revision never occupies a flight slot or starts an
  obsolete parse — it completes with a stale outcome on the committed cache
  fallback.
  Connection-label highlighting: named PORT connection labels (`.clk` in
  `.clk(wa)`) and named PARAMETER override labels (`.W` in `child #(.W(4))`)
  carry the custom `connectionLabel` semantic-token MODIFIER on top of their
  historical base types (`function` for port labels, `property` +
  `readonly` for override labels), so stock themes keep today's colors and
  one `*.connectionLabel` theme rule distinguishes labels from the connected
  signals (plain `variable`).  The legend advertises modifier bit 10; the
  marking comes from the parse-tree classifier's `TOKEN_PORT_CONN_LABEL` /
  `TOKEN_PARAM_CONN_LABEL` synthetic types and therefore holds in BOTH
  serving paths — the cached project-index tokens and the isolated
  open-buffer parse-only stream.
- `LLG_LOG=off|error|warn|info|debug|trace` controls low-overhead lifecycle
  logging; `LLG_LOG_FILE` selects an append-only file and logging is disabled
  below the configured level before formatting or writing messages.
  Lifecycle records (`LifecycleSpan`) cover requests/notifications, root jobs
  and analysis phases with process-unique `id`/`parent_id` correlation plus
  root, generation, outcome, elapsed time, result cardinality, and a physical
  memory sample when a sampler is installed.
- Known v1 limitations: definition/references are binding-precise at captured
  reference positions (UHDM `vpiActual` targets plus named-connection
  folds — the port label navigates to the child module's port, the
  parameter override label to the child module's parameter; connected
  signals and override RHS expressions resolve to their own parent-scope
  declarations), and those captures now cover every inner-scope interior
  (function/task bodies, named begin/fork blocks, generate blocks) so
  shadowed declarations navigate exactly there; away from those positions
  definition/references are name-based + scope-aware approximations that can
  still conflate same-named declarations across scopes; package/class item
  resolution covers `pkg::item` /
  `Class::member` style references.  Parse-backed enum facts additionally
  resolve qualified package/class members, imported bare members, and bare
  members with one visible declaration at their exact use position, including
  syntax-fallback analyses; ambiguous enum uses deliberately have no
  definition/reference result instead of using a same-name fallback.  Other
  package use-site references that Surelog folds to constants still lack
  goto-definition; class member-expression resolution (`obj.count`) is out
  of scope; classes remain unsupported in the simulator.  Literal include
  preflight does not resolve macro-generated or dynamic include paths; if
  shadow staging fails, the server falls back to on-disk text.

---

## Linter

`core::lint` — shared rule engine over the owned db + design model.  Rules
implement `LintRule` (`id`/`description`/`check(ctx)`); `LintCtx` hands each
rule the `Db` + `DesignModel`; findings are `LintDiag` (rule id, severity,
file, 1-based line/col, message).  The registry runs 24 default rules in a
stable order: `unused-signal`, `width-mismatch`, `incomplete-case`,
`combinational-loop`, `multi-driver`, `casez-misuse`, `if-latch`,
`naming-style`, `blocking-in-always_ff`, `nba-in-always_comb`,
`unused-parameter`, `implicit-net`, `case-default-missing`,
`comparison-width-mismatch`, `unconnected-port`, `mixed-assignments`,
`undriven-signal`, `incomplete-sensitivity-list`, `out-of-range-select`,
`xz-logical-equality`, `duplicate-case-item`, `empty-implicit-sensitivity`,
`assignment-in-condition`, `casex-statement`.  No VPI access, no raw FFI, no
LSP dependencies.

New-rule notes: `implicit-net` flags nets Surelog auto-created from
undeclared identifiers (signature in the owned db: a net whose type info has
no typespec — kind `"other"`; positioned at its creating use site).  It is
the complement of `incomplete-case`, which owns exact-`case`-without-default
in combinational/latch processes; `case-default-missing` covers everything
else (casex/casez anywhere, exact case in edge-sensitive/initial/final
processes and function/task bodies) and skips the incomplete-case domain so
a location is never reported twice.  Both width rules degrade to "skip" when
an operand width is unknown (`width-mismatch` for assignments/port links,
`comparison-width-mismatch` for comparison operators).

Configuration: `LintConfig` (per-rule `enabled` + `severity` override) is
parsed from a hand-rolled `llg-lint.toml` reader (`LintConfig::parse_toml`)
used by the simulator CLI (`llg --lint-config <file>`).  The LSP does
not read `llg-lint.toml`; it derives each root's `LintConfig` from the
`llg.toml` `[lint]` table (see `src/bin/llg_ls/config.rs`).

Consumers: the LSP merges findings into the published diagnostics with
`source: "llg-lint"` (severity Error → `ERROR`, rule id as the diagnostic
code); `llg --lint` prints findings and aborts with exit code 1 on lint
errors before codegen; `--lint-json [file]` emits a machine-readable JSON
report (see `core::lint::diags_to_json` for the schema).

---

## Simulator Architecture (v1)

Flow: `llg` (src/bin/llg.rs) → `core::compile::compile_checked` →
`sim::codegen::generate(uhdm_design)` (builds `core::db::Db` internally and
lowers from the owned database — no VPI calls in the emitter) → `IrModel` →
`sim::opt` passes → `sim::emit_c` C11 emission → write `target/sim/<design>/`
(shared `sim::write_sim_sources` helper) → build the model executable → run.

**CMake is the only model builder**, invoked automatically by the driver
right after C emission (no user step): `sim::build::build_model_cmake`
generates a `CMakeLists.txt`, runs `cmake` (`$LLG_CMAKE` override,
`-DCMAKE_C_COMPILER` from `$LLG_CC`/`$CC`/`cc`, `LLG_CFLAGS` appended to
`-DCMAKE_C_FLAGS`), and locates the exe under `<build>/bin/`.  Generator
selection: `CmakeBuildOpts.generator` (driver `--generator <backend>`) >
`$CMAKE_GENERATOR` > cmake's host default.  `sim::build::generate_model_sources`
writes sources + `CMakeLists.txt` only (`--gen-only`);
`sim::build::cmake_available()` probes for a usable cmake once per process
(test suites skip gracefully without it).

- **4-state values**: `sv4_t { uint64_t bits[16], x[16], z[16]; uint16_t
  width; int8_t is_signed }` — up to 1024 bits (`LLG_MAX_WIDTH`) stored as
  64-bit limbs, with X and Z kept distinct: `$display` prints 'x' vs 'z',
  `===`/`!==` compare them literally, and casez/casex wildcard matching
  follows LRM 12.5.1.  In every other expression context Z behaves as X (LRM
  11.4.5) while identity/copy ops (mux with a known select, selects, resize,
  concat) carry Z through.  Ops mirror `core::elab::Value` semantics (keep
  them in sync; enforced by `tests/property_elab.rs` proptests + the
  deterministic C vector table in `llg_rt_selftest.c`).
- **Processes = libaco coroutines**: one coroutine per always/initial block
  (including generate-block processes), continuous assignment, and port link;
  fork branches are children spawned with `llg_fork`. The scheduler is the
  main coroutine; processes suspend via `llg_wait_time` (#delay),
  `llg_wait_edge` (@(posedge/negedge)), `llg_wait_any` (@* / comb
  sensitivity, snapshot-based), `llg_wait_any_events` (event or-lists),
  `llg_join`/`llg_wait_fork`/`llg_disable_fork` (fork/join), and the
  `wait (cond)` loop.  Coroutines must not return without
  `llg_proc_done`/`aco_exit` (the runtime aborts on that — codegen bug).
- **Scheduler (IEEE 1800 §4 region model)**: per time step, the region loop
  runs active (ready coroutines FIFO) → **inactive (`#0`, between active and
  NBA)** → NBA (commit per-process `llg_nba` lists) → re-* iterations until
  quiescent → advance time to the next timed wakeup → break on
  `$finish`/deadlock.  Edge detection uses per-waiter last-seen values
  (posedge: 0→1, 0→X, X→1; negedge mirrored).  `force`/`release` override
  procedural writes via a per-signal force table (release restores the
  pre-force value — drivers that changed while forced are not re-evaluated;
  documented approximation).  Inout ports collapse into `llg_net_t` groups
  with LRM wire/tri resolution (all-Z → Z; equal non-Z → value; conflict/X →
  X).  Time advances in design-precision ticks: the codegen scales `#N`
  delays and `$time` per the calling module's `timescale` unit (modules
  without a directive default to 1ns/1ps with a warning), so the runtime
  stays timescale-agnostic.
- **Codegen rules of thumb** (learned the hard way):
  - Continuous assigns and `always_comb`/`@*` become comb processes:
    evaluate once at t=0, then `wait_any` on the RHS/body **read** set — the
    LHS base signal must NEVER be in the sensitivity list (self-wake bug).
  - Event or-lists (`@(posedge a or negedge b)`) must be ONE atomic
    `llg_wait_any_events` call, never sequential waits.
  - Port connections become link processes (input: child←parent; output:
    parent←child) — no aliasing, so edge detection stays per-signal.  Inout
    ports emit no link — the net group IS the connection.  Interface body
    processes emit under the ACTUAL interface instance only (per-port copies
    are views; `collect_iface_copies`).
  - `wait (cond) stmt` lowers to `for(;;){ if (sv4_to_bool(cond)) break;
    wait_any(reads(cond)); } <body>`; wait-bearing tasks are inlined.
  - `$display` format strings are parsed at codegen time; `%t` consumes an
    argument (typically `$time`) — codegen and the runtime `llg_display`
    must agree on specifier/argument counts.
  - Expression widths: constants, parameters, signals and concat/
    replication results are all checked against `LLG_MAX_WIDTH` (1024);
    division/modulo/power operands are additionally limited to 64 bits —
    silent truncation in `sv4_concat` is a real bug, keep the checks.
  - `for_stmt` in UHDM: `vpiForInitStmt`/`vpiForIncStmt` (not vpiStmt/
    vpiElseStmt) for init/incr, `vpiCondition` = condition, `vpiStmt` = body.
  - `delay_control` values are NOT exposed via VPI in Surelog v1.87 — the
    `core::db` build recovers `#N` from the source line the delay_control
    points at (`StmtKind::DelayControl { ticks }`); timescale scaling happens
    in the codegen.
  - Generated C uses GNU statement-expressions `({ ... })` for select-LHS
    write-back (gcc/clang OK, not strict ISO C).
- **Real/shortreal v1 scope**: procedural scalar variables and real parameters;
  blocking/NBA assignment; mixed arithmetic, comparisons, logical and
  conditional expressions; casts; `if`/`while`/`for` conditions; and
  `$display` `%f`/`%e`/`%g`.  `shortreal` rounds through `float`; real-to-packed
  conversion rounds to nearest (halves away from zero) and targets at most 64
  bits.  Unsupported real contexts fail during codegen; see `src/sim/readme.md`.
- **v1 rejects**: `$dumpports` extended VCD; `$displayon`/`$displayoff`
  (warn + skip); cross-process `disable <label>;`
  (only `disable fork;` is supported); string signals/parameters; real ports,
  arrays, function/task types, continuous/combinational processes, and
  double-aware scheduling/monitoring contexts; vectors wider than 1024 bits;
  fractional delays (`#0.5`); fork/join inside a function/task body; recursive
  delay-bearing tasks; task calls inside function bodies; select/part-select
  LHS drivers on inout net members.
- **Waveforms**: `$dumpfile` selects `.vcd` or `.fst`; `$dumpvars`, `$dumpon`,
  `$dumpoff`, `$dumpall`, `$dumpflush`, and `$dumplimit` lower through IR.
  Generated waveform models use a dedicated POSIX/Win32 writer thread and a
  bounded lossless SPSC ring whose sole producer is the simulation OS thread;
  the writer owns all file/libfst state. `$dumpvars` filtering is a documented
  approximation (arguments warn, then all registered user storage is dumped).
  Normal models omit the waveform runtime and GTKWave libfst sources entirely.

---

## Rust Guidelines

- Rust edition: use the edition declared in `Cargo.toml`.
- Follow idiomatic Rust:
  - Prefer `Result` / `Option` over panics.
  - Avoid `unsafe` unless strictly required for FFI or performance.
  - If you introduce `unsafe`, **explain why it is sound**.
- Formatting: assume `rustfmt` defaults.
- Error handling: use existing error types and patterns in the crate
  (e.g. `surelog::Diag`, `elab::ElabError`); do not introduce ad-hoc error
  enums unless necessary.
- Include lifetimes only when required; prefer clarity over clever generics;
  avoid prematurely optimizing.

---

## C++ Guidelines

- C++ standard: match what is configured in `CMakeLists.txt` (do not assume
  latest).
- Style: follow existing naming and formatting conventions in
  `src/wrapper/`; avoid introducing new style paradigms.
- Safety: prefer RAII; avoid raw owning pointers; use smart pointers or
  references.
- Exceptions: do not introduce exceptions unless the existing code already
  uses them.
- Headers: minimize includes; avoid leaking implementation details in public
  headers.

---

## Python Guidelines

- Don't provide comments and docstrings unless requested.

---

## Rust ↔ C++ FFI Rules

- The FFI boundary is **C ABI only**. Never expose: C++ templates, C++
  exceptions, Rust generics, Rust panics.
- Rust side: `#[repr(C)]` for shared structs; `extern "C"` for exported
  functions.
- C++ side: wrap FFI calls in a small, well-contained translation layer
  (`src/wrapper/surelog_c_api.{h,cpp}`).
- Strings crossing the boundary are `malloc`'d by the C side and freed with
  `sl_free_string`; structs like `SL_Diag` carry their own malloc'd strings.
- Ownership rules must be **explicitly documented** at the boundary.
- If assumptions are unclear, **ask for clarification** instead of guessing.

---

## Testing

- Prefer Rust unit tests (`cargo test`) and Rust-side integration tests
  (`tests/`); new code should include tests unless clearly trivial.
- Integration tests that run Surelog must `std::env::set_current_dir` to a
  fresh temp dir (Surelog writes `slpp_all/` into the CWD) and clean up
  afterwards. Execution/elaboration tests should use
  `compile::compile_checked`; use raw `compile::compile` only when a test must
  inspect partial frontend results or diagnostics, as the LSP does.
- `tests/elaboration/run_elab_check.sh` is the elaboration regression suite
  (runs `elab_check` over the test designs; ref binding must stay 100% and
  resolved parameter values must match the expected outputs).
- `tests/sim_counter.rs` is the simulator regression suite: compiles + runs
  real designs end-to-end (codegen → `cc` → execute) and asserts exact stdout
  (hand-simulated traces, documented in the test), plus the C runtime
  self-test (`llg_rt_selftest.c`, sv4 vectors + scheduler checks).
- `tests/region_conformance.rs` pins the IEEE 1800 §4 scheduling-region
  semantics (active/inactive `#0`/NBA ordering, multi-delta settle, fork/join
  timing); a `// REGION-BUG:` case means the scheduler deviates.
- `tests/lsp_stdio.rs` is also the wire-level regression suite for the
  `llg/moduleExplorer` snapshot: it checks configured-top/source-graph roots,
  recursive child and leaf hierarchy, declaration-only fallback, typed
  content, and the absence of shadow-tree URIs.  Keep the dedicated
  `tests/fixtures/lsp/module-explorer/` manifest and source-header convention
  in sync with that test.
- `tests/property_elab.rs` runs proptest properties over `core::elab::Value`
  (X-propagation, resize/concat round-trips, casez/casex truth tables) and
  hosts the generator for the deterministic C vector table checked by
  `llg_rt_selftest.c` — keep elab.rs and the runtime semantically in sync.
- `tests/sim_*.rs` are the per-feature simulator suites (counter, function,
  fork, memory, interface, interface_body, casez, monitor, timescale, stress,
  geninit, varinit, wait, force, hier, inout): each compiles a design,
  codegens, builds the C model through `sim::build::build_model_cmake`, runs
  it and asserts the exact stdout.  Model-building suites require cmake and
  skip gracefully (`SKIP: cmake not available`) when
  `sim::build::cmake_available()` is false.
- `tests/emit_decoupling.rs` pins the pipeline shape with architectural
  greps: `sim::emit_c` consumes only IR types (no `core::db`/`ffi`/`vpi`/
  `unsafe`/`VpiHandle`), and `sim::codegen` builds an `IrModel` instead of
  emitting runtime C calls directly.
- `tests/sim_opt_differential.rs` runs designs twice — once with
  `OptConfig::default()` (all passes) and once with `OptConfig::none()` —
  building both models via `sim::build::build_model_cmake` and asserting
  byte-identical stdout.
- `tests/sim_cmake.rs` covers the build path (5 cases: library-level
  end-to-end CMake build, explicit `CmakeBuildOpts` generator backend,
  invalid-generator configure error, driver default, missing-cmake
  actionable error); skips gracefully when cmake is absent.
- `tests/model_tests.rs` covers the explorer-facing model projection: formal
  ports are not duplicated as backing signals, concrete net kinds are kept,
  and packed ranges remain owned per elaborated instance without absorbing
  unpacked dimensions.
- `tests/sim_memory_guard.rs` exercises the shared `memory_limit` safeguard
  end-to-end via `LLG_MEMORY_LIMIT_MB`.
- For FFI: prefer integration tests from the Rust side.

---

## Performance Guidance

- Do not optimize without evidence.
- If suggesting optimizations: explain the trade-off and why the change is
  expected to help.
- Avoid micro-optimizations unless requested.

---

## Documentation & Comments

- Keep comments concise and factual; explain *why*, not *what*.
- Public APIs should have doc comments.
- FFI boundaries must be documented clearly.

---

## How You Should Behave

When answering questions or suggesting code:
- Be precise and conservative.
- Do not hallucinate APIs or flags (verify against the vendored Surelog/UHDM
  headers and this document).
- If unsure, say so explicitly.
- Prefer small, incremental changes.
- Ask questions only when necessary for correctness.
