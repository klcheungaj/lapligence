//! Structured C11 emission for uniquely owned packed values.
//!
//! Each expression emits ordered setup statements and yields one tracked value.
//! Operand payloads are destroyed as soon as the consuming operation completes.
//! Descriptor slots are reused within a procedure; lexical locals use separate
//! registered scopes so queued writes can retain a stable target after exit.

use super::constants::{c_string_literal, emit_const, periodic_constant, round_shortreal};
use super::context::RCtx;
use super::frame_layout::{declaration, FrameLayout, FrameStorage};
use super::names::{
    first_of, identifier_spans_in_comments, marker_starts, named_spans, named_spans_in_comments,
    NameFilter,
};
use crate::sim::execution::{
    CallMechanism, CoroutineId, ExecutionAnalysis, NestedEffects, SuspensionOperation,
};
use crate::sim::ir::*;
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

// A frame outside a coroutine has no coroutine callees.
static NO_FUNCTIONS: BTreeSet<usize> = BTreeSet::new();
static NO_INTERFACE_METHODS: BTreeSet<(usize, usize)> = BTreeSet::new();
static NO_UPPER_BOUNDS: BTreeMap<usize, usize> = BTreeMap::new();

mod activations;
mod assertion_tasks;
pub(super) mod assertions;
mod cached_fields;
mod calls;
mod captures;
mod clocking;
pub(super) mod containers;
mod control;
pub(super) mod coverage;
mod dense_cells;
mod event_waits;
mod events;
mod expressions;
mod fixed_array_cells;
mod fixed_array_reductions;
mod fixed_values;
mod force;
mod formatting;
mod frame_cells;
mod inertial;
mod input;
mod mailboxes;
pub(super) mod model;
pub(super) mod native;
mod native_access;
mod native_tasks;
pub(super) mod native_values;
pub(super) mod net_batches;
mod objects;
mod operands;
mod outline;
pub(super) mod patterns;
mod pure_calls;
mod qualifiers;
pub(super) mod randomize;
mod references;
mod repeated_values;
mod runtime_tasks;
mod scopes;
use scopes::Scopes;
mod statements;
mod storage_names;
use dense_cells::dyn_position;
pub(super) use storage_names::StorageNames;
mod stores;
mod streaming;
mod strings;
mod system;
pub(super) mod udp;

#[must_use]
struct Value {
    code: String,
    width: u32,
    signed: bool,
    fill: Option<u8>,
    slot: Option<usize>,
    borrowed_address: Option<String>,
    /// `code` is a C truth value of width 0, not a real: it can only be
    /// tested, and it is never unknown.
    truth: bool,
}

impl Value {
    /// Address of a packed value: the borrowed source pointer when there is
    /// one, otherwise the address of the owned lvalue.
    fn address(&self) -> String {
        if let Some(address) = &self.borrowed_address {
            address.clone()
        } else if self.slot.is_some() {
            format!("&{}", self.code)
        } else {
            format!("&({})", self.code)
        }
    }
    fn real(&self) -> String {
        if self.width == 0 {
            self.code.clone()
        } else {
            format!("sv4_to_real({})", self.code)
        }
    }
    fn truth(&self) -> String {
        if self.truth {
            self.code.clone()
        } else if self.width == 0 {
            format!("llg_real_to_bool({})", self.code)
        } else {
            format!("sv4_to_bool({})", self.code)
        }
    }
    fn unknown_truth(&self) -> String {
        if self.width == 0 {
            "0".to_owned()
        } else {
            format!("(!sv4_to_bool({0}) && sv4_is_unknown({0}))", self.code)
        }
    }
}

#[derive(Clone)]
struct Binding {
    address: String,
    width: u32,
    signed: bool,
    two_state: bool,
    shortreal: bool,
    automatic: bool,
}

struct Activation {
    exit: String,
    handle: String,
    lexical_depth: usize,
    /// A cancellation check branches to `exit`; the enclosing construct must
    /// check again because an outer activation may be the cancelled one.
    checked: bool,
}

#[derive(Clone)]
struct PendingSite<'a> {
    resume: u32,
    origin: Option<&'a crate::sim::semantic::Origin>,
    operation: SuspensionOperation,
    mechanism: Option<CallMechanism>,
}

/// A finished coroutine function body with its frame layout.
pub(super) struct CoroutineBody {
    pub(super) body: String,
    pub(super) layout: FrameLayout,
    /// Uninitialized C locals mirroring frame fields (see `cached_fields`),
    /// declared before the resume dispatch.
    pub(super) cached_locals: String,
}

#[derive(Clone, Debug)]
struct DeferredDeclaration {
    ty: String,
    name: String,
    standalone: bool,
}

/// How code leaves after an access failed (SV 8.4).
#[derive(Clone, Copy)]
enum FailedAccessExit {
    /// The report ended the run, also where no process runs.
    Stopped,
    /// Through the cleanup of a function that returns normally.
    Plain,
    /// Through the exit of the coroutine.
    Coroutine,
}

impl FailedAccessExit {
    /// The statement that leaves whether or not a process is exiting.
    fn leave(self) -> &'static str {
        match self {
            Self::Stopped | Self::Plain => "goto _llg_return;",
            Self::Coroutine => "return LLG_CO_EXIT;",
        }
    }
}

