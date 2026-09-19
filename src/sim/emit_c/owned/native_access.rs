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
                let member = match ty {
                    IrClassFieldType::Packed { .. } => "packed",
                    IrClassFieldType::Real { .. } => "real",
                    IrClassFieldType::String => "string",
                    IrClassFieldType::Chandle => "handle",
                };
                (
                    format!("&llg_class_field({receiver}, {class}, {field})->value.{member}"),
                    ty,
                )
            }
            IrNativeAccessKind::InterfaceMember { interface, member } => {
                let metadata = &self.ctx.model.virtual_interfaces[interface].members[member];
                let site = access.site.as_deref().unwrap_or(name);
                (
                    format!(
                        "llg_vif_member({receiver}, {interface}, {member}, {})",
                        c_string_literal(site)
                    ),
                    IrClassFieldType::Packed {
                        width: metadata.width,
                        signed: metadata.signed,
                        two_state: metadata.two_state,
                    },
                )
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
