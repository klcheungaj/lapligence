//! Owned-database collection, storage allocation, wiring, and process setup.

use super::*;

impl<'a> Codegen<'a> {
    /// Register storage for a declaration local to a procedural loop.
    ///
    /// Keeping this arena-node mapping separate from model signals preserves
    /// lexical storage and prevents loop indices from appearing as waveform
    /// globals.
    pub(super) fn collect_loop_var(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<ProcLocalInfo, String> {
        if let Some(info) = self.proc_locals.get(&node) {
            return Ok(info.clone());
        }
        let ty = match self.kind(node) {
            NodeKind::Var { ty } => ty.clone(),
            other => {
                return Err(format!(
                    "unsupported procedural loop declaration in `{path}` (node kind {other:?})"
                ))
            }
        };
        if is_real_kind(&ty.kind) {
            return Err(format!(
                "real/shortreal procedural loop variable `{}` is not supported in `{path}`",
                self.node(node).name
            ));
        }
        let width = self.signal_width(path, &self.node(node).name, &ty)?;
        let info = ProcLocalInfo {
            c_name: format!("_lv{}", node.index()),
            width,
            signed: ty.signed,
        };
        self.proc_locals.insert(node, info.clone());
        Ok(info)
    }

    pub(super) fn proc_local_target(&self, node: NodeId) -> Option<NodeId> {
        if let Some((variable, _)) = self.lexical_proc_local(node) {
            return Some(variable);
        }
        if self.proc_local_is_shadowed(node) {
            return None;
        }
        match self.kind(node) {
            NodeKind::Var { .. } if self.proc_locals.contains_key(&node) => Some(node),
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) if self.proc_locals.contains_key(target) => Some(*target),
            _ => None,
        }
    }

