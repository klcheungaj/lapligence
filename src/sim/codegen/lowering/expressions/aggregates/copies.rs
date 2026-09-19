//! Selected aggregate copies keep type identity separate from root storage.
use super::*;

pub(super) struct AggregateSelection {
    root: NodeId,
    prefix: Vec<AggregatePathPart>,
    descriptor: TypeDescriptor,
    storage: UnpackedAggregateInfo,
}

/// Check the recursive shape used by the leaf-copy ABI. Unpacked aggregates
/// retain nominal identity; fixed-array bounds may differ, but each dimension
/// must have the same extent. Packed leaf state domains are checked against the
/// captured leaf metadata, which (unlike TypeInfo) retains the two-state flag.
fn equivalent_copy_shape(left: &TypeDescriptor, right: &TypeDescriptor) -> bool {
    match (&left.shape, &right.shape) {
        (
            TypeShape::FixedArray {
                dimensions: ld,
                element: le,
            },
            TypeShape::FixedArray {
                dimensions: rd,
                element: re,
            },
        ) => {
            ld.len() == rd.len()
                && ld.iter().zip(rd).all(|(&(ll, lr), &(rl, rr))| {
                    (i64::from(ll) - i64::from(lr)).unsigned_abs()
                        == (i64::from(rl) - i64::from(rr)).unsigned_abs()
                })
                && equivalent_copy_shape(le, re)
        }
        (TypeShape::Aggregate(ll), TypeShape::Aggregate(rl)) => {
            let same_identity = match (&ll.type_identity, &rl.type_identity) {
                (Some(left), Some(right)) => left == right,
                (None, None) => left.id == right.id,
                _ => false,
            };
            same_identity
                && ll.kind == rl.kind
                && ll.members.len() == rl.members.len()
                && ll.members.iter().zip(&rl.members).all(|(left, right)| {
                    left.name == right.name
                        && left.two_state == right.two_state
                        && equivalent_copy_shape(&left.descriptor, &right.descriptor)
                })
        }
        (TypeShape::PackedAtom { .. }, TypeShape::PackedAtom { .. }) => {
            // Enum identity is nominal even when its integral representation
            // matches another enum or plain vector.
            (left.info.kind != "enum" && right.info.kind != "enum" || left.id == right.id)
                && left.info.width == right.info.width
                && left.info.signed == right.info.signed
        }
        (
            TypeShape::Real { shortreal: left },
            TypeShape::Real { shortreal: right },
        ) => left == right,
        (TypeShape::String, TypeShape::String) => true,
        (TypeShape::Opaque { kind: left }, TypeShape::Opaque { kind: right }) => {
            left == "Chandle" && right == "Chandle"
        }
        _ => false,
    }
}

