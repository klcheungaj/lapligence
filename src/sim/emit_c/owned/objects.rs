//! Native scalar operations. No object pointer is encoded as packed bits.
use super::native::{NativeKind, NativeValue};
use super::*;
mod queries;

impl Frame<'_, '_> {
    pub(super) fn chandle(&mut self, value: &IrChandleExpr) -> Result<String, String> {
        let code = match value {
            IrChandleExpr::Conditional {
                predicate,
                then,
                otherwise,
            } => {
                let selector = self.expression(predicate)?;
                let result = self.scalar("void*", "NULL".to_owned());
                self.line(format!("if ({}) {{", selector.truth()));
                let left = self.chandle(then)?;
                self.line(format!("{result} = {left};"));
                if selector.width == 0 {
                    self.line("} else {");
                    let right = self.chandle(otherwise)?;
                    self.line(format!("{result} = {right};"));
                } else {
                    self.line(format!("}} else if (!{}) {{", selector.unknown_truth()));
                    let right = self.chandle(otherwise)?;
                    self.line(format!("{result} = {right};"));
                    self.line("} else {");
                    // SV 11.4.11: both arms are evaluated; unequal chandles
                    // yield null, the type's default-uninitialized value.
                    let left = self.chandle(then)?;
                    let left = self.scalar("void*", left);
                    let right = self.chandle(otherwise)?;
                    self.line(format!("{result} = {left} == ({right}) ? {left} : NULL;"));
                }
                self.line("}");
                self.discard(selector);
                return Ok(result);
            }
            IrChandleExpr::SemaphoreNew(keys) => {
                if self.read_only_callback {
                    return Err(pending("semaphore construction in read-only callbacks"));
                }
                let keys = self.expression(keys)?;
                let handle = self.scalar("void*", format!("llg_semaphore_new({})", keys.code));
                self.discard(keys);
                return Ok(handle);
            }
            IrChandleExpr::Construct(index) => return self.construct_class(*index),
            IrChandleExpr::CopyClass { class, source } => {
                if self.read_only_callback {
                    return Err(pending("class copy in read-only callbacks"));
                }
                let source = self.chandle(source)?;
                return Ok(self.scalar("void*", format!("llg_class_copy_{class}({source})")));
            }
            IrChandleExpr::Required { handle, site } => {
                let handle = self.chandle(handle)?;
                if self.quiet_receivers {
                    return Ok(handle);
                }
                return Ok(self.scalar(
                    "void*",
                    format!("llg_class_require({handle}, {})", c_string_literal(site)),
                ));
            }
            IrChandleExpr::InterfaceInstance {
                interface,
                instance,
            } => {
                format!(
                    "(void*)&{}",
                    self.ctx.model.virtual_interfaces[*interface].instances[*instance].c_name
                )
            }
            IrChandleExpr::LocalRead(name)
                if name == "_this" && self.ctx.func.is_some_and(|f| f.receiver_class.is_some()) =>
            {
                "_this".to_owned()
            }
            IrChandleExpr::Null => "NULL".to_owned(),
            IrChandleExpr::EventObject(event) => {
                let address = self.event_address(event)?;
                let pointer = self.scalar("llg_event_t*", address);
                return Ok(self.scalar(
                    "void*",
                    format!("{pointer} ? (void*){pointer}->object : NULL"),
                ));
            }
            IrChandleExpr::Read(index) => self.ctx.model.objects[*index].c_name.clone(),
            IrChandleExpr::LocalRead(name) => format!(
                "*({})",
                self.native_lookup(name, NativeKind::Chandle)?.address
            ),
            IrChandleExpr::FormalRead(index) => {
                let function = self
                    .ctx
                    .func
                    .ok_or_else(|| "handle formal outside a function".to_owned())?;
                let formal = &function.formals[*index];
                if formal.is_ref() {
                    format!("*r{index}")
                } else if formal.is_out {
                    format!("*o{index}")
                } else {
                    format!(
                        "*({})",
                        self.native_lookup(&format!("a{index}"), NativeKind::Chandle)?
                            .address
                    )
                }
            }
            IrChandleExpr::ContainerGet { container, index } => {
                let container_name = self.container_name(*container)?;
                let mut container = self.ctx.model.containers[*container].clone();
                container.c_name = container_name;
                let index = self.expression(index)?;
                let function = match container.kind {
                    IrContainerKind::Dynamic => "llg_dyn_value_get_chandle",
                    IrContainerKind::Queue { .. } => "llg_queue_value_get_chandle",
                    IrContainerKind::Associative { .. } => "llg_assoc_value_get_integral_chandle",
                };
                let value = self.scalar(
                    "void*",
                    format!("{function}(&{}, {})", container.c_name, index.code),
                );
                self.discard(index);
                return Ok(value);
            }
            IrChandleExpr::Mailbox(mailbox) => return self.mailbox_handle(mailbox),
            IrChandleExpr::ContainerElement {
                container,
                indices,
                key,
                write,
            } => return self.container_element(*container, indices, key.as_deref(), *write),
            IrChandleExpr::Process(process) => {
                // The retained temporary stays alive until its lexical scope
                // ends, after the consuming element store has retained it.
                let value = self.process_value(process)?;
                return Ok(self.scalar("void*", format!("(void*)*({})", value.address)));
            }
            IrChandleExpr::PinnedProcess(process) => {
                let value = self.process_value(process)?;
                return Ok(self.scalar(
                    "void*",
                    format!("(void*)llg_process_pin(*({}))", value.address),
                ));
            }
            IrChandleExpr::QueuePop { container, back } => {
                if self.read_only_callback {
                    return Err(pending("mutating container query in a read-only callback"));
                }
                let name = self.container_name(*container)?;
                return Ok(self.scalar(
                    "void*",
                    format!("llg_queue_value_pop_chandle(&{name}, {})", i32::from(*back)),
                ));
            }
            IrChandleExpr::ContainerGetNested { container, indices } => {
                let container_name = self.container_name(*container)?;
                let mut container = self.ctx.model.containers[*container].clone();
                container.c_name = container_name;
                let (list, values) = self.container_indices(indices)?;
                let function = match container.kind {
                    IrContainerKind::Dynamic => "llg_dyn_value_get_nested_chandle",
                    IrContainerKind::Queue { .. } => "llg_queue_value_get_nested_chandle",
                    IrContainerKind::Associative { .. } => {
                        "llg_assoc_value_get_nested_integral_chandle"
                    }
                };
                let value = self.scalar(
                    "void*",
                    format!(
                        "{function}(&{}, {list}, {})",
                        container.c_name,
                        indices.len()
                    ),
                );
                for value in values {
                    self.discard(value);
                }
                return Ok(value);
            }
            IrChandleExpr::AssociativeGet { container, key } => {
                let name = self.container_name(*container)?;
                let key = self.string(key)?;
                let value = self.scalar(
                    "void*",
                    format!(
                        "llg_assoc_value_get_string_chandle(&{name}, ({})->data, ({})->len)",
                        key.address, key.address
                    ),
                );
                self.native_discard(key);
                return Ok(value);
            }
            IrChandleExpr::Call {
                function,
                args,
                depth,
                receiver,
                virtual_dispatch,
            } => {
                let value = self.native_method_call(
                    *function,
                    args,
                    *depth,
                    NativeKind::Chandle,
                    receiver.as_deref(),
                    *virtual_dispatch,
                )?;
                let pointer = self.scalar("void*", value.code());
                self.native_discard(value);
                return Ok(pointer);
            }
            IrChandleExpr::Verbatim(_) => {
                return Err(pending("opaque native allocation/member fragments"))
            }
        };
        Ok(self.scalar("void*", code))
    }

