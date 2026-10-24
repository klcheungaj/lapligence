# Agent guide: src

The `llg` library crate (`lib.rs`) plus the native wrapper and the binaries.
Module ownership is in the `lib.rs` crate docs; child guides own their contracts.

## Directory map

- `config/`: shared `llg.toml` schema ([readme](config/readme.md)).
- `core/`: compile, owned DB, values, projections and lint ([guide](core/AGENTS.md)).
- `ffi/`: the only Rust `unsafe`; Slang ABI receivers and platform calls
  ([guide](ffi/AGENTS.md)).
- `wrapper/`: C++ Slang capture behind the C ABI ([guide](wrapper/AGENTS.md)).
- `sim/`: simulator pipeline and C11 runtime ([guide](sim/AGENTS.md)).
- `bin/`: `llg`, `llg_ls` and demos ([guide](bin/AGENTS.md)).
- `memory_limit.rs`: process-memory policy (module docs); platform primitives in
  `ffi/process_memory.rs`. User variables: [readme](../readme.md).
- `profile.rs`: opt-in `LLG_PROFILE_STAGES=1` stage markers (module docs;
  scaling runs in [perf](../perf/README.md)).

## Platform code

- Platform conditionals live only in the platform layers; add a layer function
  instead of a conditional elsewhere
  ([layers](../docs/cross_platform.md#platform-layers)).
- Resolve and compare paths through `ffi/secure_fs.rs`, never by string.
- Check [cross-platform handling](../docs/cross_platform.md) before touching
  paths, text output, process exit, C compilation or CI.
- Windows on ARM ships `llg_ls` only: code `llg_ls` uses (library, frontend, Db,
  lint) stays portable there.

## Safeguard changes

Owners: LSP admission ([lsp guide](bin/llg_ls/lsp/AGENTS.md)); requests, caches,
explorer limits and tracing ([llg_ls guide](bin/llg_ls/AGENTS.md)); test gates
([safeguards](../tests/readme.md#safeguards)).

- Apply byte/count limits before input cloning, size-derived allocation, cache
  keys, single-flight admission, staging, parsing or optional-response
  serialization. Keep exact boundaries.
- Check or saturate arithmetic for totals, percentages, timeouts and size
  conversions.
- Open buffers are authoritative. Bound closed reads to maximum plus one byte and
  reuse admitted snapshots. Keep include authorization, canonical deduplication
  and cache-only native reads; never reopen unadmitted paths.
- Reject invalid configuration atomically, keeping the last valid config and
  servable analysis. Stale revisions or saturated flights start no frontend work.
- Emergency logs stay allocation-light, bounded, redacted and off JSON-RPC stdout.
- Test Unix limit restoration and Windows Job Object ownership natively before
  changing platform claims.
- Change defaults, diagnostics, tests, docs and guides together for any
  environment variable, configuration key, limit or fallback.
