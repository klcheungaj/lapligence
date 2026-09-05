use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use super::*;

/// A structural IR invariant violation detected between simulator phases.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrValidationError {
    path: String,
    detail: String,
}

impl IrValidationError {
    fn new(path: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            detail: detail.into(),
        }
    }

    /// Location of the invalid value within the IR model.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Human-readable description of the violated invariant.
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl fmt::Display for IrValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid simulator IR at {}: {}", self.path, self.detail)
    }
}

impl Error for IrValidationError {}

type ValidationResult = Result<(), IrValidationError>;

struct Validator<'model> {
    model: &'model IrModel,
}

impl IrModel {
    /// Validate all cross-table references and representation invariants.
    ///
    /// Lowering and optimization must call this at their phase boundaries;
    /// the C emitter may then index the model tables without defensive checks.
    pub fn validate(&self) -> Result<(), IrValidationError> {
        Validator { model: self }.validate()
    }
}

impl Validator<'_> {
    fn validate(&self) -> ValidationResult {
        if self.model.precision_ps == 0 {
            return self.fail("precision_ps", "scheduler precision must be non-zero");
        }

        let mut storage_names = HashSet::new();
        for (idx, signal) in self.model.signals.iter().enumerate() {
            let path = format!("signals[{idx}]");
            self.validate_type(&signal.ty, &format!("{path}.ty"))?;
            if let Some((group, slot)) = signal.net_driver {
                let net = self.model.net_groups.get(group).ok_or_else(|| {
                    IrValidationError::new(
                        format!("{path}.net_driver"),
                        format!("net-group index {group} is out of bounds"),
                    )
                })?;
                if slot >= net.n_drivers {
                    return self.fail(
                        format!("{path}.net_driver"),
                        format!("driver slot {slot} is out of bounds for group {group}"),
                    );
                }
                if signal.ty.width() != net.width || signal.ty.signed() != net.signed {
                    return self.fail(
                        format!("{path}.net_driver"),
                        "signal type does not match its net group",
                    );
                }
            } else if !signal.omit && !storage_names.insert(signal.c_name.as_str()) {
                return self.fail(
                    format!("{path}.c_name"),
                    format!("active storage name `{}` is not unique", signal.c_name),
                );
            }
        }

        for (idx, group) in self.model.net_groups.iter().enumerate() {
            let path = format!("net_groups[{idx}]");
            self.validate_width(group.width, &format!("{path}.width"))?;
            if group.n_drivers == 0 {
                return self.fail(format!("{path}.n_drivers"), "net group has no drivers");
            }
        }

        for (idx, array) in self.model.arrays.iter().enumerate() {
            let path = format!("arrays[{idx}]");
            self.validate_width(array.elem_width, &format!("{path}.elem_width"))?;
            if array.dims.is_empty() {
                return self.fail(format!("{path}.dims"), "array has no dimensions");
            }
            let mut total = 1u64;
            for (dim_idx, (left, right)) in array.dims.iter().copied().enumerate() {
                let extent = (i64::from(left) - i64::from(right)).unsigned_abs() + 1;
                total = total.checked_mul(extent).ok_or_else(|| {
                    IrValidationError::new(
                        format!("{path}.dims[{dim_idx}]"),
                        "dimension product overflows u64",
                    )
                })?;
            }
            if total != array.total {
                return self.fail(
                    format!("{path}.total"),
                    format!(
                        "stored total {} does not match dimension product {total}",
                        array.total
                    ),
                );
            }
        }

        for (idx, func) in self.model.funcs.iter().enumerate() {
            let path = format!("funcs[{idx}]");
            if let Some(ret) = &func.ret {
                self.validate_type(ret, &format!("{path}.ret"))?;
            }
            for (formal_idx, formal) in func.formals.iter().enumerate() {
                self.validate_width(formal.width, &format!("{path}.formals[{formal_idx}].width"))?;
            }
            for (local_idx, local) in func.locals.iter().enumerate() {
                self.validate_width(local.width, &format!("{path}.locals[{local_idx}].width"))?;
            }
            self.validate_pre_fns(&func.pre_fns, func.formals.len(), &path)?;
            self.validate_stmts(&func.body, func.formals.len(), &format!("{path}.body"))?;
        }

        for (idx, process) in self.model.processes.iter().enumerate() {
            let path = format!("processes[{idx}]");
            self.validate_pre_fns(&process.pre_fns, 0, &path)?;
            self.validate_stmts(&process.body, 0, &format!("{path}.body"))?;
        }

        for (idx, step) in self.model.init_steps.iter().enumerate() {
            self.validate_init_step(step, &format!("init_steps[{idx}]"))?;
        }

        self.validate_spawns(&self.model.spawns, "spawns")?;
        self.validate_spawns(&self.model.final_spawns, "final_spawns")?;
        let normal: HashSet<&str> = self.model.spawns.iter().map(String::as_str).collect();
        if let Some(name) = self
            .model
            .final_spawns
            .iter()
            .find(|name| normal.contains(name.as_str()))
        {
            return self.fail(
                "final_spawns",
                format!("process `{name}` is registered as both normal and final"),
            );
        }
        Ok(())
    }

    fn validate_type(&self, ty: &IrType, path: &str) -> ValidationResult {
        if let IrType::Packed { width, .. } = ty {
            self.validate_width(*width, path)?;
        }
        Ok(())
    }

    fn validate_width(&self, width: u32, path: &str) -> ValidationResult {
        if !(1..=LLG_MAX_WIDTH).contains(&width) {
            return self.fail(
                path,
                format!("packed width {width} is outside 1..={LLG_MAX_WIDTH}"),
            );
        }
        Ok(())
    }

    fn validate_const(&self, value: &IrConst, path: &str) -> ValidationResult {
        if value.real.is_some() {
            if value.width != 0 {
                return self.fail(path, "real constant has a non-zero packed width");
            }
            if value.fill.is_some() {
                return self.fail(path, "real constant carries a packed fill marker");
            }
            return Ok(());
        }
        self.validate_width(value.width, &format!("{path}.width"))?;
        if value.fill.is_some_and(|fill| fill > 3) {
            return self.fail(format!("{path}.fill"), "fill marker must be in 0..=3");
        }
        let limbs = value.width.div_ceil(64) as usize;
        for (name, values) in [("bits", &value.bits), ("x", &value.x), ("z", &value.z)] {
            if values.len() > limbs {
                return self.fail(
                    format!("{path}.{name}"),
                    format!("{} limbs exceed the {limbs}-limb width", values.len()),
                );
            }
        }
        for idx in 0..limbs {
            let x = value.x.get(idx).copied().unwrap_or(0);
            let z = value.z.get(idx).copied().unwrap_or(0);
            if x & z != 0 {
                return self.fail(format!("{path}.x"), "X and Z masks overlap");
            }
        }
        let tail = value.width % 64;
        if tail != 0 {
            let outside = !((1u64 << tail) - 1);
            for (name, values) in [("bits", &value.bits), ("x", &value.x), ("z", &value.z)] {
                if values.last().copied().unwrap_or(0) & outside != 0 {
                    return self.fail(
                        format!("{path}.{name}"),
                        "high limb contains bits outside the declared width",
                    );
                }
            }
        }
        Ok(())
    }

    fn validate_expr(&self, expr: &IrExpr, formals: usize, path: &str) -> ValidationResult {
        if expr.width > LLG_MAX_WIDTH {
            return self.fail(
                format!("{path}.width"),
                format!("expression width {} exceeds {LLG_MAX_WIDTH}", expr.width),
            );
        }
        if expr.fill.is_some_and(|fill| fill > 3) {
            return self.fail(format!("{path}.fill"), "fill marker must be in 0..=3");
        }
        match &expr.kind {
            IrExprKind::Const(value) => self.validate_const(value, &format!("{path}.const"))?,
            IrExprKind::SigRead(idx) => {
                self.model.signals.get(*idx).ok_or_else(|| {
                    IrValidationError::new(path, format!("signal index {idx} is out of bounds"))
                })?;
            }
            IrExprKind::LocalRead(_) => {}
            IrExprKind::FormalRead(idx) => {
                if *idx >= formals {
                    return self.fail(path, format!("formal index {idx} is out of bounds"));
                }
            }
            IrExprKind::CallFn(call) => self.validate_call_expr(call, formals, path)?,
            IrExprKind::Bin { a, b, .. } | IrExprKind::RealBin { a, b, .. } => {
                self.validate_expr(a, formals, &format!("{path}.a"))?;
                self.validate_expr(b, formals, &format!("{path}.b"))?;
            }
            IrExprKind::Un { a, .. }
            | IrExprKind::RealUn { a, .. }
            | IrExprKind::CastToReal { a, .. }
            | IrExprKind::CastToPacked { a }
            | IrExprKind::Resize { a }
            | IrExprKind::Convert { a } => {
                self.validate_expr(a, formals, &format!("{path}.a"))?;
            }
            IrExprKind::Mux { sel, a, b } => {
                self.validate_expr(sel, formals, &format!("{path}.sel"))?;
                self.validate_expr(a, formals, &format!("{path}.a"))?;
                self.validate_expr(b, formals, &format!("{path}.b"))?;
            }
            IrExprKind::Concat { parts } | IrExprKind::Replicate { parts, .. } => {
                for (idx, part) in parts.iter().enumerate() {
                    self.validate_expr(part, formals, &format!("{path}.parts[{idx}]"))?;
                }
            }
            IrExprKind::BitSel { base, idx } => {
                self.validate_expr(base, formals, &format!("{path}.base"))?;
                self.validate_expr(idx, formals, &format!("{path}.idx"))?;
            }
            IrExprKind::PartSel { base, left, right } => {
                self.validate_expr(base, formals, &format!("{path}.base"))?;
                self.validate_select_width(*left, *right, path)?;
            }
            IrExprKind::IdxPartSel {
                base,
                base_idx,
                width_expr,
                ..
            } => {
                self.validate_expr(base, formals, &format!("{path}.base"))?;
                self.validate_expr(base_idx, formals, &format!("{path}.base_idx"))?;
                self.validate_expr(width_expr, formals, &format!("{path}.width_expr"))?;
            }
            IrExprKind::ArrayRead {
                arr,
                indices,
                elem_sel,
            } => {
                let array = self.model.arrays.get(*arr).ok_or_else(|| {
                    IrValidationError::new(path, format!("array index {arr} is out of bounds"))
                })?;
                if indices.len() != array.dims.len() {
                    return self.fail(
                        path,
                        format!(
                            "array read has {} indices for {} dimensions",
                            indices.len(),
                            array.dims.len()
                        ),
                    );
                }
                for (idx, index) in indices.iter().enumerate() {
                    self.validate_expr(index, formals, &format!("{path}.indices[{idx}]"))?;
                }
                self.validate_elem_sel(elem_sel, formals, &format!("{path}.elem_sel"))?;
            }
            IrExprKind::Fill(fill) => {
                if *fill > 3 {
                    return self.fail(path, "fill value must be in 0..=3");
                }
            }
            IrExprKind::Verbatim { width, signed, .. } => {
                if *width != expr.width || *signed != expr.signed {
                    return self.fail(path, "verbatim payload type disagrees with expression type");
                }
            }
            IrExprKind::SysFunc(sys) => match sys {
                IrSysFunc::Clog2(arg) | IrSysFunc::Bits(arg) => {
                    self.validate_expr(arg, formals, &format!("{path}.arg"))?;
                }
                IrSysFunc::Time {
                    precision_ps,
                    unit_ps,
                    kind,
                } => {
                    if *precision_ps == 0 || *unit_ps == 0 {
                        return self.fail(path, "time scaling units must be non-zero");
                    }
                    if expr.width != kind.width() {
                        return self.fail(path, "time expression width disagrees with its kind");
                    }
                }
            },
        }
        Ok(())
    }

    fn validate_call_expr(
        &self,
        call: &IrCallExpr,
        formals: usize,
        path: &str,
    ) -> ValidationResult {
        self.validate_call_target(call.f, &call.args, formals, path)?;
        for (idx, arg) in call.args.iter().enumerate() {
            if let IrCallArg::OutTemp {
                init, writeback, ..
            } = arg
            {
                if let Some(init) = init {
                    self.validate_expr(init, formals, &format!("{path}.args[{idx}].init"))?;
                }
                self.validate_lhs(writeback, formals, &format!("{path}.args[{idx}].writeback"))?;
            }
        }
        Ok(())
    }

    fn validate_call_target(
        &self,
        function: usize,
        args: &[IrCallArg],
        formals: usize,
        path: &str,
    ) -> ValidationResult {
        let callee = self.model.funcs.get(function).ok_or_else(|| {
            IrValidationError::new(path, format!("function index {function} is out of bounds"))
        })?;
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
        for (idx, arg) in args.iter().enumerate() {
            if let IrCallArg::Val(expr) = arg {
                self.validate_expr(expr, formals, &format!("{path}.args[{idx}]"))?;
            }
        }
        Ok(())
    }

    fn validate_elem_sel(&self, sel: &IrElemSel, formals: usize, path: &str) -> ValidationResult {
        match sel {
            IrElemSel::Whole => Ok(()),
            IrElemSel::Part(left, right) => self.validate_select_width(*left, *right, path),
            IrElemSel::Bit(expr) => self.validate_expr(expr, formals, path),
        }
    }

    fn validate_select_width(&self, left: i64, right: i64, path: &str) -> ValidationResult {
        let width = (i128::from(left) - i128::from(right)).unsigned_abs() + 1;
        if width > u128::from(LLG_MAX_WIDTH) {
            return self.fail(
                path,
                format!("part-select width {width} exceeds {LLG_MAX_WIDTH}"),
            );
        }
        Ok(())
    }

    fn validate_lhs(&self, lhs: &IrLhs, formals: usize, path: &str) -> ValidationResult {
        match lhs {
            IrLhs::Whole(signal) | IrLhs::Part(signal, ..) => {
                if *signal >= self.model.signals.len() {
                    return self.fail(path, format!("signal index {signal} is out of bounds"));
                }
                if let IrLhs::Part(_, left, right) = lhs {
                    self.validate_select_width(*left, *right, path)?;
                }
            }
            IrLhs::WholeRef { width, .. } => self.validate_width(*width, path)?,
            IrLhs::Bit(signal, index) => {
                if *signal >= self.model.signals.len() {
                    return self.fail(path, format!("signal index {signal} is out of bounds"));
                }
                self.validate_expr(index, formals, &format!("{path}.index"))?;
            }
            IrLhs::IdxPart(signal, base, width, _) => {
                if *signal >= self.model.signals.len() {
                    return self.fail(path, format!("signal index {signal} is out of bounds"));
                }
                self.validate_expr(base, formals, &format!("{path}.base"))?;
                self.validate_expr(width, formals, &format!("{path}.width"))?;
            }
            IrLhs::ArrayElem {
                arr,
                indices,
                elem_sel,
            } => {
                let array = self.model.arrays.get(*arr).ok_or_else(|| {
                    IrValidationError::new(path, format!("array index {arr} is out of bounds"))
                })?;
                if indices.len() != array.dims.len() {
                    return self.fail(path, "array LHS index count does not match its dimensions");
                }
                for (idx, index) in indices.iter().enumerate() {
                    self.validate_expr(index, formals, &format!("{path}.indices[{idx}]"))?;
                }
                self.validate_elem_sel(elem_sel, formals, &format!("{path}.elem_sel"))?;
            }
        }
        Ok(())
    }

    fn validate_stmts(&self, stmts: &[IrStmt], formals: usize, path: &str) -> ValidationResult {
        for (idx, stmt) in stmts.iter().enumerate() {
            self.validate_stmt(stmt, formals, &format!("{path}[{idx}]"))?;
        }
        Ok(())
    }

    fn validate_stmt(&self, stmt: &IrStmt, formals: usize, path: &str) -> ValidationResult {
        match stmt {
            IrStmt::Block(body) | IrStmt::Forever { body } => {
                self.validate_stmts(body, formals, &format!("{path}.body"))?;
            }
            IrStmt::DeclLocal { width, init, .. } => {
                self.validate_width(*width, &format!("{path}.width"))?;
                if let Some(init) = init {
                    self.validate_expr(init, formals, &format!("{path}.init"))?;
                }
            }
            IrStmt::Assign { lhs, rhs, .. } => {
                self.validate_lhs(lhs, formals, &format!("{path}.lhs"))?;
                self.validate_expr(rhs, formals, &format!("{path}.rhs"))?;
            }
            IrStmt::If { cond, then_, els } => {
                self.validate_expr(cond, formals, &format!("{path}.cond"))?;
                self.validate_stmts(then_, formals, &format!("{path}.then"))?;
                if let Some(els) = els {
                    self.validate_stmts(els, formals, &format!("{path}.else"))?;
                }
            }
            IrStmt::While { cond, body }
            | IrStmt::Repeat { count: cond, body }
            | IrStmt::WaitCond { cond, body, .. } => {
                self.validate_expr(cond, formals, &format!("{path}.cond"))?;
                self.validate_stmts(body, formals, &format!("{path}.body"))?;
            }
            IrStmt::For {
                init,
                cond,
                incr,
                body,
            } => {
                self.validate_stmts(init, formals, &format!("{path}.init"))?;
                self.validate_expr(cond, formals, &format!("{path}.cond"))?;
                self.validate_stmts(incr, formals, &format!("{path}.incr"))?;
                self.validate_stmts(body, formals, &format!("{path}.body"))?;
            }
            IrStmt::Case { sel, items, .. } => {
                self.validate_expr(sel, formals, &format!("{path}.sel"))?;
                for (item_idx, item) in items.iter().enumerate() {
                    for (expr_idx, expr) in item.exprs.iter().enumerate() {
                        self.validate_expr(
                            expr,
                            formals,
                            &format!("{path}.items[{item_idx}].exprs[{expr_idx}]"),
                        )?;
                    }
                    self.validate_stmts(
                        &item.body,
                        formals,
                        &format!("{path}.items[{item_idx}].body"),
                    )?;
                }
            }
            IrStmt::WaitEvents { specs } => {
                for (idx, (source, _)) in specs.iter().enumerate() {
                    if let IrWaitSrc::Event(event) = source {
                        if *event >= self.model.events.len() {
                            return self.fail(
                                format!("{path}.specs[{idx}]"),
                                format!("event index {event} is out of bounds"),
                            );
                        }
                    }
                }
            }
            IrStmt::EventTrigger { ev } => {
                if *ev >= self.model.events.len() {
                    return self.fail(path, format!("event index {ev} is out of bounds"));
                }
            }
            IrStmt::Fork { branches, .. } => {
                let names: HashSet<&str> = branches.iter().map(|(name, _)| name.as_str()).collect();
                if names.len() != branches.len() {
                    return self.fail(path, "fork branch function names are not unique");
                }
            }
            IrStmt::Force { sig, value } => {
                if *sig >= self.model.signals.len() {
                    return self.fail(path, format!("signal index {sig} is out of bounds"));
                }
                self.validate_expr(value, formals, &format!("{path}.value"))?;
            }
            IrStmt::Release { sig } => {
                if *sig >= self.model.signals.len() {
                    return self.fail(path, format!("signal index {sig} is out of bounds"));
                }
            }
            IrStmt::Display { args, .. } => {
                for (idx, (expr, _)) in args.iter().enumerate() {
                    self.validate_expr(expr, formals, &format!("{path}.args[{idx}]"))?;
                }
            }
            IrStmt::WaveLimit(expr) => {
                self.validate_expr(expr, formals, &format!("{path}.limit"))?;
            }
            IrStmt::PrintTimescale {
                unit_ps,
                precision_ps,
                ..
            } => {
                if *unit_ps == 0 || *precision_ps == 0 {
                    return self.fail(path, "timescale units must be non-zero");
                }
            }
            IrStmt::Call(call) => {
                self.validate_call_target(call.f, &call.args, formals, path)?;
                let callee = &self.model.funcs[call.f];
                for (idx, (_, formal, init)) in call.temps.iter().enumerate() {
                    if *formal >= callee.formals.len() {
                        return self.fail(
                            format!("{path}.temps[{idx}]"),
                            format!("formal index {formal} is out of bounds"),
                        );
                    }
                    if let Some(init) = init {
                        self.validate_expr(init, formals, &format!("{path}.temps[{idx}].init"))?;
                    }
                }
                for (idx, (lhs, _, width, _)) in call.copyouts.iter().enumerate() {
                    self.validate_width(*width, &format!("{path}.copyouts[{idx}].width"))?;
                    self.validate_lhs(lhs, formals, &format!("{path}.copyouts[{idx}].lhs"))?;
                }
            }
            IrStmt::Return { value } => {
                if let Some(value) = value {
                    self.validate_expr(value, formals, &format!("{path}.value"))?;
                }
            }
            IrStmt::Delay { .. }
            | IrStmt::WaitAny { .. }
            | IrStmt::WaitFork
            | IrStmt::DisableFork
            | IrStmt::MonitorSet { .. }
            | IrStmt::MonitorEnable(_)
            | IrStmt::WaveFile(_)
            | IrStmt::WaveDumpVars
            | IrStmt::WaveOn
            | IrStmt::WaveOff
            | IrStmt::WaveDumpAll
            | IrStmt::WaveFlush
            | IrStmt::Finish
            | IrStmt::Label(_)
            | IrStmt::Goto(_)
            | IrStmt::Nop => {}
        }
        Ok(())
    }

    fn validate_pre_fns(
        &self,
        pre_fns: &[IrPreFn],
        formals: usize,
        path: &str,
    ) -> ValidationResult {
        for (idx, pre_fn) in pre_fns.iter().enumerate() {
            match pre_fn {
                IrPreFn::Branch { body, .. } => {
                    self.validate_stmts(body, formals, &format!("{path}.pre_fns[{idx}].body"))?;
                }
                IrPreFn::MonEval { args, .. } => {
                    for (arg_idx, arg) in args.iter().enumerate() {
                        self.validate_expr(
                            arg,
                            formals,
                            &format!("{path}.pre_fns[{idx}].args[{arg_idx}]"),
                        )?;
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_init_step(&self, step: &IrInitStep, path: &str) -> ValidationResult {
        match step {
            IrInitStep::FillArrayX(array) | IrInitStep::SetArrayElem { arr: array, .. } => {
                if *array >= self.model.arrays.len() {
                    return self.fail(path, format!("array index {array} is out of bounds"));
                }
            }
            IrInitStep::SetScalar { sig, .. } => {
                if *sig >= self.model.signals.len() {
                    return self.fail(path, format!("signal index {sig} is out of bounds"));
                }
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
        }
    }

    fn validate_spawns(&self, spawns: &[String], path: &str) -> ValidationResult {
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

    fn fail<T>(
        &self,
        path: impl Into<String>,
        detail: impl Into<String>,
    ) -> Result<T, IrValidationError> {
        Err(IrValidationError::new(path, detail))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_model() -> IrModel {
        IrModel {
            design_name: "top".to_string(),
            precision_ps: 1,
            signals: vec![IrSignal {
                c_name: "sig".to_string(),
                hdl_name: Some("sig".to_string()),
                ty: IrType::Packed {
                    width: 1,
                    signed: false,
                },
                net_driver: None,
                omit: false,
            }],
            ..IrModel::default()
        }
    }

    #[test]
    fn rejects_out_of_bounds_signal_reference() {
        let mut model = valid_model();
        model.processes.push(IrProcess {
            c_name: "proc".to_string(),
            label: "top.initial".to_string(),
            shape: IrShape::RunOnce,
            pre_fns: Vec::new(),
            body: vec![IrStmt::Release { sig: 1 }],
        });
        model.spawns.push("proc".to_string());

        let error = model
            .validate()
            .expect_err("invalid signal index must fail");
        assert_eq!(error.path(), "processes[0].body[0]");
        assert!(error.detail().contains("signal index 1"));
    }

    #[test]
    fn rejects_array_total_that_disagrees_with_dimensions() {
        let mut model = valid_model();
        model.arrays.push(IrArray {
            c_name: "memory".to_string(),
            hdl_name: "memory".to_string(),
            elem_width: 8,
            signed: false,
            dims: vec![(3, 0), (1, 0)],
            total: 7,
        });

        let error = model.validate().expect_err("invalid array total must fail");
        assert_eq!(error.path(), "arrays[0].total");
        assert!(error.detail().contains("dimension product 8"));
    }

    #[test]
    fn accepts_a_minimal_well_formed_model() {
        valid_model().validate().expect("minimal model is valid");
    }
}
