# Simulator Feature Checklist — IEEE 1364 / IEEE 1800

The authoritative list of simulation features defined by IEEE Std 1364 (Verilog)
and IEEE Std 1800 (SystemVerilog) as they relate to the `llg` simulator,
each tagged with the revision that introduced it and ticked according to what
the simulator supports today. When a feature lands **and is tested**, flip its
marker here; do not tick from implementation alone. Priority order equals
document order: Verilog-core features first, SystemVerilog additions after.

Statuses were audited against `src/sim/codegen.rs`, `src/sim/AGENTS.md`,
`src/bin/llg.rs`, the regression suites in `tests/sim_*.rs`, and one-off
end-to-end runs of `llg` (marked **(probed)** below). Section numbers cite
`docs/spec-reference-verilog.md` (§1364-2001 x.y) and
`docs/spec-reference-sv.md` / `spec-reference-verification.md` (§1800-2009 y.z).

## Status markers

- ✅ supported — test anchor in parens; `(probed)` = verified by a manual
  `llg` run during an audit, no dedicated regression test yet; no
  parenthetical = **(suite-covered)**: exercised by one of the named suites
  during audits, without a single dedicated test
- 🟨 partial / approximated — one-line caveat
- ❌ not supported — rejected or unhandled (reason cited)
- ⬜ out of scope — verification-infrastructure tier, tracked but de-prioritized
- Revision tags: **[1995]** = IEEE 1364-1995 baseline · **[2001]** = added by
  1364-2001 · **[SV-2005]** = 1800-2005 SystemVerilog baseline ·
  **[SV-2009]** = added by 1800-2009 · **[1364-2005]** = IEEE 1364-2005
  interim revision, between our bands; used only as an inline annotation

Numeric limits (all designs): vectors ≤ `LLG_MAX_WIDTH` = 1024 bits;
division/modulo/power operands ≤ 64 bits; function/task recursion depth ≤ 256;
zero-delay loop guard = 10M scheduler passes; inout net drivers ≤ 16 per group;
force slots ≤ 64; processes ≤ 4096; named-event waiters ≤ 64 per event;
final blocks ≤ 1024.

## Summary counts

| § | Area | Verilog ✅ | Verilog 🟨 | Verilog ❌ | SV ✅ | SV 🟨 | SV ❌ |
|---|---|---:|---:|---:|---:|---:|---:|
| 1 | Lexical & preprocessing | 10 | 0 | 0 | 0 | 1 | 1 |
| 2 | Data types | 10 | 1 | 5 | 5 | 2 | 4 |
| 3 | Modules & hierarchy | 8 | 1 | 1 | 2 | 1 | 0 |
| 4 | Scheduling & processes | 8 | 1 | 0 | 7 | 1 | 1 |
| 5 | Procedural statements | 17 | 2 | 1 | 3 | 1 | 3 |
| 6 | Timing controls | 2 | 1 | 3 | 0 | 0 | 0 |
| 7 | Expressions & operators | 17 | 2 | 1 | 1 | 2 | 4 |
| 8 | Continuous assign & structural | 5 | 4 | 7 | 0 | 0 | 0 |
| 9 | Functions & tasks | 4 | 0 | 5 | 3 | 0 | 1 |
| 10 | System tasks & functions | 9 | 2 | 16 | 1 | 0 | 8 |
| 11 | Compiler directives affecting sim | 5 | 0 | 0 | 4 | 0 | 0 |
| — | **Total** | **95** | **14** | **39** | **26** | **8** | **22** |

In-section ⬜ items (not counted above): §3 configurations [V], ref ports /
default port values, extern/nested modules [SV] · §4 fine-grain process control
[SV] · §6 cycle delays [SV]. Section 12 lists the whole verification tier.

---

## 1. Lexical & preprocessing relevant to simulation

Verilog era:

- ✅ **Integer literals** `[size]'base value`, x/z digits, `_` separators `` `4'b1001` `` — §1364-2001 2.5.1 **[1995]**
- ✅ **Signed literals** `` `-8'd6`, `4'shf` `` — §1364-2001 2.5.1 **[2001]**
- ✅ **Real literals** `1.2`, `2.5e10` — §1364-2001 2.5.2 **[1995]**
- ✅ **String literals** `"..."` as display format strings — §1364-2001 2.6 **[1995]** (string *type* unsupported, see §2)
- ✅ **Lexical base** comments, identifiers, escaped identifiers, operators — §1364-2001 2.1–2.7 **[1995]** via Surelog frontend
- ✅ **Attributes** `(* full_case *)` parsed+ignored — §1364-2001 2.8 **[2001]** consumed by frontend, no sim effect
- ✅ **`` `define ``/`` `undef `` macros incl. arguments** — §1364-2001 19.3 **[1995]** expanded pre-elaboration
- ✅ **`` `ifdef `` family incl. `` `elsif ``/`` `ifndef ``** — §1364-2001 19.4 **[1995]** (`elsif`/`ifndef` are [2001])
- ✅ **`` `include ``** — §1364-2001 19.5 **[1995]** resolved by frontend
- ✅ **`` `timescale `` honored** scales `#N` and `$time` per file — §1364-2001 19.8 **[1995]** first directive/file only; missing → 1ns/1ps warning; fs clamps to 1ps (implemented, untested; sim_timescale.rs pins ns/ps scaling)

SystemVerilog era:

- 🟨 **Fill literals** `'0/'1/'x/'z` — §1800-2009 5.7.1 **[SV-2005]** honored only as entire RHS of assignment; in subexpressions act as 1-bit
- ❌ **Time literals** `2.1ns` — §1800-2009 5.8 **[SV-2005]** delays are integer ticks only

## 2. Data types

Verilog era:

- ✅ **reg scalars/vectors** `reg [7:0] r;` — §1364-2001 3.2.2/3.3 **[1995]** (sim_counter.rs)
- ✅ **signed net/reg declarations** `reg signed [3:0] s;` — §1364-2001 3.3/3.8 **[2001]** `%d` prints two's complement
- ✅ **integer variables** — §1364-2001 3.9 **[1995]**
- ✅ **time variables** 64-bit unsigned storage — §1364-2001 3.9 **[1995]** (sim_counter.rs)
- ✅ **wire/tri nets** — §1364-2001 3.7 **[1995]** tri resolution inside inout net groups (sim_inout.rs); plain tri behaves like wire
- ✅ **memories/unpacked arrays N-D** element bit/part selects, guarded OOB→X — §1364-2001 3.10 **[1995]** (sim_memory.rs); multi-dim slices & element indexed-part-selects rejected
- ✅ **Net declaration assignment** `wire w = expr;` — §1364-2001 3.6 **[1995]** behaves as a continuous driver for constant and dynamic RHS expressions, using the same event-driven run-once/sensitivity-loop IR as an explicit `assign` (sim_net_decl.rs); dynamic reads of unpacked arrays and unsupported resolved-net classes are rejected explicitly
- ✅ **Variable declaration initializers** scalar `reg x = 0;`, `logic l = 1'b0;`, `int x = P+1;` — §1364-2001 6.2.1 **[2001]** constant RHS only, non-constant rejected (sim_varinit.rs, sim_geninit.rs)
- ✅ **Parameters** override + propagation — §1364-2001 3.11.1 **[1995]** (elab_resolve.rs)
- ✅ **localparam** — §1364-2001 3.11.2 **[2001]**
- 🟨 **real/realtime** — §1364-2001 3.9 **[1995]** scalar procedural vars/params subset only (sim_real.rs)
- ❌ **wand/wor/triand/trior wired resolution** — §1364-2001 3.7 **[1995]** declared like plain wire; multi-driver last-write-wins (probed)
- ❌ **tri0/tri1/trireg/supply0/supply1 pull semantics** — §1364-2001 3.7 **[1995]** no resistive/pull modeling
- ❌ **drive strength / charge strength** — §1364-2001 3.4 **[1995]** not modeled anywhere
- ❌ **specparam** — §1364-2001 3.11.3 **[1995]** specify blocks unsupported
- ❌ **vectored/scalared hints** — §1364-2001 3.3 **[1995]** no dedicated handling

SystemVerilog era:

- 🟨 **logic/bit vectors** — §1800-2009 6.9/6.11 **[SV-2005]** `logic` has 4-state storage; `bit` currently shares that storage and therefore does not enforce 2-state initialization/conversion
- 🟨 **byte/shortint/int/longint** 2-state ints — §1800-2009 6.11 **[SV-2005]** widths/signedness lower, but storage currently remains 4-state instead of coercing X/Z to zero
- ✅ **uwire nets** fold as plain wire, no unique-resolution semantics modeled — §1800-2009 6.6 **[SV-2005]** (probed)
- ✅ **typedef simple/packed-vector aliases** — §1800-2009 6.18 **[SV-2005]** resolved by frontend (probed)
- ✅ **Array declaration initializers** `'{…}` patterns applied element-wise in linear-index order — §1800-2009 10.9.1 **[SV-2005]** constant elements only (sim_memory.rs)
- ✅ **enum-typed scalar variables** — §1800-2009 6.19 **[SV-2005]** stored at the elaborated packed base width; enum constants fold through the frontend (sim_operator_semantics.rs)
- ❌ **packed struct/union signals** — §1800-2009 7.2–7.3 **[SV-2005]** "unsupported typespec" reject (probed)
- ❌ **string type/signals/params** — §1800-2009 6.16 **[SV-2005]** rejected
- ✅ **event data type** scalar `event ev;` declarations — §1800-2009 6.17 **[SV-2005]** (sim_events.rs); event arrays rejected by the Surelog frontend (grammar cannot parse them)
- ❌ **dynamic arrays / associative arrays / queues** — §1800-2009 7.5/7.8/7.10 **[SV-2005]**
- ❌ **chandle** — §1800-2009 6.14 **[SV-2005]**

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
- 🟨 **Hierarchical select reads & writes** — §1364-2001 12.4 **[1995]** select reads stay whole-signal; writes need constant bounds recovered from source text
- ❌ **defparam** — §1364-2001 12.2.1 **[1995]** frontend drops override; model build fails (probed)
- ⬜ **Configurations/libraries** — §1364-2001 ch13 **[2001]** out of scope

SystemVerilog era:

- ✅ **Interfaces + modports** actuals, per-port copies, modport links; interface-body processes emitted on actual instance only — §1800-2009 25.3/25.5 **[SV-2005]** (sim_interface.rs, sim_interface_body.rs)
- 🟨 **Packages** params/types via frontend folding — §1800-2009 26 **[SV-2005]** package subprograms not lowered ("return type has no width", probed)
- ✅ **`.name` / `.*` connection shorthands** — §1800-2009 23.3.2.3–4 **[SV-2005]** expanded by the frontend and preserved through port-link lowering (sim_hier.rs)
- ⬜ **ref ports / default port values** — §1800-2009 23.2.2.2/23.2.2.4 **[SV-2005]** out of scope
- ⬜ **extern / nested modules** — §1800-2009 23.4–23.5 **[SV-2005]** out of scope

## 4. Scheduling & processes

Verilog era:

- ✅ **Stratified regions** active / inactive(`#0`) / NBA / re-eval loop until quiescent — §1364-2001 5.3–5.4 **[1995]** pinned by region_conformance.rs
- ✅ **initial** runs once then exits — §1364-2001 9.9.1 **[1995]**
- ✅ **always** loops forever — §1364-2001 9.9.2 **[1995]**
- ✅ **Continuous assigns as comb processes** re-evaluated on RHS read set — §1364-2001 6.1 **[1995]** Verilator-style semantics
- ✅ **fork/join parallel blocks** — §1364-2001 9.8.2 **[1995]** (sim_fork.rs)
- ✅ **Named forks/blocks** `fork : name … join` — §1364-2001 9.8.3 **[1995]**
- ✅ **wait(cond)** level-sensitive, re-evaluated on condition reads — §1364-2001 9.7.6 **[1995]** false constant spins until zero-delay guard trips (region_conformance.rs)
- 🟨 **Wait-free always treated as combinational** — §1364-2001 9.9.2 **[1995]** documented approximation; warns when it reads nothing
- ✅ **disable `<label>` / task early return** — §1364-2001 ch11 **[1995]** (sim_disable.rs) same-process only: enclosing named begin blocks and self-disabling loop blocks lower to goto-exit-label; disabling the current task/function is an early return (plain C-function tasks AND inlined wait-bearing expansions); disabling a named block that IS a loop body ends that block execution only (incr/condition still run — the Verilog-1995 `continue` idiom); cross-process disables, named forks and outer inlined tasks are clean codegen rejects. Surelog quirk: nested disables lose their resolved target (`vpiExpr` empty), recovered from source text against the enclosing scope chain

