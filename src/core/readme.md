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

Strict source admission is centralized in `compile/editions.rs`, over owned
semantic records and classified lexical tokens. The table is shared by ordinary
and navigation compilations. Builtin and keyword admission is distinct from
simulator implementation. Names not in the requested edition require an explicit
`CompileOpts::system_subroutines` prototype; registering an extension does not
make it a standard builtin. Directive replacement text is excluded until used;
missing, skipped, string and escaped-identifier tokens are not treated as code.
Keep the table tests, located frontend tests and both compilation modes aligned
when updating edition capabilities.
