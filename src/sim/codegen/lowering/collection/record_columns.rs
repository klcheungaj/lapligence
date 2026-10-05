//! Column layout for large unpacked records (RTL-101).
//!
//! A record whose member arrays exceed the dense-cell threshold, or whose
//! flattened payload exceeds packed capacity, keeps each member array as one
//! fixed-array column. Scalar members stay ordinary leaf signals. Columns use
//! the RTL-002 descriptor storage and transport, so generated code scales
//! with the declaration rather than with its cell count.
use super::fixed_values::{
    fixed_array_cell_count, fixed_constant_slice, fixed_path_descriptor, fixed_width,
    fixed_width_bits,
};
use super::*;
use crate::sim::ir::LLG_DENSE_FIXED_ARRAY_CELLS;

#[cfg(test)]
mod tests;

/// One stored column of a record value, in declaration order.
#[derive(Clone)]
pub(in crate::sim::codegen) enum RecordColumn {
    /// Descriptor storage of a member array.
    Array(usize),
    /// One-cell descriptor storage of a scalar leaf.
    Cell(usize),
    /// Module storage of a real, string or chandle member.
    Leaf(AggregateMemberInfo),
}

/// A record value as its columns, with paths relative to `descriptor`.
#[derive(Clone)]
pub(in crate::sim::codegen) struct RecordValue {
    pub(in crate::sim::codegen) descriptor: TypeDescriptor,
    pub(in crate::sim::codegen) columns: Vec<(Vec<AggregatePathPart>, RecordColumn)>,
}

/// Whether a record descriptor uses column layout: an unpacked structure
/// wider than packed capacity, or one with a member array (directly or in a
/// nested unpacked structure) above the dense-cell threshold.
pub(super) fn record_column_layout(descriptor: &TypeDescriptor) -> bool {
    let TypeShape::Aggregate(layout) = &descriptor.shape else {
        return false;
    };
    if column_tagged_union(descriptor) {
        return true;
    }
    layout.kind == AggregateKind::UnpackedStruct
        && (fixed_width_bits(descriptor).is_some_and(|width| width > u64::from(LLG_MAX_WIDTH))
            || layout
                .members
                .iter()
                .any(|member| large_member_array(&member.descriptor)))
}

/// Whether values of this type use column layout (records or tagged unions).
pub(in super::super) fn record_column_layout_type(descriptor: &TypeDescriptor) -> bool {
    record_column_layout(descriptor)
}

/// A finite tagged union whose tag and payload exceed packed capacity: its
/// tag is one cell and every member keeps its own columns.
pub(in super::super) fn column_tagged_union(descriptor: &TypeDescriptor) -> bool {
    matches!(&descriptor.shape, TypeShape::Aggregate(layout) if layout.kind == AggregateKind::TaggedUnion)
        && fixed_width_bits(descriptor).is_some_and(|width| width > u64::from(LLG_MAX_WIDTH))
}

fn large_member_array(descriptor: &TypeDescriptor) -> bool {
    match &descriptor.shape {
        // An extent beyond the cell limit is large as well; column
        // collection then reports the limit instead of expanding cells.
        TypeShape::FixedArray { dimensions, .. } => fixed_array_cell_count(dimensions)
            .map_or(true, |cells| cells > LLG_DENSE_FIXED_ARRAY_CELLS),
        TypeShape::Aggregate(layout) if layout.kind == AggregateKind::UnpackedStruct => layout
            .members
            .iter()
            .any(|member| large_member_array(&member.descriptor)),
        _ => false,
    }
}

/// Display label of a member path (`a`, `inner.a`).
fn record_member_label(path: &[AggregatePathPart]) -> String {
    path.iter()
        .map(|part| match part {
            AggregatePathPart::Member(name) => name.clone(),
            AggregatePathPart::Index(index) => format!("[{index}]"),
        })
        .collect::<Vec<_>>()
        .join(".")
}

/// Whether a column leaf is a one-cell scalar column rather than a member
/// array.
pub(in super::super) fn record_cell_leaf(leaf: &AggregateMemberInfo) -> bool {
    leaf.array.is_some() && !matches!(leaf.member.descriptor.shape, TypeShape::FixedArray { .. })
}

