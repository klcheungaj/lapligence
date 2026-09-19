//! Projection of Slang's typed snapshot records into owned database metadata.

use super::{
    AggregateKind, AggregateLayout, AggregateMember, ArrayKind, AssociativeIndex,
    ElaboratedTypeRanges, NodeId, PackedMember, PackedRange, TypeDescriptor, TypeId, TypeShape,
};
use crate::core::model::TypeInfo;
use crate::ffi::slang::{
    Snapshot, Type as SlangType, TypeKind, TypeMember, TypeRange, TypeRangeKind,
};
use std::collections::{HashMap, HashSet};

const MAX_RECURSIVE_TYPE_DEPTH: usize = 64;

/// Array metadata that does not depend on an arena node or initializer edge.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct ArrayTypeProjection {
    pub kind: ArrayKind,
    pub dimensions: Vec<Option<(i32, i32)>>,
    pub element_type: TypeInfo,
}

/// All database type metadata derivable from one Slang type record.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct TypeProjection {
    pub type_info: TypeInfo,
    pub descriptor: TypeDescriptor,
    pub two_state: bool,
    pub packed_dimensions: Vec<PackedRange>,
    pub packed_members: Option<Vec<PackedMember>>,
    pub aggregate_layout: Option<AggregateLayout>,
    pub array: Option<ArrayTypeProjection>,
}

/// Indexed, reusable view over the snapshot's validated type tables.
pub(super) struct SlangTypeProjector<'a> {
    types: HashMap<u64, &'a SlangType>,
    ranges: &'a [TypeRange],
    members: &'a [TypeMember],
    constants: &'a [crate::ffi::slang::Constant],
}

impl<'a> SlangTypeProjector<'a> {
    pub fn new(snapshot: &'a Snapshot) -> Result<Self, String> {
        let types = snapshot
            .types
            .iter()
            .map(|ty| (ty.id, ty))
            .collect::<HashMap<_, _>>();
        if types.len() != snapshot.types.len() {
            return Err("duplicate Slang type id".to_owned());
        }
        Ok(Self {
            types,
            ranges: &snapshot.type_ranges,
            members: &snapshot.type_members,
            constants: &snapshot.constants,
        })
    }

    pub fn project(&self, type_id: u64) -> Result<TypeProjection, String> {
        let ty = self.ty(type_id)?;
        let type_info = self.type_info(ty)?;
        let packed_owner = self.unpacked_element(ty)?;
        let packed_dimensions = self.packed_dimensions(packed_owner)?;
        let packed_base = self.packed_base(packed_owner)?;
        let packed_members = self.packed_members(packed_base)?;
        let mut visiting = HashSet::new();
        let aggregate_layout = self.aggregate_layout(packed_base, &mut visiting)?;
        let descriptor = self.descriptor(ty, &mut HashSet::new())?;
        let array = self.array(ty)?;
        Ok(TypeProjection {
            type_info,
            descriptor,
            two_state: !ty.is_four_state,
            packed_dimensions,
            packed_members,
            aggregate_layout,
            array,
        })
    }

    fn unpacked_element(&self, ty: &'a SlangType) -> Result<&'a SlangType, String> {
        let mut current = ty;
        let mut seen = HashSet::new();
        while matches!(
            current.kind,
            TypeKind::FixedUnpackedArray
                | TypeKind::DynamicArray
                | TypeKind::AssociativeArray
                | TypeKind::Queue
        ) {
            if !seen.insert(current.id) {
                return Err(format!("cycle in Slang unpacked array type {}", current.id));
            }
            current = self.element_type(current, "unpacked array")?;
        }
        Ok(current)
    }

    pub fn elaborated_ranges(
        &self,
        declaration: NodeId,
        instance: String,
        name: String,
        type_id: u64,
    ) -> Result<ElaboratedTypeRanges, String> {
        Ok(ElaboratedTypeRanges {
            declaration,
            instance,
            name,
            packed_ranges: self
                .packed_dimensions(self.unpacked_element(self.ty(type_id)?)?)?
                .into_iter()
                .map(Some)
                .collect(),
        })
    }

    fn ty(&self, id: u64) -> Result<&'a SlangType, String> {
        self.types
            .get(&id)
            .copied()
            .ok_or_else(|| format!("unknown Slang type id {id}"))
    }

