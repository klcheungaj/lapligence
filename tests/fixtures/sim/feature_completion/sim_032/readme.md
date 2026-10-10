# SIM-032 programs, reactive lifecycles and structural program bind

IEEE 1800-2009 §§4.4.2, 4.5, 9.2.3, 23.11 and 24.3-24.7 supply the oracles.
Every positive source runs through the public CLI in both optimizer modes on
the legacy, compact/portable and compact/GMP value backends
(`run_case_backend_parity`, or `run_case_checked_matrix` where the clauses
leave an order open). `bind_targets` also runs after snapshot/Db destruction at
native O0/O3. All `.out` files are hand-derived from the clauses and the
timelines below, never captured from llg. Times are printed with `%0d $time`
in the module's own time unit, so they equal the source delays. Line numbers
refer to `SystemVerilog-1800-2009.txt`, the `pdftotext -layout` extraction.

## Clauses relied on

- 4.4.2.6 L3184-3185: "The Reactive region holds the current reactive region
  set events being evaluated and can be processed in any order."
- 4.4.2.7 L3195-3197: "If events are being executed in the reactive region
  set, an explicit #0 delay control requires the process to be suspended and
  an event to be scheduled into the Re-Inactive region of the current time
  slot so that the process can be resumed in the next Re-Inactive to Reactive
  iteration."
- 4.4.2.8 L3202-3205: "The Re-NBA region holds the events to be evaluated
  after all the Re-Inactive events are processed. If events are being
  executed in the reactive region set, a nonblocking assignment creates an
  event in the Re-NBA update region scheduled for the current or a later
  simulation time."
- 4.5 L3386-3398 (reference algorithm): the Active..Post-Observed loop runs to
  empty, then "while (any region in [Reactive ... Post-Re-NBA] is nonempty)
  { execute_region (Reactive); R = first nonempty region in [Reactive ...
  Post-Re-NBA]; if (R is nonempty) move events in R to the Reactive region;
  }"; L3421-3422: "once the Reactive, Re-Inactive Pre-Re-NBA, Re-NBA, or
  Post-Re-NBA regions are processed, iteration over the other regions does
  not resume until these five regions are empty."
- 9.2.3 L11258-11261: "All final procedures shall execute in an arbitrary
  order. No remaining scheduled events shall execute after all final
  procedures have executed. A final procedure executes when simulation ends
  due to an explicit or implicit call to $finish."
- 23.11 L42888-42893: bind "is used to specify one or more instantiations of
  a module, interface, program, or checker without modifying the code of the
  target"; L42954 (`bind cpu fpu_props fpu_rules_1(a,b,c);`): "An
  instance named fpu_rules_1 is instantiated in every instance of module
  cpu."; L42982: "By binding a program to a module or an instance, the
  program becomes part of the bound object."
- 24.3 L43112-43113: "Program port declaration syntax and semantics are the
  same as those of modules (see 23.2.2)."; Syntax 24-1 L43147-43154:
  `non_port_program_item ::= continuous_assign | ... | initial_construct |
  final_construct | ... | program_generate_item`; note 5 L43172-43173: "It
  shall be illegal for a program_generate_item to include any item that would
  be illegal in a program_declaration outside a program_generate_item."
- 24.3 L43207-43209: "Program blocks can be nested within modules or
  interfaces. This allows multiple cooperating programs to share variables
  local to the scope. Nested programs with no ports or top-level programs that
  are not explicitly instantiated are implicitly instantiated once."
- 24.3 L43225-43226: "A program block may contain one or more initial or
  final procedures. It shall not contain always procedures, primitives, UDPs,
  or declarations or instances of modules, interfaces, or other programs."
- 24.3 L43228-43232: "When all initial procedures within a program have
  reached their end, that program shall immediately terminate all descendent
  threads of initial procedures within that program. If there is at least one
  initial procedure within at least one program block, the entire simulation
  shall terminate by means of an implicit call to the $finish system task
  immediately after all the threads and all their descendent threads
  originating from all initial procedures within all programs have ended."
- 24.3 L43242-43243: "References to program signals from outside any program
  block shall be an error. It shall be legal for hierarchical references to
  extend from one program scope to another program scope."
- 24.3.1 L43262-43265 and L43267: "The continuous assignment assign tclk = clk; would
  also be scheduled in the Reactive region. Likewise, initial procedures
  within program blocks are scheduled in the Reactive region. The standard #
  delay operator within program blocks schedules process resumption in the
  Reactive region."; "Nonblocking assignments in program code
  schedule their updates in the Re-NBA region."