    pub(super) fn process_value(
        &mut self,
        expression: &IrProcessExpr,
    ) -> Result<NativeValue, String> {
        let source = match expression {
            IrProcessExpr::Null => "NULL".to_owned(),
            IrProcessExpr::SelfHandle => "llg_process_self(self)".to_owned(),
            IrProcessExpr::Read(index) => self.ctx.model.objects[*index].c_name.clone(),
            IrProcessExpr::LocalRead(name) => format!(
                "*({})",
                self.native_lookup(name, NativeKind::Process)?.address
            ),
            IrProcessExpr::FormalRead(index) => format!(
                "(llg_process_handle_t*){}",
                self.chandle(&IrChandleExpr::FormalRead(*index))?
            ),
            IrProcessExpr::Handle(handle) => {
                if let IrChandleExpr::QueuePop { container, back } = handle.as_ref() {
                    if self.read_only_callback {
                        return Err(pending("mutating container query in a read-only callback"));
                    }
                    // The popped element's reference moves into the result.
                    let name = self.container_name(*container)?;
                    let result = self.native_reserve(NativeKind::Process);
                    self.line(format!(
                        "llg_queue_value_pop_process_to((void**){}, &{name}, {});",
                        result.address,
                        i32::from(*back)
                    ));
                    return Ok(result);
                }
                format!("(llg_process_handle_t*){}", self.chandle(handle)?)
            }
        };
        let result = self.native_reserve(NativeKind::Process);
        self.line(format!(
            "llg_process_assign_temp({}, {source});",
            result.address
        ));
        Ok(result)
    }

