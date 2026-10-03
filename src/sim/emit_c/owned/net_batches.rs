//! Ordered structural contributions over immutable electrical descriptors.
use super::*;

/// Amortize a descriptor table only for several consecutive contributions.
const NET_BATCH_MIN_ASSIGNMENTS: usize = 4;

pub(in crate::sim::emit_c) struct NetBatch {
    pub name: String,
    pub rows: Vec<NetRow>,
}

pub(in crate::sim::emit_c) struct NetRow {
    pub group: usize,
    pub slot: usize,
    pub left: i64,
    pub right: i64,
    pub width: u32,
    pub signed: bool,
}

struct Contribution<'a> {
    base: &'a IrExpr,
    delay: Option<IrTransitionDelay>,
    row: NetRow,
}

impl Frame<'_, '_> {
    fn net_contribution<'a>(&self, statement: &'a IrStmt) -> Option<Contribution<'a>> {
        let (lhs, rhs, delay) = match statement {
            IrStmt::Assign {
                lhs,
                rhs,
                nba: false,
            } => (lhs, rhs, None),
            IrStmt::InertialAssign { lhs, rhs, delay } => (lhs, rhs, Some(*delay)),
            _ => return None,
        };
        let IrLhs::Whole(signal) = lhs else {
            return None;
        };
        let target = self.ctx.model.signal(*signal);
        let (group, slot) = target.net_driver?;
        let IrExprKind::PartSel { base, left, right } = &rhs.kind else {
            return None;
        };
        fn captured(expression: &IrExpr) -> bool {
            match &expression.kind {
                IrExprKind::LocalRead(_) => true,
                IrExprKind::Convert { a } | IrExprKind::Resize { a } => captured(a),
                _ => false,
            }
        }
        if !captured(base) || *left < *right || target.ty.two_state() {
            return None;
        }
        Some(Contribution {
            base,
            delay,
            row: NetRow {
                group,
                slot,
                left: *left,
                right: *right,
                width: target.ty.width(),
                signed: target.ty.signed(),
            },
        })
    }

    pub(super) fn net_batch(&mut self, statements: &[IrStmt]) -> Result<usize, String> {
        if self.pca_owner.is_none() || self.sampled_reads {
            return Ok(0);
        }
        let Some(first) = statements
            .first()
            .and_then(|stmt| self.net_contribution(stmt))
        else {
            return Ok(0);
        };
        let base = first.base.clone();
        let delay = first.delay;
        let mut rows = vec![first.row];
        for statement in &statements[1..] {
            let Some(next) = self.net_contribution(statement) else {
                break;
            };
            if next.base != &base || next.delay != delay {
                break;
            }
            rows.push(next.row);
        }
        if rows.len() < NET_BATCH_MIN_ASSIGNMENTS {
            return Ok(0);
        }
        let name = format!(
            "llg_net_rows_{}_{}",
            self.pca_owner.as_ref().expect("process owner"),
            self.net_batches.len()
        );
        let count = rows.len();
        let source = self.expression(&base)?;
        self.line("{");
        let handles = if delay.is_some() {
            let handles = self.name("net_inertial");
            self.line(format!(
                "static llg_inertial_t* {handles}[{count}] = {{0}};"
            ));
            Some(handles)
        } else {
            None
        };
        let index = self.declare("size_t", "net_i", "0".to_owned());
        self.line(format!("for (; {index} < {count}; ++{index}) {{"));
        let row = format!("{name}[{index}]");
        let selected = self.value(
            format!("sv4_part_select({}, {row}.left, {row}.right)", source.code),
            1,
            false,
        );
        let converted = self.value(
            format!("sv4_cast({}, {row}.width, {row}.is_signed)", selected.code),
            1,
            false,
        );
        if let Some(delay) = delay {
            self.line(format!(
                "llg_inertial_net(&{}[{index}], {row}.net, {row}.slot, {}, {}ULL, {}ULL, {}ULL);",
                handles.expect("delayed batch handles"),
                converted.code,
                delay.rise,
                delay.fall,
                delay.turn_off
            ));
        } else {
            self.line(format!(
                "llg_net_write({row}.net, {row}.slot, {});",
                converted.code
            ));
        }
        self.discard(converted);
        self.discard(selected);
        self.line("}");
        self.line("}");
        self.discard(source);
        self.net_batches.push(NetBatch { name, rows });
        Ok(count)
    }
}
