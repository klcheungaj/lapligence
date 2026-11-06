//! Intra-assignment delays on records with string or chandle leaves.
//!
//! Such records have no single packed destination, so the untimed
//! assignment is lowered leaf by leaf first and its writes are then
//! retimed. Every RHS leaf is still evaluated once, at issue.

use super::*;
use std::collections::HashSet;

impl EmitCtx<'_, '_> {
    /// Whether a nonblocking event-controlled target lies inside a native
    /// value or a record element, or is a whole native record: a queued write
    /// there is a located one, or one write per leaf, rather than one store.
    pub(super) fn located_event_nba(&self, lhs: NodeId) -> bool {
        let member = self.cg.p30_unwrap_cast(lhs);
        self.cg.names_element_member(member)
            || self.cg.is_container_record(member)
            || matches!(
                self.cg.native_member_select(&self.path, member),
                Ok(Some(_))
            )
            || matches!(
                self.cg.native_record_select(&self.path, member),
                Ok(Some(_))
            )
            || self.cg.persistent_native_target(member)
            || (self.cg.native_record_target(member) && !self.native_nba_target(lhs))
    }

    /// `target <= @(e) value` over a located target. The untimed write is
    /// lowered with its selectors as locals and split: everything a queued
    /// write needs is evaluated at issue, in selector, value, repeat count
    /// order, and copied into the frame of a detached process that waits for
    /// the control and then issues the same writes (SV 9.4.5, 10.4.2).
    pub(super) fn lower_located_event_nba(
        &mut self,
        h: NodeId,
        lhs: NodeId,
        rhs: NodeId,
        specs: &[EventSpec],
        count: Option<IrExpr>,
    ) -> Result<Vec<IrStmt>, String> {
        self.cg.native_select_at_issue = true;
        let statement =
            self.lower_assignment_operands(lhs, rhs, false, Operation::Assignment, false);
        self.cg.native_select_at_issue = false;
        let frame = self.cg.new_frame_id()?;
        let mut split = EventNbaSplit {
            ctx: self,
            frame,
            prefix: format!("_ne{}", h.0),
            sequence: 0,
            before: Vec::new(),
            captures: Vec::new(),
            captured: HashSet::new(),
        };
        let mut action = Vec::new();
        split.statement(statement?, &mut action)?;
        let EventNbaSplit {
            mut before,
            captures,
            ..
        } = split;
        before.push(self.spawn_process_evaluated_action(
            h,
            specs,
            count,
            frame,
            captures,
            action,
            "event_native_nba",
        )?);
        Ok(vec![IrStmt::Block(before)])
    }

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

/// Whether `statement` queues a nonblocking write.
fn queues_write(statement: &IrStmt) -> bool {
    match statement.unlocated() {
        IrStmt::Block(statements) => statements.iter().any(queues_write),
        IrStmt::If { then_, els, .. } => {
            then_.iter().any(queues_write)
                || els
                    .as_ref()
                    .is_some_and(|statements| statements.iter().any(queues_write))
        }
        IrStmt::Assign { nba, .. } => *nba,
        IrStmt::DelayedAssign { .. }
        | IrStmt::DelayedStringAssign { .. }
        | IrStmt::DelayedChandleAssign { .. } => true,
        IrStmt::Container(operation) => matches!(
            **operation,
            IrContainerStmt::NbaLocate { .. } | IrContainerStmt::NbaLocateEnd
        ),
        _ => false,
    }
}

/// Splits a lowered untimed nonblocking write into what runs at issue and
/// what the detached process runs after the event control.
struct EventNbaSplit<'e, 'c, 'a> {
    ctx: &'e EmitCtx<'c, 'a>,
    frame: FrameId,
    prefix: String,
    sequence: usize,
    before: Vec<IrStmt>,
    captures: Vec<IrCapture>,
    /// Locals with a copy in the process frame. Any other local read names
    /// issue-time storage, such as a leaf of a temporary.
    captured: HashSet<String>,
}

