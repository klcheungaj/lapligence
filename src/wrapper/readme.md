# Slang C wrapper

The C++ wrapper compiles admitted source buffers and exports an owned flat snapshot
through C ABI v4. C++ lifetime management stays behind the boundary; Rust copies
and validates the result before releasing its owner.

Capture preserves typed declarations, values, dimensions, source/lexical data,
instance topology, references, diagnostics and ordered expression edges. Library-unit
recovery supplies bounded declaration/navigation data without claiming executable
completeness. Named connections retain separate child-port labels and parent-scope
actuals; array instances retain HDL indices. Pattern operands preserve position
and declaration order even when expression IDs repeat.
Loop-generate entries retain their parent array's external name and their
elaborated source index for owned hierarchical paths.

Native source access is cache-only. See [safe FFI](../ffi/readme.md),
[owned DB](../core/db/readme.md), [source layout](../../docs/source_layout.md)
and [patch preparation](../../patches/README.md).
