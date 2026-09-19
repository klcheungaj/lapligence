//! codegen — lower a Slang-backed, owned semantic database to a C11 model for
//! the `llg` runtime (`crate::sim::rt`).
//!
//! # Pipeline
//!
//! [`generate`] accepts the owned design database ([`crate::core::db::Db`]),
//! forms the semantic and executable IR layers, and emits one C file
//! (`model.c`). Compiled together with `llg_rt.c`, `llg_random.c`, and libaco, the model is a
//! standalone simulator executable:
//!
//! - every packed scalar signal becomes a global `sv4_t G_<instance path>_<name>`
//!   (path dots become underscores), using its variable or net defaults; procedural scalar
//!   real/shortreal signals use `double` storage and typed dependency markers; every unpacked array of packed
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
//!   `initial` runs once then calls `llg_proc_done`; ordinary `always` loops
//!   `for (;;) { <body> }` with a runtime budget for zero-time back-edges;
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
//!   drivers; gate delays `#D` capture transition-specific values in the
//!   active-region inertial scheduler without suspending the comb evaluator.
//!   Switch/transistor primitives, UDP instances, primitive arrays and
//!   charge-strength forms are rejected at lowering time; illegal vector
//!   strengths, unsupported dynamic resolved-net targets, and unequal-width
//!   gate terminals remain explicit boundaries;
//! - every child-instance port pair gets a link process copying the parent
//!   side to the child side (inputs) or the child side to the parent side
//!   (outputs) whenever the source changes;
//! - every inout port collapses its parent + child nets into ONE resolved
//!   simulated net (`llg_net_t`, LRM §23.3.3.7): each member net gets a
//!   driver slot, whole-signal writes lower to `llg_net_write`, and every
//!   read goes through the shared resolution cell with each driver's strength
//!   endpoints. Inout ports emit no link; unsupported groups reject code
//!   generation rather than silently disconnecting their members;
//! - clocking input/inout members allocate owned sampled storage, while
//!   output/inout synchronous drives capture values and enqueue Re-NBA updates
//!   after their constant output skew. `##N` waits repeat the resolved default
//!   clocking event, so irregular clocks are counted by edges rather than by
//!   an assumed period;
//! - interface ports use the actual interface instance's resolved storage;
//!   scope/modport connection metadata is consumed during elaboration, not
//!   executed as a value expression or copied through per-port storage;
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
//! `@*`/`always_comb` (distinct implicit sensitivity rules with typed
//! dependencies), `#delay`, `case`/
//! `casez`/`casex` (casez/casex match per LRM 12.5.1 wildcards), `for`/
//! `while`/`repeat`/`forever`, `wait (cond)` (level-sensitive blocking:
//! re-evaluates the condition on changes of its read signals, then runs the
//! body once), function/task calls (functions and delay-free
//! tasks become C functions with a recursion depth guard; delay-bearing tasks
//! are inlined at their call sites; defaults — including defaults referencing
//! earlier formals — are supported), `fork … join`/`join_any`/`join_none` (named
//! forks included), `wait fork;`, `disable fork;`, bounded process-handle
//! control (`process::self()`, `status()`, `kill()`, `suspend()`, `resume()`,
//! and `await()`), `$display`/`$write` and
//! their b/o/h variants, `$monitor`/`$monitoron`/`$monitoroff`/`$strobe` and
//! their b/o/h variants, `$finish`, and the typed severity tasks
//! `$info`/`$warning`/`$error`/`$fatal`.  Supported
//! processes include `final begin … end` blocks (SV 1800-2005 §10.7): lowered
//! like `initial` but executed once AFTER the scheduler exits ($finish,
//! deadlock or no future events); timing controls inside a final are
//! rejected.  Supported
//! expressions: constants, casts
//! (`'(type)(expr)`), operations (arithmetic, bitwise, logical, reductions,
//! shifts, comparisons, mux, concat/replicate), refs, bit/part/indexed-part
//! selects, array-element selects (`mem[i]`, `a[i][j]`, `mem[i][3:0]`),
//! `$clog2`, `$bits`, `$signed`/`$unsigned`, `$time`, `$test$plusargs`,
//! `$value$plusargs`, and hierarchical
//! references (`top.u0.sig` — N-part paths whose final element resolves to a
//! signal) on both the READ and WRITE sides of an assignment; hierarchical
//! write targets may carry a trailing select (`top.u0.sig[3:0]`,
//! `top.u0.sig[2]`, `top.u0.sig[3 +: 4]`) with constant integer
//! indices/bounds only (the trailing select is recovered from the node name or
//! admitted source when the semantic snapshot omits its bounds). Process
//! status/equality queries remain pointer-identity operations, not packed
//! value conversions.
//!
//! The supported real subset covers procedural scalar variables, fixed unpacked
//! real and shortreal arrays, scalar real and shortreal ports, real and
//! shortreal parameters, mixed arithmetic and conditions, ordinary real
//! `case`, packed/real casts and assignment, continuous and NBA assignment,
//! real-returning value-formal subroutines, shortreal rounding, combinational
//! sensitivity, any-change event controls, `wait` conditions, and
//! `%f`/`%e`/`%g` display.
//! Packed-to-real conversion uses all model-sized limbs and treats X/Z bit
//! positions as zero; real-to-packed conversion rounds halves away from zero
//! and follows the generated model's packed capacity.
//!
//! Rejected with an `Err`: fork/join inside a function/task body, task calls
//! inside function bodies, recursive delay-bearing tasks, timing-bearing
//! class tasks, unresolved cross-instance function/task calls, string signals
//! and string parameters, class string/chandle/nested-class properties, unsupported
//! aggregate/container/reference-real subprogram contexts, widths at or above
//! the backend's exclusive generated-model capacity, and malformed
//! IR/value widths that exceed the runtime model capacity,
//! hierarchical WRITES whose final path element does not resolve to a
//! per-instance signal, hierarchical write targets with variable or
//! expression select indices/bounds, ordinary select LHS or
//! nonblocking assignment on a collapsed inout-net member (the group scan
//! rejects these before emission; clocking inout drives use their dedicated
//! resolved-net NBA path), unsupported aggregate/non-static declaration
//! initializer forms, unsupported resolved-net classes, aggregate pattern
//! display values, and unknown `$display`/`$monitor`/`$strobe` format
//! specifiers.  Structural primitives outside the supported builtin
//! set are rejected with explicit messages: switch/transistor primitives,
//! UDP instances, primitive arrays, charge-strength specifications, illegal
//! vector continuous strengths, and unsupported dynamic resolved-net targets.
//! Fixed-array elements and packed selected targets use the typed inertial
//! lowering path, as do supported whole, selected, and multi-output
//! `buf`/`not` gate forms. Expression, hierarchical, unequal-width, and
//! large-terminal gate forms remain explicit boundaries. Array constructs
//! rejected with a clear message include
//! dimension bounds that are not plain constants (an implicit `[N]` size —
//! declare `[0:N-1]` explicitly), array slices (partial indexing of a
//! multi-dimensional array), indexed part-selects on an array element, and
//! non-constant declaration-initializer elements. `$displayon`/
//! `$displayoff` are skipped with a warning. Waveform controls (`$dumpfile`/
//! `$dumpvars`/`$dumpon`/`$dumpoff`/`$dumpall`/`$dumpflush`/`$dumplimit`)
//! lower explicitly into the IR.
//! Interface instances are captured by the database walk, including inside
//! generate scopes. Slang resolves interface and modport member references
//! directly to storage on the connected concrete interface instance, so no
//! per-port storage copy or runtime link process is needed. Interface body
//! processes (always/initial/always_comb blocks inside an interface
//! definition) emit under that actual instance.
//!
//! # Database capture notes
//!
//! The database ([`crate::core::db::Db`]) does not capture every property the
//! frontend exposes. The lowering behavior for those cases is:
//!
//! - Array declaration initializers remain structural fills. Scalar variable
//!   declaration initializers arrive through `Db::vars_init` and lower to
//!   typed operations: SystemVerilog 2009 values apply before ordinary
//!   processes, while Verilog-2001 values execute in the active region.
//!   True-net forms (`wire`/`tri`/logic-net) are continuous drivers: constant
//!   RHSs run once and dynamic RHSs use a precomputed, deduplicated sensitivity
//!   set.
//! - Port connections through a bit/part select are linked on the base signal
//!   instead of being warned and skipped (the connection's select-ness is not
//!   captured; `high`/`low` resolve to the base net/var).
//! - Packed/real `force`/`release` statements are emitted through canonical
//!   target descriptors; selected net parts and packed concatenations retain
//!   their underlying drivers;
//!   procedural continuous assignment/deassignment lower to enable-guarded
//!   processes (see `lower_proc_cont_assign`).
//! - Explicit event expressions (including non-or/edge forms such as
//!   `@(a && b)`) retain dependencies from the expression and legal called
//!   function bodies, not from unrelated statements in the controlled body.
//!   Automatic locals and formals are copied into evaluator-owned frames.
//! - The signal declaration order in the emitted C is deterministic
//!   (collection order); the old codegen iterated a hash map, so its order
//!   varied between runs.
//!
//! # Backend limitations
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
//!   timescale-agnostic. Procedural delay expressions first round to local
//!   module precision before conversion to design ticks.
//! - Unsized fill literals propagate through packed expression and case
//!   contexts; self-determined concatenation/replication operands stay one bit.
//! - Generate-block processes are supported: processes inside gen scopes are
//!   emitted exactly like instance processes, with genvar references inlined
//!   to the gen-scope parameter values.  Gen-scope continuous assignments and
//!   genvar parameter references are supported.
//! - Ordinary `always` is a repeated procedure even when its body contains no
//!   wait. `always_comb` and `always_latch` retain their separate time-zero and
//!   implicit-sensitivity shaping; a zero-time ordinary loop is bounded by the
//!   generated runtime budget instead of being silently run once.
//! - Intra-assignment delays capture the RHS immediately. Blocking writes
//!   suspend until the scaled delay expires; NBAs capture their destination
//!   and schedule a future NBA without suspending the issuing process.
//!   Selected NBAs merge only their selected bits at commit time.
//!   Continuous and gate drivers capture inertial updates independently of
//!   their evaluation processes. See `docs/sim_features.md` for supported
//!   timing forms and remaining boundaries. Program process identity is
//!   retained in the IR so program initial processes launch in Reactive and
//!   `$exit` remains a typed runtime operation.

