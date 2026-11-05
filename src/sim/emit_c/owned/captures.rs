//! Activation captures. Evaluate user code before publishing raw frames.
use super::native::NativeKind;
use super::*;

pub(super) enum CapturedValue {
    Numeric(Value),
    Handle(String),
    Borrowed(String),
    /// A slot of a shared activation frame: (frame expression, slot).
    Shared(String, u32),
    /// The change marker of handle `ref` formal `.0` (`.1`, NULL for an
    /// actual no other process can name). A fork shares it among its
    /// branches through one holder slot (see `captured_fork`).
    RefMarker(usize, String),
    /// The address of a string copied into the frame.
    String(String),
    /// A string evaluated for the capture; copied into the frame, then
    /// released by the issuing frame.
    OwnedString(super::native::NativeValue),
}

fn check_capture(storage: StorageRef) -> Result<(), String> {
    if storage.ownership() == StorageOwnership::Shared
        && (storage.lifetime() != StorageLifetime::Automatic
            || !matches!(
                storage.kind(),
                StorageKind::Packed
                    | StorageKind::Real
                    | StorageKind::String
                    | StorageKind::Container
                    | StorageKind::Native
                    | StorageKind::Opaque
            ))
    {
        return Err("shared fork capture requires automatic numeric or string storage".to_owned());
    }
    if storage.kind() == StorageKind::String && storage.ownership() == StorageOwnership::Borrowed {
        return Err("string fork captures are copied or shared".to_owned());
    }
    if matches!(storage.kind(), StorageKind::Container | StorageKind::Native)
        && storage.ownership() != StorageOwnership::Shared
    {
        return Err("container and native record fork captures are shared".to_owned());
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
        // `ref` formals whose descriptor is already captured: a handle
        // formal's second capture passes its change marker.
        let mut captured_formals = Vec::new();
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
            if storage.kind() == StorageKind::String
                && storage.ownership() == StorageOwnership::Owned
            {
                if let IrExprKind::ObjectQuery(query) = initial.kind() {
                    if let IrObjectQuery::StringCapture(value) = query.as_ref() {
                        let value = self.string(value)?;
                        values.push((storage, CapturedValue::OwnedString(value)));
                        continue;
                    }
                }
                let IrExprKind::LocalRead(name) = initial.kind() else {
                    return Err("string fork capture requires a local source".to_owned());
                };
                let binding = self.native_read(name, NativeKind::String)?;
                values.push((storage, CapturedValue::String(binding.address)));
                continue;
            }
            if storage.ownership() == StorageOwnership::Shared {
                let IrExprKind::LocalRead(name) = initial.kind() else {
                    return Err("shared fork capture requires a local source".to_owned());
                };
                let (frame, slot) =
                    self.shared_cells.get(name).cloned().ok_or_else(|| {
                        format!("shared fork capture of `{name}` has no shared cell")
                    })?;
                values.push((storage, CapturedValue::Shared(frame, slot)));
                continue;
            }
            if storage.ownership() == StorageOwnership::Borrowed {
                let binding = match initial.kind() {
                    IrExprKind::LocalRead(name) => self.read_lookup(name)?,
                    IrExprKind::FormalRead(index) => {
                        if let Some(formals) = self.formal_overrides.last() {
                            formals
                                .get(*index)
                                .cloned()
                                .ok_or_else(|| "invalid inline formal index".to_owned())?
                        } else {
                            let formal = self
                                .formal_function()
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
                    // A `ref` formal named by a fork-join branch passes its
                    // descriptor, which stays valid until the join (the
                    // subroutine waits for every branch).
                    IrExprKind::FormalRead(index) => {
                        if !self.formal_overrides.is_empty()
                            || !self
                                .formal_function()
                                .and_then(|function| function.formals.get(*index))
                                .is_some_and(|formal| formal.is_ref())
                        {
                            return Err(
                                "fork capture of a `ref` formal requires its descriptor".to_owned()
                            );
                        }
                        let markerless_event = self
                            .formal_function()
                            .and_then(|function| function.formals.get(*index))
                            .is_some_and(|formal| formal.event && !formal.event_marker);
                        if captured_formals.contains(index) && markerless_event {
                            self.scalar("void*", "NULL".to_owned())
                        } else if captured_formals.contains(index) {
                            let marker = super::super::model::handle_marker_param(*index);
                            let marker = self.scalar("void*", format!("(void*){marker}"));
                            values.push((storage, CapturedValue::RefMarker(*index, marker)));
                            continue;
                        } else {
                            captured_formals.push(*index);
                            self.scalar("void*", format!("(void*)(uintptr_t)r{index}"))
                        }
                    }
                    IrExprKind::ObjectQuery(query) => match query.as_ref() {
                        IrObjectQuery::HandleCapture(handle) => self.chandle(handle)?,
                        _ => return Err("opaque capture requires a typed handle".to_owned()),
                    },
                    IrExprKind::LocalRead(name) => {
                        let binding = self.native_read(name, NativeKind::Chandle)?;
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
                CapturedValue::Handle(handle) | CapturedValue::RefMarker(_, handle) => {
                    self.line(format!(
                        "llg_frame_capture_opaque({access}, {}u, {handle});",
                        storage.slot()
                    ))
                }
                CapturedValue::Shared(frame, slot) => self.line(format!(
                    "llg_frame_alias_slot({access}, {}u, {frame}, {slot}u);",
                    storage.slot()
                )),
                CapturedValue::OwnedString(value) => {
                    self.line(format!(
                        "llg_frame_capture_string({access}, {}u, {});",
                        storage.slot(),
                        value.address
                    ));
                    self.native_discard(value);
                }
                CapturedValue::String(address) => self.line(format!(
                    "llg_frame_capture_string({access}, {}u, {address});",
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
        if storage.kind() == StorageKind::Native {
            let IrExprKind::LocalRead(local) = initial.kind() else {
                return Err("native record fork capture requires its capture name".to_owned());
            };
            let index = (0..self.ctx.model.native_values.len())
                .find(|index| crate::sim::ir::shared_native_capture_name(*index) == *local)
                .ok_or("native record fork capture names no value")?;
            self.bind_shared_native_value(index, source, storage.slot())?;
            self.shared_cells
                .insert(local.clone(), (source.to_owned(), storage.slot()));
            return Ok(());
        }
        if storage.kind() == StorageKind::Container {
            let IrExprKind::LocalRead(local) = initial.kind() else {
                return Err("container fork capture requires its capture name".to_owned());
            };
            let index = (0..self.ctx.model.containers.len())
                .find(|index| crate::sim::ir::shared_container_capture_name(*index) == *local)
                .ok_or("container fork capture names no container")?;
            let (ty, _, _) = super::super::containers::activation_storage(
                &self.ctx.model.containers[index],
                "",
            )?;
            let pointer = self.declare(
                &format!("{ty}*"),
                "capture_container",
                format!(
                    "({ty}*)llg_frame_object_address({source}, {}u)",
                    storage.slot()
                ),
            );
            self.containers.insert(index, format!("(*{pointer})"));
            self.shared_cells
                .insert(local.clone(), (source.to_owned(), storage.slot()));
            return Ok(());
        }
        if storage.kind() == StorageKind::String {
            if let IrExprKind::ObjectQuery(query) = initial.kind() {
                if matches!(query.as_ref(), IrObjectQuery::StringCapture(_)) {
                    let address = self.declare(
                        "llg_string_t*",
                        "capture_string",
                        format!("llg_frame_string_address({source}, {}u)", storage.slot()),
                    );
                    self.bind_native(name, address, NativeKind::String);
                    return Ok(());
                }
            }
            // A captured string keeps its source local's name, so the
            // branch's string reads and writes resolve unchanged.
            let IrExprKind::LocalRead(local) = initial.kind() else {
                return Err("string fork capture requires a local source".to_owned());
            };
            let address = self.declare(
                "llg_string_t*",
                "capture_string",
                format!("llg_frame_string_address({source}, {}u)", storage.slot()),
            );
            self.shared_cells
                .insert(local.clone(), (source.to_owned(), storage.slot()));
            self.bind_native(local, address.clone(), NativeKind::String);
            self.bind_native(name, address, NativeKind::String);
            return Ok(());
        }
        if storage.kind() == StorageKind::Opaque && storage.ownership() == StorageOwnership::Shared
        {
            // A shared handle keeps its source local's name, so the branch's
            // handle reads and writes resolve unchanged.
            let IrExprKind::LocalRead(local) = initial.kind() else {
                return Err("shared handle capture requires a local source".to_owned());
            };
            let pointer = self.declare(
                "void**",
                "capture_handle",
                format!("llg_frame_opaque_address({source}, {}u)", storage.slot()),
            );
            // Branch code may also name it by its capture local.
            self.bind_native(name, pointer.clone(), NativeKind::Chandle);
            self.bind_native(local, pointer, NativeKind::Chandle);
            self.shared_cells
                .insert(local.clone(), (source.to_owned(), storage.slot()));
            self.shared_cells
                .insert(name.to_owned(), (source.to_owned(), storage.slot()));
            return Ok(());
        }
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
        if matches!(
            storage.ownership(),
            StorageOwnership::Borrowed | StorageOwnership::Shared
        ) {
            if storage.ownership() == StorageOwnership::Shared {
                // Nested forks alias this branch's own slot, which aliases
                // the declaring frame.
                self.shared_cells
                    .insert(name.to_owned(), (source.to_owned(), storage.slot()));
            }
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
            let binding = Binding {
                address: pointer,
                width: initial.width,
                signed: initial.signed,
                two_state: false,
                shortreal: false,
                automatic: true,
            };
            let scope = self
                .bindings
                .last_mut()
                .expect("frame always has a binding scope");
            // Selected and projected accesses (an element of a captured
            // automatic array) name the variable by its source local, as
            // captured strings and handles do.
            if let IrExprKind::LocalRead(local) = initial.kind() {
                if local != name {
                    scope.insert(local.clone(), binding.clone());
                }
            }
            scope.insert(name.to_owned(), binding);
            return Ok(());
        }
        if storage.kind() == StorageKind::Opaque {
            if let IrExprKind::FormalRead(index) = initial.kind() {
                return self.bind_ref_formal_capture(*index, false, storage, source);
            }
            let binding = self.native_local(name, NativeKind::Chandle);
            self.line(format!(
                "*({}) = llg_frame_read_opaque({source}, {}u);",
                binding.address,
                storage.slot()
            ));
            // A copied handle local (such as a method's receiver) is also
            // named as its source in the branch.
            if let IrExprKind::ObjectQuery(query) = initial.kind() {
                if let IrObjectQuery::HandleCapture(IrChandleExpr::LocalRead(local)) =
                    query.as_ref()
                {
                    self.bind_native(local, binding.address.clone(), NativeKind::Chandle);
                }
            }
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
            self.assign(
                &binding.address,
                &format!("llg_frame_read_value({source}, {}u)", storage.slot()),
            );
        }
        Ok(())
    }

    /// Bind the descriptor of `ref` formal `index` that a fork-join branch
    /// captured under its parameter name (`rN`), so formal reads, stores,
    /// selects and waits in the branch emit exactly as in the subroutine.
    /// With `marker`, bind a handle or event formal's change marker parameter, which
    /// the branch captured in the slot after the descriptor's.
    pub(super) fn bind_ref_formal_capture(
        &mut self,
        index: usize,
        marker: bool,
        storage: StorageRef,
        source: &str,
    ) -> Result<(), String> {
        let function = self
            .formal_function()
            .ok_or("captured `ref` formal outside its subroutine")?;
        let formal = function
            .formals
            .get(index)
            .filter(|formal| formal.is_ref())
            .ok_or("captured `ref` formal is not a `ref` formal")?;
        if marker && !formal.chandle && !formal.event {
            return Err("only a handle or event `ref` formal captures a change marker".to_owned());
        }
        // The slot is reserved in every model; only some give the formal a
        // marker parameter (`IrFormal::event_marker`).
        if marker && formal.event && !formal.event_marker {
            return Ok(());
        }
        let name = if marker {
            super::super::model::handle_marker_param(index)
        } else {
            format!("r{index}")
        };
        let ty = super::super::model::owned_frame_param_fields(function)
            .into_iter()
            .find(|(_, field)| *field == name)
            .map(|(ty, _)| ty)
            .ok_or("captured `ref` formal has no descriptor parameter")?;
        let load = if marker {
            // The holder slot keeps the caller's marker, else allocates the
            // one the branches share.
            format!(
                "({ty})llg_frame_read_opaque({source}, {slot}u) ? ({ty})llg_frame_read_opaque({source}, {slot}u) : llg_frame_opaque_marker({source}, {slot}u, 1)",
                slot = storage.slot()
            )
        } else {
            format!(
                "({ty})(uintptr_t)llg_frame_read_opaque({source}, {}u)",
                storage.slot()
            )
        };
        if self.layout.storage() == FrameStorage::CoFrame {
            self.frame_field(&ty, &name)?;
            self.line(format!("{name} = {load};"));
        } else {
            self.line(format!("{ty} {name} = {load};"));
        }
        if formal.event && !marker {
            self.event_bindings
                .first_mut()
                .expect("event scope")
                .insert(super::events::event_formal_binding(index), name);
        }
        Ok(())
    }

    /// Bind `alias` to the packed or real cell already bound as `name` in
    /// the innermost scope (see `IrCapture::alias`).
    pub(super) fn alias_capture(&mut self, alias: &str, name: &str) {
        let scope = self
            .bindings
            .last_mut()
            .expect("frame always has a binding scope");
        if let Some(binding) = scope.get(name).cloned() {
            scope.insert(alias.to_owned(), binding);
        }
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
        // The branches of one fork share each handle `ref` formal's change
        // marker through one holder slot: the caller's marker, or one the
        // first branch that stores or waits allocates there (an actual no
        // other process can name has none, but sibling branches can).
        let mut holders: Vec<(usize, String)> = Vec::new();
        for values in &mut prepared {
            for (_, value) in values.iter_mut() {
                let CapturedValue::RefMarker(index, marker) = value else {
                    continue;
                };
                let holder =
                    if let Some((_, holder)) = holders.iter().find(|(formal, _)| formal == index) {
                        holder.clone()
                    } else {
                        let holder = self.declare(
                            "llg_frame_t*",
                            "marker_holder",
                            "llg_frame_new(1ULL)".into(),
                        );
                        self.line(format!("llg_frame_capture_opaque({holder}, 0u, {marker});"));
                        holders.push((*index, holder.clone()));
                        holder
                    };
                *value = CapturedValue::Shared(holder, 0);
            }
        }
        let join_kind = kind;
        if join_kind == IrJoinKind::Detached {
            if target.is_some() {
                return Err("a detached process has no disable target".to_owned());
            }
            for (branch, values) in branches.iter().zip(prepared) {
                let frame = self.name("capture_frame");
                let frame = self.publish_captures(&frame, values);
                self.line(format!(
                    "llg_spawn_detached_with_frame(&{}_desc, {}, {frame});",
                    branch.c_name(),
                    c_string_literal(branch.label())
                ));
                self.line(format!("llg_frame_release({frame});"));
            }
            return Ok(());
        }
        let kind = match join_kind {
            IrJoinKind::Join => "LLG_JOIN",
            IrJoinKind::Any => "LLG_JOIN_ANY",
            IrJoinKind::None => "LLG_JOIN_NONE",
            IrJoinKind::Detached => unreachable!("detached spawns returned above"),
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
        for (_, holder) in holders {
            self.line(format!("llg_frame_release({holder});"));
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
