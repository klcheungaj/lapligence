//! Net-delay layout of delayed and module-path destination net groups.
//!
//! Runs once every driver process and initialization step exists. Each
//! driver site is one statement writing one or more group bits (a
//! concatenation target is one site). A delayed or path-destination group
//! with any driver starts at X (decision S2-D10); an undelayed wire, wand or
//! wor group starts at X on the bits its sites write (decision T0-D3). A
//! delayed group whose
//! sites cover different bit ranges schedules each separately driven part
//! as its own vector (decision S2-D3): every edge of a site's covered run
//! starts a part. A site whose bits cannot be resolved statically makes
//! every bit its own part.

use std::collections::{BTreeSet, HashMap, HashSet};

use crate::sim::ir::{
    IrConst, IrExpr, IrExprKind, IrInitStep, IrLhs, IrModel, IrNetAliasBinding, IrNetKind, IrStmt,
};

#[derive(Default)]
struct GroupDrives {
    driven: bool,
    starts: BTreeSet<u32>,
    per_bit: bool,
    /// Bits written by the group's sites, as sorted inclusive ranges.
    runs: Vec<(u32, u32)>,
}

/// Bits one site writes, per group: resolved bit positions, or unknown.
#[derive(Default)]
struct SiteBits {
    bits: HashMap<usize, Vec<(u32, u32)>>,
    unknown: HashSet<usize>,
}

impl SiteBits {
    fn range(&mut self, group: usize, low: u32, high: u32) {
        self.bits.entry(group).or_default().push((low, high));
    }
}

pub(super) fn assign_net_delay_layouts(model: &mut IrModel) {
    let path_destinations = model
        .module_paths()
        .iter()
        .flat_map(|set| set.destinations())
        .map(|destination| destination.group())
        .collect::<HashSet<_>>();
    let relevant = model
        .net_groups
        .iter()
        .enumerate()
        .map(|(index, group)| {
            group.propagation_delay.is_some() || path_destinations.contains(&index)
        })
        .collect::<Vec<_>>();
    if model.net_groups.is_empty() {
        return;
    }
    let mut drives = HashMap::<usize, GroupDrives>::new();
    for process in &model.processes {
        visit_statements(model, process.body(), &mut drives);
    }
    for step in &model.init_steps {
        if let IrInitStep::WriteNet { group, value, .. } = step {
            let mut site = SiteBits::default();
            for (low, high) in driven_runs(value) {
                site.range(*group, low, high);
            }
            record_site(model, site, &mut drives);
        }
    }
    for (index, group) in model.net_groups.iter_mut().enumerate() {
        let Some(drive) = drives.get_mut(&index) else {
            continue;
        };
        if !relevant[index] {
            if drive.driven
                && matches!(
                    group.kind,
                    IrNetKind::Wire | IrNetKind::Wand | IrNetKind::Wor
                )
            {
                group.driven_runs = if drive.per_bit {
                    vec![(0, group.width)]
                } else {
                    merged_runs(&mut drive.runs)
                };
            }
            continue;
        }
        group.starts_unknown = drive.driven;
        if group.propagation_delay.is_none() || group.width < 2 {
            continue;
        }
        group.propagation_segments = if drive.per_bit {
            (1..group.width).collect()
        } else {
            drive
                .starts
                .iter()
                .copied()
                .filter(|start| *start > 0 && *start < group.width)
                .collect()
        };
    }
}

fn visit_statements(
    model: &IrModel,
    statements: &[IrStmt],
    drives: &mut HashMap<usize, GroupDrives>,
) {
    for statement in statements {
        visit_statement(model, statement, drives);
    }
}

