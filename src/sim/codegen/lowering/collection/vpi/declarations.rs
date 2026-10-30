//! Declaration objects of the VPI catalog (IEEE 1800-2009 37.13, 37.80):
//! generate scope arrays, genvars and io declarations (decision A19-D42).
//!
//! An io declaration shares its name with the port or declaration it
//! declares, so its key ends in a hidden `io` part that the emitter strips
//! for display, as it does for ports.

use super::*;

impl Codegen<'_> {
    /// The instance or named generate scope that declares `id`; a generate
    /// loop's own genvar is declared in the scope of its loop (37.80).
    fn vpi_declaring_scope(&self, id: NodeId) -> Option<NodeId> {
        let mut current = self.node(id).parent;
        while let Some(scope) = current {
            match self.kind(scope) {
                NodeKind::ModuleInst { .. } => return Some(scope),
                NodeKind::GenScope if !self.node(scope).name.is_empty() => return Some(scope),
                _ => current = self.node(scope).parent,
            }
        }
        None
    }

    /// The catalog key of a genvar: its declaring scope and its name.
    pub(super) fn vpi_genvar_key(&self, id: NodeId) -> Option<String> {
        let scope = self.vpi_declaring_scope(id)?;
        Some(format!(
            "{}{SEPARATOR}{}",
            self.vpi_full_name(scope),
            self.node(id).name
        ))
    }

    /// The genvar of a generate loop: each generated scope holds a local
    /// parameter of the genvar's name (27.4), which names the genvar the
    /// loop declares or the nearest enclosing scope declares.
    fn vpi_loop_genvar(&self, array: NodeId) -> Option<NodeId> {
        let node = self.node(array);
        let block = node
            .children
            .iter()
            .copied()
            .find(|child| matches!(self.kind(*child), NodeKind::GenScope))?;
        let names = self
            .node(block)
            .children
            .iter()
            .filter(|child| matches!(self.kind(**child), NodeKind::Param { local: true, .. }))
            .map(|child| self.node(*child).name.as_str())
            .collect::<Vec<_>>();
        let mut scope = Some(array);
        while let Some(current) = scope {
            for name in &names {
                if let Some(genvar) = self.node(current).children.iter().copied().find(|child| {
                    matches!(self.kind(*child), NodeKind::Genvar { .. })
                        && self.node(*child).name == *name
                }) {
                    return Some(genvar);
                }
            }
            if matches!(self.kind(current), NodeKind::ModuleInst { .. }) {
                return None;
            }
            scope = self.node(current).parent;
        }
        None
    }

    /// The row of a named generate loop (37.80): its generated scopes name
    /// it, it names its genvar.
    pub(super) fn vpi_gen_scope_array_row(&self, id: NodeId) -> Option<IrVpiObject> {
        let node = self.node(id);
        if node.name.is_empty() {
            return None;
        }
        self.vpi_declaring_scope(id)?;
        Some(IrVpiObject {
            full_name: self.vpi_full_name(id),
            name: node.name.clone(),
            kind: IrVpiObjectKind::GenScopeArray,
            port_target: self
                .vpi_loop_genvar(id)
                .and_then(|genvar| self.vpi_genvar_key(genvar)),
            ..IrVpiObject::default()
        })
    }

    /// The row of a genvar (37.80).
    pub(super) fn vpi_genvar_row(&self, id: NodeId) -> Option<IrVpiObject> {
        let node = self.node(id);
        if node.name.is_empty() {
            return None;
        }
        Some(IrVpiObject {
            full_name: self.vpi_genvar_key(id)?,
            name: node.name.clone(),
            kind: IrVpiObjectKind::GenVar,
            ..IrVpiObject::default()
        })
    }

    /// The generate scope array a generated scope belongs to.
    pub(super) fn vpi_gen_scope_array_of(&self, scope: NodeId) -> Option<String> {
        let parent = self.node(scope).parent?;
        (matches!(self.kind(parent), NodeKind::GenScopeArray) && !self.node(parent).name.is_empty())
            .then(|| self.vpi_full_name(parent))
    }

    /// An io declaration row (37.13) under the scope keyed `scope` for the
    /// declaration node `id` (a port, argument or the interface member a
    /// modport port names) declared at `at`; `target` is the object its
    /// vpiExpr leads to.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn vpi_io_decl_row(
        &self,
        typespecs: &mut TypespecTable,
        at: NodeId,
        scope: &str,
        name: &str,
        direction: IrVpiDirection,
        index: usize,
        declaration: Option<NodeId>,
        target: Option<String>,
    ) -> IrVpiObject {
        let ty = declaration.and_then(|id| match self.kind(id) {
            NodeKind::Port { ty, .. }
            | NodeKind::FuncArg { ty, .. }
            | NodeKind::Var { ty }
            | NodeKind::Net { ty, .. } => Some(ty),
            _ => None,
        });
        let integral = ty.is_some_and(|ty| {
            !matches!(
                ty.kind.as_str(),
                "real" | "shortreal" | "realtime" | "string" | "chandle" | "class" | "other"
            )
        });
        let width = ty.and_then(|ty| ty.width).filter(|_| integral).unwrap_or(0);
        IrVpiObject {
            file: self.node(at).file().map(str::to_owned),
            line: self.node(at).line,
            full_name: format!("{scope}{SEPARATOR}{name}{SEPARATOR}\u{1e}io"),
            name: name.to_owned(),
            kind: IrVpiObjectKind::IoDecl,
            width,
            signed: ty.is_some_and(|ty| ty.signed),
            direction,
            port_index: u32::try_from(index).unwrap_or(u32::MAX),
            port_target: target,
            packed_range: declaration
                .filter(|_| width > 0)
                .and_then(|id| self.vpi_packed_range(id, width)),
            typespec: declaration.and_then(|id| self.vpi_declared_typespec(typespecs, id)),
            ..IrVpiObject::default()
        }
    }

    /// The io declarations of the arguments of a task or function row.
    pub(super) fn vpi_argument_io_decls(
        &self,
        typespecs: &mut TypespecTable,
        task: NodeId,
    ) -> Vec<IrVpiObject> {
        let scope = self.vpi_full_name(task);
        self.node(task)
            .children
            .iter()
            .copied()
            .filter(|child| matches!(self.kind(*child), NodeKind::FuncArg { .. }))
            .enumerate()
            .filter(|(_, arg)| !self.node(*arg).name.is_empty())
            .filter_map(|(index, arg)| {
                let NodeKind::FuncArg { direction, .. } = self.kind(arg) else {
                    return None;
                };
                Some(self.vpi_io_decl_row(
                    typespecs,
                    arg,
                    &scope,
                    &self.node(arg).name,
                    vpi_direction(*direction),
                    index,
                    Some(arg),
                    None,
                ))
            })
            .collect()
    }

    /// The io declarations of the ports of a modport of an interface
    /// instance: a simple port declares the interface member it names.
    pub(super) fn vpi_modport_io_decls(
        &self,
        typespecs: &mut TypespecTable,
        modport: NodeId,
        interface: NodeId,
    ) -> Vec<IrVpiObject> {
        let scope = format!(
            "{}{SEPARATOR}{}",
            self.vpi_full_name(interface),
            self.node(modport).name
        );
        self.node(modport)
            .children
            .iter()
            .copied()
            .filter(|child| self.db.semantic_detail(*child) == Some("ModportPort"))
            .enumerate()
            .filter(|(_, port)| !self.node(*port).name.is_empty())
            .map(|(index, port)| {
                let name = &self.node(port).name;
                // A simple port names the interface member of its name; an
                // expression port names the member it references, if any.
                let member = match self.db.modport_port_expression(port) {
                    Some(expression) => self.vpi_reference_target(expression),
                    None => self
                        .node(interface)
                        .children
                        .iter()
                        .copied()
                        .find(|child| self.node(*child).name == *name),
                }
                .filter(|member| {
                    matches!(
                        self.kind(*member),
                        NodeKind::Var { .. } | NodeKind::Net { .. }
                    )
                });
                self.vpi_io_decl_row(
                    typespecs,
                    port,
                    &scope,
                    name,
                    self.db
                        .modport_port_direction(port)
                        .map_or(IrVpiDirection::None, vpi_direction),
                    index,
                    member,
                    member.map(|member| self.vpi_full_name(member)),
                )
            })
            .collect()
    }
}

/// The `vpiDirection` of a declared direction.
pub(super) fn vpi_direction(direction: Direction) -> IrVpiDirection {
    match direction {
        Direction::Input => IrVpiDirection::Input,
        Direction::Output => IrVpiDirection::Output,
        Direction::Inout => IrVpiDirection::Inout,
        Direction::Ref => IrVpiDirection::Ref,
        _ => IrVpiDirection::None,
    }
}
