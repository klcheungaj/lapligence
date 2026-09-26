# Executables

Binaries keep CLI/transport/presentation thin and reuse the core, FFI and simulator
libraries. LSP dependencies remain behind `lsp`.

- `llg`: compile, lint, build and run simulations.
- [`llg_ls`](llg_ls/readme.md): feature-gated tower-lsp stdio server.
- `elab_check`: owned DB and instance-tree checker.
- `helloslang`, `helloworld`, `llg_demo`: Slang snapshot demos.

`llg`/`llg_ls` help (`--help`/`-h`) and version (`--version`/`-V`) print to stdout
and exit without compilation/serving. Versions come from Cargo.toml.
