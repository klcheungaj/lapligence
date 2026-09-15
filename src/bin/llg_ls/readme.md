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

## Source organization

`module_explorer.rs` coordinates `module_explorer/` domains for catalog,
resource budgets, contents, hierarchy, compatibility, presentation and types.
Its unit tests are in `module_explorer/tests.rs`. The feature-test facade and
its domain suites live in `features/tests.rs` and `features/tests/`.

See [the source map](../../../docs/source_layout.md).
