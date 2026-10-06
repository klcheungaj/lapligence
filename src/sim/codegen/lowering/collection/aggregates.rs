//! Aggregates.

use super::*;

/// Path name of the tag leaf of a tagged union with native members. `$` cannot
/// start a SystemVerilog identifier, so no member can share it.
pub(in super::super) const NATIVE_TAG_MEMBER: &str = "$tag";

/// The synthetic packed member describing a native tagged union's tag leaf.
fn native_tag_member(tag_bits: u32, union: &TypeDescriptor) -> AggregateMember {
    let ty = crate::core::model::TypeInfo {
        kind: "logic".to_owned(),
        width: Some(tag_bits),
        signed: false,
        type_name: None,
    };
    let range = crate::core::db::PackedRange {
        left: i128::from(tag_bits) - 1,
        right: 0,
    };
    AggregateMember {
        initializer: None,
        name: NATIVE_TAG_MEMBER.to_owned(),
        ty: ty.clone(),
        two_state: false,
        packed_ranges: vec![range],
        aggregate: None,
        descriptor: TypeDescriptor {
            id: union.id,
            two_state: false,
            name: format!("{}::tag", union.name),
            info: ty,
            shape: TypeShape::PackedAtom {
                ranges: vec![range],
            },
        },
    }
}

impl<'a> Codegen<'a> {
    /// Packed width of a struct/union formal. These keep the
    /// existing scalar `sv4_t` ABI, but their member selects must resolve to
    /// the formal's activation storage rather than a module signal.
    pub(super) fn packed_formal_width(&self, io: NodeId) -> Option<u32> {
        let layout = self.db.aggregate_layout(io)?;
        match layout.kind {
            AggregateKind::PackedStruct => layout
                .members
                .iter()
                .try_fold(0u32, |sum, member| sum.checked_add(member.ty.width?)),
            AggregateKind::PackedUnion => layout
                .members
                .iter()
                .filter_map(|member| member.ty.width)
                .max(),
            AggregateKind::TaggedUnion => layout.payload_bits()?.checked_add(layout.tag_bits()?),
            _ => None,
        }
    }

