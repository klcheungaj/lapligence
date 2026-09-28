# Generated process ABI version 2

This document is the contract between the C emitter and the embedded scheduler.
It describes the stackless process ABI selected by `LLG_PROCESS_ABI_VERSION ==
2`. `llg_value` and its ownership ABI are independent of this scheduler ABI.

## Model marker and process entry

Every generated `model.c` starts its runtime includes with an explicit marker
and compile-time check:

```c
#define LLG_MODEL_PROCESS_ABI 2
#include "llg_rt.h"
#if LLG_MODEL_PROCESS_ABI != LLG_PROCESS_ABI_VERSION
#error "generated model process ABI does not match llg_rt.h"
#endif
```

The runtime header performs the same conditional check. Out-of-line `llg_co`
symbols retain their `..._abi1` link names, independently rejecting a stale
runtime archive.

A process, fork branch, assertion action, and every other suspendable generated
function has this entry type:

```c
llg_co_status_t fn(llg_co_frame_t* co, llg_co_chain_t* ch);
```

Generated code obtains the process record only at a use site with
`LLG_CO_OWNER(ch, llg_proc_t)`; it does not keep `self` in a C local. The
scheduler co-allocates a root frame after the process record and addresses it
as `LLG_CO_ROOT(&p->chain)`. The process chain is initialized from the
immutable descriptor passed to spawn. Recursive calls (D17) and callees forced
out of line by `LLG_CO_EMBED_LIMIT` (D19) allocate through `ch->arena`; there
is no generated frame `arena` field or `llg_proc_co_arena` lookup.

Each coroutine begins with a typed frame cast and dense dispatch:

```c
static llg_co_status_t p_top_initial(llg_co_frame_t* co,
                                     llg_co_chain_t* ch) {
    p_top_initial_frame_t* F = (p_top_initial_frame_t*)co;
    LLG_CO_DISPATCH_BEGIN(co)
    LLG_CO_RESUME_CASE(1)
    LLG_CO_RESUME_CASE(2)
    LLG_CO_DISPATCH_END(co)
    /* body */
}
```

The matching descriptor has a real entry pointer, never `NULL`:

```c
static const llg_co_site_t p_top_initial_sites[] = {
    {0},
    {NULL, 0, 0, "top.sv:4"},
};
static const llg_co_desc_t p_top_initial_desc = {
    p_top_initial, "top.initial", sizeof(p_top_initial_frame_t),
    p_top_initial_sites, 2, 0
};
```

## Generated-runtime mapping

The table covers process-ABI runtime entries emitted on the Phase 3 branch.
Entries not listed here keep their signatures and direct-call behavior.

