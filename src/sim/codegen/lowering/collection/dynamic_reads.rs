//! Storage that a wait reaches through a handle: class properties selected
//! through a class handle (`h.x`, `n.next.v`, `this.x`, `h.s`) and
//! virtual-interface members (`v.sig`). Their storage changes when the
//! handle is rebound, so a wait names them by an
//! [`IrDependency::NativeAccess`] that the waiting process resolves again
//! every time it arms.
//!
//! `wait` conditions, event expressions and `$monitor` observe such storage
//! (a monitor through `report_dependencies`; SV
//! 9.4.2: `@(p.status)` follows the object `p` names, and "changing the
//! value of object data members ... referenced by a method or function
//! shall cause the event expression to be reevaluated"). The bodies of the
//! functions and nonvirtual methods such an expression calls are walked
//! with their class-handle formals bound to the caller's actuals, so a read
//! of `c.x` in `function int f(C c)` called as `f(h)` waits on `h.x`. A read
//! whose changes cannot be observed this way is rejected with its location:
//! a missed dependency would leave the wait blocked forever.
//!
//! `always_comb` adds nothing for class objects, not even the handle
//! variables of property paths (SV 9.2.2.2.1), and `@*` keeps those handle
//! identifiers (SV 9.4.2.2). The implicit sensitivity of `always_comb`,
//! `always_latch` and `@*` does follow members read through a
//! virtual-interface handle to the instance it binds when the process arms
//! (`with_interface_reads`, decision S99-D7); a read it cannot follow is a
//! located error.

use super::dependencies::DeclaredStorage;
use super::*;
use crate::sim::ir::{IrElementRefKind, IrElementRoot, IrNativeAccessKind};
use crate::{IdMap, IdSet};

mod report_plan;

/// Longest handle-property chain (`a.b.c.d.v`) whose links a wait follows;
/// deeper links reject at the emitter's own access-depth guard first.
const MAX_HANDLE_PROPERTY_DEPTH: usize = 256;

/// Why a member read inside a called function cannot be followed.
const CALLEE_HANDLE: &str =
    "a virtual-interface member that a called function reads through a handle other than a virtual-interface variable";

/// Deepest nest of calls whose bodies a wait's dependency walk follows.
const MAX_CALLEE_DEPTH: usize = 64;

/// Class handles a callee body can read through, bound for one call: the
/// handle formals to the caller's actuals and `this` to the receiver. An
/// unbound handle (`None`) cannot be evaluated again when the wait arms.
#[derive(Clone, Default, PartialEq)]
struct CalleeReceivers {
    formals: HashMap<NodeId, Option<IrChandleExpr>>,
    this: Option<IrChandleExpr>,
}

/// A walk over the bodies one call reaches with receivers `R`. A body reads
/// the same storage wherever it is entered with the same receivers, so it is
/// walked once with each (a call graph has exponentially many paths).
#[derive(Default)]
struct CalleeWalk<R> {
    /// The subroutines on the call path.
    stack: Vec<NodeId>,
    walked: IdMap<NodeId, Vec<R>>,
    /// Bodies so far that were not entered because the path was too long. A
    /// walk that met one is not recorded: a shorter path enters it.
    cuts: usize,
}

impl<R: PartialEq> CalleeWalk<R> {
    fn has_walked(&self, callee: NodeId, receivers: &R) -> bool {
        self.walked
            .get(&callee)
            .is_some_and(|walked| walked.contains(receivers))
    }

    /// Whether a call is cut: its callee is on the path or the path is too
    /// long.
    fn cuts(&mut self, callee: NodeId) -> bool {
        if self.stack.contains(&callee) {
            return true;
        }
        let cut = self.stack.len() >= MAX_CALLEE_DEPTH;
        self.cuts += usize::from(cut);
        cut
    }

    /// The walk is past the end of `callee`, entered after `cuts` cuts.
    fn leave(&mut self, callee: NodeId, receivers: R, cuts: usize) {
        self.stack.pop();
        if self.cuts == cuts {
            self.walked.entry(callee).or_default().push(receivers);
        }
    }
}

/// How a `$monitor` learns that an argument changed (SV 21.2.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) enum ReportReadPlan {
    /// Named storage only: the ordinary read walk finds every trigger.
    Fixed,
    /// Property paths, and calls whose callee reads bind through the call's
    /// handles: the handles and properties they pass through are exact
    /// dependencies, resolved again after each evaluation.
    Exact,
    /// Virtual methods, calls with a read no handle binds, virtual-interface
    /// members or synchronization objects: storage no dependency names, so
    /// the monitor evaluates at every settled slot.
    EverySlot,
}

