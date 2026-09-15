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

## Test organization

`tests.rs` keeps shared fixtures and helpers. The `tests/` children group
regressions by module graphs, recovery, coordinates, hover, bindings,
connections, references, symbols, completion, diagnostics, tokens, analysis
pipeline, configuration, cross-file behavior, packages, classes and shadow
paths. Production feature entry points remain unchanged.

See [the source map](../../../../docs/source_layout.md).
