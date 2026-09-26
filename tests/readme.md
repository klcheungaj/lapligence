# Tests

## Scope and layout

- Shared Slang frontend, owned semantic database, simulator and LSP integration.
- [Shared harnesses](support/readme.md): temporary directories, cleanup and timeouts.
- [LSP fixtures](fixtures/lsp/): framed stdio tests, manifests and source headers.
- [Simulation feature status](../docs/sim_features.md): the sole support checklist.

## Simulator testing methodology

- Target IEEE 1364-2001/SystemVerilog-2009 using tracked clause/production references.
  The local [specification pack](../docs/specification/) is optional and untracked;
  its PDFs/maps are not needed for checkers or coverage audits.
- Pass checked-in HDL directly to `llg`, with default optimization and `--no-opt`.
  Rust owns independent truth-table, bit-string and width/sign oracles. Compare
  exact stdout, expected stderr/diagnostics and status; unexpected lowering
  warnings fail. Keep syntax/unsupported-context tests separate from positives.
- Isolate child CWDs and serialize/unwind-restore parent CWD changes. Focused
  frontend/DB/IR tests may use in-memory sources. Distinguish source recognition,
  typed-IR-to-C execution, full HDL execution and handwritten C components.
- Generated-C sanitizers instrument the model/runtime, not vendored Slang. A test
  source or configured CI job is not evidence of execution or platform support.

### Fixture integrity before a native build

Stage new fixtures before the tracked check; missing/unindexed inputs are errors,
not reasons to skip. The checker recognizes known static harness forms, not all
possible Rust expressions.

```sh
python3 scripts/check_sim_fixture_integrity.py --tracked
python3 -m unittest discover -s scripts -p test_sim_fixture_integrity.py
```

### Vendor patch preparation

`vendor_patches.rs` checks clean/applied trees, no-Git archives (including archives
inside another checkout), authenticated active/retired manifests, LF digest/CRLF
rendering, and symlink/reparse/hardlink/stale/untracked-input rejection. Race probes
cover ancestor/parent replacement and late staging hardlinks.

```sh
cargo test --locked --test vendor_patches -- --test-threads=1
```

## Coverage

Detailed fixture maps retain local oracle and rejection boundaries; common CLI
requirements above apply without repeating them for each suite.

| Area | Suites / fixture contracts |
| --- | --- |
| Frontend and owned models | `slang_frontend`, `slang_semantics`, `model_tests`: safe capture, diagnostics, types, bindings, initialization, complete delays and ownership after snapshot destruction. |
| Datatypes | `sim_data_types`, `sim_data_types_extended`, `sim_data_type_edges`, `sim_data_types_next`, `sim_data_types_completion`; [mixed-type/net matrix](fixtures/sim/type_conformance/readme.md) for independent arithmetic, conversion and resolution oracles. |
| Ports and nets | `sim_port_net_types`, `sim_net_resolution`, `sim_net_defaults`, `sim_net_decl`, `sim_inout`: directional collapse, independent drivers, strengths/defaults, aliases, selections and delayed publication. Port-type unit tests cover all 81 cells; frontend tests cover 49 resolved pairs. |
| Practical RTL | [RTL composition](fixtures/sim/rtl_completion/readme.md), `sim_rtl_completion`: initialization, fixed values/references/unions, ports and array/interface/inout composition. |
| Arrays and projections | `sim_fixed_array_reductions`, `sim_syn026_iterator_indices`, `sim_array_conditional_assignments`, `sim_fixed_ordering_review`, `sim_syn027_fixed_reverse`, `sim_group1_repairs`, `sim_group1_formal_repairs`: [reductions](fixtures/sim/fixed_array_reductions/readme.md), [iterator indices](fixtures/sim/syn026_iterator_indices/readme.md), [conditional assignments](fixtures/sim/array_conditional_assignments/readme.md), [fixed reverse](fixtures/sim/syn027_fixed_reverse/readme.md), activation isolation, signed member conversion, captured outputs and const/NBA negatives. |
| Expressions | `sim_fill_literals`, `sim_wildcard_eq`, `sim_bit_queries`, `sim_packed_strings`, `sim_sequential_predicates`, `sim_syn022_primitive_patterns`: fills, wildcard equality/inside, queries, [ordered predicates/patterns](fixtures/sim/sequential_predicates/readme.md) and [primitive pattern scope/edition checks](fixtures/sim/syn022_primitive_patterns/readme.md). |
| Processes and loops | [Always-family](fixtures/sim/process_semantics/readme.md), [loops](fixtures/sim/loops/readme.md), `sim_loops`, `sim_syn014_process_contexts`: source-loop exits, omissions/bounds, aggregate sensitivity, legal disjoint writers and illegal writer/event controls. |
| Scheduling and synchronization | `sim_process_control`, `sim_semaphore`, `sim_mailboxes`, `sim_procedural_assign`, `sim_force`; [processes](fixtures/sim/process_control/readme.md), [semaphores](fixtures/sim/semaphore/readme.md), [mailboxes](fixtures/sim/mailboxes/readme.md), [nonconvergence](fixtures/sim/nonconvergence/readme.md). |
| Time and numeric services | `sim_delay`, `sim_time_literals`, `sim_time_values`, `sim_physical_time`, `sim_real`, `sim_real_conversions`, `sim_random`, `sim_random_streams`: exact units/rounding, overflow, real precision, Annex N vectors and child-stream isolation. |
| I/O and observation | `sim_file_io`, `sim_memory_editions`, `sim_memory_views`, `sim_plusargs`, `sim_waveform`, `sim_vpi`; [file I/O](fixtures/sim/file_io/readme.md), [waveforms](fixtures/sim/waveform/readme.md), [partial features](fixtures/sim/partial_features/readme.md). |
| Verification and native objects | `sim_classes`, `sim_virtual_interfaces`, `sim_dpi`, [concurrent assertions](fixtures/sim/concurrent_assertions/readme.md): nominal identity, dispatch/casts, interface bindings, scalar native ABI, sampled/sequence/property behavior and explicit rejection boundaries. |
| Build and safeguards | `sim_cmake`, `cli_info`, `sim_memory_guard`, `support_harness`, `emit_decoupling`: CMake/generator failures, source cleanup, early CLI exits, memory limits, timeout trees, CWD recovery and frontend-independent emission. |
| Native components | `runtime_values`, `runtime_random`, `runtime_file_io`, `runtime_boundaries`, `runtime_value_storage`: direct runtime probes, independent of HDL lowering. |
| Integrated selected profile | `sim_syn038_ledger`, pairwise suites and `sim_syn039_acceptance`; [ledger](syn038_coverage_ledger.md), [integrated fixtures](fixtures/sim/syn039_acceptance/readme.md). |
| Compiler directives | `sim_directive_effects`, `sim_syn017_directive_effects`, `sim_edition` and `sim_syn038_ledger`; [SYN-017 matrix](fixtures/sim/syn017_directives/readme.md) covers both editions, both optimizer modes, preprocessing into execution, unit state and strict older-edition gates. |

