//! Time-zero sampled values of nets with one continuous driver.
//!
//! A sampled value at time 0 is "the result of evaluating the expression
//! using the initial values of the variables comprising the expression"
//! (IEEE 1800-2009 16.9.3), and a net with drivers assumes their output
//! (6.7). A net whose only driver is one undelayed continuous assignment or
//! port connection therefore samples, at time 0, that driver's value over
//! the initial values of the variables it reads, through chains of such nets
//! (decision T0-D5). The net's storage still starts at X (T0-D3), so the
//! driver's first evaluation remains a time-zero change.
//!
//! Only drivers that fold to a constant at compile time qualify. Every other
//! net keeps the X start as its time-zero sample: nets with several driver
//! sites, delayed or path-destination nets, aliased nets, gate outputs,
//! drivers that call functions, read arrays or non-constant initial values,
//! and computed drivers whose value has z bits (a computed z is still an X
//! to z change, T0-D4). Variables keep their declaration or default initial
//! value, as 16.9.3 states.

use std::collections::{HashMap, HashSet};

use crate::sim::ir::{
    IrConst, IrExpr, IrExprKind, IrInitPhase, IrInitStep, IrInitTarget, IrLhs, IrModel, IrNetKind,
    IrProcessKind, IrStmt, IrType,
};

/// Bound on the nets one chain evaluation visits; a longer chain keeps the
/// X start. Chains this deep are not written by hand.
const MAX_CHAIN_DEPTH: usize = 256;

/// Append a [`IrInitStep::SampledStart`] for every qualifying net that some
/// step registers for sampling or that assertions may sample.
pub(super) fn assign_net_time0_samples(model: &mut IrModel, continuous: &HashSet<String>) {
    let samples_everything = !model.assertions.is_empty() || !model.sampled_domains.is_empty();
    let registered: HashSet<usize> = model
        .init_steps
        .iter()
        .filter_map(|step| match step {
            IrInitStep::RegisterSampled(sig)
            | IrInitStep::RegisterSampledValue(sig)
            | IrInitStep::RegisterSampledHistory { sig, .. } => Some(*sig),
            _ => None,
        })
        .collect();
    if !samples_everything && registered.is_empty() {
        return;
    }
    let mut context = Context::new(model, continuous);
    let mut steps = Vec::new();
    for (sig, signal) in model.signals.iter().enumerate() {
        let Some((group, _)) = signal.net_driver else {
            continue;
        };
        if signal.omit || !signal.net_alias.is_empty() {
            continue;
        }
        if !samples_everything && !registered.contains(&sig) {
            continue;
        }
        if let Some(value) = context.group_value(group, 0) {
            if context.emitted.insert(group) {
                steps.push(IrInitStep::SampledStart { sig, value });
            }
        }
    }
    model.init_steps.extend(steps);
}

/// The one continuous driver of a net group.
struct Driver {
    low: u32,
    width: u32,
    rhs: IrExpr,
}

struct Context<'a> {
    model: &'a IrModel,
    drivers: HashMap<usize, Option<Driver>>,
    initial: HashMap<usize, Option<IrConst>>,
    values: HashMap<usize, Option<IrConst>>,
    visiting: HashSet<usize>,
    emitted: HashSet<usize>,
}

