//! Conditional values retain array element boundaries across payload flattening.
use super::*;
use crate::core::db::ConditionalPatternKind;
use crate::sim::ir::{IrPatternCheck, IrPatternExpr, IrPatternMatchKind};
use std::collections::HashSet;

impl Codegen<'_> {
    pub(super) fn lower_conditional(
        &mut self,
        scope_path: &str,
        operands: &[NodeId],
    ) -> Result<IrExpr, String> {
        // Arity is checked by lower_operation. Slang has already reconciled
        // the alternatives' types; query the typed operand before flattening.
        let array = self
            .query_descriptor(operands[1])
            .filter(|descriptor| matches!(descriptor.shape, TypeShape::FixedArray { .. }))
            .map(array_merge_default)
            .transpose()?;
        let structure = self
            .query_descriptor(operands[1])
            .filter(|descriptor| {
                matches!(
                    descriptor.shape,
                    TypeShape::Aggregate(crate::core::db::AggregateLayout {
                        kind: AggregateKind::UnpackedStruct,
                        ..
                    })
                )
            })
            .map(structure_merge_members)
            .transpose()?;
        let sel = self.lower_boolean_expr(scope_path, operands[0])?;
        self.lower_conditional_arms(scope_path, sel, operands[1], operands[2], array, structure)
    }

    pub(in super::super) fn lower_conditional_predicate(
        &mut self,
        scope_path: &str,
        predicate: &crate::core::db::ConditionalPredicate,
    ) -> Result<IrExpr, String> {
        if predicate.clauses.is_empty() {
            return Err(format!(
                "conditional predicate has no clauses in `{scope_path}`"
            ));
        }
        let clauses = predicate
            .clauses
            .iter()
            .map(|clause| self.lower_predicate_clause(scope_path, clause))
            .collect::<Result<Vec<_>, _>>()?;
        if let [clause] = clauses.as_slice() {
            return Ok(clause.clone());
        }
        Ok(IrExpr::new(
            IrExprKind::Predicate { clauses },
            1,
            false,
            None,
        ))
    }

    fn lower_predicate_clause(
        &mut self,
        scope_path: &str,
        clause: &crate::core::db::PredicateClause,
    ) -> Result<IrExpr, String> {
        let Some(pattern_id) = clause.pattern else {
            return self.lower_boolean_expr(scope_path, clause.expression);
        };
        let value = self.lower_expr(scope_path, clause.expression)?;
        self.lower_pattern_value(
            scope_path,
            clause.expression,
            pattern_id,
            value,
            IrPatternMatchKind::Exact,
        )
    }

    pub(in super::super) fn lower_pattern_value(
        &mut self,
        scope_path: &str,
        expression: NodeId,
        pattern_id: NodeId,
        value: IrExpr,
        match_kind: IrPatternMatchKind,
    ) -> Result<IrExpr, String> {
        let info = self.db.conditional_pattern(pattern_id).ok_or_else(|| {
            format!("conditional predicate pattern metadata is missing in `{scope_path}`")
        })?;
        if matches!(
            info.kind,
            ConditionalPatternKind::Structure | ConditionalPatternKind::Tagged
        ) {
            let descriptor = self.query_descriptor(expression).cloned().ok_or_else(|| {
                format!("conditional aggregate pattern source type is missing in `{scope_path}`")
            })?;
            let source_width = Codegen::fixed_descriptor_width(&descriptor).ok_or_else(|| {
                format!(
                    "conditional aggregate pattern requires a fixed-width source in `{scope_path}`"
                )
            })?;
            if value.width != source_width {
                return Err(format!(
                    "conditional aggregate pattern source width disagrees with its type in `{scope_path}`"
                ));
            }
            let mut checks = Vec::new();
            let mut active = HashSet::new();
            match info.kind {
                ConditionalPatternKind::Structure => self.lower_structure_pattern_fields(
                    scope_path,
                    pattern_id,
                    &descriptor,
                    0,
                    &mut checks,
                    &mut active,
                    match_kind,
                )?,
                ConditionalPatternKind::Tagged => self.lower_tagged_pattern_checks(
                    scope_path,
                    pattern_id,
                    &descriptor,
                    0,
                    &mut checks,
                    &mut active,
                    match_kind,
                )?,
                _ => unreachable!("aggregate pattern kind was checked above"),
            }
            return Ok(IrExpr::new(
                IrExprKind::Pattern(Box::new(IrPatternExpr {
                    value: Box::new(value),
                    constant: None,
                    binding: None,
                    match_kind,
                    checks,
                })),
                1,
                false,
                None,
            ));
        }
        if value.is_real()
            || self
                .query_descriptor(expression)
                .is_some_and(|descriptor| !matches!(descriptor.shape, TypeShape::PackedAtom { .. }))
        {
            return Err(format!(
                "conditional predicate pattern requires an integral value in `{scope_path}`"
            ));
        }
        let binding = match info.kind {
            ConditionalPatternKind::Wildcard => None,
            ConditionalPatternKind::Binding => {
                let target = info.binding.ok_or_else(|| {
                    format!("conditional predicate binding has no declaration in `{scope_path}`")
                })?;
                Some(self.lower_lhs(scope_path, target).map_err(|error| {
                    format!("conditional predicate binding cannot be assigned in `{scope_path}`: {error}")
                })?)
            }
            ConditionalPatternKind::Constant => None,
            ConditionalPatternKind::Invalid
            | ConditionalPatternKind::Tagged
            | ConditionalPatternKind::Structure
            | ConditionalPatternKind::Unsupported => {
                return Err(format!(
                    "unsupported conditional predicate pattern in `{scope_path}`"
                ));
            }
        };
        let constant = if info.kind == ConditionalPatternKind::Constant {
            let children = self.db.node(pattern_id).children();
            let [constant_node] = children else {
                return Err(format!(
                    "conditional predicate constant pattern has invalid shape in `{scope_path}`"
                ));
            };
            let constant = self.lower_expr(scope_path, *constant_node)?;
            if constant.is_real()
                || self
                    .query_descriptor(*constant_node)
                    .is_some_and(|descriptor| {
                        !matches!(descriptor.shape, TypeShape::PackedAtom { .. })
                    })
            {
                return Err(format!(
                    "conditional predicate constant pattern requires an integral constant in `{scope_path}`"
                ));
            }
            let width = value.width.max(constant.width);
            let signed = value.signed && constant.signed;
            Some(Box::new(checked_operand_with_context(
                constant,
                width,
                signed,
                scope_path,
                "conditional pattern context",
            )?))
        } else {
            None
        };
        let width = constant
            .as_ref()
            .map_or(value.width, |constant| constant.width);
        let signed = constant
            .as_ref()
            .map_or(value.signed, |constant| constant.signed);
        let value = if value.width == width && value.signed == signed {
            value
        } else {
            checked_operand_with_context(
                value,
                width,
                signed,
                scope_path,
                "conditional pattern context",
            )?
        };
        Ok(IrExpr::new(
            IrExprKind::Pattern(Box::new(IrPatternExpr {
                value: Box::new(value),
                constant,
                binding,
                match_kind,
                checks: Vec::new(),
            })),
            1,
            false,
            None,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn lower_structure_pattern_fields(
        &mut self,
        scope_path: &str,
        pattern_id: NodeId,
        descriptor: &TypeDescriptor,
        base_offset: u32,
        checks: &mut Vec<IrPatternCheck>,
        active: &mut HashSet<NodeId>,
        match_kind: IrPatternMatchKind,
    ) -> Result<(), String> {
        if !active.insert(pattern_id) {
            return Err(format!(
                "cyclic conditional structure pattern in `{scope_path}`"
            ));
        }
        let TypeShape::Aggregate(layout) = &descriptor.shape else {
            return Err(format!(
                "conditional structure pattern requires a structure source in `{scope_path}`"
            ));
        };
        if !matches!(
            layout.kind,
            AggregateKind::PackedStruct | AggregateKind::UnpackedStruct
        ) {
            return Err(format!(
                "conditional structure pattern requires a fixed structure in `{scope_path}`"
            ));
        }
        let fields = self
            .db
            .conditional_pattern_fields(pattern_id)
            .ok_or_else(|| {
                format!("conditional structure pattern fields are missing in `{scope_path}`")
            })?
            .to_vec();
        let mut seen_fields = HashSet::new();
        let mut seen_names = HashSet::new();
        for field in fields {
            if !seen_fields.insert(field.field) {
                return Err(format!(
                    "conditional structure pattern repeats a field in `{scope_path}`"
                ));
            }
            let name = self.db.node(field.field).name.clone();
            if !seen_names.insert(name.clone()) {
                return Err(format!(
                    "conditional structure pattern repeats member `{name}` in `{scope_path}`"
                ));
            }
            let index = layout
                .members
                .iter()
                .position(|member| member.name == name)
                .ok_or_else(|| {
                    format!(
                        "conditional structure pattern member `{name}` is not in its source type in `{scope_path}`"
                    )
                })?;
            let member = &layout.members[index];
            let field_descriptor = self.db.type_descriptor(field.field).ok_or_else(|| {
                format!(
                    "conditional structure pattern member `{name}` has no resolved type in `{scope_path}`"
                )
            })?;
            if field_descriptor.id != member.descriptor.id {
                return Err(format!(
                    "conditional structure pattern member `{name}` has an incompatible resolved type in `{scope_path}`"
                ));
            }
            let width = Codegen::fixed_descriptor_width(&member.descriptor).ok_or_else(|| {
                format!(
                    "conditional structure pattern member `{name}` has no fixed integral payload in `{scope_path}`"
                )
            })?;
            let displacement = layout.members[index + 1..]
                .iter()
                .try_fold(0u32, |sum, member| {
                    sum.checked_add(Codegen::fixed_descriptor_width(&member.descriptor)?)
                })
                .ok_or_else(|| {
                    format!(
                        "conditional structure pattern member offset overflows in `{scope_path}`"
                    )
                })?;
            let offset = base_offset.checked_add(displacement).ok_or_else(|| {
                format!("conditional structure pattern member offset overflows in `{scope_path}`")
            })?;
            let info = self.db.conditional_pattern(field.pattern).ok_or_else(|| {
                format!("conditional structure pattern child metadata is missing in `{scope_path}`")
            })?;
            match info.kind {
                ConditionalPatternKind::Wildcard => {}
                ConditionalPatternKind::Constant => {
                    let children = self.db.node(field.pattern).children();
                    let [constant_node] = children else {
                        return Err(format!(
                            "conditional structure constant pattern has invalid shape in `{scope_path}`"
                        ));
                    };
                    let constant = self.lower_expr(scope_path, *constant_node)?;
                    if constant.is_real()
                        || !matches!(
                            self.query_descriptor(*constant_node)
                                .map(|descriptor| &descriptor.shape),
                            Some(TypeShape::PackedAtom { .. })
                        )
                    {
                        return Err(format!(
                            "conditional structure constant pattern requires an integral constant in `{scope_path}`"
                        ));
                    }
                    let constant = checked_operand_with_context(
                        constant,
                        width,
                        member.descriptor.info.signed,
                        scope_path,
                        "conditional structure pattern context",
                    )?;
                    checks.push(IrPatternCheck {
                        offset,
                        width,
                        signed: member.descriptor.info.signed,
                        exact: false,
                        constant: Some(Box::new(constant)),
                        binding: None,
                    });
                }
                ConditionalPatternKind::Binding => {
                    let target = info.binding.ok_or_else(|| {
                        format!(
                            "conditional structure binding has no declaration in `{scope_path}`"
                        )
                    })?;
                    let target_descriptor = self.query_descriptor(target).ok_or_else(|| {
                        format!(
                            "conditional structure binding `{}` has no resolved type in `{scope_path}`",
                            self.db.node(target).name
                        )
                    })?;
                    if target_descriptor.id != member.descriptor.id {
                        return Err(format!(
                            "conditional structure binding `{}` has an incompatible resolved type with member `{name}` in `{scope_path}`",
                            self.db.node(target).name
                        ));
                    }
                    let target_width = Codegen::fixed_descriptor_width(target_descriptor).ok_or_else(|| {
                        format!(
                            "conditional structure binding `{}` has no resolved width in `{scope_path}`",
                            self.db.node(target).name
                        )
                    })?;
                    if target_width != width {
                        return Err(format!(
                            "conditional structure binding `{}` width disagrees with member `{name}` in `{scope_path}`",
                            self.db.node(target).name
                        ));
                    }
                    checks.push(IrPatternCheck {
                        offset,
                        width,
                        signed: member.descriptor.info.signed,
                        exact: false,
                        constant: None,
                        binding: Some(self.lower_lhs(scope_path, target).map_err(|error| {
                            format!(
                                "conditional structure binding cannot be assigned in `{scope_path}`: {error}"
                            )
                        })?),
                    });
                }
                ConditionalPatternKind::Structure => {
                    self.lower_structure_pattern_fields(
                        scope_path,
                        field.pattern,
                        &member.descriptor,
                        offset,
                        checks,
                        active,
                        match_kind,
                    )?;
                }
                ConditionalPatternKind::Tagged => self.lower_tagged_pattern_checks(
                    scope_path,
                    field.pattern,
                    &member.descriptor,
                    offset,
                    checks,
                    active,
                    match_kind,
                )?,
                ConditionalPatternKind::Invalid | ConditionalPatternKind::Unsupported => {
                    return Err(format!(
                        "unsupported nested conditional structure pattern in `{scope_path}`"
                    ));
                }
            }
        }
        active.remove(&pattern_id);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn lower_tagged_pattern_checks(
        &mut self,
        scope_path: &str,
        pattern_id: NodeId,
        descriptor: &TypeDescriptor,
        base_offset: u32,
        checks: &mut Vec<IrPatternCheck>,
        active: &mut HashSet<NodeId>,
        match_kind: IrPatternMatchKind,
    ) -> Result<(), String> {
        if !active.insert(pattern_id) {
            return Err(format!(
                "cyclic conditional tagged pattern in `{scope_path}`"
            ));
        }
        let TypeShape::Aggregate(layout) = &descriptor.shape else {
            return Err(format!(
                "conditional tagged pattern requires a tagged-union source in `{scope_path}`"
            ));
        };
        if layout.kind != AggregateKind::TaggedUnion {
            return Err(format!(
                "conditional tagged pattern requires a tagged-union source in `{scope_path}`"
            ));
        }
        let info = self.db.conditional_pattern(pattern_id).ok_or_else(|| {
            format!("conditional tagged pattern metadata is missing in `{scope_path}`")
        })?;
        let member_id = info.tagged_member.ok_or_else(|| {
            format!("conditional tagged pattern has no resolved union member in `{scope_path}`")
        })?;
        let member_name = self.db.node(member_id).name.clone();
        let member_index = layout
            .members
            .iter()
            .position(|member| member.name == member_name)
            .ok_or_else(|| {
                format!(
                    "conditional tagged pattern member `{member_name}` is not in its source type in `{scope_path}`"
                )
            })?;
        let member = &layout.members[member_index];
        let member_descriptor = self.db.type_descriptor(member_id).ok_or_else(|| {
            format!(
                "conditional tagged pattern member `{member_name}` has no resolved type in `{scope_path}`"
            )
        })?;
        if member_descriptor.id != member.descriptor.id {
            return Err(format!(
                "conditional tagged pattern member `{member_name}` has an incompatible resolved type in `{scope_path}`"
            ));
        }
        let tag_width = layout.tag_bits().ok_or_else(|| {
            format!("conditional tagged pattern tag width overflows in `{scope_path}`")
        })?;
        let payload_width = layout.payload_bits().ok_or_else(|| {
            format!("conditional tagged pattern payload width overflows in `{scope_path}`")
        })?;
        let total_width = tag_width.checked_add(payload_width).ok_or_else(|| {
            format!("conditional tagged pattern width overflows in `{scope_path}`")
        })?;
        if Codegen::fixed_descriptor_width(descriptor) != Some(total_width) {
            return Err(format!(
                "conditional tagged pattern source width disagrees with its tag and payload in `{scope_path}`"
            ));
        }
        if tag_width > 0 {
            let tag_offset = base_offset.checked_add(payload_width).ok_or_else(|| {
                format!("conditional tagged pattern tag offset overflows in `{scope_path}`")
            })?;
            let tag = IrConst::packed(
                vec![u64::try_from(member_index).map_err(|_| {
                    format!("conditional tagged pattern member index overflows in `{scope_path}`")
                })?],
                vec![0],
                vec![0],
                tag_width,
                false,
                None,
            )
            .map_err(|error| error.to_string())?;
            checks.push(IrPatternCheck {
                offset: tag_offset,
                width: tag_width,
                signed: false,
                // SV 12.6.1 applies the enclosing case wildcard mode to tag
                // bits too. Ordinary member-access guards remain exact.
                exact: false,
                constant: Some(Box::new(IrExpr::new(
                    IrExprKind::Const(tag),
                    tag_width,
                    false,
                    None,
                ))),
                binding: None,
            });
        }

        if let Some(payload) = info.value_pattern {
            let is_void = matches!(
                &member.descriptor.shape,
                TypeShape::Opaque { kind } if kind == "Void"
            );
            if is_void {
                return Err(format!(
                    "void tagged pattern member `{member_name}` cannot have a payload pattern in `{scope_path}`"
                ));
            }
            let member_width =
                Codegen::fixed_descriptor_width(&member.descriptor).ok_or_else(|| {
                    format!(
                    "tagged pattern member `{member_name}` has no fixed payload in `{scope_path}`"
                )
                })?;
            if member_width > payload_width {
                return Err(format!(
                    "tagged pattern member `{member_name}` exceeds its union payload in `{scope_path}`"
                ));
            }
            self.lower_pattern_component(
                scope_path,
                payload,
                &member.descriptor,
                base_offset,
                checks,
                active,
                match_kind,
            )?;
        }
        active.remove(&pattern_id);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn lower_pattern_component(
        &mut self,
        scope_path: &str,
        pattern_id: NodeId,
        descriptor: &TypeDescriptor,
        base_offset: u32,
        checks: &mut Vec<IrPatternCheck>,
        active: &mut HashSet<NodeId>,
        match_kind: IrPatternMatchKind,
    ) -> Result<(), String> {
        let info = self.db.conditional_pattern(pattern_id).ok_or_else(|| {
            format!("nested conditional pattern metadata is missing in `{scope_path}`")
        })?;
        match info.kind {
            ConditionalPatternKind::Wildcard => Ok(()),
            ConditionalPatternKind::Constant => {
                let children = self.db.node(pattern_id).children();
                let [constant_node] = children else {
                    return Err(format!(
                        "nested conditional constant pattern has invalid shape in `{scope_path}`"
                    ));
                };
                let constant = self.lower_expr(scope_path, *constant_node)?;
                if constant.is_real()
                    || !matches!(
                        self.query_descriptor(*constant_node)
                            .map(|descriptor| &descriptor.shape),
                        Some(TypeShape::PackedAtom { .. })
                    )
                {
                    return Err(format!(
                        "nested conditional constant pattern requires an integral constant in `{scope_path}`"
                    ));
                }
                let width = Codegen::fixed_descriptor_width(descriptor).ok_or_else(|| {
                    format!(
                        "nested conditional constant pattern has no fixed payload in `{scope_path}`"
                    )
                })?;
                let constant = checked_operand_with_context(
                    constant,
                    width,
                    descriptor.info.signed,
                    scope_path,
                    "conditional pattern context",
                )?;
                checks.push(IrPatternCheck {
                    offset: base_offset,
                    width,
                    signed: descriptor.info.signed,
                    exact: false,
                    constant: Some(Box::new(constant)),
                    binding: None,
                });
                Ok(())
            }
            ConditionalPatternKind::Binding => {
                let target = info.binding.ok_or_else(|| {
                    format!("nested conditional binding has no declaration in `{scope_path}`")
                })?;
                let target_descriptor = self.query_descriptor(target).ok_or_else(|| {
                    format!(
                        "conditional binding `{}` has no resolved type in `{scope_path}`",
                        self.db.node(target).name
                    )
                })?;
                if target_descriptor.id != descriptor.id {
                    return Err(format!(
                        "conditional binding `{}` has an incompatible payload type in `{scope_path}`",
                        self.db.node(target).name
                    ));
                }
                let width = Codegen::fixed_descriptor_width(descriptor).ok_or_else(|| {
                    format!(
                        "conditional binding `{}` has no fixed payload in `{scope_path}`",
                        self.db.node(target).name
                    )
                })?;
                checks.push(IrPatternCheck {
                    offset: base_offset,
                    width,
                    signed: descriptor.info.signed,
                    exact: false,
                    constant: None,
                    binding: Some(self.lower_lhs(scope_path, target).map_err(|error| {
                        format!("conditional binding cannot be assigned in `{scope_path}`: {error}")
                    })?),
                });
                Ok(())
            }
            ConditionalPatternKind::Structure => self.lower_structure_pattern_fields(
                scope_path,
                pattern_id,
                descriptor,
                base_offset,
                checks,
                active,
                match_kind,
            ),
            ConditionalPatternKind::Tagged => self.lower_tagged_pattern_checks(
                scope_path,
                pattern_id,
                descriptor,
                base_offset,
                checks,
                active,
                match_kind,
            ),
            ConditionalPatternKind::Invalid | ConditionalPatternKind::Unsupported => Err(format!(
                "unsupported nested conditional pattern in `{scope_path}`"
            )),
        }
    }

    pub(in super::super) fn lower_predicate_conditional(
        &mut self,
        scope_path: &str,
        predicate: &crate::core::db::ConditionalPredicate,
        if_true: NodeId,
        if_false: NodeId,
    ) -> Result<IrExpr, String> {
        let array = self
            .query_descriptor(if_true)
            .filter(|descriptor| matches!(descriptor.shape, TypeShape::FixedArray { .. }))
            .map(array_merge_default)
            .transpose()?;
        let structure = self
            .query_descriptor(if_true)
            .filter(|descriptor| {
                matches!(
                    descriptor.shape,
                    TypeShape::Aggregate(crate::core::db::AggregateLayout {
                        kind: AggregateKind::UnpackedStruct,
                        ..
                    })
                )
            })
            .map(structure_merge_members)
            .transpose()?;
        let sel = self.lower_conditional_predicate(scope_path, predicate)?;
        self.lower_conditional_arms(scope_path, sel, if_true, if_false, array, structure)
    }

    fn lower_conditional_arms(
        &mut self,
        scope_path: &str,
        sel: IrExpr,
        if_true: NodeId,
        if_false: NodeId,
        array: Option<(u32, IrConst)>,
        structure: Option<Vec<IrConditionalMember>>,
    ) -> Result<IrExpr, String> {
        let a = self.lower_expr(scope_path, if_true)?;
        let b = self.lower_expr(scope_path, if_false)?;
        if let Some((width, element_default)) = array {
            if a.width != width || b.width != width || a.is_real() || b.is_real() {
                return Err(format!(
                    "fixed-array conditional payload width mismatch in `{scope_path}`"
                ));
            }
            return Ok(IrExpr::new(
                IrExprKind::ArrayMux {
                    sel: Box::new(sel),
                    a: Box::new(IrExpr::resize_to(a, width, false)),
                    b: Box::new(IrExpr::resize_to(b, width, false)),
                    element_default: Box::new(element_default),
                },
                width,
                false,
                None,
            ));
        }
        if let Some(members) = structure {
            let width = members
                .iter()
                .try_fold(0u32, |total, member| total.checked_add(member.width))
                .ok_or_else(|| {
                    format!("fixed-structure conditional payload width overflow in `{scope_path}`")
                })?;
            if width == 0 || a.width != width || b.width != width || a.is_real() || b.is_real() {
                return Err(format!(
                    "fixed-structure conditional payload width mismatch in `{scope_path}`"
                ));
            }
            return Ok(IrExpr::new(
                IrExprKind::StructMux {
                    sel: Box::new(sel),
                    a: Box::new(IrExpr::resize_to(a, width, false)),
                    b: Box::new(IrExpr::resize_to(b, width, false)),
                    members,
                },
                width,
                false,
                None,
            ));
        }
        let (width, signed) = if a.is_real() || b.is_real() {
            (REAL_EXPR_WIDTH, true)
        } else {
            (a.width.max(b.width), a.signed && b.signed)
        };
        let (a, b) = if width == REAL_EXPR_WIDTH {
            (a, b)
        } else {
            (
                checked_operand_with_context(a, width, signed, scope_path, "conditional context")?,
                checked_operand_with_context(b, width, signed, scope_path, "conditional context")?,
            )
        };
        Ok(IrExpr::new(
            IrExprKind::Mux {
                sel: Box::new(sel),
                a: Box::new(a),
                b: Box::new(b),
            },
            width,
            signed,
            None,
        ))
    }
}

fn structure_merge_members(
    descriptor: &TypeDescriptor,
) -> Result<Vec<IrConditionalMember>, String> {
    let TypeShape::Aggregate(layout) = &descriptor.shape else {
        return Err("conditional merge requires an unpacked structure".to_owned());
    };
    if layout.kind != AggregateKind::UnpackedStruct || layout.members.is_empty() {
        return Err("conditional merge requires a nonempty unpacked structure".to_owned());
    }

    // The flattened representation places the first declared member at the
    // most-significant end, matching fixed aggregate projections and defaults.
    let mut offset = 0u32;
    let mut members = Vec::with_capacity(layout.members.len());
    for member in layout.members.iter().rev() {
        let width = Codegen::fixed_descriptor_width(&member.descriptor)
            .ok_or("conditional structure member has no supported fixed payload")?;
        let default = Codegen::fixed_descriptor_uninitialized(&member.descriptor)
            .ok_or("conditional structure member has no supported default-uninitialized payload")?;
        if width == 0 || default.width != width {
            return Err("conditional structure member default width mismatch".to_owned());
        }
        members.push(IrConditionalMember {
            offset,
            width,
            default,
        });
        offset = offset
            .checked_add(width)
            .ok_or("conditional structure payload width overflow")?;
    }
    if offset != Codegen::fixed_descriptor_width(descriptor).unwrap_or(0) {
        return Err("conditional structure payload width disagrees with members".to_owned());
    }
    Ok(members)
}

fn array_merge_default(descriptor: &TypeDescriptor) -> Result<(u32, IrConst), String> {
    let TypeShape::FixedArray {
        dimensions,
        element,
    } = &descriptor.shape
    else {
        return Err("conditional merge requires a fixed unpacked array".to_owned());
    };
    if dimensions.is_empty() {
        return Err("conditional array has no unpacked dimension".to_owned());
    }
    let width = Codegen::fixed_descriptor_width(descriptor)
        .ok_or("conditional array requires a supported fixed integral payload")?;
    // TypeDescriptor coalesces consecutive unpacked dimensions. Only peel
    // the outermost dimension: a differing row defaults the *whole* row.
    let immediate = if dimensions.len() == 1 {
        *element.clone()
    } else {
        TypeDescriptor {
            shape: TypeShape::FixedArray {
                dimensions: dimensions[1..].to_vec(),
                element: element.clone(),
            },
            ..descriptor.clone()
        }
    };
    let default = Codegen::fixed_descriptor_uninitialized(&immediate)
        .ok_or("conditional array element has no supported default-uninitialized payload")?;
    if default.width == 0 || !width.is_multiple_of(default.width) {
        return Err("conditional array element width does not divide its payload".to_owned());
    }
    Ok((width, default))
}

#[cfg(test)]
mod tests;