    pub(super) fn lexical_proc_local(&self, reference: NodeId) -> Option<(NodeId, &ProcLocalInfo)> {
        let name = self.node(reference).name.as_str();
        let mut parent = self.node(reference).parent;
        while let Some(scope) = parent {
            if matches!(self.kind(scope), NodeKind::Stmt(StmtKind::Begin))
                && self.node(scope).children.iter().any(|child| {
                    matches!(self.kind(*child), NodeKind::Var { .. })
                        && self.node(*child).name == name
                })
            {
                return None;
            }
            let vars = match self.kind(scope) {
                NodeKind::Stmt(StmtKind::For { vars, .. })
                | NodeKind::Stmt(StmtKind::Foreach { vars, .. }) => Some(vars.as_slice()),
                _ => None,
            };
            if let Some(variable) = vars.and_then(|vars| {
                vars.iter()
                    .find(|variable| self.node(**variable).name == name)
            }) {
                if let Some(info) = self.proc_locals.get(variable) {
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
                NodeKind::Stmt(StmtKind::For { vars, .. })
                | NodeKind::Stmt(StmtKind::Foreach { vars, .. }) => vars
                    .iter()
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
        if let Some((variable, _)) = self.lexical_proc_local(node) {
            return Some(variable);
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
            if self.proc_locals.contains_key(target) {
                return Some(*target);
            }
        }
        self.node(node)
            .children
            .iter()
            .find_map(|child| self.nested_proc_local_ref(*child))
    }

    /// Walk the instance tree, collecting signals, parameters and gen-scope
    /// paths.  Returns the top module nodes.
    pub(super) fn collect_design(&mut self) -> Result<Vec<NodeId>, String> {
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
        Ok(tops)
    }

    /// Collect the arena nodes of every per-port COPY interface instance: the
    /// `low` targets of interface-typed ports.  A modport port's `low` is the
    /// copy's `vpiModport` (whose parent is the copy interface instance); a
    /// bare interface port's `low` is the copy interface instance itself.
    /// This mirrors `emit_iface_link`'s copy lookup.
    pub(super) fn collect_iface_copies(&mut self) {
        let mut copies = HashSet::new();
        for top in self.db.tops() {
            self.collect_iface_copies_in(*top, &mut copies);
        }
        self.iface_copy_insts = copies;
    }

    fn collect_iface_copies_in(&self, inst: NodeId, copies: &mut HashSet<NodeId>) {
        for c in &self.node(inst).children {
            match self.kind(*c) {
                NodeKind::Port { low: Some(l), .. } => match self.kind(*l) {
                    NodeKind::ModPort => {
                        if let Some(iface) = self.node(*l).parent {
                            if matches!(
                                self.kind(iface),
                                NodeKind::ModuleInst {
                                    is_interface: true,
                                    ..
                                }
                            ) {
                                copies.insert(iface);
                            }
                        }
                    }
                    NodeKind::ModuleInst {
                        is_interface: true, ..
                    } => {
                        copies.insert(*l);
                    }
                    _ => {}
                },
                NodeKind::ModuleInst { .. } => self.collect_iface_copies_in(*c, copies),
                NodeKind::GenScopeArray => {
                    for gs in &self.node(*c).children {
                        if matches!(self.kind(*gs), NodeKind::GenScope) {
                            self.collect_iface_copies_in(*gs, copies);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn collect_instance(&mut self, inst: NodeId, path: &str) -> Result<(), String> {
        for child in &self.node(inst).children {
            if let NodeKind::Port { high, low, .. } = self.kind(*child) {
                for side in [high, low].into_iter().flatten() {
                    if self.signal_node_is_real(*side) {
                        return Err(format!(
                            "real/shortreal ports are not supported in `{path}` (port `{}`)",
                            self.node(*child).name
                        ));
                    }
                }
            }
        }
        let mut seen: HashSet<String> = HashSet::new();
        for c in &self.node(inst).children {
            let nid = *c;
            match self.kind(nid) {
                NodeKind::Array { ty } => {
                    let name = self.node(nid).name.clone();
                    if name.is_empty() || !seen.insert(name.clone()) {
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
                            }
                        },
                        net_driver: None,
                        omit: false,
                    });
                    self.signals.push(info.clone());
                    self.sig_globals.insert(nid, info.clone());
                    self.scope_sig_names
                        .entry(path.to_string())
                        .or_default()
                        .insert(name, info);
                }
                NodeKind::Param { value: Some(v), .. } => {
                    self.param_vals.insert(nid, v.clone());
                }
                NodeKind::NamedEvent => {
                    let name = self.node(nid).name.clone();
                    if name.is_empty() || !seen.insert(name.clone()) {
                        continue;
                    }
                    let ir = self.model.events.len();
                    let info = EventInfo {
                        global: event_global_name(path, &name),
                        ir,
                    };
                    self.model.events.push(IrEvent {
                        c_name: info.global.clone(),
                    });
                    self.events.push(info.clone());
                    self.event_globals.insert(nid, info);
                }
                // A declaration initializer on an `array_net` (`reg [7:0] m
                // [0:3] = '{…}` — Surelog models the pattern as a
                // net-decl-assign continuous assignment whose LHS is the
                // array).  Applied to the array's `ArrayInfo`; the assignment
                // itself is skipped at emission. A scalar reg initializer is
                // collected into `scalar_inits`; true nets stay available to
                // `emit_cont_assign` as continuous drivers.
                NodeKind::ContAssign {
                    net_decl: true,
                    delay: _,
                } => {
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
                    } else if matches!(self.net_decl_target(nid), NetDeclTarget::Variable) {
                        let (info, c) = self.scalar_decl_init(path, nid)?.ok_or_else(|| {
                            format!(
                                "variable declaration initializer is not a constant expression \
                                 in `{path}`"
                            )
                        })?;
                        self.scalar_inits.push((info, c));
                        self.scalar_init_ca.insert(nid);
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
        for c in &self.node(inst).children {
            if matches!(self.kind(*c), NodeKind::GenScopeArray) {
                self.collect_gen_scope_array(*c, path)?;
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
    /// lives on the var's `vpiExpr` (`logic l = 1'b0;`, `int x = 5;` —
    /// captured in [`Db::vars_init`]) into a constant and queue it for
    /// `main()`.  Called after the scope's parameters are collected so
    /// `P + 1`-style RHS refs resolve via `param_vals`.  Vars that are not
    /// collected as signals (function/block locals, per-port copies) carry no
    /// fill; their initializers are handled by their own paths.
    fn collect_var_inits(&mut self, path: &str, inst: NodeId) -> Result<(), String> {
        for c in &self.node(inst).children {
            if !matches!(self.kind(*c), NodeKind::Var { .. }) {
                continue;
            }
            let init = match self.db.var_initializer(*c) {
                Some(init) => init,
                None => continue,
            };
            let info = match self.signal_of(*c) {
                Some(info) => info.clone(),
                None => continue,
            };
            let name = self.node(*c).name.clone();
            let cconst = self.var_decl_init(path, &name, init)?;
            self.var_inits.push((info, cconst));
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

    /// Width of a Net/Var from its captured `TypeInfo`; rejects unsupported
    /// kinds and widths above `LLG_MAX_WIDTH` bits.
    fn signal_node_is_real(&self, node: NodeId) -> bool {
        match self.kind(node) {
            NodeKind::Net { ty, .. } | NodeKind::Var { ty } | NodeKind::Array { ty } => {
                is_real_kind(&ty.kind)
                    || matches!(ty.kind.as_str(), "real_array" | "shortreal_array")
            }
            _ => false,
        }
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
            "int" | "integer" | "time" | "longint" | "byte" | "shortint" | "logic" | "bit"
            | "enum" => ty.width.unwrap_or(1),
            _ => {
                return Err(format!(
                    "unsupported typespec type for signal `{name}` in `{path}`"
                ))
            }
        };
        if w > LLG_MAX_WIDTH {
            return Err(format!(
                "signal `{name}` in `{path}` is {w} bits wide; the v1 runtime \
                 supports at most {LLG_MAX_WIDTH}"
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
        // Surelog names the enclosing `gen_scope_array` (e.g. `g[0]` for the
        // genvar-loop iteration) and leaves the `gen_scope` itself unnamed;
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
                            }
                        },
                        net_driver: None,
                        omit: false,
                    });
                    self.signals.push(info.clone());
                    self.sig_globals.insert(nid, info.clone());
                    self.scope_sig_names
                        .entry(gs_path.clone())
                        .or_default()
                        .insert(name, info);
                }
                NodeKind::Param { value: Some(v), .. } => {
                    self.param_vals.insert(nid, v.clone());
                }
                NodeKind::NamedEvent => {
                    let name = self.node(nid).name.clone();
                    if name.is_empty() || !gseen.insert(name.clone()) {
                        continue;
                    }
                    let ir = self.model.events.len();
                    let info = EventInfo {
                        global: event_global_name(&gs_path, &name),
                        ir,
                    };
                    self.model.events.push(IrEvent {
                        c_name: info.global.clone(),
                    });
                    self.events.push(info.clone());
                    self.event_globals.insert(nid, info);
                }
                NodeKind::ContAssign {
                    net_decl: true,
                    delay: _,
                } => {
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
                    } else if matches!(self.net_decl_target(nid), NetDeclTarget::Variable) {
                        let (info, c) = self.scalar_decl_init(&gs_path, nid)?.ok_or_else(|| {
                            format!(
                                "variable declaration initializer is not a constant expression \
                                 in `{gs_path}`"
                            )
                        })?;
                        self.scalar_inits.push((info, c));
                        self.scalar_init_ca.insert(nid);
                    }
                }
                _ => {}
            }
        }
        // Variable declaration initializers, folded after the scope's own
        // parameters are collected (see `collect_var_inits`).
        self.collect_var_inits(&gs_path, gs)?;
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

    /// Collapse inout-port net pairs (parent `vpiHighConn` + child
    /// `vpiLowConn`) into one resolved simulated net per connected set
    /// (LRM §23.3.3.7), run after [`collect_design`](Self::collect_design)
    /// and before any emission.
    ///
    /// Every grouped member's `SignalInfo` is redirected to the shared
    /// `llg_net_t`'s `resolved` cell and tagged with its driver slot, so
    /// reads/writes/sensitivity all use the resolution cell automatically.
    /// Groups with anything the runtime cannot resolve (non-net members,
    /// mixed widths, unsupported net types, select-LHS/NBA/task-actual
    /// writes) are skipped with an explicit warning — never silently.
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
                self.warnings.push(format!(
                    "inout port `{}` of `{}`: child-side connection not \
                     resolved; port connection skipped",
                    self.node(*id).name,
                    self.node(*id)
                        .parent
                        .map(|p| self.node(p).name.clone())
                        .unwrap_or_default()
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
                self.warnings.push(format!(
                    "{joined}: member `{}` is not a net; group skipped (inout \
                     connection dropped)",
                    self.display_name(*bad)
                ));
                continue;
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
                self.warnings.push(format!(
                    "{joined}: member `{}` has a different width than `{}`; \
                     group skipped (inout connection dropped)",
                    self.display_name(*bad),
                    self.display_name(members[0])
                ));
                continue;
            }
            // 3. Only wire/tri/logic nets support the equal-strength
            //    resolution the runtime implements (Table 6-2).
            if let Some(bad) = members.iter().find(|m| match self.kind(**m) {
                NodeKind::Net { net_type, .. } => {
                    !matches!(*net_type, NetType::Wire | NetType::Tri | NetType::Logic)
                        && *net_type != vpi::vpiNet
                }
                _ => true,
            }) {
                let net_type = match self.kind(*bad) {
                    NodeKind::Net { net_type, .. } => *net_type,
                    _ => NetType::None,
                };
                self.warnings.push(format!(
                    "{joined}: member `{}` has unsupported net type {net_type} \
                     (only wire/tri/logic nets resolve); group skipped (inout \
                     connection dropped)",
                    self.display_name(*bad)
                ));
                continue;
            }
            // 4. The runtime struct has a fixed driver-slot array.
            if members.len() > LLG_MAX_NET_DRIVERS {
                self.warnings.push(format!(
                    "{joined}: {}-member group exceeds the {LLG_MAX_NET_DRIVERS} \
                     driver-slot limit; group skipped (inout connection dropped)",
                    members.len()
                ));
                continue;
            }
            // 5. Only whole-signal drivers may touch a member: select LHS,
            //    NBA writes and task `sv4_t*` actuals would bypass the
            //    resolution cell.
            if let Some(reason) = self.unsupported_member_write(members) {
                self.warnings.push(format!(
                    "{joined}: {reason}; group skipped (inout connection \
                     dropped)"
                ));
                continue;
            }

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
                n_drivers: members.len(),
            });
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
        Ok(())
    }

    /// Every arena node of the instance tree (top instances + children +
    /// generate scopes), depth-first, in deterministic order.
    fn design_nodes(&self) -> Vec<NodeId> {
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
    fn waveform_name(&self, id: NodeId) -> String {
        const SEPARATOR: &str = "\u{1f}";

        let mut parts = vec![self.node(id).name.clone()];
        let mut current = self.node(id).parent;
        while let Some(scope_id) = current {
            let scope = self.node(scope_id);
            if matches!(
                scope.kind,
                NodeKind::ModuleInst { .. } | NodeKind::GenScopeArray | NodeKind::GenScope
            ) {
                // Surelog library-qualifies top design units (`work@tb`).
                // Other `vpiName` components are source identifiers, where
                // `@` is legal in an escaped spelling and must be preserved.
                let name = match &scope.kind {
                    NodeKind::ModuleInst { is_top: true, .. } => strip_lib(&scope.name),
                    _ => scope.name.clone(),
                };
                if !name.is_empty() {
                    parts.push(name);
                }
            }
            current = scope.parent;
        }
        parts.reverse();
        parts.join(SEPARATOR)
    }

    /// The reason a candidate inout-net group cannot be supported, from a
    /// design-wide scan of every write targeting its members: `None` when
    /// every write is a whole-signal (blocking or continuous) driver.
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
                            return Some(format!(
                                "bit/part/select LHS on member `{}`",
                                self.display_name(lhs)
                            ))
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
                                self.display_name(lhs)
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
                    if self.member_write_base(lhs, &member_set).is_some() {
                        return Some(format!(
                            "nonblocking assignment to member `{}`",
                            self.display_name(lhs)
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
    /// signal (supported), or through a select (unsupported).
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
            NodeKind::Expr(ExprKind::Ref { target }) => target.filter(|t| member_set.contains(t)),
            NodeKind::Expr(
                ExprKind::BitSelect { base, .. }
                | ExprKind::PartSelect { base, .. }
                | ExprKind::IndexedPartSelect { base, .. }
                | ExprKind::ArraySelect { base, .. },
            ) => self.member_write_base(*base, member_set),
            _ => None,
        }
    }

    /// Whether a task call binds an output/inout formal to a member: those
    /// actuals become `sv4_t*` parameters in the emitted C and would write
    /// through the resolved cell, bypassing resolution.
    fn task_actual_member_write(
        &self,
        call: NodeId,
        member_set: &HashSet<NodeId>,
    ) -> Option<String> {
        let (name, callee) = match self.kind(call) {
            NodeKind::FuncCall {
                name,
                is_task: true,
                callee,
                ..
            } => (name.clone(), *callee),
            _ => return None,
        };
        let inst = self.owning_inst(call)?;
        let ft = self.resolve_callee(inst, &name, true, callee).ok()?;
        let (_, _, formals) = self.func_info(ft).ok()?;
        let args: Vec<NodeId> = self.node(call).children.clone();
        for (idx, (io, is_out)) in formals.iter().enumerate() {
            if !*is_out {
                continue;
            }
            if let Some(arg) = args.get(idx) {
                if let Some(m) = self.member_write_base(*arg, member_set) {
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
    fn owning_inst(&self, node: NodeId) -> Option<NodeId> {
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

    /// The array a `vpiNetDeclAssign` continuous assignment initializes (its
    /// LHS resolves to an `Array` node), or `None` for other assignments.
    fn cont_assign_array_target(&self, ca: NodeId) -> Option<NodeId> {
        self.node(ca)
            .children
            .first()
            .copied()
            .and_then(|lhs| self.ref_array_target(lhs))
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
                NetType::Wire | NetType::Tri | NetType::Logic => NetDeclTarget::TrueNet,
                other if other == vpi::vpiNet => NetDeclTarget::TrueNet,
                NetType::Reg => NetDeclTarget::Variable,
                other => NetDeclTarget::UnsupportedNet(other),
            },
            _ => NetDeclTarget::Unknown,
        }
    }

    /// The declaration-initializer constants of a `vpiNetDeclAssign`
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
        let vals = self.array_init_consts(path, &name, rhs)?;
        Ok(Some((target, vals)))
    }

    /// The declaration-initializer constant of a `vpiNetDeclAssign` whose LHS
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
        // The RHS is a constant expression in practice (Surelog folds
        // declaration-initializer expressions at elaboration); a plain
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
    /// lives on the var's `vpiExpr` (`logic l = 1'b0;`, `int x = 5;`).  The
    /// RHS is a constant expression in practice (Surelog folds
    /// declaration-initializer expressions at elaboration): a plain constant
    /// first, then constant-foldable operations/params via `eval_bits` (which
    /// resolves parameter references through `param_vals`).  Anything
    /// non-constant is rejected — v1 variable initializers must be constant
    /// expressions.
    fn var_decl_init(&self, path: &str, name: &str, init: NodeId) -> Result<IrConst, String> {
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
                if *op == vpi::vpiAssignmentPatternOp =>
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
            .map(|o| match self.kind(*o) {
                NodeKind::Expr(ExprKind::Constant { .. }) => self.const_of_node(*o),
                other => Err(format!(
                    "array `{name}` in `{path}`: initializer element is not a \
                     constant ({other:?})"
                )),
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
        let elem_width = match ty.kind.as_str() {
            "real" | "shortreal" | "real_array" | "shortreal_array" => {
                return Err(format!(
                    "array `{name}` in `{path}` has unsupported element type `{}`",
                    ty.kind
                ))
            }
            "int" | "integer" | "time" | "longint" | "byte" | "shortint" | "logic" | "bit" => {
                ty.width.unwrap_or(1)
            }
            _ => {
                return Err(format!(
                    "array `{name}` in `{path}` has unsupported element type `{}`",
                    ty.kind
                ))
            }
        };
        if elem_width > LLG_MAX_WIDTH {
            return Err(format!(
                "array `{name}` in `{path}` has {elem_width}-bit elements; the v1 \
                 runtime supports at most {LLG_MAX_WIDTH}"
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
            Some(eid) => Some(self.array_init_consts(path, name, eid)?),
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
            dims: dims.clone(),
            total,
        });
        Ok(ArrayInfo {
            global: global_name(path, name),
            elem_width,
            signed: ty.signed,
            dims,
            init,
            ir,
        })
    }

    // ── Functions and tasks ───────────────────────────────────────────────

    /// Emit a `static` prototype for every function/task in the instance
    /// tree, so bodies may call each other regardless of declaration order.
    /// Delay-bearing tasks are never emitted as C functions (they are inlined
    /// at their call sites), so they get no prototype.
    pub(super) fn emit_func_prototypes(&mut self, inst: NodeId) -> Result<(), String> {
        for c in &self.node(inst).children {
            if let NodeKind::FuncTask { is_task, .. } = self.kind(*c) {
                if *is_task && self.task_has_wait(*c, inst) {
                    continue;
                }
                let (is_task_f, ret, formals) = self.func_info(*c)?;
                let c_name =
                    self.func_names.get(c).cloned().ok_or_else(|| {
                        format!("function `{}` has no C name", self.node(*c).name)
                    })?;
                // Register the model entry (call-site lowering and the C
                // renderers resolve through it).
                let ir = self.model.funcs.len();
                let formals_ir = formals
                    .iter()
                    .map(|(io, is_out)| match self.kind(*io) {
                        NodeKind::FuncArg { ty, .. } => IrFormal {
                            is_out: *is_out,
                            width: ty.width.unwrap_or(0),
                            signed: ty.signed,
                        },
                        _ => unreachable!("formal kind"),
                    })
                    .collect();
                self.model.funcs.push(crate::sim::ir::IrFunc {
                    c_name,
                    ret: ret.map(|(w, s)| IrType::Packed {
                        width: w,
                        signed: s,
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
    /// Delay-bearing tasks are inlined at their call sites and never get a C
    /// function body.
    pub(super) fn emit_func_bodies(&mut self, inst: NodeId) -> Result<(), String> {
        for c in &self.node(inst).children {
            if let NodeKind::FuncTask { is_task, .. } = self.kind(*c) {
                if *is_task && self.task_has_wait(*c, inst) {
                    continue;
                }
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
    fn func_signature(&self, ft: NodeId) -> Result<(String, Vec<(NodeId, bool)>), String> {
        let (is_task, ret, formals) = self.func_info(ft)?;
        let c_name = self
            .func_names
            .get(&ft)
            .cloned()
            .ok_or_else(|| format!("function `{}` has no C name", self.node(ft).name))?;
        let ret_t = if is_task || ret.is_none() {
            "void"
        } else {
            "sv4_t"
        };
        let mut params = Vec::new();
        // Tasks: outputs first, then inputs.  The parameter names (`o{idx}` /
        // `a{idx}`) use the formal's index in the formals list, matching the
        // `arg_read`/`arg_write` maps built when emitting the body.
        for (idx, (_, is_out)) in formals.iter().enumerate() {
            if *is_out {
                params.push(format!("sv4_t* o{idx}"));
            }
        }
        for (idx, (_, is_out)) in formals.iter().enumerate() {
            if !*is_out {
                params.push(format!("sv4_t a{idx}"));
            }
        }
        params.push("int depth".to_string());
        Ok((
            format!("static {ret_t} {c_name}({}", params.join(", ")),
            formals,
        ))
    }

    /// `(is_task, return width/signed, (io_decl node, is_output) in formal
    /// order)` of a FuncTask node.  The return width is `None` for void
    /// functions and tasks.
    // The tuple mirrors UHDM's function/task signature without introducing a
    // public one-off type solely for this private lowering boundary.
    #[allow(clippy::type_complexity)]
    pub(super) fn func_info(
        &self,
        ft: NodeId,
    ) -> Result<(bool, Option<(u32, bool)>, Vec<(NodeId, bool)>), String> {
        let (is_task, ret) = match self.kind(ft) {
            NodeKind::FuncTask { is_task, ret, .. } => (*is_task, ret.clone()),
            _ => return Err("non-FuncTask passed to func_info".to_string()),
        };
        let ret = match ret {
            Some(ty) => {
                if is_real_kind(&ty.kind) {
                    return Err(format!(
                        "real/shortreal function return `{}` is not supported in v1",
                        self.node(ft).name
                    ));
                }
                match ty.width {
                    Some(w) => {
                        if w > LLG_MAX_WIDTH {
                            return Err(format!(
                                "return type of `{}` is {w} bits wide; the v1 runtime \
                             supports at most {LLG_MAX_WIDTH}",
                                self.node(ft).name
                            ));
                        }
                        Some((w, ty.signed))
                    }
                    None => {
                        return Err(format!(
                            "return type of `{}` has no width",
                            self.node(ft).name
                        ))
                    }
                }
            }
            None => None,
        };
        let mut formals = Vec::new();
        for c in &self.node(ft).children {
            match self.kind(*c) {
                NodeKind::FuncArg { direction, ty, .. } => {
                    if is_real_kind(&ty.kind) {
                        return Err(
                            "real/shortreal function formal is not supported in v1".to_string()
                        );
                    }
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

    /// The body statement of a function/task definition: the last child that
    /// is not the return variable or a formal argument (children are laid out
    /// in fixed order — return var, formals, then the body).  An empty body is
    /// captured by the database walk as a `StmtKind::Empty` placeholder, so
    /// this always finds a body when the children exist.
    pub(super) fn func_body(&self, ft: NodeId) -> Option<NodeId> {
        self.node(ft).children.iter().rev().copied().find(|c| {
            !matches!(
                self.kind(*c),
                NodeKind::Var { .. } | NodeKind::FuncArg { .. }
            )
        })
    }

    /// Emit one static C function for a function/task definition.  The body
    /// statements are emitted with the io_decls mapped to the C parameters and
    /// the locals to C locals; the function-name variable maps to a local
    /// `_ret` that `return` reads.
    fn emit_func_task(&mut self, path: &str, inst: NodeId, ft: NodeId) -> Result<(), String> {
        let (is_task, ret, formals) = self.func_info(ft)?;
        let (decl, _) = self.func_signature(ft)?;
        let c_name = self
            .func_names
            .get(&ft)
            .cloned()
            .ok_or_else(|| format!("function `{}` has no C name", self.node(ft).name))?;
        let has_ret = ret.is_some();
        let ret_var = if has_ret {
            self.node(ft).children.first().copied()
        } else {
            None
        };
        let body = self
            .func_body(ft)
            .ok_or_else(|| format!("function `{}` without a body", self.node(ft).name))?;

        // The all-X return value used by the recursion guard.
        let ret_x = match ret {
            Some((w, s)) => format!("sv4_x({w}, {})", s as u8),
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

        let mut locals: HashMap<NodeId, (String, u32, bool)> = HashMap::new();
        let mut local_seq = 0usize;
        self.collect_func_locals(body, &mut locals, &mut local_seq, "")?;

        // Function-name return variable → `_ret` local.
        let ret_ctx = match (has_ret, ret_var) {
            (true, Some(rv)) => {
                let (w, s) = ret.expect("ret width known");
                Some(RetCtx {
                    c_name: "_ret".to_string(),
                    width: w,
                    signed: s,
                    node: Some(rv),
                })
            }
            _ => None,
        };

        let mut arg_read: HashMap<NodeId, ArgMap> = HashMap::new();
        let mut arg_ir: HashMap<NodeId, IrExpr> = HashMap::new();
        let mut arg_write: HashMap<NodeId, String> = HashMap::new();
        for (idx, (io, is_out)) in formals.iter().enumerate() {
            let (w, s) = match self.kind(*io) {
                NodeKind::FuncArg { ty, .. } => {
                    if is_real_kind(&ty.kind) {
                        return Err(
                            "real/shortreal function formal is not supported in v1".to_string()
                        );
                    }
                    match ty.width {
                        Some(w) if w <= LLG_MAX_WIDTH => (w, ty.signed),
                        Some(w) => {
                            return Err(format!(
                                "formal `{}` of `{c_name}` is {w} bits wide; the v1 runtime \
                             supports at most {LLG_MAX_WIDTH}",
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
                _ => unreachable!("formal kind"),
            };
            if *is_out {
                arg_write.insert(*io, format!("o{idx}"));
                arg_ir.insert(*io, formal_read_expr(idx, w, s));
                arg_read.insert(
                    *io,
                    ArgMap {
                        width: w,
                        signed: s,
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
                    },
                );
            }
        }

        let func_ctx = FuncCtx {
            name: self.node(ft).name.clone(),
            is_task,
            ret: ret_ctx.clone(),
            arg_read,
            arg_ir,
            arg_write,
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
        let (body_stmts, pre_fns) = {
            let mut ctx = EmitCtx::new(
                self,
                path.to_string(),
                inst,
                "depth + 1",
                Some(func_ctx),
                None,
                false,
            );
            let body_stmts = ctx.lower_stmt(body)?;
            let pre_fns = std::mem::take(&mut ctx.pre_fns);
            (body_stmts, pre_fns)
        };
        // Restore the process-level context for whatever is lowered next
        // (continuous assignments, processes).
        self.func = None;
        self.cur_fn_ir = None;
        self.depth_arg = "0".to_string();

        let ir_locals = {
            let mut names = locals.into_iter().collect::<Vec<_>>();
            names.sort_by_key(|(id, _)| id.0);
            names
                .into_iter()
                .map(|(_, (c_name, width, signed))| crate::sim::ir::IrLocal {
                    c_name,
                    width,
                    signed,
                })
                .collect()
        };
        let no_entry = format!("function `{}` has no model entry", self.node(ft).name);
        let entry = self.model.funcs.get_mut(meta_ir).ok_or(no_entry)?;
        entry.locals = ir_locals;
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
        locals: &mut HashMap<NodeId, (String, u32, bool)>,
        seq: &mut usize,
        prefix: &str,
    ) -> Result<(), String> {
        if let NodeKind::Var { ty } = self.kind(node) {
            if is_real_kind(&ty.kind) {
                return Err(format!(
                    "real/shortreal function local `{}` is not supported in v1",
                    self.node(node).name
                ));
            }
            let w = match ty.width {
                Some(w) if w <= LLG_MAX_WIDTH => w,
                Some(w) => {
                    return Err(format!(
                        "local `{}` is {w} bits wide; the v1 runtime supports at most \
                         {LLG_MAX_WIDTH}",
                        self.node(node).name
                    ))
                }
                None => return Err(format!("local `{}` has no width", self.node(node).name)),
            };
            let cname = format!("{prefix}_l{seq}");
            *seq += 1;
            locals.insert(node, (cname, w, ty.signed));
            return Ok(());
        }
        for c in &self.node(node).children {
            self.collect_func_locals(*c, locals, seq, prefix)?;
        }
        Ok(())
    }

    /// Resolve a call site's callee to its FuncTask arena node.  Prefers the
    /// captured `callee` (checked to live inside `inst`); falls back to a name
    /// lookup among the owning instance's function/task definitions.  Callees
    /// outside the instance (hierarchical calls) are rejected.
    pub(super) fn resolve_callee(
        &self,
        inst: NodeId,
        name: &str,
        is_task: bool,
        callee: Option<NodeId>,
    ) -> Result<NodeId, String> {
        if let Some(ft) = callee {
            let mut cur = self.node(ft).parent;
            while let Some(p) = cur {
                if p == inst {
                    return Ok(ft);
                }
                cur = self.node(p).parent;
            }
            return Err(format!(
                "hierarchical call `{name}` is not supported (callee outside \
                 the calling instance)"
            ));
        }
        for c in &self.node(inst).children {
            if let NodeKind::FuncTask { is_task: t, .. } = self.kind(*c) {
                if *t == is_task && self.node(*c).name == name {
                    return Ok(*c);
                }
            }
        }
        Err(format!(
            "cannot resolve callee `{name}` in `{}`",
            self.node(inst).name
        ))
    }

    /// Whether a task's execution can suspend: its body (transitively over
    /// called tasks) contains a delay, event control or wait statement.
    /// Delay-bearing tasks are inlined at their call sites; the others become
    /// plain C functions.
    pub(super) fn task_has_wait(&self, ft: NodeId, inst: NodeId) -> bool {
        let mut seen: HashSet<NodeId> = HashSet::new();
        self.task_has_wait_inner(ft, inst, &mut seen)
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

    fn node_has_wait(&self, node: NodeId, inst: NodeId, seen: &mut HashSet<NodeId>) -> bool {
        match self.kind(node) {
            NodeKind::Stmt(
                StmtKind::DelayControl { .. }
                | StmtKind::EventControl { .. }
                | StmtKind::Wait { .. },
            ) => true,
            NodeKind::FuncCall {
                is_task: true,
                callee,
                ..
            } => {
                if let Ok(ft) = self.resolve_callee(inst, &self.node(node).name, true, *callee) {
                    if self.task_has_wait_inner(ft, inst, seen) {
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

    /// A call argument Surelog synthesizes for a *missing named* argument: a
    /// location-less `0` constant (genuine `0` literals carry a source line).
    fn is_synthetic_arg(&self, a: NodeId) -> bool {
        if self.node(a).line != 0 {
            return false;
        }
        match self.kind(a) {
            NodeKind::Expr(ExprKind::Constant { value, .. }) => matches!(
                value,
                ValueData::Int(0) | ValueData::UInt(0) | ValueData::Scalar(vpi::vpi0)
            ),
            _ => false,
        }
    }

    /// Bind a call's positional arguments to the callee's formals, in formal
    /// order.  Missing (or Surelog-synthesized) arguments fall back to the
    /// formal's default expression; a formal without a default errors.
    pub(super) fn bind_call_args(
        &self,
        formals: &[(NodeId, bool)],
        args: &[NodeId],
    ) -> Result<Vec<BoundArg>, String> {
        let mut bound = Vec::with_capacity(formals.len());
        for (idx, (io, _)) in formals.iter().enumerate() {
            let (w, s) = match self.kind(*io) {
                NodeKind::FuncArg { ty, .. } => {
                    if is_real_kind(&ty.kind) {
                        return Err(
                            "real/shortreal function formal is not supported in v1".to_string()
                        );
                    }
                    match ty.width {
                        Some(w) if w <= LLG_MAX_WIDTH => (w, ty.signed),
                        Some(w) => {
                            return Err(format!(
                                "formal `{}` is {w} bits wide; the v1 runtime supports \
                             at most {LLG_MAX_WIDTH}",
                                self.node(*io).name
                            ))
                        }
                        None => {
                            return Err(format!("formal `{}` has no width", self.node(*io).name))
                        }
                    }
                }
                _ => unreachable!("non-FuncArg in formals"),
            };
            let (expr, is_default) = match args.get(idx) {
                Some(a) if !self.is_synthetic_arg(*a) => (*a, false),
                _ => match self.kind(*io) {
                    NodeKind::FuncArg { default, .. } => (
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
                    _ => unreachable!(),
                },
            };
            bound.push(BoundArg {
                width: w,
                signed: s,
                expr,
                is_default,
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
        let (w, s) = (bound[idx].width, bound[idx].signed);
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
                arg_write: HashMap::new(),
                locals: HashMap::new(),
                ret_node: None,
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
        let conv_ir = ir_to_vector(e_ir, w, s)?;
        let code = self.render_ir_code(&conv_ir)?;
        arg_codes[idx] = Some(code.clone());
        if arg_irs.len() <= idx {
            arg_irs.resize(idx + 1, None);
        }
        arg_irs[idx] = Some(conv_ir.clone());
        Ok((code, conv_ir))
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
        let ft = self.resolve_callee(self.inst, name, false, callee)?;
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
        let bound = self.bind_call_args(&formals, &args)?;
        // `ret` is `None` for void functions; using one as a value (legal in
        // Surelog's parse, e.g. `out <= vf(4'd2);`) emits the call for its
        // side effects and yields all-X.
        let ret_val = meta.ret;
        let (ret_w, ret_s) = ret_val.unwrap_or((1, false));

        let mut out_args: Vec<IrCallArg> = Vec::new();
        let mut in_args: Vec<IrCallArg> = Vec::new();
        let mut arg_codes: Vec<Option<String>> = vec![None; formals.len()];
        let mut arg_irs: Vec<Option<IrExpr>> = vec![None; formals.len()];
        for (idx, (io, is_out)) in formals.iter().enumerate() {
            if *is_out {
                let tname = format!("_t{}", h.0);
                let (_init_code, init_ir) =
                    self.lower_call_temp_init(scope_path, *io, &bound[idx])?;
                let wb = self.lower_lhs(scope_path, bound[idx].expr)?;
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
                });
            }
        }
        for (idx, (_, is_out)) in formals.iter().enumerate() {
            if !*is_out {
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

    /// Lower the initializer of the caller-side temp for an output/inout
    /// formal: all-X (`None`) for outputs, the actual's current value
    /// (converted to the formal's vector shape) for inouts.  Also returns the
    /// rendered initializer for the legacy string paths.
    pub(super) fn lower_call_temp_init(
        &mut self,
        scope_path: &str,
        io: NodeId,
        b: &BoundArg,
    ) -> Result<(String, Option<IrExpr>), String> {
        match self.kind(io) {
            NodeKind::FuncArg {
                direction: DbDirection::Inout,
                ..
            } => {
                let e = self.lower_expr(scope_path, b.expr)?;
                let e = apply_assignment_expression_width(e, b.width);
                let conv = ir_to_vector(e, b.width, b.signed)?;
                let code = self.render_ir_code(&conv)?;
                Ok((code, Some(conv)))
            }
            _ => Ok((format!("sv4_x({}, {})", b.width, b.signed as u8), None)),
        }
    }

    /// Resolve a function/task body write target (output/inout formal, local
    /// or return variable) to an LHS.  Tries `node` first (locals and the
    /// return var are indexed), then `name`.
    fn func_write_target(&self, node: NodeId, name: &str) -> Option<Lhs> {
        let f = self.func.as_ref()?;
        if let Some(addr) = f.arg_write.get(&node) {
            if let Some(am) = f.arg_read.get(&node) {
                return Some(Lhs::WholeRef {
                    addr: addr.clone(),
                    width: am.width,
                    signed: am.signed,
                });
            }
        }
        if let Some((cname, w, s)) = f.locals.get(&node) {
            return Some(Lhs::WholeRef {
                addr: format!("&{cname}"),
                width: *w,
                signed: *s,
            });
        }
        if f.ret_node == Some(node) {
            if let Some(r) = &f.ret {
                return Some(Lhs::WholeRef {
                    addr: format!("&{}", r.c_name),
                    width: r.width,
                    signed: r.signed,
                });
            }
        }
        for (io, addr) in &f.arg_write {
            if self.node(*io).name == name {
                if let Some(am) = f.arg_read.get(io) {
                    return Some(Lhs::WholeRef {
                        addr: addr.clone(),
                        width: am.width,
                        signed: am.signed,
                    });
                }
            }
        }
        for (nid, (cname, w, s)) in &f.locals {
            if self.node(*nid).name == name {
                return Some(Lhs::WholeRef {
                    addr: format!("&{cname}"),
                    width: *w,
                    signed: *s,
                });
            }
        }
        if let Some(r) = &f.ret {
            if r.node.map(|n| self.node(n).name == name).unwrap_or(false) {
                return Some(Lhs::WholeRef {
                    addr: format!("&{}", r.c_name),
                    width: r.width,
                    signed: r.signed,
                });
            }
        }
        None
    }

    // ── PCA site pre-scan (two-phase discovery, phase 1) ─────────────────────

    /// Allocate every procedural continuous assignment site in the instance
    /// tree BEFORE any body lowers.  Traverses module instances, generate
    /// scopes and per-iteration instances exactly like the Procs emission
    /// pass (interface copies excluded — they never emit processes), so
    /// lower-time lookups into [`Codegen::pca_sites`] see every site
    /// regardless of process/source order: a `deassign` in a process that
    /// lowers BEFORE the process carrying the matching `assign` must still
    /// clear its enable (a lower-time allocation alone would turn it into a
    /// permanent no-op), and the multiple-active-sites reject becomes
    /// order-independent too.  Function/task definition bodies are not
    /// scanned here: they always lower before any process body, so sites
    /// inside them still allocate ahead of every process-body deassign.
    pub(super) fn prescan_pca_sites(&mut self, inst: NodeId, path: &str) -> Result<(), String> {
        let iface_copy = matches!(
            self.kind(inst),
            NodeKind::ModuleInst {
                is_interface: true,
                ..
            }
        ) && self.iface_copy_insts.contains(&inst);
        if !iface_copy {
            for c in &self.node(inst).children {
                if matches!(self.kind(*c), NodeKind::Process { .. }) {
                    self.prescan_pca_proc(inst, path, *c)?;
                }
            }
            for c in &self.node(inst).children {
                if !matches!(self.kind(*c), NodeKind::GenScopeArray) {
                    continue;
                }
                for gs in &self.node(*c).children {
                    if !matches!(self.kind(*gs), NodeKind::GenScope) {
                        continue;
                    }
                    let gs_path = self
                        .gen_scope_paths
                        .get(gs)
                        .cloned()
                        .unwrap_or_else(|| path.to_string());
                    for cc in &self.node(*gs).children {
                        match self.kind(*cc) {
                            NodeKind::Process { .. } => {
                                self.prescan_pca_proc(inst, &gs_path, *cc)?
                            }
                            // Per-iteration instances under a gen scope own
                            // their processes; recurse like the Procs pass.
                            NodeKind::ModuleInst { .. } => {
                                let child_path = self.instance_path_of(*cc);
                                self.prescan_pca_sites(*cc, &child_path)?;
                            }
                            _ => {}
                        }
                    }
                }
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
        // Instances inside generate scopes (per-iteration instances) are
        // emitted like the instance's own children: their links, processes and
        // continuous assignments all run under the gen-scope path.
        let emit_gen_scope_children =
            |cg: &mut Self, gs: &NodeId, pass: Pass, path: &str| -> Result<(), String> {
                let gs_path = cg
                    .gen_scope_paths
                    .get(gs)
                    .cloned()
                    .unwrap_or_else(|| path.to_string());
                for cc in &cg.node(*gs).children {
                    match cg.kind(*cc) {
                        NodeKind::ContAssign { .. } if pass == Pass::Comb => {
                            cg.emit_cont_assign(inst, &gs_path, *cc)?
                        }
                        NodeKind::Gate { .. } if pass == Pass::Comb => {
                            cg.emit_gate(inst, &gs_path, *cc)?
                        }
                        NodeKind::Process { .. } if pass == Pass::Procs => {
                            cg.emit_process(inst, &gs_path, *cc)?;
                        }
                        NodeKind::ModuleInst { .. } => {
                            let child_path = cg.instance_path_of(*cc);
                            match pass {
                                Pass::Comb => cg.emit_pass_inst(*cc, &child_path, Pass::Comb)?,
                                Pass::Links => {
                                    cg.emit_links(&gs_path, *cc)?;
                                    cg.emit_pass_inst(*cc, &child_path, Pass::Links)?;
                                }
                                Pass::Procs => cg.emit_pass_inst(*cc, &child_path, Pass::Procs)?,
                            }
                        }
                        _ => {}
                    }
                }
                Ok(())
            };
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
                    if matches!(self.kind(*c), NodeKind::GenScopeArray) {
                        for gs in &self.node(*c).children {
                            if matches!(self.kind(*gs), NodeKind::GenScope) {
                                emit_gen_scope_children(self, gs, Pass::Comb, path)?;
                            }
                        }
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
                    if matches!(self.kind(*c), NodeKind::GenScopeArray) {
                        for gs in &self.node(*c).children {
                            if matches!(self.kind(*gs), NodeKind::GenScope) {
                                emit_gen_scope_children(self, gs, Pass::Links, path)?;
                            }
                        }
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
                // Interface body processes (always/initial/always_comb blocks
                // inside an interface definition) belong to the ACTUAL
                // interface instance.  Surelog v1.86 does not clone them into
                // the per-port copies (they are just views); emitting one on a
                // copy would double-drive the member through the interface
                // link, so copies are always skipped here.
                let iface_copy = matches!(
                    self.kind(inst),
                    NodeKind::ModuleInst {
                        is_interface: true,
                        ..
                    }
                ) && self.iface_copy_insts.contains(&inst);
                for c in &self.node(inst).children {
                    if matches!(self.kind(*c), NodeKind::Process { .. }) {
                        if iface_copy {
                            continue;
                        }
                        self.emit_process(inst, path, *c)?;
                    }
                }
                // Processes inside generate scopes are emitted exactly like
                // instance processes (mirroring the Comb pass's gen-scope
                // walk); genvar references inline to the gen-scope parameter
                // values collected by `collect_gen_scope`.
                if !iface_copy {
                    for c in &self.node(inst).children {
                        if matches!(self.kind(*c), NodeKind::GenScopeArray) {
                            for gs in &self.node(*c).children {
                                if matches!(self.kind(*gs), NodeKind::GenScope) {
                                    emit_gen_scope_children(self, gs, Pass::Procs, path)?;
                                }
                            }
                        }
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

    // ── Continuous assignments ─────────────────────────────────────────────

    fn emit_cont_assign(&mut self, inst: NodeId, path: &str, ca: NodeId) -> Result<(), String> {
        let node = self.node(ca);
        if let NodeKind::ContAssign {
            net_decl: true,
            delay: _,
        } = self.kind(ca)
        {
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
                         {net_type} (only wire/tri/logic nets are supported)"
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
        if matches!(self.kind(ca), NodeKind::ContAssign { net_decl: true, .. })
            && self.contains_unpacked_array(rhs, &mut HashSet::new())
        {
            return Err(format!(
                "net declaration assignment in `{path}` reads an unpacked array, whose \
                 continuous sensitivity cannot be represented"
            ));
        }
        let lh = self.lower_lhs(path, lhs)?;
        let rhs_ir = self.lower_expr(path, rhs)?;
        let rhs_ir = apply_lhs_assignment_context(&self.model, &lh, rhs_ir);
        let lhs_real = matches!(&lh, IrLhs::Whole(idx)
            if matches!(self.model.signal(*idx).ty, IrType::Real { .. }));
        if lhs_real || rhs_ir.is_real() {
            return Err(format!(
                "real-valued continuous assignments are not supported in `{path}`"
            ));
        }
        let assign = IrStmt::Assign {
            lhs: lh,
            rhs: rhs_ir,
            nba: false,
        };
        // `assign #d lhs = rhs;` — the delay folds to a constant through the
        // collected parameter values and scales like a `#N` statement.  The
        // write happens D after each RHS change; a change during an open
        // window is picked up by the next iteration (no pulse filtering, see
        // below), and the t=0 first evaluation waits D too.
        let scaled_delay = match self.kind(ca) {
            NodeKind::ContAssign {
                delay: Some(de), ..
            } => {
                let dir = self.lower_expr(path, *de)?;
                if dir.is_real() {
                    return Err(format!(
                        "real-valued continuous-assignment delays are not \
                         supported in `{path}`"
                    ));
                }
                let raw = match dir.kind {
                    IrExprKind::Const(c) => const_delay_ticks(&c, path)?,
                    _ => {
                        return Err(format!(
                            "continuous-assignment delay must be a constant or \
                             parameter in `{path}`"
                        ))
                    }
                };
                let unit_ps = self.timescale_of_node(ca).unit_ps;
                Some(scale_delay_ticks(
                    raw,
                    unit_ps,
                    self.design_precision_ps,
                    path,
                )?)
            }
            _ => None,
        };
        // v1 approximation (LRM 1364-1995 §6.1.3): no pulse filtering — the
        // LHS is written with the CURRENT rhs value D after the wake, so an
        // rhs pulse shorter than D still produces a (delayed) write with the
        // post-pulse value instead of being swallowed.
        let mut body = Vec::new();
        if let Some(ticks) = scaled_delay {
            self.warnings.push(format!(
                "delayed continuous assignment in `{path}` uses the current \
                 rhs value after the #delay window (no pulse filtering)"
            ));
            body.push(IrStmt::Delay { ticks });
        }
        body.push(assign);
        let fn_name = self.new_fn_name(path, "ca");
        let sigs = self.collect_read_signals(path, rhs)?;
        let shape = if sigs.is_empty() {
            // Constant driver: evaluate once at t=0, then end (the value can
            // never change, so there is nothing to wait on).
            IrShape::RunOnce
        } else {
            IrShape::SensLoop { reads: sigs }
        };
        self.model.processes.push(IrProcess {
            c_name: fn_name,
            label: format!("{path}.assign"),
            shape,
            pre_fns: Vec::new(),
            body,
        });
        Ok(())
    }

    /// Whether an expression (including a called function body) reaches an
    /// unpacked array. Array element storage is not representable in an
    /// `IrShape::SensLoop` read set, so declaration drivers reject it rather
    /// than becoming stale after the first evaluation.
    fn contains_unpacked_array(&self, node: NodeId, visited: &mut HashSet<NodeId>) -> bool {
        if !visited.insert(node) {
            return false;
        }
        match self.kind(node) {
            NodeKind::Array { .. } | NodeKind::Expr(ExprKind::ArraySelect { .. }) => return true,
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) if matches!(self.kind(*target), NodeKind::Array { .. }) => {
                return true;
            }
            NodeKind::FuncCall {
                name,
                is_task,
                callee,
            } => {
                if let Ok(func) = self.resolve_callee(self.inst, name, *is_task, *callee) {
                    if let Some(body) = self.func_body(func) {
                        if self.contains_unpacked_array(body, visited) {
                            return true;
                        }
                    }
                }
            }
            _ => {}
        }
        self.node(node)
            .children
            .iter()
            .any(|child| self.contains_unpacked_array(*child, visited))
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
            },
            net_driver: None,
            omit: false,
        });
        ir
    }

    /// The multiple-active-sites reject, shared by the pre-scan claimer and
    /// lower-time re-check so both produce the same message.
    pub(super) fn pca_multi_site_err(&self, lhs: NodeId, path: &str) -> String {
        format!(
            "variable `{}` in `{path}` already has a procedural continuous \
             assignment (multiple active PCA sites on one variable are not \
             supported)",
            self.node(lhs).name,
        )
    }

    pub(super) fn new_fn_name(&mut self, path: &str, kind: &str) -> String {
        let n = self.proc_seq;
        self.proc_seq += 1;
        format!("p_{}_{}_{}", ident(path), kind, n)
    }

    // ── Structural gate primitives ─────────────────────────────────────────

    /// Lower one structural primitive ([`NodeKind::Gate`]) into ONE comb
    /// process shaped exactly like a continuous assignment: evaluate at
    /// spawn, then re-evaluate whenever any input-terminal signal changes
    /// (`IrShape::SensLoop`; constant drivers like pullup/pulldown use
    /// `IrShape::RunOnce`).  The output terminal is written with a
    /// whole-signal blocking write (collapsed-net members go through their
    /// driver slot automatically).
    ///
    /// Multi-input logic gates reduce their inputs left-to-right with the
    /// two-input runtime op; nand/nor/xnor negate after the full reduce.
    /// A gate delay `#D` prepends a scaled wait to the process body (the
    /// t=0 first evaluation waits too) with no pulse filtering — each wake
    /// writes the CURRENT input values D later (warned, like delayed
    /// continuous assignments).  Unsupported primitives (switches, UDPs,
    /// arrays, strengths, multi-output buf/not, width mismatches, …) are
    /// rejected with explicit errors here at lowering time.
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
                     supported in v1"
                ))
            }
            PrimClass::Udp => {
                return Err(format!(
                    "user-defined primitive instance `{shown}` in `{path}` is not \
                     supported in v1"
                ))
            }
            PrimClass::Array => {
                return Err(format!(
                    "primitive array `{shown}` in `{path}` (a range on a gate or \
                     UDP instance) is not supported in v1"
                ))
            }
        }
        if strength0 != Strength::Unspecified || strength1 != Strength::Unspecified {
            return Err(format!(
                "drive-strength specification on gate `{shown}` in `{path}` is not \
                 supported in v1"
            ));
        }
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
                    "primitive type {prim_type} of `{shown}` in `{path}` is not \
                     supported in v1"
                ))
            }
        };
        if terms.len() > LLG_MAX_GATE_TERMS {
            return Err(format!(
                "gate `{shown}` in `{path}` has {} terminals; at most \
                 {LLG_MAX_GATE_TERMS} are supported",
                terms.len()
            ));
        }
        // Resolve every terminal to its signal.  Terminals must be whole
        // plain signals (a ref to a net/var); select- or expression-
        // connected terminals are rejected cleanly in v1.
        let mut infos: Vec<SignalInfo> = Vec::new();
        for t in &terms {
            let whole = matches!(
                self.kind(t.expr),
                NodeKind::Net { .. } | NodeKind::Var { .. } | NodeKind::Expr(ExprKind::Ref { .. })
            );
            if !whole {
                return Err(format!(
                    "terminal `{}` of gate `{shown}` in `{path}` is connected \
                     through a select/expression; gate terminals must be whole \
                     plain signals in v1",
                    self.node(t.expr).name
                ));
            }
            let (_, info) = self.resolve_signal_id(path, t.expr).map_err(|_| {
                format!(
                    "terminal `{}` of gate `{shown}` in `{path}` does not resolve to \
                     a plain signal",
                    self.node(t.expr).name
                )
            })?;
            infos.push(info);
        }
        if let Some(bad) = infos.iter().position(|i| i.real) {
            return Err(format!(
                "real-valued signal `{}` on terminal {} of gate `{shown}` in \
                 `{path}` is not supported",
                infos[bad].global, bad
            ));
        }
        let w = infos.first().map(|i| i.width).unwrap_or(0);
        if w == 0 {
            return Err(format!(
                "gate `{shown}` in `{path}` has a zero-width terminal"
            ));
        }
        if infos.iter().any(|i| i.width != w) {
            let widths = infos
                .iter()
                .map(|i| i.width.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(format!(
                "gate `{shown}` in `{path}` connects terminals of different widths \
                 ({widths}); v1 requires equal terminal widths (mixed widths are \
                 legal Verilog, but not supported here yet)"
            ));
        }
        // Terminal-count/direction validation per kind.  Directions come from
        // Surelog's per-term classification (terminal 0 = output, except
        // buf/not where all but the last are outputs).
        let out_positions: Vec<usize> = terms
            .iter()
            .zip(infos.iter())
            .enumerate()
            .filter(|(_, (t, _))| t.direction == vpi::vpiOutput)
            .map(|(i, _)| i)
            .collect();
        let shape_ok = match op {
            GateOp::Pull(_) => terms.len() == 1 && out_positions.len() == 1,
            GateOp::Copy | GateOp::Not => {
                // Multi-output buf/not forms are not supported in v1.
                terms.len() == 2 && out_positions.len() == 1
            }
            GateOp::Enable { .. } => terms.len() == 3 && out_positions.len() == 1,
            GateOp::Reduce(..) => terms.len() >= 2 && out_positions.len() == 1,
        };
        if !shape_ok {
            let what = match op {
                GateOp::Pull(_) => "pullup/pulldown instance takes exactly one output terminal",
                GateOp::Copy | GateOp::Not => {
                    "one-output `buf`/`not` gates take exactly one output and one \
                     input terminal (multi-output forms are not supported in v1)"
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
        let out_pos = out_positions[0];
        let in_positions: Vec<usize> = (0..terms.len()).filter(|i| *i != out_pos).collect();
        // Callee resolution in the LHS/inputs needs the owning instance.
        self.inst = inst;
        // The output write goes through the same LHS machinery as a
        // continuous assignment (collapsed-net members lower to their driver
        // slot); select/hierarchical outputs stay unsupported in v1.
        let out_ir = match self.lower_lhs(path, terms[out_pos].expr)? {
            IrLhs::Whole(idx) => idx,
            _ => {
                return Err(format!(
                    "output terminal of gate `{shown}` in `{path}` must be connected \
                     to a whole plain signal (select/hierarchical gate outputs are \
                     not supported in v1)"
                ))
            }
        };
        // Input expressions + sensitivity set (input base signals only — the
        // LHS never triggers its own comb process).
        let mut in_exprs: Vec<IrExpr> = Vec::new();
        let mut sens: Vec<String> = Vec::new();
        for i in &in_positions {
            let e = self.lower_expr(path, terms[*i].expr)?;
            in_exprs.push(e);
            let global = &infos[*i].global;
            if !sens.contains(global) {
                sens.push(global.clone());
            }
        }
        // Output value computation (vector-wise over the common width).
        let value = match op {
            GateOp::Pull(ones) => const_bits_expr(w, ones),
            GateOp::Copy => in_exprs.remove(0),
            GateOp::Not => bitneg_full_width(in_exprs.remove(0)),
            GateOp::Reduce(bop, neg) => {
                let mut acc = in_exprs.remove(0);
                for next in in_exprs.drain(..) {
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
                let data = in_exprs.remove(0);
                let en = in_exprs.remove(0);
                // LRM 1364-1995 §7.4 Table 7-5: an ENABLED enable gate acts
                // like `buf`/`not` (Tables 7-3/7-4), so a data Z must reach
                // the output as X while known bits pass unchanged.  The mux
                // is an identity/copy context that returns its chosen arm
                // verbatim (`sv4_mux` with a known select), so the Z→X
                // normalization is done explicitly with `data|data`.
                // Correctness proof from llg_rt.c `sv4_bitwise` (op = OR),
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
                let z = const_z_expr(w);
                let (a, b) = if active_high { (data, z) } else { (z, data) };
                IrExpr::new(
                    IrExprKind::Mux {
                        sel: Box::new(en),
                        a: Box::new(a),
                        b: Box::new(b),
                    },
                    w,
                    false,
                    None,
                )
            }
        };
        // Gate delay `#D`: folded through the parameter values like a
        // continuous-assignment delay and scaled to design-precision ticks.
        let scaled_delay = match delay {
            Some(de) => {
                let dir = self.lower_expr(path, de)?;
                if dir.is_real() {
                    return Err(format!(
                        "real-valued gate delays are not supported in `{path}`"
                    ));
                }
                let raw = match dir.kind {
                    IrExprKind::Const(c) => const_delay_ticks(&c, path)?,
                    _ => {
                        return Err(format!(
                            "gate delay must be a constant or parameter in `{path}`"
                        ))
                    }
                };
                let unit_ps = self.timescale_of_node(g).unit_ps;
                Some(scale_delay_ticks(
                    raw,
                    unit_ps,
                    self.design_precision_ps,
                    path,
                )?)
            }
            None => None,
        };
        let mut body = Vec::new();
        if let Some(ticks) = scaled_delay {
            self.warnings.push(format!(
                "delayed gate `{shown}` in `{path}` uses the current input values \
                 after the #delay window (no pulse filtering)"
            ));
            body.push(IrStmt::Delay { ticks });
        }
        body.push(IrStmt::Assign {
            lhs: IrLhs::Whole(out_ir),
            rhs: value,
            nba: false,
        });
        let shape = if sens.is_empty() {
            // Constant driver (pullup/pulldown): evaluate once at t=0.
            IrShape::RunOnce
        } else {
            IrShape::SensLoop { reads: sens }
        };
        let fn_name = self.new_fn_name(path, "gate");
        self.model.processes.push(IrProcess {
            c_name: fn_name,
            label: format!("{path}.{shown}"),
            shape,
            pre_fns: Vec::new(),
            body,
        });
        Ok(())
    }

    // ── Port links ─────────────────────────────────────────────────────────

    fn emit_links(&mut self, parent_path: &str, child_inst: NodeId) -> Result<(), String> {
        let child_path = self.instance_path_of(child_inst);
        for c in &self.node(child_inst).children {
            let port = *c;
            let (direction, high, low) = match self.kind(port) {
                NodeKind::Port {
                    direction,
                    high,
                    low,
                    ..
                } => (*direction, *high, *low),
                _ => continue,
            };
            // Interface ports are wired through dedicated link processes; the
            // plain signal link machinery does not apply.
            if let Some((actual, modport)) =
                self.node(port)
                    .children
                    .iter()
                    .find_map(|cc| match self.kind(*cc) {
                        NodeKind::IfaceConn { actual, modport } => Some((*actual, modport.clone())),
                        _ => None,
                    })
            {
                self.emit_iface_link(parent_path, port, actual, &modport, &child_path)?;
                continue;
            }
            if direction == DbDirection::Inout {
                // The collapsed net group IS the connection; no link is
                // emitted.  Groups that could not be formed were already
                // warned about by `build_net_groups`.
                continue;
            }
            // Top-level ports have no parent side; nothing to link.
            let Some(hc) = high else { continue };
            let Some(lc) = low else { continue };
            let parent_side = self.link_parent_side(parent_path, port, hc)?;
            let (child_name, child_info) = self.resolve_signal_id(&child_path, lc)?;
            // A link touching a collapsed-net member would copy through the
            // resolution cell (or write it directly); the group itself is the
            // connection, so such links are skipped with a warning.
            let parent_member = match &parent_side {
                LinkSide::Signal(info) => info.net_driver.is_some(),
                LinkSide::ArrayElem(..) => false,
            };
            if parent_member || child_info.net_driver.is_some() {
                self.warnings.push(format!(
                    "port `{}` of `{child_path}` links a collapsed inout-net \
                     member; link skipped (the net group resolves the \
                     connection)",
                    self.node(port).name
                ));
                continue;
            }
            // The source (whose changes re-copy) and its width/signedness.
            let is_input = direction == DbDirection::Input;
            let (write, wait_sig) = if is_input {
                let (src_expr, wait_sig) = match &parent_side {
                    LinkSide::Signal(pinfo) => (sig_read_expr_full(pinfo), pinfo.global.clone()),
                    LinkSide::ArrayElem(ai, sel) => {
                        let e = self.lower_expr(parent_path, *sel)?;
                        (e, self.array_elem_addr(ai, *sel)?)
                    }
                };
                let write = IrStmt::Assign {
                    lhs: IrLhs::Whole(child_info.ir),
                    rhs: src_expr,
                    nba: false,
                };
                (write, wait_sig)
            } else {
                let src_expr = sig_read_expr_full(&child_info);
                let write = match &parent_side {
                    LinkSide::Signal(pinfo) => IrStmt::Assign {
                        lhs: IrLhs::Whole(pinfo.ir),
                        rhs: src_expr,
                        nba: false,
                    },
                    LinkSide::ArrayElem(ai, sel) => {
                        let lh = self.lower_lhs(parent_path, *sel)?;
                        IrStmt::Assign {
                            lhs: lh,
                            rhs: IrExpr::new(
                                IrExprKind::Verbatim {
                                    code: child_name.clone(),
                                    width: ai.elem_width,
                                    signed: ai.signed,
                                },
                                ai.elem_width,
                                ai.signed,
                                None,
                            ),
                            nba: false,
                        }
                    }
                };
                (write, child_name.clone())
            };
            let fn_name = self.new_fn_name(parent_path, "link");
            self.model.processes.push(IrProcess {
                c_name: fn_name,
                label: format!("{child_path}.link"),
                shape: IrShape::SensLoop {
                    reads: vec![wait_sig],
                },
                pre_fns: Vec::new(),
                body: vec![write],
            });
        }
        Ok(())
    }

    /// Resolve the parent side of a port connection (the `vpiHighConn`): a
    /// plain global signal, or an element of an unpacked array (when the
    /// connection selects into one — e.g. `.cnt(cnts[i])`, whose index
    /// expression the db walk captured as a child of the port).
    fn link_parent_side(
        &self,
        parent_path: &str,
        port: NodeId,
        hc: NodeId,
    ) -> Result<LinkSide, String> {
        if let Some(ai) = self.array_of(hc).cloned() {
            let sel = self.node(port).children.iter().find_map(|c| {
                matches!(
                    self.kind(*c),
                    NodeKind::Expr(
                        ExprKind::BitSelect { .. }
                            | ExprKind::PartSelect { .. }
                            | ExprKind::IndexedPartSelect { .. }
                            | ExprKind::ArraySelect { .. }
                    )
                )
                .then_some(*c)
            });
            return match sel {
                Some(s) => Ok(LinkSide::ArrayElem(ai, s)),
                None => Err(format!(
                    "array-element port connection in `{parent_path}` is missing its \
                     index expression"
                )),
            };
        }
        let (_, info) = self.resolve_signal_id(parent_path, hc)?;
        Ok(LinkSide::Signal(info))
    }

    /// C address (without the leading `&`) of the array element addressed by a
    /// constant-index select expression (`cnts[i]` → `G_tb_cnts[(0)]`), used
    /// as the wait source of an input-port link into an array element.
    /// Dynamic (non-constant) array-element port connections are not
    /// supported.
    fn array_elem_addr(&self, ai: &ArrayInfo, sel: NodeId) -> Result<String, String> {
        let idx = match self.kind(sel) {
            NodeKind::Expr(ExprKind::BitSelect { index, .. }) => self.eval_bound_i128(*index)?,
            NodeKind::Expr(ExprKind::IndexedPartSelect { base_expr, .. }) => {
                self.eval_bound_i128(*base_expr)?
            }
            NodeKind::Expr(ExprKind::PartSelect { left, .. }) => self.eval_bound_i128(*left)?,
            NodeKind::Expr(ExprKind::ArraySelect { indices, .. }) if indices.len() == 1 => {
                self.eval_bound_i128(indices[0])?
            }
            _ => {
                return Err(
                    "array-element port connection with a non-constant index is not \
                     supported"
                        .to_string(),
                )
            }
        };
        if ai.dims.len() != 1 {
            return Err(format!(
                "array-element port connection on a {}-dimensional array is not supported",
                ai.dims.len()
            ));
        }
        let (l, r) = ai.dims[0];
        let off = if l >= r {
            l as i128 - idx
        } else {
            idx - l as i128
        };
        Ok(format!("{}[({off})]", ai.global))
    }

    /// Emit the link processes wiring an interface port to its actual
    /// interface instance.  Values are copied between the per-port copy's
    /// vars and the actual interface's vars, matched by name:
    ///
    /// - modport ports: each io_decl is wired in its declared direction
    ///   (outputs flow child → actual, inputs flow actual → child);
    /// - bare interface ports: every member is wired bidirectionally (the
    ///   pair converges because same-value writes do not re-fire the wait).
    ///
    /// Every link is one process (initial copy at spawn, then `wait_any` on
    /// the source followed by a re-copy), mirroring the plain port links.
    fn emit_iface_link(
        &mut self,
        parent_path: &str,
        port: NodeId,
        actual_id: NodeId,
        modport: &str,
        child_path: &str,
    ) -> Result<(), String> {
        // The per-port copy inside the child: `low` resolves to the copy's
        // modport (modport ports) or to the copy interface instance itself
        // (bare interface ports).
        let low = match self.kind(port) {
            NodeKind::Port { low, .. } => *low,
            _ => return Ok(()),
        };
        let copy_id = match low.and_then(|l| match self.kind(l) {
            NodeKind::ModPort => self.node(l).parent,
            NodeKind::ModuleInst { .. } => Some(l),
            _ => None,
        }) {
            Some(c) => c,
            None => {
                self.warnings.push(format!(
                    "interface port `{}` of `{child_path}`: per-port copy not \
                     found; skipped",
                    self.node(port).name
                ));
                return Ok(());
            }
        };
        let actual_vars = self.collect_iface_vars(actual_id);
        let copy_vars = self.collect_iface_vars(copy_id);
        if actual_vars.is_empty() || copy_vars.is_empty() {
            self.warnings.push(format!(
                "interface port `{}` of `{child_path}`: no interface members \
                 found on the actual instance or the per-port copy; skipped",
                self.node(port).name
            ));
            return Ok(());
        }

        // (source global, source info, destination global)
        let mut links: Vec<(String, SignalInfo, String)> = Vec::new();
        if modport.is_empty() {
            // Bare interface port: bidirectional pair per member.
            for (name, cv) in &copy_vars {
                if let Some(av) = actual_vars.get(name) {
                    links.push((cv.global.clone(), cv.clone(), av.global.clone()));
                    links.push((av.global.clone(), av.clone(), cv.global.clone()));
                }
            }
        } else {
            let mp_node = self.node(copy_id).children.iter().find(|cc| {
                matches!(self.kind(**cc), NodeKind::ModPort) && self.node(**cc).name == modport
            });
            match mp_node {
                Some(mp) => {
                    for io in &self.node(*mp).children {
                        let (direction, expr) = match self.kind(*io) {
                            NodeKind::IoDecl { direction, expr } => (*direction, *expr),
                            _ => continue,
                        };
                        // The io_decl's expr resolves to the copy's own var.
                        let Some(cv) = expr.and_then(|e| self.signal_of(e)) else {
                            continue;
                        };
                        let name = self.node(*io).name.clone();
                        let Some(av) = actual_vars.get(&name) else {
                            continue;
                        };
                        match direction {
                            // Output: the child drives the actual member.
                            DbDirection::Output => {
                                links.push((cv.global.clone(), cv.clone(), av.global.clone()));
                            }
                            // Input: the child reads the actual member.
                            DbDirection::Input => {
                                links.push((av.global.clone(), av.clone(), cv.global.clone()));
                            }
                            _ => {}
                        }
                    }
                }
                None => {
                    self.warnings.push(format!(
                        "modport `{modport}` not found on the per-port copy of \
                         `{child_path}`; interface link skipped"
                    ));
                    return Ok(());
                }
            }
        }

        for (src, src_info, dst) in links {
            let dst_real = self
                .signals
                .iter()
                .any(|info| info.global == dst && info.real);
            if src_info.real || dst_real {
                return Err(format!(
                    "interface links involving real-valued member `{child_path}` are not supported"
                ));
            }
            let dst_ir = self
                .signals
                .iter()
                .find(|info| info.global == dst)
                .map(|info| info.ir)
                .ok_or_else(|| format!("interface link destination `{dst}` not collected"))?;
            let fn_name = self.new_fn_name(parent_path, "ilink");
            let write = IrStmt::Assign {
                lhs: IrLhs::Whole(dst_ir),
                rhs: sig_read_expr_full(&src_info),
                nba: false,
            };
            self.model.processes.push(IrProcess {
                c_name: fn_name,
                label: format!("{child_path}.ilink"),
                shape: IrShape::SensLoop { reads: vec![src] },
                pre_fns: Vec::new(),
                body: vec![write],
            });
            // Writing an actual member from a child drives it; several
            // children driving the same member is last-writer-wins.
            if !self.iface_driven.insert(dst.clone()) {
                self.warnings
                    .push(format!("multiple interface drivers on `{dst}`"));
            }
        }
        Ok(())
    }

    /// Name → lowered signal info for every Net/Var child of an interface
    /// instance node (the actual instance or a per-port copy).
    fn collect_iface_vars(&self, inst: NodeId) -> HashMap<String, SignalInfo> {
        let mut out = HashMap::new();
        for c in &self.node(inst).children {
            if matches!(self.kind(*c), NodeKind::Net { .. } | NodeKind::Var { .. }) {
                if let Some(info) = self.signal_of(*c) {
                    out.insert(self.node(*c).name.clone(), info.clone());
                }
            }
        }
        out
    }

    // ── Processes ──────────────────────────────────────────────────────────

    fn emit_process(&mut self, inst: NodeId, path: &str, proc: NodeId) -> Result<(), String> {
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
        let fn_name = self.new_fn_name(path, "proc");
        let (body_stmts, pre_fns, shape) = {
            let mut ctx = EmitCtx::new(self, path.to_string(), inst, "0", None, None, is_final);
            let body_stmts = ctx.lower_stmt(stmt)?;
            // Fork-branch coroutines and monitor/strobe evaluators attach to
            // the process (rendered ahead of it).
            let pre_fns = std::mem::take(&mut ctx.pre_fns);
            let kind_label = if is_initial { "initial" } else { "always" };
            let shape = if is_initial || is_final {
                // `initial` and `final` bodies run exactly once (finals after
                // the scheduler exits — the spawn phase is decided below).
                IrShape::RunOnce
            } else if !ctx.saw_wait {
                // always / always_comb / always_ff without any event/delay
                // control: a combinational process.
                if assigns_to_real(&body_stmts, &ctx.cg.model) {
                    return Err(format!("real-valued signals are not supported in combinational processes in `{path}`"));
                }
                // Run once at t=0, then re-run whenever a read signal changes.
                let sigs = ctx.cg.collect_read_signals(path, stmt)?;
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
            let _ = kind_label;
            (body_stmts, pre_fns, shape)
        };
        let kind_label = if is_initial {
            "initial"
        } else if is_final {
            "final"
        } else {
            "always"
        };
        self.model.processes.push(IrProcess {
            c_name: fn_name.clone(),
            label: format!("{path}.{kind_label}"),
            shape,
            pre_fns,
            body: body_stmts,
        });
        if is_final {
            self.final_procs.push(fn_name);
        }
        Ok(())
    }

    // ── Signal reads for sensitivity ───────────────────────────────────────

    /// Collect every signal read anywhere in the statement/expression tree
    /// rooted at `root`, as global names (deduped, deterministic order).
    /// Function/task calls descend into the callee bodies (guarded against
    /// recursion), so reads hidden behind a function call contribute to the
    /// sensitivity set.
    pub(super) fn collect_read_signals(
        &self,
        scope_path: &str,
        root: NodeId,
    ) -> Result<Vec<String>, String> {
        let mut out = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        let mut visited: HashSet<NodeId> = HashSet::new();
        self.walk_read_signals(scope_path, root, &mut seen, &mut visited, &mut out)?;
        if out.iter().any(|name| {
            self.signals
                .iter()
                .any(|info| info.real && info.global == name.as_str())
        }) {
            return Err(format!("real-valued signals cannot be used in a combinational sensitivity set in `{scope_path}`"));
        }
        Ok(out)
    }

    pub(super) fn walk_read_signals(
        &self,
        scope_path: &str,
        node: NodeId,
        seen: &mut HashSet<String>,
        visited: &mut HashSet<NodeId>,
        out: &mut Vec<String>,
    ) -> Result<(), String> {
        match self.kind(node) {
            NodeKind::Stmt(StmtKind::Assign { .. })
            | NodeKind::Stmt(StmtKind::ProcContAssign { .. }) => {
                // Sensitivity of a process body: an assignment's LHS base
                // signal must NOT trigger the process (it would self-wake
                // after every write — including the dedicated PCA guard
                // process's writes).  Only the LHS's index/bounds
                // expressions are reads.
                if let Some(rhs) = self.node(node).children.get(1) {
                    self.walk_read_signals(scope_path, *rhs, seen, visited, out)?;
                }
                if let Some(lhs) = self.node(node).children.first() {
                    self.walk_lhs_select_reads(scope_path, *lhs, seen, visited, out)?;
                }
                return Ok(());
            }
            NodeKind::Stmt(StmtKind::For { cond, body, .. }) => {
                // The old RELS-based walk does not descend into the for
                // init/incr statements.
                self.walk_read_signals(scope_path, *cond, seen, visited, out)?;
                self.walk_read_signals(scope_path, *body, seen, visited, out)?;
                return Ok(());
            }
            NodeKind::Stmt(StmtKind::Fork { branches, .. }) => {
                // A fork body reads signals (a `fork … join` branch may block
                // on signals); descend into the branches for comb sensitivity.
                for b in branches {
                    self.walk_read_signals(scope_path, *b, seen, visited, out)?;
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
                if let Ok(ft) = self.resolve_callee(self.inst, name, *is_task, *callee) {
                    if visited.insert(ft) {
                        if let Some(body) = self.func_body(ft) {
                            self.walk_read_signals(scope_path, body, seen, visited, out)?;
                        }
                    }
                }
                for c in &self.node(node).children {
                    self.walk_read_signals(scope_path, *c, seen, visited, out)?;
                }
                return Ok(());
            }
            _ => {}
        }
        self.add_node_read(node, seen, out);
        for c in &self.node(node).children {
            self.walk_read_signals(scope_path, *c, seen, visited, out)?;
        }
        Ok(())
    }

    /// Walk only the index/bounds expressions of an assignment LHS.
    fn walk_lhs_select_reads(
        &self,
        scope_path: &str,
        lhs: NodeId,
        seen: &mut HashSet<String>,
        visited: &mut HashSet<NodeId>,
        out: &mut Vec<String>,
    ) -> Result<(), String> {
        match self.kind(lhs) {
            NodeKind::Expr(ExprKind::BitSelect { index, .. }) => {
                self.walk_read_signals(scope_path, *index, seen, visited, out)
            }
            NodeKind::Expr(ExprKind::PartSelect { left, right, .. }) => {
                self.walk_read_signals(scope_path, *left, seen, visited, out)?;
                self.walk_read_signals(scope_path, *right, seen, visited, out)
            }
            NodeKind::Expr(ExprKind::IndexedPartSelect {
                base_expr,
                width_expr,
                ..
            }) => {
                self.walk_read_signals(scope_path, *base_expr, seen, visited, out)?;
                self.walk_read_signals(scope_path, *width_expr, seen, visited, out)
            }
            NodeKind::Expr(ExprKind::ArraySelect { indices, .. }) => {
                // The base is the array itself (not a read); only the index
                // expressions (and any element-level select bounds) are reads.
                for i in indices {
                    self.walk_read_signals(scope_path, *i, seen, visited, out)?;
                }
                Ok(())
            }
            NodeKind::Expr(ExprKind::HierPath { .. }) => {
                // A hierarchical LHS base signal must not trigger the owning
                // process (same rule as a plain LHS ref).  v1 supports only
                // constant indices/bounds on hierarchical targets, so there
                // are no index/bounds reads to collect.
                Ok(())
            }
            _ => Ok(()), // plain ref LHS: not part of the read set
        }
    }

    /// Add `node` to the read set if it is (or resolves to) a signal.
    fn add_node_read(&self, node: NodeId, seen: &mut HashSet<String>, out: &mut Vec<String>) {
        match self.kind(node) {
            NodeKind::Net { .. } | NodeKind::Var { .. } => {
                if let Some(info) = self.signal_of(node) {
                    if seen.insert(info.global.clone()) {
                        out.push(info.global.clone());
                    }
                }
            }
            NodeKind::Expr(ExprKind::Ref { target: Some(t) }) => {
                if let Some(info) = self.signal_of(*t) {
                    if seen.insert(info.global.clone()) {
                        out.push(info.global.clone());
                    }
                }
            }
            NodeKind::Expr(ExprKind::HierPath { .. }) => {
                if let Some(info) = self.hier_path_signal(node) {
                    if seen.insert(info.global.clone()) {
                        out.push(info.global.clone());
                    }
                }
            }
            _ => {}
        }
    }

    // ── Signal resolution ──────────────────────────────────────────────────

    /// The named-event arena node a walked operand resolves to, when it is a
    /// ref whose target is a captured [`NodeKind::NamedEvent`] (both the
    /// ref-wrapped and the direct object shapes normalize to `Ref`).
    pub(super) fn event_target_of(&self, node: NodeId) -> Option<NodeId> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target: Some(t) }) => {
                matches!(self.kind(*t), NodeKind::NamedEvent).then_some(*t)
            }
            _ => None,
        }
    }

    /// Model index of a captured named-event node.
    pub(super) fn event_index_of(&self, ev: NodeId, scope_path: &str) -> Result<usize, String> {
        self.event_globals.get(&ev).map(|i| i.ir).ok_or_else(|| {
            format!(
                "cannot resolve named event reference `{}` in `{scope_path}`",
                self.node(ev).name
            )
        })
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
        if !name.is_empty() {
            if let Some(info) = self
                .scope_sig_names
                .get(scope_path)
                .and_then(|m| m.get(&name))
            {
                return Ok((info.global.clone(), info.clone()));
            }
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

    /// Resolve a whole-signal assignment target (by arena node, then by name
    /// across every scope).
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
        for names in self.scope_sig_names.values() {
            if let Some(info) = names.get(&name) {
                return Ok((info.global.clone(), info.clone()));
            }
        }
        Err(format!("cannot resolve assignment target `{name}`"))
    }

    // ── LHS analysis ───────────────────────────────────────────────────────

    pub(super) fn analyze_lhs(&mut self, path: &str, lhs: NodeId) -> Result<Lhs, String> {
        match self.kind(lhs) {
            NodeKind::Var { .. } => {
                let info = self.proc_locals.get(&lhs).ok_or_else(|| {
                    format!(
                        "cannot resolve procedural variable `{}` in `{path}`",
                        self.node(lhs).name
                    )
                })?;
                Ok(Lhs::WholeRef {
                    addr: format!("&{}", info.c_name),
                    width: info.width,
                    signed: info.signed,
                })
            }
            NodeKind::Expr(ExprKind::Ref { target }) => {
                if let Some((_, info)) = self.lexical_proc_local(lhs) {
                    return Ok(Lhs::WholeRef {
                        addr: format!("&{}", info.c_name),
                        width: info.width,
                        signed: info.signed,
                    });
                }
                if let Some(t) = *target {
                    if let Some(info) = self.signal_of(t) {
                        return Ok(Lhs::Whole(info.clone()));
                    }
                    if !self.proc_local_is_shadowed(lhs) {
                        if let Some(info) = self.proc_locals.get(&t) {
                            return Ok(Lhs::WholeRef {
                                addr: format!("&{}", info.c_name),
                                width: info.width,
                                signed: info.signed,
                            });
                        }
                    }
                    // Function/task body writes: output/inout formals, locals
                    // and the return variable (by arena node).
                    if let Some(lh) = self.func_write_target(t, "") {
                        return Ok(lh);
                    }
                }
                let name = self.node(lhs).name.clone();
                if !name.is_empty() {
                    // io_decls are not indexed, so formals resolve by name.
                    if let Some(lh) = self.func_write_target(NodeId(0), &name) {
                        return Ok(lh);
                    }
                }
                let (name, info) = self.resolve_lhs_target(lhs, *target)?;
                Ok(Lhs::Whole(SignalInfo {
                    global: name,
                    ..info
                }))
            }
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                if let Some(ai) = self.array_of(*base).cloned() {
                    if ai.dims.len() != 1 {
                        return Err(format!(
                            "array slice access (`{}[...]` on a {}-dimensional array) \
                             is not supported in `{path}`",
                            self.node(*base).name,
                            ai.dims.len()
                        ));
                    }
                    let ie = self.emit_expr(path, *index)?;
                    return Ok(Lhs::ArrayElem(ArrayElemLhs {
                        arr: ai,
                        index_codes: vec![ie],
                        elem_sel: ElemSel::Whole,
                    }));
                }
                let (_, info) = self.base_signal(path, *base)?;
                if info.real {
                    return Err(format!(
                        "select on real-valued signal in `{path}` is not supported"
                    ));
                }
                let ie = self.emit_expr(path, *index)?;
                Ok(Lhs::Bit(info, ie))
            }
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                let ai = self.array_of(*base).cloned().ok_or_else(|| {
                    format!(
                        "cannot resolve array base of select `{}` in `{path}`",
                        self.node(*base).name
                    )
                })?;
                let ndims = ai.dims.len();
                if indices.len() == ndims {
                    let ies = indices
                        .iter()
                        .map(|i| self.emit_expr(path, *i))
                        .collect::<Result<Vec<_>, _>>()?;
                    return Ok(Lhs::ArrayElem(ArrayElemLhs {
                        arr: ai,
                        index_codes: ies,
                        elem_sel: ElemSel::Whole,
                    }));
                }
                if indices.len() == ndims + 1 {
                    let last = *indices.last().expect("non-empty indices");
                    let ies = indices[..ndims]
                        .iter()
                        .map(|i| self.emit_expr(path, *i))
                        .collect::<Result<Vec<_>, _>>()?;
                    let elem_sel = match self.kind(last) {
                        NodeKind::Expr(ExprKind::PartSelect { left, right, .. }) => {
                            let l = self.eval_bound_i128(*left)?;
                            let r = self.eval_bound_i128(*right)?;
                            ElemSel::Part(l, r)
                        }
                        NodeKind::Expr(ExprKind::IndexedPartSelect { .. }) => {
                            return Err(format!(
                                "indexed part-select on an array element is not \
                                 supported in `{path}`"
                            ))
                        }
                        _ => {
                            let ie = self.emit_expr(path, last)?;
                            ElemSel::Bit(ie)
                        }
                    };
                    return Ok(Lhs::ArrayElem(ArrayElemLhs {
                        arr: ai,
                        index_codes: ies,
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
                let (_, info) = self.base_signal(path, *base)?;
                if info.real {
                    return Err(format!(
                        "select on real-valued signal in `{path}` is not supported"
                    ));
                }
                let (l, r) = (self.eval_bound_i128(*left)?, self.eval_bound_i128(*right)?);
                Ok(Lhs::Part(info, l, r))
            }
            NodeKind::Expr(ExprKind::IndexedPartSelect {
                base,
                base_expr,
                width_expr,
                neg,
            }) => {
                let (_, info) = self.base_signal(path, *base)?;
                if info.real {
                    return Err(format!(
                        "select on real-valued signal in `{path}` is not supported"
                    ));
                }
                let be = self.emit_expr(path, *base_expr)?;
                let we = self.emit_expr(path, *width_expr)?;
                let neg = if *neg { 1 } else { 0 };
                Ok(Lhs::IdxPart(info, be, we, neg))
            }
            NodeKind::Expr(ExprKind::HierPath { .. }) => {
                // A whole-signal hierarchical WRITE (`m.data`, `tb.dut.sig`,
                // …) lowers to the resolved target signal's global, so
                // `llg_ba`/`llg_nba` (and the collapsed inout-net driver
                // path in `assign_statement`) apply unchanged.  A trailing
                // select on the target is recovered from the node name /
                // source line — Surelog v1.86's elaborated model drops
                // part-select bounds and only keeps constant bit-select
                // indices (in the object name); constant indices/bounds only
                // in v1.
                let info = self.hier_path_signal(lhs).cloned().ok_or_else(|| {
                    format!(
                        "cannot resolve hierarchical assignment LHS `{}` in \
                             `{path}` (only plain per-instance signals are \
                             supported)",
                        self.node(lhs).name
                    )
                })?;
                if let Some(sel) = self.hier_lhs_select(lhs)? {
                    return Ok(match sel {
                        HierSelect::Bit(idx) => Lhs::Bit(info, format!("SV4_C({idx}, 32)")),
                        HierSelect::Part(left, right) => Lhs::Part(info, left, right),
                        HierSelect::IdxPart(base, width, neg) => Lhs::IdxPart(
                            info,
                            format!("SV4_C({base}, 32)"),
                            format!("SV4_C({width}, 32)"),
                            if neg { 1 } else { 0 },
                        ),
                    });
                }
                Ok(Lhs::Whole(info))
            }
            _ => Err("unsupported assignment LHS".to_string()),
        }
    }

    /// Recover a trailing select on a hierarchical assignment target.
    ///
    /// Surelog v1.86's elaborated UHDM is lossy here: constant bit-select
    /// indices survive in the object's VPI name (`u.dut.sig[2]`), but
    /// part-select bounds (`u.dut.sig[3:0]`) are dropped entirely, so they
    /// are read back from the source line the node points at (the same
    /// recovery pattern as `#delay` ticks).  Only plain integer-literal
    /// indices/bounds are supported in v1; anything else is rejected with a
    /// clear error.  Returns `Ok(None)` when the target is a whole signal.
    fn hier_lhs_select(&self, lhs: NodeId) -> Result<Option<HierSelect>, String> {
        let name = self.node(lhs).name.clone();
        // Bit-selects keep their constant index in the VPI name.
        if let Some(inner) = name
            .strip_suffix(']')
            .and_then(|rest| rest.rfind('[').map(|i| &rest[i + 1..]))
        {
            if !inner.contains('[') {
                return hier_select_from_text(inner, &name).map(Some);
            }
        }
        // Part-selects (and anything else) are recovered from the source line.
        let file = self.node(lhs).file.clone().unwrap_or_default();
        let line = self.node(lhs).line;
        if file.is_empty() || line == 0 {
            return Err(format!(
                "cannot recover the select of hierarchical assignment LHS \
                 `{name}` (no source location)"
            ));
        }
        let content = std::fs::read_to_string(&file).map_err(|e| {
            format!(
                "cannot read `{file}` to recover the select of hierarchical \
                 assignment LHS `{name}`: {e}"
            )
        })?;
        let text = content.lines().nth(line as usize - 1).ok_or_else(|| {
            format!(
                "cannot read line {line} of `{file}` to recover the select of \
                 hierarchical assignment LHS `{name}`"
            )
        })?;
        let NodeKind::Expr(ExprKind::HierPath { parts, .. }) = self.kind(lhs) else {
            return Ok(None);
        };
        let path_text = parts.join(".");
        let mut search_from = 0usize;
        while let Some(rel) = text[search_from..].find(&path_text) {
            let pos = search_from + rel;
            let after = text[pos + path_text.len()..].trim_start();
            if after.starts_with('[') {
                let close = after.find(']').ok_or_else(|| {
                    format!(
                        "unterminated select on hierarchical assignment LHS \
                         `{name}` at {file}:{line}"
                    )
                })?;
                let inner = &after[1..close];
                if inner.contains('[') || inner.contains(']') {
                    return Err(format!(
                        "nested select on hierarchical assignment LHS `{name}` \
                         is not supported in v1"
                    ));
                }
                return hier_select_from_text(inner, &name).map(Some);
            }
            if after.starts_with('=') || after.starts_with('<') {
                return Ok(None); // whole-signal target
            }
            search_from = pos + path_text.len();
        }
        Err(format!(
            "cannot locate hierarchical assignment LHS `{name}` in `{file}` \
             line {line}"
        ))
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
    fn eval_bits(&self, node: NodeId) -> Result<elab::Value, String> {
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
            }) => self.eval_operation_bits(op.as_raw(), *reordered, operands),
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

    /// Evaluate the packed/real constants accepted in scalar declaration
    /// initializers.  This stays on the owned database and extends the
    /// integer-only bound evaluator only for conversion system functions.
    fn eval_decl_value(&self, node: NodeId) -> Result<Val, String> {
        if let Ok(bits) = self.eval_bits(node) {
            return Ok(Val::Bits(bits));
        }
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Constant { value, size, .. }) => {
                val_from_value_data(value, *size)
            }
            NodeKind::Expr(ExprKind::Ref { target }) => target
                .and_then(|target| self.param_vals.get(&target).cloned())
                .ok_or_else(|| "unresolved reference in declaration initializer".to_string()),
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
                    ("$itor", Val::Real(value)) => {
                        Ok(Val::Real(elab::real_to_bits(value, 32, true).to_real()))
                    }
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
        op: i32,
        reordered: bool,
        operands: &[NodeId],
    ) -> Result<elab::Value, String> {
        use vpi::*;
        let u = |i: usize| self.eval_bits(operands[i]);
        macro_rules! b {
            ($i:expr) => {
                u($i)?
            };
        }
        match op {
            vpiMinusOp => Ok(elab::minus(&b!(0))),
            vpiPlusOp => Ok(b!(0)),
            vpiNotOp => Ok(elab::log_not(&b!(0))),
            vpiBitNegOp => Ok(elab::bit_neg(&b!(0))),
            vpiUnaryAndOp => Ok(elab::unary_and(&b!(0))),
            vpiUnaryNandOp => Ok(elab::unary_nand(&b!(0))),
            vpiUnaryOrOp => Ok(elab::unary_or(&b!(0))),
            vpiUnaryNorOp => Ok(elab::unary_nor(&b!(0))),
            vpiUnaryXorOp => Ok(elab::unary_xor(&b!(0))),
            vpiUnaryXNorOp => Ok(elab::unary_xnor(&b!(0))),
            vpiSubOp => Ok(elab::sub(&b!(0), &b!(1))),
            vpiDivOp => Ok(elab::div(&b!(0), &b!(1))),
            vpiModOp => Ok(elab::rem(&b!(0), &b!(1))),
            vpiEqOp => Ok(elab::eq(&b!(0), &b!(1))),
            vpiNeqOp => Ok(elab::neq(&b!(0), &b!(1))),
            vpiCaseEqOp => Ok(elab::case_eq(&b!(0), &b!(1))),
            vpiCaseNeqOp => Ok(elab::case_neq(&b!(0), &b!(1))),
            vpiWildEqOp => Ok(elab::wildcard_eq(&b!(0), &b!(1))),
            vpiWildNeqOp => Ok(elab::wildcard_neq(&b!(0), &b!(1))),
            vpiGtOp => Ok(elab::gt(&b!(0), &b!(1))),
            vpiGeOp => Ok(elab::ge(&b!(0), &b!(1))),
            vpiLtOp => Ok(elab::lt(&b!(0), &b!(1))),
            vpiLeOp => Ok(elab::le(&b!(0), &b!(1))),
            vpiLShiftOp => Ok(elab::shl(&b!(0), &b!(1))),
            vpiRShiftOp => Ok(elab::shr(&b!(0), &b!(1))),
            vpiArithLShiftOp => Ok(elab::arith_shl(&b!(0), &b!(1))),
            vpiArithRShiftOp => Ok(elab::arith_shr(&b!(0), &b!(1))),
            vpiAddOp => Ok(elab::add(&b!(0), &b!(1))),
            vpiMultOp => Ok(elab::mul(&b!(0), &b!(1))),
            vpiPowerOp => Ok(elab::power(&b!(0), &b!(1))),
            vpiLogAndOp => Ok(elab::log_and(&b!(0), &b!(1))),
            vpiLogOrOp => Ok(elab::log_or(&b!(0), &b!(1))),
            vpiBitAndOp => Ok(elab::bit_and(&b!(0), &b!(1))),
            vpiBitOrOp => Ok(elab::bit_or(&b!(0), &b!(1))),
            vpiBitXorOp => Ok(elab::bit_xor(&b!(0), &b!(1))),
            vpiBitXNorOp => Ok(elab::bit_xnor(&b!(0), &b!(1))),
            vpiConditionOp => Ok(elab::cond(&b!(0), &b!(1), &b!(2))),
            vpiMinTypMaxOp => Ok(b!(0)),
            vpiConcatOp => {
                let mut parts = Vec::with_capacity(operands.len());
                for i in 0..operands.len() {
                    parts.push(b!(i));
                }
                if reordered {
                    parts.reverse();
                }
                Ok(elab::concat(&parts))
            }
            vpiMultiConcatOp => {
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
            other => Err(format!("unsupported operation op type {other} in bound")),
        }
    }
}
