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
not reasons to skip. The checker recognizes known static harness forms, including
`sim_undefined_behavior`'s `fixture!(stem, extension)` registrations, not all
possible Rust expressions. Use its working-tree mode before handoff when new
fixtures remain untracked.

```sh
python3 scripts/check_sim_fixture_integrity.py --tracked
python3 -m unittest discover -s scripts -p test_sim_fixture_integrity.py
```

### Feature completion slices

New tasks use `fixtures/sim/feature_completion/<task_slug>/` (ASCII slugs such as
`rtl_001`, or `rtl_002b` for a follow-up slice) and `sim_feature_completion/<task_slug>.rs`, explicitly declared with
`#[path = "sim_feature_completion/rtl_001.rs"] mod rtl_001;` in
`sim_feature_completion.rs`. Keep the existing `g1_*` fixtures with their owners.
Copy an adopted FND-002 witness from local evidence into the task directory,
rename it to an ASCII descriptive name, retain its edition/clause citation, and
add a reviewed independent `.out` oracle (or a specific rejection diagnostic).
Resolve any unresolved oracle with the owning task's policy before claiming acceptance.
Checked-in tests must work without the local evidence, plan or specification PDFs.

Use direct `sim_cli::run_case*` / `reject_case*` calls inside ordinary `#[test]`
functions, with literal fixture names and a literal or `const &str` suite path.
Stems select `.sv`; explicit `.v` and `.sv` names are also supported. Read output
files with `include_str!`; never generate expected values with production code.
These helpers assert prerequisites, status, output and diagnostics in both HDL
optimizer modes. Keep mandatory modules/tests unconditional and unignored;
do not return early on missing tools. The checker requires every new HDL fixture
to be referenced by a public CLI helper inside a declared test. It detects empty
task modules/directories, missing declarations/inputs, disabled tests and untracked
inputs (`--tracked`).
Extend its bounded recognizers and unit tests before adopting another call shape.

The [FND-003 pilot](fixtures/sim/feature_completion/fnd_003/readme.md) also uses
`sim_cli::run_case_after_db_drop`: checked compilation, snapshot destruction,
validated semantic/execution IR and owned whole-model emission, then Db destruction
before CMake and execution. This is generated-model execution; the
`component_fixture_integrity` test and existing validator/architecture probes are
component checks and do not replace public CLI tests.

Run focused acceptance with an explicit empty-selection failure:

```sh
scripts/run-tests.sh --test-work-dir /build --cargo-profile quick \
  --test sim_feature_completion --test emit_decoupling --lib \
  -E 'binary(sim_feature_completion) | binary(emit_decoupling) | test(sim::rt::tests::)' \
  --test-threads 6 --no-tests fail
```

Use the assigned host thread budget. For a single later task, select its module
with `-E 'binary(sim_feature_completion) & test(rtl_001::)'`; run the checker and
architecture gates as well. `--no-tests fail` makes a misspelled/disabled selection
fail rather than accept a zero-test run. No new acceptance wrapper is needed.

RTL-003's projection fixtures use `-E 'binary(sim_feature_completion) &
test(rtl_003::)'`. They cover selected module refs, fixed record rows, selector
capture for blocking/NBA/mutation/copy-out and synchronous scanner ref views;
representative cases also run after Db destruction at native O0/O3.

RTL-004's fixed-pattern fixtures use `-E 'binary(sim_feature_completion) &
test(rtl_004::)'`. They cover packed and record type keys, nested rows, selector
snapshots, persistent NBA publication, undefined-multiplicity effects and sparse
patterns at 16M cells. Representative fixtures run after Db destruction at O0/O3;
source-size checks supplement public execution.

RTL-005's fixed equality, conditional and cast fixtures use
`-E 'binary(sim_feature_completion) & test(rtl_005::)'`. Positive fixtures run in
both optimizer modes on the legacy and compact backends (`run_case_backend_parity`)
and after Db destruction; descriptor casts also bound generated model size.

RTL-006's arithmetic and mutation fixtures use `-E 'binary(sim_feature_completion) &
test(rtl_006::)'`; set `LLG_TEST_GMP_ROOT` to include the compact GMP lane. The
arithmetic expectations come from the in-test limb oracle
(`sim_feature_completion/rtl_006/oracle.rs`) at widths 1 through 129 and at the
8,128/8,129-bit kernel threshold, in both optimizer modes on every backend.

RTL-007's zero-time call and evaluator fixtures use
`-E 'binary(sim_feature_completion) & test(rtl_007::)'`. Positive fixtures run in
both optimizer modes on both backends (set `LLG_TEST_GMP_ROOT` for the GMP
kernel lane); stateful event helpers assert the documented process-evaluation
policy and check evaluation counters only as lower bounds.

RTL-008's initialization, package/`$unit` and let fixtures use
`-E 'binary(sim_feature_completion) & test(rtl_008::)'`. Single-file positives run
in both optimizer modes on both backends (set `LLG_TEST_GMP_ROOT` for the GMP
lane); multi-file package and `$unit` cases run in separate and merged
compilation-unit modes through `run_case_with_source_prefix`. Cross-scope
initialization oracles are order-independent or follow a static read
dependency; the Verilog-2001 race fixture asserts its allowed result set.