SystemVerilog era:

- ✅ **always_comb** sensitivity synthesized from body read set — §1800-2009 9.2.2.2 **[SV-2005]**
- ✅ **always_ff** lowered like always+event control — §1800-2009 9.2.2.4 **[SV-2005]** lint rules distinguish it
- ✅ **always_latch** generic lowering — §1800-2009 9.2.2.3 **[SV-2005]** correct on transparent-latch pattern (probed)
- ✅ **join_any/join_none** — §1800-2009 9.3.2 **[SV-2005]** (sim_fork.rs)
- ✅ **wait fork** — §1800-2009 9.6.1 **[SV-2005]**
- ✅ **disable fork** — §1800-2009 9.6.3 **[SV-2005]**
- ✅ **final blocks at end of simulation** — §1800-2009 9.2.3 **[SV-2005]** (sim_final.rs) run ONCE after the scheduler exits ($finish, deadlock or no future events); they see values committed before simulation ended and `$time` reports the end-of-run time. Nonblocking assignments, task calls, deferred `$strobe`/`$monitor`, and timing controls (`#`/`@`/`wait`/fork) are clean codegen rejects because finals permit function statements only and no scheduled events execute afterward. `$finish` inside a final terminates that final immediately and skips all remaining finals. Surelog parses `final` only in `.sv` files (frontend limitation)
- 🟨 **Observe/reactive/preponed regions** — §1800-2009 4.4 **[SV-2005]** absent; single monitor region, no program/clocking contexts to trigger them
- ❌ **$exit** program control task — §1800-2009 24.7 **[SV-2005]** unsupported system task
- ⬜ **Fine-grain process control** `process::self()` — §1800-2009 9.7 **[SV-2005]** class-based tier

## 5. Procedural statements

Verilog era:

- ✅ **Blocking assignment** whole/bit/part/indexed-part/array-element LHS — §1364-2001 9.2.1 **[1995]**
- ✅ **Nonblocking assignment** committed in NBA region — §1364-2001 9.2.2 **[1995]** array-element part RMW happens at record time
- ✅ **if/if-else-if** — §1364-2001 9.4 **[1995]**
- ✅ **case** — §1364-2001 9.5 **[1995]**
- ✅ **casez/casex wildcards** LRM 12.5.1 matching — §1364-2001 9.5.1 **[1995]** (sim_casez.rs)
- ✅ **Constant-expression case** `case(1)` priority style — §1364-2001 9.5.2 **[1995]**
- ✅ **Sequential/named blocks** `begin : name … end` — §1364-2001 9.8.1/9.8.3 **[1995]**
- ✅ **for** — §1364-2001 9.6 **[1995]**
- ✅ **while** — §1364-2001 9.6 **[1995]**
- ✅ **repeat** non-constant count runtime-evaluated — §1364-2001 9.6 **[1995]**
- ✅ **forever** — §1364-2001 9.6 **[1995]**
- ✅ **Event control @** edges/plain/or-lists/comma lists, atomic single wait — §1364-2001 9.7.2/9.7.4 **[1995]**
- ✅ **force** `force sig = expr;` whole signals, wakes waiters — §1364-2001 9.3.2 **[1995]** (sim_force.rs)
- ✅ **release** restores pre-force value — §1364-2001 9.3.2 **[1995]**
- 🟨 **force/release driver re-evaluation** — §1364-2001 9.3.2 **[1995]** drivers changed while forced are not re-evaluated (documented approximation)
- 🟨 **Condition event expressions** `@(a && b)` — §1364-2001 9.7.2 **[1995]** wait on body read set instead of condition operands
- ✅ **Intra-assignment timing** `a = #5 b;` / `a <= #5 b;` — §1364-2001 9.7.7 **[1995]** (sim_delay.rs) RHS evaluated immediately into a temp, LHS updated after the scaled delay; event/repeat forms rejected; the executing process suspends across the window for both kinds (v1 approximation)
- ❌ **Repeat event control** `repeat (n) @ev` — §1364-2001 9.7.7 **[1995]** clean codegen rejection
- ✅ **Named events** `event ev; -> ev; @ ev;` — §1364-1995 §9.7.3 **[1995]** (sim_events.rs, re-run with Surelog v1.87) trigger wakes ALL current waiters (registration order), edge-triggered (no latch); mixed or-lists `@(a or ev)` lower to ONE atomic wait (`llg_wait_mixed`); zero-delay trigger loops trip the runtime guard (sim_events.rs); 🟨 caveats: non-blocking `->>` lowers identically to `->` because v1.87 still reports `vpiBlocking=1` for both forms, and block-local event declarations behave as ordinary 1-bit logic vars; event arrays and hierarchical event references are rejected by the frontend before codegen
- ✅ **Procedural continuous assign/deassign** — §1364-2001 9.3.1 **[1995]** (sim_force.rs) `assign <reg> = expr;` lowers to a per-site enable-guarded process plus an immediate blocking write; `deassign` clears the enable only (the variable KEEPS its last value); RHS changes propagate while assigned and re-executing the same `assign` statement re-enables the site; while assigned, ordinary procedural writes to the target (blocking AND non-blocking) still take effect immediately, and the guard re-drives from the CURRENT rhs on its next wake (an RHS-read or enable change — it never wakes on changes of the target itself); sites are pre-scanned over every process body before any body lowers, so a `deassign` resolves its site regardless of process/source order; `force` keeps priority over an active PCA, `release` restores it. Clean rejects: net targets (variables only; Surelog models module-level `reg` as Net with net_type vpiReg, which counts as a variable), selects/part-selects/array elements, hierarchical targets, real variables (v1 scope), and multiple active sites on one variable (deterministic static reject — reuse one site through control flow)

