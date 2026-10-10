//! Intra-assignment delays on records with string or chandle leaves.
//!
//! Such records have no single packed destination, so the untimed
//! assignment is lowered leaf by leaf first and its writes are then
//! retimed. Every RHS leaf is still evaluated once, at issue.

use super::*;

impl EmitCtx<'_, '_> {
    /// Lower `record = #d value` or `record <= #d value` for a native record
    /// target. The nonblocking form queues every leaf with the same delay;
    /// the blocking form captures every leaf before suspending and performs
    /// the writes when the delay completes.
    pub(super) fn lower_delayed_native_record(
        &mut self,
        h: NodeId,
        lhs: NodeId,
        rhs: NodeId,
        blocking: bool,
        ticks: IrDelay,
    ) -> Result<Vec<IrStmt>, String> {
        let op = match self.cg.kind(h) {
            NodeKind::Stmt(StmtKind::Assign { op, .. }) => *op,
            _ => unreachable!("non-assignment passed to lower_delayed_native_record"),
        };
        if blocking {
            return self.lower_blocking_native_record(
                h,
                lhs,
                rhs,
                op,
                vec![IrStmt::Delay { ticks }],
            );
        }
        let statement = self.lower_assignment_operands(lhs, rhs, false, op, false)?;
        let mut before = Vec::new();
        // One runtime delay value serves every queued leaf.
        let ticks = capture_delay(ticks, &format!("_nd{}", h.0), &mut before);
        before.push(retime_nonblocking(statement, &ticks, &self.path)?);
        Ok(vec![IrStmt::Block(before)])
    }

    /// Blocking timed record assignment: every leaf value is captured before
    /// `wait` (a delay or an event control) and written after it.
    pub(super) fn lower_blocking_native_record(
        &mut self,
        h: NodeId,
        lhs: NodeId,
        rhs: NodeId,
        op: Operation,
        wait: Vec<IrStmt>,
    ) -> Result<Vec<IrStmt>, String> {
        let statement = self.lower_assignment_operands(lhs, rhs, true, op, false)?;
        let mut before = Vec::new();
        let mut after = Vec::new();
        let mut sequence = 0usize;
        split_blocking_writes(
            statement,
            &format!("_nw{}", h.0),
            &mut sequence,
            &mut before,
            &mut after,
            &self.path,
        )?;
        self.saw_wait = true;
        before.extend(wait);
        before.extend(after);
        Ok(vec![IrStmt::Block(before)])
    }
}

/// Evaluate a runtime delay once into a local so several queued leaves
/// share it.
fn capture_delay(ticks: IrDelay, name: &str, before: &mut Vec<IrStmt>) -> IrDelay {
    match ticks {
        IrDelay::Runtime {
            value,
            unit_ticks,
            precision_ticks,
        } => {
            let (width, signed) = (value.width, value.signed);
            before.push(IrStmt::DeclLocal {
                name: name.to_owned(),
                width,
                signed,
                two_state: false,
                init: Some(value),
            });
            IrDelay::Runtime {
                value: Box::new(IrExpr::new(
                    IrExprKind::LocalRead(name.to_owned()),
                    width,
                    signed,
                    None,
                )),
                unit_ticks,
                precision_ticks,
            }
        }
        constant => constant,
    }
}

/// Give every queued leaf write of an untimed record NBA the delay `ticks`.
fn retime_nonblocking(statement: IrStmt, ticks: &IrDelay, path: &str) -> Result<IrStmt, String> {
    Ok(match statement {
        IrStmt::Block(statements) => IrStmt::Block(
            statements
                .into_iter()
                .map(|statement| retime_nonblocking(statement, ticks, path))
                .collect::<Result<_, _>>()?,
        ),
        IrStmt::Located { origin, statement } => IrStmt::Located {
            origin,
            statement: Box::new(retime_nonblocking(*statement, ticks, path)?),
        },
        IrStmt::Assign {
            lhs,
            rhs,
            nba: true,
        } => IrStmt::DelayedAssign {
            lhs,
            rhs,
            ticks: ticks.clone(),
        },
        IrStmt::DelayedStringAssign { target, rhs, .. } => IrStmt::DelayedStringAssign {
            target,
            rhs,
            ticks: ticks.clone(),
        },
        IrStmt::DelayedChandleAssign { target, rhs, .. } => IrStmt::DelayedChandleAssign {
            target,
            rhs,
            ticks: ticks.clone(),
        },
        // Captures and the calls that produce native sources run at issue.
        statement @ (IrStmt::DeclLocal { .. }
        | IrStmt::DeclString { .. }
        | IrStmt::NativeValueDeclare(_)
        | IrStmt::Call(_)
        | IrStmt::Nop) => statement,
        IrStmt::Object(operation)
            if matches!(*operation, IrObjectStmt::ChandleDeclareLocal(..)) =>
        {
            IrStmt::Object(operation)
        }
        _ => {
            return Err(format!(
                "delayed nonblocking assignment of this native record shape in `{path}` is not supported"
            ))
        }
    })
}