/// Members along `path` from the record root, outermost first.
fn path_members<'a>(
    root: &'a TypeDescriptor,
    path: &[AggregatePathPart],
) -> Option<Vec<&'a AggregateMember>> {
    let mut descriptor = root;
    let mut members = Vec::with_capacity(path.len());
    for part in path {
        let AggregatePathPart::Member(name) = part else {
            return None;
        };
        let TypeShape::Aggregate(layout) = &descriptor.shape else {
            return None;
        };
        let member = layout.members.iter().find(|member| &member.name == name)?;
        members.push(member);
        descriptor = &member.descriptor;
    }
    Some(members)
}

/// A one-member structure holding `member`, so the shared default builder
/// can evaluate one member of a record that is itself too wide for it.
fn single_member_record(member: &AggregateMember, clear: bool) -> TypeDescriptor {
    let mut member = member.clone();
    member.two_state |= clear;
    TypeDescriptor {
        two_state: member.two_state,
        id: member.descriptor.id,
        name: member.name.clone(),
        info: member.descriptor.info.clone(),
        shape: TypeShape::Aggregate(crate::core::db::AggregateLayout {
            kind: AggregateKind::UnpackedStruct,
            type_identity: None,
            type_id: None,
            members: vec![member],
        }),
    }
}

/// The column shape of one record leaf, used to allocate temporaries.
enum ColumnShape {
    Packed {
        width: u32,
        signed: bool,
        two_state: bool,
    },
    Array(TypeDescriptor),
}

/// Column shapes of a record type in the declaration order used by module
/// storage (`collect_aggregate_descriptor_leaves` with columns).
fn column_shapes(
    descriptor: &TypeDescriptor,
    prefix: &[AggregatePathPart],
    two_state: bool,
    out: &mut Vec<(Vec<AggregatePathPart>, ColumnShape)>,
) -> Result<(), String> {
    if column_tagged_union(descriptor) {
        let TypeShape::Aggregate(layout) = &descriptor.shape else {
            unreachable!("tagged union shape");
        };
        let tag = layout
            .tag_bits()
            .filter(|width| *width > 0)
            .ok_or_else(|| {
                format!(
                    "tagged union `{}` has no representable tag",
                    aggregate_path_suffix(prefix)
                )
            })?;
        out.push((
            prefix.to_vec(),
            ColumnShape::Packed {
                width: tag,
                signed: false,
                two_state: descriptor.two_state,
            },
        ));
        for member in &layout.members {
            if matches!(&member.descriptor.shape, TypeShape::Opaque { kind } if kind == "Void") {
                continue;
            }
            let mut path = prefix.to_vec();
            path.push(AggregatePathPart::Member(member.name.clone()));
            column_shapes(&member.descriptor, &path, member.two_state, out)?;
        }
        return Ok(());
    }
    match &descriptor.shape {
        TypeShape::FixedArray { .. } => {
            out.push((prefix.to_vec(), ColumnShape::Array(descriptor.clone())));
            Ok(())
        }
        TypeShape::Aggregate(layout) if layout.kind == AggregateKind::UnpackedStruct => {
            for member in &layout.members {
                let mut path = prefix.to_vec();
                path.push(AggregatePathPart::Member(member.name.clone()));
                column_shapes(&member.descriptor, &path, member.two_state, out)?;
            }
            Ok(())
        }
        TypeShape::PackedAtom { .. } | TypeShape::Aggregate(_) => {
            let width = fixed_width(descriptor).ok_or_else(|| {
                format!(
                    "record member `{}` has no packed width",
                    aggregate_path_suffix(prefix)
                )
            })?;
            out.push((
                prefix.to_vec(),
                ColumnShape::Packed {
                    width,
                    signed: descriptor.info.signed,
                    two_state,
                },
            ));
            Ok(())
        }
        _ => Err(format!(
            "record member `{}` is not integral; a temporary column-layout record value supports integral members only",
            aggregate_path_suffix(prefix)
        )),
    }
}