- 24.3.1 L43277-43280: "Once a program process starts a thread of execution,
  all subsequent blocking statements in that thread are scheduled in the
  Reactive region. This includes subroutine code called by the thread, even if
  the subroutine code is declared in a module, package, or interface.
  Effectively, a section of sequential code anywhere in the design or
  testbench inherits the scheduling region of the thread that calls it."
- 24.3.2 L43296-43300: "Thus, variables on the other side of a program port
  connection are updated in the reactive region set. Similarly, the driving
  and resolution of nets on the other side of a program port connection also
  occurs in the reactive region set. ... Design processes sensitive to those
  cross-region variables and nets are scheduled for wake up in the active
  region set."
- 24.5 L43367-43371: "Calling program subroutines from within design modules
  is illegal and shall result in an error. ... When a task within a design
  module is called from a program, it shall use the reactive region set for
  its scheduling activities."
- 24.6 L43400-43404: programwide items are "accessible only to programs";
  "Anonymous programs can be used inside packages (see Clause 26) or
  compilation-unit scopes (see 3.12.1) to declare items that are part of the
  programwide space without declaring a new scope."
- 24.7 L43423-43427: "Calling $exit from a thread or its descendent thread
  originating in an initial procedure of a program block shall terminate all
  initial procedures and their descendent threads within that originating
  program block. Calling $exit from a thread or its descendent thread that
  does not originate in an initial procedure in a program shall be ignored".

## Positives

