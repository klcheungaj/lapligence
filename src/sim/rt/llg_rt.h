// llg_rt.h — event scheduler for the llg Verilog simulator's generated C11
// models. Four-state values are provided by llg_value.h.

// Processes are libaco coroutines; each generated process function is spawned
// via `llg_spawn` and suspends inside the `llg_wait_*` calls. The scheduler
// (`llg_rt_run`) keeps a separate queue for every IEEE 1800-2017 §4 execution
// region. Region queues are drained to a fixed point in the Figure 4-1 order;
// reactive work may enqueue design work and starts another design iteration
// before the postponed output point. Legacy Verilog scheduling remains the
// Active/Inactive/NBA subset of this state machine.
//
// Zero-time progress is bounded by two runtime limits.  The scheduler guard
// counts region passes and coroutine resumes; the process guard counts
// generated loop back-edges, so a coroutine that never yields is still
// interruptible.  Both default to LLG_ZERO_LOOP_LIMIT and can be configured
// with the LLG_ZERO_LOOP_LIMIT environment variable.  The per-process limit
// can be overridden with LLG_PROCESS_STEP_LIMIT (or the legacy-compatible
// LLG_NONCONVERGENCE_LIMIT alias).  Limits must be positive decimal uint64
// values; invalid or overflowing values are diagnosed before simulation.
//
// Time is measured in integer ticks; 1 tick == the design precision (the
// finest `timescale` precision across the design).  The runtime itself is
// timescale-agnostic: the codegen scales every `#N` delay and `$time`/`%t`
// read per the calling module's `timescale` unit before calling
// `llg_wait_time` / `llg_time`. `$finish` reports its validated level through
// `llg_rt_finish_with_level`, sets a flag, and exits the current coroutine;
// no coroutine is resumed after a finish. `$stop` reports through
// `llg_rt_stop_with_level`, yields the current coroutine, and preserves every
// queue, activation frame, output stream, and simulation tick until the stop
// policy resumes it.

#ifndef LLG_RT_H
#define LLG_RT_H

#include <stddef.h>
#include <stdint.h>

#include "llg_value.h"
#include "llg_string.h"
#include "llg_rng.h"

#ifndef LLG_ZERO_LOOP_LIMIT
#define LLG_ZERO_LOOP_LIMIT 10000000ULL
#endif

#ifndef LLG_PROCESS_STEP_LIMIT
#define LLG_PROCESS_STEP_LIMIT LLG_ZERO_LOOP_LIMIT
#endif