| Old emitted entry | ABI 2 entry or pattern |
| --- | --- |
| `llg_spawn(&desc, fn, name)` | `llg_spawn(&desc, name)` |
| `llg_spawn_in_region(&desc, fn, name, region)` | `llg_spawn_in_region(&desc, name, region)`; ordinary, continuous/link, and Reactive assertion-action roots share this entry |
| `llg_spawn_program_in_region(&desc, fn, name, region, instance, initial)` | `llg_spawn_program_in_region(&desc, name, region, instance, initial)` |
| assertion registration `pass_action, &pass_desc, fail_action, &fail_desc` | pass/fail descriptors only; each descriptor's `fn` is the action entry |
| `llg_fork(&desc, fn, name, group)` | `llg_fork(&desc, name, group)` |
| `llg_fork_with_frame(&desc, fn, name, group, capture)` | `llg_fork_with_frame(&desc, name, group, capture)`; `llg_frame_t` remains the distinct retained capture object |
| `llg_spawn_final(fn, name)` where `fn(llg_proc_t*)` | `llg_spawn_final(fn, name)` where plain `fn(void)` is called directly |
| `llg_proc_co_frame(self)` | the coroutine receives `co`; cast it to its typed frame |
| `llg_proc_co_arena(self)` / per-frame `arena` | `ch->arena` through `LLG_CO_ARENA_ENTER` and `LLG_CO_CALL_ARENA` |
| `llg_proc_frame(self)` | unchanged service, with `LLG_CO_OWNER(ch, llg_proc_t)` at the generated use site |
| implicit/current-process lookup | `llg_current()` returns `g.current` only during a resume or transient final; coroutine arms and process control receive explicit `self` |
| `llg_proc_done(self)` | unwind generated lexical scopes, then `return LLG_CO_DONE` |
| `llg_wait_time` | `LLG_CO_AWAIT(..., llg_arm_time(self, ...))` |
| `llg_wait_any` | `llg_arm_any(self, ...)` |
| `llg_wait_any_dependencies` | `llg_arm_any_dependencies(self, ...)` |
| `llg_wait_any_events` | `llg_arm_any_events(self, ...)` |
| `llg_wait_edge` | `llg_arm_edge(self, ...)` |
| `llg_wait_level` | `llg_arm_level(self, ...)` |
| `llg_wait_event` / `llg_wait_events` | `llg_arm_event(self, ...)` / `llg_arm_events(self, ...)` |
| `llg_wait_event_triggered` | `llg_arm_event_triggered(self, ...)` |
| `llg_wait_assertion` | `llg_arm_assertion(self, ...)` |
| `llg_wait_order(..., int* result)` | `llg_arm_order(self, ..., result)`; `result` is a frame field |
| `llg_wait_mixed` | `llg_arm_mixed(self, ...)` |
| `llg_wait_expressions` | `llg_arm_expressions(self, ...)` |
| `llg_wait_clocking_cycles` | generated frame-held `uint64_t` count plus `llg_arm_clocking_cycle` as described below |
| `llg_join` / `llg_wait_fork` | `llg_arm_join(self, ...)` / `llg_arm_wait_fork(self)` |
| `llg_process_suspend(handle)` | `llg_arm_process_suspend(self, handle)` |
| `llg_process_await(handle)` | `llg_arm_process_await(self, handle)` |
| `llg_semaphore_get` | `llg_arm_semaphore_get(self, ...)` |
| blocking `llg_mailbox_put_value` | `llg_arm_mailbox_put_value(self, ...)` |
| blocking `llg_mailbox_get_value` / peek | `llg_arm_mailbox_get_value(self, ..., peek)` |
| `llg_rt_stop_with_level` | `llg_arm_stop(self, verbosity, location)`; `llg_rt_stop` remains a non-suspending VPI-facing service |
| `llg_process_self()` | `llg_process_self(self)` |
| `llg_process_kill(handle)` / `llg_process_resume(handle)` | `llg_process_kill(self, handle)` / `llg_process_resume(self, handle)` |
| `llg_disable_fork()` | `llg_disable_fork(self)` |
| `llg_disable_target(decl, instance)` | `llg_disable_target(self, decl, instance)` |
| `llg_program_exit()` | `llg_program_exit(self)` |
| `_Noreturn llg_rt_finish*` / `llg_rt_fatal_typed` | returning calls followed immediately by the applicable exit check |
| `void llg_budget_point(location)` | `int llg_budget_point(location)`; nonzero is an already-recorded ABANDON exit |
| `llg_rt_init_with_args_precision_and_stack(...)` | `llg_rt_init_with_args_and_precision(argc, argv, precision_fs)` |
| `LLG_MODEL_STACK_VALUES` | removed; explicit frames and the chain arena own coroutine storage |

`llg_wait_resume_in_region`, nonblocking semaphore/mailbox operations,
process-handle retain/release/assignment, fork-group construction, capture
frame services, and direct scheduler/value services are unchanged. Generated
calls that can terminate still acquire the `Terminate` effect even when their
C signature is unchanged (notably assertion control and VPI calls).

## Arm protocol

An arm validates and either completes synchronously or leaves exactly one
registered waiter:

- `LLG_CO_ARM_READY`: continue in the same scheduler turn. No process may
  interleave and pending `join_none` children remain pending.
- `LLG_CO_ARM_SUSPEND`: the waiter and all delivery ownership are installed,
  and `start_pending_fork_children` has run. `LLG_CO_AWAIT` records the resume
  state and returns `LLG_CO_PENDING`.
- `LLG_CO_ARM_EXIT`: termination bookkeeping and `chain.exiting` are already
  complete. `LLG_CO_AWAIT` returns `LLG_CO_EXIT`.