pub(super) struct Frame<'a, 'm> {
    ctx: &'a RCtx<'m>,
    stack_floor: crate::stack::StackFloor,
    code: String,
    fixed_arrays: HashMap<usize, String>,
    /// Activation native values bound in this frame (`llg_value_t*` expressions).
    native_values: HashMap<usize, String>,
    /// Activation containers bound in this frame (container lvalue
    /// expressions, such as `(*ptr)`).
    containers: HashMap<usize, String>,
    slots: Vec<bool>,
    next_name: usize,
    bindings: Scopes<Binding>,
    event_bindings: Scopes<String>,
    /// The C variables of the jump states declared so far.
    jump_states: HashMap<String, String>,
    /// Locals that live in a shared activation frame (`IrStmt::SharedLocal`,
    /// `StorageOwnership::Shared` captures): name -> (frame expression, slot).
    shared_cells: HashMap<String, (String, u32)>,
    native_bindings: Scopes<native::NativeBinding>,
    /// Which rendered statements can end the simulation.
    nested_exits: NestedEffects,
    formal_overrides: Vec<Vec<Binding>>,
    callback_signal_overrides: Vec<HashMap<usize, usize>>,
    /// Per active inline call, the signal that is the function's result
    /// variable: the callback writes its private `_ret` cell instead, so an
    /// evaluation changes nothing a wait, dump or foreign reader can see.
    callback_results: Vec<Option<usize>>,
    marks: Vec<String>,
    mark_blocks: Vec<usize>,
    labels: Vec<HashMap<String, bool>>,
    temp_roots: Vec<Vec<bool>>,
    return_address: Option<String>,
    /// `return` publishes through the ordinary store (a dumped static result).
    return_publishes: bool,
    allow_calls: bool,
    read_only_callback: bool,
    /// Between `IrContainerStmt::NbaBindOpen` and `NbaBindClose`.
    nba_bound: bool,
    /// A `$strobe`/`$monitor` evaluator, run only in the read-only Postponed
    /// region. A user VPI function may run there: the runtime rejects its
    /// design-state writes (vpi_put_value in a read-only region), so it can
    /// only read values and set its own result.
    postponed_reader: bool,
    /// Emitting a deferred assertion action (SV 16.4): a Reactive callback
    /// with no process turn, so `$stop` uses the action entry point.
    deferred_action: bool,
    /// The update of an event-controlled nonblocking assignment, which runs
    /// outside every process.
    event_update: bool,
    sampled_reads: bool,
    item_callback: bool,
    sequence_addresses: HashMap<String, Binding>,
    activations: Vec<Activation>,
    /// The next activation scope is a fork branch's whole body
    /// (`llg_activation_exit_terminal`); consumed by that scope.
    terminal_activation: bool,
    cancellation_return: bool,
    /// Count of emitted cancellation points: resume points, `disable` and
    /// calls that may disable. A cancellation check is emitted only when this
    /// changed since the start of the construct it follows (design §13.2
    /// rule 1): between such points no activation of a running process can
    /// become cancelled.
    cancellation_points: usize,
    access_stack: Vec<String>,
    /// Resolving a wait dependency's native-access receiver: null checks
    /// and class-property lookups yield null or scratch storage silently,
    /// because arming a wait on an unbound handle is not an access.
    quiet_receivers: bool,
    /// Rendering `llg_model_initializers`: no process waits yet, so stores
    /// need not publish change markers, and the frame's `_llg_return` is an
    /// exit path for its initializer expressions.
    pub(super) static_initialization: bool,
    /// Nesting depth of statements being rendered. Expression code outside
    /// any statement (read-only callbacks, evaluators) has no exit path.
    statement_depth: usize,
    /// Statements publishing stores made in place during the current
    /// statement (see `publish_touches`).
    pending_touches: Vec<String>,
    /// Null checks of the handles behind property containers named since
    /// the last line (see `require_container_receivers`). Naming a container
    /// emits nothing, so its check runs just before the line that uses it.
    container_receivers: std::cell::RefCell<Vec<String>>,
    /// The next access resolved is a read (see `read_lookup`).
    access_read: bool,
    construction_stack: Vec<usize>,
    layout: FrameLayout,
    declarations: Vec<DeferredDeclaration>,
    /// Native locals of a C-stack body whose use is decided in `body`
    /// (see `check_native_use`).
    native_use_checks: usize,
    declaration_error: Option<String>,
    brace_kinds: Vec<bool>,
    in_block_comment: bool,
    structural_error: Option<String>,
    resume_probe: bool,
    pending_sites: VecDeque<PendingSite<'a>>,
    resume_numbers: Vec<u32>,
    frame_upper_bounds: &'a BTreeMap<usize, usize>,
    coroutine_functions: &'a BTreeSet<usize>,
    cached_fields: cached_fields::CachedFields,
    net_batches: net_batches::ProcessTables,
    /// Process C name that owns this frame's file-scope batch tables.
    batch_owner: Option<String>,
    cell_eligibility: frame_cells::CellEligibility,
    stack_cells: String,
    /// Calls of a recursive subprogram's coroutine that enter the chain
    /// arena (see `execution::recursion`); empty for every other frame.
    recursive_targets: BTreeSet<crate::sim::execution::CallTarget>,
    /// Class virtual slots with a suspending implementation; dispatch through
    /// them is an arena call site (SIM-011).
    suspendable_slots: &'a BTreeSet<usize>,
    /// Virtual-interface methods whose dispatch may suspend (SIM-012).
    suspendable_interface_methods: &'a BTreeSet<(usize, usize)>,
    /// A recursive subprogram's coroutine, run by a synchronous driver: it
    /// keeps plain-function exit semantics and numbers its resume points
    /// (all arena calls) during emission.
    synchronous: bool,
    /// `_llg_frame_base`, `_llg_temp_scope` and `_llg_t` are frame fields.
    scope_fields: bool,
    /// The renderer's model storage index; frames without one scan the model.
    storage_names: Option<&'a StorageNames<'m>>,
    /// Outlined statement runs of this body (`outline.rs`); `None` keeps
    /// every statement inline.
    outline: Option<outline::OutlineSink>,
    /// Rendering a statement heavier than one outlined run: its statement
    /// sequences are split even when each is lighter than a run.
    outline_pressure: bool,
    /// Rendering a sequence no heavier than one run: nothing inside it splits.
    outline_light: bool,
    /// VPI frame automatics (37.39): the activation's address array and the
    /// entry of each body-scope local, stored where the local is declared.
    /// A block local's entry is flagged: its declaration binds it without
    /// making its scope the frame's.
    vpi_autos: Option<(String, VpiAutoEntries)>,
    /// The bound entries of block locals, with the lexical depth of their
    /// declarations: leaving that scope clears them (decision A19-D88).
    vpi_block_bound: Vec<(usize, usize)>,
    /// The serial of the VPI frame the body runs in (37.39), while it is
    /// rendered.
    vpi_frame_serial: Option<String>,
    /// The lexical depth (`marks.len()`) of the scope holding the frame's
    /// body locals, once a declaration stored one's address.
    vpi_auto_depth: Option<usize>,
    /// The subroutine whose `ref` formals a fork-join branch names through
    /// captured descriptors (`rN`); `None` outside such branches.
    branch_formals: Option<&'m IrFunc>,
}

/// The address-array entry of each body-scope automatic, flagged when it is
/// a block local (decision A19-D88).
type VpiAutoEntries = HashMap<VpiAutoKey, (usize, bool)>;

/// A body-scope automatic a declaration binds: a local by IR name, or an
/// automatic container.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum VpiAutoKey {
    Name(String),
    Container(usize),
    /// A fixed-size activation array: its `double` cells or its
    /// `llg_fixed_array_t`.
    Array(usize),
    /// An activation record value: its `llg_value_t`.
    NativeValue(usize),
}

/// Whether a conditional's predicate cannot be X or Z by its operator, so
/// its ambiguous arm (SV 11.4.11) is unreachable.
fn known_predicate(predicate: &IrExpr) -> bool {
    matches!(
        predicate.kind(),
        IrExprKind::Bin {
            op: IrBinOp::CaseEq | IrBinOp::CaseNeq | IrBinOp::IndexEq | IrBinOp::IndexLt,
            ..
        }
    )
}

/// Whether a conditional's predicate may be X or Z: a packed value of any
/// other operator. A real or C truth value never is.
fn ambiguous_predicate(predicate: &IrExpr) -> bool {
    predicate.width != 0 && !known_predicate(predicate)
}

fn pending(feature: &str) -> String {
    format!("P05 ownership emission is not yet implemented for {feature}; no C model was emitted")
}

