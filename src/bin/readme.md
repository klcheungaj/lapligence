# Executables

- Purpose: keep argument parsing, transport wiring, and presentation thin;
  reusable processing lives in the library.

- `llg`: simulator driver and compile/lint/build/run presentation.
- `llg_ls`: feature-gated tower-lsp stdio server; see
  [llg_ls/readme.md](llg_ls/readme.md).
- `elab_check`: elaboration and instance-tree checker.
- `hellouhdm`, `helloworld`, `llg_demo`: low-level API demonstrations.

- Boundary: binaries use the shared core/FFI/simulator libraries; LSP-specific
  dependencies remain behind the `lsp` feature.