RTL-009's port fixtures use `-E 'binary(sim_feature_completion) & test(rtl_009::)'`.
Positive fixtures run in both optimizer modes on both backends and after Db
destruction; descriptor ports also bound generated model size. Negatives cover
assignability, ref shape, variable inout, runtime ref/net/inout selects and
output-target writer conflicts.

RTL-010's continuous topology fixtures use
`-E 'binary(sim_feature_completion) & test(rtl_010::)'`. Positive fixtures run in
both optimizer modes on both backends (set `LLG_TEST_GMP_ROOT` for the GMP lane)
and after Db destruction; descriptor scatter also bounds generated model size.
Negatives cover overlapping variable writers, runtime pattern-leaf selects and
zero-time nonconvergence; delayed pattern drivers remain ADV-002's boundary.

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
| Ports and nets | `sim_port_net_types`, `sim_net_resolution`, `sim_net_defaults`, `sim_net_decl`, `sim_inout`: directional collapse, independent drivers, strengths/defaults, aliases, selections and delayed publication. [Electrical ranges](fixtures/sim/net_partition/readme.md), `sim_net_partition` and `generated_c_frame_lint::electrical_net_partition_fixtures` cover electrical runs across 1/7/64/65/129 bits, independent value oracles, delayed descriptor loops and declared shapes; `sim_waveform`/`sim_vpi` have generated-model net-view probes. Port-type unit tests cover all 81 cells; frontend tests cover 49 resolved pairs. |
| Structural UDPs | `sim_udp`, [SYN-031 matrix](fixtures/sim/syn031_combinational_udp/readme.md): Verilog-2001 and SystemVerilog-2009 mux/parity tables, four-state inputs, `?`/`b`, unmatched rows, instance arrays, independent net drivers, delays, invalid port/row-width diagnostics and sequential/edge rejections in both optimizer modes. |
| Practical RTL | [RTL composition](fixtures/sim/rtl_completion/readme.md), `sim_rtl_completion`: initialization, fixed values/references/unions, ports and array/interface/inout composition. |
| Arrays and projections | `sim_fixed_array_reductions`, `sim_syn026_iterator_indices`, `sim_array_conditional_assignments`, `sim_fixed_ordering_review`, `sim_syn027_fixed_reverse`, `sim_syn028_fixed_sort`, `sim_group1_repairs`, `sim_group1_formal_repairs`: [reductions](fixtures/sim/fixed_array_reductions/readme.md), [iterator indices](fixtures/sim/syn026_iterator_indices/readme.md), [conditional assignments](fixtures/sim/array_conditional_assignments/readme.md), [fixed reverse](fixtures/sim/syn027_fixed_reverse/readme.md), fixed sort/rsort maps and permutation, activation isolation, signed member conversion, captured outputs and const/NBA negatives. |
| Expressions | `sim_fill_literals`, `sim_wildcard_eq`, `sim_bit_queries`, `sim_packed_strings`, `sim_sequential_predicates`, `sim_syn022_primitive_patterns`, `sim_syn023_structure_patterns`, `sim_syn024_tagged_patterns`, `sim_syn025_pattern_cases`, `sim_audit_a1_packed_constant_patterns`: fills, wildcard equality/inside, queries, [ordered predicates/patterns](fixtures/sim/sequential_predicates/readme.md), [primitive pattern scope/edition checks](fixtures/sim/syn022_primitive_patterns/readme.md), [recursive fixed structure patterns](fixtures/sim/syn023_structure_patterns/readme.md), [tagged pattern modes and bindings](fixtures/sim/syn024_tagged_patterns/readme.md), [pattern-case qualifier, control and rejection checks](fixtures/sim/syn025_pattern_cases/readme.md), and [packed integral constant-pattern acceptance and nonintegral rejection](fixtures/sim/audit_a1_packed_constant_patterns/readme.md). |
| Processes and loops | [Always-family](fixtures/sim/process_semantics/readme.md), [loops](fixtures/sim/loops/readme.md), `sim_loops`, `sim_syn014_process_contexts`: source-loop exits, omissions/bounds, aggregate sensitivity, legal disjoint writers and illegal writer/event controls. |
| Scheduling and synchronization | `sim_coroutine_semantics`, `sim_process_control`, `sim_semaphore`, `sim_mailboxes`, `sim_procedural_assign`, `sim_force`; [coroutine migration oracles](fixtures/sim/coroutine_semantics/readme.md), [processes](fixtures/sim/process_control/readme.md), [semaphores](fixtures/sim/semaphore/readme.md), [mailboxes](fixtures/sim/mailboxes/readme.md), [nonconvergence](fixtures/sim/nonconvergence/readme.md). |
| Generated coroutine storage | `sim_frame_hoisting`, `generated_c_frame_lint`: narrowed/flattened frame shape, deep-block strict-C compile/run and warning-clean C checks plus a six-way sweep of every independently generatable simulation fixture in optimized and unoptimized modes. The reusable lint permits local addresses only when the declaring block is resume-free, and rejects stack addresses in resume-bearing scopes, exact generated-name lookalikes and overlay paths used outside their emitted C block. |
| Instance body sharing | `sim_instance_sharing`, [instance identities](fixtures/sim/instance_sharing/readme.md): shared module/task and generate bodies preserve `%m`, named events, external disable isolation and net contribution slots in both optimizer modes. Render tests cover thresholds, width/parameter splits, PCA sites and deterministic records; the frame lint recognizes shared entries and model-lifetime record addresses. |
| Time and numeric services | `sim_delay`, `sim_time_literals`, `sim_time_values`, `sim_physical_time`, `sim_real`, `sim_real_conversions`, `sim_random`, `sim_random_streams`: exact units/rounding, overflow, real precision, Annex N vectors and child-stream isolation. |
| I/O and observation | `sim_file_io`, `sim_memory_editions`, `sim_memory_views`, `sim_syn030_memory_views`, `sim_plusargs`, `sim_waveform`, `sim_vpi`, `sim_syn035_pla`; [SYN-030 memory views](fixtures/sim/syn030_memory_views/readme.md), checked signed `@` spellings/bounds in both editions and selected views in SV2009, [SYN-035 PLA exclusion](fixtures/sim/syn035_pla/readme.md), [file I/O](fixtures/sim/file_io/readme.md), [waveforms](fixtures/sim/waveform/readme.md), [partial features](fixtures/sim/partial_features/readme.md). |
| Undefined behavior characterization | `sim_undefined_behavior` compares exact CLI stdout, stderr and status to checked-in [Q02/Q03 goldens](fixtures/sim/undefined_behavior/readme.md) in both optimizer modes and each legal edition. These are observations for later cross-simulator comparison, not independent conformance oracles. |
| Verification and native objects | `sim_classes`, `sim_virtual_interfaces`, `sim_dpi`, [concurrent assertions](fixtures/sim/concurrent_assertions/readme.md): nominal identity, dispatch/casts, interface bindings, scalar native ABI, sampled/sequence/property behavior and explicit rejection boundaries. |
| Build and safeguards | `sim_cmake`, `cli_info`, `sim_memory_guard`, `support_harness`, `emit_decoupling`, `generated_c_determinism`: CMake/generator failures, source cleanup, early CLI exits, memory limits, timeout trees, CWD recovery, frontend-independent emission and byte-identical generated source trees across separate processes. |
| Native components | `runtime_values`, `runtime_random`, `runtime_file_io`, `runtime_boundaries`, `runtime_value_storage`: direct runtime probes, independent of HDL lowering. |
| Integrated selected profile | `sim_syn038_ledger`, pairwise suites and `sim_syn039_acceptance`; [ledger](syn038_coverage_ledger.md), [integrated fixtures](fixtures/sim/syn039_acceptance/readme.md). SYN-039 runs four runtime-stimulated compositions in both optimizer modes and preserves the sequential-UDP rejection. |
| Compiler directives | `sim_directive_effects`, `sim_syn017_directive_effects`, `sim_edition` and `sim_syn038_ledger`; [SYN-017 matrix](fixtures/sim/syn017_directives/readme.md) covers both editions, both optimizer modes, preprocessing into execution, unit state and strict older-edition gates. |

