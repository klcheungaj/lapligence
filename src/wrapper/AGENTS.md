# C++ capture and C ABI

The wrapper exports bounded, owned Slang snapshots through a documented C ABI.
Export `extern "C"` functions only: no C++ templates/exceptions or Rust
panics/generics cross the ABI. Match the CMake-configured C++ standard, existing
naming/formatting and the C++ coding guide. Minimize includes/public-header
implementation details; prefer RAII, smart pointers and references to raw owners.
Do not introduce exceptions unless existing code uses them. Document ownership
at every function and clarify uncertain assumptions. Snapshot-owned strings are
borrowed views, never individually freed; snapshots outlive temporary compilation.
Keep [FFI](../ffi/AGENTS.md) layouts/errors/budgets aligned. `mimalloc_shim.c`
redirects C malloc/free through GNU/LLD wrapping on musl.

## Capture

- Consume only admitted cached buffers, never reopen paths or canonicalize through
  live filesystem reads. Lexically normalize keys; include misses must fail
  before content reads. Preserve source identity and typed facts. Apply mappings
  through `DiagnosticEngine`, synchronously copy `ReportedDiagnostic` records,
  and retain intrinsic errors despite suppression plus effective error severities.
- Apply ordered library include prefixes to the matching `SourceLibrary` only;
  every selected header must already be in the cache-only source set.
- Parse flagged map buffers with `SyntaxTree::fromLibraryMapBuffer` before
  compilation so configuration symbols retain native macro-use locations.
- Snapshot ownership includes typed tables, lexical/source data and uninstantiated
  source-instance records. Bound all size-derived allocation/traversal before work.
  Preserve source spelling/coordinates without deriving semantics from text.
- Library-unit recovery visits each module/generate body once, including referenced
  definitions/scopes, and captures declaration topology, direct references and
  named connections. Preserve exact type-dimension bindings. Associate temporary
  instances with definition-parent scopes; use scoped Slang lookup for ports and
  parameters. Recover initializer/continuous-assignment syntax when error types
  lose operands, never across unrelated scopes. Actuals bind in their enclosing
  module/generate scope even when isolated analysis lacks the child; labels bind
  to child ports. Recovery is navigation-only under unchanged input/output limits.
- Instance-array names retain every source index, including negative/nonzero and
  nested dimensions. Do not substitute flattened ordinals for HDL indices.
- `ConditionPattern` role 38 pairs with its indexed condition; true/false edges
  use explicit branch roles/index 0. Preserve matching syntax as typed patterns,
  not Boolean conditions. Tag extensions do not change ABI pointer/layout rules.

## Assignment patterns

Retain an `OPERAND` for every position, even when positions share one expression
ID. Resolved bound elements follow declaration order; translate Slang's
increasing-index ordering where needed. If an intermediate default is erroneous,
omit its placeholder without discarding valid bound elements. Unresolved index
keys retain `INDEX` expression edges. Structural children may deduplicate shared
IDs; indexed operands may not.

Match the Slang driver's unused/shadowed-declaration checks; changing enabled
checks requires fixture evidence and LSP diagnostic-policy review. Default
expression handling needs source-backed tests, not assumptions that all defaults
execute. Preserve type identity, member defaults, order and conversions. Extend the shared ABI/DB rather
than exposing another native traversal to a consumer.

## Validation

Test ordinary capture and reduced library-unit capture, nominal types, ordered
operands, repeated IDs, source bindings, budgets and rejection paths. Snapshot
users must remain correct after native owners are destroyed. Wrapper edits rebuild
the shim; Slang/CMake/patch edits may rebuild the frontend. Preserve the root
build's static library and portable patch-preflight integration.
