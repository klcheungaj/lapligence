# Shared core

- Purpose: provide a safe, owned semantic model shared by simulation, linting,
  and language-server features.
- `compile.rs`: in-memory Slang compilation and owned diagnostics.
- [`db/`](db/readme.md): validated semantic arena and type metadata.
- `value.rs` and `elab.rs`: exact four-state values and pure value operations.
- `model.rs`: hierarchy and editor-facing projections.
- `tokens.rs` and `macros.rs`: lexical bindings and preprocessor information.
- [`lint/`](lint/readme.md): shared design checks.

Native ownership ends in `ffi::slang`; core and all downstream consumers use
owned Rust data. Admitted source text stays in memory and is never recovered
by reopening frontend paths.
