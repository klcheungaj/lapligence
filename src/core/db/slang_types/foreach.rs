//! Preserve foreach dimensions before the type graph becomes a storage shape.

use super::*;

impl SlangTypeProjector<'_> {
    /// Project exactly the source iterator slots, including omitted slots.
    /// `None` denotes a runtime-sized dimension, not an omitted loop variable.
    /// Work from the iterated expression's type: array storage and recursive
    /// value descriptors can flatten dimensions or unwrap an enum's base type.
    pub fn foreach_dimensions(
        &self,
        type_id: u64,
        count: usize,
    ) -> Result<Vec<Option<(i32, i32)>>, String> {
        let mut dimensions = Vec::new();
        let mut current = Some(self.ty(type_id)?);
        let mut seen = HashSet::new();
        for _ in 0..count {
            let ty = current.ok_or_else(|| {
                "foreach iterator count exceeds the iterated type's dimensions".to_owned()
            })?;
            if !seen.insert(ty.id) {
                return Err("cycle in foreach array type".to_owned());
            }
            let dimension = match ty.kind {
                TypeKind::PackedArray | TypeKind::FixedUnpackedArray => {
                    let kind = if ty.kind == TypeKind::PackedArray {
                        TypeRangeKind::Packed
                    } else {
                        TypeRangeKind::Unpacked
                    };
                    let range = self.one_range(ty, kind)?;
                    let left = i32::try_from(range.left)
                        .map_err(|_| "foreach left bound does not fit an int index".to_owned())?;
                    let right = i32::try_from(range.right)
                        .map_err(|_| "foreach right bound does not fit an int index".to_owned())?;
                    current = Some(self.element_type(ty, "foreach array")?);
                    Some((left, right))
                }
                TypeKind::Integral if ty.bit_width <= 1 => {
                    return Err("foreach cannot iterate a scalar bit or logic value".to_owned());
                }
                TypeKind::Integral
                | TypeKind::Enum
                | TypeKind::PackedStruct
                | TypeKind::PackedUnion => {
                    let left = ty
                        .bit_width
                        .checked_sub(1)
                        .and_then(|left| i32::try_from(left).ok())
                        .ok_or_else(|| {
                            "foreach integral width has no int-indexed range".to_owned()
                        })?;
                    // The range belongs to this single integral value, not
                    // an enum's underlying packed range or a record's members.
                    current = None;
                    Some((left, 0))
                }
                TypeKind::DynamicArray | TypeKind::AssociativeArray | TypeKind::Queue => {
                    current = Some(self.element_type(ty, "foreach container")?);
                    None
                }
                TypeKind::String => {
                    current = None;
                    None
                }
                _ => return Err("foreach target has a non-iterable dimension".to_owned()),
            };
            dimensions.push(dimension);
        }
        Ok(dimensions)
    }
}

#[cfg(test)]
mod tests;
