//! Virtual interfaces.

use super::*;

impl Codegen<'_> {
    fn virtual_interface_access(
        &mut self,
        path: &str,
        node: NodeId,
        write: bool,
    ) -> Result<Option<VirtualInterfaceAccess>, String> {
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
        let mut member = parts
            .iter()
            .skip(position + 1)
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
        if member.is_empty() {
            return Ok(None);
        }
        let spelling = self
            .virtual_interface_spelling(handle)
            .ok_or_else(|| format!("virtual interface handle has no type in `{path}`"))?;
        let identity = Codegen::normalize_virtual_interface_identity(
            &Codegen::virtual_interface_identity_from_spelling(&spelling),
        );
        let descriptor = self
            .virtual_interface_types
            .get(&identity)
            .copied()
            .ok_or_else(|| format!("virtual interface type `{identity}` has no descriptor"))?;
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
        let slot = self
            .virtual_interface_members
            .get(&(descriptor, member.clone()))
            .copied()
            .ok_or_else(|| {
                // A view port without interface storage of its own is a
                // modport expression port (SV 25.5.4); its expression is
                // not evaluated per bound instance at run time.
                if view_port {
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
        // A handle held in a record or class member (`r.v.x`) is the value
        // of the member access expression, not of its declaration.
        let handle = if position > 0 {
            self.member_prefix_expression(node, position + 1)
                .unwrap_or(handle)
        } else {
            handle
        };
        let handle = self.lower_chandle(path, handle)?;
        Ok(Some((handle, descriptor, slot, width, signed, two_state)))
    }

    /// The nested member access of `node` whose path has `length` components.
    /// The frontend nests each member access in the next one.
    fn member_prefix_expression(&self, node: NodeId, length: usize) -> Option<NodeId> {
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

    /// Lower a member path rooted at a virtual-interface handle. The runtime
    /// receives the handle on every access, so assigning a new handle changes
    /// the target of all subsequent reads and writes.
    pub(in super::super) fn virtual_interface_member_expr(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrExpr>, String> {
        let Some((handle, descriptor, slot, width, signed, _two_state)) =
            self.virtual_interface_access(path, node, false)?
        else {
            return Ok(None);
        };
        let site = format!(
            "{path}.{}",
            self.model.virtual_interfaces[descriptor].members[slot].name
        );
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
        let Some((handle, descriptor, slot, width, signed, two_state)) =
            self.virtual_interface_access(path, node, true)?
        else {
            return Ok(None);
        };
        let site = format!(
            "{path}.{}",
            self.model.virtual_interfaces[descriptor].members[slot].name
        );
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
            shortreal: false,
        }))
    }
}
