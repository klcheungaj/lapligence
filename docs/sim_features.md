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
zero-delay loop guard = 10M scheduler passes; resolved net drivers ≤ 16 per group;
force slots ≤ 64; processes ≤ 4096; named-event waiters ≤ 64 per event;
final blocks ≤ 1024.

## Coverage and remaining work

- Supported markers describe the bounded behavior on each row, not unrestricted IEEE conformance.
- The [remaining-work inventory](#remaining-work-inventory) groups known missing forms and correctness gaps; it is not an IEEE-defined feature count.
- Testing methods, limits and commands are in [tests/readme.md](../tests/readme.md).

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
- ✅ **`` `include ``** — §1364-2001 19.5 **[1995]** resolved by frontend
- ✅ **`` `timescale `` honored** scales `#N` and `$time` with Slang's resolved owning-module time unit and precision — §1364-2001 19.8 **[1995]** compilation-unit/declaration inheritance is resolved by the frontend; fs clamps to the runtime's 1ps floor (sim_timescale.rs pins ns/ps scaling)

SystemVerilog era:

- ✅ **Fill literals** `'0/'1/'x/'z` — §1800-2009 5.7.1 **[SV-2005]** context sizing in supported packed arithmetic/bitwise expressions, comparisons, conditional branches, assignments, function arguments, and case/casez/casex; self-determined concatenation/replication operands remain one bit (sim_fill_literals.rs, optimization on/off)
- 🟨 **Time literals** `2.1ns` — §1800-2009 5.8 **[SV-2005]** the owned semantic model retains Slang's typed unit and unrounded module-scaled `real` value. Ordinary expression evaluation preserves that value, following Slang v11's IEEE 1800-2023 behavior; conversion of a complete procedural delay expression rounds once to the local time precision. Runtime scheduling remains bounded to 64-bit ticks and a minimum 1ps design precision

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
- ✅ **Net declaration assignment** `wire w = expr;` — §1364-2001 3.6 **[1995]** behaves as a continuous driver for constant and dynamic RHS expressions, using the same event-driven run-once/sensitivity-loop IR as an explicit `assign` (sim_net_decl.rs); dynamic reads of unpacked arrays and unsupported resolved-net classes are rejected explicitly
- ✅ **Variable declaration initializers** scalar `reg x = 0;`, `logic l = 1'b0;`, `int x = P+1;` — §1364-2001 6.2.1 **[2001]** constant RHS only, non-constant rejected (sim_varinit.rs, sim_geninit.rs)
- ✅ **Parameters** override + propagation — §1364-2001 3.11.1 **[1995]** (elab_resolve.rs)
- ✅ **localparam** — §1364-2001 3.11.2 **[2001]**
- 🟨 **real/realtime** — §1364-2001 3.9 **[1995]** scalar procedural vars/params subset only (sim_real.rs)
- 🟨 **wand/wor/triand/trior wired resolution** — §1364-2001 3.7 **[1995]** standalone packed nets resolve equal-strength whole-net continuous/declaration assignments with one slot per driver site (≤16); Z is neutral, 0 dominates X for wired-AND and 1 dominates X for wired-OR (sim_net_resolution.rs, runtime_values.rs); port/interface/array nets, hierarchical/select/procedural writes, force/release, gate/function/task-output drivers, and explicit strengths are rejected
- 🟨 **tri0/tri1/trireg/supply0/supply1 pull semantics** — §1364-2001 3.7 **[1995]** standalone `tri0/tri1` apply implicit pulls only to all-Z bits after ordinary-driver resolution; `supply0/supply1` dominate ordinary drivers, with correct initial defaults (sim_net_defaults.rs, runtime_values.rs); same bounded standalone-driver restrictions as wired nets; `trireg` charge storage and resistive propagation remain unsupported
- 🟨 **drive strength / charge strength** — §1364-2001 3.4 **[1995]** explicit drive-strength resolution is covered for standalone scalar `wire`/`tri` continuous-assignment drivers; vector/gate/inout/wired-strength cases and `trireg` charge storage remain unsupported
- ❌ **specparam** — §1364-2001 3.11.3 **[1995]** specify blocks unsupported
- ❌ **vectored/scalared hints** — §1364-2001 3.3 **[1995]** no dedicated handling

SystemVerilog era:

- 🟨 **logic/bit vectors** — §1800-2009 6.9/6.11 **[SV-2005]** `logic` has 4-state storage; `bit`/2-state vectors coerce X/Z to zero on assignments and casts. Scalar/vector paths and the tested packed-aggregate paths are covered; unsupported aggregate/net member contexts remain outside this claim
- 🟨 **byte/shortint/int/longint** 2-state ints — §1800-2009 6.11 **[SV-2005]** widths/signedness and X/Z-to-zero coercion are implemented for scalar/vector paths and the tested packed aggregates; unsupported net/member contexts remain outside this row's claim
- ✅ **uwire nets** — §1800-2009 6.6.2 **[SV-2005]** checked compilation rejects overlapping drivers; standalone whole/declaration and disjoint constant-selected drivers preserve four-state values and undriven Z bits. Driver release, generate scopes, ordinary ports/interface defaults, and undriven net arrays are tested in both optimizer modes (`sim_type_conformance.rs`); the ordinary-net context restrictions still apply
- ✅ **typedef simple/packed-vector aliases** — §1800-2009 6.18 **[SV-2005]** resolved by frontend (probed)
- ✅ **Array declaration initializers** `'{…}` patterns applied element-wise in linear-index order — §1800-2009 10.9.1 **[SV-2005]** constant elements only (sim_memory.rs)
- ✅ **enum-typed scalar variables** — §1800-2009 6.19 **[SV-2005]** stored at the elaborated packed base width; enum constants fold through the frontend. Base-state and signedness behavior pass at both exercised widths (sim_operator_semantics.rs)
- 🟨 **packed/unpacked struct and union aggregates** — §1800-2009 7.2–7.3, 7.4 **[SV-2005]** packed/unpacked struct assignment patterns support positional, named, default, built-in, and packed-integral typedef keys with exact range/state/signedness matching and member overrides. Packed unions support direct width-matched initialization, and packed/unpacked unions support selected-member writes over the tested fixed packed members. Nested recursive defaults, nominal type keys, nested unpacked/object members, aggregate ports/nets/subprogram storage, tagged unions, and general aggregate slices remain unsupported; the bounded `sim_data_types_next` and `sim_data_types_completion` inventories are not an exhaustive conformance claim
- 🟨 **string type/signals/params** — §1800-2009 6.16 **[SV-2005]** focused basic declaration, cast/copy, display-extra, automatic string-return-with-packed-input, and 128/512-bit `.atoreal`/`.realtoa` conversion cases are covered in both modes; string formals are not supported. Module/generate storage and other core methods remain bounded; string subroutine forms, ports, continuous-assignment/sensitivity paths, and unverified formatted/real methods remain outside this claim
- 🟨 **event data type** scalar `event ev;` declarations and direct references are captured — §1800-2009 6.17 **[SV-2005]**; event arrays and hierarchical event references are not yet projected into the owned semantic database
- 🟨 **dynamic arrays / associative arrays / queues** — §1800-2009 7.5/7.8/7.10 **[SV-2005]** bounded dynamic-array, queue, and integral/string-key associative cases cover element reductions and positional assignment patterns; legal reductions with width-changing `with` clauses are explicitly diagnosed unsupported rather than silently dropping the clause. Containers are bounded to 1-D packed elements, not general nested/object-member forms
- 🟨 **chandle** — §1800-2009 6.14 **[SV-2005]** focused native-chandle call cases are reported passing in both modes, limited to null/copy/compare/Boolean operations and chandle-input→chandle-return functions

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
- 🟨 **ref ports / default port values** — §1800-2009 23.2.2.2/23.2.2.4 **[SV-2005]** matching whole packed-variable references share storage through nested instances; constants, expressions and omitted defaults work for packed input ports. Selected/aggregate/object reference actuals remain unsupported (sim_partial_features.rs, both optimizer modes)
- ⬜ **extern / nested modules** — §1800-2009 23.4–23.5 **[SV-2005]** out of scope

## 4. Scheduling & processes

Verilog era:

- ✅ **Stratified regions** active / inactive(`#0`) / NBA / re-eval loop until quiescent — §1364-2001 5.3–5.4 **[1995]** pinned by region_conformance.rs
- ✅ **initial** runs once then exits — §1364-2001 9.9.1 **[1995]**
- ✅ **always** loops forever — §1364-2001 9.9.2 **[1995]**
- ✅ **Continuous assigns as comb processes** re-evaluated on RHS read set — §1364-2001 6.1 **[1995]** Verilator-style semantics
- ✅ **fork/join parallel blocks** — §1364-2001 9.8.2 **[1995]** (sim_fork.rs)
- ✅ **Named forks/blocks** `fork : name … join` — §1364-2001 9.8.3 **[1995]**
- ✅ **wait(cond)** level-sensitive, re-evaluated on condition reads — §1364-2001 9.7.6 **[1995]** false/unknown constants stay suspended while other processes and timed events continue (sim_wait.rs, sim_partial_features.rs)
- 🟨 **Wait-free always treated as combinational** — §1364-2001 9.9.2 **[1995]** documented approximation; warns when it reads nothing
- ✅ **disable `<label>` / task early return** — §1364-2001 ch11 **[1995]** Slang's resolved target identity drives same-process named/nested block exits and inlined task early return; cross-process and named-fork targets remain explicit rejects (sim_disable.rs)

SystemVerilog era:

- ✅ **always_comb** sensitivity synthesized from body read set — §1800-2009 9.2.2.2 **[SV-2005]**
- ✅ **always_ff** lowered like always+event control — §1800-2009 9.2.2.4 **[SV-2005]** lint rules distinguish it
- ✅ **always_latch** generic lowering — §1800-2009 9.2.2.3 **[SV-2005]** correct on transparent-latch pattern (probed)
- ✅ **join_any/join_none** — §1800-2009 9.3.2 **[SV-2005]** (sim_fork.rs)
- ✅ **wait fork** — §1800-2009 9.6.1 **[SV-2005]**
- ✅ **disable fork** — §1800-2009 9.6.3 **[SV-2005]**
- ✅ **final blocks at end of simulation** — §1800-2009 9.2.3 **[SV-2005]** (sim_final.rs) are captured as typed final processes independent of file extension and run ONCE after the scheduler exits ($finish, deadlock or no future events); they see values committed before simulation ended and `$time` reports the end-of-run time. Nonblocking assignments, task calls, deferred `$strobe`/`$monitor`, and timing controls (`#`/`@`/`wait`/fork) are clean codegen rejects because finals permit function statements only and no scheduled events execute afterward. `$finish` inside a final terminates that final immediately and skips all remaining finals
- 🟨 **Observe/reactive/preponed regions** — §1800-2009 4.4 **[SV-2005]** absent; single monitor region, no program/clocking contexts to trigger them
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
- 🟨 **Event control @** — §1364-2001 9.7.2/9.7.4 **[1995]** packed expression-change events, LSB edges, trigger-time `iff` and atomic mixed named-event lists work; array dependencies, function calls and evaluator captures of local/formal storage remain restricted (sim_events.rs, sim_partial_features.rs)
- ✅ **force** `force sig = expr;` whole signals, wakes waiters — §1364-2001 9.3.2 **[1995]** (sim_force.rs); ordinary single-driver nets retain this path, while resolved multidriver/wired-net force is explicitly unsupported
- ✅ **release** restores pre-force value — §1364-2001 9.3.2 **[1995]**
- 🟨 **force/release driver re-evaluation** — §1364-2001 9.3.2 **[1995]** drivers changed while forced are not re-evaluated (documented approximation)
- 🟨 **Condition event expressions** `@(a && b)` — §1364-2001 9.7.2 **[1995]** wait on body read set instead of condition operands
- 🟨 **Intra-assignment timing** `a = #5 b;` / `a <= #5 b;` — §1364-2001 9.7.7 **[1995]** packed/real/shortreal RHS values are captured immediately; blocking writes suspend, while constant/runtime-delay NBAs capture destinations and continue immediately. Selected NBAs preserve disjoint updates at commit; event/repeat forms remain unsupported (sim_delay.rs, sim_partial_features.rs)
- ❌ **Repeat event control** `repeat (n) @ev` — §1364-2001 9.7.7 **[1995]** clean codegen rejection
- 🟨 **Named events** `event ev; -> ev; @ ev;` — §1364-1995 §9.7.3 **[1995]** the runtime wakes all current waiters in registration order and mixed or-lists lower to one atomic wait; the Slang importer currently captures direct named-event declarations/references but does not retain the `->>` distinction, event arrays, or hierarchical event resolution, so those forms remain explicit semantic-import gaps
- ✅ **Procedural continuous assign/deassign** — §1364-2001 9.3.1 **[1995]** (sim_force.rs) `assign <reg> = expr;` lowers to a per-site enable-guarded process plus an immediate blocking write; `deassign` clears the enable only (the variable KEEPS its last value); RHS changes propagate while assigned and re-executing the same `assign` statement re-enables the site; while assigned, ordinary procedural writes to the target (blocking AND non-blocking) still take effect immediately, and the guard re-drives from the CURRENT rhs on its next wake (an RHS-read or enable change — it never wakes on changes of the target itself); sites are pre-scanned over every process body before any body lowers, so a `deassign` resolves its site regardless of process/source order; `force` keeps priority over an active PCA, `release` restores it. Clean rejects: net targets, selects/part-selects/array elements, hierarchical targets, real variables, and multiple active sites on one variable (deterministic static reject — reuse one site through control flow)

SystemVerilog era:

- ✅ **return** in functions/tasks — §1800-2009 12.8/13.4.1 **[SV-2005]**
- 🟨 **unique/priority/unique0 if & case** — §1800-2009 12.4.2/12.5.3 **[SV-2005]** lowered as plain case; no violation reports (probed)
- ✅ **case … inside** wildcard matching — §1800-2009 12.5.4 **[SV-2005]** scalar wildcard items, inclusive ranges, first-match/default behavior, and single selector evaluation (sim_wildcard_eq.rs, optimization on/off)
- ✅ **do-while** — §1800-2009 12.7.5 **[SV-2005]** post-test execution plus break/continue semantics (sim_disable.rs)
- 🟨 **foreach** — §1800-2009 12.7.3 **[SV-2005]** fixed unpacked arrays with an explicit iterator for every dimension, declared range direction, break/continue, and nested shadowing (sim_loops.rs); omitted indices, fork/deferred-output capture, and nonblocking writes to iterators remain rejected
- ✅ **break/continue** — §1800-2009 12.8 **[SV-2005]** (sim_disable.rs) for/while/repeat/forever; continue lands on the increment (for) or back-edge condition test, break exits the innermost loop; nesting pinned
- 🟨 **Inline loop-var declarations** `for (int i…)` — §1800-2009 12.7.1 **[SV-2005]** packed integral loop variables with lexical shadowing and break/continue (sim_loops.rs); real loop variables, fork/deferred-output capture, and nonblocking writes to loop locals are rejected

## 6. Timing controls

Verilog era:

- ✅ **#delay integer literal**, timescale-scaled — §1364-2001 9.7.1 **[1995]** (sim_timescale.rs)
- ✅ **@\* / @(\*) implicit sensitivity** from body read set — §1364-2001 9.7.5 **[2001]**
- 🟨 **Comb sensitivity to array elements** — §1364-2001 9.7.5 **[2001]** wakes on index signals only, not array writes
- 🟨 **Fractional delays** `#0.5` — §1364-2001 9.7.1 **[1995]** typed integer, real, scientific, and unit-suffixed values in statement and intra-assignment delays round once to the local time precision before conversion to design scheduler ticks; constant fractional continuous/gate delays use the same scaling with inertial scheduling; sub-ps scheduling remains unsupported
- 🟨 **Expression/parameter delays** `#(expr)` / `#P`, underscored `#10_000` and unit-suffixed `#5ns` literals — §1364-2001 9.7.1 **[1995]** constant and runtime packed/real expressions, function results and task inputs work in statement and intra-assignment delays. Each delay is evaluated once; real values round to local precision, X/Z means zero, and negative packed values convert to unsigned 64-bit time before checked scaling (sim_delay.rs, sim_time_literals.rs, sim_partial_features.rs). Negative/nonfinite real delays and values exceeding 64-bit scheduler ticks are rejected; sub-ps scheduling and unsupported expression forms remain gaps

SystemVerilog era:

- ⬜ **Cycle delays** `##N` — §1800-2009 14.11 **[SV-2005]** requires clocking blocks (tier)

## 7. Expressions & operators

Verilog era:

- 🟨 **Arithmetic** `+ - * / %` — §1364-2001 4.1.5 **[1995]** add/subtract/multiply/division/modulo preserve model-sized limbs; the generated backend rejects widths at its exclusive `1 << 20` capacity, while runtime constructors remain defensive
- 🟨 **Power** `**` — §1364-2001 4.1.5 **[2001]** model-sized operands are supported; backend capacity remains exclusive at `1 << 20`
- ✅ **Bitwise** `& | ^ ~ ^~` — §1364-2001 4.1.10 **[1995]**
- ✅ **Logical** `&& || !` — §1364-2001 4.1.9 **[1995]**
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
- 🟨 **Operators on real operands** `+ - * / %`, relational/logical/`?:`, casts — §1364-2001 4.1.1 **[1995]** within real subset; bitwise/reduction/shift/concat/case-eq on reals rejected

SystemVerilog era:

- 🟨 **Static casts** `int'(e)`, `signed'()`, `unsigned'()`, size casts `n'(e)` — §1800-2009 6.24.1 **[SV-2005]** Slang retains explicit/implicit conversion identity and resolved target width, signedness, and state domain. Scalar/vector typed and numeric size casts are covered through declaration, runtime-expression, and function contexts; unsupported aggregate/net paths remain outside this claim (sim_data_types.rs, sim_data_type_edges.rs, sim_data_types_next.rs)
- 🟨 **Increment/decrement** `++ --` — §1800-2009 11.4.2 **[SV-2005]** statement-position pre/post forms on whole scalar variables, including `for` increments, are supported; expression-valued and select/array-element forms remain unsupported (sim_operator_semantics.rs)
- 🟨 **Assignment operators** `+= -= *= /= %= &= |= ^= <<= >>= <<<= >>>=` — §1800-2009 11.4.1 **[SV-2005]** whole scalar variables are supported; select and array-element targets are cleanly rejected until LHS index evaluation can be preserved exactly once (sim_operator_semantics.rs)
- ✅ **Wildcard equality** `==? !=?` — §1800-2009 11.4.6 **[SV-2005]** RHS X/Z bits are wildcards; remaining LHS unknown bits yield X unless a known mismatch decides the result. Common-width/signed extension and model-sized operands are covered with optimization on/off (sim_wildcard_eq.rs).
- 🟨 **Set membership** `inside {…}` — §1800-2009 11.4.13 **[SV-2005]** scalar/range/wildcard cases are reported passing in both modes; aggregate and broader contextual forms remain outside the focused claim
- 🟨 **Streaming operators** `{<<{}}`, `{>>{}}` — §1800-2009 11.4.14 **[SV-2005]** focused packed RHS/LHS slice-order cases are reported passing in both modes; general aggregate streaming remains outside the focused claim
- ❌ **let expressions** — §1800-2009 11.13 **[SV-2009]**

## 8. Continuous assignments & structural

Verilog era:

- ✅ **Continuous assignment** `assign lhs = rhs;` — §1364-2001 6.1.2 **[1995]** comb process on RHS read set
- ✅ **Multiple/comma-form continuous assigns** — §1364-2001 6.1.2 **[1995]**
- 🟨 **Multiple drivers on one net** — §1364-2001 6.1 **[1995]** bounded ordinary standalone scalar `wire`/`tri` continuous-assignment drivers resolve with explicit strengths and high-Z endpoints; port/inout, vector-strength, gate, wired-net, and charge-storage contexts remain outside this claim
- 🟨 **Delay on continuous assign** `assign #d lhs = rhs;` — §1364-2001 6.1.3 **[1995]** whole packed drivers capture RHS values and schedule active-region inertial updates. Changed pending values cancel, unchanged results retain their original deadlines, and a return to the driver's current value cancels without replacement. Constant/parameter/fractional delays, 129-bit four-state vectors, independent drivers, scalar strengths, output-port initialization and zero-delay/NBA ordering are tested (`sim_delay.rs`, `sim_partial_features.rs`, both optimizer modes). Selected/array targets remain unsupported; separate rise/fall/turn-off delays are explicitly rejected with all expressions retained in the owned database
- 🟨 **Strength on continuous assign/gates** — §1364-2001 6.1.4/7.1.2 **[1995]** explicit strengths are reported passing for standalone scalar `wire`/`tri` continuous-assignment drivers in both modes; vector strengths are prohibited by §10.3.4, and gate/inout/wired-strength/trireg contexts remain unsupported
- ✅ **Logic gates** `and nand or nor xor xnor buf not` — §1364-2001 7.2–7.3 **[1995]** (sim_gates.rs) one comb process per gate, SensLoop over the input read set; n-input gates reduce left-to-right, nand/nor/xnor negate after the full reduce; vector gates are bitwise; the current backend requires equal terminal widths
- ✅ **Tri-state buffers** `bufif0 bufif1 notif0 notif1` — §1364-2001 7.4 Table 7-5 **[1995]** (sim_gates.rs) lowered to `sv4_mux(en, data|data, Z)` / `sv4_mux(en, Z, ~(data|data))` — the passing arm is z→x-normalized with `data|data` (per-bit), so an ENABLED gate turns a data-Z into X like buf/not while known bits pass unchanged; a DISABLED gate drives Z; unknown enable yields all-X unless both branches match
- ❌ **MOS/CMOS switches** `nmos pmos cmos rnmos rpmos rcmos` — §1364-2001 7.5–7.7 **[1995]** rejected with a clear message ("switch/transistor primitive … not supported")
- ❌ **Bidirectional switches** `tran tranif0 tranif1 rtran*` — §1364-2001 7.6 **[1995]** rejected with a clear message
- ✅ **pullup/pulldown** — §1364-2001 7.8 **[1995]** (sim_gates.rs) constant 1/0 driver process over the terminal width (RunOnce)
- ❌ **Strength modeling/resolution tables** — §1364-2001 7.9–7.13 **[1995]**
- 🟨 **Gate delays** `and #2 g(…)` / parameterized `#D` — §1364-2001 7.14 **[1995]** supported whole-signal gate terminals use captured active-region inertial updates with constant/parameter/fractional delays, X initialization and short-pulse cancellation. Enable-gate X/Z values and unchanged-result deadlines are tested (`sim_gates.rs`, `sim_partial_features.rs`, both optimizer modes). Separate rise/fall/turn-off delays are explicitly rejected; other terminal forms remain unsupported
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

- ✅ **Function declaration/return value/call in expressions** — §1364-2001 10.3 **[1995]** recursion depth guard 256 reports an error and returns the type's default beyond the guard (sim_function.rs pins within-limit recursion); static functions with output/inout formals in expression position explicitly reject until their persistent copy-out storage is supported
- ✅ **Tasks incl. output/inout args**; delay-bearing tasks inlined at call sites — §1364-2001 10.2 **[1995]** wait-bearing tasks inlined (sim_function.rs). Focused static local/formal persistence and static-task NBA copy-out cases pass; delay-free static task outputs/inouts retain storage across calls and copy out at return, including values from earlier NBAs. NBAs to automatic subroutine storage or aggregate/unpacked subprogram storage remain explicitly rejected instead of queuing dangling targets
- ✅ **automatic reentrant functions/tasks** — §1364-2001 10.2.3/10.3.1 **[2001]** recursion supported
- ✅ **Constant functions in parameter expressions** — §1364-2001 10.3.5 **[2001]** evaluated by elab Resolver; typed parameters required (probed)
- ❌ **Task calls inside function bodies** — §1364-2001 10.3.4 **[1995]** rejected
- ❌ **fork/join inside function/task bodies** — §1364-2001 9.8.2 **[1995]** rejected
- ❌ **Timing controls in function bodies** `#`/`@`/`wait` inside a function rejected — §1364-2001 10.3.4 **[1995]** nonblocking assignments are not policed: accepted and committed on the caller's NBA list (probed)
- ❌ **Recursive delay-bearing tasks** — §1364-2001 10.2.3 **[2001]** rejected
- ❌ **Cross-instance hierarchical subprogram calls** — §1364-2001 12.4 **[1995]** rejected

SystemVerilog era:

- ✅ **void functions** `function void f();` — §1800-2009 13.4.1 **[SV-2005]**
- ✅ **Default argument values** incl. references to earlier formals — §1800-2009 13.5.3 **[SV-2005]**
- ✅ **Named argument binding** `f(.b(x), .a(y))` order-independent — §1800-2009 13.5.4 **[SV-2005]** (probed)
- ❌ **ref arguments** pass-by-reference — §1800-2009 13.5.2 **[SV-2005]** pass-by-value only

## 10. System tasks & functions relevant to simulation

Verilog era — display family:

- ✅ **$display** — §1364-2001 17.1.1 **[1995]**
- ✅ **$write** — §1364-2001 17.1.1 **[1995]** same formatting as `$display`, without an appended newline (sim_monitor.rs)
- ✅ **Format specs** `%d/%h/%b/%o/%t` + width.precision; `%s` for `$display`/`$write` (not monitor/strobe) — §1364-2001 17.1.1.2 **[1995]** unknown specifiers/`%m` rejected
- ✅ **$strobe** observes settled values after active/inactive/NBA iteration, including zero-delay drivers triggered by NBAs — §1364-2001 17.1.2 **[1995]** (sim_monitor.rs, sim_partial_features.rs)
- 🟨 **$monitor/$monitoron/$monitoroff** one active monitor; change checks run after active/inactive/NBA settling — §1364-2001 17.1.3 **[1995]** (sim_monitor.rs). Initial registration still prints immediately, resume only prints changed values, and time-query arguments can spuriously trigger reports; those scheduling/trigger details remain gaps
- ❌ **b/o/h task variants** `$displayb/o/h $fwriteb…` — §1364-2001 17.1.1 **[1995]** base names only recognized

File IO:

- ❌ **Core file output** `$fopen/$fclose/$fdisplay/$fwrite/$fstrobe/$fmonitor` — §1364-2001 17.2.1–17.2.2 **[1995]** unsupported-task reject
- ❌ **$sformat/$swrite** — §1364-2001 17.2.3 **[2001]** unsupported-task reject
- ❌ **Formatted reads** `$fscanf/$sscanf/$fread/$fgets/$fgetc/$ungetc` — §1364-2001 17.2.4 **[2001]** unsupported-task reject
- ❌ **File positioning/status** `$ftell/$fseek/$rewind/$fflush/$ferror` — §1364-2001 17.2.5–17.2.7 **[2001]** unsupported-task reject

Memory load/store:

- ❌ **$readmemh/$readmemb** — §1364-2001 17.2.8 **[1995]** unsupported-task reject (probed)

Time:

- ✅ **$time** module-unit scaled, `%t` consumes it — §1364-2001 17.7.1 **[1995]**
- ✅ **$stime/$realtime** — §1364-2001 17.7.2–17.7.3 **[1995]** `$stime` returns the module-unit-scaled low 32 bits; `$realtime` preserves fractional module-unit time (sim_timescale.rs, sim_partial_features.rs)
- ✅ **$printtimescale** — §1364-2001 17.3.1 **[1995]**
- ❌ **$timeformat** — §1364-2001 17.3.2 **[1995]** unsupported-task reject

Control / misc:

- ✅ **$finish** exit code propagates to driver — §1364-2001 17.4.1 **[1995]**
- ❌ **$stop** — §1364-2001 17.4.2 **[1995]** unsupported-task reject (probed)
- ❌ **PLA modeling tasks** `$async$and$array …` — §1364-2001 17.5 **[1995]**
- ❌ **Stochastic tasks** `$q_initialize $q_add …` — §1364-2001 17.6 **[1995]**
- ❌ **$random** — §1364-2001 17.9.1 **[1995]** unsupported-function reject
- ❌ **$dist_uniform/$dist_normal/…** — §1364-2001 17.9.2 **[1995]** unsupported-function reject
- ✅ **Conversion** `$rtoi/$itor` — §1364-2001 17.8 **[1995]** truncation toward zero and signed/unsigned integral-to-real conversion, including implicit numeric argument coercion, typed parameters, and constant declaration initializers (sim_real_conversions.rs, optimization on/off)
- ✅ **Conversion** `$realtobits/$bitstoreal` — §1364-2001 17.8 **[2001]** IEEE-754 bit reinterpretation; `$bitstoreal` requires 64 bits and maps X/Z positions to zero (sim_real_conversions.rs, optimization on/off)
- ❌ **Plusargs** `$test$plusargs/$value$plusargs` — §1364-2001 17.10 **[1995]** unsupported-task reject

Waveforms:

- 🟨 **$dumpfile/$dumpvars/$dumpon/$dumpoff/$dumplimit** to VCD or FST — §1364-2001 ch18 **[1995]** `$dumpfile` selects the format by `.vcd`/`.fst`; packed signals, unpacked-array elements, real values, X/Z, aliases, hierarchy, timescale, dump activation, and size limits are emitted by a separate OS writer thread through a bounded lossless SPSC ring (sim_waveform.rs). `$dumpvars` currently warns and dumps all registered storage because depth/scope/variable filtering is not yet implemented; array elements use stable flattened linear indices.
- ✅ **$dumpall/$dumpflush** — §1364-2001 ch18 **[1995]** snapshots and synchronous flush barriers work for both VCD and FST (sim_waveform.rs; runtime self-test)
- ❌ **$dumpports extended VCD** — §1364-2001 18.3 **[2001]** unsupported-task reject

SystemVerilog era:

- ❌ **$writememh/$writememb** — §1800-2009 21.4 **[SV-2005]** unsupported-task reject
- ✅ **$clog2/$bits** — §1800-2009 20.8/20.6 **[SV-2005]** (`$clog2` first in [1364-2005])
- ✅ **Math functions** `$ln $log10 $exp $sqrt $pow $floor $ceil $sin …` — §1800-2009 20.8 **[SV-2005]** all 21 real functions map to table 20-4 C math functions, with runtime arguments, numeric coercion and C domain behavior (sim_partial_features.rs, both optimizer modes)
- ❌ **Severity tasks** `$fatal/$error/$warning/$info` — §1800-2009 20.9 **[SV-2005]** unsupported-task reject
- ❌ **$sformatf** — §1800-2009 21.3 **[SV-2005]** unsupported-function reject
- ✅ **Bit-vector helpers** `$onehot/$onehot0/$countones/$isunknown` — §1800-2009 20.6 **[SV-2005]** packed operands through the generated model width, X/Z-aware counting, parameters and constant declaration initializers, single argument evaluation, and combinational dependencies; real operands rejected (sim_bit_queries.rs, optimization on/off)
- ❌ **Sampled-value functions** `$rose/$fell/$stable/$past/$sampled` — §1800-2009 16.9.3 **[SV-2005]** unsupported-function reject
- ✅ **Shortreal conversion** `$bitstoshortreal/$shortrealtobits` — §1800-2009 20.5 **[SV-2005]** 32-bit IEEE-754 reinterpretation and shortreal rounding; `$bitstoshortreal` requires 32 bits and maps X/Z positions to zero (sim_real_conversions.rs, optimization on/off)
- ❌ **$system** — §1800-2009 **[SV-2009]** unsupported-task reject

## 11. Compiler directives affecting simulation

Verilog era:

- ✅ **`` `default_nettype ``** affects implicit net kinds at elaboration — §1364-2001 19.2 **[1995]**
- ✅ **`` `resetall ``** resets directive state — §1364-2001 19.6 **[1995]** consumed by frontend
- ✅ **`` `celldefine ``/`` `endcelldefine ``** cell tagging — §1364-2001 19.1 **[1995]** consumed by frontend, no sim effect
- ✅ **`` `unconnected_drive ``/`` `nounconnected_drive ``** — §1364-2001 19.9 **[1995]** consumed by frontend
- ✅ **`` `line ``** position override — §1364-2001 19.7 **[2001]** consumed by frontend

(`` `timescale `` is covered in §1.)

