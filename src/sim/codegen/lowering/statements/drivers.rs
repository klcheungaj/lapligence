//! Drivers.

use super::super::collection::EventEvaluation;
use super::*;
use crate::sim::ir::{IrRedrive, IrRedriveAction, IrRedriveTarget, IrRuntimeQuery};
use crate::sim::legacy_unsupported::LegacyFamily;

impl EmitCtx<'_, '_> {
    /// Lower `force lhs = rhs;` into a live runtime binding. The target keeps
    /// its canonical assignment shape so selected and concatenated net
    /// targets can be overlaid without losing their underlying drivers.
    pub(super) fn lower_force(&mut self, h: NodeId) -> Result<IrStmt, String> {
        let NodeKind::Stmt(StmtKind::Force { lhs, rhs }) = self.cg.kind(h) else {
            return Err("expected a typed force statement".to_string());
        };
        let diagnostic_path = self.force_diagnostic_path(h);
        let (lhs, rhs) = (*lhs, *rhs);
        if let Some(target) = self.nested_subroutine_auto_ref(lhs) {
            return Err(format!(
                "force target in `{diagnostic_path}` cannot refer to automatic subroutine storage `{}`",
                self.cg.node(target).name
            ));
        }
        if self.cg.nested_proc_local_ref(rhs).is_some() {
            return Err(format!(
                "force RHS in `{diagnostic_path}` cannot capture an automatic procedural local"
            ));
        }
        if let Some(target) = self.nested_subroutine_auto_ref(rhs) {
            return Err(format!(
                "force RHS in `{diagnostic_path}` cannot capture automatic subroutine storage `{}`",
                self.cg.node(target).name
            ));
        }
        if let Some(target) = self.cg.nested_capture_ref(rhs) {
            return Err(format!(
                "force RHS in `{diagnostic_path}` cannot capture activation storage `{}`",
                self.cg.node(target).name
            ));
        }
        // A force evaluator is re-run by the runtime inside the write that
        // changed its source, so it must be a read-only callback. A legal
        // helper with effects is instead evaluated by processes into a hidden
        // source that the callback merely reads (see `effectful_force`).
        let effectful = matches!(
            self.cg.classify_event_expression(rhs, &diagnostic_path),
            Ok(EventEvaluation::Process(_))
        );
        self.check_force_target_type(lhs, "force", &diagnostic_path)?;
        let lh = self
            .cg
            .lower_lhs(&self.path, lhs)
            .map_err(|error| self.force_error(&diagnostic_path, error))?;
        let lh = normalize_force_lhs(&self.cg.model, lh);
        let target_real = validate_force_lhs(&self.cg.model, &lh, &diagnostic_path)?;
        let value = self.force_value(rhs, &lh, target_real, &diagnostic_path)?;
        if effectful {
            return self.effectful_force(h, rhs, lh, value, target_real, &diagnostic_path);
        }
        let eval = self.cg.new_fn_name(&self.path, "force_eval");
        self.pre_fns.push(crate::sim::ir::IrPreFn::ForceEval {
            c_name: eval.clone(),
            value: value.clone(),
            real: target_real,
        });
        let (read_names, dependencies) =
            self.cg.collect_force_read_signals(&diagnostic_path, rhs)?;
        let mut reads = Vec::with_capacity(read_names.len());
        for name in read_names {
            let index = self
                .cg
                .model
                .signals
                .iter()
                .position(|signal| signal.c_name == name)
                .or_else(|| {
                    name.strip_prefix("llg_net_alias_")
                        .and_then(|name| name.strip_suffix(".visible"))
                        .and_then(|index| index.parse::<usize>().ok())
                        .filter(|index| {
                            self.cg
                                .model
                                .signals
                                .get(*index)
                                .is_some_and(|signal| !signal.net_alias.is_empty())
                        })
                })
                .ok_or_else(|| {
                    format!(
                        "force dependency `{name}` in `{diagnostic_path}` has no lowered storage"
                    )
                })?;
            if !reads.contains(&index) {
                reads.push(index);
            }
        }
        Ok(IrStmt::Force {
            lhs: lh,
            value: Box::new(value),
            eval,
            reads,
            dependencies,
        })
    }