### Selected-profile qualifications

- [SYN-012 fixed layouts](fixtures/sim/syn012_fixed_layout/readme.md) spans widths
  1/7/8/31/32/33/64/65/129, nested records/unions, state conversion and nominal
  rejection controls. [SYN-013 calls](fixtures/sim/syn013_zero_time_calls/readme.md)
  preserves static output state versus automatic defaults/inout copy-in.
- [SYN-014 processes](fixtures/sim/syn014_process_contexts/readme.md) checks an
  unselected aggregate arm as a dependency, no notification for unchanged results,
  and repaired pattern/array/record writer contexts. `sim_syn015_stream_contexts`
  checks fixed casts, ref projections, storage-order selectors, alignment and
  runtime oversize rejection; native preflight checks do not replace this suite.
- [SYN-016 elaboration](fixtures/sim/syn016_elaboration/readme.md),
  [SYN-017 directives](fixtures/sim/syn017_directives/readme.md),
  [SYN-018 modules](fixtures/sim/syn018_module_declarations/readme.md) and `sim_edition`
  retain strict edition gates, macros/keyword regions, legal defaults/types,
  namespace, enclosing-instance specializations and actual separate/merged-file
  boundaries.
- [SYN-021 tagged unions](fixtures/sim/syn021_tagged_union/readme.md) qualifies
  finite packed struct payloads through constructor, value, formal, static local,
  ref and stable-tag NBA contexts in 2009, with 2001 and invalid-constructor
  rejections. It also checks nested wrong-tag access and issue-time NBA targets.
  `sim_tagged_union_access` retains guard-owner stress; `sim_data_types_next`
  retains void, nested and unsupported-unpacked controls. A valid target
  retagged before commit is Q03-open.
- [SYN-032 configurations](fixtures/sim/syn032_library_configs/readme.md) and
  [SYN-033 bind](fixtures/sim/syn033_structural_bind/readme.md) keep bounded library,
  configuration and structural composition separate from arbitrary verification
  injection. Library-map tests cover final-rank ties, explicit overrides, original
  source locations, bounded logical/disk buffers and snapshot lifetime.
