//! Runtime-owned semaphore/mailbox handles and explicitly transferred messages.
use super::native::{NativeKind, NativeValue};
use super::*;

// Only leaf operands are evaluated during preparation. The message constructor
// is emitted at the consuming runtime boundary, after every user expression.
struct Message {
    code: String,
    numeric: Vec<Value>,
    native: Vec<NativeValue>,
}

impl Frame<'_, '_> {
    pub(super) fn mailbox_handle(&mut self, expression: &IrMailboxExpr) -> Result<String, String> {
        Ok(match expression {
            IrMailboxExpr::Null => "NULL".to_owned(),
            IrMailboxExpr::Read(handle) => self.chandle(handle)?,
            IrMailboxExpr::New { bound, element } => {
                if self.read_only_callback {
                    return Err(pending("mailbox construction in read-only callbacks"));
                }
                let bound = self.expression(bound)?;
                let (kind, width, signed, two_state, shortreal) = match element {
                    IrMailboxElement::Untyped => (4, 0, false, false, false),
                    IrMailboxElement::Packed {
                        width,
                        signed,
                        two_state,
                    } => (0, *width, *signed, *two_state, false),
                    IrMailboxElement::Real { shortreal } => (1, 0, false, false, *shortreal),
                    IrMailboxElement::String => (2, 0, false, false, false),
                    IrMailboxElement::Handle => (3, 0, false, false, false),
                    IrMailboxElement::Value { type_id } => {
                        let handle = self.scalar(
                            "void*",
                            format!("llg_mailbox_new_value({}, UINT64_C({type_id}))", bound.code),
                        );
                        self.discard(bound);
                        return Ok(handle);
                    }
                };
                let handle = self.scalar(
                    "void*",
                    format!(
                        "llg_mailbox_new({}, {kind}, {width}, {}, {}, {})",
                        bound.code,
                        u8::from(signed),
                        u8::from(two_state),
                        u8::from(shortreal)
                    ),
                );
                self.discard(bound);
                handle
            }
        })
    }

    fn mailbox_message(&mut self, value: &IrMailboxValue) -> Result<Message, String> {
        let mut message = Message {
            code: String::new(),
            numeric: Vec::new(),
            native: Vec::new(),
        };
        message.code = match value {
            IrMailboxValue::Typed { type_id, value } => {
                let mut message = self.mailbox_message(value)?;
                message.code = format!(
                    "llg_mailbox_typed_value({}, UINT64_C({type_id}))",
                    message.code
                );
                return Ok(message);
            }
            IrMailboxValue::Packed { value, two_state } => {
                let value = self.expression(value)?;
                let code = format!(
                    "llg_mailbox_value_packed({}, {}, {}, {})",
                    value.code,
                    value.width,
                    u8::from(value.signed),
                    u8::from(*two_state)
                );
                message.numeric.push(value);
                code
            }
            IrMailboxValue::Real { value, shortreal } => {
                let value = self.expression(value)?;
                let code = format!(
                    "llg_mailbox_value_real({}, {})",
                    value.real(),
                    u8::from(*shortreal)
                );
                message.numeric.push(value);
                code
            }
            IrMailboxValue::String(value) => {
                let value = self.string(value)?;
                let code = format!("llg_mailbox_value_string({})", value.take_string());
                message.native.push(value);
                code
            }
            IrMailboxValue::Handle(value) => {
                format!("llg_mailbox_value_handle({})", self.chandle(value)?)
            }
            IrMailboxValue::Native { value, type_id } => format!(
                "llg_mailbox_value_native({}, UINT64_C({type_id}))",
                self.native_value_address(*value)?
            ),
        };
        Ok(message)
    }

    fn release_message_operands(&mut self, message: Message) {
        for value in message.numeric {
            self.discard(value);
        }
        for value in message.native {
            self.native_discard(value);
        }
    }

