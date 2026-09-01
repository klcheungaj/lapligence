# bin — executables

## Purpose

Thin entry points over the `llg` library; all logic lives in the lib.

| Binary | Source | Role |
|---|---|---|
| `llg_ls` | `llg_ls/` | Verilog/SV Language Server (tower-lsp over stdio); installs the optional process-memory guard (`LLG_MEMORY_LIMIT_MB`) |
| `llg` | `llg.rs` | Simulator driver: compile → codegen (lowering → IR → optimize → emit) → automatic CMake build (the only model builder; `--generator <backend>` selects the cmake `-G` backend, `--gen-only` stops after emitting sources + `CMakeLists.txt`) → run (`--lint` runs the shared linter before codegen; `--lint-json [<path>]` is a report-only mode that emits the lint report as one JSON object to stdout or a file and exits without simulating; `--lint-config <path>` loads a `llg-lint.toml` to enable/disable rules and override severities); installs the optional process-memory guard |
| `elab_check` | `elab_check.rs` | Elaboration verifier (instance tree, ref binding, resolved params) |
| `hellouhdm` / `helloworld` / `llg_demo` | — | Raw-API demos |

## Requirements

- Consume the lib via `use llg::core::…` / `use llg::ffi::…` /
  `use llg::sim::…` — **no `#[path]` module includes**.
- `unsafe` is only permitted in `src/ffi/`; binaries need none.
- The `llg_ls` binary is the **only** place tower-lsp/tokio/dashmap are used.
- Binaries set the mimalloc `#[global_allocator]` at their final link point
  (currently `llg_ls` and `helloworld`; see `src/bin/llg_ls/main.rs`).
- The LSP logger is configurable through `LLG_LOG` and `LLG_LOG_FILE`; it
  writes only to stderr or a file, never stdout, because stdout carries LSP
  JSON-RPC frames.
- Both frontends may install the shared process-memory guard
  (`llg::memory_limit::install[_with_logger]`); its unsafe platform sampler
  lives in `src/ffi/process_memory.rs`, not in the binaries.

## Interactions

- `llg_ls` → `llg::core` (compile, db, model, tokens) + `llg::ffi`.
- `llg` → `llg::core::compile` + `llg::sim` (codegen, rt).
- `elab_check` → `llg::core::compile` + `llg::ffi` (+ `llg::core::elab`).
