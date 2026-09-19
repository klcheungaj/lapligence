# C wrappers

- Purpose: bridge native C++ frontend APIs to Rust through C ABIs.
- Components:
  - `slang_c_api.h/.cpp`: bounded compile-to-owned-snapshot capture from Slang,
    including compiler and analysis diagnostics, typed elaborated nodes and
    edges, resolved type and constant tables, source ranges, and lexical tokens
    with declaration/reference/connection-label bindings. DPI-C import aliases
    and context/pure flags are copied into the owned subroutine records.
    Instance-array element names retain every declared source index, including
    negative/nonzero bounds and nested dimensions, before owned DB flattening.
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

Lexical flag bit 3 (`LLG_SLANG_LEXICAL_DIRECTIVE`, owned `is_directive`) identifies
preprocessor directive text, including unexpanded macro replacement bodies.
It is independent of macro-expansion and skipped-token flags. The Rust decoder
accepts only these four known flag bits and still rejects unknown bits/reserved
fields. Update both sides together: source edition checks use the provenance to
avoid rejecting a directive body which never becomes executable source.
