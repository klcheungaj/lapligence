//! Virtual interfaces.

use super::*;

impl Codegen<'_> {
    /// The handle, member name and descriptor of a member path rooted at a
    /// virtual-interface handle, after the view direction check.
    pub(in super::super) fn virtual_interface_member_path(
        &self,
        path: &str,
        node: NodeId,
        write: bool,
    ) -> Result<Option<VirtualInterfaceMemberPath>, String> {
        let NodeKind::Expr(ExprKind::HierPath { parts, refs }) = self.kind(node) else {
            return Ok(None);
        };
        let Some((position, handle)) = refs.iter().enumerate().find_map(|(position, target)| {
            let target = (*target)?;
            self.virtual_interface_spelling(target)
                .map(|_| (position, target))
        }) else {
            return Ok(None);
        };
        if let Some(scope) = self.virtual_interface_generate_scope(parts, refs, position) {
            return Err(Self::virtual_interface_generate_scope_error(scope, path));
        }
        let steps = self.virtual_interface_nested_steps(parts, refs, position);
        let member = self.virtual_interface_member_name(node, parts, refs, position + steps.len());
        if member.is_empty() {
            return Ok(None);
        }
        let spelling = self
            .virtual_interface_spelling(handle)
            .ok_or_else(|| format!("virtual interface handle has no type in `{path}`"))?;
        let (descriptor, nested) = self.virtual_interface_nested_path(path, &spelling, &steps)?;
        let mut view_port = false;
        if let Some(view) = Codegen::virtual_interface_view_from_spelling(&spelling) {
            let direction = self
                .virtual_interface_views
                .get(&(descriptor, view.clone()))
                .and_then(|members| members.get(&member))
                .copied();
            let Some(direction) = direction else {
                return Err(format!(
                    "member `{member}` is not available through virtual interface view `{view}` in `{path}`"
                ));
            };
            view_port = true;
            if write && direction == DbDirection::Input {
                return Err(format!(
                    "input modport member `{member}` cannot be written through view `{view}` in `{path}`"
                ));
            }
        }
        Ok(Some(VirtualInterfaceMemberPath {
            position,
            handle,
            member,
            descriptor,
            nested,
            spelling,
            view_port,
        }))
    }

    /// The member a member path `node` rooted at a virtual-interface handle
    /// names after its component `last_scope` (the handle, or the last
    /// nested interface instance): `x`, or `cb.x` for a clockvar. Empty when
    /// the path ends at that component.
    pub(in super::super) fn virtual_interface_member_name(
        &self,
        node: NodeId,
        parts: &[String],
        refs: &[Option<NodeId>],
        last_scope: usize,
    ) -> String {
        let mut member = parts
            .iter()
            .skip(last_scope + 1)
            .filter(|part| !part.is_empty())
            .cloned()
            .collect::<Vec<_>>()
            .join(".");
        if let Some(clock_target) = self.db.resolve_clocking_member(node) {
            if let Some(clock_var) = self.db.clocking_var(clock_target) {
                member = format!(
                    "{}.{}",
                    self.node(clock_var.block).name,
                    self.node(clock_target).name
                );
            }
        }
        // Slang omits the clocking-block segment from a virtual-interface
        // member path (`vif.cb.data`) and binds the final reference directly
        // to the clocking variable. Recover the declaration-owned block name
        // so the runtime handle selects sampled storage, not the raw signal.
        if let Some(clock_var) = refs
            .iter()
            .rev()
            .skip_while(|target| target.is_none_or(|target| !self.db.is_clocking_var(target)))
            .flatten()
            .next()
        {
            if let Some(block) = self.db.clocking_var(*clock_var).map(|info| info.block) {
                member = format!("{}.{}", self.node(block).name, self.node(*clock_var).name);
            }
        }
        member
    }

    /// The descriptor reached from a handle of type `spelling` through the
    /// nested interface instances `steps` (`v.a.l`), and the `(descriptor,
    /// nested slot)` of each step. A view restricts the handle to the
    /// modport's ports (SV 25.5), which name no nested instance.
    pub(in super::super) fn virtual_interface_nested_path(
        &self,
        path: &str,
        spelling: &str,
        steps: &[String],
    ) -> Result<(usize, Vec<(usize, usize)>), String> {
        let identity = Codegen::normalize_virtual_interface_identity(
            &Codegen::virtual_interface_identity_from_spelling(spelling),
        );
        let mut descriptor = self
            .virtual_interface_types
            .get(&identity)
            .copied()
            .ok_or_else(|| format!("virtual interface type `{identity}` has no descriptor"))?;
        let context = if path.is_empty() {
            String::new()
        } else {
            format!(" in `{path}`")
        };
        if let (Some(view), Some(step)) = (
            Codegen::virtual_interface_view_from_spelling(spelling),
            steps.first(),
        ) {
            return Err(format!(
                "nested interface instance `{step}` is not available through virtual interface view `{view}`{context}"
            ));
        }
        let mut nested = Vec::with_capacity(steps.len());
        for step in steps {
            let slot = self
                .virtual_interface_nested
                .get(&(descriptor, step.clone()))
                .copied()
                .ok_or_else(|| {
                    format!(
                        "nested interface instance `{step}` through virtual interface `{spelling}` \
                         is not declared, as an instance of one interface type, in every \
                         elaborated instance the handle can bind{context}"
                    )
                })?;
            nested.push((descriptor, slot));
            descriptor = self.model.virtual_interfaces[descriptor].nested[slot].interface;
        }
        Ok((descriptor, nested))
    }

    fn virtual_interface_generate_scope_error(scope: &str, path: &str) -> String {
        format!(
            "generate block `{scope}` of an interface is not supported through a virtual \
             interface in `{path}`; select the interface's own members or the interface \
             instances declared in it directly"
        )
    }

    /// Lower a nested interface instance selected through a handle as a
    /// value (`vif = top.tx`, an actual argument `top.rx`; SV 25.9): the
    /// handle of the nested instance of the instance the handle binds.
    pub(in super::super) fn virtual_interface_nested_value(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrChandleExpr>, String> {
        let NodeKind::Expr(ExprKind::HierPath { parts, refs }) = self.kind(node) else {
            return Ok(None);
        };
        let Some((position, handle)) = refs.iter().enumerate().find_map(|(position, target)| {
            let target = (*target)?;
            self.virtual_interface_spelling(target)
                .map(|_| (position, target))
        }) else {
            return Ok(None);
        };
        if let Some(scope) = self.virtual_interface_generate_scope(parts, refs, position) {
            return Err(Self::virtual_interface_generate_scope_error(scope, path));
        }
        let steps = self.virtual_interface_nested_steps(parts, refs, position);
        if steps.is_empty() || position + 1 + steps.len() != parts.len() {
            return Ok(None);
        }
        let spelling = self
            .virtual_interface_spelling(handle)
            .ok_or_else(|| format!("virtual interface handle has no type in `{path}`"))?;
        let (_, nested) = self
            .virtual_interface_nested_path(path, &spelling, &steps)
            .map_err(|error| self.locate_error(node, error))?;
        let target = VirtualInterfaceMemberPath {
            position,
            handle,
            member: String::new(),
            descriptor: 0,
            nested,
            spelling,
            view_port: false,
        };
        self.virtual_interface_handle(path, node, &target).map(Some)
    }

    /// The lowered handle of a member path: a handle held in a record or
    /// class member (`r.v.x`) is the value of the member access expression,
    /// not of its declaration.
    pub(super) fn virtual_interface_handle(
        &mut self,
        path: &str,
        node: NodeId,
        target: &VirtualInterfaceMemberPath,
    ) -> Result<IrChandleExpr, String> {
        let handle = if target.position > 0 {
            self.member_prefix_expression(node, target.position + 1)
                .unwrap_or(target.handle)
        } else {
            target.handle
        };
        let handle = self.lower_chandle(path, handle)?;
        Ok(self.wrap_virtual_interface_nested(path, node, handle, &target.nested))
    }

    /// The handle of the nested instance `nested` selects from the
    /// instance `handle` binds, one lookup per step.
    fn wrap_virtual_interface_nested(
        &self,
        path: &str,
        node: NodeId,
        mut handle: IrChandleExpr,
        nested: &[(usize, usize)],
    ) -> IrChandleExpr {
        for (interface, slot) in nested {
            let name = &self.model.virtual_interfaces[*interface].nested[*slot].name;
            let site = self.required_site(
                &format!("virtual interface nested instance `{name}`"),
                path,
                node,
            );
            handle = IrChandleExpr::VirtualInterfaceNested {
                handle: Box::new(handle),
                interface: *interface,
                slot: *slot,
                site,
            };
        }
        handle
    }

    /// The receiver of a method call through a virtual interface: the
    /// handle, or the nested instance whose method the call names
    /// (`v.tx.f()`).
    pub(in super::super) fn lower_virtual_interface_receiver(
        &mut self,
        path: &str,
        call: NodeId,
        receiver: NodeId,
    ) -> Result<IrChandleExpr, String> {
        let handle = self.lower_chandle(path, receiver)?;
        let steps = self.db.virtual_interface_call_scopes(call).to_vec();
        if steps.is_empty() {
            return Ok(handle);
        }
        let spelling = self
            .virtual_interface_spelling(receiver)
            .ok_or_else(|| format!("virtual interface receiver has no type in `{path}`"))?;
        let (_, nested) = self.virtual_interface_nested_path(path, &spelling, &steps)?;
        Ok(self.wrap_virtual_interface_nested(path, call, handle, &nested))
    }

    pub(super) fn virtual_interface_access(
        &mut self,
        path: &str,
        node: NodeId,
        write: bool,
    ) -> Result<Option<VirtualInterfaceAccess>, String> {
        // An unpacked member lowers against its bound view.
        if self.staged_interface_view(node).is_some() {
            return Ok(None);
        }
        let Some(target) = self.virtual_interface_member_path(path, node, write)? else {
            return Ok(None);
        };
        let descriptor = target.descriptor;
        let member = target.member.clone();
        let spelling = target.spelling.clone();
        let view_port = target.view_port;
        let slot = self
            .virtual_interface_members
            .get(&(descriptor, member.clone()))
            .copied()
            .ok_or_else(|| {
                // A clocking block has no member storage of its own, only
                // its clockvars (`cb.x`); its event is reached by an event
                // control alone (`virtual_event_ref`).
                let prefix = format!("{member}.");
                if self
                    .virtual_interface_members
                    .keys()
                    .any(|(owner, name)| *owner == descriptor && name.starts_with(&prefix))
                {
                    format!(
                        "clocking block `{member}` is not supported as a value through virtual interface `{spelling}` in `{path}`; wait on it with an event control or read its clockvars"
                    )
                } else if self.unpacked_interface_member(descriptor, &member) {
                    // Statements and expressions bind views of such members
                    // (`stage_interface_views`); this use is in none.
                    self.locate_error(
                        node,
                        format!(
                            "unpacked member `{member}` of virtual interface `{spelling}` is \
                             not supported here in `{path}`"
                        ),
                    )
                } else if self
                    .virtual_interface_unvalued
                    .contains(&(descriptor, member.clone()))
                {
                    format!(
                        "member `{member}` of virtual interface `{spelling}` is a string or a handle, which is not supported through a virtual interface in `{path}`"
                    )
                } else if view_port {
                    // A view port without interface storage of its own is a
                    // modport expression port (SV 25.5.4); its expression is
                    // not evaluated per bound instance at run time.
                    format!(
                        "modport expression port `{member}` is not supported through virtual interface view `{spelling}` in `{path}`"
                    )
                } else {
                    format!(
                        "member `{member}` is not available through virtual interface view `{spelling}` in `{path}`"
                    )
                }
            })?;
        let metadata = self
            .model
            .virtual_interfaces
            .get(descriptor)
            .and_then(|interface| interface.members.get(slot))
            .ok_or_else(|| format!("virtual interface member `{member}` has no metadata"))?;
        let width = metadata.width;
        let signed = metadata.signed;
        let two_state = metadata.two_state;
        let shortreal = metadata.shortreal;
        let handle = self.virtual_interface_handle(path, node, &target)?;
        Ok(Some((
            handle, descriptor, slot, width, signed, two_state, shortreal,
        )))
    }

    /// The nested member access of `node` whose path has `length` components.
    /// The frontend nests each member access in the next one.
    pub(in super::super) fn member_prefix_expression(
        &self,
        node: NodeId,
        length: usize,
    ) -> Option<NodeId> {
        let mut current = node;
        loop {
            let [child] = self.node(current).children.as_slice() else {
                return None;
            };
            match self.kind(*child) {
                NodeKind::Expr(ExprKind::HierPath { refs, .. }) if refs.len() > length => {
                    current = *child;
                }
                NodeKind::Expr(ExprKind::HierPath { refs, .. }) if refs.len() == length => {
                    return Some(*child);
                }
                _ => return (length == 1).then_some(*child),
            }
        }
    }

    /// The handle declaration and member name of a member path rooted at a
    /// virtual-interface handle (`v.x`, `v.cb.x`), when `node` is one.
    pub(in super::super) fn virtual_interface_member_key(
        &self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<(NodeId, String)>, String> {
        Ok(self
            .virtual_interface_member_path(path, node, false)?
            .map(|target| (target.handle, self.virtual_interface_key_member(&target))))
    }

    /// The member path after the handle, nested instances included
    /// (`tx.x`), which with the handle identifies the selected storage in
    /// the source text.
    fn virtual_interface_key_member(&self, target: &VirtualInterfaceMemberPath) -> String {
        let mut key = String::new();
        for (interface, slot) in &target.nested {
            key.push_str(&self.model.virtual_interfaces[*interface].nested[*slot].name);
            key.push('.');
        }
        key.push_str(&target.member);
        key
    }

    /// Lower a member path rooted at a virtual-interface handle. The runtime
    /// receives the handle on every access, so assigning a new handle changes
    /// the target of all subsequent reads and writes.
    pub(in super::super) fn virtual_interface_member_expr(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrExpr>, String> {
        let Some((handle, descriptor, slot, width, signed, _two_state, _shortreal)) =
            self.virtual_interface_access(path, node, false)?
        else {
            return Ok(None);
        };
        let site = self.required_site("virtual interface member access", path, node);
        let name = self.native_access_symbol_at(
            handle,
            crate::sim::ir::IrNativeAccessKind::InterfaceMember {
                interface: descriptor,
                member: slot,
            },
            Some(site),
        );
        Ok(Some(IrExpr::new(
            IrExprKind::LocalRead(name),
            width,
            signed,
            None,
        )))
    }

    pub(in super::super) fn virtual_interface_member_lhs(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrLhs>, String> {
        let Some((handle, descriptor, slot, width, signed, two_state, shortreal)) =
            self.virtual_interface_access(path, node, true)?
        else {
            return Ok(None);
        };
        let site = self.required_site("virtual interface member access", path, node);
        let name = self.native_access_symbol_at(
            handle,
            crate::sim::ir::IrNativeAccessKind::InterfaceMember {
                interface: descriptor,
                member: slot,
            },
            Some(site),
        );
        Ok(Some(IrLhs::WholeRef {
            addr: format!("&{name}"),
            width,
            signed,
            two_state,
            shortreal,
        }))
    }
}

/// A member path rooted at a virtual-interface handle (`v.x`, `r.v.x`).
pub(in super::super) struct VirtualInterfaceMemberPath {
    /// Position of the handle among the path's components.
    pub(in super::super) position: usize,
    /// The handle declaration.
    pub(in super::super) handle: NodeId,
    /// The member name after the handle and its nested instances (`cb.x`
    /// for a clockvar).
    pub(in super::super) member: String,
    /// The descriptor of the instance that declares the member: the
    /// handle's, or that of the last nested instance.
    pub(in super::super) descriptor: usize,
    /// `(descriptor, nested slot)` of each nested interface instance between
    /// the handle and the member (`tx` in `v.tx.x`), outermost first.
    nested: Vec<(usize, usize)>,
    pub(in super::super) spelling: String,
    view_port: bool,
}

/// A fixed event array, or an element or row of one, reached through a
/// virtual-interface handle.
struct VirtualEventArray {
    member_path: NodeId,
    target: VirtualInterfaceMemberPath,
    /// The array's declaration in each instance, by instance index.
    arrays: Vec<NodeId>,
    indices: Vec<NodeId>,
}

impl Codegen<'_> {
    /// What `declared` gives for member `member` in every instance bound to
    /// descriptor `descriptor`, by instance index, or `None` when it gives
    /// nothing for one of them.
    pub(super) fn virtual_interface_declarations<T>(
        &self,
        descriptor: usize,
        member: &str,
        declared: impl Fn(NodeId) -> Option<T>,
    ) -> Option<Vec<T>> {
        let mut bound = self
            .virtual_interface_instances
            .iter()
            .filter(|(_, (owner, _))| *owner == descriptor)
            .map(|(interface, (_, instance))| (*instance, *interface))
            .collect::<Vec<_>>();
        bound.sort_unstable_by_key(|(instance, _)| *instance);
        if bound.is_empty()
            || bound
                .iter()
                .enumerate()
                .any(|(position, (instance, _))| position != *instance)
        {
            return None;
        }
        bound
            .into_iter()
            .map(|(_, interface)| {
                self.node(interface)
                    .children
                    .iter()
                    .copied()
                    .filter(|child| self.node(*child).name == member)
                    .find_map(&declared)
            })
            .collect()
    }

    /// Whether member `member` of descriptor `descriptor` is array or
    /// container storage in the instances it can bind.
    pub(in super::super) fn unpacked_interface_member(
        &self,
        descriptor: usize,
        member: &str,
    ) -> bool {
        self.virtual_interface_declarations(descriptor, member, |child| {
            (self.array_globals.contains_key(&child) || self.container_globals.contains_key(&child))
                .then_some(())
        })
        .is_some()
    }

    /// The member path (`v.mem`) below an element select whose base the
    /// frontend names as `base`: the path itself, or the structural operand
    /// of the innermost select when `base` is the member declaration.
    pub(in super::super) fn select_member_path(
        &self,
        node: NodeId,
        base: NodeId,
    ) -> Option<NodeId> {
        if matches!(self.kind(base), NodeKind::Expr(ExprKind::HierPath { .. })) {
            return Some(base);
        }
        let mut current = node;
        loop {
            let next = self.node(current).children.iter().copied().find(|child| {
                match self.kind(*child) {
                    NodeKind::Expr(ExprKind::HierPath { refs, .. }) => {
                        refs.last().copied().flatten() == Some(base)
                    }
                    NodeKind::Expr(
                        ExprKind::ArraySelect { base: inner, .. }
                        | ExprKind::BitSelect { base: inner, .. },
                    ) => *inner == base,
                    _ => false,
                }
            })?;
            if matches!(self.kind(next), NodeKind::Expr(ExprKind::HierPath { .. })) {
                return Some(next);
            }
            current = next;
        }
    }
}

impl Codegen<'_> {
    /// The member path of `node` when it names a clocking block or a scalar
    /// named event through a virtual-interface handle or view (`vif.cb`,
    /// `vif.e`, SV 14.13, 15.5, 25.9), with that declaration in an interface
    /// instance the descriptor can bind.
    pub(in super::super) fn virtual_event_member(
        &self,
        node: NodeId,
    ) -> Option<(VirtualInterfaceMemberPath, NodeId)> {
        let target = self
            .virtual_interface_member_path("", node, false)
            .ok()
            .flatten()?;
        let event = self
            .virtual_interface_instances
            .iter()
            .filter(|(_, (descriptor, _))| *descriptor == target.descriptor)
            .find_map(|(interface, _)| self.virtual_interface_event(*interface, &target.member))?;
        Some((target, event))
    }

    fn virtual_interface_event(&self, interface: NodeId, name: &str) -> Option<NodeId> {
        self.node(interface).children.iter().copied().find(|child| {
            self.node(*child).name == name
                && (self.db.is_clocking_block(*child)
                    || matches!(self.kind(*child), NodeKind::NamedEvent)
                        && self.db.event_array_meta(*child).is_none())
        })
    }

    /// The fixed event array `node` names or selects from through a
    /// virtual-interface handle (`v.ea`, `v.ea[i]`).
    fn virtual_event_array(&self, node: NodeId) -> Option<VirtualEventArray> {
        let (member_path, indices) = match self.kind(node) {
            NodeKind::Expr(ExprKind::HierPath { .. }) => (node, Vec::new()),
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                (self.select_member_path(node, *base)?, indices.clone())
            }
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                (self.select_member_path(node, *base)?, vec![*index])
            }
            _ => return None,
        };
        let target = self
            .virtual_interface_member_path("", member_path, false)
            .ok()
            .flatten()?;
        let arrays =
            self.virtual_interface_declarations(target.descriptor, &target.member, |child| {
                self.event_arrays.contains_key(&child).then_some(child)
            })?;
        Some(VirtualEventArray {
            member_path,
            target,
            arrays,
            indices,
        })
    }

    /// Whether `node` names an event, or a fixed event array or an element
    /// of one, through a virtual-interface handle.
    pub(in super::super) fn is_virtual_event(&self, node: NodeId) -> bool {
        self.virtual_event_member(node).is_some() || self.virtual_event_array(node).is_some()
    }

    /// For each instance that the handle of `array` can bind: whether it is
    /// the bound one, and the event or events `array` names there.
    fn virtual_event_arms(
        &mut self,
        path: &str,
        node: NodeId,
        array: &VirtualEventArray,
    ) -> Result<Vec<(IrExpr, EventTarget)>, String> {
        let handle = self.virtual_interface_handle(path, array.member_path, &array.target)?;
        let site = self.required_site("virtual interface event", path, node);
        Ok(array
            .arrays
            .iter()
            .enumerate()
            .map(|(instance, declaration)| {
                let bound = IrExpr::new(
                    IrExprKind::ObjectQuery(Box::new(IrObjectQuery::VirtualInterfaceInstance {
                        handle: handle.clone(),
                        interface: array.target.descriptor,
                        site: site.clone(),
                    })),
                    32,
                    false,
                    None,
                );
                let selected = IrExpr::new(
                    IrExprKind::Bin {
                        op: IrBinOp::Eq,
                        a: Box::new(bound),
                        b: Box::new(lhs_integer_expr(instance as i128)),
                    },
                    1,
                    false,
                    None,
                );
                let target = EventTarget {
                    declaration: *declaration,
                    indices: array.indices.clone(),
                    access: None,
                };
                (selected, target)
            })
            .collect())
    }

    /// The assignment of `source` to the fixed event array, or element or
    /// row of one, that `node` names through a virtual interface, in the
    /// instance bound when it executes; `None` when `node` names none.
    pub(in super::super) fn virtual_event_assignment(
        &mut self,
        path: &str,
        node: NodeId,
        source: NodeId,
    ) -> Result<Option<IrStmt>, String> {
        if let Some(array) = self.virtual_event_array(node) {
            self.virtual_interface_member_path(path, array.member_path, true)?;
            let mut statement = IrStmt::Nop;
            for (selected, target) in self
                .virtual_event_arms(path, node, &array)?
                .into_iter()
                .rev()
            {
                let assignment = if self.event_array_elements(path, &target)?.is_some() {
                    self.event_array_assignment(path, &target, source)?
                } else {
                    IrStmt::EventAssign {
                        target: self.event_ref_of(&target, path)?,
                        source: self.lower_event_source(path, source)?,
                    }
                };
                statement = IrStmt::If {
                    cond: selected,
                    then_: vec![assignment],
                    els: Some(vec![statement]),
                    check: crate::sim::ir::IrUniquePriorityCheck::None,
                };
            }
            return Ok(Some(statement));
        }
        Ok(None)
    }

    /// The handle storage of the event `node` names through a virtual
    /// interface, for an assignment or an output, inout or `ref` actual;
    /// `None` when `node` names none or an event array.
    pub(in super::super) fn virtual_event_slot(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<String>, String> {
        if self.virtual_event_array(node).is_some() {
            return Ok(None);
        }
        let Some(IrEventRef::Handle(handle)) = self.virtual_event_ref(path, node)? else {
            return Ok(None);
        };
        self.virtual_interface_member_path(path, node, true)?;
        match *handle {
            IrChandleExpr::LocalRead(slot) => Ok(Some(slot)),
            _ => Ok(None),
        }
    }

    /// The event `node` names through a virtual interface: the handle selects
    /// the descriptor's event slot of the bound instance where the operation
    /// is issued.
    pub(in super::super) fn virtual_event_ref(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrEventRef>, String> {
        if let Some(array) = self.virtual_event_array(node) {
            let mut event = IrChandleExpr::Null;
            for (selected, target) in self
                .virtual_event_arms(path, node, &array)?
                .into_iter()
                .rev()
            {
                let element = self.event_ref_of(&target, path)?;
                event = IrChandleExpr::Conditional {
                    predicate: Box::new(selected),
                    then: Box::new(IrChandleExpr::EventObject(Box::new(element))),
                    otherwise: Box::new(event),
                };
            }
            return Ok(Some(IrEventRef::Handle(Box::new(event))));
        }
        let Some((target, event)) = self.virtual_event_member(node) else {
            return Ok(None);
        };
        let slot = self.virtual_interface_event_slot(target.descriptor, &target.member)?;
        let handle = self.virtual_interface_handle(path, node, &target)?;
        let kind = if self.db.is_clocking_block(event) {
            "virtual interface clocking block event"
        } else {
            "virtual interface event"
        };
        let site = self.required_site(kind, path, node);
        let name = self.native_access_symbol_at(
            handle,
            crate::sim::ir::IrNativeAccessKind::InterfaceEvent {
                interface: target.descriptor,
                event: slot,
            },
            Some(site),
        );
        Ok(Some(IrEventRef::Handle(Box::new(
            IrChandleExpr::LocalRead(name),
        ))))
    }

    /// The descriptor event slot of the event `name`, added with each bound
    /// instance's event on first use.
    fn virtual_interface_event_slot(
        &mut self,
        descriptor: usize,
        name: &str,
    ) -> Result<usize, String> {
        let interface = self
            .model
            .virtual_interfaces
            .get(descriptor)
            .ok_or("virtual interface descriptor is out of bounds")?;
        if let Some(slot) = interface.events.iter().position(|event| event == name) {
            return Ok(slot);
        }
        let mut events = vec![None; interface.instances.len()];
        for (node, (owner, instance)) in &self.virtual_interface_instances {
            if *owner != descriptor {
                continue;
            }
            let event = self
                .virtual_interface_event(*node, name)
                .and_then(|event| self.event_globals.get(&event))
                .map(|event| event.ir);
            if let Some(entry) = events.get_mut(*instance) {
                *entry = event;
            }
        }
        let interface = &mut self.model.virtual_interfaces[descriptor];
        interface.events.push(name.to_owned());
        for (instance, event) in interface.instances.iter_mut().zip(events) {
            instance.events.push(event);
        }
        Ok(interface.events.len() - 1)
    }
}

/// A synchronous drive target reached through a virtual-interface handle
/// (`v.cb.x <= e`, `v.cb.x[i] <= e`; SV 14.16, 25.9).
pub(in super::super) struct VirtualClockingTarget {
    pub(in super::super) handle: IrChandleExpr,
    pub(in super::super) descriptor: usize,
    pub(in super::super) site: String,
    /// The clockvar node addressed by the source text; it carries the
    /// clockvar's declared type and direction.
    pub(in super::super) clockvar: NodeId,
    /// The same clockvar in each bound instance, by descriptor instance index.
    pub(in super::super) instances: Vec<NodeId>,
    /// The selects of `clockvar select` (SV 14.16), in source order.
    pub(in super::super) selects: Vec<super::super::collection::Select>,
    pub(in super::super) width: u32,
    pub(in super::super) two_state: bool,
}

impl Codegen<'_> {
    /// Resolve a synchronous drive target rooted at a virtual-interface
    /// handle. The handle is lowered once; the clockvar of every concrete
    /// instance the descriptor can bind is listed so a drive can dispatch on
    /// the instance bound when it issues.
    pub(in super::super) fn virtual_clocking_target(
        &mut self,
        path: &str,
        lhs: NodeId,
    ) -> Result<Option<VirtualClockingTarget>, String> {
        use super::super::collection::Select;
        let mut root = lhs;
        let mut selects = Vec::new();
        loop {
            let (base, select) = match self.kind(root) {
                NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                    (*base, Select::Elements(vec![*index]))
                }
                NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                    (*base, Select::Elements(indices.clone()))
                }
                NodeKind::Expr(ExprKind::PartSelect { base, left, right }) => {
                    (*base, Select::Part(*left, *right))
                }
                NodeKind::Expr(ExprKind::IndexedPartSelect {
                    base,
                    base_expr,
                    width_expr,
                    neg,
                }) => (*base, Select::Indexed(*base_expr, *width_expr, *neg)),
                _ => break,
            };
            selects.push(select);
            root = base;
        }
        if !matches!(self.kind(root), NodeKind::Expr(ExprKind::HierPath { .. })) {
            return Ok(None);
        }
        let Some(clockvar) = self.clocking_var_target(root) else {
            return Ok(None);
        };
        let Some((handle, descriptor, _slot, width, _signed, two_state, _shortreal)) =
            self.virtual_interface_access(path, root, true)?
        else {
            return Ok(None);
        };
        // Element selects precede an optional part-select (SV A.8.4 `select`).
        selects.reverse();
        if selects
            .iter()
            .rev()
            .skip(1)
            .any(|select| !matches!(select, Select::Elements(_)))
        {
            return Err(format!(
                "a part-select followed by another select of a clockvar in `{path}` is not a clockvar select"
            ));
        }
        let var = self
            .db
            .clocking_var(clockvar)
            .ok_or_else(|| format!("clocking member has no owned declaration in `{path}`"))?;
        let block_name = self.node(var.block).name.clone();
        let var_name = self.node(clockvar).name.clone();
        let mut bound = self
            .virtual_interface_instances
            .iter()
            .filter(|(_, (owner, _))| *owner == descriptor)
            .map(|(interface, (_, instance))| (*instance, *interface))
            .collect::<Vec<_>>();
        bound.sort_unstable_by_key(|(instance, _)| *instance);
        let mut instances = Vec::with_capacity(bound.len());
        for (position, (instance, interface)) in bound.into_iter().enumerate() {
            if instance != position {
                return Err(format!(
                    "virtual interface descriptor for `{path}` has non-contiguous instances"
                ));
            }
            let concrete = self
                .node(interface)
                .children
                .iter()
                .copied()
                .find(|block| {
                    self.db.is_clocking_block(*block) && self.node(*block).name == block_name
                })
                .and_then(|block| {
                    self.node(block).children.iter().copied().find(|variable| {
                        self.db.is_clocking_var(*variable) && self.node(*variable).name == var_name
                    })
                })
                .ok_or_else(|| {
                    format!(
                        "interface instance `{}` has no clockvar `{block_name}.{var_name}` for a drive in `{path}`",
                        self.node(interface).full_name
                    )
                })?;
            instances.push(concrete);
        }
        let site = self.required_site("virtual interface clocking drive", path, lhs);
        Ok(Some(VirtualClockingTarget {
            handle,
            descriptor,
            site,
            clockvar,
            instances,
            selects,
            width,
            two_state,
        }))
    }
}
