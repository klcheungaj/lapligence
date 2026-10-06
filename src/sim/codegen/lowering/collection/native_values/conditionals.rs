//! Conditional operators whose result is a native record (SV 11.4.11).
//!
//! A known predicate assigns exactly one arm. An ambiguous predicate
//! evaluates both arms once and merges them per immediate member: a member
//! whose leaves all match keeps that value; any other member takes its
//! type's default-uninitialized value as a whole, so nested records and
//! array members are never merged leaf by leaf.

use super::*;

impl Codegen<'_> {
    /// Whether `node` is a conditional operator producing a native record.
    pub(in crate::sim::codegen) fn native_record_conditional(&self, node: NodeId) -> bool {
        matches!(
            self.kind(node),
            NodeKind::Expr(
                ExprKind::Conditional { .. }
                    | ExprKind::Operation {
                        op: Operation::Conditional,
                        ..
                    }
            )
        ) && self.native_value_type(node).is_some()
    }

    /// Assign the conditional `source` to `target`.
    pub(super) fn native_conditional_into(
        &mut self,
        path: &str,
        target: &NativeEndpoint,
        descriptor: &TypeDescriptor,
        source: NodeId,
        nba: bool,
    ) -> Result<IrStmt, String> {
        let (selector, if_true, if_false) = match self.kind(source) {
            NodeKind::Expr(ExprKind::Conditional {
                predicate,
                if_true,
                if_false,
            }) => {
                let (if_true, if_false) = (*if_true, *if_false);
                let predicate = predicate.clone();
                (
                    self.lower_conditional_predicate(path, &predicate)?,
                    if_true,
                    if_false,
                )
            }
            NodeKind::Expr(ExprKind::Operation { operands, .. }) if operands.len() == 3 => {
                let (condition, if_true, if_false) = (operands[0], operands[1], operands[2]);
                (self.lower_boolean_expr(path, condition)?, if_true, if_false)
            }
            _ => {
                return Err(format!(
                    "native record conditional in `{path}` is malformed"
                ))
            }
        };
        let sequence = self.native_copy_sequence;
        self.native_copy_sequence += 1;
        let name = format!("_llg_native_sel_{sequence}");
        let (width, signed) = (selector.width, selector.signed);
        let declare = IrStmt::DeclLocal {
            name: name.clone(),
            width,
            signed,
            two_state: false,
            init: Some(Box::new(selector)),
        };
        let read = IrExpr::new(IrExprKind::LocalRead(name), width, signed, None);
        let known_false = cmp_expr_ir(IrBinOp::CaseEq, read.clone(), const_bits_expr(width, false));
        let take_true = self.native_assign_into(path, target, descriptor, if_true, nba)?;
        let take_false = self.native_assign_into(path, target, descriptor, if_false, nba)?;
        let merge =
            self.native_conditional_merge(path, target, descriptor, if_true, if_false, nba)?;
        // `if` takes its else branch for an unknown condition, so the inner
        // test separates a known false from an ambiguous predicate.
        Ok(IrStmt::Block(vec![
            declare,
            IrStmt::If {
                cond: read,
                then_: vec![take_true],
                els: Some(vec![IrStmt::If {
                    cond: known_false,
                    then_: vec![take_false],
                    els: Some(vec![merge]),
                    check: IrUniquePriorityCheck::None,
                }]),
                check: IrUniquePriorityCheck::None,
            },
        ]))
    }

    /// One evaluated arm as a leaf endpoint. Record variables are read in
    /// place; any other arm (call, pattern, nested conditional) is first
    /// assigned to a lexical temporary of the record type.
    fn native_conditional_arm(
        &mut self,
        path: &str,
        arm: NodeId,
        statements: &mut Vec<IrStmt>,
    ) -> Result<NativeEndpoint, String> {
        if let Some((endpoint, _)) = self.native_endpoint(self.p30_unwrap_cast(arm))? {
            return Ok(endpoint);
        }
        let temporary = self.native_temporary(arm)?;
        let endpoint = NativeEndpoint::Value {
            value: temporary,
            prefix: Vec::new(),
        };
        let layout = self
            .native_layout(arm)?
            .ok_or("native conditional arm has no record layout")?;
        statements.push(IrStmt::NativeValueDeclare(temporary));
        statements.push(self.native_assign_into(
            path,
            &endpoint,
            &layout.descriptor,
            arm,
            false,
        )?);
        Ok(endpoint)
    }

    fn native_conditional_merge(
        &mut self,
        path: &str,
        target: &NativeEndpoint,
        descriptor: &TypeDescriptor,
        if_true: NodeId,
        if_false: NodeId,
        nba: bool,
    ) -> Result<IrStmt, String> {
        let mut statements = Vec::new();
        let left = self.native_conditional_arm(path, if_true, &mut statements)?;
        let right = self.native_conditional_arm(path, if_false, &mut statements)?;
        let targets = self.endpoint_leaves(target)?;
        let lefts = self.endpoint_leaves(&left)?;
        let rights = self.endpoint_leaves(&right)?;
        if targets.is_empty()
            || targets.len() != lefts.len()
            || targets.len() != rights.len()
            || targets
                .iter()
                .zip(&lefts)
                .zip(&rights)
                .any(|(((target, _), (left, _)), (right, _))| target != left || target != right)
        {
            return Err(format!(
                "native record conditional in `{path}` has incompatible leaf layouts"
            ));
        }
        let sequence = self.native_copy_sequence;
        self.native_copy_sequence += 1;
        // Capture both arms completely before any write: the target may be
        // one of the arms.
        let mut captured = Vec::with_capacity(lefts.len() * 2);
        for (side, leaves) in [("a", &lefts), ("b", &rights)] {
            for (position, (_, leaf)) in leaves.iter().enumerate() {
                let value = self.endpoint_leaf_read(leaf)?;
                let name = format!("_llg_native_merge_{sequence}_{side}{position}");
                captured.push(capture_leaf(value, name, &mut statements));
            }
        }
        let (left_values, right_values) = captured.split_at(lefts.len());
        // Leaves of one immediate member, in member order; container members
        // follow the scalar leaves, so a member's leaves need not be adjacent.
        let mut members: Vec<(Option<AggregatePathPart>, Vec<usize>)> = Vec::new();
        for (position, (leaf_path, _)) in targets.iter().enumerate() {
            let member = leaf_path.first().cloned();
            match members.iter_mut().find(|(existing, _)| *existing == member) {
                Some((_, positions)) => positions.push(position),
                None => members.push((member, vec![position])),
            }
        }
        for (_, positions) in members {
            let mut matched: Option<IrExpr> = None;
            let mut keep = Vec::with_capacity(positions.len());
            let mut reset = Vec::with_capacity(positions.len());
            for position in positions {
                let (leaf_path, leaf) = &targets[position];
                let (left, right) = (&left_values[position], &right_values[position]);
                let equal = self.known_member_match(path, left.clone(), right.clone())?;
                matched = Some(match matched {
                    Some(previous) => cmp_expr_ir(IrBinOp::LogAnd, previous, equal),
                    None => equal,
                });
                keep.push(self.endpoint_leaf_write(path, leaf, left.clone(), nba)?);
                reset.push(match leaf {
                    // An unmatched container member is empty (SV Table 6-7).
                    NativeEndpointLeaf::Container(container) => {
                        IrStmt::Container(Box::new(IrContainerStmt::Delete(*container)))
                    }
                    _ => {
                        let default = self.native_leaf_default(path, descriptor, leaf_path)?;
                        self.endpoint_leaf_write(path, leaf, default, nba)?
                    }
                });
            }
            statements.push(IrStmt::If {
                cond: matched.ok_or("native record member has no leaves")?,
                then_: keep,
                els: Some(reset),
                check: IrUniquePriorityCheck::None,
            });
        }
        Ok(IrStmt::Block(statements))
    }

    /// A known test that two captured leaves match; container members
    /// match when their elements do (SV 7.2.2, 7.10).
    fn known_member_match(
        &self,
        path: &str,
        left: LeafValue,
        right: LeafValue,
    ) -> Result<IrExpr, String> {
        let (LeafValue::Container(left), LeafValue::Container(right)) = (&left, &right) else {
            return known_leaf_match(path, left, right);
        };
        if matches!(
            self.model.containers[*left].kind,
            crate::sim::ir::IrContainerKind::Associative { .. }
        ) {
            return Err(format!(
                "conditional operator with an ambiguous predicate on records with an associative array member is not supported in `{path}` (associative array equality is not supported)"
            ));
        }
        Ok(IrExpr::new(
            IrExprKind::Container(Box::new(IrContainerExpr::Equal {
                left: *left,
                right: *right,
                case: true,
                negate: false,
            })),
            1,
            false,
            None,
        ))
    }

    /// The default-uninitialized value of the leaf at `leaf_path` below a
    /// record of type `descriptor` (SV Table 6-7).
    fn native_leaf_default(
        &self,
        path: &str,
        descriptor: &TypeDescriptor,
        leaf_path: &[AggregatePathPart],
    ) -> Result<LeafValue, String> {
        // A tagged union's tag defaults to X: no member is active.
        if let (TypeShape::Aggregate(layout), [AggregatePathPart::Member(name)]) =
            (&descriptor.shape, leaf_path)
        {
            if name == NATIVE_TAG_MEMBER {
                let width = layout
                    .tag_bits()
                    .ok_or_else(|| format!("tagged union in `{path}` has no tag"))?;
                return Ok(LeafValue::Packed(IrExpr::new(
                    IrExprKind::Const(IrConst::integral_default(width, false)),
                    width,
                    false,
                    None,
                )));
            }
        }
        let leaf = Self::descriptor_at_path(descriptor, leaf_path)
            .ok_or_else(|| format!("native record leaf in `{path}` has no type"))?;
        Ok(match &leaf.shape {
            TypeShape::String => LeafValue::String(IrStringExpr::Literal(Vec::new())),
            TypeShape::Opaque { .. } => LeafValue::Chandle(IrChandleExpr::Null),
            TypeShape::Real { .. } => LeafValue::Real(real_literal_expr(0.0)),
            _ => {
                let constant = Self::fixed_descriptor_uninitialized(&leaf).ok_or_else(|| {
                    format!("native record leaf in `{path}` has no default-uninitialized value")
                })?;
                let (width, signed) = (constant.width, constant.signed);
                LeafValue::Packed(IrExpr::new(
                    IrExprKind::Const(constant),
                    width,
                    signed,
                    None,
                ))
            }
        })
    }
}

