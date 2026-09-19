//! Signatures.

use super::*;

impl<'a> Codegen<'a> {
    // ── Functions and tasks ───────────────────────────────────────────────

    /// Emit a `static` prototype for every function/task in the instance
    /// tree, so bodies may call each other regardless of declaration order.
    /// Timing-capable tasks use the same typed C-call ABI as delay-free tasks.
    /// Their `llg_wait_*` operations suspend the caller's libaco coroutine, so
    /// each recursive C activation remains resumable without source unrolling.
    pub(in super::super) fn emit_func_prototypes(&mut self, inst: NodeId) -> Result<(), String> {
        let class_methods;
        let children = if self.class_nodes.contains_key(&inst) {
            class_methods = self.class_method_nodes(inst);
            &class_methods
        } else {
            &self.node(inst).children
        };
        for c in children {
            if let NodeKind::FuncTask {
                is_task, automatic, ..
            } = self.kind(*c)
            {
                if !self.func_names.contains_key(c) {
                    continue;
                }
                let automatic = *automatic;
                let dpi = self.db.dpi_import(*c).cloned();
                let (is_task_f, function_ret, formals) = if dpi.is_some() {
                    let is_task = match self.kind(*c) {
                        NodeKind::FuncTask { is_task, .. } => *is_task,
                        _ => return Err("non-FuncTask in function prototype collection".to_owned()),
                    };
                    // DPI imports use only the owned semantic type record;
                    // ordinary source-text width recovery is intentionally
                    // bypassed for both the return and formal list.
                    (is_task, None, self.func_formals(*c))
                } else {
                    self.func_info(*c, inst)?
                };
                let ret = if dpi.is_some() {
                    self.dpi_return_info(*c)?
                } else {
                    function_ret
                };
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
                                    width: if is_handle_kind(&ty.kind) || is_real_kind(&ty.kind) {
                                        0
                                    } else if dpi.is_some() {
                                        ty.width.unwrap_or(0)
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
                                    chandle: is_handle_kind(&ty.kind),
                                    event: ty.kind == "event",
                                    string: ty.kind == "string",
                                })
                            }
                            _ => unreachable!("formal kind"),
                        }
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                if let Some(dpi) = &dpi {
                    self.validate_dpi_import(*c, inst, dpi, &formals_ir)?;
                    if self
                        .func_names
                        .values()
                        .any(|internal_name| internal_name == &dpi.c_name)
                    {
                        return Err(format!(
                            "DPI-C symbol `{}` collides with a generated simulator function name",
                            dpi.c_name
                        ));
                    }
                }
                let has_wait = *is_task && dpi.is_none() && self.task_has_wait(*c, inst);
                if !automatic && dpi.is_none() {
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
                            net_alias: Vec::new(),
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
                    let mut process_locals = HashMap::new();
                    let mut local_seq = 0;
                    self.collect_func_locals(
                        body,
                        inst,
                        &mut locals,
                        &mut chandle_locals,
                        &mut process_locals,
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
                            net_alias: Vec::new(),
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
                        } if is_handle_kind(&ty.kind)
                    ),
                    ret_string: self.is_string_return(*c),
                    dpi: dpi.as_ref().map(|dpi| crate::sim::ir::IrDpiImport {
                        c_name: dpi.c_name.clone(),
                        context: dpi.context,
                        pure: dpi.pure,
                    }),
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
                    receiver_class: self.class_nodes.get(&inst).copied().filter(|_| {
                        dpi.is_none()
                            && !matches!(
                                self.kind(*c),
                                NodeKind::FuncTask {
                                    is_static: true,
                                    ..
                                }
                            )
                    }),
                    virtual_slot: self.method_virtual_slots.get(c).copied(),
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
    pub(in super::super) fn emit_func_bodies(&mut self, inst: NodeId) -> Result<(), String> {
        // Every prototype in the design (tops, packages and compilation
        // units) has a model entry by now, so scalar declaration initializers
        // that call a zero-time function can be lowered.
        self.flush_deferred_declaration_inits()?;
        let class_methods;
        let children = if self.class_nodes.contains_key(&inst) {
            class_methods = self.class_method_nodes(inst);
            &class_methods
        } else {
            &self.node(inst).children
        };
        for c in children {
            if matches!(self.kind(*c), NodeKind::FuncTask { .. })
                && self.func_names.contains_key(c)
                && self.db.dpi_import(*c).is_none()
            {
                if matches!(self.kind(*c), NodeKind::FuncTask { is_task: true, .. })
                    && self.task_requires_event_inline(*c, inst)
                {
                    // Calls take the inline path; do not build a detached
                    // evaluator containing an unbound FormalRead.
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
    pub(super) fn func_signature(
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
        if self.class_nodes.contains_key(&inst)
            && !matches!(
                self.kind(ft),
                NodeKind::FuncTask {
                    is_static: true,
                    ..
                }
            )
        {
            params.push("void *_this".to_string());
        }
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
                    NodeKind::FuncArg { ty, .. } if is_handle_kind(&ty.kind)
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
                    NodeKind::FuncArg { ty, .. } if is_handle_kind(&ty.kind) => "void *",
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
                    NodeKind::FuncArg { ty, .. } if is_handle_kind(&ty.kind) => "void *",
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

    pub(super) fn is_chandle_return(&self, ft: NodeId) -> bool {
        matches!(
            self.kind(ft),
            NodeKind::FuncTask {
                ret: Some(ty), ..
            } if is_handle_kind(&ty.kind)
        )
    }

    pub(super) fn is_string_return(&self, ft: NodeId) -> bool {
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
    fn dpi_return_info(&self, ft: NodeId) -> Result<Option<(u32, bool, bool, bool)>, String> {
        let NodeKind::FuncTask { is_task, ret, .. } = self.kind(ft) else {
            return Err("non-FuncTask passed to dpi_return_info".to_owned());
        };
        if *is_task {
            return Ok(None);
        }
        let Some(ty) = ret else {
            return Ok(None);
        };
        if matches!(ty.kind.as_str(), "void" | "chandle" | "class" | "string") {
            return Ok(None);
        }
        if is_real_kind(&ty.kind) {
            return Ok(Some((0, false, false, ty.kind == "shortreal")));
        }
        let width = ty
            .width
            .ok_or_else(|| format!("return type of `{}` has no width", self.node(ft).name))?;
        if width > LLG_MAX_WIDTH {
            return Err(format!(
                "return type of `{}` is {width} bits wide; the runtime maximum supported width is {LLG_MAX_WIDTH}",
                self.node(ft).name
            ));
        }
        let return_var = self
            .node(ft)
            .children
            .iter()
            .copied()
            .find(|child| matches!(self.kind(*child), NodeKind::Var { .. }));
        let two_state = return_var.is_some_and(|return_var| self.db.is_two_state_type(return_var))
            || is_two_state_kind(&ty.kind);
        Ok(Some((width, ty.signed, two_state, false)))
    }

    #[allow(clippy::type_complexity)]
    pub(in super::super) fn func_info(
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
            Some(ty) if matches!(ty.kind.as_str(), "chandle" | "class" | "string") => None,
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
        Ok((is_task, ret, self.func_formals(ft)))
    }

    /// Return formal declarations in source order without applying any
    /// source-text recovery to their owned widths.  DPI imports use this
    /// helper directly so a packed range elsewhere on a declaration line
    /// cannot influence their canonical C ABI.
    pub(super) fn func_formals(&self, ft: NodeId) -> Vec<(NodeId, bool)> {
        self.node(ft)
            .children
            .iter()
            .copied()
            .take_while(|child| {
                matches!(
                    self.kind(*child),
                    NodeKind::FuncArg { .. } | NodeKind::Var { .. }
                )
            })
            .filter_map(|child| match self.kind(child) {
                NodeKind::FuncArg { direction, .. } => Some((
                    child,
                    matches!(direction, DbDirection::Output | DbDirection::Inout),
                )),
                // The function-name return variable comes before the formals
                // in the fixed child order; skip it.
                NodeKind::Var { .. } => None,
                _ => unreachable!("func_formals take_while kind"),
            })
            .collect()
    }

    /// Validate and register the bounded DPI-C ABI supported by H27.  The
    /// simulator's internal `sv4_t`/owned-string ABI remains private; only
    /// scalar canonical DPI types cross the generated thunk boundary.
    fn validate_dpi_import(
        &mut self,
        ft: NodeId,
        inst: NodeId,
        dpi: &crate::core::db::DpiImportInfo,
        formals: &[IrFormal],
    ) -> Result<(), String> {
        if dpi.c_name.is_empty()
            || !dpi.c_name.chars().enumerate().all(|(index, ch)| {
                if index == 0 {
                    ch == '_' || ch.is_ascii_alphabetic()
                } else {
                    ch == '_' || ch.is_ascii_alphanumeric()
                }
            })
        {
            return Err(format!(
                "DPI-C import `{}` has an invalid C linkage identifier `{}`",
                self.node(ft).name,
                dpi.c_name
            ));
        }
        if self.class_nodes.contains_key(&inst) {
            return Err(format!(
                "DPI-C import `{}` in a class method is not supported",
                self.node(ft).name
            ));
        }
        let is_task = match self.kind(ft) {
            NodeKind::FuncTask { is_task, .. } => *is_task,
            _ => return Err("non-FuncTask passed to validate_dpi_import".to_owned()),
        };
        let ret = self.dpi_return_info(ft)?;
        if dpi.pure && is_task {
            return Err(format!(
                "DPI-C import task `{}` cannot be pure",
                self.node(ft).name
            ));
        }
        let ret_key = match self.kind(ft) {
            NodeKind::FuncTask { ret: Some(ty), .. } if ty.kind != "void" => {
                let width = if is_real_kind(&ty.kind) || is_handle_kind(&ty.kind) {
                    0
                } else {
                    ty.width
                        .map(|width| {
                            if self.db.dpi_import(ft).is_some() {
                                width
                            } else {
                                self.effective_decl_width(ft, inst, width)
                            }
                        })
                        .unwrap_or(0)
                };
                let two_state = ret.is_some_and(|(_, _, two_state, _)| two_state);
                dpi_type_key(ty, width, two_state)?
            }
            _ => "void".to_owned(),
        };
        // C linkage is global, and these qualifiers are part of the owned
        // declaration contract even though they do not change the C ABI.
        // Treat a qualifier mismatch as a conflict rather than silently
        // assigning one optimizer/effect interpretation to both imports.
        let mut key = format!(
            "context={};pure={};return={ret_key}",
            dpi.context as u8, dpi.pure as u8
        );
        for (index, ((formal, _), ir)) in self
            .node(ft)
            .children
            .iter()
            .filter_map(|child| match self.kind(*child) {
                NodeKind::FuncArg { direction, ty, .. } => Some(((*direction, ty), *child)),
                _ => None,
            })
            .zip(formals)
            .enumerate()
        {
            let (direction, ty) = formal;
            if matches!(direction, DbDirection::Ref) || ir.is_ref() {
                return Err(format!(
                    "DPI-C import `{}` formal {index} uses unsupported ref direction",
                    self.node(ft).name
                ));
            }
            if ir.event {
                return Err(format!(
                    "DPI-C import `{}` formal {index} has unsupported event type",
                    self.node(ft).name
                ));
            }
            let width = if ir.real || ir.chandle || ir.string {
                0
            } else {
                ir.width
            };
            let type_key = dpi_type_key(ty, width, ir.two_state)?;
            if dpi.pure && !matches!(direction, DbDirection::Input) {
                return Err(format!(
                    "pure DPI-C import `{}` formal {index} must be input",
                    self.node(ft).name
                ));
            }
            key.push_str(&format!(";arg{index}={direction:?}:{type_key}"));
        }
        if let Some(previous) = self.dpi_signatures.get(&dpi.c_name) {
            if previous != &key {
                return Err(format!(
                    "DPI-C symbol `{}` has conflicting imported signatures",
                    dpi.c_name
                ));
            }
        } else {
            self.dpi_signatures.insert(dpi.c_name.clone(), key);
        }
        Ok(())
    }

    /// The body statement identified by Slang's semantic `Body` relationship.
    pub(in super::super) fn func_body(&self, ft: NodeId) -> Option<NodeId> {
        self.db.subroutine_body(ft)
    }
}
