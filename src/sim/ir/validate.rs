use std::cell::Cell;
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
    pub(super) fn new(path: impl Into<String>, detail: impl Into<String>) -> Self {
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
    max_width: Cell<u128>,
}

impl IrModel {
    /// Validate all cross-table references and representation invariants.
    ///
    /// Lowering and optimization must call this at their phase boundaries;
    /// the C emitter may then index the model tables without defensive checks.
    pub fn validate(&self) -> Result<(), IrValidationError> {
        self.packed_capacity().map(|_| ())
    }

    /// Validate the model and find its required packed storage capacity.
    /// Includes intermediate expressions, function locals, and initializers.
    pub fn packed_capacity(&self) -> Result<u128, IrValidationError> {
        let validator = Validator::new(self);
        validator.validate()?;
        Ok(validator.max_width.get())
    }

    /// Validate a detached expression against this model's index tables.
    pub fn validate_expr(
        &self,
        expression: &IrExpr,
        function: Option<&IrFunc>,
    ) -> Result<(), IrValidationError> {
        self.expression_capacity(expression, function).map(|_| ())
    }

    /// Validate a detached expression and report its widest packed shape.
    pub fn expression_capacity(
        &self,
        expression: &IrExpr,
        function: Option<&IrFunc>,
    ) -> Result<u128, IrValidationError> {
        let validator = Validator::new(self);
        validator.validate_expr(expression, function.map_or(&[], IrFunc::formals), "expr")?;
        Ok(validator.max_width.get())
    }

    /// Validate a detached statement against this model's index tables.
    pub fn validate_stmt(
        &self,
        statement: &IrStmt,
        function: Option<&IrFunc>,
    ) -> Result<(), IrValidationError> {
        self.statement_capacity(statement, function).map(|_| ())
    }

    /// Validate a detached statement and report its widest packed shape.
    pub fn statement_capacity(
        &self,
        statement: &IrStmt,
        function: Option<&IrFunc>,
    ) -> Result<u128, IrValidationError> {
        let validator = Validator::new(self);
        validator.validate_stmts(
            std::slice::from_ref(statement),
            function.map_or(&[], IrFunc::formals),
            "stmt",
        )?;
        Ok(validator.max_width.get())
    }

    /// Validate a detached helper function against this model's index tables.
    pub fn validate_pre_fn(
        &self,
        pre_fn: &IrPreFn,
        function: Option<&IrFunc>,
    ) -> Result<(), IrValidationError> {
        self.pre_fn_capacity(pre_fn, function).map(|_| ())
    }

    /// Validate a detached helper and report its widest packed shape.
    pub fn pre_fn_capacity(
        &self,
        pre_fn: &IrPreFn,
        function: Option<&IrFunc>,
    ) -> Result<u128, IrValidationError> {
        let validator = Validator::new(self);
        validator.validate_pre_fns(
            std::slice::from_ref(pre_fn),
            function.map_or(&[], IrFunc::formals),
            "pre_fn",
        )?;
        Ok(validator.max_width.get())
    }
}

