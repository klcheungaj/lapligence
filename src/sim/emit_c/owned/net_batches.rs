//! Ordered structural contributions over immutable electrical descriptors.
use super::*;

/// Amortize a descriptor table only for several consecutive contributions.
const NET_BATCH_MIN_ASSIGNMENTS: usize = 4;

pub(in crate::sim::emit_c) struct NetBatch {
    pub name: String,
    pub rows: Vec<NetRow>,
}

/// One process's file-scope tables: structural contribution batches and the
/// per-site runtime state its body addresses.
#[derive(Default)]
pub(in crate::sim::emit_c) struct ProcessTables {
    pub batches: Vec<NetBatch>,
    pub statics: Vec<SiteStatic>,
}

/// Mutable per-site state (an inertial driver handle or a sparse-array
/// handle key) of one process instance. It lives at file scope, named after
/// the owning process, rather than as a function-local static: the body then
/// addresses it like any other model storage, so instances whose bodies
/// differ only in this state share one body with the state as a record
/// operand. Its lifetime and zero initialization are those of the local
/// static it replaces.
pub(in crate::sim::emit_c) struct SiteStatic {
    pub name: String,
    /// Element type: `llg_inertial_t*` or `const char`.
    pub ty: &'static str,
    /// Element count of a per-element handle array.
    pub len: Option<u64>,
}

pub(in crate::sim::emit_c) struct NetRow {
    pub group: usize,
    pub slot: usize,
    pub left: i64,
    pub right: i64,
    pub cast_width: u32,
    pub cast_signed: bool,
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
            // Runtime-valued delays are evaluated per site, never batched.
            IrStmt::InertialAssign { lhs, rhs, delay } => (lhs, rhs, Some(delay.constant()?)),
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
                cast_width: target.ty.width(),
                cast_signed: target.ty.signed(),
            },
        })
    }

    /// Emit a statement sequence, folding runs of consecutive structural net
    /// contributions into one descriptor-table loop and runs of whole-cell
    /// stores into consecutive dense array cells into one range call.
    pub(super) fn statements(&mut self, mut statements: &[IrStmt]) -> Result<(), String> {
        while let Some(statement) = statements.first() {
            let mut consumed = self.net_batch(statements)?;
            if consumed == 0 {
                consumed = self.dense_cell_stores(statements)?;
            }
            if consumed == 0 {
                self.statement(statement)?;
                consumed = 1;
            }
            statements = &statements[consumed..];
        }
        Ok(())
    }

    pub(super) fn net_batch(&mut self, statements: &[IrStmt]) -> Result<usize, String> {
        if self.batch_owner.is_none() || self.sampled_reads {
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
            self.batch_owner.as_ref().expect("process owner"),
            self.net_batches.batches.len()
        );
        let count = rows.len();
        let source = self.expression(&base)?;
        self.line("{");
        let handles = if delay.is_some() {
            Some(self.site_static("llg_inertial_t*", "net_inertial", Some(count as u64)))
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
            format!(
                "sv4_cast({}, {row}.cast_width, {row}.cast_signed)",
                selected.code
            ),
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
        self.net_batches.batches.push(NetBatch { name, rows });
        Ok(count)
    }

    /// Declare zero-initialized per-site state and return its C name. A
    /// coroutine process hoists it to file scope (see [`SiteStatic`]); any
    /// other body keeps a function-local static.
    pub(super) fn site_static(
        &mut self,
        ty: &'static str,
        purpose: &str,
        len: Option<u64>,
    ) -> String {
        let local = self.name(purpose);
        if let Some(owner) = &self.batch_owner {
            let name = format!("{owner}{local}");
            self.net_batches.statics.push(SiteStatic {
                name: name.clone(),
                ty,
                len,
            });
            return name;
        }
        self.line(match len {
            Some(len) => format!("static {ty} {local}[{len}] = {{0}};"),
            None if ty.ends_with('*') => format!("static {ty} {local} = NULL;"),
            None => format!("static {ty} {local} = 0;"),
        });
        local
    }
}