All ABI 2 runtime arms are one-shot and use `LLG_CO_AWAIT`. Semaphore keys,
mailbox values, process completion, and `wait_order` results are delivered by
the runtime before wake, so re-arming would duplicate the operation. The
library's `LLG_CO_AWAIT_RETRY` remains available for a future arm that cannot
deliver on wake, but no current emitted operation uses it. Its required shape
would be:

```c
LLG_CO_AWAIT_RETRY(co, ch, 7,
    retry_arm(LLG_CO_OWNER(ch, llg_proc_t), F->persistent_state));
```

Every value read by a retry expression must be a global or frame field.

The per-arm outcomes are:

| Arm (old entry) | READY | SUSPEND and delivery | EXIT |
| --- | --- | --- | --- |
| `llg_arm_time` (`llg_wait_time`) | invalid/no current/read-only no-op | time or zero-delay waiter registered | none |
| `llg_arm_any`, `llg_arm_any_dependencies`, `llg_arm_any_events`, `llg_arm_edge`, `llg_arm_level` (matching `llg_wait_*`) | invalid/no-current/read-only no-op | copied signal/dependency specification wakes once | none |
| `llg_arm_event`, `llg_arm_events` (`llg_wait_event[s]`) | empty/invalid no-op | copied atomic event list wakes once | none |
| `llg_arm_event_triggered` (`llg_wait_event_triggered`) | already triggered or invalid | persistent-trigger waiter wakes once | none |
| `llg_arm_assertion` (`llg_wait_assertion`) | invalid/no-current no-op | assertion result wakes in Reactive | none |
| `llg_arm_order` (`llg_wait_order`) | invalid/empty request; result remains zero | runtime writes frame `result` to 1 or -1 before wake | none |
| `llg_arm_expressions` (`llg_wait_expressions`) | invalid/no-current path after releasing transferred contexts | copied evaluators/dependencies wake only for a qualifying expression | terminating evaluator effects set the current exit |
| `llg_arm_mixed` (`llg_wait_mixed`) | invalid/empty no-op | one atomic copied signal/event registration | none |
| `llg_arm_clocking_cycle` (`llg_wait_clocking_cycles`) | allowed current-slot event for `##0`, or invalid | one future mixed-source event | none |
| `llg_arm_join` (`llg_join`) | empty, `join_none`, or satisfied `join_any` | selected group condition wakes once | none |
| `llg_arm_wait_fork` (`llg_wait_fork`) | no live groups | last live group wakes once | none |
| `llg_arm_process_suspend` (`llg_process_suspend`) | invalid/terminal/already suspended, or another target was suspended | target resolves to `self`; resume requeues this continuation | termination discovered during control bookkeeping |
| `llg_arm_process_await` (`llg_process_await`) | null, self, or terminal handle | target terminal transition wakes once | none |
| `llg_arm_semaphore_get` (`llg_semaphore_get`) | keys granted immediately or invalid/no-op | FIFO head receives its keys before wake | none |
| `llg_arm_mailbox_put_value` (`llg_mailbox_put_value`) | value transferred/queued immediately; the arm consumes it | bounded FIFO owns the value until it is accepted or cancelled | type/null failure sets COMPLETE |
| `llg_arm_mailbox_get_value` (`llg_mailbox_get_value`) | get/peek delivered immediately | runtime retains the target and writes it before wake | blocking type/null failure sets COMPLETE without consuming or assigning |
| `llg_arm_stop` (`llg_rt_stop_with_level`) | none for a valid generated call | continuation and queues remain live until stop resume | invalid stop context sets controlled failure and COMPLETE |

The inventory's private `llg_mailbox_wait_get` yield is folded into
`llg_arm_mailbox_get_value`; it is not a second generated ABI entry.

A normal wait is:

```c
LLG_CO_AWAIT(co, ch, 1,
    llg_arm_time(LLG_CO_OWNER(ch, llg_proc_t), F->ticks));
```

Descriptor arrays may be compound literals only where the selected arm copies
them before returning. An address retained on SUSPEND—`wait_order`'s `int*`, a
mailbox target, or a process-handle local—must instead name a frame field or a
registered stable cell.

Clocking cycles deliberately keep their count in generated state. After
evaluating and discarding the four-state count operand, emit the equivalent of:

```c
F->clock_remaining = llg_repeat_count(F->clock_count);
if (F->clock_remaining == 0) {
    LLG_CO_AWAIT(co, ch, 3,
        llg_arm_clocking_cycle(LLG_CO_OWNER(ch, llg_proc_t),
                              F->clock_sources, F->clock_source_count, 1));
} else {
    while (F->clock_remaining != 0) {
        LLG_CO_AWAIT(co, ch, 4,
            llg_arm_clocking_cycle(LLG_CO_OWNER(ch, llg_proc_t),
                                  F->clock_sources,
                                  F->clock_source_count, 0));
        --F->clock_remaining;
    }
}
```

`accept_current == 1` implements `##0`: READY if a source already fired in
the current slot, otherwise one SUSPEND. Positive counts always wait for
future events. The runtime copies the source array at each suspended cycle.

Other synchronization forms are ordinary awaits:

```c
LLG_CO_AWAIT(co, ch, 5,
    llg_arm_join(LLG_CO_OWNER(ch, llg_proc_t), F->group));
LLG_CO_AWAIT(co, ch, 6,
    llg_arm_stop(LLG_CO_OWNER(ch, llg_proc_t), F->verbosity, F->location));
LLG_CO_AWAIT(co, ch, 7,
    llg_arm_process_suspend(LLG_CO_OWNER(ch, llg_proc_t), F->target));
```

Suspending another process is a READY result for the caller; suspending self
is SUSPEND. Awaiting a null, terminal, or self handle is READY.

## Suspendable calls

Arguments and copy-out addresses are stored in the callee frame before entry.
A polled static call uses caller-owned embedded storage:

```c
F->calls.c0.arg = F->arg;
LLG_CO_CALL(co, ch, 8, fn_task, &F->calls.c0.co);
```

An anchored static call stores the same frame after its site-owned prefix:

```c
F->calls.a1.f.arg = F->arg;
LLG_CO_CALL_ANCHOR(co, ch, 9, &fn_task_desc, &F->calls.a1.an);
```

Recursive SCC edges (D17) and frames forced out by the size decision (D19)
use the chain arena. The anchor pointer is a frame field so it survives entry:

```c
LLG_CO_ARENA_ENTER(ch, &fn_task_desc, F->arena_call);
((fn_task_frame_t*)LLG_CO_ANCHOR_FRAME(F->arena_call))->arg = F->arg;
LLG_CO_CALL_ARENA(co, ch, 10, &fn_task_desc, F->arena_call);
```

The call macro pops the arena allocation only after completion. `CALL`, `CALLED`, child
return, and READY never visit the scheduler or release `join_none` children.

## Termination and process end

`chain.exiting` has two nonzero values:

| Exit kind | Set by | Scheduler action |
| --- | --- | --- |
| `LLG_EXIT_COMPLETE` | finish/fatal; blocking mailbox type/null error; process or assertion-control kill when program completion requests finish; terminating VPI request | run the former `llg_proc_done` completion sequence |
| `LLG_EXIT_ABANDON` | self/ancestor process kill; disable reaching self; program exit; budget abort; assertion-control kill of current | do not complete twice; cancellation bookkeeping already ran |

`llg_rt_request_finish` is the returning foreign-callback path and sets
COMPLETE when a process is current. `vpi_control(vpiStop)` cannot preserve a
foreign C continuation: `llg_rt_stop` therefore returns a controlled failure
instead of suspending. Generated `$stop` always uses `llg_arm_stop`.

After every operation with the `Terminate` effect, a coroutine checks before
executing another HDL statement:

```c
llg_rt_finish_with_level(F->verbosity, F->location);
LLG_CO_EXIT_CHECK(ch);
```

Process control follows the same rule:

```c
llg_process_kill(LLG_CO_OWNER(ch, llg_proc_t), F->target);
LLG_CO_EXIT_CHECK(ch);
```

Plain generated functions have no chain parameter and use their common unwind
label after every Terminate operation and terminating call:

```c
llg_rt_finish();
if (LLG_CO_UNLIKELY(llg_rt_exiting())) goto _llg_return;
```

Loop back-edges check the cooperative budget. Coroutine and plain-function
forms are respectively:

```c
if (LLG_CO_UNLIKELY(llg_budget_point(location))) return LLG_CO_EXIT;
if (LLG_CO_UNLIKELY(llg_budget_point(location))) goto _llg_return;
```