    pub(super) fn collect_aggregate(&mut self, path: &str, node: NodeId) -> Result<bool, String> {
        let Some(layout) = self.db.aggregate_layout(node).cloned() else {
            return Ok(false);
        };
        // A declaration can be reached through both its instance child list
        // and a reference-port connection.  Its leaf storage is owned by the
        // declaration, so collection must be idempotent before allocating
        // recursive object/signal leaves.
        if self.unpacked_aggregates.contains_key(&node) {
            return Ok(true);
        }
        match layout.kind {
            AggregateKind::PackedStruct => return Ok(false),
            AggregateKind::PackedUnion => {
                let width = layout.members.first().and_then(|member| member.ty.width);
                if width.is_none() || layout.members.iter().any(|member| member.ty.width != width) {
                    return Err(format!(
                        "packed untagged union `{}` in `{path}` has members of unequal or unresolved width",
                        self.node(node).name
                    ));
                }
                return Ok(false);
            }
            AggregateKind::TaggedUnion => {
                if let Some(descriptor) = self
                    .query_descriptor(node)
                    .filter(|descriptor| super::record_columns::column_tagged_union(descriptor))
                    .cloned()
                {
                    if !matches!(self.kind(node), NodeKind::Var { .. }) {
                        if matches!(
                            self.kind(node),
                            NodeKind::Net { .. }
                                | NodeKind::Array { .. }
                                | NodeKind::Port { .. }
                                | NodeKind::IoDecl { .. }
                        ) {
                            return Err(format!(
                                "tagged union `{}` in `{path}` beyond packed capacity must be a variable",
                                self.node(node).name
                            ));
                        }
                        // Type declarations allocate no storage.
                        return Ok(false);
                    }
                    // Beyond packed capacity, the tag is one cell and each
                    // member keeps its own columns (RTL-101).
                    let object_name = self.node(node).name.clone();
                    let root = AggregateMember {
                        initializer: None,
                        name: object_name.clone(),
                        ty: descriptor.info.clone(),
                        two_state: descriptor.two_state,
                        packed_ranges: Vec::new(),
                        aggregate: None,
                        descriptor: descriptor.clone(),
                    };
                    let mut leaves = Vec::new();
                    self.collect_tagged_columns(
                        path,
                        node,
                        &object_name,
                        &root,
                        &descriptor,
                        &[],
                        &mut leaves,
                    )?;
                    let members = layout
                        .members
                        .iter()
                        .map(|member| AggregateMemberInfo {
                            member: member.clone(),
                            signal: None,
                            object: None,
                            array: None,
                            container: None,
                            path: vec![AggregatePathPart::Member(member.name.clone())],
                        })
                        .collect();
                    self.unpacked_aggregates.insert(
                        node,
                        UnpackedAggregateInfo {
                            kind: layout.kind,
                            type_identity: layout.type_identity,
                            columns: true,
                            members,
                            leaves,
                        },
                    );
                    return Ok(true);
                }
                if self
                    .query_descriptor(node)
                    .is_some_and(|descriptor| Self::fixed_descriptor_width(descriptor).is_some())
                {
                    // A finite tagged packed union uses the ordinary packed
                    // scalar storage path. Its tag occupies the most
                    // significant bits; member access resolves the payload
                    // from the same owned layout.
                    return Ok(false);
                }
                // A union with a string, real or handle member keeps its tag
                // and every member in separate leaf storage (SIM-007); member
                // accesses check the tag like a packed tagged union.
                if let Some(descriptor) = self
                    .query_descriptor(node)
                    .filter(|descriptor| Self::fixed_descriptor_width_bits(descriptor).is_none())
                    .cloned()
                {
                    if matches!(self.kind(node), NodeKind::Var { .. }) {
                        self.collect_native_tagged_union(path, node, &layout, &descriptor)?;
                        return Ok(true);
                    }
                    // Resizable and fixed arrays of such unions are container
                    // storage whose elements own their tag and members.
                    if matches!(self.kind(node), NodeKind::Array { .. })
                        && (self
                            .db
                            .array_meta(node)
                            .is_some_and(|meta| !matches!(meta.kind(), ArrayKind::Static))
                            || self.is_fixed_handle_array(node))
                    {
                        return Ok(false);
                    }
                    // A variable port's storage is its variable, as for
                    // records; port links copy the tag and member leaves.
                    if matches!(
                        self.kind(node),
                        NodeKind::IoDecl { .. }
                            | NodeKind::Port {
                                direction: DbDirection::Input | DbDirection::Output,
                                ..
                            }
                    ) {
                        return Ok(false);
                    }
                    if matches!(
                        self.kind(node),
                        NodeKind::Net { .. } | NodeKind::Array { .. } | NodeKind::FuncArg { .. }
                    ) {
                        return Err(format!(
                            "tagged union `{}` in `{path}` with string, real or handle members must be a variable",
                            self.node(node).name
                        ));
                    }
                    // Type declarations allocate no storage.
                    return Ok(false);
                }
                // A finite payload beyond packed capacity has no descriptor
                // record transport yet; reject it instead of flattening.
                if let Some(width) = self
                    .query_descriptor(node)
                    .and_then(Self::fixed_descriptor_width_bits)
                    .filter(|width| *width > u64::from(LLG_MAX_WIDTH))
                {
                    return Err(Self::fixed_descriptor_capacity_error(
                        &format!("tagged union `{}` in `{path}`", self.node(node).name),
                        width,
                    ));
                }
                return Err(format!(
                    "tagged union `{}` in `{path}` requires a fixed packed representation",
                    self.node(node).name
                ));
            }
            AggregateKind::UnpackedStruct | AggregateKind::UnpackedUnion => {}
        }
        if matches!(self.kind(node), NodeKind::Array { .. })
            && self.query_descriptor(node).is_some_and(|descriptor| matches!(&descriptor.shape,
                TypeShape::FixedArray { element, .. } if Self::fixed_descriptor_width(element).is_some())) {
            return Ok(false);
        }
        // Resizable arrays of records, and fixed arrays of native records,
        // are container storage whose elements own their record values;
        // they are not per-leaf aggregate storage.
        if matches!(self.kind(node), NodeKind::Array { .. })
            && (self
                .db
                .array_meta(node)
                .is_some_and(|meta| !matches!(meta.kind(), ArrayKind::Static))
                || self.is_fixed_handle_array(node))
        {
            return Ok(false);
        }
        let object_name = self.node(node).name.clone();
        if matches!(self.kind(node), NodeKind::Net { .. }) {
            let descriptor = self
                .query_descriptor(node)
                .cloned()
                .ok_or("aggregate net has no type")?;
            let width = Self::fixed_descriptor_width(&descriptor)
                .ok_or("aggregate net requires fixed integral members")?;
            let signal = self.collect_aggregate_member_signal(
                path,
                node,
                &object_name,
                &object_name,
                (width, false, false),
            )?;
            self.insert_sig_global(node, signal.clone());
            let mut ty = descriptor.info.clone();
            ty.width = Some(width);
            let leaf = AggregateMemberInfo {
                member: AggregateMember {
                    initializer: None,
                    name: object_name,
                    ty,
                    two_state: false,
                    packed_ranges: Vec::new(),
                    aggregate: None,
                    descriptor,
                },
                signal: Some(signal),
                object: None,
                array: None,
                container: None,
                path: Vec::new(),
            };
            let members = layout
                .members
                .iter()
                .map(|member| AggregateMemberInfo {
                    member: member.clone(),
                    signal: None,
                    object: None,
                    array: None,
                    container: None,
                    path: vec![AggregatePathPart::Member(member.name.clone())],
                })
                .collect();
            self.unpacked_aggregates.insert(
                node,
                UnpackedAggregateInfo {
                    kind: layout.kind,
                    type_identity: layout.type_identity,
                    columns: false,
                    members,
                    leaves: vec![leaf],
                },
            );
            return Ok(true);
        }
        if let Some(aggregate) = sorted_node_ids(&self.unpacked_aggregates)
            .into_iter()
            .find(|existing| {
                // Procedural-block records are never port views.
                !self.block_records.contains_key(existing)
                    && self.node(*existing).name == object_name
                    && self.instance_path_of(*existing) == path
            })
            .map(|existing| self.unpacked_aggregates[&existing].clone())
        {
            // Slang can expose the same ref-port aggregate declaration through
            // more than one child node.  The instance path plus declaration
            // name identifies one storage owner; reuse its descriptor rather
            // than allocating duplicate recursive leaves.
            self.unpacked_aggregates.insert(node, aggregate);
            return Ok(true);
        }
        let is_ref_port = matches!(
            self.kind(node),
            NodeKind::Port {
                direction: DbDirection::Ref,
                ..
            }
        );
        if !matches!(self.kind(node), NodeKind::Var { .. }) && !is_ref_port {
            if matches!(
                self.kind(node),
                NodeKind::Net { .. }
                    | NodeKind::Array { .. }
                    | NodeKind::FuncArg { .. }
                    | NodeKind::IoDecl { .. }
            ) {
                return Err(format!(
                    "unpacked aggregate net/array/port `{}` in `{path}` is not supported",
                    self.node(node).name
                ));
            }
            // Slang projects aggregate layouts onto type declarations as well
            // as the variables that use them. Type-only nodes allocate no
            // runtime storage; the corresponding Var is collected separately.
            return Ok(false);
        }
        let is_union = layout.kind == AggregateKind::UnpackedUnion;
        // An untagged union has one storage extent, not one storage slot per
        // display member.  Legal unequal-width members therefore share a
        // slot sized to the largest packed member; reads/writes apply the
        // selected member's own width and signedness at the boundary.
        let union_signal = if is_union {
            if layout
                .members
                .iter()
                .any(|member| Self::fixed_descriptor_width(&member.descriptor).is_none())
            {
                return Err(format!(
                    "unpacked union `{object_name}` in `{path}` requires fixed packed value members"
                ));
            }
            let width = layout
                .members
                .iter()
                .filter_map(|member| Self::fixed_descriptor_width(&member.descriptor))
                .max()
                .ok_or_else(|| {
                    format!(
                        "unpacked union `{object_name}` in `{path}` has an unresolved member width"
                    )
                })?;
            let storage_two_state = layout.members.iter().all(|member| member.two_state);
            Some(self.collect_aggregate_member_signal(
                path,
                node,
                &object_name,
                &object_name,
                (width, false, storage_two_state),
            )?)
        } else {
            None
        };
        let columns = self
            .query_descriptor(node)
            .is_some_and(super::record_columns::record_column_layout);
        let mut leaves = Vec::new();
        for member in &layout.members {
            if let Some(signal) = &union_signal {
                let mut storage_member = member.clone();
                storage_member.ty.width = Self::fixed_descriptor_width(&member.descriptor);
                leaves.push(AggregateMemberInfo {
                    member: storage_member,
                    signal: Some(signal.clone()),
                    object: None,
                    array: None,
                    container: None,
                    path: vec![AggregatePathPart::Member(member.name.clone())],
                });
                continue;
            }
            self.collect_aggregate_descriptor_leaves(
                path,
                node,
                &object_name,
                member,
                &member.descriptor,
                &[AggregatePathPart::Member(member.name.clone())],
                union_signal.as_ref(),
                columns,
                &mut leaves,
            )?;
        }
        if leaves.is_empty() {
            return Err(format!(
                "unpacked aggregate `{object_name}` in `{path}` has no supported value leaves"
            ));
        }
        if columns {
            // Column leaves carry their own typed defaults.
            self.record_columns = true;
        } else if let Some(descriptor) = self.query_descriptor(node).cloned() {
            if let Some(default) = Self::fixed_descriptor_default(&descriptor) {
                for leaf in &leaves {
                    let Some(signal) = &leaf.signal else {
                        continue;
                    };
                    let value = if is_union {
                        default.clone()
                    } else {
                        let (_, offset) =
                            super::fixed_values::fixed_path_descriptor(&descriptor, &leaf.path)
                                .ok_or("aggregate default leaf has no fixed path")?;
                        super::fixed_values::fixed_constant_slice(
                            &default,
                            offset,
                            signal.width,
                            signal.signed,
                        )
                    };
                    self.model.signals[signal.ir].fixed_default = Some(value);
                }
            } else if !is_union {
                self.apply_record_member_defaults(path, node, &descriptor, &leaves)?;
            }
        }
        let mut members = Vec::with_capacity(layout.members.len());
        for member in &layout.members {
            let path_part = AggregatePathPart::Member(member.name.clone());
            let storage = leaves
                .iter()
                .find(|leaf| leaf.path.as_slice() == [path_part.clone()])
                .cloned();
            members.push(storage.unwrap_or_else(|| AggregateMemberInfo {
                member: member.clone(),
                signal: None,
                object: None,
                array: None,
                container: None,
                path: vec![path_part],
            }));
        }
        self.unpacked_aggregates.insert(
            node,
            UnpackedAggregateInfo {
                kind: layout.kind,
                type_identity: layout.type_identity,
                columns,
                members,
                leaves,
            },
        );
        Ok(true)
    }

