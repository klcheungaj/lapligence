// llg_rt.h — 4-state value model and event scheduler for the llg Verilog
// simulator's generated C11 models.
//
// Values
// ------
// `sv4_t` models a 4-state vector of at most LLG_MAX_WIDTH (1024) bits, stored
// as three parallel arrays of 64-bit limbs: bit i lives in `bits[i/64]` /
// `x[i/64]` / `z[i/64]` at position `i%64`.  The invariant x & z == 0 holds:
// bit i is X iff `(x[i/64] >> (i%64)) & 1`, Z iff `(z[i/64] >> (i%64)) & 1`,
// otherwise its value is `(bits[i/64] >> (i%64)) & 1`.  For expression
// semantics Z behaves like X in every op that propagates unknown bits (LRM
// 11.4.5); X and Z are only distinguished by `$display`, casez/casex wildcard
// matching, `===`/`!==` and the identity/copy ops (mux with a known select,
// selects, resize, concat) which carry Z through.  Every operation keeps limbs
// above the vector's width zero, and masks the top partial limb to the width.
//
// Scheduler
// ---------
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

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

// ── 4-state values ────────────────────────────────────────────────────────────

#define LLG_MAX_WIDTH 1024u
#define LLG_LIMBS ((LLG_MAX_WIDTH + 63u) / 64u) /* 16 */

typedef struct {
    uint64_t bits[LLG_LIMBS]; // known bits; valid where x/z bits are 0
    uint64_t x[LLG_LIMBS];    // X bits (unknown)
    uint64_t z[LLG_LIMBS];    // Z bits (high-impedance); x & z == 0
    uint16_t width;            // vector width (0..LLG_MAX_WIDTH)
    int8_t is_signed;          // signedness for resize/compare
} sv4_t;

// Compile-time bit mask for a width literal (<= 64).
#define LLG_MASK(w) ((w) >= 64 ? ~0ULL : ((1ULL << (w)) - 1))

// Build a value from single-limb bit/x/z masks.  Limbs above limb 0 are zero.
// `SV4_C`/`SV4_S`/`SV4_X` clamp the width to 64 — they fill limb 0 only and are
// meant for the code generator's <= 64-bit constants (wider values use
// `sv4_from_limbs` / the wide codegen follow-up).
#define SV4_INIT(b, x, z, w, s) \
    ((sv4_t){ { [0] = (uint64_t)(b) }, { [0] = (uint64_t)(x) }, \
              { [0] = (uint64_t)(z) }, \
              (uint16_t)(w), (int8_t)(s) })
// Unsigned / signed clean value with `w` bits.
#define SV4_C(b, w) SV4_INIT((b), 0, 0, ((w) > 64u ? 64u : (w)), 0)
#define SV4_S(b, w) SV4_INIT((b), 0, 0, ((w) > 64u ? 64u : (w)), 1)
// All-X value of `w` bits (constant expression for literal `w`).
#define SV4_X(w) SV4_INIT(0, LLG_MASK(w), 0, ((w) > 64u ? 64u : (w)), 0)
// All-Z value of `w` bits (constant expression for literal `w`).
#define SV4_Z(w) SV4_INIT(0, 0, LLG_MASK(w), ((w) > 64u ? 64u : (w)), 0)

// ── Value constructors / inspectors ───────────────────────────────────────────

sv4_t sv4_x(uint16_t width, int8_t is_signed);
sv4_t sv4_from_u64(uint64_t v, uint16_t width, int8_t is_signed);
sv4_t sv4_from_i64(int64_t v, uint16_t width);
double sv4_to_real(sv4_t v);
sv4_t sv4_from_real(double v, uint16_t width, int8_t is_signed);
int llg_real_to_bool(double v);
// Build from raw limb arrays (any may be NULL to zero-fill); the top partial
// limb is masked to `width` and the width is clamped to LLG_MAX_WIDTH.
sv4_t sv4_from_limbs(const uint64_t* bits, const uint64_t* x, const uint64_t* z,
                     uint16_t width, int8_t is_signed);
sv4_t sv4_resize(sv4_t v, uint16_t width, int8_t is_signed);
// Value-preserving conversion (LRM 1800-2009 §6.24.1 / §10.7): widening
// extends by the SOURCE's signedness (`v.is_signed`), narrowing truncates;
// the result carries `is_signed`.  Unlike `sv4_resize`, whose extension
// follows the passed flag, an unsigned source zero-extends even into a
// signed target and a signed source sign-extends even into an unsigned one.
sv4_t sv4_cast(sv4_t v, uint16_t width, int8_t is_signed);
// All `width` bits set to one literal bit value: bit 0, bit 1, bit 2 = X,
// or bit 3 = Z.
sv4_t sv4_fill(uint8_t bit, uint16_t width, int8_t is_signed);
sv4_t sv4_clog2(sv4_t v);

int sv4_is_unknown(sv4_t v);      // any bit X or Z
int sv4_to_bool(sv4_t v);         // != 0 with no unknown bits, else 0
uint64_t sv4_to_u64(sv4_t v);     // low limb; meaningful only when width <= 64
int64_t sv4_to_i64(sv4_t v);      // two's-complement interpretation of low bits
int sv4_fits_i64(sv4_t v);        // exact signed conversion is representable
int sv4_same(sv4_t a, sv4_t b);   // bits + x + z equal (ignores width/signed)

