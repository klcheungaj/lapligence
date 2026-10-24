# Agent guide: src/wrapper

C++ Slang capture behind a documented C ABI. After capture the compilation is
destroyed and every table streams to the caller's `LlgSlangSink`. Contracts:
[readme](readme.md); ABI codes and layouts: `slang_c_api.h` comments.

## Directory map

- `slang_c_api.h`: the C ABI (the source of truth for records, codes, edge roles
  and auxiliary bits).
- `slang_c_api.cpp`: compile, capture, metadata parse and streaming.
- `slang_platform.hpp/.cpp`: the only platform conditionals (glibc heap trim).
- `mimalloc_shim.c`: GNU/LLD `--wrap` of C `malloc`/`free` on musl.
- `slang/`: vendored Slang (patched through `patches/slang/`; never edited here).

## ABI and C++ rules

- Export `extern "C"` functions only; no C++ templates or exceptions and no Rust
  panics or generics cross the ABI.
- Keep record layouts, table order, stream header and budgets in sync with
  [FFI](../ffi/AGENTS.md); bump `LLG_SLANG_ABI_VERSION` on both sides.
- Match the CMake-configured C++ standard and existing naming/formatting.
  Minimize includes and public-header implementation details; prefer RAII, smart
  pointers and references to raw owners. Add no exceptions where existing code
  has none (`SinkAborted` is caught only at the C entry points).
- Document ownership at every function; clarify uncertain assumptions.
- Extend the shared ABI/DB rather than exposing another native traversal.

## Capture rules

- Consume only admitted cached buffers; never reopen paths or canonicalize
  through live filesystem reads. Include misses fail before content reads.
- Every selected library header must already be in the cache-only source set.
- Charge every string occurrence against the logical export budget, even when
  interned bytes are shared, and charge records before storing them.
- Never preallocate in proportion to a limit, only to counts already captured.
  Bound size-derived allocation and traversal before the work; reject edge
  requests above the ceiling before UDP work.
- Keep source spelling and coordinates; never derive semantics from text.
- Preserve typed facts: matching syntax stays a typed pattern, not a Boolean
  condition; instance-array names keep every HDL index, never flattened ordinals.
- When a Slang upgrade adds syntax kinds, classify each against IEEE 1364-2001
  Annex A for `Verilog2001SyntaxProfile`; unlisted kinds reject by default.
- Changing the enabled unused/shadowed-declaration checks needs fixture evidence
  and an LSP diagnostic-policy review. Default-expression handling needs
  source-backed tests.

## Pitfalls

- Pending edge vectors are authoritative; the role/index caches of high-fanout
  parents must stay aligned on append and role conversion and be invalidated
  when erases move positions ([edge indexes](readme.md#edge-indexes)).

## Validation

- Test ordinary and reduced library-unit capture, nominal types, ordered and
  repeated operands, source bindings, budgets and rejection paths.
- Snapshot users stay correct after native owners are destroyed.
- Wrapper edits rebuild the shim; Slang/CMake/patch edits may rebuild the
  frontend. Keep the root build's static library and patch-preflight integration.
