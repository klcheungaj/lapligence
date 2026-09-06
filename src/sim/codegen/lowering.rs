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
//! Packed-to-real conversion uses all model-sized limbs and treats X/Z bit
//! positions as zero; real-to-packed conversion rounds halves away from zero
//! and follows the generated model's packed capacity.
//!
//! Rejected with an `Err`: fork/join inside a function/task body, task calls
//! inside function bodies, recursive delay-bearing tasks, hierarchical
//! (cross-instance) function/task calls, string/class signals and string
//! parameters, unsupported real contexts (ports, arrays, function/task types,
//! continuous/combinational processes, and double-aware scheduling), widths at
//! or above the backend's exclusive generated-model capacity, and malformed
//! IR/value widths that exceed the runtime model capacity,
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
//! - Vectors use the generated model's packed capacity (strictly below the
//!   backend's exclusive `1 << 20` limit). Division/modulo/power preserve that
//!   model-sized limb width; backend and runtime checks reject malformed or
//!   over-capacity values defensively.
//! - Timescale is honored per file: `#N` delays scale by the calling module's
//!   time unit (`timescale unit/precision`, parsed from the first directive
//!   of the source file; modules without a directive default to 1ns/1ps with
//!   a TIMESCALEMOD-style warning), and `$time` returns the current time in
//!   the calling module's unit.  The scheduler runs in design-precision ticks
//!   (the finest precision across the design), so the runtime itself is
//!   timescale-agnostic. Fixed-point/unit-suffixed procedural delay literals
//!   first round to local module precision; real expressions and sub-ps
//!   scheduler precision remain unsupported.
//! - Unsized fill literals propagate through packed expression and case
//!   contexts; self-determined concatenation/replication operands stay one bit.
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
//!   blocking — v1 approximation). Event/repeat-controlled forms are rejected;
//!   bounded integer parameter expressions and fixed-point/time literals work.
//!   Continuous-assignment delays
//!   (`assign #N lhs = rhs;`) delay every write by N after the triggering
//!   rhs change, including at t=0; there is no pulse filtering — each wake
//!   writes the CURRENT rhs value D later (warned).

#![allow(non_upper_case_globals)]

use std::collections::{HashMap, HashSet};

use super::timescale::{
    eval_procedural_delay, parse_timescale, time_literal_to_real, DelayParameter, DelayValue,
    Timescale,
};
use super::CodegenError;
use crate::core::db::{
    AggregateKind, AggregateMember, ArrayKind, AssignmentPatternKeyType, AssociativeIndex,
    CaseKind as DbCaseKind, ConstantSource, ConstantType, Db, Direction as DbDirection, EventSpec,
    ExprKind, IntraControl, JoinKind as DbJoinKind, NetType, NodeId, NodeKind, Operation,
    PackedMember, PrimClass, PrimitiveType, ProcessKind, StmtKind, Strength,
};
use crate::core::elab::{self, Bit, Val};
use crate::ffi::vpi::{self, ValueData, VpiHandle};
use crate::sim::emit_c::{
    escaped_char, event_global_name, global_name, ident, real_global_name, render_expr, strip_lib,
    RCtx, LLG_MAX_WIDTH,
};
use crate::sim::ir::{
    IrAssocKey, IrAssocTraversal, IrBinOp, IrBitQuery, IrCall, IrCallArg, IrCallExpr, IrCaseItem,
    IrCaseKind, IrChandleExpr, IrConst, IrContainer, IrContainerExpr, IrContainerKind,
    IrContainerStmt, IrDepth, IrEdge, IrElemSel, IrEvent, IrExpr, IrExprKind, IrFormal,
    IrInsideItem, IrJoinKind, IrLhs, IrModel, IrProcess, IrRealBinOp, IrRealUnOp, IrShape,
    IrSignal, IrStmt, IrStreamDirection, IrSysFunc, IrTimeKind, IrType, IrUnOp, IrWaitSrc,
    LLG_MAX_NET_DRIVERS,
};

mod collection;
mod containers;
mod expressions;
mod objects;
mod statements;

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
///
/// Source-dependent time-literal values require an explicitly admitted
/// [`Db::build_with_source_files`] snapshot passed to
/// [`generate_from_db_with_opts`]; this bare-handle entry point does not admit
/// new constant-source reads.
pub fn generate(design: VpiHandle) -> Result<GeneratedModel, CodegenError> {
    generate_with_opts(design, &crate::sim::opt::OptConfig::default())
}

/// Lower the elaborated design into C11 with an explicit optimization
/// configuration.  See [`generate`] for the calling contract.
pub fn generate_with_opts(
    design: VpiHandle,
    cfg: &crate::sim::opt::OptConfig,
) -> Result<GeneratedModel, CodegenError> {
    let db = Db::build(design).map_err(|error| CodegenError::new(error.to_string()))?;
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
) -> Result<GeneratedModel, CodegenError> {
    generate_from_db_with_opts_impl(db, cfg).map_err(CodegenError::new)
}

fn generate_from_db_with_opts_impl(
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
    let mut model = std::mem::replace(
        &mut cg.model,
        IrModel::new(String::new(), Timescale::DEFAULT.precision_ps)
            .expect("the default timescale has non-zero precision"),
    );
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
    model.validate().map_err(|error| error.to_string())?;
    crate::sim::opt::run(&mut model, cfg);
    model.validate().map_err(|error| error.to_string())?;
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
    two_state: bool,
    real: bool,
    shortreal: bool,
    /// For members of a collapsed inout-net group: `(net C name, driver
    /// slot)`.  `global` is then `<net>.resolved` so every read goes through
    /// the resolution cell; whole-signal writes lower to `llg_net_write`.
    net_driver: Option<(String, usize)>,
    /// Index into [`Codegen::model`].signals.
    ir: usize,
}

