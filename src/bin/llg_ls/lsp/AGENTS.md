# LSP backend lifecycle and input admission

Applies to `lsp.rs`, `lsp/handlers.rs`, and the `handlers/` children.
Read [../AGENTS.md](../AGENTS.md) for protocol and feature contracts,
[../../../AGENTS.md](../../../AGENTS.md) for memory safeguards, and
[../../../../tests/AGENTS.md](../../../../tests/AGENTS.md) for validation.

## Scheduling and snapshots

`handlers/state.rs` owns backend/root state; `scheduling.rs` owns rescans,
jobs, debounce, configuration reloads, and watchers; `staging.rs` owns
bounded source snapshots, include authorization, and private mirrors;
`diagnostics.rs` owns publication and shared-file aggregation.

Run blocking Slang compilation and owned DB/model/index construction inside one
`spawn_blocking` closure. Do not hold backend state locks across discovery,
cleanup, source admission, or compilation. Generation checks discard stale
results; a trigger during a run marks the root dirty and arms one follow-up.

Diagnostics always publish. A valid owned analysis replaces `last_good`.
Compile/admission failures retain the last-good navigation snapshot. Replacing
or clearing a snapshot advances the process-wide analysis epoch and invalidates
request caches.

## Input admission

Only `.v` and `.sv` files are compilation units. Headers and other included
files enter as include-only buffers. Resolve literal includes beside the
including file, then in configured source/include directory order. Apply
lexical, canonical, and symlink containment checks against authorized
directories, and count canonical identities once so include cycles terminate.

Measure every unique source against `analysis.max_file_bytes` and the root
against `analysis.max_total_input_bytes`. Use max-plus-one reads for closed
files and exact UTF-8 byte lengths for open buffers. Reject unreadable,
unmeasurable, invalid UTF-8, changed-during-admission, and over-budget inputs
before native compilation. Never fall back to a live path after admission.

Pass the complete admitted `InputSnapshots` set to
`config::compile_opts_sources`: root files are compilation units and resolved
includes are include-only sources. Slang must receive no permission to open
unadmitted project files.

## Requests and shutdown

Open-document semantic token work uses the captured revision and admitted text.
Recheck staleness before cache lookup, flight admission, and blocking compile.
The bounded single-flight table refuses new work when saturated and falls back
to committed tokens.

Stdout remains JSON-RPC only. Shutdown clears roots, request caches, flights,
watchers, and the private process mirror. Tests live in
`handlers/tests.rs`; keep source fixtures within the configured admission
limits and preserve last-good, cancellation, and exact-buffer regressions.
