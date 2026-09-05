# LSP backend lifecycle and input admission

Applies to `lsp.rs`, `lsp/handlers.rs`, and `handlers/` children. Read
[../AGENTS.md](../AGENTS.md) for protocol/request limits and logging,
[../../../AGENTS.md](../../../AGENTS.md) for the shared memory safeguard and
change-review checklist, and [../../../../tests/AGENTS.md](../../../../tests/AGENTS.md)
for validation.

`handlers/state.rs` owns Backend/root state; `scheduling.rs` owns rescans,
jobs, config reloads and watchers; `staging.rs` owns bounded snapshots,
include authorization and shadow paths; `diagnostics.rs` owns publication,
deduplication and shared-file aggregation. `handlers.rs` holds protocol types,
custom requests and `LanguageServer`; `handlers/tests.rs` tests the combined
backend without widening production visibility.

## Scheduling and snapshots

- Surelog is blocking, uses C++ global state, and its session is not `Send`.
  Run compile + DB/model + tokens in one `spawn_blocking` closure behind the
  process-wide mutex, drop the session there, and return only owned `Analysis`.
  Do not hold backend state locks across discovery, cleanup, staging or config I/O.
- `scheduler.rs` is an event-driven, pure idle→pending(timer)→running→dirty
  state machine, unit-tested without tokio. An idle root has no polling loop.
  Use one ~300 ms trailing-edge timer per root. Triggers during a run mark it
  dirty rather than queueing jobs; completion arms exactly one follow-up on
  newest inputs. Finite bursts cost at most two runs; continuous editing
  converges without supersession churn. Generation checks discard stale results.
  Identical full-text `didChange`s do not reschedule or churn analysis epochs.
- `Analysis::has_feature_data` permits every non-Fatal analysis with index
  declarations, model modules or tokens to replace `last_good`. Parse/Compile
  outcomes serve best-effort over surviving files; non-syntax errors can still
  leave elaborated data. Syntax errors skip Surelog compile/UHDM, so
  `features::parse_tree_feature_parts` uses `core::tokens::collect_parse_tokens`
  to provide parse tokens, recorded port/
  signal/parameter declarations and same-file references, plus a modules-only
  model (module names/header positions). Named labels are retyped for reference
  indexing. Instance data, elaborated values and general cross-module use-site
  resolution remain unavailable, with named-connection/enum fallback bindings
  described in [../features/AGENTS.md](../features/AGENTS.md).
- Fatal results (absent/poisoned UHDM or isolation aborts) and empty unservable
  results retain the last served snapshot; before the first servable commit
  there is nothing to serve. Diagnostics always publish. Snapshot replacement
  or clearing assigns a fresh process-global analysis epoch; model/source-graph/
  top changes or clearing notify explorer clients as described in the parent guide.

## Workspace configuration and ownership

`config.rs` owns typed `LlgConfig`, `SourcesConfig`, `CompileConfig`, and
`AnalysisConfig`, loading/retention, source/include derivation and conversion
to `CompileOpts`, including the `.v`/`.sv` compilation-unit predicate.

- Each workspace folder is independent, using `<root>/llg.toml` schema v1
  unless the client supplies a config location in `initializationOptions`:
  `{ "llg": { "protocolVersion": 1, "configFiles": [
  { "workspaceUri": ..., "path": ... } ] } }`. Clients supply locations,
  not discovery globs. Missing configs use the root as source directory,
  recursive `.v`/`.sv` discovery, and excludes `slpp_all/**`, `.git/**`, `target/**`.
- Only `.v`/`.sv` are compilation units. Headers `.vh`/`.svh` and arbitrary
  extensions enter through includes. Source dirs are include-search dirs;
  `compile.include_dirs` adds search-only dirs, including external dirs.
  Root-relative `sources.include`/`exclude` globs are evaluated against each
  source dir, with exclude winning.
- Longest-root ownership is structural over configured include dirs: deepest
  matching dir wins, then lowest normalized root path for ties. An outer root
  does not compile a file owned by a nested root even if the nested discovery
  filters reject it. Root add/remove rescans and re-evaluates ownership without
  replacing other roots' state/config/shadows/snapshots.
- Shared files tracked through discovery/includes use owner-wins analysis;
  other roots consume the owner's results. Full per-root re-analysis of shared
  files remains deferred. Semantic tokens use the owner; since only the owner
  has tokens, classification conflicts cannot be compared (log ownership at
  trace). Shared-file hover labels config-dependent values/define text with
  the owner root name.
- `[compile] top` selects the elaboration top; `defines` entries `NAME` or
  `NAME=VALUE` become `-D` for every root source. First duplicate define name
  wins. `[compile.param_overrides]` maps identifier keys to strings/integers
  (integers normalized to decimal), producing `-PNAME=VALUE`, equivalent to
  top-level `-GNAME=value`. Overrides affect explicit/auto-detected top
  instances only, not per-module scopes; a key declared by no top is an error.
