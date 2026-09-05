# Executables

Keep frontend-independent logic in the library. Bins use `llg::core`,
`llg::ffi`, and `llg::sim` imports; no `#[path]` includes or `unsafe`.
LSP-only tower-lsp/tokio/dashmap code stays in `llg_ls`.

| Binary | Role |
| --- | --- |
| `llg_ls` (`llg_ls/`) | tower-lsp stdio language server; [guide](llg_ls/AGENTS.md) |
| `llg` (`llg.rs`) | Simulator: compile → lower/IR/opt/emit → automatic CMake build → run |
| `elab_check` (`elab_check.rs`) | Verifies instance tree, reference bindings, resolved parameters through core compile/elab and FFI |
| `hellouhdm`, `helloworld`, `llg_demo` | Raw-API demos |

## Simulator driver

- `--generator <backend>` selects CMake `-G`; `--gen-only` stops after model
  sources + `CMakeLists.txt`. CMake is the only model builder; see
  [../sim/AGENTS.md](../sim/AGENTS.md) for compiler/flags/environment selection.
- `--lint` runs the shared linter before codegen and exits 1 on lint errors.
  `--lint-config <path>` loads `llg-lint.toml` rule enable/severity settings.
- `--lint-json [<path>]` is report-only: one JSON object to stdout or file,
  exiting without codegen/simulation. It wins over `--lint`; see
  [../core/lint/AGENTS.md](../core/lint/AGENTS.md) for schema and exit behavior.

## Startup and process state

Binaries set mimalloc's `#[global_allocator]` at final link (see `llg_ls/main.rs`
and `helloworld.rs`). Both frontends may install
`llg::memory_limit::install[_with_logger]`; policy, defaults, native behavior,
and generated-child limits are in [../AGENTS.md](../AGENTS.md).
Platform calls stay in `ffi/process_memory.rs`. The LSP logger uses `LLG_LOG`
and `LLG_LOG_FILE`, never stdout (the framed JSON-RPC transport).
