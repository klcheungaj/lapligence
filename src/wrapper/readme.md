# C wrappers

- Purpose: bridge native C++ frontend APIs to Rust through C ABIs.
- Components:
  - `slang_c_api.h/.cpp`: bounded compile-to-owned-snapshot capture from Slang,
    including compiler and analysis diagnostics, typed elaborated nodes and
    edges, resolved type and constant tables, source ranges, and lexical tokens
    with declaration/reference/connection-label bindings.
  - `slang/CMakeLists.txt`: isolated Slang and C ABI shim build.
  - `mimalloc_shim.c`: musl-link allocation redirection.
- Boundary: C++ ownership and exceptions stop here; Rust uses C-compatible APIs.
  Slang inputs live for one compile call. Its returned snapshot owns all table
  and string storage; views borrow that storage until snapshot destruction.
  Semantic errors are successful snapshots carrying an error flag, while bridge
  failures return an owned error record. Destruction accepts null.
- Admission: buffers are explicitly marked as compilation units or include-only.
  Cache-only reads and lexical path normalization restrict includes to admitted
  buffers; missing includes cannot read file contents.
- Scope: the snapshot is the only native frontend boundary. Rust converts its
  semantic records into independently testable semantic and execution IRs.
- Consumer: [Rust FFI layer](../ffi/readme.md).
- Build: wrapper changes rebuild the bridge; vendored Slang changes rebuild the
  frontend. musl targets use the selected musl C++ compiler and static runtime.