#[derive(Clone)]
struct ProcLocalInfo {
    c_name: String,
    width: u32,
    signed: bool,
    two_state: bool,
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

#[derive(Clone)]
struct ContainerInfo {
    ir: usize,
}

#[derive(Clone)]
struct AggregateMemberInfo {
    member: AggregateMember,
    signal: SignalInfo,
}

#[derive(Clone)]
struct UnpackedAggregateInfo {
    kind: AggregateKind,
    type_identity: Option<String>,
    members: Vec<AggregateMemberInfo>,
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
    Bit(IrExpr),
}

/// LHS of an assignment to one array element (with optional element-level
/// bit/part select).
struct ArrayElemLhs {
    arr: ArrayInfo,
    /// One typed expression per dimension index, in declaration order.
    indices: Vec<IrExpr>,
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
    two_state: bool,
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
    UnsupportedNet(NetType),
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
    /// Persistent storage for every formal of static subroutines,
    /// keyed by (owning instance, formal declaration).
    static_formals: HashMap<(NodeId, NodeId), SignalInfo>,
    /// Native pointer storage for static chandle formals.
    static_chandle_formals: HashMap<(NodeId, NodeId), usize>,
    /// Persistent storage for locals of static delay-bearing tasks. Those
    /// tasks are inlined, so their storage must live outside each call site.
    static_task_locals: HashMap<(NodeId, NodeId), SignalInfo>,
    /// All lowered signals, in collection order (deterministic emission).
    signals: Vec<SignalInfo>,
    /// Net/Var arena node → lowered signal info (all instances + gen scopes).
    sig_globals: HashMap<NodeId, SignalInfo>,
    object_globals: HashMap<NodeId, usize>,
    scope_object_names: HashMap<String, HashMap<String, usize>>,
    /// Top-level unpacked aggregate variables lowered to member storage.
    unpacked_aggregates: HashMap<NodeId, UnpackedAggregateInfo>,
    /// Inline procedural declaration node → lexical C local information.
    proc_locals: HashMap<NodeId, ProcLocalInfo>,
    /// Legacy storage for scalar declaration-initializer fills that need a
    /// collapsed-net driver slot. True-net declarations now lower as
    /// continuous processes, so ordinary wire/tri entries do not use it.
    net_inits: Vec<(String, usize, IrConst)>,
    /// All lowered arrays, in collection order (deterministic emission).
    arrays: Vec<ArrayInfo>,
    /// Array arena node → lowered array info.
    array_globals: HashMap<NodeId, ArrayInfo>,
    /// Dynamic arrays, queues, and associative arrays use owned runtime
    /// storage and never alias fixed unpacked-array storage.
    container_globals: HashMap<NodeId, ContainerInfo>,
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
    /// Whole-net continuous assignment node -> synthetic signal index carrying
    /// that wired net driver's distinct runtime slot.
    wired_driver_sites: HashMap<NodeId, usize>,
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
            model: IrModel::new(String::new(), Timescale::DEFAULT.precision_ps)
                .expect("the default timescale has non-zero precision"),
            cur_fn_ir: None,
            func_meta: HashMap::new(),
            static_formals: HashMap::new(),
            static_chandle_formals: HashMap::new(),
            static_task_locals: HashMap::new(),
            signals: Vec::new(),
            sig_globals: HashMap::new(),
            object_globals: HashMap::new(),
            scope_object_names: HashMap::new(),
            unpacked_aggregates: HashMap::new(),
            proc_locals: HashMap::new(),
            net_inits: Vec::new(),
            arrays: Vec::new(),
            array_globals: HashMap::new(),
            container_globals: HashMap::new(),
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
            wired_driver_sites: HashMap::new(),
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

    /// Fold a procedural delay recovered from source text. Identifier lookup
    /// follows the owned parent chain so generate-local parameters shadow
    /// parameters in their enclosing module instance.
    fn procedural_delay_ticks(
        &mut self,
        delay_node: NodeId,
        expression: &str,
    ) -> Result<u64, String> {
        let timescale = self.timescale_of_node(delay_node);
        let delay = eval_procedural_delay(expression, timescale, |name| {
            let mut scope = Some(delay_node);
            while let Some(node_id) = scope {
                for child in &self.node(node_id).children {
                    if self.node(*child).name != name {
                        continue;
                    }
                    if let Some(Val::Real(value)) = self.param_vals.get(child) {
                        return Some(DelayParameter::Real(*value));
                    }
                    if let Some(Val::Bits(value)) = self.param_vals.get(child) {
                        let (declared_width, declared_signed) = match self.kind(*child) {
                            NodeKind::Param { ty, .. } => (ty.width, Some(ty.signed)),
                            _ => (None, None),
                        };
                        let width = declared_width.or_else(|| u32::try_from(value.width()).ok())?;
                        return value
                            .to_u128()
                            .and_then(|raw| {
                                DelayValue::from_raw(
                                    raw,
                                    width,
                                    declared_signed.unwrap_or(value.signed),
                                )
                            })
                            .map(DelayParameter::Integer);
                    }
                    // A nearer nonconstant declaration shadows outer parameters.
                    if matches!(
                        self.kind(*child),
                        NodeKind::Var { .. }
                            | NodeKind::Net { .. }
                            | NodeKind::Param { .. }
                            | NodeKind::FuncArg { .. }
                            | NodeKind::Array { .. }
                            | NodeKind::Port { .. }
                    ) {
                        return None;
                    }
                }
                scope = self.node(node_id).parent;
            }
            None
        })
        .map_err(|error| {
            format!(
                "cannot evaluate procedural `#({expression})` in `{}`: {error}",
                self.instance_path_of(self.inst)
            )
        })?;
        let (ticks, unit_ps) = delay.ticks_and_unit_ps(timescale);
        scale_delay_ticks(
            ticks,
            unit_ps,
            self.design_precision_ps,
            &self.instance_path_of(self.inst),
        )
    }

