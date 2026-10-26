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

## Semantic token legend

The legend lists the standard LSP token types plus custom entries; clients
receive them in `semanticTokensProvider.legend`. Custom entries:

| Entry | Kind | Words or symbols |
| --- | --- | --- |
| `primitive` | type | Built-in data and net type words: `bit logic reg byte shortint int longint integer time shortreal real realtime string chandle event void signed unsigned genvar supply0 supply1 tri triand trior trireg tri0 tri1 uwire wire wand wor struct union enum`. |
| `declarationKeyword` | type | Direction, kind and qualifier words: `input output inout ref parameter localparam specparam const var typedef default automatic static`. |
| `connectionLabel` | modifier | Named port and parameter connection labels (`.a(x)`, `.W(2)`). |

Both custom types carry no modifier bits. Every other keyword is `keyword`, and
user typedef names are `type`. The custom types exist because VS Code styles
`keyword` from `keyword.control` and `type`/`class`/`struct` from
`entity.name.type*`, which would color these words like control flow or like
user type names. VS Code gives a type it has no registration or scope mapping
for no semantic style, so without a client contribution these words keep the
editor's base style. A client extension maps them, for example in its
`package.json` (use the language ids the client registers):

```json
"contributes": {
  "semanticTokenTypes": [
    { "id": "primitive", "description": "Built-in SystemVerilog data and net type words." },
    { "id": "declarationKeyword", "description": "SystemVerilog direction, kind and qualifier words." }
  ],
  "semanticTokenScopes": [
    { "language": "systemverilog", "scopes": {
      "primitive": ["storage.type.primitive.systemverilog"],
      "declarationKeyword": ["storage.modifier.systemverilog"] } },
    { "language": "verilog", "scopes": {
      "primitive": ["storage.type.primitive.verilog"],
      "declarationKeyword": ["storage.modifier.verilog"] } }
  ]
}
```

Give neither type a `superType`: a theme rule for `keyword` or `type` would
then win over the scope mapping. A user can instead set
`editor.semanticTokenColorCustomizations.rules` for `primitive` and
`declarationKeyword`. In Neovim the tokens reach the `@lsp.type.primitive` and
`@lsp.type.declarationKeyword` highlight groups, which can be linked to, for
example, `@type.builtin` and `@keyword.modifier`.

Explorer tests live in `module_explorer/tests.rs`; feature-test helpers and domain
suites in `features/tests.rs` and `features/tests/`. See
[source layout](../../../docs/source_layout.md).