impl<'a> Context<'a> {
    fn new(model: &'a IrModel, continuous: &HashSet<String>) -> Self {
        let mut drivers: HashMap<usize, Option<Driver>> = HashMap::new();
        let mut add = |group: usize, driver: Option<Driver>| {
            drivers
                .entry(group)
                .and_modify(|existing| *existing = None)
                .or_insert(driver);
        };
        for process in &model.processes {
            let assignment = match process.body() {
                [statement] => plain_assignment(statement),
                _ => None,
            };
            let qualifies =
                process.kind == IrProcessKind::Synthetic && continuous.contains(&process.c_name);
            match assignment {
                Some((lhs, rhs)) if qualifies => {
                    if let Some((group, low, width)) = lhs_range(model, lhs) {
                        add(
                            group,
                            Some(Driver {
                                low,
                                width,
                                rhs: rhs.clone(),
                            }),
                        );
                        continue;
                    }
                    for group in written_groups(model, process.body()) {
                        add(group, None);
                    }
                }
                _ => {
                    for group in written_groups(model, process.body()) {
                        add(group, None);
                    }
                }
            }
        }
        for step in &model.init_steps {
            if let IrInitStep::WriteNet { group, .. } = step {
                add(*group, None);
            }
        }
        let mut initial: HashMap<usize, Option<IrConst>> = HashMap::new();
        for step in &model.init_steps {
            match step {
                IrInitStep::SetScalar { sig, value } => {
                    initial.insert(*sig, Some(value.clone()));
                }
                IrInitStep::Initialize(init) => {
                    if let IrInitTarget::Signal(sig) = init.target() {
                        let value = (init.phase() == IrInitPhase::BeforeProcesses)
                            .then(|| fold_constant(init.value()))
                            .flatten();
                        initial.insert(*sig, value);
                    }
                }
                _ => {}
            }
        }
        Self {
            model,
            drivers,
            initial,
            values: HashMap::new(),
            visiting: HashSet::new(),
            emitted: HashSet::new(),
        }
    }

    /// The time-zero sampled value of `group`, when its driver determines one.
    fn group_value(&mut self, group: usize, depth: usize) -> Option<IrConst> {
        if let Some(value) = self.values.get(&group) {
            return value.clone();
        }
        if depth > MAX_CHAIN_DEPTH || !self.visiting.insert(group) {
            return None;
        }
        let value = self.evaluate_group(group, depth);
        self.visiting.remove(&group);
        self.values.insert(group, value.clone());
        value
    }

    fn evaluate_group(&mut self, group: usize, depth: usize) -> Option<IrConst> {
        let net = self.model.net_groups.get(group)?;
        if net.propagation_delay.is_some()
            || net.runtime_delay
            || net.starts_unknown
            || !matches!(net.kind, IrNetKind::Wire | IrNetKind::Wand | IrNetKind::Wor)
        {
            return None;
        }
        let width = net.width;
        let driver = self.drivers.get(&group)?.as_ref()?;
        let (low, driven, rhs) = (driver.low, driver.width, driver.rhs.clone());
        let constant_driver = is_constant(&rhs);
        let mut rhs = rhs;
        let mut failed = false;
        crate::sim::opt::walk_expr_mut(&mut rhs, &mut |expression: &mut IrExpr| {
            if failed {
                return;
            }
            if let IrExprKind::SigRead(sig) = expression.kind() {
                match self.signal_value(*sig, depth) {
                    Some(mut value) if value.width == expression.width() => {
                        value.signed = expression.signed();
                        expression.kind = IrExprKind::Const(value);
                    }
                    _ => failed = true,
                }
                return;
            }
            fold_selects(expression);
            crate::sim::opt::fold_constant_node(expression);
        });
        if failed {
            return None;
        }
        let IrExprKind::Const(value) = rhs.kind() else {
            return None;
        };
        if value.real.is_some() || value.width != driven || low + driven > width {
            return None;
        }
        let value = unfill(value);
        // A computed z is an X to z change at time 0 (T0-D4), so its bits
        // keep the X start; only a constant driver's z bits are z.
        if !constant_driver && value.z.iter().any(|mask| *mask != 0) {
            return None;
        }
        let mut result = IrConst::integral_default(width, false);
        for bit in 0..width {
            let (limb, shift) = ((bit / 64) as usize, bit % 64);
            result.x[limb] &= !(1 << shift);
            if bit < low || bit >= low + driven {
                result.z[limb] |= 1 << shift;
                continue;
            }
            let source = bit - low;
            let (slimb, sshift) = ((source / 64) as usize, source % 64);
            let get = |masks: &[u64]| masks.get(slimb).is_some_and(|m| m >> sshift & 1 == 1);
            if get(&value.bits) {
                result.bits[limb] |= 1 << shift;
            }
            if get(&value.x) {
                result.x[limb] |= 1 << shift;
            }
            if get(&value.z) {
                result.z[limb] |= 1 << shift;
            }
        }
        result.signed = net.signed;
        Some(result)
    }

