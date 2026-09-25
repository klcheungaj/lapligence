# Simulator Feature Checklist — IEEE 1364 / IEEE 1800

This is the maintained feature inventory for the `llg` simulator. It records
implementation scope separately from acceptance evidence. Revision tags and
standard references retain the existing inventory's terminology. Host regression
results establish their tested scopes, not clause-by-clause conformance.
The file remains the sole maintained simulator feature-status document.

Section numbers retain the original inventory's Verilog (§1364-2001 x.y),
SystemVerilog and verification (§1800-2009 y.z) references. Clause numbers and
production labels needed to read and audit the selected profile are recorded
in this tracked document, ledger, and manifest. The workspace-local
`docs/specification/` PDFs and extracted maps are optional review references;
they are untracked and are not required to run the checker or reproduce the
coverage audit. They were not independently revalidated in this source-status
audit.
Detailed source investigations and run records belong in local `persistence/`.

SYN-025-N09 now retains the enclosing pattern-case comparison mode for both
union tag bits and payload constants. The prior casex/X-tag negative oracle is
corrected; the casez/X-tag negative and ordinary checked-access diagnostics
remain. `sim_review_tasks08_11` and owned emitter `pattern_modes` regressions
cover the source and comparison-owner paths; Rust/public/generated-model
qualification of this increment is pending.

SYN-022-N08 extends wildcard and identifier-binding patterns to supported whole
fixed values, including packed/unpacked records and finite tagged values.
Binding identity and payload width are checked before the existing typed
assignment path is used; integral constant-pattern and dynamic/native gates
are not weakened. The `n08_whole_patterns` and scope-error public fixtures,
plus snapshot-drop import/lowering tests, are added but unexecuted here.

SYN-001-N03 captures Slang's resolved declaration-order fixed-array elements
whenever type setters are present, including explicit-index/type/default
mixtures. Recursive record/row values and shared operand occurrences are kept;
index-only patterns still retain semantic constant keys. Public local/call/
return/NBA/input-port fixtures and snapshot-drop operand tests are added.
The wrapper/Rust/public pipeline has not been executed for this increment;
existing duplicate-index and uncovered-pattern diagnostics remain controls.

SYN-032-N10 now defers assignment until authorized filesystem/logical map
candidates have been ranked: explicit filename > wildcarded filename > directory
with trailing separator. Same-library repeats are coalesced, higher-rank matches
resolve lower-rank ties, and explicit library assignments override maps. Matched
CLI/buffer sources reuse their admitted text; source count and library metadata
are charged once. The new core resolver and public configured-design fixtures
have not been executed through Rust/Slang here. The N12 source interpretation
is recorded below; Q04's `-incdir` contract and full library acceptance remain open.

### Configuration declarations inside library maps

SYN-032-N12 adopts the **formal-grammar product interpretation** for both target
editions: V Annex A.1.1 and SV Annex A.1.1/33.3.1 include `config_declaration`
in library text. V 13.2.2 and SV 33.3.2 contain narrower nearby prose. That
source inconsistency is retained explicitly; this is the selected implementation
policy, not an IEEE erratum or an assertion that the prose was corrected.

Literal configuration blocks in admitted filesystem and logical maps, including
included maps, now reach the normal Slang configuration parser and binding
engine. Library clauses are handled by the existing authorized Rust map resolver.
Configuration bytes retain their positions in a same-length projection; original
map bytes are restored before diagnostic projection and owned import, preserving
filenames, UTF-16 columns, and source/cache provenance. Separate source-file
configurations remain supported. Malformed blocks and unexpected map tokens
receive direct diagnostics rather than being silently discarded.

The block boundary is lexical: comments, strings, escaped identifiers and named
end clauses do not impersonate `endconfig`. Macro-generated map declarations or
macro-generated block delimiters are not added by this input route. No extra
filesystem discovery, native ABI change or runtime configuration interpreter is
introduced. Existing per-library `-incdir` handling is not generalized; its Q04
contract still requires separate qualification.

`sim_review_tasks12_15` adds public root/included/multiple/separate-source map
cases in both editions and optimizer modes, exact-source configuration-change
and snapshot-teardown checks, and source-position diagnostics. Core tests cover
lexing, bounded projection/publication, source conflicts and original UTF-8
restoration. These Rust/Slang/HDL tests are supplied but not executed here;
N12 and the selected profile are not marked fully accepted.

### Packed conditional pipeline qualification

SYN-020 retains the supplied published-table policy: an ambiguous packed
selector merges Z/Z to X, while a known-true selector must retain a selected Z.
Qualification inspection found a narrower frontend gap in the tracked Slang
conditional patch: `hasUnknown()` also accepted a multi-bit selector containing
a known one. Its merge path could evaluate the unchosen arm and rewrite that
selected Z. The tracked patch now excludes logically true predicates from the
ambiguous branch. It does not change `SVInt` globally or weaken gate truth rules.

The dual-edition `packed_conditional_matrix` adds all 64 scalar state triples
through 129-bit runtime formals and constant generate results, vector logical
truth, exact arm-call counts, common signedness/width and tri-state controls.
Owned folding/identity tests and a literal native mux table supplement it.
The new frontend and public tests have not run here; native helpers alone cannot
close SYN-020. The tracked patch's applied digests change, so an already patched
vendor checkout must be transitioned to the matching complete patch-set state
before rebuilding; a clean documented vendor base uses the normal preparer.

### Direct unpacked-record conditional qualification

SYN-004 preserves the existing runtime `StructMux` and its immediate member
boundaries. Inspection found an additional frontend constant-evaluation cell:
Slang's conditional evaluator merged arrays but defaulted an entire unpacked
structure. The tracked conditional patch now compares each immediate structure
member using logical equality and returns its type's default-uninitialized value
on mismatch. Matching members survive. A differing nested record or row defaults
as one member; declared member initializers are not fallback values. Packed
structures remain integral and use the separate packed table.

The added `struct_conditional_matrix` spans 1/7/33/65/129-bit members, frontend
parameters and constant functions, known/X/Z and dominant-one predicates,
automatic formal/return values, lazy call counts, combinational content changes,
and legal module-storage NBA capture. Owned snapshot-drop and actual-emitter
model tests supplement the public fixture. The frontend, Rust, public HDL and
actual generated-model tests are added but unexecuted here; this is not full
SYN-004 acceptance. Existing direct-record and arrays-of-record controls remain.

### Fixed-array type-key context qualification

SYN-001 continues the N03 resolved-element repair without a second key resolver.
The `type_key_context_matrix` adds 1/7/65/129-bit typedef and signed leaves,
recursive mixed-state records, explicit constant-function/arithmetic indices,
last-type precedence, immediate row-type precedence, unused keys with defaults,
module/automatic initialization, argument/return values, and legal NBA capture.
Packed-structure type keys and index-only semantic keys remain neighboring paths.
Runtime fill values are side-effect free; the tests do not impose an evaluation
count where the assignment-pattern rules leave it undefined.

New duplicate-equivalent-index, uncovered-element, and incompatible-value
fixtures require their specific frontend diagnostic classes. Snapshot-drop
lowering supplements public execution in both optimizer modes. These are added
qualification owners, not executed passes or closure of every SYN-001 context.
The earlier N03 input-port and original pattern suites remain required controls;
no production key-resolution rewrite was justified by this scoped source check.

## Current source status and qualification — 2026-09-23

This inventory records the source status reconciled against the task checkout
on 2026-09-24. The committed source identity, vendored base revisions and
tracked native patches are captured in the local evidence record; the prior
full-suite result remains historical; the current scoped acceptance evidence is
recorded in the SYN-039 section below.

**R01–R05 have bounded source implementations. R06 implements sequential Boolean
clauses, SYN-022 adds integral constant, wildcard and identifier-binding
`matches` patterns with owned lexical storage, SYN-023 adds recursive fixed
packed/unpacked structure patterns with resolved member checks, SYN-024 adds
finite tagged-union discriminant and payload patterns, and SYN-025 adds
primitive/structure/tagged pattern-case items with ordered filters and owned
selector capture. The SYN-038 value-pair denominator is frozen with zero legal
gaps. The composed selected-profile acceptance gate passed on the stated Linux
WSL2 host. This inventory remains a bounded scope description, not a complete synthesizable
Verilog/SystemVerilog claim.**
The numbered feature sections and stable grouped inventory below record the
implemented forms and remaining restrictions, including corrections to older
contradictory rows for `defparam`, `let`, initialization and read-only helper
calls.

### Review-derived continuation repairs

SYN-021-N05 now routes ordinary tagged read and selected-target comparisons
through frame-owned expected-tag and equality-result temporaries. Both are
released after converting the result to a native Boolean; exact tag checking,
source diagnostics and single receiver capture are retained. The nested guard
stress fixture and emitter ownership tests are added, but Rust/public HDL and
actual generated-model sanitizer qualification remain pending. This change does
not itself close the separate valid-member signedness or pattern-case match-mode
contracts; the N01 and N09 follow-ups are described below. SYN-000
identifies the supplied 03-2 baseline only; no new
full-suite green baseline or final selected-profile acceptance is asserted.

SYN-038-N11 corrects ordinary casez matching in the native runtime and owned
constant evaluator: Z in either operand is a wildcard; other states compare
exactly. Constant case pruning uses that same corrected owned operation. The
property-test reference and affected original C vector expectation are corrected,
with independent one-bit, mixed-width, optimizer and dual-edition public cases
added. Runtime-facade checks can establish native behavior; Rust/HDL and final
selected-profile qualification remain pending. The packed conditional policy,
asymmetric wildcard equality/inside rules and N09's separate tagged-pattern
comparator dispatch are unchanged.

### Singleton-concatenation identity repair

SYN-038-N06 now removes a singleton concatenation only when the child keeps
its width, signedness and self-determined fill boundary. Literal-owned fill
markers and `Fill` nodes cannot bypass this guard. The checked-in
`n06_singleton_concat_signed_cast.sv` and optimizer metadata tests cover the
signed four-bit -2 -> unsigned 14 conversion and neighboring fill/width cases.
These Rust/public-HDL regressions have not been run in the delivery environment;
this source repair is not full SYN-015/SYN-038 acceptance.

### Tagged-member signedness repair

SYN-021-N01 now restores the selected member's `sv4_t.is_signed` on the valid
read path, after checked projection and two-state conversion. Wider assignment,
formal and return conversions therefore see the member's declared signedness;
ordinary part-selects remain unsigned. N05's owned comparison temporaries and
exact inactive-tag diagnostics are retained. Direct emitter and public signed/
unsigned, bit/logic and nested/formal/selected-root regressions are supplied;
Rust/public and actual generated-model qualification remain pending.

### Static callback result-flow repair

SYN-011-N02 now keeps normal, explicit-return and lexical break/continue exits
separate when proving static-result independence. An inner loop consumes only
its own jumps, and a do-loop jump cannot reach a later result write. Unknown
control transfers (including unproved named disable) fail closed; loop-header
assignments are not credited as whole-result definitions. Existing conservative
result-read/effect checks and ordinary procedural static storage are unchanged.
Owned-graph tests plus public rejection, ordinary-persistence and eligible-event
controls are added. Their Rust/public execution remains unqualified here.

### Fixed ordering receiver repair

SYN-027-N07 now captures automatic/formal receiver coordinates once for reverse,
sort and rsort using the same typed freezing contract as call copy-back. Reverse
snapshots the selected value; sorting rereads live storage through captured
coordinates after swaps. The collected-array view implementation, element
conversion, bounds and write notifications are retained. New owned-IR selector
checks and public local/input/inout/ref, record/row, singleton, signed-bound and
const-ref controls are supplied. These Rust/public and actual generated-model
ownership checks have not run here; SYN-027/SYN-028 acceptance remains pending.

### Replicated fixed-pattern context qualification

SYN-002 retains the shared per-dimension operand normalizer. It now also rejects
negative signed owned counts before treating their bits as an unsigned extent,
and uses fallible allocation after checked cardinality. Repeated operand slots
remain distinct, including repeated references to the same node. Added owned
unit tests and the `continuation_16_19/replicated_contexts.sv` matrix cover
1/7/65/129-bit leaves, nested three-dimensional rows, packed dimensions, records,
automatic initialization, calls/returns and NBA capture. These Rust/public tests
are supplied but unexecuted in the delivery environment; full SYN-002 acceptance
remains pending. No source side-effect evaluation count is promised for pattern
replication, and replicated lvalues remain illegal.

### Fixed-array membership context qualification

`inside` storage shortcuts now require a declared fixed array and a direct
storage reference, retaining existing real-array storage as a neighboring control.
A packed cast remains one set member; an array cast or value
expression retains its conversion/evaluation before element traversal. Unpacked
records are not silently enumerated as member bags. Captured integral array
leaves restore their native signedness before wildcard equality and common-width
extension, without changing ordinary unsigned part selects or RHS-only wildcard
rules. Tests in `sim_review_tasks16_19` cover signed 7/65/129-bit array values,
function/conditional/row/member receivers, packed and two-state casts, and X
fallback versus a definite wildcard match. Actual-emitter signedness/ownership
regressions are supplied in `owned::tests::inside_values`. Rust, public HDL, and
actual generated-model qualification remain pending; these are not new passes.

### Memory-file conversion and diagnostic qualification

Short binary/hexadecimal words now left-pad a leading X/Z digit with that state;
known digits zero-extend even for signed storage. This follows the selected
memory-file clauses' source-description digit convention together with V 2.5.1
and SV 5.7.1. Two-state conversion occurs before enum width validation, retaining
all original high bits for the numeric range check. The accepted signed enum
redundant-extension and ordinary non-enum truncation paths remain unchanged.
Sparse SV2009 files containing address directives no longer receive the
no-address short-file warning (SV 21.4); V2001's count-warning policy remains, including
excess data written by repeated in-range address directives.

`sim_review_tasks16_19` supplies exact public value/diagnostic cases in the
applicable editions and both optimizer modes. `runtime_value_storage` adds a
native `memory_image` test with original-width enum overflow, short X/Z values
and sparse-file controls, including zero live allocations on return. Execution
results belong to the accompanying delivery record; public Rust/HDL qualification
is still pending. These resolve the specified Q02 short-word and sparse-warning
cells, not every possible malformed-token policy or four-state enum numeric
interpretation. Existing memory view, ordering, range-stop and notification
controls remain required.

### Hierarchical wired-net context qualification

The existing canonical electrical groups and per-source contribution slots are
retained; no second hierarchical driver implementation is introduced.
`sim_review_tasks16_19` adds generated, constant-selected and concatenated driver
sites at widths 1/7/65/129, all 64 three-input four-state combinations, Z removal,
unchanged-resolution event suppression, and upward sibling drives through
homogeneous inout collapse. A private collector test checks independent
continuous-site slots for two wired kinds in two instances after native snapshot
teardown. Illegal procedural net assignments remain rejection controls in both
editions. These new Rust/HDL entries remain unexecuted pending host qualification.
The native collapsed-net probe additionally checks the literal wand/wor tables;
that checks the runtime contract, not the hierarchical source-admission path.

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
§9.9.2 and IEEE 1800-2009 §§7.2, 7.4.2, 7.4.6, 7.7, and 9.2.2.2–9.2.2.4; the
workspace-local section indexes
[`spec-reference-verilog.md`](specification/spec-reference-verilog.md) and
[`spec-reference-sv.md`](specification/spec-reference-sv.md) are optional
review aids, not audit inputs.

### SYN-034 finite metaprogramming and legacy extension profile — 2026-09-21

SYN-034 records the target-policy decision for the six finite metaprogramming
and legacy extension subprofiles named by the implementation plan. A
`Supported` disposition would require a bounded legal witness through the
owned simulator pipeline and concrete acceptance evidence from the selected
synthesis target. The repository has simulator regressions for several of
these constructs, but no checked-in target-tool run, netlist, or equivalent
target acceptance artifact. The exact clause references and exclusion bases
are recorded in this tracked document; the workspace-local clause maps under
`docs/specification/` are optional human review aids, not required inputs to
the checked-in scope decision or any coverage audit.

All six forms are therefore **excluded by the selected SYN-000 profile**. The
exclusions apply to synthesis qualification; existing simulator behavior and
its narrower source-status rows remain documented below. No child repair task
is created because no selected form has a concrete implementation failure.

| Subprofile | Disposition | Language and implementation evidence | Reason and boundary |
| --- | --- | --- | --- |
| Elaboration-only static class methods and constant objects | Excluded by selected profile; no child repair | SystemVerilog-2009 §§8.8–8.9 classify static class properties/methods with the class facility as non-synthesizable in [`spec-reference-sv.md`](specification/spec-reference-sv.md). Class objects and methods have a runtime lowering path in [`collection/classes.rs`](../src/sim/codegen/lowering/collection/classes.rs), while [`semantic.rs`](../src/sim/semantic.rs) classifies every reachable `ClassDef` as `RuntimeObject`. Existing class fixtures are simulator evidence only. | A folded ordinary constant function remains covered by Core elaboration when its source form is selected; that does not admit class/object syntax. Heap lifetime, handles, and virtual dispatch remain outside the synthesis profile. |
| Operator-overloading declarations | Excluded by selected profile; no child repair | SystemVerilog-2009 §11.11 and Annex A.4 identify operator overloading; this tracked profile classifies it `N`. There is no separate owned overload declaration or operation path: [`semantic.rs`](../src/sim/semantic.rs) enumerates ordinary `Operation` values in `is_synthesis_operation`, and the wrapper captures ordinary operator expressions. | Overload binding can change the meaning of an otherwise ordinary operator for non-RTL types. No target acceptance evidence establishes a finite overload subset, so ordinary operator support does not imply overload support. |
| Fixed-input locator results consumed entirely at elaboration | Excluded by selected profile; no child repair | SystemVerilog-2009 §7.12.1 and Annex A.6 cover `find*`, `min`, `max`, and `unique` locator results and classify the array-method family as non-synthesizable. The SYN-034 exclusion is limited to those locator results; SYN-027 and SYN-028 separately select fixed `reverse`/`sort`/`rsort` ordering tasks, with bounded simulator implementation and tests. [`methods.rs`](../src/sim/codegen/lowering/containers/methods.rs) lowers runtime method families, but imported `MethodCall` nodes are treated as `RuntimeService` by the synthesis validator. Fixed reduction witnesses cover `sum`/`product`/`and`/`or`/`xor`, not locators. | A constant input or elaboration-only use does not supply target acceptance for the source-level locator form. The selected profile retains the named fixed reductions and the separately scoped SYN-027/SYN-028 ordering tasks; target acceptance remains a separate qualification. |
| Legacy procedural `assign`/`deassign` and static-initialization `force` | Excluded by selected profile; no child repair | IEEE 1364-2001 §§9.3.1–9.3.2 and SystemVerilog-2009 §10.6 describe procedural continuous assignment and force/release; this tracked profile classifies them `N` or target-dependent. Simulation lowering and regressions exist in [`drivers.rs`](../src/sim/codegen/lowering/statements/drivers.rs) and [`sim_procedural_assign.rs`](../tests/sim_procedural_assign.rs), but [`classify_statement`](../src/sim/semantic.rs) reports `ForceOrProceduralDriver` for these statements. | Constant memory/preload initialization is already recognized as `StorageInitialization` and remains rejected by `PortableRtl`; putting `force` in an initialization process does not change that boundary. No target-specific static-force contract is present. |
| Module-body `specparam` declarations | Excluded by selected profile; no child repair | IEEE 1364-2001 §§3.11.3 and 14.1 and SystemVerilog-2009 §30.3 define `specparam` inside specify behavior; this tracked profile classifies specify parameters as non-synthesizable. The [`slang_c_api.cpp`](../src/wrapper/slang_c_api.cpp) wrapper maps a frontend `Specparam` symbol to parameter metadata, but no specify/path-delay lowering exists; the grouped inventory keeps specify blocks missing. | This decision concerns specify-region timing parameters, not ordinary `parameter` or `localparam` declarations selected by Core elaboration. Metadata capture alone cannot establish executable or target hardware semantics. |
| Finite assertion and monitor synthesis | Excluded by selected profile; no child repair | SystemVerilog-2009 §§16.3, 16.4, and 16.15.1 classify immediate/deferred/concurrent assertions as verification constructs; Verilog-2001 §17.1.3 and SystemVerilog-2009 §21.2.3 classify `$monitor` as simulation output. Bounded assertion and monitor lowering exists in [`assertions.rs`](../src/sim/codegen/lowering/assertions.rs) and [`runtime_tasks.rs`](../src/sim/emit_c/owned/runtime_tasks.rs), while the synthesis validator admits only the small synthesis system-call set and treats monitor services as runtime behavior. | Finite predicates or a bounded monitor do not define a portable hardware mapping. Existing assertion/monitor fixtures prove simulator semantics only; no selected target acceptance establishes an on-chip assertion or monitor profile. |

### SYN-035 fixed-personality PLA target decision — 2026-09-21

SYN-035 requires a concrete legacy synthesis target before selecting any PLA
subfamily. This audit found no named target, target-tool invocation, netlist,
synthesis report, or equivalent acceptance artifact in the tracked repository
or the existing SYN handoffs. The clauses and scope disposition are recorded
in this tracked document. Optional local LRM maps can aid standards review, but
they are not needed to reproduce the profile decision or constitute target
acceptance evidence.

All sixteen Verilog/SystemVerilog array and plane forms remain **excluded by
the selected SYN-000 profile and deferred pending target evidence**:
`$async$and/nand/or/nor$array`, `$async$and/nand/or/nor$plane`, and their
`$sync$` counterparts. This is a synthesis-profile decision; it does not
claim that every synthesis product rejects every finite PLA use, or that the
simulator cannot parse a PLA system-task call.

| Decision area | Evidence and boundary |
| --- | --- |
| Language references | This tracked inventory records Verilog §17.5, SystemVerilog §20.17 and Annex A.7 alongside the sixteen excluded forms. Workspace-local reference maps are optional review aids; these clause references identify the language family, not acceptance by a selected target. |
| Existing implementation | [`editions.rs`](../src/core/compile/editions.rs) admits the names for edition checking. [`system_tasks.rs`](../src/sim/codegen/lowering/statements/system_tasks.rs) has no typed PLA personality or array/plane lowering; the generic system-call branch emits a VPI call. [`semantic.rs`](../src/sim/semantic.rs) admits only `$bits`, `$clog2`, `$signed`, and `$unsigned` as synthesis system calls, so PLA calls are classified as `RuntimeService`. |
| Selected scope | No PLA form is selected and no child implementation or regression is created. The existing ❌ source-status row below remains a simulator implementation gap, while this section records why it is also outside the selected synthesis profile. |
| Reopening gate | Name the target and tool/version, define the fixed personality encoding and dimensions, provide an independently calculable legal witness, and attach target acceptance output. Only then should a bounded asynchronous or synchronous subset be implemented and tested. |

General classes/heap/queues/randomization, testbench infrastructure, full SVA/
coverage/DPI/PLI and arbitrary device-level timing/SDF behavior remain outside
this selected RTL profile.

### Latest supplied validation status

| Evidence | Result | Qualification boundary |
| --- | --- | --- |
| Historical clean-checkout baseline before SYN-038 | **2,353 tests run: 2,353 passed, 1 skipped** | Full `--all-features` nextest on Linux from the isolated pre-ledger worktree at `fb3e4d12`; the skip is `property_elab::gen_c_vectors`. This is historical checkout evidence, not current-branch or clause-by-clause conformance. |
| Superseded user-reported pre-`_03` run | **2,350 tests run: 2,346 passed, 4 failed, 1 skipped** | Historical evidence only. The four reported failures and the subsequent `_02` compile error are not current baseline failures; the current run supersedes them. |
| Recorded native component runs after R01–R06 and the first corrective patch | **50/50 GCC ASan/UBSan; 62/62 unsanitized** | Results come from the supplied validation logs, not a new run. The suites overlap; the sanitizer lane excludes real coroutine stack switching. Handwritten C probes do not validate the complete frontend-to-generated-model path. |
| Recorded operand-slot witness for corrective patch `_02` | **9 checks passed with GCC; 9 with Clang**, both ASan/UBSan | Extracted bridge functions with AST stand-ins check repeated operand positions. This is not a full Slang bridge build or HDL run. |
| Current committed source, 2026-09-24 | **HEAD `179bd76fdd72683cd3b1b9c2056444848c867127`; tree `d3195386ac77e61950bd2cb8d60890f0407063fc`** | Includes frontend `554a343`, simulator repairs and held-out fixtures `ec7032b`, SYN-038 witnesses `97ba0df`, and checker/manifest/ledger `179bd76`. The serialized all-features gate passed on this Linux WSL2 host: 2,756 passed, zero failed, one ignored across 242 targets. SYN-038/SYN-039 selected-profile host acceptance is bounded to the stated scope. |

