//! Virtual interfaces.

use super::*;
use crate::sim::codegen::error::located;
use crate::IdSet;

impl<'a> Codegen<'a> {
    /// Return the leaf virtual-interface spelling of a declaration. Fixed
    /// unpacked arrays retain their outer descriptor, so walk the owned type
    /// shape instead of parsing an array suffix from a display name.
    pub(in super::super) fn virtual_interface_spelling(&self, node: NodeId) -> Option<String> {
        fn leaf(descriptor: &TypeDescriptor) -> Option<String> {
            match &descriptor.shape {
                TypeShape::FixedArray { element, .. } | TypeShape::Container { element, .. } => {
                    leaf(element)
                }
                TypeShape::Opaque { kind } if kind == "VirtualInterface" => descriptor
                    .info
                    .type_name
                    .clone()
                    .or_else(|| Some(descriptor.name.clone())),
                _ => None,
            }
        }
        if let Some(descriptor) = self.db.type_descriptor(node) {
            return leaf(descriptor);
        }
        let ty = match self.kind(node) {
            NodeKind::Var { ty } | NodeKind::Array { ty } | NodeKind::FuncArg { ty, .. } => ty,
            NodeKind::Expr(ExprKind::Cast { ty, .. }) => ty,
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => return self.virtual_interface_spelling(*target),
            NodeKind::Expr(ExprKind::ArraySelect { base, .. }) => {
                return self.virtual_interface_spelling(*base)
            }
            _ => return None,
        };
        (ty.kind == "virtual_interface")
            .then(|| ty.type_name.clone())
            .flatten()
    }

    pub(super) fn is_virtual_interface_array(&self, node: NodeId) -> bool {
        matches!(self.kind(node), NodeKind::Array { .. })
            && self.virtual_interface_spelling(node).is_some()
    }

    /// A fixed unpacked array whose element is an identity handle stored by
    /// the container runtime (see `is_fixed_handle_element`).
    pub(super) fn is_fixed_handle_array(&self, node: NodeId) -> bool {
        matches!(self.kind(node), NodeKind::Array { .. })
            && self
                .db
                .array_meta(node)
                .is_some_and(|meta| matches!(meta.kind(), ArrayKind::Static))
            && self.db.type_descriptor(node).is_some_and(|descriptor| {
                matches!(&descriptor.shape, TypeShape::FixedArray { element, .. }
                    if Self::is_fixed_handle_element(element))
            })
    }

    /// Remove a modport view from a virtual-interface type spelling. Dots in
    /// parameter expressions are ignored while scanning parenthesis depth.
    pub(in super::super) fn virtual_interface_identity_from_spelling(spelling: &str) -> String {
        let mut depth = 0usize;
        let mut last_dot = None;
        for (index, byte) in spelling.as_bytes().iter().copied().enumerate() {
            match byte {
                b'(' => depth += 1,
                b')' => depth = depth.saturating_sub(1),
                b'.' if depth == 0 => last_dot = Some(index),
                _ => {}
            }
        }
        let base = last_dot.map_or(spelling, |index| &spelling[..index]);
        base.split('$').next().unwrap_or(base).to_owned()
    }

    /// Return the optional modport suffix from a virtual-interface type
    /// spelling. Dots inside parameter expressions are ignored.
    pub(in super::super) fn virtual_interface_view_from_spelling(spelling: &str) -> Option<String> {
        let mut depth = 0usize;
        let mut last_dot = None;
        for (index, byte) in spelling.as_bytes().iter().copied().enumerate() {
            match byte {
                b'(' => depth += 1,
                b')' => depth = depth.saturating_sub(1),
                b'.' if depth == 0 => last_dot = Some(index),
                _ => {}
            }
        }
        last_dot
            .and_then(|index| spelling.get(index + 1..))
            .filter(|view| !view.is_empty())
            .map(ToOwned::to_owned)
    }