    /// The initial value a read of `sig` evaluates to at time 0.
    fn signal_value(&mut self, sig: usize, depth: usize) -> Option<IrConst> {
        let signal = self.model.signals.get(sig)?;
        if !signal.net_alias.is_empty() || signal.alias.is_some() {
            return None;
        }
        if let Some((group, _)) = signal.net_driver {
            return self.group_value(group, depth + 1);
        }
        let IrType::Packed {
            width, two_state, ..
        } = signal.ty
        else {
            return None;
        };
        match self.initial.get(&sig) {
            Some(value) => value
                .clone()
                .filter(|value| value.real.is_none())
                .map(|v| unfill_to(&v, width)),
            None if signal.fixed_default.is_none() => {
                Some(IrConst::integral_default(width, two_state))
            }
            None => None,
        }
    }
}

fn plain_assignment(statement: &IrStmt) -> Option<(&IrLhs, &IrExpr)> {
    match statement {
        IrStmt::Located { statement, .. } => plain_assignment(statement),
        IrStmt::Assign {
            lhs,
            rhs,
            nba: false,
        } => Some((lhs, rhs)),
        _ => None,
    }
}

/// The group, low bit and width of a driver target that is a whole driver
/// slot or a constant part of one.
fn lhs_range(model: &IrModel, lhs: &IrLhs) -> Option<(usize, u32, u32)> {
    match lhs {
        IrLhs::Whole(sig) => {
            let signal = model.signals.get(*sig)?;
            let (group, _) = signal.net_driver?;
            signal.net_alias.is_empty().then_some(())?;
            Some((group, 0, signal.ty.width()))
        }
        IrLhs::Part(sig, left, right, _) => {
            let signal = model.signals.get(*sig)?;
            let (group, _) = signal.net_driver?;
            signal.net_alias.is_empty().then_some(())?;
            let (low, high) = if left < right {
                (*left, *right)
            } else {
                (*right, *left)
            };
            let low = u32::try_from(low).ok()?;
            let high = u32::try_from(high).ok()?;
            Some((group, low, high - low + 1))
        }
        _ => None,
    }
}

/// Every net group a statement list may write, through driver slots or
/// alias bindings. Continuous drivers lower to assignments, possibly inside
/// conditionals and blocks (as `net_delay_layout` walks them).
fn written_groups(model: &IrModel, statements: &[IrStmt]) -> Vec<usize> {
    let mut groups = Vec::new();
    for statement in statements {
        statement_groups(model, statement, &mut groups);
    }
    groups
}

fn statement_groups(model: &IrModel, statement: &IrStmt, groups: &mut Vec<usize>) {
    match statement {
        IrStmt::Located { statement, .. } => statement_groups(model, statement, groups),
        IrStmt::Assign { lhs, .. } | IrStmt::InertialAssign { lhs, .. } => {
            lhs_groups(model, lhs, groups)
        }
        IrStmt::Block(body)
        | IrStmt::While { body, .. }
        | IrStmt::Repeat { body, .. }
        | IrStmt::Forever { body }
        | IrStmt::WaitCond { body, .. }
        | IrStmt::WaitEventTriggered { body, .. }
        | IrStmt::ActivationScope { body, .. } => {
            groups.extend(written_groups(model, body));
        }
        IrStmt::For {
            init, incr, body, ..
        } => {
            for list in [init, incr, body] {
                groups.extend(written_groups(model, list));
            }
        }
        IrStmt::If { then_, els, .. } => {
            groups.extend(written_groups(model, then_));
            if let Some(els) = els {
                groups.extend(written_groups(model, els));
            }
        }
        IrStmt::Case { items, .. } => {
            for item in items {
                groups.extend(written_groups(model, &item.body));
            }
        }
        IrStmt::WaitOrder {
            success, failure, ..
        } => {
            groups.extend(written_groups(model, success));
            groups.extend(written_groups(model, failure));
        }
        _ => {}
    }
}

