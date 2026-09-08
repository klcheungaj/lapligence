# Verilog/SystemVerilog language server

This guide covers the LSP; simulator discovery/configuration is separate.
The tower-lsp stdio binary provides semantic tokens, hover, definition,
references, document/workspace symbols, completion, prepareRename/rename,
diagnostics and custom read-only views.

## Ownership map and invariants

- [lsp/AGENTS.md](lsp/AGENTS.md) owns backend scheduling, config/workspace
  lifecycle, diagnostics, watchers, bounded input admission and shadow staging.
  Read it when editing `lsp.rs`, `config.rs`, `workspace.rs`, `scheduler.rs`
  or backend children, including request paths that read source text.
- [features/AGENTS.md](features/AGENTS.md) owns analysis, bindings, index,
  fallback navigation, parameter/macro hover and known feature limitations.
  Read it when editing `features.rs`, rename or other feature consumers.
- Shared memory policy and safeguard review rules are in
  [../../AGENTS.md](../../AGENTS.md); focused tests and acceptance contracts
  are in [../../../tests/AGENTS.md](../../../tests/AGENTS.md).
- `llg_ls` is default-on feature `lsp` gated (`required-features = ["lsp"]`);
  tower-lsp/tokio/dashmap/toml/serde stay in the bin, not the library.
  Use `llg::core`/`llg::ffi` imports; no `unsafe` or `#[path]` includes.
- Surelog work is serialized in `spawn_blocking`; keep the non-Send session
  and all capture in one closure and transfer only owned `Analysis`.
- Core collectors are silent. Stdout is exclusively framed JSON-RPC; logs use
  stderr or a file. Convert Surelog 1-based diagnostics to 0-based wire
  positions exactly once in `features::lsp_diagnostics`.
- `main.rs` installs mimalloc/tokio and registers standard/custom methods with
  `LspService::build(...).custom_method(...)`. The transport `Service` wrapper
  closes tower-lsp 0.20 gaps: `"params": null` shutdown performs the same
  deterministic shadow-base cleanup even when it misses
  `LanguageServer::shutdown`; `exit` terminates without waiting for stdin EOF
  (code 0 after shutdown).
- Startup may use `llg::memory_limit::install_with_logger`, connecting its
  physical-memory sampler to lifecycle logging.
- With the opt-in Cargo feature `slang`, setting
  `LLG_SLANG_DIAGNOSTICS=1` runs the migration frontend over the exact bounded
  in-memory inputs admitted for each root. Its compiler and analysis findings
  are published alongside the current Surelog/Rust findings with distinct
  sources and namespaced codes. The environment variable defaults off so an
  all-feature build retains ordinary LSP behavior. Slang findings never change
  the Surelog-owned semantic outcome or promote a diagnostic-only result into
  a servable snapshot.

## Entry and transport boundaries

- A session initialized without workspace folders adopts an opened `.v` or
  `.sv` file through the normal root scheduler, using its nearest `llg.toml`
  ancestor or its parent directory. `.vh`/`.svh` files remain include-only.
- Published Surelog syntax diagnostics use
  `core::diagnostics::user_message`; analysis and debug logs retain the
  original diagnostic text. Presentation reads owned/staged data, not files.
- Project trees and other filesystem inputs are treated as untrusted and
  read-only. Compiler side effects stay under the private process shadow
  directory; stdout remains reserved for framed JSON-RPC.

## Request cache (`request_cache.rs`)

Bounded thread-safe LRU stores memoize definition, hover, references and
isolated open-buffer semantic tokens. Navigation keys include request kind/
parameters, URI, position and `RootState::analysis_epoch`. `commit_job` stamps
a fresh process-global epoch whenever `last_good` is replaced/cleared, so
committed edits, watched saves and config/define/override reloads invalidate
without TTLs. Old epochs age out (128 entries per store; 8 token payloads);
values never pin retired `Analysis` snapshots.

Token keys hash buffer text and `-D` defines: identical repeats bypass
staging/parsing, while edits/define reloads miss. Cache locks stay outside the
serialized lifecycle queue; `get` holds its mutex through lookup, recency and
stats updates. Replacement releases the old value immediately. Apply mutable
presentation mapping (shadow→real URI, shared-file hover labels) after lookup.
Trace records include per-request elapsed µs; `dumpTokens` exposes aggregate
hit/miss counts as described below.

## Semantic tokens (`semantic_tokens.rs`)

The legend/delta encoder classifies opening and closing scope keywords alike
(`module` via `paModule_keyword`, `endmodule`, etc.). `connectionLabel` is
modifier bit 10: port labels `.clk` retain base `function`; parameter labels
`.W` retain `property` + `readonly`; connected signals remain `variable`.
Themes use `*.connectionLabel`, and dump rows show `sym=…/connectionLabel`.
`core::tokens::classify_identifier_ancestor` produces `TOKEN_PORT_CONN_LABEL`
and `TOKEN_PARAM_CONN_LABEL`, so this holds for both serving paths.

