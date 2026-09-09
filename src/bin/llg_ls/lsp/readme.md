# LSP backend

- Purpose: the `lsp.rs` facade connects tower-lsp requests to the backend
  implementation.
- Scope: manages workspace state, configuration, bounded input admission,
  shadow staging, analysis scheduling, and diagnostic publication.

- `handlers/state.rs`: backend and per-root state queries/updates.
- `handlers/scheduling.rs`: rescans, jobs, debounce, config reloads, watchers.
- `handlers/staging.rs`: bounded snapshots, shadow paths, includes, isolation.
- `handlers/diagnostics.rs`: publication, deduplication, shared-file findings.
- `handlers.rs`: protocol types, custom requests, and `LanguageServer`.
- `handlers/tests.rs`: backend unit and async regressions.

- Boundary: blocking Slang compilation consumes admitted buffers; async request
  paths consume owned `Analysis` results.
