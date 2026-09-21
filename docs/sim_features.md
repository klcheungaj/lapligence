# Simulator Feature Checklist — IEEE 1364 / IEEE 1800

This is the maintained feature inventory for the `llg` simulator. It records
implementation scope separately from acceptance evidence. Revision tags and
standard references retain the existing inventory's terminology. Host regression
results establish their tested scopes, not clause-by-clause conformance.
The file remains the sole maintained simulator feature-status document.

Section numbers retain the original inventory's Verilog (§1364-2001 x.y),
SystemVerilog and verification (§1800-2009 y.z) references. The clause mappings
link to the checked-in references under `docs/specification/`; they were not
independently revalidated in this source-status audit.
Detailed source investigations and run records belong in local `persistence/`.

## Current source status and qualification — 2026-09-20

This inventory records the source status reconciled against the clean
2026-09-20 checkout. The baseline source identity, vendored base revisions and
tracked native patches are captured in the local evidence record; this file
remains the maintained feature-scope document.

**R01–R05 have bounded source implementations. R06 implements sequential Boolean
clauses, SYN-022 adds integral constant, wildcard and identifier-binding
`matches` patterns with owned lexical storage, and SYN-023 adds recursive fixed
packed/unpacked structure patterns with resolved member checks. The current clean
checkout has a green Linux all-features baseline; this is test evidence for the
checkout, not a claim of complete synthesizable Verilog/SystemVerilog support.**
The numbered feature sections and stable grouped inventory below record the
implemented forms and remaining restrictions, including corrections to older
contradictory rows for `defparam`, `let`, initialization and read-only helper
calls.

### Selected synthesis profiles

SYN-000 adopts the three profile axes from the implementation plan. The
profiles keep language legality, simulation execution and synthesis policy as
separate qualifications.

| Profile | Selected scope | Qualification |
| --- | --- | --- |
| Core finite RTL | Fixed integral packed/unpacked data; deterministic combinational, sequential and latch procedures; finite loops; zero-time subroutines; parameters, types, packages, generate and concrete interfaces; legal net, variable and reference connectivity. | Core work is selected for both Verilog-2001 and SystemVerilog-2009 where the edition permits the construct. Each context still needs its own execution evidence. |
| Core elaboration | Legal 2001/2009 source forms that elaborate to finite Core RTL, including folded real/string constants used for finite values, widths or names. | Slang elaboration is reused; runtime real/string hardware is not implied. |
| Extended finite / target-dependent | Tagged fixed values and patterns; fixed-array iterator/order methods; memory images; combinational UDPs; explicit library/configuration selection; structural bind. | These tracks are selected as named work items. SYN-034 excludes the six finite metaprogramming and legacy forms recorded below; SYN-035 keeps legacy PLA excluded pending a named target and acceptance evidence. |
| Companion semantics and capacity | Exact edition semantics, four-state/X/Z policy, resource ceilings and per-context evidence. | Required for truthful support claims, but kept distinct from a syntax-family completion claim. |

### SYN-037 finite loop control and local named disable — 2026-09-21

SYN-037 closes the finite, zero-time local control matrix for the selected
Core simulator scope. The public command-line fixtures run in optimized and
unoptimized modes through the ordinary simulator test harness:

- [syn_037_finite_control_2001.sv](../tests/fixtures/sim/loops/syn_037_finite_control_2001.sv)
  covers the Verilog-2001 for, repeat, while and forever forms, named-block
  exits, automatic function and task locals, and task output copy-out. Its
  source references IEEE 1364-2001 §§9.6, 10.2-10.3, and 11.
- [syn_037_finite_control.sv](../tests/fixtures/sim/loops/syn_037_finite_control.sv)
  covers the SystemVerilog matrix with multiple for initializers and steps,
  nested lexical break/continue targets, per-iteration declarations,
  do/foreach controls, reverse extreme endpoints, local named-block disable,
  function early return, task disable and copy-out, and duplicate names in
  independent scopes. Its source references IEEE 1364-2001 §§9.6, 10.2-10.3,
  and 11 plus IEEE 1800-2009 §§9.6.2, 12.7.1-12.7.6, and 12.8.

The qualification is limited to finite local control. Budget exhaustion and
nontermination remain scheduler policy; cross-process cancellation, recursive
timed-task cancellation, dynamic string iterator captures, and jumps across
lexical scopes remain outside this closure.

### SYN-014 aggregate sensitivity and always-process qualification — 2026-09-21

SYN-014 closes the selected fixed integral aggregate sensitivity and
always-process evidence path. The public CLI witness
[`sim_syn014_process_contexts.rs`](../tests/sim_syn014_process_contexts.rs) runs
with optimization enabled and disabled and covers:

- `always_comb` reads of packed members and nested fixed-array leaves through
  aggregate value arguments and called functions, including both conditional
  arms and written-member exclusion;
- an aggregate input link waking the child `always_comb` process after a
  nested element update;
- ordinary `always @*` retaining its Verilog-2001 call-site sensitivity rule;
- disjoint packed-member writers, time-zero `always_latch` behavior and a
  resettable `always_ff` whose data changes do not wake it without its clock or
  reset event; and
- rejection of overlapping packed writers, extra `always_ff` event controls
  and explicit `always_latch` event controls.

The accepted scope is fixed integral packed/unpacked records, fixed-array
leaves, and the admitted value/input-link paths. Dynamic or native aggregate
callbacks, event objects, clocking regions, and concurrent assertions remain
outside this qualification. The language mapping is IEEE 1364-2001 §9.7.5 and
§9.9.2 and IEEE 1800-2009 §§7.2, 7.4.2, 7.4.6, 7.7, and 9.2.2.2–9.2.2.4; the local section
index is [`spec-reference-verilog.md`](specification/spec-reference-verilog.md)
and [`spec-reference-sv.md`](specification/spec-reference-sv.md).

### SYN-034 finite metaprogramming and legacy extension profile — 2026-09-21

SYN-034 records the target-policy decision for the six finite metaprogramming
and legacy extension subprofiles named by the implementation plan. A
`Supported` disposition would require a bounded legal witness through the
owned simulator pipeline and concrete acceptance evidence from the selected
synthesis target. The repository has simulator regressions for several of
these constructs, but no checked-in target-tool run, netlist, or equivalent
target acceptance artifact. The clause maps under `docs/specification/` are
language references and synthesis classifications, not target acceptance.

All six forms are therefore **excluded by the selected SYN-000 profile**. The
exclusions apply to synthesis qualification; existing simulator behavior and
its narrower source-status rows remain documented below. No child repair task
is created because no selected form has a concrete implementation failure.

| Subprofile | Disposition | Language and implementation evidence | Reason and boundary |
| --- | --- | --- | --- |
| Elaboration-only static class methods and constant objects | Excluded by selected profile; no child repair | SystemVerilog-2009 §§8.8–8.9 classify static class properties/methods with the class facility as non-synthesizable in [`spec-reference-sv.md`](specification/spec-reference-sv.md). Class objects and methods have a runtime lowering path in [`collection/classes.rs`](../src/sim/codegen/lowering/collection/classes.rs), while [`semantic.rs`](../src/sim/semantic.rs) classifies every reachable `ClassDef` as `RuntimeObject`. Existing class fixtures are simulator evidence only. | A folded ordinary constant function remains covered by Core elaboration when its source form is selected; that does not admit class/object syntax. Heap lifetime, handles, and virtual dispatch remain outside the synthesis profile. |
| Operator-overloading declarations | Excluded by selected profile; no child repair | SystemVerilog-2009 §11.11 and Annex A.4 identify operator overloading; the local Annex A audit classifies it `N`. There is no separate owned overload declaration or operation path: [`semantic.rs`](../src/sim/semantic.rs) enumerates ordinary `Operation` values in `is_synthesis_operation`, and the wrapper captures ordinary operator expressions. | Overload binding can change the meaning of an otherwise ordinary operator for non-RTL types. No target acceptance evidence establishes a finite overload subset, so ordinary operator support does not imply overload support. |
| Fixed-input locator results consumed entirely at elaboration | Excluded by selected profile; no child repair | SystemVerilog-2009 §7.12.1 and Annex A.6 cover `find*`, `min`, `max`, and `unique` locator results and classify the array-method family as non-synthesizable. The SYN-034 exclusion is limited to those locator results; SYN-027 and SYN-028 separately select fixed `reverse`/`sort`/`rsort` ordering tasks, with bounded simulator implementation and tests. [`methods.rs`](../src/sim/codegen/lowering/containers/methods.rs) lowers runtime method families, but imported `MethodCall` nodes are treated as `RuntimeService` by the synthesis validator. Fixed reduction witnesses cover `sum`/`product`/`and`/`or`/`xor`, not locators. | A constant input or elaboration-only use does not supply target acceptance for the source-level locator form. The selected profile retains the named fixed reductions and the separately scoped SYN-027/SYN-028 ordering tasks; target acceptance remains a separate qualification. |
| Legacy procedural `assign`/`deassign` and static-initialization `force` | Excluded by selected profile; no child repair | IEEE 1364-2001 §§9.3.1–9.3.2 and SystemVerilog-2009 §10.6 describe procedural continuous assignment and force/release; the local references classify them `N` or target-dependent. Simulation lowering and regressions exist in [`drivers.rs`](../src/sim/codegen/lowering/statements/drivers.rs) and [`sim_procedural_assign.rs`](../tests/sim_procedural_assign.rs), but [`classify_statement`](../src/sim/semantic.rs) reports `ForceOrProceduralDriver` for these statements. | Constant memory/preload initialization is already recognized as `StorageInitialization` and remains rejected by `PortableRtl`; putting `force` in an initialization process does not change that boundary. No target-specific static-force contract is present. |
| Module-body `specparam` declarations | Excluded by selected profile; no child repair | IEEE 1364-2001 §§3.11.3 and 14.1 and SystemVerilog-2009 §30.3 define `specparam` inside specify behavior; the local references classify specify parameters as non-synthesizable. The [`slang_c_api.cpp`](../src/wrapper/slang_c_api.cpp) wrapper maps a frontend `Specparam` symbol to parameter metadata, but no specify/path-delay lowering exists; the grouped inventory keeps specify blocks missing. | This decision concerns specify-region timing parameters, not ordinary `parameter` or `localparam` declarations selected by Core elaboration. Metadata capture alone cannot establish executable or target hardware semantics. |
| Finite assertion and monitor synthesis | Excluded by selected profile; no child repair | SystemVerilog-2009 §§16.3, 16.4, and 16.15.1 classify immediate/deferred/concurrent assertions as verification constructs; Verilog-2001 §17.1.3 and SystemVerilog-2009 §21.2.3 classify `$monitor` as simulation output. Bounded assertion and monitor lowering exists in [`assertions.rs`](../src/sim/codegen/lowering/assertions.rs) and [`runtime_tasks.rs`](../src/sim/emit_c/owned/runtime_tasks.rs), while the synthesis validator admits only the small synthesis system-call set and treats monitor services as runtime behavior. | Finite predicates or a bounded monitor do not define a portable hardware mapping. Existing assertion/monitor fixtures prove simulator semantics only; no selected target acceptance establishes an on-chip assertion or monitor profile. |

### SYN-035 fixed-personality PLA target decision — 2026-09-21

SYN-035 requires a concrete legacy synthesis target before selecting any PLA
subfamily. This audit found no named target, target-tool invocation, netlist,
synthesis report, or equivalent acceptance artifact in the tracked repository
or the existing SYN handoffs. The local LRM maps identify the syntax and its
simulation meaning, but they are not target acceptance evidence.

All sixteen Verilog/SystemVerilog array and plane forms remain **excluded by
the selected SYN-000 profile and deferred pending target evidence**:
`$async$and/nand/or/nor$array`, `$async$and/nand/or/nor$plane`, and their
`$sync$` counterparts. This is a synthesis-profile decision; it does not
claim that every synthesis product rejects every finite PLA use, or that the
simulator cannot parse a PLA system-task call.

| Decision area | Evidence and boundary |
| --- | --- |
| Language references | The local Verilog reference maps PLA tasks to §17.5 and the SystemVerilog maps them to §20.17; Annex A.7 lists all sixteen names and records the local synthesis classification. These references define the language family, not acceptance by a selected target. |
| Existing implementation | [`editions.rs`](../src/core/compile/editions.rs) admits the names for edition checking. [`system_tasks.rs`](../src/sim/codegen/lowering/statements/system_tasks.rs) has no typed PLA personality or array/plane lowering; the generic system-call branch emits a VPI call. [`semantic.rs`](../src/sim/semantic.rs) admits only `$bits`, `$clog2`, `$signed`, and `$unsigned` as synthesis system calls, so PLA calls are classified as `RuntimeService`. |
| Selected scope | No PLA form is selected and no child implementation or regression is created. The existing ❌ source-status row below remains a simulator implementation gap, while this section records why it is also outside the selected synthesis profile. |
| Reopening gate | Name the target and tool/version, define the fixed personality encoding and dimensions, provide an independently calculable legal witness, and attach target acceptance output. Only then should a bounded asynchronous or synchronous subset be implemented and tested. |

General classes/heap/queues/randomization, testbench infrastructure, full SVA/
coverage/DPI/PLI and arbitrary device-level timing/SDF behavior remain outside
this selected RTL profile.

### Latest supplied validation status

| Evidence | Result | Qualification boundary |
| --- | --- | --- |
| Current clean-checkout baseline | **2,353 tests run: 2,353 passed, 1 skipped** | Full `--all-features` nextest on Linux from the isolated clean worktree at `fb3e4d12`; the skip is `property_elab::gen_c_vectors`. This is current checkout evidence, not clause-by-clause conformance. |
| Superseded user-reported pre-`_03` run | **2,350 tests run: 2,346 passed, 4 failed, 1 skipped** | Historical evidence only. The four reported failures and the subsequent `_02` compile error are not current baseline failures; the current run supersedes them. |
| Recorded native component runs after R01–R06 and the first corrective patch | **50/50 GCC ASan/UBSan; 62/62 unsanitized** | Results come from the supplied validation logs, not a new run. The suites overlap; the sanitizer lane excludes real coroutine stack switching. Handwritten C probes do not validate the complete frontend-to-generated-model path. |
| Recorded operand-slot witness for corrective patch `_02` | **9 checks passed with GCC; 9 with Clang**, both ASan/UBSan | Extracted bridge functions with AST stand-ins check repeated operand positions. This is not a full Slang bridge build or HDL run. |

The current baseline establishes a clean Linux build and full Rust/HDL
nextest result, including the corrected fixed-array conditional, NBA and
default-pattern cases where those suites run them. Existing regression source
files are not themselves pass evidence. Native Windows/macOS execution, the
generated-model ASan boundary and every language/context combination remain
separately unqualified. Commands and test methodology belong in the [test
guide](../tests/readme.md); detailed run records belong in `persistence/`.

## Dynamic value migration acceptance boundary

