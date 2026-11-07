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
            || self.whole_unpacked_record(member)
    }

    /// A record with no packed image: its leaves are written one by one.
    pub(super) fn whole_unpacked_record(&self, node: NodeId) -> bool {
        self.cg
            .resolve_unpacked_aggregate(node)
            .is_some_and(|selection| {
                matches!(
                    &selection.descriptor.shape,
                    TypeShape::Aggregate(layout)
                        if matches!(
                            layout.kind,
                            AggregateKind::UnpackedStruct | AggregateKind::UnpackedUnion
                        )
                )
            })
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
            hold: None,
            guards: Vec::new(),
        };
        let mut action = Vec::new();
        split.statement(statement?, &mut action)?;
        let EventNbaSplit {
            mut before,
            captures,
            hold,
            ..
        } = split;
        // Last, so a guarded write that did not run leaves nothing behind.
        action.extend(hold.map(|held| {
            IrStmt::Container(Box::new(IrContainerStmt::NbaHoldRelease {
                held: IrChandleExpr::LocalRead(held),
                delay: None,
            }))
        }));
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
        let mut retime = Retime {
            ticks: &ticks,
            path: &self.path,
            prefix: format!("_nd{}", h.0),
            sequence: 0,
        };
        before.push(retime.statement(statement)?);
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
        IrStmt::Container(operation)
            if matches!(
                **operation,
                IrContainerStmt::NbaLocate { .. } | IrContainerStmt::NbaLocateEnd
            ) =>
        {
            true
        }
        statement => builds_update_in_runtime(statement),
    }
}

/// A queued write whose value the runtime copies, or that writes in a loop:
/// no local can stand for its value, so it runs at issue inside a hold.
fn builds_update_in_runtime(statement: &IrStmt) -> bool {
    match statement.unlocated() {
        IrStmt::FixedValueAssign { nba, .. }
        | IrStmt::FixedArrayFill { nba, .. }
        | IrStmt::FixedArrayCopy { nba, .. }
        | IrStmt::StreamAssign { nba, .. } => *nba,
        IrStmt::Container(operation) => matches!(
            **operation,
            IrContainerStmt::Nonblocking { .. }
                | IrContainerStmt::ContainerNonblocking { .. }
                | IrContainerStmt::RealColumnAssign { nba: true, .. }
        ),
        IrStmt::For { body, .. } | IrStmt::While { body, .. } | IrStmt::Repeat { body, .. } => {
            body.iter().any(queues_write)
        }
        _ => false,
    }
}

fn nba_hold_write(held: &str, write: Vec<IrStmt>) -> Vec<IrStmt> {
    let mut statements = vec![IrStmt::Container(Box::new(IrContainerStmt::NbaHoldOpen(
        IrChandleExpr::LocalRead(held.to_owned()),
    )))];
    statements.extend(write);
    statements.push(IrStmt::Container(Box::new(IrContainerStmt::NbaHoldClose)));
    statements
}

