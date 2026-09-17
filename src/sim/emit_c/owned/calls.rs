//! Calls keep argument owners registered across possible coroutine suspension.
use super::*;
use super::native::{NativeKind, NativeValue};

pub(super) enum CallValue { Numeric(Value), Native(NativeValue) }


impl Frame<'_, '_> {
    pub(super) fn call_values(&mut self, f: usize, args: &[IrCallArg], depth: IrDepth) -> Result<Option<CallValue>, String> {
        self.call_target(f, args, depth, None, false, None)
    }

    fn call_target(&mut self, f: usize, args: &[IrCallArg], depth: IrDepth,
        receiver: Option<&IrChandleExpr>, virtual_dispatch: bool, virtual_call: Option<&IrVirtualCall>,
    ) -> Result<Option<CallValue>, String> {
        if !self.allow_calls { return Err(pending("subprogram calls in declaration initialization")); }
        let function = self.ctx.model.func(f).clone();
        if model::inline_event_template(&function) {
            return Err("event-formal calls must be inlined before C emission".to_owned());
        }
        model::check_function(&function)?;
        if self.read_only_callback { return Err(pending("native or impure calls in evaluator callbacks")); }
        // A native return slot must precede the call mark: argument cleanup
        // must not invalidate the returned value, including aliased copyouts.
        let native_result = if function.ret_string { Some(self.native_reserve(NativeKind::String)) }
            else if function.ret_chandle { Some(self.native_reserve(NativeKind::Chandle)) } else { None };
        let marker = self.name("call_mark");
        self.line(format!("llg_value_scope_t* {marker} = llg_value_scope_mark();"));
        self.bindings.push(HashMap::new());
        self.native_bindings.push(HashMap::new());
        let mut parameters = Vec::new();
        let mut callee = function.c_name.clone();
        if let Some(call) = virtual_call {
            parameters.push(self.chandle(&call.receiver)?);
            callee = format!("llg_vif_call_{}_{}", call.interface, call.method);
        } else if let Some(receiver) = receiver {
            parameters.push(self.chandle(receiver)?);
            if virtual_dispatch {
                let slot = function.virtual_slot.ok_or_else(|| "virtual call has no slot".to_owned())?;
                callee = format!("llg_class_dispatch_{slot}");
            }
        } else if function.receiver_class.is_some() {
            return Err("class method call has no receiver".to_owned());
        }
        // Tie retained queue-reference cells to the same lexical cleanup stack.
        // This covers normal return, named disable and nonlocal process exit.
        if args.iter().any(|arg| matches!(arg, IrCallArg::RefAddr { .. })) {
            self.line("llg_ref_scope_begin_owned();");
        }
        let mut owners = Vec::new();
        let mut copyouts = Vec::new();
        let mut native_owners = Vec::new();
        let mut string_copyouts = Vec::new();
        // The IR stores arguments in the C ABI order (addresses, inputs).
        // Evaluate this explicit order, never nested C argument expressions.
        let order = function.formals.iter().enumerate().filter(|(_, p)| p.is_address())
            .chain(function.formals.iter().enumerate().filter(|(_, p)| !p.is_address()));
        if args.len() != function.formals.len() { return Err("call arity mismatch".to_owned()); }
        for ((_, formal), argument) in order.zip(args) {
            match argument {
                IrCallArg::Val(expr) => {
                    let value = self.expression(expr)?;
                    let value = self.convert(value, if formal.real { 0 } else { formal.width }, formal.signed, formal.two_state, formal.shortreal);
                    parameters.push(value.code.clone());
                    owners.push(value);
                }
                IrCallArg::StringVal(expression) => {
                    let value = self.string(expression)?;
                    parameters.push(value.code()); native_owners.push(value);
                }
                IrCallArg::ChandleVal(expression) => parameters.push(self.chandle(expression)?),
                IrCallArg::StringOutAddr(address) | IrCallArg::StringRefAddr { addr: address, .. } => {
                    parameters.push(self.native_address(address, NativeKind::String)?.address);
                }
                IrCallArg::ChandleAddr(address) | IrCallArg::ChandleRefAddr(address) => {
                    parameters.push(self.native_address(address, NativeKind::Chandle)?.address);
                }
                IrCallArg::StringOutTemp { name, init, writeback, storage_addr, .. } => {
                    let temporary = self.native_local(name, NativeKind::String);
                    if let Some(init) = init { self.string_assign(&temporary.address, init)?; }
                    let target = self.native_address(writeback, NativeKind::String)?;
                    let storage = if let Some(address) = storage_addr {
                        let storage = self.native_address(address, NativeKind::String)?;
                        self.line(format!("llg_string_move({}, llg_string_clone({}));", storage.address, temporary.address));
                        storage
                    } else { temporary };
                    parameters.push(storage.address.clone());
                    string_copyouts.push((target, storage));
                }
                IrCallArg::OutAddr(addr) => {
                    let binding = self.address(addr)?;
                    if binding.width != if formal.real { 0 } else { formal.width } {
                        return Err("output argument storage width mismatch".to_owned());
                    }
                    parameters.push(binding.address);
                }
                IrCallArg::OutTemp { name, init, writeback, storage_addr, selector_inits, .. } => {
                    for (name, width, signed, two_state, initial) in selector_inits {
                        self.local(name, *width, *signed, *two_state, Some(initial))?;
                    }
                    self.local(name, if formal.real { 0 } else { formal.width }, formal.signed, formal.two_state, init.as_deref())?;
                    let temporary = self.lookup(name).ok_or_else(|| "missing call temporary".to_owned())?;
                    let target = self.target(writeback)?;
                    let storage = if let Some(address) = storage_addr {
                        let storage = self.address(address)?;
                        let initial = self.read_binding(&temporary);
                        let initial = self.convert(initial, storage.width, storage.signed, storage.two_state, storage.shortreal);
                        if storage.width == 0 { self.line(format!("*({}) = {};", storage.address, initial.code)); }
                        else { self.line(format!("sv4_move({}, &{});", storage.address, initial.code)); }
                        self.discard(initial);
                        storage
                    } else { temporary };
                    parameters.push(storage.address.clone());
                    copyouts.push((target, storage));
                }
                IrCallArg::RefAddr { lhs, read, width, signed, two_state, .. } => {
                    parameters.push(self.reference_argument(lhs, read, *width, *signed, *two_state)?);
                }
                _ => return Err(pending("native-object or reference call arguments")),
            }
        }
        parameters.push(depth.code().to_owned());
        let invocation = format!("{callee}({})", parameters.join(", "));
        let result = if let Some(value) = native_result {
            self.line(format!("{} = {invocation};", value.code()));
            Some(CallValue::Native(value))
        } else if let Some(ty) = function.ret {
            Some(CallValue::Numeric(self.value(invocation, ty.width(), ty.signed())))
        } else { self.line(format!("{invocation};")); None };
        self.cancellation_check()?;
        for value in owners { self.discard(value); }
        for (target, storage) in copyouts {
            let value = self.read_binding(&storage);
            self.store(&target, value, false, "0")?;
            self.release_target(target);
            self.cancellation_check()?;
        }
        for (target, storage) in string_copyouts {
            self.line(format!("llg_string_move({}, llg_string_clone({}));", target.address, storage.address));
            self.cancellation_check()?;
        }
        for value in native_owners { self.native_discard(value); }
        self.native_bindings.pop();
        self.bindings.pop();
        self.line(format!("llg_value_scopes_end_since({marker});"));
        Ok(result)
    }

