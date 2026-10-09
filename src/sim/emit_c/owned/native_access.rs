//! Resolve validated dynamic storage identities at the use site.
use super::native::{NativeBinding, NativeKind};
use super::*;

impl Frame<'_, '_> {
    fn access_address(&mut self, name: &str) -> Result<Option<(String, IrClassFieldType)>, String> {
        let Some(access) = self
            .ctx
            .model
            .native_accesses
            .iter()
            .find(|a| a.name == name)
            .cloned()
        else {
            return Ok(None);
        };
        if let IrNativeAccessKind::ValueItem { value, ty } = access.kind {
            let address = self.native_value_leaf(value, &access.item_path, ty)?;
            let c_type = match ty {
                IrClassFieldType::Packed { .. } => "sv4_t*",
                IrClassFieldType::Real { .. } => "double*",
                IrClassFieldType::String => "llg_string_t*",
                IrClassFieldType::Chandle => "void**",
            };
            return Ok(Some((self.scalar(c_type, address), ty)));
        }
        if let IrNativeAccessKind::ElementItem { ty } = access.kind {
            // The locator is re-evaluated at every use: element addresses
            // are borrowed only until the container next changes shape.
            let element = self.chandle(&access.receiver)?;
            let mut item = format!("((llg_value_t*){element})");
            for (depth, step) in access.item_path.iter().enumerate() {
                item = if depth == 0 {
                    format!("{item}->value.items[{step}]")
                } else {
                    format!("{item}.value.items[{step}]")
                };
            }
            if access.item_path.is_empty() {
                return Err("element item requires an item path".to_owned());
            }
            let c_type = match ty {
                IrClassFieldType::Packed { .. } => "sv4_t*",
                IrClassFieldType::Real { .. } => "double*",
                IrClassFieldType::String => "llg_string_t*",
                IrClassFieldType::Chandle => "void**",
            };
            let address = format!("&{item}.value.{}", super::native_values::leaf_member(ty));
            return Ok(Some((self.scalar(c_type, address), ty)));
        }
        if self.access_stack.iter().any(|active| active == name) || self.access_stack.len() >= 256 {
            return Err("cyclic or excessively deep native storage access".to_owned());
        }
        self.access_stack.push(name.to_owned());
        let receiver = self.chandle(&access.receiver);
        self.access_stack.pop();
        let receiver = receiver?;
        let (address, ty) = match access.kind {
            IrNativeAccessKind::ClassField { class, field } => {
                let ty = self.ctx.model.classes[class].fields[field].ty;
                let (member, kind) = match ty {
                    IrClassFieldType::Packed { .. } => ("packed", 0),
                    IrClassFieldType::Real { .. } => ("real", 1),
                    IrClassFieldType::String => ("string", 2),
                    IrClassFieldType::Chandle => ("handle", 3),
                };
                let lookup = if self.quiet_receivers {
                    "llg_class_field_quiet"
                } else {
                    "llg_class_field"
                };
                (
                    format!("&{lookup}({receiver}, {class}, {field}, {kind})->value.{member}"),
                    ty,
                )
            }
            IrNativeAccessKind::InterfaceMember { interface, member } => {
                let metadata = &self.ctx.model.virtual_interfaces[interface].members[member];
                let site = access.site.as_deref().unwrap_or(name);
                let kind = if metadata.real {
                    "real_member"
                } else {
                    "member"
                };
                (
                    if self.quiet_receivers {
                        format!("llg_vif_{kind}_quiet({receiver}, {interface}, {member})")
                    } else {
                        format!(
                            "llg_vif_{kind}({receiver}, {interface}, {member}, {})",
                            c_string_literal(site)
                        )
                    },
                    if metadata.real {
                        IrClassFieldType::Real {
                            shortreal: metadata.shortreal,
                        }
                    } else {
                        IrClassFieldType::Packed {
                            width: metadata.width,
                            signed: metadata.signed,
                            two_state: metadata.two_state,
                        }
                    },
                )
            }
            IrNativeAccessKind::ValueItem { .. } | IrNativeAccessKind::ElementItem { .. } => {
                unreachable!("value and element items resolve above")
            }
        };
        let c_type = match ty {
            IrClassFieldType::Packed { .. } => "sv4_t*",
            IrClassFieldType::Real { .. } => "double*",
            IrClassFieldType::String => "llg_string_t*",
            IrClassFieldType::Chandle => "void**",
        };
        Ok(Some((self.scalar(c_type, address), ty)))
    }