    fn mailbox_destination(&mut self, target: &IrMailboxTarget) -> Result<String, String> {
        Ok(match target {
            IrMailboxTarget::Typed { type_id, target } => {
                let target = self.mailbox_destination(target)?;
                format!("llg_mailbox_typed_target({target}, UINT64_C({type_id}))")
            }
            IrMailboxTarget::Ref { addr } => {
                format!("llg_mailbox_target_ref({})", self.reference_address(addr)?)
            }
            IrMailboxTarget::Packed {
                addr,
                width,
                signed,
                two_state,
            } => {
                let binding = self.address(addr)?;
                if binding.width != *width {
                    return Err("mailbox target width mismatch".to_owned());
                }
                format!(
                    "llg_mailbox_target_packed({}, {width}, {}, {})",
                    binding.address,
                    u8::from(*signed),
                    u8::from(*two_state)
                )
            }
            IrMailboxTarget::Real { addr, shortreal } => {
                let binding = self.address(addr)?;
                if binding.width != 0 {
                    return Err("mailbox real target is not real storage".to_owned());
                }
                format!(
                    "llg_mailbox_target_real({}, {})",
                    binding.address,
                    u8::from(*shortreal)
                )
            }
            IrMailboxTarget::String { addr } => format!(
                "llg_mailbox_target_string({})",
                self.native_address(addr, NativeKind::String)?.address
            ),
            IrMailboxTarget::Handle { addr } => format!(
                "llg_mailbox_target_handle({})",
                self.native_address(addr, NativeKind::Chandle)?.address
            ),
            IrMailboxTarget::Native { value, type_id } => format!(
                "llg_mailbox_target_native({}, UINT64_C({type_id}))",
                self.native_value_address(*value)?
            ),
        })
    }

    pub(super) fn mailbox_query(
        &mut self,
        query: &IrObjectQuery,
        expression: &IrExpr,
    ) -> Result<Value, String> {
        let cancellation_mark = self.cancellation_mark();
        use IrObjectQuery::*;
        if self.read_only_callback && !matches!(query, MailboxEq(..) | MailboxNum(..)) {
            return Err(pending("synchronization mutation in read-only callbacks"));
        }
        let code = match query {
            SemaphoreTryGet(handle, keys) => {
                let handle = self.chandle(handle)?;
                let keys = self.expression(keys)?;
                let status = self.scalar(
                    "int",
                    format!(
                        "llg_semaphore_try_get((llg_semaphore_t*){handle}, {})",
                        keys.code
                    ),
                );
                self.discard(keys);
                status
            }
            MailboxNum(handle) => {
                let handle = self.chandle(handle)?;
                format!("llg_mailbox_num((llg_mailbox_t*){handle})")
            }
            MailboxEq(left, right) => {
                let left = self.mailbox_handle(left)?;
                let right = self.mailbox_handle(right)?;
                format!("({left} == {right})")
            }
            MailboxTryPut { mailbox, value } => {
                let handle = self.chandle(mailbox)?;
                let message = self.mailbox_message(value)?;
                let status = self.scalar(
                    "int",
                    format!(
                        "llg_mailbox_try_put_value((llg_mailbox_t*){handle}, {})",
                        message.code
                    ),
                );
                self.release_message_operands(message);
                status
            }
            MailboxTryGet {
                mailbox,
                target,
                peek,
            } => {
                let handle = self.chandle(mailbox)?;
                let target = self.mailbox_destination(target)?;
                self.scalar(
                    "int",
                    format!(
                        "llg_mailbox_try_get_value((llg_mailbox_t*){handle}, {target}, {})",
                        u8::from(*peek)
                    ),
                )
            }
            _ => return Err(pending("object query ownership contract")),
        };
        // Identity comparisons are one-bit unsigned; method statuses are
        // signed `int` (Annex G.4). The value carries the expression's sign.
        let code = if expression.signed {
            format!("sv4_from_i64((int64_t)({code}), {})", expression.width)
        } else {
            format!("sv4_from_u64((uint64_t)({code}), {}, 0)", expression.width)
        };
        let result = self.value(code, expression.width, expression.signed);
        self.cancellation_check_covering(cancellation_mark)?;
        Ok(result)
    }

