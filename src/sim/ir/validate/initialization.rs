//! Initialization.

use super::*;

impl Validator<'_> {
    pub(super) fn validate_pre_fns(
        &self,
        pre_fns: &[IrPreFn],
        formals: &[IrFormal],
        path: &str,
    ) -> ValidationResult {
        let saved_return = self.chandle_return.replace(None);
        let saved_string_return = self.string_return.replace(None);
        let branch_formals = &[];
        for (idx, pre_fn) in pre_fns.iter().enumerate() {
            match pre_fn {
                IrPreFn::Branch { body, .. } => {
                    self.validate_stmts(
                        body,
                        branch_formals,
                        &format!("{path}.pre_fns[{idx}].body"),
                    )?;
                }
                IrPreFn::CapturedBranch { captures, body, .. } => {
                    for (capture_idx, capture) in captures.iter().enumerate() {
                        self.validate_capture(
                            capture.storage(),
                            capture.initial(),
                            formals,
                            &format!("{path}.pre_fns[{idx}].captures[{capture_idx}].initial"),
                        )?;
                    }
                    // Each `ref` formal's descriptor is captured once; a
                    // handle formal's change marker follows it in the next
                    // slot.
                    let mut captured_formals: Vec<usize> = Vec::new();
                    for (capture_idx, capture) in captures.iter().enumerate() {
                        let IrExprKind::FormalRead(index) = capture.initial().kind() else {
                            continue;
                        };
                        if capture.storage().kind() != StorageKind::Opaque {
                            continue;
                        }
                        let repeated = captured_formals.contains(index);
                        let handle = formals
                            .get(*index)
                            .is_some_and(|formal| formal.chandle || formal.event);
                        let paired = capture_idx > 0
                            && matches!(captures[capture_idx - 1].initial().kind(),
                                IrExprKind::FormalRead(previous) if previous == index)
                            && captured_formals
                                .iter()
                                .filter(|seen| *seen == index)
                                .count()
                                == 1;
                        let marker_expected = handle
                            && !repeated
                            && !captures.get(capture_idx + 1).is_some_and(|next| {
                                matches!(next.initial().kind(), IrExprKind::FormalRead(n) if n == index)
                            });
                        if (repeated && !(handle && paired)) || marker_expected {
                            return self.fail(
                                format!("{path}.pre_fns[{idx}].captures[{capture_idx}]"),
                                "a `ref` formal is captured once, followed by its change marker for a handle or event formal",
                            );
                        }
                        captured_formals.push(*index);
                    }
                    // A fork-join branch names the `ref` formals whose
                    // descriptors it captured as the subroutine does.
                    let ref_formals = captures.iter().any(|capture| {
                        matches!(capture.initial().kind(), IrExprKind::FormalRead(_))
                    });
                    self.validate_stmts(
                        body,
                        if ref_formals { formals } else { branch_formals },
                        &format!("{path}.pre_fns[{idx}].body"),
                    )?;
                }
                IrPreFn::MonEval { args, context, .. } => {
                    for (arg_idx, arg) in args.iter().enumerate() {
                        self.validate_expr(
                            arg,
                            branch_formals,
                            &format!("{path}.pre_fns[{idx}].args[{arg_idx}]"),
                        )?;
                    }
                    if let Some(context) = context {
                        for (capture_idx, capture) in context.captures().iter().enumerate() {
                            self.validate_capture(
                                capture.storage(),
                                capture.initial(),
                                formals,
                                &format!(
                                    "{path}.pre_fns[{idx}].context.captures[{capture_idx}].initial"
                                ),
                            )?;
                        }
                    }
                }
                IrPreFn::EventAssign {
                    frame: pre_frame,
                    captures,
                    lhs,
                    rhs,
                    ..
                } => {
                    let mut slots = HashSet::new();
                    for (capture_idx, capture) in captures.iter().enumerate() {
                        if capture.storage().frame() != *pre_frame
                            || !slots.insert(capture.storage().slot())
                        {
                            return self.fail(
                                format!("{path}.pre_fns[{idx}].captures[{capture_idx}]"),
                                "event assignment captures must use unique slots in their frame",
                            );
                        }
                        self.validate_capture(
                            capture.storage(),
                            capture.initial(),
                            formals,
                            &format!("{path}.pre_fns[{idx}].captures[{capture_idx}].initial"),
                        )?;
                    }
                    self.validate_lhs(lhs, branch_formals, &format!("{path}.pre_fns[{idx}].lhs"))?;
                    self.validate_expr(rhs, branch_formals, &format!("{path}.pre_fns[{idx}].rhs"))?;
                }
                IrPreFn::DeferredAssertion {
                    frame: pre_frame,
                    captures,
                    body,
                    ..
                } => {
                    let mut slots = HashSet::new();
                    for (capture_idx, capture) in captures.iter().enumerate() {
                        if capture.storage().frame() != *pre_frame
                            || !slots.insert(capture.storage().slot())
                        {
                            return self.fail(
                                format!("{path}.pre_fns[{idx}].captures[{capture_idx}]"),
                                "deferred assertion captures must use unique slots in their frame",
                            );
                        }
                        self.validate_capture(
                            capture.storage(),
                            capture.initial(),
                            formals,
                            &format!("{path}.pre_fns[{idx}].captures[{capture_idx}].initial"),
                        )?;
                    }
                    self.validate_stmts(
                        body,
                        branch_formals,
                        &format!("{path}.pre_fns[{idx}].body"),
                    )?;
                }
                IrPreFn::MonitorReads { reads, .. } => {
                    for read in reads {
                        if !matches!(read, IrDependency::NativeAccess(_)) {
                            return self
                                .fail(path, "a monitor resolves only native accesses at run time");
                        }
                    }
                }
                IrPreFn::DisplayEval { args, prelude, .. } => {
                    self.validate_stmts(
                        prelude,
                        branch_formals,
                        &format!("{path}.pre_fns[{idx}].prelude"),
                    )?;
                    for (arg_idx, arg) in args.iter().enumerate() {
                        arg.validate(
                            self.model,
                            self.string_return.get(),
                            &format!("{path}.pre_fns[{idx}].args[{arg_idx}]"),
                        )?;
                        let mut result = Ok(());
                        arg.expressions(&mut |expression| {
                            result = result.clone().and_then(|_| {
                                self.validate_expr(
                                    expression,
                                    branch_formals,
                                    &format!("{path}.pre_fns[{idx}].args[{arg_idx}]"),
                                )
                            });
                        });
                        result?;
                    }
                }
                IrPreFn::RealEval { value, context, .. } => {
                    if !value.is_real() {
                        return self.fail(
                            format!("{path}.pre_fns[{idx}].value"),
                            "real event evaluator requires a real expression",
                        );
                    }
                    self.validate_expr(
                        value,
                        branch_formals,
                        &format!("{path}.pre_fns[{idx}].value"),
                    )?;
                    if let Some(context) = context {
                        for (capture_idx, capture) in context.captures().iter().enumerate() {
                            self.validate_capture(
                                capture.storage(),
                                capture.initial(),
                                formals,
                                &format!(
                                    "{path}.pre_fns[{idx}].context.captures[{capture_idx}].initial"
                                ),
                            )?;
                        }
                    }
                }
                IrPreFn::ForceEval { value, real, .. } => {
                    if *real != value.is_real() {
                        return self.fail(
                            format!("{path}.pre_fns[{idx}].value"),
                            "force evaluator real flag disagrees with its value",
                        );
                    }
                    self.validate_expr(
                        value,
                        branch_formals,
                        &format!("{path}.pre_fns[{idx}].value"),
                    )?;
                }
            }
        }
        self.chandle_return.set(saved_return);
        self.string_return.set(saved_string_return);
        Ok(())
    }

    pub(super) fn validate_init_step(&self, step: &IrInitStep, path: &str) -> ValidationResult {
        match step {
            IrInitStep::FillArrayX(array)
            | IrInitStep::FillArrayZ(array)
            | IrInitStep::SetArrayElem { arr: array, .. } => {
                if *array >= self.model.arrays.len() {
                    return self.fail(path, format!("array index {array} is out of bounds"));
                }
            }
            IrInitStep::SetScalar { sig, .. } => {
                if *sig >= self.model.signals.len() {
                    return self.fail(path, format!("signal index {sig} is out of bounds"));
                }
            }
            IrInitStep::RegisterSampled(sig) => {
                let Some(signal) = self.model.signals.get(*sig) else {
                    return self.fail(path, format!("sampled signal index {sig} is out of bounds"));
                };
                if signal.net_driver.is_some() && matches!(signal.ty, IrType::Real { .. }) {
                    return self.fail(path, "sampled real source must be a variable");
                }
            }
            IrInitStep::RegisterSampledHistory { sig, ticks } => {
                let Some(signal) = self.model.signals.get(*sig) else {
                    return self.fail(path, format!("sampled signal index {sig} is out of bounds"));
                };
                if !matches!(signal.ty, IrType::Packed { .. }) || *ticks == 0 {
                    return self.fail(path, "sampled history needs a packed source and a skew");
                }
            }
            IrInitStep::StaticScopeEnter { variable, location }
                if variable.is_empty() || location.is_empty() =>
            {
                return self.fail(path, "static scope needs a variable and a location");
            }
            IrInitStep::StaticScopeEnter { .. } | IrInitStep::StaticScopeLeave => {}
            IrInitStep::RegisterSampledValue(sig) => {
                let Some(signal) = self.model.signals.get(*sig) else {
                    return self.fail(path, format!("sampled signal index {sig} is out of bounds"));
                };
                if matches!(signal.ty, IrType::Real { .. }) || signal.ty.width() == 0 {
                    return self.fail(path, "sampled value source must be packed");
                }
            }
            IrInitStep::Initialize(initialization) => match &initialization.target {
                IrInitTarget::Fixed(lhs) => {
                    self.validate_lhs(lhs, &[], &format!("{path}.target"))?;
                    if self.has_transient_target(lhs) {
                        return self.fail(
                            path,
                            "fixed declaration initializer requires persistent storage",
                        );
                    }
                }
                IrInitTarget::Signal(signal) => {
                    if *signal >= self.model.signals.len() {
                        return self.fail(path, format!("signal index {signal} is out of bounds"));
                    }
                }
                IrInitTarget::StaticLocal { function, name } => {
                    let function_ref = self.model.funcs.get(*function).ok_or_else(|| {
                        IrValidationError::new(
                            path,
                            format!("function index {function} is out of bounds"),
                        )
                    })?;
                    if !function_ref
                        .locals
                        .iter()
                        .any(|local| local.c_name() == name)
                    {
                        return self.fail(
                            path,
                            format!("static local `{name}` is not present in function {function}"),
                        );
                    }
                }
            },
            IrInitStep::Execute { body, .. } => {
                self.validate_stmts(std::slice::from_ref(body), &[], &format!("{path}.body"))?;
            }
            IrInitStep::WriteNet { group, slot, .. } => {
                let net = self.model.net_groups.get(*group).ok_or_else(|| {
                    IrValidationError::new(
                        path,
                        format!("net-group index {group} is out of bounds"),
                    )
                })?;
                if *slot >= net.n_drivers {
                    return self.fail(path, format!("driver slot {slot} is out of bounds"));
                }
            }
        }
        match step {
            IrInitStep::SetArrayElem { value, .. }
            | IrInitStep::SetScalar { value, .. }
            | IrInitStep::WriteNet { value, .. } => self.validate_const(value, path),
            IrInitStep::FillArrayX(_) => Ok(()),
            IrInitStep::RegisterSampled(_)
            | IrInitStep::RegisterSampledHistory { .. }
            | IrInitStep::RegisterSampledValue(_)
            | IrInitStep::StaticScopeEnter { .. }
            | IrInitStep::StaticScopeLeave => Ok(()),
            // The body was validated above. Suspension is rejected by the
            // owned emitter, which owns the execution-effect analysis.
            IrInitStep::Execute { .. } => Ok(()),
            IrInitStep::FillArrayZ(array) => {
                if self.model.arrays[*array].two_state {
                    self.fail(path, "Z initialization requires four-state array elements")
                } else {
                    Ok(())
                }
            }
            IrInitStep::Initialize(initialization) => {
                if initialization.lifetime != StorageLifetime::Static {
                    return self.fail(
                        path,
                        "declaration initialization must target static storage",
                    );
                }
                match &initialization.target {
                    IrInitTarget::Fixed(lhs) => {
                        if initialization.value.is_real()
                            || self.lhs_packed_width(lhs) != Some(initialization.value.width)
                        {
                            return self.fail(
                                path,
                                "fixed declaration initializer width disagrees with its target",
                            );
                        }
                    }
                    IrInitTarget::Signal(signal) => {
                        let ty = self.model.signal(*signal).ty;
                        match ty {
                            IrType::Real { .. } => {
                                if !initialization.value.is_real() {
                                    return self.fail(
                                        path,
                                        "real declaration initializer must produce a real value",
                                    );
                                }
                            }
                            IrType::Packed { width, signed, .. } => {
                                if initialization.value.is_real()
                                    || initialization.value.width != width
                                    || initialization.value.signed != signed
                                {
                                    return self.fail(
                                        path,
                                        "signal declaration initializer type disagrees with its target",
                                    );
                                }
                            }
                        }
                    }
                    IrInitTarget::StaticLocal { function, name } => {
                        let local = self
                            .model
                            .func(*function)
                            .locals
                            .iter()
                            .find(|local| local.c_name() == name);
                        let local = local.ok_or_else(|| {
                            IrValidationError::new(
                                path,
                                format!(
                                    "static local `{name}` is not present in function {function}"
                                ),
                            )
                        })?;
                        if initialization.value.is_real() != local.real
                            || initialization.value.width != local.width()
                            || initialization.value.signed != local.signed()
                        {
                            return self.fail(
                                path,
                                "static local initializer type disagrees with its target",
                            );
                        }
                    }
                }
                self.validate_expr(&initialization.value, &[], &format!("{path}.value"))
            }
        }
    }

    pub(super) fn validate_spawns(&self, spawns: &[String], path: &str) -> ValidationResult {
        let processes: HashSet<&str> = self
            .model
            .processes
            .iter()
            .map(|process| process.c_name.as_str())
            .collect();
        let mut seen = HashSet::new();
        for (idx, spawn) in spawns.iter().enumerate() {
            if !processes.contains(spawn.as_str()) {
                return self.fail(
                    format!("{path}[{idx}]"),
                    format!("process `{spawn}` does not exist"),
                );
            }
            if !seen.insert(spawn.as_str()) {
                return self.fail(format!("{path}[{idx}]"), "duplicate process registration");
            }
        }
        Ok(())
    }

    pub(super) fn fail<T>(
        &self,
        path: impl Into<String>,
        detail: impl Into<String>,
    ) -> Result<T, IrValidationError> {
        Err(IrValidationError::new(path, detail))
    }
}