    /// The force RHS converted to its target: a real cast for a real target,
    /// otherwise the packed target width and signedness.
    fn force_value(
        &mut self,
        rhs: NodeId,
        lh: &IrLhs,
        target_real: bool,
        diagnostic_path: &str,
    ) -> Result<IrExpr, String> {
        let rhs_ir = self
            .cg
            .lower_expr(&self.path, rhs)
            .map_err(|error| self.force_error(diagnostic_path, error))?;
        Ok(if target_real {
            let shortreal = match lh {
                IrLhs::Whole(index) => matches!(
                    self.cg.model.signal(*index).ty,
                    IrType::Real { shortreal: true }
                ),
                IrLhs::ArrayElem { arr, .. } => self.cg.model.array(*arr).shortreal,
                _ => {
                    return Err(format!(
                        "real force target in `{diagnostic_path}` must be a whole variable or array element"
                    ))
                }
            };
            IrExpr::new(
                IrExprKind::CastToReal {
                    a: Box::new(rhs_ir),
                    shortreal,
                },
                0,
                true,
                None,
            )
        } else {
            let width = packed_lhs_width(&self.cg.model, lh)
                .ok_or_else(|| format!("force target in `{}` has no packed width", self.path))?;
            let signed = force_lhs_signed(&self.cg.model, lh);
            let rhs_ir = apply_assignment_expression_width(rhs_ir, width);
            // Force uses the same value-preserving RHS-to-target conversion as
            // ordinary assignments (LRM §10.7). Per-part two-state coercion is
            // applied by the runtime when a force spans a concatenation.
            ir_to_vector(rhs_ir, width, signed)?
        })
    }

    /// Lower a force whose RHS calls a legal helper with effects (visible or
    /// persistent writes, unproved static results or descriptor transport).
    ///
    /// SV 10.6 treats a force RHS like a continuous assignment that is
    /// re-evaluated whenever an operand changes while the force is in effect.
    /// The force statement evaluates the RHS in its own process into a hidden
    /// per-site source, then binds the target to that source with an ordinary
    /// read-only evaluator, so the force takes effect immediately (SV 10.6.2).
    /// A synthetic guard process per site waits on the RHS read set and
    /// re-evaluates it into the source only while a live force binding still
    /// reads that source. The helper's effects therefore publish from a
    /// process in the Active region, like a continuous assignment's, and never
    /// run after release or replacement. Evaluation counts are unspecified:
    /// one at the force statement and one per guard wake while in effect.
    fn effectful_force(
        &mut self,
        h: NodeId,
        rhs: NodeId,
        lhs: IrLhs,
        value: IrExpr,
        real: bool,
        diagnostic_path: &str,
    ) -> Result<IrStmt, String> {
        let source = self.cg.model.signals.len();
        let ty = if real {
            let IrExprKind::CastToReal { shortreal, .. } = value.kind() else {
                return Err("real force value lost its real conversion".to_owned());
            };
            IrType::Real {
                shortreal: *shortreal,
            }
        } else {
            IrType::Packed {
                width: value.width,
                signed: value.signed,
                two_state: false,
            }
        };
        self.cg.model.signals.push(IrSignal {
            fixed_default: None,
            c_name: format!("llg_force_src_{source}"),
            hdl_name: None,
            ty,
            net_driver: None,
            net_alias: Vec::new(),
            alias: None,
            omit: false,
        });
        let read = IrExpr::new(IrExprKind::SigRead(source), ty.width(), ty.signed(), None);
        // The guard is a top-level process: re-lower the RHS at process call
        // depth, since a force inside a task body lowers its calls relative
        // to that body's frame.
        let saved_depth = std::mem::replace(&mut self.cg.depth_arg, "0".to_owned());
        let guard_value = self.force_value(rhs, &lhs, real, diagnostic_path);
        self.cg.depth_arg = saved_depth;
        let guard_value = guard_value?;
        let evaluate = IrStmt::Assign {
            lhs: IrLhs::Whole(source),
            rhs: value,
            nba: false,
        };
        let sens = self.cg.collect_evaluator_sensitivity(&self.path, rhs)?;
        let guard = vec![
            IrStmt::WaitAny {
                sens,
                refresh: false,
            },
            IrStmt::If {
                cond: IrExpr::new(
                    IrExprKind::RuntimeQuery(IrRuntimeQuery::ForceSourceActive(source)),
                    1,
                    false,
                    None,
                ),
                then_: vec![IrStmt::Assign {
                    lhs: IrLhs::Whole(source),
                    rhs: guard_value,
                    nba: false,
                }],
                els: None,
                check: IrUniquePriorityCheck::None,
            },
        ];
        let guard_name = self.cg.new_fn_name(&self.path, "force_source");
        let origin = self.cg.origin(h);
        self.cg.model.processes.push(IrProcess::new_with_origin(
            guard_name,
            format!("{}.force_source", self.path),
            IrShape::Loop,
            Vec::new(),
            guard,
            origin,
        ));
        let eval = self.cg.new_fn_name(&self.path, "force_eval");
        self.pre_fns.push(crate::sim::ir::IrPreFn::ForceEval {
            c_name: eval.clone(),
            value: read.clone(),
            real,
        });
        Ok(IrStmt::Block(vec![
            evaluate,
            IrStmt::Force {
                lhs,
                value: Box::new(read),
                eval,
                reads: vec![source],
                dependencies: Vec::new(),
            },
        ]))
    }