That recorded baseline established a clean Linux build and full Rust/HDL
nextest result, including the corrected fixed-array conditional, NBA and
default-pattern cases where those suites ran them. Existing regression source
files are not themselves pass evidence. The current SYN-039 scope and command
results are recorded below. Native Windows/macOS execution, the generated-model
ASan boundary and every language/context combination remain separately
unqualified. Commands and test methodology belong in the [test
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
resizable SystemVerilog array.

The fixed-array cell ceiling is below the minimum capacity required by the
supplied language editions. The project profile deliberately keeps this
generated-model resource limit and records the conformance deviation instead
of claiming the standard minimum:

| Boundary | Supplied standard minimum | Lapligence limit | Status |
| --- | ---: | ---: | --- |
| Elements/words in one array | 16,777,216 (2^24) under [IEEE 1364-2001 §3.10](specification/Verilog-1364-2001.pdf), printed p.33 / PDF p.56, and [IEEE 1800-2009 §7.4.2](specification/SystemVerilog-1800-2009.pdf), printed p.104 / PDF p.142 | 65,536 (2^16) total cells per generated fixed unpacked array | **Does not meet the cited minimum**; the project limit is 256 times smaller. |
| One packed value or aggregate payload | The array-element minimum does not remove the separate value-representation requirement. | At most 1,048,575 bits; `LLG_SUPPORTED_WIDTH_LIMIT` is exclusive at 1 << 20 bits. | Values above this limit are rejected in one-payload contexts even when their array cell count is allowed. |
| Cell count and dimension products | Bounds and declared array shapes remain language-level inputs. | Each extent and the product of all extents are checked before storage construction; products above 65,536 are rejected. | Rank/stride arithmetic and aggregate temporary-width limits are separate from the cell-count guard. |
| Simulation work budgets | The cited array clauses do not define a scheduler step budget. | Scheduler and process budgets default to 10,000,000 steps. | Execution policy, not array syntax capacity or evidence of the 2^24-element minimum. |

The selected bounded product profile therefore does **not** claim full array
minimum-capacity conformance for designs requiring more than 65,536 fixed-array
cells. `SYN-036` covers the implemented thresholds and overflow-safe admission;
the held-out `r11_array_capacity_65537.sv` case is owned by
`tests/sim_syn036_capacity.rs::review_bundle_capacity_probe_rejects_the_65537th_cell`.
That public-CLI test uses explicit SV2009, both optimizer modes, empty stdout,
exit code 1, and exact stderr for the 65,537-cell rejection. Its result is
evidence of this disclosed profile limit, not evidence that the standard
minimum is met. No 16,777,216-cell simulation is part of the fast regression
lane. See [`IrArray`](../src/sim/ir/storage.rs),
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
| R04 | Module-procedural fixed-array expressions use type-directed admission and one complete RHS capture before stores or NBA issue. | Bounded integral representations; no new continuous/timed-assignment paths. The corrected nested-default/NBA cases passed in the recorded pre-SYN038 Linux baseline; broader contexts remain unqualified. | [sim_array_conditional_assignments.rs](../tests/sim_array_conditional_assignments.rs) |
| R05 | Directional dissimilar `inout` collapse selects resolver/default/delay on admitted whole, selected/concatenated and fixed net-array-cell connections. | `trireg` and frontend-rejected `uwire` inouts remain excluded; true-alias type rules stay strict; no new whole-array port shapes or net-array declaration-delay capture. | [sim_port_net_types.rs](../tests/sim_port_net_types.rs) |
| R06 | Ordered Boolean `&&&` predicates in `if` and `?:`, with independent branch roles, short-circuiting and full-value truth. | **Partial:** primitive, recursive fixed structure and finite tagged-union `matches` forms are covered by SYN-022–SYN-024, and SYN-025 adds primitive/structure/tagged pattern-case execution. Timing-check `&&&` is a separate missing feature. | [sim_sequential_predicates.rs](../tests/sim_sequential_predicates.rs) |

Implemented scope:

- Fixed unpacked arrays: reversed/negative bounds by logical coordinates,
  overlapping slice self-assignment, array/struct module ports with per-leaf
  notification, memory declaration initialization and slicing.
- Fixed-array conditional values:
  `ArrayMux` retains the immediate unpacked element width and its
  default-uninitialized payload. Ambiguous selectors preserve known-equal
  elements and default entire differing elements, including nested rows and
  fixed integral records; explicit member initializers are not fallback values.
  Packed-vector muxes follow the published conditional table: equal 0, 1, or X
  bits survive an ambiguous selector, while Z/Z and other differing pairs
  produce X. This packed-bit policy remains separate from fixed unpacked
  aggregate merging. The owned emitter captures each evaluated
  alternative once; constant folding follows the same merge rule.
  Regression sources are in `sim_rtl_completion` (`array_conditional_*`), with
  separate native ownership tests. Current acceptance is subject to the
  [validation status](#latest-supplied-validation-status), not inferred from
  fixture presence.
- Direct fixed unpacked-structure conditional values (SYN-004): `StructMux`
  retains each immediate member boundary, including nested fixed row/record
  members, and defaults a differing member to its type's uninitialized value
  under X/Z selectors. Known selectors evaluate one arm; ambiguous selectors
  evaluate both reached arms once. Packed structures retain ordinary mux
  semantics; native/resizable and tagged-union-specific paths remain outside
  this direct mux path.
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
  in source order. SYN-025 adds `case`/`casez`/`casex ... matches` with one
  selector evaluation, source-ordered pattern-local filters, first-match and
  default behavior, unique/priority checks, and primitive/structure bindings.
  SYN-024 adds finite packed tagged-union patterns with void arms, fixed
  primitive/structure payloads, nested tags and automatic payload bindings.
  Pattern discriminants and payload constants inherit the enclosing
  `case`/`casez`/`casex` mode (SV 12.6.1); ordinary checked member access and
  `if ... matches` remain exact. Dynamic/native payloads remain fail-closed.
  The `sim_sequential_predicates` and owned-
  import tests cover these bounded forms in the recorded pre-SYN038 Linux
  baseline. Native
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
  corrected cases passed in the recorded pre-SYN038 Linux baseline; the original NBA fixture
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
- Dynamic/native aggregate fields and unsupported dynamic/native tagged
  payloads retain the restrictions in their owning rows below; SYN-021 covers
  finite packed tagged storage, construction and access, and SYN-024 covers
  finite packed tagged predicate/case patterns.
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
| Array methods and queries | Fixed-array integral reductions use lexical maps and can capture automatic locals/formals (R03); non-direct receivers retain bounded packed payloads, and iterator queries accept `index()`/constant `index(1)` or a checked dynamic integral dimension for the live fixed unpacked iterator. Fixed arrays support `reverse()` over admitted immediate integral or fixed-record elements, and `sort()`/`rsort()` over integral elements or with an integral key selected from a fixed record; selected rows remain whole array elements (SYN-027/SYN-028). Reversed/negative bounds and automatic locals/formals are represented. Reverse/sort with-clause or const-ref cases outside those contracts remain rejected. Packed item and higher/unvisited dimensions remain rejected. Resizable-container callbacks still require packed items and cannot capture automatic locals/formals. Non-packed queue endpoint/pop expressions and several nested scalar-query forms remain rejected. | [fixed reduction lowering](../src/sim/codegen/lowering/containers/fixed_reductions.rs), [fixed-array method lowering](../src/sim/codegen/lowering/containers/methods.rs), [fixed reduction emission](../src/sim/emit_c/owned/fixed_array_reductions.rs), [container callbacks](../src/sim/codegen/lowering/containers/callbacks.rs) |
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

This ledger records the selected Core-elaboration cells from the SYN-019 plan.
Its tracked rows state the editions, clause references, fixtures, outcomes and
owners needed to reproduce the ledger checks. The IEEE 1364-2001 and IEEE
1800-2009 PDFs and workspace-local clause maps under `docs/specification/` may
be consulted as optional review sources; they are untracked and are not
required to run the checks or reproduce this audit. No local errata file was
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

The tracked feature inventory records IEEE 1364-2001 §§8.1, 8.2, 8.6 and IEEE
1800-2009 §§29.3, 29.4, 29.8. Workspace-local specification references are
optional review aids, not prerequisites for running or auditing the checked-in
evidence. The
negative fixtures cover sequential level/edge UDPs (1364 §§8.3–8.5; 1800
§§29.5–29.7) and edge-sensitive table rows. Vector or aggregate terminals,
switch/transistor primitives, and specify/SDF behavior remain outside this
cell.

### SYN-038 selected Core grammar-by-context ledger — 2026-09-23

This ledger is the finite requirement denominator for the selected Core
profiles. Each row has a stable ID, an edition gate, an Annex A/B production or
material semantic variant, a context vector, a checked-in fixture, and an
expected result. `PASS` and `REJECT` describe the required positive or negative
oracle; neither spelling records a run by itself. The `Behavioral owner(s)`
column pairs every checked-in fixture with a Rust regression test, including
the corrected rows. That ownership map is a traceability check; it does not
claim that a test owner ran in the current checkout or proves every
cross-context pair. The v4 pair-value manifest and checker define the value-pair
denominator, link covered pairs to source observations and their exact fixtures,
tests, invocations and oracles, and classify impossible, outside-profile and
open legal-gap pairs. Exact run evidence and source revision belong to SYN-039.
The `PI-03` row is an explicit Extended SYN-031 boundary marker; it does not
promote UDP syntax into the Core denominator.

The production names and clause numbers below are written into these tracked
rows, together with their context, fixtures, oracles and owners. The checker
and coverage audit use the checked-in ledger, manifest, rule bases and
fixtures; they do not require the workspace-local Annex A/B index in
`docs/specification/spec-reference-annex-a.md` or its chapter maps. Those
untracked files may be consulted as optional review aids. The rows use exact
production labels rather than friendly feature names as substitutes for
grammar rows. An `SV2009` gate is intentional whenever a named production or
the checked-in witness uses SystemVerilog-only syntax; a `V2001/SV2009` gate is
reserved for rows whose complete witness remains legal in both editions.

| ID | Edition | Annex A/B production or variant | Context cells | Fixture / check | Expected | Behavioral owner(s) |
| --- | --- | --- | --- | --- | --- |
| SYN038-CORE-LX-01 | V2001/SV2009 | `comment`, `white_space`, `identifier`, `escaped_identifier` (B.1; V §§2.1–2.7, SV §5.6) | lexical; ordinary and escaped identifier declaration/use | `tests/fixtures/sim/review_bundle/r12_escaped_identifier.sv` | PASS | `tests/sim_syn038_ledger.rs::escaped_identifier_grammar_fixture_executes_in_both_editions` |
| SYN038-CORE-LX-02 | SV2009 | `integral_number`, `binary_number`, `octal_number`, `hex_number`, `x_digit`, `z_digit` (B.2; V §2.5.1, SV §5.7.1) | literal; width 1/7/8/32 | `tests/fixtures/sim/feature_completion/g1_10/fill_literal_context.sv` | PASS | `tests/sim_g1_closure.rs::fill_literal_context` |
| SYN038-CORE-LX-03 | SV2009 | `real_number`, `time_number`, `string_literal` in finite constant contexts (B.1/B.2; V §§2.5.2–2.6, SV §§5.7.2–5.9) | constant real/string parameters; time-literal parameter initializer | `tests/fixtures/sim/syn016_elaboration/elaboration_matrix.sv`; `tests/fixtures/sim/review_bundle/r12_time_literal_parameter.sv` | PASS | `tests/sim_syn016_elaboration.rs::elaboration_matrix_matches_both_compilation_unit_policies_and_optimizers`; `tests/sim_syn038_ledger.rs::constant_time_literal_parameter_fixture_executes` |
| SYN038-CORE-LX-04 | V2001/SV2009 | `text_macro_definition`, `text_macro_usage`, `include_compiler_directive`, `conditional_compilation_directive` (B.1/B.18; V §§19.3–19.5, SV §§22.5–22.6) | compilation unit; module | `tests/fixtures/sim/directive_effects/macros_include.sv` | PASS | `tests/sim_directive_effects.rs::macro_include_and_conditional_state_reaches_execution` |
| SYN038-CORE-LX-05 | SV2009 | `attribute_instance`, `pragma`, `pragma_expression` (B.1/B.18; V §2.8, SV §§5.12, 22.11) | module attribute; diagnostic pragma with a simple pragma expression | `tests/fixtures/sim/review_bundle/r12_attribute_pragma.sv` | PASS | `tests/sim_syn038_ledger.rs::attribute_and_pragma_fixture_executes` |
| SYN038-CORE-LX-06 | SV2009 | `timescale_compiler_directive`, `timeunits_declaration`, `default_nettype_compiler_directive` (B.18; V §19.8, SV §§22.7, 22.11) | compilation unit; module | `tests/fixtures/sim/partial_features/time_literal_values_2009.sv` | PASS | `tests/sim_edition.rs::systemverilog_2009_rounds_time_literals_before_value_use` |
| SYN038-CORE-TY-01 | SV2009 | `built_in_data_type`, `integer_vector_type`, `signing` (B.3; V §§3.2–3.3, SV §§6.11, 6.11.3) | scalar/vector; signed/unsigned; 2/4-state | `tests/fixtures/sim/data_types/two_state.sv` | PASS | `tests/sim_data_types.rs::four_to_two_state_128_bits` |
| SYN038-CORE-TY-02 | SV2009 | `net_type`, `variable_type`, `net_port_type` (B.3/B.19; V §§3.2, 3.7–3.8, SV §6.6) | module; net/variable; driver | `tests/fixtures/sim/data_types_next/continuous_assignment_strengths.sv` | PASS | `tests/sim_data_types_next.rs::continuous_assignment_drive_strength_resolution` |
| SYN038-CORE-TY-03 | SV2009 | `packed_dimension`, `unpacked_dimension`, `dimension` (B.3/B.4; V §§3.3, 3.10, SV §§7.4, 7.5) | packed/unpacked; reversed/nonzero bounds | `tests/fixtures/sim/syn012_fixed_layout/matrix.sv` | PASS | `tests/sim_syn012_fixed_layout.rs::fixed_layout_matrix_runs_at_required_widths_in_both_optimizer_modes` |
| SYN038-CORE-TY-04 | SV2009 | `typedef`, `type_declaration`, `enum_name_declaration`, `enum_identifier` (B.3; SV §§6.18–6.19) | type; enum; declaration-order methods; four-state and two-state enum bases | `tests/fixtures/sim/feature_completion/g1_10/enum_navigation_sparse.sv` | PASS | `tests/sim_g1_closure.rs::enum_navigation_sparse` |
| SYN038-CORE-TY-05 | SV2009 | `struct_union`, `struct_union_member` for packed structures (B.3; SV §7.2) | packed struct; member; width 1/7/8/65/129 | `tests/fixtures/sim/data_types_completion/packed_struct_assignment_patterns.sv` | PASS | `tests/sim_data_types_completion.rs::packed_aggregate_assignment_patterns` |
| SYN038-CORE-TY-06 | SV2009 | `struct_union`, `struct_union_member` for equal-width packed unions (B.3; SV §7.3.1) | packed union; selected member; alias view | `tests/fixtures/sim/data_types_completion/packed_union_assignment_patterns.sv` | PASS | `tests/sim_data_types_completion.rs::packed_union_initialization_and_member_writes` |
| SYN038-CORE-TY-07 | SV2009 | `struct_union`, `struct_union_member` for fixed unpacked records (B.3/B.4; SV §7.2) | unpacked record with fixed array member; copy/reference; function return | `tests/fixtures/sim/rtl_completion/fixed_struct_calls.sv` | PASS | `tests/sim_rtl_completion.rs::fixed_structs_preserve_value_copy_and_reference_semantics`; `tests/sim_syn038_ledger.rs::fixed_record_array_member_and_return_context_executes` |
| SYN038-CORE-TY-08 | SV2009 | `array_type`, `array_identifier`, `variable_dimension` (B.4; SV §§7.4–7.5) | 2-D record grid; 3-D fixed record array; 3-D array defaults; arrays of records; reversed/nonzero bounds | `tests/fixtures/sim/syn012_fixed_layout/matrix.sv`; `tests/fixtures/sim/review_bundle/r12_record_grid_3d.sv`; `tests/fixtures/sim/array_conditional_assignments/deep_defaults.sv`; `tests/fixtures/sim/rtl_completion/struct_array_values.sv` | PASS | `tests/sim_syn012_fixed_layout.rs::fixed_layout_matrix_runs_at_required_widths_in_both_optimizer_modes`; `tests/sim_syn038_ledger.rs::fixed_record_grid_executes_across_three_unpack_dimensions`; `tests/sim_array_conditional_assignments.rs::array_conditional_assignment_deep_defaults_preserve_shared_wide_values`; `tests/sim_rtl_completion.rs::fixed_arrays_of_structs_preserve_member_paths_and_formal_shapes` |
| SYN038-CORE-TY-09 | SV2009 | `parameter_declaration`, `local_parameter_declaration`, `parameter_port_declaration` (B.19; V §3.11, SV §6.20) | package parameter; localparam; module parameter port; parameterized instance override; generate | `tests/fixtures/sim/syn016_elaboration/elaboration_matrix.sv` | PASS | `tests/sim_syn016_elaboration.rs::elaboration_matrix_matches_both_compilation_unit_policies_and_optimizers` |
| SYN038-CORE-TY-10 | SV2009 | `const_identifier`, `lifetime`, `static`/`automatic` declarations (B.3; SV §6.20.6–6.21) | module; static local; automatic local | `tests/fixtures/sim/feature_completion/g1_18/static_vs_automatic_loop.sv` | PASS | `tests/sim_varinit.rs::static_local_once_automatic_local_per_activation` |
| SYN038-CORE-TY-11 | SV2009 | `assignment_pattern`, `assignment_pattern_key`, `array_pattern_key`, `structure_pattern_key` (B.4; SV §§7.2, 10.9) | fixed array; record; duplicate type keys; recursive type/default keys; constant expression index keys | `tests/fixtures/sim/p30_fixed_arrays/replicated_assignment_patterns.sv`; `tests/fixtures/sim/review_bundle/r04_duplicate_type_keys.sv`; `tests/fixtures/sim/review_bundle/r04_recursive_type_keys.sv`; `tests/fixtures/sim/review_bundle/r04_unmatched_type_with_default.sv`; `tests/fixtures/sim/review_bundle/r05_constant_expression_index.sv` | PASS | `tests/sim_p30_fixed_arrays.rs::replicated_assignment_patterns_public_cli`; `tests/sim_review_bundle_patterns.rs::fixed_array_type_keys_obey_last_match_recursion_and_default_coverage`; `tests/sim_review_bundle_patterns.rs::constant_expression_array_index_keys_are_evaluated_semantically` |
| SYN038-CORE-TY-12 | SV2009 | `array_query_function`, `array_dimension_function`, `range_function`, `size_function` (B.4; SV §7.4.6) | type; selected row; bounds | `tests/fixtures/sim/feature_completion/g1_27/query_dimension_order.sv` | PASS | `tests/sim_g1_closure.rs::query_dimension_order` |
| SYN038-CORE-EX-01 | SV2009 | `expression`, `primary`, `constant_expression`, `mintypmax_expression` (B.8; V §4.2, SV §§11.5–11.6) | RHS; positive finite real/string constant; package parameter; localparam; declaration | `tests/fixtures/sim/syn016_elaboration/elaboration_matrix.sv` | PASS | `tests/sim_syn016_elaboration.rs::elaboration_matrix_matches_both_compilation_unit_policies_and_optimizers` |
| SYN038-CORE-EX-02 | V2001/SV2009 | `unary_operator`, `binary_operator`, arithmetic operators (B.8; V §§4.1.5–4.1.6, SV §11.4) | scalar/vector; signed; width 7/33/65 | `tests/fixtures/sim/feature_completion/g1_11/arithmetic_small_exhaustive.sv` | PASS | `tests/sim_g1_closure.rs::arithmetic_small_exhaustive` |
| SYN038-CORE-EX-03 | V2001/SV2009 | relational and equality operators in `expression` (B.8; V §4.1.7–4.1.8, SV §11.4) | 4-state; X/Z; conditional | `tests/fixtures/sim/data_types/equality_unknown.v` | PASS | `tests/sim_data_types.rs::known_mismatch_dominates_unknown_equality_128_bits` |
| SYN038-CORE-EX-04 | V2001/SV2009 | logical, bitwise and reduction operators (B.8; V §§4.1.9–4.1.11, SV §11.4) | predicate; RHS; full-value truth | `tests/fixtures/sim/data_types/four_state_truth.v` | PASS | `tests/sim_data_types.rs::four_state_truth_tables_2048_bits` |
| SYN038-CORE-EX-05 | SV2009 | shift and `conditional_expression` (B.8; V §§4.1.12–4.1.13, SV §11.4) | conditional arm; signed; X selector | `tests/fixtures/sim/data_types_extended/shifts_concat_conditional.sv` | PASS | `tests/sim_data_types_extended.rs::shifts_concat_conditional_4096_bits` |
| SYN038-CORE-EX-06 | SV2009 | `concatenation`, `multiple_concatenation` (B.4/B.8; V §4.1.14, SV §11.4) | packed lvalue; replication; overlap | `tests/fixtures/sim/feature_completion/g1_12/overlap_concat_store.sv` | PASS | `tests/sim_feature_completion_g1.rs::overlap_concat_store` |
| SYN038-CORE-EX-07 | SV2009 | `select`, `bit_select`, `part_select_range`, `indexed_range` (B.4; V §4.2.1, SV §11.5) | packed; fixed-array element; runtime index | `tests/fixtures/sim/feature_completion/g1_12/nested_array_runtime_lane.sv` | PASS | `tests/sim_feature_completion_g1.rs::nested_array_runtime_lane` |
| SYN038-CORE-EX-08 | SV2009 | `constant_select`, `constant_bit_select`, `constant_indexed_range` (B.4; V §4.2.1, SV §11.5) | declaration; part/indexed part; bounds | `tests/fixtures/sim/data_type_edges/indexed_part_assignment_context.sv` | PASS | `tests/sim_data_type_edges.rs::indexed_part_assignment_uses_selected_width_and_clips` |
| SYN038-CORE-EX-09 | SV2009 | `cast`, `constant_cast`, `casting_type` (B.8; SV §6.24) | RHS; return; enum; two-state conversion | `tests/fixtures/sim/data_types/casts_conformance.sv` | PASS | `tests/sim_data_types.rs::sized_casts_128_bits` |
| SYN038-CORE-EX-10 | SV2009 | `stream_concatenation`, `stream_expression`, `stream_operator` (B.4; SV §11.4.14) | RHS; fixed source; selected destination | `tests/fixtures/sim/data_types_next/syn_015_fixed_stream_contexts.sv` | PASS | `tests/sim_data_types_next.rs::syn_015_fixed_stream_contexts` |
| SYN038-CORE-EX-11 | SV2009 | `inside_expression`, `open_range_list`, `value_range` (B.8; SV §11.4.13) | predicate; fixed-array value; wildcard X/Z | `tests/fixtures/sim/data_types_next/syn_005_inside_array_values.sv` | PASS | `tests/sim_data_types_next.rs::inside_fixed_array_value_expressions` |
| SYN038-CORE-EX-12 | SV2009 | `assignment_pattern_expression`, `constant_assignment_pattern_expression` (B.4; SV §§7.2, 10.9) | RHS; call; return; NBA; type/default keys; constant expression index keys | `tests/fixtures/sim/p30_fixed_arrays/replicated_assignment_patterns.sv`; `tests/fixtures/sim/review_bundle/r04_duplicate_type_keys.sv`; `tests/fixtures/sim/review_bundle/r04_recursive_type_keys.sv`; `tests/fixtures/sim/review_bundle/r04_unmatched_type_with_default.sv`; `tests/fixtures/sim/review_bundle/r05_constant_expression_index.sv` | PASS | `tests/sim_p30_fixed_arrays.rs::replicated_assignment_patterns_public_cli`; `tests/sim_review_bundle_patterns.rs::fixed_array_type_keys_obey_last_match_recursion_and_default_coverage`; `tests/sim_review_bundle_patterns.rs::constant_expression_array_index_keys_are_evaluated_semantically` |
| SYN038-CORE-AS-01 | SV2009 | `continuous_assign`, `net_assignment`, `net_decl_assignment` (B.7; V §§6.1–6.1.2, SV §10.3) | scalar net declaration assignment and explicit driver; fixed-array continuous assignment | `tests/fixtures/sim/partial_features/net_declaration_propagation.sv`; `tests/fixtures/sim/rtl_completion/syn_006_array_continuous.sv` | PASS | `tests/sim_net_decl.rs::net_declaration_propagation_is_after_resolution_and_not_double_delayed`; `tests/sim_rtl_completion.rs::whole_array_continuous_assignments_keep_sources_cells_and_rhs_snapshots` |
| SYN038-CORE-AS-02 | SV2009 | `blocking_assignment` and `variable_assignment` (B.7; V §6.2, SV §10.4) | whole variable; field; selected element | `tests/fixtures/sim/expression_mutations/expression_mutations.sv` | PASS | `tests/sim_expression_mutations.rs::expression_mutations_preserve_target_capture_and_value_conversions` |
| SYN038-CORE-AS-03 | SV2009 | `nonblocking_assignment` (B.7; V §6.2, SV §10.4) | fixed-array NBA issue/commit; record conditional value, two-state member and NBA destination | `tests/fixtures/sim/array_conditional_assignments/nba.sv`; `tests/fixtures/sim/review_bundle/r12_record_conditional_2state_nba.sv` | PASS | `tests/sim_array_conditional_assignments.rs::array_conditional_assignment_nba`; `tests/sim_syn038_ledger.rs::record_conditional_two_state_nba_context_executes` |
| SYN038-CORE-AS-04 | SV2009 | `operator_assignment`, `inc_or_dec_expression`, `inc_or_dec_operator` (B.7; SV §10.4) | whole/selected lvalue; prefix/postfix increment and decrement; expression-valued assignment; one-time selected index | `tests/fixtures/sim/expression_mutations/expression_mutations.sv`; `tests/fixtures/sim/feature_completion/g1_31/compound_index_once.sv` | PASS | `tests/sim_expression_mutations.rs::expression_mutations_preserve_target_capture_and_value_conversions`; `tests/sim_feature_completion_g1.rs::compound_index_once` |
| SYN038-CORE-AS-05 | SV2009 | `assignment_pattern_variable_lvalue` (B.4; SV §10.9) | positional fixed-array pattern; packed/unpacked structure targets; packed-array target; blocking/NBA | `tests/fixtures/sim/syn003_pattern_lvalues/syn_003_pattern_lvalues.sv`; `tests/fixtures/sim/review_bundle/r06_packed_struct_pattern_lvalue.sv`; `tests/fixtures/sim/review_bundle/r06_unpacked_struct_pattern_lvalue.sv`; `tests/fixtures/sim/review_bundle/r06_packed_array_pattern_lvalue.sv` | PASS | `tests/sim_syn003_pattern_lvalues.rs::positional_pattern_lvalues_run_in_both_optimizer_modes`; `tests/sim_review_bundle_patterns.rs::typed_struct_and_packed_array_pattern_lvalues_deconstruct_positions` |
| SYN038-CORE-AS-06 | SV2009 | `assignment_pattern_net_lvalue` and keyed/default pattern lvalue boundary (B.4) | negative lvalue legality | `tests/fixtures/sim/syn003_pattern_lvalues/syn_003_keyed_lvalue.sv` | REJECT | `tests/sim_syn003_pattern_lvalues.rs::keyed_pattern_lvalue_is_rejected` |
| SYN038-CORE-AS-07 | SV2009 | `array_identifier`, `variable_concatenation`, fixed-array value assignment (B.4/B.7) | whole array; slice; nested row | `tests/fixtures/sim/p30_fixed_arrays/fixed_array_assignment.sv` | PASS | `tests/sim_p30_fixed_arrays.rs::fixed_array_assignment_and_views` |
| SYN038-CORE-AS-08 | SV2009 | replicated `assignment_pattern_expression` (B.4; SV §10.9.1) | nested row; repeated operands; shape | `tests/fixtures/sim/p30_fixed_arrays/replicated_assignment_patterns.sv` | PASS | `tests/sim_p30_fixed_arrays.rs::replicated_assignment_patterns_public_cli` |
| SYN038-CORE-AS-09 | SV2009 | `net_alias`, `list_of_net_assignments` (B.7/B.19; SV §10.11) | net; selected element; true alias | `tests/fixtures/sim/net_resolution/syn_010_fixed_net_aliases.sv` | PASS | `tests/sim_net_resolution.rs::static_net_array_and_packed_alias_projections_keep_shared_connectivity` |
| SYN038-CORE-AS-10 | SV2009 | fixed-array continuous assignment context (B.7; SV §10.3) | net/variable; element wakeup; generate | `tests/fixtures/sim/rtl_completion/syn_006_array_continuous.sv` | PASS | `tests/sim_rtl_completion.rs::whole_array_continuous_assignments_keep_sources_cells_and_rhs_snapshots` |
| SYN038-CORE-PR-01 | V2001/SV2009 | `initial_construct`, `always_construct`, `always_keyword` (B.6; V §§9.2, 9.7, SV §9.2) | module; time-zero; process | `tests/fixtures/sim/loops/syn_037_finite_control_2001.sv` | PASS | `tests/sim_loops.rs::syn_037_verilog_2001_local_disable_preserves_loop_and_copyout` |
| SYN038-CORE-PR-02 | SV2009 | `event_control`, `event_expression`, `procedural_timing_control` (B.6; V §9.7, SV §9.2) | clock/reset; finite event; sensitivity | `tests/fixtures/sim/syn014_process_contexts/process_contexts.sv` | PASS | `tests/sim_syn014_process_contexts.rs::aggregate_sensitivity_and_always_family_contracts_match_in_both_modes` |
| SYN038-CORE-PR-03 | SV2009 | `always_comb`, `always_latch`, `always_ff` (B.6; SV §9.2.2.2–9.2.2.4) | combinational/latch/FF; time-zero | `tests/fixtures/sim/syn014_process_contexts/process_contexts.sv` | PASS | `tests/sim_syn014_process_contexts.rs::aggregate_sensitivity_and_always_family_contracts_match_in_both_modes` |
| SYN038-CORE-PR-04 | V2001/SV2009 | `loop_statement`, `for_initialization`, `for_step`, `repeat`/`while` variants (B.9; V §9.6, SV §§12.7–12.8) | finite loop; bounds; body effects | `tests/fixtures/sim/loops/syn_037_finite_control_2001.sv` | PASS | `tests/sim_loops.rs::syn_037_verilog_2001_local_disable_preserves_loop_and_copyout` |
| SYN038-CORE-PR-05 | SV2009 | `foreach` loop form and `loop_variables` (B.9; SV §12.7.3) | fixed array; mixed dimensions; cleanup | `tests/fixtures/sim/loops/foreach_mixed_control.sv` | PASS | `tests/sim_loops.rs::foreach_mixed_control_keeps_source_loop_jumps_and_signed_endpoints` |
| SYN038-CORE-PR-06 | SV2009 | `do`/`break`/`continue` jump forms (B.9; SV §§12.7.4–12.7.6) | nested lexical loop; finite exit | `tests/fixtures/sim/loops/syn_037_finite_control.sv` | PASS | `tests/sim_loops.rs::syn_037_finite_control_preserves_local_targets_and_copyout` |
| SYN038-CORE-PR-07 | SV2009 | `case_statement`, `case_item`, `case_inside_item` (B.9; V §9.5, SV §12.5) | case/casez/casex; X/Z; inside | `tests/fixtures/sim/feature_completion/g1_21/case_four_state.sv` | PASS | `tests/sim_procedural_control.rs::case_four_state_checks` |
| SYN038-CORE-PR-08 | SV2009 | `unique_priority` and conditional statement (B.9; SV §§12.4–12.5) | priority; diagnostics; predicate | `tests/fixtures/sim/unique_priority/inside.sv` | PASS | `tests/sim_unique_priority.rs::qualified_case_inside_counts_overlapping_membership_groups` |
| SYN038-CORE-PR-09 | V2001/SV2009 | `seq_block`, `disable_statement`, named block scope (B.6/B.9; V §§9.6, 11, SV §12.8) | local named disable; function/task | `tests/fixtures/sim/loops/syn_037_finite_control_2001.sv` | PASS | `tests/sim_loops.rs::syn_037_verilog_2001_local_disable_preserves_loop_and_copyout` |
| SYN038-CORE-PR-10 | V2001/SV2009 | `wait_statement` and finite event-trigger control (B.6/B.9; V §9.7, SV §12.4) | zero-time; finite wait; process | `tests/fixtures/sim/partial_features/wait_constant_false.sv` | PASS | `tests/sim_partial_features/events.rs::constant_false_wait_suspends_without_blocking_time_advance` |
| SYN038-CORE-SB-01 | SV2009 | `function_declaration`, `task_declaration`, `subroutine_call` (B.10; V §§10–11, SV §13) | `$unit` and module declarations/calls; package function call; return value | `tests/fixtures/sim/syn013_zero_time_calls/zero_time_calls.sv`; `tests/fixtures/sim/syn016_elaboration/elaboration_matrix.sv` | PASS | `tests/sim_syn013_zero_time_calls.rs::zero_time_calls_preserve_fixed_values_lifetimes_and_references`; `tests/sim_syn016_elaboration.rs::elaboration_matrix_matches_both_compilation_unit_policies_and_optimizers` |
| SYN038-CORE-SB-02 | SV2009 | `task_port_item`, `function_port_list`, `formal_argument`, `actual_argument` (B.10; V §10, SV §13.5) | input/output/inout; copy-out | `tests/fixtures/sim/function/reference_argument_alias.sv` | PASS | `tests/sim_reference_args.rs::reference_argument_aliases_run_in_both_optimizer_modes` |
| SYN038-CORE-SB-03 | SV2009 | `ref_declaration`, `const` reference formal and actual matching (B.10/B.19; SV §§6.21, 13.5) | ref/const-ref; selected aggregate | `tests/fixtures/sim/feature_completion/g1_17/fixed_ref_alias_visibility.sv` | PASS | `tests/sim_reference_args.rs::fixed_ref_alias_visibility` |
| SYN038-CORE-SB-04 | SV2009 | default formal argument and `list_of_formal_arguments` (B.10; V §10, SV §13.5.4) | omitted/default; side effect once | `tests/fixtures/sim/feature_completion/g1_17/fixed_call_defaults_copyout.sv` | PASS | `tests/sim_function.rs::sim_fixed_call_defaults_copyout` |
| SYN038-CORE-SB-05 | SV2009 | fixed aggregate function result and `function_data_type_or_implicit` (B.10; SV §§6.13, 13.4) | return value; array/record | `tests/fixtures/sim/feature_completion/g1_17/packed_struct_return.sv` | PASS | `tests/sim_function.rs::sim_packed_struct_return` |
| SYN038-CORE-SB-06 | V2001/SV2009 | zero-time `subroutine_call_statement` and timing legality (B.10; V §§10–11, SV §13.4) | initializer; process; no timing | `tests/fixtures/sim/syn013_zero_time_calls/legacy_calls.sv` | PASS | `tests/sim_syn013_zero_time_calls.rs::verilog_2001_zero_time_calls_preserve_automatic_activations` |
| SYN038-CORE-SB-07 | SV2009 | recursive `function_declaration` with finite termination (B.10; V §10.3, SV §13.4) | automatic frame; finite recursion | `tests/fixtures/sim/feature_completion/g1_17/automatic_recursive_function.sv` | PASS | `tests/sim_function.rs::sim_automatic_recursive_function` |
| SYN038-CORE-SB-08 | SV2009 | `lifetime` and output copy-out storage (B.3/B.10; SV §6.21, §13.5) | static/automatic; local/formal | `tests/fixtures/sim/feature_completion/g1_18/static_vs_automatic_loop.sv` | PASS | `tests/sim_varinit.rs::static_local_once_automatic_local_per_activation` |
| SYN038-CORE-HY-01 | SV2009 | `module_declaration`, `module_ansi_header`, `module_nonansi_header` (B.19; V §12.1–12.3, SV §23.2) | top; nested; ANSI/non-ANSI | `tests/fixtures/sim/syn018_module_declarations/module_declarations.sv` | PASS | `tests/sim_syn018_module_declarations.rs::nested_and_extern_modules_execute_in_both_compilation_unit_modes` |
| SYN038-CORE-HY-02 | SV2009 | `module_instantiation`, `ordered_port_connection`, `named_port_connection` (B.19; V §12.1–12.3, SV §23.3) | instance; named/ordered; arrays | `tests/fixtures/sim/feature_completion/g1_22/port_array_aggregate_roundtrip.sv` | PASS | `tests/sim_feature_completion_g1.rs::port_array_aggregate_roundtrip` |
| SYN038-CORE-HY-03 | SV2009 | `parameter_override`, `parameter_value_assignment`, `defparam_assignment` (B.19; V §12.2, SV §§23.10, 6.20) | positional/named instance parameter value assignment; hierarchical defparam; re-elaboration | `tests/fixtures/sim/rtl_completion/defparam_value.sv` | PASS | `tests/sim_rtl_completion.rs::defparam_values_match_parameter_overrides` |
| SYN038-CORE-HY-04 | SV2009 | `generate_region`, `generate_block`, `if_generate_construct`, `generate_case_statement` (B.23; V §12.4, SV §§27.3–27.5) | generate; parameter; hierarchy | `tests/fixtures/sim/feature_completion/g1_34/hier_generate_pipeline.sv` | PASS | `tests/sim_rtl_composition.rs::rtl_composition_hierarchy_generate_pipeline` |
| SYN038-CORE-HY-05 | SV2009 | `loop_generate_construct`, `genvar_declaration`, `genvar_iteration` (B.23; V §12.4, SV §27.4) | generated instances; array | `tests/fixtures/sim/feature_completion/g1_34/hier_generate_pipeline.sv` | PASS | `tests/sim_rtl_composition.rs::rtl_composition_hierarchy_generate_pipeline` |
| SYN038-CORE-HY-06 | SV2009 | `hierarchical_identifier`, `hierarchical_task_identifier`, `hierarchical_function_identifier` (B.19; V §12.6, SV §23.8) | hierarchical cross-instance task/function calls; per-instance child state; parent-scope task dispatch | `tests/fixtures/sim/review_bundle/r12_hierarchical_subroutines.sv` | PASS | `tests/sim_syn038_ledger.rs::hierarchical_task_and_function_calls_execute_per_instance` |
| SYN038-CORE-HY-07 | SV2009 | `package_declaration`, `package_import_declaration`, `package_scope` (B.22; SV §26) | package; `$unit`; qualified type/call | `tests/fixtures/sim/syn016_elaboration/elaboration_matrix.sv` | PASS | `tests/sim_syn016_elaboration.rs::elaboration_matrix_matches_both_compilation_unit_policies_and_optimizers` |
| SYN038-CORE-HY-08 | SV2009 | `interface_declaration`, `interface_instantiation`, `modport_declaration` (B.21; SV §§25.3, 25.5) | interface; modport; instance | `tests/fixtures/sim/feature_completion/g1_34/interface_modport_struct.sv` | PASS | `tests/sim_rtl_composition.rs::rtl_composition_interface_modport_struct` |
| SYN038-CORE-HY-09 | SV2009 | `input_declaration`, `output_declaration`, `inout_declaration`, fixed aggregate port variants (B.19; SV §§23.2.2, 23.3) | input/output/ref; whole/row/slice | `tests/fixtures/sim/rtl_completion/syn_008_port_shape_matrix.sv` | PASS | `tests/sim_rtl_completion.rs::fixed_output_ref_and_aggregate_port_shapes_preserve_storage` |
| SYN038-CORE-HY-10 | SV2009 | `module_declaration` for nested and extern module forms (B.19; SV §§23.2, 23.4–23.5) | extern body; nested scope | `tests/fixtures/sim/syn018_module_declarations/extern_child.sv` | PASS | `tests/sim_syn018_module_declarations.rs::nested_and_extern_modules_execute_in_both_compilation_unit_modes` |
| SYN038-CORE-ED-01 | V2001 | legacy `module_declaration`, continuous assignment and `always @*` forms (B.6/B.7/B.19) | edition gate; public CLI | `tests/fixtures/sim/partial_features/edition_legacy_forms.sv` | PASS | `tests/sim_edition.rs::selected_edition_matrix_accepts_legacy_and_systemverilog_cli_forms` |
| SYN038-CORE-ED-02 | V2001 | SV-only `typedef`, `struct`, whole-array and assignment-pattern forms rejected by edition policy (B.3/B.4) | negative edition gate | `tests/fixtures/sim/partial_features/edition_2001_sv_only.sv` | REJECT | `tests/sim_edition.rs::verilog_2001_rejects_systemverilog_constructs` |
| SYN038-CORE-ED-03 | V2001 | SV-only fixed-array value/port forms rejected while indexed memories remain legal (B.4/B.19) | negative edition gate; storage | `tests/fixtures/sim/partial_features/edition_2001_whole_array_assignment.sv` | REJECT | `tests/sim_edition.rs::selected_edition_matrix_rejects_single_fault_2001_and_later_builtin_fixtures` |
| SYN038-CORE-ED-04 | SV2009 | `logic`, `typedef`, packed `struct`, assignment pattern and `always_comb` accepted (B.3/B.4/B.6) | edition gate; type/process | `tests/fixtures/sim/partial_features/edition_2009_sv_types.sv` | PASS | `tests/sim_edition.rs::selected_edition_matrix_accepts_legacy_and_systemverilog_cli_forms` |
| SYN038-CORE-ED-05 | SV2009 | `keywords_directive` changes lexical keywords without changing selected edition (B.1/B.18) | preprocessor; edition | `tests/fixtures/sim/partial_features/edition_begin_keywords_legacy_identifier.sv` | PASS | `tests/sim_edition.rs::selected_edition_matrix_accepts_legacy_and_systemverilog_cli_forms` |
| SYN038-CORE-ED-06 | SV2009 | later `countbits`/final/assertcontrol forms remain outside the selected 2009 set (B.17) | negative later-edition gate | `tests/fixtures/sim/partial_features/edition_2009_countbits.sv` | REJECT | `tests/sim_edition.rs::selected_edition_matrix_rejects_single_fault_2001_and_later_builtin_fixtures` |
| SYN038-CORE-PI-01 | V2001/SV2009 | `gate_instantiation`, `n_input_gate_instance`, `enable_gate_instance` (B.24; V §§7.1–7.4, SV §§28.3–28.6) | structural; terminal; driver | `tests/fixtures/sim/gates/terminal_matrix.sv` | PASS | `tests/sim_gates.rs::gate_terminal_matrix`; `tests/sim_gates.rs::gate_terminal_matrix_verilog_2001` |
| SYN038-CORE-PI-02 | V2001/SV2009 | `drive_strength`, `pullup_strength`, `pulldown_strength` (B.24; V §§3.4, 7.1.2, SV §28.11) | scalar net; resolved driver | `tests/fixtures/sim/gates/pull_undriven.v` | PASS | `tests/sim_gates.rs::sim_gates_pullup_pulldown_drive_undriven_wire` |
| SYN038-CORE-PI-03 | V2001/SV2009 | `udp_declaration`/`combinational_body` are tracked as selected Extended SYN-031, not Core syntax | extension boundary; source map | `tests/fixtures/sim/partial_features/udp_comb.sv` | PASS | `tests/sim_udp.rs::combinational_udp_truth_table_and_drivers` |
| SYN038-CORE-PI-04 | V2001/SV2009 | `sequential_body`, `edge_input_list` UDP forms remain outside Core (B.25; V §§8.3–8.5, SV §§29.5–29.7) | negative extension boundary | `tests/fixtures/sim/partial_features/udp_sequential_rejected.sv` | REJECT | `tests/sim_udp.rs::sequential_udp_remains_rejected` |

The 78 Core grammar rows assign productions and material variants. The
separate v4 value-pair catalog uses 13 focal-value/context factors across all
78 factor-pair blocks. Its frozen checker result is **2,247 raw / 1,849
selected / 1,849 covered / 0 legal gaps / 296 impossible / 102 outside-profile**.
The checked-in manifest requires zero legal gaps. The final checker SHA-256 is
`b1ccaad76c1488195b55923ce5391c748cfe94cb6879b61215064ace4d43ea6e`; the
manifest SHA-256 is
`941e2f1ab12064cfa820686e36ba2e29bbc8a418940c26e265f3d1edfe460757`.
The checker and ledger reject a selected legal gap; a mutation that removes a
closing witness also fails the checker.

Audited W91–W96 add 54 selected-pair gains across O503–O548; W97 adds three;
W98 adds 21; W99–W102 add nine; W103 adds ten; and W104 adds seven after one
overlap. Later source-bound witnesses cover written fixed-array reduction
receivers, event lvalues and drivers, same-slot NBA event controls, return and
initializer consumers, port and task/function input paths, selected input
actuals, const-ref actuals, and blocking/NBA ref/inout actuals. W119 adds five
selected-input paths, W120 seven const-ref prior-source paths, W121 four
blocking ref/inout paths, W122 two NBA ref/inout paths, and W123 one read-only
ref actual to a continuously driven variable. Each fixture, Rust owner, public
CLI mode, and exact oracle is listed below. The SYN-038/SYN-039 selected-profile
host acceptance status is recorded with the composed gates described later.

The auditor also accepted 23 legacy focal corrections:
O03/O11/O12/O18/O29/O32/O38/O44/O45/O47/O48/O50/O51/O52/O53/O65/O66/O103/O119
and O424–O427. These rows describe the receiving target, so their source-side
`CP` is `none` and their target operation is `direct_projection`; a distinct
function result is not used to label the target. The source-side replacements
are checked separately by W103/W104. A further 18 target-address observations
(W79 O329–O342 and W103 O582–O585) omit `CO`: their conditional/equality/cast/
pattern operation selects the LHS address, while a separate value supplies
the assignment RHS. O332/O336 also omit `TY` because their outer unpacked
arrays of packed records have no matching outer type level in this catalog.

The focal auditor accepted W73 O243–O249 as literal same-root continuous
assignment RHS sources. Slang binding identities and disjoint bit projections
establish that the focal source is read from the same declared object as the
continuous target. The wider assignment-RHS review retains `CO=assignment_rhs`
only where the same focal slot is actually read on that assignment RHS. Of the
210 target-adjacent rows reviewed, 16 retain `CO` (O69, O109, O114, O243–O249,
O418, O495, O586, O591, O592 and O594); 176 become `CO`-sparse, while the 18
W79/W103 address-only rows were already sparse. W99 O573–O575 and W75 O271–O275
describe writes whose RHS is a separate source, so those target observations do
not claim an assignment-RHS consumer.

The accepted focal-write and read-site applicability basis allows an explicit
write to a focal source slot to be paired with a later read at a distinct use
site, including event, reduction, return, port, task/function, const-ref,
inout, and ref consumers when the source identity and write shape support the
path. `SYN038-R-FOCAL-WRITE`, `SYN038-R-READ-SITE`,
`SYN038-R-REDUCTION-READ`, `SYN038-R-ACTUAL-ADDRESS`,
`SYN038-R-REF-ACTUAL-VARIABLE`, and the continuous-storage rules encode those
boundaries. All 32 newly applicable cells have individual rule pins and
covering observations; the inout × continuous-variable cell remains explicitly
impossible. W105's event-control witness and W106's same-root assignment-RHS
witnesses now have stable manifest mappings. The W106 rows are O597–O601, and
its checked pair closures are
`CO-assignment_rhs__LV-field` and
`CO-assignment_rhs__LV-positional_pattern`.

The two narrow W96 rules remain active. `SYN038-R-CONST-REF-SOURCE-OP` makes
the four direct conditional/equality/inside/cast/assignment-pattern expression
actuals impossible for `FM=const_ref`, while index-only operators leave the
focal source as a direct variable projection. `SYN038-R-RUNTIME-DECL-SCOPE`
makes only `HC=subroutine × IN=runtime_declaration` impossible; module/interface
runtime initializers remain distinct from local subroutine declarations. See
the rule definitions and exact negative/positive oracles below. These bounded
rules establish the audited denominator; the overall SYN-038/SYN-039 product
acceptance still requires the composed run gates.

| Factor | Semantic distinction | Finite levels in the v4 checker |
| --- | --- | --- |
| `TY` | Outer declared type of the focal slot before member or element projection; width and leaf state are qualifiers; `tagged_extended` is outside Core. | `integral_bit_logic`; `enum`; `packed_struct`; `untagged_packed_union`; `fixed_array_integral`; `fixed_array_record`; `unpacked_record`; `tagged_extended` |
| `OP` | Operation on a focal source or direct target projection; equality/inside results may have a different type from the source. | `direct_projection`; `conditional`; `equality_inside`; `cast_stream`; `assignment_pattern` |
| `CO` | Selected Core use site consuming the focal value or operation result; `assignment_rhs` requires a read of that same focal slot on the assignment RHS. `call_argument` means a user-defined task/function actual; system-task oracle arguments such as `$display` are outside this lane. | `assignment_rhs`; `port_actual`; `call_argument`; `function_return_statement`; `declaration_initializer`; `constant_elaboration`; `event_expression` |
| `LV` | Address form only when the focal slot is a write target; `none` means read-only. | `none`; `whole_object`; `field`; `element`; `row_slice`; `concatenation`; `positional_pattern` |
| `SL` | Storage of the focal source or target, independently of formal direction; `interface_member` means a mutable interface signal, not a parameter. | `module_package`; `static_local`; `automatic_local`; `formal`; `return_slot`; `interface_member` |
| `FM` | Direction of a formal receiving a port actual or subroutine argument, independent of actual storage. | `none`; `input`; `output`; `inout`; `ref`; `const_ref` |
| `HC` | Lexical caller or use-site scope, independent of the referenced target route. | `module`; `subroutine`; `generate`; `interface` |
| `HR` | Selected route from `HC`, in priority order: qualified identifier or task/function callee → `hierarchical_identifier`; otherwise concrete interface data member → `interface_member`; otherwise instantiated child link → `child_port`; remaining unqualified access → `local`. | `local`; `child_port`; `hierarchical_identifier`; `interface_member` |
| `CP` | Source-side function result or fixed-array reduction receiver; a reduction reads its receiver. | `none`; `function`; `fixed_array_reduction` |
| `CT` | Receiving user-defined subroutine kind for a Core call-argument actual; system-task oracles are outside this lane, and tasks have no expression return. | `none`; `function`; `task` |
| `IN` | Initializer kind at the consumer site; `memory_image` is a separate Extended lane. | `none`; `constant_declaration`; `automatic_local`; `static_local`; `runtime_declaration`; `memory_image` |
| `WK` | Explicit assignment form writing the focal slot, potentially before a separate `CO` read of that same slot; `none` covers source-only and implicit copy paths. | `none`; `continuous_net`; `continuous_variable`; `procedural_blocking`; `procedural_nba` |
| `PC` | Process executing the path; `none` covers continuous, port-link and compile-time paths. | `none`; `initial`; `always`; `always_comb`; `always_latch`; `always_ff` |

Under IEEE 1800-2009 §6.20.2, parameter/localparam constant expressions may
not use an instance-hierarchical name, directly, through a constant function,
or in a constant query such as `$bits`. The checker records this as
`SYN038-R-HIERARCHICAL-CONSTANT` and classifies
`SYN038-PW-CO-constant_elaboration__HR-hierarchical_identifier` and
`SYN038-PW-HR-hierarchical_identifier__IN-constant_declaration` as impossible
(`SYN038-IAP-CO-constant_elaboration__HR-hierarchical_identifier` and
`SYN038-IAP-HR-hierarchical_identifier__IN-constant_declaration`). Package
references such as `source_pkg::LIMIT` and package constant functions remain
lexical package-scope paths, not hierarchical routes. Runtime declaration
initialization from `u.LIMIT` remains selected and is covered by W78. See
[IEEE 1800-2009 §6.20.2](specification/SystemVerilog-1800-2009.pdf).

`SYN038-R-PRODUCER-FORMAL-ACTUAL` applies IEEE 1800-2009 §13.5.2 to the same
focal expression used directly as a subroutine actual. Function results cannot
serve as `output` or `inout` assignment targets or as `ref`/`const ref` variable
actuals; fixed-array reduction results likewise cannot be `const ref` actuals.
This moves exactly five pairs to impossible: `FM=output/inout/ref/const_ref ×
CP=function` and `FM=const_ref × CP=fixed_array_reduction`. Input actuals remain
expression-valued. Copying a result into a separate variable creates a new
focal slot and can provide a valid reference actual; the copied-variable
positive control and direct SV2009 rejection probe support the boundary. The
rule does not reclassify `SYN038-PW-LV-row_slice__FM-ref`: a packed part-select
bound to a module `ref` port is legal and W90/O502 covers that selected cell.
See [IEEE 1800-2009 §13.5.2](specification/SystemVerilog-1800-2009.pdf).

Two further narrow applicability rules were independently accepted at the W96
denominator boundary and remain applied in the later mappings.
`SYN038-R-CONST-REF-SOURCE-OP` classifies exactly the four
`OP=conditional/equality_inside/cast_stream/assignment_pattern × FM=const_ref`
pairs as impossible: each operation result is a temporary, while a const-ref
actual must denote an eligible variable. An operation used only to select an
element in a read-only actual's index does not change the focal source
operation from `direct_projection`; those selected-variable controls remain
legal. `SYN038-R-RUNTIME-DECL-SCOPE` classifies only
`HC=subroutine × IN=runtime_declaration` as impossible. A runtime declaration
initializer is a module- or interface-scope declaration site; a declaration
inside a subroutine is `automatic_local` or `static_local`, even when its
initializer calls a function. The rule follows IEEE 1800-2009 §§6.8 and 6.21;
W82/O379 records the distinct module runtime-initializer path from an automatic
function local. The corresponding five applicability IDs are
`SYN038-IAP-OP-conditional__FM-const_ref`,
`SYN038-IAP-OP-equality_inside__FM-const_ref`,
`SYN038-IAP-OP-cast_stream__FM-const_ref`,
`SYN038-IAP-OP-assignment_pattern__FM-const_ref`, and
`SYN038-IAP-HC-subroutine__IN-runtime_declaration`.
The const-ref source-operation witnesses are
`tests/fixtures/sim/syn038_pairwise/constref_conditional_rejected.sv`,
`tests/fixtures/sim/syn038_pairwise/constref_equality_rejected.sv`,
`tests/fixtures/sim/syn038_pairwise/constref_cast_rejected.sv`, and
`tests/fixtures/sim/syn038_pairwise/constref_pattern_rejected.sv`; the positive
selected-element controls and exact stderr checks are owned by
`tests/sim_syn038_operation_context_matrix.rs::typed_operation_contexts_keep_source_and_use_site_in_both_cli_modes`.
Each negative case runs in both SV2009 optimizer modes and requires empty
stdout, exit code 1, and Slang's exact “invalid expression for pass by
reference; only variables, class properties, and members of unpacked structs
and arrays are allowed” diagnostic.

Edition, compilation-unit policy, optimizer mode and compile-time/runtime
selection are validation qualifiers attached to witnesses. The native snapshot
drop probe is source-level owned-database evidence; it does not claim public
CLI coverage. The selected three-way composition chains remain separate from
the pairwise denominator and are listed in the evidence map below.

#### SYN-038 audited evidence map

Each named Rust integration test checks the listed stdout and stderr oracle.
The shared `sim_cli` helper runs both optimization modes, compares stdout
exactly, compares runtime/lowering stderr after discarding the frontend's
`Warning: ` banner, and requires the exact expected lowering-warning set. The
tests below expect no lowering warnings. Commands that select an edition
explicitly list that argument in the invocation cell. W13 and W21–W23 use the
documented SystemVerilog-2009 CLI default and pass no `--edition` argument.
W13's expected stdout is computed from independently specified type and width
metadata, then compared byte-for-byte by the public CLI helper. Edition lists
are stated where one fixture is run under both editions.

| ID | Context cell | Fixture | Owning test | CLI invocation | Expected oracle |
| --- | --- | --- | --- | --- | --- |
| SYN038-CORE-LX-01 | Ordinary and escaped identifiers in declarations and uses | `tests/fixtures/sim/review_bundle/r12_escaped_identifier.sv` | `tests/sim_syn038_ledger.rs::escaped_identifier_grammar_fixture_executes_in_both_editions` | `review_bundle/r12_escaped_identifier`; `--edition 2001`; `--edition 2009` | stdout `dot=1 under=0 dash=1\n`; stderr `""` |
| SYN038-CORE-LX-03 | Real/string parameter elaboration | `tests/fixtures/sim/syn016_elaboration/elaboration_matrix.sv` | `tests/sim_syn016_elaboration.rs::elaboration_matrix_matches_both_compilation_unit_policies_and_optimizers` | `syn016_elaboration/elaboration_matrix`; `--edition 2009`; `--compilation-units separate`; `--compilation-units merged` | stdout `child=4 label=syn016 dim=4\n` `child=8 label=syn016 dim=4\n` `out=5/d5 bits=4/8 base=3 bump=5 twice=6\n`; stderr `llg: $finish at time 1000 at tb:84:5\n` |
| SYN038-CORE-LX-03 | Unit-suffixed time literal in a constant parameter initializer | `tests/fixtures/sim/review_bundle/r12_time_literal_parameter.sv` | `tests/sim_syn038_ledger.rs::constant_time_literal_parameter_fixture_executes` | `review_bundle/r12_time_literal_parameter`; `--edition 2009` | stdout `param=2\n`; stderr `""` |
| SYN038-CORE-LX-05 | Module attribute and diagnostic pragma expression | `tests/fixtures/sim/review_bundle/r12_attribute_pragma.sv` | `tests/sim_syn038_ledger.rs::attribute_and_pragma_fixture_executes` | `review_bundle/r12_attribute_pragma`; `--edition 2009` | stdout `attribute_pragma=1\n`; stderr `""` |
| SYN038-CORE-TY-07 | Fixed unpacked record with array member, copy/reference argument and function return | `tests/fixtures/sim/rtl_completion/fixed_struct_calls.sv` | `tests/sim_syn038_ledger.rs::fixed_record_array_member_and_return_context_executes` | `rtl_completion/fixed_struct_calls`; `--edition 2009` | stdout `source=2,9,2,3\nresult=f,1,5,3\n`; stderr `""` |
| SYN038-CORE-TY-08 | Three-dimensional fixed array of records and member selection | `tests/fixtures/sim/review_bundle/r12_record_grid_3d.sv` | `tests/sim_syn038_ledger.rs::fixed_record_grid_executes_across_three_unpack_dimensions` | `review_bundle/r12_record_grid_3d`; `--edition 2009` | stdout `record_grid3d=7,56 flags=0,1\n`; stderr `llg: $finish at time 0 at tb:21:5\n` |
| SYN038-CORE-SB-05 | Packed-structure function result and assignment to packed destinations | `tests/fixtures/sim/feature_completion/g1_17/packed_struct_return.sv` | `tests/sim_function.rs::sim_packed_struct_return` | `feature_completion/g1_17/packed_struct_return`; `--edition 2009` | stdout `swap=3412 widen=13cb src=1234\n`; stderr `""` |
| SYN038-CORE-AS-01 | Scalar net declaration assignment and explicit continuous driver | `tests/fixtures/sim/partial_features/net_declaration_propagation.sv` | `tests/sim_net_decl.rs::net_declaration_propagation_is_after_resolution_and_not_double_delayed` | `partial_features/net_declaration_propagation`; `--edition 2009` | stdout `t2 z z x z x\nt3 1 1 1 x x\n`; stderr `""` |
| SYN038-CORE-AS-01 | Whole and selected fixed-array continuous assignment | `tests/fixtures/sim/rtl_completion/syn_006_array_continuous.sv` | `tests/sim_rtl_completion.rs::whole_array_continuous_assignments_keep_sources_cells_and_rhs_snapshots` | `rtl_completion/syn_006_array_continuous`; `--edition 2009` | stdout `t1 net=a1,x2 var=a1,a2 cond=a1,xx\n` + `t1 pattern=c1,c2 selected=zz,a2 split=a1,b2 bit=01,02 row=31,32 ` + `func=e1,e3 calls=1\n` + `t2 net=x1,x2 var=a1,d2 cond=a1,d2\n` + `t2 pattern=c1,c2 selected=zz,d2 split=a1,c2 bit=03,02 row=41,32 ` + `func=e1,e4 calls=2\n`; stderr `""` |
| SYN038-CORE-AS-03 | Fixed-array NBA issue/commit and selected-target capture | `tests/fixtures/sim/array_conditional_assignments/nba.sv` | `tests/sim_array_conditional_assignments.rs::array_conditional_assignment_nba` | `array_conditional_assignments/nba`; `--edition 2009` | stdout `nba=33,ee row=xx,5a\n`; stderr `""` |
| SYN038-CORE-AS-03 | Conditional record value with a two-state member assigned by NBA | `tests/fixtures/sim/review_bundle/r12_record_conditional_2state_nba.sv` | `tests/sim_syn038_ledger.rs::record_conditional_two_state_nba_context_executes` | `review_bundle/r12_record_conditional_2state_nba`; `--edition 2009` | stdout `record_nba=xx,0\n`; stderr `""`; the owner also checks pre-NBA storage and two-state member conversion |
| SYN038-CORE-HY-06 | Hierarchical task/function identifiers, per-instance state and parent dispatch | `tests/fixtures/sim/review_bundle/r12_hierarchical_subroutines.sv` | `tests/sim_syn038_ledger.rs::hierarchical_task_and_function_calls_execute_per_instance` | `review_bundle/r12_hierarchical_subroutines`; `--edition 2009` | stdout `hier=6,2,12,6,25\n`; stderr `""` |
| SYN038-PAIR-01 | Declaration/type × shape: unpacked record array passed to a function formal | `tests/fixtures/sim/rtl_completion/struct_array_values.sv` | `tests/sim_rtl_completion.rs::fixed_arrays_of_structs_preserve_member_paths_and_formal_shapes` | `rtl_completion/struct_array_values`; `--edition 2009` | stdout `sum=16 data=5a,a5\n`; stderr `""` |
| SYN038-PAIR-02 | Width × signedness × state domain in conditional array values | `tests/fixtures/sim/rtl_completion/array_conditional_wide.sv` | `tests/sim_rtl_completion.rs::array_conditional_wide_elements_preserve_state_domains` | `rtl_completion/array_conditional_wide`; `--edition 2009` | stdout `array conditional wide states passed\n`; stderr `""` |
| SYN038-PAIR-03 | Storage lifetime × local destination | `tests/fixtures/sim/rtl_completion/fixed_block_locals.sv` | `tests/sim_rtl_completion.rs::fixed_block_locals_observe_automatic_and_static_lifetimes` | `rtl_completion/fixed_block_locals`; `--edition 2009` | stdout `local=26 saved=11\nlocal=26 saved=12\n`; stderr `""` |
| SYN038-PAIR-04 | Driver/process × fixed-array shape and RHS snapshot | `tests/fixtures/sim/rtl_completion/syn_006_array_continuous.sv` | `tests/sim_rtl_completion.rs::whole_array_continuous_assignments_keep_sources_cells_and_rhs_snapshots` | `rtl_completion/syn_006_array_continuous`; `--edition 2009` | stdout `t1 net=a1,x2 var=a1,a2 cond=a1,xx\n` + `t1 pattern=c1,c2 selected=zz,a2 split=a1,b2 bit=01,02 row=31,32 ` + `func=e1,e3 calls=1\n` + `t2 net=x1,x2 var=a1,d2 cond=a1,d2\n` + `t2 pattern=c1,c2 selected=zz,d2 split=a1,c2 bit=03,02 row=41,32 ` + `func=e1,e4 calls=2\n`; stderr `""` |
| SYN038-PAIR-05 | Elaboration × hierarchy × parameterized type | `tests/fixtures/sim/syn016_elaboration/elaboration_matrix.sv` | `tests/sim_syn016_elaboration.rs::elaboration_matrix_matches_both_compilation_unit_policies_and_optimizers` | `syn016_elaboration/elaboration_matrix`; `--edition 2009`; `--compilation-units separate`; `--compilation-units merged` | stdout `child=4 label=syn016 dim=4\n` `child=8 label=syn016 dim=4\n` `out=5/d5 bits=4/8 base=3 bump=5 twice=6\n`; stderr `llg: $finish at time 1000 at tb:84:5\n` |
| SYN038-PAIR-06 | Function call in runtime declaration initializer before the initial process | `tests/fixtures/sim/feature_completion/g1_18/initializer_function_before_initial.sv` | `tests/sim_varinit.rs::initializer_function_before_initial` | `feature_completion/g1_18/initializer_function_before_initial`; `--edition 2009` | stdout `x=42 observed=41\n`; stderr `""` |
| SYN038-PAIR-07 | Module-local process reads and writes members of concrete interface instances | `tests/fixtures/sim/syn016_elaboration/elaboration_matrix.sv` | `tests/sim_syn016_elaboration.rs::elaboration_matrix_matches_both_compilation_unit_policies_and_optimizers` | `syn016_elaboration/elaboration_matrix`; `--edition 2009`; `--compilation-units separate`; `--compilation-units merged` | stdout `child=4 label=syn016 dim=4\n` `child=8 label=syn016 dim=4\n` `out=5/d5 bits=4/8 base=3 bump=5 twice=6\n`; stderr `llg: $finish at time 1000 at tb:84:5\n` |
| SYN038-3WAY-01 | Fixed-array value + function return + input port | `tests/fixtures/sim/rtl_completion/syn_007_array_input_values.sv` | `tests/sim_rtl_completion.rs::fixed_array_input_ports_capture_values_and_runtime_rows` | `rtl_completion/syn_007_array_input_values`; `--edition 2009` | stdout `t1 cond=a1,b1 func=12,22 selected=33,43 slice=14,24 pattern=17,39 reverse=5b,6c calls=1\n` `t2 cond=11,30 func=12,31 selected=e3,d3 slice=14,24 pattern=17,39 reverse=5b,6c calls=2\n`; stderr `""` |
| SYN038-3WAY-02 | Record conditional + two-state member + NBA destination | `tests/fixtures/sim/review_bundle/r12_record_conditional_2state_nba.sv` | `tests/sim_syn038_ledger.rs::record_conditional_two_state_nba_context_executes` | `review_bundle/r12_record_conditional_2state_nba`; `--edition 2009` | stdout `record_nba=xx,0\n`; stderr `""`; the owner also checks pre-NBA storage and two-state member conversion |
| SYN038-3WAY-03 | Fixed-array map + function formal + evaluated event expression | `tests/fixtures/sim/syn011_rtl_helper_events/fixed_array.sv` | `tests/sim_partial_features/evaluated_events.rs::static_helpers_keep_fixed_array_and_qualified_values_private` | `syn011_rtl_helper_events/fixed_array`; `--edition 2009` | stdout `fixed_array changes=2 value=9\n`; stderr `""` |
| SYN038-3WAY-04 | True alias + selected inout port + distinct driver sites | `tests/fixtures/sim/rtl_completion/inout_composition.sv` | `tests/sim_rtl_completion.rs::inout_arrays_nested_peers_and_selected_ports_share_resolution` | `rtl_completion/inout_composition`; `--edition 2009` | stdout `lane=5a sibling=zz bus=z5a5\nlane=a5 sibling=zz bus=zzz5\n`; stderr `""` |
| SYN038-W10 | Declaration initializer scheduling and process interaction in both selected editions | `tests/fixtures/sim/partial_features/declaration_init_edition.sv` | `tests/sim_edition.rs::declaration_initialization_keeps_edition_specific_scheduling` | `partial_features/declaration_init_edition`; `--edition 2001`; `--edition 2009` | stdout `PASS declaration_init_edition\n`; stderr `llg: $finish at time 0 at tb:15:9\n` |
| SYN038-W12 | Unpacked-structure positional pattern lvalue | `tests/fixtures/sim/review_bundle/r06_unpacked_struct_pattern_lvalue.sv` | `tests/sim_review_bundle_patterns.rs::typed_struct_and_packed_array_pattern_lvalues_deconstruct_positions` | `review_bundle/r06_unpacked_struct_pattern_lvalue`; `--edition 2009` | stdout `PASS r06_unpacked_struct_pattern_lvalue\n`; stderr `llg: $finish at time 0 at tb:12:5\n` |
| SYN038-W13 | 129-bit conversion matrix for integral, enum, packed-struct, and packed-union values | `tests/fixtures/sim/type_conformance/conversions-129.sv` | `tests/sim_type_conformance/integral.rs::conversion_cases!::conversions_129` | `type_conformance/conversions-129`; default edition SystemVerilog-2009, with no `--edition` argument | stdout is independently generated by `conversion_matrix($width)` for `conversions_129: 129` and compared exactly; stderr is empty |
| SYN038-W14 | Packed-structure positional pattern lvalue | `tests/fixtures/sim/review_bundle/r06_packed_struct_pattern_lvalue.sv` | `tests/sim_review_bundle_patterns.rs::typed_struct_and_packed_array_pattern_lvalues_deconstruct_positions` | `review_bundle/r06_packed_struct_pattern_lvalue`; `--edition 2009` | stdout `PASS r06_packed_struct_pattern_lvalue\n`; stderr `llg: $finish at time 0 at tb:12:5\n` |
| SYN038-W16 | Packed ambiguous conditional result under both supplied editions | `tests/fixtures/sim/review_bundle/r10_packed_mux_policy.sv` | `tests/sim_conditional_policy.rs::packed_conditional_policy_uses_published_z_z_cell_in_both_editions` | `review_bundle/r10_packed_mux_policy`; `--edition 2001`; `--edition 2009` | stdout `MUX_POLICY result=x\n`; stderr `llg: $finish at time 0 at tb:10:5\n` |
| SYN038-W19 | Nested fixed-array conditional values use uninitialized element defaults | `tests/fixtures/sim/rtl_completion/array_conditional_nested.sv` | `tests/sim_rtl_completion.rs::array_conditional_nested_elements_use_uninitialized_defaults` | `rtl_completion/array_conditional_nested`; `--edition 2009` | stdout `array conditional nested defaults passed\n`; stderr `""` |
| SYN038-W21 | Aggregate sensitivity and always-family process contexts | `tests/fixtures/sim/syn014_process_contexts/process_contexts.sv` | `tests/sim_syn014_process_contexts.rs::aggregate_sensitivity_and_always_family_contracts_match_in_both_modes` | `syn014_process_contexts/process_contexts`; default edition SystemVerilog-2009, with no `--edition` argument | stdout `aggregate arm=1 at=2 feedback=c linked=07 split=c1 latch=x ff=00 delta=1\n` `enabled arm=5 at=2 feedback=c linked=15 split=c5 latch=5 ff=00\n` `triggered arm=c at=c feedback=c linked=15 split=c5 latch=5 ff=00\n` `array arm=c at=c feedback=c linked=31 split=c5 latch=5 ff=00\n` `held ff=00\n` `edge ff=2a\n` `reset ff=00\n`; stderr `""` |
| SYN038-W22 | Interface modports with packed-structure transfer and sequential output capture | `tests/fixtures/sim/feature_completion/g1_34/interface_modport_struct.sv` | `tests/sim_rtl_composition.rs::rtl_composition_interface_modport_struct` | `feature_completion/g1_34/interface_modport_struct`; default edition SystemVerilog-2009, with no `--edition` argument | stdout `reset wv=0 rv=0 rt=00 rp=0000 ov=0 of=000000\n` `t1 wv=1 rv=0 rt=00 rp=0000 ov=0 of=000000\n` `t2 wv=0 rv=1 rt=2b rp=bff0 ov=0 of=000000\n` `t3 wv=0 rv=0 rt=2b rp=bff0 ov=1 of=2bbff0\n`; stderr `""` |
| SYN038-W23 | Fixed-array and aggregate assignments with zero-time subroutine copy-out | `tests/fixtures/sim/feature_completion/g1_34/aggregate_functions.sv` | `tests/sim_rtl_composition.rs::rtl_composition_aggregate_functions` | `feature_completion/g1_34/aggregate_functions`; default edition SystemVerilog-2009, with no `--edition` argument | stdout `arr src=10203040 dst=10203040 part=00102010\n` `sum=160\n` `struct a=aa55 b=1122 idx=3 va=aa55 vb=1122\n` `mix=15 p=1005 acc=2007 o=2007 sw=0510\n` `defaults calls=1 y=41\n` `defaults calls=1 y=5\n` `combine s=15 c=107\n` `combine s=11 c=112\n` `ref r=16\n` `fib=55\n`; stderr `""` |
| SYN038-W44 | Interface-body `always_latch` whole-member blocking write and interface-body `always_ff` whole-member NBA writes | `tests/fixtures/sim/syn038_pairwise/interface_processes.sv` | `tests/sim_syn038_interface_processes.rs::interface_body_latch_and_ff_members_match_in_both_modes` | `syn038_pairwise/interface_processes`; `--edition 2009` | stdout `latch open=3c\n` `ff issue q=00 captured=00\n` `latch held=3c\n` `ff commit q=5a captured=00\n` `ff issue q=5a captured=00\n` `ff commit q=c3 captured=5a\n`; stderr `""` |
| SYN038-W45 | Generated child inout actuals use net-array elements; generated initial blocks pass separate variable-array elements to task `ref` and function `const ref` formals | `tests/fixtures/sim/syn038_pairwise/generate_actuals.sv` | `tests/sim_syn038_generate_actuals.rs::generated_scope_actuals_reach_ports_and_subroutine_formals` | `syn038_pairwise/generate_actuals`; `--edition 2009` | stdout `pads=3c,a5 ref=12,24 const=55,c3\n`; stderr `""` |
| SYN038-W46 | Interface-body `always_ff` writes a packed-union field; the whole packed-union interface member supplies a child input actual | `tests/fixtures/sim/syn038_pairwise/union_interface.sv` | `tests/sim_syn038_union_interface.rs::interface_union_field_nba_and_child_input_actual_match_in_both_modes` | `syn038_pairwise/union_interface`; `--edition 2009` | stdout `first word=3a nibbles=3/a child=3a/3/a\n` `second word=c5 nibbles=c/5 child=c5/c/5\n`; stderr `""` |
| SYN038-W47 | Automatic packed-union function result feeds a child input; child output targets a separate union actual; a clocked conditional NBA writes a union field | `tests/fixtures/sim/syn038_pairwise/union_function_port.sv` | `tests/sim_syn038_union_function_port.rs::packed_union_function_and_child_port_paths_keep_four_state_fields` | `syn038_pairwise/union_function_port`; `--edition 2009` | stdout `echo=a5z3 halves=a5,z3 captured=xx\n`; stderr `""` |
| SYN038-W48 | Packed-union parameter projection drives constant localparams and a typedef width; runtime union member reads are checked separately | `tests/fixtures/sim/syn038_pairwise/union_constant.sv` | `tests/sim_syn038_union_constant.rs::packed_union_projection_drives_constant_dimension_and_runtime_reads` | `syn038_pairwise/union_constant`; `--edition 2009` | stdout `base=a5 low=5 elaborated_width=10\n` `runtime=3c low=c\n`; stderr `""` |
| SYN038-W49 | Parent code passes a whole packed union by hierarchical task `ref`; the task writes a packed-struct field, with a same-task local-call control | `tests/fixtures/sim/syn038_pairwise/union_hier_ref.sv` | `tests/sim_syn038_union_hier_ref.rs::hierarchical_task_ref_updates_union_storage_and_named_view` | `syn038_pairwise/union_hier_ref`; `--edition 2009` | stdout `hier=12ff child-local=ab41 view=12/ff\n`; stderr `""` |
| SYN038-W50 | Automatic unpacked record-array assignment-pattern initialization and selected element `ref` update keep the neighboring field and row intact across activations | `tests/fixtures/sim/syn038_pairwise/record_auto_ref.sv` | `tests/sim_syn038_record_auto_ref.rs::automatic_record_array_initializer_and_selected_ref_match_oracle` | `syn038_pairwise/record_auto_ref`; `--edition 2009` | stdout `call=c5 row0=11/a1 row1=c5/b2\n` `call=e6 row0=11/a1 row1=e6/b2\n`; stderr `""` |
| SYN038-W51 | Interface process passes one unpacked-record field as a task `inout` actual; the scalar formal write preserves the adjacent record field | `tests/fixtures/sim/syn038_pairwise/interface_record_inout.sv` | `tests/sim_syn038_interface_record_inout.rs::interface_record_field_inout_task_preserves_neighbor_field_in_both_modes` | `syn038_pairwise/interface_record_inout`; `--edition 2009` | stdout `22 45\n`; stderr `""` |
| SYN038-W52 | Automatic enum local initialized by a full-width positional pattern and returned from calls in `initial` and `always_ff`; the clocked call result is captured by an enum NBA | `tests/fixtures/sim/syn038_pairwise/enum_auto_function.sv` | `tests/sim_syn038_enum_auto_function.rs::automatic_enum_pattern_function_and_clocked_nba_match_oracle` | `syn038_pairwise/enum_auto_function`; `--edition 2009` | stdout `initial=00000101 state=00000111\n`; stderr `""` |
| SYN038-W53 | Packed-structure cast initializes a function return slot consumed only as an `always_ff` NBA RHS | `tests/fixtures/sim/syn038_pairwise/packed_return_ff.sv` | `tests/sim_syn038_packed_return_ff.rs::packed_cast_function_return_is_captured_by_always_ff` | `syn038_pairwise/packed_return_ff`; `--edition 2009` | stdout `12 34\n`; stderr `""` |
| SYN038-W54 | Generated enum equality is used both as an `always` event expression and, as a distinct one-bit result, as a generated child input actual | `tests/fixtures/sim/syn038_pairwise/generate_enum_predicate.sv` | `tests/sim_syn038_generate_enum_predicate.rs::generated_enum_equality_event_and_child_predicate_match_in_both_modes` | `syn038_pairwise/generate_enum_predicate`; `--edition 2009` | stdout `events=1\n` `seen=1\n`; stderr `""` |
| SYN038-W55 | Fixed-record-array reduction with a mapped member initializes a separate automatic task-local integer | `tests/fixtures/sim/syn038_pairwise/record_reduction_init.sv` | `tests/sim_syn038_record_reduction_init.rs::fixed_record_reduction_initializes_automatic_local` | `syn038_pairwise/record_reduction_init`; `--edition 2009` | stdout `total=10\n`; stderr `""` |
| SYN038-W56 | Static function return slots are whole-object `output`, `inout`, and `ref` actuals; each helper's returned value is stored separately (O125–O127) | `tests/fixtures/sim/syn038_pairwise/return_slot_formals.sv` | `tests/sim_syn038_return_slot_formals.rs::static_function_return_slots_bind_each_writable_formal_direction` | `syn038_pairwise/return_slot_formals`; `--edition 2009` | stdout `output=1 inout=1 ref=1\n`; stderr `""` |
| SYN038-W57 | An automatic function return slot receives a full 8-bit positional pattern; its sole call is the RHS of module `always_comb`, with `result` a separate destination (O128) | `tests/fixtures/sim/syn038_pairwise/packed_pattern_return_comb.sv` | `tests/sim_syn038_packed_pattern_return_comb.rs::packed_pattern_function_return_runs_in_always_comb` | `syn038_pairwise/packed_pattern_return_comb`; `--edition 2009` | stdout `result=a5\n`; stderr `""` |
| SYN038-W58 | Separate interface `always` event sources read a fixed array-of-record reduction and compare `store.key` with a known byte; each source change counts once, while the result and counters are separate slots (O129–O130) | `tests/fixtures/sim/syn038_pairwise/interface_record_events.sv` | `tests/sim_syn038_interface_record_events.rs::interface_record_reduction_and_field_events_count_one_source_change` | `syn038_pairwise/interface_record_events`; `--edition 2009` | stdout `events=1,1\n`; stderr `""` |
| SYN038-W59 | A generated child output drives packed-structure field `pair.hi`; parent-written `pair.lo` and the child input source are separate slots (O131) | `tests/fixtures/sim/syn038_pairwise/generate_output_field.sv` | `tests/sim_syn038_generate_output_field.rs::generated_child_output_propagates_to_a_packed_struct_field` | `syn038_pairwise/generate_output_field`; `--edition 2009` | stdout `hi=5a\n`; stderr `""` |
| SYN038-W60 | One module-scope continuous variable assignment reaches static `read_state.state` through a hierarchical function-scope identifier; a define-selected second driver is a negative control (O132) | `tests/fixtures/sim/syn038_pairwise/static_local_continuous.sv` | `tests/sim_syn038_static_local_continuous.rs::static_function_local_accepts_continuous_variable_assignment`; `tests/sim_syn038_static_local_continuous.rs::duplicate_static_local_continuous_drivers_are_rejected_by_codegen` | Positive: `syn038_pairwise/static_local_continuous`; `--edition 2009`. Negative control: `--edition 2009`; `--define SYN038_DUPLICATE_DRIVER` | Positive stdout `state=1\n`, stderr `""`; negative control exits 1 with empty stdout and stderr containing `semantic error: multiple continuous assignments to variable storage` |
| SYN038-W61 | An interface initial process passes the whole unpacked record `store` to local `read_key(const ref record_t)`; `readback` and the receiving formal are separate slots (O133) | `tests/fixtures/sim/syn038_pairwise/interface_record_constref.sv` | `tests/sim_syn038_interface_record_constref.rs::interface_record_const_ref_function_returns_the_source_key` | `syn038_pairwise/interface_record_constref`; `--edition 2009` | stdout `read=3c\n`; stderr `""` |
| SYN038-W62 | Each of `always`, `always_comb`, `always_latch`, and `always_ff` calls input and `const ref` functions plus output, inout, and ref tasks on separate slots; the inout task adds four to its process-local value (O134–O153) | `tests/fixtures/sim/syn038_pairwise/formal_process_matrix.sv` | `tests/sim_syn038_formal_process_matrix.rs::all_formal_modes_execute_from_each_process_family` | `syn038_pairwise/formal_process_matrix`; `--edition 2009` | stdout `always=8,9,3,14,5\n` `comb=8,9,3,24,5\n` `latch=8,9,3,34,5\n` `ff=8,9,3,44,5\n`; stderr `""` |
| SYN038-W63 | Separate module source drives enum, packed-structure, and packed-union targets through element, row-slice, and concatenation lvalues across `initial`, `always_comb`, `always_latch`, and `always_ff` (O154–O162) | `tests/fixtures/sim/syn038_pairwise/typed_lvalue_matrix.sv` | `tests/sim_syn038_typed_lvalue_matrix.rs::typed_selected_lvalue_matrix_matches_in_both_optimizer_modes` | `syn038_pairwise/typed_lvalue_matrix`; `--edition 2009` | stdout `enum=01/a0/a5 struct=0100/a000/b5c2 union=0100/a000/d3a4\n`; stderr `""` |
| SYN038-W64 | Whole-object declaration initializers cover packed-structure patterns, packed-union casts, and integral predicates in constant, runtime, static-local, and automatic-local storage; readback checks stored values (O163–O174) | `tests/fixtures/sim/syn038_pairwise/typed_initializer_matrix.sv` | `tests/sim_syn038_typed_initializer_matrix.rs::typed_initializers_keep_their_constant_and_runtime_values` | `syn038_pairwise/typed_initializer_matrix`; `--edition 2009` | stdout `const=1234,b5c6,1 runtime=2143,c5d6,1 static=3153,d5e6,1 auto=4163,e5f6,1\n`; stderr `""` |
| SYN038-W65 | A whole module variable is passed as a child `ref` port actual through a structural child link; parent writes and child writes share the alias while the child mirror remains a separate output (O175) | `tests/fixtures/sim/syn038_pairwise/ref_port.sv` | `tests/sim_syn038_ref_port.rs::module_ref_port_tracks_parent_and_child_updates_in_both_modes` | `syn038_pairwise/ref_port`; `--edition 2009` | stdout `refport=c3\n`; stderr `""` |
| SYN038-W66 | One module-scope continuous variable assignment targets static function result `f` through a hierarchical identifier; `f()` reads the same whole return slot after the source changes (O176) | `tests/fixtures/sim/syn038_pairwise/static_return_continuous.sv` | `tests/sim_syn038_static_return_continuous.rs::static_function_result_accepts_hierarchical_continuous_variable_assignment` | `syn038_pairwise/static_return_continuous`; `--edition 2009`; helper chain `assert_exact_cli` → `invoke` → `fixture_path` | stdout `result=1\n`; stderr `Warning: {absolute_fixture_path}:10:27 non-void function 'f' does not return a value\n` (Slang `NoReturnStatement`) |
| SYN038-W67 | Constant localparam packed-struct member, packed-union member, and packed-union element projections feed separate runtime scalar assignment targets (O177–O179; no new pair IDs) | `tests/fixtures/sim/syn038_pairwise/localparam_member_reads.sv` | `tests/sim_syn038_localparam_member_reads.rs::packed_localparam_members_lower_as_constant_runtime_reads` | `syn038_pairwise/localparam_member_reads`; `--edition 2009` | stdout `struct=12\nunion=a5c3\noctet=a5\n`; stderr `""` |
| SYN038-W68 | Typed user-defined task actuals preserve distinct `output`, `inout`, `ref`, and `const ref` directions across enum, packed-struct, packed-union, fixed-record-array, and unpacked-record values; formal slots remain distinct from module storage (O180–O190; 13 pair closures) | `tests/fixtures/sim/syn038_pairwise/typed_formal_matrix.sv` | `tests/sim_syn038_typed_formal_matrix.rs::typed_actuals_keep_formal_directions_distinct_from_storage` | `syn038_pairwise/typed_formal_matrix`; `--edition 2009` | stdout `enum=01,01,01 pair=5aa5 union=5aa5 records=11/a1,22/b2 inout=c3/44 ref=31/c7\n`; stderr `""` |
| SYN038-W69 | Integral sources pass conditional, equality, cast, and assignment-pattern expressions through function calls and returns, event controls, a child input, constant dimensions, and a runtime declaration initializer (O191–O203; 28 pair closures) | `tests/fixtures/sim/syn038_pairwise/op_consumer_matrix.sv` | `tests/sim_syn038_op_consumer_matrix.rs::expression_consumers_keep_distinct_contexts_in_both_cli_modes` | `syn038_pairwise/op_consumer_matrix`; `--edition 2009`; explicit public-CLI runs with and without optimization | stdout `calls=18,0,18,18 events=1,1,1 widths=5,7,2\n`; stderr `""` |
| SYN038-W70 | Interface-body mutable byte members receive element, row-slice, concatenation, and positional-pattern blocking writes; a separately read-back member uses runtime declaration initialization (O204–O208) | `tests/fixtures/sim/syn038_pairwise/interface_lvalues.sv` | `tests/sim_syn038_interface_lvalues.rs::interface_member_lvalues_and_initializer_keep_separate_readbacks` | `syn038_pairwise/interface_lvalues`; `--edition 2009` | stdout `interface=01,a0,b2,01 seeded=12 lanes=34,56\n`; stderr `""` |
| SYN038-W71 | Runtime module scalar, enum, and fixed-array sources initialize distinct static and automatic locals; static initialization runs without a process and automatic initialization is checked in `initial` (O209–O214) | `tests/fixtures/sim/syn038_pairwise/runtime_source_local_initializers.sv` | `tests/sim_syn038_runtime_source_local_initializers.rs::runtime_sources_initialize_static_and_automatic_locals` | `syn038_pairwise/runtime_source_local_initializers`; `--edition 2009` | stdout `runtime-local-init=39,39 color=5c,5c lanes=39,4a:39,4a\n`; stderr `llg: simulation ended without $finish (no processes remain) at time 0\n` |
| SYN038-W72 | Element, row-slice, and concatenation blocking writes target an output formal, static local, automatic local, and function return slot; immediate assertions check every intermediate write before separate module readbacks (O215–O226; 13 pair closures) | `tests/fixtures/sim/syn038_pairwise/selected_activation_lvalues.sv` | `tests/sim_syn038_selected_activation_lvalues.rs::selected_formal_local_and_return_lvalues_match_in_both_optimizer_modes` | `syn038_pairwise/selected_activation_lvalues`; `--edition 2009` | stdout `local=b6,b7 formal=b4 return=b5\n`; stderr `""` |
| SYN038-W73 | Whole-object continuous net/variable drivers cover integral, enum, packed struct, packed union, unpacked record, and fixed record array sources, with field, concatenation, slice, and element targets; independent field/element checks validate whole aggregates, while conditional/equality/cast/pattern self-source paths read bits disjoint from their continuous target bits (O227–O249; 27 pair closures) | `tests/fixtures/sim/syn038_pairwise/typed_continuous_matrix.sv` | `tests/sim_syn038_typed_continuous_matrix.rs::typed_continuous_assignments_match_with_and_without_optimization` | `syn038_pairwise/typed_continuous_matrix`; `--edition 2009` | stdout `operation=03,03,03,03,03,05,05\nlvalue=1234,1234,b2,b3,c6\ncont=01,01,01,1234,1234,a5c3,a5c3,56,78,11,44\n`; stderr `""` |
| SYN038-W74 | Runtime indexing reads constant packed-struct/union localparams at ascending and descending endpoints, with signed and unsigned selectors; controls check selected Z, unknown and out-of-bounds indices, and a packed-record neighboring-field sentinel (O250–O260; adds zero pair IDs) | `tests/fixtures/sim/syn038_pairwise/localparam_runtime_index.sv` | `tests/sim_syn038_localparam_runtime_index.rs::localparam_packed_member_runtime_indices_preserve_four_state_selection` | `syn038_pairwise/localparam_runtime_index`; `--edition 2009` | stdout `valid=a5/c3 asc=a5/c3 signed=c3 unsigned=xx z=z3 unknown=xx out=xx/xx/xx\n`; stderr `""` |
| SYN038-W75 | Qualified child values feed conditional, equality, cast, assignment-pattern, field, array-element, port-actual, function-return, and event contexts; the armed child enum event is explicitly triggered, and immediate assertions check field, element, slice, concatenation, then positional-pattern writes (O261–O275; 22 pair closures) | `tests/fixtures/sim/syn038_pairwise/hierarchical_value_matrix.sv` | `tests/sim_syn038_hierarchical_value_matrix.rs::hierarchical_child_values_and_selected_writes_execute` | `syn038_pairwise/hierarchical_value_matrix`; `--edition 2009` | stdout `hier=01,1,12,56,21,33,56,12,53 event=1\n`; stderr `""` |
| SYN038-W76 | Task/function actuals cover static and automatic locals, formals, a function return slot, interface members, and module sources; immediate checks observe call results, copy-out, and aliases. Interface-member data has route priority for unqualified calls, while a qualified child callee selects `hierarchical_identifier`; `iface_actual` is static local and `hier_const_value` is module storage (O276–O300; 35 pair closures) | `tests/fixtures/sim/syn038_pairwise/call_storage_matrix.sv` | `tests/sim_syn038_call_storage_matrix.rs::call_actual_storage_and_routes_match_in_both_optimizer_modes` | `syn038_pairwise/call_storage_matrix`; `--edition 2009` | stdout `calls=12,43,13,44,25,2d,55 iface=11,41,13,2c,42,12 hier=33,6f\n`; stderr `""` |
| SYN038-W77 | `always`, `always_comb`, `always_latch`, and `always_ff` cover typed whole and selected writes plus conditional, equality, cast, and assignment-pattern sources; assertions check each process result, including committed NBA values (O301–O324; 26 selected pair gains) | `tests/fixtures/sim/syn038_pairwise/process_value_matrix.sv` | `tests/sim_syn038_process_value_matrix.rs::process_values_match_across_optimizer_modes` | `syn038_pairwise/process_value_matrix`; `--edition 2009` | stdout `process=12,abcd,55,a,b2,01 c3,01,12/34,11/22/33/44,a5,1,25 d4,01,01,55/66,1,25,a5 e,01,12/34,1\n`; stderr `""` |
| SYN038-W78 | Parent module variable declaration initializers read a fixed child localparam and separate overridden child localparams, preserving sibling instance identity; a procedural hierarchical read is a control (O325–O328; closes `SYN038-GAP-CO-declaration_initializer__HR-hierarchical_identifier` and `SYN038-GAP-HR-hierarchical_identifier__IN-runtime_declaration`) | `tests/fixtures/sim/syn038_pairwise/child_param_decl_init.sv` | `tests/sim_syn038_child_param_decl_init.rs::child_parameter_initializers_keep_instance_identity_in_both_modes` | `syn038_pairwise/child_param_decl_init`; `--edition 2009` | stdout `decl=05 siblings=05/0a procedural=05\n`; stderr `""` |
| SYN038-W79 | Conditional, equality/inside, cast, and assignment-pattern expressions compute selected unpacked-array addresses for element, field, row-slice, concatenation, and positional-pattern lvalues; immediate low/high checks verify selected destinations and neighboring storage (O329–O342; 18 distinct selected-pair gains: 14 `OP × LV` and four `TY × OP/LV` cells). `O329–O331`, `O333–O335`, and `O337–O342` use outer `TY=fixed_array_integral`; `O332` and `O336` omit `TY` because an unpacked array of packed records has no matching outer type level in the finite `TY` set. | `tests/fixtures/sim/syn038_pairwise/op_lvalue_address_matrix.sv` | `tests/sim_syn038_op_lvalue_address_matrix.rs::operation_lvalues_use_the_selected_address_in_both_cli_modes` | `syn038_pairwise/op_lvalue_address_matrix`; `--edition 2009` | stdout `conditional=a2,b6,0 equality=a5,b4,1 cast=b6,b4,1 pattern=10110110,d4,1\n`; stderr `""` |
| SYN038-W80 | Whole, field, element, row-slice, concatenation, and positional-pattern lvalues span always-family processes, interface members, a child output port, output task/function actuals, declaration initializers, and function return reads; independent readbacks preserve distinct source and destination slots (O343–O374; 31 selected pair gains). O370–O372 cover `SL=static_local/interface_member/return_slot × IN=automatic_local`. | `tests/fixtures/sim/syn038_pairwise/lvalue_context_matrix.sv` | `tests/sim_syn038_lvalue_context_matrix.rs::selected_lvalue_contexts_keep_actual_storage_and_process_paths_distinct` | `syn038_pairwise/lvalue_context_matrix`; `--edition 2009` | stdout `proc=11,22,3,45,01 comb=89,80 latch=cd,81 ff=5,01 if=31,4,56,01 call=c1,d2,6,7,e3,f4 child=9a init=23,34,45,56,12 extra=45,12,78,67,12\n`; stderr `""` |
| SYN038-W81 | Continuous assignment positional patterns cover a resolved net with an independent competing driver, a selected wire, and a pure variable; pre/post source-change assertions check target bits, while a separate function-counting control checks RHS sensitivity (O375–O377; closes `LV=positional_pattern × WK=continuous_net`, `LV=positional_pattern × WK=continuous_variable`, and `LV=positional_pattern × PC=none`). | `tests/fixtures/sim/syn038_pairwise/continuous_pattern_lhs.sv` | `tests/sim_syn038_continuous_pattern_lhs.rs::continuous_pattern_lvalues_preserve_targets_and_rhs_sensitivity` | `syn038_pairwise/continuous_pattern_lhs`; `--edition 2009` | stdout `rhs_eval\nrhs_eval\nnet=01 selected_net=01 pure_variable=01 counted_variable=01\n`; stderr `""` |
| SYN038-W82 | Function-result sources from automatic locals, formals, return slots, and static locals, plus interface-member sources, feed separate constant, runtime, static-local, and automatic-local declarations; assertions distinguish source values and initializer phase (O378–O393; 18 distinct pair gains). Static block declarations nested in `initial` still use `PC=none`; automatic initial-block declarations use `PC=initial`. | `tests/fixtures/sim/syn038_pairwise/initializer_source_matrix.sv` | `tests/sim_syn038_initializer_source_matrix.rs::declaration_initializers_read_each_source_storage_kind` | `syn038_pairwise/initializer_source_matrix`; `--edition 2009`; optimized and `--no-opt` | stdout `automatic=11,21,31,41 formal=52,62,72 return=84,94,a4,b4 static=01,01,02 interface=00,00,d5\n`; stderr `""` |
| SYN038-W83 | Interface and generated-scope assignments, storage writes across all process families, function actual/return paths, constant-function evaluation, and event sources retain their distinct scope and focal storage (O394–O423; 34 net selected-pair gains after one pair shared with W82). Immediate phase assertions check source updates, delayed NBA state, interface/generated values, and event results. | `tests/fixtures/sim/syn038_pairwise/scope_storage_process_matrix.sv` | `tests/sim_syn038_scope_storage_process_matrix.rs::storage_and_process_contexts_keep_distinct_source_paths_observable` | `syn038_pairwise/scope_storage_process_matrix`; `--edition 2009`; optimized and `--no-opt` | stdout `scope=5b,5b,5b,5b,5b,5a,5b event=5a gen=1,1 if=1,1 const=5\n`; stderr `""` |
| SYN038-W84 | Equality, `inside`, cast, and assignment-pattern values feed whole-object NBA destinations in `always_comb`; an `always_latch` arithmetic NBA and a distinct continuous const-ref function actual provide controls (O424–O429; six net selected-pair gains after two pairs already covered by W83). Assertions distinguish the process commits and continuous-call updates. | `tests/fixtures/sim/syn038_pairwise/process_write_remainders.sv` | `tests/sim_syn038_process_write_remainders.rs::remaining_process_write_paths_match_in_both_cli_modes` | `syn038_pairwise/process_write_remainders`; `--edition 2009`; optimized and `--no-opt` | stdout `ops=0,0,21,21 comb=24 latch=25 const_ref=a1\n`; stderr `""` |
| SYN038-W85 | Child hierarchy and concrete interface-member values are read across generate, interface, module, and subroutine scopes; typed interface sources include enums, integral arrays, record arrays, and unpacked records, with initializer defaults and distinct process/event routes checked (O430–O449; 27 distinct selected-pair gains). Assertions cover hierarchy priority, nested interface routes, and pre-process static default values. | `tests/fixtures/sim/syn038_pairwise/scope_hierarchy_matrix.sv` | `tests/sim_syn038_scope_hierarchy_matrix.rs::source_routes_keep_their_lexical_scope_and_storage_identity` | `syn038_pairwise/scope_hierarchy_matrix`; `--edition 2009`; optimized and `--no-opt` | stdout `scope=if:10,6b,00 gen:20,a7 module:10 types:5c,22,88,44 op:a7,0,05,e5 return:a7 init:1000a700 event=1 latch=a7,10 ff=10\n`; stderr `""` |
| SYN038-W86 | Function return slots and fixed-array reduction receivers remain the focal producer values as conditional, comparison, cast, pattern, call, return, port, constant, event, initializer, hierarchy, and process consumers are checked separately (O450–O475; 42 selected-pair gains). Receiving formals and assignment destinations are distinct oracle slots, not the producer. | `tests/fixtures/sim/syn038_pairwise/call_provenance_matrix.sv` | `tests/sim_syn038_call_provenance_matrix.rs::call_provenance_paths_match_in_both_optimizer_modes` | `syn038_pairwise/call_provenance_matrix`; `--edition 2009`; optimized and `--no-opt` | stdout `calls=13,1,19,21,18 task=13,12 routes=12,12,5a,12 reductions=6,1,6,12,05,05,0d,09 processes=12,05,05,1 parameter=0 overrides=51,84\n`; stderr `""` |
| SYN038-W87 | A module variable declaration initializer reads a two-state interface member before a later procedural write; same-scope and interface controls verify their default-zero snapshots and preserve Slang's source-located warnings (O476; one selected-pair gain). | `tests/fixtures/sim/syn038_pairwise/interface_runtime_initializer.sv` | `tests/sim_syn038_interface_runtime_initializer.rs::module_initializer_reads_bound_interface_storage_in_both_modes` | Fixture `syn038_pairwise/interface_runtime_initializer`; direct public CLI `llg --top tb [--no-opt] --edition 2009 {absolute_fixture_path}`; both optimizer modes | stdout `copy=00,source=5a,control=00\n`; stderr `Warning: {absolute_fixture_path}:9:35 initializer for static variable 'same_scope_copy' refers to 'local_seed' which will not have a value at initialization time\nWarning: {absolute_fixture_path}:10:24 initializer for static variable 'copy' refers to 'value' which will not have a value at initialization time\n` |
| SYN038-W88 | Output formals, static task locals, return slots, enum/structure formals, and static record locals exercise whole-object, field, continuous-net, blocking, and NBA writes with committed/readback checks (O477–O484; eight selected-pair gains). | `tests/fixtures/sim/syn038_pairwise/storage_write_remainders.sv` | `tests/sim_syn038_storage_write_remainders.rs::storage_write_remainder_cells_run_in_both_optimizer_modes` | `syn038_pairwise/storage_write_remainders`; `--edition 2009`; optimized and `--no-opt` | stdout `storage_write_remainders=passed\n`; stderr `Warning: {absolute_fixture_path}:101:33 non-void function 'hier_result' does not return a value\nllg: $finish at time 4000 at tb:153:9\n` |
| SYN038-W89 | A static function's hierarchical implicit result variable is passed directly as a task `ref` actual; a task write and function readback verify the same aliased slot (O485; zero additional pair gains). | `tests/fixtures/sim/syn038_pairwise/static_return_ref_actual.sv` | `tests/sim_syn038_static_return_ref_actual.rs::hierarchical_static_function_return_slot_binds_to_task_ref_formal` | `syn038_pairwise/static_return_ref_actual`; `--edition 2009`; optimized and `--no-opt` | stdout `static-return-ref=passed\n`; stderr `llg: $finish at time 0 at tb:18:9\n` |
| SYN038-W90 | Positional-pattern and concatenation output/inout actuals cover module, task, and function calls; formal, static, automatic, and function-return targets retain distinct storage, with selected reads/writes checking fields, slices, and copy-out. A direct packed-slice module `ref` port is legal and closes `LV=row_slice × FM=ref` (O486–O502; 23 selected-pair gains). | `tests/fixtures/sim/syn038_pairwise/lvalue_storage_formal_matrix.sv` | `tests/sim_syn038_lvalue_storage_formal_matrix.rs::lvalue_storage_and_formal_paths_preserve_selected_values` | `syn038_pairwise/lvalue_storage_formal_matrix`; `--edition 2009`; optimized and `--no-opt` | stdout `union=1234,5678 port=10 task=10 function=10:01 dynamic=10/1,1 direct=10 concat=10 inout=11 concat_inout=11 concat_fn=4142 field=61,62 row=31,32 formal=11,12 static=21,22 automatic=23,24 return=10 task_local=92 ref=a0 ref_field=7a\n`; stderr `""` |
| SYN038-W91 | Typed conditional/equality/cast sources, typed constant widths and initializers, equality-event sources, and typed function returns cover the remaining value sources; full-width readbacks keep focal sources separate from consumers (O503–O519; 17 selected-pair gains). | `tests/fixtures/sim/syn038_pairwise/typed_context_remainders.sv` | `tests/sim_syn038_typed_context_remainders.rs::typed_context_remainders_keep_exact_values_in_both_optimizer_modes` | `syn038_pairwise/typed_context_remainders`; `--edition 2009`; optimized and `--no-opt` | stdout `const=2,5,6,4,2 op=1234,0,2143,11223344,556677,1 init=5a6b,21,11,55,77,64 target=20,60,81,92 events=1,1 row=3344 override=62,82,43 extra=1,0,1,c0de\n`; stderr `""` |
| SYN038-W92 | Typed lvalues add fixed-record-array row-slice, concatenation and positional-pattern targets, plus unpacked-record selected-member targets and enum positional targets; immediate checks preserve adjacent fields and sibling elements (O520–O525; six selected-pair gains). | `tests/fixtures/sim/syn038_pairwise/typed_aggregate_lvalue_matrix.sv` | `tests/sim_syn038_typed_aggregate_lvalue_matrix.rs::typed_aggregate_lvalue_matrix_matches_exact_oracle_in_both_optimizer_modes` | `syn038_pairwise/typed_aggregate_lvalue_matrix`; `--edition 2009`; optimized and `--no-opt` | stdout `records=20,21,22,23 payload=70,71,61 enum=1,2\n`; stderr `""` |
| SYN038-W93 | Generated `initial` processes write a fixed-array row slice, packed concatenation and positional-pattern target, with immediate checks at each generated destination (O526–O528; three selected-pair gains). | `tests/fixtures/sim/syn038_pairwise/lvalue_generate_scope_remainders.sv` | `tests/sim_syn038_lvalue_generate_scope_remainders.rs::generated_scope_lvalues_write_the_selected_objects` | `syn038_pairwise/lvalue_generate_scope_remainders`; `--edition 2009`; optimized and `--no-opt` | stdout `generated_row=31,32 generated_concat=4142 generated_pattern=01\n`; stderr `""` |
| SYN038-W94 | Subroutine, generate and interface scopes cover constant, runtime, automatic and static declaration initializers; process and declaration-phase checks distinguish local initialization from enclosing process execution (O529–O536; 13 selected-pair gains). | `tests/fixtures/sim/syn038_pairwise/scope_initializer_remainders.sv` | `tests/sim_syn038_scope_initializer_remainders.rs::scope_initializer_remainders_match_in_both_cli_modes` | `syn038_pairwise/scope_initializer_remainders`; `--edition 2009`; optimized and `--no-opt` | stdout `subroutine=a1\ngenerate=11,22,33,44\ninterface=51,62,73\n`; stderr `""` |
| SYN038-W95 | Generated fixed-array reduction, interface function result, generated latch target, nested interface child-port actual, and lexical package constants cover distinct scope and consumer paths (O537–O543; 10 selected-pair gains). | `tests/fixtures/sim/syn038_pairwise/hierarchy_process_call_remainders.sv` | `tests/sim_syn038_hierarchy_process_call_remainders.rs::hierarchy_process_call_remainders_match_in_both_cli_modes` | `syn038_pairwise/hierarchy_process_call_remainders`; `--edition 2009`; optimized and `--no-opt` | stdout `reduction=07 function=a6 latch=91 child_port=5c const=5,7,2a\n`; stderr `""` |
| SYN038-W96 | Complete enum, packed-struct and fixed-integral-array sources in interface `always_comb`, plus complete packed-union and unpacked-record sources in generated `always_comb`, are copied into same-typed destinations and checked after settling (O544–O548; five selected-pair gains). | `tests/fixtures/sim/syn038_pairwise/typed_scope_remainders.sv` | `tests/sim_syn038_typed_scope_remainders.rs::typed_values_keep_their_outer_type_in_interface_and_generate_scopes` | `syn038_pairwise/typed_scope_remainders`; `--edition 2009`; optimized and `--no-opt` | stdout `tyhc=a,2b,c,5a,d\n`; stderr `""` |
| SYN038-W97 | Automatic locals, static locals and formals flow through constant functions into distinct packed typedef dimensions; `CP=function` records each actual result producer and the elaborated widths are checked (O549–O551; three selected-pair gains). | `tests/fixtures/sim/syn038_pairwise/typed_constant_event_matrix.sv` | `tests/sim_syn038_typed_constant_event_matrix.rs::typed_constant_event_paths_keep_exact_values_in_both_optimizer_modes` | `syn038_pairwise/typed_constant_event_matrix`; `--edition 2009`; optimized and `--no-opt` | stdout `widths=3,5,5,4 constfunc=5,6,7 const=4/12,3/0d runtime=5/17,4/0b static=6/1a,2/09 return=5aa5 overrides=2/1a,7/03,4/12 nested=51,62,73,84,31,42 events=1,1\n`; stderr `""` |
| SYN038-W98 | Operation sources span union array patterns, typed function inputs, constant dimensions, automatic/static/formal locals, generate/interface processes, task inputs and event expressions (O552–O572; 21 selected-pair gains). Direct operator-expression `const ref` negatives are separate from legal selected-element variable controls. | `tests/fixtures/sim/syn038_pairwise/operation_context_matrix.sv` and four `constref_*_rejected.sv` controls | `tests/sim_syn038_operation_context_matrix.rs::typed_operation_contexts_keep_source_and_use_site_in_both_cli_modes` | `syn038_pairwise/operation_context_matrix`; `--top tb --edition 2009`; optimized and `--no-opt` | stdout `types=1122,0,0,11223344,5162 task=25,0,25,2534 formal=2534 constref=34,34,34,34,5a locals=25,25,25,25,25 snapshot=00,00 constant=22,3 generate=25,25,2534 interface=25,25,2534\nunion-pattern=12,34\nfunction-types=2,56 event=1\n`; stderr `""`. Each direct-expression const-ref negative returns 1, emits no stdout, and must match the exact §13.5.2 Slang diagnostic in both modes. |
| SYN038-W99 | Hierarchical child/interface driver targets and qualified `ref` actual address paths preserve their own storage and route; O576/O577 classify only the call-argument address path, with later alias readback as an independent oracle (O573–O577; five selected-pair gains). | `tests/fixtures/sim/syn038_pairwise/storage_hier_route_remainders.sv` | `tests/sim_syn038_storage_hier_route_remainders.rs::storage_and_hierarchical_routes_run_in_both_optimizer_modes` | `syn038_pairwise/storage_hier_route_remainders`; `--edition 2009`; optimized and `--no-opt` | stdout `storage_hier_route_remainders=passed\n`; stderr `llg: $finish at time 2000 at tb:146:9\n` |
| SYN038-W100 | A static procedural local is used as an explicitly qualified child input actual; qualification determines the hierarchical route (O578; one selected-pair gain). | `tests/fixtures/sim/syn038_pairwise/static_hierarchical_block_port_actual.sv` | `tests/sim_syn038_static_hierarchical_block_port_actual.rs::static_block_local_hierarchical_port_actual_runs_in_both_optimizer_modes` | `syn038_pairwise/static_hierarchical_block_port_actual`; `--edition 2009`; optimized and `--no-opt` | stdout `static_hierarchical_block_port_actual=passed\n`; stderr `llg: $finish at time 1000 at tb:32:9\n` |
| SYN038-W101 | Mutable interface-member values feed a nested child-interface input port and are checked before and after source updates (O579; one selected-pair gain). | `tests/fixtures/sim/syn038_pairwise/interface_member_child_port.sv` | `tests/sim_syn038_interface_member_child_port.rs::mutable_interface_member_drives_nested_child_port_readback_in_both_modes` | `syn038_pairwise/interface_member_child_port`; `--edition 2009`; optimized and `--no-opt` | stdout `interface-child-port=a5,a5\n`; stderr `""` |
| SYN038-W102 | A concrete interface member is read as a function's source and observed through separate caller samples; a sibling instance is a control (O580; two selected-pair gains). | `tests/fixtures/sim/syn038_pairwise/interface_function_source.sv` | `tests/sim_syn038_interface_function_source.rs::mutable_interface_member_source_flows_through_function_result_in_both_modes` | `syn038_pairwise/interface_function_source`; `--edition 2009`; optimized and `--no-opt` | stdout `member_through_function=31,6b sibling=92\n`; stderr `llg: $finish at time 0 at tb:35:9\n` |
| SYN038-W103 | Source-only union/function and conditional producer paths are paired with LHS address operations, continuous self-source and `always_ff` source controls (O581–O587; ten selected-pair gains). O582–O585 omit `CO` because each operator computes a target address and the RHS is separate. | `tests/fixtures/sim/syn038_pairwise/legacy_op_replacements.sv` | `tests/sim_syn038_legacy_op_replacements.rs::selected_write_addresses_and_source_only_paths_have_independent_oracles` | `syn038_pairwise/legacy_op_replacements`; `--edition 2009`; optimized and `--no-opt` | stdout `conditional=a5,a5 equality=b6,b6 cast=c7,c7 pattern=d8,d8 net=05 union=5678,1234 source=11,22\n`; stderr `""` |
| SYN038-W104 | Function return/source values are checked at their source slots across packed records, array element writes, continuous net/variable reads and combinational/sequential processes; receiver samples remain separate (O588–O594; seven net selected-pair gains). Focal review confirms O591/O592/O594 are same-focal RHS reads and retain `CO=assignment_rhs`. | `tests/fixtures/sim/syn038_pairwise/legacy_cp_replacements.sv` | `tests/sim_syn038_legacy_cp_replacements.rs::function_source_replacement_witnesses_match_in_both_optimizer_modes` | `syn038_pairwise/legacy_cp_replacements`; `--edition 2009`; optimized and `--no-opt` | stdout `initial=12/34 array=12,34,56 conditional=21,43 net=5a function=5a direct=5a var=7d function=7d direct=7d comb=6b/6b\nupdated=net=a5 function=a5 direct=a5 var=c6 function=c6 direct=c6 comb=7c/7c\nff=11->22 sample=11\nff=22->33 sample=22\n`; stderr `""` |
| SYN038-W105 | A module source and shared static task-formal source are changed by blocking writes and observed by event controls from a subroutine across concurrent activations (O595–O596). The source-bound events and the two same-slot write/use paths have explicit rules and coverage pins. | `tests/fixtures/sim/syn038_pairwise/subroutine_event_signal_matrix.sv` | `tests/sim_syn038_subroutine_event_signal_matrix.rs::subroutine_event_sources_observe_transitions_in_both_cli_modes` | `syn038_pairwise/subroutine_event_signal_matrix`; `--edition 2009`; optimized and `--no-opt` | stdout `module=1 formal=1\n`; stderr `""` |
| SYN038-W106 same-root assignment RHS | Five same-root RHS shapes cover whole-object blocking, packed-record field, concatenation, positional-pattern and whole-object NBA assignments (O597–O601). The DB owner binds every RHS read to its own LHS root; the selected pair closures are `CO-assignment_rhs__LV-field` and `CO-assignment_rhs__LV-positional_pattern`. | `tests/fixtures/sim/syn038_pairwise/co_same_root_assignment_rhs.sv` | `tests/sim_syn038_co_same_root_assignment_rhs.rs::same_root_assignment_rhs_witnesses_match_in_both_optimizer_modes`; DB proof: `slang_binds_each_assignment_rhs_to_its_own_destination_declaration` | `syn038_pairwise/co_same_root_assignment_rhs`; `--top tb --edition 2009`; optimized and `--no-opt`; DB proves source-identity equality at fixture lines 22, 26, 30, 34 and 38 | stdout `same_root=11,21,69,01,31\n`; stderr `""` |
| SYN038-W107 | Whole-array, element, row-slice, concatenation and pattern writes are followed by fixed-array reduction reads of the same receiver (O602–O606). | `tests/fixtures/sim/syn038_pairwise/written_array_reduction_receivers.sv` | `tests/sim_syn038_written_array_reduction_receivers.rs::written_fixed_array_reductions_match_both_optimizer_modes` | `syn038_pairwise/written_array_reduction_receivers`; `--edition 2009`; optimized and `--no-opt` | stdout `written-reduction=0a,29,37,30,1e\n`; stderr `""` |
| SYN038-W108 | Fixed-array reduction receivers observe continuous-net, continuous-variable and NBA element writes (O607–O609). | `tests/fixtures/sim/syn038_pairwise/written_reduction_continuous_and_nba.sv` | `tests/sim_syn038_written_reduction_continuous_and_nba.rs::fixed_array_reductions_observe_continuous_and_nba_writes_in_both_cli_modes` | `syn038_pairwise/written_reduction_continuous_and_nba`; `--edition 2009`; optimized and `--no-opt` | stdout `net-reduction=6\nvar-reduction=6\nnba-reduction=5\n`; stderr `""` |
| SYN038-W109 | Scalar, packed-field, unpacked-member, element, row-slice, concatenation and pattern lvalues bind to event operands of the same declared object (O610–O615). Owned DB binding proof: `event_operands_and_lvalue_forms_share_each_owned_declaration_identity`. | `tests/fixtures/sim/syn038_pairwise/event_lvalue_matrix.sv` | `tests/sim_syn038_event_lvalue_matrix.rs::event_expression_lvalues_run_in_both_cli_modes` | `syn038_pairwise/event_lvalue_matrix`; `--edition 2009`; optimized and `--no-opt` | stdout `events=111111 field=31,80 unpacked=35,80 element=70,42 row=51,52/73 concat=a6c2 pattern=5c\n`; stderr `""` |
| SYN038-W110 | An event-expression read and an NBA write share one focal slot; before/after output distinguishes the old value from the committed value (O616). | `tests/fixtures/sim/syn038_pairwise/event_expression_nba_same_slot.sv` | `tests/sim_syn038_event_expression_nba_same_slot.rs::event_expression_and_nba_write_observe_the_same_slot_in_both_cli_modes` | `syn038_pairwise/event_expression_nba_same_slot`; `--edition 2009`; optimized and `--no-opt` | stdout `before=11 old=00 event=00 seen=0\nafter=11 state=22 event=22 seen=1\n`; stderr `""` |
| SYN038-W111 | Continuous net and variable drivers feed event controls, combinational RHS consumers and latch RHS consumers across their listed process sites (O617–O628). Owned DB binding proof: `owned_continuous_lhs_event_and_assignment_rhs_consumers_bind_to_outer_source`. | `tests/fixtures/sim/syn038_pairwise/continuous_event_drivers.sv` | `tests/sim_syn038_continuous_event_drivers.rs::continuous_net_and_variable_events_match_exact_cli_oracle_in_both_modes` | `syn038_pairwise/continuous_event_drivers`; `--edition 2009`; optimized and `--no-opt` | stdout `wire=1,1,1,1,0,1 logic=1,1,1,1,0,1\n`; stderr `""` |
| SYN038-W112 | Prior NBA and continuous net/variable writes are read as task input actuals (O629–O631). | `tests/fixtures/sim/syn038_pairwise/written_sources_call_arguments.sv` | `tests/sim_syn038_written_sources_call_arguments.rs::written_module_sources_reach_task_inputs_in_both_cli_modes` | `syn038_pairwise/written_sources_call_arguments`; `--edition 2009`; optimized and `--no-opt` | stdout `nba_call=5a/5a\ncontinuous_call=5a/5a\n`; stderr `""` |
| SYN038-W113 | Same-source blocking writes are observed through function returns and automatic, static, module runtime and constant declaration initializers (O632–O638). | `tests/fixtures/sim/syn038_pairwise/written_source_return_initializers.sv` | `tests/sim_syn038_written_source_return_initializers.rs::written_return_and_initializer_sources_match_in_both_cli_modes` | `syn038_pairwise/written_source_return_initializers`; `--edition 2009`; optimized and `--no-opt` | stdout `local_initializer=5a\ninitializer_source=5a\nstatic_initializer=5a\nlocal_return=5a\nmodule_initializer=5a\nconstant_initializer=5a\n`; stderr `""` |
| SYN038-W114 | A continuous net source drives a child input port actual and is checked at the child (O639). Owned DB binding proof: `owned_wire_driver_and_child_input_actual_share_outer_source_identity`. | `tests/fixtures/sim/syn038_pairwise/port_driven_wire_child_input.sv` | `tests/sim_syn038_written_child_inputs.rs::continuously_driven_wire_child_input_matches_both_cli_modes` | `syn038_pairwise/port_driven_wire_child_input`; `--edition 2009`; optimized and `--no-opt` | stdout `port_source=5a/5a\n`; stderr `""` |
| SYN038-W115 | A continuous variable source drives a child input port actual and is checked at the child (O640). Owned DB binding proof: `owned_logic_driver_and_child_input_actual_share_outer_source_identity`. | `tests/fixtures/sim/syn038_pairwise/port_driven_logic_child_input.sv` | `tests/sim_syn038_written_child_inputs.rs::continuously_driven_logic_child_input_matches_both_cli_modes` | `syn038_pairwise/port_driven_logic_child_input`; `--edition 2009`; optimized and `--no-opt` | stdout `port_logic=5a/5a\n`; stderr `""` |
| SYN038-W116 | An NBA-written variable is read through a child input port actual (O641). Owned DB binding proof: `owned_nba_write_binds_to_static_child_input_actual`. | `tests/fixtures/sim/syn038_pairwise/nba_written_child_input.sv` | `tests/sim_syn038_written_consumer_inputs.rs::nba_written_child_input_matches_both_cli_modes` | `syn038_pairwise/nba_written_child_input`; `--edition 2009`; optimized and `--no-opt` | stdout `nba_port=5a/5a\n`; stderr `""` |
| SYN038-W117 | A blocking-written variable is read through a child input port actual (O642). Owned DB binding proof: `owned_blocking_write_binds_to_static_child_input_actual`. | `tests/fixtures/sim/syn038_pairwise/blocking_written_child_input.sv` | `tests/sim_syn038_written_consumer_inputs.rs::blocking_written_child_input_matches_both_cli_modes` | `syn038_pairwise/blocking_written_child_input`; `--edition 2009`; optimized and `--no-opt` | stdout `blocking_port=5a/5a\n`; stderr `""` |
| SYN038-W118 | Continuous net/variable and NBA-written values are read as function input actuals (O643–O645). Owned DB binding proof: `owned_function_input_actuals_share_each_written_source_identity`. | `tests/fixtures/sim/syn038_pairwise/function_inputs_from_drivers.sv` | `tests/sim_syn038_written_consumer_inputs.rs::driven_sources_passed_to_function_inputs_match_both_cli_modes` | `syn038_pairwise/function_inputs_from_drivers`; `--edition 2009`; optimized and `--no-opt` | stdout `function_inputs=5a/5a/5a\n`; stderr `""` |
| SYN038-W119 | Five blocking-written selected sources are read as task input actuals using field, element, row-slice, concatenation and positional-pattern writes (O649–O653; +5 selected-input paths). Owned DB binding proof: `owned_selected_writes_and_input_actuals_share_source_identity`. | `tests/fixtures/sim/syn038_pairwise/written_selected_input_actuals.sv` | `tests/sim_syn038_written_selected_input_actuals.rs::written_selected_sources_reach_task_inputs_in_both_cli_modes` | `syn038_pairwise/written_selected_input_actuals`; `--edition 2009`; optimized and `--no-opt` | stdout `selected-inputs=31/80,42/17,51,52/73,a6/74,5c,7d/75\n`; stderr `""` |
| SYN038-W120 | Whole variable, field, element, row-slice, concatenation, pattern, NBA and continuous-variable prior writes feed whole-variable const-ref task actuals (O668–O675; +7 selected paths). Owned DB binding proof: `owned_prior_writes_and_const_ref_actuals_share_source_identity`. | `tests/fixtures/sim/syn038_pairwise/constref_written_source_actuals.sv` | `tests/sim_syn038_constref_written_source_actuals.rs::prior_writes_reach_whole_const_ref_actuals_in_both_cli_modes` | `syn038_pairwise/constref_written_source_actuals`; `--edition 2009`; optimized and `--no-opt` | stdout `constref=31,42/81,53/16/75,51,62/76,a7,b8/77,8d,9e/78\nextra=86/97\n`; stderr `""` |
| SYN038-W121 | Blocking-written whole variables and selected fields/elements/slices/concatenations/patterns are supplied to ref/inout tasks (O654–O665; +4 selected paths). Owned DB binding proof: `owned_blocking_writes_and_whole_ref_inout_actuals_share_source_identity`. | `tests/fixtures/sim/syn038_pairwise/written_ref_inout_actuals.sv` | `tests/sim_syn038_written_ref_inout_actuals.rs::blocking_written_whole_variables_reach_ref_and_inout_tasks_in_both_cli_modes` | `syn038_pairwise/written_ref_inout_actuals`; `--edition 2009`; optimized and `--no-opt` | stdout `prior-ref=10>ef,21>de,32>cd,54>ab,a6>59,5c>a3\nprior-inout=11>ee,22>dd,42>bd,64>9b,b6>49,6c>93\n`; stderr `""` |
| SYN038-W122 | Prior NBA values are read through whole-variable ref and inout actuals; output checks both aliases and final state (O666–O667; +2 selected paths). Owned DB binding proof: `owned_prior_nba_writes_and_ref_inout_actuals_share_source_identity`. | `tests/fixtures/sim/syn038_pairwise/written_nba_ref_inout_actuals.sv` | `tests/sim_syn038_written_nba_ref_inout_actuals.rs::prior_nba_values_reach_whole_ref_and_inout_tasks_in_both_cli_modes` | `syn038_pairwise/written_nba_ref_inout_actuals`; `--edition 2009`; optimized and `--no-opt` | stdout `nba-ref-inout=5a>a5,5a>a5 final=a5/a5\n`; stderr `""` |
| SYN038-W123 | A one-time-unit settle allows a read-only ref task actual to observe a continuously driven `logic` variable; the owned DB proves the task reads but does not assign the formal, and that writer and actual bind to the same source (O676; +1 selected pair). Owned DB binding proof: `owned_continuous_driver_and_ref_actual_share_source_identity`. | `tests/fixtures/sim/syn038_pairwise/ref_read_continuous_variable.sv` | `tests/sim_syn038_ref_read_continuous_variable.rs::read_only_ref_of_continuously_driven_logic_runs_in_both_cli_modes` | `syn038_pairwise/ref_read_continuous_variable`; `--edition 2009`; optimized and `--no-opt`; `#1` precedes the call to settle the static continuous driver | stdout `ref-read=5a\n`; stderr `Warning: {absolute fixture path}:15:21 cannot mix continuous and procedural assignments to variable 'source'\n` |

These rows preserve the focal source distinctions recorded for each witness;
coverage of a type or operation does not imply coverage of every source slot
that uses it. The earlier W77–W104 rows retain their audited deltas and exact
oracles. W105–W123 add accepted event, reduction, return, initializer,
continuous-driver, port, call, and ref/inout source paths; W119–W123 add the
selected-input, const-ref, blocking ref/inout, NBA ref/inout, and settled
continuous-variable ref cases shown above. The two W96 applicability rules
remain in the checker. In total, every selected pair in the frozen denominator
has a checked-in source-bound observation and oracle. The checker reports 1,849
selected and covered pairs with zero legal gaps; this denominator result does
not by itself complete SYN-038/SYN-039 product acceptance.

The accepted 23-row legacy focal correction keeps `CP=function` on function
result sources rather than on separate receiving assignment targets. The
corrected target rows are O03/O11/O12/O18/O29/O32/O38/O44/O45/O47/O48/O50/O51/
O52/O53/O65/O66/O103/O119/O424–O427; they use `CP=none` and
`OP=direct_projection` for target-side focal values. The 18 previously accepted
sparse `CO` rows are W79 O329–O342 and W103 O582–O585. In the wider audit, 16
same-focal assignment RHS observations retain `CO=assignment_rhs`; 176
target-adjacent rows are sparse, and the 18 earlier address-only rows remain
sparse. `O332` and `O336` also omit `TY` because the outer unpacked arrays of
packed records have no matching outer type level. Literal same-focal RHS use,
not a write-target shape alone, is the basis for assignment-RHS coverage.

The zero-gap result is enforced by `required_zero_legal_gaps=true` in the
manifest and by the ledger gate. The final checker and manifest hashes are
recorded above. SYN-038's denominator is closed for this snapshot; final
selected-profile acceptance still depends on the composed gates and formal
review recorded in SYN-039.

#### SYN-038 selected-profile exclusions

These are explicit profile decisions. An excluded production is not an
unassigned Core requirement and is not counted as a failing positive cell.
The negative fixture, when present, checks the selected boundary; target or
verification extensions require a named SYN-034 child before they can enter
the Core denominator. SYN-021's finite packed tagged storage and SYN-025's
primitive/structure case forms have their own Extended evidence, and SYN-028's
fixed reverse/sort/rsort track is accepted as a separate Extended task; tagged
finite packed tagged predicate and case forms are implemented by SYN-024 and
remain in the selected Extended track; dynamic/native tagged forms are not
claimed complete here.

| ID | Edition | Annex A/B production family | Fixture / owner | Exclusion reason and profile boundary |
| --- | --- | --- | --- | --- |
| SYN038-EX-01 | SV2009 | `tagged_union_expression`, `cond_pattern`, `case_pattern_item` (B.3/B.8/B.9) | `tests/fixtures/sim/sequential_predicates/bad_matches_conditional.sv`; SYN-021–025 | Selected Extended tagged/pattern track; SYN-021 finite storage, SYN-024 finite packed tagged predicate/case forms and SYN-025 primitive/structure/tagged cases have separate evidence. Dynamic/native tagged forms remain outside this bounded track; no Core denominator. |
| SYN038-EX-02 | SV2009 | `queue_dimension`, `associative_dimension`, `dynamic_array_variable_identifier` (B.3/B.4) | `tests/fixtures/sim/data_types_next/dynamic_array_invalid_size.sv`; SYN-034 | Resizable/native storage has no portable Core RTL mapping. |
| SYN038-EX-03 | V2001/SV2009 | `non_integer_type`, runtime `real_type`, `string`, `chandle` (B.3) | `tests/fixtures/sim/data_types_next/inside_chandle_rejected.sv`; SYN-016 | Constants may elaborate; runtime real/native objects are outside Core. |
| SYN038-EX-04 | V2001/SV2009 | `procedural_continuous_assignment`, `force`/`release` extensions (B.7/B.9) | `tests/fixtures/sim/procedural_assign/real.sv`; SYN-034 | Target-dependent legacy drivers are excluded without named target evidence. |
| SYN038-EX-05 | V2001/SV2009 | `mos_switch_instance`, `pass_switch_instance`, resistive switch forms (B.24) | [`sim_gates_reject_switch_primitive`](../tests/sim_gates.rs) (`tests/sim_gates.rs`); SYN-034 | Device-level switch semantics are outside portable Core RTL; the inline test supplies the single-fault variable-terminal rejection. |
| SYN038-EX-06 | V2001/SV2009 | `sequential_body`, `edge_input_list` UDP forms (B.25) | `tests/fixtures/sim/partial_features/udp_sequential_rejected.sv`; SYN-031 | SYN-031 owns the accepted scalar combinational UDP track; sequential/device UDP needs separate implementation and remains outside Core. |
| SYN038-EX-07 | V2001/SV2009 | `specify_block`, `system_timing_check`, `path_delay_value` (B.26) | `tests/fixtures/sim/partial_features/negative_constant_delay.sv`; groups 17–19 | Timing/path/SDF behavior is simulation infrastructure, not Core synthesis. |
| SYN038-EX-08 | SV2009 | `class_declaration`, `program_declaration`, `clocking_declaration` (B.5/B.11/B.20) | `tests/fixtures/sim/classes/basic.sv`; SYN-034 | Verification/object/process containers are outside Core. |
| SYN038-EX-09 | SV2009 | `assertion_item`, `property_declaration`, `checker_declaration` (B.13/B.14) | `tests/fixtures/sim/concurrent_assertions/unsupported_assertion_argument.sv`; SYN-034 | Bounded simulation assertions do not establish target hardware synthesis. |
| SYN038-EX-10 | SV2009 | `constraint_declaration`, `randomize_call`, `randcase_statement` (B.15) | `tests/fixtures/sim/data_types_next/dynamic_array_invalid_size.sv`; SYN-034 | Solver/randomization facilities have no Core RTL mapping. |
| SYN038-EX-11 | V2001/SV2009 | `dpi_import_export`, `extern_tf_declaration`, VPI/PLI forms (B.29/B.30) | `tests/fixtures/sim/dpi/unsupported_vector.sv`; groups 70–71 | Foreign APIs are not synthesizable language requirements. |
| SYN038-EX-12 | V2001/SV2009 | `pla_system_task`, `dumpports_task`, `vcdclose_task` (B.17; Annex A §§A.7–A.8) | SYN-035; no Core fixture | PLA and extended VCD need target/file-format evidence and stay outside Core. |
| SYN038-EX-13 | SV2009 | `array_method_call` locator, `find*`, `min`, `max`, `unique` (B.4; Annex A §A.6) | `tests/fixtures/sim/fixed_array_reductions/nonintegral_map.sv`; SYN-034 | Fixed reductions/order methods have separate selected tasks; locator results are excluded. |
| SYN038-EX-14 | V2001/SV2009 | `par_block`, `join_keyword`, timed recursive `task_declaration` (B.6/B.10) | `tests/fixtures/sim/function/recursive_timed_task.sv`; groups 44–45 | Timed/parallel capture and recursive suspension are outside finite zero-time Core. |

#### SYN-038 72-group disposition

The original grouped inventory is a historical grouping, not the denominator
for this ledger. Each ID is classified once here using the plan's section 8
dispositions: `CORE` maps to one or more ledger rows; `RETAIN` keeps a bounded
existing simulator form; `EXT` belongs to a named selected Extended task;
`POLICY` is an explicit SYN-034/SYN-035 profile choice; `CAPACITY` is a
resource ceiling; `OUTSIDE` is not a Core synthesis requirement.

| Old ID | Group | Disposition | Evidence owner / boundary |
| ---: | --- | --- | --- |
| 1 | Charge-storage nets | OUTSIDE | SYN-034 target-mapped keeper/cell decision |
| 2 | Tagged unions | EXT | SYN-021 supports finite packed storage/construction/access; SYN-024 supports finite packed tagged predicate/case patterns, with primitive/structure/tagged case forms covered by SYN-025; dynamic/native forms remain outside the selected track |
| 3 | Real types | CORE + POLICY | SYN-016 constants; runtime real target extensions need SYN-034 |
| 4 | Strings | CORE + POLICY | SYN-016/SYN-029/SYN-030 constants/images; native runtime strings outside Core |
| 5 | Chandles | OUTSIDE | Native pointer/foreign handles |
| 6 | Structures and untagged unions | CORE | `TY-05`–`TY-07`, `AS-07`, SYN-012/SYN-013/SYN-015 |
| 7 | Fixed unpacked arrays | CORE + CAPACITY | `TY-03`, `TY-08`, `AS-07`–`AS-10`, SYN-001–008/012–015; SYN-036 ceiling |
| 8 | Resizable containers and array methods | EXT + POLICY | SYN-026–028; SYN-028 fixed ordering is accepted on current main; unrestricted resizing outside Core |
| 9 | Runtime enum methods | CORE | `TY-04`, SYN-016/SYN-038 finite numeric methods |
| 10 | Casts | CORE | `EX-09`–`EX-10`, SYN-015 |
| 11 | Data and array queries | CORE | `TY-12`, SYN-016/SYN-026; dynamic unsupported dimensions excluded |
| 12 | Initialization and storage lifetimes | CORE | `TY-09`–`TY-10`, `SB-08`, SYN-012/SYN-013/SYN-016 |
| 13 | Net resolution and strengths | CORE | `TY-02`, `AS-01`, `AS-09`, `PI-01`–`PI-02`, SYN-009/SYN-010 |
| 14 | Switch-level primitives | OUTSIDE | SYN-034 target selection required |
| 15 | Primitive instance arrays | RETAIN + EXT | `PI-01`; built-in gate arrays retained, combinational UDP arrays follow SYN-031 |
| 16 | User-defined primitives | EXT | SYN-031 combinational UDP; sequential UDP outside Core |
| 17 | Specify blocks | OUTSIDE + POLICY | SYN038-EX-07; no Core timing path; static legacy specparams require SYN-034 selection |
| 18 | Timing checks | OUTSIDE | SYN038-EX-07; Boolean predicate `&&&` is distinct |
| 19 | SDF annotation | OUTSIDE | No synthesis-subset dependency |
| 20 | Gate terminal forms | CORE | `PI-01`–`PI-02`, SYN-008/SYN-009; typed builtins retained |
| 21 | Continuous and gate delays | RETAIN | Constant simulator delays; dynamic timing outside Core |
| 22 | Delay expressions | CORE + POLICY | Constant/edition forms in SYN-016/SYN-019; runtime timing extensions outside Core |
| 23 | Time precision and literal semantics | CORE + RETAIN | `LX-03`, `LX-06`, SYN-019; existing scheduling support retained |
| 24 | Port connections | CORE | `HY-02`, `HY-09`, `AS-09`–`AS-10`, SYN-007–010/014 |
| 25 | Libraries and configurations | EXT | SYN-032 selected design assembly |
| 26 | Packages | CORE | `HY-07`, SYN-013/SYN-016 |
| 27 | Net aliases | CORE | `AS-09`, SYN-010 strict true-alias rules |
| 28 | SystemVerilog scheduling regions | RETAIN | `PR-01`–`PR-03`; full verification regions outside Core |
| 29 | Zero-delay process behavior | CORE + CAPACITY | `PR-01`–`PR-10`; SYN-036 budget boundary |
| 30 | Conditional event controls | CORE | `PR-02`, SYN-011/SYN-014 finite numeric helpers |
| 31 | Named-event references | OUTSIDE | Testbench synchronization, no Core backlog |
| 32 | Array/container sensitivity | CORE | `PR-02`–`PR-03`, SYN-005/SYN-007/SYN-014 |
| 33 | Intra-assignment timing | RETAIN | Existing NBA capture; timing-bearing/native forms excluded |
| 34 | Force and release | OUTSIDE + POLICY | General forcing is outside Core; SYN-034 selected static/legacy target required |
| 35 | Procedural assign/deassign | POLICY | SYN-034 bounded legacy target required |
| 36 | Named disable | CORE | `PR-09`, SYN-037 local finite control |
| 37 | Loop forms and captures | CORE | `PR-04`–`PR-06`, SYN-037 |
| 38 | Uniqueness and priority checking | CORE | `PR-08`, SYN-011/SYN-025 finite checks |
| 39 | Pattern matching | EXT | SYN-022–025; SYN-024 finite packed tagged predicate/case forms and SYN-025 primitive/structure/tagged cases accepted; dynamic/native forms remain excluded and the Core row records that profile boundary |
| 40 | Side-effecting operators | CORE | `AS-04`, canonical mutation path |
| 41 | Set membership | CORE | `EX-11`, SYN-005 fixed value expressions |
| 42 | Streaming | CORE | `EX-10`, SYN-015 admitted fixed forms |
| 43 | Reference subroutine arguments | CORE | `SB-03`, `SB-08`, SYN-013/SYN-015 legality |
| 44 | Recursive timed tasks | OUTSIDE | SYN038-EX-14; finite zero-time recursion is `SB-07` |
| 45 | Parallel subroutine bodies | OUTSIDE | SYN038-EX-14; ordinary zero-time calls retained |
| 46 | Cross-instance subroutine calls | CORE | `HY-06`, `SB-01`, SYN-013/SYN-016/SYN-018 |
| 47 | Subroutine copy-out and storage | CORE | `SB-02`–`SB-08`, SYN-012/SYN-013 |
| 48 | File I/O | OUTSIDE | Host scanning/I/O; memory images are SYN-029/SYN-030 |
| 49 | Display families and formatting | RETAIN + OUTSIDE | Simulation diagnostics retained; aggregate formatting and monitor behavior are outside Core |
| 50 | String formatting tasks/functions | RETAIN + POLICY | Constant elaboration may use strings; runtime diagnostic facilities retained outside Core |
| 51 | Memory file loading and writing | EXT | SYN-029/SYN-030 selected initialization profile |
| 52 | Real-time reporting and time formatting | RETAIN + OUTSIDE | Simulation reporting retained, not Core syntax |
| 53 | Simulation suspension | OUTSIDE | Interactive `$stop`/debugger behavior |
| 54 | PLA modeling | POLICY | SYN-035 pending named legacy target and artifact |
| 55 | Stochastic queues | OUTSIDE | Simulation statistics/queues |
| 56 | Random-number facilities | OUTSIDE | Simulator RNG does not imply synthesizable RNG |
| 57 | Command-line plusargs | OUTSIDE | Host input, not a constant parameter override |
| 58 | Runtime mathematical functions | CORE + POLICY | Constant folding in SYN-016; real hardware target needs SYN-034 |
| 59 | Runtime severity tasks | RETAIN + OUTSIDE | Elaboration diagnostics and simulation checks retained outside Core |
| 60 | Host command execution | OUTSIDE | `$system` is host process control |
| 61 | Waveform selection and extended VCD | RETAIN + OUTSIDE | `$dumpvars` companion retained; extended VCD excluded |
| 62 | Classes | POLICY | SYN-034 finite static/elaboration decision; general heap outside Core |
| 63 | Program blocks | OUTSIDE | Testbench reactive construct |
| 64 | Clocking declarations, sampling and synchronous drives | OUTSIDE | Verification profile; no Core clocking grammar |
| 65 | Advanced interprocess synchronization | OUTSIDE | Mailbox/semaphore/process APIs |
| 66 | Assertions and sampled values | RETAIN + POLICY | Simulation checks retained; target assertion synthesis needs SYN-034 |
| 67 | Checkers | OUTSIDE | Verification container |
| 68 | Functional coverage | OUTSIDE | Coverage database/bins |
| 69 | Constrained and structured randomization | OUTSIDE | Solver and randomization |
| 70 | DPI | OUTSIDE | Foreign-code calling convention |
| 71 | PLI/VPI | OUTSIDE | External introspection/control APIs |
| 72 | Virtual interfaces | CORE + OUTSIDE | Concrete interfaces are `HY-08`; runtime virtual handles outside Core |

No Core row is left without a fixture or an explicit single-fault rejection
control. The old inventory's `Missing`/`Partial` labels remain source-status
descriptions below and are not copied into the selected Core denominator. A
new legal failing cell found while executing this ledger must receive a new
SYN child and dependency; it must not be converted into a broad `Partial`
label or silently treated as an exclusion.

### SYN-039 selected-profile acceptance status — Linux WSL2 host gate passed

The SYN-038 value-pair denominator is frozen at **2,247 raw / 1,849 selected /
1,849 covered / 0 legal gaps / 296 impossible / 102 outside-profile**. The
checker SHA-256 is
`b1ccaad76c1488195b55923ce5391c748cfe94cb6879b61215064ace4d43ea6e`; the
manifest SHA-256 is
`941e2f1ab12064cfa820686e36ba2e29bbc8a418940c26e265f3d1edfe460757`.
`required_zero_legal_gaps=true` is enforced by the checker and ledger, and a
witness-removal mutation was rejected. This closes the audited denominator for
this source snapshot. The composed serialized all-features run passed for the
bounded selected profile on the stated Linux WSL2 host. This does not qualify
all synthesizable Verilog/SystemVerilog or other platforms.

The current frozen source identity is committed HEAD
`179bd76fdd72683cd3b1b9c2056444848c867127`, with tree
`d3195386ac77e61950bd2cb8d60890f0407063fc`. Its four scoped commits are
`554a343` (frontend), `ec7032b` (simulator repairs and held-out fixtures),
`97ba0df` (SYN-038 witnesses), and `179bd76` (checker, manifest, and ledger).
The earlier `c2824c38babd55beb5632af25147f03f85ff55fb1786dea2a2907080a61f7d4f`
working-tree digest on base `b4874776ede211437123f6c1d0505e67c3cb92cf` is
historical pre-commit provenance only; it is not the current source identity.
The serialized `cargo test --locked --all-features --no-fail-fast -- --test-threads=1`
run is recorded in
`persistence/synth-review-full-suite-20260924T1515.log`: **2,756 passed, zero
failed, one ignored across 242 targets**. The validation host was
`Linux 6.6.87.2-microsoft-standard-WSL2 x86_64`
with rustc 1.98.0 (2026-08-18), Cargo 1.98.0 (2026-08-05), Debian cc/GCC
14.2.0, and CMake 3.31.6. The earlier all-features attempt, before the W123
ledger-contract and interface-storage fixes, reported 2,754 passed, 2 failed
and 1 ignored across 242 targets. Both failures passed in the completed run on
the committed source identity above.

The static source gates passed on this host: `cargo fmt --all -- --check`,
`git diff --check`, `cargo check --locked --all-targets --all-features`,
`cargo clippy --locked --all-targets --all-features -- -D warnings`,
`cargo build --locked --bin llg --bin llg_ls --bin elab_check`, and
`cargo check --locked --lib --no-default-features`. The tracked fixture scan
found 902 references across 851 paths with zero errors. These checks establish
format, compilation, warning cleanliness, no-default library build and fixture
integrity and complement the completed serialized test gate.

The held-out review evidence is host-scoped. At 15:30 UTC, the settled-source
`llg` replay ran R01–R09 in SystemVerilog-2009 in both optimizer modes; the R10
mux-policy case additionally ran in Verilog-2001 in both modes. All 36
return-code/stdout/stderr outcomes matched the reviewed run after normalizing
only temporary-directory prefixes. Of these, 22 matched positive oracles and
14 retained their documented manual-review policy or diagnostic status; there
were no timeouts. The separate R11 generation-only 65,537-cell cap replay
matched 2/2 optimizer-mode outcomes.

R11 documents a capacity deviation. The supplied IEEE editions require support
for arrays of at least 16,777,216 elements; the selected generated fixed-array
limit is 65,536 cells. The 65,537-cell fixture is a generation-only rejection
control and does not simulate the standard minimum. The product therefore does
not claim minimum-capacity conformance above its disclosed 65,536-cell ceiling.
No conclusion is drawn for larger arrays or other capacities that were not run.

R13 adds two SystemVerilog-2009 public-CLI composition witnesses in
`sim_review_bundle_composition`; each runs optimized and `--no-opt` with exact
output checks:

| Fixture / owner | Composition | Both-mode oracle |
| --- | --- | --- |
| `r13_recursive_pattern_function_port.sv`; `sim_review_bundle_composition::recursive_type_keys_cross_function_return_array_port_and_comb_logic` | Recursive type keys through an automatic fixed-array function return, aggregate input port and `always_comb` reader. | stdout `recursive pattern function/port passed: 34\n`; stderr `""` |
| `r13_record_pattern_selected_nba.sv`; `sim_review_bundle_composition::record_deconstruction_captures_selected_nba_target_and_source` | Packed-record deconstruction into a runtime-selected destination through NBA scheduling, with selector and source changes before commit. | stdout `selected record NBA scatter passed: 12/34\n`; stderr `""` |

The focused R13 owner passed 2/2 tests on the stated Linux WSL2 host. This is
bounded composition evidence, not a synthesis-tool result or native
Windows/macOS qualification.

Generated-model sanitizer evidence is separate from the full suite. Focused GCC
ASan/UBSan runs across seven snapshots passed at 12:36 (18 tests/18 model logs),
13:04 (4/4), 13:11 (2/2), 13:13 (2/2), 14:02 (10/10), 14:15 (10/10, including
the post-fix W123 run), and 15:26 (2/2 `sim_syn038_union_interface` modes after
the interface-storage fix). These snapshots total 48 test executions and 48
inspected per-model logs; some suites recur across snapshots, so 48 is not a
count of unique tests or a full sanitizer lane. The
logs contained only the known libaco coroutine stack-switch warning, with no
ASan/UBSan/LSan error or summary. These runs instrument generated C models, not
the vendored Slang archive. Handwritten native C probes are separately scoped
and are not included in this generated-model sanitizer count.

The selected profile covers the exact legal Verilog-2001/SystemVerilog-2009
forms in the checked-in SYN-038 ledger and the separately named finite
extensions. Held-out R01–R09 use SV2009; R10 additionally uses V2001; R11 is a
separate SV2001 generation-only boundary; R13 uses SV2009. This evidence does not qualify
native Linux distributions, Windows or macOS, other compiler/runtime versions,
or synthesis-tool output. Acceptance here is limited to the checked-in selected
profile on the stated Linux WSL2 host.

The composed suite contains four positive public-CLI witnesses and one
neighboring single-fault rejection:

| Witness | Composition |
| --- | --- |
| `array_record_datapath` | Fixed array of unpacked records through value ports, a zero-time function, `always_comb`, and `always_ff`. |
| `interface_generate_memory` | Independent parameterized interface instances, finite initialized memories, generated structural links, and runtime address changes. |
| `alias_wired_multi` | Two instance drivers through a true alias and a `wand` resolver, including all-Z, single-driver and conflicting-driver states. |
| `extended_top` with `syn039_select:config` | Selected library/configuration binding below a combinational UDP and finite tagged pattern-case datapath. |
| `unsupported_sequential_udp` | Sequential UDP outside the selected combinational UDP profile; expected rejection. |

`sim_syn039_acceptance`, `sim_rtl_composition`, `sim_rtl_completion`,
`sim_sequential_predicates`, `sim_udp`, `sim_syn032_library_configs`,
`sim_net_resolution`, `sim_memory_views`, `sim_syn036_capacity`, the repaired
SYN-038 ledger, the pairwise public-CLI owners, and the review-bundle suites are
part of the composed selected-profile gate. The serialized all-features run
passed with 2,756 tests passed, zero failed and one ignored across 242 targets.

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
- 🟨 **Array declaration initializers** `'{…}` — §1800-2009 7.6/10.9.1 **[SV-2005]** fixed integral array values use declaration-order element correspondence and typed defaults, including admitted runtime zero-time calls in the 2009 pre-process phase. This is no longer a constant-elements-only capability. Nested-default corrections retain repeated bound operands and packed-vector leaf ranges; the corrected cases passed in the recorded pre-SYN038 Linux baseline. Unsupported native/resizable element layouts remain outside this claim (sim_memory.rs, sim_rtl_completion.rs, sim_array_conditional_assignments.rs).
- 🟨 **Fixed unpacked array values, assignments and reductions** — §§1800-2009 7.4, 7.6, 7.12.3, 11.4.11 **[SV-2005]** R01 retains array-specific conditional merging; R04 admits bounded type-compatible module-procedural expressions with one RHS snapshot; R03 implements integral reductions and lexical `with` maps without converting fixed arrays to resizable containers. Whole copies, overlapping slices, partial-index rows, reversed/negative bounds and fixed integral subroutine/port paths are represented. SYN-036 caps generated fixed-array storage at 65,536 cells and keeps direct rank-one reductions cell-wise; value/formal/stream contexts require one packed payload and report that measured capacity. Refer to §5 for assignment boundaries and §7 for operator/method boundaries. Native/resizable aggregate elements, unsupported shapes and values exceeding the bounded payload path remain restricted (sim_p30_fixed_arrays.rs, sim_rtl_completion.rs, sim_array_conditional_assignments.rs, sim_fixed_array_reductions.rs, [SYN-036](../tests/sim_syn036_capacity.rs)).
- 🟦 **enum-typed scalar variables and runtime methods** — §1800-2009 6.19/6.19.5 **[SV-2005]** stored at the elaborated packed base width; enum constants fold through the frontend, and declaration-order first/last/next/prev/num/name methods retain sparse signed values, wrapping step counts, owned names, and the four-/two-state invalid-value defaults (sim_operator_semantics.rs, sim_data_types_next.rs)
- 🟨 **packed/unpacked struct and union aggregates** — §1800-2009 7.2–7.3, 7.4 **[SV-2005]** packed patterns retain positional/named/default/type-key forms, exact state/signedness and member overrides. Packed unions use overlapping width-matched views; fixed recursive unpacked struct/array leaves have initialization, deep copy and member paths for represented packed, real, string and chandle storage. Unequal-width unpacked untagged unions use the maximum packed member extent. SYN-021 supplies finite packed tagged storage, construction and access; SYN-024 supplies finite packed tagged predicate/case patterns with exact tag guards, while SYN-025 covers primitive/structure/tagged pattern-case forms. Dynamic/native tagged payloads and unsupported declaration/element layouts, native/resizable aggregate subroutine storage and general slices remain restricted. Recursive type identity is present, not a missing blanket capability (sim_data_types_completion.rs, sim_partial_features/ports.rs).
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

- 🟨 **Whole/selected fixed-array procedural assignment** `result = sel ? a : b;` / `result <= sel ? a : b;` — §§1800-2009 7.6, 10.4, 11.4.11 **[SV-2005]** R04 admits bounded fixed integral expressions by owned result shape after the existing copy/slice/concatenation/pattern/cast paths. It captures the full RHS once before stores or NBA issue, preserving overlapping assignments, declared element order and specialized per-cell conversions. R01 supplies aggregate ambiguous-selector merging. The separate continuous-assignment/intra-assignment timing paths are unchanged; illegal automatic/reference-formal NBA destinations remain rejected. The nested-default/NBA corrections passed in the recorded pre-SYN038 Linux baseline (sim_array_conditional_assignments.rs, sim_p30_fixed_arrays.rs).
- 🟨 **Assignment-pattern values and positional lvalues** `'{…}`, integer/member keys and `default` — §§1800-2009 10.9.1–10.9.2 **[SV-2005]** existing fixed-array/record paths retain element context, missing/duplicate-key checks and declaration order. The corrections export bound nested defaults without error placeholders and preserve repeated positional edges when elements share one expression node. Bounded packed-vector leaves support positional/integer/default keys using declared `ranges` and checked bounds; typed row defaults and untyped recursive bit defaults remain distinct. Fixed unpacked-array type keys now match owned integral, typedef, row and record descriptors with explicit-index > type-key > default precedence; nested fixed integral arrays/records, reversed/negative bounds and declaration/local/argument/return/NBA contexts are covered by `sim_syn001_type_keys.rs` in both optimizer modes. Replicated assignment-pattern values now expand their constant count through the owned pipeline for fixed unpacked arrays and fixed integral aggregates, preserving syntactic order and repeated positions across nested rows, packed multidimensional values, typedef-qualified values, reversed bounds, declaration, argument/return, and NBA contexts; the public regression covers both optimizer modes (`sim_p30_fixed_arrays.rs`). Positional assignment-pattern lvalues now use typed fixed-array scatter plans for plain and typedef-qualified targets, nested positions, selected fixed-array cells, reversed source bounds, overlap-safe one-time RHS capture, pattern RHSs, and blocking/legal persistent-storage NBA contexts; `sim_syn003_pattern_lvalues.rs` covers both optimizer modes. Frontend-invalid zero counts and shape mismatches remain rejected, source side-effect evaluation count is not promised for replication, keyed/default/replicated lvalue forms, width/type mismatches, constant destinations, automatic/reference-formal NBA targets, packed-vector type-key matching, and general native/resizable aggregate shapes remain restricted.
- 🟦 **Sequential Boolean predicates** `if (a &&& b)` — §§1800-2009 12.6.2–12.6.3 **[SV-2005]** R06 stores all clauses in source order and keeps branch references separate. Each reached clause is evaluated once; only definite true advances, while false or ambiguous truth stops. Ambiguous statement predicates take else. Numeric truth uses the whole packed value or real nonzero comparison; unique/priority handling remains attached to the statement. This is distinct from ordinary logical `&&` and from timing-check conditions (sim_sequential_predicates.rs).
- 🟨 **Conditional pattern execution and bindings** `matches`, pattern variables — §1800-2009 12.6, 12.6.2-12.6.3 **[SV-2005]** integral constant patterns, `.*` wildcards and `.name` bindings execute in `if`/`?:` predicates with source-ordered `&&&`, defined match Booleans, automatic binding storage, later-clause/true-arm scope and effect/dependency/lint coverage. SYN-023 adds recursive fixed packed/unpacked structure patterns with resolved member checks. SYN-024 adds finite packed tagged discriminant/payload patterns with exact active-tag guards, void/primitive/fixed-structure/nested payloads and automatic bindings. SYN-025 adds primitive/structure/tagged `case`/`casez`/`casex ... matches` items, per-item filters, first-match/default behavior and unique/priority checks with one selector capture; dynamic/native tagged payloads remain fail-closed (sim_sequential_predicates.rs, sim_semantic.rs).
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
- 🟨 **Conditional** `?:` — §1364-2001 Table 28 / §1800-2009 Table 11-20 **[2001/SV-2009]** packed known selectors choose one arm; with an ambiguous selector, each bit follows the published table: equal 0, 1, or X pairs survive, while Z/Z and other differing pairs become X. R06 also corrects real-valued alternatives to evaluate both and return zero for an ambiguous selector. Fixed unpacked array/record merging has its separate SystemVerilog rule and implementation path (sim_operator_semantics.rs, sim_conditional_policy.rs, sim_sequential_predicates.rs).
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
- 🟨 **Fixed-array reverse method** `.reverse()` — §1800-2009 7.12.2 **[SV-2009]** SYN-027 reverses admitted immediate fixed integral or unpacked-record elements in declaration order, retaining ascending, descending and negative unpacked bounds. Rows remain whole immediate elements when reordered. The lowering captures every source element before publishing normal blocking writes, so selected rows and overlapping storage are safe; automatic locals/formals use their owned payload. `with` clauses, const-ref receivers, native/real/string elements and unsupported payload representations remain rejected (sim_fixed_ordering_review.rs, sim_data_types_completion.rs, methods.rs).
- 🟨 **Fixed-array sort methods** `.sort()` / `.rsort()` — §1800-2009 7.12.2 **[SV-2009]** SYN-028 emits a checked finite comparison schedule for immediate fixed integral elements and for records with an integral `with` key, including selected row arrays. Signed, unsigned and enum keys retain declared values and bounds; complete source elements are captured before each overlapping swap, so non-key record fields move with the key. Repeated-key stability is unspecified; a record without an admitted integral comparison key, shuffle, locator result queues, native/real/string elements, incompatible maps and const-ref receivers remain rejected (sim_fixed_ordering_review.rs, sim_data_types_completion.rs, methods.rs).
- 🟨 **Sequential Boolean predicates in conditional values** `a &&& b ? t : f` — §§1800-2009 11.4.11, 12.6.3 **[SV-2005]** R06 uses the same ordered truth sequence as `if`; a later clause is not evaluated after false or ambiguous truth. An ambiguous result evaluates both arms and uses the existing packed, R01 aggregate or real-zero result rule. `Predicate` IR, folding, effects and dependency collection retain clause order; primitive integral `matches` clauses, SYN-023 recursive fixed structure clauses and SYN-024 finite packed tagged clauses use the same owned sequence, while dynamic/native tagged payloads remain rejected (sim_sequential_predicates.rs).
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
- ❌ **Sequential UDPs** level/edge-sensitive — §1364-2001 8.3–8.5 **[1995]** remain outside the selected UDP profile; SYN-031 accepts scalar combinational UDPs, while sequential state/edge table semantics require separate work
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
- 🟨 **fork/join inside function/task bodies** — §1364-2001 9.8.2 / §1800-2009 9.3.2 **[1995/SV-2005]** detached `join_none` branches capture automatic packed locals and formals in owned activation frames. A bounded nested-task regression also covers synchronous `fork ... join`: automatic numeric cells use the suspended activation frame, joined automatic output updates propagate to the caller, and event waiters on persistent static task locals/formals observe changes to their signal-backed storage. `sim_syn038_nested_task_fork_events.rs` runs `nested_task_fork_static_event.sv` (`static-event=1\n`), `nested_task_fork_formal_event.sv` (`formal-hier-event=1\n`), and `nested_task_fork_real_join.sv` (`real=2.5\n`) under SV2009 with and without optimization; the first two assert exact source-located upward-hierarchy warnings. `join_any`/`join_none` retain snapshot behavior, and this evidence does not claim detached automatic alias propagation or new `ref`-formal capture behavior. Ordinary blocking timing in functions, recursive timed tasks, and richer subroutine activation forms remain rejected.
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

- 🟦 **$readmemh/$readmemb** — §1364-2001 17.2.8 / §1800-2009 21.4 **[1995/SV-2005]** whitespace/comment-separated binary and hexadecimal files, four-state digits, and `@` address jumps are implemented for fixed packed memories with one or more unpacked dimensions. A constant higher-dimension selection retains a legal lower-dimensional view; file words walk each remaining dimension low-to-high in row-major order while declaration direction determines flat storage offsets. Verilog-2001 one-dimensional omitted ranges follow declaration direction; SystemVerilog-2009 omitted ranges follow low-to-high address order, including start-only calls; explicit ranges retain their requested direction. Runtime-selected memory views capture their selectors once, and selected fixed rows/slices accept explicit start/finish addresses; a range outside the selected destination is diagnosed and leaves all cells unchanged. `@` jumps select the highest remaining dimension, incomplete final rows leave unwritten subwords unchanged, and invalid selected ranges or jumps terminate the load while retaining prior writes. For enum memories, each known numeric word is checked against the enum base width before conversion; redundant sign extension is accepted for signed bases, while an invalid word emits a nonfatal simulation diagnostic and stops the load without undoing earlier writes. Ordinary packed memories retain their normal truncation behavior. Two-state X/Z words are diagnosed and converted to zero. Dynamic/queue/associative memories, real or non-packed aggregate elements, and fully indexed scalar memory elements remain rejected (sim_memory_views.rs, sim_memory_editions.rs, sim_memory.rs)

- **SYN-030 selected memory-view acceptance (R09)** — Public-CLI cases cover selected fixed slices and runtime-selected rows, verify selector side effects occur once, and check that an explicit start/finish range outside a selected slice is diagnosed while every memory cell remains unchanged. The cases use SystemVerilog-2009 and exact stdout/stderr oracles in both optimizer modes (`tests/sim_memory_views.rs::review_bundle_readmem_accepts_slices_and_runtime_rows`, `runtime_memory_view_selector_is_evaluated_once`).

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
| 2 | Partial | Tagged unions | SYN-021 supports finite packed tagged storage, construction and access. SYN-024 supports finite packed tagged predicate/case patterns with exact tag guards; SYN-025 covers primitive/structure/tagged pattern-case forms. Dynamic/native payloads and broader tagged contexts remain restricted. | [aggregates.rs](../src/sim/codegen/lowering/collection/aggregates.rs), [predicates.rs](../src/core/db/database/predicates.rs), [statement_import.rs](../src/core/db/database/statement_import.rs) |
| 3 | Partial | Real types | Scalar/fixed-array real storage, ports, value-formal calls, arithmetic, waits/events, changed-write notification and typed formatting are present. Generic container real/shortreal leaf operations exist. Addressable automatic real locals use registered heap payloads and real publication pins them. Queued automatic-real destinations, general real-reference calls and real-valued sampled/method callback results remain restricted. | [owned.rs](../src/sim/emit_c/owned.rs), [stores.rs](../src/sim/emit_c/owned/stores.rs), [dependencies.c](../src/sim/rt/scheduler/dependencies.c) |
| 4 | Partial | Strings | Owned strings now have module/static/automatic storage, input/output/inout/ref/const-ref call paths, returns, core methods, conversion and dynamic formatting. Collected string value ports and contents dependencies exist. Automatic string NBA targets, unsupported native captures/automatic monitors and broader aggregate/continuous forms remain; string formals and ports are no longer blanket-missing. | [strings.rs](../src/sim/emit_c/owned/strings.rs), [model.rs](../src/sim/emit_c/owned/model.rs), [ports.rs](../src/sim/codegen/lowering/collection/ports.rs) |
| 5 | Partial | Chandles | Native chandle null/copy/identity/Boolean operations, automatic/static locals, admitted aggregate/class fields, typed mixed signatures and input/output/inout/ref/const-ref/return paths exist. Matching collected reference-port leaves can share storage; ordinary chandle value-port links remain rejected. Packed containment, arithmetic, general continuous assignment and non-string object sensitivity are outside the bounded path. | [objects.rs](../src/sim/emit_c/owned/objects.rs), [ports.rs](../src/sim/codegen/lowering/collection/ports.rs) |
| 6 | Partial | Structures and untagged unions | Packed patterns/overlapping union views, recursive fixed unpacked leaves, deep copy, explicit member defaults and represented unequal-width unpacked untagged-union storage are present. SYN-012 closes the local packed-aggregate member projection gap and records legal packed struct, equal-width packed union, fixed unpacked struct, untagged union view and nested array-of-record contexts at widths 1/7/8/31/32/33/64/65/129, with mixed signed/unsigned `bit`/`logic` and reversed bounds, through declaration/local/member/row, equality/conditional/assignment/return cells. SYN-021 supplies finite packed tagged storage, construction and access; SYN-024 supplies finite packed tagged predicate/case patterns, while SYN-025 covers primitive/structure/tagged pattern-case forms. Dynamic/native tagged payloads, unsupported declaration/net layouts, general native/resizable subroutine storage and arbitrary slices remain. | [aggregates.rs](../src/sim/codegen/lowering/collection/aggregates.rs), [fixed_projections.rs](../src/sim/codegen/lowering/collection/fixed_projections.rs), [fixed_values.rs](../src/sim/codegen/lowering/collection/fixed_values.rs), [fixed_patterns.rs](../src/sim/codegen/lowering/collection/fixed_patterns.rs), [SYN-012 matrix](../tests/sim_syn012_fixed_layout.rs) |
| 7 | Partial | Fixed unpacked arrays | Bounded copies, concatenations, slices and partial-index views retain logical coordinates and captured indices. R01 provides fixed-array conditional element/default semantics; R04 stages type-compatible module-procedural expressions before cell stores/NBA issue; R03 adds integral reductions and lexical fixed-record/row maps. SYN-027 adds declaration-order `reverse()` for immediate integral or fixed unpacked-record elements, including selected rows; SYN-028 adds `sort()`/`rsort()` for integral elements or an integral `with` key selected from a fixed record, including selected rows. Reversed/negative bounds, repeated keys and automatic local/formal payloads are covered; rows move as whole immediate elements. SYN-007 accepts compatible fixed integral input-port value expressions, including conditional/function/pattern values, fixed slices and runtime-selected rows, with one source capture and declaration-order scatter. SYN-008 qualifies bounded fixed integral output whole/row/slice links, matching fixed-array references, aggregate output values, nested member targets and instance-array distribution. Bound nested patterns preserve repeated operand slots and packed leaf ranges, and the corrected NBA/default-pattern cases passed in the recorded pre-SYN038 Linux baseline. General native/resizable element, real-array expression, runtime output/ref/inout rewiring, subroutine and over-limit value combinations remain restricted. | [fixed_arrays.rs](../src/sim/codegen/lowering/containers/fixed_arrays.rs), [ports.rs](../src/sim/codegen/lowering/collection/ports.rs), [methods.rs](../src/sim/codegen/lowering/containers/methods.rs), [fixed_patterns.rs](../src/sim/codegen/lowering/collection/fixed_patterns.rs), [conditionals.rs](../src/sim/codegen/lowering/expressions/conditionals.rs), [fixed_reductions.rs](../src/sim/codegen/lowering/containers/fixed_reductions.rs), [sim_fixed_ordering_review.rs](../tests/sim_fixed_ordering_review.rs), [sim_rtl_completion.rs](../tests/sim_rtl_completion.rs), [sim_feature_completion_g1.rs](../tests/sim_feature_completion_g1.rs) |
| 8 | Partial | Resizable containers and array methods | Dynamic/queue/associative allocation, copy, resize, defaults, traversal, bounded patterns, generic/nested leaves and collected value-port copies have source paths. Packed callbacks, reductions, locator/min/max/unique and ordering operations are present; their method metadata survives semantic-table growth. Fixed integral reductions use the separate R03 lexical path in group 7; that does not enable automatic captures for resizable callbacks. Non-packed endpoint/pop expressions, string-key index results, general subroutine/nested scalar-query forms and broader recursive/object combinations remain gated. | [containers/](../src/sim/codegen/lowering/containers/), [containers.rs](../src/sim/emit_c/owned/containers.rs), [containers/](../src/sim/emit_c/owned/containers/) |
| 9 | Source-implemented | Runtime enum methods | Scalar enum first/last/next/prev/num/name methods use owned declaration-order metadata, including sparse/signed values, wrapping counts, invalid-value defaults and owned names. | [queries.rs](../src/sim/emit_c/owned/objects/queries.rs), [strings.rs](../src/sim/emit_c/owned/strings.rs) |
| 10 | Partial | Casts | Scalar packed/real dynamic casts with success-only writes and enum membership, checked nominal class casts, fixed-size aggregate/array and packed-element dynamic/queue bit-stream paths exist. Native-string/object/recursive stream forms and unsupported reference targets remain restricted. Class downcasts are no longer blanket-missing. | [casts.rs](../src/sim/codegen/lowering/expressions/casts.rs), [queries.rs](../src/sim/emit_c/owned/objects/queries.rs), [objects.rs](../src/sim/emit_c/owned/objects.rs) |
| 11 | Partial | Data and array queries | Executed `$typename`, `$isunbounded`, `$bits`, `$left`/`$right`/`$low`/`$high`/`$increment`/`$size`/`$dimensions`/`$unpacked_dimensions` with descriptor-backed packed, fixed-array, dynamic/queue, associative-integral, and string metadata; nested runtime container dimensions and string/wildcard associative bounds remain unsupported. | [array_queries.rs](../src/sim/codegen/lowering/expressions/array_queries.rs), [queries.rs](../src/sim/emit_c/owned/objects/queries.rs) |
| 12 | Partial | Initialization and storage lifetimes | Declaration identity, lifetime and edition-specific initialization are represented. Scalar and fixed integral composite initializers admit zero-time user calls before SystemVerilog processes; static storage initializes once and automatic storage per activation. Explicit member defaults, mixed state domains and nested bound pattern operands have paths. The corrected packed-default path passed in the recorded pre-SYN038 Linux baseline. Timing-bearing calls, general native/resizable aggregate/subroutine layouts and ambiguous lifetime provenance remain restricted. | [initialization.rs](../src/sim/codegen/lowering/collection/initialization.rs), [declarations.rs](../src/sim/codegen/lowering/statements/declarations.rs), [calls.rs](../src/sim/emit_c/owned/calls.rs) |
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
| 39 | Partial | Pattern matching | Integral constant, `.*` wildcard and `.name` binding patterns, recursive fixed packed/unpacked structure patterns, and finite tagged-union discriminant/payload patterns execute in conditional `if`/`?:` predicates and SYN-025 `case`/`casez`/`casex ... matches` statements with source-ordered filters, one selector capture, resolved member identities, defined Boolean results, automatic lexical bindings and unique/priority diagnostics. Tagged discriminants retain exact four-state matching under wildcard case modes; dynamic/native payloads remain restricted. | [predicates.rs](../src/core/db/database/predicates.rs), [conditionals.rs](../src/sim/codegen/lowering/expressions/conditionals.rs), [statement_import.rs](../src/core/db/database/statement_import.rs), [control_flow.rs](../src/sim/codegen/lowering/statements/control_flow.rs), [control.rs](../src/sim/emit_c/owned/control.rs) |
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
| 51 | Source-implemented | Memory file loading and writing | Fixed packed memories with multiple unpacked dimensions and selected rows/slices use one owned view descriptor. R09 adds runtime-selected views with selector capture once and selected-slice start/finish bounds checks that diagnose without modifying cells. Remaining dimensions load and write in low-to-high row-major order, declaration direction maps to flat storage, `@` addresses the highest dimension, incomplete final rows preserve unread subwords, and explicit ranges preserve source direction. Verilog-2001 one-dimensional defaults keep declaration order; SystemVerilog-2009 defaults use low-to-high order. R08 checks known enum words against base width before conversion, accepts redundant sign extension for signed bases, and stops at an invalid word while retaining earlier writes; ordinary packed memories keep normal truncation. Dynamic/queue/associative memories, real or non-packed aggregate elements, and fully indexed scalar memory elements remain restricted, as are writers in Verilog-2001 mode. Paths accept owned string expressions. | [runtime_tasks.rs](../src/sim/emit_c/owned/runtime_tasks.rs), [memory_io.c](../src/sim/rt/scheduler/memory_io.c), [sim_memory_views.rs](../tests/sim_memory_views.rs) |
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