impl<'a, 'm> Frame<'a, 'm> {
    /// The subroutine whose formals this frame names: the function being
    /// rendered, or the subroutine of a fork-join branch that names its
    /// `ref` formals.
    fn formal_function(&self) -> Option<&'m IrFunc> {
        self.ctx.func.or(self.branch_formals)
    }

    pub(super) fn new(ctx: &'a RCtx<'m>) -> Self {
        Self::with_storage(ctx, FrameStorage::CStack)
    }

    /// Refuse to emit a nested operation on a stack that holds no more of
    /// them (`crate::stack`). Syntax alone never gets there.
    fn check_stack(&self) -> Result<(), String> {
        if self.stack_floor.reached() {
            return Err(crate::stack::EXHAUSTED.to_owned());
        }
        Ok(())
    }

    pub(super) fn new_coframe(
        ctx: &'a RCtx<'m>,
        analysis: &'a ExecutionAnalysis,
        owner: CoroutineId,
        frame_upper_bounds: &'a BTreeMap<usize, usize>,
    ) -> Result<Self, String> {
        let mut frame = Self::with_storage(ctx, FrameStorage::CoFrame);
        frame.frame_upper_bounds = frame_upper_bounds;
        frame.coroutine_functions = analysis.coroutine_functions();
        frame.suspendable_slots = analysis.suspendable_dispatch_slots();
        frame.suspendable_interface_methods = analysis.suspendable_interface_methods();
        let mut sites = analysis
            .sites(owner)
            .into_iter()
            .flat_map(|sites| sites.values())
            .map(|site| PendingSite {
                resume: site.resume(),
                origin: site.origin(),
                operation: site.operation().clone(),
                mechanism: site.mechanism(),
            })
            .collect::<Vec<_>>();
        sites.sort_by_key(|site| site.resume);
        frame.resume_numbers = sites.iter().map(|site| site.resume).collect();
        frame.pending_sites = sites.into();
        if !frame.resume_numbers.is_empty() {
            frame.declare_scope_fields()?;
        }
        Ok(frame)
    }

    /// Frame for the coroutine of recursive subprogram `function`. Its resume
    /// points are exactly its arena calls, numbered as they are emitted.
    pub(super) fn new_recursive(
        ctx: &'a RCtx<'m>,
        analysis: &'a ExecutionAnalysis,
        function: usize,
    ) -> Result<Self, String> {
        let mut frame = Self::with_storage(ctx, FrameStorage::CoFrame);
        frame.coroutine_functions = analysis.coroutine_functions();
        frame.suspendable_slots = analysis.suspendable_dispatch_slots();
        frame.suspendable_interface_methods = analysis.suspendable_interface_methods();
        frame.recursive_targets = analysis
            .effects()
            .call_targets(function)
            .iter()
            .filter(|target| analysis.is_recursive_call(ctx.model, function, target))
            .copied()
            .collect();
        frame.synchronous = true;
        frame.declare_scope_fields()?;
        Ok(frame)
    }

    fn declare_scope_fields(&mut self) -> Result<(), String> {
        for (ty, name) in [
            ("llg_value_scope_t*", "_llg_frame_base"),
            ("llg_value_scope_t*", "_llg_temp_scope"),
            ("sv4_t*", "_llg_t"),
        ] {
            self.layout.declare(ty, name)?;
        }
        // `_llg_temp_scope` is only ever written, so it has nothing to cache.
        for (ty, name) in [
            ("llg_value_scope_t*", "_llg_frame_base"),
            ("sv4_t*", "_llg_t"),
        ] {
            self.cached_fields.register(ty, name, true);
        }
        self.scope_fields = true;
        Ok(())
    }

    /// Allocate the next resume number of a synchronous coroutine.
    fn next_recursive_resume(&mut self) -> u32 {
        let resume = u32::try_from(self.resume_numbers.len() + 1).unwrap_or(u32::MAX);
        self.resume_numbers.push(resume);
        resume
    }

    /// Number of resume points of a finished synchronous coroutine.
    pub(super) fn recursive_sites(&self) -> usize {
        self.resume_numbers.len()
    }

    /// Leave at once when `failed`, a C condition that holds only after an
    /// access through a null class handle (SV 8.4) or a null or unbound
    /// virtual interface (25.9) reported its fatal run-time error: nothing
    /// after the failed access runs, not even the rest of its statement.
    /// Checking the failed value keeps the success path to one register
    /// compare.
    fn exit_on_failed_access(&mut self, failed: &str) {
        let Some(leave) = self.failed_access_exit() else {
            return;
        };
        self.line(match leave {
            FailedAccessExit::Stopped => {
                format!("if (LLG_CO_UNLIKELY({failed})) goto _llg_return;")
            }
            FailedAccessExit::Plain => format!(
                "if (LLG_CO_UNLIKELY({failed})) {{ if (llg_rt_exiting()) goto _llg_return; }}"
            ),
            FailedAccessExit::Coroutine => {
                format!("if (LLG_CO_UNLIKELY({failed})) LLG_CO_EXIT_CHECK(ch);")
            }
        });
    }

    /// How code leaves after an access failed, where it can.
    fn failed_access_exit(&self) -> Option<FailedAccessExit> {
        // A failed access in a report evaluator already ended the run (SV
        // 8.4): the evaluator stops and the report prints nothing, and the
        // update of an event-controlled assignment writes nothing.
        if (self.postponed_reader || self.event_update) && !self.quiet_receivers {
            return Some(FailedAccessExit::Stopped);
        }
        if !self.has_exit_path() || self.read_only_callback || self.quiet_receivers {
            return None;
        }
        Some(if self.plain_exits() {
            FailedAccessExit::Plain
        } else {
            FailedAccessExit::Coroutine
        })
    }

    /// Leave at once after a plain call (one that returns instead of
    /// propagating `LLG_CO_EXIT`) whose callee may have requested
    /// termination: a fatal access, `$finish` or a budget stop inside it
    /// returns early, and the caller must not run on with its result
    /// (`llg_rt.h`: callers check immediately).
    fn exit_after_call(&mut self, function: usize, dynamic: bool) {
        if !self.has_exit_path() || self.read_only_callback || self.quiet_receivers {
            return;
        }
        let may_terminate = dynamic
            || self
                .ctx
                .effects
                .of_running(function)
                .contains(crate::sim::execution::ExecutionEffect::Terminate);
        if !may_terminate {
            return;
        }
        if self.plain_exits() {
            self.line("if (LLG_CO_UNLIKELY(llg_rt_exiting())) goto _llg_return;");
        } else {
            self.line("LLG_CO_EXIT_CHECK(ch);");
        }
    }

    /// Whether code rendered now can leave the frame: inside a statement, or
    /// anywhere in the static initialization frame, whose declaration
    /// initializer expressions run outside any statement but still stop at
    /// a `$finish`, `$fatal` or run-ending error inside them.
    fn has_exit_path(&self) -> bool {
        self.statement_depth > 0 || self.static_initialization
    }

    /// Whether this frame exits like a plain function (`goto _llg_return`
    /// after termination) rather than by returning `LLG_CO_EXIT`.
    fn plain_exits(&self) -> bool {
        self.layout.storage() == FrameStorage::CStack || self.synchronous
    }

    fn with_storage(ctx: &'a RCtx<'m>, storage: FrameStorage) -> Self {
        Self {
            ctx,
            stack_floor: crate::stack::StackFloor::current(),
            code: String::new(),
            fixed_arrays: HashMap::new(),
            native_values: HashMap::new(),
            containers: HashMap::new(),
            slots: Vec::new(),
            next_name: 0,
            bindings: Scopes::new(),
            event_bindings: Scopes::new(),
            jump_states: HashMap::new(),
            shared_cells: HashMap::new(),
            native_bindings: Scopes::new(),
            nested_exits: NestedEffects::default(),
            formal_overrides: Vec::new(),
            callback_signal_overrides: Vec::new(),
            callback_results: Vec::new(),
            marks: Vec::new(),
            mark_blocks: Vec::new(),
            labels: Vec::new(),
            temp_roots: Vec::new(),
            return_address: None,
            return_publishes: false,
            allow_calls: true,
            read_only_callback: false,
            nba_bound: false,
            branch_formals: None,
            postponed_reader: false,
            deferred_action: false,
            event_update: false,
            sampled_reads: ctx.sampled,
            item_callback: false,
            sequence_addresses: HashMap::new(),
            activations: Vec::new(),
            terminal_activation: false,
            cancellation_return: false,
            cancellation_points: 0,
            access_stack: Vec::new(),
            quiet_receivers: false,
            static_initialization: false,
            statement_depth: 0,
            pending_touches: Vec::new(),
            container_receivers: Default::default(),
            access_read: false,
            construction_stack: Vec::new(),
            layout: FrameLayout::with_backend(storage, ctx.value_backend),
            declarations: Vec::new(),
            native_use_checks: 0,
            declaration_error: None,
            brace_kinds: Vec::new(),
            in_block_comment: false,
            structural_error: None,
            resume_probe: false,
            pending_sites: VecDeque::new(),
            resume_numbers: Vec::new(),
            frame_upper_bounds: &NO_UPPER_BOUNDS,
            coroutine_functions: &NO_FUNCTIONS,
            cached_fields: cached_fields::CachedFields::default(),
            net_batches: net_batches::ProcessTables::default(),
            batch_owner: None,
            cell_eligibility: frame_cells::CellEligibility::default(),
            stack_cells: String::new(),
            recursive_targets: BTreeSet::new(),
            suspendable_slots: &NO_FUNCTIONS,
            suspendable_interface_methods: &NO_INTERFACE_METHODS,
            synchronous: false,
            scope_fields: false,
            storage_names: None,
            outline: None,
            outline_pressure: false,
            outline_light: false,
            vpi_autos: None,
            vpi_block_bound: Vec::new(),
            vpi_frame_serial: None,
            vpi_auto_depth: None,
        }
    }
    /// Stop before a line that uses a property container of a null handle:
    /// such a container has no storage to continue on (SV 8.4).
    fn require_container_receivers(&mut self) {
        for handle in std::mem::take(self.container_receivers.get_mut()) {
            let required = self.scalar("void*", handle);
            self.exit_on_failed_access(&format!("!{required}"));
        }
    }

    fn line(&mut self, text: impl AsRef<str>) {
        if !self.container_receivers.get_mut().is_empty() {
            self.require_container_receivers();
        }
        let text = text.as_ref();
        if self.resume_probe {
            self.mark_continuation_fields(text);
            if has_statement_boundary(text) {
                self.resume_probe = false;
            }
        }
        let exits = if self.layout.storage() == FrameStorage::CoFrame {
            self.track_frame_blocks(text)
        } else {
            Vec::new()
        };
        for block in exits {
            self.poison_block(block);
        }
        let rewritten = (self.layout.storage() == FrameStorage::CStack)
            .then(|| self.rewrite_frame_accesses(text));
        let text = rewritten.as_deref().unwrap_or(text);
        self.code.push_str("    ");
        self.code.push_str(text);
        self.code.push('\n');
        if is_runtime_suspension(text) {
            match resume_state(text) {
                Some(state) => self.layout.mark_resume_state(state),
                None => self.layout.mark_resume(),
            }
            self.resume_probe = true;
            self.cancellation_points += 1;
            if self.layout.storage() == FrameStorage::CoFrame {
                // A resume lands inside the suspension macro with every cached
                // local indeterminate; the frame holds the only surviving copy.
                if let Some(reload) = self.cached_fields.reload_placeholder() {
                    self.code.push_str(&reload);
                }
            }
        }
    }

    fn mark_continuation_fields(&mut self, text: &str) {
        for span in identifier_spans_in_comments(text) {
            self.layout.mark_hot(&text[span]);
        }
    }

    /// Mirror the structural braces in emitted C. Aggregate initializer and
    /// compound-literal braces are balanced too, but do not create frame
    /// blocks. This single text path makes a missing block event an error
    /// rather than silently producing a layout that differs from the C body.
    fn track_frame_blocks(&mut self, text: &str) -> Vec<usize> {
        let mut exits = Vec::new();
        let bytes = text.as_bytes();
        // Most lines neither hold a brace nor open a comment.
        if !self.in_block_comment && first_of(bytes, 0, *b"{}/").is_none() {
            return exits;
        }
        let mut index = 0;
        let mut segment_start = 0;
        let mut paren_depth = 0usize;
        let mut quoted = None;
        let mut escaped = false;
        while index < bytes.len() {
            if self.in_block_comment {
                if bytes[index..].starts_with(b"*/") {
                    self.in_block_comment = false;
                    index += 2;
                } else {
                    index += 1;
                }
                continue;
            }
            if let Some(quote) = quoted {
                let byte = bytes[index];
                index += 1;
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == quote {
                    quoted = None;
                }
                continue;
            }
            match bytes[index] {
                b'/' if bytes.get(index + 1) == Some(&b'/') => break,
                b'/' if bytes.get(index + 1) == Some(&b'*') => {
                    self.in_block_comment = true;
                    index += 2;
                    continue;
                }
                b'\'' | b'"' => quoted = Some(bytes[index]),
                b'(' | b'[' => paren_depth += 1,
                b')' | b']' => paren_depth = paren_depth.saturating_sub(1),
                b';' if paren_depth == 0 => segment_start = index + 1,
                b'{' => {
                    let inside_initializer = self.brace_kinds.last() == Some(&false);
                    let segment = text[segment_start..index].trim();
                    let structural = !inside_initializer && is_structural_block_open(segment);
                    self.brace_kinds.push(structural);
                    if structural {
                        self.layout.begin_block();
                        self.cached_fields.open_block();
                    }
                    segment_start = index + 1;
                }
                b'}' => {
                    match self.brace_kinds.pop() {
                        Some(true) => {
                            exits.push(self.layout.current_block());
                            self.cached_fields.close_block();
                            if let Err(error) = self.layout.end_block() {
                                self.structural_error.get_or_insert(error);
                            }
                        }
                        Some(false) => {}
                        None => {
                            self.structural_error.get_or_insert_with(|| {
                                "generated coroutine body closed an unmatched C brace".to_owned()
                            });
                        }
                    }
                    segment_start = index + 1;
                }
                _ => {}
            }
            index += 1;
        }
        exits
    }

    fn poison_block(&mut self, block: usize) {
        if self.layout.storage() == FrameStorage::CoFrame {
            self.code
                .push_str(&format!("/*__llg_poison_block_{block}__*/\n"));
        }
    }

    fn poison_loop_exit(&mut self, condition: &str) {
        if self.layout.storage() == FrameStorage::CoFrame {
            let block = self.layout.current_block();
            self.code
                .push_str(&format!("/*__llg_poison_block_{block}__*/{condition}\n"));
        }
    }

    fn poison_completed_frame(&mut self) {
        if self.layout.storage() == FrameStorage::CoFrame {
            self.line("#ifdef LLG_CO_DEBUG");
            self.line("LLG_CO_DEBUG_POISON_FRAME(F, sizeof(*F));");
            self.line("#endif");
        }
    }

    fn rewrite_frame_accesses<'t>(&self, text: &'t str) -> Cow<'t, str> {
        let coframe = self.layout.storage() == FrameStorage::CoFrame;
        // A C-stack body has no frame fields.
        if !coframe && !text.contains("self") {
            return Cow::Borrowed(text);
        }
        let bytes = text.as_bytes();
        let mut out = String::with_capacity(text.len() + 16);
        let mut copied = 0;
        let mut names = self.layout.field_names();
        names.insert("self");
        for span in named_spans_in_comments(text, names) {
            if span.start > 0 && matches!(bytes[span.start - 1], b'.' | b'>') {
                continue;
            }
            let ident = &text[span.clone()];
            let (prefix, replacement) = if ident == "self" {
                (
                    "",
                    if coframe && !self.synchronous {
                        "LLG_CO_OWNER(ch, llg_proc_t)"
                    } else {
                        "llg_current()"
                    },
                )
            } else {
                match self.layout.field_access(ident) {
                    Some(access) if !self.cached_fields.is_cached(ident) => ("F->", access),
                    _ => continue,
                }
            };
            out.push_str(&text[copied..span.start]);
            out.push_str(prefix);
            out.push_str(replacement);
            copied = span.end;
        }
        out.push_str(&text[copied..]);
        Cow::Owned(out)
    }
    fn name(&mut self, purpose: &str) -> String {
        let result = format!("_llg_{purpose}_{}", self.next_name);
        self.next_name += 1;
        result
    }
    fn access(&self, name: &str) -> String {
        name.to_owned()
    }
    fn frame_field(&mut self, ty: &str, name: &str) -> Result<String, String> {
        self.layout.declare_required(ty, name)
    }
    fn take_call_slot(
        &mut self,
        callee: usize,
        callee_type: &str,
    ) -> Result<super::frame_layout::CallSlot, String> {
        let plan = self
            .pending_sites
            .pop_front()
            .ok_or_else(|| format!("missing coroutine call-site analysis for callee {callee}"))?;
        let SuspensionOperation::Call {
            callee: expected_callee,
        } = plan.operation
        else {
            return Err(format!(
                "coroutine suspension site {} is {:?}, emitted call to callee {callee}",
                plan.resume, plan.operation
            ));
        };
        if expected_callee.is_some_and(|expected| expected != callee) {
            return Err(format!(
                "coroutine call-site analysis expected callee {:?}, emitted {callee}",
                expected_callee
            ));
        }
        let mechanism = plan
            .mechanism
            .ok_or_else(|| format!("coroutine call site {} has no mechanism", plan.resume))?;
        let upper_bound = if mechanism == CallMechanism::Arena {
            0
        } else {
            self.frame_upper_bounds
                .get(&callee)
                .copied()
                .ok_or_else(|| format!("missing frame upper bound for coroutine callee {callee}"))?
        };
        self.layout
            .add_call(plan.resume, callee_type, mechanism, upper_bound)
    }
    fn take_resume(&mut self, operation: SuspensionOperation) -> Result<u32, String> {
        let site = self
            .pending_sites
            .pop_front()
            .ok_or_else(|| format!("emitted unnumbered coroutine suspension site {operation:?}"))?;
        if site.operation != operation {
            return Err(format!(
                "coroutine suspension site {} is {:?}, emitted {:?}",
                site.resume, site.operation, operation
            ));
        }
        Ok(site.resume)
    }
    fn await_arm(
        &mut self,
        operation: SuspensionOperation,
        arm: impl AsRef<str>,
    ) -> Result<(), String> {
        let resume = self.take_resume(operation)?;
        self.line(format!("LLG_CO_AWAIT(co, ch, {resume}, {});", arm.as_ref()));
        Ok(())
    }
    /// [`Self::await_arm`] of an arm that also exits when the handle it
    /// operates on is null: that is reported as `failure` at `site`, a C
    /// literal.
    fn await_handle_arm(
        &mut self,
        operation: SuspensionOperation,
        arm: &str,
        failure: &str,
        site: &str,
    ) -> Result<(), String> {
        let resume = self.take_resume(operation)?;
        self.line(format!(
            "LLG_CO_AWAIT_OR(co, ch, {resume}, {arm}, llg_rt_failed_access(\"{failure}\", {site}));"
        ));
        Ok(())
    }
    /// [`Self::await_handle_arm`] of a wait on events.
    fn await_event_arm(
        &mut self,
        operation: SuspensionOperation,
        access: &str,
        arm: impl AsRef<str>,
    ) -> Result<(), String> {
        let site = match self.pending_sites.front().and_then(|site| site.origin) {
            Some(origin) => format!("{access}\u{1f}{}", origin.location()),
            None => access.to_owned(),
        };
        self.await_handle_arm(
            operation,
            arm.as_ref(),
            "null event handle access",
            &c_string_literal(&site),
        )
    }
    fn dispatch(&self) -> String {
        let mut dispatch = String::from("    LLG_CO_DISPATCH_BEGIN(co)\n");
        for resume in &self.resume_numbers {
            dispatch.push_str(&format!("    LLG_CO_RESUME_CASE({resume})\n"));
        }
        dispatch.push_str("    LLG_CO_DISPATCH_END(co)\n");
        dispatch
    }
    fn declare_named(&mut self, ty: &str, name: &str, init: String) -> String {
        self.declare_named_with(ty, name, init, false)
    }
    /// Like `declare_named`, for a pointer that is assigned only here and read
    /// afterwards: a frame-resident field is then mirrored by a C local.
    fn declare_named_cached(&mut self, ty: &str, name: &str, init: String) -> String {
        self.declare_named_with(ty, name, init, true)
    }
    fn declare_named_with(&mut self, ty: &str, name: &str, init: String, cache: bool) -> String {
        let registered = match self.layout.declare(ty, name) {
            Ok(access) => {
                if cache && self.layout.storage() == FrameStorage::CoFrame {
                    self.cached_fields.register(ty, name, false);
                }
                access
            }
            Err(error) => {
                self.declaration_error.get_or_insert(error);
                name.to_owned()
            }
        };
        if self.layout.storage() == FrameStorage::CoFrame {
            let init = if init.trim_start().starts_with('{') {
                format!("({ty}){init}")
            } else {
                init
            };
            let declaration_index = self.declarations.len();
            let target = self.defer_declaration(ty, name, false);
            self.line(format!(
                "{target} = {init}; /*__llg_local_use_{declaration_index}__*/"
            ));
            name.to_owned()
        } else {
            self.line(format!("{} = {init};", declaration(ty, name)));
            registered
        }
    }
    fn declare(&mut self, ty: &str, purpose: &str, init: String) -> String {
        let name = self.name(purpose);
        self.declare_named(ty, &name, init)
    }
    fn loop_variable(&mut self, ty: &str, purpose: &str) -> (String, String) {
        let name = self.name(purpose);
        if self.layout.storage() == FrameStorage::CoFrame {
            match self.layout.declare(ty, &name) {
                Ok(_) => {}
                Err(error) => {
                    self.declaration_error.get_or_insert(error);
                }
            }
            let target = self.defer_declaration(ty, &name, false);
            (name, target)
        } else {
            (name.clone(), declaration(ty, &name))
        }
    }
    fn declaration_target_named(&mut self, ty: &str, name: &str) -> (String, String) {
        if self.layout.storage() == FrameStorage::CoFrame {
            match self.layout.declare(ty, name) {
                Ok(_) => {}
                Err(error) => {
                    self.declaration_error.get_or_insert(error);
                }
            }
            let target = self.defer_declaration(ty, name, false);
            (name.to_owned(), target)
        } else {
            (name.to_owned(), declaration(ty, name))
        }
    }
    fn declare_array(&mut self, ty: &str, purpose: &str, count: usize) -> String {
        let name = self.name(purpose);
        let array_ty = format!("{ty}[{count}]");
        let registered = match self.layout.declare(&array_ty, &name) {
            Ok(access) => access,
            Err(error) => {
                self.declaration_error.get_or_insert(error);
                name.clone()
            }
        };
        if self.layout.storage() == FrameStorage::CoFrame {
            let declaration = self.defer_declaration(&array_ty, &name, true);
            self.line(format!("{declaration};"));
            self.line(format!("memset({name}, 0, sizeof({name}));"));
            name
        } else {
            self.line(format!("{ty} {name}[{count}] = {{0}};"));
            registered
        }
    }
    fn declare_array_init(
        &mut self,
        ty: &str,
        purpose: &str,
        count: usize,
        entries: &str,
    ) -> String {
        let name = self.name(purpose);
        let array_ty = format!("{ty}[{count}]");
        let registered = match self.layout.declare(&array_ty, &name) {
            Ok(access) => access,
            Err(error) => {
                self.declaration_error.get_or_insert(error);
                name.clone()
            }
        };
        if self.layout.storage() == FrameStorage::CoFrame {
            let declaration = self.defer_declaration(&array_ty, &name, true);
            self.line(format!("{declaration};"));
            self.line(format!(
                "memcpy({name}, ({ty}[]){{ {entries} }}, sizeof({name}));"
            ));
            name
        } else {
            self.line(format!("{ty} {name}[{count}] = {{ {entries} }};"));
            registered
        }
    }

    /// Array argument of a runtime arm that copies it before returning (see
    /// the copy contract on each arm in `llg_rt.h`). A coroutine passes it as
    /// a compound literal, which lives for the enclosing block and therefore
    /// covers the arm call, so it needs neither a frame field nor a `memcpy`.
    /// `count` must be nonzero.
    fn arm_array(&mut self, ty: &str, purpose: &str, count: usize, entries: &str) -> String {
        if self.layout.storage() == FrameStorage::CoFrame {
            format!("({ty}[]){{ {entries} }}")
        } else {
            self.declare_array_init(ty, purpose, count, entries)
        }
    }

    fn defer_declaration(&mut self, ty: &str, name: &str, standalone: bool) -> String {
        let index = self.declarations.len();
        self.declarations.push(DeferredDeclaration {
            ty: ty.to_owned(),
            name: name.to_owned(),
            standalone,
        });
        format!("__llg_declaration_{index}__")
    }
    fn scalar(&mut self, ty: &str, code: String) -> String {
        self.declare(ty, "scalar", code)
    }
    fn reserve(&mut self, width: u32, signed: bool) -> Value {
        let slot = if let Some(index) = self.slots.iter().position(|used| !used) {
            self.slots[index] = true;
            index
        } else {
            self.slots.push(true);
            self.slots.len() - 1
        };
        Value {
            code: format!("_llg_t[{slot}]"),
            width,
            signed,
            fill: None,
            slot: Some(slot),
            borrowed_address: None,
            truth: false,
        }
    }
    fn value(&mut self, code: String, width: u32, signed: bool) -> Value {
        if width == 0 {
            return Value {
                code: self.scalar("double", code),
                width,
                signed,
                fill: None,
                slot: None,
                borrowed_address: None,
                truth: false,
            };
        }
        let value = self.reserve(width, signed);
        self.assign(&format!("&{}", value.code), &code);
        value
    }
    /// Replace the initialized packed owner at `destination` (an address
    /// expression) with `producer`'s fresh result, in destination-passing
    /// form when the producer has one (see `emit_c::destinations`).
    fn assign(&mut self, destination: &str, producer: &str) {
        self.line(super::destinations::assign(destination, producer));
    }
    fn discard(&mut self, value: Value) {
        if let Some(slot) = value.slot {
            self.line(format!("sv4_destroy(&{});", value.code));
            self.slots[slot] = false;
        } else if value.width == 0 && !value.truth {
            self.line(format!("(void){};", value.code));
        }
    }
    /// Free the slot of a packed owner that the generated code left empty on
    /// every path: unfilled, moved from or destroyed.
    fn release_emptied(&mut self, value: Value) {
        if let Some(slot) = value.slot {
            self.slots[slot] = false;
        }
    }
    fn replace(&mut self, mut value: Value, code: String, width: u32, signed: bool) -> Value {
        if value.slot.is_some() && width != 0 {
            self.assign(&format!("&{}", value.code), &code);
            value.width = width;
            value.signed = signed;
            value.fill = None;
            value
        } else {
            let result = self.value(code, width, signed);
            self.discard(value);
            result
        }
    }
    fn convert(
        &mut self,
        value: Value,
        width: u32,
        signed: bool,
        two_state: bool,
        shortreal: bool,
    ) -> Value {
        let same_shape =
            width != 0 && value.fill.is_none() && value.width == width && value.signed == signed;
        if width != 0 {
            if let Some(fill) = value.fill {
                if self.ctx.constants.is_some() {
                    let result = self.packed_fill(fill, width, signed, value.slot.is_none());
                    self.discard(value);
                    return if two_state {
                        let code = format!("sv4_to_two_state({})", result.code);
                        self.replace(result, code, width, signed)
                    } else {
                        result
                    };
                }
            }
        }
        let code = if width == 0 {
            round_shortreal(value.real(), shortreal)
        } else if let Some(fill) = value.fill {
            format!("sv4_fill({fill}, {width}, {})", u8::from(signed))
        } else if value.width == 0 {
            format!(
                "sv4_from_real({}, {width}, {})",
                value.code,
                u8::from(signed)
            )
        } else {
            format!("sv4_cast({}, {width}, {})", value.code, u8::from(signed))
        };
        let result = if same_shape {
            value
        } else {
            self.replace(value, code, width, signed)
        };
        if width != 0 && two_state {
            let code = format!("sv4_to_two_state({})", result.code);
            self.replace(result, code, width, signed)
        } else {
            result
        }
    }
    /// The address of the packed, real, string or handle cell `name` is
    /// bound to, for a VPI frame's automatics.
    fn vpi_binding_address(&self, name: &str) -> Option<String> {
        if let Some(binding) = self.lookup(name) {
            return Some(binding.address);
        }
        self.native_bindings
            .get(name)
            .map(|binding| binding.address.clone())
    }

    /// Record the address of VPI automatic `key` in the activation's
    /// address array once its declaration has bound it.
    fn vpi_auto_bind(&mut self, key: VpiAutoKey) {
        let Some((slots, (index, block))) = self
            .vpi_autos
            .as_ref()
            .and_then(|(slots, locals)| Some((slots.clone(), *locals.get(&key)?)))
        else {
            return;
        };
        let address = match &key {
            VpiAutoKey::Name(name) => self.vpi_binding_address(name),
            VpiAutoKey::Container(container) => self
                .containers
                .get(container)
                .map(|bound| format!("&{bound}")),
            VpiAutoKey::Array(array) => self.fixed_arrays.get(array).cloned(),
            VpiAutoKey::NativeValue(value) => self.native_values.get(value).cloned(),
        };
        if let Some(address) = address {
            self.line(format!(
                "{slots}[{}] = (void*)({address});",
                index + model::VPI_AUTOS_FIRST
            ));
            let depth = self.marks.len();
            if block {
                self.vpi_block_bound.push((depth, index));
            } else {
                self.vpi_auto_depth = Some(self.vpi_auto_depth.map_or(depth, |d| d.min(depth)));
            }
        }
    }

    /// Clear the entries of the block locals declared in the lexical scopes
    /// from mark index `first` on before those scopes are left: a block's
    /// variables have no value outside it (decision A19-D88). `forget`
    /// drops them from the bound set when the scopes end on every path.
    fn vpi_block_end(&mut self, first: usize, forget: bool) {
        let Some((slots, _)) = self.vpi_autos.as_ref() else {
            return;
        };
        let slots = slots.clone();
        let cleared = self
            .vpi_block_bound
            .iter()
            .filter(|(depth, _)| *depth > first)
            .map(|(_, index)| *index)
            .collect::<Vec<_>>();
        for index in cleared {
            self.line(format!(
                "{slots}[{}] = NULL;",
                index + model::VPI_AUTOS_FIRST
            ));
        }
        if forget {
            self.vpi_block_bound.retain(|(depth, _)| *depth <= first);
        }
    }

    /// End the VPI frame before releasing the lexical scopes from mark
    /// index `first` on when they hold its body locals: cbEndOfFrame then
    /// still reads them (38.36.1, decision A19-D78), and no later read sees
    /// released storage. Clearing the serial leaves the exit at the
    /// function's return a branch.
    fn vpi_frame_end(&mut self, first: usize) {
        let Some(depth) = self.vpi_auto_depth else {
            return;
        };
        if first < depth {
            if let Some(serial) = self.vpi_frame_serial.clone() {
                self.line(format!(
                    "if ({serial}) {{ llg_vpi_frame_exit({serial}); {serial} = 0u; }}"
                ));
            }
        }
    }

    fn lookup(&self, name: &str) -> Option<Binding> {
        match self.bindings.get(name) {
            Some(binding) => Some(binding.clone()),
            None => self.model_binding(name),
        }
    }
    /// The model storage `name` resolves to when no frame scope binds it.
    fn model_binding(&self, name: &str) -> Option<Binding> {
        let model = self.ctx.model;
        match self.storage_names.filter(|names| names.indexes(model)) {
            Some(names) => names.find_checked(name),
            None => storage_names::scan(model, name),
        }
        .map(|storage| storage.binding(model, name))
    }
    fn address(&mut self, address: &str) -> Result<Binding, String> {
        if let Some(binding) = self.sequence_addresses.get(address) {
            return Ok(binding.clone());
        }
        if let Some(name) = address.strip_prefix('&') {
            return self.resolve_lookup(name);
        }
        if let Some(func) = self.formal_function() {
            for (index, formal) in func.formals.iter().enumerate() {
                if address == format!("o{index}") && formal.is_out && !formal.is_ref() {
                    return Ok(Binding {
                        address: address.to_owned(),
                        width: if formal.real { 0 } else { formal.width },
                        signed: formal.signed,
                        two_state: formal.two_state,
                        shortreal: formal.shortreal,
                        automatic: true,
                    });
                }
            }
        }
        Err(pending(&format!("opaque address {address}")))
    }
    fn read_binding(&mut self, binding: &Binding) -> Value {
        let code = if binding.width == 0 {
            format!("*({})", binding.address)
        } else {
            format!("sv4_clone({})", binding.address)
        };
        self.value(code, binding.width, binding.signed)
    }
    fn local(
        &mut self,
        name: &str,
        width: u32,
        signed: bool,
        two_state: bool,
        init: Option<&IrExpr>,
    ) -> Result<(), String> {
        let pointer = self.name("local");
        let address = if self.cell_eligibility.permits(name) {
            let cell = self.frame_cell(
                if width == 0 { "double" } else { "sv4_t" },
                (width == 0).then_some("NULL"),
            );
            self.declare_named_cached(
                if width == 0 { "double*" } else { "sv4_t*" },
                &pointer,
                cell,
            )
        } else if width == 0 {
            // Another coroutine may publish through this address while ours is
            // suspended. A native C stack address cannot survive a stackless
            // return, so the registered scope owns stable storage.
            let owner = self.scalar(
                "llg_value_scope_t*",
                "llg_value_scope_begin_object(sizeof(double), NULL)".to_owned(),
            );
            let pointer = self.declare_named_cached(
                "double*",
                &pointer,
                format!("(double*)llg_value_scope_object({owner})"),
            );
            self.line(format!("*{pointer} = 0.0;"));
            pointer
        } else {
            self.declare_named_cached(
                "sv4_t*",
                &pointer,
                "llg_value_scope_values(llg_value_scope_begin(1))".to_owned(),
            )
        };
        let binding = Binding {
            address: address.clone(),
            width,
            signed,
            two_state,
            shortreal: false,
            automatic: true,
        };
        // Make the default visible to self-referential initializers.
        if width != 0 {
            self.assign(
                &address,
                &super::expressions::packed_default(width, signed, two_state),
            );
        }
        self.bindings.insert(name, binding.clone());
        if let Some(init) = init {
            let value = self.expression(init)?;
            let value = self.convert(value, width, signed, two_state, false);
            if width == 0 {
                self.line(format!("*({address}) = {};", value.code));
            } else {
                self.line(format!("sv4_move({address}, &{});", value.code));
            }
            self.discard(value);
        }
        Ok(())
    }
    /// A local in a fresh one-slot activation frame owned by the current
    /// lexical scope; fork branches alias the slot and retain the frame.
    fn shared_local(
        &mut self,
        name: &str,
        width: u32,
        signed: bool,
        two_state: bool,
        init: Option<&IrExpr>,
    ) -> Result<(), String> {
        let owner = self.scalar(
            "llg_frame_t**",
            "(llg_frame_t**)llg_value_scope_object(llg_value_scope_begin_object(sizeof(llg_frame_t*), llg_owned_frame_drop))"
                .to_owned(),
        );
        self.line(format!("*{owner} = llg_frame_new(1ULL);"));
        let address = if width == 0 {
            self.line(format!("llg_frame_capture_real(*{owner}, 0u, 0.0);"));
            self.scalar("double*", format!("llg_frame_real_address(*{owner}, 0u)"))
        } else {
            let address = self.scalar("sv4_t*", format!("llg_frame_value_address(*{owner}, 0u)"));
            self.assign(
                &address,
                &super::expressions::packed_default(width, signed, two_state),
            );
            address
        };
        self.shared_cells
            .insert(name.to_owned(), (format!("(*{owner})"), 0));
        self.bindings.insert(
            name,
            Binding {
                address: address.clone(),
                width,
                signed,
                two_state,
                shortreal: false,
                automatic: true,
            },
        );
        if let Some(init) = init {
            let value = self.expression(init)?;
            let value = self.convert(value, width, signed, two_state, false);
            if width == 0 {
                self.line(format!("*({address}) = {};", value.code));
            } else {
                self.line(format!("sv4_move({address}, &{});", value.code));
            }
            self.discard(value);
        }
        Ok(())
    }
    pub(super) fn prologue(&self) -> String {
        if self.layout.storage() == FrameStorage::CoFrame && self.scope_fields {
            format!("    F->_llg_frame_base = llg_value_scope_mark();\n    F->_llg_temp_scope = llg_value_scope_begin({});\n    F->_llg_t = llg_value_scope_values(F->_llg_temp_scope);\n    (void)F->_llg_t;\n", self.slots.len())
        } else {
            format!("{}{}    llg_value_scope_t* _llg_frame_base = llg_value_scope_mark();\n    llg_value_scope_t* _llg_temp_scope = llg_value_scope_begin({});\n    sv4_t* _llg_t = llg_value_scope_values(_llg_temp_scope);\n    (void)_llg_t;\n", self.stack_cells, if self.layout.storage() == FrameStorage::CoFrame { "    (void)F;\n    (void)ch;\n" } else { "" }, self.slots.len())
        }
    }
    pub(super) fn macro_epilogue(&self) -> &'static str {
        ""
    }
    /// The rendered C-stack body. A native local that nothing names after
    /// its declaration gets a `(void)sizeof` use there (compilers diagnose an
    /// unused variable); a body without such locals is the buffer itself.
    pub(super) fn body(&self) -> Cow<'_, str> {
        if self.native_use_checks == 0 {
            Cow::Borrowed(&self.code)
        } else {
            Cow::Owned(resolve_native_uses(&self.code))
        }
    }
    /// Mark the C-stack native local `name`, declared on the line just
    /// emitted, for the use check in `body`. A coroutine frame checks every
    /// declaration through `resolve_local_uses` instead.
    fn check_native_use(&mut self, name: &str) {
        if self.layout.storage() == FrameStorage::CStack {
            self.code.push_str(NATIVE_USE_MARKER);
            self.code.push_str(name);
            self.code.push_str(NATIVE_USE_MARKER_END);
            self.native_use_checks += 1;
        }
    }
    pub(super) fn into_coframe(mut self) -> Result<CoroutineBody, String> {
        // Emission can fold an expression which still has a conservative call
        // site in the pre-emission execution analysis. Preserve storage and
        // descriptor offsets for those unreachable sites even though no call
        // expression consumed their slots.
        if let Some(site) = self.pending_sites.front() {
            return Err(format!(
                "coroutine suspension site {} ({:?}) was not emitted",
                site.resume, site.operation
            ));
        }
        if !self.brace_kinds.is_empty() {
            self.structural_error.get_or_insert_with(|| {
                format!(
                    "generated coroutine body has {} unclosed C brace(s)",
                    self.brace_kinds.len()
                )
            });
        }
        self.layout.finish_blocks()?;
        if let Some(error) = self
            .declaration_error
            .take()
            .or(self.structural_error.take())
        {
            Err(error)
        } else {
            let flattened = self.layout.finalize_paths()?;
            if !flattened.is_empty() {
                self.code = rewrite_call_paths(&self.code, &flattened);
            }
            let overlays = self.layout.overlay_paths();
            let mut poisoned = String::with_capacity(self.code.len());
            for line in self.code.split_inclusive('\n') {
                if let Some(block) = line.strip_prefix("/*__llg_poison_block_") {
                    let (block, condition) = block
                        .split_once("__*/")
                        .ok_or_else(|| "invalid deferred overlay poison".to_owned())?;
                    let block = block
                        .parse::<usize>()
                        .map_err(|_| "invalid deferred overlay poison block".to_owned())?;
                    if let Some(path) = overlays.get(&block) {
                        let operation = if condition.trim().is_empty() {
                            format!("LLG_CO_DEBUG_POISON(&F->{path}, sizeof(F->{path}));")
                        } else {
                            format!(
                                "LLG_CO_DEBUG_POISON_LOOP_EXIT({}, &F->{path}, sizeof(F->{path}));",
                                condition.trim()
                            )
                        };
                        poisoned
                            .push_str(&format!("#ifdef LLG_CO_DEBUG\n    {operation}\n#endif\n"));
                    }
                } else {
                    poisoned.push_str(line);
                }
            }
            self.code = poisoned;
            let layout = &self.layout;
            self.cached_fields
                .decide(&self.code, |name| layout.field_access(name).is_some());
            let declarations = self.resolve_declarations()?;
            let resolved = self.resolve_local_uses(&declarations)?;
            self.code = self
                .cached_fields
                .expand_reloads(&resolved, |name| self.layout.field_access(name))?;
            let rewritten = self.rewrite_frame_accesses(&self.code);
            let loads = self
                .cached_fields
                .prologue_loads(|name| self.layout.field_access(name));
            let mut layout = self.layout;
            layout.compute_gc_map();
            layout.release_emission_state();
            Ok(CoroutineBody {
                body: format!("{loads}{rewritten}"),
                cached_locals: self.cached_fields.locals(),
                layout,
            })
        }
    }

    fn resolve_declarations(&self) -> Result<String, String> {
        const PREFIX: &str = "__llg_declaration_";
        const SUFFIX: &str = "__";
        let mut output = String::with_capacity(self.code.len());
        let mut copied = 0;
        for (start, _) in self.code.match_indices(PREFIX) {
            output.push_str(&self.code[copied..start]);
            let (index, end) = marker_index(&self.code, start + PREFIX.len(), SUFFIX)
                .ok_or_else(|| "invalid deferred coroutine declaration".to_owned())?;
            let record = self
                .declarations
                .get(index)
                .ok_or_else(|| format!("unknown deferred coroutine declaration {index}"))?;
            if let Some(access) = self.layout.field_access(&record.name) {
                if !record.standalone {
                    if self.cached_fields.is_cached(&record.name) {
                        output.push_str(&record.name);
                        output.push_str(" = ");
                    }
                    output.push_str("F->");
                    output.push_str(access);
                }
            } else {
                output.push_str(&declaration(&record.ty, &record.name));
            }
            copied = end;
        }
        output.push_str(&self.code[copied..]);
        Ok(output)
    }

    fn resolve_local_uses(&self, code: &str) -> Result<String, String> {
        const PREFIX: &str = "/*__llg_local_use_";
        const SUFFIX: &str = "__*/";
        let locals = self
            .declarations
            .iter()
            .map(|record| record.name.as_str())
            .filter(|name| self.layout.field_access(name).is_none())
            .collect::<HashSet<_>>();
        let uses = identifier_counts(code, &locals);
        let mut output = String::with_capacity(code.len());
        let mut copied = 0;
        for start in marker_starts(code, PREFIX) {
            output.push_str(&code[copied..start]);
            let (index, end) = marker_index(code, start + PREFIX.len(), SUFFIX)
                .ok_or_else(|| "invalid deferred coroutine local use".to_owned())?;
            let record = self
                .declarations
                .get(index)
                .ok_or_else(|| format!("unknown deferred coroutine local use {index}"))?;
            if locals.contains(record.name.as_str())
                && uses.get(record.name.as_str()).copied().unwrap_or(0) <= 1
            {
                output.push_str("\n    (void)sizeof(");
                output.push_str(&record.name);
                output.push_str(");");
            }
            copied = end;
        }
        output.push_str(&code[copied..]);
        Ok(output)
    }
}