use std::collections::{HashMap, HashSet};

use super::timescale::{real_delay_ticks, round_time_literal, time_literal_delay_ticks, Timescale};
use super::CodegenError;
use crate::core::db::{
    AggregateKind, AggregateMember, AlwaysKind, ArrayKind, AssignmentPatternKeyType,
    AssociativeIndex, CaseKind as DbCaseKind, ClockingEdge, ClockingSkew, ConstantSource,
    ConstantType, Db, Direction as DbDirection, DriverDelay, EventSpec, EventTriggerTiming,
    ExprKind, ImmediateAssertionKind, IntraControl, JoinKind as DbJoinKind, NetType, NodeId,
    NodeKind, Operation, PackedMember, PrimClass, PrimitiveType, ProcessKind, StmtKind,
    StreamingDirection as DbStreamingDirection, Strength, TypeDescriptor, TypeShape,
    VariableLifetime,
};
use crate::core::elab::{self, Bit, Val};
use crate::core::model::TypeInfo;
use crate::core::value::ValueData;
use crate::ffi::slang::LanguageEdition;
use crate::sim::emit_c::{
    array_guard, escaped_char, event_global_name, global_name, ident, real_global_name,
    render_expr, strip_lib, RCtx, LLG_MAX_WIDTH,
};
use crate::sim::ir::{
    FrameId, IrAssocKey, IrAssocTraversal, IrBinOp, IrBitQuery, IrCall, IrCallArg, IrCallExpr,
    IrCapture, IrCapturedBranch, IrCaseItem, IrCaseKind, IrChandleExpr, IrClockingSampleMode,
    IrConst, IrContainer, IrContainerExpr, IrContainerKind, IrContainerStmt, IrDeferredAction,
    IrDelay, IrDependency, IrDepth, IrDisplayRadix, IrEdge, IrElemSel, IrEvent, IrEventCapture,
    IrEventContext, IrEventRef, IrExpr, IrExprKind, IrFormal, IrImmediateAssertionKind,
    IrInitPhase, IrInitTarget, IrInitialization, IrJoinKind, IrLhs, IrMemoryRadix, IrModel,
    IrNetAliasBinding, IrProcess, IrProcessKind, IrRealBinOp, IrRealUnOp, IrSeverityLevel, IrShape,
    IrSignal, IrStmt, IrStochasticStmt, IrStreamDirection, IrStreamTarget, IrSysFunc, IrTimeKind,
    IrTransitionDelay, IrType, IrUnOp, IrUniquePriorityCheck, IrVpiObject, IrVpiObjectKind,
    IrWaitSrc, StorageKind, StorageLifetime, StorageOwnership, StorageRef, LLG_MAX_NET_DRIVERS,
};

mod assertion_context;
mod clocking_context;
mod delays;
mod initialization;
mod references;
mod selections;

mod assertions;
mod collection;
mod containers;
mod expressions;
mod objects;
mod statements;

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

/// Native pointer-valued SystemVerilog declarations. Class handles share the
/// existing object ABI with `chandle`; their nominal layout and method
/// receiver are carried separately by the class tables below.
pub(super) fn is_handle_kind(kind: &str) -> bool {
    matches!(kind, "chandle" | "class" | "virtual_interface")
}

/// The generated C model plus non-fatal warnings collected while lowering.
pub struct GeneratedModel {
    /// Complete `model.c` source: `#include "llg_rt.h"`, signal globals,
    /// process functions, and `main`.
    pub model_c: String,
    /// The design name (also embedded in the model's first comment line).
    pub design_name: String,
    /// Non-fatal diagnostics about supported lowering limitations.
    pub warnings: Vec<String>,
}

/// Lower an owned Slang semantic database with default optimizations.
pub fn generate(db: &Db) -> Result<GeneratedModel, CodegenError> {
    generate_with_opts(db, &crate::sim::opt::OptConfig::default())
}

/// Lower an owned Slang semantic database with explicit optimizations.
pub fn generate_with_opts(
    db: &Db,
    cfg: &crate::sim::opt::OptConfig,
) -> Result<GeneratedModel, CodegenError> {
    generate_from_db_with_opts(db, cfg)
}

