//! Fixed storage defaults preserve explicit member initializers and state domains.
use super::fixed_values::{fixed_width, two_state};
use super::*;

impl Codegen<'_> {
    pub(in super::super) fn fixed_default_literal(&self, node: NodeId) -> Option<IrConst> {
        Self::fixed_descriptor_default(self.query_descriptor(node)?)
    }

    pub(in super::super) fn fixed_descriptor_default(
        descriptor: &TypeDescriptor,
    ) -> Option<IrConst> {
        if !matches!(
            &descriptor.shape,
            TypeShape::FixedArray { .. }
                | TypeShape::Aggregate(crate::core::db::AggregateLayout {
                    kind: AggregateKind::UnpackedStruct | AggregateKind::UnpackedUnion,
                    ..
                })
        ) {
            return None;
        }
        let width = fixed_width(descriptor)?;
        let mut bits = vec![Bit::X; width as usize];
        fn member_default(member: &AggregateMember, offset: u32, bits: &mut [Bit]) -> Option<()> {
            if let Some(initializer) = &member.initializer {
                let width = fixed_width(&member.descriptor)?;
                let Val::Bits(value) =
                    val_from_value_data(initializer, i32::try_from(width).ok()?).ok()?
                else {
                    return None;
                };
                for index in 0..width {
                    bits[(offset + index) as usize] = value.bit_lsb(index as usize);
                }
                Some(())
            } else {
                defaults(&member.descriptor, offset, member.two_state, bits)
            }
        }
        fn defaults(
            descriptor: &TypeDescriptor,
            offset: u32,
            clear: bool,
            bits: &mut [Bit],
        ) -> Option<()> {
            let width = fixed_width(descriptor)?;
            if clear || two_state(descriptor) {
                for bit in offset..offset.checked_add(width)? {
                    bits[bit as usize] = Bit::Zero;
                }
            }
            match &descriptor.shape {
                TypeShape::Aggregate(layout) if layout.kind == AggregateKind::UnpackedStruct => {
                    let mut cursor = offset;
                    for member in layout.members.iter().rev() {
                        member_default(member, cursor, bits)?;
                        cursor = cursor.checked_add(fixed_width(&member.descriptor)?)?;
                    }
                }
                TypeShape::Aggregate(layout) if layout.kind == AggregateKind::UnpackedUnion => {
                    let member = layout.members.first()?;
                    let start = if matches!(&member.descriptor.shape, TypeShape::Aggregate(layout) if layout.kind == AggregateKind::UnpackedStruct)
                    {
                        offset + width - fixed_width(&member.descriptor)?
                    } else {
                        offset
                    };
                    member_default(member, start, bits)?;
                }
                TypeShape::FixedArray { element, .. } => {
                    let stride = fixed_width(element)?;
                    for index in 0..width / stride {
                        defaults(element, offset + index * stride, false, bits)?;
                    }
                }
                _ => {}
            }
            Some(())
        }
        defaults(descriptor, 0, false, &mut bits)?;
        bits.reverse();
        val_to_const(&elab::Value::from_bits(bits, descriptor.info.signed)).ok()
    }
}