/// Move every write of a lowered blocking record assignment behind the
/// delay. Each written value is captured into a fresh local in its original
/// position, so evaluation order and values are those at issue.
fn split_blocking_writes(
    statement: IrStmt,
    prefix: &str,
    sequence: &mut usize,
    before: &mut Vec<IrStmt>,
    after: &mut Vec<IrStmt>,
    path: &str,
) -> Result<(), String> {
    let mut fresh = || {
        *sequence += 1;
        format!("{prefix}_{sequence}")
    };
    match statement {
        IrStmt::Block(statements) => {
            for statement in statements {
                split_blocking_writes(statement, prefix, sequence, before, after, path)?;
            }
        }
        IrStmt::Located { statement, .. } => {
            split_blocking_writes(*statement, prefix, sequence, before, after, path)?
        }
        IrStmt::Assign {
            lhs,
            rhs,
            nba: false,
        } => {
            let name = fresh();
            let (width, signed) = (rhs.width, rhs.signed);
            before.push(IrStmt::DeclLocal {
                name: name.clone(),
                width,
                signed,
                two_state: false,
                init: Some(Box::new(rhs)),
            });
            after.push(IrStmt::Assign {
                lhs,
                rhs: IrExpr::new(IrExprKind::LocalRead(name), width, signed, None),
                nba: false,
            });
        }
        IrStmt::Object(operation) => match *operation {
            IrObjectStmt::StringAssign(index, value) => {
                let name = fresh();
                before.push(IrStmt::DeclString {
                    name: name.clone(),
                    init: Some(value),
                });
                after.push(IrStmt::Object(Box::new(IrObjectStmt::StringAssign(
                    index,
                    IrStringExpr::LocalRead(name),
                ))));
            }
            IrObjectStmt::StringAssignLocal(target, value) => {
                let name = fresh();
                before.push(IrStmt::DeclString {
                    name: name.clone(),
                    init: Some(value),
                });
                after.push(IrStmt::Object(Box::new(IrObjectStmt::StringAssignLocal(
                    target,
                    IrStringExpr::LocalRead(name),
                ))));
            }
            IrObjectStmt::ChandleAssign(index, value) => {
                let name = fresh();
                before.push(IrStmt::Object(Box::new(IrObjectStmt::ChandleDeclareLocal(
                    name.clone(),
                    Some(value),
                ))));
                after.push(IrStmt::Object(Box::new(IrObjectStmt::ChandleAssign(
                    index,
                    IrChandleExpr::LocalRead(name),
                ))));
            }
            IrObjectStmt::ChandleAssignLocal(target, value) => {
                let name = fresh();
                before.push(IrStmt::Object(Box::new(IrObjectStmt::ChandleDeclareLocal(
                    name.clone(),
                    Some(value),
                ))));
                after.push(IrStmt::Object(Box::new(IrObjectStmt::ChandleAssignLocal(
                    target,
                    IrChandleExpr::LocalRead(name),
                ))));
            }
            operation @ IrObjectStmt::ChandleDeclareLocal(..) => {
                before.push(IrStmt::Object(Box::new(operation)))
            }
            _ => {
                return Err(format!(
                    "blocking delayed assignment of this native record shape in `{path}` is not supported"
                ))
            }
        },
        statement @ (IrStmt::DeclLocal { .. }
        | IrStmt::DeclString { .. }
        | IrStmt::NativeValueDeclare(_)
        | IrStmt::Call(_)
        | IrStmt::Nop) => before.push(statement),
        _ => {
            return Err(format!(
                "blocking delayed assignment of this native record shape in `{path}` is not supported"
            ))
        }
    }
    Ok(())
}
