# bin/llg — Verilog/SystemVerilog Language Server

This document covers the LSP only.  Simulator discovery, configuration, and
validation are outside the scope of this LSP work.

## Purpose

tower-lsp server (stdio) for VSCode-style editors, built on the shared core:

- `main.rs` — entry (mimalloc, tokio).  A thin tower `Service` wrapper fixes
  two tower-lsp 0.20 transport gaps: a `"params": null` shutdown request
  (sent by VS Code's languageclient and the E2E harness) never reaches
  `LanguageServer::shutdown`, so the wrapper triggers the same deterministic
  shadow-base cleanup; and the `exit` notification terminates the process
  (spec exit code: 0 after `shutdown`) instead of hanging until stdin EOF.
- `logging.rs` — low-overhead, configurable stderr/file logger.  `LLG_LOG`
  selects the level and `LLG_LOG_FILE` selects an append-only destination;
  stdout is never used for logs.  Also provides `LifecycleSpan`
  request/notification/root-job/phase records with process-unique
  `id`/`parent_id` correlation, plus an optional pluggable physical-memory
  sampler.
- `config.rs` — typed `llg.toml` v1 parsing (`LlgConfig`, `SourcesConfig`,
  `CompileConfig`), config loading with last-valid retention, safe defaults,
  source/include directory derivation, `CompileOpts` conversion and the
  `.v`/`.sv` compilation-unit predicate.  `compile.defines` (list of `NAME`
  / `NAME=VALUE`) and `[compile.param_overrides]` (`NAME` → string or
  integer value) become validated Surelog `-D`/`-P` arguments: integers are
  normalized to decimal strings, keys must be SystemVerilog identifiers.
  Structural errors reject the whole config atomically; a malformed define
  entry or an invalid/empty override key/value is dropped with a warning
  published against the TOML URI instead (first entry per duplicate define
  name wins).
- `lsp.rs` — `Backend`: per-root debounced recompile (~300 ms trailing edge)
  with latest-wins coalescing (see `scheduler.rs`: one armed timer per root;
  triggers arriving while a job runs never queue another job — they mark the
  root dirty and completion arms exactly ONE follow-up over the newest
  inputs; bursts therefore cost at most two runs and continuous editing
  converges without supersession churn), project-wide diagnostics publishing
  (every analyzed file gets a publication after each commit, open or closed;
  identical per-URI payloads are suppressed via per-root digests), handlers
  for semantic tokens, hover,
  goto-definition, references, document symbols, workspace symbols,
  completion, prepareRename/rename.  Unsaved buffers are staged into a private per-process shadow
  tree under the OS temp dir (`ShadowPaths`: deterministic real↔shadow
  bijection, `llg-{pid}-{rand}`) so diagnostics and features track the buffer
  contents without a save.  Per-root `llg.toml` config loads, dynamic
  watchers, and multi-root aggregation live here too.
- `scheduler.rs` — pure idle→pending(timer)→running→dirty state machine
  behind the analysis scheduling above; unit-tested without tokio.
- `request_cache.rs` — bounded thread-safe LRU memoization for read-only
  requests (`textDocument/definition`, `hover`, `references`, and the
  open-buffer isolated `semanticTokens/full` parse).  Navigation results are
  keyed on `(request kind + parameters, uri, position, analysis epoch)`;
  every commit that replaces or clears a root's `last_good` snapshot stamps
  it with a fresh process-global epoch (`RootState::analysis_epoch`,
  assigned in `commit_job`), so ANY input change that flows through an
  analysis commit — buffer edits, watched-file saves, config hot reload incl.
  `[compile]` defines/param_overrides — invalidates structurally (no TTLs);
  entries keyed by superseded epochs simply age out of the LRU (128 entries
  per store; token payloads 8 — values never pin a retired `Analysis`).
  Open-buffer token streams are pure functions of `(buffer text, -D
  defines)`; both are hashed into their key, so edits and defines hot
  reloads miss while identical repeats short-circuit the stage+parse
  pipeline entirely.  Identical full-text `didChange`s carry no input change
  and are NOT rescheduled (they would only churn epochs).  Cache access is
  mutex-guarded and stays outside the serialized lifecycle queue exactly
  like the handlers themselves; `get` holds its lock across the hit lookup,
  recency refresh and stats update, and a replaced entry releases the stale
  value immediately (no retained `Analysis` is pinned).  Presentation steps
  that depend on mutable
  backend state (shadow→real URI mapping, shared-file hover annotation)
  run AFTER the memoized value is fetched.  Aggregate hit/miss counters are
  surfaced on every `llg/dumpTokens` payload as a `# request-cache:` line
  inserted BEFORE the trailing `# analysis:` summary (which must stay last);
  per-request serving is logged at trace level with elapsed µs.
- Custom request `llg/dumpTokens` (registered via the
  `LspService::build(...).custom_method(...)` builder in `main.rs`):
  `Backend::dump_tokens` serves the `--dump-tokens` rows of one document plus
  the trailing `# analysis:` summary; failures answer a single `# error:` line.
- Custom request `llg/moduleExplorer` (registered in the same builder):
  `Backend::module_explorer` reads only committed per-root `Analysis` models
  and returns a deterministic multi-root recursive instance snapshot, module
  definitions and typed contents.  Elaborated generate scopes retain their
  direct instance children; syntax-fallback analyses still expose parsed
  module definitions.  A committed model replacement or clear emits the
  empty-params `llg/moduleExplorerChanged` notification so clients refetch;
  diagnostics-only/fatal commits that retain the served model do not emit it.
- `features.rs` — pure feature logic over `Analysis`
  (`{ diagnostics, outcome, model, tokens, index, ref_bindings, lint }`);
  `analyze()` runs
  the whole Surelog pipeline + db build + token collection + lint pass in one
  blocking call (`analyze_with_config(opts, &LintConfig)` is the configurable
  variant).  When a syntax error makes Surelog skip compile/UHDM,
  `parse_tree_feature_parts` falls back to the surviving parse tree: parse
  tokens plus a modules-only model, with the collector's recorded
  declaration positions threaded into `SymbolIndex::from_parts` so ports,
  nets/regs and parameters are indexed as DECLs and same-file previously
  declared uses as REFs (assignment LHS excluded via
  `ancestor_is_assignment_lvalue`), so `has_feature_data()` (the
  feature-serving gate: non-Fatal + any servable data) holds and
  declaration-level navigation serves until the project compiles.
   Named-connection DEFINITION semantics (both modes): the two sides
   of a named connection resolve DIFFERENTLY — the port label `.clk` jumps to
   the matching PORT declaration inside the CHILD module and the parameter
   override label `.W` (in `child #(.W(4)) u0 (...)`) jumps to the child's
   PARAMETER declaration, while the connected ACTUAL `wa` in `.clk(wa)` and
   the override RHS in `.W(rhs)` stay on their OWN declarations in the
   instantiating (parent) scope (enclosing-module-span containment of the
   instantiation line, tie-break nearest above; global nearest-above
   fallback; no candidate ⇒ the actual stays unbound).  UHDM mode folds
   resolved labels (`via_label`) plus their paired actuals
   (`via_connection`, positions and identifier text from the parse-tree
   pairing in `scan_named_port_connections`, which covers named PORT
   connections AND named PARAMETER overrides) into `ref_bindings`; override
   RHS references additionally gain elaboration-backed targets because the
   token walk visits the `vpiParamAssign` RHS hanging off each instance.
   The parse-fallback mode resolves labels against the recorded
   declarations (label text ↔ child module's port/parameter name, exact
   match) and actuals/RHS against the recorded parent-scope declarations,
   and retypes label tokens to plain reference types so they stay indexed
   and visible to the dump.  Unresolvable connections are skipped silently;
   an unresolvable PARAMETER override label yields NO definition — its
   namespace is the instantiated module, so every name-based fallback would
   be wrong by construction (a same-named localparam of the instantiating
   scope must never capture it).
   `ref_bindings` (reusing `core::tokens::RefBindings`/`DeclTarget`) maps the
    0-based position of every emitted reference token to its elaboration-bound
    declaration (`vpiActual`), unioned with named-connection bindings
    (UHDM targets win on collisions; the ACTUAL-side fold only fills
    still-unbound positions so an existing explicit binding always wins);
    `definition_at` serves the bound target at exact key positions before the
    index/fallback logic.
   SHADOWING (inner-scope declarations shadowing same-named outer ones):
   the VPI token walk reaches EVERY inner-scope interior so refs there carry
   elaboration bindings instead of falling through to the module-granularity
   name fallback — function/task BODIES are descended through the
   task/function handle's own `vpiStmt` (process iteration never covers
   them), generate-block interiors are reached ONLY off the ELABORATED
   instance tree (`uhdmtopModules` → `vpiGenScopeArray` → `vpiGenScope`;
   definition-level `uhdmallModules` handles expose neither internal scopes
   nor gen-scope arrays), and named begin/fork blocks are emitted when the
   statement-tree descent hits `vpiNamedBegin`/`vpiNamedFork`/`vpiBegin`/
   `vpiFork` (VPI-MAPPED types — Surelog does not report its `named_begin`
   UHDM discriminant there).  A scope-identity set keyed by
   `(vpiType, vpiFullName, line, col)` keeps doubly-reachable scopes single-
    visited; raw handle addresses are unusable because UHDM recycles freed
    handle addresses across walks.  Block/genblk/function-local DECLARATIONS
    therefore get their UHDM view next to the parse view, which flips them to
    DECL under the multi-view heuristic — goto-definition ON a shadowing
    declaration resolves to itself.  `hover_at` consults `ref_bindings`
    FIRST (exact key, then cursor-normalized start column, mirroring
    `definition_at`) and renders the bound target's detail: the indexed
    declaration entry at the target position, else the position-accurate
    snippet from `decl_details`.  Parameter/localparam hovers — at the
    DECLARATION site and at binding-precise REFERENCE sites (override labels
    included) — additionally append an elaborated-value line
    (`value = 32'sd8`) rendered ONLY from the committed model: the target's
    enclosing module span selects its instance family and every same-named
    direct/generate-scope parameter across those clones must agree on one
    distinct constant, so `-P` overrides show wherever they are committed;
    divergent per-instance overrides, unresolved values, and positions
    outside any module (which fall back to same-file packages) omit the line
    instead of guessing, and NOTHING is parsed or elaborated in the request
    path.  An identical inline value tail on the model detail
    (`parameter W: int = …`) normalizes into that line so the value renders
    exactly once.
    MACRO hovers (`core::macros::MacroTable`, built once per analysis commit
    and attached to `Analysis`): Surelog carves `` `define `` directives out
    of every frontend artifact reachable over FFI (UHDM + parse-tree
    FileContents; the pp-level parse is internal), so macro usage/definition
    positions never carry indexed symbols and hover consults a dedicated
    table FIRST in `hover_at`.  Value sourcing — config `[compile] defines`
    seed EVERY analyzed file as command-line defines (authoritative base,
    hot-reloaded through config commits), then ONE conservative scan per
    compiled file resolves in-source directives positionally: `` `define ``
    (simple + function-like, backslash continuations joined for display),
    `` `undef ``, `` `undefineall ``, and `` `ifdef``/`` `ifndef``/
    `` `elsif``/`` `else``/`` `endif`` evaluated against the evolving
    in-file table; comments, string literals, and dead conditional branches
    are skipped; LAST DEFINITION WINS per the LRM, so an in-source redefine
    overrides the config value only from its definition point onward and
    `` `undef `` removes even a config-seeded name.  A usage renders against
    the table state AT ITS OWN POSITION; usages inside a `` `define `` body
    resolve at the definition point.  Documented approximations (can lose a
    value, never invent one): includes are not followed (header-only macros
    read as undefined) and definitions do not leak across files.  Rendering
    follows the parameter house style: `macro WIDTH = 8`,
    `macro MAX(a, b) = …`, or `macro ENABLE` (body-less) inside the standard
    SystemVerilog fence plus `defined at <file>:<line>` for source origins
    (shadow paths display as their real project paths); an UNDEFINED usage
    renders an explicit "`X` is not defined under the current configuration"
    message naming the effective config file (attached at commit time via
    `attach_macro_config_note`) instead of any value.  The table build reads
    the exact compiled sources once per commit (shadow paths carry open-
    buffer text); requests only look positions up, so warm repeats stay
    memoized request-cache hits.
  `references_at_with_options` is binding-
   aware: the query's own binding anchors the reference set to exactly that
   declaration (`heads`); occurrences whose captured binding targets a
   DIFFERENT declaration are dropped from the v1 name-based pool, precisely-
   bound positions outside the pool are added, and declaration sites recorded
   by `decl_details` behave as declarations even where classification left
   them REF-shaped.  Rename inherits all of this verbatim (it computes its
   occurrence set with `references_at_with_options`).
   `decl_details` (`core::tokens::DeclDetails`) maps `(file, line1, col1)` of
   every declared object to a rendered snippet (`logic [3:0] val`,
   `input logic [1:0] sel`, …) captured during the same VPI walk straight
   from each object's typespec (mirroring `core::model::TypeInfo::render()`;
   first writer wins per position so a port view beats its underlying
   variable view).  It replaces the NAME-based model detail on indexed
   Port/Net/Var declarations (`Analysis::with_decl_details`, which also
   refreshes the index lookup maps) — under shadowing the model lookup would
   describe the outer same-named object — while parameters keep their value-
   rich model detail.
  `SymbolIndex` provides cross-file declarations/
  references and merges per-root indexes for workspace symbols.
  `shadow_path`/`real_path` are the pure mirror helpers behind `ShadowPaths`.
- `workspace.rs` — `llg.toml`-driven discovery filters, `.v`/`.sv`/`.vh`/
  `.svh` classification, longest-root ownership, and per-root descriptor
  state.  Classes have full model data: class declarations (with
  methods/fields) in hover, document symbols (class symbol with method/field
  children), and completion after a `Class::` prefix (mirroring the `pkg::`
  branch).
- `semantic_tokens.rs` — token legend + delta encoding.  Parse-tree scope
  keywords (`module` via `paModule_keyword`, `endmodule`, …) are classified
  as `keyword`; the leading declaration keyword is tokenized exactly like
  its closing counterpart.
  The legend carries the custom `connectionLabel` MODIFIER (bit 10) for
  module-instantiation connection labels: port labels (`.clk`) keep their
  historical `function` base type, parameter-override labels (`.W`) their
  `property` + `readonly` base, and the modifier — not a new type — is what
  themes style (`*.connectionLabel`) to tell labels from connected signals
  (plain `variable`).  Labels reach `encode` as the parse-tree classifier's
  `TOKEN_PORT_CONN_LABEL` / `TOKEN_PARAM_CONN_LABEL` synthetic types
  (`core::tokens::classify_identifier_ancestor`), so the marking holds in
  BOTH serving paths below; `dumpTokens` rows render it naturally in
  `sym=…/connectionLabel`.
  `textDocument/semanticTokens/full` parses an open document's exact current
  buffer as one request-local staged source via Surelog `-parseonly`
  (`-nocache -nobuiltin`), so project units and include contents do not enter
  that token stream.  Unopened documents continue to use the owner root's
  cached project analysis. Frontend diagnostics retain the current buffer's
  partial/supplemented stream; only staging/session/task failures fall back to
  the cache, while a successful empty parse is authoritative. The request
  does not change diagnostics or navigation snapshots.
  Open-buffer misses are single-flighted per `(uri, text, defines)` with a
  bounded in-flight table; a captured revision that is no longer the
  document's current snapshot is rejected before cache serving, flight
  admission, and any parse-only Surelog work (re-checked inside the blocking
  parse), so it never occupies a flight slot or starts an obsolete parse and
  falls back to the committed project tokens with a stale outcome.

## Requirements

- This bin is gated behind the crate's `lsp` feature (default-on): the
  `[[bin]]` entry declares `required-features = ["lsp"]`, so building it pulls
  in `tower-lsp`/`tokio`/`dashmap`/`toml`/`serde`; the lib itself never
  depends on them.
- All Surelog work runs in `spawn_blocking` behind a process-wide mutex;
  `SurelogSession` is not `Send` — compile + model + tokens happen in one
  blocking closure and the session is dropped there.
- The core token collectors are silent.  The LSP retains only the
  process-global Surelog analysis mutex required by Surelog's global state;
  lifecycle logging stays in the LSP binary and never uses stdout.  Backend
  state locks must not be held during filesystem discovery, shadow cleanup,
  shadow staging, or config I/O.
- Lint findings from `core::lint` are merged into the published diagnostics
  with `source: "llg-lint"` (severity Error → `ERROR`, rule id as the code).
- No `unsafe`; no `#[path]` includes (use `llg::core` / `llg::ffi`).
- The server may install the shared process-memory guard
  (`llg::memory_limit::install_with_logger`), wiring its sampler into
  lifecycle logging; see `docs/lsp_safeguards.md`.