    /// Lower `release lhs;` against the same canonical target descriptor used
    /// by `force`, so selected nets release only their matching overlay.
    pub(super) fn lower_release(&mut self, h: NodeId) -> Result<IrStmt, String> {
        let NodeKind::Stmt(StmtKind::Release { lhs }) = self.cg.kind(h) else {
            return Err("expected a typed release statement".to_string());
        };
        let diagnostic_path = self.force_diagnostic_path(h);
        let lhs = *lhs;
        if let Some(target) = self.nested_subroutine_auto_ref(lhs) {
            return Err(format!(
                "release target in `{diagnostic_path}` cannot refer to automatic subroutine storage `{}`",
                self.cg.node(target).name
            ));
        }
        self.check_force_target_type(lhs, "release", &diagnostic_path)?;
        let lh = self
            .cg
            .lower_lhs(&self.path, lhs)
            .map_err(|error| self.force_error(&diagnostic_path, error))?;
        let lh = normalize_force_lhs(&self.cg.model, lh);
        validate_force_lhs(&self.cg.model, &lh, &diagnostic_path)?;
        let mut released = Vec::new();
        released_variables(&self.cg.model, &lh, &mut released);
        let mut released_arrays = Vec::new();
        released_array_variables(&lh, &mut released_arrays);
        // IEEE 1800-2009 10.6.2: releasing a variable driven by a continuous
        // assignment re-establishes it. The runtime does that inside the
        // release, from the model's redrive rows; `collect_redrives` gives
        // the variables named here their rows once every driver is lowered.
        self.cg
            .released_targets
            .extend(released.into_iter().map(IrRedriveTarget::Signal));
        self.cg
            .released_targets
            .extend(released_arrays.into_iter().map(IrRedriveTarget::Array));
        Ok(IrStmt::Release { lhs: lh })
    }

