//! Design.

use super::*;

fn contains_unpacked_union(descriptor: &TypeDescriptor) -> bool {
    match &descriptor.shape {
        TypeShape::FixedArray { element, .. } => contains_unpacked_union(element),
        TypeShape::Aggregate(layout) => {
            layout.kind == AggregateKind::UnpackedUnion
                || layout
                    .members
                    .iter()
                    .any(|member| contains_unpacked_union(&member.descriptor))
        }
        _ => false,
    }
}

impl<'a> Codegen<'a> {
    /// Walk the instance tree, collecting signals, parameters and gen-scope
    /// paths.  Returns the top module nodes.
    pub(in super::super) fn collect_design(&mut self) -> Result<Vec<NodeId>, String> {
        self.collect_classes()?;
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
            if net_type.is_some()
                && self
                    .query_descriptor(node)
                    .is_some_and(contains_unpacked_union)
            {
                return Err(format!(
                    "unpacked union in `{}` is not a valid net type under IEEE 1800-2009 section 6.7",
                    self.display_name(node)
                ));
            }
        }
        let mut tops = Vec::new();
        for class in self.db.classes() {
            self.collect_class_funcs(*class)?;
        }
        let mut next_virtual_slot = 0;
        let mut class_virtual_slots = HashMap::new();
        for class in self.db.classes().to_vec() {
            self.assign_class_virtual_slots(
                class,
                &mut class_virtual_slots,
                &mut next_virtual_slot,
            );
        }
        // Package and `$unit` constants precede every importing module (SV
        // §26.3), whose declaration initializers read them while collecting.
        for scope in self
            .db
            .packages()
            .iter()
            .copied()
            .chain(self.compilation_unit_scopes())
        {
            self.collect_parameter_subtree(scope)?;
        }
        for top in self.db.tops() {
            let source_name = strip_lib(&self.node(*top).name);
            if source_name.is_empty() {
                return Err("top instance has no name".to_string());
            }
            let path = display_ident(&source_name);
            if self.design_name.is_empty() {
                self.design_name = source_name.clone();
                self.model.design_name = source_name;
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
            self.reject_shared_scope_redeclarations(*package)?;
            self.collect_instance(*package, &path)?;
            self.collect_funcs(*package, &path)?;
        }
        // Compilation-unit declarations form one shared `$unit` namespace per
        // admitted compilation unit. They are not design roots, so collect
        // them explicitly after package storage is allocated; references keep
        // their owned declaration identity across every importing module.
        for unit in self.compilation_unit_scopes() {
            let path = self.instance_path_of(unit);
            self.reject_shared_scope_redeclarations(unit)?;
            self.collect_instance(unit, &path)?;
            self.collect_funcs(unit, &path)?;
        }
        Ok(tops)
    }

    /// A name may be declared once per package or compilation-unit scope
    /// (SV §§3.12.1, 3.13). The frontend reports a merged-unit or package
    /// redeclaration only as a warning and keeps both declarations, which
    /// would otherwise alias one shared static storage name.
    fn reject_shared_scope_redeclarations(&self, scope: NodeId) -> Result<(), String> {
        let mut seen: HashMap<&str, NodeId> = HashMap::new();
        for child in &self.node(scope).children {
            if !matches!(
                self.kind(*child),
                NodeKind::Var { .. }
                    | NodeKind::Net { .. }
                    | NodeKind::Array { .. }
                    | NodeKind::NamedEvent
                    | NodeKind::FuncTask { .. }
            ) {
                continue;
            }
            let name = self.node(*child).name.as_str();
            if name.is_empty() {
                continue;
            }
            if let Some(previous) = seen.insert(name, *child) {
                let location = |node: NodeId| {
                    let node = self.node(node);
                    format!(
                        "{}:{}:{}",
                        node.file.as_deref().unwrap_or("<unknown>"),
                        node.line,
                        node.col
                    )
                };
                let scope_name = if self.is_compilation_unit(scope) {
                    "the compilation-unit scope".to_owned()
                } else {
                    format!("package `{}`", self.namespace_path(scope))
                };
                return Err(format!(
                    "redefinition of `{name}` in {scope_name} at {} (previous declaration at {}); a name is declared once per package or compilation-unit scope",
                    location(*child),
                    location(previous)
                ));
            }
        }
        Ok(())
    }

    pub(in super::super) fn compilation_unit_scopes(&self) -> Vec<NodeId> {
        self.db
            .node_ids()
            .filter(|id| self.is_compilation_unit(*id))
            .collect()
    }

    pub(in super::super) fn is_compilation_unit(&self, id: NodeId) -> bool {
        matches!(self.kind(id), NodeKind::Stmt(StmtKind::Begin))
            && self.db.semantic_detail(id) == Some("CompilationUnit")
    }

    pub(in super::super) fn is_runtime_environment(&self, id: NodeId) -> bool {
        matches!(self.kind(id), NodeKind::Package) || self.is_compilation_unit(id)
    }

    pub(in super::super) fn namespace_path(&self, id: NodeId) -> String {
        if matches!(self.kind(id), NodeKind::Package) {
            return strip_lib(&self.node(id).name);
        }
        if self.is_compilation_unit(id) {
            return format!("$unit_{}", id.index());
        }
        strip_lib(&self.node(id).name)
    }

