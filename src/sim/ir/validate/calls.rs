//! Calls.

use super::*;

impl Validator<'_> {
    pub(super) fn validate_call_expr(
        &self,
        call: &IrCallExpr,
        formals: &[IrFormal],
        path: &str,
    ) -> ValidationResult {
        self.validate_call_target(call.f, &call.args, formals, path, false)?;
        let callee = &self.model.funcs[call.f];
        if let Some(virtual_call) = &call.virtual_call {
            self.validate_virtual_call(call.f, virtual_call, formals, path)?;
            if call.receiver.is_some() {
                return self.fail(path, "virtual-interface call cannot carry a class receiver");
            }
        } else if callee.receiver_class.is_some() {
            let receiver = call
                .receiver
                .as_ref()
                .ok_or_else(|| IrValidationError::new(path, "class method call has no receiver"))?;
            receiver.validate(self.model, formals, self.chandle_return.get())?;
        } else if call.receiver.is_some() {
            return self.fail(path, "non-method call cannot carry a receiver");
        }
        for (idx, arg) in call.args.iter().enumerate() {
            if let IrCallArg::OutTemp {
                init,
                writeback,
                storage_lhs,
                storage_read,
                selector_inits,
                ..
            } = arg
            {
                if let Some(init) = init {
                    self.validate_expr(init, formals, &format!("{path}.args[{idx}].init"))?;
                }
                self.validate_lhs(writeback, formals, &format!("{path}.args[{idx}].writeback"))?;
                if let Some(storage_lhs) = storage_lhs {
                    self.validate_lhs(
                        storage_lhs,
                        formals,
                        &format!("{path}.args[{idx}].storage_lhs"),
                    )?;
                }
                if let Some(storage_read) = storage_read {
                    self.validate_expr(
                        storage_read,
                        formals,
                        &format!("{path}.args[{idx}].storage_read"),
                    )?;
                }
                for (selector_idx, (_, width, signed, two_state, init)) in
                    selector_inits.iter().enumerate()
                {
                    if *width == 0 {
                        return self.fail(
                            format!("{path}.args[{idx}].selector_inits[{selector_idx}]"),
                            "selector initializer must be packed",
                        );
                    }
                    self.validate_width(
                        *width,
                        &format!("{path}.args[{idx}].selector_inits[{selector_idx}].width"),
                    )?;
                    if *two_state && init.is_real() {
                        return self.fail(
                            format!("{path}.args[{idx}].selector_inits[{selector_idx}]"),
                            "two-state selector initializer cannot be real",
                        );
                    }
                    if init.width != *width || init.signed != *signed {
                        return self.fail(
                            format!("{path}.args[{idx}].selector_inits[{selector_idx}]"),
                            "selector initializer type disagrees with its capture",
                        );
                    }
                    self.validate_expr(
                        init,
                        formals,
                        &format!("{path}.args[{idx}].selector_inits[{selector_idx}].init"),
                    )?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn validate_virtual_call(
        &self,
        function: usize,
        call: &crate::sim::ir::IrVirtualCall,
        formals: &[IrFormal],
        path: &str,
    ) -> ValidationResult {
        let interface = self
            .model
            .virtual_interfaces
            .get(call.interface)
            .ok_or_else(|| {
                IrValidationError::new(
                    path,
                    format!(
                        "virtual-interface descriptor {} is out of bounds",
                        call.interface
                    ),
                )
            })?;
        let method = interface.methods.get(call.method).ok_or_else(|| {
            IrValidationError::new(
                path,
                format!("virtual-interface method {} is out of bounds", call.method),
            )
        })?;
        if method.function != function {
            return self.fail(
                path,
                "virtual-interface method function disagrees with call target",
            );
        }
        call.receiver
            .validate(self.model, formals, self.chandle_return.get())
    }

    pub(super) fn validate_call_target(
        &self,
        function: usize,
        args: &[IrCallArg],
        formals: &[IrFormal],
        path: &str,
        allow_object_return: bool,
    ) -> ValidationResult {
        let callee = self.model.funcs.get(function).ok_or_else(|| {
            IrValidationError::new(path, format!("function index {function} is out of bounds"))
        })?;
        if (!allow_object_return && (callee.ret_chandle || callee.ret_string))
            || callee.formals.iter().any(|formal| formal.event)
        {
            return self.fail(path, "non-integral subprogram requires its typed call path");
        }
        if args.len() != callee.formals.len() {
            return self.fail(
                path,
                format!(
                    "call has {} arguments for {} formals",
                    args.len(),
                    callee.formals.len()
                ),
            );
        }
        let parameter_order = callee
            .formals
            .iter()
            .filter(|formal| formal.is_address())
            .chain(callee.formals.iter().filter(|formal| !formal.is_address()));
        for (idx, (arg, formal)) in args.iter().zip(parameter_order).enumerate() {
            let arg_path = format!("{path}.args[{idx}]");
            match arg {
                IrCallArg::Val(_) if formal.is_address() => {
                    return self.fail(arg_path, "address formal requires an address argument");
                }
                IrCallArg::Val(expr) => {
                    if formal.chandle {
                        return self
                            .fail(arg_path, "chandle formal requires a typed pointer value");
                    }
                    self.validate_expr(expr, formals, &arg_path)?;
                    if formal.real != expr.is_real()
                        || expr.width != formal.width
                        || expr.signed != formal.signed
                    {
                        return self
                            .fail(arg_path, "input argument type disagrees with its formal");
                    }
                }
                IrCallArg::StringVal(value) => {
                    if formal.string && !formal.is_address() {
                        value.validate(self.model, self.string_return.get())?;
                    } else {
                        return self.fail(arg_path, "string value requires a string input formal");
                    }
                }
                IrCallArg::StringOutAddr(addr) => {
                    if !formal.string || !formal.is_out || formal.is_ref() {
                        return self.fail(arg_path, "string address requires output/inout formal");
                    }
                    if addr.is_empty() {
                        return self.fail(arg_path, "string output address must not be empty");
                    }
                }
                IrCallArg::StringRefAddr { addr, const_ref } => {
                    if !formal.string || !formal.is_ref() {
                        return self
                            .fail(arg_path, "string reference requires a string ref formal");
                    }
                    if addr.is_empty() || (*const_ref && !formal.const_ref) {
                        return self.fail(arg_path, "invalid string reference descriptor");
                    }
                }
                IrCallArg::ChandleVal(value) => {
                    if formal.chandle && !formal.is_address() {
                        value.validate(self.model, formals, None).map_err(|error| {
                            IrValidationError::new(arg_path.clone(), error.to_string())
                        })?;
                    } else {
                        return self.fail(arg_path, "typed chandle value requires an input formal");
                    }
                }
                IrCallArg::ChandleAddr(addr) => {
                    if !formal.chandle || !formal.is_out || formal.is_ref() {
                        return self.fail(arg_path, "typed chandle address requires output/inout");
                    }
                    if addr.is_empty() {
                        return self.fail(arg_path, "chandle output address must not be empty");
                    }
                }
                IrCallArg::ChandleRefAddr(addr) => {
                    if !formal.chandle || !formal.is_ref() {
                        return self.fail(arg_path, "typed chandle reference requires ref formal");
                    }
                    if addr.is_empty() {
                        return self.fail(arg_path, "chandle reference address must not be empty");
                    }
                }
                IrCallArg::RefAddr {
                    addr,
                    width,
                    signed,
                    two_state,
                    const_ref,
                    lhs,
                    read,
                } => {
                    if !formal.is_ref() {
                        return self
                            .fail(arg_path, "output/inout formal requires an output address");
                    }
                    if addr.is_empty() {
                        return self.fail(arg_path, "reference address must not be empty");
                    }
                    if *width != formal.width
                        || *signed != formal.signed
                        || *two_state != formal.two_state
                    {
                        return self.fail(
                            arg_path,
                            "reference argument type disagrees with its formal",
                        );
                    }
                    if *const_ref && !formal.const_ref {
                        return self.fail(
                            arg_path,
                            "const reference cannot bind to a writable ref formal",
                        );
                    }
                    self.validate_ref_actual_lhs(
                        lhs,
                        formals,
                        &format!("{arg_path}.lhs"),
                        formal.const_ref,
                    )?;
                    self.validate_expr(read, formals, &format!("{arg_path}.read"))?;
                    if read.width != *width || read.signed != *signed {
                        return self.fail(
                            format!("{arg_path}.read"),
                            "reference read type disagrees with its descriptor",
                        );
                    }
                }
                IrCallArg::OutAddr(_) | IrCallArg::OutTemp { .. } if formal.is_ref() => {
                    return self.fail(arg_path, "ref formal requires a reference descriptor");
                }
                IrCallArg::OutAddr(_) | IrCallArg::OutTemp { .. } if !formal.is_address() => {
                    return self.fail(arg_path, "input formal requires a value argument");
                }
                IrCallArg::OutTemp {
                    storage_addr: Some(addr),
                    ..
                } if addr.is_empty() => {
                    return self.fail(
                        arg_path,
                        "persistent output storage address must not be empty",
                    );
                }
                IrCallArg::OutTemp {
                    init: Some(init), ..
                } => {
                    if formal.real != init.is_real()
                        || init.width != formal.width
                        || init.signed != formal.signed
                    {
                        return self.fail(
                            format!("{arg_path}.init"),
                            "output/inout temp initializer type disagrees with its formal",
                        );
                    }
                }
                IrCallArg::StringOutTemp {
                    name,
                    init,
                    writeback,
                    storage_addr,
                    storage_read,
                } => {
                    if !formal.string || !formal.is_out || formal.is_ref() {
                        return self.fail(arg_path, "string temp requires output/inout formal");
                    }
                    if name.is_empty() || writeback.is_empty() {
                        return self.fail(arg_path, "string output temp has empty storage");
                    }
                    if let Some(init) = init {
                        init.validate(self.model, self.string_return.get())?;
                    }
                    if let Some(addr) = storage_addr {
                        if addr.is_empty() {
                            return self
                                .fail(arg_path, "string persistent storage address is empty");
                        }
                    }
                    if let Some(read) = storage_read {
                        read.validate(self.model, self.string_return.get())?;
                    }
                }
                IrCallArg::OutAddr(_) | IrCallArg::OutTemp { init: None, .. } => {}
            }
        }
        Ok(())
    }

    fn validate_ref_actual_lhs(
        &self,
        lhs: &IrLhs,
        formals: &[IrFormal],
        path: &str,
        allow_const: bool,
    ) -> ValidationResult {
        match lhs {
            IrLhs::PackedSelect { target, steps, .. } => {
                self.validate_ref_actual_lhs(
                    target,
                    formals,
                    &format!("{path}.target"),
                    allow_const,
                )?;
                if self.lhs_packed_width(target).is_none() {
                    return self.fail(path, "reference view requires packed backing storage");
                }
                self.validate_elem_sel(&IrElemSel::PackedChain(steps.clone()), formals, path)
            }
            IrLhs::Ref {
                addr,
                width,
                bit,
                const_ref,
                ..
            } => {
                if *const_ref && !allow_const {
                    return self.fail(path, "const reference cannot bind to a writable ref formal");
                }
                if bit.is_some() {
                    return self.fail(path, "packed bit selects cannot be passed by reference");
                }
                if addr.is_empty() {
                    return self.fail(path, "reference descriptor address must not be empty");
                }
                self.validate_width(*width, path)
            }
            _ => self.validate_lhs(lhs, formals, path),
        }
    }
}