impl Codegen<'_> {
    /// The instance property `node` selects, when it is packed, real, string
    /// or handle storage held directly in the object.
    fn dynamic_class_property(&self, node: NodeId) -> Option<NodeId> {
        if !matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::Ref { .. } | ExprKind::HierPath { .. })
        ) {
            return None;
        }
        let field = self.class_field_target(node)?;
        let (class, index) = self.class_fields.get(&field).copied()?;
        let layout = self.model.classes.get(class)?.fields.get(index)?;
        (layout.container.is_none() && layout.native_value.is_none()).then_some(field)
    }

    /// The layout type of instance property `field`, when it is held
    /// directly in the object.
    fn direct_field_type(&self, field: NodeId) -> Option<IrClassFieldType> {
        let (class, index) = self.class_fields.get(&field).copied()?;
        let layout = self.model.classes.get(class)?.fields.get(index)?;
        (layout.container.is_none() && layout.native_value.is_none()).then_some(layout.ty)
    }

    /// Whether `node` names a member through a virtual-interface handle.
    fn dynamic_interface_member(&self, node: NodeId) -> bool {
        let NodeKind::Expr(ExprKind::HierPath { parts, refs }) = self.kind(node) else {
            return false;
        };
        refs.iter().enumerate().any(|(position, target)| {
            target.is_some_and(|target| self.virtual_interface_spelling(target).is_some())
                && parts.iter().skip(position + 1).any(|part| !part.is_empty())
        })
    }

    /// Whether `node` calls an interface method through a virtual-interface
    /// handle (`v.f()`).
    fn virtual_interface_method_call(&self, node: NodeId) -> bool {
        matches!(self.kind(node), NodeKind::MethodCall {
            receiver: Some(receiver),
            ..
        } if self.virtual_interface_spelling(*receiver).is_some())
    }

    /// The nonvirtual class function a method call `node` invokes, with its
    /// receiver expression (SV 9.4.2 admits only those in event controls).
    fn class_method_call(&self, node: NodeId) -> Option<(NodeId, NodeId)> {
        let NodeKind::MethodCall {
            receiver: Some(receiver),
            callee: Some(method),
            ..
        } = self.kind(node)
        else {
            return None;
        };
        (matches!(
            self.kind(*method),
            NodeKind::FuncTask {
                is_task: false,
                is_virtual: false,
                ..
            }
        ) && self.class_method_owner(*method).is_some())
        .then_some((*method, *receiver))
    }

    /// Whether evaluating `root` reads storage selected through a class or
    /// virtual-interface handle, a static class property or a built-in
    /// synchronization object, directly or in the body of a function or
    /// method it calls: the waiting process must evaluate such an expression
    /// and re-arm on the storage the handles name after every wake.
    pub(in super::super) fn reads_dynamic_storage(&self, root: NodeId) -> bool {
        self.reads_dynamic_storage_in(root, &mut IdSet::new())
    }

    fn reads_dynamic_storage_in(&self, root: NodeId, visited: &mut IdSet<NodeId>) -> bool {
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            if self.class_storage_field(node).is_some()
                || self.class_container_property(node).is_some()
                || self.dynamic_interface_member(node)
                || self.class_method_call(node).is_some()
                || self.synchronization_method_call(node)
            {
                return true;
            }
            if let Some(callee) = self.called_subroutine(node) {
                if visited.insert(callee)
                    && self
                        .func_body(callee)
                        .is_some_and(|body| self.reads_dynamic_storage_in(body, visited))
                {
                    return true;
                }
            }
            pending.extend(self.node(node).children.iter().copied());
        }
        false
    }

    /// Whether a nonblocking assignment under event control `specs` ends with
    /// its issuer (decision S09-D8): the control names an event, or reads
    /// storage that is neither a net nor a declared variable (a class
    /// property, also a static one, an element of a dynamically sized array,
    /// a member of a virtual interface).
    pub(in super::super) fn event_control_binds(&self, specs: &[EventSpec]) -> bool {
        let mut pending = Vec::new();
        for spec in specs {
            self.event_spec_expressions(spec, &mut pending);
        }
        if pending
            .iter()
            .any(|root| self.event_target_of(*root).is_some() || self.reads_dynamic_storage(*root))
        {
            return true;
        }
        while let Some(node) = pending.pop() {
            let target = match self.kind(node) {
                NodeKind::Expr(ExprKind::Ref { target }) => *target,
                NodeKind::Expr(ExprKind::HierPath { refs, .. }) => refs.first().copied().flatten(),
                _ => None,
            };
            if target.is_some_and(|target| self.is_static_property(target))
                || self.container_of(node).is_some()
            {
                return true;
            }
            pending.extend(self.node(node).children.iter().copied());
        }
        false
    }

    /// Whether `root` calls a function with a string formal, directly or in
    /// the body of a function it calls.
    pub(in super::super) fn calls_string_formal_helper(&self, root: NodeId) -> bool {
        let mut visited = IdSet::new();
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            if let Some(callee) = self.called_subroutine(node) {
                if self.func_formals(callee).iter().any(|(formal, _)| {
                    matches!(self.kind(*formal), NodeKind::FuncArg { ty, .. } if ty.kind == "string")
                }) {
                    return true;
                }
                if visited.insert(callee) {
                    pending.extend(self.func_body(callee));
                }
            }
            pending.extend(self.node(node).children.iter().copied());
        }
        false
    }

    /// How a `$monitor` watches report argument `root`. A call of a
    /// function or nonvirtual method is exact when every class storage its
    /// body reads binds through the call's receiver, handle formals, handle
    /// properties, static handles or handle locals that hold one handle
    /// (`report_plan`); otherwise, and for a virtual method, a
    /// virtual-interface member or a synchronization object, the monitor
    /// evaluates at every settled slot.
    pub(in super::super) fn report_read_plan(&self, root: NodeId) -> ReportReadPlan {
        if !self.reads_dynamic_storage(root) {
            return ReportReadPlan::Fixed;
        }
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            if self.dynamic_interface_member(node) || self.synchronization_method_call(node) {
                return ReportReadPlan::EverySlot;
            }
            if let Some(callee) = self.called_subroutine(node) {
                let watched = self.calls_virtual_method(node)
                    || self.class_method_call(node).is_some()
                    || self
                        .func_body(callee)
                        .is_some_and(|body| self.reads_dynamic_storage_in(body, &mut IdSet::new()));
                if watched && !self.report_call_binds(node, callee) {
                    return ReportReadPlan::EverySlot;
                }
            }
            pending.extend(self.node(node).children.iter().copied());
        }
        ReportReadPlan::Exact
    }

    /// The dependencies of a [`ReportReadPlan::Exact`] report argument. Every
    /// property it names must resolve to one: a report that could never fire
    /// is a located rejection, as a wait that could never wake is.
    pub(in super::super) fn report_dependencies(
        &mut self,
        path: &str,
        root: NodeId,
    ) -> Result<Vec<IrDependency>, String> {
        let nodes = {
            let mut nodes = Vec::new();
            let mut pending = vec![root];
            while let Some(node) = pending.pop() {
                nodes.push(node);
                pending.extend(self.node(node).children.iter().copied());
            }
            nodes
        };
        self.with_prepared_reads(path, root, |cg| {
            for node in &nodes {
                let named = cg.class_storage_field(*node).is_some()
                    || cg.class_container_property(*node).is_some();
                let watched = cg.dynamic_reads.contains_key(node)
                    || cg
                        .node(*node)
                        .children
                        .iter()
                        .any(|child| cg.dynamic_reads.contains_key(child));
                if named && !watched {
                    return Err(crate::sim::codegen::error::located(
                        &cg.source_location(*node),
                        format!(
                            "`$monitor` of class storage `{}` in `{}` has no dependency that sees it change",
                            cg.node(*node).name,
                            cg.source_path(path)
                        ),
                    ));
                }
            }
            // A static handle property is an object no read names, and
            // rebinding it moves everything selected through it.
            let mut reads = cg.collect_read_signals(path, root)?;
            for read in reads.clone() {
                if let IrDependency::NativeAccess(name) = read {
                    let mut roots = Vec::new();
                    cg.root_handle_dependency(&name, &mut roots);
                    for root in roots {
                        if !reads.contains(&root) {
                            reads.push(root);
                        }
                    }
                }
            }
            Ok(reads)
        })
    }

    /// The last class property `node` names, including container and native
    /// record properties, when `node` reads object storage.
    pub(in super::super) fn class_storage_field(&self, node: NodeId) -> Option<NodeId> {
        let is_field = |target: &NodeId| {
            self.class_fields.contains_key(target) || self.class_native_fields.contains_key(target)
        };
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => is_field(target).then_some(*target),
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => refs
                .iter()
                .rev()
                .flatten()
                .copied()
                .find(|target| is_field(target)),
            _ => None,
        }
    }

    /// Whether `node` calls a method of a mailbox or semaphore: their state
    /// (`num()`, `try_get()`) publishes no change marker.
    fn synchronization_method_call(&self, node: NodeId) -> bool {
        let NodeKind::MethodCall {
            receiver: Some(receiver),
            ..
        } = self.kind(node)
        else {
            return false;
        };
        self.query_descriptor(*receiver).is_some_and(|descriptor| {
            descriptor.name == "semaphore" || descriptor.name.starts_with("mailbox")
        })
    }

    /// The user function or method `node` calls, when its body is walked
    /// for wait dependencies.
    fn called_subroutine(&self, node: NodeId) -> Option<NodeId> {
        let callee = match self.kind(node) {
            NodeKind::FuncCall {
                callee: Some(callee),
                is_task: false,
                ..
            } if !self.is_mailbox_constructor_call(node)
                && !self.is_semaphore_constructor_call(node)
                && !self.is_process_self_call(node) =>
            {
                *callee
            }
            NodeKind::MethodCall {
                callee: Some(callee),
                ..
            } => *callee,
            _ => return None,
        };
        matches!(self.kind(callee), NodeKind::FuncTask { is_task: false, .. }).then_some(callee)
    }

    /// A located rejection of a read whose changes no wait can observe.
    fn unobservable_read(&self, path: &str, node: NodeId, what: &str) -> String {
        self.locate_error(
            node,
            format!(
                "{what} cannot be observed by a wait or event expression in `{}`: llg publishes no change for it, so the wait could never resume (IEEE 1800-2009 9.4.2)",
                self.source_path(path)
            ),
        )
    }

    /// Reject a comparison of event variables under a wait or event
    /// expression: assigning an event variable publishes nothing, so the
    /// result never changes for a waiter. A class event property is the
    /// served form, with its rebinding marker.
    pub(in super::super) fn reject_event_comparison(
        &self,
        path: &str,
        node: NodeId,
    ) -> Result<(), String> {
        if let NodeKind::Expr(ExprKind::Operation { op, operands, .. }) = self.kind(node) {
            if matches!(
                op,
                Operation::Equal
                    | Operation::NotEqual
                    | Operation::CaseEqual
                    | Operation::CaseNotEqual
            ) && operands
                .iter()
                .any(|operand| self.event_target_of(*operand).is_some())
                && !operands
                    .iter()
                    .any(|operand| self.dynamic_class_property(*operand).is_some())
            {
                return Err(self.unobservable_read(path, node, "an event variable comparison"));
            }
        }
        Ok(())
    }

    /// Reject reads in a wait or event expression that publish no change:
    /// mailbox and semaphore methods other than `num()`, whose count change
    /// a mailbox publishes once a wait observes it.
    fn reject_unobservable_read(&self, path: &str, node: NodeId) -> Result<(), String> {
        if self.synchronization_method_call(node) && !self.mailbox_num_call(node) {
            return Err(self.unobservable_read(path, node, "a mailbox or semaphore method call"));
        }
        self.reject_event_comparison(path, node)?;
        // `f().v`: only the call can name this storage, and the walk over
        // `f` sees what it reads, not what the returned handle selects.
        if let NodeKind::Expr(ExprKind::MemberSelect { base, .. }) = self.kind(node) {
            if self.class_field_target(node).is_some() && self.called_subroutine(*base).is_some() {
                return Err(self.unobservable_read(
                    path,
                    node,
                    "a class property selected through a call result",
                ));
            }
        }
        // An event array property publishes no change of its handles.
        if self.container_of(node).is_some_and(|container| {
            self.model
                .containers
                .get(container.ir)
                .is_some_and(|container| {
                    container.class_field.is_some()
                        && matches!(container.element, crate::sim::ir::IrContainerElement::Event)
                })
        }) {
            return Err(self.unobservable_read(
                path,
                node,
                &format!("event array class property `{}`", self.node(node).name),
            ));
        }
        Ok(())
    }

    /// Whether `node` is a `num()` call on a mailbox.
    fn mailbox_num_call(&self, node: NodeId) -> bool {
        matches!(self.kind(node), NodeKind::MethodCall {
            name,
            receiver: Some(receiver),
            ..
        } if name == "num"
            && self
                .query_descriptor(*receiver)
                .is_some_and(|descriptor| descriptor.name.starts_with("mailbox")))
    }

    /// The class container property `node` names, as `(class, field)`. An
    /// event array property is a set of event handles, waited on as events.
    fn class_container_property(&self, node: NodeId) -> Option<(usize, usize)> {
        let container = self.container_of(node)?;
        let container = self.model.containers.get(container.ir)?;
        if matches!(container.element, crate::sim::ir::IrContainerElement::Event) {
            return None;
        }
        container.class_field
    }

    /// The container property declaration `node` names.
    fn container_property_declaration(&self, node: NodeId) -> Option<NodeId> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => Some(*target),
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => refs.last().copied().flatten(),
            _ => None,
        }
    }

    /// The change marker of the container property `(class, field)` of the
    /// object `receiver` names, with the handle properties it is selected
    /// through.
    fn class_container_dependencies(
        &mut self,
        receiver: IrChandleExpr,
        (class, field): (usize, usize),
    ) -> Vec<IrDependency> {
        let access = self.native_access_symbol(
            receiver,
            IrNativeAccessKind::ClassContainer { class, field },
        );
        let mut dependencies = vec![IrDependency::NativeAccess(access.clone())];
        self.handle_property_receivers(&access, &mut dependencies);
        self.root_handle_dependency(&access, &mut dependencies);
        dependencies
    }

    /// A `ref` actual bound to a formal its task waits on, when the actual
    /// names storage inside a class object. The waiter follows the
    /// reference's own marker: an element of a container property
    /// (`c.q[i]`, `c.a[i].m`) takes the container's marker, which the
    /// property then publishes on its object. A packed member of a record
    /// property (`c.r.a`) binds a reference to the property's value, which
    /// then follows its object's marker ([`IrElementRef::observed`]). Any
    /// other binding into a record property has no marker a direct store
    /// reaches, so the call rejects rather than waiting forever. `argument`
    /// is the lowered actual.
    pub(in crate::sim::codegen) fn observe_waited_ref_actual(
        &mut self,
        path: &str,
        actual: NodeId,
        argument: Option<&mut IrCallArg>,
    ) -> Result<(), String> {
        let mut node = actual;
        loop {
            if let Some(container) = self
                .class_container_property(node)
                .filter(|_| self.container_property_declaration(node).is_some())
            {
                let receiver = match self
                    .container_of(node)
                    .and_then(|container| self.model.containers[container.ir].receiver.clone())
                {
                    Some(receiver) => Some(receiver),
                    None => self
                        .container_property_declaration(node)
                        .and_then(|field| self.class_receiver_for(path, node, field).ok()),
                };
                let Some(receiver) = receiver else {
                    return Err(self.unobservable_read(
                        path,
                        actual,
                        &format!(
                            "an element of container class property `{}` bound to a `ref` formal its task waits on, selected through a handle the call cannot name,",
                            self.node(node).name
                        ),
                    ));
                };
                self.class_container_dependencies(receiver, container);
                return Ok(());
            }
            if let Some(field) = self.class_record_property(node) {
                let receiver = self.class_receiver_for(path, node, field).ok();
                if let (Some(receiver), Some(IrCallArg::ElementRef(reference))) =
                    (receiver, argument)
                {
                    if matches!(reference.root, IrElementRoot::Value(_))
                        && matches!(reference.kind, IrElementRefKind::Packed { .. })
                    {
                        self.class_record_dependencies(receiver, field);
                        reference.observed = true;
                        return Ok(());
                    }
                }
                return Err(self.unobservable_read(
                    path,
                    actual,
                    &format!(
                        "storage of record class property `{}` bound to a `ref` formal its task waits on",
                        self.node(field).name
                    ),
                ));
            }
            node = match self.kind(node) {
                // A member array is a detached array node, named by the
                // select's path child (`c.fa` in `c.fa[i]`, `c.r.arr` in
                // `c.r.arr[1]`).
                NodeKind::Expr(ExprKind::ArraySelect { base, indices })
                    if !matches!(self.kind(*base), NodeKind::Expr(_)) =>
                {
                    let path_child = self.node(node).children.iter().copied().find(|child| {
                        !indices.contains(child)
                            && matches!(
                                self.kind(*child),
                                NodeKind::Expr(ExprKind::HierPath { .. })
                            )
                    });
                    match path_child {
                        Some(child) => child,
                        None => return Ok(()),
                    }
                }
                NodeKind::Expr(
                    ExprKind::ArraySelect { base, .. }
                    | ExprKind::BitSelect { base, .. }
                    | ExprKind::PartSelect { base, .. }
                    | ExprKind::IndexedPartSelect { base, .. },
                ) => *base,
                NodeKind::Expr(ExprKind::HierPath { refs, .. }) => match refs.first() {
                    Some(Some(first))
                        if matches!(
                            self.kind(*first),
                            NodeKind::Expr(
                                ExprKind::ArraySelect { .. } | ExprKind::BitSelect { .. }
                            )
                        ) =>
                    {
                        *first
                    }
                    _ => return Ok(()),
                },
                _ => return Ok(()),
            };
        }
    }

    /// The native record property `node` names (`h.r`, `h.r.a`, or `r` in a
    /// method of its class): one value per object, whose leaves have no
    /// storage a wait could name.
    fn class_record_property(&self, node: NodeId) -> Option<NodeId> {
        self.class_storage_field(node)
            .filter(|field| self.class_native_fields.contains_key(field))
    }

    /// The contents marker of the record property `field` of the object
    /// `receiver` names, with the handle properties it is selected through.
    /// Every store below the record toggles it, so the waiter evaluates
    /// what it reads again.
    fn class_record_dependencies(
        &mut self,
        receiver: IrChandleExpr,
        field: NodeId,
    ) -> Vec<IrDependency> {
        let Some((class, field)) = self
            .class_native_fields
            .get(&field)
            .and_then(|value| self.model.native_values[*value].class_field)
        else {
            return Vec::new();
        };
        let access =
            self.native_access_symbol(receiver, IrNativeAccessKind::ClassRecord { class, field });
        let mut dependencies = vec![IrDependency::NativeAccess(access.clone())];
        self.handle_property_receivers(&access, &mut dependencies);
        self.root_handle_dependency(&access, &mut dependencies);
        dependencies
    }

    /// An automatic record with string, real, handle or container members is
    /// one value per activation with no marker. A fork shares it with its
    /// branches, and a branch that waits on a member another process can
    /// store into while it waits could never resume: reject that wait. A
    /// record nothing stores into once a fork names it cannot change under a
    /// waiter and is read as it is; a container member has its own markers.
    fn reject_shared_automatic_records(&self, path: &str, root: NodeId) -> Result<(), String> {
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            let record = self
                .native_path_of(node)
                .ok()
                .flatten()
                .filter(|(value, _)| {
                    let storage = &self.model.native_values[*value];
                    storage.activation && storage.class_field.is_none()
                });
            let Some((value, member)) = record else {
                pending.extend(self.node(node).children.iter().copied());
                continue;
            };
            // Below the node only its selectors are reads of their own; the
            // nodes that spell the record's own path are not.
            pending.extend(self.node(node).children.iter().copied().filter(|child| {
                !matches!(self.native_path_of(*child), Ok(Some((root, _))) if root == value)
            }));
            if self.container_of(node).is_some() || self.native_companion(value, &member).is_some()
            {
                continue;
            }
            let Some(declaration) = self.native_value_layouts.get(&value).copied() else {
                continue;
            };
            if self.shared_locals.contains(&declaration)
                && self.stored_while_fork_shared(declaration)
            {
                return Err(self.unobservable_read(
                    path,
                    node,
                    &format!(
                        "automatic record `{}` shared with a fork",
                        self.node(declaration).name
                    ),
                ));
            }
        }
        Ok(())
    }

    /// Whether `node` selects below declaration `root` (`r`, `r.a`, `r.m[i]`).
    fn selects_below(&self, node: NodeId, root: NodeId) -> bool {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target == Some(root),
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => refs.contains(&Some(root)),
            NodeKind::Expr(
                ExprKind::MemberSelect { base, .. }
                | ExprKind::BitSelect { base, .. }
                | ExprKind::ArraySelect { base, .. }
                | ExprKind::PartSelect { base, .. }
                | ExprKind::IndexedPartSelect { base, .. },
            ) => self.selects_below(*base, root),
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Concat,
                operands,
                ..
            }) => operands
                .iter()
                .any(|operand| self.selects_below(*operand, root)),
            _ => false,
        }
    }

    /// Whether some store into fork-shared automatic record `declaration`
    /// can run while a fork branch waits on it: a store inside a fork branch,
    /// or any store at all once a `join_none` or `join_any` fork names the
    /// record (its parent goes on while the branch waits). A store is an
    /// assignment or update target, an actual of a formal that is not an
    /// input, or an argument of a system call other than the display family.
    fn stored_while_fork_shared(&self, declaration: NodeId) -> bool {
        let scope = match self.enclosing_func_task(declaration) {
            Some(function) => function,
            None => {
                // A block record's `Var` hangs on a scope block outside the
                // statement tree; its declaration statement is in it.
                let mut scope = self
                    .block_activation_locals
                    .get(&declaration)
                    .copied()
                    .unwrap_or(declaration);
                while let Some(parent) = self.node(scope).parent {
                    if matches!(self.kind(parent), NodeKind::ModuleInst { .. }) {
                        break;
                    }
                    scope = parent;
                }
                scope
            }
        };
        let mut stores = Vec::new();
        let mut detached = false;
        // `(node, inside a fork branch)`.
        let mut pending = vec![(scope, false)];
        while let Some((node, in_branch)) = pending.pop() {
            let mut child_in_branch = in_branch;
            match self.kind(node) {
                NodeKind::Stmt(StmtKind::Fork { join_kind, .. }) => {
                    child_in_branch = true;
                    if *join_kind != crate::core::db::JoinKind::All && !detached {
                        let mut names = vec![node];
                        while let Some(inner) = names.pop() {
                            if self.selects_below(inner, declaration) {
                                detached = true;
                                break;
                            }
                            names.extend(self.node(inner).children.iter().copied());
                        }
                    }
                }
                NodeKind::Stmt(StmtKind::Assign { .. } | StmtKind::ProcContAssign { .. }) => {
                    if let Some(lhs) = self.node(node).children.first() {
                        if self.selects_below(*lhs, declaration) {
                            stores.push(in_branch);
                        }
                    }
                }
                NodeKind::Expr(ExprKind::Operation {
                    op:
                        Operation::Assignment
                        | Operation::PostIncrement
                        | Operation::PreIncrement
                        | Operation::PostDecrement
                        | Operation::PreDecrement
                        | Operation::OverloadUpdate
                        | Operation::OverloadPostUpdate,
                    operands,
                    ..
                }) => {
                    if operands
                        .first()
                        .is_some_and(|lhs| self.selects_below(*lhs, declaration))
                    {
                        stores.push(in_branch);
                    }
                }
                NodeKind::SysCall { name } => {
                    let display = [
                        "$display",
                        "$write",
                        "$strobe",
                        "$monitor",
                        "$fdisplay",
                        "$fwrite",
                        "$fstrobe",
                        "$fmonitor",
                        "$error",
                        "$warning",
                        "$info",
                        "$fatal",
                        "$sformatf",
                        "$psprintf",
                    ]
                    .iter()
                    .any(|task| name.starts_with(task));
                    if !display
                        && self
                            .node(node)
                            .children
                            .iter()
                            .any(|argument| self.selects_below(*argument, declaration))
                    {
                        stores.push(in_branch);
                    }
                }
                NodeKind::FuncCall {
                    callee: Some(callee),
                    ..
                }
                | NodeKind::MethodCall {
                    callee: Some(callee),
                    ..
                } => {
                    let formals = self
                        .node(*callee)
                        .children
                        .iter()
                        .copied()
                        .filter(|child| matches!(self.kind(*child), NodeKind::FuncArg { .. }))
                        .collect::<Vec<_>>();
                    for (position, argument) in
                        self.call_argument_nodes(node).into_iter().enumerate()
                    {
                        let input = formals.get(position).is_some_and(|formal| {
                            matches!(
                                self.kind(*formal),
                                NodeKind::FuncArg {
                                    direction: crate::core::db::Direction::Input,
                                    ..
                                }
                            )
                        });
                        if !input && self.selects_below(argument, declaration) {
                            stores.push(in_branch);
                        }
                    }
                }
                _ => {}
            }
            pending.extend(
                self.node(node)
                    .children
                    .iter()
                    .map(|child| (*child, child_in_branch)),
            );
        }
        stores.iter().any(|in_branch| *in_branch || detached)
    }

    /// The handle variable at the root of native access `name`'s receiver
    /// chain (`h` in `h.n.q`): rebinding it moves the access. A method or
    /// built-in call on the property does not read the receiver path, so the
    /// read walk adds no dependency on it.
    pub(in crate::sim::codegen) fn root_handle_dependency(
        &self,
        name: &str,
        out: &mut Vec<IrDependency>,
    ) {
        let mut current = name.to_owned();
        for _ in 0..MAX_HANDLE_PROPERTY_DEPTH {
            let Some(access) = self
                .model
                .native_accesses
                .iter()
                .find(|access| access.name == current)
            else {
                return;
            };
            let mut receiver = &access.receiver;
            // A nested interface instance (`v.tx.x`) is selected through
            // the handle, so rebinding the handle moves the access too.
            while let IrChandleExpr::Required { handle, .. }
            | IrChandleExpr::VirtualInterfaceNested { handle, .. } = receiver
            {
                receiver = handle;
            }
            match receiver {
                IrChandleExpr::LocalRead(inner) => current = inner.clone(),
                IrChandleExpr::Read(object) => {
                    if self
                        .model
                        .objects
                        .get(*object)
                        .is_some_and(|object| object.ty.has_change_marker())
                    {
                        out.push(IrDependency::Object(*object));
                    }
                    return;
                }
                _ => return,
            }
        }
    }

    /// Whether a handle expression can be evaluated again whenever the wait
    /// arms without side effects.
    pub(in crate::sim::codegen) fn rearmable_handle(handle: &IrChandleExpr) -> bool {
        match handle {
            IrChandleExpr::Null
            | IrChandleExpr::Read(_)
            | IrChandleExpr::LocalRead(_)
            | IrChandleExpr::FormalRead(_)
            | IrChandleExpr::InterfaceInstance { .. } => true,
            IrChandleExpr::Required { handle, .. }
            | IrChandleExpr::VirtualInterfaceNested { handle, .. } => {
                Self::rearmable_handle(handle)
            }
            _ => false,
        }
    }

    /// Whether `formal` passes a class or virtual-interface handle.
    fn is_class_handle_formal(&self, formal: NodeId) -> bool {
        matches!(self.kind(formal), NodeKind::FuncArg { ty, .. }
            if ty.kind == "class" && ty.type_name.as_deref() != Some("process"))
            || (matches!(self.kind(formal), NodeKind::FuncArg { .. })
                && self.virtual_interface_spelling(formal).is_some())
    }

    /// The formals of `callee` paired with the actuals of `call`.
    fn handle_formal_actuals(&self, call: NodeId, callee: NodeId) -> Vec<(NodeId, NodeId)> {
        let formals = self
            .node(callee)
            .children
            .iter()
            .copied()
            .filter(|child| matches!(self.kind(*child), NodeKind::FuncArg { .. }));
        formals
            .zip(self.call_argument_nodes(call))
            .filter(|(formal, _)| self.is_class_handle_formal(*formal))
            .collect()
    }

    fn reject_virtual_callee(
        &self,
        path: &str,
        call: NodeId,
        callee: NodeId,
    ) -> Result<(), String> {
        if matches!(
            self.kind(callee),
            NodeKind::FuncTask {
                is_virtual: true,
                ..
            }
        ) {
            return Err(self.unobservable_read(
                path,
                call,
                "the storage a virtual method reads (its implementation is chosen at run time)",
            ));
        }
        Ok(())
    }

    /// Bind the handles a call in the wait expression itself passes to its
    /// callee, lowered in the waiting process's own context.
    fn root_call_receivers(
        &mut self,
        path: &str,
        call: NodeId,
        callee: NodeId,
    ) -> Result<CalleeReceivers, String> {
        self.reject_virtual_callee(path, call, callee)?;
        let mut receivers = CalleeReceivers::default();
        if self.class_method_owner(callee).is_some() {
            receivers.this = self
                .class_method_receiver(path, call)
                .ok()
                .flatten()
                .filter(Self::rearmable_handle);
        }
        for (formal, actual) in self.handle_formal_actuals(call, callee) {
            let handle = self
                .lower_chandle(path, actual)
                .ok()
                .filter(Self::rearmable_handle);
            receivers.formals.insert(formal, handle);
        }
        Ok(receivers)
    }

    /// Bind the handles a call inside a callee body passes to its callee.
    fn nested_call_receivers(
        &mut self,
        path: &str,
        call: NodeId,
        callee: NodeId,
        outer: &CalleeReceivers,
    ) -> Result<CalleeReceivers, String> {
        self.reject_virtual_callee(path, call, callee)?;
        let mut receivers = CalleeReceivers::default();
        let is_static = matches!(
            self.kind(callee),
            NodeKind::FuncTask {
                is_static: true,
                ..
            }
        );
        if self.class_method_owner(callee).is_some() && !is_static {
            receivers.this = match self.kind(call) {
                NodeKind::MethodCall {
                    receiver: Some(receiver),
                    ..
                } => self.callee_handle(*receiver, outer),
                _ => outer.this.clone(),
            };
        }
        for (formal, actual) in self.handle_formal_actuals(call, callee) {
            let handle = self.callee_handle(actual, outer);
            receivers.formals.insert(formal, handle);
        }
        Ok(receivers)
    }

    /// Bind the automatic class-handle locals of a callee body whose every
    /// write stores one handle the wait can evaluate again (`d = c;` with
    /// `c` a bound formal): a read through such a local reads through that
    /// handle. A local written any other way (an output or `ref` actual, a
    /// system task, two different handles) stays unbound. Locals bind in
    /// declaration order, so one may copy an earlier one.
    fn bind_callee_locals(
        &mut self,
        body: NodeId,
        mut receivers: CalleeReceivers,
    ) -> CalleeReceivers {
        let mut locals = Vec::new();
        let mut parents: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
        let mut assigned: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
        let mut pending = vec![body];
        while let Some(node) = pending.pop() {
            if let NodeKind::Stmt(StmtKind::VariableDecl { declaration }) = self.kind(node) {
                if matches!(self.kind(*declaration), NodeKind::Var { ty } if ty.kind == "class")
                    && self.db.variable_lifetime(*declaration)
                        == crate::core::db::VariableLifetime::Automatic
                {
                    locals.push((
                        self.node(*declaration).line,
                        self.node(*declaration).col,
                        *declaration,
                    ));
                    if let Some(initializer) = self.db.var_initializer(*declaration) {
                        assigned.entry(*declaration).or_default().push(initializer);
                    }
                }
            }
            if let NodeKind::Stmt(StmtKind::Assign { .. }) = self.kind(node) {
                if let [lhs, rhs, ..] = self.node(node).children.as_slice() {
                    if let NodeKind::Expr(ExprKind::Ref {
                        target: Some(target),
                    }) = self.kind(*lhs)
                    {
                        assigned.entry(*target).or_default().push(*rhs);
                    }
                }
            }
            for child in &self.node(node).children {
                parents.entry(*child).or_default().push(node);
                pending.push(*child);
            }
        }
        locals.sort_by_key(|(line, col, _)| (*line, *col));
        for (_, _, local) in locals {
            if !self.local_written_only_by_assignment(local, body, &parents) {
                continue;
            }
            let mut bound: Option<IrChandleExpr> = None;
            let mut consistent = true;
            for value in assigned.get(&local).into_iter().flatten().copied() {
                match self.callee_handle(value, &receivers) {
                    Some(handle) if bound.as_ref().is_none_or(|bound| *bound == handle) => {
                        bound = Some(handle);
                    }
                    _ => consistent = false,
                }
            }
            if consistent {
                if let Some(handle) = bound {
                    receivers.formals.insert(local, Some(handle));
                }
            }
        }
        receivers
    }

    /// Whether every reference to `local` in `body` reads it or is the
    /// target of a whole assignment: no output, `inout` or `ref` actual and
    /// no system task argument can write it behind the binding.
    fn local_written_only_by_assignment(
        &self,
        local: NodeId,
        body: NodeId,
        parents: &HashMap<NodeId, Vec<NodeId>>,
    ) -> bool {
        let mut pending = vec![body];
        while let Some(node) = pending.pop() {
            pending.extend(self.node(node).children.iter().copied());
            if !matches!(self.kind(node), NodeKind::Expr(ExprKind::Ref { target: Some(target) }) if *target == local)
            {
                continue;
            }
            for parent in parents.get(&node).into_iter().flatten().copied() {
                match self.kind(parent) {
                    NodeKind::SysCall { .. } => return false,
                    NodeKind::MethodCall { receiver, .. } if *receiver == Some(node) => {}
                    NodeKind::FuncCall {
                        callee: Some(callee),
                        ..
                    }
                    | NodeKind::MethodCall {
                        callee: Some(callee),
                        ..
                    } => {
                        let callee = *callee;
                        let Some(position) = self
                            .call_argument_nodes(parent)
                            .iter()
                            .position(|argument| *argument == node)
                        else {
                            return false;
                        };
                        let input = self
                            .node(callee)
                            .children
                            .iter()
                            .filter(|child| matches!(self.kind(**child), NodeKind::FuncArg { .. }))
                            .nth(position)
                            .is_some_and(|formal| {
                                matches!(
                                    self.kind(*formal),
                                    NodeKind::FuncArg {
                                        direction: crate::core::db::Direction::Input,
                                        ..
                                    }
                                )
                            });
                        if !input {
                            return false;
                        }
                    }
                    NodeKind::FuncCall { .. } | NodeKind::MethodCall { .. } => return false,
                    _ => {}
                }
            }
        }
        true
    }

    /// A class-handle property access on `receiver`, as a handle value.
    fn property_handle(&mut self, receiver: IrChandleExpr, field: NodeId) -> Option<IrChandleExpr> {
        if self.direct_field_type(field) != Some(IrClassFieldType::Chandle) {
            return None;
        }
        let (class, index) = self.class_fields.get(&field).copied()?;
        Some(IrChandleExpr::LocalRead(self.native_access_symbol(
            receiver,
            IrNativeAccessKind::ClassField {
                class,
                field: index,
            },
        )))
    }

    /// The handle a declaration names inside a callee body: a bound formal,
    /// a handle property of `this`, or a static handle variable.
    fn callee_root_handle(
        &mut self,
        target: NodeId,
        receivers: &CalleeReceivers,
    ) -> Option<IrChandleExpr> {
        if let Some(bound) = receivers.formals.get(&target) {
            return bound.clone();
        }
        if self.class_fields.contains_key(&target) {
            let this = receivers.this.clone()?;
            return self.property_handle(this, target);
        }
        let object = self
            .object_globals
            .get(&target)
            .map(|object| self.reference_object(*object))?;
        (self.model.objects.get(object)?.ty == crate::sim::ir::IrObjectType::Chandle)
            .then_some(IrChandleExpr::Read(object))
    }

    /// The handle value expression `node` evaluates to inside a callee body,
    /// when it can be evaluated again whenever the wait arms.
    fn callee_handle(
        &mut self,
        node: NodeId,
        receivers: &CalleeReceivers,
    ) -> Option<IrChandleExpr> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => self.callee_root_handle(*target, receivers),
            NodeKind::Expr(ExprKind::HierPath { parts, refs }) => {
                if refs.len() == 1
                    && refs[0].is_none()
                    && parts.first().is_some_and(|part| part == "this")
                {
                    return receivers.this.clone();
                }
                let field = refs.iter().rev().flatten().copied().next()?;
                if refs.len() == 1 {
                    return self.callee_root_handle(field, receivers);
                }
                let receiver = self.callee_property_receiver(node, field, receivers)?;
                self.property_handle(receiver, field)
            }
            NodeKind::Expr(ExprKind::Cast { operand, ty, .. }) if ty.kind == "class" => {
                self.callee_handle(*operand, receivers)
            }
            _ => None,
        }
    }

    /// The receiver of property `field` that `node` selects inside a callee
    /// body (see `explicit_class_receiver` for the path shapes).
    fn callee_property_receiver(
        &mut self,
        node: NodeId,
        field: NodeId,
        receivers: &CalleeReceivers,
    ) -> Option<IrChandleExpr> {
        let NodeKind::Expr(ExprKind::HierPath { parts, refs }) = self.kind(node) else {
            // A bare property name inside a method: the implicit `this`.
            return receivers.this.clone();
        };
        let end = refs
            .iter()
            .rposition(|reference| *reference == Some(field))
            .unwrap_or(refs.len());
        if end == 0 {
            return receivers.this.clone();
        }
        if refs[end - 1].is_some_and(|previous| {
            self.direct_field_type(previous) == Some(IrClassFieldType::Chandle)
        }) && end > 1
        {
            // The frontend nests each member access in the next: descend to
            // the access that ends at the handle property.
            let mut current = node;
            while let [child] = self.node(current).children.as_slice() {
                let child = *child;
                match self.kind(child) {
                    NodeKind::Expr(ExprKind::HierPath { refs: prefix, .. })
                        if prefix.len() > end =>
                    {
                        current = child;
                    }
                    _ => return self.callee_handle(child, receivers),
                }
            }
            return None;
        }
        if end == 1 {
            return match refs[0] {
                Some(root) => self.callee_root_handle(root, receivers),
                None if parts.first().is_some_and(|part| part == "this") => receivers.this.clone(),
                None => None,
            };
        }
        None
    }

    /// Dependencies of one class property read inside a callee body.
    fn callee_property_dependencies(
        &mut self,
        path: &str,
        node: NodeId,
        field: NodeId,
        receivers: &CalleeReceivers,
        out: &mut Vec<IrDependency>,
    ) -> Result<(), String> {
        let name = self.node(field).name.clone();
        if self.class_native_fields.contains_key(&field) {
            let Some(receiver) = self.callee_property_receiver(node, field, receivers) else {
                return Err(self.unobservable_read(
                    path,
                    node,
                    &format!(
                        "record class property `{name}` read by a called subroutine through a handle other than a class-handle input formal, `this`, a handle property or a static handle"
                    ),
                ));
            };
            out.extend(self.class_record_dependencies(receiver, field));
            return Ok(());
        }
        if self.direct_field_type(field).is_none() {
            return Err(self.unobservable_read(
                path,
                node,
                &format!("class property `{name}` read by a called subroutine has no storage a wait can name"),
            ));
        }
        let Some(receiver) = self.callee_property_receiver(node, field, receivers) else {
            return Err(self.unobservable_read(
                path,
                node,
                &format!(
                    "class property `{name}` read by a called subroutine through a handle other than a class-handle input formal, `this`, a handle property or a static handle"
                ),
            ));
        };
        let Some((class, index)) = self.class_fields.get(&field).copied() else {
            return Ok(());
        };
        self.class_event_identity_read |= self.is_class_event_field(field);
        let access = self.native_access_symbol(
            receiver,
            IrNativeAccessKind::ClassField {
                class,
                field: index,
            },
        );
        out.push(IrDependency::NativeAccess(access.clone()));
        self.handle_property_receivers(&access, out);
        Ok(())
    }

    /// Dependencies of one virtual-interface member read inside a callee
    /// body, through a bound virtual-interface formal, a module or static
    /// handle, or a handle selected through a class property.
    fn callee_interface_member(
        &mut self,
        path: &str,
        node: NodeId,
        receivers: &CalleeReceivers,
        out: &mut Vec<IrDependency>,
    ) -> Result<(), String> {
        let unobservable = |cg: &Self| {
            cg.unobservable_read(
                path,
                node,
                "a virtual-interface member read by a called subroutine through a handle other than a virtual-interface formal, a static handle or a class property",
            )
        };
        let Some(target) = self.virtual_interface_member_path(path, node, false)? else {
            return Err(unobservable(self));
        };
        let Some(slot) = self
            .virtual_interface_members
            .get(&(target.descriptor, target.member.clone()))
            .copied()
        else {
            return Err(unobservable(self));
        };
        let handle = if target.position > 0 {
            self.member_prefix_expression(node, target.position + 1)
                .and_then(|prefix| self.callee_handle(prefix, receivers))
        } else {
            self.callee_root_handle(target.handle, receivers)
        };
        let Some(handle) = handle.filter(Self::rearmable_handle) else {
            return Err(unobservable(self));
        };
        let access = self.native_access_symbol(
            handle,
            IrNativeAccessKind::InterfaceMember {
                interface: target.descriptor,
                member: slot,
            },
        );
        out.push(IrDependency::NativeAccess(access.clone()));
        self.handle_property_receivers(&access, out);
        self.root_handle_dependency(&access, out);
        Ok(())
    }

    /// Collect the dependencies of the class storage a called function or
    /// method body reads (and, for a method, the module storage it reads,
    /// which the ordinary read walk does not follow into method bodies).
    fn callee_dependencies(
        &mut self,
        path: &str,
        call: NodeId,
        callee: NodeId,
        receivers: CalleeReceivers,
        walk: &mut CalleeWalk<CalleeReceivers>,
        out: &mut Vec<IrDependency>,
    ) -> Result<(), String> {
        let Some(body) = self.func_body(callee) else {
            return Ok(());
        };
        if walk.cuts(callee) {
            if self.reads_dynamic_storage_in(body, &mut IdSet::new()) {
                return Err(self.unobservable_read(
                    path,
                    call,
                    "the class storage a recursive call reads",
                ));
            }
            return Ok(());
        }
        if walk.has_walked(callee, &receivers) {
            return Ok(());
        }
        // The locals the body binds are not part of what it was entered with.
        let entered = receivers.clone();
        let receivers = self.bind_callee_locals(body, receivers);
        let cuts = walk.cuts;
        walk.stack.push(callee);
        if self.class_method_owner(callee).is_some() {
            let reads = self.collect_read_signals(path, body)?;
            if !reads.is_empty() {
                let mut declared = DeclaredStorage::default();
                self.add_declared_storage(callee, &mut declared);
                out.extend(reads.into_iter().filter(|dependency| {
                    !self.activation_dependency(dependency) && !declared.holds(dependency)
                }));
            }
        }
        let mut pending = vec![body];
        while let Some(node) = pending.pop() {
            self.reject_unobservable_read(path, node)?;
            if self.mailbox_num_call(node) {
                let handle = match self.kind(node) {
                    NodeKind::MethodCall {
                        receiver: Some(receiver),
                        ..
                    } => self.callee_handle(*receiver, &receivers),
                    _ => None,
                };
                let Some(handle) = handle else {
                    return Err(self.unobservable_read(
                        path,
                        node,
                        "a mailbox `num()` a called subroutine makes through a handle other than a handle formal, `this`, a handle property or a static handle",
                    ));
                };
                let access = self.native_access_symbol(handle, IrNativeAccessKind::MailboxCount);
                out.push(IrDependency::NativeAccess(access));
                pending.extend(self.node(node).children.iter().copied());
                continue;
            }
            if self.dynamic_interface_member(node) {
                self.callee_interface_member(path, node, &receivers, out)?;
                continue;
            }
            if let Some(container) = self.class_container_property(node) {
                let receiver = self
                    .container_property_declaration(node)
                    .and_then(|field| self.callee_property_receiver(node, field, &receivers));
                let Some(receiver) = receiver else {
                    return Err(self.unobservable_read(
                        path,
                        node,
                        &format!(
                            "container class property `{}` read by a called subroutine through a handle other than a class-handle input formal, `this`, a handle property or a static handle",
                            self.node(node).name
                        ),
                    ));
                };
                out.extend(self.class_container_dependencies(receiver, container));
                continue;
            }
            if let Some(field) = self.class_storage_field(node) {
                self.callee_property_dependencies(path, node, field, &receivers, out)?;
                continue;
            }
            if let Some(next) = self.called_subroutine(node) {
                let bound = self.nested_call_receivers(path, node, next, &receivers)?;
                self.callee_dependencies(path, node, next, bound, walk, out)?;
            }
            pending.extend(self.node(node).children.iter().copied());
        }
        walk.leave(callee, entered, cuts);
        Ok(())
    }

    /// Give every class-property or interface-member read in `root` its
    /// native access, keyed by the reading node, for the read walker. A call
    /// contributes the class storage its callee's body reads, through the
    /// handles the call binds (see [`CalleeReceivers`]).
    fn prepare_dynamic_reads(&mut self, path: &str, root: NodeId) -> Result<(), String> {
        self.reject_shared_automatic_records(path, root)?;
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            pending.extend(self.node(node).children.iter().copied());
            self.reject_unobservable_read(path, node)?;
            if self.mailbox_num_call(node) {
                let NodeKind::MethodCall {
                    receiver: Some(receiver),
                    ..
                } = self.kind(node)
                else {
                    continue;
                };
                let receiver = *receiver;
                let Some(handle) = self
                    .lower_chandle(path, receiver)
                    .ok()
                    .filter(Self::rearmable_handle)
                else {
                    return Err(self.unobservable_read(
                        path,
                        node,
                        "a mailbox `num()` through a handle the wait cannot evaluate again",
                    ));
                };
                let access = self.native_access_symbol(handle, IrNativeAccessKind::MailboxCount);
                self.dynamic_reads
                    .insert(node, vec![IrDependency::NativeAccess(access)]);
                continue;
            }
            if let Some(container) = self.class_container_property(node) {
                let receiver = match self
                    .container_of(node)
                    .and_then(|container| self.model.containers[container.ir].receiver.clone())
                {
                    Some(receiver) => Some(receiver),
                    None => self
                        .container_property_declaration(node)
                        .and_then(|field| self.class_receiver_for(path, node, field).ok()),
                }
                .filter(Self::rearmable_handle);
                let Some(receiver) = receiver else {
                    return Err(self.unobservable_read(
                        path,
                        node,
                        &format!(
                            "container class property `{}` selected through a handle the wait cannot evaluate again",
                            self.node(node).name
                        ),
                    ));
                };
                let dependencies = self.class_container_dependencies(receiver, container);
                self.dynamic_reads.insert(node, dependencies);
                continue;
            }
            if let Some(field) = self.class_record_property(node) {
                let receiver = self
                    .class_receiver_for(path, node, field)
                    .ok()
                    .filter(Self::rearmable_handle);
                let Some(receiver) = receiver else {
                    return Err(self.unobservable_read(
                        path,
                        node,
                        &format!(
                            "record class property `{}` selected through a handle the wait cannot evaluate again",
                            self.node(field).name
                        ),
                    ));
                };
                let dependencies = self.class_record_dependencies(receiver, field);
                self.dynamic_reads.insert(node, dependencies);
                continue;
            }
            if let Some(callee) = self.called_subroutine(node) {
                let receivers = self.root_call_receivers(path, node, callee)?;
                let mut dependencies = Vec::new();
                self.callee_dependencies(
                    path,
                    node,
                    callee,
                    receivers,
                    &mut CalleeWalk::default(),
                    &mut dependencies,
                )?;
                if !dependencies.is_empty() {
                    self.dynamic_reads.insert(node, dependencies);
                }
                continue;
            }
            let read = match self.dynamic_class_property(node) {
                Some(field)
                    if matches!(
                        self.direct_field_type(field),
                        Some(IrClassFieldType::String | IrClassFieldType::Chandle)
                    ) =>
                {
                    // String and handle properties have no packed cell: name
                    // the property itself.
                    let (class, index) = self.class_fields[&field];
                    self.class_event_identity_read |= self.is_class_event_field(field);
                    let receiver = self.class_receiver_for(path, node, field)?;
                    Some(IrExprKind::LocalRead(self.native_access_symbol(
                        receiver,
                        IrNativeAccessKind::ClassField {
                            class,
                            field: index,
                        },
                    )))
                }
                Some(_) => match self.class_field_cell(path, node)? {
                    Some((read, ..)) => Some(read.kind().clone()),
                    None => self
                        .class_field_expr(path, node)?
                        .map(|read| read.kind().clone()),
                },
                None if self.dynamic_interface_member(node) => self
                    .virtual_interface_member_expr(path, node)?
                    .map(|read| read.kind().clone()),
                None => None,
            };
            // Only accesses a waiting process can resolve again qualify; a
            // property no such access names would leave the wait blocked.
            let resolved = match read {
                Some(IrExprKind::LocalRead(name))
                    if self.model.native_accesses.iter().any(|access| {
                        access.name == name
                            && matches!(
                                access.kind,
                                IrNativeAccessKind::ClassField { .. }
                                    | IrNativeAccessKind::InterfaceMember { .. }
                            )
                    }) =>
                {
                    Some(name)
                }
                _ => None,
            };
            let Some(name) = resolved else {
                if self.dynamic_class_property(node).is_some() {
                    return Err(self.unobservable_read(
                        path,
                        node,
                        &format!(
                            "class property `{}` selected through a handle the wait cannot evaluate again",
                            self.node(node).name
                        ),
                    ));
                }
                continue;
            };
            let mut dependencies = vec![IrDependency::NativeAccess(name.clone())];
            self.handle_property_receivers(&name, &mut dependencies);
            self.dynamic_reads.insert(node, dependencies);
        }
        Ok(())
    }

    /// The handle properties a native access selects its receiver through
    /// (`n.next` for `n.next.v`): rebinding one of them moves the access.
    fn handle_property_receivers(&self, name: &str, out: &mut Vec<IrDependency>) {
        let mut current = name.to_owned();
        while let Some(access) = self
            .model
            .native_accesses
            .iter()
            .find(|access| access.name == current)
        {
            let mut receiver = &access.receiver;
            // A nested interface instance (`v.tx.x`) is selected through
            // the handle, so rebinding the handle moves the access too.
            while let IrChandleExpr::Required { handle, .. }
            | IrChandleExpr::VirtualInterfaceNested { handle, .. } = receiver
            {
                receiver = handle;
            }
            let IrChandleExpr::LocalRead(inner) = receiver else {
                return;
            };
            let handle_property = self.model.native_accesses.iter().any(|access| {
                access.name == *inner
                    && matches!(
                        access.kind,
                        crate::sim::ir::IrNativeAccessKind::ClassField { class, field }
                            if self.model.classes.get(class).and_then(|class| class.fields.get(field)).is_some_and(|field| {
                                field.ty == IrClassFieldType::Chandle
                                    && field.container.is_none()
                                    && field.native_value.is_none()
                            })
                    )
            });
            if !handle_property || out.len() >= MAX_HANDLE_PROPERTY_DEPTH {
                return;
            }
            out.push(IrDependency::NativeAccess(inner.clone()));
            current = inner.clone();
        }
    }

    /// Run `collect` (a read walk over `root`) with the class-property and
    /// interface-member reads of `root` resolved to native-access
    /// dependencies. Use it only where the consumer arms its wait again
    /// after every wake.
    pub(in super::super) fn with_dynamic_reads(
        &mut self,
        path: &str,
        root: NodeId,
        collect: impl FnOnce(&Self) -> Result<Vec<IrDependency>, String>,
    ) -> Result<Vec<IrDependency>, String> {
        self.with_prepared_reads(path, root, collect)
    }

    fn with_prepared_reads(
        &mut self,
        path: &str,
        root: NodeId,
        collect: impl FnOnce(&Self) -> Result<Vec<IrDependency>, String>,
    ) -> Result<Vec<IrDependency>, String> {
        // Unpacked members through handles are read through views of their
        // storage, resolved through the handle when the wait arms.
        let nodes = if self.interface_views_possible {
            self.implicit_read_nodes(root, true)
        } else {
            Vec::new()
        };
        let views = self.stage_dependency_views(path, &nodes)?;
        self.dynamic_read_walk = true;
        let prepared = self.prepare_dynamic_reads(path, root);
        let result = prepared.and_then(|()| collect(self));
        self.dynamic_read_walk = false;
        self.dynamic_reads.clear();
        self.end_dependency_views(views, result)
    }

    /// End the staging [`Self::stage_dependency_views`] opened, if `views`,
    /// and resolve the dependencies of its views.
    fn end_dependency_views(
        &mut self,
        views: bool,
        result: Result<Vec<IrDependency>, String>,
    ) -> Result<Vec<IrDependency>, String> {
        if !views {
            return result;
        }
        // A read walk places no binding, so ending it cannot fail.
        let result = self
            .end_interface_views()
            .and(result)
            .and_then(|reads| self.interface_view_dependencies(reads));
        self.dependency_views.clear();
        result
    }

    /// Run `collect` (an implicit-sensitivity read walk over `root`) with
    /// the virtual-interface member reads of `root` resolved to dependencies
    /// on the storage of the instance the handle binds. always_comb,
    /// always_latch and `@*` are sensitive to what the block reads (SV
    /// 9.2.2.2.1, 9.4.2.2); through a virtual interface that is the member of
    /// the instance the handle names (25.9), so the process waits on that
    /// member, resolved again every time it arms, beside the handle variable
    /// the read walk already adds: rebinding the handle wakes the process,
    /// which arms on the new instance. Members the block also writes through
    /// the same handle and member are left out (9.2.2.2.1 b). Function bodies
    /// are followed when `include_function_bodies` (always_comb).
    pub(in super::super) fn with_interface_reads(
        &mut self,
        path: &str,
        root: NodeId,
        include_function_bodies: bool,
        collect: impl FnOnce(&Self) -> Result<Vec<IrDependency>, String>,
    ) -> Result<Vec<IrDependency>, String> {
        if self.model.virtual_interfaces.is_empty() {
            return collect(self);
        }
        let nodes = if self.interface_views_possible {
            self.implicit_read_nodes(root, include_function_bodies)
        } else {
            Vec::new()
        };
        let views = self.stage_dependency_views(path, &nodes)?;
        let saved_reads = std::mem::take(&mut self.dynamic_reads);
        let saved_errors = std::mem::take(&mut self.dynamic_read_errors);
        let prepared = self.prepare_interface_reads(path, root, include_function_bodies);
        let result = prepared.and_then(|()| collect(self));
        self.dynamic_reads = saved_reads;
        self.dynamic_read_errors = saved_errors;
        self.end_dependency_views(views, result)
    }

    /// The nodes an implicit-sensitivity walk of `root` can reach, each with
    /// whether it lies in a called function's body. Procedural concurrent
    /// assertions (S99-D2), class property paths and class method bodies add
    /// nothing (SV 9.2.2.2.1), so they are not entered.
    fn implicit_read_nodes(
        &self,
        root: NodeId,
        include_function_bodies: bool,
    ) -> Vec<(NodeId, bool)> {
        let mut nodes = Vec::new();
        let mut visited = IdSet::new();
        let mut pending = vec![(root, false)];
        while let Some((node, in_callee)) = pending.pop() {
            match self.kind(node) {
                NodeKind::Stmt(StmtKind::ConcurrentAssertion { kind, .. })
                    if *kind != ConcurrentAssertionKind::Expect =>
                {
                    continue;
                }
                NodeKind::Expr(ExprKind::HierPath { .. })
                    if self.class_field_target(node).is_some() =>
                {
                    continue;
                }
                NodeKind::FuncCall {
                    callee: Some(callee),
                    is_task: false,
                    ..
                } if include_function_bodies && visited.insert(*callee) => {
                    if let Some(body) = self.func_body(*callee) {
                        pending.push((body, true));
                    }
                }
                _ => {}
            }
            nodes.push((node, in_callee));
            pending.extend(
                self.node(node)
                    .children
                    .iter()
                    .rev()
                    .map(|child| (*child, in_callee)),
            );
        }
        nodes
    }

    /// The handle and member of each virtual-interface member an assignment
    /// target `lhs` writes (`v.x`, `v.x[i]`, `v.mem[i]`, concatenations).
    fn written_interface_members(
        &mut self,
        path: &str,
        lhs: NodeId,
        out: &mut HashSet<(NodeId, String)>,
    ) -> Result<(), String> {
        let base = match self.kind(lhs) {
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Concat,
                ..
            }) => {
                for child in self.node(lhs).children.clone() {
                    self.written_interface_members(path, child, out)?;
                }
                return Ok(());
            }
            NodeKind::Expr(ExprKind::HierPath { .. }) => {
                if let Some(key) = self.virtual_interface_member_key(path, lhs)? {
                    out.insert(key);
                }
                return Ok(());
            }
            NodeKind::Expr(
                ExprKind::BitSelect { base, .. }
                | ExprKind::ArraySelect { base, .. }
                | ExprKind::PartSelect { base, .. }
                | ExprKind::IndexedPartSelect { base, .. },
            ) => *base,
            _ => return Ok(()),
        };
        self.written_interface_members(path, base, out)
    }

    /// A located rejection of a virtual-interface read that an implicit
    /// sensitivity cannot follow.
    fn unobservable_interface_read(&self, path: &str, node: NodeId, what: &str) -> String {
        self.locate_error(
            node,
            format!(
                "the implicit sensitivity of `{}` cannot follow {what} (IEEE 1800-2009 9.2.2.2.1, 25.9)",
                self.source_path(path)
            ),
        )
    }

    fn prepare_interface_reads(
        &mut self,
        path: &str,
        root: NodeId,
        include_function_bodies: bool,
    ) -> Result<(), String> {
        let nodes = self.implicit_read_nodes(root, include_function_bodies);
        let mut written = HashSet::new();
        for (node, _) in &nodes {
            if let NodeKind::Stmt(StmtKind::Assign { .. }) = self.kind(*node) {
                if let Some(lhs) = self.node(*node).children.first().copied() {
                    self.written_interface_members(path, lhs, &mut written)?;
                }
            }
        }
        // Parents are listed before their children, so a member path that
        // fails (`v.s.f`) yields to a member path below it that succeeds.
        let mut failed: Vec<(NodeId, String)> = Vec::new();
        for (node, in_callee) in nodes {
            // always_comb follows called functions, but not into the
            // implementation a virtual-interface method call selects.
            if include_function_bodies && self.virtual_interface_method_call(node) {
                let error = self.unobservable_interface_read(
                    path,
                    node,
                    "the storage a virtual-interface method call reads",
                );
                self.dynamic_read_errors.insert(node, error);
                continue;
            }
            if !self.dynamic_interface_member(node) {
                continue;
            }
            let key = match self.virtual_interface_member_key(path, node) {
                Ok(key) => key,
                Err(error) => {
                    failed.push((node, error));
                    continue;
                }
            };
            if key.is_some_and(|key| written.contains(&key)) {
                continue;
            }
            let read = match self.virtual_interface_member_expr(path, node) {
                Ok(read) => read,
                // A handle formal or local of a called function has no
                // value when the process arms.
                Err(_) if in_callee => {
                    let error = self.unobservable_interface_read(path, node, CALLEE_HANDLE);
                    self.dynamic_read_errors.insert(node, error);
                    continue;
                }
                Err(error) => {
                    failed.push((node, error));
                    continue;
                }
            };
            let Some(IrExprKind::LocalRead(name)) = read.map(|read| read.kind().clone()) else {
                continue;
            };
            let receiver = self
                .model
                .native_accesses
                .iter()
                .find(|access| access.name == name)
                .map(|access| access.receiver.clone());
            // Inside a called function only a module-level handle names the
            // same storage when the process arms.
            let followed = receiver.is_some_and(|receiver| {
                if in_callee {
                    matches!(receiver, IrChandleExpr::Read(_))
                } else {
                    Self::rearmable_handle(&receiver)
                }
            });
            if !followed {
                let error = self.unobservable_interface_read(
                    path,
                    node,
                    if in_callee {
                        CALLEE_HANDLE
                    } else {
                        "a virtual-interface member read through a handle that is neither a variable nor a formal"
                    },
                );
                self.dynamic_read_errors.insert(node, error);
                continue;
            }
            self.dynamic_reads
                .entry(node)
                .or_default()
                .push(IrDependency::NativeAccess(name));
        }
        for (node, error) in failed {
            let resolved_below = self.node(node).children.iter().any(|child| {
                self.dynamic_reads.contains_key(child)
                    || self.dynamic_read_errors.contains_key(child)
            });
            if !resolved_below {
                self.dynamic_read_errors.insert(node, error);
            }
        }
        Ok(())
    }
}