Parse-backed genvar names are `variable` (with the declaration modifier at
declarations); actual `genvar` spans are `keyword`. Both serving paths preserve
UTF-16 positions for these tokens, using already-admitted current text for the
isolated path. See the [source-binding contract](features/AGENTS.md#genvar-source-bindings).

- Open `textDocument/semanticTokens/full` parses the current buffer as one
  request-local staged source with Surelog `-parseonly -nocache -nobuiltin`.
  Mask inactive branches under effective defines and non-lexical compiler
  directive lines/continuations with position-preserving spaces; otherwise
  valid directives such as `` `include`` become false syntax errors. No
  project units or include content enters this stream.
- Unopened documents use the owner's committed project tokens. The isolated
  path does not change diagnostics or navigation snapshots. Any syntax
  diagnostic for current text yields authoritative empty tokens, avoiding
  unstable partial highlighting. Valid unsaved text uses its current parse;
  successful empty parses are authoritative. Staging/session/task failures
  alone may fall back to committed tokens after admission.
- Before cache-key construction, flight admission, staging or parsing, reject
  current text exceeding `analysis.max_file_bytes`: serve committed project
  tokens (or empty), outcome `too-large`, with no flight slot consumed.
- Single-flight identical misses on `(uri, text, defines)` in a bounded table.
  Reject stale captured revisions before cache serving, flight admission or
  parsing, re-checking inside the blocking parse: no stale slot/obsolete
  Surelog work, outcome `stale` with committed fallback. When saturated,
  refuse new keys immediately and use committed project/cache tokens (or
  empty); bursts must not create an unbounded Surelog queue.

## Other bounded source requests

`llg/inactiveRanges` and completion check `analysis.max_file_bytes` before
cloning open text or reading/scanning closed text. Over-limit input returns
empty/no-data and a bounded diagnostic log. Closed reads use the shared
max-plus-one snapshot helper, never unbounded `read_to_string`. See the backend
guide for config-file bounds and atomic reload behavior.

## Custom read-only views

`llg/dumpTokens` uses `Backend::dump_tokens` to return `--dump-tokens` rows for
one document, a `# request-cache:` stats line, and trailing `# analysis:`
summary (always last). Failure returns one `# error:` line.

`module_explorer.rs` / `Backend::module_explorer` implement this contract:

The custom `llg/moduleExplorer` request reads only committed per-root `Analysis`
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
The serialization cap is shared across all workspace roots, and each
workspace retains a local truncation-marker reservation.

## Logging and high-memory diagnosis (`logging.rs`)

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
- At `trace`, the transport wrapper records every decoded inbound request or
  notification (`event=transport.receive`) and its completed dispatch
  (`event=transport.dispatch.end`) with bounded method/id metadata,
  parameter presence, and response/notification/error outcome.
- At `debug`, targeted `event=` records bracket workspace discovery, scheduler
  admission/debounce, input-budget and include staging, Surelog construction /
  return / session drop, parse fallback, owned DB/model/token/lint/index
  assembly, and root-job publication/completion.  `event=surelog.invoke`
  includes argv_count, a bounded/redacted argv_repr, a fixed-width argv
  fingerprint, and the configured parse, write-preprocessed-output, compile,
  elaborate, UHDM-elaboration, mute, and quiet modes.  Flag names and
  bounded paths remain visible; each argument is capped at 128 bytes and the
  overall representation at 2048 bytes; -D/-P values are redacted.  Isolated
  open-document parses emit the same record with their parse-only arguments.
  NUL-rejected invocations are recorded as rejected with argv_count=0.
  Records report counts and timings, never source contents or unbounded
  token/item dumps.

Use `LLG_LOG=debug` for lower-volume pipeline/Surelog traces and `LLG_LOG=trace` when a request
stalls before reaching a handler. Logging is disabled below the configured
level before formatting/writing. Transport metadata is escaped and bounded;
source and LSP payload bodies are never logged. Invocation representations
preserve flag ordering, escape controls, display bounded include/file paths,
and keep `-D`/`-P` names while replacing values with `<redacted>`. Rejected NUL
arguments never appear in accepted invocation records.

Useful lifecycle phases:

- `surelog.session_construct_parse_compile_elaborate`, `surelog.session_drop`;
- `analysis.source_graph`, `analysis.db_build`, `analysis.model_projection`,
  `analysis.lint`, `analysis.token_collection`;
- `analysis.macro_table`, `analysis.symbol_index`, `analysis.assembly`;
- `surelog.parse_only` for isolated open-document semantic tokens.

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
  (`llg --lint-config`) still uses its own `llg-lint.toml` reader in
  `core::lint`.

Findings from `core::lint` merge into diagnostics as source `llg-lint`, rule ID
as code, severity Error → `ERROR`. This runs inside analysis over owned DB/model.

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

Binding limitations and enum ambiguity behavior are in the feature guide.
