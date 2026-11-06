//! Which record types a design stores as columns below the dense threshold.
//!
//! A module record below the threshold keeps each member-array cell as its
//! own leaf, which costs per access site, not per declaration: a run-time
//! index compares against every cell, and a selection of a record that fits
//! the packed limit reads or writes the whole flattened record. One column
//! makes both one cell lookup and keeps generated code flat, so the choice is
//! made per type from the design's own selections: any selection of a member
//! array of at least [`LLG_RECORD_COLUMN_MIN_CELLS`] cells.
use super::super::native_values::{SelectRoot, SelectStep};
use super::*;
use crate::sim::ir::LLG_RECORD_COLUMN_MIN_CELLS;

/// The cell count of the member array `steps` index in `descriptor`, if any;
/// a select inside a packed leaf is not one.
fn selected_member_cells(descriptor: &TypeDescriptor, steps: &[SelectStep]) -> Option<u64> {
    let mut descriptor = descriptor;
    for step in steps {
        match (step, &descriptor.shape) {
            (SelectStep::Member(name), TypeShape::Aggregate(layout))
                if layout.kind == AggregateKind::UnpackedStruct =>
            {
                descriptor = &layout.members.iter().find(|m| &m.name == name)?.descriptor;
            }
            (
                SelectStep::Constant(_) | SelectStep::Run(_),
                TypeShape::FixedArray { dimensions, .. },
            ) => {
                return fixed_array_cell_count(dimensions).ok();
            }
            _ => return None,
        }
    }
    None
}

/// Whether column layout can hold every member array of this structure: a
/// column needs elements of packed width.
fn columns_hold(descriptor: &TypeDescriptor) -> bool {
    match &descriptor.shape {
        TypeShape::FixedArray { element, .. } => fixed_width(element).is_some(),
        TypeShape::Aggregate(layout) if layout.kind == AggregateKind::UnpackedStruct => layout
            .members
            .iter()
            .all(|member| columns_hold(&member.descriptor)),
        TypeShape::PackedAtom { .. } | TypeShape::Real { .. } | TypeShape::String => true,
        TypeShape::Aggregate(_) => fixed_width(descriptor).is_some(),
        TypeShape::Opaque { kind } => kind == "Chandle",
        _ => false,
    }
}

/// Add `descriptor` and the structures nested in it: a value of the nested
/// type must have one form wherever it is declared.
fn choose_with_nested(descriptor: &TypeDescriptor, chosen: &mut HashSet<TypeId>) {
    if let TypeShape::Aggregate(layout) = &descriptor.shape {
        if layout.kind == AggregateKind::UnpackedStruct {
            chosen.insert(descriptor.id);
            for member in &layout.members {
                choose_with_nested(&member.descriptor, chosen);
            }
        }
    }
}

/// Whether some type of the design is a structure with a fixed-array member,
/// the only case that can be chosen; the common design stops here.
pub(in super::super::super) fn has_record_member_array(db: &Db) -> bool {
    db.node_ids().any(|node| {
        db.type_descriptor(node).is_some_and(|descriptor| {
            matches!(&descriptor.shape, TypeShape::Aggregate(layout)
            if layout.kind == AggregateKind::UnpackedStruct
                && layout.members.iter().any(|member| {
                    matches!(
                        member.descriptor.shape,
                        TypeShape::FixedArray { .. } | TypeShape::Aggregate(_)
                    )
                }))
        })
    })
}

impl Codegen<'_> {
    /// The record types this design's selections move to column layout.
    /// Needs collected storage.
    pub(in super::super::super) fn record_column_choice(&self) -> HashSet<TypeId> {
        let mut chosen = HashSet::new();
        for node in self.db.node_ids() {
            if !matches!(
                self.kind(node),
                NodeKind::Expr(ExprKind::ArraySelect { .. } | ExprKind::BitSelect { .. })
            ) {
                continue;
            }
            let Some((SelectRoot::Module(root), steps)) = self.selected_steps(node) else {
                continue;
            };
            let Some(descriptor) = self.query_descriptor(root) else {
                continue;
            };
            if chosen.contains(&descriptor.id) || self.record_column_layout(descriptor) {
                continue;
            }
            let Some(cells) = selected_member_cells(descriptor, &steps) else {
                continue;
            };
            if (LLG_RECORD_COLUMN_MIN_CELLS..=LLG_DENSE_FIXED_ARRAY_CELLS).contains(&cells)
                && columns_hold(descriptor)
            {
                choose_with_nested(descriptor, &mut chosen);
            }
        }
        chosen
    }
}
