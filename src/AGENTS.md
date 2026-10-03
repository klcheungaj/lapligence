# Shared source policies

`lib.rs` exposes `ffi`, `core`, `sim` and `memory_limit`. Root architecture rules
apply throughout; child guides own module contracts.

## Process-memory safeguard

[memory_limit.rs](memory_limit.rs) owns policy;
[ffi/process_memory.rs](ffi/process_memory.rs) owns platform primitives. `llg` and
`llg_ls` install the guard after accepting CLI work and before source admission,
not for help, version or usage errors. `llg_ls` integrates lifecycle logging;
`llg` sends status/warnings to stderr.

The optional process-wide budget covers Rust and in-process Slang/C++ allocations.
Configure it before starting either frontend:

| Variable | Meaning | Default |
| --- | --- | --- |
| `LLG_MEMORY_LIMIT_MB` | Positive integer physical-memory budget in MiB | unset/disabled |
| `LLG_MEMORY_WARNING_PERCENT` | Percentage triggering one warning | `80` |
| `LLG_MEMORY_POLL_MS` | Sampling interval in milliseconds | `1000` |
| `LLG_MEMORY_ADDRESS_SPACE_LIMIT` | Reversible Linux/macOS `RLIMIT_AS`; accepts `true/false`, `yes/no`, `on/off`, `1/0` | `false` |

The watchdog samples Linux RSS, macOS physical footprint/resident usage or Windows
working set. Budget exhaustion prints a fixed emergency stderr message and
terminates immediately, avoiding shutdown allocations. Windows also uses a Job
Object process limit. Optional Unix `RLIMIT_AS` limits virtual, not physical,
memory and may fail allocations before the watchdog. Failed native installation
is reported; retain the watchdog when it can start, including inside an existing
Windows host job.

Budget for startup and peak Slang parsing/capture, not idle usage. The guard covers
concurrent LSP work, frontend compilations and driver activity. It samples and
terminates only the frontend, not its generated simulator or aggregate child RSS.
Unix children inherit enabled `RLIMIT_AS`; Windows descendants remaining in the job
may inherit native limits. These effects are host-dependent; guard child physical
memory separately.

## Safeguard changes

[LSP admission](bin/llg_ls/lsp/AGENTS.md) owns staging;
[LSP policy](bin/llg_ls/AGENTS.md) owns requests, caches, explorer limits and tracing;
[validation](../tests/AGENTS.md) owns gates.

- Apply byte/count limits before input cloning, size-derived allocation, cache
  keys, single-flight admission, staging, parsing or optional-response serialization.
  Preserve exact boundaries; check or saturate arithmetic for totals, percentages,
  timeouts and size conversions.
- Keep open buffers authoritative. Bound closed reads to maximum plus one byte
  and reuse admitted snapshots. Preserve include authorization, canonical
  deduplication and cache-only native reads; never reopen unadmitted paths.
- Reject invalid configuration atomically, retaining the documented last-valid
  config and servable analysis. Stale revisions or saturated flights must not
  start frontend work.
- Keep emergency logs allocation-light, bounded, redacted and off JSON-RPC stdout.
  Test Unix limit restoration and Windows Job Object ownership natively before
  changing platform claims.
- Update defaults, diagnostics, tests and owning guides together for any changed
  environment variable, configuration key, limit or fallback.

## Pipeline profiling

`profile.rs` emits opt-in `llg-profile begin/end` stderr markers when
`LLG_PROFILE_STAGES=1`. Wrapper markers separate Slang elaboration/analysis from
capture; FFI, DB import, semantic/typed/execution lowering, optimization and
rendering use the same format. Keep this diagnostic helper frontend-neutral;
markers must not affect ABI fields, diagnostics or generated files.
`perf/scripts/frontend_scale.py` combines these markers with sampled Linux RSS
and GNU time whole-process peaks. RSS includes all representations live in a
stage and allocator-retained pages, not that stage's exclusive allocation cost.
