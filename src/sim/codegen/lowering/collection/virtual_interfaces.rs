//! Virtual interfaces.

use super::*;

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
                let NodeKind::Param { value, .. } = self.kind(*child) else {
                    return None;
                };
                let value =
                    self.param_vals
                        .get(child)
                        .or(value.as_ref())
                        .map(|value| match value {
                            Val::Bits(bits) => bits
                                .to_i128()
                                .map(|value| value.to_string())
                                .unwrap_or_else(|| value.format_verilog()),
                            Val::Real(value) => value.to_string(),
                            Val::Str(value) => format!("\"{value}\""),
                        })?;
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
            .into_iter()
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

        for identity in identities {
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
            let templates = self
                .db
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
                    if self.node(parent).parent.is_some()
                        || self.node(*interface).children.is_empty()
                    {
                        return false;
                    }
                    Self::normalize_virtual_interface_identity(&strip_lib(def_name))
                        == identity_base
                })
                .collect::<Vec<_>>();
            let mut member_names = Vec::<String>::new();
            let mut method_names = Vec::<String>::new();
            let mut member_info = HashMap::<String, SignalInfo>::new();
            let mut method_info = HashMap::<String, usize>::new();
            let mut view_members = HashMap::<String, HashMap<String, DbDirection>>::new();
            let mut view_methods = HashMap::<String, HashSet<String>>::new();
            let mut entries = Vec::new();
            let mut metadata_interfaces = Vec::new();
            let mut seen_metadata = HashSet::new();
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
            let members = member_names
                .iter()
                .enumerate()
                .map(|(member_index, name)| {
                    let signal = member_info.get(name).ok_or_else(|| {
                        format!("virtual interface member `{name}` has no signal metadata")
                    })?;
                    if signal.real || signal.width == 0 || signal.width > LLG_MAX_WIDTH {
                        return Err(format!(
                            "virtual interface member `{name}` must be a packed signal within the runtime width limit"
                        ));
                    }
                    self.virtual_interface_members
                        .insert((descriptor_index, name.clone()), member_index);
                    Ok(IrVirtualInterfaceMember {
                        name: name.clone(),
                        width: signal.width,
                        signed: signal.signed,
                        two_state: signal.two_state,
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
                    c_name: format!("llg_vif_env_{}", ident(&self.node(interface).full_name)),
                    members: member_slots,
                    methods: method_slots,
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
            });
            for (view, members) in view_members {
                self.virtual_interface_views
                    .insert((descriptor_index, view), members);
            }
            for (view, methods) in view_methods {
                self.virtual_interface_view_methods
                    .insert((descriptor_index, view), methods);
            }
        }
        Ok(())
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
        let Some(&descriptor) = self.virtual_interface_types.get(&identity) else {
            return Err(format!(
                "virtual interface type `{identity}` has no descriptor for method `{name}`"
            ));
        };
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
