# Rust FFI boundary

- Purpose: expose checked Rust APIs over the native C ABI.
- Modules:
  - `surelog.rs`: compile sessions and diagnostics.
  - `vpi.rs`: UHDM handles, traversal, and owned values.
  - `slang.rs` (feature `slang`): in-memory Slang compilation and bounded,
    owned diagnostic, hierarchy, type, and parameter observations.
  - `process_memory.rs`: platform memory sampling and limits.
- Boundary: the only Rust module permitted to contain `unsafe`.
- Slang ownership: the C++ shim owns snapshot storage during decoding; Rust
  validates and copies every exported record before the native owner is freed.
  No Slang AST address or native allocation crosses the safe API.
- Consumers: shared-core capture and frontend process safeguards.
- Related: [C wrapper](../wrapper/readme.md) and [shared core](../core/readme.md).