| Fixture | Criterion | Derivation |
| --- | --- | --- |
| `dual_regions` | A01 | Time 0, Active: the module initial schedules `m <= 1` (NBA), prints `A0 m=0 p=0`, and `#0` moves it to Inactive, which runs before NBA: `A1 m=0 p=0`. NBA commits `m = 1`. The Active set is empty, so the nested program's initial runs in Reactive: `R0 m=1 p=0`, schedules `p <= 1` in Re-NBA and `#0` goes to Re-Inactive, which is the first nonempty reactive region: `R1 m=1 p=0`; the second `#0` again lands in Re-Inactive, still before Re-NBA: `R2 m=1 p=0`. Re-NBA commits `p = 1` and wakes `always @(p)`, which runs only after the reactive set is empty (4.5): `A2 module saw p=1`. At 1: `R3 m=1 p=1`. The program ends, so the implicit `$finish` ends the run. |
| `task_origin` | A01 | 24.5's task `T` called from a module thread and from a program thread at time 5. Module thread (Active): `nb <= 1` is pending, so `T1 S1 nb=0 last=0 t=5`; `last <= 1` (NBA); `#0` resumes in Inactive before NBA: `T1 after #0 last=0`. NBA commits `nb = 1`, `last = 1`: `module saw last=1 t=5`. Program thread (Reactive, after the Active set): `T2 S1 nb=1 last=1 t=5`; `last <= 2` goes to Re-NBA; `#0` resumes in Re-Inactive first: `T2 after #0 last=1`. Re-NBA commits `last = 2`: `module saw last=2 t=5`. At 7 the module thread resumes in Active, then the program thread in Reactive: `T1 after #2 last=2 t=7`, `T2 after #2 last=2 t=7`, `program done t=7`. |
| `program_assign` | A01 | The 24.3.2 example plus a delayed and a net-declaration assignment in the program. `r = 0` at 0 gives `dw1 = 0`; the program's `assign pw2 = pw1` runs in Reactive and drives `dw2` from `z` to 0, which wakes the module's `always @(dw2)` in the next Active iteration: `dw2 is 0 t=0`. `py` follows `pw1` 2 units later (0 at 2), `pz = pn = ~pw1 = 1`: at 3 `y=0 z=1`. At 10 `r = 1`: `dw2 is 1 t=10`; `pz` becomes 0 at once and `py` becomes 1 at 12: at 11 `y=0 z=0`, at 13 `y=1 z=0`. The program has no initial, so only the module's `$finish` at 20 ends the run. |
| `program_generate` | — | A loop generate and an `if` generate inside a program (Syntax 24-1). The `if` block prints `cond N=3` at 0; block `i` prints at `i + 1`. All four initials are program initials, so the implicit `$finish` at 3 ends the run before the module's display at 10. |
| `interface_ports` | — | A modport port. Clock edges at 5, 15, 25. The module samples in Active before the program (Reactive) acts on the same edge: at 5 `valid=0 data=xx`; the program then drives `3c`/`1`; at 15 `valid=1 data=3c`, then `4d`; at 25 `valid=1 data=4d`, then `drv done t=25`, and the implicit `$finish`. |
| `program_scopes` | — | `a` imports `cfg` (`apply(4) = 12`) and declares a class whose timed task prints at 1: `box v=12 t=1`. `b` reads `tb.a0.shared` (program to program, legal) at 2; `a` reads `tb.b0.x` at 3; the nested program shares `tb.common` (24.3 L43207-43209) and prints at 4, after which every program has ended. |
| `anonymous_programs` | — | Items of a `$unit` anonymous program (`say`, `twice`) and of a package anonymous program (`scaled`, `wait_and_say`, class `Item`) called from a program: `unit task 42`, `scaled = 100 + 6 = 106`, `item = 5 + 100 = 105`, then `package task t=2 base=100`. |
| `exit_detached` | A02 | `pe` calls a module task that forks detached workers (they inherit `pe` as origin, 24.3.1). All three workers' `#0` increments run at 0 (`hits = 3`). Worker 1 prints at 1. `pe` exits at 10: its second initial (20) and worker 2 (50) are terminated, the line after `$exit` never runs. `po`'s worker 3 prints at 15 and `po` ends at 30, which finishes the run. The three final procedures (`pe final`, `po final`, `tb final t=30`) run once each, in an arbitrary order (9.2.3), so the test compares them as a set after the exact leading lines. |
| `exit_descendant` | A02 | A grandchild of `pe`'s initial calls `$exit` at 3; its sibling (8), its parent fork branch (9) and the initial itself (7) are terminated. `po` continues to 12, then the implicit `$finish`; the single final prints `final t=12`. |
| `exit_module_task` | A02 | `tb.quit` calls `$exit`. Called from the module initial at 0 it is ignored (24.7) and the module continues; called from `p` at 2 it terminates `p`; `q` prints at 4, then the implicit `$finish`; the final prints `final t=4`. |
| `implicit_finish` | A02 | The module clock never stops; posedges at 5, 15, 25 increment `n` in Active. The program prints at 25 in Reactive, after the third increment, and is the last program thread, so the implicit `$finish` runs: `final n=3 t=25`. |
| `multi_initial` | A02 | Two initials in one program. The first ends at 2; the module ticker prints at 3; the second ends at 4, which ends the program and terminates its detached child (10) and, being the last program, finishes the run (the ticker's 6 never prints). The program's final procedure sees `n = 2`. |
| `bind_targets` | A03 | `bind dut chk c(.v(v))` puts `c` into all three `dut` instances, `bind dut: tb.u0, tb.u2 tag t(...)` into two of them, and `bind u1 tag t1(...)` (inside `tb`) into `u1`. Each `v = x + 1` is the target's own signal (2, 3, 4), settled in Active before the program initials start in Reactive. `chk` prints at `v`, `tag` at `v + 10`, so the six lines are ordered by time. |
| `witness_program_bind` | A03 | FND-002 L-F03-09-03, adopted unchanged: a program bound into `tb` prints its port actual `7`. |
| `program_race` | A01 | Both programs run in Reactive after the module initial wrote `d = 5` in Active, so both print `d=5`. They race on `shared` in the same Reactive region (4.4.2.6 "any order"): the final value is the later writer's. The test accepts exactly the two permitted outputs (each program's line order matches its write order) and asserts no order between them. |

## Negatives

Each is rejected with a located error in both optimizer modes, before
simulation:

- `neg_always`, `neg_always_in_generate` (24.3 L43225-43226 and Syntax 24-1
  note 5), `neg_primitive`: "member not allowed in program declaration".
- `neg_module_instance`, `neg_interface_instance`, `neg_nested_program`
  (24.3 L43226): "cannot instantiate a module/an interface/a program in a
  program".
- `neg_bind_module_into_program` (23.11, 24.3 L43226): binding a module into a
  program target is a module instance in a program.
- `neg_module_calls_program_task` (24.5 L43367-43368),
  `neg_module_reads_program_var` (24.3 L43242),
  `neg_anonymous_from_module` (24.6 L43400-43401): "cannot reference program item
  from outside of a program".

## Not asserted

- When a program input port's value follows a design write made in the
  reactive region set: 24.3.2 states that variables and nets on the other
  side of a program port are updated in the reactive region set, which
  describes program outputs; it does not say in which region the program side
  of an input port follows a design variable written by another program. No
  fixture observes that instant.
- `program_natural`-style pending Re-NBA updates at the implicit `$finish` are
  llg policy, recorded as portable decision `S32-D1` in
  `docs/lrm_decisions.md`; programs bound into interfaces are decision
  `S32-D2`.
