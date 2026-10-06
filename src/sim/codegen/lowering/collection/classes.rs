//! Classes.

use super::*;

impl<'a> Codegen<'a> {
    pub(super) fn collect_class_funcs(&mut self, class: NodeId) -> Result<(), String> {
        let class_index = self
            .class_nodes
            .get(&class)
            .copied()
            .ok_or_else(|| format!("class `{}` has no layout", self.node(class).name))?;
        for child in self.class_method_nodes(class) {
            let name = self.node(child).name.clone();
            self.func_names.insert(
                child,
                format!("fn_class_{class_index}_{}", function_ident(&name)),
            );
            self.func_labels
                .insert(child, format!("{}::{name}", self.node(class).name));
        }
        Ok(())
    }

    /// Return executable class subroutines, unwrapping Slang's method
    /// prototype nodes. A prototype owns a synthetic subroutine child; the
    /// prototype itself has no body and must not become a C function.
    pub(in super::super) fn class_method_nodes(&self, class: NodeId) -> Vec<NodeId> {
        fn visit(cg: &Codegen<'_>, node: NodeId, methods: &mut Vec<NodeId>) {
            let NodeKind::FuncTask { .. } = cg.kind(node) else {
                return;
            };
            if cg.db.subroutine_body(node).is_some() {
                methods.push(node);
            } else {
                for child in cg.node(node).children.iter().copied() {
                    visit(cg, child, methods);
                }
            }
        }

        let mut methods = Vec::new();
        for child in self.node(class).children.iter().copied() {
            visit(self, child, &mut methods);
        }
        methods
    }

    fn method_signature_key(&self, method: NodeId) -> String {
        let mut key = self.node(method).name.clone();
        key.push('#');
        for argument in self
            .node(method)
            .children
            .iter()
            .copied()
            .filter(|child| matches!(self.kind(*child), NodeKind::FuncArg { .. }))
        {
            if let NodeKind::FuncArg { ty, direction, .. } = self.kind(argument) {
                key.push_str(&format!(
                    "{:?}:{}:{}:{:?};",
                    direction,
                    ty.kind,
                    ty.width.unwrap_or_default(),
                    ty.signed
                ));
            }
        }
        key
    }

    pub(super) fn assign_class_virtual_slots(
        &mut self,
        class: NodeId,
        all_slots: &mut HashMap<NodeId, HashMap<String, usize>>,
        next: &mut usize,
    ) {
        if all_slots.contains_key(&class) {
            return;
        }
        let mut slots = self
            .db
            .class_metadata(class)
            .and_then(|metadata| metadata.base)
            .map(|base| {
                self.assign_class_virtual_slots(base, all_slots, next);
                all_slots.get(&base).cloned().unwrap_or_default()
            })
            .unwrap_or_default();
        for child in self.class_method_nodes(class) {
            let is_virtual = matches!(
                self.kind(child),
                NodeKind::FuncTask {
                    is_virtual: true,
                    ..
                }
            );
            if !is_virtual {
                continue;
            }
            let key = self.method_signature_key(child);
            let slot = slots.entry(key).or_insert_with(|| {
                let slot = *next;
                *next += 1;
                slot
            });
            self.method_virtual_slots.insert(child, *slot);
        }
        all_slots.insert(class, slots);
    }

    pub(in super::super) fn emit_class_func_prototypes(&mut self) -> Result<(), String> {
        for class in self.db.classes().to_vec() {
            self.emit_func_prototypes(class)?;
        }
        Ok(())
    }

    pub(in super::super) fn emit_class_func_bodies(&mut self) -> Result<(), String> {
        for class in self.db.classes().to_vec() {
            self.emit_func_bodies(class)?;
        }
        Ok(())
    }