impl Codegen<'_> {
    pub(super) fn resolve_unpacked_aggregate(&self, node: NodeId) -> Option<AggregateSelection> {
        let (root, prefix, storage) = if let Some((root, storage)) = self.unpacked_aggregate_info(node) {
            (root, Vec::new(), storage)
        } else {
            let (root, prefix) = self.unpacked_path_for_expr(node)?;
            let storage = self.unpacked_aggregates.get(&root)?.clone();
            (root, prefix, storage)
        };
        let descriptor = Self::descriptor_at_path(self.query_descriptor(root)?, &prefix)?;
        if !matches!(
            descriptor.shape,
            TypeShape::Aggregate(_) | TypeShape::FixedArray { .. }
        ) {
            return None;
        }
        Some(AggregateSelection {
            root,
            prefix,
            descriptor,
            storage,
        })
    }

    pub(super) fn lower_unpacked_subaggregate_copy(
        &mut self,
        path: &str,
        lhs: &AggregateSelection,
        rhs: &AggregateSelection,
        nba: bool,
        op: Operation,
    ) -> Result<IrStmt, String> {
        if op != Operation::Assignment {
            return Err(format!(
                "compound assignment of unpacked aggregates in `{path}` is not supported"
            ));
        }
        if !equivalent_copy_shape(&lhs.descriptor, &rhs.descriptor) {
            return Err(format!(
                "assignment between incompatible unpacked aggregate types `{}` and `{}` in `{path}`",
                lhs.descriptor.name, rhs.descriptor.name
            ));
        }
        if matches!(
            &lhs.descriptor.shape,
            TypeShape::Aggregate(layout) if layout.kind == AggregateKind::UnpackedUnion
        ) {
            return Err(format!(
                "nested unpacked union assignment in `{path}` is not supported"
            ));
        }
        // collect_aggregate_descriptor_leaves emits members and array elements
        // in declaration order. Keep both paths: equal numeric array indices
        // are not the language's assignment correspondence rule.
        let left: Vec<_> = lhs
            .storage
            .leaves
            .iter()
            .filter(|leaf| leaf.path.starts_with(&lhs.prefix))
            .collect();
        let right: Vec<_> = rhs
            .storage
            .leaves
            .iter()
            .filter(|leaf| leaf.path.starts_with(&rhs.prefix))
            .collect();
        if left.len() != right.len() || left.is_empty() {
            return Err(format!(
                "selected aggregate copy has incompatible leaf storage in `{path}`"
            ));
        }
        let mut captures = Vec::with_capacity(left.len());
        let mut assignments = Vec::with_capacity(left.len());
        for (position, (left, right)) in left.into_iter().zip(right).enumerate() {
            if left.member.two_state != right.member.two_state
                || !equivalent_copy_shape(&left.member.descriptor, &right.member.descriptor)
            {
                return Err(format!(
                    "selected aggregate copy has incompatible leaf types in `{path}`"
                ));
            }
            let name = format!("_agg_copy_{}_{}_{}", lhs.root.0, rhs.root.0, position);
            if let (Some(lhs_object), Some(rhs_object)) = (left.object, right.object) {
                if nba {
                    return Err(format!(
                        "nonblocking assignment to object aggregate member `{}` is not supported in `{path}`",
                        aggregate_path_suffix(&left.path)
                    ));
                }
                let lhs_object = self.reference_object(lhs_object);
                let rhs_object = self.reference_object(rhs_object);
                let operation = match self.model.objects[lhs_object].ty {
                    IrObjectType::String => {
                        captures.push(IrStmt::DeclString {
                            name: name.clone(),
                            init: Some(IrStringExpr::Read(rhs_object)),
                        });
                        IrObjectStmt::StringAssign(lhs_object, IrStringExpr::LocalRead(name))
                    }
                    IrObjectType::Chandle | IrObjectType::Semaphore => {
                        captures.push(IrStmt::Object(IrObjectStmt::ChandleDeclareLocal(
                            name.clone(),
                            Some(IrChandleExpr::Read(rhs_object)),
                        )));
                        IrObjectStmt::ChandleAssign(lhs_object, IrChandleExpr::LocalRead(name))
                    }
                    IrObjectType::Process => {
                        return Err(format!(
                            "process aggregate member assignment is not supported in `{path}`"
                        ));
                    }
                };
                assignments.push(IrStmt::Object(operation));
                continue;
            }
            let target = self.aggregate_leaf_lhs(left)?;
            let mut value = self.aggregate_leaf_read(right)?;
            if right.member.two_state && !value.is_real() {
                value = IrExpr::to_two_state(value);
            }
            let (width, signed) = (value.width, value.signed);
            captures.push(IrStmt::DeclLocal {
                name: name.clone(),
                width,
                signed,
                two_state: false,
                init: Some(Box::new(value)),
            });
            let value = IrExpr::new(IrExprKind::LocalRead(name), width, signed, None);
            assignments.push(IrStmt::Assign {
                rhs: apply_lhs_assignment_context(&self.model, &target, value),
                lhs: target,
                nba,
            });
        }
        // Capture the entire RHS before any write, including native leaves.
        // Self-copies and overlapping selected values cannot observe a prefix
        // of this assignment as their own source.
        captures.extend(assignments);
        Ok(IrStmt::Block(captures))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::db::{AggregateLayout, TypeId};
    use crate::core::model::TypeInfo;

    fn atom(id: u64) -> TypeDescriptor {
        TypeDescriptor {
            id: TypeId(id),
            name: "logic [7:0]".to_owned(),
            info: TypeInfo {
                kind: "logic".to_owned(),
                width: Some(8),
                signed: false,
                type_name: None,
            },
            shape: TypeShape::PackedAtom { ranges: vec![] },
        }
    }

    fn array(bounds: Vec<(i32, i32)>, element: TypeDescriptor) -> TypeDescriptor {
        TypeDescriptor {
            id: TypeId(100),
            name: "array".to_owned(),
            info: TypeInfo::default(),
            shape: TypeShape::FixedArray {
                dimensions: bounds,
                element: Box::new(element),
            },
        }
    }

    #[test]
    fn array_equivalence_uses_extents_not_logical_index_spellings() {
        let left = array(vec![(0, 1), (-3, -1)], atom(1));
        let right = array(vec![(5, 4), (8, 6)], atom(2));
        assert!(equivalent_copy_shape(&left, &right));
        assert!(!equivalent_copy_shape(
            &left,
            &array(vec![(5, 3), (8, 6)], atom(2)),
        ));
        let mut signed = atom(2);
        signed.info.signed = true;
        assert!(!equivalent_copy_shape(
            &left,
            &array(vec![(5, 4), (8, 6)], signed),
        ));
    }

    #[test]
    fn nominal_aggregate_identity_is_not_replaced_by_storage_width() {
        let descriptor = |id| TypeDescriptor {
            id: TypeId(id),
            name: "anonymous struct".to_owned(),
            info: TypeInfo::default(),
            shape: TypeShape::Aggregate(AggregateLayout {
                kind: AggregateKind::UnpackedStruct,
                type_identity: Some(format!("type#{id}")),
                type_id: Some(TypeId(id)),
                members: vec![],
            }),
        };
        assert!(equivalent_copy_shape(&descriptor(1), &descriptor(1)));
        assert!(!equivalent_copy_shape(&descriptor(1), &descriptor(2)));
    }
}
