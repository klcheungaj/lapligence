//! Owned-database collection, storage allocation, wiring, and process setup.

use super::*;
use crate::sim::ir::{
    IrChandleExpr, IrContainerElement, IrContainerMember, IrObjectStmt, IrObjectType, IrStringExpr,
};

fn lower_container_element(descriptor: &TypeDescriptor) -> Result<IrContainerElement, String> {
    let packed = || {
        descriptor
            .info
            .width
            .filter(|width| *width != 0)
            .map(|width| IrContainerElement::Packed {
                width,
                signed: descriptor.info.signed,
                two_state: descriptor.info.kind == "bit"
                    || matches!(
                        descriptor.info.kind.as_str(),
                        "int" | "longint" | "byte" | "shortint"
                    ),
            })
    };
    match &descriptor.shape {
        TypeShape::PackedAtom { .. } => packed().ok_or_else(|| {
            format!(
                "container element `{}` has no representable packed width",
                descriptor.name
            )
        }),
        TypeShape::Real { shortreal } => Ok(IrContainerElement::Real {
            shortreal: *shortreal,
        }),
        TypeShape::String => Ok(IrContainerElement::String),
        TypeShape::Aggregate(layout)
            if matches!(
                layout.kind,
                AggregateKind::PackedStruct | AggregateKind::PackedUnion
            ) && descriptor.info.width.is_some() =>
        {
            packed().ok_or_else(|| {
                format!(
                    "container element `{}` has no representable packed width",
                    descriptor.name
                )
            })
        }
        TypeShape::Aggregate(layout) => Ok(IrContainerElement::Aggregate {
            type_id: descriptor.id.0,
            members: layout
                .members
                .iter()
                .map(|member| {
                    Ok(IrContainerMember {
                        name: member.name.clone(),
                        element: Box::new(lower_container_element(&member.descriptor)?),
                    })
                })
                .collect::<Result<Vec<_>, String>>()?,
        }),
        TypeShape::FixedArray {
            dimensions,
            element,
        } => Ok(IrContainerElement::FixedArray {
            dimensions: dimensions.clone(),
            element: Box::new(lower_container_element(element)?),
        }),
        TypeShape::Container { kind, element } => Ok(IrContainerElement::Container {
            type_id: element.id.0,
            kind: kind.clone(),
            element: Box::new(lower_container_element(element)?),
        }),
        TypeShape::Opaque { kind } if kind == "Chandle" => Ok(IrContainerElement::Chandle),
        TypeShape::Opaque { kind } if kind == "Event" => Ok(IrContainerElement::Event),
        TypeShape::Opaque { kind } => Ok(IrContainerElement::Opaque {
            type_id: descriptor.id.0,
            kind: kind.clone(),
        }),
    }
}

pub(super) fn aggregate_path_suffix(path: &[AggregatePathPart]) -> String {
    path.iter()
        .map(|part| match part {
            AggregatePathPart::Member(name) => name.clone(),
            AggregatePathPart::Index(index) => format!("i{index}"),
        })
        .collect::<Vec<_>>()
        .join("__")
}

pub(super) fn aggregate_path_key(path: &[AggregatePathPart]) -> String {
    path.iter()
        .map(|part| match part {
            AggregatePathPart::Member(name) => format!("m:{name}"),
            AggregatePathPart::Index(index) => format!("i:{index}"),
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn leaf_member(base: &AggregateMember, descriptor: &TypeDescriptor) -> AggregateMember {
    let packed_ranges = match &descriptor.shape {
        TypeShape::PackedAtom { ranges } => ranges.clone(),
        _ => base.packed_ranges.clone(),
    };
    AggregateMember {
        name: base.name.clone(),
        ty: descriptor.info.clone(),
        two_state: base.two_state,
        packed_ranges,
        aggregate: None,
        descriptor: descriptor.clone(),
    }
}

fn port_array_index_vectors(dims: &[(i32, i32)]) -> Vec<Vec<i32>> {
    fn visit(
        dims: &[(i32, i32)],
        dimension: usize,
        current: &mut Vec<i32>,
        values: &mut Vec<Vec<i32>>,
    ) {
        if dimension == dims.len() {
            values.push(current.clone());
            return;
        }
        let (left, right) = dims[dimension];
        let step = if left <= right { 1 } else { -1 };
        let mut index = left;
        loop {
            current.push(index);
            visit(dims, dimension + 1, current, values);
            current.pop();
            if index == right {
                break;
            }
            index = index.saturating_add(step);
        }
    }

    let mut values = Vec::new();
    visit(dims, 0, &mut Vec::new(), &mut values);
    values
}

#[derive(Default)]
struct ProcessContractScan {
    event_controls: Vec<NodeId>,
    fork_controls: Vec<NodeId>,
    blocking_timing_controls: Vec<NodeId>,
    disallowed_assignments: Vec<NodeId>,
    event_triggers: Vec<NodeId>,
}

#[derive(Clone)]
struct ProcessWriter {
    node: NodeId,
    label: String,
    writes: HashSet<IrDependency>,
}

impl<'a> Codegen<'a> {
    pub(super) fn signal_dependency(&self, info: &SignalInfo) -> IrDependency {
        self.reference_dependency(info)
    }

    /// Register storage for a procedural declaration.
    ///
    /// Automatic variables remain lexical C locals. Static variables become
    /// hidden model signals so they retain values across block reentry and
    /// participate in typed optimizer read/write accounting.
    pub(super) fn collect_loop_var(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<ProcLocalInfo, String> {
        let lifetime = self.db.variable_lifetime(node);
        match lifetime {
            VariableLifetime::Automatic => {
                if let Some(info) = self.proc_locals.get(&node) {
                    return Ok(info.clone());
                }
            }
            VariableLifetime::Static => {
                if let Some(info) = self.proc_local_instances.get(&(self.inst, node)) {
                    let info = info.clone();
                    self.proc_locals.insert(node, info.clone());
                    return Ok(info);
                }
            }
            VariableLifetime::Unavailable => {}
        }
        let ty = match self.kind(node) {
            NodeKind::Var { ty } => ty.clone(),
            other => {
                return Err(format!(
                    "unsupported procedural loop declaration in `{path}` (node kind {other:?})"
                ))
            }
        };
        let real = is_real_kind(&ty.kind);
        let width = if real {
            0
        } else {
            self.signal_width(path, &self.node(node).name, &ty)?
        };
        let (c_name, static_signal) = match lifetime {
            VariableLifetime::Automatic => (format!("_lv{}", node.index()), None),
            VariableLifetime::Static => {
                let ir = self.model.signals.len();
                let signal = SignalInfo {
                    global: format!("_ls{}_{}", self.inst.index(), node.index()),
                    width,
                    signed: ty.signed,
                    two_state: self.db.is_two_state_type(node) || is_two_state_kind(&ty.kind),
                    real,
                    shortreal: real && ty.kind == "shortreal",
                    net_driver: None,
                    ir,
                };
                self.model.signals.push(IrSignal {
                    c_name: signal.global.clone(),
                    hdl_name: None,
                    ty: if real {
                        IrType::Real {
                            shortreal: ty.kind == "shortreal",
                        }
                    } else {
                        IrType::Packed {
                            width: signal.width,
                            signed: signal.signed,
                            two_state: signal.two_state,
                        }
                    },
                    net_driver: None,
                    alias: None,
                    omit: false,
                });
                if let Some(initializer) = self.db.var_initializer(node) {
                    let lowered = self.lower_declaration_initializer(
                        path,
                        node,
                        initializer,
                        IrInitTarget::Signal(signal.ir),
                        signal.width,
                        signal.signed,
                        signal.two_state,
                        signal.real,
                    );
                    match lowered {
                        Ok(initializer) => self.declaration_inits.push(initializer),
                        Err(lowering_error) => {
                            let value = self
                                .var_decl_init(path, &self.node(node).name, initializer)
                                .map_err(|_| lowering_error)?;
                            self.var_inits.push((signal.clone(), value));
                        }
                    }
                }
                (signal.global.clone(), Some(signal))
            }
            VariableLifetime::Unavailable => {
                return Err(format!(
                    "resolved lifetime is unavailable for procedural variable `{}` in `{path}`",
                    self.node(node).name
                ));
            }
        };
        let info = ProcLocalInfo {
            c_name,
            width,
            signed: ty.signed,
            two_state: if real {
                false
            } else {
                self.db.is_two_state_type(node) || is_two_state_kind(&ty.kind)
            },
            static_signal,
        };
        if matches!(lifetime, VariableLifetime::Static) {
            self.proc_local_instances
                .insert((self.inst, node), info.clone());
        }
        self.proc_locals.insert(node, info.clone());
        Ok(info)
    }

    /// Register an automatic native-string foreach iterator. Strings use the
    /// object ABI rather than packed/real `ProcLocalInfo`; the generated name
    /// is still declaration-derived so nested loop scopes cannot collide.
    pub(super) fn collect_loop_string_var(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<String, String> {
        if let Some(name) = self.proc_string_locals.get(&node) {
            return Ok(name.clone());
        }
        let is_string = matches!(
            self.kind(node),
            NodeKind::Var { ty } if ty.kind == "string"
        );
        if !is_string {
            return Err(format!(
                "foreach string iterator `{}` in `{path}` is not a string variable",
                self.node(node).name
            ));
        }
        if self.db.variable_lifetime(node) != VariableLifetime::Automatic {
            return Err(format!(
                "string foreach iterator `{}` in `{path}` must have automatic lifetime",
                self.node(node).name
            ));
        }
        let name = format!("_lv{}", node.index());
        self.proc_string_locals.insert(node, name.clone());
        Ok(name)
    }

    pub(super) fn proc_string_local_name(&self, node: NodeId) -> Option<&str> {
        self.proc_string_locals.get(&node).map(String::as_str)
    }

    pub(super) fn is_foreach_iterator(&self, node: NodeId) -> bool {
        self.db.node_ids().any(|id| match self.kind(id) {
            NodeKind::Stmt(StmtKind::Foreach { vars, .. }) => {
                vars.iter().flatten().any(|variable| *variable == node)
            }
            _ => false,
        })
    }

    /// Resolve a string loop iterator through its lexical statement scopes.
    /// This mirrors `lexical_proc_local` while keeping native-string storage
    /// out of packed expression paths.
    pub(super) fn lexical_proc_string_local(&self, reference: NodeId) -> Option<(NodeId, &str)> {
        let name = self.node(reference).name.as_str();
        let mut parent = self.node(reference).parent;
        while let Some(scope) = parent {
            if matches!(self.kind(scope), NodeKind::Stmt(StmtKind::Begin)) {
                if let Some(variable) = self.node(scope).children.iter().find(|child| {
                    matches!(self.kind(**child), NodeKind::Var { .. })
                        && self.node(**child).name == name
                }) {
                    if let Some(c_name) = self.proc_string_local_name(*variable) {
                        return Some((*variable, c_name));
                    }
                }
            }
            let variable = match self.kind(scope) {
                NodeKind::Stmt(StmtKind::For { vars, .. }) => vars
                    .iter()
                    .find(|variable| self.node(**variable).name == name)
                    .copied(),
                NodeKind::Stmt(StmtKind::Foreach { vars, .. }) => vars
                    .iter()
                    .flatten()
                    .find(|variable| self.node(**variable).name == name)
                    .copied(),
                _ => None,
            };
            if let Some(variable) = variable {
                if let Some(c_name) = self.proc_string_local_name(variable) {
                    return Some((variable, c_name));
                }
            }
            parent = self.node(scope).parent;
        }
        None
    }

    /// Resolve a process-local declaration in the current elaborated
    /// instance. Static declarations need the instance-qualified map because
    /// one owned declaration node can be instantiated more than once.
    pub(super) fn proc_local_info(&self, node: NodeId) -> Option<&ProcLocalInfo> {
        match self.db.variable_lifetime(node) {
            VariableLifetime::Static => self.proc_local_instances.get(&(self.inst, node)),
            VariableLifetime::Automatic | VariableLifetime::Unavailable => {
                self.proc_locals.get(&node)
            }
        }
    }

    pub(super) fn proc_local_target(&self, node: NodeId) -> Option<NodeId> {
        if let Some((variable, _)) = self.lexical_proc_local(node) {
            return self
                .proc_local_info(variable)
                .is_some_and(|info| info.static_signal.is_none())
                .then_some(variable);
        }
        if self.proc_local_is_shadowed(node) {
            return None;
        }
        match self.kind(node) {
            NodeKind::Var { .. }
                if self
                    .proc_local_info(node)
                    .is_some_and(|info| info.static_signal.is_none()) =>
            {
                Some(node)
            }
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) if self
                .proc_local_info(*target)
                .is_some_and(|info| info.static_signal.is_none()) =>
            {
                Some(*target)
            }
            _ => None,
        }
    }

    pub(super) fn lexical_proc_local(&self, reference: NodeId) -> Option<(NodeId, &ProcLocalInfo)> {
        let name = self.node(reference).name.as_str();
        let mut parent = self.node(reference).parent;
        while let Some(scope) = parent {
            if matches!(self.kind(scope), NodeKind::Stmt(StmtKind::Begin)) {
                if let Some(variable) = self.node(scope).children.iter().find(|child| {
                    matches!(self.kind(**child), NodeKind::Var { .. })
                        && self.node(**child).name == name
                }) {
                    return self
                        .proc_local_info(*variable)
                        .map(|info| (*variable, info));
                }
            }
            let vars = match self.kind(scope) {
                NodeKind::Stmt(StmtKind::For { vars, .. }) => {
                    return vars
                        .iter()
                        .find(|variable| self.node(**variable).name == name)
                        .and_then(|variable| {
                            self.proc_local_info(*variable)
                                .map(|info| (*variable, info))
                        });
                }
                NodeKind::Stmt(StmtKind::Foreach { vars, .. }) => Some(vars.as_slice()),
                _ => None,
            };
            if let Some(variable) = vars.and_then(|vars| {
                vars.iter()
                    .flatten()
                    .find(|variable| self.node(**variable).name == name)
            }) {
                if let Some(info) = self.proc_local_info(*variable) {
                    return Some((*variable, info));
                }
            }
            parent = self.node(scope).parent;
        }
        None
    }

    pub(super) fn proc_local_is_shadowed(&self, reference: NodeId) -> bool {
        let name = self.node(reference).name.as_str();
        let mut parent = self.node(reference).parent;
        while let Some(scope) = parent {
            if matches!(self.kind(scope), NodeKind::Stmt(StmtKind::Begin))
                && self.node(scope).children.iter().any(|child| {
                    matches!(self.kind(*child), NodeKind::Var { .. })
                        && self.node(*child).name == name
                })
            {
                return true;
            }
            let is_loop_var = match self.kind(scope) {
                NodeKind::Stmt(StmtKind::For { vars, .. }) => vars
                    .iter()
                    .any(|variable| self.node(*variable).name == name),
                NodeKind::Stmt(StmtKind::Foreach { vars, .. }) => vars
                    .iter()
                    .flatten()
                    .any(|variable| self.node(*variable).name == name),
                _ => false,
            };
            if is_loop_var {
                return false;
            }
            parent = self.node(scope).parent;
        }
        false
    }

    pub(super) fn nested_proc_local_ref(&self, node: NodeId) -> Option<NodeId> {
        if let Some((variable, info)) = self.lexical_proc_local(node) {
            return info.static_signal.is_none().then_some(variable);
        }
        if self.proc_local_is_shadowed(node) {
            return self
                .node(node)
                .children
                .iter()
                .find_map(|child| self.nested_proc_local_ref(*child));
        }
        if let NodeKind::Expr(ExprKind::Ref {
            target: Some(target),
        }) = self.kind(node)
        {
            if self
                .proc_local_info(*target)
                .is_some_and(|info| info.static_signal.is_none())
            {
                return Some(*target);
            }
        }
        self.node(node)
            .children
            .iter()
            .find_map(|child| self.nested_proc_local_ref(*child))
    }

    /// Collect automatic values referenced by one evaluated event expression
    /// and place them in a private, copied activation frame. The callback may
    /// run after the issuing process suspends or is re-entered, so it must
    /// never refer directly to a lexical C local.
    pub(super) fn event_context(
        &mut self,
        expression: NodeId,
    ) -> Result<Option<IrEventContext>, String> {
        fn visit(cg: &Codegen<'_>, node: NodeId, out: &mut HashSet<NodeId>) {
            let target = match cg.kind(node) {
                NodeKind::Expr(ExprKind::Ref {
                    target: Some(target),
                }) => Some(*target),
                _ => cg.lexical_proc_local(node).map(|(target, _)| target),
            };
            if let Some(target) = target {
                if cg
                    .capture_source(target)
                    .is_some_and(|source| source.info.static_signal.is_none())
                {
                    out.insert(target);
                }
            }
            for child in &cg.node(node).children {
                visit(cg, *child, out);
            }
        }

        let mut targets = HashSet::new();
        visit(self, expression, &mut targets);
        let mut targets = targets.into_iter().collect::<Vec<_>>();
        targets.sort_by_key(|target| target.index());
        if targets.is_empty() {
            return Ok(None);
        }

        let sources = targets
            .iter()
            .map(|target| {
                self.capture_source(*target).ok_or_else(|| {
                    format!(
                        "automatic declaration `{}` was not collected before event capture",
                        self.node(*target).name
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let frame = self.new_frame_id()?;
        let captures = sources
            .into_iter()
            .zip(targets)
            .filter_map(|(source, target)| {
                // Inlined const-ref formals can be substituted directly with
                // their signal/array actual. Such references already have a
                // stable dependency and must not manufacture a frame whose
                // initializer would be rendered outside the caller's formal
                // context. Only lexical C locals need a copied evaluator slot.
                let IrExprKind::LocalRead(local) = source.initial.kind() else {
                    return None;
                };
                Some((
                    self.declaration_identity(target),
                    local.clone(),
                    source.lifetime,
                    super::storage_kind(source.info.width),
                    source.initial,
                ))
            })
            .enumerate()
            .map(|(slot, (declaration, local, lifetime, kind, initial))| {
                Ok(IrEventCapture::new(
                    StorageRef::for_declaration(
                        frame,
                        slot as u32,
                        declaration?,
                        lifetime,
                        StorageOwnership::Owned,
                    )
                    .with_kind(kind),
                    local,
                    initial,
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;
        if captures.is_empty() {
            return Ok(None);
        }
        Ok(Some(IrEventContext::new(frame, captures)))
    }

    /// Walk the instance tree, collecting signals, parameters and gen-scope
    /// paths.  Returns the top module nodes.
    pub(super) fn collect_design(&mut self) -> Result<Vec<NodeId>, String> {
        for node in self.design_nodes() {
            let net_type = match self.kind(node) {
                NodeKind::Net { net_type, .. } => Some(*net_type),
                NodeKind::Array { .. } => self.db.array_meta(node).and_then(|meta| meta.net_type()),
                _ => None,
            };
            if net_type == Some(NetType::TriReg) {
                return Err(format!(
                    "unsupported net type TriReg: trireg charge storage is not supported for `{}`; outside the standalone subset",
                    self.display_name(node)
                ));
            }
        }
        let mut tops = Vec::new();
        for top in self.db.tops() {
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
        // Packages have one shared static environment. Collect them
        // separately so package state is not duplicated per module user.
        for package in self.db.packages() {
            let path = self.instance_path_of(*package);
            if path.is_empty() {
                return Err("package has no name".to_string());
            }
            self.collect_instance(*package, &path)?;
            self.collect_funcs(*package, &path)?;
        }
        // Compilation-unit declarations form one shared `$unit` namespace per
        // admitted compilation unit. They are not design roots, so collect
        // them explicitly after package storage is allocated; references keep
        // their owned declaration identity across every importing module.
        for unit in self.compilation_unit_scopes() {
            let path = self.instance_path_of(unit);
            self.collect_instance(unit, &path)?;
            self.collect_funcs(unit, &path)?;
        }
        Ok(tops)
    }

    pub(super) fn compilation_unit_scopes(&self) -> Vec<NodeId> {
        self.db
            .node_ids()
            .filter(|id| self.is_compilation_unit(*id))
            .collect()
    }

    fn is_compilation_unit(&self, id: NodeId) -> bool {
        matches!(self.kind(id), NodeKind::Stmt(StmtKind::Begin))
            && self.db.semantic_detail(id) == Some("CompilationUnit")
    }

    pub(super) fn is_runtime_environment(&self, id: NodeId) -> bool {
        matches!(self.kind(id), NodeKind::Package) || self.is_compilation_unit(id)
    }

    pub(super) fn namespace_path(&self, id: NodeId) -> String {
        if matches!(self.kind(id), NodeKind::Package) {
            return strip_lib(&self.node(id).name);
        }
        if self.is_compilation_unit(id) {
            return format!("$unit_{}", id.index());
        }
        strip_lib(&self.node(id).name)
    }

    fn collect_aggregate(&mut self, path: &str, node: NodeId) -> Result<bool, String> {
        let Some(layout) = self.db.aggregate_layout(node).cloned() else {
            return Ok(false);
        };
        // A declaration can be reached through both its instance child list
        // and a reference-port connection.  Its leaf storage is owned by the
        // declaration, so collection must be idempotent before allocating
        // recursive object/signal leaves.
        if self.unpacked_aggregates.contains_key(&node) {
            return Ok(true);
        }
        match layout.kind {
            AggregateKind::PackedStruct => return Ok(false),
            AggregateKind::PackedUnion => {
                let width = layout.members.first().and_then(|member| member.ty.width);
                if width.is_none() || layout.members.iter().any(|member| member.ty.width != width) {
                    return Err(format!(
                        "packed untagged union `{}` in `{path}` has members of unequal or unresolved width",
                        self.node(node).name
                    ));
                }
                return Ok(false);
            }
            AggregateKind::TaggedUnion => {
                return Err(format!(
                    "tagged union `{}` in `{path}` is not supported",
                    self.node(node).name
                ));
            }
            AggregateKind::UnpackedStruct | AggregateKind::UnpackedUnion => {}
        }
        let object_name = self.node(node).name.clone();
        if let Some(aggregate) = self
            .unpacked_aggregates
            .iter()
            .find(|(existing, _)| {
                self.node(**existing).name == object_name
                    && self.instance_path_of(**existing) == path
            })
            .map(|(_, aggregate)| aggregate.clone())
        {
            // Slang can expose the same ref-port aggregate declaration through
            // more than one child node.  The instance path plus declaration
            // name identifies one storage owner; reuse its descriptor rather
            // than allocating duplicate recursive leaves.
            self.unpacked_aggregates.insert(node, aggregate);
            return Ok(true);
        }
        let is_ref_port = matches!(
            self.kind(node),
            NodeKind::Port {
                direction: DbDirection::Ref,
                ..
            }
        );
        if !matches!(self.kind(node), NodeKind::Var { .. }) && !is_ref_port {
            if matches!(
                self.kind(node),
                NodeKind::Net { .. }
                    | NodeKind::Array { .. }
                    | NodeKind::FuncArg { .. }
                    | NodeKind::IoDecl { .. }
            ) {
                return Err(format!(
                    "unpacked aggregate net/array/port `{}` in `{path}` is not supported",
                    self.node(node).name
                ));
            }
            // Slang projects aggregate layouts onto type declarations as well
            // as the variables that use them. Type-only nodes allocate no
            // runtime storage; the corresponding Var is collected separately.
            return Ok(false);
        }
        let is_union = layout.kind == AggregateKind::UnpackedUnion;
        // An untagged union has one storage extent, not one storage slot per
        // display member.  Legal unequal-width members therefore share a
        // slot sized to the largest packed member; reads/writes apply the
        // selected member's own width and signedness at the boundary.
        let union_signal = if is_union {
            if layout
                .members
                .iter()
                .any(|member| !matches!(member.descriptor.shape, TypeShape::PackedAtom { .. }))
            {
                return Err(format!(
                    "unpacked union `{object_name}` in `{path}` requires fixed packed value members"
                ));
            }
            let width = layout
                .members
                .iter()
                .filter_map(|member| member.descriptor.fixed_size_bits())
                .max()
                .and_then(|width| u32::try_from(width).ok())
                .ok_or_else(|| {
                    format!(
                        "unpacked union `{object_name}` in `{path}` has an unresolved member width"
                    )
                })?;
            let storage_two_state = layout.members.iter().all(|member| member.two_state);
            Some(self.collect_aggregate_member_signal(
                path,
                node,
                &object_name,
                &object_name,
                (width, false, storage_two_state),
            )?)
        } else {
            None
        };
        let mut leaves = Vec::new();
        for member in &layout.members {
            self.collect_aggregate_descriptor_leaves(
                path,
                node,
                &object_name,
                member,
                &member.descriptor,
                &[AggregatePathPart::Member(member.name.clone())],
                union_signal.as_ref(),
                &mut leaves,
            )?;
        }
        if leaves.is_empty() {
            return Err(format!(
                "unpacked aggregate `{object_name}` in `{path}` has no supported value leaves"
            ));
        }
        let mut members = Vec::with_capacity(layout.members.len());
        for member in &layout.members {
            let path_part = AggregatePathPart::Member(member.name.clone());
            let storage = leaves
                .iter()
                .find(|leaf| leaf.path.as_slice() == [path_part.clone()])
                .cloned();
            members.push(storage.unwrap_or_else(|| AggregateMemberInfo {
                member: member.clone(),
                signal: None,
                object: None,
                path: vec![path_part],
            }));
        }
        self.unpacked_aggregates.insert(
            node,
            UnpackedAggregateInfo {
                kind: layout.kind,
                type_identity: layout.type_identity,
                members,
                leaves,
            },
        );
        Ok(true)
    }

    /// Recursively lower a fixed non-class descriptor to owned leaf storage.
    /// This is an emission detail only; compatibility and copy policy remain
    /// governed by the recursive descriptor captured in `core::db`.
    #[allow(clippy::too_many_arguments)]
    fn collect_aggregate_descriptor_leaves(
        &mut self,
        path: &str,
        object: NodeId,
        object_name: &str,
        member: &AggregateMember,
        descriptor: &TypeDescriptor,
        member_path: &[AggregatePathPart],
        shared: Option<&SignalInfo>,
        leaves: &mut Vec<AggregateMemberInfo>,
    ) -> Result<(), String> {
        match &descriptor.shape {
            TypeShape::PackedAtom { .. } => {
                let signal = match shared {
                    Some(signal) => signal.clone(),
                    None => self.collect_aggregate_member_signal(
                        path,
                        object,
                        object_name,
                        &aggregate_path_suffix(member_path),
                        (
                            descriptor.info.width.unwrap_or_default(),
                            descriptor.info.signed,
                            member.two_state,
                        ),
                    )?,
                };
                if signal.width == 0 {
                    return Err(format!(
                        "unpacked aggregate member `{}.{}` in `{path}` has zero width",
                        object_name,
                        aggregate_path_suffix(member_path)
                    ));
                }
                leaves.push(AggregateMemberInfo {
                    member: leaf_member(member, descriptor),
                    signal: Some(signal),
                    object: None,
                    path: member_path.to_vec(),
                });
            }
            TypeShape::Real { shortreal } => {
                if shared.is_some() {
                    return Err(format!(
                        "real member in unpacked union `{object_name}` in `{path}` is not a packed overlay"
                    ));
                }
                let signal = self.collect_aggregate_member_signal(
                    path,
                    object,
                    object_name,
                    &aggregate_path_suffix(member_path),
                    (0, false, member.two_state),
                )?;
                let ir = signal.ir;
                self.model.signals[ir].ty = crate::sim::ir::IrType::Real {
                    shortreal: *shortreal,
                };
                self.signals[ir].real = true;
                self.signals[ir].shortreal = *shortreal;
                leaves.push(AggregateMemberInfo {
                    member: leaf_member(member, descriptor),
                    signal: Some(signal),
                    object: None,
                    path: member_path.to_vec(),
                });
            }
            TypeShape::String => {
                if shared.is_some() {
                    return Err(format!(
                        "string member in unpacked union `{object_name}` in `{path}` is not a packed overlay"
                    ));
                }
                let key = aggregate_path_key(member_path);
                let index = self.model.objects.len();
                self.model.objects.push(crate::sim::ir::IrObject {
                    c_name: format!(
                        "O_{}_{}_{}",
                        ident(path),
                        ident(object_name),
                        ident(&aggregate_path_suffix(member_path))
                    ),
                    ty: crate::sim::ir::IrObjectType::String,
                    initial: None,
                });
                self.aggregate_objects.insert((object, key), index);
                leaves.push(AggregateMemberInfo {
                    member: leaf_member(member, descriptor),
                    signal: None,
                    object: Some(index),
                    path: member_path.to_vec(),
                });
            }
            TypeShape::Aggregate(layout) => {
                for nested in &layout.members {
                    let mut nested_path = member_path.to_vec();
                    nested_path.push(AggregatePathPart::Member(nested.name.clone()));
                    self.collect_aggregate_descriptor_leaves(
                        path,
                        object,
                        object_name,
                        nested,
                        &nested.descriptor,
                        &nested_path,
                        shared,
                        leaves,
                    )?;
                }
            }
            TypeShape::FixedArray {
                dimensions,
                element,
            } => {
                let Some((left, right)) = dimensions.first().copied() else {
                    return Err(format!(
                        "fixed array member `{}.{}` in `{path}` has no captured bounds",
                        object_name,
                        aggregate_path_suffix(member_path)
                    ));
                };
                let rest = if dimensions.len() == 1 {
                    None
                } else {
                    Some(TypeDescriptor {
                        id: descriptor.id,
                        name: descriptor.name.clone(),
                        info: descriptor.info.clone(),
                        shape: TypeShape::FixedArray {
                            dimensions: dimensions[1..].to_vec(),
                            element: element.clone(),
                        },
                    })
                };
                let next = rest.as_ref().unwrap_or(element.as_ref());
                let mut index = left;
                loop {
                    let mut element_path = member_path.to_vec();
                    element_path.push(AggregatePathPart::Index(index));
                    self.collect_aggregate_descriptor_leaves(
                        path,
                        object,
                        object_name,
                        member,
                        next,
                        &element_path,
                        shared,
                        leaves,
                    )?;
                    if index == right {
                        break;
                    }
                    index = if left >= right {
                        index.checked_sub(1)
                    } else {
                        index.checked_add(1)
                    }
                    .ok_or_else(|| {
                        format!("fixed array bounds overflow in `{object_name}` in `{path}`")
                    })?;
                }
            }
            TypeShape::Opaque { kind } if kind == "Chandle" => {
                if shared.is_some() {
                    return Err(format!(
                        "chandle member in unpacked union `{object_name}` in `{path}` is not a packed overlay"
                    ));
                }
                let key = aggregate_path_key(member_path);
                let index = self.model.objects.len();
                self.model.objects.push(crate::sim::ir::IrObject {
                    c_name: format!(
                        "O_{}_{}_{}",
                        ident(path),
                        ident(object_name),
                        ident(&aggregate_path_suffix(member_path))
                    ),
                    ty: crate::sim::ir::IrObjectType::Chandle,
                    initial: None,
                });
                self.aggregate_objects.insert((object, key), index);
                leaves.push(AggregateMemberInfo {
                    member: leaf_member(member, descriptor),
                    signal: None,
                    object: Some(index),
                    path: member_path.to_vec(),
                });
            }
            TypeShape::Container { kind, .. } | TypeShape::Opaque { kind } => {
                return Err(format!(
                    "unpacked aggregate member `{}.{}` has unsupported recursive storage type `{kind}` in `{path}`",
                    object_name,
                    aggregate_path_suffix(member_path)
                ));
            }
        }
        Ok(())
    }

    fn collect_aggregate_member_signal(
        &mut self,
        path: &str,
        object: NodeId,
        object_name: &str,
        member_name: &str,
        packed_type: (u32, bool, bool),
    ) -> Result<SignalInfo, String> {
        let (width, signed, two_state) = packed_type;
        if width > LLG_MAX_WIDTH {
            return Err(format!(
                "unpacked aggregate storage `{object_name}` in `{path}` is {width} bits wide; the runtime maximum supported width is {LLG_MAX_WIDTH}"
            ));
        }
        let storage_name = format!("{object_name}__{member_name}");
        let global = if width == 0 {
            real_global_name(path, &storage_name)
        } else {
            global_name(path, &storage_name)
        };
        let mut hdl_name = self.waveform_name(object);
        hdl_name.push('\u{1f}');
        hdl_name.push_str(member_name);
        let ir = self.model.signals.len();
        let info = SignalInfo {
            global: global.clone(),
            width,
            signed,
            two_state,
            real: width == 0,
            shortreal: false,
            net_driver: None,
            ir,
        };
        self.model.signals.push(IrSignal {
            c_name: global,
            hdl_name: Some(hdl_name),
            ty: if width == 0 {
                IrType::Real { shortreal: false }
            } else {
                IrType::Packed {
                    width,
                    signed,
                    two_state,
                }
            },
            net_driver: None,
            alias: None,
            omit: false,
        });
        self.signals.push(info.clone());
        Ok(info)
    }

    fn collect_instance(&mut self, inst: NodeId, path: &str) -> Result<(), String> {
        let mut seen: HashSet<String> = HashSet::new();
        for c in &self.node(inst).children {
            let nid = *c;
            if self.collect_object(path, nid)? {
                continue;
            }
            if self.collect_aggregate(path, nid)? {
                continue;
            }
            match self.kind(nid) {
                NodeKind::Array { ty } => {
                    let name = self.node(nid).name.clone();
                    if name.is_empty() || !seen.insert(name.clone()) {
                        continue;
                    }
                    if !self
                        .db
                        .array_meta(nid)
                        .is_some_and(|meta| matches!(meta.kind(), ArrayKind::Static))
                    {
                        let info = self.container_info(path, &name, nid, ty)?;
                        self.container_globals.insert(nid, info);
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
                        two_state: self.db.is_two_state_type(nid) || is_two_state_kind(&ty.kind),
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
                                two_state: info.two_state,
                            }
                        },
                        net_driver: None,
                        alias: None,
                        omit: false,
                    });
                    self.signals.push(info.clone());
                    self.sig_globals.insert(nid, info.clone());
                    self.scope_sig_names
                        .entry(path.to_string())
                        .or_default()
                        .insert(name, info);
                }
                NodeKind::Param { value, .. } => {
                    if let Some(value) =
                        self.collected_parameter_value(inst, nid, value.as_ref())?
                    {
                        self.param_vals.insert(nid, value);
                    }
                }
                NodeKind::NamedEvent => {
                    self.collect_named_event(path, nid, &mut seen)?;
                }
                // A declaration initializer on an `array_net` (`reg [7:0] m
                // [0:3] = '{…}` — represented as a
                // net-decl-assign continuous assignment whose LHS is the
                // array).  Applied to the array's `ArrayInfo`; the assignment
                // itself is skipped at emission. A scalar reg initializer is
                // collected into `scalar_inits`; true nets stay available to
                // `emit_cont_assign` as continuous drivers.
                NodeKind::ContAssign { net_decl: true, .. } => {
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
                    } else if self.collect_aggregate_cont_assign_init(path, nid)? {
                        self.scalar_init_ca.insert(nid);
                    } else if matches!(self.net_decl_target(nid), NetDeclTarget::Variable) {
                        // Scalar declaration assignments are lowered after
                        // the complete scope has allocated its signals and
                        // parameters, so runtime RHS references can resolve
                        // regardless of declaration order.
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
        for child in &self.node(inst).children {
            if matches!(
                self.kind(*child),
                NodeKind::ContAssign { net_decl: true, .. }
            ) && matches!(self.net_decl_target(*child), NetDeclTarget::Variable)
                && !self.scalar_init_ca.contains(child)
            {
                self.collect_scalar_decl_init(path, *child)?;
            }
        }
        for c in &self.node(inst).children {
            match self.kind(*c) {
                NodeKind::GenScopeArray => self.collect_gen_scope_array(*c, path)?,
                NodeKind::GenScope => self.collect_gen_scope(*c, path)?,
                _ => {}
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
    /// is attached to the variable (`logic l = 1'b0;`, `int x = 5;` —
    /// captured in [`Db::vars_init`]) into a constant and queue it for
    /// `main()`.  Called after the scope's parameters are collected so
    /// `P + 1`-style RHS refs resolve via `param_vals`.  Vars that are not
    /// collected as signals (function/block locals) carry no
    /// fill; their initializers are handled by their own paths.
    fn collect_var_inits(&mut self, path: &str, inst: NodeId) -> Result<(), String> {
        self.inst = inst;
        for c in &self.node(inst).children {
            if !matches!(self.kind(*c), NodeKind::Var { .. }) {
                continue;
            }
            let init = match self.db.var_initializer(*c) {
                Some(init) => init,
                None => continue,
            };
            if let Some(aggregate) = self.unpacked_aggregates.get(c).cloned() {
                self.collect_unpacked_aggregate_decl_init(path, *c, init, &aggregate)?;
                continue;
            }
            let info = match self.signal_of(*c) {
                Some(info) => info.clone(),
                None => continue,
            };
            if let Some(layout) = self.db.aggregate_layout(*c) {
                if matches!(
                    layout.kind,
                    AggregateKind::PackedStruct | AggregateKind::PackedUnion
                ) && matches!(
                    self.kind(init),
                    NodeKind::Expr(ExprKind::Operation { op, .. })
                        if *op == Operation::AssignmentPattern
                ) {
                    let value = self.packed_aggregate_decl_init(path, init, layout, &info)?;
                    self.var_inits.push((info, value));
                    continue;
                }
            }
            let name = self.node(*c).name.clone();
            let initializer = self.lower_declaration_initializer(
                path,
                *c,
                init,
                IrInitTarget::Signal(info.ir),
                info.width,
                info.signed,
                info.two_state,
                info.real,
            );
            match initializer {
                Ok(initializer) => self.declaration_inits.push(initializer),
                Err(lowering_error) => match self.var_decl_init(path, &name, init) {
                    Ok(cconst) => self.var_inits.push((info, cconst)),
                    Err(_) => return Err(lowering_error),
                },
            }
        }
        Ok(())
    }

    fn packed_aggregate_decl_init(
        &self,
        path: &str,
        init: NodeId,
        layout: &crate::core::db::AggregateLayout,
        storage: &SignalInfo,
    ) -> Result<IrConst, String> {
        let value = self.packed_aggregate_decl_value(path, init, layout)?;
        let value = materialize_decl_cast_value(
            value,
            storage.width as usize,
            storage.signed,
            storage.two_state,
        );
        decl_value_to_const(Val::Bits(value))
    }

    fn collect_unpacked_aggregate_decl_init(
        &mut self,
        path: &str,
        object: NodeId,
        init: NodeId,
        aggregate: &UnpackedAggregateInfo,
    ) -> Result<(), String> {
        let layout = self.db.aggregate_layout(object).ok_or_else(|| {
            format!(
                "unpacked aggregate `{}` in `{path}` has no captured layout",
                self.node(object).name
            )
        })?;
        let mut values = Vec::new();
        self.aggregate_pattern_leaf_values(path, init, layout, &[], &mut values)?;
        for (member_path, value_node) in values {
            let member = aggregate
                .leaves
                .iter()
                .find(|leaf| leaf.path == member_path)
                .ok_or_else(|| {
                    format!(
                        "aggregate initializer path `{}` has no storage in `{path}`",
                        aggregate_path_suffix(&member_path)
                    )
                })?;
            if let Some(index) = member.object {
                match self.model.objects[index].ty {
                    crate::sim::ir::IrObjectType::String => {
                        self.model.objects[index].initial =
                            Some(self.lower_string(path, value_node)?);
                    }
                    crate::sim::ir::IrObjectType::Chandle => {
                        if self.lower_chandle(path, value_node)? != IrChandleExpr::Null {
                            return Err(format!(
                                "chandle aggregate initializer `{}` must be null",
                                aggregate_path_suffix(&member_path)
                            ));
                        }
                    }
                }
                continue;
            }
            let signal = member.signal.as_ref().ok_or_else(|| {
                format!(
                    "aggregate initializer path `{}` has no scalar storage in `{path}`",
                    aggregate_path_suffix(&member_path)
                )
            })?;
            if signal.real {
                let value = match self.eval_decl_value(value_node) {
                    Ok(value) => value,
                    Err(_) => {
                        let initializer = self.lower_declaration_initializer(
                            path,
                            object,
                            value_node,
                            IrInitTarget::Signal(signal.ir),
                            signal.width,
                            signal.signed,
                            signal.two_state,
                            true,
                        )?;
                        self.declaration_inits.push(initializer);
                        continue;
                    }
                };
                let value = match value {
                    Val::Real(value) => Val::Real(value),
                    Val::Bits(value) => {
                        Val::Real(elab::ieee_bits_to_real(&value).ok_or_else(|| {
                            "real aggregate initializer is not an IEEE value".to_owned()
                        })?)
                    }
                    Val::Str(_) => {
                        return Err(format!(
                            "string value cannot initialize real aggregate member `{}` in `{path}`",
                            aggregate_path_suffix(&member_path)
                        ))
                    }
                };
                self.var_inits
                    .push((signal.clone(), decl_value_to_const(value)?));
                continue;
            }
            let width = member.member.ty.width.ok_or_else(|| {
                format!(
                    "unpacked member `{}` has unresolved width in `{path}`",
                    aggregate_path_suffix(&member_path)
                )
            })?;
            match self.aggregate_member_decl_value(path, value_node, &member.member, width) {
                Ok(value) => {
                    let value = value.cast(signal.width as usize, signal.signed);
                    self.var_inits
                        .push((signal.clone(), decl_value_to_const(Val::Bits(value))?));
                }
                Err(_) => {
                    let initializer = self.lower_declaration_initializer(
                        path,
                        object,
                        value_node,
                        IrInitTarget::Signal(signal.ir),
                        signal.width,
                        signal.signed,
                        signal.two_state,
                        false,
                    )?;
                    self.declaration_inits.push(initializer);
                }
            }
        }
        Ok(())
    }

    fn aggregate_member_decl_value(
        &self,
        path: &str,
        value_node: NodeId,
        member: &AggregateMember,
        width: u32,
    ) -> Result<elab::Value, String> {
        if let Some(layout) = member.aggregate_layout() {
            if matches!(
                self.kind(value_node),
                NodeKind::Expr(ExprKind::Operation { op, .. })
                    if *op == Operation::AssignmentPattern
            ) {
                if !matches!(
                    layout.kind,
                    AggregateKind::PackedStruct | AggregateKind::PackedUnion
                ) {
                    return Err(format!(
                        "nested unpacked aggregate member `{}` in `{path}` is not supported",
                        member.name
                    ));
                }
                let value = self.packed_aggregate_decl_value(path, value_node, layout)?;
                return Ok(materialize_decl_cast_value(
                    value,
                    width as usize,
                    member.ty.signed,
                    member.two_state,
                ));
            }
        }
        if let Some(fill) = self.source_fill_literal(value_node) {
            let fill = match fill {
                0 => Bit::Zero,
                1 => Bit::One,
                2 => Bit::X,
                3 => Bit::Z,
                _ => return Err(format!("invalid aggregate fill value {fill} in `{path}`")),
            };
            return Ok(materialize_decl_cast_value(
                elab::Value {
                    bits: vec![fill],
                    signed: false,
                    fill: Some(fill),
                },
                width as usize,
                member.ty.signed,
                member.two_state,
            ));
        }
        let value = match self.eval_decl_value(value_node)? {
            Val::Bits(value) => value,
            Val::Real(value) => elab::real_to_bits(value, width as usize, member.ty.signed),
            Val::Str(_) => {
                return Err(format!(
                    "string value for aggregate member `{}` is not supported",
                    member.name
                ))
            }
        };
        Ok(materialize_decl_cast_value(
            value,
            width as usize,
            member.ty.signed,
            member.two_state,
        ))
    }

    fn packed_aggregate_decl_value(
        &self,
        path: &str,
        init: NodeId,
        layout: &crate::core::db::AggregateLayout,
    ) -> Result<elab::Value, String> {
        let values = self.aggregate_pattern_values(path, init, layout)?;
        let mut members = Vec::with_capacity(values.len());
        for (member_index, value_node) in values {
            let member = layout.members.get(member_index).ok_or_else(|| {
                format!("aggregate initializer member index {member_index} is out of bounds")
            })?;
            let width = member.ty.width.ok_or_else(|| {
                format!(
                    "packed member `{}` has unresolved width in `{path}`",
                    member.name
                )
            })?;
            members.push(self.aggregate_member_decl_value(path, value_node, member, width)?);
        }
        if layout.kind == AggregateKind::PackedUnion {
            members
                .into_iter()
                .next()
                .ok_or_else(|| format!("packed union assignment pattern is empty in `{path}`"))
        } else {
            Ok(elab::concat(&members))
        }
    }

    pub(super) fn aggregate_pattern_values(
        &self,
        path: &str,
        init: NodeId,
        layout: &crate::core::db::AggregateLayout,
    ) -> Result<Vec<(usize, NodeId)>, String> {
        let NodeKind::Expr(ExprKind::Operation {
            op,
            operands,
            reordered,
            ..
        }) = self.kind(init)
        else {
            return Err(format!(
                "declaration initializer for aggregate in `{path}` is not an assignment pattern"
            ));
        };
        if *op != Operation::AssignmentPattern {
            return Err(format!(
                "declaration initializer for aggregate in `{path}` is not an assignment pattern"
            ));
        }
        let mut operands = operands.clone();
        if *reordered {
            operands.reverse();
        }
        let tagged = operands.iter().any(|operand| {
            matches!(
                self.kind(*operand),
                NodeKind::Expr(ExprKind::TaggedPattern { .. })
            )
        });
        let is_union = matches!(
            layout.kind,
            AggregateKind::PackedUnion | AggregateKind::UnpackedUnion
        );
        if !tagged {
            let expected = if is_union { 1 } else { layout.members.len() };
            if operands.len() != expected {
                return Err(format!(
                    "aggregate assignment pattern in `{path}` has {} positional values; expected {expected}",
                    operands.len()
                ));
            }
            return Ok(operands.into_iter().enumerate().collect());
        }
        if operands.iter().any(|operand| {
            !matches!(
                self.kind(*operand),
                NodeKind::Expr(ExprKind::TaggedPattern { .. })
            )
        }) {
            if !is_union && operands.len() == layout.members.len() {
                // The elaborated snapshot replaces resolved member/default
                // keys with their values but retains resolved type keys as
                // tagged operands, all in declaration order.
                return operands
                    .into_iter()
                    .enumerate()
                    .map(|(index, operand)| match self.kind(operand) {
                        NodeKind::Expr(ExprKind::TaggedPattern {
                            value: Some(value),
                            ..
                        }) => Ok((index, *value)),
                        NodeKind::Expr(ExprKind::TaggedPattern { .. }) => Err(format!(
                            "flattened aggregate assignment pattern operand {index} has no value in `{path}`"
                        )),
                        _ => Ok((index, operand)),
                    })
                    .collect();
            }
            return Err(format!(
                "mixed positional and keyed aggregate assignment pattern in `{path}` is not supported"
            ));
        }

        let mut explicit = vec![None; layout.members.len()];
        let mut type_values: Vec<(String, AssignmentPatternKeyType, NodeId)> = Vec::new();
        let mut default = None;
        for operand in operands {
            let NodeKind::Expr(ExprKind::TaggedPattern {
                key,
                key_type,
                value,
            }) = self.kind(operand)
            else {
                continue;
            };
            let key = key.as_deref().ok_or_else(|| {
                format!("aggregate assignment pattern key is unavailable in `{path}`")
            })?;
            let value = value.ok_or_else(|| {
                format!("aggregate assignment pattern key `{key}` has no value in `{path}`")
            })?;
            if key == "default" {
                if default.replace(value).is_some() {
                    return Err(format!(
                        "duplicate default key in aggregate assignment pattern in `{path}`"
                    ));
                }
                continue;
            }
            if let Some(index) = layout.members.iter().position(|member| member.name == key) {
                if explicit[index].replace(value).is_some() {
                    return Err(format!(
                        "duplicate aggregate member key `{key}` in `{path}`"
                    ));
                }
                continue;
            }
            let Some(key_type) = key_type else {
                return Err(format!(
                    "aggregate assignment pattern key `{key}` has no matching member or type in `{path}`"
                ));
            };
            if !layout
                .members
                .iter()
                .any(|member| aggregate_member_matches_type_key(member, key, Some(key_type)))
            {
                return Err(format!(
                    "aggregate assignment pattern key `{key}` has no matching member or type in `{path}`"
                ));
            }
            if type_values
                .iter()
                .any(|(_, previous, _)| pattern_key_types_equal(previous, key_type))
            {
                return Err(format!("duplicate aggregate type key `{key}` in `{path}`"));
            }
            type_values.push((key.to_owned(), key_type.clone(), value));
        }

        let resolved = layout
            .members
            .iter()
            .enumerate()
            .filter_map(|(index, member)| {
                explicit[index]
                    .or_else(|| {
                        type_values.iter().rev().find_map(|(key, key_type, value)| {
                            aggregate_member_matches_type_key(member, key, Some(key_type))
                                .then_some(*value)
                        })
                    })
                    .or(default)
                    .map(|value| (index, value))
            })
            .collect::<Vec<_>>();
        if is_union {
            if resolved.len() != 1 {
                return Err(format!(
                    "untagged union assignment pattern in `{path}` must select exactly one member"
                ));
            }
        } else if resolved.len() != layout.members.len() {
            let missing = layout
                .members
                .iter()
                .enumerate()
                .find(|(index, _)| !resolved.iter().any(|(set, _)| set == index))
                .map(|(_, member)| member.name.as_str())
                .unwrap_or("<unknown>");
            return Err(format!(
                "aggregate assignment pattern in `{path}` does not cover member `{missing}`"
            ));
        }
        Ok(resolved)
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
            "int" | "integer" | "time" | "longint" | "byte" | "shortint" | "logic" | "reg"
            | "bit" | "enum" => ty.width.unwrap_or(1),
            "struct" | "union" | "array" => ty.width.ok_or_else(|| {
                format!("packed type of signal `{name}` in `{path}` has no resolved width")
            })?,
            _ => {
                return Err(format!(
                    "unsupported typespec type for signal `{name}` in `{path}`"
                ))
            }
        };
        if w > LLG_MAX_WIDTH {
            return Err(format!(
                "signal `{name}` in `{path}` is {w} bits wide; the runtime \
                 maximum supported width is {LLG_MAX_WIDTH}"
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
        // An enclosing generate-scope array can hold the iteration name (for
        // example `g[0]`) while the generate scope itself is unnamed;
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
            if self.collect_object(&gs_path, nid)? {
                continue;
            }
            if self.collect_aggregate(&gs_path, nid)? {
                continue;
            }
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
                        two_state: self.db.is_two_state_type(nid) || is_two_state_kind(&ty.kind),
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
                                two_state: info.two_state,
                            }
                        },
                        net_driver: None,
                        alias: None,
                        omit: false,
                    });
                    self.signals.push(info.clone());
                    self.sig_globals.insert(nid, info.clone());
                    self.scope_sig_names
                        .entry(gs_path.clone())
                        .or_default()
                        .insert(name, info);
                }
                NodeKind::Param { value, .. } => {
                    if let Some(value) = self.collected_parameter_value(gs, nid, value.as_ref())? {
                        self.param_vals.insert(nid, value);
                    }
                }
                NodeKind::NamedEvent => {
                    self.collect_named_event(&gs_path, nid, &mut gseen)?;
                }
                NodeKind::ContAssign { net_decl: true, .. } => {
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
                    } else if self.collect_aggregate_cont_assign_init(&gs_path, nid)? {
                        self.scalar_init_ca.insert(nid);
                    } else if matches!(self.net_decl_target(nid), NetDeclTarget::Variable) {
                        // See the module-instance path above: defer scalar
                        // declaration assignments until this scope's
                        // storage and parameters are complete.
                    }
                }
                _ => {}
            }
        }
        // Variable declaration initializers, folded after the scope's own
        // parameters are collected (see `collect_var_inits`).
        self.collect_var_inits(&gs_path, gs)?;
        for child in &self.node(gs).children {
            if matches!(
                self.kind(*child),
                NodeKind::ContAssign { net_decl: true, .. }
            ) && matches!(self.net_decl_target(*child), NetDeclTarget::Variable)
                && !self.scalar_init_ca.contains(child)
            {
                self.collect_scalar_decl_init(&gs_path, *child)?;
            }
        }
        for child in self.node(gs).children.clone() {
            match self.kind(child) {
                NodeKind::GenScope => self.collect_gen_scope(child, &gs_path)?,
                NodeKind::GenScopeArray => self.collect_gen_scope_array(child, &gs_path)?,
                _ => {}
            }
        }
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

    /// Collapse inout-port net pairs (parent high connection + child low
    /// connection) into one resolved simulated net per connected set
    /// (LRM §23.3.3.7), run after [`collect_design`](Self::collect_design)
    /// and before any emission.
    ///
    /// Every grouped member's `SignalInfo` is redirected to the shared
    /// `llg_net_t`'s `resolved` cell and tagged with its driver slot, so
    /// reads/writes/sensitivity all use the resolution cell automatically.
    /// Groups with anything the runtime cannot resolve (non-net members,
    /// mixed widths, unsupported net types, dynamic/NBA/task-actual writes)
    /// reject code generation rather than disconnecting the net group.
    fn net_propagation_delay_for_members(
        &mut self,
        members: &[NodeId],
        shown: &str,
    ) -> Result<Option<crate::sim::ir::IrTransitionDelay>, String> {
        let mut selected = None;
        for &member in members {
            let Some(delay) = self.db.net_delay(member) else {
                continue;
            };
            let previous_inst = self.inst;
            if let Some(instance) = self.owning_inst(member) {
                self.inst = instance;
            }
            let result = self.driver_delay_ticks(member, delay);
            self.inst = previous_inst;
            let converted = result.map_err(|error| {
                format!(
                    "net propagation delay for `{shown}` at {}:{}:{}: {error}",
                    self.node(member).file.as_deref().unwrap_or("<unknown>"),
                    self.node(member).line,
                    self.node(member).col,
                )
            })?;
            if let Some(existing) = selected {
                if existing != converted {
                    return Err(format!(
                        "net propagation delay for `{shown}` has conflicting member delays at {}:{}:{}",
                        self.node(member).file.as_deref().unwrap_or("<unknown>"),
                        self.node(member).line,
                        self.node(member).col,
                    ));
                }
            } else {
                selected = Some(converted);
            }
        }
        Ok(selected)
    }

    pub(super) fn build_net_groups(&mut self) -> Result<(), String> {
        let nodes = self.design_nodes();
        // Union-find over the parent/child nets of every inout port.
        let mut parent: HashMap<NodeId, NodeId> = HashMap::new();
        let mut rank: HashMap<NodeId, u8> = HashMap::new();
        let mut inout_ports: Vec<NodeId> = Vec::new();
        for id in &nodes {
            if let NodeKind::Port {
                direction: DbDirection::Inout,
                high: Some(h),
                low: Some(l),
                ..
            } = self.kind(*id)
            {
                if let NodeKind::Port {
                    high_expr: Some(actual),
                    ..
                } = self.kind(*id)
                {
                    let whole_actual = *actual == *h
                        || matches!(
                            self.kind(*actual),
                            NodeKind::Expr(ExprKind::Ref {
                                target: Some(target)
                            }) if *target == *h
                        )
                        || self
                            .hier_path_signal(*actual)
                            .zip(self.signal_of(*h))
                            .is_some_and(|(actual, target)| actual.ir == target.ir);
                    if !whole_actual {
                        return Err(format!(
                            "inout port `{}` has a selected or concatenated actual that cannot be resolved safely at {}:{}:{}",
                            self.node(*id).name,
                            self.node(*id).file.as_deref().unwrap_or("<unknown>"),
                            self.node(*id).line,
                            self.node(*id).col,
                        ));
                    }
                }
                inout_ports.push(*id);
                union(&mut parent, &mut rank, *h, *l);
            } else if let NodeKind::Port {
                direction: DbDirection::Inout,
                high: None,
                ..
            } = self.kind(*id)
            {
                // A top-level inout port has no parent-side connection; there
                // is nothing to collapse, so it stays a plain net (no link is
                // ever emitted for top-level ports).
            } else if let NodeKind::Port {
                direction: DbDirection::Inout,
                high: Some(_),
                low: None,
                ..
            } = self.kind(*id)
            {
                return Err(format!(
                    "inout port `{}` has no child-side connection at {}:{}:{}",
                    self.node(*id).name,
                    self.node(*id).file.as_deref().unwrap_or("<unknown>"),
                    self.node(*id).line,
                    self.node(*id).col,
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
                return Err(format!(
                    "{joined}: member `{}` is not a net at {}:{}:{}",
                    self.display_name(*bad),
                    self.node(*bad).file.as_deref().unwrap_or("<unknown>"),
                    self.node(*bad).line,
                    self.node(*bad).col,
                ));
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
                return Err(format!(
                    "{joined}: member `{}` has a different width than `{}` at {}:{}:{}",
                    self.display_name(*bad),
                    self.display_name(members[0]),
                    self.node(*bad).file.as_deref().unwrap_or("<unknown>"),
                    self.node(*bad).line,
                    self.node(*bad).col,
                ));
            }
            // 3. Every member contributes to one resolution family. Net and
            // tri spellings share wire resolution, while wired and biased
            // spellings retain their own canonical resolver.
            let kind = match self.kind(members[0]) {
                NodeKind::Net { net_type, .. } => Self::ir_net_kind(*net_type),
                _ => None,
            };
            let Some(kind) = kind else {
                return Err(format!(
                    "{joined}: member `{}` has unsupported net type for an executable inout \
                     connection",
                    self.display_name(members[0])
                ));
            };
            if let Some(bad) = members
                .iter()
                .skip(1)
                .find(|member| match self.kind(**member) {
                    NodeKind::Net { net_type, .. } => Self::ir_net_kind(*net_type) != Some(kind),
                    _ => true,
                })
            {
                return Err(format!(
                    "{joined}: member `{}` has an incompatible net type at {}:{}:{}",
                    self.display_name(*bad),
                    self.node(*bad).file.as_deref().unwrap_or("<unknown>"),
                    self.node(*bad).line,
                    self.node(*bad).col,
                ));
            }
            // 4. The runtime struct has a fixed driver-slot array.
            if members.len() > LLG_MAX_NET_DRIVERS {
                return Err(format!(
                    "{joined}: {}-member group exceeds the {LLG_MAX_NET_DRIVERS} driver-slot limit at {}:{}:{}",
                    members.len(),
                    self.node(members[0]).file.as_deref().unwrap_or("<unknown>"),
                    self.node(members[0]).line,
                    self.node(members[0]).col,
                ));
            }
            // 5. Constant packed selects are admitted as masked contributions
            //    to the member's dedicated slot. Dynamic selects, NBA writes
            //    and task `sv4_t*` actuals would bypass the resolution cell.
            if let Some(reason) = self.unsupported_member_write(members) {
                return Err(format!(
                    "{joined}: {reason} at {}:{}:{}",
                    self.node(members[0]).file.as_deref().unwrap_or("<unknown>"),
                    self.node(members[0]).line,
                    self.node(members[0]).col,
                ));
            }
            let propagation_delay = self.net_propagation_delay_for_members(members, &joined)?;

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
                kind,
                n_drivers: members.len(),
                driver_strengths: vec![(6, 6); members.len()],
                propagation_delay,
            });
            for member in members {
                let Some(signal) = self.sig_globals.get(member).map(|info| info.ir) else {
                    continue;
                };
                let id = self.record_structural_driver(signal)?;
                self.structural_driver_sites.insert((*member, gidx), id);
            }
            let sources = self.structural_site_sources(members)?;
            for (source, strengths) in sources {
                let signal = self.add_structural_driver(gidx, source, strengths)?;
                if matches!(self.kind(source), NodeKind::ContAssign { .. }) {
                    self.wired_driver_sites.insert(source, signal);
                }
            }
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
        self.build_wired_net_groups(&nodes)?;
        Ok(())
    }

    fn add_structural_driver(
        &mut self,
        group: usize,
        source: NodeId,
        strengths: (u8, u8),
    ) -> Result<usize, String> {
        self.add_structural_driver_for_terminal(group, source, strengths, 0)
    }

    /// Add a canonical contribution slot for one primitive output terminal.
    /// Terminal zero uses the original source/group identity; later outputs
    /// get an independent slot when they share that resolved group.
    fn add_structural_driver_for_terminal(
        &mut self,
        group: usize,
        source: NodeId,
        strengths: (u8, u8),
        terminal: usize,
    ) -> Result<usize, String> {
        let existing = if terminal == 0 {
            self.structural_driver_sites.get(&(source, group))
        } else {
            self.structural_driver_terminal_sites
                .get(&(source, group, terminal))
        };
        if let Some(signal) = existing {
            return self
                .structural_drivers
                .get(signal.0 as usize)
                .map(|record| record.signal)
                .ok_or_else(|| format!("structural driver {} has no recorded signal", signal.0));
        }
        let net = self
            .model
            .net_groups
            .get_mut(group)
            .ok_or_else(|| format!("structural driver references missing net group {group}"))?;
        let slot = net.n_drivers;
        if slot >= LLG_MAX_NET_DRIVERS {
            return Err(format!(
                "resolved net `{}` has too many structural drivers (limit {})",
                net.c_name, LLG_MAX_NET_DRIVERS
            ));
        }
        net.n_drivers += 1;
        net.driver_strengths.push(strengths);
        let c_name = net.c_name.clone();
        let width = net.width;
        let signed = net.signed;
        let signal = self.model.signals.len();
        self.model.signals.push(IrSignal {
            c_name: format!("{c_name}.resolved"),
            hdl_name: None,
            ty: IrType::Packed {
                width,
                signed,
                two_state: false,
            },
            net_driver: Some((group, slot)),
            alias: None,
            omit: false,
        });
        let id = self.record_structural_driver(signal)?;
        if terminal == 0 {
            self.structural_driver_sites.insert((source, group), id);
        } else {
            self.structural_driver_terminal_sites
                .insert((source, group, terminal), id);
        }
        Ok(signal)
    }

    fn record_structural_driver(&mut self, signal: usize) -> Result<DriverId, String> {
        let id = DriverId(
            u32::try_from(self.structural_drivers.len())
                .map_err(|_| "structural driver id space exhausted".to_string())?,
        );
        self.structural_drivers
            .push(StructuralDriverRecord { signal });
        Ok(id)
    }

    fn structural_driver_signal(&self, source: NodeId, group: usize) -> Option<usize> {
        self.structural_driver_sites
            .get(&(source, group))
            .and_then(|id| self.structural_drivers.get(id.0 as usize))
            .map(|record| record.signal)
    }

    fn structural_driver_signal_for_terminal(
        &self,
        source: NodeId,
        group: usize,
        terminal: usize,
    ) -> Option<usize> {
        if terminal == 0 {
            return self.structural_driver_signal(source, group);
        }
        self.structural_driver_terminal_sites
            .get(&(source, group, terminal))
            .and_then(|id| self.structural_drivers.get(id.0 as usize))
            .map(|record| record.signal)
    }

    fn has_structural_driver(&self, source: NodeId) -> bool {
        self.structural_driver_sites
            .keys()
            .any(|(candidate, _)| *candidate == source)
    }

    /// Resolve the strength of an output-port link from the port metadata and
    /// its child-side net declaration.  Slang attaches a net declaration's
    /// drive strength to that child net (the port symbol itself has no
    /// independent drive-strength syntax), while a port can still carry
    /// explicit metadata in a future frontend snapshot.  Keep the fallback
    /// strong/strong so variable outputs and ordinary implicit nets retain
    /// the default structural-driver strength.
    fn effective_port_driver_strengths(
        &self,
        port: NodeId,
        explicit0: Strength,
        explicit1: Strength,
        low: Option<NodeId>,
    ) -> Result<(u8, u8), String> {
        let (strength0, strength1) =
            if explicit0 == Strength::Unspecified && explicit1 == Strength::Unspecified {
                match low.map(|id| self.kind(id)) {
                    Some(NodeKind::Net {
                        strength0,
                        strength1,
                        ..
                    }) => (*strength0, *strength1),
                    _ => (explicit0, explicit1),
                }
            } else {
                (explicit0, explicit1)
            };
        port_driver_strengths(strength0, strength1, &self.display_name(port))
    }

    #[allow(clippy::type_complexity)]
    fn structural_site_sources(
        &self,
        members: &[NodeId],
    ) -> Result<Vec<(NodeId, (u8, u8))>, String> {
        let member_set: HashSet<NodeId> = members.iter().copied().collect();
        let mut sources: HashMap<NodeId, (u8, u8)> = HashMap::new();
        for id in self.design_nodes() {
            match self.kind(id) {
                NodeKind::ContAssign {
                    strength0,
                    strength1,
                    ..
                } => {
                    let Some(lhs) = self.node(id).children.first().copied() else {
                        continue;
                    };
                    if self.nested_member_target(lhs, &member_set).is_some() {
                        let width = self
                            .sig_globals
                            .get(&members[0])
                            .map(|info| info.width)
                            .unwrap_or(1);
                        let strengths = continuous_assignment_strengths_for_width(
                            *strength0,
                            *strength1,
                            &self.display_name(id),
                            width,
                        )?;
                        sources.insert(id, strengths);
                    }
                }
                NodeKind::Gate {
                    prim_type,
                    strength0,
                    strength1,
                    terms,
                    ..
                } => {
                    if terms.iter().any(|term| {
                        matches!(term.direction, DbDirection::Output | DbDirection::Inout)
                            && self.nested_member_target(term.expr, &member_set).is_some()
                    }) {
                        let strengths = gate_driver_strengths(
                            *prim_type,
                            *strength0,
                            *strength1,
                            &self.display_name(id),
                        )?;
                        sources.entry(id).or_insert(strengths);
                    }
                }
                NodeKind::Port {
                    direction,
                    high,
                    low,
                    high_expr,
                    strength0,
                    strength1,
                    ..
                } => {
                    let target = match direction {
                        DbDirection::Input => *low,
                        DbDirection::Output => high_expr.or(*high),
                        _ => None,
                    };
                    if target.is_some_and(|target| {
                        self.nested_member_target(target, &member_set).is_some()
                    }) {
                        let strengths =
                            self.effective_port_driver_strengths(id, *strength0, *strength1, *low)?;
                        sources.entry(id).or_insert(strengths);
                    }
                }
                _ => {}
            }
        }
        let mut sources = sources.into_iter().collect::<Vec<_>>();
        sources.sort_by_key(|(source, _)| source.index());
        Ok(sources)
    }

    fn ir_net_kind(net_type: NetType) -> Option<crate::sim::ir::IrNetKind> {
        match net_type {
            NetType::Wire | NetType::Tri | NetType::Uwire | NetType::Logic => {
                Some(crate::sim::ir::IrNetKind::Wire)
            }
            NetType::Wand | NetType::TriAnd => Some(crate::sim::ir::IrNetKind::Wand),
            NetType::Wor | NetType::TriOr => Some(crate::sim::ir::IrNetKind::Wor),
            NetType::Tri0 => Some(crate::sim::ir::IrNetKind::Tri0),
            NetType::Tri1 => Some(crate::sim::ir::IrNetKind::Tri1),
            NetType::Supply0 => Some(crate::sim::ir::IrNetKind::Supply0),
            NetType::Supply1 => Some(crate::sim::ir::IrNetKind::Supply1),
            NetType::TriReg | NetType::Reg | NetType::None | NetType::Unsupported => None,
        }
    }

    /// Build one resolved group per standalone scalar wire, tri, or wired net.
    /// Unlike collapsed inout groups, driver identity belongs to the
    /// continuous-assignment site, not to the declaration: two `assign w =
    /// ...` statements must retain two contributions even though both target
    /// the same net object. Ordinary nets participating in module ports or
    /// interfaces stay on the existing link/inout path.
    fn build_wired_net_groups(&mut self, nodes: &[NodeId]) -> Result<(), String> {
        let inout_members: HashSet<NodeId> = nodes
            .iter()
            .filter_map(|id| match self.kind(*id) {
                NodeKind::Port {
                    direction: DbDirection::Inout,
                    high: Some(high),
                    low: Some(low),
                    ..
                } => Some([*high, *low]),
                _ => None,
            })
            .flatten()
            .collect();
        if let Some(array) = nodes.iter().find(|id| {
            matches!(self.kind(**id), NodeKind::Array { .. })
                && self.db.array_meta(**id).is_some_and(|meta| {
                    matches!(
                        meta.net_type(),
                        Some(
                            NetType::Wand
                                | NetType::TriAnd
                                | NetType::Wor
                                | NetType::TriOr
                                | NetType::Tri0
                                | NetType::Tri1
                                | NetType::Supply0
                                | NetType::Supply1
                        )
                    )
                })
        }) {
            return Err(format!(
                "unpacked wired-net array `{}` is not supported",
                self.display_name(*array)
            ));
        }
        let standalone: Vec<(NodeId, crate::sim::ir::IrNetKind)> = nodes
            .iter()
            .filter_map(|id| match self.kind(*id) {
                NodeKind::Net { net_type, .. } => match net_type {
                    NetType::Wire | NetType::Tri | NetType::Uwire | NetType::Logic => {
                        let in_interface = self.node(*id).parent.is_some_and(|parent| {
                            matches!(
                                self.kind(parent),
                                NodeKind::ModuleInst {
                                    is_interface: true,
                                    ..
                                }
                            )
                        });
                        let already_collapsed = self
                            .sig_globals
                            .get(id)
                            .is_some_and(|info| self.model.signals[info.ir].net_driver.is_some());
                        (!in_interface && !already_collapsed && !inout_members.contains(id))
                            .then_some((*id, crate::sim::ir::IrNetKind::Wire))
                    }
                    NetType::Wand | NetType::TriAnd => (!inout_members.contains(id))
                        .then_some((*id, crate::sim::ir::IrNetKind::Wand)),
                    NetType::Wor | NetType::TriOr => (!inout_members.contains(id))
                        .then_some((*id, crate::sim::ir::IrNetKind::Wor)),
                    NetType::Tri0 => (!inout_members.contains(id))
                        .then_some((*id, crate::sim::ir::IrNetKind::Tri0)),
                    NetType::Tri1 => (!inout_members.contains(id))
                        .then_some((*id, crate::sim::ir::IrNetKind::Tri1)),
                    NetType::Supply0 => (!inout_members.contains(id))
                        .then_some((*id, crate::sim::ir::IrNetKind::Supply0)),
                    NetType::Supply1 => (!inout_members.contains(id))
                        .then_some((*id, crate::sim::ir::IrNetKind::Supply1)),
                    _ => None,
                },
                _ => None,
            })
            .collect();

        for (net, kind) in standalone {
            let shown = self.display_name(net);
            let member_set = HashSet::from([net]);
            if self.node(net).parent.is_some_and(|parent| {
                matches!(
                    self.kind(parent),
                    NodeKind::ModuleInst {
                        is_interface: true,
                        ..
                    }
                )
            }) {
                return Err(format!(
                    "wired net `{shown}` declared in an interface is not supported"
                ));
            }
            let mut sites: HashMap<NodeId, (u8, u8)> = HashMap::new();
            for id in nodes {
                match self.kind(*id) {
                    NodeKind::ContAssign {
                        strength0,
                        strength1,
                        ..
                    } => {
                        let Some(lhs) = self.node(*id).children.first().copied() else {
                            continue;
                        };
                        if matches!(self.kind(lhs), NodeKind::Expr(ExprKind::HierPath { .. }))
                            && self.member_write_base(lhs, &member_set).is_some()
                        {
                            return Err(format!(
                                "hierarchical continuous assignment to wired net `{shown}` is not supported"
                            ));
                        }
                        if self.cont_assign_source_has_hier_lhs(*id, net) {
                            return Err(format!(
                                "hierarchical continuous assignment to wired net `{shown}` is not supported"
                            ));
                        }
                        match self.member_write_kind(lhs, &member_set) {
                            MemberWrite::None => {
                                if matches!(
                                    self.kind(lhs),
                                    NodeKind::Expr(ExprKind::HierPath { .. })
                                ) {
                                    let width = self
                                        .sig_globals
                                        .get(&net)
                                        .map(|info| info.width)
                                        .unwrap_or(1);
                                    let strengths = continuous_assignment_strengths_for_width(
                                        *strength0, *strength1, &shown, width,
                                    )?;
                                    sites.insert(*id, strengths);
                                } else if self.nested_member_target(lhs, &member_set).is_some() {
                                    if !matches!(kind, crate::sim::ir::IrNetKind::Wire) {
                                        return Err(format!(
                                            "concatenated/complex continuous-assignment LHS containing wired net `{shown}` is not supported"
                                        ));
                                    }
                                    let width = self
                                        .sig_globals
                                        .get(&net)
                                        .map(|info| info.width)
                                        .unwrap_or(1);
                                    let strengths = continuous_assignment_strengths_for_width(
                                        *strength0, *strength1, &shown, width,
                                    )?;
                                    sites.insert(*id, strengths);
                                }
                            }
                            MemberWrite::Whole => {
                                let width = self
                                    .sig_globals
                                    .get(&net)
                                    .map(|info| info.width)
                                    .unwrap_or(1);
                                sites.insert(
                                    *id,
                                    continuous_assignment_strengths_for_width(
                                        *strength0, *strength1, &shown, width,
                                    )?,
                                );
                            }
                            MemberWrite::Select => {
                                let width = self
                                    .sig_globals
                                    .get(&net)
                                    .map(|info| info.width)
                                    .unwrap_or(1);
                                let strengths = continuous_assignment_strengths_for_width(
                                    *strength0, *strength1, &shown, width,
                                )?;
                                sites.insert(*id, strengths);
                            }
                        }
                    }
                    NodeKind::Stmt(StmtKind::Assign { blocking, .. }) => {
                        let Some(lhs) = self.node(*id).children.first().copied() else {
                            continue;
                        };
                        if self.nested_member_target(lhs, &member_set).is_some() {
                            let form = if *blocking { "blocking" } else { "nonblocking" };
                            return Err(format!(
                                "procedural {form} assignment to wired net `{shown}` is not supported"
                            ));
                        }
                    }
                    NodeKind::Stmt(StmtKind::ProcContAssign { lhs, .. }) => {
                        if self.nested_member_target(*lhs, &member_set).is_some() {
                            if matches!(kind, crate::sim::ir::IrNetKind::Wire) {
                                return Err(format!(
                                    "procedural continuous assignment targets variables only; resolved net `{shown}` is not supported"
                                ));
                            }
                            return Err(format!(
                                "procedural continuous assignment to wired net `{shown}` is not supported"
                            ));
                        }
                    }
                    NodeKind::Stmt(StmtKind::Deassign { lhs }) => {
                        if self.nested_member_target(*lhs, &member_set).is_some() {
                            return Err(format!(
                                "procedural deassign of wired net `{shown}` is not supported"
                            ));
                        }
                    }
                    NodeKind::Stmt(StmtKind::Force { .. })
                    | NodeKind::Stmt(StmtKind::Release { .. }) => {}
                    NodeKind::Gate {
                        prim_type,
                        strength0,
                        strength1,
                        terms,
                        ..
                    } => {
                        if terms.iter().any(|term| {
                            matches!(term.direction, DbDirection::Output | DbDirection::Inout)
                                && self.nested_member_target(term.expr, &member_set).is_some()
                        }) {
                            let strengths = gate_driver_strengths(
                                *prim_type,
                                *strength0,
                                *strength1,
                                &self.display_name(*id),
                            )?;
                            sites.insert(*id, strengths);
                        }
                    }
                    NodeKind::Port {
                        direction,
                        high,
                        low,
                        high_expr,
                        strength0,
                        strength1,
                        ..
                    } => {
                        let target = match direction {
                            DbDirection::Input => *low,
                            DbDirection::Output => high_expr.or(*high),
                            _ => None,
                        };
                        if target.is_some_and(|target| {
                            self.nested_member_target(target, &member_set).is_some()
                        }) {
                            sites.insert(
                                *id,
                                self.effective_port_driver_strengths(
                                    *id, *strength0, *strength1, *low,
                                )?,
                            );
                        }
                    }
                    NodeKind::FuncCall { .. }
                        if self.task_actual_member_write(*id, &member_set).is_some() =>
                    {
                        return Err(format!(
                            "function/task output/inout driving wired net `{shown}` is not supported"
                        ));
                    }
                    _ => {}
                }
            }
            let mut sites = sites.into_iter().collect::<Vec<_>>();
            sites.sort_by_key(|(id, _)| id.0);
            if sites.len() > LLG_MAX_NET_DRIVERS {
                return Err(format!(
                    "wired net `{shown}` has {} continuous driver sites, exceeding the {LLG_MAX_NET_DRIVERS} driver-slot limit",
                    sites.len()
                ));
            }

            let info = self
                .sig_globals
                .get(&net)
                .cloned()
                .ok_or_else(|| format!("wired net `{shown}` has no lowered scalar storage"))?;
            if info.real {
                return Err(format!("real-valued wired net `{shown}` is not supported"));
            }
            let propagation_delay =
                self.net_propagation_delay_for_members(std::slice::from_ref(&net), &shown)?;
            let name = format!("g_net_{}", self.model.net_groups.len());
            let group = self.model.net_groups.len();
            let initial_drivers = usize::from(sites.is_empty());
            self.model.net_groups.push(crate::sim::ir::IrNetGroup {
                c_name: name.clone(),
                width: info.width,
                signed: info.signed,
                kind,
                n_drivers: initial_drivers,
                driver_strengths: vec![(6, 6); initial_drivers],
                propagation_delay,
            });

            let old_global = info.global;
            let resolved = format!("{name}.resolved");
            if let Some(mapped) = self.sig_globals.get_mut(&net) {
                mapped.global = resolved.clone();
                mapped.net_driver = Some((name.clone(), 0));
            }
            if let Some(signal) = self.model.signals.get_mut(info.ir) {
                signal.c_name = resolved.clone();
                signal.net_driver = Some((group, 0));
            }
            for signal in &mut self.signals {
                if signal.global == old_global {
                    signal.global = resolved.clone();
                    signal.net_driver = Some((name.clone(), 0));
                }
            }
            for names in self.scope_sig_names.values_mut() {
                for signal in names.values_mut() {
                    if signal.global == old_global {
                        signal.global = resolved.clone();
                        signal.net_driver = Some((name.clone(), 0));
                    }
                }
            }

            if sites.is_empty() {
                let id = self.record_structural_driver(info.ir)?;
                self.structural_driver_sites.insert((net, group), id);
            }

            for (source, strengths) in sites {
                let signal = self.add_structural_driver(group, source, strengths)?;
                if matches!(self.kind(source), NodeKind::ContAssign { .. }) {
                    self.wired_driver_sites.insert(source, signal);
                }
            }
        }
        Ok(())
    }

    /// Use admitted source text only as a rejection fallback for an unresolved
    /// top-self hierarchical continuous LHS. Accepted direct drivers still
    /// require an owned target identity.
    fn cont_assign_source_has_hier_lhs(&self, ca: NodeId, net: NodeId) -> bool {
        let node = self.node(ca);
        let Some(file) = node.file.as_deref() else {
            return false;
        };
        if node.line == 0 {
            return false;
        }
        let Some(source) = self.db.source_text(file) else {
            return false;
        };
        let Some(line) = source.lines().nth(node.line as usize - 1) else {
            return false;
        };
        let lhs = line.split('=').next().unwrap_or(line);
        lhs.contains(&format!(".{}", self.node(net).name))
    }

    /// Every arena node of the instance tree (top instances + children +
    /// generate scopes), depth-first, in deterministic order.
    pub(super) fn design_nodes(&self) -> Vec<NodeId> {
        fn walk(db: &Db, id: NodeId, out: &mut Vec<NodeId>) {
            out.push(id);
            for c in &db.node(id).children {
                walk(db, *c, out);
            }
        }
        let mut out = Vec::new();
        for top in self.db.tops() {
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
                if self.is_runtime_environment(p) {
                    let namespace = if self.is_compilation_unit(p) {
                        "$unit".to_string()
                    } else {
                        strip_lib(&self.node(p).name)
                    };
                    return format!("{namespace}::{}", node.name);
                }
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
    pub(super) fn waveform_name_for(&self, id: NodeId) -> String {
        self.waveform_name(id)
    }

    fn waveform_name(&self, id: NodeId) -> String {
        const SEPARATOR: &str = "\u{1f}";

        let root_name = match self.kind(id) {
            NodeKind::ModuleInst { is_top: true, .. } => strip_lib(&self.node(id).name),
            _ => self.node(id).name.clone(),
        };
        let mut parts = vec![root_name];
        let mut current = self.node(id).parent;
        while let Some(scope_id) = current {
            let scope = self.node(scope_id);
            if matches!(
                scope.kind,
                NodeKind::ModuleInst { .. } | NodeKind::GenScopeArray | NodeKind::GenScope
            ) {
                // A frontend can library-qualify top design units (`work@tb`).
                // Other name components are source identifiers, where
                // `@` is legal in an escaped spelling and must be preserved.
                let name = match &scope.kind {
                    NodeKind::ModuleInst { is_top: true, .. } => strip_lib(&scope.name),
                    _ => scope.name.clone(),
                };
                if !name.is_empty() {
                    parts.push(name);
                }
            } else if self.is_runtime_environment(scope_id) {
                let name = if self.is_compilation_unit(scope_id) {
                    "$unit"
                } else {
                    scope.name.as_str()
                };
                if !name.is_empty() {
                    parts.push(name.to_owned());
                }
            }
            current = scope.parent;
        }
        parts.reverse();
        parts.join(SEPARATOR)
    }

    /// Resolve a `$dumpvars` argument to the owned HDL identity used by the
    /// waveform catalog.  This deliberately consumes semantic targets rather
    /// than reconstructing a path from a generated C name.
    pub(super) fn waveform_selection_name(&self, node: NodeId) -> Result<String, String> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::ScopeRef { target }) => self.waveform_selection_name(*target),
            NodeKind::Expr(ExprKind::Ref { target }) => target
                .map(|target| self.waveform_selection_name(target))
                .unwrap_or_else(|| {
                    Err(format!(
                        "`$dumpvars` reference `{}` has no resolved target",
                        self.node(node).name
                    ))
                }),
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => refs
                .iter()
                .rev()
                .flatten()
                .next()
                .copied()
                .map(|target| self.waveform_selection_name(target))
                .unwrap_or_else(|| {
                    Err(format!(
                        "`$dumpvars` hierarchy `{}` has no resolved target",
                        self.node(node).name
                    ))
                }),
            NodeKind::Expr(ExprKind::Cast { operand, .. }) => {
                self.waveform_selection_name(*operand)
            }
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                let target = self.waveform_array_target(*base).ok_or_else(|| {
                    format!(
                        "cannot resolve `$dumpvars` array selection `{}`",
                        self.node(node).name
                    )
                })?;
                let info = self.array_of(*base).ok_or_else(|| {
                    format!(
                        "array `{}` is not represented in the waveform catalog",
                        self.node(target).name
                    )
                })?;
                if indices.len() != info.dims.len() {
                    return Err(format!(
                        "`$dumpvars` array selection `{}` requires {} declared indices",
                        self.node(target).name,
                        info.dims.len()
                    ));
                }
                let mut name = self.waveform_name(target);
                for (dimension, index) in indices.iter().enumerate() {
                    let value = self.eval_bound_i128(*index).map_err(|error| {
                        format!(
                            "`$dumpvars` array index for `{}` must be a resolved constant: {error}",
                            self.node(target).name
                        )
                    })?;
                    let (left, right) = info.dims.get(dimension).copied().ok_or_else(|| {
                        format!(
                            "internal error while resolving `$dumpvars` array `{}`",
                            self.node(target).name
                        )
                    })?;
                    let low = i128::from(left.min(right));
                    let high = i128::from(left.max(right));
                    if value < low || value > high {
                        return Err(format!(
                            "`$dumpvars` array index {value} for `{}` is outside declared bounds [{left}:{right}]",
                            self.node(target).name
                        ));
                    }
                    name.push('[');
                    name.push_str(&value.to_string());
                    name.push(']');
                }
                Ok(name)
            }
            NodeKind::ModuleInst { .. }
            | NodeKind::GenScopeArray
            | NodeKind::GenScope
            | NodeKind::Port { .. }
            | NodeKind::Net { .. }
            | NodeKind::Var { .. }
            | NodeKind::Array { .. } => Ok(self.waveform_name(node)),
            _ => Err(format!(
                "`$dumpvars` argument `{}` is not a scope or dumpable storage reference",
                self.node(node).name
            )),
        }
    }

    fn waveform_array_target(&self, node: NodeId) -> Option<NodeId> {
        match self.kind(node) {
            NodeKind::Array { .. } => Some(node),
            NodeKind::Expr(ExprKind::Ref { target }) => {
                target.and_then(|target| self.waveform_array_target(target))
            }
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => refs
                .iter()
                .rev()
                .flatten()
                .copied()
                .find_map(|target| self.waveform_array_target(target)),
            _ => None,
        }
    }

    /// The reason a candidate inout-net group cannot be supported, from a
    /// design-wide scan of every write targeting its members. Constant packed
    /// selected continuous drivers are admitted; procedural and dynamic
    /// writes still fail closed.
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
                            if !self.net_lvalue_selects_are_constant(lhs) {
                                return Some(format!(
                                    "dynamic bit/part/select LHS on member `{}`",
                                    self.display_name(
                                        self.member_write_base(lhs, &member_set).unwrap_or(lhs)
                                    )
                                ));
                            }
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
                                self.display_name(
                                    self.member_write_base(lhs, &member_set).unwrap_or(lhs)
                                )
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
                    if let Some(member) = self.member_write_base(lhs, &member_set) {
                        return Some(format!(
                            "nonblocking assignment to member `{}`",
                            self.display_name(member)
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
    /// signal, or through a packed select. Dynamic select validation is kept
    /// separate so the same classifier can be used by all driver scans.
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
            NodeKind::Expr(ExprKind::Ref { target }) => target.as_ref().and_then(|target| {
                if member_set.contains(target) {
                    Some(*target)
                } else {
                    self.member_write_base(*target, member_set)
                }
            }),
            NodeKind::Expr(ExprKind::HierPath { .. }) => {
                let signal = self.hier_path_signal(node)?;
                member_set.iter().find_map(|member| {
                    self.signal_of(*member)
                        .filter(|candidate| candidate.ir == signal.ir)
                        .map(|_| *member)
                })
            }
            NodeKind::Expr(
                ExprKind::BitSelect { base, .. }
                | ExprKind::PartSelect { base, .. }
                | ExprKind::IndexedPartSelect { base, .. }
                | ExprKind::ArraySelect { base, .. },
            ) => self.member_write_base(*base, member_set),
            _ => None,
        }
    }

    /// Find a wired member anywhere inside an LHS-shaped expression.  This
    /// closes fail-closed checks for concatenations and other compound actuals
    /// that the supported direct/ref/select classifier intentionally ignores.
    fn nested_member_target(&self, node: NodeId, member_set: &HashSet<NodeId>) -> Option<NodeId> {
        self.member_write_base(node, member_set).or_else(|| {
            self.node(node)
                .children
                .iter()
                .find_map(|child| self.nested_member_target(*child, member_set))
        })
    }

    /// Whether a task call binds an output/inout formal to a member: those
    /// actuals become `sv4_t*` parameters in the emitted C and would write
    /// through the resolved cell, bypassing resolution.
    fn task_actual_member_write(
        &self,
        call: NodeId,
        member_set: &HashSet<NodeId>,
    ) -> Option<String> {
        let (name, is_task, callee) = match self.kind(call) {
            NodeKind::FuncCall {
                name,
                is_task,
                callee,
                ..
            } => (name.clone(), *is_task, *callee),
            _ => return None,
        };
        let inst = self.owning_inst(call)?;
        let (ft, callee_inst) = self.resolve_callee_env(inst, &name, is_task, callee).ok()?;
        let (_, _, formals) = self.func_info(ft, callee_inst).ok()?;
        let args: Vec<NodeId> = self.node(call).children.clone();
        for (idx, (io, is_out)) in formals.iter().enumerate() {
            if !*is_out {
                continue;
            }
            if let Some(arg) = args.get(idx) {
                if let Some(m) = self.nested_member_target(*arg, member_set) {
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
    pub(super) fn owning_inst(&self, node: NodeId) -> Option<NodeId> {
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

    /// The array a net-declaration continuous assignment initializes (its
    /// LHS resolves to an `Array` node), or `None` for other assignments.
    fn cont_assign_array_target(&self, ca: NodeId) -> Option<NodeId> {
        self.node(ca)
            .children
            .first()
            .copied()
            .and_then(|lhs| self.ref_array_target(lhs))
    }

    fn cont_assign_decl_target(&self, ca: NodeId) -> Option<NodeId> {
        let lhs = self.node(ca).children.first().copied()?;
        match self.kind(lhs) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            NodeKind::Net { .. } | NodeKind::Var { .. } | NodeKind::Array { .. } => Some(lhs),
            _ => None,
        }
    }

    fn collect_aggregate_cont_assign_init(
        &mut self,
        path: &str,
        ca: NodeId,
    ) -> Result<bool, String> {
        if !matches!(self.net_decl_target(ca), NetDeclTarget::Variable) {
            return Ok(false);
        }
        let Some(target) = self.cont_assign_decl_target(ca) else {
            return Ok(false);
        };
        let Some(rhs) = self.node(ca).children.get(1).copied() else {
            return Ok(false);
        };
        if !matches!(
            self.kind(rhs),
            NodeKind::Expr(ExprKind::Operation { op, .. })
                if *op == Operation::AssignmentPattern
        ) {
            return Ok(false);
        }
        if let Some(aggregate) = self.unpacked_aggregates.get(&target).cloned() {
            self.collect_unpacked_aggregate_decl_init(path, target, rhs, &aggregate)?;
            return Ok(true);
        }
        let Some(layout) = self.db.aggregate_layout(target).cloned() else {
            return Ok(false);
        };
        if !matches!(
            layout.kind,
            AggregateKind::PackedStruct | AggregateKind::PackedUnion
        ) {
            return Ok(false);
        }
        let Some(info) = self.signal_of(target).cloned() else {
            return Ok(false);
        };
        let value = self.packed_aggregate_decl_init(path, rhs, &layout, &info)?;
        self.scalar_inits.push((info, value));
        Ok(true)
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
                NetType::None => NetDeclTarget::Variable,
                NetType::Wire
                | NetType::Tri
                | NetType::Uwire
                | NetType::Logic
                | NetType::Wand
                | NetType::TriAnd
                | NetType::Wor
                | NetType::TriOr
                | NetType::Tri0
                | NetType::Tri1
                | NetType::Supply0
                | NetType::Supply1 => NetDeclTarget::TrueNet,
                NetType::Reg => NetDeclTarget::Variable,
                other => NetDeclTarget::UnsupportedNet(other),
            },
            _ => NetDeclTarget::Unknown,
        }
    }

    /// The declaration-initializer constants of a net-declaration
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
        let vals = match self.array_init_consts(path, &name, rhs) {
            Ok(values) => values,
            Err(_) => return Ok(None),
        };
        Ok(Some((target, vals)))
    }

    fn collect_scalar_decl_init(&mut self, path: &str, ca: NodeId) -> Result<(), String> {
        let target = self.cont_assign_decl_target(ca).ok_or_else(|| {
            format!("variable declaration initializer in `{path}` has no scalar target")
        })?;
        let info = self.signal_of(target).cloned().ok_or_else(|| {
            format!(
                "variable declaration initializer for `{}` in `{path}` has no storage",
                self.node(target).name
            )
        })?;
        let initializer = self.node(ca).children.get(1).copied().ok_or_else(|| {
            format!(
                "variable declaration initializer for `{}` in `{path}` has no RHS",
                self.node(target).name
            )
        })?;
        if let Some(inst) = self.owning_inst(ca) {
            self.inst = inst;
        }
        let lowered = self.lower_declaration_initializer(
            path,
            target,
            initializer,
            IrInitTarget::Signal(info.ir),
            info.width,
            info.signed,
            info.two_state,
            info.real,
        );
        match lowered {
            Ok(initializer) => self.declaration_inits.push(initializer),
            Err(lowering_error) => {
                let value = self.scalar_decl_init(path, ca)?.ok_or(lowering_error)?.1;
                self.scalar_inits.push((info, value));
            }
        }
        self.scalar_init_ca.insert(ca);
        Ok(())
    }

    /// The declaration-initializer constant of a net-declaration assignment whose LHS
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
        // The RHS is a constant expression after elaboration; try a plain
        // constant first, then constant-foldable operations/params.  Anything
        // non-constant falls through to the emission error path.
        let c = match self.const_of_node(rhs) {
            Ok(c) => c,
            Err(_) => match self.eval_decl_value(rhs) {
                Ok(v) => decl_value_to_const(v)?,
                Err(_) => return Ok(None),
            },
        };
        Ok(Some((info, c)))
    }

    /// The declaration-initializer constant of a scalar VARIABLE whose init
    /// is attached to the variable (`logic l = 1'b0;`, `int x = 5;`). The RHS
    /// is a constant expression after elaboration: try a plain constant
    /// first, then constant-foldable operations/params via `eval_bits` (which
    /// resolves parameter references through `param_vals`).  Anything
    /// non-constant is rejected because variable initializers must be constant
    /// expressions.
    pub(super) fn var_decl_init(
        &self,
        path: &str,
        name: &str,
        init: NodeId,
    ) -> Result<IrConst, String> {
        match self.const_of_node(init) {
            Ok(c) => Ok(c),
            Err(_) => match self.eval_decl_value(init) {
                Ok(v) => decl_value_to_const(v),
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
                if *op == Operation::AssignmentPattern =>
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
            .map(|operand| {
                self.const_of_node(*operand)
                    .or_else(|_| self.eval_decl_value(*operand).and_then(decl_value_to_const))
                    .map_err(|_| {
                        format!(
                            "array `{name}` in `{path}`: initializer element is not a \
                             supported constant expression ({:?})",
                            self.kind(*operand)
                        )
                    })
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
            .arrays()
            .get(&node)
            .ok_or_else(|| format!("array `{name}` in `{path}` has no captured metadata"))?;
        let (elem_width, real, shortreal) = match ty.kind.as_str() {
            "real" | "shortreal" => (0, true, ty.kind == "shortreal"),
            "int" | "integer" | "time" | "longint" | "byte" | "shortint" | "logic" | "reg"
            | "bit" => (ty.width.unwrap_or(1), false, false),
            _ => {
                return Err(format!(
                    "array `{name}` in `{path}` has unsupported element type `{}`",
                    ty.kind
                ))
            }
        };
        if !real && elem_width > LLG_MAX_WIDTH {
            return Err(format!(
                "array `{name}` in `{path}` has {elem_width}-bit elements; the \
                 runtime maximum supported width is {LLG_MAX_WIDTH}"
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
            Some(eid) => match self.array_init_consts(path, name, eid) {
                Ok(values) => Some(values),
                Err(_) => {
                    self.array_initializers.push((node, eid));
                    None
                }
            },
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
            two_state: self.db.is_two_state_type(node) || is_two_state_kind(&ty.kind),
            real,
            shortreal,
            dims: dims.clone(),
            total,
        });
        Ok(ArrayInfo {
            global: global_name(path, name),
            elem_width,
            signed: ty.signed,
            real,
            shortreal,
            is_net: meta.net_type().is_some(),
            dims,
            init,
            ir,
        })
    }

    fn container_info(
        &mut self,
        path: &str,
        name: &str,
        node: NodeId,
        _ty: &crate::core::model::TypeInfo,
    ) -> Result<ContainerInfo, String> {
        let meta = self
            .db
            .array_meta(node)
            .ok_or_else(|| format!("container `{name}` in `{path}` has no captured metadata"))?;
        let has_initializer = meta.initializer().is_some();
        let descriptor = self.db.type_descriptor(node).ok_or_else(|| {
            format!("container `{name}` in `{path}` has no recursive type descriptor")
        })?;
        let element = match &descriptor.shape {
            TypeShape::Container { element, .. } => lower_container_element(element)?,
            _ => {
                return Err(format!(
                    "container `{name}` in `{path}` has a non-container type descriptor"
                ))
            }
        };
        let kind = match meta.kind() {
            ArrayKind::Static => return Err("internal: static array reached container lowering".into()),
            ArrayKind::Dynamic => IrContainerKind::Dynamic,
            ArrayKind::Queue { maximum_elements } => IrContainerKind::Queue {
                maximum_elements: *maximum_elements,
            },
            ArrayKind::Associative(index) => IrContainerKind::Associative {
                key: match index {
                    AssociativeIndex::Wildcard => IrAssocKey::Wildcard,
                    AssociativeIndex::Integral {
                        width,
                        signed,
                        two_state,
                    } => IrAssocKey::Integral {
                        width: *width,
                        signed: *signed,
                        two_state: *two_state,
                    },
                    AssociativeIndex::String => IrAssocKey::String,
                    AssociativeIndex::Unsupported(kind) => {
                        return Err(format!(
                            "associative array `{name}` in `{path}` has unsupported index type `{kind}`"
                        ))
                    }
                },
            },
        };
        let ir = self.model.containers.len();
        self.model.containers.push(IrContainer {
            c_name: global_name(path, name),
            element,
            kind,
        });
        if has_initializer {
            self.container_initializers.push((node, ir));
        }
        Ok(ContainerInfo { ir })
    }

    // ── Functions and tasks ───────────────────────────────────────────────

    /// Emit a `static` prototype for every function/task in the instance
    /// tree, so bodies may call each other regardless of declaration order.
    /// Timing-capable tasks use the same typed C-call ABI as delay-free tasks.
    /// Their `llg_wait_*` operations suspend the caller's libaco coroutine, so
    /// each recursive C activation remains resumable without source unrolling.
    pub(super) fn emit_func_prototypes(&mut self, inst: NodeId) -> Result<(), String> {
        for c in &self.node(inst).children {
            if let NodeKind::FuncTask {
                is_task, automatic, ..
            } = self.kind(*c)
            {
                let automatic = *automatic;
                let has_wait = *is_task && self.task_has_wait(*c, inst);
                let (is_task_f, ret, formals) = self.func_info(*c, inst)?;
                let formals_ir: Vec<IrFormal> = formals
                    .iter()
                    .map(|(io, is_out)| -> Result<IrFormal, String> {
                        match self.kind(*io) {
                            NodeKind::FuncArg {
                                direction,
                                const_ref,
                                ref_static,
                                ty,
                                ..
                            } => {
                                let mode = match direction {
                                    DbDirection::Input => crate::sim::ir::IrFormalMode::Input,
                                    DbDirection::Output => crate::sim::ir::IrFormalMode::Output,
                                    DbDirection::Inout => crate::sim::ir::IrFormalMode::Inout,
                                    DbDirection::Ref => crate::sim::ir::IrFormalMode::Ref,
                                    _ => {
                                        return Err(format!(
                                            "unsupported formal direction for `{}`",
                                            self.node(*io).name
                                        ));
                                    }
                                };
                                Ok(IrFormal {
                                    is_out: *is_out,
                                    mode,
                                    const_ref: *const_ref,
                                    ref_static: *ref_static,
                                    width: if ty.kind == "chandle" || is_real_kind(&ty.kind) {
                                        0
                                    } else {
                                        ty.width
                                            .map(|width| {
                                                self.effective_decl_width(*io, inst, width)
                                            })
                                            .unwrap_or(0)
                                    },
                                    signed: ty.signed,
                                    two_state: self.db.is_two_state_type(*io)
                                        || is_two_state_kind(&ty.kind),
                                    real: is_real_kind(&ty.kind),
                                    shortreal: ty.kind == "shortreal",
                                    chandle: ty.kind == "chandle",
                                    event: ty.kind == "event",
                                    string: ty.kind == "string",
                                })
                            }
                            _ => unreachable!("formal kind"),
                        }
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                if !automatic {
                    for (idx, ((io, _), formal)) in formals.iter().zip(&formals_ir).enumerate() {
                        if formal.is_ref() {
                            continue;
                        }
                        if formal.chandle || formal.event {
                            let object = self.model.objects.len();
                            if formal.event {
                                continue;
                            }
                            self.model.objects.push(crate::sim::ir::IrObject {
                                c_name: format!("O_f{}_{}_a{idx}", inst.index(), c.index()),
                                ty: crate::sim::ir::IrObjectType::Chandle,
                                initial: None,
                            });
                            self.static_chandle_formals.insert((inst, *io), object);
                            continue;
                        }
                        if formal.string {
                            let object = self.model.objects.len();
                            self.model.objects.push(crate::sim::ir::IrObject {
                                c_name: format!("O_f{}_{}_a{idx}", inst.index(), c.index()),
                                ty: crate::sim::ir::IrObjectType::String,
                                initial: None,
                            });
                            self.static_string_formals.insert((inst, *io), object);
                            continue;
                        }
                        let signal = self.model.signals.len();
                        let info = SignalInfo {
                            global: format!("S_f{}_{}_a{idx}", inst.index(), c.index()),
                            width: formal.width,
                            signed: formal.signed,
                            two_state: formal.two_state,
                            real: formal.real,
                            shortreal: formal.shortreal,
                            net_driver: None,
                            ir: signal,
                        };
                        self.model.signals.push(IrSignal {
                            c_name: info.global.clone(),
                            hdl_name: None,
                            ty: if info.real {
                                IrType::Real {
                                    shortreal: info.shortreal,
                                }
                            } else {
                                IrType::Packed {
                                    width: info.width,
                                    signed: info.signed,
                                    two_state: info.two_state,
                                }
                            },
                            net_driver: None,
                            alias: None,
                            omit: false,
                        });
                        self.signals.push(info.clone());
                        self.static_formals.insert((inst, *io), info);
                    }
                }
                if has_wait {
                    let body = self
                        .func_body(*c)
                        .ok_or_else(|| format!("task `{}` without a body", self.node(*c).name))?;
                    let mut locals = HashMap::new();
                    let mut chandle_locals = HashMap::new();
                    let mut local_seq = 0;
                    self.collect_func_locals(
                        body,
                        inst,
                        &mut locals,
                        &mut chandle_locals,
                        &mut local_seq,
                        "",
                    )?;
                    for (local, _) in chandle_locals {
                        match self.db.variable_lifetime(local) {
                            VariableLifetime::Automatic => continue,
                            VariableLifetime::Static => {}
                            VariableLifetime::Unavailable => {
                                return Err(format!(
                                    "resolved lifetime is unavailable for task local `{}`",
                                    self.node(local).name
                                ));
                            }
                        }
                        let object = self.model.objects.len();
                        self.model.objects.push(crate::sim::ir::IrObject {
                            c_name: format!("O_f{}_{}_l{}", inst.index(), c.index(), local.index()),
                            ty: crate::sim::ir::IrObjectType::Chandle,
                            initial: None,
                        });
                        self.static_task_chandle_locals
                            .insert((inst, local), object);
                    }
                    for (local, (_, width, signed, two_state, shortreal)) in locals {
                        match self.db.variable_lifetime(local) {
                            VariableLifetime::Automatic => continue,
                            VariableLifetime::Static => {}
                            VariableLifetime::Unavailable => {
                                return Err(format!(
                                    "resolved lifetime is unavailable for task local `{}`",
                                    self.node(local).name
                                ));
                            }
                        }
                        if matches!(self.kind(local), NodeKind::Var { ty } if ty.kind == "string") {
                            let initializer = self.db.var_initializer(local);
                            let initial = initializer
                                .map(|initializer| {
                                    self.lower_string(&self.instance_path_of(inst), initializer)
                                })
                                .transpose()?;
                            let object = self.model.objects.len();
                            self.model.objects.push(crate::sim::ir::IrObject {
                                c_name: format!(
                                    "O_f{}_{}_l{}",
                                    inst.index(),
                                    c.index(),
                                    local.index()
                                ),
                                ty: crate::sim::ir::IrObjectType::String,
                                initial,
                            });
                            self.static_string_task_locals.insert((inst, local), object);
                            continue;
                        }
                        let signal = self.model.signals.len();
                        let info = SignalInfo {
                            global: format!("S_f{}_{}_l{}", inst.index(), c.index(), local.index()),
                            width,
                            signed,
                            two_state,
                            real: width == 0,
                            shortreal,
                            net_driver: None,
                            ir: signal,
                        };
                        self.model.signals.push(IrSignal {
                            c_name: info.global.clone(),
                            hdl_name: None,
                            ty: if width == 0 {
                                IrType::Real { shortreal }
                            } else {
                                IrType::Packed {
                                    width,
                                    signed,
                                    two_state,
                                }
                            },
                            net_driver: None,
                            alias: None,
                            omit: false,
                        });
                        self.signals.push(info.clone());
                        if let Some(initializer) = self.db.var_initializer(local) {
                            self.inst = inst;
                            let lowered = self.lower_declaration_initializer(
                                &self.instance_path_of(inst),
                                local,
                                initializer,
                                IrInitTarget::Signal(info.ir),
                                info.width,
                                info.signed,
                                info.two_state,
                                info.real,
                            );
                            match lowered {
                                Ok(initializer) => self.declaration_inits.push(initializer),
                                Err(lowering_error) => {
                                    let value = self
                                        .var_decl_init(
                                            &self.instance_path_of(inst),
                                            &self.node(local).name,
                                            initializer,
                                        )
                                        .map_err(|_| lowering_error)?;
                                    self.var_inits.push((info.clone(), value));
                                }
                            }
                        }
                        self.static_task_locals.insert((inst, local), info);
                    }
                }
                let c_name =
                    self.func_names.get(c).cloned().ok_or_else(|| {
                        format!("function `{}` has no C name", self.node(*c).name)
                    })?;
                // Register the model entry (call-site lowering and the C
                // renderers resolve through it).
                let ir = self.model.funcs.len();
                self.model.funcs.push(crate::sim::ir::IrFunc {
                    c_name,
                    automatic,
                    ret_chandle: matches!(
                        self.kind(*c),
                        NodeKind::FuncTask {
                            ret: Some(ty), ..
                        } if ty.kind == "chandle"
                    ),
                    ret_string: self.is_string_return(*c),
                    ret: ret.map(|(w, s, two_state, shortreal)| {
                        if w == 0 {
                            IrType::Real { shortreal }
                        } else {
                            IrType::Packed {
                                width: w,
                                signed: s,
                                two_state,
                            }
                        }
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
                        ret_chandle: self.is_chandle_return(*c),
                        ret_string: self.is_string_return(*c),
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
    /// Emit every function/task body. Timing-capable tasks are ordinary C
    /// calls whose waits suspend the current libaco coroutine.
    pub(super) fn emit_func_bodies(&mut self, inst: NodeId) -> Result<(), String> {
        for c in &self.node(inst).children {
            if matches!(self.kind(*c), NodeKind::FuncTask { .. }) {
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
    fn func_signature(
        &self,
        ft: NodeId,
        inst: NodeId,
    ) -> Result<(String, Vec<(NodeId, bool)>), String> {
        let (is_task, ret, formals) = self.func_info(ft, inst)?;
        let c_name = self
            .func_names
            .get(&ft)
            .cloned()
            .ok_or_else(|| format!("function `{}` has no C name", self.node(ft).name))?;
        let ret_t = if is_task || ret.is_none() {
            if !is_task && self.is_string_return(ft) {
                "llg_string_t"
            } else if !is_task && self.is_chandle_return(ft) {
                "void *"
            } else {
                "void"
            }
        } else {
            "sv4_t"
        };
        let mut params = Vec::new();
        // Address formals first (outputs/inouts use `o{idx}`, refs use
        // `r{idx}`), then by-value inputs. Names use the formal's declaration
        // index, matching the maps built when emitting the body.
        for (idx, (_, is_out)) in formals.iter().enumerate() {
            let direction = match self.kind(formals[idx].0) {
                NodeKind::FuncArg { direction, .. } => *direction,
                _ => return Err("non-formal in function signature".to_string()),
            };
            if matches!(direction, DbDirection::Ref) {
                let const_ref = matches!(
                    self.kind(formals[idx].0),
                    NodeKind::FuncArg {
                        const_ref: true,
                        ..
                    }
                );
                let is_string = matches!(
                    self.kind(formals[idx].0),
                    NodeKind::FuncArg { ty, .. } if ty.kind == "string"
                );
                let is_chandle = matches!(
                    self.kind(formals[idx].0),
                    NodeKind::FuncArg { ty, .. } if ty.kind == "chandle"
                );
                params.push(if is_string {
                    format!(
                        "{}llg_string_t* r{idx}",
                        if const_ref { "const " } else { "" }
                    )
                } else if is_chandle {
                    format!("void *{}* r{idx}", if const_ref { "const " } else { "" })
                } else {
                    format!("{}llg_ref_t* r{idx}", if const_ref { "const " } else { "" })
                });
            } else if *is_out {
                let ty = match self.kind(formals[idx].0) {
                    NodeKind::FuncArg { ty, .. } if ty.kind == "string" => "llg_string_t",
                    NodeKind::FuncArg { ty, .. } if ty.kind == "chandle" => "void *",
                    NodeKind::FuncArg { ty, .. } if is_real_kind(&ty.kind) => "double",
                    _ => "sv4_t",
                };
                params.push(format!("{ty}* o{idx}"));
            }
        }
        for (idx, (_, is_out)) in formals.iter().enumerate() {
            let is_ref = matches!(
                self.kind(formals[idx].0),
                NodeKind::FuncArg {
                    direction: DbDirection::Ref,
                    ..
                }
            );
            if !*is_out && !is_ref {
                let ty = match self.kind(formals[idx].0) {
                    NodeKind::FuncArg { ty, .. } if ty.kind == "string" => "llg_string_t",
                    NodeKind::FuncArg { ty, .. } if ty.kind == "chandle" => "void *",
                    NodeKind::FuncArg { ty, .. } if is_real_kind(&ty.kind) => "double",
                    _ => "sv4_t",
                };
                params.push(format!("{ty} a{idx}"));
            }
        }
        params.push("int depth".to_string());
        Ok((
            format!("static {ret_t} {c_name}({}", params.join(", ")),
            formals,
        ))
    }

    fn is_chandle_return(&self, ft: NodeId) -> bool {
        matches!(
            self.kind(ft),
            NodeKind::FuncTask {
                ret: Some(ty), ..
            } if ty.kind == "chandle"
        )
    }

    fn is_string_return(&self, ft: NodeId) -> bool {
        matches!(
            self.kind(ft),
            NodeKind::FuncTask {
                ret: Some(ty), ..
            } if ty.kind == "string"
        )
    }

    /// `(is_task, return width/signed, (io_decl node, is_output) in formal
    /// order)` of a FuncTask node.  The return width is `None` for void
    /// functions and tasks.
    // The tuple mirrors the semantic function/task signature without introducing a
    // public one-off type solely for this private lowering boundary.
    #[allow(clippy::type_complexity)]
    pub(super) fn func_info(
        &self,
        ft: NodeId,
        inst: NodeId,
    ) -> Result<(bool, Option<(u32, bool, bool, bool)>, Vec<(NodeId, bool)>), String> {
        let (is_task, ret) = match self.kind(ft) {
            NodeKind::FuncTask { is_task, ret, .. } => (*is_task, ret.clone()),
            _ => return Err("non-FuncTask passed to func_info".to_string()),
        };
        let ret_two_state = self
            .node(ft)
            .children
            .iter()
            .copied()
            .find(|child| matches!(self.kind(*child), NodeKind::Var { .. }))
            .is_some_and(|return_var| self.db.is_two_state_type(return_var));
        let ret = match ret {
            Some(ty) if matches!(ty.kind.as_str(), "chandle" | "string") => None,
            Some(ty) => {
                if is_real_kind(&ty.kind) {
                    Some((0, false, false, ty.kind == "shortreal"))
                } else {
                    match ty.width {
                        Some(w) => {
                            let w = self.effective_decl_width(ft, inst, w);
                            if w > LLG_MAX_WIDTH {
                                return Err(format!(
                                    "return type of `{}` is {w} bits wide; the runtime \
                             maximum supported width is {LLG_MAX_WIDTH}",
                                    self.node(ft).name
                                ));
                            }
                            Some((
                                w,
                                ty.signed,
                                ret_two_state || is_two_state_kind(&ty.kind),
                                false,
                            ))
                        }
                        None => {
                            return Err(format!(
                                "return type of `{}` has no width",
                                self.node(ft).name
                            ))
                        }
                    }
                }
            }
            None => None,
        };
        let mut formals = Vec::new();
        for c in &self.node(ft).children {
            match self.kind(*c) {
                NodeKind::FuncArg { direction, .. } => {
                    let is_out = matches!(direction, DbDirection::Output | DbDirection::Inout);
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

    /// The body statement identified by Slang's semantic `Body` relationship.
    pub(super) fn func_body(&self, ft: NodeId) -> Option<NodeId> {
        self.db.subroutine_body(ft)
    }

    /// Emit one static C function for a function/task definition.  The body
    /// statements are emitted with the io_decls mapped to the C parameters and
    /// the locals to C locals; the function-name variable maps to a local
    /// `_ret` that `return` reads.
    fn emit_func_task(&mut self, path: &str, inst: NodeId, ft: NodeId) -> Result<(), String> {
        let automatic = matches!(
            self.kind(ft),
            NodeKind::FuncTask {
                automatic: true,
                ..
            }
        );
        let (is_task, ret, formals) = self.func_info(ft, inst)?;
        let ret_chandle = self.is_chandle_return(ft);
        let ret_string = self.is_string_return(ft);
        let (decl, _) = self.func_signature(ft, inst)?;
        let c_name = self
            .func_names
            .get(&ft)
            .cloned()
            .ok_or_else(|| format!("function `{}` has no C name", self.node(ft).name))?;
        let has_ret = ret.is_some() || ret_chandle || ret_string;
        // Slang binds an assignment to the function name directly to the
        // subroutine symbol; that symbol is the return-storage identity.
        let ret_var = has_ret.then_some(ft);
        let body = self
            .func_body(ft)
            .ok_or_else(|| format!("function `{}` without a body", self.node(ft).name))?;
        if self.node_has_stack_backed_subroutine_nba(body, ft, automatic) {
            let kind = if is_task { "task" } else { "function" };
            return Err(format!(
                "nonblocking assignment in {kind} `{}` targets stack-backed input/formal/local storage which cannot outlive the call",
                self.node(ft).name
            ));
        }

        // The all-X return value used by the recursion guard.
        let ret_x = match ret {
            Some((w, s, two_state, _shortreal)) if w > 0 => format!(
                "{}({w}, {})",
                if two_state { "sv4_zero" } else { "sv4_x" },
                s as u8
            ),
            Some((_, _, _, _)) => "0.0".to_owned(),
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

        let mut locals: HashMap<NodeId, (String, u32, bool, bool, bool)> = HashMap::new();
        let mut chandle_locals: HashMap<NodeId, String> = HashMap::new();
        let mut local_seq = 0usize;
        let local_prefix = format!("_f{}_{}_", inst.index(), ft.index());
        self.collect_func_locals(
            body,
            inst,
            &mut locals,
            &mut chandle_locals,
            &mut local_seq,
            &local_prefix,
        )?;
        let mut declaration_initializers = HashMap::new();
        self.collect_subroutine_decl_initializers(body, &mut declaration_initializers);

        // Function-name return variable → `_ret` local.
        let ret_ctx = match (ret, ret_var) {
            (Some((w, s, two_state, shortreal)), Some(rv)) => Some(RetCtx {
                c_name: "_ret".to_string(),
                width: w,
                signed: s,
                two_state,
                shortreal,
                node: Some(rv),
            }),
            _ => None,
        };

        let mut arg_read: HashMap<NodeId, ArgMap> = HashMap::new();
        let mut arg_ir: HashMap<NodeId, IrExpr> = HashMap::new();
        let mut arg_write: HashMap<NodeId, String> = HashMap::new();
        let mut arg_lhs: HashMap<NodeId, Lhs> = HashMap::new();
        let mut const_refs: HashSet<NodeId> = HashSet::new();
        let const_ref_lhs = HashMap::new();
        let mut persistent = HashMap::new();
        let mut chandle_read = HashMap::new();
        let mut chandle_write = HashMap::new();
        let mut string_read = HashMap::new();
        let mut string_write = HashMap::new();
        let mut string_addr = HashMap::new();
        let mut static_input_copies = Vec::new();
        for (local, name) in &chandle_locals {
            if self.db.variable_lifetime(*local) == VariableLifetime::Static {
                let object = if let Some(object) = self
                    .static_task_chandle_locals
                    .get(&(inst, *local))
                    .copied()
                {
                    object
                } else {
                    let object = self.model.objects.len();
                    self.model.objects.push(crate::sim::ir::IrObject {
                        c_name: format!("O_f{}_l{}", inst.index(), local.index()),
                        ty: crate::sim::ir::IrObjectType::Chandle,
                        initial: None,
                    });
                    self.static_task_chandle_locals
                        .insert((inst, *local), object);
                    object
                };
                chandle_read.insert(*local, IrChandleExpr::Read(object));
                chandle_write.insert(*local, ChandleTarget::Object(object));
            } else {
                chandle_read.insert(*local, IrChandleExpr::LocalRead(name.clone()));
                chandle_write.insert(*local, ChandleTarget::Local(name.clone()));
            }
        }
        for (local, (name, ..)) in &locals {
            if matches!(self.kind(*local), NodeKind::Var { ty } if ty.kind == "string") {
                string_read.insert(*local, IrStringExpr::LocalRead(name.clone()));
                string_write.insert(*local, name.clone());
                string_addr.insert(*local, name.clone());
            }
        }
        for (idx, (io, is_out)) in formals.iter().enumerate() {
            if matches!(self.kind(*io), NodeKind::FuncArg { ty, .. } if ty.kind == "event") {
                // Event formals are admitted only through inline call
                // lowering, which substitutes the caller's object identity.
                // The fallback C body is retained for deterministic model
                // shape but has no packed formal storage.
                continue;
            }
            if matches!(self.kind(*io), NodeKind::FuncArg { ty, .. } if ty.kind == "chandle") {
                let is_ref = matches!(
                    self.kind(*io),
                    NodeKind::FuncArg {
                        direction: DbDirection::Ref,
                        ..
                    }
                );
                let const_ref = matches!(
                    self.kind(*io),
                    NodeKind::FuncArg {
                        direction: DbDirection::Ref,
                        const_ref: true,
                        ..
                    }
                );
                if !is_ref && !*is_out {
                    if let Some(object) = (!automatic)
                        .then(|| self.static_chandle_formals.get(&(inst, *io)).copied())
                        .flatten()
                    {
                        chandle_read.insert(*io, IrChandleExpr::Read(object));
                        chandle_write.insert(*io, ChandleTarget::Object(object));
                        static_input_copies.push(IrStmt::Object(
                            crate::sim::ir::IrObjectStmt::ChandleAssign(
                                object,
                                IrChandleExpr::FormalRead(idx),
                            ),
                        ));
                        continue;
                    }
                }
                {
                    chandle_read.insert(*io, IrChandleExpr::FormalRead(idx));
                    if !const_ref {
                        let target = if is_ref {
                            format!("*r{idx}")
                        } else if *is_out {
                            format!("*o{idx}")
                        } else {
                            format!("a{idx}")
                        };
                        chandle_write.insert(*io, ChandleTarget::Local(target));
                    }
                }
                continue;
            }
            if matches!(self.kind(*io), NodeKind::FuncArg { ty, .. } if ty.kind == "string") {
                let is_ref = matches!(
                    self.kind(*io),
                    NodeKind::FuncArg {
                        direction: DbDirection::Ref,
                        ..
                    }
                );
                let const_ref = matches!(
                    self.kind(*io),
                    NodeKind::FuncArg {
                        const_ref: true,
                        ..
                    }
                );
                if is_ref {
                    string_read.insert(*io, IrStringExpr::FormalRead(idx));
                    string_addr.insert(*io, format!("*r{idx}"));
                    if !const_ref {
                        string_write.insert(*io, format!("*r{idx}"));
                    }
                } else if let Some(object) = (!automatic)
                    .then(|| self.static_string_formals.get(&(inst, *io)).copied())
                    .flatten()
                {
                    let name = self.model.objects[object].c_name.clone();
                    string_read.insert(*io, IrStringExpr::Read(object));
                    string_write.insert(*io, name.clone());
                    string_addr.insert(*io, name);
                    if !*is_out {
                        static_input_copies.push(IrStmt::Object(IrObjectStmt::StringAssign(
                            object,
                            IrStringExpr::FormalRead(idx),
                        )));
                    }
                } else {
                    string_read.insert(*io, IrStringExpr::FormalRead(idx));
                    string_write.insert(
                        *io,
                        if *is_out {
                            format!("*o{idx}")
                        } else {
                            format!("a{idx}")
                        },
                    );
                    string_addr.insert(
                        *io,
                        if *is_out {
                            format!("*o{idx}")
                        } else {
                            format!("a{idx}")
                        },
                    );
                }
                continue;
            }
            let (w, s, two_state, _real, _shortreal) = match self.kind(*io) {
                NodeKind::FuncArg { ty, .. } => {
                    if is_real_kind(&ty.kind) {
                        (0, false, false, true, ty.kind == "shortreal")
                    } else {
                        match ty.width {
                            Some(w) if w <= LLG_MAX_WIDTH => (
                                self.effective_decl_width(*io, inst, w),
                                ty.signed,
                                self.db.is_two_state_type(*io) || is_two_state_kind(&ty.kind),
                                false,
                                false,
                            ),
                            Some(w) => {
                                return Err(format!(
                                    "formal `{}` of `{c_name}` is {w} bits wide; the runtime \
                             maximum supported width is {LLG_MAX_WIDTH}",
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
                }
                _ => unreachable!("formal kind"),
            };
            let (is_ref, const_ref) = match self.kind(*io) {
                NodeKind::FuncArg {
                    direction: DbDirection::Ref,
                    const_ref,
                    ..
                } => (true, *const_ref),
                _ => (false, false),
            };
            if is_ref {
                arg_ir.insert(*io, formal_read_expr(idx, w, s));
                arg_read.insert(
                    *io,
                    ArgMap {
                        width: w,
                        signed: s,
                        two_state,
                    },
                );
                if const_ref {
                    const_refs.insert(*io);
                } else {
                    arg_lhs.insert(
                        *io,
                        Lhs::Ref {
                            addr: format!("r{idx}"),
                            width: w,
                            signed: s,
                            two_state,
                            const_ref: false,
                        },
                    );
                }
                continue;
            }
            if let Some(storage) = (!automatic)
                .then(|| self.static_formals.get(&(inst, *io)).cloned())
                .flatten()
            {
                arg_write.insert(*io, format!("&{}", storage.global));
                persistent.insert(*io, storage.clone());
                arg_ir.insert(*io, sig_read_expr_full(&storage));
                arg_read.insert(
                    *io,
                    ArgMap {
                        width: w,
                        signed: s,
                        two_state,
                    },
                );
                if !*is_out {
                    let lhs = IrLhs::Whole(storage.ir);
                    static_input_copies.push(IrStmt::Assign {
                        lhs: lhs.clone(),
                        rhs: apply_lhs_assignment_context(
                            &self.model,
                            &lhs,
                            formal_read_expr(idx, w, s),
                        ),
                        nba: false,
                    });
                }
            } else if *is_out {
                arg_write.insert(*io, format!("o{idx}"));
                arg_ir.insert(*io, formal_read_expr(idx, w, s));
                arg_read.insert(
                    *io,
                    ArgMap {
                        width: w,
                        signed: s,
                        two_state,
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
                        two_state,
                    },
                );
            }
        }
        if ret_chandle {
            let return_var = ret_var.ok_or_else(|| {
                format!(
                    "chandle function `{}` has no return variable",
                    self.node(ft).name
                )
            })?;
            chandle_read.insert(return_var, IrChandleExpr::LocalRead("_ret".to_string()));
            chandle_write.insert(return_var, ChandleTarget::Local("_ret".to_string()));
        }
        if ret_string {
            let return_var = ret_var.ok_or_else(|| {
                format!(
                    "string function `{}` has no return variable",
                    self.node(ft).name
                )
            })?;
            string_read.insert(
                return_var,
                crate::sim::ir::IrStringExpr::LocalRead("_ret".to_string()),
            );
            string_write.insert(return_var, "_ret".to_string());
            string_addr.insert(return_var, "_ret".to_string());
        }

        let func_ctx = FuncCtx {
            name: self.node(ft).name.clone(),
            is_task,
            ret: ret_ctx.clone(),
            arg_read,
            arg_ir,
            event_args: HashMap::new(),
            arg_dependencies: HashMap::new(),
            arg_write,
            arg_lhs,
            const_refs,
            const_ref_lhs,
            persistent,
            chandle_read,
            chandle_write,
            string_read,
            string_write,
            string_addr,
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
        let (mut body_stmts, mut pre_fns) = {
            let mut ctx = EmitCtx::new(
                self,
                path.to_string(),
                inst,
                "depth + 1",
                Some(func_ctx),
                None,
                false,
            );
            let mut body_stmts = static_input_copies;
            body_stmts.extend(ctx.lower_stmt(body)?);
            let pre_fns = std::mem::take(&mut ctx.pre_fns);
            (body_stmts, pre_fns)
        };
        pre_fns.extend(std::mem::take(&mut self.pending_container_pre_fns));
        // Delay-free tasks need the same declaration-level activation as an
        // inlined timed task. A self-disable must cancel every active invocation.
        if is_task {
            body_stmts = vec![IrStmt::ActivationScope {
                target: self.activation_target(ft)?,
                exit: self.new_fn_name(path, "task_exit"),
                body: body_stmts,
            }];
        }
        let mut declaration_initializations = Vec::new();
        for (local, (c_name, width, signed, two_state, shortreal)) in &locals {
            if self.db.variable_lifetime(*local) != VariableLifetime::Static {
                continue;
            }
            let initializer = self
                .db
                .var_initializer(*local)
                .or_else(|| declaration_initializers.get(local).copied());
            let Some(initializer) = initializer else {
                continue;
            };
            let initialization = self
                .lower_declaration_initializer(
                    path,
                    *local,
                    initializer,
                    IrInitTarget::StaticLocal {
                        function: meta_ir,
                        name: c_name.clone(),
                    },
                    *width,
                    *signed,
                    *two_state,
                    *shortreal,
                )
                .map_err(|cause| {
                    format!(
                        "static subprogram initializer for `{}` cannot be lowered: {cause}",
                        self.node(*local).name
                    )
                })?;
            declaration_initializations.push(initialization);
        }
        self.declaration_inits.extend(declaration_initializations);
        // Restore the process-level context for whatever is lowered next
        // (continuous assignments, processes).
        self.func = None;
        self.cur_fn_ir = None;
        self.depth_arg = "0".to_string();

        let ir_locals = {
            let mut names = locals.into_iter().collect::<Vec<_>>();
            names.sort_by_key(|(id, _)| id.0);
            let mut emitted = HashSet::new();
            names
                .into_iter()
                .filter(|(local, _)| self.db.variable_lifetime(*local) == VariableLifetime::Static)
                .filter(|(_, (c_name, ..))| emitted.insert(c_name.clone()))
                .map(|(local, (c_name, width, signed, two_state, shortreal))| {
                    Ok(crate::sim::ir::IrLocal {
                        c_name,
                        width,
                        signed,
                        two_state,
                        real: width == 0,
                        shortreal,
                        string: matches!(self.kind(local), NodeKind::Var { ty } if ty.kind == "string"),
                        initial: None,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?
        };
        let no_entry = format!("function `{}` has no model entry", self.node(ft).name);
        let entry = self.model.funcs.get_mut(meta_ir).ok_or(no_entry)?;
        entry.locals = ir_locals;
        entry.automatic = automatic;
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
    pub(super) fn collect_func_locals(
        &self,
        node: NodeId,
        inst: NodeId,
        locals: &mut HashMap<NodeId, (String, u32, bool, bool, bool)>,
        chandle_locals: &mut HashMap<NodeId, String>,
        seq: &mut usize,
        prefix: &str,
    ) -> Result<(), String> {
        if let NodeKind::Stmt(StmtKind::For { body, .. }) = self.kind(node) {
            // For-declaration variables have loop-entry lifetime and are
            // collected by `lower_for`; they are not function-entry locals.
            return self.collect_func_locals(*body, inst, locals, chandle_locals, seq, prefix);
        }
        if let NodeKind::Stmt(StmtKind::Foreach { body, .. }) = self.kind(node) {
            // Foreach iterator variables have the same loop-entry lifetime;
            // omitted slots carry no declaration and are skipped naturally.
            return self.collect_func_locals(*body, inst, locals, chandle_locals, seq, prefix);
        }
        if self.is_foreach_iterator(node) {
            // Foreach iterators are declared by `lower_foreach` at loop entry,
            // not as function-entry locals.
            return Ok(());
        }
        if let NodeKind::Var { ty } = self.kind(node) {
            self.explicit_local_lifetime(node)?;
            if ty.kind == "chandle" {
                chandle_locals.entry(node).or_insert_with(|| {
                    let cname = format!("{prefix}_l{seq}");
                    *seq += 1;
                    cname
                });
                return Ok(());
            }
            if locals.contains_key(&node) {
                return Ok(());
            }
            let (w, shortreal) = if ty.kind == "string" {
                (0, false)
            } else if is_real_kind(&ty.kind) {
                (0, ty.kind == "shortreal")
            } else {
                (
                    match ty.width {
                        Some(w) if w <= LLG_MAX_WIDTH => self.effective_decl_width(node, inst, w),
                        Some(w) => {
                            return Err(format!(
                                "local `{}` is {w} bits wide; the runtime supports at most \
                         {LLG_MAX_WIDTH}",
                                self.node(node).name
                            ))
                        }
                        None => {
                            return Err(format!("local `{}` has no width", self.node(node).name))
                        }
                    },
                    false,
                )
            };
            let cname = format!("{prefix}_l{seq}");
            *seq += 1;
            locals.insert(
                node,
                (
                    cname,
                    w,
                    if w == 0 { false } else { ty.signed },
                    if w == 0 {
                        false
                    } else {
                        is_two_state_kind(&ty.kind)
                    },
                    shortreal,
                ),
            );
            return Ok(());
        }
        for c in &self.node(node).children {
            self.collect_func_locals(*c, inst, locals, chandle_locals, seq, prefix)?;
        }
        Ok(())
    }

    fn explicit_local_lifetime(&self, node: NodeId) -> Result<Option<&'static str>, String> {
        use crate::core::db::VariableLifetimeQualifier;
        match self.db.variable_lifetime_qualifier(node) {
            VariableLifetimeQualifier::None => Ok(None),
            VariableLifetimeQualifier::Static => Ok(Some("static")),
            VariableLifetimeQualifier::Automatic => Ok(Some("automatic")),
            VariableLifetimeQualifier::Ambiguous => Err(format!(
                "subprogram local `{}` has ambiguous explicit lifetime provenance",
                self.node(node).name
            )),
            VariableLifetimeQualifier::Unavailable => Err(format!(
                "cannot determine whether subprogram local `{}` has an explicit lifetime qualifier because admitted source provenance is unavailable",
                self.node(node).name
            )),
        }
    }

    pub(super) fn check_event_expression_effects(
        &self,
        expression: NodeId,
        scope_path: &str,
    ) -> Result<(), String> {
        let mut visited = HashSet::new();
        self.check_event_node(expression, scope_path, &mut visited, None)
    }

    fn check_event_node(
        &self,
        node: NodeId,
        scope_path: &str,
        visited_functions: &mut HashSet<NodeId>,
        function: Option<NodeId>,
    ) -> Result<(), String> {
        let rejected = |reason: &str| {
            Err(format!(
                "function calls in evaluated event controls are not supported in `{scope_path}`: {reason}"
            ))
        };
        match self.kind(node) {
            NodeKind::FuncCall {
                name,
                is_task,
                callee,
            } => {
                if *is_task {
                    return rejected("task calls are not read-only");
                }
                let (ft, callee_inst) = self
                    .resolve_callee_env(self.inst, name, false, *callee)
                    .map_err(|_| {
                        format!(
                            "function calls in evaluated event controls are not supported in `{scope_path}`: cannot resolve `{name}`"
                        )
                    })?;
                let (_, _, formals) = self.func_info(ft, callee_inst).map_err(|error| {
                    format!(
                        "function calls in evaluated event controls are not supported in `{scope_path}`: {error}"
                    )
                })?;
                for (formal, is_out) in formals {
                    if is_out {
                        return rejected("output/inout formals are not read-only");
                    }
                    if matches!(
                        self.kind(formal),
                        NodeKind::FuncArg {
                            direction: DbDirection::Ref,
                            const_ref: false,
                            ..
                        }
                    ) {
                        return rejected("non-const ref formals are not read-only");
                    }
                }
                if visited_functions.insert(ft) {
                    let body = self.func_body(ft).ok_or_else(|| {
                        format!(
                            "function calls in evaluated event controls are not supported in `{scope_path}`: function `{name}` has no body"
                        )
                    })?;
                    self.check_event_node(body, scope_path, visited_functions, Some(ft))?;
                }
            }
            NodeKind::SysCall { name } => {
                return rejected(&format!("system call `{name}` has no pure effect summary"));
            }
            NodeKind::MethodCall { name, .. } => {
                return rejected(&format!("method call `{name}` has no pure effect summary"));
            }
            NodeKind::Stmt(
                StmtKind::DelayControl { .. }
                | StmtKind::EventControl { .. }
                | StmtKind::Wait { .. }
                | StmtKind::WaitOrder { .. }
                | StmtKind::EventTrigger { .. }
                | StmtKind::Force { .. }
                | StmtKind::Release { .. }
                | StmtKind::ProcContAssign { .. }
                | StmtKind::Fork { .. }
                | StmtKind::WaitFork
                | StmtKind::DisableFork,
            ) => return rejected("timing, event, or scheduler effects are not allowed"),
            NodeKind::Stmt(StmtKind::Assign { .. }) => {
                let lhs = self.node(node).children.first().copied().ok_or_else(|| {
                    format!(
                        "function calls in evaluated event controls are not supported in `{scope_path}`: malformed assignment"
                    )
                })?;
                if !self.event_local_write_allowed(function, lhs) {
                    return rejected("function body writes external or persistent storage");
                }
            }
            NodeKind::Expr(ExprKind::Operation {
                op:
                    Operation::PostIncrement
                    | Operation::PreIncrement
                    | Operation::PostDecrement
                    | Operation::PreDecrement
                    | Operation::Assignment,
                operands,
                ..
            }) => {
                let lhs = operands.first().copied().ok_or_else(|| {
                    format!(
                        "function calls in evaluated event controls are not supported in `{scope_path}`: malformed assignment expression"
                    )
                })?;
                if !self.event_local_write_allowed(function, lhs) {
                    return rejected("function body writes external or persistent storage");
                }
            }
            _ => {}
        }
        for child in &self.node(node).children {
            self.check_event_node(*child, scope_path, visited_functions, function)?;
        }
        Ok(())
    }

    fn event_local_write_allowed(&self, function: Option<NodeId>, lhs: NodeId) -> bool {
        let Some(function) = function else {
            return false;
        };
        let target = match self.kind(lhs) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            NodeKind::Expr(
                ExprKind::BitSelect { base, .. }
                | ExprKind::PartSelect { base, .. }
                | ExprKind::IndexedPartSelect { base, .. }
                | ExprKind::ArraySelect { base, .. },
            ) => match self.kind(*base) {
                NodeKind::Expr(ExprKind::Ref { target }) => *target,
                _ => None,
            },
            NodeKind::Var { .. } => Some(lhs),
            _ => None,
        };
        let Some(target) = target else {
            return false;
        };
        let target = self.canonical_func_target(target).unwrap_or(target);
        if target == function {
            return true;
        }
        if !self.node_is_within(target, function) {
            return false;
        }
        match self.kind(target) {
            NodeKind::FuncArg {
                direction: DbDirection::Input,
                ..
            }
            | NodeKind::Var { .. } => self.db.variable_lifetime(target) != VariableLifetime::Static,
            _ => false,
        }
    }

    /// Resolve a call site's callee and its owning elaborated environment.
    /// The captured semantic target is authoritative: its parent chain gives
    /// the concrete module/interface instance or the single shared package.
    /// Name lookup remains only for local calls whose frontend snapshot did
    /// not retain a callee edge.
    pub(super) fn resolve_callee_env(
        &self,
        inst: NodeId,
        name: &str,
        is_task: bool,
        callee: Option<NodeId>,
    ) -> Result<(NodeId, NodeId), String> {
        if let Some(ft) = callee {
            if !matches!(self.kind(ft), NodeKind::FuncTask { is_task: t, .. } if *t == is_task) {
                return Err(format!(
                    "callee `{name}` has an incompatible function/task kind"
                ));
            }
            let environment = self.callable_environment(ft).ok_or_else(|| {
                format!(
                    "callee `{name}` has no elaborated module, interface, or package environment"
                )
            })?;
            return Ok((ft, environment));
        }
        for c in &self.node(inst).children {
            if let NodeKind::FuncTask { is_task: t, .. } = self.kind(*c) {
                if *t == is_task && self.node(*c).name == name {
                    return Ok((*c, inst));
                }
            }
        }
        Err(format!(
            "cannot resolve callee `{name}` in `{}`",
            self.node(inst).name
        ))
    }

    /// The concrete environment that owns one captured subroutine clone.
    /// This is structural and never reconstructed from a display name, so
    /// sibling instances and package users cannot alias.
    fn callable_environment(&self, ft: NodeId) -> Option<NodeId> {
        let mut current = self.node(ft).parent;
        while let Some(id) = current {
            if matches!(self.kind(id), NodeKind::ModuleInst { .. })
                || self.is_runtime_environment(id)
            {
                return Some(id);
            }
            current = self.node(id).parent;
        }
        None
    }

    /// Whether a task's execution can suspend: its body (transitively over
    /// called tasks) contains a delay, event control or wait statement.
    /// Delay-bearing tasks are inlined at their call sites; the others become
    /// plain C functions.
    pub(super) fn task_has_wait(&self, ft: NodeId, inst: NodeId) -> bool {
        let mut seen: HashSet<NodeId> = HashSet::new();
        self.task_has_wait_inner(ft, inst, &mut seen)
    }

    /// Whether a task can cancel its activation through a named `disable`.
    /// Such tasks are lowered inline so output/inout copy-out remains inside
    /// the cancellation boundary instead of running after a C-call returns.
    pub(super) fn task_has_disable(&self, ft: NodeId, inst: NodeId) -> bool {
        let mut seen: HashSet<NodeId> = HashSet::new();
        self.task_has_disable_inner(ft, inst, &mut seen)
    }

    /// Whether a task declaration is the target of an explicit `disable`.
    ///
    /// A direct C-call has no cancellation result in its typed ABI. If an
    /// external disable can name the task, keep the call-site expansion so
    /// cancellation unwinds before output/inout copy-out. This is deliberately
    /// a declaration-level check: every invocation shares the same runtime
    /// activation identity and therefore needs the same lowering boundary.
    pub(super) fn task_is_disable_target(&self, ft: NodeId) -> bool {
        self.db.node_ids().any(|node| {
            matches!(
                self.kind(node),
                NodeKind::Stmt(StmtKind::Disable {
                    target: Some(target)
                }) if *target == ft
            )
        })
    }

    /// Whether an NBA in `node` targets subroutine storage which does not
    /// outlive the generated C call. Static formals, locals and return
    /// variables use persistent storage; every automatic formal/local is
    /// call-stack storage.
    pub(super) fn node_has_stack_backed_subroutine_nba(
        &self,
        node: NodeId,
        subroutine: NodeId,
        automatic: bool,
    ) -> bool {
        if let NodeKind::Stmt(StmtKind::For { body, .. }) = self.kind(node) {
            // IEEE 1800-2009 §12.7 restricts for-initialization to variable
            // assignments and for-step assignments to operator assignments,
            // increment/decrement expressions, or function calls. Inline
            // declaration initializers are emitted as blocking assignments;
            // lower_for deliberately emits those assignments as blocking.
            // Only the loop body can therefore contain an NBA owned by this
            // subroutine.
            return self.node_has_stack_backed_subroutine_nba(*body, subroutine, automatic);
        }
        if let NodeKind::Stmt(StmtKind::Assign {
            blocking: false, ..
        }) = self.kind(node)
        {
            if let Some(lhs) = self.node(node).children.first().copied() {
                let target = match self.kind(lhs) {
                    NodeKind::Expr(ExprKind::Ref { target }) => *target,
                    NodeKind::Expr(ExprKind::BitSelect { base, .. })
                    | NodeKind::Expr(ExprKind::PartSelect { base, .. })
                    | NodeKind::Expr(ExprKind::IndexedPartSelect { base, .. })
                    | NodeKind::Expr(ExprKind::ArraySelect { base, .. }) => {
                        match self.kind(*base) {
                            NodeKind::Expr(ExprKind::Ref { target }) => *target,
                            _ => Some(*base),
                        }
                    }
                    _ => None,
                };
                let declaration = target.or_else(|| {
                    let name = self
                        .node(lhs)
                        .name
                        .split_once('[')
                        .map_or(self.node(lhs).name.as_str(), |(name, _)| name);
                    self.subroutine_storage_named(subroutine, name)
                });
                let mut current = declaration;
                while let Some(target) = current {
                    if target == subroutine {
                        return declaration.is_some_and(|declaration| {
                            match self.kind(declaration) {
                                NodeKind::FuncArg { .. } => automatic,
                                NodeKind::Var { .. } => {
                                    self.db.variable_lifetime(declaration)
                                        == VariableLifetime::Automatic
                                }
                                NodeKind::Array { .. } => true,
                                _ => false,
                            }
                        });
                    }
                    current = self.node(target).parent;
                }
            }
        }
        self.node(node)
            .children
            .iter()
            .any(|child| self.node_has_stack_backed_subroutine_nba(*child, subroutine, automatic))
    }

    fn subroutine_storage_named(&self, node: NodeId, name: &str) -> Option<NodeId> {
        for child in &self.node(node).children {
            if matches!(
                self.kind(*child),
                NodeKind::FuncArg { .. } | NodeKind::Var { .. } | NodeKind::Array { .. }
            ) && self.node(*child).name == name
            {
                return Some(*child);
            }
            if let Some(found) = self.subroutine_storage_named(*child, name) {
                return Some(found);
            }
        }
        None
    }

    fn collect_subroutine_decl_initializers(
        &self,
        node: NodeId,
        initializers: &mut HashMap<NodeId, NodeId>,
    ) {
        if let NodeKind::Stmt(StmtKind::For { body, .. }) = self.kind(node) {
            // A for initializer can also expose a direct Var LHS, but it must
            // execute on loop entry rather than initialize function storage.
            self.collect_subroutine_decl_initializers(*body, initializers);
            return;
        }
        if matches!(self.kind(node), NodeKind::Stmt(StmtKind::Assign { .. })) {
            if let [lhs, rhs, ..] = self.node(node).children.as_slice() {
                if matches!(self.kind(*lhs), NodeKind::Var { .. }) {
                    initializers.insert(*lhs, *rhs);
                    return;
                }
            }
        }
        for child in &self.node(node).children {
            self.collect_subroutine_decl_initializers(*child, initializers);
        }
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

    fn task_has_disable_inner(&self, ft: NodeId, inst: NodeId, seen: &mut HashSet<NodeId>) -> bool {
        if !seen.insert(ft) {
            return false;
        }
        let Some(body) = self.func_body(ft) else {
            return false;
        };
        self.node_has_disable(body, inst, seen)
    }

    fn node_has_disable(&self, node: NodeId, inst: NodeId, seen: &mut HashSet<NodeId>) -> bool {
        match self.kind(node) {
            NodeKind::Stmt(StmtKind::Disable { .. }) => true,
            NodeKind::FuncCall {
                is_task: true,
                callee,
                ..
            } => {
                if let Ok((ft, callee_inst)) =
                    self.resolve_callee_env(inst, &self.node(node).name, true, *callee)
                {
                    if self.task_has_disable_inner(ft, callee_inst, seen) {
                        return true;
                    }
                }
                self.node(node)
                    .children
                    .iter()
                    .any(|c| self.node_has_disable(*c, inst, seen))
            }
            _ => self
                .node(node)
                .children
                .iter()
                .any(|c| self.node_has_disable(*c, inst, seen)),
        }
    }

    fn node_has_wait(&self, node: NodeId, inst: NodeId, seen: &mut HashSet<NodeId>) -> bool {
        match self.kind(node) {
            NodeKind::Stmt(
                StmtKind::DelayControl { .. }
                | StmtKind::EventControl { .. }
                | StmtKind::Wait { .. }
                | StmtKind::WaitOrder { .. },
            ) => true,
            NodeKind::FuncCall {
                is_task: true,
                callee,
                ..
            } => {
                if let Ok((ft, callee_inst)) =
                    self.resolve_callee_env(inst, &self.node(node).name, true, *callee)
                {
                    if self.task_has_wait_inner(ft, callee_inst, seen) {
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

    /// A call argument a frontend can synthesize for a *missing named* argument: a
    /// location-less `0` constant (genuine `0` literals carry a source line).
    fn is_synthetic_arg(&self, a: NodeId) -> bool {
        if self.node(a).line != 0 {
            return false;
        }
        match self.kind(a) {
            NodeKind::Expr(ExprKind::Constant { value, .. }) => matches!(
                value,
                ValueData::Int(0)
                    | ValueData::UInt(0)
                    | ValueData::Scalar(crate::core::value::ScalarValue::Zero)
            ),
            _ => false,
        }
    }

    /// Bind a call's positional arguments to the callee's formals, in formal
    /// order. Missing (or frontend-synthesized) arguments fall back to the
    /// formal's default expression; a formal without a default errors.
    pub(super) fn bind_call_args(
        &self,
        inst: NodeId,
        formals: &[(NodeId, bool)],
        args: &[NodeId],
    ) -> Result<Vec<BoundArg>, String> {
        let mut bound = Vec::with_capacity(formals.len());
        for (idx, (io, is_out)) in formals.iter().enumerate() {
            let (w, s, two_state, real, shortreal, is_event, is_string) = match self.kind(*io) {
                NodeKind::FuncArg { ty, .. } => {
                    if ty.kind == "event" {
                        (0, false, false, false, false, true, false)
                    } else if ty.kind == "chandle" {
                        (0, false, false, false, false, false, false)
                    } else if ty.kind == "string" {
                        (0, false, false, false, false, false, true)
                    } else {
                        if is_real_kind(&ty.kind) {
                            (0, false, false, true, ty.kind == "shortreal", false, false)
                        } else {
                            match ty.width {
                                Some(w) if w <= LLG_MAX_WIDTH => (
                                    self.effective_decl_width(*io, inst, w),
                                    ty.signed,
                                    self.db.is_two_state_type(*io) || is_two_state_kind(&ty.kind),
                                    false,
                                    false,
                                    false,
                                    false,
                                ),
                                Some(w) => {
                                    return Err(format!(
                                        "formal `{}` is {w} bits wide; the runtime supports \
                             at most {LLG_MAX_WIDTH}",
                                        self.node(*io).name
                                    ))
                                }
                                None => {
                                    return Err(format!(
                                        "formal `{}` has no width",
                                        self.node(*io).name
                                    ))
                                }
                            }
                        }
                    }
                }
                _ => unreachable!("non-FuncArg in formals"),
            };
            let default = match self.kind(*io) {
                NodeKind::FuncArg { default, .. } => *default,
                _ => unreachable!(),
            };
            let (mut expr, is_default) = match args.get(idx) {
                // Slang inserts the formal's owned default expression node
                // directly into the elaborated call argument list.
                Some(a) if Some(*a) == default => (*a, true),
                Some(a) if !self.is_synthetic_arg(*a) => (*a, false),
                _ => (
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
            };
            if *is_out && !is_default {
                if let NodeKind::Expr(ExprKind::Operation { op, operands, .. }) = self.kind(expr) {
                    if *op == Operation::Assignment {
                        let [actual, _converted] = operands.as_slice() else {
                            return Err(format!(
                                "output argument for formal `{}` has a malformed assignment wrapper",
                                self.node(*io).name
                            ));
                        };
                        expr = *actual;
                    }
                }
            }
            bound.push(BoundArg {
                width: w,
                signed: s,
                two_state,
                real,
                shortreal,
                string: is_string,
                expr,
                is_default,
                is_event,
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
    pub(super) fn lower_bound_arg_code(
        &mut self,
        scope_path: &str,
        formals: &[(NodeId, bool)],
        bound: &[BoundArg],
        idx: usize,
        arg_codes: &mut [Option<String>],
        arg_irs: &mut Vec<Option<IrExpr>>,
    ) -> Result<(String, IrExpr), String> {
        let (w, s, two_state) = (bound[idx].width, bound[idx].signed, bound[idx].two_state);
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
                            two_state: bound[j].two_state,
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
                event_args: HashMap::new(),
                arg_dependencies: HashMap::new(),
                arg_write: HashMap::new(),
                arg_lhs: HashMap::new(),
                const_refs: HashSet::new(),
                const_ref_lhs: HashMap::new(),
                persistent: HashMap::new(),
                chandle_read: HashMap::new(),
                chandle_write: HashMap::new(),
                string_read: HashMap::new(),
                string_write: HashMap::new(),
                string_addr: HashMap::new(),
                locals: HashMap::new(),
                ret_node: None,
                // Formal defaults carry exact references to the same formal
                // NodeIds used as keys above. Avoid remapping them through an
                // executable function context while arguments are being
                // assembled.
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
        let conv_ir = if bound[idx].real {
            IrExpr::new(
                IrExprKind::CastToReal {
                    a: Box::new(e_ir),
                    shortreal: bound[idx].shortreal,
                },
                0,
                false,
                None,
            )
        } else {
            ir_to_storage(e_ir, w, s, two_state)?
        };
        let code = self.render_ir_code(&conv_ir)?;
        arg_codes[idx] = Some(code.clone());
        if arg_irs.len() <= idx {
            arg_irs.resize(idx + 1, None);
        }
        arg_irs[idx] = Some(conv_ir.clone());
        Ok((code, conv_ir))
    }

    fn ir_constant_i128(value: &IrExpr) -> Option<i128> {
        let IrExprKind::Const(value) = value.kind() else {
            return None;
        };
        if value.real_value().is_some()
            || value.x_mask().iter().any(|mask| *mask != 0)
            || value.z_mask().iter().any(|mask| *mask != 0)
            || value.width() > 128
        {
            return None;
        }
        let low = value.bits().first().copied().unwrap_or(0) as u128;
        let high = value.bits().get(1).copied().unwrap_or(0) as u128;
        let raw = low | (high << 64);
        if value.signed() && value.width() < 128 && value.width() != 0 {
            let sign = 1u128 << (value.width() - 1);
            if raw & sign != 0 {
                return Some((raw | (!0u128 << value.width())) as i128);
            }
        }
        Some(raw as i128)
    }

    pub(super) fn array_constant_linear_index(
        array: &ArrayInfo,
        indices: &[IrExpr],
    ) -> Option<u64> {
        if indices.len() != array.dims.len() {
            return None;
        }
        let mut linear = 0u64;
        for ((left, right), index) in array.dims.iter().zip(indices) {
            let index = Self::ir_constant_i128(index)?;
            let lo = i128::from((*left).min(*right));
            let hi = i128::from((*left).max(*right));
            if index < lo || index > hi {
                return None;
            }
            let offset = if left >= right {
                i128::from(*left) - index
            } else {
                index - i128::from(*left)
            };
            let extent = (i64::from(*left) - i64::from(*right)).unsigned_abs() + 1;
            linear = linear.checked_mul(extent)?.checked_add(offset as u64)?;
        }
        Some(linear)
    }

    /// Lower a `ref` actual to a checked canonical descriptor.  Unlike an
    /// output/inout binding this never creates a temporary or a writeback:
    /// the descriptor names the caller's original storage directly.
    fn lower_ref_actual_lhs(&mut self, scope_path: &str, node: NodeId) -> Result<IrLhs, String> {
        let Some(function) = self.func.as_ref() else {
            return self.lower_lhs(scope_path, node);
        };
        let target = self.canonical_func_target(node).unwrap_or(node);
        let target = if function.const_refs.contains(&target) {
            Some(target)
        } else {
            let name = self.node(node).name.as_str();
            function
                .const_refs
                .iter()
                .find(|formal| self.node(**formal).name == name)
                .copied()
        };
        let Some(target) = target else {
            return self.lower_lhs(scope_path, node);
        };
        if let Some(lhs) = function.const_ref_lhs.get(&target) {
            return self.lhs_to_ir(lhs.clone());
        }
        let Some(arg) = function.arg_ir.get(&target) else {
            return Err(format!(
                "const ref actual `{}` has no canonical descriptor in `{scope_path}`",
                self.node(node).name
            ));
        };
        let IrExprKind::FormalRead(index) = arg.kind() else {
            return Err(format!(
                "const ref actual `{}` has no canonical descriptor in `{scope_path}`",
                self.node(node).name
            ));
        };
        let info = function.arg_read.get(&target).ok_or_else(|| {
            format!(
                "const ref actual `{}` has no type metadata in `{scope_path}`",
                self.node(node).name
            )
        })?;
        Ok(IrLhs::Ref {
            addr: format!("r{index}"),
            width: info.width,
            signed: info.signed,
            two_state: info.two_state,
            const_ref: true,
        })
    }

    pub(super) fn lower_ref_arg(
        &mut self,
        scope_path: &str,
        bound: &BoundArg,
        const_ref: bool,
        ref_static: bool,
    ) -> Result<IrCallArg, String> {
        if bound.is_default {
            return Err(format!(
                "ref formal cannot use a default argument in `{scope_path}`"
            ));
        }
        if ref_static {
            let target = match self.kind(bound.expr) {
                NodeKind::Var { .. } => Some(bound.expr),
                NodeKind::Expr(ExprKind::Ref { target }) => *target,
                NodeKind::Expr(
                    ExprKind::BitSelect { base, .. }
                    | ExprKind::PartSelect { base, .. }
                    | ExprKind::IndexedPartSelect { base, .. }
                    | ExprKind::ArraySelect { base, .. },
                ) => match self.kind(*base) {
                    NodeKind::Expr(ExprKind::Ref { target }) => *target,
                    NodeKind::Var { .. } | NodeKind::Array { .. } => Some(*base),
                    _ => None,
                },
                _ => None,
            }
            .map(|target| self.canonical_func_target(target).unwrap_or(target));
            match target.map(|target| self.db.variable_lifetime(target)) {
                Some(VariableLifetime::Static) => {}
                Some(VariableLifetime::Automatic) => {
                    return Err(format!(
                        "ref static actual in `{scope_path}` cannot bind automatic storage"
                    ));
                }
                Some(VariableLifetime::Unavailable) | None => {
                    return Err(format!(
                        "ref static actual in `{scope_path}` has unavailable storage lifetime"
                    ));
                }
            }
        }

        // Queue elements cannot be represented by a stable `sv4_t *`: any
        // structural queue edit can reallocate or shift the backing storage.
        // Keep the evaluated index and queue epoch in the ref descriptor so
        // the runtime can reject stale reads/writes deterministically.
        let queue_actual = match self.kind(bound.expr) {
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => self
                .container_of(*base)
                .filter(|container| {
                    matches!(
                        self.model.containers[container.ir].kind,
                        IrContainerKind::Queue { .. }
                    )
                })
                .map(|container| (container.ir, *index)),
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) if indices.len() == 1 => self
                .container_of(*base)
                .filter(|container| {
                    matches!(
                        self.model.containers[container.ir].kind,
                        IrContainerKind::Queue { .. }
                    )
                })
                .map(|container| (container.ir, indices[0])),
            _ => None,
        };
        if let Some((container, index_node)) = queue_actual {
            let (width, signed, two_state) = match self.model.containers[container].element {
                IrContainerElement::Packed {
                    width,
                    signed,
                    two_state,
                } => (width, signed, two_state),
                _ => {
                    return Err(format!(
                        "ref actual queue element in `{scope_path}` must have a packed integral type"
                    ));
                }
            };
            if width == 0 {
                return Err(format!(
                    "ref actual queue element in `{scope_path}` has zero width"
                ));
            }
            if (width, signed, two_state) != (bound.width, bound.signed, bound.two_state) {
                return Err(format!(
                    "ref actual type does not exactly match formal in `{scope_path}`"
                ));
            }
            let index = self.lower_queue_index(scope_path, container, index_node)?;
            let index_code = self.render_ir_code(&index)?;
            let queue = &self.model.containers[container];
            let lhs = IrLhs::WholeRef {
                // This typed placeholder is used for dependency/type analysis;
                // the emitted descriptor deliberately uses `.queue` instead
                // of this address because the data pointer is relocatable.
                addr: format!("&{}.data[sv4_to_index({index_code})]", queue.c_name),
                width,
                signed,
                two_state,
                shortreal: false,
            };
            let descriptor = format!(
                "&(llg_ref_t){{ .queue = &{}, \
                 .queue_identity = llg_queue_ref_identity(&{}, sv4_to_index({index_code})), \
                 .queue_read = llg_queue_ref_read, .queue_write = llg_queue_ref_write, \
                 .width = {width}, .is_signed = {}, .two_state = {}, \
                 .kind = LLG_REF_QUEUE }}",
                queue.c_name, queue.c_name, signed as u8, two_state as u8
            );
            return Ok(IrCallArg::RefAddr {
                addr: descriptor,
                width,
                signed,
                two_state,
                const_ref: false,
                lhs: Box::new(lhs),
                read: Box::new(self.lower_expr(scope_path, bound.expr)?),
            });
        }

        let lhs = self.lower_ref_actual_lhs(scope_path, bound.expr)?;
        let read = self.lower_expr(scope_path, bound.expr)?;
        let (base, width, signed, two_state, actual_const, kind, fields) = match &lhs {
            IrLhs::Whole(index) => {
                let signal = self.model.signals.get(*index).ok_or_else(|| {
                    format!("reference actual signal {index} is out of bounds in `{scope_path}`")
                })?;
                if signal.net_driver.is_some() {
                    return Err(format!(
                        "ref actual in `{scope_path}` must be a variable, not a net"
                    ));
                }
                let IrType::Packed {
                    width,
                    signed,
                    two_state,
                } = signal.ty
                else {
                    return Err(format!(
                        "ref actual in `{scope_path}` must have a packed integral type"
                    ));
                };
                (
                    format!("&{}", signal.c_name),
                    width,
                    signed,
                    two_state,
                    false,
                    "LLG_REF_WHOLE",
                    String::new(),
                )
            }
            IrLhs::WholeRef {
                addr,
                width,
                signed,
                two_state,
                ..
            } => (
                addr.clone(),
                *width,
                *signed,
                *two_state,
                false,
                "LLG_REF_WHOLE",
                String::new(),
            ),
            IrLhs::Ref {
                addr,
                width,
                signed,
                two_state,
                const_ref: actual_const,
            } => (
                addr.clone(),
                *width,
                *signed,
                *two_state,
                *actual_const,
                "LLG_REF_NESTED",
                String::new(),
            ),
            IrLhs::Bit(index, bit, two_state) => {
                let signal = self.model.signals.get(*index).ok_or_else(|| {
                    format!("reference actual signal {index} is out of bounds in `{scope_path}`")
                })?;
                if signal.net_driver.is_some() {
                    return Err(format!("ref actual in `{scope_path}` must be a variable"));
                }
                let IrType::Packed { .. } = signal.ty else {
                    return Err(format!("ref actual in `{scope_path}` must be integral"));
                };
                let bit = self.render_ir_code(bit)?;
                (
                    format!("&{}", signal.c_name),
                    1,
                    false,
                    *two_state,
                    false,
                    "LLG_REF_BIT",
                    format!(".index = sv4_to_index({bit})"),
                )
            }
            IrLhs::Part(index, left, right, two_state) => {
                let signal = self.model.signals.get(*index).ok_or_else(|| {
                    format!("reference actual signal {index} is out of bounds in `{scope_path}`")
                })?;
                if signal.net_driver.is_some() {
                    return Err(format!("ref actual in `{scope_path}` must be a variable"));
                }
                let IrType::Packed { .. } = signal.ty else {
                    return Err(format!("ref actual in `{scope_path}` must be integral"));
                };
                let width = left.abs_diff(*right) as u32 + 1;
                (
                    format!("&{}", signal.c_name),
                    width,
                    false,
                    *two_state,
                    false,
                    "LLG_REF_PART",
                    format!(".left = {left}, .right = {right}"),
                )
            }
            IrLhs::IdxPart(index, base_index, _, width, negative, two_state) => {
                let signal = self.model.signals.get(*index).ok_or_else(|| {
                    format!("reference actual signal {index} is out of bounds in `{scope_path}`")
                })?;
                if signal.net_driver.is_some() {
                    return Err(format!("ref actual in `{scope_path}` must be a variable"));
                }
                let IrType::Packed { .. } = signal.ty else {
                    return Err(format!("ref actual in `{scope_path}` must be integral"));
                };
                let base_index = self.render_ir_code(base_index)?;
                (
                    format!("&{}", signal.c_name),
                    *width,
                    false,
                    *two_state,
                    false,
                    "LLG_REF_INDEXED",
                    format!(
                        ".index = sv4_to_index({base_index}), .indexed_width = {width}, \
                         .indexed_negative = {}",
                        *negative as u8
                    ),
                )
            }
            IrLhs::ArrayElem {
                arr,
                indices,
                elem_sel,
            } => {
                let array = self.model.arrays.get(*arr).ok_or_else(|| {
                    format!("reference actual array {arr} is out of bounds in `{scope_path}`")
                })?;
                let array_info = ArrayInfo {
                    global: array.c_name.clone(),
                    elem_width: array.elem_width,
                    signed: array.signed,
                    real: array.real,
                    shortreal: array.shortreal,
                    is_net: false,
                    dims: array.dims.clone(),
                    init: None,
                    ir: *arr,
                };
                let constant_linear = Self::array_constant_linear_index(&array_info, indices);
                if matches!(elem_sel, IrElemSel::Whole) && constant_linear.is_none() {
                    if indices
                        .iter()
                        .all(|index| Self::ir_constant_i128(index).is_some())
                    {
                        return Err(format!(
                            "ref actual array index in `{scope_path}` must be a constant in range"
                        ));
                    }
                    let index_codes = indices
                        .iter()
                        .map(|index| self.render_ir_code(index))
                        .collect::<Result<Vec<_>, _>>()?;
                    let (decls, condition, linear) =
                        array_guard(array, &index_codes).ok_or_else(|| {
                            format!("ref actual array index in `{scope_path}` has no dimensions")
                        })?;
                    let index = format!(
                        "({{ {decls} ({condition}) ? (uint64_t)({linear}) : UINT64_MAX; }})"
                    );
                    (
                        array.c_name.clone(),
                        array.elem_width,
                        array.signed,
                        array.two_state,
                        false,
                        "LLG_REF_ARRAY",
                        format!(".array_size = {}ULL, .index = {index}", array.total),
                    )
                } else {
                    let linear = Self::array_constant_linear_index(&array_info, indices)
                        .ok_or_else(|| {
                            format!(
                        "ref actual array index in `{scope_path}` must be a constant in range"
                    )
                        })?;
                    let base = format!("&{}[{}]", array.c_name, linear);
                    match elem_sel {
                        IrElemSel::Whole => (
                            base,
                            array.elem_width,
                            array.signed,
                            array.two_state,
                            false,
                            "LLG_REF_WHOLE",
                            String::new(),
                        ),
                        IrElemSel::Part(left, right) => (
                            base,
                            left.abs_diff(*right) as u32 + 1,
                            false,
                            array.two_state,
                            false,
                            "LLG_REF_PART",
                            format!(".left = {left}, .right = {right}"),
                        ),
                        IrElemSel::Bit(index) => {
                            let index = self.render_ir_code(index)?;
                            (
                                base,
                                1,
                                false,
                                array.two_state,
                                false,
                                "LLG_REF_BIT",
                                format!(".index = sv4_to_index({index})"),
                            )
                        }
                        IrElemSel::Indexed {
                            base: index,
                            width,
                            negative,
                        } => {
                            let index = self.render_ir_code(index)?;
                            (
                                base,
                                *width,
                                false,
                                array.two_state,
                                false,
                                "LLG_REF_INDEXED",
                                format!(
                                    ".index = sv4_to_index({index}), .indexed_width = {width}, \
                                 .indexed_negative = {}",
                                    *negative as u8
                                ),
                            )
                        }
                    }
                }
            }
            IrLhs::Stream { .. } => {
                return Err(format!(
                    "streaming concatenation is not a legal ref actual in `{scope_path}`"
                ));
            }
        };
        if width != bound.width || signed != bound.signed || two_state != bound.two_state {
            return Err(format!(
                "ref actual type does not exactly match formal in `{scope_path}`"
            ));
        }
        if actual_const && !const_ref {
            return Err(format!(
                "const ref actual cannot bind to writable ref formal in `{scope_path}`"
            ));
        }
        let descriptor = if kind == "LLG_REF_NESTED" {
            base
        } else {
            format!(
                "&(llg_ref_t){{ .base = {base}, .width = {width}, .is_signed = {}, \
                 .two_state = {}, .kind = {kind}, {fields} }}",
                signed as u8, two_state as u8
            )
        };
        Ok(IrCallArg::RefAddr {
            addr: descriptor,
            width,
            signed,
            two_state,
            const_ref: actual_const,
            lhs: Box::new(lhs),
            read: Box::new(read),
        })
    }

    pub(super) fn ref_lhs_type(&self, lhs: &IrLhs) -> Option<(u32, bool, bool, bool)> {
        match lhs {
            IrLhs::Whole(index) => match self.model.signal(*index).ty {
                IrType::Packed {
                    width,
                    signed,
                    two_state,
                } => Some((width, signed, two_state, false)),
                IrType::Real { .. } => None,
            },
            IrLhs::WholeRef {
                width,
                signed,
                two_state,
                ..
            } => Some((*width, *signed, *two_state, false)),
            IrLhs::Ref {
                width,
                signed,
                two_state,
                const_ref,
                ..
            } => Some((*width, *signed, *two_state, *const_ref)),
            IrLhs::Bit(_, _, two_state) => Some((1, false, *two_state, false)),
            IrLhs::Part(_, left, right, two_state) => {
                Some((left.abs_diff(*right) as u32 + 1, false, *two_state, false))
            }
            IrLhs::IdxPart(_, _, _, width, _, two_state) => {
                Some((*width, false, *two_state, false))
            }
            IrLhs::ArrayElem { arr, elem_sel, .. } => {
                let array = self.model.arrays.get(*arr)?;
                let (width, signed) = match elem_sel {
                    IrElemSel::Whole => (array.elem_width, array.signed),
                    IrElemSel::Part(left, right) => (left.abs_diff(*right) as u32 + 1, false),
                    IrElemSel::Bit(_) => (1, false),
                    IrElemSel::Indexed { width, .. } => (*width, false),
                };
                Some((width, signed, array.two_state, false))
            }
            IrLhs::Stream { .. } => None,
        }
    }

    /// Lower a `func_call` expression used as a value: `fn_<callee>(<args>,
    /// <depth>)` with output/inout formals bound to caller-side temps that
    /// are written back into the bound actuals after the call (the backend
    /// wraps those into one GNU statement expression).
    pub(super) fn lower_func_call_expr(
        &mut self,
        scope_path: &str,
        h: NodeId,
        name: &str,
        callee: Option<NodeId>,
    ) -> Result<IrExpr, String> {
        let (ft, callee_inst) = self.resolve_callee_env(self.inst, name, false, callee)?;
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
        let bound = self.bind_call_args(self.inst, &formals, &args)?;
        for (idx, (io, _is_out)) in formals.iter().enumerate() {
            if bound[idx].is_event {
                return Err(format!(
                    "event formal `{}` in function expression `{name}` has no typed value call path",
                    self.node(*io).name
                ));
            }
        }
        // `ret` is `None` for void functions; when the frontend accepts one as
        // a value (for example `out <= vf(4'd2);`), emit the call for its
        // side effects and yields all-X.
        let ret_val = meta.ret;
        let (ret_w, ret_s, _, _) = ret_val.unwrap_or((1, false, false, false));

        let mut out_args: Vec<IrCallArg> = Vec::new();
        let mut in_args: Vec<IrCallArg> = Vec::new();
        let mut arg_codes: Vec<Option<String>> = vec![None; formals.len()];
        let mut arg_irs: Vec<Option<IrExpr>> = vec![None; formals.len()];
        for (idx, (io, is_out)) in formals.iter().enumerate() {
            if matches!(
                self.kind(*io),
                NodeKind::FuncArg { ty, .. } if ty.kind == "chandle"
            ) {
                let is_ref = matches!(
                    self.kind(*io),
                    NodeKind::FuncArg {
                        direction: DbDirection::Ref,
                        ..
                    }
                );
                if is_ref || *is_out {
                    let (target, _) = self.lower_chandle_lvalue(scope_path, bound[idx].expr)?;
                    let address = self.chandle_target_address(&target);
                    if is_ref {
                        out_args.push(IrCallArg::ChandleRefAddr(address));
                    } else {
                        out_args.push(IrCallArg::ChandleAddr(address));
                    }
                }
                continue;
            }
            let is_ref = matches!(
                self.kind(*io),
                NodeKind::FuncArg {
                    direction: DbDirection::Ref,
                    ..
                }
            );
            if is_ref {
                let (const_ref, ref_static) = match self.kind(*io) {
                    NodeKind::FuncArg {
                        direction: DbDirection::Ref,
                        const_ref,
                        ref_static,
                        ..
                    } => (*const_ref, *ref_static),
                    _ => unreachable!("ref formal"),
                };
                if bound[idx].string {
                    if !const_ref {
                        self.ensure_string_actual_writable(scope_path, bound[idx].expr)?;
                    }
                    out_args.push(IrCallArg::StringRefAddr {
                        addr: self.lower_string_actual_address(scope_path, bound[idx].expr)?,
                        const_ref,
                    });
                } else {
                    out_args.push(self.lower_ref_arg(
                        scope_path,
                        &bound[idx],
                        const_ref,
                        ref_static,
                    )?);
                }
                if !bound[idx].string {
                    let read_ir = self.lower_expr(scope_path, bound[idx].expr)?;
                    arg_codes[idx] = Some(self.render_ir_code(&read_ir)?);
                    arg_irs[idx] = Some(read_ir);
                }
                continue;
            }
            if *is_out {
                if bound[idx].string {
                    self.ensure_string_actual_writable(scope_path, bound[idx].expr)?;
                    let tname = format!("_st{}_{}", h.0, idx);
                    let writeback =
                        self.lower_string_actual_address(scope_path, bound[idx].expr)?;
                    let init = matches!(
                        self.kind(*io),
                        NodeKind::FuncArg {
                            direction: DbDirection::Inout,
                            ..
                        }
                    )
                    .then(|| self.lower_string(scope_path, bound[idx].expr))
                    .transpose()?;
                    let (storage_addr, storage_read) = self
                        .static_string_formals
                        .get(&(callee_inst, *io))
                        .map(|object| {
                            (
                                Some(format!("&{}", self.model.objects[*object].c_name)),
                                Some(Box::new(IrStringExpr::Read(*object))),
                            )
                        })
                        .unwrap_or((None, None));
                    out_args.push(IrCallArg::StringOutTemp {
                        name: tname,
                        init: init.map(Box::new),
                        writeback,
                        storage_addr,
                        storage_read,
                    });
                    continue;
                }
                let tname = format!("_t{}_{}", h.0, idx);
                let (wb, actual_read, selector_inits) =
                    self.lower_call_actual(scope_path, bound[idx].expr, &format!("{}_{idx}", h.0))?;
                let (_init_code, init_ir) =
                    self.lower_call_temp_init_from_expr(*io, &bound[idx], actual_read)?;
                let storage = self.static_formals.get(&(callee_inst, *io)).cloned();
                let (storage_addr, storage_lhs, storage_read) = if let Some(storage) = storage {
                    let lhs = IrLhs::Whole(storage.ir);
                    let read = sig_read_expr_full(&storage);
                    (
                        Some(format!("&{}", storage.global)),
                        Some(Box::new(lhs)),
                        Some(Box::new(read)),
                    )
                } else {
                    (None, None, None)
                };
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
                    storage_addr,
                    storage_lhs,
                    storage_read,
                    selector_inits,
                });
            }
        }
        for (idx, (io, is_out)) in formals.iter().enumerate() {
            let is_ref = matches!(
                self.kind(*io),
                NodeKind::FuncArg {
                    direction: DbDirection::Ref,
                    ..
                }
            );
            if !*is_out
                && !is_ref
                && matches!(self.kind(*io), NodeKind::FuncArg { ty, .. } if ty.kind == "chandle")
            {
                in_args.push(IrCallArg::ChandleVal(
                    self.lower_chandle(scope_path, bound[idx].expr)?,
                ));
            } else if !*is_out && !is_ref {
                if bound[idx].string {
                    in_args.push(IrCallArg::StringVal(
                        self.lower_string(scope_path, bound[idx].expr)?,
                    ));
                    continue;
                }
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

    /// Lower a caller-side output/inout temp initializer from an already
    /// evaluated actual read.  Callers use this after freezing selected-LHS
    /// indices so copy-in and copy-out share one actual identity.
    pub(super) fn lower_call_temp_init_from_expr(
        &self,
        io: NodeId,
        b: &BoundArg,
        e: IrExpr,
    ) -> Result<(String, Option<IrExpr>), String> {
        match self.kind(io) {
            NodeKind::FuncArg {
                direction: DbDirection::Inout,
                ..
            } => {
                let e = apply_assignment_expression_width(e, b.width);
                let conv = if b.real {
                    IrExpr::new(
                        IrExprKind::CastToReal {
                            a: Box::new(e),
                            shortreal: b.shortreal,
                        },
                        0,
                        false,
                        None,
                    )
                } else {
                    ir_to_storage(e, b.width, b.signed, b.two_state)?
                };
                let code = self.render_ir_code(&conv)?;
                Ok((code, Some(conv)))
            }
            _ if b.real => Ok(("0.0".to_string(), None)),
            _ => Ok((format!("sv4_x({}, {})", b.width, b.signed as u8), None)),
        }
    }

    /// Resolve a whole native string actual to its caller-owned C slot.
    pub(super) fn lower_string_actual_address(
        &self,
        path: &str,
        actual: NodeId,
    ) -> Result<String, String> {
        let actual = match self.kind(actual) {
            NodeKind::Expr(ExprKind::Cast { operand, ty, .. }) if ty.kind == "string" => *operand,
            _ => actual,
        };
        if matches!(
            self.kind(actual),
            NodeKind::Expr(ExprKind::BitSelect { .. })
        ) {
            return Err(format!(
                "string output/ref actual in `{path}` must be a whole string lvalue"
            ));
        }
        if let Some(index) = self.object_of(path, actual) {
            let object = self
                .model
                .objects
                .get(index)
                .ok_or_else(|| "string object index is out of bounds".to_owned())?;
            if object.ty != IrObjectType::String {
                return Err(format!("actual in `{path}` is not string storage"));
            }
            return Ok(format!("&{}", object.c_name));
        }
        if let Some((_, name)) = self.lexical_proc_string_local(actual) {
            return Ok(format!("&{name}"));
        }
        let target = match self.kind(actual) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            _ => Some(actual),
        };
        let target = self.func.as_ref().and_then(|function| {
            target
                .and_then(|target| {
                    function
                        .string_write
                        .get(&target)
                        .cloned()
                        .or_else(|| function.string_addr.get(&target).cloned())
                })
                .or_else(|| {
                    matches!(
                        self.kind(actual),
                        NodeKind::Expr(ExprKind::Ref { target: None })
                    )
                    .then(|| {
                        function
                            .string_write
                            .iter()
                            .find(|(target, _)| self.node(**target).name == self.node(actual).name)
                            .map(|(_, value)| value.clone())
                            .or_else(|| {
                                function
                                    .string_addr
                                    .iter()
                                    .find(|(target, _)| {
                                        self.node(**target).name == self.node(actual).name
                                    })
                                    .map(|(_, value)| value.clone())
                            })
                    })
                    .flatten()
                })
        });
        target
            .map(|target| format!("&{target}"))
            .ok_or_else(|| format!("string output/ref actual in `{path}` is not a writable lvalue"))
    }

    /// Reject a string output/inout or mutable-ref actual that is only a
    /// readable const-ref alias.  Address formation itself remains available
    /// for const-ref formals and therefore must not perform this check.
    pub(super) fn ensure_string_actual_writable(
        &self,
        path: &str,
        actual: NodeId,
    ) -> Result<(), String> {
        let actual = match self.kind(actual) {
            NodeKind::Expr(ExprKind::Cast { operand, ty, .. }) if ty.kind == "string" => *operand,
            _ => actual,
        };
        if let Some(index) = self.object_of(path, actual) {
            let object = self
                .model
                .objects
                .get(index)
                .ok_or_else(|| "string object index is out of bounds".to_owned())?;
            return if object.ty == IrObjectType::String {
                Ok(())
            } else {
                Err(format!("actual in `{path}` is not string storage"))
            };
        }
        let target = match self.kind(actual) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            _ => Some(actual),
        };
        let writable = self.func.as_ref().is_some_and(|function| {
            target
                .and_then(|target| function.string_write.get(&target))
                .is_some()
                || (matches!(
                    self.kind(actual),
                    NodeKind::Expr(ExprKind::Ref { target: None })
                ) && function
                    .string_write
                    .keys()
                    .any(|target| self.node(*target).name == self.node(actual).name))
        });
        if writable || self.lexical_proc_string_local(actual).is_some() {
            Ok(())
        } else {
            Err(format!(
                "string output/ref actual in `{path}` is a const-ref or non-writable lvalue"
            ))
        }
    }

    /// Lower one output/inout actual once.  Runtime selector expressions are
    /// captured in caller-side locals before the callee starts; the returned
    /// read and writeback LHS then use those locals rather than re-evaluating
    /// an index after the call.
    #[allow(clippy::type_complexity)]
    pub(super) fn lower_call_actual(
        &mut self,
        path: &str,
        actual: NodeId,
        tag: &str,
    ) -> Result<(IrLhs, IrExpr, Vec<(String, u32, bool, bool, IrExpr)>), String> {
        let lhs = self.lower_lhs(path, actual)?;
        let mut captures = Vec::new();
        let mut sequence = 0usize;
        let (lhs, read) = self.freeze_call_lhs(lhs, tag, &mut sequence, &mut captures)?;
        let read = read.unwrap_or(self.lower_expr(path, actual)?);
        Ok((lhs, read, captures))
    }

    fn freeze_call_lhs(
        &self,
        lhs: IrLhs,
        tag: &str,
        sequence: &mut usize,
        captures: &mut Vec<(String, u32, bool, bool, IrExpr)>,
    ) -> Result<(IrLhs, Option<IrExpr>), String> {
        let mut capture = |expr: IrExpr| -> Result<IrExpr, String> {
            if expr.is_real() {
                return Err(format!(
                    "real-valued selector in subroutine output/inout actual `{tag}` is not supported"
                ));
            }
            let name = format!("_call_idx_{tag}_{}", *sequence);
            *sequence += 1;
            let frozen = IrExpr::new(
                IrExprKind::LocalRead(name.clone()),
                expr.width,
                expr.signed,
                None,
            );
            captures.push((name, expr.width, expr.signed, false, expr));
            Ok(frozen)
        };
        let read_signal = |signal: usize| -> Result<IrExpr, String> {
            let ty = self
                .model
                .signals
                .get(signal)
                .ok_or_else(|| format!("subroutine actual signal {signal} is out of bounds"))?
                .ty;
            Ok(IrExpr::new(
                IrExprKind::SigRead(signal),
                ty.width(),
                ty.signed(),
                None,
            ))
        };
        match lhs {
            IrLhs::Whole(signal) => Ok((lhs, Some(read_signal(signal)?))),
            IrLhs::Bit(signal, index, two_state) => {
                let index = capture(index)?;
                let read = IrExpr::new(
                    IrExprKind::BitSel {
                        base: Box::new(read_signal(signal)?),
                        idx: Box::new(index.clone()),
                    },
                    1,
                    false,
                    None,
                );
                Ok((IrLhs::Bit(signal, index, two_state), Some(read)))
            }
            IrLhs::Part(signal, left, right, two_state) => {
                let width = left
                    .abs_diff(right)
                    .checked_add(1)
                    .and_then(|width| u32::try_from(width).ok())
                    .ok_or_else(|| "part-select width exceeds the supported range".to_string())?;
                let read = IrExpr::new(
                    IrExprKind::PartSel {
                        base: Box::new(read_signal(signal)?),
                        left,
                        right,
                    },
                    width,
                    false,
                    None,
                );
                Ok((IrLhs::Part(signal, left, right, two_state), Some(read)))
            }
            IrLhs::IdxPart(signal, base, width_expr, width, negative, two_state) => {
                let base = capture(base)?;
                let width_expr = capture(width_expr)?;
                let read = IrExpr::new(
                    IrExprKind::IdxPartSel {
                        base: Box::new(read_signal(signal)?),
                        base_idx: Box::new(base.clone()),
                        width_expr: Box::new(width_expr.clone()),
                        neg: negative,
                    },
                    width,
                    false,
                    None,
                );
                Ok((
                    IrLhs::IdxPart(signal, base, width_expr, width, negative, two_state),
                    Some(read),
                ))
            }
            IrLhs::ArrayElem {
                arr,
                indices,
                elem_sel,
            } => {
                let array = self
                    .model
                    .arrays
                    .get(arr)
                    .ok_or_else(|| format!("subroutine actual array {arr} is out of bounds"))?;
                if array.real {
                    return Err(
                        "real unpacked-array output/inout actuals are not supported".to_string()
                    );
                }
                let indices = indices
                    .into_iter()
                    .map(&mut capture)
                    .collect::<Result<Vec<_>, _>>()?;
                let elem_sel = match elem_sel {
                    IrElemSel::Whole => IrElemSel::Whole,
                    IrElemSel::Part(left, right) => IrElemSel::Part(left, right),
                    IrElemSel::Bit(index) => IrElemSel::Bit(Box::new(capture(*index)?)),
                    IrElemSel::Indexed {
                        base,
                        width,
                        negative,
                    } => IrElemSel::Indexed {
                        base: Box::new(capture(*base)?),
                        width,
                        negative,
                    },
                };
                let (width, signed) = match elem_sel {
                    IrElemSel::Whole => (array.elem_width, array.signed),
                    IrElemSel::Part(left, right) => (
                        left.abs_diff(right)
                            .checked_add(1)
                            .and_then(|width| u32::try_from(width).ok())
                            .ok_or_else(|| {
                                "array element part-select width exceeds the supported range"
                                    .to_string()
                            })?,
                        false,
                    ),
                    IrElemSel::Bit(_) => (1, false),
                    IrElemSel::Indexed { width, .. } => (width, false),
                };
                let read = IrExpr::new(
                    IrExprKind::ArrayRead {
                        arr,
                        indices: indices.clone(),
                        elem_sel: elem_sel.clone(),
                    },
                    width,
                    signed,
                    None,
                );
                Ok((
                    IrLhs::ArrayElem {
                        arr,
                        indices,
                        elem_sel,
                    },
                    Some(read),
                ))
            }
            lhs @ (IrLhs::WholeRef { .. } | IrLhs::Ref { .. } | IrLhs::Stream { .. }) => {
                Ok((lhs, None))
            }
        }
    }

    /// Resolve a function/task body write target (output/inout formal, local
    /// or return variable) to an LHS.  Tries `node` first (locals and the
    /// return var are indexed), then `name`.
    fn func_write_target(&self, node: NodeId, name: &str) -> Option<Lhs> {
        let f = self.func.as_ref()?;
        let node = self.canonical_func_target(node).unwrap_or(node);
        if let Some(lhs) = f.arg_lhs.get(&node) {
            return Some(lhs.clone());
        }
        if f.const_refs.contains(&node) {
            return None;
        }
        if let Some(info) = f.persistent.get(&node) {
            return Some(Lhs::Whole(info.clone()));
        }
        if let Some(addr) = f.arg_write.get(&node) {
            if let Some(am) = f.arg_read.get(&node) {
                return Some(Lhs::WholeRef {
                    addr: addr.clone(),
                    width: am.width,
                    signed: am.signed,
                    two_state: am.two_state,
                    shortreal: matches!(
                        self.kind(node),
                        NodeKind::FuncArg { ty, .. } if ty.kind == "shortreal"
                    ),
                });
            }
        }
        if let Some((cname, w, s, two_state, shortreal)) = f.locals.get(&node) {
            return Some(Lhs::WholeRef {
                addr: format!("&{cname}"),
                width: *w,
                signed: *s,
                two_state: *two_state,
                shortreal: *shortreal,
            });
        }
        if f.ret_node == Some(node) {
            if let Some(r) = &f.ret {
                return Some(Lhs::WholeRef {
                    addr: format!("&{}", r.c_name),
                    width: r.width,
                    signed: r.signed,
                    two_state: r.two_state,
                    shortreal: r.shortreal,
                });
            }
        }
        for (io, addr) in &f.arg_write {
            if self.node(*io).name == name {
                if let Some(info) = f.persistent.get(io) {
                    return Some(Lhs::Whole(info.clone()));
                }
                if let Some(am) = f.arg_read.get(io) {
                    return Some(Lhs::WholeRef {
                        addr: addr.clone(),
                        width: am.width,
                        signed: am.signed,
                        two_state: am.two_state,
                        shortreal: matches!(
                            self.kind(*io),
                            NodeKind::FuncArg { ty, .. } if ty.kind == "shortreal"
                        ),
                    });
                }
            }
        }
        for (io, lhs) in &f.arg_lhs {
            if self.node(*io).name == name {
                return Some(lhs.clone());
            }
        }
        for (nid, (cname, w, s, two_state, shortreal)) in &f.locals {
            if self.node(*nid).name == name {
                return Some(Lhs::WholeRef {
                    addr: format!("&{cname}"),
                    width: *w,
                    signed: *s,
                    two_state: *two_state,
                    shortreal: *shortreal,
                });
            }
        }
        if let Some(r) = &f.ret {
            if r.node.map(|n| self.node(n).name == name).unwrap_or(false) {
                return Some(Lhs::WholeRef {
                    addr: format!("&{}", r.c_name),
                    width: r.width,
                    signed: r.signed,
                    two_state: r.two_state,
                    shortreal: r.shortreal,
                });
            }
        }
        None
    }

    fn is_const_ref_target(&self, node: NodeId, name: &str) -> bool {
        let Some(func) = self.func.as_ref() else {
            return false;
        };
        let node = self.canonical_func_target(node).unwrap_or(node);
        func.const_refs.contains(&node)
            || func
                .const_refs
                .iter()
                .any(|formal| self.node(*formal).name == name)
    }

    pub(super) fn subroutine_auto_target(&self, node: NodeId) -> bool {
        self.subroutine_auto_ref(node).is_some()
    }

    /// Return the canonical automatic subroutine storage referenced by a
    /// target or expression node.  Force evaluators are emitted outside the
    /// activation that issued them, so stack-backed formals and locals may
    /// not be captured; persistent static subroutine storage remains valid.
    pub(super) fn subroutine_auto_ref(&self, node: NodeId) -> Option<NodeId> {
        let function = self.func.as_ref()?;
        if !self.function_is_automatic(function) {
            return None;
        }
        let target = match self.kind(node) {
            NodeKind::Var { .. } => node,
            NodeKind::Expr(ExprKind::Ref { target }) => target.unwrap_or(node),
            NodeKind::Expr(
                ExprKind::BitSelect { base, .. }
                | ExprKind::PartSelect { base, .. }
                | ExprKind::IndexedPartSelect { base, .. }
                | ExprKind::ArraySelect { base, .. },
            ) => match self.kind(*base) {
                NodeKind::Expr(ExprKind::Ref { target }) => target.unwrap_or(*base),
                _ => *base,
            },
            _ => return None,
        };
        let target = self.canonical_func_target(target).unwrap_or(target);
        if function.persistent.contains_key(&target) {
            return None;
        }
        (function.locals.contains_key(&target)
            || function.arg_read.contains_key(&target)
            || function.arg_write.contains_key(&target)
            || function.ret_node == Some(target))
        .then_some(target)
    }

    /// Normalize a Slang instantiated subroutine declaration identity to the
    /// definition identity used by the current function context. The match is
    /// structural: same subroutine signature plus formal ordinal. This avoids
    /// redirecting an unrelated same-named declaration.
    pub(super) fn canonical_func_target(&self, node: NodeId) -> Option<NodeId> {
        let definition = self.func.as_ref()?.def_node?;
        if node == definition {
            return Some(node);
        }
        match self.kind(node) {
            NodeKind::FuncTask { .. } => self
                .same_subroutine_signature(node, definition)
                .then_some(definition),
            NodeKind::FuncArg { .. } => {
                let owner = self.enclosing_func_task(node)?;
                if !self.same_subroutine_signature(owner, definition) {
                    return None;
                }
                let ordinal = self
                    .node(owner)
                    .children
                    .iter()
                    .filter(|child| matches!(self.kind(**child), NodeKind::FuncArg { .. }))
                    .position(|child| *child == node)?;
                self.node(definition)
                    .children
                    .iter()
                    .filter(|child| matches!(self.kind(**child), NodeKind::FuncArg { .. }))
                    .nth(ordinal)
                    .copied()
            }
            _ => None,
        }
    }

    fn enclosing_func_task(&self, node: NodeId) -> Option<NodeId> {
        let mut parent = self.node(node).parent;
        while let Some(candidate) = parent {
            if matches!(self.kind(candidate), NodeKind::FuncTask { .. }) {
                return Some(candidate);
            }
            parent = self.node(candidate).parent;
        }
        None
    }

    fn same_subroutine_signature(&self, left: NodeId, right: NodeId) -> bool {
        let same_kind = match (self.kind(left), self.kind(right)) {
            (
                NodeKind::FuncTask {
                    is_task: left_task,
                    ret: left_return,
                    ..
                },
                NodeKind::FuncTask {
                    is_task: right_task,
                    ret: right_return,
                    ..
                },
            ) => {
                left_task == right_task
                    && left_return
                        .as_ref()
                        .map(|ty| (&ty.kind, ty.width, ty.signed))
                        == right_return
                            .as_ref()
                            .map(|ty| (&ty.kind, ty.width, ty.signed))
            }
            _ => false,
        };
        if !same_kind || self.node(left).name != self.node(right).name {
            return false;
        }
        let formals = |subroutine: NodeId| {
            self.node(subroutine)
                .children
                .iter()
                .filter_map(|child| match self.kind(*child) {
                    NodeKind::FuncArg {
                        direction,
                        ty,
                        const_ref,
                        ref_static,
                        ..
                    } => Some((
                        *direction,
                        ty.kind.clone(),
                        ty.width,
                        ty.signed,
                        *const_ref,
                        *ref_static,
                    )),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        formals(left) == formals(right)
    }

    // ── PCA site pre-scan (two-phase discovery, phase 1) ─────────────────────

    /// Allocate every procedural continuous assignment site in the instance
    /// tree BEFORE any body lowers. Traverses module instances, generate
    /// scopes and per-iteration instances exactly like the Procs emission
    /// pass, so lower-time lookups into [`Codegen::pca_sites`] see every site
    /// regardless of process/source order: a `deassign` in a process that
    /// lowers BEFORE the process carrying the matching `assign` must still
    /// clear its enable (a lower-time allocation alone would turn it into a
    /// permanent no-op). Function/task definition bodies are not scanned
    /// here: they always lower before any process body, so sites inside them
    /// still allocate ahead of every process-body deassign.
    pub(super) fn prescan_pca_sites(&mut self, inst: NodeId, path: &str) -> Result<(), String> {
        for c in &self.node(inst).children {
            if matches!(self.kind(*c), NodeKind::Process { .. }) {
                self.prescan_pca_proc(inst, path, *c)?;
            }
        }
        for c in &self.node(inst).children {
            match self.kind(*c) {
                NodeKind::GenScope => self.prescan_pca_gen_scope(inst, *c, path)?,
                NodeKind::GenScopeArray => {
                    for gs in self.node(*c).children.clone() {
                        if matches!(self.kind(gs), NodeKind::GenScope) {
                            self.prescan_pca_gen_scope(inst, gs, path)?;
                        }
                    }
                }
                _ => {}
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

    fn prescan_pca_gen_scope(
        &mut self,
        inst: NodeId,
        gs: NodeId,
        parent_path: &str,
    ) -> Result<(), String> {
        let gs_path = self
            .gen_scope_paths
            .get(&gs)
            .cloned()
            .unwrap_or_else(|| parent_path.to_string());
        for child in self.node(gs).children.clone() {
            match self.kind(child) {
                NodeKind::Process { .. } => self.prescan_pca_proc(inst, &gs_path, child)?,
                NodeKind::GenScope => self.prescan_pca_gen_scope(inst, child, &gs_path)?,
                NodeKind::GenScopeArray => {
                    for nested in self.node(child).children.clone() {
                        if matches!(self.kind(nested), NodeKind::GenScope) {
                            self.prescan_pca_gen_scope(inst, nested, &gs_path)?;
                        }
                    }
                }
                NodeKind::ModuleInst { .. } => {
                    let child_path = self.instance_path_of(child);
                    self.prescan_pca_sites(child, &child_path)?;
                }
                _ => {}
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

    pub(super) fn emit_pass(&mut self, top: NodeId, pass: Pass) -> Result<(), String> {
        let path = self.instance_path_of(top);
        self.emit_pass_inst(top, &path, pass)
    }

    fn emit_pass_inst(&mut self, inst: NodeId, path: &str, pass: Pass) -> Result<(), String> {
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
                    match self.kind(*c) {
                        NodeKind::GenScope => self.emit_gen_scope(inst, *c, path, Pass::Comb)?,
                        NodeKind::GenScopeArray => {
                            for gs in self.node(*c).children.clone() {
                                if matches!(self.kind(gs), NodeKind::GenScope) {
                                    self.emit_gen_scope(inst, gs, path, Pass::Comb)?;
                                }
                            }
                        }
                        _ => {}
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
                    match self.kind(*c) {
                        NodeKind::GenScope => self.emit_gen_scope(inst, *c, path, Pass::Links)?,
                        NodeKind::GenScopeArray => {
                            for gs in self.node(*c).children.clone() {
                                if matches!(self.kind(gs), NodeKind::GenScope) {
                                    self.emit_gen_scope(inst, gs, path, Pass::Links)?;
                                }
                            }
                        }
                        _ => {}
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
                for c in &self.node(inst).children {
                    if matches!(self.kind(*c), NodeKind::Process { .. }) {
                        self.emit_process(inst, path, *c)?;
                    }
                }
                // Processes inside generate scopes are emitted exactly like
                // instance processes (mirroring the Comb pass's gen-scope
                // walk); genvar references inline to the gen-scope parameter
                // values collected by `collect_gen_scope`.
                for c in &self.node(inst).children {
                    match self.kind(*c) {
                        NodeKind::GenScope => self.emit_gen_scope(inst, *c, path, Pass::Procs)?,
                        NodeKind::GenScopeArray => {
                            for gs in self.node(*c).children.clone() {
                                if matches!(self.kind(gs), NodeKind::GenScope) {
                                    self.emit_gen_scope(inst, gs, path, Pass::Procs)?;
                                }
                            }
                        }
                        _ => {}
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

    /// Emit one elaborated generate scope, recursively preserving its concrete
    /// hierarchy path for nested scopes and generated instances.
    fn emit_gen_scope(
        &mut self,
        inst: NodeId,
        gs: NodeId,
        parent_path: &str,
        pass: Pass,
    ) -> Result<(), String> {
        let gs_path = self
            .gen_scope_paths
            .get(&gs)
            .cloned()
            .unwrap_or_else(|| parent_path.to_string());
        for child in self.node(gs).children.clone() {
            match self.kind(child) {
                NodeKind::ContAssign { .. } if pass == Pass::Comb => {
                    self.emit_cont_assign(inst, &gs_path, child)?
                }
                NodeKind::Gate { .. } if pass == Pass::Comb => {
                    self.emit_gate(inst, &gs_path, child)?
                }
                NodeKind::Process { .. } if pass == Pass::Procs => {
                    self.emit_process(inst, &gs_path, child)?
                }
                NodeKind::GenScope => self.emit_gen_scope(inst, child, &gs_path, pass)?,
                NodeKind::GenScopeArray => {
                    for nested in self.node(child).children.clone() {
                        if matches!(self.kind(nested), NodeKind::GenScope) {
                            self.emit_gen_scope(inst, nested, &gs_path, pass)?;
                        }
                    }
                }
                NodeKind::ModuleInst { .. } => {
                    let child_path = self.instance_path_of(child);
                    if pass == Pass::Links {
                        self.emit_links(&gs_path, child)?;
                    }
                    self.emit_pass_inst(child, &child_path, pass)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    // ── Continuous assignments ─────────────────────────────────────────────

    fn emit_cont_assign(&mut self, inst: NodeId, path: &str, ca: NodeId) -> Result<(), String> {
        let node = self.node(ca);
        if let NodeKind::ContAssign { net_decl: true, .. } = self.kind(ca) {
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
                         {net_type:?} (trireg and biased/resolved net classes outside the \
                         standalone subset are not supported)"
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
        let has_structural_driver = self.has_structural_driver(ca);
        if has_structural_driver && !self.net_lvalue_selects_are_constant(lhs) {
            return Err(format!(
                "continuous assignment to a resolved net in `{path}` requires constant select \
                 indices and bounds; dynamic or unpacked-array-dependent selectors are not \
                 supported"
            ));
        }
        let mut lh = self.lower_lhs(path, lhs)?;
        if has_structural_driver {
            if let Some(group) = self.unmapped_structural_group(&lh, ca) {
                return Err(format!(
                    "continuous assignment `{}` has no structural driver mapping for resolved net group {} at {}:{}:{}",
                    self.display_name(ca),
                    group,
                    self.node(ca).file.as_deref().unwrap_or("<unknown>"),
                    self.node(ca).line,
                    self.node(ca).col,
                ));
            }
            lh = self.remap_structural_lhs(lh, ca);
        }
        let rhs_ir = self.lower_expr(path, rhs)?;
        let rhs_ir = apply_lhs_assignment_context(&self.model, &lh, rhs_ir);
        // Driver evaluation must keep watching its inputs while a captured
        // propagation event is pending (IEEE 1364-2001 6.1.3).
        let scaled_delay = match self.kind(ca) {
            NodeKind::ContAssign { net_decl: true, .. } => None,
            NodeKind::ContAssign {
                delay: Some(de), ..
            } => Some(self.driver_delay_ticks(ca, *de)?),
            _ => None,
        };
        let assign = if let Some(delay) = scaled_delay {
            if let IrLhs::Whole(index) = &lh {
                self.initialize_delayed_driver(*index)?;
            }
            IrStmt::InertialAssign {
                lhs: lh,
                rhs: rhs_ir,
                delay,
            }
        } else {
            IrStmt::Assign {
                lhs: lh,
                rhs: rhs_ir,
                nba: false,
            }
        };
        let body = vec![assign];
        let fn_name = self.new_fn_name(path, "ca");
        let sigs = self.collect_read_signals(path, rhs)?;
        let shape = if sigs.is_empty() {
            // Constant driver: evaluate once at t=0, then end (the value can
            // never change, so there is nothing to wait on).
            IrShape::RunOnce
        } else {
            IrShape::SensLoop { reads: sigs }
        };
        let origin = self.origin(ca);
        self.model.processes.push(IrProcess::new_with_origin(
            fn_name,
            format!("{path}.assign"),
            shape,
            Vec::new(),
            body,
            origin,
        ));
        Ok(())
    }

    /// Net lvalues admit only constant selects. This runs after `self.inst`
    /// is set to the owning instance so elaborated parameters and genvars are
    /// accepted while runtime signal or array-dependent selectors fail.
    fn net_lvalue_selects_are_constant(&self, lhs: NodeId) -> bool {
        match self.kind(lhs) {
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                self.net_lvalue_selects_are_constant(*base) && self.eval_bound_i128(*index).is_ok()
            }
            NodeKind::Expr(ExprKind::PartSelect { base, left, right }) => {
                self.net_lvalue_selects_are_constant(*base)
                    && self.eval_bound_i128(*left).is_ok()
                    && self.eval_bound_i128(*right).is_ok()
            }
            NodeKind::Expr(ExprKind::IndexedPartSelect {
                base,
                base_expr,
                width_expr,
                ..
            }) => {
                self.net_lvalue_selects_are_constant(*base)
                    && self.eval_bound_i128(*base_expr).is_ok()
                    && self.eval_bound_i128(*width_expr).is_ok()
            }
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                self.net_lvalue_selects_are_constant(*base)
                    && indices
                        .iter()
                        .all(|index| self.eval_bound_i128(*index).is_ok())
            }
            _ => true,
        }
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
    pub(super) fn new_pca_enable(&mut self, path: &str) -> usize {
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
                two_state: false,
            },
            net_driver: None,
            alias: None,
            omit: false,
        });
        ir
    }

    pub(super) fn new_fn_name(&mut self, path: &str, kind: &str) -> String {
        let n = self.proc_seq;
        self.proc_seq += 1;
        format!("p_{}_{}_{}", ident(path), kind, n)
    }

    pub(super) fn new_frame_id(&mut self) -> Result<FrameId, String> {
        let id = FrameId::new(self.frame_seq);
        self.frame_seq = self
            .frame_seq
            .checked_add(1)
            .ok_or_else(|| "activation frame id space exhausted".to_string())?;
        Ok(id)
    }

    pub(super) fn capture_binding(&self, node: NodeId) -> Option<&CaptureBinding> {
        self.capture_locals.get(&node)
    }

    pub(super) fn capture_source(&self, target: NodeId) -> Option<CaptureSource> {
        if let Some(binding) = self.capture_binding(target) {
            let info = binding.local.clone();
            let initial = IrExpr::new(
                IrExprKind::LocalRead(Self::capture_local_name(binding.storage)),
                info.width,
                info.signed,
                None,
            );
            return Some(CaptureSource {
                info,
                initial,
                lifetime: binding.storage.lifetime(),
            });
        }
        if let Some(info) = self.proc_local_info(target) {
            if info.static_signal.is_none() {
                return Some(CaptureSource {
                    info: info.clone(),
                    initial: IrExpr::new(
                        IrExprKind::LocalRead(info.c_name.clone()),
                        info.width,
                        info.signed,
                        None,
                    ),
                    lifetime: StorageLifetime::Automatic,
                });
            }
            return None;
        }
        let function = self.func.as_ref()?;
        if let Some((c_name, width, signed, two_state, _shortreal)) = function.locals.get(&target) {
            return Some(CaptureSource {
                info: ProcLocalInfo {
                    c_name: c_name.clone(),
                    width: *width,
                    signed: *signed,
                    two_state: *two_state,
                    static_signal: None,
                },
                initial: IrExpr::new(IrExprKind::LocalRead(c_name.clone()), *width, *signed, None),
                lifetime: if self.function_is_automatic(function) {
                    StorageLifetime::Automatic
                } else {
                    StorageLifetime::Static
                },
            });
        }
        if let Some(storage) = function.persistent.get(&target) {
            return Some(CaptureSource {
                info: ProcLocalInfo {
                    c_name: storage.global.clone(),
                    width: storage.width,
                    signed: storage.signed,
                    two_state: storage.two_state,
                    static_signal: Some(storage.clone()),
                },
                initial: sig_read_expr_full(storage),
                lifetime: StorageLifetime::Static,
            });
        }
        if let Some(arg) = function.arg_read.get(&target) {
            let initial = function.arg_ir.get(&target)?.clone();
            return Some(CaptureSource {
                info: ProcLocalInfo {
                    c_name: function
                        .arg_write
                        .get(&target)
                        .and_then(|address| address.strip_prefix('&'))
                        .unwrap_or_default()
                        .to_owned(),
                    width: arg.width,
                    signed: arg.signed,
                    two_state: arg.two_state,
                    static_signal: function.persistent.get(&target).cloned(),
                },
                initial,
                lifetime: if function.persistent.contains_key(&target) {
                    StorageLifetime::Static
                } else {
                    StorageLifetime::Automatic
                },
            });
        }
        if function.ret_node == Some(target) {
            let ret = function.ret.as_ref()?;
            return Some(CaptureSource {
                info: ProcLocalInfo {
                    c_name: ret.c_name.clone(),
                    width: ret.width,
                    signed: ret.signed,
                    two_state: ret.two_state,
                    static_signal: None,
                },
                initial: IrExpr::new(
                    IrExprKind::LocalRead(ret.c_name.clone()),
                    ret.width,
                    ret.signed,
                    None,
                ),
                lifetime: if self.function_is_automatic(function) {
                    StorageLifetime::Automatic
                } else {
                    StorageLifetime::Static
                },
            });
        }
        None
    }

    fn function_is_automatic(&self, function: &FuncCtx) -> bool {
        function.def_node.is_some_and(|definition| {
            matches!(
                self.kind(definition),
                NodeKind::FuncTask {
                    automatic: true,
                    ..
                }
            )
        }) || function.def_node.is_none()
    }

    pub(super) fn capture_target(&self, node: NodeId) -> Option<NodeId> {
        if self.capture_locals.contains_key(&node) {
            return Some(node);
        }
        if let NodeKind::Expr(ExprKind::Ref {
            target: Some(target),
        }) = self.kind(node)
        {
            if self.capture_locals.contains_key(target) {
                return Some(*target);
            }
        }
        self.lexical_proc_local(node)
            .map(|(target, _)| target)
            .filter(|target| self.capture_locals.contains_key(target))
    }

    pub(super) fn nested_capture_ref(&self, node: NodeId) -> Option<NodeId> {
        if let Some(target) = self.capture_target(node) {
            return Some(target);
        }
        self.node(node)
            .children
            .iter()
            .find_map(|child| self.nested_capture_ref(*child))
    }

    pub(super) fn capture_local_name(storage: StorageRef) -> String {
        format!("_fc{}_{}", storage.frame().index(), storage.slot())
    }

    fn node_is_within(&self, node: NodeId, scope: NodeId) -> bool {
        let mut current = Some(node);
        while let Some(candidate) = current {
            if candidate == scope {
                return true;
            }
            current = self.node(candidate).parent;
        }
        false
    }

    /// Find automatic declarations referenced by a fork branch. Declarations
    /// inside the branch are owned by that branch and are not captures.
    pub(super) fn fork_capture_targets(&self, branch: NodeId) -> Vec<NodeId> {
        fn visit(cg: &Codegen<'_>, node: NodeId, branch: NodeId, out: &mut HashSet<NodeId>) {
            let target = match cg.kind(node) {
                NodeKind::Expr(ExprKind::Ref {
                    target: Some(target),
                }) => Some(*target),
                _ => cg
                    .lexical_proc_local(node)
                    .map(|(target, _)| target)
                    .or_else(|| cg.lexical_proc_string_local(node).map(|(target, _)| target)),
            };
            if let Some(target) = target {
                if !cg.node_is_within(target, branch)
                    && (cg.capture_source(target).is_some()
                        || cg.proc_string_local_name(target).is_some())
                {
                    out.insert(target);
                }
            }
            for child in &cg.node(node).children {
                visit(cg, *child, branch, out);
            }
        }

        let mut targets = HashSet::new();
        visit(self, branch, branch, &mut targets);
        let mut targets = targets.into_iter().collect::<Vec<_>>();
        targets.sort_by_key(|node| node.index());
        targets
    }

    // ── Structural gate primitives ─────────────────────────────────────────

    /// Lower one structural primitive ([`NodeKind::Gate`]) into comb processes
    /// shaped like continuous assignments. Every process evaluates at spawn,
    /// then re-evaluates whenever an input-terminal dependency changes. A
    /// multi-output `buf`/`not` gets one process per output so each output can
    /// retain an independent canonical driver contribution.
    ///
    /// Multi-input logic gates reduce their inputs left-to-right with the
    /// two-input runtime op; nand/nor/xnor negate after the full reduce.
    /// A gate delay `#D` schedules a captured inertial update while the
    /// process continues watching its inputs. Unsupported primitives
    /// (switches and UDPs) are rejected with explicit errors here at lowering
    /// time; terminal legality is checked by the typed LHS/expression
    /// lowerers.
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
                     supported"
                ))
            }
            PrimClass::Udp => {
                return Err(format!(
                    "user-defined primitive instance `{shown}` in `{path}` is not \
                     supported"
                ))
            }
            PrimClass::Array => {}
        }
        let driver_strengths =
            gate_driver_strengths(prim_type, strength0, strength1, &format!("{path}.{shown}"))?;
        // Which builtin gate this is; everything outside the supported set
        // (switch/transistor prim types, sequential/combinational UDP types)
        // is rejected.  UDP instances never reach this point (their class was
        // rejected above); the prim-type reject covers unknown/other kinds.
        let op = match prim_type {
            PrimitiveType::And => GateOp::Reduce(IrBinOp::BitAnd, false),
            PrimitiveType::Nand => GateOp::Reduce(IrBinOp::BitAnd, true),
            PrimitiveType::Or => GateOp::Reduce(IrBinOp::BitOr, false),
            PrimitiveType::Nor => GateOp::Reduce(IrBinOp::BitOr, true),
            PrimitiveType::Xor => GateOp::Reduce(IrBinOp::BitXor, false),
            PrimitiveType::Xnor => GateOp::Reduce(IrBinOp::BitXor, true),
            PrimitiveType::Buf => GateOp::Copy,
            PrimitiveType::Not => GateOp::Not,
            PrimitiveType::Bufif1 => GateOp::Enable {
                invert_out: false,
                active_high: true,
            },
            PrimitiveType::Bufif0 => GateOp::Enable {
                invert_out: false,
                active_high: false,
            },
            PrimitiveType::Notif1 => GateOp::Enable {
                invert_out: true,
                active_high: true,
            },
            PrimitiveType::Notif0 => GateOp::Enable {
                invert_out: true,
                active_high: false,
            },
            PrimitiveType::Pullup => GateOp::Pull(true),
            PrimitiveType::Pulldown => GateOp::Pull(false),
            _ => {
                return Err(format!(
                    "primitive type {prim_type:?} of `{shown}` in `{path}` is not \
                     supported"
                ))
            }
        };
        // Terminal-count/direction validation per kind. Directions are
        // captured by the frontend from the primitive definition: `buf` and
        // `not` mark every terminal except the final input as an output.
        let out_positions: Vec<usize> = terms
            .iter()
            .enumerate()
            .filter(|(_, t)| t.direction == DbDirection::Output)
            .map(|(i, _)| i)
            .collect();
        let in_positions: Vec<usize> = terms
            .iter()
            .enumerate()
            .filter(|(_, t)| t.direction == DbDirection::Input)
            .map(|(i, _)| i)
            .collect();
        let shape_ok = match op {
            GateOp::Pull(_) => {
                terms.len() == 1 && out_positions.len() == 1 && in_positions.is_empty()
            }
            GateOp::Copy | GateOp::Not => {
                !out_positions.is_empty()
                    && in_positions.len() == 1
                    && out_positions.len() + in_positions.len() == terms.len()
            }
            GateOp::Enable { .. } => {
                terms.len() == 3 && out_positions.len() == 1 && in_positions.len() == 2
            }
            GateOp::Reduce(..) => {
                terms.len() >= 2
                    && out_positions.len() == 1
                    && !in_positions.is_empty()
                    && out_positions.len() + in_positions.len() == terms.len()
            }
        };
        if !shape_ok {
            let what = match op {
                GateOp::Pull(_) => "pullup/pulldown instance takes exactly one output terminal",
                GateOp::Copy | GateOp::Not => {
                    "`buf`/`not` gates take one or more output terminals followed by \
                     exactly one input terminal"
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

        // Callee resolution in the LHS/inputs needs the owning instance.
        self.inst = inst;

        // Lower every terminal through the typed paths used by ordinary
        // assignments and expressions. This admits selects, constants and
        // hierarchical references while keeping real values and invalid
        // output expressions as precise gate diagnostics.
        let mut terminal_lhs: Vec<Option<IrLhs>> = vec![None; terms.len()];
        let mut terminal_exprs: Vec<Option<IrExpr>> = vec![None; terms.len()];
        let mut widths = vec![0u32; terms.len()];
        let mut sens = Vec::new();
        for i in &in_positions {
            let expr = self.lower_expr(path, terms[*i].expr).map_err(|error| {
                format!(
                    "input terminal {} of gate `{shown}` in `{path}` is not a legal \
                     expression: {error}",
                    i
                )
            })?;
            if expr.is_real() {
                return Err(format!(
                    "real-valued input terminal {} of gate `{shown}` in `{path}` \
                     is not supported",
                    i
                ));
            }
            if expr.width() == 0 {
                return Err(format!(
                    "input terminal {} of gate `{shown}` in `{path}` has zero width",
                    i
                ));
            }
            widths[*i] = expr.width();
            terminal_exprs[*i] = Some(expr);
            for dependency in self.collect_read_signals(path, terms[*i].expr)? {
                if !sens.contains(&dependency) {
                    sens.push(dependency);
                }
            }
        }
        for i in &out_positions {
            let lhs = self.lower_lhs(path, terms[*i].expr).map_err(|error| {
                format!(
                    "output terminal {} of gate `{shown}` in `{path}` is not a legal \
                     lvalue: {error}",
                    i
                )
            })?;
            let width = packed_lhs_width(&self.model, &lhs).ok_or_else(|| {
                format!(
                    "output terminal {} of gate `{shown}` in `{path}` must be a \
                     packed net or variable",
                    i
                )
            })?;
            if width == 0 {
                return Err(format!(
                    "output terminal {} of gate `{shown}` in `{path}` has zero width",
                    i
                ));
            }
            widths[*i] = width;
            terminal_lhs[*i] = Some(lhs);
        }

        let scaled_delay = match delay {
            Some(de) => Some(self.driver_delay_ticks(g, de)?),
            None => None,
        };

        for (output_ordinal, out_pos) in out_positions.iter().enumerate() {
            let raw_lhs = terminal_lhs[*out_pos]
                .clone()
                .expect("gate output LHS lowered above");
            let output_width = widths[*out_pos];

            let group = self.structural_group_for_lhs(&raw_lhs);
            let mut lhs = raw_lhs.clone();
            if let Some(group) = group {
                // Primitive output terminals are numbered within each
                // resolved group. A gate may write one group before another
                // and then return to the first; using the global output
                // ordinal would give that later terminal a stale slot key.
                let terminal = out_positions[..output_ordinal]
                    .iter()
                    .filter(|previous| {
                        terminal_lhs[**previous]
                            .as_ref()
                            .and_then(|lhs| self.structural_group_for_lhs(lhs))
                            == Some(group)
                    })
                    .count();
                if terminal == 0 {
                    if self.structural_driver_signal(g, group).is_none() {
                        return Err(format!(
                            "gate `{shown}` has no structural driver mapping for resolved net \
                             group {} at {}:{}:{}",
                            group,
                            self.node(g).file.as_deref().unwrap_or("<unknown>"),
                            self.node(g).line,
                            self.node(g).col,
                        ));
                    }
                } else {
                    self.add_structural_driver_for_terminal(group, g, driver_strengths, terminal)?;
                }
                lhs = self.remap_structural_lhs_for_terminal(lhs, g, terminal);
                if let Some(unmapped) =
                    self.unmapped_structural_group_for_terminal(&raw_lhs, g, terminal)
                {
                    return Err(format!(
                        "gate `{shown}` has no structural driver mapping for resolved net group {unmapped} at {}:{}:{}",
                        self.node(g).file.as_deref().unwrap_or("<unknown>"),
                        self.node(g).line,
                        self.node(g).col,
                    ));
                }
            }

            let mut input_values = Vec::with_capacity(in_positions.len());
            for i in &in_positions {
                let expr = terminal_exprs[*i]
                    .clone()
                    .expect("gate input expression lowered above");
                input_values.push(IrExpr::resize_to(expr, output_width, false));
            }
            let value = match op {
                GateOp::Pull(ones) => const_bits_expr(output_width, ones),
                GateOp::Copy => input_values
                    .into_iter()
                    .next()
                    .expect("buf/not shape checked"),
                GateOp::Not => bitneg_full_width(
                    input_values
                        .into_iter()
                        .next()
                        .expect("buf/not shape checked"),
                ),
                GateOp::Reduce(bop, neg) => {
                    let mut values = input_values.into_iter();
                    let mut acc = values.next().expect("logic gate shape checked");
                    for next in values {
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
                    let mut values = input_values.into_iter();
                    let data = values.next().expect("enable gate shape checked");
                    let en = IrExpr::resize_to(
                        values.next().expect("enable gate shape checked"),
                        1,
                        false,
                    );
                    // LRM 1364-1995 §7.4 Table 7-5: an ENABLED enable gate acts
                    // like `buf`/`not` (Tables 7-3/7-4), so a data Z must reach
                    // the output as X while known bits pass unchanged.  The mux
                    // is an identity/copy context that returns its chosen arm
                    // verbatim (`sv4_mux` with a known select), so the Z→X
                    // normalization is done explicitly with `data|data`.
                    // Correctness proof from llg_value.c `sv4_bitwise` (op = OR),
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
                    let z = const_z_expr(output_width);
                    let (a, b) = if active_high { (data, z) } else { (z, data) };
                    IrExpr::new(
                        IrExprKind::Mux {
                            sel: Box::new(en),
                            a: Box::new(a),
                            b: Box::new(b),
                        },
                        output_width,
                        false,
                        None,
                    )
                }
            };
            let mut body = Vec::new();
            if let Some(delay) = scaled_delay {
                if let IrLhs::Whole(index) = &lhs {
                    self.initialize_delayed_driver(*index)?;
                }
                body.push(IrStmt::InertialAssign {
                    lhs,
                    rhs: value,
                    delay,
                });
            } else {
                body.push(IrStmt::Assign {
                    lhs,
                    rhs: value,
                    nba: false,
                });
            }
            let shape = if sens.is_empty() {
                IrShape::RunOnce
            } else {
                IrShape::SensLoop {
                    reads: sens.clone(),
                }
            };
            let fn_name = self.new_fn_name(path, "gate");
            let origin = self.origin(g);
            self.model.processes.push(IrProcess::new_with_origin(
                fn_name,
                format!("{path}.{shown}[output{output_ordinal}]"),
                shape,
                Vec::new(),
                body,
                origin,
            ));
        }
        Ok(())
    }

    // ── Port links ─────────────────────────────────────────────────────────

    pub(super) fn bind_reference_ports(&mut self) -> Result<(), String> {
        for port in self.design_nodes() {
            let NodeKind::Port {
                direction: DbDirection::Ref,
                high_expr,
                high,
                low,
                ..
            } = self.kind(port)
            else {
                continue;
            };
            let actual = high_expr.or(*high).ok_or_else(|| {
                format!("reference port `{}` has no actual", self.display_name(port))
            })?;
            let internal = low.ok_or_else(|| {
                format!(
                    "reference port `{}` has no storage",
                    self.display_name(port)
                )
            })?;
            let path = self
                .owning_inst(port)
                .map(|instance| self.instance_path_of(instance))
                .unwrap_or_default();
            if let Some(child) = self.signal_of(internal).cloned() {
                let target = self.lower_lhs(&path, actual).map_err(|error| {
                    format!(
                        "reference port `{}` has an invalid actual: {error}",
                        self.display_name(port)
                    )
                })?;
                let target = self.reference_lhs(target)?;
                let Some(target_ty) = self.reference_lhs_type(&target) else {
                    return Err(format!(
                        "reference port `{}` requires a typed variable actual",
                        self.display_name(port)
                    ));
                };
                if !self.reference_actual_is_variable(actual)
                    || !self.reference_lhs_is_variable(&target)
                    || self.model.signals[child.ir].ty != target_ty
                {
                    return Err(format!(
                        "reference port `{}` requires matching variable storage",
                        self.display_name(port)
                    ));
                }
                self.reference_signals.insert(child.ir, target);
                continue;
            }

            if let Some(child) = self.array_of(internal).cloned() {
                let Some(actual_array) = self.array_of(actual).cloned() else {
                    return Err(format!(
                        "reference port `{}` requires a matching fixed-array variable actual",
                        self.display_name(port)
                    ));
                };
                if !self.reference_actual_is_variable(actual)
                    || child.is_net
                    || actual_array.is_net
                    || child.dims != actual_array.dims
                    || child.elem_width != actual_array.elem_width
                    || child.signed != actual_array.signed
                    || self.model.arrays[child.ir].two_state
                        != self.model.arrays[actual_array.ir].two_state
                    || child.real != actual_array.real
                    || child.shortreal != actual_array.shortreal
                {
                    return Err(format!(
                        "reference port `{}` requires matching fixed-array variable storage (variable {}/{}, formal {:?}/{}/{}/{}, actual {:?}/{}/{}/{})",
                        self.display_name(port),
                        self.reference_actual_is_variable(actual),
                        child.is_net || actual_array.is_net,
                        child.dims,
                        child.elem_width,
                        child.signed,
                        self.model.arrays[child.ir].two_state,
                        actual_array.dims,
                        actual_array.elem_width,
                        actual_array.signed,
                        self.model.arrays[actual_array.ir].two_state,
                    ));
                }
                self.reference_arrays
                    .insert(child.ir, self.reference_array(actual_array.ir));
                continue;
            }

            if let Some(child) = self.unpacked_aggregates.get(&internal).cloned() {
                self.bind_reference_aggregate(port, &path, actual, child)?;
                continue;
            }

            if let Some(child_object) = self.object_of(&path, internal) {
                let Some(actual_object) = self.object_of(&path, actual) else {
                    return Err(format!(
                        "reference port `{}` requires a matching object variable actual",
                        self.display_name(port)
                    ));
                };
                if !self.reference_actual_is_variable(actual)
                    || self.model.objects[child_object].ty != self.model.objects[actual_object].ty
                {
                    return Err(format!(
                        "reference port `{}` requires matching object storage",
                        self.display_name(port)
                    ));
                }
                self.reference_objects
                    .insert(child_object, self.reference_object(actual_object));
                continue;
            }

            if self.container_of(internal).is_some() {
                return Err(format!(
                    "reference port `{}` cannot bind resizable container storage",
                    self.display_name(port)
                ));
            }
            return Err(format!(
                "reference port `{}` requires a supported variable, array, aggregate, or object actual",
                self.display_name(port)
            ));
        }

        let signal_bindings = self.reference_signals.keys().copied().collect::<Vec<_>>();
        for child in signal_bindings {
            let target = self.reference_lhs(IrLhs::Whole(child))?;
            self.reference_signals.insert(child, target.clone());
            if let IrLhs::Whole(target) = target {
                let canonical = self.model.signals[target].clone();
                self.model.signals[child].alias = Some(target);
                self.model.signals[child].c_name = canonical.c_name;
            }
        }
        let array_bindings = self.reference_arrays.keys().copied().collect::<Vec<_>>();
        for child in array_bindings {
            let target = self.reference_array(child);
            self.reference_arrays.insert(child, target);
        }
        let object_bindings = self.reference_objects.keys().copied().collect::<Vec<_>>();
        for child in object_bindings {
            let target = self.reference_object(child);
            self.reference_objects.insert(child, target);
        }

        let signals = &self.model.signals;
        let canonicalize = |info: &mut SignalInfo| {
            if let Some(target) = signals[info.ir].alias {
                info.ir = target;
                info.global = signals[target].c_name.clone();
            }
        };
        for info in self.sig_globals.values_mut() {
            canonicalize(info);
        }
        for info in &mut self.signals {
            canonicalize(info);
        }
        for scope in self.scope_sig_names.values_mut() {
            for info in scope.values_mut() {
                canonicalize(info);
            }
        }
        Ok(())
    }

    fn bind_reference_aggregate(
        &mut self,
        port: NodeId,
        _path: &str,
        actual: NodeId,
        child: UnpackedAggregateInfo,
    ) -> Result<(), String> {
        let (actual_target, prefix) = self
            .unpacked_aggregate_target(actual)
            .map(|target| (target, Vec::new()))
            .or_else(|| self.unpacked_path_for_expr(actual))
            .ok_or_else(|| {
                format!(
                    "reference port `{}` requires a matching aggregate variable actual",
                    self.display_name(port)
                )
            })?;
        let parent = self
            .unpacked_aggregates
            .get(&actual_target)
            .cloned()
            .ok_or_else(|| {
                format!(
                    "reference port `{}` actual aggregate has no captured storage",
                    self.display_name(port)
                )
            })?;
        if !self.reference_actual_is_variable(actual) {
            return Err(format!(
                "reference port `{}` requires a variable aggregate actual",
                self.display_name(port)
            ));
        }
        if prefix.is_empty()
            && child
                .type_identity
                .as_deref()
                .zip(parent.type_identity.as_deref())
                .is_some_and(|(left, right)| left != right)
        {
            return Err(format!(
                "reference port `{}` requires matching aggregate type",
                self.display_name(port)
            ));
        }
        for child_leaf in &child.leaves {
            let mut parent_path = prefix.clone();
            parent_path.extend(child_leaf.path.iter().cloned());
            let parent_leaf = parent
                .leaves
                .iter()
                .find(|leaf| leaf.path == parent_path)
                .ok_or_else(|| {
                    format!(
                        "reference port `{}` aggregate member `{}` has no matching actual storage",
                        self.display_name(port),
                        aggregate_path_suffix(&child_leaf.path)
                    )
                })?;
            match (
                &child_leaf.signal,
                &parent_leaf.signal,
                child_leaf.object,
                parent_leaf.object,
            ) {
                (Some(child_signal), Some(parent_signal), _, _) => {
                    let target = self.reference_lhs(IrLhs::Whole(parent_signal.ir))?;
                    let Some(target_ty) = self.reference_lhs_type(&target) else {
                        return Err(format!(
                            "reference port `{}` aggregate member `{}` has invalid storage",
                            self.display_name(port),
                            aggregate_path_suffix(&child_leaf.path)
                        ));
                    };
                    if !self.reference_lhs_is_variable(&target)
                        || self.model.signals[child_signal.ir].ty != target_ty
                    {
                        return Err(format!(
                            "reference port `{}` aggregate member `{}` has incompatible storage",
                            self.display_name(port),
                            aggregate_path_suffix(&child_leaf.path)
                        ));
                    }
                    self.reference_signals.insert(child_signal.ir, target);
                }
                (None, None, Some(child_object), Some(parent_object)) => {
                    if self.model.objects[child_object].ty != self.model.objects[parent_object].ty {
                        return Err(format!(
                            "reference port `{}` aggregate member `{}` has incompatible object storage",
                            self.display_name(port),
                            aggregate_path_suffix(&child_leaf.path)
                        ));
                    }
                    self.reference_objects
                        .insert(child_object, self.reference_object(parent_object));
                }
                _ => {
                    return Err(format!(
                        "reference port `{}` aggregate member `{}` has incompatible shape",
                        self.display_name(port),
                        aggregate_path_suffix(&child_leaf.path)
                    ));
                }
            }
        }
        Ok(())
    }

    fn remap_structural_lhs(&self, lhs: IrLhs, source: NodeId) -> IrLhs {
        let remap = |index: usize| {
            self.model
                .signals
                .get(index)
                .and_then(|signal| signal.net_driver.map(|(group, _)| group))
                .and_then(|group| self.structural_driver_signal(source, group))
                .unwrap_or(index)
        };
        match lhs {
            IrLhs::Whole(index) => IrLhs::Whole(remap(index)),
            IrLhs::Bit(index, expression, two_state) => {
                IrLhs::Bit(remap(index), expression, two_state)
            }
            IrLhs::Part(index, left, right, two_state) => {
                IrLhs::Part(remap(index), left, right, two_state)
            }
            IrLhs::IdxPart(index, base, width, selected_width, negative, two_state) => {
                IrLhs::IdxPart(
                    remap(index),
                    base,
                    width,
                    selected_width,
                    negative,
                    two_state,
                )
            }
            IrLhs::Stream {
                parts,
                width,
                slice,
                direction,
            } => IrLhs::Stream {
                parts: parts
                    .into_iter()
                    .map(|(part, part_width)| (self.remap_structural_lhs(part, source), part_width))
                    .collect(),
                width,
                slice,
                direction,
            },
            other => other,
        }
    }

    fn structural_group_for_lhs(&self, lhs: &IrLhs) -> Option<usize> {
        let signal_group = |index: usize| {
            self.model
                .signals
                .get(index)
                .and_then(|signal| signal.net_driver.map(|(group, _)| group))
        };
        match lhs {
            IrLhs::Whole(index)
            | IrLhs::Bit(index, ..)
            | IrLhs::Part(index, ..)
            | IrLhs::IdxPart(index, ..) => signal_group(*index),
            IrLhs::Stream { parts, .. } => parts
                .iter()
                .find_map(|(part, _)| self.structural_group_for_lhs(part)),
            IrLhs::WholeRef { .. } | IrLhs::Ref { .. } | IrLhs::ArrayElem { .. } => None,
        }
    }

    fn remap_structural_lhs_for_terminal(
        &self,
        lhs: IrLhs,
        source: NodeId,
        terminal: usize,
    ) -> IrLhs {
        let remap = |index: usize| {
            self.model
                .signals
                .get(index)
                .and_then(|signal| signal.net_driver.map(|(group, _)| group))
                .and_then(|group| {
                    self.structural_driver_signal_for_terminal(source, group, terminal)
                })
                .unwrap_or(index)
        };
        match lhs {
            IrLhs::Whole(index) => IrLhs::Whole(remap(index)),
            IrLhs::Bit(index, expression, two_state) => {
                IrLhs::Bit(remap(index), expression, two_state)
            }
            IrLhs::Part(index, left, right, two_state) => {
                IrLhs::Part(remap(index), left, right, two_state)
            }
            IrLhs::IdxPart(index, base, width, selected_width, negative, two_state) => {
                IrLhs::IdxPart(
                    remap(index),
                    base,
                    width,
                    selected_width,
                    negative,
                    two_state,
                )
            }
            IrLhs::Stream {
                parts,
                width,
                slice,
                direction,
            } => IrLhs::Stream {
                parts: parts
                    .into_iter()
                    .map(|(part, part_width)| {
                        (
                            self.remap_structural_lhs_for_terminal(part, source, terminal),
                            part_width,
                        )
                    })
                    .collect(),
                width,
                slice,
                direction,
            },
            other => other,
        }
    }

    fn unmapped_structural_group(&self, lhs: &IrLhs, source: NodeId) -> Option<usize> {
        let mut mapped = |group| self.structural_driver_signal(source, group);
        self.unmapped_structural_group_for(lhs, &mut mapped)
    }

    fn unmapped_structural_group_for_terminal(
        &self,
        lhs: &IrLhs,
        source: NodeId,
        terminal: usize,
    ) -> Option<usize> {
        let mut mapped =
            |group| self.structural_driver_signal_for_terminal(source, group, terminal);
        self.unmapped_structural_group_for(lhs, &mut mapped)
    }

    fn unmapped_structural_group_for(
        &self,
        lhs: &IrLhs,
        mapped: &mut dyn FnMut(usize) -> Option<usize>,
    ) -> Option<usize> {
        match lhs {
            IrLhs::Whole(index)
            | IrLhs::Bit(index, ..)
            | IrLhs::Part(index, ..)
            | IrLhs::IdxPart(index, ..) => self
                .model
                .signals
                .get(*index)
                .and_then(|signal| signal.net_driver.map(|(group, _)| group))
                .filter(|group| mapped(*group).is_none()),
            IrLhs::Stream { parts, .. } => parts
                .iter()
                .find_map(|(part, _)| self.unmapped_structural_group_for(part, mapped)),
            IrLhs::WholeRef { .. } | IrLhs::Ref { .. } | IrLhs::ArrayElem { .. } => None,
        }
    }

    fn emit_link_process(
        &mut self,
        parent_path: &str,
        child_path: &str,
        port: NodeId,
        reads: Vec<IrDependency>,
        body: IrStmt,
    ) {
        let shape = if reads.is_empty() {
            IrShape::RunOnce
        } else {
            IrShape::SensLoop { reads }
        };
        let fn_name = self.new_fn_name(parent_path, "link");
        let origin = self.origin(port);
        self.model.processes.push(IrProcess::new_with_origin(
            fn_name,
            format!("{child_path}.link"),
            shape,
            Vec::new(),
            vec![body],
            origin,
        ));
    }

    fn emit_array_port_link(
        &mut self,
        parent_path: &str,
        child_path: &str,
        port: NodeId,
        direction: DbDirection,
        actual: NodeId,
        internal: NodeId,
    ) -> Result<bool, String> {
        let child_array = self.array_of(internal).cloned();
        let actual_array = self.array_of(actual).cloned();
        let (child_array, actual_array) = match (child_array, actual_array) {
            (None, None) => return Ok(false),
            (Some(_), None) | (None, Some(_)) => {
                return Err(format!(
                    "port `{}` connects a fixed array to a non-array actual in `{child_path}`",
                    self.display_name(port)
                ));
            }
            (Some(child), Some(actual)) => (child, actual),
        };
        if child_array.dims.len() != actual_array.dims.len()
            || child_array.dims.iter().zip(&actual_array.dims).any(
                |((child_left, child_right), (actual_left, actual_right))| {
                    (i64::from(*child_left) - i64::from(*child_right)).unsigned_abs()
                        != (i64::from(*actual_left) - i64::from(*actual_right)).unsigned_abs()
                },
            )
        {
            return Err(format!(
                "fixed array port `{}` has incompatible rank or dimensions in `{child_path}`",
                self.display_name(port)
            ));
        }

        let (target, source) = if direction == DbDirection::Input {
            (&child_array, &actual_array)
        } else {
            (&actual_array, &child_array)
        };
        let target_array = self.reference_array(target.ir);
        let source_array = self.reference_array(source.ir);
        let target_indices = port_array_index_vectors(&target.dims);
        let source_indices = port_array_index_vectors(&source.dims);
        if target_indices.len() != source_indices.len() {
            return Err(format!(
                "fixed array port `{}` has incompatible element count in `{child_path}`",
                self.display_name(port)
            ));
        }
        let mut assignments = Vec::with_capacity(target_indices.len());
        for (target_indices, source_indices) in target_indices.iter().zip(source_indices) {
            let target_indices = target_indices
                .iter()
                .map(|index| lhs_integer_expr(i128::from(*index)))
                .collect();
            let source_indices = source_indices
                .iter()
                .map(|index| lhs_integer_expr(i128::from(*index)))
                .collect();
            let lhs = IrLhs::ArrayElem {
                arr: target_array,
                indices: target_indices,
                elem_sel: IrElemSel::Whole,
            };
            let rhs = IrExpr::new(
                IrExprKind::ArrayRead {
                    arr: source_array,
                    indices: source_indices,
                    elem_sel: IrElemSel::Whole,
                },
                if source.real { 0 } else { source.elem_width },
                source.signed,
                None,
            );
            assignments.push(IrStmt::Assign {
                lhs: lhs.clone(),
                rhs: apply_lhs_assignment_context(&self.model, &lhs, rhs),
                nba: false,
            });
        }
        self.emit_link_process(
            parent_path,
            child_path,
            port,
            vec![IrDependency::ArrayContents(source_array)],
            IrStmt::Block(assignments),
        );
        Ok(true)
    }

    fn aggregate_link_dependencies(&self, aggregate: &UnpackedAggregateInfo) -> Vec<IrDependency> {
        let mut reads = Vec::new();
        for leaf in &aggregate.leaves {
            if let Some(object) = leaf.object {
                let dependency = IrDependency::Object(self.reference_object(object));
                if !reads.contains(&dependency) {
                    reads.push(dependency);
                }
                continue;
            }
            let Some(signal) = &leaf.signal else {
                continue;
            };
            let dependency = self.signal_dependency(signal);
            if !reads.contains(&dependency) {
                reads.push(dependency);
            }
        }
        reads
    }

    fn emit_aggregate_port_link(
        &mut self,
        parent_path: &str,
        child_path: &str,
        port: NodeId,
        direction: DbDirection,
        actual: NodeId,
        internal: NodeId,
    ) -> Result<bool, String> {
        let child_aggregate = self.unpacked_aggregate_info(internal);
        let actual_aggregate = self.unpacked_aggregate_info(actual);
        let (child_aggregate, actual_aggregate) = match (child_aggregate, actual_aggregate) {
            (None, None) => return Ok(false),
            (Some(_), None) | (None, Some(_)) => {
                return Err(format!(
                    "aggregate port `{}` connects to a non-aggregate actual in `{child_path}`",
                    self.display_name(port)
                ));
            }
            (Some(child), Some(actual)) => (child, actual),
        };
        let source = if direction == DbDirection::Input {
            actual_aggregate.1.clone()
        } else {
            child_aggregate.1.clone()
        };
        let (target_node, source_node, path) = if direction == DbDirection::Input {
            (internal, actual, child_path)
        } else {
            (actual, internal, parent_path)
        };
        let reads = self.aggregate_link_dependencies(&source);
        let statement = self
            .lower_unpacked_aggregate_assignment(
                path,
                target_node,
                source_node,
                false,
                Operation::Assignment,
            )?
            .ok_or_else(|| {
                format!(
                    "aggregate port `{}` did not lower as a complete aggregate assignment",
                    self.display_name(port)
                )
            })?;
        self.emit_link_process(parent_path, child_path, port, reads, statement);
        Ok(true)
    }

    fn emit_container_port_link(
        &mut self,
        parent_path: &str,
        child_path: &str,
        port: NodeId,
        direction: DbDirection,
        actual: NodeId,
        internal: NodeId,
    ) -> Result<bool, String> {
        let child_container = self.container_of(internal);
        let actual_container = self.container_of(actual);
        let (child_container, actual_container) = match (child_container, actual_container) {
            (None, None) => return Ok(false),
            (Some(_), None) | (None, Some(_)) => {
                return Err(format!(
                    "resizable port `{}` connects to a non-container actual in `{child_path}`",
                    self.display_name(port)
                ));
            }
            (Some(child), Some(actual)) => (child, actual),
        };
        let (target, source) = if direction == DbDirection::Input {
            (child_container.ir, actual_container.ir)
        } else {
            (actual_container.ir, child_container.ir)
        };
        self.emit_link_process(
            parent_path,
            child_path,
            port,
            vec![
                IrDependency::ContainerContents(source),
                IrDependency::ContainerShape(source),
            ],
            IrStmt::Container(IrContainerStmt::Copy {
                dst: target,
                src: source,
            }),
        );
        Ok(true)
    }

    fn emit_object_port_link(
        &mut self,
        parent_path: &str,
        child_path: &str,
        port: NodeId,
        direction: DbDirection,
        actual: NodeId,
        internal: NodeId,
    ) -> Result<bool, String> {
        let child_object = self.object_of(child_path, internal);
        let actual_object = self.object_of(parent_path, actual);
        let (child_object, actual_object) = match (child_object, actual_object) {
            (None, None) => return Ok(false),
            (Some(_), None) | (None, Some(_)) => {
                return Err(format!(
                    "object port `{}` connects to a non-object actual in `{child_path}`",
                    self.display_name(port)
                ));
            }
            (Some(child), Some(actual)) => (child, actual),
        };
        let (target, source) = if direction == DbDirection::Input {
            (child_object, actual_object)
        } else {
            (actual_object, child_object)
        };
        if self.model.objects[target].ty != IrObjectType::String
            || self.model.objects[source].ty != IrObjectType::String
        {
            return Err(format!(
                "chandle port `{}` is not supported by value links in `{child_path}`",
                self.display_name(port)
            ));
        }
        self.emit_link_process(
            parent_path,
            child_path,
            port,
            vec![IrDependency::Object(source)],
            IrStmt::Object(IrObjectStmt::StringAssign(
                self.reference_object(target),
                IrStringExpr::Read(self.reference_object(source)),
            )),
        );
        Ok(true)
    }

    fn emit_links(&mut self, parent_path: &str, child_inst: NodeId) -> Result<(), String> {
        let child_path = self.instance_path_of(child_inst);
        self.inst = self.owning_inst(child_inst).unwrap_or(child_inst);
        for c in &self.node(child_inst).children {
            let port = *c;
            let (direction, high, low, high_expr, high_present, high_open) = match self.kind(port) {
                NodeKind::Port {
                    direction,
                    high,
                    low,
                    high_expr,
                    high_present,
                    high_open,
                    ..
                } => (
                    *direction,
                    *high,
                    *low,
                    *high_expr,
                    *high_present,
                    *high_open,
                ),
                _ => continue,
            };
            if let Some(actual) =
                self.node(port)
                    .children
                    .iter()
                    .find_map(|cc| match self.kind(*cc) {
                        NodeKind::IfaceConn { actual, .. } => Some(*actual),
                        _ => None,
                    })
            {
                if high != Some(actual) {
                    return Err(format!(
                        "interface port `{}` of `{child_path}` has inconsistent bound actual identities",
                        self.node(port).name
                    ));
                }
                continue;
            }
            if matches!(direction, DbDirection::Inout | DbDirection::Ref) {
                continue;
            }
            let Some(actual) = high_expr.or(high) else {
                if high_present && !high_open {
                    return Err(format!(
                        "port `{}` of `{child_path}` declares a connection but has no actual expression at {}:{}:{}",
                        self.node(port).name,
                        self.node(port).file.as_deref().unwrap_or("<unknown>"),
                        self.node(port).line,
                        self.node(port).col,
                    ));
                }
                continue;
            };
            let Some(internal) = low else {
                return Err(format!(
                    "port `{}` of `{child_path}` has an actual connection but no child-side storage at {}:{}:{}",
                    self.node(port).name,
                    self.node(port).file.as_deref().unwrap_or("<unknown>"),
                    self.node(port).line,
                    self.node(port).col,
                ));
            };
            if self.emit_array_port_link(
                parent_path,
                &child_path,
                port,
                direction,
                actual,
                internal,
            )? {
                continue;
            }
            if self.emit_aggregate_port_link(
                parent_path,
                &child_path,
                port,
                direction,
                actual,
                internal,
            )? {
                continue;
            }
            if self.emit_container_port_link(
                parent_path,
                &child_path,
                port,
                direction,
                actual,
                internal,
            )? {
                continue;
            }
            if self.emit_object_port_link(
                parent_path,
                &child_path,
                port,
                direction,
                actual,
                internal,
            )? {
                continue;
            }
            let (_, child_info) = self.resolve_signal_id(&child_path, internal)?;
            let (lhs, rhs, reads) = if direction == DbDirection::Input {
                let rhs = self.lower_expr(parent_path, actual)?;
                let reads = self.collect_read_signals(parent_path, actual)?;
                let lhs = self.remap_structural_lhs(IrLhs::Whole(child_info.ir), port);
                if let Some(group) = self.unmapped_structural_group(&lhs, port) {
                    return Err(format!(
                        "input port `{}` has no structural driver mapping for resolved net group {} at {}:{}:{}",
                        self.display_name(port),
                        group,
                        self.node(port).file.as_deref().unwrap_or("<unknown>"),
                        self.node(port).line,
                        self.node(port).col,
                    ));
                }
                (lhs, rhs, reads)
            } else {
                let lhs = self.lower_lhs(parent_path, actual)?;
                if let Some(group) = self.unmapped_structural_group(&lhs, port) {
                    return Err(format!(
                        "output port `{}` has no structural driver mapping for resolved net group {} at {}:{}:{}",
                        self.display_name(port),
                        group,
                        self.node(port).file.as_deref().unwrap_or("<unknown>"),
                        self.node(port).line,
                        self.node(port).col,
                    ));
                }
                let lhs = self.remap_structural_lhs(lhs, port);
                let child_dependency = self.signal_dependency(&child_info);
                let mut reads = vec![child_dependency.clone()];
                let mut seen = HashSet::from([child_dependency]);
                self.walk_lhs_select_reads(
                    parent_path,
                    actual,
                    &mut seen,
                    &mut HashSet::new(),
                    &mut reads,
                )?;
                (lhs, sig_read_expr_full(&child_info), reads)
            };
            let rhs = apply_lhs_assignment_context(&self.model, &lhs, rhs);
            let shape = if reads.is_empty() {
                IrShape::RunOnce
            } else {
                IrShape::SensLoop { reads }
            };
            let fn_name = self.new_fn_name(parent_path, "link");
            let origin = self.origin(port);
            self.model.processes.push(IrProcess::new_with_origin(
                fn_name,
                format!("{child_path}.link"),
                shape,
                Vec::new(),
                vec![IrStmt::Assign {
                    lhs,
                    rhs,
                    nba: false,
                }],
                origin,
            ));
        }
        Ok(())
    }

    // ── Processes ──────────────────────────────────────────────────────────

    /// Validate process-family contracts before any process is lowered. These
    /// checks intentionally live in the simulator semantic boundary rather
    /// than in lint: disabling lint must not turn an invalid process into a
    /// generated model with different scheduling semantics.
    pub(super) fn validate_process_semantics(&mut self) -> Result<(), String> {
        let process_ids: Vec<NodeId> = self
            .design_nodes()
            .into_iter()
            .filter(|id| matches!(self.kind(*id), NodeKind::Process { .. }))
            .collect();
        let mut writers = Vec::new();
        for process in process_ids {
            let Some(inst) = self.owning_inst(process) else {
                continue;
            };
            self.inst = inst;
            let (always_type, stmt) = match self.kind(process) {
                NodeKind::Process {
                    kind: ProcessKind::Always { always_type },
                } => (
                    Some(*always_type),
                    self.node(process).children.first().copied(),
                ),
                NodeKind::Process {
                    kind: ProcessKind::Initial | ProcessKind::Final,
                } => (None, self.node(process).children.first().copied()),
                _ => (None, None),
            };
            let stmt = stmt.ok_or_else(|| {
                format!(
                    "process `{}` has no executable statement",
                    self.node(process).full_name()
                )
            })?;
            let path = self.instance_path_of(inst);
            self.validate_process_contract(&path, stmt, always_type)?;
            let writes = self.collect_process_writes(stmt)?;
            let label = self.process_kind_label(always_type);
            writers.push(ProcessWriter {
                node: process,
                label: format!("{path}.{label}"),
                writes,
            });
        }

        // Continuous assignments are independent drivers. Declaration
        // initializers for variables/arrays are initialization, not another
        // process writer; true nets retain their continuous-driver identity.
        for id in self.db.node_ids() {
            if !matches!(self.kind(id), NodeKind::ContAssign { .. })
                || !self.is_runtime_continuous_driver(id)
            {
                continue;
            }
            let Some(inst) = self.owning_inst(id) else {
                continue;
            };
            self.inst = inst;
            writers.push(ProcessWriter {
                node: id,
                label: format!("{}.continuous", self.instance_path_of(inst)),
                writes: self.collect_process_writes(id)?,
            });
        }

        // A structural gate is also a driver of its output terminal. Include
        // it so an always-family process cannot silently share that storage.
        for id in self.db.node_ids() {
            let NodeKind::Gate { terms, .. } = self.kind(id) else {
                continue;
            };
            let Some(inst) = self.owning_inst(id) else {
                continue;
            };
            self.inst = inst;
            let mut writes = HashSet::new();
            for term in terms {
                if matches!(term.direction, DbDirection::Output | DbDirection::Inout) {
                    self.add_process_lhs_write(term.expr, &mut writes);
                }
            }
            if !writes.is_empty() {
                writers.push(ProcessWriter {
                    node: id,
                    label: format!("{}.gate", self.instance_path_of(inst)),
                    writes,
                });
            }
        }

        // A connected port link is a driver on the side it writes: an input
        // link writes the child storage, an output link writes the parent
        // actual, and an inout link writes both. Treat links as writers here
        // so an always-family process cannot share storage with a port path.
        for id in self.design_nodes() {
            let (direction, high, low, high_expr) = match self.kind(id) {
                NodeKind::Port {
                    direction,
                    high,
                    low,
                    high_expr,
                    ..
                } => (*direction, *high, *low, *high_expr),
                _ => continue,
            };
            let Some(inst) = self.owning_inst(id) else {
                continue;
            };
            let targets = match direction {
                DbDirection::Input => vec![low],
                DbDirection::Output => vec![high_expr.or(high)],
                DbDirection::Inout => vec![low, high_expr.or(high)],
                DbDirection::Mixed
                | DbDirection::None
                | DbDirection::Ref
                | DbDirection::Unsupported => Vec::new(),
            };
            let mut writes = HashSet::new();
            self.inst = inst;
            for target in targets.into_iter().flatten() {
                self.add_process_lhs_write(target, &mut writes);
            }
            if !writes.is_empty() {
                writers.push(ProcessWriter {
                    node: id,
                    label: format!("{}.port", self.instance_path_of(inst)),
                    writes,
                });
            }
        }

        for restricted in writers.iter().filter(|writer| {
            matches!(
                self.kind(writer.node),
                NodeKind::Process {
                    kind: ProcessKind::Always {
                        always_type: AlwaysKind::Comb | AlwaysKind::Latch | AlwaysKind::FlipFlop,
                    },
                }
            )
        }) {
            for other in &writers {
                if restricted.node == other.node {
                    continue;
                }
                if let Some(storage) = restricted.writes.iter().find(|write| {
                    other
                        .writes
                        .iter()
                        .any(|candidate| self.same_storage(write, candidate))
                }) {
                    return Err(format!(
                        "semantic error: process `{}` has multiple writers for `{}` at {} (also written by `{}` at {})",
                        restricted.label,
                        self.dependency_label(storage),
                        self.source_location(restricted.node),
                        other.label,
                        self.source_location(other.node),
                    ));
                }
            }
        }
        Ok(())
    }

    fn process_kind_label(&self, process_kind: Option<AlwaysKind>) -> &'static str {
        match process_kind {
            Some(AlwaysKind::Comb) => "always_comb",
            Some(AlwaysKind::Latch) => "always_latch",
            Some(AlwaysKind::FlipFlop) => "always_ff",
            Some(AlwaysKind::Always) | Some(AlwaysKind::Unsupported) => "always",
            None => "process",
        }
    }

    fn dependency_label(&self, dependency: &IrDependency) -> String {
        match dependency {
            IrDependency::Scalar(name) | IrDependency::Real(name) => name.clone(),
            IrDependency::ArrayElement { array, index } => {
                format!("array[{array}] element {index}")
            }
            IrDependency::ArrayContents(array) => format!("array[{array}] contents"),
            IrDependency::ContainerContents(container) => {
                format!("container[{container}] contents")
            }
            IrDependency::ContainerShape(container) => format!("container[{container}] shape"),
            IrDependency::Object(object) => format!("object[{object}] contents"),
        }
    }

    fn is_runtime_continuous_driver(&self, node: NodeId) -> bool {
        match self.kind(node) {
            NodeKind::ContAssign { net_decl: true, .. } => {
                matches!(self.net_decl_target(node), NetDeclTarget::TrueNet)
            }
            NodeKind::ContAssign { .. } => true,
            _ => false,
        }
    }

    fn validate_process_contract(
        &self,
        path: &str,
        stmt: NodeId,
        process_kind: Option<AlwaysKind>,
    ) -> Result<(), String> {
        let Some(process_kind) = process_kind else {
            return Ok(());
        };
        let mut scan = ProcessContractScan::default();
        let mut visited = HashSet::new();
        self.scan_process_contract(stmt, &mut scan, &mut visited)?;
        let label = self.process_kind_label(Some(process_kind));
        if matches!(process_kind, AlwaysKind::Comb | AlwaysKind::Latch)
            && (!scan.event_controls.is_empty()
                || !scan.blocking_timing_controls.is_empty()
                || !scan.fork_controls.is_empty())
        {
            let node = scan
                .event_controls
                .first()
                .or_else(|| scan.blocking_timing_controls.first())
                .or_else(|| scan.fork_controls.first())
                .copied()
                .unwrap_or(stmt);
            return Err(format!(
                "semantic error: {label} process `{path}` cannot contain a blocking timing control or fork at {}",
                self.source_location(node)
            ));
        }
        if process_kind == AlwaysKind::FlipFlop {
            if scan.event_controls.len() != 1 {
                return Err(format!(
                    "semantic error: always_ff process `{path}` must contain exactly one event control (found {})",
                    scan.event_controls.len()
                ));
            }
            if let Some(node) = scan.blocking_timing_controls.first() {
                return Err(format!(
                    "semantic error: always_ff process `{path}` cannot contain a timing control at {}",
                    self.source_location(*node)
                ));
            }
            if let Some(node) = scan.fork_controls.first() {
                return Err(format!(
                    "semantic error: always_ff process `{path}` cannot contain a fork at {}",
                    self.source_location(*node)
                ));
            }
            if let Some(node) = scan.event_triggers.first() {
                return Err(format!(
                    "semantic error: always_ff process `{path}` cannot trigger an event at {}",
                    self.source_location(*node)
                ));
            }
            if let Some(node) = scan.disallowed_assignments.first() {
                return Err(format!(
                    "semantic error: always_ff process `{path}` contains an unsupported procedural assignment at {}",
                    self.source_location(*node)
                ));
            }
        }
        Ok(())
    }

    fn source_location(&self, node: NodeId) -> String {
        let node = self.node(node);
        format!(
            "{}:{}:{}",
            node.file.as_deref().unwrap_or("<unknown>"),
            node.line,
            node.col
        )
    }

    fn scan_process_contract(
        &self,
        node: NodeId,
        scan: &mut ProcessContractScan,
        visited_functions: &mut HashSet<NodeId>,
    ) -> Result<(), String> {
        match self.kind(node) {
            NodeKind::FuncCall { .. } if self.is_process_self_call(node) => return Ok(()),
            NodeKind::Stmt(StmtKind::EventControl { .. }) => scan.event_controls.push(node),
            NodeKind::Stmt(StmtKind::DelayControl { .. })
            | NodeKind::Stmt(StmtKind::Wait { .. })
            | NodeKind::Stmt(StmtKind::WaitOrder { .. })
            | NodeKind::Stmt(StmtKind::WaitFork) => {
                scan.blocking_timing_controls.push(node);
            }
            NodeKind::Stmt(StmtKind::Assign {
                blocking: true,
                delay,
                ..
            }) => {
                if delay.is_some() {
                    scan.blocking_timing_controls.push(node);
                }
            }
            NodeKind::Stmt(StmtKind::Fork { join_kind, .. }) => {
                scan.fork_controls.push(node);
                if *join_kind != DbJoinKind::None {
                    scan.blocking_timing_controls.push(node);
                }
            }
            NodeKind::Stmt(
                StmtKind::ProcContAssign { .. }
                | StmtKind::Force { .. }
                | StmtKind::Release { .. }
                | StmtKind::Deassign { .. },
            ) => scan.disallowed_assignments.push(node),
            NodeKind::Stmt(StmtKind::EventTrigger { .. }) => {
                scan.event_triggers.push(node);
            }
            NodeKind::FuncCall {
                name,
                is_task,
                callee,
            } => {
                let (function, _) = self.resolve_callee_env(self.inst, name, *is_task, *callee)?;
                if visited_functions.insert(function) {
                    if let Some(body) = self.func_body(function) {
                        self.scan_process_contract(body, scan, visited_functions)?;
                    }
                }
            }
            _ => {}
        }
        for child in &self.node(node).children {
            self.scan_process_contract(*child, scan, visited_functions)?;
        }
        Ok(())
    }

    fn emit_process(&mut self, inst: NodeId, path: &str, proc: NodeId) -> Result<(), String> {
        // Effect collection resolves callees before EmitCtx::new installs its
        // context. Never reuse the previous process's instance for this scan.
        self.inst = inst;
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
        let always_type = match kind {
            ProcessKind::Always { always_type } => Some(*always_type),
            ProcessKind::Initial | ProcessKind::Final => None,
        };
        let ir_kind = match kind {
            ProcessKind::Initial => IrProcessKind::Initial,
            ProcessKind::Final => IrProcessKind::Final,
            ProcessKind::Always {
                always_type: AlwaysKind::Always,
            } => IrProcessKind::Always,
            ProcessKind::Always {
                always_type: AlwaysKind::Comb,
            } => IrProcessKind::Comb,
            ProcessKind::Always {
                always_type: AlwaysKind::Latch,
            } => IrProcessKind::Latch,
            ProcessKind::Always {
                always_type: AlwaysKind::FlipFlop,
            } => IrProcessKind::FlipFlop,
            ProcessKind::Always {
                always_type: AlwaysKind::Unsupported,
            } => IrProcessKind::Always,
        };
        let mut writes: Vec<IrDependency> =
            self.collect_process_writes(stmt)?.into_iter().collect();
        writes.sort_by_key(|dependency| self.dependency_label(dependency));
        let fn_name = self.new_fn_name(path, "proc");
        let (body_stmts, mut pre_fns, shape) = {
            let mut ctx = EmitCtx::new(self, path.to_string(), inst, "0", None, None, is_final);
            ctx.process_kind = always_type;
            let body_stmts = ctx.lower_stmt(stmt)?;
            // Fork-branch coroutines and monitor/strobe evaluators attach to
            // the process (rendered ahead of it).
            let pre_fns = std::mem::take(&mut ctx.pre_fns);
            let plain_always = matches!(
                kind,
                ProcessKind::Always {
                    always_type: AlwaysKind::Always
                }
            );
            let shape = if is_initial || is_final {
                // `initial` and `final` bodies run exactly once (finals after
                // the scheduler exits — the spawn phase is decided below).
                IrShape::RunOnce
            } else if !plain_always && !ctx.saw_wait {
                // always_comb/always_latch/always_ff without any event or
                // delay control use the existing sensitivity-driven shape.
                // Run once at t=0, then re-run whenever a read signal changes.
                let sigs = ctx
                    .cg
                    .collect_process_sensitivity(path, stmt, always_type)?;
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
            (body_stmts, pre_fns, shape)
        };
        pre_fns.extend(std::mem::take(&mut self.pending_container_pre_fns));
        let kind_label = if is_initial {
            "initial"
        } else if is_final {
            "final"
        } else {
            self.process_kind_label(always_type)
        };
        let origin = self.origin(proc);
        self.model
            .processes
            .push(IrProcess::new_with_kind_and_writes(
                fn_name.clone(),
                format!("{path}.{kind_label}"),
                ir_kind,
                shape,
                writes,
                pre_fns,
                body_stmts,
                origin,
            ));
        if is_final {
            self.final_procs.push(fn_name);
        }
        Ok(())
    }

    // ── Signal reads for sensitivity ───────────────────────────────────────

    /// Collect every storage dependency read anywhere in the
    /// statement/expression tree rooted at `root` (deduped, deterministic
    /// order).
    /// Function/task calls descend into the callee bodies (guarded against
    /// recursion), so reads hidden behind a function call contribute to the
    /// sensitivity set.
    pub(super) fn collect_read_signals(
        &self,
        scope_path: &str,
        root: NodeId,
    ) -> Result<Vec<IrDependency>, String> {
        self.collect_read_dependencies(scope_path, root, true)
    }

    /// Collect the implicit sensitivity set for a plain `always @*` block.
    ///
    /// Verilog wildcard event controls use the expressions visible at the
    /// call site.  In particular, reads hidden inside a called function are
    /// not inspected; call arguments themselves remain ordinary reads.
    pub(super) fn collect_at_star_signals(
        &self,
        scope_path: &str,
        root: NodeId,
    ) -> Result<Vec<IrDependency>, String> {
        self.collect_read_dependencies_mode(scope_path, root, false)
    }

    /// Build the implicit sensitivity set for an always-family process. The
    /// SystemVerilog forms remove every storage object written by the process
    /// (including writes performed by called subroutines); plain `@*` keeps
    /// the Verilog call-site-only read walk and has no such exclusion.
    fn collect_process_sensitivity(
        &self,
        scope_path: &str,
        root: NodeId,
        process_kind: Option<AlwaysKind>,
    ) -> Result<Vec<IrDependency>, String> {
        let mut reads = if process_kind == Some(AlwaysKind::Always) {
            self.collect_at_star_signals(scope_path, root)?
        } else {
            self.collect_read_signals(scope_path, root)?
        };
        if matches!(process_kind, Some(AlwaysKind::Comb | AlwaysKind::Latch)) {
            let writes = self.collect_process_writes(root)?;
            reads.retain(|read| !writes.iter().any(|write| self.same_storage(read, write)));
        }
        Ok(reads)
    }

    fn same_storage(&self, read: &IrDependency, write: &IrDependency) -> bool {
        match (read, write) {
            (IrDependency::Scalar(read), IrDependency::Scalar(write))
            | (IrDependency::Real(read), IrDependency::Real(write))
            | (IrDependency::Scalar(read), IrDependency::Real(write))
            | (IrDependency::Real(read), IrDependency::Scalar(write)) => read == write,
            (
                IrDependency::ArrayElement {
                    array: read_array,
                    index: read_index,
                },
                IrDependency::ArrayElement {
                    array: write_array,
                    index: write_index,
                },
            ) => read_array == write_array && read_index == write_index,
            (IrDependency::ArrayElement { array, .. }, IrDependency::ArrayContents(write))
            | (IrDependency::ArrayContents(array), IrDependency::ArrayContents(write))
            | (
                IrDependency::ArrayContents(array),
                IrDependency::ArrayElement { array: write, .. },
            ) => array == write,
            (
                IrDependency::ContainerContents(container)
                | IrDependency::ContainerShape(container),
                IrDependency::ContainerContents(write) | IrDependency::ContainerShape(write),
            ) => container == write,
            (IrDependency::Object(read), IrDependency::Object(write)) => read == write,
            _ => false,
        }
    }

    fn collect_process_writes(&self, root: NodeId) -> Result<HashSet<IrDependency>, String> {
        let mut writes = HashSet::new();
        let mut visited = HashSet::new();
        self.walk_process_writes(root, &mut writes, &mut visited)?;
        Ok(writes)
    }

    fn walk_process_writes(
        &self,
        node: NodeId,
        writes: &mut HashSet<IrDependency>,
        visited_functions: &mut HashSet<NodeId>,
    ) -> Result<(), String> {
        match self.kind(node) {
            NodeKind::FuncCall { .. } if self.is_process_self_call(node) => return Ok(()),
            NodeKind::Stmt(StmtKind::Assign { .. })
            | NodeKind::Stmt(StmtKind::ProcContAssign { .. })
            | NodeKind::Stmt(StmtKind::Force { .. })
            | NodeKind::ContAssign { .. } => {
                if let Some(lhs) = self.node(node).children.first() {
                    self.add_process_lhs_write(*lhs, writes);
                }
            }
            NodeKind::Stmt(StmtKind::Release { lhs })
            | NodeKind::Stmt(StmtKind::Deassign { lhs }) => {
                self.add_process_lhs_write(*lhs, writes);
                return Ok(());
            }
            NodeKind::Stmt(StmtKind::VariableDecl { declaration }) => {
                // A declaration itself is local storage, not a process write
                // for implicit-sensitivity purposes. Its initializer can
                // still call a side-effecting function.
                if let Some(initializer) = self.db.var_initializer(*declaration) {
                    self.walk_process_writes(initializer, writes, visited_functions)?;
                }
                return Ok(());
            }
            NodeKind::Expr(ExprKind::Operation {
                op:
                    Operation::PostIncrement
                    | Operation::PreIncrement
                    | Operation::PostDecrement
                    | Operation::PreDecrement
                    | Operation::Assignment,
                operands,
                ..
            }) => {
                if let Some(lhs) = operands.first() {
                    self.add_process_lhs_write(*lhs, writes);
                }
            }
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
            } if Self::mutating_container_method(name) => {
                self.add_process_lhs_write(*receiver, writes);
            }
            NodeKind::FuncCall {
                name,
                is_task,
                callee,
            } => {
                let (ft, callee_inst) =
                    self.resolve_callee_env(self.inst, name, *is_task, *callee)?;
                if visited_functions.insert(ft) {
                    if let Some(body) = self.func_body(ft) {
                        self.walk_process_writes(body, writes, visited_functions)?;
                    }
                }
                let (_, _, formals) = self.func_info(ft, callee_inst)?;
                for ((formal, is_out), actual) in formals.iter().zip(&self.node(node).children) {
                    let writes_actual = *is_out
                        || matches!(
                            self.kind(*formal),
                            NodeKind::FuncArg {
                                direction: DbDirection::Ref,
                                const_ref: false,
                                ..
                            }
                        );
                    if writes_actual {
                        self.add_process_lhs_write(self.unwrap_output_actual(*actual), writes);
                    }
                }
            }
            _ => {}
        }
        for child in &self.node(node).children {
            self.walk_process_writes(*child, writes, visited_functions)?;
        }
        Ok(())
    }

    fn mutating_container_method(name: &str) -> bool {
        matches!(
            name,
            "delete"
                | "push_front"
                | "push_back"
                | "pop_front"
                | "pop_back"
                | "insert"
                | "sort"
                | "rsort"
                | "reverse"
                | "shuffle"
        )
    }

    fn unwrap_output_actual(&self, node: NodeId) -> NodeId {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Operation { op, operands, .. })
                if *op == Operation::Assignment =>
            {
                operands.first().copied().unwrap_or(node)
            }
            _ => node,
        }
    }

    fn add_process_lhs_write(&self, lhs: NodeId, writes: &mut HashSet<IrDependency>) {
        match self.kind(lhs) {
            NodeKind::Net { .. } | NodeKind::Var { .. } => {
                if let Some(info) = self.signal_of(lhs) {
                    writes.insert(self.signal_dependency(info));
                }
            }
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => {
                if let Some(array) = self.array_of(*target) {
                    writes.insert(IrDependency::ArrayContents(self.reference_array(array.ir)));
                } else if let Some(container) = self.container_of(*target) {
                    writes.insert(IrDependency::ContainerContents(container.ir));
                    writes.insert(IrDependency::ContainerShape(container.ir));
                } else if let Some(info) = self.signal_of(*target) {
                    writes.insert(self.signal_dependency(info));
                }
            }
            NodeKind::Expr(ExprKind::HierPath { .. }) => {
                if let Some((_, _, member)) = self.unpacked_member_info(lhs) {
                    if let Some(signal) = member.signal.as_ref() {
                        writes.insert(self.signal_dependency(signal));
                    }
                } else if let Some(info) = self.hier_path_signal(lhs) {
                    writes.insert(self.signal_dependency(info));
                }
            }
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                if let Some(array) = self.array_of(*base) {
                    self.add_process_array_write(array, &[*index], writes);
                } else if let Some(container) = self.container_of(*base) {
                    writes.insert(IrDependency::ContainerContents(container.ir));
                    writes.insert(IrDependency::ContainerShape(container.ir));
                } else {
                    self.add_process_lhs_write(*base, writes);
                }
            }
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                if let Some(array) = self.array_of(*base) {
                    self.add_process_array_write(array, indices, writes);
                } else if let Some(container) = self.container_of(*base) {
                    writes.insert(IrDependency::ContainerContents(container.ir));
                    writes.insert(IrDependency::ContainerShape(container.ir));
                } else {
                    self.add_process_lhs_write(*base, writes);
                }
            }
            NodeKind::Expr(
                ExprKind::PartSelect { base, .. } | ExprKind::IndexedPartSelect { base, .. },
            ) => {
                self.add_process_lhs_write(*base, writes);
            }
            NodeKind::Expr(ExprKind::Operation { operands, .. }) => {
                for operand in operands {
                    self.add_process_lhs_write(*operand, writes);
                }
            }
            _ => {}
        }
    }

    fn add_process_array_write(
        &self,
        array: &ArrayInfo,
        indices: &[NodeId],
        writes: &mut HashSet<IrDependency>,
    ) {
        let mut dependencies = Vec::new();
        let mut seen = HashSet::new();
        self.add_fixed_array_dependency(array, indices, &mut seen, &mut dependencies);
        writes.extend(dependencies);
    }

    /// Collect force-expression dependencies, including real-valued storage.
    /// Unlike a combinational process sensitivity list, a force evaluator is
    /// driven by the runtime's typed dependency table, so real reads are
    /// observable without requiring a packed wait source.
    pub(super) fn collect_force_read_signals(
        &self,
        scope_path: &str,
        root: NodeId,
    ) -> Result<Vec<String>, String> {
        let dependencies = self.collect_read_dependencies(scope_path, root, true)?;
        dependencies
            .into_iter()
            .map(|dependency| match dependency {
                IrDependency::Scalar(name) | IrDependency::Real(name) => Ok(name),
                IrDependency::ArrayElement { .. }
                | IrDependency::ArrayContents(_)
                | IrDependency::ContainerContents(_)
                | IrDependency::ContainerShape(_)
                | IrDependency::Object(_) => Err(format!(
                    "array/container dependencies cannot yet drive force evaluators in `{scope_path}`"
                )),
            })
            .collect()
    }

    fn collect_read_dependencies(
        &self,
        scope_path: &str,
        root: NodeId,
        _allow_real: bool,
    ) -> Result<Vec<IrDependency>, String> {
        self.collect_read_dependencies_mode(scope_path, root, true)
    }

    fn collect_read_dependencies_mode(
        &self,
        scope_path: &str,
        root: NodeId,
        include_function_bodies: bool,
    ) -> Result<Vec<IrDependency>, String> {
        let mut out = Vec::new();
        let mut seen: HashSet<IrDependency> = HashSet::new();
        let mut visited: HashSet<NodeId> = HashSet::new();
        self.walk_read_signals_mode(
            scope_path,
            root,
            &mut seen,
            &mut visited,
            &mut out,
            include_function_bodies,
        )?;
        Ok(out)
    }

    pub(super) fn walk_read_signals(
        &self,
        scope_path: &str,
        node: NodeId,
        seen: &mut HashSet<IrDependency>,
        visited: &mut HashSet<NodeId>,
        out: &mut Vec<IrDependency>,
    ) -> Result<(), String> {
        self.walk_read_signals_mode(scope_path, node, seen, visited, out, true)
    }

    fn walk_read_signals_mode(
        &self,
        scope_path: &str,
        node: NodeId,
        seen: &mut HashSet<IrDependency>,
        visited: &mut HashSet<NodeId>,
        out: &mut Vec<IrDependency>,
        include_function_bodies: bool,
    ) -> Result<(), String> {
        if self.is_process_self_call(node) {
            return Ok(());
        }
        if self.object_of(scope_path, node).is_some() {
            return Err(format!("string/chandle changes cannot yet be used in sensitivity or wait expressions in `{scope_path}`"));
        }
        match self.kind(node) {
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                if let Some(array) = self.array_of(*base) {
                    self.add_fixed_array_dependency(array, &[*index], seen, out);
                    return self.walk_read_signals_mode(
                        scope_path,
                        *index,
                        seen,
                        visited,
                        out,
                        include_function_bodies,
                    );
                }
                if let Some(container) = self.container_of(*base) {
                    self.add_container_dependencies(container.ir, true, true, seen, out);
                    return self.walk_read_signals_mode(
                        scope_path,
                        *index,
                        seen,
                        visited,
                        out,
                        include_function_bodies,
                    );
                }
            }
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                if let Some(array) = self.array_of(*base) {
                    self.add_fixed_array_dependency(array, indices, seen, out);
                    for index in indices {
                        self.walk_read_signals_mode(
                            scope_path,
                            *index,
                            seen,
                            visited,
                            out,
                            include_function_bodies,
                        )?;
                    }
                    return Ok(());
                }
                if let Some(container) = self.container_of(*base) {
                    self.add_container_dependencies(container.ir, true, true, seen, out);
                    for index in indices {
                        self.walk_read_signals_mode(
                            scope_path,
                            *index,
                            seen,
                            visited,
                            out,
                            include_function_bodies,
                        )?;
                    }
                    return Ok(());
                }
            }
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
            } => {
                if let Some(container) = self.container_of(*receiver) {
                    // A pure mutation statement has no read of the receiver.
                    // Keeping its storage out of an implicit @* set prevents
                    // an external mutation from re-running a process whose
                    // only container access is its own write. Value-producing
                    // methods (including pop_*) still read the receiver.
                    let receiver_is_read = !matches!(
                        name.as_str(),
                        "delete"
                            | "push_front"
                            | "push_back"
                            | "insert"
                            | "sort"
                            | "rsort"
                            | "reverse"
                            | "shuffle"
                    );
                    if receiver_is_read {
                        let (contents, shape) = match name.as_str() {
                            "size" | "num" | "exists" | "first" | "last" | "next" | "prev" => {
                                (false, true)
                            }
                            _ => (true, true),
                        };
                        self.add_container_dependencies(container.ir, contents, shape, seen, out);
                    }
                    for child in &self.node(node).children {
                        if *child != *receiver {
                            self.walk_read_signals_mode(
                                scope_path,
                                *child,
                                seen,
                                visited,
                                out,
                                include_function_bodies,
                            )?;
                        }
                    }
                    return Ok(());
                }
            }
            NodeKind::Stmt(StmtKind::Assign { .. })
            | NodeKind::Stmt(StmtKind::ProcContAssign { .. }) => {
                // Sensitivity of a process body: an assignment's LHS base
                // signal must NOT trigger the process (it would self-wake
                // after every write — including the dedicated PCA guard
                // process's writes).  Only the LHS's index/bounds
                // expressions are reads.
                if let Some(rhs) = self.node(node).children.get(1) {
                    self.walk_read_signals_mode(
                        scope_path,
                        *rhs,
                        seen,
                        visited,
                        out,
                        include_function_bodies,
                    )?;
                }
                if let Some(lhs) = self.node(node).children.first() {
                    self.walk_lhs_select_reads_mode(
                        scope_path,
                        *lhs,
                        seen,
                        visited,
                        out,
                        include_function_bodies,
                    )?;
                }
                return Ok(());
            }
            NodeKind::Stmt(StmtKind::VariableDecl { declaration }) => {
                if let Some(initializer) = self.db.var_initializer(*declaration) {
                    self.walk_read_signals_mode(
                        scope_path,
                        initializer,
                        seen,
                        visited,
                        out,
                        include_function_bodies,
                    )?;
                }
                return Ok(());
            }
            NodeKind::Expr(ExprKind::Operation {
                op:
                    Operation::Assignment
                    | Operation::PostIncrement
                    | Operation::PreIncrement
                    | Operation::PostDecrement
                    | Operation::PreDecrement,
                operands,
                ..
            }) => {
                if let Some(rhs) = operands.get(1) {
                    self.walk_read_signals_mode(
                        scope_path,
                        *rhs,
                        seen,
                        visited,
                        out,
                        include_function_bodies,
                    )?;
                }
                if let Some(lhs) = operands.first() {
                    self.walk_lhs_select_reads_mode(
                        scope_path,
                        *lhs,
                        seen,
                        visited,
                        out,
                        include_function_bodies,
                    )?;
                }
                return Ok(());
            }
            NodeKind::Stmt(StmtKind::For {
                vars,
                init,
                cond,
                incr,
                body,
            }) => {
                for variable in vars {
                    if let Some(initializer) = self.db.var_initializer(*variable) {
                        self.walk_read_signals_mode(
                            scope_path,
                            initializer,
                            seen,
                            visited,
                            out,
                            include_function_bodies,
                        )?;
                    }
                }
                for statement in init {
                    self.walk_read_signals_mode(
                        scope_path,
                        *statement,
                        seen,
                        visited,
                        out,
                        include_function_bodies,
                    )?;
                }
                self.walk_read_signals_mode(
                    scope_path,
                    *cond,
                    seen,
                    visited,
                    out,
                    include_function_bodies,
                )?;
                self.walk_read_signals_mode(
                    scope_path,
                    *body,
                    seen,
                    visited,
                    out,
                    include_function_bodies,
                )?;
                for statement in incr {
                    self.walk_read_signals_mode(
                        scope_path,
                        *statement,
                        seen,
                        visited,
                        out,
                        include_function_bodies,
                    )?;
                }
                return Ok(());
            }
            NodeKind::Stmt(StmtKind::Fork { branches, .. }) => {
                // A fork body reads signals (a `fork … join` branch may block
                // on signals); descend into the branches for comb sensitivity.
                for b in branches {
                    self.walk_read_signals_mode(
                        scope_path,
                        *b,
                        seen,
                        visited,
                        out,
                        include_function_bodies,
                    )?;
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
                let (ft, callee_inst) =
                    self.resolve_callee_env(self.inst, name, *is_task, *callee)?;
                if include_function_bodies && visited.insert(ft) {
                    if let Some(body) = self.func_body(ft) {
                        self.walk_read_signals_mode(
                            scope_path,
                            body,
                            seen,
                            visited,
                            out,
                            include_function_bodies,
                        )?;
                    }
                }
                let (_, _, formals) = self.func_info(ft, callee_inst)?;
                for (formal, _) in formals.iter().skip(self.node(node).children.len()) {
                    if let NodeKind::FuncArg {
                        default: Some(default),
                        ..
                    } = self.kind(*formal)
                    {
                        self.walk_read_signals_mode(
                            scope_path,
                            *default,
                            seen,
                            visited,
                            out,
                            include_function_bodies,
                        )?;
                    }
                }
                for c in &self.node(node).children {
                    self.walk_read_signals_mode(
                        scope_path,
                        *c,
                        seen,
                        visited,
                        out,
                        include_function_bodies,
                    )?;
                }
                return Ok(());
            }
            _ => {}
        }
        if let Some(container) = self.container_of(node) {
            self.add_container_dependencies(container.ir, true, true, seen, out);
        }
        if let Some((_, _, member)) = self.unpacked_member_info(node) {
            if let Some(signal) = member.signal.as_ref() {
                self.add_dependency(self.signal_dependency(signal), seen, out);
            }
            return Ok(());
        }
        if let NodeKind::Expr(ExprKind::Ref {
            target: Some(target),
        }) = self.kind(node)
        {
            if let Some(dependencies) = self
                .func
                .as_ref()
                .and_then(|func| func.arg_dependencies.get(target))
            {
                for dependency in dependencies {
                    self.add_dependency(dependency.clone(), seen, out);
                }
            }
            if let Some(array) = self.array_of(*target) {
                self.add_dependency(
                    IrDependency::ArrayContents(self.reference_array(array.ir)),
                    seen,
                    out,
                );
                return Ok(());
            }
            if let Some(container) = self.container_of(*target) {
                self.add_container_dependencies(container.ir, true, true, seen, out);
                return Ok(());
            }
        }
        if let NodeKind::Array { .. } = self.kind(node) {
            if let Some(array) = self.array_of(node) {
                self.add_dependency(
                    IrDependency::ArrayContents(self.reference_array(array.ir)),
                    seen,
                    out,
                );
                return Ok(());
            }
        }
        self.add_node_read(node, seen, out);
        for c in &self.node(node).children {
            self.walk_read_signals_mode(
                scope_path,
                *c,
                seen,
                visited,
                out,
                include_function_bodies,
            )?;
        }
        Ok(())
    }

    /// Walk only the index/bounds expressions of an assignment LHS.
    fn walk_lhs_select_reads(
        &self,
        scope_path: &str,
        lhs: NodeId,
        seen: &mut HashSet<IrDependency>,
        visited: &mut HashSet<NodeId>,
        out: &mut Vec<IrDependency>,
    ) -> Result<(), String> {
        self.walk_lhs_select_reads_mode(scope_path, lhs, seen, visited, out, true)
    }

    fn walk_lhs_select_reads_mode(
        &self,
        scope_path: &str,
        lhs: NodeId,
        seen: &mut HashSet<IrDependency>,
        visited: &mut HashSet<NodeId>,
        out: &mut Vec<IrDependency>,
        include_function_bodies: bool,
    ) -> Result<(), String> {
        match self.kind(lhs) {
            NodeKind::Expr(ExprKind::BitSelect { index, .. }) => self.walk_read_signals_mode(
                scope_path,
                *index,
                seen,
                visited,
                out,
                include_function_bodies,
            ),
            NodeKind::Expr(ExprKind::PartSelect { left, right, .. }) => {
                self.walk_read_signals_mode(
                    scope_path,
                    *left,
                    seen,
                    visited,
                    out,
                    include_function_bodies,
                )?;
                self.walk_read_signals_mode(
                    scope_path,
                    *right,
                    seen,
                    visited,
                    out,
                    include_function_bodies,
                )
            }
            NodeKind::Expr(ExprKind::IndexedPartSelect {
                base_expr,
                width_expr,
                ..
            }) => {
                self.walk_read_signals_mode(
                    scope_path,
                    *base_expr,
                    seen,
                    visited,
                    out,
                    include_function_bodies,
                )?;
                self.walk_read_signals_mode(
                    scope_path,
                    *width_expr,
                    seen,
                    visited,
                    out,
                    include_function_bodies,
                )
            }
            NodeKind::Expr(ExprKind::ArraySelect { indices, .. }) => {
                // The base is the array itself (not a read); only the index
                // expressions (and any element-level select bounds) are reads.
                for i in indices {
                    self.walk_read_signals_mode(
                        scope_path,
                        *i,
                        seen,
                        visited,
                        out,
                        include_function_bodies,
                    )?;
                }
                Ok(())
            }
            NodeKind::Expr(ExprKind::HierPath { .. }) => {
                // A hierarchical LHS base signal must not trigger the owning
                // process (same rule as a plain LHS ref). The backend supports only
                // constant indices/bounds on hierarchical targets, so there
                // are no index/bounds reads to collect.
                Ok(())
            }
            _ => Ok(()), // plain ref LHS: not part of the read set
        }
    }

    /// Add `node` to the read set if it is (or resolves to) a signal.
    fn add_node_read(
        &self,
        node: NodeId,
        seen: &mut HashSet<IrDependency>,
        out: &mut Vec<IrDependency>,
    ) {
        match self.kind(node) {
            NodeKind::Net { .. } | NodeKind::Var { .. } => {
                if let Some(info) = self.signal_of(node) {
                    self.add_dependency(self.signal_dependency(info), seen, out);
                }
            }
            NodeKind::Expr(ExprKind::Ref { target: Some(t) }) => {
                if let Some(info) = self.signal_of(*t) {
                    self.add_dependency(self.signal_dependency(info), seen, out);
                }
            }
            NodeKind::Expr(ExprKind::HierPath { .. }) => {
                if let Some(info) = self.hier_path_signal(node) {
                    self.add_dependency(self.signal_dependency(info), seen, out);
                }
            }
            _ => {}
        }
    }

    fn add_dependency(
        &self,
        dependency: IrDependency,
        seen: &mut HashSet<IrDependency>,
        out: &mut Vec<IrDependency>,
    ) {
        if seen.insert(dependency.clone()) {
            out.push(dependency);
        }
    }

    fn add_container_dependencies(
        &self,
        container: usize,
        contents: bool,
        shape: bool,
        seen: &mut HashSet<IrDependency>,
        out: &mut Vec<IrDependency>,
    ) {
        if contents {
            self.add_dependency(IrDependency::ContainerContents(container), seen, out);
        }
        if shape {
            self.add_dependency(IrDependency::ContainerShape(container), seen, out);
        }
    }

    fn add_fixed_array_dependency(
        &self,
        array: &ArrayInfo,
        indices: &[NodeId],
        seen: &mut HashSet<IrDependency>,
        out: &mut Vec<IrDependency>,
    ) {
        if indices.len() != array.dims.len() {
            self.add_dependency(
                IrDependency::ArrayContents(self.reference_array(array.ir)),
                seen,
                out,
            );
            return;
        }
        let mut linear = 0u64;
        for ((left, right), node) in array.dims.iter().zip(indices) {
            let Some(value) = self.eval_bound_i128(*node).ok() else {
                self.add_dependency(
                    IrDependency::ArrayContents(self.reference_array(array.ir)),
                    seen,
                    out,
                );
                return;
            };
            let lo = i128::from((*left).min(*right));
            let hi = i128::from((*left).max(*right));
            if value < lo || value > hi {
                return;
            }
            let offset = if left >= right {
                i128::from(*left) - value
            } else {
                value - i128::from(*left)
            };
            let extent = (i64::from(*left) - i64::from(*right)).unsigned_abs() + 1;
            linear = match linear
                .checked_mul(extent)
                .and_then(|value| value.checked_add(offset as u64))
            {
                Some(value) => value,
                None => {
                    self.add_dependency(
                        IrDependency::ArrayContents(self.reference_array(array.ir)),
                        seen,
                        out,
                    );
                    return;
                }
            };
        }
        self.add_dependency(
            IrDependency::ArrayElement {
                array: self.reference_array(array.ir),
                index: linear,
            },
            seen,
            out,
        );
    }

    // ── Signal resolution ──────────────────────────────────────────────────

    fn collect_named_event(
        &mut self,
        path: &str,
        declaration: NodeId,
        seen: &mut HashSet<String>,
    ) -> Result<(), String> {
        let name = self.node(declaration).name.clone();
        if name.is_empty() || !seen.insert(name.clone()) {
            return Ok(());
        }
        let Some(metadata) = self.db.event_array_meta(declaration) else {
            let info = self.new_event_info(event_global_name(path, &name));
            self.event_globals.insert(declaration, info);
            return Ok(());
        };
        if !matches!(metadata.kind(), ArrayKind::Static) {
            return Err(format!(
                "named event array `{name}` in `{path}` has unsupported storage kind"
            ));
        }
        let mut total = 1u64;
        let mut dims = Vec::with_capacity(metadata.dimensions().len());
        for (dimension, bounds) in metadata.dimensions().iter().enumerate() {
            let Some((left, right)) = bounds else {
                return Err(format!(
                    "named event array `{name}` in `{path}` has unresolved dimension {dimension}"
                ));
            };
            let extent = (i64::from(*left) - i64::from(*right)).unsigned_abs() + 1;
            total = total
                .checked_mul(extent)
                .ok_or_else(|| format!("named event array `{name}` in `{path}` is too large"))?;
            dims.push((*left, *right));
        }
        if dims.is_empty() {
            return Err(format!(
                "named event array `{name}` in `{path}` has no dimensions"
            ));
        }
        for linear in 0..total {
            let info = self.new_event_info(event_global_name(path, &format!("{name}_{linear}")));
            self.event_elements.insert((declaration, linear), info);
        }
        let elements = (0..total)
            .map(|linear| {
                self.event_elements
                    .get(&(declaration, linear))
                    .map(|info| info.ir)
                    .ok_or_else(|| {
                        format!("named event array `{name}` in `{path}` lost element {linear}")
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let array = self.model.events.len();
        self.model.events.push(crate::sim::ir::IrEvent::new_array(
            event_global_name(path, &name),
            dims,
            elements,
        ));
        self.event_arrays.insert(declaration, array);
        Ok(())
    }

    fn new_event_info(&mut self, global: String) -> EventInfo {
        let ir = self.model.events.len();
        let info = EventInfo { global, ir };
        self.model.events.push(IrEvent::new(info.global.clone()));
        self.events.push(info.clone());
        info
    }

    /// Resolve an event expression to its declaration and any unpacked-array
    /// indices. The expression remains owned by the DB until this point so a
    /// reassigned handle can be lowered separately from its synchronization
    /// object.
    pub(super) fn event_target_of(&self, node: NodeId) -> Option<EventTarget> {
        match self.kind(node) {
            NodeKind::NamedEvent => Some(EventTarget {
                declaration: node,
                indices: Vec::new(),
            }),
            NodeKind::Var { .. }
                if self.event_globals.contains_key(&node)
                    || self.event_arrays.contains_key(&node)
                    || self.db.event_array_meta(node).is_some() =>
            {
                Some(EventTarget {
                    declaration: node,
                    indices: Vec::new(),
                })
            }
            NodeKind::FuncArg { ty, .. } if ty.kind == "event" => Some(EventTarget {
                declaration: node,
                indices: Vec::new(),
            }),
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) if (matches!(self.kind(*target), NodeKind::NamedEvent)
                || matches!(self.kind(*target), NodeKind::Var { .. })
                    && (self.event_globals.contains_key(target)
                        || self.event_arrays.contains_key(target)
                        || self.db.event_array_meta(*target).is_some()))
                || matches!(self.kind(*target), NodeKind::FuncArg { ty, .. } if ty.kind == "event") =>
            {
                Some(EventTarget {
                    declaration: *target,
                    indices: Vec::new(),
                })
            }
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                let mut target = self.event_target_of(*base)?;
                target.indices.extend(indices.iter().copied());
                Some(target)
            }
            NodeKind::Expr(ExprKind::ScopeRef { target }) => self.event_target_of(*target),
            NodeKind::Expr(ExprKind::Cast { operand, .. }) => self.event_target_of(*operand),
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => refs
                .iter()
                .rev()
                .copied()
                .flatten()
                .find(|target| {
                    matches!(self.kind(*target), NodeKind::NamedEvent)
                        || matches!(self.kind(*target), NodeKind::Var { .. })
                            && (self.event_globals.contains_key(target)
                                || self.event_arrays.contains_key(target)
                                || self.db.event_array_meta(*target).is_some())
                })
                .map(|declaration| EventTarget {
                    declaration,
                    indices: Vec::new(),
                }),
            _ => None,
        }
    }

    pub(super) fn is_null_event_expression(&self, node: NodeId) -> bool {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Constant {
                const_type: ConstantType::Null,
                ..
            }) => true,
            NodeKind::Expr(ExprKind::Cast { operand, .. }) => {
                self.is_null_event_expression(*operand)
            }
            _ => false,
        }
    }

    /// Model index of a captured named-event expression.
    pub(super) fn event_index_of(
        &self,
        target: &EventTarget,
        scope_path: &str,
    ) -> Result<usize, String> {
        if let Some(metadata) = self.db.event_array_meta(target.declaration) {
            if target.indices.len() != metadata.dimensions().len() {
                return Err(format!(
                    "named event array `{}` in `{scope_path}` requires {} indices",
                    self.node(target.declaration).name,
                    metadata.dimensions().len()
                ));
            }
            let mut linear = 0u64;
            for (bounds, index) in metadata.dimensions().iter().zip(&target.indices) {
                let Some((left, right)) = *bounds else {
                    return Err(format!(
                        "named event array `{}` in `{scope_path}` has unresolved bounds",
                        self.node(target.declaration).name
                    ));
                };
                let value = self.eval_bound_i128(*index).map_err(|_| {
                    format!(
                        "named event array index for `{}` in `{scope_path}` must be a known constant",
                        self.node(target.declaration).name
                    )
                })?;
                let lo = i128::from(left.min(right));
                let hi = i128::from(left.max(right));
                if value < lo || value > hi {
                    return Err(format!(
                        "named event array index for `{}` in `{scope_path}` is out of range",
                        self.node(target.declaration).name
                    ));
                }
                let offset = if left >= right {
                    i128::from(left) - value
                } else {
                    value - i128::from(left)
                } as u64;
                let extent = (i64::from(left) - i64::from(right)).unsigned_abs() + 1;
                linear = linear
                    .checked_mul(extent)
                    .and_then(|value| value.checked_add(offset))
                    .ok_or_else(|| {
                        format!(
                            "named event array index for `{}` in `{scope_path}` is too large",
                            self.node(target.declaration).name
                        )
                    })?;
            }
            return self
                .event_elements
                .get(&(target.declaration, linear))
                .map(|info| info.ir)
                .ok_or_else(|| {
                    format!(
                        "cannot resolve named event array element `{}` in `{scope_path}`",
                        self.node(target.declaration).name
                    )
                });
        }
        if !target.indices.is_empty() {
            return Err(format!(
                "named event `{}` in `{scope_path}` is not an array",
                self.node(target.declaration).name
            ));
        }
        self.event_globals
            .get(&target.declaration)
            .map(|info| info.ir)
            .ok_or_else(|| {
                format!(
                    "cannot resolve named event reference `{}` in `{scope_path}`",
                    self.node(target.declaration).name
                )
            })
    }

    /// Lower an event target without losing a runtime array index. Constant
    /// selects retain the compact scalar handle path; variable selects carry
    /// typed index expressions to the runtime pointer table.
    pub(super) fn event_ref_of(
        &mut self,
        target: &EventTarget,
        scope_path: &str,
    ) -> Result<IrEventRef, String> {
        if let Some(event) = self
            .func
            .as_ref()
            .and_then(|function| function.event_args.get(&target.declaration))
        {
            if !target.indices.is_empty() {
                return Err(format!(
                    "event formal `{}` cannot be indexed in `{scope_path}`",
                    self.node(target.declaration).name
                ));
            }
            return Ok(event.clone());
        }
        if matches!(
            self.kind(target.declaration),
            NodeKind::FuncArg { ty, .. } if ty.kind == "event"
        ) {
            // Event-formal definitions are never called through a C ABI
            // function: call sites inline them so the handle identity remains
            // an alias of the actual. Keep the separately emitted body
            // structurally valid for model construction; its null fallback is
            // unreachable from an admitted call.
            return Ok(IrEventRef::Null);
        }
        if self.db.event_array_meta(target.declaration).is_some() {
            if target.indices.len()
                != self
                    .db
                    .event_array_meta(target.declaration)
                    .expect("event metadata checked above")
                    .dimensions()
                    .len()
            {
                return Err(format!(
                    "named event array `{}` requires {} indices",
                    self.node(target.declaration).name,
                    self.db
                        .event_array_meta(target.declaration)
                        .expect("event metadata checked above")
                        .dimensions()
                        .len()
                ));
            }
            if target
                .indices
                .iter()
                .all(|index| self.eval_bound_i128(*index).is_ok())
            {
                return Ok(IrEventRef::Static(self.event_index_of(target, scope_path)?));
            }
            let array = self
                .event_arrays
                .get(&target.declaration)
                .copied()
                .ok_or_else(|| {
                    format!(
                        "cannot resolve named event array `{}` in `{scope_path}`",
                        self.node(target.declaration).name
                    )
                })?;
            let indices = target
                .indices
                .iter()
                .map(|index| {
                    let expression = self.lower_expr(scope_path, *index)?;
                    if expression.is_real() {
                        return Err(format!(
                            "named event array index for `{}` in `{scope_path}` must be integral",
                            self.node(target.declaration).name
                        ));
                    }
                    Ok(expression)
                })
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(IrEventRef::Array { array, indices });
        }
        Ok(IrEventRef::Static(self.event_index_of(target, scope_path)?))
    }

    /// Resolve a net/var/ref node to a global signal name (used by port
    /// links and event sensitivities).
    pub(super) fn resolve_signal_id(
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
        Err(format!(
            "cannot resolve signal reference `{name}` in `{scope_path}`"
        ))
    }

    /// Resolve a select node's base arena node to its global signal.
    pub(super) fn base_signal(
        &self,
        _scope_path: &str,
        base: NodeId,
    ) -> Result<(String, SignalInfo), String> {
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

    /// Resolve a whole-signal assignment target by its bound declaration.
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
        Err(format!("cannot resolve assignment target `{name}`"))
    }

    // ── LHS analysis ───────────────────────────────────────────────────────

    fn array_element_lhs(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<ArrayElemLhs>, String> {
        let base = match self.kind(node) {
            NodeKind::Expr(
                ExprKind::BitSelect { base, .. } | ExprKind::ArraySelect { base, .. },
            ) => *base,
            _ => return Ok(None),
        };
        if self.array_of(base).is_none() {
            return Ok(None);
        }
        match self.analyze_lhs(path, node)? {
            Lhs::ArrayElem(element) if matches!(element.elem_sel, ElemSel::Whole) => {
                Ok(Some(element))
            }
            _ => Err(format!(
                "nested selects of an array element are not supported in `{path}`"
            )),
        }
    }

    pub(super) fn fixed_stream_lhs_parts(
        &mut self,
        path: &str,
        value: NodeId,
        with_node: Option<NodeId>,
    ) -> Result<Option<Vec<Lhs>>, String> {
        let Some(array) = self.array_of(value).cloned() else {
            return Ok(None);
        };
        if array.real {
            return Err(format!(
                "real array streaming assignment target is not supported in `{path}`"
            ));
        }
        let selected = match with_node {
            Some(with_node) => self
                .static_stream_selector_indices(path, with_node)?
                .ok_or_else(|| {
                    format!(
                        "runtime `with` selector on a fixed streaming target is not supported in `{path}`"
                    )
                })?,
            None => {
                let (left, right) = array.dims.first().copied().ok_or_else(|| {
                    format!("fixed streaming target has no dimensions in `{path}`")
                })?;
                let step = if left <= right { 1 } else { -1 };
                let mut values = Vec::new();
                let mut index = i128::from(left);
                loop {
                    values.push(index);
                    if index == i128::from(right) {
                        break;
                    }
                    index += i128::from(step);
                }
                values
            }
        };
        let rest = port_array_index_vectors(&array.dims[1..]);
        let mut parts = Vec::new();
        for index in selected {
            for suffix in &rest {
                let mut indices = Vec::with_capacity(1 + suffix.len());
                indices.push(lhs_integer_expr(index));
                indices.extend(
                    suffix
                        .iter()
                        .map(|value| lhs_integer_expr(i128::from(*value))),
                );
                parts.push(Lhs::ArrayElem(ArrayElemLhs {
                    arr: array.clone(),
                    indices,
                    elem_sel: ElemSel::Whole,
                }));
            }
        }
        if parts.is_empty() {
            return Err(format!("empty fixed streaming target in `{path}`"));
        }
        Ok(Some(parts))
    }

    pub(super) fn analyze_lhs(&mut self, path: &str, lhs: NodeId) -> Result<Lhs, String> {
        match self.kind(lhs) {
            NodeKind::Expr(ExprKind::Streaming {
                direction,
                slice_size,
                streams,
            }) => {
                if streams.is_empty() {
                    return Err(format!("empty streaming assignment target in `{path}`"));
                }
                let streams = streams.clone();
                let mut parts = Vec::new();
                for stream in streams {
                    if let Some(fixed_parts) =
                        self.fixed_stream_lhs_parts(path, stream.value, stream.with_expr)?
                    {
                        parts.extend(fixed_parts);
                        continue;
                    }
                    match self.kind(stream.value) {
                        NodeKind::Expr(ExprKind::Operation {
                            op: Operation::Concat,
                            reordered,
                            operands,
                            ..
                        }) => {
                            let mut operands = operands.clone();
                            if *reordered {
                                operands.reverse();
                            }
                            for operand in operands {
                                parts.push(self.analyze_lhs(path, operand)?);
                            }
                        }
                        _ => parts.push(self.analyze_lhs(path, stream.value)?),
                    }
                }
                Ok(Lhs::Stream {
                    parts,
                    slice: (*slice_size != 0).then_some(u128::from(*slice_size)),
                    direction: match direction {
                        DbStreamingDirection::LeftToRight => IrStreamDirection::LeftToRight,
                        DbStreamingDirection::RightToLeft => IrStreamDirection::RightToLeft,
                    },
                })
            }
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Concat,
                reordered,
                operands,
                ..
            }) => {
                if operands.is_empty() {
                    return Err(format!("empty concatenation assignment target in `{path}`"));
                }
                let mut operands = operands.clone();
                if *reordered {
                    operands.reverse();
                }
                let parts = operands
                    .into_iter()
                    .map(|operand| self.analyze_lhs(path, operand))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Lhs::Stream {
                    parts,
                    slice: Some(1),
                    direction: IrStreamDirection::LeftToRight,
                })
            }
            NodeKind::Var { .. } => {
                if let Some(info) = self.proc_local_info(lhs) {
                    if let Some(signal) = &info.static_signal {
                        return Ok(Lhs::Whole(signal.clone()));
                    }
                    return Ok(Lhs::WholeRef {
                        addr: format!("&{}", info.c_name),
                        width: info.width,
                        signed: info.signed,
                        two_state: info.two_state,
                        shortreal: false,
                    });
                }
                let name = self.node(lhs).name.clone();
                self.func_write_target(lhs, &name).ok_or_else(|| {
                    if self.is_const_ref_target(lhs, &name) {
                        format!("cannot write through const ref `{name}` in `{path}`")
                    } else {
                        format!("cannot resolve procedural variable `{name}` in `{path}`")
                    }
                })
            }
            NodeKind::Expr(ExprKind::Ref { target }) => {
                if let Some((_, info)) = self.lexical_proc_local(lhs) {
                    if let Some(signal) = &info.static_signal {
                        return Ok(Lhs::Whole(signal.clone()));
                    }
                    return Ok(Lhs::WholeRef {
                        addr: format!("&{}", info.c_name),
                        width: info.width,
                        signed: info.signed,
                        two_state: info.two_state,
                        shortreal: false,
                    });
                }
                if let Some(t) = *target {
                    if let Some(info) = self.signal_of(t) {
                        return Ok(Lhs::Whole(info.clone()));
                    }
                    if !self.proc_local_is_shadowed(lhs) {
                        if let Some(info) = self.proc_local_info(t) {
                            if let Some(signal) = &info.static_signal {
                                return Ok(Lhs::Whole(signal.clone()));
                            }
                            return Ok(Lhs::WholeRef {
                                addr: format!("&{}", info.c_name),
                                width: info.width,
                                signed: info.signed,
                                two_state: info.two_state,
                                shortreal: false,
                            });
                        }
                    }
                    // Function/task body writes: output/inout formals, locals
                    // and the return variable (by arena node).
                    if let Some(lh) = self.func_write_target(t, "") {
                        return Ok(lh);
                    }
                    if self.is_const_ref_target(t, &self.node(lhs).name) {
                        return Err(format!(
                            "cannot write through const ref `{}` in `{path}`",
                            self.node(lhs).name
                        ));
                    }
                }
                let name = self.node(lhs).name.clone();
                if target.is_none() && !name.is_empty() {
                    // io_decls are not indexed, so formals resolve by name.
                    if let Some(lh) = self.func_write_target(NodeId(0), &name) {
                        return Ok(lh);
                    }
                    if self.is_const_ref_target(NodeId(0), &name) {
                        return Err(format!(
                            "cannot write through const ref `{name}` in `{path}`"
                        ));
                    }
                }
                let (name, info) = self.resolve_lhs_target(lhs, *target)?;
                Ok(Lhs::Whole(SignalInfo {
                    global: name,
                    ..info
                }))
            }
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                if let Some(mut element) = self.array_element_lhs(path, *base)? {
                    if element.arr.real {
                        return Err(format!(
                            "select on a real array element in `{path}` is not supported"
                        ));
                    }
                    element.elem_sel = ElemSel::Bit(self.lower_packed_index(path, *base, *index)?);
                    return Ok(Lhs::ArrayElem(element));
                }
                if let Some((info, member, lsb, width)) =
                    self.packed_member_select_info(*base, &[*index])?
                {
                    return Ok(Lhs::Part(
                        info,
                        i128::from(lsb) + i128::from(width) - 1,
                        i128::from(lsb),
                        member.two_state,
                    ));
                }
                if let Some((info, member)) = self.packed_member_info(*base) {
                    let index = self.eval_bound_i128(*index)?;
                    let relative = self.aggregate_member_relative_bound(
                        &member.name,
                        &member.packed_ranges,
                        index,
                    )?;
                    return Ok(Lhs::Bit(
                        info,
                        lhs_integer_expr(i128::from(member.lsb) + i128::from(relative)),
                        member.two_state,
                    ));
                }
                if let Some(ai) = self.array_of(*base).cloned() {
                    if ai.dims.len() != 1 {
                        return Err(format!(
                            "array slice access (`{}[...]` on a {}-dimensional array) \
                             is not supported in `{path}`",
                            self.node(*base).name,
                            ai.dims.len()
                        ));
                    }
                    let index = self.lower_expr(path, *index)?;
                    return Ok(Lhs::ArrayElem(ArrayElemLhs {
                        arr: ai,
                        indices: vec![index],
                        elem_sel: ElemSel::Whole,
                    }));
                }
                if let Some((info, lsb, width)) = self.packed_select_info(*base, &[*index])? {
                    if width > 1 {
                        let right = i128::from(lsb);
                        let two_state = info.two_state;
                        return Ok(Lhs::Part(
                            info,
                            right + i128::from(width) - 1,
                            right,
                            two_state,
                        ));
                    }
                }
                let (_, info) = self.base_signal(path, *base)?;
                if info.real {
                    return Err(format!(
                        "select on real-valued signal in `{path}` is not supported"
                    ));
                }
                let index = self.lower_packed_index(path, *base, *index)?;
                let two_state = info.two_state;
                Ok(Lhs::Bit(info, index, two_state))
            }
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                if let Some((_target, _kind, member_info)) = self.unpacked_member_info(lhs) {
                    let member = member_info.member;
                    let info = member_info.signal.ok_or_else(|| {
                        format!(
                            "aggregate member `{}` is not a packed assignment target",
                            member.name
                        )
                    })?;
                    if info.real {
                        return Ok(Lhs::Whole(info));
                    }
                    let width = member.ty.width.ok_or_else(|| {
                        format!("unpacked member `{}` has unresolved width", member.name)
                    })?;
                    return Ok(Lhs::Part(info, i128::from(width - 1), 0, member.two_state));
                }
                if let Some((info, member, lsb, width)) =
                    self.packed_member_select_info(*base, indices)?
                {
                    return Ok(Lhs::Part(
                        info,
                        i128::from(lsb) + i128::from(width) - 1,
                        i128::from(lsb),
                        member.two_state,
                    ));
                }
                if let Some((info, lsb, width)) = self.packed_select_info(*base, indices)? {
                    let right = i128::from(lsb);
                    let two_state = info.two_state;
                    return Ok(Lhs::Part(
                        info,
                        right + i128::from(width) - 1,
                        right,
                        two_state,
                    ));
                }
                let ai = self.array_of(*base).cloned().ok_or_else(|| {
                    format!(
                        "cannot resolve array base of select `{}` in `{path}`",
                        self.node(*base).name,
                    )
                })?;
                let ndims = ai.dims.len();
                if indices.len() == ndims {
                    let ies = indices
                        .iter()
                        .map(|i| self.lower_expr(path, *i))
                        .collect::<Result<Vec<_>, _>>()?;
                    return Ok(Lhs::ArrayElem(ArrayElemLhs {
                        arr: ai,
                        indices: ies,
                        elem_sel: ElemSel::Whole,
                    }));
                }
                if indices.len() == ndims + 1 {
                    if ai.real {
                        return Err(format!(
                            "select on a real array element in `{path}` is not supported"
                        ));
                    }
                    let last = *indices.last().expect("non-empty indices");
                    let ies = indices[..ndims]
                        .iter()
                        .map(|i| self.lower_expr(path, *i))
                        .collect::<Result<Vec<_>, _>>()?;
                    let elem_sel = match self.kind(last) {
                        NodeKind::Expr(ExprKind::PartSelect { left, right, .. }) => {
                            let l =
                                self.packed_relative_bound(*base, self.eval_bound_i128(*left)?)?;
                            let r =
                                self.packed_relative_bound(*base, self.eval_bound_i128(*right)?)?;
                            ElemSel::Part(l, r)
                        }
                        NodeKind::Expr(ExprKind::IndexedPartSelect {
                            base_expr,
                            width_expr,
                            neg,
                            ..
                        }) => {
                            let width = self.indexed_part_select_width(*width_expr, path)?;
                            ElemSel::Indexed(
                                self.lower_packed_index(path, *base, *base_expr)?,
                                width,
                                *neg ^ self.packed_range_ascending(*base),
                            )
                        }
                        _ => ElemSel::Bit(self.lower_packed_index(path, *base, last)?),
                    };
                    return Ok(Lhs::ArrayElem(ArrayElemLhs {
                        arr: ai,
                        indices: ies,
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
                if let Some((info, member, lsb, width)) = self.packed_member_range_info(
                    *base,
                    self.eval_bound_i128(*left)?,
                    self.eval_bound_i128(*right)?,
                )? {
                    return Ok(Lhs::Part(
                        info,
                        i128::from(lsb) + i128::from(width) - 1,
                        i128::from(lsb),
                        member.two_state,
                    ));
                }
                if let Some(mut element) = self.array_element_lhs(path, *base)? {
                    if element.arr.real {
                        return Err(format!(
                            "select on a real array element in `{path}` is not supported"
                        ));
                    }
                    element.elem_sel = ElemSel::Part(
                        self.packed_relative_bound(*base, self.eval_bound_i128(*left)?)?,
                        self.packed_relative_bound(*base, self.eval_bound_i128(*right)?)?,
                    );
                    return Ok(Lhs::ArrayElem(element));
                }
                if let Some((info, member)) = self.packed_member_info(*base) {
                    let left = self.aggregate_member_relative_bound(
                        &member.name,
                        &member.packed_ranges,
                        self.eval_bound_i128(*left)?,
                    )?;
                    let right = self.aggregate_member_relative_bound(
                        &member.name,
                        &member.packed_ranges,
                        self.eval_bound_i128(*right)?,
                    )?;
                    return Ok(Lhs::Part(
                        info,
                        i128::from(member.lsb) + i128::from(left),
                        i128::from(member.lsb) + i128::from(right),
                        member.two_state,
                    ));
                }
                let (_, info) = self.base_signal(path, *base)?;
                if info.real {
                    return Err(format!(
                        "select on real-valued signal in `{path}` is not supported"
                    ));
                }
                let (l, r) = (
                    self.packed_relative_bound(*base, self.eval_bound_i128(*left)?)?,
                    self.packed_relative_bound(*base, self.eval_bound_i128(*right)?)?,
                );
                let two_state = info.two_state;
                Ok(Lhs::Part(info, l, r, two_state))
            }
            NodeKind::Expr(ExprKind::IndexedPartSelect {
                base,
                base_expr,
                width_expr,
                neg,
            }) => {
                if let Some(mut element) = self.array_element_lhs(path, *base)? {
                    if element.arr.real {
                        return Err(format!(
                            "select on a real array element in `{path}` is not supported"
                        ));
                    }
                    let width = self.indexed_part_select_width(*width_expr, path)?;
                    element.elem_sel = ElemSel::Indexed(
                        self.lower_packed_index(path, *base, *base_expr)?,
                        width,
                        *neg ^ self.packed_range_ascending(*base),
                    );
                    return Ok(Lhs::ArrayElem(element));
                }
                let (_, info) = self.base_signal(path, *base)?;
                if info.real {
                    return Err(format!(
                        "select on real-valued signal in `{path}` is not supported"
                    ));
                }
                let width = self.indexed_part_select_width(*width_expr, path)?;
                let ascending = self.packed_range_ascending(*base);
                let base = self.lower_packed_index(path, *base, *base_expr)?;
                let width_expr = self.lower_expr(path, *width_expr)?;
                let two_state = info.two_state;
                Ok(Lhs::IdxPart(
                    info,
                    base,
                    width_expr,
                    width,
                    *neg ^ ascending,
                    two_state,
                ))
            }
            NodeKind::Expr(ExprKind::HierPath { .. }) => {
                if let Some((_target, _kind, member_info)) = self.unpacked_member_info(lhs) {
                    let member = member_info.member;
                    let member_width = member.ty.width.ok_or_else(|| {
                        format!("unpacked member `{}` has unresolved width", member.name)
                    })?;
                    let info = member_info.signal.ok_or_else(|| {
                        format!(
                            "aggregate member `{}` is not a packed assignment target",
                            member.name
                        )
                    })?;
                    let two_state = member.two_state;
                    return Ok(if info.real {
                        Lhs::Whole(info)
                    } else {
                        Lhs::Part(info, i128::from(member_width - 1), 0, two_state)
                    });
                }
                if let Some((info, member)) = self.packed_member_info(lhs) {
                    let two_state = member.two_state;
                    return Ok(Lhs::Part(
                        info,
                        i128::from(member.lsb + member.width - 1),
                        i128::from(member.lsb),
                        two_state,
                    ));
                }
                // A whole-signal hierarchical WRITE (`m.data`, `tb.dut.sig`,
                // …) lowers to the resolved target signal's global, so
                // `llg_ba`/`llg_nba` (and the collapsed inout-net driver
                // path in `assign_statement`) apply unchanged. Selects are
                // represented by their own typed expression nodes and lower
                // through the corresponding arms above.
                let info = self.hier_path_signal(lhs).cloned().ok_or_else(|| {
                    format!(
                        "cannot resolve hierarchical assignment LHS `{}` in \
                             `{path}` (only plain per-instance signals are \
                             supported)",
                        self.node(lhs).name
                    )
                })?;
                Ok(Lhs::Whole(info))
            }
            _ => Err("unsupported assignment LHS".to_string()),
        }
    }

    // ── Constant-ish bound evaluation ──────────────────────────────────────

    /// Evaluate a constant expression node (part-select bound) to an integer.
    pub(super) fn eval_bound_i128(&self, node: NodeId) -> Result<i128, String> {
        match self.eval_bits(node) {
            Ok(v) if !v.is_unknown() => if v.signed {
                v.to_i128()
            } else {
                v.to_u128().and_then(|value| value.try_into().ok())
            }
            .ok_or_else(|| "part_select bound does not fit in i128".to_string()),
            Ok(_) => Err("unknown part_select bound".to_string()),
            Err(e) => Err(format!("part_select bound: {e}")),
        }
    }

    /// Evaluate a constant-ish expression node to a 4-state value, mirroring
    /// `core::elab::Resolver::eval_expr` for the constructs that can appear in
    /// elaborated bound positions.
    pub(super) fn eval_bits(&self, node: NodeId) -> Result<elab::Value, String> {
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
                    Val::Str(value) => string_to_value(&value),
                    Val::Real(_) => Err("non-integer constant in bound".to_string()),
                }
            }
            NodeKind::EnumConst { value } => match value {
                Some(Val::Bits(b)) => Ok(b.clone()),
                _ => Err("enum constant without value in bound".to_string()),
            },
            NodeKind::Expr(ExprKind::Ref { target }) => {
                match target.and_then(|t| self.param_vals.get(&t).map(|value| (t, value))) {
                    Some((_, Val::Bits(b))) => Ok(b.clone()),
                    Some((target, Val::Str(value))) => match self.kind(target) {
                        NodeKind::Param { ty, .. } if ty.kind != "string" => match ty.width {
                            Some(width) => {
                                Ok(string_to_value(value)?.cast(width as usize, ty.signed))
                            }
                            None => Err("non-integer parameter in bound".to_string()),
                        },
                        _ => Err("non-integer parameter in bound".to_string()),
                    },
                    Some((_, Val::Real(_))) => Err("non-integer parameter in bound".to_string()),
                    None => Err("unresolved reference in bound".to_string()),
                }
            }
            NodeKind::Expr(ExprKind::Operation {
                op,
                reordered,
                operands,
                ..
            }) => self.eval_operation_bits(*op, *reordered, operands),
            NodeKind::Expr(ExprKind::Cast { ty, .. })
                if !is_real_kind(&ty.kind) && ty.kind != "string" && ty.kind != "chandle" =>
            {
                match self.eval_decl_value(node)? {
                    Val::Bits(value) => Ok(value),
                    _ => Err("non-integral cast in bound".to_owned()),
                }
            }
            NodeKind::SysCall { name }
                if matches!(
                    name.as_str(),
                    "$countones" | "$onehot" | "$onehot0" | "$isunknown"
                ) =>
            {
                let [arg] = self.node(node).children.as_slice() else {
                    return Err(format!("{name} requires exactly one argument"));
                };
                let arg = self.eval_bits(*arg)?;
                Ok(match name.as_str() {
                    "$countones" => elab::countones(&arg),
                    "$onehot" => elab::onehot(&arg),
                    "$onehot0" => elab::onehot0(&arg),
                    _ => elab::isunknown(&arg),
                })
            }
            other => Err(format!("unsupported bound expression: {other:?}")),
        }
    }

    fn collected_parameter_value(
        &self,
        scope: NodeId,
        parameter: NodeId,
        frontend_value: Option<&Val>,
    ) -> Result<Option<Val>, String> {
        let fallback = || frontend_value.map(materialize_parameter_value);
        let NodeKind::Param { ty, .. } = self.kind(parameter) else {
            return Ok(fallback());
        };
        if self.db.parameter_is_overridden(parameter) {
            return Ok(fallback());
        }
        let parameter_name = self.node(parameter).name.as_str();
        let assignment_rhs = self.node(scope).children.iter().find_map(|child| {
            if !matches!(self.kind(*child), NodeKind::ParamAssign { .. }) {
                return None;
            }
            let [lhs, rhs] = self.node(*child).children.as_slice() else {
                return None;
            };
            (self.node(*lhs).name == parameter_name).then_some(*rhs)
        });
        let assignment_rhs = assignment_rhs.or_else(|| {
            self.node(parameter)
                .children
                .iter()
                .copied()
                .find(|child| matches!(self.kind(*child), NodeKind::Expr(_)))
        });
        let Some(assignment_rhs) = assignment_rhs else {
            return Ok(fallback());
        };
        let integral_cast = matches!(
            self.kind(assignment_rhs),
            NodeKind::Expr(ExprKind::Cast { ty, .. })
                if !is_real_kind(&ty.kind) && ty.kind != "string" && ty.kind != "chandle"
        );
        if frontend_value.is_some()
            && !integral_cast
            && !self.contains_time_literal(assignment_rhs, &mut HashSet::new())
        {
            return Ok(fallback());
        }
        let Ok(value) = self.eval_decl_value(assignment_rhs) else {
            return Ok(fallback());
        };
        if ty.kind == "real" {
            return Ok(Some(Val::Real(match value {
                Val::Bits(value) => value.to_real(),
                Val::Real(value) => value,
                Val::Str(_) => return Ok(fallback()),
            })));
        }
        if ty.kind == "shortreal" {
            let value = match value {
                Val::Bits(value) => value.to_real(),
                Val::Real(value) => value,
                Val::Str(_) => return Ok(fallback()),
            };
            return Ok(Some(Val::Real((value as f32) as f64)));
        }
        let Some(width) = ty.width else {
            return Ok(fallback());
        };
        let value = match value {
            Val::Bits(value) => value,
            Val::Real(value) => elab::real_to_bits(value, width as usize, ty.signed),
            Val::Str(_) => return Ok(fallback()),
        };
        Ok(Some(Val::Bits(materialize_decl_cast_value(
            value,
            width as usize,
            ty.signed,
            self.db.is_two_state_type(parameter) || is_two_state_kind(&ty.kind),
        ))))
    }

    fn contains_time_literal(&self, node: NodeId, visited: &mut HashSet<NodeId>) -> bool {
        if !visited.insert(node) {
            return false;
        }
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Constant { const_type, .. }) => {
                *const_type == ConstantType::Time
            }
            NodeKind::Expr(ExprKind::Operation { operands, .. }) => operands
                .iter()
                .any(|operand| self.contains_time_literal(*operand, visited)),
            NodeKind::Expr(ExprKind::Cast { operand, .. }) => {
                self.contains_time_literal(*operand, visited)
            }
            _ => self
                .node(node)
                .children
                .iter()
                .any(|child| self.contains_time_literal(*child, visited)),
        }
    }

    /// Evaluate the packed/real constants accepted in scalar declaration
    /// initializers.  This stays on the owned database and extends the
    /// integer-only bound evaluator only for conversion system functions.
    pub(super) fn eval_decl_value(&self, node: NodeId) -> Result<Val, String> {
        // Explicit casts are value-materialization boundaries. Handle them
        // before the general integral evaluator, whose Value result retains
        // an unbased fill marker for surrounding expression contexts.
        if !matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::Cast { ty, .. })
                if !is_real_kind(&ty.kind) && ty.kind != "string" && ty.kind != "chandle"
        ) {
            if let Ok(bits) = self.eval_bits(node) {
                return Ok(Val::Bits(bits));
            }
        }
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Constant {
                value,
                size,
                const_type,
                source,
                time_scale,
                ..
            }) => {
                let value = val_from_value_data(value, *size)?;
                if *const_type == ConstantType::Time && self.round_time_literals {
                    let Val::Real(value) = value else {
                        return Err("time literal has no real value".to_owned());
                    };
                    return Ok(Val::Real(self.rounded_time_literal(
                        node,
                        value,
                        source,
                        *time_scale,
                    )?));
                }
                Ok(value)
            }
            NodeKind::Expr(ExprKind::Ref { target }) => target
                .and_then(|target| self.param_vals.get(&target).cloned())
                .ok_or_else(|| "unresolved reference in declaration initializer".to_string()),
            NodeKind::Expr(ExprKind::Operation { op, operands, .. }) => {
                super::validate_operation_arity(*op, operands.len(), "constant expression")?;
                if *op == Operation::MinTypMax {
                    return self.eval_decl_value(operands[1]);
                }
                if *op == Operation::Conditional {
                    let condition = self.eval_bits(operands[0])?;
                    let known = condition
                        .to_u128()
                        .ok_or("unknown condition in constant real expression")?;
                    return self.eval_decl_value(operands[if known == 0 { 2 } else { 1 }]);
                }
                let values = operands
                    .iter()
                    .map(|operand| self.eval_decl_value(*operand))
                    .collect::<Result<Vec<_>, _>>()?;
                if !values.iter().any(|value| matches!(value, Val::Real(_))) {
                    return Err("integral constant operation could not be evaluated".to_owned());
                }
                let real = |value: &Val| match value {
                    Val::Real(value) => Ok(*value),
                    Val::Bits(value) => Ok(value.to_real()),
                    Val::Str(_) => Err("string operand in constant real expression".to_owned()),
                };
                let logical = |value: &Val| match value {
                    Val::Real(value) => {
                        Ok(elab::Value::from_u64(u64::from(*value != 0.0), 1, false))
                    }
                    Val::Bits(value) => Ok(value.clone()),
                    Val::Str(_) => Err("string operand in constant logical expression".to_owned()),
                };
                let value = match *op {
                    Operation::UnaryPlus => Val::Real(real(&values[0])?),
                    Operation::UnaryMinus => Val::Real(-real(&values[0])?),
                    Operation::Add => Val::Real(real(&values[0])? + real(&values[1])?),
                    Operation::Subtract => Val::Real(real(&values[0])? - real(&values[1])?),
                    Operation::Multiply => Val::Real(real(&values[0])? * real(&values[1])?),
                    Operation::Divide => Val::Real(real(&values[0])? / real(&values[1])?),
                    Operation::Modulo => Val::Real(real(&values[0])? % real(&values[1])?),
                    Operation::Power => Val::Real(real(&values[0])?.powf(real(&values[1])?)),
                    Operation::LogicalAnd => {
                        Val::Bits(elab::log_and(&logical(&values[0])?, &logical(&values[1])?))
                    }
                    Operation::LogicalOr => {
                        Val::Bits(elab::log_or(&logical(&values[0])?, &logical(&values[1])?))
                    }
                    Operation::Imply => Val::Bits(elab::log_imply(
                        &logical(&values[0])?,
                        &logical(&values[1])?,
                    )),
                    Operation::LogicalEquivalence => Val::Bits(elab::log_equiv(
                        &logical(&values[0])?,
                        &logical(&values[1])?,
                    )),
                    _ => return Err(format!("unsupported constant real operation {op:?}")),
                };
                match value {
                    Val::Bits(value) => Ok(Val::Bits(value)),
                    Val::Real(value) => value
                        .is_finite()
                        .then_some(Val::Real(value))
                        .ok_or_else(|| "constant real expression is not finite".to_owned()),
                    Val::Str(_) => Err("constant logical expression produced a string".to_owned()),
                }
            }
            NodeKind::Expr(ExprKind::Cast { operand, ty, .. }) if is_real_kind(&ty.kind) => {
                let value = match self.eval_decl_value(*operand)? {
                    Val::Bits(value) => value.to_real(),
                    Val::Real(value) => value,
                    Val::Str(_) => {
                        return Err(
                            "string-to-real cast in declaration initializer is not supported"
                                .to_owned(),
                        )
                    }
                };
                Ok(Val::Real(if ty.kind == "shortreal" {
                    (value as f32) as f64
                } else {
                    value
                }))
            }
            NodeKind::Expr(ExprKind::Cast {
                operand,
                ty,
                size_cast,
                size_cast_expr,
                cast_kind_known,
                two_state,
                propagated,
            }) if !is_real_kind(&ty.kind) && ty.kind != "string" && ty.kind != "chandle" => {
                if !cast_kind_known {
                    return Err("declaration-initializer cast kind cannot be determined".to_owned());
                }
                let width = size_cast_expr
                    .as_deref()
                    .and_then(|expression| self.source_size_cast_width(expression))
                    .or(ty.width)
                    .ok_or("integral declaration-initializer cast has no width")?;
                if width > LLG_MAX_WIDTH {
                    return Err(format!(
                        "declaration-initializer cast is {width} bits wide; maximum supported width is {LLG_MAX_WIDTH}"
                    ));
                }
                let cast_bits = |mut value: elab::Value| {
                    if *propagated {
                        value.signed = ty.signed;
                    }
                    let signed = if *size_cast { value.signed } else { ty.signed };
                    Val::Bits(materialize_decl_cast_value(
                        value,
                        width as usize,
                        signed,
                        *two_state || is_two_state_kind(&ty.kind),
                    ))
                };
                match self.eval_decl_value(*operand)? {
                    Val::Bits(value) => Ok(cast_bits(value)),
                    Val::Str(value) => Ok(cast_bits(string_to_value(&value)?)),
                    Val::Real(value) => Ok(cast_bits(elab::real_to_bits(
                        value,
                        width as usize,
                        if *size_cast { false } else { ty.signed },
                    ))),
                }
            }
            NodeKind::SysCall { name }
                if matches!(
                    name.as_str(),
                    "$rtoi"
                        | "$itor"
                        | "$realtobits"
                        | "$bitstoreal"
                        | "$shortrealtobits"
                        | "$bitstoshortreal"
                ) =>
            {
                let [arg] = self.node(node).children.as_slice() else {
                    return Err(format!("{name} requires exactly one argument"));
                };
                let arg = self.eval_decl_value(*arg)?;
                match (name.as_str(), arg) {
                    ("$rtoi", Val::Real(value)) => Ok(Val::Bits(elab::rtoi_value(value))),
                    ("$rtoi", Val::Bits(value)) => Ok(Val::Bits(elab::rtoi_value(value.to_real()))),
                    ("$itor", Val::Bits(value)) => Ok(Val::Real(value.to_real())),
                    ("$itor", Val::Real(value)) => Ok(Val::Real(value)),
                    ("$realtobits", Val::Real(value)) => {
                        Ok(Val::Bits(elab::real_to_ieee_bits(value)))
                    }
                    ("$realtobits", Val::Bits(value)) => {
                        Ok(Val::Bits(elab::real_to_ieee_bits(value.to_real())))
                    }
                    ("$bitstoreal", Val::Bits(value)) if value.width() == 64 => Ok(Val::Real(
                        elab::ieee_bits_to_real(&value)
                            .ok_or_else(|| "invalid $bitstoreal width".to_string())?,
                    )),
                    ("$shortrealtobits", Val::Real(value)) => {
                        Ok(Val::Bits(elab::shortreal_to_ieee_bits(value)))
                    }
                    ("$shortrealtobits", Val::Bits(value)) => {
                        Ok(Val::Bits(elab::shortreal_to_ieee_bits(value.to_real())))
                    }
                    ("$bitstoshortreal", Val::Bits(value)) if value.width() == 32 => Ok(Val::Real(
                        elab::ieee_bits_to_shortreal(&value)
                            .ok_or_else(|| "invalid $bitstoshortreal width".to_string())?,
                    )),
                    _ => Err(format!(
                        "invalid argument to {name} in declaration initializer"
                    )),
                }
            }
            other => Err(format!(
                "unsupported declaration initializer expression: {other:?}"
            )),
        }
    }

    fn eval_operation_bits(
        &self,
        op: Operation,
        reordered: bool,
        operands: &[NodeId],
    ) -> Result<elab::Value, String> {
        super::validate_operation_arity(op, operands.len(), "constant expression")?;
        let u = |i: usize| self.eval_bits(operands[i]);
        macro_rules! b {
            ($i:expr) => {
                u($i)?
            };
        }
        match op {
            Operation::UnaryMinus => Ok(elab::minus(&b!(0))),
            Operation::UnaryPlus => Ok(b!(0)),
            Operation::LogicalNot => Ok(elab::log_not(&b!(0))),
            Operation::BitwiseNot => Ok(elab::bit_neg(&b!(0))),
            Operation::ReductionAnd => Ok(elab::unary_and(&b!(0))),
            Operation::ReductionNand => Ok(elab::unary_nand(&b!(0))),
            Operation::ReductionOr => Ok(elab::unary_or(&b!(0))),
            Operation::ReductionNor => Ok(elab::unary_nor(&b!(0))),
            Operation::ReductionXor => Ok(elab::unary_xor(&b!(0))),
            Operation::ReductionXnor => Ok(elab::unary_xnor(&b!(0))),
            Operation::Subtract => Ok(elab::sub(&b!(0), &b!(1))),
            Operation::Divide => Ok(elab::div(&b!(0), &b!(1))),
            Operation::Modulo => Ok(elab::rem(&b!(0), &b!(1))),
            Operation::Equal => Ok(elab::eq(&b!(0), &b!(1))),
            Operation::NotEqual => Ok(elab::neq(&b!(0), &b!(1))),
            Operation::CaseEqual => Ok(elab::case_eq(&b!(0), &b!(1))),
            Operation::CaseNotEqual => Ok(elab::case_neq(&b!(0), &b!(1))),
            Operation::WildEqual => Ok(elab::wildcard_eq(&b!(0), &b!(1))),
            Operation::WildNotEqual => Ok(elab::wildcard_neq(&b!(0), &b!(1))),
            Operation::Greater => Ok(elab::gt(&b!(0), &b!(1))),
            Operation::GreaterEqual => Ok(elab::ge(&b!(0), &b!(1))),
            Operation::Less => Ok(elab::lt(&b!(0), &b!(1))),
            Operation::LessEqual => Ok(elab::le(&b!(0), &b!(1))),
            Operation::ShiftLeft => Ok(elab::shl(&b!(0), &b!(1))),
            Operation::ShiftRight => Ok(elab::shr(&b!(0), &b!(1))),
            Operation::ArithmeticShiftLeft => Ok(elab::arith_shl(&b!(0), &b!(1))),
            Operation::ArithmeticShiftRight => Ok(elab::arith_shr(&b!(0), &b!(1))),
            Operation::Add => Ok(elab::add(&b!(0), &b!(1))),
            Operation::Multiply => Ok(elab::mul(&b!(0), &b!(1))),
            Operation::Power => Ok(elab::power(&b!(0), &b!(1))),
            Operation::LogicalAnd => Ok(elab::log_and(&b!(0), &b!(1))),
            Operation::LogicalOr => Ok(elab::log_or(&b!(0), &b!(1))),
            Operation::Imply => {
                let left = b!(0);
                // Match Slang's short-circuit constant evaluation: a known
                // false antecedent determines the result without touching
                // the consequent.
                if left.to_u128() == Some(0) {
                    Ok(elab::Value::from_u64(1, 1, false))
                } else {
                    let right = b!(1);
                    Ok(elab::log_imply(&left, &right))
                }
            }
            Operation::LogicalEquivalence => Ok(elab::log_equiv(&b!(0), &b!(1))),
            Operation::BitwiseAnd => Ok(elab::bit_and(&b!(0), &b!(1))),
            Operation::BitwiseOr => Ok(elab::bit_or(&b!(0), &b!(1))),
            Operation::BitwiseXor => Ok(elab::bit_xor(&b!(0), &b!(1))),
            Operation::BitwiseXnor => Ok(elab::bit_xnor(&b!(0), &b!(1))),
            Operation::Conditional => Ok(elab::cond(&b!(0), &b!(1), &b!(2))),
            Operation::MinTypMax => Ok(b!(0)),
            Operation::Concat => {
                let mut parts = Vec::with_capacity(operands.len());
                for i in 0..operands.len() {
                    parts.push(b!(i));
                }
                if reordered {
                    parts.reverse();
                }
                Ok(elab::concat(&parts))
            }
            Operation::MultiConcat => {
                let count = b!(0);
                if count.is_unknown() {
                    return Err("unknown replication count".to_string());
                }
                let n: usize = count
                    .to_u128()
                    .and_then(|value| value.try_into().ok())
                    .ok_or_else(|| "replication count does not fit in usize".to_string())?;
                let mut parts = Vec::with_capacity(operands.len().saturating_sub(1));
                for i in 1..operands.len() {
                    parts.push(b!(i));
                }
                let pat = elab::concat(&parts);
                let total_width = pat
                    .width()
                    .checked_mul(n)
                    .ok_or_else(|| "replication width overflow".to_string())?;
                if total_width > LLG_MAX_WIDTH as usize {
                    return Err(format!(
                        "replication result is too wide ({total_width} bits; max {LLG_MAX_WIDTH})"
                    ));
                }
                let mut bits = Vec::with_capacity(total_width);
                for _ in 0..n {
                    bits.extend(pat.bits.iter().cloned());
                }
                Ok(elab::Value::from_bits(bits, false))
            }
            other => Err(format!("unsupported operation op type {other:?} in bound")),
        }
    }
}

fn materialize_decl_cast_value(
    value: elab::Value,
    width: usize,
    signed: bool,
    two_state: bool,
) -> elab::Value {
    let mut value = value.cast(width, signed);
    // A cast returns the value held by a temporary of the target type (IEEE
    // 1800-2009 §6.24.1). Its target width has therefore materialized an
    // unbased unsized fill; do not let an enclosing declaration assignment
    // refill to a second, wider destination.
    value.fill = None;
    if two_state {
        for bit in &mut value.bits {
            if matches!(*bit, Bit::X | Bit::Z) {
                *bit = Bit::Zero;
            }
        }
    }
    value
}

fn aggregate_member_matches_type_key(
    member: &AggregateMember,
    _key: &str,
    key_type: Option<&AssignmentPatternKeyType>,
) -> bool {
    let Some(key_type) = key_type else {
        return false;
    };
    pattern_key_matches_descriptor(
        key_type,
        &member.descriptor,
        member.two_state,
        Some(&member.packed_ranges),
    )
}

/// Match a resolved assignment-pattern type key against a complete recursive
/// descriptor. Frontend type identity is authoritative; display names and
/// structural similarity cannot make two nominal types compatible.
pub(super) fn pattern_key_matches_descriptor(
    key_type: &AssignmentPatternKeyType,
    descriptor: &TypeDescriptor,
    two_state: bool,
    packed_ranges: Option<&[crate::core::db::PackedRange]>,
) -> bool {
    if key_type.type_id != descriptor.id {
        return false;
    }
    let nominal = |kind: &str| matches!(kind, "struct" | "union" | "enum" | "class");
    if nominal(&key_type.ty.kind) || nominal(&descriptor.info.kind) {
        return key_type.ty.kind == descriptor.info.kind && key_type.two_state == two_state;
    }
    if key_type.ty.kind == "array" || descriptor.info.kind == "array" {
        return key_type.ty.kind == descriptor.info.kind;
    }
    if key_type.ty.kind == "real" || descriptor.info.kind == "real" {
        return key_type.ty.kind == descriptor.info.kind;
    }
    if key_type.ty.kind == "string" || descriptor.info.kind == "string" {
        return key_type.ty.kind == descriptor.info.kind;
    }
    if !is_integral_pattern_key_kind(&key_type.ty.kind)
        || !is_integral_pattern_key_kind(&descriptor.info.kind)
        || key_type.ty.signed != descriptor.info.signed
        || key_type.two_state != two_state
    {
        return false;
    }
    let descriptor_ranges = match &descriptor.shape {
        TypeShape::PackedAtom { ranges } => ranges.as_slice(),
        _ => &[],
    };
    let effective_ranges = |ranges: &[crate::core::db::PackedRange], width: Option<u32>| {
        if !ranges.is_empty() {
            return ranges.to_vec();
        }
        width
            .and_then(|width| width.checked_sub(1))
            .map(|left| {
                vec![crate::core::db::PackedRange {
                    left: i128::from(left),
                    right: 0,
                }]
            })
            .unwrap_or_default()
    };
    effective_ranges(&key_type.packed_ranges, key_type.ty.width)
        == effective_ranges(
            packed_ranges.unwrap_or(descriptor_ranges),
            descriptor.info.width,
        )
}

pub(super) fn pattern_key_types_equal(
    left: &AssignmentPatternKeyType,
    right: &AssignmentPatternKeyType,
) -> bool {
    left.type_id == right.type_id
        && left.two_state == right.two_state
        && left.ty.kind == right.ty.kind
        && left.ty.width == right.ty.width
        && left.ty.signed == right.ty.signed
        && left.packed_ranges == right.packed_ranges
}

pub(super) fn pattern_key_matches_type_descriptor(
    key_type: &AssignmentPatternKeyType,
    descriptor: &TypeDescriptor,
    two_state: bool,
) -> bool {
    pattern_key_matches_descriptor(key_type, descriptor, two_state, None)
}

fn is_integral_pattern_key_kind(kind: &str) -> bool {
    matches!(
        kind,
        "bit" | "logic" | "reg" | "byte" | "shortint" | "int" | "longint" | "integer" | "time"
    )
}

fn materialize_parameter_value(value: &Val) -> Val {
    match value {
        Val::Bits(value) => {
            let mut value = value.clone();
            // A parameter reference denotes its declared/inferred finite
            // value (IEEE 1800-2009 §6.20.2). A frontend can retain the
            // initializer's unbased fill marker after it has already resized
            // the payload, so preserve the elaborated width/bits/signedness
            // but clear that stale contextual marker.
            value.fill = None;
            Val::Bits(value)
        }
        Val::Str(value) => Val::Str(value.clone()),
        Val::Real(value) => Val::Real(*value),
    }
}

/// Convert semantic drive strengths to the ordered IEEE 1800-2009
/// Table 28-7 scale used by the generated runtime. Charge strengths are valid
/// for trireg storage, not continuous-assignment drive strengths.
fn continuous_assignment_strengths(
    strength0: Strength,
    strength1: Strength,
    net_name: &str,
) -> Result<(u8, u8), String> {
    fn level(strength: Strength, net_name: &str) -> Result<u8, String> {
        match strength {
            Strength::Unspecified | Strength::Strong => Ok(6),
            Strength::Supply => Ok(7),
            Strength::Pull => Ok(5),
            Strength::Weak => Ok(3),
            Strength::HighZ => Ok(0),
            Strength::Large | Strength::Medium | Strength::Small => Err(format!(
                "charge strength on continuous assignment to net `{net_name}` is not supported"
            )),
            Strength::Unsupported => Err(format!(
                "unsupported drive strength on continuous assignment to net `{net_name}`"
            )),
        }
    }

    let zero = level(strength0, net_name)?;
    let one = level(strength1, net_name)?;
    if zero == 0 && one == 0 {
        return Err(format!(
            "continuous assignment to net `{net_name}` specifies high impedance for both logic values"
        ));
    }
    Ok((zero, one))
}

/// Apply the language restriction that an explicit continuous-assignment
/// strength belongs only to a scalar net.  The resolved runtime still carries
/// one strength pair per structural driver, so scalar wired and collapsed
/// groups use the same path as ordinary wires.
fn continuous_assignment_strengths_for_width(
    strength0: Strength,
    strength1: Strength,
    net_name: &str,
    width: u32,
) -> Result<(u8, u8), String> {
    if (strength0 != Strength::Unspecified || strength1 != Strength::Unspecified) && width != 1 {
        return Err(format!(
            "drive strength on non-scalar net `{net_name}` is not permitted by IEEE 1800-2009 10.3.4"
        ));
    }
    continuous_assignment_strengths(strength0, strength1, net_name)
}

/// Return the effective drive pair for a primitive output.  Ordinary gate
/// outputs default to strong/strong, while pullup/pulldown primitives are
/// pull-strength sources.  Explicit gate strengths retain their asymmetric
/// endpoints and flow into the canonical structural-driver slot.
fn gate_driver_strengths(
    prim_type: PrimitiveType,
    strength0: Strength,
    strength1: Strength,
    net_name: &str,
) -> Result<(u8, u8), String> {
    let (strength0, strength1) =
        if strength0 == Strength::Unspecified && strength1 == Strength::Unspecified {
            match prim_type {
                PrimitiveType::Pullup => (Strength::HighZ, Strength::Pull),
                PrimitiveType::Pulldown => (Strength::Pull, Strength::HighZ),
                _ => (Strength::Strong, Strength::Strong),
            }
        } else {
            (strength0, strength1)
        };
    continuous_assignment_strengths(strength0, strength1, net_name)
}

/// Return the effective drive pair for an output port. Port strengths use the
/// same endpoint legality as continuous drivers; an omitted declaration is
/// the ordinary strong/strong source. Keeping this conversion beside the
/// gate/continuous helpers gives collapsed groups one resolver contract for
/// every structural source.
fn port_driver_strengths(
    strength0: Strength,
    strength1: Strength,
    net_name: &str,
) -> Result<(u8, u8), String> {
    continuous_assignment_strengths(strength0, strength1, net_name)
}

#[cfg(test)]
mod tests {
    use super::{materialize_decl_cast_value, materialize_parameter_value};
    use crate::core::elab::{Bit, Val, Value};

    #[test]
    fn declaration_cast_materializes_fill_before_outer_assignment() {
        let mut fill = Value::from_bits(vec![Bit::One], false);
        fill.fill = Some(Bit::One);

        let cast = materialize_decl_cast_value(fill, 1, false, false);
        assert_eq!(cast.fill, None);
        assert_eq!(cast.cast(8, false).to_u128(), Some(1));
    }

    #[test]
    fn parameter_value_drops_initializer_fill_after_declared_resize() {
        let mut elaborated = Value::from_u64(1, 8, false);
        elaborated.fill = Some(Bit::One);

        let Val::Bits(materialized) = materialize_parameter_value(&Val::Bits(elaborated)) else {
            panic!("packed parameter must remain packed");
        };
        assert_eq!(materialized.width(), 8);
        assert!(!materialized.signed);
        assert_eq!(materialized.fill, None);
        assert_eq!(materialized.to_u128(), Some(1));
    }
}