    /// Reject force/release targets by declared type before lowering, so each
    /// gets a located, specific message. IEEE 1800-2009 10.6.2 admits "a
    /// singular variable, a net, a constant bit-select of a vector net, a
    /// constant part-select of a vector net, or a concatenation of these";
    /// unpacked arrays, structures and unions are not singular (6.4).
    fn check_force_target_type(
        &self,
        lhs: NodeId,
        statement: &str,
        diagnostic_path: &str,
    ) -> Result<(), String> {
        if let NodeKind::Expr(ExprKind::Operation {
            op: Operation::Concat,
            operands,
            ..
        }) = self.cg.kind(lhs)
        {
            for operand in operands.clone() {
                self.check_force_target_type(operand, statement, diagnostic_path)?;
            }
            return Ok(());
        }
        let Some(descriptor) = self.cg.query_descriptor(lhs) else {
            return Ok(());
        };
        let name = match self.cg.kind(lhs) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => self.cg.node(*target).name.clone(),
            _ => self.cg.node(lhs).name.clone(),
        };
        let unpacked = match &descriptor.shape {
            TypeShape::FixedArray { .. } | TypeShape::Container { .. } => Some("an unpacked array"),
            TypeShape::Aggregate(layout)
                if matches!(
                    layout.kind,
                    AggregateKind::UnpackedStruct | AggregateKind::UnpackedUnion
                ) =>
            {
                Some("an unpacked structure or union")
            }
            _ => None,
        };
        if let Some(what) = unpacked {
            // A whole net array is a net; its cells are forced together.
            if self
                .cg
                .array_net_target_parts(lhs)
                .is_some_and(|(array, indices)| array.is_net && indices.is_empty())
            {
                return Ok(());
            }
            return Err(format!(
                "{statement} target `{name}` in `{diagnostic_path}` is {what}, which is not a singular variable (IEEE 1800-2009 10.6.2)"
            ));
        }
        let kind = match &descriptor.shape {
            TypeShape::String => Some("string"),
            TypeShape::Opaque { kind } if kind == "Class" => Some("class handle"),
            TypeShape::Opaque { kind } if kind == "Chandle" => Some("chandle"),
            TypeShape::Opaque { kind } if kind == "Event" => Some("event"),
            TypeShape::Opaque { .. } => Some("handle"),
            _ => None,
        };
        // Industry-standard behaviour rejects these singular (6.4) but
        // dynamic or opaque targets, and so does llg (decision S23-D5).
        if let Some(kind) = kind {
            return Err(format!(
                "{statement} of the {kind} variable `{name}` in `{diagnostic_path}` is rejected: string, class-handle, chandle and event variables are not force or release targets (decision S23-D5); force/release supports packed, real and enumerated variables and nets"
            ));
        }
        Ok(())
    }

    pub(super) fn nested_subroutine_auto_ref(&self, node: NodeId) -> Option<NodeId> {
        if let Some(target) = self.cg.subroutine_auto_ref(node) {
            return Some(target);
        }
        self.cg
            .node(node)
            .children
            .iter()
            .find_map(|child| self.nested_subroutine_auto_ref(*child))
    }

    fn force_error(&self, diagnostic_path: &str, error: String) -> String {
        if diagnostic_path == self.path {
            error
        } else {
            format!("{error} (source location: {diagnostic_path})")
        }
    }

    fn force_diagnostic_path(&self, node: NodeId) -> String {
        let source = self.cg.node(node);
        match (source.file.as_deref(), source.line, source.col) {
            (Some(file), line, col) if line > 0 => {
                format!("{} at {file}:{line}:{col}", self.path)
            }
            _ => self.path.clone(),
        }
    }

    /// Source-located rejection of a construct unsupported by design.
    pub(super) fn legacy_unsupported(
        &self,
        node: NodeId,
        construct: &str,
        family: LegacyFamily,
    ) -> String {
        self.cg
            .legacy_unsupported_at(&self.path, node, construct, family)
    }

    /// Procedural `assign`/`deassign` (IEEE 1364-2001 9.3.1) is unsupported
    /// in every form by user decision (2026-10-09). `legacy_unsupported::scan`
    /// reports every reachable statement before lowering; this covers
    /// statements the reachability walk does not visit (for example class
    /// method bodies).
    pub(super) fn reject_procedural_assign(&self, h: NodeId, stmt: &str) -> String {
        self.legacy_unsupported(
            h,
            &format!("procedural `{stmt}`"),
            LegacyFamily::ProceduralAssign,
        )
    }
}

