//! Numeric capture snapshots. Evaluate user code before publishing raw frames.
use super::*;
use super::native::NativeKind;

pub(super) enum CapturedValue { Numeric(Value), Handle(String) }

fn check_capture(storage: StorageRef) -> Result<(), String> {
    if storage.ownership() != StorageOwnership::Owned {
        return Err(pending("opaque, borrowed, or shared activation captures"));
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
    pub(super) fn publish_captures(&mut self, name: &str, values: Vec<(StorageRef, CapturedValue)>) {
        let count = values.iter().map(|(storage, _)| u64::from(storage.slot()) + 1)
            .max().unwrap_or(0);
        self.line(format!("llg_frame_t* {name} = llg_frame_new({count}ULL);"));
        for (storage, value) in values {
            match value {
                CapturedValue::Handle(handle) => self.line(format!("llg_frame_capture_opaque({name}, {}u, {handle});", storage.slot())),
                CapturedValue::Numeric(value) => {
                    let operation = if storage.kind() == StorageKind::Real { "real" } else { "value" };
                    self.line(format!("llg_frame_capture_{operation}({name}, {}u, {});", storage.slot(), value.code));
                    self.discard(value);
                }
            }
        }
    }

    pub(super) fn bind_capture(
        &mut self,
        name: &str,
        storage: StorageRef,
        initial: &IrExpr,
        source: &str,
    ) -> Result<(), String> {
        check_capture(storage)?;
        if storage.kind() == StorageKind::Opaque {
            let binding = self.native_local(name, NativeKind::Chandle);
            self.line(format!("*({}) = llg_frame_read_opaque({source}, {}u);", binding.address, storage.slot()));
            return Ok(());
        }
        if (initial.width == 0) != (storage.kind() == StorageKind::Real) {
            return Err("capture representation does not match its expression".to_owned());
        }
        self.local(name, initial.width, initial.signed, false, None)?;
        let binding = self.lookup(name).ok_or_else(|| "missing capture owner".to_owned())?;
        if storage.kind() == StorageKind::Real {
            self.line(format!("*({}) = llg_frame_read_real({source}, {}u);", binding.address, storage.slot()));
        } else {
            self.line(format!("sv4_replace({}, llg_frame_read_value({source}, {}u));", binding.address, storage.slot()));
        }
        Ok(())
    }

    pub(super) fn captured_fork(
        &mut self,
        kind: IrJoinKind,
        branches: &[IrCapturedBranch],
        target: Option<IrActivationTarget>,
    ) -> Result<(), String> {
        // Evaluate every initializer before creating a group or child frame.
        let mut prepared = Vec::new();
        for branch in branches {
            prepared.push(self.prepare_captures(branch.captures().iter()
                .map(|capture| (capture.storage(), capture.initial())))?);
        }
        let kind = match kind {
            IrJoinKind::Join => "LLG_JOIN", IrJoinKind::Any => "LLG_JOIN_ANY",
            IrJoinKind::None => "LLG_JOIN_NONE",
        };
        let group = self.fork_group(kind, target);
        for (branch, values) in branches.iter().zip(prepared) {
            let frame = self.name("capture_frame");
            self.publish_captures(&frame, values);
            self.line(format!("llg_fork_with_frame({}, {}, {group}, {frame});",
                branch.c_name(), c_string_literal(branch.label())));
            self.line(format!("llg_frame_release({frame});"));
        }
        self.line(format!("llg_join({group});"));
        Ok(())
    }
}