    fn collect_instance(&mut self, inst: NodeId, path: &str) -> Result<(), String> {
        let display_path = self.instance_path_of(inst);
        self.display_paths.insert(path.to_owned(), display_path);
        self.instance_paths.insert(inst, path.to_owned());
        self.scope_nodes.insert(path.to_owned(), inst);
        let components = if self.is_runtime_environment(inst) {
            vec![path.to_owned()]
        } else {
            self.waveform_name(inst)
                .split('\u{1f}')
                .map(str::to_owned)
                .collect()
        };
        self.c_paths.insert(path.to_owned(), components);
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
                    if self.is_virtual_interface_array(nid) || self.is_fixed_handle_array(nid) {
                        let info = self.container_info(path, &name, nid, ty)?;
                        self.container_globals.insert(nid, info);
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
                NodeKind::Var { .. } if self.db.is_clocking_var(nid) => {}
                NodeKind::Net { ty, .. } | NodeKind::Var { ty } => {
                    let name = self.node(nid).name.clone();
                    if name.is_empty() || !seen.insert(name.clone()) {
                        continue;
                    }
                    let w = match self.unpacked_tagged_width(nid) {
                        Some(width) => width,
                        None => self.signal_width(path, name.as_str(), ty)?,
                    };
                    let ir = self.model.signals.len();
                    let info = SignalInfo {
                        global: if is_real_kind(&ty.kind) {
                            self.real_global_name(path, &name)
                        } else {
                            self.global_name(path, &name)
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
                        fixed_default: None,
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
                        net_alias: Vec::new(),
                        alias: None,
                        omit: false,
                    });
                    self.signals.push(info.clone());
                    self.insert_sig_global(nid, info.clone());
                    self.scope_sig_names
                        .entry(path.to_string())
                        .or_default()
                        .insert(name, info);
                }
                NodeKind::Param { .. } => self.collect_parameter_value(inst, nid)?,
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
        self.collect_scope_block_records(path, inst)?;
        // Parent declaration initializers may read elaborated child parameters
        // through bound hierarchical references. Populate only parameter values
        // here so descendant runtime initializers retain their normal order.
        self.collect_descendant_parameter_values(inst)?;
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
        // The frontend reports a repeated instance name only as a warning
        // when a bind injects it (SV §23.11 places bound instances in the
        // target's name space, where a repeated name is illegal).
        let mut child_names: HashSet<&str> = HashSet::new();
        for c in &self.node(inst).children {
            if matches!(self.kind(*c), NodeKind::ModuleInst { .. })
                && !child_names.insert(self.node(*c).name.as_str())
            {
                let node = self.node(*c);
                return Err(format!(
                    "duplicate instance name `{}` in `{}` at {}:{}:{}; bound and declared instances share the target scope's name space (SV 23.11)",
                    node.name,
                    self.display_path(path),
                    node.file.as_deref().unwrap_or("<unknown>"),
                    node.line,
                    node.col
                ));
            }
        }
        for c in &self.node(inst).children {
            if matches!(self.kind(*c), NodeKind::ModuleInst { .. }) {
                let cname = self.node(*c).name.clone();
                if cname.is_empty() {
                    return Err(format!("unnamed child instance in `{path}`"));
                }
                let child_path = format!("{path}.{}", display_ident(&cname));
                self.collect_instance(*c, &child_path)?;
            }
        }
        Ok(())
    }

    /// Procedural-block records of the processes directly in `scope`,
    /// collected after the scope's own declarations.
    fn collect_scope_block_records(&mut self, path: &str, scope: NodeId) -> Result<(), String> {
        for child in &self.node(scope).children {
            if matches!(self.kind(*child), NodeKind::Process { .. }) {
                self.collect_block_records(path, *child)?;
            }
        }
        Ok(())
    }

    fn collect_parameter_value(&mut self, scope: NodeId, parameter: NodeId) -> Result<(), String> {
        if self.param_vals.contains_key(&parameter) {
            return Ok(());
        }
        let frontend_value = match self.kind(parameter) {
            NodeKind::Param { value, .. } => value.clone(),
            _ => return Ok(()),
        };
        if let Some(value) =
            self.collected_parameter_value(scope, parameter, frontend_value.as_ref())?
        {
            self.param_vals.insert(parameter, value);
        }
        Ok(())
    }

    fn collect_descendant_parameter_values(&mut self, scope: NodeId) -> Result<(), String> {
        for child in self.node(scope).children.clone() {
            if matches!(
                self.kind(child),
                NodeKind::GenScopeArray | NodeKind::GenScope | NodeKind::ModuleInst { .. }
            ) {
                self.collect_parameter_subtree(child)?;
            }
        }
        Ok(())
    }

    fn collect_parameter_subtree(&mut self, scope: NodeId) -> Result<(), String> {
        let children = self.node(scope).children.clone();
        for child in &children {
            if matches!(self.kind(*child), NodeKind::Param { .. }) {
                self.collect_parameter_value(scope, *child)?;
            }
        }
        for child in children {
            if matches!(
                self.kind(child),
                NodeKind::GenScopeArray | NodeKind::GenScope | NodeKind::ModuleInst { .. }
            ) {
                self.collect_parameter_subtree(child)?;
            }
        }
        Ok(())
    }

    /// Record the C function name of every function/task definition in the
    /// instance tree, recursing into child instances and instances inside
    /// generate scopes.
    fn collect_funcs(&mut self, inst: NodeId, path: &str) -> Result<(), String> {
        for c in &self.node(inst).children {
            if matches!(self.kind(*c), NodeKind::FuncTask { .. }) {
                let fname = self.node(*c).name.clone();
                let c_name = self.c_name("fn", path, &[&fname]);
                self.func_labels
                    .insert(*c, format!("{}.{fname}", self.source_path(path)));
                self.func_names.insert(*c, c_name);
            }
        }
        for c in &self.node(inst).children {
            if matches!(
                self.kind(*c),
                NodeKind::ModuleInst { .. } | NodeKind::GenScope | NodeKind::GenScopeArray
            ) {
                let child_path = self.instance_path_of(*c);
                self.collect_funcs(*c, &child_path)?;
            }
        }
        Ok(())
    }

    /// Storage width of a finite unpacked tagged union. Like a packed tagged
    /// union it is one packed owner: the tag in the most significant bits and
    /// each member right-justified below it. The layout is internal because
    /// unpacked unions are not bit-stream types (SV 7.3.2, 11.4.14).
    pub(in super::super) fn unpacked_tagged_width(&self, node: NodeId) -> Option<u32> {
        let descriptor = self.query_descriptor(node)?;
        let TypeShape::Aggregate(layout) = &descriptor.shape else {
            return None;
        };
        // Slang gives unpacked aggregates no integral width (absent or zero).
        if layout.kind != AggregateKind::TaggedUnion
            || descriptor.info.width.is_some_and(|width| width != 0)
        {
            return None;
        }
        Self::fixed_descriptor_width(descriptor)
    }

    pub(super) fn signal_width(
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
            format!("{path}.{}", display_ident(&gs_name))
        };
        let mut components = self
            .c_paths
            .get(path)
            .cloned()
            .unwrap_or_else(|| vec![path.to_owned()]);
        components.push(if gs_name.is_empty() {
            "genblk".to_owned()
        } else {
            gs_name
        });
        self.c_paths.insert(gs_path.clone(), components);
        self.gen_scope_paths.insert(gs, gs_path.clone());
        self.scope_nodes.insert(gs_path.clone(), gs);
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
                    if self.is_virtual_interface_array(nid)
                        || self.is_fixed_handle_array(nid)
                        || !self
                            .db
                            .array_meta(nid)
                            .is_some_and(|meta| matches!(meta.kind(), ArrayKind::Static))
                    {
                        let info = self.container_info(&gs_path, &name, nid, ty)?;
                        self.container_globals.insert(nid, info);
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
                NodeKind::Var { .. } if self.db.is_clocking_var(nid) => {}
                NodeKind::Net { ty, .. } | NodeKind::Var { ty } => {
                    let name = self.node(nid).name.clone();
                    if name.is_empty() || !gseen.insert(name.clone()) {
                        continue;
                    }
                    let w = match self.unpacked_tagged_width(nid) {
                        Some(width) => width,
                        None => self.signal_width(&gs_path, &name, ty)?,
                    };
                    let ir = self.model.signals.len();
                    let info = SignalInfo {
                        global: if is_real_kind(&ty.kind) {
                            self.real_global_name(&gs_path, &name)
                        } else {
                            self.global_name(&gs_path, &name)
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
                        fixed_default: None,
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
                        net_alias: Vec::new(),
                        alias: None,
                        omit: false,
                    });
                    self.signals.push(info.clone());
                    self.insert_sig_global(nid, info.clone());
                    self.scope_sig_names
                        .entry(gs_path.clone())
                        .or_default()
                        .insert(name, info);
                }
                NodeKind::Param { .. } => self.collect_parameter_value(gs, nid)?,
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
        self.collect_scope_block_records(&gs_path, gs)?;
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
                let child_path = format!("{gs_path}.{}", display_ident(&self.node(*c).name));
                self.collect_instance(*c, &child_path)?;
            }
        }
        Ok(())
    }

    /// Use admitted source text only as a rejection fallback for an unresolved
    /// top-self hierarchical continuous LHS. Accepted direct drivers still
    /// require an owned target identity.
    pub(super) fn cont_assign_source_has_hier_lhs(&self, ca: NodeId, net: NodeId) -> bool {
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
    pub(in super::super) fn design_nodes(&self) -> Vec<NodeId> {
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

    /// The module instance that owns `node` (walking up the parent chain).
    pub(in super::super) fn owning_inst(&self, node: NodeId) -> Option<NodeId> {
        let mut cur = self.node(node).parent;
        while let Some(p) = cur {
            if matches!(self.kind(p), NodeKind::ModuleInst { .. }) {
                return Some(p);
            }
            cur = self.node(p).parent;
        }
        None
    }
}