SystemVerilog era:

- ✅ **return** in functions/tasks — §1800-2009 12.8/13.4.1 **[SV-2005]**
- 🟨 **unique/priority/unique0 if & case** — §1800-2009 12.4.2/12.5.3 **[SV-2005]** lowered as plain case; no violation reports (probed)
- ❌ **case … inside** wildcard matching — §1800-2009 12.5.4 **[SV-2005]** `==?` ops unsupported
- ✅ **do-while** — §1800-2009 12.7.5 **[SV-2005]** post-test execution plus break/continue semantics (sim_disable.rs)
- ❌ **foreach** — §1800-2009 12.7.3 **[SV-2005]** clean codegen rejection
- ✅ **break/continue** — §1800-2009 12.8 **[SV-2005]** (sim_disable.rs) for/while/repeat/forever; continue lands on the increment (for) or back-edge condition test, break exits the innermost loop; nesting pinned
- ❌ **Inline loop-var declarations** `for (int i…)` — §1800-2009 12.7.1 **[SV-2005]** loop var unresolved (probed)

## 6. Timing controls

Verilog era:

- ✅ **#delay integer literal**, timescale-scaled — §1364-2001 9.7.1 **[1995]** (sim_timescale.rs)
- ✅ **@\* / @(\*) implicit sensitivity** from body read set — §1364-2001 9.7.5 **[2001]**
- 🟨 **Comb sensitivity to array elements** — §1364-2001 9.7.5 **[2001]** wakes on index signals only, not array writes
- ❌ **Fractional delays** `#0.5` — §1364-2001 9.7.1 **[1995]** clean codegen reject ("cannot determine the `#delay` value"), not a silent `#0` (sim_delay.rs)
- ❌ **Expression/parameter delays** `#(expr)` / `#P`, underscored `#10_000` and unit-suffixed `#5ns` literals — §1364-2001 9.7.1 **[1995]** clean rejects; only plain integer-literal ticks are recovered from source text (sim_delay.rs)
- ❌ **min:typ:max delays** `#(1:2:3)` — §1364-2001 4.3 **[1995]**

SystemVerilog era:

- ⬜ **Cycle delays** `##N` — §1800-2009 14.11 **[SV-2005]** requires clocking blocks (tier)

## 7. Expressions & operators

Verilog era:

- ✅ **Arithmetic** `+ - * / %` — §1364-2001 4.1.5 **[1995]** div/mod operands >64 bits rejected up front
- 🟨 **Power** `**` — §1364-2001 4.1.5 **[2001]** ≤64-bit operands else rejected
- ✅ **Bitwise** `& | ^ ~ ^~` — §1364-2001 4.1.10 **[1995]**
- ✅ **Logical** `&& || !` — §1364-2001 4.1.9 **[1995]**
- ✅ **Reductions** `& ~& | ~| ^ ~^` — §1364-2001 4.1.11 **[1995]**
- ✅ **Shifts** `<< >>` — §1364-2001 4.1.12 **[1995]**
- ✅ **Arithmetic shifts** `<<< >>>` sign-fill — §1364-2001 4.1.12 **[2001]**
- ✅ **Relational** `< <= > >=` — §1364-2001 4.1.7 **[1995]**
- ✅ **Equality** `== != === !==` — §1364-2001 4.1.8 **[1995]** X/Z compared literally by `===`
- ✅ **Conditional** `?:` Z-carrying mux — §1364-2001 4.1.13 **[1995]**
- ✅ **Concatenation** `{}` reordered-concat respected — §1364-2001 4.1.14 **[1995]**
- ✅ **Replication** `{n{}}` — §1364-2001 4.1.14 **[1995]**
- ✅ **Bit-select/part-select operands** — §1364-2001 4.2.1 **[1995]**
- ✅ **Indexed part-select** `[+:w]` / `[-:w]` — §1364-2001 4.2.1 **[2001]**
- ✅ **Array addressing** `mem[i][j]` + element selects — §1364-2001 4.2.2 **[1995]**
- ❌ **Strings as operands** reg vectors holding 8-bit ASCII — §1364-2001 4.2.3 **[1995]** string constants in expressions rejected ("string constant in expression", probed); display format strings only
- ✅ **$signed/$unsigned** — §1364-2001 4.5 **[2001]**
- ✅ **Signedness/self-determined width rules** mirrored by runtime — §1364-2001 4.4–4.5 **[1995]** pinned by property_elab.rs
- ✅ **X/Z expression semantics** Z=X except identity/copy ops — §1364-2001 3.1 **[1995]** proptests + C vector table
- 🟨 **Operators on real operands** `+ - * / %`, relational/logical/`?:`, casts — §1364-2001 4.1.1 **[1995]** within real subset; bitwise/reduction/shift/concat/case-eq on reals rejected

SystemVerilog era:

- ✅ **Static casts** `int'(e)`, `signed'()`, `unsigned'()`, size casts `n'(e)` — §1800-2009 6.24.1 **[SV-2005]** (sim_counter.rs `sim_static_casts`, re-run with Surelog v1.87) value-preserving: widening extends by the SOURCE's signedness (`sv4_cast`/IR `Convert`; §10.7 assignment padding follows the RHS too, so `int'(8'hFF)`=255 and a signed RHS sign-extends into wider unsigned targets). v1.87 still omits `vpiSigned` on based constants, so codegen recovers the `'s` marker from the literal's source token; size-cast targets remain degraded to int(32) unsigned by the frontend
- 🟨 **Increment/decrement** `++ --` — §1800-2009 11.4.2 **[SV-2005]** statement-position pre/post forms on whole scalar variables, including `for` increments, are supported; expression-valued and select/array-element forms remain unsupported (sim_operator_semantics.rs)
- 🟨 **Assignment operators** `+= -= *= /= %= &= |= ^= <<= >>= <<<= >>>=` — §1800-2009 11.4.1 **[SV-2005]** whole scalar variables are supported; select and array-element targets are cleanly rejected until LHS index evaluation can be preserved exactly once (sim_operator_semantics.rs)
- ❌ **Wildcard equality** `==? !=?` — §1800-2009 11.4.6 **[SV-2005]**
- ❌ **Set membership** `inside {…}` — §1800-2009 11.4.13 **[SV-2005]**
- ❌ **Streaming operators** `{<<{}}`, `{>>{}}` — §1800-2009 11.4.14 **[SV-2005]**
- ❌ **let expressions** — §1800-2009 11.13 **[SV-2009]**

## 8. Continuous assignments & structural

Verilog era:

- ✅ **Continuous assignment** `assign lhs = rhs;` — §1364-2001 6.1.2 **[1995]** comb process on RHS read set
- ✅ **Multiple/comma-form continuous assigns** — §1364-2001 6.1.2 **[1995]**
- 🟨 **Multiple drivers on one net** — §1364-2001 6.1 **[1995]** no strength resolution; last write wins (probed)
- 🟨 **Delay on continuous assign** `assign #d lhs = rhs;` — §1364-2001 6.1.3 **[1995]** (sim_delay.rs) constant/parameter delays and t=0 wait; inertial pulse rejection is not implemented, tracked by an ignored `DELAY-BUG` conformance case
- ❌ **Strength on continuous assign/gates** — §1364-2001 6.1.4/7.1.2 **[1995]** nonzero drive-strength properties are rejected when the frontend exposes them; silent property loss was observed on v1.86 and remains unverified on v1.87
- ✅ **Logic gates** `and nand or nor xor xnor buf not` — §1364-2001 7.2–7.3 **[1995]** (sim_gates.rs) one comb process per gate, SensLoop over the input read set; n-input gates reduce left-to-right, nand/nor/xnor negate after the full reduce; vector gates are bitwise; v1 requires equal terminal widths
- ✅ **Tri-state buffers** `bufif0 bufif1 notif0 notif1` — §1364-2001 7.4 Table 7-5 **[1995]** (sim_gates.rs) lowered to `sv4_mux(en, data|data, Z)` / `sv4_mux(en, Z, ~(data|data))` — the passing arm is z→x-normalized with `data|data` (per-bit), so an ENABLED gate turns a data-Z into X like buf/not while known bits pass unchanged; a DISABLED gate drives Z; unknown enable yields all-X unless both branches match
- ❌ **MOS/CMOS switches** `nmos pmos cmos rnmos rpmos rcmos` — §1364-2001 7.5–7.7 **[1995]** rejected with a clear message ("switch/transistor primitive … not supported")
- ❌ **Bidirectional switches** `tran tranif0 tranif1 rtran*` — §1364-2001 7.6 **[1995]** rejected with a clear message
- ✅ **pullup/pulldown** — §1364-2001 7.8 **[1995]** (sim_gates.rs) constant 1/0 driver process over the terminal width (RunOnce)
- ❌ **Strength modeling/resolution tables** — §1364-2001 7.9–7.13 **[1995]**
- 🟨 **Gate delays** `and #2 g(…)` / parameterized `#D` — §1364-2001 7.14 **[1995]** (sim_gates.rs) constant/parameter delays only: the write happens D after each input change and the t=0 first evaluation waits too; v1 approximation: no pulse filtering (the delayed write uses the CURRENT input values), warned at codegen
- ❌ **Gate instance arrays** `and g[3:0] (…)` — §1364-2001 7.1 **[1995]** captured by the db walk and rejected with a clear message
- ❌ **Combinational UDPs** definition/table/instances — §1364-2001 8.1–8.2/8.6 **[1995]** instances captured and rejected with a clear message ("user-defined primitive instance … not supported")
- ❌ **Sequential UDPs** level/edge-sensitive — §1364-2001 8.3–8.5 **[1995]** same reject as combinational UDPs
- 🟨 **Gate terminal connections** — §1364-2001 7.1 **[1995]** whole plain signals only; select/expression terminals, hierarchical terminals, multi-output `buf`/`not`, and >64-terminal gates are clean rejects, and unequal terminal widths are rejected (a v1 requirement — mixed widths are legal Verilog)

(SystemVerilog era: 1800-2009 ch28–29 restore gates/UDPs verbatim — same
statuses as the rows above.)

## 9. Functions & tasks

Verilog era:

- ✅ **Function declaration/return value/call in expressions** — §1364-2001 10.3 **[1995]** recursion depth guard 256 returns all-X beyond (beyond-limit behavior implemented, untested; sim_function.rs pins within-limit recursion)
- ✅ **Tasks incl. output/inout args**; delay-bearing tasks inlined at call sites — §1364-2001 10.2 **[1995]** wait-bearing tasks inlined (sim_function.rs)
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
- ✅ **$strobe** post-NBA values once per time step — §1364-2001 17.1.2 **[1995]** (sim_monitor.rs)
- ✅ **$monitor/$monitoron/$monitoroff** change detect after NBA commit, single active monitor — §1364-2001 17.1.3 **[1995]** (sim_monitor.rs)
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
- 🟨 **$stime/$realtime** — §1364-2001 17.7.2–17.7.3 **[1995]** `$stime` returns the module-unit-scaled low 32 bits; `$realtime` remains unsupported (sim_timescale.rs)
- ✅ **$printtimescale** — §1364-2001 17.3.1 **[1995]**
- ❌ **$timeformat** — §1364-2001 17.3.2 **[1995]** unsupported-task reject

Control / misc:

- ✅ **$finish** exit code propagates to driver — §1364-2001 17.4.1 **[1995]**
- ❌ **$stop** — §1364-2001 17.4.2 **[1995]** unsupported-task reject (probed)
- ❌ **PLA modeling tasks** `$async$and$array …` — §1364-2001 17.5 **[1995]**
- ❌ **Stochastic tasks** `$q_initialize $q_add …` — §1364-2001 17.6 **[1995]**
- ❌ **$random** — §1364-2001 17.9.1 **[1995]** unsupported-function reject
- ❌ **$dist_uniform/$dist_normal/…** — §1364-2001 17.9.2 **[1995]** unsupported-function reject
- ❌ **Conversion** `$rtoi/$itor` — §1364-2001 17.8 **[1995]** unsupported-function reject
- ❌ **Conversion** `$realtobits/$bitstoreal` — §1364-2001 17.8 **[2001]** unsupported-function reject
- ❌ **Plusargs** `$test$plusargs/$value$plusargs` — §1364-2001 17.10 **[1995]** unsupported-task reject

Waveforms:

- 🟨 **$dumpfile/$dumpvars/$dumpon/$dumpoff/$dumplimit** to VCD or FST — §1364-2001 ch18 **[1995]** `$dumpfile` selects the format by `.vcd`/`.fst`; packed signals, unpacked-array elements, real values, X/Z, aliases, hierarchy, timescale, dump activation, and size limits are emitted by a separate OS writer thread through a bounded lossless SPSC ring (sim_waveform.rs). `$dumpvars` currently warns and dumps all registered storage because depth/scope/variable filtering is not yet implemented; array elements use stable flattened linear indices.
- ✅ **$dumpall/$dumpflush** — §1364-2001 ch18 **[1995]** snapshots and synchronous flush barriers work for both VCD and FST (sim_waveform.rs; runtime self-test)
- ❌ **$dumpports extended VCD** — §1364-2001 18.3 **[2001]** unsupported-task reject

SystemVerilog era:

- ❌ **$writememh/$writememb** — §1800-2009 21.4 **[SV-2005]** unsupported-task reject
- ✅ **$clog2/$bits** — §1800-2009 20.8/20.6 **[SV-2005]** (`$clog2` first in [1364-2005])
- ❌ **Math functions** `$ln $log10 $exp $sqrt $pow $floor $ceil $sin …` — §1800-2009 20.8 **[SV-2009]** unsupported-function reject
- ❌ **Severity tasks** `$fatal/$error/$warning/$info` — §1800-2009 20.9 **[SV-2005]** unsupported-task reject
- ❌ **$sformatf** — §1800-2009 21.3 **[SV-2005]** unsupported-function reject
- ❌ **Bit-vector helpers** `$onehot/$onehot0/$countones/$isunknown` — §1800-2009 20.6 **[SV-2005]** unsupported-function reject
- ❌ **Sampled-value functions** `$rose/$fell/$stable/$past/$sampled` — §1800-2009 16.9.3 **[SV-2005]** unsupported-function reject
- ❌ **Shortreal conversion** `$bitstoshortreal/$shortrealtobits` — §1800-2009 20.5 **[SV-2005]** unsupported-function reject
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

1. Land the feature in `src/sim/codegen.rs` (+ runtime/opt changes if needed).
2. Add or extend a regression test under `tests/` asserting exact behavior.
3. Flip this item's marker (🟨→✅, ❌→✅, …), replace `(probed)` with the test
   file name, and keep any caveat note accurate.
4. Update the counts in the Summary table for that row.
5. Mention the batch/PR that changed the marker in the commit message; this
   file is documentation-only — never commit simulator behavior without a test.