/// Lower an already-owned database with an explicit optimization
/// configuration.
///
/// Reuse this entry point when producing multiple variants of one elaborated
/// design. The executable lowering remains independent of the frontend
/// snapshot lifetime because the database owns its semantic relationships.
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
    let semantic = crate::sim::semantic::SemanticModel::from_db(db);
    if let Err(issues) = semantic.validate_simulation() {
        if let Some(issue) = issues.into_iter().next() {
            return Err(issue.diagnostic(&semantic));
        }
        return Err("simulation validation failed without an issue".to_owned());
    }
    let mut cg = Codegen::new(&semantic);
    cg.validate_program_constructs()?;
    let tops = cg.collect_design()?;
    if tops.is_empty() {
        return Err("no top modules in the elaborated design".to_string());
    }
    let compilation_units = cg.compilation_unit_scopes();
    // Collapse inout-port net groups (parent + child nets → one resolved
    // simulated net) before any signal/process lowering so member reads and
    // writes use the resolution cell.
    cg.bind_reference_ports()?;
    // Timescales must be fixed before any `#delay`/`$time` is lowered so the
    // design precision (scheduler tick unit) is consistent across the model.
    cg.collect_timescales();
    cg.build_net_groups()?;
    cg.collect_clocking_storage()?;
    cg.validate_process_semantics()?;
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
    cg.emit_class_func_prototypes()?;
    for top in &tops {
        cg.emit_func_prototypes(*top)?;
    }
    for package in db.packages() {
        cg.emit_func_prototypes(*package)?;
    }
    for unit in &compilation_units {
        cg.emit_func_prototypes(*unit)?;
    }
    // Virtual-interface descriptors need both concrete member storage and
    // subroutine prototypes. Build them before any body is lowered so all
    // call sites share one rebinding table.
    cg.collect_virtual_interfaces()?;
    cg.emit_class_func_bodies()?;
    for top in &tops {
        cg.emit_func_bodies(*top)?;
    }
    for package in db.packages() {
        cg.emit_func_bodies(*package)?;
    }
    for unit in &compilation_units {
        cg.emit_func_bodies(*unit)?;
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
    cg.emit_clocking_processes()?;
    cg.emit_array_initializers()?;
    cg.emit_container_initializers()?;
    cg.emit_class_object_initializers()?;
    cg.emit_semaphore_initializers()?;
    cg.emit_mailbox_object_initializers()?;
    let mut model = std::mem::replace(
        &mut cg.model,
        IrModel::new(String::new(), Timescale::DEFAULT.precision_fs)
            .expect("the default timescale has non-zero precision"),
    );
    cg.build_init_steps(&mut model)?;
    cg.populate_vpi_metadata(&mut model, &tops)?;
    // Final-block processes render like any other but spawn into the
    // post-simulation phase: they run after `llg_rt_run` returns, not at
    // t=0.
    let final_names = std::mem::take(&mut cg.final_procs);
    let assertion_action_procs = std::mem::take(&mut cg.assertion_action_procs);
    model.spawns = model
        .processes
        .iter()
        .map(|p| p.c_name.clone())
        .filter(|n| !final_names.contains(n))
        .filter(|n| !assertion_action_procs.contains(n))
        .collect();
    model.final_spawns = final_names;
    model.validate().map_err(|error| error.to_string())?;
    let mut execution =
        crate::sim::execution::ExecutionModel::lower(model).map_err(|error| error.to_string())?;
    crate::sim::opt::run(&mut execution, cfg).map_err(|error| error.to_string())?;
    execution.validate().map_err(|error| error.to_string())?;
    let model_c = crate::sim::emit_c::render(&execution)?;
    Ok(GeneratedModel {
        design_name: cg.design_name.clone(),
        model_c,
        warnings: cg.warnings,
    })
}

