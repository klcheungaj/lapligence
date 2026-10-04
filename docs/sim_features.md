# Simulator Feature Checklist — IEEE 1364 / IEEE 1800

This is the maintained `llg` simulation-feature inventory. Support is limited to
the forms and contexts stated below; it does not imply full IEEE conformance or
synthesis-tool acceptance.

<a id="dynamic-value-migration-acceptance-boundary"></a>

<a id="status-markers"></a>

## Status and review basis

**Source review: 2026-09-27, supplied project snapshot.** This inventory follows
frontend admission and the owned elaborated database through semantic checks,
IR/lowering, whole-model C emission and runtime consumers. Regression sources
were inspected alongside those paths. Parser acceptance, an IR variant, a
legacy C-rendering helper or a test name alone does not establish execution
support. Simulator status is not an LSP or synthesis-support guarantee.

Generated sources have experimental compile-time packed-value selection
(legacy ABI 4 by default, compact ABI 5 with portable or optional GMP kernels).
Compact selection remains experimental; representative HDL parity is tested,
while production qualification remains a separate gate. See [the facade contract](../src/sim/rt/value/facade.md).

| Marker | Meaning |
| --- | --- |
| 🟦 Source-implemented | A source implementation exists for the explicitly stated scope; not a claim of full family or IEEE conformance. |
| 🟨 Partial | Some forms execute, but context, lifetime, representation or known behavior gaps are identified. |
| ❌ Not implemented / rejected | No supported built-in execution path for the named scope; language-illegal forms are identified separately. |
| ⬜ Out of scope | Unimplemented advanced forms outside the current scope. |
| ✅ Accepted | Reserved for a specific scope with matching post-change HDL execution evidence, not a whole-family claim. |
| ⚠️ Undefined behavior (Qxx) | Owner-directed provisional disposition for a named interaction whose recorded result lacks a settled conformance oracle under the supplied references. This is not an IEEE designation for the whole feature. |

