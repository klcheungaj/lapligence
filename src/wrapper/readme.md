# Slang C wrapper

The C++ wrapper compiles admitted source buffers and streams the captured flat
tables through the C ABI (`LLG_SLANG_ABI_VERSION`) into a caller-supplied sink (`LlgSlangSink`). Capture
finishes while Slang's compilation is live; the compilation is then destroyed
and each table is delivered in bounded batches and released as soon as it has
been delivered (semantic nodes chunk by chunk). C++ lifetime management stays
behind the boundary; nothing native outlives `llg_slang_compile`.

Snapshot strings are interned in stable native storage. Export charging still
counts every string view, so admission limits retain their logical byte contract.
Pending edge storage is released as each ordered final window is copied.

Capture preserves typed declarations, values, dimensions, source/lexical data,
instance topology, references, diagnostics and ordered expression edges. Library-unit
recovery supplies bounded declaration/navigation data without claiming executable
completeness. Named connections retain separate child-port labels and parent-scope
actuals; array instances retain HDL indices. Pattern operands preserve position
and declaration order even when expression IDs repeat.
Class constraint blocks and their constraint trees are captured as semantic
kinds 29/30 with ordered roles; `randomize` calls carry a class/scope tag and
a BODY edge to their inline constraints. `dist` expressions keep their items and
`:=`/`:/` weights as ordered roles; foreach constraints name their loop variables.
Loop-generate entries retain their parent array's external name and their
elaborated source index for owned hierarchical paths.

Native source access is cache-only. Ordered library include prefixes attach to
their named `SourceLibrary` and cannot authorize a native filesystem read.
Flagged library-map buffers use Slang's map parser and preprocessor; its
configuration syntax nodes enter the same compilation as ordinary sources.
See [safe FFI](../ffi/readme.md),
[owned DB](../core/db/readme.md), [source layout](../../docs/source_layout.md)
and [patch preparation](../../patches/README.md).

## Native budgets

Defaults and structural ceilings: the `kHardMax*` and default-limit comments in
`slang_c_api.cpp`. A larger export budget does not bound total Slang/Rust RSS.

## Library sources and recovery

- Library sources follow the compilation-unit mode: one syntax tree per file in
  separate mode, one per library in merged mode. Each instance's, package's and
  compilation unit's source library is exported in `source_libraries` (at most
  one record per node), never inferred from file names.
- Diagnostics go through `DiagnosticEngine` mappings; `ReportedDiagnostic`
  records are copied synchronously, keeping intrinsic errors despite suppression
  and effective error severities.
- Library-unit recovery visits each module/generate body once, including
  referenced definitions and scopes, and captures declaration topology, direct
  references and named connections with exact type-dimension bindings.
  Temporary instances attach to definition-parent scopes; ports and parameters
  use scoped Slang lookup. Initializer/continuous-assignment syntax is recovered
  when error types lose operands, never across unrelated scopes. Actuals bind in
  the enclosing module/generate scope even when isolated analysis lacks the
  child; labels bind to child ports. Recovery is navigation-only under unchanged
  limits.
- Typedef-name lexical kinds come from Slang's resolved symbol kind. Cast
  targets and `$bits`/`type()` operands bind to the alias their expression type
  resolved, only when the spelling matches.
- Both capture modes bind, from the resolved AST, the name tokens no reference
  node carries: `::` qualifiers to the enclosing package or class, hierarchical
  path steps to the symbols of Slang's resolved path, `disable` targets and
  symbol operands, called subroutine names, and interface-port type names.
  System subroutine and built-in method names get the
  `LLG_SLANG_LEXICAL_SYSTEM_SUBROUTINE` kind with no target. Slang wraps a
  failed statement or expression, with its enclosing list, in an Invalid node
  the AST visitor never enters; capture walks that child for lexical bindings
  only (no semantic nodes), so only the names Slang could not resolve stay
  unbound identifiers.
- Named statement blocks, labeled statements (Slang makes each one a named
  block) and generate block names get `LLG_SLANG_LEXICAL_BLOCK` from the
  syntax, so declaration-only capture classifies them too: the opening name is
  the declaration, a matching end label (`end : blk`, `join : f`) binds to it,
  and resolved references (`disable blk`, the `g` of `g[0].x`) take the kind
  from the bound generate or statement-block scope. Instance names keep their
  kind. A keyword carries `LLG_SLANG_LEXICAL_CONTROL_FLOW` only where its
  syntax position makes it control flow: the `default` of a case,
  case-generate, property-case or randsequence-case item.

## Parameter overrides

Overrides are planned before the elaborating compilation: trees are parsed
first, and a throwaway compilation sharing them instantiates the tops (requested,
or uninstantiated outermost default-library modules/programs) to classify each
target (`slang_c_api.cpp` override planning comments). Type parameters get
`type(VALUE)` through the tracked Slang patch; local parameters are rejected
(`LocalParameterOverride`) and dropped. Classification uses default types, so a
type that depends on another override is not re-derived. Messages show the
given spelling.

## Assignment patterns

Every position keeps an `OPERAND`, even when positions share one expression ID;
structural children may deduplicate shared IDs, indexed operands may not. An
erroneous intermediate default omits its placeholder without discarding valid
bound elements. Unresolved index keys keep `INDEX` expression edges.

## Edge indexes

Role/index and child-target indexes are built only for parents with at least 64
edges. Child attachment keeps target deduplication and the original next-child
index, including gaps after role conversion. There is no per-node index storage,
so many duplicate child positions can still make front removal linear in that
target's duplicates.
