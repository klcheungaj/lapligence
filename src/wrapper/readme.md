# C wrapper

- Purpose: bridge Surelog's C++ API to Rust through a C ABI.
- Components:
  - `surelog_c_api.h/.cpp`: handles, session flags, diagnostics, and design access.
  - `mimalloc_shim.c`: native allocation redirection.
- Boundary: C++ ownership and exceptions stop here; Rust uses C-compatible APIs.
- Consumer: [Rust FFI layer](../ffi/readme.md).
- Build: wrapper changes rebuild the bridge; vendored frontend changes rebuild Surelog.