impl<'a> Codegen<'a> {
    /// Capture the source-owned hierarchy and storage identities used by the
    /// generated VPI catalog.  This runs after collection has assigned stable
    /// IR indices but before optimization can prune any otherwise dead signal.
    fn populate_vpi_metadata(&self, model: &mut IrModel, tops: &[NodeId]) -> Result<(), String> {
        use std::collections::BTreeMap;

        fn reachable(db: &Db, node: NodeId, tops: &[NodeId]) -> bool {
            let mut current = Some(node);
            while let Some(id) = current {
                if tops.contains(&id) {
                    return true;
                }
                current = db.node(id).parent;
            }
            false
        }

        fn components(path: &str) -> impl Iterator<Item = &str> {
            path.split('\u{1f}')
        }

        let mut objects = BTreeMap::<String, IrVpiObject>::new();
        for id in self.db.node_ids() {
            if !reachable(self.db, id, tops) {
                continue;
            }
            let node = self.node(id);
            match self.kind(id) {
                NodeKind::ModuleInst { def_name, .. } => {
                    let full_name = self.waveform_name_for(id);
                    let name = components(&full_name)
                        .last()
                        .unwrap_or(&node.name)
                        .to_owned();
                    objects.entry(full_name.clone()).or_insert_with(|| {
                        let mut object = IrVpiObject::module(
                            full_name,
                            name,
                            def_name.clone(),
                            node.file.clone(),
                            node.line,
                        );
                        object.time_unit_fs = self.timescale_of_node(id).unit_fs;
                        object
                    });
                }
                NodeKind::Net { .. } | NodeKind::Var { .. } => {
                    let Some(info) = self.sig_globals.get(&id) else {
                        continue;
                    };
                    let Some(signal) = model.signals.get(info.ir) else {
                        return Err(format!(
                            "VPI signal metadata references missing IR signal {} for `{}`",
                            info.ir, node.full_name
                        ));
                    };
                    let Some(full_name) = signal.hdl_name.clone() else {
                        continue;
                    };
                    let name = components(&full_name)
                        .last()
                        .unwrap_or(&node.name)
                        .to_owned();
                    let (width, signed, real) = match signal.ty {
                        IrType::Packed { width, signed, .. } => (width, signed, false),
                        IrType::Real { .. } => (0, false, true),
                    };
                    let net = matches!(self.kind(id), NodeKind::Net { .. })
                        || signal.net_driver.is_some()
                        || !signal.net_alias.is_empty();
                    let kind = if real {
                        IrVpiObjectKind::RealVar
                    } else if net {
                        IrVpiObjectKind::Net
                    } else {
                        IrVpiObjectKind::Reg
                    };
                    objects
                        .entry(full_name.clone())
                        .or_insert_with(|| IrVpiObject {
                            time_unit_fs: self.timescale_of_node(id).unit_fs,
                            full_name,
                            name,
                            definition_name: None,
                            file: node.file.clone(),
                            line: node.line,
                            kind,
                            width,
                            signed,
                            real,
                            net,
                            signal: Some(info.ir),
                            array: None,
                        });
                }
                NodeKind::Array { .. } => {
                    let Some(info) = self.array_globals.get(&id) else {
                        continue;
                    };
                    let Some(array) = model.arrays.get(info.ir) else {
                        return Err(format!(
                            "VPI array metadata references missing IR array {} for `{}`",
                            info.ir, node.full_name
                        ));
                    };
                    let full_name = array.hdl_name.clone();
                    let name = components(&full_name)
                        .last()
                        .unwrap_or(&node.name)
                        .to_owned();
                    objects
                        .entry(full_name.clone())
                        .or_insert_with(|| IrVpiObject {
                            time_unit_fs: self.timescale_of_node(id).unit_fs,
                            full_name,
                            name,
                            definition_name: None,
                            file: node.file.clone(),
                            line: node.line,
                            kind: IrVpiObjectKind::RegArray,
                            width: array.elem_width,
                            signed: array.signed,
                            real: array.real,
                            net: false,
                            signal: None,
                            array: Some(info.ir),
                        });
                }
                _ => {}
            }
        }
        model.vpi_objects = objects.into_values().collect();
        Ok(())
    }
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
struct ClockingSampleInfo {
    source: NodeId,
    sample: SignalInfo,
}

/// Clock inferred from an enclosing property or supplied as a sampled-value
/// clocking event. The optional gate is evaluated in the same sampled domain.
#[derive(Clone, Copy)]
pub(super) struct SampledClock {
    pub(super) signal: usize,
    pub(super) posedge: bool,
    pub(super) gate: Option<NodeId>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct DriverId(u32);

#[derive(Clone, Debug)]
struct StructuralDriverRecord {
    signal: usize,
}

#[derive(Clone)]
struct ProcLocalInfo {
    c_name: String,
    width: u32,
    signed: bool,
    two_state: bool,
    /// Static procedural locals use hidden model storage; automatic locals
    /// remain C block locals and are recreated on each declaration entry.
    static_signal: Option<SignalInfo>,
}

#[derive(Clone)]
struct CaptureBinding {
    storage: StorageRef,
    local: ProcLocalInfo,
}

#[derive(Clone)]
struct CaptureSource {
    info: ProcLocalInfo,
    initial: IrExpr,
    lifetime: StorageLifetime,
    kind: StorageKind,
}

fn storage_kind(width: u32) -> StorageKind {
    if width == 0 {
        StorageKind::Real
    } else {
        StorageKind::Packed
    }
}

/// A lowered unpacked array: a flat C array of `sv4_t` elements plus the
/// per-dimension metadata needed to linearize indices.
#[derive(Clone)]
struct ArrayInfo {
    global: String,
    /// Element vector width in bits.
    elem_width: u32,
    signed: bool,
    real: bool,
    shortreal: bool,
    /// Net elements initialize to Z; variable elements use their type default.
    is_net: bool,
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
    /// Packed and real leaves use typed signal storage. Strings use the
    /// existing owned-object runtime path; aggregate nodes have neither.
    signal: Option<SignalInfo>,
    object: Option<usize>,
    path: Vec<AggregatePathPart>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum AggregatePathPart {
    Member(String),
    Index(i32),
}

#[derive(Clone)]
struct UnpackedAggregateInfo {
    kind: AggregateKind,
    type_identity: Option<String>,
    members: Vec<AggregateMemberInfo>,
    /// Recursive descriptors are lowered to deterministic leaves only at the
    /// backend boundary. The descriptor itself remains the compatibility and
    /// ownership authority.
    leaves: Vec<AggregateMemberInfo>,
}

fn aggregate_array_member_path(
    layout: &crate::core::db::AggregateLayout,
    name: &str,
    prefix: &[AggregatePathPart],
) -> Option<Vec<AggregatePathPart>> {
    for member in &layout.members {
        let mut member_path = prefix.to_vec();
        member_path.push(AggregatePathPart::Member(member.name.clone()));
        match &member.descriptor.shape {
            TypeShape::FixedArray { .. } if member.name == name => return Some(member_path),
            TypeShape::Aggregate(nested) => {
                if let Some(path) = aggregate_array_member_path(nested, name, &member_path) {
                    return Some(path);
                }
            }
            TypeShape::FixedArray { element, .. } => {
                if let TypeShape::Aggregate(nested) = &element.shape {
                    if let Some(path) = aggregate_array_member_path(nested, name, &member_path) {
                        return Some(path);
                    }
                }
            }
            _ => {}
        }
    }
    None
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

#[derive(Clone)]
pub(super) struct EventTarget {
    pub(super) declaration: NodeId,
    pub(super) indices: Vec<NodeId>,
}

/// An element-level select applied after the array index of a
/// `mem[addr][3:0]`-style selection.
#[derive(Clone)]
enum ElemSel {
    /// Whole element.
    Whole,
    /// Part-select `[left:right]` of the element.
    Part(i128, i128),
    /// Bit-select of the element by a runtime index expression.
    Bit(IrExpr),
    /// Indexed part-select with a translated runtime base and constant width.
    Indexed(IrExpr, u32, bool),
}

/// LHS of an assignment to one array element (with optional element-level
/// bit/part select).
#[derive(Clone)]
struct ArrayElemLhs {
    arr: ArrayInfo,
    /// One typed expression per dimension index, in declaration order.
    indices: Vec<IrExpr>,
    elem_sel: ElemSel,
}

/// One procedural continuous assignment site (`assign <var> = …;`): the
/// enable guard's storage plus the runtime site identity and which statement
/// node materialized the guard process.
struct PcaSite {
    /// Enable-signal IR index (`G_<path>_pca$<n>_en`, starts X = disabled).
    en: usize,
    /// Runtime identity of this syntactic assignment site. Distinct sites
    /// targeting one variable replace one another at execution time.
    site: usize,
    /// ProcContAssign arena node whose lowering created the guard process;
    /// `None` between pre-scan allocation and first lowering. The SAME node
    /// again (a delay-bearing task body inlined at several call sites) reuses
    /// the existing site and guard.
    guarded_by: Option<NodeId>,
}

/// A call argument bound to one formal: its width/signedness and the arena
/// node of the actual expression (the bound argument, or the formal's default
/// when the call omits it).
struct BoundArg {
    width: u32,
    signed: bool,
    two_state: bool,
    real: bool,
    shortreal: bool,
    string: bool,
    expr: NodeId,
    /// `true` when `expr` is the formal's default expression rather than a
    /// caller-provided argument.  Default expressions are emitted under a
    /// temporary formal-aware context so references to earlier formals resolve
    /// to the bound arguments.
    is_default: bool,
    /// Named-event arguments are handles rather than packed values. They are
    /// resolved into the caller's event object when a task is inlined.
    is_event: bool,
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
    /// Bit/part/indexed-part/array select write. Constant selected
    /// continuous drivers are mapped to a source-specific slot; procedural
    /// selected writes are rejected by the caller.
    Select,
}

/// Declaration kind targeted by a declaration assignment. True-net drivers
/// and variable initializers have intentionally different scheduling.
#[derive(Copy, Clone)]
enum NetDeclTarget {
    Array,
    Variable,
    TrueNet,
    UnsupportedNet(NetType),
    Unknown,
}

struct Codegen<'a> {
    origins: Vec<crate::sim::semantic::Origin>,
    db: &'a Db,
    /// Ordinary values use 2009 time-literal rounding. Delay expressions
    /// temporarily disable it so their complete expression rounds once.
    round_time_literals: bool,
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
    /// C linkage name → canonical lowered DPI signature.  Slang diagnoses
    /// conflicting imports in one frontend compilation; this second check
    /// protects the owned/lowered boundary when cloned declarations arrive
    /// through different semantic paths.
    dpi_signatures: HashMap<String, String>,
    /// Persistent storage for every formal of static subroutines,
    /// keyed by (owning instance, formal declaration).
    static_formals: HashMap<(NodeId, NodeId), SignalInfo>,
    /// Persistent native string storage for static subroutine formals.
    static_string_formals: HashMap<(NodeId, NodeId), usize>,
    /// Native pointer storage for static chandle formals.
    static_chandle_formals: HashMap<(NodeId, NodeId), usize>,
    /// Persistent storage for locals of static delay-bearing tasks. Those
    /// tasks are inlined, so their storage must live outside each call site.
    static_task_locals: HashMap<(NodeId, NodeId), SignalInfo>,
    /// Persistent native string storage for static delay-bearing task locals.
    static_string_task_locals: HashMap<(NodeId, NodeId), usize>,
    /// Persistent native-pointer storage for static chandle locals in
    /// functions and delay-bearing tasks.
    static_task_chandle_locals: HashMap<(NodeId, NodeId), usize>,
    /// All lowered signals, in collection order (deterministic emission).
    signals: Vec<SignalInfo>,
    /// Net/Var arena node → lowered signal info (all instances + gen scopes).
    sig_globals: HashMap<NodeId, SignalInfo>,
    /// Clocking block variable → synthesized sampled storage and source.
    clocking_samples: HashMap<NodeId, ClockingSampleInfo>,
    /// Canonical lvalues for module `ref` port storage.  A target may be a
    /// whole signal, a packed selection, or one fixed-array element; keeping
    /// the typed lvalue here makes nested ref ports compose without creating
    /// a copy-in/copy-out link.
    reference_signals: HashMap<usize, IrLhs>,
    /// Canonical fixed-array storage for module `ref` array ports.
    reference_arrays: HashMap<usize, usize>,
    /// Canonical object storage for module `ref` string/chandle ports.
    reference_objects: HashMap<usize, usize>,
    object_globals: HashMap<NodeId, usize>,
    scope_object_names: HashMap<String, HashMap<String, usize>>,
    /// Nominal class declaration → execution-IR layout index.
    class_nodes: HashMap<NodeId, usize>,
    /// Class method declaration → virtual dispatch slot.
    method_virtual_slots: HashMap<NodeId, usize>,
    /// Class property declaration → `(layout index, field index)` for
    /// non-static properties.
    class_fields: HashMap<NodeId, (usize, usize)>,
    /// Static class properties use ordinary model storage and retain their
    /// declaration identity here.
    class_static_signals: HashMap<NodeId, SignalInfo>,
    class_static_objects: HashMap<NodeId, usize>,
    /// Class variables with declaration-time `new(...)` initializers are
    /// deferred until class methods have prototypes and can be called.
    class_object_initializers: Vec<(NodeId, usize, NodeId, String)>,
    /// Semaphore variables with declaration-time `new(...)` initializers are
    /// deferred until the generated runtime process starts.
    semaphore_initializers: Vec<(NodeId, usize, NodeId, String)>,
    /// Mailbox variables with declaration-time `new(...)` initializers use
    /// the same time-zero ordering, but are constructed by the mailbox
    /// runtime rather than a user class constructor.
    mailbox_object_initializers: Vec<(NodeId, usize, NodeId, String)>,
    /// Receiver used while lowering a class property's default expression or
    /// constructor body during a fresh allocation.
    class_init_receiver: Option<IrChandleExpr>,
    /// Top-level unpacked aggregate variables lowered to member storage.
    unpacked_aggregates: HashMap<NodeId, UnpackedAggregateInfo>,
    /// Synthetic owned string objects for recursive aggregate leaves. The
    /// key uses declaration identity and canonical member/index path, never a
    /// display spelling or frontend pointer.
    aggregate_objects: HashMap<(NodeId, String), usize>,
    /// Procedural declaration node → automatic C local or hidden static signal.
    /// Procedural declaration node -> automatic C local or hidden static
    /// signal for the currently lowered process context.
    proc_locals: HashMap<NodeId, ProcLocalInfo>,
    /// Automatic string locals used by foreach string-key iterators. Native
    /// strings have a distinct C representation and therefore do not fit the
    /// packed/real `ProcLocalInfo` table.
    proc_string_locals: HashMap<NodeId, String>,
    /// Automatic mailbox handles declared in a process body. Mailboxes use
    /// native runtime pointers, so they are kept separate from packed locals.
    proc_mailbox_locals: HashMap<NodeId, String>,
    /// Persistent mailbox objects for static procedural declarations, keyed
    /// by elaborated instance and declaration identity.
    proc_mailbox_static_objects: HashMap<(NodeId, NodeId), usize>,
    /// Automatic process handles declared inside a process body. Process
    /// identities use the runtime's reference-counted handle ABI rather than
    /// packed/real process-local storage.
    proc_process_locals: HashMap<NodeId, String>,
    /// Persistent process-handle objects for static procedural declarations,
    /// keyed by elaborated instance and declaration identity.
    proc_process_static_objects: HashMap<(NodeId, NodeId), usize>,
    /// Automatic semaphore locals declared in process bodies.
    proc_semaphore_locals: HashMap<NodeId, String>,
    /// Persistent semaphore objects for static procedural declarations,
    /// keyed by elaborated instance and declaration identity.
    proc_semaphore_static_objects: HashMap<(NodeId, NodeId), usize>,
    /// Hidden static process-local storage keyed by elaborated instance and
    /// declaration. A declaration node is shared by module instances, while
    /// its static lifetime is per elaborated instance.
    proc_local_instances: HashMap<(NodeId, NodeId), ProcLocalInfo>,
    /// Parent-declaration bindings active while one fork branch is lowered.
    /// The typed storage descriptor remains the semantic identity; `local` is
    /// only the branch emitter's private C spelling.
    capture_locals: HashMap<NodeId, CaptureBinding>,
    /// Legacy storage for scalar declaration-initializer fills that need a
    /// collapsed-net driver slot. True-net declarations now lower as
    /// continuous processes, so ordinary wire/tri entries do not use it.
    net_inits: Vec<(String, usize, IrConst)>,
    /// Initial contributions of delayed drivers, applied after storage defaults.
    delayed_driver_inits: Vec<crate::sim::ir::IrInitStep>,
    /// All lowered arrays, in collection order (deterministic emission).
    arrays: Vec<ArrayInfo>,
    /// Array arena node → lowered array info.
    array_globals: HashMap<NodeId, ArrayInfo>,
    /// Dynamic arrays, queues, and associative arrays use owned runtime
    /// storage and never alias fixed unpacked-array storage.
    container_globals: HashMap<NodeId, ContainerInfo>,
    /// Resizable-container declaration patterns are lowered after all
    /// functions and processes exist, so nonconstant elements use the same
    /// expression/capture machinery as procedural assignments.
    container_initializers: Vec<(NodeId, usize)>,
    /// Implicit iterator binding while lowering an array-method `with`
    /// expression: declaration identity plus the packed source element and
    /// index types.
    container_iterator: Option<ContainerIterator>,
    /// Method callback helpers are discovered while expression lowering, but
    /// attach to the owning process/function only after its body is complete.
    pending_container_pre_fns: Vec<crate::sim::ir::IrPreFn>,
    /// Fixed-array declaration assignments whose RHS is not a static constant
    /// pattern. These become run-once initialization processes after every
    /// array and container has been collected.
    array_initializers: Vec<(NodeId, NodeId)>,
    /// All lowered named events, in collection order (deterministic emission).
    events: Vec<EventInfo>,
    /// NamedEvent arena node → lowered event info.
    event_globals: HashMap<NodeId, EventInfo>,
    /// Event-array declaration and linear element index → lowered event info.
    event_elements: HashMap<(NodeId, u64), EventInfo>,
    /// Named-event array declaration → descriptor index in the IR event table.
    event_arrays: HashMap<NodeId, usize>,
    /// (signal info, constant) declaration-initializer fills for scalar
    /// variable-like net objects (`reg y = 0;`), applied in `main()` before
    /// any process runs (mirrors the array declaration-initializer handling).
    scalar_inits: Vec<(SignalInfo, IrConst)>,
    /// (signal info, constant) declaration-initializer fills for scalar
    /// variables whose initializer is captured in `Db::vars_init` (`logic
    /// l = 1'b0;`, `int x = 5;`), applied in
    /// `main()` after the net-decl fills and before any process runs.
    var_inits: Vec<(SignalInfo, IrConst)>,
    /// Typed declaration initializers whose value is evaluated at the
    /// recorded scheduling phase. These are kept separate from constant
    /// storage fills so runtime-dependent initializers never fall back to
    /// compile-time evaluation.
    declaration_inits: Vec<IrInitialization>,
    /// Scalar declaration initializers whose expression contains a user
    /// function call. Subroutine metadata does not exist until prototypes are
    /// emitted, so lowering is deferred until then. Each tuple is
    /// `(declaration, initializer, owning instance, resolved storage)`.
    deferred_declaration_inits: Vec<(NodeId, NodeId, NodeId, SignalInfo)>,
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
    frame_seq: u32,
    /// Design time precision in ps: the finest precision across every module,
    /// which sets the scheduler tick unit (1 tick = `design_precision_fs` fs).
    design_precision_fs: u64,
    /// Procedural continuous assignment sites: (ProcContAssign arena node,
    /// target signal) → site. Sites are allocated by a pre-scan over ALL
    /// process bodies BEFORE any body lowers
    /// ([`Codegen::prescan_pca_sites`]), so every assignment site has a stable
    /// runtime identity regardless of process/source order or instance.
    pca_sites: HashMap<(NodeId, usize), PcaSite>,
    /// PCA site sequence, used for unique enable-global names
    /// (`G_<path>_pca$<n>_en`; the `$` can never occur in an
    /// ident()-sanitized user name, so synthesized enables cannot collide
    /// with a user variable's global).
    pca_seq: usize,
    /// Whole-net continuous assignment node -> synthetic signal index carrying
    /// that wired net driver's distinct runtime slot.
    wired_driver_sites: HashMap<NodeId, usize>,
    /// Structural source node and resolved-group index -> synthetic signal
    /// carrying that source's independent contribution slot.  A source can
    /// feed more than one canonical group (for example a hierarchical port
    /// connection), so the group is part of the key rather than being inferred
    /// from a display name.
    structural_driver_sites: HashMap<(NodeId, usize), DriverId>,
    /// Typed structural-driver records kept until the model is fully lowered.
    /// The record is the single source of truth for contribution identity;
    /// synthetic signal indices are only an emission detail.
    structural_drivers: Vec<StructuralDriverRecord>,
    /// Additional contribution slots for multi-output primitives whose
    /// outputs land in the same canonical resolved group. The ordinary
    /// source/group key remains terminal zero for compatibility with the
    /// existing structural-driver inventory.
    structural_driver_terminal_sites: HashMap<(NodeId, usize, usize), DriverId>,
    /// Final-block process function names (`ProcessKind::Final`), in
    /// emission order — spawned into [`IrModel::final_spawns`] instead of
    /// the t=0 spawn list.
    final_procs: Vec<String>,
    /// Assertion action helpers are registered by the runtime and must not
    /// also be spawned as ordinary design processes at time zero.
    assertion_action_procs: HashSet<String>,
    /// Clock inferred while lowering a property or its Reactive action.
    sampled_clock: Option<SampledClock>,
    /// Slang-materialized local assertion declarations currently being
    /// lowered into one sequence graph. The map is scoped to the graph so
    /// every runtime attempt receives an independent slot set.
    assertion_local_bindings: Option<HashMap<NodeId, AssertionLocalBinding>>,
    /// Assertion formal-to-actual bindings active while recursively lowering
    /// an owned named sequence/property instance.
    assertion_formal_actuals: Option<HashMap<NodeId, NodeId>>,
    /// Sampled initializers for local input formals, keyed by their Slang
    /// declaration identity. The final sequence graph orders these by local
    /// slot and evaluates them once when an attempt is created.
    assertion_local_initializers: Option<HashMap<NodeId, IrExpr>>,
    /// A legal sequence match-item call is statement-like even when the
    /// shared expression lowerer represents a void task/function result as X.
    /// Keep that context local so it does not produce a misleading warning.
    lowering_assertion_match_item: bool,
    /// Nesting depth while expanding owned assertion instances. Local formal
    /// capture is only valid for the top-level instance that starts a graph;
    /// delayed or nested invocations need distinct slots and remain a
    /// fail-closed boundary until the IR represents invocation frames.
    assertion_instance_depth: usize,
    /// Whether the current top-level assertion instance begins at sequence
    /// attempt entry. This is paired with `assertion_instance_depth` so a
    /// formal initializer cannot silently be evaluated at graph creation when
    /// the invocation actually starts after a temporal prefix.
    assertion_local_capture_at_attempt_entry: bool,
    /// Canonical virtual-interface type identity → execution descriptor.
    virtual_interface_types: HashMap<String, usize>,
    /// Concrete interface instance → `(descriptor, instance)` binding.
    virtual_interface_instances: HashMap<NodeId, (usize, usize)>,
    /// `(descriptor, member path)` → member slot.
    virtual_interface_members: HashMap<(usize, String), usize>,
    /// `(descriptor, method name)` → method slot.
    virtual_interface_methods: HashMap<(usize, String), usize>,
    /// Modport view restrictions keyed by descriptor and view name. Each
    /// member entry retains its captured direction for assignment checks.
    virtual_interface_views: HashMap<(usize, String), HashMap<String, DbDirection>>,
    /// Methods explicitly imported/exported by each modport view.
    virtual_interface_view_methods: HashMap<(usize, String), HashSet<String>>,
}

/// Runtime-visible bindings for one array-method `with` expression. A zero
/// index width marks a legal receiver whose index is not representable by the
/// packed callback ABI (for example, a string-key associative array).
#[derive(Clone, Copy)]
struct ContainerIterator {
    node: NodeId,
    item_width: u32,
    item_signed: bool,
    index_width: u32,
    index_signed: bool,
}

#[derive(Clone, Copy)]
struct AssertionLocalBinding {
    slot: u32,
    width: u32,
    signed: bool,
    two_state: bool,
}

impl<'a> Codegen<'a> {
    fn new(semantic: &crate::sim::semantic::SemanticModel<'a>) -> Codegen<'a> {
        let db = semantic.db();
        Codegen {
            origins: semantic.origins().to_vec(),
            db: semantic.db(),
            round_time_literals: db.edition() == LanguageEdition::SystemVerilog2009,
            warnings: Vec::new(),
            model: IrModel::new(String::new(), Timescale::DEFAULT.precision_fs)
                .expect("the default timescale has non-zero precision"),
            cur_fn_ir: None,
            func_meta: HashMap::new(),
            dpi_signatures: HashMap::new(),
            static_formals: HashMap::new(),
            static_string_formals: HashMap::new(),
            static_chandle_formals: HashMap::new(),
            static_task_locals: HashMap::new(),
            static_string_task_locals: HashMap::new(),
            static_task_chandle_locals: HashMap::new(),
            signals: Vec::new(),
            sig_globals: HashMap::new(),
            clocking_samples: HashMap::new(),
            reference_signals: HashMap::new(),
            reference_arrays: HashMap::new(),
            reference_objects: HashMap::new(),
            object_globals: HashMap::new(),
            scope_object_names: HashMap::new(),
            class_nodes: HashMap::new(),
            method_virtual_slots: HashMap::new(),
            class_fields: HashMap::new(),
            class_static_signals: HashMap::new(),
            class_static_objects: HashMap::new(),
            class_object_initializers: Vec::new(),
            semaphore_initializers: Vec::new(),
            mailbox_object_initializers: Vec::new(),
            class_init_receiver: None,
            unpacked_aggregates: HashMap::new(),
            aggregate_objects: HashMap::new(),
            proc_locals: HashMap::new(),
            proc_string_locals: HashMap::new(),
            proc_mailbox_locals: HashMap::new(),
            proc_mailbox_static_objects: HashMap::new(),
            proc_process_locals: HashMap::new(),
            proc_process_static_objects: HashMap::new(),
            proc_semaphore_locals: HashMap::new(),
            proc_semaphore_static_objects: HashMap::new(),
            proc_local_instances: HashMap::new(),
            capture_locals: HashMap::new(),
            net_inits: Vec::new(),
            delayed_driver_inits: Vec::new(),
            arrays: Vec::new(),
            array_globals: HashMap::new(),
            container_globals: HashMap::new(),
            container_initializers: Vec::new(),
            container_iterator: None,
            pending_container_pre_fns: Vec::new(),
            array_initializers: Vec::new(),
            events: Vec::new(),
            event_globals: HashMap::new(),
            event_elements: HashMap::new(),
            event_arrays: HashMap::new(),
            scalar_inits: Vec::new(),
            scalar_init_ca: HashSet::new(),
            var_inits: Vec::new(),
            declaration_inits: Vec::new(),
            deferred_declaration_inits: Vec::new(),
            scope_array_names: HashMap::new(),
            param_vals: HashMap::new(),
            scope_sig_names: HashMap::new(),
            gen_scope_paths: HashMap::new(),
            func_names: HashMap::new(),
            func: None,
            depth_arg: "0".to_string(),
            inst: NodeId(0),
            design_name: String::new(),
            proc_seq: 0,
            frame_seq: 0,
            design_precision_fs: Timescale::DEFAULT.precision_fs,
            pca_sites: HashMap::new(),
            pca_seq: 0,
            wired_driver_sites: HashMap::new(),
            structural_driver_sites: HashMap::new(),
            structural_drivers: Vec::new(),
            structural_driver_terminal_sites: HashMap::new(),
            final_procs: Vec::new(),
            assertion_action_procs: HashSet::new(),
            sampled_clock: None,
            assertion_local_bindings: None,
            assertion_formal_actuals: None,
            assertion_local_initializers: None,
            lowering_assertion_match_item: false,
            assertion_instance_depth: 0,
            assertion_local_capture_at_attempt_entry: false,
            virtual_interface_types: HashMap::new(),
            virtual_interface_instances: HashMap::new(),
            virtual_interface_members: HashMap::new(),
            virtual_interface_methods: HashMap::new(),
            virtual_interface_views: HashMap::new(),
            virtual_interface_view_methods: HashMap::new(),
        }
    }

    fn origin(&self, node: NodeId) -> crate::sim::semantic::Origin {
        self.origins.get(node.index()).cloned().unwrap_or_else(|| {
            crate::sim::semantic::Origin::Synthetic {
                reason: format!("lowered {}", self.node(node).full_name()),
            }
        })
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
            if self.is_runtime_environment(id) {
                return self.namespace_path(id);
            }
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
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => refs
                .first()
                .copied()
                .flatten()
                .or_else(|| refs.last().copied().flatten())
                .and_then(|target| self.array_globals.get(&target)),
            NodeKind::Expr(ExprKind::Cast { operand, .. }) => self.array_of(*operand),
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Assignment,
                operands,
                ..
            }) => operands.first().and_then(|operand| self.array_of(*operand)),
            _ => None,
        }
    }

