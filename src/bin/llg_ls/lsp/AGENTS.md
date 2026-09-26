# LSP lifecycle and admission

Applies to `lsp.rs`, `lsp/handlers.rs` and its children. Read
[server policy](../AGENTS.md), [memory safeguards](../../../AGENTS.md) and
[validation](../../../../tests/AGENTS.md).

## Scheduling and snapshots

`handlers/state.rs` owns backend/root state; `scheduling.rs` owns rescans, jobs,
debounce, reloads and watchers; `staging.rs` owns bounded snapshots, authorized
includes and private mirrors; `diagnostics.rs` owns publication/shared-file unions.

Build Slang, owned DB/model/indexes in one `spawn_blocking` closure. Hold no backend
lock across discovery, cleanup, admission or compilation. Discard stale generations;
triggers during a run arm one dirty-root follow-up. Always publish diagnostics.
Valid analysis replaces `last_good`; compile/admission failures retain navigation.
Snapshot replacement/clearing advances the process-wide epoch and invalidates caches.

## Admission

- Admit `.v`/`.sv` units; headers/arbitrary included extensions are include-only.
  Search beside the includer, then configured source/include directories in order.
  Enforce lexical, canonical and symlink containment; count canonical identities
  once to terminate cycles.
- Check unique files against `analysis.max_file_bytes` and roots against
  `analysis.max_total_input_bytes`, with max-plus-one closed reads and exact open
  UTF-8 lengths. Reject unreadable, unmeasurable, invalid-UTF-8, changed or
  over-budget input before compilation. Never fall back to a live path.
- Log size rejection at error level with bounded path/size fields and
  `[sources].exclude`/`[analysis]` guidance; publish the same admission diagnostic.
  `LLG_MEMORY_LIMIT_MB` does not raise source/native export caps.
- Pass complete `InputSnapshots` to `config::compile_opts_sources`, preserving
  unit/include-only roles. Slang may not open unadmitted project files.

## Requests and shutdown

Isolated tokens use captured revisions/admitted text. Recheck staleness before
cache lookup, flight admission and blocking compile. Saturated bounded single-flight
tables refuse new work and fall back to committed tokens. Keep stdout JSON-RPC only.

Shutdown clears roots, caches, flights, watchers and the private process mirror.
`handlers/tests.rs` preserves last-good, cancellation and exact-buffer regressions;
keep fixtures within configured admission limits.