    /// Resolve a hierarchical reference read (`a.b.sig`, or the 2-part
    /// interface member `m.data`) to its signal, when the LAST path element
    /// resolves to a captured Net/Var (per-instance, via the db's refs).
    /// Longer or unresolvable paths return `None`.
    fn hier_path_signal(&self, node: NodeId) -> Option<&SignalInfo> {
        if let NodeKind::Expr(ExprKind::HierPath { parts, refs }) = self.kind(node) {
            if let Some(t) = refs.last().copied().flatten() {
                return self.signal_of(t);
            }
            let (target, base_index) = self.hier_path_signal_target(parts, refs)?;
            if base_index + 1 == parts.len() {
                return self.signal_of(target);
            }
        }
        None
    }

    /// Recover a signal target when Surelog omits every `vpiActual` reference
    /// from a generated-scope hierarchical path. Scope/name lookup remains on
    /// the already-collected owned model and requires an exact scope prefix.
    fn hier_path_signal_target(
        &self,
        parts: &[String],
        refs: &[Option<NodeId>],
    ) -> Option<(NodeId, usize)> {
        if let Some((index, target)) = refs
            .iter()
            .enumerate()
            .find_map(|(index, target)| target.map(|target| (index, target)))
        {
            if self.signal_of(target).is_some() {
                return Some((target, index));
            }
        }
        for base_index in (0..parts.len()).rev() {
            let mut scope = self.design_name.clone();
            if base_index != 0 {
                scope.push('.');
                scope.push_str(&parts[..base_index].join("."));
            }
            let Some(info) = self
                .scope_sig_names
                .get(&scope)
                .and_then(|names| names.get(&parts[base_index]))
            else {
                continue;
            };
            if let Some(target) = self
                .sig_globals
                .iter()
                .find_map(|(target, candidate)| (candidate.ir == info.ir).then_some(*target))
            {
                return Some((target, base_index));
            }
        }
        None
    }

    fn packed_member_info(&self, node: NodeId) -> Option<(SignalInfo, PackedMember)> {
        let NodeKind::Expr(ExprKind::HierPath { parts, refs }) = self.kind(node) else {
            return None;
        };
        let (target, base_index) = self.hier_path_signal_target(parts, refs)?;
        let info = self.signal_of(target)?.clone();
        let mut layout = self.db.aggregate_layout(target)?;
        if !matches!(
            layout.kind,
            AggregateKind::PackedStruct | AggregateKind::PackedUnion
        ) {
            return None;
        }
        let mut absolute_lsb = 0u32;
        let mut selected: Option<&AggregateMember> = None;
        for (part_index, member_name) in parts.iter().enumerate().skip(base_index + 1) {
            let index = layout
                .members
                .iter()
                .position(|member| member.name == *member_name)?;
            let member = &layout.members[index];
            let relative_lsb = if layout.kind == AggregateKind::PackedUnion {
                0
            } else {
                layout.members[index + 1..]
                    .iter()
                    .try_fold(0u32, |offset, following| {
                        offset.checked_add(following.ty.width?)
                    })?
            };
            absolute_lsb = absolute_lsb.checked_add(relative_lsb)?;
            selected = Some(member);
            match member.aggregate.as_deref() {
                Some(nested)
                    if matches!(
                        nested.kind,
                        AggregateKind::PackedStruct | AggregateKind::PackedUnion
                    ) =>
                {
                    layout = nested;
                }
                _ if part_index + 1 == parts.len() => break,
                _ => return None,
            }
        }
        let member = selected?;
        Some((
            info,
            PackedMember {
                name: member.name.clone(),
                lsb: absolute_lsb,
                width: member.ty.width?,
                signed: member.ty.signed,
                two_state: member.two_state,
                packed_ranges: member.packed_ranges.clone(),
            },
        ))
    }

    fn unpacked_aggregate_target(&self, node: NodeId) -> Option<NodeId> {
        match self.kind(node) {
            NodeKind::Var { .. } => self.unpacked_aggregates.contains_key(&node).then_some(node),
            NodeKind::Expr(ExprKind::Ref { target }) => {
                target.filter(|target| self.unpacked_aggregates.contains_key(target))
            }
            NodeKind::Expr(ExprKind::HierPath { parts, refs }) if parts.len() == 1 => refs
                .first()
                .copied()
                .flatten()
                .filter(|target| self.unpacked_aggregates.contains_key(target)),
            _ => None,
        }
    }

