# Simulator Feature Checklist — IEEE 1364 / IEEE 1800

This is the maintained `llg` simulation-feature inventory. Support is limited to
the forms and contexts stated below; it does not imply full IEEE conformance or
synthesis-tool acceptance.

<a id="dynamic-value-migration-acceptance-boundary"></a>

## Status markers

| Marker | Meaning |
| --- | --- |
| 🟦 Source-implemented | An implementation path exists for the stated scope. |
| 🟨 Partial | Implemented forms have the listed context, lifetime or representation limits. |
| ❌ Not implemented / rejected | No supported execution path; language-illegal forms are identified separately. |
| ⬜ Out of scope | Unimplemented advanced forms outside the current scope. |
| ✅ Accepted | A specific scope has matching post-change HDL evidence; not a whole-family claim. |

Source and test presence are not acceptance. Recorded Linux results do not qualify
later changes, full generated-model sanitizer coverage, or native Windows/macOS
execution. Combinations of implemented features can still encounter the listed
storage and callback restrictions. Test methodology and coverage data are in the
[test guide](../tests/readme.md).

References: **V** = IEEE 1364-2001; **SV** = IEEE 1800-2009. Revision tags retain
**[1995]**, **[2001]**, **[1364-2005]**, **[SV-2005]** and **[SV-2009]**.

## Target language editions

`--edition 2001|2009` selects one compilation-wide policy; the default is 2009.
`` `begin_keywords `` / `` `end_keywords `` change lexical keywords in 2009,
not that policy; the 2001 profile rejects them. The pinned Slang API exposes
semantic checks from 2017 onward; the project's 2009 keyword, time and
later-form gates do not establish complete 2009 semantic conformance.

| Boundary | Verilog-2001 | SystemVerilog-2009 |
| --- | --- | --- |
| Legacy modules, ANSI/non-ANSI ports, `assign`, `always @*`, `$display`, `$finish`, `$signed` | Admitted | Admitted |
| Scalar declaration initialization | Active-process race retained | Static initialization precedes ordinary processes |
| `logic`, `typedef`, packed structures, type parameters, patterns, `always_comb` | Rejected | Admitted within the feature limits below |
| Whole unpacked-array values, assignments and ports | Rejected; memory declarations, indexed elements and admitted memory-I/O storage arguments remain legal | Admitted fixed forms |
| Unbased-unsized literals and SV-only `for` headers | Rejected | Admitted |
| `$clog2` | Rejected | Admitted |
| Later `$countbits`, `assert final`, `$assertcontrol` forms | Rejected | Rejected; later internal paths do not override the selected edition |

Unknown system names require explicit registration. Expanded unbased-literal,
`for`-header and memory-argument edition checks still require Rust/frontend and
public-CLI qualification; complete Annex A coverage is not claimed.

`--compilation-units separate|merged` defaults to `separate`. Separate mode gives
each source its own preprocessor and `$unit` scope; merged mode shares them in
caller order while preserving file identities. Included buffers belong to their
including source. Repeated `--include-dir`/`-I` and `--define`/`-D` options are
supported.

## Resource limits

| Resource | Limit or behavior |
| --- | --- |
| Packed value / single aggregate payload | At most 1,048,575 bits; `LLG_SUPPORTED_WIDTH_LIMIT = 1 << 20` is exclusive. Packed storage uses the actual width. |
| Generated fixed unpacked array | At most 65,536 total cells (`LLG_MAX_FIXED_ARRAY_CELLS`); dimensions and products are checked before allocation. Each packed cell has the packed-value limit. |
| Fixed array used as a value, formal or stream | Must fit one packed payload. Direct rank-one reductions read cells individually and may exceed that total payload width. |
| Subroutine recursion | Guard at depth 256; diagnoses exhaustion and returns the type default. |
| Read-only helper inlining | Guard at 32 levels. |
| Scheduler/process work | Default budgets of 10,000,000; controlled by `LLG_ZERO_LOOP_LIMIT`, `LLG_PROCESS_STEP_LIMIT` and its `LLG_NONCONVERGENCE_LIMIT` alias. |
| Driver, alias, process, final, event-waiter, procedural-driver and force registries | Size to the design or grow with checked allocation; the former 16-driver and 256-alias ceilings do not apply. |

**The 65,536-cell array limit does not meet the cited 16,777,216-element minimum
in V §3.10 and SV §7.4.2.** Capacity rejection is not syntax coverage or
minimum-capacity conformance. Zero/negative size operands are language errors;
negative range labels are valid. Internal zero-width packed descriptors are empty; native
real values use a separate representation.

## 1. Lexical & preprocessing relevant to simulation

- 🟦 **Lexical base and literals** — Comments, whitespace, ordinary/escaped
  identifiers, operators, sized/unsized binary/octal/decimal/hex integers, X/Z
  digits, underscores, real/scientific and string literals. Signed literals and
  signed declarations retain their resolved meaning. V §§2.1–2.7 **[1995/2001]**.
- 🟦 **Attributes** — Parsed without simulator-specific effects, including
  `(* full_case *)`. V §2.8 **[2001]**.
