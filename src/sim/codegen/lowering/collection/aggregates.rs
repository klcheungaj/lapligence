//! Aggregates.

use super::*;

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
                return Err(format!(
                    "tagged union `{}` in `{path}` is not supported",
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
            self.sig_globals.insert(node, signal.clone());
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
                path: Vec::new(),
            };
            let members = layout
                .members
                .iter()
                .map(|member| AggregateMemberInfo {
                    member: member.clone(),
                    signal: None,
                    object: None,
                    path: vec![AggregatePathPart::Member(member.name.clone())],
                })
                .collect();
            self.unpacked_aggregates.insert(
                node,
                UnpackedAggregateInfo {
                    kind: layout.kind,
                    type_identity: layout.type_identity,
                    members,
                    leaves: vec![leaf],
                },
            );
            return Ok(true);
        }
        if let Some(aggregate) = self
            .unpacked_aggregates
            .iter()
            .find(|(existing, _)| {
                self.node(**existing).name == object_name
                    && self.instance_path_of(**existing) == path
            })
            .map(|(_, aggregate)| aggregate.clone())
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
        let mut leaves = Vec::new();
        for member in &layout.members {
            if let Some(signal) = &union_signal {
                let mut storage_member = member.clone();
                storage_member.ty.width = Self::fixed_descriptor_width(&member.descriptor);
                leaves.push(AggregateMemberInfo {
                    member: storage_member,
                    signal: Some(signal.clone()),
                    object: None,
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
                &mut leaves,
            )?;
        }
        if leaves.is_empty() {
            return Err(format!(
                "unpacked aggregate `{object_name}` in `{path}` has no supported value leaves"
            ));
        }
        if let Some(descriptor) = self.query_descriptor(node).cloned() {
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
                path: vec![path_part],
            }));
        }
        self.unpacked_aggregates.insert(
            node,
            UnpackedAggregateInfo {
                kind: layout.kind,
                type_identity: layout.type_identity,
                members,
                leaves,
            },
        );
        Ok(true)
    }

    /// Recursively lower a fixed non-class descriptor to owned leaf storage.
    /// This is an emission detail only; compatibility and copy policy remain
    /// governed by the recursive descriptor captured in `core::db`.
    #[allow(clippy::too_many_arguments)]
    fn collect_aggregate_descriptor_leaves(
        &mut self,
        path: &str,
        object: NodeId,
        object_name: &str,
        member: &AggregateMember,
        descriptor: &TypeDescriptor,
        member_path: &[AggregatePathPart],
        shared: Option<&SignalInfo>,
        leaves: &mut Vec<AggregateMemberInfo>,
    ) -> Result<(), String> {
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
                    c_name: format!(
                        "O_{}_{}_{}",
                        ident(path),
                        ident(object_name),
                        ident(&aggregate_path_suffix(member_path))
                    ),
                    ty: crate::sim::ir::IrObjectType::String,
                    initial: None,
                });
                self.aggregate_objects.insert((object, key), index);
                leaves.push(AggregateMemberInfo {
                    member: leaf_member(member, descriptor),
                    signal: None,
                    object: Some(index),
                    path: member_path.to_vec(),
                });
            }
            TypeShape::Aggregate(layout)
                if matches!(
                    layout.kind,
                    AggregateKind::PackedStruct
                        | AggregateKind::PackedUnion
                        | AggregateKind::UnpackedUnion
                ) =>
            {
                let width =
                    Self::fixed_descriptor_width(descriptor).ok_or("packed member has no width")?;
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
            TypeShape::Opaque { kind } if kind == "Chandle" => {
                if shared.is_some() {
                    return Err(format!(
                        "chandle member in unpacked union `{object_name}` in `{path}` is not a packed overlay"
                    ));
                }
                let key = aggregate_path_key(member_path);
                let index = self.model.objects.len();
                self.model.objects.push(crate::sim::ir::IrObject {
                    c_name: format!(
                        "O_{}_{}_{}",
                        ident(path),
                        ident(object_name),
                        ident(&aggregate_path_suffix(member_path))
                    ),
                    ty: crate::sim::ir::IrObjectType::Chandle,
                    initial: None,
                });
                self.aggregate_objects.insert((object, key), index);
                leaves.push(AggregateMemberInfo {
                    member: leaf_member(member, descriptor),
                    signal: None,
                    object: Some(index),
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
        let storage_name = format!("{object_name}__{member_name}");
        let global = if width == 0 {
            real_global_name(path, &storage_name)
        } else {
            global_name(path, &storage_name)
        };
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
        let init = match meta.init {
            Some(eid) => match self.array_init_consts(path, name, eid) {
                Ok(values) => Some(values),
                Err(_) => {
                    self.array_initializers.push((node, eid));
                    None
                }
            },
            None => None,
        };
        let ir = self.model.arrays.len();
        let total = dims
            .iter()
            .map(|(l, r)| ((*l as i64 - *r as i64).abs() + 1) as u64)
            .product::<u64>();
        self.model.arrays.push(crate::sim::ir::IrArray {
            net_elements: Vec::new(),
            element_default: self.query_descriptor(node).and_then(|descriptor| {
                match &descriptor.shape {
                    TypeShape::FixedArray { element, .. } => {
                        Self::fixed_descriptor_default(element)
                    }
                    _ => None,
                }
            }),
            c_name: global_name(path, name),
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
            global: global_name(path, name),
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
        let has_initializer = meta.initializer().is_some();
        let descriptor = self.db.type_descriptor(node).ok_or_else(|| {
            format!("container `{name}` in `{path}` has no recursive type descriptor")
        })?;
        let element = match &descriptor.shape {
            TypeShape::Container { element, .. } => lower_container_element(element)?,
            TypeShape::FixedArray { element, .. } if matches!(element.shape, TypeShape::Opaque { ref kind } if kind == "VirtualInterface") => {
                IrContainerElement::Chandle
            }
            _ => {
                return Err(format!(
                    "container `{name}` in `{path}` has a non-container type descriptor"
                ))
            }
        };
        let kind = match meta.kind() {
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
        };
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
        self.model.containers.push(IrContainer {
            c_name: global_name(path, name),
            element,
            kind,
            initial_size,
        });
        if has_initializer {
            self.container_initializers.push((node, ir));
        }
        Ok(ContainerInfo { ir })
    }
}