/// Whether signal `index` (after alias resolution) is a variable: a
/// released variable keeps its value, a released net resolves its drivers.
fn is_variable(model: &IrModel, index: usize) -> bool {
    let signal = model.signal(index);
    signal.net_driver.is_none() && signal.net_alias.is_empty()
}

/// Persistent variables (not nets) released by `lhs`, in target order.
fn released_variables(model: &IrModel, lhs: &IrLhs, out: &mut Vec<usize>) {
    match lhs {
        // A record leaf, whole or a member inside it, selected by an element
        // release; with a run-time index every leaf may be the one released.
        IrLhs::Whole(index) | IrLhs::Part(index, ..) => {
            let index = model.signal(*index).alias.unwrap_or(*index);
            if is_variable(model, index) && !out.contains(&index) {
                out.push(index);
            }
        }
        IrLhs::Stream { parts, .. } => {
            for (part, _) in parts {
                released_variables(model, part, out);
            }
        }
        IrLhs::PackedSelect { target, .. } => released_variables(model, target, out),
        _ => {}
    }
}

/// Array variables whose element a `release` names.
fn released_array_variables(lhs: &IrLhs, out: &mut Vec<usize>) {
    match lhs {
        IrLhs::ArrayElem { arr, .. } => {
            if !out.contains(arr) {
                out.push(*arr);
            }
        }
        IrLhs::Stream { parts, .. } => {
            for (part, _) in parts {
                released_array_variables(part, out);
            }
        }
        _ => {}
    }
}

impl Codegen<'_> {
    /// Give every continuous driver of a releasable variable its redrive row
    /// (IEEE 1800-2009 10.6.2): the variables and arrays `release`
    /// statements name and, with `vpi_release`, every continuously driven
    /// variable, because a VPI application may release any of them (38.34).
    /// Continuous assignments and port links are the synthetic
    /// evaluate-then-wait processes; procedural writers have no row, so a
    /// variable they write keeps its forced value until the next procedural
    /// assignment. The row holds a copy of the driver's statements (or names
    /// the copied source, or the driver's committed value when it is
    /// delayed), so later process merging or reordering cannot invalidate
    /// it and the drivers themselves stay as they are. Runs once after
    /// every process and subroutine body is lowered; a design without a
    /// release statement and without a VPI application has no rows.
    pub(in super::super) fn collect_redrives(&mut self, vpi_release: bool) -> Result<(), String> {
        if !vpi_release && self.released_targets.is_empty() {
            return Ok(());
        }
        let mut redrives = Vec::new();
        for process in &self.model.processes {
            if process.kind != IrProcessKind::Synthetic
                || !matches!(process.shape, IrShape::RunOnce | IrShape::SensLoop { .. })
            {
                continue;
            }
            let mut written = Vec::new();
            let mut written_arrays = Vec::new();
            for statement in &process.body {
                continuous_writes(statement, &mut written, &mut written_arrays);
            }
            let mut targets: Vec<IrRedriveTarget> = written
                .into_iter()
                .map(|signal| self.model.signals[signal].alias.unwrap_or(signal))
                .filter(|signal| is_variable(&self.model, *signal))
                .map(IrRedriveTarget::Signal)
                .chain(written_arrays.into_iter().map(IrRedriveTarget::Array))
                .filter(|target| vpi_release || self.released_targets.contains(target))
                .collect();
            if targets.is_empty() {
                continue;
            }
            targets.sort_unstable();
            targets.dedup();
            let Some(action) = redrive_action(&self.model, process) else {
                // A release statement must find its driver; a variable only
                // a VPI application could release keeps the forced value
                // until the driver next runs.
                if targets
                    .iter()
                    .any(|target| self.released_targets.contains(target))
                {
                    return Err(format!(
                        "release of a variable driven by `{}`: this continuous driver cannot be \
                         re-established by a release",
                        process.label
                    ));
                }
                continue;
            };
            redrives.push(IrRedrive { targets, action });
        }
        self.model.redrives = redrives;
        Ok(())
    }
}

