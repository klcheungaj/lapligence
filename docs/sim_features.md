# Simulator Feature Checklist — IEEE 1364 / IEEE 1800

The authoritative list of simulation features defined by IEEE Std 1364 (Verilog)
and IEEE Std 1800 (SystemVerilog) as they relate to the `llg` simulator,
each tagged with the revision that introduced it and ticked according to what
the simulator supports today. When a feature lands **and is tested**, flip its
marker here; do not tick from implementation alone. Current implementation priority is correctness gaps in partial features, then
smaller high-priority missing capabilities, followed by difficult features.
This file is the sole maintained simulator feature-status document. Historical
audits and session handoffs belong in local `persistence/`.

Statuses are audited against the simulator lowering, IR, runtime, and emitter,
`src/bin/llg.rs`, the regression suites in `tests/sim_*.rs`, and one-off
end-to-end runs of `llg` (marked **(probed)** below). Section numbers follow the
[Verilog reference](specification/spec-reference-verilog.md) (§1364-2001 x.y),
[SystemVerilog reference](specification/spec-reference-sv.md) and
[verification reference](specification/spec-reference-verification.md) (§1800-2009 y.z).

## Status markers

- ✅ supported — test anchor in parens; `(probed)` = verified by a manual
  `llg` run during an audit, no dedicated regression test yet; no
  parenthetical = **(suite-covered)**: exercised by one of the named suites
  during audits, without a single dedicated test
- 🟨 partial / approximated — one-line bounded-support caveat
- ❌ not supported — rejected or unhandled (reason cited)
- ⬜ out of scope — verification-infrastructure tier, tracked but de-prioritized
- Revision tags: **[1995]** = IEEE 1364-1995 baseline · **[2001]** = added by
  1364-2001 · **[SV-2005]** = 1800-2005 SystemVerilog baseline ·
  **[SV-2009]** = added by 1800-2009 · **[1364-2005]** = IEEE 1364-2005
  interim revision, between our bands; used only as an inline annotation

Numeric limits: packed-vector capacity is selected per generated model;
`LLG_MODEL_MAX_WIDTH` is an exclusive-backend capacity below `1 << 20` bits,
and runtime widths are `uint32_t`. Division/modulo/power and packed/real
conversions use the model-sized limb capacity; the IR has no separate 1024/64
bit semantic cap. Function/task recursion depth ≤ 256;
zero-time scheduler and per-process loop guards default to 10M steps and are
configurable with `LLG_ZERO_LOOP_LIMIT` and `LLG_PROCESS_STEP_LIMIT` (the
`LLG_NONCONVERGENCE_LIMIT` alias is also accepted);
resolved net drivers ≤ 16 per group;
force slots ≤ 64; processes ≤ 4096; named-event waiters ≤ 64 per event;
final blocks ≤ 1024.

## Coverage and remaining work

