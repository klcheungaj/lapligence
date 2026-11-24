# Agent guide: src/bin/llg_ls/features

Analysis and request projections: `features.rs` and `features/`. Components:
[readme](readme.md); server rules: [llg_ls guide](../AGENTS.md); admission:
[lsp guide](../lsp/AGENTS.md).

## Directory map

- `analysis.rs`: blocking analysis assembly and library-unit recovery.
- `slang.rs`: frontend diagnostics conversion.
- `source_graph.rs`, `fallback.rs`: source-level graph and syntax-error extraction.
- `symbol_index.rs`, `requests.rs`: indexes and pure projections.
- `package_context.rs`: package- and definition-declaring units an isolated token
  parse adds.
- `protected.rs`: protected envelopes as black boxes (diagnostic dispositions).
- `tests.rs`, `tests/`: helpers and domain regressions.

## Rules

- `Analysis` owns all request data; no native handles or AST pointers enter
  feature code (`features.rs` docs give the analysis and coordinate contract).
- Merge elaborated instances with owned source-instance records to keep incoming
  edges, roots and declaration fallback for bodies excluded by explicit top
  selection; never reparse or reread files.
- Slang lexical tokens and semantic IDs define identity; nothing is resolved by
  name. Tokens and definitions read one table, `core::tokens::Bindings` beside
  the tokens; `TokenRule` holds the rule and its exceptions. To make a name
  navigate, bind it in the capture.
- Hover, references and rename read the same bindings and the committed
  `Analysis` only; so do explorer requests. Parameter hover/lint uses owned
  DB/model data, macro hover the frontend's macros (`core::macros`).

## Recovery

- On a native capture limit, retry the same buffers once as library units under
  unchanged limits (budgets: `AnalysisConfig::frontend_limits`; guidance texts:
  `llg_ls/config.rs`).
- Recovery keeps lexical tokens, definition topology, direct references and
  named connections; it omits lint and instance-specific elaboration.
- Never import the partial graph into execution, diagnose its intentionally
  absent expressions as DB failures, raise limits or read more files.
- Keep the limit guidance in fatal and reduced-mode diagnostics when recovery
  succeeds. `LLG_MEMORY_LIMIT_MB` raises no capture limit.

## Limits

- Class-instance member selection and positional connection pairing stay
  unsupported until Slang provides exact associations.
