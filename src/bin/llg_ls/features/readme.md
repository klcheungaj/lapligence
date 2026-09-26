# LSP feature modules

`features.rs` projects owned analysis into navigation, symbols, tokens, hover,
completion, references, rename and explorer requests. Source-level/genvar facts
supplement elaborated data for precise navigation and highlighting.

| Module | Responsibility |
| --- | --- |
| `analysis.rs` | Blocking analysis assembly. |
| `source_graph.rs` | Owned source-level module graph. |
| `fallback.rs` | Syntax-error feature extraction. |
| `symbol_index.rs`, `requests.rs` | Declaration/reference indexes and pure projections. |
| `tests.rs`, `tests/` | Shared helpers and domain regressions. |

Test domains cover graphs, recovery, coordinates, hover, bindings/connections,
references/symbols/completion, diagnostics/tokens, analysis/configuration,
cross-file behavior, packages, classes and shadow paths. See
[source layout](../../../../docs/source_layout.md).
