//! Fixed value activations keep declaration-order payloads and typed projections.
use super::*;

pub(super) fn fixed_width(descriptor: &TypeDescriptor) -> Option<u32> {
    let width = match &descriptor.shape {
        TypeShape::PackedAtom { .. } => descriptor.info.width?,
        TypeShape::Aggregate(layout) => {
            let mut widths = layout
                .members
                .iter()
                .map(|member| fixed_width(&member.descriptor));
            if matches!(
                layout.kind,
                AggregateKind::PackedUnion | AggregateKind::UnpackedUnion
            ) {
                widths.try_fold(0, |largest, width| Some(largest.max(width?)))?
            } else if matches!(
                layout.kind,
                AggregateKind::PackedStruct | AggregateKind::UnpackedStruct
            ) {
                widths.try_fold(0u32, |sum, width| sum.checked_add(width?))?
            } else {
                return None;
            }
        }
        TypeShape::FixedArray {
            dimensions,
            element,
        } => dimensions
            .iter()
            .try_fold(fixed_width(element)?, |width, (left, right)| {
                let count = i64::from(*left)
                    .abs_diff(i64::from(*right))
                    .checked_add(1)?;
                u32::try_from(u64::from(width).checked_mul(count)?).ok()
            })?,
        _ => return None,
    };
    (width != 0 && width <= LLG_MAX_WIDTH).then_some(width)
}

pub(super) fn two_state(descriptor: &TypeDescriptor) -> bool {
    descriptor.two_state
}

pub(super) fn fixed_path_descriptor(
    root: &TypeDescriptor,
    path: &[AggregatePathPart],
) -> Option<(TypeDescriptor, u32)> {
    let mut descriptor = root.clone();
    let mut offset = 0u32;
    for part in path {
        match (part, &descriptor.shape) {
            (AggregatePathPart::Member(name), TypeShape::Aggregate(layout)) => {
                let index = layout
                    .members
                    .iter()
                    .position(|member| &member.name == name)?;
                let member = &layout.members[index];
                let displacement = if layout.kind == AggregateKind::UnpackedUnion
                    && matches!(&member.descriptor.shape, TypeShape::Aggregate(layout) if layout.kind == AggregateKind::UnpackedStruct)
                {
                    fixed_width(&descriptor)? - fixed_width(&member.descriptor)?
                } else if matches!(
                    layout.kind,
                    AggregateKind::UnpackedUnion | AggregateKind::PackedUnion
                ) {
                    0
                } else {
                    layout.members[index + 1..]
                        .iter()
                        .try_fold(0u32, |sum, member| {
                            sum.checked_add(fixed_width(&member.descriptor)?)
                        })?
                };
                offset = offset.checked_add(displacement)?;
                descriptor = member.descriptor.clone();
            }
            (
                AggregatePathPart::Index(index),
                TypeShape::FixedArray {
                    dimensions,
                    element,
                },
            ) => {
                let (left, right) = *dimensions.first()?;
                if *index < left.min(right) || *index > left.max(right) {
                    return None;
                }
                let next = if dimensions.len() == 1 {
                    *element.clone()
                } else {
                    TypeDescriptor {
                        shape: TypeShape::FixedArray {
                            dimensions: dimensions[1..].to_vec(),
                            element: element.clone(),
                        },
                        ..descriptor.clone()
                    }
                };
                let ordinal = u32::try_from(i64::from(*index).abs_diff(i64::from(right))).ok()?;
                offset = offset.checked_add(ordinal.checked_mul(fixed_width(&next)?)?)?;
                descriptor = next;
            }
            _ => return None,
        }
    }
    Some((descriptor, offset))
}

pub(super) fn fixed_constant_slice(
    value: &IrConst,
    offset: u32,
    width: u32,
    signed: bool,
) -> IrConst {
    let mut bits = Vec::with_capacity(width as usize);
    for index in (offset..offset + width).rev() {
        let mask = 1u64 << (index % 64);
        let word = (index / 64) as usize;
        bits.push(if value.x[word] & mask != 0 {
            Bit::X
        } else if value.z[word] & mask != 0 {
            Bit::Z
        } else if value.bits[word] & mask != 0 {
            Bit::One
        } else {
            Bit::Zero
        });
    }
    val_to_const(&elab::Value::from_bits(bits, signed)).expect("checked fixed storage width")
}

impl Codegen<'_> {
    pub(super) fn fixed_formal_shape(
        &self,
        node: NodeId,
    ) -> Result<Option<IrContainerElement>, String> {
        let Some(descriptor) = self.query_descriptor(node) else {
            return Ok(None);
        };
        if !matches!(
            descriptor.shape,
            TypeShape::FixedArray { .. } | TypeShape::Aggregate(_)
        ) {
            return Ok(None);
        }
        let Some(_) = fixed_width(descriptor) else {
            return Ok(None);
        };
        lower_container_element(descriptor).map(Some)
    }

    pub(in super::super) fn fixed_descriptor_width(descriptor: &TypeDescriptor) -> Option<u32> {
        fixed_width(descriptor)
    }

    pub(super) fn formal_value_width(&self, node: NodeId) -> Option<u32> {
        self.fixed_value_width(node)
            .or_else(|| self.packed_formal_width(node))
    }

    pub(in super::super) fn fixed_value_width(&self, node: NodeId) -> Option<u32> {
        fixed_width(self.query_descriptor(node)?)
    }

    pub(in super::super) fn fixed_storage_lhs(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrLhs>, String> {
        if let Some(lhs) = self.fixed_activation_lhs(path, node)? {
            return Ok(Some(lhs));
        }
        let mut parts = Vec::new();
        if let Some(array) = self.array_of(node).cloned() {
            if array.real {
                return Ok(None);
            }
            for indices in port_array_index_vectors(&array.dims) {
                parts.push((
                    IrLhs::ArrayElem {
                        arr: self.reference_array(array.ir),
                        indices: indices
                            .into_iter()
                            .map(|index| lhs_integer_expr(i128::from(index)))
                            .collect(),
                        elem_sel: IrElemSel::Whole,
                    },
                    array.elem_width,
                ));
            }
        } else if let Some((_, aggregate)) = self.unpacked_aggregate_info(node) {
            for leaf in &aggregate.leaves {
                let Some(width) = leaf.member.ty.width.filter(|width| *width != 0) else {
                    return Ok(None);
                };
                if leaf.object.is_some() {
                    return Ok(None);
                }
                parts.push((self.aggregate_leaf_lhs(leaf)?, width));
            }
        } else {
            return Ok(None);
        }
        let width = parts
            .iter()
            .try_fold(0u32, |sum, (_, width)| sum.checked_add(*width))
            .filter(|width| *width != 0 && *width <= LLG_MAX_WIDTH)
            .ok_or("fixed value payload exceeds supported width")?;
        Ok(Some(IrLhs::Stream {
            parts,
            width,
            slice: 1,
            direction: IrStreamDirection::LeftToRight,
        }))
    }
}
