# Rust FFI boundary

- Purpose: expose checked Rust APIs over the native C ABI.
- Modules:
  - `surelog.rs`: compile sessions and diagnostics.
  - `vpi.rs`: UHDM handles, traversal, and owned values.
  - `process_memory.rs`: platform memory sampling and limits.
- Boundary: the only Rust module permitted to contain `unsafe`.
- Consumers: shared-core capture and frontend process safeguards.
- Related: [C wrapper](../wrapper/readme.md) and [shared core](../core/readme.md).
