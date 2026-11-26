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
    fn virtual_interface_handle(
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
            let site = self.source_site(
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
                } else if self
                    .virtual_interface_array_members(descriptor, &member)
                    .is_some()
                {
                    self.locate_error(
                        node,
                        format!(
                            "unpacked array member `{member}` of virtual interface `{spelling}` \
                             is supported only through element selects (`{member}[i]`, \
                             `{member}[i][b]`) that are read or assigned with `=` or `<=`; a \
                             whole-array or slice use, or a use in an event control or wait, \
                             is not supported in `{path}`"
                        ),
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
        let site = self.source_site("virtual interface member access", path, node);
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
        let site = self.source_site("virtual interface member access", path, node);
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
    spelling: String,
    view_port: bool,
}

/// An element of an unpacked array member reached through a
/// virtual-interface handle (`v.mem[i]`; SV 25.9). The member's array in
/// every instance the descriptor can bind is listed by instance index, so an
/// access dispatches on the instance bound when it executes, like the
/// SIM-034 clocking drives.
pub(in super::super) struct VirtualInterfaceArrayElement {
    pub(in super::super) handle: IrChandleExpr,
    pub(in super::super) descriptor: usize,
    pub(in super::super) site: String,
    pub(in super::super) arrays: Vec<ArrayInfo>,
    /// One index per unpacked dimension.
    pub(in super::super) indices: Vec<NodeId>,
    /// A bit, part or indexed part select of the element (`v.mem[i][3:0]`).
    pub(in super::super) select: Option<NodeId>,
    /// The member declaration the frontend names as the select base; it
    /// carries the element's packed range.
    pub(in super::super) declaration: NodeId,
    /// The member path through the handle (`v.mem`), the select's
    /// structural operand.
    pub(in super::super) member_path: NodeId,
    /// The handle declaration and member name, which identify the selected
    /// storage in the source text.
    pub(in super::super) key: (NodeId, String),
}

impl Codegen<'_> {
    /// The array of unpacked member `member` in every instance bound to
    /// descriptor `descriptor`, by instance index, or `None` when the member
    /// is not an unpacked array of the bound instances.
    fn virtual_interface_array_members(
        &self,
        descriptor: usize,
        member: &str,
    ) -> Option<Vec<ArrayInfo>> {
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
                    .find_map(|child| self.array_globals.get(&child).cloned())
            })
            .collect()
    }

    /// Resolve `v.mem[i]` or `v.mem[i][b]`: an element (or a packed select
    /// of an element) of an unpacked array member through a
    /// virtual-interface handle. Other selects of such a member (slices,
    /// fewer indices than dimensions, real elements) and instances whose
    /// arrays differ in shape are located errors.
    pub(in super::super) fn virtual_interface_array_element(
        &mut self,
        path: &str,
        node: NodeId,
        write: bool,
    ) -> Result<Option<VirtualInterfaceArrayElement>, String> {
        if let NodeKind::Expr(
            ExprKind::PartSelect { base, .. } | ExprKind::IndexedPartSelect { base, .. },
        ) = self.kind(node)
        {
            let inner = *base;
            let Some(mut element) = self.virtual_interface_array_element(path, inner, write)?
            else {
                return Ok(None);
            };
            if element.select.is_some() {
                return Err(self.locate_error(
                    node,
                    format!(
                        "a select of a select of an element of an unpacked array member \
                     through a virtual interface is not supported in `{path}`"
                    ),
                ));
            }
            element.select = Some(node);
            return Ok(Some(element));
        }
        let (declaration, mut indices) = match self.kind(node) {
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => (*base, vec![*index]),
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => (*base, indices.clone()),
            _ => return Ok(None),
        };
        let base = declaration;
        // The select names the member declaration as its base; the path
        // through the handle is its structural operand, below the inner
        // selects of a multidimensional element select.
        let base = if matches!(self.kind(base), NodeKind::Expr(ExprKind::HierPath { .. })) {
            base
        } else {
            let mut current = node;
            loop {
                let next = self.node(current).children.iter().copied().find(|child| {
                    match self.kind(*child) {
                        NodeKind::Expr(ExprKind::HierPath { refs, .. }) => {
                            refs.last().copied().flatten() == Some(base)
                        }
                        NodeKind::Expr(ExprKind::ArraySelect { base: inner, .. }) => *inner == base,
                        _ => false,
                    }
                });
                match next {
                    Some(child)
                        if matches!(
                            self.kind(child),
                            NodeKind::Expr(ExprKind::HierPath { .. })
                        ) =>
                    {
                        break child;
                    }
                    Some(child) => current = child,
                    None => return Ok(None),
                }
            }
        };
        let Some(target) = self.virtual_interface_member_path(path, base, write)? else {
            return Ok(None);
        };
        if self
            .virtual_interface_members
            .contains_key(&(target.descriptor, target.member.clone()))
        {
            return Ok(None);
        }
        let Some(arrays) = self.virtual_interface_array_members(target.descriptor, &target.member)
        else {
            return Ok(None);
        };
        let location = self.source_location(node);
        let member = &target.member;
        let first = &arrays[0];
        if arrays.iter().any(|array| {
            array.elem_width != first.elem_width
                || array.signed != first.signed
                || array.real != first.real
                || array.dims.len() != first.dims.len()
        }) {
            return Err(crate::sim::codegen::error::located(
                &location,
                format!(
                    "unpacked array member `{member}` has different element types or \
                 dimensions in the instances virtual interface `{}` can bind, in `{path}`",
                    target.spelling
                ),
            ));
        }
        if first.real || first.shortreal {
            return Err(crate::sim::codegen::error::located(
                &location,
                format!(
                    "an element of real unpacked array member `{member}` through \
                 virtual interface `{}` is not supported in `{path}`",
                    target.spelling
                ),
            ));
        }
        let select = (indices.len() == first.dims.len() + 1)
            .then(|| indices.pop())
            .flatten();
        if indices.len() != first.dims.len() {
            return Err(crate::sim::codegen::error::located(
                &location,
                format!(
                    "a slice of unpacked array member `{member}` through virtual \
                 interface `{}` is not supported in `{path}`; select one element",
                    target.spelling
                ),
            ));
        }
        let handle = self.virtual_interface_handle(path, base, &target)?;
        let site = self.source_site("virtual interface member access", path, node);
        let key = (target.handle, self.virtual_interface_key_member(&target));
        Ok(Some(VirtualInterfaceArrayElement {
            handle,
            descriptor: target.descriptor,
            site,
            arrays,
            indices,
            select,
            declaration,
            member_path: base,
            key,
        }))
    }

    /// The element selection of an array-element access and its width.
    pub(in super::super) fn virtual_interface_element_select(
        &mut self,
        path: &str,
        element: &VirtualInterfaceArrayElement,
    ) -> Result<(IrElemSel, u32), String> {
        let base = element.declaration;
        let Some(last) = element.select else {
            return Ok((IrElemSel::Whole, element.arrays[0].elem_width));
        };
        Ok(match self.kind(last) {
            NodeKind::Expr(ExprKind::PartSelect { left, right, .. }) => {
                let (left, right) = (*left, *right);
                let l = self.packed_relative_bound(base, self.eval_bound_i128(left)?)?;
                let r = self.packed_relative_bound(base, self.eval_bound_i128(right)?)?;
                let (l, r, width) = checked_select_bounds(l, r, "array-element part select")?;
                (IrElemSel::Part(l, r), width)
            }
            NodeKind::Expr(ExprKind::IndexedPartSelect {
                base_expr,
                width_expr,
                neg,
                ..
            }) => {
                let (base_expr, width_expr, neg) = (*base_expr, *width_expr, *neg);
                let width = self.indexed_part_select_width(width_expr, path)?;
                (
                    IrElemSel::Indexed {
                        base: Box::new(self.lower_packed_index(path, base, base_expr)?),
                        width,
                        negative: neg ^ self.packed_range_ascending(base),
                    },
                    width,
                )
            }
            _ => (
                IrElemSel::Bit(Box::new(self.lower_packed_index(path, base, last)?)),
                1,
            ),
        })
    }

    /// The bound instance of an array-element access, as an unsigned 32-bit
    /// selector; an invalid handle reports at the access site.
    pub(in super::super) fn virtual_interface_instance_query(
        element: &VirtualInterfaceArrayElement,
    ) -> IrExpr {
        IrExpr::new(
            IrExprKind::ObjectQuery(Box::new(IrObjectQuery::VirtualInterfaceInstance {
                handle: element.handle.clone(),
                interface: element.descriptor,
                site: element.site.clone(),
            })),
            32,
            false,
            None,
        )
    }

    /// Read `v.mem[i]`: the element of the bound instance's array. The
    /// indices are lowered once per instance arm, and exactly one arm runs;
    /// an invalid handle reads X.
    pub(in super::super) fn virtual_interface_array_element_expr(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrExpr>, String> {
        let Some(element) = self.virtual_interface_array_element(path, node, false)? else {
            return Ok(None);
        };
        let indices = element
            .indices
            .iter()
            .map(|index| self.lower_expr(path, *index))
            .collect::<Result<Vec<_>, _>>()?;
        let (elem_sel, width) = self.virtual_interface_element_select(path, &element)?;
        let signed = element.select.is_none() && element.arrays[0].signed;
        let query = Codegen::virtual_interface_instance_query(&element);
        let mut value = const_x_expr(width);
        for (instance, array) in element.arrays.iter().enumerate().rev() {
            let read = IrExpr::new(
                IrExprKind::ArrayRead {
                    arr: self.reference_array(array.ir),
                    indices: indices.clone(),
                    elem_sel: elem_sel.clone(),
                },
                width,
                signed,
                None,
            );
            let selected = IrExpr::new(
                IrExprKind::Bin {
                    op: IrBinOp::Eq,
                    a: Box::new(query.clone()),
                    b: Box::new(lhs_integer_expr(instance as i128)),
                },
                1,
                false,
                None,
            );
            value = IrExpr::new(
                IrExprKind::Mux {
                    sel: Box::new(selected),
                    a: Box::new(read),
                    b: Box::new(value),
                },
                width,
                signed,
                None,
            );
        }
        Ok(Some(value))
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

    /// The event `node` names through a virtual interface: the handle selects
    /// the descriptor's event slot of the bound instance where the operation
    /// is issued.
    pub(in super::super) fn virtual_event_ref(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrEventRef>, String> {
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
        let site = self.source_site(kind, path, node);
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
        let site = self.source_site("virtual interface clocking drive", path, lhs);
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