impl Codegen<'_> {
    /// Allocate the fixed-array column for a record member array.
    pub(super) fn collect_record_column(
        &mut self,
        path: &str,
        object: NodeId,
        object_name: &str,
        member: &AggregateMember,
        descriptor: &TypeDescriptor,
        member_path: &[AggregatePathPart],
    ) -> Result<AggregateMemberInfo, String> {
        let suffix = aggregate_path_suffix(member_path);
        let TypeShape::FixedArray {
            dimensions,
            element,
        } = &descriptor.shape
        else {
            return Err(format!(
                "record column `{object_name}.{suffix}` in `{path}` is not a fixed array"
            ));
        };
        let elem_width = fixed_width(element).ok_or_else(|| {
            format!(
                "record member array `{object_name}.{suffix}` in `{path}` needs integral elements within the packed limit of {LLG_MAX_WIDTH} bits for column layout"
            )
        })?;
        let total = fixed_array_cell_count(dimensions).map_err(|error| {
            format!("record member array `{object_name}.{suffix}` in `{path}`: {error}")
        })?;
        let root = self
            .query_descriptor(object)
            .cloned()
            .ok_or("column-layout record has no type")?;
        let chain = path_members(&root, member_path)
            .ok_or_else(|| format!("record column path `{suffix}` in `{path}` is unresolved"))?;
        if chain.iter().any(|member| member.initializer.is_some()) {
            // A member initializer may give each cell a different value; a
            // column only has a uniform element default.
            return Err(format!(
                "member initializer on record member array `{object_name}.{suffix}` in `{path}` is not supported with column layout"
            ));
        }
        let two_state = element.two_state || chain.iter().any(|member| member.two_state);
        let c_name = self.c_name("G", path, &[object_name, &suffix]);
        let mut hdl_name = self.waveform_name(object);
        hdl_name.push('\u{1f}');
        hdl_name.push_str(&record_member_label(member_path));
        let ir = self.model.arrays.len();
        self.model.arrays.push(crate::sim::ir::IrArray {
            activation: false,
            descriptor: true,
            net: None,
            net_elements: Vec::new(),
            element_default: Self::fixed_descriptor_default(element),
            element_uninitialized: Self::fixed_element_uninitialized(element),
            c_name: c_name.clone(),
            hdl_name,
            elem_width,
            signed: element.info.signed,
            two_state,
            real: false,
            shortreal: false,
            dims: dimensions.clone(),
            total,
        });
        self.record_columns = true;
        Ok(AggregateMemberInfo {
            member: leaf_member(member, descriptor),
            signal: None,
            object: None,
            array: Some(ArrayInfo {
                global: c_name,
                elem_width,
                signed: element.info.signed,
                real: false,
                shortreal: false,
                is_net: false,
                dims: dimensions.clone(),
                init: None,
                ir,
            }),
            path: member_path.to_vec(),
        })
    }

    /// Allocate the one-cell column of a scalar record leaf. Every integral
    /// leaf of a column-layout record is descriptor storage, so whole-record
    /// copies, comparisons and subroutine transport are uniform per column.
    pub(super) fn collect_record_cell(
        &mut self,
        path: &str,
        object: NodeId,
        object_name: &str,
        member: &AggregateMember,
        descriptor: &TypeDescriptor,
        member_path: &[AggregatePathPart],
    ) -> Result<AggregateMemberInfo, String> {
        let suffix = aggregate_path_suffix(member_path);
        let width = fixed_width(descriptor).ok_or_else(|| {
            format!("record member `{object_name}.{suffix}` in `{path}` has no packed width")
        })?;
        let root = self
            .query_descriptor(object)
            .cloned()
            .ok_or("column-layout record has no type")?;
        let default = Self::record_leaf_default(&root, member_path, false).ok_or_else(|| {
            format!("record member `{object_name}.{suffix}` in `{path}` has no fixed default")
        })?;
        let chain = path_members(&root, member_path)
            .ok_or_else(|| format!("record member path `{suffix}` in `{path}` is unresolved"))?;
        let two_state = descriptor.two_state || chain.iter().any(|member| member.two_state);
        let c_name = self.c_name("G", path, &[object_name, &suffix]);
        let mut hdl_name = self.waveform_name(object);
        hdl_name.push('\u{1f}');
        hdl_name.push_str(&record_member_label(member_path));
        let ir = self.model.arrays.len();
        self.model.arrays.push(crate::sim::ir::IrArray {
            activation: false,
            descriptor: true,
            net: None,
            net_elements: Vec::new(),
            element_default: Some(default),
            element_uninitialized: Self::fixed_descriptor_uninitialized(descriptor)
                .filter(|_| matches!(descriptor.shape, TypeShape::Aggregate(_))),
            c_name: c_name.clone(),
            hdl_name,
            elem_width: width,
            signed: descriptor.info.signed,
            two_state,
            real: false,
            shortreal: false,
            dims: vec![(0, 0)],
            total: 1,
        });
        self.record_columns = true;
        Ok(AggregateMemberInfo {
            member: leaf_member(member, descriptor),
            signal: None,
            object: None,
            array: Some(ArrayInfo {
                global: c_name,
                elem_width: width,
                signed: descriptor.info.signed,
                real: false,
                shortreal: false,
                is_net: false,
                dims: vec![(0, 0)],
                init: None,
                ir,
            }),
            path: member_path.to_vec(),
        })
    }

    /// The tag cell and member columns of a tagged union too wide for one
    /// packed value, rooted at `member_path` of `object`.
    pub(super) fn collect_tagged_columns(
        &mut self,
        path: &str,
        object: NodeId,
        object_name: &str,
        member: &AggregateMember,
        descriptor: &TypeDescriptor,
        member_path: &[AggregatePathPart],
        leaves: &mut Vec<AggregateMemberInfo>,
    ) -> Result<(), String> {
        let TypeShape::Aggregate(layout) = &descriptor.shape else {
            return Err(format!(
                "tagged union `{object_name}` in `{path}` has no layout"
            ));
        };
        let width = layout
            .tag_bits()
            .filter(|width| *width > 0)
            .ok_or_else(|| {
                format!("tagged union `{object_name}` in `{path}` has no representable tag")
            })?;
        // `$` cannot start an identifier, so the tag never names a member.
        let suffix = if member_path.is_empty() {
            "$tag".to_owned()
        } else {
            format!("{}__$tag", aggregate_path_suffix(member_path))
        };
        let c_name = self.c_name("G", path, &[object_name, &suffix]);
        let mut hdl_name = self.waveform_name(object);
        if !member_path.is_empty() {
            hdl_name.push('\u{1f}');
            hdl_name.push_str(&record_member_label(member_path));
        }
        hdl_name.push_str("\u{1f}tag");
        let ir = self.model.arrays.len();
        self.model.arrays.push(crate::sim::ir::IrArray {
            activation: false,
            descriptor: true,
            net: None,
            net_elements: Vec::new(),
            element_default: None,
            element_uninitialized: None,
            c_name: c_name.clone(),
            hdl_name,
            elem_width: width,
            signed: false,
            two_state: descriptor.two_state,
            real: false,
            shortreal: false,
            dims: vec![(0, 0)],
            total: 1,
        });
        let mut tag = member.clone();
        tag.descriptor = TypeDescriptor {
            two_state: descriptor.two_state,
            id: descriptor.id,
            name: format!("{} tag", descriptor.name),
            info: crate::core::model::TypeInfo {
                kind: "logic".to_owned(),
                width: Some(width),
                signed: false,
                type_name: None,
            },
            shape: TypeShape::PackedAtom { ranges: Vec::new() },
        };
        leaves.push(AggregateMemberInfo {
            member: leaf_member(&tag, &tag.descriptor),
            signal: None,
            object: None,
            array: Some(ArrayInfo {
                global: c_name,
                elem_width: width,
                signed: false,
                real: false,
                shortreal: false,
                is_net: false,
                dims: vec![(0, 0)],
                init: None,
                ir,
            }),
            path: member_path.to_vec(),
        });
        for nested in &layout.members {
            if matches!(&nested.descriptor.shape, TypeShape::Opaque { kind } if kind == "Void") {
                continue;
            }
            let mut nested_path = member_path.to_vec();
            nested_path.push(AggregatePathPart::Member(nested.name.clone()));
            self.collect_aggregate_descriptor_leaves(
                path,
                object,
                object_name,
                nested,
                &nested.descriptor,
                &nested_path,
                None,
                true,
                leaves,
            )?;
        }
        self.record_columns = true;
        Ok(())
    }

    /// Default of the leaf at `path`, evaluated on the outermost member that
    /// fits packed capacity.
    fn record_leaf_default(
        root: &TypeDescriptor,
        path: &[AggregatePathPart],
        clear: bool,
    ) -> Option<IrConst> {
        let (first, rest) = path.split_first()?;
        let AggregatePathPart::Member(name) = first else {
            return None;
        };
        let TypeShape::Aggregate(layout) = &root.shape else {
            return None;
        };
        let member = layout.members.iter().find(|member| &member.name == name)?;
        if fixed_width(&member.descriptor).is_none() {
            if member.initializer.is_some() {
                return None;
            }
            return Self::record_leaf_default(&member.descriptor, rest, clear || member.two_state);
        }
        let record = single_member_record(member, clear);
        let default = Self::fixed_descriptor_default(&record)?;
        let (leaf, offset) = fixed_path_descriptor(&record, path)?;
        let width = fixed_width(&leaf)?;
        Some(fixed_constant_slice(
            &default,
            offset,
            width,
            leaf.info.signed,
        ))
    }

    /// Storage for one member-array column of a record value: lexical
    /// activation storage, or persistent descriptor storage for a static
    /// subroutine.
    pub(in super::super) fn record_temporary_array(
        &mut self,
        path: &str,
        descriptor: &TypeDescriptor,
        activation: bool,
    ) -> Result<usize, String> {
        let TypeShape::FixedArray {
            dimensions,
            element,
        } = &descriptor.shape
        else {
            return Err(format!("record column in `{path}` is not a fixed array"));
        };
        let elem_width = Self::fixed_descriptor_width(element)
            .ok_or_else(|| format!("record column element in `{path}` exceeds packed capacity"))?;
        let total = fixed_array_cell_count(dimensions)?;
        let ir = self.model.arrays.len();
        let c_name = if activation {
            self.new_fn_name(path, "record_column")
        } else {
            format!("S_llg_record_{ir}")
        };
        self.model.arrays.push(crate::sim::ir::IrArray {
            activation,
            descriptor: !activation,
            net: None,
            net_elements: Vec::new(),
            element_default: Self::fixed_descriptor_default(element),
            element_uninitialized: Self::fixed_element_uninitialized(element),
            c_name,
            hdl_name: String::new(),
            elem_width,
            signed: element.info.signed,
            two_state: element.two_state || descriptor.two_state,
            real: false,
            shortreal: false,
            dims: dimensions.clone(),
            total,
        });
        Ok(ir)
    }

    /// The array an element select addresses: a record column for
    /// `r.a[i]`, otherwise the storage of its base.
    pub(in super::super) fn select_array_of(
        &self,
        select: NodeId,
        base: NodeId,
    ) -> Option<&ArrayInfo> {
        self.record_column_select(select)
            .map(|(column, _)| column)
            .or_else(|| self.array_of(base))
    }

    /// One-cell column for a scalar leaf of a record value, holding
    /// `default` until written.
    pub(in super::super) fn record_temporary_cell(
        &mut self,
        path: &str,
        shape: (u32, bool, bool),
        default: Option<IrConst>,
        activation: bool,
    ) -> usize {
        let (width, signed, two_state) = shape;
        let ir = self.model.arrays.len();
        let c_name = if activation {
            self.new_fn_name(path, "record_cell")
        } else {
            format!("S_llg_record_{ir}")
        };
        self.model.arrays.push(crate::sim::ir::IrArray {
            activation,
            descriptor: !activation,
            net: None,
            net_elements: Vec::new(),
            element_default: default,
            element_uninitialized: None,
            c_name,
            hdl_name: String::new(),
            elem_width: width,
            signed,
            two_state,
            real: false,
            shortreal: false,
            dims: vec![(0, 0)],
            total: 1,
        });
        ir
    }

    /// Allocate the columns of a record value of type `descriptor`.
    pub(in super::super) fn allocate_record_columns(
        &mut self,
        path: &str,
        descriptor: &TypeDescriptor,
        activation: bool,
    ) -> Result<RecordValue, String> {
        let mut shapes = Vec::new();
        column_shapes(descriptor, &[], descriptor.two_state, &mut shapes)?;
        let mut columns = Vec::with_capacity(shapes.len());
        for (member_path, shape) in shapes {
            let column = match shape {
                ColumnShape::Packed {
                    width,
                    signed,
                    two_state,
                } => {
                    let default = Self::record_leaf_default(descriptor, &member_path, false);
                    RecordColumn::Cell(self.record_temporary_cell(
                        path,
                        (width, signed, two_state),
                        default,
                        activation,
                    ))
                }
                ColumnShape::Array(array) => {
                    RecordColumn::Array(self.record_temporary_array(path, &array, activation)?)
                }
            };
            columns.push((member_path, column));
        }
        Ok(RecordValue {
            descriptor: descriptor.clone(),
            columns,
        })
    }

    /// Declarations of the lexical columns of `value`.
    pub(in super::super) fn declare_record_columns(value: &RecordValue) -> Vec<IrStmt> {
        value
            .columns
            .iter()
            .filter_map(|(_, column)| match column {
                RecordColumn::Array(array) | RecordColumn::Cell(array) => {
                    Some(IrStmt::FixedArrayDeclare(*array))
                }
                RecordColumn::Leaf(_) => None,
            })
            .collect()
    }

    /// Whether values of this type use column layout.
    pub(in super::super) fn column_layout_descriptor(descriptor: &TypeDescriptor) -> bool {
        record_column_layout(descriptor)
    }

    /// Column storage of `root` at member `path`: a module record column or
    /// a member-array column of a subroutine record value.
    pub(in super::super) fn record_column_at(
        &self,
        root: NodeId,
        path: &[AggregatePathPart],
    ) -> Option<&ArrayInfo> {
        if let Some(value) = self.activation_records.get(&root) {
            return value
                .columns
                .iter()
                .find_map(|(column_path, column)| match column {
                    RecordColumn::Array(array) if column_path == path => {
                        self.record_array_infos.get(array)
                    }
                    _ => None,
                });
        }
        self.unpacked_aggregates
            .get(&root)?
            .leaves
            .iter()
            .find(|leaf| leaf.path == path && !record_cell_leaf(leaf))?
            .array
            .as_ref()
    }

    /// The declaration and member path of a reference into a subroutine
    /// record value (formal, result or local).
    pub(in super::super) fn activation_record_path(
        &self,
        node: NodeId,
    ) -> Option<(NodeId, Vec<AggregatePathPart>)> {
        if self.activation_records.is_empty() {
            return None;
        }
        let root = |node: NodeId| {
            let node = self.canonical_func_target(node).unwrap_or(node);
            self.activation_records.contains_key(&node).then_some(node)
        };
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => root(*target).map(|target| (target, Vec::new())),
            NodeKind::Expr(ExprKind::HierPath { parts, refs }) => {
                let (index, target) = refs.iter().enumerate().find_map(|(index, target)| {
                    target.and_then(root).map(|target| (index, target))
                })?;
                Some((
                    target,
                    parts[index + 1..]
                        .iter()
                        .cloned()
                        .map(AggregatePathPart::Member)
                        .collect(),
                ))
            }
            NodeKind::Expr(ExprKind::Cast { operand, .. }) => self.activation_record_path(*operand),
            _ => root(node).map(|target| (target, Vec::new())),
        }
    }

    /// The column a whole member reference (`r.a`, `r.s.a`) names.
    pub(in super::super) fn record_column_array(&self, node: NodeId) -> Option<&ArrayInfo> {
        if !self.record_columns
            || !matches!(self.kind(node), NodeKind::Expr(ExprKind::HierPath { .. }))
        {
            return None;
        }
        let (root, path) = self
            .activation_record_path(node)
            .or_else(|| self.unpacked_path_for_expr(node))?;
        self.record_column_at(root, &path)
    }

    /// The column and element indices of an element select on a record
    /// member array (`r.a[i]`). Slang flattens successive element indices
    /// into one select whose owned path names the record and its members.
    pub(in super::super) fn record_column_select(
        &self,
        node: NodeId,
    ) -> Option<(&ArrayInfo, &[NodeId])> {
        if !self.record_columns {
            return None;
        }
        let NodeKind::Expr(ExprKind::ArraySelect { base, indices }) = self.kind(node) else {
            return None;
        };
        if let Some((root, members)) = self.db.array_select_path(node) {
            let root = self.canonical_func_target(root).unwrap_or(root);
            let path = members
                .iter()
                .cloned()
                .map(AggregatePathPart::Member)
                .collect::<Vec<_>>();
            if let Some(column) = self.record_column_at(root, &path) {
                return Some((column, indices.as_slice()));
            }
        }
        self.record_column_array(*base)
            .map(|column| (column, indices.as_slice()))
    }

    /// Whether a subroutine returns a column-layout record.
    pub(in super::super) fn record_return(&self, function: NodeId) -> bool {
        // Decided by type: signature queries can precede column allocation.
        matches!(self.kind(function), NodeKind::FuncTask { ret: Some(_), .. })
            && self
                .query_descriptor(function)
                .is_some_and(record_column_layout)
    }

    /// Whether a formal, result or local is a column-layout record value.
    pub(in super::super) fn record_declaration(&self, node: NodeId) -> bool {
        self.activation_records.contains_key(&node)
    }

    /// Allocate column storage for every column-layout record formal, result
    /// and local of `function`: lexical activations for automatic storage,
    /// persistent descriptors for static storage.
    pub(in super::super) fn prepare_record_function(
        &mut self,
        function: NodeId,
        automatic: bool,
    ) -> Result<(), String> {
        let mut nodes = self
            .func_formals(function)
            .into_iter()
            .map(|(node, _)| (node, automatic))
            .collect::<Vec<_>>();
        let returns = matches!(self.kind(function), NodeKind::FuncTask { ret: Some(_), .. });
        if returns {
            nodes.push((function, automatic));
        }
        if let Some(body) = self.func_body(function) {
            self.record_locals(body, &mut nodes);
        }
        let path = self.instance_path_of(function);
        for (node, activation) in nodes {
            if self.activation_records.contains_key(&node) {
                continue;
            }
            let Some(descriptor) = self.query_descriptor(node).cloned() else {
                continue;
            };
            if !record_column_layout(&descriptor) {
                continue;
            }
            let value = self
                .allocate_record_columns(&path, &descriptor, activation)
                .map_err(|error| {
                    format!(
                        "column-layout record `{}` in `{path}`: {error}",
                        self.node(node).name
                    )
                })?;
            for (_, column) in &value.columns {
                if let RecordColumn::Array(array) = column {
                    let info = ArrayInfo {
                        global: self.model.arrays[*array].c_name.clone(),
                        elem_width: self.model.arrays[*array].elem_width,
                        signed: self.model.arrays[*array].signed,
                        real: false,
                        shortreal: false,
                        is_net: false,
                        dims: self.model.arrays[*array].dims.clone(),
                        init: None,
                        ir: *array,
                    };
                    self.record_array_infos.insert(*array, info);
                }
            }
            self.record_columns = true;
            self.activation_records.insert(node, value);
        }
        Ok(())
    }

    fn record_locals(&self, node: NodeId, out: &mut Vec<(NodeId, bool)>) {
        match self.kind(node) {
            NodeKind::Var { .. } => {
                // Detached member declarations reached through selects have
                // no parent scope and are not locals.
                if self.node(node).parent.is_some()
                    && self
                        .query_descriptor(node)
                        .is_some_and(record_column_layout)
                {
                    let automatic = self.db.variable_lifetime(node) == VariableLifetime::Automatic;
                    out.push((node, automatic));
                }
            }
            NodeKind::FuncTask { .. } => {}
            _ => {
                for child in &self.node(node).children {
                    self.record_locals(*child, out);
                }
            }
        }
    }

    /// The IR formals of a column-layout record formal or result, one per
    /// column in declaration order.
    pub(in super::super) fn record_formal_columns(&self, node: NodeId) -> Vec<usize> {
        self.activation_records
            .get(&node)
            .map(|value| {
                value
                    .columns
                    .iter()
                    .filter_map(|(_, column)| match column {
                        RecordColumn::Array(array) | RecordColumn::Cell(array) => Some(*array),
                        RecordColumn::Leaf(_) => None,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}