    /// Why a subroutine formal, result or local of `node`'s type has no
    /// storage representation, for a specific diagnostic.
    pub(in super::super) fn unrepresented_storage_reason(&self, node: NodeId) -> Option<String> {
        let descriptor = self.query_descriptor(node)?;
        match &descriptor.shape {
            TypeShape::Aggregate(layout)
                if layout.kind == AggregateKind::TaggedUnion
                    && Self::fixed_descriptor_width_bits(descriptor).is_none() =>
            {
                Some(format!(
                    "tagged union `{}` with string, real or handle members is supported only in module and static variables",
                    descriptor.name
                ))
            }
            TypeShape::FixedArray {
                dimensions,
                element,
            } if dimensions.len() > 1 && Self::is_fixed_handle_element(element) => Some(format!(
                "multidimensional fixed array of `{}` elements is not supported",
                element.name
            )),
            TypeShape::Aggregate(layout) if layout.members.iter().any(|member| {
                has_resizable_member(&member.descriptor)
            }) =>
            {
                Some(format!(
                    "record `{}` with a queue, dynamic or associative array member is not supported",
                    descriptor.name
                ))
            }
            _ => None,
        }
    }

    /// Leaf storage of a tagged union with a native member: one tag signal
    /// (four-state, so an unassigned union has no active member) and each
    /// non-void member's own leaves. Inactive members keep stale storage
    /// that no checked access can observe (SV 7.3.2).
    fn collect_native_tagged_union(
        &mut self,
        path: &str,
        node: NodeId,
        layout: &AggregateLayout,
        descriptor: &TypeDescriptor,
    ) -> Result<(), String> {
        let object_name = self.node(node).name.clone();
        let tag_bits = layout
            .tag_bits()
            .ok_or_else(|| format!("tagged union `{object_name}` in `{path}` has no members"))?;
        let mut leaves = Vec::new();
        if tag_bits > 0 {
            let signal = self.collect_aggregate_member_signal(
                path,
                node,
                &object_name,
                NATIVE_TAG_MEMBER,
                (tag_bits, false, false),
            )?;
            leaves.push(AggregateMemberInfo {
                member: native_tag_member(tag_bits, descriptor),
                signal: Some(signal),
                object: None,
                array: None,
                container: None,
                path: vec![AggregatePathPart::Member(NATIVE_TAG_MEMBER.to_owned())],
            });
        }
        for member in &layout.members {
            if matches!(&member.descriptor.shape, TypeShape::Opaque { kind } if kind == "Void") {
                continue;
            }
            self.collect_aggregate_descriptor_leaves(
                path,
                node,
                &object_name,
                member,
                &member.descriptor,
                &[AggregatePathPart::Member(member.name.clone())],
                None,
                false,
                &mut leaves,
            )?;
        }
        let members = layout
            .members
            .iter()
            .map(|member| {
                let part = AggregatePathPart::Member(member.name.clone());
                leaves
                    .iter()
                    .find(|leaf| leaf.path.as_slice() == [part.clone()])
                    .cloned()
                    .unwrap_or_else(|| AggregateMemberInfo {
                        member: member.clone(),
                        signal: None,
                        object: None,
                        array: None,
                        container: None,
                        path: vec![part],
                    })
            })
            .collect();
        self.unpacked_aggregates.insert(
            node,
            UnpackedAggregateInfo {
                kind: AggregateKind::TaggedUnion,
                type_identity: layout.type_identity.clone(),
                columns: false,
                members,
                leaves,
            },
        );
        Ok(())
    }

