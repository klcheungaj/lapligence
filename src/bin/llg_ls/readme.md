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

| Modifier | Index | Used with | Words or symbols |
| --- | --- | --- | --- |
| `connectionLabel` | 10 | `function`, `property` | Named port and parameter connection labels (`.a(x)`, `.W(2)`). |
| `typeDefinition` | 11 | `keyword` | `typedef struct union enum`. |
| `direction` | 12 | `keyword` | `input output inout ref`. |
| `primitive` | 13 | `keyword` | Built-in data and net types and declaration kind and qualifier words: `parameter localparam specparam const var bit logic reg byte shortint int longint integer time shortreal real realtime string chandle event void signed unsigned genvar supply0 supply1 tri triand trior trireg tri0 tri1 uwire wire wand wor default automatic static`. |
| `controlFlow` | 14 | `keyword` | Control flow: `if else case casex casez randcase endcase for foreach while do repeat forever return break continue wait wait_order` everywhere, and, where the syntax makes them control flow, `unique unique0 priority` qualifying an if or case, the `disable` of a disable statement, and a case item `default`. |

A keyword carries at most one llg modifier. The control-flow words are the
IEEE 1800-2009 statement keywords that choose, repeat, leave or wait for
execution (A.6.5 to A.6.8); `if`, `else`, `case` and `for` keep the modifier in
conditional and loop generate constructs, property statements and
constraints, which select or iterate the same way. Positional words are
marked by the capture, not by their text: a case, case-inside, case-generate,
property-case or randsequence-case item `default` is control flow while every
other `default` (assignment-pattern keys, `default clocking`, `default disable
iff`, default bins) keeps `primitive`; the `disable` of `disable iff` and the
`unique` of a uniqueness constraint stay plain. Block delimiters (`begin`,
`end`, `fork`, `join*`), `iff`, `inside`, `matches` and every other keyword are
`keyword` without modifier bits; `->` is an `operator`. `static` and
`automatic` are always lifetime or class qualifiers. User typedef names are
`type`. A client that knows no llg modifier still renders the words as
keywords.

Named and labeled statement blocks and generate block names use the standard
`namespace` type, like packages: the opening name (`begin : blk`, `lbl:`,
`fork : f`, `begin : g` in a generate) adds `declaration`; end labels and
references (`end : blk`, `disable blk`, `g[0].x`, `top.g.x`) carry no
modifier and navigate to the opening name. Instance names stay `variable`.

### Client mapping

VS Code gives the standard `keyword` type the default scope `keyword.control`
(Dark+ `#C586C0`), the control-flow color, so without a client mapping every
keyword looks like control flow; the server alone cannot change that.
rust-analyzer's VS Code extension solves the same problem by contributing
`"keyword": ["keyword.other.rust"]` and `"keyword.controlFlow":
["keyword.control.rust"]`. VS Code matches selector modifiers by name, needs
no registration for them, and lets a selector with more matching modifiers
(and then one naming the language) win, so a client extension maps, per
language id it registers:

```json
"contributes": {
  "semanticTokenModifiers": [
    { "id": "controlFlow", "description": "SystemVerilog control-flow keywords." },
    { "id": "typeDefinition", "description": "SystemVerilog type-definition words." },
    { "id": "direction", "description": "SystemVerilog port directions." },
    { "id": "primitive", "description": "SystemVerilog built-in type and declaration words." }
  ],
  "semanticTokenScopes": [
    { "language": "systemverilog", "scopes": {
      "keyword": ["keyword.other.systemverilog"],
      "keyword.controlFlow": ["keyword.control.systemverilog"] } },
    { "language": "verilog", "scopes": {
      "keyword": ["keyword.other.verilog"],
      "keyword.controlFlow": ["keyword.control.verilog"] } }
  ]
}
```

With that, plain keywords resolve to `keyword` (`#569CD6` in Dark+ and Dark
Modern) and control flow to `keyword.control` (`#C586C0`). The grouped words
then share the plain keyword color; a client that wants them apart adds
`keyword.typeDefinition`, `keyword.direction` and `keyword.primitive`
selectors (for example `storage.type.systemverilog` and
`storage.modifier.systemverilog`, also `#569CD6` in Dark+). The mapping works
with the TextMate grammar disabled, because VS Code probes the scopes against
the theme's token colors. A theme or user semantic rule for `keyword` that
sets the foreground still wins over any contributed scope. A user without
such an extension can set the colors directly in `settings.json`:

```json
"editor.semanticTokenColorCustomizations": {
  "rules": {
    "keyword:systemverilog": "#569cd6",
    "keyword.controlFlow:systemverilog": "#c586c0"
  }
}
```

The `:systemverilog` suffix limits a rule to one language id; repeat the rules
for each id the client uses, or drop the suffix to apply them everywhere.

Explorer tests live in `module_explorer/tests.rs`; feature-test helpers and domain
suites in `features/tests.rs` and `features/tests/`. See
[source layout](../../../docs/source_layout.md).