    fn unpacked_aggregate_info(&self, node: NodeId) -> Option<(NodeId, UnpackedAggregateInfo)> {
        let target = self.unpacked_aggregate_target(node)?;
        self.unpacked_aggregates
            .get(&target)
            .cloned()
            .map(|aggregate| (target, aggregate))
    }

    fn unpacked_member_info(
        &self,
        node: NodeId,
    ) -> Option<(NodeId, AggregateKind, AggregateMemberInfo)> {
        let NodeKind::Expr(ExprKind::HierPath { parts, refs }) = self.kind(node) else {
            return None;
        };
        let (target, base_index) = if let Some(target) = refs.first().copied().flatten() {
            (target, 0)
        } else {
            let mut found = None;
            for base_index in (0..parts.len()).rev() {
                let mut scope = self.design_name.clone();
                if base_index != 0 {
                    scope.push('.');
                    scope.push_str(&parts[..base_index].join("."));
                }
                found = self.unpacked_aggregates.keys().find_map(|target| {
                    (self.node(*target).name == parts[base_index]
                        && self.instance_path_of(*target) == scope)
                        .then_some((*target, base_index))
                });
                if found.is_some() {
                    break;
                }
            }
            found?
        };
        let aggregate = self.unpacked_aggregates.get(&target)?;
        let member_name = parts.get(base_index + 1)?;
        let member = aggregate
            .members
            .iter()
            .find(|member| member.member.name == *member_name)?
            .clone();
        Some((target, aggregate.kind, member))
    }

    fn packed_member_select(&self, node: NodeId) -> Result<Option<PackedMemberSelect>, String> {
        let name = &self.node(node).name;
        let Some(select) = name
            .strip_suffix(']')
            .and_then(|prefix| prefix.rfind('[').map(|open| &prefix[open + 1..]))
        else {
            return Ok(None);
        };
        if let Some((left, right)) = select.split_once(':') {
            return Ok(Some(PackedMemberSelect::Part(
                self.packed_member_bound(left)?,
                self.packed_member_bound(right)?,
            )));
        }
        Ok(Some(PackedMemberSelect::Bit(
            self.packed_member_bound(select)?,
        )))
    }

    fn packed_member_bound(&self, index: &str) -> Result<i128, String> {
        let parameter_value = |parameter: &str| {
            self.param_vals.iter().find_map(|(node, value)| {
                (self.node(*node).name == parameter)
                    .then_some(value)
                    .and_then(|value| match value {
                        Val::Bits(value) if !value.is_unknown() => value.to_i128(),
                        _ => None,
                    })
            })
        };
        let term = |text: &str| {
            let text = text.trim().replace('_', "");
            text.parse::<i128>().ok().or_else(|| parameter_value(&text))
        };
        for operator in ['+', '-'] {
            let split = index
                .char_indices()
                .skip(1)
                .find(|(_, ch)| *ch == operator)
                .map(|(at, _)| (&index[..at], &index[at + 1..]));
            if let Some((left, right)) = split {
                if let (Some(left), Some(right)) = (term(left), term(right)) {
                    let value = if operator == '+' {
                        left.checked_add(right)
                    } else {
                        left.checked_sub(right)
                    }
                    .ok_or_else(|| format!("packed-member index `{index}` overflows"))?;
                    return Ok(value);
                }
            }
        }
        if let Some(value) = term(index) {
            return Ok(value);
        }
        let delay = eval_procedural_delay(index, Timescale::DEFAULT, |parameter| {
            self.param_vals.iter().find_map(|(node, value)| {
                if self.node(*node).name != parameter {
                    return None;
                }
                let Val::Bits(value) = value else {
                    return None;
                };
                let width = u32::try_from(value.width()).ok()?;
                value
                    .to_u128()
                    .and_then(|raw| DelayValue::from_raw(raw, width, value.signed))
                    .map(DelayParameter::Integer)
            })
        })
        .map_err(|error| format!("packed-member index `{index}`: {error}"))?;
        let (index, _) = delay.ticks_and_unit_ps(Timescale::DEFAULT);
        Ok(i128::from(index))
    }

    fn aggregate_member_relative_bound(
        &self,
        member_name: &str,
        packed_ranges: &[crate::core::db::PackedRange],
        bound: i128,
    ) -> Result<u32, String> {
        let (left, right) = match packed_ranges {
            [range] => (range.left, range.right),
            [] => {
                return Err(format!(
                    "packed-member `{}` has no captured packed range",
                    member_name
                ));
            }
            _ => {
                return Err(format!(
                    "select on multidimensional packed member `{}` is not supported",
                    member_name
                ));
            }
        };
        if bound < left.min(right) || bound > left.max(right) {
            return Err(format!(
                "packed-member select bound {bound} is outside `{}` range [{left}:{right}]",
                member_name
            ));
        }
        let relative = if left >= right {
            bound.checked_sub(right)
        } else {
            right.checked_sub(bound)
        }
        .ok_or_else(|| format!("packed-member select bound {bound} overflows"))?;
        u32::try_from(relative)
            .map_err(|_| format!("packed-member select bound {bound} does not fit in u32"))
    }