- Supported markers describe the bounded behavior on each row, not unrestricted IEEE conformance.
- The [remaining-work inventory](#remaining-work-inventory) groups known missing forms and correctness gaps; it is not an IEEE-defined feature count.
- Testing methods, limits and commands are in [tests/readme.md](../tests/readme.md).

## Target language editions

`llg` selects one compilation-wide language policy with `--edition 2001` or
`--edition 2009`. The default is SystemVerilog-2009. The native frontend keeps
that policy separate from lexical `` `begin_keywords ``/`` `end_keywords ``
regions: those directives may select a local keyword table, but cannot change
the owned model's edition or the simulator's edition-sensitive rules.

The 2001/2009 policy currently covers frontend parsing and the simulator's
time-literal boundary. The tested ledger is intentionally narrow: under
`--edition 2001`, SV-only syntax is rejected by the global policy; a
`` `begin_keywords "1800-2009" `` region changes lexical keywords only and
does not change the owned edition; under `--edition 2009`, local
`timeunit`/`timeprecision` and signed/sub-femtosecond unit-suffixed literals
use exact femtosecond boundary rounding; scientific delay spellings are covered
separately. Initialization rules, accepted
type/operator families, function restrictions, and legacy compatibility APIs
remain separately tracked in the feature rows below; selecting an edition does
not claim that those later boundaries are complete. The pinned Slang frontend
exposes SystemVerilog semantic checks starting at its 2017 API level, so the
2009 policy uses its exact 2009 keyword table and Lapligence's 2009 time rules
while those later semantic boundaries remain an explicit inventory item.

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

- ✅ **Integer literals** `[size]'base value`, x/z digits, `_` separators `` `4'b1001` `` — §1364-2001 2.5.1 **[1995]**
- ✅ **Signed literals** `` `-8'd6`, `4'shf` `` — §1364-2001 2.5.1 **[2001]**
- ✅ **Real literals** `1.2`, `2.5e10` — §1364-2001 2.5.2 **[1995]**
- ✅ **String literals** `"..."` as display format strings — §1364-2001 2.6 **[1995]** (SystemVerilog `string` is a separate partial type, see §2)
- ✅ **Lexical base** comments, identifiers, escaped identifiers, operators — §1364-2001 2.1–2.7 **[1995]** via the Slang frontend
- ✅ **Attributes** `(* full_case *)` parsed+ignored — §1364-2001 2.8 **[2001]** consumed by frontend, no sim effect
- ✅ **`` `define ``/`` `undef `` macros incl. arguments** — §1364-2001 19.3 **[1995]** expanded pre-elaboration
- ✅ **`` `ifdef `` family incl. `` `elsif ``/`` `ifndef ``** — §1364-2001 19.4 **[1995]** (`elsif`/`ifndef` are [2001])
- ✅ **`` `include ``** — §1364-2001 19.5 **[1995]** literal and bounded object/function-like macro-expanded filenames are admitted through canonical source/include roots in both compilation-unit modes; missing, dynamic, and unauthorized paths remain frontend diagnostics
- ✅ **`` `timescale `` honored** scales `#N` and rounded integer `$time`/`$stime` with Slang's resolved owning-module time unit and precision — §1364-2001 19.8 **[1995]** compilation-unit/declaration inheritance is resolved by the frontend; scheduler ticks use checked femtoseconds across 1fs through 100s (sim_physical_time.rs and sim_timescale.rs)

SystemVerilog era:

- ✅ **Fill literals** `'0/'1/'x/'z` — §1800-2009 5.7.1 **[SV-2005]** context sizing in supported packed arithmetic/bitwise expressions, comparisons, conditional branches, assignments, function arguments, and case/casez/casex; self-determined concatenation/replication operands remain one bit (sim_fill_literals.rs, optimization on/off)
- ✅ **Time literals** `2.1ns` — §1800-2009 5.8 **[SV-2005]** the owned semantic model retains Slang's typed unit and source provenance. In the default 2009 policy, ordinary expression use rounds the scope-scaled value to the owning precision (including arithmetic, parameters, and initializers); a complete procedural delay expression is converted and rounded separately once at its delay boundary. Local `timeunit`/`timeprecision`, signed/sub-femtosecond unit suffixes, exact femtosecond boundary values, and scientific delay spellings are covered; runtime scheduling uses checked 64-bit femtosecond ticks, including 1fs precision

## 2. Data types

The [testing methodology and coverage](../tests/readme.md) describe the
tested type combinations, truth tables, conversions, and width boundaries.

Verilog era:

- ✅ **reg scalars/vectors** `reg [7:0] r;` — §1364-2001 3.2.2/3.3 **[1995]** (sim_counter.rs)
- ✅ **signed net/reg declarations** `reg signed [3:0] s;` — §1364-2001 3.3/3.8 **[2001]** `%d` prints two's complement
- ✅ **integer variables** — §1364-2001 3.9 **[1995]**
- ✅ **time variables** 64-bit unsigned storage — §1364-2001 3.9 **[1995]** (sim_counter.rs)
- ✅ **wire/tri nets** — §1364-2001 3.7 **[1995]** tri resolution inside inout net groups and ordinary per-continuous-assignment driver groups; plain tri behaves like wire (sim_inout.rs, sim_net_resolution.rs)
- 🟨 **memories/unpacked arrays N-D** element bit/part/indexed-part selects with declared packed ranges, two-state assignment conversion, guarded invalid indices and masked delayed NBAs — §1364-2001 3.10 **[1995]** (sim_memory.rs, sim_partial_features.rs); whole-array procedural assignment, slices and general element types remain unsupported
- ✅ **Net declaration assignment** `wire w = expr;` — §1364-2001 3.6 **[1995]** behaves as a continuous driver for constant and dynamic RHS expressions, using the same event-driven run-once/sensitivity-loop IR as an explicit `assign` (sim_net_decl.rs); fixed-array and resizable-container reads use stable dependency markers, while unsupported resolved-net classes remain rejected explicitly
- 🟨 **Variable declaration initializers and procedural lifetimes** scalar `reg x = 0;`, `logic l = 1'b0;`, `int x = P+1;` — §1364-2001 6.2.1 / §1800-2009 6.8, 6.21, 10.5 **[2001/SV-2005]** owned declaration identity and resolved static/automatic lifetime are preserved. Scalar runtime initializers execute before processes in the selected 2009 policy and may race in explicitly selected Verilog-2001; static block/subprogram locals initialize once, automatic locals initialize on each activation, including mixed recursive routines, timed tasks, and per-instance storage (sim_varinit.rs, sim_geninit.rs, sim_variable_lifetime.rs, sim_edition.rs). Recursive aggregate and general subprogram initializers remain outside this bounded claim.
- ✅ **Parameters** override + propagation — §1364-2001 3.11.1 **[1995]** (elab_resolve.rs)
- ✅ **localparam** — §1364-2001 3.11.2 **[2001]**
- 🟨 **real/realtime/shortreal** — §1364-2001 3.9 **[1995]** scalar and fixed-unpacked-array procedural storage and parameters, scalar real/shortreal ports, real-returning value-formal subroutines, continuous assignments, mixed arithmetic, ordinary real `case`, combinational reads, `wait` conditions, any-change event controls, and retained captures of automatic real locals in detached forks are covered; aggregate/container/reference-real subprogram storage and monitor/strobe arguments remain outside the bounded subset (sim_real.rs, sim_partial_features.rs)
- 🟨 **wand/wor/triand/trior wired resolution** — §1364-2001 3.7 **[1995]** standalone packed nets and bounded same-kind collapsed inout groups resolve per-site drivers with ordered strength endpoints, including scalar continuous, gate and port-derived sources. Z is neutral, stronger endpoints dominate, and equal-strength ties use the wired-AND/OR rule (sim_net_resolution.rs, runtime_values.rs). Selected delayed continuous drivers retain their independent Z-backed slots (sim_inout.rs). Interface/array nets, hierarchical/procedural writes and function/task-output drivers remain rejected; explicit vector continuous-assignment strengths remain illegal.
- 🟨 **tri0/tri1/trireg/supply0/supply1 pull semantics** — §1364-2001 3.7 **[1995]** standalone `tri0/tri1` apply implicit pulls only to all-Z bits after ordinary-driver resolution; `supply0/supply1` dominate ordinary drivers, with correct initial defaults (sim_net_defaults.rs, runtime_values.rs); same bounded standalone-driver restrictions as wired nets; `trireg` charge storage and resistive propagation remain unsupported
- 🟨 **drive strength / charge strength** — §1364-2001 3.4 **[1995]** scalar continuous, gate, wired and collapsed-inout drivers preserve ordered supply/strong/pull/weak/high-Z endpoints, with implicit tri and supply defaults; explicit vector continuous-assignment strengths remain prohibited by §10.3.4, and `trireg` charge storage/resistive propagation remain unsupported
- ❌ **specparam** — §1364-2001 3.11.3 **[1995]** specify blocks unsupported
- ❌ **vectored/scalared hints** — §1364-2001 3.3 **[1995]** no dedicated handling

SystemVerilog era:

- 🟨 **logic/bit vectors** — §1800-2009 6.9/6.11 **[SV-2005]** `logic` has 4-state storage; `bit`/2-state vectors coerce X/Z to zero on assignments and casts. Scalar/vector paths and the tested packed-aggregate paths are covered; unsupported aggregate/net member contexts remain outside this claim
- 🟨 **byte/shortint/int/longint** 2-state ints — §1800-2009 6.11 **[SV-2005]** widths/signedness and X/Z-to-zero coercion are implemented for scalar/vector paths and the tested packed aggregates; unsupported net/member contexts remain outside this row's claim
- ✅ **uwire nets** — §1800-2009 6.6.2 **[SV-2005]** checked compilation rejects overlapping drivers; standalone whole/declaration and disjoint constant-selected drivers preserve four-state values and undriven Z bits. Driver release, generate scopes, ordinary ports/interface defaults, and undriven net arrays are tested in both optimizer modes (`sim_type_conformance.rs`); the ordinary-net context restrictions still apply
- ✅ **typedef simple/packed-vector aliases** — §1800-2009 6.18 **[SV-2005]** resolved by frontend (probed)
- ✅ **Array declaration initializers** `'{…}` patterns applied element-wise in linear-index order — §1800-2009 10.9.1 **[SV-2005]** constant elements only (sim_memory.rs)
- ✅ **enum-typed scalar variables and runtime methods** — §1800-2009 6.19/6.19.5 **[SV-2005]** stored at the elaborated packed base width; enum constants fold through the frontend, and declaration-order first/last/next/prev/num/name methods retain sparse signed values, wrapping step counts, owned names, and the four-/two-state invalid-value defaults (sim_operator_semantics.rs, sim_data_types_next.rs)
- 🟨 **packed/unpacked struct and union aggregates** — §1800-2009 7.2–7.3, 7.4 **[SV-2005]** packed/unpacked struct assignment patterns support positional, named, default, built-in, and packed-integral typedef keys with exact range/state/signedness matching and member overrides. Packed unions support direct width-matched initialization, overlapping views, and nested multidimensional member reads/writes with ascending/descending ranges across wide vectors. Fixed nested unpacked structs/arrays with packed, real, and string leaves support recursive initialization, deep value copy, member updates, and unequal-width untagged-union storage (`sim_data_types_completion`). Recursive defaults, resizable/object members, aggregate ports/nets/subprogram storage, tagged unions, and general aggregate slices remain unsupported; the bounded `sim_data_types_next` and `sim_data_types_completion` inventories are not an exhaustive conformance claim
- 🟨 **string type/signals/params** — §1800-2009 6.16 **[SV-2005]** focused basic declaration, cast/copy, display-extra, automatic string-return-with-packed-input, and 128/512-bit `.atoreal`/`.realtoa` conversion cases are covered in both modes; string formals are not supported. Module/generate storage and other core methods remain bounded; string subroutine forms, ports, continuous-assignment/sensitivity paths, and unverified formatted/real methods remain outside this claim
- 🟨 **event data type** scalar and fixed unpacked `event` declarations, runtime-indexed selects, hierarchical references, null/default handles, and task-formal aliases preserve stable event-object identity — §1800-2009 6.17 **[SV-2005]**; dynamic/associative/queue event storage remains outside the bounded subset
- 🟨 **dynamic arrays / associative arrays / queues** — §1800-2009 7.5/7.8/7.10/7.12 **[SV-2005]** bounded dynamic-array and queue cases cover packed-element reductions (including width-changing `with` expressions with typed `item`/`item.index()` bindings), locator/min/max/unique result queues, sort/rsort mutation with typed ordering callbacks, and reverse/shuffle mutation; packed associative arrays also support locator/min/max/unique result queues (integral-key index results) and reductions with `with` clauses. Unsupported generic/nested/object elements, associative string-key index result queues, declaration initializers, keyed/default patterns, subroutine/port storage, and external automatic captures in callbacks remain explicitly rejected. Shuffle uses the shared deterministic RNG service through its existing container seed API; it is not yet tied to process/object stream hierarchy
- 🟨 **chandle** — §1800-2009 6.14 **[SV-2005]** focused native-chandle storage and call cases pass in both modes: null/default and identity-preserving copy, compare/Boolean operations, automatic/static locals, represented aggregate members, mixed packed-plus-chandle signatures, input/output/inout/ref/const-ref aliases, delayed tasks, and chandle-input→chandle-return functions. Ports, packed containment, arithmetic, continuous assignment, and sensitivity remain unsupported

## 3. Modules, ports, parameters, hierarchy

Verilog era:

- ✅ **Module definitions/instantiation** positional + named `.p(e)` connections — §1364-2001 12.1.2/12.3.5–6 **[1995]** (sim_hier.rs)
- ✅ **ANSI port headers** `module m(input a, output [3:0] b);` — §1364-2001 12.3.2–12.3.3 **[2001]** (sim_counter.rs)
- ✅ **Non-ANSI port headers** — §1364-2001 12.3.4 **[1995]**
- ✅ **Parameter override** `m #(.N(8)) u (...)` — §1364-2001 12.2.2 **[1995]**
- ✅ **generate for/if/case + genvar** processes & instances inside gen scopes — §1364-2001 12.1.3 **[2001]** (sim_geninit.rs)
- ✅ **Module instance arrays** `sub u[1:0](e);` elaborated per index — §1800-2009 23.3.2 **[1995]** (probed)
- ✅ **Hierarchical name reads** `top.u.sig` in expressions/display args — §1364-2001 12.4 **[1995]** (sim_hier.rs)
- ✅ **Upward name references** partially-qualified paths (`mid.sig` from an inner scope, up-then-down included) resolve through the elaborated ref binding — §1364-2001 12.4 **[1995]** (probed)
- ✅ **Hierarchical select reads & writes** — §1364-2001 12.4 **[1995]** typed bit/part-select expressions preserve blocking and nonblocking targets, including variable bit indices (sim_hier.rs)
- ❌ **defparam** — §1364-2001 12.2.1 **[1995]** has no dedicated owned semantic mapping yet
- ⬜ **Configurations/libraries** — §1364-2001 ch13 **[2001]** out of scope

SystemVerilog era:

- ✅ **Interfaces + modports** actuals and member references bind directly to concrete interface storage; interface-body processes emit on the actual instance — §1800-2009 25.3/25.5 **[SV-2005]** (sim_interface.rs, sim_interface_body.rs)
- 🟨 **Packages** params/types via frontend folding — §1800-2009 26 **[SV-2005]** package subprograms not lowered ("return type has no width", probed)
- ✅ **`.name` / `.*` connection shorthands** — §1800-2009 23.3.2.3–4 **[SV-2005]** expanded by the frontend and preserved through port-link lowering (sim_hier.rs)
- 🟨 **ref ports / default port values** — §1800-2009 23.2.2.2/23.2.2.4 **[SV-2005]** matching packed-variable references, legal packed selections, fixed arrays, recursive aggregate leaves and string/chandle objects share storage through nested instances; constants, expressions and omitted defaults remain value-port behavior. Resizable containers and ordinary aggregate copy ports remain unsupported (sim_partial_features.rs, both optimizer modes)
- ⬜ **extern / nested modules** — §1800-2009 23.4–23.5 **[SV-2005]** out of scope

## 4. Scheduling & processes

Verilog era:

- ✅ **Stratified regions** active / inactive(`#0`) / NBA / re-eval loop until quiescent — §1364-2001 5.3–5.4 **[1995]** pinned by region_conformance.rs
- ✅ **initial** runs once then exits — §1364-2001 9.9.1 **[1995]**
- ✅ **always** loops forever — §1364-2001 9.9.2 **[1995]**
- ✅ **Continuous assigns as comb processes** re-evaluated on RHS read set — §1364-2001 6.1 **[1995]** Verilator-style semantics
- ✅ **fork/join parallel blocks** — §1364-2001 9.8.2 **[1995]** (sim_fork.rs)
- ✅ **Named forks/blocks** `fork : name … join` — §1364-2001 9.8.3 **[1995]**
- ✅ **wait(cond)** level-sensitive, re-evaluated on typed packed or real condition reads — §1364-2001 9.7.6 **[1995]** false/unknown constants stay suspended while other processes and timed events continue (sim_wait.rs, sim_partial_features.rs)
- ✅ **Wait-free ordinary `always` repeats** — §1364-2001 9.9.2 **[1995]** a generated loop remains a procedure even without timing control; cooperative zero-time budgets report nonconvergence with the process source location instead of silently accepting combinational/run-once behavior
- 🟨 **disable `<label>` / task** — §1364-2001 ch11 **[1995]** uses resolved declaration/instance activation identity. Self-disable is not an early return: it cancels all active invocations of the target. Lexical exits clean up activation records. P18 remains partial; this source review does not establish complete cross-process cancellation support.

SystemVerilog era:

- 🟨 **always_comb** time-zero execution and typed implicit sensitivity — §1800-2009 9.2.2.2 **[SV-2005]** retains called-function reads and excludes written variables, including admitted fixed-array elements. Blocking timing controls and forks are rejected; nonblocking delayed assignments are not rejected merely for their delay. P12 remains partial outside the admitted storage paths (sim_process_semantics.rs).
- 🟨 **always_ff** typed process kind and semantic contracts — §1800-2009 9.2.2.4 **[SV-2005]** requires one event control and prohibits blocking timing controls, forks, and extra writers; blocking data assignments and delayed nonblocking data assignments are legal. Rejections retain source locations (sim_process_semantics.rs).
- 🟨 **always_latch** time-zero execution and typed implicit sensitivity — §1800-2009 9.2.2.3 **[SV-2005]** uses the same timing restrictions as always_comb and retains called-function diagnostics. P12 remains partial outside the admitted storage paths (sim_process_semantics.rs).
- ✅ **join_any/join_none** — §1800-2009 9.3.2 **[SV-2005]** (sim_fork.rs)
- ✅ **wait fork** — §1800-2009 9.6.1 **[SV-2005]**
- ✅ **disable fork** — §1800-2009 9.6.3 **[SV-2005]**
- ✅ **final blocks at end of simulation** — §1800-2009 9.2.3 **[SV-2005]** (sim_final.rs) are captured as typed final processes independent of file extension and run ONCE after the scheduler exits ($finish, deadlock or no future events); they see values committed before simulation ended and `$time` reports the end-of-run time. Nonblocking assignments, task calls, deferred `$strobe`/`$monitor`, and timing controls (`#`/`@`/`wait`/fork) are clean codegen rejects because finals permit function statements only and no scheduled events execute afterward. `$finish` inside a final terminates that final immediately and skips all remaining finals
- 🟨 **Observe/reactive/preponed regions** — §1800-2009 4.4 **[SV-2005]** typed runtime regions, explicit sensitivity-wait migration, fixed-point re-entry and immutable observation views are present; immediate assertion actions run in the current process, and clocking input sampling uses the supported preponed/observed regions, while deferred/concurrent assertions, programs and public VPI constructs remain unsupported
- 🟨 **Clocking input events and sampling** — §§1800-2009 14.3, 14.4, 14.5, 14.6, 14.7, 14.8, 14.9, 14.10, 14.12, 14.13, 14.14, 14.15 **[SV-2005]** owned clocking declarations preserve input directions, aliases, default/global modifiers, concrete interface members and event controls; `#1step`, `#0` and constant positive input skews retain preponed, observed and history samples (sim_partial_features.rs). Output drives, inout driving and `##` cycle delays remain in the H14 boundary.
- ❌ **$exit** program control task — §1800-2009 24.7 **[SV-2005]** unsupported system task
- ⬜ **Fine-grain process control** `process::self()` — §1800-2009 9.7 **[SV-2005]** class-based tier

## 5. Procedural statements

Verilog era:

- ✅ **Blocking assignment** whole/bit/part/indexed-part/array-element LHS — §1364-2001 9.2.1 **[1995]**
- ✅ **Nonblocking assignment** committed in NBA region — §1364-2001 9.2.2 **[1995]** selected updates capture values/indices when issued and merge only selected bits at commit
- ✅ **if/if-else-if** — §1364-2001 9.4 **[1995]**
- ✅ **case** — §1364-2001 9.5 **[1995]**
- ✅ **casez/casex wildcards** LRM 12.5.1 matching — §1364-2001 9.5.1 **[1995]** (sim_casez.rs)
- ✅ **Constant-expression case** `case(1)` priority style — §1364-2001 9.5.2 **[1995]**
- ✅ **Sequential/named blocks** `begin : name … end` — §1364-2001 9.8.1/9.8.3 **[1995]**
- ✅ **for** — §1364-2001 9.6 **[1995]**
- ✅ **while** — §1364-2001 9.6 **[1995]**
- ✅ **repeat** non-constant count runtime-evaluated — §1364-2001 9.6 **[1995]**
- ✅ **forever** — §1364-2001 9.6 **[1995]**
- 🟨 **Event control @** — §1364-2001 9.7.2/9.7.4 **[1995]** packed and scalar real/shortreal expression-change events, fixed-array/resizable-container dependencies, LSB edges, trigger-time `iff`, legal input/const-ref function calls, automatic local/formal evaluator captures and atomic mixed named-event lists work; real edge descriptors and functions with disallowed effects remain rejected (sim_events.rs, sim_partial_features.rs)
- 🟨 **force** `force lhs = expr;` live whole packed/real variables plus constant-selected, concatenated and resolved wire/wired/inout targets, with waiter wakeups, strength-preserving release and explicit RHS dependencies — §1364-2001 9.3.2 **[1995]** (sim_force.rs, sim_net_resolution.rs); variable selects, arrays, automatic/local references and other unsupported net contexts are rejected
- ✅ **release** — §1364-2001 9.3.2 **[1995]** removes the matching force driver; variables retain the forced value unless an active PCA resumes, while nets immediately resolve their current driver slots
- ✅ **force/release driver re-evaluation** — §1364-2001 9.3.2 **[1995]** force RHS changes propagate while active; procedural and structural writes remain underneath according to object kind, including four-state and two-state conversion
- 🟨 **Condition event expressions** `@(a && b)` — §1364-2001 9.7.2 **[1995]** expression operands and legal called-function dependencies drive reevaluation; the controlled body's unrelated read set does not
- 🟨 **Intra-assignment timing** `a = #5 b;` / `a <= #5 b;` — §1364-2001 9.7.7 **[1995]** packed/real/shortreal RHS values are captured immediately; blocking writes suspend, while constant/runtime-delay NBAs capture destinations and continue immediately. Selected NBAs preserve disjoint updates at commit. Explicit event and repeated-event controls capture the RHS at issue time, use update-time blocking selectors versus issue-time NBA selectors, preserve suspension/order, and normalize zero/X/Z/negative repeat counts (sim_delay.rs, sim_partial_features.rs)
- ❌ **Standalone repeat event control** `repeat (n) @ev` — §1364-2001 9.7.7 **[1995]** remains a clean codegen rejection outside intra-assignment timing
- 🟨 **Named events** `event ev; -> ev; ->> ev; @ ev;` — §1364-1995 §9.7.3 / §1800-2009 15.5.1 **[1995/SV-2005]** immediate triggers wake current waiters; untimed and delay-controlled `->>` queue a captured event in NBA without suspending the issuer. Fixed event arrays, hierarchy, null/default handles, reassignment, and task-formal aliases resolve the same persistent object identity while existing waiters remain attached to their original object. Same-slot `.triggered` waits and ordered `wait_order` success/failure arms are supported (§1800-2009 15.5.2–15.5.4); dynamic/associative/queue event storage and event-controlled/repeated-event `->>` remain explicit boundaries.
- ✅ **Procedural continuous assign/deassign** — §1364-2001 9.3.1 **[1995]** (sim_force.rs, sim_procedural_assign.rs) `assign <reg> = expr;` creates one replaceable target binding with a live RHS evaluator and an immediate drive; whole real/shortreal variables and ordinary packed concatenations are also supported; `deassign` removes each binding while retaining the last driven value; RHS and called-function dependencies propagate while assigned; blocking and non-blocking procedural writes cannot override an active PCA or wake its evaluator; a later textual assign site replaces the earlier binding at runtime, and pre-scanned sites make deassign-before-assign order independent; `force` keeps priority over an active PCA and `release` returns to its latest live RHS. Clean rejects: net targets, selects/part-selects/array elements, hierarchical and streaming targets, and recursive aggregate values.

SystemVerilog era:

- ✅ **return** in functions/tasks — §1800-2009 12.8/13.4.1 **[SV-2005]**
- ✅ **unique/priority/unique0 if & case** — §1800-2009 12.4.2/12.5.3 **[SV-2005]** runtime no-match and multiple-match diagnostics retain source identity, default/else suppression, and case/casez/casex four-state matching (sim_unique_priority.rs)
- ✅ **case … inside** wildcard matching — §1800-2009 12.5.4 **[SV-2005]** scalar wildcard items, inclusive ranges, first-match/default behavior, and single selector evaluation (sim_wildcard_eq.rs, optimization on/off)
- ✅ **do-while** — §1800-2009 12.7.5 **[SV-2005]** post-test execution plus break/continue semantics (sim_disable.rs)
- 🟨 **foreach** — §1800-2009 12.7.3 **[SV-2005]** fixed unpacked arrays with declaration-order ranges and omitted dimensions, dynamic arrays/queues, integral and string-keyed associative arrays (including key deletion during traversal), break/continue, and automatic lexical iterators (sim_loops.rs); nested resizable-container elements, nonblocking writes to iterators, and fork captures of string iterators remain rejected
- ✅ **break/continue** — §1800-2009 12.8 **[SV-2005]** (sim_disable.rs) for/while/repeat/forever; continue lands on the increment (for) or back-edge condition test, break exits the innermost loop; nesting pinned
- 🟨 **Inline loop-var declarations** `for (int i…)` — §1800-2009 12.7.1 **[SV-2005]** packed and real loop variables with lexical shadowing, break/continue, and legal synchronous/detached fork value capture (sim_loops.rs, sim_partial_features.rs); nonblocking writes and fork captures of string loop locals remain rejected

## 6. Timing controls

Verilog era:

- ✅ **#delay integer literal**, timescale-scaled — §1364-2001 9.7.1 **[1995]** (sim_timescale.rs)
- ✅ **@\* / @(\*) implicit sensitivity** from call-site body read set — §1364-2001 9.7.5 **[2001]** distinct from `always_comb` called-function sensitivity (sim_process_semantics.rs)
- ✅ **Comb sensitivity to array elements** — §1364-2001 9.7.5 **[2001]** fixed-array element/contents and resizable-container contents/shape dependencies wake implicit and explicit readers without retaining reallocatable element addresses (sim_array_sensitivity.rs)
- 🟨 **Fractional delays** `#0.5` — §1364-2001 9.7.1 **[1995]** typed integer, real, scientific, and unit-suffixed values in statement and intra-assignment delays round once to the local time precision before conversion to design scheduler ticks; constant fractional continuous/gate delays use the same scaling with inertial scheduling; values below the selected precision round to zero
- 🟨 **Expression/parameter delays** `#(expr)` / `#P`, underscored `#10_000` and unit-suffixed `#5ns` literals — §1364-2001 9.7.1 **[1995]** constant and runtime packed/real expressions, function results and task inputs work in statement and intra-assignment delays. Each delay is evaluated once; real values round to local precision, X/Z means zero, and negative packed values convert to unsigned 64-bit time before checked scaling (sim_delay.rs, sim_time_literals.rs, sim_physical_time.rs, sim_partial_features.rs). Negative/nonfinite real delays and values exceeding 64-bit femtosecond scheduler ticks are rejected; unsupported expression forms remain gaps

SystemVerilog era:

- ⬜ **Cycle delays** `##N` — §1800-2009 14.11 **[SV-2005]** requires clocking blocks (tier)

## 7. Expressions & operators

Verilog era:

- 🟨 **Arithmetic** `+ - * / %` — §1364-2001 4.1.5 **[1995]** add/subtract/multiply/division/modulo preserve model-sized limbs; the generated backend rejects widths at its exclusive `1 << 20` capacity, while runtime constructors remain defensive
- 🟨 **Power** `**` — §1364-2001 4.1.5 **[2001]** model-sized operands are supported; backend capacity remains exclusive at `1 << 20`
- ✅ **Bitwise** `& | ^ ~ ^~` — §1364-2001 4.1.10 **[1995]**
- ✅ **Logical** `&& || ! -> <->` — §1364-2001 4.1.9 / §1800-2009 11.4.7 **[1995/SV-2009]** ordinary expression implication short-circuits a known-false antecedent; equivalence evaluates both operands and both preserve four-state X/Z truth (sim_logical_ops.rs). SVA property implication `|->`/`|=>` remains in the assertion boundary.
- ✅ **Reductions** `& ~& | ~| ^ ~^` — §1364-2001 4.1.11 **[1995]**
- ✅ **Shifts** `<< >>` — §1364-2001 4.1.12 **[1995]**
- ✅ **Arithmetic shifts** `<<< >>>` sign-fill — §1364-2001 4.1.12 **[2001]**
- ✅ **Relational** `< <= > >=` — §1364-2001 4.1.7 **[1995]**
- 🟨 **Equality** `== != === !==` — §1364-2001 4.1.8 **[1995]** X/Z compare literally with `===`/`!==`; logical equality preserves known-mismatch dominance over unrelated unknown bits (the broader formal matrix remains a partial claim)
- ✅ **Conditional** `?:` Z-carrying mux — §1364-2001 4.1.13 **[1995]**
- ✅ **Concatenation** `{}` reordered-concat respected — §1364-2001 4.1.14 **[1995]**
- ✅ **Replication** `{n{}}` — §1364-2001 4.1.14 **[1995]**
- ✅ **Bit-select/part-select operands** — §1364-2001 4.2.1 **[1995]**
- ✅ **Indexed part-select** `[+:w]` / `[-:w]` — §1364-2001 4.2.1 **[2001]**
- ✅ **Array addressing** `mem[i][j]` + element selects — §1364-2001 4.2.2 **[1995]**
- ✅ **Strings as operands** reg vectors holding 8-bit ASCII — §1364-2001 4.2.3 **[1995]** packed literal assignment, comparison, concatenation, escapes, padding/truncation, and packed-parameter declaration initializers (sim_packed_strings.rs, optimization on/off); SystemVerilog `string` storage is covered separately by the bounded partial row in §2
- ✅ **$signed/$unsigned** — §1364-2001 4.5 **[2001]**
- ✅ **Signedness/self-determined width rules** mirrored by runtime — §1364-2001 4.4–4.5 **[1995]** pinned by property_elab.rs
- ✅ **X/Z expression semantics** Z=X except identity/copy ops — §1364-2001 3.1 **[1995]** proptests + C vector table
- 🟨 **Operators on real operands** `+ - * / %`, relational/logical/`?:`, casts and ordinary `case` equality — §1364-2001 4.1.1 **[1995]** within real subset; bitwise/reduction/shift/concat/case-eq and real selects are rejected

SystemVerilog era:

- 🟨 **Static casts** `int'(e)`, `signed'()`, `unsigned'()`, size casts `n'(e)` — §1800-2009 6.24.1 **[SV-2005]** Slang retains explicit/implicit conversion identity and resolved target width, signedness, and state domain. Scalar/vector typed and numeric size casts are covered through declaration, runtime-expression, and function contexts; unsupported aggregate/net paths remain outside this claim (sim_data_types.rs, sim_data_type_edges.rs, sim_data_types_next.rs)
- 🟨 **Increment/decrement** `++ --` — §1800-2009 11.4.2 **[SV-2005]** statement-position pre/post forms on whole scalar variables plus expression-valued forms over packed selects/members, fixed-array elements, and real targets are supported; non-lvalues and unsupported aggregate/object targets remain rejected (sim_operator_semantics.rs, sim_expression_mutations.rs)
- 🟨 **Assignment operators** `+= -= *= /= %= &= |= ^= <<= >>= <<<= >>>=` — §1800-2009 11.4.1 **[SV-2005]** whole scalar statement forms plus expression-valued forms over packed selects/members, fixed-array elements, and real targets are supported; statement-position selected/array forms remain outside this claim (sim_operator_semantics.rs, sim_expression_mutations.rs)
- ✅ **Wildcard equality** `==? !=?` — §1800-2009 11.4.6 **[SV-2005]** RHS X/Z bits are wildcards; remaining LHS unknown bits yield X unless a known mismatch decides the result. Common-width/signed extension and model-sized operands are covered with optimization on/off (sim_wildcard_eq.rs).
- 🟨 **Set membership** `inside {…}` — §1800-2009 11.4.13 **[SV-2005]** scalar/range/wildcard cases are reported passing in both modes; aggregate and broader contextual forms remain outside the focused claim
- 🟨 **Streaming operators** `{<<{}}`, `{>>{}}` — §1800-2009 11.4.14 **[SV-2005]** packed and fixed/resizable packed-element RHS/LHS streams preserve declaration order, non-divisible slices, static fixed-array selectors, runtime resizable-array `with` selectors, overlap temporaries, and X/Z in optimized and unoptimized models (`streaming_general.sv`); runtime fixed-array selectors, recursive/object forms, and native `string` stream operands remain outside the supported subset
- ❌ **let expressions** — §1800-2009 11.13 **[SV-2009]**

## 8. Continuous assignments & structural

Verilog era:

- ✅ **Continuous assignment** `assign lhs = rhs;` — §1364-2001 6.1.2 **[1995]** comb process on RHS read set
- ✅ **Multiple/comma-form continuous assigns** — §1364-2001 6.1.2 **[1995]**
- 🟨 **Multiple drivers on one net** — §1364-2001 6.1 **[1995]** bounded ordinary and collapsed scalar net groups resolve continuous, gate and port-derived drivers with explicit strengths and high-Z endpoints; vector-strength, interface/array, and charge-storage contexts remain outside this claim
- 🟨 **Delay on continuous assign** `assign #d lhs = rhs;` — §1364-2001 6.1.3 **[1995]** whole packed, constant-selected, and fixed-unpacked-array element drivers capture RHS values and schedule active-region inertial updates. Changed pending values cancel, unchanged results retain their original deadlines, and a return to the driver's current value cancels without replacement; array elements retain independent cancellation handles. Constant/parameter/fractional delays, 129-bit four-state vectors, scalar strengths, output-port initialization, vector transition selection, and zero-delay/NBA ordering are tested (`sim_delay.rs`, `sim_partial_features.rs`, both optimizer modes). Net-declaration propagation delays and unsupported aggregate/dynamic resolved-net targets remain outside this bounded subset
- 🟨 **Strength on continuous assign/gates** — §1364-2001 6.1.4/7.1.2 **[1995]** explicit scalar strengths flow through continuous assignments, logic/pull gates, wired nets and collapsed inout groups in both optimizer modes; vector continuous strengths remain prohibited by §10.3.4, and switch-level/resistive/trireg behavior remains unsupported
- ✅ **Logic gates** `and nand or nor xor xnor buf not` — §1364-2001 7.2–7.3 **[1995]** (sim_gates.rs) one comb process per gate, SensLoop over the input read set; n-input gates reduce left-to-right, nand/nor/xnor negate after the full reduce; vector gates are bitwise; the current backend requires equal terminal widths
- ✅ **Tri-state buffers** `bufif0 bufif1 notif0 notif1` — §1364-2001 7.4 Table 7-5 **[1995]** (sim_gates.rs) lowered to `sv4_mux(en, data|data, Z)` / `sv4_mux(en, Z, ~(data|data))` — the passing arm is z→x-normalized with `data|data` (per-bit), so an ENABLED gate turns a data-Z into X like buf/not while known bits pass unchanged; a DISABLED gate drives Z; unknown enable yields all-X unless both branches match
- ❌ **MOS/CMOS switches** `nmos pmos cmos rnmos rpmos rcmos` — §1364-2001 7.5–7.7 **[1995]** rejected with a clear message ("switch/transistor primitive … not supported")
- ❌ **Bidirectional switches** `tran tranif0 tranif1 rtran*` — §1364-2001 7.6 **[1995]** rejected with a clear message
- ✅ **pullup/pulldown** — §1364-2001 7.8 **[1995]** (sim_gates.rs) constant 1/0 driver process over the terminal width (RunOnce)
- 🟨 **Strength modeling/resolution tables** — §1364-2001 7.9–7.13 **[1995]** scalar drive endpoints, X uncertainty, wired-net tie rules, pull/supply defaults, gate outputs and collapsed output-port links are modeled; switch-level/resistive propagation, charge strength and aggregate contexts remain unsupported
- 🟨 **Gate delays** `and #2 g(…)` / parameterized `#D` — §1364-2001 7.14 **[1995]** supported whole-signal and constant-selected gate terminals use captured active-region inertial updates with single, rise/fall, and rise/fall/turn-off delays, X initialization and short-pulse cancellation. Enable-gate X/Z values, vector transition selection and unchanged-result deadlines are tested (`sim_gates.rs`, `sim_partial_features.rs`, both optimizer modes); primitive arrays and other terminal forms remain unsupported
- ❌ **Gate instance arrays** `and g[3:0] (…)` — §1364-2001 7.1 **[1995]** captured by the db walk and rejected with a clear message
- ❌ **Combinational UDPs** definition/table/instances — §1364-2001 8.1–8.2/8.6 **[1995]** instances captured and rejected with a clear message ("user-defined primitive instance … not supported")
- ❌ **Sequential UDPs** level/edge-sensitive — §1364-2001 8.3–8.5 **[1995]** same reject as combinational UDPs
- 🟨 **Gate terminal connections** — §1364-2001 7.1 **[1995]** whole plain signals only; select/expression terminals, hierarchical terminals, multi-output `buf`/`not`, and >64-terminal gates are clean rejects, and unequal terminal widths are rejected (a backend restriction — mixed widths are legal Verilog)

(SystemVerilog era: 1800-2009 ch28–29 restore gates/UDPs verbatim — same
statuses as the rows above.)

## 9. Functions & tasks

The next-phase lowering carries persistent static function/task-local and
static-task-NBA storage paths. Focused cases pass; runtime-dependent static
initializers are explicitly rejected rather than evaluated on first call.
Same-lifetime qualifiers are accepted when owned capture is available;
ambiguous or opposite-lifetime overrides are rejected. The established rows
below retain their documented boundaries.

Verilog era:

- ✅ **Function declaration/return value/call in expressions** — §1364-2001 10.3 **[1995]** recursion depth guard 256 reports an error and returns the type's default beyond the guard (sim_function.rs pins within-limit recursion); static output/inout formals use typed persistent copy-in/copy-out storage, including packed and real/shortreal values, and selected actual indices are captured once at the call boundary
- ✅ **Tasks incl. output/inout args**; delay-bearing tasks inlined at call sites — §1364-2001 10.2 **[1995]** wait-bearing tasks inlined (sim_function.rs). Focused static local/formal persistence and static-task NBA copy-out cases pass; delay-free static task outputs/inouts retain storage across calls and copy out at return, including values from earlier NBAs. NBAs to automatic subroutine storage or aggregate/unpacked subprogram storage remain explicitly rejected instead of queuing dangling targets
- ✅ **automatic reentrant functions/tasks** — §1364-2001 10.2.3/10.3.1 **[2001]** recursion supported
- ✅ **Constant functions in parameter expressions** — §1364-2001 10.3.5 **[2001]** evaluated by elab Resolver; typed parameters required (probed)
- ❌ **Task calls inside function bodies** — §1364-2001 10.3.4 **[1995]** rejected
- 🟨 **fork/join inside function/task bodies** — §1364-2001 9.8.2 / §1800-2009 9.3.2 **[1995/SV-2005]** detached `join_none` branches capture automatic packed locals and formals in owned activation frames; ordinary blocking timing in functions, recursive timed tasks, and richer subroutine activation forms remain rejected
- ❌ **Timing controls in function bodies** `#`/`@`/`wait` inside a function rejected — §1364-2001 10.3.4 **[1995]** direct function timing remains rejected; timing controls in a legal detached `join_none` branch use that branch's retained frame. Nonblocking assignments to automatic storage remain rejected
- ❌ **Recursive delay-bearing tasks** — §1364-2001 10.2.3 **[2001]** rejected
- ❌ **Cross-instance hierarchical subprogram calls** — §1364-2001 12.4 **[1995]** rejected

SystemVerilog era:

- ✅ **void functions** `function void f();` — §1800-2009 13.4.1 **[SV-2005]**
- ✅ **Default argument values** incl. references to earlier formals — §1800-2009 13.5.3 **[SV-2005]**
- ✅ **Named argument binding** `f(.b(x), .a(y))` order-independent — §1800-2009 13.5.4 **[SV-2005]** (probed)
- 🟨 **ref arguments** typed `ref`/`const ref` descriptors cover whole packed variables and runtime-selected elements of fixed unpacked arrays, with one-time index binding, immediate aliasing, forwarded nested calls and suspension observation — §1800-2009 13.5.2 **[SV-2005]**. Packed bit/part-select actuals are language-illegal in the 2009 target and are rejected by the frontend; aggregate and resizable-array reference lifetimes remain incomplete (sim_reference_args.rs)

## 10. System tasks & functions relevant to simulation

Verilog era — display family:

- ✅ **$display** — §1364-2001 17.1.1 **[1995]**
- ✅ **$write** — §1364-2001 17.1.1 **[1995]** same formatting as `$display`, without an appended newline (sim_monitor.rs)
- ✅ **Typed format specs** `%d/%h/%x/%b/%o/%c/%u/%z/%v/%t`, `%f/%e/%g`, `%s`, `%m`, and `%l` with the width/precision grammar admitted by Slang; packed X/Z digits, strength text, real/string values, and exact `%%` are preserved across display/write/strobe/monitor and string-producing formatters — §1364-2001 17.1.1.2 / §1800-2009 21.2 **[1995/SV-2005]**. `%p` is limited to scalar packed/string values; aggregate pattern values remain unsupported
- ✅ **$strobe** observes settled values after active/inactive/NBA iteration, including zero-delay drivers triggered by NBAs — §1364-2001 17.1.2 **[1995]** (sim_monitor.rs, sim_partial_features.rs)
- ✅ **$monitor/$monitoron/$monitoroff** one active monitor; registration and re-enabling queue one report at the settled observation point, same-slot changes coalesce, and packed, real, and native-string arguments trigger re-evaluation on change — §1364-2001 17.1.3 **[1995]** (sim_monitor.rs, sim_inout.rs). Aggregate/container values remain outside the bounded formatter
- ✅ **b/o/h console task variants** `$displayb/o/h`, `$writeb/o/h`, `$strobeb/o/h`, `$monitorb/o/h` — §1364-2001 17.1.1–17.1.3 **[1995]** unformatted integral arguments use the variant radix, while explicit format directives retain their own conversion (sim_monitor.rs)

File IO:

- ✅ **Core file output** `$fopen/$fclose/$fdisplay/$fwrite/$fstrobe/$fmonitor` — §1364-2001 17.2.1–17.2.2 / §1800-2009 21.3.1–21.3.2 **[1995/SV-2005]** portable owned descriptors, standard-stream masks, multichannel fan-out, typed radix formatting, and postponed file monitor/strobe output are covered in both optimizer modes; formatted reads remain separate
- ✅ **$sformat/$swrite** and radix variants — §1364-2001 17.2.3 **[2001]** use the shared typed formatter, preserve source-order argument evaluation, and write native string or packed string-like destinations with normal width truncation/padding
- ✅ **Formatted reads** `$fscanf/$sscanf/$fread/$fgets/$fgetc/$ungetc` — §1364-2001 17.2.4 / §1800-2009 21.3.4.1–21.3.4.4 **[2001/SV-2005]** HDL-aware formatted scanning preserves packed widths and X/Z digits, character/line input retains EOF and embedded bytes, and binary reads cover packed values plus ascending/descending memories with start/count bounds (`sim_file_io.rs`, `runtime_file_io.rs`)
- ✅ **File positioning/status** `$ftell/$fseek/$rewind/$fflush/$ferror/$feof` — §1364-2001 17.2.5–17.2.7 / §1800-2009 21.3.5–21.3.8 **[2001/SV-2005]** seek, rewind, flush, EOF/error reporting, output-string ownership, and invalid/closed descriptor status use checked portable host-I/O paths; formatted reads remain separate

Memory load/store:

- ✅ **$readmemh/$readmemb** — §1364-2001 17.2.8 **[1995]** whitespace/comment-separated binary and hexadecimal files, four-state digits, `@` address jumps, declaration-order traversal, optional bounds and checked file/range diagnostics are implemented for fixed one-dimensional packed memories; dynamic/queue/associative, multidimensional and real memories remain rejected (sim_memory.rs)

Time:

- ✅ **$time** module-unit scaled and rounded, `%t` consumes it — §1364-2001 17.7.1 **[1995]** using checked femtosecond-to-unit conversion
- ✅ **$stime/$realtime** — §1364-2001 17.7.2–17.7.3 **[1995]** `$stime` rounds in the calling module's unit before returning the low 32 bits; `$realtime` preserves fractional module-unit time (sim_timescale.rs, sim_physical_time.rs, sim_partial_features.rs)
- ✅ **$printtimescale** — §1364-2001 17.3.1 **[1995]**
- ❌ **$timeformat** — §1364-2001 17.3.2 **[1995]** unsupported-task reject

Control / misc:

- ✅ **$finish** is a nonreturning termination control with default level 1 and explicit level 0/1/2 diagnostics plus exactly-once final-block handoff; the optional target-edition `finish_number` is a constant integral 0, 1, or 2, so runtime, real, unknown, and out-of-range arguments reject before model emission; its argument never becomes the generated model's OS exit status — §1364-2001 17.4.1 / §1800-2009 20.2 **[1995]**
- ✅ **$stop** yields the issuing coroutine and preserves pending queues, activation frames, output state and simulation time until it resumes; default noninteractive CLI runs use `--stop-policy resume`, while `--stop-policy exit` returns control to the caller without running finals or draining future work. Stop numbers use the same constant integral 0/1/2 diagnostic levels as `$finish` and never become process exit codes — §1364-2001 17.4.2 **[1995]** (sim_partial_features.rs)
- ❌ **PLA modeling tasks** `$async$and$array …` — §1364-2001 17.5 **[1995]**
- 🟨 **Stochastic tasks** `$q_initialize/$q_add/$q_remove/$q_full/$q_exam` — §1364-2001 17.6 / §1800-2009 20.16 **[1995]**
  ID-indexed FIFO/LIFO queues retain job/information IDs and simulation-precision arrival ticks; full/empty/unknown/duplicate/type/length status codes and the six `$q_exam` statistics use checked integer arithmetic. Whole packed integer output variables are supported and tested in both optimizer modes (`sim_stochastic.rs`); selected or real output targets remain rejected.
- ✅ **$random** — §1364-2001 17.9.1 **[1995]** writable-seed and implicit-stream calls use the Annex N reference algorithm (`sim_random.rs`, `runtime_random.rs`)
- ✅ **$dist_uniform/$dist_normal/$dist_exponential/$dist_poisson/$dist_chi_square/$dist_t/$dist_erlang** — §1364-2001 17.9.2–17.9.3 **[1995]** checked integer arguments, writable seeds, specified parameter boundaries, and deterministic Annex N vectors (`sim_random.rs`, `runtime_random.rs`)
- ✅ **Conversion** `$rtoi/$itor` — §1364-2001 17.8 **[1995]** truncation toward zero and signed/unsigned integral-to-real conversion, including implicit numeric argument coercion, typed parameters, and constant declaration initializers (sim_real_conversions.rs, optimization on/off)
- ✅ **Conversion** `$realtobits/$bitstoreal` — §1364-2001 17.8 **[2001]** IEEE-754 bit reinterpretation; `$bitstoreal` requires 64 bits and maps X/Z positions to zero (sim_real_conversions.rs, optimization on/off)
- ✅ **Plusargs** `$test$plusargs/$value$plusargs` — §1364-2001 17.10 / §1800-2009 21.6 **[1995]/[SV-2005]**. `llg` passes arguments after `--` to the generated model; test queries use exact leading-`+` prefix matching, and value queries support `%d/%h/%x/%o/%b/%f/%e/%g/%s`, literal `%%`, repeated-argument first-match behavior, wide 4-state destinations, and failure retention (sim_plusargs.rs)

Waveforms:

- 🟨 **$dumpfile/$dumpvars/$dumpon/$dumpoff/$dumplimit** to VCD or FST — §1364-2001 ch18 **[1995]** `$dumpfile` selects the format by `.vcd`/`.fst`; packed signals, unpacked-array elements, real values, X/Z, aliases, hierarchy, exact femtosecond timestamps, dump activation, and size limits are emitted by a separate OS writer thread through a bounded lossless SPSC ring (sim_waveform.rs, sim_physical_time.rs). `$dumpvars` carries owned depth/scope/variable identities into the catalog, filters the catalog before the fixed header, and retains declared array bounds in element names; `$dumpports` remains a separate unsupported extended-VCD family.
- ✅ **$dumpall/$dumpflush** — §1364-2001 ch18 **[1995]** snapshots and synchronous flush barriers work for both VCD and FST (sim_waveform.rs; runtime self-test)
- ❌ **$dumpports extended VCD** — §1364-2001 18.3 **[2001]** unsupported-task reject

SystemVerilog era:

- ✅ **$writememh/$writememb** — §1800-2009 21.5 **[SV-2005]** bounded fixed-memory writers emit consumable binary/hex words in selected declaration/range order and round-trip four-state values; non-packed and resizable memories remain rejected, and writers are rejected in Verilog-2001 mode (sim_memory.rs)
- ✅ **$clog2/$bits** — §1800-2009 20.8/20.6 **[SV-2005]** (`$clog2` first in [1364-2005])
- ✅ **Math functions** `$ln $log10 $exp $sqrt $pow $floor $ceil $sin …` — §1800-2009 20.8 **[SV-2005]** all 21 real functions map to table 20-4 C math functions, with runtime arguments, numeric coercion and C domain behavior (sim_partial_features.rs, both optimizer modes)
- ✅ **Severity tasks** `$fatal/$error/$warning/$info` use the typed display
  formatter, preserve HDL source context and `%m` scope, evaluate each
  message argument once in source order, and write level-specific diagnostics
  to stderr. `$fatal` is an immediate nonreturning termination with
  the constant finish number 0/1/2; nonfatal levels continue execution.
  Severity counters are reported with level-2 finish statistics — §1800-2009
  20.9/20.10 **[SV-2005]** (sim_partial_features/severity.rs)
- ✅ **$sformatf** — §1800-2009 21.3.3 **[SV-2005]** returns an owned formatted string, supports dynamic and nested format expressions, and evaluates typed arguments once in source order
- ✅ **Bit-vector helpers** `$onehot/$onehot0/$countones/$isunknown` — §1800-2009 20.6 **[SV-2005]** packed operands through the generated model width, X/Z-aware counting, parameters and constant declaration initializers, single argument evaluation, and combinational dependencies; real operands rejected (sim_bit_queries.rs, optimization on/off)
- ❌ **Sampled-value functions** `$rose/$fell/$stable/$past/$sampled` — §1800-2009 16.9.3 **[SV-2005]** unsupported-function reject
- ✅ **Shortreal conversion** `$bitstoshortreal/$shortrealtobits` — §1800-2009 20.5 **[SV-2005]** 32-bit IEEE-754 reinterpretation and shortreal rounding; `$bitstoshortreal` requires 32 bits and maps X/Z positions to zero (sim_real_conversions.rs, optimization on/off)
- ✅ **$system** — §1800-2009 20.18 **[SV-2009]** task and function forms lower
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

- ✅ **`` `default_nettype ``** affects implicit net kinds at elaboration — §1364-2001 19.2 **[1995]**
- ✅ **`` `resetall ``** resets directive state — §1364-2001 19.6 **[1995]** consumed by frontend
- ✅ **`` `celldefine ``/`` `endcelldefine ``** cell tagging — §1364-2001 19.1 **[1995]** consumed by frontend, no sim effect
- ✅ **`` `unconnected_drive ``/`` `nounconnected_drive ``** — §1364-2001 19.9 **[1995]** consumed by frontend
- ✅ **`` `line ``** position override — §1364-2001 19.7 **[2001]** consumed by frontend

(`` `timescale `` is covered in §1.)

