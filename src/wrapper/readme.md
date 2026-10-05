# Slang C wrapper

The C++ wrapper compiles admitted source buffers and streams the captured flat
tables through C ABI v14 into a caller-supplied sink (`LlgSlangSink`). Capture
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
Loop-generate entries retain their parent array's external name and their
elaborated source index for owned hierarchical paths.

Native source access is cache-only. Ordered library include prefixes attach to
their named `SourceLibrary` and cannot authorize a native filesystem read.
Flagged library-map buffers use Slang's map parser and preprocessor; its
configuration syntax nodes enter the same compilation as ordinary sources.
See [safe FFI](../ffi/readme.md),
[owned DB](../core/db/readme.md), [source layout](../../docs/source_layout.md)
and [patch preparation](../../patches/README.md).
