# LSP analysis and feature projections

Applies to `features.rs` and its children: `analysis.rs`, `source_graph.rs`,
`fallback.rs`, `symbol_index.rs`, `requests.rs`, and `tests.rs`. Read
[../AGENTS.md](../AGENTS.md) for protocol and cache contracts and
[../lsp/AGENTS.md](../lsp/AGENTS.md) for snapshot serving and source admission.

## Analysis boundary

`Analysis` owns every value used by request handlers. `analyze()` compiles the
admitted buffers with Slang, builds `core::db::Db` from the owned snapshot,
projects the design model, runs lint when the frontend is valid, and constructs
the lexical token and symbol indexes. Native handles and AST pointers never
enter feature code.

The module source graph merges elaborated DB instances with the snapshot's
owned source-instance records. Slang does not elaborate bodies excluded by an
explicit top selection, so those records preserve incoming edges, root
classification, and declaration fallback without reparsing or reading files.

Use Slang lexical tokens and their semantic IDs for declaration/reference
identity. `core::tokens::RefBindings` maps zero-based source positions to exact
owned declaration targets. Conflicting targets at the same position remain
unbound; request code must omit a result instead of choosing by name.

## Diagnostics and last-good serving

Preserve Slang's diagnostic provider, code, ranges, related locations, and
formatted message in `frontend_diagnostics`. The compact core diagnostic list
only classifies snapshot validity and supplies preflight failures. Syntax or
compile failures must not replace a root's last-good navigation snapshot.

If a multi-unit workspace exceeds a native export limit before any snapshot
can be returned, retry the same already-admitted buffers once as Slang library
units. This bounded recovery keeps lexical tokens, declarations, module source
topology, and cross-file module bindings, but omits expression and statement
capture, lint, and elaborated hierarchy. It must never raise the native limit
or read additional files.

## Requests

Request projections are pure reads of committed `Analysis`. Open-document
semantic tokens may compile the exact admitted unsaved buffer in isolation;
syntax errors yield an authoritative empty stream. Definition, references,
rename, hover, symbols, completion, and the module explorer must not read files
or start compilation.

Parameter hover values and lint findings come from the owned DB/model.
Macro hover uses the bounded source table constructed during analysis. Preserve
UTF-16 conversion at the source boundary and keep internal token coordinates
one-based until LSP response construction.

## Limits

Navigation outside an exact semantic binding can use the existing scoped index
fallback, but it must not override an exact binding or invent a target for an
ambiguous location. Class instance member selection and positional connection
pairing remain unsupported until Slang exposes the required exact associations.