/// The index at `start` of a marker that `suffix` ends, and where it ends.
fn marker_index(code: &str, start: usize, suffix: &str) -> Option<(usize, usize)> {
    let digits = code[start..].bytes().take_while(u8::is_ascii_digit).count();
    let end = start + digits;
    let index = code[start..end].parse().ok()?;
    code[end..]
        .starts_with(suffix)
        .then(|| (index, end + suffix.len()))
}

const NATIVE_USE_MARKER: &str = "/*__llg_native_use ";
const NATIVE_USE_MARKER_END: &str = " */\n";

/// Replace each `check_native_use` marker line: dropped when the code after it
/// names the local, a `(void)sizeof` use when nothing does. Names are unique
/// within a frame, so any later occurrence is this local.
fn resolve_native_uses(code: &str) -> String {
    let markers = marker_starts(code, NATIVE_USE_MARKER)
        .filter_map(|start| {
            let rest = &code[start + NATIVE_USE_MARKER.len()..];
            Some((start, &rest[..rest.find(NATIVE_USE_MARKER_END)?]))
        })
        .collect::<Vec<_>>();
    let mut last_uses = markers
        .iter()
        .map(|(start, name)| (*name, *start))
        .collect::<HashMap<_, _>>();
    let filter = last_uses.keys().copied().collect::<NameFilter>();
    for span in named_spans(code, filter) {
        if let Some(last) = last_uses.get_mut(&code[span.clone()]) {
            *last = span.start;
        }
    }
    let mut output = String::with_capacity(code.len());
    let mut copied = 0;
    for (start, name) in markers {
        output.push_str(&code[copied..start]);
        if last_uses[name] <= start {
            output.push_str("    (void)sizeof(");
            output.push_str(name);
            output.push_str(");\n");
        }
        copied = start + NATIVE_USE_MARKER.len() + name.len() + NATIVE_USE_MARKER_END.len();
    }
    output.push_str(&code[copied..]);
    output
}

