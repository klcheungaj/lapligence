# wrapper — C wrappers around native frontends

## Purpose

Small, well-contained C-ABI translation layers over native C++ APIs so Rust
(`src/ffi/`) never sees C++ directly:

- `slang_c_api.h/.cpp` — the snapshot-owned Slang frontend boundary;
  `slang/CMakeLists.txt` builds it with the vendored release.
- `mimalloc_shim.c` — redirects C `malloc`/`free` to mimalloc in musl builds
  through GNU/LLD `--wrap`.

## Requirements

- **C ABI only**: no C++ templates/exceptions across the boundary; exported
  functions are `extern "C"`.
- Slang strings borrow from an owned snapshot and are never freed individually.
  Ownership is documented at each function.
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
- The Slang capture must contain every semantic record required by the Rust
  semantic IR. It also emits owned, uninstantiated source-instance records from
  module syntax so consumers retain source topology for definitions excluded by
  top-selected elaboration. Do not silently treat unsupported semantic data as
  captured.
- Library-unit recovery is the explicit exception for language-server feature
  continuity after an export limit: retain lexical tokens, declarations,
  source module topology, and module-type bindings while omitting expression
  and statement records. Visit each source module body and generate block once,
  and capture expression references directly as lexical bindings. Check a
  source body for referenced definitions as well as inferred roots; associate
  temporary instances with their definition's parent scope. Resolve named
  connections through Slang's scoped definition lookup and source ports and
  parameters. Apply the same input and output limits.
- Capture exact scoped bindings for names in declared type dimensions, which
  Slang's default AST visitor omits. In library-unit recovery, also resolve
  initializer and continuous-assignment syntax when error-typed expressions
  have lost their AST operands; never resolve these names across unrelated scopes.
  Capture instance parameter/port actuals in their enclosing module or generate
  scope even when the child definition is absent from isolated analysis.
- Match Slang driver's default downstream analysis checks for unused and
  shadowed declarations. Changes to enabled checks require fixture evidence and
  corresponding language-server diagnostic policy review.

## Interactions

- Below: vendored Slang (built by `build.rs` via CMake).
- Above: modules under `src/ffi/` (the only consumers).

## Build cost

Editing `src/wrapper/*` triggers a fast wrapper rebuild; editing `vendor/slang`
triggers the frontend rebuild.
