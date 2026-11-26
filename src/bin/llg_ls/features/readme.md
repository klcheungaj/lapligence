# LSP feature modules

`features.rs` projects owned analysis into navigation, symbols, tokens, hover,
completion, references, rename and explorer requests. Source-level/genvar facts
supplement elaborated data for precise navigation and highlighting.

| Module | Responsibility |
| --- | --- |
| `analysis.rs` | Blocking analysis assembly. |
| `source_graph.rs` | Owned source-level module graph. |
| `fallback.rs` | Syntax-error feature extraction. |
| `protected.rs` | Protected envelopes as black boxes: hints instead of errors the hidden text explains ([behavior](../../../../docs/config.md#lsp-protected-source)). |
| `symbol_index.rs` | Declaration index of document and workspace symbols, completion and the explorer. |
| `requests.rs` | Pure projections, in the positions of the analysed text: the identifier table of a document (tokens and definitions), hover, references. `identifier_range` is the one rule for the width of a written name: an escaped identifier spans its backslash, which declaration names leave out. The outline ranges a design unit, class or subroutine over its whole declaration (`declaration_range`, from the captured `Analysis::decl_extents`); its name is the selection range. |
| `tests.rs`, `tests/` | Shared helpers and domain regressions. |

Test domains cover graphs, recovery, coordinates, hover, identifiers, bindings/connections,
references/symbols/completion, diagnostics/tokens, analysis/configuration,
cross-file behavior, packages, classes and shadow paths. See
[source layout](../../../../docs/source_layout.md).