    /// Resolve a select on a multidimensional packed declaration to its
    /// corresponding slice in the flattened runtime vector.
    fn packed_select_info(
        &self,
        base: NodeId,
        indices: &[NodeId],
    ) -> Result<Option<(SignalInfo, u32, u32)>, String> {
        let target = match self.kind(base) {
            NodeKind::Net { .. } | NodeKind::Var { .. } => Some(base),
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => refs.last().copied().flatten(),
            _ => None,
        };
        let Some(target) = target else {
            return Ok(None);
        };
        let Some(dimensions) = self.db.packed_dimensions(target) else {
            return Ok(None);
        };
        if dimensions.len() < 2 || indices.is_empty() || indices.len() > dimensions.len() {
            return Ok(None);
        }
        let info = self
            .signal_of(target)
            .cloned()
            .ok_or_else(|| "packed select target is not a signal".to_string())?;
        let mut remaining = dimensions
            .iter()
            .try_fold(1u128, |width, range| {
                range
                    .left
                    .abs_diff(range.right)
                    .checked_add(1)
                    .and_then(|extent| width.checked_mul(extent))
            })
            .ok_or_else(|| "packed select width overflows".to_string())?;
        let mut lsb = 0u128;
        for (range, index_node) in dimensions.iter().zip(indices) {
            let extent = range
                .left
                .abs_diff(range.right)
                .checked_add(1)
                .ok_or_else(|| "packed select dimension overflows".to_string())?;
            let index = self.eval_bound_i128(*index_node)?;
            let low = range.left.min(range.right);
            let high = range.left.max(range.right);
            if !(low..=high).contains(&index) {
                return Err(format!(
                    "packed select index {index} is outside [{low}:{high}]"
                ));
            }
            remaining /= extent;
            let slot = if range.left >= range.right {
                index - range.right
            } else {
                range.right - index
            };
            let slot =
                u128::try_from(slot).map_err(|_| "packed select offset is negative".to_string())?;
            lsb = lsb
                .checked_add(
                    slot.checked_mul(remaining)
                        .ok_or_else(|| "packed select offset overflows".to_string())?,
                )
                .ok_or_else(|| "packed select offset overflows".to_string())?;
        }
        let lsb = u32::try_from(lsb)
            .map_err(|_| "packed select offset does not fit in u32".to_string())?;
        let width = u32::try_from(remaining)
            .map_err(|_| "packed select width does not fit in u32".to_string())?;
        Ok(Some((info, lsb, width)))
    }

    /// Recover a parameterized function return range from its declaration.
    /// Surelog v1.86 can retain the module's default parameter value on the
    /// return object even when the enclosing instance overrides it.
    fn declared_source_width(&self, declaration: NodeId, inst: NodeId) -> Option<u32> {
        let node = self.node(declaration);
        let file = node.file.as_deref()?;
        let source = std::fs::read_to_string(file).ok()?;
        let line = source.lines().nth(node.line.checked_sub(1)? as usize)?;
        let range = line.split_once('[')?.1.split_once(']')?.0;
        let (left, right) = range.split_once(':')?;
        let owning_module = |mut node: NodeId| loop {
            if matches!(self.kind(node), NodeKind::ModuleInst { .. }) {
                break Some(node);
            }
            node = self.node(node).parent()?;
        };
        let function_module = owning_module(inst)?;
        let term = |text: &str| {
            let text = text.trim().replace('_', "");
            text.parse::<u128>().ok().or_else(|| {
                self.param_vals.iter().find_map(|(node, value)| {
                    if self.node(*node).name != text
                        || owning_module(*node) != Some(function_module)
                    {
                        return None;
                    }
                    let Val::Bits(value) = value else {
                        return None;
                    };
                    (!value.is_unknown()).then(|| value.to_u128()).flatten()
                })
            })
        };
        let evaluate = |expression: &str| {
            for operator in ['+', '-'] {
                if let Some((left, right)) = expression.split_once(operator) {
                    let (left, right) = (term(left)?, term(right)?);
                    return if operator == '+' {
                        left.checked_add(right)
                    } else {
                        left.checked_sub(right)
                    };
                }
            }
            term(expression)
        };
        let left = evaluate(left)?;
        let right = evaluate(right)?;
        u32::try_from(left.abs_diff(right).checked_add(1)?).ok()
    }

    fn effective_decl_width(&self, declaration: NodeId, inst: NodeId, captured: u32) -> u32 {
        self.declared_source_width(declaration, inst)
            .unwrap_or(captured)
    }