    pub(super) fn object_query(
        &mut self,
        query: &IrObjectQuery,
        expression: &IrExpr,
    ) -> Result<Value, String> {
        use IrObjectQuery::*;
        Ok(match query {
            StringInside { value, items } => self.string_inside(value, items)?,
            StringLen(text) | StringAtoi(text, _) | StringAtoreal(text) | StringPacked(text) => {
                let text = self.string(text)?;
                let code = match query {
                    StringLen(_) => format!("llg_string_len({})", text.take_string()),
                    StringAtoi(_, base) => {
                        format!("llg_string_atoi({}, {base})", text.take_string())
                    }
                    StringAtoreal(_) => format!("llg_string_atoreal({})", text.take_string()),
                    _ => format!(
                        "llg_string_to_packed({}, {}, {})",
                        text.take_string(),
                        expression.width,
                        u8::from(expression.signed)
                    ),
                };
                let result = self.value(code, expression.width, expression.signed);
                self.native_discard(text);
                result
            }
            StringGetc(text, index) => {
                let text = self.string(text)?;
                let index = self.expression(index)?;
                let result = self.value(
                    format!("llg_string_getc({}, {})", text.take_string(), index.code),
                    expression.width,
                    expression.signed,
                );
                self.native_discard(text);
                self.discard(index);
                result
            }
            StringCompare(left, right, ignore_case) => {
                let left = self.string(left)?;
                let right = self.string(right)?;
                let result = self.value(
                    format!(
                        "llg_string_compare({}, {}, {})",
                        left.take_string(),
                        right.take_string(),
                        u8::from(*ignore_case)
                    ),
                    expression.width,
                    expression.signed,
                );
                self.native_discard(left);
                self.native_discard(right);
                result
            }
            ChandleEq(left, right) => {
                let left = self.chandle(left)?;
                let right = self.chandle(right)?;
                self.value(format!("sv4_from_u64({left} == {right}, 1, 0)"), 1, false)
            }
            ProcessEq(left, right) => {
                let left = self.process_value(left)?;
                let right = self.process_value(right)?;
                let result = self.value(
                    format!("sv4_from_u64({} == {}, 1, 0)", left.code(), right.code()),
                    1,
                    false,
                );
                self.native_discard(left);
                self.native_discard(right);
                result
            }
            ProcessStatus(handle) => {
                let handle = self.process_value(handle)?;
                let result = self.value(
                    format!("sv4_from_u64(llg_process_status({}), 32, 1)", handle.code()),
                    32,
                    true,
                );
                self.native_discard(handle);
                result
            }
            ArrayQuery(query) => self.array_query(query, expression)?,
            HandleCapture(_) | EventCapture(_) => {
                return Err("opaque capture cannot be used as a packed expression".to_owned())
            }
            _ => return self.mailbox_query(query, expression),
        })
    }

