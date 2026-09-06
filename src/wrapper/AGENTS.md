# wrapper — C wrapper around Surelog/UHDM

## Purpose

Small, well-contained C-ABI translation layer over the Surelog C++ API so
Rust (`src/ffi/`) never sees C++ directly:

- `surelog_c_api.h/.cpp` — opaque handles (`SL_*`), session/flag setters,
  structured diagnostics (`SL_Diag`: severity/file/line/col/message), VPI
  design access.
- `mimalloc_shim.c` — redirects C `malloc`/`free` to mimalloc in musl builds
  through GNU/LLD `--wrap`.

## Requirements

- **C ABI only**: no C++ templates/exceptions across the boundary; exported
  functions are `extern "C"`.
- Strings crossing the boundary are `malloc`'d on the C side and freed with
  `sl_free_string`; `SL_Diag` carries its own malloc'd strings. Ownership is
  documented at each function.
- Match the C++ standard configured in `CMakeLists.txt`. Follow existing
  naming/formatting; minimize includes and keep implementation details out
  of public headers.
- Prefer RAII, smart pointers and references over raw owning pointers. Do not
  introduce exceptions unless the existing code already uses them; none may
  cross the C ABI. Rust panics/generics cannot cross either.
- Document ownership at every boundary; clarify uncertain assumptions.

## Interactions

- Below: vendored Surelog/UHDM/ANTLR (built by `build.rs` via CMake).
- Above: `src/ffi/surelog.rs` / `src/ffi/vpi.rs` (the only consumers).

## Build cost

Editing `src/wrapper/*` triggers a fast wrapper rebuild; editing
`vendor/Surelog` or its CMake config triggers a full Surelog rebuild (slow).