- 🟦 **Macros and conditional compilation** — `` `define `` / `` `undef ``,
  arguments, token concatenation, stringification and the `` `ifdef `` family;
  `` `elsif `` / `` `ifndef `` are the 2001 additions. V §§19.3–19.4
  **[1995/2001]**.
- 🟦 **Includes** — Literal and bounded object/function-like macro-expanded names
  resolve beneath admitted source/include roots in both compilation-unit modes.
  Missing, dynamic and unauthorized paths are diagnosed. V §19.5 **[1995]**.
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
  assignments and casts convert X/Z to zero. Unsupported aggregate/net-member
  contexts remain restricted. SV §§6.9, 6.11 **[SV-2005]**.
- 🟦 **Net declarations** — `wire`/`tri` share ordinary resolved-net behavior;
  declaration assignments are continuous drivers. `uwire` rejects overlapping
  drivers and supports whole/disjoint constant-selected drivers, undriven Z,
  ordinary ports and same-type aliases without inout edges. The frontend rejects
  `uwire` inouts. Wired/pull/supply nets and strengths are detailed in §8.
  V §§3.6–3.7; SV §6.6 **[1995/SV-2005]**.
- 🟨 **Memories and fixed unpacked arrays** — Multidimensional storage, signed or
  reversed bounds, element selections, rows, slices, whole copies and overlapping
  self-assignment retain logical coordinates. Whole-array values are SV-only;
  admitted fixed integral calls/ports, patterns and operators have the limits in
  §§3, 5, 7 and 9. Native/resizable elements, general real-array expressions,
  unsupported layouts and over-limit payloads remain restricted.
  V §3.10; SV §§7.4, 7.6 **[1995/SV-2005]**.
- 🟨 **Initialization and lifetimes** — Scalar and fixed integral composite
  initializers, including zero-time calls, run before 2009 processes; 2001 keeps
  its initialization schedule. Static locals/formals initialize once, not on
  first call; automatic storage initializes per activation. Explicit member
  defaults, recursive array defaults and mixed state domains are retained.
  Timing-bearing initializer calls are illegal; unsupported native/resizable
  layouts and ambiguous/opposite-lifetime captures remain rejected.
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
  extent; unequal-width packed unions reject. General native/resizable subroutine
  layouts, declaration/net combinations and arbitrary slices remain restricted.
  SV §§7.2–7.4 **[SV-2005]**.
- 🟨 **Tagged unions** — Finite packed storage, construction and checked member
  access support void, primitive, fixed-structure and nested-tag payloads in
  module storage, static/automatic locals, value, constructor, port,
  input/output/inout, const-ref/ref and stable-tag selected NBA contexts. Valid
  reads restore the selected member's state/sign; inactive-tag reads and writes
  produce source-addressed runtime errors. Dynamic/native payloads remain
  restricted. Q03: SV §§4.9.4 and 10.4.2 fix the NBA target and RHS using
  issue-time values; a wrong tag at issue still diagnoses if the variable is
  retagged before commit. The selected successful NBAs keep the tag stable.
  Whether a valid issue target requires another tag check after an intervening
  retag remains unqualified. Pattern matching is covered in §5.
  SV §§7.3, 11.9, 12.6 **[SV-2005]**.
- 🟨 **Strings** — Module/static/automatic byte strings support copies, casts,
  core methods, `atoreal/realtoa`, formatting, value/reference formals, copy-out,
  returns and collected input/output links. Contents changes feed sensitivity;
  inputs/returns have independent ownership. Automatic string NBAs, unsupported
  captures, automatic monitors and broader aggregate/continuous combinations
  remain restricted. SV §6.16 **[SV-2005]**.
- 🟨 **Events** — Scalar/fixed-array declarations, indexed/hierarchical access,
  null/default handles, reassignment and task aliases retain event identity.
  Dynamic/associative/queue event storage is unsupported. SV §6.17 **[SV-2005]**.
- 🟨 **Dynamic arrays, associative arrays and queues** — Allocation, resize,
  delete, copy, bounded patterns, generic/nested leaves, associative defaults and
  traversal, queue slices/overflow, and collected value-port copies are present.
  Contents/shape changes notify readers. General subroutine storage, non-packed
  endpoint/pop expressions, string-key index-result queues, nested scalar queries
  and broader recursive/object forms remain restricted. Methods are in §7.
  SV §§7.5, 7.8, 7.10, 7.12 **[SV-2005]**.
- 🟨 **Chandle** — Typed native-pointer null/copy/identity/Boolean operations,
  locals, admitted aggregate/class fields, mixed signatures, returns and
  input/output/inout/ref/const-ref calls are present. Matching collected reference
  ports share storage. Value-port links, packed containment, arithmetic, general
  continuous assignment and non-string object sensitivity remain unsupported.
  SV §6.14 **[SV-2005]**.

## 3. Modules, ports, parameters, hierarchy

- 🟦 **Modules and hierarchy** — ANSI/non-ANSI headers, positional/named ports,
  instance arrays, `generate for/if/case`, `genvar`, hierarchical/upward reads and
  admitted selected blocking/NBA writes. V §§12.1–12.4 **[1995/2001]**.
- 🟦 **Parameters and constant elaboration** — Parameters, `localparam`, named/
  positional overrides and `defparam` re-elaboration propagate values and widths;
  overriding `localparam` is illegal. Finite constant functions, type-dependent
  defaults, nominal enums, `type()`, constant queries and folded real/string
  values support finite extents and names. Fixed struct/union parameters are
  readable whole or by member; override expressions retain their parent scope.
  Expanded recursion/type/namespace checks still need Rust/frontend/CLI
  qualification. Constant functions reading `defparam`-affected parameters have
  undefined results under the supplied references. V §§3.11, 10.3.5, 12.2;
  SV §§6.20, 6.23, 13.4.3 **[1995/2001/SV-2005]**.
- 🟨 **Packages and `$unit`** — Qualified/imported/re-exported names, wildcard
  exports, shared variables, dependent initialization, static subprogram state,
  types and constants are represented. Separate/merged unit scope is retained.
  Unsupported layouts, callable environments and post-2009 forms remain
  restricted. Instance-hierarchical names are not constant-expression operands;
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
  values, nested member targets and instance-array distribution. Invalid output
  expressions, chandle value links, uncollected layouts and runtime output/inout
  rewiring reject. V §12.3; SV §§7.4.6, 23.2–23.3 **[1995/SV-2005]**.
- 🟨 **Reference and inout ports** — Matching packed/fixed-array references and
  collected recursive aggregate/string/chandle leaves share storage. Admitted
  module ref-port packed projections are distinct from subroutine ref-actual
  legality (§9). Inequivalent shapes and runtime reference rewiring reject;
  dissimilar resolved-net inouts follow §8. SV §23.2.2 **[SV-2005]**.
- 🟨 **Libraries and configurations** — Explicit `--libmap`, repeatable
  `--libfile [library=]path`, `--library-order`, `--default-library`, top
  `module:config`, default `liblist`, cell/instance `use`, nested configurations
  and parameter propagation use ordinary elaboration. Included maps and
  `*`/`?`/`**` patterns expand in sorted order. Matching precedence is explicit
  filename > wildcard filename > trailing-separator directory; explicit library
  assignments override maps. Relative parent paths resolve from the map directory.
  Missing/ambiguous bindings, encrypted IP, foreign co-simulation and unrestricted
  filesystem discovery reject. Per-library `-incdir` remains unqualified.
  V ch.13; SV ch.33 **[2001]**.

  Literal configuration blocks are admitted in root/included library maps as
  well as source files. This follows the formal grammar in both editions; the
  narrower nearby V §13.2.2 / SV §33.3.2 prose is an acknowledged source
  inconsistency, not a claimed IEEE correction. Macro-generated map declarations
  or block delimiters are not admitted by this route.
- 🟨 **Structural bind** — Finite module-type/selected-instance and interface-to-interface binds preserve parameters, target-local ports and instance identity.
  Unknown/primitive targets reject; module injection into an interface is illegal.
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
- 🟨 **`always_comb` / `always_latch`** — Execute at time zero; include transitive
  function reads and exclude written expressions. Fixed arrays, record members,
  conditional arms, sequential predicates and aggregate input links contribute
  dependencies. Unchanged results do not notify downstream readers; a closed
  latch retains its value. Blocking timing and forks reject; delayed NBAs are not
  rejected merely for their delay. Broader dynamic/native aggregate and object
  contexts remain partial. SV §§9.2.2.2–9.2.2.3 **[SV-2005]**.
- 🟨 **`always_ff` and writer rules** — Requires one event control and rejects
  blocking timing, forks and extra overlapping writers. Blocking data assignments
  and delayed NBAs are legal. Data changes alone do not wake it. Disjoint array
  cells/record or packed members remain separate writers; overlapping procedural,
  positional-pattern and continuous writes are diagnosed within admitted storage.
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
  conversions are retained. Native/resizable and over-limit values remain
  restricted; automatic-variable and subroutine-reference-formal NBAs reject.
  SV §§7.6, 10.4 **[SV-2005]**.
- 🟨 **Assignment-pattern values** — Positional, member/integer-index, type and
  default keys support admitted fixed arrays/records. Explicit-index > type-key >
  default precedence, last matching type key, immediate row types and recursive
  defaults are retained; semantic duplicate indices and uncovered elements reject.
  Array-valued items supply subarray cells in declaration order and evaluate once;
  only scalar fill values broadcast. Repeated operand positions remain distinct.
  Packed structures retain their type keys; ordinary packed-vector type-key
  matching outside the described paths remains restricted. Type/default values
  do not have a qualified side-effect evaluation count.
  SV §§10.9.1–10.9.2 **[SV-2005]**.
- 🟨 **Replicated patterns** — Constant counts expand fixed integral arrays,
  nested rows and aggregates in syntactic order. Zero/negative counts, incompatible
  shapes and unsupported native/resizable values reject. No source side-effect
  evaluation count is promised for replication. SV §10.9.1 **[SV-2005]**.
- 🟨 **Positional pattern lvalues** — Plain/typed fixed arrays, packed arrays and
  packed/unpacked records deconstruct into nested/selected destinations. Capture
  the RHS and every destination coordinate before scatter writes; earlier stores
  cannot redirect later targets. Blocking and legal persistent NBA targets are
  represented; static net targets retain per-element resolution. Keyed/default/
  replicated lvalues, constants, mismatches and illegal NBA targets reject.
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
  automatic bindings extend to later clauses and the true arm. Cases keep first-match/default and unique/priority behavior. Tag/payload comparisons inherit the
  enclosing case mode; ordinary member access and `if ... matches` remain exact.
  Primitive integral constant patterns compare X/Z exactly; bindings end at the true arm.
  Whole dynamic/native wildcard and binding patterns, and dynamic/native tagged
  payloads, remain restricted. SV §12.6 **[SV-2005]**.
- 🟦 **Qualified selection** — `unique`, `unique0`, `priority` diagnose no-match/
  multiple-match with source locations and default/else suppression. `case inside`
  supports wildcard items, ranges, qualified string cases and one selector
  evaluation. SV §§12.4.2, 12.5.3–12.5.4 **[SV-2005]**.
- 🟦 **Loops and jumps** — `for`, `while`, runtime-count `repeat`, `forever`,
  `do-while`, nested `break`/`continue` and subroutine `return`. Continue executes
  the for-step list or condition back edge; break exits the innermost loop.
  For headers support multiple initializers/steps and ordinary HDL function
  steps, including copy-out/ref effects and discarded values. Tasks as steps
  reject; break/return bypass the steps. Function-step additions still need
  Rust/CLI and generated-model ownership qualification. V §9.6; SV §§12.7–12.8
  **[1995/SV-2005]**.
- 🟨 **Loop declarations and `foreach`** — Packed/real loop locals preserve
  shadowing and admitted fork captures. Fixed foreach retains mixed packed/
  unpacked dimensions, omitted slots, singleton/implicit vectors, declared/formal
  bounds and signed endpoint guards. Iterators are automatic, lexical and
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
  forms reject. Live evaluators have the restrictions in §9. V §9.3.2 **[1995]**.

## 6. Timing controls

- 🟨 **Delays and precision** — Integer, fractional, scientific, unit-suffixed,
  parameter and runtime packed/real expressions, function results and task inputs
  are evaluated once for statement/intra-assignment delays. Local precision
  rounding precedes checked 64-bit femtosecond scheduling (1fs–100s units).
  Sub-precision delays round to zero; X/Z means zero. Negative packed values
  convert to unsigned 64-bit time before scaling; negative/nonfinite reals and
  tick overflow reject. Continuous/gate delays are separately bounded in §8.
  V §9.7.1; SV §§3.14, 5.8 **[1995/SV-2005]**.
- 🟦 **Implicit sensitivity** — `@*`/`@(*)` use call-site body reads, unlike
  `always_comb`'s transitive function reads. Fixed-array elements/contents,
  container contents/shape and string markers wake admitted readers after copies,
  mutations or resizing. Non-string object and unrepresented nested dependencies
  remain restricted. V §9.7.5 **[2001]**.
- 🟨 **Evaluated events** — Packed/scalar-real any-change expressions, packed LSB
  edges, trigger-time `iff`, numeric activation captures and atomic mixed named-event lists are represented. Sensitivity follows operands and eligible helpers,
  not unrelated body reads. Real edge descriptors, unsupported qualifiers/captures
  and helper effects outside §9 reject. V §§9.7.2–9.7.4 **[1995]**.
- 🟨 **Intra-assignment controls** — Packed/real/shortreal RHS values are captured
  immediately. Blocking assignments suspend and use update-time selectors; NBAs
  continue with issue-time destinations. Explicit event/repeated-event controls
  preserve order and normalize zero/X/Z/negative repeat counts. Nested repeat
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
  resource limit. V §4.1.5 **[1995/2001]**.
- 🟦 **Bitwise, logical, reduction, shift and relational operators** — `& | ^ ~`,
  XNOR forms, `&& || !`, reduction AND/NAND/OR/NOR/XOR/XNOR, `<< >> <<< >>>`
  and `< <= > >=` retain resolved widths, signs and four-state behavior.
  Ordinary SV `->` short-circuits a known-false antecedent; `<->` evaluates both
  sides. SVA `|->`/`|=>` belongs to the assertion boundary (§12), not these operators.
  V §§4.1.7, 4.1.9–4.1.12; SV §11.4.7 **[1995/2001/SV-2009]**.
- 🟨 **Equality** — `===`/`!==` compare X/Z literally; `==`/`!=` preserve known-mismatch dominance. Admitted fixed integral aggregates compare complete
  member payloads even when only one side is storage-backed. Broader formal/
  native aggregate combinations remain partial. V §4.1.8; SV §11.4.5 **[1995]**.
- 🟨 **Conditional values** — Known truth selects one arm, including a vector
  predicate with a dominant known 1. Ambiguous truth evaluates both reached arms
  once. Packed values follow the selected published-table policy: equal 0, 1 or X
  survives; Z/Z and differing pairs become X. Real alternatives return zero.
  Fixed arrays compare immediate elements and unpacked structures compare
  immediate members: known-equal boundaries survive; differing boundaries take
  their type's uninitialized default. Nested rows/records default as whole
  members, not recursively merged leaves; explicit member initializers are not
  fallbacks. Dynamic/native aggregate merges remain restricted.
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
  `$cast` supports packed/real values, enum membership and nominal class checks
  with success-only writes. Fixed bit-stream casts are described below;
  unsupported aggregate/net/reference targets remain restricted.
  SV §6.24 **[SV-2005]**.
- 🟨 **Mutating operators** — Prefix/postfix `++/--` and `+= -= *= /= %= &= |= ^=`
  plus shift assignments capture an admitted whole/member/selected/fixed-array
  target once, perform one store and return the appropriate expression value.
  Packed state conversion and real targets are represented; non-lvalues and
  unsupported aggregates/objects reject. SV §§11.4.1–11.4.2 **[SV-2005]**.
- 🟦 **Wildcard equality** — `==?`/`!=?` treat RHS X/Z as wildcards. Remaining
  LHS unknowns yield X unless a known mismatch decides the result; common-width
  and signed extension apply. SV §11.4.6 **[SV-2005]**.
- 🟨 **Set membership** — `inside` supports scalar/range/wildcard and admitted
  real/string contexts. Fixed-array values, including calls, conditionals, casts,
  rows and members, are captured once and traversed with leaf signedness intact;
  packed dynamic/queue/associative storage has separate traversal. Packed values
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
  with declared bounds. Packed, higher and unvisited dimensions reject;
  native/string/real/resizable maps also reject. Receiver capacity is defined above.
  SV §§7.12.3–7.12.4 **[SV-2005]**.
- 🟨 **Fixed ordering** — `reverse()` moves immediate integral/fixed-record
  elements in declaration order; rows move whole. `sort()`/`rsort()` accept
  integral elements or fixed row/record `with` integral keys, preserving
  signed/enum keys and non-key fields. Selected receiver coordinates are
  captured once; swaps reread live storage and snapshot overlapping elements.
  Automatic locals/formals are represented. Reverse qualification covers
  1/2/3/17 elements, negative/reversed bounds, selected 2-D/3-D rows, unpacked
  records, input/inout/ref formals, and settled reader notifications in both
  optimizer modes; SV2001 rejects the method. Integral `with` keys may query the
  live iterator's declared index on ascending, descending and row arrays.
  Repeated-key stability is unspecified. Reverse `with`,
  const-ref receivers, record sorting without a key, fixed shuffle/locators,
  native/real/string elements and incompatible maps are outside this fixed
  integral profile and reject.
  SV §7.12.2 **[SV-2009]**.
- 🟨 **Resizable-container methods** — Packed reductions/`with` callbacks,
  locators, min/max/unique result queues, sort/rsort/reverse/shuffle are present.
  Callbacks require packed items and cannot capture automatic locals/formals;
  generic leaf storage does not remove method/result limits. Shuffle uses its
  container seed API, not full process/object RNG integration.
  SV §7.12 **[SV-2005]**.
- 🟨 **Bit-stream casts and streaming** — Fixed arrays/nested records, selected
  rows/members, call results and admitted ref/const-ref projections preserve
  state conversion and non-dividing/type slice sizes. Packed and bounded
  dynamic/queue-element streams capture one RHS, then publish destinations in
  stream order with overlap-safe snapshots. A stream assigned to a wider fixed
  target is left-aligned and zero-filled on the right; oversize streams reject.
  Unpacking consumes the leftmost required bits; undersized sources reject.
  `with` follows slice/declaration order, including descending arrays and `-:`
  ranges, and requires a one-dimensional operand. At most one resizable
  destination is allowed. Fixed-size cast mismatches, real/associative operands,
  native strings, recursive objects and unsupported reference combinations reject.
  SV §§6.24.3, 11.4.14 **[SV-2005]**.
- 🟨 **Let expressions** — Numeric expansions bind free names in declaration
  scope; recursive lets reject. Expanded assertion/native/aggregate bodies still
  need an admitted consumer representation. Operator-overloading declarations
  have no separately qualified simulation path; ordinary operator support does not
  establish overload support. SV §§11.11, 11.13 **[SV-2009]**.

## 8. Continuous assignments & structural

- 🟦 **Continuous drivers** — Explicit, comma-form and net-declaration
  assignments evaluate from RHS dependencies; constant RHSs run once.
  V §6.1 **[1995]**.
- 🟨 **Fixed-array continuous assignments** — Whole/selected fixed values,
  slices, conditionals, patterns and function returns capture one RHS per
  activation. Net arrays retain per-cell/per-bit contribution slots; static
  positional net patterns drive only named elements. Variable storage permits
  one overlapping continuous writer and separate disjoint cells. Overlapping
  ordinary procedural writes or declaration initializers reject; force/release/
  deassign are not ordinary writers for this check. Dynamic topology, net-array
  declaration delays and unsupported aggregate shapes remain restricted.
  SV §§6.5–6.7, 7.6, 10.3 **[SV-2005]**.
- 🟨 **Resolution and strengths** — Continuous, gate, port and admitted
  hierarchical/fixed-array drivers retain independent contributions. `wire/tri`,
  `wand/triand` and `wor/trior` use their resolution rules; Z is neutral and
  equal-strength wired ties use AND/OR. Supply/strong/pull/weak/high-Z endpoints
  and X uncertainty are retained. `tri0/tri1` pull only all-Z bits; supplies
  dominate ordinary drivers. Changed resolution wakes readers, unchanged results
  do not. Explicit vector continuous strengths are prohibited; general aggregate,
  function/task-output driver and charge/resistive contexts remain restricted.
  V §§3.4, 3.7, 6.1, 7.9–7.13; SV §10.3.4 **[1995]**.
- 🟨 **Dissimilar inout collapse** — Admitted whole/selected/concatenated packed
  and fixed net-array-cell connections use the directional port-type table. The
  winning declaration supplies resolver, default and propagation delay, including
  no delay. Parent links precede descendants; same-depth warning-only ties use
  stable design order and produce located warnings. This is not an order-independent guarantee for every multiway conflict. `uwire` inouts, `trireg`,
  new whole-array shapes and net-array declaration delays remain unsupported.
  V §12.3.10 / Table 45; SV §23.3.3.7 / Table 23-1 **[1995]**.
- 🟨 **True aliases** — Same-declared-net-type packed networks, fully indexed
  fixed net-array elements and multidimensional/static indexed projections share
  force/release, dependency and waveform identity. Ascending declarations with
  `-:` selections preserve physical bit pairing. Dissimilar port rules do not
  relax alias type rules. Self/duplicate/variable/incompatible aliases, dynamic
  or forbidden cross-scope forms and unmapped writes reject. The pinned frontend
  rejects direct packed-struct member selects in `alias`; admitted part-select
  projections remain available. SV §10.11 **[SV-2005]**.
- 🟨 **Continuous, gate and net delays** — Constant single/rise-fall/turn-off
  delays schedule captured Active-region inertial updates for admitted whole,
  constant-selected and fixed-array-element drivers. Changed pending results
  cancel; unchanged results retain deadlines; returning to the current value
  cancels without replacement. Net propagation delays apply after resolution.
  Conflicting selected true-alias delays, dynamic driver-delay expressions,
  net-array declaration delays and unsupported terminals remain restricted.
  V §§6.1.3, 7.14 **[1995]**.
- 🟦 **Logic and pull gates** — `and/nand/or/nor/xor/xnor/buf/not/pullup/pulldown`
  use typed terminals and independent output drivers. Multi-output buf/not,
  input expressions/constants, selected/hierarchical references, mixed-width
  normalization and built-in instance arrays are represented; no blanket equal-width or 64-terminal limit applies. Outputs need an admitted packed structural
  target; real/invalid terminals and incompatible driver contexts reject.
  V §§7.1–7.3, 7.8 **[1995]**.
- 🟦 **Tri-state gates** — `bufif0/1` and `notif0/1` normalize enabled data-Z to
  X, drive Z when disabled and retain the gate's unknown-enable truth behavior.
  V §7.4 **[1995]**.
- 🟨 **Combinational UDPs** — Scalar tables/instances and primitive arrays admit
  `0/1/x/b/?`, treat input Z as X, return X for unmatched combinations and reject
  conflicting overlapping rows. Independent drivers, optional legal strengths
  and delays are represented. Vector/aggregate terminals and sequential
  level/edge/state-initialization semantics remain unsupported.
  V §§8.1–8.2, 8.6; SV §§29.3–29.4, 29.8 **[1995/SV-2009]**.
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
  copy-out retain declaration order, state and lifetime. Numeric and admitted
  native string/chandle signatures have separate paths. Static outputs retain
  formal storage; only inout copy-in overwrites it, while automatic outputs get
  typed defaults. General native/resizable aggregates remain restricted.
  V §§10.2–10.3; SV §§13.3–13.5 **[1995/2001/SV-2005]**.
- 🟦 **Automatic/reentrant and finite zero-time calls** — Per-activation
  storage, finite recursion, local named-block exits, selected copy-out and
  nested const-ref forwarding are represented. Static storage is per instance.
  Recursion is capacity-limited as stated above. V §§10.2.3, 10.3.1;
  SV §§6.21–6.22, 13.3–13.5 **[2001/SV-2005]**.
- 🟨 **References** — `ref`/`const ref` alias matching packed variables, fixed
  integral arrays/records/unions, admitted unpacked members/elements and retained
  packed queue cells. Removal/reallocation preserves a queue reference's original
  detached cell. String/chandle references use native storage. General native/
  resizable aggregates, non-packed queue references, reference-formal NBAs and
  selected-reference file targets remain restricted. Subroutine actuals must be
  eligible variables, not function/reduction/conditional/cast/pattern temporaries;
  packed bit/part actuals rejected by the frontend are not legalized by internal
  projections. SV §13.5.2 **[SV-2005]**.
- 🟨 **Timed tasks and parallel bodies** — Admitted delay-only tasks suspend;
  event/cancellation-sensitive calls preserve values/selectors and check
  cancellation before copy-out. Synchronous joined numeric branches can share
  the suspended activation; join_any/join_none retain supported snapshots of
  automatic packed/real values and recognized handles. This does not provide
  arbitrary detached alias propagation or ref-formal captures. Static task NBAs
  are represented; automatic-subroutine NBAs reject. V §§9.8.2, 10.2;
  SV §9.3.2 **[1995/SV-2005]**.
- 🟨 **Hierarchical calls and special formals** — Resolved module/interface/
  package/parent callees retain their owning instance. Event formals use inline
  task paths, not numeric value calls; event-formal virtual dispatch and process-handle formal ABI remain unsupported. Unresolved environments and broader
  timing/native/aggregate combinations reject. V §12.4 **[1995]**.
- 🟨 **Read-only helper calls** — Event, continuous, force and other read-only
  evaluators admit bounded numeric value/const-ref helpers with private locals,
  loops, nested eligible calls and fixed integral reductions. Private
  concatenation/positional-pattern stores and compound mutations are allowed
  only when every destination is private. Stateless static formals use private
  callback copies; result independence must hold across normal/return/loop exits.
  Persistent non-return state, stateful reads, visible writes, unproved control
  transfers, native/DPI dispatch, suspension and arbitrary shared/native captures
  reject. Unique/priority diagnostics remain active; side-effect-free source
  alone does not establish eligibility.
- ❌ **Unsupported or illegal call forms** — Recursive delay/wait-bearing tasks,
  timing-bearing class/virtual-interface tasks and jumps into other lexical
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
  strengths, real and string values. Packed `%s` uses ASCII bytes without leading
  zero bytes; real `%s` rejects. `%p` is limited to scalar packed/string values,
  not aggregates. V §17.1.1.2; SV §21.2 **[1995/SV-2005]**.
- 🟨 **Postponed output** — Strobe observes settled values after Active/Inactive/
  NBA iteration. One active monitor coalesces same-slot changes; registration and
  re-enable queue one report. Packed, real and string changes are supported;
  automatic monitor dependencies and aggregate/container formatting reject.
  Read-only helper restrictions apply. V §§17.1.2–17.1.3 **[1995]**.
- 🟦 **String formatting** — `$sformat/$swrite` and radix variants write native
  strings or packed string-like destinations with normal padding/truncation.
  `$sformatf` returns an owned string. Dynamic/nested formats and arguments
  evaluate once in source order. V §17.2.3; SV §21.3.3 **[2001/SV-2005]**.

### Input and memory files

- 🟨 **Scanning/character/line input** — `$fscanf/$sscanf/$fgets/$fgetc/$ungetc`
  retain input/format values, admitted packed/selected/string/real destinations,
  EOF, byte and X/Z behavior. Selected sub-accesses through ref formals and
  general aggregate targets reject. V §17.2.4; SV §21.3.4 **[2001/SV-2005]**.
- 🟨 **Binary input** — `$fread` supports admitted packed/memory targets with
  bounded start/count. Rank-one memories advance from lowest to highest HDL
  address in either declaration direction. Packed reads accept and ignore
  start/count; the exact memory-storage argument is legal in 2001 without
  admitting unrelated whole-array value uses. Packed-bound/public-edition
  additions still need Rust/CLI qualification. V §17.2.4.4; SV §21.3.4.4
  **[2001/SV-2005]**.
- 🟦 **File position/status** — `$ftell/$fseek/$rewind/$fflush/$ferror/$feof`
  preserve EOF/error and invalid/closed-descriptor status. V §§17.2.5–17.2.7;
  SV §§21.3.5–21.3.8 **[2001/SV-2005]**.
- 🟦 **Memory images** — `$readmemh/$readmemb` accept owned string paths,
  whitespace/comments, binary/hex digits, X/Z and `@` addresses.
  Fixed packed memories may have multiple unpacked dimensions and selected
  rows/slices; runtime view selectors are captured once. Remaining dimensions
  walk low-to-high in row-major order; one-dimensional 2001 omitted ranges
  instead follow declaration direction. 2009 defaults, including start-only
  calls, are low-to-high; explicit ranges retain requested direction.
  `@` addresses the highest remaining dimension. Invalid selected ranges leave
  every cell unchanged; invalid later jumps stop while retaining earlier writes.
  Incomplete rows leave unread subwords unchanged.
  Public SV2009 coverage includes mixed-direction 3-D declarations, a runtime
  selected slice with explicit descending bounds, a 129-bit selected word, and
  settled per-leaf reader notification after a selected load. Same-time-slot
  wakeup order is unqualified.
  V §17.2.8; SV §21.4 **[1995/SV-2005]**.

  Short words pad a leading X/Z with that state; known digits zero-extend.
  Ordinary packed memories truncate normally. Enum words are checked at original
  width, allowing redundant signed extension; invalid words diagnose and stop
  without undoing earlier writes. Two-state X/Z conversion occurs before enum
  range checking, without discarding high bits. Sparse addressed 2009 files omit
  the no-address short-file warning; 2001 count warnings remain, including excess
  data from repeated in-range addresses. Q02's additional diagnostics and short
  X/Z token-extension intersections, including selected views, remain unqualified.
  General malformed-token and four-state enum numeric policies are not fully
  qualified. Dynamic/queue/associative,
  real/non-packed memories and fully indexed scalar memory-element targets reject.
- 🟦 **Memory writers** — `$writememh/$writememb` emit consumable four-state words
  from admitted fixed-memory views in low-to-high row-major order. The same
  element/view restrictions apply; writers reject in 2001 mode.
  SV §21.5 **[SV-2005]**.

### Time, control and utility services

- 🟦 **Time reporting** — `$time` rounds to the calling module's unit (exact
  halves upward); `$stime` then returns its low 32 bits; `$realtime` retains
  fractions. `$printtimescale`, design-wide `$timeformat` units/precision/suffix/
  minimum width and `%t` integral/real conversions are represented.
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
  string metadata. Nested runtime dimensions and string/wildcard associative
  bounds remain unsupported. SV §§20.6–20.7 **[SV-2005]**.
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
- ❌ **PLA tasks** — All synchronous/asynchronous AND/NAND/OR/NOR array/plane
  families (such as `$async$and$array`) lack supported simulator lowering. Name admission or a generic VPI
  fallback does not implement PLA semantics. V §17.5; SV §20.17 **[1995]**.
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
  Unavailable include paths fail admission. V §§19.3–19.5; SV §§22.4–22.6,
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
- 🟨 **`` `unconnected_drive `` / `` `nounconnected_drive ``** — Omitted scalar/
  packed inputs receive pull0/pull1/Z. Strength conflicts and aggregate/resizable
  formals remain outside the qualified boundary. V §19.9 **[1995]**.
- 🟨 **Source mapping** — `` `line ``, `` `__FILE__ `` and `` `__LINE__ `` expose
  mapped values in 2009; 2001 admits `` `line `` without the later predefined
  macros. Diagnostics retain physical source ranges.
  V §19.7; SV §§22.12–22.13 **[2001/SV-2009]**.
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
  Dynamic skews, unsupported cross-clock forms and concatenated clockvar lvalues
  rejected by the frontend remain excluded. SV ch.14 **[SV-2005]**.
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

🟨 **Immediate/deferred assertions** evaluate `assert/assume/cover` once with
four-state truth. Deferred `#0` forms retain issue-time values, legal action-time
references, same-slot glitch coalescing and Reactive actions. Module-level members
are represented. SV §§16.3–16.4 **[SV-2009]**.