The 2026-09-19 migration checkpoint predates the R01–R06 and corrective patch
chain. Its results below are retained as historical evidence, not current
acceptance. The [latest supplied validation status](#latest-supplied-validation-status)
governs qualification of the current sources.

**The ownership migration remains partial.** The old blanket model gates for
containers, native objects, classes, aliases, assertions, sampling and VPI have
been replaced by bounded source implementations and narrower rejection paths.
That is implementation progress, not restored full-language acceptance. Models
combining otherwise implemented features can still encounter a remaining guard;
recorded function/helper definitions can reject emission even when not executed.
Detached expression/statement C-fragment renderers remain deliberately fenced.
The active path is the structured whole-model emitter under
[`src/sim/emit_c/owned/`](../src/sim/emit_c/owned/readme.md).

The source now uses exact-width packed owners, registered native payloads, explicit
copy/move/destruction, retained delayed packed destinations, and cleanup across
normal return, suspension and cancellation. Class objects are model-owned until
close, including unreachable objects and cycles; this is not garbage collection.
A migration away from maximum-width packed arrays is not evidence that every
model uses less total memory or runs faster. Descriptor, allocator, callback,
queue and temporary-storage costs still matter.

### Historical migration checkpoint — 2026-09-19

These are the original archive's recorded results; they were not reproduced by
this documentation update and must not be attributed to the current patch chain.

| Evidence layer | Result | Scope and limitation |
| --- | --- | --- |
| Linux all-feature Rust/HDL nextest | **2,176 passed, 1 skipped** | The only skip is the manual C-vector generator. All three formerly ignored positive RTL defects are enabled. |
| Build and integration checks | Formatting, warnings-denied Clippy, all-target/all-feature and minimal-feature checks, documentation tests and fixture integrity passed | Documentation tests contain no runnable examples. These checks do not establish other host platforms. |
| Native runtime/waveform ownership components | **39/39 passed** with GCC ASan/UBSan | Includes composite/view references, queued masks and allocation plateaus. Real coroutine stack-switch probes are excluded from this component lane. |
| Selected generated-model regressions | **37/37 passed** with GCC UBSan | Public CLI cases run in both optimizer modes. |
| Generated-model GCC ASan/UBSan lane | **4 passed, 33 failed** | libaco stack-switch warnings from `__asan_handle_no_return` fail exact stderr checks. No sanitizer memory-error report appeared, but this is not an ASan acceptance pass. |
| Flattened runtime and ownership ABI | GCC and Clang strict-C11 checks passed | Both accept ABI 4 and reject stale ABI 3. These are component compilation checks. |
| Native Windows/macOS generated simulators | Not executed in this validation | Configured release jobs and historical linkage checks do not qualify these hosts. |

Test commands and methodology are maintained in the
[test guide](../tests/readme.md#dynamic-ownership-validation). Full-suite counts
are test inventory, not counts of distinct supported syntax forms. Group 1
release qualification remains open for generated ASan and native platform evidence.

## Status markers

- 🟦 **Source-implemented**: an implementation path exists for the stated scope.
  Named tests are regression sources unless matching run evidence is explicitly
  recorded; neither their presence nor a historical suite count establishes
  current acceptance or every context in the feature family.
- 🟨 **Partial**: bounded implementation exists, with remaining unsupported forms,
  emitter/lifetime restrictions or unresolved integration boundaries stated on
  the row. A passing bounded regression does not close those remaining forms.
- ❌ **Not implemented / rejected**: no supported execution path is established,
  or the named form is deliberately rejected. Language-illegal forms are identified
  separately and are not implementation backlog.
- ⬜ **Out of scope**: an unimplemented/de-prioritized advanced form; bounded
  implementations in the same family are described separately.
- ✅ **Accepted** is reserved for a stated scope with matching post-change HDL
  evidence. No whole language family is promoted solely from a suite count.

Parenthetical test names identify regression sources and their actual bounded
cases. `(historical probe)` records an earlier manual-audit claim; it is not
independent evidence for an untested context. A missing test anchor does not imply suite
coverage. Source links in the grouped inventory identify implementation or
rejection owners; a file or match arm alone is not a conformance proof.

Revision tags: **[1995]** = IEEE 1364-1995 baseline; **[2001]** = 1364-2001;
**[SV-2005]** = 1800-2005; **[SV-2009]** = 1800-2009;
**[1364-2005]** identifies the interim Verilog revision.

Numeric limits: packed values use exact-width dynamic storage below the exclusive
`LLG_SUPPORTED_WIDTH_LIMIT` (`1 << 20` bits), with `uint32_t` runtime widths.
The model/runtime contract is **value ABI 4**, not a model-maximum inline array.
Zero-width packed descriptors are empty; IR real values use their separate native
representation. Arithmetic and conversions operate over actual limb counts.
The generated subroutine recursion guard is 256. Scheduler and process budgets
default to 10,000,000 and use `LLG_ZERO_LOOP_LIMIT`, `LLG_PROCESS_STEP_LIMIT`
and the latter's `LLG_NONCONVERGENCE_LIMIT` alias.

SYN-036 keeps these limits distinct from fixed-array syntax support. One
generated fixed unpacked array may contain at most 65,536 cells
(`LLG_MAX_FIXED_ARRAY_CELLS`); dimension extents and their product are checked
before storage tables, initializers or C declarations are created. Each
non-real cell still has the packed value limit above. Direct rank-one fixed
array reductions own and read one cell at a time, so the selected boundary can
represent 65,536 17-bit cells (1,114,112 flattened bits). A fixed array used as
a value, formal, stream or other one-payload context must fit one packed value
and reports the packed capacity with the measured payload width. This profile
does not add segmented general aggregate values or automatic conversion to a
resizable SystemVerilog array. The selected cell limit is a generated-model
resource ceiling; IEEE 1364-2001 §§3.3.1, 3.10 and IEEE 1800-2009 §7.4.2 do
not define this project limit. See [`IrArray`](../src/sim/ir/storage.rs),
[`fixed_values.rs`](../src/sim/codegen/lowering/collection/fixed_values.rs),
[`fixed_reductions.rs`](../src/sim/codegen/lowering/containers/fixed_reductions.rs),
and [`SYN-036 fixtures`](../tests/fixtures/sim/syn036_capacity/).

The net-group driver and alias tables are no longer fixed: driver tables are
emitted at the exact elaborated size and the alias table grows on demand with
checked allocation, so neither the old 16-driver nor the 256-alias ceiling
applies. The process registry, final-block registry, named-event ordinary and
persistent-trigger waiter tables, packed/real PCA binding tables and the
force/release entry table likewise grow on demand; `LLG_MAX_PROCS` survives
only as the standalone runtime self-test's sequential-fork base and is not a
concurrency ceiling. These are resource limits, not packed-value capacities,
and every growth path is checked rather than unbounded. See
[`llg_value.h`](../src/sim/rt/llg_value.h),
[`llg_rt.h`](../src/sim/rt/llg_rt.h),
[`src/sim/rt/readme.md`](../src/sim/rt/readme.md) and
[`process_registry.c`](../src/sim/rt/scheduler/process_registry.c).

## Group 1 (practical synthesizable subset) source status — 2026-09-20

The practical RTL work uses the existing owned database, validated IR and C11
runtime. Fixed integral arrays and aggregates now have activation-owned value,
return and reference paths, with file-backed tests in both optimizer modes.
The original defparam, signed-return, fixed-formal, packed-member, dependent
argument-default, stream-selector, and compilation-unit ordering defects have
regressions in `sim_rtl_completion`, `sim_rtl_composition`, and `sim_edition`.

The Group 1 release gate remains open until its task-to-evidence audit and the
required native platform runs are complete. A Linux test result does not prove
Windows/macOS generated-simulator support or every language/context combination.

### R01–R06 implementation map

All rows below are source status, subject to the current qualification boundary.
A bounded repair does not close an entire feature family.

| Review ID | Implemented scope | Material remaining boundary | Regression sources |
| --- | --- | --- | --- |
| R01 | Fixed-array conditional values retain immediate element boundaries and default differing elements on ambiguous selectors. | Bounded fixed integral payloads; not general dynamic/native aggregates. Packed mux policy remains separate. | [sim_rtl_completion.rs](../tests/sim_rtl_completion.rs) (`array_conditional_*`) |
| R02 | Fixed `foreach` uses the iterated type's mixed unpacked/packed dimensions, original omitted slots and declared bounds. | Does not add nested resizable traversal or unsupported iterator captures. | [sim_loops.rs](../tests/sim_loops.rs), [foreach import tests](../tests/slang_semantics/foreach.rs) |
| R03 | Fixed-array `sum`/`product`/`and`/`or`/`xor`, including lexical `with` maps, nested fixed-row reductions and live fixed-array iterator index queries. | Integral mapped results; bounded value receivers; `index()`, constant `index(1)` and checked dynamic integral dimensions are limited to the current fixed unpacked iterator dimension, retaining declared signed bounds and nested iterator identities. Packed item dimensions and higher/unvisited dimensions remain rejected; fixed ordering methods are covered separately by SYN-028. | [sim_fixed_array_reductions.rs](../tests/sim_fixed_array_reductions.rs) |
| R04 | Module-procedural fixed-array expressions use type-directed admission and one complete RHS capture before stores or NBA issue. | Bounded integral representations; no new continuous/timed-assignment paths. The corrected nested-default/NBA cases pass in the current Linux baseline; broader contexts remain unqualified. | [sim_array_conditional_assignments.rs](../tests/sim_array_conditional_assignments.rs) |
| R05 | Directional dissimilar `inout` collapse selects resolver/default/delay on admitted whole, selected/concatenated and fixed net-array-cell connections. | `trireg` and frontend-rejected `uwire` inouts remain excluded; true-alias type rules stay strict; no new whole-array port shapes or net-array declaration-delay capture. | [sim_port_net_types.rs](../tests/sim_port_net_types.rs) |
| R06 | Ordered Boolean `&&&` predicates in `if` and `?:`, with independent branch roles, short-circuiting and full-value truth. | **Partial:** primitive and recursive fixed structure `matches` forms are covered by SYN-022/SYN-023; tagged patterns and pattern-case execution remain unsupported. Timing-check `&&&` is a separate missing feature. | [sim_sequential_predicates.rs](../tests/sim_sequential_predicates.rs) |

Implemented scope:

- Fixed unpacked arrays: reversed/negative bounds by logical coordinates,
  overlapping slice self-assignment, array/struct module ports with per-leaf
  notification, memory declaration initialization and slicing.
- Fixed-array conditional values:
  `ArrayMux` retains the immediate unpacked element width and its
  default-uninitialized payload. Ambiguous selectors preserve known-equal
  elements and default entire differing elements, including nested rows and
  fixed integral records; explicit member initializers are not fallback values.
  Packed-vector muxes follow the supplied edition tables: equal known `0`/`1`
  bits survive, while X/Z branch bits, including equal Z/Z, become X. The
  owned emitter captures each evaluated alternative once; constant folding
  follows the same merge rule.
  Regression sources are in `sim_rtl_completion` (`array_conditional_*`), with
  separate native ownership tests. Current acceptance is subject to the
  [validation status](#latest-supplied-validation-status), not inferred from
  fixture presence.
- Direct fixed unpacked-structure conditional values (SYN-004): `StructMux`
  retains each immediate member boundary, including nested fixed row/record
  members, and defaults a differing member to its type's uninitialized value
  under X/Z selectors. Known selectors evaluate one arm; ambiguous selectors
  evaluate both reached arms once. Packed structures retain ordinary mux
  semantics; native/resizable and tagged structures remain outside this path.
  Regression sources are `sim_rtl_completion.rs`:
  `unpacked_structure_conditionals_*`.
- Sequential Boolean conditional predicates (R06): `if` and `?:` retain all
  `&&&` clauses in source order, with separately captured then/else roles.
  Every reached clause is evaluated once and converted to full-value truth;
  false or ambiguous truth stops the sequence. A later false clause cannot
  resolve an earlier X/Z, unlike ordinary `&&`. Ambiguous `if` takes else;
  conditional values evaluate both alternatives and keep the existing packed
  or R01 aggregate merge. Native real alternatives are both evaluated and
  produce zero on an ambiguous selector (also corrected for ordinary `?:`).
  Predicate folding, dependencies, effects, lint flow and stack accounting
  preserve clause order and early exits. SYN-022 additionally lowers integral
  constant patterns with defined Boolean results, `.*` wildcards and `.name`
  bindings. A successful binding is visible to later `&&&` clauses and the
  true arm, while false/X/Z prefixes suppress later evaluation; pattern
  variables are owned automatic temporaries and are excluded from signal lint
  activity. SYN-023 extends this to recursive fixed packed/unpacked structure
  patterns, evaluating one source snapshot and applying resolved member checks
  in source order. Tagged patterns and pattern-case execution still fail
  explicitly before optimization. The `sim_sequential_predicates` and owned-
  import tests cover these bounded forms in the current Linux baseline. Native
  ownership probes are handwritten counterparts, not output from the Rust
  emitter. The captured `ConditionPattern` semantic edge requires the wrapper
  and Rust consumer to be rebuilt together.
- Dissimilar inout net types (R05): the owned internal/external port roles
  select the effective resolver, implicit pull/supply default and propagation
  delay using IEEE 1364-2001 Table 45 / IEEE 1800-2009 Table 23-1. This applies
  to the existing whole packed, selected/concatenated/aliased packed, and
  fixed net-array cell connection paths. True aliases are checked for matching
  declared net types before port collapse; the relaxed port table does not
  legalize incompatible aliases. Warning table cells retain source locations.
  A selected declaration with no delay stays undelayed even when a dominated
  declaration has a delay. Parent connections precede descendants; sibling
  warning-only ties use stable owned design order, not hash or union-root order.
  The bundled frontend rejects `uwire` inout declarations/actuals; `trireg`
  charge storage, new whole-array port shapes and net-array declaration-delay
  capture are not enabled. Expression-only actuals (no single high declaration,
  but a present `high_expr`) follow their selected/concatenated connections rather
  than being treated as disconnected. `sim_port_net_types` and `slang_semantics`
  contain the regression sources; see the current validation boundary.
- Module-procedural fixed-array expressions (R04): whole-array and selected
  array destinations accept a bounded fixed integral RHS by its owned result
  shape after the established copy/slice/concatenation/pattern/cast paths.
  The complete RHS is staged before cell writes or NBA issue; conditional
  selectors and evaluated arms are not repeated per destination cell. R01's
  `ArrayMux` retains aggregate X/Z/default semantics. Bound simple assignment
  patterns retain each operand occurrence when synthesized default rows share
  expression identities; structural-child deduplication does not change their
  positional arity. Packed-array pattern leaves reached by recursive defaults
  lower at their outer packed element width before concatenation and final
  state/signedness conversion. Positional, integer-index and default keys use
  the existing strict shape checks; this does not add packed type-key matching
  or assignment-pattern lvalues. Specialized per-cell conversions are retained.
  Bound packed ranges are taken from `PackedAtom.ranges`, with checked conversion
  to the pattern resolver's signed index bounds. The
  `sim_array_conditional_assignments` CLI suite and owned-import generation tests
  include `nba`, `nested_defaults`, `typed_defaults` and `deep_defaults`. These
  corrected cases pass in the current Linux baseline; the original NBA fixture
  and exact-output checks were not weakened.
- Fixed-array input value ports (SYN-007): compatible fixed integral input
  arrays accept conditional and function values, assignment patterns, fixed
  slices and runtime-selected rows. The source expression is evaluated once,
  its elements are scattered in declaration order into formal storage, and
  source contents plus runtime row selectors establish link dependencies.
- Fixed output/ref/aggregate port shapes (SYN-008): bounded fixed integral
  output arrays accept whole arrays, constant rows and legal unpacked slices;
  slice coordinates retain their selected order when writing the parent
  storage. Matching fixed-array `ref` ports, fixed aggregate value outputs,
  nested member terminals and module instance-array distribution preserve
  their typed storage or reference identity. Conditional output actuals and
  inequivalent ref shapes remain diagnosed. Real, native, resizable and
  runtime-rewired output/ref/inout combinations remain restricted.
  `sim_rtl_completion` covers the positive matrix and its single-fault
  assignability/reference controls (1800-2009 §§7.4.6, 23.2.2.2, 23.3.2 and
  23.3.3); the existing gate terminal matrix is also run under 1364-2001 for
  scalar/net terminal links (1800-2009 §§28.3-28.6; 1364-2001 §§12.3.6 and
  12.3.9).
- Fixed `foreach` loops: owned per-slot bounds retain mixed unpacked/packed
  dimensions, signed ascending/descending ranges, implicit integral element
  vectors, singleton dimensions, and source omissions. Bounds come from the
  iterated expression's type at import, not flattened storage or enum base
  ranges. Source `break`/`continue` scope and endpoint-before-increment checks
  are retained. `sim_loops` adds `foreach_mixed_*` CLI regression sources in
  both optimizer modes. Test presence and older host results are distinct from
  complete post-correction acceptance.
- Fixed-array reduction methods (R03): `sum`, `product`, `and`, `or`, and `xor`
  lower without resizable-container storage. Each fold retains immediate unpacked
  element boundaries and its self-determined result type; `with` can capture
  enclosing automatic locals/formals and use distinct nested iterator bindings.
  First-element seeding preserves singleton X/Z values. Whole rank-one model
  arrays are read cell-by-cell, so their combined payload need not fit a packed
  value. Other fixed receivers use one bounded captured payload. Iterator
  `index()` and `index(1)` retain declared bounds; checked dynamic integral
  dimensions select the live fixed unpacked iterator dimension, while packed item
  dimensions and higher/unvisited dimensions remain rejected. Fixed integral
  records/rows may be mapped to integral results; native/string/real/resizable-element
  maps are not added.
  `sim_fixed_array_reductions` and owned-import/unit tests cover these paths.
  The bridge publishes method metadata after semantic-table growth so iterator
  capture cannot invalidate a borrowed vector entry. Handwritten native probes
  check reused value/ownership operations separately; see the current validation
  boundary for the complete Rust/HDL path.
- Packed structs and untagged packed unions: cross-view aliasing, per-member
  two/four-state conversion, unequal-width members still rejected.
- Assignment patterns: nested keyed/default patterns (including a default that
  fans out over a fixed-array member), positional side effects, duplicate explicit-index and
  missing key rejections; sub-aggregate patterns and deep copies.
- Packed selections: constant and runtime packed-dimension/part/bit/indexed-part
  chains beneath a fixed-array element, for reads and writes, with host-safe X/Z and
  out-of-range behavior: X read positions and in-range-only writes.
- Streaming: slice tails, runtime `with` selectors on fixed-array destinations
  and (1-D) sources, staged overlap-safe stores; size/type mismatches rejected.
- Packed variable blocking/nonblocking assignment: issue-time capture, source-ordered commit,
  overlapping masked updates; NBA to automatic variables and through `ref`
  formals remains rejected per IEEE 1800-2009 §6.21/§10.4.2.
- always_comb/always_latch: time-zero execution, written-expression exclusion
  and transitive function reads. always_ff retains its event-control contract;
  all three retain their implemented conflicting-writer diagnostics.
- Control flow: nested `foreach` cleanup, case/casez/casex/`case inside`
  wildcard rules, qualified string `case inside`, unique/priority diagnostics.
- Resolved nets: small-width resolution truth matrix, fixed wired arrays with
  pull/supply defaults, interface instance identity, selected scalar/array inouts,
  shared and nested inout peers, aggregate net members, hierarchical/concatenated
  drivers, alias strengths, delayed wakeup and Postponed reads.
- Gates: terminal matrix, gate-array distribution, tri-state X/Z enables.
- Calls: defaults evaluate only when omitted and can read earlier captured
  inputs without repeating side effects. Fixed integral arrays/structs/unions
  support value inputs, locals, returns, output/inout copyout, recursive `ref`
  and selected `const ref` forwarding. Illegal packed bit/part reference actuals
  and NBAs through subroutine reference formals remain rejected.
- Initialization: legal zero-time user calls for scalar and fixed composite
  declarations finish before SystemVerilog processes start. Explicit struct
  member defaults and mixed two/four-state leaf defaults survive arrays,
  automatic/static locals, output formals and returns.
- Let expressions, constant functions in generate/width contexts, located
  rejection of a reached `case ... matches` pattern, and a three-axis
  synthesis classification (simulation support vs synthesis policy).
- Zero-time effect summaries: callback-safe numeric helpers with private
  locals, loops, nested calls, fixed-array reductions, and stateless static
  formals in evaluated event controls; visible writes, timing and foreign
  effects retain distinct rejections.
- Edition enforcement: standard builtin/keyword inventories plus kind-scoped
  later-form rejection. `$asserton`/`$assertoff` are admitted under 2009, not 2001;
  `$assertcontrol` is not a standard builtin of either requested edition.
  Unknown system names require explicit registration. Admission is distinct from
  simulation support; this is not a proof of complete edition conformance.
  `defparam` re-elaboration and source-indexed module instance arrays.
- Capacity: the net driver/alias, process, final, named-event waiter, PCA and
  force registries all grow with checked allocation.

Retained boundaries:

- The general fixed-array expression path uses a bounded packed payload;
  payloads above the backend limit and expressions without a supported value
  representation still fail closed. Dynamic/native aggregate elements remain
  outside the fixed integral array-merge operation. R04 does not change the
  separate continuous-assignment or intra-assignment timing paths.
- Static fixed-array port connectivity uses constant elaborated array
  coordinates. Compatible fixed integral input value expressions use the
  captured value-link path described above. Dissimilar net types in the
  supported R05 port table are no longer a blanket rejection; incompatible true
  aliases, unsupported net classes/layouts and dynamic resolved-net targets
  still reject. Pairwise table selection does not establish order-independent
  semantics for every multiway warning-only conflict.
- Streaming `with` selectors follow the admitted one-dimensional operand forms;
  native/string/object extensions remain separate from fixed integral RTL.
- Dynamic/native aggregate fields, tagged unions and later verification/foreign
  facilities retain the restrictions in their owning rows below.
- No complete IEEE syntax percentage or cross-platform release acceptance is
  inferred from this regression inventory.

## Coverage and remaining work

The twelve feature sections and the original 72 grouped audit IDs are retained.
The [grouped inventory](#remaining-work-inventory) now separates source-implemented,
partial and missing capabilities rather than calling unaccepted patches completed.
No percentage of IEEE coverage or current failure count is inferred from these
categories. Prioritize explicit correctness/lifetime gaps and public-pipeline
acceptance before expanding unsupported combinations.

## Correction qualification boundaries

The current array-pattern corrections preserve repeated positional operand
edges, visit type-bound nested defaults rather than error placeholders, and lower
bounded packed-array leaves using their declared ranges. The first corrective
patch also repairs method-metadata publication after vector growth and follows
expression-only concatenated inout actuals. These are source corrections, not
suppressed diagnostics or relaxed expected-output checks. The
[latest supplied validation status](#latest-supplied-validation-status) records
why the complete corrected pipeline still needs a host rerun.

The earlier migration correction removes the invalid vector/string-key reference lookup,
passes the native index type to `llg_ref_write_bit`, places addressable automatic
real locals in registered heap payloads, and pins those payloads during real-value
publication. Corresponding regression sources are in
[`owned/tests/review_regressions.rs`](../src/sim/emit_c/owned/tests/review_regressions.rs)
and [`review_lifetimes_probe.c`](../tests/runtime_value_storage/review_lifetimes_probe.c).
These changes do **not** enable queued writes to automatic real/string destinations.
Existing mailbox-nominal, queue-reference and sequence-correctness corrections also
remain subject to current HDL acceptance; their presence is not a completion mark.

### Explicit current migration restrictions

| Boundary | Implemented scope and remaining restriction | Source owner |
| --- | --- | --- |
| Pre-process initialization | Typed scalar and fixed composite initializers, including zero-time user calls, execute before SystemVerilog processes. Timing-bearing initializer calls remain illegal. | [calls](../src/sim/emit_c/owned/calls.rs), [initialization](../src/sim/emit_c/owned/model/initialization.rs) |
| Native delayed destinations | Addressable automatic real/string values have registered storage, but queued writes to automatic real cells and delayed writes to automatic strings remain rejected. Persistent destinations have source paths. | [stores](../src/sim/emit_c/owned/stores.rs), [statements](../src/sim/emit_c/owned/statements.rs) |
| Reference writes | Packed and fixed aggregate/array references retain canonical leaf views; retained packed queue references have separate cells. NBAs through subroutine reference formals, general native/resizable aggregate arguments and selected reference file destinations remain restricted. | [references](../src/sim/emit_c/owned/references.rs), [stores](../src/sim/emit_c/owned/stores.rs), [input](../src/sim/emit_c/owned/input.rs) |
| Read-only evaluators | Numeric helpers can use private locals, loops, nested eligible calls and fixed-array integral reductions; stateless static helpers with numeric value/const-ref formals use private callback copies. Writes to visible or persistent non-return state, unsupported native receivers and DPI calls remain excluded from read-only callbacks, while unique/priority checks are retained. | [pure calls](../src/sim/emit_c/owned/pure_calls.rs), [event contracts](../src/sim/codegen/lowering/collection/call_contracts.rs), [callbacks](../src/sim/emit_c/owned/model/callbacks.rs) |
| Activation captures | Owned packed/real values and recognized opaque handle snapshots are implemented. Borrowed/shared ownership and arbitrary native/string/process captures remain rejected. | [captures](../src/sim/emit_c/owned/captures.rs) |
| Monitor dependencies | Persistent packed, real, string and container markers have paths. Automatic monitor dependencies remain rejected; read-only evaluator limits also apply. | [runtime tasks](../src/sim/emit_c/owned/runtime_tasks.rs) |
| Array methods and queries | Fixed-array integral reductions use lexical maps and can capture automatic locals/formals (R03); non-direct receivers retain bounded packed payloads, and iterator queries accept `index()`/constant `index(1)` or a checked dynamic integral dimension for the live fixed unpacked iterator. Fixed integral arrays support `reverse()` over declaration-order elements and `sort()`/`rsort()` with integral maps, including selected rows, packed records, repeated keys, reversed/negative ranges, singleton arrays, and automatic locals/formals (SYN-027/SYN-028). Reverse and sort with-clause or const-ref cases outside those contracts remain rejected. Packed item and higher/unvisited dimensions remain rejected. Resizable-container callbacks still require packed items and cannot capture automatic locals/formals. Non-packed queue endpoint/pop expressions and several nested scalar-query forms remain rejected. | [fixed reduction lowering](../src/sim/codegen/lowering/containers/fixed_reductions.rs), [fixed-array method lowering](../src/sim/codegen/lowering/containers/methods.rs), [fixed reduction emission](../src/sim/emit_c/owned/fixed_array_reductions.rs), [container callbacks](../src/sim/codegen/lowering/containers/callbacks.rs) |
| Strings and object sensitivity | Native string expressions/formals/returns and string dependency markers exist. Non-string object dependencies, handle arithmetic and general native-object sensitivity are not enabled. | [native values](../src/sim/emit_c/owned/native.rs), [events](../src/sim/emit_c/owned/events.rs) |
| Process/event formals | Process-handle formal ABI is still rejected. Named-event formals use inline lowering templates, not numeric C procedures; event-formal virtual-interface dispatch remains rejected. | [objects](../src/sim/emit_c/owned/objects.rs), [model](../src/sim/emit_c/owned/model.rs) |
| Class/interface tasks | Typed class construction, members and delay-free dispatch exist. Timing-bearing class/virtual-interface tasks remain rejected. Class allocations stay live until model close. | [task lowering](../src/sim/codegen/lowering/statements/calls.rs), [classes](../src/sim/emit_c/model/classes.rs) |
| Streaming | Packed and mixed packed/container forms require packed stream data and at most one resizable destination. Fixed nested aggregates, selected rows/members, one-time source snapshots, and `ref`/`const ref` projections are qualified by `sim_data_types_next.rs`; associative destinations, native-string/recursive object streams and generalized aggregate/reference combinations remain outside the path. | [streaming](../src/sim/emit_c/owned/streaming.rs), [stream lowering](../src/sim/codegen/lowering/containers/streaming.rs) |
| Aliases and force | R05 supports directional table-based collapse, source-located warnings and winning-declaration delays on admitted inout paths. SYN-010 extends true aliases to fully indexed fixed net-array elements and multidimensional packed projections, retaining same-declared-type checks, canonical drivers, force/release, dependency and waveform identities. Expression-only concatenated actuals are traversed. Explicit alias-force mappings exist. Unmapped alias writes, dynamic selections, variables, forbidden cross-scope forms, automatic/unsupported indirect force targets and unsupported resolved-net contexts still reject. | [net lowering](../src/sim/codegen/lowering/collection/nets.rs), [port type plan](../src/sim/codegen/lowering/collection/port_net_types.rs), [force](../src/sim/emit_c/owned/force.rs), [SYN-010 regression](../tests/sim_net_resolution.rs) |
| Assertions and sampling | Bounded graph/predicate/history callbacks exist; general temporal properties, multi-cycle combinators, wider local-formal/action forms and real-valued sampled registrations remain restricted. | [assertion lowering](../src/sim/codegen/lowering/assertions.rs), [assertion emission](../src/sim/emit_c/owned/assertions.rs) |
| Foreign interfaces | Bounded scalar DPI imports and VPI call-site callbacks exist. DPI exports/open arrays/context callbacks and the full PLI/VPI interface remain unsupported. | [DPI contracts](../src/sim/codegen/lowering/collection/signatures.rs), [DPI emission](../src/sim/emit_c/model/dpi.rs), [VPI](../src/sim/rt/llg_vpi.c) |
| Unstructured/opaque IR | Jumps into another lexical scope, backward unstructured jumps and opaque executable fragments remain rejected; structured loops have a separate implementation. | [statements](../src/sim/emit_c/owned/statements.rs), [expressions](../src/sim/emit_c/owned/expressions.rs) |

## Target language editions

`llg` selects one compilation-wide language policy with `--edition 2001` or
`--edition 2009`. The default is SystemVerilog-2009. The native frontend keeps
that policy separate from lexical `` `begin_keywords ``/`` `end_keywords ``
regions: those directives may select a local keyword table, but cannot change
the owned model's edition or the simulator's edition-sensitive rules.

The 2001/2009 policy has frontend parsing and simulator time-literal paths.
The existing regression ledger is intentionally narrow: under
`--edition 2001`, SV-only syntax is rejected by the global policy; a
`` `begin_keywords "1800-2009" `` region changes lexical keywords only and
does not change the owned edition; under `--edition 2009`, local
`timeunit`/`timeprecision` and signed/sub-femtosecond unit-suffixed literals
use exact femtosecond boundary rounding; scientific delay spellings have separate regression sources. Initialization rules, accepted
type/operator families, function restrictions, and legacy compatibility APIs
remain separately tracked in the feature rows below; selecting an edition does
not claim that those later boundaries are complete. The pinned Slang frontend
exposes SystemVerilog semantic checks starting at its 2017 API level, so the
2009 policy uses its exact 2009 keyword table and Lapligence's 2009 time rules
while those later semantic boundaries remain an explicit inventory item.

### SYN-019 selected edition boundary ledger — 2026-09-20

This ledger closes the selected Core-elaboration cells from the SYN-019 plan.
The supplied IEEE 1364-2001 and IEEE 1800-2009 PDFs and the local clause maps
under `docs/specification/` are the normative inputs; no local errata file was
available to amend these rulings. Every public CLI fixture below runs through
`llg` and `llg --no-opt` in [`sim_edition.rs`](../tests/sim_edition.rs).

| Selected cell | IEEE 1364-2001 | IEEE 1800-2009 | Evidence and policy owner |
| --- | --- | --- | --- |
| ANSI and non-ANSI module ports, continuous assignments, `always @*`, `$display`, `$finish`, `$signed` | Accepted | Accepted | [`edition_legacy_forms.sv`](../tests/fixtures/sim/partial_features/edition_legacy_forms.sv); §§6.2, 9.7.5, 12.3, 17.4, 4.5. |
| Scalar declaration initialization | Accepted; active-process race is retained | Accepted; static initialization precedes ordinary processes | [`declaration_init_edition.sv`](../tests/fixtures/sim/partial_features/declaration_init_edition.sv); 1364 §6.2.1 and 1800 §§6.8, 6.21, 10.5. |
| `logic`, `typedef`, packed `struct`, type parameters, assignment patterns, and `always_comb` | Rejected by frontend or owned strict policy | Accepted in the selected finite forms | [`edition_2009_sv_types.sv`](../tests/fixtures/sim/partial_features/edition_2009_sv_types.sv) and the single-fault 2001 fixtures; 1800 §§6.18, 6.20.3, 7.2.1, 9.2.2.2, 10.9. |
| `$clog2` in a constant context | Rejected by the owned system-name policy | Accepted in the selected constant context | [`edition_2001_clog2.sv`](../tests/fixtures/sim/partial_features/edition_2001_clog2.sv), [`edition_2009_sv_types.sv`](../tests/fixtures/sim/partial_features/edition_2009_sv_types.sv); 1800 §20.8. |
| Whole fixed unpacked assignment, equality, conditional value, and unpacked array ports | Rejected by the owned type-aware policy; memory declarations and element selects remain legal | Accepted in the selected finite forms | [`edition_2009_whole_arrays.sv`](../tests/fixtures/sim/partial_features/edition_2009_whole_arrays.sv) and paired 2001 fixtures; 1800 §§7.4.2, 7.6, 11.4.11, 23.2.2.2. |
| `` `begin_keywords `` and macro scopes | Old keyword tables and macro replacement cannot relax the selected policy | An old 1364 table can make `logic` an identifier while the owned edition remains 2009 | [`edition_begin_keywords_legacy_identifier.sv`](../tests/fixtures/sim/partial_features/edition_begin_keywords_legacy_identifier.sv), [`edition_2001_begin_keywords_array.sv`](../tests/fixtures/sim/partial_features/edition_2001_begin_keywords_array.sv), and [`edition_2001_macro_sv_only.sv`](../tests/fixtures/sim/partial_features/edition_2001_macro_sv_only.sv); 1800 §§22.5, 22.14. |
| Later builtin and assertion forms (`$countbits`, `assert final`, `$assertcontrol`) | Rejected | Rejected when absent from the selected 2009 set | [`edition_2009_countbits.sv`](../tests/fixtures/sim/partial_features/edition_2009_countbits.sv), [`edition_assert_final.sv`](../tests/fixtures/sim/partial_features/edition_assert_final.sv), [`edition_assertcontrol.sv`](../tests/fixtures/sim/partial_features/edition_assertcontrol.sv). |

The whole-array policy is semantic rather than a keyword blacklist because the
newer Slang API accepts these memory values under the 2001 parser mode. The
owned check examines fixed-unpacked value nodes and array ports, reports a
located edition diagnostic, and leaves indexed memory access unchanged. Body
expressions are present only in execution snapshots; navigation/library-unit
snapshots intentionally omit them, so this ledger does not claim body-level
whole-array diagnostics in that read-only mode. Array-port declarations remain
visible and are checked in both snapshot modes.

The following cells remain untested by this ledger: the complete 1364/1800
Annex A production sets; switch/transistor primitives, specify and timing-check
forms, and PLA/file system
task catalogs; package/interface/class/program/clocking/assertion grammar
combinations; dynamic, associative, queue, and whole-array subroutine argument
forms; aggregate or memory declaration-initialization variants beyond the
scalar race probe; port default/ref directions; and later-edition boundaries
beyond the selected `assert final`/`$countbits` probes. The positive cases are
execution witnesses for the named finite forms, not a synthesis-completeness
claim.

### SYN-031 selected combinational UDP ledger — 2026-09-20

The selected UDP cell accepts scalar combinational user-defined primitive
definitions and instances for both language editions. `Db::from_slang` owns the
normalized table, port count, symbols, and source order before simulation
lowering. Rows accept `0`, `1`, `x`, `b`, and `?`; runtime `z` matches the `x`
input symbol, and an unmatched input combination produces `x`. Slang's table
overlap decision is repeated at the owned boundary so conflicting overlapping
rows cannot enter the simulator. Output driver identity, optional legal gate
strengths/delays, repeated instances, primitive instance arrays, and net
resolution retain the ordinary structural-driver path. The public
`udp_comb.sv` fixture runs with and without optimization and the owned database
test checks the copied table rows.

The accepted references are IEEE 1364-2001 §§8.1, 8.2, 8.6 and IEEE 1800-2009
§§29.3, 29.4, 29.8, as mapped by the checked-in specification references. The
negative fixtures cover sequential level/edge UDPs (1364 §§8.3–8.5; 1800
§§29.5–29.7) and edge-sensitive table rows. Vector or aggregate terminals,
switch/transistor primitives, and specify/SDF behavior remain outside this
cell.

### Compilation-unit grouping

The driver accepts `--compilation-units separate|merged` (default `separate`)
and the compile APIs expose the same typed choice. Separate mode gives every
admitted source buffer its own preprocessor and `$unit` scope. Merged mode
passes all admitted compilation-unit buffers, in caller order, to one Slang
syntax tree; source buffers remain distinct for diagnostics and owned source
identity. Include-only buffers are never promoted to compilation units and
remain owned by the source that includes them. The choice is recorded in the
owned snapshot, so cached results cannot be mistaken across modes.
The driver also accepts repeated `--include-dir <path>`/`-I <path>` and
`--define <NAME[=VALUE]>`/`-D <NAME[=VALUE]>` options. The compile boundary
resolves literal and bounded macro-expanded include names only below the
including source or an explicitly supplied include root, and supplies the
admitted bytes to Slang's cache-only source manager.

---

## 1. Lexical & preprocessing relevant to simulation

Verilog era:

- 🟦 **Integer literals** `[size]'base value`, x/z digits, `_` separators `` `4'b1001` `` — §1364-2001 2.5.1 **[1995]**
- 🟦 **Signed literals** `` `-8'd6`, `4'shf` `` — §1364-2001 2.5.1 **[2001]**
- 🟦 **Real literals** `1.2`, `2.5e10` — §1364-2001 2.5.2 **[1995]**
- 🟦 **String literals** `"..."` as display format strings — §1364-2001 2.6 **[1995]** (SystemVerilog `string` has its own partial row, see §2)
- 🟦 **Lexical base** comments, identifiers, escaped identifiers, operators — §1364-2001 2.1–2.7 **[1995]** via the Slang frontend
- 🟦 **Attributes** `(* full_case *)` parsed+ignored — §1364-2001 2.8 **[2001]** consumed by frontend, no sim effect
- 🟦 **`` `define ``/`` `undef `` macros incl. arguments** — §1364-2001 19.3 **[1995]** expanded pre-elaboration; argument substitution, token concatenation and stringification reach the executable model (sim_directive_effects.rs)
- 🟦 **`` `ifdef `` family incl. `` `elsif ``/`` `ifndef ``** — §1364-2001 19.4 **[1995]** (`elsif`/`ifndef` are [2001]); selected and skipped branches are fixed before elaboration and have both-edition CLI controls (sim_directive_effects.rs)
- 🟦 **`` `include ``** — §1364-2001 19.5 **[1995]** literal and bounded object/function-like macro-expanded filenames are admitted through canonical source/include roots in both compilation-unit modes; include order and unavailable-file diagnostics have public CLI coverage, while missing, dynamic, and unauthorized paths remain frontend diagnostics (sim_directive_effects.rs)
- 🟦 **`` `timescale `` honored** scales `#N` and rounded integer `$time`/`$stime` with Slang's resolved owning-module time unit and precision — §1364-2001 19.8 **[1995]** compilation-unit/declaration inheritance is resolved by the frontend; scheduler ticks use checked femtoseconds across 1fs through 100s (sim_physical_time.rs and sim_timescale.rs)

SystemVerilog era:

- 🟦 **Fill literals** `'0/'1/'x/'z` — §1800-2009 5.7.1 **[SV-2005]** context sizing in supported packed arithmetic/bitwise expressions, comparisons, conditional branches, assignments, function arguments, and case/casez/casex; self-determined concatenation/replication operands remain one bit (sim_fill_literals.rs, optimization on/off)
- 🟦 **Time literals** `2.1ns` — §1800-2009 5.8 **[SV-2005]** the owned semantic model retains Slang's typed unit and source provenance. In the default 2009 policy, ordinary expression use rounds the scope-scaled value to the owning precision (including arithmetic, parameters, and initializers); a complete procedural delay expression is converted and rounded separately once at its delay boundary. Local `timeunit`/`timeprecision`, signed/sub-femtosecond unit suffixes, exact femtosecond boundary values, and scientific delay spellings are represented; runtime scheduling uses checked 64-bit femtosecond ticks, including 1fs precision

## 2. Data types

The [testing methodology and coverage](../tests/readme.md) describe the
regression type combinations, truth tables, conversions, and width boundaries.

Verilog era:

- 🟦 **reg scalars/vectors** `reg [7:0] r;` — §1364-2001 3.2.2/3.3 **[1995]** (sim_counter.rs)
- 🟦 **signed net/reg declarations** `reg signed [3:0] s;` — §1364-2001 3.3/3.8 **[2001]** `%d` prints two's complement
- 🟦 **integer variables** — §1364-2001 3.9 **[1995]**
- 🟦 **time variables** 64-bit unsigned storage — §1364-2001 3.9 **[1995]** (sim_counter.rs)
- 🟦 **wire/tri nets** — §1364-2001 3.7 **[1995]** tri resolution inside inout net groups and ordinary per-continuous-assignment driver groups; plain tri behaves like wire (sim_inout.rs, sim_net_resolution.rs)
- 🟨 **memories/unpacked arrays N-D** element bit/part/indexed-part selects, declared packed ranges, two-state conversion, guarded indices and masked delayed NBAs — §1364-2001 3.10 **[1995]**. SystemVerilog whole-array expression/assignment, pattern and reduction support is recorded separately below; those later forms are not admitted as Verilog-2001 syntax. General element/subprogram/port combinations remain bounded (sim_memory.rs, sim_p30_fixed_arrays.rs, sim_partial_features.rs).
- 🟦 **Net declaration assignment** `wire w = expr;` — §1364-2001 3.6 **[1995]** behaves as a continuous driver for constant and dynamic RHS expressions, using the same event-driven run-once/sensitivity-loop IR as an explicit `assign` (sim_net_decl.rs); fixed-array and resizable-container reads use stable dependency markers, while unsupported resolved-net classes remain rejected explicitly
- 🟨 **Variable declaration initializers and procedural lifetimes** scalar `reg x = 0;`, `logic l = 1'b0;`, `int x = P+1;` — §1364-2001 6.2.1 / §1800-2009 6.8, 6.21, 10.5 **[2001/SV-2005]** declaration identity and resolved static/automatic lifetime are preserved. Admitted scalar and fixed integral composite initializers, including zero-time user-subprogram calls, execute before processes under the 2009 policy; explicitly selected Verilog-2001 retains its initialization scheduling. Static block/subprogram locals initialize once, automatic locals on each activation. Timing-bearing calls and unrepresented aggregate/native/call combinations are not enabled by this path (sim_varinit.rs, sim_geninit.rs, sim_variable_lifetime.rs, sim_edition.rs, sim_rtl_completion.rs: `fixed_initializers_execute_before_initial_processes`).
- 🟦 **Parameters** override + propagation — §1364-2001 3.11.1 **[1995]** (elab_resolve.rs)
- 🟦 **localparam** — §1364-2001 3.11.2 **[2001]**
- 🟨 **real/realtime/shortreal** — §1364-2001 3.9 **[1995]** scalar/fixed-array storage, parameters, scalar ports, value-formal calls, arithmetic, ordinary real `case`, changed-write notifications, waits, any-change events and numeric captures have source paths. Generic containers have real/shortreal leaf operations; this does not enable general real-reference calls or real method callbacks. Addressable automatic real locals now use registered heap storage across blocking mailbox delivery; publication pins their owner. Queued writes to automatic real cells remain rejected (sim_real.rs, sim_mailboxes.rs, owned/tests/review_regressions.rs).
- 🟨 **wand/wor/triand/trior wired resolution** — §1364-2001 3.7.2, 6.1, 12.4 / §1800-2009 6.6, 10.3, 23.6 **[1995/SV-2005]** standalone packed nets and bounded directional port-collapsed inout groups (R05, Tables 45/23-1) resolve per-site drivers with ordered strength endpoints, including scalar continuous, gate, port-derived and admitted hierarchical continuous sources. Hierarchical parent-to-child, selected constant-bit/part, generated-name and upward-qualified driver paths retain an owning-instance contribution slot, independent wakeups and Z-backed values. Z is neutral, stronger endpoints dominate, and equal-strength ties use wired-AND/OR rules (sim_net_resolution.rs, runtime_values.rs). Selected delayed continuous drivers retain independent Z-backed slots (sim_inout.rs). Admitted selected/concatenated scalar and fixed net-array-cell inout paths support dissimilar resolver classes, source-located warnings and winning-declaration delays. General aggregate ports, procedural net writes and function/task-output driver contexts remain restricted; explicit vector continuous-assignment strengths retain their existing legality checks. Tests are in sim_net_resolution.rs and sim_port_net_types.rs; the accepted hierarchical scope is covered in both Verilog-2001 and SystemVerilog-2009 public-CLI fixtures.
- 🟨 **tri0/tri1/trireg/supply0/supply1 pull semantics** — §1364-2001 3.7 **[1995]** standalone `tri0/tri1` apply implicit pulls only to all-Z bits after ordinary-driver resolution; `supply0/supply1` dominate ordinary drivers, with correct initial defaults (sim_net_defaults.rs, runtime_values.rs); same bounded standalone-driver restrictions as wired nets; `trireg` charge storage and resistive propagation remain unsupported
- 🟨 **drive strength / charge strength** — §1364-2001 3.4 **[1995]** scalar continuous, gate, wired and collapsed-inout drivers preserve ordered supply/strong/pull/weak/high-Z endpoints, with implicit tri and supply defaults; explicit vector continuous-assignment strengths remain prohibited by §10.3.4, and `trireg` charge storage/resistive propagation remain unsupported
- ❌ **specparam** — §1364-2001 3.11.3 **[1995]** specify blocks unsupported
- 🟨 **vectored/scalared hints** — §1364-2001 3.3.2 **[1995]** declarations are admitted and retain their packed simulation values; the attributes are advisory and have no dedicated simulator or vendor-synthesis semantics (sim_directive_effects.rs)

SystemVerilog era:

- 🟨 **logic/bit vectors** — §1800-2009 6.9/6.11 **[SV-2005]** `logic` has 4-state storage; `bit`/2-state vectors coerce X/Z to zero on assignments and casts. Scalar/vector paths and the represented packed-aggregate paths have source implementations; unsupported aggregate/net member contexts remain outside this claim
- 🟨 **byte/shortint/int/longint** 2-state ints — §1800-2009 6.11 **[SV-2005]** widths/signedness and X/Z-to-zero coercion are implemented for scalar/vector paths and the represented packed aggregates; unsupported net/member contexts remain outside this row's claim
- 🟦 **uwire nets** — §1800-2009 6.6.2 **[SV-2005]** checked compilation rejects overlapping drivers; standalone whole/declaration and disjoint constant-selected drivers preserve four-state values and undriven Z bits. Driver release, generate scopes, ordinary ports/interface defaults, and undriven net arrays have regression sources for both optimizer modes (`sim_type_conformance.rs`). Same-type aliases without inout edges retain their existing path. The bundled frontend rejects `uwire` inout declarations/actuals; the R05 table unit tests do not establish their execution (sim_port_net_types.rs).
- 🟦 **typedef simple/packed-vector aliases** — §1800-2009 6.18 **[SV-2005]** resolved by frontend (historical probe)
- 🟨 **Array declaration initializers** `'{…}` — §1800-2009 7.6/10.9.1 **[SV-2005]** fixed integral array values use declaration-order element correspondence and typed defaults, including admitted runtime zero-time calls in the 2009 pre-process phase. This is no longer a constant-elements-only capability. Nested-default corrections retain repeated bound operands and packed-vector leaf ranges; the corrected cases pass in the current Linux baseline. Unsupported native/resizable element layouts remain outside this claim (sim_memory.rs, sim_rtl_completion.rs, sim_array_conditional_assignments.rs).
- 🟨 **Fixed unpacked array values, assignments and reductions** — §§1800-2009 7.4, 7.6, 7.12.3, 11.4.11 **[SV-2005]** R01 retains array-specific conditional merging; R04 admits bounded type-compatible module-procedural expressions with one RHS snapshot; R03 implements integral reductions and lexical `with` maps without converting fixed arrays to resizable containers. Whole copies, overlapping slices, partial-index rows, reversed/negative bounds and fixed integral subroutine/port paths are represented. SYN-036 caps generated fixed-array storage at 65,536 cells and keeps direct rank-one reductions cell-wise; value/formal/stream contexts require one packed payload and report that measured capacity. Refer to §5 for assignment boundaries and §7 for operator/method boundaries. Native/resizable aggregate elements, unsupported shapes and values exceeding the bounded payload path remain restricted (sim_p30_fixed_arrays.rs, sim_rtl_completion.rs, sim_array_conditional_assignments.rs, sim_fixed_array_reductions.rs, [SYN-036](../tests/sim_syn036_capacity.rs)).
- 🟦 **enum-typed scalar variables and runtime methods** — §1800-2009 6.19/6.19.5 **[SV-2005]** stored at the elaborated packed base width; enum constants fold through the frontend, and declaration-order first/last/next/prev/num/name methods retain sparse signed values, wrapping step counts, owned names, and the four-/two-state invalid-value defaults (sim_operator_semantics.rs, sim_data_types_next.rs)
- 🟨 **packed/unpacked struct and union aggregates** — §1800-2009 7.2–7.3, 7.4 **[SV-2005]** packed patterns retain positional/named/default/type-key forms, exact state/signedness and member overrides. Packed unions use overlapping width-matched views; fixed recursive unpacked struct/array leaves have initialization, deep copy and member paths for represented packed, real, string and chandle storage. Unequal-width unpacked untagged unions use the maximum packed member extent. Bounded recursive reference-port leaves and collected aggregate copy links exist; tagged unions, unsupported declaration/element layouts, native/resizable aggregate subroutine storage and general slices remain restricted. Recursive type identity is present, not a missing blanket capability (sim_data_types_completion.rs, sim_partial_features/ports.rs).
- 🟨 **string type/signals/params** — §1800-2009 6.16 **[SV-2005]** module/static/automatic byte strings, copy/casts, core methods, `.atoreal`/`.realtoa`, dynamic formatting, value and string-reference formals, output/inout copy-out and automatic/static returns have ownership-aware source paths. Collected string input/output links and string sensitivity use dependency markers. Returns/inputs are independently owned and copy-out follows cancellation checks. Automatic string NBA destinations, string captures not representable by the activation frame, automatic monitor dependencies and broader aggregate/continuous/native combinations remain restricted (sim_data_types_next.rs, sim_h04_string_format.rs, sim_partial_features/ports.rs).
- 🟨 **event data type** scalar and fixed unpacked `event` declarations, runtime-indexed selects, hierarchical references, null/default handles, and task-formal aliases preserve stable event-object identity — §1800-2009 6.17 **[SV-2005]**; dynamic/associative/queue event storage remains outside the bounded subset
- 🟨 **dynamic arrays / associative arrays / queues** — §1800-2009 7.5/7.8/7.10/7.12 **[SV-2005]** source paths include allocation/resize/delete, copy, packed and descriptor-backed generic/nested leaf operations, bounded initialization/patterns, queue slices/overflow, associative defaults/traversal, collected value-port copies and contents/shape dependencies. Packed reductions and `with` methods, locator/min/max/unique queues and sort/rsort/reverse/shuffle are represented. Generic storage does not remove packed-only method/result boundaries: automatic callback captures, string-key index-result queues, non-packed queue endpoint/pop expressions, unsupported nested scalar accesses, general subroutine storage and broader recursive/object forms remain restricted. Shuffle retains its container seed API rather than full process/object RNG integration (sim_data_types_next.rs, sim_data_types_completion.rs, sim_array_sensitivity.rs).
- 🟨 **chandle** — §1800-2009 6.14 **[SV-2005]** null/copy/identity/Boolean operations, automatic/static locals, represented aggregate/class members, mixed packed/native signatures, output/inout/ref/const-ref aliases and native returns have source paths. Chandle is a typed native pointer, not a packed integer. Ordinary chandle value-port links, packed containment, arithmetic, general continuous assignment and non-string object sensitivity remain unsupported (sim_data_types_next.rs, sim_type_conformance/storage.rs).

## 3. Modules, ports, parameters, hierarchy

Verilog era:

- 🟦 **Module definitions/instantiation** positional + named `.p(e)` connections — §1364-2001 12.1.2/12.3.5–6 **[1995]** (sim_hier.rs)
- 🟦 **ANSI port headers** `module m(input a, output [3:0] b);` — §1364-2001 12.3.2–12.3.3 **[2001]** (sim_counter.rs)
- 🟦 **Non-ANSI port headers** — §1364-2001 12.3.4 **[1995]**
- 🟦 **Parameter override** `m #(.N(8)) u (...)` — §1364-2001 12.2.2 **[1995]**
- 🟦 **generate for/if/case + genvar** processes & instances inside gen scopes — §1364-2001 12.1.3 **[2001]** (sim_geninit.rs)
- 🟦 **Module instance arrays** `sub u[1:0](e);` elaborated per index — §1800-2009 23.3.2 **[1995]** (historical probe)
- 🟦 **Hierarchical name reads** `top.u.sig` in expressions/display args — §1364-2001 12.4 **[1995]** (sim_hier.rs)
- 🟦 **Upward name references** partially-qualified paths (`mid.sig` from an inner scope, up-then-down included) resolve through the elaborated ref binding — §1364-2001 12.4 **[1995]** (historical probe)
- 🟦 **Hierarchical select reads & writes** — §1364-2001 12.4 **[1995]** typed bit/part-select expressions preserve blocking and nonblocking targets, including variable bit indices in admitted storage (sim_hier.rs)
- 🟦 **defparam** — §1364-2001 12.2.1 **[1995]** the frontend applies overrides during re-elaboration, and the owned importer records the consumed declaration as an overridden parameter assignment rather than executable syntax. Selected-instance parameter values and dependent widths have regression sources; attempts to override a localparam are rejected (elab_resolve.rs, sim_geninit.rs, sim_rtl_completion.rs). This is not a runtime parameter-mutation facility.
- ⬜ **Configurations/libraries** — §1364-2001 ch13 **[2001]** out of scope

SystemVerilog era:

- 🟦 **Interfaces + modports** actuals and member references bind directly to concrete interface storage; interface-body processes emit on the actual instance — §1800-2009 25.3/25.5 **[SV-2005]** (sim_interface.rs, sim_interface_body.rs)
- 🟨 **Virtual-interface handles and dispatch** typed handles retain concrete instance identity and optional modport views through rebinding, represented class/formal/fixed-array and bounded dynamic-array/queue handle storage, packed member reads/writes, delay-free methods and clocking input samples — §§1800-2009 25.5, 25.7, 25.9, 25.10 **[SV-2009]**. Timing-bearing tasks, event-formal dispatch, dynamic output/inout clocking dispatch, associative/nested interface arrays and broader polymorphic/capture combinations remain restricted (sim_virtual_interfaces.rs).
- 🟨 **Packages** — §1800-2009 26 **[SV-2005]** frontend value/type parameter specialization, typedef/enum metadata, constant `$bits`/range queries, finite folded real/string constants, shared runtime variables, dependent initialization, static subprogram state, `$unit`, and resolved qualified/imported/re-exported names have source paths. The SYN-016 matrix covers wildcard package exports, declaration-scope `let`, terminating constant functions, named generate scopes, parameterized interfaces and separate/merged compilation units in both optimizer modes (sim_syn016_elaboration.rs). The common pre-process initializer path admits zero-time calls for supported scalar/fixed integral values, not every native/aggregate/package combination. Unsupported layouts, unresolved namespace/call environments, and post-2009 forms remain restricted (sim_function.rs, package_runtime_state.sv, sim_rtl_completion.rs).
- 🟦 **`.name` / `.*` connection shorthands** — §1800-2009 23.3.2.3–4 **[SV-2005]** expanded by the frontend and preserved through port-link lowering (sim_hier.rs)
- 🟨 **ref ports / default port values** — §1800-2009 23.2.2.2/23.2.2.4 **[SV-2005]** matching packed references, fixed arrays and collected recursive aggregate/string/chandle reference leaves share storage. Input expressions/defaults use value behavior. Collected string, aggregate and resizable-container input/output links have typed copy/dependency paths; chandle value links and uncollected or incompatible layouts remain rejected. These module-port paths do not imply general aggregate subroutine arguments (sim_partial_features/ports.rs).
- 🟦 **extern / nested modules** — §1800-2009 23.4–23.5 **[SV-2005]** parameterized extern declarations and matching bodies, plus nested module definitions with enclosing parameter references, are preserved through owned capture and generated simulation. Separate and merged compilation-unit policies, independently scoped same-named nested definitions, missing bodies and signature mismatches are covered by [`sim_syn018_module_declarations.rs`](../tests/sim_syn018_module_declarations.rs). Configuration/library selection is bounded by the SYN-032 flow below; later-edition module features remain outside this scope.

## 4. Scheduling & processes

Verilog era:

- 🟦 **Stratified regions** active / inactive(`#0`) / NBA / re-eval loop until quiescent — §1364-2001 5.3–5.4 **[1995]** pinned by region_conformance.rs
- 🟦 **initial** runs once then exits — §1364-2001 9.9.1 **[1995]**
- 🟦 **always** loops forever — §1364-2001 9.9.2 **[1995]**
- 🟦 **Continuous assigns as comb processes** re-evaluated on the lowered RHS dependency set, including admitted fixed-array/container markers — §1364-2001 6.1 **[1995]**; scheduling is implemented by the project's execution IR and C runtime, not by an external simulator.
- 🟦 **fork/join parallel blocks** — §1364-2001 9.8.2 **[1995]** (sim_fork.rs)
- 🟦 **Named forks/blocks** `fork : name … join` — §1364-2001 9.8.3 **[1995]**
- 🟦 **wait(cond)** level-sensitive, re-evaluated on typed packed or real condition reads — §1364-2001 9.7.6 **[1995]** false/unknown constants stay suspended while other processes and timed events continue (sim_wait.rs, sim_partial_features.rs)
- 🟦 **Wait-free ordinary `always` repeats** — §1364-2001 9.9.2 **[1995]** a generated loop remains a procedure even without timing control; cooperative zero-time budgets report nonconvergence with the process source location instead of silently accepting combinational/run-once behavior
- 🟨 **disable `<label>` / task** — §1364-2001 ch11 **[1995]** uses resolved declaration/instance activation identity. Self-disable is not an early return: it cancels all active invocations of the target. Lexical exits clean up activation records. P18 remains partial; this source review does not establish complete cross-process cancellation support.

SystemVerilog era:

- 🟨 **always_comb** time-zero execution and typed implicit sensitivity — §1800-2009 9.2.2.2 **[SV-2005]** retains called-function reads and excludes written variables, including admitted fixed-array elements and aggregate projections. Blocking timing controls and forks are rejected; nonblocking delayed assignments are not rejected merely for their delay. P12 remains partial outside the admitted storage paths (sim_process_semantics.rs, sim_syn014_process_contexts.rs).
- 🟨 **always_ff** typed process kind and semantic contracts — §1800-2009 9.2.2.4 **[SV-2005]** requires one event control and prohibits blocking timing controls, forks, and extra writers; blocking data assignments and delayed nonblocking data assignments are legal. Aggregate data changes do not wake the process outside its declared event control. Rejections retain source locations (sim_process_semantics.rs, sim_syn014_process_contexts.rs).
- 🟨 **always_latch** time-zero execution and typed implicit sensitivity — §1800-2009 9.2.2.3 **[SV-2005]** uses the same timing restrictions as always_comb and retains called-function diagnostics. Aggregate reads and fixed-array leaves are covered by the bounded SYN-014 path; P12 remains partial outside the admitted storage paths (sim_process_semantics.rs, sim_syn014_process_contexts.rs).
- 🟦 **join_any/join_none** — §1800-2009 9.3.2 **[SV-2005]** (sim_fork.rs)
- 🟦 **wait fork** — §1800-2009 9.6.1 **[SV-2005]**
- 🟦 **disable fork** — §1800-2009 9.6.3 **[SV-2005]**
- 🟦 **final blocks at end of simulation** — §1800-2009 9.2.3 **[SV-2005]** (sim_final.rs) are captured as typed final processes independent of file extension and run ONCE after the scheduler exits ($finish, deadlock or no future events); they see values committed before simulation ended and `$time` reports the end-of-run time. Nonblocking assignments, task calls, deferred `$strobe`/`$monitor`, and timing controls (`#`/`@`/`wait`/fork) are clean codegen rejects because finals permit function statements only and no scheduled events execute afterward. `$finish` inside a final terminates that final immediately and skips all remaining finals
- 🟨 **Observe/reactive/preponed regions** — §1800-2009 4.4 **[SV-2005]** typed runtime regions, explicit sensitivity-wait migration, fixed-point re-entry and immutable observation views are present; deferred `assert`/`assume`/`cover #0` actions mature in Reactive from issue-time samples, bounded concurrent assertions sample packed predicates and sequence atoms in Preponed, compose bounded one-cycle property booleans and named instances, resolve attempts in Observed and dispatch actions in Reactive (`sim_concurrent_assertions.rs`), clocking input sampling uses the preponed/observed regions, program processes use the Reactive/Re-Inactive/Re-NBA set, and the bounded VPI bridge has startup/start/end hooks. Unsupported property forms and the broader VPI region API remain unsupported
- 🟨 **Clocking input events and sampling** — §§1800-2009 14.3, 14.4, 14.5, 14.6, 14.7, 14.8, 14.9, 14.10, 14.12, 14.13, 14.14, 14.15 **[SV-2005]** owned clocking declarations preserve input directions, aliases, default/global modifiers, concrete interface members and event controls; `#1step`, `#0` and constant positive input skews retain preponed, observed and history samples (sim_partial_features.rs).
- 🟨 **Clocking output/inout drives and cycle delays** — §§1800-2009 14.11, 14.16 **[SV-2005]** constant output skews capture values and schedule Re-NBA drives (off-event drives wait for the next matching clocking event), inout clocking members drive their resolved net slots while retaining sampled reads, and `##N` repeats the resolved default clocking event (including irregular periods). Selected packed targets, legal assertion sequence/property clock-flow across exact `##0`/`##1` boundaries, default-clock inheritance, and optimizer-parity behavior have regression sources in `sim_partial_features.rs` and `sim_concurrent_assertions.rs`; dynamic output skews, unsupported cross-clock delays/combinators and frontend-rejected concatenated clockvar lvalues remain outside this bounded support.
- 🟨 **Program blocks and `$exit`** — §1800-2009 24.3, 24.3.1 and 24.7 **[SV-2005]** program initial processes launch in Reactive, and #0/NBA remain in the reactive region set. Initials and descendants retain an elaborated program-instance origin: `$exit` cancels only that origin; calls outside a program-initial origin are ignored. Last-initial completion cancels that program's detached descendants; all program initials ending is an immediate implicit finish boundary. Source corrections and regression sources are present (`sim_program.rs`, `sim_review_batch2.rs`); this review did not execute them or close the whole phase.
- 🟨 **Fine-grain process control** `process::self()`, `status()`, `kill()`, `suspend()`, `resume()`, and `await()` — §1800-2009 9.7 **[SV-2005]** stable reference-counted handles preserve waiting conditions, recursively clean descendants, and retain terminal status (sim_process_control.rs). Process formals, process arrays, and the broader class API remain outside the bounded subset.
- 🟨 **Semaphores** `new`, `get`, `put`, and `try_get` — §1800-2009 15.3 **[SV-2005]** runtime-owned key counts, zero-key operations, specified FIFO blocking order, cancellation-safe waiter cleanup, and automatic task-handle arguments have source paths and regression cases for both optimizer modes (sim_semaphore.rs). Semaphore arrays, process-handle formals/arrays, and broader synchronization remain outside the bounded subset.
- 🟨 **Mailboxes** `new`, `num`, `put`, `get`, `peek`, `try_put`, `try_get`, and `try_peek` — §1800-2009 15.4 **[SV-2005]** typed/untyped bounded/unbounded FIFO paths retain scalar packed, real/shortreal, string and admitted handle messages. Declared nominal enum/class/handle identities are now carried through typed message/target records. Empty, mismatch and successful delivery remain distinct; mismatches retain the message and destination. Consuming delivery unlinks before callbacks, peek is nonconsuming, and owned delivery snapshots survive reentrancy/cancellation. Arrays, general aggregate payloads and arbitrary automatic native captures remain outside the bounded implementation (sim_mailboxes.rs, sim_review_batch2.rs, sim_review_batch4.rs).

## 5. Procedural statements

Verilog era:

- 🟦 **Blocking assignment** whole/bit/part/indexed-part/array-element LHS — §1364-2001 9.2.1 **[1995]**
- 🟦 **Nonblocking assignment** committed in NBA region — §1364-2001 9.2.2 **[1995]** selected updates capture values/indices when issued and merge only selected bits at commit
- 🟦 **if/if-else-if** — §1364-2001 9.4 **[1995]**
- 🟦 **case** — §1364-2001 9.5 **[1995]**
- 🟦 **casez/casex wildcards** LRM 12.5.1 matching — §1364-2001 9.5.1 **[1995]** (sim_casez.rs)
- 🟦 **Constant-expression case** `case(1)` priority style — §1364-2001 9.5.2 **[1995]**
- 🟦 **Sequential/named blocks** `begin : name … end` — §1364-2001 9.8.1/9.8.3 **[1995]**
- 🟦 **for** — §1364-2001 9.6 **[1995]**
- 🟦 **while** — §1364-2001 9.6 **[1995]**
- 🟦 **repeat** non-constant count runtime-evaluated — §1364-2001 9.6 **[1995]**
- 🟦 **forever** — §1364-2001 9.6 **[1995]**
- 🟨 **Event control @** — §1364-2001 9.7.2/9.7.4 **[1995]** packed/scalar-real expression changes, LSB edges, trigger-time `iff`, fixed-array/container dependencies, numeric activation captures and atomic mixed named-event lists have source paths. Read-only callbacks admit bounded numeric helpers with value/const-ref formals, private locals, structured loops, nested eligible calls, fixed-array integral reductions and stateless static formals. Persistent locals or stateful reads, visible writes, native/DPI dispatch, suspension, unsupported qualifiers/captures and real edge descriptors remain rejected; unique/priority diagnostics are preserved, and arbitrary side-effect-free source is not automatically eligible (sim_events.rs, sim_partial_features/evaluated_events.rs, sim_rtl_completion.rs).
- 🟨 **force** `force lhs = expr;` live whole packed/real variables plus constant-selected, concatenated and resolved wire/wired/inout targets, with waiter wakeups, strength-preserving release and explicit RHS dependencies — §1364-2001 9.3.2 **[1995]** (sim_force.rs, sim_net_resolution.rs); variable selects, arrays, automatic/local references and other unsupported net contexts are rejected
- 🟦 **release** — §1364-2001 9.3.2 **[1995]** removes the matching force driver; variables retain the forced value unless an active PCA resumes, while nets immediately resolve their current driver slots
- 🟦 **force/release driver re-evaluation** — §1364-2001 9.3.2 **[1995]** force RHS changes propagate while active; procedural and structural writes remain underneath according to object kind, including four-state and two-state conversion
- 🟨 **Condition event expressions** `@(a && b)` — §1364-2001 9.7.2 **[1995]** evaluated operands and admitted called-function dependencies drive reevaluation rather than the controlled body's unrelated reads. The current evaluator/capture restrictions in the migration table apply (sim_partial_features/evaluated_events.rs).
- 🟨 **Intra-assignment timing** `a = #5 b;` / `a <= #5 b;` — §1364-2001 9.7.7 **[1995]** packed/real/shortreal RHS values are captured immediately; blocking writes suspend, while constant/runtime-delay NBAs capture destinations and continue immediately. Selected NBAs preserve disjoint updates at commit. Explicit event and repeated-event controls capture the RHS at issue time, use update-time blocking selectors versus issue-time NBA selectors, preserve suspension/order, and normalize zero/X/Z/negative repeat counts (sim_delay.rs, sim_partial_features.rs)
- 🟨 **Standalone repeat event control** `repeat (n) @ev;` — §1364-2001 9.7.7 **[1995]** has a source path as an ordinary repeat-loop whose body waits on the event: the vendored parser, wrapper, owned statement import, loop lowering and structured emitter preserve that composition. The packed count is evaluated once and retained across waits. This is distinct from an explicit repeated-timing node, which the standalone timing importer does not admit, and from intra-assignment repetition. Real counts and unsupported event/capture forms remain rejected; no dedicated post-change HDL execution is recorded.
- 🟨 **Named events** `event ev; -> ev; ->> ev; @ ev;` — §1364-1995 §9.7.3 / §1800-2009 15.5.1 **[1995/SV-2005]** source paths preserve scalar/fixed-array, hierarchy, null/reassignment and inline task-formal object identity. Immediate triggers, untimed/delay-controlled NBA triggers, and event-controlled/repeated-event `->>` are represented; the latter capture target/source identity and register at issue time without suspending the issuer. Same-slot `.triggered` and `wait_order` arms are present. Dynamic/associative/queue event storage, unsupported repeat shapes, event-formal numeric C calls and event-formal virtual-interface dispatch remain restricted (sim_events.rs, sim_partial_features/nonblocking_events.rs).
- 🟦 **Procedural continuous assign/deassign** — §1364-2001 9.3.1 **[1995]** (sim_force.rs, sim_procedural_assign.rs) `assign <reg> = expr;` creates one replaceable target binding with a live RHS evaluator and an immediate drive; whole real/shortreal variables and ordinary packed concatenations are also supported; `deassign` removes each binding while retaining the last driven value; RHS and called-function dependencies propagate while assigned; blocking and non-blocking procedural writes cannot override an active PCA or wake its evaluator; a later textual assign site replaces the earlier binding at runtime, and pre-scanned sites make deassign-before-assign order independent; `force` keeps priority over an active PCA and `release` returns to its latest live RHS. Clean rejects: net targets, selects/part-selects/array elements, hierarchical and streaming targets, and recursive aggregate values.

SystemVerilog era:

- 🟨 **Whole/selected fixed-array procedural assignment** `result = sel ? a : b;` / `result <= sel ? a : b;` — §§1800-2009 7.6, 10.4, 11.4.11 **[SV-2005]** R04 admits bounded fixed integral expressions by owned result shape after the existing copy/slice/concatenation/pattern/cast paths. It captures the full RHS once before stores or NBA issue, preserving overlapping assignments, declared element order and specialized per-cell conversions. R01 supplies aggregate ambiguous-selector merging. The separate continuous-assignment/intra-assignment timing paths are unchanged; illegal automatic/reference-formal NBA destinations remain rejected. The nested-default/NBA corrections pass in the current Linux baseline (sim_array_conditional_assignments.rs, sim_p30_fixed_arrays.rs).
- 🟨 **Assignment-pattern values and positional lvalues** `'{…}`, integer/member keys and `default` — §§1800-2009 10.9.1–10.9.2 **[SV-2005]** existing fixed-array/record paths retain element context, missing/duplicate-key checks and declaration order. The corrections export bound nested defaults without error placeholders and preserve repeated positional edges when elements share one expression node. Bounded packed-vector leaves support positional/integer/default keys using declared `ranges` and checked bounds; typed row defaults and untyped recursive bit defaults remain distinct. Fixed unpacked-array type keys now match owned integral, typedef, row and record descriptors with explicit-index > type-key > default precedence; nested fixed integral arrays/records, reversed/negative bounds and declaration/local/argument/return/NBA contexts are covered by `sim_syn001_type_keys.rs` in both optimizer modes. Replicated assignment-pattern values now expand their constant count through the owned pipeline for fixed unpacked arrays and fixed integral aggregates, preserving syntactic order and repeated positions across nested rows, packed multidimensional values, typedef-qualified values, reversed bounds, declaration, argument/return, and NBA contexts; the public regression covers both optimizer modes (`sim_p30_fixed_arrays.rs`). Positional assignment-pattern lvalues now use typed fixed-array scatter plans for plain and typedef-qualified targets, nested positions, selected fixed-array cells, reversed source bounds, overlap-safe one-time RHS capture, pattern RHSs, and blocking/legal persistent-storage NBA contexts; `sim_syn003_pattern_lvalues.rs` covers both optimizer modes. Frontend-invalid zero counts and shape mismatches remain rejected, source side-effect evaluation count is not promised for replication, keyed/default/replicated lvalue forms, width/type mismatches, constant destinations, automatic/reference-formal NBA targets, packed-vector type-key matching, and general native/resizable aggregate shapes remain restricted.
- 🟦 **Sequential Boolean predicates** `if (a &&& b)` — §§1800-2009 12.6.2–12.6.3 **[SV-2005]** R06 stores all clauses in source order and keeps branch references separate. Each reached clause is evaluated once; only definite true advances, while false or ambiguous truth stops. Ambiguous statement predicates take else. Numeric truth uses the whole packed value or real nonzero comparison; unique/priority handling remains attached to the statement. This is distinct from ordinary logical `&&` and from timing-check conditions (sim_sequential_predicates.rs).
- 🟨 **Conditional pattern execution and bindings** `matches`, pattern variables — §1800-2009 12.6, 12.6.2-12.6.3 **[SV-2005]** integral constant patterns, `.*` wildcards and `.name` bindings execute in `if`/`?:` predicates with source-ordered `&&&`, defined match Booleans, automatic binding storage, later-clause/true-arm scope and effect/dependency/lint coverage. SYN-023 adds recursive fixed packed/unpacked structure patterns with resolved member checks; tagged patterns, `case ... matches` and pattern-case bodies remain fail-closed (sim_sequential_predicates.rs, sim_semantic.rs).
- 🟦 **return** in functions/tasks — §1800-2009 12.8/13.4.1 **[SV-2005]**
- 🟦 **unique/priority/unique0 if & case** — §1800-2009 12.4.2/12.5.3 **[SV-2005]** runtime no-match and multiple-match diagnostics retain source identity, default/else suppression, and case/casez/casex four-state matching (sim_unique_priority.rs)
- 🟦 **case … inside** wildcard matching — §1800-2009 12.5.4 **[SV-2005]** scalar wildcard items, inclusive ranges, first-match/default behavior, and single selector evaluation (sim_wildcard_eq.rs, optimization on/off)
- 🟦 **do-while** — §1800-2009 12.7.5 **[SV-2005]** post-test execution plus break/continue semantics (sim_disable.rs)
- 🟨 **foreach** — §1800-2009 12.7.3 **[SV-2005]** R02 preserves the iterated expression type's complete fixed unpacked/packed dimension sequence, including declared ascending/descending and negative bounds, implicit integral vectors, explicit singleton dimensions, omitted slots and shorter iterator lists. Formals retain their own bounds rather than the actual storage's bounds. Source-level break/continue, endpoint-before-increment guards and automatic read-only lexical iterators remain. Dynamic arrays/queues and integral/string-keyed associative traversal use their separate existing paths; nested resizable elements and unsupported string-iterator fork captures remain rejected. Writes to read-only foreach iterators are not missing legal-language coverage (sim_loops.rs: `foreach_mixed_*`, slang_semantics/foreach.rs).
- 🟦 **break/continue** — §1800-2009 12.8 **[SV-2005]** (sim_disable.rs) for/while/repeat/forever; continue lands on the increment (for) or back-edge condition test, break exits the innermost loop; nesting pinned
- 🟨 **Inline loop-var declarations** `for (int i…)` — §1800-2009 12.7.1 **[SV-2005]** packed and real loop variables with lexical shadowing, break/continue, and legal synchronous/detached fork value capture (sim_loops.rs, sim_partial_features.rs); nonblocking writes and fork captures of string loop locals remain rejected

## 6. Timing controls

Verilog era:

- 🟦 **#delay integer literal**, timescale-scaled — §1364-2001 9.7.1 **[1995]** (sim_timescale.rs)
- 🟦 **@\* / @(\*) implicit sensitivity** from call-site body read set — §1364-2001 9.7.5 **[2001]** distinct from `always_comb` called-function sensitivity (sim_process_semantics.rs)
- 🟦 **Comb sensitivity to array elements** — §1364-2001 9.7.5 **[2001]** fixed-array element/contents and resizable-container contents/shape dependencies wake implicit and explicit readers without retaining reallocatable element addresses (sim_array_sensitivity.rs)
- 🟨 **Fractional delays** `#0.5` — §1364-2001 9.7.1 **[1995]** typed integer, real, scientific, and unit-suffixed values in statement and intra-assignment delays round once to the local time precision before conversion to design scheduler ticks; constant fractional continuous/gate delays use the same scaling with inertial scheduling; values below the selected precision round to zero
- 🟨 **Expression/parameter delays** `#(expr)` / `#P`, underscored `#10_000` and unit-suffixed `#5ns` literals — §1364-2001 9.7.1 **[1995]** constant and runtime packed/real expressions, function results and task inputs work in statement and intra-assignment delays. Each delay is evaluated once; real values round to local precision, X/Z means zero, and negative packed values convert to unsigned 64-bit time before checked scaling (sim_delay.rs, sim_time_literals.rs, sim_physical_time.rs, sim_partial_features.rs). Negative/nonfinite real delays and values exceeding 64-bit femtosecond scheduler ticks are rejected; unsupported expression forms remain gaps

SystemVerilog era:

- 🟨 **Cycle delays** `##N` — §1800-2009 14.11 **[SV-2005]** integral procedural counts use the resolved default clocking event; captured output/cycle-drive paths are present. Sequence delays/repetition and legal `##0`/`##1` multiclock boundaries have a separate bounded assertion implementation, not a blanket rejection. Missing default events, dynamic skews and unsupported cross-clock delays/combinators remain rejected (sim_partial_features/clocking.rs, sim_concurrent_assertions.rs).

## 7. Expressions & operators

Verilog era:

- 🟨 **Arithmetic** `+ - * / %` — §1364-2001 4.1.5 **[1995]** add/subtract/multiply/division/modulo use each operand/result's actual allocated limbs; the generated backend rejects widths at its exclusive `1 << 20` capacity, while runtime constructors remain defensive
- 🟨 **Power** `**` — §1364-2001 4.1.5 **[2001]** exact-width packed operands have source paths; backend capacity remains exclusive at `1 << 20`
- 🟦 **Bitwise** `& | ^ ~ ^~` — §1364-2001 4.1.10 **[1995]**
- 🟦 **Logical** `&& || ! -> <->` — §1364-2001 4.1.9 / §1800-2009 11.4.7 **[1995/SV-2009]** ordinary expression implication short-circuits a known-false antecedent; equivalence evaluates both operands and both preserve four-state X/Z truth (sim_logical_ops.rs). SVA property implication `|->`/`|=>` remains in the assertion boundary.
- 🟦 **Reductions** `& ~& | ~| ^ ~^` — §1364-2001 4.1.11 **[1995]**
- 🟦 **Shifts** `<< >>` — §1364-2001 4.1.12 **[1995]**
- 🟦 **Arithmetic shifts** `<<< >>>` sign-fill — §1364-2001 4.1.12 **[2001]**
- 🟦 **Relational** `< <= > >=` — §1364-2001 4.1.7 **[1995]**
- 🟨 **Equality** `== != === !==` — §1364-2001 4.1.8 **[1995]** X/Z compare literally with `===`/`!==`; logical equality preserves known-mismatch dominance over unrelated unknown bits (the broader formal matrix remains a partial claim)
- 🟨 **Conditional** `?:` — §1364-2001 4.1.13 Table 28 / §1800-2009 11.4.11 Table 11-20 **[2001/SV-2009]** packed known selectors choose one arm and ambiguous selectors use the standards' per-bit table: equal known `0`/`1` bits survive, while any branch bit involving X or Z, including Z/Z, becomes X. R06 also corrects real-valued alternatives to evaluate both and return zero for an ambiguous selector. Array-valued conditionals have a separate SystemVerilog row (sim_operator_semantics.rs, sim_conditional_policy.rs, sim_sequential_predicates.rs).
- 🟦 **Concatenation** `{}` reordered-concat respected — §1364-2001 4.1.14 **[1995]**
- 🟦 **Replication** `{n{}}` — §1364-2001 4.1.14 **[1995]**
- 🟦 **Bit-select/part-select operands** — §1364-2001 4.2.1 **[1995]**
- 🟦 **Indexed part-select** `[+:w]` / `[-:w]` — §1364-2001 4.2.1 **[2001]**
- 🟦 **Array addressing** `mem[i][j]` + element selects — §1364-2001 4.2.2 **[1995]**
- 🟦 **Strings as operands** reg vectors holding 8-bit ASCII — §1364-2001 4.2.3 **[1995]** packed literal assignment, comparison, concatenation, escapes, padding/truncation, and packed-parameter declaration initializers (sim_packed_strings.rs, optimization on/off); SystemVerilog `string` storage is covered separately by the bounded partial row in §2
- 🟦 **$signed/$unsigned** — §1364-2001 4.5 **[2001]**
- 🟦 **Signedness/self-determined width rules** mirrored by runtime — §1364-2001 4.4–4.5 **[1995]** pinned by property_elab.rs
- 🟦 **X/Z expression semantics** Z=X except identity/copy ops — §1364-2001 3.1 **[1995]** proptests + C vector table
- 🟨 **Operators on real operands** `+ - * / %`, relational/logical/`?:`, casts and ordinary `case` equality — §1364-2001 4.1.1 **[1995]** within real subset; bitwise/reduction/shift/concat/case-eq and real selects are rejected

SystemVerilog era:

- 🟨 **Fixed-array and direct unpacked-structure conditional values** `sel ? a : b` — §1800-2009 7.2, 11.4.11 **[SV-2005]** R01's `ArrayMux` retains immediate unpacked element boundaries, while SYN-004's `StructMux` retains direct unpacked-structure member boundaries. An ambiguous selector preserves known-equal elements or members and defaults each differing boundary from its uninitialized value; explicit declaration member initializers are not fallback values. Packed vectors keep per-bit merging. Reached alternatives are captured once; fixed-value subroutine contexts and R04 module-procedural assignments use these paths. General dynamic/native aggregate merging and payloads beyond the backend limit remain unsupported (sim_rtl_completion.rs: `array_conditional_*`, `unpacked_structure_conditionals_*`, sim_array_conditional_assignments.rs).
- 🟨 **Fixed-array reduction methods** `.sum()`, `.product()`, `.and()`, `.or()`, `.xor()` — §1800-2009 7.12.3/7.12.4 **[SV-2005]** R03 supports optional parentheses, named iterators and `with` expressions, nested fixed rows and admitted fixed-record maps. Accumulator/result width and signedness come from the element or mapped expression, not a wider destination; first-element seeding preserves singleton X/Z. Lexical maps can read enclosing automatic locals/formals and distinguish nested iterator identities. Whole rank-one model arrays are read cell-by-cell within the SYN-036 65,536-cell storage ceiling; other receivers capture one bounded packed payload. Integral maps may query the live fixed unpacked iterator dimension with `index()`, constant `index(1)`, or a checked dynamic integral dimension; declared negative/ascending/descending bounds are retained. Packed item dimensions, higher/unvisited dimensions, fixed-array locator/ordering methods, native/string/real/resizable-element maps and over-limit value receivers remain outside the path (sim_fixed_array_reductions.rs, [SYN-036](../tests/sim_syn036_capacity.rs)).
- 🟨 **Fixed-array reverse method** `.reverse()` — §1800-2009 7.12.2 **[SV-2009]** SYN-027 reverses immediate fixed integral elements in declaration order, retaining ascending, descending and negative unpacked bounds. The lowering captures every source element before publishing normal blocking writes, so selected rows, packed-record elements and overlapping storage are safe; automatic locals/formals use their owned packed payload. `with` clauses, const-ref receivers, native/real/string elements and unsupported payload representations remain rejected (sim_data_types_completion.rs, methods.rs).
- 🟨 **Fixed-array sort methods** `.sort()` / `.rsort()` — §1800-2009 7.12.2 **[SV-2009]** SYN-028 emits a checked finite comparison schedule for immediate fixed integral elements and integral `with` maps. Signed, unsigned, enum and packed-record keys retain declaration-order bounds; source elements are captured before each overlapping swap, and automatic locals/formals use owned packed payloads. Repeated-key stability is unspecified; shuffle, locator result queues, native/real/string elements, incompatible maps and const-ref receivers remain rejected (sim_data_types_completion.rs, methods.rs).
- 🟨 **Sequential Boolean predicates in conditional values** `a &&& b ? t : f` — §§1800-2009 11.4.11, 12.6.3 **[SV-2005]** R06 uses the same ordered truth sequence as `if`; a later clause is not evaluated after false or ambiguous truth. An ambiguous result evaluates both arms and uses the existing packed, R01 aggregate or real-zero result rule. `Predicate` IR, folding, effects and dependency collection retain clause order; primitive integral `matches` clauses and SYN-023 recursive fixed structure clauses use the same sequence, while tagged/case patterns remain rejected (sim_sequential_predicates.rs).
- 🟨 **Static casts** `int'(e)`, `signed'()`, `unsigned'()`, size casts `n'(e)` — §1800-2009 6.24.1 **[SV-2005]** Slang retains explicit/implicit conversion identity and resolved target width, signedness, and state domain. Scalar/vector typed and numeric size casts have source paths through declaration, runtime-expression, and function contexts; unsupported aggregate/net paths remain outside this claim (sim_data_types.rs, sim_data_type_edges.rs, sim_data_types_next.rs)
- 🟨 **Increment/decrement** `++ --` — §1800-2009 11.4.2 **[SV-2005]** statement-position pre/post forms on admitted whole, packed-select/member and fixed-array-element lvalues use the canonical one-store mutation path; expression-valued forms cover packed selects/members, fixed-array elements and real targets. Non-lvalues and unsupported aggregate/object targets remain rejected (sim_operator_semantics.rs, sim_expression_mutations.rs, sim_feature_completion_g1.rs)
- 🟨 **Assignment operators** `+= -= *= /= %= &= |= ^= <<= >>= <<<= >>>=` — §1800-2009 11.4.1 **[SV-2005]** whole-variable and admitted selected/array statement forms use the canonical one-store mutation path; expression-valued forms cover packed selects/members, fixed-array elements and real targets. Unsupported aggregate/object targets remain rejected (sim_operator_semantics.rs, sim_expression_mutations.rs, sim_feature_completion_g1.rs)
- 🟦 **Wildcard equality** `==? !=?` — §1800-2009 11.4.6 **[SV-2005]** RHS X/Z bits are wildcards; remaining LHS unknown bits yield X unless a known mismatch decides the result. Common-width/signed extension and exact-width packed operands have regression sources for optimization on/off (sim_wildcard_eq.rs).
- 🟨 **Set membership** `inside {…}` — §1800-2009 11.4.13 **[SV-2005]** scalar/range/wildcard matching and admitted real/string contexts have typed lowering. Fixed unpacked-array RHS values from function returns, conditionals, nested arrays, selected rows and fixed casts are captured once and projected to packed elements with optimizer parity; storage arrays and packed dynamic/queue/associative containers retain their existing owned traversal. Packed vectors remain one set item, while unpacked structures are frontend-illegal set items. General recursive/native-object membership, resizable value RHS forms and over-limit fixed payloads remain restricted (sim_data_types_next.rs, membership.rs, owned/control.rs).
- 🟨 **Streaming operators** `{<<{}}`, `{>>{}}` — §1800-2009 11.4.14 **[SV-2005]** source paths cover packed and bounded fixed/resizable packed-element packing/unpacking, fixed nested aggregate casts, selected rows/members, `ref`/`const ref` projections, runtime fixed-array and container selectors, non-divisible slices and one RHS snapshot followed by ordered destination publication. Mixed assignments permit at most one resizable destination. Dynamic/queue notifications occur after intermediate cleanup. Associative destinations, native strings, recursive object streams and unsupported general reference combinations remain restricted (sim_data_types_next.rs, owned/streaming.rs).
- 🟨 **let expressions** — §1800-2009 11.13 **[SV-2009]** frontend-expanded numeric bodies with free names bound in declaration scope reach the ordinary owned expression/lowering paths. Declaration-scope shadowing and recursive-let rejection have public-CLI regression sources. This is not evidence for arbitrary assertion/native/aggregate let bodies; their expanded expressions must satisfy the existing consumer restrictions (sim_constant_eval.rs).

## 8. Continuous assignments & structural

Verilog era:

- 🟦 **Continuous assignment** `assign lhs = rhs;` — §1364-2001 6.1.2 **[1995]** comb process on RHS read set
- 🟦 **Multiple/comma-form continuous assigns** — §1364-2001 6.1.2 **[1995]**
- 🟨 **Whole and selected fixed-array continuous assignments** `assign y = rhs;` — §§1800-2009 6.6–6.7, 7.6, 10.3; §1800-2005 6.5 **[SV-2005]** whole and selected fixed unpacked array RHS values, conditionals, assignment patterns and typed function returns lower through owned array cells. Ordinary net arrays retain per-cell/per-bit source slots and four-state resolution; variable storage accepts one overlapping continuous writer while disjoint selected cells remain legal. RHS values are captured once per activation, with dependency markers and optimizer parity. Dynamic indices, delayed net-array declarations and unsupported aggregate shapes remain restricted (sim_rtl_completion.rs: `syn_006_array_continuous*`).
- 🟨 **Multiple drivers on one net** — §1364-2001 6.1 **[1995]** ordinary packed, bounded fixed wired arrays and admitted collapsed inout groups retain separate continuous/gate/port driver contributions. R05 selects effective resolver/default/delay for dissimilar admitted endpoints; selected/concatenated actuals and fixed net-array-cell links do not require one high-side declaration. Unsupported layouts, vector-strength legality restrictions and charge-storage contexts remain separate boundaries (sim_net_resolution.rs, sim_inout.rs, sim_port_net_types.rs).
- 🟨 **Dissimilar net types across inout ports** — §1364-2001 12.3.10 / §1800-2009 23.3.3.7 **[1995]** R05 retains internal/external roles and applies the supported part of the port table for wire/tri, wand/triand, wor/trior, tri0/tri1 and supply0/supply1. The winning declaration supplies the resolver, pull/supply default and propagation delay, including an undelayed winner. Source-located warning pairs are deduplicated. Parent connections precede descendants; same-depth warning-only conflicts follow stable owned design order rather than hash/union-root order. The 81-entry table unit test does not enable rejected uwire inouts or trireg, new whole-array port shapes, or net-array declaration-delay capture (sim_port_net_types.rs).
- 🟨 **Delay on continuous assign** `assign #d lhs = rhs;` — §1364-2001 6.1.3 **[1995]** whole packed, constant-selected and fixed-unpacked-array element drivers capture values and schedule Active-region inertial updates. Changed pending values cancel; unchanged results keep their deadline; return to the current driver value cancels without replacement. Array elements keep independent handles. Separately, admitted net-declaration propagation delays are captured from resolved members, stored on the net group and applied after resolution, with alias publication at commit. R05 selects the winning port declaration's propagation delay (including no delay) rather than requiring all connected declarations to agree. Conflicting selected true-alias member delays, unsupported aggregate/dynamic resolved-net targets and unrepresented terminal forms remain restricted (sim_delay.rs, sim_net_decl.rs, sim_review_batch2.rs).
- 🟨 **Strength on continuous assign/gates** — §1364-2001 6.1.4/7.1.2 **[1995]** explicit scalar strengths flow through continuous assignments, logic/pull gates, wired nets and collapsed inout groups in both optimizer modes; vector continuous strengths remain prohibited by §10.3.4, and switch-level/resistive/trireg behavior remains unsupported
- 🟦 **Logic gates** `and nand or nor xor xnor buf not` — §1364-2001 7.2–7.3 **[1995]** typed input expressions and output lvalues lower to dependency-driven processes; multi-output `buf`/`not` gets an independent driver per output. Inputs are resized to each output width in this path; equal terminal widths and a 64-terminal cap are not current blanket backend restrictions. Built-in primitive-array elements are admitted (sim_gates.rs).
- 🟦 **Tri-state buffers** `bufif0 bufif1 notif0 notif1` — §1364-2001 7.4 Table 7-5 **[1995]** (sim_gates.rs) lowered to `sv4_mux(en, data|data, Z)` / `sv4_mux(en, Z, ~(data|data))` — the passing arm is z→x-normalized with `data|data` (per-bit), so an ENABLED gate turns a data-Z into X like buf/not while known bits pass unchanged; a DISABLED gate drives Z; unknown enable yields all-X unless both branches match
- ❌ **MOS/CMOS switches** `nmos pmos cmos rnmos rpmos rcmos` — §1364-2001 7.5–7.7 **[1995]** rejected with a clear message ("switch/transistor primitive … not supported")
- ❌ **Bidirectional switches** `tran tranif0 tranif1 rtran*` — §1364-2001 7.6 **[1995]** rejected with a clear message
- 🟦 **pullup/pulldown** — §1364-2001 7.8 **[1995]** (sim_gates.rs) constant 1/0 driver process over the terminal width (RunOnce)
- 🟨 **Strength modeling/resolution tables** — §1364-2001 7.9–7.13 **[1995]** scalar drive endpoints, X uncertainty, wired-net tie rules, pull/supply defaults, gate outputs and collapsed output-port links are modeled; switch-level/resistive propagation, charge strength and aggregate contexts remain unsupported
- 🟨 **Gate delays** `and #2 g(…)` / parameterized `#D` — §1364-2001 7.14 **[1995]** supported whole-signal and constant-selected gate terminals use captured active-region inertial updates with single, rise/fall, and rise/fall/turn-off delays, X initialization and short-pulse cancellation. Enable-gate X/Z values, vector transition selection and unchanged-result deadlines have regression sources (`sim_gates.rs`, `sim_partial_features.rs`, both optimizer modes); unsupported primitive kinds and other unrepresented terminal forms remain restricted
- 🟦 **Gate instance arrays** `and g[3:0] (…)` — §1364-2001 7.1 **[1995]** built-in gate-array elements and scalar combinational UDP-array elements are imported with typed terminals and admitted by semantic/lowering paths. `sim_gates_gate_array_distributes_bits` and `udp_comb.sv` cover bit distribution and independent UDP drivers; switch/sequential arrays remain rejected by primitive kind.
- 🟦 **Combinational UDPs** definition/table/instances — §1364-2001 8.1–8.2/8.6 **[1995]** and §1800-2009 29.3–29.4/29.8 **[SV-2009]** scalar tables are imported into owned rows, validate overlap, match runtime Z as X, return X for unmatched rows, and lower through structural drivers; repeated instances and primitive instance arrays retain independent drivers (`sim_udp.rs`, `udp_comb.sv`, both optimizer modes)
- ❌ **Sequential UDPs** level/edge-sensitive — §1364-2001 8.3–8.5 **[1995]** same reject as combinational UDPs
- 🟨 **Gate terminal connections** — §1364-2001 7.1 **[1995]** typed input expressions, constants, selected and hierarchical references, legal selected outputs, mixed-width normalization and multi-output `buf`/`not` have source paths. Output lvalues still require a supported packed target and structural driver mapping; real terminals, invalid expressions, unsupported primitives and resolved-net combinations reject. The former whole-signal-only, equal-width and 64-terminal statements are obsolete (sim_gates.rs, sim_net_resolution.rs).
- 🟨 **Structural bind** — §1800-2009 23.11 **[SV-2005]** finite module-type and selected-instance binds, plus interface-to-interface binds, are elaborated as ordinary owned hierarchy with parameter values, target-local port resolution, distinct instance paths and optimizer-parity generated-model traces (`sim_syn033_structural_bind.rs`). Unknown targets and primitive targets remain frontend diagnostics; module injection into an interface is rejected by the SystemVerilog target-kind rule, and checker/program/coverage injection remains outside this selected RTL scope.

(SystemVerilog era: 1800-2009 ch28–29 restore gates/UDPs verbatim — same
statuses as the rows above.)

SystemVerilog addition:

- 🟨 **Net aliases with port connections and static projections** — §1800-2009 10.11; §§6.5–6.7 **[SV-2005]** true aliases are checked for matching declared net types before relaxed inout collapse is applied. SYN-010 covers fully indexed fixed net-array elements, reversed ranges, multidimensional packed projections, force/release visibility, and an admitted array-element inout link with optimizer parity. The expression-only actual correction traverses a concatenation even when the port has no single high-side declaration. Incompatible, duplicate, self, variable, dynamic, forbidden cross-scope, and switch-level forms still reject (`tests/sim_net_resolution.rs`: `static_net_array_and_packed_alias_projections_keep_shared_connectivity`, `static_net_alias_legality_rejects_single_fault_invalid_forms`).

## 9. Functions & tasks

Persistent static function/task-local and static-task-NBA storage paths are
present. Automatic storage is established per activation; static storage is
initialized once rather than on first call. Admitted zero-time user-call
initializers for scalar and fixed integral composite values execute before 2009
processes; the earlier blanket pre-process call restriction is obsolete. This
does not enable timing-bearing calls or arbitrary native/aggregate layouts.
Same-lifetime overrides require an owned capture path; ambiguous/opposite-lifetime
forms remain rejected. Existing regression sources are not current acceptance.

Verilog era:

- 🟨 **Function declaration/return value/call in expressions** — §1364-2001 10.3 **[1995]** typed numeric/native calls, selected actual capture, automatic/static returns, fixed integral array/struct/union values and bounded output/inout storage have source paths. The 2009 pre-process phase admits zero-time calls for supported scalar/fixed composite initializers. Depth 256 reports a diagnostic and returns the type default. Event-formal value calls and unsupported native/resizable-aggregate/evaluator combinations remain guarded (sim_function.rs, sim_dynamic_ownership.rs, sim_rtl_completion.rs).
- 🟨 **Tasks incl. output/inout args** — §1364-2001 10.2 **[1995]** delay-only supported tasks can suspend through native C calls; event-formal/event-controlled and relevant timed-cancellation tasks use call-site expansion. Values and captured selectors survive suspension and cancellation is checked before copy-out. Static formal/local persistence and static NBA paths are represented. Recursive timed tasks, timing-bearing class/interface methods, illegal automatic-subroutine NBAs and native/resizable aggregate arguments outside the fixed-value ABI remain restricted (sim_function.rs, sim_dynamic_ownership.rs).
- 🟦 **automatic reentrant functions/tasks** — §1364-2001 10.2.3/10.3.1 **[2001]** recursion supported
- 🟦 **Constant functions in parameter expressions** — §1364-2001 10.3.5 **[2001]** evaluated by elab Resolver; typed parameters required (historical probe)
- ❌ **Task calls inside function bodies** — §1364-2001 10.3.4 **[1995]** rejected
- 🟨 **fork/join inside function/task bodies** — §1364-2001 9.8.2 / §1800-2009 9.3.2 **[1995/SV-2005]** detached `join_none` branches capture automatic packed locals and formals in owned activation frames; ordinary blocking timing in functions, recursive timed tasks, and richer subroutine activation forms remain rejected
- ❌ **Timing controls in function bodies** `#`/`@`/`wait` inside a function rejected — §1364-2001 10.3.4 **[1995]** direct function timing remains rejected; timing controls in a legal detached `join_none` branch use that branch's retained frame. Nonblocking assignments to automatic storage remain rejected
- ❌ **Recursive delay-bearing tasks** — §1364-2001 10.2.3 **[2001]** rejected
- 🟨 **Cross-instance hierarchical subprogram calls** — §1364-2001 12.4 **[1995]** resolved callee identity selects its owning module/interface/package environment; per-instance and parent/hierarchical dispatch have source paths and regression sources in `sim_function.rs`. The old blanket rejection is obsolete. Unresolved callees and timing/native/aggregate restrictions still apply; broader combinations need acceptance.

SystemVerilog era:

- 🟦 **void functions** `function void f();` — §1800-2009 13.4.1 **[SV-2005]**
- 🟦 **Default argument values** incl. references to earlier formals — §1800-2009 13.5.3 **[SV-2005]**
- 🟦 **Named argument binding** `f(.b(x), .a(y))` order-independent — §1800-2009 13.5.4 **[SV-2005]** (historical probe)
- 🟨 **ref arguments** typed `ref`/`const ref` descriptors alias matching packed variables, fixed integral arrays/structs/unions, legal unpacked members/elements and retained packed queue cells — §1800-2009 13.5.2 **[SV-2005]**. Queue removal/reallocation preserves the original detached cell rather than retargeting a later element; registered reference scopes release on return/cancellation. String/chandle references use typed native-address paths. General native/resizable aggregate references, NBAs through reference formals and selected-reference file destinations remain restricted. Frontend-illegal packed bit/part actuals in the 2009 policy are not promoted to legal HDL by internal descriptor support (sim_reference_args.rs, sim_review_batch4.rs).
- 🟦 **Finite zero-time subroutine closure** — IEEE 1364-2001 §§10.2.2–10.2.3, 10.3.1–10.3.3 and IEEE 1800-2009 §§6.21–6.22, 13.3–13.5. Delay-free calls retain named/dependent defaults, negative fixed-array bounds, fixed aggregate return/copy-out values, nested `const ref` forwarding, local named-block disable cleanup, and per-instance static versus automatic storage. The bounded witness runs under both optimizer modes in Verilog-2001 and SystemVerilog-2009 (`sim_syn013_zero_time_calls.rs`, `syn013_zero_time_calls/zero_time_calls.sv`). Timed recursion, event/process formals, resizable/native aggregate ABIs and illegal reference/NBA forms remain outside this Core closure.

## 10. System tasks & functions relevant to simulation

Verilog era — display family:

- 🟦 **$display** — §1364-2001 17.1.1 **[1995]**
- 🟦 **$write** — §1364-2001 17.1.1 **[1995]** same formatting as `$display`, without an appended newline (sim_monitor.rs)
- 🟦 **Typed format specs** `%d/%h/%x/%b/%o/%c/%u/%z/%v/%t`, `%f/%e/%g`, `%s`, `%m`, and `%l` with the width/precision grammar admitted by Slang; packed X/Z digits, strength text, real/string values, and exact `%%` are preserved across display/write/strobe/monitor and string-producing formatters — §1364-2001 17.1.1.2 / §1800-2009 21.2 **[1995/SV-2005]**. `%p` is limited to scalar packed/string values; aggregate pattern values remain unsupported
- 🟦 **$strobe** observes settled values after active/inactive/NBA iteration, including zero-delay drivers triggered by NBAs — §1364-2001 17.1.2 **[1995]** (sim_monitor.rs, sim_partial_features.rs)
- 🟦 **$monitor/$monitoron/$monitoroff** one active monitor; registration and re-enabling queue one report at the settled observation point, same-slot changes coalesce, and packed, real, and native-string arguments trigger re-evaluation on change — §1364-2001 17.1.3 **[1995]** (sim_monitor.rs, sim_inout.rs). Aggregate/container values remain outside the bounded formatter
- 🟦 **b/o/h console task variants** `$displayb/o/h`, `$writeb/o/h`, `$strobeb/o/h`, `$monitorb/o/h` — §1364-2001 17.1.1–17.1.3 **[1995]** unformatted integral arguments use the variant radix, while explicit format directives retain their own conversion (sim_monitor.rs)

File IO:

- 🟦 **Core file output** `$fopen/$fclose/$fdisplay/$fwrite/$fstrobe/$fmonitor` — §1364-2001 17.2.1–17.2.2 / §1800-2009 21.3.1–21.3.2 **[1995/SV-2005]** portable owned descriptors, standard-stream masks, multichannel fan-out, typed radix formatting, and postponed file monitor/strobe output have source paths and regression cases for both optimizer modes; formatted reads remain separate
- 🟦 **$sformat/$swrite** and radix variants — §1364-2001 17.2.3 **[2001]** use the shared typed formatter, preserve source-order argument evaluation, and write native string or packed string-like destinations with normal width truncation/padding
- 🟨 **Formatted reads** `$fscanf/$sscanf/$fread/$fgets/$fgetc/$ungetc` — §1364-2001 17.2.4 / §1800-2009 21.3.4.1–21.3.4.4 **[2001/SV-2005]** typed input paths retain source/format strings, capture admitted packed/selected/string/real targets and preserve EOF, bytes, X/Z and bounded memory start/count behavior. Selected sub-accesses through a reference formal and general aggregate destinations remain rejected (sim_file_io.rs, runtime_file_io.rs).
- 🟦 **File positioning/status** `$ftell/$fseek/$rewind/$fflush/$ferror/$feof` — §1364-2001 17.2.5–17.2.7 / §1800-2009 21.3.5–21.3.8 **[2001/SV-2005]** seek, rewind, flush, EOF/error reporting, output-string ownership, and invalid/closed descriptor status use checked portable host-I/O paths; formatted reads remain separate

Memory load/store:

- 🟦 **$readmemh/$readmemb** — §1364-2001 17.2.8 / §1800-2009 21.4 **[1995/SV-2005]** whitespace/comment-separated binary and hexadecimal files, four-state digits, and `@` address jumps are implemented for fixed packed memories with one or more unpacked dimensions. A constant higher-dimension selection retains a legal lower-dimensional view; file words walk each remaining dimension low-to-high in row-major order while declaration direction determines flat storage offsets. Verilog-2001 one-dimensional omitted ranges follow declaration direction; SystemVerilog-2009 omitted ranges follow low-to-high address order, including start-only calls; explicit ranges retain their requested direction. `@` jumps select the highest remaining dimension, incomplete final rows leave unwritten subwords unchanged, and invalid selected ranges or jumps terminate the load while retaining prior writes. Enum-typed memory reads reject non-member words; two-state X/Z words are diagnosed and converted to zero. Dynamic/queue/associative, real, non-packed aggregate, range-selected, and fully indexed memories remain rejected (sim_memory_views.rs, sim_memory_editions.rs, sim_memory.rs)

Time:

- 🟦 **$time** module-unit scaled and rounded, `%t` consumes it — §1364-2001 17.7.1 **[1995]** using checked femtosecond-to-unit conversion
- 🟦 **$stime/$realtime** — §1364-2001 17.7.2–17.7.3 **[1995]** `$stime` rounds in the calling module's unit before returning the low 32 bits; `$realtime` preserves fractional module-unit time (sim_timescale.rs, sim_physical_time.rs, sim_partial_features.rs)
- 🟦 **$printtimescale** — §1364-2001 17.3.1 **[1995]**
- 🟦 **$timeformat** — §1364-2001 17.3.2 / §1800-2009 20.4.2 **[1995/SV-2009]** design-wide units, precision, suffix, and minimum-width state; runtime expressions and `%t` conversion across display/write/strobe/monitor are represented (sim_partial_features.rs)

Control / misc:

- 🟦 **$finish** is a nonreturning termination control with default level 1 and explicit level 0/1/2 diagnostics plus exactly-once final-block handoff; the optional target-edition `finish_number` is a constant integral 0, 1, or 2, so runtime, real, unknown, and out-of-range arguments reject before model emission; its argument never becomes the generated model's OS exit status — §1364-2001 17.4.1 / §1800-2009 20.2 **[1995]**
- 🟦 **$stop** yields the issuing coroutine and preserves pending queues, activation frames, output state and simulation time until it resumes; default noninteractive CLI runs use `--stop-policy resume`, while `--stop-policy exit` returns control to the caller without running finals or draining future work. Stop numbers use the same constant integral 0/1/2 diagnostic levels as `$finish` and never become process exit codes — §1364-2001 17.4.2 **[1995]** (sim_partial_features.rs)
- ❌ **PLA modeling tasks** `$async$and$array …` — §1364-2001 17.5 / §1800-2009 20.17 **[1995/SV-2009]**. All sixteen array/plane forms remain excluded by the selected SYN-000 synthesis profile pending a named target and acceptance artifact; the language admission table and generic VPI fallback do not provide PLA synthesis semantics.
- 🟨 **Stochastic tasks** `$q_initialize/$q_add/$q_remove/$q_full/$q_exam` — §1364-2001 17.6 / §1800-2009 20.16 **[1995]**
  ID-indexed FIFO/LIFO queues retain job/information IDs and simulation-precision arrival ticks; full/empty/unknown/duplicate/type/length status codes and the six `$q_exam` statistics use checked integer arithmetic. Whole packed integer output variables are supported with regression sources for both optimizer modes (`sim_stochastic.rs`); selected or real output targets remain rejected.
- 🟦 **$random** — §1364-2001 17.9.1 **[1995]** writable-seed and implicit-stream calls implement the project's Annex N reference algorithm (`sim_random.rs`, `runtime_random.rs`)
- 🟦 **$dist_uniform/$dist_normal/$dist_exponential/$dist_poisson/$dist_chi_square/$dist_t/$dist_erlang** — §1364-2001 17.9.2–17.9.3 **[1995]** checked integer arguments, writable seeds, specified parameter boundaries, and deterministic Annex N vectors (`sim_random.rs`, `runtime_random.rs`)
- 🟦 **Conversion** `$rtoi/$itor` — §1364-2001 17.8 **[1995]** truncation toward zero and signed/unsigned integral-to-real conversion, including implicit numeric argument coercion, typed parameters, and constant declaration initializers (sim_real_conversions.rs, optimization on/off)
- 🟦 **Conversion** `$realtobits/$bitstoreal` — §1364-2001 17.8 **[2001]** IEEE-754 bit reinterpretation; `$bitstoreal` requires 64 bits and maps X/Z positions to zero (sim_real_conversions.rs, optimization on/off)
- 🟦 **Plusargs** `$test$plusargs/$value$plusargs` — §1364-2001 17.10 / §1800-2009 21.6 **[1995]/[SV-2005]**. `llg` passes arguments after `--` to the generated model; test queries use exact leading-`+` prefix matching, and value queries support `%d/%h/%x/%o/%b/%f/%e/%g/%s`, literal `%%`, repeated-argument first-match behavior, wide 4-state destinations, and failure retention (sim_plusargs.rs)

Waveforms:

- 🟨 **$dumpfile/$dumpvars/$dumpon/$dumpoff/$dumplimit** to VCD or FST — §1364-2001 ch18 **[1995]** `$dumpfile` selects the format by `.vcd`/`.fst`; packed signals, unpacked-array elements, real values, X/Z, aliases, hierarchy, exact femtosecond timestamps, dump activation, and size limits are emitted by a separate OS writer thread through a bounded lossless SPSC ring (sim_waveform.rs, sim_physical_time.rs). `$dumpvars` carries owned depth/scope/variable identities into the catalog, filters the catalog before the fixed header, and retains declared array bounds in element names; `$dumpports` remains a separate unsupported extended-VCD family.
- 🟦 **$dumpall/$dumpflush** — §1364-2001 ch18 **[1995]** snapshots and synchronous flush barriers work for both VCD and FST (sim_waveform.rs; runtime self-test)
- ❌ **$dumpports extended VCD** — §1364-2001 18.3 **[2001]** unsupported-task reject

SystemVerilog era:

- 🟦 **$writememh/$writememb** — §1800-2009 21.5 **[SV-2005]** bounded fixed-memory views emit consumable binary/hex words in low-to-high row-major order across remaining dimensions and round-trip four-state values; constant selected rows/slices, non-packed and resizable memories remain checked by the same view contract, and writers are rejected in Verilog-2001 mode (sim_memory_views.rs, sim_memory.rs)
- 🟦 **$clog2/$bits** — §1800-2009 20.8/20.6 **[SV-2005]** (`$clog2` first in [1364-2005])
- 🟦 **Math functions** `$ln $log10 $exp $sqrt $pow $floor $ceil $sin …` — §1800-2009 20.8 **[SV-2005]** all 21 real functions map to table 20-4 C math functions, with runtime arguments, numeric coercion and C domain behavior (sim_partial_features.rs, both optimizer modes)
- 🟦 **Severity tasks** `$fatal/$error/$warning/$info` use the typed display
  formatter, preserve HDL source context and `%m` scope, evaluate each
  message argument once in source order, and write level-specific diagnostics
  to stderr. `$fatal` is an immediate nonreturning termination with
  the constant finish number 0/1/2; nonfatal levels continue execution.
  Severity counters are reported with level-2 finish statistics — §1800-2009
  20.9/20.10 **[SV-2005]** (sim_partial_features/severity.rs)
- 🟦 **$sformatf** — §1800-2009 21.3.3 **[SV-2005]** returns an owned formatted string, supports dynamic and nested format expressions, and evaluates typed arguments once in source order
- 🟦 **Bit-vector helpers** `$onehot/$onehot0/$countones/$isunknown` — §1800-2009 20.6 **[SV-2005]** packed operands use exact-width storage below the supported limit, with X/Z-aware counting, parameters and constant declaration initializers, single argument evaluation, and combinational dependencies; real operands rejected (sim_bit_queries.rs, optimization on/off)
- 🟨 **Sampled-value functions** `$sampled/$rose/$fell/$stable/$changed/$past` and the 2009 global-clock status/history forms — §1800-2009 16.9.3, 16.9.4, 20.13 **[SV-2005]** packed explicit/default edge domains, gated/initial history, Preponed reads, LSB/X/Z edges and bounded sequence `.matched` have source paths. Future global forms, complex clocks, real-valued sampling, sequence `.triggered` and calls outside the bounded read-only evaluator contract remain restricted (sim_sampled_values.rs).
- 🟦 **Shortreal conversion** `$bitstoshortreal/$shortrealtobits` — §1800-2009 20.5 **[SV-2005]** 32-bit IEEE-754 reinterpretation and shortreal rounding; `$bitstoshortreal` requires 32 bits and maps X/Z positions to zero (sim_real_conversions.rs, optimization on/off)
- 🟦 **$system** — §1800-2009 20.18 **[SV-2009]** task and function forms lower
  one optional typed string command and evaluate it exactly once in the
  generated simulator process. Execution is denied unless that child process
  has `LLG_ALLOW_SYSTEM=1` (or `true`/`yes`/`on`); denial diagnoses the call,
  returns a signed 32-bit `-1` status and fails the model without invoking a
  shell. The omitted form calls `system(NULL)`, distinct from an explicit empty
  string; enabled calls return the host C `system()` status unchanged, so shell
  syntax, empty-command behavior, and nonzero-status encoding remain
  platform-specific. Embedded-NUL command strings are rejected before host
  dispatch (sim_partial_features/system.rs).

## 11. Compiler directives affecting simulation

Verilog era:

- 🟦 **`` `default_nettype ``** affects implicit net admission and diagnostics at elaboration — §1364-2001 19.2 **[1995]**; `none` rejection and the reset boundary are exercised in both editions (sim_directive_effects.rs)
- 🟦 **`` `resetall ``** resets directive state used by later elaboration — §1364-2001 19.6 **[1995]**; the restored implicit-net behavior is executed after the frontend snapshot is released (sim_directive_effects.rs)
- 🟦 **`` `celldefine ``/`` `endcelldefine ``** cell tagging — §1364-2001 19.1 **[1995]** consumed by frontend, no sim effect
- 🟨 **`` `unconnected_drive ``/`` `nounconnected_drive ``** — §1364-2001 19.9 **[1995]** omitted scalar and packed input links receive pull0/pull1/Z values after elaboration; strength conflicts and aggregate/resizable formals remain outside the tested boundary (sim_directive_effects.rs)
- 🟨 **`` `line ``** position override — §1364-2001 19.7 **[2001]** mapped `__FILE__`/`__LINE__` values reach execution; owned frontend diagnostics retain physical source ranges (sim_directive_effects.rs)

(`` `timescale `` is covered in §1.)

SystemVerilog era:

- 🟦 **`` `begin_keywords ``/`` `end_keywords ``** keyword-set selection — §1800-2009 22.14 **[SV-2005]** (`begin_keywords` first in [1364-2005]) consumed by the frontend as lexical state only; it does not override the selected global edition
- 🟦 **`` `pragma ``** — §1800-2009 22.11 **[SV-2009]** consumed by frontend
- 🟦 **`` `undefineall ``** — §1800-2009 22.5.3 **[SV-2009]** consumed by frontend
- 🟨 **`` `__FILE__ ``/`` `__LINE__ `` predefined macros** — §1800-2009 22.13 **[SV-2009]** mapped values reach execution; diagnostic identity remains the owned physical source range (sim_directive_effects.rs)

## 12. Out-of-scope verification-infrastructure tier (⬜)

This tier is retained for continuity. Bounded class, synchronization, assertion,
clocking and foreign-interface paths now exist; only the **remaining advanced
forms** are out of scope. The partial/source statuses above and in groups 62–72
take precedence over this tier heading. No full verification-infrastructure
acceptance is claimed.

| Area | Constructs | Reference | Tag |
|---|---|---|---|
| Advanced classes | bounded packed/real/string/chandle/class-handle properties and delay-free methods now have source paths; timing-bearing tasks, unsupported field/capture layouts, garbage collection and constrained randomization remain outside the implementation | §1800-2009 ch8 | [SV-2005] |
| Clocking advanced verification controls | bounded sequence/property clock flow and procedural sample/drive/cycle-delay paths exist; unsupported dynamic-skew, cross-clock and verification-only timing combinations remain | §1800-2009 14.16.2 | [SV-2005] |
| Interprocess sync | semaphore/mailbox arrays, process-handle formals/arrays, and broader synchronization beyond bounded semaphores, mailboxes, and process handles | §1800-2009 ch15 | [SV-2005] |
| Assertions | immediate/deferred assertions and concurrent assert/assume/cover instances with sampled packed implications, legal `##0`/`##1` multiclock sequence boundaries, default-clock inheritance, `##` sequence concatenation/ranges, consecutive/nonconsecutive/goto repetition including unbounded endpoints, `or`, direct one-cycle `and`/`intersect`/`throughout`/`within`, `first_match`, bounded one-cycle property `not`/`and`/`or`/`iff`/`implies`/conditional forms, named sequence/property instances with positional/named/default argument expansion, compatible declaration/call-site clock metadata, asynchronous single-signal `disable iff`, bounded `accept_on`/`reject_on` controls and synchronous variants, sampled attempts, vacuity accounting and Reactive actions, plus Slang-owned sequence locals, top-level per-attempt local input-formal capture, declaration defaults, ordered local assignment/increment and subroutine-call match items, and isolated local snapshots for overlapping and branching sequence threads. H26 adds bounded blocking `expect`, sequence `.matched`, `$asserton`/`$assertoff`/`$assertkill`, and post-2009 level-0 ON/OFF/KILL `$assertcontrol` with hierarchy selectors; output/inout/ref formal copy-out, delayed/nested local-formal invocations, selected-local lvalues, repeated match-item bodies, unsupported expanded formal bodies, conflicting clock/disable metadata, cross-clock delays/combinators outside the legal boundary, unsupported temporal property operators, pass/fail/vacuity assertion-action controls, `.triggered`, and invalid control arguments/scopes remain fail-closed | §1800-2009 16.3–16.9, 16.11, 16.18, 20.11–20.12, F | [SV-2009] |
| Checkers | `checker … endchecker` | §1800-2009 ch17 | [SV-2009] |
| Coverage | covergroups, coverpoints, cross | §1800-2009 ch18 | [SV-2005] |
| Constrained randomization | `randomize()`, rand/c, constraints, `std::randomize` | §1800-2009 ch18 | [SV-2005] |
| DPI beyond the bounded subset | exports, packed/open arrays, reference formals, and context callbacks | §1800-2009 ch35 | [SV-2005] |
| PLI/VPI | full `tf_`/`acc_`/`vpi_` interface and PLI applications beyond the bounded bridge | §1364-2001 ch20–27 | [1995]/[2001] |
| Specify blocks | module paths, edge/state-dependent paths, `PATHPULSE$` | §1800-2009 ch30 | [1995]/[SV-2009 restored] |
| Timing checks | `$setup/$hold/$width/…`, notifiers, `&&&` conditions | §1800-2009 ch31 | [1995]/[SV-2009 restored] |
| SDF backannotation | `$sdf_annotate`, delay/check mapping | §1800-2009 ch32 | [1995]/[SV-2009 restored] |
| bind | bounded elaboration-time injection of finite modules/interfaces; checker/program/coverage injection remains outside the selected RTL scope | §1800-2009 23.11 | [SV-2005] |

## How to update this document

1. Inspect the owning lowering, IR, emitter and runtime together; record implemented
   forms and every material rejection/lifetime boundary, not just a match arm.
2. Add or maintain a regression source under `tests/`. Mark a new source path 🟦,
   or 🟨 when forms remain partial; a test that has not run is not acceptance.
3. Promote a stated scope to ✅ only with matching post-change public HDL results,
   recording the source revision, optimizer modes, toolchain and platform. Keep
   component-runtime evidence separate from actual Rust-emitted model execution.
4. Update the corresponding stable inventory ID and its source links. Calculate
   grouped counts from the table; never convert repair-target counts to pass counts.
5. Keep commands/methodology in `tests/readme.md` and dated run logs/investigations
   in ignored `persistence/`. Preserve standard tags unless separately reviewed.

## Remaining-work inventory

The original audit IDs are stable. **None of the 72 groups is marked completed or
accepted for this snapshot.** Of these, **8 are source-implemented** for their stated scope,
**51 are partial**, and **13 are missing**.
These are project-defined groups, not counts of IEEE keywords or clauses, and
`Source-implemented` is not `Completed`. Source links identify the owning path or
rejection boundary. Missing/out-of-scope entries do not imply every declaration
is rejected by the frontend; parsing or declaration capture is not execution.

| # | Status | Feature group | Current scope and remaining work | Implementation / rejection owner |
| ---: | --- | --- | --- | --- |
| 1 | Missing | Charge-storage nets | `trireg`, charge strengths, charge decay and charge sharing. | [design.rs](../src/sim/codegen/lowering/collection/design.rs), [nets.c](../src/sim/rt/scheduler/nets.c) |
| 2 | Missing | Tagged unions | Tagged storage, construction and matching. | [aggregates.rs](../src/sim/codegen/lowering/collection/aggregates.rs) |
| 3 | Partial | Real types | Scalar/fixed-array real storage, ports, value-formal calls, arithmetic, waits/events, changed-write notification and typed formatting are present. Generic container real/shortreal leaf operations exist. Addressable automatic real locals use registered heap payloads and real publication pins them. Queued automatic-real destinations, general real-reference calls and real-valued sampled/method callback results remain restricted. | [owned.rs](../src/sim/emit_c/owned.rs), [stores.rs](../src/sim/emit_c/owned/stores.rs), [dependencies.c](../src/sim/rt/scheduler/dependencies.c) |
| 4 | Partial | Strings | Owned strings now have module/static/automatic storage, input/output/inout/ref/const-ref call paths, returns, core methods, conversion and dynamic formatting. Collected string value ports and contents dependencies exist. Automatic string NBA targets, unsupported native captures/automatic monitors and broader aggregate/continuous forms remain; string formals and ports are no longer blanket-missing. | [strings.rs](../src/sim/emit_c/owned/strings.rs), [model.rs](../src/sim/emit_c/owned/model.rs), [ports.rs](../src/sim/codegen/lowering/collection/ports.rs) |
| 5 | Partial | Chandles | Native chandle null/copy/identity/Boolean operations, automatic/static locals, admitted aggregate/class fields, typed mixed signatures and input/output/inout/ref/const-ref/return paths exist. Matching collected reference-port leaves can share storage; ordinary chandle value-port links remain rejected. Packed containment, arithmetic, general continuous assignment and non-string object sensitivity are outside the bounded path. | [objects.rs](../src/sim/emit_c/owned/objects.rs), [ports.rs](../src/sim/codegen/lowering/collection/ports.rs) |
| 6 | Partial | Structures and untagged unions | Packed patterns/overlapping union views, recursive fixed unpacked leaves, deep copy, explicit member defaults and represented unequal-width unpacked untagged-union storage are present. SYN-012 closes the local packed-aggregate member projection gap and records legal packed struct, equal-width packed union, fixed unpacked struct, untagged union view and nested array-of-record contexts at widths 1/7/8/31/32/33/64/65/129, with mixed signed/unsigned `bit`/`logic` and reversed bounds, through declaration/local/member/row, equality/conditional/assignment/return cells. Collected recursive reference leaves and aggregate links exist. Unsupported declaration/net layouts, general native/resizable subroutine storage, tagged unions and arbitrary slices remain. | [aggregates.rs](../src/sim/codegen/lowering/collection/aggregates.rs), [fixed_projections.rs](../src/sim/codegen/lowering/collection/fixed_projections.rs), [fixed_values.rs](../src/sim/codegen/lowering/collection/fixed_values.rs), [fixed_patterns.rs](../src/sim/codegen/lowering/collection/fixed_patterns.rs), [SYN-012 matrix](../tests/sim_syn012_fixed_layout.rs) |
| 7 | Partial | Fixed unpacked arrays | Bounded copies, concatenations, slices and partial-index views retain logical coordinates and captured indices. R01 provides fixed-array conditional element/default semantics; R04 stages type-compatible module-procedural expressions before cell stores/NBA issue; R03 adds integral reductions and lexical fixed-record/row maps; SYN-027 adds declaration-order `reverse()` for fixed integral elements, selected rows and automatic local/formal payloads; SYN-028 adds deterministic `sort()`/`rsort()` for fixed integral elements, including selected rows, packed records, repeated keys, reversed/negative bounds, singleton arrays, and automatic local/formal payloads. SYN-007 accepts compatible fixed integral input-port value expressions, including conditional/function/pattern values, fixed slices and runtime-selected rows, with one source capture and declaration-order scatter. SYN-008 qualifies bounded fixed integral output whole/row/slice links, matching fixed-array references, aggregate output values, nested member targets and instance-array distribution. Bound nested patterns preserve repeated operand slots and packed leaf ranges, and the corrected NBA/default-pattern cases pass in the current Linux baseline. General native/resizable element, real-array expression, runtime output/ref/inout rewiring, subroutine and over-limit value combinations remain restricted. | [fixed_arrays.rs](../src/sim/codegen/lowering/containers/fixed_arrays.rs), [ports.rs](../src/sim/codegen/lowering/collection/ports.rs), [methods.rs](../src/sim/codegen/lowering/containers/methods.rs), [fixed_patterns.rs](../src/sim/codegen/lowering/collection/fixed_patterns.rs), [conditionals.rs](../src/sim/codegen/lowering/expressions/conditionals.rs), [fixed_reductions.rs](../src/sim/codegen/lowering/containers/fixed_reductions.rs), [sim_rtl_completion.rs](../tests/sim_rtl_completion.rs), [sim_feature_completion_g1.rs](../tests/sim_feature_completion_g1.rs) |
| 8 | Partial | Resizable containers and array methods | Dynamic/queue/associative allocation, copy, resize, defaults, traversal, bounded patterns, generic/nested leaves and collected value-port copies have source paths. Packed callbacks, reductions, locator/min/max/unique and ordering operations are present; their method metadata survives semantic-table growth. Fixed integral reductions use the separate R03 lexical path in group 7; that does not enable automatic captures for resizable callbacks. Non-packed endpoint/pop expressions, string-key index results, general subroutine/nested scalar-query forms and broader recursive/object combinations remain gated. | [containers/](../src/sim/codegen/lowering/containers/), [containers.rs](../src/sim/emit_c/owned/containers.rs), [containers/](../src/sim/emit_c/owned/containers/) |
| 9 | Source-implemented | Runtime enum methods | Scalar enum first/last/next/prev/num/name methods use owned declaration-order metadata, including sparse/signed values, wrapping counts, invalid-value defaults and owned names. | [queries.rs](../src/sim/emit_c/owned/objects/queries.rs), [strings.rs](../src/sim/emit_c/owned/strings.rs) |
| 10 | Partial | Casts | Scalar packed/real dynamic casts with success-only writes and enum membership, checked nominal class casts, fixed-size aggregate/array and packed-element dynamic/queue bit-stream paths exist. Native-string/object/recursive stream forms and unsupported reference targets remain restricted. Class downcasts are no longer blanket-missing. | [casts.rs](../src/sim/codegen/lowering/expressions/casts.rs), [queries.rs](../src/sim/emit_c/owned/objects/queries.rs), [objects.rs](../src/sim/emit_c/owned/objects.rs) |
| 11 | Partial | Data and array queries | Executed `$typename`, `$isunbounded`, `$bits`, `$left`/`$right`/`$low`/`$high`/`$increment`/`$size`/`$dimensions`/`$unpacked_dimensions` with descriptor-backed packed, fixed-array, dynamic/queue, associative-integral, and string metadata; nested runtime container dimensions and string/wildcard associative bounds remain unsupported. | [array_queries.rs](../src/sim/codegen/lowering/expressions/array_queries.rs), [queries.rs](../src/sim/emit_c/owned/objects/queries.rs) |
| 12 | Partial | Initialization and storage lifetimes | Declaration identity, lifetime and edition-specific initialization are represented. Scalar and fixed integral composite initializers admit zero-time user calls before SystemVerilog processes; static storage initializes once and automatic storage per activation. Explicit member defaults, mixed state domains and nested bound pattern operands have paths. The corrected packed-default path passes in the current Linux baseline. Timing-bearing calls, general native/resizable aggregate/subroutine layouts and ambiguous lifetime provenance remain restricted. | [initialization.rs](../src/sim/codegen/lowering/collection/initialization.rs), [declarations.rs](../src/sim/codegen/lowering/statements/declarations.rs), [calls.rs](../src/sim/emit_c/owned/calls.rs) |
| 13 | Partial | Net resolution and strengths | Continuous, gate, packed port, fixed wired-array, admitted hierarchical and collapsed-inout drivers retain per-source strengths, wire/tri and wired rules, pulls/supplies, disjoint selected writes and delayed slots. Hierarchical drivers are accepted for parent-to-child, selected constant-bit/part, generated-name and upward-qualified targets through owning-instance slots; R05 applies directional dissimilar-type selection, source-located warnings and winning-declaration propagation delays, including no delay. Expression-only concatenated actuals retain their connectivity; strict true-alias type checks are separate. Unsupported net/aggregate shapes, procedural net writes, uwire inouts, trireg and switch/resistive propagation remain; explicit vector continuous-assignment strength legality is unchanged. | [port_net_types.rs](../src/sim/codegen/lowering/collection/port_net_types.rs), [net_collapse.rs](../src/sim/codegen/lowering/collection/net_collapse.rs), [nets.rs](../src/sim/codegen/lowering/collection/nets.rs), [nets.c](../src/sim/rt/scheduler/nets.c) |
| 14 | Missing | Switch-level primitives | `nmos`, `pmos`, `cmos`, `rnmos`, `rpmos`, `rcmos`, `tran`, `tranif0/1`, `rtran`, `rtranif0/1`, including resistive strength propagation. | [gates.rs](../src/sim/codegen/lowering/collection/gates.rs), [semantic.rs](../src/sim/semantic.rs) |
| 15 | Partial | Primitive instance arrays | Built-in gate-array elements are imported and lowered through the ordinary typed gate path, with an existing bit-distribution regression. UDP and switch array kinds remain rejected; broader primitive-array/delay/connection combinations lack current acceptance. | [node_import.rs](../src/core/db/database/node_import.rs), [gates.rs](../src/sim/codegen/lowering/collection/gates.rs) |
| 16 | Partial | User-defined primitives | Scalar combinational UDP tables and instances are captured through the owned snapshot and executed with 0/1/X/Z, `b`, and `?` matching, unmatched-X behavior, overlap validation, delays/strengths, repeated instances and primitive arrays. Sequential level/edge/state initialization, vector/aggregate ports, and switch-level UDP behavior remain rejected. | [slang_c_api.cpp](../src/wrapper/slang_c_api.cpp), [semantics.rs](../src/ffi/slang/semantics.rs), [gates.rs](../src/sim/codegen/lowering/collection/gates.rs) |
| 17 | Missing | Specify blocks | Specify parameters, module path delays, state/edge-dependent paths and pulse-control semantics including `PATHPULSE$`. | [semantic.rs](../src/sim/semantic.rs), [design.rs](../src/sim/codegen/lowering/collection/design.rs) |
| 18 | Missing | Timing checks | `$setup`, `$hold`, `$setuphold`, `$recovery`, `$removal`, `$recrem`, `$skew`, `$timeskew`, `$fullskew`, `$period`, `$width`, `$nochange`, notifiers and timing-check conditions. R06's Boolean conditional predicates do not implement timing-check `&&&` conditions. | [system_tasks.rs](../src/sim/codegen/lowering/statements/system_tasks.rs), [semantic.rs](../src/sim/semantic.rs) |
| 19 | Missing | SDF annotation | `$sdf_annotate` and application of annotated delays/checks. | [system_tasks.rs](../src/sim/codegen/lowering/statements/system_tasks.rs) |
| 20 | Partial | Gate terminal forms | Selected/hierarchical input and output forms, input expressions/constants, mixed-width normalization and multiple-output buf/not have typed lowering and per-output driver slots. The earlier universal equal-width and 64-terminal caps are not in this path. Invalid/real terminals, unsupported output shapes and incompatible driver contexts remain restricted. | [gates.rs](../src/sim/codegen/lowering/collection/gates.rs), [sim_gates.rs](../tests/sim_gates.rs) |
| 21 | Partial | Continuous and gate delays | Whole packed, constant-selected and fixed-array continuous/gate drivers use captured inertial updates, independent cancellation identity and constant delay tuples. Standalone/collapsed/aliased net propagation delays apply after resolution. R05 selects the winning port declaration's delay, including an undelayed winner; conflicting selected true-alias member delays remain rejected. Built-in gate-array elements use the gate path. Dynamic driver-delay expressions, net-array declaration-delay capture, unsupported resolved-net/aggregate/terminal combinations and switch/UDP kinds remain restricted. | [delays.rs](../src/sim/codegen/lowering/delays.rs), [nets.rs](../src/sim/codegen/lowering/collection/nets.rs), [storage.rs](../src/sim/emit_c/model/storage.rs), [inertial.rs](../src/sim/emit_c/owned/inertial.rs), [nets.c](../src/sim/rt/scheduler/nets.c) |
| 22 | Partial | Delay expressions | Negative real delays and unsupported expression forms remain. Runtime packed/real procedural delays, X/Z-to-zero delays, negative packed time conversion, femtosecond precision, local-precision rounding and checked overflow have regression sources for both optimizer modes. | [delays.rs](../src/sim/codegen/lowering/delays.rs), [events.rs](../src/sim/emit_c/owned/events.rs) |
| 23 | Partial | Time precision and literal semantics | Physical scheduler ticks are checked femtoseconds across the standard 1fs–100s range. Ordinary time-literal values, source spelling, local `timeunit`/`timeprecision`, signed/scientific unit suffixes, and direct-delay rounding follow the selected 2009 policy; broader edition-sensitive literal semantics remain separately tracked. | [timescale.rs](../src/sim/codegen/timescale.rs), [dispatch.rs](../src/sim/codegen/lowering/expressions/dispatch.rs) |
| 24 | Partial | Port connections | Packed/default/expression links, selected outputs, bounded fixed-array and recursive reference leaves, scalar-real links, collected string/aggregate copies and resizable copy links are represented. SYN-007 adds compatible fixed integral input-array value links for conditional/function/pattern values, fixed slices and runtime-selected rows; source and selector dependencies re-evaluate one captured value into formal storage. SYN-008 adds bounded fixed integral output whole/row/slice links, matching fixed-array references, aggregate output values, nested member terminals and module instance-array distribution; invalid output expressions and inequivalent refs retain diagnostics. R05 admits dissimilar resolved inouts on existing whole/selected/concatenated/fixed-cell paths; a high_expr without one high declaration is a connected expression, not an empty port. Chandle value links, uwire inouts, trireg, uncollected layouts, real/native/resizable array expressions and runtime output/ref/inout rewiring remain restricted. | [ports.rs](../src/sim/codegen/lowering/collection/ports.rs), [fixed_arrays.rs](../src/sim/codegen/lowering/containers/fixed_arrays.rs), [nets.rs](../src/sim/codegen/lowering/collection/nets.rs), [port_net_types.rs](../src/sim/codegen/lowering/collection/port_net_types.rs), [sim_rtl_completion.rs](../tests/sim_rtl_completion.rs) |
| 25 | Partial | Libraries and configurations | SYN-032 admits explicit `--libmap`, repeatable `--libfile [library=]path`, `--library-order`, and `--default-library` inputs through the cache-only source boundary. Named library units, default liblist search, top `module:config` selection, cell/instance `use`, nested configurations, and parameter propagation use ordinary owned elaborated instances. Map includes and `*`/`?`/`**` file patterns are expanded in sorted order. Encrypted IP, foreign co-simulation, unrestricted native filesystem discovery, and unsupported or ambiguous/missing bindings remain rejected with source diagnostics. | [compile.rs](../src/core/compile.rs), [slang_c_api.cpp](../src/wrapper/slang_c_api.cpp), [llg.rs](../src/bin/llg.rs) |
| 26 | Partial | Packages | Shared runtime package variables, dependent initialization, static package state, qualified/imported/re-exported declarations and subroutines, wildcard package exports, and bounded `$unit` visibility have source paths. The SYN-016 matrix checks finite type/value parameter specialization, typedef/enum and constant query metadata, folded real/string constants, declaration-scope `let`, terminating constant functions, generated scopes, parameterized interfaces, and separate/merged compilation units in both optimizer modes. The common initializer path admits zero-time calls for represented scalar/fixed integral values, not arbitrary package/native/resizable combinations. Unsupported layouts and unresolved callable environments remain restricted. | [design.rs](../src/sim/codegen/lowering/collection/design.rs), [call_contracts.rs](../src/sim/codegen/lowering/collection/call_contracts.rs), [sim_syn016_elaboration.rs](../tests/sim_syn016_elaboration.rs) |
| 27 | Partial | Net aliases | Canonical packed networks, alias-visible cells, independent contribution slots, force mappings, lifecycle and waveform/dependency addresses are present. SYN-010 adds fully indexed fixed net-array elements, reversed ranges, multidimensional packed projections, and an admitted array-element inout link; both optimizer modes exercise the shared connectivity and force/release path. True aliases require matching declared net types before R05 port collapse; repeated, self, variable, or incompatible forms remain frontend rejections. Unmapped writes and unsupported dynamic, forbidden cross-scope, aggregate, and switch-level forms remain restricted. | [nets.rs](../src/sim/codegen/lowering/collection/nets.rs), [port_net_types.rs](../src/sim/codegen/lowering/collection/port_net_types.rs), [net alias regression](../tests/sim_net_resolution.rs), [fixture](../tests/fixtures/sim/net_resolution/syn_010_fixed_net_aliases.sv) |
| 28 | Partial | SystemVerilog scheduling regions | The runtime has typed Preponed, Active, Inactive, Pre-NBA/NBA/Post-NBA, Pre-Observed/Observed/Post-Observed, Reactive/Re-Inactive/Re-NBA, Pre-Postponed/Postponed, and explicit PLI callback queues with fixed-point re-entry and immutable observation views. Clocking sampling and output drives, program processes, and deferred/concurrent assertions consume these regions; the bounded VPI bridge provides startup/start/end hooks, while the broader VPI region API remains later work. | [scheduler.c](../src/sim/rt/scheduler/scheduler.c), [llg_rt.h](../src/sim/rt/llg_rt.h) |
| 29 | Partial | Zero-delay process behavior | Ordinary wait-free `always` repeats with a cooperative, configurable zero-time budget; exhausted budgets produce a source-bearing nonconvergence diagnostic and nonzero simulator status. `always_comb`/`always_latch` keep their time-zero and sensitivity shaping, including the bounded aggregate projections qualified by SYN-014. Constant-false/unknown `wait` remains suspended without preventing time advancement. | [statements.rs](../src/sim/emit_c/owned/statements.rs), [policy.c](../src/sim/rt/scheduler/policy.c), [sim_syn014_process_contexts.rs](../tests/sim_syn014_process_contexts.rs) |
| 30 | Partial | Conditional event controls | Packed/scalar-real evaluation, LSB edges, iff, array/container dependencies and mixed event lists are present. Read-only callbacks admit bounded numeric helpers with value/const-ref formals, private locals, structured loops, nested eligible calls, fixed-array integral reductions and stateless static formals (32-level inlining guard). Persistent locals or stateful reads, native/DPI dispatch, visible effects, suspension, real edges and unsupported captures/qualifiers remain rejected; unique/priority diagnostics are retained. Numeric/typed-handle captures do not establish arbitrary shared/native capture support. | [pure_calls.rs](../src/sim/emit_c/owned/pure_calls.rs), [call_contracts.rs](../src/sim/codegen/lowering/collection/call_contracts.rs), [event_waits.rs](../src/sim/emit_c/owned/event_waits.rs), [captures.rs](../src/sim/emit_c/owned/captures.rs) |
| 31 | Partial | Named-event references | Scalar/fixed-array event identity, runtime selection, hierarchy, reassignment/null aliases, inline event formals, triggered state and wait_order have paths. Direct, delayed, event-controlled and repeated-event nonblocking triggers are represented. Dynamic/associative/queue events, unsupported repeat shapes and event-formal numeric/virtual dispatch remain restricted. | [events.rs](../src/sim/codegen/lowering/statements/events.rs), [events.rs](../src/sim/emit_c/owned/events.rs), [statements.rs](../src/sim/emit_c/owned/statements.rs) |
| 32 | Partial | Array/container sensitivity | Fixed-array element/contents and container contents/shape markers feed implicit/explicit readers, copies, resizing and admitted methods. SYN-014 additionally qualifies fixed record projections, aggregate input links, called-function reads, conditional arms and written-member exclusion in `always_comb`/`always_latch`; R03 fixed-reduction maps can read lexical automatic locals/formals; R06 includes every predicate clause in dependency analysis despite runtime short-circuiting. String objects have markers. Non-string object dependencies, unrepresented nested readers, automatic resizable-method callback captures and automatic monitor dependencies remain restricted. | [dependencies.rs](../src/sim/codegen/lowering/collection/dependencies.rs), [events.rs](../src/sim/emit_c/owned/events.rs), [dependencies.c](../src/sim/rt/scheduler/dependencies.c), [sim_syn014_process_contexts.rs](../tests/sim_syn014_process_contexts.rs) |
| 33 | Partial | Intra-assignment timing | Constant/runtime delay and explicit event/repeat NBA paths capture values and destinations; blocking timing keeps the RHS across suspension and selected commits merge current destination bits. Event-trigger scheduling has its own issue-time path. Nested repeat timing, queued automatic real/string targets, reference-formal NBAs and unsupported subroutine-storage forms remain restricted. Ordinary repeat-loop/event-body composition also has a source path; this does not admit every standalone repeated-timing-node form. Its current HDL acceptance is unverified. | [assignments.rs](../src/sim/codegen/lowering/statements/assignments.rs), [dispatch.rs](../src/sim/codegen/lowering/statements/dispatch.rs), [statement_import.rs](../src/core/db/database/statement_import.rs), [statements.rs](../src/sim/emit_c/owned/statements.rs), [stores.rs](../src/sim/emit_c/owned/stores.rs) |
| 34 | Partial | Force and release | Persistent packed/real force, constant-selected/concatenated and canonical alias-force mappings have source paths. Live RHS uses the bounded read-only evaluator. Automatic/array and unsupported indirect/hierarchical/net targets still reject; ownership-safe alias mapping does not admit raw writes to alias-visible cells. | [force.rs](../src/sim/emit_c/owned/force.rs), [drivers.rs](../src/sim/codegen/lowering/statements/drivers.rs) |
| 35 | Partial | Procedural assign/deassign | Whole packed/real PCA targets and admitted packed concatenations use replaceable bindings, live RHS, force precedence and retained values after deassign. Selected/hierarchical/array/streaming/recursive aggregate targets and RHS calls outside the evaluator contract remain restricted. | [runtime_tasks.rs](../src/sim/emit_c/owned/runtime_tasks.rs), [drivers.rs](../src/sim/codegen/lowering/statements/drivers.rs) |
| 36 | Partial | Named disable | Declaration/instance-resolved activation targets, named forks, cancellation edges and cleanup-before-copyout have source paths. Broader recursive/native/shared-capture interactions remain bounded; prior cross-process test names do not establish post-migration acceptance. | [activations.rs](../src/sim/emit_c/owned/activations.rs), [calls.rs](../src/sim/codegen/lowering/statements/calls.rs) |
| 37 | Partial | Loop forms and captures | R02 retains mixed fixed unpacked/packed dimensions from the iterated type, omitted slot positions, formal bounds, singleton/implicit integral vectors and signed ascending/descending endpoints. Break/continue scope and endpoint-before-increment guards remain. Dynamic/queue and integral/string associative traversal, private packed/real locals and recognized captures have separate paths. Nested resizable foreach and unsupported string/shared/native captures remain rejected; writes to read-only foreach iterators are not legal-language backlog. | [foreach.rs](../src/core/db/slang_types/foreach.rs), [control_flow.rs](../src/sim/codegen/lowering/statements/control_flow.rs), [captures.rs](../src/sim/emit_c/owned/captures.rs) |
| 38 | Source-implemented | Uniqueness and priority checking | Runtime `unique`, `unique0` and `priority` violation checks retain source identity, ordinary first-match execution, default/else suppression, and exact/casez/casex matching. | [qualifiers.rs](../src/sim/emit_c/owned/qualifiers.rs), [sim_unique_priority.rs](../tests/sim_unique_priority.rs) |
| 39 | Partial | Pattern matching | Integral constant, `.*` wildcard and `.name` binding patterns plus recursive fixed packed/unpacked structure patterns execute in conditional `if`/`?:` predicates with source-ordered `&&&`, resolved member identities, defined Boolean results, automatic lexical bindings and effect/dependency/lint coverage. Tagged patterns, `case ... matches` execution and pattern-case bodies remain fail-closed. | [predicates.rs](../src/core/db/database/predicates.rs), [conditionals.rs](../src/sim/codegen/lowering/expressions/conditionals.rs), [statement_import.rs](../src/core/db/database/statement_import.rs), [control.rs](../src/sim/emit_c/owned/control.rs) |
| 40 | Partial | Side-effecting operators | Statement and expression-valued `++/--` and compound assignments use the canonical mutation path for admitted whole, selected and fixed-array-element lvalues, preserving one-time target evaluation, prefix/postfix results, packed state conversion and real updates. Non-lvalues and unsupported aggregate/object targets remain rejected. | [assignments.rs](../src/sim/codegen/lowering/statements/assignments.rs), [stores.rs](../src/sim/emit_c/owned/stores.rs), [operations.rs](../src/sim/codegen/lowering/expressions/operations.rs), [sim_feature_completion_g1.rs](../tests/sim_feature_completion_g1.rs) |
| 41 | Partial | Set membership | Typed scalar/range/wildcard and admitted real/string/fixed-aggregate contexts, plus packed dynamic/queue/associative value membership, have paths. General recursive/native-object and unrepresented aggregate contexts remain restricted. | [membership.rs](../src/sim/codegen/lowering/expressions/membership.rs), [expressions.rs](../src/sim/emit_c/owned/expressions.rs) |
| 42 | Partial | Streaming | Packed and bounded fixed/resizable packed-element streams capture the full RHS, then evaluate/publish destinations in stream order. Fixed nested aggregate casts, selected rows/members, and `ref`/`const ref` projections are qualified; mixed targets permit at most one dynamic/queue destination, and deferred container notifications follow intermediate cleanup. Runtime `with` selectors on admitted fixed-array destinations and one-dimensional sources are implemented. Associative destinations, native strings and general recursive/reference/object forms remain restricted. | [streaming.rs](../src/sim/emit_c/owned/streaming.rs), [streaming.rs](../src/sim/codegen/lowering/containers/streaming.rs), [sim_data_types_next.rs](../tests/sim_data_types_next.rs) |
| 43 | Partial | Reference subroutine arguments | Packed and fixed aggregate/array descriptors and packed queue-element cells retain original identity across removal and cancellation. Native string/chandle references have separate address paths. General native/resizable aggregate references, non-packed queue references, reference-formal NBAs and selected-reference file destinations remain restricted. Internal selected descriptors do not make frontend-illegal packed bit/part actuals legal HDL. | [references.rs](../src/sim/emit_c/owned/references.rs), [reference_writes.c](../src/sim/rt/scheduler/reference_writes.c) |
| 44 | Missing | Recursive timed tasks | Recursion through delay/wait-bearing tasks. | [calls.rs](../src/sim/codegen/lowering/statements/calls.rs) |
| 45 | Partial | Parallel subroutine bodies | Owned packed/real branch frames and recognized handle snapshots, delay-only native task suspension, event/cancellation-sensitive inline task paths and cancellation-before-copyout are represented. Recursive timed calls, timing-bearing class/interface tasks and arbitrary shared/native captures remain outside the bounded implementation. Ordinary blocking timing in functions is illegal. | [calls.rs](../src/sim/codegen/lowering/statements/calls.rs), [captures.rs](../src/sim/emit_c/owned/captures.rs) |
| 46 | Partial | Cross-instance subroutine calls | Resolved callee identity can select an owning module/interface/package environment, including hierarchical/parent and per-instance dispatch. Existing regression sources include sim_hierarchical_subroutine_instances_and_parent_dispatch in sim_function.rs. Unresolved targets, event-formal value calls, timing-bearing class/interface methods and general aggregate/native combinations remain restricted; blanket cross-instance rejection is obsolete. | [call_contracts.rs](../src/sim/codegen/lowering/collection/call_contracts.rs), [sim_function.rs](../tests/sim_function.rs) |
| 47 | Partial | Subroutine copy-out and storage | Packed/real and native string/chandle owners, fixed integral array/struct/union value formals/locals/returns, static storage and cancellation-aware output/inout copy-out have source paths. R01 array conditionals preserve immediate-element defaults within admitted calls. Event formals are inline templates. The SYN-013 matrix exercises finite zero-time aggregate copy-out, nested const-ref forwarding, local named-block cleanup and static/automatic storage in both language editions. Process-handle formals, general native/resizable aggregate arguments, unrepresented automatic/shared lifetimes and queued automatic real/string targets remain restricted. Illegal automatic-subroutine NBAs are not missing legal-language coverage. | [calls.rs](../src/sim/emit_c/owned/calls.rs), [model.rs](../src/sim/emit_c/owned/model.rs), [calls.rs](../src/sim/codegen/lowering/statements/calls.rs), [sim_syn013_zero_time_calls.rs](../tests/sim_syn013_zero_time_calls.rs) |
| 48 | Partial | File I/O | Typed console/file descriptors, multichannel output, retained input/format strings, bounded scanning/character/line/binary reads and position/status operations are present. Sources/targets are staged across publication callbacks. Selected sub-accesses through reference formals and broader aggregate/native input targets remain restricted; test anchors are not current pass evidence. | [input.rs](../src/sim/emit_c/owned/input.rs), [scanning.c](../src/sim/rt/scheduler/scanning.c) |
| 49 | Partial | Display families and formatting | Typed packed/real/string console, file and string formatting supports the retained conversion/radix/width/precision/hierarchy contracts. Owned postponed output and monitor re-evaluation paths exist. Aggregate formatting, automatic monitor dependencies and callbacks outside the restricted read-only function/capture model remain unsupported. | [formatting.rs](../src/sim/emit_c/owned/formatting.rs), [runtime_tasks.rs](../src/sim/emit_c/owned/runtime_tasks.rs) |
| 50 | Source-implemented | String formatting tasks/functions | `$sformat`, `$swrite` and their radix variants, plus `$sformatf`, use typed owned arguments and the shared formatter. Native string and packed string-like destinations receive normal truncation/padding; dynamic/nested format expressions and source-order, exactly-once arguments have regression anchors in `sim_h04_string_format.rs`. | [strings.rs](../src/sim/emit_c/owned/strings.rs), [formatting.rs](../src/sim/emit_c/owned/formatting.rs) |
| 51 | Source-implemented | Memory file loading and writing | Fixed packed memories with multiple unpacked dimensions and constant selected rows/slices use one owned view descriptor. Remaining dimensions load and write in low-to-high row-major order, declaration direction maps to flat storage, `@` addresses the highest dimension, incomplete final rows preserve unread subwords, and explicit ranges preserve source direction. Verilog-2001 one-dimensional defaults keep declaration order; SystemVerilog-2009 defaults use low-to-high order. Dynamic/queue/associative, real, non-packed aggregate, range-selected, and fully indexed memories remain restricted, as are writers in Verilog-2001 mode. Paths accept owned string expressions. | [runtime_tasks.rs](../src/sim/emit_c/owned/runtime_tasks.rs), [memory_io.c](../src/sim/rt/scheduler/memory_io.c), [sim_memory_views.rs](../tests/sim_memory_views.rs) |
| 52 | Partial | Real-time reporting and time formatting | `$timeformat` stores design-wide units, precision, suffix, and minimum field width; `%t` converts integral and real values from each owning module's time unit across display/write/strobe/monitor, while `$realtime` remains fractional and `$time`/`$stime` round to the nearest local unit (exact halves upward) before `$stime` applies its low-32-bit result width. | [native_tasks.rs](../src/sim/emit_c/owned/native_tasks.rs), [system.rs](../src/sim/emit_c/owned/system.rs) |
| 53 | Partial | Simulation suspension | `$stop` supports resumable coroutine suspension and explicit CLI resume/exit policy; a full interactive debugger/control protocol is outside this boundary. | [simulation_control.c](../src/sim/rt/scheduler/simulation_control.c) |
| 54 | Missing | PLA modeling | Synchronous/asynchronous AND/NAND/OR/NOR array/plane system tasks remain missing from simulator lowering; SYN-035 excludes them from the selected synthesis profile until a named target supplies acceptance evidence. | [system_tasks.rs](../src/sim/codegen/lowering/statements/system_tasks.rs) |
| 55 | Partial | Stochastic queues | `$q_initialize/$q_add/$q_remove/$q_full/$q_exam` with FIFO/LIFO state, documented status codes, simulation-time statistics and whole packed integer outputs; selected/real output targets remain rejected. | [native_tasks.rs](../src/sim/emit_c/owned/native_tasks.rs), [stochastic.c](../src/sim/rt/scheduler/stochastic.c) |
| 56 | Partial | Random-number facilities | Legacy $random and seven $dist_* routines, per-process $urandom/$urandom_range, process srandom/randstate and fork-derived streams have source paths with retained deterministic C vectors. Class-object RNG methods, constrained/structured randomization and full container-shuffle/process-stream integration remain absent. | [system.rs](../src/sim/emit_c/owned/system.rs), [objects.rs](../src/sim/emit_c/owned/objects.rs), [llg_random.c](../src/sim/rt/llg_random.c) |
| 57 | Source-implemented | Command-line plusargs | `$test$plusargs/$value$plusargs` receive arguments after the `llg` `--` delimiter; exact prefix matching, typed decimal/hex/binary/octal/real/string conversion, wide 4-state values, literal percent escapes, repeated-argument first match, and unchanged destinations on failed queries have regression anchors in `sim_plusargs.rs`. | [input.rs](../src/sim/emit_c/owned/input.rs), [plusargs.c](../src/sim/rt/scheduler/plusargs.c) |
| 58 | Source-implemented | Runtime mathematical functions | All 21 real functions from IEEE 1800-2009 table 20-4 now use typed IR and the specified C math functions, with numeric argument conversion. Procedural tests cover runtime arguments, one-time evaluation and C domain behavior. Existing real-context restrictions are counted in group 3. | [system.rs](../src/sim/emit_c/owned/system.rs) |
| 59 | Source-implemented | Runtime severity tasks | `$fatal/$error/$warning/$info` use typed, exactly-once message evaluation with source-context prefixes; `$fatal` validates constant finish number 0/1/2, runs the existing termination/final handoff, and level-2 finish statistics include stable severity counters. Elaboration-time frontend diagnostics remain a separate capability. | [statements.rs](../src/sim/emit_c/owned/statements.rs), [simulation_control.c](../src/sim/rt/scheduler/simulation_control.c) |
| 60 | Source-implemented | Host command execution | `$system` task/function forms preserve omitted (`system(NULL)`) versus explicit-empty commands, use one optional owned string and explicit generated-process permission, and return raw host `system()` status; shell syntax and status encoding remain platform-specific. | [native_tasks.rs](../src/sim/emit_c/owned/native_tasks.rs), [simulation_control.c](../src/sim/rt/scheduler/simulation_control.c) |
| 61 | Partial | Waveform selection and extended VCD | `$dumpvars` depth/scope/variable filtering is implemented for ordinary VCD/FST catalogs; the `$dumpports` extended-VCD family remains unsupported. | [statements.rs](../src/sim/emit_c/owned/statements.rs), [llg_wave.c](../src/sim/rt/llg_wave.c) |
| 62 | Partial | Classes | Nominal construction/constructor chaining, inherited/static dispatch, virtual/super methods, parameterized layouts, forward/const/access metadata and null checks have typed source paths. Packed, real, string and admitted chandle/class-handle fields have explicit storage/teardown. Objects, including unreachable/cyclic graphs, remain until model close; no garbage collector exists. Timing-bearing class tasks, general field/capture layouts and constrained randomization remain restricted. | [native_access.rs](../src/sim/emit_c/owned/native_access.rs), [classes.rs](../src/sim/emit_c/model/classes.rs), [classes.rs](../src/sim/codegen/lowering/collection/classes.rs) |
| 63 | Partial | Program blocks | Reactive initial launch and region behavior are represented; origin-specific `$exit`, separate initial/descendant counts, last-initial descendant cancellation, and immediate all-program completion are corrected in source. Regression sources are in `sim_program.rs` and `sim_review_batch2.rs`; acceptance is pending. | [process_registry.c](../src/sim/rt/scheduler/process_registry.c), [processes.rs](../src/sim/codegen/lowering/collection/processes.rs) |
| 64 | Partial | Clocking declarations, sampling and synchronous drives | Clocking declarations, default/global clocking, input directions/aliases, clocking events and constant `#1step`/`#0`/positive input skews use owned sample storage and runtime scheduling and optimizer-parity regression sources. Constant output/inout skews use captured Re-NBA drives (off-event drives defer to the next event, including resolved net slots), simple signal output edge qualifiers and selected targets are supported, and integral `##N` waits repeat the resolved default clocking event. Legal assertion sequence/property clock-flow across `##0`/`##1` boundaries and default-clock inheritance are also supported; dynamic output skews, unsupported cross-clock delays/combinators and frontend-rejected concatenated clockvar lvalues remain unsupported. | [clocking.rs](../src/sim/emit_c/owned/clocking.rs), [clocking_context.rs](../src/sim/codegen/lowering/clocking_context.rs) |
| 65 | Partial | Advanced interprocess synchronization | Process identity/status/control/await, semaphore new/get/put/try_get and bounded typed/untyped mailbox FIFO operations have source paths. Mailbox typed records retain enum/class/handle nominal identity and mismatch status; consuming delivery commits before callbacks and peek does not consume. Packed/native-real publication pins registered destinations. Semaphore/mailbox arrays, process formals/arrays, general aggregate messages and arbitrary native/shared captures remain outside the bounded path. | [mailboxes.rs](../src/sim/emit_c/owned/mailboxes.rs), [mailboxes.c](../src/sim/rt/scheduler/mailboxes.c), [wait_queues.c](../src/sim/rt/scheduler/wait_queues.c) |
| 66 | Partial | Assertions and sampled values | Immediate and deferred `assert`/`assume`/`cover` evaluate once with four-state truth; deferred `#0` forms retain issue-time value captures, action-time legal references, same-slot glitch coalescing, module-level members, Reactive actions and optimizer-parity regression sources (`sim_partial_features/assertions.rs`). Concurrent instances cover Preponed sampling, legal `##0`/`##1` multiclock sequence boundaries, nearest default-clock inheritance, asynchronous single-signal `disable iff`, bounded `accept_on`/`reject_on` and synchronous variants, overlapping attempts, vacuity accounting, Reactive actions, sequence `##` delays/ranges, consecutive/nonconsecutive/goto repetition with unbounded endpoints, `or`, direct one-cycle `and`/`intersect`/`throughout`/`within`, `first_match` endpoint selection, bounded one-cycle property `not`/`and`/`or`/`iff`/`implies`/conditional forms, and named sequence/property instances with positional/named/default argument expansion and compatible declaration/call-site clock metadata (`sim_concurrent_assertions.rs`). H24 additionally covers Slang-owned sequence-local storage, top-level typed local input-formal capture and declaration defaults, ordered local assignment/increment/subroutine-call match items, and isolated local snapshots across overlapping attempts and branching sequence threads (`h24_local_match.sv`, `h24_formal_default.sv`, `h24_branch_locals.sv`). H26 adds bounded blocking `expect`, sequence `.matched`, `$asserton`/`$assertoff`/`$assertkill`, and post-2009 level-0 ON/OFF/KILL `$assertcontrol` with hierarchy selectors (`sim_concurrent_assertions.rs`). Sampled-value functions use explicit/default edge domains with initial and gated history, preponed `$sampled`, packed status functions, LSB/X/Z edge rules, and global-clock history/status forms (`sim_sampled_values.rs`). Output/inout/ref formal copy-out, delayed/nested local-formal invocations, selected-local lvalues, repeated match-item bodies, formal-bound instances with unsupported expanded bodies, conflicting clock/disable metadata, unsupported cross-clock delay/combinator forms, unsupported temporal property operators, pass/fail/vacuity assertion-action controls, future global sampled-value functions, complex sampled event controls, `.triggered`, and invalid control arguments/scopes remain fail-closed. Current emitter acceptance is narrower than retained graph/lowering coverage: arbitrary match-item/callee bodies and shared/native captures must pass the owned callback restrictions; real sampled registrations remain rejected. Sequence-local source fixes do not establish whole-suite acceptance. | [assertions.rs](../src/sim/codegen/lowering/assertions.rs), [assertions/](../src/sim/emit_c/owned/assertions/), [pure_calls.rs](../src/sim/emit_c/owned/pure_calls.rs) |
| 67 | Missing | Checkers | Checker declarations, instances and checker execution. | [semantic.rs](../src/sim/semantic.rs) |
| 68 | Missing | Functional coverage | Covergroups, coverpoints, bins, crosses, sampling, coverage queries/control and coverage database system tasks. | [semantic.rs](../src/sim/semantic.rs), [system_tasks.rs](../src/sim/codegen/lowering/statements/system_tasks.rs) |
| 69 | Missing | Constrained and structured randomization | `rand/randc`, constraints, object and `std::randomize`, `randcase` and `randsequence`. | [semantic.rs](../src/sim/semantic.rs), [methods.rs](../src/sim/codegen/lowering/objects/methods.rs) |
| 70 | Partial | DPI | Bounded scalar DPI-C imports stage owned outputs while borrowing inputs; all foreign string results are cloned before aliased copy-out, which follows cancellation checks. Existing contracts cover scalar bit/logic/reg, integral atoms, real/shortreal, chandle/string and explicit libraries. Exports, packed/open arrays, ref/event formals and context callbacks remain unsupported; foreign-code execution has no new acceptance result. | [signatures.rs](../src/sim/codegen/lowering/collection/signatures.rs), [dpi.rs](../src/sim/emit_c/model/dpi.rs) |
| 71 | Partial | PLI/VPI | Bounded generated VPI metadata, registration, compiletf/sizetf/calltf, scalar/vector/real values, hierarchy lookup/iteration, plugin startup and start/end callbacks have source paths. Call arguments borrow registered snapshots. Full tf_/acc_/vpi_ interfaces, arbitrary properties/iterators/regions/control and VPI calls from read-only evaluators remain restricted. | [native_tasks.rs](../src/sim/emit_c/owned/native_tasks.rs), [llg_vpi.c](../src/sim/rt/llg_vpi.c) |
| 72 | Partial | Virtual interfaces | Typed instance/modport handles, packed members, delay-free methods, clocking-input reads, null/type checks and bounded fixed/dynamic/queue handle storage have paths. Timing-bearing tasks, event-formal virtual dispatch, dynamic output/inout clocking, associative/nested layouts and general polymorphic/native-capture forms remain restricted. | [virtual_interfaces.rs](../src/sim/codegen/lowering/collection/virtual_interfaces.rs), [native_access.rs](../src/sim/emit_c/owned/native_access.rs), [interfaces.rs](../src/sim/emit_c/model/interfaces.rs) |
