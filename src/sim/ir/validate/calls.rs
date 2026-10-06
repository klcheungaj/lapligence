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
            || callee
                .formals
                .iter()
                .any(|formal| formal.event && formal.is_address())
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
            if formal.fixed_array.is_some()
                && !matches!(arg, IrCallArg::FixedArray(_) | IrCallArg::FixedValue(_))
            {
                return self.fail(&arg_path, "descriptor formal requires descriptor operand");
            }
            if formal.native_value.is_some()
                != matches!(
                    arg,
                    IrCallArg::NativeValue(_)
                        | IrCallArg::NativeLeaves { .. }
                        | IrCallArg::NativeCall { .. }
                )
            {
                return self.fail(&arg_path, "native-value formal and operand must match");
            }
            if formal.container.is_some()
                != matches!(
                    arg,
                    IrCallArg::Container(_) | IrCallArg::ContainerValues { .. }
                )
            {
                return self.fail(&arg_path, "container formal and operand must match");
            }
            if formal.real_array.is_some()
                != matches!(
                    arg,
                    IrCallArg::RealArray(_)
                        | IrCallArg::RealArrayValues(_)
                        | IrCallArg::RealArrayCall { .. }
                )
            {
                return self.fail(&arg_path, "real-array formal and operand must match");
            }
            match arg {
                IrCallArg::RealArray(array) => {
                    self.validate_fixed_activation(*array, &arg_path)?;
                    let expected = formal
                        .real_array
                        .and_then(|array| self.model.arrays.get(array));
                    let actual = self.model.arrays.get(*array);
                    if !actual.zip(expected).is_some_and(|(actual, expected)| {
                        actual.real
                            && expected.real
                            && actual.shortreal == expected.shortreal
                            && actual.total == expected.total
                    }) {
                        return self.fail(&arg_path, "real-array operand shape mismatch");
                    }
                }
                IrCallArg::RealArrayCall { array, call } => {
                    let expected = formal
                        .real_array
                        .and_then(|array| self.model.arrays.get(array));
                    let result = self.model.arrays.get(*array);
                    if formal.is_address()
                        || !result.zip(expected).is_some_and(|(result, expected)| {
                            result.real
                                && result.activation
                                && result.shortreal == expected.shortreal
                                && result.total == expected.total
                        })
                        || !call.args.iter().any(
                            |argument| matches!(argument, IrCallArg::RealArray(index) if index == array),
                        )
                    {
                        return self.fail(&arg_path, "real-array call operand requires an owned result");
                    }
                    self.fixed_activations
                        .borrow_mut()
                        .push(HashSet::from([*array]));
                    let valid = self.validate_stmt(&IrStmt::Call(call.clone()), formals, &arg_path);
                    self.fixed_activations.borrow_mut().pop();
                    valid?;
                }
                IrCallArg::RealArrayValues(values) => {
                    let expected = formal
                        .real_array
                        .and_then(|array| self.model.arrays.get(array));
                    if formal.is_address()
                        || expected.is_none_or(|expected| expected.total != values.len() as u64)
                    {
                        return self.fail(
                            &arg_path,
                            "real-array values require an input formal of the same size",
                        );
                    }
                    for (index, value) in values.iter().enumerate() {
                        let path = format!("{arg_path}.values[{index}]");
                        self.validate_expr(value, formals, &path)?;
                        if !value.is_real() {
                            return self.fail(path, "real-array element value must be real");
                        }
                    }
                }
                IrCallArg::NativeCall { value, call } => {
                    let expected = formal
                        .native_value
                        .and_then(|value| self.model.native_values.get(value));
                    let result = self.model.native_values.get(*value);
                    if formal.is_address()
                        || result.is_none_or(|result| {
                            !result.activation || expected.is_none_or(|expected| expected.ty != result.ty)
                        })
                        || !call
                            .args
                            .iter()
                            .any(|argument| matches!(argument, IrCallArg::NativeValue(index) if index == value))
                    {
                        return self.fail(&arg_path, "native call operand requires an owned result");
                    }
                    self.native_activations
                        .borrow_mut()
                        .push(HashSet::from([*value]));
                    let valid = self.validate_stmt(&IrStmt::Call(call.clone()), formals, &arg_path);
                    self.native_activations.borrow_mut().pop();
                    valid?;
                }
                IrCallArg::NativeLeaves {
                    ty,
                    leaves,
                    containers,
                } => {
                    let expected = formal
                        .native_value
                        .and_then(|value| self.model.native_values.get(value));
                    if formal.is_address() || expected.is_none_or(|expected| expected.ty != *ty) {
                        return self
                            .fail(&arg_path, "native leaf operand requires an input formal");
                    }
                    if containers.len() != formal.native_companions.len()
                        || containers.iter().zip(&formal.native_companions).any(
                            |(actual, expected)| {
                                self.model.containers.get(*actual).is_none_or(|actual| {
                                    !actual.same_storage_type(&self.model.containers[*expected])
                                })
                            },
                        )
                    {
                        return self.fail(&arg_path, "native leaf operand container mismatch");
                    }
                    self.validate_native_leaf_values(*ty, leaves, formals, &arg_path)?;
                }
                IrCallArg::ContainerValues { container, values } => {
                    let actual = self.model.containers.get(*container);
                    let expected = formal
                        .container
                        .and_then(|container| self.model.containers.get(container));
                    let Some((actual, expected)) = actual.zip(expected) else {
                        return self.fail(&arg_path, "container argument type mismatch");
                    };
                    if formal.is_address()
                        || !actual.activation
                        || !actual.same_storage_type(expected)
                        || matches!(actual.kind, IrContainerKind::Associative { .. })
                        || !(actual.element.is_packed() || actual.element.is_real())
                    {
                        return self.fail(
                            &arg_path,
                            "container values require a packed or real dynamic array or queue input",
                        );
                    }
                    for (index, value) in values.iter().enumerate() {
                        self.validate_expr(value, formals, &format!("{arg_path}.values[{index}]"))?;
                        if value.is_real() != actual.element.is_real() {
                            return self.fail(&arg_path, "container value kind mismatch");
                        }
                    }
                }
                IrCallArg::Container(container) => {
                    let actual = self.model.containers.get(*container);
                    let expected = formal
                        .container
                        .and_then(|container| self.model.containers.get(container));
                    // A `ref` container formal is an alias bound per call.
                    if formal.is_ref() && !expected.is_some_and(|expected| expected.activation)
                        || !matches!((actual, expected), (Some(actual), Some(expected))
                            if actual.same_storage_type(expected))
                    {
                        return self.fail(&arg_path, "container argument type mismatch");
                    }
                }
                IrCallArg::NativeValue(value) => {
                    self.validate_native_value_use(*value, &arg_path)?;
                    let expected = formal
                        .native_value
                        .and_then(|value| self.model.native_values.get(value));
                    if expected
                        .is_none_or(|expected| expected.ty != self.model.native_values[*value].ty)
                        || formal.is_ref()
                    {
                        return self.fail(&arg_path, "native argument type mismatch");
                    }
                    let actual = &self.model.native_values[*value].companions;
                    if actual.len() != formal.native_companions.len()
                        || actual
                            .iter()
                            .zip(&formal.native_companions)
                            .any(|(actual, expected)| {
                                !self.model.containers[*actual]
                                    .same_storage_type(&self.model.containers[*expected])
                            })
                    {
                        return self.fail(&arg_path, "native argument companion mismatch");
                    }
                }
                IrCallArg::FixedValue(value) => {
                    let bits = self.validate_fixed_value(value, formals, &arg_path)?;
                    let expected = formal
                        .fixed_array
                        .and_then(|array| self.model.arrays.get(array))
                        .ok_or_else(|| {
                            IrValidationError::new(
                                &arg_path,
                                "fixed operand requires descriptor formal",
                            )
                        })?;
                    let formal_bits = expected.total * u64::from(expected.elem_width);
                    // A stream may be shorter than its formal (it is left-
                    // justified and zero-filled); runtime parts count as zero.
                    let fits = if matches!(value.as_ref(), IrFixedValue::Stream { .. }) {
                        bits <= formal_bits
                    } else {
                        bits == formal_bits
                    };
                    if !fits
                        || (formal.is_ref() && !matches!(value.as_ref(), IrFixedValue::Array(_)))
                    {
                        return self.fail(
                            &arg_path,
                            "fixed argument shape or reference identity mismatch",
                        );
                    }
                }
                IrCallArg::FixedArray(array) => {
                    self.validate_fixed_activation(*array, &arg_path)?;
                    let actual = self.model.arrays.get(*array).ok_or_else(|| {
                        IrValidationError::new(&arg_path, "invalid descriptor argument")
                    })?;
                    let expected = formal
                        .fixed_array
                        .and_then(|array| self.model.arrays.get(array))
                        .ok_or_else(|| {
                            IrValidationError::new(
                                &arg_path,
                                "descriptor argument requires descriptor formal",
                            )
                        })?;
                    if !actual.sparse()
                        || actual.total != expected.total
                        || actual.elem_width != expected.elem_width
                        || actual
                            .dims
                            .iter()
                            .map(|(l, r)| l.abs_diff(*r))
                            .ne(expected.dims.iter().map(|(l, r)| l.abs_diff(*r)))
                    {
                        return self.fail(&arg_path, "descriptor argument shape mismatch");
                    }
                }

                IrCallArg::Val(_) if formal.is_address() => {
                    return self.fail(arg_path, "address formal requires an address argument");
                }
                IrCallArg::Val(_) if formal.event => {
                    return self.fail(arg_path, "event formal requires an event argument");
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
                IrCallArg::EventVal(event) => {
                    if !formal.event || formal.is_address() {
                        return self.fail(arg_path, "event value requires an input event formal");
                    }
                    self.validate_event_ref(event, formals, &arg_path)?;
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
                    if formal.real {
                        self.validate_real_ref_actual(
                            lhs, read, *width, *const_ref, formal, formals, &arg_path,
                        )?;
                        continue;
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

    /// A real reference operand names one real storage cell: a real signal,
    /// real local/formal storage, a whole real array element, or a forwarded
    /// real reference formal of the enclosing function.
    #[allow(clippy::too_many_arguments)]
    fn validate_real_ref_actual(
        &self,
        lhs: &IrLhs,
        read: &IrExpr,
        width: u32,
        const_ref: bool,
        formal: &IrFormal,
        formals: &[IrFormal],
        path: &str,
    ) -> ValidationResult {
        if width != 0 || !read.is_real() {
            return self.fail(path, "real reference operand must carry a real value");
        }
        if const_ref && !formal.const_ref {
            return self.fail(path, "const reference cannot bind to a writable ref formal");
        }
        let real_storage = match lhs {
            IrLhs::Whole(signal) => self
                .model
                .signals
                .get(*signal)
                .is_some_and(|signal| matches!(signal.ty, IrType::Real { .. })),
            IrLhs::WholeRef { width: 0, .. } => true,
            IrLhs::ArrayElem {
                arr,
                elem_sel: IrElemSel::Whole,
                ..
            } => self.model.arrays.get(*arr).is_some_and(|array| array.real),
            IrLhs::Ref {
                addr,
                width: 0,
                bit: None,
                const_ref: actual_const,
                ..
            } => {
                if *actual_const && !formal.const_ref {
                    return self.fail(path, "const reference cannot bind to a writable ref formal");
                }
                // A const real reference is not an assignment target, so it is
                // checked here rather than through `validate_lhs`.
                return match super::lvalues::real_ref_formal(formals, addr) {
                    Some(_) => self.validate_expr(read, formals, &format!("{path}.read")),
                    None => self.fail(path, "forwarded real reference names no real ref formal"),
                };
            }
            _ => false,
        };
        if !real_storage {
            return self.fail(
                path,
                "real reference operand requires real variable storage",
            );
        }
        self.validate_lhs(lhs, formals, &format!("{path}.lhs"))?;
        self.validate_expr(read, formals, &format!("{path}.read"))
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
