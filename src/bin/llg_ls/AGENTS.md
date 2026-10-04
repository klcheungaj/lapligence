# Verilog/SystemVerilog language server

Tower-LSP stdio serves diagnostics, tokens, hover, definition, references, symbols,
completion, prepareRename/rename, inactive ranges, token dumps and a read-only
module explorer.

## Ownership and transport

[lsp](lsp/AGENTS.md) owns scheduling, admission, snapshots, diagnostics, watchers
and handlers; [features](features/AGENTS.md) owns analysis, bindings, indexes and
projections. Read [shared safeguards](../../AGENTS.md) and
[validation](../../../tests/AGENTS.md).

Keep LSP dependencies in this binary behind default-on `lsp`. Serving stdout is
JSON-RPC only; logs go to stderr/configured file. Help, version and token dumps
print ordinary text without serving. Invalid CLI arguments exit 2 with stderr usage.

## Frontend boundary

Root jobs snapshot every unit/literal include within budgets. One blocking closure
compiles those exact buffers and builds owned DB/indexes/`Analysis`. Request
handlers neither access native objects nor start project compilation.

Preserve Slang diagnostic provider, name/code, full message, primary range and
related locations. Core diagnostics classify admission/DB failures. Publish current
failures while serving navigation from the last-good root snapshot.

Open-document semantic tokens compile the exact admitted unsaved buffer in
isolation; syntax errors return an authoritative empty stream. Unopened documents
use committed tokens. Cache keys include applicable buffer, defines, URI, request
arguments and analysis epoch. Parameters/localparams use `property.readonly`;
data/net types, port directions, typedef names (declaration and every type use,
including casts and `$bits`/`type()` operands) and the `parameter`/`localparam`/
`specparam` keywords use `type`; module and program names (declaration and
instantiation, even when the definition is outside the compiled buffer) use
`class`, interfaces `interface`, packages `namespace`; control/module words and
`defparam` remain `keyword`.

## Source and serving rules

Each root has independent config/scheduling. Only `.v`/`.sv` are compilation units;
headers are include-only. Resolve literal includes beside the including file,
then configured source/include directories; canonical identities deduplicate cycles.
Native reads must never reach unmeasured paths.

Apply `analysis.max_file_bytes` and `analysis.max_total_input_bytes` before cloning,
staging, caching or compiling. Use max-plus-one closed reads and exact open UTF-8
byte lengths. Admission failure preserves the last valid snapshot.

Use trailing debounce and generation checks. Triggers during a run mark the root
dirty for one newest-input follow-up. Hold no backend lock during discovery, I/O
or compilation. Navigation, token dumps and explorer requests read committed owned
data; exact semantic bindings outrank scoped fallback, and ambiguity yields no
target. Snapshot replacement/clearing advances the epoch; servable module-graph
changes notify explorer clients.

Bound definition/hover/reference/isolated-token caches. Apply shadow-to-real
presentation mapping after lookup; cache values must not pin retired native state.

## Logging

`LLG_LOG`: `off`, `error`, `warn`, `info`, `debug`, `trace`.
`LLG_LOG_FILE`: append-only file with stderr fallback. Lifecycle logs may contain
bounded paths, counts, outcomes, timings and memory samples, never source buffers
or unbounded LSP payloads.