    fn type_info(&self, ty: &SlangType) -> Result<TypeInfo, String> {
        let builtin_integral = matches!(ty.kind, TypeKind::Integral)
            .then(|| {
                let spelling = ty
                    .display_name
                    .strip_suffix(" unsigned")
                    .unwrap_or(&ty.display_name);
                match spelling {
                    "logic" | "bit" | "reg" | "int" | "integer" | "longint" | "byte"
                    | "shortint" | "time" => Some(spelling),
                    _ => None,
                }
            })
            .flatten();
        let kind = match ty.kind {
            TypeKind::Integral if builtin_integral.is_some() => {
                builtin_integral.unwrap_or_default().to_owned()
            }
            TypeKind::Integral if ty.is_four_state => "logic".to_owned(),
            TypeKind::Integral => "bit".to_owned(),
            TypeKind::Floating if ty.bit_width == 32 => "shortreal".to_owned(),
            TypeKind::Floating => "real".to_owned(),
            TypeKind::String => "string".to_owned(),
            TypeKind::Aggregate => "other".to_owned(),
            TypeKind::UnpackedStruct => "struct".to_owned(),
            TypeKind::UnpackedUnion => "union".to_owned(),
            TypeKind::Enum => "enum".to_owned(),
            TypeKind::PackedArray => self.type_info(self.element_type(ty, "packed array")?)?.kind,
            TypeKind::FixedUnpackedArray
            | TypeKind::DynamicArray
            | TypeKind::AssociativeArray
            | TypeKind::Queue => "array".to_owned(),
            TypeKind::PackedStruct => "struct".to_owned(),
            TypeKind::PackedUnion => "union".to_owned(),
            TypeKind::Class => "class".to_owned(),
            TypeKind::Chandle => "chandle".to_owned(),
            TypeKind::Event => "event".to_owned(),
            TypeKind::Void => "void".to_owned(),
            TypeKind::VirtualInterface => "virtual_interface".to_owned(),
            TypeKind::Other => "other".to_owned(),
        };
        let width = if ty.is_fixed_size {
            Some(
                u32::try_from(ty.bit_width)
                    .map_err(|_| format!("Slang type {} is wider than u32", ty.id))?,
            )
        } else {
            None
        };
        Ok(TypeInfo {
            kind,
            width,
            signed: ty.is_signed,
            type_name: if matches!(ty.kind, TypeKind::VirtualInterface) {
                Some(ty.display_name.clone())
            } else if builtin_integral.is_some() {
                None
            } else {
                nominal_identity(&ty.display_name)
            },
        })
    }

