# Shared source policies

`lib.rs` exposes `ffi`, `core`, `sim`, and `memory_limit`. Root architecture
rules apply throughout. See child guides for module contracts.

## Process-memory safeguard

Owned by [memory_limit.rs](memory_limit.rs), with platform primitives in
[ffi/process_memory.rs](ffi/process_memory.rs). Both frontend entry points
install it; `llg_ls` wires the sampler into lifecycle logging and `llg`
reports status and warnings to stderr.

The `llg_ls` language-server and `llg` simulator-driver executables can
enforce a process-wide memory ceiling. The ceiling covers Rust allocations and
the in-process Surelog/UHDM and optional Slang frontends, including their
C++ allocations.

The safeguard is disabled by default. Enable it with an environment variable
before starting either `llg_ls` or `llg`:

| Variable | Meaning | Default |
| --- | --- | --- |
| `LLG_MEMORY_LIMIT_MB` | Maximum physical-memory budget in MiB. A positive integer enables the safeguard. | unset (disabled) |
| `LLG_MEMORY_WARNING_PERCENT` | Emit one warning when physical usage reaches this percentage of the budget. | `80` |
| `LLG_MEMORY_POLL_MS` | Watchdog sampling interval in milliseconds. | `1000` |
| `LLG_MEMORY_ADDRESS_SPACE_LIMIT` | On Linux/macOS, also request a reversible `RLIMIT_AS` limit. Accepts `true`/`false`, `yes`/`no`, `on`/`off`, or `1`/`0`. | `false` |

The watchdog measures the complete process: RSS on Linux, physical
footprint/resident usage on macOS, and working-set usage on Windows. If the
physical usage exceeds the configured budget, it writes a fixed emergency
message to stderr and terminates immediately. Immediate termination avoids
allocating during a graceful shutdown after the process has already exceeded
the safety boundary.

Native enforcement is platform-specific:

- Windows uses a Job Object process-memory limit in addition to the watchdog.
- Linux and macOS use the watchdog by default. The optional `RLIMIT_AS`
  setting limits virtual address space, not physical memory, and may cause an
  allocation to fail earlier than the physical watchdog.
- If a native limit cannot be installed (for example, because the process is
  already inside a host job on Windows), `llg_ls` logs the error and retains the
  portable watchdog when it can start. `llg` reports the same safeguard
  status and warnings to stderr.

Choose a budget above the server's or driver's normal startup footprint:
Surelog parsing and UHDM construction can temporarily require substantially
more memory than an idle server. A budget that is too small can terminate the
server during its first analysis. The native limit and watchdog are
process-wide, so the budget also covers concurrent LSP work and every Surelog
session in `llg_ls`, or the driver-side work in `llg`.

`llg` launches the generated simulator as a separate child process. The
portable watchdog samples and terminates only the `llg` driver; it does not
aggregate the child's RSS or install a watchdog inside the generated binary.
On Unix, an enabled `RLIMIT_AS` is inherited by child processes. On Windows,
Job Object limits may apply to descendants when the child remains in the job.
Those native behaviors are platform/host dependent, so a simulator child that
needs its own physical-memory budget should be guarded separately.

## Safeguard change review

LSP input admission and staging live in
[bin/llg_ls/lsp/AGENTS.md](bin/llg_ls/lsp/AGENTS.md); request/cache/explorer
limits and diagnostic tracing in [bin/llg_ls/AGENTS.md](bin/llg_ls/AGENTS.md).
Validation commands and CI scope live in [../tests/AGENTS.md](../tests/AGENTS.md).

- Apply byte and count limits before cloning input, allocating from an
  input-derived size, computing cache keys, joining a single flight, staging,
  parsing, or serializing optional response content.
- Preserve exact-boundary behavior and use checked or saturating arithmetic
  for totals, percentages, timeouts, and size conversions.
- Keep open-buffer text authoritative while bounding closed-file reads to
  the configured maximum plus one byte; reuse the admitted snapshot afterward.
- Preserve include authorization, canonical-path deduplication, and the
  fail-closed rule that Surelog reads only staged shadow inputs and Slang
  reads only admitted in-memory buffers through its cache-only source manager.
- Keep invalid configuration atomic and retain the last valid config and
  last servable analysis snapshot where documented.
- Verify scheduler and cache backpressure: stale revisions and saturated
  flights must not start new Surelog work.
- Keep emergency logging allocation-light, bounded, redacted, and off
  stdout; stdout remains the JSON-RPC transport.
- Exercise Linux/macOS limit restoration and Windows Job Object ownership
  on their native platforms before changing platform-specific claims.
- Update defaults, diagnostics, tests, and the owning AGENTS.md together whenever
  an environment variable, configuration key, limit, or fallback changes.
