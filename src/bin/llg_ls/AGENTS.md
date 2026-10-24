# Agent guide: src/bin/llg_ls

tower-lsp stdio server over owned analysis: diagnostics, tokens, hover,
navigation, symbols, completion, rename, inactive ranges, token dumps and a
read-only module explorer. Components: [readme](readme.md); user behavior:
[configuration](../../../docs/config.md).

## Ownership

- `lsp.rs`, `lsp/`: lifecycle, scheduling, admission, staging, diagnostics and
  handlers ([guide](lsp/AGENTS.md)).
- `features.rs`, `features/`: analysis, bindings, indexes and projections
  ([guide](features/AGENTS.md)).
- `scheduler.rs`: debounce and follow-up policy (module docs).
- `semantic_tokens.rs`: token types and modifiers (the mapping is in the code).
- `request_cache.rs`: bounded request caches; `logging.rs`: `LLG_LOG`/`LLG_LOG_FILE`.
- `config.rs`: re-exports `llg::config` and builds Slang options; add or change
  keys in the library, never here.
- `module_explorer.rs`, `module_explorer/`: explorer catalog and budgets.

## Rules

- tower-lsp, tokio and dashmap stay in this binary behind default-on `lsp`.
- Serving stdout is JSON-RPC only; logs go to stderr or the configured file.
  Help, version and `--dump-tokens` print ordinary text without serving; invalid
  arguments exit 2 with usage on stderr.
- Driver-only keys (marked `llg` in `docs/config.md`) are validated and ignored.
  An unknown key rejects the whole file atomically, in the server and in
  `--dump-tokens`.
- Request handlers never touch native objects or start project compilation; one
  blocking closure compiles admitted buffers into owned DB, indexes and
  `Analysis`.
- Keep the Slang diagnostic provider, code, full message, primary range and
  related locations. Ranges stay on the physical buffer; a `` `line``-mapped
  position adds a first related entry on that range.
- Publish current failures while serving navigation from the last-good snapshot.
- Exact semantic bindings outrank scoped fallback; ambiguity yields no target.
- Cache keys include the applicable buffer, defines, URI, request arguments and
  analysis epoch. Snapshot replacement or clearing advances the epoch; servable
  module-graph changes notify explorer clients.
- Bounded caches must not pin retired native state; apply shadow-to-real
  presentation mapping after lookup.
- Lifecycle logs may carry bounded paths, counts, outcomes, timings and memory
  samples, never source buffers or unbounded LSP payloads.

## Pointers

- Safeguard rules: [src guide](../../AGENTS.md#safeguard-changes).
- Wire tests: [LSP acceptance](../../../tests/readme.md#lsp-acceptance).
- Platform: [cross-platform handling](../../../docs/cross_platform.md).