// Format one value into `buf` (NUL-terminated).  `fmt` is 'd', 'h', 'b' or 'o'.
// %b prints all width bits: 'x' for X bits and 'z' for Z bits; %h prints
// ceil(width/4) digits ('x' if any bit of the nibble is X, else 'z' if any is
// Z); %o likewise in octal; %d prints 'x' when any bit is X or Z.
void sv4_format(char fmt, sv4_t v, char* buf, size_t cap);
// Unsigned decimal via long division across limbs; any unknown bit -> "x".
// A signed value (`is_signed`) with the sign bit set prints '-' followed by
// its two's-complement magnitude (`~v + 1` within the value's width).
void sv4_to_dec_string(sv4_t v, char* buf, size_t cap);

// ── Arithmetic / logic ops (IEEE 1364 semantics) ──────────────────────────────
//
// Result widths are self-determined exactly like src/core/elab.rs: arithmetic
// and bitwise ops use max operand width, shifts keep the LHS width, compares /
// reductions / logical ops yield 1 bit.  Unknown operand bits propagate: any
// unknown bit makes arithmetic results all-X; 0 dominates AND and 1 dominates
// OR per bit; a shift with unknown amount yields all-X.

sv4_t sv4_add(sv4_t a, sv4_t b);
sv4_t sv4_sub(sv4_t a, sv4_t b);
sv4_t sv4_mul(sv4_t a, sv4_t b);
sv4_t sv4_div(sv4_t a, sv4_t b);
sv4_t sv4_mod(sv4_t a, sv4_t b);
sv4_t sv4_pow(sv4_t a, sv4_t b);
sv4_t sv4_neg(sv4_t a);              // unary minus
sv4_t sv4_bitneg(sv4_t a);           // ~
sv4_t sv4_lognot(sv4_t a);           // !
sv4_t sv4_and(sv4_t a, sv4_t b);     // &
sv4_t sv4_or(sv4_t a, sv4_t b);      // |
sv4_t sv4_xor(sv4_t a, sv4_t b);     // ^
sv4_t sv4_xnor(sv4_t a, sv4_t b);    // ~^
sv4_t sv4_logand(sv4_t a, sv4_t b);  // &&
sv4_t sv4_logor(sv4_t a, sv4_t b);   // ||
sv4_t sv4_reduce_and(sv4_t a);       // &a
sv4_t sv4_reduce_nand(sv4_t a);
sv4_t sv4_reduce_or(sv4_t a);        // |a
sv4_t sv4_reduce_nor(sv4_t a);
sv4_t sv4_reduce_xor(sv4_t a);       // ^a
sv4_t sv4_reduce_xnor(sv4_t a);
sv4_t sv4_shl(sv4_t a, sv4_t b);     // <<
sv4_t sv4_shr(sv4_t a, sv4_t b);     // >>
sv4_t sv4_ashl(sv4_t a, sv4_t b);    // <<<
sv4_t sv4_ashr(sv4_t a, sv4_t b);    // >>>
sv4_t sv4_eq(sv4_t a, sv4_t b);      // == (X when any operand bit X/Z)
sv4_t sv4_neq(sv4_t a, sv4_t b);
sv4_t sv4_case_eq(sv4_t a, sv4_t b); // === (never X; X/Z compared literally)
sv4_t sv4_case_neq(sv4_t a, sv4_t b);
// casez/casex wildcard match (never X; 1-bit result), per LRM 12.5.1.  Both
// resize the operands to max width (zero-extend) and test bits LSB-up:
//   casez: item z/? -> don't-care; item x -> matches selector x only;
//          item known -> matches only an equal selector bit (sel x/z -> no).
//   casex: item x/z/? -> don't-care; item known -> matches unless the
//          selector holds the opposite known bit (sel x/z -> match).
sv4_t sv4_casez_eq(sv4_t sel, sv4_t item);
sv4_t sv4_casex_eq(sv4_t sel, sv4_t item);
sv4_t sv4_lt(sv4_t a, sv4_t b);
sv4_t sv4_le(sv4_t a, sv4_t b);
sv4_t sv4_gt(sv4_t a, sv4_t b);
sv4_t sv4_ge(sv4_t a, sv4_t b);
sv4_t sv4_mux(sv4_t sel, sv4_t a, sv4_t b);
sv4_t sv4_concat(sv4_t hi, sv4_t lo);     // hi is the MS part
sv4_t sv4_repeat(sv4_t pat, uint64_t n);  // {n{pat}}
sv4_t sv4_part_select(sv4_t v, int64_t left, int64_t right); // handles reversed ranges
void sv4_part_select_set(sv4_t* tgt, int64_t left, int64_t right, sv4_t value);
sv4_t sv4_bit_select(sv4_t v, uint64_t i);
void sv4_bit_select_set(sv4_t* tgt, uint64_t i, sv4_t value);
sv4_t sv4_idx_part_select(sv4_t v, uint64_t base, uint16_t width, int neg);
void sv4_idx_part_select_set(sv4_t* tgt, uint64_t base, uint16_t width, int neg,
                             sv4_t value);

// ── Collapsed inout nets ──────────────────────────────────────────────────────
//
// An inout port collapses its parent and child nets into ONE simulated net
// (IEEE 1800-2017 §23.3.3.7): every side writes a per-driver slot and readers
// see the wire/tri resolution (Table 6-2, equal strengths) of all slots.
// All-Z when no driver is active; any X or mixed 0/1 resolves to X.
//
// The struct is a valid file-scope static initializer: driver cells are
// separate `sv4_t` globals whose addresses the codegen wires into `drivers`.

#define LLG_MAX_NET_DRIVERS 16

typedef struct {
    sv4_t resolved;                       /* what readers/waiters see */
    uint16_t width;
    int8_t is_signed;
    int n_drivers;
    sv4_t* drivers[LLG_MAX_NET_DRIVERS]; /* per-driver contribution cells */
} llg_net_t;

void llg_net_resolve(llg_net_t* net); /* LRM wire/tri, equal strengths, per limb */
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