    pub(super) fn native_call(&mut self, function: usize, args: &[IrCallArg], depth: IrDepth, kind: NativeKind) -> Result<NativeValue, String> {
        match self.call_values(function, args, depth)? {
            Some(CallValue::Native(value)) if value.kind == kind => Ok(value),
            _ => Err("native call result type mismatch".to_owned()),
        }
    }

    pub(super) fn native_method_call(&mut self, function: usize, args: &[IrCallArg], depth: IrDepth,
        kind: NativeKind, receiver: Option<&IrChandleExpr>, virtual_dispatch: bool,
    ) -> Result<NativeValue, String> {
        match self.call_target(function, args, depth, receiver, virtual_dispatch, None)? {
            Some(CallValue::Native(value)) if value.kind == kind => Ok(value),
            _ => Err("native method result type mismatch".to_owned()),
        }
    }

    pub(super) fn call_expression(&mut self, call: &IrCallExpr) -> Result<Value, String> {
        Ok(match self.call_target(call.f, &call.args, call.depth, call.receiver.as_ref(), call.virtual_dispatch, call.virtual_call.as_ref())? {
            Some(CallValue::Numeric(value)) => value,
            Some(CallValue::Native(_)) => return Err("native call used as a packed expression".to_owned()),
            None if call.void_x => self.value("sv4_x(1, 0)".to_owned(), 1, false),
            None => return Err("void call has no expression value".to_owned()),
        })
    }

    pub(super) fn call_statement(&mut self, call: &IrCall) -> Result<(), String> {
        self.begin_block(&[]);
        let function = self.ctx.model.func(call.f).clone();
        for (name, index, initial) in &call.temps {
            let formal = &function.formals[*index];
            self.local(name, if formal.real { 0 } else { formal.width }, formal.signed, formal.two_state, initial.as_ref())?;
        }
        let mut captured = Vec::new();
        for (lhs, name, width, signed) in &call.copyouts {
            captured.push((self.target(lhs)?, name, *width, *signed));
        }
        if let Some(value) = self.call_target(call.f, &call.args, call.depth, call.receiver.as_ref(), call.virtual_dispatch, call.virtual_call.as_ref())? {
            match value { CallValue::Numeric(value) => self.discard(value), CallValue::Native(value) => self.native_discard(value) }
        }
        for (target, name, width, signed) in captured {
            let binding = self.lookup(name).ok_or_else(|| format!("unknown copyout {name}"))?;
            let value = self.read_binding(&binding);
            let value = self.convert(value, width, signed, false, false);
            self.store(&target, value, false, "0")?;
            self.release_target(target);
            self.cancellation_check()?;
        }
        self.end_block();
        Ok(())
    }
}
