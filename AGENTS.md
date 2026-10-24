# Agent guide for lapligence

Lapligence (`llg`) shares a Rust core and vendored Slang v11.0 between a
simulator (source → Slang → owned `core::db` → semantic/execution IR →
optimization → C11 → CMake-built stackless model) and the `llg_ls` language
server (the same frontend → owned analysis → tower-lsp stdio features).

## Agentic file budget

- Root guide: under about 80 lines. Subtree guides: under about 100 (aim for
  40-80): local ownership, rules, pitfalls, commands and pointers.
- Each rule lives in one place. Contracts, procedures, platform notes and
  examples go to module readmes or docs ([placement](docs/AGENTS.md)); guides
  link to them and never restate a source comment.

## Universal work rules

- Before editing a file, read the nearest `AGENTS.md` and each parent guide up
  to this one.
- Follow [coding practices](docs/coding_practices.md). Make conservative,
  incremental changes; verify APIs and flags against vendored headers; state
  uncertainty and ask when correctness depends on it.
- Use the Cargo.toml edition, rustfmt defaults, `Result`/`Option` over panics and
  the existing `compile::Diag`/`elab::ElabError` types. Add error enums or
  explicit lifetimes only as needed; prefer clarity over clever generics. Add
  Python comments or docstrings only when asked.
- Speed and memory come first, but optimize only with evidence and explained
  trade-offs; no unsolicited micro-optimizations.
- New nontrivial code needs tests ([test guide](tests/AGENTS.md)).
- Comments are factual and say why; document public APIs and FFI ownership.
  Update a module readme when its purpose or interactions change.
- Keep handoffs, plans, findings and run evidence in ignored `persistence/`;
  never stage, commit or force-add it.

## Architecture rules

- Prefer Rust; use C++ only for performance, ABI stability or legacy needs.
  Cross only a documented C ABI: no C++ templates or exceptions, no Rust
  generics or panics.
- Rust `unsafe` lives only in `src/ffi/`, with soundness explanations:
  `grep -rn "unsafe" src --include=*.rs | grep -v src/ffi` must be empty.
- `core::db::Db::from_slang` is the single owned semantic import. Consumers read
  owned data; extend capture instead of adding native AST traversals. Safe APIs
  expose neither AST pointers nor borrowed native memory.
- tower-lsp, tokio and dashmap stay in `llg_ls` behind default-on `lsp`
  (`required-features = ["lsp"]`); `cargo build --lib --no-default-features`
  must work.
- The C11 runtime, including `llg_co`, ships only with generated models, never
  linked into Rust binaries.
- Windows on ARM ships `llg_ls` only; `llg` is unsupported there.

## Build

- Root `build.rs` drives CMake for Slang, fmt and the wrapper; even `cargo check`
  may trigger a native build. Build one binary with `cargo build --bin <name>`.
- Vendored Slang: the Rust patch preparer applies tracked `patches/slang/` to the
  documented upstream-base gitlink; no project-specific submodule commits
  ([patch preparation](patches/README.md)).
- Accelerators (opt-in, never required): from the repository root,
  `export LLG_SCCACHE=1 LLG_MOLD=1; source scripts/dev-env.sh`, omitting
  unavailable tools; mold only on Linux GNU hosts
  ([accelerators](tests/readme.md#optional-development-accelerators)).
- Parallel worktrees: `scripts/run-tests.sh --test-work-dir /build` (the actual
  tmpfs mount); never share mutable Cargo/CMake trees or delete another run's
  scratch ([storage and cleanup](tests/readme.md#parallel-worktrees)).

## Project map

- [src/AGENTS.md](src/AGENTS.md): library crate, platform code, safeguards.
  - [core](src/core/AGENTS.md), [lint](src/core/lint/AGENTS.md),
    [ffi](src/ffi/AGENTS.md), [wrapper](src/wrapper/AGENTS.md),
    [sim](src/sim/AGENTS.md), [bin](src/bin/AGENTS.md),
    [llg_ls](src/bin/llg_ls/AGENTS.md), [config](src/config/readme.md).
- [tests/AGENTS.md](tests/AGENTS.md): tests, CI and release.
- [docs/AGENTS.md](docs/AGENTS.md): documentation placement.
- Other: `patches/`, `perf/`, `scripts/`, `vendor/`, `build_support/`.

## Source-of-truth docs

- [Source layout](docs/source_layout.md) (ownership, doc index),
  [cross-platform](docs/cross_platform.md), [CI](tests/ci.md),
  [configuration](docs/config.md), [feature status](docs/sim_features.md).
- Local only: LRMs in `docs/specification/`, `persistence/ROADMAP.md`.