// Continuous drivers lower to assignments, possibly inside conditionals and
// blocks; procedural code cannot drive a net, so other statement kinds hold
// no driver site.
fn visit_statement(model: &IrModel, statement: &IrStmt, drives: &mut HashMap<usize, GroupDrives>) {
    match statement {
        IrStmt::Located { statement, .. } => visit_statement(model, statement, drives),
        IrStmt::Assign { lhs, .. } | IrStmt::InertialAssign { lhs, .. } => {
            let mut site = SiteBits::default();
            lhs_bits(model, lhs, &mut site);
            record_site(model, site, drives);
        }
        IrStmt::Block(body)
        | IrStmt::While { body, .. }
        | IrStmt::Repeat { body, .. }
        | IrStmt::Forever { body }
        | IrStmt::WaitCond { body, .. }
        | IrStmt::WaitEventTriggered { body, .. }
        | IrStmt::ActivationScope { body, .. } => visit_statements(model, body, drives),
        IrStmt::For {
            init, incr, body, ..
        } => {
            visit_statements(model, init, drives);
            visit_statements(model, incr, drives);
            visit_statements(model, body, drives);
        }
        IrStmt::If { then_, els, .. } => {
            visit_statements(model, then_, drives);
            if let Some(els) = els {
                visit_statements(model, els, drives);
            }
        }
        IrStmt::Case { items, .. } => {
            for item in items {
                visit_statements(model, &item.body, drives);
            }
        }
        IrStmt::WaitOrder {
            success, failure, ..
        } => {
            visit_statements(model, success, drives);
            visit_statements(model, failure, drives);
        }
        _ => {}
    }
}

/// Merge sorted-or-not inclusive bit ranges into `(low, count)` runs.
fn merged_runs(ranges: &mut [(u32, u32)]) -> Vec<(u32, u32)> {
    ranges.sort_unstable();
    let mut runs: Vec<(u32, u32)> = Vec::new();
    for &(low, high) in ranges.iter() {
        match runs.last_mut() {
            Some((first, count)) if low <= *first + *count => {
                *count = (*count).max(high + 1 - *first);
            }
            _ => runs.push((low, high + 1 - low)),
        }
    }
    runs
}

fn record_site(model: &IrModel, site: SiteBits, drives: &mut HashMap<usize, GroupDrives>) {
    for group in site.unknown {
        let drive = drives.entry(group).or_default();
        drive.driven = true;
        drive.per_bit = true;
    }
    for (group, mut ranges) in site.bits {
        let Some(width) = model.net_groups.get(group).map(|group| group.width) else {
            continue;
        };
        let drive = drives.entry(group).or_default();
        drive.driven = true;
        ranges.sort_unstable();
        let mut run: Option<(u32, u32)> = None;
        for (low, high) in ranges {
            let high = high.min(width.saturating_sub(1));
            if low > high {
                continue;
            }
            drive.runs.push((low, high));
            run = match run {
                Some((first, last)) if low <= last.saturating_add(1) => {
                    Some((first, last.max(high)))
                }
                Some((first, last)) => {
                    drive.starts.insert(first);
                    drive.starts.insert(last + 1);
                    Some((low, high))
                }
                None => Some((low, high)),
            };
        }
        if let Some((first, last)) = run {
            drive.starts.insert(first);
            drive.starts.insert(last + 1);
        }
    }
}

/// The group bits a driver target writes. A select whose position is not a
/// constant leaves its groups unknown.
fn lhs_bits(model: &IrModel, lhs: &IrLhs, site: &mut SiteBits) {
    match lhs {
        IrLhs::Whole(signal) => {
            let width = model
                .signals
                .get(*signal)
                .map_or(0, |signal| signal.ty.width());
            if width > 0 {
                signal_bits(model, *signal, 0, width - 1, site);
            }
        }
        IrLhs::Part(signal, left, right, _) => {
            let (low, high) = if left < right {
                (*left, *right)
            } else {
                (*right, *left)
            };
            match (u32::try_from(low.max(0)), u32::try_from(high)) {
                (Ok(low), Ok(high)) if high >= low => signal_bits(model, *signal, low, high, site),
                _ => {}
            }
        }
        IrLhs::Bit(signal, index, _) => match constant_u32(index) {
            Some(bit) => signal_bits(model, *signal, bit, bit, site),
            None => unknown_bits(model, *signal, site),
        },
        IrLhs::IdxPart(signal, base, _, width, negative, _) => {
            let low = constant_u32(base).and_then(|base| {
                if *negative {
                    base.checked_sub(width.saturating_sub(1))
                } else {
                    Some(base)
                }
            });
            match low.and_then(|low| {
                low.checked_add(width.saturating_sub(1))
                    .map(|high| (low, high))
            }) {
                Some((low, high)) if *width > 0 => signal_bits(model, *signal, low, high, site),
                _ => unknown_bits(model, *signal, site),
            }
        }
        IrLhs::Stream { parts, .. } => {
            for (part, _) in parts {
                lhs_bits(model, part, site);
            }
        }
        IrLhs::PackedSelect { target, .. } | IrLhs::TaggedSelect { target, .. } => {
            if let Some(signal) = root_signal(target) {
                unknown_bits(model, signal, site);
            }
        }
        IrLhs::WholeRef { .. } | IrLhs::Ref { .. } | IrLhs::ArrayElem { .. } => {}
    }
}

