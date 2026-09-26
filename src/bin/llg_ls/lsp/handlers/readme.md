# Backend handlers

Child modules share backend state/protocol types and consume owned results.
Blocking Slang work and filesystem isolation stay in scheduling/staging.

- `state`: owned root snapshots and synchronous queries/updates.
- `scheduling`: jobs, debounce, rescans, config reloads and watchers.
- `staging`: bounded inputs, private shadow paths and include authorization.
- `diagnostics`: commits, publication, deduplication and shared-file aggregation.
- `tests`: private backend regressions.