🟨 **Concurrent assertions** support the following bounded forms (SV ch.16, Annex F):

| Area | Implemented forms |
| --- | --- |
| Attempts and actions | Preponed packed sampling, Observed resolution, overlapping attempts, vacuity accounting and Reactive actions. |
| Clock/disable flow | Nearest default-clock inheritance, compatible declaration/call-site clocks, legal multiclock `##0`/`##1` boundaries, asynchronous single-signal `disable iff`, bounded `accept_on/reject_on` and synchronous variants. |
| Sequences | `##` concatenation/ranges; consecutive/nonconsecutive/goto repetition with unbounded endpoints; `or`; direct one-cycle `and/intersect/throughout/within`; `first_match`. |
| Properties and instances | One-cycle `not/and/or/iff/implies` and conditional forms; named sequence/property instances with positional/named/default arguments. |
| Locals and match items | Per-attempt local input capture/defaults, ordered assignment/increment/subroutine-call items and isolated local snapshots for overlapping/branching threads. |
| Control | Bounded blocking `expect`, sequence `.matched`, `$asserton/$assertoff/$assertkill` and hierarchy selectors. Internal post-2009 level-0 ON/OFF/KILL `$assertcontrol` support is not admitted by either target edition. |

Output/inout/ref formal copy-out, delayed/nested local-formal calls, selected-local
lvalues, repeated match-item bodies, unsupported expanded bodies, conflicting
clock/disable metadata, other temporal/cross-clock combinators, pass/fail/vacuity
action controls, `.triggered` and invalid scopes/arguments remain rejected.
Match-item callees and shared/native captures must also satisfy §9's read-only
rules; graph/lowering support alone is not executable acceptance.