fn nba_hold_declare(held: &str) -> IrStmt {
    IrStmt::Object(Box::new(IrObjectStmt::ChandleDeclareLocal(
        held.to_owned(),
        Some(IrChandleExpr::NbaHold),
    )))
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
    /// The local naming the hold of the updates built at issue, once a write
    /// needs one.
    hold: Option<String>,
    /// The enclosing branches, as issue-time conditions and the arm taken: a
    /// write in the hold runs at issue under the same choice.
    guards: Vec<(IrExpr, bool)>,
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

    /// Run `write` now under the enclosing branches, with its updates kept
    /// for the action.
    fn hold_write(&mut self, write: Vec<IrStmt>) {
        let held = match &self.hold {
            Some(held) => held.clone(),
            None => {
                let held = self.fresh();
                self.before.push(nba_hold_declare(&held));
                self.capture_handle(&held);
                self.hold = Some(held.clone());
                held
            }
        };
        let mut statements = nba_hold_write(&held, write);
        for (cond, taken) in self.guards.iter().rev() {
            let (then_, els) = if *taken {
                (statements, None)
            } else {
                (Vec::new(), Some(statements))
            };
            statements = vec![IrStmt::If {
                cond: cond.clone(),
                then_,
                els,
                check: IrUniquePriorityCheck::None,
            }];
        }
        self.before.extend(statements);
    }

    fn statement(&mut self, statement: IrStmt, action: &mut Vec<IrStmt>) -> Result<(), String> {
        match statement {
            IrStmt::Block(statements) => {
                let mut statements = statements.into_iter().peekable();
                while let Some(statement) = statements.next() {
                    let locates = matches!(
                        statement.unlocated(),
                        IrStmt::Container(operation)
                            if matches!(**operation, IrContainerStmt::NbaLocate { .. })
                    );
                    // A located write keeps its locator: the position is
                    // fixed at issue and the item found again at commit.
                    if locates && statements.peek().is_some_and(builds_update_in_runtime) {
                        let mut group = vec![statement];
                        group.extend(statements.next());
                        group.extend(statements.next());
                        self.hold_write(group);
                        continue;
                    }
                    self.statement(statement, action)?;
                }
            }
            IrStmt::Located { statement, .. } => self.statement(*statement, action)?,
            statement if builds_update_in_runtime(&statement) => {
                self.hold_write(vec![statement]);
            }
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
                // The target is chosen at issue, like its selectors.
                let cond = self.issue_value(cond);
                let mut then_action = Vec::new();
                self.guards.push((cond.clone(), true));
                for statement in then_ {
                    self.statement(statement, &mut then_action)?;
                }
                self.guards.pop();
                let mut else_action = Vec::new();
                self.guards.push((cond.clone(), false));
                for statement in els.into_iter().flatten() {
                    self.statement(statement, &mut else_action)?;
                }
                self.guards.pop();
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

/// Gives every queued leaf write of an untimed record NBA the delay `ticks`.
struct Retime<'a> {
    ticks: &'a IrDelay,
    path: &'a str,
    prefix: String,
    sequence: usize,
}

impl Retime<'_> {
    fn statement(&mut self, statement: IrStmt) -> Result<IrStmt, String> {
        let ticks = self.ticks;
        let path = self.path;
        Ok(match statement {
        IrStmt::Block(statements) => IrStmt::Block(
            statements
                .into_iter()
                .map(|statement| self.statement(statement))
                .collect::<Result<_, _>>()?,
        ),
        IrStmt::Located { origin, statement } => IrStmt::Located {
            origin,
            statement: Box::new(self.statement(*statement)?),
        },
        // The branch of a run-time member select is chosen at issue.
        IrStmt::If {
            cond,
            then_,
            els,
            check,
        } => {
            let mut retime = |statements: Vec<IrStmt>| {
                statements
                    .into_iter()
                    .map(|statement| self.statement(statement))
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
        IrStmt::Container(operation)
            if matches!(*operation, IrContainerStmt::Nonblocking { .. }) =>
        {
            let IrContainerStmt::Nonblocking {
                target,
                dst_start,
                src,
                src_start,
                count,
                ..
            } = *operation
            else {
                unreachable!("guard checked the statement");
            };
            IrStmt::Container(Box::new(IrContainerStmt::Nonblocking {
                target,
                dst_start,
                src,
                src_start,
                count,
                delay: Some(ticks.clone()),
            }))
        }
        // No delay field: the updates are built here and delayed as a group.
        statement if builds_update_in_runtime(&statement) => {
            self.sequence += 1;
            let held = format!("{}_h{}", self.prefix, self.sequence);
            let mut statements = vec![nba_hold_declare(&held)];
            statements.extend(nba_hold_write(&held, vec![statement]));
            statements.push(IrStmt::Container(Box::new(
                IrContainerStmt::NbaHoldRelease {
                    held: IrChandleExpr::LocalRead(held),
                    delay: Some(ticks.clone()),
                },
            )));
            IrStmt::Block(statements)
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