`runtime_value_facade` runs the cheap private-field/template guard in
`scripts/check_value_facade.py`. Its explicit whitelist contains only nonpacked
metadata receivers; value backends and vendored GTKWave are excluded. Run the
script directly for a quick audit, or with `--self-test` to check rejection cases.
Native layout expressions (`sizeof`/`_Alignof`) remain legal; V07 owns numeric
emitter frame estimates and backend ABI selection.

### Selected-profile qualifications

- [SYN-012 fixed layouts](fixtures/sim/syn012_fixed_layout/readme.md) spans widths
  1/7/8/31/32/33/64/65/129, nested records/unions, state conversion and nominal
  rejection controls. [SYN-013 calls](fixtures/sim/syn013_zero_time_calls/readme.md)
  preserves static output state versus automatic defaults/inout copy-in and
  checks persistent static task-local array NBAs, selected rows, per-instance
  state, notification and automatic-storage rejection in both optimizer modes.
- [SYN-014 processes](fixtures/sim/syn014_process_contexts/readme.md) checks an
  unselected aggregate arm as a dependency, no notification for unchanged results,
  and repaired pattern/array/record writer contexts. `sim_syn015_stream_contexts`
  checks fixed casts, ref projections, storage-order selectors, alignment and
  runtime oversize rejection; native preflight checks do not replace this suite.
- `sim_syn028_fixed_sort` checks signed/unsigned/enum and mapped packed/unpacked
  records, repeated-key permutation, reverse declared bounds, zero-time function
  use, 2001 rejection and illegal map/const-ref cases. `sim_review_next4`
  retains selected automatic/formal receiver checks; `sim_data_types_completion`
  retains queue and dynamic-array ordering controls. All positive fixtures run
  in both optimizer modes. `sim_container_sort` covers queue/dynamic `sort` and
  `rsort` (stable ties, X/Z-key barriers, wide/signed keys, function keys and a
  20000-element size) on both value backends; the native key-evaluation count and
  element-identity checks live in the `runtime_containers` probe.
