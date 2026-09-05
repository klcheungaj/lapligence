# LSP feature modules

This directory separates the blocking analysis pipeline (`analysis.rs`), the
owned parse-tree module graph (`source_graph.rs`), syntax-error fallback
extraction (`fallback.rs`), symbol indexing (`symbol_index.rs`), and pure LSP
request projections (`requests.rs`). The parent `features.rs` remains the
compatibility facade used by the backend, dump, rename, and module explorer.
The original feature regression suite lives in `tests.rs` so the facade stays
small while exercising the same combined API surface.