- [SYN-036 capacity](fixtures/sim/syn036_capacity/readme.md) separates syntax from
  resource limits. Test 65,536/1,048,575-bit values and rejection at 1,048,576;
  fixed-array cell limits are distinct from flattened-value limits. Two-state net
  illegality is distinct from conversion into two-state storage. Registry growth
  probes must exceed retired ceilings without silently changing HDL semantics.
- SYN-037 [finite-loop fixtures](fixtures/sim/loops/readme.md) include function
  step lists, ordered copy-out, continue versus break/return, named disable,
  shadowing and repeated wide-owner activations. Task-step rejection stays separate.

### Review and composition regressions

- [Held-out bundle](fixtures/sim/review_bundle/readme.md): runtime-failure witnesses
  require runtime diagnostics, not frontend/lowering rejection. R03 checks callback
  policy; R10 paired-edition packed conditional policy; R11 generation-only capacity.
  `sim_review_bundle_patterns` covers duplicate/unused type keys, recursive matching,
  constant index expressions and positional deconstruction. R13 composition crosses
  recursive patterns/functions/ports and selected NBA destinations.
- `sim_review_next4` covers concat signedness/fills, tagged reads, helper-flow proofs
  and ordering. Unit filters `singleton_concat`, `return_flow::tests`,
  `arguments::tests::frozen_activation_receivers` and `owned::tests::tagged_signed`
  pin typed rewrites, lexical exits, captured targets and real emitted-C ownership.
- `sim_review_batch2`, `sim_review_batch3`, `sim_review_batch4` retain program-origin
  completion, pure alias observation, mailbox mismatch retention, assertion counts,
  Observed clocking events, constructor/receiver ordering, nonpacked initialization,
  scanner delimiters, FD/MCD separation, detached queue refs, empty repetition,
  tied/nested first-match, physical-time multiclock endpoints and selected-prefix
  writers. Process/semaphore/VPI/DPI suites cover ancestor cancellation, cancellation
  batch FIFO, call-handle lifetime and aliased foreign string snapshots.
- Fixed-array tests preserve first-element Z folds, mapped types/declared indices,
  nested row/record views, bounded ownership, direct procedural conditional RHSs,
  overlapping snapshots, and shape errors despite equal bit counts. Pattern tests
  retain repeated expression IDs, typed versus untyped defaults and branch scopes.
  The packed conditional matrix retains its independent 64-cell policy oracle;
  casez/casex tests cover all four-state combinations rather than implementation-derived
  expected results.
- [Imported acceptance probes](fixtures/sim/imported_probes/acceptance/README.md)
  and [counterexamples](fixtures/sim/imported_probes/counterexamples/README.md)
  retain original sources/expectations and their active-suite mapping. C fragments
  without registration are not standalone plugins. Replacement missing fixtures
  are not recovered originals or historical pass evidence.

## Coverage metadata

The [grammar/context ledger](syn038_coverage_ledger.md) and
[compact pairwise source](syn038_pairwise.json) map selected language
contexts to fixtures, test owners and expected results.
`sim_syn038_ledger` validates the ledger and runs selected public-CLI
witnesses.

Check pairwise metadata from the repository root:

```sh
python3 scripts/check_syn038_pairwise_manifest.py --gaps-by-block
```

Metadata coverage is not proof of HDL correctness or complete language
support; the corresponding execution tests must also pass.

## Running tests

### Prerequisites

- Pinned Rust 1.98.0, initialized vendors, and
  `cargo install cargo-nextest --locked`.
- CMake/C compiler for public conformance; run commands at repository root.
- Runtime cache defaults to `target/llg-runtime-cache`; relative
  `LLG_RUNTIME_CACHE_DIR` overrides resolve from the root.
- Nextest defaults to 8 concurrent tests; `--profile max-threads` opts into 32.
  Preserve suite-specific skips: `sim_cmake` may skip absent CMake; native DPI
  shared-library tests need Unix/CMake/compiler. These are not blanket skips.

### Focused simulator suites

```sh
cargo nextest run --locked --test sim_type_conformance --test sim_partial_features
cargo nextest run --locked --test sim_data_types --test sim_data_types_extended --test sim_data_type_edges
cargo nextest run --locked --test sim_data_types_next --test sim_data_types_completion --test sim_net_resolution --test sim_net_defaults --test runtime_values --test runtime_random
cargo nextest run --locked --test sim_physical_time --test sim_mailboxes
cargo test --locked --lib core::compile::editions::tests -- --test-threads=1
cargo test --locked --test sim_loops --test sim_edition --test sim_syn016_elaboration --test sim_file_io --test runtime_file_io -- --test-threads=1
```

### One readable fixture

```sh
cargo run --locked --bin llg -- --top tb tests/fixtures/sim/type_conformance/uwire.sv
cargo run --locked --bin llg -- --no-opt --top tb tests/fixtures/sim/type_conformance/uwire.sv
```