    /// Recursively lower a fixed non-class descriptor to owned leaf storage.
    /// This is an emission detail only; compatibility and copy policy remain
    /// governed by the recursive descriptor captured in `core::db`.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn collect_aggregate_descriptor_leaves(
        &mut self,
        path: &str,
        object: NodeId,
        object_name: &str,
        member: &AggregateMember,
        descriptor: &TypeDescriptor,
        member_path: &[AggregatePathPart],
        shared: Option<&SignalInfo>,
        columns: bool,
        leaves: &mut Vec<AggregateMemberInfo>,
    ) -> Result<(), String> {
        if columns && shared.is_none() && super::record_columns::column_tagged_union(descriptor) {
            return self.collect_tagged_columns(
                path,
                object,
                object_name,
                member,
                descriptor,
                member_path,
                leaves,
            );
        }
        if columns
            && shared.is_none()
            && member_path
                .iter()
                .all(|part| matches!(part, AggregatePathPart::Member(_)))
        {
            let array = matches!(descriptor.shape, TypeShape::FixedArray { .. });
            let packed = matches!(descriptor.shape, TypeShape::PackedAtom { .. })
                || matches!(&descriptor.shape, TypeShape::Aggregate(layout)
                    if layout.kind != AggregateKind::UnpackedStruct);
            if array || packed {
                let leaf = if array {
                    self.collect_record_column(
                        path,
                        object,
                        object_name,
                        member,
                        descriptor,
                        member_path,
                    )?
                } else {
                    self.collect_record_cell(
                        path,
                        object,
                        object_name,
                        member,
                        descriptor,
                        member_path,
                    )?
                };
                leaves.push(leaf);
                return Ok(());
            }
        }
        match &descriptor.shape {
            TypeShape::PackedAtom { .. } => {
                let signal = match shared {
                    Some(signal) => signal.clone(),
                    None => self.collect_aggregate_member_signal(
                        path,
                        object,
                        object_name,
                        &aggregate_path_suffix(member_path),
                        (
                            descriptor.info.width.unwrap_or_default(),
                            descriptor.info.signed,
                            member.two_state,
                        ),
                    )?,
                };
                if signal.width == 0 {
                    return Err(format!(
                        "unpacked aggregate member `{}.{}` in `{path}` has zero width",
                        object_name,
                        aggregate_path_suffix(member_path)
                    ));
                }
                leaves.push(AggregateMemberInfo {
                    member: leaf_member(member, descriptor),
                    signal: Some(signal),
                    object: None,
                    array: None,
                    container: None,
                    path: member_path.to_vec(),
                });
            }
            TypeShape::Real { shortreal } => {
                if shared.is_some() {
                    return Err(format!(
                        "real member in unpacked union `{object_name}` in `{path}` is not a packed overlay"
                    ));
                }
                let signal = self.collect_aggregate_member_signal(
                    path,
                    object,
                    object_name,
                    &aggregate_path_suffix(member_path),
                    (0, false, member.two_state),
                )?;
                let ir = signal.ir;
                self.model.signals[ir].ty = crate::sim::ir::IrType::Real {
                    shortreal: *shortreal,
                };
                self.signals[ir].real = true;
                self.signals[ir].shortreal = *shortreal;
                leaves.push(AggregateMemberInfo {
                    member: leaf_member(member, descriptor),
                    signal: Some(signal),
                    object: None,
                    array: None,
                    container: None,
                    path: member_path.to_vec(),
                });
            }
            TypeShape::String => {
                if shared.is_some() {
                    return Err(format!(
                        "string member in unpacked union `{object_name}` in `{path}` is not a packed overlay"
                    ));
                }
                let key = aggregate_path_key(member_path);
                let index = self.model.objects.len();
                self.model.objects.push(crate::sim::ir::IrObject {
                    c_name: self.c_name(
                        "O",
                        path,
                        &[object_name, &aggregate_path_suffix(member_path)],
                    ),
                    ty: crate::sim::ir::IrObjectType::String,
                    initial: None,
                });
                self.aggregate_objects.insert((object, key), index);
                leaves.push(AggregateMemberInfo {
                    member: leaf_member(member, descriptor),
                    signal: None,
                    object: Some(index),
                    array: None,
                    container: None,
                    path: member_path.to_vec(),
                });
            }
            TypeShape::Aggregate(layout)
                if matches!(
                    layout.kind,
                    AggregateKind::PackedStruct
                        | AggregateKind::PackedUnion
                        | AggregateKind::UnpackedUnion
                        | AggregateKind::TaggedUnion
                ) =>
            {
                let width = Self::fixed_descriptor_width(descriptor).ok_or_else(|| {
                    if layout.kind == AggregateKind::TaggedUnion {
                        format!(
                            "tagged union member `{object_name}.{}` with string, real or handle members nested in a record or array in `{path}` is not supported",
                            aggregate_path_suffix(member_path)
                        )
                    } else {
                        "packed member has no width".to_owned()
                    }
                })?;
                let signal = match shared {
                    Some(signal) => signal.clone(),
                    None => self.collect_aggregate_member_signal(
                        path,
                        object,
                        object_name,
                        &aggregate_path_suffix(member_path),
                        (width, descriptor.info.signed, member.two_state),
                    )?,
                };
                leaves.push(AggregateMemberInfo {
                    member: leaf_member(member, descriptor),
                    signal: Some(signal),
                    object: None,
                    array: None,
                    container: None,
                    path: member_path.to_vec(),
                });
            }
            TypeShape::Aggregate(layout) => {
                for nested in &layout.members {
                    let mut nested_path = member_path.to_vec();
                    nested_path.push(AggregatePathPart::Member(nested.name.clone()));
                    self.collect_aggregate_descriptor_leaves(
                        path,
                        object,
                        object_name,
                        nested,
                        &nested.descriptor,
                        &nested_path,
                        shared,
                        columns,
                        leaves,
                    )?;
                }
            }
            TypeShape::FixedArray {
                dimensions,
                element,
            } => {
                let Some((left, right)) = dimensions.first().copied() else {
                    return Err(format!(
                        "fixed array member `{}.{}` in `{path}` has no captured bounds",
                        object_name,
                        aggregate_path_suffix(member_path)
                    ));
                };
                let rest = if dimensions.len() == 1 {
                    None
                } else {
                    Some(TypeDescriptor {
                        two_state: descriptor.two_state,
                        id: descriptor.id,
                        name: descriptor.name.clone(),
                        info: descriptor.info.clone(),
                        shape: TypeShape::FixedArray {
                            dimensions: dimensions[1..].to_vec(),
                            element: element.clone(),
                        },
                    })
                };
                let next = rest.as_ref().unwrap_or(element.as_ref());
                let mut index = left;
                loop {
                    let mut element_path = member_path.to_vec();
                    element_path.push(AggregatePathPart::Index(index));
                    self.collect_aggregate_descriptor_leaves(
                        path,
                        object,
                        object_name,
                        member,
                        next,
                        &element_path,
                        shared,
                        columns,
                        leaves,
                    )?;
                    if index == right {
                        break;
                    }
                    index = if left >= right {
                        index.checked_sub(1)
                    } else {
                        index.checked_add(1)
                    }
                    .ok_or_else(|| {
                        format!("fixed array bounds overflow in `{object_name}` in `{path}`")
                    })?;
                }
            }
            // Class and virtual-interface handles are identity leaves stored
            // like chandles, and a semaphore member is a semaphore object
            // (SIM-016); the built-in mailbox class keeps its own object
            // kind and is not a record leaf. A process member is a plain
            // handle leaf holding pinned identities (SIM-015).
            TypeShape::Opaque { kind }
                if kind == "Chandle"
                    || kind == "VirtualInterface"
                    || (kind == "Class" && descriptor.name.as_str() != "mailbox") =>
            {
                if shared.is_some() {
                    return Err(format!(
                        "chandle member in unpacked union `{object_name}` in `{path}` is not a packed overlay"
                    ));
                }
                let key = aggregate_path_key(member_path);
                let index = self.model.objects.len();
                self.model.objects.push(crate::sim::ir::IrObject {
                    c_name: self.c_name(
                        "O",
                        path,
                        &[object_name, &aggregate_path_suffix(member_path)],
                    ),
                    ty: if kind == "Class" && descriptor.name == "semaphore" {
                        crate::sim::ir::IrObjectType::Semaphore
                    } else {
                        crate::sim::ir::IrObjectType::Chandle
                    },
                    initial: None,
                });
                self.aggregate_objects.insert((object, key), index);
                leaves.push(AggregateMemberInfo {
                    member: leaf_member(member, descriptor),
                    signal: None,
                    object: Some(index),
                    array: None,
                    container: None,
                    path: member_path.to_vec(),
                });
            }
            // SIM-007: a resizable member of a module or static record is
            // its own container, addressed through the member path like a
            // string leaf's object. Unions and column layouts cannot hold one.
            TypeShape::Container { element, array, .. } if shared.is_none() && !columns => {
                let suffix = aggregate_path_suffix(member_path);
                let display = format!("{object_name}.{suffix}");
                let ir = self.model.containers.len();
                self.model.containers.push(crate::sim::ir::IrContainer {
                    c_name: self.c_name("G", path, &[object_name, &suffix]),
                    element: lower_container_element(element)?,
                    kind: ir_container_kind(array, &display, path)?,
                    initial_size: None,
                    activation: false,
                    class_field: None,
                    receiver: None,
                });
                leaves.push(AggregateMemberInfo {
                    member: leaf_member(member, descriptor),
                    signal: None,
                    object: None,
                    array: None,
                    container: Some(ContainerInfo { ir }),
                    path: member_path.to_vec(),
                });
            }
            TypeShape::Container { kind, .. } | TypeShape::Opaque { kind } => {
                return Err(format!(
                    "unpacked aggregate member `{}.{}` has unsupported recursive storage type `{kind}` in `{path}`",
                    object_name,
                    aggregate_path_suffix(member_path)
                ));
            }
        }
        Ok(())
    }

    fn collect_aggregate_member_signal(
        &mut self,
        path: &str,
        object: NodeId,
        object_name: &str,
        member_name: &str,
        packed_type: (u32, bool, bool),
    ) -> Result<SignalInfo, String> {
        let (width, signed, two_state) = packed_type;
        if width > LLG_MAX_WIDTH {
            return Err(format!(
                "unpacked aggregate storage `{object_name}` in `{path}` is {width} bits wide; the runtime maximum supported width is {LLG_MAX_WIDTH}"
            ));
        }
        let global = self.c_name(
            if width == 0 { "D" } else { "G" },
            path,
            &[object_name, member_name],
        );
        let mut hdl_name = self.waveform_name(object);
        hdl_name.push('\u{1f}');
        hdl_name.push_str(member_name);
        let ir = self.model.signals.len();
        let info = SignalInfo {
            global: global.clone(),
            width,
            signed,
            two_state,
            real: width == 0,
            shortreal: false,
            net_driver: None,
            ir,
        };
        self.model.signals.push(IrSignal {
            fixed_default: None,
            c_name: global,
            hdl_name: Some(hdl_name),
            ty: if width == 0 {
                IrType::Real { shortreal: false }
            } else {
                IrType::Packed {
                    width,
                    signed,
                    two_state,
                }
            },
            net_driver: None,
            net_alias: Vec::new(),
            alias: None,
            omit: false,
        });
        self.signals.push(info.clone());
        Ok(info)
    }

    /// Lower an `Array` arena node: element width, per-dimension bounds/sizes,
    /// total size and (constant) declaration initializer.  Rejects
    /// non-constant dimension bounds, unsupported element types and oversized
    /// arrays with clear messages.
    pub(super) fn array_info(
        &mut self,
        path: &str,
        name: &str,
        node: NodeId,
        ty: &crate::core::model::TypeInfo,
    ) -> Result<ArrayInfo, String> {
        let meta = self
            .db
            .arrays()
            .get(&node)
            .ok_or_else(|| format!("array `{name}` in `{path}` has no captured metadata"))?;
        let (elem_width, real, shortreal) = match ty.kind.as_str() {
            "real" | "shortreal" => (0, true, ty.kind == "shortreal"),
            "int" | "integer" | "time" | "longint" | "byte" | "shortint" | "logic" | "reg"
            | "bit" => (ty.width.unwrap_or(1), false, false),
            "enum" | "struct" | "union" => {
                let width = self
                    .query_descriptor(node)
                    .and_then(|descriptor| match &descriptor.shape {
                        TypeShape::FixedArray { element, .. } => {
                            Self::fixed_descriptor_width(element)
                        }
                        _ => None,
                    })
                    .or_else(|| self.packed_formal_width(node))
                    .ok_or_else(|| {
                        format!(
                            "array `{name}` in `{path}` requires a packed aggregate element type"
                        )
                    })?;
                (width, false, false)
            }
            _ => {
                return Err(format!(
                    "array `{name}` in `{path}` has unsupported element type `{}`",
                    ty.kind
                ))
            }
        };
        if !real && elem_width > LLG_MAX_WIDTH {
            return Err(format!(
                "array `{name}` in `{path}` has {elem_width}-bit elements; the \
                 runtime maximum supported width is {LLG_MAX_WIDTH}"
            ));
        }
        let mut dims: Vec<(i32, i32)> = Vec::new();
        for d in &meta.dims {
            match d {
                Some((l, r)) => {
                    dims.push((*l, *r));
                }
                None => {
                    return Err(format!(
                        "array `{name}` in `{path}` has a dimension whose bounds are \
                         not plain constants (e.g. an implicit `[N]` size); \
                         declare the range explicitly, e.g. `[0:N-1]`"
                    ))
                }
            }
        }
        let total = fixed_values::fixed_array_cell_count(&dims)
            .map_err(|error| format!("array `{name}` in `{path}`: {error}"))?;
        let init = match meta.init {
            Some(eid) => match self.array_init_consts(path, name, eid) {
                Ok(values) => Some(values),
                Err(_) => {
                    self.reserve_initializer_order(node);
                    self.array_initializers.push((node, eid));
                    None
                }
            },
            None => None,
        };
        let ir = self.model.arrays.len();
        self.model.arrays.push(crate::sim::ir::IrArray {
            activation: false,
            descriptor: false,
            net: None,
            net_elements: Vec::new(),
            element_default: self.query_descriptor(node).and_then(|descriptor| {
                match &descriptor.shape {
                    TypeShape::FixedArray { element, .. } => {
                        Self::fixed_descriptor_default(element)
                    }
                    _ => None,
                }
            }),
            element_uninitialized: self.query_descriptor(node).and_then(|descriptor| {
                match &descriptor.shape {
                    TypeShape::FixedArray { element, .. } => {
                        Self::fixed_element_uninitialized(element)
                    }
                    _ => None,
                }
            }),
            c_name: self.global_name(path, name),
            hdl_name: self.waveform_name(node),
            elem_width,
            signed: ty.signed,
            two_state: self.db.is_two_state_type(node) || is_two_state_kind(&ty.kind),
            real,
            shortreal,
            dims: dims.clone(),
            total,
        });
        Ok(ArrayInfo {
            global: self.global_name(path, name),
            elem_width,
            signed: ty.signed,
            real,
            shortreal,
            is_net: meta.net_type().is_some(),
            dims,
            init,
            ir,
        })
    }

    /// Fixed unpacked arrays of identity handles (chandles, virtual
    /// interfaces and class-like objects) use pointer-table container storage
    /// so runtime selects copy identities without flattening objects;
    /// process elements keep their reference counts in that storage.
    ///
    /// Strings and unpacked records or untagged unions with a string, real,
    /// handle or container leaf (no integral fixed payload) use the same
    /// storage: their elements are owned recursive values (SIM-007), so a
    /// runtime select reads or writes one element in place instead of
    /// flattening leaves per cell.
    pub(super) fn is_fixed_handle_element(element: &TypeDescriptor) -> bool {
        match &element.shape {
            TypeShape::Opaque { kind } => {
                matches!(kind.as_str(), "Chandle" | "VirtualInterface" | "Class")
            }
            TypeShape::String => true,
            TypeShape::Aggregate(layout) => {
                matches!(
                    layout.kind,
                    AggregateKind::UnpackedStruct
                        | AggregateKind::UnpackedUnion
                        | AggregateKind::TaggedUnion
                ) && Self::fixed_descriptor_width_bits(element).is_none()
            }
            _ => false,
        }
    }

    pub(super) fn container_info(
        &mut self,
        path: &str,
        name: &str,
        node: NodeId,
        _ty: &crate::core::model::TypeInfo,
    ) -> Result<ContainerInfo, String> {
        let meta = self
            .db
            .array_meta(node)
            .ok_or_else(|| format!("container `{name}` in `{path}` has no captured metadata"))?;
        let descriptor = self.db.type_descriptor(node).ok_or_else(|| {
            format!("container `{name}` in `{path}` has no recursive type descriptor")
        })?;
        let element = match &descriptor.shape {
            TypeShape::Container { element, .. } => lower_container_element(element)?,
            TypeShape::FixedArray {
                dimensions,
                element,
            } if Self::is_fixed_handle_element(element) => {
                if dimensions.len() != 1 {
                    return Err(format!(
                        "multidimensional fixed array `{name}` of {} elements in `{path}` is not supported",
                        element.name
                    ));
                }
                lower_container_element(element)?
            }
            _ => {
                return Err(format!(
                    "container `{name}` in `{path}` has a non-container type descriptor"
                ))
            }
        };
        self.container_from_meta(path, name, node, meta, element)
    }

    /// Allocate resizable storage for a declaration whose element shape is
    /// already lowered. Named-event declarations keep their array metadata
    /// separately from ordinary variables, so both enter here.
    pub(super) fn container_from_meta(
        &mut self,
        path: &str,
        name: &str,
        node: NodeId,
        meta: &crate::core::db::ArrayMeta,
        element: IrContainerElement,
    ) -> Result<ContainerInfo, String> {
        let has_initializer = meta.initializer().is_some();
        let kind = ir_container_kind(meta.kind(), name, path)?;
        let initial_size = matches!(meta.kind(), ArrayKind::Static)
            .then(|| {
                meta.dimensions().iter().try_fold(1u64, |total, bounds| {
                    let (left, right) = bounds.as_ref()?;
                    let extent = (i64::from(*left) - i64::from(*right))
                        .unsigned_abs()
                        .checked_add(1)?;
                    total.checked_mul(extent)
                })
            })
            .flatten();
        let ir = self.model.containers.len();
        if matches!(meta.kind(), ArrayKind::Static) {
            if let [Some(range)] = meta.dimensions() {
                self.fixed_view_ranges.insert(ir, *range);
            }
        }
        self.model.containers.push(IrContainer {
            c_name: self.global_name(path, name),
            element,
            kind,
            initial_size,
            activation: false,
            class_field: None,
            receiver: None,
        });
        if has_initializer {
            self.container_initializers.push((node, ir));
        }
        Ok(ContainerInfo { ir })
    }
}