Nonzero means the diagnostic, failure flag, scheduler finish request, and
ABANDON exit are already recorded. Callers of a plain suspendable callee also
propagate termination; `LLG_CO_CALL*` performs the chain check when a callee
returns DONE.

Natural process completion performs existing generated scope unwinding and
then returns; it does not call a runtime completion function:

```c
_llg_return:
llg_value_scopes_end_since(F->_llg_frame_base);
return LLG_CO_DONE;
```

The scheduler maps DONE and EXIT/COMPLETE to its internal completion routine.
EXIT/ABANDON only reaches safe reclamation.

## Spawn sites, finals, and model lifecycle

All coroutine spawn sites pass only a descriptor:

```c
llg_spawn(&p_top_initial_desc, "top.initial");
llg_spawn_in_region(&p_link_desc, "top.link", LLG_REGION_ACTIVE);
llg_spawn_program_in_region(&p_program_desc, "pgm.initial",
                            LLG_REGION_REACTIVE, instance, 1);
llg_fork(&p_branch_desc, "branch", F->group);
```

Concurrent-assertion registration passes nullable action descriptors rather
than a redundant action function plus descriptor. Runtime action spawning is
Reactive and otherwise follows `llg_spawn_in_region`.

Finals cannot suspend and remain plain calls:

```c
static void f_top_final(void) {
    /* plain body; Terminate operations use llg_rt_exiting + _llg_return */
_llg_return:
    return;
}
llg_spawn_final(f_top_final, "top.final");
```

The final runner sets `g.current` to a transient record around each call, then
clears it. This gives `$time`, process-owned services, finish/fatal, and
`llg_rt_exiting()` their normal current-process semantics without allocating a
coroutine. A finish in a final stops that final at its check and suppresses
later finals according to the existing policy.

Generated startup uses only:

```c
llg_rt_init_with_args_and_precision(argc, argv, LLG_MODEL_PRECISION_FS);
```

There is no stack-values macro or stack-sized init entry.

## Ownership and scheduling invariants

- No C local, parameter copy, loop index, scope mark, handle, result slot, or
  native scalar is live across a resume point. Such state is a frame field.
  Macro temporaries and copied arm compound literals are the only exceptions.
- Frames are POD. The runtime owns the co-allocated process/root frame; callers
  own embedded polled and anchored callee storage; `ch->arena` owns dynamic,
  recursive, and oversized callee storage. Registered value scopes remain the
  sole payload owners.
- Cancellation never resumes coroutine code. It unlinks waits and groups,
  drains value/reference scopes and activations, releases the chain arena, and
  defers the current record's free until `llg_co_run` returns.
- `start_pending_fork_children` runs immediately before every arm returns
  SUSPEND and during natural completion. It never runs for READY, CALL, or a
  child return.
- The scheduler sets `g.current` immediately around each `llg_co_run` and each
  final call. `llg_current()` returns that pointer and is otherwise NULL. It
  is a single-thread runtime field, not TLS or `llg_co` state.
- Wait descriptor arrays are copied before SUSPEND. Runtime-delivery pointers
  and capture objects outlive the wait. A creator may release its capture
  reference after spawn; the child retains until completion/cancellation.
- Process handles retain terminal identity independently of coroutine
  storage. Completed parents retained for detached descendants also retain
  their co-allocated root frames.
- Descriptors and names are immutable and outlive every process using them.
- `llg_co` contains no globals or TLS. Arms and process-control calls receive
  `llg_proc_t* self`; other services remain on the MT-0 single-thread context.
- `llg_value` stays scheduler-, waveform-, and coroutine-independent.

## Removed interfaces

ABI 2 removes `llg_proc_done`, `llg_proc_co_frame`, `llg_proc_co_arena`,
`llg_libaco_desc`, `llg_rt_init_with_stack`,
`llg_rt_init_with_args_precision_and_stack`, `llg_coroutine_stack_size`, the
private `llg_stack_values`, `LLG_DEFAULT_STACK_VALUES`, generated
`LLG_MODEL_STACK_VALUES`, and every `aco_*` process/lifecycle entry. Generated
descriptors with `fn = NULL` are invalid. The capture type `llg_frame_t`
remains; it is unrelated to `llg_co_frame_t` and is intentionally not renamed
in this ABI.
