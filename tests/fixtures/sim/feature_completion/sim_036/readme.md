# SIM-036 deferred immediate assertion contexts

IEEE 1800-2009 §§16.4, 20.11, 9.2.3, 9.2.2.2.1 and 4.4.2.6 supply the
oracles. Every positive source runs through the public CLI in both optimizer
modes on the legacy, compact/portable and compact/GMP value backends
(`run_case_backend_parity`; `function_two_processes` uses
`run_case_checked_matrix` and compares line sets because two processes'
reports share one Reactive region). All `.out` files are hand-derived from
the clauses and the timelines below, never captured from llg. Times are
printed with `%0d $time` in the module's own time unit, so they equal the
source delays. Line numbers refer to `SystemVerilog-1800-2009.txt`, the
`pdftotext -layout` extraction.

## Clauses relied on

- 16.4 L21220-21227: "The pass and fail statements in a deferred assertion's
  action_block, if present, shall each consist of a single subroutine call.
  The subroutine can be a task, task method, void function, void function
  method, or system task. The subroutine shall be scheduled in the Reactive
  region. A subroutine argument may be passed by value as an input or passed
  by reference as a ref or const ref. Actual argument expressions that are
  passed by value use the values of the underlying variables at the instant
  the deferred assertion expression was evaluated. Actual argument
  expressions that are passed by reference use or assign the current values
  of the underlying variables in the Reactive region. It shall be an error to
  pass automatic or dynamic variables as actuals to a ref or const ref
  formal."
- 16.4 L21227-21229: "The requirement of a single subroutine call implies that
  no begin-end construct shall surround the pass or fail statements, as begin
  is itself a statement which is not a subroutine call."
- 16.4.1 L21243-21250: "the action block subroutine call (or $error, if an
  assert or assume fails and no action_block is present) and the current
  values of its input arguments are placed in a deferred assertion report
  queue associated with the currently executing process. ... If a deferred
  assertion flush point (see 16.4.2) is reached in a process, its deferred
  assertion report queue is cleared. Any pending assertion reports will not
  be executed."
- 16.4.1 L21262-21266: "In the Observed region of each simulation time step,
  each pending assertion report that has not been flushed from its queue shall
  mature, or be confirmed for reporting. Once a report matures, it may no
  longer be flushed. Then the associated subroutine call ... is executed in
  the Reactive region".
- 16.4.2 L21276-21281: "A process is defined to have reached a deferred
  assertion flush point if any of the following occur: — The process, having
  been suspended earlier due to reaching an event control or wait statement,
  resumes execution. — The process was declared by an always_comb or
  always_latch, and its execution is resumed due to a transition on one of
  its dependent signals. — The outermost scope of the process is disabled by
  a disable statement (see 16.4.4)".
- 16.4.3 L21341-21342: "it is treated as if it were contained in an
  always_comb procedure."
- 16.4.4 L21359-21365: "A specific deferred assertion may be disabled. Any
  pending assertion reports for that assertion are cancelled. — When a
  disable is applied to the outermost scope of a procedure that has an active
  deferred assertion queue, in addition to normal disable activities (see
  9.6.2), the deferred assertion report queue is flushed and all pending
  assertion reports on the queue are cleared. Disabling a task or a
  non-outermost scope of a procedure does not cause flushing of any pending
  reports."
- 16.4.5 L21404-21407: "deferred assertions are inherently associated with the
  process in which they are executed. This means that a deferred assertion
  within a function may be executed several times due to the function being
  called by several different processes, and each of these different process
  executions is independent."
- 20.11 L35048-35057: "$assertoff ... In the case of a deferred assertion (see
  16.4), currently queued reports are not flushed and may still mature,
  though further checking is prevented until the $asserton. ... $assertkill
  ... also flushes any queued pending reports of deferred assertions (see
  16.4) ... that have not yet matured."
- 9.2.3 L11256-11261: the final procedure "executes in zero time, as a series
  of function calls from a single process. ... No remaining scheduled events
  shall execute after all final procedures have executed. A final procedure
  executes when simulation ends due to an explicit or implicit call to
  $finish."