- `sim_unknown_digits` checks `%h/%x/%o/%d/%b` and `$sformatf` text for all-x/all-z,
  partial-x/z and mixed x+z digits at widths 1, 4, 8, 65 and 130 (lowercase when
  every bit of a digit is unknown, uppercase otherwise; IEEE 1800-2009
  21.2.1.4) in both optimizer modes and on both value backends.
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
  retagged before commit has Q03 undefined-behavior characterization, not a
  conformance oracle.
- [SYN-032 configurations](fixtures/sim/syn032_library_configs/readme.md) and
  [SYN-033 bind](fixtures/sim/syn033_structural_bind/readme.md) keep bounded library,
  configuration and structural composition separate from arbitrary verification
  injection. Library-map tests cover final-rank ties, explicit overrides, original
  source locations, bounded logical/disk buffers and snapshot lifetime. SYN-032
  also checks both input contracts and editions for selected bindings,
  same-name map composition changes, binding failures and library-scoped
  `-incdir` headers, ordered directory lookup, logical buffer resolution and
  missing/unauthorized directory diagnostics. Macro maps test generated
  declarations, paths, config delimiters, conditional command-line selection,
  included-map scope, macro `-incdir`, edition-specific directives and located
  failures through disk CLI in both modes/editions and logical owned compilation
  (V §13.2/§19.3-§19.5;
  SV §33.3/§22.4-§22.6).
  `sim_syn033_structural_bind` checks type-wide, selected-instance,
  generate-for/if and interface binds through the public CLI in both optimizer
  modes, then checks elaborated names and parameters after snapshot destruction;
  unknown/illegal targets, duplicate names and out-of-scope actuals reject.
- [SYN-036 capacity](fixtures/sim/syn036_capacity/readme.md) separates syntax from
  resource limits. Public cases cover the former 65,536-cell boundary and the
  exclusive 1,048,576-bit packed boundary in both editions and optimizer modes;
  2009 fixed-value formals also cover descriptor transport beyond packed capacity. Nested
  values, cell-wise reductions, recursion and checked dimension products/strides retain
  separate controls. Run the large generated models in the dedicated resource lane:
  `scripts/run-tests.sh --test-work-dir /build --test-threads 10 --test sim_syn036_capacity --run-ignored only`.
  `sim_nonconvergence` and `runtime_boundaries` own scheduler/process budget
  probes; `sim_capacity` owns retired registry-growth controls. Two-state net
  illegality remains distinct from conversion into two-state storage.
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
The grammar register retains 93 stable IDs: 87 positive Core rows, five
negative legality/profile controls and one positive Extended UDP control.
Negative controls and verification-only old groups are excluded from the
positive Core denominator.
Its N04 witnesses check 7/33/65-bit signed and unsigned runtime arithmetic in
both editions and optimizer modes, including X/Z and zero divisors, and check
SV `const` reads plus a rejected write. The ledger's review/Extended links name
their behavioral test owners separately from open policy, excluded forms,
generated-model sanitizer lanes, the SYN-035 PLA exclusion and SYN-036
capacity limits. SYN-036 has four ignored resource tests that require an
explicit `--run-ignored` invocation for execution.

The Core audit checks fixture paths and named test functions, then compares
claimed syntax, widths, editions and contexts with the actual fixture and
harness; a fixture path alone does not establish a passing outcome.
The four named three-way chains are separate from the pairwise count; the
record conditional/NBA chain takes `+seed=1` through the public CLI so its
X-selector and differing data arms are runtime selected.
The numeric let witness takes `+seed=4` in SV2009, checks typed/untyped
formals, constant expansion, lexical parameter scope and runtime substitution,
and has a V2001 rejection control.
The genvar-function witness takes `+seed=22` through both editions and modes;
its constant function uses a case and loop to set the generate bound. It checks three generated XOR lanes (5), generate-case selection (1),
ANSI task output/inout copy-out (6/6), variable-concatenation nibbles (1/6), and a child
integer output and hierarchical function call (44 each).
The pull-gate fixture now has a dual-edition public-CLI owner.

The [Annex A assignment table](syn038_annex_assignments.json) assigns each of
940 reconciled PDF/addendum names to a witnessed Core row, qualified Extended
owner, implemented outside-Core owner, named exclusion or assigned parent
production. The PDF extraction has 811
distinct names and the reference addendum has 794: 665 overlap, 146 appear
only in the PDF extraction and 129 only in the addendum. The addendum-only
entries mark their edition as an inference. These are name inventories, not
grammar-support percentages. The 940 assignments contain 228 direct Core
names, 61 Extended forms, 18 implemented outside-Core forms, 411 exclusions
and 222 aliases/helpers, with zero OPEN or BOUNDARY names. The checker
requires Extended entries to name real owning task fixtures and test functions;
implemented outside-Core entries name a product row and execution owner.
Sequential UDP edge/state forms remain excluded while scalar combinational
UDP forms have SYN-031 evidence. The ordinary checker validates the frozen PDF
name sets and the checked-in [addendum name/section snapshot](syn038_annex_reference_names.json).
The snapshot records the ignored source file's SHA-256 and generator command;
it contains names and section IDs only. Optional source re-verification needs
the local specification pack (and `pypdf` for PDFs). Missing requested inputs
fail explicitly:

```sh
python3 scripts/check_syn038_annex_assignments.py
python3 scripts/check_syn038_annex_assignments.py --reference-addendum docs/specification/spec-reference-annex-a.md
python3 scripts/check_syn038_annex_assignments.py --pdf-root docs/specification
```

`scripts/generate_syn038_annex_assignments.py` regenerates the table from
those optional PDFs and the addendum. Run the checker and review every changed
disposition after regeneration; grammar names that share a parent do not by
themselves prove every semantic variant executes.
Regenerate the addendum snapshot with
`python3 scripts/generate_syn038_annex_reference_names.py` when that reference
changes; review its name/section diff with the assignment table.

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
- Cargo-launched runs get `LLG_RUNTIME_CACHE_DIR=<repo>/target/llg-runtime-cache`
  from `.cargo/config.toml` `[env]`, so CLI tests in temporary CWDs share it.
  An explicit value wins; use an absolute path, because `llg` resolves
  relative values from its own CWD.
- Nextest defaults to 8 concurrent tests; `--profile max-threads` opts into 32.
  CI sets `NEXTEST_PROFILE=ci`, which runs one test per available logical CPU.
  Preserve suite-specific skips: `sim_cmake` may skip absent CMake; native DPI
  shared-library tests need Unix/CMake/compiler. These are not blanket skips.

### Cargo build profiles

Daily `cargo test`, `cargo nextest run` and `scripts/run-tests.sh` builds use the
`test` profile at `opt-level = 3`, matching release optimization. Debug assertions
and overflow checks stay enabled to catch invariant and arithmetic failures.
The simulator's in-process Rust pipeline benefits from this optimization.

Use `quick` for shorter rebuilds during edit-test loops:

```sh
cargo test --locked --profile quick --test sim_counter
cargo nextest run --locked --cargo-profile quick --test sim_counter
scripts/run-tests.sh --test-work-dir /build --cargo-profile quick --test sim_counter
```

`quick` inherits `test` with `opt-level = 0`. Cargo's `--profile` and nextest's
`--cargo-profile` choose the build profile; nextest's `--profile` independently
chooses runner settings such as concurrency. Run the optimized default for
repository validation and performance comparisons. These profiles control Rust
compilation; generated-model optimization and CLI `--no-opt` runs keep their
existing settings.

`quick` optimizes dependencies at level 3 while leaving workspace code at level 0.
Cargo reuses those dependencies across workspace edits; compiler caches can also
reuse them across builds. Both profiles keep debug assertions and overflow checks
and use line tables for file/line backtraces rather than full variable debug info.

Default test artifacts use `target/debug/`; `quick` adds a second Cargo artifact
tree at `target/quick/`, including its own dependencies and incremental state.
Budget disk for both. The worktree runner keeps both on disk and uses the same
isolated scratch and compatible runtime cache for either profile.

### Optional development accelerators

All accelerators are opt-in. With no settings, Cargo and the test runner keep
their existing compiler, linker and wrapper defaults; no extra tools are required.
Install the requested tools on `PATH` before opting in. A missing tool is an error.

```sh
# Rust cache and Linux GNU host linker for this test invocation:
CARGO_BUILD_JOBS=4 scripts/run-tests.sh --test-work-dir /build \
  --cargo-profile quick --test-threads 4 --test sim_function --sccache --mold

# Plain cargo build/test/nextest in this Bash shell (source from repository root):
export LLG_SCCACHE=1 LLG_MOLD=1; source scripts/dev-env.sh
cargo build --locked --profile quick --bin llg
```

The runner also accepts `LLG_SCCACHE=1` and `LLG_MOLD=1` directly. Plain Cargo
needs the sourced helper; these variables alone cannot configure Cargo's linker
or Rust wrapper. Sourcing validates all requested tools before exporting settings.
An existing nonempty `RUSTC_WRAPPER` wins. The tracked sccache wrapper sets
`TMPDIR=/tmp` only in cache/compiler subprocesses, so the server's Unix socket
fits even when the test runner uses a long scratch path. Tests keep their own
`TMPDIR`; existing sccache cache/server settings are preserved.
Sccache cannot cache Rust's incremental compilation or final executable linking.
For cache reuse of workspace library objects across cleans, optionally set
`CARGO_INCREMENTAL=0`; the helper leaves Cargo's incremental setting unchanged.