    fn source_size_cast_width(&self, expression: &str) -> Option<u32> {
        let token = expression.trim();
        let value = token.replace('_', "").parse::<u128>().ok().or_else(|| {
            self.param_vals.iter().find_map(|(node, value)| {
                if self.node(*node).name != token {
                    return None;
                }
                let Val::Bits(value) = value else {
                    return None;
                };
                (!value.is_unknown()).then(|| value.to_u128()).flatten()
            })
        })?;
        u32::try_from(value).ok().filter(|width| *width != 0)
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

    /// Recover an unbased unsized fill literal from its source spelling.
    /// Surelog can constant-fold a fill used below an operator or in a case
    /// item into an ordinary sized 0/1/X/Z constant, losing `vpiSize == -1`.
    fn source_fill_literal(&self, node: NodeId) -> Option<u8> {
        let node = self.node(node);
        if let Some(fill) = fill_literal_token(&node.name) {
            return Some(fill);
        }
        // Folded compound constants can inherit the compound expression's
        // starting column. Only a two-character source span identifies the
        // constant itself as the fill token.
        if node.end_line != node.line || node.end_col != node.col.saturating_add(2) {
            return None;
        }
        let (file, line) = (node.file.as_deref()?, node.line);
        if line == 0 {
            return None;
        }
        let content = std::fs::read_to_string(file).ok()?;
        let text = content.lines().nth(line as usize - 1)?;
        fill_literal_token_at(text, node.col)
    }

    /// Parse the timescale of every distinct source file referenced by the
    /// design and fix the design precision (the finest precision, which sets
    /// the scheduler tick unit).  Runs before any emission so every `#delay`
    /// and `$time` scales consistently.
    fn collect_timescales(&mut self) {
        self.design_precision_ps = u64::MAX;
        for top in self.db.tops() {
            self.walk_files(*top);
        }
        for m in self.db.flat_modules() {
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

/// SystemVerilog two-state integral types (IEEE 1800-2009 Table 6-8).
fn is_two_state_kind(kind: &str) -> bool {
    matches!(kind, "bit" | "byte" | "shortint" | "int" | "longint")
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

fn checked_select_bounds(
    left: i128,
    right: i128,
    context: &str,
) -> Result<(i64, i64, u32), String> {
    let left =
        i64::try_from(left).map_err(|_| format!("{context} left bound does not fit in 64 bits"))?;
    let right = i64::try_from(right)
        .map_err(|_| format!("{context} right bound does not fit in 64 bits"))?;
    let width = left
        .abs_diff(right)
        .checked_add(1)
        .ok_or_else(|| format!("{context} width overflow"))?;
    if width > u64::from(LLG_MAX_WIDTH) {
        return Err(format!(
            "{context} is too wide ({width} bits; max {LLG_MAX_WIDTH})"
        ));
    }
    Ok((left, right, width as u32))
}

// ── Constant reading ──────────────────────────────────────────────────────────

/// A lowered constant value: see [`crate::sim::ir::IrConst`].  X and Z bits
/// live in separate `x`/`z` limb arrays (x & z == 0), matching the runtime's
/// 4-state split.
///
/// Convert a captured constant (`ValueData` + `vpiSize`) to an [`IrConst`].
fn read_const_from(vd: &ValueData, size: i32) -> Result<IrConst, String> {
    match val_from_value_data(vd, size)? {
        Val::Bits(value) => val_to_const(&value),
        Val::Real(value) => Ok(IrConst {
            bits: vec![0],
            x: vec![0],
            z: vec![0],
            width: 0,
            signed: false,
            real: Some(value),
            fill: None,
        }),
        Val::Str(value) => string_to_const(&value),
    }
}

/// Convert a Verilog string constant to its packed, unsigned byte value.
/// The leftmost source character occupies the most-significant byte, as
/// required when a string is used as an integral expression.
fn string_to_const(value: &str) -> Result<IrConst, String> {
    val_to_const(&string_to_value(value)?)
}

fn string_to_value(value: &str) -> Result<elab::Value, String> {
    let mut decoded = decode_verilog_string(value)?;
    // An empty packed string has the same 8-bit zero representation used by
    // established Verilog simulators.
    if decoded.is_empty() {
        decoded.push(0);
    }
    let width = decoded
        .len()
        .checked_mul(8)
        .ok_or_else(|| "string constant width overflow".to_string())?;
    if width > LLG_MAX_WIDTH as usize {
        return Err(format!(
            "string constant is too wide ({width} bits; max {LLG_MAX_WIDTH})"
        ));
    }

    let mut bits = Vec::with_capacity(width);
    for byte in decoded {
        for bit in (0..8).rev() {
            bits.push(if byte & (1 << bit) != 0 {
                Bit::One
            } else {
                Bit::Zero
            });
        }
    }
    Ok(elab::Value::from_bits(bits, false))
}

/// Decode the escape spellings retained in Surelog's owned string payload.
fn decode_verilog_string(value: &str) -> Result<Vec<u8>, String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'\\' {
            decoded.push(bytes[i]);
            i += 1;
            continue;
        }
        i += 1;
        let escape = *bytes
            .get(i)
            .ok_or_else(|| "string constant ends with an incomplete escape".to_string())?;
        match escape {
            b'n' => {
                decoded.push(b'\n');
                i += 1;
            }
            b't' => {
                decoded.push(b'\t');
                i += 1;
            }
            b'v' => {
                decoded.push(0x0b);
                i += 1;
            }
            b'f' => {
                decoded.push(0x0c);
                i += 1;
            }
            b'a' => {
                decoded.push(0x07);
                i += 1;
            }
            b'\\' | b'"' => {
                decoded.push(escape);
                i += 1;
            }
            b'0'..=b'7' => {
                let mut value = 0u16;
                let mut digits = 0;
                while digits < 3 && i < bytes.len() && matches!(bytes[i], b'0'..=b'7') {
                    value = (value << 3) | u16::from(bytes[i] - b'0');
                    digits += 1;
                    i += 1;
                }
                decoded.push(value as u8);
            }
            b'x' => {
                let first = bytes.get(i + 1).and_then(|b| hex_digit(*b));
                let second = bytes.get(i + 2).and_then(|b| hex_digit(*b));
                let (Some(first), Some(second)) = (first, second) else {
                    return Err("hex string escape requires two digits".to_string());
                };
                decoded.push((first << 4) | second);
                i += 3;
            }
            _ => {
                return Err(format!(
                    "unsupported string escape `\\{}`",
                    char::from(escape)
                ));
            }
        }
    }
    Ok(decoded)
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
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
        fill: v.fill.map(|bit| match bit {
            Bit::Zero => 0,
            Bit::One => 1,
            Bit::X => 2,
            Bit::Z => 3,
        }),
    })
}

fn decl_value_to_const(value: Val) -> Result<IrConst, String> {
    match value {
        Val::Bits(value) => val_to_const(&value),
        Val::Real(value) => Ok(IrConst {
            bits: vec![0],
            x: vec![0],
            z: vec![0],
            width: 0,
            signed: true,
            real: Some(value),
            fill: None,
        }),
        Val::Str(value) => string_to_const(&value),
    }
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

// ── Shared lowering helpers and statement state ──────────────────────────────

/// Convert a captured constant (`ValueData` + `vpiSize`) to a `Val`, mirroring
/// `elab::read_value` without a VPI handle.
fn val_from_value_data(vd: &ValueData, size: i32) -> Result<Val, String> {
    elab::decode_value_data(vd, size).map_err(|error| error.to_string())
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
    two_state: bool,
    node: Option<NodeId>,
}

/// Call-site resolution metadata of an emitted function/task: its model
/// index (for `CallFn` nodes) plus the signature pieces lowering needs.
#[derive(Clone)]
struct FuncMeta {
    ir: usize,
    is_task: bool,
    ret: Option<(u32, bool, bool)>,
    ret_chandle: bool,
    ret_string: bool,
    formals: Vec<(NodeId, bool)>,
}

/// How to read a formal argument (or local) in the lowered IR: width and
/// signedness of the storage (the read expression itself is carried by
/// [`FuncCtx::arg_ir`]).
#[derive(Clone)]
struct ArgMap {
    width: u32,
    signed: bool,
    two_state: bool,
}

#[derive(Clone)]
enum ChandleTarget {
    Object(usize),
    Local(String),
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
    /// Static formal/local arena node → persistent model storage. Keeping
    /// this structural mapping lets validation and optimization see writes;
    /// `arg_write` alone is an opaque C address.
    persistent: HashMap<NodeId, SignalInfo>,
    /// Chandle formals/return variables remain native pointer values rather
    /// than being encoded as packed integers.
    chandle_read: HashMap<NodeId, IrChandleExpr>,
    chandle_write: HashMap<NodeId, ChandleTarget>,
    string_read: HashMap<NodeId, crate::sim::ir::IrStringExpr>,
    string_write: HashMap<NodeId, String>,
    /// local var arena node → (C local name, width, signed, two-state).
    locals: HashMap<NodeId, (String, u32, bool, bool)>,
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

/// Lowered LHS of an assignment.
enum Lhs {
    Whole(SignalInfo),
    /// A complete C address/lvalue expression (a `sv4_t*` parameter such as
    /// `o0`, or `&_l0` for a local); no `&` is prepended by [`lhs_rhs_code`].
    WholeRef {
        addr: String,
        width: u32,
        signed: bool,
        two_state: bool,
    },
    Bit(SignalInfo, IrExpr, bool),
    Part(SignalInfo, i128, i128, bool),
    IdxPart(SignalInfo, IrExpr, IrExpr, u32, bool, bool),
    /// A write to one array element, with an optional element-level
    /// bit/part-select.  Emitted as a guarded statement (out-of-range or
    /// unknown indices are no-ops), never as a plain `llg_ba` argument.
    ArrayElem(ArrayElemLhs),
    /// Streaming-concatenation assignment target and its unevaluated static
    /// slice size. The effective slice is clamped after the target width is
    /// known, matching expression streaming.
    Stream {
        parts: Vec<Lhs>,
        slice: Option<u128>,
        direction: IrStreamDirection,
    },
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

enum PackedMemberSelect {
    Bit(i128),
    Part(i128, i128),
}

/// One side of a port connection: a plain global signal, or an element of an
/// unpacked array (the parent side of a connection like `.cnt(cnts[i])`,
/// addressed by the select expression the db captured on the port).
enum LinkSide {
    Signal(SignalInfo),
    ArrayElem(ArrayInfo, NodeId),
}

// ── Expression seam: IR lowering + rendering ──────────────────────────────────

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
    let (width, signed) = if info.real {
        (0, false)
    } else {
        (info.width, info.signed)
    };
    IrExpr::new(IrExprKind::SigRead(info.ir), width, signed, None)
}

/// Packed assignment width, when it is statically known.  This is expression
/// context (LRM 11.6.1), not the final RHS-to-LHS conversion performed by the
/// emitter after expression evaluation.
fn packed_lhs_width(model: &IrModel, lhs: &IrLhs) -> Option<u32> {
    let width = match lhs {
        IrLhs::Whole(idx) => model.signal(*idx).ty.width(),
        IrLhs::WholeRef { width, .. } => *width,
        IrLhs::Bit(..) => 1,
        IrLhs::Part(_, left, right, _) => ((left - right).abs() + 1) as u32,
        IrLhs::IdxPart(_, _, _, width, _, _) => *width,
        IrLhs::ArrayElem { arr, elem_sel, .. } => match elem_sel {
            IrElemSel::Whole => model.array(*arr).elem_width,
            IrElemSel::Part(left, right) => ((left - right).abs() + 1) as u32,
            IrElemSel::Bit(_) => 1,
        },
        IrLhs::Stream { width, .. } => *width,
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

/// Propagate a packed expression context into operators whose operands are
/// context-determined. Concatenation/replication operands, shift counts,
/// logical operands, and select indices deliberately remain self-determined.
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

/// Apply packed operand context after checking the implementation width limit.
fn checked_operand_with_context(
    expr: IrExpr,
    width: u32,
    signed: bool,
    scope_path: &str,
    context: &str,
) -> Result<IrExpr, String> {
    if width > LLG_MAX_WIDTH {
        return Err(format!(
            "{context} width {width} exceeds maximum {LLG_MAX_WIDTH} in `{scope_path}`"
        ));
    }
    Ok(expression_operand_with_context(expr, width, signed))
}

fn wildcard_operand_with_context(
    expr: IrExpr,
    width: u32,
    signed: bool,
    scope_path: &str,
) -> Result<IrExpr, String> {
    checked_operand_with_context(
        expr,
        width,
        signed,
        scope_path,
        "wildcard comparison context",
    )
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

fn common_bin_expr_with_context(
    op: IrBinOp,
    a: IrExpr,
    b: IrExpr,
    scope_path: &str,
) -> Result<IrExpr, String> {
    let (width, signed) = (a.width.max(b.width), a.signed && b.signed);
    Ok(IrExpr::new(
        IrExprKind::Bin {
            op,
            a: Box::new(checked_operand_with_context(
                a,
                width,
                signed,
                scope_path,
                "arithmetic/bitwise context",
            )?),
            b: Box::new(checked_operand_with_context(
                b,
                width,
                signed,
                scope_path,
                "arithmetic/bitwise context",
            )?),
        },
        width,
        signed,
        None,
    ))
}

/// A comparison/equality node after applying the common packed operand type.
/// Its result remains one-bit unsigned.
fn common_cmp_expr_ir(
    op: IrBinOp,
    a: IrExpr,
    b: IrExpr,
    scope_path: &str,
) -> Result<IrExpr, String> {
    if a.is_real() || b.is_real() {
        return Ok(cmp_expr_ir(op, a, b));
    }
    let width = a.width.max(b.width);
    let signed = a.signed && b.signed;
    Ok(cmp_expr_ir(
        op,
        checked_operand_with_context(a, width, signed, scope_path, "comparison context")?,
        checked_operand_with_context(b, width, signed, scope_path, "comparison context")?,
    ))
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

fn lhs_integer_expr(value: i128) -> IrExpr {
    let signed = value < 0;
    let width: u32 = if (signed && value >= i128::from(i32::MIN))
        || (!signed && value <= i128::from(u32::MAX))
    {
        32
    } else if (signed && value >= i128::from(i64::MIN))
        || (!signed && value <= i128::from(u64::MAX))
    {
        64
    } else {
        128
    };
    let raw = value as u128;
    let mut bits = vec![raw as u64];
    if width == 128 {
        bits.push((raw >> 64) as u64);
    } else if width == 32 {
        bits[0] &= u64::from(u32::MAX);
    }
    IrExpr::new(
        IrExprKind::Const(IrConst {
            bits,
            x: vec![0; width.div_ceil(64) as usize],
            z: vec![0; width.div_ceil(64) as usize],
            width,
            signed,
            real: None,
            fill: None,
        }),
        width,
        signed,
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

fn fill_literal_token(text: &str) -> Option<u8> {
    let bytes = text.trim().as_bytes();
    if bytes.len() != 2 || bytes[0] != b'\'' {
        return None;
    }
    match bytes[1].to_ascii_lowercase() {
        b'0' => Some(0),
        b'1' => Some(1),
        b'x' => Some(2),
        b'z' => Some(3),
        _ => None,
    }
}

fn fill_literal_token_at(line: &str, col: u32) -> Option<u8> {
    let bytes = line.as_bytes();
    let base = col.saturating_sub(1) as usize;
    for pos in [
        base,
        col as usize,
        base.saturating_sub(1),
        base.saturating_add(1),
    ] {
        for start in [pos, pos.saturating_sub(1)] {
            let Some(token) = bytes.get(start..start.saturating_add(2)) else {
                continue;
            };
            if start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_') {
                continue;
            }
            if bytes
                .get(start + 2)
                .is_some_and(|next| next.is_ascii_alphanumeric() || *next == b'_')
            {
                continue;
            }
            if let Ok(token) = std::str::from_utf8(token) {
                if let Some(fill) = fill_literal_token(token) {
                    return Some(fill);
                }
            }
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
/// through `sv4_from_real`, unsized fills through `sv4_fill`, everything else
/// through the source-signedness-aware resize chain.
fn ir_to_vector(e: IrExpr, width: u32, signed: bool) -> Result<IrExpr, String> {
    if e.is_real() {
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

/// Convert to one packed storage type, including the X/Z-to-zero rule of
/// SystemVerilog two-state destinations.
fn ir_to_storage(e: IrExpr, width: u32, signed: bool, two_state: bool) -> Result<IrExpr, String> {
    let converted = ir_to_vector(e, width, signed)?;
    Ok(if two_state {
        IrExpr::to_two_state(converted)
    } else {
        converted
    })
}

/// IR form of a value-preserving conversion to a formal/return shape
/// (`sv4_cast`; extension keyed off the source's signedness, LRM §6.24.1 /
/// §10.7).
fn ir_arg_resize(e: IrExpr, width: u32, signed: bool) -> IrExpr {
    IrExpr::convert_to(e, width, signed)
}
