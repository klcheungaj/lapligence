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

The legend lists only standard LSP token types; llg-specific modifiers follow
the standard modifiers (their indices are stable). Clients receive both lists
in `semanticTokensProvider.legend`. llg modifiers:

| Modifier | Used with | Words or symbols |
| --- | --- | --- |
| `connectionLabel` | `function`, `property` | Named port and parameter connection labels (`.a(x)`, `.W(2)`). |
| `typeDefinition` | `keyword` | `typedef struct union enum`. |
| `direction` | `keyword` | `input output inout ref`. |
| `primitive` | `keyword` | Built-in data and net types and declaration kind and qualifier words: `parameter localparam specparam const var bit logic reg byte shortint int longint integer time shortreal real realtime string chandle event void signed unsigned genvar supply0 supply1 tri triand trior trireg tri0 tri1 uwire wire wand wor default automatic static`. |

Each grouped word carries exactly its one keyword modifier at every occurrence.
Every other keyword is `keyword` without modifier bits, and user typedef names
are `type`. A client that knows no llg modifier still renders the words as
keywords.

VS Code matches a selector's modifiers by name, needs no registration for
them, and lets a selector with more matching modifiers win. Without a mapping
for `keyword.<modifier>`, a grouped word therefore falls back to the `keyword`
default `keyword.control`, the same color as control flow; the server alone
cannot change that. A client extension maps the three selectors to the scopes
C/C++ grammars use for `int` and `const` (both `#569cd6` in Dark+ and Dark
Modern), for example in its `package.json` (use the language ids the client
registers):

```json
"contributes": {
  "semanticTokenModifiers": [
    { "id": "typeDefinition", "description": "SystemVerilog type-definition words." },
    { "id": "direction", "description": "SystemVerilog port directions." },
    { "id": "primitive", "description": "SystemVerilog built-in type and declaration words." }
  ],
  "semanticTokenScopes": [
    { "language": "systemverilog", "scopes": {
      "keyword.typeDefinition": ["storage.type.systemverilog"],
      "keyword.direction": ["storage.modifier.systemverilog"],
      "keyword.primitive": ["storage.type.systemverilog"] } },
    { "language": "verilog", "scopes": {
      "keyword.typeDefinition": ["storage.type.verilog"],
      "keyword.direction": ["storage.modifier.verilog"],
      "keyword.primitive": ["storage.type.verilog"] } }
  ]
}
```

The scope mapping works with the TextMate grammar disabled, because VS Code
probes the scopes against the theme's token colors. Registering the modifiers
only documents them; matching does not need it. A theme without a `storage`
rule, or a theme or user semantic rule for `keyword` that sets the foreground,
still leaves the words keyword-colored. A user without such an extension can
set the colors directly in `settings.json`:

```json
"editor.semanticTokenColorCustomizations": {
  "rules": {
    "keyword.typeDefinition:systemverilog": "#569cd6",
    "keyword.direction:systemverilog": "#569cd6",
    "keyword.primitive:systemverilog": "#569cd6"
  }
}
```

The `:systemverilog` suffix limits a rule to one language id; repeat the rules
for each id the client uses, or drop the suffix to apply them everywhere.

Explorer tests live in `module_explorer/tests.rs`; feature-test helpers and domain
suites in `features/tests.rs` and `features/tests/`. See
[source layout](../../../docs/source_layout.md).