    pub(super) fn mailbox_statement(&mut self, statement: &IrObjectStmt) -> Result<(), String> {
        let cancellation_mark = self.cancellation_mark();
        use IrObjectStmt::*;
        if self.read_only_callback {
            return Err(pending("synchronization statements in read-only callbacks"));
        }
        match statement {
            SemaphorePut(receiver, keys) | SemaphoreGet(receiver, keys) => {
                let receiver = self.chandle(receiver)?;
                let keys = self.expression(keys)?;
                if matches!(statement, SemaphorePut(..)) {
                    self.line(format!(
                        "llg_semaphore_put((llg_semaphore_t*){receiver}, {});",
                        keys.code
                    ));
                } else {
                    self.await_arm(
                        SuspensionOperation::SemaphoreGet,
                        format!(
                            "llg_arm_semaphore_get(self, (llg_semaphore_t*){receiver}, {})",
                            keys.code
                        ),
                    )?;
                    self.line("llg_semaphore_grant_taken(self);");
                }
                self.discard(keys);
            }
            MailboxAssign(index, value) => {
                let value = self.mailbox_handle(value)?;
                self.line(format!(
                    "{} = {value};",
                    self.ctx.model.objects[*index].c_name
                ));
            }
            MailboxAssignLocal(name, value) => {
                let target = self.native_lookup(name, NativeKind::Chandle)?;
                let value = self.mailbox_handle(value)?;
                self.line(format!("*({}) = {value};", target.address));
            }
            MailboxPut(_, mailbox, value, attempt)
            | MailboxPutLocal(_, mailbox, value, attempt) => {
                self.mailbox_put(mailbox, value, *attempt)?;
            }
            MailboxTryPut(_, mailbox, value) | MailboxTryPutLocal(_, mailbox, value) => {
                self.mailbox_put(mailbox, value, true)?;
            }
            MailboxGet(_, mailbox, target, peek)
            | MailboxGetLocal(_, mailbox, target, peek)
            | MailboxTryGet(_, mailbox, target, peek)
            | MailboxTryGetLocal(_, mailbox, target, peek) => {
                let mailbox = self.chandle(mailbox)?;
                let target = self.mailbox_destination(target)?;
                if matches!(statement, MailboxTryGet(..) | MailboxTryGetLocal(..)) {
                    self.line(format!(
                        "(void)llg_mailbox_try_get_value((llg_mailbox_t*){mailbox}, {target}, {});",
                        u8::from(*peek)
                    ));
                } else {
                    self.await_arm(
                        SuspensionOperation::MailboxGet,
                        format!(
                            "llg_arm_mailbox_get_value(self, (llg_mailbox_t*){mailbox}, {target}, {})",
                            u8::from(*peek)
                        ),
                    )?;
                    // A suspended get writes its destination on resume, so
                    // the write lands in live storage (SIM-017).
                    self.line(format!("llg_mailbox_delivery_take(self, {target});"));
                }
            }
            _ => return Err(pending("object statement ownership contract")),
        }
        self.cancellation_check_covering(cancellation_mark)
    }

    fn mailbox_put(
        &mut self,
        mailbox: &IrChandleExpr,
        value: &IrMailboxValue,
        attempt: bool,
    ) -> Result<(), String> {
        let mailbox = self.chandle(mailbox)?;
        let message = self.mailbox_message(value)?;
        if attempt {
            self.line(format!(
                "(void)llg_mailbox_try_put_value((llg_mailbox_t*){mailbox}, {});",
                message.code
            ));
        } else {
            self.await_arm(
                SuspensionOperation::MailboxPut,
                format!(
                    "llg_arm_mailbox_put_value(self, (llg_mailbox_t*){mailbox}, {})",
                    message.code
                ),
            )?;
        }
        self.release_message_operands(message);
        Ok(())
    }
}
