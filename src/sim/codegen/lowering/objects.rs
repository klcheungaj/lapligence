//! Lower non-integral values without encoding their storage as packed bits.
use super::*;
use crate::sim::ir::{
    IrChandleExpr, IrDisplayArg, IrEnumMember, IrEnumMethod, IrEnumQuery, IrExpr, IrObject,
    IrObjectQuery, IrObjectStmt, IrObjectType, IrProcessControl, IrProcessExpr, IrStringExpr,
};

type VirtualInterfaceAccess = (IrChandleExpr, usize, usize, u32, bool, bool);

impl Codegen<'_> {
    pub(super) fn is_process_self_call(&self, node: NodeId) -> bool {
        matches!(self.kind(node), NodeKind::FuncCall { name, .. } if name == "self")
    }

    pub(super) fn is_semaphore_constructor_call(&self, node: NodeId) -> bool {
        if !matches!(self.kind(node), NodeKind::FuncCall { name, .. } if name == "new") {
            return false;
        }
        self.db.node_ids().any(|owner| {
            matches!(
                self.kind(owner),
                NodeKind::Expr(ExprKind::NewClass {
                    class_name: Some(name),
                    constructor: Some(constructor),
                    ..
                }) if name == "semaphore" && *constructor == node
            )
        })
    }

    fn is_process_rng_receiver(&self, node: NodeId) -> bool {
        match self.kind(node) {
            NodeKind::FuncCall { .. } if self.is_process_self_call(node) => true,
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => self.node(node).name == "self" || self.node(*target).name == "self",
            _ => self.node(node).name == "self",
        }
    }

    /// Lower class-valued declaration initializers into run-once processes.
    /// They are inserted before user processes so allocation, property
    /// defaults, and constructors have completed at time zero.
    pub(super) fn emit_class_object_initializers(&mut self) -> Result<(), String> {
        let initializers = std::mem::take(&mut self.class_object_initializers);
        let mut processes = Vec::with_capacity(initializers.len());
        for (index, (object_node, object, initializer, path)) in
            initializers.into_iter().enumerate()
        {
            let owner = self.owning_inst(object_node).ok_or_else(|| {
                format!(
                    "class initializer for `{}` has no owning module instance",
                    self.node(object_node).full_name
                )
            })?;
            self.inst = owner;
            let value = self.lower_chandle(&path, initializer)?;
            let c_name = format!("p_{}_class_init_{index}", ident(&path));
            let label = format!("{path}.class_initializer.{index}");
            processes.push(IrProcess::new_with_origin(
                c_name,
                label,
                IrShape::RunOnce,
                Vec::new(),
                vec![IrStmt::Object(IrObjectStmt::ChandleAssign(object, value))],
                self.origin(object_node),
            ));
        }
        self.model.processes.splice(0..0, processes);
        Ok(())
    }

    /// Construct semaphore objects from declaration initializers in a small
    /// run-once process.  `new(...)` is a runtime operation and cannot appear
    /// in a C static initializer; running these before user processes preserves
    /// the time-zero object construction order.
    pub(super) fn emit_semaphore_initializers(&mut self) -> Result<(), String> {
        let initializers = std::mem::take(&mut self.semaphore_initializers);
        let mut processes = Vec::with_capacity(initializers.len());
        for (index, (object_node, object, initializer, path)) in
            initializers.into_iter().enumerate()
        {
            let owner = self.owning_inst(object_node).ok_or_else(|| {
                format!(
                    "semaphore initializer for `{}` has no owning module instance",
                    self.node(object_node).full_name
                )
            })?;
            self.inst = owner;
            let value = self.lower_chandle(&path, initializer)?;
            let c_name = format!("p_{}_semaphore_init_{index}", ident(&path));
            let label = format!("{path}.semaphore_initializer.{index}");
            processes.push(IrProcess::new_with_origin(
                c_name,
                label,
                IrShape::RunOnce,
                Vec::new(),
                vec![IrStmt::Object(IrObjectStmt::ChandleAssign(object, value))],
                self.origin(object_node),
            ));
        }
        self.model.processes.splice(0..0, processes);
        Ok(())
    }

    fn class_field_target(&self, node: NodeId) -> Option<NodeId> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target }) => target.filter(|target| {
                self.class_fields.contains_key(target)
                    || self.class_static_signals.contains_key(target)
                    || self.class_static_objects.contains_key(target)
            }),
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => {
                refs.iter().rev().flatten().copied().find(|target| {
                    self.class_fields.contains_key(target)
                        || self.class_static_signals.contains_key(target)
                        || self.class_static_objects.contains_key(target)
                })
            }
            NodeKind::Var { .. } if self.class_fields.contains_key(&node) => Some(node),
            _ => None,
        }
    }

    pub(super) fn is_class_method_call(&self, node: NodeId) -> bool {
        let callee = match self.kind(node) {
            NodeKind::MethodCall { callee, .. } | NodeKind::FuncCall { callee, .. } => *callee,
            _ => None,
        };
        callee.is_some_and(|callee| self.class_method_owner(callee).is_some())
    }

    /// Find the class owning a method declaration. Out-of-block methods are
    /// represented by a method-prototype wrapper between the executable
    /// subroutine and its class, so checking only the immediate parent loses
    /// the class receiver for those calls.
    pub(super) fn class_method_owner(&self, callee: NodeId) -> Option<NodeId> {
        let mut current = Some(callee);
        while let Some(node) = current {
            if self.class_nodes.contains_key(&node) {
                return Some(node);
            }
            current = self.node(node).parent;
        }
        None
    }

    /// Whether a class call should use the runtime override selected by its
    /// receiver's nominal class.  `super` explicitly suppresses virtual
    /// dispatch and therefore binds to the declaring base implementation.
    pub(super) fn class_method_virtual_dispatch(&self, node: NodeId) -> bool {
        let (callee, receiver, is_super) = match self.kind(node) {
            NodeKind::MethodCall {
                callee, receiver, ..
            } => (*callee, *receiver, false),
            NodeKind::FuncCall {
                callee, is_super, ..
            } => (*callee, None, *is_super),
            _ => return false,
        };
        if is_super {
            return false;
        }
        if receiver.is_some_and(|receiver| {
            matches!(
                self.kind(receiver),
                NodeKind::Expr(ExprKind::NewClass {
                    is_super_class: true,
                    ..
                })
            )
        }) {
            return false;
        }
        callee.is_some_and(|callee| {
            matches!(
                self.kind(callee),
                NodeKind::FuncTask {
                    is_virtual: true,
                    ..
                }
            ) && !matches!(
                self.kind(callee),
                NodeKind::FuncTask {
                    is_static: true,
                    ..
                }
            )
        })
    }

    /// Resolve the receiver for a class subroutine call. Explicit receivers
    /// are evaluated at the call site; an omitted receiver means `this` when
    /// lowering a method body (or a fresh allocation's constructor body).
    pub(super) fn class_method_receiver(
        &mut self,
        node: NodeId,
    ) -> Result<Option<IrChandleExpr>, String> {
        let (callee, explicit) = match self.kind(node) {
            NodeKind::MethodCall {
                callee, receiver, ..
            } => (*callee, *receiver),
            NodeKind::FuncCall { callee, .. } => (*callee, None),
            _ => return Ok(None),
        };
        let Some(callee) = callee else {
            return Err("class method call has no resolved callee".to_owned());
        };
        if self.class_method_owner(callee).is_none() {
            return Ok(None);
        }
        if matches!(
            self.kind(callee),
            NodeKind::FuncTask {
                is_static: true,
                ..
            }
        ) {
            return Ok(None);
        }
        if let Some(explicit) = explicit {
            return self.lower_chandle("class method call", explicit).map(Some);
        }
        self.func
            .as_ref()
            .and_then(|function| function.class_receiver.clone())
            .or_else(|| self.class_init_receiver.clone())
            .map(Some)
            .ok_or_else(|| "class method call has no receiver".to_owned())
    }

    fn class_receiver_code(&self, receiver: &IrChandleExpr) -> String {
        match receiver {
            IrChandleExpr::Null => "NULL".to_owned(),
            IrChandleExpr::Verbatim(code) => code.clone(),
            IrChandleExpr::Read(index) => self.model.objects[*index].c_name.clone(),
            IrChandleExpr::LocalRead(name) => name.clone(),
            IrChandleExpr::FormalRead(index) => {
                let Some(function) = self.func.as_ref() else {
                    return format!("a{index}");
                };
                let Some((formal, _)) = function
                    .chandle_read
                    .iter()
                    .find(|(_, value)| matches!(value, IrChandleExpr::FormalRead(formal) if formal == index))
                else {
                    return format!("a{index}");
                };
                if let Some(ChandleTarget::Local(name)) = function.chandle_write.get(formal) {
                    name.clone()
                } else {
                    format!("*r{index}")
                }
            }
            IrChandleExpr::ContainerGet { .. }
            | IrChandleExpr::ContainerGetNested { .. }
            | IrChandleExpr::AssociativeGet { .. }
            | IrChandleExpr::Call { .. } => "NULL".to_owned(),
        }
    }

    fn virtual_interface_handle_code(&self, handle: &IrChandleExpr) -> Result<String, String> {
        match handle {
            IrChandleExpr::Null => Ok("NULL".to_owned()),
            IrChandleExpr::Verbatim(code) | IrChandleExpr::LocalRead(code) => Ok(code.clone()),
            IrChandleExpr::Read(index) => Ok(self.model.objects[*index].c_name.clone()),
            IrChandleExpr::FormalRead(index) => Ok(
                if self
                    .cur_fn_ir
                    .and_then(|function| self.model.funcs.get(function))
                    .and_then(|function| function.formals.get(*index))
                    .is_some_and(IrFormal::is_ref)
                {
                    format!("*r{index}")
                } else if self
                    .cur_fn_ir
                    .and_then(|function| self.model.funcs.get(function))
                    .and_then(|function| function.formals.get(*index))
                    .is_some_and(|formal| formal.is_out)
                {
                    format!("*o{index}")
                } else {
                    format!("a{index}")
                },
            ),
            IrChandleExpr::ContainerGet { container, index } => {
                let getter = match self.model.containers[*container].kind {
                    crate::sim::ir::IrContainerKind::Dynamic => "llg_dyn_value_get_chandle",
                    crate::sim::ir::IrContainerKind::Queue { .. } => "llg_queue_value_get_chandle",
                    crate::sim::ir::IrContainerKind::Associative { .. } => {
                        return Err(
                            "virtual interface associative element receivers are unsupported"
                                .to_owned(),
                        )
                    }
                };
                Ok(format!(
                    "{}(&{}, {})",
                    getter,
                    self.model.containers[*container].c_name,
                    self.render_ir_code(index)?
                ))
            }
            IrChandleExpr::ContainerGetNested { container, indices } => {
                let getter = match self.model.containers[*container].kind {
                    crate::sim::ir::IrContainerKind::Dynamic => "llg_dyn_value_get_nested_chandle",
                    crate::sim::ir::IrContainerKind::Queue { .. } => {
                        "llg_queue_value_get_nested_chandle"
                    }
                    crate::sim::ir::IrContainerKind::Associative { .. } => {
                        return Err(
                            "virtual interface associative element receivers are unsupported"
                                .to_owned(),
                        )
                    }
                };
                let count = indices.len();
                let indices = indices
                    .iter()
                    .map(|index| self.render_ir_code(index))
                    .collect::<Result<Vec<_>, _>>()?
                    .join(", ");
                Ok(format!(
                    "{}(&{}, (const sv4_t[]){{ {} }}, {})",
                    getter, self.model.containers[*container].c_name, indices, count
                ))
            }
            IrChandleExpr::AssociativeGet { .. } | IrChandleExpr::Call { .. } => Err(
                "virtual interface member receiver must be a named handle or array element"
                    .to_owned(),
            ),
        }
    }

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
                format!(
                    "member `{member}` is not available through virtual interface view `{spelling}` in `{path}`"
                )
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
        let handle = self.lower_chandle(path, handle)?;
        Ok(Some((handle, descriptor, slot, width, signed, two_state)))
    }

    /// Lower a member path rooted at a virtual-interface handle. The runtime
    /// receives the handle on every access, so assigning a new handle changes
    /// the target of all subsequent reads and writes.
    pub(super) fn virtual_interface_member_expr(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrExpr>, String> {
        let Some((handle, descriptor, slot, width, signed, _two_state)) =
            self.virtual_interface_access(path, node, false)?
        else {
            return Ok(None);
        };
        let handle = self.virtual_interface_handle_code(&handle)?;
        Ok(Some(IrExpr::new(
            IrExprKind::Verbatim {
                code: format!(
                    "llg_vif_read((void *){handle}, {descriptor}, {slot}, {}, {}, \"{}\")",
                    width,
                    signed as u8,
                    self.node(node).full_name.replace('"', "'"),
                ),
                width,
                signed,
            },
            width,
            signed,
            None,
        )))
    }

    pub(super) fn virtual_interface_member_lhs(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrLhs>, String> {
        let Some((handle, descriptor, slot, width, signed, two_state)) =
            self.virtual_interface_access(path, node, true)?
        else {
            return Ok(None);
        };
        let handle = self.virtual_interface_handle_code(&handle)?;
        Ok(Some(IrLhs::WholeRef {
            addr: format!(
                "llg_vif_member((void *){handle}, {descriptor}, {slot}, \"{}\")",
                self.node(node).full_name.replace('"', "'"),
            ),
            width,
            signed,
            two_state,
            shortreal: false,
        }))
    }

    fn class_receiver_for(
        &mut self,
        path: &str,
        node: NodeId,
        field: NodeId,
    ) -> Result<IrChandleExpr, String> {
        if let NodeKind::Expr(ExprKind::HierPath { refs, .. }) = self.kind(node) {
            if let Some(base) = refs
                .iter()
                .take_while(|reference| **reference != Some(field))
                .flatten()
                .find(|base| self.object_of(path, **base).is_some())
            {
                return self.lower_chandle(path, *base);
            }
        }
        self.func
            .as_ref()
            .and_then(|function| function.class_receiver.clone())
            .or_else(|| self.class_init_receiver.clone())
            .ok_or_else(|| {
                format!(
                    "class property `{}` has no receiver in `{path}`",
                    self.node(field).name
                )
            })
    }

    fn class_field_layout(
        &self,
        field: NodeId,
    ) -> Option<(usize, usize, &crate::sim::ir::IrClassField)> {
        let (class, index) = self.class_fields.get(&field).copied()?;
        Some((
            class,
            index,
            self.model.classes.get(class)?.fields.get(index)?,
        ))
    }

    fn class_field_value_shape(&self, field: NodeId) -> Result<(u32, bool, bool), String> {
        match self.kind(field) {
            NodeKind::Var { ty } if is_real_kind(&ty.kind) => Ok((0, false, false)),
            NodeKind::Var { ty } => Ok((
                ty.width.ok_or_else(|| {
                    format!("class property `{}` has no width", self.node(field).name)
                })?,
                ty.signed,
                self.db.is_two_state_type(field) || is_two_state_kind(&ty.kind),
            )),
            _ => Err(format!(
                "class field target `{}` is not a variable",
                self.node(field).name
            )),
        }
    }

    fn node_contains_super_constructor(&self, root: NodeId) -> bool {
        let mut pending = vec![root];
        let mut visited = HashSet::new();
        while let Some(node) = pending.pop() {
            if !visited.insert(node) {
                continue;
            }
            if matches!(
                self.kind(node),
                NodeKind::Expr(ExprKind::NewClass {
                    is_super_class: true,
                    ..
                })
            ) {
                return true;
            }
            pending.extend(self.node(node).children.iter().copied());
        }
        false
    }

    fn class_constructor(&self, class: NodeId) -> Option<NodeId> {
        self.class_method_nodes(class).into_iter().find(|child| {
            matches!(
                self.kind(*child),
                NodeKind::FuncTask {
                    is_constructor: true,
                    ..
                }
            )
        })
    }

    pub(super) fn class_field_expr(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrExpr>, String> {
        let Some(field) = self.class_field_target(node) else {
            return Ok(None);
        };
        if let Some(info) = self.class_static_signals.get(&field).cloned() {
            return Ok(Some(self.signal_read_expr(&info)?));
        }
        if self.class_static_objects.contains_key(&field) {
            return Err(format!(
                "string/class object static property `{}` is not a packed expression",
                self.node(field).name
            ));
        }
        if matches!(self.kind(field), NodeKind::Var { ty } if is_handle_kind(&ty.kind)) {
            return Ok(None);
        }
        let Some((class, _index, class_field)) = self.class_field_layout(field) else {
            return Ok(None);
        };
        let (width, signed, _two_state) = self.class_field_value_shape(field)?;
        let field_name = class_field.c_name.clone();
        let receiver = self.class_receiver_for(path, node, field)?;
        let receiver = self.class_receiver_code(&receiver);
        let code = format!(
            "((llg_class_{class}_t*)llg_class_require({receiver}, \"{}\"))->{}",
            self.node(field).full_name,
            field_name
        );
        Ok(Some(IrExpr::new(
            IrExprKind::Verbatim {
                code,
                width,
                signed,
            },
            width,
            signed,
            None,
        )))
    }

    pub(super) fn class_field_lhs(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrLhs>, String> {
        let Some(field) = self.class_field_target(node) else {
            return Ok(None);
        };
        if let Some(info) = self.class_static_signals.get(&field).cloned() {
            return Ok(Some(IrLhs::Whole(info.ir)));
        }
        if self.class_static_objects.contains_key(&field) {
            return Ok(None);
        }
        if matches!(self.kind(field), NodeKind::Var { ty } if is_handle_kind(&ty.kind)) {
            return Ok(None);
        }
        let Some((class, _index, class_field)) = self.class_field_layout(field) else {
            return Ok(None);
        };
        let (width, signed, two_state) = self.class_field_value_shape(field)?;
        let field_name = class_field.c_name.clone();
        let shortreal = matches!(
            class_field.ty,
            crate::sim::ir::IrClassFieldType::Real { shortreal: true }
        );
        let receiver = self.class_receiver_for(path, node, field)?;
        let receiver = self.class_receiver_code(&receiver);
        let addr = format!(
            "&(((llg_class_{class}_t*)llg_class_require({receiver}, \"{}\"))->{})",
            self.node(field).full_name,
            field_name
        );
        Ok(Some(IrLhs::WholeRef {
            addr,
            width,
            signed,
            two_state,
            shortreal,
        }))
    }

    fn class_field_chandle_lvalue(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<String>, String> {
        let Some(field) = self.class_field_target(node) else {
            return Ok(None);
        };
        if !matches!(self.kind(field), NodeKind::Var { ty } if is_handle_kind(&ty.kind)) {
            return Ok(None);
        }
        let Some((class, _index, class_field)) = self.class_field_layout(field) else {
            return Ok(None);
        };
        let field_name = class_field.c_name.clone();
        let field_full_name = self.node(field).full_name.clone();
        let receiver = self.class_receiver_for(path, node, field)?;
        let receiver = self.class_receiver_code(&receiver);
        Ok(Some(format!(
            "((llg_class_{class}_t*)llg_class_require({receiver}, \"{}\"))->{}",
            field_full_name, field_name
        )))
    }

    fn lower_new_class(
        &mut self,
        path: &str,
        node: NodeId,
        class_name: Option<&str>,
        class_type: Option<crate::core::db::TypeId>,
        constructor: Option<NodeId>,
        is_super_class: bool,
    ) -> Result<IrChandleExpr, String> {
        if is_super_class {
            return self
                .func
                .as_ref()
                .and_then(|function| function.class_receiver.clone())
                .or_else(|| self.class_init_receiver.clone())
                .ok_or_else(|| format!("super constructor has no receiver in `{path}"));
        }
        if class_name == Some("semaphore") {
            let args = constructor
                .map(|constructor| self.call_argument_nodes(constructor))
                .unwrap_or_default();
            let keys = match args.as_slice() {
                [] => lhs_integer_expr(0),
                [value] => self.semaphore_key_argument(path, *value)?,
                _ => {
                    return Err(format!(
                        "semaphore constructor takes zero or one key-count argument in `{path}`"
                    ))
                }
            };
            return Ok(IrChandleExpr::Verbatim(format!(
                "llg_semaphore_new({})",
                self.render_ir_code(&keys)?
            )));
        }
        let name = class_name.unwrap_or("<specialized class>");
        let class_node = class_type
            .and_then(|type_id| self.db.class_for_type(type_id))
            .or_else(|| {
                self.db
                    .classes()
                    .iter()
                    .find(|class| self.node(**class).name == name)
                    .copied()
            });
        let class_node = class_node
            .ok_or_else(|| format!("class `{name}` has no captured layout in `{path}"))?;
        if let Some(metadata) = self.db.class_metadata(class_node) {
            if metadata.is_abstract || metadata.is_interface {
                return Err(format!(
                    "cannot construct {} class `{name}` in `{path}`",
                    if metadata.is_interface {
                        "interface"
                    } else {
                        "abstract"
                    }
                ));
            }
        }
        let class = self
            .class_nodes
            .get(&class_node)
            .copied()
            .ok_or_else(|| format!("class `{name}` has no captured layout in `{path}`"))?;
        let layout = self
            .model
            .classes
            .get(class)
            .ok_or_else(|| format!("class `{name}` has no execution layout"))?
            .clone();
        let mut code = format!(
            "({{ llg_class_{}_t *_llg_obj = (llg_class_{}_t*)calloc(1, sizeof(llg_class_{}_t)); ",
            class, class, class
        );
        code.push_str(
            "if (!_llg_obj) { fprintf(stderr, \"llg: class allocation failed\\n\"); exit(EXIT_FAILURE); } ",
        );
        code.push_str(&format!("_llg_obj->_llg_class_id = {class}; "));
        // Preserve an enclosing constructor's receiver while lowering a
        // nested `new` expression.  Class-valued locals are legal even though
        // class-valued properties remain outside this bounded layout, and a
        // nested allocation must not make subsequent outer `this` accesses
        // receiver-less.
        let previous_class_init_receiver = self
            .class_init_receiver
            .replace(IrChandleExpr::LocalRead("_llg_obj".to_owned()));
        let mut class_chain = Vec::new();
        let mut current = Some(class_node);
        while let Some(current_class) = current {
            class_chain.push(self.class_nodes[&current_class]);
            current = self
                .db
                .class_metadata(current_class)
                .and_then(|metadata| metadata.base);
        }
        class_chain.reverse();
        let mut class_fields = class_chain
            .into_iter()
            .flat_map(|owner| {
                self.class_fields
                    .iter()
                    .filter(move |(_, (class_index, _))| *class_index == owner)
                    .map(|(field_node, (_, field_index))| (*field_node, *field_index))
            })
            .collect::<Vec<_>>();
        class_fields.sort_by_key(|(_, field_index)| *field_index);
        for (field_node, field_index) in class_fields {
            let field = &layout.fields[field_index];
            match field.ty {
                crate::sim::ir::IrClassFieldType::Packed {
                    width,
                    signed,
                    two_state,
                } => {
                    let value = self
                        .db
                        .var_initializer(field_node)
                        .map(|initializer| self.lower_expr(path, initializer))
                        .transpose()?;
                    let value = value
                        .map(|value| {
                            ir_to_storage(value, width, signed, two_state)
                                .and_then(|value| self.render_ir_code(&value))
                        })
                        .transpose()?
                        .unwrap_or_else(|| {
                            if two_state {
                                format!("sv4_from_u64(0, {width}, {})", signed as u8)
                            } else {
                                format!("sv4_x({width}, {})", signed as u8)
                            }
                        });
                    code.push_str(&format!("_llg_obj->{} = {}; ", field.c_name, value));
                }
                crate::sim::ir::IrClassFieldType::Real { shortreal } => {
                    let value = self
                        .db
                        .var_initializer(field_node)
                        .map(|initializer| self.lower_expr(path, initializer))
                        .transpose()?
                        .map(|value| {
                            self.render_ir_code(&IrExpr::new(
                                IrExprKind::CastToReal {
                                    a: Box::new(value),
                                    shortreal,
                                },
                                0,
                                true,
                                None,
                            ))
                        })
                        .transpose()?
                        .unwrap_or_else(|| "0.0".to_owned());
                    code.push_str(&format!("_llg_obj->{} = {}; ", field.c_name, value));
                }
                crate::sim::ir::IrClassFieldType::String
                | crate::sim::ir::IrClassFieldType::Chandle => {}
            }
        }
        let explicit_base = constructor
            .and_then(|constructor| match self.kind(constructor) {
                NodeKind::FuncCall { callee, .. } => *callee,
                _ => None,
            })
            .and_then(|constructor| self.func_body(constructor))
            .is_some_and(|body| self.node_contains_super_constructor(body));
        if !explicit_base {
            let base_constructor = self
                .db
                .class_metadata(class_node)
                .and_then(|metadata| metadata.base_constructor)
                .and_then(|base_call| match self.kind(base_call) {
                    NodeKind::FuncCall { callee, .. } => Some((base_call, *callee)),
                    _ => None,
                })
                .or_else(|| {
                    self.db
                        .class_metadata(class_node)
                        .and_then(|metadata| metadata.base)
                        .and_then(|base| self.class_constructor(base))
                        .map(|constructor| (node, Some(constructor)))
                });
            if let Some((base_call, base_constructor)) = base_constructor {
                let Some(base_constructor) = base_constructor else {
                    return Err(format!(
                        "base constructor for `{name}` is unresolved in `{path}`"
                    ));
                };
                let mut call = if base_call == node {
                    self.lower_func_call_expr_with_args(
                        path,
                        base_call,
                        "new",
                        Some(base_constructor),
                        &[],
                    )?
                } else {
                    self.lower_func_call_expr(path, base_call, "new", Some(base_constructor))?
                };
                if let IrExprKind::CallFn(call_expr) = &mut call.kind {
                    call_expr.receiver = Some(IrChandleExpr::LocalRead("_llg_obj".to_owned()));
                    call_expr.virtual_dispatch = false;
                }
                code.push_str(&format!("{}; ", self.render_ir_code(&call)?));
            }
        }
        if let Some(constructor) = constructor {
            let (name, callee) = match self.kind(constructor) {
                NodeKind::FuncCall { name, callee, .. } => (name.clone(), *callee),
                _ => return Err("class constructor edge does not name a function call".to_owned()),
            };
            let mut call = self.lower_func_call_expr(path, constructor, &name, callee)?;
            if let IrExprKind::CallFn(call) = &mut call.kind {
                call.receiver = Some(IrChandleExpr::LocalRead("_llg_obj".to_owned()));
            }
            code.push_str(&format!("{}; ", self.render_ir_code(&call)?));
        }
        code.push_str("(void*)_llg_obj; })");
        self.class_init_receiver = previous_class_init_receiver;
        let _ = node;
        Ok(IrChandleExpr::Verbatim(code))
    }

    fn object_int_argument(
        &mut self,
        path: &str,
        node: NodeId,
        width: u32,
    ) -> Result<IrExpr, String> {
        let value = self.lower_expr(path, node)?;
        ir_to_storage(value, width, true, true)
    }

    fn semaphore_key_argument(&mut self, path: &str, node: NodeId) -> Result<IrExpr, String> {
        let value = self.lower_expr(path, node)?;
        if value.is_real() {
            return Err(format!("semaphore key count must be integral in `{path}`"));
        }
        // Built-in semaphore methods take a signed 32-bit `int` keyCount.
        // Keep X/Z bits intact for the runtime boundary instead of applying
        // the two-state conversion used by ordinary array indices.
        ir_to_storage(value, 32, true, false)
    }
    pub(super) fn lower_boolean_expr(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<IrExpr, String> {
        if self.is_chandle_expr(path, node) {
            let equal = object_query(
                IrObjectQuery::ChandleEq(self.lower_chandle(path, node)?, IrChandleExpr::Null),
                1,
                false,
            );
            return Ok(IrExpr::new(
                IrExprKind::Un {
                    op: IrUnOp::LogNot,
                    a: Box::new(equal),
                },
                1,
                false,
                None,
            ));
        }
        self.lower_expr(path, node)
    }
    pub(super) fn collect_object(&mut self, path: &str, node: NodeId) -> Result<bool, String> {
        let ty = match self.kind(node) {
            NodeKind::Var { ty } => match ty.kind.as_str() {
                "string" => IrObjectType::String,
                "chandle" => IrObjectType::Chandle,
                "class" if ty.type_name.as_deref() == Some("process") => IrObjectType::Process,
                "class" if ty.type_name.as_deref() == Some("semaphore") => IrObjectType::Semaphore,
                "class" => IrObjectType::Chandle,
                "virtual_interface" => IrObjectType::Chandle,
                _ => return Ok(false),
            },
            _ => return Ok(false),
        };
        if self.object_globals.contains_key(&node) {
            return Ok(true);
        }
        let name = self.node(node).name.clone();
        let index = self.model.objects.len();
        self.model.objects.push(IrObject {
            c_name: format!("O_{}_{}", ident(path), ident(&name)),
            ty,
            initial: None,
        });
        self.object_globals.insert(node, index);
        self.scope_object_names
            .entry(path.to_owned())
            .or_default()
            .insert(name, index);
        if let Some(init) = self.db.var_initializer(node) {
            match ty {
                IrObjectType::String => {
                    self.model.objects[index].initial = Some(self.lower_string(path, init)?)
                }
                IrObjectType::Chandle => {
                    let is_class =
                        matches!(self.kind(node), NodeKind::Var { ty } if ty.kind == "class");
                    if is_class {
                        self.class_object_initializers
                            .push((node, index, init, path.to_owned()));
                    } else if matches!(self.kind(node), NodeKind::Var { ty } if ty.kind == "virtual_interface")
                    {
                        if !matches!(
                            self.kind(init),
                            NodeKind::Expr(ExprKind::Constant {
                                const_type: ConstantType::Null,
                                ..
                            })
                        ) {
                            self.validate_virtual_interface_assignment(node, init, path)?;
                            self.class_object_initializers.push((
                                node,
                                index,
                                init,
                                path.to_owned(),
                            ));
                        }
                    } else if self.lower_chandle(path, init)? != IrChandleExpr::Null {
                        return Err("chandle declaration initializer must be null".to_owned());
                    }
                }
                IrObjectType::Semaphore => {
                    if matches!(self.kind(init), NodeKind::Expr(ExprKind::NewClass { .. })) {
                        self.semaphore_initializers
                            .push((node, index, init, path.to_owned()));
                    } else if self.lower_chandle(path, init)? != IrChandleExpr::Null {
                        return Err(
                            "semaphore declaration initializer must be new or null".to_owned()
                        );
                    }
                }
                IrObjectType::Process => {
                    if self.lower_process(path, init)? != IrProcessExpr::Null {
                        return Err("process declaration initializer must be null".to_owned());
                    }
                }
            }
        }
        Ok(true)
    }

    pub(super) fn object_of(&self, path: &str, node: NodeId) -> Option<usize> {
        if let Some((target, _kind, member)) = self.unpacked_member_info(node) {
            if let Some(index) = member.object {
                return Some(self.reference_object(index));
            }
            // A recursive aggregate leaf is still declaration-owned even when
            // its representation is a packed/real signal. Do not fall
            // through to display-name lookup for those values.
            if self.unpacked_aggregates.contains_key(&target) {
                return None;
            }
        }
        if let Some(index) = self.object_globals.get(&node) {
            return Some(self.reference_object(*index));
        }
        if let NodeKind::Expr(ExprKind::Ref {
            target: Some(target),
        }) = self.kind(node)
        {
            return self
                .object_globals
                .get(target)
                .copied()
                .map(|index| self.reference_object(index));
        }
        if !matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::Ref { target: None })
        ) {
            return None;
        }
        self.scope_object_names
            .get(path)?
            .get(&self.node(node).name)
            .copied()
            .map(|index| self.reference_object(index))
    }

    pub(super) fn is_string_expr(&self, path: &str, node: NodeId) -> bool {
        let target = match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            _ => Some(node),
        };
        if self.func.as_ref().is_some_and(|function| {
            target.is_some_and(|target| function.string_read.contains_key(&target))
                || matches!(
                    self.kind(node),
                    NodeKind::Expr(ExprKind::Ref { target: None })
                ) && function
                    .string_read
                    .keys()
                    .any(|target| self.node(*target).name == self.node(node).name)
        }) {
            return true;
        }
        if matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::Ref { target: None })
        ) && self.func.as_ref().is_some_and(|function| {
            let name = &self.node(node).name;
            function
                .arg_ir
                .keys()
                .chain(function.locals.keys())
                .any(|target| self.node(*target).name == *name)
                || function.ret.as_ref().is_some_and(|ret| {
                    ret.node
                        .is_some_and(|target| self.node(target).name == *name)
                })
        }) {
            return false;
        }
        if let NodeKind::FuncCall {
            is_task: false,
            callee,
            ..
        } = self.kind(node)
        {
            if self
                .resolve_callee_env(self.inst, &self.node(node).name, false, *callee)
                .ok()
                .and_then(|(function, _)| self.func_meta.get(&function))
                .is_some_and(|meta| meta.ret_string)
            {
                return true;
            }
        }
        if self
            .object_of(path, node)
            .is_some_and(|index| self.model.objects[index].ty == IrObjectType::String)
        {
            return true;
        }
        if self.lexical_proc_string_local(node).is_some() {
            return true;
        }
        if self.is_container_string_expr(node) {
            return true;
        }
        match self.kind(node) {
            NodeKind::SysCall { name } if name == "$typename" || name == "$sformatf" => true,
            NodeKind::Param { ty, .. } => ty.kind == "string",
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => matches!(self.kind(*target),NodeKind::Param{ty,..} if ty.kind=="string"),
            NodeKind::Expr(ExprKind::Cast { ty, .. }) => ty.kind == "string",
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
                ..
            } => {
                (matches!(name.as_str(), "toupper" | "tolower" | "substr")
                    && self.is_string_expr(path, *receiver))
                    || (name == "get_randstate" && self.is_process_rng_receiver(*receiver))
                    || (name == "name"
                        && self
                            .enum_metadata_for_expr(*receiver)
                            .is_some_and(|_| self.node(node).children.len() == 1))
            }
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Concat | Operation::MultiConcat,
                operands,
                ..
            }) => operands
                .iter()
                .any(|operand| self.is_string_expr(path, *operand)),
            _ => false,
        }
    }

    /// Return whether an expression is a native chandle without requiring it
    /// to be backed by a model-global object. Function formals, automatic
    /// locals, and chandle-returning calls all live in the function context.
    pub(super) fn is_chandle_expr(&self, path: &str, node: NodeId) -> bool {
        if matches!(self.kind(node), NodeKind::Expr(ExprKind::NewClass { .. })) {
            return true;
        }
        if let NodeKind::Expr(ExprKind::Cast { operand, ty, .. }) = self.kind(node) {
            if is_handle_kind(&ty.kind) {
                return true;
            }
            if self.is_chandle_expr(path, *operand) {
                return false;
            }
        }
        let target = match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            _ => Some(node),
        };
        if let Some(function) = &self.func {
            if target.is_some_and(|target| function.chandle_read.contains_key(&target)) {
                return true;
            }
            if matches!(
                self.kind(node),
                NodeKind::Expr(ExprKind::Ref { target: None })
            ) && function
                .chandle_read
                .keys()
                .any(|target| self.node(*target).name == self.node(node).name)
            {
                return true;
            }
        }
        if let Some(index) = self.object_of(path, node) {
            if matches!(
                self.model.objects[index].ty,
                IrObjectType::Chandle | IrObjectType::Semaphore
            ) {
                return true;
            }
        }
        if self.is_container_chandle_expr(node) {
            return true;
        }
        if let NodeKind::FuncCall {
            is_task: false,
            callee,
            ..
        } = self.kind(node)
        {
            return self
                .resolve_callee_env(self.inst, &self.node(node).name, false, *callee)
                .ok()
                .and_then(|(function, _)| self.func_meta.get(&function))
                .is_some_and(|meta| meta.ret_chandle);
        }
        false
    }

    /// Return whether an expression is specifically a SystemVerilog
    /// semaphore handle.  Semaphores share the native pointer ABI with
    /// chandles, but their methods must lower to the blocking runtime service.
    pub(super) fn is_semaphore_expr(&self, path: &str, node: NodeId) -> bool {
        if matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::NewClass {
                class_name: Some(name),
                ..
            }) if name == "semaphore"
        ) {
            return true;
        }
        if let NodeKind::Expr(ExprKind::Cast { operand, ty, .. }) = self.kind(node) {
            if ty.kind == "class" && ty.type_name.as_deref() == Some("semaphore") {
                return true;
            }
            if self.is_semaphore_expr(path, *operand) {
                return true;
            }
        }
        let target = match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            _ => Some(node),
        };
        if target.is_some_and(|target| {
            matches!(
                self.kind(target),
                NodeKind::Var { ty } | NodeKind::FuncArg { ty, .. }
                    if ty.kind == "class" && ty.type_name.as_deref() == Some("semaphore")
            )
        }) {
            return true;
        }
        if let Some(function) = &self.func {
            if target.is_some_and(|target| function.chandle_read.contains_key(&target))
                && target.is_some_and(|target| {
                    matches!(
                        self.kind(target),
                        NodeKind::Var { ty } | NodeKind::FuncArg { ty, .. }
                            if ty.kind == "class"
                                && ty.type_name.as_deref() == Some("semaphore")
                    )
                })
            {
                return true;
            }
            if matches!(
                self.kind(node),
                NodeKind::Expr(ExprKind::Ref { target: None })
            ) && function.chandle_read.keys().any(|target| {
                self.node(*target).name == self.node(node).name
                    && matches!(
                        self.kind(*target),
                        NodeKind::Var { ty } | NodeKind::FuncArg { ty, .. }
                            if ty.kind == "class"
                                && ty.type_name.as_deref() == Some("semaphore")
                    )
            }) {
                return true;
            }
        }
        if self
            .object_of(path, node)
            .is_some_and(|index| self.model.objects[index].ty == IrObjectType::Semaphore)
        {
            return true;
        }
        if self.lexical_proc_semaphore_decl(node).is_some() {
            return true;
        }
        if let NodeKind::FuncCall {
            is_task: false,
            callee,
            ..
        } = self.kind(node)
        {
            if self
                .resolve_callee_env(self.inst, &self.node(node).name, false, *callee)
                .ok()
                .is_some_and(|(function, _)| {
                    matches!(
                        self.kind(function),
                        NodeKind::FuncTask {
                            ret: Some(ty), ..
                        } if ty.kind == "class"
                            && ty.type_name.as_deref() == Some("semaphore")
                    )
                })
            {
                return true;
            }
        }
        false
    }

    /// Return whether an expression denotes a SystemVerilog `process`
    /// handle. Process values are intentionally kept out of packed lowering;
    /// callers must route them through [`lower_process`].
    pub(super) fn is_process_expr(&self, path: &str, node: NodeId) -> bool {
        if matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::Constant {
                const_type: ConstantType::Null,
                ..
            })
        ) || self.is_process_self_call(node)
        {
            return true;
        }
        let target = match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            _ => Some(node),
        };
        if let Some(function) = &self.func {
            if target.is_some_and(|target| function.process_read.contains_key(&target)) {
                return true;
            }
            if matches!(
                self.kind(node),
                NodeKind::Expr(ExprKind::Ref { target: None })
            ) && function
                .process_read
                .keys()
                .any(|target| self.node(*target).name == self.node(node).name)
            {
                return true;
            }
        }
        if self.lexical_proc_process_decl(node).is_some() {
            return true;
        }
        if target.is_some_and(|target| {
            matches!(
                self.kind(target),
                NodeKind::Var { ty }
                    if ty.kind == "class" && ty.type_name.as_deref() == Some("process")
            )
        }) {
            return true;
        }
        self.object_of(path, node)
            .is_some_and(|index| self.model.objects[index].ty == IrObjectType::Process)
    }

    fn process_target_node(&self, node: NodeId) -> Option<NodeId> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            _ => Some(node),
        }
    }

    /// Lower a process handle expression while preserving its stable runtime
    /// identity. Only null, self, declared process objects and mapped locals
    /// are legal sources at this stage.
    pub(super) fn lower_process(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<IrProcessExpr, String> {
        if matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::Constant {
                const_type: ConstantType::Null,
                ..
            })
        ) {
            return Ok(IrProcessExpr::Null);
        }
        if self.is_process_self_call(node) {
            return Ok(IrProcessExpr::SelfHandle);
        }
        let target = self.process_target_node(node);
        if let Some(function) = &self.func {
            if let Some(value) = target.and_then(|target| function.process_read.get(&target)) {
                return Ok(value.clone());
            }
            if matches!(
                self.kind(node),
                NodeKind::Expr(ExprKind::Ref { target: None })
            ) {
                if let Some((_, value)) = function
                    .process_read
                    .iter()
                    .find(|(target, _)| self.node(**target).name == self.node(node).name)
                {
                    return Ok(value.clone());
                }
            }
        }
        if let Some(variable) = self.lexical_proc_process_decl(node) {
            if let Some(name) = self.proc_process_local_name(variable) {
                return Ok(IrProcessExpr::LocalRead(name.to_owned()));
            }
            if self.db.variable_lifetime(variable) == VariableLifetime::Static {
                let index = self.collect_process_static_object(path, variable)?;
                return Ok(IrProcessExpr::Read(index));
            }
        }
        if let Some(index) = self.object_of(path, node) {
            if self.model.objects[index].ty == IrObjectType::Process {
                return Ok(IrProcessExpr::Read(index));
            }
        }
        if let Some(variable) = target.filter(|target| {
            matches!(
                self.kind(*target),
                NodeKind::Var { ty }
                    if ty.kind == "class" && ty.type_name.as_deref() == Some("process")
            )
        }) {
            if let Some(name) = self.proc_process_local_name(variable) {
                return Ok(IrProcessExpr::LocalRead(name.to_owned()));
            }
            if self.db.variable_lifetime(variable) == VariableLifetime::Static {
                let index = self.collect_process_static_object(path, variable)?;
                return Ok(IrProcessExpr::Read(index));
            }
        }
        Err(format!(
            "process handle `{}` cannot be resolved in `{path}`",
            self.node(node).name
        ))
    }

    /// Resolve a process lvalue and retain the same storage identity for
    /// assignment, method control, and later status/await operations.
    pub(super) fn lower_process_lvalue(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<(ProcessTarget, IrProcessExpr), String> {
        let read = self.lower_process(path, node)?;
        let target = self.process_target_node(node);
        if let Some(function) = &self.func {
            if let Some(value) = target.and_then(|target| function.process_write.get(&target)) {
                return Ok((value.clone(), read));
            }
            if matches!(
                self.kind(node),
                NodeKind::Expr(ExprKind::Ref { target: None })
            ) {
                if let Some((_, value)) = function
                    .process_write
                    .iter()
                    .find(|(target, _)| self.node(**target).name == self.node(node).name)
                {
                    return Ok((value.clone(), read));
                }
            }
        }
        if let Some(variable) = self.lexical_proc_process_decl(node) {
            if let Some(name) = self.proc_process_local_name(variable) {
                return Ok((ProcessTarget::Local(name.to_owned()), read));
            }
            if self.db.variable_lifetime(variable) == VariableLifetime::Static {
                let index = self.collect_process_static_object(path, variable)?;
                return Ok((ProcessTarget::Object(index), read));
            }
        }
        if let Some(index) = self.object_of(path, node) {
            if self.model.objects[index].ty == IrObjectType::Process {
                return Ok((ProcessTarget::Object(index), read));
            }
        }
        if let Some(variable) = target.filter(|target| {
            matches!(
                self.kind(*target),
                NodeKind::Var { ty }
                    if ty.kind == "class" && ty.type_name.as_deref() == Some("process")
            )
        }) {
            if let Some(name) = self.proc_process_local_name(variable) {
                return Ok((ProcessTarget::Local(name.to_owned()), read));
            }
            if self.db.variable_lifetime(variable) == VariableLifetime::Static {
                let index = self.collect_process_static_object(path, variable)?;
                return Ok((ProcessTarget::Object(index), read));
            }
        }
        Err("process output/ref actual must be a named process lvalue".to_owned())
    }

    /// Return the owned metadata for an expression whose resolved type is an
    /// enum. This uses only the captured descriptor/type tables.
    pub(super) fn enum_metadata_for_expr(
        &self,
        node: NodeId,
    ) -> Option<&crate::core::db::EnumTypeMetadata> {
        let descriptor = self.query_descriptor(node)?;
        self.db.enum_type_metadata(descriptor.id)
    }

    fn enum_members(
        metadata: &crate::core::db::EnumTypeMetadata,
    ) -> Result<Vec<IrEnumMember>, String> {
        metadata
            .members
            .iter()
            .map(|member| {
                let Val::Bits(value) = &member.value else {
                    return Err(format!(
                        "enum member {} has a non-integral value",
                        member.name
                    ));
                };
                let value = val_to_const(value)?;
                if value.width != metadata.width || value.signed != metadata.signed {
                    return Err(format!(
                        "enum member {} value type does not match its enum type",
                        member.name
                    ));
                }
                Ok(IrEnumMember {
                    value: IrExpr::new(
                        IrExprKind::Const(value),
                        metadata.width,
                        metadata.signed,
                        None,
                    ),
                    name: member.name.as_bytes().to_vec(),
                })
            })
            .collect()
    }

    fn enum_default(metadata: &crate::core::db::EnumTypeMetadata) -> Result<IrExpr, String> {
        let limbs = metadata.width.div_ceil(64) as usize;
        let mut x = vec![0; limbs];
        if !metadata.two_state {
            x.fill(u64::MAX);
            if let Some(last) = x.last_mut() {
                if !metadata.width.is_multiple_of(64) {
                    *last = (1_u64 << (metadata.width % 64)) - 1;
                }
            }
        }
        let value = IrConst::packed(
            vec![0; limbs],
            x,
            vec![0; limbs],
            metadata.width,
            metadata.signed,
            None,
        )
        .map_err(|error| error.to_string())?;
        Ok(IrExpr::new(
            IrExprKind::Const(value),
            metadata.width,
            metadata.signed,
            None,
        ))
    }

    fn enum_step_default() -> IrExpr {
        IrExpr::new(
            IrExprKind::Const(
                IrConst::packed(vec![1], vec![0], vec![0], 32, false, None)
                    .expect("fixed-width enum step literal is valid"),
            ),
            32,
            false,
            None,
        )
    }

    /// Lower the numeric enum methods. Type-only methods deliberately leave
    /// their receiver unevaluated, while navigation evaluates it once.
    pub(super) fn lower_enum_method(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrExpr>, String> {
        let (name, receiver) = match self.kind(node) {
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
                ..
            } => (name.as_str(), *receiver),
            _ => return Ok(None),
        };
        let method = match name {
            "first" => IrEnumMethod::First,
            "last" => IrEnumMethod::Last,
            "next" => IrEnumMethod::Next,
            "prev" => IrEnumMethod::Prev,
            "num" => IrEnumMethod::Num,
            _ => return Ok(None),
        };
        let metadata = self
            .enum_metadata_for_expr(receiver)
            .cloned()
            .ok_or_else(|| format!("enum method {name} has no owned enum metadata in {path}"))?;
        let args = self.node(node).children.get(1..).unwrap_or_default();
        if !matches!(method, IrEnumMethod::Next | IrEnumMethod::Prev) && !args.is_empty() {
            return Err(format!("enum method {name} takes no arguments in {path}"));
        }
        if matches!(method, IrEnumMethod::Next | IrEnumMethod::Prev) && args.len() > 1 {
            return Err(format!(
                "enum method {name} takes at most one argument in {path}"
            ));
        }
        let members = Self::enum_members(&metadata)?;
        if members.is_empty() {
            return Err(format!(
                "enum method {name} has no declared members in {path}"
            ));
        }
        let default = Self::enum_default(&metadata)?;
        let receiver = matches!(method, IrEnumMethod::Next | IrEnumMethod::Prev)
            .then(|| self.lower_expr(path, receiver))
            .transpose()?
            .map(Box::new);
        let step = if matches!(method, IrEnumMethod::Next | IrEnumMethod::Prev) {
            Some(Box::new(match args {
                [] => Self::enum_step_default(),
                [arg] => ir_to_storage(self.lower_expr(path, *arg)?, 32, false, true)?,
                _ => unreachable!(),
            }))
        } else {
            None
        };
        let (width, signed) = if method == IrEnumMethod::Num {
            (32, true)
        } else {
            (metadata.width, metadata.signed)
        };
        Ok(Some(IrExpr::new(
            IrExprKind::EnumMethod(Box::new(IrEnumQuery {
                method,
                receiver,
                step,
                members,
                default,
            })),
            width,
            signed,
            None,
        )))
    }

    /// Lower name to an owned string expression with declaration names
    /// captured for the receiver's enum type.
    fn lower_enum_name(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrStringExpr>, String> {
        let (name, receiver) = match self.kind(node) {
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
                ..
            } => (name.as_str(), *receiver),
            _ => return Ok(None),
        };
        if name != "name" {
            return Ok(None);
        }
        let args = self.node(node).children.get(1..).unwrap_or_default();
        if !args.is_empty() {
            return Err(format!("enum method name takes no arguments in {path}"));
        }
        let metadata = self
            .enum_metadata_for_expr(receiver)
            .cloned()
            .ok_or_else(|| format!("enum method name has no owned enum metadata in {path}"))?;
        let members = Self::enum_members(&metadata)?;
        if members.is_empty() {
            return Err(format!(
                "enum method name has no declared members in {path}"
            ));
        }
        Ok(Some(IrStringExpr::EnumName {
            receiver: Box::new(self.lower_expr(path, receiver)?),
            members,
        }))
    }

    /// Resolve a chandle lvalue to its native pointer identity and its
    /// caller-owned pointer slot. The address is never an integer encoding.
    pub(super) fn lower_chandle_lvalue(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<(ChandleTarget, IrChandleExpr), String> {
        let read = self.lower_chandle(path, node)?;
        if let Some(address) = self.class_field_chandle_lvalue(path, node)? {
            return Ok((ChandleTarget::Local(format!("*({address})")), read));
        }
        let target = match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            _ => Some(node),
        };
        let mapped = self.func.as_ref().and_then(|function| {
            target
                .and_then(|target| function.chandle_write.get(&target).cloned())
                .or_else(|| {
                    matches!(
                        self.kind(node),
                        NodeKind::Expr(ExprKind::Ref { target: None })
                    )
                    .then(|| {
                        function
                            .chandle_write
                            .iter()
                            .find(|(target, _)| self.node(**target).name == self.node(node).name)
                            .map(|(_, target)| target.clone())
                    })
                    .flatten()
                })
        });
        if let Some(target) = mapped {
            return Ok((target, read));
        }
        if let Some(index) = self.object_of(path, node) {
            if matches!(
                self.model.objects[index].ty,
                IrObjectType::Chandle | IrObjectType::Semaphore
            ) {
                return Ok((ChandleTarget::Object(index), read));
            }
        }
        if let Some(target) = self.semaphore_lvalue_target(path, node)? {
            return Ok((target, read));
        }
        Err("chandle output/ref actual must be a named chandle lvalue".to_owned())
    }

    /// Resolve a procedural semaphore's pointer slot for assignments and
    /// output/ref actuals. Procedural automatic storage is emitted as a C
    /// local, while static storage is represented by a runtime-owned object.
    fn semaphore_lvalue_target(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<ChandleTarget>, String> {
        let target = match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            _ => Some(node),
        };
        let declaration = self.lexical_proc_semaphore_decl(node).or_else(|| {
            target.filter(|target| {
                matches!(
                    self.kind(*target),
                    NodeKind::Var { ty }
                        if ty.kind == "class"
                            && ty.type_name.as_deref() == Some("semaphore")
                )
            })
        });
        let Some(declaration) = declaration else {
            return Ok(None);
        };
        if let Some(name) = self.proc_semaphore_local_name(declaration) {
            return Ok(Some(ChandleTarget::Local(name.to_owned())));
        }
        if self.db.variable_lifetime(declaration) == VariableLifetime::Static {
            return Ok(Some(ChandleTarget::Object(
                self.collect_semaphore_static_object(path, declaration)?,
            )));
        }
        Ok(None)
    }

    pub(super) fn chandle_target_address(&self, target: &ChandleTarget) -> String {
        match target {
            ChandleTarget::Object(index) => format!("&{}", self.model.objects[*index].c_name),
            ChandleTarget::Local(name) if name.starts_with('*') => format!("&({name})"),
            ChandleTarget::Local(name) => format!("&{name}"),
        }
    }

    pub(super) fn lower_chandle(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<IrChandleExpr, String> {
        if let Some(value) = self.lower_container_chandle_query(path, node)? {
            return Ok(value);
        }
        if let NodeKind::Expr(ExprKind::Cast { operand, ty, .. }) = self.kind(node) {
            if is_handle_kind(&ty.kind) {
                return self.lower_chandle(path, *operand);
            }
        }
        if let NodeKind::Expr(ExprKind::NewClass {
            class_name,
            class_type,
            constructor,
            is_super_class,
        }) = self.kind(node)
        {
            return self.lower_new_class(
                path,
                node,
                class_name.as_deref(),
                *class_type,
                *constructor,
                *is_super_class,
            );
        }
        if matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::Constant {
                const_type: ConstantType::Null,
                ..
            })
        ) {
            return Ok(IrChandleExpr::Null);
        }
        if let NodeKind::Expr(ExprKind::ScopeRef { target }) = self.kind(node) {
            if let Some((descriptor, instance)) = self.virtual_interface_instances.get(target) {
                let env = self
                    .model
                    .virtual_interfaces
                    .get(*descriptor)
                    .and_then(|interface| interface.instances.get(*instance))
                    .ok_or_else(|| {
                        format!(
                            "virtual interface target `{}` has no runtime environment in `{path}`",
                            self.node(*target).full_name
                        )
                    })?;
                return Ok(IrChandleExpr::Verbatim(format!("(void *)&{}", env.c_name)));
            }
            if matches!(
                self.kind(*target),
                NodeKind::ModuleInst {
                    is_interface: true,
                    ..
                }
            ) {
                return Err(format!(
                    "virtual interface target `{}` has no compatible runtime descriptor in `{path}`",
                    self.node(*target).full_name
                ));
            }
        }
        if let Some(address) = self.class_field_chandle_lvalue(path, node)? {
            return Ok(IrChandleExpr::Verbatim(address));
        }
        let target = match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            _ => Some(node),
        };
        if let Some(captured) = self
            .capture_target(node)
            .or_else(|| target.filter(|target| self.capture_locals.contains_key(target)))
        {
            let binding = self
                .capture_binding(captured)
                .expect("capture target must have a binding");
            if binding.storage.kind() == StorageKind::Opaque {
                return Ok(IrChandleExpr::LocalRead(Codegen::capture_local_name(
                    binding.storage,
                )));
            }
        }
        if let Some(function) = &self.func {
            if let Some(value) = target.and_then(|target| function.chandle_read.get(&target)) {
                return Ok(value.clone());
            }
            if matches!(
                self.kind(node),
                NodeKind::Expr(ExprKind::Ref { target: None })
            ) {
                if let Some((_, value)) = function
                    .chandle_read
                    .iter()
                    .find(|(target, _)| self.node(**target).name == self.node(node).name)
                {
                    return Ok(value.clone());
                }
            }
        }
        // Module/class object storage is collected before procedural locals.
        // Resolve it first so a top-level semaphore is not mistaken for a
        // second hidden procedural object for the same declaration.
        if let Some(index) = self.object_of(path, node) {
            if matches!(
                self.model.objects[index].ty,
                IrObjectType::Chandle | IrObjectType::Semaphore
            ) {
                return Ok(IrChandleExpr::Read(index));
            }
        }
        if let Some(variable) = self.lexical_proc_semaphore_decl(node) {
            if let Some(name) = self.proc_semaphore_local_name(variable) {
                return Ok(IrChandleExpr::LocalRead(name.to_owned()));
            }
            if self.db.variable_lifetime(variable) == VariableLifetime::Static {
                let index = self.collect_semaphore_static_object(path, variable)?;
                return Ok(IrChandleExpr::Read(index));
            }
        }
        if let Some(variable) = target.filter(|target| {
            matches!(
                self.kind(*target),
                NodeKind::Var { ty }
                    if ty.kind == "class" && ty.type_name.as_deref() == Some("semaphore")
            )
        }) {
            if let Some(name) = self.proc_semaphore_local_name(variable) {
                return Ok(IrChandleExpr::LocalRead(name.to_owned()));
            }
            if self.db.variable_lifetime(variable) == VariableLifetime::Static {
                let index = self.collect_semaphore_static_object(path, variable)?;
                return Ok(IrChandleExpr::Read(index));
            }
        }
        if let NodeKind::FuncCall {
            is_task: false,
            callee,
            ..
        } = self.kind(node)
        {
            let (ft, _callee_inst) =
                self.resolve_callee_env(self.inst, &self.node(node).name, false, *callee)?;
            let meta = self
                .func_meta
                .get(&ft)
                .cloned()
                .ok_or_else(|| format!("function `{}` has no C name", self.node(ft).name))?;
            if !meta.ret_chandle {
                return Err(format!(
                    "function `{}` does not return chandle",
                    self.node(ft).name
                ));
            }
            let args = self.node(node).children.clone();
            let bound = self.bind_call_args(self.inst, &meta.formals, &args)?;
            let mut out_args = Vec::new();
            let mut in_args = Vec::new();
            let mut arg_codes = vec![None; meta.formals.len()];
            let mut arg_irs = vec![None; meta.formals.len()];
            for (idx, (formal, is_out)) in meta.formals.iter().enumerate() {
                let is_chandle = matches!(
                    self.kind(*formal),
                    NodeKind::FuncArg { ty, .. } if is_handle_kind(&ty.kind)
                );
                let is_ref = matches!(
                    self.kind(*formal),
                    NodeKind::FuncArg {
                        direction: DbDirection::Ref,
                        ..
                    }
                );
                if is_ref || *is_out {
                    if !is_chandle {
                        return Err(format!(
                            "chandle function `{}` does not support packed output/ref formals",
                            self.node(ft).name
                        ));
                    }
                    let (target, _) = self.lower_chandle_lvalue(path, bound[idx].expr)?;
                    let address = self.chandle_target_address(&target);
                    if is_ref {
                        out_args.push(IrCallArg::ChandleRefAddr(address));
                    } else {
                        out_args.push(IrCallArg::ChandleAddr(address));
                    }
                } else if is_chandle {
                    in_args.push(IrCallArg::ChandleVal(
                        self.lower_chandle(path, bound[idx].expr)?,
                    ));
                } else {
                    let (_, value) = self.lower_bound_arg_code(
                        path,
                        &meta.formals,
                        &bound,
                        idx,
                        &mut arg_codes,
                        &mut arg_irs,
                    )?;
                    in_args.push(IrCallArg::Val(value));
                }
            }
            out_args.extend(in_args);
            return Ok(IrChandleExpr::Call {
                function: meta.ir,
                args: out_args,
                depth: parse_depth(&self.depth_arg),
            });
        }
        Err(format!(
            "chandle values can only be copied from chandle or null (node {:?} `{}` in `{path}`)",
            self.kind(node),
            self.node(node).name
        ))
    }

    pub(super) fn lower_string(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<IrStringExpr, String> {
        if let NodeKind::MethodCall {
            name,
            receiver: Some(receiver),
            ..
        } = self.kind(node)
        {
            if name == "get_randstate"
                && self.is_process_rng_receiver(*receiver)
                && self.node(node).children.len() == 1
            {
                return Ok(IrStringExpr::RandomState);
            }
        }
        if let Some(value) = self.lower_container_string_query(path, node)? {
            return Ok(value);
        }
        let target = match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            _ => Some(node),
        };
        if let Some((_, name)) = self.lexical_proc_string_local(node) {
            return Ok(IrStringExpr::LocalRead(name.to_owned()));
        }
        if let Some(function) = &self.func {
            if let Some(value) = target.and_then(|target| function.string_read.get(&target)) {
                return Ok(value.clone());
            }
            if matches!(
                self.kind(node),
                NodeKind::Expr(ExprKind::Ref { target: None })
            ) {
                if let Some((_, value)) = function
                    .string_read
                    .iter()
                    .find(|(target, _)| self.node(**target).name == self.node(node).name)
                {
                    return Ok(value.clone());
                }
            }
        }
        if let NodeKind::FuncCall {
            is_task: false,
            callee,
            ..
        } = self.kind(node)
        {
            let (ft, _) =
                self.resolve_callee_env(self.inst, &self.node(node).name, false, *callee)?;
            let meta = self
                .func_meta
                .get(&ft)
                .cloned()
                .ok_or_else(|| format!("function `{}` has no C name", self.node(ft).name))?;
            if !meta.ret_string {
                return Err(format!(
                    "function `{}` does not return string",
                    self.node(ft).name
                ));
            }
            let formals = meta.formals.clone();
            let actuals = self.node(node).children.clone();
            let bound = self.bind_call_args(self.inst, &formals, &actuals)?;
            let mut arg_codes = vec![None; formals.len()];
            let mut arg_irs = vec![None; formals.len()];
            let mut out_args = Vec::new();
            let mut in_args = Vec::new();
            for (idx, (io, is_out)) in formals.iter().enumerate() {
                let is_ref = matches!(
                    self.kind(*io),
                    NodeKind::FuncArg {
                        direction: DbDirection::Ref,
                        ..
                    }
                );
                if is_ref {
                    if !bound[idx].string {
                        return Err(format!(
                            "string-returning function `{}` only supports string ref formals",
                            self.node(ft).name
                        ));
                    }
                    let const_ref = matches!(
                        self.kind(*io),
                        NodeKind::FuncArg {
                            const_ref: true,
                            ..
                        }
                    );
                    if !const_ref {
                        self.ensure_string_actual_writable(path, bound[idx].expr)?;
                    }
                    let addr = self.lower_string_actual_address(path, bound[idx].expr)?;
                    out_args.push(IrCallArg::StringRefAddr { addr, const_ref });
                } else if *is_out {
                    if !bound[idx].string {
                        return Err(format!(
                            "string-returning function `{}` does not support packed output formals",
                            self.node(ft).name
                        ));
                    }
                    self.ensure_string_actual_writable(path, bound[idx].expr)?;
                    let writeback = self.lower_string_actual_address(path, bound[idx].expr)?;
                    let init = matches!(
                        self.kind(*io),
                        NodeKind::FuncArg {
                            direction: DbDirection::Inout,
                            ..
                        }
                    )
                    .then(|| self.lower_string(path, bound[idx].expr))
                    .transpose()?;
                    let (storage_addr, storage_read) = self
                        .static_string_formals
                        .get(&(self.inst, *io))
                        .map(|object| {
                            (
                                Some(format!("&{}", self.model.objects[*object].c_name)),
                                Some(Box::new(IrStringExpr::Read(*object))),
                            )
                        })
                        .unwrap_or((None, None));
                    out_args.push(IrCallArg::StringOutTemp {
                        name: format!("_st{}_{}", node.0, idx),
                        init: init.map(Box::new),
                        writeback,
                        storage_addr,
                        storage_read,
                    });
                } else if bound[idx].string {
                    in_args.push(IrCallArg::StringVal(
                        self.lower_string(path, bound[idx].expr)?,
                    ));
                } else {
                    let (_, arg) = self.lower_bound_arg_code(
                        path,
                        &formals,
                        &bound,
                        idx,
                        &mut arg_codes,
                        &mut arg_irs,
                    )?;
                    in_args.push(IrCallArg::Val(arg));
                }
            }
            out_args.extend(in_args);
            let typed = out_args.iter().any(|arg| {
                matches!(
                    arg,
                    IrCallArg::StringVal(_)
                        | IrCallArg::StringOutAddr(_)
                        | IrCallArg::StringRefAddr { .. }
                        | IrCallArg::StringOutTemp { .. }
                )
            });
            if !typed {
                let args = out_args
                    .into_iter()
                    .map(|arg| match arg {
                        IrCallArg::Val(value) => Ok(value),
                        _ => Err("internal non-packed argument in string call".to_owned()),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                return Ok(IrStringExpr::Call {
                    function: meta.ir,
                    args,
                    depth: parse_depth(&self.depth_arg),
                });
            }
            return Ok(IrStringExpr::TypedCall {
                function: meta.ir,
                args: out_args,
                depth: parse_depth(&self.depth_arg),
            });
        }
        if let Some(index) = self.object_of(path, node) {
            if self.model.objects[index].ty == IrObjectType::String {
                return Ok(IrStringExpr::Read(index));
            }
            return Err("chandle cannot be converted to string".to_owned());
        }
        match self.kind(node) {
            NodeKind::SysCall { name } if name == "$sformatf" => {
                let args = self.node(node).children.clone();
                let Some((format, values)) = args.split_first() else {
                    return Err(format!("$sformatf requires a format argument in `{path}`"));
                };
                let format = self.lower_string(path, *format)?;
                let args = values
                    .iter()
                    .map(|value| self.lower_format_arg(path, *value))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(IrStringExpr::Format {
                    format: Box::new(format),
                    args,
                    scope: path.to_owned(),
                })
            }
            NodeKind::SysCall { name } if name == "$typename" => {
                let [argument] = self.node(node).children.as_slice() else {
                    return Err(format!("$typename requires exactly one argument in `{path}`"));
                };
                let descriptor = self.query_descriptor(*argument).ok_or_else(|| {
                    format!(
                        "$typename argument has no owned type metadata in `{path}`"
                    )
                })?;
                Ok(IrStringExpr::Literal(descriptor.name.as_bytes().to_vec()))
            }
            NodeKind::Expr(ExprKind::Constant { const_type: ConstantType::String, value, .. }) => Ok(IrStringExpr::Literal(decoded_string_bytes(value)?)),
            NodeKind::Expr(ExprKind::Ref {target:Some(target)}) if matches!(self.kind(*target),NodeKind::Param {ty,..} if ty.kind=="string") => self.lower_string(path,*target),
            NodeKind::Param {ty,value,..} if ty.kind=="string" => {
                match self.param_vals.get(&node).or(value.as_ref()) {
                    Some(Val::Str(value))=>Ok(IrStringExpr::Literal(decode_verilog_string(value)?)),
                    _=>Err("string parameter has no captured string value".to_owned()),
                }
            }
            NodeKind::Expr(ExprKind::Cast { operand, ty, .. }) if ty.kind == "string" => {
                let operand = *operand;
                if self.is_string_expr(path,operand) { return self.lower_string(path,operand); }
                if let NodeKind::Expr(ExprKind::Constant {
                    const_type: ConstantType::String,
                    value,
                    ..
                }) = self.kind(operand)
                {
                    return Ok(IrStringExpr::Literal(decoded_string_bytes(value)?));
                }
                let value = self.lower_expr(path,operand)?;
                if value.is_real() { return Err("real to string cast is unsupported".to_owned()); }
                Ok(IrStringExpr::FromPacked(Box::new(value)))
            }
            NodeKind::Expr(ExprKind::Operation {op,operands,reordered,..}) if matches!(op, Operation::Concat | Operation::MultiConcat) => {
                let repeat = *op == Operation::MultiConcat;
                let mut operands = operands.clone();
                if *reordered { operands.reverse(); }
                let count = if repeat {
                    if operands.len() < 2 { return Err("malformed string replication".to_owned()); }
                    Some(self.lower_expr(path, operands.remove(0))?)
                } else { None };
                let parts = operands.into_iter().map(|operand| self.lower_string(path,operand)).collect::<Result<Vec<_>,_>>()?;
                let result = IrStringExpr::Concat(parts);
                Ok(match count {Some(count) => IrStringExpr::Repeat(Box::new(result),Box::new(count)),None => result})
            }
            NodeKind::MethodCall { name, receiver: Some(receiver), .. } => {
                if name == "name" {
                    if let Some(value) = self.lower_enum_name(path, node)? {
                        return Ok(value);
                    }
                }
                let name = name.clone(); let receiver = *receiver;
                let args = self.node(node).children[1..].to_vec();
                let value = Box::new(self.lower_string(path,receiver)?);
                match (name.as_str(),args.as_slice()) {
                    ("toupper",[]) => Ok(IrStringExpr::Case(value,true)),
                    ("tolower",[]) => Ok(IrStringExpr::Case(value,false)),
                    ("substr",[first,last]) => Ok(IrStringExpr::Substr(value,Box::new(self.object_int_argument(path,*first,32)?),Box::new(self.object_int_argument(path,*last,32)?))),
                    _ => Err(format!("unsupported string method or argument count: {name}")),
                }
            }
            _ => Err(format!("string assignment requires a string expression, literal, or explicit cast in `{path}`")),
        }
    }

    /// Lower one argument retained by the shared typed formatter.  Native
    /// strings remain owned string expressions and reals never pass through a
    /// packed conversion.
    pub(super) fn lower_format_arg(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<IrDisplayArg, String> {
        if self.is_string_expr(path, node)
            || matches!(
                self.kind(node),
                NodeKind::Expr(ExprKind::Constant {
                    const_type: ConstantType::String,
                    ..
                })
            )
            || self
                .query_descriptor(node)
                .is_some_and(|descriptor| descriptor.shape == TypeShape::String)
        {
            return Ok(IrDisplayArg::String(self.lower_string(path, node)?));
        }
        let value = self.lower_expr(path, node)?;
        Ok(if value.is_real() {
            IrDisplayArg::Real(value)
        } else {
            IrDisplayArg::Packed(value)
        })
    }

    pub(super) fn lower_object_query(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrExpr>, String> {
        let query = match self.kind(node) {
            NodeKind::Expr(ExprKind::Operation { op, operands, .. })
                if *op == Operation::LogicalNot
                    && operands.len() == 1
                    && self.is_chandle_expr(path, operands[0]) =>
            {
                (
                    IrObjectQuery::ChandleEq(
                        self.lower_chandle(path, operands[0])?,
                        IrChandleExpr::Null,
                    ),
                    1,
                    false,
                )
            }
            NodeKind::Expr(ExprKind::BitSelect { base, index })
                if self.is_string_expr(path, *base) =>
            {
                let (base, index) = (*base, *index);
                (
                    IrObjectQuery::StringGetc(
                        self.lower_string(path, base)?,
                        Box::new(self.object_int_argument(path, index, 32)?),
                    ),
                    8,
                    true,
                )
            }
            NodeKind::Expr(ExprKind::Cast { operand, ty, .. })
                if self.is_string_expr(path, *operand) && ty.kind != "string" =>
            {
                let operand = *operand;
                let ty = ty.clone();
                let width = ty
                    .width
                    .ok_or("string cast requires resolved integral target width")?;
                (
                    IrObjectQuery::StringPacked(self.lower_string(path, operand)?),
                    width,
                    ty.signed,
                )
            }
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
                ..
            } if name == "try_get" && self.is_semaphore_expr(path, *receiver) => {
                let receiver = *receiver;
                let args = self.node(node).children.get(1..).unwrap_or_default();
                let keys = match args {
                    [] => lhs_integer_expr(1),
                    [value] => self.semaphore_key_argument(path, *value)?,
                    _ => {
                        return Err(format!(
                            "semaphore try_get takes zero or one key-count argument in `{path}`"
                        ))
                    }
                };
                (
                    IrObjectQuery::SemaphoreTryGet(self.lower_chandle(path, receiver)?, keys),
                    32,
                    true,
                )
            }
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
                ..
            } if name == "status" && self.is_process_expr(path, *receiver) => {
                let receiver = *receiver;
                let args = self.node(node).children.get(1..).unwrap_or_default();
                if !args.is_empty() {
                    return Err(format!("process status takes no arguments in `{path}`"));
                }
                return Ok(Some(object_query(
                    IrObjectQuery::ProcessStatus(self.lower_process(path, receiver)?),
                    32,
                    false,
                )));
            }
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
                ..
            } if self.is_string_expr(path, *receiver) => {
                let name = name.clone();
                let receiver = *receiver;
                let args = self.node(node).children[1..].to_vec();
                let value = self.lower_string(path, receiver)?;
                match (name.as_str(), args.as_slice()) {
                    ("len", []) => (IrObjectQuery::StringLen(value), 32, true),
                    ("atoreal", []) => (IrObjectQuery::StringAtoreal(value), REAL_EXPR_WIDTH, true),
                    ("getc", [index]) => (
                        IrObjectQuery::StringGetc(
                            value,
                            Box::new(self.object_int_argument(path, *index, 32)?),
                        ),
                        8,
                        true,
                    ),
                    ("compare" | "icompare", [other]) => (
                        IrObjectQuery::StringCompare(
                            value,
                            self.lower_string(path, *other)?,
                            name == "icompare",
                        ),
                        32,
                        true,
                    ),
                    ("atoi" | "atohex" | "atooct" | "atobin", []) => (
                        IrObjectQuery::StringAtoi(
                            value,
                            match name.as_str() {
                                "atohex" => 16,
                                "atooct" => 8,
                                "atobin" => 2,
                                _ => 10,
                            },
                        ),
                        32,
                        true,
                    ),
                    _ => {
                        return Err(format!(
                            "unsupported string value method or argument count: {name}"
                        ))
                    }
                }
            }
            NodeKind::Expr(ExprKind::Operation { op, operands, .. })
                if operands.len() == 2 && *op != Operation::Inside =>
            {
                let op = *op;
                let (a, b) = (operands[0], operands[1]);
                // `null` is shared by process, semaphore, class, and chandle
                // types. Let the non-null operand select the equality domain.
                let is_process = [a, b].iter().any(|node| {
                    !matches!(
                        self.kind(*node),
                        NodeKind::Expr(ExprKind::Constant {
                            const_type: ConstantType::Null,
                            ..
                        })
                    ) && self.is_process_expr(path, *node)
                });
                if is_process {
                    if !matches!(
                        op,
                        Operation::Equal
                            | Operation::NotEqual
                            | Operation::CaseEqual
                            | Operation::CaseNotEqual
                    ) {
                        return Err("operator is not valid for process handle".to_owned());
                    }
                    let value = object_query(
                        IrObjectQuery::ProcessEq(
                            self.lower_process(path, a)?,
                            self.lower_process(path, b)?,
                        ),
                        1,
                        false,
                    );
                    return Ok(Some(
                        if matches!(op, Operation::NotEqual | Operation::CaseNotEqual) {
                            IrExpr::new(
                                IrExprKind::Un {
                                    op: IrUnOp::LogNot,
                                    a: Box::new(value),
                                },
                                1,
                                false,
                                None,
                            )
                        } else {
                            value
                        },
                    ));
                }
                let is_chandle = [a, b].iter().any(|node| self.is_chandle_expr(path, *node));
                if is_chandle {
                    if matches!(op, Operation::LogicalAnd | Operation::LogicalOr) {
                        let a = self.lower_boolean_expr(path, a)?;
                        let b = self.lower_boolean_expr(path, b)?;
                        return Ok(Some(IrExpr::new(
                            IrExprKind::Bin {
                                op: if op == Operation::LogicalAnd {
                                    IrBinOp::LogAnd
                                } else {
                                    IrBinOp::LogOr
                                },
                                a: Box::new(a),
                                b: Box::new(b),
                            },
                            1,
                            false,
                            None,
                        )));
                    }
                    if !matches!(
                        op,
                        Operation::Equal
                            | Operation::NotEqual
                            | Operation::CaseEqual
                            | Operation::CaseNotEqual
                    ) {
                        return Err("operator is not valid for chandle".to_owned());
                    }
                    let value = object_query(
                        IrObjectQuery::ChandleEq(
                            self.lower_chandle(path, a)?,
                            self.lower_chandle(path, b)?,
                        ),
                        1,
                        false,
                    );
                    return Ok(Some(
                        if matches!(op, Operation::NotEqual | Operation::CaseNotEqual) {
                            IrExpr::new(
                                IrExprKind::Un {
                                    op: IrUnOp::LogNot,
                                    a: Box::new(value),
                                },
                                1,
                                false,
                                None,
                            )
                        } else {
                            value
                        },
                    ));
                }
                if !self.is_string_expr(path, a) && !self.is_string_expr(path, b) {
                    return Ok(None);
                }
                let bin = match op {
                    Operation::Equal => IrBinOp::Eq,
                    Operation::NotEqual => IrBinOp::Neq,
                    Operation::Less => IrBinOp::Lt,
                    Operation::LessEqual => IrBinOp::Le,
                    Operation::Greater => IrBinOp::Gt,
                    Operation::GreaterEqual => IrBinOp::Ge,
                    _ => return Err("operator is not valid for string".to_owned()),
                };
                let compare = object_query(
                    IrObjectQuery::StringCompare(
                        self.lower_string(path, a)?,
                        self.lower_string(path, b)?,
                        false,
                    ),
                    32,
                    true,
                );
                let zero = IrExpr::new(
                    IrExprKind::Const(
                        IrConst::packed(vec![0], vec![], vec![], 32, true, None)
                            .map_err(|e| e.to_string())?,
                    ),
                    32,
                    true,
                    None,
                );
                return Ok(Some(IrExpr::new(
                    IrExprKind::Bin {
                        op: bin,
                        a: Box::new(compare),
                        b: Box::new(zero),
                    },
                    1,
                    false,
                    None,
                )));
            }
            _ => return Ok(None),
        };
        Ok(Some(object_query(query.0, query.1, query.2)))
    }

    pub(super) fn lower_object_assignment(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
        blocking: bool,
        op: Operation,
    ) -> Result<Option<IrStmt>, String> {
        let indexed = match self.kind(lhs) {
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => Some((*base, *index)),
            _ => None,
        };
        let object_node = indexed.map_or(lhs, |(base, _)| base);
        if let Some(field) = self.class_field_target(object_node) {
            if matches!(self.kind(field), NodeKind::Var { ty } if is_handle_kind(&ty.kind)) {
                if indexed.is_some() {
                    return Err("virtual interface handle cannot be indexed".to_owned());
                }
                if !blocking {
                    return Err(
                        "nonblocking assignment to virtual interface handle is not supported"
                            .to_owned(),
                    );
                }
                if op != Operation::Assignment {
                    return Err(
                        "compound assignment to virtual interface handle is unsupported".to_owned(),
                    );
                }
                self.validate_virtual_interface_assignment(object_node, rhs, path)?;
                let address = self
                    .class_field_chandle_lvalue(path, object_node)?
                    .ok_or_else(|| "class virtual interface field has no storage".to_owned())?;
                return Ok(Some(IrStmt::Object(IrObjectStmt::ChandleAssignLocal(
                    address,
                    self.lower_chandle(path, rhs)?,
                ))));
            }
        }
        let target_node = match self.kind(object_node) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            _ => Some(object_node),
        };
        let process_target = self
            .is_process_expr(path, object_node)
            .then(|| self.lower_process_lvalue(path, object_node))
            .transpose()?;
        let mut chandle_target = self.func.as_ref().and_then(|function| {
            target_node
                .and_then(|target| function.chandle_write.get(&target).cloned())
                .or_else(|| {
                    matches!(
                        self.kind(object_node),
                        NodeKind::Expr(ExprKind::Ref { target: None })
                    )
                    .then(|| {
                        function
                            .chandle_write
                            .iter()
                            .find(|(target, _)| {
                                self.node(**target).name == self.node(object_node).name
                            })
                            .map(|(_, target)| target.clone())
                    })
                    .flatten()
                })
        });
        if chandle_target.is_none() {
            chandle_target = self.semaphore_lvalue_target(path, object_node)?;
        }
        let string_target = self.func.as_ref().and_then(|function| {
            target_node
                .and_then(|target| function.string_write.get(&target).cloned())
                .or_else(|| {
                    matches!(
                        self.kind(object_node),
                        NodeKind::Expr(ExprKind::Ref { target: None })
                    )
                    .then(|| {
                        function
                            .string_write
                            .iter()
                            .find(|(target, _)| {
                                self.node(**target).name == self.node(object_node).name
                            })
                            .map(|(_, target)| target.clone())
                    })
                    .flatten()
                })
        });
        let string_target = string_target.or_else(|| {
            self.lexical_proc_string_local(object_node)
                .map(|(_, name)| name.to_owned())
        });
        let string_const_ref = self.func.as_ref().is_some_and(|function| {
            target_node.is_some_and(|target| {
                function.string_read.contains_key(&target)
                    && !function.string_write.contains_key(&target)
            })
        });
        let index = self.object_of(path, object_node);
        if string_const_ref && string_target.is_none() {
            return Err("cannot mutate a const-ref string formal".to_owned());
        }
        if index.is_none()
            && process_target.is_none()
            && chandle_target.is_none()
            && string_target.is_none()
        {
            return Ok(None);
        }
        if !blocking {
            return Err(
                "nonblocking assignment to dynamic string/chandle storage is not supported"
                    .to_owned(),
            );
        }
        if op != Operation::Assignment {
            return Err("compound assignment to non-integral storage is unsupported".to_owned());
        }
        if let Some((target, _)) = process_target {
            if indexed.is_some() {
                return Err("process handle cannot be indexed".to_owned());
            }
            let value = self.lower_process(path, rhs)?;
            let operation = match target {
                ProcessTarget::Object(index) => IrObjectStmt::ProcessAssign(index, value),
                ProcessTarget::Local(name) => IrObjectStmt::ProcessAssignLocal(name, value),
            };
            return Ok(Some(IrStmt::Object(operation)));
        }
        if let Some(target) = chandle_target {
            if indexed.is_some() {
                return Err("chandle cannot be indexed".to_owned());
            }
            self.validate_virtual_interface_assignment(object_node, rhs, path)?;
            let value = self.lower_chandle(path, rhs)?;
            return Ok(Some(IrStmt::Object(match target {
                ChandleTarget::Object(index) => IrObjectStmt::ChandleAssign(index, value),
                ChandleTarget::Local(name) => IrObjectStmt::ChandleAssignLocal(name, value),
            })));
        }
        if let Some(target) = string_target {
            if let Some((_, position)) = indexed {
                return Ok(Some(IrStmt::Object(IrObjectStmt::StringPutcLocal(
                    target,
                    self.object_int_argument(path, position, 32)?,
                    self.object_int_argument(path, rhs, 8)?,
                ))));
            }
            return Ok(Some(IrStmt::Object(IrObjectStmt::StringAssignLocal(
                target,
                self.lower_string(path, rhs)?,
            ))));
        }
        let index = index.expect("object target checked above");
        let operation = match self.model.objects[index].ty {
            IrObjectType::String => match indexed {
                Some((_, position)) => IrObjectStmt::StringPutc(
                    index,
                    self.object_int_argument(path, position, 32)?,
                    self.object_int_argument(path, rhs, 8)?,
                ),
                None => IrObjectStmt::StringAssign(index, self.lower_string(path, rhs)?),
            },
            IrObjectType::Chandle => {
                if indexed.is_some() {
                    return Err("chandle cannot be indexed".to_owned());
                }
                self.validate_virtual_interface_assignment(object_node, rhs, path)?;
                IrObjectStmt::ChandleAssign(index, self.lower_chandle(path, rhs)?)
            }
            IrObjectType::Semaphore => {
                if indexed.is_some() {
                    return Err("semaphore handle cannot be indexed".to_owned());
                }
                IrObjectStmt::ChandleAssign(index, self.lower_chandle(path, rhs)?)
            }
            IrObjectType::Process => {
                if indexed.is_some() {
                    return Err("process handle cannot be indexed".to_owned());
                }
                IrObjectStmt::ProcessAssign(index, self.lower_process(path, rhs)?)
            }
        };
        Ok(Some(IrStmt::Object(operation)))
    }

    pub(super) fn lower_object_method(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<IrStmt, String> {
        let (name, receiver) = match self.kind(node) {
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
                ..
            } => (name.clone(), *receiver),
            _ => return Err("object method has no receiver".to_owned()),
        };
        if self.is_process_expr(path, receiver)
            && matches!(name.as_str(), "kill" | "suspend" | "resume" | "await")
        {
            let args = self.node(node).children.get(1..).unwrap_or_default();
            if !args.is_empty() {
                return Err(format!(
                    "process method `{name}` takes no arguments in `{path}`"
                ));
            }
            if matches!(name.as_str(), "suspend" | "await")
                && self.func.as_ref().is_some_and(|function| !function.is_task)
            {
                return Err(format!(
                    "process method `{name}` inside a function body in `{path}` is not supported"
                ));
            }
            let target = self.lower_process(path, receiver)?;
            return match name.as_str() {
                "kill" => Ok(IrStmt::Object(IrObjectStmt::ProcessControl {
                    op: IrProcessControl::Kill,
                    target,
                })),
                "suspend" => Ok(IrStmt::Object(IrObjectStmt::ProcessControl {
                    op: IrProcessControl::Suspend,
                    target,
                })),
                "resume" => Ok(IrStmt::Object(IrObjectStmt::ProcessControl {
                    op: IrProcessControl::Resume,
                    target,
                })),
                "await" => Ok(IrStmt::Object(IrObjectStmt::ProcessAwait(target))),
                _ => Err(format!("unsupported process method: {name}")),
            };
        }
        if self.is_process_rng_receiver(receiver) {
            let args = self.node(node).children.get(1..).unwrap_or_default();
            return match (name.as_str(), args) {
                ("srandom", [seed]) => {
                    let seed = self.lower_expr(path, *seed)?;
                    if seed.is_real() {
                        return Err(format!("srandom seed must be integral in {path}"));
                    }
                    Ok(IrStmt::RandomSeed {
                        seed: IrExpr::convert_to(seed, 32, false),
                    })
                }
                ("set_randstate", [state]) => Ok(IrStmt::RandomStateSet {
                    state: self.lower_string(path, *state)?,
                }),
                ("srandom", _) | ("set_randstate", _) => Err(format!(
                    "{} requires exactly one argument in {}",
                    name, path
                )),
                _ => Err(format!("unsupported process random method: {name}")),
            };
        }
        if self.is_semaphore_expr(path, receiver) {
            let args = self.node(node).children.get(1..).unwrap_or_default();
            let keys = match args {
                [] => lhs_integer_expr(1),
                [value] => self.semaphore_key_argument(path, *value)?,
                _ => {
                    return Err(format!(
                        "semaphore method `{name}` takes zero or one key-count argument in `{path}`"
                    ))
                }
            };
            let receiver = self.lower_chandle(path, receiver)?;
            return match name.as_str() {
                "put" => Ok(IrStmt::Object(IrObjectStmt::SemaphorePut(receiver, keys))),
                "get" => Ok(IrStmt::Object(IrObjectStmt::SemaphoreGet(receiver, keys))),
                _ => Err(format!("unsupported semaphore method: {name}")),
            };
        }
        let index = self.object_of(path, receiver);
        let local = if index.is_none() {
            let target = match self.kind(receiver) {
                NodeKind::Expr(ExprKind::Ref {
                    target: Some(target),
                }) => Some(*target),
                _ => Some(receiver),
            };
            self.func
                .as_ref()
                .and_then(|function| {
                    target
                        .and_then(|target| function.string_write.get(&target).cloned())
                        .or_else(|| {
                            function
                                .string_write
                                .iter()
                                .find(|(target, _)| {
                                    self.node(**target).name == self.node(receiver).name
                                })
                                .map(|(_, value)| value.clone())
                        })
                })
                .or_else(|| {
                    self.lexical_proc_string_local(receiver)
                        .map(|(_, name)| name.to_owned())
                })
        } else {
            None
        };
        if index.is_none() && local.is_none() {
            let const_ref = self.func.as_ref().is_some_and(|function| {
                let target = match self.kind(receiver) {
                    NodeKind::Expr(ExprKind::Ref {
                        target: Some(target),
                    }) => Some(*target),
                    _ => Some(receiver),
                };
                target.is_some_and(|target| {
                    function.string_read.contains_key(&target)
                        && !function.string_write.contains_key(&target)
                })
            });
            if const_ref {
                return Err("cannot mutate a const-ref string formal".to_owned());
            }
            return Err("unsupported object method receiver".to_owned());
        }
        if let Some(index) = index {
            if self.model.objects[index].ty != IrObjectType::String {
                return Err("chandle has no built-in methods".to_owned());
            }
        }
        let args = self.node(node).children[1..].to_vec();
        let operation = match (name.as_str(), args.as_slice()) {
            ("realtoa", [value]) => {
                let value = self.lower_expr(path, *value)?;
                let value = if value.is_real() {
                    value
                } else {
                    IrExpr::new(
                        IrExprKind::CastToReal {
                            a: Box::new(value),
                            shortreal: false,
                        },
                        REAL_EXPR_WIDTH,
                        true,
                        None,
                    )
                };
                match index {
                    Some(index) => IrObjectStmt::StringRealtoa(index, value),
                    None => IrObjectStmt::StringRealtoaLocal(local.clone().unwrap(), value),
                }
            }
            ("putc", [position, value]) => match index {
                Some(index) => IrObjectStmt::StringPutc(
                    index,
                    self.object_int_argument(path, *position, 32)?,
                    self.object_int_argument(path, *value, 8)?,
                ),
                None => IrObjectStmt::StringPutcLocal(
                    local.clone().unwrap(),
                    self.object_int_argument(path, *position, 32)?,
                    self.object_int_argument(path, *value, 8)?,
                ),
            },
            ("itoa" | "hextoa" | "octtoa" | "bintoa", [value]) => {
                let value = ir_to_storage(self.lower_expr(path, *value)?, 32, true, false)?;
                let base = match name.as_str() {
                    "hextoa" => 16,
                    "octtoa" => 8,
                    "bintoa" => 2,
                    _ => 10,
                };
                match index {
                    Some(index) => IrObjectStmt::StringItoa(index, value, base),
                    None => IrObjectStmt::StringItoaLocal(local.clone().unwrap(), value, base),
                }
            }
            _ => {
                return Err(format!(
                    "unsupported string statement method or argument count: {name}"
                ))
            }
        };
        Ok(IrStmt::Object(operation))
    }
}

pub(super) fn object_query(query: IrObjectQuery, width: u32, signed: bool) -> IrExpr {
    IrExpr::new(
        IrExprKind::ObjectQuery(Box::new(query)),
        width,
        signed,
        None,
    )
}