/// How a release repeats the assignment of continuous driver `process`, or
/// `None` for a driver whose statements cannot run outside its process: one
/// with helper functions of its own, or a delayed driver of a part of its
/// target (its record keeps no part to assign again).
fn redrive_action(model: &IrModel, process: &IrProcess) -> Option<IrRedriveAction> {
    if !process.pre_fns.is_empty() {
        return None;
    }
    let mut delayed = Vec::new();
    for statement in &process.body {
        delayed_writes(statement, &mut delayed);
    }
    if !delayed.is_empty() {
        let whole = delayed.iter().all(|lhs| {
            matches!(
                lhs,
                IrLhs::Whole(_)
                    | IrLhs::ArrayElem {
                        elem_sel: IrElemSel::Whole,
                        ..
                    }
            )
        });
        return whole.then_some(IrRedriveAction::Committed);
    }
    if let [statement] = process.body.as_slice() {
        let mut statement = statement;
        while let IrStmt::Located {
            statement: inner, ..
        } = statement
        {
            statement = inner;
        }
        if let IrStmt::Assign {
            lhs: IrLhs::Whole(target),
            rhs,
            nba: false,
        } = statement
        {
            if let IrExprKind::SigRead(source) = rhs.kind() {
                let (to, from) = (model.signal(*target), model.signal(*source));
                let plain = |signal: &IrSignal| {
                    signal.alias.is_none()
                        && signal.net_driver.is_none()
                        && signal.net_alias.is_empty()
                };
                if plain(to)
                    && plain(from)
                    && matches!(to.ty, IrType::Packed { .. })
                    && to.ty == from.ty
                    && rhs.width() == to.ty.width()
                {
                    return Some(IrRedriveAction::Copy(*source));
                }
            }
        }
    }
    Some(IrRedriveAction::Evaluate(process.body.clone()))
}

/// The targets of the delayed assignments of a continuous driver body.
fn delayed_writes<'a>(statement: &'a IrStmt, out: &mut Vec<&'a IrLhs>) {
    match statement {
        IrStmt::InertialAssign { lhs, .. } | IrStmt::DelayedAssign { lhs, .. } => out.push(lhs),
        IrStmt::InertialEnable(enable) => {
            out.push(&enable.zero_lhs);
            out.push(&enable.one_lhs);
        }
        IrStmt::Located { statement, .. } => delayed_writes(statement, out),
        IrStmt::Block(statements)
        | IrStmt::While {
            body: statements, ..
        } => {
            for statement in statements {
                delayed_writes(statement, out);
            }
        }
        IrStmt::If { then_, els, .. } => {
            for statement in then_.iter().chain(els.iter().flatten()) {
                delayed_writes(statement, out);
            }
        }
        _ => {}
    }
}

/// Whole persistent signals, and arrays, a continuous driver body assigns.
fn continuous_writes(statement: &IrStmt, out: &mut Vec<usize>, arrays: &mut Vec<usize>) {
    match statement {
        IrStmt::Assign { lhs, .. } | IrStmt::InertialAssign { lhs, .. } => {
            lhs_signals(lhs, out, arrays);
        }
        IrStmt::InertialEnable(enable) => {
            lhs_signals(&enable.zero_lhs, out, arrays);
            lhs_signals(&enable.one_lhs, out, arrays);
        }
        IrStmt::Located { statement, .. } => continuous_writes(statement, out, arrays),
        IrStmt::Block(statements)
        | IrStmt::While {
            body: statements, ..
        } => {
            for statement in statements {
                continuous_writes(statement, out, arrays);
            }
        }
        IrStmt::If { then_, els, .. } => {
            for statement in then_.iter().chain(els.iter().flatten()) {
                continuous_writes(statement, out, arrays);
            }
        }
        _ => {}
    }
}