    pub(super) fn object_statement(&mut self, statement: &IrObjectStmt) -> Result<(), String> {
        use IrObjectStmt::*;
        match statement {
            StringPrint(text) => {
                let value = self.string(text)?;
                self.line(format!("llg_string_print({});", value.take_string()));
                self.native_discard(value);
            }
            StringAssign(index, text) => {
                self.string_assign(&format!("&{}", self.ctx.model.objects[*index].c_name), text)?
            }
            StringAssignLocal(name, text) => {
                let address = self.native_lookup(name, NativeKind::String)?.address;
                self.string_assign(&address, text)?;
            }
            StringPutc(index, position, character) => {
                let address = format!("&{}", self.ctx.model.objects[*index].c_name);
                self.string_putc(&address, position, character)?;
            }
            StringPutcLocal(name, position, character) => {
                let address = self.native_lookup(name, NativeKind::String)?.address;
                self.string_putc(&address, position, character)?;
            }
            StringItoa(index, value, base) => {
                let address = format!("&{}", self.ctx.model.objects[*index].c_name);
                self.string_number(&address, value, Some(*base))?;
            }
            StringItoaLocal(name, value, base) => {
                let address = self.native_lookup(name, NativeKind::String)?.address;
                self.string_number(&address, value, Some(*base))?;
            }
            StringRealtoa(index, value) => {
                let address = format!("&{}", self.ctx.model.objects[*index].c_name);
                self.string_number(&address, value, None)?;
            }
            StringRealtoaLocal(name, value) => {
                let address = self.native_lookup(name, NativeKind::String)?.address;
                self.string_number(&address, value, None)?;
            }
            ChandleDeclareShared(name, initializer) => {
                let owner = self.scalar(
                    "llg_frame_t**",
                    "(llg_frame_t**)llg_value_scope_object(llg_value_scope_begin_object(sizeof(llg_frame_t*), llg_owned_frame_drop))"
                        .to_owned(),
                );
                self.line(format!("*{owner} = llg_frame_new(1ULL);"));
                self.line(format!("llg_frame_capture_opaque(*{owner}, 0u, NULL);"));
                let address =
                    self.scalar("void**", format!("llg_frame_opaque_address(*{owner}, 0u)"));
                self.shared_cells
                    .insert(name.clone(), (format!("(*{owner})"), 0));
                self.bind_native(name, address.clone(), NativeKind::Chandle);
                if let Some(initializer) = initializer {
                    let value = self.chandle(initializer)?;
                    self.line(format!("*({address}) = {value};"));
                }
            }
            ChandleDeclareLocal(name, initializer) => {
                let binding = self.native_local(name, NativeKind::Chandle);
                if let Some(initializer) = initializer {
                    let value = self.chandle(initializer)?;
                    self.line(format!("*({}) = {value};", binding.address));
                }
            }
            ChandleAssign(index, value) => {
                let value = self.chandle(value)?;
                let object = &self.ctx.model.objects[*index];
                let name = &object.c_name;
                // Readers wait on a chandle or class handle's change marker
                // (SIM-007); built-in semaphore handles have none.
                if object.ty == crate::sim::ir::IrObjectType::Chandle {
                    self.line(format!(
                        "{{ void *_llg_handle = {value}; if (_llg_handle != {name}) {{ {name} = _llg_handle; llg_dependency_changed(&{name}_llg_dep); }} }}"
                    ));
                } else {
                    self.line(format!("{name} = {value};"));
                }
            }
            ChandleAssignLocal(name, value) if self.class_handle_property(name).is_some() => {
                // Rebinding a handle property toggles its object's marker,
                // which waits on properties selected through it observe.
                let Some(access) = self.class_handle_property(name) else {
                    unreachable!("guarded above");
                };
                let IrNativeAccessKind::ClassField { class, field } = access.kind else {
                    unreachable!("class handle properties are class fields");
                };
                let receiver = self.chandle(&access.receiver)?;
                let receiver = self.scalar("void*", receiver);
                let value = self.chandle(value)?;
                self.line(format!(
                    "llg_class_handle_store({receiver}, {class}, {field}, {value});"
                ));
            }
            ChandleAssignLocal(name, value) => {
                let address = self.native_lookup(name, NativeKind::Chandle)?.address;
                let value = self.chandle(value)?;
                self.line(format!("*({address}) = {value};"));
            }
            ProcessDeclareLocal(name, initializer) => {
                let binding = self.native_local(name, NativeKind::Process);
                if let Some(initializer) = initializer {
                    let value = self.process_value(initializer)?;
                    self.line(format!(
                        "llg_process_assign({}, {});",
                        binding.address,
                        value.code()
                    ));
                    self.native_discard(value);
                }
            }
            ProcessAssign(index, source) => {
                // Readers wait on the handle object's change marker.
                let value = self.process_value(source)?;
                let name = &self.ctx.model.objects[*index].c_name;
                self.line(format!(
                    "if ({value} != {name}) {{ llg_process_assign(&{name}, {value}); llg_dependency_changed(&{name}_llg_dep); }}",
                    value = value.code()
                ));
                self.native_discard(value);
            }
            ProcessAssignLocal(name, source) => {
                let binding = self.native_lookup(name, NativeKind::Process)?;
                let value = self.process_value(source)?;
                self.line(format!(
                    "llg_process_assign({}, {});",
                    binding.address,
                    value.code()
                ));
                self.native_discard(value);
            }
            ProcessControl { op, target } => {
                let target = self.process_value(target)?;
                match op {
                    IrProcessControl::Kill => {
                        self.line(format!("llg_process_kill(self, {});", target.code()))
                    }
                    IrProcessControl::Suspend => self.await_arm(
                        SuspensionOperation::ProcessSuspend,
                        format!("llg_arm_process_suspend(self, {})", target.code()),
                    )?,
                    IrProcessControl::Resume => {
                        self.line(format!("llg_process_resume(self, {});", target.code()))
                    }
                }
                self.native_discard(target);
            }
            ProcessRandom { target, op } => {
                let target = self.process_value(target)?;
                match op {
                    crate::sim::ir::IrProcessRandom::Seed(seed) => {
                        let seed = self.expression(seed)?;
                        self.line(format!(
                            "llg_process_handle_srandom({}, {});",
                            target.code(),
                            seed.code
                        ));
                        self.discard(seed);
                    }
                    crate::sim::ir::IrProcessRandom::SetState(state) => {
                        let state = self.string(state)?;
                        self.line(format!(
                            "(void)llg_process_handle_set_randstate({}, {});",
                            target.code(),
                            state.take_string()
                        ));
                        self.native_discard(state);
                    }
                }
                self.native_discard(target);
            }
            ObjectRandom { target, op } => {
                let target = self.chandle(target)?;
                match op {
                    crate::sim::ir::IrProcessRandom::Seed(seed) => {
                        let seed = self.expression(seed)?;
                        self.line(format!(
                            "llg_object_srandom(llg_class_rng({target}, \"srandom\"), {});",
                            seed.code
                        ));
                        self.discard(seed);
                    }
                    crate::sim::ir::IrProcessRandom::SetState(state) => {
                        let state = self.string(state)?;
                        self.line(format!(
                            "llg_object_set_randstate(llg_class_rng({target}, \"set_randstate\"), {});",
                            state.take_string()
                        ));
                        self.native_discard(state);
                    }
                }
            }
            ProcessAwait(target) => {
                let target = self.process_value(target)?;
                self.await_arm(
                    SuspensionOperation::ProcessAwait,
                    format!("llg_arm_process_await(self, {})", target.code()),
                )?;
                self.native_discard(target);
            }
            _ => return self.mailbox_statement(statement),
        }
        Ok(())
    }
    fn string_putc(
        &mut self,
        address: &str,
        position: &IrExpr,
        character: &IrExpr,
    ) -> Result<(), String> {
        let position = self.expression(position)?;
        let character = self.expression(character)?;
        self.line(format!(
            "llg_string_putc({address}, {}, {});",
            position.code, character.code
        ));
        self.discard(position);
        self.discard(character);
        Ok(())
    }
    fn string_number(
        &mut self,
        address: &str,
        value: &IrExpr,
        base: Option<u32>,
    ) -> Result<(), String> {
        let value = self.expression(value)?;
        self.line(match base {
            Some(base) => format!("llg_string_itoa({address}, {}, {base});", value.code),
            None => format!("llg_string_realtoa({address}, {});", value.real()),
        });
        self.discard(value);
        Ok(())
    }
}