    pub(super) fn resolve_lookup(&mut self, name: &str) -> Result<Binding, String> {
        if let Some((address, ty)) = self.access_address(name)? {
            let (width, signed, two_state, shortreal) = match ty {
                IrClassFieldType::Packed {
                    width,
                    signed,
                    two_state,
                } => (width, signed, two_state, false),
                IrClassFieldType::Real { shortreal } => (0, false, false, shortreal),
                _ => return Err("native object member used as packed storage".to_owned()),
            };
            return Ok(Binding {
                address,
                width,
                signed,
                two_state,
                shortreal,
                automatic: false,
            });
        }
        self.lookup(name)
            .ok_or_else(|| pending(&format!("unresolved local read {name}")))
    }

    pub(super) fn native_access_binding(
        &mut self,
        name: &str,
        kind: NativeKind,
    ) -> Result<Option<NativeBinding>, String> {
        let Some((address, ty)) = self.access_address(name)? else {
            return Ok(None);
        };
        if !matches!(
            (ty, kind),
            (IrClassFieldType::String, NativeKind::String)
                | (IrClassFieldType::Chandle, NativeKind::Chandle)
        ) {
            return Err("native member storage type mismatch".to_owned());
        }
        Ok(Some(NativeBinding {
            address,
            kind,
            automatic: false,
        }))
    }

    pub(super) fn construct_class(&mut self, index: usize) -> Result<String, String> {
        if self.read_only_callback {
            return Err(pending("class construction in read-only callbacks"));
        }
        if self.construction_stack.contains(&index) || self.construction_stack.len() >= 256 {
            return Err("cyclic or excessively deep inline constructor recipe".to_owned());
        }
        let allocation = self.ctx.model.class_allocations[index].clone();
        let result = self.scalar("void*", format!("llg_class_new_{}()", allocation.class));
        self.begin_block(&allocation.body);
        let local = self.native_local(&allocation.local, NativeKind::Chandle);
        self.line(format!("*({}) = {result};", local.address));
        self.construction_stack.push(index);
        for statement in &allocation.body {
            self.statement(statement)?;
        }
        self.construction_stack.pop();
        self.end_block();
        Ok(result)
    }
}

/// The change a store into handle storage must publish when it does not go
/// through `llg_class_handle_store` or a module handle's assignment (task
/// output copy-out, mailbox retrieval, task-form `$cast`; SV 9.4.2).
pub(super) struct HandleStorePublish {
    /// The slot's value before the store.
    previous: String,
    /// The statement that publishes a changed value.
    publish: String,
    slot: String,
}

impl Frame<'_, '_> {
    /// Capture what a store through `slot` (resolved from IR address
    /// `address`) must publish: a module handle object's change marker, or
    /// the marker of the object holding a handle property. Call it before
    /// the store; other handle storage publishes nothing.
    pub(super) fn handle_store_publish(
        &mut self,
        address: &str,
        slot: &str,
    ) -> Result<Option<HandleStorePublish>, String> {
        let publish = if let Some(object) = self.ctx.model.objects.iter().find(|object| {
            object.ty == IrObjectType::Chandle && slot == format!("&{}", object.c_name)
        }) {
            format!("llg_dependency_changed(&{}_llg_dep);", object.c_name)
        } else {
            let Some(name) = address.strip_prefix('&') else {
                return Ok(None);
            };
            let Some(access) = self
                .ctx
                .model
                .native_accesses
                .iter()
                .find(|access| access.name == name)
                .cloned()
            else {
                return Ok(None);
            };
            let IrNativeAccessKind::ClassField { class, field } = access.kind else {
                return Ok(None);
            };
            let layout = &self.ctx.model.classes[class].fields[field];
            if layout.ty != IrClassFieldType::Chandle
                || layout.container.is_some()
                || layout.native_value.is_some()
            {
                return Ok(None);
            }
            // The store itself already reported a null receiver.
            let previous = std::mem::replace(&mut self.quiet_receivers, true);
            let receiver = self.chandle(&access.receiver);
            self.quiet_receivers = previous;
            let receiver = self.scalar("void*", receiver?);
            format!("llg_class_handle_published({receiver});")
        };
        let previous = self.scalar("void*", format!("*(void**)({slot})"));
        Ok(Some(HandleStorePublish {
            previous,
            publish,
            slot: slot.to_owned(),
        }))
    }

    /// Publish a store captured by [`Self::handle_store_publish`] if it
    /// changed the handle.
    pub(super) fn finish_handle_store_publish(&mut self, publish: Option<HandleStorePublish>) {
        if let Some(publish) = publish {
            self.line(format!(
                "if (*(void**)({}) != {}) {{ {} }}",
                publish.slot, publish.previous, publish.publish
            ));
        }
    }
}
