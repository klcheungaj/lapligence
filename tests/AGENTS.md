# Agent guide: tests

Repository validation: Rust unit/integration suites, Rust-side FFI probes,
checked-in HDL fixtures, LSP wire tests and CI. Methodology, suite contracts and
commands live in [readme.md](readme.md); CI rules in [ci.md](ci.md).

## Layout

- `tests/<suite>.rs`: a module of a grouped test binary (`autotests = false`).
  Register it with `mod <suite>;` in its group root
  ([test binaries](readme.md#test-binaries)); `test_layout` enforces this.
- `support/`: shared harnesses (`sim_harness`, `sim_cli`, `c_compiler`), declared
  once per group root and imported with `use crate::...`, never a `#[path]` copy.
- `fixtures/sim/<area>/`: checked-in HDL with a `readme.md` holding its
  contracts; `fixtures/sim/feature_completion/<task_slug>/` for new tasks.
- `fixtures/lsp/`: LSP manifests (`test.json`) and per-root `llg.toml`.
- `runtime_value_storage/`: native runtime probes and `validate.py`.

## Rules

- Prefer deterministic Rust unit/integration tests and Rust-side FFI probes.
  New nontrivial code needs tests.
- End-to-end simulation passes checked-in `.v`/`.sv` files directly to `llg`; never
  embed or generate those designs in Rust. Rust owns independent expected results.
- Focused frontend/DB/IR tests may use exact in-memory sources.
- Use `compile_checked` for successful execution/elaboration; raw `compile` only
  for partial snapshots/diagnostics, including proof that checked compilation
  withholds blocking-error snapshots.
- Serialize parent CWD changes with a fresh temp directory and unwind-safe
  restoration; isolate child CWDs.
- Run public fixtures with default optimization and `--no-opt`. Missing tools
  block acceptance; they are never a skip or a pass, except the documented
  `sim_cmake` and native-DPI skips ([suite contracts](readme.md#suite-contracts)).
- Expected-output changes need a feature/clause justification; never bless them
  automatically.
- Label parser/DB/IR/handwritten-C tests `component_*`; they are not
  generated-model acceptance.
- `generated_c_frame_lint` stays in the default suite; make it faster, never
  move it out.
- A new LSP/frontend-only `general` suite joins the `ci-lsp` filter; never add
  simulator-only tests to it ([Windows on ARM](ci.md#windows-on-arm)).
- Never commit the applied `vendor/libfst` state
  ([vendor patch preparation](readme.md#vendor-patch-preparation)).
- Successful runs remove their scratch; remove only inactive runs your task owns
  and prune the shared runtime cache only when all users have stopped.
- Keep logs and run evidence in ignored `persistence/`.

## Commands

```sh
scripts/run-tests.sh --test-work-dir /build --cargo-profile quick --test <suite>
python3 scripts/check_sim_fixture_integrity.py --tracked
```

- Before publishing new fixtures:
  [fixture integrity](readme.md#fixture-integrity-before-a-native-build).
- Parallel worktrees: [storage and cleanup](readme.md#parallel-worktrees).
- Profiles: [Cargo build profiles](readme.md#cargo-build-profiles); the
  optimized default is the [repository gate](readme.md#repository-gate), never
  fmt/check/clippy alone.

## Pointers

- New simulator features: [feature completion checklist](readme.md#feature-completion-checklist).
- LSP wire contracts: [LSP acceptance](readme.md#lsp-acceptance).
- `llg.toml` coverage by layer: [configuration](readme.md#llgtoml-configuration).
- Memory and admission safeguards: [safeguards](readme.md#safeguards).
- Platform pitfalls in tests: [cross-platform handling](../docs/cross_platform.md).
- Feature status: only [sim_features.md](../docs/sim_features.md).