The current review did **not** rerun the Rust/frontend/public-CLI HDL suites:
a Rust toolchain was unavailable. Coverage-data checks and handwritten native C
component probes were run, including a sanitized value-isolation probe; these do
not qualify generated HDL models. No row is promoted to ✅ by this review.
Historical Linux acceptance is not evidence for subsequent changes or native
Windows/macOS execution. See the [test guide](../tests/readme.md) for test
contracts and the [source/test map](#source-and-regression-evidence) below for
implementation entry points.

The current stackless runtime has no sanitizer-specific stack-switch path; its
configured generated-model lanes use ordinary ASan/UBSan flags. Configuration
alone is not fresh execution evidence or native-platform qualification.

References: **V** = IEEE 1364-2001; **SV** = IEEE 1800-2009. Revision tags retain
**[1995]**, **[2001]**, **[1364-2005]**, **[SV-2005]** and **[SV-2009]**.

## Coverage summary

<!-- Capability counts: one marker + bold feature heading in sections 1-12.
     Do not count legend entries, paragraphs, test cases or Annex productions. -->
| Section | 🟦 Source | 🟨 Partial | ❌ Missing | ⬜ Out of scope | ✅ Accepted | Total |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 1. Lexical/literals | 4 | 0 | 0 | 0 | 0 | 4 |
| 2. Data types | 3 | 10 | 0 | 0 | 0 | 13 |
| 3. Modules/ports/elaboration | 4 | 5 | 0 | 0 | 0 | 9 |
| 4. Scheduling/processes | 4 | 3 | 0 | 0 | 1 | 8 |
| 5. Procedural statements | 5 | 8 | 0 | 0 | 0 | 13 |
| 6. Timing controls | 0 | 7 | 0 | 0 | 0 | 7 |
| 7. Expressions/operators | 4 | 13 | 0 | 0 | 0 | 17 |
| 8. Continuous/structural | 3 | 6 | 3 | 0 | 0 | 12 |
| 9. Functions/tasks | 1 | 5 | 1 | 0 | 0 | 7 |
| 10. System tasks/functions | 9 | 11 | 3 | 0 | 0 | 23 |
| 11. Compiler directives | 5 | 2 | 0 | 0 | 0 | 7 |
| 12. Verification/foreign interfaces | 1 | 12 | 0 | 3 | 0 | 16 |
| **Total** | **43** | **82** | **7** | **3** | **1** | **136** |

**126 rows have some source implementation; 82 of those remain partial and one
is accepted.** The accepted row (`always_ff` and writer rules) carries
post-change HDL execution evidence from RTL-013 and RTL-099; other rows have no
row-level acceptance promotion yet, which does not mean they lack passing tests.

These are **grouped capability rows**, not individual grammar productions or a
language-support percentage. A partial row can contain both substantial working
behavior and unsupported combinations. Macros/includes are counted once in §11;
immediate/deferred assertions, sequential UDPs and major missing verification
families are distinguished rather than hidden in broader rows. Changes in row
count therefore do not imply newly implemented simulator functionality.

The [SYN-038 Annex assignments](../tests/syn038_annex_assignments.json) are a
separate finite-profile inventory: 811 extracted Annex names and 794 reference
names have 665 in common, yielding 940 distinct names. Their dispositions are
228 Core, 61 Extended, 18 implemented simulator forms outside Core, 411 profile
exclusions and 222 aliases/helpers, with zero unassigned names. The
[pairwise source](../tests/syn038_pairwise.json) accounts for 1,849
selected Core cases, 296 impossible cases and 102 cases outside the profile
(2,247 raw cases). A ledger assignment or covered case records its selected
scope and witness; it does not establish all legal contexts or a fresh pass.

<a id="syn-039-selected-profile-completion-boundary"></a>

## Selected-profile acceptance boundary

SYN-038/SYN-039 targets a finite Verilog-2001/SystemVerilog-2009 RTL Core plus
selected Extended forms: fixed values/ports/processes, bounded tagged/pattern
cases, fixed-array methods and memory views, combinational UDPs, library/config
selection and structural bind. The [integrated regression sources](../tests/sim_syn039_acceptance.rs)
compose array-of-record, interface/generate-memory, aliased wired-network and
tagged-pattern/UDP/configuration/bind designs with runtime stimulus in both
optimizer modes. This is not universal IEEE or hardware-synthesis coverage.

The preceding checklist records a historical clean-export acceptance at code
revision `006e874851d05ecda2a0964b6fb097d0933a3fc4`: 3,055 main-suite passes,
five separately run ignored resource tests and 111 targeted generated-model
sanitizer tests on WSL2 Linux x86_64. Those results are retained as historical
provenance, **not re-executed or independently confirmed by this review**. Native
Windows/macOS and other release-matrix hosts remain unqualified here. Current
feature rows must still honor their explicit limits even when a selected-profile
ledger is complete.

The selected packed conditional policy converts Z/Z to X for an ambiguous
condition (§7). The generated fixed-array ceiling is 16,777,216 cells ([resource
limits](#resource-limits)), not unlimited language capacity. N12 admits literal/macro-generated
configuration blocks in library maps under the formal grammar despite conflicting
adjacent prose; this remains the owner's 2026-09-27 policy, not a claimed IEEE
correction. Per-library `-incdir` behavior is described in §3. Q01 mixed-state
structure-pattern constants have a member-specific two-state conversion path.
Q02 memory-file intersections retain unresolved oracle questions. Q03
member-NBA retagging is resolved from SV §§4.9.4, 10.4.2, 7.3.2 and 11.9: the
member write is checked against the tag at commit (§2). Q02 same-slot reader ordering follows the
allowed scheduling interleavings (V §§5.4–5.5; SV §§4.6–4.7). Comparisons with
other simulators are corroboration, not a normative resolution. Their
[characterization fixtures](../tests/fixtures/sim/undefined_behavior/readme.md)
are not independent conformance oracles.

The six SYN-034 optional target uses remain outside this selected profile:
elaboration-only static class methods/constant objects; operator overloading;
elaboration-only fixed-input locator results; legacy procedural assign/deassign
or static-initialization force; module-body specparams; and finite
assertion/monitor synthesis. Profile exclusion does not negate separately
implemented simulator behavior. PLA tasks remain explicitly unsupported (§10).

## Target language editions

`--edition 2001|2009` selects one compilation-wide policy; the default is 2009.
`` `begin_keywords `` / `` `end_keywords `` change lexical keywords in 2009,
not that policy; the 2001 profile rejects them. The pinned Slang API exposes
semantic checks from 2017 onward; the project's 2009 keyword, time and
later-form gates do not establish complete 2009 semantic conformance.

| Boundary | Verilog-2001 | SystemVerilog-2009 |
| --- | --- | --- |
| Legacy modules, ANSI/non-ANSI ports, `assign`, `always @*`, `$display`, `$finish`, `$signed` | Admitted | Admitted |
| Declaration initialization | Active-process race retained | Static initialization precedes ordinary processes, ordered by static reads |
| `logic`, `typedef`, packed structures, type parameters, patterns, `always_comb` | Rejected | Admitted within the feature limits below |
| Whole unpacked-array values, assignments and ports | Rejected; memory declarations, indexed elements and admitted memory-I/O storage arguments remain legal | Admitted fixed forms |
| Unbased-unsized literals and SV-only `for` headers | Rejected | Admitted |
| `$clog2` | Rejected | Admitted |
| Keyword-free later grammar: queue/dynamic/associative and `[size]` dimensions, multiple packed ranges, end and statement labels, `.name` connections, `edge` events, casts, time literals, inline/region-free generate loops, `localparam` ports, default/output function arguments, input-less functions, multi-statement or empty subroutine bodies, unnamed-block declarations, procedural declaration initializers, empty `()` on user subroutines | Rejected with a source-located strict-edition diagnostic | Admitted |
| Later `$countbits`, `assert final`, `$assertcontrol`, covergroup bins `with`/`matches`/set-expression forms | Rejected | Rejected; later internal paths do not override the selected edition |
| `ref` formal of a static subroutine (SV §13.5.2) | Rejected (`ref` is SV-only) | Rejected as an error, not a frontend warning |

Unknown system names require explicit registration. The selected edition's
standard-name allowlist is an **admission policy, not a built-in implementation
list**: unimplemented standard routines can reach the VPI fallback and fail for
lack of a registration (§10). [Edition regression sources](../tests/sim_edition.rs)
include unbased literals, function/multiple-step `for` headers and memory-storage
exceptions; they were not rerun here. The [RTL-019 suite](../tests/sim_feature_completion/rtl_019.rs)
executes every keyword-free later form under 2009 and rejects each one alone
under 2001, and keeps the legal 2001 neighbours (including `$readmemh`/`$fread`
storage arguments) executing in both editions. Complete Annex A coverage is not
claimed: only the forms listed above are gated beyond keywords and system names.

`--compilation-units separate|merged` defaults to `separate`. Separate mode gives
each source its own preprocessor and `$unit` scope; merged mode shares them in
caller order while preserving file identities. Library sources follow the same
mode, merged per library rather than with work sources. Included buffers belong to their
including source. Repeated `--include-dir`/`-I` and `--define`/`-D` options are
supported.

## Resource limits

| Resource | Limit or behavior |
| --- | --- |
| Packed element or value | 1–1,048,575 bits inclusive; `LLG_SUPPORTED_WIDTH_LIMIT = 1 << 20` is exclusive. Each packed cell uses its actual width. |
| Generated fixed unpacked array | At most 16,777,216 cells in the product of all dimensions (`LLG_MAX_FIXED_ARRAY_CELLS`). Extents/products are checked before allocation; an over-limit declaration receives a resource diagnostic. |
| Fixed array used as a value, formal or stream | Integral variable arrays use non-flattened descriptor transport: whole and selected-row copies, equality, conditionals (element-wise merge for an ambiguous selector), default fills, declaration initializers, array-valued pattern items, pattern-lvalue row scatter and multi-segment/unaligned streams, including constant in-bounds `with` ranges. Module, package, function-static and block-static declaration initializers run in the static schedule; automatic block and function arrays initialize per entry. Static, automatic and recursive functions pass such arrays through input, output, inout and ref formals and return them. Arrays of unpacked records whose elements fit the packed limit use the same transport ([RTL-099](../tests/sim_feature_completion/rtl_099.rs) executes 1,048,576 mixed-state records through copies, equality, ambiguous conditionals, function values and an NBA with a bounded model). A single record or tagged union wider than 1,048,575 bits keeps the packed payload limit as a value, and a record member array above the 4,096-cell dense threshold is expanded per cell ([known issue](known_issues.md#remaining-non-flattened-fixed-value-contexts)). Direct reductions read cells individually. |
| Subroutine recursion | At most 256 active calls; a further call emits a recursion-limit diagnostic and returns the result type's default. Recursive calls, including through class virtual and virtual-interface dispatch, use heap frames, so their depth does not consume native stack; recursion re-entering through DPI C code does. |
| Read-only helper inlining | At most 32 nested callback calls; deeper emission receives an explicit diagnostic. |
| Scheduler region passes | Default 10,000,000 per time slot; `LLG_ZERO_LOOP_LIMIT` accepts a positive decimal `uint64`. Exhaustion diagnoses a zero-delay loop. |
| Process back-edges | Default 10,000,000 per process; `LLG_PROCESS_STEP_LIMIT` overrides the `LLG_NONCONVERGENCE_LIMIT` alias. An explicitly set region limit also supplies the process limit when neither process variable is set. Both require positive decimal `uint64` values. |
| Driver, alias, process, final, event-waiter, procedural-driver and force registries | Size to the design or grow with checked allocation; the former 16-driver and 256-alias ceilings do not apply. |

The required capacity lane executes 16,777,216-element storage in both editions
and an SV whole-array copy at that capacity. Large integral variable arrays share
an immutable default and materialize stable cells only for writes or registrations;
source descriptors and whole-copy operations do not grow with the logical extent.
This execution evidence does not qualify every net/native aggregate or value context. Zero/negative size operands are language errors;
negative range labels are valid. Internal zero-width packed descriptors are empty; native
real values use a separate representation.

## 1. Lexical & preprocessing relevant to simulation

Macros, includes and their edition-specific behavior are counted in §11.

- 🟦 **Lexical base and literals** — Comments, whitespace, ordinary/escaped
  identifiers, operators, sized/unsized binary/octal/decimal/hex integers, X/Z
  digits, underscores, real/scientific and string literals. Signed literals and
  signed declarations retain their resolved meaning. V §§2.1–2.7 **[1995/2001]**.
- 🟦 **Attributes** — Parsed without simulator-specific effects, including
  `(* full_case *)`. V §2.8 **[2001]**.
- 🟦 **Fill literals** — `'0`, `'1`, `'x`, `'z` follow context sizing in admitted
  expressions, assignments, calls and case operands. Self-determined
  concatenation/replication operands remain one bit. SV §5.7.1 **[SV-2005]**.
- 🟦 **Time literals** — Unit-suffixed, signed, sub-femtosecond and scientific
  spellings retain owning-scope units and precision. Ordinary 2009 expression
  values round to that precision; complete delay expressions round separately at
  the delay boundary. SV §5.8 **[SV-2005]**; see §6 for scheduling limits.

## 2. Data types

- 🟦 **Basic Verilog storage** — `reg` scalars/vectors, signed net/reg declarations,
  `integer` and unsigned 64-bit `time`; `%d` respects signed two's-complement
  values. V §§3.2–3.3, 3.8–3.9 **[1995/2001]**.
- 🟨 **SystemVerilog integral storage** — Four-state `logic`, two-state `bit`,
  `byte`, `shortint`, `int` and `longint` retain widths/signs; two-state
  assignments and casts convert X/Z to zero, including selected packed-record
  members. Fixed record nets preserve four-state member storage; two-state or
  native net members reject under SV §6.7. Native/resizable aggregate contexts
  retain their separate limits below. SV §§6.9, 6.11 **[SV-2005]**.
- 🟦 **Net declarations** — `wire`/`tri` share ordinary resolved-net behavior;
  declaration assignments are continuous drivers. `uwire` rejects overlapping
  drivers and supports whole/disjoint constant-selected drivers, undriven Z,
  ordinary ports, same-type aliases and uwire actuals on inout ports, whose
  collapsed net keeps one driver per bit. `inout uwire` formals and pass-switch
  terminals reject. Wired/pull/supply nets and strengths are detailed in §8.
  V §§3.6–3.7; SV §6.6 **[1995/SV-2005]**.
- 🟨 **Memories and fixed unpacked arrays** — Multidimensional storage, signed or
  reversed bounds, element selections, rows, slices, whole copies and overlapping
  self-assignment retain logical coordinates. Whole-array values are SV-only;
  admitted fixed integral calls/ports, patterns and operators have the limits in
  §§3, 5, 7 and 9. Native/resizable elements, general real-array expressions,
  and records wider than the packed limit remain restricted. Integral arrays
  through 16,777,216 cells copy, compare, select rows, pass through formals and
  module ports, and stream without packed flattening; arrays of unpacked
  records beyond the packed limit copy, compare, merge, pass through function
  values and publish NBAs the same way (RTL-099). Fixed integral record arrays also
  retain recursive member selections and constant-selected electrical net views.
  V §3.10; SV §§7.4, 7.6 **[1995/SV-2005]**.
- 🟨 **Initialization and lifetimes** — Scalar, fixed integral composite and
  descriptor-backed array initializers, including zero-time calls, run before
  2009 processes in one schedule: each runs after the static declarations it
  reads (directly or through called functions) and otherwise in declaration
  order; 2001 keeps its active-region initialization race. Static
  locals/formals of functions, tasks and blocks initialize once per module,
  interface, generate scope or package, not on first call; automatic storage,
  including descriptor-backed arrays, initializes per activation. SV `const`
  module variables and automatic-function locals retain their initialized
  values; subsequent writes diagnose. Explicit member defaults, recursive array
  defaults and mixed state domains are retained.
  Timing-bearing initializer calls are illegal; unsupported native/resizable
  layouts, oversized records and ambiguous/opposite-lifetime captures remain
  rejected.
  V §6.2.1; SV §§6.8, 6.21, 10.5 **[2001/SV-2005]**.
- 🟨 **Real types** — `real`/`realtime`/`shortreal` support scalar/fixed-array
  storage, parameters, scalar ports, value calls, arithmetic, ordinary `case`,
  waits, any-change events and changed-write notification. Generic containers
  support real/shortreal leaves; automatic real storage survives admitted blocking
  mailbox delivery. Queued automatic-real writes, general real-reference calls
  and real sampled/method callbacks remain rejected. V §3.9 **[1995]**.
- 🟦 **Typedefs and enums** — Simple/packed aliases resolve through the frontend.
  Enums retain nominal identity, base width and sparse signed declaration-order
  values. `first/last/next/prev/num/name` support wrapping counts, owned names and
  two-/four-state invalid-value defaults. SV §§6.18–6.19 **[SV-2005]**.
- 🟨 **Structures and untagged unions** — Packed members preserve state/sign and
  equal-width union views overlap. Recursive fixed unpacked records/arrays
  support initialization, deep copy and represented packed/real/string/chandle
  leaves. Unequal-width unpacked untagged unions use the maximum packed-member
  extent; unequal-width packed unions reject. Fixed integral record copies,
  array slices and packed member selects retain owned layouts and declared
  bounds. Four-state packed records and recursive fixed unpacked records/arrays
  admit nets, selected continuous contributions and inout connections. Unpacked
  union net members reject under SV §6.7. General native/resizable subroutine
  layouts and native aggregate slices remain restricted.
  SV §§6.7, 7.2–7.4 **[SV-2005]**.
- 🟨 **Tagged unions** — Packed and unpacked tagged unions with fixed payloads
  use one finite storage owner: the tag in the most significant bits and each
  member right-justified below it. Construction and checked member access
  support void, two-/four-state and signed integral, packed and unpacked
  record, fixed unpacked array and nested-tag payloads in module storage,
  static/automatic locals, records, unpacked arrays, value, constructor, port,
  input/output/inout, const-ref/ref and NBA contexts, including bit, part,
  indexed-part and element selects of a member with runtime selectors.
  Selected active members are actuals for formals of the member type. Valid
  reads restore the selected member's state/sign; inactive-member reads,
  writes and selects produce source-addressed runtime errors, read X and
  store nothing. Real/string/chandle and dynamic payloads (SIM-007) and
  payloads beyond packed capacity reject with explicit diagnostics.
  **Q03 (resolved):** SV §§4.9.4 and 10.4.2 fix an NBA's target and RHS at
  issue and perform the member assignment at commit; SV §11.9 requires that
  assignment to be consistent with the tag current then, and SV §7.3.2 never
  stores one member's value under another member's tag. A wrong tag at issue
  reports at issue. A valid issue whose member is inactive at commit (another
  member, void, a narrower member, another nested tag, an earlier-committed
  whole-variable NBA or another process's retag) reports `nonblocking write to
  tagged-union member M ... found an inactive tag at commit` and leaves the
  retagged value unchanged (owner policy for the post-error state); a
  same-member or away-and-back retag publishes the issue-time RHS.
  Interprocess races are tested as allowed result sets (SV §§4.6–4.7).
  Pattern matching is covered in §5.
  SV §§7.3, 11.9, 12.6 **[SV-2005]**.
- 🟨 **Strings** — Module/static/automatic byte strings support copies, casts,
  core methods, `atoreal/realtoa`, formatting, value/reference formals, copy-out,
  returns and collected input/output links. Contents changes feed sensitivity;
  inputs/returns have independent ownership. Blocking delayed assignments and
  delayed NBAs to persistent strings have an owned-value path, covered by the
  [delayed-string regression](../tests/fixtures/sim/data_types_next/string_delayed_nba.sv).
  **Ordinary untimed native-string NBAs still reject**, including persistent
  targets: [object assignment lowering](../src/sim/codegen/lowering/objects/assignments.rs)
  rejects nonblocking writes, whereas the
  [delayed-assignment path](../src/sim/codegen/lowering/statements/assignments.rs)
  handles `s <= #delay value` separately. Automatic delayed-NBA targets,
  unsupported captures, automatic monitors and broader aggregate/continuous
  combinations remain restricted. SV §6.16 **[SV-2005]**.
- 🟨 **Events** — Scalar/fixed-array declarations, indexed/hierarchical access,
  null/default handles, reassignment and task aliases retain event identity.
  Dynamic/associative/queue event storage is unsupported. SV §6.17 **[SV-2005]**.
- 🟨 **Dynamic arrays, associative arrays and queues** — Allocation, resize,
  delete, copy, bounded patterns, generic/nested leaves, associative defaults and
  traversal, queue slices/overflow, collected value-port copies and bit/part
  selects of packed elements (written as one element read/modify/write) are
  present.
  Contents/shape changes notify readers. General subroutine storage, non-packed
  endpoint/pop expressions, string-key index-result queues, nested scalar queries
  and broader recursive/object forms remain restricted. Methods are in §7.
  SV §§7.5, 7.8, 7.10, 7.12 **[SV-2005]**.
- 🟨 **Chandle** — Typed native-pointer null/copy/identity/Boolean operations,
  locals, admitted aggregate/class fields, mixed signatures, returns and
  input/output/inout/ref/const-ref calls are present. Nonblocking object
  assignment remains unsupported. Chandle ports (including ref ports), packed
  containment, arithmetic, continuous assignments and sensitivity/event
  expressions are language-illegal under SV §6.14, rather than implementation
  gaps. Other object sensitivity contexts remain partial.
  SV §6.14 **[SV-2005]**.

## 3. Modules, ports, parameters, hierarchy

- 🟦 **Modules and hierarchy** — ANSI/non-ANSI headers, positional/named ports,
  instance arrays, `generate for/if/case`, `genvar`, hierarchical/upward reads and
  admitted selected blocking/NBA writes. Constant functions in a generate-loop
  bound and step elaborate three lanes whose values follow a runtime input in
  both selected editions and optimizer modes. V §§12.1–12.4 **[1995/2001]**.
- 🟦 **Parameters and constant elaboration** — Parameters, `localparam`, named/
  positional overrides and `defparam` re-elaboration propagate values and widths;
  overriding `localparam` is illegal. Finite constant functions, type-dependent
  defaults, nominal enums, `type()`, constant queries and folded real/string
  values support finite extents and names. Fixed struct/union parameters are
  readable whole or by member; override expressions retain their parent scope.
  [Elaboration regression sources](../tests/sim_syn016_elaboration.rs) exercise
  bounded function/type/namespace cases; source presence is not fresh acceptance.
  Constant functions reading `defparam`-affected parameters have
  undefined results under the supplied references. V §§3.11, 10.3.5, 12.2;
  SV §§6.20, 6.23, 13.4.3 **[1995/2001/SV-2005]**.
- 🟨 **Packages and `$unit`** — Qualified/imported/re-exported names, wildcard
  exports, shared variables, dependent initialization, static subprogram state,
  types and typed (including aggregate) constants are represented, and module
  initializers read them regardless of collection order. Separate/merged unit
  scope is retained; a name redeclared in a package or unit scope rejects.
  Native element layouts (for example string arrays), unsupported callable
  environments and post-2009 forms remain restricted. Instance-hierarchical names are not constant-expression operands;
  lexical package references remain distinct. SV §26 **[SV-2005]**.
- 🟦 **Interfaces and modports** — Concrete storage, instance-local processes,
  parameterized interfaces, member references and modport views are represented.
  `.name` and `.*` connection shorthands retain their resolved links. Runtime
  virtual handles are covered in §12. SV §§23.3.2, 25.3, 25.5 **[SV-2005]**.
- 🟦 **Extern and nested modules** — Parameterized declarations/matching bodies,
  same-scope enclosing parameter references, distinct enclosing-instance
  specializations and independently scoped same-named definitions work in both
  compilation-unit modes. Nested instances in selected generate branches use
  explicit ports/parameters; declarations directly inside generate blocks are
  illegal under 2009 Annex A.1.4/A.4.2. Missing bodies, signature mismatches and
  out-of-scope references are diagnosed; 2001 rejects both declaration forms.
  SV §§23.4–23.5 **[SV-2005]**.
- 🟨 **Value ports** — Packed expressions/defaults, scalar-real links and
  collected string/aggregate/container copies have typed value behavior. Fixed
  integral input arrays accept call/conditional/pattern/cast values, slices and
  runtime-selected rows, captured once and scattered in declaration order.
  Explicit/implicit intermediate casts remain effective, including two-state
  conversion followed by a four-state formal; contents and selectors establish
  dependencies. Fixed outputs admit whole arrays, constant rows/slices, aggregate
  values, nested member and member-array targets and instance-array
  distribution; interface modport arrays link through generate and forwarding.
  A variable output target with runtime selects is an implied continuous
  assignment: selector changes retarget it and unknown selectors write nothing.
  Descriptor-backed arrays (to 16M cells) cross ports as descriptor copies.
  Output targets also written procedurally or by another port, invalid output
  expressions, language-illegal chandle ports (SV §6.14), uncollected layouts
  and runtime-selected net or inout connections reject. V §12.3;
  SV §§6.5, 7.4.6, 23.2–23.3, 25.5 **[1995/SV-2005]**.
- 🟨 **Reference and inout ports** — Matching packed/fixed-array references and
  collected recursive aggregate/string leaves share storage. Chandle ports are
  prohibited by SV §6.14. Admitted module ref-port packed projections are
  distinct from subroutine ref-actual legality (§9). Inequivalent shapes and
  runtime reference rewiring reject. Fixed bit/part/indexed selections through
  selected module refs retain each intermediate boundary, including packed
  members below fixed-array rows. Blocking writes, mutations, NBA issue-time
  capture and call copy-out use the checked projection. Runtime-selected ref
  connections remain an unsupported binding boundary with unresolved rebinding
  semantics. Whole net-array inout ports, rows and slices join each formal cell
  to its left-to-left actual cell. Dissimilar resolved-net inouts follow §8.
  SV §§23.2.2, 23.3.3.5 **[SV-2005]**.
- 🟨 **Libraries and configurations** — Explicit `--libmap`, repeatable
  `--libfile [library=]path`, `--library-order`, `--default-library`, top
  `module:config`, default `liblist`, cell/instance `use`, nested configurations
  and parameter propagation use ordinary elaboration, including configuration
  `use #(...)` overrides and resets (SV §33.4.3) of cells selected inside
  generate scopes, composed with binds, interface memories and descriptor-sized
  fixed memories crossing configured and bound ports. `%l` prints the bound
  `library.cell` (or `library.$unit` outside a design element) for literal
  formats (V §13.6; SV §33.7); a runtime-built `$sformatf` format still
  prints `work.<scope>`. Without a configuration, the declared library order
  resolves same-named cells, an implementation-defined tie. Library sources follow
  `--compilation-units`: separate mode preprocesses each file alone, merged mode
  shares one preprocessor and `$unit` per library in admission order. Included maps and
  `*`/`?`/`**` patterns expand in sorted order. Matching precedence is explicit
  filename > wildcard filename > trailing-separator directory; explicit library
  assignments override maps. Relative parent paths resolve from the map directory.
  Missing/ambiguous bindings, encrypted IP, foreign co-simulation and unrestricted
  filesystem discovery reject. Per-library `-incdir` paths resolve from the
  containing map, expand wildcard directory matches in sorted order, keep
  declaration order, and search only for that library's sources. The including
  file's directory wins first, followed by global command-line include
  directories and then library directories. This precedence is the pinned Slang
  default where V §13.2 and SV §33.3 do not prescribe a tie rule.
  All include content is admitted before cache-only native compilation;
  invalid or missing directories reject at map admission. In-memory maps select
  logical include directories that hold admitted buffers or are listed in
  `CompileOpts::logical_directories`, which represents an empty directory.
  V §13.2; SV §33.3.
  V ch.13; SV ch.33 **[2001]**.

  Literal and macro-generated `library`, `include`, and `config ... endconfig`
  declarations are admitted in root/included library maps. Conditional compilation,
  macro path lists and macro-generated config delimiters use the same selected
  edition as design sources. Command-line defines seed each map; definitions
  persist in source order within that map, but maps and design source units do
  not inherit each other's local macros. Map `include` opens another map with
  its own macro environment. Compiler `` `include`` inside a map is diagnosed;
  use a map `include` declaration. V §13.2 / §19.3-§19.5 and SV §33.3 /
  §22.4-§22.6 supply the grammar and preprocessing basis. The narrower nearby
  V §13.2.2 / SV §33.3.2 prose is an acknowledged source inconsistency, not a
  claimed IEEE correction.
- 🟨 **Structural bind** — Finite module-type/selected-instance and
  interface-to-interface binds preserve parameters, target-local ports and
  instance identity after frontend snapshot destruction. Selected generate-for/if
  instance paths execute in both optimizer modes, as do interfaces bound into
  module targets and binds into configuration-selected library cells.
  Unknown/primitive/package/class/generate-block targets, port actuals outside the
  target scope and duplicate bound instance names reject; a name repeated across
  module-type and instance binds (which the frontend only warns about) is a
  lowering error. Module injection into an interface or program is illegal. A
  module-type bind target names the definition its own library search resolves,
  an implementation-defined choice when several libraries define that name.
  Checker/program/coverage injection is outside the selected scope.
  SV §23.11 **[SV-2005]**.

## 4. Scheduling & processes

- 🟦 **Basic scheduling** — `initial` runs once; ordinary `always` repeats,
  including wait-free bodies. Active/Inactive (`#0`)/NBA work iterates to
  quiescence. Wait-free nonconvergence reports the process location and a nonzero
  status when its work budget expires. V §§5.3–5.4, 9.9 **[1995]**.
- 🟦 **Parallel blocks** — Named blocks/forks, `fork/join`, `join_any`,
  `join_none`, `wait fork` and `disable fork` have paths. Subroutine and capture
  limits are in §9. V §9.8; SV §§9.3.2, 9.6 **[1995/SV-2005]**.
- 🟦 **Level-sensitive wait** — Packed/real conditions re-evaluate on dependencies;
  constant false/unknown waits stay suspended without blocking other processes
  or time advancement. V §9.7.6 **[1995]**.
- 🟨 **Named disable** — Resolved declaration/instance targets terminate their
  active invocations and clean up lexical activations before copy-out.
  Self-disable is not an early return. Finite local named-block exits are
  represented; broader cross-process, recursive, native and shared-capture
  cancellation remains partial. V ch.11 **[1995]**.
- 🟨 **`always_comb` / `always_latch`** — Execute once at time zero after every
  initial and always procedure has started; include transitive function reads
  and exclude written expressions. Fixed arrays (including 65,537-cell
  descriptor arrays, by contents marker rather than per cell), nested record and
  packed members, constant/runtime selectors, conditional arms, sequential
  predicates, ref ports (nested, to members and cells), string variables and
  string record members, and aggregate input links contribute dependencies.
  Unchanged results do not notify downstream readers; a closed latch retains
  its value. Branches pruned by the optimizer keep their wake sources. Blocking
  timing and forks reject; delayed NBAs are not rejected merely for their delay.
  Non-string object and dynamic/native aggregate contexts remain partial
  (SIM-013). SV §§9.2.2.2–9.2.2.3 **[SV-2005]**.
- ✅ **`always_ff` and writer rules** — Requires one event control and rejects
  blocking timing (also in called tasks), forks and extra overlapping writers.
  Blocking data assignments, timing-free calls, delayed NBAs, event triggers and
  force/release are legal. Data changes alone do not wake it, including `iff`
  qualifiers and asynchronous-reset or-lists. Disjoint array cells, rows of any
  width (one interval per row), record or packed members and ref-port targets
  remain separate writers; overlapping procedural, called-function,
  hierarchical, ref-port, positional-pattern, output-port and continuous writes
  are diagnosed within admitted storage. Force/release and procedural
  `assign`/`deassign` are overrides, not competing writers. Constant rows and
  slices of descriptor arrays bound to output ports are one cell-interval
  writer each ([RTL-099](../tests/sim_feature_completion/rtl_099.rs)).
  SV §9.2.2.4 **[SV-2005]**.
- 🟦 **Final blocks** — Run once after scheduler exit (`$finish`, deadlock or no
  future events), observing committed values and end time. Timing controls,
  forks, task calls, NBAs and deferred strobe/monitor calls reject. `$finish`
  inside a final stops it and skips remaining finals. SV §9.2.3 **[SV-2005]**.
- 🟨 **SystemVerilog regions** — Preponed, Active/Inactive, Pre-NBA/NBA/Post-NBA,
  Pre-Observed/Observed/Post-Observed, Reactive/Re-Inactive/Re-NBA and
  Pre-Postponed/Postponed queues support fixed-point re-entry and immutable
  observation. Clocking, program and assertion consumers are bounded by §12;
  the full PLI/VPI region API is not implemented. SV §4.4 **[SV-2005]**.

## 5. Procedural statements

- 🟦 **Basic statements** — Sequential/named blocks, `if`/`else`, ordinary
  `case`, `case(1)`, `casez` and `casex`. `casez` treats Z in either operand as a
  wildcard; `casex` additionally treats X as a wildcard. V §§9.4–9.5, 9.8
  **[1995]**.
- 🟦 **Packed assignments** — Whole/bit/part/indexed-part/array-element blocking
  stores and NBA updates. NBAs capture RHS and coordinates at issue time, commit
  in source order and merge only selected bits into current storage.
  V §§9.2.1–9.2.2 **[1995]**.
- 🟨 **Fixed-array assignments** — Whole/selected fixed integral values, slices,
  concatenations, patterns, casts, calls and conditionals capture the complete
  RHS before any store or NBA issue. Overlap, declared element order and per-cell
  conversions are retained. Static task-local arrays and explicitly static arrays
  in automatic tasks retain per-declaration, per-instance storage through NBA
  publication; changed elements notify sensitive readers. Native/resizable and
  other oversized value contexts remain restricted. Large whole-variable copies
  and descriptor-backed scalar patterns use issue-time snapshots for blocking/NBA
  publication, including sparse index keys, type keys and nonuniform repeats;
  automatic-variable and subroutine-reference-formal NBAs reject. SV §§7.6, 10.4.2, 13.3.2
  **[SV-2005]**.
- 🟨 **Assignment-pattern values** — Positional, member/integer-index, type and
  default keys support admitted fixed arrays/records. Explicit-index > type-key >
  default precedence, last matching type key, immediate row types and recursive
  defaults are retained; semantic duplicate indices and uncovered elements reject.
  Array-valued items supply subarray cells in declaration order and evaluate once;
  only scalar fill values broadcast. Repeated operand positions remain distinct.
  Packed vectors/arrays and structures match their immediate declared element/member
  types, including equivalent non-nominal integral types. Descriptor-backed patterns
  retain sparse defaults and snapshot exceptions without a packed payload; their
  oversized array-valued items are captured once into descriptor snapshots and
  copied as rows, and oversized declaration initializers use the same transport. Type/default evaluation
  multiplicity is undefined; value tests do not prescribe invocation counts.
  SV §§10.9.1–10.9.2 **[SV-2005]**.
- 🟨 **Replicated patterns** — Constant counts expand fixed integral arrays,
  nested rows and aggregates in syntactic order. Zero/negative counts, incompatible
  shapes and unsupported native/resizable values reject in the admitted nonempty
  fixed slice. Descriptor arrays use sparse fills or loops, with bounded generated
  source for repeated syntax. Replication side-effect multiplicity is undefined. SV §10.9.1 **[SV-2005]**.
- 🟨 **Positional pattern lvalues** — Plain/typed fixed arrays, packed arrays and
  packed/unpacked records deconstruct into nested/selected destinations. Capture
  the RHS and every destination coordinate before scatter writes; earlier stores
  cannot redirect later targets. Blocking and legal persistent NBA targets are
  represented; continuous net, net-array and true-alias leaves retain per-leaf
  resolution. Keyed/default/replicated lvalues, constants, mismatches and
  illegal NBA targets reject. Oversized array and typed-pattern sources scatter
  whole descriptor rows into array targets, including continuous targets.
  SV §10.9 **[SV-2005]**.
- 🟦 **Sequential Boolean predicates** — `&&&` in `if` and `?:` evaluates reached
  clauses once, left to right. Only definite true advances; false or X/Z stops.
  A later false cannot resolve an earlier unknown. Ambiguous `if` takes else;
  ambiguous values use the conditional rules in §7. This is distinct from `&&`
  and timing-check conditions. Dependencies include every clause despite runtime
  short-circuiting. SV §§12.6.2–12.6.3 **[SV-2005]**.
- 🟨 **Pattern matching** — Integral constants, `.*`, `.name`, recursive fixed
  packed/unpacked structures and finite tagged payloads work in `if`/`?:` and
  `case`/`casez`/`casex ... matches`. Whole fixed values admit wildcard/binding
  patterns. One selector capture feeds ordered member checks/filters; successful
  automatic bindings extend to later clauses and the true arm. Cases keep
  first-match/default behavior; `unique` reports zero or multiple matches,
  `unique0` reports multiple matches, and `priority` reports zero matches
  after item filters. A default suppresses zero-match reports. Tag/payload
  comparisons inherit the enclosing case mode; ordinary member access and
  `if ... matches` remain exact. Integral constant patterns also accept whole
  packed structs and untagged packed unions, including nested packed members
  and tagged packed payloads. A packed struct in a matching case retains the
  enclosing case mode; outside `casez`/`casex` it compares X/Z exactly.
  Unpacked aggregates and real values are not integral constant patterns.
  Bindings end at the true arm or case item.
  Fixed structure pattern checks read each member in its own state domain: a
  two-state member in a four-state packed record converts X/Z to zero before
  exact constant comparison (SV §§7.2.1, 12.6). Fixed arrays of structs can
  be bound as complete members; §12.6 defines no recursive array pattern form.
  Finite tagged payload patterns, including unpacked record and array
  payloads, check their tag before payload checks; `casez` wildcards Z tag
  bits and `casex` wildcards X/Z tag bits, so an undefined tag matches only in
  `casex`. Whole tagged bindings retain their type through later `&&&`
  clauses. Wrong tag names and non-tagged sources reject. Whole dynamic/native
  wildcard and binding patterns, dynamic/native tagged payloads and whole-value
  sources beyond packed capacity remain restricted. SV §§7.3.2, 12.6
  **[SV-2005]**.
- 🟦 **Qualified selection** — `unique`, `unique0`, `priority` diagnose no-match/
  multiple-match with source locations and default/else suppression. `case inside`
  supports wildcard items, ranges, qualified string cases and one selector
  evaluation. SV §§12.4.2, 12.5.3–12.5.4 **[SV-2005]**.
- 🟦 **Loops and jumps** — `for`, `while`, runtime-count `repeat`, `forever`,
  `do-while`, nested `break`/`continue` and subroutine `return`. Continue executes
  the for-step list or condition back edge; break exits the innermost loop.
  For headers support multiple initializers/steps and ordinary HDL function
  steps, including copy-out/ref effects and discarded values. Tasks as steps
  reject; break/return bypass the steps. Function-step and multiple-step cases
  have [edition regression sources](../tests/sim_edition.rs); generated-model
  execution/ownership was not requalified here. V §9.6; SV §§12.7–12.8
  **[1995/SV-2005]**.
- 🟨 **Loop declarations and `foreach`** — Packed/real loop locals preserve
  shadowing and admitted fork captures. Fixed foreach retains mixed packed/
  unpacked dimensions, omitted slots, singleton/implicit vectors, declared/formal
  bounds and signed endpoint guards, including descriptor-backed arrays (to 16M
  cells) traversed in place. Iterators are automatic, lexical and
  read-only. Dynamic/queue and integral/string-keyed associative traversal have
  separate paths. Nested resizable traversal, string/shared/native captures and
  illegal iterator writes remain rejected. SV §§12.7.1, 12.7.3 **[SV-2005]**.
- 🟨 **Procedural assign/deassign** — Whole packed/real/shortreal variables and
  admitted packed concatenations use replaceable live RHS bindings. `deassign`
  retains the last value; ordinary writes cannot override an active binding.
  `force` wins, and release resumes the latest live RHS. Net, selected, array,
  hierarchical, streaming and recursive aggregate targets reject.
  V §9.3.1 **[1995]**.
- 🟨 **Force/release** — Persistent packed/real variables and admitted constant-selected, concatenated, resolved-net and canonical alias targets support live
  RHS dependencies and wakeups. Release restores net resolution; variables retain
  the forced value unless a procedural continuous assignment resumes. Dynamic
  selects, automatic/array targets and unsupported indirect/hierarchical/net
  forms reject. Live RHS sources include fixed-array elements and whole arrays
  (through their change markers). A helper with visible writes, persistent
  state or descriptor formals is evaluated by the force statement and then, while
  the force is in effect, by a per-site guard process into a hidden source the
  read-only evaluator reads; release or replacement stops it (§9).
  V §9.3.2 **[1995]**.

## 6. Timing controls

- 🟨 **Delays and precision** — Integer, fractional, scientific, unit-suffixed,
  parameter and runtime packed/real expressions, function results and task inputs
  are evaluated once for statement/intra-assignment delays. Local precision
  rounding precedes checked 64-bit femtosecond scheduling (1fs–100s units).
  Sub-precision delays round to zero; X/Z means zero. Negative packed values
  convert to unsigned 64-bit time before scaling; negative/nonfinite reals and
  tick overflow reject. Continuous/gate delays are separately bounded in §8.
  V §9.7.1; SV §§3.14, 5.8 **[1995/SV-2005]**.
- 🟨 **Implicit sensitivity** — `@*`/`@(*)` use call-site body reads, unlike
  `always_comb`'s transitive function reads. Fixed-array elements/contents
  (including descriptor arrays), nested record members, ref-port targets,
  container contents/shape, and string variables and string record members wake
  admitted readers after copies, mutations or resizing. Non-string object
  (chandle/class handle) dependencies remain restricted (SIM-013); these are
  dependency-collection limits, not full implicit-sensitivity support.
  V §9.7.5 **[2001]**.
- 🟨 **Evaluated events** — Packed/scalar-real any-change expressions, packed LSB
  edges, trigger-time `iff`, numeric activation captures and atomic mixed named-event lists are represented. Sensitivity follows operands and eligible helpers,
  not unrelated body reads. Helpers with visible writes, persistent static state
  or descriptor-array formals are evaluated by the waiting process when the
  control is reached and after each dependency change (§9), including lists
  with declared named events (trigger counts) and real values (IEEE bit
  patterns). Real edge descriptors, unsupported qualifiers/captures, event
  handles or array-reading helpers in such named-event lists and helper forms
  outside §9 reject.
  V §§9.7.2–9.7.4 **[1995]**.
- 🟨 **Intra-assignment controls** — Packed/real/shortreal RHS values are captured
  immediately. Blocking assignments suspend and use update-time selectors; NBAs
  continue with issue-time destinations. Explicit event/repeated-event controls
  preserve order and normalize zero/X/Z/negative repeat counts. Controls with
  effectful helpers wait in the blocking process; NBA and `->>` forms arm at
  issue and wait in a detached process outside `wait fork`/`disable fork`
  (§9). Nested repeat
  timing, queued automatic real/string writes, reference-formal NBAs and
  unsupported storage forms remain restricted. V §9.7.7 **[1995]**.
- 🟨 **Standalone repeated event waits** — `repeat(n) @ev;` is represented as a
  repeat loop around an event wait, retaining one packed count across waits.
  This does not admit every standalone repeated-timing-node form; real counts
  and unsupported event captures reject. Dedicated post-change HDL acceptance
  remains unverified. V §9.7.7 **[1995]**.
- 🟨 **Named-event operations** — `->`, untimed/delayed `->>`, event-controlled/
  repeated-event `->>`, `.triggered` and `wait_order` are represented. Nonblocking
  triggers capture source/target identity and register at issue time without
  suspending the issuer. Unsupported repeat forms, dynamic event storage and
  event-formal numeric/virtual-interface dispatch remain restricted.
  V §9.7.3; SV §15.5 **[1995/SV-2005]**.
- 🟨 **Cycle delays** — Procedural `##N` repeats the resolved default clocking
  event, including irregular periods. Missing default events and unsupported
  cross-clock forms reject. Clocking drives and assertion sequence delays are
  bounded separately in §12. SV §14.11 **[SV-2005]**.

## 7. Expressions & operators

- 🟨 **Arithmetic** — `+ - * / % **` operate at actual packed widths below the
  resource limit on the legacy and compact value backends (portable and GMP
  kernels). Public 2001/2009 witnesses compare every pair of 13 boundary
  operands (signed minima/maxima, -1, -3, patterns, X/Z) at widths 1, 31, 32,
  63, 64, 65, 127, 128, 129, 8,128 and 8,129 with an independent limb oracle.
  They cover zero divisors, both exponent signs (Table 11-4), oversized and
  X/Z shift counts, mixed-width/mixed-sign assignment contexts and literal
  operands folded by the optimizer, in both optimizer modes. Powers of 0, 1,
  -1 and even bases finish early; other odd bases take one product per
  exponent bit. V §§4.1.5–4.1.6, 4.4–4.5; SV §§11.4, 11.6–11.8
  **[1995/2001/SV-2009]**.
- 🟦 **Bitwise, logical, reduction, shift and relational operators** — `& | ^ ~`,
  XNOR forms, `&& || !`, reduction AND/NAND/OR/NOR/XOR/XNOR, `<< >> <<< >>>`
  and `< <= > >=` retain resolved widths, signs and four-state behavior.
  Ordinary SV `->` short-circuits a known-false antecedent; `<->` evaluates both
  sides. SVA `|->`/`|=>` belongs to the assertion boundary (§12), not these operators.
  V §§4.1.7, 4.1.9–4.1.12; SV §11.4.7 **[1995/2001/SV-2009]**.
- 🟨 **Equality** — `===`/`!==` compare X/Z literally; `==`/`!=` preserve known-mismatch dominance. Fixed integral records and arrays compare immediate
  members from storage, input/const-ref/inout/output/task-ref formals, returns,
  ports, structure nets, casts and conditionals. An invalid index reads the
  element type's uninitialized default, so a two-state member still decides a
  known mismatch. Descriptor-backed arrays compare cell-wise after reshaping or
  two-state casts. Native/dynamic aggregates and real-member formal/return
  operands remain partial. V §4.1.8; SV §§7.4.6, 11.4.5 **[1995]**.
- 🟨 **Conditional values** — Known truth selects one arm, including a vector
  predicate with a dominant known 1. Ambiguous truth evaluates both reached arms
  once. Packed values follow the selected published-table policy: equal 0, 1 or X
  survives; Z/Z and differing pairs become X. Real alternatives return zero.
  Fixed arrays compare immediate elements and unpacked structures compare
  immediate members: known-equal boundaries survive; differing boundaries take
  their type's uninitialized default. Nested rows/records default as whole
  members, not recursively merged leaves; explicit member initializers are not
  fallbacks. A runtime-selected unpacked record with an eight-bit `logic` member
  and a one-bit `bit` member retains these defaults through a persistent NBA:
  unequal data becomes X and the two-state member becomes zero at commit.
  Constant folding and identical-arm simplification keep every reached arm's
  side effects. Descriptor-backed arrays merge cells without packed flattening.
  Dynamic/native aggregate merges remain restricted.
  V Table 28; SV Table 11-20, §11.4.11 **[2001/SV-2009]**.
- 🟦 **Concatenation, replication and selection** — Preserve order and
  self-determined widths, including singleton-concatenation unsigned/fill
  boundaries. Packed multidimensional, bit/part/indexed-part chains beneath
  fixed-array elements support constant/runtime indices. Out-of-range/X/Z reads
  produce X positions; writes affect only in-range positions. Ordinary part
  selects remain unsigned. V §§4.1.14, 4.2.1–4.2.2 **[1995/2001]**.
- 🟦 **Packed strings and sign conversion** — Eight-bit ASCII vectors support
  literals/escapes, assignment, comparison, concatenation and padding/truncation.
  `$signed`/`$unsigned`, resolved sign/self-determined width rules and X/Z
  semantics are retained; identity/copy preserves Z. V §§3.1, 4.2.3, 4.4–4.5
  **[1995/2001]**.
- 🟨 **Real operations** — Admitted arithmetic, relational/logical/conditional,
  numeric casts and ordinary `case` comparisons have paths. Bitwise, reduction,
  shifts, concatenation, case equality and real selects reject. V §4.1.1 **[1995]**.
- 🟨 **Static and dynamic casts** — Typed, size and sign casts preserve width,
  state and conversion boundaries in admitted scalar/vector and fixed contexts.
  `$cast` supports packed/real values, enum membership against the complete
  source value and nominal class checks. Selectors and the source are evaluated
  once and only a successful cast writes; a failed task-form `$cast` reports a
  run-time error. Non-singular destinations reject. Fixed bit-stream casts are
  described below; descriptor-backed arrays reshape and clear X/Z for two-state
  targets before comparison or storage. Real-source enum `$cast` and unsupported
  aggregate/net/reference targets remain restricted.
  SV §6.24 **[SV-2005]**.
- 🟨 **Mutating operators** — Prefix/postfix `++/--` and `+= -= *= /= %= &= |= ^=`
  plus shift assignments capture an admitted whole/member/selected/fixed-array
  target once, perform one store and return the appropriate expression value,
  including nested fixed module-ref projections. Call-valued indices,
  record/interface/hierarchical receivers and right-hand sides run once in
  procedural code, automatic and static functions, and functions called from
  continuous assignments and port expressions; widths, signs, X and overflow
  follow the operator at the target's width. Packed state conversion and real
  targets are represented; non-lvalues, continuous/port mutation expressions
  and unsupported aggregates/objects (including call-valued class receivers)
  reject. SV §§11.4.1–11.4.2 **[SV-2005]**.
- 🟦 **Wildcard equality** — `==?`/`!=?` treat RHS X/Z as wildcards. Remaining
  LHS unknowns yield X unless a known mismatch decides the result; common-width
  and signed extension apply. SV §11.4.6 **[SV-2005]**.
- 🟨 **Set membership** — `inside` supports scalar/range/wildcard and admitted
  real/string contexts. Fixed-array values, including calls, conditionals, casts,
  rows and members, are captured once and traversed with leaf signedness intact.
  Stored arrays and selected rows, including descriptor-backed storage, are
  traversed cell by cell in one loop (small dense whole arrays unroll); an
  unknown row selector reads default elements. Packed dynamic/queue/associative
  storage has separate traversal. Packed values
  are single set items; casts are not bypassed; unpacked structures are illegal
  member bags. Definite matches dominate an X fallback. Recursive/native objects,
  unsupported resizable value expressions and over-limit payloads reject.
  SV §11.4.13 **[SV-2005]**.
- 🟨 **Fixed reductions** — `sum/product/and/or/xor`, optional parentheses and
  lexical `with` maps support immediate integral elements and admitted fixed-row/
  record maps. Result width/sign comes from the element/map, not the destination;
  first-element seeding preserves singleton X/Z. Maps may capture automatic
  locals/formals and distinct nested iterators. `index`, `index()`, `index(1)`
  and checked dynamic dimensions address the live fixed unpacked iterator
  with declared bounds. Selected last-dimension rows, including descriptor-backed
  rows, reduce cell by cell in place. Packed, higher and unvisited dimensions reject;
  native/string/real/resizable maps also reject. Receiver capacity is defined above;
  a map whose item is a row wider than the packed limit rejects.
  SV §§7.12.3–7.12.4 **[SV-2005]**.
- 🟨 **Fixed ordering** — `reverse()` moves immediate integral/fixed-record
  elements in declaration order; rows move whole. `sort()`/`rsort()` accept
  integral elements or fixed row/record `with` integral keys, preserving
  signed/enum keys and non-key fields. Selected receiver coordinates are
  captured once. Each element's key is evaluated once from its original
  position, so `item.index` names the element's own index, before any element
  moves. Stored receivers of more than 16 elements, descriptor-backed arrays (to
  16M cells), selected rows and rows of oversized arrays reorder in place with a
  stable O(n log n) cell-wise loop and per-cell publication; smaller receivers and
  packed-value locals/formals use a straight-line schedule. A `with` key over a row
  wider than the packed limit rejects. Automatic locals/formals are represented. Reverse qualification covers
  1/2/3/17 elements, negative/reversed bounds, selected 2-D/3-D rows, unpacked
  records, input/inout/ref formals, and settled reader notifications in both
  optimizer modes; SV2001 rejects the method. Integral `with` keys may query the
  live iterator's declared index on ascending, descending and row arrays.
  Sort/rsort qualification covers signed and unsigned integral and enum elements,
  packed and unpacked record maps, repeated keys, ascending/descending bounds,
  singleton arrays, zero-time function calls and selected automatic/formal rows
  in both optimizer modes; SV2001 rejects fixed ordering. Repeated-key stability
  and X/Z key order are unspecified. Reverse `with`,
  const-ref receivers, record sorting without a key, fixed shuffle/locators,
  native/real/string elements and incompatible maps are outside this fixed
  integral profile and reject.
  SV §7.12.2 **[SV-2009]**.
- 🟨 **Resizable-container methods** — Packed reductions/`with` callbacks,
  locators, min/max/unique result queues, sort/rsort/reverse/shuffle are present.
  Queue/dynamic `sort`/`rsort` evaluate each `with` key once in index order, then
  stable merge-sort in O(n log n); elements with an X/Z key stay in place.
  Callbacks require packed items and cannot capture automatic locals/formals;
  generic leaf storage does not remove method/result limits. Shuffle uses its
  container seed API, not full process/object RNG integration.
  SV §7.12 **[SV-2005]**.
- 🟨 **Bit-stream casts and streaming** — Fixed arrays/nested records, selected
  rows/members, call results and admitted ref/const-ref projections preserve
  state conversion and non-dividing/type slice sizes within packed capacity.
  Oversized streams of integral arrays, selected rows, call results and constant
  in-bounds `with` ranges support multiple segments and unaligned slice sizes
  through a lazily read stream image; the RHS is snapshotted before publication.
  Runtime `with` ranges and nested oversized streams in such streams reject.
  Packed and bounded
  dynamic/queue-element streams capture one RHS, then publish destinations in
  stream order with overlap-safe snapshots. A stream assigned to a wider fixed
  target is left-aligned and zero-filled on the right; oversize streams reject.
  Unpacking consumes the leftmost required bits, also for `<<` with runtime
  ranges; undersized sources reject.
  `with` follows slice/declaration order, including descending arrays and `-:`
  ranges, and requires a one-dimensional operand: model and descriptor arrays,
  ref/const-ref formals, automatic locals, members, rows and call results. A
  source range past the bounds streams element defaults; a target range past
  them writes the in-range part and reports an error. Fixed destinations,
  including runtime `with` ranges, accept nonblocking streams with issue-time
  sources and selectors. At most one resizable destination is allowed;
  mixed/resizable destinations use a
  [blocking-only assignment path](../src/sim/codegen/lowering/containers/streaming.rs).
  Nonblocking or right-to-left selectors that read an earlier target of the same
  unpack, runtime target ranges in copy-out or delayed assignments, and runtime
  target ranges over mixed two-state/four-state record elements reject.
  Compound streaming assignments are outside the assignment grammar
  (SV §11.4.14.3, Annex A.6.2). Fixed-size cast mismatches, unpacked-union
  bit-stream casts, real/associative operands, native strings, recursive objects
  and unsupported reference combinations reject.
  SV §§6.24.3, 11.4.14 **[SV-2005]**.
- 🟨 **Let expressions** — Expansions bind free names and defaults in
  declaration scope, take positional, named, default and typed actuals,
  re-evaluate operands at every use, and return numeric and fixed packed,
  unpacked and descriptor-backed aggregate results in procedural, continuous,
  comparison and declaration-initializer contexts in SV2009, while V2001
  rejects the syntax. Recursive lets reject. Expanded assertion and native
  bodies still need an admitted consumer representation.
  SV §11.13 **[SV-2009]**.
- 🟨 **Operator overloading** — `bind <op> function` declarations in module,
  interface, program, package, compilation-unit, generate, block and
  subroutine scopes resolve `+ - * / % **`, unary `+ -`, `++ --`,
  `== != < <= > >=` and `=` where the built-in operator is illegal for the
  operand types; legal built-in operations keep their meaning. A resolved use
  is an ordinary call to the function found from the use's scope, so oversized
  fixed arrays cross it by descriptor. Exact formal matches win, with the
  integral implicit-cast exception; result types are selected by assignment,
  argument, port, return and cast contexts, and remaining ambiguity requires a
  cast. Declarations are visible after declaration and inner ones shadow outer
  ones; compound assignments build `A = op(A, B)` and increments
  `A = f(A)`, rejecting targets with side effects. A postfix increment's value
  and a prefix one used inside a larger aggregate expression, expected types
  from relational operands, and package declarations seen through imports are
  not admitted; V2001 rejects the syntax. Native/non-integral extensions are
  SIM-021. SV §11.11 **[SV-2005]**.

## 8. Continuous assignments & structural

- 🟦 **Continuous drivers** — Explicit, comma-form and net-declaration
  assignments evaluate from RHS dependencies; constant RHSs run once. Writes by
  functions called from the RHS are procedural (SV §6.5): they may share storage
  with procedural writers but not with a continuous driver. V §6.1 **[1995]**.
- 🟨 **Fixed-array continuous assignments** — Whole/selected fixed values,
  slices, conditionals, patterns and function returns capture one RHS per
  activation. Net arrays retain per-cell/per-bit contribution slots. Each
  positional pattern leaf is its own contribution to a net, unpacked-structure
  net or member, net-array cell or selected bits, or true-net alias view,
  beside competing drivers and a changing runtime RHS; leaves need constant
  selects. Oversized sources, including typed patterns, scatter descriptor rows;
  zero-delay drivers of whole descriptor arrays or constant descriptor rows copy
  through the fixed-array owner without flattening. A zero-delay driver whose write changes one of its own operands re-evaluates
  until it settles; nonconvergence stops at the process step limit. Variable
  storage permits one continuous writer per longest static prefix (member,
  packed-member range, cell, constant row of any width, hierarchical
  variable). Overlapping continuous, ordinary procedural or declaration-
  initializer writers reject; force/release/deassign are not ordinary writers.
  [Continuous-process lowering](../src/sim/codegen/lowering/collection/processes.rs)
  rejects delayed positional-pattern drivers, which with net-array declaration
  delays belong to ADV-002. Dynamic topology remains restricted.
  SV §§6.5–6.7, 7.6, 10.3 **[SV-2005]**.
- 🟨 **Resolution and strengths** — Continuous, gate, port and admitted
  hierarchical/fixed-array drivers retain independent contributions. `wire/tri`,
  `wand/triand` and `wor/trior` use their resolution rules; Z is neutral and
  equal-strength wired ties use AND/OR. Supply/strong/pull/weak/high-Z endpoints
  and X uncertainty are retained; a highz endpoint gives one-sided X, and
  undelayed `bufif`/`notif` gates with an unknown enable drive L/H. `tri0/tri1`
  and supplies are pull/supply sources, not post-resolution defaults. `%v`
  reports the resolved strength range (mnemonic, two-digit range, L/H) of nets,
  their constant selects, aliases and net-array cells; strength-only changes
  reach `%v` monitors while value readers wake only on value changes. The
  [RTL-012 matrix](../tests/fixtures/sim/feature_completion/rtl_012/readme.md)
  checks every source pair against an exhaustive outcome oracle in both
  editions. Explicit continuous strengths on vectors, selected vector bits and
  supply nets reject; gate instance arrays keep per-instance strengths. Delayed
  enable gates still drive X for an unknown enable; VPI strength values,
  charge/resistive contexts (ADV-005/006) remain outside this row.
  V §§3.4, 3.7, 6.1, 7.9–7.13, 17.1.1.5; SV §§10.3.4, 21.2.1.5 **[1995]**.
- 🟨 **Dissimilar inout collapse** — Admitted whole/selected/concatenated packed
  and fixed net-array row/cell connections use the directional port-type table
  through any depth. The winning declaration supplies resolver, default and
  propagation delay, including no delay. Parent links precede descendants, and
  each hierarchy depth is one batch independent of instance and port order:
  edge winners reduce to the types no other winner strictly dominates, and a
  same-depth warning-only tie selects the first in Table 23-1 column order with
  a located warning. A uwire actual collapses with one driver per bit (SV
  §6.6.2); `inout uwire` formals stay frontend-rejected. `trireg` (ADV-006),
  pass switches (ADV-005) and net-array declaration delays (ADV-002) remain
  unsupported; oversized net arrays still expand per cell (known issue).
  V §12.3.10 / Table 45; SV §§23.3.3.6–23.3.3.7 / Table 23-1 **[1995]**.
- 🟨 **True aliases** — Same-declared-net-type packed networks, fully indexed
  fixed net-array elements, multidimensional/static indexed projections and
  constant member selects of packed and unpacked structure nets (a local
  frontend patch admits the A.8.3/A.8.5 `constant_select` members) share
  force/release, dependency and waveform identity, also across inout ports.
  Ascending declarations with `+:`/`-:` selections preserve physical bit
  pairing. Dissimilar port rules do not relax alias type rules.
  Self/duplicate/overlapping/variable/incompatible/width-mismatched aliases,
  runtime selects, hierarchical (cross-scope) references and unmapped writes
  reject. Delayed alias drivers belong to ADV-002. SV §10.11 **[SV-2005]**.
- 🟨 **Continuous, gate and net delays** — Constant single/rise-fall/turn-off
  delays schedule captured Active-region inertial updates for admitted whole,
  constant-selected and fixed-array-element drivers. Changed pending results
  cancel; unchanged results retain deadlines; returning to the current value
  cancels without replacement. Net propagation delays apply after resolution.
  Conflicting selected true-alias delays, dynamic driver-delay expressions,
  net-array declaration delays and unsupported terminals remain restricted.
  V §§6.1.3, 7.14 **[1995]**.
- 🟦 **Logic and pull gates** — `and/nand/or/nor/xor/xnor/buf/not/pullup/pulldown`
  use typed terminals and independent output drivers. Pullup/pulldown values
  and an undriven Z net have a dual-edition public-CLI witness in both modes.
  Multi-output buf/not,
  input expressions/constants, selected/hierarchical references, mixed-width
  normalization and built-in instance arrays are represented; no blanket equal-width or 64-terminal limit applies. Outputs need an admitted packed structural
  target; real/invalid terminals and incompatible driver contexts reject.
  V §§7.1–7.3, 7.8 **[1995]**.
- 🟦 **Tri-state gates** — `bufif0/1` and `notif0/1` normalize enabled data-Z to
  X, drive Z when disabled and retain the gate's unknown-enable truth behavior.
  V §7.4 **[1995]**.
- 🟨 **Combinational UDPs** — Scalar tables/instances and primitive arrays admit
  `0/1/x/X/b/B/?`, treat input Z as X, return X for unmatched combinations and
  reject conflicting overlapping rows. Scalar terminals may select bits of
  vectors, packed/unpacked array elements (including descriptor-backed arrays
  above the packed limit), structure members, hierarchical names, constants and
  expressions; outputs may drive selected bits, net-array cells and
  hierarchical nets. Instance arrays slice whole vectors, part-selects,
  concatenations, literals and expressions, including multidimensional and
  unpacked-net-array connections. Independent drivers, legal strengths (`%v`)
  and delay2 inertial delays are represented. Each definition of up to 10
  inputs evaluates through one dense index lookup with inputs read in place.
  The SYN-031 and RTL-020 matrices run both editions, both optimizer modes and
  both value backends. Invalid port lists, row widths, terminal counts and
  vector/aggregate terminals are diagnostic boundaries (scalar-only by
  definition). An output on one cell of a large net array costs only that
  cell (200,000 cells: 21 KB of `model.c`); thousands of driven cells share
  the per-cell code cost of driven net-array cells ([known issue](known_issues.md#driven-net-array-cells-emit-per-cell-electrical-code)).
  Sequential UDPs are a separate missing capability below.
  V §§8.1–8.2, 8.6; SV §§29.3–29.4, 29.8 **[1995/SV-2009]**.
- ❌ **Sequential UDPs** — State-holding level/edge tables and UDP
  state-initialization semantics are not implemented. The
  [UDP regression sources](../tests/sim_udp.rs) retain explicit sequential/edge
  rejection checks; combinational UDP support does not imply sequential support.
  V §8.3; SV §§29.5–29.6 **[1995]**.
- ❌ **Switches and charge storage** — MOS/CMOS/resistive forms
  (`nmos/pmos/cmos/rnmos/rpmos/rcmos`), bidirectional `tran/tranif/rtran` families,
  `trireg`, charge strength, decay and sharing. V §§3.7.3, 7.5–7.7 **[1995]**.
- ❌ **Specify, timing checks and SDF** — `specparam`, module/state/edge paths,
  `PATHPULSE$`, notifiers, timing-check `&&&`, `$setup/$hold/$setuphold`,
  `$recovery/$removal/$recrem`, `$skew/$timeskew/$fullskew`, `$period/$width/$nochange` and `$sdf_annotate` delay/check application are not implemented.
  V §§3.11.3, ch.14–16; SV ch.30–32 **[1995/SV-2009 restored]**.

## 9. Functions & tasks

- 🟨 **Calls and storage** — Functions/tasks, expression returns, void functions,
  named arguments and defaults are represented. Defaults evaluate only when
  omitted and may read earlier captured formals without repeating side effects.
  Fixed integral array/struct/union inputs, locals, returns and output/inout
  copy-out retain declaration order, state and lifetime, including nested
  forwarding, named/default aggregate arguments, early returns/local disables
  and arrays beyond packed capacity (descriptor transport). Numeric and admitted
  native string/chandle signatures have separate paths. Static outputs retain
  formal storage; only inout copy-in overwrites it, while automatic outputs get
  typed defaults. General native/resizable aggregates remain restricted.
  V §§10.2–10.3; SV §§13.3–13.5 **[1995/2001/SV-2005]**.
- 🟦 **Automatic/reentrant and finite zero-time calls** — Per-activation
  storage, finite recursion, local named-block exits, selected copy-out and
  nested const-ref forwarding are represented. Static scalar and fixed-array
  storage is per declaration and module instance, including explicit static
  locals in automatic subroutines.
  Recursion is capacity-limited as stated above. V §§10.2.3, 10.3.1;
  SV §§6.21–6.22, 13.3–13.5 **[2001/SV-2005]**.
- 🟨 **References** — `ref`/`const ref` alias matching packed variables, fixed
  integral arrays/records/unions, admitted unpacked members/elements (including
  members of unpacked-record array elements, with runtime indices bound once at
  the call) and retained packed queue cells. Removal/reallocation preserves a queue reference's original
  detached cell. String/chandle references use native storage. General native/
  resizable aggregates, non-packed queue references and reference-formal NBAs
  remain restricted. Fixed packed scanner destinations retain checked selected
  views through ref formals. Subroutine actuals must be
  eligible variables, not function/reduction/conditional/cast/pattern temporaries;
  packed bit/part actuals rejected by the frontend are not legalized by internal
  projections. SV §13.5.2 **[SV-2005]**.
- 🟨 **Timed tasks and parallel bodies** — Admitted delay-only tasks suspend;
  event/cancellation-sensitive calls preserve values/selectors and check
  cancellation before copy-out. Synchronous joined numeric branches can share
  the suspended activation; join_any/join_none retain supported snapshots of
  automatic packed/real values and recognized handles. This does not provide
  arbitrary detached alias propagation or ref-formal captures. Static-local
  task NBAs are represented, including arrays; NBAs targeting automatic locals
  or automatic formals reject. V §§9.8.2, 10.2; SV §§9.3.2, 10.4.2, 13.3.2
  **[1995/SV-2005]**.
- 🟨 **Hierarchical calls and special formals** — Resolved module/interface/
  package/parent callees retain their owning instance, including fixed aggregate
  formals/results and per-instance static state. Input event formals are typed
  by-value event-handle parameters; a `ref` formal read by an event control
  binds a whole module-signal actual per specialized task copy; output/inout/ref
  event formals, event controls reading other subroutine storage and
  class-method event formals use inline task paths; event-formal virtual dispatch and process-handle formal ABI remain unsupported. Unresolved environments and broader
  timing/native/aggregate combinations reject. V §12.4 **[1995]**.
- 🟨 **Read-only helper calls** — Event, continuous, force and other read-only
  evaluators admit bounded numeric value/const-ref helpers with private locals,
  loops, nested eligible calls and fixed integral reductions. Private
  concatenation/positional-pattern stores and compound mutations are allowed
  only when every destination is private. Stateless static formals use private
  callback copies; result independence must hold across normal/return/loop exits.
  Legal helpers that are not read-only (visible writes, persistent static state,
  descriptor-array formals) run in the waiting process for blocking event
  controls and in the evaluating process for continuous assignments, whose
  called-function writes are procedural (SV 6.5); event evaluation count is the
  owner policy of one evaluation at arm time and one per dependency change.
  Force sources, intra-assignment and NBA/`->>` event controls and named-event
  or real event lists also evaluate them in processes (RTL-007b).
  `$monitor`/`$strobe` arguments run them in Postponed only when their stores
  target the helpers' own storage, applied without publication; a visible write
  rejects (SV 4.4.2.9). Native/DPI dispatch, suspension and arbitrary
  shared/native captures reject.
  Unique/priority diagnostics remain active; side-effect-free source alone does
  not establish eligibility.
- ❌ **Unsupported or illegal call forms** — Recursive task calls requiring
  inline-only event/ref environments remain rejected. Typed recursive
  delay-bearing tasks use independent SCC/arena activations; direct and mutual
  recursion have witnesses in [the function/task suite](../tests/sim_function.rs).
  Timing-bearing class/virtual-interface tasks and jumps into other lexical
  scopes/backward unstructured jumps remain rejected. Direct task calls and
  blocking `#/@/wait` inside functions are illegal; timing in an admitted detached
  join_none branch is separate. Disabling a task leaves output/inout results
  unspecified, unlike local block-disable followed by normal task return.
  V §§10.3.4, 11; SV §§9.6, 12.8, 13.4.

## 10. System tasks & functions relevant to simulation

### Output and formatting

- 🟦 **Console/file output** — `$display/$write/$strobe/$monitor`, monitor
  `$monitoron/$monitoroff` and b/o/h variants;
  `$fopen/$fclose/$fdisplay/$fwrite/$fstrobe/$fmonitor` support owned descriptors, standard streams and multichannel fan-out.
  Write omits the added newline; explicit format conversions override variant
  radix. V §§17.1, 17.2.1–17.2.2 **[1995/SV-2005]**.
- 🟨 **Typed formatting** — `%d/%h/%x/%b/%o/%c/%u/%z/%v/%t`, `%f/%e/%g`, `%s`,
  `%m`, `%l`, `%%` and admitted width/precision grammar retain packed X/Z,
  strengths, real and string values. A `%h/%x/%o` digit or `%d` value with every
  bit x/z prints `x`/`z`; a partially unknown one prints `X` (any x bit) or `Z`.
  Packed `%s` uses ASCII bytes without leading
  zero bytes; real `%s` rejects. `%p` is limited to scalar packed/string values,
  not aggregates. V §17.1.1.2; SV §21.2 **[1995/SV-2005]**.
- 🟨 **Postponed output** — Strobe observes settled values after Active/Inactive/
  NBA iteration. One active monitor coalesces same-slot changes; registration and
  re-enable queue one report. Packed, real and string changes are supported;
  automatic monitor dependencies and aggregate/container formatting reject.
  Helpers may keep their own static state or take descriptor arrays; visible
  helper writes reject (§9). V §§17.1.2–17.1.3 **[1995]**.
- 🟦 **String formatting** — `$sformat/$swrite` and radix variants write native
  strings or packed string-like destinations with normal padding/truncation.
  `$sformatf` returns an owned string. Dynamic/nested formats and arguments
  evaluate once in source order. V §17.2.3; SV §21.3.3 **[2001/SV-2005]**.

### Input and memory files

- 🟨 **Scanning/character/line input** — `$fscanf/$sscanf/$fgets/$fgetc/$ungetc`
  retain input/format values, admitted packed/selected/string/real destinations,
  EOF, byte and X/Z behavior. Fixed packed scanner sub-accesses through ref
  formals preserve checked selection plans; general aggregate targets reject. V §17.2.4; SV §21.3.4 **[2001/SV-2005]**.
- 🟨 **Binary input** — `$fread` supports admitted packed/memory targets with
  bounded start/count. Rank-one memories advance from lowest to highest HDL
  address in either declaration direction. Packed reads accept and ignore
  start/count; the exact memory-storage argument is legal in 2001 without
  admitting unrelated whole-array value uses. Packed-bound and memory-edition
  cases have [file-I/O](../tests/sim_file_io.rs) and
  [edition](../tests/sim_edition.rs) regression sources; these were not rerun here.
  V §17.2.4.4; SV §21.3.4.4
  **[2001/SV-2005]**.
- 🟦 **File position/status** — `$ftell/$fseek/$rewind/$fflush/$ferror/$feof`
  preserve EOF/error and invalid/closed-descriptor status. V §§17.2.5–17.2.7;
  SV §§21.3.5–21.3.8 **[2001/SV-2005]**.
- 🟨 **Memory images** — `$readmemh/$readmemb` accept owned string paths,
  whitespace/comments, binary/hex digits, X/Z and `@` addresses.
  Fixed packed memories may have multiple unpacked dimensions and selected
  rows/slices; runtime view selectors are captured once. Remaining dimensions
  walk low-to-high in row-major order; one-dimensional 2001 omitted ranges
  instead follow declaration direction. 2009 defaults, including start-only
  calls, are low-to-high; explicit ranges retain requested direction.
  `@` addresses the highest remaining dimension. Invalid selected ranges leave
  every cell unchanged; invalid later jumps stop while retaining earlier writes.
  The admitted signed `@` extension converts checked hexadecimal magnitude to
  a signed index after sign application; zero padding of the token does not
  change the selected cell. Magnitudes outside signed 64-bit index range and
  jumps outside a selected view diagnose before another write.
  Incomplete rows leave unread subwords unchanged.
  Public SV2009 coverage includes mixed-direction 3-D declarations, a runtime
  selected slice with explicit descending bounds, a 129-bit selected word, and
  settled per-leaf reader notification after a selected load.
  **Allowed nondeterminism (Q02):** Same-slot readers may interleave under
  V §§5.4–5.5 / SV §§4.6–4.7. Each read follows its triggering word update;
  the memory-load clauses do not impose a total order between independent
  readers or require the caller to print before them. `q02_wakeup` in
  `sim_undefined_behavior` records current `llg` order, **not a conformance
  claim**; a conformance test must use these ordering constraints.
  V §17.2.8; SV §21.4 **[1995/SV-2005]**.

  Short words pad a leading X/Z with that state; known digits zero-extend.
  Ordinary packed memories truncate normally. Enum words are checked at original
  width, allowing redundant signed extension; invalid words diagnose and stop
  without undoing earlier writes. Two-state X/Z conversion occurs before enum
  range checking, without discarding high bits. Sparse addressed 2009 files omit
  the no-address short-file warning; 2001 count warnings remain, including excess
  data from repeated in-range addresses. **Unresolved oracle (Q02):** The
  short X/Z token-extension intersections for binary and hex words of different
  widths, signed/two-state/enum destinations and selected views, plus malformed
  token handling, four-state enum numeric corners and diagnostic details beyond
  the prescribed warning/error categories, have no settled oracle in the
  supplied V §17.2.8 / SV §§21.4–21.4.3 text. The `q02_*` fixtures in
  `sim_undefined_behavior` record current `llg` values, diagnostics and status,
  **not a conformance claim**; resolution requires a clause-based oracle.
  Dynamic arrays and queues of packed words (without resizing), and
  integral-key associative memories, are legal under SV §21.4.1 but unsupported.
  Real/non-packed words and fully indexed scalar targets are outside the
  memory-load argument contract; string-key associative memories are prohibited.
- 🟦 **Memory writers** — `$writememh/$writememb` emit consumable four-state words
  from admitted fixed-memory views in low-to-high row-major order. The same
  element/view restrictions apply; writers reject in 2001 mode.
  SV §21.5 **[SV-2005]**.

- ❌ **String-sourced memory loading** — `$sreadmemb/$sreadmemh` appear in the
  frontend's standard-name allowlist, but have no built-in simulator lowering or
  runtime implementation. File-based memory loading and `$sscanf` do not provide
  these routines; the generic VPI fallback requires an external registration.
  These are optional informative services (V Annex C.13; SV Annex D.14), not
  required language features.

### Time, control and utility services

- 🟨 **Time reporting** — `$time` rounds to the calling module's unit (exact
  halves upward); `$stime` then returns its low 32 bits; `$realtime` retains
  fractions. Design-wide `$timeformat` units/precision/suffix/minimum width and
  `%t` integral/real conversions are represented. **`$printtimescale(scope)`
  does not honor its scope operand**: the
  [system-task lowerer](../src/sim/codegen/lowering/statements/system_tasks.rs)
  emits the caller's timescale and label without reading that operand. Only the
  no-argument caller-scope behavior is represented correctly by this path.
  `$timeformat` accepts the documented zero-or-four-argument syntax
  (V §17.3.2, Syntax 17-10; SV §20.4.2, Syntax 20-4). Intermediate arities
  would be a convenience extension, not a required missing feature.
  The [timescale regressions](../tests/sim_timescale.rs) contain a no-argument
  witness, not proof of explicit-scope behavior.
  V §§17.3, 17.7; SV §20.4 **[1995/SV-2009]**.
- 🟦 **Finish and severity** — `$finish` is nonreturning with default diagnostic
  level 1; `$fatal/$error/$warning/$info` format messages once in source order
  with source/scope context. Fatal terminates through the final-block handoff;
  nonfatal levels continue. Finish/fatal numbers must be constant integral
  0/1/2, not OS exit codes; level 2 includes severity statistics.
  V §17.4.1; SV §§20.2, 20.9–20.10 **[1995/SV-2005]**.
- 🟨 **Stop/resume** — `$stop` preserves pending work, activations, output and
  time. `--stop-policy resume` is the noninteractive default; `exit` returns
  control without finals or future-work draining. Stop numbers use the same
  constant 0/1/2 diagnostic levels. A full interactive debugger is not provided.
  V §17.4.2 **[1995]**.
- 🟦 **Conversions** — `$rtoi` truncates toward zero; `$itor` handles numeric
  signed/unsigned conversion. `$realtobits/$bitstoreal` and
  `$shortrealtobits/$bitstoshortreal` reinterpret IEEE-754 at 64/32 bits; reverse
  conversions require that width and zero X/Z positions. Shortreal rounding is
  retained. V §17.8; SV §20.5 **[1995/2001/SV-2005]**.
- 🟨 **Data/array queries** — `$typename/$isunbounded/$bits`,
  `$left/$right/$low/$high/$increment/$size/$dimensions/$unpacked_dimensions`
  use represented packed, fixed-array, dynamic/queue, associative-integral and
  string metadata. Fixed queries cover selected rows, declared formal bounds and
  descriptor-backed arrays; an out-of-range or unknown dimension yields X.
  Nested runtime dimensions remain unsupported. Associative
  dimension bounds require integral index types; string/wildcard bounds are
  prohibited by SV §20.7. SV §§20.6–20.7 **[SV-2005]**.
- 🟦 **Bit and math helpers** — `$onehot/$onehot0/$countones/$isunknown` use
  packed X/Z-aware values and reject reals. `$clog2` and all 21 real functions
  mapped by SV Table 20-4 support admitted constant/runtime values, numeric
  coercion and C math-domain behavior. SV §§20.6, 20.8 **[SV-2005]**;
  `$clog2` first appeared in **[1364-2005]**.
- 🟨 **Random facilities** — Legacy `$random` and
  `$dist_uniform`, `$dist_normal`, `$dist_exponential`, `$dist_poisson`,
  `$dist_chi_square`, `$dist_t` and `$dist_erlang` retain checked arguments,
  writable seeds and the project's Annex N algorithm. Per-process
  `$urandom/$urandom_range`, `srandom`, randstate and fork-derived streams have
  paths. Class-object RNG, constrained/structured randomization and full
  container-shuffle/process-stream integration remain absent.
  V §17.9; SV ch.18 **[1995/SV-2005]**.
- 🟨 **Stochastic queues** — `$q_initialize/$q_add/$q_remove/$q_full/$q_exam`
  support ID-indexed FIFO/LIFO state, job/information IDs, status codes,
  precision-tick arrivals and six statistics. Outputs must be whole packed
  integers; selected/real targets reject. V §17.6; SV §20.16 **[1995]**.
- ❌ **PLA tasks** — No legacy PLA target is selected. All synchronous/asynchronous
  AND/NAND/OR/NOR array/plane families (such as `$async$and$array`) lack
  simulator lowering. A call receives an explicit unsupported-PLA diagnostic;
  it does not execute as a no-op. V §17.5;
  SV §20.17 **[1995]**.
- 🟦 **Plusargs** — Arguments after `--` reach `$test$plusargs/$value$plusargs`.
  Leading-`+` prefix matching, first repeated match, literal `%%`,
  `%d/%h/%x/%o/%b/%f/%e/%g/%s`, wide four-state values and unchanged destinations
  on failure are represented. V §17.10; SV §21.6 **[1995/SV-2005]**.
- 🟦 **Host commands** — `$system` evaluates one optional string once and
  requires generated-process `LLG_ALLOW_SYSTEM=1` (also `true/yes/on`). Denial
  diagnoses, returns signed 32-bit -1 and fails without shell execution.
  Omitted commands call `system(NULL)`, distinct from empty strings; enabled
  calls return raw host status. Embedded NUL rejects; shell/status behavior is
  platform-specific. SV §20.18 **[SV-2009]**.

- ❌ **Unimplemented legacy system services** — No built-in paths were found
  for `$countdrivers`, `$getpattern`, `$scale`, `$scope/$showscopes/$showvars`,
  `$list/$input/$key/$nokey/$log/$nolog`, or
  `$save/$incsave/$restart/$reset/$reset_count/$reset_value`. Their presence in
  the [edition allowlist](../src/core/compile/editions.rs) is not implementation
  evidence. An admitted call can reach generic VPI dispatch; without a matching
  plugin, the [runtime bridge](../src/sim/rt/llg_vpi.c) reports an unresolved or
  unregistered call. Other contexts may reject earlier. This is distinct from
  the implemented numeric conversions, file I/O, `$stop` policy and VPI API.

### Waveforms

- 🟨 **VCD/FST** — `$dumpfile` selects `.vcd`/`.fst`; `$dumpvars` filters depth,
  scope and variables before the fixed header. Packed values, array elements
  with declared bounds, real values, X/Z, aliases, hierarchy and femtosecond
  timestamps are represented. `$dumpon/$dumpoff/$dumplimit`, `$dumpall`
  snapshots and synchronous `$dumpflush` are supported. Extended VCD
  `$dumpports` and its related controls remain unsupported.
  V ch.18 **[1995/2001]**.

## 11. Compiler directives affecting simulation

- 🟦 **Macros, includes and conditional compilation** — 2001 parameter macros,
  `define`/`undef`, `ifdef`/`ifndef`/`elsif`/`else`, generated ranges and
  caller-ordered admitted includes affect executable values in separate and
  merged units. 2009 adds token paste and stringification to construct names
  and strings; the strict 2001 profile rejects those later operators, predefined
  `__FILE__`/`__LINE__`, `undefineall`, `pragma` and keyword directives.
  Literal and bounded object/function-like macro-expanded include names resolve
  within admitted source/include roots. Missing, dynamic and unauthorized paths
  fail admission. V §§19.3–19.5; SV §§22.4–22.6,
  22.11, 22.13–22.14 **[2001/SV-2009]**.
- 🟦 **`` `timescale ``** — Resolved module/declaration inheritance scales delays
  and time reporting; local `timeunit/timeprecision` and rounding are covered in
  §§1 and 6. V §19.8 **[1995]**.
- 🟦 **`` `default_nettype `` / `` `resetall ``** — Control implicit-net admission
  and reset later directive state; `none` rejects undeclared nets, `wire`
  restores them, and separate/merged units retain their specified boundaries.
  V §§19.2, 19.6 **[1995]**.
- 🟦 **Cell tags and pragmas** — `` `celldefine `` / `` `endcelldefine `` and
  `` `pragma `` are frontend-consumed; no dedicated simulator effects are claimed.
  `vectored/scalared` and standard attributes are accepted as simulation-neutral
  metadata, with packed values unchanged. V §§2.8, 3.3.2, 19.1;
  SV §22.11 **[1995/SV-2009]**.
- 🟨 **`` `unconnected_drive `` / `` `nounconnected_drive ``** — An omitted
  input net formal, including a net-array formal, receives a pull0/pull1
  contribution that competes with the formal's internal drivers and net type
  (tri0/tri1, wired and supply nets); variable formals receive the value without
  strength. Includes, `` `resetall ``, generated and arrayed instances and
  separate/merged compilation units are covered. Resizable (dynamic) formals
  remain outside the qualified boundary. V §19.9; SV §22.9 **[1995]**.
- 🟨 **Source mapping** — `` `line ``, `` `__FILE__ `` and `` `__LINE__ `` expose
  mapped values in 2009 through nested includes, include restoration, macro
  use sites and both compilation-unit modes; a directive maps only its own
  file. 2001 admits `` `line `` without the later predefined macros. The owned
  Db keeps each node's physical position and a separate per-file `` `line ``
  map. Frontend diagnostics and scope-based runtime locations (`$finish`,
  severity tasks) stay physical; file-based simulator diagnostics, assertion
  messages and coroutine site locations append `` (`line file:line) ``.
  Physical `` `__FILE__ `` is the opened file's base name. Frontend diagnostics
  do not yet carry the mapped position. V §19.7; SV §§22.12–22.13
  **[2001/SV-2009]**.
- 🟦 **Keyword/macro state** — `` `begin_keywords `` / `` `end_keywords `` retain
  lexical tables without changing the edition; `` `undefineall `` clears macros.
  SV §§22.5.3, 22.14 **[SV-2005/SV-2009]**; keyword directives first appeared in
  **[1364-2005]**.

## 12. Verification and foreign interfaces

These are bounded implementations, not full verification-infrastructure support.

- 🟨 **Classes** — Nominal construction, constructor chaining, inherited/static/
  virtual/super dispatch, parameterized layouts, forward/const/access metadata
  and null checks are represented. Packed/real/string/chandle/class-handle fields
  have explicit storage. Objects, including unreachable cycles, remain until
  model close; there is no garbage collector. Timing-bearing tasks, unsupported
  field/capture layouts and constrained randomization remain restricted.
  SV ch.8 **[SV-2005]**.
- 🟨 **Virtual interfaces** — Typed instance/modport identity survives rebinding,
  class/formal/fixed-array and bounded dynamic/queue storage. Packed member access,
  delay-free methods, null/type checks and clocking-input samples are present.
  Timed tasks, event-formal dispatch, dynamic clocking output/inout dispatch,
  associative/nested layouts and broader polymorphic/capture forms reject.
  SV §§25.5, 25.7, 25.9–25.10 **[SV-2009]**.
- 🟨 **Programs** — Initials launch in Reactive; `#0`/NBA stay in the reactive
  set. `$exit` cancels only its program-initial origin and is ignored outside
  one. Last-initial completion cancels that program's detached descendants;
  all program initials completing causes immediate implicit finish. The broader
  program phase remains unaccepted. SV §§24.3, 24.7 **[SV-2005]**.
- 🟨 **Clocking** — Declarations, aliases, default/global clocks, concrete
  interfaces and events support input `#1step`, `#0` and constant positive-skew
  samples. Constant output/inout skews capture values for Re-NBA; off-event
  drives wait for the next event. Inouts retain resolved-net drives and sampled
  reads; admitted signal-edge qualifiers and packed selected targets are present.
  [Input collection](../src/sim/codegen/lowering/clocking_context.rs) requires a
  collected packed signal: real-valued inputs and uncollected source expressions
  reject. Output/inout writes must use admitted nonblocking assignments; compound
  writes and concatenated clockvar lvalues are prohibited by SV §14.16. Skews
  must be constant expressions (SV §14.4); runtime skews are language-illegal.
  Unsupported cross-clock forms remain excluded.
  SV ch.14 **[SV-2005]**.
- 🟨 **Process control** — `process::self/status/kill/suspend/resume/await`
  retain identity, wait conditions, descendant cleanup and terminal status.
  Process formals/arrays and the broader class API remain unsupported.
  SV §9.7 **[SV-2005]**.
- 🟨 **Semaphores** — `new/get/put/try_get`, zero-key operations, FIFO blocking,
  cancellation cleanup and automatic task-handle arguments are represented;
  semaphore arrays remain unsupported. SV §15.3 **[SV-2005]**.
- 🟨 **Mailboxes** — Typed/untyped bounded/unbounded FIFO
  `new/num/put/get/peek/try_put/try_get/try_peek` supports packed, real/shortreal,
  string and admitted handle messages with nominal enum/class/handle identity.
  Empty, mismatch and delivery differ; mismatch preserves message/destination,
  peek does not consume, and consuming delivery commits before callbacks.
  Delivery survives admitted reentrancy/cancellation; arrays, general aggregates
  and arbitrary automatic native/shared captures reject. SV §15.4 **[SV-2005]**.

### Assertions and sampled values — partial

- 🟦 **Immediate assertions** — Ordinary `assert/assume/cover` evaluates its
  condition once with four-state truth and executes the selected admitted action.
  Actions inherit the restrictions of their containing process/subroutine and
  statement kinds. SV §16.3 **[SV-2005]**.
- 🟨 **Deferred immediate assertions** — `#0` forms retain copied issue-time
  values, admitted action-time references, same-slot coalescing and Reactive
  actions; module-level members are represented. The
  [deferred-action lowerer](../src/sim/codegen/lowering/statements/assertions.rs)
  rejects function/task/final-block contexts. An action must lower to one
  supported call (or an empty action); packed/real values and admitted static
  packed/string references have capture paths. Native-string value arguments,
  chandle arguments, output/inout copy-out, `$fatal`, `$finish` and `$stop`
  actions reject. A literal formatting string is not a captured native-string
  value argument. Output/inout action formals and multi-statement action blocks
  are prohibited by SV §16.4; their rejection is a legality boundary.
  Post-2009 `assert final` remains edition-gated and unsupported
  by this lowerer. SV §16.4 **[SV-2009]**.

🟨 **Concurrent assertions** support the following bounded forms (SV ch.16, Annex F):

| Area | Implemented forms |
| --- | --- |
| Attempts and actions | Preponed packed sampling, Observed resolution, overlapping attempts, vacuity accounting and Reactive actions. |
| Clock/disable flow | Nearest default-clock inheritance, compatible declaration/call-site clocks, legal multiclock `##0`/`##1` boundaries, asynchronous single-signal `disable iff`, bounded `accept_on/reject_on` and synchronous variants. |
| Sequences | `##` concatenation/ranges; consecutive/nonconsecutive/goto repetition with unbounded endpoints; `or`; direct one-cycle `and/intersect/throughout/within`; `first_match`. |
| Properties and instances | One-cycle `not/and/or/iff/implies` and `if/else` forms; named sequence/property instances with positional/named/default arguments. |
| Locals and match items | Per-attempt local input capture/defaults, ordered assignment/increment/subroutine-call items and isolated local snapshots for overlapping/branching threads. |
| Control | Bounded blocking `expect`, sequence `.matched`, `$asserton/$assertoff/$assertkill` and hierarchy selectors. Internal post-2009 level-0 ON/OFF/KILL `$assertcontrol` support is not admitted by either target edition. |

Output/inout/ref formal copy-out, delayed/nested local-formal calls, selected-local
lvalues, repeated match-item bodies, unsupported expanded bodies, conflicting
clock/disable metadata, nested `disable iff` or nested `accept_on/reject_on`
controls, conditional properties without an `else`, other temporal/cross-clock
combinators, pass/fail/vacuity action controls, `.triggered` and invalid
scopes/arguments remain rejected by the
[assertion lowerer](../src/sim/codegen/lowering/assertions.rs).
Match-item callees and shared/native captures must also satisfy §9's read-only
rules; graph/lowering support alone is not executable acceptance.

🟨 **Sampled functions** — `$sampled/$rose/$fell/$stable/$changed/$past` and
2009 global-clock history/status forms support packed explicit/default edge
domains, gated/initial history, Preponed reads and LSB/X/Z edge rules; `$past`
counts only clock time steps strictly before its evaluation. Future global
forms, complex clocks and real-valued sampling remain rejected. Future global
functions are legal in SV2009 property/sequence contexts under §16.9.4, with
global clocking, nonnesting and match-item restrictions and delayed assertion
actions; their rejection is an implementation gap. Procedural and action-block
uses remain illegal. SV §§16.9.3–16.9.4,
20.13 **[SV-2005]**.

### Foreign interfaces and missing infrastructure

- 🟨 **DPI-C imports** — Bounded scalar bit/logic/reg, integral, real/shortreal,
  chandle/string signatures and explicit libraries retain owned outputs and
  borrowed inputs. Foreign string results are copied before aliased copy-out,
  which checks cancellation. Exports, packed/open arrays and context callbacks
  are unsupported. DPI ref directions and event/class signature types are
  prohibited by SV §§35.5.1.2, 35.5.6; they remain negative legality cases.
  SV ch.35 **[SV-2005]**.
- 🟨 **PLI/VPI bridge** — Generated metadata, registration,
  `compiletf/sizetf/calltf`, scalar/vector/real values, hierarchy lookup/iteration,
  plugin startup and start/end callbacks have paths; arguments use registered
  snapshots. Full `tf_/acc_/vpi_`, arbitrary properties/iterators/regions/control
  remain unsupported. Registered packed/real system-function calls have direct
  expression-evaluation paths; foreign calls nested in proven read-only HDL
  helpers remain unsupported. Direct calls do not establish those helper paths.
  V ch.20–27 **[1995/2001]**.
- ⬜ **Checkers** — Checker execution and checker bind are not implemented;
  module/interface structural bind and ordinary assertion instances do not imply
  checker support. SV **[SV-2009]**.
- ⬜ **Functional coverage** — Covergroups, coverpoints, bins, crosses and their
  sampling/query/control/database services are not implemented. `cover` assertions
  and waveform/VPI observation are separate capabilities. SV **[SV-2005]**.
- ⬜ **Constrained and structured randomization** — `rand/randc`, constraints,
  object or `std::randomize`, `randcase` and `randsequence` are not implemented.
  Numeric random functions and container shuffle do not establish these features.
  SV **[SV-2005]**.

## Source and regression evidence

The following are representative implementation and regression entry points,
not a list of tests executed during this review. A test may be a positive case,
an explicit rejection or a product-characterization oracle. The
[test guide](../tests/readme.md) maps the larger fixture families; the feature
rows above remain the authority for scope and known gaps.

| Area | Implementation entry points | Representative regression sources |
| --- | --- | --- |
| Editions, preprocessing and frontend ownership (§§1, 11) | [Edition admission](../src/core/compile/editions.rs), [compile facade](../src/core/compile.rs), [Slang bridge](../src/wrapper/slang_c_api.cpp) | [Edition gates](../tests/sim_edition.rs), [compilation units](../tests/compilation_units.rs), [directives](../tests/sim_syn017_directive_effects.rs), [owned model](../tests/model_tests.rs) |
| Storage and lifetimes (§2) | [Collected aggregates](../src/sim/codegen/lowering/collection/aggregates.rs), [objects](../src/sim/codegen/lowering/objects/), [value runtime](../src/sim/rt/value/), [container runtime](../src/sim/rt/container/) | [Type matrix](../tests/sim_type_conformance.rs), [native datatypes](../tests/sim_data_types_next.rs), [dynamic ownership](../tests/sim_dynamic_ownership.rs) |
| Elaboration, ports and hierarchy (§3) | [Design collection](../src/sim/codegen/lowering/collection/design.rs), [ports](../src/sim/codegen/lowering/collection/ports.rs), [library mapping](../src/core/compile/library_mapping.rs), [configs](../src/core/compile/library_configs.rs) | [Elaboration](../tests/sim_syn016_elaboration.rs), [library configurations](../tests/sim_syn032_library_configs.rs), [bind](../tests/sim_syn033_structural_bind.rs) |
| Processes and timing (§§4, 6) | [Statement dispatch](../src/sim/codegen/lowering/statements/dispatch.rs), [event lowering](../src/sim/codegen/lowering/statements/events.rs), [scheduler](../src/sim/rt/scheduler/) | [Process semantics](../tests/sim_process_semantics.rs), [events](../tests/sim_events.rs), [timing/regions](../tests/sim_partial_features.rs) |
| Statements, values and expressions (§§5, 7) | [Assignments](../src/sim/codegen/lowering/statements/assignments.rs), [expressions](../src/sim/codegen/lowering/expressions/), [container streaming](../src/sim/codegen/lowering/containers/streaming.rs) | [Loops](../tests/sim_loops.rs), [stream contexts](../tests/sim_syn015_stream_contexts.rs), [tagged patterns](../tests/sim_syn024_tagged_patterns.rs), [fixed reductions](../tests/sim_fixed_array_reductions.rs) |
| Structural drivers (§8) | [Continuous processes](../src/sim/codegen/lowering/collection/processes.rs), [nets](../src/sim/codegen/lowering/collection/nets.rs), [gates/UDPs](../src/sim/codegen/lowering/collection/gates.rs) | [Resolution](../tests/sim_net_resolution.rs), [port types](../tests/sim_port_net_types.rs), [UDPs](../tests/sim_udp.rs) |
| Subroutines and callback limits (§9) | [Call contracts](../src/sim/codegen/lowering/collection/call_contracts.rs), [function bodies](../src/sim/codegen/lowering/collection/function_bodies.rs), [owned emitter](../src/sim/emit_c/owned/) | [Functions](../tests/sim_function.rs), [zero-time calls](../tests/sim_syn013_zero_time_calls.rs), [activation frames](../tests/sim_partial_features/activation_frames.rs) |
| System tasks and memory I/O (§10) | [System tasks](../src/sim/codegen/lowering/statements/system_tasks.rs), [system functions](../src/sim/codegen/lowering/expressions/system_functions.rs), [runtime](../src/sim/rt/llg_rt.c) | [File I/O](../tests/sim_file_io.rs), [memory views](../tests/sim_memory_views.rs), [memory editions](../tests/sim_memory_editions.rs), [time reporting](../tests/sim_timescale.rs) |
| Assertions, clocking and verification objects (§12) | [Immediate/deferred assertions](../src/sim/codegen/lowering/statements/assertions.rs), [concurrent assertions](../src/sim/codegen/lowering/assertions.rs), [clocking](../src/sim/codegen/lowering/clocking_context.rs), [class collection](../src/sim/codegen/lowering/collection/classes.rs) | [Concurrent assertions](../tests/sim_concurrent_assertions.rs), [deferred assertions](../tests/sim_partial_features/assertions.rs), [clocking](../tests/sim_partial_features/clocking.rs), [classes](../tests/sim_classes.rs), [virtual interfaces](../tests/sim_virtual_interfaces.rs) |
| Foreign interfaces and observation (§§10, 12) | [Native access IR](../src/sim/ir/native_access.rs), [VPI runtime](../src/sim/rt/llg_vpi.c), [waveforms](../src/sim/rt/llg_wave.c) | [DPI](../tests/sim_dpi.rs), [VPI](../tests/sim_vpi.rs), [waveforms](../tests/sim_waveform.rs) |
| Executable validation and cross-feature boundaries | [Semantic coverage](../src/sim/semantic.rs), [IR validation](../src/sim/ir/validate.rs), [whole-model emission](../src/sim/emit_c.rs) | [Optimizer comparison](../tests/sim_opt_differential.rs), [integrated profile](../tests/sim_syn039_acceptance.rs), [Q02 characterization](../tests/sim_undefined_behavior.rs) |

Source support must survive the **whole-model ownership emitter**, not only
frontend/lowering checks. Legacy expression/statement fragment APIs deliberately
return an ownership-migration error; unsupported owned callbacks, captures,
queued writes or native layouts can still reject a combination of otherwise
implemented features. Such rejection must not be counted as complete support
because an older rendering helper contains code for the operation.
