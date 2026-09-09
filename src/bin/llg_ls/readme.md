# `llg_ls`

- Purpose: Tower-LSP stdio server for owned Verilog/SystemVerilog analysis
  snapshots.
- Scope: manages independent workspace roots, schedules serialized analysis,
  stages read-only inputs, and presents navigation and diagnostics over
  JSON-RPC.

- `main.rs` selects the async runner and process setup.
- `transport.rs` owns framed stdio service construction and lifecycle hooks.
- `features.rs` projects owned analysis into navigation, symbols, tokens,
  hover, completion, references, rename, and explorer data.
- `lsp.rs` and `lsp/` own workspace state, scheduling, staging, diagnostics,
  configuration, and wire handlers.

- Boundary: the backend passes bounded, admitted source buffers to Slang and
  returns only owned snapshots and indexes to request handling. The transport
  keeps stdout exclusively for JSON-RPC.
