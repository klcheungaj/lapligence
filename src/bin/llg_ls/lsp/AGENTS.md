# Agent guide: src/bin/llg_ls/lsp

Backend lifecycle and admission: `lsp.rs`, `handlers.rs` and `handlers/`.
Components: [readme](readme.md), [handlers](handlers/readme.md). Server rules:
[llg_ls guide](../AGENTS.md).

## Directory map

- `handlers/state.rs`: backend/root state, `DiscoveryOrder`, `ClientOrder`.
- `handlers/inputs.rs`: which events start an analysis (module docs).
- `handlers/scheduling.rs`: rescans, jobs, debounce, reloads, watchers.
- `handlers/identifiers.rs`: the table that answers a document's token and
  definition requests.
- `handlers/positions.rs`: positions between an analysed and an edited text.
- `handlers/staging.rs`: bounded snapshots, authorized includes, private mirrors,
  single-flight tables.
- `handlers/diagnostics.rs`: publication order and shared-file unions.
- `handlers/tests.rs`: last-good, cancellation and exact-buffer regressions;
  keep fixtures within the configured admission limits.

## Scheduling rules

- Build Slang, DB, model and indexes in one `spawn_blocking` closure. Hold no
  backend lock across discovery, cleanup, admission or compilation.
- An event starts an analysis only through the content comparisons of
  `inputs.rs`; a change only admits its text
  ([user rules](../../../../docs/config.md#when-an-analysis-starts)).
- A job asks its token (`RootJob::cancel`) after each wait and stage. A cancelled
  one returns `JobEnd::Cancelled`: it publishes nothing, leaves `last_good` and
  `analysed` alone and has one `event=root_job.end outcome=cancelled` line.
- No handler takes the staging lock on the async runtime: a job holds it for a
  whole analysis, and its handler would keep the save that cancels it waiting.
- tower-lsp runs notifications concurrently: apply a discovery only when no
  later-started one was applied, discard stale generations, and only the job
  holding the root's current generation releases its running slot.
- Always publish diagnostics, after the state lock is released, through the
  ticketed `publish_ordered` path (`diagnostics.rs` docs).
- Valid analysis replaces `last_good`; compile/admission failures keep navigation.
- A request position reaches `last_good` only through `Placement::analysed`, and
  its positions leave the server only through `Places`, or not at all; sent
  diagnostics move through `BackendState::place_diagnostics` after the digest
  comparison ([handlers](handlers/readme.md)). Open texts change only through
  `BackendState::set_document` and `remove_document`.
- Watched-file registrations are serialized by their own gate and read the
  watcher set only after taking their turn. No notification handler waits for
  a reply of the client: tower-lsp stops reading when its handlers are taken.

## Admission rules

- Only `.v`/`.sv` are units; headers and other included extensions are
  include-only. Search beside the includer, then configured source/include
  directories in order. Enforce lexical, canonical and symlink containment;
  count canonical identities once to end cycles.
- Check unique files against `analysis.max_file_bytes` and roots against
  `analysis.max_total_input_bytes` before cloning, staging, caching or
  compiling, with max-plus-one closed reads and exact open UTF-8 lengths.
- Reject unreadable, unmeasurable, invalid-UTF-8, changed or over-budget input
  before compilation; never fall back to a live path. Admission failure keeps the
  last valid snapshot.
- Log size rejections at error level with bounded fields and
  `[sources].exclude`/`[analysis]` guidance, and publish the same diagnostic.
  `LLG_MEMORY_LIMIT_MB` does not raise source or native caps.
- Pass complete `InputSnapshots` to `config::compile_opts_sources`, keeping
  unit/include-only roles; Slang never opens unadmitted files.

## Requests and shutdown

- Tokens and definitions of a document read one table (`DocumentTable` docs):
  the committed analysis, or the isolated parse of an uncommitted text. Never
  answer one of the two requests from anything else.
- An isolated parse uses captured revisions, admitted text and the committed
  analysis' include and package context (never a file access), so it runs beside
  a project analysis, one at a time (`isolated_parse_lock`); recheck staleness
  before cache lookup, flight admission and the blocking compile. An edit cancels
  the parse of the older text (`cancel_superseded`). Saturated single-flight tables
  refuse new work; the committed table answers.
- Record each token request's inputs under the capture lock; a commit that
  changes its stream sends `workspace/semanticTokens/refresh` from a spawned
  task (`ServedTokens` docs), never from a request handler.
- Shutdown, `exit` and the end of the input (once its requests are answered)
  cancel the running analysis and isolated parse (`cancel_analyses`), whose end
  the cleanup waits for.
- Shutdown clears roots, caches, flights, watchers and the private process
  mirror; only the per-process child of the staging root is ever removed
  (`--staging-dir` handling: `main.rs`).
