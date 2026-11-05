//! Which record types a design stores as columns below the dense threshold.
//!
//! A module record below the threshold keeps each member-array cell as its
//! own leaf, which costs per access site, not per declaration: a run-time
//! index compares against every cell, and a selection of a record that fits
//! the packed limit reads or writes the whole flattened record. One column
//! makes both one cell lookup. The cost is a hash lookup in place of a direct
//! access, so the choice is made per type from the design's own selections:
//! a record that fits the packed limit on any selection of a member array
//! of at least [`LLG_RECORD_COLUMN_MIN_CELLS`] cells, and any other record
//! only on a run-time index into one. Constant selections of leaf objects are
//! direct and stay as they are.
//!
//! Column layout does not cover every use of a type, so this is a preference:
//! the caller lowers the chosen types and keeps the per-cell model when that
//! lowering fails ([`crate::sim::codegen::lowering`]).
use super::super::native_values::{SelectRoot, SelectStep};
use super::*;
use crate::sim::ir::LLG_RECORD_COLUMN_MIN_CELLS;

/// What one selection does to a member array.
struct MemberArraySelect {
    cells: u64,
    run_time: bool,
}

/// The member array `steps` index in `descriptor`, if any; a select inside a
/// packed leaf is not one.
fn member_array_select(
    descriptor: &TypeDescriptor,
    steps: &[SelectStep],
) -> Option<MemberArraySelect> {
    let mut descriptor = descriptor;
    let mut steps = steps.iter();
    while let Some(step) = steps.next() {
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
                let cells = fixed_array_cell_count(dimensions).ok()?;
                let run_time = std::iter::once(step)
                    .chain(steps.by_ref().take(dimensions.len() - 1))
                    .any(|step| matches!(step, SelectStep::Run(_)));
                return Some(MemberArraySelect { cells, run_time });
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
            let Some(select) = member_array_select(descriptor, &steps) else {
                continue;
            };
            if select.cells >= LLG_RECORD_COLUMN_MIN_CELLS
                && select.cells <= LLG_DENSE_FIXED_ARRAY_CELLS
                && (select.run_time || fixed_width_bits(descriptor).is_some())
                && columns_hold(descriptor)
            {
                choose_with_nested(descriptor, &mut chosen);
            }
        }
        chosen
    }
}
