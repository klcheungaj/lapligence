//! codegen — lower an elaborated UHDM design (Surelog v1.87 `-elabuhdm`) to a
//! C11 model for the `llg` runtime (`crate::sim::rt`).
//!
//! # Pipeline
//!
//! [`generate`] builds the owned design database ([`crate::core::db::Db`]) with
//! a single VPI walk, then emits one C file (`model.c`) from the database — no
//! VPI access outside the db build.  Compiled together with `llg_rt.c` and
//! libaco, the model is a standalone simulator executable:
//!
//! - every packed scalar signal becomes a global `sv4_t G_<instance path>_<name>`
//!   (path dots become underscores), starting as all-X; procedural scalar
//!   real/shortreal signals use `double` storage; every unpacked array of packed
//!   elements becomes a flat `sv4_t G_<path>_<name>[N]` (product of the
//!   dimension sizes) with
//!   indexed access lowered to guarded element reads/writes (out-of-range or
//!   unknown indices read X / no-op) and declaration initializers. Array
//!   patterns and scalar variable initializers are applied in `main()` before
//!   any process runs; true-net declaration assignments use the same
//!   event-driven processes as explicit continuous assignments;
//! - every parameter becomes an inline packed or real constant (`SV4_*` up to
//!   64 packed bits, `sv4_from_limbs` beyond — see [`emit_const`]) whose values
//!   come from the database, resolved through `core::elab::Resolver`; the
//!   parameter object's own value is stale for overridden parameters);
//! - every process becomes a coroutine function `p_<path>_<n>` (including
//!   processes inside generate scopes, named by their gen-scope path):
//!   `initial` runs once then calls `llg_proc_done`; `always` loops
//!   `for (;;) { <body> }` and the body must contain a blocking wait;
//! - every `fork … join` statement becomes a fork group: each branch is its
//!   own coroutine function (`p_<path>_fork_<n>_b<k>`) spawned with
//!   `llg_fork` and joined per `join_kind` (join / join_any / join_none);
//!   `wait fork;` and `disable fork;` lower to `llg_wait_fork()` /
//!   `llg_disable_fork()`;
//! - every continuous assignment becomes a comb process that evaluates at
//!   spawn and re-evaluates when any signal its RHS reads changes
//!   (`llg_wait_any` on the RHS's read set — Verilator-style semantics);
//! - every structural builtin gate (`and`/`or`/`nand`/`nor`/`xor`/`xnor`,
//!   `buf`/`not`, `bufif0/1`, `notif0/1`, `pullup`/`pulldown`) becomes ONE
//!   comb process shaped exactly like a continuous assignment: evaluate at
//!   spawn, then re-evaluate when any input-terminal signal changes and
//!   write the output terminal with a whole-signal blocking write.
//!   N-input gates reduce left-to-right; nand/nor/xnor negate after the
//!   full reduce; enable gates normalize the passing arm with `data|data`
//!   (per-bit z→x, LRM 1364-1995 §7.4 Table 7-5: an enabled gate acts like
//!   buf/not) and lower to `sv4_mux(en, data|data, Z)` /
//!   `sv4_mux(en, Z, ~(data|data))`; pullup/pulldown are constant RunOnce
//!   drivers; gate delays `#D` prepend a scaled wait to the process body with
//!   no pulse filtering (warned).  Switch/transistor primitives, UDP
//!   instances, primitive arrays, drive strengths, select terminals and width
//!   mismatches are rejected at lowering time;
//! - every child-instance port pair gets a link process copying the parent
//!   side to the child side (inputs) or the child side to the parent side
//!   (outputs) whenever the source changes;
//! - every inout port collapses its parent + child nets into ONE resolved
//!   simulated net (`llg_net_t`, LRM §23.3.3.7): each member net gets a
//!   driver slot, whole-signal writes lower to `llg_net_write`, and every
//!   read goes through the shared resolution cell (wire/tri, equal
//!   strengths).  Inout ports emit no link; unsupported groups are skipped
//!   with an explicit warning;
//! - every interface port gets dedicated link processes between the per-port
//!   copy's vars and the actual interface instance's vars (matched by name;
//!   modport io_decls wired in their declared direction, bare interface ports
//!   wired bidirectionally);
//! - `main()` initializes the runtime, spawns every process in a deterministic
//!   order (comb, links, then always/initial), registers every final block,
//!   runs the scheduler, then executes the finals phase (`llg_rt_run_finals`)
//!   after the scheduler exits ($finish / deadlock / no future events).
//!
//! # Supported / rejected constructs
//!
//! Supported statements: `begin`, `if`/`if-else`, blocking and non-blocking
//! assignment (whole signal, bit-select, part-select and indexed-part-select
//! LHS, plus array-element LHS with element bit/part-selects),
//! procedural continuous assignment/deassignment (`assign x = e;` /
//! `deassign x;`, one enable-guarded process per site — see below),
//! `@(...)` event control with edge or plain sensitivity (or-lists),
//! `@*`/`always_comb` (sensitivity derived from the body), `#delay`, `case`/
//! `casez`/`casex` (casez/casex match per LRM 12.5.1 wildcards), `for`/
//! `while`/`repeat`/`forever`, `wait (cond)` (level-sensitive blocking:
//! re-evaluates the condition on changes of its read signals, then runs the
//! body once), function/task calls (functions and delay-free
//! tasks become C functions with a recursion depth guard; delay-bearing tasks
//! are inlined at their call sites; defaults — including defaults referencing
//! earlier formals — are supported), `fork … join`/`join_any`/`join_none` (named
//! forks included), `wait fork;`, `disable fork;`, `$display`/`$write`,
//! `$monitor`/`$monitoron`/`$monitoroff`/`$strobe`, `$finish`.  Supported
//! processes include `final begin … end` blocks (SV 1800-2005 §10.7): lowered
//! like `initial` but executed once AFTER the scheduler exits ($finish,
//! deadlock or no future events); timing controls inside a final are
//! rejected.  Supported
//! expressions: constants, casts
//! (`'(type)(expr)`), operations (arithmetic, bitwise, logical, reductions,
//! shifts, comparisons, mux, concat/replicate), refs, bit/part/indexed-part
//! selects, array-element selects (`mem[i]`, `a[i][j]`, `mem[i][3:0]`),
//! `$clog2`, `$bits`, `$signed`/`$unsigned`, `$time`, and hierarchical
//! references (`top.u0.sig` — N-part paths whose final element resolves to a
//! signal) on both the READ and WRITE sides of an assignment; hierarchical
//! write targets may carry a trailing select (`top.u0.sig[3:0]`,
//! `top.u0.sig[2]`, `top.u0.sig[3 +: 4]`) with constant integer
//! indices/bounds only (the trailing select is recovered from the node name /
//! source line — Surelog v1.86's elaborated model drops part-select bounds
//! and only keeps constant bit-select indices in the object name).
//!
//! The supported real subset covers procedural scalar variables, real and
//! shortreal parameters, mixed arithmetic and conditions, packed/real casts and
//! assignment, NBA assignment, shortreal rounding, and `%f`/`%e`/`%g` display.
//! Packed-to-real conversion uses all limbs and treats X/Z bit positions as zero;
//! real-to-packed conversion rounds halves away from zero and is limited to 64
//! target bits.
//!
//! Rejected with an `Err`: fork/join inside a function/task body, task calls
//! inside function bodies, recursive delay-bearing tasks, hierarchical
//! (cross-instance) function/task calls, string/class signals and string
//! parameters, unsupported real contexts (ports, arrays, function/task types,
//! continuous/combinational processes, and double-aware scheduling), widths
//! above `LLG_MAX_WIDTH` (1024) bits, and division/modulo/power with an
//! operand wider than 64 bits,
//! hierarchical WRITES whose final path element does not resolve to a
//! per-instance signal, hierarchical write targets with variable or
//! expression select indices/bounds, select LHS or
//! nonblocking assignment on a collapsed inout-net member (the group scan
//! normally skips such groups with a warning before emission; these errors
//! are a backstop), variable declaration initializers whose RHS is not a
//! constant expression (`logic z = a;` is rejected), true-net declaration
//! assignments that read unpacked arrays or use unsupported resolved-net
//! classes, and unknown
//! `$display`/`$monitor`/`$strobe` format specifiers (`%s` in
//! monitors/strobes).  Structural primitives outside the supported builtin
//! set are rejected with explicit messages: switch/transistor primitives,
//! UDP instances, primitive (gate/UDP) arrays, drive-strength specifications,
//! select/expression/hierarchical terminals, terminal width mismatches (LRM
//! same-width rule), multi-output `buf`/`not`, gates with more than
//! [`LLG_MAX_GATE_TERMS`] terminals, and non-constant gate delays.  Array
//! constructs rejected with a clear message include
//! dimension bounds that are not plain constants (an implicit `[N]` size —
//! declare `[0:N-1]` explicitly), array slices (partial indexing of a
//! multi-dimensional array), indexed part-selects on an array element, and
//! non-constant declaration-initializer elements. `$displayon`/
//! `$displayoff` are skipped with a warning. Waveform controls (`$dumpfile`/
//! `$dumpvars`/`$dumpon`/`$dumpoff`/`$dumpall`/`$dumpflush`/`$dumplimit`)
//! lower explicitly into the IR.
//! Interface instances are captured by the database walk (actuals and
//! per-port copies), including inside generate scopes.  Interface body
//! processes (always/initial/always_comb blocks inside an interface
//! definition) are emitted for the ACTUAL interface instance only — the
//! definition's processes are not cloned into the per-port copies (verified
//! against Surelog v1.86 elaboration), which are just views kept in sync by
//! the interface link processes.
//!
//! # Database-driven deviations from the VPI-based codegen
//!
//! The database ([`crate::core::db::Db`]) does not capture every property the
//! old handle-walking codegen read, so a few v1 behaviors changed (none are
//! exercised by the test suite):
//!
//! - Array and scalar variable declaration initializers are applied in
//!   `main()` before any process runs. `reg y = 0` arrives through
//!   `ContAssign { net_decl: true }`; `logic l = 0`/`int x = 5` arrive through
//!   `Db::vars_init`. True-net forms (`wire`/`tri`/logic-net) are continuous
//!   drivers: constant RHSs run once and dynamic RHSs use a precomputed,
//!   deduplicated sensitivity set.
//! - Port connections through a bit/part select are linked on the base signal
//!   instead of being warned and skipped (the connection's select-ness is not
//!   captured; `high`/`low` resolve to the base net/var).
//! - Whole-signal `force`/`release` and null statements are emitted;
//!   procedural continuous assignment/deassignment lower to enable-guarded
//!   processes (see `lower_proc_cont_assign`).
//! - Event expressions with non-or/edge operations (e.g. `@(a && b)`) wait on
//!   the body's read set instead of the condition's operands.
//! - The signal declaration order in the emitted C is deterministic
//!   (collection order); the old codegen iterated a hash map, so its order
//!   varied between runs.
//!
//! # v1 limitations (documented)
//!
//! - X and Z are stored and displayed distinctly (`$display` prints 'x' vs
//!   'z'; `===`/`!==` compare them literally), and casez/casex wildcard
//!   matching is supported per LRM 12.5.1.  In every other expression context
//!   Z behaves as X (LRM 11.4.5), and identity/copy ops (mux with a known
//!   select, selects, resize, concat) carry Z through unchanged.
//! - Vectors may be up to `LLG_MAX_WIDTH` (1024) bits wide, but
//!   division/modulo/power accept operands of at most 64 bits (the runtime
//!   returns all-X for wider operands; the codegen rejects them up front).
//! - Timescale is honored per file: `#N` delays scale by the calling module's
//!   time unit (`timescale unit/precision`, parsed from the first directive
//!   of the source file; modules without a directive default to 1ns/1ps with
//!   a TIMESCALEMOD-style warning), and `$time` returns the current time in
//!   the calling module's unit.  The scheduler runs in design-precision ticks
//!   (the finest precision across the design), so the runtime itself is
//!   timescale-agnostic.
//! - Unsized fill literals (`'1`) are filled correctly only when they are the
//!   entire RHS of an assignment; inside expressions they act as 1-bit values.
//! - Generate-block processes are supported: processes inside gen scopes are
//!   emitted exactly like instance processes, with genvar references inlined
//!   to the gen-scope parameter values.  Gen-scope continuous assignments and
//!   genvar parameter references are supported.
//! - An `always` whose body contains no wait (no delay/event control and no
//!   `wait (cond)`) is treated as combinational: it
//!   evaluates once at t=0, then re-runs when any signal it reads changes (a
//!   warning is emitted when it reads nothing, and it runs once).
//! - Intra-assignment delays (`a = #5 b;`, `a <= #5 b;`) evaluate the RHS
//!   into a temp immediately and apply it after the scaled delay; the
//!   executing process suspends across the window for BOTH assignment kinds
//!   (LRM 1364-1995 §9.7.4 lets a nonblocking assignment continue without
//!   blocking — v1 approximation).  Event/repeat-controlled and
//!   parameterized forms are rejected.  Continuous-assignment delays
//!   (`assign #N lhs = rhs;`) delay every write by N after the triggering
//!   rhs change, including at t=0; there is no pulse filtering — each wake
//!   writes the CURRENT rhs value D later (warned).

#![allow(non_upper_case_globals)]

use std::collections::{HashMap, HashSet};

use crate::core::db::{
    Db, EventSpec, ExprKind, IntraControl, NodeId, NodeKind, PrimClass, ProcessKind, StmtKind,
};
use crate::core::elab::{self, Bit, Val};
use crate::ffi::vpi::{self, ValueData, VpiHandle};
use crate::sim::emit_c::{
    escaped_char, event_global_name, global_name, ident, real_global_name, render_expr, strip_lib,
    RCtx,
};
use crate::sim::ir::{
    IrBinOp, IrCall, IrCallArg, IrCallExpr, IrCaseItem, IrCaseKind, IrConst, IrDepth, IrEdge,
    IrElemSel, IrEvent, IrExpr, IrExprKind, IrFormal, IrJoinKind, IrLhs, IrModel, IrProcess,
    IrRealBinOp, IrRealUnOp, IrShape, IrSignal, IrStmt, IrSysFunc, IrType, IrUnOp, IrWaitSrc,
    LLG_MAX_WIDTH,
};

/// Maximum driver slots of one collapsed inout net.  Keep in sync with
/// `LLG_MAX_NET_DRIVERS` in `src/sim/rt/llg_rt.h` (16).
const LLG_MAX_NET_DRIVERS: usize = 16;

/// Sanity cap on the terminal count of one structural gate.
const LLG_MAX_GATE_TERMS: usize = 64;

/// The lowered computation of one builtin gate primitive
/// ([`NodeKind::Gate`]).
enum GateOp {
    /// `and`/`or`/`xor` (and the negated nand/nor/xnor): reduce the inputs
    /// left-to-right with the two-input op, then optionally negate.
    Reduce(IrBinOp, bool),
    /// `buf`: identity copy of the single input.
    Copy,
    /// `not`: bitwise negation of the single input.
    Not,
    /// `bufif0/1`, `notif0/1`: three-terminal enable gates — an ACTIVE
    /// enable passes the (optionally inverted) data through like `buf`/`not`
    /// (LRM 1364-1995 §7.4 Table 7-5), an INACTIVE one drives Z.  Lowered to
    /// `sv4_mux(en, data|data, Z)` / `sv4_mux(en, Z, ~(data|data))`: the
    /// mux is an identity/copy context that would carry a data Z verbatim,
    /// so the passing arm is z→x-normalized with `data|data` first.
    Enable { invert_out: bool, active_high: bool },
    /// `pullup`/`pulldown`: constant 1/0 driver over the terminal width.
    Pull(bool),
}

/// A real expression is represented by width zero; packed values are never
/// zero-width in the lowering.  This keeps lowered expressions compact while
/// making accidental vector operations easy to reject.
const REAL_EXPR_WIDTH: u32 = 0;

/// Maximum function/task call nesting in the generated C; the depth guard at
/// the top of every emitted function returns all-X beyond this.
const LLG_MAX_FUNC_DEPTH: u32 = 256;

/// VPI case statement subtypes (vendor/Surelog/third_party/UHDM/include/
/// vpi_user.h): `vpiCaseExact` = 1 (`case`, re-exported from `ffi::vpi`),
/// `vpiCaseX` = 2 (`casex`), `vpiCaseZ` = 3 (`casez`).  Defined here rather
/// than in `ffi::vpi` to keep the FFI module untouched.
const VPI_CASE_X: i32 = 2;
const VPI_CASE_Z: i32 = 3;

/// The generated C model plus non-fatal warnings collected while lowering.
pub struct GeneratedModel {
    /// Complete `model.c` source: `#include "llg_rt.h"`, signal globals,
    /// process functions, and `main`.
    pub model_c: String,
    /// The design name (also embedded in the model's first comment line).
    pub design_name: String,
    /// Non-fatal warnings (unsupported constructs that were skipped or
    /// degraded, e.g. `$dumpvars` skipped).
    pub warnings: Vec<String>,
}

/// Lower the elaborated design into C11 with the default optimization
/// configuration (all passes on).  Call while the surelog session is alive
/// (the design handle is only valid then); returns `Err` with a message
/// naming the construct and instance when the design uses something outside
/// the supported subset.
pub fn generate(design: VpiHandle) -> Result<GeneratedModel, String> {
    generate_with_opts(design, &crate::sim::opt::OptConfig::default())
}

/// Lower the elaborated design into C11 with an explicit optimization
/// configuration.  See [`generate`] for the calling contract.
pub fn generate_with_opts(
    design: VpiHandle,
    cfg: &crate::sim::opt::OptConfig,
) -> Result<GeneratedModel, String> {
    let db = Db::build(design)?;
    generate_from_db_with_opts(&db, cfg)
}

/// Lower an already-owned database with an explicit optimization
/// configuration.
///
/// Reuse this entry point when producing multiple variants of one elaborated
/// design. It avoids repeated VPI traversal and is robust to frontend
/// relationships that can be consumed while building the owned database.
pub fn generate_from_db_with_opts(
    db: &Db,
    cfg: &crate::sim::opt::OptConfig,
) -> Result<GeneratedModel, String> {
    let mut cg = Codegen::new(db);
    cg.collect_iface_copies();
    let tops = cg.collect_design()?;
    if tops.is_empty() {
        return Err("no top modules in the elaborated design".to_string());
    }
    // Collapse inout-port net groups (parent + child nets → one resolved
    // simulated net) before any signal/process lowering so member reads and
    // writes use the resolution cell.
    cg.build_net_groups()?;
    // Timescales must be fixed before any `#delay`/`$time` is lowered so the
    // design precision (scheduler tick unit) is consistent across the model.
    cg.collect_timescales();
    // Two-phase PCA site discovery, phase 1: allocate every procedural
    // continuous `assign <var> = …;` site BEFORE any body lowers (see
    // `prescan_pca_sites`), so `deassign` lowering never depends on process
    // order.
    for top in &tops {
        let path = cg.instance_path_of(*top);
        cg.prescan_pca_sites(*top, &path)?;
    }
    // Functions/tasks become static C functions (prototypes first so bodies
    // may call each other regardless of declaration order), lowered before any
    // process code references them.
    for top in &tops {
        cg.emit_func_prototypes(*top)?;
    }
    for top in &tops {
        cg.emit_func_bodies(*top)?;
    }
    // Three passes over the instance tree so every comb process, then every
    // link, then every always/initial process runs at t=0 in that order;
    // push order equals spawn order.
    for top in &tops {
        cg.emit_pass(*top, Pass::Comb)?;
    }
    for top in &tops {
        cg.emit_pass(*top, Pass::Links)?;
    }
    for top in &tops {
        cg.emit_pass(*top, Pass::Procs)?;
    }
    let mut model = std::mem::take(&mut cg.model);
    cg.build_init_steps(&mut model)?;
    // Final-block processes render like any other but spawn into the
    // post-simulation phase: they run after `llg_rt_run` returns, not at
    // t=0.
    let final_names = std::mem::take(&mut cg.final_procs);
    model.spawns = model
        .processes
        .iter()
        .map(|p| p.c_name.clone())
        .filter(|n| !final_names.contains(n))
        .collect();
    model.final_spawns = final_names;
    crate::sim::opt::run(&mut model, cfg);
    // Lowering-time invariant: signal C names must be unique — a collision
    // would silently merge two variables' storage in `render_signal_decls`.
    // Synthesized PCA enables embed a `$`, which ident()-sanitized user
    // names can never produce, so this only fires when two identifiers
    // sanitize to the same spelling.  Skips exactly what the renderer
    // skips: collapsed-net members share the group's `<net>.resolved` cell
    // on purpose, and omitted signals are pruned before emission.
    let mut seen_names: HashSet<&str> = HashSet::with_capacity(model.signals.len());
    for sig in &model.signals {
        if sig.net_driver.is_some() || sig.omit {
            continue;
        }
        if !seen_names.insert(sig.c_name.as_str()) {
            return Err(format!(
                "C signal name `{}` is not unique (two signals sanitize to \
                 the same identifier); refusing to merge their storage",
                sig.c_name
            ));
        }
    }
    let model_c = crate::sim::emit_c::render(&model)?;
    Ok(GeneratedModel {
        design_name: cg.design_name.clone(),
        model_c,
        warnings: cg.warnings,
    })
}

// ── Collected model info ──────────────────────────────────────────────────────

#[derive(Clone)]
struct SignalInfo {
    global: String,
    width: u32,
    signed: bool,
    real: bool,
    shortreal: bool,
    /// For members of a collapsed inout-net group: `(net C name, driver
    /// slot)`.  `global` is then `<net>.resolved` so every read goes through
    /// the resolution cell; whole-signal writes lower to `llg_net_write`.
    net_driver: Option<(String, usize)>,
    /// Index into [`Codegen::model`].signals.
    ir: usize,
}

/// A lowered unpacked array: a flat C array of `sv4_t` elements plus the
/// per-dimension metadata needed to linearize indices.
#[derive(Clone)]
struct ArrayInfo {
    global: String,
    /// Element vector width in bits.
    elem_width: u32,
    signed: bool,
    /// `(left, right)` per declared dimension, in declaration order.
    dims: Vec<(i32, i32)>,
    /// Declaration-initializer constants (`'{…}` pattern) in linear-index
    /// order, when the array is initialized at declaration.
    init: Option<Vec<IrConst>>,
    /// Index into [`Codegen::model`].arrays.
    ir: usize,
}

/// A lowered named event (`event ev;`): its waiter-table C name and index
/// into [`Codegen::model`].events.  Events carry no value — only triggers and
/// waits reference them.
#[derive(Clone)]
struct EventInfo {
    global: String,
    /// Index into [`Codegen::model`].events.
    ir: usize,
}

/// An element-level select applied after the array index (the last `vpiIndex`
/// of a `mem[addr][3:0]`-style `var_select`).
enum ElemSel {
    /// Whole element.
    Whole,
    /// Part-select `[left:right]` of the element.
    Part(i128, i128),
    /// Bit-select of the element by a runtime index expression.
    Bit(String),
}

/// LHS of an assignment to one array element (with optional element-level
/// bit/part select).
struct ArrayElemLhs {
    arr: ArrayInfo,
    /// One C expression per dimension index, in declaration order.
    index_codes: Vec<String>,
    elem_sel: ElemSel,
}

/// One procedural continuous assignment site (`assign <var> = …;` /
/// `deassign <var>;`): the enable guard's storage plus which statement node
/// materialized the guard process.
struct PcaSite {
    /// Enable-signal IR index (`G_<path>_pca$<n>_en`, starts X = disabled).
    en: usize,
    /// ProcContAssign arena node whose lowering created the guard process;
    /// `None` between pre-scan allocation and first lowering.  A second,
    /// DIFFERENT node reaching an occupied site is the
    /// multiple-active-sites reject; the SAME node again (a delay-bearing
    /// task body inlined at several call sites) reuses the existing site
    /// and guard.
    guarded_by: Option<NodeId>,
}

/// A call argument bound to one formal: its width/signedness and the arena
/// node of the actual expression (the bound argument, or the formal's default
/// when the call omits it).
struct BoundArg {
    width: u32,
    signed: bool,
    expr: NodeId,
    /// `true` when `expr` is the formal's default expression rather than a
    /// caller-provided argument.  Default expressions are emitted under a
    /// temporary formal-aware context so references to earlier formals resolve
    /// to the bound arguments.
    is_default: bool,
}

#[derive(Copy, Clone, PartialEq)]
enum Pass {
    Comb,
    Links,
    Procs,
}

/// How an assignment LHS touches a collapsed-net member set.
#[derive(Copy, Clone, PartialEq)]
enum MemberWrite {
    /// Not a member.
    None,
    /// Whole-signal write: lowered to `llg_net_write`.
    Whole,
    /// Bit/part/indexed-part/array select write: unsupported on members.
    Select,
}

/// Declaration kind targeted by a `vpiNetDeclAssign`. Surelog uses that
/// marker for both true-net continuous drivers and legacy reg declaration
/// initializers, whose simulator scheduling is intentionally different.
#[derive(Copy, Clone)]
enum NetDeclTarget {
    Array,
    Variable,
    TrueNet,
    UnsupportedNet(i32),
    Unknown,
}

struct Codegen<'a> {
    db: &'a Db,
    warnings: Vec<String>,
    /// The typed IR being built (signals/arrays/functions/processes); the C
    /// renderers in `emit_c` consume it.  During the seam transition the
    /// string emitter below still produces the final text.
    model: IrModel,
    /// Model index of the function whose body is currently being emitted
    /// (`None` outside function bodies); formal reads resolve through it.
    cur_fn_ir: Option<usize>,
    /// FuncTask arena node → call-site resolution metadata (model index,
    /// signature).  Emitted functions only; registered by the prototype walk.
    func_meta: HashMap<NodeId, FuncMeta>,
    /// All lowered signals, in collection order (deterministic emission).
    signals: Vec<SignalInfo>,
    /// Net/Var arena node → lowered signal info (all instances + gen scopes).
    sig_globals: HashMap<NodeId, SignalInfo>,
    /// Legacy storage for scalar declaration-initializer fills that need a
    /// collapsed-net driver slot. True-net declarations now lower as
    /// continuous processes, so ordinary wire/tri entries do not use it.
    net_inits: Vec<(String, usize, IrConst)>,
    /// All lowered arrays, in collection order (deterministic emission).
    arrays: Vec<ArrayInfo>,
    /// Array arena node → lowered array info.
    array_globals: HashMap<NodeId, ArrayInfo>,
    /// All lowered named events, in collection order (deterministic emission).
    events: Vec<EventInfo>,
    /// NamedEvent arena node → lowered event info.
    event_globals: HashMap<NodeId, EventInfo>,
    /// (signal info, constant) declaration-initializer fills for scalar
    /// variable-like net objects (`reg y = 0;`), applied in `main()` before
    /// any process runs (mirrors the array declaration-initializer handling).
    scalar_inits: Vec<(SignalInfo, IrConst)>,
    /// (signal info, constant) declaration-initializer fills for scalar
    /// VARIABLES whose initializer lives on the var's `vpiExpr` (`logic
    /// l = 1'b0;`, `int x = 5;` — captured in `Db::vars_init`), applied in
    /// `main()` after the net-decl fills and before any process runs.
    var_inits: Vec<(SignalInfo, IrConst)>,
    /// ContAssign arena nodes already collected as scalar variable
    /// declaration initializers; skipped at emission. True-net declaration
    /// assignments remain event-driven continuous-assignment processes.
    scalar_init_ca: HashSet<NodeId>,
    /// instance/gen-scope path → (array name → info), for name fallback.
    scope_array_names: HashMap<String, HashMap<String, ArrayInfo>>,
    /// Param arena node → resolved value.
    param_vals: HashMap<NodeId, Val>,
    /// instance/gen-scope path → (signal name → info), for name fallback.
    scope_sig_names: HashMap<String, HashMap<String, SignalInfo>>,
    /// gen_scope node → its path (e.g. "top.genblk").
    gen_scope_paths: HashMap<NodeId, String>,
    /// Actual interface members already driven by an interface output link
    /// (last-writer-wins; used for the multiple-driver warning).
    iface_driven: HashSet<String>,
    /// Per-port COPY interface instance arena nodes: the `low` targets of
    /// interface-typed ports (a `ModPort`'s owning interface, or a bare-port
    /// `ModuleInst` directly).  Interface body processes are emitted only for
    /// the ACTUAL interface instances — the copies are just views and never
    /// carry the definition's processes (verified in Surelog v1.86
    /// elaboration).
    iface_copy_insts: HashSet<NodeId>,
    /// FuncTask arena node → emitted C function name.
    func_names: HashMap<NodeId, String>,
    /// Current function/task body context while emitting one (`None` in
    /// process and continuous-assignment contexts).  Expression and LHS
    /// resolution consult it to map formals, locals and the return variable;
    /// the owning [`EmitCtx`] keeps its own copy for statement-level logic.
    func: Option<FuncCtx>,
    /// C expression for the recursion depth at call sites in the current
    /// context (`"0"` in processes, `"depth + 1"` in function bodies).
    depth_arg: String,
    /// Owning module-instance arena node of the current emission context;
    /// used to resolve unbound callees by name.
    inst: NodeId,
    design_name: String,
    proc_seq: usize,
    /// Timescale per source file (parsed once per file; files without a
    /// `timescale directive get the 1ns/1ps default).
    file_timescale: HashMap<String, Timescale>,
    /// Source files already warned about a missing `timescale directive.
    warned_no_timescale: HashSet<String>,
    /// Design time precision in ps: the finest precision across every module,
    /// which sets the scheduler tick unit (1 tick = `design_precision_ps` ps).
    design_precision_ps: u64,
    /// Procedural continuous assignment sites: target signal IR index →
    /// site.  Sites are allocated by a pre-scan over ALL process bodies
    /// BEFORE any body lowers ([`Codegen::prescan_pca_sites`]), so a
    /// `deassign` finds its site regardless of process/source order; at
    /// most ONE site per variable (a second `assign` on the same variable
    /// is rejected).
    pca_sites: HashMap<usize, PcaSite>,
    /// PCA site sequence, used for unique enable-global names
    /// (`G_<path>_pca$<n>_en`; the `$` can never occur in an
    /// ident()-sanitized user name, so synthesized enables cannot collide
    /// with a user variable's global).
    pca_seq: usize,
    /// Final-block process function names (`ProcessKind::Final`), in
    /// emission order — spawned into [`IrModel::final_spawns`] instead of
    /// the t=0 spawn list.
    final_procs: Vec<String>,
    /// Whether the one model-wide `$dumpvars` filtering approximation warning
    /// has already been emitted.
    warned_dumpvars_filtering: bool,
}

impl<'a> Codegen<'a> {
    fn new(db: &'a Db) -> Codegen<'a> {
        Codegen {
            db,
            warnings: Vec::new(),
            model: IrModel {
                precision_ps: Timescale::DEFAULT.precision_ps,
                ..IrModel::default()
            },
            cur_fn_ir: None,
            func_meta: HashMap::new(),
            signals: Vec::new(),
            sig_globals: HashMap::new(),
            net_inits: Vec::new(),
            arrays: Vec::new(),
            array_globals: HashMap::new(),
            events: Vec::new(),
            event_globals: HashMap::new(),
            scalar_inits: Vec::new(),
            scalar_init_ca: HashSet::new(),
            var_inits: Vec::new(),
            scope_array_names: HashMap::new(),
            param_vals: HashMap::new(),
            scope_sig_names: HashMap::new(),
            gen_scope_paths: HashMap::new(),
            iface_driven: HashSet::new(),
            iface_copy_insts: HashSet::new(),
            func_names: HashMap::new(),
            func: None,
            depth_arg: "0".to_string(),
            inst: NodeId(0),
            design_name: String::new(),
            proc_seq: 0,
            file_timescale: HashMap::new(),
            warned_no_timescale: HashSet::new(),
            design_precision_ps: Timescale::DEFAULT.precision_ps,
            pca_sites: HashMap::new(),
            pca_seq: 0,
            final_procs: Vec::new(),
            warned_dumpvars_filtering: false,
        }
    }

    /// The node at `id` (borrowed from the database, not from `self`).
    fn node(&self, id: NodeId) -> &'a crate::core::db::Node {
        self.db.node(id)
    }

    /// The kind of the node at `id` (borrowed from the database).
    fn kind(&self, id: NodeId) -> &'a NodeKind {
        self.db.node_kind(id)
    }

    /// Instance path used for global names: `"top.u0"` for child instances,
    /// the (library-prefix-stripped) name for top instances.
    fn instance_path_of(&self, id: NodeId) -> String {
        let path = self.db.instance_path(id);
        if path.is_empty() {
            strip_lib(&self.node(id).name)
        } else {
            path
        }
    }

    /// Lowered info for a Net/Var arena node, if it was collected.
    fn signal_of(&self, id: NodeId) -> Option<&SignalInfo> {
        self.sig_globals.get(&id)
    }

    /// Lowered info for an Array arena node (or a Ref resolving to one), if
    /// the array was collected.
    fn array_of(&self, node: NodeId) -> Option<&ArrayInfo> {
        match self.kind(node) {
            NodeKind::Array { .. } => self.array_globals.get(&node),
            NodeKind::Expr(ExprKind::Ref { target }) => {
                target.and_then(|t| self.array_globals.get(&t))
            }
            _ => None,
        }
    }

    /// Resolve a hierarchical reference read (`a.b.sig`, or the 2-part
    /// interface member `m.data`) to its signal, when the LAST path element
    /// resolves to a captured Net/Var (per-instance, via the db's refs).
    /// Longer or unresolvable paths return `None`.
    fn hier_path_signal(&self, node: NodeId) -> Option<&SignalInfo> {
        if let NodeKind::Expr(ExprKind::HierPath { refs, .. }) = self.kind(node) {
            if let Some(t) = refs.last().copied().flatten() {
                return self.signal_of(t);
            }
        }
        None
    }

    /// Timescale of the source file at `path`, parsed once and cached.  Files
    /// without a `timescale directive (or unreadable files) get the 1ns/1ps
    /// default plus a TIMESCALEMOD-style warning the first time they are seen.
    fn timescale_of_file(&mut self, path: &str) -> Timescale {
        if let Some(ts) = self.file_timescale.get(path) {
            return *ts;
        }
        let found = std::fs::read_to_string(path)
            .ok()
            .and_then(|text| parse_timescale(&text));
        let ts = found.unwrap_or(Timescale::DEFAULT);
        if found.is_none() && !path.is_empty() && self.warned_no_timescale.insert(path.to_string())
        {
            self.warnings.push(format!(
                "module in `{path}` has no `timescale directive; assuming \
                 1ns/1ps (Verilator-style TIMESCALEMOD)"
            ));
        }
        self.file_timescale.insert(path.to_string(), ts);
        ts
    }

    /// Timescale of the module whose source file contains `node` (the node's
    /// own `file`, i.e. the file where the delay/`$time` is written).
    fn timescale_of_node(&mut self, node: NodeId) -> Timescale {
        let file = self.node(node).file.clone().unwrap_or_default();
        self.timescale_of_file(&file)
    }

    /// Recover the signed marker of a based literal from its source token.
    /// Surelog v1.86 does not expose `vpiSigned` on `vpiConstant` objects and
    /// returns signed based literals through the unsigned value arm.
    fn signed_based_constant(&self, node: NodeId) -> bool {
        self.signed_based_literal_info(node).0
    }

    /// Recover the explicit width and signed marker of a based literal.  The
    /// elaborated initializer may report the destination width instead of the
    /// literal width, so the source token is also needed for sign extension.
    fn signed_based_literal_info(&self, node: NodeId) -> (bool, Option<u32>) {
        let node = self.node(node);
        if is_signed_based_literal(&node.name) {
            return (true, based_literal_width(&node.name));
        }
        let (Some(file), line) = (node.file.as_deref(), node.line) else {
            return (false, None);
        };
        if line == 0 {
            return (false, None);
        }
        let Ok(content) = std::fs::read_to_string(file) else {
            return (false, None);
        };
        let Some(text) = content.lines().nth(line as usize - 1) else {
            return (false, None);
        };
        let Some(token) = signed_based_literal_token_at(text, node.col) else {
            return (false, None);
        };
        (true, based_literal_width(token))
    }

    /// Parse the timescale of every distinct source file referenced by the
    /// design and fix the design precision (the finest precision, which sets
    /// the scheduler tick unit).  Runs before any emission so every `#delay`
    /// and `$time` scales consistently.
    fn collect_timescales(&mut self) {
        self.design_precision_ps = u64::MAX;
        for top in &self.db.tops {
            self.walk_files(*top);
        }
        for m in &self.db.flat_modules {
            self.walk_files(*m);
        }
        if self.design_precision_ps == u64::MAX {
            // No source files at all (should not happen for a real design).
            self.design_precision_ps = Timescale::DEFAULT.precision_ps;
        }
        self.model.precision_ps = self.design_precision_ps;
    }

    /// Convert the collected declaration initializers into `main()` init
    /// steps, in application order: unpacked-array fills (+ pattern
    /// elements), then scalar net-decl fills, then variable fills, then
    /// collapsed-net member writes.
    fn build_init_steps(&self, model: &mut IrModel) -> Result<(), String> {
        use crate::sim::ir::IrInitStep;
        for ai in &self.arrays {
            model.init_steps.push(IrInitStep::FillArrayX(ai.ir));
            if let Some(vals) = &ai.init {
                for (i, c) in vals.iter().enumerate() {
                    model.init_steps.push(IrInitStep::SetArrayElem {
                        arr: ai.ir,
                        index: i as u64,
                        value: c.clone(),
                    });
                }
            }
        }
        for (info, c) in &self.scalar_inits {
            model.init_steps.push(IrInitStep::SetScalar {
                sig: info.ir,
                value: c.clone(),
            });
        }
        for (info, c) in &self.var_inits {
            model.init_steps.push(IrInitStep::SetScalar {
                sig: info.ir,
                value: c.clone(),
            });
        }
        for (net, slot, c) in &self.net_inits {
            let group = model
                .net_groups
                .iter()
                .position(|g| &g.c_name == net)
                .ok_or_else(|| format!("net group `{net}` not collected"))?;
            model.init_steps.push(IrInitStep::WriteNet {
                group,
                slot: *slot,
                value: c.clone(),
            });
        }
        Ok(())
    }

    /// Visit every node under `node`, parsing the timescale of each distinct
    /// source file (the per-file cache dedupes; the design precision is the
    /// minimum precision seen).
    fn walk_files(&mut self, node: NodeId) {
        let file = self.node(node).file.clone().unwrap_or_default();
        if !file.is_empty() {
            let ts = self.timescale_of_file(&file);
            self.design_precision_ps = self.design_precision_ps.min(ts.precision_ps);
        }
        let kids: Vec<NodeId> = self.node(node).children.clone();
        for c in kids {
            self.walk_files(c);
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn is_real_kind(kind: &str) -> bool {
    matches!(kind, "real" | "shortreal")
}

// ── Union-find (collapsed inout-net groups) ───────────────────────────────────

fn find(parent: &mut HashMap<NodeId, NodeId>, x: NodeId) -> NodeId {
    let mut cur = x;
    while parent.get(&cur).copied() != Some(cur) {
        match parent.get(&cur).copied() {
            Some(p) => {
                // Path halving: point at the grandparent.
                if let Some(gp) = parent.get(&p).copied() {
                    parent.insert(cur, gp);
                }
                cur = p;
            }
            None => {
                parent.insert(cur, cur);
                return cur;
            }
        }
    }
    cur
}

fn union(
    parent: &mut HashMap<NodeId, NodeId>,
    rank: &mut HashMap<NodeId, u8>,
    a: NodeId,
    b: NodeId,
) {
    let ra = find(parent, a);
    let rb = find(parent, b);
    if ra == rb {
        return;
    }
    let (ka, kb) = (
        rank.get(&ra).copied().unwrap_or(0),
        rank.get(&rb).copied().unwrap_or(0),
    );
    if ka < kb {
        parent.insert(ra, rb);
    } else if ka > kb {
        parent.insert(rb, ra);
    } else {
        parent.insert(rb, ra);
        rank.insert(ra, ka + 1);
    }
}

// ── Timescale ─────────────────────────────────────────────────────────────────

/// A Verilog time unit/precision pair, both in picoseconds (`1ns` = 1000 ps,
/// `1ps` = 1 ps).  Modules without a `timescale directive default to 1ns/1ps
/// (Verilator's TIMESCALEMOD behavior).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Timescale {
    unit_ps: u64,
    precision_ps: u64,
}

impl Timescale {
    /// The default timescale for a module without a `timescale directive
    /// (1ns/1ps).
    const DEFAULT: Timescale = Timescale {
        unit_ps: 1_000,
        precision_ps: 1,
    };
}

/// Parse the FIRST `` `timescale <unit>/<precision> `` directive in `text`
/// (a simple text scan; `timescale 1ns / 1ps` with optional whitespace).
/// Returns `None` when the file has no usable directive.
fn parse_timescale(text: &str) -> Option<Timescale> {
    let idx = text.find("`timescale")?;
    let rest = &text[idx + "`timescale".len()..];
    let (unit_ps, rest) = parse_timescale_value(skip_ws(rest))?;
    let rest = skip_ws(rest).strip_prefix('/')?;
    let (precision_ps, _) = parse_timescale_value(skip_ws(rest))?;
    Some(Timescale {
        unit_ps,
        precision_ps,
    })
}

/// Parse `<digits><unit>` (optional whitespace between the digits and the
/// unit letters) at the start of `s`, returning the value in picoseconds and
/// the remainder after the unit.  Units: s/ms/us/ns/ps/fs with 1/10/100
/// multipliers; sub-picosecond (fs) values clamp up to 1 ps so the ps-integer
/// representation stays exact.
fn parse_timescale_value(s: &str) -> Option<(u64, &str)> {
    let (digits, rest) = split_at_while(s, |c| c.is_ascii_digit());
    if digits.is_empty() {
        return None;
    }
    let mult: u64 = digits.parse().ok()?;
    let (unit, rest) = split_at_while(skip_ws(rest), |c| c.is_ascii_alphabetic());
    let base = match unit {
        "s" => 1_000_000_000_000,
        "ms" => 1_000_000_000,
        "us" => 1_000_000,
        "ns" => 1_000,
        "ps" => 1,
        "fs" => 1, // sub-ps: clamp up to 1 ps
        _ => return None,
    };
    Some((mult * base, rest))
}

/// Split `s` at the first character not matching `pred`, returning the
/// matching prefix and the rest.
fn split_at_while(s: &str, pred: impl Fn(char) -> bool) -> (&str, &str) {
    let n = s.find(|c| !pred(c)).unwrap_or(s.len());
    (&s[..n], &s[n..])
}

/// Extract the raw (source-unit) tick count of a folded continuous-assignment
/// delay constant.  Fractional, X/Z, negative or >64-bit values are rejected
/// with a clear message.
fn const_delay_ticks(c: &IrConst, path: &str) -> Result<u64, String> {
    if c.real.is_some() || c.fill.is_some() {
        return Err(format!(
            "continuous-assignment delay must be an integer constant in `{path}`"
        ));
    }
    if c.width > 64 || c.bits.len() > 1 {
        return Err(format!(
            "continuous-assignment delay must be a 64-bit-or-less constant in `{path}`"
        ));
    }
    if c.x.iter().any(|&x| x != 0) || c.z.iter().any(|&z| z != 0) {
        return Err(format!(
            "continuous-assignment delay must be a known (non-X/Z) constant in `{path}`"
        ));
    }
    // Negative signed constants carry their two's-complement bit pattern, so
    // the raw limb alone would wrap to a huge tick count; reject via the sign
    // bit instead.
    if c.signed && c.width > 0 && ((c.bits.first().copied().unwrap_or(0) >> (c.width - 1)) & 1) == 1
    {
        return Err(format!(
            "continuous-assignment delay must be a non-negative constant in `{path}`"
        ));
    }
    Ok(c.bits.first().copied().unwrap_or(0))
}

/// Scale a raw `#N` tick count from the calling module's time unit to
/// design-precision ticks (`N * unit / precision`).  Products beyond the u64
/// range are rejected instead of silently truncating through the `as u64`
/// cast.
fn scale_delay_ticks(
    raw: u64,
    unit_ps: u64,
    design_precision_ps: u64,
    path: &str,
) -> Result<u64, String> {
    let scaled = raw as u128 * unit_ps as u128 / design_precision_ps as u128;
    if scaled > u64::MAX as u128 {
        return Err(format!(
            "delay `#{raw}` scales past the 64-bit tick range in `{path}`"
        ));
    }
    Ok(scaled as u64)
}

/// Skip ASCII whitespace at the start of `s`.
fn skip_ws(s: &str) -> &str {
    s.trim_start_matches([' ', '\t'])
}

// ── Constant reading ──────────────────────────────────────────────────────────

/// A lowered constant value: see [`crate::sim::ir::IrConst`].  X and Z bits
/// live in separate `x`/`z` limb arrays (x & z == 0), matching the runtime's
/// 4-state split.
///
/// Convert a captured constant (`ValueData` + `vpiSize`) to an [`IrConst`].
fn read_const_from(vd: &ValueData, size: i32) -> Result<IrConst, String> {
    match vd {
        ValueData::Bin(s) => parse_radix(s, 1, size),
        ValueData::Oct(s) => parse_radix(s, 3, size),
        ValueData::Hex(s) => parse_radix(s, 4, size),
        ValueData::Dec(s) => {
            let mut limbs = dec_str_to_limbs(s.trim());
            let width = if size > 0 { size as u32 } else { 64 };
            truncate_limbs(&mut limbs, width);
            Ok(IrConst {
                bits: limbs,
                x: vec![0],
                z: vec![0],
                width,
                signed: false,
                real: None,
                fill: None,
            })
        }
        ValueData::Scalar(sc) => {
            // 0/1/2=X/3=Z: X and Z stay distinct so `$display` and casez/casex
            // (and the unsized-fill `'z`) see them separately.
            let b: u8 = match *sc {
                vpi::vpi0 | vpi::vpiL => 0,
                vpi::vpi1 | vpi::vpiH => 1,
                vpi::vpiX => 2,
                vpi::vpiZ => 3,
                _ => 2,
            };
            let fill = if size == -1 { Some(b) } else { None };
            Ok(IrConst {
                bits: vec![if b == 1 { 1 } else { 0 }],
                x: vec![if b == 2 { 1 } else { 0 }],
                z: vec![if b == 3 { 1 } else { 0 }],
                width: 1,
                signed: false,
                real: None,
                fill,
            })
        }
        ValueData::Int(val) => {
            let width = if size > 0 { size as u32 } else { 32 };
            Ok(IrConst {
                bits: vec![*val as u64],
                x: vec![0],
                z: vec![0],
                width,
                signed: true,
                real: None,
                fill: None,
            })
        }
        ValueData::UInt(val) => {
            let width = if size > 0 { size as u32 } else { 64 };
            Ok(IrConst {
                bits: vec![*val],
                x: vec![0],
                z: vec![0],
                width,
                signed: false,
                real: None,
                fill: None,
            })
        }
        ValueData::Real(value) => Ok(IrConst {
            bits: vec![0],
            x: vec![0],
            z: vec![0],
            width: 0,
            signed: false,
            real: Some(*value),
            fill: None,
        }),
        ValueData::Str(_) => Err("string constant in expression".to_string()),
        _ => Err("unsupported constant value format".to_string()),
    }
}

/// Parse an unsigned decimal digit string into LSB-first 64-bit limbs
/// (per-digit multiply-and-accumulate across the limbs).
fn dec_str_to_limbs(s: &str) -> Vec<u64> {
    let mut limbs = vec![0u64];
    for ch in s.chars() {
        let d = ch.to_digit(10).unwrap_or(0) as u64;
        let mut carry = d;
        for limb in limbs.iter_mut() {
            let cur = (*limb as u128) * 10 + carry as u128;
            *limb = cur as u64;
            carry = (cur >> 64) as u64;
        }
        if carry != 0 {
            limbs.push(carry);
        }
    }
    while limbs.len() > 1 && limbs.last() == Some(&0) {
        limbs.pop();
    }
    limbs
}

/// Drop limbs above `width` and mask the top partial limb.
fn truncate_limbs(limbs: &mut Vec<u64>, width: u32) {
    let n = (width as usize).div_ceil(64);
    if limbs.len() > n {
        limbs.truncate(n);
    }
    if !width.is_multiple_of(64) {
        if let Some(top) = limbs.last_mut() {
            *top &= (1u64 << (width % 64)) - 1;
        }
    }
}

/// Parse a BIN/OCT/HEX digit string (which may contain x/z/?) into a `IrConst`.
/// The stored string omits leading zeros, so a positive `vpiSize` LSB-aligns
/// the value into that width (zero-extending when shorter).  A one-digit
/// string with `size == -1` marks an unsized fill literal.  Digits map to bit
/// values 0/1/2=X/3=Z (`?` is a z synonym); X and Z land in separate limb
/// arrays.
fn parse_radix(s: &str, base_bits: usize, size: i32) -> Result<IrConst, String> {
    let mut vec: Vec<u8> = Vec::new(); // MSB-first; 0/1/2=X/3=Z
    for ch in s.chars() {
        let c = ch.to_ascii_lowercase();
        match c {
            'x' => vec.extend(std::iter::repeat_n(2u8, base_bits)),
            'z' | '?' => vec.extend(std::iter::repeat_n(3u8, base_bits)),
            _ => {
                let d = c.to_digit(16).unwrap_or(0);
                for i in (0..base_bits).rev() {
                    vec.push(if d & (1 << i) != 0 { 1 } else { 0 });
                }
            }
        }
    }
    let fill = if size == -1 && vec.len() == base_bits && base_bits == 1 {
        Some(vec[0])
    } else {
        None
    };
    if size > 0 {
        let width = size as usize;
        if vec.len() < width {
            let mut padded = vec![0u8; width - vec.len()];
            padded.extend(vec);
            vec = padded;
        } else if vec.len() > width {
            vec = vec[vec.len() - width..].to_vec();
        }
    }
    if vec.len() > LLG_MAX_WIDTH as usize {
        return Err(format!("constant wider than {LLG_MAX_WIDTH} bits"));
    }
    let nlimbs = vec.len().div_ceil(64);
    let mut bits = vec![0u64; nlimbs];
    let mut x = vec![0u64; nlimbs];
    let mut z = vec![0u64; nlimbs];
    for (i, b) in vec.iter().rev().enumerate() {
        match b {
            1 => bits[i / 64] |= 1u64 << (i % 64),
            2 => x[i / 64] |= 1u64 << (i % 64),
            3 => z[i / 64] |= 1u64 << (i % 64),
            _ => {}
        }
    }
    Ok(IrConst {
        bits,
        x,
        z,
        width: vec.len() as u32,
        signed: false,
        real: None,
        fill,
    })
}

fn val_to_const(v: &elab::Value) -> Result<IrConst, String> {
    if v.width() > LLG_MAX_WIDTH as usize {
        return Err(format!("parameter wider than {LLG_MAX_WIDTH} bits"));
    }
    let nlimbs = v.width().div_ceil(64);
    let mut bits = vec![0u64; nlimbs];
    let mut x = vec![0u64; nlimbs];
    let mut z = vec![0u64; nlimbs];
    for i in 0..v.width() {
        match v.bit_lsb(i) {
            Bit::One => bits[i / 64] |= 1u64 << (i % 64),
            Bit::X => x[i / 64] |= 1u64 << (i % 64),
            Bit::Z => z[i / 64] |= 1u64 << (i % 64),
            Bit::Zero => {}
        }
    }
    Ok(IrConst {
        bits,
        x,
        z,
        width: v.width() as u32,
        signed: v.signed,
        real: None,
        fill: None,
    })
}

/// Parse the contents of a hierarchical select bracket (`3:0`, `2`, `3 +: 4`,
/// `8'h2a`) into a [`HierSelect`].  Only plain integer literals are accepted;
/// identifiers (variable indices) and expressions are rejected in v1.
fn hier_select_from_text(inner: &str, target_name: &str) -> Result<HierSelect, String> {
    let s = inner.trim();
    let bad = || {
        format!(
            "hierarchical select `[{inner}]` on `{target_name}` is not \
             supported in v1 (constant integer indices/bounds only)"
        )
    };
    if let Some(plus) = s.find("+:") {
        let (base, width) = (s[..plus].trim(), s[plus + 2..].trim());
        let base = parse_select_int(base).ok_or_else(bad)?;
        let width = parse_select_int(width).ok_or_else(bad)?;
        if width <= 0 {
            return Err(format!(
                "indexed part-select `[{inner}]` on `{target_name}` must have \
                 a positive width"
            ));
        }
        return Ok(HierSelect::IdxPart(base, width, false));
    }
    if let Some(minus) = s.find("-:") {
        let (base, width) = (s[..minus].trim(), s[minus + 2..].trim());
        let base = parse_select_int(base).ok_or_else(bad)?;
        let width = parse_select_int(width).ok_or_else(bad)?;
        if width <= 0 {
            return Err(format!(
                "indexed part-select `[{inner}]` on `{target_name}` must have \
                 a positive width"
            ));
        }
        return Ok(HierSelect::IdxPart(base, width, true));
    }
    if let Some(colon) = s.find(':') {
        let (left, right) = (s[..colon].trim(), s[colon + 1..].trim());
        let left = parse_select_int(left).ok_or_else(bad)?;
        let right = parse_select_int(right).ok_or_else(bad)?;
        return Ok(HierSelect::Part(left, right));
    }
    let bit = parse_select_int(s).ok_or_else(bad)?;
    Ok(HierSelect::Bit(bit))
}

/// Parse a plain Verilog integer literal (decimal, sized/unsized radix form,
/// optional sign) into an `i128`.  `None` for identifiers, x/z digits, or any
/// other form the v1 hierarchical-select support does not handle.
fn parse_select_int(s: &str) -> Option<i128> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let (neg, s) = match s.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, s),
    };
    let (radix, digits) = if let Some(q) = s.find('\'') {
        let rest = &s[q + 1..];
        let rest = rest
            .strip_prefix('s')
            .or_else(|| rest.strip_prefix('S'))
            .unwrap_or(rest);
        let mut chars = rest.chars();
        let base = chars.next()?;
        match base {
            'd' | 'D' => (10, chars.as_str()),
            'h' | 'H' => (16, chars.as_str()),
            'b' | 'B' => (2, chars.as_str()),
            'o' | 'O' => (8, chars.as_str()),
            _ => return None,
        }
    } else {
        (10, s)
    };
    let digits = digits.replace('_', "");
    if digits.is_empty()
        || digits
            .chars()
            .any(|c| matches!(c, 'x' | 'X' | 'z' | 'Z' | '?'))
    {
        return None;
    }
    let v = i128::from_str_radix(&digits, radix).ok()?;
    Some(if neg { -v } else { v })
}

// ── Design collection ─────────────────────────────────────────────────────────

impl<'a> Codegen<'a> {
    /// Walk the instance tree, collecting signals, parameters and gen-scope
    /// paths.  Returns the top module nodes.
    fn collect_design(&mut self) -> Result<Vec<NodeId>, String> {
        let mut tops = Vec::new();
        for top in &self.db.tops {
            let path = strip_lib(&self.node(*top).name);
            if path.is_empty() {
                return Err("top instance has no name".to_string());
            }
            if self.design_name.is_empty() {
                self.design_name = path.clone();
                self.model.design_name = path.clone();
            }
            self.collect_instance(*top, &path)?;
            self.collect_funcs(*top, &path)?;
            tops.push(*top);
        }
        Ok(tops)
    }

    /// Collect the arena nodes of every per-port COPY interface instance: the
    /// `low` targets of interface-typed ports.  A modport port's `low` is the
    /// copy's `vpiModport` (whose parent is the copy interface instance); a
    /// bare interface port's `low` is the copy interface instance itself.
    /// This mirrors `emit_iface_link`'s copy lookup.
    fn collect_iface_copies(&mut self) {
        let mut copies = HashSet::new();
        for top in &self.db.tops {
            self.collect_iface_copies_in(*top, &mut copies);
        }
        self.iface_copy_insts = copies;
    }

    fn collect_iface_copies_in(&self, inst: NodeId, copies: &mut HashSet<NodeId>) {
        for c in &self.node(inst).children {
            match self.kind(*c) {
                NodeKind::Port { low: Some(l), .. } => match self.kind(*l) {
                    NodeKind::ModPort => {
                        if let Some(iface) = self.node(*l).parent {
                            if matches!(
                                self.kind(iface),
                                NodeKind::ModuleInst {
                                    is_interface: true,
                                    ..
                                }
                            ) {
                                copies.insert(iface);
                            }
                        }
                    }
                    NodeKind::ModuleInst {
                        is_interface: true, ..
                    } => {
                        copies.insert(*l);
                    }
                    _ => {}
                },
                NodeKind::ModuleInst { .. } => self.collect_iface_copies_in(*c, copies),
                NodeKind::GenScopeArray => {
                    for gs in &self.node(*c).children {
                        if matches!(self.kind(*gs), NodeKind::GenScope) {
                            self.collect_iface_copies_in(*gs, copies);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn collect_instance(&mut self, inst: NodeId, path: &str) -> Result<(), String> {
        for child in &self.node(inst).children {
            if let NodeKind::Port { high, low, .. } = self.kind(*child) {
                for side in [high, low].into_iter().flatten() {
                    if self.signal_node_is_real(*side) {
                        return Err(format!(
                            "real/shortreal ports are not supported in `{path}` (port `{}`)",
                            self.node(*child).name
                        ));
                    }
                }
            }
        }
        let mut seen: HashSet<String> = HashSet::new();
        for c in &self.node(inst).children {
            let nid = *c;
            match self.kind(nid) {
                NodeKind::Array { ty } => {
                    let name = self.node(nid).name.clone();
                    if name.is_empty() || !seen.insert(name.clone()) {
                        continue;
                    }
                    let info = self.array_info(path, &name, nid, ty)?;
                    self.arrays.push(info.clone());
                    self.array_globals.insert(nid, info.clone());
                    self.scope_array_names
                        .entry(path.to_string())
                        .or_default()
                        .insert(name, info);
                }
                NodeKind::Net { ty, .. } | NodeKind::Var { ty } => {
                    let name = self.node(nid).name.clone();
                    if name.is_empty() || !seen.insert(name.clone()) {
                        continue;
                    }
                    let w = self.signal_width(path, name.as_str(), ty)?;
                    let ir = self.model.signals.len();
                    let info = SignalInfo {
                        global: if is_real_kind(&ty.kind) {
                            real_global_name(path, &name)
                        } else {
                            global_name(path, &name)
                        },
                        width: w,
                        signed: ty.signed,
                        real: is_real_kind(&ty.kind),
                        shortreal: ty.kind == "shortreal",
                        net_driver: None,
                        ir,
                    };
                    self.model.signals.push(IrSignal {
                        c_name: info.global.clone(),
                        hdl_name: Some(self.waveform_name(nid)),
                        ty: if info.real {
                            IrType::Real {
                                shortreal: info.shortreal,
                            }
                        } else {
                            IrType::Packed {
                                width: info.width,
                                signed: info.signed,
                            }
                        },
                        net_driver: None,
                        omit: false,
                    });
                    self.signals.push(info.clone());
                    self.sig_globals.insert(nid, info.clone());
                    self.scope_sig_names
                        .entry(path.to_string())
                        .or_default()
                        .insert(name, info);
                }
                NodeKind::Param { value: Some(v), .. } => {
                    self.param_vals.insert(nid, v.clone());
                }
                NodeKind::NamedEvent => {
                    let name = self.node(nid).name.clone();
                    if name.is_empty() || !seen.insert(name.clone()) {
                        continue;
                    }
                    let ir = self.model.events.len();
                    let info = EventInfo {
                        global: event_global_name(path, &name),
                        ir,
                    };
                    self.model.events.push(IrEvent {
                        c_name: info.global.clone(),
                    });
                    self.events.push(info.clone());
                    self.event_globals.insert(nid, info);
                }
                // A declaration initializer on an `array_net` (`reg [7:0] m
                // [0:3] = '{…}` — Surelog models the pattern as a
                // net-decl-assign continuous assignment whose LHS is the
                // array).  Applied to the array's `ArrayInfo`; the assignment
                // itself is skipped at emission. A scalar reg initializer is
                // collected into `scalar_inits`; true nets stay available to
                // `emit_cont_assign` as continuous drivers.
                NodeKind::ContAssign {
                    net_decl: true,
                    delay: _,
                } => {
                    if let Some((arr, vals)) = self.cont_assign_array_init(path, nid)? {
                        let name = self.node(arr).name.clone();
                        let ai = self.array_globals.get_mut(&arr).ok_or_else(|| {
                            format!(
                                "array initializer for `{name}` in `{path}` references an \
                                 array that was not collected"
                            )
                        })?;
                        if ai.init.is_some() {
                            return Err(format!(
                                "array `{name}` in `{path}` has more than one declaration \
                                 initializer"
                            ));
                        }
                        ai.init = Some(vals.clone());
                        // Keep the deterministic-emission Vec in sync (its
                        // entry was cloned before the initializer was known).
                        if let Some(vi) = self.arrays.iter_mut().find(|vi| vi.global == ai.global) {
                            vi.init = Some(vals);
                        }
                    } else if matches!(self.net_decl_target(nid), NetDeclTarget::Variable) {
                        let (info, c) = self.scalar_decl_init(path, nid)?.ok_or_else(|| {
                            format!(
                                "variable declaration initializer is not a constant expression \
                                 in `{path}`"
                            )
                        })?;
                        self.scalar_inits.push((info, c));
                        self.scalar_init_ca.insert(nid);
                    }
                }
                _ => {}
            }
        }
        // Variable declaration initializers are folded AFTER the instance's
        // own parameters are collected: a `int y = P + 1;` RHS references the
        // instance's `P` through `param_vals` (params are walked after vars,
        // see `walk_module_inst`).
        self.collect_var_inits(path, inst)?;
        for c in &self.node(inst).children {
            if matches!(self.kind(*c), NodeKind::GenScopeArray) {
                self.collect_gen_scope_array(*c, path)?;
            }
        }
        for c in &self.node(inst).children {
            if matches!(self.kind(*c), NodeKind::ModuleInst { .. }) {
                let cname = self.node(*c).name.clone();
                if cname.is_empty() {
                    return Err(format!("unnamed child instance in `{path}`"));
                }
                let child_path = format!("{path}.{}", ident(&cname));
                self.collect_instance(*c, &child_path)?;
            }
        }
        Ok(())
    }

    /// Fold every declaration initializer of a scalar VARIABLE whose init
    /// lives on the var's `vpiExpr` (`logic l = 1'b0;`, `int x = 5;` —
    /// captured in [`Db::vars_init`]) into a constant and queue it for
    /// `main()`.  Called after the scope's parameters are collected so
    /// `P + 1`-style RHS refs resolve via `param_vals`.  Vars that are not
    /// collected as signals (function/block locals, per-port copies) carry no
    /// fill; their initializers are handled by their own paths.
    fn collect_var_inits(&mut self, path: &str, inst: NodeId) -> Result<(), String> {
        for c in &self.node(inst).children {
            if !matches!(self.kind(*c), NodeKind::Var { .. }) {
                continue;
            }
            let init = match self.db.vars_init.get(c) {
                Some(init) => *init,
                None => continue,
            };
            let info = match self.signal_of(*c) {
                Some(info) => info.clone(),
                None => continue,
            };
            let name = self.node(*c).name.clone();
            let cconst = self.var_decl_init(path, &name, init)?;
            self.var_inits.push((info, cconst));
        }
        Ok(())
    }

    /// Record the C function name of every function/task definition in the
    /// instance tree, recursing into child instances and instances inside
    /// generate scopes.
    fn collect_funcs(&mut self, inst: NodeId, path: &str) -> Result<(), String> {
        for c in &self.node(inst).children {
            if matches!(self.kind(*c), NodeKind::FuncTask { .. }) {
                let fname = self.node(*c).name.clone();
                let c_name = format!("fn_{}_{}", ident(path), ident(&fname));
                self.func_names.insert(*c, c_name);
            }
        }
        for c in &self.node(inst).children {
            if matches!(self.kind(*c), NodeKind::GenScopeArray) {
                for gs in &self.node(*c).children {
                    if matches!(self.kind(*gs), NodeKind::GenScope) {
                        for cc in &self.node(*gs).children {
                            if matches!(self.kind(*cc), NodeKind::ModuleInst { .. }) {
                                let child_path = self.instance_path_of(*cc);
                                self.collect_funcs(*cc, &child_path)?;
                            }
                        }
                    }
                }
            }
        }
        for c in &self.node(inst).children {
            if matches!(self.kind(*c), NodeKind::ModuleInst { .. }) {
                let child_path = self.instance_path_of(*c);
                self.collect_funcs(*c, &child_path)?;
            }
        }
        Ok(())
    }

    /// Width of a Net/Var from its captured `TypeInfo`; rejects unsupported
    /// kinds and widths above `LLG_MAX_WIDTH` bits.
    fn signal_node_is_real(&self, node: NodeId) -> bool {
        match self.kind(node) {
            NodeKind::Net { ty, .. } | NodeKind::Var { ty } | NodeKind::Array { ty } => {
                is_real_kind(&ty.kind)
                    || matches!(ty.kind.as_str(), "real_array" | "shortreal_array")
            }
            _ => false,
        }
    }

    fn signal_width(
        &self,
        path: &str,
        name: &str,
        ty: &crate::core::model::TypeInfo,
    ) -> Result<u32, String> {
        let w = match ty.kind.as_str() {
            // real/string/class variables (and nets with such types).
            "real" | "shortreal" => 0,
            "string" | "class" => {
                return Err(format!(
                    "string/class signals are not supported: `{name}` in `{path}`"
                ))
            }
            "int" | "integer" | "time" | "longint" | "byte" | "shortint" | "logic" | "bit"
            | "enum" => ty.width.unwrap_or(1),
            _ => {
                return Err(format!(
                    "unsupported typespec type for signal `{name}` in `{path}`"
                ))
            }
        };
        if w > LLG_MAX_WIDTH {
            return Err(format!(
                "signal `{name}` in `{path}` is {w} bits wide; the v1 runtime \
                 supports at most {LLG_MAX_WIDTH}"
            ));
        }
        Ok(w)
    }

    fn collect_gen_scope_array(&mut self, gsa: NodeId, path: &str) -> Result<(), String> {
        for c in &self.node(gsa).children {
            if matches!(self.kind(*c), NodeKind::GenScope) {
                self.collect_gen_scope(*c, path)?;
            }
        }
        Ok(())
    }

    fn collect_gen_scope(&mut self, gs: NodeId, path: &str) -> Result<(), String> {
        // Surelog names the enclosing `gen_scope_array` (e.g. `g[0]` for the
        // genvar-loop iteration) and leaves the `gen_scope` itself unnamed;
        // fall back to the array's name so per-iteration paths stay distinct.
        let gs_node = self.node(gs);
        let gs_name = if gs_node.name.is_empty() {
            gs_node
                .parent
                .map(|p| self.node(p).name.clone())
                .unwrap_or_default()
        } else {
            gs_node.name.clone()
        };
        let gs_path = if gs_name.is_empty() {
            format!("{path}.genblk")
        } else {
            format!("{path}.{}", ident(&gs_name))
        };
        self.gen_scope_paths.insert(gs, gs_path.clone());
        let mut gseen: HashSet<String> = HashSet::new();
        for c in &self.node(gs).children {
            let nid = *c;
            match self.kind(nid) {
                NodeKind::Array { ty } => {
                    let name = self.node(nid).name.clone();
                    if name.is_empty() || !gseen.insert(name.clone()) {
                        continue;
                    }
                    let info = self.array_info(&gs_path, &name, nid, ty)?;
                    self.arrays.push(info.clone());
                    self.array_globals.insert(nid, info.clone());
                    self.scope_array_names
                        .entry(gs_path.clone())
                        .or_default()
                        .insert(name, info);
                }
                NodeKind::Net { ty, .. } | NodeKind::Var { ty } => {
                    let name = self.node(nid).name.clone();
                    if name.is_empty() || !gseen.insert(name.clone()) {
                        continue;
                    }
                    let w = self.signal_width(&gs_path, &name, ty)?;
                    let ir = self.model.signals.len();
                    let info = SignalInfo {
                        global: if is_real_kind(&ty.kind) {
                            real_global_name(&gs_path, &name)
                        } else {
                            global_name(&gs_path, &name)
                        },
                        width: w,
                        signed: ty.signed,
                        real: is_real_kind(&ty.kind),
                        shortreal: ty.kind == "shortreal",
                        net_driver: None,
                        ir,
                    };
                    self.model.signals.push(IrSignal {
                        c_name: info.global.clone(),
                        hdl_name: Some(self.waveform_name(nid)),
                        ty: if info.real {
                            IrType::Real {
                                shortreal: info.shortreal,
                            }
                        } else {
                            IrType::Packed {
                                width: info.width,
                                signed: info.signed,
                            }
                        },
                        net_driver: None,
                        omit: false,
                    });
                    self.signals.push(info.clone());
                    self.sig_globals.insert(nid, info.clone());
                    self.scope_sig_names
                        .entry(gs_path.clone())
                        .or_default()
                        .insert(name, info);
                }
                NodeKind::Param { value: Some(v), .. } => {
                    self.param_vals.insert(nid, v.clone());
                }
                NodeKind::NamedEvent => {
                    let name = self.node(nid).name.clone();
                    if name.is_empty() || !gseen.insert(name.clone()) {
                        continue;
                    }
                    let ir = self.model.events.len();
                    let info = EventInfo {
                        global: event_global_name(&gs_path, &name),
                        ir,
                    };
                    self.model.events.push(IrEvent {
                        c_name: info.global.clone(),
                    });
                    self.events.push(info.clone());
                    self.event_globals.insert(nid, info);
                }
                NodeKind::ContAssign {
                    net_decl: true,
                    delay: _,
                } => {
                    if let Some((arr, vals)) = self.cont_assign_array_init(&gs_path, nid)? {
                        let name = self.node(arr).name.clone();
                        let ai = self.array_globals.get_mut(&arr).ok_or_else(|| {
                            format!(
                                "array initializer for `{name}` in `{gs_path}` references an \
                                 array that was not collected"
                            )
                        })?;
                        if ai.init.is_some() {
                            return Err(format!(
                                "array `{name}` in `{gs_path}` has more than one declaration \
                                 initializer"
                            ));
                        }
                        ai.init = Some(vals.clone());
                        if let Some(vi) = self.arrays.iter_mut().find(|vi| vi.global == ai.global) {
                            vi.init = Some(vals);
                        }
                    } else if matches!(self.net_decl_target(nid), NetDeclTarget::Variable) {
                        let (info, c) = self.scalar_decl_init(&gs_path, nid)?.ok_or_else(|| {
                            format!(
                                "variable declaration initializer is not a constant expression \
                                 in `{gs_path}`"
                            )
                        })?;
                        self.scalar_inits.push((info, c));
                        self.scalar_init_ca.insert(nid);
                    }
                }
                _ => {}
            }
        }
        // Variable declaration initializers, folded after the scope's own
        // parameters are collected (see `collect_var_inits`).
        self.collect_var_inits(&gs_path, gs)?;
        // Module instances inside the generate scope are collected like
        // regular child instances (signals, arrays, params, processes,
        // nested gen scopes), under their full instance path.
        for c in &self.node(gs).children {
            if matches!(self.kind(*c), NodeKind::ModuleInst { .. }) {
                let child_path = self.instance_path_of(*c);
                self.collect_instance(*c, &child_path)?;
            }
        }
        Ok(())
    }

    // ── Collapsed inout-net groups ────────────────────────────────────────

    /// Collapse inout-port net pairs (parent `vpiHighConn` + child
    /// `vpiLowConn`) into one resolved simulated net per connected set
    /// (LRM §23.3.3.7), run after [`collect_design`](Self::collect_design)
    /// and before any emission.
    ///
    /// Every grouped member's `SignalInfo` is redirected to the shared
    /// `llg_net_t`'s `resolved` cell and tagged with its driver slot, so
    /// reads/writes/sensitivity all use the resolution cell automatically.
    /// Groups with anything the runtime cannot resolve (non-net members,
    /// mixed widths, unsupported net types, select-LHS/NBA/task-actual
    /// writes) are skipped with an explicit warning — never silently.
    fn build_net_groups(&mut self) -> Result<(), String> {
        let nodes = self.design_nodes();
        // Union-find over the parent/child nets of every inout port.
        let mut parent: HashMap<NodeId, NodeId> = HashMap::new();
        let mut rank: HashMap<NodeId, u8> = HashMap::new();
        let mut inout_ports: Vec<NodeId> = Vec::new();
        for id in &nodes {
            if let NodeKind::Port {
                direction: crate::core::model::Direction::Inout,
                high: Some(h),
                low: Some(l),
                ..
            } = self.kind(*id)
            {
                inout_ports.push(*id);
                union(&mut parent, &mut rank, *h, *l);
            } else if let NodeKind::Port {
                direction: crate::core::model::Direction::Inout,
                high: None,
                ..
            } = self.kind(*id)
            {
                // A top-level inout port has no parent-side connection; there
                // is nothing to collapse, so it stays a plain net (no link is
                // ever emitted for top-level ports).
            } else if let NodeKind::Port {
                direction: crate::core::model::Direction::Inout,
                high: Some(_),
                low: None,
                ..
            } = self.kind(*id)
            {
                self.warnings.push(format!(
                    "inout port `{}` of `{}`: child-side connection not \
                     resolved; port connection skipped",
                    self.node(*id).name,
                    self.node(*id)
                        .parent
                        .map(|p| self.node(p).name.clone())
                        .unwrap_or_default()
                ));
            }
        }

        // Bucket every distinct member by its union root (a parent net shared
        // by several ports lands in one group).
        let mut members: HashSet<NodeId> = HashSet::new();
        for port in &inout_ports {
            if let NodeKind::Port {
                high: Some(h),
                low: Some(l),
                ..
            } = self.kind(*port)
            {
                members.insert(*h);
                members.insert(*l);
            }
        }
        let mut buckets: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
        for m in members {
            let r = find(&mut parent, m);
            buckets.entry(r).or_default().push(m);
        }
        let mut groups: Vec<Vec<NodeId>> = buckets.into_values().collect();
        for g in &mut groups {
            g.sort_by_key(|id| id.0);
        }
        groups.sort_by_key(|g| g[0].0);

        let mut member_slots: HashMap<NodeId, (String, usize)> = HashMap::new();
        let mut old_globals: HashMap<String, NodeId> = HashMap::new();
        for members in &groups {
            let names = members
                .iter()
                .map(|m| self.display_name(*m))
                .collect::<Vec<_>>()
                .join(", ");
            let joined = format!("inout-net group {{{names}}}");
            // 1. Members must be plain nets (vars/arrays cannot resolve).
            if let Some(bad) = members
                .iter()
                .find(|m| !matches!(self.kind(**m), NodeKind::Net { .. }))
            {
                self.warnings.push(format!(
                    "{joined}: member `{}` is not a net; group skipped (inout \
                     connection dropped)",
                    self.display_name(*bad)
                ));
                continue;
            }
            // 2. Widths must agree across the collapsed net.
            let first_ty = match self.kind(members[0]) {
                NodeKind::Net { ty, .. } => ty.clone(),
                _ => unreachable!("validated above"),
            };
            let width = first_ty.width.unwrap_or(1);
            if let Some(bad) = members.iter().skip(1).find(|m| match self.kind(**m) {
                NodeKind::Net { ty, .. } => ty.width.unwrap_or(1) != width,
                _ => true,
            }) {
                self.warnings.push(format!(
                    "{joined}: member `{}` has a different width than `{}`; \
                     group skipped (inout connection dropped)",
                    self.display_name(*bad),
                    self.display_name(members[0])
                ));
                continue;
            }
            // 3. Only wire/tri/logic nets support the equal-strength
            //    resolution the runtime implements (Table 6-2).
            if let Some(bad) = members.iter().find(|m| match self.kind(**m) {
                NodeKind::Net { net_type, .. } => {
                    !matches!(*net_type, vpi::vpiWire | vpi::vpiTri | vpi::vpiNet)
                }
                _ => true,
            }) {
                let net_type = match self.kind(*bad) {
                    NodeKind::Net { net_type, .. } => *net_type,
                    _ => 0,
                };
                self.warnings.push(format!(
                    "{joined}: member `{}` has unsupported net type {net_type} \
                     (only wire/tri/logic nets resolve); group skipped (inout \
                     connection dropped)",
                    self.display_name(*bad)
                ));
                continue;
            }
            // 4. The runtime struct has a fixed driver-slot array.
            if members.len() > LLG_MAX_NET_DRIVERS {
                self.warnings.push(format!(
                    "{joined}: {}-member group exceeds the {LLG_MAX_NET_DRIVERS} \
                     driver-slot limit; group skipped (inout connection dropped)",
                    members.len()
                ));
                continue;
            }
            // 5. Only whole-signal drivers may touch a member: select LHS,
            //    NBA writes and task `sv4_t*` actuals would bypass the
            //    resolution cell.
            if let Some(reason) = self.unsupported_member_write(members) {
                self.warnings.push(format!(
                    "{joined}: {reason}; group skipped (inout connection \
                     dropped)"
                ));
                continue;
            }

            // Group is valid: assign one driver slot per member (NodeId
            // order) and redirect every member's storage to the resolved cell.
            let name = format!("g_net_{}", self.model.net_groups.len());
            let gidx = self.model.net_groups.len();
            for (slot, m) in members.iter().enumerate() {
                let old_global = self.sig_globals.get(m).map(|i| i.global.clone());
                if let Some(info) = self.sig_globals.get_mut(m) {
                    info.global = format!("{name}.resolved");
                    info.net_driver = Some((name.clone(), slot));
                    if let Some(sig) = self.model.signals.get_mut(info.ir) {
                        sig.c_name = format!("{name}.resolved");
                        sig.net_driver = Some((gidx, slot));
                    }
                }
                if let Some(g) = old_global {
                    old_globals.insert(g, *m);
                }
                member_slots.insert(*m, (name.clone(), slot));
            }
            // Keep the deterministic emission Vec (`signals`) in sync with
            // the global map so `emit_signals` skips members.
            for info in &mut self.signals {
                if info.net_driver.is_none() {
                    if let Some(m) = old_globals.get(&info.global) {
                        if let Some((n, slot)) = member_slots.get(m) {
                            info.global = format!("{n}.resolved");
                            info.net_driver = Some((n.clone(), *slot));
                        }
                    }
                }
            }
            // Name fallbacks (refs resolved by name) must see the resolved
            // cell too.
            for map in self.scope_sig_names.values_mut() {
                for info in map.values_mut() {
                    if let Some(m) = old_globals.get(&info.global) {
                        if let Some((n, slot)) = member_slots.get(m) {
                            info.global = format!("{n}.resolved");
                            info.net_driver = Some((n.clone(), *slot));
                        }
                    }
                }
            }
            self.model.net_groups.push(crate::sim::ir::IrNetGroup {
                c_name: name.clone(),
                width,
                signed: first_ty.signed,
                n_drivers: members.len(),
            });
        }

        // Declaration initializers on grouped members (`wire bus = 8'hzz;`)
        // are applied through their driver slot instead of a direct write to
        // the resolved cell.
        let mut keep = Vec::new();
        for (info, c) in std::mem::take(&mut self.scalar_inits) {
            match old_globals
                .get(&info.global)
                .and_then(|m| member_slots.get(m))
            {
                Some((net, slot)) => self.net_inits.push((net.clone(), *slot, c)),
                None => keep.push((info, c)),
            }
        }
        self.scalar_inits = keep;
        Ok(())
    }

    /// Every arena node of the instance tree (top instances + children +
    /// generate scopes), depth-first, in deterministic order.
    fn design_nodes(&self) -> Vec<NodeId> {
        fn walk(db: &Db, id: NodeId, out: &mut Vec<NodeId>) {
            out.push(id);
            for c in &db.node(id).children {
                walk(db, *c, out);
            }
        }
        let mut out = Vec::new();
        for top in &self.db.tops {
            walk(self.db, *top, &mut out);
        }
        out
    }

    /// `lib@`-stripped name of a signal, with its scope path when available
    /// (`"tb.bus"`, `"tb.u0.bus"`).
    fn display_name(&self, id: NodeId) -> String {
        let node = self.node(id);
        match node.parent {
            Some(p) => {
                let scope = self.db.instance_path(p);
                if scope.is_empty() {
                    strip_lib(&node.name)
                } else {
                    format!("{}.{}", scope, node.name)
                }
            }
            None => strip_lib(&node.name),
        }
    }

    /// Full HDL hierarchy for waveform metadata, with ASCII unit-separator
    /// bytes between components.  The separator is not legal inside a source
    /// identifier, unlike `.`, so an escaped identifier such as `\a.b` cannot
    /// be mistaken for two scopes by the C waveform runtime.  Generate-scope
    /// spelling is retained verbatim (`g[0]`, not `g_0_`).
    fn waveform_name(&self, id: NodeId) -> String {
        const SEPARATOR: &str = "\u{1f}";

        let mut parts = vec![self.node(id).name.clone()];
        let mut current = self.node(id).parent;
        while let Some(scope_id) = current {
            let scope = self.node(scope_id);
            if matches!(
                scope.kind,
                NodeKind::ModuleInst { .. } | NodeKind::GenScopeArray | NodeKind::GenScope
            ) {
                // Surelog library-qualifies top design units (`work@tb`).
                // Other `vpiName` components are source identifiers, where
                // `@` is legal in an escaped spelling and must be preserved.
                let name = match &scope.kind {
                    NodeKind::ModuleInst { is_top: true, .. } => strip_lib(&scope.name),
                    _ => scope.name.clone(),
                };
                if !name.is_empty() {
                    parts.push(name);
                }
            }
            current = scope.parent;
        }
        parts.reverse();
        parts.join(SEPARATOR)
    }

    /// The reason a candidate inout-net group cannot be supported, from a
    /// design-wide scan of every write targeting its members: `None` when
    /// every write is a whole-signal (blocking or continuous) driver.
    fn unsupported_member_write(&self, members: &[NodeId]) -> Option<String> {
        let member_set: HashSet<NodeId> = members.iter().copied().collect();
        for id in self.design_nodes() {
            match self.kind(id) {
                NodeKind::ContAssign { .. } => {
                    let Some(lhs) = self.node(id).children.first().copied() else {
                        continue;
                    };
                    match self.member_write_kind(lhs, &member_set) {
                        MemberWrite::None | MemberWrite::Whole => {}
                        MemberWrite::Select => {
                            return Some(format!(
                                "bit/part/select LHS on member `{}`",
                                self.display_name(lhs)
                            ))
                        }
                    }
                }
                NodeKind::Stmt(StmtKind::Assign { blocking: true, .. }) => {
                    let Some(lhs) = self.node(id).children.first().copied() else {
                        continue;
                    };
                    match self.member_write_kind(lhs, &member_set) {
                        MemberWrite::None | MemberWrite::Whole => {}
                        MemberWrite::Select => {
                            return Some(format!(
                                "bit/part/select LHS on member `{}`",
                                self.display_name(lhs)
                            ))
                        }
                    }
                }
                NodeKind::Stmt(StmtKind::Assign {
                    blocking: false, ..
                }) => {
                    let Some(lhs) = self.node(id).children.first().copied() else {
                        continue;
                    };
                    if self.member_write_base(lhs, &member_set).is_some() {
                        return Some(format!(
                            "nonblocking assignment to member `{}`",
                            self.display_name(lhs)
                        ));
                    }
                }
                NodeKind::FuncCall { is_task: true, .. } => {
                    if let Some(reason) = self.task_actual_member_write(id, &member_set) {
                        return Some(reason);
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// How an assignment LHS touches a member set: not at all, as a whole
    /// signal (supported), or through a select (unsupported).
    fn member_write_kind(&self, lhs: NodeId, member_set: &HashSet<NodeId>) -> MemberWrite {
        match self.kind(lhs) {
            NodeKind::Net { .. } if member_set.contains(&lhs) => MemberWrite::Whole,
            NodeKind::Expr(ExprKind::Ref { target }) => match target {
                Some(t) if member_set.contains(t) => MemberWrite::Whole,
                _ => MemberWrite::None,
            },
            NodeKind::Expr(
                ExprKind::BitSelect { .. }
                | ExprKind::PartSelect { .. }
                | ExprKind::IndexedPartSelect { .. }
                | ExprKind::ArraySelect { .. },
            ) => {
                if self.member_write_base(lhs, member_set).is_some() {
                    MemberWrite::Select
                } else {
                    MemberWrite::None
                }
            }
            _ => MemberWrite::None,
        }
    }

    /// The member (if any) a select chain or ref ultimately writes to.
    fn member_write_base(&self, node: NodeId, member_set: &HashSet<NodeId>) -> Option<NodeId> {
        match self.kind(node) {
            NodeKind::Net { .. } if member_set.contains(&node) => Some(node),
            NodeKind::Expr(ExprKind::Ref { target }) => target.filter(|t| member_set.contains(t)),
            NodeKind::Expr(
                ExprKind::BitSelect { base, .. }
                | ExprKind::PartSelect { base, .. }
                | ExprKind::IndexedPartSelect { base, .. }
                | ExprKind::ArraySelect { base, .. },
            ) => self.member_write_base(*base, member_set),
            _ => None,
        }
    }

    /// Whether a task call binds an output/inout formal to a member: those
    /// actuals become `sv4_t*` parameters in the emitted C and would write
    /// through the resolved cell, bypassing resolution.
    fn task_actual_member_write(
        &self,
        call: NodeId,
        member_set: &HashSet<NodeId>,
    ) -> Option<String> {
        let (name, callee) = match self.kind(call) {
            NodeKind::FuncCall {
                name,
                is_task: true,
                callee,
                ..
            } => (name.clone(), *callee),
            _ => return None,
        };
        let inst = self.owning_inst(call)?;
        let ft = self.resolve_callee(inst, &name, true, callee).ok()?;
        let (_, _, formals) = self.func_info(ft).ok()?;
        let args: Vec<NodeId> = self.node(call).children.clone();
        for (idx, (io, is_out)) in formals.iter().enumerate() {
            if !*is_out {
                continue;
            }
            if let Some(arg) = args.get(idx) {
                if let Some(m) = self.member_write_base(*arg, member_set) {
                    return Some(format!(
                        "task output/inout actual `{}` on member `{}`",
                        self.node(*io).name,
                        self.display_name(m)
                    ));
                }
            }
        }
        None
    }

    /// The module instance that owns `node` (walking up the parent chain).
    fn owning_inst(&self, node: NodeId) -> Option<NodeId> {
        let mut cur = self.node(node).parent;
        while let Some(p) = cur {
            if matches!(self.kind(p), NodeKind::ModuleInst { .. }) {
                return Some(p);
            }
            cur = self.node(p).parent;
        }
        None
    }

    /// The arena node of the array an LHS `Ref` resolves to, or `None`.
    fn ref_array_target(&self, node: NodeId) -> Option<NodeId> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target: Some(t) })
                if matches!(self.kind(*t), NodeKind::Array { .. }) =>
            {
                Some(*t)
            }
            _ => None,
        }
    }

    /// The array a `vpiNetDeclAssign` continuous assignment initializes (its
    /// LHS resolves to an `Array` node), or `None` for other assignments.
    fn cont_assign_array_target(&self, ca: NodeId) -> Option<NodeId> {
        self.node(ca)
            .children
            .first()
            .copied()
            .and_then(|lhs| self.ref_array_target(lhs))
    }

    /// Classify the declaration object on the LHS of a net-declaration
    /// assignment. `wire`, `tri`, and SV `logic` nets are true continuous
    /// drivers; `reg` and variable objects retain declaration-initializer
    /// behavior. Unpacked arrays stay on the dedicated initializer path.
    fn net_decl_target(&self, ca: NodeId) -> NetDeclTarget {
        let Some(lhs) = self.node(ca).children.first().copied() else {
            return NetDeclTarget::Unknown;
        };
        let target = match self.kind(lhs) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            NodeKind::Net { .. } | NodeKind::Var { .. } | NodeKind::Array { .. } => Some(lhs),
            _ => None,
        };
        match target.map(|target| self.kind(target)) {
            Some(NodeKind::Array { .. }) => NetDeclTarget::Array,
            Some(NodeKind::Var { .. }) => NetDeclTarget::Variable,
            Some(NodeKind::Net { net_type, .. }) => match *net_type {
                vpi::vpiWire | vpi::vpiTri | vpi::vpiNet => NetDeclTarget::TrueNet,
                vpi::vpiReg => NetDeclTarget::Variable,
                other => NetDeclTarget::UnsupportedNet(other),
            },
            _ => NetDeclTarget::Unknown,
        }
    }

    /// The declaration-initializer constants of a `vpiNetDeclAssign`
    /// continuous assignment whose LHS resolves to an unpacked array
    /// (`reg [7:0] m [0:3] = '{…}`), or `None` when the assignment is not an
    /// array initializer.
    fn cont_assign_array_init(
        &self,
        path: &str,
        ca: NodeId,
    ) -> Result<Option<(NodeId, Vec<IrConst>)>, String> {
        let target = match self.cont_assign_array_target(ca) {
            Some(t) => t,
            None => return Ok(None),
        };
        if !matches!(self.kind(target), NodeKind::Array { .. }) {
            return Ok(None);
        }
        let name = self.node(target).name.clone();
        let rhs = self
            .node(ca)
            .children
            .get(1)
            .copied()
            .ok_or_else(|| format!("array initializer for `{name}` in `{path}` without RHS"))?;
        let vals = self.array_init_consts(path, &name, rhs)?;
        Ok(Some((target, vals)))
    }

    /// The declaration-initializer constant of a `vpiNetDeclAssign` whose LHS
    /// is a scalar variable-like object (`reg y = 0`). The caller classifies
    /// the target first; true nets never enter this constant-only path.
    fn scalar_decl_init(
        &self,
        path: &str,
        ca: NodeId,
    ) -> Result<Option<(SignalInfo, IrConst)>, String> {
        let lhs = match self.node(ca).children.first() {
            Some(l) => *l,
            None => return Ok(None),
        };
        let rhs = match self.node(ca).children.get(1) {
            Some(r) => *r,
            None => return Ok(None),
        };
        // Whole-signal LHS only: `resolve_signal_id` rejects selects and
        // arrays (the latter are registered as refs to `Array` nodes, which
        // carry no `SignalInfo`).
        let (_, info) = match self.resolve_signal_id(path, lhs) {
            Ok(g) => g,
            Err(_) => return Ok(None),
        };
        // The RHS is a constant expression in practice (Surelog folds
        // declaration-initializer expressions at elaboration); a plain
        // constant first, then constant-foldable operations/params.  Anything
        // non-constant falls through to the emission error path.
        let c = match self.const_of_node(rhs) {
            Ok(c) => c,
            Err(_) => match self.eval_bits(rhs) {
                Ok(v) => val_to_const(&v)?,
                Err(_) => return Ok(None),
            },
        };
        Ok(Some((info, c)))
    }

    /// The declaration-initializer constant of a scalar VARIABLE whose init
    /// lives on the var's `vpiExpr` (`logic l = 1'b0;`, `int x = 5;`).  The
    /// RHS is a constant expression in practice (Surelog folds
    /// declaration-initializer expressions at elaboration): a plain constant
    /// first, then constant-foldable operations/params via `eval_bits` (which
    /// resolves parameter references through `param_vals`).  Anything
    /// non-constant is rejected — v1 variable initializers must be constant
    /// expressions.
    fn var_decl_init(&self, path: &str, name: &str, init: NodeId) -> Result<IrConst, String> {
        match self.const_of_node(init) {
            Ok(c) => Ok(c),
            Err(_) => match self.eval_bits(init) {
                Ok(v) => val_to_const(&v),
                Err(_) => Err(format!(
                    "variable initializer is not a constant expression in `{name}` in `{path}`"
                )),
            },
        }
    }

    /// The constant operands of an assignment-pattern (`'{…}`) initializer
    /// expression, in linear-index order.
    fn array_init_consts(
        &self,
        path: &str,
        name: &str,
        init: NodeId,
    ) -> Result<Vec<IrConst>, String> {
        let operands: Vec<NodeId> = match self.kind(init) {
            NodeKind::Expr(ExprKind::Operation { op, operands, .. })
                if *op == vpi::vpiAssignmentPatternOp =>
            {
                operands.clone()
            }
            other => {
                return Err(format!(
                    "array `{name}` in `{path}` has an unsupported declaration \
                     initializer: {other:?}"
                ))
            }
        };
        operands
            .iter()
            .map(|o| match self.kind(*o) {
                NodeKind::Expr(ExprKind::Constant { .. }) => self.const_of_node(*o),
                other => Err(format!(
                    "array `{name}` in `{path}`: initializer element is not a \
                     constant ({other:?})"
                )),
            })
            .collect()
    }

    /// Lower an `Array` arena node: element width, per-dimension bounds/sizes,
    /// total size and (constant) declaration initializer.  Rejects
    /// non-constant dimension bounds, unsupported element types and oversized
    /// arrays with clear messages.
    fn array_info(
        &mut self,
        path: &str,
        name: &str,
        node: NodeId,
        ty: &crate::core::model::TypeInfo,
    ) -> Result<ArrayInfo, String> {
        let meta = self
            .db
            .arrays
            .get(&node)
            .ok_or_else(|| format!("array `{name}` in `{path}` has no captured metadata"))?;
        let elem_width = match ty.kind.as_str() {
            "real" | "shortreal" | "real_array" | "shortreal_array" => {
                return Err(format!(
                    "array `{name}` in `{path}` has unsupported element type `{}`",
                    ty.kind
                ))
            }
            "int" | "integer" | "time" | "longint" | "byte" | "shortint" | "logic" | "bit" => {
                ty.width.unwrap_or(1)
            }
            _ => {
                return Err(format!(
                    "array `{name}` in `{path}` has unsupported element type `{}`",
                    ty.kind
                ))
            }
        };
        if elem_width > LLG_MAX_WIDTH {
            return Err(format!(
                "array `{name}` in `{path}` has {elem_width}-bit elements; the v1 \
                 runtime supports at most {LLG_MAX_WIDTH}"
            ));
        }
        let mut dims: Vec<(i32, i32)> = Vec::new();
        for d in &meta.dims {
            match d {
                Some((l, r)) => {
                    dims.push((*l, *r));
                }
                None => {
                    return Err(format!(
                        "array `{name}` in `{path}` has a dimension whose bounds are \
                         not plain constants (e.g. an implicit `[N]` size); \
                         declare the range explicitly, e.g. `[0:N-1]`"
                    ))
                }
            }
        }
        let init = match meta.init {
            Some(eid) => Some(self.array_init_consts(path, name, eid)?),
            None => None,
        };
        let ir = self.model.arrays.len();
        let total = dims
            .iter()
            .map(|(l, r)| ((*l as i64 - *r as i64).abs() + 1) as u64)
            .product::<u64>();
        self.model.arrays.push(crate::sim::ir::IrArray {
            c_name: global_name(path, name),
            hdl_name: self.waveform_name(node),
            elem_width,
            signed: ty.signed,
            dims: dims.clone(),
            total,
        });
        Ok(ArrayInfo {
            global: global_name(path, name),
            elem_width,
            signed: ty.signed,
            dims,
            init,
            ir,
        })
    }

    // ── Functions and tasks ───────────────────────────────────────────────

    /// Emit a `static` prototype for every function/task in the instance
    /// tree, so bodies may call each other regardless of declaration order.
    /// Delay-bearing tasks are never emitted as C functions (they are inlined
    /// at their call sites), so they get no prototype.
    fn emit_func_prototypes(&mut self, inst: NodeId) -> Result<(), String> {
        for c in &self.node(inst).children {
            if let NodeKind::FuncTask { is_task, .. } = self.kind(*c) {
                if *is_task && self.task_has_wait(*c, inst) {
                    continue;
                }
                let (is_task_f, ret, formals) = self.func_info(*c)?;
                let c_name =
                    self.func_names.get(c).cloned().ok_or_else(|| {
                        format!("function `{}` has no C name", self.node(*c).name)
                    })?;
                // Register the model entry (call-site lowering and the C
                // renderers resolve through it).
                let ir = self.model.funcs.len();
                let formals_ir = formals
                    .iter()
                    .map(|(io, is_out)| match self.kind(*io) {
                        NodeKind::FuncArg { ty, .. } => IrFormal {
                            is_out: *is_out,
                            width: ty.width.unwrap_or(0),
                            signed: ty.signed,
                        },
                        _ => unreachable!("formal kind"),
                    })
                    .collect();
                self.model.funcs.push(crate::sim::ir::IrFunc {
                    c_name,
                    ret: ret.map(|(w, s)| IrType::Packed {
                        width: w,
                        signed: s,
                    }),
                    formals: formals_ir,
                    locals: Vec::new(),
                    pre_fns: Vec::new(),
                    body: Vec::new(),
                });
                self.func_meta.insert(
                    *c,
                    FuncMeta {
                        ir,
                        is_task: is_task_f,
                        ret,
                        formals,
                    },
                );
            }
        }
        for c in &self.node(inst).children {
            if matches!(self.kind(*c), NodeKind::ModuleInst { .. }) {
                self.emit_func_prototypes(*c)?;
            }
        }
        Ok(())
    }

    /// Emit the C function body for every function/task in the instance tree.
    /// Delay-bearing tasks are inlined at their call sites and never get a C
    /// function body.
    fn emit_func_bodies(&mut self, inst: NodeId) -> Result<(), String> {
        for c in &self.node(inst).children {
            if let NodeKind::FuncTask { is_task, .. } = self.kind(*c) {
                if *is_task && self.task_has_wait(*c, inst) {
                    continue;
                }
                let path = self.instance_path_of(inst);
                self.emit_func_task(&path, inst, *c)?;
            }
        }
        for c in &self.node(inst).children {
            if matches!(self.kind(*c), NodeKind::ModuleInst { .. }) {
                self.emit_func_bodies(*c)?;
            }
        }
        Ok(())
    }

    /// `(return type, params, depth)` → `(declaration prefix, formals)`.
    /// Functions pass inputs by value; tasks pass outputs/inouts first as
    /// `sv4_t*` pointers, then inputs by value.  Both end with `int depth`.
    fn func_signature(&self, ft: NodeId) -> Result<(String, Vec<(NodeId, bool)>), String> {
        let (is_task, ret, formals) = self.func_info(ft)?;
        let c_name = self
            .func_names
            .get(&ft)
            .cloned()
            .ok_or_else(|| format!("function `{}` has no C name", self.node(ft).name))?;
        let ret_t = if is_task || ret.is_none() {
            "void"
        } else {
            "sv4_t"
        };
        let mut params = Vec::new();
        // Tasks: outputs first, then inputs.  The parameter names (`o{idx}` /
        // `a{idx}`) use the formal's index in the formals list, matching the
        // `arg_read`/`arg_write` maps built when emitting the body.
        for (idx, (_, is_out)) in formals.iter().enumerate() {
            if *is_out {
                params.push(format!("sv4_t* o{idx}"));
            }
        }
        for (idx, (_, is_out)) in formals.iter().enumerate() {
            if !*is_out {
                params.push(format!("sv4_t a{idx}"));
            }
        }
        params.push("int depth".to_string());
        Ok((
            format!("static {ret_t} {c_name}({}", params.join(", ")),
            formals,
        ))
    }

    /// `(is_task, return width/signed, (io_decl node, is_output) in formal
    /// order)` of a FuncTask node.  The return width is `None` for void
    /// functions and tasks.
    // The tuple mirrors UHDM's function/task signature without introducing a
    // public one-off type solely for this private lowering boundary.
    #[allow(clippy::type_complexity)]
    fn func_info(
        &self,
        ft: NodeId,
    ) -> Result<(bool, Option<(u32, bool)>, Vec<(NodeId, bool)>), String> {
        let (is_task, ret) = match self.kind(ft) {
            NodeKind::FuncTask { is_task, ret, .. } => (*is_task, ret.clone()),
            _ => return Err("non-FuncTask passed to func_info".to_string()),
        };
        let ret = match ret {
            Some(ty) => {
                if is_real_kind(&ty.kind) {
                    return Err(format!(
                        "real/shortreal function return `{}` is not supported in v1",
                        self.node(ft).name
                    ));
                }
                match ty.width {
                    Some(w) => {
                        if w > LLG_MAX_WIDTH {
                            return Err(format!(
                                "return type of `{}` is {w} bits wide; the v1 runtime \
                             supports at most {LLG_MAX_WIDTH}",
                                self.node(ft).name
                            ));
                        }
                        Some((w, ty.signed))
                    }
                    None => {
                        return Err(format!(
                            "return type of `{}` has no width",
                            self.node(ft).name
                        ))
                    }
                }
            }
            None => None,
        };
        let mut formals = Vec::new();
        for c in &self.node(ft).children {
            match self.kind(*c) {
                NodeKind::FuncArg { direction, ty, .. } => {
                    if is_real_kind(&ty.kind) {
                        return Err(
                            "real/shortreal function formal is not supported in v1".to_string()
                        );
                    }
                    let is_out = matches!(
                        direction,
                        crate::core::model::Direction::Output
                            | crate::core::model::Direction::Inout
                    );
                    formals.push((*c, is_out));
                }
                // The function-name return variable comes before the formals
                // in the fixed child order; skip it.
                NodeKind::Var { .. } => {}
                _ => break, // body comes after the formals
            }
        }
        Ok((is_task, ret, formals))
    }

    /// The body statement of a function/task definition: the last child that
    /// is not the return variable or a formal argument (children are laid out
    /// in fixed order — return var, formals, then the body).  An empty body is
    /// captured by the database walk as a `StmtKind::Empty` placeholder, so
    /// this always finds a body when the children exist.
    fn func_body(&self, ft: NodeId) -> Option<NodeId> {
        self.node(ft).children.iter().rev().copied().find(|c| {
            !matches!(
                self.kind(*c),
                NodeKind::Var { .. } | NodeKind::FuncArg { .. }
            )
        })
    }

    /// Emit one static C function for a function/task definition.  The body
    /// statements are emitted with the io_decls mapped to the C parameters and
    /// the locals to C locals; the function-name variable maps to a local
    /// `_ret` that `return` reads.
    fn emit_func_task(&mut self, path: &str, inst: NodeId, ft: NodeId) -> Result<(), String> {
        let (is_task, ret, formals) = self.func_info(ft)?;
        let (decl, _) = self.func_signature(ft)?;
        let c_name = self
            .func_names
            .get(&ft)
            .cloned()
            .ok_or_else(|| format!("function `{}` has no C name", self.node(ft).name))?;
        let has_ret = ret.is_some();
        let ret_var = if has_ret {
            self.node(ft).children.first().copied()
        } else {
            None
        };
        let body = self
            .func_body(ft)
            .ok_or_else(|| format!("function `{}` without a body", self.node(ft).name))?;

        // The all-X return value used by the recursion guard.
        let ret_x = match ret {
            Some((w, s)) => format!("sv4_x({w}, {})", s as u8),
            None => String::new(),
        };
        let guard = if has_ret {
            format!(
                "if (depth >= {LLG_MAX_FUNC_DEPTH}) {{\n        fprintf(stderr, \"llg: recursion limit exceeded in %s\\n\", \"{c_name}\");\n        return {ret_x};\n    }}\n"
            )
        } else {
            format!(
                "if (depth >= {LLG_MAX_FUNC_DEPTH}) {{\n        fprintf(stderr, \"llg: recursion limit exceeded in %s\\n\", \"{c_name}\");\n        return;\n    }}\n"
            )
        };

        let mut locals: HashMap<NodeId, (String, u32, bool)> = HashMap::new();
        let mut local_seq = 0usize;
        self.collect_func_locals(body, &mut locals, &mut local_seq, "")?;

        // Function-name return variable → `_ret` local.
        let ret_ctx = match (has_ret, ret_var) {
            (true, Some(rv)) => {
                let (w, s) = ret.expect("ret width known");
                Some(RetCtx {
                    c_name: "_ret".to_string(),
                    width: w,
                    signed: s,
                    node: Some(rv),
                })
            }
            _ => None,
        };

        let mut arg_read: HashMap<NodeId, ArgMap> = HashMap::new();
        let mut arg_ir: HashMap<NodeId, IrExpr> = HashMap::new();
        let mut arg_write: HashMap<NodeId, String> = HashMap::new();
        for (idx, (io, is_out)) in formals.iter().enumerate() {
            let (w, s) = match self.kind(*io) {
                NodeKind::FuncArg { ty, .. } => {
                    if is_real_kind(&ty.kind) {
                        return Err(
                            "real/shortreal function formal is not supported in v1".to_string()
                        );
                    }
                    match ty.width {
                        Some(w) if w <= LLG_MAX_WIDTH => (w, ty.signed),
                        Some(w) => {
                            return Err(format!(
                                "formal `{}` of `{c_name}` is {w} bits wide; the v1 runtime \
                             supports at most {LLG_MAX_WIDTH}",
                                self.node(*io).name
                            ))
                        }
                        None => {
                            return Err(format!(
                                "formal `{}` of `{c_name}` has no width",
                                self.node(*io).name
                            ))
                        }
                    }
                }
                _ => unreachable!("formal kind"),
            };
            if *is_out {
                arg_write.insert(*io, format!("o{idx}"));
                arg_ir.insert(*io, formal_read_expr(idx, w, s));
                arg_read.insert(
                    *io,
                    ArgMap {
                        width: w,
                        signed: s,
                    },
                );
            } else {
                // Input formal: a by-value C parameter.  Writing an input
                // formal is legal SystemVerilog (it is a local copy), so the
                // parameter itself is also a valid write target.
                arg_write.insert(*io, format!("&a{idx}"));
                arg_ir.insert(*io, formal_read_expr(idx, w, s));
                arg_read.insert(
                    *io,
                    ArgMap {
                        width: w,
                        signed: s,
                    },
                );
            }
        }

        let func_ctx = FuncCtx {
            name: self.node(ft).name.clone(),
            is_task,
            ret: ret_ctx.clone(),
            arg_read,
            arg_ir,
            arg_write,
            locals: locals.clone(),
            ret_node: ret_var,
            def_node: Some(ft),
        };
        let meta_ir = self
            .func_meta
            .get(&ft)
            .map(|m| m.ir)
            .ok_or_else(|| format!("function `{}` has no C name", self.node(ft).name))?;
        self.cur_fn_ir = Some(meta_ir);
        // Lower the body under the function context; the guard, `_ret`
        // declaration and locals are rendered by the backend from the
        // `IrFunc` metadata.
        let (body_stmts, pre_fns) = {
            let mut ctx = EmitCtx::new(
                self,
                path.to_string(),
                inst,
                "depth + 1",
                Some(func_ctx),
                None,
                false,
            );
            let body_stmts = ctx.lower_stmt(body)?;
            let pre_fns = std::mem::take(&mut ctx.pre_fns);
            (body_stmts, pre_fns)
        };
        // Restore the process-level context for whatever is lowered next
        // (continuous assignments, processes).
        self.func = None;
        self.cur_fn_ir = None;
        self.depth_arg = "0".to_string();

        let ir_locals = {
            let mut names = locals.into_iter().collect::<Vec<_>>();
            names.sort_by_key(|(id, _)| id.0);
            names
                .into_iter()
                .map(|(_, (c_name, width, signed))| crate::sim::ir::IrLocal {
                    c_name,
                    width,
                    signed,
                })
                .collect()
        };
        let no_entry = format!("function `{}` has no model entry", self.node(ft).name);
        let entry = self.model.funcs.get_mut(meta_ir).ok_or(no_entry)?;
        entry.locals = ir_locals;
        entry.pre_fns = pre_fns;
        entry.body = body_stmts;
        let _ = (guard, decl, has_ret, ret_x, c_name.as_str());
        Ok(())
    }

    /// Collect the local variables declared by a function/task body's begin
    /// blocks (recursively) into `locals`, keyed by the var arena node.
    /// `prefix` disambiguates the C local names across inline sites (each
    /// inlined task body gets its own prefix); pass `""` for C function
    /// bodies, whose locals are scoped per function.
    fn collect_func_locals(
        &self,
        node: NodeId,
        locals: &mut HashMap<NodeId, (String, u32, bool)>,
        seq: &mut usize,
        prefix: &str,
    ) -> Result<(), String> {
        if let NodeKind::Var { ty } = self.kind(node) {
            if is_real_kind(&ty.kind) {
                return Err(format!(
                    "real/shortreal function local `{}` is not supported in v1",
                    self.node(node).name
                ));
            }
            let w = match ty.width {
                Some(w) if w <= LLG_MAX_WIDTH => w,
                Some(w) => {
                    return Err(format!(
                        "local `{}` is {w} bits wide; the v1 runtime supports at most \
                         {LLG_MAX_WIDTH}",
                        self.node(node).name
                    ))
                }
                None => return Err(format!("local `{}` has no width", self.node(node).name)),
            };
            let cname = format!("{prefix}_l{seq}");
            *seq += 1;
            locals.insert(node, (cname, w, ty.signed));
            return Ok(());
        }
        for c in &self.node(node).children {
            self.collect_func_locals(*c, locals, seq, prefix)?;
        }
        Ok(())
    }

    /// Resolve a call site's callee to its FuncTask arena node.  Prefers the
    /// captured `callee` (checked to live inside `inst`); falls back to a name
    /// lookup among the owning instance's function/task definitions.  Callees
    /// outside the instance (hierarchical calls) are rejected.
    fn resolve_callee(
        &self,
        inst: NodeId,
        name: &str,
        is_task: bool,
        callee: Option<NodeId>,
    ) -> Result<NodeId, String> {
        if let Some(ft) = callee {
            let mut cur = self.node(ft).parent;
            while let Some(p) = cur {
                if p == inst {
                    return Ok(ft);
                }
                cur = self.node(p).parent;
            }
            return Err(format!(
                "hierarchical call `{name}` is not supported (callee outside \
                 the calling instance)"
            ));
        }
        for c in &self.node(inst).children {
            if let NodeKind::FuncTask { is_task: t, .. } = self.kind(*c) {
                if *t == is_task && self.node(*c).name == name {
                    return Ok(*c);
                }
            }
        }
        Err(format!(
            "cannot resolve callee `{name}` in `{}`",
            self.node(inst).name
        ))
    }

    /// Whether a task's execution can suspend: its body (transitively over
    /// called tasks) contains a delay, event control or wait statement.
    /// Delay-bearing tasks are inlined at their call sites; the others become
    /// plain C functions.
    fn task_has_wait(&self, ft: NodeId, inst: NodeId) -> bool {
        let mut seen: HashSet<NodeId> = HashSet::new();
        self.task_has_wait_inner(ft, inst, &mut seen)
    }

    fn task_has_wait_inner(&self, ft: NodeId, inst: NodeId, seen: &mut HashSet<NodeId>) -> bool {
        if !seen.insert(ft) {
            return false;
        }
        let Some(body) = self.func_body(ft) else {
            return false;
        };
        self.node_has_wait(body, inst, seen)
    }

    fn node_has_wait(&self, node: NodeId, inst: NodeId, seen: &mut HashSet<NodeId>) -> bool {
        match self.kind(node) {
            NodeKind::Stmt(
                StmtKind::DelayControl { .. }
                | StmtKind::EventControl { .. }
                | StmtKind::Wait { .. },
            ) => true,
            NodeKind::FuncCall {
                is_task: true,
                callee,
                ..
            } => {
                if let Ok(ft) = self.resolve_callee(inst, &self.node(node).name, true, *callee) {
                    if self.task_has_wait_inner(ft, inst, seen) {
                        return true;
                    }
                }
                self.node(node)
                    .children
                    .iter()
                    .any(|c| self.node_has_wait(*c, inst, seen))
            }
            _ => self
                .node(node)
                .children
                .iter()
                .any(|c| self.node_has_wait(*c, inst, seen)),
        }
    }

    /// A call argument Surelog synthesizes for a *missing named* argument: a
    /// location-less `0` constant (genuine `0` literals carry a source line).
    fn is_synthetic_arg(&self, a: NodeId) -> bool {
        if self.node(a).line != 0 {
            return false;
        }
        match self.kind(a) {
            NodeKind::Expr(ExprKind::Constant { value, .. }) => matches!(
                value,
                ValueData::Int(0) | ValueData::UInt(0) | ValueData::Scalar(vpi::vpi0)
            ),
            _ => false,
        }
    }

    /// Bind a call's positional arguments to the callee's formals, in formal
    /// order.  Missing (or Surelog-synthesized) arguments fall back to the
    /// formal's default expression; a formal without a default errors.
    fn bind_call_args(
        &self,
        formals: &[(NodeId, bool)],
        args: &[NodeId],
    ) -> Result<Vec<BoundArg>, String> {
        let mut bound = Vec::with_capacity(formals.len());
        for (idx, (io, _)) in formals.iter().enumerate() {
            let (w, s) = match self.kind(*io) {
                NodeKind::FuncArg { ty, .. } => {
                    if is_real_kind(&ty.kind) {
                        return Err(
                            "real/shortreal function formal is not supported in v1".to_string()
                        );
                    }
                    match ty.width {
                        Some(w) if w <= LLG_MAX_WIDTH => (w, ty.signed),
                        Some(w) => {
                            return Err(format!(
                                "formal `{}` is {w} bits wide; the v1 runtime supports \
                             at most {LLG_MAX_WIDTH}",
                                self.node(*io).name
                            ))
                        }
                        None => {
                            return Err(format!("formal `{}` has no width", self.node(*io).name))
                        }
                    }
                }
                _ => unreachable!("non-FuncArg in formals"),
            };
            let (expr, is_default) = match args.get(idx) {
                Some(a) if !self.is_synthetic_arg(*a) => (*a, false),
                _ => match self.kind(*io) {
                    NodeKind::FuncArg { default, .. } => (
                        default.ok_or_else(|| {
                            format!(
                                "missing argument for formal `{}` of `{}`",
                                self.node(*io).name,
                                self.node(*io)
                                    .parent
                                    .map(|p| self.node(p).name.clone())
                                    .unwrap_or_default()
                            )
                        })?,
                        true,
                    ),
                    _ => unreachable!(),
                },
            };
            bound.push(BoundArg {
                width: w,
                signed: s,
                expr,
                is_default,
            });
        }
        Ok(bound)
    }

    /// Lower the C value expression for bound argument `idx` of a call and
    /// record it in `arg_codes[idx]` (rendered, for the legacy string paths)
    /// and `arg_irs[idx]` (IR) for later formals' default expressions to
    /// reference.
    ///
    /// A formal's default expression (`input logic b = a + 1`) is written in
    /// the callee's scope and may reference earlier formals; it is lowered
    /// under a temporary formal-aware context mapping those formals to their
    /// already-lowered argument expressions.  Caller provided arguments are
    /// lowered in the caller's own context.
    fn lower_bound_arg_code(
        &mut self,
        scope_path: &str,
        formals: &[(NodeId, bool)],
        bound: &[BoundArg],
        idx: usize,
        arg_codes: &mut [Option<String>],
        arg_irs: &mut Vec<Option<IrExpr>>,
    ) -> Result<(String, IrExpr), String> {
        let (w, s) = (bound[idx].width, bound[idx].signed);
        let e_ir = if bound[idx].is_default {
            let mut arg_read: HashMap<NodeId, ArgMap> = HashMap::new();
            let mut arg_ir: HashMap<NodeId, IrExpr> = HashMap::new();
            for (j, (io, _)) in formals.iter().enumerate().take(idx) {
                if arg_codes[j].is_some() {
                    let (wj, sj) = (bound[j].width, bound[j].signed);
                    arg_read.insert(
                        *io,
                        ArgMap {
                            width: wj,
                            signed: sj,
                        },
                    );
                    if let Some(ir) = arg_irs[j].clone() {
                        arg_ir.insert(*io, ir);
                    }
                }
            }
            let temp_func = FuncCtx {
                name: String::new(),
                is_task: false,
                ret: None,
                arg_read,
                arg_ir,
                arg_write: HashMap::new(),
                locals: HashMap::new(),
                ret_node: None,
                def_node: None,
            };
            let saved = self.func.take();
            self.func = Some(temp_func);
            let res = self.lower_expr(scope_path, bound[idx].expr);
            self.func = saved;
            res?
        } else {
            self.lower_expr(scope_path, bound[idx].expr)?
        };
        let e_ir = apply_assignment_expression_width(e_ir, w);
        let conv_ir = ir_to_vector(e_ir, w, s)?;
        let code = self.render_ir_code(&conv_ir)?;
        arg_codes[idx] = Some(code.clone());
        if arg_irs.len() <= idx {
            arg_irs.resize(idx + 1, None);
        }
        arg_irs[idx] = Some(conv_ir.clone());
        Ok((code, conv_ir))
    }

    /// Lower a `func_call` expression used as a value: `fn_<callee>(<args>,
    /// <depth>)` with output/inout formals bound to caller-side temps that
    /// are written back into the bound actuals after the call (the backend
    /// wraps those into one GNU statement expression).
    fn lower_func_call_expr(
        &mut self,
        scope_path: &str,
        h: NodeId,
        name: &str,
        callee: Option<NodeId>,
    ) -> Result<IrExpr, String> {
        let ft = self.resolve_callee(self.inst, name, false, callee)?;
        let meta = self
            .func_meta
            .get(&ft)
            .cloned()
            .ok_or_else(|| format!("function `{name}` has no C name"))?;
        if meta.is_task {
            return Err(format!(
                "task call `{name}` used as an expression in `{scope_path}`"
            ));
        }
        let formals = meta.formals.clone();
        let args: Vec<NodeId> = self.node(h).children.clone();
        let bound = self.bind_call_args(&formals, &args)?;
        // `ret` is `None` for void functions; using one as a value (legal in
        // Surelog's parse, e.g. `out <= vf(4'd2);`) emits the call for its
        // side effects and yields all-X.
        let ret_val = meta.ret;
        let (ret_w, ret_s) = ret_val.unwrap_or((1, false));

        let mut out_args: Vec<IrCallArg> = Vec::new();
        let mut in_args: Vec<IrCallArg> = Vec::new();
        let mut arg_codes: Vec<Option<String>> = vec![None; formals.len()];
        let mut arg_irs: Vec<Option<IrExpr>> = vec![None; formals.len()];
        for (idx, (io, is_out)) in formals.iter().enumerate() {
            if *is_out {
                let tname = format!("_t{}", h.0);
                let (_init_code, init_ir) =
                    self.lower_call_temp_init(scope_path, *io, &bound[idx])?;
                let wb = self.lower_lhs(scope_path, bound[idx].expr)?;
                // The temp is the correctly-sized value of the formal while
                // the call runs (all-X for outputs, the actual for inouts).
                arg_codes[idx] = Some(tname.clone());
                arg_irs[idx] = Some(IrExpr::new(
                    IrExprKind::LocalRead(tname.clone()),
                    bound[idx].width,
                    bound[idx].signed,
                    None,
                ));
                out_args.push(IrCallArg::OutTemp {
                    name: tname,
                    init: init_ir.map(Box::new),
                    writeback: Box::new(wb),
                });
            }
        }
        for (idx, (_, is_out)) in formals.iter().enumerate() {
            if !*is_out {
                let (_code, ir) = self.lower_bound_arg_code(
                    scope_path,
                    &formals,
                    &bound,
                    idx,
                    &mut arg_codes,
                    &mut arg_irs,
                )?;
                in_args.push(IrCallArg::Val(ir));
            }
        }
        out_args.extend(in_args);
        if ret_val.is_none() {
            self.warnings.push(format!(
                "void function `{name}` used as a value in `{scope_path}`; result is X"
            ));
        }
        let depth = parse_depth(&self.depth_arg);
        Ok(IrExpr::new(
            IrExprKind::CallFn(Box::new(IrCallExpr {
                f: meta.ir,
                args: out_args,
                depth,
                void_x: ret_val.is_none(),
            })),
            ret_w,
            ret_s,
            None,
        ))
    }

    /// Lower the initializer of the caller-side temp for an output/inout
    /// formal: all-X (`None`) for outputs, the actual's current value
    /// (converted to the formal's vector shape) for inouts.  Also returns the
    /// rendered initializer for the legacy string paths.
    fn lower_call_temp_init(
        &mut self,
        scope_path: &str,
        io: NodeId,
        b: &BoundArg,
    ) -> Result<(String, Option<IrExpr>), String> {
        match self.kind(io) {
            NodeKind::FuncArg {
                direction: crate::core::model::Direction::Inout,
                ..
            } => {
                let e = self.lower_expr(scope_path, b.expr)?;
                let e = apply_assignment_expression_width(e, b.width);
                let conv = ir_to_vector(e, b.width, b.signed)?;
                let code = self.render_ir_code(&conv)?;
                Ok((code, Some(conv)))
            }
            _ => Ok((format!("sv4_x({}, {})", b.width, b.signed as u8), None)),
        }
    }

    /// Resolve a function/task body write target (output/inout formal, local
    /// or return variable) to an LHS.  Tries `node` first (locals and the
    /// return var are indexed), then `name`.
    fn func_write_target(&self, node: NodeId, name: &str) -> Option<Lhs> {
        let f = self.func.as_ref()?;
        if let Some(addr) = f.arg_write.get(&node) {
            if let Some(am) = f.arg_read.get(&node) {
                return Some(Lhs::WholeRef {
                    addr: addr.clone(),
                    width: am.width,
                    signed: am.signed,
                });
            }
        }
        if let Some((cname, w, s)) = f.locals.get(&node) {
            return Some(Lhs::WholeRef {
                addr: format!("&{cname}"),
                width: *w,
                signed: *s,
            });
        }
        if f.ret_node == Some(node) {
            if let Some(r) = &f.ret {
                return Some(Lhs::WholeRef {
                    addr: format!("&{}", r.c_name),
                    width: r.width,
                    signed: r.signed,
                });
            }
        }
        for (io, addr) in &f.arg_write {
            if self.node(*io).name == name {
                if let Some(am) = f.arg_read.get(io) {
                    return Some(Lhs::WholeRef {
                        addr: addr.clone(),
                        width: am.width,
                        signed: am.signed,
                    });
                }
            }
        }
        for (nid, (cname, w, s)) in &f.locals {
            if self.node(*nid).name == name {
                return Some(Lhs::WholeRef {
                    addr: format!("&{cname}"),
                    width: *w,
                    signed: *s,
                });
            }
        }
        if let Some(r) = &f.ret {
            if r.node.map(|n| self.node(n).name == name).unwrap_or(false) {
                return Some(Lhs::WholeRef {
                    addr: format!("&{}", r.c_name),
                    width: r.width,
                    signed: r.signed,
                });
            }
        }
        None
    }

    // ── PCA site pre-scan (two-phase discovery, phase 1) ─────────────────────

    /// Allocate every procedural continuous assignment site in the instance
    /// tree BEFORE any body lowers.  Traverses module instances, generate
    /// scopes and per-iteration instances exactly like the Procs emission
    /// pass (interface copies excluded — they never emit processes), so
    /// lower-time lookups into [`Codegen::pca_sites`] see every site
    /// regardless of process/source order: a `deassign` in a process that
    /// lowers BEFORE the process carrying the matching `assign` must still
    /// clear its enable (a lower-time allocation alone would turn it into a
    /// permanent no-op), and the multiple-active-sites reject becomes
    /// order-independent too.  Function/task definition bodies are not
    /// scanned here: they always lower before any process body, so sites
    /// inside them still allocate ahead of every process-body deassign.
    fn prescan_pca_sites(&mut self, inst: NodeId, path: &str) -> Result<(), String> {
        let iface_copy = matches!(
            self.kind(inst),
            NodeKind::ModuleInst {
                is_interface: true,
                ..
            }
        ) && self.iface_copy_insts.contains(&inst);
        if !iface_copy {
            for c in &self.node(inst).children {
                if matches!(self.kind(*c), NodeKind::Process { .. }) {
                    self.prescan_pca_proc(inst, path, *c)?;
                }
            }
            for c in &self.node(inst).children {
                if !matches!(self.kind(*c), NodeKind::GenScopeArray) {
                    continue;
                }
                for gs in &self.node(*c).children {
                    if !matches!(self.kind(*gs), NodeKind::GenScope) {
                        continue;
                    }
                    let gs_path = self
                        .gen_scope_paths
                        .get(gs)
                        .cloned()
                        .unwrap_or_else(|| path.to_string());
                    for cc in &self.node(*gs).children {
                        match self.kind(*cc) {
                            NodeKind::Process { .. } => {
                                self.prescan_pca_proc(inst, &gs_path, *cc)?
                            }
                            // Per-iteration instances under a gen scope own
                            // their processes; recurse like the Procs pass.
                            NodeKind::ModuleInst { .. } => {
                                let child_path = self.instance_path_of(*cc);
                                self.prescan_pca_sites(*cc, &child_path)?;
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        for c in &self.node(inst).children {
            if matches!(self.kind(*c), NodeKind::ModuleInst { .. }) {
                let child_path = self.instance_path_of(*c);
                self.prescan_pca_sites(*c, &child_path)?;
            }
        }
        Ok(())
    }

    /// Pre-scan ONE process body: collect its ProcContAssign statements and
    /// claim a site for each (enable allocated now, guard materialized when
    /// the statement itself lowers).
    fn prescan_pca_proc(&mut self, inst: NodeId, path: &str, proc: NodeId) -> Result<(), String> {
        let stmt = self
            .node(proc)
            .children
            .first()
            .copied()
            .ok_or_else(|| format!("process without statement in `{path}`"))?;
        let mut nodes = Vec::new();
        self.collect_pca_nodes(stmt, &mut nodes);
        if nodes.is_empty() {
            return Ok(());
        }
        let mut ctx = EmitCtx::new(self, path.to_string(), inst, "0", None, None, false);
        ctx.claim_pca_sites(&nodes)
    }

    /// Collect every `StmtKind::ProcContAssign` node in the statement tree
    /// rooted at `root`, in source order.  Recursion descends through
    /// statement nodes only — expression subtrees never contain statements,
    /// and descending into refs could wander into unrelated declarations.
    fn collect_pca_nodes(&self, root: NodeId, out: &mut Vec<NodeId>) {
        match self.kind(root) {
            NodeKind::Stmt(StmtKind::ProcContAssign { .. }) => out.push(root),
            NodeKind::Stmt(_) => {
                for c in &self.node(root).children {
                    self.collect_pca_nodes(*c, out);
                }
            }
            _ => {}
        }
    }

    fn emit_pass(&mut self, top: NodeId, pass: Pass) -> Result<(), String> {
        let path = self.instance_path_of(top);
        self.emit_pass_inst(top, &path, pass)
    }

    fn emit_pass_inst(&mut self, inst: NodeId, path: &str, pass: Pass) -> Result<(), String> {
        // Instances inside generate scopes (per-iteration instances) are
        // emitted like the instance's own children: their links, processes and
        // continuous assignments all run under the gen-scope path.
        let emit_gen_scope_children =
            |cg: &mut Self, gs: &NodeId, pass: Pass, path: &str| -> Result<(), String> {
                let gs_path = cg
                    .gen_scope_paths
                    .get(gs)
                    .cloned()
                    .unwrap_or_else(|| path.to_string());
                for cc in &cg.node(*gs).children {
                    match cg.kind(*cc) {
                        NodeKind::ContAssign { .. } if pass == Pass::Comb => {
                            cg.emit_cont_assign(inst, &gs_path, *cc)?
                        }
                        NodeKind::Gate { .. } if pass == Pass::Comb => {
                            cg.emit_gate(inst, &gs_path, *cc)?
                        }
                        NodeKind::Process { .. } if pass == Pass::Procs => {
                            cg.emit_process(inst, &gs_path, *cc)?;
                        }
                        NodeKind::ModuleInst { .. } => {
                            let child_path = cg.instance_path_of(*cc);
                            match pass {
                                Pass::Comb => cg.emit_pass_inst(*cc, &child_path, Pass::Comb)?,
                                Pass::Links => {
                                    cg.emit_links(&gs_path, *cc)?;
                                    cg.emit_pass_inst(*cc, &child_path, Pass::Links)?;
                                }
                                Pass::Procs => cg.emit_pass_inst(*cc, &child_path, Pass::Procs)?,
                            }
                        }
                        _ => {}
                    }
                }
                Ok(())
            };
        match pass {
            Pass::Comb => {
                for c in &self.node(inst).children {
                    match self.kind(*c) {
                        NodeKind::ContAssign { .. } => self.emit_cont_assign(inst, path, *c)?,
                        NodeKind::Gate { .. } => self.emit_gate(inst, path, *c)?,
                        _ => {}
                    }
                }
                for c in &self.node(inst).children {
                    if matches!(self.kind(*c), NodeKind::GenScopeArray) {
                        for gs in &self.node(*c).children {
                            if matches!(self.kind(*gs), NodeKind::GenScope) {
                                emit_gen_scope_children(self, gs, Pass::Comb, path)?;
                            }
                        }
                    }
                }
                for c in &self.node(inst).children {
                    if matches!(self.kind(*c), NodeKind::ModuleInst { .. }) {
                        let child_path = self.instance_path_of(*c);
                        self.emit_pass_inst(*c, &child_path, Pass::Comb)?;
                    }
                }
            }
            Pass::Links => {
                for c in &self.node(inst).children {
                    if matches!(self.kind(*c), NodeKind::GenScopeArray) {
                        for gs in &self.node(*c).children {
                            if matches!(self.kind(*gs), NodeKind::GenScope) {
                                emit_gen_scope_children(self, gs, Pass::Links, path)?;
                            }
                        }
                    }
                }
                for c in &self.node(inst).children {
                    if matches!(self.kind(*c), NodeKind::ModuleInst { .. }) {
                        let child_path = self.instance_path_of(*c);
                        self.emit_links(path, *c)?;
                        self.emit_pass_inst(*c, &child_path, Pass::Links)?;
                    }
                }
            }
            Pass::Procs => {
                // Interface body processes (always/initial/always_comb blocks
                // inside an interface definition) belong to the ACTUAL
                // interface instance.  Surelog v1.86 does not clone them into
                // the per-port copies (they are just views); emitting one on a
                // copy would double-drive the member through the interface
                // link, so copies are always skipped here.
                let iface_copy = matches!(
                    self.kind(inst),
                    NodeKind::ModuleInst {
                        is_interface: true,
                        ..
                    }
                ) && self.iface_copy_insts.contains(&inst);
                for c in &self.node(inst).children {
                    if matches!(self.kind(*c), NodeKind::Process { .. }) {
                        if iface_copy {
                            continue;
                        }
                        self.emit_process(inst, path, *c)?;
                    }
                }
                // Processes inside generate scopes are emitted exactly like
                // instance processes (mirroring the Comb pass's gen-scope
                // walk); genvar references inline to the gen-scope parameter
                // values collected by `collect_gen_scope`.
                if !iface_copy {
                    for c in &self.node(inst).children {
                        if matches!(self.kind(*c), NodeKind::GenScopeArray) {
                            for gs in &self.node(*c).children {
                                if matches!(self.kind(*gs), NodeKind::GenScope) {
                                    emit_gen_scope_children(self, gs, Pass::Procs, path)?;
                                }
                            }
                        }
                    }
                }
                for c in &self.node(inst).children {
                    if matches!(self.kind(*c), NodeKind::ModuleInst { .. }) {
                        let child_path = self.instance_path_of(*c);
                        self.emit_pass_inst(*c, &child_path, Pass::Procs)?;
                    }
                }
            }
        }
        Ok(())
    }

    // ── Continuous assignments ─────────────────────────────────────────────

    fn emit_cont_assign(&mut self, inst: NodeId, path: &str, ca: NodeId) -> Result<(), String> {
        let node = self.node(ca);
        if let NodeKind::ContAssign {
            net_decl: true,
            delay: _,
        } = self.kind(ca)
        {
            // Array and variable declaration initializers are applied in
            // `main()` at collection time. True-net declarations continue
            // below and use the ordinary event-driven continuous-assignment
            // path, including RunOnce for a constant RHS.
            if self.cont_assign_array_target(ca).is_some() || self.scalar_init_ca.contains(&ca) {
                return Ok(());
            }
            match self.net_decl_target(ca) {
                NetDeclTarget::TrueNet => {}
                NetDeclTarget::UnsupportedNet(net_type) => {
                    return Err(format!(
                        "net declaration assignment in `{path}` targets unsupported net type \
                         {net_type} (only wire/tri/logic nets are supported)"
                    ));
                }
                NetDeclTarget::Array | NetDeclTarget::Variable | NetDeclTarget::Unknown => {
                    return Err(format!(
                        "declaration initializer (`net = value` at declaration) in `{path}` \
                         is not supported"
                    ));
                }
            }
        }
        let lhs = node
            .children
            .first()
            .copied()
            .ok_or_else(|| format!("continuous assignment without LHS in `{path}`"))?;
        let rhs = node
            .children
            .get(1)
            .copied()
            .ok_or_else(|| format!("continuous assignment without RHS in `{path}`"))?;
        // Callee resolution in the RHS needs the owning instance.
        self.inst = inst;
        if matches!(self.kind(ca), NodeKind::ContAssign { net_decl: true, .. })
            && self.contains_unpacked_array(rhs, &mut HashSet::new())
        {
            return Err(format!(
                "net declaration assignment in `{path}` reads an unpacked array, whose \
                 continuous sensitivity cannot be represented"
            ));
        }
        let lh = self.lower_lhs(path, lhs)?;
        let rhs_ir = self.lower_expr(path, rhs)?;
        let rhs_ir = apply_lhs_assignment_context(&self.model, &lh, rhs_ir);
        let lhs_real = matches!(&lh, IrLhs::Whole(idx)
            if matches!(self.model.signal(*idx).ty, IrType::Real { .. }));
        if lhs_real || rhs_ir.is_real() {
            return Err(format!(
                "real-valued continuous assignments are not supported in `{path}`"
            ));
        }
        let assign = IrStmt::Assign {
            lhs: lh,
            rhs: rhs_ir,
            nba: false,
        };
        // `assign #d lhs = rhs;` — the delay folds to a constant through the
        // collected parameter values and scales like a `#N` statement.  The
        // write happens D after each RHS change; a change during an open
        // window is picked up by the next iteration (no pulse filtering, see
        // below), and the t=0 first evaluation waits D too.
        let scaled_delay = match self.kind(ca) {
            NodeKind::ContAssign {
                delay: Some(de), ..
            } => {
                let dir = self.lower_expr(path, *de)?;
                if dir.is_real() {
                    return Err(format!(
                        "real-valued continuous-assignment delays are not \
                         supported in `{path}`"
                    ));
                }
                let raw = match dir.kind {
                    IrExprKind::Const(c) => const_delay_ticks(&c, path)?,
                    _ => {
                        return Err(format!(
                            "continuous-assignment delay must be a constant or \
                             parameter in `{path}`"
                        ))
                    }
                };
                let unit_ps = self.timescale_of_node(ca).unit_ps;
                Some(scale_delay_ticks(
                    raw,
                    unit_ps,
                    self.design_precision_ps,
                    path,
                )?)
            }
            _ => None,
        };
        // v1 approximation (LRM 1364-1995 §6.1.3): no pulse filtering — the
        // LHS is written with the CURRENT rhs value D after the wake, so an
        // rhs pulse shorter than D still produces a (delayed) write with the
        // post-pulse value instead of being swallowed.
        let mut body = Vec::new();
        if let Some(ticks) = scaled_delay {
            self.warnings.push(format!(
                "delayed continuous assignment in `{path}` uses the current \
                 rhs value after the #delay window (no pulse filtering)"
            ));
            body.push(IrStmt::Delay { ticks });
        }
        body.push(assign);
        let fn_name = self.new_fn_name(path, "ca");
        let sigs = self.collect_read_signals(path, rhs)?;
        let shape = if sigs.is_empty() {
            // Constant driver: evaluate once at t=0, then end (the value can
            // never change, so there is nothing to wait on).
            IrShape::RunOnce
        } else {
            IrShape::SensLoop { reads: sigs }
        };
        self.model.processes.push(IrProcess {
            c_name: fn_name,
            label: format!("{path}.assign"),
            shape,
            pre_fns: Vec::new(),
            body,
        });
        Ok(())
    }

    /// Whether an expression (including a called function body) reaches an
    /// unpacked array. Array element storage is not representable in an
    /// `IrShape::SensLoop` read set, so declaration drivers reject it rather
    /// than becoming stale after the first evaluation.
    fn contains_unpacked_array(&self, node: NodeId, visited: &mut HashSet<NodeId>) -> bool {
        if !visited.insert(node) {
            return false;
        }
        match self.kind(node) {
            NodeKind::Array { .. } | NodeKind::Expr(ExprKind::ArraySelect { .. }) => return true,
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) if matches!(self.kind(*target), NodeKind::Array { .. }) => {
                return true;
            }
            NodeKind::FuncCall {
                name,
                is_task,
                callee,
            } => {
                if let Ok(func) = self.resolve_callee(self.inst, name, *is_task, *callee) {
                    if let Some(body) = self.func_body(func) {
                        if self.contains_unpacked_array(body, visited) {
                            return true;
                        }
                    }
                }
            }
            _ => {}
        }
        self.node(node)
            .children
            .iter()
            .any(|child| self.contains_unpacked_array(*child, visited))
    }

    /// Allocate the 1-bit enable signal of one procedural continuous
    /// assignment site (`G_<path>_pca$<n>_en`).  Enables are ordinary IR
    /// signals on purpose: the optimizer's read/write collectors, branch
    /// pruning and folding see them exactly like user storage (a guard's
    /// `If(en)` condition is never constant, and an enabled signal is both
    /// read and written so `unused_storage` always keeps it).  The `$`
    /// separator cannot appear in an ident()-sanitized user name (`ident`
    /// maps it to `_`), so a synthesized enable never collides with a user
    /// variable's global — a collision would silently merge their storage.
    fn new_pca_enable(&mut self, path: &str) -> usize {
        let n = self.pca_seq;
        self.pca_seq += 1;
        let c_name = format!("G_{}_pca${}_en", ident(path), n);
        let ir = self.model.signals.len();
        self.model.signals.push(IrSignal {
            c_name,
            hdl_name: None,
            ty: IrType::Packed {
                width: 1,
                signed: false,
            },
            net_driver: None,
            omit: false,
        });
        ir
    }

    /// The multiple-active-sites reject, shared by the pre-scan claimer and
    /// lower-time re-check so both produce the same message.
    fn pca_multi_site_err(&self, lhs: NodeId, path: &str) -> String {
        format!(
            "variable `{}` in `{path}` already has a procedural continuous \
             assignment (multiple active PCA sites on one variable are not \
             supported)",
            self.node(lhs).name,
        )
    }

    fn new_fn_name(&mut self, path: &str, kind: &str) -> String {
        let n = self.proc_seq;
        self.proc_seq += 1;
        format!("p_{}_{}_{}", ident(path), kind, n)
    }

    // ── Structural gate primitives ─────────────────────────────────────────

    /// Lower one structural primitive ([`NodeKind::Gate`]) into ONE comb
    /// process shaped exactly like a continuous assignment: evaluate at
    /// spawn, then re-evaluate whenever any input-terminal signal changes
    /// (`IrShape::SensLoop`; constant drivers like pullup/pulldown use
    /// `IrShape::RunOnce`).  The output terminal is written with a
    /// whole-signal blocking write (collapsed-net members go through their
    /// driver slot automatically).
    ///
    /// Multi-input logic gates reduce their inputs left-to-right with the
    /// two-input runtime op; nand/nor/xnor negate after the full reduce.
    /// A gate delay `#D` prepends a scaled wait to the process body (the
    /// t=0 first evaluation waits too) with no pulse filtering — each wake
    /// writes the CURRENT input values D later (warned, like delayed
    /// continuous assignments).  Unsupported primitives (switches, UDPs,
    /// arrays, strengths, multi-output buf/not, width mismatches, …) are
    /// rejected with explicit errors here at lowering time.
    fn emit_gate(&mut self, inst: NodeId, path: &str, g: NodeId) -> Result<(), String> {
        let (class, prim_type, strength0, strength1, delay, terms) = match self.kind(g) {
            NodeKind::Gate {
                class,
                prim_type,
                strength0,
                strength1,
                delay,
                terms,
            } => (
                *class,
                *prim_type,
                *strength0,
                *strength1,
                *delay,
                Vec::clone(terms),
            ),
            _ => unreachable!("non-gate node passed to emit_gate"),
        };
        let gname = self.node(g).name.clone();
        let shown = if gname.is_empty() {
            "gate"
        } else {
            gname.as_str()
        };
        match class {
            PrimClass::Gate => {}
            PrimClass::Switch => {
                return Err(format!(
                    "switch/transistor primitive `{shown}` in `{path}` is not \
                     supported in v1"
                ))
            }
            PrimClass::Udp => {
                return Err(format!(
                    "user-defined primitive instance `{shown}` in `{path}` is not \
                     supported in v1"
                ))
            }
            PrimClass::Array => {
                return Err(format!(
                    "primitive array `{shown}` in `{path}` (a range on a gate or \
                     UDP instance) is not supported in v1"
                ))
            }
        }
        if strength0 != 0 || strength1 != 0 {
            return Err(format!(
                "drive-strength specification on gate `{shown}` in `{path}` is not \
                 supported in v1"
            ));
        }
        // Which builtin gate this is; everything outside the supported set
        // (switch/transistor prim types, sequential/combinational UDP types)
        // is rejected.  UDP instances never reach this point (their class was
        // rejected above); the prim-type reject covers unknown/other kinds.
        let op = match prim_type {
            vpi::vpiAndPrim => GateOp::Reduce(IrBinOp::BitAnd, false),
            vpi::vpiNandPrim => GateOp::Reduce(IrBinOp::BitAnd, true),
            vpi::vpiOrPrim => GateOp::Reduce(IrBinOp::BitOr, false),
            vpi::vpiNorPrim => GateOp::Reduce(IrBinOp::BitOr, true),
            vpi::vpiXorPrim => GateOp::Reduce(IrBinOp::BitXor, false),
            vpi::vpiXnorPrim => GateOp::Reduce(IrBinOp::BitXor, true),
            vpi::vpiBufPrim => GateOp::Copy,
            vpi::vpiNotPrim => GateOp::Not,
            vpi::vpiBufif1Prim => GateOp::Enable {
                invert_out: false,
                active_high: true,
            },
            vpi::vpiBufif0Prim => GateOp::Enable {
                invert_out: false,
                active_high: false,
            },
            vpi::vpiNotif1Prim => GateOp::Enable {
                invert_out: true,
                active_high: true,
            },
            vpi::vpiNotif0Prim => GateOp::Enable {
                invert_out: true,
                active_high: false,
            },
            vpi::vpiPullupPrim => GateOp::Pull(true),
            vpi::vpiPulldownPrim => GateOp::Pull(false),
            _ => {
                return Err(format!(
                    "primitive type {prim_type} of `{shown}` in `{path}` is not \
                     supported in v1"
                ))
            }
        };
        if terms.len() > LLG_MAX_GATE_TERMS {
            return Err(format!(
                "gate `{shown}` in `{path}` has {} terminals; at most \
                 {LLG_MAX_GATE_TERMS} are supported",
                terms.len()
            ));
        }
        // Resolve every terminal to its signal.  Terminals must be whole
        // plain signals (a ref to a net/var); select- or expression-
        // connected terminals are rejected cleanly in v1.
        let mut infos: Vec<SignalInfo> = Vec::new();
        for t in &terms {
            let whole = matches!(
                self.kind(t.expr),
                NodeKind::Net { .. } | NodeKind::Var { .. } | NodeKind::Expr(ExprKind::Ref { .. })
            );
            if !whole {
                return Err(format!(
                    "terminal `{}` of gate `{shown}` in `{path}` is connected \
                     through a select/expression; gate terminals must be whole \
                     plain signals in v1",
                    self.node(t.expr).name
                ));
            }
            let (_, info) = self.resolve_signal_id(path, t.expr).map_err(|_| {
                format!(
                    "terminal `{}` of gate `{shown}` in `{path}` does not resolve to \
                     a plain signal",
                    self.node(t.expr).name
                )
            })?;
            infos.push(info);
        }
        if let Some(bad) = infos.iter().position(|i| i.real) {
            return Err(format!(
                "real-valued signal `{}` on terminal {} of gate `{shown}` in \
                 `{path}` is not supported",
                infos[bad].global, bad
            ));
        }
        let w = infos.first().map(|i| i.width).unwrap_or(0);
        if w == 0 {
            return Err(format!(
                "gate `{shown}` in `{path}` has a zero-width terminal"
            ));
        }
        if infos.iter().any(|i| i.width != w) {
            let widths = infos
                .iter()
                .map(|i| i.width.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(format!(
                "gate `{shown}` in `{path}` connects terminals of different widths \
                 ({widths}); v1 requires equal terminal widths (mixed widths are \
                 legal Verilog, but not supported here yet)"
            ));
        }
        // Terminal-count/direction validation per kind.  Directions come from
        // Surelog's per-term classification (terminal 0 = output, except
        // buf/not where all but the last are outputs).
        let out_positions: Vec<usize> = terms
            .iter()
            .zip(infos.iter())
            .enumerate()
            .filter(|(_, (t, _))| t.direction == vpi::vpiOutput)
            .map(|(i, _)| i)
            .collect();
        let shape_ok = match op {
            GateOp::Pull(_) => terms.len() == 1 && out_positions.len() == 1,
            GateOp::Copy | GateOp::Not => {
                // Multi-output buf/not forms are not supported in v1.
                terms.len() == 2 && out_positions.len() == 1
            }
            GateOp::Enable { .. } => terms.len() == 3 && out_positions.len() == 1,
            GateOp::Reduce(..) => terms.len() >= 2 && out_positions.len() == 1,
        };
        if !shape_ok {
            let what = match op {
                GateOp::Pull(_) => "pullup/pulldown instance takes exactly one output terminal",
                GateOp::Copy | GateOp::Not => {
                    "one-output `buf`/`not` gates take exactly one output and one \
                     input terminal (multi-output forms are not supported in v1)"
                }
                GateOp::Enable { .. } => {
                    "enable gates take exactly one output, one data input and one \
                     enable input terminal"
                }
                GateOp::Reduce(..) => {
                    "logic gates take exactly one output terminal plus at least one \
                     input terminal"
                }
            };
            return Err(format!(
                "gate `{shown}` in `{path}`: {what} (found {} output(s) among {} \
                 terminals)",
                out_positions.len(),
                terms.len()
            ));
        }
        let out_pos = out_positions[0];
        let in_positions: Vec<usize> = (0..terms.len()).filter(|i| *i != out_pos).collect();
        // Callee resolution in the LHS/inputs needs the owning instance.
        self.inst = inst;
        // The output write goes through the same LHS machinery as a
        // continuous assignment (collapsed-net members lower to their driver
        // slot); select/hierarchical outputs stay unsupported in v1.
        let out_ir = match self.lower_lhs(path, terms[out_pos].expr)? {
            IrLhs::Whole(idx) => idx,
            _ => {
                return Err(format!(
                    "output terminal of gate `{shown}` in `{path}` must be connected \
                     to a whole plain signal (select/hierarchical gate outputs are \
                     not supported in v1)"
                ))
            }
        };
        // Input expressions + sensitivity set (input base signals only — the
        // LHS never triggers its own comb process).
        let mut in_exprs: Vec<IrExpr> = Vec::new();
        let mut sens: Vec<String> = Vec::new();
        for i in &in_positions {
            let e = self.lower_expr(path, terms[*i].expr)?;
            in_exprs.push(e);
            let global = &infos[*i].global;
            if !sens.contains(global) {
                sens.push(global.clone());
            }
        }
        // Output value computation (vector-wise over the common width).
        let value = match op {
            GateOp::Pull(ones) => const_bits_expr(w, ones),
            GateOp::Copy => in_exprs.remove(0),
            GateOp::Not => bitneg_full_width(in_exprs.remove(0)),
            GateOp::Reduce(bop, neg) => {
                let mut acc = in_exprs.remove(0);
                for next in in_exprs.drain(..) {
                    acc = bin_expr(bop, acc, next);
                }
                if neg {
                    bitneg_full_width(acc)
                } else {
                    acc
                }
            }
            GateOp::Enable {
                invert_out,
                active_high,
            } => {
                let data = in_exprs.remove(0);
                let en = in_exprs.remove(0);
                // LRM 1364-1995 §7.4 Table 7-5: an ENABLED enable gate acts
                // like `buf`/`not` (Tables 7-3/7-4), so a data Z must reach
                // the output as X while known bits pass unchanged.  The mux
                // is an identity/copy context that returns its chosen arm
                // verbatim (`sv4_mux` with a known select), so the Z→X
                // normalization is done explicitly with `data|data`.
                // Correctness proof from llg_rt.c `sv4_bitwise` (op = OR),
                // both operands identical so every bit takes one rule:
                // known 0 → `!ax && !ab && !bx && !bb` → 0; known 1 →
                // `!ax && ab` → 1; a Z bit reads as `sv4_lsb_bit() == 3`,
                // i.e. unknown, and falls to the `else o_x = 1` arm → X
                // (Z behaves as X in expression ops, LRM 11.4.5); X likewise
                // stays X.  Same operand ⇒ same width/signedness, resize is
                // a no-op; opt's identity pass has no `a|a → a` rule, so the
                // normalization survives optimization.
                let data = bin_expr(IrBinOp::BitOr, data.clone(), data);
                let data = if invert_out {
                    bitneg_full_width(data)
                } else {
                    data
                };
                let z = const_z_expr(w);
                let (a, b) = if active_high { (data, z) } else { (z, data) };
                IrExpr::new(
                    IrExprKind::Mux {
                        sel: Box::new(en),
                        a: Box::new(a),
                        b: Box::new(b),
                    },
                    w,
                    false,
                    None,
                )
            }
        };
        // Gate delay `#D`: folded through the parameter values like a
        // continuous-assignment delay and scaled to design-precision ticks.
        let scaled_delay = match delay {
            Some(de) => {
                let dir = self.lower_expr(path, de)?;
                if dir.is_real() {
                    return Err(format!(
                        "real-valued gate delays are not supported in `{path}`"
                    ));
                }
                let raw = match dir.kind {
                    IrExprKind::Const(c) => const_delay_ticks(&c, path)?,
                    _ => {
                        return Err(format!(
                            "gate delay must be a constant or parameter in `{path}`"
                        ))
                    }
                };
                let unit_ps = self.timescale_of_node(g).unit_ps;
                Some(scale_delay_ticks(
                    raw,
                    unit_ps,
                    self.design_precision_ps,
                    path,
                )?)
            }
            None => None,
        };
        let mut body = Vec::new();
        if let Some(ticks) = scaled_delay {
            self.warnings.push(format!(
                "delayed gate `{shown}` in `{path}` uses the current input values \
                 after the #delay window (no pulse filtering)"
            ));
            body.push(IrStmt::Delay { ticks });
        }
        body.push(IrStmt::Assign {
            lhs: IrLhs::Whole(out_ir),
            rhs: value,
            nba: false,
        });
        let shape = if sens.is_empty() {
            // Constant driver (pullup/pulldown): evaluate once at t=0.
            IrShape::RunOnce
        } else {
            IrShape::SensLoop { reads: sens }
        };
        let fn_name = self.new_fn_name(path, "gate");
        self.model.processes.push(IrProcess {
            c_name: fn_name,
            label: format!("{path}.{shown}"),
            shape,
            pre_fns: Vec::new(),
            body,
        });
        Ok(())
    }

    // ── Port links ─────────────────────────────────────────────────────────

    fn emit_links(&mut self, parent_path: &str, child_inst: NodeId) -> Result<(), String> {
        let child_path = self.instance_path_of(child_inst);
        for c in &self.node(child_inst).children {
            let port = *c;
            let (direction, high, low) = match self.kind(port) {
                NodeKind::Port {
                    direction,
                    high,
                    low,
                    ..
                } => (*direction, *high, *low),
                _ => continue,
            };
            // Interface ports are wired through dedicated link processes; the
            // plain signal link machinery does not apply.
            if let Some((actual, modport)) =
                self.node(port)
                    .children
                    .iter()
                    .find_map(|cc| match self.kind(*cc) {
                        NodeKind::IfaceConn { actual, modport } => Some((*actual, modport.clone())),
                        _ => None,
                    })
            {
                self.emit_iface_link(parent_path, port, actual, &modport, &child_path)?;
                continue;
            }
            if direction == crate::core::model::Direction::Inout {
                // The collapsed net group IS the connection; no link is
                // emitted.  Groups that could not be formed were already
                // warned about by `build_net_groups`.
                continue;
            }
            // Top-level ports have no parent side; nothing to link.
            let Some(hc) = high else { continue };
            let Some(lc) = low else { continue };
            let parent_side = self.link_parent_side(parent_path, port, hc)?;
            let (child_name, child_info) = self.resolve_signal_id(&child_path, lc)?;
            // A link touching a collapsed-net member would copy through the
            // resolution cell (or write it directly); the group itself is the
            // connection, so such links are skipped with a warning.
            let parent_member = match &parent_side {
                LinkSide::Signal(info) => info.net_driver.is_some(),
                LinkSide::ArrayElem(..) => false,
            };
            if parent_member || child_info.net_driver.is_some() {
                self.warnings.push(format!(
                    "port `{}` of `{child_path}` links a collapsed inout-net \
                     member; link skipped (the net group resolves the \
                     connection)",
                    self.node(port).name
                ));
                continue;
            }
            // The source (whose changes re-copy) and its width/signedness.
            let is_input = direction == crate::core::model::Direction::Input;
            let (write, wait_sig) = if is_input {
                let (src_expr, wait_sig) = match &parent_side {
                    LinkSide::Signal(pinfo) => (sig_read_expr_full(pinfo), pinfo.global.clone()),
                    LinkSide::ArrayElem(ai, sel) => {
                        let e = self.lower_expr(parent_path, *sel)?;
                        (e, self.array_elem_addr(ai, *sel)?)
                    }
                };
                let write = IrStmt::Assign {
                    lhs: IrLhs::Whole(child_info.ir),
                    rhs: src_expr,
                    nba: false,
                };
                (write, wait_sig)
            } else {
                let src_expr = sig_read_expr_full(&child_info);
                let write = match &parent_side {
                    LinkSide::Signal(pinfo) => IrStmt::Assign {
                        lhs: IrLhs::Whole(pinfo.ir),
                        rhs: src_expr,
                        nba: false,
                    },
                    LinkSide::ArrayElem(ai, sel) => {
                        let lh = self.lower_lhs(parent_path, *sel)?;
                        IrStmt::Assign {
                            lhs: lh,
                            rhs: IrExpr::new(
                                IrExprKind::Verbatim {
                                    code: child_name.clone(),
                                    width: ai.elem_width,
                                    signed: ai.signed,
                                },
                                ai.elem_width,
                                ai.signed,
                                None,
                            ),
                            nba: false,
                        }
                    }
                };
                (write, child_name.clone())
            };
            let fn_name = self.new_fn_name(parent_path, "link");
            self.model.processes.push(IrProcess {
                c_name: fn_name,
                label: format!("{child_path}.link"),
                shape: IrShape::SensLoop {
                    reads: vec![wait_sig],
                },
                pre_fns: Vec::new(),
                body: vec![write],
            });
        }
        Ok(())
    }

    /// Resolve the parent side of a port connection (the `vpiHighConn`): a
    /// plain global signal, or an element of an unpacked array (when the
    /// connection selects into one — e.g. `.cnt(cnts[i])`, whose index
    /// expression the db walk captured as a child of the port).
    fn link_parent_side(
        &self,
        parent_path: &str,
        port: NodeId,
        hc: NodeId,
    ) -> Result<LinkSide, String> {
        if let Some(ai) = self.array_of(hc).cloned() {
            let sel = self.node(port).children.iter().find_map(|c| {
                matches!(
                    self.kind(*c),
                    NodeKind::Expr(
                        ExprKind::BitSelect { .. }
                            | ExprKind::PartSelect { .. }
                            | ExprKind::IndexedPartSelect { .. }
                            | ExprKind::ArraySelect { .. }
                    )
                )
                .then_some(*c)
            });
            return match sel {
                Some(s) => Ok(LinkSide::ArrayElem(ai, s)),
                None => Err(format!(
                    "array-element port connection in `{parent_path}` is missing its \
                     index expression"
                )),
            };
        }
        let (_, info) = self.resolve_signal_id(parent_path, hc)?;
        Ok(LinkSide::Signal(info))
    }

    /// C address (without the leading `&`) of the array element addressed by a
    /// constant-index select expression (`cnts[i]` → `G_tb_cnts[(0)]`), used
    /// as the wait source of an input-port link into an array element.
    /// Dynamic (non-constant) array-element port connections are not
    /// supported.
    fn array_elem_addr(&self, ai: &ArrayInfo, sel: NodeId) -> Result<String, String> {
        let idx = match self.kind(sel) {
            NodeKind::Expr(ExprKind::BitSelect { index, .. }) => self.eval_bound_i128(*index)?,
            NodeKind::Expr(ExprKind::IndexedPartSelect { base_expr, .. }) => {
                self.eval_bound_i128(*base_expr)?
            }
            NodeKind::Expr(ExprKind::PartSelect { left, .. }) => self.eval_bound_i128(*left)?,
            NodeKind::Expr(ExprKind::ArraySelect { indices, .. }) if indices.len() == 1 => {
                self.eval_bound_i128(indices[0])?
            }
            _ => {
                return Err(
                    "array-element port connection with a non-constant index is not \
                     supported"
                        .to_string(),
                )
            }
        };
        if ai.dims.len() != 1 {
            return Err(format!(
                "array-element port connection on a {}-dimensional array is not supported",
                ai.dims.len()
            ));
        }
        let (l, r) = ai.dims[0];
        let off = if l >= r {
            l as i128 - idx
        } else {
            idx - l as i128
        };
        Ok(format!("{}[({off})]", ai.global))
    }

    /// Emit the link processes wiring an interface port to its actual
    /// interface instance.  Values are copied between the per-port copy's
    /// vars and the actual interface's vars, matched by name:
    ///
    /// - modport ports: each io_decl is wired in its declared direction
    ///   (outputs flow child → actual, inputs flow actual → child);
    /// - bare interface ports: every member is wired bidirectionally (the
    ///   pair converges because same-value writes do not re-fire the wait).
    ///
    /// Every link is one process (initial copy at spawn, then `wait_any` on
    /// the source followed by a re-copy), mirroring the plain port links.
    fn emit_iface_link(
        &mut self,
        parent_path: &str,
        port: NodeId,
        actual_id: NodeId,
        modport: &str,
        child_path: &str,
    ) -> Result<(), String> {
        // The per-port copy inside the child: `low` resolves to the copy's
        // modport (modport ports) or to the copy interface instance itself
        // (bare interface ports).
        let low = match self.kind(port) {
            NodeKind::Port { low, .. } => *low,
            _ => return Ok(()),
        };
        let copy_id = match low.and_then(|l| match self.kind(l) {
            NodeKind::ModPort => self.node(l).parent,
            NodeKind::ModuleInst { .. } => Some(l),
            _ => None,
        }) {
            Some(c) => c,
            None => {
                self.warnings.push(format!(
                    "interface port `{}` of `{child_path}`: per-port copy not \
                     found; skipped",
                    self.node(port).name
                ));
                return Ok(());
            }
        };
        let actual_vars = self.collect_iface_vars(actual_id);
        let copy_vars = self.collect_iface_vars(copy_id);
        if actual_vars.is_empty() || copy_vars.is_empty() {
            self.warnings.push(format!(
                "interface port `{}` of `{child_path}`: no interface members \
                 found on the actual instance or the per-port copy; skipped",
                self.node(port).name
            ));
            return Ok(());
        }

        // (source global, source info, destination global)
        let mut links: Vec<(String, SignalInfo, String)> = Vec::new();
        if modport.is_empty() {
            // Bare interface port: bidirectional pair per member.
            for (name, cv) in &copy_vars {
                if let Some(av) = actual_vars.get(name) {
                    links.push((cv.global.clone(), cv.clone(), av.global.clone()));
                    links.push((av.global.clone(), av.clone(), cv.global.clone()));
                }
            }
        } else {
            let mp_node = self.node(copy_id).children.iter().find(|cc| {
                matches!(self.kind(**cc), NodeKind::ModPort) && self.node(**cc).name == modport
            });
            match mp_node {
                Some(mp) => {
                    for io in &self.node(*mp).children {
                        let (direction, expr) = match self.kind(*io) {
                            NodeKind::IoDecl { direction, expr } => (*direction, *expr),
                            _ => continue,
                        };
                        // The io_decl's expr resolves to the copy's own var.
                        let Some(cv) = expr.and_then(|e| self.signal_of(e)) else {
                            continue;
                        };
                        let name = self.node(*io).name.clone();
                        let Some(av) = actual_vars.get(&name) else {
                            continue;
                        };
                        match direction {
                            // Output: the child drives the actual member.
                            crate::core::model::Direction::Output => {
                                links.push((cv.global.clone(), cv.clone(), av.global.clone()));
                            }
                            // Input: the child reads the actual member.
                            crate::core::model::Direction::Input => {
                                links.push((av.global.clone(), av.clone(), cv.global.clone()));
                            }
                            _ => {}
                        }
                    }
                }
                None => {
                    self.warnings.push(format!(
                        "modport `{modport}` not found on the per-port copy of \
                         `{child_path}`; interface link skipped"
                    ));
                    return Ok(());
                }
            }
        }

        for (src, src_info, dst) in links {
            let dst_real = self
                .signals
                .iter()
                .any(|info| info.global == dst && info.real);
            if src_info.real || dst_real {
                return Err(format!(
                    "interface links involving real-valued member `{child_path}` are not supported"
                ));
            }
            let dst_ir = self
                .signals
                .iter()
                .find(|info| info.global == dst)
                .map(|info| info.ir)
                .ok_or_else(|| format!("interface link destination `{dst}` not collected"))?;
            let fn_name = self.new_fn_name(parent_path, "ilink");
            let write = IrStmt::Assign {
                lhs: IrLhs::Whole(dst_ir),
                rhs: sig_read_expr_full(&src_info),
                nba: false,
            };
            self.model.processes.push(IrProcess {
                c_name: fn_name,
                label: format!("{child_path}.ilink"),
                shape: IrShape::SensLoop { reads: vec![src] },
                pre_fns: Vec::new(),
                body: vec![write],
            });
            // Writing an actual member from a child drives it; several
            // children driving the same member is last-writer-wins.
            if !self.iface_driven.insert(dst.clone()) {
                self.warnings
                    .push(format!("multiple interface drivers on `{dst}`"));
            }
        }
        Ok(())
    }

    /// Name → lowered signal info for every Net/Var child of an interface
    /// instance node (the actual instance or a per-port copy).
    fn collect_iface_vars(&self, inst: NodeId) -> HashMap<String, SignalInfo> {
        let mut out = HashMap::new();
        for c in &self.node(inst).children {
            if matches!(self.kind(*c), NodeKind::Net { .. } | NodeKind::Var { .. }) {
                if let Some(info) = self.signal_of(*c) {
                    out.insert(self.node(*c).name.clone(), info.clone());
                }
            }
        }
        out
    }

    // ── Processes ──────────────────────────────────────────────────────────

    fn emit_process(&mut self, inst: NodeId, path: &str, proc: NodeId) -> Result<(), String> {
        let (kind, stmt) = match self.kind(proc) {
            NodeKind::Process { kind } => {
                let stmt = self
                    .node(proc)
                    .children
                    .first()
                    .copied()
                    .ok_or_else(|| format!("process without statement in `{path}`"))?;
                (kind, stmt)
            }
            _ => unreachable!("non-process passed to emit_process"),
        };
        let is_initial = matches!(kind, ProcessKind::Initial);
        let is_final = matches!(kind, ProcessKind::Final);
        let fn_name = self.new_fn_name(path, "proc");
        let (body_stmts, pre_fns, shape) = {
            let mut ctx = EmitCtx::new(self, path.to_string(), inst, "0", None, None, is_final);
            let body_stmts = ctx.lower_stmt(stmt)?;
            // Fork-branch coroutines and monitor/strobe evaluators attach to
            // the process (rendered ahead of it).
            let pre_fns = std::mem::take(&mut ctx.pre_fns);
            let kind_label = if is_initial { "initial" } else { "always" };
            let shape = if is_initial || is_final {
                // `initial` and `final` bodies run exactly once (finals after
                // the scheduler exits — the spawn phase is decided below).
                IrShape::RunOnce
            } else if !ctx.saw_wait {
                // always / always_comb / always_ff without any event/delay
                // control: a combinational process.
                if assigns_to_real(&body_stmts, &ctx.cg.model) {
                    return Err(format!("real-valued signals are not supported in combinational processes in `{path}`"));
                }
                // Run once at t=0, then re-run whenever a read signal changes.
                let sigs = ctx.cg.collect_read_signals(path, stmt)?;
                if sigs.is_empty() {
                    ctx.cg.warnings.push(format!(
                        "combinational always process in `{path}` reads no \
                         signals; evaluating once at time 0"
                    ));
                    IrShape::RunOnce
                } else {
                    IrShape::SensLoop { reads: sigs }
                }
            } else {
                IrShape::Loop
            };
            let _ = kind_label;
            (body_stmts, pre_fns, shape)
        };
        let kind_label = if is_initial {
            "initial"
        } else if is_final {
            "final"
        } else {
            "always"
        };
        self.model.processes.push(IrProcess {
            c_name: fn_name.clone(),
            label: format!("{path}.{kind_label}"),
            shape,
            pre_fns,
            body: body_stmts,
        });
        if is_final {
            self.final_procs.push(fn_name);
        }
        Ok(())
    }

    // ── Signal reads for sensitivity ───────────────────────────────────────

    /// Collect every signal read anywhere in the statement/expression tree
    /// rooted at `root`, as global names (deduped, deterministic order).
    /// Function/task calls descend into the callee bodies (guarded against
    /// recursion), so reads hidden behind a function call contribute to the
    /// sensitivity set.
    fn collect_read_signals(&self, scope_path: &str, root: NodeId) -> Result<Vec<String>, String> {
        let mut out = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        let mut visited: HashSet<NodeId> = HashSet::new();
        self.walk_read_signals(scope_path, root, &mut seen, &mut visited, &mut out)?;
        if out.iter().any(|name| {
            self.signals
                .iter()
                .any(|info| info.real && info.global == name.as_str())
        }) {
            return Err(format!("real-valued signals cannot be used in a combinational sensitivity set in `{scope_path}`"));
        }
        Ok(out)
    }

    fn walk_read_signals(
        &self,
        scope_path: &str,
        node: NodeId,
        seen: &mut HashSet<String>,
        visited: &mut HashSet<NodeId>,
        out: &mut Vec<String>,
    ) -> Result<(), String> {
        match self.kind(node) {
            NodeKind::Stmt(StmtKind::Assign { .. })
            | NodeKind::Stmt(StmtKind::ProcContAssign { .. }) => {
                // Sensitivity of a process body: an assignment's LHS base
                // signal must NOT trigger the process (it would self-wake
                // after every write — including the dedicated PCA guard
                // process's writes).  Only the LHS's index/bounds
                // expressions are reads.
                if let Some(rhs) = self.node(node).children.get(1) {
                    self.walk_read_signals(scope_path, *rhs, seen, visited, out)?;
                }
                if let Some(lhs) = self.node(node).children.first() {
                    self.walk_lhs_select_reads(scope_path, *lhs, seen, visited, out)?;
                }
                return Ok(());
            }
            NodeKind::Stmt(StmtKind::For { cond, body, .. }) => {
                // The old RELS-based walk does not descend into the for
                // init/incr statements.
                self.walk_read_signals(scope_path, *cond, seen, visited, out)?;
                self.walk_read_signals(scope_path, *body, seen, visited, out)?;
                return Ok(());
            }
            NodeKind::Stmt(StmtKind::Fork { branches, .. }) => {
                // A fork body reads signals (a `fork … join` branch may block
                // on signals); descend into the branches for comb sensitivity.
                for b in branches {
                    self.walk_read_signals(scope_path, *b, seen, visited, out)?;
                }
                return Ok(());
            }
            NodeKind::Stmt(StmtKind::WaitFork | StmtKind::DisableFork) => {
                // Neither reads signals: they touch the fork machinery only.
                return Ok(());
            }
            // A call's arguments are walked below; the callee body's reads
            // (assignments to module signals, reads of them) are part of the
            // calling process's sensitivity too.
            NodeKind::FuncCall {
                name,
                is_task,
                callee,
            } => {
                if let Ok(ft) = self.resolve_callee(self.inst, name, *is_task, *callee) {
                    if visited.insert(ft) {
                        if let Some(body) = self.func_body(ft) {
                            self.walk_read_signals(scope_path, body, seen, visited, out)?;
                        }
                    }
                }
                for c in &self.node(node).children {
                    self.walk_read_signals(scope_path, *c, seen, visited, out)?;
                }
                return Ok(());
            }
            _ => {}
        }
        self.add_node_read(node, seen, out);
        for c in &self.node(node).children {
            self.walk_read_signals(scope_path, *c, seen, visited, out)?;
        }
        Ok(())
    }

    /// Walk only the index/bounds expressions of an assignment LHS.
    fn walk_lhs_select_reads(
        &self,
        scope_path: &str,
        lhs: NodeId,
        seen: &mut HashSet<String>,
        visited: &mut HashSet<NodeId>,
        out: &mut Vec<String>,
    ) -> Result<(), String> {
        match self.kind(lhs) {
            NodeKind::Expr(ExprKind::BitSelect { index, .. }) => {
                self.walk_read_signals(scope_path, *index, seen, visited, out)
            }
            NodeKind::Expr(ExprKind::PartSelect { left, right, .. }) => {
                self.walk_read_signals(scope_path, *left, seen, visited, out)?;
                self.walk_read_signals(scope_path, *right, seen, visited, out)
            }
            NodeKind::Expr(ExprKind::IndexedPartSelect {
                base_expr,
                width_expr,
                ..
            }) => {
                self.walk_read_signals(scope_path, *base_expr, seen, visited, out)?;
                self.walk_read_signals(scope_path, *width_expr, seen, visited, out)
            }
            NodeKind::Expr(ExprKind::ArraySelect { indices, .. }) => {
                // The base is the array itself (not a read); only the index
                // expressions (and any element-level select bounds) are reads.
                for i in indices {
                    self.walk_read_signals(scope_path, *i, seen, visited, out)?;
                }
                Ok(())
            }
            NodeKind::Expr(ExprKind::HierPath { .. }) => {
                // A hierarchical LHS base signal must not trigger the owning
                // process (same rule as a plain LHS ref).  v1 supports only
                // constant indices/bounds on hierarchical targets, so there
                // are no index/bounds reads to collect.
                Ok(())
            }
            _ => Ok(()), // plain ref LHS: not part of the read set
        }
    }

    /// Add `node` to the read set if it is (or resolves to) a signal.
    fn add_node_read(&self, node: NodeId, seen: &mut HashSet<String>, out: &mut Vec<String>) {
        match self.kind(node) {
            NodeKind::Net { .. } | NodeKind::Var { .. } => {
                if let Some(info) = self.signal_of(node) {
                    if seen.insert(info.global.clone()) {
                        out.push(info.global.clone());
                    }
                }
            }
            NodeKind::Expr(ExprKind::Ref { target: Some(t) }) => {
                if let Some(info) = self.signal_of(*t) {
                    if seen.insert(info.global.clone()) {
                        out.push(info.global.clone());
                    }
                }
            }
            NodeKind::Expr(ExprKind::HierPath { .. }) => {
                if let Some(info) = self.hier_path_signal(node) {
                    if seen.insert(info.global.clone()) {
                        out.push(info.global.clone());
                    }
                }
            }
            _ => {}
        }
    }

    // ── Signal resolution ──────────────────────────────────────────────────

    /// The named-event arena node a walked operand resolves to, when it is a
    /// ref whose target is a captured [`NodeKind::NamedEvent`] (both the
    /// ref-wrapped and the direct object shapes normalize to `Ref`).
    fn event_target_of(&self, node: NodeId) -> Option<NodeId> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target: Some(t) }) => {
                matches!(self.kind(*t), NodeKind::NamedEvent).then_some(*t)
            }
            _ => None,
        }
    }

    /// Model index of a captured named-event node.
    fn event_index_of(&self, ev: NodeId, scope_path: &str) -> Result<usize, String> {
        self.event_globals.get(&ev).map(|i| i.ir).ok_or_else(|| {
            format!(
                "cannot resolve named event reference `{}` in `{scope_path}`",
                self.node(ev).name
            )
        })
    }

    /// Resolve a net/var/ref node to a global signal name (used by port
    /// links and event sensitivities).
    fn resolve_signal_id(
        &self,
        scope_path: &str,
        node: NodeId,
    ) -> Result<(String, SignalInfo), String> {
        let name = self.node(node).name.clone();
        match self.kind(node) {
            NodeKind::Net { .. } | NodeKind::Var { .. } => {
                if let Some(info) = self.signal_of(node) {
                    return Ok((info.global.clone(), info.clone()));
                }
            }
            NodeKind::Expr(ExprKind::Ref { target: Some(t) }) => {
                if let Some(info) = self.signal_of(*t) {
                    return Ok((info.global.clone(), info.clone()));
                }
            }
            NodeKind::Expr(ExprKind::HierPath { .. }) => {
                if let Some(info) = self.hier_path_signal(node) {
                    return Ok((info.global.clone(), info.clone()));
                }
            }
            _ => {}
        }
        if !name.is_empty() {
            if let Some(info) = self
                .scope_sig_names
                .get(scope_path)
                .and_then(|m| m.get(&name))
            {
                return Ok((info.global.clone(), info.clone()));
            }
        }
        Err(format!(
            "cannot resolve signal reference `{name}` in `{scope_path}`"
        ))
    }

    /// Resolve a select node's base arena node to its global signal.
    fn base_signal(&self, _scope_path: &str, base: NodeId) -> Result<(String, SignalInfo), String> {
        match self.kind(base) {
            NodeKind::Net { .. } | NodeKind::Var { .. } => {
                if let Some(info) = self.signal_of(base) {
                    return Ok((info.global.clone(), info.clone()));
                }
            }
            NodeKind::Expr(ExprKind::Ref { target: Some(t) }) => {
                if let Some(info) = self.signal_of(*t) {
                    return Ok((info.global.clone(), info.clone()));
                }
            }
            NodeKind::Expr(ExprKind::HierPath { .. }) => {
                if let Some(info) = self.hier_path_signal(base) {
                    return Ok((info.global.clone(), info.clone()));
                }
            }
            _ => {}
        }
        Err(format!(
            "cannot resolve base signal of select `{}`",
            self.node(base).name
        ))
    }

    /// Resolve a whole-signal assignment target (by arena node, then by name
    /// across every scope).
    fn resolve_lhs_target(
        &self,
        node: NodeId,
        target: Option<NodeId>,
    ) -> Result<(String, SignalInfo), String> {
        if let Some(t) = target {
            if let Some(info) = self.signal_of(t) {
                return Ok((info.global.clone(), info.clone()));
            }
        }
        let name = self.node(node).name.clone();
        for names in self.scope_sig_names.values() {
            if let Some(info) = names.get(&name) {
                return Ok((info.global.clone(), info.clone()));
            }
        }
        Err(format!("cannot resolve assignment target `{name}`"))
    }

    // ── LHS analysis ───────────────────────────────────────────────────────

    fn analyze_lhs(&mut self, path: &str, lhs: NodeId) -> Result<Lhs, String> {
        match self.kind(lhs) {
            NodeKind::Expr(ExprKind::Ref { target }) => {
                if let Some(t) = *target {
                    if let Some(info) = self.signal_of(t) {
                        return Ok(Lhs::Whole(info.clone()));
                    }
                    // Function/task body writes: output/inout formals, locals
                    // and the return variable (by arena node).
                    if let Some(lh) = self.func_write_target(t, "") {
                        return Ok(lh);
                    }
                }
                let name = self.node(lhs).name.clone();
                if !name.is_empty() {
                    // io_decls are not indexed, so formals resolve by name.
                    if let Some(lh) = self.func_write_target(NodeId(0), &name) {
                        return Ok(lh);
                    }
                }
                let (name, info) = self.resolve_lhs_target(lhs, *target)?;
                Ok(Lhs::Whole(SignalInfo {
                    global: name,
                    ..info
                }))
            }
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                if let Some(ai) = self.array_of(*base).cloned() {
                    if ai.dims.len() != 1 {
                        return Err(format!(
                            "array slice access (`{}[...]` on a {}-dimensional array) \
                             is not supported in `{path}`",
                            self.node(*base).name,
                            ai.dims.len()
                        ));
                    }
                    let ie = self.emit_expr(path, *index)?;
                    return Ok(Lhs::ArrayElem(ArrayElemLhs {
                        arr: ai,
                        index_codes: vec![ie],
                        elem_sel: ElemSel::Whole,
                    }));
                }
                let (_, info) = self.base_signal(path, *base)?;
                if info.real {
                    return Err(format!(
                        "select on real-valued signal in `{path}` is not supported"
                    ));
                }
                let ie = self.emit_expr(path, *index)?;
                Ok(Lhs::Bit(info, ie))
            }
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                let ai = self.array_of(*base).cloned().ok_or_else(|| {
                    format!(
                        "cannot resolve array base of select `{}` in `{path}`",
                        self.node(*base).name
                    )
                })?;
                let ndims = ai.dims.len();
                if indices.len() == ndims {
                    let ies = indices
                        .iter()
                        .map(|i| self.emit_expr(path, *i))
                        .collect::<Result<Vec<_>, _>>()?;
                    return Ok(Lhs::ArrayElem(ArrayElemLhs {
                        arr: ai,
                        index_codes: ies,
                        elem_sel: ElemSel::Whole,
                    }));
                }
                if indices.len() == ndims + 1 {
                    let last = *indices.last().expect("non-empty indices");
                    let ies = indices[..ndims]
                        .iter()
                        .map(|i| self.emit_expr(path, *i))
                        .collect::<Result<Vec<_>, _>>()?;
                    let elem_sel = match self.kind(last) {
                        NodeKind::Expr(ExprKind::PartSelect { left, right, .. }) => {
                            let l = self.eval_bound_i128(*left)?;
                            let r = self.eval_bound_i128(*right)?;
                            ElemSel::Part(l, r)
                        }
                        NodeKind::Expr(ExprKind::IndexedPartSelect { .. }) => {
                            return Err(format!(
                                "indexed part-select on an array element is not \
                                 supported in `{path}`"
                            ))
                        }
                        _ => {
                            let ie = self.emit_expr(path, last)?;
                            ElemSel::Bit(ie)
                        }
                    };
                    return Ok(Lhs::ArrayElem(ArrayElemLhs {
                        arr: ai,
                        index_codes: ies,
                        elem_sel,
                    }));
                }
                Err(format!(
                    "array `{}` in `{path}`: {}-level select on a {}-dimensional \
                     array is not supported",
                    self.node(*base).name,
                    indices.len(),
                    ndims
                ))
            }
            NodeKind::Expr(ExprKind::PartSelect { base, left, right }) => {
                let (_, info) = self.base_signal(path, *base)?;
                if info.real {
                    return Err(format!(
                        "select on real-valued signal in `{path}` is not supported"
                    ));
                }
                let (l, r) = (self.eval_bound_i128(*left)?, self.eval_bound_i128(*right)?);
                Ok(Lhs::Part(info, l, r))
            }
            NodeKind::Expr(ExprKind::IndexedPartSelect {
                base,
                base_expr,
                width_expr,
                neg,
            }) => {
                let (_, info) = self.base_signal(path, *base)?;
                if info.real {
                    return Err(format!(
                        "select on real-valued signal in `{path}` is not supported"
                    ));
                }
                let be = self.emit_expr(path, *base_expr)?;
                let we = self.emit_expr(path, *width_expr)?;
                let neg = if *neg { 1 } else { 0 };
                Ok(Lhs::IdxPart(info, be, we, neg))
            }
            NodeKind::Expr(ExprKind::HierPath { .. }) => {
                // A whole-signal hierarchical WRITE (`m.data`, `tb.dut.sig`,
                // …) lowers to the resolved target signal's global, so
                // `llg_ba`/`llg_nba` (and the collapsed inout-net driver
                // path in `assign_statement`) apply unchanged.  A trailing
                // select on the target is recovered from the node name /
                // source line — Surelog v1.86's elaborated model drops
                // part-select bounds and only keeps constant bit-select
                // indices (in the object name); constant indices/bounds only
                // in v1.
                let info = self.hier_path_signal(lhs).cloned().ok_or_else(|| {
                    format!(
                        "cannot resolve hierarchical assignment LHS `{}` in \
                             `{path}` (only plain per-instance signals are \
                             supported)",
                        self.node(lhs).name
                    )
                })?;
                if let Some(sel) = self.hier_lhs_select(lhs)? {
                    return Ok(match sel {
                        HierSelect::Bit(idx) => Lhs::Bit(info, format!("SV4_C({idx}, 32)")),
                        HierSelect::Part(left, right) => Lhs::Part(info, left, right),
                        HierSelect::IdxPart(base, width, neg) => Lhs::IdxPart(
                            info,
                            format!("SV4_C({base}, 32)"),
                            format!("SV4_C({width}, 32)"),
                            if neg { 1 } else { 0 },
                        ),
                    });
                }
                Ok(Lhs::Whole(info))
            }
            _ => Err("unsupported assignment LHS".to_string()),
        }
    }

    /// Recover a trailing select on a hierarchical assignment target.
    ///
    /// Surelog v1.86's elaborated UHDM is lossy here: constant bit-select
    /// indices survive in the object's VPI name (`u.dut.sig[2]`), but
    /// part-select bounds (`u.dut.sig[3:0]`) are dropped entirely, so they
    /// are read back from the source line the node points at (the same
    /// recovery pattern as `#delay` ticks).  Only plain integer-literal
    /// indices/bounds are supported in v1; anything else is rejected with a
    /// clear error.  Returns `Ok(None)` when the target is a whole signal.
    fn hier_lhs_select(&self, lhs: NodeId) -> Result<Option<HierSelect>, String> {
        let name = self.node(lhs).name.clone();
        // Bit-selects keep their constant index in the VPI name.
        if let Some(inner) = name
            .strip_suffix(']')
            .and_then(|rest| rest.rfind('[').map(|i| &rest[i + 1..]))
        {
            if !inner.contains('[') {
                return hier_select_from_text(inner, &name).map(Some);
            }
        }
        // Part-selects (and anything else) are recovered from the source line.
        let file = self.node(lhs).file.clone().unwrap_or_default();
        let line = self.node(lhs).line;
        if file.is_empty() || line == 0 {
            return Err(format!(
                "cannot recover the select of hierarchical assignment LHS \
                 `{name}` (no source location)"
            ));
        }
        let content = std::fs::read_to_string(&file).map_err(|e| {
            format!(
                "cannot read `{file}` to recover the select of hierarchical \
                 assignment LHS `{name}`: {e}"
            )
        })?;
        let text = content.lines().nth(line as usize - 1).ok_or_else(|| {
            format!(
                "cannot read line {line} of `{file}` to recover the select of \
                 hierarchical assignment LHS `{name}`"
            )
        })?;
        let NodeKind::Expr(ExprKind::HierPath { parts, .. }) = self.kind(lhs) else {
            return Ok(None);
        };
        let path_text = parts.join(".");
        let mut search_from = 0usize;
        while let Some(rel) = text[search_from..].find(&path_text) {
            let pos = search_from + rel;
            let after = text[pos + path_text.len()..].trim_start();
            if after.starts_with('[') {
                let close = after.find(']').ok_or_else(|| {
                    format!(
                        "unterminated select on hierarchical assignment LHS \
                         `{name}` at {file}:{line}"
                    )
                })?;
                let inner = &after[1..close];
                if inner.contains('[') || inner.contains(']') {
                    return Err(format!(
                        "nested select on hierarchical assignment LHS `{name}` \
                         is not supported in v1"
                    ));
                }
                return hier_select_from_text(inner, &name).map(Some);
            }
            if after.starts_with('=') || after.starts_with('<') {
                return Ok(None); // whole-signal target
            }
            search_from = pos + path_text.len();
        }
        Err(format!(
            "cannot locate hierarchical assignment LHS `{name}` in `{file}` \
             line {line}"
        ))
    }

    // ── Constant-ish bound evaluation ──────────────────────────────────────

    /// Evaluate a constant expression node (part-select bound) to an integer.
    fn eval_bound_i128(&self, node: NodeId) -> Result<i128, String> {
        match self.eval_bits(node) {
            Ok(v) if !v.is_unknown() => {
                if v.signed {
                    Ok(v.to_i64().unwrap() as i128)
                } else {
                    Ok(v.to_u64().unwrap() as i128)
                }
            }
            Ok(_) => Err("unknown part_select bound".to_string()),
            Err(e) => Err(format!("part_select bound: {e}")),
        }
    }

    /// Evaluate a constant-ish expression node to a 4-state value, mirroring
    /// `core::elab::Resolver::eval_expr` for the constructs that can appear in
    /// elaborated bound positions.
    fn eval_bits(&self, node: NodeId) -> Result<elab::Value, String> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Constant { value, size, .. }) => {
                let mut value = val_from_value_data(value, *size)?;
                if self.signed_based_constant(node) {
                    if let Val::Bits(bits) = &mut value {
                        if let Some(width) = self
                            .signed_based_literal_info(node)
                            .1
                            .map(|width| width as usize)
                        {
                            if width < bits.width() {
                                *bits = bits.resize(width, true);
                            }
                        }
                        bits.signed = true;
                    }
                }
                match value {
                    Val::Bits(b) => Ok(b),
                    Val::Str(_) | Val::Real(_) => Err("non-integer constant in bound".to_string()),
                }
            }
            NodeKind::EnumConst { value } => match value {
                Some(Val::Bits(b)) => Ok(b.clone()),
                _ => Err("enum constant without value in bound".to_string()),
            },
            NodeKind::Expr(ExprKind::Ref { target }) => {
                match target.and_then(|t| self.param_vals.get(&t)) {
                    Some(Val::Bits(b)) => Ok(b.clone()),
                    Some(Val::Str(_)) | Some(Val::Real(_)) => {
                        Err("non-integer parameter in bound".to_string())
                    }
                    None => Err("unresolved reference in bound".to_string()),
                }
            }
            NodeKind::Expr(ExprKind::Operation {
                op,
                reordered,
                operands,
            }) => self.eval_operation_bits(*op, *reordered, operands),
            other => Err(format!("unsupported bound expression: {other:?}")),
        }
    }

    fn eval_operation_bits(
        &self,
        op: i32,
        reordered: bool,
        operands: &[NodeId],
    ) -> Result<elab::Value, String> {
        use vpi::*;
        let u = |i: usize| self.eval_bits(operands[i]);
        macro_rules! b {
            ($i:expr) => {
                u($i)?
            };
        }
        match op {
            vpiMinusOp => Ok(elab::minus(&b!(0))),
            vpiPlusOp => Ok(b!(0)),
            vpiNotOp => Ok(elab::log_not(&b!(0))),
            vpiBitNegOp => Ok(elab::bit_neg(&b!(0))),
            vpiUnaryAndOp => Ok(elab::unary_and(&b!(0))),
            vpiUnaryNandOp => Ok(elab::unary_nand(&b!(0))),
            vpiUnaryOrOp => Ok(elab::unary_or(&b!(0))),
            vpiUnaryNorOp => Ok(elab::unary_nor(&b!(0))),
            vpiUnaryXorOp => Ok(elab::unary_xor(&b!(0))),
            vpiUnaryXNorOp => Ok(elab::unary_xnor(&b!(0))),
            vpiSubOp => Ok(elab::sub(&b!(0), &b!(1))),
            vpiDivOp => Ok(elab::div(&b!(0), &b!(1))),
            vpiModOp => Ok(elab::rem(&b!(0), &b!(1))),
            vpiEqOp => Ok(elab::eq(&b!(0), &b!(1))),
            vpiNeqOp => Ok(elab::neq(&b!(0), &b!(1))),
            vpiCaseEqOp => Ok(elab::case_eq(&b!(0), &b!(1))),
            vpiCaseNeqOp => Ok(elab::case_neq(&b!(0), &b!(1))),
            vpiGtOp => Ok(elab::gt(&b!(0), &b!(1))),
            vpiGeOp => Ok(elab::ge(&b!(0), &b!(1))),
            vpiLtOp => Ok(elab::lt(&b!(0), &b!(1))),
            vpiLeOp => Ok(elab::le(&b!(0), &b!(1))),
            vpiLShiftOp => Ok(elab::shl(&b!(0), &b!(1))),
            vpiRShiftOp => Ok(elab::shr(&b!(0), &b!(1))),
            vpiArithLShiftOp => Ok(elab::arith_shl(&b!(0), &b!(1))),
            vpiArithRShiftOp => Ok(elab::arith_shr(&b!(0), &b!(1))),
            vpiAddOp => Ok(elab::add(&b!(0), &b!(1))),
            vpiMultOp => Ok(elab::mul(&b!(0), &b!(1))),
            vpiPowerOp => Ok(elab::power(&b!(0), &b!(1))),
            vpiLogAndOp => Ok(elab::log_and(&b!(0), &b!(1))),
            vpiLogOrOp => Ok(elab::log_or(&b!(0), &b!(1))),
            vpiBitAndOp => Ok(elab::bit_and(&b!(0), &b!(1))),
            vpiBitOrOp => Ok(elab::bit_or(&b!(0), &b!(1))),
            vpiBitXorOp => Ok(elab::bit_xor(&b!(0), &b!(1))),
            vpiBitXNorOp => Ok(elab::bit_xnor(&b!(0), &b!(1))),
            vpiConditionOp => Ok(elab::cond(&b!(0), &b!(1), &b!(2))),
            vpiMinTypMaxOp => Ok(b!(0)),
            vpiConcatOp => {
                let mut parts = Vec::with_capacity(operands.len());
                for i in 0..operands.len() {
                    parts.push(b!(i));
                }
                if reordered {
                    parts.reverse();
                }
                Ok(elab::concat(&parts))
            }
            vpiMultiConcatOp => {
                let count = b!(0);
                if count.is_unknown() {
                    return Err("unknown replication count".to_string());
                }
                let n = count.to_u64().unwrap();
                let mut parts = Vec::with_capacity(operands.len().saturating_sub(1));
                for i in 1..operands.len() {
                    parts.push(b!(i));
                }
                let pat = elab::concat(&parts);
                let mut bits = Vec::new();
                for _ in 0..n {
                    bits.extend(pat.bits.iter().cloned());
                }
                Ok(elab::Value::from_bits(bits, false))
            }
            other => Err(format!("unsupported operation op type {other} in bound")),
        }
    }
}

/// Convert a captured constant (`ValueData` + `vpiSize`) to a `Val`, mirroring
/// `elab::read_value` without a VPI handle.
fn val_from_value_data(vd: &ValueData, size: i32) -> Result<Val, String> {
    match vd {
        ValueData::Bin(s) => Ok(Val::Bits(parse_radix_val(s, 1, size))),
        ValueData::Oct(s) => Ok(Val::Bits(parse_radix_val(s, 3, size))),
        ValueData::Hex(s) => Ok(Val::Bits(parse_radix_val(s, 4, size))),
        ValueData::Dec(s) => {
            let val = s.trim().parse::<u64>().unwrap_or(0);
            let width = if size > 0 { size as usize } else { 64 };
            Ok(Val::Bits(elab::Value::from_u64(val, width, false)))
        }
        ValueData::Scalar(sc) => {
            let b = match *sc {
                vpi::vpi0 => Bit::Zero,
                vpi::vpi1 | vpi::vpiH => Bit::One,
                vpi::vpiZ => Bit::Z,
                vpi::vpiL => Bit::Zero,
                _ => Bit::X,
            };
            let fill = if size == -1 { Some(b) } else { None };
            Ok(Val::Bits(elab::Value {
                bits: vec![b],
                signed: false,
                fill,
            }))
        }
        ValueData::Int(val) => {
            let width = if size > 0 { size as usize } else { 32 };
            Ok(Val::Bits(elab::Value::from_u64(*val as u64, width, true)))
        }
        ValueData::UInt(val) => {
            let width = if size > 0 { size as usize } else { 64 };
            Ok(Val::Bits(elab::Value::from_u64(*val, width, false)))
        }
        ValueData::Str(s) => Ok(Val::Str(s.clone())),
        ValueData::Real(_) => Err("real value in bound".to_string()),
        ValueData::None => Err("object has no stored value".to_string()),
        ValueData::Vector(_) => Err("vector value format in bound".to_string()),
    }
}

/// Parse a BIN/OCT/HEX digit string into bits (see `elab::parse_radix`).
fn parse_radix_val(s: &str, base_bits: usize, size: i32) -> elab::Value {
    let mut bits = Vec::new();
    for ch in s.chars() {
        let c = ch.to_ascii_lowercase();
        match c {
            'x' => bits.extend(std::iter::repeat_n(Bit::X, base_bits)),
            'z' => bits.extend(std::iter::repeat_n(Bit::Z, base_bits)),
            _ => {
                let d = c.to_digit(16).unwrap_or(0);
                for i in (0..base_bits).rev() {
                    bits.push(if d & (1 << i) != 0 {
                        Bit::One
                    } else {
                        Bit::Zero
                    });
                }
            }
        }
    }
    let fill = if size == -1 && bits.len() == base_bits && base_bits == 1 {
        bits.first().copied()
    } else {
        None
    };
    if size > 0 {
        let width = size as usize;
        if bits.len() < width {
            bits.splice(0..0, std::iter::repeat_n(Bit::Zero, width - bits.len()));
        } else if bits.len() > width {
            bits = bits[bits.len() - width..].to_vec();
        }
    }
    elab::Value {
        bits,
        signed: false,
        fill,
    }
}

// ── Statement emission ────────────────────────────────────────────────────────

/// The return-variable context of a function/task being emitted: the C local
/// holding the function-name variable, its width/signedness, and the arena
/// node of the captured return variable.
#[derive(Clone)]
struct RetCtx {
    c_name: String,
    width: u32,
    signed: bool,
    node: Option<NodeId>,
}

/// Call-site resolution metadata of an emitted function/task: its model
/// index (for `CallFn` nodes) plus the signature pieces lowering needs.
#[derive(Clone)]
struct FuncMeta {
    ir: usize,
    is_task: bool,
    ret: Option<(u32, bool)>,
    formals: Vec<(NodeId, bool)>,
}

/// How to read a formal argument (or local) in the lowered IR: width and
/// signedness of the storage (the read expression itself is carried by
/// [`FuncCtx::arg_ir`]).
#[derive(Clone)]
struct ArgMap {
    width: u32,
    signed: bool,
}

/// Context for emitting a function/task definition body (or an inlined task
/// body at a call site): formals mapped to their C expressions, locals to C
/// locals, and the return variable.
#[derive(Clone)]
struct FuncCtx {
    /// The Verilog function/task name.
    name: String,
    /// `true` for a task, `false` for a function (task calls are rejected
    /// inside function bodies).
    is_task: bool,
    /// Return variable context; `None` for void functions and tasks.
    ret: Option<RetCtx>,
    /// io_decl arena node → C expression reading the formal.
    arg_read: HashMap<NodeId, ArgMap>,
    /// io_decl arena node → the IR expression reading the formal (mirrors
    /// `arg_read`; `FormalRead`/substituted bound arguments).
    arg_ir: HashMap<NodeId, IrExpr>,
    /// io_decl arena node → complete C address/lvalue expression for output
    /// and inout formals (`o0` for a C-function parameter, `&G_x` for an
    /// inlined task's bound argument).
    arg_write: HashMap<NodeId, String>,
    /// local var arena node → (C local name, width, signed).
    locals: HashMap<NodeId, (String, u32, bool)>,
    /// Arena node of the function-name return variable (when captured).
    ret_node: Option<NodeId>,
    /// Arena node of the function/task definition (`disable <taskname>;`
    /// inside the body targets it as an early return).  `None` for the
    /// temporary contexts of inlined task bodies — there the same statement
    /// must jump to the expansion's done label instead (see
    /// [`InlineCtx::def`]), never a C return out of the caller's coroutine.
    def_node: Option<NodeId>,
}

/// Context for a delay-bearing task body inlined at a call site.  The
/// argument remapping itself lives in the inline's [`FuncCtx`] (`arg_read` /
/// `arg_write` bound to the caller's expressions); this carries the early-
/// `return` `goto` target and the chain of enclosing inlined tasks (used to
/// reject recursive delay-bearing tasks).
#[derive(Clone)]
struct InlineCtx {
    /// Label for early `return;` statements in the inlined body.
    done_label: String,
    /// The inlined task/function definition node: a `disable <taskname>;`
    /// inside the body targets it exactly like an early `return;`.
    def: NodeId,
    /// Names of the enclosing inlined tasks, innermost last.
    chain: Vec<String>,
    /// Whether a `return;` actually emitted a `goto` to `done_label` (the
    /// label is only emitted when set, to keep `-Wall` clean).
    used: bool,
}

/// One entry of the control-flow scope stack while lowering statements:
/// either a named begin block (`disable <name>` from within jumps to its
/// exit label), a loop construct (`break`/`continue` jump to their
/// labels), or an inlined task body (a lexical barrier `break`/`continue`
/// must not cross into the caller's loops).  The `*_used` flags keep
/// unused labels out of the C output (`-Wall` warns on them), mirroring
/// [`InlineCtx::used`].
enum CtrlScope {
    Block {
        node: NodeId,
        exit: String,
        exit_used: bool,
    },
    Loop {
        brk: String,
        brk_used: bool,
        cont: String,
        cont_used: bool,
    },
    /// Body of a delay-bearing task inlined at a call site: it expands
    /// within the caller's `ctrl` stack, so this marker stops `break`/
    /// `continue` resolution from silently binding to one of the CALLER's
    /// loops (jumping out of the expansion).
    TaskBody,
}

struct EmitCtx<'c, 'a> {
    cg: &'c mut Codegen<'a>,
    path: String,
    saw_wait: bool,
    /// Owning module-instance arena node (used to resolve unbound callees by
    /// name among the instance's function/task definitions).
    inst: NodeId,
    /// C expression for the recursion depth at a call site (`"0"` in
    /// processes, `"depth + 1"` in function bodies).
    depth_arg: String,
    /// Function/task body context while emitting a definition or an inlined
    /// task body.
    func: Option<FuncCtx>,
    /// Inline context while emitting a delay-bearing task body at a call site.
    inline: Option<InlineCtx>,
    /// Fork-branch coroutines and monitor/strobe eval fns created while
    /// lowering this body, in encounter order (rendered ahead of the owning
    /// function/process).
    pre_fns: Vec<crate::sim::ir::IrPreFn>,
    /// Control-flow scope stack (named blocks = `disable` targets, loops =
    /// break/continue targets, inlined task bodies = break/continue
    /// barriers).  Fresh per emitted C function — each
    /// process, function/task definition and fork branch starts its own
    /// context — while inlined task bodies expand within the caller's
    /// context; combined with the sequential [`EmitCtx::label_seq`] this
    /// keeps every emitted label text unique within one C function.
    ctrl: Vec<CtrlScope>,
    label_seq: usize,
    /// `true` while lowering a `final begin … end` body (SV 1800-2005
    /// §10.7): timing controls (`#`/`@`/`wait`/fork suspension) are rejected.
    in_final: bool,
}

impl<'c, 'a> EmitCtx<'c, 'a> {
    /// Build an emission context and record `func`/`depth_arg`/`inst` on the
    /// codegen, where expression and LHS resolution reads them (refs inside
    /// nested operations recurse through `Codegen::emit_expr`, which has no
    /// access to the `EmitCtx` itself).
    fn new(
        cg: &'c mut Codegen<'a>,
        path: String,
        inst: NodeId,
        depth_arg: &str,
        func: Option<FuncCtx>,
        inline: Option<InlineCtx>,
        in_final: bool,
    ) -> EmitCtx<'c, 'a> {
        cg.func = func.clone();
        cg.depth_arg = depth_arg.to_string();
        cg.inst = inst;
        EmitCtx {
            cg,
            path,
            saw_wait: false,
            inst,
            depth_arg: depth_arg.to_string(),
            func,
            inline,
            pre_fns: Vec::new(),
            ctrl: Vec::new(),
            label_seq: 0,
            in_final,
        }
    }

    /// Allocate a fresh control-flow label (`_xb3`, `_bk7`, `_ct9`).  The
    /// sequence is per emitted C function (see [`EmitCtx::ctrl`]); distinct
    /// tags cannot collide with each other or with the inline-task done
    /// labels (`_id<node>`).
    fn new_label(&mut self, tag: &str) -> String {
        self.label_seq += 1;
        format!("_{tag}{}", self.label_seq)
    }

    /// Lower a loop body under a break/continue scope.  The continue label
    /// is appended to the body's END — for every loop shape that lands on
    /// the next-iteration point (for: the increment step; while/repeat/
    /// forever: the back-edge condition test).  Returns the lowered body and
    /// the trailing break label when any `break` used it.
    fn lower_loop_body(
        &mut self,
        body_node: NodeId,
    ) -> Result<(Vec<IrStmt>, Option<IrStmt>), String> {
        let brk = self.new_label("bk");
        let cont = self.new_label("ct");
        self.ctrl.push(CtrlScope::Loop {
            brk,
            brk_used: false,
            cont,
            cont_used: false,
        });
        let mut body = self.lower_stmt(body_node)?;
        match self.ctrl.pop() {
            Some(CtrlScope::Loop {
                brk,
                brk_used,
                cont,
                cont_used,
            }) => {
                if cont_used {
                    body.push(IrStmt::Label(cont));
                }
                if brk_used {
                    Ok((body, Some(IrStmt::Label(brk))))
                } else {
                    Ok((body, None))
                }
            }
            _ => unreachable!("loop scope stack imbalance"),
        }
    }

    /// Lower a SystemVerilog `do body while (cond)` post-test loop.  The
    /// source body runs before the condition on every iteration.  A source
    /// `continue` jumps to the label immediately before that condition, and
    /// both a false condition and a source `break` jump to the label after
    /// the enclosing forever loop.
    fn lower_do_while(
        &mut self,
        cond_node: NodeId,
        body_node: NodeId,
    ) -> Result<Vec<IrStmt>, String> {
        let cond = self.cg.lower_expr(&self.path, cond_node)?;
        let brk = self.new_label("bk");
        let cont = self.new_label("ct");
        self.ctrl.push(CtrlScope::Loop {
            brk: brk.clone(),
            brk_used: false,
            cont: cont.clone(),
            cont_used: false,
        });
        let mut body = self.lower_stmt(body_node)?;
        match self.ctrl.pop() {
            Some(CtrlScope::Loop {
                cont_used,
                brk: actual_brk,
                cont: actual_cont,
                ..
            }) => {
                debug_assert_eq!(actual_brk, brk);
                debug_assert_eq!(actual_cont, cont);
                if cont_used {
                    body.push(IrStmt::Label(cont));
                }
            }
            _ => unreachable!("loop scope stack imbalance"),
        }
        body.push(IrStmt::If {
            cond,
            then_: Vec::new(),
            els: Some(vec![IrStmt::Goto(brk.clone())]),
        });
        Ok(vec![IrStmt::Forever { body }, IrStmt::Label(brk)])
    }

    /// Lower `break;` / `continue;`: an atomic jump to the innermost loop's
    /// break/continue label.  Resolution stops at an inlined-task-body
    /// boundary (a `break` can never target a loop of the CALLER) and
    /// outside a loop this is a syntax-level error that Surelog normally
    /// rejects first; kept as a clean codegen reject.
    fn lower_break_continue(&mut self, is_break: bool) -> Result<Vec<IrStmt>, String> {
        for scope in self.ctrl.iter_mut().rev() {
            match scope {
                CtrlScope::Loop {
                    brk,
                    brk_used,
                    cont,
                    cont_used,
                } => {
                    return Ok(if is_break {
                        *brk_used = true;
                        vec![IrStmt::Goto(brk.clone())]
                    } else {
                        *cont_used = true;
                        vec![IrStmt::Goto(cont.clone())]
                    });
                }
                // Inlined task expansion: its body must resolve jumps within
                // itself only.
                CtrlScope::TaskBody => break,
                CtrlScope::Block { .. } => {}
            }
        }
        Err(format!(
            "`{}` outside a loop in `{}`",
            if is_break { "break" } else { "continue" },
            self.path
        ))
    }

    /// Lower `disable <label>;` (1364-1995 §11) as a same-process jump:
    ///
    /// - target is an enclosing named begin block on the scope stack →
    ///   `goto` its exit label (statements after the disable are skipped);
    /// - target is the task/function currently being INLINED at this site →
    ///   jump to that expansion's done label (= early `return;`);
    /// - target is the plain (non-inlined) function/task whose body is being
    ///   lowered → an early C `return` from it;
    /// - anything else (a block/task of another process, a named fork,
    ///   an outer inlined task) would need cross-coroutine termination and
    ///   is rejected with a clear error instead of mis-lowering.
    fn lower_disable(&mut self, target: Option<NodeId>) -> Result<Vec<IrStmt>, String> {
        let Some(t) = target else {
            return Err(format!(
                "cannot resolve the target of `disable` in `{}`: no matching \
                 enclosing named block or task of the same process (cross-\
                 process disables are not supported in v1)",
                self.path
            ));
        };
        for scope in self.ctrl.iter_mut().rev() {
            if let CtrlScope::Block {
                node,
                exit,
                exit_used,
            } = scope
            {
                if *node == t {
                    *exit_used = true;
                    return Ok(vec![IrStmt::Goto(exit.clone())]);
                }
            }
        }
        // Disabling the innermost inlined task equals an early return out of
        // THIS expansion only; outer inlined tasks stay rejected (their done
        // labels belong to other expansions).
        if let Some(inl) = self.inline.as_mut() {
            if inl.def == t {
                inl.used = true;
                return Ok(vec![IrStmt::Goto(inl.done_label.clone())]);
            }
        }
        if let Some(f) = self.func.as_ref() {
            if f.def_node == Some(t) {
                return Ok(vec![IrStmt::Return { value: None }]);
            }
        }
        Err(format!(
            "disable of `{}` in `{}` is not supported in v1: only an \
             enclosing named block of the same process, or the task itself \
             as an early return, can be disabled (cross-process disables, \
             named forks and outer inlined tasks are not supported)",
            self.cg.node(t).name,
            self.path
        ))
    }
}

impl EmitCtx<'_, '_> {
    /// Lower one statement (or construct) into its IR statements.  Mirrors
    /// the pre-IR emitter decision-for-decision: same errors, warnings,
    /// sensitivity sets and wait tracking.
    fn lower_stmt(&mut self, h: NodeId) -> Result<Vec<IrStmt>, String> {
        match self.cg.kind(h) {
            NodeKind::Stmt(StmtKind::Begin) => {
                let mut body = Vec::new();
                // A named block is a potential `disable` target: its exit
                // label is allocated before the body lowers so disables
                // inside (including inside inlined task expansions) can
                // reference it.
                let named = !self.cg.node(h).name.is_empty();
                if named {
                    let exit = self.new_label("xb");
                    self.ctrl.push(CtrlScope::Block {
                        node: h,
                        exit,
                        exit_used: false,
                    });
                }
                for s in &self.cg.node(h).children {
                    // Local variable declarations inside the block are hoisted
                    // by the function-local collection; skip them here.
                    if matches!(
                        self.cg.kind(*s),
                        NodeKind::Var { .. } | NodeKind::FuncArg { .. }
                    ) {
                        continue;
                    }
                    body.extend(self.lower_stmt(*s)?);
                }
                if named {
                    match self.ctrl.pop() {
                        Some(CtrlScope::Block {
                            exit, exit_used, ..
                        }) => {
                            if exit_used {
                                body.push(IrStmt::Label(exit));
                            }
                        }
                        _ => unreachable!("block scope stack imbalance"),
                    }
                }
                Ok(vec![IrStmt::Block(body)])
            }
            NodeKind::Stmt(StmtKind::IfElse { cond }) => {
                let c = self.cg.lower_expr(&self.path, *cond)?;
                let then_node = self
                    .cg
                    .node(h)
                    .children
                    .get(1)
                    .copied()
                    .ok_or_else(|| "if without then branch".to_string())?;
                let then_ = self.lower_stmt(then_node)?;
                let els = match self.cg.node(h).children.get(2) {
                    Some(e) => Some(self.lower_stmt(*e)?),
                    None => None,
                };
                Ok(vec![IrStmt::If {
                    cond: c,
                    then_,
                    els,
                }])
            }
            NodeKind::Stmt(StmtKind::Assign {
                blocking, delay, ..
            }) => match delay {
                None => Ok(vec![self.lower_assignment(h, false)?]),
                Some(IntraControl::Ticks(ticks)) => {
                    if self.in_final {
                        return Err(format!(
                            "intra-assignment delay inside a final block in `{}` is not \
                             allowed (no timing controls in final)",
                            self.path
                        ));
                    }
                    self.lower_delayed_assignment(h, *blocking, *ticks)
                }
                Some(IntraControl::EventOrRepeat) => Err(format!(
                    "intra-assignment event/repeat control (`@(…)` or \
                     `repeat (n) @(…)`) in `{}` is not supported in v1",
                    self.path
                )),
                Some(IntraControl::UnresolvedDelay) => {
                    let node = self.cg.node(h);
                    let file = node.file.clone().unwrap_or_default();
                    Err(format!(
                        "cannot determine the `#delay` value at {file}:{} \
                         (parameterized delays are not supported in v1)",
                        node.line
                    ))
                }
            },
            NodeKind::Stmt(StmtKind::DelayControl { ticks }) => {
                if self.in_final {
                    return Err(format!(
                        "`#delay` inside a final block in `{}` is not allowed \
                         (LRM 1800-2005 §10.7: no timing controls in final)",
                        self.path
                    ));
                }
                if self.func.is_some() && self.inline.is_none() {
                    return Err(format!(
                        "delay `#{}` inside a function/task body in `{}` is not \
                         supported (delay-bearing tasks are inlined at their call \
                         sites)",
                        ticks
                            .map(|t| t.to_string())
                            .unwrap_or_else(|| "?".to_string()),
                        self.path
                    ));
                }
                let v = match ticks {
                    Some(t) => *t,
                    None => {
                        let file = self.cg.node(h).file.clone().unwrap_or_default();
                        let line = self.cg.node(h).line;
                        return Err(format!(
                            "cannot determine the `#delay` value at {file}:{line} \
                             (parameterized delays are not supported in v1)"
                        ));
                    }
                };
                // `#N` is in the CALLING module's time unit; scale to the
                // design precision (the scheduler tick unit) before waiting.
                let unit_ps = self.cg.timescale_of_node(h).unit_ps;
                let scaled =
                    scale_delay_ticks(v, unit_ps, self.cg.design_precision_ps, &self.path)?;
                self.saw_wait = true;
                let mut out = vec![IrStmt::Delay { ticks: scaled }];
                // The body may be a `Stmt(Empty)` placeholder for a bare
                // `#N;`; skip it.
                if let Some(s) = self.cg.node(h).children.first() {
                    if !matches!(self.cg.kind(*s), NodeKind::Stmt(StmtKind::Empty)) {
                        out.extend(self.lower_stmt(*s)?);
                    }
                }
                Ok(out)
            }
            NodeKind::Stmt(StmtKind::EventControl { .. }) => {
                if self.in_final {
                    return Err(format!(
                        "`@(...)` event control inside a final block in `{}` is not \
                         allowed (LRM 1800-2005 §10.7: no timing controls in final)",
                        self.path
                    ));
                }
                if self.func.is_some() && self.inline.is_none() {
                    return Err(format!(
                        "event control inside a function/task body in `{}` is not \
                         supported (delay-bearing tasks are inlined at their call \
                         sites)",
                        self.path
                    ));
                }
                self.lower_event_control(h)
            }
            NodeKind::Stmt(StmtKind::EventTrigger { target, .. }) => {
                // `-> ev;` (and `->> ev;`, indistinguishable in Surelog v1.86
                // output): wake every current waiter of the event.  A trigger
                // never suspends the process, so it is accepted inside
                // function bodies: IEEE 1800-2009 §13.4.4 explicitly allows
                // non-blocking statements there ("specifically, nonblocking
                // assignments, event triggers, …"), and 1364-2001 §10.3.4(a)
                // bans only statements introduced with #/@/wait.  The
                // `blocking` property is not relied on: Surelog reports 1 for
                // both trigger forms.
                let target = target.ok_or_else(|| {
                    format!(
                        "cannot resolve named event reference `{}` in `{}`",
                        self.cg.node(h).name,
                        self.path
                    )
                })?;
                let idx = self.cg.event_index_of(target, &self.path)?;
                Ok(vec![IrStmt::EventTrigger { ev: idx }])
            }
            NodeKind::Stmt(StmtKind::Case { .. }) => self.lower_case(h),
            NodeKind::Stmt(StmtKind::For { .. }) => self.lower_for(h),
            NodeKind::Stmt(StmtKind::While { cond, body }) => {
                let c = self.cg.lower_expr(&self.path, *cond)?;
                let (body, brk) = self.lower_loop_body(*body)?;
                // `continue` lands on the back edge (the condition test):
                // its label sits at the END of the body (lower_loop_body).
                let mut out = vec![IrStmt::While { cond: c, body }];
                out.extend(brk);
                Ok(out)
            }
            NodeKind::Stmt(StmtKind::DoWhile { cond, body }) => self.lower_do_while(*cond, *body),
            NodeKind::Stmt(StmtKind::Repeat { cond, body }) => {
                let c = self.cg.lower_expr(&self.path, *cond)?;
                if c.is_real() {
                    return Err(format!(
                        "real-valued repeat counts are not supported in `{}`",
                        self.path
                    ));
                }
                if !matches!(c.kind, IrExprKind::Const(_)) {
                    self.cg.warnings.push(format!(
                        "repeat count in `{}` is not a constant; evaluated at runtime",
                        self.path
                    ));
                }
                let (body, brk) = self.lower_loop_body(*body)?;
                let mut out = vec![IrStmt::Repeat { count: c, body }];
                out.extend(brk);
                Ok(out)
            }
            NodeKind::Stmt(StmtKind::Forever { body }) => {
                let (body, brk) = self.lower_loop_body(*body)?;
                let mut out = vec![IrStmt::Forever { body }];
                out.extend(brk);
                Ok(out)
            }
            NodeKind::Stmt(StmtKind::Wait { cond }) => {
                if self.in_final {
                    return Err(format!(
                        "`wait (...)` inside a final block in `{}` is not allowed \
                         (LRM 1800-2005 §10.7: no timing controls in final)",
                        self.path
                    ));
                }
                if self.func.is_some() && self.inline.is_none() {
                    return Err(format!(
                        "wait inside a function/task body in `{}` is not supported \
                         (wait-bearing tasks are inlined at their call sites)",
                        self.path
                    ));
                }
                let c = self.cg.lower_expr(&self.path, *cond)?;
                if c.is_real() {
                    return Err(format!(
                        "real-valued wait conditions are not supported in `{}`",
                        self.path
                    ));
                }
                let sens = self.cg.collect_read_signals(&self.path, *cond)?;
                self.saw_wait = true;
                // The body is optional (`wait (cond);`); the database walk
                // captures it as a child only when Surelog emits a `vpiStmt`.
                // Skip the `Empty` placeholder like the delay/event controls.
                let mut body = Vec::new();
                if let Some(b) = self.cg.node(h).children.get(1) {
                    if !matches!(self.cg.kind(*b), NodeKind::Stmt(StmtKind::Empty)) {
                        body = self.lower_stmt(*b)?;
                    }
                }
                Ok(vec![IrStmt::WaitCond {
                    cond: c,
                    sens,
                    body,
                }])
            }
            NodeKind::Stmt(StmtKind::Force { .. }) => Ok(vec![self.lower_force(h)?]),
            NodeKind::Stmt(StmtKind::Release { .. }) => Ok(vec![self.lower_release(h)?]),
            NodeKind::Stmt(StmtKind::ProcContAssign { .. }) => self.lower_proc_cont_assign(h),
            NodeKind::Stmt(StmtKind::Deassign { lhs }) => self.lower_deassign(*lhs),
            NodeKind::Stmt(StmtKind::Empty) => Ok(vec![IrStmt::Nop]),
            NodeKind::Stmt(StmtKind::Return { value }) => Ok(vec![self.lower_return(*value)?]),
            NodeKind::Stmt(StmtKind::Fork {
                join_kind,
                branches,
            }) => self.lower_fork(*join_kind, branches),
            NodeKind::Stmt(StmtKind::WaitFork) => {
                // `wait fork;` suspends until every live fork group of the
                // current process has completed.
                if self.in_final {
                    return Err(format!(
                        "`wait fork` inside a final block in `{}` is not allowed \
                         (no timing controls or waits in final)",
                        self.path
                    ));
                }
                self.saw_wait = true;
                Ok(vec![IrStmt::WaitFork])
            }
            NodeKind::Stmt(StmtKind::DisableFork) => {
                // `disable fork;` kills every descendant of the current
                // process; the runtime discards their pending NBAs.
                Ok(vec![IrStmt::DisableFork])
            }
            NodeKind::Stmt(StmtKind::Break) | NodeKind::Stmt(StmtKind::Continue) => self
                .lower_break_continue(matches!(self.cg.kind(h), NodeKind::Stmt(StmtKind::Break))),
            NodeKind::Stmt(StmtKind::Disable { target }) => self.lower_disable(*target),
            NodeKind::Stmt(StmtKind::Foreach) => Err(format!(
                "`foreach` in `{}` is not supported in v1",
                self.path
            )),
            NodeKind::Expr(ExprKind::Operation { op, operands, .. })
                if matches!(
                    *op,
                    vpi::vpiPostIncOp | vpi::vpiPreIncOp | vpi::vpiPostDecOp | vpi::vpiPreDecOp
                ) =>
            {
                Ok(vec![self.lower_inc_dec(*op, operands)?])
            }
            NodeKind::SysCall { name } => self.lower_sys_call(h, name),
            NodeKind::FuncCall {
                name,
                is_task,
                callee,
            } => {
                if self.in_final && *is_task {
                    return Err(format!(
                        "task call `{name}` inside a final block in `{}` is not allowed \
                         (final permits function statements only)",
                        self.path
                    ));
                }
                Ok(vec![self.lower_task_call(h, name, *is_task, *callee)?])
            }
            NodeKind::Stmt(StmtKind::Unsupported { vpi_type }) => {
                let node = self.cg.node(h);
                let file = node.file.as_deref().unwrap_or("<unknown>");
                Err(format!(
                    "unsupported executable statement VPI type {vpi_type} at \
                     {file}:{}:{} in `{}`",
                    node.line, node.col, self.path
                ))
            }
            other => Err(format!(
                "unsupported statement in `{}` (node kind {other:?})",
                self.path
            )),
        }
    }

    /// Lower an assignment without intra-assignment delay (`force_blocking`
    /// pins blocking semantics for for-loop init/increment statements).
    fn lower_assignment(&mut self, h: NodeId, force_blocking: bool) -> Result<IrStmt, String> {
        let (blocking, op) = match self.cg.kind(h) {
            NodeKind::Stmt(StmtKind::Assign { blocking, op, .. }) => (*blocking, *op),
            _ => unreachable!("non-assignment passed to lower_assignment"),
        };
        if matches!(
            self.cg.kind(h),
            NodeKind::Stmt(StmtKind::Assign { delay: Some(_), .. })
        ) {
            // Reached only from the for-loop init/increment path; plain
            // statement assignments are classified in lower_stmt.
            return Err(format!(
                "intra-assignment delay on a for-loop init/increment \
                 assignment in `{}` is not supported",
                self.path
            ));
        }
        let lhs = self
            .cg
            .node(h)
            .children
            .first()
            .copied()
            .ok_or_else(|| "assignment without LHS".to_string())?;
        let rhs = self
            .cg
            .node(h)
            .children
            .get(1)
            .copied()
            .ok_or_else(|| "assignment without RHS".to_string())?;
        let blocking = force_blocking || blocking;
        if self.in_final && !blocking {
            return Err(format!(
                "nonblocking assignment inside a final block in `{}` is not \
                 allowed (final permits function statements only)",
                self.path
            ));
        }
        let lh = self.cg.lower_lhs(&self.path, lhs)?;
        let rhs_ir = self.lower_assignment_rhs(lhs, rhs, op, &lh)?;
        let rhs_ir = apply_lhs_assignment_context(&self.cg.model, &lh, rhs_ir);
        Ok(IrStmt::Assign {
            lhs: lh,
            rhs: rhs_ir,
            nba: !blocking,
        })
    }

    /// Lower the value written by a normal or compound procedural
    /// assignment.  Compound assignments currently require a whole scalar
    /// target, which guarantees the LHS is evaluated once; select and array
    /// targets need index temporaries before they can preserve that rule.
    fn lower_assignment_rhs(
        &mut self,
        lhs_node: NodeId,
        rhs_node: NodeId,
        op: i32,
        lhs: &IrLhs,
    ) -> Result<IrExpr, String> {
        let rhs = self.cg.lower_expr(&self.path, rhs_node)?;
        if op == 0 || op == vpi::vpiAssignmentOp {
            return Ok(rhs);
        }
        if !matches!(lhs, IrLhs::Whole(_) | IrLhs::WholeRef { .. }) {
            return Err(format!(
                "compound assignment to a select or array element in `{}` is not supported yet",
                self.path
            ));
        }
        let current = self.cg.lower_expr(&self.path, lhs_node)?;
        self.lower_compound_expr(op, current, rhs)
    }

    fn lower_compound_expr(&self, op: i32, lhs: IrExpr, rhs: IrExpr) -> Result<IrExpr, String> {
        let real = lhs.is_real() || rhs.is_real();
        let result = match op {
            vpi::vpiAddOp | vpi::vpiSubOp | vpi::vpiMultOp => {
                if real {
                    let op = match op {
                        vpi::vpiAddOp => IrRealBinOp::Add,
                        vpi::vpiSubOp => IrRealBinOp::Sub,
                        _ => IrRealBinOp::Mul,
                    };
                    real_bin_expr(op, lhs, rhs)
                } else {
                    let op = match op {
                        vpi::vpiAddOp => IrBinOp::Add,
                        vpi::vpiSubOp => IrBinOp::Sub,
                        _ => IrBinOp::Mul,
                    };
                    common_bin_expr(op, lhs, rhs)
                }
            }
            vpi::vpiDivOp | vpi::vpiModOp => {
                if real {
                    real_bin_expr(
                        if op == vpi::vpiDivOp {
                            IrRealBinOp::Div
                        } else {
                            IrRealBinOp::Mod
                        },
                        lhs,
                        rhs,
                    )
                } else {
                    if lhs.width > 64 || rhs.width > 64 {
                        return Err(format!(
                            "wide division/modulo not yet supported (operand wider than 64 bits) in `{}`",
                            self.path
                        ));
                    }
                    common_bin_expr(
                        if op == vpi::vpiDivOp {
                            IrBinOp::Div
                        } else {
                            IrBinOp::Mod
                        },
                        lhs,
                        rhs,
                    )
                }
            }
            vpi::vpiBitAndOp | vpi::vpiBitOrOp | vpi::vpiBitXorOp => {
                if real {
                    return Err(format!(
                        "bitwise compound assignment on a real value in `{}` is not supported",
                        self.path
                    ));
                }
                let op = match op {
                    vpi::vpiBitAndOp => IrBinOp::BitAnd,
                    vpi::vpiBitOrOp => IrBinOp::BitOr,
                    _ => IrBinOp::BitXor,
                };
                common_bin_expr(op, lhs, rhs)
            }
            vpi::vpiLShiftOp | vpi::vpiRShiftOp | vpi::vpiArithLShiftOp | vpi::vpiArithRShiftOp => {
                if real {
                    return Err(format!(
                        "shift compound assignment on a real value in `{}` is not supported",
                        self.path
                    ));
                }
                let width = lhs.width;
                let signed = lhs.signed;
                let op = match op {
                    vpi::vpiLShiftOp => IrBinOp::Shl,
                    vpi::vpiRShiftOp => IrBinOp::Shr,
                    vpi::vpiArithLShiftOp => IrBinOp::Ashl,
                    _ => IrBinOp::Ashr,
                };
                IrExpr::new(
                    IrExprKind::Bin {
                        op,
                        a: Box::new(lhs),
                        b: Box::new(rhs),
                    },
                    width,
                    signed,
                    None,
                )
            }
            other => {
                return Err(format!(
                    "unsupported compound assignment operation {other} in `{}`",
                    self.path
                ))
            }
        };
        Ok(result)
    }

    /// Lower a statement-position pre/post increment or decrement.  Since
    /// the operation's value is discarded in statement position, pre and
    /// post forms have the same blocking-write behavior.  Expression-valued
    /// forms require a side-effecting expression IR and remain unsupported.
    fn lower_inc_dec(&mut self, op: i32, operands: &[NodeId]) -> Result<IrStmt, String> {
        let operand = match operands {
            [operand] => *operand,
            _ => {
                return Err(format!(
                    "increment/decrement in `{}` must have exactly one operand",
                    self.path
                ))
            }
        };
        let lhs = self.cg.lower_lhs(&self.path, operand)?;
        if !matches!(lhs, IrLhs::Whole(_) | IrLhs::WholeRef { .. }) {
            return Err(format!(
                "increment/decrement of a select or array element in `{}` is not supported yet",
                self.path
            ));
        }
        let current = self.cg.lower_expr(&self.path, operand)?;
        let increment = matches!(op, vpi::vpiPostIncOp | vpi::vpiPreIncOp);
        let rhs = if current.is_real() {
            real_bin_expr(
                if increment {
                    IrRealBinOp::Add
                } else {
                    IrRealBinOp::Sub
                },
                current,
                real_literal_expr(1.0),
            )
        } else {
            let one = IrExpr::new(
                IrExprKind::Const(IrConst {
                    bits: vec![1],
                    x: vec![0],
                    z: vec![0],
                    width: 32,
                    signed: true,
                    real: None,
                    fill: None,
                }),
                32,
                true,
                None,
            );
            common_bin_expr(
                if increment {
                    IrBinOp::Add
                } else {
                    IrBinOp::Sub
                },
                current,
                one,
            )
        };
        let rhs = apply_lhs_assignment_context(&self.cg.model, &lhs, rhs);
        Ok(IrStmt::Assign {
            lhs,
            rhs,
            nba: false,
        })
    }

    /// Lower an intra-assignment-delayed assignment (`lhs = #N rhs;`,
    /// `lhs <= #N rhs;`; LRM 1364-1995 §9.7.4): the RHS is evaluated once,
    /// immediately, into a temp; the process suspends N ticks; then the LHS
    /// is updated from the temp — as a blocking write, or recorded for the
    /// delayed step's NBA region.  `#0` suspends through the runtime's
    /// zero-delay (inactive-region) wait.
    fn lower_delayed_assignment(
        &mut self,
        h: NodeId,
        blocking: bool,
        ticks_raw: u64,
    ) -> Result<Vec<IrStmt>, String> {
        if self.func.is_some() && self.inline.is_none() {
            return Err(format!(
                "delay inside a function/task body in `{}` is not supported \
                 (delay-bearing tasks are inlined at their call sites)",
                self.path
            ));
        }
        let lhs = self
            .cg
            .node(h)
            .children
            .first()
            .copied()
            .ok_or_else(|| "assignment without LHS".to_string())?;
        let rhs = self
            .cg
            .node(h)
            .children
            .get(1)
            .copied()
            .ok_or_else(|| "assignment without RHS".to_string())?;
        let lh = self.cg.lower_lhs(&self.path, lhs)?;
        let op = match self.cg.kind(h) {
            NodeKind::Stmt(StmtKind::Assign { op, .. }) => *op,
            _ => unreachable!("non-assignment passed to lower_delayed_assignment"),
        };
        let rhs_ir = self.lower_assignment_rhs(lhs, rhs, op, &lh)?;
        let rhs_ir = apply_lhs_assignment_context(&self.cg.model, &lh, rhs_ir);
        if rhs_ir.is_real() {
            return Err(format!(
                "real-valued intra-assignment delays are not supported in `{}`",
                self.path
            ));
        }
        // `#N` is in the CALLING module's time unit; scale to the design
        // precision (the scheduler tick unit), exactly like a `#N` statement.
        let unit_ps = self.cg.timescale_of_node(h).unit_ps;
        let scaled =
            scale_delay_ticks(ticks_raw, unit_ps, self.cg.design_precision_ps, &self.path)?;
        self.saw_wait = true;
        // The temp name is unique per assignment node; each site's Block keeps
        // re-declarations (loops, repeated task inlining) out of one C scope.
        let tmp = format!("_t{}", h.0);
        let (w, s) = (rhs_ir.width, rhs_ir.signed);
        Ok(vec![IrStmt::Block(vec![
            IrStmt::DeclLocal {
                name: tmp.clone(),
                width: w,
                signed: s,
                init: Some(Box::new(rhs_ir)),
            },
            IrStmt::Delay { ticks: scaled },
            IrStmt::Assign {
                lhs: lh,
                rhs: IrExpr::new(IrExprKind::LocalRead(tmp), w, s, None),
                nba: !blocking,
            },
        ])])
    }

    /// Lower `@(…)`: explicit edge/any specs become ONE atomic
    /// `llg_wait_any_events`; implicit sensitivity waits on the body's read
    /// set.
    fn lower_event_control(&mut self, h: NodeId) -> Result<Vec<IrStmt>, String> {
        let (specs, implicit, body) = match self.cg.kind(h) {
            NodeKind::Stmt(StmtKind::EventControl {
                specs,
                implicit,
                body,
            }) => (specs, *implicit, *body),
            _ => unreachable!("non-event-control passed to lower_event_control"),
        };
        let body = body.ok_or_else(|| "event_control without body".to_string())?;
        let spec_pairs = self.lower_event_specs(specs)?;
        let wait = if implicit || spec_pairs.is_empty() {
            // @* / always_comb without explicit sensitivity, or a condition
            // that produced no specs: wait on the body's read set.
            IrStmt::WaitAny {
                sens: self.cg.collect_read_signals(&self.path, body)?,
            }
        } else {
            IrStmt::WaitEvents { specs: spec_pairs }
        };
        self.saw_wait = true;
        let mut out = vec![wait];
        // The body may be a `Stmt(Empty)` placeholder for a bare
        // `@(posedge clk);`; skip it.
        if !matches!(self.cg.kind(body), NodeKind::Stmt(StmtKind::Empty)) {
            out.extend(self.lower_stmt(body)?);
        }
        Ok(out)
    }

    /// Resolve the captured event specs into atomic wait sources: signal
    /// globals with edges, plus named events.  A mixed list stays ONE
    /// `WaitEvents` statement (one runtime call), so a trigger can never be
    /// lost between two separate waits.
    fn lower_event_specs(
        &mut self,
        specs: &[EventSpec],
    ) -> Result<Vec<(IrWaitSrc, IrEdge)>, String> {
        let mut out = Vec::new();
        for s in specs {
            match s {
                EventSpec::Edge { sig, posedge } => {
                    // Edge controls on named events are rejected cleanly
                    // (v1): an event has no value, so posedge/negedge have no
                    // meaning (LRM 1364-1995 §9.7.3 note).
                    if let Some(ev) = self.cg.event_target_of(*sig) {
                        let name = self.cg.node(ev).name.clone();
                        return Err(format!(
                            "edge control on a named event is not supported in v1 \
                             (`{name}` in `{}`)",
                            self.path
                        ));
                    }
                    let (name, info) = self.cg.resolve_signal_id(&self.path, *sig)?;
                    if info.real {
                        return Err(format!(
                            "real-valued signals cannot be event-controlled in `{}`",
                            self.path
                        ));
                    }
                    out.push((
                        IrWaitSrc::Sig(name),
                        if *posedge {
                            IrEdge::Posedge
                        } else {
                            IrEdge::Negedge
                        },
                    ));
                }
                EventSpec::AnyChange { sig } => {
                    if let Some(ev) = self.cg.event_target_of(*sig) {
                        let idx = self.cg.event_index_of(ev, &self.path)?;
                        out.push((IrWaitSrc::Event(idx), IrEdge::Any));
                        continue;
                    }
                    let (name, info) = self.cg.resolve_signal_id(&self.path, *sig)?;
                    if info.real {
                        return Err(format!(
                            "real-valued signals cannot be event-controlled in `{}`",
                            self.path
                        ));
                    }
                    out.push((IrWaitSrc::Sig(name), IrEdge::Any));
                }
                EventSpec::Named(ev) => {
                    // `@(ev)` — wait on the event trigger itself; events are
                    // edge-triggered (a trigger before the wait does not
                    // latch).
                    let idx = self.cg.event_index_of(*ev, &self.path)?;
                    out.push((IrWaitSrc::Event(idx), IrEdge::Any));
                }
            }
        }
        Ok(out)
    }

    fn lower_case(&mut self, h: NodeId) -> Result<Vec<IrStmt>, String> {
        let (case_type, items) = match self.cg.kind(h) {
            NodeKind::Stmt(StmtKind::Case { case_type, items }) => (*case_type, items),
            _ => unreachable!("non-case passed to lower_case"),
        };
        // VPI case subtypes (vpi_user.h): vpiCaseExact=1 (case), vpiCaseX=2
        // (casex), vpiCaseZ=3 (casez).  casez/casex keep wildcard matching
        // per LRM 12.5.1 instead of degrading to exact equality.
        let kind = match case_type {
            vpi::vpiCaseExact => IrCaseKind::Exact,
            VPI_CASE_X => IrCaseKind::Casex,
            VPI_CASE_Z => IrCaseKind::Casez,
            other => return Err(format!("unsupported case type {other} in `{}`", self.path)),
        };
        let sel = self
            .cg
            .node(h)
            .children
            .first()
            .copied()
            .ok_or_else(|| "case without selector".to_string())?;
        let sel_ir = self.cg.lower_expr(&self.path, sel)?;
        if sel_ir.is_real() {
            return Err(format!(
                "real-valued case selectors are not supported in `{}`",
                self.path
            ));
        }
        let mut ir_items = Vec::with_capacity(items.len());
        for item in items.iter() {
            let mut exprs = Vec::with_capacity(item.exprs.len());
            for e in &item.exprs {
                let c = self.cg.lower_expr(&self.path, *e)?;
                if c.is_real() {
                    return Err(format!(
                        "real-valued case items are not supported in `{}`",
                        self.path
                    ));
                }
                exprs.push(c);
            }
            let mut body = Vec::new();
            if let Some(s) = item.body {
                body = self.lower_stmt(s)?;
            }
            ir_items.push(IrCaseItem { exprs, body });
        }
        Ok(vec![IrStmt::Case {
            sel: sel_ir,
            kind,
            items: ir_items,
        }])
    }

    fn lower_for(&mut self, h: NodeId) -> Result<Vec<IrStmt>, String> {
        // Verified against UHDM for_stmt.h: vpiForInitStmt (75) = init stmt(s),
        // vpiCondition (71) = condition, vpiForIncStmt (74) = increment stmt(s),
        // vpiStmt (104) = body.
        let (init, cond, incr, body) = match self.cg.kind(h) {
            NodeKind::Stmt(StmtKind::For {
                init,
                cond,
                incr,
                body,
            }) => (init.clone(), *cond, incr.clone(), *body),
            _ => unreachable!("non-for passed to lower_for"),
        };
        let cond_ir = self.cg.lower_expr(&self.path, cond)?;
        let mut init_stmts = Vec::with_capacity(init.len());
        for s in &init {
            init_stmts.push(self.lower_assignment(*s, true)?);
        }
        let (body_stmts, brk) = self.lower_loop_body(body)?;
        let mut incr_stmts = Vec::with_capacity(incr.len());
        for s in &incr {
            match self.cg.kind(*s) {
                NodeKind::Stmt(StmtKind::Assign { .. }) => {
                    incr_stmts.push(self.lower_assignment(*s, true)?);
                }
                NodeKind::Expr(ExprKind::Operation { op, operands, .. })
                    if matches!(
                        *op,
                        vpi::vpiPostIncOp | vpi::vpiPreIncOp | vpi::vpiPostDecOp | vpi::vpiPreDecOp
                    ) =>
                {
                    incr_stmts.push(self.lower_inc_dec(*op, operands)?)
                }
                other => {
                    return Err(format!(
                        "unsupported for-loop increment in `{}` (node kind {other:?})",
                        self.path
                    ))
                }
            }
        }
        // `continue` must run the increment before the condition test: its
        // label sits at the END of the body, and the emitter renders the
        // body BEFORE the incr statements inside the C `for`.
        let mut out = vec![IrStmt::For {
            init: init_stmts,
            cond: cond_ir,
            incr: incr_stmts,
            body: body_stmts,
        }];
        // The break label trails the whole construct.
        out.extend(brk);
        Ok(out)
    }

    /// Lower one `fork … join` site.  Each branch becomes its own coroutine
    /// function attached to the enclosing process's pre-functions; nested
    /// constructs inside a branch append their own pre-functions first,
    /// mirroring the pre-IR emission order.
    ///
    /// Fork inside a function/task body is rejected: the branch functions are
    /// standalone coroutines with no access to the enclosing function's
    /// formals/locals.
    fn lower_fork(&mut self, join_kind: i32, branches: &[NodeId]) -> Result<Vec<IrStmt>, String> {
        if self.func.is_some() {
            return Err(format!(
                "fork/join inside a function/task body in `{}` is not supported in v1",
                self.path
            ));
        }
        if self.in_final {
            // A join suspends the process; a final block may not suspend
            // (LRM 1800-2005 §10.7).
            return Err(format!(
                "fork/join inside a final block in `{}` is not allowed \
                 (no timing controls or waits in final)",
                self.path
            ));
        }
        let join = match join_kind {
            0 => IrJoinKind::Join,
            1 => IrJoinKind::None,
            2 => IrJoinKind::Any,
            k => return Err(format!("unsupported fork join type {k} in `{}`", self.path)),
        };
        let mut names = Vec::with_capacity(branches.len());
        for (k, branch) in branches.iter().enumerate() {
            let fn_name = format!("{}_b{}", self.cg.new_fn_name(&self.path, "fork"), k);
            // Branches live in the same instance scope: same path, same
            // owning instance; refs resolve to the instance globals.
            let mut bctx = EmitCtx::new(
                self.cg,
                self.path.clone(),
                self.inst,
                "0",
                None,
                None,
                false,
            );
            let body = bctx.lower_stmt(*branch)?;
            self.pre_fns.append(&mut bctx.pre_fns);
            self.pre_fns.push(crate::sim::ir::IrPreFn::Branch {
                c_name: fn_name.clone(),
                body,
            });
            names.push((fn_name, format!("{}.br{k}", self.path)));
        }
        // A fork is a wait even for join_none: a wait-free `always` containing
        // `fork … join_none` must not be wrapped as a comb process (its
        // branches are child coroutines, not combinational re-evaluation).
        self.saw_wait = true;
        Ok(vec![IrStmt::Fork {
            join_kind: join,
            branches: names,
        }])
    }

    /// Lower `force lhs = rhs;` — override a whole signal's value until
    /// released.  While forced, the runtime drops procedural writes to the
    /// signal; only whole-signal targets are supported (no selects/arrays, no
    /// collapsed-net members).
    fn lower_force(&mut self, h: NodeId) -> Result<IrStmt, String> {
        let lhs = self
            .cg
            .node(h)
            .children
            .first()
            .copied()
            .ok_or_else(|| "force without LHS".to_string())?;
        let rhs = self
            .cg
            .node(h)
            .children
            .get(1)
            .copied()
            .ok_or_else(|| "force without RHS".to_string())?;
        let lh = self.cg.lower_lhs(&self.path, lhs)?;
        let (sig_idx, width, signed) = match &lh {
            IrLhs::Whole(idx) => {
                let sig = self.cg.model.signal(*idx);
                match sig.ty {
                    IrType::Real { .. } => {
                        return Err(
                            "force/release of real-valued signal is not supported".to_string()
                        );
                    }
                    t => {
                        if sig.net_driver.is_none() {
                            (*idx, t.width(), t.signed())
                        } else {
                            return Err(format!(
                                "force on `{}` in `{}` is not supported (whole signals only)",
                                self.cg.node(lhs).name,
                                self.path
                            ));
                        }
                    }
                }
            }
            _ => {
                return Err(format!(
                    "force on `{}` in `{}` is not supported (whole signals only)",
                    self.cg.node(lhs).name,
                    self.path
                ))
            }
        };
        let rhs_ir = apply_assignment_expression_width(self.cg.lower_expr(&self.path, rhs)?, width);
        // Force writes an assignment into the signal: value-preserving
        // RHS→target conversion (LRM §10.7).
        let value = ir_to_vector(rhs_ir, width, signed)?;
        Ok(IrStmt::Force {
            sig: sig_idx,
            value,
        })
    }

    /// Lower `release lhs;` — cancel a procedural force on a whole signal.
    fn lower_release(&mut self, h: NodeId) -> Result<IrStmt, String> {
        let lhs = self
            .cg
            .node(h)
            .children
            .first()
            .copied()
            .ok_or_else(|| "release without LHS".to_string())?;
        let lh = self.cg.lower_lhs(&self.path, lhs)?;
        match &lh {
            IrLhs::Whole(idx) => {
                let sig = self.cg.model.signal(*idx);
                if matches!(sig.ty, IrType::Real { .. }) {
                    return Err("force/release of real-valued signal is not supported".to_string());
                }
                if sig.net_driver.is_some() {
                    return Err(format!(
                        "release on `{}` in `{}` is not supported (whole signals only)",
                        self.cg.node(lhs).name,
                        self.path
                    ));
                }
                Ok(IrStmt::Release { sig: *idx })
            }
            _ => Err(format!(
                "release on `{}` in `{}` is not supported (whole signals only)",
                self.cg.node(lhs).name,
                self.path
            )),
        }
    }

    /// Resolve the target of a procedural continuous `assign` / `deassign`.
    /// Variables only (LRM 1364-1995 §9.4): nets, selects/part-selects/
    /// array elements, hierarchical paths and (v1 scope) real variables are
    /// rejected cleanly.  Returns the signal's IR index plus its info.
    fn pca_target(&mut self, lhs: NodeId, stmt: &str) -> Result<(usize, SignalInfo), String> {
        if matches!(self.cg.kind(lhs), NodeKind::Expr(ExprKind::HierPath { .. })) {
            return Err(format!(
                "procedural continuous `{stmt}` on a hierarchical target in `{}` is not \
                 supported (variables of the current scope only)",
                self.path
            ));
        }
        let lh = self.cg.lower_lhs(&self.path, lhs)?;
        let sig_idx = match &lh {
            IrLhs::Whole(idx) => *idx,
            _ => {
                return Err(format!(
                    "procedural continuous `{stmt}` on a select in `{}` is not supported \
                     (whole variables only)",
                    self.path
                ));
            }
        };
        // Net targets: the elaborated ref normally binds the declaration, so
        // the net/var distinction is read straight off the arena node.
        // Surelog v1.86 quirk: a module-level `reg` is captured as
        // NodeKind::Net with net_type vpiReg (48) — that IS a variable.
        if let NodeKind::Expr(ExprKind::Ref { target: Some(t) }) = self.cg.kind(lhs) {
            if let NodeKind::Net { net_type, .. } = self.cg.kind(*t) {
                if *net_type != vpi::vpiReg {
                    return Err(format!(
                        "procedural continuous `{stmt}` on net `{}` in `{}` is not supported \
                         (variables only)",
                        self.cg.node(*t).name,
                        self.path
                    ));
                }
            }
        }
        let info = self
            .cg
            .signals
            .iter()
            .find(|i| i.ir == sig_idx)
            .cloned()
            .ok_or_else(|| {
                format!(
                    "cannot collect `{}` in `{}` as a procedural continuous assignment \
                     target (whole variables of the current scope only)",
                    self.cg.node(lhs).name,
                    self.path
                )
            })?;
        if info.real {
            return Err(format!(
                "procedural continuous `{stmt}` on real-valued variable in `{}` is not \
                 supported in v1",
                self.path
            ));
        }
        Ok((sig_idx, info))
    }

    /// Pre-scan phase 1 for one process body: resolve each collected
    /// ProcContAssign target and allocate its site (enable signal) without
    /// lowering anything.  A variable that already has a site is rejected
    /// HERE — order-independently, whichever statement comes second in the
    /// db child order.
    fn claim_pca_sites(&mut self, nodes: &[NodeId]) -> Result<(), String> {
        for n in nodes {
            let lhs = self
                .cg
                .node(*n)
                .children
                .first()
                .copied()
                .ok_or_else(|| "procedural continuous assignment without LHS".to_string())?;
            let (sig_idx, _) = self.pca_target(lhs, "assign")?;
            if self.cg.pca_sites.contains_key(&sig_idx) {
                return Err(self.cg.pca_multi_site_err(lhs, &self.path));
            }
            let en = self.cg.new_pca_enable(&self.path);
            self.cg.pca_sites.insert(
                sig_idx,
                PcaSite {
                    en,
                    guarded_by: None,
                },
            );
        }
        Ok(())
    }

    /// Lower `assign <variable> = expr;` (procedural continuous assignment,
    /// LRM 1364-1995 §9.4).  Decided model, one process per SITE:
    ///
    /// ```text
    /// for (;;) {                       // IrShape::Loop guard process
    ///     wait_any(rhs_reads ∪ en);    // en changes wake the guard too
    ///     if (en) write(lhs, rhs_now); // disabled wakes skip silently
    /// }
    /// ```
    ///
    /// The statement itself sets `en = 1` AND performs an immediate blocking
    /// write (so the value updates in the same delta); `deassign` clears
    /// `en` only — the variable KEEPS its last value (LRM).  Both writes go
    /// through the normal `llg_ba` path, so an active `force` keeps
    /// overriding the PCA (LRM 10.6 interplay).
    ///
    /// NBA-vs-PCA interplay: while a site is enabled, ordinary procedural
    /// writes to the variable — blocking AND non-blocking — still take
    /// effect immediately/as usual; the guard never wakes on changes of the
    /// TARGET itself, so such a write survives until the next wake
    /// triggered by an RHS-read or the enable, which re-drives the variable
    /// from the CURRENT rhs.
    ///
    /// Sites are pre-allocated by [`Codegen::prescan_pca_sites`] before any
    /// body lowers; this method only CLAIMS the pre-allocated site by
    /// materializing its guard process.
    fn lower_proc_cont_assign(&mut self, h: NodeId) -> Result<Vec<IrStmt>, String> {
        let lhs = self
            .cg
            .node(h)
            .children
            .first()
            .copied()
            .ok_or_else(|| "procedural continuous assignment without LHS".to_string())?;
        let rhs = self
            .cg
            .node(h)
            .children
            .get(1)
            .copied()
            .ok_or_else(|| "procedural continuous assignment without RHS".to_string())?;
        let (sig_idx, info) = self.pca_target(lhs, "assign")?;

        // The dedicated guard process: waits on the RHS read set ∪ {en} and
        // re-writes the CURRENT rhs whenever it wakes while enabled.
        let mut sens = Vec::new();
        let mut seen = HashSet::new();
        let mut visited = HashSet::new();
        self.cg
            .walk_read_signals(&self.path, rhs, &mut seen, &mut visited, &mut sens)?;
        let rhs_reads_real = sens
            .iter()
            .any(|name| self.cg.signals.iter().any(|i| i.real && &i.global == name));
        if rhs_reads_real {
            return Err(format!(
                "real-valued signals in the RHS of a procedural continuous \
                 assignment in `{}` are not supported in v1",
                self.path
            ));
        }
        let rhs_ir =
            apply_assignment_expression_width(self.cg.lower_expr(&self.path, rhs)?, info.width);
        // The guard re-writes an assignment into the variable:
        // value-preserving RHS→target conversion (LRM §10.7).
        let value = ir_to_vector(rhs_ir, info.width, info.signed)?;
        let one = IrExpr::new(
            IrExprKind::Const(IrConst {
                bits: vec![1],
                x: vec![0],
                z: vec![0],
                width: 1,
                signed: false,
                real: None,
                fill: None,
            }),
            1,
            false,
            None,
        );

        // Site bookkeeping.  Sites exist for every process-body statement
        // already (pre-scan); a missing entry is only reachable from trees
        // the pre-scan does not walk (defensive fallback with identical
        // semantics).
        let existing = self
            .cg
            .pca_sites
            .get(&sig_idx)
            .map(|s| (s.en, s.guarded_by));
        let en_ir = match existing {
            Some((en, Some(prev))) => {
                if prev != h {
                    return Err(self.cg.pca_multi_site_err(lhs, &self.path));
                }
                // The same statement lowered again (a delay-bearing task
                // body inlined at several call sites): site and guard both
                // exist — only re-execute enable + immediate write.
                return Ok(vec![
                    IrStmt::Assign {
                        lhs: IrLhs::Whole(en),
                        rhs: one,
                        nba: false,
                    },
                    IrStmt::Assign {
                        lhs: IrLhs::Whole(sig_idx),
                        rhs: value,
                        nba: false,
                    },
                ]);
            }
            Some((en, None)) => {
                if let Some(site) = self.cg.pca_sites.get_mut(&sig_idx) {
                    site.guarded_by = Some(h);
                }
                en
            }
            None => {
                let en = self.cg.new_pca_enable(&self.path);
                self.cg.pca_sites.insert(
                    sig_idx,
                    PcaSite {
                        en,
                        guarded_by: Some(h),
                    },
                );
                en
            }
        };
        let en_global = self.cg.model.signals[en_ir].c_name.clone();
        if !sens.contains(&en_global) {
            sens.push(en_global);
        }
        let guard_body = vec![
            IrStmt::WaitAny { sens },
            IrStmt::If {
                cond: IrExpr::new(IrExprKind::SigRead(en_ir), 1, false, None),
                then_: vec![IrStmt::Assign {
                    lhs: IrLhs::Whole(sig_idx),
                    rhs: value.clone(),
                    nba: false,
                }],
                els: None,
            },
        ];
        let guard_name = self.cg.new_fn_name(&self.path, "pca");
        self.cg.model.processes.push(IrProcess {
            c_name: guard_name,
            label: format!("{}.pca", self.path),
            shape: IrShape::Loop,
            pre_fns: Vec::new(),
            body: guard_body,
        });

        // Statement execution: enable, then the immediate blocking write
        // (dropped by the runtime while the target is forced).
        Ok(vec![
            IrStmt::Assign {
                lhs: IrLhs::Whole(en_ir),
                rhs: one,
                nba: false,
            },
            IrStmt::Assign {
                lhs: IrLhs::Whole(sig_idx),
                rhs: value,
                nba: false,
            },
        ])
    }

    /// Lower `deassign <variable>;` — cancel the procedural continuous
    /// assignment by clearing the site's enable.  The variable KEEPS its
    /// last assigned value (LRM 1364-1995 §9.4).  A `deassign` on a variable
    /// without any PCA site has no effect (warned); net/select/hierarchical
    /// targets are rejected like `assign` targets.  Sites are pre-allocated
    /// before any body lowers (`Codegen::prescan_pca_sites`), so this finds
    /// its site regardless of which process lowers first.
    fn lower_deassign(&mut self, lhs: NodeId) -> Result<Vec<IrStmt>, String> {
        let (sig_idx, _) = self.pca_target(lhs, "deassign")?;
        match self.cg.pca_sites.get(&sig_idx) {
            Some(site) => {
                let en_ir = site.en;
                let zero = IrExpr::new(
                    IrExprKind::Const(IrConst {
                        bits: vec![0],
                        x: vec![0],
                        z: vec![0],
                        width: 1,
                        signed: false,
                        real: None,
                        fill: None,
                    }),
                    1,
                    false,
                    None,
                );
                Ok(vec![IrStmt::Assign {
                    lhs: IrLhs::Whole(en_ir),
                    rhs: zero,
                    nba: false,
                }])
            }
            None => {
                self.cg.warnings.push(format!(
                    "deassign of `{}` in `{}` has no effect (no procedural \
                     continuous assignment on this variable)",
                    self.cg.node(lhs).name,
                    self.path
                ));
                Ok(vec![IrStmt::Nop])
            }
        }
    }

    /// Lower system-task calls ($display/$monitor/$strobe/$finish/…).
    /// Skippable constructs warn here and produce no statements.
    fn lower_sys_call(&mut self, h: NodeId, name: &str) -> Result<Vec<IrStmt>, String> {
        let args: Vec<NodeId> = self.cg.node(h).children.clone();
        match name {
            "$display" | "$write" => {
                let (fmt, display_args) = self.parse_display_call(name, &args, true)?;
                Ok(vec![IrStmt::Display {
                    fmt,
                    args: display_args,
                    newline: name == "$display",
                }])
            }
            "$monitor" | "$strobe" => {
                if self.in_final {
                    return Err(format!(
                        "{name} inside a final block in `{}` is not supported: \
                         no scheduled output events execute after final procedures",
                        self.path
                    ));
                }
                let (fmt, display_args) = self.parse_display_call(name, &args, false)?;
                // Generated re-evaluator: reads the CURRENT values of the
                // displayed arguments each time the runtime prints (after an
                // NBA commit for the monitor, at the end of the time step for
                // $strobe).  Attached ahead of the enclosing function/process.
                let eval_name = self.cg.new_fn_name(&self.path, "mon");
                let eval_args = display_args.iter().map(|(e, _)| e.clone()).collect();
                self.pre_fns.push(crate::sim::ir::IrPreFn::MonEval {
                    c_name: eval_name.clone(),
                    args: eval_args,
                });
                Ok(vec![IrStmt::MonitorSet {
                    strobe: name == "$strobe",
                    fmt,
                    eval: eval_name,
                    n_args: display_args.len(),
                }])
            }
            "$monitoron" => Ok(vec![IrStmt::MonitorEnable(true)]),
            "$monitoroff" => Ok(vec![IrStmt::MonitorEnable(false)]),
            "$dumpfile" => {
                if args.len() != 1 {
                    return Err(format!(
                        "$dumpfile requires exactly one literal string argument in `{}`",
                        self.path
                    ));
                }
                let path = match self.cg.kind(args[0]) {
                    NodeKind::Expr(ExprKind::Constant {
                        const_type: vpi::vpiStringConst,
                        value: ValueData::Str(path),
                        ..
                    }) => path.clone(),
                    _ => {
                        return Err(format!(
                            "$dumpfile requires a literal string argument in `{}`",
                            self.path
                        ))
                    }
                };
                let lower_path = path.to_ascii_lowercase();
                if !lower_path.ends_with(".vcd") && !lower_path.ends_with(".fst") {
                    return Err(format!(
                        "$dumpfile path must end in .vcd or .fst in `{}`",
                        self.path
                    ));
                }
                self.cg.model.waveform = true;
                Ok(vec![IrStmt::WaveFile(path)])
            }
            "$dumpvars" => {
                if !args.is_empty() && !self.cg.warned_dumpvars_filtering {
                    self.cg.warnings.push(
                        "$dumpvars depth/scope filtering is not yet implemented; dumping all \
                         registered storage"
                            .to_string(),
                    );
                    self.cg.warned_dumpvars_filtering = true;
                }
                self.cg.model.waveform = true;
                Ok(vec![IrStmt::WaveDumpVars])
            }
            "$dumpon" | "$dumpoff" | "$dumpall" | "$dumpflush" => {
                if !args.is_empty() {
                    return Err(format!("{name} takes no arguments in `{}`", self.path));
                }
                self.cg.model.waveform = true;
                Ok(vec![match name {
                    "$dumpon" => IrStmt::WaveOn,
                    "$dumpoff" => IrStmt::WaveOff,
                    "$dumpall" => IrStmt::WaveDumpAll,
                    "$dumpflush" => IrStmt::WaveFlush,
                    _ => unreachable!(),
                }])
            }
            "$dumplimit" => {
                if args.len() != 1 {
                    return Err(format!(
                        "$dumplimit requires exactly one packed expression in `{}`",
                        self.path
                    ));
                }
                let limit = self.cg.lower_expr(&self.path, args[0])?;
                if limit.is_real() {
                    return Err(format!(
                        "$dumplimit requires a packed expression, not real, in `{}`",
                        self.path
                    ));
                }
                self.cg.model.waveform = true;
                Ok(vec![IrStmt::WaveLimit(limit)])
            }
            "$finish" => Ok(vec![IrStmt::Finish]),
            "$printtimescale" => {
                let ts = self.cg.timescale_of_node(h);
                Ok(vec![IrStmt::PrintTimescale {
                    unit_ps: ts.unit_ps,
                    precision_ps: ts.precision_ps,
                    label: self.path.clone(),
                }])
            }
            "$displayon" | "$displayoff" => {
                self.cg.warnings.push(format!(
                    "{name} in `{}` skipped (not supported in v1)",
                    self.path
                ));
                Ok(vec![])
            }
            _ => Err(format!("unsupported system task {name} in `{}`", self.path)),
        }
    }

    /// Parse a $display/$monitor/$strobe call's arguments into the C format
    /// string and the lowered value expressions (with their realness flags).
    /// The first string constant is the format; every remaining argument is a
    /// value expression consumed by one format specifier (`%d/%h/%b/%o/%t`,
    /// `%s` only when `allow_strings`).  `allow_strings` is true for $display
    /// (whose `llg_display` reads string arguments from the varargs);
    /// monitors and strobes pass false because their arguments are
    /// re-evaluated as `sv4_t` values by the generated eval function.
    fn parse_display_call(
        &mut self,
        name: &str,
        args: &[NodeId],
        allow_strings: bool,
    ) -> Result<(String, Vec<(IrExpr, bool)>), String> {
        let mut fmt_arg: Option<String> = None;
        let mut display_args = Vec::new();
        for a in args {
            let is_fmt = matches!(
                self.cg.kind(*a),
                NodeKind::Expr(ExprKind::Constant {
                    const_type: vpi::vpiStringConst,
                    ..
                })
            );
            if is_fmt && fmt_arg.is_none() {
                fmt_arg = match self.cg.kind(*a) {
                    NodeKind::Expr(ExprKind::Constant {
                        value: ValueData::Str(s),
                        ..
                    }) => Some(s.clone()),
                    _ => None,
                };
            } else {
                let e = self.cg.lower_expr(&self.path, *a)?;
                display_args.push((e.clone(), e.is_real()));
            }
        }
        let fmt =
            fmt_arg.ok_or_else(|| format!("{name} without a format string in `{}`", self.path))?;
        if !allow_strings && display_args.iter().any(|(e, _)| e.is_real()) {
            return Err(format!(
                "{name} cannot monitor/strobe real-valued arguments in `{}`",
                self.path
            ));
        }
        let mut c_fmt = String::from("\"");
        let mut arg_idx = 0usize;
        let mut chars = fmt.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch != '%' {
                c_fmt.push_str(&escaped_char(ch));
                continue;
            }
            let mut spec = String::from("%");
            while let Some(&n) = chars.peek() {
                if n == '-' || n == '+' || n == '0' || n == '.' || n.is_ascii_digit() {
                    spec.push(chars.next().unwrap());
                } else {
                    break;
                }
            }
            let conv = chars.next().unwrap_or('%');
            spec.push(conv);
            match conv {
                'd' | 'h' | 'b' | 'o' => {
                    if arg_idx >= display_args.len() {
                        return Err(format!(
                            "{name} format `%{conv}` in `{}` has no argument",
                            self.path
                        ));
                    }
                    if display_args[arg_idx].0.is_real() {
                        return Err(format!(
                            "{name} integer format `%{conv}` cannot consume a real in `{}`",
                            self.path
                        ));
                    }
                    arg_idx += 1;
                    c_fmt.push('%');
                    c_fmt.push(conv);
                }
                's' => {
                    if !allow_strings {
                        return Err(format!(
                            "{name} format `%s` in `{}` is not supported (monitor/\
                             strobe arguments are re-evaluated as values)",
                            self.path
                        ));
                    }
                    if arg_idx >= display_args.len() {
                        return Err(format!(
                            "{name} format `%{conv}` in `{}` has no argument",
                            self.path
                        ));
                    }
                    arg_idx += 1;
                    c_fmt.push('%');
                    c_fmt.push(conv);
                }
                'f' | 'e' | 'g' => {
                    if !allow_strings {
                        return Err(format!(
                            "{name} real formatting is not supported for monitor/strobe in `{}`",
                            self.path
                        ));
                    }
                    if arg_idx >= display_args.len() {
                        return Err(format!(
                            "{name} format `%{conv}` in `{}` has no argument",
                            self.path
                        ));
                    }
                    if !display_args[arg_idx].0.is_real() {
                        return Err(format!(
                            "{name} real format `%{conv}` requires a real argument in `{}`",
                            self.path
                        ));
                    }
                    arg_idx += 1;
                    c_fmt.push_str(&spec);
                }
                't' => {
                    // %t consumes an argument (typically $time); the runtime
                    // prints the argument's value as a decimal (already scaled
                    // to the caller's time unit by the $time emission).
                    if arg_idx >= display_args.len() {
                        return Err(format!(
                            "{name} format `%{conv}` in `{}` has no argument",
                            self.path
                        ));
                    }
                    arg_idx += 1;
                    c_fmt.push('%');
                    c_fmt.push('t');
                }
                '%' => c_fmt.push('%'),
                other => {
                    return Err(format!(
                        "unsupported {name} format specifier `%{other}` in `{}`",
                        self.path
                    ))
                }
            }
        }
        if arg_idx != display_args.len() {
            return Err(format!(
                "{name} in `{}` has {} argument(s) for {} format specifier(s)",
                self.path,
                display_args.len(),
                arg_idx
            ));
        }
        c_fmt.push('"');
        Ok((c_fmt, display_args))
    }

    /// Lower a `task_call` statement (or a function call used as a statement).
    /// Delay-bearing tasks are inlined at the call site; delay-free tasks (and
    /// functions) become IR calls with caller-side temps for output formals.
    fn lower_task_call(
        &mut self,
        h: NodeId,
        name: &str,
        is_task: bool,
        callee: Option<NodeId>,
    ) -> Result<IrStmt, String> {
        if let Some(f) = &self.func {
            if !f.is_task {
                return Err(format!(
                    "task call `{name}` inside function `{}` is not supported",
                    f.name
                ));
            }
        }
        let ft = self.cg.resolve_callee(self.inst, name, is_task, callee)?;
        // All function/task definitions carry names; the model index exists
        // only for emitted (delay-free) callees.
        self.cg
            .func_names
            .get(&ft)
            .ok_or_else(|| format!("task `{name}` has no C name"))?;
        let (_, _, formals) = self.cg.func_info(ft)?;
        let args: Vec<NodeId> = self.cg.node(h).children.clone();
        let bound = self.cg.bind_call_args(&formals, &args)?;
        if is_task && self.cg.task_has_wait(ft, self.inst) {
            self.lower_task_inline(ft, h, &formals, &bound)
        } else {
            let fidx = self
                .cg
                .func_meta
                .get(&ft)
                .map(|m| m.ir)
                .ok_or_else(|| format!("task `{name}` has no C name"))?;
            self.lower_call_stmts(fidx, h, &formals, &bound)
        }
    }

    /// Lower a delay-free task/function statement call: caller-side temps for
    /// select-target output/inout formals, direct addresses for addressable
    /// lvalues, inputs by value.
    fn lower_call_stmts(
        &mut self,
        fidx: usize,
        h: NodeId,
        formals: &[(NodeId, bool)],
        bound: &[BoundArg],
    ) -> Result<IrStmt, String> {
        let mut temps: Vec<(String, usize, Option<IrExpr>)> = Vec::new();
        let mut copyouts: Vec<(IrLhs, String, u32, bool)> = Vec::new();
        // The C signature orders all outputs first, then inputs — build the
        // argument list in that order, not by formal declaration index.
        let mut out_args: Vec<IrCallArg> = Vec::new();
        let mut in_args: Vec<IrCallArg> = Vec::new();
        let mut arg_codes: Vec<Option<String>> = vec![None; formals.len()];
        let mut arg_irs: Vec<Option<IrExpr>> = vec![None; formals.len()];
        for (idx, (io, is_out)) in formals.iter().enumerate() {
            if !*is_out {
                continue;
            }
            // An output/inout actual bound to a whole signal (or another
            // addressable lvalue) is passed through directly, so writes
            // inside the task — including non-blocking ones, which commit
            // in the NBA region *after* the call returns — hit the actual
            // itself.  Select targets keep the temp + writeback (a temp is
            // not an addressable whole signal).
            let lh = self.cg.lower_lhs(&self.path, bound[idx].expr)?;
            match &lh {
                IrLhs::Whole(sig_i) => {
                    let sig = self.cg.model.signal(*sig_i);
                    let global = sig.c_name.clone();
                    let src_signed = sig.ty.signed();
                    let sig_w = sig.ty.width();
                    arg_irs[idx] = Some(ir_arg_resize(
                        IrExpr::new(IrExprKind::SigRead(*sig_i), sig_w, src_signed, None),
                        bound[idx].width,
                        bound[idx].signed,
                    ));
                    arg_codes[idx] = Some(arg_resize(&global, bound[idx].width, bound[idx].signed));
                    out_args.push(IrCallArg::OutAddr(format!("&{global}")));
                }
                IrLhs::WholeRef {
                    addr,
                    width,
                    signed,
                } => {
                    let value = if let Some(rest) = addr.strip_prefix('&') {
                        rest.to_string()
                    } else {
                        format!("(*{addr})")
                    };
                    arg_irs[idx] = Some(ir_arg_resize(
                        verbatim_code_owned(value.clone(), *width, *signed),
                        bound[idx].width,
                        bound[idx].signed,
                    ));
                    arg_codes[idx] = Some(arg_resize(&value, *width, bound[idx].signed));
                    out_args.push(IrCallArg::OutAddr(addr.clone()));
                }
                _ => {
                    let tname = format!("_a{}", h.0);
                    let (_init_code, init_ir) =
                        self.cg.lower_call_temp_init(&self.path, *io, &bound[idx])?;
                    temps.push((tname.clone(), idx, init_ir));
                    copyouts.push((lh, tname.clone(), bound[idx].width, bound[idx].signed));
                    arg_irs[idx] = Some(IrExpr::new(
                        IrExprKind::LocalRead(tname.clone()),
                        bound[idx].width,
                        bound[idx].signed,
                        None,
                    ));
                    arg_codes[idx] = Some(tname.clone());
                    out_args.push(IrCallArg::OutAddr(format!("&{tname}")));
                }
            }
        }
        for (idx, (_, is_out)) in formals.iter().enumerate() {
            if !*is_out {
                let mut own_irs: Vec<Option<IrExpr>> = vec![None; formals.len()];
                let (_code, ir) = self.cg.lower_bound_arg_code(
                    &self.path,
                    formals,
                    bound,
                    idx,
                    &mut arg_codes,
                    &mut own_irs,
                )?;
                in_args.push(IrCallArg::Val(ir));
            }
        }
        out_args.extend(in_args);
        let depth = parse_depth(&self.depth_arg);
        Ok(IrStmt::Call(IrCall {
            f: fidx,
            args: out_args,
            depth,
            temps,
            copyouts,
        }))
    }

    /// Lower a delay-bearing task body inlined at its call site: the task's
    /// io_decls are bound to the caller's argument expressions (writes go
    /// straight to the bound actuals through the func-context remap), locals
    /// get fresh names, and the body lowers under the inline context.  A call
    /// to a task already being inlined (recursion) is rejected.
    fn lower_task_inline(
        &mut self,
        ft: NodeId,
        h: NodeId,
        formals: &[(NodeId, bool)],
        bound: &[BoundArg],
    ) -> Result<IrStmt, String> {
        let tname = self.cg.node(ft).name.clone();
        if let Some(inl) = &self.inline {
            if inl.chain.contains(&tname) {
                return Err(format!(
                    "recursive delay-bearing task `{tname}` is not supported"
                ));
            }
        }
        let body = self
            .cg
            .func_body(ft)
            .ok_or_else(|| format!("task `{tname}` without a body"))?;

        // Task locals get fresh C names per inline site (the same task may be
        // inlined several times in one block).
        let mut locals: HashMap<NodeId, (String, u32, bool)> = HashMap::new();
        let mut local_seq = 0usize;
        let prefix = format!("_i{}", h.0);
        self.cg
            .collect_func_locals(body, &mut locals, &mut local_seq, &prefix)?;

        // Formals bound to the caller's argument expressions.
        let mut arg_read: HashMap<NodeId, ArgMap> = HashMap::new();
        let mut arg_ir: HashMap<NodeId, IrExpr> = HashMap::new();
        let mut arg_write: HashMap<NodeId, String> = HashMap::new();
        let mut arg_codes: Vec<Option<String>> = vec![None; formals.len()];
        let mut arg_irs: Vec<Option<IrExpr>> = vec![None; formals.len()];
        // Input formals bound to caller rvalue expressions need a writable
        // local copy (an input formal is a local copy in SystemVerilog).
        let mut input_copies: Vec<(String, IrExpr)> = Vec::new();
        for (idx, (io, is_out)) in formals.iter().enumerate() {
            let b = &bound[idx];
            if *is_out {
                let lh = self.cg.lower_lhs(&self.path, b.expr)?;
                let addr = match &lh {
                    IrLhs::Whole(sig_i) => format!("&{}", self.cg.model.signal(*sig_i).c_name),
                    IrLhs::WholeRef { addr, .. } => addr.clone(),
                    _ => {
                        return Err(format!(
                            "output argument of task `{tname}` bound to a select \
                             is not supported"
                        ))
                    }
                };
                let read_ir = self.cg.lower_expr(&self.path, b.expr)?;
                arg_write.insert(*io, addr);
                arg_ir.insert(*io, read_ir.clone());
                arg_read.insert(
                    *io,
                    ArgMap {
                        width: b.width,
                        signed: b.signed,
                    },
                );
                arg_irs[idx] = Some(read_ir.clone());
                arg_codes[idx] = Some(self.cg.render_ir_code(&read_ir)?);
            } else {
                let (_code, ir) = self.cg.lower_bound_arg_code(
                    &self.path,
                    formals,
                    bound,
                    idx,
                    &mut arg_codes,
                    &mut arg_irs,
                )?;
                let cname = format!("_il{}_{}", h.0, idx);
                arg_write.insert(*io, format!("&{cname}"));
                arg_ir.insert(
                    *io,
                    IrExpr::new(
                        IrExprKind::LocalRead(cname.clone()),
                        b.width,
                        b.signed,
                        None,
                    ),
                );
                arg_read.insert(
                    *io,
                    ArgMap {
                        width: b.width,
                        signed: b.signed,
                    },
                );
                input_copies.push((cname, ir));
            }
        }

        let done_label = format!("_id{}", h.0);
        let mut chain = match &self.inline {
            Some(inl) => inl.chain.clone(),
            None => Vec::new(),
        };
        chain.push(tname.clone());
        let func = FuncCtx {
            name: tname.clone(),
            is_task: true,
            ret: None,
            arg_read,
            arg_ir,
            arg_write,
            locals,
            ret_node: None,
            // Deliberately None: a `disable <taskname>;` inside an inlined
            // body must jump to THIS expansion's done label (the InlineCtx
            // check runs first), never emit a C return out of the caller's
            // coroutine.
            def_node: None,
        };
        let inline = InlineCtx {
            done_label: done_label.clone(),
            def: ft,
            chain,
            used: false,
        };

        // Swap in the inline context (and sync the codegen for expression
        // resolution), then restore on the way out.
        let saved_cg = (self.cg.func.take(), self.cg.depth_arg.clone());
        let saved_ctx = (self.func.take(), self.inline.take(), self.depth_arg.clone());
        let depth = format!("({}) + 1", saved_ctx.2);
        self.cg.func = Some(func.clone());
        self.cg.depth_arg = depth.clone();
        self.func = Some(func);
        self.inline = Some(inline);
        self.depth_arg = depth;

        let mut stmts: Vec<IrStmt> = Vec::new();
        // Locals sorted by node id (emission order of the pre-IR emitter).
        let mut local_names: Vec<(u32, (String, u32, bool))> = self
            .func
            .as_ref()
            .map(|f| {
                f.locals
                    .iter()
                    .map(|(id, v)| (id.0, v.clone()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        local_names.sort_by_key(|(id, _)| *id);
        for (_, (cname, w, s)) in local_names {
            stmts.push(IrStmt::DeclLocal {
                name: cname,
                width: w,
                signed: s,
                init: None,
            });
        }
        for (cname, ir) in input_copies {
            stmts.push(IrStmt::DeclLocal {
                name: cname,
                width: 0,
                signed: false,
                init: Some(Box::new(ir)),
            });
        }
        // The body expands within the caller's control stack; the TaskBody
        // barrier keeps a `break`/`continue` inside it from binding to one
        // of the caller's loops (it must resolve within the task body or be
        // rejected cleanly).
        self.ctrl.push(CtrlScope::TaskBody);
        stmts.extend(self.lower_stmt(body)?);
        match self.ctrl.pop() {
            Some(CtrlScope::TaskBody) => {}
            _ => unreachable!("task-body scope stack imbalance"),
        }
        if self.inline.as_ref().map(|i| i.used).unwrap_or(false) {
            stmts.push(IrStmt::Label(done_label));
        }

        self.cg.func = saved_cg.0;
        self.cg.depth_arg = saved_cg.1;
        self.func = saved_ctx.0;
        self.inline = saved_ctx.1;
        self.depth_arg = saved_ctx.2;
        Ok(IrStmt::Block(stmts))
    }

    /// Lower a `return` statement.  In a non-void function the value becomes
    /// the `_ret` conversion + C return (rendered); in a void function/task it
    /// is a bare return.  Inside an inlined task body it jumps to the done
    /// label.
    fn lower_return(&mut self, value: Option<NodeId>) -> Result<IrStmt, String> {
        if let Some(inl) = self.inline.as_mut() {
            if value.is_some() {
                return Err("return with a value inside a task".to_string());
            }
            inl.used = true;
            return Ok(IrStmt::Goto(inl.done_label.clone()));
        }
        match self.func.as_ref() {
            Some(f) if f.ret.is_some() => {
                let r = f.ret.as_ref().expect("ret width known").clone();
                match value {
                    Some(v) => {
                        let e = self.cg.lower_expr(&self.path, v)?;
                        let e = apply_assignment_expression_width(e, r.width);
                        Ok(IrStmt::Return {
                            value: Some(Box::new(e)),
                        })
                    }
                    None => Ok(IrStmt::Return { value: None }),
                }
            }
            Some(_) => Ok(IrStmt::Return { value: None }),
            None => Err(format!(
                "return statement outside a function/task in `{}`",
                self.path
            )),
        }
    }
}

/// A verbatim rendered fragment as an IR expression (function-scope storage
/// reads such as `(*o0)` or a local name).
fn verbatim_code_owned(code: String, width: u32, signed: bool) -> IrExpr {
    IrExpr::new(
        IrExprKind::Verbatim {
            code,
            width,
            signed,
        },
        width,
        signed,
        None,
    )
}

/// Convert a call value to a formal's `(width, signed)` shape:
/// value-preserving (`sv4_cast`, extension keyed off the *source* value's
/// own signedness — an unsigned source zero-extends even into a signed
/// formal, LRM 1800-2009 §6.24.1 / §10.7).  Plain `sv4_resize` would
/// sign-extend whenever the target is signed, corrupting unsigned values
/// whose MSB is set (e.g. 3-bit `5` resized to a signed 32-bit integer
/// formal), and zero-extend into unsigned targets, losing a signed value's
/// sign.
fn arg_resize(code: &str, width: u32, signed: bool) -> String {
    format!("sv4_cast({code}, {width}, {})", signed as u8)
}

/// Lowered LHS of an assignment.
enum Lhs {
    Whole(SignalInfo),
    /// A complete C address/lvalue expression (a `sv4_t*` parameter such as
    /// `o0`, or `&_l0` for a local); no `&` is prepended by [`lhs_rhs_code`].
    WholeRef {
        addr: String,
        width: u32,
        signed: bool,
    },
    Bit(SignalInfo, String),
    Part(SignalInfo, i128, i128),
    IdxPart(SignalInfo, String, String, i32),
    /// A write to one array element, with an optional element-level
    /// bit/part-select.  Emitted as a guarded statement (out-of-range or
    /// unknown indices are no-ops), never as a plain `llg_ba` argument.
    ArrayElem(ArrayElemLhs),
}

/// A trailing select on a hierarchical assignment target, recovered from the
/// node name / source line (Surelog v1.86's elaborated model does not carry
/// hierarchical selects as structured expressions — see
/// [`Codegen::hier_lhs_select`]).
enum HierSelect {
    /// `[i]` — bit select.
    Bit(i128),
    /// `[l:r]` — part select.
    Part(i128, i128),
    /// `[b +: w]` / `[b -: w]` — indexed part select.
    IdxPart(i128, i128, bool),
}

/// One side of a port connection: a plain global signal, or an element of an
/// unpacked array (the parent side of a connection like `.cnt(cnts[i])`,
/// addressed by the select expression the db captured on the port).
enum LinkSide {
    Signal(SignalInfo),
    ArrayElem(ArrayInfo, NodeId),
}

// ── Expression seam: IR lowering + rendering ──────────────────────────────────

impl<'a> Codegen<'a> {
    /// Render context for the IR built so far (the enclosing function, when
    /// any, resolves formal reads).
    fn render_ctx(&self) -> RCtx<'_> {
        RCtx {
            model: &self.model,
            func: self.cur_fn_ir.map(|i| &self.model.funcs[i]),
        }
    }

    /// Render a lowered expression to its C code.
    fn render_ir_code(&self, ir: &IrExpr) -> Result<String, String> {
        let ctx = self.render_ctx();
        Ok(render_expr(&ctx, ir)?.code)
    }

    /// Lower `h` to IR and render it in one step.  This is THE expression
    /// seam: every consumer sees exactly the pre-IR rendered C text.
    fn emit_expr(&mut self, scope_path: &str, h: NodeId) -> Result<String, String> {
        let ir = self.lower_expr(scope_path, h)?;
        self.render_ir_code(&ir)
    }

    /// Lower an expression node decision-for-decision like the pre-IR
    /// emitter: same widths, signednesses, fills, and error strings.
    fn lower_expr(&mut self, scope_path: &str, h: NodeId) -> Result<IrExpr, String> {
        match self.kind(h) {
            NodeKind::Expr(ExprKind::Constant { .. }) => {
                let c = self.const_of_node(h)?;
                Ok(IrExpr::new(
                    IrExprKind::Const(c.clone()),
                    c.width,
                    c.signed,
                    c.fill,
                ))
            }
            NodeKind::EnumConst { value } => match value {
                Some(Val::Bits(v)) => {
                    let c = val_to_const(v)?;
                    Ok(IrExpr::new(
                        IrExprKind::Const(c.clone()),
                        c.width,
                        c.signed,
                        None,
                    ))
                }
                Some(Val::Real(v)) => Ok(real_literal_expr(*v)),
                Some(Val::Str(_)) => Err("string enum constant in expression".to_string()),
                None => Err("enum constant without value in expression".to_string()),
            },
            NodeKind::Expr(ExprKind::Ref { target }) => self.lower_ref_expr(scope_path, h, *target),
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                if let Some(ai) = self.array_of(*base).cloned() {
                    if ai.dims.len() != 1 {
                        return Err(format!(
                            "array slice access (`{}[...]` on a {}-dimensional array) \
                             is not supported in `{scope_path}`",
                            self.node(*base).name,
                            ai.dims.len()
                        ));
                    }
                    let ie = self.lower_expr(scope_path, *index)?;
                    return Ok(IrExpr::new(
                        IrExprKind::ArrayRead {
                            arr: ai.ir,
                            indices: vec![ie],
                            elem_sel: IrElemSel::Whole,
                        },
                        ai.elem_width,
                        ai.signed,
                        None,
                    ));
                }
                let (_, info) = self.base_signal(scope_path, *base)?;
                if info.real {
                    return Err(format!(
                        "select on real-valued signal in `{scope_path}` is not supported"
                    ));
                }
                let ie = self.lower_expr(scope_path, *index)?;
                Ok(IrExpr::new(
                    IrExprKind::BitSel {
                        base: Box::new(sig_read_expr(info.ir)),
                        idx: Box::new(ie),
                    },
                    1,
                    false,
                    None,
                ))
            }
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                let ai = self.array_of(*base).cloned().ok_or_else(|| {
                    format!(
                        "cannot resolve array base of select `{}` in `{scope_path}`",
                        self.node(*base).name
                    )
                })?;
                let ndims = ai.dims.len();
                if indices.len() == ndims {
                    let ies = indices
                        .iter()
                        .map(|i| self.lower_expr(scope_path, *i))
                        .collect::<Result<Vec<_>, _>>()?;
                    return Ok(IrExpr::new(
                        IrExprKind::ArrayRead {
                            arr: ai.ir,
                            indices: ies,
                            elem_sel: IrElemSel::Whole,
                        },
                        ai.elem_width,
                        ai.signed,
                        None,
                    ));
                }
                if indices.len() == ndims + 1 {
                    let last = *indices.last().expect("non-empty indices");
                    let ies = indices[..ndims]
                        .iter()
                        .map(|i| self.lower_expr(scope_path, *i))
                        .collect::<Result<Vec<_>, _>>()?;
                    let (elem_sel, width) = match self.kind(last) {
                        NodeKind::Expr(ExprKind::PartSelect { left, right, .. }) => {
                            let l = self.eval_bound_i128(*left)?;
                            let r = self.eval_bound_i128(*right)?;
                            let width = ((l - r).abs() + 1) as u32;
                            (IrElemSel::Part(l, r), width)
                        }
                        NodeKind::Expr(ExprKind::IndexedPartSelect { .. }) => {
                            return Err(format!(
                                "indexed part-select on an array element is not \
                                 supported in `{scope_path}`"
                            ))
                        }
                        _ => {
                            let ie = self.lower_expr(scope_path, last)?;
                            (IrElemSel::Bit(Box::new(ie)), 1)
                        }
                    };
                    return Ok(IrExpr::new(
                        IrExprKind::ArrayRead {
                            arr: ai.ir,
                            indices: ies,
                            elem_sel,
                        },
                        width,
                        false,
                        None,
                    ));
                }
                Err(format!(
                    "array `{}` in `{scope_path}`: {}-level select on a \
                     {}-dimensional array is not supported",
                    self.node(*base).name,
                    indices.len(),
                    ndims
                ))
            }
            NodeKind::Expr(ExprKind::PartSelect { base, left, right }) => {
                let (_, info) = self.base_signal(scope_path, *base)?;
                if info.real {
                    return Err(format!(
                        "select on real-valued signal in `{scope_path}` is not supported"
                    ));
                }
                let l = self.eval_bound_i128(*left)?;
                let r = self.eval_bound_i128(*right)?;
                let width = ((l - r).abs() + 1) as u32;
                Ok(IrExpr::new(
                    IrExprKind::PartSel {
                        base: Box::new(sig_read_expr(info.ir)),
                        left: l,
                        right: r,
                    },
                    width,
                    false,
                    None,
                ))
            }
            NodeKind::Expr(ExprKind::IndexedPartSelect {
                base,
                base_expr,
                width_expr,
                neg,
            }) => {
                let (_, info) = self.base_signal(scope_path, *base)?;
                if info.real {
                    return Err(format!(
                        "select on real-valued signal in `{scope_path}` is not supported"
                    ));
                }
                let be = self.lower_expr(scope_path, *base_expr)?;
                let we = self.lower_expr(scope_path, *width_expr)?;
                let width = match self.const_of_node(*width_expr) {
                    Ok(c) if c.real.is_none() => c.width,
                    Ok(_) => {
                        return Err(format!(
                            "indexed part-select width cannot be real in `{scope_path}`"
                        ))
                    }
                    Err(_) => {
                        return Err(format!(
                            "indexed part-select width must be a constant in `{scope_path}`"
                        ))
                    }
                };
                Ok(IrExpr::new(
                    IrExprKind::IdxPartSel {
                        base: Box::new(sig_read_expr(info.ir)),
                        base_idx: Box::new(be),
                        width_expr: Box::new(we),
                        neg: *neg,
                    },
                    width,
                    false,
                    None,
                ))
            }
            NodeKind::Expr(ExprKind::Operation {
                op,
                reordered,
                operands,
            }) => self.lower_operation(scope_path, *op, *reordered, operands),
            NodeKind::Expr(ExprKind::Cast { operand, ty }) => {
                let v = self.lower_expr(scope_path, *operand)?;
                if matches!(ty.kind.as_str(), "real" | "shortreal") {
                    return Ok(IrExpr::new(
                        IrExprKind::CastToReal {
                            a: Box::new(v),
                            shortreal: ty.kind == "shortreal",
                        },
                        REAL_EXPR_WIDTH,
                        true,
                        None,
                    ));
                }
                let (w, s) = match (ty.width, ty.signed) {
                    (Some(w), s) => (w, s),
                    (None, _) => {
                        return Err(format!(
                            "cast with unsized target type `{}` in `{scope_path}`",
                            ty.kind
                        ))
                    }
                };
                if w > LLG_MAX_WIDTH {
                    return Err(format!(
                        "cast target in `{scope_path}` is {w} bits wide; the v1 \
                         runtime supports at most {LLG_MAX_WIDTH}"
                    ));
                }
                // Value-preserving conversion (LRM 1800-2009 §6.24.1: the
                // cast yields the value a variable of the cast type holds
                // after the assignment — extension follows the SOURCE's
                // signedness, so int'(8'hFF) is 255, not -1).
                Ok(ir_to_vector(v, w, s)?)
            }
            NodeKind::SysCall { name } => self.lower_sys_func_expr(scope_path, name, h),
            NodeKind::FuncCall {
                name,
                is_task,
                callee,
            } => {
                if *is_task {
                    return Err(format!(
                        "task call `{name}` used as an expression in `{scope_path}`"
                    ));
                }
                self.lower_func_call_expr(scope_path, h, name, *callee)
            }
            NodeKind::Expr(ExprKind::HierPath { .. }) => {
                // 2-part interface member access (`m.data`): a read of the
                // resolved per-port copy var.
                if let Some(info) = self.hier_path_signal(h) {
                    return Ok(sig_read_expr_full(info));
                }
                Err(format!(
                    "hierarchical references are not supported (in `{scope_path}`)"
                ))
            }
            other => Err(format!(
                "unsupported expression in `{scope_path}` (node kind {other:?})"
            )),
        }
    }

    fn lower_ref_expr(
        &self,
        scope_path: &str,
        r: NodeId,
        target: Option<NodeId>,
    ) -> Result<IrExpr, String> {
        if let Some(t) = target {
            if let Some(info) = self.signal_of(t) {
                return Ok(sig_read_expr_full(info));
            }
            // Function/task body reads: formals, locals and the return
            // variable (by arena node).
            if let Some(f) = &self.func {
                if let Some(ir) = f.arg_ir.get(&t) {
                    return Ok(ir.clone());
                }
                if let Some((cname, w, s)) = f.locals.get(&t) {
                    return Ok(IrExpr::new(
                        IrExprKind::LocalRead(cname.clone()),
                        *w,
                        *s,
                        None,
                    ));
                }
                if f.ret_node == Some(t) {
                    if let Some(rctx) = &f.ret {
                        return Ok(IrExpr::new(
                            IrExprKind::LocalRead(rctx.c_name.clone()),
                            rctx.width,
                            rctx.signed,
                            None,
                        ));
                    }
                }
            }
            if let Some(v) = self.param_vals.get(&t) {
                return match v {
                    Val::Bits(b) => {
                        let c = val_to_const(b)?;
                        Ok(IrExpr::new(
                            IrExprKind::Const(c.clone()),
                            c.width,
                            c.signed,
                            None,
                        ))
                    }
                    Val::Real(value) => Ok(real_literal_expr(*value)),
                    Val::Str(_) => Err(format!(
                        "string parameter `{}` used as a value is not supported",
                        self.node(t).name
                    )),
                };
            }
            if let NodeKind::EnumConst { value } = self.kind(t) {
                return enum_value_expr(value.as_ref(), &self.node(t).name);
            }
        }
        // Name fallback within the current scope.
        let name = self.node(r).name.clone();
        if !name.is_empty() {
            // io_decls are not indexed, so formals resolve by name.
            if let Some(f) = &self.func {
                for (io, ir) in &f.arg_ir {
                    if self.node(*io).name == name {
                        return Ok(ir.clone());
                    }
                }
                for (node, (cname, w, s)) in &f.locals {
                    if self.node(*node).name == name {
                        return Ok(IrExpr::new(
                            IrExprKind::LocalRead(cname.clone()),
                            *w,
                            *s,
                            None,
                        ));
                    }
                }
                if let Some(rctx) = &f.ret {
                    if rctx
                        .node
                        .map(|n| self.node(n).name == name)
                        .unwrap_or(false)
                    {
                        return Ok(IrExpr::new(
                            IrExprKind::LocalRead(rctx.c_name.clone()),
                            rctx.width,
                            rctx.signed,
                            None,
                        ));
                    }
                }
            }
            if let Some(info) = self
                .scope_sig_names
                .get(scope_path)
                .and_then(|m| m.get(&name))
            {
                return Ok(sig_read_expr_full(info));
            }
            // Surelog v1.87 leaves some unqualified module-local enum uses
            // without `vpiActual`.  Resolve those only against the current
            // instance's matching flat module definition and only when the
            // enumerator name is unique there.
            let def_name = match self.kind(self.inst) {
                NodeKind::ModuleInst { def_name, .. } => strip_lib(def_name),
                _ => String::new(),
            };
            let mut matches = self
                .db
                .flat_modules
                .iter()
                .filter(|module| match self.kind(**module) {
                    NodeKind::ModuleInst {
                        def_name: candidate,
                        ..
                    } => strip_lib(candidate) == def_name,
                    _ => false,
                })
                .flat_map(|module| self.node(*module).children.iter())
                .filter_map(|candidate| match self.kind(*candidate) {
                    NodeKind::EnumConst { value } if self.node(*candidate).name == name => {
                        Some((value.as_ref(), self.node(*candidate).name.as_str()))
                    }
                    _ => None,
                });
            if let Some((value, enum_name)) = matches.next() {
                if matches.next().is_none() {
                    return enum_value_expr(value, enum_name);
                }
            }
        }
        Err(format!(
            "cannot resolve expression reference `{name}` in `{scope_path}`"
        ))
    }

    /// A plain constant node (`ExprKind::Constant`); used where the old code
    /// called `read_const` directly on a handle.
    fn const_of_node(&self, node: NodeId) -> Result<IrConst, String> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Constant { value, size, .. }) => {
                let mut c = read_const_from(value, *size)?;
                let (signed, literal_width) = self.signed_based_literal_info(node);
                if signed {
                    if let Some(width) = literal_width {
                        if width < c.width {
                            c = read_const_from(value, width as i32)?;
                        }
                    }
                    c.signed = true;
                }
                Ok(c)
            }
            _ => Err("unsupported constant value format".to_string()),
        }
    }

    /// Lower one operation, mirroring the pre-IR emitter's operand shapes,
    /// result widths/signedness and error strings arm-for-arm.
    fn lower_operation(
        &mut self,
        scope_path: &str,
        otype: i32,
        reordered: bool,
        operands: &[NodeId],
    ) -> Result<IrExpr, String> {
        use vpi::*;
        macro_rules! op {
            ($i:expr) => {
                self.lower_expr(scope_path, operands[$i])?
            };
        }
        let maxw = |a: &IrExpr, b: &IrExpr| a.width.max(b.width);

        match otype {
            vpiAddOp => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Ok(real_bin_expr(IrRealBinOp::Add, a, b));
                }
                Ok(common_bin_expr(IrBinOp::Add, a, b))
            }
            vpiSubOp => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Ok(real_bin_expr(IrRealBinOp::Sub, a, b));
                }
                Ok(common_bin_expr(IrBinOp::Sub, a, b))
            }
            vpiMultOp => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Ok(real_bin_expr(IrRealBinOp::Mul, a, b));
                }
                Ok(common_bin_expr(IrBinOp::Mul, a, b))
            }
            vpiDivOp | vpiModOp | vpiPowerOp => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    let rop = match otype {
                        vpiDivOp => IrRealBinOp::Div,
                        vpiModOp => IrRealBinOp::Mod,
                        _ => IrRealBinOp::Pow,
                    };
                    return Ok(real_bin_expr(rop, a, b));
                }
                // The runtime returns all-X for div/mod/pow with operands
                // wider than 64 bits; reject before emission instead.
                if a.width > 64 || b.width > 64 {
                    return Err(format!(
                        "wide division/modulo/power not yet supported (operand \
                         wider than 64 bits) in `{scope_path}`"
                    ));
                }
                let f = match otype {
                    vpiDivOp => IrBinOp::Div,
                    vpiModOp => IrBinOp::Mod,
                    _ => IrBinOp::Pow,
                };
                if matches!(f, IrBinOp::Div | IrBinOp::Mod) {
                    Ok(common_bin_expr(f, a, b))
                } else {
                    let (width, signed) = (a.width, a.signed);
                    Ok(IrExpr::new(
                        IrExprKind::Bin {
                            op: f,
                            a: Box::new(a),
                            b: Box::new(b),
                        },
                        width,
                        signed,
                        None,
                    ))
                }
            }
            vpiBitAndOp => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "bitwise operation on real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(common_bin_expr(IrBinOp::BitAnd, a, b))
            }
            vpiBitOrOp => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "bitwise operation on real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(common_bin_expr(IrBinOp::BitOr, a, b))
            }
            vpiBitXorOp => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "bitwise operation on real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(common_bin_expr(IrBinOp::BitXor, a, b))
            }
            vpiBitXNorOp => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "bitwise operation on real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(common_bin_expr(IrBinOp::BitXNor, a, b))
            }
            vpiLogAndOp => {
                let a = op!(0);
                let b = op!(1);
                Ok(cmp_expr_ir(IrBinOp::LogAnd, a, b))
            }
            vpiLogOrOp => {
                let a = op!(0);
                let b = op!(1);
                Ok(cmp_expr_ir(IrBinOp::LogOr, a, b))
            }
            vpiEqOp => {
                let a = op!(0);
                let b = op!(1);
                Ok(cmp_expr_ir(IrBinOp::Eq, a, b))
            }
            vpiNeqOp => {
                let a = op!(0);
                let b = op!(1);
                Ok(cmp_expr_ir(IrBinOp::Neq, a, b))
            }
            vpiCaseEqOp => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "case equality on real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(cmp_expr_ir(IrBinOp::CaseEq, a, b))
            }
            vpiCaseNeqOp => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "case equality on real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(cmp_expr_ir(IrBinOp::CaseNeq, a, b))
            }
            vpiLtOp => {
                let a = op!(0);
                let b = op!(1);
                Ok(cmp_expr_ir(IrBinOp::Lt, a, b))
            }
            vpiLeOp => {
                let a = op!(0);
                let b = op!(1);
                Ok(cmp_expr_ir(IrBinOp::Le, a, b))
            }
            vpiGtOp => {
                let a = op!(0);
                let b = op!(1);
                Ok(cmp_expr_ir(IrBinOp::Gt, a, b))
            }
            vpiGeOp => {
                let a = op!(0);
                let b = op!(1);
                Ok(cmp_expr_ir(IrBinOp::Ge, a, b))
            }
            vpiLShiftOp | vpiRShiftOp | vpiArithLShiftOp | vpiArithRShiftOp => {
                let a = op!(0);
                let b = op!(1);
                if a.is_real() || b.is_real() {
                    return Err(format!(
                        "shift on real value in `{scope_path}` is not supported"
                    ));
                }
                let f = match otype {
                    vpiLShiftOp => IrBinOp::Shl,
                    vpiRShiftOp => IrBinOp::Shr,
                    vpiArithLShiftOp => IrBinOp::Ashl,
                    _ => IrBinOp::Ashr,
                };
                let (w, s) = (a.width, a.signed);
                Ok(IrExpr::new(
                    IrExprKind::Bin {
                        op: f,
                        a: Box::new(a),
                        b: Box::new(b),
                    },
                    w,
                    s,
                    None,
                ))
            }
            vpiConditionOp => {
                let sel = op!(0);
                let a = op!(1);
                let b = op!(2);
                let (w, s) = if a.is_real() || b.is_real() {
                    (REAL_EXPR_WIDTH, true)
                } else {
                    (maxw(&a, &b), a.signed && b.signed)
                };
                Ok(IrExpr::new(
                    IrExprKind::Mux {
                        sel: Box::new(sel),
                        a: Box::new(a),
                        b: Box::new(b),
                    },
                    w,
                    s,
                    None,
                ))
            }
            vpiMinusOp => {
                let a = op!(0);
                let w = a.width;
                if a.is_real() {
                    return Ok(real_un_expr(a));
                }
                // Unary minus of an unsized decimal literal (`-3`): Surelog
                // represents the literal as an unsigned 64-bit UInt constant,
                // dropping the LRM signedness (unsized decimal literals are
                // signed, LRM 5.7.1).  Restore it so `$display("%d", -3)`
                // prints "-3" instead of the unsigned wrap.  Sized radix
                // literals (`-4'h3`) stay unsigned per the LRM.
                let signed_lit = matches!(
                    self.kind(operands[0]),
                    NodeKind::Expr(ExprKind::Constant {
                        value: ValueData::UInt(_),
                        ..
                    })
                ) && !a.signed;
                let s = a.signed;
                let neg = IrExpr::new(
                    IrExprKind::Un {
                        op: IrUnOp::Neg,
                        a: Box::new(a),
                    },
                    w,
                    s && !signed_lit,
                    None,
                );
                if signed_lit {
                    Ok(IrExpr::resize_to(neg, w, true))
                } else {
                    Ok(neg)
                }
            }
            vpiPlusOp => {
                let a = op!(0);
                Ok(a)
            }
            vpiNotOp => {
                let a = op!(0);
                Ok(IrExpr::new(
                    IrExprKind::Un {
                        op: IrUnOp::LogNot,
                        a: Box::new(a),
                    },
                    1,
                    false,
                    None,
                ))
            }
            vpiBitNegOp => {
                let a = op!(0);
                if a.is_real() {
                    return Err(format!(
                        "bitwise negation of real value in `{scope_path}` is not supported"
                    ));
                }
                let w = a.width;
                let s = a.signed;
                Ok(IrExpr::new(
                    IrExprKind::Un {
                        op: IrUnOp::BitNeg,
                        a: Box::new(a),
                    },
                    w,
                    s,
                    None,
                ))
            }
            vpiUnaryAndOp => {
                let a = op!(0);
                if a.is_real() {
                    return Err(format!(
                        "reduction of real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(un_expr(IrUnOp::RedAnd, a))
            }
            vpiUnaryNandOp => {
                let a = op!(0);
                if a.is_real() {
                    return Err(format!(
                        "reduction of real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(un_expr(IrUnOp::RedNand, a))
            }
            vpiUnaryOrOp => {
                let a = op!(0);
                if a.is_real() {
                    return Err(format!(
                        "reduction of real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(un_expr(IrUnOp::RedOr, a))
            }
            vpiUnaryNorOp => {
                let a = op!(0);
                if a.is_real() {
                    return Err(format!(
                        "reduction of real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(un_expr(IrUnOp::RedNor, a))
            }
            vpiUnaryXorOp => {
                let a = op!(0);
                if a.is_real() {
                    return Err(format!(
                        "reduction of real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(un_expr(IrUnOp::RedXor, a))
            }
            vpiUnaryXNorOp => {
                let a = op!(0);
                if a.is_real() {
                    return Err(format!(
                        "reduction of real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(un_expr(IrUnOp::RedXNor, a))
            }
            vpiConcatOp => {
                let mut parts = Vec::new();
                for operand in operands {
                    parts.push(self.lower_expr(scope_path, *operand)?);
                }
                if reordered {
                    parts.reverse();
                }
                if parts.is_empty() {
                    return Err(format!("empty concatenation in `{scope_path}`"));
                }
                if parts.iter().any(|p| p.is_real()) {
                    return Err(format!(
                        "concatenation of real value in `{scope_path}` is not supported"
                    ));
                }
                let mut width = 0u32;
                for p in &parts {
                    width += p.width;
                }
                if width > LLG_MAX_WIDTH {
                    return Err(format!(
                        "concatenation in `{scope_path}` is {width} bits wide; \
                         the v1 runtime supports at most {LLG_MAX_WIDTH}"
                    ));
                }
                Ok(IrExpr::new(
                    IrExprKind::Concat { parts },
                    width,
                    false,
                    None,
                ))
            }
            vpiMultiConcatOp => {
                let count = {
                    let c = self.const_of_node(operands[0])?;
                    if c.x.iter().any(|&v| v != 0) || c.z.iter().any(|&v| v != 0) {
                        return Err(format!("unknown replication count in `{scope_path}`"));
                    }
                    c.bits.first().copied().unwrap_or(0)
                };
                let mut pat_parts = Vec::new();
                for operand in operands.iter().skip(1) {
                    pat_parts.push(self.lower_expr(scope_path, *operand)?);
                }
                if pat_parts.is_empty() {
                    return Err(format!("empty replication in `{scope_path}`"));
                }
                if pat_parts.iter().any(|p| p.is_real()) {
                    return Err(format!(
                        "replication of real value in `{scope_path}` is not supported"
                    ));
                }
                let mut pwidth = pat_parts[0].width as u128;
                for p in &pat_parts[1..] {
                    pwidth += p.width as u128;
                }
                let total = pwidth * count as u128;
                if total > LLG_MAX_WIDTH as u128 {
                    return Err(format!(
                        "replication in `{scope_path}` is {total} bits wide; \
                         the v1 runtime supports at most {LLG_MAX_WIDTH}"
                    ));
                }
                Ok(IrExpr::new(
                    IrExprKind::Replicate {
                        count,
                        parts: pat_parts,
                    },
                    total as u32,
                    false,
                    None,
                ))
            }
            vpiCastOp => Err(format!(
                "cast expressions are not supported in `{scope_path}` \
                 (the database does not capture the cast typespec)"
            )),
            vpiMinTypMaxOp => {
                let a = op!(0);
                Ok(a)
            }
            other => Err(format!(
                "unsupported operation op type {other} in `{scope_path}`"
            )),
        }
    }

    /// Lower system-function expressions ($clog2/$time/$stime/$bits/$signed/
    /// $unsigned); timescale scaling happens here.
    fn lower_sys_func_expr(
        &mut self,
        scope_path: &str,
        name: &str,
        call: NodeId,
    ) -> Result<IrExpr, String> {
        let args: Vec<NodeId> = self.node(call).children.clone();
        match name {
            "$clog2" => {
                let a = args
                    .first()
                    .copied()
                    .ok_or_else(|| format!("$clog2 without argument in `{scope_path}`"))?;
                let a = self.lower_expr(scope_path, a)?;
                if a.is_real() {
                    return Err(format!(
                        "clog2 on real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::Clog2(Box::new(a))),
                    32,
                    false,
                    None,
                ))
            }
            "$time" | "$stime" => {
                // Both functions return the current time in the CALLING
                // module's unit; `$stime` is the 32-bit form. `llg_time()` is
                // in design-precision ticks (1 tick = design_precision_ps
                // ps), so now_ps = now * P.
                let unit_ps = self.timescale_of_node(call).unit_ps;
                let width = if name == "$stime" { 32 } else { 64 };
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::Time {
                        precision_ps: self.design_precision_ps,
                        unit_ps,
                        width,
                    }),
                    width,
                    false,
                    None,
                ))
            }
            "$bits" => {
                let a = args
                    .first()
                    .copied()
                    .ok_or_else(|| format!("$bits without argument in `{scope_path}`"))?;
                let a = self.lower_expr(scope_path, a)?;
                if a.is_real() {
                    return Err(format!(
                        "bits on real value in `{scope_path}` is not supported"
                    ));
                }
                Ok(IrExpr::new(
                    IrExprKind::SysFunc(IrSysFunc::Bits(Box::new(a))),
                    32,
                    true,
                    None,
                ))
            }
            "$signed" | "$unsigned" => {
                let a = args
                    .first()
                    .copied()
                    .ok_or_else(|| format!("{name} without argument in `{scope_path}`"))?;
                let a = self.lower_expr(scope_path, a)?;
                if a.is_real() {
                    return Err(format!(
                        "{name} on real value in `{scope_path}` is not supported"
                    ));
                }
                let s = name == "$signed";
                let w = a.width;
                Ok(IrExpr::resize_to(a, w, s))
            }
            _ => Err(format!(
                "unsupported system function {name} in `{scope_path}`"
            )),
        }
    }

    /// Lower an assignment LHS: the pre-IR [`Self::analyze_lhs`] decisions
    /// converted to [`IrLhs`] (identical by construction during the seam
    /// transition; sub-expression codes ride along verbatim).
    fn lower_lhs(&mut self, path: &str, lhs: NodeId) -> Result<IrLhs, String> {
        let lh = self.analyze_lhs(path, lhs)?;
        Ok(self.lhs_to_ir(&lh))
    }

    /// Convert a pre-IR [`Lhs`] to its [`IrLhs`] form using the registered
    /// model indices.
    fn lhs_to_ir(&self, lh: &Lhs) -> IrLhs {
        match lh {
            Lhs::Whole(info) => IrLhs::Whole(info.ir),
            Lhs::WholeRef {
                addr,
                width,
                signed,
            } => IrLhs::WholeRef {
                addr: addr.clone(),
                width: *width,
                signed: *signed,
            },
            Lhs::Bit(info, ie) => IrLhs::Bit(info.ir, verbatim_code(ie)),
            Lhs::Part(info, left, right) => IrLhs::Part(info.ir, *left, *right),
            Lhs::IdxPart(info, be, we, neg) => {
                IrLhs::IdxPart(info.ir, verbatim_code(be), verbatim_code(we), *neg != 0)
            }
            Lhs::ArrayElem(ae) => IrLhs::ArrayElem {
                arr: ae.arr.ir,
                indices: ae.index_codes.iter().map(|c| verbatim_code(c)).collect(),
                elem_sel: match &ae.elem_sel {
                    ElemSel::Whole => IrElemSel::Whole,
                    ElemSel::Part(l, r) => IrElemSel::Part(*l, *r),
                    ElemSel::Bit(s) => IrElemSel::Bit(Box::new(verbatim_code(s))),
                },
            },
        }
    }
}

fn enum_value_expr(value: Option<&Val>, name: &str) -> Result<IrExpr, String> {
    match value {
        Some(Val::Bits(bits)) => {
            let constant = val_to_const(bits)?;
            Ok(IrExpr::new(
                IrExprKind::Const(constant.clone()),
                constant.width,
                constant.signed,
                None,
            ))
        }
        Some(Val::Real(value)) => Ok(real_literal_expr(*value)),
        Some(Val::Str(_)) => Err(format!(
            "string enum constant `{name}` used as a value is not supported"
        )),
        None => Err(format!("enum constant `{name}` has no value")),
    }
}

// ── Lowering helpers ──────────────────────────────────────────────────────────

/// Whether any lowered statement in the tree drives a real companion signal
/// (`llg_ba_d`/`llg_nba_d` in the emitted model).
fn assigns_to_real(stmts: &[IrStmt], model: &IrModel) -> bool {
    stmts.iter().any(|st| match st {
        IrStmt::Assign { lhs, .. } => matches!(
            lhs,
            IrLhs::Whole(idx) if matches!(model.signal(*idx).ty, IrType::Real { .. })
        ),
        other => assigns_to_real(nested_stmts(other), model),
    })
}

/// The directly nested statement lists of a compound statement (for
/// structural walks over lowered bodies).
pub(crate) fn nested_stmts(st: &IrStmt) -> &[IrStmt] {
    match st {
        IrStmt::Block(b)
        | IrStmt::If { then_: b, .. }
        | IrStmt::While { body: b, .. }
        | IrStmt::Repeat { body: b, .. }
        | IrStmt::Forever { body: b }
        | IrStmt::WaitCond { body: b, .. } => b,
        _ => &[],
    }
}

/// A real literal used as an expression (width 0, signed).
fn real_literal_expr(value: f64) -> IrExpr {
    IrExpr::new(
        IrExprKind::Const(IrConst {
            bits: vec![0],
            x: vec![0],
            z: vec![0],
            width: 0,
            signed: true,
            real: Some(value),
            fill: None,
        }),
        REAL_EXPR_WIDTH,
        true,
        None,
    )
}

/// A packed signal read by model index.
fn sig_read_expr(idx: usize) -> IrExpr {
    IrExpr::new(IrExprKind::SigRead(idx), 0, false, None)
}

/// A width-preserving bitwise negation (gate `not`/`nand`/`nor`/`xnor`).
fn bitneg_full_width(a: IrExpr) -> IrExpr {
    let w = a.width;
    let s = a.signed;
    IrExpr::new(
        IrExprKind::Un {
            op: IrUnOp::BitNeg,
            a: Box::new(a),
        },
        w,
        s,
        None,
    )
}

/// A constant with every bit set to 1 or 0 over `width` bits (pullup/
/// pulldown drivers).
fn const_bits_expr(width: u32, ones: bool) -> IrExpr {
    let nlimbs = (width as usize).div_ceil(64);
    let limb = if ones { u64::MAX } else { 0 };
    let mut bits = vec![limb; nlimbs];
    let tail = width as usize % 64;
    if tail != 0 && ones {
        bits[nlimbs - 1] = (1u64 << tail) - 1;
    }
    IrExpr::new(
        IrExprKind::Const(IrConst {
            bits,
            x: vec![0; nlimbs],
            z: vec![0; nlimbs],
            width,
            signed: false,
            real: None,
            fill: None,
        }),
        width,
        false,
        None,
    )
}

/// An all-Z constant over `width` bits (`sv4_mux`'s disabled branch of
/// enable gates).
fn const_z_expr(width: u32) -> IrExpr {
    let nlimbs = (width as usize).div_ceil(64);
    let mut z = vec![u64::MAX; nlimbs];
    let tail = width as usize % 64;
    if tail != 0 {
        z[nlimbs - 1] = (1u64 << tail) - 1;
    }
    IrExpr::new(
        IrExprKind::Const(IrConst {
            bits: vec![0; nlimbs],
            x: vec![0; nlimbs],
            z,
            width,
            signed: false,
            real: None,
            fill: None,
        }),
        width,
        false,
        None,
    )
}

/// A signal read with the signal's own width/signedness.
fn sig_read_expr_full(info: &SignalInfo) -> IrExpr {
    IrExpr::new(IrExprKind::SigRead(info.ir), info.width, info.signed, None)
}

/// Packed assignment width, when it is statically known.  This is expression
/// context (LRM 11.6.1), not the final RHS-to-LHS conversion performed by the
/// emitter after expression evaluation.
fn packed_lhs_width(model: &IrModel, lhs: &IrLhs) -> Option<u32> {
    let width = match lhs {
        IrLhs::Whole(idx) => model.signal(*idx).ty.width(),
        IrLhs::WholeRef { width, .. } => *width,
        IrLhs::Bit(..) => 1,
        IrLhs::Part(_, left, right) => ((left - right).abs() + 1) as u32,
        IrLhs::IdxPart(..) => return None,
        IrLhs::ArrayElem { arr, elem_sel, .. } => match elem_sel {
            IrElemSel::Whole => model.array(*arr).elem_width,
            IrElemSel::Part(left, right) => ((left - right).abs() + 1) as u32,
            IrElemSel::Bit(_) => 1,
        },
    };
    (width > 0).then_some(width)
}

fn apply_lhs_assignment_context(model: &IrModel, lhs: &IrLhs, rhs: IrExpr) -> IrExpr {
    if let Some(width) = packed_lhs_width(model, lhs) {
        apply_assignment_expression_width(rhs, width)
    } else {
        rhs
    }
}

fn is_context_binary(op: IrBinOp) -> bool {
    matches!(
        op,
        IrBinOp::Add
            | IrBinOp::Sub
            | IrBinOp::Mul
            | IrBinOp::Div
            | IrBinOp::Mod
            | IrBinOp::BitAnd
            | IrBinOp::BitOr
            | IrBinOp::BitXor
            | IrBinOp::BitXNor
    )
}

/// Include a wider assignment LHS in context-determined expression width. The
/// expression's signedness still comes only from its operands; the LHS type
/// affects the later assignment conversion, not expression signedness.
fn apply_assignment_expression_width(expr: IrExpr, width: u32) -> IrExpr {
    if expr.is_real() || width < expr.width {
        return expr;
    }
    let signed = expr.signed;
    match expr.kind {
        IrExprKind::Bin { op, a, b } if is_context_binary(op) => IrExpr::new(
            IrExprKind::Bin {
                op,
                a: Box::new(expression_operand_with_context(*a, width, signed)),
                b: Box::new(expression_operand_with_context(*b, width, signed)),
            },
            width,
            signed,
            None,
        ),
        IrExprKind::Bin {
            op: op @ (IrBinOp::Pow | IrBinOp::Shl | IrBinOp::Shr | IrBinOp::Ashl | IrBinOp::Ashr),
            a,
            b,
        } => IrExpr::new(
            IrExprKind::Bin {
                op,
                a: Box::new(expression_operand_with_context(*a, width, signed)),
                b,
            },
            width,
            signed,
            None,
        ),
        IrExprKind::Mux { sel, a, b } => IrExpr::new(
            IrExprKind::Mux {
                sel,
                a: Box::new(expression_operand_with_context(*a, width, signed)),
                b: Box::new(expression_operand_with_context(*b, width, signed)),
            },
            width,
            signed,
            None,
        ),
        IrExprKind::Un {
            op: op @ (IrUnOp::Neg | IrUnOp::BitNeg),
            a,
        } => IrExpr::new(
            IrExprKind::Un {
                op,
                a: Box::new(expression_operand_with_context(*a, width, signed)),
            },
            width,
            signed,
            None,
        ),
        kind => IrExpr::new(kind, expr.width, expr.signed, expr.fill),
    }
}

fn expression_operand_with_context(expr: IrExpr, width: u32, signed: bool) -> IrExpr {
    match expr.kind {
        IrExprKind::Bin { op, a, b } if is_context_binary(op) => IrExpr::new(
            IrExprKind::Bin {
                op,
                a: Box::new(expression_operand_with_context(*a, width, signed)),
                b: Box::new(expression_operand_with_context(*b, width, signed)),
            },
            width,
            signed,
            None,
        ),
        IrExprKind::Bin {
            op: op @ (IrBinOp::Pow | IrBinOp::Shl | IrBinOp::Shr | IrBinOp::Ashl | IrBinOp::Ashr),
            a,
            b,
        } => IrExpr::new(
            IrExprKind::Bin {
                op,
                a: Box::new(expression_operand_with_context(*a, width, signed)),
                b,
            },
            width,
            signed,
            None,
        ),
        IrExprKind::Mux { sel, a, b } => IrExpr::new(
            IrExprKind::Mux {
                sel,
                a: Box::new(expression_operand_with_context(*a, width, signed)),
                b: Box::new(expression_operand_with_context(*b, width, signed)),
            },
            width,
            signed,
            None,
        ),
        IrExprKind::Un {
            op: op @ (IrUnOp::Neg | IrUnOp::BitNeg),
            a,
        } => IrExpr::new(
            IrExprKind::Un {
                op,
                a: Box::new(expression_operand_with_context(*a, width, signed)),
            },
            width,
            signed,
            None,
        ),
        kind => IrExpr::resize_to(
            IrExpr::new(kind, expr.width, expr.signed, expr.fill),
            width,
            signed,
        ),
    }
}

/// A context-determined packed arithmetic/bitwise node. Any unsigned operand
/// makes the common expression type unsigned; the runtime coerces both
/// operands to `max(w)` before applying the operation.
fn common_bin_expr(op: IrBinOp, a: IrExpr, b: IrExpr) -> IrExpr {
    let (w, s) = (a.width.max(b.width), a.signed && b.signed);
    IrExpr::new(
        IrExprKind::Bin {
            op,
            a: Box::new(a),
            b: Box::new(b),
        },
        w,
        s,
        None,
    )
}

/// Internal binary node for already-shaped structural expressions.
fn bin_expr(op: IrBinOp, a: IrExpr, b: IrExpr) -> IrExpr {
    let (w, s) = (a.width.max(b.width), a.signed || b.signed);
    IrExpr::new(
        IrExprKind::Bin {
            op,
            a: Box::new(a),
            b: Box::new(b),
        },
        w,
        s,
        None,
    )
}

/// A comparison/equality node (1-bit unsigned result).
fn cmp_expr_ir(op: IrBinOp, a: IrExpr, b: IrExpr) -> IrExpr {
    IrExpr::new(
        IrExprKind::Bin {
            op,
            a: Box::new(a),
            b: Box::new(b),
        },
        1,
        false,
        None,
    )
}

/// A unary reduction/logical node (result shape per the pre-IR emitter).
fn un_expr(op: IrUnOp, a: IrExpr) -> IrExpr {
    IrExpr::new(IrExprKind::Un { op, a: Box::new(a) }, 1, false, None)
}

fn real_bin_expr(op: IrRealBinOp, a: IrExpr, b: IrExpr) -> IrExpr {
    IrExpr::new(
        IrExprKind::RealBin {
            op,
            a: Box::new(a),
            b: Box::new(b),
        },
        REAL_EXPR_WIDTH,
        true,
        None,
    )
}

fn real_un_expr(a: IrExpr) -> IrExpr {
    IrExpr::new(
        IrExprKind::RealUn {
            op: IrRealUnOp::Neg,
            a: Box::new(a),
        },
        REAL_EXPR_WIDTH,
        true,
        None,
    )
}

fn verbatim_code(code: &str) -> IrExpr {
    IrExpr::new(
        IrExprKind::Verbatim {
            code: code.to_string(),
            width: 32,
            signed: false,
        },
        32,
        false,
        None,
    )
}

fn is_signed_based_literal(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.windows(3).any(|window| {
        window[0] == b'\''
            && window[1].eq_ignore_ascii_case(&b's')
            && matches!(window[2].to_ascii_lowercase(), b'b' | b'o' | b'd' | b'h')
    })
}

fn based_literal_width(text: &str) -> Option<u32> {
    let apostrophe = text.find('\'')?;
    let digits: String = text[..apostrophe]
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

fn signed_based_literal_token_at(line: &str, col: u32) -> Option<&str> {
    let bytes = line.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    // UHDM columns are normally 1-based, but accepting the adjacent byte
    // also handles producers that point at the apostrophe or use 0-based
    // columns.
    let base = col.saturating_sub(1) as usize;
    for pos in [
        base,
        col as usize,
        base.saturating_sub(1),
        base.saturating_add(1),
    ] {
        if pos >= bytes.len() {
            continue;
        }
        let is_literal_char =
            |byte: u8| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'\'');
        let mut start = pos;
        while start > 0 && is_literal_char(bytes[start - 1]) {
            start -= 1;
        }
        let mut end = pos;
        while end < bytes.len() && is_literal_char(bytes[end]) {
            end += 1;
        }
        if start < end && is_signed_based_literal(&line[start..end]) {
            return Some(&line[start..end]);
        }
    }
    None
}

/// The IR form of a formal read; the backend spells the C shape (`a{idx}` or
/// `sv4_resize(*o{idx}, …)`) from the owning function's signature.
fn formal_read_expr(idx: usize, width: u32, signed: bool) -> IrExpr {
    IrExpr::new(IrExprKind::FormalRead(idx), width, signed, None)
}

/// Parse a depth-argument string (`"0"`, `"depth + 1"`, `"(…) + 1"` nests)
/// back into its structured form.
fn parse_depth(s: &str) -> IrDepth {
    let mut nest = 0u32;
    let mut cur = s;
    while let Some(rest) = cur.strip_prefix('(').and_then(|r| r.strip_suffix(") + 1")) {
        nest += 1;
        cur = rest;
    }
    IrDepth {
        func_base: cur == "depth + 1",
        nest,
    }
}

/// IR form of `expr_to_vector`: convert a lowered value to an explicit
/// `(width, signed)` vector target — real payloads through `sv4_from_real`
/// (at most 64 bits), unsized fills through `sv4_fill`, everything else
/// through the source-signedness-aware resize chain.
fn ir_to_vector(e: IrExpr, width: u32, signed: bool) -> Result<IrExpr, String> {
    if e.is_real() {
        if width > 64 {
            return Err(format!(
                "real-to-packed conversion target is {width} bits wide; v1 supports at most 64 bits"
            ));
        }
        Ok(IrExpr::new(
            IrExprKind::CastToPacked { a: Box::new(e) },
            width,
            signed,
            None,
        ))
    } else {
        Ok(match e.fill {
            Some(fill) => IrExpr::new(IrExprKind::Fill(fill), width, signed, Some(fill)),
            None => ir_arg_resize(e, width, signed),
        })
    }
}

/// IR form of a value-preserving conversion to a formal/return shape
/// (`sv4_cast`; extension keyed off the source's signedness, LRM §6.24.1 /
/// §10.7).
fn ir_arg_resize(e: IrExpr, width: u32, signed: bool) -> IrExpr {
    IrExpr::convert_to(e, width, signed)
}