## Logging

- `LLG_LOG` selects the stderr/file log level: `off`, `error`, `warn`,
  `info`, `debug`, or `trace` (default: `warn`; invalid values use the
  default).
- `LLG_LOG_FILE` appends logs to the given path.  An empty, invalid, or
  unwritable path falls back to stderr.  Logs never use stdout, which is
  reserved for LSP framing.
- At `info`/`debug`, `LifecycleSpan` records correlate a request/notification
  to its coalesced root job and the analysis phases it started via process-
  unique `id`/`parent_id`; each record carries root, generation, file count,
  outcome, elapsed time, result cardinality, and a physical-memory sample
  when a sampler is installed.

## Workspace discovery and lifecycle

- Each client-supplied workspace folder is an independent analysis root whose
  configuration comes from its effective `llg.toml` (v1 schema).  The client
  passes only config *locations* in `initializationOptions`
  (`{ "llg": { "protocolVersion": 1, "configFiles": [
    { "workspaceUri": ..., "path": ... } ] } }`); without an override the
  server loads `<root>/llg.toml`.  Missing configs use safe defaults: the
  root as the sole source directory, recursive `.v`/`.sv`, excludes
  `slpp_all/**` (Surelog preprocessed-output defense), `.git/**` and
  `target/**`.
- Discovery is driven by each root's config, not by client globs.  Only
  `.v`/`.sv` files become compilation units; `.vh`/`.svh` and
  arbitrary-extension files enter analysis only through include resolution.
  Every configured source directory is automatically an include-search
  directory; `compile.include_dirs` adds search-only directories and may be
  external.  `sources.include`/`exclude` are root-relative globs evaluated
  against each source directory; exclude wins.
