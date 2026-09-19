//! Native scalar operations. No object pointer is encoded as packed bits.
use super::native::{NativeKind, NativeValue};
use super::*;
mod queries;

impl Frame<'_, '_> {
    pub(super) fn chandle(&mut self, value: &IrChandleExpr) -> Result<String, String> {
        let code = match value {
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
                let container = self.ctx.model.containers[*container].clone();
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
            IrChandleExpr::ContainerGetNested { container, indices } => {
                let container = self.ctx.model.containers[*container].clone();
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
                let name = self.ctx.model.containers[*container].c_name.clone();
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

    fn process_value(&mut self, expression: &IrProcessExpr) -> Result<NativeValue, String> {
        let source = match expression {
            IrProcessExpr::Null => "NULL".to_owned(),
            IrProcessExpr::SelfHandle => "llg_process_self()".to_owned(),
            IrProcessExpr::Read(index) => self.ctx.model.objects[*index].c_name.clone(),
            IrProcessExpr::LocalRead(name) => format!(
                "*({})",
                self.native_lookup(name, NativeKind::Process)?.address
            ),
            IrProcessExpr::FormalRead(_) => return Err(pending("process-handle formal ABI")),
        };
        let result = self.native_reserve(NativeKind::Process);
        self.line(format!("llg_process_assign({}, {source});", result.address));
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
            HandleCapture(_) => {
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
            ChandleDeclareLocal(name, initializer) => {
                let binding = self.native_local(name, NativeKind::Chandle);
                if let Some(initializer) = initializer {
                    let value = self.chandle(initializer)?;
                    self.line(format!("*({}) = {value};", binding.address));
                }
            }
            ChandleAssign(index, value) => {
                let value = self.chandle(value)?;
                self.line(format!(
                    "{} = {value};",
                    self.ctx.model.objects[*index].c_name
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
                let value = self.process_value(source)?;
                self.line(format!(
                    "llg_process_assign(&{}, {});",
                    self.ctx.model.objects[*index].c_name,
                    value.code()
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
                let function = match op {
                    IrProcessControl::Kill => "llg_process_kill",
                    IrProcessControl::Suspend => "llg_process_suspend",
                    IrProcessControl::Resume => "llg_process_resume",
                };
                self.line(format!("{function}({});", target.code()));
                self.native_discard(target);
            }
            ProcessAwait(target) => {
                let target = self.process_value(target)?;
                self.line(format!("llg_process_await({});", target.code()));
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
