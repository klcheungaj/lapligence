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

    /// Index the methods of every virtual slot, in class order, once all
    /// slots are assigned.
    pub(super) fn index_virtual_methods(&mut self) {
        for class in self.db.classes() {
            for method in self.class_method_nodes(*class) {
                if let Some(slot) = self.method_virtual_slots.get(&method) {
                    self.virtual_slot_methods
                        .entry(*slot)
                        .or_default()
                        .push((method, *class));
                }
            }
        }
    }

    /// The methods that a virtual call of `method` may run, with their
    /// classes; empty when the method is not virtual.
    pub(in super::super) fn virtual_family(&self, method: NodeId) -> &[(NodeId, NodeId)] {
        self.method_virtual_slots
            .get(&method)
            .and_then(|slot| self.virtual_slot_methods.get(slot))
            .map_or(&[], Vec::as_slice)
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

    /// A static class property (SV 8.9): one variable, with the storage of a
    /// package variable, that the class and every object of it share. A dump
    /// shows no class, so it has no waveform identity.
    fn collect_static_property(&mut self, class_index: usize, child: NodeId) -> Result<(), String> {
        let path = Self::static_property_path(class_index);
        let first_signal = self.model.signals.len();
        let first_array = self.model.arrays.len();
        if matches!(self.kind(child), NodeKind::Var { ty } if ty.kind == "event") {
            let global = self.event_global_name(&path, &self.node(child).name);
            let class = self.node(child).parent;
            let scope =
                class.map_or_else(String::new, |class| self.node(class).full_name().to_owned());
            self.collect_event_global(&scope, child, global);
            return Ok(());
        }
        // A fixed array of events or containers has no array storage; it is a
        // fixed-size view of the container runtime, like one of an object.
        match self.db.array_meta(child).cloned() {
            Some(meta) if self.is_fixed_array_of_containers(child) => {
                let view = self.class_container_view(class_index, child, &meta)?;
                self.container_globals.insert(child, view);
            }
            _ => {
                self.collect_variable(&path, child, &mut HashSet::new())?;
            }
        }
        for signal in &mut self.model.signals[first_signal..] {
            signal.hdl_name = None;
        }
        for array in &mut self.model.arrays[first_array..] {
            array.hdl_name.clear();
        }
        if self.db.var_initializer(child).is_some() {
            self.reserve_initializer_order(child);
        }
        Ok(())
    }

    /// Whether `property` is a fixed array of events or resizable containers.
    fn is_fixed_array_of_containers(&self, property: NodeId) -> bool {
        self.query_descriptor(property).is_some_and(|descriptor| {
            matches!(&descriptor.shape, TypeShape::FixedArray { element, .. }
                if matches!(element.shape, TypeShape::Container { .. })
                    || lower_container_element(element).is_ok_and(|element| element.is_event()))
        })
    }

    /// The storage scope of the static properties of class `class_index`.
    fn static_property_path(class_index: usize) -> String {
        format!("llg_class{class_index}")
    }

    /// The declaration initializers of the static class properties, which
    /// run in the static schedule before every process (SV 6.8). They may
    /// read any class, package, module or parameter, so what does not fold is
    /// lowered once every declaration is collected.
    pub(super) fn collect_static_property_inits(&mut self) -> Result<(), String> {
        for class in self.db.classes().to_vec() {
            let path = Self::static_property_path(self.class_nodes[&class]);
            for child in self.node(class).children.clone() {
                if !matches!(self.kind(child), NodeKind::Var { .. })
                    || self.db.variable_lifetime(child) != VariableLifetime::Static
                {
                    continue;
                }
                let Some(initializer) = self.db.var_initializer(child) else {
                    continue;
                };
                if let Some(aggregate) = self.unpacked_aggregates.get(&child).cloned() {
                    if self.reads_uncollected_storage(initializer) {
                        self.record_statement_initializers
                            .push((child, initializer, path.clone()));
                    } else {
                        self.collect_unpacked_aggregate_decl_init(
                            &path,
                            child,
                            initializer,
                            &aggregate,
                        )?;
                    }
                } else if let Some(info) = self.signal_of(child).cloned() {
                    match self.const_of_node(initializer) {
                        Ok(value) => self.var_inits.push((info, value)),
                        Err(_) => {
                            self.deferred_declaration_inits
                                .push((child, initializer, class, info))
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// An instance `event` property (SV 15.5): a per-object handle slot that
    /// construction points at a fresh synchronization object.
    fn collect_class_event(
        &mut self,
        class_index: usize,
        child: NodeId,
        fields: &mut Vec<IrClassField>,
    ) {
        let field_index = fields.len();
        fields.push(IrClassField {
            c_name: format!(
                "f_{class_index}_{field_index}_{}",
                ident(&self.node(child).name)
            ),
            ty: IrClassFieldType::Chandle,
            container: None,
            native_value: None,
            event: true,
        });
        self.class_fields.insert(child, (class_index, field_index));
    }

    /// Number the classes for keys of class-indexed arrays (S07-D9): classes
    /// of compilation-unit scope, then of packages, then of instances, each
    /// in declaration order; instances in post-order of the hierarchy. All
    /// specializations of a generic class share its position. Nothing here
    /// depends on a run, so neither does the order of keys.
    pub(in crate::sim::codegen) fn rank_class_keys(&mut self) {
        fn holds(element: &IrContainerElement) -> bool {
            match element {
                IrContainerElement::Container { key, element, .. } => {
                    matches!(key, Some(IrAssocKey::Handle { object: true })) || holds(element)
                }
                IrContainerElement::FixedArray { element, .. } => holds(element),
                IrContainerElement::Aggregate { members, .. }
                | IrContainerElement::Union { members, .. } => {
                    members.iter().any(|member| holds(&member.element))
                }
                _ => false,
            }
        }
        let used = self.model.containers.iter().any(|container| {
            matches!(
                container.kind,
                IrContainerKind::Associative {
                    key: IrAssocKey::Handle { object: true }
                }
            ) || holds(&container.element)
        });
        if !used {
            return;
        }
        fn post_order(db: &Db, node: NodeId, order: &mut HashMap<NodeId, usize>) {
            for child in &db.node(node).children {
                if !matches!(db.node(*child).kind, NodeKind::ClassDef) {
                    post_order(db, *child, order);
                }
            }
            if matches!(db.node(node).kind, NodeKind::ModuleInst { .. }) {
                let next = order.len();
                order.insert(node, next);
            }
        }
        let mut instances = HashMap::new();
        for top in self.db.tops() {
            post_order(self.db, *top, &mut instances);
        }
        let mut files: HashMap<std::sync::Arc<str>, usize> = HashMap::new();
        let packages: HashMap<NodeId, usize> = self
            .db
            .packages()
            .iter()
            .enumerate()
            .map(|(index, package)| (*package, index))
            .collect();
        let mut keyed: Vec<_> = self
            .db
            .classes()
            .iter()
            .map(|class| {
                let scope = self
                    .db
                    .class_metadata(*class)
                    .and_then(|metadata| metadata.scope);
                let owner = match scope {
                    Some(scope) => match packages.get(&scope) {
                        Some(position) => (1, *position),
                        None => (2, instances.get(&scope).copied().unwrap_or(usize::MAX)),
                    },
                    None => (0, 0),
                };
                let node = self.node(*class);
                let next = files.len();
                let file = node
                    .file
                    .as_ref()
                    .map(|file| *files.entry(file.clone()).or_insert(next));
                ((owner, file, node.line, node.col), self.class_nodes[class])
            })
            .collect();
        keyed.sort_by_key(|entry| entry.0);
        for (rank, (_, class)) in keyed.into_iter().enumerate() {
            self.model.classes[class].rank = rank as u32 + 1;
        }
        // Covergroup objects never meet a key; they only need a rank.
        let last = self.model.classes.len() as u32;
        for class in self
            .model
            .classes
            .iter_mut()
            .filter(|class| class.rank == 0)
        {
            class.rank = last;
        }
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
                if self.db.variable_lifetime(child) == VariableLifetime::Static
                    && matches!(
                        self.kind(child),
                        NodeKind::Var { .. } | NodeKind::Array { .. }
                    )
                {
                    self.collect_static_property(class_index, child)?;
                    continue;
                }
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
                if ty.kind == "event" {
                    self.collect_class_event(class_index, child, &mut fields);
                    continue;
                }
                let field_ty = self.class_field_type(child, ty)?;
                let field_index = fields.len();
                fields.push(IrClassField {
                    c_name: format!(
                        "f_{class_index}_{field_index}_{}",
                        ident(&self.node(child).name)
                    ),
                    ty: field_ty,
                    container: None,
                    native_value: None,
                    event: false,
                });
                self.class_fields.insert(child, (class_index, field_index));
            }
            self.allocate_random_modes(class, class_index, &mut fields);
            self.model.classes.push(IrClass {
                c_name: format!("llg_class_{class_index}"),
                base: self
                    .db
                    .class_metadata(class)
                    .and_then(|metadata| metadata.base)
                    .and_then(|base| self.class_nodes.get(&base).copied()),
                fields,
                rank: 0,
            });
        }
        self.share_constraint_modes();
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
                for (owner, index) in self.class_fields.values_mut_unordered() {
                    if *owner == class_index {
                        *index += inherited.len();
                    }
                }
                for bit in self
                    .rand_mode_bits
                    .values_mut()
                    .chain(self.constraint_mode_bits.values_mut())
                    .chain(self.randc_states.values_mut().map(|(bit, _)| bit))
                {
                    if let super::super::randomization::ModeBit::Field(owner, index) = bit {
                        if *owner == class_index {
                            *index += inherited.len();
                        }
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
        let layout = self
            .native_layout(child)?
            .ok_or_else(|| format!("class property `{name}` has no native record layout"))?;
        if !layout.containers.is_empty() {
            return Err(self.locate_error(child, format!(
                "class property `{name}` of a record type with queue, dynamic or associative members is not supported (SIM-011)"
            )));
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
                marker: false,
                ref_rooted: false,
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
            event: false,
        });
        Ok(())
    }

    /// An instance container class property (SV 8.5, 7.5-7.10): one container
    /// per object, created with the object and addressed through the receiver
    /// of the class's own methods.
    fn collect_class_container(
        &mut self,
        class_index: usize,
        child: NodeId,
        meta: &crate::core::db::ArrayMeta,
        fields: &mut Vec<IrClassField>,
    ) -> Result<(), String> {
        let info = self.class_container_view(class_index, child, meta)?;
        // The container initializer list is for model storage; an instance
        // property initializes per object at construction.
        if meta.initializer().is_some() {
            self.container_initializers
                .retain(|(owner, _)| *owner != child);
        }
        let field_index = fields.len();
        self.model.containers[info.ir].class_field = Some((class_index, field_index));
        self.model.containers[info.ir].c_name =
            format!("S_llg_class_container_{class_index}_{field_index}");
        fields.push(IrClassField {
            c_name: format!(
                "f_{class_index}_{field_index}_{}",
                ident(&self.node(child).name)
            ),
            ty: IrClassFieldType::Chandle,
            container: Some(info.ir),
            native_value: None,
            event: false,
        });
        self.container_globals.insert(child, info);
        Ok(())
    }

    /// The container storage of the container or fixed array property `child`.
    fn class_container_view(
        &mut self,
        class_index: usize,
        child: NodeId,
        meta: &crate::core::db::ArrayMeta,
    ) -> Result<ContainerInfo, String> {
        let name = self.node(child).name.clone();
        let descriptor = self
            .query_descriptor(child)
            .ok_or_else(|| format!("class property `{name}` has no recursive type descriptor"))?;
        let mut multidimensional = None;
        let element = match &descriptor.shape {
            TypeShape::Container { element, .. } => element,
            TypeShape::FixedArray {
                dimensions,
                element,
            } => {
                if dimensions.len() != 1 {
                    // A multidimensional property of packed words keeps all
                    // its cells in one fixed-size container, in declaration
                    // (row-major) order, as `%p` and memory files read them.
                    if !matches!(
                        lower_container_element(element)?,
                        IrContainerElement::Packed { .. }
                    ) {
                        return Err(self.locate_error(
                            child,
                            format!(
                                "multidimensional fixed array class property `{}` needs packed elements (SIM-011)",
                                self.node(child).full_name
                            ),
                        ));
                    }
                    multidimensional = Some(dimensions.clone());
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
        let path = Self::static_property_path(class_index);
        let info = self.container_from_meta(&path, &name, child, meta, element)?;
        if let Some(dimensions) = multidimensional {
            self.fixed_view_dims.insert(info.ir, dimensions);
        }
        Ok(info)
    }
}