impl EventNbaSplit<'_, '_, '_> {
    fn fresh(&mut self) -> String {
        self.sequence += 1;
        format!("{}_{}", self.prefix, self.sequence)
    }

    fn storage(&self, kind: StorageKind) -> StorageRef {
        StorageRef::new(
            self.frame,
            self.captures.len() as u32,
            StorageLifetime::Automatic,
            StorageOwnership::Owned,
        )
        .with_kind(kind)
    }

    /// Declare the packed or real local `name` at issue and name its copy in
    /// the process frame the same way.
    fn capture_value(&mut self, name: &str, width: u32, signed: bool) {
        self.captured.insert(name.to_owned());
        let kind = if width == 0 {
            StorageKind::Real
        } else {
            StorageKind::Packed
        };
        let capture = IrCapture::new(
            self.storage(kind),
            IrExpr::new(IrExprKind::LocalRead(name.to_owned()), width, signed, None),
        )
        .with_alias(Some(name.to_owned()));
        self.captures.push(capture);
    }

    fn capture_string(&mut self, name: &str) {
        self.captured.insert(name.to_owned());
        let capture = IrCapture::new(
            self.storage(StorageKind::String),
            IrExpr::new(IrExprKind::LocalRead(name.to_owned()), 0, false, None),
        );
        self.captures.push(capture);
    }

    fn capture_handle(&mut self, name: &str) {
        self.captured.insert(name.to_owned());
        let capture = IrCapture::new(
            self.storage(StorageKind::Opaque),
            IrExpr::new(
                IrExprKind::ObjectQuery(Box::new(IrObjectQuery::HandleCapture(
                    IrChandleExpr::LocalRead(name.to_owned()),
                ))),
                1,
                false,
                None,
            ),
        );
        self.captures.push(capture);
    }

    /// `value` as a read of a local evaluated at issue.
    fn issue_value(&mut self, value: IrExpr) -> IrExpr {
        if matches!(value.kind(), IrExprKind::LocalRead(name) if self.captured.contains(name)) {
            return value;
        }
        let name = self.fresh();
        let (width, signed) = (value.width, value.signed);
        self.before.push(IrStmt::DeclLocal {
            name: name.clone(),
            width,
            signed,
            two_state: false,
            init: Some(Box::new(value)),
        });
        self.capture_value(&name, width, signed);
        IrExpr::new(IrExprKind::LocalRead(name), width, signed, None)
    }

    fn issue_lhs(&mut self, lhs: IrLhs) -> Result<IrLhs, String> {
        self.ctx
            .capture_event_assignment_lhs(self.frame, &mut self.captures, lhs)
    }

    fn statement(&mut self, statement: IrStmt, action: &mut Vec<IrStmt>) -> Result<(), String> {
        match statement {
            IrStmt::Block(statements) => {
                for statement in statements {
                    self.statement(statement, action)?;
                }
            }
            IrStmt::Located { statement, .. } => self.statement(*statement, action)?,
            IrStmt::If {
                cond,
                then_,
                els,
                check,
            } if then_.iter().any(queues_write)
                || els
                    .as_ref()
                    .is_some_and(|statements| statements.iter().any(queues_write)) =>
            {
                let mut then_action = Vec::new();
                for statement in then_ {
                    self.statement(statement, &mut then_action)?;
                }
                let mut else_action = Vec::new();
                for statement in els.into_iter().flatten() {
                    self.statement(statement, &mut else_action)?;
                }
                action.push(IrStmt::If {
                    cond,
                    then_: then_action,
                    els: (!else_action.is_empty()).then_some(else_action),
                    check,
                });
            }
            IrStmt::Assign {
                lhs,
                rhs,
                nba: true,
            }
            | IrStmt::DelayedAssign { lhs, rhs, .. } => {
                let lhs = self.issue_lhs(lhs)?;
                let rhs = self.issue_value(rhs);
                action.push(IrStmt::Assign {
                    lhs,
                    rhs,
                    nba: true,
                });
            }
            IrStmt::DelayedStringAssign { target, rhs, ticks } => {
                let rhs = if matches!(&rhs, IrStringExpr::LocalRead(name) if self.captured.contains(name))
                {
                    rhs
                } else {
                    let name = self.fresh();
                    self.before.push(IrStmt::DeclString {
                        name: name.clone(),
                        init: Some(rhs),
                    });
                    self.capture_string(&name);
                    IrStringExpr::LocalRead(name)
                };
                action.push(IrStmt::DelayedStringAssign { target, rhs, ticks });
            }
            IrStmt::DelayedChandleAssign { target, rhs, ticks } => {
                let rhs = if matches!(&rhs, IrChandleExpr::LocalRead(name) if self.captured.contains(name))
                {
                    rhs
                } else {
                    let name = self.fresh();
                    self.before
                        .push(IrStmt::Object(Box::new(IrObjectStmt::ChandleDeclareLocal(
                            name.clone(),
                            Some(rhs),
                        ))));
                    self.capture_handle(&name);
                    IrChandleExpr::LocalRead(name)
                };
                action.push(IrStmt::DelayedChandleAssign { target, rhs, ticks });
            }
            IrStmt::Container(operation)
                if matches!(
                    *operation,
                    IrContainerStmt::NbaLocate { .. } | IrContainerStmt::NbaLocateEnd
                ) =>
            {
                action.push(IrStmt::Container(operation));
            }
            IrStmt::Container(operation)
                if matches!(*operation, IrContainerStmt::ContainerNonblocking { .. }) =>
            {
                return Err(format!(
                    "an event-controlled nonblocking assignment of a queue, dynamic or associative member or of a member array held in a container in `{}` is not supported; use a delay or an event control before the statement",
                    self.ctx.path
                ));
            }
            IrStmt::DeclLocal {
                name,
                width,
                signed,
                two_state,
                init,
            } => {
                self.before.push(IrStmt::DeclLocal {
                    name: name.clone(),
                    width,
                    signed,
                    two_state,
                    init,
                });
                self.capture_value(&name, width, signed);
            }
            IrStmt::DeclString { name, init } => {
                self.before.push(IrStmt::DeclString {
                    name: name.clone(),
                    init,
                });
                self.capture_string(&name);
            }
            IrStmt::Object(operation)
                if matches!(*operation, IrObjectStmt::ChandleDeclareLocal(..)) =>
            {
                let IrObjectStmt::ChandleDeclareLocal(name, init) = *operation else {
                    unreachable!("guard checked the statement");
                };
                self.before
                    .push(IrStmt::Object(Box::new(IrObjectStmt::ChandleDeclareLocal(
                        name.clone(),
                        init,
                    ))));
                self.capture_handle(&name);
            }
            // Temporaries and the calls that produce sources run at issue.
            statement => self.before.push(statement),
        }
        Ok(())
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
        // The branch of a run-time member select is chosen at issue.
        IrStmt::If {
            cond,
            then_,
            els,
            check,
        } => {
            let retime = |statements: Vec<IrStmt>| {
                statements
                    .into_iter()
                    .map(|statement| retime_nonblocking(statement, ticks, path))
                    .collect::<Result<Vec<_>, _>>()
            };
            IrStmt::If {
                cond,
                then_: retime(then_)?,
                els: els.map(retime).transpose()?,
                check,
            }
        }
        IrStmt::Assign {
            lhs,
            rhs,
            nba: true,
        } => IrStmt::DelayedAssign {
            lhs,
            rhs,
            ticks: ticks.clone(),
        },
        IrStmt::DelayedAssign { lhs, rhs, .. } => IrStmt::DelayedAssign {
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
        IrStmt::Container(operation)
            if matches!(*operation, IrContainerStmt::ContainerNonblocking { .. }) =>
        {
            let IrContainerStmt::ContainerNonblocking { target, src, .. } = *operation else {
                unreachable!("guard checked the statement");
            };
            IrStmt::Container(Box::new(IrContainerStmt::ContainerNonblocking {
                target,
                src,
                delay: Some(ticks.clone()),
            }))
        }
        // Every other container statement fills a capture at issue.
        statement @ IrStmt::Container(_) => statement,
        // Captures and the calls that produce native sources run at issue.
        statement @ (IrStmt::DeclLocal { .. }
        | IrStmt::DeclString { .. }
        | IrStmt::NativeValueDeclare(_)
        | IrStmt::Call(_)
        | IrStmt::Nop) => statement,
        IrStmt::Object(operation)
            if matches!(
                *operation,
                IrObjectStmt::ChandleDeclareLocal(..)
                    | IrObjectStmt::StringAssignLocal(..)
                    | IrObjectStmt::ChandleAssignLocal(..)
            ) =>
        {
            IrStmt::Object(operation)
        }
        // A blocking write in a queued statement fills a capture: pattern
        // members into a lexical temporary.
        statement @ IrStmt::Assign { nba: false, .. } => statement,
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