impl Frame<'_, '_> {
    /// The class handle property that native access `name` selects.
    fn class_handle_property(&self, name: &str) -> Option<IrNativeAccess> {
        self.ctx
            .model
            .native_accesses
            .iter()
            .find(|access| access.name == name)
            .filter(|access| match access.kind {
                IrNativeAccessKind::ClassField { class, field } => {
                    let layout = &self.ctx.model.classes[class].fields[field];
                    layout.ty == IrClassFieldType::Chandle
                        && layout.container.is_none()
                        && layout.native_value.is_none()
                }
                _ => false,
            })
            .cloned()
    }

    /// Address of one element of descriptor-backed container storage for an
    /// element-item access. A missing or invalid element resolves to a fresh
    /// default value owned by the current lexical scope, so reads see the
    /// Table 7-1 default and writes to it are discarded (SV 7.4.6, 7.8.6).
    fn container_element(
        &mut self,
        container: usize,
        indices: &[IrExpr],
        key: Option<&IrStringExpr>,
        write: bool,
    ) -> Result<String, String> {
        if write && self.read_only_callback {
            return Err(pending("container element writes in read-only callbacks"));
        }
        let storage = self.ctx.model.containers[container].clone();
        let name = self.container_name(container)?;
        let (call, depth, touch) = match storage.kind {
            IrContainerKind::Associative {
                key: IrAssocKey::String,
            } => {
                let key = key.ok_or("string-keyed element locator requires a key")?;
                let key = self.string(key)?;
                let call = format!(
                    "llg_assoc_value_element_string(&{name}, ({})->data, ({})->len, {})",
                    key.address,
                    key.address,
                    i32::from(write)
                );
                let element = self.scalar("llg_value_t*", call);
                self.native_discard(key);
                (element, 1, "llg_assoc_value_touch")
            }
            kind => {
                let (list, values) = self.container_indices(indices)?;
                let call = match kind {
                    IrContainerKind::Dynamic => {
                        format!("llg_dyn_value_element(&{name}, {list}, {})", indices.len())
                    }
                    IrContainerKind::Queue { .. } => format!(
                        "llg_queue_value_element(&{name}, {list}, {})",
                        indices.len()
                    ),
                    IrContainerKind::Associative { .. } => format!(
                        "llg_assoc_value_element_integral(&{name}, {list}, {}, {})",
                        indices.len(),
                        i32::from(write)
                    ),
                };
                let element = self.scalar("llg_value_t*", call);
                for value in values {
                    self.discard(value);
                }
                let touch = match kind {
                    IrContainerKind::Dynamic => "llg_dyn_value_touch",
                    IrContainerKind::Queue { .. } => "llg_queue_value_touch",
                    IrContainerKind::Associative { .. } => "llg_assoc_value_touch",
                };
                (element, indices.len(), touch)
            }
        };
        self.line(format!(
            "if (!{call}) {{ {call} = (llg_value_t*)llg_value_scope_object(llg_value_scope_begin_object(sizeof(llg_value_t), llg_native_value_destroy)); llg_native_value_init({call}, llg_value_element_desc({name}.element, {depth})); }}"
        ));
        if write {
            self.pending_touches.push((name, touch));
        }
        Ok(call)
    }
}