    /// Visit every node and select the finest resolved module precision.
    fn walk_files(&mut self, node: NodeId) {
        if matches!(self.kind(node), NodeKind::ModuleInst { .. }) {
            let ts = self.timescale_of_node(node);
            self.design_precision_fs = self.design_precision_fs.min(ts.precision_fs);
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

fn time_exponent_to_fs(exponent: i32) -> u64 {
    // Slang stores a decimal time scale as a base-10 exponent. The standard
    // range is 1fs through 100s, which is 10^0 through 10^17 femtoseconds.
    let Some(power) = exponent
        .checked_add(15)
        .filter(|power| (0..=17).contains(power))
    else {
        return 0;
    };
    10_u64.pow(power as u32)
}

/// SystemVerilog two-state integral types (IEEE 1800-2009 Table 6-8).
fn is_two_state_kind(kind: &str) -> bool {
    matches!(kind, "bit" | "byte" | "shortint" | "int" | "longint")
}

// ── Union-find (collapsed inout-net groups) ───────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct AliasBit {
    net: NodeId,
    bit: u32,
}

fn alias_find(parent: &mut HashMap<AliasBit, AliasBit>, x: AliasBit) -> AliasBit {
    let mut cur = x;
    while parent.get(&cur).copied() != Some(cur) {
        match parent.get(&cur).copied() {
            Some(p) => {
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

fn alias_union(
    parent: &mut HashMap<AliasBit, AliasBit>,
    rank: &mut HashMap<AliasBit, u8>,
    a: AliasBit,
    b: AliasBit,
) {
    let ra = alias_find(parent, a);
    let rb = alias_find(parent, b);
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

/// Scale a raw `#N` tick count from the calling module's time unit to
/// design-precision ticks (`N * unit / precision`).  Products beyond the u64
/// range are rejected instead of silently truncating through the `as u64`
/// cast.
fn scale_delay_ticks(
    raw: u64,
    unit_fs: u64,
    design_precision_fs: u64,
    path: &str,
) -> Result<u64, String> {
    if unit_fs == 0 || design_precision_fs == 0 {
        return Err(format!("delay in `{path}` has an invalid time scale"));
    }
    let scaled = (raw as u128)
        .checked_mul(unit_fs as u128)
        .ok_or_else(|| format!("delay `#{raw}` overflows physical time in `{path}`"))?
        / design_precision_fs as u128;
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
/// Convert a captured semantic constant and resolved width to an [`IrConst`].
fn read_const_from(vd: &ValueData, size: i32) -> Result<IrConst, String> {
    if let ValueData::Bytes(bytes) = vd {
        return val_to_const(&bytes_to_value(bytes)?);
    }
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
    bytes_to_value(&decode_verilog_string(value)?)
}

fn decoded_string_bytes(value: &ValueData) -> Result<Vec<u8>, String> {
    match value {
        ValueData::Bytes(bytes) => Ok(bytes.clone()),
        ValueData::Str(value) => decode_verilog_string(value),
        _ => Err("string constant has no byte value".to_owned()),
    }
}

fn decoded_string_text(value: &ValueData, context: &str) -> Result<String, String> {
    String::from_utf8(decoded_string_bytes(value)?).map_err(|_| {
        format!("{context} must contain valid UTF-8; arbitrary bytes are supported only as values")
    })
}

fn bytes_to_value(value: &[u8]) -> Result<elab::Value, String> {
    let mut decoded = value.to_vec();
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

/// Decode legacy source-spelled escapes when the semantic value is textual.
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

// ── Shared lowering helpers and statement state ──────────────────────────────

/// Convert a captured semantic constant and resolved width to a `Val`.
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
    shortreal: bool,
    node: Option<NodeId>,
}

/// Call-site resolution metadata of an emitted function/task: its model
/// index (for `CallFn` nodes) plus the signature pieces lowering needs.
#[derive(Clone)]
struct FuncMeta {
    ir: usize,
    is_task: bool,
    ret: Option<(u32, bool, bool, bool)>,
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

#[derive(Clone)]
enum ProcessTarget {
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
    /// Hidden class-object receiver for a method body. Ordinary functions
    /// and tasks leave this unset.
    class_receiver: Option<IrChandleExpr>,
    /// Return variable context; `None` for void functions and tasks.
    ret: Option<RetCtx>,
    /// io_decl arena node → C expression reading the formal.
    arg_read: HashMap<NodeId, ArgMap>,
    /// io_decl arena node → the IR expression reading the formal (mirrors
    /// `arg_read`; `FormalRead`/substituted bound arguments).
    arg_ir: HashMap<NodeId, IrExpr>,
    /// Event formal → caller handle identity for inlined task activations.
    event_args: HashMap<NodeId, IrEventRef>,
    /// io_decl arena node -> dependencies of a live actual binding. By-value
    /// inputs intentionally have no entry: their event expressions read the
    /// copied activation slot, not the caller's source after invocation.
    arg_dependencies: HashMap<NodeId, Vec<IrDependency>>,
    /// io_decl arena node → complete C address/lvalue expression for output
    /// and inout formals (`o0` for a C-function parameter, `&G_x` for an
    /// inlined task's bound argument).
    arg_write: HashMap<NodeId, String>,
    /// Structured actual lvalues used by inlined reference formals.
    arg_lhs: HashMap<NodeId, Lhs>,
    /// Const-ref formals are readable but deliberately lack a writable
    /// binding so an attempted assignment is rejected during lowering.
    const_refs: HashSet<NodeId>,
    /// Canonical actual lvalues for const-ref formals in inlined task bodies.
    /// These are used only when rebinding the alias for a nested call; writes
    /// still consult `const_refs` and are rejected.
    const_ref_lhs: HashMap<NodeId, Lhs>,
    /// Static formal/local arena node → persistent model storage. Keeping
    /// this structural mapping lets validation and optimization see writes;
    /// `arg_write` alone is an opaque C address.
    persistent: HashMap<NodeId, SignalInfo>,
    /// Chandle formals/return variables remain native pointer values rather
    /// than being encoded as packed integers.
    chandle_read: HashMap<NodeId, IrChandleExpr>,
    chandle_write: HashMap<NodeId, ChandleTarget>,
    /// Process handles remain runtime-owned identities rather than packed
    /// values. Automatic locals use direct C locals; object-backed handles
    /// use the model object table.
    process_read: HashMap<NodeId, crate::sim::ir::IrProcessExpr>,
    process_write: HashMap<NodeId, ProcessTarget>,
    string_read: HashMap<NodeId, crate::sim::ir::IrStringExpr>,
    string_write: HashMap<NodeId, String>,
    /// Native address for a string binding, including const-ref aliases.
    /// `string_write` intentionally remains mutation-only.
    string_addr: HashMap<NodeId, String>,
    /// local var arena node → (C local name, width, signed, two-state).
    locals: HashMap<NodeId, (String, u32, bool, bool, bool)>,
    /// Arena node of the function-name return variable (when captured).
    ret_node: Option<NodeId>,
    /// Arena node of the separately emitted function/task definition, used
    /// when resolving lexical storage. Inlined task contexts have no separate
    /// C function identity; disable targets are runtime activation identities.
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
    /// Names of the enclosing inlined tasks, innermost last.
    chain: Vec<String>,
    /// Whether a `return;` actually emitted a `goto` to `done_label` (the
    /// label is only emitted when set, to keep `-Wall` clean).
    used: bool,
}

/// One entry of the control-flow scope stack while lowering statements:
/// either a named begin block (its runtime activation handles disable),
/// a loop construct (`break`/`continue` jump to their
/// labels), or an inlined task body (a lexical barrier `break`/`continue`
/// must not cross into the caller's loops).  The `*_used` flags keep
/// unused labels out of the C output (`-Wall` warns on them), mirroring
/// [`InlineCtx::used`].
enum CtrlScope {
    Block,
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
    /// Exact process kind for the root process being lowered. Nested
    /// function/task contexts leave this unset; it distinguishes plain
    /// `always @*` from the SystemVerilog implicit-sensitivity forms.
    process_kind: Option<AlwaysKind>,
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
#[derive(Clone)]
enum Lhs {
    Whole(SignalInfo),
    /// A complete C address/lvalue expression (a `sv4_t*` parameter such as
    /// `o0`, or `&_l0` for a local); no `&` is prepended by [`lhs_rhs_code`].
    WholeRef {
        addr: String,
        width: u32,
        signed: bool,
        two_state: bool,
        shortreal: bool,
    },
    /// A canonical reference-formal descriptor supplied by a caller.
    Ref {
        addr: String,
        width: u32,
        signed: bool,
        two_state: bool,
        const_ref: bool,
    },
    /// A fully lowered lvalue retained while an inlined reference formal is
    /// mapped to its caller's actual.  Keeping the typed IR here avoids
    /// rebuilding (or stringifying) selected lvalues during body lowering.
    Canonical(IrLhs),
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
        IrLhs::WholeRef { width, .. } | IrLhs::Ref { width, .. } => *width,
        IrLhs::Bit(..) => 1,
        IrLhs::Part(_, left, right, _) => ((left - right).abs() + 1) as u32,
        IrLhs::IdxPart(_, _, _, width, _, _) => *width,
        IrLhs::ArrayElem { arr, elem_sel, .. } => match elem_sel {
            IrElemSel::Whole => model.array(*arr).elem_width,
            IrElemSel::Part(left, right) => ((left - right).abs() + 1) as u32,
            IrElemSel::Bit(_) => 1,
            IrElemSel::Indexed { width, .. } => *width,
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

/// Lower the arithmetic portion of a compound assignment or increment. The
/// target read is supplied by the mutation expression emitter; keeping this
/// helper independent of `StmtLower` lets expression-valued forms use exactly
/// the same width, signedness, real, and illegal-operation rules.
pub(super) fn lower_compound_expr_ir(
    path: &str,
    op: Operation,
    lhs: IrExpr,
    rhs: IrExpr,
) -> Result<IrExpr, String> {
    let real = lhs.is_real() || rhs.is_real();
    let result = match op {
        Operation::Add | Operation::Subtract | Operation::Multiply => {
            if real {
                let op = match op {
                    Operation::Add => IrRealBinOp::Add,
                    Operation::Subtract => IrRealBinOp::Sub,
                    _ => IrRealBinOp::Mul,
                };
                real_bin_expr(op, lhs, rhs)
            } else {
                let op = match op {
                    Operation::Add => IrBinOp::Add,
                    Operation::Subtract => IrBinOp::Sub,
                    _ => IrBinOp::Mul,
                };
                common_bin_expr(op, lhs, rhs)
            }
        }
        Operation::Divide | Operation::Modulo => {
            if real {
                real_bin_expr(
                    if op == Operation::Divide {
                        IrRealBinOp::Div
                    } else {
                        IrRealBinOp::Mod
                    },
                    lhs,
                    rhs,
                )
            } else {
                common_bin_expr(
                    if op == Operation::Divide {
                        IrBinOp::Div
                    } else {
                        IrBinOp::Mod
                    },
                    lhs,
                    rhs,
                )
            }
        }
        Operation::BitwiseAnd | Operation::BitwiseOr | Operation::BitwiseXor => {
            if real {
                return Err(format!(
                    "bitwise compound assignment on a real value in `{path}` is not supported"
                ));
            }
            common_bin_expr(
                match op {
                    Operation::BitwiseAnd => IrBinOp::BitAnd,
                    Operation::BitwiseOr => IrBinOp::BitOr,
                    _ => IrBinOp::BitXor,
                },
                lhs,
                rhs,
            )
        }
        Operation::ShiftLeft
        | Operation::ShiftRight
        | Operation::ArithmeticShiftLeft
        | Operation::ArithmeticShiftRight => {
            if real {
                return Err(format!(
                    "shift compound assignment on a real value in `{path}` is not supported"
                ));
            }
            let width = lhs.width;
            let signed = lhs.signed;
            IrExpr::new(
                IrExprKind::Bin {
                    op: match op {
                        Operation::ShiftLeft => IrBinOp::Shl,
                        Operation::ShiftRight => IrBinOp::Shr,
                        Operation::ArithmeticShiftLeft => IrBinOp::Ashl,
                        _ => IrBinOp::Ashr,
                    },
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
                "unsupported compound assignment operation {other:?} in `{path}`"
            ))
        }
    };
    Ok(result)
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

/// Reject malformed semantic operations before expression lowering indexes an
/// operand. Both runtime expression lowering and constant evaluation use this
/// boundary, so a partial frontend projection becomes a diagnostic rather
/// than a process panic.
fn validate_operation_arity(
    operation: Operation,
    actual: usize,
    context: &str,
) -> Result<(), String> {
    let Some(expected) = crate::sim::semantic::operation_arity_requirement(operation) else {
        return Ok(());
    };
    if actual < expected.0 || expected.1.is_some_and(|maximum| actual > maximum) {
        return Err(format!(
            "malformed {operation:?} operation in `{context}`: expected {} operands, got {actual}",
            expected.2
        ));
    }
    Ok(())
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
    if width == 0 {
        return e
            .is_real()
            .then_some(e)
            .ok_or_else(|| "real storage requires a real initializer".to_owned());
    }
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

#[cfg(test)]
mod operation_arity_tests {
    use super::*;

    #[test]
    fn malformed_indexed_operations_are_rejected_before_lowering() {
        for (operation, operands) in [
            (Operation::Add, 0),
            (Operation::Add, 1),
            (Operation::Conditional, 2),
            (Operation::MultiConcat, 1),
        ] {
            let error = validate_operation_arity(operation, operands, "top.initial").unwrap_err();
            assert!(error.contains("malformed"));
            assert!(error.contains("top.initial"));
            assert!(error.contains(&format!("got {operands}")));
        }
    }

    #[test]
    fn legal_variable_and_fixed_operation_arities_are_accepted() {
        for (operation, operands) in [
            (Operation::UnaryMinus, 1),
            (Operation::Add, 2),
            (Operation::Conditional, 3),
            (Operation::Concat, 1),
            (Operation::MultiConcat, 2),
            (Operation::StreamLeftToRight, 2),
        ] {
            validate_operation_arity(operation, operands, "top.initial").unwrap();
        }
    }
}

#[cfg(test)]
mod semantic_string_tests {
    use super::*;

    #[test]
    fn native_string_bytes_are_not_escape_decoded_twice() {
        assert_eq!(
            decoded_string_bytes(&ValueData::Bytes(b"line\\n".to_vec())).unwrap(),
            b"line\\n"
        );
        assert_eq!(
            decoded_string_bytes(&ValueData::Str("line\\n".to_owned())).unwrap(),
            b"line\n"
        );
    }

    #[test]
    fn display_format_accepts_native_string_bytes() {
        assert_eq!(
            decoded_string_text(&ValueData::Bytes(b"count=%0d".to_vec()), "$display format")
                .unwrap(),
            "count=%0d"
        );
    }
}