    fn virtual_interface_source_spelling(&self, node: NodeId) -> Option<String> {
        if matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::Constant {
                const_type: ConstantType::Null,
                ..
            })
        ) {
            return None;
        }
        if let NodeKind::Expr(ExprKind::ScopeRef { target }) = self.kind(node) {
            if let Some(identity) = self.concrete_virtual_interface_identity(*target) {
                return Some(identity);
            }
            if matches!(self.kind(*target), NodeKind::ModPort) {
                return self
                    .node(*target)
                    .parent()
                    .and_then(|parent| self.concrete_virtual_interface_identity(parent));
            }
        }
        self.virtual_interface_spelling(node)
    }

    /// Check the nominal specialization and modport-view compatibility at a
    /// virtual-interface assignment boundary. A bare interface view may bind
    /// any compatible modport, while two distinct explicit modport views are
    /// not assignment-compatible.
    pub(in super::super) fn validate_virtual_interface_assignment(
        &self,
        lhs: NodeId,
        rhs: NodeId,
        path: &str,
    ) -> Result<(), String> {
        let Some(destination) = self.virtual_interface_spelling(lhs) else {
            return Ok(());
        };
        let Some(source) = self.virtual_interface_source_spelling(rhs) else {
            return Ok(());
        };
        let destination_identity = Self::normalize_virtual_interface_identity(
            &Self::virtual_interface_identity_from_spelling(&destination),
        );
        let source_identity = Self::normalize_virtual_interface_identity(
            &Self::virtual_interface_identity_from_spelling(&source),
        );
        if destination_identity != source_identity {
            return Err(format!(
                "virtual interface assignment in `{path}` has incompatible parameter specialization: `{destination}` <- `{source}`"
            ));
        }
        if let Some(destination_view) = Self::virtual_interface_view_from_spelling(&destination) {
            if let Some(source_view) = Self::virtual_interface_view_from_spelling(&source) {
                if destination_view != source_view {
                    return Err(format!(
                        "virtual interface assignment in `{path}` has incompatible modport views: `{destination_view}` <- `{source_view}`"
                    ));
                }
            }
        }
        Ok(())
    }

    pub(in super::super) fn normalize_virtual_interface_identity(identity: &str) -> String {
        identity.chars().filter(|ch| !ch.is_whitespace()).collect()
    }

    fn concrete_virtual_interface_identity(&self, node: NodeId) -> Option<String> {
        let NodeKind::ModuleInst {
            def_name,
            is_interface: true,
            ..
        } = self.kind(node)
        else {
            return None;
        };
        let params = self
            .node(node)
            .children
            .iter()
            .filter_map(|child| {
                if !matches!(self.kind(*child), NodeKind::Param { .. }) {
                    return None;
                }
                let value = match self.parameter_value(*child)?.as_ref() {
                    Val::Bits(bits) => bits
                        .to_i128()
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| bits.format_verilog()),
                    Val::Real(value) => value.to_string(),
                    Val::Str(value) => format!("\"{value}\""),
                };
                Some(format!("{}={value}", self.node(*child).name))
            })
            .collect::<Vec<_>>();
        let identity = if params.is_empty() {
            strip_lib(def_name)
        } else {
            format!("{}#({})", strip_lib(def_name), params.join(","))
        };
        Some(Self::normalize_virtual_interface_identity(&identity))
    }

    /// Collect concrete signal and method entries for one interface instance.
    /// Clocking variables are exposed under `clocking.member` and point at the
    /// sampled storage allocated by `collect_clocking_storage`.
    fn concrete_virtual_interface_entries(
        &self,
        interface: NodeId,
    ) -> (VirtualInterfaceMemberEntries, VirtualInterfaceMethodEntries) {
        let mut members = Vec::new();
        let mut methods = Vec::new();
        for child in self.node(interface).children.iter().copied() {
            match self.kind(child) {
                NodeKind::Var { .. } | NodeKind::Net { .. } => {
                    if self.db.is_clocking_var(child) {
                        continue;
                    }
                    if let Some(signal) = self.signal_of(child).cloned() {
                        members.push((self.node(child).name.clone(), signal));
                    }
                }
                NodeKind::FuncTask { body: Some(_), .. } => {
                    if let Some(meta) = self.func_meta.get(&child) {
                        methods.push((self.node(child).name.clone(), meta.ir));
                    }
                }
                NodeKind::Stmt(StmtKind::Begin) if self.db.is_clocking_block(child) => {
                    for variable in self.node(child).children.iter().copied() {
                        let Some(sample) = self.clocking_samples.get(&variable) else {
                            continue;
                        };
                        members.push((
                            format!("{}.{}", self.node(child).name, self.node(variable).name),
                            sample.sample.clone(),
                        ));
                    }
                }
                _ => {}
            }
        }
        (members, methods)
    }

    /// Capture member types from an interface definition. Definitions are
    /// retained in the owned database even when a design only declares a
    /// null virtual-interface handle; their metadata is still needed to lower
    /// the access so the runtime can report the null dereference at execution.
    fn virtual_interface_template_entries(
        &self,
        interface: NodeId,
    ) -> (VirtualInterfaceMemberEntries, Vec<String>) {
        fn template_signal(id: NodeId, ty: &crate::core::model::TypeInfo) -> SignalInfo {
            SignalInfo {
                global: String::new(),
                width: ty.width.unwrap_or(0),
                signed: ty.signed,
                two_state: is_two_state_kind(&ty.kind),
                real: is_real_kind(&ty.kind),
                shortreal: ty.kind == "shortreal",
                net_driver: None,
                // A template has no emitted storage. This slot is never used
                // to build an instance entry; concrete entries replace it.
                ir: id.index(),
            }
        }

        let mut members = Vec::new();
        let mut methods = Vec::new();
        for child in self.node(interface).children.iter().copied() {
            match self.kind(child) {
                NodeKind::Var { ty } | NodeKind::Net { ty, .. } => {
                    if !self.db.is_clocking_var(child) {
                        members.push((self.node(child).name.clone(), template_signal(child, ty)));
                    }
                }
                NodeKind::FuncTask { .. } => methods.push(self.node(child).name.clone()),
                NodeKind::Stmt(StmtKind::Begin) if self.db.is_clocking_block(child) => {
                    for variable in self.node(child).children.iter().copied() {
                        let ty = match self.kind(variable) {
                            NodeKind::Var { ty } | NodeKind::Net { ty, .. } => ty,
                            _ => continue,
                        };
                        members.push((
                            format!("{}.{}", self.node(child).name, self.node(variable).name),
                            template_signal(variable, ty),
                        ));
                    }
                }
                _ => {}
            }
        }
        (members, methods)
    }

    /// Build one owned descriptor for each elaborated virtual-interface type.
    /// All subsequent lowerings use these tables; no native Slang path is
    /// consulted when a handle is rebound at runtime.
    pub(in super::super) fn collect_virtual_interfaces(&mut self) -> Result<(), String> {
        let mut identities = BTreeSet::new();
        for node in self.db.node_ids() {
            let Some(spelling) = self.virtual_interface_spelling(node) else {
                continue;
            };
            identities.insert(Self::normalize_virtual_interface_identity(
                &Self::virtual_interface_identity_from_spelling(&spelling),
            ));
        }
        if identities.is_empty() {
            return Ok(());
        }
        let interfaces = self
            .design_nodes()
            .iter()
            .copied()
            .filter(|node| {
                matches!(
                    self.kind(*node),
                    NodeKind::ModuleInst {
                        is_interface: true,
                        ..
                    }
                )
            })
            .collect::<Vec<_>>();

        self.validate_virtual_interface_input_port_writes(&interfaces)?;
        for identity in identities {
            self.add_virtual_interface_descriptor(identity, &interfaces)?;
        }
        self.collect_virtual_interface_nesting(&interfaces)?;
        Ok(())
    }

    /// Add a nested slot for each nested interface instance a member path
    /// selects through a handle (`v.tx`, `v.tx.x`, `v.a.l.x`; SV 25.9), with
    /// a descriptor for each nested type that has none yet. Paths are
    /// scanned once, in node order, so slots and added descriptors follow
    /// the source. A step that no bound instance resolves adds nothing; its
    /// lowering reports it.
    fn collect_virtual_interface_nesting(&mut self, interfaces: &[NodeId]) -> Result<(), String> {
        let mut paths = Vec::new();
        for node in self.db.node_ids() {
            if let NodeKind::MethodCall {
                receiver: Some(receiver),
                ..
            } = self.kind(node)
            {
                let steps = self.db.virtual_interface_call_scopes(node);
                if let (false, Some(spelling)) =
                    (steps.is_empty(), self.virtual_interface_spelling(*receiver))
                {
                    paths.push((spelling, steps.to_vec(), None));
                }
                continue;
            }
            let NodeKind::Expr(ExprKind::HierPath { parts, refs }) = self.kind(node) else {
                continue;
            };
            let Some((position, spelling)) =
                refs.iter().enumerate().find_map(|(position, target)| {
                    self.virtual_interface_spelling((*target)?)
                        .map(|spelling| (position, spelling))
                })
            else {
                continue;
            };
            let steps = self.virtual_interface_nested_steps(parts, refs, position);
            if !steps.is_empty() {
                // A path that ends at a nested instance is a value of its
                // virtual-interface type.
                let value_type = (position + 1 + steps.len() == parts.len())
                    .then(|| self.virtual_interface_spelling(node))
                    .flatten();
                paths.push((spelling, steps, value_type));
            }
        }
        for (spelling, steps, value_type) in paths {
            let identity = Self::normalize_virtual_interface_identity(
                &Self::virtual_interface_identity_from_spelling(&spelling),
            );
            let Some(mut descriptor) = self.virtual_interface_types.get(&identity).copied() else {
                continue;
            };
            let last = steps.len() - 1;
            for (step, name) in steps.iter().enumerate() {
                let value_type = value_type.as_deref().filter(|_| step == last);
                match self.add_virtual_interface_nested(descriptor, name, value_type, interfaces)? {
                    Some(child) => descriptor = child,
                    None => break,
                }
            }
        }
        Ok(())
    }

    /// The names of the nested interface instances a member path selects
    /// after its handle at `position`: the components the frontend binds to
    /// an instance (`tx` in `v.tx.x`, `ls[1]` in `v.ls[1]`).
    pub(in super::super) fn virtual_interface_nested_steps(
        &self,
        parts: &[String],
        refs: &[Option<NodeId>],
        position: usize,
    ) -> Vec<String> {
        parts
            .iter()
            .zip(refs)
            .skip(position + 1)
            .take_while(|(_, target)| {
                target
                    .is_some_and(|target| matches!(self.kind(target), NodeKind::ModuleInst { .. }))
            })
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// The generate block a member path names after its handle at
    /// `position` (`g` in `v.g.l`): its scopes are not reached through a
    /// handle, which binds the interface's own members and the interface
    /// instances nested in it.
    pub(in super::super) fn virtual_interface_generate_scope<'p>(
        &self,
        parts: &'p [String],
        refs: &[Option<NodeId>],
        position: usize,
    ) -> Option<&'p str> {
        parts
            .iter()
            .zip(refs)
            .skip(position + 1)
            .find(|(_, target)| {
                target.is_some_and(|target| {
                    matches!(
                        self.kind(target),
                        NodeKind::GenScope | NodeKind::GenScopeArray
                    )
                })
            })
            .map(|(name, _)| name.as_str())
    }

    /// The interface instance named `name` declared directly in interface
    /// instance `parent`, or as an element of an instance array there.
    fn nested_interface_instance(&self, parent: NodeId, name: &str) -> Option<NodeId> {
        let mut pending = self.node(parent).children.clone();
        while let Some(child) = pending.pop() {
            match self.kind(child) {
                NodeKind::ModuleInst {
                    is_interface: true, ..
                } if self.node(child).name == name => return Some(child),
                NodeKind::InstanceArray => pending.extend(self.node(child).children.iter()),
                _ => {}
            }
        }
        None
    }

    /// The variables of design interface instances that a write through a
    /// virtual-interface handle can store to: for every assignment target,
    /// output or inout actual and non-`const` `ref` actual in the owned
    /// database that names a member through a handle (`v.x`, `v.x[i]`,
    /// `{v.x, y}`, `v.tx.x`, a clocking drive `v.cb.x <= ...`), the member
    /// variable of each instance of the type the handle (or its last nested
    /// instance) selects or, for a clockvar, each variable its clocking
    /// expression names. Port collapse keeps the link of such a sink
    /// (`port_aliases.rs`). It runs before the descriptors exist, so it
    /// groups the instances by the type identity the descriptors use; a
    /// path the descriptors cannot resolve fails its own lowering.
    pub(super) fn virtual_interface_written_variables(&self, nodes: &[NodeId]) -> HashSet<NodeId> {
        let paths = self.virtual_interface_write_paths();
        let mut written = HashSet::new();
        if paths.is_empty() {
            return written;
        }
        let mut instances: HashMap<String, Vec<NodeId>> = HashMap::new();
        for node in nodes {
            if let Some(identity) = self.concrete_virtual_interface_identity(*node) {
                instances.entry(identity).or_default().push(*node);
            }
        }
        let mut seen = HashSet::new();
        for &path in paths {
            let NodeKind::Expr(ExprKind::HierPath { parts, refs }) = self.kind(path) else {
                continue;
            };
            let Some((position, spelling)) =
                refs.iter().enumerate().find_map(|(position, target)| {
                    self.virtual_interface_spelling((*target)?)
                        .map(|spelling| (position, spelling))
                })
            else {
                continue;
            };
            if self
                .virtual_interface_generate_scope(parts, refs, position)
                .is_some()
            {
                continue;
            }
            let steps = self.virtual_interface_nested_steps(parts, refs, position);
            let member =
                self.virtual_interface_member_name(path, parts, refs, position + steps.len());
            if member.is_empty() {
                continue;
            }
            let identity = Self::normalize_virtual_interface_identity(
                &Self::virtual_interface_identity_from_spelling(&spelling),
            );
            if !seen.insert((identity.clone(), steps.clone(), member.clone())) {
                continue;
            }
            let Some(bound) = self.nested_identity(&identity, &steps, &instances) else {
                continue;
            };
            for interface in instances.get(&bound).into_iter().flatten() {
                written.extend(self.interface_member_variables(*interface, &member));
            }
        }
        written
    }

    /// The member paths through a virtual-interface handle that an
    /// assignment target, output or inout actual or non-`const` `ref` actual
    /// in the owned database writes ([`Self::written_targets`],
    /// [`Self::handle_member_writes`]), in node order. Code of an untaken
    /// generate block writes nothing: it does not exist (27.3). Built on
    /// first use.
    fn virtual_interface_write_paths(&self) -> &[NodeId] {
        self.virtual_interface_write_paths.get_or_init(|| {
            let mut paths = Vec::new();
            let mut targets = Vec::new();
            for id in self.db.instantiated_node_ids() {
                targets.clear();
                self.written_targets(id, &mut targets);
                for target in &targets {
                    self.handle_member_writes(*target, &mut paths);
                }
            }
            paths
        })
    }

    /// "Assignments to variables declared as input ports shall be illegal"
    /// (IEEE 1800-2009 23.3.3.2), also through a virtual-interface handle.
    /// The handle's interface type decides, not the instance it holds at run
    /// time: every instance of a type declares the same ports, so a write
    /// (procedural or nonblocking assignment, output, inout or `ref` actual)
    /// whose member is an input port variable of the type of the handle (or
    /// of its last nested instance) is rejected where it is written, whether
    /// or not it runs. A clocking drive cannot reach such a member: its
    /// clocking output declaration is already a frontend error. Input ports
    /// of a net type are not variables and are not checked here.
    fn validate_virtual_interface_input_port_writes(
        &self,
        interfaces: &[NodeId],
    ) -> Result<(), String> {
        let mut inputs: HashMap<String, HashSet<&str>> = HashMap::new();
        let mut instances: HashMap<String, Vec<NodeId>> = HashMap::new();
        for interface in interfaces {
            let Some(identity) = self.concrete_virtual_interface_identity(*interface) else {
                continue;
            };
            for child in &self.node(*interface).children {
                if let NodeKind::Port {
                    direction: DbDirection::Input,
                    low: Some(formal),
                    ..
                } = self.kind(*child)
                {
                    if self.is_variable_declaration(*formal) {
                        inputs
                            .entry(identity.clone())
                            .or_default()
                            .insert(self.node(*formal).name.as_str());
                    }
                }
            }
            instances.entry(identity).or_default().push(*interface);
        }
        if inputs.is_empty() {
            return Ok(());
        }
        for &path in self.virtual_interface_write_paths() {
            let NodeKind::Expr(ExprKind::HierPath { parts, refs }) = self.kind(path) else {
                continue;
            };
            let Some((position, spelling)) =
                refs.iter().enumerate().find_map(|(position, target)| {
                    self.virtual_interface_spelling((*target)?)
                        .map(|spelling| (position, spelling))
                })
            else {
                continue;
            };
            if self
                .virtual_interface_generate_scope(parts, refs, position)
                .is_some()
            {
                continue;
            }
            let steps = self.virtual_interface_nested_steps(parts, refs, position);
            let member =
                self.virtual_interface_member_name(path, parts, refs, position + steps.len());
            // A clockvar member (`cb.x`) names a clocking block, never a port.
            let Some(name) = member.split('.').next().filter(|name| !name.is_empty()) else {
                continue;
            };
            let identity = Self::normalize_virtual_interface_identity(
                &Self::virtual_interface_identity_from_spelling(&spelling),
            );
            let Some(bound) = self.nested_identity(&identity, &steps, &instances) else {
                continue;
            };
            if inputs.get(&bound).is_some_and(|names| names.contains(name)) {
                let written = parts
                    .iter()
                    .filter(|part| !part.is_empty())
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(".");
                return Err(located(
                    &self.source_location(path),
                    format!(
                        "semantic error: `{written}` writes `{name}`, an input port variable of \
                         interface `{bound}`, through a virtual interface; assignments to \
                         variables declared as input ports are illegal (IEEE 1800-2009 23.3.3.2)"
                    ),
                ));
            }
        }
        Ok(())
    }

    /// The type identity of the nested instances `steps` select from every
    /// instance of type `identity`, as the nested descriptor slots resolve
    /// them (`add_virtual_interface_nested`): `None` when there is no such
    /// instance or the instances do not all declare `steps` with one type.
    fn nested_identity(
        &self,
        identity: &str,
        steps: &[String],
        instances: &HashMap<String, Vec<NodeId>>,
    ) -> Option<String> {
        let mut identity = identity.to_owned();
        for step in steps {
            let mut child_identity = None;
            for parent in instances.get(&identity)? {
                let child = self.nested_interface_instance(*parent, step)?;
                let found = self.concrete_virtual_interface_identity(child)?;
                if child_identity.as_ref().is_some_and(|known| *known != found) {
                    return None;
                }
                child_identity = Some(found);
            }
            identity = child_identity?;
        }
        Some(identity)
    }

    /// The assignment targets node `id` writes: the target of an assignment
    /// statement, continuous or procedural continuous assignment, force,
    /// release or deassign; the first operand of an assignment, compound
    /// assignment, increment or decrement operation (the frontend wraps an
    /// output or inout actual in an assignment whose first operand is the
    /// actual); and the actuals of a call's non-`const` `ref` formals.
    pub(super) fn written_targets(&self, id: NodeId, out: &mut Vec<NodeId>) {
        match self.kind(id) {
            NodeKind::Stmt(StmtKind::Assign { .. }) | NodeKind::ContAssign { .. } => {
                out.extend(self.node(id).children.first().copied());
            }
            NodeKind::Stmt(
                StmtKind::Force { lhs, .. }
                | StmtKind::Release { lhs }
                | StmtKind::Deassign { lhs }
                | StmtKind::ProcContAssign { lhs, .. },
            ) => out.push(*lhs),
            NodeKind::Expr(ExprKind::Operation {
                op,
                assignment,
                operands,
                ..
            }) if *assignment
                || matches!(
                    op,
                    Operation::Assignment
                        | Operation::PostIncrement
                        | Operation::PreIncrement
                        | Operation::PostDecrement
                        | Operation::PreDecrement
                        | Operation::OverloadUpdate
                        | Operation::OverloadPostUpdate
                ) =>
            {
                out.extend(operands.first().copied());
            }
            NodeKind::FuncCall {
                callee: Some(callee),
                ..
            } => self.ref_actuals(self.node(id).children.as_slice(), *callee, out),
            NodeKind::MethodCall {
                receiver,
                callee: Some(callee),
                ..
            } => {
                let children = self.node(id).children.as_slice();
                let actuals = match children.split_first() {
                    Some((first, rest)) if receiver.as_ref() == Some(first) => rest,
                    _ => children,
                };
                self.ref_actuals(actuals, *callee, out);
            }
            _ => {}
        }
    }

    /// The actuals among `actuals` bound to a non-`const` `ref` formal of
    /// subroutine `callee`.
    fn ref_actuals(&self, actuals: &[NodeId], callee: NodeId, out: &mut Vec<NodeId>) {
        if !matches!(self.kind(callee), NodeKind::FuncTask { .. }) {
            return;
        }
        let formals = self
            .node(callee)
            .children
            .iter()
            .filter(|child| matches!(self.kind(**child), NodeKind::FuncArg { .. }));
        for (actual, formal) in actuals.iter().zip(formals) {
            if matches!(
                self.kind(*formal),
                NodeKind::FuncArg {
                    direction: DbDirection::Ref,
                    const_ref: false,
                    ..
                }
            ) {
                out.push(*actual);
            }
        }
    }

    /// The member paths through a virtual-interface handle that the
    /// assignment target `target` writes: the path itself, the base of a
    /// select of it, and each member of a concatenation, assignment pattern
    /// or streaming target. Selector expressions are reads, not followed.
    pub(super) fn handle_member_writes(&self, target: NodeId, out: &mut Vec<NodeId>) {
        match self.kind(target) {
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => {
                if refs
                    .iter()
                    .flatten()
                    .any(|target| self.virtual_interface_spelling(*target).is_some())
                {
                    out.push(target);
                }
            }
            NodeKind::Expr(
                ExprKind::BitSelect { base, .. }
                | ExprKind::PartSelect { base, .. }
                | ExprKind::IndexedPartSelect { base, .. }
                | ExprKind::ArraySelect { base, .. },
            ) => self.handle_member_writes(*base, out),
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Concat | Operation::AssignmentPattern,
                operands,
                ..
            }) => {
                for operand in operands {
                    self.handle_member_writes(*operand, out);
                }
            }
            NodeKind::Expr(ExprKind::Streaming { streams, .. }) => {
                for stream in streams {
                    self.handle_member_writes(stream.value, out);
                }
            }
            _ => {}
        }
    }

    /// The variables of interface instance `interface` that a write of its
    /// member `member` (`x`, or `cb.x` for a clockvar) stores to.
    fn interface_member_variables(&self, interface: NodeId, member: &str) -> Vec<NodeId> {
        let children = &self.node(interface).children;
        let Some((block, variable)) = member.split_once('.') else {
            return children
                .iter()
                .copied()
                .filter(|child| {
                    matches!(self.kind(*child), NodeKind::Var { .. })
                        && !self.db.is_clocking_var(*child)
                        && self.node(*child).name == member
                })
                .collect();
        };
        let Some(clockvar) = children
            .iter()
            .copied()
            .find(|child| self.db.is_clocking_block(*child) && self.node(*child).name == block)
            .and_then(|block| {
                self.node(block).children.iter().copied().find(|child| {
                    self.db.is_clocking_var(*child) && self.node(*child).name == variable
                })
            })
        else {
            return Vec::new();
        };
        let Some(info) = self.db.clocking_var(clockvar) else {
            return Vec::new();
        };
        let mut declarations = Vec::new();
        let mut stack = vec![info.expression];
        while let Some(node) = stack.pop() {
            match self.kind(node) {
                NodeKind::Expr(ExprKind::Ref {
                    target: Some(target),
                }) => declarations.push(*target),
                NodeKind::Expr(ExprKind::HierPath { refs, .. }) => {
                    declarations.extend(refs.iter().flatten().copied());
                }
                _ => {}
            }
            stack.extend(self.node(node).children.iter().copied());
        }
        declarations
    }

    /// The descriptor of nested instance `name` of `descriptor`, adding its
    /// nested slot (and the nested type's descriptor) on first use. `None`
    /// when the descriptor has no bound instance, or its bound instances do
    /// not all declare an interface instance `name` of one type identity.
    fn add_virtual_interface_nested(
        &mut self,
        descriptor: usize,
        name: &str,
        value_type: Option<&str>,
        interfaces: &[NodeId],
    ) -> Result<Option<usize>, String> {
        if let Some(slot) = self
            .virtual_interface_nested
            .get(&(descriptor, name.to_owned()))
        {
            return Ok(Some(
                self.model.virtual_interfaces[descriptor].nested[*slot].interface,
            ));
        }
        let mut bound = self
            .virtual_interface_instances
            .iter()
            .filter(|(_, (owner, _))| *owner == descriptor)
            .map(|(interface, (_, instance))| (*instance, *interface))
            .collect::<Vec<_>>();
        bound.sort_unstable_by_key(|(instance, _)| *instance);
        if bound.is_empty() {
            // No instance can be bound, so the handle is null whenever the
            // selection executes. A selection of the nested instance as a
            // value names its type (`value_type`); a member path through it
            // names none and is reported by its lowering.
            let Some(child_descriptor) = value_type.and_then(|spelling| {
                self.virtual_interface_types
                    .get(&Self::normalize_virtual_interface_identity(
                        &Self::virtual_interface_identity_from_spelling(spelling),
                    ))
                    .copied()
            }) else {
                return Ok(None);
            };
            let interface = &mut self.model.virtual_interfaces[descriptor];
            let slot = interface.nested.len();
            interface.nested.push(IrVirtualInterfaceNested {
                name: name.to_owned(),
                interface: child_descriptor,
            });
            self.virtual_interface_nested
                .insert((descriptor, name.to_owned()), slot);
            return Ok(Some(child_descriptor));
        }
        let mut children = Vec::with_capacity(bound.len());
        let mut identity = None;
        for (_, parent) in &bound {
            let Some(child) = self.nested_interface_instance(*parent, name) else {
                return Ok(None);
            };
            let child_identity = self.concrete_virtual_interface_identity(child);
            if child_identity.is_none() || (identity.is_some() && identity != child_identity) {
                return Ok(None);
            }
            identity = child_identity;
            children.push(child);
        }
        let Some(identity) = identity else {
            return Ok(None);
        };
        let child_descriptor = match self.virtual_interface_types.get(&identity) {
            Some(child_descriptor) => *child_descriptor,
            None => self.add_virtual_interface_descriptor(identity, interfaces)?,
        };
        let instances = children
            .iter()
            .map(|child| {
                self.virtual_interface_instances
                    .get(child)
                    .filter(|(owner, _)| *owner == child_descriptor)
                    .map(|(_, instance)| *instance)
                    .ok_or_else(|| {
                        format!(
                            "nested interface instance `{}` has no runtime environment",
                            self.node(*child).full_name
                        )
                    })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let interface = &mut self.model.virtual_interfaces[descriptor];
        let slot = interface.nested.len();
        interface.nested.push(IrVirtualInterfaceNested {
            name: name.to_owned(),
            interface: child_descriptor,
        });
        let mut instances = instances.into_iter();
        for instance in interface.instances.iter_mut() {
            instance.nested.push(instances.next());
        }
        self.virtual_interface_nested
            .insert((descriptor, name.to_owned()), slot);
        Ok(Some(child_descriptor))
    }

    /// The owned interface definitions of type `identity_base` (an identity
    /// without parameters), kept as uninstantiated instances under the root.
    fn virtual_interface_templates(&self, identity_base: &str) -> Vec<NodeId> {
        self.db
            .node_ids()
            .filter(|interface| {
                let NodeKind::ModuleInst {
                    def_name,
                    is_interface: true,
                    ..
                } = self.kind(*interface)
                else {
                    return false;
                };
                let Some(parent) = self.node(*interface).parent else {
                    return false;
                };
                if self.node(parent).parent.is_some() || self.node(*interface).children.is_empty() {
                    return false;
                }
                Self::normalize_virtual_interface_identity(&strip_lib(def_name)) == identity_base
            })
            .collect()
    }

    /// Add the descriptor of one elaborated virtual-interface type identity,
    /// with every design instance of that identity as a concrete instance.
    fn add_virtual_interface_descriptor(
        &mut self,
        identity: String,
        interfaces: &[NodeId],
    ) -> Result<usize, String> {
        let descriptor_index = self.model.virtual_interfaces.len();
        self.virtual_interface_types
            .insert(identity.clone(), descriptor_index);
        let identity_base = identity.split("#(").next().unwrap_or(&identity);
        let concrete = interfaces
            .iter()
            .copied()
            .filter(|interface| {
                self.concrete_virtual_interface_identity(*interface)
                    .as_deref()
                    == Some(identity.as_str())
            })
            .collect::<Vec<_>>();
        // The owned interface definitions are distinct from the
        // elaborated instance tree. Include a matching definition as a
        // metadata template so a type with no concrete instance (notably
        // a null-handle-only design) still has its member widths and
        // modport restrictions available to codegen.
        let templates = self.virtual_interface_templates(identity_base);
        let mut member_names = Vec::<String>::new();
        let mut method_names = Vec::<String>::new();
        let mut member_info = HashMap::<String, SignalInfo>::new();
        let mut method_info = HashMap::<String, usize>::new();
        let mut view_members = HashMap::<String, HashMap<String, DbDirection>>::new();
        let mut view_methods = HashMap::<String, HashSet<String>>::new();
        let mut entries = Vec::new();
        let mut metadata_interfaces = Vec::new();
        let mut seen_metadata = IdSet::new();
        for interface in templates.iter().chain(concrete.iter()) {
            if seen_metadata.insert(*interface) {
                metadata_interfaces.push(*interface);
            }
        }
        for interface in metadata_interfaces {
            for child in self.node(interface).children.iter().copied() {
                let NodeKind::ModPort = self.kind(child) else {
                    continue;
                };
                let view_name = self.node(child).name.clone();
                let members = view_members.entry(view_name.clone()).or_default();
                let methods = view_methods.entry(view_name).or_default();
                for port in self.node(child).children.iter().copied() {
                    match self.kind(port) {
                        NodeKind::ModPort => {
                            if let Some(direction) = self.db.modport_port_direction(port) {
                                members.insert(self.node(port).name.clone(), direction);
                            }
                        }
                        NodeKind::FuncTask { .. } => {
                            methods.insert(self.node(port).name.clone());
                        }
                        _ if self.db.semantic_detail(port) == Some("ModportClocking") => {
                            // `modport m(clocking cb)` exports every
                            // clockvar of the interface's block `cb`
                            // with its clocking direction (SV §25.5).
                            let name = &self.node(port).name;
                            let block =
                                self.node(interface).children.iter().copied().find(|block| {
                                    self.db.is_clocking_block(*block)
                                        && self.node(*block).name == *name
                                });
                            // The block event itself is read-only.
                            members.insert(name.clone(), DbDirection::Input);
                            for variable in block
                                .map(|block| self.node(block).children.clone())
                                .unwrap_or_default()
                            {
                                if let Some(info) = self.db.clocking_var(variable) {
                                    members.insert(
                                        format!("{name}.{}", self.node(variable).name),
                                        info.direction,
                                    );
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            let (template_members, _) = self.virtual_interface_template_entries(interface);
            for (name, signal) in template_members {
                member_info.entry(name.clone()).or_insert_with(|| {
                    member_names.push(name);
                    signal
                });
            }
        }
        for interface in &concrete {
            let (members, methods) = self.concrete_virtual_interface_entries(*interface);
            for (name, signal) in members {
                if let Some(existing) = member_info.get_mut(&name) {
                    // Concrete storage supplies the slot used by runtime
                    // entries; retain the template's name/order.
                    *existing = signal;
                } else {
                    member_names.push(name.clone());
                    member_info.insert(name, signal);
                }
            }
            for (name, function) in methods {
                if let std::collections::hash_map::Entry::Vacant(entry) =
                    method_info.entry(name.clone())
                {
                    entry.insert(function);
                    if !method_names.contains(&name) {
                        method_names.push(name);
                    }
                }
            }
        }
        // A member of another type (a class handle, a string) has no slot: an
        // access through the interface finds none and reports it there. A
        // string or handle member is recorded so that report names its type.
        let has_slot = |signal: &SignalInfo| {
            signal.real || (signal.width != 0 && signal.width <= LLG_MAX_WIDTH)
        };
        for name in &member_names {
            if member_info
                .get(name)
                .is_some_and(|signal| !signal.real && signal.width == 0)
            {
                self.virtual_interface_unvalued
                    .insert((descriptor_index, name.clone()));
            }
        }
        let members = member_names
            .iter()
            .filter(|name| member_info.get(*name).is_none_or(has_slot))
            .enumerate()
            .map(|(member_index, name)| {
                let signal = member_info.get(name).ok_or_else(|| {
                    format!("virtual interface member `{name}` has no signal metadata")
                })?;
                self.virtual_interface_members
                    .insert((descriptor_index, name.clone()), member_index);
                Ok(IrVirtualInterfaceMember {
                    name: name.clone(),
                    width: if signal.real { 0 } else { signal.width },
                    signed: signal.signed && !signal.real,
                    two_state: signal.two_state,
                    real: signal.real,
                    shortreal: signal.shortreal,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let methods = method_names
            .iter()
            .enumerate()
            .map(|(method_index, name)| {
                let function = *method_info.get(name).ok_or_else(|| {
                    format!("virtual interface method `{name}` has no function metadata")
                })?;
                self.virtual_interface_methods
                    .insert((descriptor_index, name.clone()), method_index);
                Ok(IrVirtualInterfaceMethod {
                    name: name.clone(),
                    function,
                    instances: vec![None; concrete.len()],
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        for (instance_index, interface) in concrete.iter().copied().enumerate() {
            let (concrete_members, concrete_methods) =
                self.concrete_virtual_interface_entries(interface);
            let concrete_members = concrete_members.into_iter().collect::<HashMap<_, _>>();
            let concrete_methods = concrete_methods.into_iter().collect::<HashMap<_, _>>();
            let member_slots = members
                .iter()
                .map(|member| concrete_members.get(&member.name).map(|signal| signal.ir))
                .collect::<Vec<_>>();
            let method_slots = methods
                .iter()
                .map(|method| concrete_methods.get(&method.name).copied())
                .collect::<Vec<_>>();
            entries.push(IrVirtualInterfaceInstance {
                c_name: format!(
                    "llg_vif_env_{}",
                    path_ident(
                        &self
                            .waveform_name(interface)
                            .split('\u{1f}')
                            .collect::<Vec<_>>()
                    )
                ),
                path: self.vpi_full_name(interface),
                members: member_slots,
                methods: method_slots,
                events: Vec::new(),
                nested: Vec::new(),
            });
            self.virtual_interface_instances
                .insert(interface, (descriptor_index, instance_index));
        }
        let mut methods = methods;
        for (method_index, method) in methods.iter_mut().enumerate() {
            method.instances = entries
                .iter()
                .map(|instance| instance.methods.get(method_index).copied().flatten())
                .collect();
        }
        self.model.virtual_interfaces.push(IrVirtualInterface {
            identity,
            members,
            instances: entries,
            methods,
            events: Vec::new(),
            nested: Vec::new(),
        });
        for (view, members) in view_members {
            self.virtual_interface_views
                .insert((descriptor_index, view), members);
        }
        for (view, methods) in view_methods {
            self.virtual_interface_view_methods
                .insert((descriptor_index, view), methods);
        }
        Ok(descriptor_index)
    }

    /// Resolve a method call whose receiver is a virtual-interface view to a
    /// descriptor method and one representative concrete implementation. The
    /// representative supplies the ordinary typed ABI; the IR call retains
    /// the descriptor/method pair so emission can dispatch on the runtime
    /// environment selected by the handle.
    pub(in super::super) fn virtual_interface_method_info(
        &self,
        node: NodeId,
    ) -> Result<Option<VirtualInterfaceMethodInfo>, String> {
        let NodeKind::MethodCall {
            name,
            receiver: Some(receiver),
            ..
        } = self.kind(node)
        else {
            return Ok(None);
        };
        let Some(spelling) = self.virtual_interface_spelling(*receiver) else {
            return Ok(None);
        };
        let identity = Self::normalize_virtual_interface_identity(
            &Self::virtual_interface_identity_from_spelling(&spelling),
        );
        if !self.virtual_interface_types.contains_key(&identity) {
            return Err(format!(
                "virtual interface type `{identity}` has no descriptor for method `{name}`"
            ));
        }
        // A method of a nested instance (`v.tx.f()`) dispatches on the
        // nested instance's descriptor.
        let (descriptor, _) = self.virtual_interface_nested_path(
            "",
            &spelling,
            self.db.virtual_interface_call_scopes(node),
        )?;
        let Some(&method) = self
            .virtual_interface_methods
            .get(&(descriptor, name.clone()))
        else {
            return Err(format!(
                "method `{name}` is not available through virtual interface view `{spelling}`"
            ));
        };
        if let Some(view) = Self::virtual_interface_view_from_spelling(&spelling) {
            let allowed = self
                .virtual_interface_view_methods
                .get(&(descriptor, view.clone()))
                .is_some_and(|methods| methods.contains(name));
            if !allowed {
                return Err(format!(
                    "method `{name}` is not imported through virtual interface view `{view}`"
                ));
            }
        }
        let function = self
            .model
            .virtual_interfaces
            .get(descriptor)
            .and_then(|interface| interface.methods.get(method))
            .map(|method| method.function)
            .ok_or_else(|| format!("virtual interface method `{name}` has no IR function"))?;
        let (&ft, _) = self
            .func_meta
            .iter()
            .find(|(_, meta)| meta.ir == function)
            .ok_or_else(|| format!("virtual interface method `{name}` has no function metadata"))?;
        let callee_inst = self.callable_environment(ft).ok_or_else(|| {
            format!("virtual interface method `{name}` has no concrete interface environment")
        })?;
        Ok(Some((descriptor, method, ft, callee_inst, *receiver)))
    }
}
