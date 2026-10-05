//! Conditional values retain array element boundaries across payload flattening.
use super::super::collection::{RecordColumn, RecordValue};
use super::*;
use crate::core::db::ConditionalPatternKind;
use crate::sim::ir::{IrPatternCheck, IrPatternExpr, IrPatternMatchKind};
use std::collections::HashSet;

fn supported_fixed_integral_value(descriptor: &TypeDescriptor) -> bool {
    matches!(
        &descriptor.shape,
        TypeShape::PackedAtom { .. }
            | TypeShape::Aggregate(crate::core::db::AggregateLayout {
                kind: AggregateKind::PackedStruct | AggregateKind::PackedUnion,
                ..
            })
    ) && Codegen::fixed_descriptor_width(descriptor).is_some()
}

/// A whole-value wildcard or binding needs one packed owner for its source.
/// A finite source beyond packed capacity has no descriptor pattern
/// transport yet, so it is rejected with its size instead of being flattened.
fn whole_pattern_source_error(descriptor: &TypeDescriptor, scope_path: &str) -> String {
    match Codegen::fixed_descriptor_width_bits(descriptor) {
        Some(width) if width > u64::from(LLG_MAX_WIDTH) => {
            Codegen::fixed_descriptor_capacity_error(
                &format!("conditional whole-value pattern source in `{scope_path}`"),
                width,
            )
        }
        _ => format!(
            "conditional whole-value pattern requires a supported fixed value in `{scope_path}`"
        ),
    }
}

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
        if self.record_columns {
            let source = self.p30_unwrap_cast(clause.expression);
            if let Some(value) = self.column_record_storage(source) {
                return self.lower_column_pattern(
                    scope_path,
                    &value,
                    pattern_id,
                    IrPatternMatchKind::Exact,
                );
            }
            if self.column_record_type(clause.expression) {
                return Err(format!(
                    "a `matches` source beyond packed capacity must be a variable in `{scope_path}`"
                ));
            }
        }
        let info = self.db.conditional_pattern(pattern_id).ok_or_else(|| {
            format!("conditional predicate pattern metadata is missing in `{scope_path}`")
        })?;
        if matches!(
            info.kind,
            ConditionalPatternKind::Wildcard | ConditionalPatternKind::Binding
        ) {
            // An unsupported source may not lower as a value, so diagnose its
            // pattern type before resolving the expression reference.
            let descriptor = self.query_descriptor(clause.expression).ok_or_else(|| {
                format!("conditional whole-value pattern source type is missing in `{scope_path}`")
            })?;
            if Codegen::fixed_descriptor_width(descriptor).is_none() {
                return Err(whole_pattern_source_error(descriptor, scope_path));
            }
        }
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
        if matches!(
            info.kind,
            ConditionalPatternKind::Wildcard | ConditionalPatternKind::Binding
        ) {
            // Wildcards and identifier patterns act on the complete matched
            // type, not just scalar integral syntax (SV 12.6). Only fixed
            // values with an owned payload are admitted by this backend.
            let descriptor = self.query_descriptor(expression).ok_or_else(|| {
                format!("conditional whole-value pattern source type is missing in `{scope_path}`")
            })?;
            let width = Codegen::fixed_descriptor_width(descriptor)
                .ok_or_else(|| whole_pattern_source_error(descriptor, scope_path))?;
            if value.is_real() || value.width != width {
                return Err(format!(
                    "conditional whole-value pattern source width disagrees with its type in `{scope_path}`"
                ));
            }
            let binding = if info.kind == ConditionalPatternKind::Binding {
                let target = info.binding.ok_or_else(|| {
                    format!("conditional predicate binding has no declaration in `{scope_path}`")
                })?;
                let target_descriptor = self.query_descriptor(target).ok_or_else(|| {
                    format!("conditional predicate binding type is missing in `{scope_path}`")
                })?;
                if target_descriptor.id != descriptor.id
                    || Codegen::fixed_descriptor_width(target_descriptor) != Some(width)
                {
                    return Err(format!(
                        "conditional predicate binding has an incompatible matched type in `{scope_path}`"
                    ));
                }
                Some(self.lower_lhs(scope_path, target).map_err(|error| {
                    format!("conditional predicate binding cannot be assigned in `{scope_path}`: {error}")
                })?)
            } else {
                None
            };
            return Ok(IrExpr::new(
                IrExprKind::Pattern(Box::new(IrPatternExpr {
                    value: Box::new(value),
                    constant: None,
                    binding,
                    match_kind,
                    checks: Vec::new(),
                })),
                1,
                false,
                None,
            ));
        }
        if info.kind != ConditionalPatternKind::Constant {
            return Err(format!(
                "unsupported conditional predicate pattern in `{scope_path}`"
            ));
        }
        if value.is_real()
            || self
                .query_descriptor(expression)
                .is_some_and(|descriptor| !supported_fixed_integral_value(descriptor))
        {
            return Err(format!(
                "conditional predicate pattern requires an integral value in `{scope_path}`"
            ));
        }
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
                .is_some_and(|descriptor| !supported_fixed_integral_value(descriptor))
        {
            return Err(format!(
                "conditional predicate constant pattern requires an integral constant in `{scope_path}`"
            ));
        }
        let width = value.width.max(constant.width);
        let signed = value.signed && constant.signed;
        let constant = Some(Box::new(checked_operand_with_context(
            constant,
            width,
            signed,
            scope_path,
            "conditional pattern context",
        )?));
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
                binding: None,
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
                    if !supported_fixed_integral_value(&member.descriptor) {
                        return Err(format!(
                            "conditional structure constant pattern requires an integral member in `{scope_path}`"
                        ));
                    }
                    let children = self.db.node(field.pattern).children();
                    let [constant_node] = children else {
                        return Err(format!(
                            "conditional structure constant pattern has invalid shape in `{scope_path}`"
                        ));
                    };
                    let constant = self.lower_expr(scope_path, *constant_node)?;
                    if constant.is_real()
                        || !self
                            .query_descriptor(*constant_node)
                            .is_some_and(supported_fixed_integral_value)
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
                        two_state: member.descriptor.two_state,
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
                        two_state: member.descriptor.two_state,
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
                two_state: false,
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
                if !supported_fixed_integral_value(descriptor) {
                    return Err(format!(
                        "nested conditional constant pattern requires an integral payload in `{scope_path}`"
                    ));
                }
                let children = self.db.node(pattern_id).children();
                let [constant_node] = children else {
                    return Err(format!(
                        "nested conditional constant pattern has invalid shape in `{scope_path}`"
                    ));
                };
                let constant = self.lower_expr(scope_path, *constant_node)?;
                if constant.is_real()
                    || !self
                        .query_descriptor(*constant_node)
                        .is_some_and(supported_fixed_integral_value)
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
                    two_state: descriptor.two_state,
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
                    two_state: descriptor.two_state,
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

    /// The selector and arms of a conditional operator whose result has the
    /// native type `shape` (string or chandle). The selector is lowered with
    /// the same predicate rules as packed conditionals.
    pub(in super::super) fn lower_native_conditional_parts(
        &mut self,
        scope_path: &str,
        node: NodeId,
        shape: &TypeShape,
    ) -> Result<Option<(IrExpr, NodeId, NodeId)>, String> {
        if self
            .query_descriptor(node)
            .is_none_or(|descriptor| descriptor.shape != *shape)
        {
            return Ok(None);
        }
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Conditional {
                predicate,
                if_true,
                if_false,
            }) => {
                let (if_true, if_false) = (*if_true, *if_false);
                let predicate = predicate.clone();
                let selector = self.lower_conditional_predicate(scope_path, &predicate)?;
                Ok(Some((selector, if_true, if_false)))
            }
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Conditional,
                operands,
                ..
            }) if operands.len() == 3 => {
                let (condition, if_true, if_false) = (operands[0], operands[1], operands[2]);
                let selector = self.lower_boolean_expr(scope_path, condition)?;
                Ok(Some((selector, if_true, if_false)))
            }
            _ => Ok(None),
        }
    }

    /// Whether `node` is a conditional operator with native result `shape`.
    pub(in super::super) fn is_native_conditional(&self, node: NodeId, shape: &TypeShape) -> bool {
        matches!(
            self.kind(node),
            NodeKind::Expr(
                ExprKind::Conditional { .. }
                    | ExprKind::Operation {
                        op: Operation::Conditional,
                        ..
                    }
            )
        ) && self
            .query_descriptor(node)
            .is_some_and(|descriptor| descriptor.shape == *shape)
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

impl Codegen<'_> {
    /// `value matches pattern` for a column-layout record or tagged union.
    /// The source has no packed payload, so each leaf the pattern inspects is
    /// tested in its own column and the tests combine in pattern order: a
    /// tag test first, then member tests, each evaluated only after every
    /// earlier test matched (SV 12.6).
    pub(in super::super) fn lower_column_pattern(
        &mut self,
        scope_path: &str,
        value: &RecordValue,
        pattern_id: NodeId,
        match_kind: IrPatternMatchKind,
    ) -> Result<IrExpr, String> {
        let info = self
            .db
            .conditional_pattern(pattern_id)
            .ok_or_else(|| format!("conditional pattern metadata is missing in `{scope_path}`"))?;
        let TypeShape::Aggregate(layout) = &value.descriptor.shape else {
            return Err(format!(
                "column-layout pattern source has no layout in `{scope_path}`"
            ));
        };
        let layout = layout.clone();
        match info.kind {
            ConditionalPatternKind::Wildcard => Ok(pattern_truth(true)),
            ConditionalPatternKind::Binding => {
                let target = info.binding.ok_or_else(|| {
                    format!("conditional pattern binding has no declaration in `{scope_path}`")
                })?;
                self.lower_record_binding(scope_path, target, value)
            }
            ConditionalPatternKind::Tagged => {
                if layout.kind != AggregateKind::TaggedUnion {
                    return Err(format!(
                        "conditional tagged pattern requires a tagged-union source in `{scope_path}`"
                    ));
                }
                let member_id = info.tagged_member.ok_or_else(|| {
                    format!("conditional tagged pattern has no resolved union member in `{scope_path}`")
                })?;
                let member_name = self.db.node(member_id).name.clone();
                let index = layout
                    .members
                    .iter()
                    .position(|member| member.name == member_name)
                    .ok_or_else(|| {
                        format!("conditional tagged pattern member `{member_name}` is not in its source type in `{scope_path}`")
                    })?;
                let tag = value
                    .columns
                    .iter()
                    .find(|(path, _)| path.is_empty())
                    .map(|(_, column)| column.clone())
                    .ok_or_else(|| format!("tagged union source has no tag column in `{scope_path}`"))?;
                let tag_value = self.record_column_read(&tag)?;
                let tag_width = tag_value.width;
                let expected = IrConst::packed(
                    vec![u64::try_from(index).map_err(|_| "tagged member index overflows")?],
                    vec![0],
                    vec![0],
                    tag_width,
                    false,
                    None,
                )
                .map_err(|error| error.to_string())?;
                let tag_test = IrExpr::new(
                    IrExprKind::Pattern(Box::new(IrPatternExpr {
                        value: Box::new(tag_value),
                        constant: None,
                        binding: None,
                        match_kind,
                        checks: vec![IrPatternCheck {
                            offset: 0,
                            width: tag_width,
                            signed: false,
                            two_state: false,
                            // The enclosing case mode applies to tag bits too
                            // (SV 12.6.1), as in the packed layout.
                            exact: false,
                            constant: Some(Box::new(IrExpr::new(
                                IrExprKind::Const(expected),
                                tag_width,
                                false,
                                None,
                            ))),
                            binding: None,
                        }],
                    })),
                    1,
                    false,
                    None,
                );
                let Some(payload) = info.value_pattern else {
                    return Ok(tag_test);
                };
                let member = layout.members[index].clone();
                let member_test =
                    self.lower_column_member_pattern(scope_path, value, &member, payload, match_kind)?;
                Ok(cmp_expr_ir(IrBinOp::LogAnd, tag_test, member_test))
            }
            ConditionalPatternKind::Structure => {
                if layout.kind != AggregateKind::UnpackedStruct {
                    return Err(format!(
                        "conditional structure pattern requires a structure source in `{scope_path}`"
                    ));
                }
                let fields = self
                    .db
                    .conditional_pattern_fields(pattern_id)
                    .ok_or_else(|| {
                        format!("conditional structure pattern fields are missing in `{scope_path}`")
                    })?
                    .to_vec();
                let mut seen = HashSet::new();
                let mut result: Option<IrExpr> = None;
                for field in fields {
                    let name = self.db.node(field.field).name.clone();
                    if !seen.insert(name.clone()) {
                        return Err(format!(
                            "conditional structure pattern repeats member `{name}` in `{scope_path}`"
                        ));
                    }
                    let member = layout
                        .members
                        .iter()
                        .find(|member| member.name == name)
                        .cloned()
                        .ok_or_else(|| {
                            format!("conditional structure pattern member `{name}` is not in its source type in `{scope_path}`")
                        })?;
                    let test = self.lower_column_member_pattern(
                        scope_path,
                        value,
                        &member,
                        field.pattern,
                        match_kind,
                    )?;
                    result = Some(match result {
                        Some(previous) => cmp_expr_ir(IrBinOp::LogAnd, previous, test),
                        None => test,
                    });
                }
                Ok(result.unwrap_or_else(|| pattern_truth(true)))
            }
            ConditionalPatternKind::Constant
            | ConditionalPatternKind::Invalid
            | ConditionalPatternKind::Unsupported => Err(format!(
                "unsupported conditional pattern for a value beyond packed capacity in `{scope_path}`"
            )),
        }
    }

    /// The test of one immediate member of a column-layout source.
    fn lower_column_member_pattern(
        &mut self,
        scope_path: &str,
        value: &RecordValue,
        member: &AggregateMember,
        pattern_id: NodeId,
        match_kind: IrPatternMatchKind,
    ) -> Result<IrExpr, String> {
        let key = AggregatePathPart::Member(member.name.clone());
        let columns = value
            .columns
            .iter()
            .filter(|(path, _)| path.first() == Some(&key))
            .map(|(path, column)| (path[1..].to_vec(), column.clone()))
            .collect::<Vec<_>>();
        let info = self.db.conditional_pattern(pattern_id).ok_or_else(|| {
            format!("nested conditional pattern metadata is missing in `{scope_path}`")
        })?;
        if info.kind == ConditionalPatternKind::Wildcard {
            return Ok(pattern_truth(true));
        }
        if matches!(&member.descriptor.shape, TypeShape::Opaque { kind } if kind == "Void") {
            return Err(format!(
                "void tagged pattern member `{}` cannot have a payload pattern in `{scope_path}`",
                member.name
            ));
        }
        if super::super::collection::record_column_layout_type(&member.descriptor) {
            let nested = RecordValue {
                descriptor: member.descriptor.clone(),
                columns,
            };
            return self.lower_column_pattern(scope_path, &nested, pattern_id, match_kind);
        }
        let [(path, column)] = columns.as_slice() else {
            return Err(format!(
                "pattern on member `{}` needs one packed member value in `{scope_path}`",
                member.name
            ));
        };
        if let (true, RecordColumn::Array(column)) = (path.is_empty(), column) {
            if info.kind == ConditionalPatternKind::Binding {
                let target = info.binding.ok_or_else(|| {
                    format!("conditional pattern binding has no declaration in `{scope_path}`")
                })?;
                return self.lower_array_binding(scope_path, target, *column);
            }
        }
        if !path.is_empty() || matches!(column, RecordColumn::Array(_)) {
            return Err(format!(
                "pattern on member array `{}` of a value beyond packed capacity supports only `.*` and `.name` in `{scope_path}`",
                member.name
            ));
        }
        let source = self.record_column_read(column)?;
        let mut checks = Vec::new();
        let mut active = HashSet::new();
        self.lower_pattern_component(
            scope_path,
            pattern_id,
            &member.descriptor,
            0,
            &mut checks,
            &mut active,
            match_kind,
        )?;
        Ok(IrExpr::new(
            IrExprKind::Pattern(Box::new(IrPatternExpr {
                value: Box::new(source),
                constant: None,
                binding: None,
                match_kind,
                checks,
            })),
            1,
            false,
            None,
        ))
    }
}

fn pattern_truth(value: bool) -> IrExpr {
    IrExpr::new(
        IrExprKind::Const(
            IrConst::packed(vec![u64::from(value)], vec![0], vec![0], 1, false, None)
                .expect("one-bit pattern truth"),
        ),
        1,
        false,
        None,
    )
}