### Generated-runtime sanitizers (GCC)

```sh
LLG_CC=gcc \
LLG_CFLAGS='-DACO_USE_ASAN -fsanitize=address,undefined -fno-omit-frame-pointer -fno-sanitize-recover=all' \
ASAN_OPTIONS='detect_leaks=1:strict_string_checks=1:log_path=/tmp/llg-asan-model' \
UBSAN_OPTIONS='print_stacktrace=1:halt_on_error=1' \
cargo nextest run --locked --test sim_partial_features --test sim_type_conformance --test sim_procedural_assign --test runtime_values --test runtime_random
```

Inspect every `/tmp/llg-asan-model.*` file for errors. Generated coroutine models
may emit the `__asan_handle_no_return` stack-switch warning; file routing preserves
exact program-stderr assertions without hiding sanitizer reports. Real stack-switch
acceptance is distinct from sanitizer-safe components.

### Repository gate

```sh
cargo fmt --check
cargo check --locked --all-targets --all-features
cargo check --locked --lib --no-default-features
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo nextest run --locked --all-features
cargo test --locked --doc --all-features
```

[CI](../.github/workflows/ci.yml) owns job/suite selection, audits and release checks.
Configuration or another host's results do not establish native Windows/macOS or
architecture-specific generated-model support.

### Reproducible baseline and per-patch regression

Use [rust-toolchain.toml](../rust-toolchain.toml), [Cargo.lock](../Cargo.lock) and
upstream-base gitlinks. [Portable patch preparation](../patches/README.md) applies
tracked patches and authenticates complete clean/applied/retired manifests before
native source consumption. Git metadata is optional; when present, pinned HEAD and
untracked/out-of-set inputs are checked. No project-specific vendor commits.

```sh
scripts/run-regression.sh --label before --output-dir persistence/u05/before
scripts/run-regression.sh --label after --output-dir persistence/u05/after
diff -u persistence/u05/before/summary.tsv persistence/u05/after/summary.tsv
diff -u persistence/u05/before/test-inventory.log persistence/u05/after/test-inventory.log
```

The serialized runner records provenance, commands, phases, timings and full logs.
Dirty-root/submodule overrides mark `metadata.tsv` reproducible=no, not clean baseline
evidence. Inventory diffs expose additions/removals/renames; neither workflow nor
component success blesses changed expected output. Keep run evidence in ignored
`persistence/`, not continuation chapters.

## Dynamic ownership validation

Use [the standalone guide](runtime_value_storage/readme.md) for native CMake/CTest,
allocation instrumentation, independent integer oracles and `validate.py`.
Its Debug/Release builds are not HDL optimizer modes. `--full` additionally requires
Rust checks, actual emitted-C tests, public HDL and the repository suite; missing
prerequisites are blocked. Native generated-HDL execution does not implicitly enable
sanitizers.

`generated_scopes_probe.c`/`generated_coroutine_probe.c` are handwritten output-shape
probes. Actual emitter tests render numeric `ExecutionModel` instances, compile C
and exercise loops and repeated start/advance/close. Public HDL acceptance then
adds frontend/lowering coverage. Preserve all three layers and original runtime/
waveform assertions; component-only tests do not establish zero leaks in unexecuted
paths or whole-simulator speedups.

```sh
cargo test --lib --no-default-features sim::emit_c::owned::tests
cargo test --lib --no-default-features structured_owned_model_
cargo test --locked --no-default-features --test sim_dynamic_ownership
python3 tests/runtime_value_storage/validate.py --compiler gcc --compiler clang --sanitizers --full
```

Shared Cargo/CMake isolation probes retain their assertions; temporary-owner adapters
are test-only. Check one retain per context field, including shared frames;
joined-parent borrowing versus detached copies; cancellation before copy-out;
stream bounds before publication; and owned input/format/key snapshots across
callbacks. `$finish(0)` avoids informational termination output in quiet fixtures,
not actual diagnostics. Use existing active tests rather than adding `--ignored`.

Binary `$fread` checks address 2 then 3 for `[3:0]` start 2/count 2; native and HDL
oracles must agree. Unknown/invalid bounds or negative counts consume/store nothing;
packed optional bounds are evaluated but ignored. Rank-one evidence does not qualify
multidimensional binary reads. Fixed-stream preflight checks must supply declared
array bounds to the current eight-argument helper, not cast shifted arguments.

## Source organization

Domain modules below LSP/integration facades use explicit crate paths and must not
become accidental Cargo targets. Use domain-qualified name filters when necessary.
Fragment/embedding-order tests do not compile runtime fragments independently;
facade compilation and generated-model execution are separate checks.