    fn class_field_type(
        &self,
        field: NodeId,
        ty: &crate::core::model::TypeInfo,
    ) -> Result<IrClassFieldType, String> {
        if is_real_kind(&ty.kind) {
            return Ok(IrClassFieldType::Real {
                shortreal: ty.kind == "shortreal",
            });
        }
        if ty.kind == "string" {
            return Ok(IrClassFieldType::String);
        }
        if is_handle_kind(&ty.kind) {
            return Ok(IrClassFieldType::Chandle);
        }
        // An unpacked record or union of integral leaves is one fixed-width
        // image; member selections project into it (SIM-011).
        if let Some(descriptor) = self.query_descriptor(field).filter(|descriptor| {
            matches!(&descriptor.shape, TypeShape::Aggregate(layout)
                if matches!(layout.kind, AggregateKind::UnpackedStruct | AggregateKind::UnpackedUnion))
        }) {
            let width = super::fixed_values::fixed_width(descriptor).ok_or_else(|| {
                if Self::fixed_descriptor_width_bits(descriptor).is_none() {
                    format!(
                        "class property `{}` of an unpacked record or union type with string, real, handle or container members is not supported (SIM-011)",
                        self.node(field).full_name
                    )
                } else {
                    format!(
                        "class property `{}` of an unpacked record or union type wider than packed capacity is not supported (SIM-011)",
                        self.node(field).full_name
                    )
                }
            })?;
            return Ok(IrClassFieldType::Packed {
                width,
                signed: false,
                two_state: super::fixed_values::two_state(descriptor),
            });
        }
        let width = ty.width.ok_or_else(|| {
            // A record or tagged union with string, real, handle or
            // container leaves has no packed payload, and class fields hold
            // only scalar leaves.
            if self.query_descriptor(field).is_some_and(|descriptor| {
                matches!(&descriptor.shape, TypeShape::Aggregate(layout)
                    if !matches!(layout.kind, AggregateKind::PackedStruct | AggregateKind::PackedUnion))
            }) {
                return format!(
                    "class property `{}` of an unpacked record or union type with string, real, handle or container members is not supported",
                    self.node(field).full_name
                );
            }
            format!(
                "class property `{}` has no resolved packed width",
                self.node(field).full_name
            )
        })?;
        if width == 0 || width > LLG_MAX_WIDTH {
            return Err(format!(
                "class property `{}` has unsupported width {width}",
                self.node(field).full_name
            ));
        }
        Ok(IrClassFieldType::Packed {
            width,
            signed: ty.signed,
            two_state: self.db.is_two_state_type(field) || is_two_state_kind(&ty.kind),
        })
    }