- Roots are analyzed independently.  Discovery filters, lint configuration,
  shadow paths, and compile/navigation snapshots belong to their root; adding
  or removing one workspace folder does not replace the other roots.
  Longest-root ownership is structural over each root's configured include
  directories (the deepest matching include dir within a root wins; equal
  depths tie-break deterministically to the lowest normalized root path): a
  discovered file
  whose longest-prefix owner is a different (e.g. nested) root is not
  compiled by the outer root, even when the outer root's discovery filters
  would include it.
- Shared files (tracked by multiple roots via discovery or resolved include
  deps) follow an **owner-wins** model: only the single longest-prefix owner
  root analyzes the file; other roots consume the owner's results.  This is a
  deliberate v1 deviation from the migration plan's "aggregate per-root
  re-analysis" goal — full per-root re-analysis of shared files is deferred.
  The minimal aggregation slice that v1 does implement:
  - Diagnostics for every multi-root-tracked file are published as the UNION
    of all tracking roots' diagnostics for that file (usually just the
    owner's), even when the file is not open.  Exact duplicates (same
    URI/range/severity/code/message) appear once; when distinct findings
    share a location, non-owner-root copies carry a `[<root-name>]` message
    suffix identifying which root's configuration produced them.  Shared
    paths are owned EXCLUSIVELY by this slice: the per-root publication map
    excludes them so the labeled union can never be swallowed by a same-
    commit primary publication.
  - Semantic tokens: owner root wins.  In v1 only the owner holds token data
    for a shared file, so conflicting classifications cannot be compared; the
    ownership decision is logged at trace level instead.
  - Hover in a shared file annotates configuration-dependent sections
    (parameter values / define-derived text) with the owner root name so
    users know which config produced them.
