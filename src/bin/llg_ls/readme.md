# `llg_ls`

Tower-LSP stdio serves owned HDL analysis over JSON-RPC, with independent roots,
serialized analysis and read-only staged inputs. Bounded admitted buffers enter
Slang; only owned snapshots/indexes reach requests. Serving stdout is JSON-RPC only.

| Component | Responsibility |
| --- | --- |
| `main.rs`, `transport.rs` | Process setup, async runner, framed stdio and lifecycle. |
| `features.rs`, `features/` | Navigation, symbols, tokens, hover, completion, references, rename and explorer projections. |
| `config.rs` | LSP view of the shared `llg::config` schema: re-exports plus Slang option construction. |
| `lsp.rs`, `lsp/` | Workspace/configuration state, scheduling, staging, diagnostics and handlers. |
| `module_explorer.rs`, `module_explorer/` | Catalog, budgets, contents, hierarchy, compatibility, presentation and types. |

An instance of an unknown module is a black box: Slang's `UnknownModule` error is
published once on the module name, the analysis (a `Compile` outcome with feature
data) replaces the last-good snapshot, and everything else stays elaborated. The
instance name hovers as `<module> <instance>`, its actuals bind to the parent
scope, its labels and module name have no target, lint treats its connections as
possibly driven and read, and the explorer shows it as a declaration-only child
without a definition.

Explorer tests live in `module_explorer/tests.rs`; feature-test helpers and domain
suites in `features/tests.rs` and `features/tests/`. See
[source layout](../../../docs/source_layout.md).