- Config structural errors (including nonpositive analysis budgets) reject
  atomically, publish against the TOML URI, and retain last-valid config or
  safe defaults before any valid load. Malformed define entries and invalid/
  empty override keys/values are dropped with `llg-config` warnings while the
  rest loads. Config reads are separately bounded to `MAX_CONFIG_BYTES`
  (1 MiB) plus one byte; oversize/invalid UTF-8 is a bounded read failure.
- Any parsed-config change reschedules analysis: discovery changes rescan,
  top/defines/overrides/include dirs rebuild compile inputs, lint changes
  republish. State-changing reloads send empty-params `llg/configChanged`
  for config-derived views (especially inactive-region dimming); identical
  parsed reloads do not notify. TOML content never requires restart. VS Code
  settings `hdlsupport.llg.path`, `.configPath`, `.logLevel` trigger client-side
  restart (`restartPolicy.ts`).

## Diagnostics and watchers

- After each commit publish diagnostics for every analyzed file, open or
  closed. Keep clean compilation units with empty lists and union current/
  previous URI keys to clear files leaving analysis. Suppress identical
  per-URI payloads with per-root digests, evicting digests when no longer
  published. `didClose` removes staged open text and reschedules an on-disk
  refresh; it does not clear diagnostics immediately.
- Shared paths are published exclusively through a union of tracking roots'
  findings, never the primary per-root map. Deduplicate identical URI/range/
  severity/code/message; distinct same-location non-owner findings get a
  `[<root-name>]` suffix. This prevents primary publication swallowing labels.
- Initial scans and standard `workspace/didChangeWatchedFiles` drive updates;
  there is no internal filesystem watcher. Dynamically register watchers when
  supported for effective configs, `.v`/`.sv` under source dirs and exact
  resolved include dependencies of any extension. Re-register after every
  feature-data-bearing commit, including non-valid analyses; reuse the
  registration ID and suppress unchanged sets.
- At commit rebuild dep-path→dependent-roots indexing. Route watched events
  through include dependencies first, scheduling EVERY dependent root, then
  apply source/config classification. Newly resolved dependencies become known
  after compile. Input rejection leaves discovery/watcher registration policy
  unchanged.

## Read-only staging and isolation

- Open text from `didOpen`/full-text `didChange` wins over disk; `didClose`
  removes it. `ShadowPaths` uses pure `shadow_path`/`real_path` helpers for a
  deterministic real↔shadow bijection in OS-temp `llg-{pid}-{rand}`, mirroring
  absolute paths. Stage literal transitive dependencies including unopened disk
  files and open non-HDL or filtered buffers; map all returned locations to real URIs.
- Authorize literal includes under the owning root's configured source/include
  dirs (possibly external), using lexical, canonical and symlink containment
  checks. Reject escapes from every allowed dir; there is no blanket ban on
  crossing workspace roots.
- Project compile uses staged shadow include directories exclusively. Missing,
  newly appearing or macro-generated/dynamic includes cannot fall through to
  live `-I` dirs; they produce frontend diagnostics. Dump/general compilation
  deliberately retains its separate live-directory behavior.
- Park the process CWD in `<shadow base>/work/analyze/` during blocking compile:
  Surelog freezes its first-session CWD and writes `slpp_all/`, logs and caches
  there. Reset contents between jobs, remove on shutdown, and clean stale-root
  state. Never create, modify, chmod or delete project/config/external files.

## Input-size safeguards

Each root's schema-v1 `llg.toml` may set an `[analysis]` byte budget:

| Field | Default | Meaning |
| --- | ---: | --- |
| `max_file_bytes` | `1048576` (1 MiB) | Maximum bytes in one unique compilation unit or resolved literal include. |
| `max_total_input_bytes` | `8388608` (8 MiB) | Maximum bytes across all unique compilation units and resolved literal includes. |

Both values must be positive integers. Invalid or zero values reject the
configuration atomically, preserving the previous valid configuration. The
server measures open UTF-8 text by its actual byte length and otherwise uses
filesystem metadata; canonicalized include paths are counted once, including
cycles. Literal includes are resolved beside the including file first, then
through configured source directories followed by `compile.include_dirs`, in
the same order supplied to Surelog. Readable closed inputs are bounded-read
once during admission and the exact admitted text is reused by isolation and
shadow staging, closing the closed-file growth/re-read gap. An over-limit root
publishes an `input-size-limit` diagnostic with the real path, measured bytes,
configured budget, and budget kind before shadow staging or Surelog. Discovery
and watched-file registration are unchanged, and the last-good analysis
snapshot remains served.

If a file was measurable at budget admission but its bounded read, UTF-8
decode, or shadow staging later fails, the root is rejected with an
input-snapshot or input-staging diagnostic. The server does not fall back to
the live file in that case. Discovered/root compilation units that are absent,
unreadable, or otherwise unmeasurable at admission are rejected the same way;
no root file reaches Surelog unrestricted. A newly appearing literal include
is not admitted retroactively.