- Diagnostics are published PROJECT-WIDE: after every commit each root
  publishes its latest diagnostics for every analyzed file, open or closed —
  clients render Problems-panel entries for closed files without them being
  opened first.  Compilation units keep an entry even when clean (an empty
  list clears stale client-side errors), and the publication map is unioned
  with the previous key set so URIs that left the analysis are cleared.
  Identical per-URI payloads are not re-sent: each root keeps a digest of the
  last payload it published and skips unchanged ones (digests are evicted for
  URIs the root no longer publishes).  `didClose` therefore does NOT clear a
  document's diagnostics; the staged buffer leaves the shadow tree and the
  rescheduled post-close recompile refreshes that URI from on-disk state.
- The server performs an initial scan, consumes standard
  `workspace/didChangeWatchedFiles` notifications, and uses LSP dynamic
  registration for those notifications when the client supports it.  Watchers
  cover each effective `llg.toml`, `.v`/`.sv` units under every configured
  source directory, and the exact resolved include dependencies (any
  extension).  Watchers are re-registered once per feature-data-bearing
  analysis commit (see `Analysis::has_feature_data`) — newly resolved
  include deps only become known after a compile, and roots that never reach
  strict validity must still register them — reusing the same registration
  id and coalescing updates whose
  watcher set is unchanged.  Watched events are routed through each root's
  tracked include-dependency set FIRST (via a dep-path → dependent-roots
  reverse index rebuilt at commit time), so a change to a resolved dep of any
  extension schedules EVERY root that depends on it, in addition to the usual
  source/config classification.  Added or removed workspace folders trigger
  the corresponding root scan, ownership re-evaluation, and analysis update.