🟨 **Sampled functions** — `$sampled/$rose/$fell/$stable/$changed/$past` and
2009 global-clock history/status forms support packed explicit/default edge
domains, gated/initial history, Preponed reads and LSB/X/Z edge rules. Future global
forms, complex clocks and real-valued sampling remain rejected. SV §§16.9.3–16.9.4,
20.13 **[SV-2005]**.

### Foreign interfaces and missing infrastructure

- 🟨 **DPI-C imports** — Bounded scalar bit/logic/reg, integral, real/shortreal,
  chandle/string signatures and explicit libraries retain owned outputs and
  borrowed inputs. Foreign string results are copied before aliased copy-out,
  which checks cancellation. Exports, packed/open arrays, ref/event formals and
  context callbacks are unsupported. SV ch.35 **[SV-2005]**.
- 🟨 **PLI/VPI bridge** — Generated metadata, registration,
  `compiletf/sizetf/calltf`, scalar/vector/real values, hierarchy lookup/iteration,
  plugin startup and start/end callbacks have paths; arguments use registered
  snapshots. Full `tf_/acc_/vpi_`, arbitrary properties/iterators/regions/control
  and VPI calls from read-only evaluators remain unsupported.
  V ch.20–27 **[1995/2001]**.
- ⬜ **Checkers, functional coverage and constrained/structured randomization** —
  Checker execution, covergroups/coverpoints/bins/crosses, coverage sampling/
  queries/control/database tasks, `rand/randc`, constraints, object or
  `std::randomize`, `randcase` and `randsequence` are not implemented.
  SV **[SV-2005/SV-2009]**.