SystemVerilog era:

- ✅ **`` `begin_keywords ``/`` `end_keywords ``** keyword-set selection — §1800-2009 22.14 **[SV-2005]** (`begin_keywords` first in [1364-2005]) consumed by the frontend as lexical state only; it does not override the selected global edition
- ✅ **`` `pragma ``** — §1800-2009 22.11 **[SV-2009]** consumed by frontend
- ✅ **`` `undefineall ``** — §1800-2009 22.5.3 **[SV-2009]** consumed by frontend
- ✅ **`` `__FILE__ ``/`` `__LINE__ `` predefined macros** — §1800-2009 22.13 **[SV-2009]** consumed by frontend

## 12. Out-of-scope verification-infrastructure tier (⬜)

Tracked so nothing is lost; all de-prioritized behind RTL-simulation support.

| Area | Constructs | Reference | Tag |
|---|---|---|---|
| Advanced classes | inheritance, virtual methods, parameterized classes, access/lifetime rules, and virtual-interface handles | §1800-2009 ch8, 25.9 | [SV-2005] |
| Programs | `program … endprogram`, race-elimination region, `$exit` | §1800-2009 ch24 | [SV-2005] |
| Clocking output/cycle controls | output skews, synchronous drives, inout driving, `##` delays | §1800-2009 14.11, 14.16.2 | [SV-2005] |
| Interprocess sync | semaphores, mailboxes, process suspend/resume/kill | §1800-2009 ch15 | [SV-2005] |
| Assertions | deferred/concurrent assert-assume-cover, sequences/properties, assertion control tasks; immediate forms are covered in the bounded simulator subset | §1800-2009 ch16, 20.11 | [SV-2005] |
| Checkers | `checker … endchecker` | §1800-2009 ch17 | [SV-2009] |
| Coverage | covergroups, coverpoints, cross | §1800-2009 ch18 | [SV-2005] |
| Constrained randomization | `randomize()`, rand/c, constraints, `std::randomize` | §1800-2009 ch18 | [SV-2005] |
| DPI | import/export functions, contexts | §1800-2009 ch35 | [SV-2005] |
| PLI/VPI | `tf_`/`acc_`/`vpi_` interface, PLI applications | §1364-2001 ch20–27 | [1995]/[2001] |
| Specify blocks | module paths, edge/state-dependent paths, `PATHPULSE$` | §1800-2009 ch30 | [1995]/[SV-2009 restored] |
| Timing checks | `$setup/$hold/$width/…`, notifiers, `&&&` conditions | §1800-2009 ch31 | [1995]/[SV-2009 restored] |
| SDF backannotation | `$sdf_annotate`, delay/check mapping | §1800-2009 ch32 | [1995]/[SV-2009 restored] |
| bind | elaboration-time injection of modules/interfaces | §1800-2009 23.11 | [SV-2005] |
| Virtual interfaces | `virtual ifc` handles in classes | §1800-2009 25.9 | [SV-2005] |

