//! Activation captures. Evaluate user code before publishing raw frames.
use super::native::NativeKind;
use super::*;

pub(super) enum CapturedValue {
    Numeric(Value),
    Handle(String),
    Borrowed(String),
}

fn check_capture(storage: StorageRef) -> Result<(), String> {
    if storage.ownership() == StorageOwnership::Shared {
        return Err(pending("shared activation captures"));
    }
    if storage.ownership() == StorageOwnership::Borrowed
        && (storage.lifetime() != StorageLifetime::Automatic
            || storage.kind() == StorageKind::Opaque)
    {
        return Err("borrowed fork capture requires automatic numeric or event storage".to_owned());
    }
    Ok(())
}

impl Frame<'_, '_> {
    pub(super) fn prepare_captures<'e>(
        &mut self,
        captures: impl Iterator<Item = (StorageRef, &'e IrExpr)>,
    ) -> Result<Vec<(StorageRef, CapturedValue)>, String> {
        let mut values = Vec::new();
        for (storage, initial) in captures {
            check_capture(storage)?;
            if storage.kind() == StorageKind::Event {
                let IrExprKind::ObjectQuery(query) = initial.kind() else {
                    return Err("event capture requires a typed event handle".to_owned());
                };
                let IrObjectQuery::EventCapture(event) = query.as_ref() else {
                    return Err("event capture requires a typed event handle".to_owned());
                };
                if storage.ownership() == StorageOwnership::Borrowed
                    && !matches!(event, IrEventRef::Formal(_) | IrEventRef::Captured(_))
                {
                    return Err("borrowed event capture requires an automatic handle".to_owned());
                }
                let address = self.event_address(event)?;
                let pointer = self.scalar("llg_event_t*", address);
                let handle = if storage.ownership() == StorageOwnership::Borrowed {
                    pointer
                } else {
                    self.scalar(
                        "llg_event_object_t*",
                        format!("{pointer} ? {pointer}->object : NULL"),
                    )
                };
                values.push((storage, CapturedValue::Handle(handle)));
                continue;
            }
            if storage.ownership() == StorageOwnership::Borrowed {
                let binding = match initial.kind() {
                    IrExprKind::LocalRead(name) => self.resolve_lookup(name)?,
                    IrExprKind::FormalRead(index) => {
                        if let Some(formals) = self.formal_overrides.last() {
                            formals
                                .get(*index)
                                .cloned()
                                .ok_or_else(|| "invalid inline formal index".to_owned())?
                        } else {
                            let formal = self
                                .ctx
                                .func
                                .and_then(|func| func.formals.get(*index))
                                .ok_or_else(|| {
                                    "borrowed capture has no enclosing formal".to_owned()
                                })?;
                            if formal.is_ref() {
                                return Err("borrowed capture cannot alias a reference descriptor"
                                    .to_owned());
                            }
                            if formal.is_out {
                                self.address(&format!("o{index}"))?
                            } else {
                                self.resolve_lookup(&format!("a{index}"))?
                            }
                        }
                    }
                    _ => {
                        return Err(
                            "borrowed fork capture requires a local or formal source".to_owned()
                        );
                    }
                };
                if !binding.automatic || binding.width != initial.width {
                    return Err("borrowed fork capture source is not the automatic cell".to_owned());
                }
                values.push((storage, CapturedValue::Borrowed(binding.address)));
                continue;
            }
            if storage.kind() == StorageKind::Opaque {
                let handle = match initial.kind() {
                    IrExprKind::ObjectQuery(query) => match query.as_ref() {
                        IrObjectQuery::HandleCapture(handle) => self.chandle(handle)?,
                        _ => return Err("opaque capture requires a typed handle".to_owned()),
                    },
                    IrExprKind::LocalRead(name) => {
                        let binding = self.native_lookup(name, NativeKind::Chandle)?;
                        self.scalar("void*", format!("*({})", binding.address))
                    }
                    _ => return Err("opaque capture requires a typed handle".to_owned()),
                };
                values.push((storage, CapturedValue::Handle(handle)));
                continue;
            }
            let value = self.expression(initial)?;
            if (value.width == 0) != (storage.kind() == StorageKind::Real) {
                return Err("capture representation does not match its expression".to_owned());
            }
            values.push((storage, CapturedValue::Numeric(value)));
        }
        Ok(values)
    }

