# Lapligence agent guide

Lapligence (`llg`) shares a Rust core and vendored Slang v11.0 between:

- Simulator: source → Slang parse/compile/elaborate → owned `core::db` →
  semantic/execution IR → optimization → C11 → CMake-built runtime/libaco model.
- LSP: the same frontend → owned analysis → tower-lsp stdio diagnostics, tokens,
  hover, definition, symbols, completion, references, rename/prepareRename and
  read-only module explorer.

## Instruction map

Read the owning guide before editing a module or sibling facade such as
`features.rs` or `codegen.rs`. Keep detailed contracts with their owner and link
rather than duplicate. Use the [source map](docs/source_layout.md) to find domains.

| Area | Guide |
| --- | --- |
| Shared safeguards | [src/AGENTS.md](src/AGENTS.md) |
| Compile, DB, values and projections | [core](src/core/AGENTS.md) |
| Native ownership and platform calls | [ffi](src/ffi/AGENTS.md) |
| C ABI and C++ | [wrapper](src/wrapper/AGENTS.md) |
| Simulator pipeline, lowering, emitter and runtime | [sim](src/sim/AGENTS.md) |
| Drivers and allocators | [bin](src/bin/AGENTS.md) |
| LSP backend and features | [llg_ls](src/bin/llg_ls/AGENTS.md) |
| Lint API, configuration and rules | [lint](src/core/lint/AGENTS.md) |
| Tests, safeguards, CI and release | [tests](tests/AGENTS.md) |
| Human documentation | [docs](docs/AGENTS.md) |

## Architecture

- Prefer Rust; use C++ for performance, ABI stability or legacy needs. Cross only
  a documented C ABI: no C++ templates/exceptions, Rust generics or panics.
  Clarify uncertain ownership or boundary assumptions.
- Confine Rust `unsafe` to `src/ffi/`, with soundness explanations. The check
  `grep -rn "unsafe" src --include=*.rs | grep -v src/ffi` must be empty.
  Other modules use stable safe FFI APIs.
- `core::db::Db::from_slang` is the single owned semantic import. Model, lint,
  simulator and LSP consumers read owned data; extend capture instead of adding
  native AST traversals. Use frontend-neutral `core::value` with exact four-state
  and byte payloads. Safe APIs expose neither AST pointers nor borrowed native memory.
- Keep tower-lsp/tokio/dashmap in `llg_ls`, behind default-on `lsp` and the bin's
  `required-features = ["lsp"]`. `cargo build --lib --no-default-features` must
  work without them. Bins import `llg::core`, `llg::ffi` and `llg::sim`; no
  `#[path]` includes. Embed libaco only in generated-model runtime archives,
  never Rust binaries.

## Build and references

Root `build.rs` drives CMake for Slang, fmt and the C wrapper; even `cargo check`
may trigger a native build. Build `llg_ls`, `llg` or `elab_check` with
`cargo build --bin <name>`; demo bins are also available.

For parallel worktrees with an available executable tmpfs, use
`scripts/run-tests.sh --test-work-dir /build` (substitute the actual mount path;
append nextest filters as needed). This opt-in flow isolates scratch per
worktree/run, shares the runtime cache, and keeps Cargo/Slang builds on disk.
Do not share mutable Cargo/CMake build trees or delete another active run's
scratch. Follow [test storage and cleanup](tests/readme.md#parallel-worktrees).

The portable Rust patch preparer applies tracked `patches/slang/` and
`patches/libaco/` before consuming native sources. Keep documented upstream-base
vendor gitlinks; no project-specific submodule commits. Accept clean or fully
applied trees; reject partial/mismatched trees with actionable diagnostics.
Wrapper edits rebuild the shim; Slang/CMake/patch edits may rebuild the frontend.

`llg_ls` and `helloworld` use mimalloc as Rust global allocator. On musl Linux,
`build.rs` and `mimalloc_shim.c` wrap C `malloc`/`free` for every binary and use
static target archives. Preserve `ffi/slang.rs` native `#[link]` attributes for
wrapper, Slang and fmt linkage through the library.

The release matrix targets static-musl Linux x86_64/arm64, MSVC Windows
x86_64/arm64 and macOS arm64, with linkage audits. Configuration is not validated
support; keep platform evidence and generated-model limits in local
`persistence/platforms.md`. [Tests](tests/AGENTS.md) owns CI/release commands.
`vendor/libaco` documents coroutine/shared/save stacks; LRMs are in
`docs/specification/`. Local `persistence/ROADMAP.md` records remaining work.

## Working conventions

Follow [coding practices](docs/coding_practices.md), subject to architecture and
safety rules. Make conservative incremental changes, verify APIs/flags against
vendored headers, state uncertainty and clarify when correctness requires it.
Use the Cargo.toml edition, rustfmt defaults, `Result`/`Option` over panics and existing
`compile::Diag`/`elab::ElabError` types. Add error enums or explicit lifetimes
only as needed; prefer clarity over clever generics. Optimize only with evidence
and explained trade-offs; no unsolicited micro-optimizations. Follow the wrapper
C++ guide. Add Python comments/docstrings only when requested.

New nontrivial code needs tests; prefer Rust unit/integration and Rust-side FFI
coverage. Use exact in-memory sources for focused library tests and the test
guide's checked-in HDL contract for end-to-end simulation. Serialize CWD changes
and restore them. Use `compile_checked` for execution/elaboration; reserve raw
`compile` for partial-result/diagnostic tests. Follow test cleanup/regression rules.
Keep comments factual and focused on why; document public APIs and FFI ownership.
Update module READMEs when purpose, requirements or interactions change.

## Local records

Keep handoffs, plans, findings, run evidence and inter-session memory in ignored
`persistence/`; never stage, commit or force-add it. Consult it on resumption and
update decisions, evidence, limits, remaining work and moved references. Human
guides belong in `docs/`; module documentation belongs beside source.