Mold uses `CARGO_TARGET_<GNU_HOST_TRIPLE>_LINKER` and the tracked wrapper's
`cc -fuse-ld=mold`. It leaves `RUSTFLAGS` and `.cargo/config.toml` unchanged,
including Linux `split-debuginfo=unpacked`. A conflicting explicit host linker
is an error; clear it before opting in. `LLG_MOLD_CC=clang` selects another
compiler driver. Only the Linux GNU host triple is changed; musl and other
cross targets retain their configured linkers, and opting in on a non-GNU host
fails. Generated-model linkers retain their defaults.
`LLG_MOLD_THREADS=4` limits mold's internal threads on a busy host; unset uses
mold's own default. Cargo and nextest concurrency remain separate settings.

Native caching is independent of the Rust opt-in:

```sh
# Slang and its wrapper, through root build.rs (1/on/true still select ccache):
export LLG_CCACHE=sccache  # or ccache; 0/off/false disable

# Generated models through the existing compiler option/environment:
export LLG_CC="$PWD/scripts/sccache-cc.sh"  # uses LLG_SCCACHE_CC, otherwise cc
# Or select a launcher for an individual model:
target/quick/llg --launcher "$PWD/scripts/sccache.sh" --top tb design.sv
# ccache works through the same --launcher option: --launcher ccache
```

`LLG_CCACHE` now rejects invalid values or a missing requested executable instead
of continuing uncached. Changing the launcher reconfigures the native CMake cache
while retaining objects. The Unix Slang sccache launcher and both model examples
use the short-TMPDIR wrapper. Sourcing the helper checks `LLG_CCACHE` too; choose it
before sourcing if you want that early check for plain Cargo.
`LLG_SCCACHE_CC=clang` changes the generated-model wrapper's underlying compiler;
it uses a separate variable because CMake can set `CC` to the wrapper itself.

Settings exported into a shell remain until unset or the shell exits. To return
to defaults, unset `LLG_SCCACHE`, `LLG_MOLD`, the helper-installed `RUSTC_WRAPPER`
and `CARGO_TARGET_<GNU_HOST_TRIPLE>_LINKER`; unset `LLG_CCACHE`/`LLG_CC` if selected.
Use a fresh shell if you need to restore earlier user overrides.

Script regression checks (fake tools; no Rust/native build required):

```sh
python3 -m unittest discover -s scripts -p test_dev_env.py
python3 -m unittest discover -s scripts -p test_run_tests.py
```

Native launcher selection and CMake cache-state regressions run with
`scripts/run-tests.sh --cargo-profile quick --test compiler_cache`.

### Test build storage

#### Parallel worktrees

Use the opt-in runner when several worktrees share a limited tmpfs. Run this
command in each worktree, substituting your existing executable mount path:

```sh
scripts/run-tests.sh --test-work-dir /build --test-threads 8
# Focused run; all nextest selection arguments remain available:
scripts/run-tests.sh --test-work-dir /build --test sim_counter
```

`--test-work-dir` selects the root for temporary test files, generated simulator
builds and the shared runtime cache. Rust and Slang builds stay in each worktree.

The runner needs Bash and `sha256sum` or `shasum`. It checks execution permission
before building, so a `noexec` mount fails early. Relative `--test-work-dir` paths
resolve from the caller's directory. The option overrides inherited scratch,
runtime-cache and Cargo output-directory environment settings for this run.
Omitting it preserves existing settings and defaults; `/build` is never assumed.

| Storage | Placement with `--test-work-dir PATH` |
| --- | --- |
| Generated model sources, CMake trees, objects and simulators | `PATH/lapligence/worktrees/<worktree-hash>/run.<unique>/tests/` |
| Temporary files from tests/tools honoring Unix `TMPDIR` | The same run's `tmp/` |
| Compatible simulation runtime archives | Shared `PATH/lapligence/runtime-cache/` |
| Cargo targets and intermediate build artifacts | Each worktree's `target/` on its existing filesystem |
| Native Slang CMake build | Each worktree's existing `target/slang/` |
| Cargo downloads and optional compiler caches | Existing persistent locations |

The hash uses the canonical worktree path, and each invocation receives a fresh
run directory even within the same worktree. All agents can use the same command;
no manual agent number is needed. Runtime entries are keyed by sources, ABI,
toolchain, flags and build options, with process locks for cache population.
Do not share mutable model/CMake/Cargo build directories between worktrees or
pass `--target-dir` to override the runner's worktree-local Cargo location.

Successful runs remove their run directory, including leftover temporary files.
Failures retain remaining scratch and print its path; interrupted runs can also
leave scratch behind. Preserve needed logs/artifacts in the worktree's ignored
`persistence/` before removing an inactive run. Never delete another running
agent's directory or prune the shared runtime cache while tests use it. The
worktree parent directories and runtime cache persist; cache eviction is manual.

With four agents, eight test threads per agent means up to 32 concurrent tests,
plus compiler subprocesses. The runner adds no global concurrency limit or disk
quota. Keep Rust targets on disk for a 32 GiB tmpfs; monitor `df -h /build` and
`du -sh /build/lapligence/*`, adjusting the example paths as needed. Leave RAM
for the agents and compilers. Directory isolation alone does not prevent a full
tmpfs, and mounting a filesystem as tmpfs does not preallocate its capacity.

