# Executables

Binaries keep CLI/transport/presentation thin and reuse the core, FFI and simulator
libraries. LSP dependencies remain behind `lsp`.

- `llg` (`llg/`): compile, lint, build and run simulations; unsupported on Windows
  on ARM (it exits 2 there; only `llg_ls` ships for that platform). `cli.rs` parses the
  command line, `settings.rs` loads the `--config` file (never discovered) and
  applies the precedence command line > environment > config file > default.
- [`llg_ls`](llg_ls/readme.md): feature-gated tower-lsp stdio server.
- `elab_check`: owned DB and instance-tree checker.
- `helloslang`, `helloworld`, `llg_demo`: Slang snapshot demos.

`llg` messages: every diagnostic of the driver and of the model it runs goes to
stderr in the layout of `src/sim/rt/llg_msg_layout.def` (rendered by
`llg::core::report` here and by `llg_msg.h` in the model) with an id from the
[message catalog](../../docs/messages.md). Stdout carries only requested output:
help, version, `--lint-json`, the `--gen-only`/`--dpi-import-lib` path and the
design's own output. Each phase that reported a warning or worse ends with a
`TOOL-000` summary before the model starts; the model prints `SIM-000` at its
exit. Exit codes: 2 usage, 1 failure, else the model's status.

`llg`/`llg_ls` help (`--help`/`-h`) and version (`--version`/`-V`) print to stdout
and exit without compilation/serving. Versions come from Cargo.toml.
