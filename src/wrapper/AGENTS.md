# wrapper — C wrappers around native frontends

## Purpose

Small, well-contained C-ABI translation layers over native C++ APIs so Rust
(`src/ffi/`) never sees C++ directly:

- `surelog_c_api.h/.cpp` — opaque handles (`SL_*`), session/flag setters,
  structured diagnostics (`SL_Diag`: severity/file/line/col/message), VPI
  design access.
- `slang_c_api.h/.cpp` — an opt-in, snapshot-owned bridge for Slang diagnostics
  and initial semantic capture; `slang/CMakeLists.txt` builds it with Slang.
- `mimalloc_shim.c` — redirects C `malloc`/`free` to mimalloc in musl builds
  through GNU/LLD `--wrap`.

## Requirements

- **C ABI only**: no C++ templates/exceptions across the boundary; exported
  functions are `extern "C"`.
- Surelog strings are `malloc`'d on the C side and freed with `sl_free_string`;
  `SL_Diag` carries its own malloc'd strings. Slang strings borrow from an owned
  snapshot and are never freed individually. Ownership is documented at each
  function.
- Match the C++ standard configured in `CMakeLists.txt`. Follow existing
  naming/formatting; minimize includes and keep implementation details out
  of public headers.
- Prefer RAII, smart pointers and references over raw owning pointers. Do not
  introduce exceptions unless the existing code already uses them; none may
  cross the C ABI. Rust panics/generics cannot cross either.
- Document ownership at every boundary; clarify uncertain assumptions.
- The Slang shim accepts explicit source buffers and typed options only. Enable
  cache-only source reads and disable filesystem path canonicalization before
  parsing. Lexically normalize cache keys; an include cache miss must fail
  before a filesystem content read.
- A Slang snapshot owns its strings and tables independently of the temporary
  compilation. Its views contain no Slang AST pointers. Enforce source, record,
  value-bit, related-diagnostic and exported-payload limits before allocation.
- Apply diagnostic mappings through `DiagnosticEngine` and synchronously copy
  `ReportedDiagnostic` records. Preserve intrinsic compiler errors even when a
  diagnostic is suppressed, and also record effective error severities.
- The initial Slang capture covers diagnostics, hierarchy, instance parameters,
  resolved parameter types and constants. Do not describe it as a complete DB
  or silently treat unsupported semantic data as captured.
- Match Slang driver's default downstream analysis checks for unused and
  shadowed declarations. Changes to enabled checks require fixture evidence and
  corresponding language-server diagnostic policy review.

## Interactions

- Below: vendored Surelog/UHDM/ANTLR and Slang (built by `build.rs` via CMake).
- Above: modules under `src/ffi/` (the only consumers).

## Build cost

Editing `src/wrapper/*` triggers a fast wrapper rebuild; editing
`vendor/Surelog` or `vendor/slang` triggers the corresponding frontend rebuild.