    /// Capture nominal class layouts and allocate shared static properties.
    /// Instance properties live in the emitted heap object and are mapped by
    /// declaration identity, so same-named classes/properties cannot alias.
    pub(super) fn collect_classes(&mut self) -> Result<(), String> {
        let classes = self.db.classes().to_vec();
        // Allocate every nominal index before resolving bases.  Generic
        // specializations can be visited in an order that differs from their
        // source declaration, so a one-pass map would lose a base edge.
        for (class_index, class) in classes.iter().copied().enumerate() {
            self.class_nodes.insert(class, class_index);
        }
        for class in classes {
            let class_index = self.class_nodes[&class];
            let mut fields = Vec::new();
            for child in self.node(class).children.clone() {
                // Resizable containers and fixed unpacked arrays (fixed-size
                // views of the container runtime, SIM-011) are per-object
                // container storage.
                if let Some(meta) = self.db.array_meta(child).cloned() {
                    self.collect_class_container(class_index, child, &meta, &mut fields)?;
                    continue;
                }
                let NodeKind::Var { ty } = self.kind(child) else {
                    continue;
                };
                // A record with string, real or handle leaves is one native
                // value per object (SIM-011).
                if self.native_value_type(child).is_some() {
                    self.collect_class_native(class_index, child, &mut fields)?;
                    continue;
                }
                let field_ty = self.class_field_type(child, ty)?;
                if self.db.variable_lifetime(child) == VariableLifetime::Static {
                    match field_ty {
                        IrClassFieldType::Packed {
                            width,
                            signed,
                            two_state,
                        } => {
                            let ir = self.model.signals.len();
                            let info = SignalInfo {
                                global: format!(
                                    "G_class_{class_index}_{}",
                                    ident(&self.node(child).name)
                                ),
                                width,
                                signed,
                                two_state,
                                real: false,
                                shortreal: false,
                                net_driver: None,
                                ir,
                            };
                            self.model.signals.push(IrSignal {
                                fixed_default: None,
                                c_name: info.global.clone(),
                                hdl_name: None,
                                ty: IrType::Packed {
                                    width,
                                    signed,
                                    two_state,
                                },
                                net_driver: None,
                                net_alias: Vec::new(),
                                alias: None,
                                omit: false,
                            });
                            self.signals.push(info.clone());
                            self.class_static_signals.insert(child, info.clone());
                            if let Some(initializer) = self.db.var_initializer(child) {
                                let value = self.const_of_node(initializer).map_err(|error| {
                                    format!(
                                        "static class property `{}` initializer: {error}",
                                        self.node(child).full_name
                                    )
                                })?;
                                self.var_inits.push((info, value));
                            }
                        }
                        IrClassFieldType::Real { shortreal } => {
                            let ir = self.model.signals.len();
                            let info = SignalInfo {
                                global: format!(
                                    "G_class_{class_index}_{}",
                                    ident(&self.node(child).name)
                                ),
                                width: 0,
                                signed: false,
                                two_state: false,
                                real: true,
                                shortreal,
                                net_driver: None,
                                ir,
                            };
                            self.model.signals.push(IrSignal {
                                fixed_default: None,
                                c_name: info.global.clone(),
                                hdl_name: None,
                                ty: IrType::Real { shortreal },
                                net_driver: None,
                                net_alias: Vec::new(),
                                alias: None,
                                omit: false,
                            });
                            self.signals.push(info.clone());
                            self.class_static_signals.insert(child, info.clone());
                            if let Some(initializer) = self.db.var_initializer(child) {
                                let value = self.const_of_node(initializer).map_err(|error| {
                                    format!(
                                        "static class property `{}` initializer: {error}",
                                        self.node(child).full_name
                                    )
                                })?;
                                self.var_inits.push((info, value));
                            }
                        }
                        IrClassFieldType::String => {
                            let index = self.model.objects.len();
                            let initial = self
                                .db
                                .var_initializer(child)
                                .map(|initializer| self.lower_string("$class", initializer))
                                .transpose()?;
                            self.model.objects.push(crate::sim::ir::IrObject {
                                c_name: format!(
                                    "O_class_{class_index}_{}",
                                    ident(&self.node(child).name)
                                ),
                                ty: IrObjectType::String,
                                initial,
                            });
                            self.class_static_objects.insert(child, index);
                        }
                        IrClassFieldType::Chandle => {
                            let index = self.model.objects.len();
                            if let Some(initializer) = self.db.var_initializer(child) {
                                if !matches!(
                                    self.kind(initializer),
                                    NodeKind::Expr(ExprKind::Constant {
                                        const_type: ConstantType::Null,
                                        ..
                                    })
                                ) {
                                    return Err(format!(
                                        "static class handle property `{}` must initialize to null",
                                        self.node(child).full_name
                                    ));
                                }
                            }
                            self.model.objects.push(crate::sim::ir::IrObject {
                                c_name: format!(
                                    "O_class_{class_index}_{}",
                                    ident(&self.node(child).name)
                                ),
                                ty: IrObjectType::Chandle,
                                initial: None,
                            });
                            self.class_static_objects.insert(child, index);
                        }
                    }
                    continue;
                }
                let field_index = fields.len();
                fields.push(IrClassField {
                    c_name: format!(
                        "f_{class_index}_{field_index}_{}",
                        ident(&self.node(child).name)
                    ),
                    ty: field_ty,
                    container: None,
                    native_value: None,
                });
                self.class_fields.insert(child, (class_index, field_index));
            }
            self.model.classes.push(IrClass {
                c_name: format!("llg_class_{class_index}"),
                base: self
                    .db
                    .class_metadata(class)
                    .and_then(|metadata| metadata.base)
                    .and_then(|base| self.class_nodes.get(&base).copied()),
                fields,
            });
        }
        // Derived objects use a flattened prefix-compatible layout.  Process
        // bases before derived classes even if generic specialization capture
        // ordered the semantic class nodes the other way around. Keep field
        // declaration identities mapped to their final index so base
        // references, hidden members, and downcasts all select the right C
        // member without a second object allocation.
        let mut pending = self.db.classes().to_vec();
        let mut flattened = HashSet::new();
        while !pending.is_empty() {
            let before = pending.len();
            pending.retain(|class| {
                let class_index = self.class_nodes[class];
                let Some(base_index) = self.model.classes[class_index].base else {
                    flattened.insert(class_index);
                    return false;
                };
                if !flattened.contains(&base_index) {
                    return true;
                }
                let inherited = self.model.classes[base_index].fields.clone();
                let own_len = self.model.classes[class_index].fields.len();
                for (owner, index) in self.class_fields.values_mut() {
                    if *owner == class_index {
                        *index += inherited.len();
                    }
                }
                for container in &mut self.model.containers {
                    if let Some((owner, index)) = container.class_field.as_mut() {
                        if *owner == class_index {
                            *index += inherited.len();
                        }
                    }
                }
                for value in &mut self.model.native_values {
                    if let Some((owner, index)) = value.class_field.as_mut() {
                        if *owner == class_index {
                            *index += inherited.len();
                        }
                    }
                }
                let mut fields = inherited;
                fields.extend(
                    self.model.classes[class_index].fields[..own_len]
                        .iter()
                        .cloned(),
                );
                self.model.classes[class_index].fields = fields;
                flattened.insert(class_index);
                false
            });
            if pending.len() == before {
                return Err("class inheritance layout contains a cycle".to_owned());
            }
        }
        Ok(())
    }

