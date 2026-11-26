//! Views of unpacked interface members through virtual-interface handles
//! (SV 25.9): `v.a` where `a` is a fixed array of any element type, a queue,
//! or a dynamic or associative array of the interface.
//!
//! A statement that evaluates its operands once binds a view of each such
//! member it names before it runs, and an expression evaluated more than
//! once (a condition, a loop bound) binds before each evaluation
//! ([`Codegen::stage_interface_views`]). The statement then lowers once
//! against the view as it would against the member of one instance: a view
//! is an array marked `bound`, or an activation container, of the member's
//! shape, which `IrStmt::InterfaceBind` binds to the descriptor's storage
//! slot of the instance the handle binds. Generated code does not grow with
//! the number of instances a handle can bind.
//!
//! The frontend names the member's declaration in its interface type as the
//! base of an element select (`v.a[i]`), whatever handle selects it. While a
//! staging has one view of a member, that declaration stands for the view;
//! when two views share it (`w.a[0] = v.a[1]`), each select binds its own
//! view while it lowers ([`Codegen::lower_interface_select`]).
//!
//! Where something the statement runs may rebind a handle (a call of a user
//! subroutine, an assignment operator), a use reads the handle where it is
//! evaluated (decision VM-D1). A handle held in storage is read again by
//! each use (`IrInterfaceBinding::refresh`). A computed handle (`va[k]`) is
//! evaluated once per use: a use that such an effect precedes in evaluation
//! order gets a view of its own, bound by the lowering of the node the use
//! starts at ([`Codegen::placed_interface_bindings`]), so the handle is read
//! after that effect and before the use's own selects; a use no effect
//! precedes binds before the statement, as every use of a statement without
//! such effects does.

use super::virtual_interfaces::VirtualInterfaceMemberPath;
use super::*;
use crate::sim::codegen::lowering::collection::data_query;
use crate::sim::codegen::lowering::expressions::after_statements;
use crate::sim::ir::{IrDependency, IrInterfaceBinding, IrInterfacePart, IrInterfaceStorage};

/// What a wait on a view staged for a read walk resolves when it arms: the
/// handle, descriptor and storage slot; or the located reason no wait can.
pub(in crate::sim::codegen) type DependencyView = Result<(IrChandleExpr, usize, usize), String>;

/// A view of the storage of an unpacked interface member, named by the uses
/// of one handle path in a staging.
#[derive(Clone)]
pub(in crate::sim::codegen) struct InterfaceView {
    storage: IrInterfaceStorage,
    array: Option<ArrayInfo>,
    /// The member's declaration that a select of it names as its base.
    declaration: Option<NodeId>,
}

/// What one staging of views added, so that ending it removes exactly that.
#[derive(Default)]
pub(in crate::sim::codegen) struct InterfaceStaging {
    paths: Vec<NodeId>,
    /// Declarations bound to the one view of the staging that names them,
    /// with the binding each replaced.
    declarations: Vec<(NodeId, DeclarationBinding)>,
    /// Declarations that two views of the staging share.
    shared: HashSet<NodeId>,
    /// The nodes whose lowering runs a binding of the staging
    /// (`Codegen::placed_interface_bindings`).
    placed: Vec<NodeId>,
}

/// How [`Codegen::stage_use`] binds the view of a use.
enum StagedBinding {
    /// A new view bound before the statement.
    Before(IrStmt),
    /// A view of the use's own, bound where the use starts.
    Placed(IrStmt),
    /// The view of an earlier use of the same handle path.
    Shared,
}

/// One step of the evaluation-order walk of
/// [`Codegen::uses_after_effects`].
enum OrderStep {
    /// Evaluate the node.
    Visit(NodeId),
    /// The effect of an enclosing node has happened.
    Effect,
    /// A called subroutine copies its formal out to this actual.
    CopyOut(NodeId),
}

fn visit(nodes: &[NodeId]) -> impl Iterator<Item = OrderStep> + '_ {
    nodes.iter().map(|node| OrderStep::Visit(*node))
}

/// What a declaration resolves to in the storage tables.
#[derive(Clone)]
pub(in crate::sim::codegen) struct DeclarationBinding {
    array: Option<ArrayInfo>,
    container: Option<ContainerInfo>,
}