Runner regression checks (no Rust compiler required):

```sh
python3 -m unittest discover -s scripts -p test_run_tests.py
```

#### Individual environment overrides

Set `LLG_TEST_BUILD_DIR` to place the shared simulator harness's temporary builds
on another filesystem, such as a tmpfs. This covers generated C sources, CMake
build trees and simulator executables, including public CLI acceptance runs:

```sh
LLG_TEST_BUILD_DIR=/build/llg-tests \
LLG_RUNTIME_CACHE_DIR=/build/llg-runtime-cache \
cargo nextest run --locked --test-threads 8
```

Both overrides are optional; `/build` is only an example. Without
`LLG_TEST_BUILD_DIR`, the harness continues to use the system temporary directory.
A relative `LLG_TEST_BUILD_DIR` resolves from the Cargo workspace, including
inside nested temporary CWDs; give `LLG_RUNTIME_CACHE_DIR` as an absolute path.
Missing directories are created; an empty or unusable `LLG_TEST_BUILD_DIR` fails
rather than falling back to another disk. The filesystem must permit
execution (`noexec` prevents CMake compiler checks and simulators from running).

Test parallelism is unchanged. Model and runtime-archive builds run
`cmake --build --parallel <N>`, with `N` from `$CMAKE_BUILD_PARALLEL_LEVEL` when
set, else the host CPU count; the harness leaves it unset because the runtime
archive is built once per cache key under a lock and a model is one translation
unit (revisit when models are split into parts). Every invocation owns a unique child directory and
removes that child on completion or unwind; the configured root and unrelated
files are retained. `LLG_RUNTIME_CACHE_DIR` separately relocates the shared runtime
archive cache, which persists across tests and runs until removed or unmounted.
Size tmpfs for concurrent builds and leave RAM for compilers and tests.

`LLG_TEST_BUILD_DIR` does not relocate Cargo artifacts or tests that use their own
temporary-directory helpers. Cargo supports `CARGO_TARGET_DIR`; system temporary
files follow platform settings such as `TMPDIR` on Unix. The native Slang build
still uses the repository's `target/slang` directory.

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
LLG_CFLAGS='-fsanitize=address,undefined -fno-omit-frame-pointer -fno-sanitize-recover=all' \
ASAN_OPTIONS='detect_leaks=1:strict_string_checks=1:log_path=/tmp/llg-asan-model' \
UBSAN_OPTIONS='print_stacktrace=1:halt_on_error=1' \
cargo nextest run --locked --test sim_partial_features --test sim_type_conformance --test sim_procedural_assign --test runtime_values --test runtime_random
```

Inspect every `/tmp/llg-asan-model.*` file for errors. Stackless coroutine frames
need no stack-switch suppression; file routing preserves exact program-stderr
assertions without hiding sanitizer reports.

For SYN-039, use those same sanitizer/compiler settings with the worktree
runner and the following filters:

```sh
export CARGO_BUILD_JOBS=10
LLG_CC=gcc \
LLG_CFLAGS='-fsanitize=address,undefined -fno-omit-frame-pointer -fno-sanitize-recover=all' \
ASAN_OPTIONS='detect_leaks=1:strict_string_checks=1:log_path=/tmp/llg-asan-syn039-model' \
UBSAN_OPTIONS='print_stacktrace=1:halt_on_error=1' \
scripts/run-tests.sh --test-work-dir /build --test-threads 10 \
  --test sim_syn039_acceptance --test sim_syn038_ledger \
  --test sim_syn033_structural_bind --test sim_syn032_library_configs \
  --test sim_syn024_tagged_patterns --test sim_tagged_union_access \
  --test sim_review_next4 --test sim_review_tasks08_11 \
  --test sim_fixed_ordering_review --test sim_casez
scripts/run-tests.sh --test-work-dir /build --test-threads 10 --lib \
  -E 'test(sim::emit_c::owned::tests::tagged_guards)'
