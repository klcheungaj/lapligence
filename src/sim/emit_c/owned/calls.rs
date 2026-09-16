//! Calls keep argument owners registered across possible coroutine suspension.
use super::*;

impl Frame<'_, '_> {
    fn call_values(&mut self, f: usize, args: &[IrCallArg], depth: IrDepth) -> Result<Option<Value>, String> {
        if !self.allow_calls { return Err(pending("subprogram calls in declaration initialization")); }
        let function = self.ctx.model.func(f);
        model::check_function(function)?;
        let marker = self.name("call_mark");
        self.line(format!("llg_value_scope_t* {marker} = llg_value_scope_mark();"));
        self.bindings.push(HashMap::new());
        let mut parameters = Vec::new();
        let mut owners = Vec::new();
        let mut copyouts = Vec::new();
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
                _ => return Err(pending("native-object or reference call arguments")),
            }
        }
        parameters.push(depth.code().to_owned());
        let invocation = format!("{}({})", function.c_name, parameters.join(", "));
        let result = if let Some(ty) = function.ret {
            Some(self.value(invocation, ty.width(), ty.signed()))
        } else { self.line(format!("{invocation};")); None };
        for value in owners { self.discard(value); }
        for (target, storage) in copyouts {
            let value = self.read_binding(&storage);
            self.store(&target, value, false, "0")?;
            self.release_target(target);
        }
        self.bindings.pop();
        self.line(format!("llg_value_scopes_end_since({marker});"));
        Ok(result)
    }

    pub(super) fn call_expression(&mut self, call: &IrCallExpr) -> Result<Value, String> {
        if call.receiver.is_some() || call.virtual_call.is_some() || call.virtual_dispatch {
            return Err(pending("method and interface calls"));
        }
        Ok(match self.call_values(call.f, &call.args, call.depth)? {
            Some(value) => value,
            None if call.void_x => self.value("sv4_x(1, 0)".to_owned(), 1, false),
            None => return Err("void call has no expression value".to_owned()),
        })
    }

    pub(super) fn call_statement(&mut self, call: &IrCall) -> Result<(), String> {
        if call.receiver.is_some() || call.virtual_call.is_some() || call.virtual_dispatch {
            return Err(pending("method and interface calls"));
        }
        self.begin_block(&[]);
        let function = self.ctx.model.func(call.f);
        for (name, index, initial) in &call.temps {
            let formal = &function.formals[*index];
            self.local(name, if formal.real { 0 } else { formal.width }, formal.signed, formal.two_state, initial.as_ref())?;
        }
        let mut captured = Vec::new();
        for (lhs, name, width, signed) in &call.copyouts {
            captured.push((self.target(lhs)?, name, *width, *signed));
        }
        if let Some(value) = self.call_values(call.f, &call.args, call.depth)? { self.discard(value); }
        for (target, name, width, signed) in captured {
            let binding = self.lookup(name).ok_or_else(|| format!("unknown copyout {name}"))?;
            let value = self.read_binding(&binding);
            let value = self.convert(value, width, signed, false, false);
            self.store(&target, value, false, "0")?;
            self.release_target(target);
        }
        self.end_block();
        Ok(())
    }
}