impl Codegen<'_> {
    /// Note whether some member path names a member of an interface through
    /// a handle that has no packed slot: only then do statements look for
    /// views to bind. Runs once the descriptors exist.
    pub(in super::super) fn note_interface_views(&mut self) {
        if self.model.virtual_interfaces.is_empty() {
            return;
        }
        self.interface_views_possible = self.db.node_ids().any(|node| {
            self.virtual_interface_member_path("", node, false)
                .ok()
                .flatten()
                .is_some_and(|target| {
                    !self
                        .virtual_interface_members
                        .contains_key(&(target.descriptor, target.member))
                })
        });
    }

    /// The view staged for member path `node`, if any.
    pub(in super::super) fn staged_interface_view(&self, node: NodeId) -> Option<&InterfaceView> {
        if self.staged_interface_views.is_empty() {
            return None;
        }
        self.staged_interface_views.get(&node)
    }

    /// The array info of the view staged for `node`.
    pub(in super::super) fn interface_view_array(&self, node: NodeId) -> Option<&ArrayInfo> {
        self.staged_interface_view(node)
            .and_then(|view| view.array.as_ref())
    }

    /// The container of the view staged for `node`.
    pub(in super::super) fn interface_view_container(&self, node: NodeId) -> Option<ContainerInfo> {
        match self.staged_interface_view(node)?.storage {
            IrInterfaceStorage::Container(ir) => Some(ContainerInfo { ir }),
            IrInterfaceStorage::Array(_) => None,
        }
    }

    /// Bind views of the unpacked members that the operands of `root`
    /// (a statement that evaluates them once, or an expression) name
    /// through handles and that no open staging binds yet: the uses of one
    /// handle path share one view. Returns the bindings, or `None` when
    /// there is nothing to bind; the caller lowers `root` and then ends the
    /// staging with [`Self::end_interface_views`]. `refresh` binds each use
    /// again where it runs whatever `root` calls.
    pub(in crate::sim::codegen) fn stage_interface_views(
        &mut self,
        path: &str,
        root: NodeId,
        refresh: bool,
    ) -> Result<Option<Vec<IrStmt>>, String> {
        if !self.interface_views_possible {
            return Ok(None);
        }
        let mut uses = Vec::new();
        self.collect_interface_view_uses(root, &mut uses);
        if uses.is_empty() {
            return Ok(None);
        }
        let mut written = Vec::new();
        self.interface_view_writes(root, &mut written);
        let refresh = refresh || self.may_rebind_handles(root);
        let placed = if refresh {
            self.uses_after_effects(root, &uses)
        } else {
            Vec::new()
        };
        let uses = uses
            .into_iter()
            .map(|node| {
                let start = placed
                    .iter()
                    .find(|(path, _)| *path == node)
                    .map(|(_, start)| *start);
                (node, written.contains(&node), start)
            })
            .collect();
        self.stage_views(path, uses, refresh, None)
    }

    /// The bindings among `bindings`, a timed blocking assignment's staging,
    /// of the views its target `lhs` names. The assignment runs them again
    /// when its control has passed, where it locates the target, so the
    /// handle, a computed one too, is read then (SV 10.4.1 L13024-13025: "If
    /// variable_lvalue requires an evaluation, it shall be evaluated at the
    /// time specified by the intra-assignment timing control").
    pub(in crate::sim::codegen) fn target_interface_bindings(
        &self,
        lhs: NodeId,
        bindings: &[IrStmt],
    ) -> Vec<IrStmt> {
        let mut views = Vec::new();
        let mut pending = vec![lhs];
        while let Some(node) = pending.pop() {
            if let Some(view) = self.staged_interface_view(node) {
                views.push(view.storage);
            }
            pending.extend(self.node(node).children.iter().copied());
        }
        bindings
            .iter()
            .filter(|statement| {
                matches!(statement, IrStmt::InterfaceBind(binding) if views.contains(&binding.view))
            })
            .cloned()
            .collect()
    }

    /// Stage views of the unpacked members that the nodes of a read walk
    /// name through handles (`nodes`, each with whether it lies in a called
    /// function's body), for the walk alone: a read of a view is a
    /// dependency on its storage, which [`Self::interface_view_dependencies`]
    /// turns into a wait on the storage of the instance the handle binds
    /// when the wait arms. Returns whether views were staged; the caller
    /// ends the staging with [`Self::end_interface_views`].
    pub(in crate::sim::codegen) fn stage_dependency_views(
        &mut self,
        path: &str,
        nodes: &[(NodeId, bool)],
    ) -> Result<bool, String> {
        if !self.interface_views_possible {
            return Ok(false);
        }
        let callee: HashSet<NodeId> = nodes
            .iter()
            .filter(|(_, in_callee)| *in_callee)
            .map(|(node, _)| *node)
            .collect();
        let uses: Vec<(NodeId, bool, Option<NodeId>)> = nodes
            .iter()
            .filter(|(node, _)| self.names_member_through_handle(*node))
            .map(|(node, _)| (*node, false, None))
            .collect();
        if uses.is_empty() {
            return Ok(false);
        }
        Ok(self
            .stage_views(path, uses, false, Some(&callee))?
            .is_some())
    }

    /// Stage a view for each use `(node, written, start)` that names an
    /// unpacked member through a handle; the uses of one handle path share
    /// one view. A use with a `start` node that reads a computed handle gets
    /// a view of its own, which the lowering of `start` binds. A read walk's
    /// staging (`callee` holds the uses in called functions) notes what each
    /// view's dependency resolves.
    fn stage_views(
        &mut self,
        path: &str,
        uses: Vec<(NodeId, bool, Option<NodeId>)>,
        refresh: bool,
        callee: Option<&HashSet<NodeId>>,
    ) -> Result<Option<Vec<IrStmt>>, String> {
        let mut staging = InterfaceStaging::default();
        let mut bindings = Vec::new();
        let mut views: Vec<(NodeId, InterfaceView)> = Vec::new();
        // Views of uses bound where they start: no other use shares them.
        let mut own: Vec<InterfaceView> = Vec::new();
        for (node, written, start) in uses {
            match self.stage_use(path, node, written, start, refresh, callee, &mut views) {
                Ok(Some((view, binding))) => {
                    match (binding, start) {
                        (StagedBinding::Placed(binding), Some(start)) => {
                            self.placed_interface_bindings
                                .entry(start)
                                .or_default()
                                .push(binding);
                            if !staging.placed.contains(&start) {
                                staging.placed.push(start);
                            }
                            own.push(view.clone());
                        }
                        (StagedBinding::Placed(binding) | StagedBinding::Before(binding), _) => {
                            bindings.push(binding)
                        }
                        (StagedBinding::Shared, _) => {}
                    }
                    self.staged_interface_views.insert(node, view);
                    staging.paths.push(node);
                }
                Ok(None) => {}
                Err(error) => {
                    self.unstage_failed(staging);
                    return Err(error);
                }
            }
        }
        if staging.paths.is_empty() {
            return Ok(None);
        }
        let mut declarations: Vec<(NodeId, usize)> = Vec::new();
        for view in views.iter().map(|(_, view)| view).chain(&own) {
            let Some(declaration) = view.declaration else {
                continue;
            };
            match declarations
                .iter_mut()
                .find(|(seen, _)| *seen == declaration)
            {
                Some((_, count)) => *count += 1,
                None => declarations.push((declaration, 1)),
            }
        }
        for (declaration, count) in declarations {
            let view = views
                .iter()
                .map(|(_, view)| view)
                .chain(&own)
                .find(|view| view.declaration == Some(declaration))
                .cloned();
            let previous = if count == 1 {
                self.bind_interface_declaration(declaration, view.as_ref())
            } else {
                staging.shared.insert(declaration);
                self.bind_interface_declaration(declaration, None)
            };
            staging.declarations.push((declaration, previous));
        }
        self.interface_stagings.push(staging);
        Ok(Some(bindings))
    }

    /// The view of use `node` and how it is bound. A use with a `start`
    /// node through a computed handle gets a view of its own, whose binding
    /// the caller places at `start`; any other use shares the view of its
    /// handle path with the earlier uses in `views`.
    #[allow(clippy::too_many_arguments)]
    fn stage_use(
        &mut self,
        path: &str,
        node: NodeId,
        written: bool,
        start: Option<NodeId>,
        refresh: bool,
        callee: Option<&HashSet<NodeId>>,
        views: &mut Vec<(NodeId, InterfaceView)>,
    ) -> Result<Option<(InterfaceView, StagedBinding)>, String> {
        let Some((target, slot)) = self.interface_view_path(path, node, written)? else {
            return Ok(None);
        };
        // A handle held in storage is read again by each use instead.
        let lowered = match start {
            Some(_) => Some(self.virtual_interface_handle(path, node, &target)?),
            None => None,
        };
        if let Some(handle) = lowered
            .as_ref()
            .filter(|handle| !Self::rearmable_handle(handle))
        {
            let (view, binding) =
                self.new_interface_view(path, node, &target, slot, handle.clone(), false)?;
            return Ok(Some((view, StagedBinding::Placed(binding))));
        }
        if let Some((_, view)) = views
            .iter()
            .find(|(first, _)| self.same_member_path(*first, node))
        {
            return Ok(Some((view.clone(), StagedBinding::Shared)));
        }
        let handle = match lowered {
            Some(handle) => handle,
            None => self.virtual_interface_handle(path, node, &target)?,
        };
        let (view, binding) =
            self.new_interface_view(path, node, &target, slot, handle, refresh)?;
        if let Some(callee) = callee {
            let dependency =
                self.dependency_view(path, node, &target, &binding, callee.contains(&node));
            self.dependency_views.insert(view.storage, dependency);
        }
        views.push((node, view.clone()));
        Ok(Some((view, StagedBinding::Before(binding))))
    }

    /// Undo what a staging that failed added.
    fn unstage_failed(&mut self, staging: InterfaceStaging) {
        for node in staging.paths {
            self.staged_interface_views.remove(&node);
        }
        for start in staging.placed {
            self.placed_interface_bindings.remove(&start);
        }
    }

    /// The bindings that the lowering of `node` runs before it, if a use
    /// starts there (`placed_interface_bindings`).
    pub(in super::super) fn take_placed_interface_bindings(
        &mut self,
        node: NodeId,
    ) -> Option<Vec<IrStmt>> {
        if self.placed_interface_bindings.is_empty() {
            return None;
        }
        self.placed_interface_bindings.remove(&node)
    }

    /// `lhs`, the target of a use that starts at it, with `bindings` run
    /// just before its first selector: after the right-hand side, which the
    /// target follows (SV 10.4.1), and before the selects of the element.
    pub(in super::super) fn target_after_bindings(
        &self,
        lhs: IrLhs,
        bindings: Vec<IrStmt>,
    ) -> Result<IrLhs, String> {
        match lhs {
            IrLhs::ArrayElem {
                arr,
                mut indices,
                elem_sel,
            } if !indices.is_empty() => {
                let first = indices.remove(0);
                indices.insert(0, after_statements(bindings, first));
                Ok(IrLhs::ArrayElem {
                    arr,
                    indices,
                    elem_sel,
                })
            }
            _ => Err(Self::unplaced_interface_use()),
        }
    }

    /// `value`, a string element read of a use that starts at it, with
    /// `bindings` run just before its index, which precedes the read.
    pub(in super::super) fn string_after_bindings(
        &self,
        value: IrStringExpr,
        bindings: Vec<IrStmt>,
    ) -> Result<IrStringExpr, String> {
        match value {
            IrStringExpr::ContainerGet { container, index } => Ok(IrStringExpr::ContainerGet {
                container,
                index: Box::new(after_statements(bindings, *index)),
            }),
            IrStringExpr::ContainerGetNested {
                container,
                mut indices,
            } if !indices.is_empty() => {
                let first = indices.remove(0);
                indices.insert(0, after_statements(bindings, first));
                Ok(IrStringExpr::ContainerGetNested { container, indices })
            }
            _ => Err(Self::unplaced_interface_use()),
        }
    }

    fn unplaced_interface_use() -> String {
        "an unpacked interface member through a computed virtual-interface handle is not \
         supported in this operand of a statement that calls a subroutine or assigns in an \
         expression; assign the handle to a virtual-interface variable first"
            .to_owned()
    }

    /// What a wait on view storage bound by `binding` resolves when it arms:
    /// the handle, descriptor and slot. A handle the waiting process cannot
    /// evaluate again (a computed one, or inside a called function one that
    /// is not a module-level variable) cannot be followed.
    fn dependency_view(
        &self,
        path: &str,
        node: NodeId,
        target: &VirtualInterfaceMemberPath,
        binding: &IrStmt,
        in_callee: bool,
    ) -> DependencyView {
        let IrStmt::InterfaceBind(binding) = binding else {
            return Err("interface view without binding".to_owned());
        };
        let followed = if in_callee {
            matches!(binding.handle, IrChandleExpr::Read(_))
        } else {
            Self::rearmable_handle(&binding.handle)
        };
        if !followed {
            return Err(self.locate_error(
                node,
                format!(
                    "a wait in `{}` cannot follow unpacked member `{}` of virtual interface \
                     `{}` through a handle that {} (IEEE 1800-2009 9.2.2.2.1, 25.9)",
                    self.source_path(path),
                    target.member,
                    target.spelling,
                    if in_callee {
                        "a called function reads other than a virtual-interface variable"
                    } else {
                        "is neither a variable nor a formal"
                    }
                ),
            ));
        }
        Ok((binding.handle.clone(), binding.interface, binding.slot))
    }

    /// The dependencies of a read walk over views staged by
    /// [`Self::stage_dependency_views`], with each dependency on view storage
    /// replaced by a wait on that storage of the instance the view's handle
    /// binds when the wait arms, beside the handle variable: rebinding it
    /// wakes the waiter, which arms on the new instance.
    pub(in crate::sim::codegen) fn interface_view_dependencies(
        &mut self,
        reads: Vec<IrDependency>,
    ) -> Result<Vec<IrDependency>, String> {
        if self.dependency_views.is_empty() {
            return Ok(reads);
        }
        let mut out = Vec::with_capacity(reads.len());
        for read in reads {
            let (storage, part) = match &read {
                IrDependency::ArrayContents(array) => {
                    (IrInterfaceStorage::Array(*array), IrInterfacePart::Contents)
                }
                IrDependency::ArrayElement { array, index } => (
                    IrInterfaceStorage::Array(*array),
                    IrInterfacePart::Cell(*index),
                ),
                IrDependency::PackedRange { storage, .. }
                    if matches!(storage.as_ref(), IrDependency::ArrayElement { .. }) =>
                {
                    let IrDependency::ArrayElement { array, index } = storage.as_ref() else {
                        continue;
                    };
                    (
                        IrInterfaceStorage::Array(*array),
                        IrInterfacePart::Cell(*index),
                    )
                }
                IrDependency::ContainerContents(container) => (
                    IrInterfaceStorage::Container(*container),
                    IrInterfacePart::Contents,
                ),
                IrDependency::ContainerShape(container) => (
                    IrInterfaceStorage::Container(*container),
                    IrInterfacePart::Shape,
                ),
                _ => {
                    if !out.contains(&read) {
                        out.push(read);
                    }
                    continue;
                }
            };
            let Some(view) = self.dependency_views.get(&storage).cloned() else {
                if !out.contains(&read) {
                    out.push(read);
                }
                continue;
            };
            let (handle, interface, slot) = view?;
            let name = self.native_access_symbol(
                handle,
                crate::sim::ir::IrNativeAccessKind::InterfaceStorage {
                    interface,
                    slot,
                    part,
                },
            );
            let mut roots = vec![IrDependency::NativeAccess(name.clone())];
            self.root_handle_dependency(&name, &mut roots);
            for dependency in roots {
                if !out.contains(&dependency) {
                    out.push(dependency);
                }
            }
        }
        Ok(out)
    }

    /// Whether `node` is a member path through a virtual-interface handle
    /// that no staging binds yet.
    fn names_member_through_handle(&self, node: NodeId) -> bool {
        !self.staged_interface_views.contains_key(&node) && self.handle_member_path(node)
    }

    /// Whether `node` is a member path through a virtual-interface handle.
    fn handle_member_path(&self, node: NodeId) -> bool {
        matches!(self.kind(node), NodeKind::Expr(ExprKind::HierPath { refs, .. })
            if refs
                .iter()
                .flatten()
                .any(|target| self.virtual_interface_spelling(*target).is_some()))
    }

    /// End the innermost staging [`Self::stage_interface_views`] opened. A
    /// binding placed at a node whose lowering never ran it is a located
    /// error: its use would read a view nothing bound.
    pub(in crate::sim::codegen) fn end_interface_views(&mut self) -> Result<(), String> {
        let Some(staging) = self.interface_stagings.pop() else {
            return Ok(());
        };
        for node in staging.paths {
            self.staged_interface_views.remove(&node);
        }
        for (declaration, previous) in staging.declarations.into_iter().rev() {
            self.restore_interface_declaration(declaration, previous);
        }
        let mut unplaced = None;
        for start in staging.placed {
            if self.placed_interface_bindings.remove(&start).is_some() && unplaced.is_none() {
                unplaced = Some(start);
            }
        }
        match unplaced {
            Some(start) => Err(self.locate_error(start, Self::unplaced_interface_use())),
            None => Ok(()),
        }
    }

    /// Make `declaration` resolve to `view` (nothing for `None`) in the
    /// storage tables, returning what it resolved to before.
    fn bind_interface_declaration(
        &mut self,
        declaration: NodeId,
        view: Option<&InterfaceView>,
    ) -> DeclarationBinding {
        let previous = DeclarationBinding {
            array: self.array_globals.remove(&declaration),
            container: self.container_globals.remove(&declaration),
        };
        match view.map(|view| (view.storage, view.array.clone())) {
            Some((IrInterfaceStorage::Array(_), Some(array))) => {
                self.array_globals.insert(declaration, array);
            }
            Some((IrInterfaceStorage::Container(ir), _)) => {
                self.container_globals
                    .insert(declaration, ContainerInfo { ir });
            }
            _ => {}
        }
        previous
    }

    fn restore_interface_declaration(&mut self, declaration: NodeId, binding: DeclarationBinding) {
        self.array_globals.remove(&declaration);
        self.container_globals.remove(&declaration);
        if let Some(array) = binding.array {
            self.array_globals.insert(declaration, array);
        }
        if let Some(container) = binding.container {
            self.container_globals.insert(declaration, container);
        }
    }

    /// The select `h` of a member whose declaration two staged views share,
    /// and the view of its own member path.
    fn interface_select_view(&self, h: NodeId) -> Option<(NodeId, InterfaceView)> {
        if self.staged_interface_views.is_empty()
            || self
                .interface_stagings
                .iter()
                .all(|staging| staging.shared.is_empty())
        {
            return None;
        }
        let base = match self.kind(h) {
            NodeKind::Expr(
                ExprKind::ArraySelect { base, .. } | ExprKind::BitSelect { base, .. },
            ) => *base,
            _ => return None,
        };
        if !self
            .interface_stagings
            .iter()
            .any(|staging| staging.shared.contains(&base))
        {
            return None;
        }
        let path = self.select_member_path(h, base)?;
        let view = self.staged_interface_view(path)?.clone();
        Some((base, view))
    }

    /// The array view of the member path a select `select` names, when two
    /// views of the staging share its declaration.
    pub(in super::super) fn interface_select_array(&self, select: NodeId) -> Option<&ArrayInfo> {
        let (declaration, _) = self.interface_select_view(select)?;
        self.interface_view_array(self.select_member_path(select, declaration)?)
    }

    /// The container view of the member path a select `select` names, when
    /// two views of the staging share its declaration.
    pub(in super::super) fn interface_select_container(
        &self,
        select: NodeId,
    ) -> Option<ContainerInfo> {
        match self.interface_select_view(select)?.1.storage {
            IrInterfaceStorage::Container(ir) => Some(ContainerInfo { ir }),
            IrInterfaceStorage::Array(_) => None,
        }
    }

    /// Lower `h` with the declaration its select names bound to the view of
    /// its own member path, when two views of the staging share it.
    pub(in super::super) fn with_interface_select<T>(
        &mut self,
        h: NodeId,
        lower: impl FnOnce(&mut Self) -> Result<T, String>,
    ) -> Result<T, String> {
        let Some((declaration, view)) = self.interface_select_view(h) else {
            return lower(self);
        };
        let previous = self.bind_interface_declaration(declaration, Some(&view));
        let lowered = lower(self);
        self.restore_interface_declaration(declaration, previous);
        lowered
    }

    /// An expression evaluated outside a staged statement binds the views
    /// it names before each evaluation.
    pub(in super::super) fn lower_with_interface_views(
        &mut self,
        path: &str,
        h: NodeId,
    ) -> Result<Option<IrExpr>, String> {
        let Some(bindings) = self.stage_interface_views(path, h, false)? else {
            return Ok(None);
        };
        let value = self.lower_expr(path, h);
        let ended = self.end_interface_views();
        let value = value?;
        ended?;
        Ok(Some(after_statements(bindings, value)))
    }

    /// Whether evaluating `root` may rebind a handle before a use: it calls
    /// a subroutine or holds an assignment operator below its root.
    fn may_rebind_handles(&self, root: NodeId) -> bool {
        // A called subroutine at the root runs before its output actuals
        // are copied out.
        let mut pending: Vec<NodeId> = match self.kind(root) {
            NodeKind::FuncCall { .. } | NodeKind::MethodCall { .. } => vec![root],
            _ => self.node(root).children.clone(),
        };
        while let Some(node) = pending.pop() {
            if matches!(self.kind(node), NodeKind::Stmt(_)) {
                continue;
            }
            if self.rebinding_effect(node) {
                return true;
            }
            pending.extend(self.node(node).children.iter().copied());
        }
        false
    }

    /// Whether `node`, once its operands are evaluated, may rebind a handle:
    /// a call of a user subroutine or an assignment operator.
    fn rebinding_effect(&self, node: NodeId) -> bool {
        match self.kind(node) {
            NodeKind::FuncCall {
                callee: Some(callee),
                ..
            }
            | NodeKind::MethodCall {
                callee: Some(callee),
                ..
            } => matches!(self.kind(*callee), NodeKind::FuncTask { .. }),
            NodeKind::Expr(ExprKind::Operation { op, assignment, .. }) => {
                *assignment || Self::assignment_operator(*op)
            }
            _ => false,
        }
    }

    /// Whether `op` writes its first operand: an assignment, an increment or
    /// a decrement, or an overloaded update.
    pub(in crate::sim::codegen) fn assignment_operator(op: Operation) -> bool {
        matches!(
            op,
            Operation::Assignment
                | Operation::PostIncrement
                | Operation::PreIncrement
                | Operation::PostDecrement
                | Operation::PreDecrement
                | Operation::OverloadUpdate
                | Operation::OverloadPostUpdate
        )
    }

    /// The member paths among `uses` that a rebinding effect of `root`
    /// precedes in evaluation order, each with the node its use starts at:
    /// the outermost select or method call of the member path, or the path
    /// itself. Operands evaluate left to right, an effect after its own
    /// operands, a method's receiver before its arguments; a simple
    /// assignment locates an element target after its right-hand side (SV
    /// 10.4.1, and after its control when it has one), a compound one and a
    /// whole-member target before it. Effects inside a use (in its handle or
    /// its selects) do not precede it.
    fn uses_after_effects(&self, root: NodeId, uses: &[NodeId]) -> Vec<(NodeId, NodeId)> {
        let mut seen: Vec<NodeId> = Vec::new();
        let mut placed = Vec::new();
        let mut effect = false;
        let mut pending = vec![OrderStep::Visit(root)];
        while let Some(step) = pending.pop() {
            let node = match step {
                OrderStep::Effect => {
                    effect = true;
                    continue;
                }
                // The target of an element actual is located again at the
                // copy-out, as any copied-out actual's is (SV 13.5), after
                // the call.
                OrderStep::CopyOut(actual) => {
                    if let Some(path) = self.use_member_path(actual) {
                        if path != actual
                            && uses.contains(&path)
                            && !placed.iter().any(|(placed, _)| *placed == path)
                        {
                            placed.push((path, actual));
                        }
                    }
                    continue;
                }
                OrderStep::Visit(node) => node,
            };
            if node != root && matches!(self.kind(node), NodeKind::Stmt(_)) {
                continue;
            }
            if let Some(path) = self.use_member_path(node) {
                if uses.contains(&path) && !seen.contains(&path) {
                    seen.push(path);
                    if effect {
                        placed.push((path, node));
                    }
                }
            }
            // A whole member passed to a subroutine is bound when the call
            // starts, before its other actuals, as the reference simulator
            // does; the call's own lowering binds it.
            if matches!(
                self.kind(node),
                NodeKind::FuncCall { .. } | NodeKind::MethodCall { .. }
            ) {
                for actual in self.call_argument_nodes(node) {
                    let actual = self.unwrap_output_actual(actual);
                    if self.use_member_path(actual) == Some(actual)
                        && uses.contains(&actual)
                        && !seen.contains(&actual)
                    {
                        seen.push(actual);
                        if effect {
                            placed.push((actual, node));
                        }
                    }
                }
            }
            let steps = self.evaluation_steps(node);
            pending.extend(steps.into_iter().rev());
        }
        placed
    }

    /// The operands of `node` in the order they are evaluated, with the
    /// node's own rebinding effect where it happens.
    fn evaluation_steps(&self, node: NodeId) -> Vec<OrderStep> {
        let children = &self.node(node).children;
        match self.kind(node) {
            NodeKind::Stmt(StmtKind::Assign {
                op,
                delay,
                blocking,
            }) if children.len() >= 2 => {
                let (lhs, rhs) = (children[0], children[1]);
                let rest = visit(&children[2..]);
                let whole = self.use_member_path(lhs) == Some(lhs);
                if *op != Operation::Assignment || whole {
                    [OrderStep::Visit(lhs), OrderStep::Visit(rhs)]
                        .into_iter()
                        .chain(rest)
                        .collect()
                } else {
                    // A nonblocking assignment locates its target at issue.
                    let control = (*blocking && delay.is_some()).then_some(OrderStep::Effect);
                    std::iter::once(OrderStep::Visit(rhs))
                        .chain(rest)
                        .chain(control)
                        .chain([OrderStep::Visit(lhs)])
                        .collect()
                }
            }
            NodeKind::Expr(ExprKind::Operation {
                op,
                assignment,
                operands,
                ..
            }) if (*assignment || Self::assignment_operator(*op)) && !operands.is_empty() => {
                // A simple assignment evaluates its value before its
                // target, a compound one its target first.
                let mut steps: Vec<OrderStep> =
                    if *op == Operation::Assignment && operands.len() == 2 {
                        visit(&[operands[1], operands[0]]).collect()
                    } else {
                        visit(operands).collect()
                    };
                steps.push(OrderStep::Effect);
                steps
            }
            NodeKind::FuncCall { .. } | NodeKind::MethodCall { .. } => self.call_steps(node),
            _ => visit(children).collect(),
        }
    }

    /// The evaluation steps of call `call`: a method's receiver, then the
    /// actuals in order (the target of an output or inout actual, which the
    /// frontend wraps in an assignment from the formal, is read there for
    /// an inout one), then a user subroutine's effect and its copy-out.
    fn call_steps(&self, call: NodeId) -> Vec<OrderStep> {
        let receiver = match self.kind(call) {
            NodeKind::MethodCall { receiver, .. } => *receiver,
            _ => None,
        };
        let copied = if self.rebinding_effect(call) {
            self.copied_out_actuals(call)
        } else {
            Vec::new()
        };
        let mut steps: Vec<OrderStep> = receiver.map(OrderStep::Visit).into_iter().collect();
        for child in &self.node(call).children {
            if Some(*child) == receiver {
                continue;
            }
            steps.push(OrderStep::Visit(if copied.contains(child) {
                self.unwrap_output_actual(*child)
            } else {
                *child
            }));
        }
        if self.rebinding_effect(call) {
            steps.push(OrderStep::Effect);
            steps.extend(
                copied
                    .into_iter()
                    .map(|actual| OrderStep::CopyOut(self.unwrap_output_actual(actual))),
            );
        }
        steps
    }

    /// The member path a use starting at `node` names: `node` itself, a
    /// select of it, a member of its element (`va[k].p[1].y`, a path rooted
    /// at the select), a method called on one of these, or a data query of
    /// one (`$size(va[k].q)`), which its lowering takes whole.
    fn use_member_path(&self, node: NodeId) -> Option<NodeId> {
        let mut current = node;
        loop {
            match self.kind(current) {
                NodeKind::Expr(ExprKind::HierPath { refs, .. }) => {
                    if self.handle_member_path(current) {
                        return Some(current);
                    }
                    match refs.first().copied().flatten() {
                        Some(first) if matches!(self.kind(first), NodeKind::Expr(_)) => {
                            current = first
                        }
                        _ => return None,
                    }
                }
                NodeKind::Expr(ExprKind::MemberSelect { base, .. }) => current = *base,
                NodeKind::SysCall { name } if data_query(name) => {
                    current = *self.node(current).children.first()?
                }
                NodeKind::Expr(
                    ExprKind::ArraySelect { base, .. }
                    | ExprKind::BitSelect { base, .. }
                    | ExprKind::PartSelect { base, .. }
                    | ExprKind::IndexedPartSelect { base, .. },
                ) => {
                    let base = *base;
                    if let Some(path) = self.select_member_path(current, base) {
                        return Some(path);
                    }
                    current = base;
                }
                NodeKind::MethodCall {
                    receiver: Some(receiver),
                    ..
                } => current = *receiver,
                _ => return None,
            }
        }
    }

    /// The located rejection of `node`, a member path through a handle or a
    /// select of one, when it names an unpacked member that no statement or
    /// expression binds a view of here.
    pub(in super::super) fn unbound_interface_member(
        &self,
        path: &str,
        node: NodeId,
    ) -> Option<String> {
        if !self.interface_views_possible {
            return None;
        }
        let mut current = node;
        let member_path = loop {
            match self.kind(current) {
                NodeKind::Expr(ExprKind::HierPath { .. }) => break current,
                NodeKind::Expr(
                    ExprKind::ArraySelect { base, .. }
                    | ExprKind::BitSelect { base, .. }
                    | ExprKind::PartSelect { base, .. }
                    | ExprKind::IndexedPartSelect { base, .. },
                ) => {
                    let base = *base;
                    if let Some(member_path) = self.select_member_path(current, base) {
                        break member_path;
                    }
                    current = base;
                }
                _ => return None,
            }
        };
        if self.staged_interface_view(member_path).is_some() {
            return None;
        }
        let target = self
            .virtual_interface_member_path(path, member_path, false)
            .ok()
            .flatten()?;
        if self
            .virtual_interface_members
            .contains_key(&(target.descriptor, target.member.clone()))
            || !self.unpacked_interface_member(target.descriptor, &target.member)
        {
            return None;
        }
        Some(self.locate_error(
            node,
            format!(
                "unpacked member `{}` of virtual interface `{}` is not supported here in `{path}`",
                target.member, target.spelling
            ),
        ))
    }

    /// Whether a staged view stands for a member path in `node`.
    pub(in crate::sim::codegen) fn names_interface_view(&self, node: NodeId) -> bool {
        if self.staged_interface_views.is_empty() {
            return false;
        }
        let mut pending = vec![node];
        while let Some(node) = pending.pop() {
            if self.staged_interface_views.contains_key(&node) {
                return true;
            }
            pending.extend(self.node(node).children.iter().copied());
        }
        false
    }

    /// The member paths below `node` that name a member through a handle and
    /// that no staging binds yet, in node order. Nested statements stage
    /// their own, and so does the `with` clause of an array method, which
    /// runs in a callback of its own.
    fn collect_interface_view_uses(&self, node: NodeId, out: &mut Vec<NodeId>) {
        if self.names_member_through_handle(node) {
            out.push(node);
        }
        if let NodeKind::MethodCall {
            receiver: Some(receiver),
            ..
        } = self.kind(node)
        {
            if self.db.method_call_has_with_clause(node) {
                return self.collect_interface_view_uses(*receiver, out);
            }
        }
        for child in &self.node(node).children {
            if !matches!(self.kind(*child), NodeKind::Stmt(_)) {
                self.collect_interface_view_uses(*child, out);
            }
        }
    }

    /// The member paths through handles that `root` writes: assignment
    /// targets, output, inout and `ref` actuals, and the paths below their
    /// selects.
    fn interface_view_writes(&self, root: NodeId, out: &mut Vec<NodeId>) {
        let mut targets = Vec::new();
        self.written_targets(root, &mut targets);
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            if node != root {
                self.written_targets(node, &mut targets);
            }
            for child in &self.node(node).children {
                if !matches!(self.kind(*child), NodeKind::Stmt(_)) {
                    pending.push(*child);
                }
            }
        }
        for target in targets {
            let mut paths = Vec::new();
            self.handle_member_writes(target, &mut paths);
            out.extend(paths);
            let mut current = target;
            // `v.a[i]`: the select names the declaration; its member path
            // is the structural operand.
            while let NodeKind::Expr(
                ExprKind::ArraySelect { base, .. }
                | ExprKind::BitSelect { base, .. }
                | ExprKind::PartSelect { base, .. }
                | ExprKind::IndexedPartSelect { base, .. },
            ) = self.kind(current)
            {
                let base = *base;
                if let Some(path) = self.select_member_path(current, base) {
                    out.push(path);
                    break;
                }
                current = base;
            }
        }
    }

    /// The member path and descriptor storage slot of `node` when it names
    /// an unpacked member through a handle.
    fn interface_view_path(
        &mut self,
        path: &str,
        node: NodeId,
        write: bool,
    ) -> Result<Option<(VirtualInterfaceMemberPath, usize)>, String> {
        let Some(target) = self
            .virtual_interface_member_path(path, node, write)
            .map_err(|error| self.locate_error(node, error))?
        else {
            return Ok(None);
        };
        if self
            .virtual_interface_members
            .contains_key(&(target.descriptor, target.member.clone()))
        {
            return Ok(None);
        }
        let Some(slot) = self.virtual_interface_storage_slot(path, node, &target)? else {
            return Ok(None);
        };
        Ok(Some((target, slot)))
    }

    /// Whether member paths `a` and `b` name one member through one handle
    /// path made of declarations (`v.a`, `c.v.a`), so one view serves both.
    fn same_member_path(&self, a: NodeId, b: NodeId) -> bool {
        match (self.kind(a), self.kind(b)) {
            (
                NodeKind::Expr(ExprKind::HierPath {
                    parts: left_parts,
                    refs: left,
                }),
                NodeKind::Expr(ExprKind::HierPath {
                    parts: right_parts,
                    refs: right,
                }),
            ) => {
                left_parts == right_parts
                    && left == right
                    && left.iter().all(|target| {
                        target.is_some_and(|target| {
                            matches!(
                                self.kind(target),
                                NodeKind::Var { .. }
                                    | NodeKind::FuncArg { .. }
                                    | NodeKind::Array { .. }
                                    | NodeKind::ModuleInst { .. }
                                    | NodeKind::ModPort
                            )
                        })
                    })
            }
            _ => false,
        }
    }

    /// The storage slot of unpacked member `target.member` of its
    /// descriptor, added with each bound instance's storage on first use;
    /// `None` when the member is not array or container storage in every
    /// instance the handle can bind. Instances whose storage differs in
    /// shape (a parameterized interface) are a located error.
    fn virtual_interface_storage_slot(
        &mut self,
        path: &str,
        node: NodeId,
        target: &VirtualInterfaceMemberPath,
    ) -> Result<Option<usize>, String> {
        let descriptor = target.descriptor;
        if let Some(slot) = self.model.virtual_interfaces[descriptor]
            .storage
            .iter()
            .position(|name| *name == target.member)
        {
            return Ok(Some(slot));
        }
        let Some(storage) =
            self.virtual_interface_declarations(descriptor, &target.member, |child| {
                self.array_globals
                    .get(&child)
                    .map(|array| IrInterfaceStorage::Array(array.ir))
                    .or_else(|| {
                        self.container_globals
                            .get(&child)
                            .map(|container| IrInterfaceStorage::Container(container.ir))
                    })
            })
        else {
            return Ok(None);
        };
        let same = |first: IrInterfaceStorage, other: IrInterfaceStorage| match (first, other) {
            (IrInterfaceStorage::Array(a), IrInterfaceStorage::Array(b)) => {
                let (a, b) = (&self.model.arrays[a], &self.model.arrays[b]);
                a.elem_width == b.elem_width
                    && a.signed == b.signed
                    && a.two_state == b.two_state
                    && a.real == b.real
                    && a.shortreal == b.shortreal
                    && a.dims == b.dims
                    && a.descriptor == b.descriptor
                    && a.sparse() == b.sparse()
            }
            (IrInterfaceStorage::Container(a), IrInterfaceStorage::Container(b)) => {
                let (a, b) = (&self.model.containers[a], &self.model.containers[b]);
                a.element == b.element && a.kind == b.kind
            }
            _ => false,
        };
        if storage.iter().any(|other| !same(storage[0], *other)) {
            return Err(self.locate_error(
                node,
                format!(
                    "unpacked member `{}` has a different shape in the instances virtual \
                     interface `{}` can bind, in `{path}`",
                    target.member, target.spelling
                ),
            ));
        }
        let interface = &mut self.model.virtual_interfaces[descriptor];
        interface.storage.push(target.member.clone());
        for (instance, storage) in interface.instances.iter_mut().zip(storage) {
            instance.storage.push(Some(storage));
        }
        Ok(Some(interface.storage.len() - 1))
    }

    /// A new view of storage slot `slot` and the statement that binds it to
    /// the instance the handle of `target` binds.
    fn new_interface_view(
        &mut self,
        path: &str,
        node: NodeId,
        target: &VirtualInterfaceMemberPath,
        slot: usize,
        handle: IrChandleExpr,
        refresh: bool,
    ) -> Result<(InterfaceView, IrStmt), String> {
        let descriptor = target.descriptor;
        let source = self.model.virtual_interfaces[descriptor]
            .instances
            .iter()
            .find_map(|instance| instance.storage.get(slot).copied().flatten())
            .ok_or_else(|| {
                format!(
                    "unpacked member `{}` of virtual interface `{}` has no storage",
                    target.member, target.spelling
                )
            })?;
        let (storage, array) = match source {
            IrInterfaceStorage::Array(source) => {
                let ir = self.model.arrays.len();
                let mut view = self.model.arrays[source].clone();
                view.c_name = format!("V_llg_view_{ir}");
                view.bound = true;
                view.activation = false;
                view.net = None;
                view.net_elements.clear();
                view.sampled_image = false;
                view.initial_cells = None;
                let info = ArrayInfo {
                    global: view.c_name.clone(),
                    elem_width: view.elem_width,
                    signed: view.signed,
                    real: view.real,
                    shortreal: view.shortreal,
                    is_net: false,
                    dims: view.dims.clone(),
                    init: None,
                    ir,
                };
                self.model.arrays.push(view);
                (IrInterfaceStorage::Array(ir), Some(info))
            }
            IrInterfaceStorage::Container(source) => {
                let ir = self.model.containers.len();
                let mut view = self.model.containers[source].clone();
                view.c_name = format!("S_llg_container_{ir}");
                view.activation = true;
                view.class_field = None;
                view.receiver = None;
                self.model.containers.push(view);
                self.container_types_like.insert(ir, source);
                if let Some(range) = self.fixed_view_ranges.get(&source).copied() {
                    self.fixed_view_ranges.insert(ir, range);
                }
                if let Some(dimensions) = self.fixed_view_dims.get(&source).cloned() {
                    self.fixed_view_dims.insert(ir, dimensions);
                }
                (IrInterfaceStorage::Container(ir), None)
            }
        };
        let site = self.required_site("virtual interface member access", path, node);
        let declaration = match self.kind(node) {
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => refs.last().copied().flatten(),
            _ => None,
        };
        Ok((
            InterfaceView {
                storage,
                array,
                declaration,
            },
            IrStmt::InterfaceBind(Box::new(IrInterfaceBinding {
                view: storage,
                handle,
                interface: descriptor,
                slot,
                site,
                refresh,
            })),
        ))
    }
}
