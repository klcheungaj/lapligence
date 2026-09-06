// llg_rt.h — event scheduler for the llg Verilog simulator's generated C11
// models. Four-state values are provided by llg_value.h.

// Processes are libaco coroutines; each generated process function is spawned
// via `llg_spawn` and suspends inside the `llg_wait_*` calls.  The scheduler
// (`llg_rt_run`) implements the IEEE 1800-2017 §4 region model within one
// time step:
//
//   1. active region: run every ready coroutine once (FIFO ready queue);
//   2. inactive region: wake every `#0` waiter (`llg_wait_time(0)`) and run
//      the resulting active work; a `#0` executed from an inactive
//      continuation schedules a re-inactive pass, so the region is drained in
//      a loop;
//   3. NBA region: commit every process's recorded non-blocking assignments
//      (per-process lists, recording order); commits write signals and wake
//      waiters, re-triggering the active region;
//   4. repeat the active → inactive → NBA sequence while new events appeared
//      in this time step;
//   5. otherwise advance time to the next pending timed wakeup (sorted list);
//      with none left, stop on `$finish` or report a deadlock.
//
// A zero-delay guard trips after LLG_ZERO_LOOP_LIMIT scheduler iterations
// within one time step (`always #0;` / NBA-oscillation loops / non-quiescing
// trigger cascades) and aborts the run with "llg: zero-delay loop detected
// at time N".  The guard unit is a scheduler iteration: each region pass AND
// each individual coroutine resume counts toward it.
//
// Time is measured in integer ticks; 1 tick == the design precision (the
// finest `timescale` precision across the design).  The runtime itself is
// timescale-agnostic: the codegen scales every `#N` delay and `$time`/`%t`
// read per the calling module's `timescale` unit before calling
// `llg_wait_time` / `llg_time`.  `$finish` sets a flag and `llg_rt_run`
// returns; no coroutine is resumed after a finish.

#ifndef LLG_RT_H
#define LLG_RT_H

#include <stddef.h>
#include <stdint.h>

#include "llg_value.h"