## How to update this document

1. Land the feature in the owning lowering/runtime/IR/emitter modules.
2. Add or extend a regression test under `tests/` asserting exact behavior.
3. Flip this item's marker (🟨→✅, ❌→✅, …), replace `(probed)` with the test
   file name, and keep any caveat note accurate.
4. Update the remaining-work inventory and its grouped counts when an item closes.
5. Mention the batch/PR that changed the marker in the commit message; this
   file is documentation-only — never commit simulator behavior without a test.

## Remaining-work inventory

The original audit IDs are stable. This inventory currently contains 63 remaining
groups (21 missing, 42 partial); groups 9, 27, 38, 50, 51, 57, 58, 59 and 60 are completed. Counts refer to grouped
capabilities, not individual keywords, system functions or standard clauses.


| # | Status | Feature group | Missing forms or behavior |
|---:|---|---|---|
| 1 | Missing | Charge-storage nets | `trireg`, charge strengths, charge decay and charge sharing. |
| 2 | Missing | Tagged unions | Tagged storage, construction and matching. |
| 3 | Partial | Real types | Scalar `real`/`realtime`/`shortreal` ports, combinational reads, event/wait controls and typed changed-write notifications are covered. General ports, arrays, subroutine storage, continuous assignments, and real monitor/strobe arguments remain. |
| 4 | Partial | Strings | General string ports, formals, locals, static string-returning functions, continuous assignments and sensitivity. |
| 5 | Partial | Chandles | Ports, packed containment, arithmetic, continuous assignment and sensitivity remain unsupported; the bounded implementation covers null/copy/comparison and Boolean operations, automatic/static locals, represented aggregate members, mixed signatures, output/inout/ref/const-ref aliases, delay-bearing tasks and chandle-input→chandle-return functions. |
| 6 | Partial | Structures and untagged unions | Nested unpacked/object members, recursive defaults and nominal type keys, anonymous copies without type identity, unequal-width unpacked unions, ordinary aggregate copy ports/nets and general subroutine storage. |
| 7 | Partial | Fixed unpacked arrays | Whole-array procedural assignment and concatenation, slices/partial indexing, general element types, multidimensional copy-port forms and runtime-indexed copy-port actuals. Fixed-array reference ports and element indexed part-selects now have file-based read/write, range, state-conversion, wide/invalid-index and delayed-NBA coverage. |
| 8 | Partial | Resizable containers and array methods | Packed one-dimensional dynamic-array/queue reductions with typed `item`/`item.index()` `with` callbacks, locator/min/max/unique result queues, and sort/rsort/reverse/shuffle mutation are covered; packed associative arrays also cover locator/min/max/unique results with integral-key indices. General nested/multidimensional or object elements, string-key index result queues, declaration initializers, keyed/default patterns, subroutine/port storage, and callback captures of automatic locals remain unsupported. |
| 9 | Completed | Runtime enum methods | Scalar enum first/last/next/prev/num/name methods use owned declaration-order metadata, including sparse/signed values, wrapping counts, invalid-value defaults and owned names. |
| 10 | Partial | Casts | Dynamic `$cast` for packed/real scalar destinations (including enum membership checks), plus fixed-size aggregate/array and packed-element dynamic/queue bit-stream casts. Class downcasts and string/object or recursive resizable-element paths remain unsupported; scalar/vector static casts and tested numeric conversions work. |
| 11 | Partial | Data and array queries | Executed `$typename`, `$isunbounded`, `$bits`, `$left`/`$right`/`$low`/`$high`/`$increment`/`$size`/`$dimensions`/`$unpacked_dimensions` with descriptor-backed packed, fixed-array, dynamic/queue, associative-integral, and string metadata; nested runtime container dimensions and string/wildcard associative bounds remain unsupported. |
| 12 | Partial | Initialization and storage lifetimes | Scalar nonconstant declaration initializers and runtime-dependent static subroutine initializers are lowered with declaration identity, static/automatic storage lifetime, source origin, and edition-specific scheduling (SV2009 pre-process initialization versus Verilog-2001 active-region initialization). Automatic activations and explicit static locals are supported for the packed scalar subset; recursive aggregate defaults, general aggregate/subroutine storage, and ambiguous lifetime provenance remain outside the boundary. |
| 13 | Partial | Net resolution and strengths | Continuous, gate, packed port and collapsed-inout drivers retain per-source strength endpoints across wire/tri, wand/wor, tri0/tri1 and supply nets, including implicit pull/supply defaults, constant selected/disjoint writes and selected delayed drivers. Heterogeneous/aggregate connectivity and switch-level/resistive propagation remain; explicit vector continuous-assignment strengths are language-illegal and are not counted. |
| 14 | Missing | Switch-level primitives | `nmos`, `pmos`, `cmos`, `rnmos`, `rpmos`, `rcmos`, `tran`, `tranif0/1`, `rtran`, `rtranif0/1`, including resistive strength propagation. |
| 15 | Missing | Primitive instance arrays | Arrays of gate and UDP instances. |
| 16 | Missing | User-defined primitives | Combinational and sequential UDP tables, state/edge behavior and instances. |
| 17 | Missing | Specify blocks | Specify parameters, module path delays, state/edge-dependent paths and pulse-control semantics including `PATHPULSE$`. |
| 18 | Missing | Timing checks | `$setup`, `$hold`, `$setuphold`, `$recovery`, `$removal`, `$recrem`, `$skew`, `$timeskew`, `$fullskew`, `$period`, `$width`, `$nochange`, notifiers and timing-check conditions. |
| 19 | Missing | SDF annotation | `$sdf_annotate` and application of annotated delays/checks. |
| 20 | Partial | Gate terminal forms | Selected, expression and hierarchical terminals; unequal terminal widths; multiple-output `buf`/`not`; more than 64 terminals. |
| 21 | Partial | Continuous and gate delays | Whole packed, constant-selected and fixed-unpacked-array element continuous/gate drivers capture values, cancel pulses and commit in the active region without suspending evaluation; array elements keep independent cancellation identity. Single, rise/fall and rise/fall/turn-off lists select transition-specific ticks, including the bounded minimum for ambiguous X transitions. Net-declaration propagation delays, primitive arrays, dynamic resolved-net targets and other unsupported gate terminal contexts remain. |
| 22 | Partial | Delay expressions | Negative real delays and unsupported expression forms remain. Runtime packed/real procedural delays, X/Z-to-zero delays, negative packed time conversion, femtosecond precision, local-precision rounding and checked overflow are tested in both optimizer modes. |
| 23 | Partial | Time precision and literal semantics | Physical scheduler ticks are checked femtoseconds across the standard 1fs–100s range. Ordinary time-literal values, source spelling, local `timeunit`/`timeprecision`, signed/scientific unit suffixes, and direct-delay rounding follow the selected 2009 policy; broader edition-sensitive literal semantics remain separately tracked. |
| 24 | Partial | Port connections | Constants, input expressions, omitted defaults, selected output targets, packed/fixed-array reference aliases, recursive aggregate/object reference leaves, resolved packed net links and scalar real/shortreal links now work. Resizable containers, ordinary aggregate copy ports and heterogeneous links remain restricted. |
| 25 | Missing | Libraries and configurations | The standard library/configuration selection flow is not provided as a supported simulator feature. |
| 26 | Partial | Packages | Shared runtime package variables, dependent initialization, static package subroutines, qualified/imported/re-exported calls, and bounded `$unit` visibility retain owned namespace identity. General aggregate/package storage and broader compilation-unit/header forms remain outside this boundary. |
| 27 | Completed | Net aliases | True-net `alias` declarations merge legal whole-net, constant selected, constant indexed-selected and concatenated packed-net expressions into bit-level canonical networks. Continuous/gate drivers, packed input/output/inout links, force/release, dependency wakeups and waveform observations use one resolved network across alias names; dynamic selects, aggregate members and switch-level/resistive aliasing remain outside the bounded implementation — §1800-2009 10.11 **[SV-2009]** (sim_net_resolution.rs, optimizer on/off). |
| 28 | Partial | SystemVerilog scheduling regions | The runtime has typed Preponed, Active, Inactive, Pre-NBA/NBA/Post-NBA, Pre-Observed/Observed/Post-Observed, Reactive/Re-Inactive/Re-NBA, Pre-Postponed/Postponed, and explicit PLI callback queues with fixed-point re-entry and immutable observation views. Clocking input sampling now consumes the preponed/observed paths; assertions, program blocks, output clocking drives and public VPI registration remain later feature work. |
| 29 | Partial | Zero-delay process behavior | Ordinary wait-free `always` repeats with a cooperative, configurable zero-time budget; exhausted budgets produce a source-bearing nonconvergence diagnostic and nonzero simulator status. `always_comb`/`always_latch` keep their time-zero and sensitivity shaping. Constant-false/unknown `wait` remains suspended without preventing time advancement. |
| 30 | Partial | Conditional event controls | Packed and scalar real expression changes, LSB edge semantics, trigger-time `iff`, fixed-array/container dependencies, legal input/const-ref calls, and evaluator captures of automatic procedural/subroutine locals and formals work, including mixed named events. Real edge descriptors and functions with output/inout/ref writes or other disallowed effects remain rejected. |
| 31 | Partial | Named-event references | Direct scalar and fixed-array named events, qualified/hierarchical event-control sources, runtime array selects, reassignment/null identity, and task-formal aliases are tested. Dynamic/associative/queue event storage remains outside this boundary; advanced event operations are in item 65. |
| 32 | Partial | Array/container sensitivity | Fixed-array element/contents and resizable-container contents/shape dependencies wake continuous, explicit/implicit combinational, event, wait, NBA, copy, resize, and supported method readers; whole-array representation, nested/object elements, and callback-dependent array-method sensitivity remain outside this claim. |
| 33 | Partial | Intra-assignment timing | Constant/runtime-delay and explicit event/repeat NBAs capture values/destinations without suspending, preserve issue order and outlive their issuer. Blocking event controls suspend while preserving RHS-before-wait and LHS-at-update behavior; selected NBAs merge into current storage at commit. Zero/X/Z/negative repeat counts complete without a waiter. Standalone repeat event controls, nested repeat controls, and unsupported automatic function/task storage remain outside this boundary. |
| 34 | Partial | Force and release | General array/automatic targets and some advanced hierarchical/net-driver contexts remain unsupported; live RHS evaluation, persistent hierarchical/static-subroutine targets, object-correct variable/net release, PCA precedence, selected net targets and bounded resolved-driver overlays are implemented. |
| 35 | Partial | Procedural assign/deassign | Whole packed and real/shortreal variable targets plus ordinary packed concatenations have PCA priority, runtime site replacement, dependency-driven reevaluation and deassign retention, including the active-PCA/force layer. Selected/hierarchical/array/streaming and recursive aggregate targets remain unsupported. |
| 36 | Partial | Named disable | Declaration/instance-resolved cross-process block/task disables, named-fork targets, simultaneous task activations, hierarchical instance selection, inactive-target no-ops, and retained nested-descendant cleanup are tested. Broader recursive and advanced subroutine-storage interactions remain outside this boundary. |
| 37 | Partial | Loop forms and captures | Omitted fixed-array dimensions, dynamic arrays, queues, integral/string associative traversal, real for-loop locals, shadowing, break/continue, and packed/real captures are tested. Nested resizable-container elements, nonblocking loop-local writes, and string-loop captures remain. |
| 38 | Completed | Uniqueness and priority checking | Runtime `unique`, `unique0` and `priority` violation checks retain source identity, ordinary first-match execution, default/else suppression, and exact/casez/casex matching. |
| 39 | Missing | Pattern matching | General `case ... matches` and conditional pattern matching; the probed packed-struct case produced no required branch output. |
| 40 | Partial | Side-effecting operators | Expression-valued `++/--` and compound assignments preserve one-time selected/array target evaluation, prefix/postfix results, packed state conversion and real updates; statement-position selected/array forms and broader aggregate targets remain. |
| 41 | Partial | Set membership | General aggregate/contextual `inside` forms; tested scalar/range/wildcard forms work. |
| 42 | Partial | Streaming | Packed and fixed/resizable packed-element packing/unpacking, static fixed-array selectors, runtime resizable-array `with` selectors, non-divisible slices and overlap writes are covered; runtime fixed-array selectors, native strings, and recursive/object forms remain incomplete. |
| 43 | Partial | Reference subroutine arguments | Typed `ref`/`const ref` descriptors alias whole packed variables and runtime-selected elements of fixed unpacked arrays, including one-time index binding, forwarded aliases, bounded automatic calls and suspension observation. Aggregate and resizable-array lifetimes remain incomplete. Packed bit/part-select actuals are language-illegal for 2009; they must not be listed as supported. |
| 44 | Missing | Recursive timed tasks | Recursion through delay/wait-bearing tasks. |
| 45 | Partial | Parallel subroutine bodies | Detached `join_none` branches in packed subroutine bodies retain automatic formals/locals; resumable timed task activations, full ref/copy-out semantics and broader detached-function forms remain. Ordinary blocking timing in functions is illegal. |
| 46 | Missing | Cross-instance subroutine calls | Hierarchical calls to tasks/functions outside the calling instance, including interface/package subroutine contexts. |
| 47 | Partial | Subroutine copy-out and storage | General unpacked/aggregate storage plus string/event output/inout call paths and legal NBAs to persistent unpacked subroutine storage remain. Chandle output/inout/ref/const-ref aliases and bounded delayed-task lifetime are covered. Packed and scalar real/shortreal static function output/inout expression copy-out is covered; NBAs to automatic variables are language-illegal and excluded. |
| 48 | Partial | File I/O | Owned standard-stream/ordinary-file descriptors, multichannel output, `$fopen/$fclose/$fdisplay/$fwrite/$fstrobe/$fmonitor`, `$fscanf/$sscanf/$fread/$fgets/$fgetc/$ungetc`, and `$ftell/$fseek/$rewind/$fflush/$ferror/$feof` are covered in both optimizer modes. Broader aggregate input remains separate. |
| 49 | Partial | Display families and formatting | Typed console, file and string formatting preserves packed, real, and owned string values, supports the legal `%d/%h/%x/%b/%o/%c/%u/%z/%v/%t/%f/%e/%g/%s/%m/%l` conversions, width/precision directives, exact `%%` escaping, and HDL hierarchy names. Deferred string/real snapshots retain safe ownership, monitor object dependencies trigger settled reports, and strobes retain issue order; aggregate pattern values remain outside the bounded formatter. |
| 50 | Completed | String formatting tasks/functions | `$sformat`, `$swrite` and their radix variants, plus `$sformatf`, use typed owned arguments and the shared formatter. Native string and packed string-like destinations receive normal truncation/padding; dynamic/nested format expressions and source-order, exactly-once arguments are covered by `sim_h04_string_format.rs`. |
| 51 | Completed | Memory file loading and writing | Fixed one-dimensional packed memories support four-state `$readmemh/$readmemb` parsing (comments, radix words, `@` jumps, range/order/size diagnostics) and SystemVerilog `$writememh/$writememb` output with round-trip coverage; dynamic/queue/associative, multidimensional and real memories are rejected, as are writers in Verilog-2001 mode. |
| 52 | Partial | Real-time reporting and time formatting | `$timeformat` remains missing. `$realtime` returns fractional time in the calling module's units; `$time` and `$stime` round to the nearest local unit (exact halves upward) before `$stime` applies its low-32-bit result width. |
| 53 | Partial | Simulation suspension | `$stop` supports resumable coroutine suspension and explicit CLI resume/exit policy; a full interactive debugger/control protocol is outside this boundary. |
| 54 | Missing | PLA modeling | Synchronous/asynchronous AND/NAND/OR/NOR array/plane system tasks. |
| 55 | Partial | Stochastic queues | `$q_initialize/$q_add/$q_remove/$q_full/$q_exam` with FIFO/LIFO state, documented status codes, simulation-time statistics and whole packed integer outputs; selected/real output targets remain rejected. |
| 56 | Partial | Random-number facilities | Legacy `$random` and all seven `$dist_*` functions are implemented with writable seeds and Annex N vectors. `$urandom`/`$urandom_range` use deterministic per-process streams with inclusive unbiased ranges; `process::self().srandom()` and randstate save/restore are covered in both optimizer modes (`sim_random_streams.rs`, `runtime_random.rs`), and process/fork creation derives stable child streams. Class-object stream methods, constrained randomization, and other SystemVerilog randomization APIs remain. |
| 57 | Completed | Command-line plusargs | `$test$plusargs/$value$plusargs` receive arguments after the `llg` `--` delimiter; exact prefix matching, typed decimal/hex/binary/octal/real/string conversion, wide 4-state values, literal percent escapes, repeated-argument first match, and unchanged destinations on failed queries are covered by `sim_plusargs.rs`. |
| 58 | Completed | Runtime mathematical functions | All 21 real functions from IEEE 1800-2009 table 20-4 now use typed IR and the specified C math functions, with numeric argument conversion. Procedural tests cover runtime arguments, one-time evaluation and C domain behavior. Existing real-context restrictions are counted in group 3. |
| 59 | Completed | Runtime severity tasks | `$fatal/$error/$warning/$info` use typed, exactly-once message evaluation with source-context prefixes; `$fatal` validates constant finish number 0/1/2, runs the existing termination/final handoff, and level-2 finish statistics include stable severity counters. Elaboration-time frontend diagnostics remain a separate capability. |
| 60 | Completed | Host command execution | `$system` task/function forms preserve omitted (`system(NULL)`) versus explicit-empty commands, use one optional owned string and explicit generated-process permission, and return raw host `system()` status; shell syntax and status encoding remain platform-specific. |
| 61 | Partial | Waveform selection and extended VCD | `$dumpvars` depth/scope/variable filtering is implemented for ordinary VCD/FST catalogs; the `$dumpports` extended-VCD family remains unsupported. |
| 62 | Partial | Classes | Nominal class handles, heap `new`, packed/real properties, default/explicit constructors, static packed/real properties and methods, this-bound methods, null checks, and shallow handle aliasing are covered by `sim_classes.rs` in both optimizer modes. Inheritance, virtual/parameterized classes, string/chandle/class properties, timing-bearing class tasks, access qualifiers, and virtual-interface handles remain missing. |
| 63 | Missing | Program blocks | Program execution semantics, reactive scheduling and `$exit`. |
| 64 | Partial | Clocking input sampling | Clocking declarations, default/global clocking, input directions/aliases, clocking events and constant `#1step`/`#0`/positive input skews use owned sample storage and optimizer-parity runtime scheduling. Output skews/drives, inout driving and `##` cycle delays remain unsupported. |
| 65 | Missing | Advanced interprocess synchronization | Semaphores, mailboxes, process handles/status/suspend/resume/kill/await; event `.triggered` and `wait_order` are covered in the named-events row, as are direct nonblocking named-event triggers. |
| 66 | Partial | Assertions and sampled values | Immediate `assert`/`assume`/`cover` evaluate once with four-state truth, explicit or standard default actions, labels and optimizer-preserved cover callbacks (`sim_partial_features/assertions.rs`); deferred/concurrent forms, sequences/properties, `expect`, assertion-control tasks and sampled-value functions including `$sampled/$rose/$fell/$stable/$changed/$past` and the 2009 global-clocking forms remain. |
| 67 | Missing | Checkers | Checker declarations, instances and checker execution. |
| 68 | Missing | Functional coverage | Covergroups, coverpoints, bins, crosses, sampling, coverage queries/control and coverage database system tasks. |
| 69 | Missing | Constrained and structured randomization | `rand/randc`, constraints, object and `std::randomize`, `randcase` and `randsequence`. |
| 70 | Missing | DPI | DPI import/export of functions/tasks, context/pure semantics and foreign-call integration. |
| 71 | Missing | PLI/VPI | Standard `tf_`, `acc_`, `vpi_` interfaces, callbacks and PLI application integration. |
| 72 | Missing | Virtual interfaces | Virtual-interface handles, assignment and member access through those handles. |
