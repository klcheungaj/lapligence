# Agent guide: src/bin/llg_ls/lsp

Backend lifecycle and admission: `lsp.rs`, `handlers.rs` and `handlers/`.
Components: [readme](readme.md), [handlers](handlers/readme.md). Server rules:
[llg_ls guide](../AGENTS.md).

## Directory map

- `handlers/state.rs`: backend/root state, `DiscoveryOrder`, `ClientOrder`.
- `handlers/scheduling.rs`: rescans, jobs, debounce, reloads, watchers.
- `handlers/staging.rs`: bounded snapshots, authorized includes, private mirrors,
  single-flight tables.
- `handlers/diagnostics.rs`: publication order and shared-file unions.
- `handlers/tests.rs`: last-good, cancellation and exact-buffer regressions;
  keep fixtures within the configured admission limits.

## Scheduling rules

- Build Slang, DB, model and indexes in one `spawn_blocking` closure. Hold no
  backend lock across discovery, cleanup, admission or compilation.
- tower-lsp runs notifications concurrently: apply a discovery only when no
  later-started one was applied, discard stale generations, and only the job
  holding the root's current generation releases its running slot.
- Always publish diagnostics, after the state lock is released, through the
  ticketed `publish_ordered` path (`diagnostics.rs` docs).
- Valid analysis replaces `last_good`; compile/admission failures keep navigation.
- Watched-file registrations are serialized by their own gate and read the
  watcher set only after taking their turn.

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

- Isolated tokens use captured revisions and admitted text; recheck staleness
  before cache lookup, flight admission and the blocking compile. Saturated
  single-flight tables refuse new work and fall back to committed tokens.
- Shutdown clears roots, caches, flights, watchers and the private process
  mirror; only the per-process child of the staging root is ever removed
  (`--staging-dir` handling: `main.rs`).
