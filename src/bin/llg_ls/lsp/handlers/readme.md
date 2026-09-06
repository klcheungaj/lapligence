# Backend handlers

- Purpose: child modules share the backend state and protocol types.

- `state`: owned workspace snapshots and synchronous state operations.
- `scheduling`: analysis jobs, debounce, rescans, config reloads, and watchers.
- `staging`: bounded inputs, private shadow paths, and include authorization.
- `diagnostics`: result commits, publication, and aggregation.
- `tests`: backend regressions kept private to the module.

- Boundary: handlers consume owned results; blocking Surelog work and
  filesystem isolation remain in the established scheduling/staging paths.
