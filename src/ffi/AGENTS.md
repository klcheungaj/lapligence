# ffi — Rust ↔ C(++) FFI layer

## Purpose

The only module that talks to the vendored Surelog/UHDM C++ world, over a
C-ABI boundary (`src/wrapper/`):

- `surelog.rs` — Surelog compile sessions: `SurelogSession`, `SessionBuilder`
  (parse/compile/elaborate/`-elabuhdm` flags), structured `Diag`/`Severity`.
- `vpi.rs` — safe wrapper over the UHDM VPI traversal API: `iterate`/`handle`
  (with the `OwnedHandle` lifetime rule), `get`/`get_str`/`read_value`.
- `process_memory.rs` — platform-specific physical-footprint sampler
  (Linux/macOS/Windows) backing the shared `memory_limit` safeguard; the only
  place `unsafe` platform calls live.

## Requirements

- **This is the ONLY module allowed to contain `unsafe`** (extern blocks,
  `unsafe impl Send`, the single union read inside `vpi::read_value`).
  Enforced: `grep -rn "unsafe" src --include=*.rs | grep -v src/ffi` must be
  empty.
- Exposes **stable safe APIs** for `core`, `sim`, and the binaries: values are
  read via `vpi::read_value -> ValueData` (owned), never via the raw
  `VpiValueData` union.
- `SessionBuilder` owns its three C++ construction handles until `build()`
  explicitly transfers them to `SurelogSession`; abandoning the builder frees
  them in dependency order. The legacy `start_compiler()` API returns `None`
  instead of wrapping a null C++ compiler pointer.
- Handle lifetime rule (documented in `vpi.rs`): raw handles from `iterate`
  are borrowed; handles from `handle(...)` are `OwnedHandle`s that free on
  drop — keep them alive while in use, use `.raw()` for nested calls.

## Interactions

- Below: `src/wrapper/` (C ABI) and `vendor/Surelog` (C++).
- Above: `src/core/` (compile, elab, db, model, tokens) — all UHDM access
  goes through here; `src/sim/` and the binaries consume `core`'s owned data
  and never call VPI directly.
