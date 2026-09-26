# LSP analysis and projections

Applies to `features.rs`, `analysis.rs`, `source_graph.rs`, `fallback.rs`,
`symbol_index.rs`, `requests.rs` and `tests.rs`. Read [server](../AGENTS.md) and
[admission](../lsp/AGENTS.md) contracts.

## Analysis and identity

`Analysis` owns all request data. `analyze()` compiles admitted buffers, imports
`core::db::Db`, projects the model, lints valid frontend results and builds lexical
and symbol indexes. No native handles/AST pointers enter feature code.

Merge elaborated instances with owned source-instance records to preserve incoming
edges, roots and declaration fallback for bodies excluded by explicit top selection;
never reparse or reread files. Slang lexical tokens/semantic IDs define identity.
`core::tokens::RefBindings` maps zero-based positions to exact owned declarations;
conflicting targets remain unbound, never selected by name.

## Diagnostics and recovery

Preserve provider, code, ranges, related locations and formatted Slang messages in
`frontend_diagnostics`. Compact core diagnostics classify validity/preflight failures.
Syntax/compile failures do not replace last-good navigation.

Root semantic capture is capped at 100,000 nodes. On a native limit, retry the same
buffers once as library units under unchanged limits, even for one unit. Visit each
module/generate body once, check every definition's source body and capture direct
reference/named-connection bindings without expression/statement graphs. Retain lexical
tokens and definition topology, omitting lint and instance-specific elaboration.
Do not import this partial graph into execution or diagnose its intentionally absent
expressions as DB failure. Never raise limits or read more files during recovery.

Log source/export/capture limits at error level before recovery with configured
limits and exclusion guidance. Retain guidance in fatal/reduced-mode diagnostics
when recovery succeeds. Native caps are neither `llg.toml` settings nor raised by
`LLG_MEMORY_LIMIT_MB`.

## Requests

Read committed `Analysis` only; navigation/explorer requests do not read files or
compile. Isolated semantic tokens are the exception: exact admitted unsaved buffers,
with authoritative empty output on syntax errors. Parameter hover/lint uses owned
DB/model data; macro hover uses the bounded analysis-time source table. Preserve
UTF-16 boundary conversion and one-based internal token coordinates until response
construction.

Scoped fallback must not override exact bindings or resolve ambiguous locations.
Class-instance member selection and positional connection pairing remain unsupported
until Slang provides exact associations.