/// How often `code` names each of `names` outside C literals.
fn identifier_counts<'n>(code: &str, names: &HashSet<&'n str>) -> HashMap<&'n str, usize> {
    let mut counts = HashMap::new();
    if !names.is_empty() {
        let filter = names.iter().copied().collect::<NameFilter>();
        for span in named_spans_in_comments(code, filter) {
            if let Some(name) = names.get(&code[span]) {
                *counts.entry(*name).or_insert(0) += 1;
            }
        }
    }
    counts
}

/// Replace the structural path of each call slot that `flattened` lists by
/// its final one, after every `F->`.
fn rewrite_call_paths(code: &str, flattened: &HashMap<String, String>) -> String {
    const FRAME: &str = "F->";
    let mut output = String::with_capacity(code.len());
    let mut copied = 0;
    for start in marker_starts(code, FRAME) {
        let start = start + FRAME.len();
        let length = code[start..]
            .bytes()
            .take_while(|byte| matches!(byte, b'.' | b'_') || byte.is_ascii_alphanumeric())
            .count();
        // Members of the callee's frame may follow the slot.
        let mut path = &code[start..start + length];
        while !path.is_empty() {
            if let Some(replacement) = flattened.get(path) {
                output.push_str(&code[copied..start]);
                output.push_str(replacement);
                copied = start + path.len();
                break;
            }
            path = path.rsplit_once('.').map_or("", |(parent, _)| parent);
        }
    }
    output.push_str(&code[copied..]);
    output
}