#ifdef __cplusplus
extern "C" {
#endif

// ── Collapsed inout nets ──────────────────────────────────────────────────────
//
// An inout port collapses its parent and child nets into ONE simulated net
// (IEEE 1800-2017 §23.3.3.7): every side writes a per-driver slot and readers
// see the wire/tri resolution of all slots. Standalone continuous-assignment
// groups carry per-slot drive-strength endpoints; collapsed inout and wired
// groups use the default strong endpoints with their established modes.
// All-Z when no driver is active; equal-strength conflicts resolve to X.
//
// The struct is a valid file-scope static initializer: driver cells are
// separate `sv4_t` globals whose addresses the codegen wires into `drivers`.

#define LLG_MAX_NET_DRIVERS 16

typedef struct {
    sv4_t resolved;                       /* what readers/waiters see */
    uint32_t width;
    int8_t is_signed;
    int8_t resolution;
    int n_drivers;
    sv4_t* drivers[LLG_MAX_NET_DRIVERS]; /* per-driver contribution cells */
    uint8_t strength0[LLG_MAX_NET_DRIVERS];
    uint8_t strength1[LLG_MAX_NET_DRIVERS];
} llg_net_t;

void llg_net_resolve(llg_net_t* net); /* strength-aware resolution, per limb */
void llg_net_write(llg_net_t* net, int idx, sv4_t value);

// ── Scheduler ─────────────────────────────────────────────────────────────────

typedef struct llg_proc llg_proc_t;
typedef struct { sv4_t* sig; int kind; } llg_event_spec_t;

#define LLG_MAX_PROCS 4096

// Event kinds used by llg_event_spec_t.
enum {
    LLG_EV_ANY = 0,     // any change
    LLG_EV_POSEDGE = 1, // 0->1, 0->X, X->1
    LLG_EV_NEGEDGE = 2, // 1->0, 1->X, X->0
};

void llg_rt_init(void);
// Release all runtime-owned scheduler, coroutine, fork-group, monitor and
// strobe allocations. Call only when no runtime coroutine is executing; init
// and run invoke it automatically. Repeated calls are safe.
void llg_rt_cleanup(void);
// Run until $finish, a deadlock, or all processes ending.
void llg_rt_run(void);
void llg_rt_finish(void);             // $finish
uint64_t llg_time(void);              // current tick count
uint64_t llg_time_scaled(uint64_t precision_ps, uint64_t unit_ps);
// Diagnostic count of allocated process objects, including completed fork
// parents retained while detached descendants are still live.
int llg_rt_process_count(void);

void llg_display(const char* fmt, ...);  // formatted output followed by a newline
void llg_write(const char* fmt, ...);    // formatted output without a newline

// ── $monitor / $strobe ────────────────────────────────────────────────────────
//
// A monitor's or strobe's arguments are re-evaluated by generated code through
// `eval`, which writes one `sv4_t` per argument into `out`, so the runtime
// reads the CURRENT values each time it prints (after the NBA region commits,
// for $strobe).  Format strings support the same specifiers as
// `llg_display` (`%d %h %b %o %t`; `%s` is rejected by the codegen for
// monitors/strobes).

typedef void (*llg_mon_eval_fn)(sv4_t* out);

// Register (or replace) the active $monitor: prints `fmt` immediately with
// the current argument values, then after every NBA commit whenever any
// argument differs from the last printed line.  Only the most recent
// $monitor is active; a new call replaces the previous one.
void llg_monitor(const char* fmt, int n, llg_mon_eval_fn eval);
// Queue a $strobe: prints `fmt` once with the argument values read after the
// NBA region of the current time step commits (unlike $display, which reads
// them when the statement executes).
void llg_strobe(const char* fmt, int n, llg_mon_eval_fn eval);
// $monitoron / $monitoroff: resume / suspend the active monitor.  While
// suspended the last-printed snapshot is kept; resuming re-prints if any
// argument changed since the last printed line.
void llg_monitor_set(int on);

// Spawn one process; `fn` must never return without calling `llg_proc_done`.
llg_proc_t* llg_spawn(void (*fn)(llg_proc_t*), const char* name);
// Terminate the current process (wraps aco_exit; never returns).
void llg_proc_done(llg_proc_t* self);

// ── final blocks ──────────────────────────────────────────────────────────────
//
// `final begin … end` processes (SV 1800-2005 §10.7) run ONCE at the end of
// simulation — after `llg_rt_run` returns on $finish, deadlock, or running
// out of future events.  Generated `main()` registers them via
// `llg_spawn_final` (before or after `llg_rt_run`; registration is plain
// bookkeeping) and calls `llg_rt_run_finals()` afterwards.
//
// Finals run SEQUENTIALLY to completion in registration order.  Timing
// controls inside a final are rejected by codegen (LRM §10.7) — fork/join
// included, so a final never suspends and never leaves children behind; the
// ready-queue drain in `llg_rt_run_finals` is dead-defensive only.
// `$finish` inside a final terminates that final immediately and skips all
// remaining final procedures, as required by LRM §10.7.
// `$time` inside finals reports the time of the last scheduler event.

#define LLG_MAX_FINALS 1024

// Register one final-block process (no coroutine is created here).
void llg_spawn_final(void (*fn)(llg_proc_t*), const char* name);
// Run every registered final process sequentially and then release the
// runtime (the finals phase owns teardown).  A no-op when nothing was
// registered.  Repeated init/run cycles reset the registration list.
void llg_rt_run_finals(void);

// ── fork/join ─────────────────────────────────────────────────────────────────
//
// Processes can spawn children with `llg_fork`; a child is a full coroutine
// on the shared scheduler stack with its own 256 KB save stack (grown on
// demand by libaco) and must end with `llg_proc_done` like any other process.
// Children are only ever resumed by the scheduler — never inline.  A
// `llg_fork_group_t` tracks the children of one `fork` statement:
//
//     llg_fork_group_t* g = llg_fork_group_new(LLG_JOIN);
//     llg_fork(child_a, "a", g);
//     llg_fork(child_b, "b", g);
//     llg_join(g);   // suspend until the group completes
//
// `llg_wait_fork` suspends until every live group of the current process is
// done (useful after join_none / join_any, whose groups outlive the parent's
// wait).  `llg_disable_fork` kills all descendants of the current process;
// killed children's pending NBAs are discarded and never committed.

typedef struct llg_fork_group llg_fork_group_t;

// Join kinds (VPI numbering: vpiJoin=0, vpiJoinNone=1, vpiJoinAny=2).
enum {
    LLG_JOIN = 0,      // wait for every child
    LLG_JOIN_NONE = 1, // return immediately
    LLG_JOIN_ANY = 2,  // wait for the first child to finish
};

// Create a fork group owned by the current process (registers it on the
// process's live-group list).
llg_fork_group_t* llg_fork_group_new(int join_kind);
// Spawn `fn` as a child of `grp`; `fn` must end with `llg_proc_done`.
llg_proc_t* llg_fork(void (*fn)(llg_proc_t*), const char* name, llg_fork_group_t* grp);
// Suspend until `grp` completes according to its join kind.
void llg_join(llg_fork_group_t* grp);
// Suspend until every live group of the current process has completed.
void llg_wait_fork(void);
// Kill every descendant of the current process (pending NBAs are discarded).
void llg_disable_fork(void);

void llg_wait_time(uint64_t ticks);   // #delay; 0 yields into the inactive region of the same time step
// Suspend until the requested transition of `sig`.
void llg_wait_edge(sv4_t* sig, int posedge);
// Suspend until any of `sigs` differs from its value at wait time.
void llg_wait_any(sv4_t** sigs, int n);
// Suspend until any event spec fires (or-list of @(posedge a or b ...)).
void llg_wait_any_events(llg_event_spec_t* specs, int n);
// Suspend until `sig` equals `value` (unknown never matches).
void llg_wait_level(sv4_t* sig, sv4_t value);

// ── Named events ──────────────────────────────────────────────────────────────
//
// A named event (`event ev; -> ev; @(ev);`, LRM 1364-1995 §9.7.3) carries no
// value: it is purely a wakeup channel.  Triggering wakes EVERY process
// currently suspended on the event, in deterministic waiter-table order —
// registration order until earlier partial unlinks (swap-with-last) reorder
// the table; a trigger with no waiters is lost — events are edge-triggered,
// not stateful, so trigger-before-wait never latches.  Each event holds a
// fixed-size waiter table; more concurrent waiters on ONE event than
// LLG_MAX_EVENT_WAITERS aborts the run like the other runtime resource
// limits.
//
// The struct is a valid static initializer (`{{ 0 }, 0}`): generated models
// define one global per declared event.

#define LLG_MAX_EVENT_WAITERS 64

typedef struct {
    llg_proc_t* waiters[LLG_MAX_EVENT_WAITERS];
    int n_waiters;
} llg_event_t;

// Wake every current waiter of `ev` and clear its waiter list.
void llg_event_trigger(llg_event_t* ev);
// Suspend until `ev` is triggered.
void llg_wait_event(llg_event_t* ev);
// Suspend until any of `evs` is triggered (one atomic registration).
void llg_wait_events(const llg_event_t* const* evs, int n);

// One source of a mixed signal/event or-list (`@(posedge a or ev)`): exactly
// one of `sig`/`ev` is set.  Exactly one such wait covers ALL entries, so a
// trigger arriving while the process is parked on the signal half is not lost.
typedef struct {
    sv4_t* sig;               // signal entry (NULL for an event entry)
    int kind;                 // LLG_EV_* edge kind for signal entries
    const llg_event_t* ev;   // event entry (NULL for a signal entry)
} llg_wait_src_t;

// Atomic mixed wait until any signal entry matches its edge kind or any event
// entry is triggered.
void llg_wait_mixed(llg_wait_src_t* srcs, int n);

// Assignments.  llg_nba records on the current process's list and commits in
// the NBA region; llg_ba writes immediately and notifies waiters.
void llg_nba(sv4_t* target, sv4_t value);
void llg_ba(sv4_t* target, sv4_t value);
void llg_nba_d(double* target, double value);
void llg_ba_d(double* target, double value);

// ── force / release ───────────────────────────────────────────────────────────
//
// `force sig = expr;` overrides a signal's value until `release sig;` (LRM
// 10.6.2): while forced, procedural writes — blocking (`llg_ba`) and
// non-blocking (NBA commits) — to the signal are ignored.  `llg_force` saves
// the pre-force value and writes the forced value through `sig_write` (so
// waiters wake); `llg_release` removes the entry and restores the saved
// value.  Re-forcing an already forced signal updates the forced value but
// keeps the ORIGINAL saved value; releasing an unforced signal is a no-op.
// The table is fixed-size (LLG_MAX_FORCE); a force beyond the limit aborts
// like the other runtime resource limits.  Net resolution writes
// (`llg_net_write`) and monitor/strobe reads are unaffected.

#define LLG_MAX_FORCE 64

void llg_force(sv4_t* sig, sv4_t value);
void llg_release(sv4_t* sig);

#ifdef __cplusplus
}
#endif

#endif // LLG_RT_H