- 4.4.2.6 L3184-3185: "The Reactive region holds the current reactive region
  set events being evaluated and can be processed in any order."
- 6.21 L7007-7008: "members or elements of dynamic variables—class
  properties and dynamically sized variables".

## Fixtures

| Fixture | Criterion | Expected and derivation |
| --- | --- | --- |
| `function_two_processes` | A01 | The 16.4.5 scenario. t0: `b1` and `b2` both fail; one report each (line set, any order). t1: `b1` fails with `x=5,y=6`, then `#0 y=5` re-triggers `b1` (always_comb flush point) and it passes; `b2` passes. t2: `b1` fails (`1` vs `5`) with no later flush; `b2` passes. Each report prints the function's automatic formals as they were when the assertion executed. |
| `subroutine_contexts` | A01 | t0: a static task (`a=3`; `v` is read inside the action task at Reactive time, after `v=2`), an automatic task whose local is changed after issue (`a=4 b=8`), and a class method whose property argument `lim` is copied at issue (`lim=5` although later set to 100). t1: two forked processes call `odd`; the first fails (`c=1`); the second fails with 3, resumes from `@(g)` in the same step (flush) and passes with 4. t2/t4: `always @(posedge clk)` calls `odd(count)`: 0 passes, 1 fails at t4. |
| `flush_points` | A02 | t0: three executions in one loop, no flush point: three reports (decision S36-D1). t1: a report survives `#0` (S36-D2). t2: reports issued before `wait (go)` and `@(ev)` are flushed when the process resumes in the same step. t3/t4: the 16.4.2 examples — the transient `a2` failure and the transient `c2` coverage are flushed by the re-triggered always_comb; t4 covers `a=0 b=1` once; t5 prints `glitches=0 hits=1`. |
| `disable_flush` | A02 | The 16.4.4 examples. t1: `a1` fails and is reported. t3: `a1` fails and `disable a1` cancels it. t4: `a3` fails and is reported. t5: `a3` fails in the Active region; `b3` disables `b2` (its outermost scope) from the Inactive region while `b2` waits at `@(c)`: flushed. t6: `disable inner` (not outermost) keeps the `a4` report. |
| `assert_control` | A02 | t0: `a1` queued before `$assertoff` matures; `a2` is not checked. t1: `$assertkill` flushes `a3`. t2: the Reactive action `killer` calls `$assertkill`; the matured `a5` still runs (S36-D5). t3: `a6` is not checked (killed); after `$asserton`, `a7` reports. |
| `native_arguments` | A02 | String (`issue`, `issue!`; the action reads the module string `s` directly as `later`), null chandle, real, packed struct, unpacked struct, class handle (the first object, `first/7`), 100-bit vector and fixed array arguments keep their issue-time values although all are changed afterwards; `ref`/`const ref` read `live=2` and `s=later` in the Reactive region; the method-call receiver is the first object (S36-D3). |
| `control_actions` | A03 | Each allowed action runs once in the Reactive region of t0, after the issuing process printed `issued n=0`: void function pass/cover/assume-fail actions (`n` = 1, 11, 1011), a task, `$display`, and `$info`/`$warning`/`$error` (stderr). t1: `$stop` reports and, under llg's non-interactive stop policy, the run continues to t2. |
| `finish_action` | A03 | `$finish` in a pass action runs after the issuing process printed `same step`; the final procedure follows; `BAD` never prints. |
| `fatal_action` | A03 | `$fatal` in a fail action ends the run in the Reactive region of t0 (`issued v=2` first; the message on stderr shows the issue-time `v=1`). |
| `final_block` | A03 | Decision S36-D4: the report of the final procedure's failing assertion executes after the procedure returns, with the issue-time `v=3`. |
| `neg_begin_action` | A03 | begin-end action: located frontend error. |
| `neg_output_formal`, `neg_inout_formal` | A03 | output/inout action formals: located frontend error. |
| `neg_automatic_ref`, `neg_class_property_ref` | A03 | automatic local and class property to `ref`: located frontend error. |
| `neg_dynamic_array_ref` | A03 | dynamic array element to `ref` (S36-D8): located llg error. |
| `neg_assert_final` | A03 | `assert final` is post-2009: edition error. |