    fn element_type(&self, ty: &SlangType, context: &str) -> Result<&'a SlangType, String> {
        let id = ty
            .element_type_id
            .ok_or_else(|| format!("Slang {context} type {} has no element type", ty.id))?;
        self.ty(id)
    }

    fn type_ranges(&self, ty: &SlangType) -> Result<&'a [TypeRange], String> {
        let start = usize::try_from(ty.range_start)
            .map_err(|_| format!("Slang type {} range start is too large", ty.id))?;
        let count = usize::try_from(ty.range_count)
            .map_err(|_| format!("Slang type {} range count is too large", ty.id))?;
        let end = start
            .checked_add(count)
            .ok_or_else(|| format!("Slang type {} range window overflowed", ty.id))?;
        self.ranges
            .get(start..end)
            .ok_or_else(|| format!("Slang type {} range window is invalid", ty.id))
    }

    fn type_members(&self, ty: &SlangType) -> Result<&'a [TypeMember], String> {
        let start = usize::try_from(ty.member_start)
            .map_err(|_| format!("Slang type {} member start is too large", ty.id))?;
        let count = usize::try_from(ty.member_count)
            .map_err(|_| format!("Slang type {} member count is too large", ty.id))?;
        let end = start
            .checked_add(count)
            .ok_or_else(|| format!("Slang type {} member window overflowed", ty.id))?;
        self.members
            .get(start..end)
            .ok_or_else(|| format!("Slang type {} member window is invalid", ty.id))
    }

    fn one_range(&self, ty: &SlangType, kind: TypeRangeKind) -> Result<&'a TypeRange, String> {
        let ranges = self.type_ranges(ty)?;
        match ranges {
            [range] if range.kind == kind => Ok(range),
            _ => Err(format!(
                "Slang type {} does not have exactly one {kind:?} range",
                ty.id
            )),
        }
    }

    fn packed_dimensions(&self, ty: &SlangType) -> Result<Vec<PackedRange>, String> {
        let mut dimensions = Vec::new();
        let mut current = ty;
        let mut seen = HashSet::new();
        while matches!(current.kind, TypeKind::PackedArray | TypeKind::Enum) {
            if !seen.insert(current.id) {
                return Err(format!("cycle in Slang packed type {}", current.id));
            }
            if current.kind == TypeKind::Enum {
                current = self.element_type(current, "enum base")?;
                continue;
            }
            let range = self.one_range(current, TypeRangeKind::Packed)?;
            dimensions.push(PackedRange {
                left: i128::from(range.left),
                right: i128::from(range.right),
            });
            current = self.element_type(current, "packed array")?;
        }
        Ok(dimensions)
    }

    fn packed_base(&self, ty: &'a SlangType) -> Result<&'a SlangType, String> {
        let mut current = ty;
        let mut seen = HashSet::new();
        while current.kind == TypeKind::PackedArray {
            if !seen.insert(current.id) {
                return Err(format!("cycle in Slang packed array type {}", current.id));
            }
            current = self.element_type(current, "packed array")?;
        }
        Ok(current)
    }

    fn packed_members(&self, ty: &SlangType) -> Result<Option<Vec<PackedMember>>, String> {
        if !matches!(ty.kind, TypeKind::PackedStruct | TypeKind::PackedUnion) {
            return Ok(None);
        }
        self.type_members(ty)?
            .iter()
            .map(|member| {
                let member_ty = self.ty(member.type_id)?;
                Ok(PackedMember {
                    name: member.name.clone(),
                    lsb: u32::try_from(member.bit_offset).map_err(|_| {
                        format!("packed member `{}` offset is too large", member.name)
                    })?,
                    width: u32::try_from(member.bit_width).map_err(|_| {
                        format!("packed member `{}` width is too large", member.name)
                    })?,
                    signed: member_ty.is_signed,
                    two_state: !member_ty.is_four_state,
                    packed_ranges: self.member_packed_ranges(member_ty)?,
                })
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some)
    }

    fn aggregate_layout(
        &self,
        ty: &SlangType,
        visiting: &mut HashSet<u64>,
    ) -> Result<Option<AggregateLayout>, String> {
        let kind = match ty.kind {
            TypeKind::PackedStruct => AggregateKind::PackedStruct,
            TypeKind::PackedUnion => AggregateKind::PackedUnion,
            TypeKind::UnpackedStruct => AggregateKind::UnpackedStruct,
            TypeKind::UnpackedUnion => AggregateKind::UnpackedUnion,
            _ => return Ok(None),
        };
        if visiting.len() >= MAX_RECURSIVE_TYPE_DEPTH {
            return Err(format!(
                "Slang aggregate type {} exceeds the recursive type depth limit",
                ty.id
            ));
        }
        if !visiting.insert(ty.id) {
            return Err(format!("cycle in Slang aggregate type {}", ty.id));
        }
        let members = self
            .type_members(ty)?
            .iter()
            .map(|member| {
                let member_ty = self.ty(member.type_id)?;
                let descriptor = self.descriptor(member_ty, visiting)?;
                let initializer = member
                    .initializer_constant_id
                    .map(|id| {
                        usize::try_from(id)
                            .ok()
                            .and_then(|id| self.constants.get(id))
                            .map(|constant| super::database::value_data_from_slang(&constant.value))
                            .ok_or_else(|| {
                                "aggregate member initializer constant is missing".to_string()
                            })
                    })
                    .transpose()?;
                Ok(AggregateMember {
                    initializer,
                    name: member.name.clone(),
                    ty: self.type_info(member_ty)?,
                    two_state: !member_ty.is_four_state,
                    packed_ranges: self.member_packed_ranges(member_ty)?,
                    // The descriptor is the canonical recursive representation.
                    // Keep the legacy projection empty to avoid duplicating every
                    // nested subtree at each aggregate member.
                    aggregate: None,
                    descriptor,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        visiting.remove(&ty.id);
        Ok(Some(AggregateLayout {
            kind,
            type_identity: Some(format!("type#{}", ty.id)),
            type_id: Some(TypeId(ty.id)),
            members,
        }))
    }

    /// Build one complete recursive descriptor from the owned Slang type
    /// table. Arrays retain bounds and their element descriptor; aggregates
    /// retain nominal identity and every nested member shape.
    fn descriptor(
        &self,
        ty: &SlangType,
        visiting: &mut HashSet<u64>,
    ) -> Result<TypeDescriptor, String> {
        if visiting.len() >= MAX_RECURSIVE_TYPE_DEPTH {
            return Err(format!(
                "Slang value type {} exceeds the recursive type depth limit",
                ty.id
            ));
        }
        if !visiting.insert(ty.id) {
            return Err(format!("cycle in Slang value type {}", ty.id));
        }
        let info = self.type_info(ty)?;
        let shape = match ty.kind {
            TypeKind::Integral | TypeKind::Enum | TypeKind::PackedArray => TypeShape::PackedAtom {
                ranges: self.packed_dimensions(ty)?,
            },
            TypeKind::Floating => TypeShape::Real {
                shortreal: ty.bit_width == 32,
            },
            TypeKind::String => TypeShape::String,
            TypeKind::PackedStruct
            | TypeKind::PackedUnion
            | TypeKind::UnpackedStruct
            | TypeKind::UnpackedUnion => {
                // `aggregate_layout` owns the insertion/removal of the
                // aggregate id while it walks members.  The descriptor has
                // already inserted it for the outer shape, so hand that
                // ownership to the layout walk before delegating.
                visiting.remove(&ty.id);
                TypeShape::Aggregate(
                    self.aggregate_layout(ty, visiting)?
                        .ok_or_else(|| format!("missing aggregate layout for type {}", ty.id))?,
                )
            }
            TypeKind::FixedUnpackedArray => {
                let mut dimensions = Vec::new();
                let mut current = ty;
                let mut seen = HashSet::new();
                while current.kind == TypeKind::FixedUnpackedArray {
                    if !seen.insert(current.id) {
                        return Err(format!("cycle in Slang unpacked array type {}", current.id));
                    }
                    let range = self.one_range(current, TypeRangeKind::Unpacked)?;
                    dimensions.push((
                        i32::try_from(range.left).map_err(|_| {
                            format!("array type {} left bound is outside i32", current.id)
                        })?,
                        i32::try_from(range.right).map_err(|_| {
                            format!("array type {} right bound is outside i32", current.id)
                        })?,
                    ));
                    current = self.element_type(current, "fixed unpacked array")?;
                }
                TypeShape::FixedArray {
                    dimensions,
                    element: Box::new(self.descriptor(current, visiting)?),
                }
            }
            TypeKind::DynamicArray | TypeKind::AssociativeArray | TypeKind::Queue => {
                TypeShape::Container {
                    kind: format!("{:?}", ty.kind),
                    element: Box::new(
                        self.descriptor(self.element_type(ty, "container")?, visiting)?,
                    ),
                }
            }
            TypeKind::Chandle
            | TypeKind::Class
            | TypeKind::Event
            | TypeKind::Void
            | TypeKind::VirtualInterface
            | TypeKind::Aggregate
            | TypeKind::Other => TypeShape::Opaque {
                kind: format!("{:?}", ty.kind),
            },
        };
        visiting.remove(&ty.id);
        Ok(TypeDescriptor {
            two_state: !ty.is_four_state,
            id: TypeId(ty.id),
            name: ty.display_name.clone(),
            info,
            shape,
        })
    }

    fn array(&self, ty: &SlangType) -> Result<Option<ArrayTypeProjection>, String> {
        let (kind, element, dimensions) = match ty.kind {
            TypeKind::FixedUnpackedArray => {
                let mut dimensions = Vec::new();
                let mut current = ty;
                let mut seen = HashSet::new();
                while current.kind == TypeKind::FixedUnpackedArray {
                    if !seen.insert(current.id) {
                        return Err(format!("cycle in Slang unpacked array type {}", current.id));
                    }
                    let range = self.one_range(current, TypeRangeKind::Unpacked)?;
                    let left = i32::try_from(range.left).map_err(|_| {
                        format!("array type {} left bound is outside i32", current.id)
                    })?;
                    let right = i32::try_from(range.right).map_err(|_| {
                        format!("array type {} right bound is outside i32", current.id)
                    })?;
                    dimensions.push(Some((left, right)));
                    current = self.element_type(current, "fixed unpacked array")?;
                }
                (ArrayKind::Static, current, dimensions)
            }
            TypeKind::DynamicArray => (
                ArrayKind::Dynamic,
                self.element_type(ty, "dynamic array")?,
                Vec::new(),
            ),
            TypeKind::AssociativeArray => {
                let index = match ty.index_type_id {
                    None => AssociativeIndex::Wildcard,
                    Some(id) => {
                        let index = self.ty(id)?;
                        match index.kind {
                            TypeKind::Integral
                            | TypeKind::Enum
                            | TypeKind::PackedArray
                            | TypeKind::PackedStruct
                            | TypeKind::PackedUnion => AssociativeIndex::Integral {
                                width: u32::try_from(index.bit_width).map_err(|_| {
                                    format!("associative index type {} is wider than u32", index.id)
                                })?,
                                signed: index.is_signed,
                                two_state: !index.is_four_state,
                            },
                            TypeKind::String => AssociativeIndex::String,
                            _ => AssociativeIndex::Unsupported(if index.display_name.is_empty() {
                                format!("{:?}", index.kind)
                            } else {
                                index.display_name.clone()
                            }),
                        }
                    }
                };
                (
                    ArrayKind::Associative(index),
                    self.element_type(ty, "associative array")?,
                    Vec::new(),
                )
            }
            TypeKind::Queue => {
                let ranges = self.type_ranges(ty)?;
                let maximum_elements = match ranges {
                    [] => None,
                    [range] if range.kind == TypeRangeKind::QueueBound && range.right >= 0 => Some(
                        u64::try_from(range.right)
                            .ok()
                            .and_then(|bound| bound.checked_add(1))
                            .ok_or_else(|| format!("queue type {} bound overflowed", ty.id))?,
                    ),
                    _ => return Err(format!("Slang queue type {} has an invalid bound", ty.id)),
                };
                (
                    ArrayKind::Queue { maximum_elements },
                    self.element_type(ty, "queue")?,
                    Vec::new(),
                )
            }
            _ => return Ok(None),
        };
        Ok(Some(ArrayTypeProjection {
            kind,
            dimensions,
            element_type: self.type_info(element)?,
        }))
    }

    fn member_packed_ranges(&self, ty: &SlangType) -> Result<Vec<PackedRange>, String> {
        let ranges = self.packed_dimensions(ty)?;
        if !ranges.is_empty()
            || !ty.is_fixed_size
            || ty.bit_width == 0
            || !matches!(ty.kind, TypeKind::Integral | TypeKind::Enum)
        {
            return Ok(ranges);
        }
        Ok(vec![PackedRange {
            left: i128::from(ty.bit_width - 1),
            right: 0,
        }])
    }
}

fn nominal_identity(display_name: &str) -> Option<String> {
    let name = display_name.trim();
    (!name.is_empty()
        && name
            .chars()
            .all(|character| character.is_alphanumeric() || matches!(character, '_' | '$' | ':')))
    .then(|| name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::db::{
        ValueCopySemantics, ValueDefaultSemantics, ValueDestroySemantics, ValueEqualitySemantics,
    };

    fn ty(id: u64, kind: TypeKind, bit_width: u64) -> SlangType {
        SlangType {
            id,
            kind,
            is_signed: false,
            is_four_state: true,
            is_fixed_size: true,
            bit_width,
            display_name: String::new(),
            element_type_id: None,
            index_type_id: None,
            range_start: 0,
            range_count: 0,
            member_start: 0,
            member_count: 0,
        }
    }

    #[test]
    fn projects_multidimensional_static_array_without_losing_element_type() {
        let element = ty(0, TypeKind::Integral, 8);
        let mut inner = ty(1, TypeKind::FixedUnpackedArray, 32);
        inner.element_type_id = Some(0);
        inner.range_count = 1;
        let mut outer = ty(2, TypeKind::FixedUnpackedArray, 64);
        outer.element_type_id = Some(1);
        outer.range_start = 1;
        outer.range_count = 1;
        let types = [element, inner, outer];
        let ranges = [
            TypeRange {
                left: 3,
                right: 0,
                kind: TypeRangeKind::Unpacked,
            },
            TypeRange {
                left: 1,
                right: 0,
                kind: TypeRangeKind::Unpacked,
            },
        ];
        let projector = SlangTypeProjector {
            constants: &[],
            types: types.iter().map(|ty| (ty.id, ty)).collect(),
            ranges: &ranges,
            members: &[],
        };

        let projection = projector.project(2).expect("project static array");
        match &projection.descriptor.shape {
            TypeShape::FixedArray {
                dimensions,
                element,
            } => {
                assert_eq!(dimensions, &vec![(1, 0), (3, 0)]);
                assert_eq!(element.info.width, Some(8));
                assert!(matches!(element.shape, TypeShape::PackedAtom { .. }));
            }
            shape => panic!("expected recursive fixed-array descriptor, got {shape:?}"),
        }
        assert_eq!(
            projection.descriptor.copy_semantics(),
            ValueCopySemantics::Deep
        );
        assert_eq!(
            projection.descriptor.default_semantics(),
            ValueDefaultSemantics::Recursive
        );
        assert_eq!(
            projection.descriptor.destroy_semantics(),
            ValueDestroySemantics::Recursive
        );
        assert_eq!(
            projection.descriptor.equality_semantics(),
            ValueEqualitySemantics::Recursive
        );
        let array = projection.array.expect("array metadata");
        assert_eq!(array.kind, ArrayKind::Static);
        assert_eq!(array.dimensions, vec![Some((1, 0)), Some((3, 0))]);
        assert_eq!(array.element_type.kind, "logic");
        assert_eq!(array.element_type.width, Some(8));
    }

    #[test]
    fn projects_packed_member_ranges_and_nested_layout() {
        let element = ty(0, TypeKind::Integral, 1);
        let mut byte = ty(1, TypeKind::PackedArray, 8);
        byte.element_type_id = Some(0);
        byte.range_count = 1;
        let mut record = ty(2, TypeKind::PackedStruct, 8);
        record.display_name = "record_t".to_owned();
        record.member_count = 1;
        let types = [element, byte, record];
        let ranges = [TypeRange {
            left: 7,
            right: 0,
            kind: TypeRangeKind::Packed,
        }];
        let members = [TypeMember {
            initializer_constant_id: None,
            name: "data".to_owned(),
            type_id: 1,
            bit_offset: 0,
            bit_width: 8,
        }];
        let projector = SlangTypeProjector {
            constants: &[],
            types: types.iter().map(|ty| (ty.id, ty)).collect(),
            ranges: &ranges,
            members: &members,
        };

        let projection = projector.project(2).expect("project packed struct");
        match &projection.descriptor.shape {
            TypeShape::Aggregate(layout) => match &layout.members[0].descriptor.shape {
                TypeShape::PackedAtom { ranges } => {
                    assert_eq!(ranges, &[PackedRange { left: 7, right: 0 }]);
                }
                shape => panic!("expected packed member descriptor, got {shape:?}"),
            },
            shape => panic!("expected aggregate descriptor, got {shape:?}"),
        }
        let packed = projection.packed_members.expect("packed members");
        assert_eq!(packed[0].name, "data");
        assert_eq!(
            packed[0].packed_ranges,
            vec![PackedRange { left: 7, right: 0 }]
        );
        let layout = projection.aggregate_layout.expect("aggregate layout");
        assert_eq!(layout.kind, AggregateKind::PackedStruct);
        assert_eq!(layout.type_identity.as_deref(), Some("type#2"));
        assert_eq!(layout.members[0].ty.kind, "logic");
        assert_eq!(layout.members[0].ty.width, Some(8));
    }

    #[test]
    fn nested_aggregate_uses_one_canonical_recursive_layout() {
        let leaf = ty(0, TypeKind::Integral, 8);
        let mut inner = ty(1, TypeKind::UnpackedStruct, 0);
        inner.member_count = 1;
        let mut outer = ty(2, TypeKind::UnpackedStruct, 0);
        outer.member_start = 1;
        outer.member_count = 1;
        let types = [leaf, inner, outer];
        let members = [
            TypeMember {
                initializer_constant_id: None,
                name: "leaf".to_owned(),
                type_id: 0,
                bit_offset: 0,
                bit_width: 8,
            },
            TypeMember {
                initializer_constant_id: None,
                name: "inner".to_owned(),
                type_id: 1,
                bit_offset: 0,
                bit_width: 0,
            },
        ];
        let projector = SlangTypeProjector {
            constants: &[],
            types: types.iter().map(|ty| (ty.id, ty)).collect(),
            ranges: &[],
            members: &members,
        };

        let projection = projector.project(2).expect("project nested aggregate");
        let TypeShape::Aggregate(layout) = &projection.descriptor.shape else {
            panic!("expected aggregate descriptor");
        };
        let nested = &layout.members[0];
        assert!(nested.aggregate.is_none());
        assert_eq!(
            nested
                .aggregate_layout()
                .expect("descriptor-backed nested layout")
                .type_id,
            Some(TypeId(1))
        );
    }

    #[test]
    fn recursive_type_projection_is_depth_bounded() {
        let aggregate_count = MAX_RECURSIVE_TYPE_DEPTH + 1;
        let mut types = Vec::with_capacity(aggregate_count + 1);
        let mut members = Vec::with_capacity(aggregate_count);
        types.push(ty(0, TypeKind::Integral, 1));
        for index in 0..aggregate_count {
            let id = u64::try_from(index + 1).expect("test type id");
            let mut aggregate = ty(id, TypeKind::UnpackedStruct, 0);
            aggregate.member_start = index as u64;
            aggregate.member_count = 1;
            types.push(aggregate);
            members.push(TypeMember {
                initializer_constant_id: None,
                name: format!("level_{index}"),
                type_id: if index == 0 { 0 } else { id - 1 },
                bit_offset: 0,
                bit_width: 0,
            });
        }
        let projector = SlangTypeProjector {
            constants: &[],
            types: types.iter().map(|ty| (ty.id, ty)).collect(),
            ranges: &[],
            members: &members,
        };

        let error = projector
            .project(aggregate_count as u64)
            .expect_err("deep recursive descriptors must be rejected");
        assert!(error.contains("recursive type depth limit"));
    }

    #[test]
    fn packed_union_and_fixed_array_bit_sizes_use_overlaid_width() {
        let byte = ty(0, TypeKind::Integral, 8);
        let word = ty(1, TypeKind::Integral, 16);
        let mut union = ty(2, TypeKind::PackedUnion, 16);
        union.member_count = 2;
        let types = [byte, word, union];
        let members = [
            TypeMember {
                initializer_constant_id: None,
                name: "byte".to_owned(),
                type_id: 0,
                bit_offset: 0,
                bit_width: 8,
            },
            TypeMember {
                initializer_constant_id: None,
                name: "word".to_owned(),
                type_id: 1,
                bit_offset: 0,
                bit_width: 16,
            },
        ];
        let projector = SlangTypeProjector {
            constants: &[],
            types: types.iter().map(|ty| (ty.id, ty)).collect(),
            ranges: &[],
            members: &members,
        };
        let union = projector
            .project(2)
            .expect("project packed union")
            .descriptor;
        assert_eq!(union.fixed_size_bits(), Some(16));
        let fixed = TypeDescriptor {
            two_state: false,
            id: TypeId(3),
            name: "union_array".to_owned(),
            info: union.info.clone(),
            shape: TypeShape::FixedArray {
                dimensions: vec![(0, 2)],
                element: Box::new(union),
            },
        };
        assert_eq!(fixed.fixed_size_bits(), Some(48));
    }

    #[test]
    fn anonymous_unpacked_aggregates_keep_canonical_type_identity() {
        let record = ty(7, TypeKind::UnpackedStruct, 0);
        let types = [record];
        let projector = SlangTypeProjector {
            constants: &[],
            types: types.iter().map(|ty| (ty.id, ty)).collect(),
            ranges: &[],
            members: &[],
        };

        let layout = projector
            .project(7)
            .expect("project unpacked struct")
            .aggregate_layout
            .expect("unpacked layout");
        assert_eq!(layout.type_identity.as_deref(), Some("type#7"));
    }

    #[test]
    fn structurally_equal_aggregates_keep_distinct_canonical_identities() {
        let mut first = ty(7, TypeKind::UnpackedStruct, 0);
        first.display_name = "record_t".to_owned();
        let mut second = ty(8, TypeKind::UnpackedStruct, 0);
        second.display_name = "record_t".to_owned();
        let types = [first, second];
        let projector = SlangTypeProjector {
            constants: &[],
            types: types.iter().map(|ty| (ty.id, ty)).collect(),
            ranges: &[],
            members: &[],
        };

        let first = projector
            .project(7)
            .expect("project first unpacked struct")
            .aggregate_layout
            .expect("first unpacked layout");
        let second = projector
            .project(8)
            .expect("project second unpacked struct")
            .aggregate_layout
            .expect("second unpacked layout");
        assert_ne!(first.type_identity, second.type_identity);
        assert_eq!(
            first.type_identity,
            projector
                .project(7)
                .expect("project alias of first aggregate")
                .aggregate_layout
                .expect("alias layout")
                .type_identity
        );
    }

    #[test]
    fn projects_packed_integral_associative_index() {
        let element = ty(0, TypeKind::Integral, 16);
        let bit = ty(1, TypeKind::Integral, 1);
        let mut index = ty(2, TypeKind::PackedArray, 8);
        index.element_type_id = Some(1);
        index.range_count = 1;
        let mut associative = ty(3, TypeKind::AssociativeArray, 0);
        associative.is_fixed_size = false;
        associative.element_type_id = Some(0);
        associative.index_type_id = Some(2);
        let types = [element, bit, index, associative];
        let ranges = [TypeRange {
            left: 7,
            right: 0,
            kind: TypeRangeKind::Packed,
        }];
        let projector = SlangTypeProjector {
            constants: &[],
            types: types.iter().map(|ty| (ty.id, ty)).collect(),
            ranges: &ranges,
            members: &[],
        };

        let projection = projector.project(3).expect("project associative array");
        let array = projection.array.expect("array metadata");
        assert_eq!(
            array.kind,
            ArrayKind::Associative(AssociativeIndex::Integral {
                width: 8,
                signed: false,
                two_state: false,
            })
        );
    }
}
