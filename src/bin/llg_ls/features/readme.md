# LSP feature modules

This directory separates the blocking analysis pipeline (`analysis.rs`), the
owned parse-tree module graph (`source_graph.rs`), syntax-error fallback
extraction (`fallback.rs`), symbol indexing (`symbol_index.rs`), and pure LSP
request projections (`requests.rs`). The parent `features.rs` remains the
compatibility facade used by the backend, dump, rename, and module explorer.
The original feature regression suite lives in `tests.rs` so the facade stays
small while exercising the same combined API surface.

Source-level genvar facts supplement elaborated symbols for scoped navigation,
rename, hover, and highlighting, including pruned loops and syntax fallback.
See the [binding contract](AGENTS.md#genvar-source-bindings) and the framed-LSP
regressions in `tests/lsp_stdio/genvar.rs`.
