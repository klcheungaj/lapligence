# Lapligence agent guide

Lapligence (`llg`) is a Verilog/SystemVerilog simulator and language server using vendored Slang
v11.0 and a shared Rust core:

- Simulator: source → Slang parse/compile/elaborate → owned `core::db` → semantic/execution IR →
  optimization → C11 emission → CMake-built executable containing the runtime and libaco
  coroutines.
- LSP: the same frontend → owned analysis → diagnostics, semantic tokens, hover, definition,
  symbols, completion, references, rename/prepareRename, and the read-only module explorer,
  served over tower-lsp stdio.

## Instruction map

Before editing a module or its sibling facade (`features.rs`, `codegen.rs`, etc.), read its
guide. Keep detailed contracts with their owner; link rather than duplicate. The [source
map](docs/source_layout.md) locates implementation domains.

| Area | Guide and responsibilities |
| --- | --- |
| Shared source policy | [src/AGENTS.md](src/AGENTS.md): process-memory guard and safeguard review rules |
| Shared core | [core](src/core/AGENTS.md): compile, owned DB/model, elaboration, tokens, macros |
| FFI | [ffi](src/ffi/AGENTS.md): snapshot ownership, checked decoding, platform memory primitives |
| C wrapper | [wrapper](src/wrapper/AGENTS.md): C ABI and C++ conventions |
| Simulator | [sim](src/sim/AGENTS.md): IR, optimization, emission, model builds; links to lowering and runtime guides |
| Executables | [bin](src/bin/AGENTS.md): driver modes, allocators, entry points |
| Language server | [llg_ls](src/bin/llg_ls/AGENTS.md): protocol, cache, logging, explorer; links to backend and feature guides |
| Linter | [lint](src/core/lint/AGENTS.md): API/configuration; links to rule contracts |
| Validation | [tests](tests/AGENTS.md): suite map, safeguards, CI gates and fixtures |
| Human documentation | [docs](docs/AGENTS.md): product documentation requirements |

## Architecture invariants

- Rust is preferred for new features; change C++ for performance, ABI stability, or legacy
  needs. Interoperation is C ABI only: no C++ templates/exceptions, Rust generics, or Rust
  panics cross it. Document ownership explicitly; clarify uncertain boundary assumptions rather
  than guessing.
- `unsafe` is confined to `src/ffi/`, with soundness explanations. Enforcement: `grep -rn
  "unsafe" src --include=*.rs | grep -v src/ffi` must be empty. Everyone else uses stable safe
  FFI APIs.
- `core::db::Db::from_slang` is the single import of the owned Slang semantic graph into the
  validated database. Model, lint, simulator and LSP consumers read owned data; extend capture
  instead of adding native AST traversals.
- Values use frontend-neutral `core::value` and exact four-state/byte payloads. Slang AST
  pointers and borrowed native memory never cross the safe FFI API.
- LSP-only dependencies (tower-lsp/tokio/dashmap) stay in `llg_ls`, behind the default-on `lsp`
  feature and the bin's `required-features = ["lsp"]`. `cargo build --lib --no-default-features`
  must work without them.
- Bins import `llg::core`, `llg::ffi`, and `llg::sim`; no `#[path]` includes. libaco sources are
  embedded and compiled with generated C models, never linked into Rust binaries.

## Build and references

Cargo's root `build.rs` drives CMake for Slang, fmt, and the C ABI wrapper. Even `cargo check`
can require a native build. The release workflow targets static-musl Linux on x86_64/arm64, MSVC
Windows on x86_64/arm64, and Apple Silicon macOS, and audits their linkage contracts. Do not
treat a configured matrix leg as validated support; keep run evidence and generated-simulator
limitations in local `persistence/platforms.md`.

The root build script applies the tracked patches under `patches/slang/` and `patches/libaco/`
with its portable Rust patch preparer before native sources are consumed. Keep `vendor/slang`
and `vendor/libaco` at their documented base gitlinks; project-specific commits in either
submodule are forbidden. A clean checkout is patched, an already-applied checkout is accepted,
and a partial or mismatched checkout fails with an actionable diagnostic.

- Build with `cargo build --bin llg_ls`, `--bin llg`, or `--bin elab_check`; demo bins are also
  available.
- `llg_ls` and `helloworld` select mimalloc as their Rust global allocator. On musl Linux,
  `build.rs` and `mimalloc_shim.c` redirect C `malloc`/`free` through `--wrap` for every binary.
- Preserve the native `#[link]` attributes in `ffi/slang.rs`; they carry the wrapper, Slang, and
  fmt archives through the Rust library boundary.
- Wrapper changes rebuild the shim; vendored Slang/CMake changes and tracked vendor patches can
  rebuild the frontend. Musl uses static target archives and the shared allocator shim.
- `vendor/libaco` documents `aco_create`, `aco_resume`, `aco_yield`, `aco_exit`, shared stacks
  and per-coroutine save stacks. Verilog/SystemVerilog LRMs are PDFs in `docs/specification/`.
- CI/release commands and caveats live in [tests/AGENTS.md](tests/AGENTS.md).
  `persistence/ROADMAP.md` is the remaining-work plan of record.

## Working conventions

Follow [docs/coding_practices.md](docs/coding_practices.md); architecture and safety rules take
precedence. Be precise and conservative. Make incremental changes, verify APIs/flags against
vendored headers, state uncertainty, and clarify only for correctness.

Use the Cargo.toml edition, rustfmt defaults, `Result`/`Option` over panics, and existing errors
(`compile::Diag`, `elab::ElabError`). Add error enums or explicit lifetimes only as needed;
prefer clarity to clever generics. Require evidence for optimization and explain
benefits/trade-offs; no unsolicited micro-optimizations. Follow the wrapper's C++ guide. Add
Python comments or docstrings only when requested.

Prefer Rust unit/integration tests, including Rust-side FFI tests; new code needs coverage
unless clearly trivial. Prefer exact in-memory source fixtures. Tests mutating the process CWD
must restore it and use serialized execution; execution/elaboration uses `compile_checked`,
while raw `compile` is reserved for partial-result/diagnostic tests. See the test guide for
cleanup and regression requirements.

Keep comments factual and focused on why, document public APIs and FFI ownership, and update
module `readme.md` purpose/requirements/interactions when responsibilities change.

## Agent persistence records

Keep handoffs, plans, findings, validation evidence, and inter-session memory in local
Git-ignored `persistence/`; never stage, commit, or force-add it. Consult it on resumption;
update decisions, evidence, limitations, remaining work, and moved references. Human guides
belong in `docs/`, module docs beside source, not in agent memory.
