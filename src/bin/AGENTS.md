# Executables

Keep frontend-independent logic in the library. Bins use `llg::core`,
`llg::ffi`, and `llg::sim` imports; no `#[path]` includes or `unsafe`.
LSP-only tower-lsp/tokio/dashmap code stays in `llg_ls`.

| Binary | Role |
| --- | --- |
| `llg_ls` (`llg_ls/`) | tower-lsp stdio language server; [guide](llg_ls/AGENTS.md) |
| `llg` (`llg.rs`) | Simulator: compile → lower/IR/opt/emit → automatic CMake build → run |
| `elab_check` (`elab_check.rs`) | Verifies the owned Slang semantic database and resolved hierarchy |
| `helloslang`, `helloworld`, `llg_demo` | Owned Slang snapshot demonstrations |

## Simulator driver

- `--generator <backend>` selects CMake `-G`; `--gen-only` stops after model
  sources + `CMakeLists.txt`. CMake is the only model builder; see
  [../sim/AGENTS.md](../sim/AGENTS.md) for compiler/flags/environment selection.
- `--no-opt` disables simulator IR optimization passes; the default enables
  them. File-based conformance tests exercise both CLI modes.
- `--lint` runs the shared linter before codegen and exits 1 on lint errors.
  `--lint-config <path>` loads `llg-lint.toml` rule enable/severity settings.
- `--lint-json [<path>]` is report-only: one JSON object to stdout or file,
  exiting without codegen/simulation. It wins over `--lint`; see
  [../core/lint/AGENTS.md](../core/lint/AGENTS.md) for schema and exit behavior.
- Simulation builds one owned DB using the compilation's physical source-file
  inventory for bounded time-literal recovery and reuses it after lint.
  Report-only lint retains ordinary DB capture without new constant-source reads.

## Startup and process state

Both `llg` and `llg_ls` accept `--help`/`-h` and `--version`/`-V`.
These modes print to stdout and exit successfully before installing memory
guards, compiling, or serving. Version text uses `env!("CARGO_PKG_VERSION")`.

`llg_ls` and `helloworld` set mimalloc's `#[global_allocator]`; musl builds
also wrap C allocation for every binary in the root build script. Binaries may install
`llg::memory_limit::install[_with_logger]`; policy, defaults, native behavior,
and generated-child limits are in [../AGENTS.md](../AGENTS.md).
Platform calls stay in `ffi/process_memory.rs`. The LSP logger uses `LLG_LOG`
and `LLG_LOG_FILE`, never stdout (the framed JSON-RPC transport).