#ifdef __cplusplus
extern "C" {
#endif

// Typed display values. The runtime owns string members after a display call
// or while a deferred monitor/strobe snapshot is live.
enum {
    LLG_FMT_PACKED = 0,
    LLG_FMT_REAL = 1,
    LLG_FMT_STRING = 2,
};

enum {
    LLG_SEVERITY_INFO = 0,
    LLG_SEVERITY_WARNING = 1,
    LLG_SEVERITY_ERROR = 2,
    LLG_SEVERITY_FATAL = 3,
};

enum {
    LLG_ASSERTION_ASSERT = 0,
    LLG_ASSERTION_ASSUME = 1,
    LLG_ASSERTION_COVER = 2,
};

typedef struct {
    int kind;
    union {
        sv4_t packed;
        double real;
        llg_string_t string;
    } value;
} llg_fmt_arg_t;

typedef struct {
    int kind;
    void* ptr;
} llg_display_read_t;

// ── Collapsed inout nets ──────────────────────────────────────────────────────
//
// An inout port collapses its parent and child nets into ONE simulated net
// (IEEE 1800-2017 §23.3.3.7): every side writes a per-driver slot and readers
// see the wire/tri resolution of all slots. Standalone continuous-assignment
// groups carry per-slot drive-strength endpoints; collapsed inout and wired
// groups carry their per-slot strength endpoints through the same resolver.
// TRI0/TRI1 and SUPPLY0/SUPPLY1 retain their implicit pull/supply source.
// All-Z is produced only when no explicit or implicit source is active;
// equal-strength conflicts resolve according to the net's wired rule.
//
// The struct is a valid file-scope static initializer: driver cells are
// separate `sv4_t` globals whose addresses the codegen wires into `drivers`.

#define LLG_MAX_NET_DRIVERS 16
#define LLG_MAX_NET_ALIASES 256

typedef struct llg_inertial llg_inertial_t;
typedef struct llg_net llg_net_t;
typedef struct llg_net_alias_part llg_net_alias_part_t;
typedef struct llg_net_alias llg_net_alias_t;

struct llg_net {
    sv4_t resolved;                       /* what readers/waiters see */
    uint32_t width;
    int8_t is_signed;
    int8_t resolution;
    int n_drivers;
    sv4_t* drivers[LLG_MAX_NET_DRIVERS]; /* per-driver contribution cells */
    uint8_t strength0[LLG_MAX_NET_DRIVERS];
    uint8_t strength1[LLG_MAX_NET_DRIVERS];
    int8_t propagation_enabled;
    llg_inertial_t* propagation;
    uint64_t propagation_rise;
    uint64_t propagation_fall;
    uint64_t propagation_turn_off;
    int n_aliases;
    llg_net_alias_t* aliases[LLG_MAX_NET_ALIASES];
};

struct llg_net_alias_part {
    llg_net_t* net;
    int slot;
    uint32_t signal_bit;
    uint32_t group_bit;
};

struct llg_net_alias {
    sv4_t* storage;
    sv4_t visible;
    uint32_t width;
    int8_t is_signed;
    const llg_net_alias_part_t* parts;
    uint32_t n_parts;
};

void llg_net_resolve(llg_net_t* net); /* strength-aware resolution, per limb */
void llg_net_write(llg_net_t* net, int idx, sv4_t value);
void llg_net_alias_bind(llg_net_alias_t* alias);
sv4_t llg_net_alias_read(llg_net_alias_t* alias);
void llg_net_alias_write(llg_net_alias_t* alias, sv4_t value);

// The runtime owns each inertial driver and its pending event. The caller's
// initially NULL handle, target and net must persist until cleanup, which
// resets the handle to NULL. Repeated evaluation never suspends the caller.
void llg_inertial_assign(llg_inertial_t** handle, sv4_t* target,
                         sv4_t value, uint64_t rise, uint64_t fall,
                         uint64_t turn_off);
void llg_inertial_net(llg_inertial_t** handle, llg_net_t* net, int slot,
                      sv4_t value, uint64_t rise, uint64_t fall,
                      uint64_t turn_off);
void llg_inertial_selected_assign(llg_inertial_t** handle, sv4_t* target,
                                  sv4_t value, sv4_t mask, uint64_t rise,
                                  uint64_t fall, uint64_t turn_off);
void llg_inertial_selected_net(llg_inertial_t** handle, llg_net_t* net,
                               int slot, sv4_t value, sv4_t mask,
                               uint64_t rise, uint64_t fall,
                               uint64_t turn_off);

// ── Scheduler ─────────────────────────────────────────────────────────────────

typedef struct llg_proc llg_proc_t;
typedef struct llg_frame llg_frame_t;
typedef struct llg_activation llg_activation_t;
typedef struct { sv4_t* sig; int kind; } llg_event_spec_t;
// One typed storage dependency. Exactly one pointer is non-null; real
// dependencies point directly at the generated double companion. Real
// equality follows the write path's bitwise comparison and never converts
// through packed storage.
typedef struct {
    sv4_t* sig;
    double* real;
} llg_wait_dependency_t;

// Execution regions, in reference-algorithm order. PLI control points are
// explicit even when no public VPI registration has been lowered yet. The
// semantic aliases keep generated Verilog-facing code readable.
typedef enum {
    LLG_REGION_PREPONED = 0,
    LLG_REGION_PREPONED_PLI,
    LLG_REGION_PRE_ACTIVE_PLI,
    LLG_REGION_ACTIVE,
    LLG_REGION_INACTIVE,
    LLG_REGION_PRE_NBA_PLI,
    LLG_REGION_PRE_NBA,
    LLG_REGION_NBA,
    LLG_REGION_POST_NBA,
    LLG_REGION_POST_NBA_PLI,
    LLG_REGION_PRE_OBSERVED_PLI,
    LLG_REGION_PRE_OBSERVED,
    LLG_REGION_OBSERVED,
    LLG_REGION_POST_OBSERVED,
    LLG_REGION_POST_OBSERVED_PLI,
    LLG_REGION_REACTIVE,
    LLG_REGION_RE_INACTIVE,
    LLG_REGION_PRE_RE_NBA_PLI,
    LLG_REGION_PRE_RE_NBA,
    LLG_REGION_RE_NBA,
    LLG_REGION_POST_RE_NBA,
    LLG_REGION_POST_RE_NBA_PLI,
    LLG_REGION_PRE_POSTPONED_PLI,
    LLG_REGION_PRE_POSTPONED,
    LLG_REGION_POSTPONED,
    LLG_REGION_POSTPONED_PLI,
    LLG_REGION_COUNT
} llg_region_t;

#define LLG_REGION_NONBLOCKING_ASSIGN LLG_REGION_NBA
#define LLG_REGION_RE_NONBLOCKING_ASSIGN LLG_REGION_RE_NBA

#define LLG_MAX_PROCS 4096

// Event kinds used by llg_event_spec_t.
enum {
    LLG_EV_ANY = 0,     // any change
    LLG_EV_POSEDGE = 1, // 0->1, 0->X, X->1
    LLG_EV_NEGEDGE = 2, // 1->0, 1->X, X->0
};

void llg_rt_init(void);
// Initialize the runtime and retain the generated model's argv view for
// `$test$plusargs`/`$value$plusargs`. The runtime never takes ownership of
// `argv`; callers keep it valid for the duration of the simulation.
void llg_rt_init_with_args(int argc, char** argv);
// Release all runtime-owned scheduler, coroutine, fork-group, monitor and
// strobe allocations. Call only when no runtime coroutine is executing; init
// and run invoke it automatically. Repeated calls are safe.
void llg_rt_cleanup(void);
// Run until $finish, a deadlock, all processes ending, or a `$stop` whose
// policy is `exit`. The default stop policy is `resume`, which automatically
// resumes the stopped process at the same simulation time so noninteractive
// command-line runs cannot hang waiting for input. An embedding may select
// `exit`, inspect `llg_rt_is_suspended`, call `llg_rt_resume`, and invoke
// `llg_rt_run` again.
void llg_rt_run(void);
// True when the runtime stopped because of a configuration, nonconvergence,
// or other controlled simulation failure.  The result survives cleanup.
int llg_rt_failed(void);
// Region currently being drained. During initialization this is PREPONED.
llg_region_t llg_current_region(void);
// Return a stable printable name for diagnostics and callback traces.
const char* llg_region_name(llg_region_t region);
// True for sampling/observation/output regions while the scheduler is running.
int llg_region_is_read_only(void);
// Request scheduler termination from a non-coroutine callback. Unlike
// llg_rt_finish, this returns to the callback and is safe outside a process.
void llg_rt_request_finish(void);
// Terminate the current simulation process and mark the scheduler for exit;
// neither entry point returns to generated HDL. The legacy entry point is a
// quiet level-0 finish without source metadata.
_Noreturn void llg_rt_finish(void);
_Noreturn void llg_rt_finish_with_level(int verbosity, const char* location);
// `$stop` diagnostics use the same validated 0/1/2 verbosity levels as
// `$finish`, but suspension is resumable and does not enter the final phase.
// The call returns after the issuing coroutine is resumed. The legacy entry
// point is a quiet level-0 stop without source metadata.
void llg_rt_stop(void);
void llg_rt_stop_with_level(int verbosity, const char* location);

enum {
    LLG_STOP_POLICY_RESUME = 0,
    LLG_STOP_POLICY_EXIT = 1,
};

// Select how `$stop` behaves when the scheduler reaches the stop point. This
// may be called before `llg_rt_init` or while the runtime is suspended. It
// returns zero for an invalid policy or a running scheduler and one on
// success. `LLG_STOP_POLICY_RESUME` is the default.
int llg_rt_set_stop_policy(int policy);
int llg_rt_stop_policy(void);
// True after a stop with the EXIT policy yielded a process. The scheduler
// context remains live until `llg_rt_resume` or `llg_rt_cleanup` is called.
int llg_rt_is_suspended(void);
// Resume the process suspended by `$stop`, queueing its continuation at the
// same simulation time. Returns one when a suspension was resumed and zero
// when no resumable stop is pending.
int llg_rt_resume(void);
uint64_t llg_time(void);              // current tick count
// Current time rounded to the nearest local unit; exact half units round up.
// The caller applies any result-width conversion (for example, $stime's
// low-32-bit result) after this operation.
uint64_t llg_time_scaled(uint64_t precision_fs, uint64_t unit_fs);
// Diagnostic count of allocated process objects, including completed fork
// parents retained while detached descendants are still live.
int llg_rt_process_count(void);

// ── IEEE stochastic analysis queues ─────────────────────────────────────────
//
// These queues implement the Verilog stochastic analysis system tasks
// (IEEE 1364-2001 §17.6 / IEEE 1800-2009 §20.16). They are deliberately
// separate from SystemVerilog queue containers: each queue stores a job ID,
// an information ID, and the simulation tick at which the job arrived.
// Integer arguments are checked four-state values; unknown or out-of-range
// values are reported as a controlled runtime failure instead of being
// silently truncated. Output values are written through the ordinary
// procedural-write path so force/PCA rules remain consistent with HDL.
enum {
    LLG_Q_OK = 0,
    LLG_Q_FULL = 1,
    LLG_Q_UNKNOWN_ID = 2,
    LLG_Q_EMPTY = 3,
    LLG_Q_BAD_TYPE = 4,
    LLG_Q_BAD_LENGTH = 5,
    LLG_Q_DUPLICATE_ID = 6,
    LLG_Q_NO_MEMORY = 7,
};

void llg_q_initialize(sv4_t q_id, sv4_t q_type, sv4_t max_length,
                      sv4_t* status);
void llg_q_add(sv4_t q_id, sv4_t job_id, sv4_t inform_id, sv4_t* status);
void llg_q_remove(sv4_t q_id, sv4_t* job_id, sv4_t* inform_id,
                  sv4_t* status);
sv4_t llg_q_full(sv4_t q_id, sv4_t* status);
void llg_q_exam(sv4_t q_id, sv4_t stat_code, sv4_t* stat_value,
                sv4_t* status);

// ── Process/object random streams ───────────────────────────────────────────
//
// Every generated process owns one stream. Top-level processes derive from
// the model root in creation order; forked children derive from their parent
// in branch-creation order. A draw mutates only its owning stream. The
// scheduler-independent llg_rng_state_t API is also used by future class
// objects and constrained-random services.
sv4_t llg_urandom(void);
sv4_t llg_urandom_seed(sv4_t seed);
sv4_t llg_urandom_range(sv4_t max, sv4_t min, int has_min);
void llg_process_srandom(sv4_t seed);
llg_string_t llg_process_get_randstate(void);
int llg_process_set_randstate(llg_string_t state);

// Stable dependency markers used by generated fixed-array and container
// readers. A marker's address remains valid when a resizable container moves
// its backing storage. Bindings are cleared by llg_rt_cleanup.
void llg_dependency_bind(sv4_t* target, sv4_t* dependency);
void llg_dependency_bind_real(double* target, sv4_t* dependency);
void llg_dependency_changed(sv4_t* dependency);
void llg_dependency_notify(sv4_t* contents, sv4_t* shape, int change);

void llg_display(const char* fmt, ...);  // formatted output followed by a newline
void llg_write(const char* fmt, ...);    // formatted output without a newline
void llg_display_typed(const char* fmt, llg_fmt_arg_t* args, int n,
                       const char* scope);
void llg_write_typed(const char* fmt, llg_fmt_arg_t* args, int n,
                     const char* scope);
// Format into a newly-owned string. The format value and argument array are
// consumed exactly once, including destruction of every owned string member.
llg_string_t llg_string_format_typed(llg_string_t format, llg_fmt_arg_t* args,
                                     int n, const char* scope);
// Runtime severity tasks use the same typed formatter as display tasks and
// write one source-context diagnostic to stderr. The argument array is
// consumed exactly once, including destruction of owned strings.
void llg_rt_severity_typed(int severity, const char* fmt, llg_fmt_arg_t* args,
                           int n, const char* scope, const char* location);
_Noreturn void llg_rt_fatal_typed(int finish_number, const char* fmt,
                                  llg_fmt_arg_t* args, int n,
                                  const char* scope, const char* location);
// Counts reset at llg_rt_init and remain available through final-block
// execution. Invalid levels return zero.
uint64_t llg_rt_severity_count(int severity);
// Immediate assertion default-failure and successful-cover callbacks. The
// identity is retained at the call boundary for future per-assertion APIs.
void llg_assertion_failure(int kind, uint64_t identity, const char* label,
                           const char* location);
void llg_assertion_cover(uint64_t identity, const char* label, const char* location);
uint64_t llg_assertion_count(int kind);

// ── Command-line plusargs ───────────────────────────────────────────────────
//
// Plusargs are the argv entries beginning with '+'. Test queries use literal
// prefix matching after the leading '+'. Value queries accept the standard
// %d/%o/%h/%x/%b/%e/%f/%g/%s conversions (including uppercase and leading-0
// forms); an unmatched query returns zero and leaves its destination unchanged.
int llg_test_plusargs(const char* pattern);
int llg_value_plusargs_packed(const char* format, sv4_t* out, uint32_t width,
                              int is_signed, int two_state);
int llg_value_plusargs_real(const char* format, double* out);
int llg_value_plusargs_string(const char* format, llg_string_t* out);
// Execute one `$system` command in the generated simulator process. This
// host boundary is disabled unless LLG_ALLOW_SYSTEM is set to 1, true, yes,
// or on. When disabled, the runtime diagnoses the attempted command, marks
// the simulation failed, and returns an all-known -1 status without invoking
// a shell. `has_command == 0` preserves the standard's omitted-argument
// `system(NULL)` query; `has_command == 1` passes the owned command, including
// an explicit empty string, to the host C `system()` function. The returned
// 32-bit signed value is the host C `system()` status; its nonzero encoding is
// platform-specific and is not normalized here. `command` is consumed
// regardless of whether execution is permitted.
sv4_t llg_system(llg_string_t command, int has_command);

// ── File descriptors and output ─────────────────────────────────────────────
// Descriptors are 32-bit masks: bit 0 is stdout, bit 1 is stderr, and each
// ordinary opened file receives one higher bit.  The runtime owns ordinary
// FILE objects and closes them during cleanup; standard streams are borrowed.
uint32_t llg_file_descriptor(sv4_t value);
// Consumes the owned path/mode strings; an omitted mode selects write mode.
uint32_t llg_file_open(llg_string_t path, llg_string_t mode, int has_mode);
void llg_file_close(uint32_t descriptor);
int llg_file_flush(uint32_t descriptor, int all);
void llg_file_rewind(uint32_t descriptor);
int64_t llg_file_tell(uint32_t descriptor);
int llg_file_seek(uint32_t descriptor, sv4_t offset, sv4_t operation);
int llg_file_error(uint32_t descriptor, llg_string_t* message);
int llg_file_eof(uint32_t descriptor);
void llg_file_display_typed(uint32_t descriptor, const char* fmt,
                            llg_fmt_arg_t* args, int n, const char* scope,
                            int newline);

// ── Memory file tasks ────────────────────────────────────────────────────────
// Consume an owned path and read/write a fixed one-dimensional packed memory.
// `dims` carries the declaration's left/right bounds; start/finish are used
// only when the corresponding flag is non-zero. Radix is 2 for binary and 16
// for hexadecimal files. File syntax accepts whitespace, comments, radix
// digits, and `@` address jumps while preserving four-state X/Z digits.
void llg_memory_read(llg_string_t path, sv4_t* memory, uint64_t total,
                     uint32_t elem_width, int8_t elem_signed, int8_t two_state,
                     const int32_t* dims, int n_dims, sv4_t start, sv4_t finish,
                     int has_start, int has_finish, int radix);
void llg_memory_write(llg_string_t path, sv4_t* memory, uint64_t total,
                      uint32_t elem_width, int8_t elem_signed, int8_t two_state,
                      const int32_t* dims, int n_dims, sv4_t start, sv4_t finish,
                      int has_start, int has_finish, int radix);

// ── File input ─────────────────────────────────────────────────────────────
// Formatted input uses an HDL-aware scanner rather than the host scanf family:
// packed destinations preserve X/Z and arbitrary model widths, while return
// values count successful assignments only.  Target descriptors are borrowed
// for the duration of one call and are never retained by the runtime.
enum {
    LLG_FILE_INPUT_PACKED = 0,
    LLG_FILE_INPUT_REAL = 1,
    LLG_FILE_INPUT_STRING = 2,
};

typedef struct {
    int kind;
    llg_ref_t* packed;
    double* real;
    llg_string_t* string;
    int shortreal;
} llg_file_input_target_t;

int llg_file_getc(uint32_t descriptor);
int llg_file_ungetc(uint32_t descriptor, sv4_t character);
int llg_file_gets(uint32_t descriptor, llg_string_t* target);
int llg_file_gets_packed(uint32_t descriptor, llg_ref_t* target);
int llg_file_scanf(uint32_t descriptor, const char* format,
                   const llg_file_input_target_t* targets, int target_count);
int llg_string_scanf(const char* source, size_t source_length,
                     const char* format,
                     const llg_file_input_target_t* targets, int target_count);
int llg_file_read_packed(uint32_t descriptor, llg_ref_t* target);
int llg_file_read_array(uint32_t descriptor, sv4_t* values, uint32_t elem_width,
                        int elem_signed, int elem_two_state, uint64_t total,
                        const int32_t* dimensions, int dimension_count,
                        int has_start, sv4_t start, int has_count, sv4_t count);

// ── $monitor / $strobe ────────────────────────────────────────────────────────
//
// A monitor's or strobe's arguments are re-evaluated by generated code through
// `eval`, which writes one owned `llg_fmt_arg_t` per argument into `out`, so
// the runtime reads CURRENT values each time it prints (after the NBA region
// commits, for $strobe). Format strings use the same typed formatter as
// immediate display/write, including `%m`, real, and string conversions.

typedef void (*llg_mon_eval_fn)(sv4_t* out, void* context);
typedef void (*llg_real_eval_fn)(double* out, void* context);
typedef void (*llg_display_eval_fn)(llg_fmt_arg_t* out, void* context);

// Register (or replace) the active $monitor.  `reads` contains the signal
// pointers that trigger re-evaluation; display-only time queries are not
// triggers.  Registration queues a report for the scheduler's settled
// observation point rather than printing immediately.  Only the most recent
// $monitor is active; a new call replaces the previous one.
void llg_monitor_with_reads(const char* fmt, int n, llg_mon_eval_fn eval,
                            sv4_t* const* reads, int n_reads);
// Compatibility entry point for callers without an explicit trigger set.
void llg_monitor(const char* fmt, int n, llg_mon_eval_fn eval);
// Queue a $strobe: prints `fmt` once with the argument values read after the
// NBA region of the current time step commits (unlike $display, which reads
// them when the statement executes). Printing waits for active/inactive/NBA
// iteration to settle, including driver updates triggered by NBAs.
void llg_strobe(const char* fmt, int n, llg_mon_eval_fn eval);
void llg_monitor_with_typed_reads(const char* fmt, int n,
                                  llg_display_eval_fn eval, const char* scope,
                                  const llg_display_read_t* reads, int n_reads);
void llg_strobe_typed(const char* fmt, int n, llg_display_eval_fn eval,
                      const char* scope);
void llg_file_monitor_with_typed_reads(
    uint32_t descriptor, const char* fmt, int n, llg_display_eval_fn eval,
    const char* scope, const llg_display_read_t* reads, int n_reads);
void llg_file_strobe_typed(uint32_t descriptor, const char* fmt, int n,
                           llg_display_eval_fn eval, const char* scope);
// $monitoron / $monitoroff: resume / suspend the active monitor.  While
// suspended the last-printed snapshot is kept; enabling queues one report at
// the next settled observation point even when values are unchanged.
void llg_monitor_set(int on);

// Spawn one process; `fn` must never return without calling `llg_proc_done`.
llg_proc_t* llg_spawn(void (*fn)(llg_proc_t*), const char* name);
// Spawn a non-program process directly into an explicit execution region.
// This remains the runtime hook for assertion/VPI lowering; ordinary HDL
// processes use llg_spawn (ACTIVE), while programs use the typed entry below.
llg_proc_t* llg_spawn_in_region(void (*fn)(llg_proc_t*), const char* name,
                                llg_region_t region);
// Spawn a program process into a reactive region and account for its
// completion.  `$exit` and natural termination use that accounting to stop
// the simulation after all program processes (including fork children) end.
llg_proc_t* llg_spawn_program_in_region(void (*fn)(llg_proc_t*),
                                        const char* name,
                                        llg_region_t region);
// Return the activation frame retained by a process, or NULL for ordinary
// static-storage processes. The returned pointer is borrowed from `self`.
llg_frame_t* llg_proc_frame(llg_proc_t* self);
// Terminate the current process (wraps aco_exit; never returns).
_Noreturn void llg_proc_done(llg_proc_t* self);
// Terminate all program processes and descendants, then perform the implicit
// `$finish` transition.  Lowering only emits this call inside a program.
_Noreturn void llg_program_exit(void);
// Cooperative generated-loop interruption point.  It returns while the
// current process remains within its zero-time budget; on exhaustion it emits
// a source-bearing diagnostic and exits that coroutine without returning.
void llg_budget_point(const char* location);
// Report a SystemVerilog unique/unique0/priority branch check. `check` is
// 1=unique, 2=unique0, 3=priority; `matched` counts matching case groups (or
// is zero/one for a conditional); `has_default` suppresses no-match reports.
// Diagnostics are warnings and do not stop simulation.
void llg_unique_priority_check(int check, int matched, int has_default,
                               const char* location);

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
// `join_none` children are created in source order but become eligible only
// when their parent first suspends or terminates. `llg_wait_fork` suspends until
// every live group of the current process is done (useful after join_none /
// join_any, whose groups outlive the parent's wait). `llg_disable_fork` kills
// all descendants of the current process, including children still pending
// their first execution; killed children's immediate NBA lists are discarded.
// Future updates already in the global timed NBA queue retain their persistent
// targets.

typedef struct llg_fork_group llg_fork_group_t;

// Join kinds shared with the executable IR's C emission.
enum {
    LLG_JOIN = 0,      // wait for every child
    LLG_JOIN_NONE = 1, // return immediately
    LLG_JOIN_ANY = 2,  // wait for the first child to finish
};

// Create a fork group owned by the current process (registers it on the
// process's live-group list).
llg_fork_group_t* llg_fork_group_new(int join_kind);
// Create a named fork group whose resolved target can be disabled from any
// process in the same elaborated instance. Anonymous groups use the function
// above and carry no disable target.
llg_fork_group_t* llg_fork_group_new_target(int join_kind,
                                            uint32_t declaration,
                                            uint32_t instance);
// Spawn `fn` as a child of `grp`; `fn` must end with `llg_proc_done`.
llg_proc_t* llg_fork(void (*fn)(llg_proc_t*), const char* name, llg_fork_group_t* grp);
// Spawn a child with one retained reference to `frame`. The child releases
// that reference on completion or cancellation; the caller retains ownership
// of its own reference and may release it after this call.
llg_proc_t* llg_fork_with_frame(void (*fn)(llg_proc_t*), const char* name,
                                llg_fork_group_t* grp, llg_frame_t* frame);
// Create and manage typed activation storage. Slots hold copied values by
// default; frame aliases retain their source frame, while legacy model-storage
// aliases borrow only the generated static cell. No activation slot retains a
// host stack pointer.
typedef enum {
    LLG_FRAME_PACKED = 0,
    LLG_FRAME_REAL = 1,
    LLG_FRAME_OPAQUE = 2,
} llg_frame_slot_kind_t;
llg_frame_t* llg_frame_new(size_t slots);
void llg_frame_retain(llg_frame_t* frame);
void llg_frame_release(llg_frame_t* frame);
void llg_frame_capture_value(llg_frame_t* frame, size_t slot, sv4_t value);
void llg_frame_capture_real(llg_frame_t* frame, size_t slot, double value);
void llg_frame_alias_value(llg_frame_t* frame, size_t slot, sv4_t* target);
void llg_frame_alias_real(llg_frame_t* frame, size_t slot, double* target);
void llg_frame_alias_slot(llg_frame_t* frame, size_t slot,
                          llg_frame_t* target, size_t target_slot);
llg_frame_slot_kind_t llg_frame_slot_kind(const llg_frame_t* frame,
                                          size_t slot);
sv4_t llg_frame_read_value(const llg_frame_t* frame, size_t slot);
void llg_frame_write_value(llg_frame_t* frame, size_t slot, sv4_t value);
double llg_frame_read_real(const llg_frame_t* frame, size_t slot);
void llg_frame_write_real(llg_frame_t* frame, size_t slot, double value);
// Suspend until `grp` completes according to its join kind.
void llg_join(llg_fork_group_t* grp);
// Suspend until every live group of the current process has completed.
void llg_wait_fork(void);
// Kill every descendant of the current process (immediate NBA lists are discarded).
void llg_disable_fork(void);

// Named block/task activation registry. Declaration and instance identities
// come from the owned semantic database; textual names never reach this ABI.
llg_activation_t* llg_activation_enter(uint32_t declaration,
                                       uint32_t instance);
void llg_activation_exit(llg_activation_t* activation);
int llg_activation_cancelled(void);
void llg_disable_target(uint32_t declaration, uint32_t instance);

void llg_wait_time(uint64_t ticks);   // #delay; 0 yields into the inactive region of the same time step
// Set the explicit region used by the next signal/dependency wait. Generated
// sensitivity terminators use this to migrate a coroutine across region sets.
void llg_wait_resume_in_region(llg_region_t region);
// Suspend until the requested transition of `sig`.
void llg_wait_edge(sv4_t* sig, int posedge);
// Suspend until any of `sigs` differs from its value at wait time.
void llg_wait_any(sv4_t** sigs, int n);
// Suspend until any packed or real dependency changes. Each entry has exactly
// one of `sig`/`real` set.
void llg_wait_any_dependencies(const llg_wait_dependency_t* deps, int n);
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
// The struct is a valid zero initializer: generated models define one global
// per declared event. Ordinary waiters and persistent-trigger waiters are
// separate so a trigger never latches an ordinary `@(event)` control. The
// generation distinguishes a completed runtime from its next initialization,
// so a static generated event cannot retain `.triggered` across runs.

#define LLG_MAX_EVENT_WAITERS 64

typedef struct {
    llg_proc_t* waiters[LLG_MAX_EVENT_WAITERS];
    int n_waiters;
    llg_proc_t* triggered_waiters[LLG_MAX_EVENT_WAITERS];
    int n_triggered_waiters;
    uint64_t triggered_time;
    uint64_t triggered_generation;
    int triggered;
} llg_event_object_t;

typedef struct {
    llg_event_object_t* object;
} llg_event_t;

// Resolve a fixed unpacked event-array select using declaration-order
// flattening. Unknown/out-of-range indices resolve to a null handle, which
// preserves the ordinary null-event trigger/wait behavior.
llg_event_t* llg_event_array_select(llg_event_t* const* elements,
                                     uint64_t total,
                                     const int32_t* left,
                                     const int32_t* right,
                                     const sv4_t* indices,
                                     int n);

// Assigning a handle changes only the future object resolved by the target;
// waiter registrations already attached to the previous object are retained.
void llg_event_assign(llg_event_t* target, const llg_event_t* source);
void llg_event_assign_null(llg_event_t* target);

// Wake every current waiter of `ev` and clear its waiter list.
void llg_event_trigger(llg_event_t* ev);
// Return whether the synchronization object was triggered in the current
// simulation time slot. A null handle is never triggered.
int llg_event_triggered(const llg_event_t* ev);
// Queue a nonblocking event trigger for the NBA region. The event pointer is
// copied into runtime-owned queue state, so the issuing process may finish
// before the trigger commits.
void llg_nba_event(llg_event_t* ev);
// Queue a nonblocking event trigger after `ticks`; zero stays in the current
// time slot's NBA region, while a positive delay enters the timed NBA queue.
void llg_nba_event_after(llg_event_t* ev, uint64_t ticks);
// Suspend until `ev` is triggered.
void llg_wait_event(llg_event_t* ev);
// Suspend until any of `evs` is triggered (one atomic registration).
void llg_wait_events(const llg_event_t* const* evs, int n);
// Suspend until the event's persistent same-time-slot triggered state is set.
// If it is already set in the current slot, this returns immediately.
void llg_wait_event_triggered(const llg_event_t* ev);
// Suspend until the listed event objects trigger in order. Repeated events
// already consumed are ignored; a future event arriving early fails the
// monitor and stores a negative result in `result`.
void llg_wait_order(const llg_event_t* const* evs, int n, int* result);

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

// Evaluators and dependencies refer to model storage. The runtime copies
// every descriptor and dependency array before suspending the caller.
// Callbacks must not suspend or mutate scheduler-observed storage. When a
// generated descriptor supplies eval_context or condition_context, it passes
// ownership of one initial llg_frame_t reference to the expression wait; the
// wait releases that reference on wake, cancellation, or runtime teardown.
typedef struct {
    sv4_t* sig;
    llg_mon_eval_fn eval;
    llg_mon_eval_fn condition;
    const llg_event_t* event;
    // Resolved at wait registration so a later handle assignment does not
    // move or suppress this expression waiter.
    llg_event_object_t* event_object;
    sv4_t** reads;
    int n_reads;
    int kind;
    double* real_sig;
    llg_real_eval_fn real_eval;
    llg_wait_dependency_t* dependencies;
    int n_dependencies;
    int real;
    void* eval_context;
    void* condition_context;
} llg_expr_event_spec_t;
void llg_wait_expressions(const llg_expr_event_spec_t* specs, int n);
// Register a nonblocking trigger whose source control is evaluated at issue
// time. The copied descriptors remain live until one source matches; the
// target event is then submitted to the ordinary NBA queue. `repeat` is zero
// for a no-op request and otherwise the number of matches required.
void llg_nba_event_when(const llg_expr_event_spec_t* specs, int n,
                        llg_event_t* target, uint64_t repeat);
// Register a nonblocking assignment whose source control is evaluated at
// issue time. The runtime owns `frame` until the source matches or teardown;
// `action` submits the detached NBA using the captured frame values. A zero
// repeat count invokes `action` immediately, without registering a waiter.
typedef void (*llg_event_assignment_fn)(llg_frame_t* frame);
void llg_nba_event_assign_when(const llg_expr_event_spec_t* specs, int n,
                               uint64_t repeat,
                               llg_event_assignment_fn action,
                               llg_frame_t* frame);
// Normalize a packed repeat count without truncating values wider than 64 bits.
uint64_t llg_repeat_count(sv4_t value);

// One-shot region callback hooks. `data` remains caller-owned and is passed
// unchanged. A zero delay schedules the callback in the current time slot;
// positive delays enter the timed callback queue. Writable iterative regions
// may re-enter design/reactive work; read-only phases reject current-slot
// scheduling except for the Observed-to-Reactive assertion handoff.
typedef void (*llg_region_callback_fn)(void* data);
int llg_schedule_region_callback(llg_region_t region,
                                 llg_region_callback_fn callback, void* data);
int llg_schedule_region_callback_after(llg_region_t region,
                                       llg_region_callback_fn callback,
                                       void* data, uint64_t ticks);
// Explicit name for PLI users; currently one-shot and otherwise identical to
// llg_schedule_region_callback.
int llg_register_pli_callback(llg_region_t region,
                              llg_region_callback_fn callback, void* data);

// Register a signal for a copied, immutable value sampled at the beginning of
// each time slot. The returned pointer is runtime-owned and valid until the
// next llg_rt_cleanup. Unregistered signals produce a controlled diagnostic.
void llg_sampled_register(sv4_t* signal);
const sv4_t* llg_sampled_value(const sv4_t* signal);
int llg_sampled_copy(const sv4_t* signal, sv4_t* out);
// Clocking input copies. Observed copies are queued into the current time
// slot's observed region; history copies read the preponed sample at or before
// `ticks` simulation ticks in the past.
int llg_clocking_sample_observed(sv4_t* source, sv4_t* sample);
int llg_clocking_sample_history(sv4_t* source, sv4_t* sample,
                                uint64_t ticks);

// Assignments.  llg_nba records on the current process's list and commits in
// the NBA region; llg_ba writes immediately and notifies waiters.
void llg_nba(sv4_t* target, sv4_t value);
// Capture values now, retaining target storage through the future NBA commit.
// A zero tick delay stays in the current time slot's NBA region.
void llg_nba_after(sv4_t* target, sv4_t value, uint64_t ticks);
void llg_nba_d_after(double* target, double value, uint64_t ticks);
void llg_string_nba_after(llg_string_t* target, llg_string_t value,
                          uint64_t ticks);
// Merge only known-one mask positions into the target at commit time.
void llg_nba_masked(sv4_t* target, sv4_t value, sv4_t mask, uint64_t ticks);
void llg_ba(sv4_t* target, sv4_t value);
// Commit a write through a canonical `ref` descriptor immediately. Selected
// aliases update the original storage once, preserving normal wakeups and
// force/continuous-assignment checks.
void llg_ref_write(llg_ref_t* ref, sv4_t value);
void llg_nba_d(double* target, double value);
void llg_ba_d(double* target, double value);

// Procedural continuous assignments. Each generated assignment site has a
// stable identity; executing a new site replaces the target's active binding.
// Ordinary blocking/NBA writes to an active target are ignored. Deassign keeps
// the last driven value, and a live binding remains beneath force/release.
#define LLG_MAX_PCA 4096
void llg_pca_assign(sv4_t* target, sv4_t* enable, uint64_t site, sv4_t value);
void llg_pca_drive(sv4_t* target, sv4_t* enable, uint64_t site, sv4_t value);
void llg_pca_deassign(sv4_t* target);
void llg_pca_assign_d(double* target, sv4_t* enable, uint64_t site, double value);
void llg_pca_drive_d(double* target, sv4_t* enable, uint64_t site, double value);
void llg_pca_deassign_d(double* target);

// ── force / release ───────────────────────────────────────────────────────────
//
// A force entry is an overriding live driver. Its callback is evaluated when
// the force is installed and whenever one of its explicitly registered source
// values changes. Packed targets are described as one or more canonical
// storage parts; a net pointer on a part keeps the underlying driver
// resolution available for release and re-application. No pre-force value is
// saved: variables retain the currently forced value on release, while nets
// are recomputed from their current driver slots.

#define LLG_MAX_FORCE 64

typedef struct {
    sv4_t* target;
    llg_net_t* net;
    int64_t left;
    int64_t right;
    uint32_t width;
    uint32_t value_lsb;
    int two_state;
} llg_force_part_t;

typedef struct {
    sv4_t* sig;
    double* real;
    int is_real;
} llg_force_read_t;

typedef void (*llg_force_eval_fn)(sv4_t* out);
typedef void (*llg_force_real_eval_fn)(double* out);

void llg_force_expr_parts(const llg_force_part_t* parts, int n_parts,
                          uint32_t stream_slice, int stream_right_to_left,
                          llg_force_eval_fn eval,
                          const llg_force_read_t* reads, int n_reads);
void llg_release_parts(const llg_force_part_t* parts, int n_parts,
                       uint32_t stream_slice, int stream_right_to_left);
void llg_force_real(double* target, llg_force_real_eval_fn eval,
                    const llg_force_read_t* reads, int n_reads);
void llg_release_real(double* target);

// Legacy constant-value entry points retained for runtime self-tests and
// hand-written generated models. They use the same live-entry table but have
// no source dependencies.
void llg_force(sv4_t* sig, sv4_t value);
void llg_release(sv4_t* sig);

#ifdef __cplusplus
}
#endif

#endif // LLG_RT_H