    // The caller must not emit any user expression between publishing a frame
    // and transferring/releasing its reference. All source values are already
    // registered owners, so $finish during preparation cannot leak a raw frame.
    pub(super) fn publish_captures(
        &mut self,
        name: &str,
        values: Vec<(StorageRef, CapturedValue)>,
    ) -> String {
        let count = values
            .iter()
            .map(|(storage, _)| u64::from(storage.slot()) + 1)
            .max()
            .unwrap_or(0);
        let access = self.declare_named("llg_frame_t*", name, format!("llg_frame_new({count}ULL)"));
        for (storage, value) in values {
            match value {
                CapturedValue::Borrowed(address) => {
                    let operation = if storage.kind() == StorageKind::Real {
                        "real"
                    } else {
                        "value"
                    };
                    self.line(format!(
                        "llg_frame_alias_{operation}({access}, {}u, {address});",
                        storage.slot()
                    ));
                }
                CapturedValue::Handle(handle) => self.line(format!(
                    "llg_frame_capture_opaque({access}, {}u, {handle});",
                    storage.slot()
                )),
                CapturedValue::Numeric(value) => {
                    let operation = if storage.kind() == StorageKind::Real {
                        "real"
                    } else {
                        "value"
                    };
                    self.line(format!(
                        "llg_frame_capture_{operation}({access}, {}u, {});",
                        storage.slot(),
                        value.code
                    ));
                    self.discard(value);
                }
            }
        }
        access
    }

    pub(super) fn bind_capture(
        &mut self,
        name: &str,
        storage: StorageRef,
        initial: &IrExpr,
        source: &str,
    ) -> Result<(), String> {
        check_capture(storage)?;
        if storage.kind() == StorageKind::Event {
            let address = if storage.ownership() == StorageOwnership::Borrowed {
                self.declare(
                    "llg_event_t*",
                    "event_alias",
                    format!("llg_frame_read_opaque({source}, {}u)", storage.slot()),
                )
            } else {
                let local = self.declare(
                    "llg_event_t",
                    "event_capture",
                    format!("{{ llg_frame_read_opaque({source}, {}u) }}", storage.slot()),
                );
                format!("&{local}")
            };
            self.event_bindings
                .last_mut()
                .expect("event scope")
                .insert(name.to_owned(), address);
            return Ok(());
        }
        if storage.ownership() == StorageOwnership::Borrowed {
            let (ty, operation) = if storage.kind() == StorageKind::Real {
                ("double", "real")
            } else {
                ("sv4_t", "value")
            };
            let pointer = self.declare(
                &format!("{ty}*"),
                "capture_alias",
                format!(
                    "llg_frame_{operation}_address({source}, {}u)",
                    storage.slot()
                ),
            );
            self.bindings
                .last_mut()
                .expect("frame always has a binding scope")
                .insert(
                    name.to_owned(),
                    Binding {
                        address: pointer,
                        width: initial.width,
                        signed: initial.signed,
                        two_state: false,
                        shortreal: false,
                        automatic: true,
                    },
                );
            return Ok(());
        }
        if storage.kind() == StorageKind::Opaque {
            let binding = self.native_local(name, NativeKind::Chandle);
            self.line(format!(
                "*({}) = llg_frame_read_opaque({source}, {}u);",
                binding.address,
                storage.slot()
            ));
            return Ok(());
        }
        if (initial.width == 0) != (storage.kind() == StorageKind::Real) {
            return Err("capture representation does not match its expression".to_owned());
        }
        self.local(name, initial.width, initial.signed, false, None)?;
        let binding = self
            .lookup(name)
            .ok_or_else(|| "missing capture owner".to_owned())?;
        if storage.kind() == StorageKind::Real {
            self.line(format!(
                "*({}) = llg_frame_read_real({source}, {}u);",
                binding.address,
                storage.slot()
            ));
        } else {
            self.line(format!(
                "sv4_replace({}, llg_frame_read_value({source}, {}u));",
                binding.address,
                storage.slot()
            ));
        }
        Ok(())
    }

    pub(super) fn captured_fork(
        &mut self,
        kind: IrJoinKind,
        branches: &[IrCapturedBranch],
        target: Option<IrActivationTarget>,
    ) -> Result<(), String> {
        if kind != IrJoinKind::Join
            && branches.iter().any(|branch| {
                branch
                    .captures()
                    .iter()
                    .any(|capture| capture.storage().ownership() == StorageOwnership::Borrowed)
            })
        {
            return Err("only a synchronous fork join may borrow automatic storage".to_owned());
        }
        // Evaluate every initializer before creating a group or child frame.
        let mut prepared = Vec::new();
        for branch in branches {
            prepared.push(
                self.prepare_captures(
                    branch
                        .captures()
                        .iter()
                        .map(|capture| (capture.storage(), capture.initial())),
                )?,
            );
        }
        let join_kind = kind;
        let kind = match join_kind {
            IrJoinKind::Join => "LLG_JOIN",
            IrJoinKind::Any => "LLG_JOIN_ANY",
            IrJoinKind::None => "LLG_JOIN_NONE",
        };
        let group = self.fork_group(kind, target);
        for (branch, values) in branches.iter().zip(prepared) {
            let frame = self.name("capture_frame");
            let frame = self.publish_captures(&frame, values);
            self.line(format!(
                "llg_fork_with_frame(&{}_desc, {}, {group}, {frame});",
                branch.c_name(),
                c_string_literal(branch.label())
            ));
            self.line(format!("llg_frame_release({frame});"));
        }
        if !branches.is_empty() && join_kind != IrJoinKind::None {
            self.await_arm(
                SuspensionOperation::ForkJoin,
                format!("llg_arm_join(self, {group})"),
            )?;
        }
        Ok(())
    }
}