```

The integrated Extended fixture includes the combinational UDP; the ledger
also runs its ANSI control. Inspect generated-model reports separately from
handwritten native probes. Test counts represent repeated optimizer
executions, not unique designs.

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

For a focused debug-frame check, run the overlay render tests and the checked-in
coroutine fixtures through both GCC/Clang debug and release syntax checks:

```sh
LLG_CFLAGS=-DLLG_CO_DEBUG scripts/run-tests.sh --test-work-dir /build --cargo-profile quick --lib --test generated_c_frame_lint -E 'test(debug_poison) | test(debug_frame_overlay_fixtures)' --test-threads 8
```

The native `coroutine_poison_release`/`coroutine_poison_debug` probes read fresh
callee and dead overlay bytes, preserve headers/anchors, and check owner release
before poisoning. Select them with `ctest -R '^coroutine_poison_'` in a configured
native build; keep generated HDL fork/ref/recursive lifetime tests alongside them.

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

The packed-value emitter traffic regressions run through the public CLI in both
optimizer modes:

```sh
scripts/run-tests.sh --test-work-dir /build --cargo-profile quick --test sim_emit_value_traffic
```

Render tests in `sim::emit_c::owned::tests::value_traffic` cover borrow eligibility,
matching/mismatched conversions, constant lifetimes and retained frame values.
`runtime_value_storage/value_ownership_probe.c` checks destination aliases and
allocation-free same-width arithmetic, including X/Z and 64/65-bit boundaries.

Standalone compact-backend checks and microbenchmarks, including net/strength,
real/time, formatting/index and facade adapter probes, are opt-in CMake targets;
see [native value probes](runtime_value_storage/readme.md#standalone-compact-value-backend).
They cover core values and the S2/S3 shifts, reductions, case/wildcard and range
families with portable and optional GMP limb kernels, independently of generated
model selection and do not replace later HDL/model integration acceptance.

### Experimental value backend builds

`sim_value_backends` checks selected runtime archives, clean source-only builds,
both wrong-backend and wrong-kernel links, exact C selectors, missing GMP and
positive generated-HDL parity with independent expected outputs. Set
`LLG_TEST_GMP_ROOT` to include the GMP lane; without it legacy and compact portable
run, and GMP parity is reported blocked. Native `selected_*_facade` probes in
`runtime_value_storage` exercise the common consumer bridge in all three modes.

```sh
CARGO_BUILD_JOBS=6 CMAKE_BUILD_PARALLEL_LEVEL=6 LLG_TEST_GMP_ROOT=/path/to/gmp scripts/run-tests.sh --test-work-dir /build --cargo-profile quick --test sim_value_backends --test-threads 6
```

`CodegenOptions.value_config` and `CmakeBuildOpts.value_config` must agree. Driver
selection uses `LLG_VALUE_BACKEND` and `LLG_COMPACT_KERNELS`; default legacy/portable.
GMP mode requires `GMP_ROOT`, with no system fallback. Compact frame checks use a
24-byte descriptor instead of legacy's 32-byte descriptor on 64-bit hosts;
`compact_selected_frame_lint` covers suspended values, shared instances and
structural nets with both kernels and optimizer modes.

### Required RTL-002 capacity lane

This normal suite executes 16,777,216-cell storage, a multidimensional product at
that capacity, whole copies, equality, issue-time NBA snapshots, automatic value
calls, streams, selected dependencies and memory-file services. It also retains
separate packed-width and language-illegal witnesses. Run with the launch's thread
budget (six in this example):

```sh
CARGO_BUILD_JOBS=6 CMAKE_BUILD_PARALLEL_LEVEL=6 scripts/run-tests.sh --test-work-dir /build --cargo-profile quick --test sim_feature_completion -E 'test(rtl_002)' --test-threads 6 --no-tests fail
```

The source-size case compares 65,537 and 16,777,216 cells. Native
`fixed_array_storage_probe.c` checks repeated descriptor copies, stable cells,
queued snapshot cleanup and zero remaining value owners.

The public CLI, shared `support/sim.rs` execution helpers, owned-DB lifetime helper,
VPI and waveform execution harnesses honor `LLG_VALUE_BACKEND` and
`LLG_COMPACT_KERNELS`. Direct library/component tests retain explicit or default
configurations; this environment does not change the library's legacy defaults.
Run the whole suite with compact portable using:

```sh
LLG_VALUE_BACKEND=compact LLG_COMPACT_KERNELS=portable CARGO_BUILD_JOBS=6 CMAKE_BUILD_PARALLEL_LEVEL=6 scripts/run-tests.sh --test-work-dir /build --cargo-profile quick --test-threads 6 --no-fail-fast
```

Run the cross-backend parity matrix alone with:

```sh
LLG_TEST_GMP_ROOT=/path/to/gmp CARGO_BUILD_JOBS=6 CMAKE_BUILD_PARALLEL_LEVEL=6 scripts/run-tests.sh --test-work-dir /build --cargo-profile quick --test sim_value_backends -E 'test(parity::compact_parity_) | test(compact_hdl_subset_matches_legacy_and_independent_outputs)' --test-threads 6
```

These tests force both backends,
both compact kernels and both HDL optimizer modes independently of the runner's
selection. VPI requires a Unix shared-library compiler. The matrix covers
arithmetic destinations, selections, streams, nets/force, NBA capture, VPI and
waveforms, RTL-001 through RTL-004 fixtures, a 16,777,216-cell array copy/NBA
fixture and the `sv4_gmp_integration` owner fixtures (scheduler snapshots,
containers/mailboxes, recursive activations and canceled waits, clocking/
inertial/sequence captures). `sim_stack_bounds` adds the deep-recursion and
long-body fixtures to the same matrix.

The native runtime probes run against each backend through
`tests/runtime_value_storage.rs` (legacy, compact portable, and compact GMP when
`LLG_TEST_GMP_ROOT` is set):

```sh
LLG_TEST_GMP_ROOT=/path/to/gmp CARGO_BUILD_JOBS=6 CMAKE_BUILD_PARALLEL_LEVEL=6 scripts/run-tests.sh --test-work-dir /build --cargo-profile quick --test runtime_value_storage --test-threads 6
```