fn is_structural_block_open(segment: &str) -> bool {
    if segment.is_empty() {
        return true;
    }
    let segment = segment.trim_start_matches('}').trim_start();
    ["if", "else", "for", "while", "switch", "do"]
        .into_iter()
        .any(|keyword| {
            segment == keyword
                || segment
                    .strip_prefix(keyword)
                    .is_some_and(|rest| rest.starts_with([' ', '(']))
        })
}

fn has_statement_boundary(text: &str) -> bool {
    let mut quoted = None;
    let mut escaped = false;
    for byte in text.bytes() {
        if let Some(quote) = quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == quote {
                quoted = None;
            }
        } else if matches!(byte, b'\'' | b'"') {
            quoted = Some(byte);
        } else if byte == b';' {
            return true;
        }
    }
    false
}

/// Runtime calls corresponding to Phase 2 resume sites. Direct coroutine
/// calls arm the same probe explicitly at their emission site.
/// The resume number of a numbered suspension macro (`LLG_CO_AWAIT(co, ch,
/// N, ...)` and the `LLG_CO_CALL*` forms): its third argument.
fn resume_state(text: &str) -> Option<u32> {
    [
        "LLG_CO_AWAIT(",
        "LLG_CO_AWAIT_OR(",
        "LLG_CO_CALL(",
        "LLG_CO_CALL_ANCHOR(",
        "LLG_CO_CALL_ARENA(",
    ]
    .into_iter()
    .find_map(|macro_name| {
        let start = text.find(macro_name)? + macro_name.len();
        let mut arguments = text[start..].splitn(4, ',');
        arguments.next()?;
        arguments.next()?;
        arguments.next()?.trim().parse::<u32>().ok()
    })
}

fn is_runtime_suspension(text: &str) -> bool {
    const SUSPENSIONS: [&str; 12] = [
        "LLG_CO_AWAIT",
        "LLG_CO_AWAIT_OR",
        "LLG_CO_CALL",
        "LLG_CO_CALL_ANCHOR",
        "LLG_CO_CALL_ARENA",
        "llg_join",
        "llg_process_suspend",
        "llg_process_await",
        "llg_semaphore_get",
        "llg_mailbox_get_value",
        "llg_mailbox_put_value",
        "llg_rt_stop_with_level",
    ];
    let bytes = text.as_bytes();
    let parentheses = std::iter::successors(first_of(bytes, 0, *b"("), |open| {
        first_of(bytes, open + 1, *b"(")
    });
    parentheses.into_iter().any(|open| {
        SUSPENSIONS
            .iter()
            .any(|suspension| bytes[..open].ends_with(suspension.as_bytes()))
    })
}

#[cfg(test)]
mod tests;
