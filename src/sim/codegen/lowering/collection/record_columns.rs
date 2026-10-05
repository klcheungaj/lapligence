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

/// Whether a record descriptor uses column layout: an unpacked structure
/// wider than packed capacity, or one with a member array (directly or in a
/// nested unpacked structure) above the dense-cell threshold.
pub(super) fn record_column_layout(descriptor: &TypeDescriptor) -> bool {
    let TypeShape::Aggregate(layout) = &descriptor.shape else {
        return false;
    };
    layout.kind == AggregateKind::UnpackedStruct
        && (fixed_width_bits(descriptor).is_some_and(|width| width > u64::from(LLG_MAX_WIDTH))
            || layout
                .members
                .iter()
                .any(|member| large_member_array(&member.descriptor)))
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
        let c_name = self.c_name("A", path, &[object_name, &suffix]);
        let mut hdl_name = self.waveform_name(object);
        hdl_name.push('\u{1f}');
        hdl_name.push_str(
            &member_path
                .iter()
                .map(|part| match part {
                    AggregatePathPart::Member(name) => name.clone(),
                    AggregatePathPart::Index(index) => format!("[{index}]"),
                })
                .collect::<Vec<_>>()
                .join("."),
        );
        let ir = self.model.arrays.len();
        self.model.arrays.push(crate::sim::ir::IrArray {
            activation: false,
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

    /// Typed defaults of the scalar leaves of a column-layout record. The
    /// whole record may exceed packed capacity, so each leaf takes its slice
    /// of its immediate member's default (member initializers and two-state
    /// domains included).
    pub(super) fn record_column_leaf_defaults(
        &mut self,
        path: &str,
        node: NodeId,
        leaves: &[AggregateMemberInfo],
    ) -> Result<(), String> {
        let root = self
            .query_descriptor(node)
            .cloned()
            .ok_or("column-layout record has no type")?;
        for leaf in leaves {
            let Some(signal) = &leaf.signal else {
                continue;
            };
            if signal.real {
                continue;
            }
            let default = Self::record_leaf_default(&root, &leaf.path, false).ok_or_else(|| {
                format!(
                    "record member `{}` in `{path}` has no fixed default",
                    aggregate_path_suffix(&leaf.path)
                )
            })?;
            self.model.signals[signal.ir].fixed_default = Some(fixed_constant_slice(
                &default,
                0,
                signal.width,
                signal.signed,
            ));
        }
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

    /// Lexical storage for one column of a temporary record value.
    pub(in super::super) fn record_temporary_array(
        &mut self,
        path: &str,
        descriptor: &TypeDescriptor,
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
        let c_name = self.new_fn_name(path, "record_column");
        self.model.arrays.push(crate::sim::ir::IrArray {
            activation: true,
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

    /// Whether values of this type use column layout.
    pub(in super::super) fn column_layout_descriptor(descriptor: &TypeDescriptor) -> bool {
        record_column_layout(descriptor)
    }

    /// Column storage of `root` at member `path`.
    pub(in super::super) fn record_column_at(
        &self,
        root: NodeId,
        path: &[AggregatePathPart],
    ) -> Option<&ArrayInfo> {
        self.unpacked_aggregates
            .get(&root)?
            .leaves
            .iter()
            .find(|leaf| leaf.path == path)?
            .array
            .as_ref()
    }

    /// The column a whole member reference (`r.a`, `r.s.a`) names.
    pub(in super::super) fn record_column_array(&self, node: NodeId) -> Option<&ArrayInfo> {
        if !self.record_columns
            || !matches!(self.kind(node), NodeKind::Expr(ExprKind::HierPath { .. }))
        {
            return None;
        }
        let (root, path) = self.unpacked_path_for_expr(node)?;
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

    /// Whether `node` names a whole column-layout record.
    pub(in super::super) fn is_column_record(&self, node: NodeId) -> bool {
        self.record_columns
            && self
                .unpacked_aggregate_info(node)
                .is_some_and(|(_, info)| info.columns)
    }
}