- Include authorization follows literal `` `include "..." `` targets
  transitively.  An include is permitted when its resolved target lies under
  a configured source or include directory of the owning root (which may be
  outside the workspace); targets that escape all configured directories are
  rejected with a preflight diagnostic.  There is no blanket
  "cannot cross workspace root" rule.
- Read-only file access: closed files are read directly from disk; open
  buffers are tracked via `didOpen`/full-text `didChange`/`didClose` and win
  over disk text.  Unsaved buffers and resolved include dependencies are
  staged into the private per-process shadow tree
  (`llg-{pid}-{rand}` under the OS temp dir); shadow include directories
  precede the real include directories in `CompileOpts` so unsaved headers
  win.  Every analysis also parks the process CWD inside
  `<shadow base>/work/analyze/` for the duration of the blocking compile:
  Surelog writes `slpp_all/`, logs and caches into its (process-wide,
  first-session-frozen) working directory, so this keeps those artifacts out
  of project/external trees entirely; the directory's contents are reset
  between jobs and removed on shutdown.  The server never creates, modifies,
  chmods, or deletes project, config, or external files; temporary state is
  cleaned on shutdown and stale-root removal.
- Config changes are parsed atomically: a malformed or invalid `llg.toml`
  publishes a diagnostic against the TOML URI, retains the last-valid config
  on reload, and safe defaults apply until a valid config has ever loaded.
  Entry-level issues inside `compile.defines`/`compile.param_overrides` are
  not structural: they are dropped with a `llg-config` warning against the
  TOML URI while the rest of the file loads.  Reloads rescan source
  directories when discovery changes, rebuild compile options when
  top/defines/param overrides/include dirs change (a reload compares the
  whole parsed config, so any of those edits reschedules the root's
   analysis), and republish diagnostics when lint rules change — all without
   restarting the server.  Every state-changing reload ALSO pushes the
   `llg/configChanged` notification (empty params) so clients refresh
   config-derived views they pull on demand — inactive-region dimming above
   all, which would otherwise stay stale on every open buffer until the next
   buffer event; a reload that parses to an identical config notifies
   nothing.  NO restart is ever required for `llg.toml`
   CONTENT changes: every effective config file is covered by a dynamic
   watched-file registration and hot-reloads in place.  A server restart is
   triggered only client-side, by VS Code settings (`hdlsupport.llg.path`,
   `.configPath`, `.logLevel`) changing — see `restartPolicy.ts` on the
   extension side.
- The latest diagnostics are published even when a compile fails.  Feature
  serving is gated by `Analysis::has_feature_data`: every analysis that is
  not Fatal AND carries servable data (index declarations, model modules, or
  tokens) replaces the per-root navigation snapshot — Parse/Compile outcomes
  serve best-effort over the surviving set (Surelog still elaborates the
  files that parsed when only non-syntax errors are present).  A project with
  a SYNTAX error — Surelog skips its whole compile/UHDM stage then — serves
  DECLARATION-LEVEL features through the parse-tree fallback
  (`features::parse_tree_feature_parts`): tokens come from
  `core::tokens::collect_parse_tokens` (with named-port-connection label
  tokens retyped to plain reference types, which would otherwise be
  misindexed as function/task declarations); the
  recorded parse-declaration positions seed the index so ports/signals/
  params are DECL entries and previously declared same-file uses are REF
  entries; the model is modules-only (name + declaration position per parsed
  module header); instances, elaboration-derived values and cross-module
  use-site resolution stay unavailable until the project compiles.  Fatal
  analyses (no/poisoned UHDM, include-isolation preflight aborts) and analyses without
  any servable data (e.g. an empty project) never replace the snapshot;
  before the first feature-data-bearing commit a root has nothing to serve.
  Diagnostics always publish regardless of the gate.

## Stdio integration-test acceptance

`tests/lsp_stdio.rs` is the process-level acceptance suite.  It launches the
`llg` binary and speaks only framed LSP JSON-RPC over stdio, covering default
and overridden config files, config reload without restart, `.v`/`.sv`
discovery with `.vh`/`.svh` include-only behavior, include/exclude precedence,
independent multi-root scans, longest-root ownership transfer on workspace
folder add/remove, dynamic watched-file registration (re-registered after
feature-data-bearing analyses), config/source/resolved-include watch events,
`llg/configChanged` pushes on state-changing config reloads (and their
absence for identical reloads),
arbitrary-
extension include-dep changes that flip published output, dep changes that
refresh every dependent root, shared-file aggregation with `[root-name]`
labels and once-only identical findings, per-root lint configuration from
`llg.toml`, unsaved source and header buffers, project-wide diagnostics
(errors published for files that were never opened, then refreshed when such
a file is fixed on disk through a watched-file event), include authorization
(allowed configured-dir includes and rejected escapes), feature serving from
an error project whose analysis still carries UHDM data (non-syntax errors),
feature serving at declaration level from a syntax-broken project via the
parse-tree fallback (documentSymbol/workspace-symbol/hover on modules while
an unterminated module keeps the root Parse), feature-less responses while
only a Fatal analysis exists, analysis upgrades after a watched-file fix
turns a parse-failed project valid, last-good
navigation during failed compiles, binding-precise goto-definition (each
instance-scope net use resolves to exactly its own module's declaration,
named port connections resolve to the child module's port declaration
from BOTH the `.clk` label and the connected signal, and named parameter
overrides resolve the `.W` label to the child module's parameter while the
override RHS stays parent-scope — single- and
multi-line instantiations, plus the same navigation through a
syntax-broken sibling whose parse-fallback analysis binds port labels,
parameter override labels and actuals via the dump `bind=` oracle),
read-only shadow staging, and absence of
Surelog artifacts (`slpp_all/`, logs) inside a workspace used as the server
CWD.  Its
fixtures must continue to use `tests/fixtures/lsp/test.json` with schema
`llg.lsp.fixture/v1`; every fixture source file must retain the
`// llg-lsp-fixture:` source header and every root ships an effective
`llg.toml`.