fn lhs_signals(lhs: &IrLhs, out: &mut Vec<usize>, arrays: &mut Vec<usize>) {
    match lhs {
        IrLhs::Whole(index)
        | IrLhs::Bit(index, ..)
        | IrLhs::Part(index, ..)
        | IrLhs::IdxPart(index, ..) => out.push(*index),
        IrLhs::ArrayElem { arr, .. } => arrays.push(*arr),
        IrLhs::PackedSelect { target, .. } | IrLhs::TaggedSelect { target, .. } => {
            lhs_signals(target, out, arrays);
        }
        IrLhs::Stream { parts, .. } => {
            for (part, _) in parts {
                lhs_signals(part, out, arrays);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::ir::IrPackedSelect;

    fn constant(value: i64, width: u32, signed: bool) -> IrExpr {
        let bits = if width >= 64 {
            value as u64
        } else {
            (value as u64) & ((1u64 << width) - 1)
        };
        IrExpr::new(
            IrExprKind::Const(IrConst {
                bits: vec![bits],
                x: vec![0],
                z: vec![0],
                width,
                signed,
                real: None,
                fill: None,
            }),
            width,
            signed,
            None,
        )
    }

    fn binary(op: IrBinOp, a: IrExpr, b: IrExpr) -> IrExpr {
        IrExpr::new(
            IrExprKind::Bin {
                op,
                a: Box::new(a),
                b: Box::new(b),
            },
            33,
            true,
            None,
        )
    }

    fn model_with(signals: Vec<IrSignal>) -> IrModel {
        let mut model = IrModel::new("t".to_owned(), 1).expect("model");
        model.signals = signals;
        model
    }

    fn packed(name: &str, width: u32, net: Option<(usize, usize)>) -> IrSignal {
        IrSignal::new(
            name.to_owned(),
            None,
            IrType::Packed {
                width,
                signed: false,
                two_state: false,
            },
            net,
        )
        .expect("signal")
    }

    #[test]
    fn component_force_selector_constants_fold_normalized_ranges() {
        // `n[3 +: 2]` of `wire [8:1] n` lowers to `3 - 1`.
        let base = binary(IrBinOp::Sub, constant(3, 32, true), constant(1, 32, false));
        assert_eq!(force_constant_i64(&base), Some(2));
        assert_eq!(force_constant_i64(&constant(-2, 8, true)), Some(-2));
        let product = binary(IrBinOp::Mul, constant(4, 32, false), constant(3, 32, false));
        assert_eq!(force_constant_i64(&product), Some(12));
        let read = IrExpr::new(IrExprKind::SigRead(0), 32, false, None);
        assert_eq!(force_constant_i64(&read), None);
        let unknown = IrExpr::new(
            IrExprKind::Const(IrConst {
                bits: vec![0],
                x: vec![1],
                z: vec![0],
                width: 4,
                signed: false,
                real: None,
                fill: None,
            }),
            4,
            false,
            None,
        );
        assert_eq!(force_constant_i64(&unknown), None);
    }

    #[test]
    fn component_force_net_selects_become_fixed_parts_and_variables_do_not() {
        let model = model_with(vec![packed("net", 8, Some((0, 0))), packed("var", 8, None)]);
        let indexed = |signal: usize, negative: bool| {
            IrLhs::IdxPart(
                signal,
                Box::new(constant(5, 32, true)),
                Box::new(constant(2, 32, true)),
                2,
                negative,
                false,
            )
        };
        assert_eq!(
            normalize_force_lhs(&model, indexed(0, false)),
            IrLhs::Part(0, 6, 5, false)
        );
        assert_eq!(
            normalize_force_lhs(&model, indexed(0, true)),
            IrLhs::Part(0, 5, 4, false)
        );
        assert_eq!(
            normalize_force_lhs(&model, indexed(1, false)),
            indexed(1, false)
        );
        let member = |signal: usize| IrLhs::PackedSelect {
            target: Box::new(IrLhs::Whole(signal)),
            steps: vec![
                IrPackedSelect {
                    base: constant(4, 32, false),
                    width: 4,
                },
                IrPackedSelect {
                    base: constant(1, 32, false),
                    width: 2,
                },
            ],
            signed: false,
            two_state: false,
        };
        assert_eq!(
            normalize_force_lhs(&model, member(0)),
            IrLhs::Part(0, 6, 5, false)
        );
        assert_eq!(normalize_force_lhs(&model, member(1)), member(1));
        // A step outside its parent selection is left for validation.
        let outside = IrLhs::PackedSelect {
            target: Box::new(IrLhs::Whole(0)),
            steps: vec![IrPackedSelect {
                base: constant(6, 32, false),
                width: 4,
            }],
            signed: false,
            two_state: false,
        };
        assert_eq!(normalize_force_lhs(&model, outside.clone()), outside);
    }

    #[test]
    fn component_redrive_actions_follow_the_driver_shape() {
        let model = model_with(vec![
            packed("target", 4, None),
            packed("source", 4, None),
            packed("wide", 8, None),
        ]);
        let read = |signal: usize, width: u32| {
            IrExpr::new(IrExprKind::SigRead(signal), width, false, None)
        };
        let driver = |body: Vec<IrStmt>| {
            IrProcess::new(
                "p".to_owned(),
                "t.assign".to_owned(),
                IrShape::SensLoop { reads: Vec::new() },
                Vec::new(),
                body,
            )
        };
        let assign = |rhs: IrExpr| IrStmt::Assign {
            lhs: IrLhs::Whole(0),
            rhs,
            nba: false,
        };
        // A whole signal of the same type is copied.
        assert_eq!(
            redrive_action(&model, &driver(vec![assign(read(1, 4))])),
            Some(IrRedriveAction::Copy(1))
        );
        // Another width converts: the statements run.
        let widened = vec![assign(read(2, 8))];
        assert_eq!(
            redrive_action(&model, &driver(widened.clone())),
            Some(IrRedriveAction::Evaluate(widened))
        );
        let computed = vec![assign(binary(
            IrBinOp::Add,
            read(1, 4),
            constant(1, 4, false),
        ))];
        assert_eq!(
            redrive_action(&model, &driver(computed.clone())),
            Some(IrRedriveAction::Evaluate(computed))
        );
    }

    #[test]
    fn component_released_variables_are_variables_not_nets() {
        let mut aliased = packed("alias", 4, None);
        aliased.alias = Some(1);
        let model = model_with(vec![
            packed("net", 4, Some((0, 0))),
            packed("var", 4, None),
            aliased,
        ]);
        let mut released = Vec::new();
        let target = IrLhs::Stream {
            parts: vec![
                (IrLhs::Whole(0), 4),
                (IrLhs::Whole(1), 4),
                (IrLhs::Whole(2), 4),
            ],
            width: 12,
            slice: 1,
            direction: IrStreamDirection::LeftToRight,
        };
        released_variables(&model, &target, &mut released);
        assert_eq!(released, vec![1]);
    }

    #[test]
    fn component_continuous_writes_see_through_feedback_wrappers() {
        let assign = |signal: usize| IrStmt::Assign {
            lhs: IrLhs::Part(signal, 3, 0, false),
            rhs: constant(0, 4, false),
            nba: false,
        };
        let body = vec![IrStmt::While {
            cond: constant(1, 1, false),
            body: vec![IrStmt::If {
                cond: constant(1, 1, false),
                then_: vec![assign(3)],
                els: Some(vec![IrStmt::Block(vec![assign(5)])]),
                check: IrUniquePriorityCheck::None,
            }],
        }];
        let mut written = Vec::new();
        let mut arrays = Vec::new();
        for statement in &body {
            continuous_writes(statement, &mut written, &mut arrays);
        }
        assert_eq!(written, vec![3, 5]);
        assert!(arrays.is_empty());
    }
}