    /// A record class property with string, real or handle leaves (SIM-011):
    /// one descriptor-backed native value per object, created with the
    /// object and addressed through `this` or an explicit handle.
    fn collect_class_native(
        &mut self,
        class_index: usize,
        child: NodeId,
        fields: &mut Vec<IrClassField>,
    ) -> Result<(), String> {
        let name = self.node(child).full_name.clone();
        if self.db.variable_lifetime(child) == VariableLifetime::Static {
            return Err(format!(
                "static class property `{name}` of an unpacked record or union type with string, real, handle or container members is not supported (SIM-011)"
            ));
        }
        let layout = self
            .native_layout(child)?
            .ok_or_else(|| format!("class property `{name}` has no native record layout"))?;
        if !layout.containers.is_empty() {
            return Err(format!(
                "class property `{name}` of a record type with queue, dynamic or associative members is not supported (SIM-011)"
            ));
        }
        let field_index = fields.len();
        let value = self.model.native_values.len();
        self.model
            .native_values
            .push(crate::sim::ir::IrNativeValue {
                c_name: format!("S_llg_class_native_{class_index}_{field_index}"),
                ty: layout.ty,
                activation: false,
                companions: Vec::new(),
                class_field: Some((class_index, field_index)),
                receiver: None,
            });
        self.native_value_layouts.insert(value, child);
        self.class_native_fields.insert(child, value);
        fields.push(IrClassField {
            c_name: format!(
                "f_{class_index}_{field_index}_{}",
                ident(&self.node(child).name)
            ),
            ty: IrClassFieldType::Chandle,
            container: None,
            native_value: Some(value),
        });
        Ok(())
    }

    /// A resizable-container class property (SV 8.5, 7.5-7.10). A static
    /// property is one model-global container; an instance property is one
    /// container per object, created with the object and addressed through
    /// the receiver of the class's own methods.
    fn collect_class_container(
        &mut self,
        class_index: usize,
        child: NodeId,
        meta: &crate::core::db::ArrayMeta,
        fields: &mut Vec<IrClassField>,
    ) -> Result<(), String> {
        let name = self.node(child).name.clone();
        let descriptor = self
            .query_descriptor(child)
            .ok_or_else(|| format!("class property `{name}` has no recursive type descriptor"))?;
        let element = match &descriptor.shape {
            TypeShape::Container { element, .. } => element,
            TypeShape::FixedArray {
                dimensions,
                element,
            } => {
                if dimensions.len() != 1 {
                    return Err(format!(
                        "multidimensional fixed array class property `{}` is not supported (SIM-011)",
                        self.node(child).full_name
                    ));
                }
                element
            }
            _ => {
                return Err(format!(
                    "class property `{name}` has a non-container type descriptor"
                ))
            }
        };
        let element = lower_container_element(element)?;
        let path = format!("llg_class{class_index}");
        let info = self.container_from_meta(&path, &name, child, meta, element)?;
        if self.db.variable_lifetime(child) != VariableLifetime::Static {
            // The container initializer list is for model storage; an
            // instance property initializes per object at construction.
            if meta.initializer().is_some() {
                self.container_initializers
                    .retain(|(owner, _)| *owner != child);
            }
            let field_index = fields.len();
            self.model.containers[info.ir].class_field = Some((class_index, field_index));
            self.model.containers[info.ir].c_name =
                format!("S_llg_class_container_{class_index}_{field_index}");
            fields.push(IrClassField {
                c_name: format!("f_{class_index}_{field_index}_{}", ident(&name)),
                ty: IrClassFieldType::Chandle,
                container: Some(info.ir),
                native_value: None,
            });
        }
        self.container_globals.insert(child, info);
        Ok(())
    }
}