/// Runtime container kind of a queue, dynamic or associative array type.
/// Fixed arrays of handles reuse the dynamic-array runtime.
pub(super) fn ir_container_kind(
    array: &ArrayKind,
    name: &str,
    path: &str,
) -> Result<IrContainerKind, String> {
    Ok(match array {
        // Fixed virtual-interface arrays use the same owned pointer-table
        // runtime as dynamic arrays; their HDL bounds remain in the
        // frontend descriptor and selectors are still checked by Slang.
        ArrayKind::Static => IrContainerKind::Dynamic,
        ArrayKind::Dynamic => IrContainerKind::Dynamic,
        ArrayKind::Queue { maximum_elements } => IrContainerKind::Queue {
            maximum_elements: *maximum_elements,
        },
        ArrayKind::Associative(index) => IrContainerKind::Associative {
            key: match index {
                AssociativeIndex::Wildcard => IrAssocKey::Wildcard,
                AssociativeIndex::Integral {
                    width,
                    signed,
                    two_state,
                } => IrAssocKey::Integral {
                    width: *width,
                    signed: *signed,
                    two_state: *two_state,
                },
                AssociativeIndex::String => IrAssocKey::String,
                AssociativeIndex::Unsupported(kind) => {
                    return Err(format!(
                        "associative array `{name}` in `{path}` has unsupported index type `{kind}`"
                    ))
                }
            },
        },
    })
}

/// Whether a record member is, or contains below fixed arrays and nested
/// records, a resizable container.
fn has_resizable_member(descriptor: &TypeDescriptor) -> bool {
    match &descriptor.shape {
        TypeShape::Container { .. } => true,
        TypeShape::FixedArray { element, .. } => has_resizable_member(element),
        TypeShape::Aggregate(layout) => layout
            .members
            .iter()
            .any(|member| has_resizable_member(&member.descriptor)),
        _ => false,
    }
}