## Lint configuration

Lint configuration is part of each root's `llg.toml`:

```toml
[lint]
enabled = true

[lint.rules.unused-signal]
enabled = false

[lint.rules.width-mismatch]
severity = "error"
```

- `lint.enabled` is the global switch: `false` disables every known rule
  (per-rule entries can re-enable individual rules).
- Each `lint.rules.<id>` entry accepts `enabled` (bool) and `severity`
  (`"error"` | `"warning"` | `"info"`).
- The config is loaded once per root at startup and on every watched
  `llg.toml` change; malformed configs retain the last-valid lint policy and
  publish a `llg-config` diagnostic against the TOML URI.
- `llg-lint.toml` is not read by the LSP.  The simulator CLI
  (`llg_sim --lint-config`) still uses its own `llg-lint.toml` reader in
  `core::lint`.

## Rename

`textDocument/prepareRename` + `textDocument/rename` live in `rename.rs`
(pure functions over `Analysis`, registered as standard tower-lsp methods;
the capability is advertised as
`rename_provider: Some(OneOf::Right(RenameOptions { prepare_provider: Some(true), .. }))`,
so vscode-languageclient registers the providers client-side).  Semantics:

- `prepare_rename` answers the identifier range + current name ONLY for
  positions that hit an indexed symbol entry (`SymbolIndex::entry_at`);
  keywords and unindexed tokens are not renamable (null response).
  Instance-name declarations are excluded on purpose: the index resolves an
  instance to its module DEFINITION, so a rename there would retarget the
  whole type family instead of the instance name.
