# Shared core

- Purpose: provide a safe, owned semantic model shared by simulation, linting,
  and language-server features.
- `compile.rs`: bounded Slang compilation and owned diagnostics, including
  cache-only admission of explicit named library sources and deterministic
  library-map expansion for configured elaboration.
- [`db/`](db/readme.md): validated semantic arena and type metadata.
- `value.rs` and `elab.rs`: exact four-state values and pure value operations.
- `model.rs`: hierarchy and editor-facing projections.
- `tokens.rs` and `macros.rs`: lexical bindings and preprocessor information.
- [`lint/`](lint/readme.md): shared design checks.

Native ownership ends in `ffi::slang`; core and all downstream consumers use
owned Rust data. Admitted source text stays in memory and is never recovered
by reopening frontend paths.

Library and configuration inputs follow the same boundary: `--libmap` and
`--libfile` paths are read and expanded by Rust, then named library buffers and
the selected default liblist are passed to the native bridge. Slang therefore
resolves `module:config`, cell and instance rules over admitted buffers without
performing implicit filesystem discovery.

`compile/library_mapping.rs` ranks all admitted map candidates before publishing
source-library identity: explicit filename, wildcarded filename, then trailing
separator directory patterns. Parent directory wildcards do not lower an explicit
filename's rank. Only unresolved cross-library ties at the final highest rank
fail; repeated same-library matches reuse one source. Explicit `--libfile` and
`LibrarySource` assignments override map candidates. Filesystem and logical map
inputs share the resolver, with bounded matching/cloning and one metadata charge
per newly mapped buffer. Original buffers and admitted path handles remain the
source of truth; ranking never grants a native filesystem read. Embedded map
configurations and per-library `-incdir` remain separately tracked policy work.

Strict source admission is centralized in `compile/editions.rs`, over owned
semantic records and classified lexical tokens. The table is shared by ordinary
and navigation compilations. Builtin and keyword admission is distinct from
simulator implementation. Names not in the requested edition require an explicit
`CompileOpts::system_subroutines` prototype; registering an extension does not
make it a standard builtin. Directive replacement text is excluded until used;
missing, skipped, string and escaped-identifier tokens are not treated as code.
Keep the table tests, located frontend tests and both compilation modes aligned
when updating edition capabilities.
