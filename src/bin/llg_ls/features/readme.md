# LSP feature modules

- Purpose: the `features.rs` facade projects owned analysis into LSP feature
  requests, including navigation, symbols, tokens, hover, completion,
  references, rename, and the module explorer.

- `analysis.rs`: blocking analysis assembly.
- `source_graph.rs`: owned source-level module graph.
- `fallback.rs`: syntax-error feature extraction.
- `symbol_index.rs`: declarations and references.
- `requests.rs`: pure request projections.
- `tests.rs`: feature regressions.

- Interaction: genvar and other source-level facts supplement elaborated data
  for navigation, rename, hover, and highlighting.