impl Validator<'_> {
    fn new(model: &IrModel) -> Validator<'_> {
        Validator {
            model,
            max_width: Cell::new(0),
        }
    }

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
            if group.n_drivers > LLG_MAX_NET_DRIVERS {
                return self.fail(
                    format!("{path}.n_drivers"),
                    format!("net group exceeds {LLG_MAX_NET_DRIVERS} drivers"),
                );
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
            self.validate_pre_fns(&func.pre_fns, &func.formals, &path)?;
            self.validate_stmts(&func.body, &func.formals, &format!("{path}.body"))?;
        }

        for (idx, process) in self.model.processes.iter().enumerate() {
            let path = format!("processes[{idx}]");
            self.validate_pre_fns(&process.pre_fns, &[], &path)?;
            self.validate_stmts(&process.body, &[], &format!("{path}.body"))?;
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
        if width == 0 {
            return self.fail(path, "packed width must be nonzero");
        }
        self.max_width
            .set(self.max_width.get().max(u128::from(width)));
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
                if values.get(limbs - 1).copied().unwrap_or(0) & outside != 0 {
                    return self.fail(
                        format!("{path}.{name}"),
                        "high limb contains bits outside the declared width",
                    );
                }
            }
        }
        Ok(())
    }

    fn validate_expr(&self, expr: &IrExpr, formals: &[IrFormal], path: &str) -> ValidationResult {
        self.max_width
            .set(self.max_width.get().max(u128::from(expr.width)));
        if expr.fill.is_some_and(|fill| fill > 3) {
            return self.fail(format!("{path}.fill"), "fill marker must be in 0..=3");
        }
        if expr.width == 0 && expr.fill.is_some() {
            return self.fail(
                format!("{path}.fill"),
                "real expression carries a packed fill marker",
            );
        }
        match &expr.kind {
            IrExprKind::Const(value) => {
                self.validate_const(value, &format!("{path}.const"))?;
                if value.width != expr.width
                    || value.signed != expr.signed
                    || value.fill != expr.fill
                {
                    return self.fail(path, "constant payload type disagrees with expression type");
                }
            }
            IrExprKind::SigRead(idx) => {
                let signal = self.model.signals.get(*idx).ok_or_else(|| {
                    IrValidationError::new(path, format!("signal index {idx} is out of bounds"))
                })?;
                if signal.ty.width() != expr.width || signal.ty.signed() != expr.signed {
                    return self.fail(path, "signal type disagrees with expression type");
                }
            }
            IrExprKind::LocalRead(_) => {}
            IrExprKind::FormalRead(idx) => {
                let formal = formals.get(*idx).ok_or_else(|| {
                    IrValidationError::new(path, format!("formal index {idx} is out of bounds"))
                })?;
                if formal.width != expr.width || formal.signed != expr.signed {
                    return self.fail(path, "formal type disagrees with expression type");
                }
            }
            IrExprKind::CallFn(call) => {
                self.validate_call_expr(call, formals, path)?;
                let callee = &self.model.funcs[call.f];
                match callee.ret {
                    Some(ret) => {
                        if call.void_x {
                            return self.fail(path, "value-returning call is marked as void");
                        }
                        if ret.width() != expr.width || ret.signed() != expr.signed {
                            return self
                                .fail(path, "callee return type disagrees with expression type");
                        }
                    }
                    None => {
                        if !call.void_x {
                            return self.fail(path, "void call used as a value is not marked as X");
                        }
                        if expr.width != 1 || expr.signed || expr.fill.is_some() {
                            return self
                                .fail(path, "void-call fallback must be a 1-bit unsigned X");
                        }
                    }
                }
            }
            IrExprKind::Bin { a, b, .. } | IrExprKind::RealBin { a, b, .. } => {
                self.validate_expr(a, formals, &format!("{path}.a"))?;
                self.validate_expr(b, formals, &format!("{path}.b"))?;
            }
            IrExprKind::Un { a, .. }
            | IrExprKind::RealUn { a, .. }
            | IrExprKind::CastToReal { a, .. }
            | IrExprKind::CastToPacked { a }
            | IrExprKind::Resize { a }
            | IrExprKind::Convert { a }
            | IrExprKind::ToTwoState { a } => {
                self.validate_expr(a, formals, &format!("{path}.a"))?;
            }
            IrExprKind::Mux { sel, a, b } => {
                self.validate_expr(sel, formals, &format!("{path}.sel"))?;
                self.validate_expr(a, formals, &format!("{path}.a"))?;
                self.validate_expr(b, formals, &format!("{path}.b"))?;
            }
            IrExprKind::Concat { parts } | IrExprKind::Replicate { parts, .. } => {
                if parts.is_empty() {
                    return self.fail(path, "concatenation requires at least one operand");
                }
                let sum: u128 = parts.iter().map(|part| u128::from(part.width)).sum();
                let expected = match &expr.kind {
                    IrExprKind::Replicate { count, .. } => sum * u128::from(*count),
                    _ => sum,
                };
                if expected != u128::from(expr.width) {
                    return self.fail(path, format!("concatenation/replication width {} disagrees with derived width {expected}", expr.width));
                }
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
                IrSysFunc::Rtoi(arg) => {
                    self.validate_expr(arg, formals, &format!("{path}.arg"))?;
                    if expr.width != 32 || !expr.signed {
                        return self.fail(path, "$rtoi requires a signed int result");
                    }
                }
                IrSysFunc::Itor(arg) => {
                    self.validate_expr(arg, formals, &format!("{path}.arg"))?;
                    if arg.is_real() || !expr.is_real() {
                        return self.fail(path, "$itor requires a packed argument and real result");
                    }
                }
                IrSysFunc::RealToBits(arg) => {
                    self.validate_expr(arg, formals, &format!("{path}.arg"))?;
                    if expr.width != 64 || expr.signed {
                        return self.fail(path, "$realtobits requires a 64-bit unsigned result");
                    }
                }
                IrSysFunc::BitsToReal(arg) => {
                    self.validate_expr(arg, formals, &format!("{path}.arg"))?;
                    if arg.is_real() || arg.width != 64 || !expr.is_real() {
                        return self.fail(
                            path,
                            "$bitstoreal requires a 64-bit packed argument and real result",
                        );
                    }
                }
                IrSysFunc::ShortRealToBits(arg) => {
                    self.validate_expr(arg, formals, &format!("{path}.arg"))?;
                    if expr.width != 32 || expr.signed {
                        return self
                            .fail(path, "$shortrealtobits requires a 32-bit unsigned result");
                    }
                }
                IrSysFunc::BitsToShortReal(arg) => {
                    self.validate_expr(arg, formals, &format!("{path}.arg"))?;
                    if arg.is_real() || arg.width != 32 || !expr.is_real() {
                        return self.fail(
                            path,
                            "$bitstoshortreal requires a 32-bit packed argument and real result",
                        );
                    }
                }
                IrSysFunc::BitQuery { kind, arg } => {
                    self.validate_expr(arg, formals, &format!("{path}.arg"))?;
                    if arg.is_real() || (expr.width, expr.signed) != kind.result_type() {
                        return self.fail(
                            path,
                            "bit query requires a packed argument and its declared result type",
                        );
                    }
                }
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
        formals: &[IrFormal],
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
        formals: &[IrFormal],
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
        let parameter_order = callee
            .formals
            .iter()
            .filter(|formal| formal.is_out)
            .chain(callee.formals.iter().filter(|formal| !formal.is_out));
        for (idx, (arg, formal)) in args.iter().zip(parameter_order).enumerate() {
            let arg_path = format!("{path}.args[{idx}]");
            match arg {
                IrCallArg::Val(_) if formal.is_out => {
                    return self.fail(arg_path, "output/inout formal requires an address argument");
                }
                IrCallArg::Val(expr) => {
                    self.validate_expr(expr, formals, &arg_path)?;
                    if expr.width != formal.width || expr.signed != formal.signed {
                        return self
                            .fail(arg_path, "input argument type disagrees with its formal");
                    }
                }
                IrCallArg::OutAddr(_) | IrCallArg::OutTemp { .. } if !formal.is_out => {
                    return self.fail(arg_path, "input formal requires a value argument");
                }
                IrCallArg::OutTemp {
                    init: Some(init), ..
                } => {
                    if init.width != formal.width || init.signed != formal.signed {
                        return self.fail(
                            format!("{arg_path}.init"),
                            "output/inout temp initializer type disagrees with its formal",
                        );
                    }
                }
                IrCallArg::OutAddr(_) | IrCallArg::OutTemp { init: None, .. } => {}
            }
        }
        Ok(())
    }

    fn validate_elem_sel(
        &self,
        sel: &IrElemSel,
        formals: &[IrFormal],
        path: &str,
    ) -> ValidationResult {
        match sel {
            IrElemSel::Whole => Ok(()),
            IrElemSel::Part(left, right) => self.validate_select_width(*left, *right, path),
            IrElemSel::Bit(expr) => self.validate_expr(expr, formals, path),
        }
    }

    fn validate_select_width(&self, left: i64, right: i64, _path: &str) -> ValidationResult {
        let width = (i128::from(left) - i128::from(right)).unsigned_abs() + 1;
        self.max_width.set(self.max_width.get().max(width));
        Ok(())
    }

    fn validate_lhs(&self, lhs: &IrLhs, formals: &[IrFormal], path: &str) -> ValidationResult {
        match lhs {
            IrLhs::Whole(signal) | IrLhs::Part(signal, ..) => {
                if *signal >= self.model.signals.len() {
                    return self.fail(path, format!("signal index {signal} is out of bounds"));
                }
                if let IrLhs::Part(_, left, right, _) = lhs {
                    self.validate_select_width(*left, *right, path)?;
                }
            }
            IrLhs::WholeRef { width, .. } => self.validate_width(*width, path)?,
            IrLhs::Bit(signal, index, _) => {
                if *signal >= self.model.signals.len() {
                    return self.fail(path, format!("signal index {signal} is out of bounds"));
                }
                self.validate_expr(index, formals, &format!("{path}.index"))?;
            }
            IrLhs::IdxPart(signal, base, width_expr, width, _, _) => {
                if *signal >= self.model.signals.len() {
                    return self.fail(path, format!("signal index {signal} is out of bounds"));
                }
                self.validate_expr(base, formals, &format!("{path}.base"))?;
                self.validate_expr(width_expr, formals, &format!("{path}.width_expr"))?;
                self.validate_width(*width, &format!("{path}.selected_width"))?;
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

    fn validate_stmts(
        &self,
        stmts: &[IrStmt],
        formals: &[IrFormal],
        path: &str,
    ) -> ValidationResult {
        for (idx, stmt) in stmts.iter().enumerate() {
            self.validate_stmt(stmt, formals, &format!("{path}[{idx}]"))?;
        }
        Ok(())
    }

    fn validate_stmt(&self, stmt: &IrStmt, formals: &[IrFormal], path: &str) -> ValidationResult {
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
                    let formal_ty = callee.formals.get(*formal).ok_or_else(|| {
                        IrValidationError::new(
                            format!("{path}.temps[{idx}]"),
                            format!("formal index {formal} is out of bounds"),
                        )
                    })?;
                    if !formal_ty.is_out {
                        return self.fail(
                            format!("{path}.temps[{idx}]"),
                            "call temp refers to an input formal",
                        );
                    }
                    if let Some(init) = init {
                        self.validate_expr(init, formals, &format!("{path}.temps[{idx}].init"))?;
                        if init.width != formal_ty.width || init.signed != formal_ty.signed {
                            return self.fail(
                                format!("{path}.temps[{idx}].init"),
                                "call temp initializer type disagrees with its formal",
                            );
                        }
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
        formals: &[IrFormal],
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
        let mut model = IrModel::new("top".to_string(), 1).unwrap();
        model.signals = vec![IrSignal {
            c_name: "sig".to_string(),
            hdl_name: Some("sig".to_string()),
            ty: IrType::Packed {
                width: 1,
                signed: false,
                two_state: false,
            },
            net_driver: None,
            omit: false,
        }];
        model
    }

    fn packed_const(value: u64, width: u32) -> IrExpr {
        IrExpr::new(
            IrExprKind::Const(
                IrConst::packed(vec![value], vec![], vec![], width, false, None).unwrap(),
            ),
            width,
            false,
            None,
        )
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
            two_state: false,
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

    #[test]
    fn indexed_lhs_selected_width_contributes_to_capacity() {
        let statement = IrStmt::Assign {
            lhs: IrLhs::IdxPart(
                0,
                packed_const(0, 32),
                packed_const(96, 32),
                96,
                false,
                false,
            ),
            rhs: packed_const(1, 1),
            nba: false,
        };

        assert_eq!(
            valid_model().statement_capacity(&statement, None).unwrap(),
            96
        );
    }

    #[test]
    fn indexed_lhs_preserves_wide_base_expression_capacity() {
        let part = || packed_const(0, 32);
        let base = IrExpr::new(
            IrExprKind::Concat {
                parts: vec![part(), part(), part()],
            },
            96,
            false,
            None,
        );
        let statement = IrStmt::Assign {
            lhs: IrLhs::IdxPart(0, base, packed_const(8, 32), 8, false, false),
            rhs: packed_const(1, 1),
            nba: false,
        };

        assert_eq!(
            valid_model().statement_capacity(&statement, None).unwrap(),
            96
        );
    }

    #[test]
    fn constructors_reject_invalid_local_invariants() {
        assert!(IrModel::new("top".to_string(), 0).is_err());
        assert!(IrType::packed(0, false).is_err());
        assert!(IrExpr::try_new(IrExprKind::Fill(4), 1, false, Some(4)).is_err());
        assert!(IrArray::new("a".into(), "a".into(), 8, false, Vec::new()).is_err());
        assert!(IrNetGroup::new("n".into(), 1, false, IrNetKind::Wire, 0).is_err());
        assert!(IrNetGroup::new(
            "n".into(),
            1,
            false,
            IrNetKind::Wire,
            LLG_MAX_NET_DRIVERS + 1,
        )
        .is_err());
    }

    #[test]
    fn packed_constructor_checks_the_declared_high_limb() {
        let value = IrConst::packed(vec![u64::MAX, 1], vec![], vec![], 65, false, None)
            .expect("bit 64 is inside a 65-bit value");
        assert_eq!(value.bits(), &[u64::MAX, 1]);

        let error = IrConst::packed(vec![0, 2], vec![], vec![], 65, false, None)
            .expect_err("bit 65 lies outside a 65-bit value");
        assert_eq!(error.path(), "const.bits");
    }

    #[test]
    fn public_parts_build_a_nonempty_valid_model() {
        let signal = IrSignal::new(
            "sig".to_string(),
            Some("top.sig".to_string()),
            IrType::packed(1, false).unwrap(),
            None,
        )
        .unwrap();
        let model = IrModel::from_parts(
            "top".to_string(),
            1,
            IrModelParts {
                signals: vec![signal],
                ..IrModelParts::default()
            },
        )
        .expect("a nonempty valid model must be constructible through the public API");
        assert_eq!(model.signals().len(), 1);
        assert_eq!(model.signal(0).hdl_name(), Some("top.sig"));
    }

    #[test]
    fn public_parts_reject_invalid_cross_table_references() {
        let process = IrProcess::new(
            "proc".to_string(),
            "top.initial".to_string(),
            IrShape::RunOnce,
            Vec::new(),
            vec![IrStmt::Release { sig: 0 }],
        );
        let error = IrModel::from_parts(
            "top".to_string(),
            1,
            IrModelParts {
                processes: vec![process],
                spawns: vec!["proc".to_string()],
                ..IrModelParts::default()
            },
        )
        .expect_err("a model cannot reference a missing signal");
        assert_eq!(error.path(), "processes[0].body[0]");
    }

    #[test]
    fn detached_nested_expression_checks_exact_formal_type() {
        let context = IrFunc::new(
            "context".to_string(),
            None,
            vec![IrFormal::new(false, 8, true).unwrap()],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        let expression = IrExpr::new(
            IrExprKind::Bin {
                op: IrBinOp::Add,
                a: Box::new(IrExpr::new(IrExprKind::FormalRead(0), 4, false, None)),
                b: Box::new(IrExpr::new(
                    IrExprKind::Const(
                        IrConst::packed(vec![1], vec![], vec![], 4, false, None).unwrap(),
                    ),
                    4,
                    false,
                    None,
                )),
            },
            4,
            false,
            None,
        );

        let error = valid_model()
            .validate_expr(&expression, Some(&context))
            .expect_err("a nested formal read must carry the formal's exact type");
        assert_eq!(error.path(), "expr.a");
        assert!(error.detail().contains("formal type"));
    }

    #[test]
    fn call_arguments_follow_output_then_input_parameter_order() {
        let callee = IrFunc::new(
            "callee".to_string(),
            Some(IrType::packed(1, false).unwrap()),
            vec![
                IrFormal::new(false, 8, false).unwrap(),
                IrFormal::new(true, 16, true).unwrap(),
            ],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        let model = IrModel::from_parts(
            "top".to_string(),
            1,
            IrModelParts {
                funcs: vec![callee],
                ..IrModelParts::default()
            },
        )
        .unwrap();
        let input = IrExpr::new(
            IrExprKind::Const(IrConst::packed(vec![7], vec![], vec![], 8, false, None).unwrap()),
            8,
            false,
            None,
        );
        let valid = IrExpr::new(
            IrExprKind::CallFn(Box::new(IrCallExpr::new(
                0,
                vec![
                    IrCallArg::OutAddr("&out".to_string()),
                    IrCallArg::Val(input.clone()),
                ],
                IrDepth::PROC,
                false,
            ))),
            1,
            false,
            None,
        );
        model
            .validate_expr(&valid, None)
            .expect("C-order output then input arguments are valid");

        let invalid = IrExpr::new(
            IrExprKind::CallFn(Box::new(IrCallExpr::new(
                0,
                vec![
                    IrCallArg::Val(input),
                    IrCallArg::OutAddr("&out".to_string()),
                ],
                IrDepth::PROC,
                false,
            ))),
            1,
            false,
            None,
        );
        let error = model
            .validate_expr(&invalid, None)
            .expect_err("argument variants must agree with formal directions");
        assert_eq!(error.path(), "expr.args[0]");
        assert!(error.detail().contains("address argument"));
    }
}
