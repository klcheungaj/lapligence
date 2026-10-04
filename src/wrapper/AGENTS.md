# C++ capture and C ABI

The wrapper exports bounded, owned Slang snapshots through a documented C ABI.
Export `extern "C"` functions only: no C++ templates/exceptions or Rust
panics/generics cross the ABI. Match the CMake-configured C++ standard, existing
naming/formatting and the C++ coding guide. Minimize includes/public-header
implementation details; prefer RAII, smart pointers and references to raw owners.
Do not introduce exceptions unless existing code uses them. Document ownership
at every function and clarify uncertain assumptions. Snapshot-owned strings are
interned stable borrowed views, never individually freed. Charge every occurrence
against the logical export budget even when its bytes are shared; snapshots
outlive temporary compilation.
Keep [FFI](../ffi/AGENTS.md) layouts/errors/budgets aligned. `mimalloc_shim.c`
redirects C malloc/free through GNU/LLD wrapping on musl.

Native zero-request defaults remain 64 MiB export, 1M semantic nodes, 4M edges
and 1M constants. Hard ceilings are 16 GiB export, 64M nodes, 256M edges and
16M constants; reject edge requests above the ceiling before UDP work. Charge
export records and strings before storing them, and name the effective exhausted
budget in diagnostics. A larger export budget does not bound total Slang/Rust RSS.

Capture's ordered pending edge vectors remain authoritative. Build role/index and
child-target indexes only for parents with at least 64 charged edges. Keep cache
positions aligned on append and role conversion; invalidate them when erases move
positions or bulk operations replace roles. Child attachment keeps target dedup
and the original next-child index, including gaps after role conversion. Avoid
per-node index storage; many duplicate child positions can still make front
removal linear in that target's duplicates.

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
- Library sources follow the compilation-unit mode: one syntax tree per file in
  separate mode, one per library in merged mode. Export the source library of
  each instance, package and compilation unit in the `source_libraries` table
  (at most one record per semantic node), never inferred from file names.
- Export each `LineDirectiveSyntax` the lexical capture visits as a
  `line_directives` record: the offset of the next physical line and the
  SourceManager's `getLineNumber`/`getFileName` there, so the values match the
  preprocessor's `` `__LINE__``/`` `__FILE__``. A directive on the last line
  maps nothing. Promote `RefArgAutomaticFunc` (ref formal of a static
  subroutine, IEEE 1800-2009 13.5.2) to an error before issuing diagnostics.
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
- Lexical kinds for typedef names come from Slang's resolved symbol kind. Cast
  targets and `$bits`/`type()` operands bind to the alias their expression type
  resolved, only when the spelling matches it. An instantiation type name whose
  definition is outside the compilation keeps the module kind with no role, so
  no navigation target is implied; a resolved binding for the token wins.
- Instance-array names retain every source index, including negative/nonzero and
  nested dimensions. Do not substitute flattened ordinals for HDL indices.
- Slang slices instance-array connections as `X[l:r][i]`/`X[l:r][m:n]`. A
  constant select inside a constant slice exports `X` as its base, because the
  slice type keeps `X`'s numbering; a slice with a run-time base stays.
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
