# Executables

- Purpose: keep argument parsing, transport wiring, and presentation thin;
  reusable processing lives in the library.

- `llg`: simulator driver and compile/lint/build/run presentation.
- `llg_ls`: feature-gated tower-lsp stdio server; see
  [llg_ls/readme.md](llg_ls/readme.md).
- `elab_check`: owned semantic-database and instance-tree checker.
- `helloslang`, `helloworld`, `llg_demo`: Slang snapshot demonstrations.

Both `llg` and `llg_ls` support `--help` (`-h`) and `--version` (`-V`).
The version is the package version from `Cargo.toml`. These commands print
to stdout and exit without compiling sources or starting the language server.

- Boundary: binaries use the shared core/FFI/simulator libraries; LSP-specific
  dependencies remain behind the `lsp` feature.