- The occurrence set is computed by exactly the same machinery as
  find-references: `features::references_at_with_options(...,
  include_declaration = true)`.  Rename and find-references can therefore
  never disagree; each edit replaces ONLY the identifier span of one indexed
  occurrence, grouped per URI into a plain `changes` WorkspaceEdit.  Module
  renames cover instantiation-site type-name references because those are
  indexed module-kind refs.
- The new name must be a plain identifier (`[A-Za-z_][A-Za-z0-9_$]*`);
  anything else is rejected with an invalidParams error (no escaped-
  identifier support).  The server never writes files — it returns the edit.

Known limitation inherited from the symbol index: resolution away from
binding-precise positions is still name-based + scope-aware, so a reference
position that carries NO captured binding can be misattributed when same-named
declarations shadow each other across scopes; every position the UHDM walk
captured a binding for is attributed exactly (see the shadowing paragraph in
the `features.rs` section).

## Interactions

- `llg::core::compile` (pipeline), `llg::core::db` (owned database),
  `llg::core::model` (`DesignModel::from_db`), `llg::core::tokens`
  (semantic token collection).

## Known v1 limitations

Unsaved buffers compile via the private per-process shadow tree
(`llg-{pid}-{rand}` under the OS temp dir, see `features::shadow_path` /
`lsp::ShadowPaths`): literal transitive includes resolve from staged disk
files and open buffers, with open text taking precedence.  Literal include
preflight cannot resolve macro-generated or dynamic include paths.  If
staging fails (e.g. an unwritable shadow dir) the server falls back to the
on-disk files.
Definitions are binding-precise at captured reference positions
(`ref_bindings`: UHDM `vpiActual` targets plus named-connection folds —
port-label side `via=label` to the child module's port, parameter-override
label side likewise `via=label` to the child module's parameter,
connected-signal/override-RHS side
`via_connection` to the actual's own declaration in the instantiating scope,
the latter never overriding an existing explicit binding); the captured
positions now cover every inner-scope interior (function/task bodies, named
begin/fork blocks, generate blocks) as well as process/module-level uses, so
hover, goto-definition and find-references resolve shadowed declarations
exactly at those positions.  Parameter value lines inherit the binding map's
granularity: instance context beyond "all clones of the owning module" is not
distinguishable from a source position (bindings key shared positions), so a
module instantiated several times with DIFFERENT overrides omits the line at
its declarations/uses, and generate-pruned branch-local declarations reached
only through the name-based fallback can inherit the surviving branch's
same-named constant.  Away from those
positions definitions and references remain
name-based, scope-aware approximations; named port connections resolve the
LABEL to the child module's port and the ACTUAL to the parent-scope
declaration (enclosing-module-span containment of the instantiation line,
nearest-above tie-break/fallback), including multi-line instantiations;
named parameter overrides resolve the label to the child module's parameter
and the RHS to the parent-scope declaration, with unresolvable override
labels yielding no definition at all; in
parse-fallback mode the same semantics are resolved against the recorded
parse declarations (exact name match; positional matching for unnamed/`.*`
connections is out of scope); classes have model data — class
definitions with methods and fields surface in hover, document symbols and
`Class::` completion — but there is no class INSTANCE support in the
simulator and no member-expression resolution like `obj.count` (out of scope).
Functions/tasks have full model data (signature hover, document symbols,
completion).  Package parameters and enum constants are modeled and resolve
from the declaration side (hover, goto-definition, and completion after a
`pkg::` prefix).  Package/class-qualified enum uses, imported enum members,
and bare enum members with one visible declaration are bound from the parse
tree at their exact member positions; ambiguous bare/qualified uses
intentionally remain unresolved rather than falling through to name-based
navigation.