/// A known (0/1) test that two captured leaves match. Packed leaves match
/// only when equality is known true, so X/Z never matches (as for fixed
/// aggregates); strings compare bytes and chandles compare pointers.
fn known_leaf_match(path: &str, left: LeafValue, right: LeafValue) -> Result<IrExpr, String> {
    Ok(match (left, right) {
        (LeafValue::String(a), LeafValue::String(b)) => cmp_expr_ir(
            IrBinOp::Eq,
            IrExpr::new(
                IrExprKind::ObjectQuery(Box::new(IrObjectQuery::StringCompare(a, b, false))),
                32,
                true,
                None,
            ),
            const_bits_expr(32, false),
        ),
        (LeafValue::Chandle(a), LeafValue::Chandle(b)) => IrExpr::new(
            IrExprKind::ObjectQuery(Box::new(IrObjectQuery::ChandleEq(a, b))),
            1,
            false,
            None,
        ),
        (LeafValue::Real(a), LeafValue::Real(b)) => common_cmp_expr_ir(IrBinOp::Eq, a, b, path)?,
        (LeafValue::Packed(a), LeafValue::Packed(b)) => cmp_expr_ir(
            IrBinOp::CaseEq,
            common_cmp_expr_ir(IrBinOp::Eq, a, b, path)?,
            const_bits_expr(1, true),
        ),
        _ => {
            return Err(format!(
                "native record conditional in `{path}` has mismatched member kinds"
            ))
        }
    })
}
