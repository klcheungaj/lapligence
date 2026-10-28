# Tests

## Scope and layout

- Shared Slang frontend, owned semantic database, simulator and LSP integration.
- [Shared harnesses](support/readme.md): temporary directories, cleanup and timeouts.
- [LSP fixtures](fixtures/lsp/): framed stdio tests, manifests and source headers.
- [Simulation feature status](../docs/sim_features.md): the sole support checklist.
- [LRM decision register](../docs/lrm_decisions.md): portable cases in
  [fixtures/sim/lrm_decisions/](fixtures/sim/lrm_decisions/) (see [LRM decision cases](#lrm-decision-cases)).

## Test binaries

Cargo `autotests` is off. Each `tests/<suite>.rs` file is a module of one grouped
test binary, so the library, Slang and the C++ wrapper are linked once per group
instead of once per file (each link is ~80 MB in `quick`; per-file binaries made a
single profile's `deps/` ~22 GB):

| Binary | Suites (`mod` lines in the root) |
| --- | --- |
| `sim_syn` | `tests/sim_syn*.rs` |
| `sim_a_m` | other `tests/sim_[a-m]*.rs` |
| `sim_n_z` | `tests/sim_[n-z]*.rs` except `sim_syn*` |
| `runtime` | `tests/runtime_*.rs` |
| `general` | every other `tests/*.rs` (frontend, LSP, lint, CLI, configuration, tooling); the Windows on ARM lane runs only its language-server suites (`ci-lsp` profile) |
| `sim_feature_completion` | its `#[path]` task modules |
| `sim_cmake`, `wide_elab` | standalone: they change the process environment or CWD outside the shared harness lock, which plain `cargo test` would share across a group |
| `cli_info` | standalone small harness for the CI release linkage audit |

A new suite needs one `mod <suite>;` line in its group root (`tests/<group>.rs`);
the `general` binary's `test_layout` suite fails with that exact line otherwise.
Shared helpers (`support/sim.rs` as `sim_harness`, `support/sim_cli.rs` as
`sim_cli`, `support/c_compiler.rs`, `support/generated_c_lint.rs`, `support/`) are
declared once in each group root; suites import them with
`use crate::sim_harness;` rather than their own `#[path]` copy.

Test names carry the suite module: a test `t` in `tests/sim_force.rs` is
`sim_force::t` in binary `sim_a_m`. Select a suite by name prefix:

```sh
scripts/run-tests.sh --test sim_force                 # translated (below)
cargo nextest run -E 'test(/^sim_force::/)'
cargo test --test sim_a_m sim_force::                 # plain cargo test filter
```

`scripts/run-tests.sh --test <suite>` translates a grouped suite into its group
binary plus `test(/^<suite>::/)`; binaries named directly stay whole and any `-E`
filtersets are intersected with that selection. Under nextest every test still
runs in its own process. Plain `cargo test` runs a whole group in one process, so
process-global changes go through the shared harness lock or a standalone binary.

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

### Suite contracts

- `support/sim_cli.rs` suites require CMake, isolated child directories, default
  and `--no-opt` public CLI runs, exact stdout, expected stderr and status, and
  specific frontend/lowering/runtime diagnostics. Positive migration or
  unsupported-feature rejection is a failure, never a skip or success
  substitute. Keep each suite's frontend, timeout and sanitizer contracts.
- Every `llg` run lints. Expectations ignore lint warning/info lines and the
  count line (`sim_harness::is_compile_report_line`,
  `sim_harness::strip_lint_reports`) like frontend `Warning:` lines; lint errors
  still fail. A fixture that deliberately mixes blocking and nonblocking
  assignments or has combinational feedback is listed with its exact errors in
  `fixtures/sim/lint/expected_errors.tsv`: `sim_cli` first requires the default
  run to stop with exactly those errors, then reruns with the
  `fixtures/sim/lint/allow_*.toml` config and checks the expectations. Add such
  fixtures to the manifest instead of passing a config by hand; custom harnesses
  use `sim_cli::expected_lint`.
- Oracles use explicit truth tables, bit strings and width/sign arithmetic, never
  production helpers. Keep specification-derived X/Z, scope, signedness and
  capacity boundaries. Expected-output changes need a feature/clause
  justification and are never blessed automatically. Prefer behavioral
  assertions to private generated-name substrings.
- `support_harness` checks CWD/mutex unwind recovery and descendant timeout
  cleanup (POSIX process groups with kill, Windows `taskkill /T`).
- `sim_dynamic_ownership` is positive public-HDL acceptance: keep its runtime/
  waveform assertions, exact CTest inventories and cleanup tests; smaller
  components, parser-only checks or recognized source shapes do not replace them.
  `runtime_value_storage/validate.py --full` requires Rust, real emitted-model
  tests, CLI acceptance and repository tests; missing prerequisites are blocked,
  not passed, and zero/missing/duplicate/disabled tests or stale products are
  rejected. Component sanitizer probes and generated-model sanitizer execution
  are separate gates.
- `slang_frontend`, `slang_semantics` and `model_tests` independently exercise
  safe capture, typed relationships, complete delays, declaration-owned ranges,
  source topology and copy-before-destroy; downstream tests use validated DB/IR
  builders. Unsupported capture is never silently complete. `emit_decoupling`
  source checks and fragment-order checks are architecture checks, not execution.
- `property_elab` mirrors identical Rust value vectors in the C self-test; keep
  independent arithmetic/real-conversion/net-resolution probes. `runtime_values`
  compiles values without the scheduler/coroutine runtime; random tests cover
  Annex N and process-stream rules with explicit seeds/ranges at -O0/-O2.
- `sim_cmake` keeps its missing-CMake graceful skips and its source-generation,
  generator, configure-error and stale-source checks. Native DPI shared-library
  cases skip on non-Unix or missing CMake/C compiler. Never extend these skips to
  mandatory public conformance suites.
- Preserve supplied imported sources and provenance. C VPI fragments are not
  standalone plugins; handwritten output-shape probes are not Rust-emitted C.
  Recreated missing inputs are replacement witnesses, not recovered evidence.

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

### Feature completion checklist

Complete each affected step and record a reason when a step or failure case
does not apply; the owning source guides hold the details.

- Capture through `Db::from_slang`, copying values, relationships and source
  locations into owned descriptors; prove consumers survive native snapshot
  destruction.
- Add typed semantic/execution operations and validate IDs, types, shapes and
  contexts. Extend effect/dependency analysis and every optimizer traversal.
- Emit from owned `ExecutionModel` data through the whole-model emitter. A legacy
  C fragment or handwritten runtime probe alone is not feature acceptance.
- Define runtime initialization, copy/alias ownership, suspension, cancellation
  and destruction. Feature code uses public `sv4_*` operations and the neutral
  `llg_sv4_*` bridge ([value facade](../src/sim/rt/value/facade.md)), including
  independent packed-value owners; never inspect private payloads.
- Embed new runtime fragments in `src/sim/rt/mod.rs` and the generated-model
  CMake inputs; runtime C stays out of Rust binaries. Keep fragment-order tests,
  `emit_decoupling` and `generated_c_frame_lint` active.
- Add checked-in positive, nearest-illegal, boundary and composition fixtures to
  the task module ([slices](#feature-completion-slices)), with public CLI
  acceptance and independent clause-derived oracles; missing tools block
  acceptance. Label parser/DB/IR/handwritten-C tests `component_*` and report
  them separately from generated-model execution.

| Failure or boundary | Required coverage when affected |
| --- | --- |
| Invalid IDs/shapes | Focused owned DB/IR validator tests reject malformed descriptors before emission. |
| Allocation failure | Use the allocator's existing fault-injection path; verify partial-initialization cleanup and unchanged destinations. If none exists, report the missing probe rather than infer success. |
| Suspension/cancellation | Public HDL cancels at each new suspension point; verify wakeup detachment, no late publication and exactly-once cleanup. |
| Aliasing | Public HDL covers overlapping source/destination, copied values and captured values surviving mutation. |
| Source maps/lifetimes | Generate after snapshot destruction and build/execute after Db destruction with `sim_cli::run_case_after_db_drop`; check surviving fixture identity. Add source-location assertions for new descriptors. |
| Optimizers/native builds | Public fixtures run with default and `--no-opt`; representative affected paths also run `--model-opt-level O0` and `O3`. The owned-lifetime helper runs all four combinations. |

Serialize edits to shared Db/IR enums and validators, facade headers and neutral
bridges, emitter facades, scheduler roots, `src/sim/rt/mod.rs`, Cargo/CMake/build
files and suite declarations: coordinate the hunk with its owner and the
integration orchestrator, keep changes additive and small, and use separate
native build trees.

### Feature completion slices

New tasks use `fixtures/sim/feature_completion/<task_slug>/` (ASCII slugs such as
`rtl_001`, `rtl_002b` for a follow-up slice, or `ki_packed_range` for a
known-issue fix) and `sim_feature_completion/<task_slug>.rs`, explicitly declared with
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
  -E 'binary(sim_feature_completion) | test(/^emit_decoupling::/) | test(sim::rt::tests::)' \
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
test(rtl_006::)'`. The
arithmetic expectations come from the in-test limb oracle
(`sim_feature_completion/rtl_006/oracle.rs`) at widths 1 through 129 and at the
8,128/8,129-bit kernel threshold, in both optimizer modes on every backend.

RTL-007's zero-time call and evaluator fixtures use
`-E 'binary(sim_feature_completion) & test(rtl_007::)'`. Positive fixtures run in
both optimizer modes on both backends; stateful event helpers assert the documented process-evaluation
policy and check evaluation counters only as lower bounds. RTL-007b's effectful
helpers in force sources, intra-assignment/NBA/`->>` controls, named-event and
real lists and `$monitor`/`$strobe` use
`-E 'binary(sim_feature_completion) & test(rtl_007b::)'` with the same lanes;
the runtime queries are also covered by `sim_counter::sim_rt_selftest`.

RTL-008's initialization, package/`$unit` and let fixtures use
`-E 'binary(sim_feature_completion) & test(rtl_008::)'`. Single-file positives run
in both optimizer modes on both backends; multi-file package and `$unit` cases run in separate and merged
compilation-unit modes through `run_case_with_source_prefix`. Cross-scope
initialization oracles are order-independent or follow a static read
dependency; the Verilog-2001 race fixture asserts its allowed result set.


RTL-009's port fixtures use `-E 'binary(sim_feature_completion) & test(rtl_009::)'`.
Positive fixtures run in both optimizer modes on both backends and after Db
destruction; descriptor ports also bound generated model size. Negatives cover
assignability, ref shape, variable inout, runtime ref/net/inout selects and
output-target writer conflicts.

RTL-102's modport expression-port fixtures use
`-E 'binary(sim_feature_completion) & test(rtl_102::)'`. Positive fixtures run in
both optimizer modes on every backend
and after Db destruction. Negatives cover non-lvalue outputs, input writes,
non-constant selectors, writer conflicts through expression ports and the
deferred virtual-interface access.

RTL-015's bit-stream and streaming fixtures use
`-E 'binary(sim_feature_completion) & test(rtl_015::)'`. Positive fixtures run in
both optimizer modes on both backends,
most also after Db destruction; expectations are independent bit-string
derivations. `target_bounds` prints its results to stderr, compared exactly with
the run-time bounds reports.

RTL-103's streaming and pattern fixtures use
`-E 'binary(sim_feature_completion) & test(rtl_103::)'`. Positive fixtures run in
both optimizer modes on every backend,
the main ones also after Db destruction; `copyout_bounds` and `mixed_state`
print their results to stderr, compared exactly with the run-time bounds
reports. Expectations are hand derivations of the stream and pattern rules.

The positional pattern row and for-step update fixtures use
`-E 'binary(sim_feature_completion) & test(ki_pattern_rows::)'`: small dense,
descriptor and nested row targets (blocking, NBA and continuous) and overloaded
`for` steps on packed, native and descriptor targets run in both optimizer
modes on every backend; a row shape
mismatch, a runtime continuous row and an automatic NBA row reject. The RTL-103
and these fixtures also run under the generated-runtime sanitizers below with
`-E 'test(rtl_103::) | test(ki_pattern_rows::)'`.

RTL-018's library, configuration and bind fixtures use
`-E 'binary(sim_feature_completion) & test(rtl_018::)'`, with
`test(/^sim_syn032_library_configs::/)` and `test(/^sim_syn033_structural_bind::/)`.
Companion maps, library sources and configurations are named through
`sim_cli::run_case_with_inputs`/`reject_case_with_inputs`, whose input lists
the fixture checker treats as references. The composition runs in both
compilation-unit modes on every backend and, through `sim_cli::run_compile_opts_after_db_drop`, after snapshot and
Db destruction.

RTL-019's source-mapping and edition-admission fixtures use
`-E 'binary(sim_feature_completion) & test(rtl_019::)'`, with
`test(/^sim_edition::/)`, `test(/^sim_syn017_directive_effects::/)` and the LSP
binaries for shared source-map/edition changes. Mapped `__FILE__`/`__LINE__`
values and runtime locations are counted by hand; the mapping and
resumed-task fixtures run on every backend and after snapshot and Db destruction. Each `neg_2001_*.v` holds one
later form that must reject under `--edition v2001` and compile under 2009.

RTL-106's mapped-diagnostic and strict-2001 fixtures use
`-E 'binary(sim_feature_completion) & test(rtl_106::)'`, with
`test(line_mapped) | test(lint_diagnostics_add)` for the LSP projection.
Each `neg_*.v` holds one later form (several built by macros) or variable
driver, rejects under 2001 at an exact `file:line:col` and compiles under 2009;
`legal_2001.v` executes on every backend in both
editions. To re-audit the 2001 profile, run every `.v`/`.sv` fixture with
`llg --edition v2001 --gen-only` and compare the strict-edition diagnostics with
the previous revision: no fixture accepted before may newly reject unless it is
language-illegal.

SIM-003's native record fixtures use
`-E 'binary(sim_feature_completion) & test(sim_003::)'`. Positive call,
lifetime and sensitivity fixtures run in both optimizer modes on every backend
and after Db destruction; the
borrowed-chandle fixture builds its companion C file into a DPI library (Unix
hosts). Component checks are `native_value_descriptors_probe.c` in
`runtime_value_storage` and the `sim::ir::validate::tests::native_values` unit
tests.

SIM-004's native assignment, link and conditional fixtures use
`-E 'binary(sim_feature_completion) & test(sim_004::)'`. Positive fixtures run in
both optimizer modes on every backend;
single-file ones also run after Db destruction, and the chandle fixture builds
its companion C file into a DPI library (Unix hosts).

SIM-005's real reference, real-array and real-sampling fixtures use
`-E 'binary(sim_feature_completion) & test(sim_005::)'`. Positive fixtures run
in both optimizer modes on every backend and, except the numeric-extremes fixture (which also runs at model
`-O0`/`-O3`), after Db destruction. Component checks are the
`sim::ir::validate::tests::real_values` unit tests.

SIM-035's sampled-value fixtures use
`-E 'binary(sim_feature_completion) & test(sim_035::)'`. Positive fixtures run
in both optimizer modes on every backend; `complex_clocks` also runs at model
`-O0` and after Db destruction. Component checks are the
`sim::ir::validate::tests::sampled_*` unit tests and the sampled-domain ring
checks in `runtime_value_storage`'s `retention_probe.c`.

SIM-034's synchronous-drive fixtures use
`-E 'binary(sim_feature_completion) & test(sim_034::)'`. Positive fixtures run
in both optimizer modes on every backend; `drive_timeline` and `vif_drives` also
run at model `-O0`/`-O3` after Db destruction. Their decisions S34-D1-D4 are in
`sim_lrm_decisions`; the component check is
`sim::ir::validate::tests::clocking_drive_cycle_counts_and_instance_dispatch_are_checked`.

SIM-040's DPI-C packed, sized and open-array fixtures use
`-E 'binary(sim_feature_completion) & test(sim_040::)'`. Each positive fixture
builds its checked-in C companion into a shared library (strict C11, with
`$LLG_CFLAGS`; a DLL on Windows, as for SIM-041 below) and runs in both
optimizer modes on every backend; negatives cover frontend, codegen and
link-time errors, and `model_import_library_is_explicit_about_the_toolchain`
checks `llg --dpi-import-lib` (an import library naming `sim.exe` under MSVC,
an explicit error elsewhere). Decisions S40-D1-D5,
with `.c` companions, are in `sim_lrm_decisions`; component checks are the
`sim::build::dpi_link::tests` unit tests.

SIM-041's DPI-C export and context call-chain fixtures use
`-E 'binary(sim_feature_completion) & test(sim_041::)'`. Positives build their
companions as shared libraries and run in both optimizer modes on every
backend; `scopes` also links its companion as a static archive and `chains`
also runs with `-funsigned-char` (MSVC `/J`). On Windows every shared companion
is a DLL built with `/std:c11 /W4 /WX /LD` against the design's import library
(`sim_cli::model_import_library`, `llg --dpi-import-lib`; see
`c_compiler::shared_library`), and the undeclared-export and `svGetTime`
negatives check the DLL link's error instead of the model link's. The
runtime negatives share `errors.c` and must stop with a "DPI-C protocol
violation". Decisions S41-D1-D8 are in `sim_lrm_decisions`; the component
check is `sim_dpi::dpi_exports_lower_to_one_entry_and_sorted_scopes`. Under
ASan run the set with `LLG_CFLAGS='-fsanitize=address,undefined'`, which also
instruments the companions.

SIM-042's VPI fixtures use
`-E 'binary(sim_feature_completion) & test(sim_042::)'`. Each `.sv` has a C
application that registers its system tasks and functions; positives load it
with `LLG_VPI_PLUGIN` (on Windows a DLL linked against a model import library)
and run in both optimizer modes on every backend; `objects` and `systf` also
link it as a static archive through `--dpi-lib`, `values` also runs with
`-funsigned-char` (MSVC `/J`), `systf` also runs a model generated after the frontend is
dropped, and `missing` must stop at startup. `errors` includes the handle
churn that the ASan lane exercises
(`LLG_CC=gcc LLG_CFLAGS='-fsanitize=address,undefined -DLLG_CO_DEBUG'`).
Decisions S42-D1-D13, with `.c` applications linked as static archives
(`tests/support/c_compiler.rs::static_archive`), are in `sim_lrm_decisions`;
the runtime probe is `runtime_value_storage`'s `vpi_ownership` and the unit
checks are `sim::build::vpi_static::tests` and
`sim::codegen::lowering::collection::vpi::tests`.

ADV-019 families 1 to 4 (typespecs, structure and union members, structure,
expression and statement objects) use
`-E 'binary(sim_feature_completion) & test(adv_019::)'`: `typespecs` prints the
typespec tree of each declaration, `members` reads, finds, watches and
writes members and `structure` walks ports, processes, assignments, tasks,
primitives, instance arrays and named events, all linked as static archives;
`timing` (VPI-T1) walks module paths and timing checks with their terms;
`expressions` walks expression trees with their values and `vpiDecompile`,
and `statements` the statement trees of processes, tasks and functions.
Decisions A19-D1..D13 are in
`sim_lrm_decisions` (`-E 'test(/a19_d/)'`); the unit checks are
`core::db::database::capture::tests`,
`sim::codegen::lowering::collection::vpi::tests`,
`sim::codegen::lowering::collection::vpi::timing::tests` and
`sim::emit_c::model::vpi::tests`; `sim_043`'s
`generated_model_omits_the_catalog_by_default` checks that the code tables
exist only in VPI models.

ADV-017's assertion API fixtures use
`-E 'binary(sim_feature_completion) & test(adv_017::)'`: `lifecycle`
correlates assertion callbacks with HDL action counters and the 40.5.3
counters (also linked as a static archive), `control` drives every
`vpi_control` assertion operation, `objects` checks the 39.3 static
information and reentrant callbacks, and `relations` (`--vpi`) walks the
37.45-37.51 relations, sequence and property instances and declarations,
`failExpr` and instance callbacks. Decisions A17-D1..D8 are in
`sim_lrm_decisions` (`-E 'test(/a17_d/)'`).

ADV-020's VPI parity fixtures use
`-E 'binary(sim_feature_completion) & test(adv_020::)'`. `symbols` takes the
address of every Annex K/M routine and runs as an `LLG_VPI_PLUGIN` plugin (a
Windows DLL linked against the model import library) and as a static archive
(every host); `mcd` exercises `vpi_mcd_*` on the `$fopen` table;
`value_array` reads and writes array sections with
`vpi_get_value_array`/`vpi_put_value_array`; `force_callbacks` and `disable`
cover `cbForce`/`cbRelease`/`cbAssign`/`cbDeassign` and `cbDisable`; `delays`
(VPI-T1) reads and writes path, check, primitive and continuous assignment
delays and counts `cbTchkViolation` records. Decisions
A20-D1..D15 are in `sim_lrm_decisions` (`-E 'test(/a20_d/)'`); the export list
check is `sim::build::import_library::tests`.

SIM-043's callback fixtures use
`-E 'binary(sim_feature_completion) & test(sim_043::)'`. `regions`, `reentry`,
`readonly` and `control` load their C application with `LLG_VPI_PLUGIN` and
run in both optimizer modes on every backend; `regions` also links it as a
static archive, `control` also runs with `--stop-policy exit`, `start_control`
runs its start-callback stop and finish (selected by `SIM043_START_CONTROL`)
under both stop policies, and `catalog` runs with and without `--vpi`.
Decisions S43-D1-D10 are in
`sim_lrm_decisions`; the CLI check is
`settings::tests::vpi_catalog_follows_vpi_and_linked_libraries`. Under ASan
run the set with `LLG_CC=gcc LLG_CFLAGS='-fsanitize=address,undefined
-DLLG_CO_DEBUG'`.

SIM-044's covergroup fixtures use
`-E 'binary(sim_feature_completion) & test(sim_044::)'`. Positive cases run in
both optimizer modes on every value backend with hand-derived percentages
(fixture readme); `embedded` and `dropped_waiters` run under
`LLG_GC_STRESS=1` to prove the owner and handle arguments of covergroup
objects, and the per-instance event waiters that hold them, stay reachable,
and `storage` keeps variables read only by coverpoints, guards and sampling
events. Decisions S44-D1-D9 are in `sim_lrm_decisions`; the runtime store probe is
`runtime_coverage` in the `runtime` binary, and the capture unit test is
`core::db::database::coverage::tests`.

SIM-045's bins, transition, cross and option fixtures use
`-E 'binary(sim_feature_completion) & test(sim_045::)'`, with the same
matrix and hand-derived percentages; decisions S45-D1-D18 are in
`sim_lrm_decisions` (`-E 'test(/s45_d/)'`), and the transition automaton unit
tests are `sim::codegen::lowering::coverage::transitions::tests`.

SIM-046's query, database and coverage-API fixtures use
`-E 'binary(sim_feature_completion) & test(sim_046::)'`. Database tests run a
model several times in one scratch directory (`sim_cli::run_case_sequence`),
and the corrupt-database cases are built in Rust from the format documented
in `src/sim/rt/coverage/database.c`; `vpi_coverage` loads a VPI application
from `LLG_VPI_PLUGIN`. Decisions S46-D1-D12 are in `sim_lrm_decisions`
(`-E 'test(/s46_d/)'`).

ADV-002's delay fixtures use
`-E 'binary(sim_feature_completion) & test(adv_002::)'`. The Verilog sources
run in both editions, both optimizer modes and on every value backend, with
outputs hand-derived from V §§4.3, 6.1.3, 7.14 and SV §§10.3.3, 11.12, 28.16
(fixture readme); SystemVerilog-only sources (aliases, patterns) run in
sv2009. Decisions S2-D1-D9 are in `sim_lrm_decisions` (`-E 'test(/s2_d/)'`).

ADV-007's specify and module-path fixtures use
`-E 'binary(sim_feature_completion) & test(adv_007::)'`
([readme](fixtures/sim/feature_completion/adv_007/readme.md)). The Verilog
sources run in both editions, both optimizer modes and on every value
backend, with outputs hand-derived from V §§3.11.3, 14.2-14.4 and SV §§6.20.5,
30.4-30.6 (Tables 30-2 and 30-3); `wired.v` runs in sv2009 and is rejected in
v2001 (14.2.1); the negatives pin the frontend's and llg's located errors.
Decisions A7-D1-D6 are in `sim_lrm_decisions` (`-E 'test(/a7_d/)'`). Unit
tests: `core::db::database::specify::tests`, the ffi specify decode test,
`sim::ir::timing::tests`, the IR validation test
`module_path_sets_check_sources_destinations_and_ranges`, the emitter test
`module_path_tables_bind_only_their_destination_nets` and the runtime selftest
`test_module_paths` (`sim_counter::sim_rt_selftest`). Under ASan run the set
with `LLG_CC=gcc LLG_CFLAGS='-fsanitize=address,undefined
-fno-omit-frame-pointer -fno-sanitize-recover=all -DLLG_CO_DEBUG'`.

ADV-008's state-dependent, edge-sensitive and pulse-filtering fixtures use
`-E 'binary(sim_feature_completion) & test(adv_008::)'`
([readme](fixtures/sim/feature_completion/adv_008/readme.md)): Verilog sources
in both editions, `edge_any.sv` in sv2009, both optimizer modes and every
value backend, outputs hand-derived from V §§14.2.3-14.2.4, 14.6 and SV
§§30.4.3-30.4.4, 30.7; `options.v` also runs with `--pulse-reject`,
`--pulse-error`, `--pulse-style`, `--show-cancelled` and the `[simulator]`
keys. Decisions A8-D1-D9 are in `sim_lrm_decisions` (`-E 'test(/a8_d/)'`).
Unit tests: `path_pulses_keep_limits_and_resolved_terminals`, the IR
validation and emitter tests above plus
`module_path_triggers_limits_styles_and_options_are_emitted`, the settings and
config pulse tests, and the runtime selftest `test_module_path_pulses`.

ADV-009's timing check fixtures use
`-E 'binary(sim_feature_completion) & test(adv_009::)'`
([readme](fixtures/sim/feature_completion/adv_009/readme.md)): Verilog sources
in both editions, both optimizer modes and every value backend, outputs
hand-derived from V §§15.1-15.7 and SV §§31.1-31.8 printing notifier values;
the stderr violation reports, the duplicate-check warnings, the
`--no-timing-checks`/`--no-notifiers`/`--no-timing-check-messages` options and
their `[simulator]` keys and a Db-dropped run are checked as well. Decisions
A9-D1-D13 are in `sim_lrm_decisions` (`-E 'test(/a9_d/)'`). Unit tests:
`timing_checks_keep_events_edges_conditions_limits_and_notifiers`, the FFI
timing check decode, the IR validation test `timing_check_sets_check_terminals_events_and_notifiers`,
the emitter test `timing_check_tables_are_emitted_only_for_designs_with_checks`,
the lint activity tests, the settings and config timing tests and the runtime
selftest `test_timing_checks`.

ADV-010's combined and negative timing check fixtures use
`-E 'binary(sim_feature_completion) & test(adv_010::)'`
([readme](fixtures/sim/feature_completion/adv_010/readme.md)): Verilog sources
in both editions, both optimizer modes and every value backend, run with and
without `--neg-timing-checks` (and with `--no-timing-checks` and the
`[simulator] neg_timing_checks` key), outputs hand-derived from V §§15.2-15.8
and SV §§31.3-31.9 printing notifier change counts, last change times and
delayed signal traces; the violation report texts and times, the settlement
and adjusted-limit warnings and the located errors for invalid delayed
signals and skew flags are checked as well. Decisions A10-D1-D10 are in
`sim_lrm_decisions` (`-E 'test(/a10_d/)'`; the "option" cases run with
`--neg-timing-checks`). Unit tests: the delay solver tests in
`codegen/lowering/collection/timing_checks.rs` (Figure 31-6 delays, edge
classes, inconsistent limits), the Db test
`combined_checks_keep_conditions_and_delayed_signals`, `neg_timing_checks` in
the settings and config timing tests and the runtime selftests `test_timing_checks`
(delayed line, doorbell run) and `test_timing_skew_timers` (an annotated
limit moves or expires an open skew window without a stale report).

ADV-011's SDF reader uses `-E 'binary(sim_feature_completion) & test(adv_011::)'`
([readme](fixtures/sim/feature_completion/adv_011/readme.md)): the annotation
targets of `cells.v`, accepted SDF 2.1/3.0 files resolved to plans, files
read with warnings and malformed files with their exact first error;
`LLG_SDF_BLESS=1` rewrites the goldens for review. Unit tests:
`--lib -E 'test(/sim::sdf::/)'` (lexer, numbers, conditions, parser, resolver
Table 32-2 mapping, catalog names, small read buffers).

ADV-012's SDF annotation uses `-E 'binary(sim_feature_completion) & test(adv_012::)'`
([readme](fixtures/sim/feature_completion/adv_012/readme.md)): path delays and
pulse limits, every timing check entry, negative limits re-solved, member
selection, scaling, configuration and log files, DEVICE, interconnect,
`--sdf` and diagnostics, with hand-derived expected output. The decision
cases `A12-D*` (`-E 'test(/sim_lrm_decisions::a12_/)'`) carry their SDF and
configuration files beside the `.sv`. Valuing unit tests:
`--lib -E 'test(/sim::sdf::apply/)'`.

ADV-016's randsequence fixtures use
`-E 'binary(sim_feature_completion) & test(adv_016::)'`. Positives run in both
optimizer modes on every backend with oracles from SV 18.17 (fixture readme):
the clause's outcome lists, chi-square bounds for weights and join steps, and
structural relations for random grammars; `seed_replay` compares its
generator-dependent checksum across the six runs only. `depth_limit` and
`invalid_runtime` must end with exit status 1 and located diagnostics.
Decisions S16-D1-D9 are in `sim_lrm_decisions` (`-E 'test(/s16_d/)'`); the
capture unit tests are `core::db::database::randsequence::tests`. Under ASan
run `suspend_kill` and `recursion` with `LLG_CC=gcc
LLG_CFLAGS='-fsanitize=address,undefined -DLLG_CO_DEBUG'`.

SIM-047's waveform fixtures use
`-E 'binary(sim_feature_completion) & test(sim_047::)'` together with
`-E 'test(/^sim_waveform::/) | test(/^sim_lrm_decisions::s47_/)'` and the
waveform probes `runtime_value_storage::dynamic_storage_and_waveform`. Every VCD
is read by the independent reader `support/vcd.rs` and compared with
hand-derived end-of-step traces in both optimizer modes; `trace_values` also
compares the VCD bytes on every value backend, FST goes through GTKWave's
reader built from the bundled libfst, `excluded` runs under `LLG_GC_STRESS=1`,
`flush_read` needs `LLG_ALLOW_SYSTEM=1` (set by the test), and
`the_storage_pass_never_changes_a_dump` builds and runs any model the
unused-storage pass changes. Decisions S47-D1-D13 are in `sim_lrm_decisions`.
Under ASan run the set with `LLG_CC=gcc LLG_CFLAGS='-fsanitize=address,undefined
-fno-omit-frame-pointer'`.

ADV-026's string-sourced memory loads (`$sreadmemb`/`$sreadmemh`) use
`-E 'binary(sim_feature_completion) & test(adv_026::)'`
([readme](fixtures/sim/feature_completion/adv_026/readme.md)). Each positive
loads a memory from strings and from the data file holding the same text and
compares them (`equivalence*`, `contexts*`); `notify` compares the per-time-step
event sets of a string load and a file load, including VPI value-change
callbacks; `diagnostics*` check every warning/error category and that nothing is
written outside the range; the negatives pin the located rejections. Positives
run in both optimizer modes on every backend, and `.v` fixtures in both
editions. The native `memory_image` probe has `text`/`textmalformed` modes for
the text source. Decisions S26-D16..D20 are in `sim_lrm_decisions`. The CI
sanitizer job includes `adv_026`.

SIM-099's gate fixtures use
`-E 'binary(sim_feature_completion) & test(sim_099::)'`. The integrated
testbench runs with its DPI-C model and VPI application linked as static
archives (every host), as a shared companion plus `LLG_VPI_PLUGIN` (skipped
where `sim_040::plugin_library` cannot build one) and under
`LLG_GC_STRESS=1 LLG_GC_VERIFY=1`; `selected_writes` compares each time step's
lines as a set (SV 4.7); `random_replay` compares its generator-dependent
checksum across the twelve runs only. `builtin_inventory_is_complete` reads
`vendor/slang/scripts/systemnames.txt` and every simulator fixture (about
0.2 s) and fails when a built-in is neither called by a positive fixture nor
classified in `sim_099/builtin_inventory.tsv`. The CI sanitizer job includes
`sim_099`; locally run the set with `LLG_CC=gcc
LLG_CFLAGS='-fsanitize=address,undefined -DLLG_CO_DEBUG'`.

The class-defects fixtures (named blocks and class parameters in class
methods) use `-E 'binary(sim_feature_completion) & test(ki_class_defects::)'`;
decisions CD-D1 and CD-D2 are in `sim_lrm_decisions`. Locally run them under
the same sanitizer flags as `sim_099`.

SIM-037's sequence-composition fixtures use
`-E 'binary(sim_feature_completion) & test(sim_037::)'`. `exhaustive` compares
36 sequences on all 1024 two-signal traces of length 5 with the test-side
Annex F interpreter in `sim_037.rs`; the other positives are hand-derived match
sets. All positives run in both optimizer modes on every backend, compared as
sorted line multisets. `budget` also runs with a small
`LLG_SEQUENCE_THREAD_LIMIT`. The component check is
`sim::ir::validate::tests::sequence_joins_*`.

SIM-006's container, record-element, handle-array and associative-default
fixtures use `-E 'binary(sim_feature_completion) & test(sim_006::)'`. Positive
fixtures run in both optimizer modes on every backend; fixtures without runtime warnings also run after Db
destruction, and the two warning fixtures compare their SV 7.8.6 warnings with
stderr exactly. Component checks are `container_values_probe.c` in
`runtime_value_storage` and the
`sim::ir::validate::tests::subroutine_containers` unit tests.

SIM-018's collector fixtures use
`-E 'binary(sim_feature_completion) & test(sim_018::)'`. Positive fixtures run
in both optimizer modes on both backends, once with the default policy and once
with `LLG_GC_STRESS=1 LLG_GC_VERIFY=1` (collect at every safe point, poison
instead of free), so a missed root fails with a located diagnostic.
`LLG_GC_STATS=1` statistics are the live-memory oracle of the plateau, container
and deep-list fixtures. The component check is `gc_probe.c`
(`runtime_object_collector`) in `runtime_value_storage`, which also covers
injected collector allocation failures.

SIM-019's container-method fixtures use
`-E 'binary(sim_feature_completion) & test(sim_019::)'`. Positive fixtures run
in both optimizer modes on both backends; the negative fixtures check the
frontend diagnostics of illegal methods, reductions and arguments. The
component check is `method_gather_probe.c` in `runtime_value_storage`.

ADV-032's unsupported-construct fixtures use
`-E 'binary(sim_feature_completion) & test(adv_032::)'`. Each negative fixture
asserts the exact `error: <file>:<line>:<col>: unsupported: <construct>
(<family>) is not supported by llg` lines, exit status 1, an empty stdout and
an empty `--out-dir` (no C generation) in both optimizer modes and in both
editions where the syntax exists. Procedural `assign`/`deassign` is rejected in
every form (`assign_forms`, `assign_hierarchical*`); `queue_supported` pins the
supported `$q_*` subset, and `hier_continuous_assign*` and `hier_force_release`
pin hierarchical continuous assignment and force/release on all value backends
and optimizer modes, each paired with a nearest-illegal located language error. The
PLI 1.0 cases refuse a `--dpi-lib` that names `veriusertfs` or a `tf_*`/`acc_*`
routine (tool-free stand-in libraries, plus shared objects built with the
host C compiler on Unix, reported as `SKIP` without one). The language-server
side is `-E 'binary(general) & test(legacy_constructs)'`: the server must serve
these files without error diagnostics.

RTL-016's tagged-union, pattern and Q03 fixtures use
`-E 'binary(sim_feature_completion) & test(rtl_016::)'`. Positive fixtures and
the run-time error fixtures run in both optimizer modes on both backends. Run-time error fixtures print their
results to stderr, compared exactly with the reports; the interprocess Q03 race
asserts its allowed result set.

RTL-010's continuous topology fixtures use
`-E 'binary(sim_feature_completion) & test(rtl_010::)'`. Positive fixtures run in
both optimizer modes on both backends
and after Db destruction; descriptor scatter also bounds generated model size.
Negatives cover overlapping variable writers, runtime pattern-leaf selects and
zero-time nonconvergence; the delayed pattern driver boundary now runs as a
positive case (ADV-002).

RTL-011's alias and inout-collapse fixtures use
`-E 'binary(sim_feature_completion) & test(rtl_011::)'`, together with
`test(/^sim_port_net_types::/)` and `test(net_collapse::)`. Positive fixtures run in
both optimizer modes on both backends;
warning-free ones except the waveform fixture also run after Db destruction.
Collapse warnings carry absolute paths, so the module compares their text up to
the location, and the permuted chain twin must print the same values and
warnings. `alias_identity`
also parses its VCD. Negatives cover uwire drivers, pass switches and the alias
legality rules; the `inout uwire` formal is positive since RTL-105.

SIM-001's region and procedural-time fixtures use
`-E 'binary(sim_feature_completion) & test(sim_001::)'` with
`test(/^runtime_regions::/)` for the native region-callback probe. Every executed fixture
runs in both optimizer modes on both backends. `sim_cli::run_case_checked_matrix` hands each output to a checker, so the
region litmus is compared as a line multiset plus the orders the §4.5 reference
algorithm fixes, races accept every permitted outcome, and tick-limit failures
compare the output printed before the diagnostic.

SIM-002's time-reporting fixtures use
`-E 'binary(sim_feature_completion) & test(sim_002::)'`. Executed fixtures run in
both optimizer modes on both backends;
`.v` fixtures also run with `--edition v2001`, and the scope fixtures also run after
the native snapshot and Db are dropped.

RTL-012's strength and `unconnected_drive` fixtures use
`-E 'binary(sim_feature_completion) & test(rtl_012::)'`. The scalar matrix and
the 70-bit composition compare against the exhaustive outcome oracle in
`sim_feature_completion/rtl_012/oracle.rs`; positives run in both optimizer
modes on both backends, the matrix
and tristate fixtures in both editions. Negatives cover supply-net, selected
vector-bit, highz-pair and pullup-strength0 forms.

RTL-014's fixed loop, method, membership and query fixtures use
`-E 'binary(sim_feature_completion) & test(rtl_014::)'`. Positive fixtures run in
both optimizer modes on both backends,
most also after Db destruction; descriptor methods also bound generated model
size. Unspecified equal-key and unknown-key orders are checked only through
permutation invariants. Negatives cover read-only iterators and receivers,
reverse `with`, record sort and set members, iterator dimensions, IEEE 2001
methods and the packed limit for row keys.

RTL-013's sensitivity and always-writer fixtures use
`-E 'binary(sim_feature_completion) & test(rtl_013::)'`. Positive fixtures run in
both optimizer modes on both backends,
most also after Db destruction; wake counters compare changes after the first
sample, and descriptor processes also bound generated model size. Negatives
cover timing, forks and event-control counts in always_comb/always_latch/
always_ff and overlapping member, cell, wide-row, ref-port, hierarchical and
called-function writers.

RTL-020's combinational UDP fixtures use
`-E 'binary(sim_feature_completion) & test(rtl_020::)'`, together with
`test(/^sim_udp::/)` and `test(emit_c::owned::tests::udp::)`. Positive fixtures run
in both optimizer modes on both backends, `.v` sources also as Verilog-2001, and three after Db destruction. The
exhaustive table sweep compares with the all-matching-rows oracle in
`sim_feature_completion/rtl_020.rs`. Negatives cover vector definition ports,
row widths, conflicting rows, terminal counts, empty terminals, mis-sized array
connections and vector or aggregate terminals of single instances.

ADV-003's sequential UDP fixtures use
`-E 'binary(sim_feature_completion) & test(adv_003::)'`, together with
`test(/^sim_udp::/)`, `test(emit_c::owned::tests::udp::)`,
`test(ir::validate::tests::udp::)` and the `S3-D*` cases of
`sim_lrm_decisions`. Positive `.v` fixtures run in both editions, both
optimizer modes and on the legacy, compact/portable and compact/GMP value
backends, two of them after Db destruction; the hand-derived oracles quote
IEEE 1364-2001 8 and IEEE 1800-2009 29 ([readme](fixtures/sim/feature_completion/adv_003/readme.md)).
Negatives are single-fault sources that stop with the frontend's located
diagnostic in both editions, plus a vector actual on a single instance.

ADV-005's pass-switch fixtures use
`-E 'binary(sim_feature_completion) & test(adv_005::)'`, together with
`test(ir::validate::tests::switch_networks::)` and the `A5-D*` cases of
`sim_lrm_decisions`. Positive `.sv` fixtures (plain Verilog-2001) run in both
editions, both optimizer modes and on the legacy, compact/portable and
compact/GMP value backends; the readme quotes IEEE 1364-2001 5.6.5 and 7.6 and
IEEE 1800-2009 4.9.5, 28.8 and 28.12-28.15
([readme](fixtures/sim/feature_completion/adv_005/readme.md)). The A01 test
generates 160 random networks over four phases and compares every node's `%v`
with an in-test oracle that solves each conducting/nonconducting combination
of the unknown switches by a strongest-first simple-path search, in both
optimizer modes. Negatives cover MOS switches, wired-logic nodes, wide and
mis-sized array terminals, `tran` delays and variable terminals.

RTL-099's gate fixtures use `-E 'binary(sim_feature_completion) & test(rtl_099::)'`.
They pin the output-port writer rule for constant slices of descriptor-backed
arrays: disjoint procedural writers execute in both optimizer modes on both
backends and after Db destruction,
and procedural, second-port and always_ff writers overlapping a slice reject.

RTL-104's operator-overload fixtures use
`-E 'binary(sim_feature_completion) & test(rtl_104::)'`: increment and compound
values, once-evaluated targets, relational-operand expected types and package
overloads run in both optimizer modes on both backends; nested or unmatched relational operands,
explicit, late and re-exported imports, two imported candidates and a native
update target with a side-effecting selector reject. The package ruling is in
[sim_data_semantics](../docs/sim_data_semantics.md#project-rulings-where-the-lrm-is-silent).

SIM-021's native operator-overload fixtures use
`-E 'binary(sim_feature_completion) & test(sim_021::)'`: records with string,
queue, dynamic-array and handle members, unions, real operands, expected
result types (including ports and casts), native and oversized update values
and legal function effects run in both optimizer modes on all value backends;
already-legal operators, time-consuming or non-function bindings, invisible or
ambiguous prototypes and effectful callback uses reject. Their decisions are
S21-D1/S21-D2 in `sim_lrm_decisions`.

RTL-105's frontend-admission fixtures use
`-E 'binary(sim_feature_completion) & test(rtl_105::)'`: `inout uwire` formals,
concatenated uwire inout actuals and runtime-selected continuous assignments to
variables run in both optimizer modes on both backends and after Db destruction; collapsed uwire driver conflicts,
net and gate-terminal runtime selects and writers inside a runtime select's
longest static prefix reject. The legality ruling is in the
[fixture readme](fixtures/sim/feature_completion/rtl_105/readme.md).

### LRM decision cases

- [`fixtures/sim/lrm_decisions/`](fixtures/sim/lrm_decisions/) holds one
  portable case per semantic decision: `<ID>_<name>.sv` (plain IEEE 1800-2009,
  top `tb`, no llg options, header quoting the clause) and its exact stdout
  `<ID>_<name>.out`. An empty `.out` marks a negative (compile-error) case;
  a `.c` companion is a DPI-C library.
- The [decision register](../docs/lrm_decisions.md) lists every case, how
  to run it on another simulator, and what changes on a mismatch (the
  trusted simulator wins).
- `sim_lrm_decisions.rs` runs each case on all value backends in both optimizer
  modes: `scripts/run-tests.sh -E 'test(/^sim_lrm_decisions::/)'`.
- [`fixtures/sim/llg_policy/`](fixtures/sim/llg_policy/readme.md) holds llg-only
  policy cases that have no portable form (RNG draw mappings, whole `%m` names
  of class scopes, NULL arguments of the DPI scope routines);
  `sim_llg_policy.rs` runs them the same way:
  `scripts/run-tests.sh -E 'test(/^sim_llg_policy::/)'`.

### Vendor patch preparation

`vendor_patches.rs` checks clean/applied trees, no-Git archives (including archives
inside another checkout), authenticated active/retired manifests, LF digest/CRLF
rendering, symlink/reparse/hardlink/stale/untracked-input rejection, and restoration
of files an earlier committed patch set applied (Git history only; mixed hand edits
and no-Git copies fail unchanged with the manual restore hint). Race probes
cover ancestor/parent replacement and late staging hardlinks. The plain tracked
`vendor/libfst` is patched in place by the same preparer, so its working tree is
clean or applied; tests cover in-place application, idempotence, partial/mismatched
rejection, and `committed_libfst_blobs_are_pristine`, which fails if HEAD or the
index holds the applied state (restore with
`git restore --staged --worktree -- vendor/libfst`).

```sh
cargo test --locked --test general vendor_patches:: -- --test-threads=1
```

## LSP acceptance

`lsp_stdio.rs` and its domain modules launch `llg_ls` over framed JSON-RPC, never
stdout debug text. `lsp_stdio`, `shadowing` and `dump_tokens` are gated by `lsp`
so no-default builds cannot launch stale binaries. Wire contracts to keep:

- Independent multi-root scans, default/client-overridden `llg.toml`,
  `llg.configFiles`, per-root lint and hot reload; `llg/configChanged` fires only
  for changed parsed config. Dynamic config/source/include watchers re-register
  after feature-data commits; dependency changes refresh every dependent root.
- `.v`/`.sv` units, include-only headers/arbitrary extensions, include/exclude
  precedence and longest-root ownership transfer on workspace add/remove.
  Unsaved sources/headers are authoritative; reject include escapes, use
  read-only staging and create no compiler artifacts in the workspace CWD.
- Shared-file diagnostics deduplicate identical findings and label distinct ones
  `[<root-name>]`. Owner-wins tokens/hover labeling; publish unopened files,
  watched fixes and stale-diagnostic clearing. New lint rules need triggering and
  nearby quiet controls with identical stable IDs in CLI JSON and LSP
  publication/config tests.
- Failed compiles keep last-good navigation. Non-syntax errors with surviving data
  keep features; syntax-broken roots keep declaration symbols/hover where
  possible, fatal-only roots do not. Watched fixes upgrade analysis. A
  syntax-invalid file keeps the semantic tokens Slang recovered in both serving
  paths, and a current open buffer never falls back to stale cached tokens.
- `lsp_stdio/macro_tokens.rs`: both serving paths agree token for token on
  in-source, header (`[compile] include_dirs`) and `[compile] defines` macros;
  each use site is one `macro` token with no expansion token over it, text
  written in actual arguments (also nested) keeps its own kind, directives are
  keywords and inactive branches have no token. A macro defined only in another
  project file is unknown in both paths (separate compilation units): its
  diagnostic is published and the rest of the buffer stays tokenized. Unsaved
  edits expand new macros with the committed include context.
- `lsp_stdio/package_imports.rs`: package items used through compilation-unit,
  header and body wildcard imports, explicit import items and `pkg::name`,
  from a package in another file (also one exported from a third package) or
  the same file, keep their own kinds, and definition, hover and references
  reach the package declarations in both serving paths, which agree token for
  token. Import item names are references (`namespace`, the imported kind),
  never document symbols; unsaved edits resolve through the committed
  package units.
- `lsp_stdio/black_box.rs`: an unknown module yields one Slang `UnknownModule`
  error on its name, fresh analysis is committed, siblings/parents stay
  elaborated and navigable, black-box actuals bind to the parent scope and the
  explorer lists it as a declaration-only child without a definition.
- Exact instance-scope bindings: named port/parameter labels refer to child
  declarations, actuals/override RHS to the parent scope. Check single/multiline
  and syntax-fallback forms with dumpTokens `bind=`. Module types stay distinct
  from namesake instances; enum navigation has wire coverage.
- `lsp_stdio/genvar.rs` covers explicit/inline genvars in hover/definition/
  references/prepareRename/rename/symbols/tokens, including unused/pruned scopes,
  ordinary namesakes, labels/members, fallback and UTF-16 columns
  (`fixtures/lsp/genvar/`, `genvar-fallback/`).
- Explorer roots honor configured tops/source graphs; keep recursive children,
  leaves, typed contents, declaration fallback and useful roots under truncation;
  never return shadow URIs.
- Semantic tokens follow the numbered rules of
  [semantic token requirements](../docs/semantic_tokens.md).
  `lsp_stdio/semantic_token_requirements.rs` checks every `ST` rule with exact
  type and modifier bits in both serving paths over
  `fixtures/semantic_tokens/requirements.sv` (its package in another project
  file is `requirements_pkg.sv`, built-ins are in `builtin_functions.sv`,
  macros, directive operands, class-scope names and delays, with the empty
  `textDocument/documentColor` answer, are in `macros_and_class_scope.sv`),
  `size_casts.sv` (ST-52) and the no-config package example
  `pkg_example_top.sv` / `pkg_example_pkg.sv` (ST-50, ST-51, started through
  `LspProcess::initialize_with_token_refresh`, which advertises
  `refreshSupport` and does not wait for the first analysis); the unit tests in
  `src/bin/llg_ls/semantic_tokens.rs` repeat the classification in every
  capture profile (isolated, `library_units` off and on; library-unit capture
  elaborates no hierarchy, so hierarchical names there may stay unresolved).
  Keep the compact-capture tests.

Fixtures use `fixtures/lsp/test.json` (schema `llg.lsp.fixture/v1`), the
effective per-root `llg.toml` and `// llg-lsp-fixture:` headers. Keep the
module-explorer manifest/header convention in sync with its suite. Keep domain
facades explicit; nested implementation files must not become Cargo test targets.

## Coverage

Detailed fixture maps retain local oracle and rejection boundaries; common CLI
requirements above apply without repeating them for each suite.

| Area | Suites / fixture contracts |
| --- | --- |
| Frontend and owned models | `slang_frontend`, `slang_semantics`, `model_tests`: safe capture, diagnostics, types, bindings, initialization, complete delays and ownership after snapshot destruction. |
| Datatypes | `sim_data_types`, `sim_data_types_extended`, `sim_data_type_edges`, `sim_data_types_next`, `sim_data_types_completion`; [mixed-type/net matrix](fixtures/sim/type_conformance/readme.md) for independent arithmetic, conversion and resolution oracles. |
| Ports and nets | `sim_port_net_types`, `sim_net_resolution`, `sim_net_defaults`, `sim_net_decl`, `sim_inout`: directional collapse, independent drivers, strengths/defaults, aliases, selections and delayed publication. [`sim_output_port_init`](fixtures/sim/output_port_init/readme.md) covers ANSI, non-ANSI, interface and program output variable port initializers and the rejected inout/net forms. [`sim_port_expressions`](fixtures/sim/port_expressions/readme.md) covers concatenated and selected port references in every direction and edition, explicitly named ANSI port expressions, instance arrays, width differences and the located rejections. [Electrical ranges](fixtures/sim/net_partition/readme.md), `sim_net_partition` and `generated_c_frame_lint::electrical_net_partition_fixtures` cover electrical runs across 1/7/64/65/129 bits, independent value oracles, delayed descriptor loops and declared shapes; [undriven cells](fixtures/sim/net_interval/readme.md) and `sim_net_interval` cover typed undriven net-array cells on every value backend plus `model.c`/codegen-time caps for 65,537- and 200,000-cell arrays; `sim_waveform`/`sim_vpi` have generated-model net-view probes. Port-type unit tests cover all 81 cells; frontend tests cover 49 resolved pairs. |
| Structural UDPs | `sim_udp`, [SYN-031 matrix](fixtures/sim/syn031_combinational_udp/readme.md): Verilog-2001 and SystemVerilog-2009 mux/parity tables, four-state inputs, `?`/`b`, unmatched rows, instance arrays, independent net drivers, delays, invalid port/row-width diagnostics and sequential/edge rejections in both optimizer modes. |
| Practical RTL | [RTL composition](fixtures/sim/rtl_completion/readme.md), `sim_rtl_completion`: initialization, fixed values/references/unions, ports and array/interface/inout composition. |
| Arrays and projections | `sim_fixed_array_reductions`, `sim_syn026_iterator_indices`, `sim_array_conditional_assignments`, `sim_fixed_ordering_review`, `sim_syn027_fixed_reverse`, `sim_syn028_fixed_sort`, `sim_group1_repairs`, `sim_group1_formal_repairs`: [reductions](fixtures/sim/fixed_array_reductions/readme.md), [iterator indices](fixtures/sim/syn026_iterator_indices/readme.md), [conditional assignments](fixtures/sim/array_conditional_assignments/readme.md), [fixed reverse](fixtures/sim/syn027_fixed_reverse/readme.md), fixed sort/rsort maps and permutation, activation isolation, signed member conversion, captured outputs and const/NBA negatives. |
| Expressions | `sim_fill_literals`, `sim_wildcard_eq`, `sim_bit_queries`, `sim_packed_strings`, `sim_sequential_predicates`, `sim_syn022_primitive_patterns`, `sim_syn023_structure_patterns`, `sim_syn024_tagged_patterns`, `sim_syn025_pattern_cases`, `sim_audit_a1_packed_constant_patterns`: fills, wildcard equality/inside, queries, [ordered predicates/patterns](fixtures/sim/sequential_predicates/readme.md), [primitive pattern scope/edition checks](fixtures/sim/syn022_primitive_patterns/readme.md), [recursive fixed structure patterns](fixtures/sim/syn023_structure_patterns/readme.md), [tagged pattern modes and bindings](fixtures/sim/syn024_tagged_patterns/readme.md), [pattern-case qualifier, control and rejection checks](fixtures/sim/syn025_pattern_cases/readme.md), and [packed integral constant-pattern acceptance and nonintegral rejection](fixtures/sim/audit_a1_packed_constant_patterns/readme.md). |
| Processes and loops | [Always-family](fixtures/sim/process_semantics/readme.md), [loops](fixtures/sim/loops/readme.md), `sim_loops`, `sim_syn014_process_contexts`: source-loop exits, omissions/bounds, aggregate sensitivity, legal disjoint writers and illegal writer/event controls. |
| Scheduling and synchronization | `sim_coroutine_semantics`, `sim_process_control`, `sim_semaphore`, `sim_mailboxes`, `sim_force`; [coroutine migration oracles](fixtures/sim/coroutine_semantics/readme.md), [processes](fixtures/sim/process_control/readme.md), [semaphores](fixtures/sim/semaphore/readme.md), [mailboxes](fixtures/sim/mailboxes/readme.md), [nonconvergence](fixtures/sim/nonconvergence/readme.md). |
| Generated coroutine storage | `sim_frame_hoisting`, `generated_c_frame_lint`: narrowed/flattened frame shape, deep-block strict-C compile/run and warning-clean C checks plus a six-way sweep of every independently generatable simulation fixture in optimized and unoptimized modes. The reusable lint permits local addresses only when the declaring block is resume-free, and rejects stack addresses in resume-bearing scopes, exact generated-name lookalikes and overlay paths used outside their emitted C block. |
| Gate fast paths | `sim_gate_path_differential` builds fixed-seed random gate/UDP netlists (every gate kind, delays, strengths, wired and multiply driven nets, gate arrays, X/Z stimulus with same-slot glitches, a killed `@*` waiter) with and without the development-only C define `LLG_DEV_LEGACY_GATE_PATH` and requires identical per-wake traces. `sim_counter::sim_rt_selftest` checks the wake order of retained dependency waits directly. |
| Unused-storage optimizer | `opt_storage_differential` generates every fixture with and without the `unused_storage` pass: pruning must never make generation fail, a differing optimized model must compile, and every eighth differing fixture per shard is built and run in both forms with identical status and output (`LLG_STORAGE_DIFF_EXEC=all` runs all of them). `sim_opt_storage` and [opt_storage](fixtures/sim/opt_storage/readme.md) run variables read only through event-handle indices, handle receivers and arguments, and clocked nonblocking wait sources in both optimizer modes; `sim::opt::passes::tests::storage_references` covers each reference kind. |
| Instance body sharing | `sim_instance_sharing`, [instance identities](fixtures/sim/instance_sharing/readme.md): shared module/task and generate bodies preserve `%m`, named events, external disable isolation and net contribution slots in both optimizer modes. Render tests cover thresholds, width/parameter splits, PCA sites, port-link and path/UDP operands and deterministic records; `netlist_cells.sv` shares cell bodies across bus-bit instances with specify paths and UDP flops. `sim_scale` generates one netlist at two sizes and caps the time ratio and coroutine body count, so generation stays linear in the instance count. The frame lint recognizes shared entries and model-lifetime record addresses. |
| Time and numeric services | `sim_delay`, `sim_time_literals`, `sim_time_values`, `sim_physical_time`, `sim_real`, `sim_real_conversions`, `sim_random`, `sim_random_streams`: exact units/rounding, overflow, real precision, Annex N vectors and child-stream isolation. |
| I/O and observation | `sim_file_io`, `sim_memory_editions`, `sim_memory_views`, `sim_syn030_memory_views`, `sim_plusargs`, `sim_waveform`, `sim_vpi`, `sim_syn035_pla`, `sim_feature_completion::sim_027`, `sim_feature_completion::adv_026`; [SIM-027 memory files](fixtures/sim/feature_completion/sim_027/readme.md), [SYN-030 memory views](fixtures/sim/syn030_memory_views/readme.md), checked signed `@` spellings/bounds in both editions and selected views in SV2009, [SYN-035 PLA exclusion](fixtures/sim/syn035_pla/readme.md), [file I/O](fixtures/sim/file_io/readme.md), [waveforms](fixtures/sim/waveform/readme.md), [partial features](fixtures/sim/partial_features/readme.md). |
| Undefined behavior characterization | `sim_undefined_behavior` compares exact CLI stdout, stderr and status to checked-in [Q02 goldens](fixtures/sim/undefined_behavior/readme.md) in both optimizer modes and each legal edition. These are observations for later cross-simulator comparison, not independent conformance oracles. |
| Verification and native objects | `sim_classes`, `sim_virtual_interfaces`, `sim_dpi`, [concurrent assertions](fixtures/sim/concurrent_assertions/readme.md): nominal identity, dispatch/casts, interface bindings, scalar native ABI, sampled/sequence/property behavior and explicit rejection boundaries; `sim_feature_completion::adv_013` for [multiclocked and recursive assertions](fixtures/sim/feature_completion/adv_013/readme.md), whose `oracle` case checks randomized two-clock traces against the Annex F interpreter in `sim_feature_completion/adv_013/oracle.rs`; `sim_feature_completion::adv_014` for [global clocking future sampled value functions](fixtures/sim/feature_completion/adv_014/readme.md). |
| Build and safeguards | `sim_cmake`, `cli_info`, `sim_memory_guard`, `support_harness`, `emit_decoupling`, `generated_c_determinism`: CMake/generator failures, source cleanup, early CLI exits, memory limits, timeout trees, CWD recovery, frontend-independent emission and byte-identical generated source trees across separate processes. |
| Native components | `runtime_values`, `runtime_random`, `runtime_constraints`, `runtime_file_io`, `runtime_boundaries`, `runtime_value_storage`, `runtime_gmp_closure`: direct runtime probes, independent of HDL lowering (`runtime_constraints` checks the constraint solver against an exhaustive enumerator and hand-built template problems). |
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
  retains void, nested and unpacked-storage controls. RTL-016 owns unpacked
  payloads, member selects and the resolved Q03 commit check.
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
Sequential UDP edge/state forms remain excluded from the selected profile
(ADV-003 implements them as Extended forms) while scalar combinational UDP
forms have SYN-031 evidence. The ordinary checker validates the frozen PDF
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
- CMake, Ninja (the default model generator; set `CMAKE_GENERATOR` to use
  another) and a C compiler for public conformance; run commands at repository root.
- Cargo-launched runs get `LLG_RUNTIME_CACHE_DIR=<repo>/target/llg-runtime-cache`
  from `.cargo/config.toml` `[env]`, so CLI tests in temporary CWDs share it.
  An explicit value wins; use an absolute path, because `llg` resolves
  relative values from its own CWD.
- Nextest defaults to 8 concurrent tests; `--profile max-threads` opts into 32.
  CI sets `NEXTEST_PROFILE=ci`, which runs one test per available logical CPU.
  Release-target CI builds a `cargo nextest archive` per platform and runs it in
  per-platform `test` jobs (each needing only its own platform's `build`) (CI caching: compiled third-party dependencies and
  generated-model ccache only, see [CI caching](ci.md#caching-binding-user-rule)); the equivalent local form is
  `cargo nextest archive --locked --all-features --cargo-profile release --archive-file F`
  then `cargo nextest run --archive-file F --workspace-remap ROOT --extract-to ROOT`
  with ROOT the absolute checkout path used for the build.
- Windows on ARM: `llg_ls` only; `llg` unsupported. Its CI lane builds only
  `llg_ls` and runs nextest profile `ci-lsp` (`NEXTEST_PROFILE=ci-lsp`, a
  `default-filter` in `.config/nextest.toml`): the `llg_ls` unit tests, library
  `core`/`config`/`ffi`/`memory_limit` tests and the `general` suites that need
  no simulator, C compiler or `llg` driver (`lsp_stdio`, `shadowing`,
  `dump_tokens`, `config_effect`, `compilation_units`, `include_dir_search`,
  `parse_only`, `probe_bindings`, `slang_frontend`, `slang_static_function_return`,
  `elab_resolve`). Reproduce it on any host with
  `scripts/run-tests.sh --cargo-profile quick --profile ci-lsp --all-features --lib --bin llg_ls --test general`
  (about 1,030 tests). A new LSP or frontend-only `general` suite must join the
  filter; `test_layout` checks that its names exist and run no simulator code.
  Preserve suite-specific skips: `sim_cmake` may skip absent CMake; native DPI
  shared-library tests need Unix/CMake/compiler. These are not blanket skips.

### Cargo build profiles

Daily `cargo test`, `cargo nextest run` and `scripts/run-tests.sh` builds use the
`test` profile at `opt-level = 3`, matching release optimization. Debug assertions
and overflow checks stay enabled to catch invariant and arithmetic failures.
The simulator's in-process Rust pipeline benefits from this optimization.

Use `quick` for shorter rebuilds during edit-test loops:

```sh
cargo test --locked --profile quick --test sim_a_m sim_counter::
cargo nextest run -E 'test(/^sim_counter::/)' --locked --cargo-profile quick
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
# Or one environment variable for every model build in this shell and for the
# tests (LLG_CC must stay a single program, so it cannot carry a launcher):
export LLG_C_LAUNCHER=ccache CCACHE_BASEDIR=/build CCACHE_NOHASHDIR=1
```

`LLG_C_LAUNCHER` mirrors `--launcher`: `--launcher` > `$LLG_C_LAUNCHER` >
`build.launcher` > none in `llg` (the shared CLI > environment > config rule);
library callers of `sim::build` get option > `$LLG_C_LAUNCHER` > none. An empty
variable is unset and an explicit empty library option suppresses it. `sim::build` forwards it as
`CMAKE_C_COMPILER_LAUNCHER` for the runtime archive and the model, and it is
part of the runtime cache key. It never reaches the root `build.rs` Slang/fmt/
wrapper build, which only reads `LLG_CCACHE`. Test directories are unique per
process, so ccache needs `CCACHE_BASEDIR` set to a common parent of those
directories (the test scratch root, `LLG_TEST_BUILD_DIR`/`--test-work-dir`, or
the temp dir) and `CCACHE_NOHASHDIR=1`; without them paths enter the hash and
every build misses. Test directories are canonicalized, so the base directory
must match their resolved spelling (Windows long names rather than 8.3 short
names, macOS `/private/var`); ccache 4.12+ accepts a `;`/`:` list of both. Harness paths that build a model through `sim::build` or
the `llg` binary (`tests/support/sim.rs`, `tests/support/sim_cli.rs`,
`sim_cmake`) and the `runtime_value_storage` CMake probes honour it. Probes that
compile runtime C directly with `$LLG_CC`/`$CC` (`runtime_values`,
`runtime_random`, `runtime_rng`, `runtime_containers`, `sim_dpi`, `sim_waveform`,
`sim_feature_completion/sim_004`) run one compiler command each and do not use
the launcher. Without either variable they use the model default (MSVC `cl` on
Windows); `tests/support/c_compiler.rs` (`host_c_compiler`,
`strict_c11_executable`) translates their strict C11 build to `/W4 /WX` for
MSVC. The `sim_dpi` and `sim_vpi` probes stay Unix-only; the static
archive routes of SIM-041, SIM-042 and the S42 decisions run on every host,
the DLL companions of the feature-completion and decision suites build with
`/MD` (the model's shared UCRT, so their `printf` output shares its `stdout`
buffer) and export every import definition with `DPI_DLLESPEC`
(`sim_040::dll_companions_export_every_import_definition`), and the `sim_047`
FST reader builds with the MSVC flag set.

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
python3 -m unittest discover -s scripts -p test_ci_cache.py  # CI ccache/prune helpers
python3 -m unittest discover -s scripts -p test_ci_build_timings.py  # CI model-build timing summary
python3 -m unittest discover -s scripts -p test_ci_windows_runner.py  # Windows CI runner cost probe
python3 -m unittest discover -s scripts -p test_ci_resource_sampler.py  # CI test-job resource sampler
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
| Compatible simulation runtime archives and CMake toolchain-detection seeds | Shared `PATH/lapligence/runtime-cache/` |
| Cargo targets and intermediate build artifacts | Each worktree's `target/` on its existing filesystem |
| Native Slang CMake build | Each worktree's existing `target/slang/` |
| Cargo downloads and optional compiler caches | Existing persistent locations |

The hash uses the canonical worktree path, and each invocation receives a fresh
run directory even within the same worktree. All agents can use the same command;
no manual agent number is needed. Runtime entries are keyed by sources, ABI,
toolchain, flags and build options, with process locks for cache population.
Toolchain seeds (`<cache>/cmake-toolchain/<hash>/`) and MSVC compiler probe memos
(`<cache>/compiler-probe/<hash>`) live in the auxiliary directories named by
`sim::build::RUNTIME_CACHE_AUX_DIRS`; every other
root child is a runtime entry with a `ready` marker. Seeds are keyed by
CMake, generator, compiler, flags, launcher and detection environment; tests that
inspect configure behaviour can disable them with `LLG_CMAKE_TOOLCHAIN_SEED=0`.
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

#### Stale Cargo outputs

Cargo never deletes outputs whose hash changed: a new dependency version,
feature set, flag or toolchain leaves the previous copy of the library, test
binaries and incremental sessions behind in `target/<profile>/`. Remove them,
while no build runs in that worktree, with:

```sh
scripts/prune-target.py --dry-run     # report what would go
scripts/prune-target.py               # default --keep-days 1
scripts/prune-target.py --keep-days 0 # everything superseded, however recent
```

A unit (one fingerprinted package/target-kind hash with its `deps/` and `build/`
outputs, or an incremental session) is removed only when none of its files changed
within the keep window and either a newer unit of the same package and target kind
exists in that profile, or it builds a workspace test, binary, example or benchmark
that `cargo metadata` no longer lists (for example the former per-file test
binaries). The newest copy of every live target is kept and configurations in
active alternation (for example clippy `--all-features` and the test build) are
not evicted. Cargo rebuilds anything it still needs. Checks:
`python3 -m unittest scripts.test_prune_target`.

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
On Windows the harness names its directories `llg-<pid>-<n>` (hex) instead of
`llg-<tag>-<pid>-<nanos>-<n>`: MSVC cannot write `try_compile` objects past
`MAX_PATH` (260 characters), and a runtime cache inside a test directory already
puts them about 200 characters below it. Keep Windows roots short.
A relative `LLG_TEST_BUILD_DIR` resolves from the Cargo workspace, including
inside nested temporary CWDs; give `LLG_RUNTIME_CACHE_DIR` as an absolute path.
Missing directories are created; an empty or unusable `LLG_TEST_BUILD_DIR` fails
rather than falling back to another disk. The filesystem must permit
execution (`noexec` prevents CMake compiler checks and simulators from running).

Test parallelism is unchanged. Model and runtime-archive builds run
`cmake --build --parallel <N>`, with `N` from `$CMAKE_BUILD_PARALLEL_LEVEL` when
set, else the host CPU count; the harness leaves it unset because the runtime
archive is built once per cache key under a lock and a model is one translation
unit (revisit when models are split into parts). Tests that run plain
`cmake --build` on a Makefiles tree (self-contained export, GMP closure, storage
probes) pass `--parallel` from `sim_harness::cmake_build_jobs()`, which follows the
same variable and then the usable CPUs; without it the build is serial in CI.
Every invocation owns a unique child directory and removes that child on completion or unwind; the configured root and unrelated
files are retained. `LLG_RUNTIME_CACHE_DIR` separately relocates the shared runtime
archive cache, which persists across tests and runs until removed or unmounted.
Size tmpfs for concurrent builds and leave RAM for compilers and tests.

Generated models build in Release without debug information. The suite's own
native CMake probe projects (`runtime_value_storage`, `runtime_gmp_closure`) build
their Debug configuration without it too: `LLG_TEST_DEBUG_INFO=1` (also `on`,
`true`, `yes`) restores `-g`, MSVC `/Z7` and the `/debug` link for debugging a
probe. Optimization levels, `/RTC1`, `assert()` and `LLG_CO_DEBUG` are unchanged.

`LLG_TEST_BUILD_DIR` does not relocate Cargo artifacts or tests that use their own
temporary-directory helpers. Cargo supports `CARGO_TARGET_DIR`; system temporary
files follow platform settings such as `TMPDIR` on Unix. The native Slang build
still uses the repository's `target/slang` directory.

### Focused simulator suites

```sh
cargo nextest run -E 'test(/^(sim_type_conformance|sim_partial_features)::/)' --locked
cargo nextest run -E 'test(/^(sim_data_types|sim_data_types_extended|sim_data_type_edges)::/)' --locked
cargo nextest run -E 'test(/^(sim_data_types_next|sim_data_types_completion|sim_net_resolution|sim_net_defaults|runtime_values|runtime_random)::/)' --locked
cargo nextest run -E 'test(/^(sim_physical_time|sim_mailboxes)::/)' --locked
cargo test --locked --lib core::compile::editions::tests -- --test-threads=1
cargo test --locked --test sim_a_m --test sim_syn --test runtime -- sim_loops:: sim_edition:: sim_syn016_elaboration:: sim_file_io:: runtime_file_io:: --test-threads=1
```

### `llg.toml` configuration

```sh
cargo nextest run --locked --lib config::
cargo nextest run --locked --bin llg settings::
cargo nextest run -E 'test(/^(llg_config_cli|lint_config_cli)::/)' --locked
cargo nextest run --locked --bin llg_ls config::
cargo nextest run -E 'test(/^dump_tokens::/)' --locked dump_accepts_driver_only_config_keys
cargo nextest run -E 'test(/^lsp_stdio::/)' --locked lsp_stdio_accepts_driver_keys
```

Ownership by layer (a new key needs coverage in each layer it touches):
library `config::tests` own schema, path resolution, unknown keys and error
context; `llg` `settings::tests` own command line > environment > config
precedence; `llg_config_cli` runs the public driver; `dump_tokens`/`lsp_stdio`
prove the server accepts driver-only keys and rejects unknown ones atomically.

`llg_config_cli` copies `tests/fixtures/config_cli` into an isolated directory
and runs the public `llg` from it with a written `llg.toml`: no discovery of a
file in the current directory, explicit and missing `--config`, command-line
override of top, replace-versus-`--append-<list>` for lists, define override,
environment-versus-config-versus-command-line compiler selection, unknown keys
and invalid values. `config::` and `settings::` cover schema parsing, path resolution and
the precedence rules without a simulator build.

### Safeguards

Cover exact input/response budgets, cache backpressure, logging and startup
integration of the [memory guard](../src/memory_limit.rs) and LSP admission:

```sh
cargo nextest run --locked --lib memory_limit::tests
cargo nextest run --locked --lib ffi::process_memory
cargo nextest run -E 'test(/^sim_memory_guard::/)' --locked
cargo nextest run --locked --bin llg_ls input_budget
cargo nextest run --locked --bin llg_ls oversized
cargo nextest run --locked --bin llg_ls response_budget
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
cargo nextest run -E 'test(/^(sim_partial_features|sim_type_conformance|runtime_values|runtime_random)::/)' --locked
```

Inspect every `/tmp/llg-asan-model.*` file for errors. Stackless coroutine frames
need no stack-switch suppression; file routing preserves exact program-stderr
assertions without hiding sanitizer reports. The CI sanitizer job also runs
`sim_023::`, whose force/release fixtures kill and disable forcing processes and
end the run with forces active.

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
architecture-specific generated-model support. Windows on ARM: `llg_ls` only;
`llg` unsupported, so no generated-model result exists or is expected there.

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
LLG_STORAGE_DIFF_EXEC=all scripts/run-tests.sh --test-work-dir /build --cargo-profile quick --test opt_storage_differential
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
cargo test --locked --no-default-features --test sim_a_m sim_dynamic_ownership::
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

### Value backend builds

`sim_value_backends` checks selected runtime archives, clean source-only builds,
both wrong-backend and wrong-kernel links, exact C selectors, missing GMP and
positive generated-HDL parity with independent expected outputs. The GMP lane
builds the bundled `vendor/gmp` subset; `LLG_TEST_GMP_ROOT` makes the lanes use
an installation instead and enables the installation-only tests below. Native `selected_*_facade` probes in
`runtime_value_storage` exercise the common consumer bridge in all three modes.

```sh
CARGO_BUILD_JOBS=6 CMAKE_BUILD_PARALLEL_LEVEL=6 scripts/run-tests.sh --test-work-dir /build --cargo-profile quick --test sim_value_backends --test-threads 6
```

`CodegenOptions.value_config` and `CmakeBuildOpts.value_config` must agree; both
default to compact/GMP, the only user configuration. The legacy backend and
portable kernels are development references: the driver selects them only
through `LLG_DEV_VALUE_BACKEND=legacy|compact` and
`LLG_DEV_COMPACT_KERNELS=portable|gmp` (the former `LLG_VALUE_BACKEND` and
`LLG_COMPACT_KERNELS` are ignored), and library tests name
`ValueConfig::LEGACY`/`COMPACT_PORTABLE`. GMP builds the bundled subset unless
`GMP_ROOT` names an installation (no system fallback). Compact frame checks use a
24-byte descriptor instead of legacy's 32-byte descriptor on 64-bit hosts;
`compact_selected_frame_lint` covers suspended values, shared instances and
structural nets with both kernels and optimizer modes.

### GMP dependency qualification

`sim_value_backends::qualification` adds the V09 dependency witnesses: wide
mul/div/mod/decimal results of the bundled compact/GMP build (reaching its Toom
and FFT paths) equal legacy and portable, and `--gen-only` exports, which carry
the bundled sources, are moved before they configure and build. With
`LLG_TEST_GMP_ROOT` naming an installation (`include/gmp.h` and a static
library), the same probe runs against it and, on Linux/macOS hosts, against a
copy whose header selects the other 64-bit limb spelling (`_LONG_LONG_LIMB`),
which drives the macOS limb-copy path; byte-identical installation copies share
one runtime archive and edited ones do not; and header/library mismatches fail
configure. Without it those installation tests print SKIP. The default suite
already runs every model with compact/GMP.

After moving the `vendor/gmp` submodule, run `python3 scripts/gmp_tables.py` to
regenerate the bundled tables (`--check` compares; CI's lint job runs it).

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
VPI and waveform execution harnesses honor `LLG_DEV_VALUE_BACKEND` and
`LLG_DEV_COMPACT_KERNELS`. Direct library/component tests retain explicit or
default (compact/GMP) configurations; this environment does not change library
defaults. Run the whole suite against a development reference, for example
compact portable, using:

```sh
LLG_DEV_VALUE_BACKEND=compact LLG_DEV_COMPACT_KERNELS=portable CARGO_BUILD_JOBS=6 CMAKE_BUILD_PARALLEL_LEVEL=6 scripts/run-tests.sh --test-work-dir /build --cargo-profile quick --test-threads 6 --no-fail-fast
```

Run the cross-backend parity matrix alone with:

```sh
CARGO_BUILD_JOBS=6 CMAKE_BUILD_PARALLEL_LEVEL=6 scripts/run-tests.sh --test-work-dir /build --cargo-profile quick --test sim_value_backends -E 'test(parity::compact_parity_) | test(compact_hdl_subset_matches_legacy_and_independent_outputs)' --test-threads 6
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
`tests/runtime_value_storage.rs` (legacy, compact portable and compact GMP, the
last with the bundled GMP or `LLG_TEST_GMP_ROOT`), and build the strict-warning
runtime targets once in Release:

```sh
CARGO_BUILD_JOBS=6 CMAKE_BUILD_PARALLEL_LEVEL=6 scripts/run-tests.sh --test-work-dir /build --cargo-profile quick --test runtime_value_storage --test-threads 6
```
