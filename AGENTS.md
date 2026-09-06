# Lapligence agent guide

Lapligence (`llg`) implements a Verilog/SystemVerilog simulator and language
server over vendored Surelog v1.87 + UHDM. Both use the shared Rust core:

- Simulator: source → parse/compile/elaborate → UHDM → owned `core::db` →
  `sim::ir::IrModel` → optimization → C11 emission → CMake-built executable
  containing the runtime and libaco coroutines.
- LSP: the same frontend → owned analysis → diagnostics, semantic tokens,
  hover, definition, symbols, completion, references, rename/prepareRename,
  and the read-only module explorer, served over tower-lsp stdio.

## Instruction map

Read the relevant module guide before changing its code, including when editing
its sibling Rust facade (`features.rs`, `codegen.rs`, etc.). Keep detailed
contracts in their owning guide and link to them instead of copying them here.

| Area | Guide and responsibilities |
| --- | --- |
| Shared source policy | [src/AGENTS.md](src/AGENTS.md): process-memory guard and safeguard review rules |
| Shared core | [core](src/core/AGENTS.md): compile, owned DB/model, elaboration, tokens, macros |
| FFI | [ffi](src/ffi/AGENTS.md): sessions, ownership, UHDM/VPI field notes, platform memory primitives |
| C wrapper | [wrapper](src/wrapper/AGENTS.md): C ABI and C++ conventions |
| Simulator | [sim](src/sim/AGENTS.md): IR, optimization, emission, model builds; links to lowering and runtime guides |
| Executables | [bin](src/bin/AGENTS.md): driver modes, allocators, entry points |
| Language server | [llg_ls](src/bin/llg_ls/AGENTS.md): protocol, cache, logging, explorer; links to backend and feature guides |
| Linter | [lint](src/core/lint/AGENTS.md): API/configuration; links to rule contracts |
| Validation | [tests](tests/AGENTS.md): suite map, safeguards, CI gates and fixtures |
| Human documentation | [docs](docs/AGENTS.md): product documentation requirements |

## Architecture invariants

- Rust is preferred for new features; change C++ for performance, ABI stability,
  or legacy needs. Interoperation is C ABI only: no C++ templates/exceptions,
  Rust generics, or Rust panics cross it. Document ownership explicitly; clarify
  uncertain boundary assumptions rather than guessing.
- `unsafe` is confined to `src/ffi/`, with soundness explanations. Enforcement:
  `grep -rn "unsafe" src --include=*.rs | grep -v src/ffi` must be empty.
  Everyone else uses stable safe FFI APIs.
- `core::db::Db::build(design)` is the single elaborated-design VPI traversal
  into an owned arena. Model, lint, simulator and LSP analysis consumers read
  owned nodes; extend the DB instead of adding consumer VPI walks.
- Read values through `vpi::read_value` → owned `ValueData`; the raw
  `VpiValueData` union is read only inside `ffi/vpi.rs`.
- LSP-only dependencies (tower-lsp/tokio/dashmap) stay in `llg_ls`, behind the
  default-on `lsp` feature and the bin's `required-features = ["lsp"]`.
  `cargo build --lib --no-default-features` must work without them.
- Bins import `llg::core`, `llg::ffi`, and `llg::sim`; no `#[path]` includes.
  libaco sources are embedded and compiled with generated C models, never
  linked into Rust binaries.

## Build and references

Cargo's root `build.rs` drives CMake for Surelog/UHDM/ANTLR and the C wrapper.
Even `cargo check` can require a native build. The release workflow targets
static-musl Linux on x86_64/arm64, MSVC Windows on x86_64/arm64, and Apple
Silicon macOS, and audits their linkage contracts. Do not treat a configured
matrix leg as validated support; keep run evidence and generated-simulator
limitations in local `persistence/platforms.md`.

- Build with `cargo build --bin llg_ls`, `--bin llg`, or `--bin elab_check`;
  demo bins are also available.
- `llg_ls` and `helloworld` select mimalloc as their Rust global allocator. On
  musl Linux, `build.rs` and `mimalloc_shim.c` redirect C `malloc`/`free`
  through `--wrap` for every binary.
- Preserve the native `#[link]` attributes in `ffi/surelog.rs` and
  `ffi/vpi.rs`; they carry the wrapper and frontend archives through the Rust
  library boundary.
- Wrapper changes rebuild quickly; vendored Surelog/CMake changes trigger a
  full frontend rebuild.
- `vendor/libaco` documents `aco_create`, `aco_resume`, `aco_yield`, `aco_exit`,
  shared stacks and per-coroutine save stacks.
  Verilog/SystemVerilog LRMs are PDFs in `docs/specification/`.
- CI/release commands and caveats live in [tests/AGENTS.md](tests/AGENTS.md).
  `persistence/ROADMAP.md` is the remaining-work plan of record.

## Working conventions

Follow [docs/coding_practices.md](docs/coding_practices.md); project architecture
and safety rules take precedence. Prefer small, incremental changes. Verify
APIs/flags against vendored headers, be precise and conservative, state
uncertainty, and ask only when needed for correctness.

Use the Cargo.toml Rust edition, rustfmt defaults, idiomatic `Result`/`Option`
over panics, and existing error types (`surelog::Diag`, `elab::ElabError`). Add
error enums or explicit lifetimes only when needed; favor clarity over clever
generics. Do not optimize without evidence: explain expected benefit and
trade-offs; avoid unsolicited micro-optimizations. C++ conventions live in the
wrapper guide. Do not add Python comments/docstrings unless requested.

Prefer Rust unit/integration tests, including Rust-side FFI tests; new code
needs coverage unless clearly trivial. Surelog tests use a fresh temporary CWD
and serialized execution; execution/elaboration uses `compile_checked`, while
raw `compile` is reserved for partial-result/diagnostic tests. See the test
guide for cleanup and regression requirements.

Keep comments concise and factual (why, not what), public APIs documented,
FFI ownership explicit, and module `readme.md` purpose/requirements/interactions
accurate when responsibilities change.

## Agent persistence records

Store session handoffs, plans, investigation findings, validation evidence and
other inter-session memory in local, Git-ignored `persistence/`. Never stage,
commit or force-add these records. Consult relevant records on resumption and
update decisions, evidence, limitations and remaining work; repair references
when records move. Human guides belong in `docs/` and module documentation
beside source, not in agent memory.