SystemVerilog era:

- ✅ **`` `begin_keywords ``/`` `end_keywords ``** keyword-set selection — §1800-2009 22.14 **[SV-2005]** (`begin_keywords` first in [1364-2005]) consumed by frontend
- ✅ **`` `pragma ``** — §1800-2009 22.11 **[SV-2009]** consumed by frontend
- ✅ **`` `undefineall ``** — §1800-2009 22.5.3 **[SV-2009]** consumed by frontend
- ✅ **`` `__FILE__ ``/`` `__LINE__ `` predefined macros** — §1800-2009 22.13 **[SV-2009]** consumed by frontend

## 12. Out-of-scope verification-infrastructure tier (⬜)

Tracked so nothing is lost; all de-prioritized behind RTL-simulation support.

| Area | Constructs | Reference | Tag |
|---|---|---|---|
| Classes | class/new/handles, properties/methods, inheritance, virtual methods, static/local, `::` | §1800-2009 ch8 | [SV-2005] |
| Programs | `program … endprogram`, race-elimination region, `$exit` | §1800-2009 ch24 | [SV-2005] |
| Clocking blocks | clocking decl/default/global, skews, synchronous drives, `##` delays | §1800-2009 ch14 | [SV-2005] |
| Interprocess sync | semaphores, mailboxes, process suspend/resume/kill, `.triggered`, `->>`, `wait_order` | §1800-2009 ch15 | [SV-2005] |
| Assertions | immediate/deferred/concurrent assert-assume-cover, sequences/properties, assertion control tasks | §1800-2009 ch16, 20.11 | [SV-2005] |
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