fn root_signal(lhs: &IrLhs) -> Option<usize> {
    match lhs {
        IrLhs::Whole(signal)
        | IrLhs::Part(signal, ..)
        | IrLhs::Bit(signal, ..)
        | IrLhs::IdxPart(signal, ..) => Some(*signal),
        IrLhs::PackedSelect { target, .. } | IrLhs::TaggedSelect { target, .. } => {
            root_signal(target)
        }
        _ => None,
    }
}

/// Map signal bits `low..=high` to the groups they drive: a driver slot
/// signal spans its group; an aliased view reaches its bound group bits.
fn signal_bits(model: &IrModel, signal: usize, low: u32, high: u32, site: &mut SiteBits) {
    let Some(info) = model.signals.get(signal) else {
        return;
    };
    if let Some((group, _)) = info.net_driver {
        site.range(group, low, high);
    }
    for binding in &info.net_alias {
        let IrNetAliasBinding {
            group,
            signal_bit,
            group_bit,
            ..
        } = *binding;
        if (low..=high).contains(&signal_bit) {
            site.range(group, group_bit, group_bit);
        }
    }
}

fn unknown_bits(model: &IrModel, signal: usize, site: &mut SiteBits) {
    let Some(info) = model.signals.get(signal) else {
        return;
    };
    if let Some((group, _)) = info.net_driver {
        site.unknown.insert(group);
    }
    for binding in &info.net_alias {
        site.unknown.insert(binding.group);
    }
}

fn constant_u32(expression: &IrExpr) -> Option<u32> {
    match expression.kind() {
        IrExprKind::Const(value) => {
            if value.real_value().is_some()
                || value.fill().is_some()
                || value.x_mask().iter().any(|mask| *mask != 0)
                || value.z_mask().iter().any(|mask| *mask != 0)
                || value.bits().iter().skip(1).any(|limb| *limb != 0)
            {
                return None;
            }
            u32::try_from(value.bits().first().copied().unwrap_or(0)).ok()
        }
        IrExprKind::Convert { a } | IrExprKind::Resize { a } => constant_u32(a),
        _ => None,
    }
}

/// Maximal runs of non-Z bits of an initialization value.
fn driven_runs(value: &IrConst) -> Vec<(u32, u32)> {
    let width = value.width();
    if width == 0 {
        return Vec::new();
    }
    if let Some(fill) = value.fill() {
        return if fill == 3 {
            Vec::new()
        } else {
            vec![(0, width - 1)]
        };
    }
    let mut runs = Vec::new();
    let mut start = None;
    for bit in 0..width {
        let limb = (bit / 64) as usize;
        let z = value
            .z_mask()
            .get(limb)
            .is_some_and(|mask| mask >> (bit % 64) & 1 != 0);
        match (z, start) {
            (false, None) => start = Some(bit),
            (true, Some(first)) => {
                runs.push((first, bit - 1));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(first) = start {
        runs.push((first, width - 1));
    }
    runs
}

#[cfg(test)]
mod tests;
