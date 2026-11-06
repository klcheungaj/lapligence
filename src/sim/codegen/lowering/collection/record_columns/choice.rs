//! Which record types a design stores as columns.
//!
//! A structure holding a member array of at least
//! [`LLG_RECORD_COLUMN_MIN_CELLS`] cells, at any depth, keeps that array in one
//! column: a selection is one cell lookup and generated code stays flat. Below
//! that size each cell is its own leaf, whose code is small and whose readers
//! need no lookup. Nothing else selects the form, and a use the column form
//! cannot lower is an error at its source location, never a second lowering
//! in the other form. Above [`LLG_DENSE_FIXED_ARRAY_CELLS`] cells the sparse
//! form makes the same choice ([`Codegen::record_column_layout`]).
use super::*;
use crate::sim::ir::LLG_RECORD_COLUMN_MIN_CELLS;

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

/// Whether a structure holds a member array of the minimum size, directly
/// or in a nested structure or element.
fn holds_column(descriptor: &TypeDescriptor) -> bool {
    match &descriptor.shape {
        TypeShape::FixedArray {
            dimensions,
            element,
        } => {
            fixed_array_cell_count(dimensions)
                .map_or(true, |cells| cells >= LLG_RECORD_COLUMN_MIN_CELLS)
                || holds_column(element)
        }
        TypeShape::Aggregate(layout) if layout.kind == AggregateKind::UnpackedStruct => layout
            .members
            .iter()
            .any(|member| holds_column(&member.descriptor)),
        _ => false,
    }
}

/// Refuse `descriptor` and every structure nested in it or in its elements.
fn refuse_with_nested(descriptor: &TypeDescriptor, refused: &mut HashSet<TypeId>) {
    match &descriptor.shape {
        TypeShape::FixedArray { element, .. } => refuse_with_nested(element, refused),
        TypeShape::Aggregate(layout) if layout.kind == AggregateKind::UnpackedStruct => {
            refused.insert(descriptor.id);
            for member in &layout.members {
                refuse_with_nested(&member.descriptor, refused);
            }
        }
        _ => {}
    }
}

/// Every structure of `descriptor` or of its elements: those that take the
/// column form in `chosen`, those that cannot in `refused`.
fn visit_records<'a>(
    descriptor: &'a TypeDescriptor,
    chosen: &mut Vec<&'a TypeDescriptor>,
    refused: &mut HashSet<TypeId>,
    seen: &mut HashSet<TypeId>,
) {
    match &descriptor.shape {
        TypeShape::FixedArray { element, .. } => visit_records(element, chosen, refused, seen),
        TypeShape::Aggregate(layout)
            if layout.kind == AggregateKind::UnpackedStruct && seen.insert(descriptor.id) =>
        {
            if !holds_column(descriptor) {
                return;
            }
            if columns_hold(descriptor) {
                chosen.push(descriptor);
                return;
            }
            refuse_with_nested(descriptor, refused);
        }
        _ => {}
    }
}

/// The record types of the design stored as columns. A type has one form
/// wherever it is declared, so a structure nesting a refused type is refused
/// too, and so is what it nests.
pub(in super::super::super) fn record_column_choice(db: &Db) -> HashSet<TypeId> {
    let mut roots = Vec::new();
    let mut refused = HashSet::new();
    let mut seen = HashSet::new();
    for node in db.node_ids() {
        if let Some(descriptor) = db.type_descriptor(node) {
            visit_records(descriptor, &mut roots, &mut refused, &mut seen);
        }
    }
    let mut chosen: Vec<HashSet<TypeId>> = roots
        .into_iter()
        .map(|root| {
            let mut nested = HashSet::new();
            choose_with_nested(root, &mut nested);
            nested
        })
        .collect();
    loop {
        let (kept, dropped): (Vec<_>, Vec<_>) = chosen
            .into_iter()
            .partition(|nested| nested.is_disjoint(&refused));
        chosen = kept;
        if dropped.is_empty() {
            return chosen.into_iter().flatten().collect();
        }
        refused.extend(dropped.into_iter().flatten());
    }
}
