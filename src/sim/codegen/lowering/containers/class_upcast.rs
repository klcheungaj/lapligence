//! Unpacked arrays of derived-class handles assigned to arrays of base-class
//! handles (decision UVB-D1).

use super::*;

impl Codegen<'_> {
    /// The operand of `node` when `node` is the frontend's implicit
    /// conversion of an unpacked array of derived-class handles to an
    /// unpacked array of base-class handles (decision UVB-D1). Each element
    /// converts as a handle upcast, which keeps the object handle unchanged
    /// (SV 8.15), so the copy reads the operand's elements directly.
    pub(in super::super) fn class_array_upcast_operand(&self, node: NodeId) -> Option<NodeId> {
        let NodeKind::Expr(ExprKind::Cast { operand, .. }) = self.kind(node) else {
            return None;
        };
        if !self.db.is_implicit_conversion(node) {
            return None;
        }
        let operand = *operand;
        self.class_array_upcasts(node, operand).then_some(operand)
    }

    /// Whether the unpacked array type of `source` differs from that of
    /// `target` only in element classes derived from the target's
    /// (decision UVB-D1).
    pub(in super::super) fn class_array_upcasts(&self, target: NodeId, source: NodeId) -> bool {
        let (Some(target), Some(source)) =
            (self.query_descriptor(target), self.query_descriptor(source))
        else {
            return false;
        };
        match (&target.shape, &source.shape) {
            (
                TypeShape::FixedArray { element: t, .. } | TypeShape::Container { element: t, .. },
                TypeShape::FixedArray { element: s, .. } | TypeShape::Container { element: s, .. },
            ) => self.element_upcast(t, s) == Some(true),
            _ => false,
        }
    }

    /// Whether elements of type `source` assign to elements of type `target`
    /// as handles: `Some(true)` when they differ only by derived classes,
    /// `Some(false)` when they are the same type, `None` otherwise. Inner
    /// dimensions must match in kind and size.
    fn element_upcast(&self, target: &TypeDescriptor, source: &TypeDescriptor) -> Option<bool> {
        if target.id == source.id {
            return Some(false);
        }
        match (&target.shape, &source.shape) {
            (TypeShape::Opaque { .. }, TypeShape::Opaque { .. })
                if target.info.kind == "class" && source.info.kind == "class" =>
            {
                let base = self.db.class_for_type(target.id)?;
                let mut class = self.db.class_for_type(source.id);
                while let Some(current) = class {
                    if current == base {
                        return Some(true);
                    }
                    class = self.db.class_metadata(current).and_then(|meta| meta.base);
                }
                None
            }
            (
                TypeShape::FixedArray {
                    dimensions: target_dims,
                    element: target_element,
                },
                TypeShape::FixedArray {
                    dimensions: source_dims,
                    element: source_element,
                },
            ) if target_dims.len() == source_dims.len()
                && target_dims
                    .iter()
                    .zip(source_dims)
                    .all(|(t, s)| t.0.abs_diff(t.1) == s.0.abs_diff(s.1)) =>
            {
                self.element_upcast(target_element, source_element)
            }
            (
                TypeShape::Container {
                    kind: target_kind,
                    element: target_element,
                    array: target_array,
                },
                TypeShape::Container {
                    kind: source_kind,
                    element: source_element,
                    array: source_array,
                },
            ) if target_kind == source_kind && target_array == source_array => {
                self.element_upcast(target_element, source_element)
            }
            _ => None,
        }
    }
}
