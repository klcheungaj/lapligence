# Shared core

- Purpose: safe, shared Surelog/UHDM processing for the simulator and language server.
- Modules:
  - `compile.rs`: frontend sessions and diagnostics.
  - [`db/`](db/readme.md): validated, owned design capture.
  - `elab.rs` and `model.rs`: value evaluation and design projections.
  - `tokens.rs` and `macros.rs`: editor tokens and source-level bindings.
  - [`lint/`](lint/readme.md): shared design checks.
- Boundaries: use safe FFI APIs; downstream consumers read owned data.
- Consumers: [simulator](../sim/readme.md) and [language server](../bin/llg_ls/readme.md).