fn lhs_groups(model: &IrModel, lhs: &IrLhs, groups: &mut Vec<usize>) {
    let root = match lhs {
        IrLhs::Whole(sig)
        | IrLhs::Part(sig, ..)
        | IrLhs::Bit(sig, ..)
        | IrLhs::IdxPart(sig, ..) => Some(*sig),
        IrLhs::PackedSelect { target, .. } | IrLhs::TaggedSelect { target, .. } => {
            lhs_groups(model, target, groups);
            None
        }
        IrLhs::Stream { parts, .. } => {
            for (part, _) in parts {
                lhs_groups(model, part, groups);
            }
            None
        }
        IrLhs::WholeRef { .. } | IrLhs::Ref { .. } | IrLhs::ArrayElem { .. } => None,
    };
    if let Some(signal) = root.and_then(|sig| model.signals.get(sig)) {
        if let Some((group, _)) = signal.net_driver {
            groups.push(group);
        }
        groups.extend(signal.net_alias.iter().map(|binding| binding.group));
    }
}

fn is_constant(expression: &IrExpr) -> bool {
    match expression.kind() {
        IrExprKind::Const(value) => value.real.is_none(),
        IrExprKind::Concat { parts } => parts.iter().all(is_constant),
        _ => false,
    }
}

/// Fold the declaration initializer of a variable when it is constant.
fn fold_constant(value: &IrExpr) -> Option<IrConst> {
    let mut value = value.clone();
    crate::sim::opt::walk_expr_mut(&mut value, &mut |expression: &mut IrExpr| {
        fold_selects(expression);
        crate::sim::opt::fold_constant_node(expression);
    });
    match value.kind() {
        IrExprKind::Const(value) if value.width == 0 || value.real.is_none() => Some(value.clone()),
        _ => None,
    }
}

/// Constant bit and part selects of a constant, which the optimizer's
/// folding leaves to the runtime.
fn fold_selects(expression: &mut IrExpr) {
    let picked = match expression.kind() {
        IrExprKind::BitSel { base, idx } => match (base.kind(), idx.kind()) {
            (IrExprKind::Const(value), IrExprKind::Const(index))
                if value.fill.is_none()
                    && value.real.is_none()
                    && index.fill.is_none()
                    && index.x.iter().chain(&index.z).all(|mask| *mask == 0)
                    && index.bits.iter().skip(1).all(|limb| *limb == 0) =>
            {
                let bit = index.bits.first().copied().unwrap_or(0);
                u32::try_from(bit)
                    .ok()
                    .filter(|bit| *bit < value.width)
                    .map(|bit| slice(value, bit, 1))
            }
            _ => None,
        },
        IrExprKind::PartSel { base, left, right } => match base.kind() {
            IrExprKind::Const(value)
                if value.fill.is_none()
                    && value.real.is_none()
                    && *right >= 0
                    && left >= right
                    && *left < i64::from(value.width) =>
            {
                Some(slice(value, *right as u32, (*left - *right + 1) as u32))
            }
            _ => None,
        },
        _ => None,
    };
    if let Some(mut value) = picked {
        if value.width == expression.width() {
            value.signed = expression.signed();
            expression.kind = IrExprKind::Const(value);
        }
    }
}

fn slice(value: &IrConst, low: u32, width: u32) -> IrConst {
    let mut result = IrConst::integral_default(width, true);
    for bit in 0..width {
        let source = low + bit;
        let (slimb, sshift) = ((source / 64) as usize, source % 64);
        let (limb, shift) = ((bit / 64) as usize, bit % 64);
        for (from, to) in [
            (&value.bits, &mut result.bits),
            (&value.x, &mut result.x),
            (&value.z, &mut result.z),
        ] {
            if from.get(slimb).is_some_and(|mask| mask >> sshift & 1 == 1) {
                to[limb] |= 1 << shift;
            }
        }
    }
    result
}

/// A fill literal spelled out to its own width.
fn unfill(value: &IrConst) -> IrConst {
    unfill_to(value, value.width)
}

fn unfill_to(value: &IrConst, width: u32) -> IrConst {
    let Some(fill) = value.fill else {
        return value.clone();
    };
    let mut result = IrConst::integral_default(width, true);
    for bit in 0..width {
        let (limb, shift) = ((bit / 64) as usize, bit % 64);
        match fill {
            1 => result.bits[limb] |= 1 << shift,
            2 => result.x[limb] |= 1 << shift,
            3 => result.z[limb] |= 1 << shift,
            _ => {}
        }
    }
    result
}

#[cfg(test)]
mod tests;
