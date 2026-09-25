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
  buffers; missing includes cannot read file contents. Named library buffers and
  the default library search order are supplied as borrowed request metadata,
  so configuration elaboration also remains restricted to admitted contents.
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

## Conditional pattern roles

`LLG_SLANG_EDGE_CONDITION_PATTERN` (38) pairs a conditional statement/expression
pattern with the same source index as its `CONDITION` edge. `THEN` and `ELSE`
remain separate branch roles at index zero. This extends the repository-owned
semantic tag set without changing C record layouts or exporting native pointers.
Update the Rust checked decoder together with the bridge. Capturing a pattern
is not a claim of executable pattern-matching support; it prevents consumers
from silently converting `value matches pattern` into a Boolean test of `value`.

## Assignment-pattern operands and keys

Simple assignment patterns export one indexed `OPERAND` edge for each entry
in Slang's bound element list, even when several positions share one expression
identity. Structured fixed-array patterns with type setters, including mixed
explicit-index/type/default forms, export Slang's resolved elements in
declaration order; Slang has already applied explicit-index precedence,
recursive type matching, the last matching type setter, and defaults. Fixed-array patterns whose nested default is an error-typed
intermediate also export their valid bound elements, omitting those synthetic
placeholders. Fixed-array elements are reordered from Slang's increasing-index
storage to declared left-to-right order. Other explicit index-key patterns keep
their keyed operands and an `INDEX` edge to the original key expression,
including parameter and constant-function expressions. Structural child links
may be deduplicated; positional operands may not. Consumers must use ordered
operand edges rather than reconstructing operand counts from structural
children, and evaluate retained index keys from owned semantic values rather
than source text.