The original audit IDs are stable. This inventory currently contains 71 remaining
groups (38 missing, 33 partial); group 58 is completed. Counts refer to grouped
capabilities, not individual keywords, system functions or standard clauses.


| # | Status | Feature group | Missing forms or behavior |
|---:|---|---|---|
| 1 | Missing | Charge-storage nets | `trireg`, charge strengths, charge decay and charge sharing. |
| 2 | Missing | Tagged unions | Tagged storage, construction and matching. |
| 3 | Partial | Real types | General `real`/`realtime`/`shortreal` ports, arrays, subroutine storage, continuous/combinational use, event/wait controls, and real monitor/strobe arguments. |
| 4 | Partial | Strings | General string ports, formals, locals, static string-returning functions, continuous assignments and sensitivity. |
| 5 | Partial | Chandles | General chandle locals, output/inout or mixed signatures, ports and delay-bearing tasks; the implemented subset is null/copy/comparison and bounded input/return functions. |
| 6 | Partial | Structures and untagged unions | Nested unpacked/object members, recursive defaults and nominal type keys, anonymous copies without type identity, unequal-width unpacked unions, aggregate ports/nets and general subroutine storage. |
| 7 | Partial | Fixed unpacked arrays | Whole-array procedural assignment and concatenation, slices/partial indexing, general element types, multidimensional port forms and runtime-indexed port actuals. Element indexed part-selects now have file-based read/write, range, state-conversion, wide/invalid-index and delayed-NBA coverage. |
| 8 | Partial | Resizable containers and array methods | General nested/multidimensional or object elements, declaration initializers, keyed/default patterns, subroutine/port storage, reduction `with` clauses, locator methods (`find*`, `min/max`, `unique*`) and ordering methods (`sort/rsort/reverse/shuffle`). |
| 9 | Missing | Runtime enum methods | Enum `.first()`, `.last()`, `.next()`, `.prev()`, `.num()` and `.name()` have no general runtime lowering; `.next()` fails in a fresh probe. |
| 10 | Partial | Casts | Dynamic `$cast` and general aggregate/bit-stream conversion paths; scalar/vector static casts and the tested numeric conversions already work. |
| 11 | Partial | Data and array queries | Executed `$typename`, `$isunbounded`, `$left/$right/$low/$high/$increment/$size/$dimensions/$unpacked_dimensions`; some parameter-context queries are folded successfully by Slang. |
| 12 | Partial | Initialization and storage lifetimes | Nonconstant declaration initializers, runtime-dependent static subroutine initializers and opposite/ambiguous local lifetime overrides. |
| 13 | Partial | Net resolution and strengths | General gate/mixed-driver, port/inout/interface, wired/pull/supply-net, force and selected delayed-driver contexts. Standalone scalar wire/tri strength resolution already exists; explicit vector continuous-assignment strengths are language-illegal and are not counted. |
| 14 | Missing | Switch-level primitives | `nmos`, `pmos`, `cmos`, `rnmos`, `rpmos`, `rcmos`, `tran`, `tranif0/1`, `rtran`, `rtranif0/1`, including resistive strength propagation. |
| 15 | Missing | Primitive instance arrays | Arrays of gate and UDP instances. |
| 16 | Missing | User-defined primitives | Combinational and sequential UDP tables, state/edge behavior and instances. |
| 17 | Missing | Specify blocks | Specify parameters, module path delays, state/edge-dependent paths and pulse-control semantics including `PATHPULSE$`. |
| 18 | Missing | Timing checks | `$setup`, `$hold`, `$setuphold`, `$recovery`, `$removal`, `$recrem`, `$skew`, `$timeskew`, `$fullskew`, `$period`, `$width`, `$nochange`, notifiers and timing-check conditions. |
| 19 | Missing | SDF annotation | `$sdf_annotate` and application of annotated delays/checks. |
| 20 | Partial | Gate terminal forms | Selected, expression and hierarchical terminals; unequal terminal widths; multiple-output `buf`/`not`; more than 64 terminals. |
| 21 | Partial | Continuous and gate delays | Whole packed continuous/gate drivers now capture values, cancel pulses and commit in the active region without suspending evaluation. Delayed selected/array targets, separate rise/fall/turn-off delays, net-declaration propagation delays and unsupported gate terminal contexts remain. Multi-delay lists are retained and explicitly rejected rather than silently using their first expression. |
| 22 | Partial | Delay expressions | Negative real delays, sub-picosecond scheduling and unsupported expression forms remain. Runtime packed/real procedural delays, X/Z-to-zero delays, negative packed time conversion and local-precision rounding are tested in both optimizer modes; overflow is diagnosed. |
| 23 | Partial | Time precision and literal semantics | Sub-picosecond scheduling is clamped to 1 ps. Ordinary time-literal values follow the frontend's newer unrounded semantics rather than the targeted 2009 rounding semantics. |
| 24 | Partial | Port connections | Constants, input expressions, omitted defaults, selected output targets and whole packed-variable reference aliases now work. General aggregate/real/string/container ports, selected reference actuals and links involving resolved groups remain restricted. |
| 25 | Missing | Libraries and configurations | The standard library/configuration selection flow is not provided as a supported simulator feature. |
| 26 | Partial | Packages | Runtime package variables and package subroutine calls; parameter/type use via frontend elaboration already works. |
| 27 | Missing | Net aliases | `alias a = b` does not provide net aliasing: a fresh probe drives `a=1` but observes `b=z`. |
| 28 | Partial | SystemVerilog scheduling regions | Full preponed, observed, reactive and associated verification-region semantics; the existing active/inactive/NBA scheduler is narrower. |
| 29 | Partial | Zero-delay process behavior | Wait-free `always` is still approximated as combinational. Constant-false/unknown `wait` now remains suspended without preventing time advancement. |
| 30 | Partial | Conditional event controls | Packed expression changes, LSB edge semantics and trigger-time `iff` qualification now work, including mixed named events. Array dependencies and evaluator captures of procedural/subroutine locals or formals remain unsupported; function calls in evaluated event expressions/qualifiers are explicitly rejected pending reentrant effect handling. |
| 31 | Partial | Named-event references | Event arrays and hierarchical references are not generally projected; direct scalar named events already work. Advanced event operations are in item 65. |
| 32 | Partial | Array/container sensitivity | Combinational array reads do not reliably wake on element writes; resizable-container sensitivity/wait dependencies are unsupported. |
| 33 | Partial | Intra-assignment timing | Event/repeat timing forms and repeat event controls are rejected. Constant/runtime-delay NBAs capture values/destinations without suspending, preserve issue order and outlive their issuer. Selected NBAs merge into current storage at commit. Blocking packed/real/shortreal captures are tested. |
| 34 | Partial | Force and release | Driver re-evaluation after release and general selected, real, array and resolved-multidriver targets; the present release path restores a saved value. |
| 35 | Partial | Procedural assign/deassign | Driver priority is incomplete: ordinary procedural writes still affect an assigned variable. General selected/hierarchical/array/real targets and multiple assignment sites are unsupported. |
| 36 | Partial | Named disable | Cross-process disables, named-fork targets and some outer inlined-task targets. |
| 37 | Partial | Loop forms and captures | General container/omitted-index `foreach`, real loop locals, and legal fork/deferred-output captures of lexical loop variables. |
| 38 | Missing | Uniqueness and priority checking | Runtime violation checks for `unique`, `unique0` and `priority`; ordinary branch execution alone is implemented. |
| 39 | Missing | Pattern matching | General `case ... matches` and conditional pattern matching; the probed packed-struct case produced no required branch output. |
| 40 | Partial | Side-effecting operators | Expression-valued `++/--`, selected/array-element increment/decrement and selected/array-element compound assignments. |
| 41 | Partial | Set membership | General aggregate/contextual `inside` forms; tested scalar/range/wildcard forms work. |
| 42 | Partial | Streaming | General aggregate packing/unpacking and streaming `with` selectors; tested packed streaming works. |
| 43 | Missing | Reference subroutine arguments | True `ref`/`const ref` semantics; an automatic function's write through `ref` did not reach the caller in the probe. |
| 44 | Missing | Recursive timed tasks | Recursion through delay/wait-bearing tasks. |
| 45 | Missing | Parallel subroutine bodies | Fork constructs in task bodies and the permitted detached-process function forms; ordinary blocking timing in functions is illegal and is not a missing feature. |
| 46 | Missing | Cross-instance subroutine calls | Hierarchical calls to tasks/functions outside the calling instance, including interface/package subroutine contexts. |
| 47 | Partial | Subroutine copy-out and storage | Static functions with output/inout arguments used in expressions, general unpacked/aggregate subroutine storage and legal NBAs to persistent unpacked subroutine storage. NBAs to automatic variables are language-illegal and excluded. |
| 48 | Missing | File I/O | `$fopen/$fclose/$fdisplay/$fwrite/$fstrobe/$fmonitor`, `$fscanf/$sscanf/$fread/$fgets/$fgetc/$ungetc`, and `$ftell/$fseek/$rewind/$fflush/$ferror/$feof`. |
| 49 | Partial | Display families and formatting | `b/o/h` task variants, unsupported format specifiers such as `%m`, string/real formatting in monitor/strobe contexts, and monitor registration/resume/time-query trigger semantics. Strobe and change-driven monitor checks now wait for active/inactive/NBA settling. |
| 50 | Missing | String formatting tasks/functions | `$sformat`, `$swrite` and `$sformatf`, including their applicable base variants. |
| 51 | Missing | Memory file loading and writing | `$readmemh/$readmemb/$writememh/$writememb`. |
| 52 | Partial | Real-time reporting and time formatting | `$timeformat` remains missing. `$realtime` now returns fractional time in the calling module's units; `$time` and `$stime` already work. |
| 53 | Missing | Simulation suspension | `$stop`. |
| 54 | Missing | PLA modeling | Synchronous/asynchronous AND/NAND/OR/NOR array/plane system tasks. |
| 55 | Missing | Stochastic queues | `$q_initialize/$q_add/$q_remove/$q_full/$q_exam`. |
| 56 | Missing | Random-number facilities | `$random`, `$urandom`, `$urandom_range`, random-state/seeding methods, and `$dist_uniform/$dist_normal/$dist_exponential/$dist_poisson/$dist_chi_square/$dist_t/$dist_erlang`. |
| 57 | Missing | Command-line plusargs | `$test$plusargs/$value$plusargs`. |
| 58 | Completed | Runtime mathematical functions | All 21 real functions from IEEE 1800-2009 table 20-4 now use typed IR and the specified C math functions, with numeric argument conversion. Procedural tests cover runtime arguments, one-time evaluation and C domain behavior. Existing real-context restrictions are counted in group 3. |
| 59 | Missing | Runtime severity tasks | `$fatal/$error/$warning/$info`; elaboration-time frontend diagnostics are a separate capability. |
| 60 | Missing | Host command execution | `$system`. |
| 61 | Partial | Waveform selection and extended VCD | `$dumpvars` depth/scope/variable filtering and the `$dumpports` extended-VCD family; ordinary VCD/FST dumping works. |
| 62 | Missing | Classes | Class objects/handles, construction, properties, methods, inheritance, virtual dispatch and access/lifetime rules. |
| 63 | Missing | Program blocks | Program execution semantics, reactive scheduling and `$exit`. |
| 64 | Missing | Clocking blocks | Clocking declarations, default/global clocking, input/output skews, synchronous drives and `##` cycle delays. |
| 65 | Missing | Advanced interprocess synchronization | Semaphores, mailboxes, process handles/status/suspend/resume/kill/await, event `.triggered`, nonblocking `->>` and `wait_order`. |
| 66 | Missing | Assertions and sampled values | Immediate/deferred/concurrent assert/assume/cover, sequences/properties, `expect`, assertion-control tasks and sampled-value functions including `$sampled/$rose/$fell/$stable/$changed/$past` and the 2009 global-clocking forms. |
| 67 | Missing | Checkers | Checker declarations, instances and checker execution. |
| 68 | Missing | Functional coverage | Covergroups, coverpoints, bins, crosses, sampling, coverage queries/control and coverage database system tasks. |
| 69 | Missing | Constrained and structured randomization | `rand/randc`, constraints, object and `std::randomize`, `randcase` and `randsequence`. |
| 70 | Missing | DPI | DPI import/export of functions/tasks, context/pure semantics and foreign-call integration. |
| 71 | Missing | PLI/VPI | Standard `tf_`, `acc_`, `vpi_` interfaces, callbacks and PLI application integration. |
| 72 | Missing | Virtual interfaces | Virtual-interface handles, assignment and member access through those handles. |
