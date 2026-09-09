# Verilog/SystemVerilog language server

This guide covers the Tower-LSP stdio server. The server provides diagnostics,
semantic tokens, hover, definition, references, symbols, completion,
prepareRename/rename, inactive ranges, token dumps, and the read-only module
explorer.

## Ownership

- [lsp/AGENTS.md](lsp/AGENTS.md) owns backend scheduling, source admission,
  root snapshots, diagnostics, watchers, and wire handlers.
- [features/AGENTS.md](features/AGENTS.md) owns analysis, exact bindings, the
  symbol index, and feature projections.
- Shared memory safeguards are in [../../AGENTS.md](../../AGENTS.md), and
  validation contracts are in [../../../tests/AGENTS.md](../../../tests/AGENTS.md).
- LSP dependencies stay in this binary behind the default-on `lsp` feature.
  Keep stdout exclusively for JSON-RPC framing and send logs to stderr or a
  configured file.

## Frontend boundary

Root jobs take bounded snapshots of every compilation unit and literal include.
The blocking analysis closure passes those exact buffers to Slang, builds the
owned semantic database and indexes, and returns an owned `Analysis`. Request
handlers never access native objects or start project compilation.

Slang diagnostics retain their provider, name/code, full message, primary
range, and related locations. Core diagnostics represent admission and database
failures. Failed compilation publishes current diagnostics while navigation
continues from the root's last-good snapshot.

Open-document semantic token requests compile the exact admitted unsaved buffer
in isolation. A syntax error produces an authoritative empty token stream.
Unopened documents use committed project tokens. Cache keys include the buffer,
effective defines, URI, request arguments, and analysis epoch as appropriate.

## Source identity and limits

Each workspace root has independent config and scheduling. Only `.v` and
`.sv` files are compilation units; headers enter through admitted includes.
Resolve literal includes beside the including file, then through configured
source and include directories. Canonical identities deduplicate cycles.
Never allow the native frontend to read an unmeasured path.

Apply `analysis.max_file_bytes` and `analysis.max_total_input_bytes` before
cloning, staging, cache insertion, or compilation. Closed files use max-plus-one
bounded reads. Open UTF-8 text is measured by its actual byte length. Admission
failure preserves the previous valid snapshot.

## Serving and cache behavior

Root scheduling uses a trailing debounce and generation checks. Triggers during
a run mark the root dirty and schedule one follow-up with the newest inputs.
No backend state lock may be held across discovery, I/O, or compilation.

Definition, references, rename, hover, symbols, completion, token dumps, and
module explorer requests read committed owned data. Exact Slang semantic
bindings take precedence over scoped name fallback. Ambiguous bindings yield no
target. Any snapshot replacement or clearing advances the analysis epoch;
servable module graph changes notify module explorer clients.

The bounded request caches store definition, hover, references, and isolated
semantic token results. Apply shadow-to-real presentation mapping after cache
lookup. Cache values must not pin retired native state.

## Logging

`LLG_LOG` accepts `off`, `error`, `warn`, `info`, `debug`, or
`trace`; `LLG_LOG_FILE` selects an append-only file with stderr fallback.
Lifecycle records may contain bounded paths, counts, outcomes, timing, and
memory samples. Never log source buffers or unbounded LSP payloads.
