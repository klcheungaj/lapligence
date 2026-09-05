# LSP backend

This directory contains the tower-lsp backend behind the narrow `lsp.rs`
facade.

- `handlers/state.rs` owns the backend and per-root state plus synchronous state
  queries and updates.
- `handlers/scheduling.rs` owns rescans, job creation and compilation, debounce
  orchestration, config reload scheduling, and dynamic watcher registration.
- `handlers/staging.rs` owns bounded input snapshots, shadow-tree staging, isolated
  semantic-token staging, include resolution, and path-containment checks.
- `handlers/diagnostics.rs` owns config and compile diagnostic publication, payload
  deduplication, and shared-file aggregation.
- `handlers.rs` owns protocol data types, custom requests, and the
  `LanguageServer` implementation.
- `handlers/tests.rs` is the conventional `handlers::tests` child module and
  keeps backend unit and async regression tests close to the split
  implementation without widening production visibility.

All Surelog work remains blocking and serialized through the existing state
and staging locks. Only owned `Analysis` data crosses back into async request
handling.
