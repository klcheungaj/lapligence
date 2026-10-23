//! Conditional values retain array element boundaries across payload flattening.
use super::super::collection::{RecordColumn, RecordValue};
use super::*;
use crate::core::db::ConditionalPatternKind;
use crate::sim::ir::{
    IrNativeBinding, IrNativeLeafExpr, IrPatternCheck, IrPatternExpr, IrPatternMatchKind,
};
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
        let merge = self
            .query_descriptor(operands[1])
            .map(aggregate_conditional_merge)
            .transpose()?
            .flatten();
        let sel = self.lower_boolean_expr(scope_path, operands[0])?;
        self.lower_conditional_arms(scope_path, sel, operands[1], operands[2], merge)
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
        if let Some(test) = self.lower_native_pattern(
            scope_path,
            clause.expression,
            pattern_id,
            IrPatternMatchKind::Exact,
        )? {
            return Ok(test);
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
                    native_bindings: Vec::new(),
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
                    native_bindings: Vec::new(),
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
                native_bindings: Vec::new(),
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
        let merge = self
            .query_descriptor(if_true)
            .map(aggregate_conditional_merge)
            .transpose()?
            .flatten();
        let sel = self.lower_conditional_predicate(scope_path, predicate)?;
        self.lower_conditional_arms(scope_path, sel, if_true, if_false, merge)
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
        merge: Option<AggregateMerge>,
    ) -> Result<IrExpr, String> {
        let a = self.lower_expr(scope_path, if_true)?;
        let b = self.lower_expr(scope_path, if_false)?;
        if let Some(merge) = merge {
            let width = merge.width;
            if a.width != width || b.width != width || a.is_real() || b.is_real() {
                return Err(format!(
                    "fixed aggregate conditional payload width mismatch in `{scope_path}`"
                ));
            }
            let (sel, a, b) = (
                Box::new(sel),
                Box::new(IrExpr::resize_to(a, width, false)),
                Box::new(IrExpr::resize_to(b, width, false)),
            );
            let kind = match merge.leaves {
                AggregateLeaves::Uniform(element_default) => IrExprKind::ArrayMux {
                    sel,
                    a,
                    b,
                    element_default: Box::new(element_default),
                },
                AggregateLeaves::Mixed(leaves) => IrExprKind::StructMux {
                    sel,
                    a,
                    b,
                    leaves: Box::new(leaves),
                },
            };
            return Ok(IrExpr::new(kind, width, false, None));
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

/// How an ambiguous conditional merges a fixed aggregate payload
/// (IEEE 1800-2009 11.4.11).
pub(in crate::sim::codegen) struct AggregateMerge {
    width: u32,
    leaves: AggregateLeaves,
}

pub(in crate::sim::codegen) enum AggregateLeaves {
    /// Every leaf has this width and default-uninitialized value.
    Uniform(IrConst),
    /// One period (a record, or one record element of an array) with leaves
    /// of different widths or defaults.
    Mixed(IrLeafMerge),
}

/// The merge of a fixed unpacked array or unpacked structure result, or
/// `None` for any other type. The text combines the arms "element by
/// element"; an element that is itself an unpacked array or structure is
/// combined the same way, so only leaves (packed, real and other scalar
/// values, and unions as a whole) are compared and replaced. Defaults are
/// default-uninitialized values, never member initializers.
pub(in crate::sim::codegen) fn aggregate_conditional_merge(
    descriptor: &TypeDescriptor,
) -> Result<Option<AggregateMerge>, String> {
    let mut period = descriptor;
    while let TypeShape::FixedArray {
        element,
        dimensions,
    } = &period.shape
    {
        if dimensions.is_empty() {
            return Err("conditional array has no unpacked dimension".to_owned());
        }
        period = element;
    }
    if std::ptr::eq(period, descriptor) && !is_unpacked_structure(descriptor) {
        return Ok(None);
    }
    let width = Codegen::fixed_descriptor_width(descriptor)
        .ok_or("conditional aggregate requires a supported fixed integral payload")?;
    let leaves = record_leaf_merge(period)?;
    let period_width = leaves.default.width;
    if !width.is_multiple_of(period_width) {
        return Err("conditional aggregate element width does not divide its payload".to_owned());
    }
    let leaves = match uniform_leaf_default(&leaves) {
        Some(default) => AggregateLeaves::Uniform(default),
        None => AggregateLeaves::Mixed(leaves),
    };
    Ok(Some(AggregateMerge { width, leaves }))
}

/// The leaf layout of one storage cell of a descriptor-backed fixed array
/// of type `descriptor`: a record element merges leaf by leaf, while any
/// other cell is one leaf (`None`).
pub(in crate::sim::codegen) fn fixed_cell_leaves(
    descriptor: &TypeDescriptor,
) -> Result<Option<Box<IrLeafMerge>>, String> {
    let mut cell = descriptor;
    while let TypeShape::FixedArray { element, .. } = &cell.shape {
        cell = element;
    }
    if std::ptr::eq(cell, descriptor) || !is_unpacked_structure(cell) {
        return Ok(None);
    }
    record_leaf_merge(cell).map(|leaves| Some(Box::new(leaves)))
}

fn is_unpacked_structure(descriptor: &TypeDescriptor) -> bool {
    matches!(
        &descriptor.shape,
        TypeShape::Aggregate(layout) if layout.kind == AggregateKind::UnpackedStruct
    )
}

/// The default-uninitialized value of `period` and the start bit of each
/// of its leaves. The default comes from the whole type, so member state
/// flags apply exactly as they do to storage defaults.
pub(in crate::sim::codegen) fn record_leaf_merge(
    period: &TypeDescriptor,
) -> Result<IrLeafMerge, String> {
    let default = Codegen::fixed_descriptor_uninitialized(period)
        .ok_or("conditional aggregate element has no supported default-uninitialized payload")?;
    let width = Codegen::fixed_descriptor_width(period)
        .filter(|width| *width == default.width)
        .ok_or("conditional aggregate element default width mismatch")?;
    let mut starts = vec![0u64; width.div_ceil(64) as usize];
    mark_leaf_starts(period, 0, &mut starts)?;
    let starts = IrConst::packed(starts, Vec::new(), Vec::new(), width, false, None)
        .map_err(|error| error.to_string())?;
    let default = IrConst::packed(
        default.bits().to_vec(),
        default.x_mask().to_vec(),
        default.z_mask().to_vec(),
        width,
        false,
        None,
    )
    .map_err(|error| error.to_string())?;
    Ok(IrLeafMerge { default, starts })
}

/// Set the start bit of every leaf of `descriptor` placed at `offset`. The
/// flattened payload places the first declared member and the left array
/// element at the most-significant end.
fn mark_leaf_starts(
    descriptor: &TypeDescriptor,
    offset: u32,
    starts: &mut [u64],
) -> Result<(), String> {
    let width = |descriptor: &TypeDescriptor| {
        Codegen::fixed_descriptor_width(descriptor)
            .ok_or_else(|| "conditional aggregate leaf has no supported fixed payload".to_owned())
    };
    match &descriptor.shape {
        TypeShape::FixedArray { element, .. } => {
            let (total, stride) = (width(descriptor)?, width(element)?);
            let leaf = !matches!(element.shape, TypeShape::FixedArray { .. })
                && !is_unpacked_structure(element);
            for index in 0..total / stride {
                let at = offset + index * stride;
                if leaf {
                    starts[(at / 64) as usize] |= 1u64 << (at % 64);
                } else {
                    mark_leaf_starts(element, at, starts)?;
                }
            }
        }
        TypeShape::Aggregate(layout) if layout.kind == AggregateKind::UnpackedStruct => {
            let mut cursor = offset;
            for member in layout.members.iter().rev() {
                mark_leaf_starts(&member.descriptor, cursor, starts)?;
                cursor += width(&member.descriptor)?;
            }
            if cursor - offset != width(descriptor)? {
                return Err("conditional structure payload width disagrees with members".to_owned());
            }
        }
        _ => {
            width(descriptor)?;
            starts[(offset / 64) as usize] |= 1u64 << (offset % 64);
        }
    }
    Ok(())
}

/// The common leaf default when every leaf of the period has the same width
/// and default, so the stride merge applies.
fn uniform_leaf_default(leaves: &IrLeafMerge) -> Option<IrConst> {
    let bit = |limbs: &[u64], index: u32| {
        limbs
            .get((index / 64) as usize)
            .is_some_and(|limb| limb >> (index % 64) & 1 != 0)
    };
    let width = leaves.default.width;
    let stride = (1..width)
        .find(|index| bit(leaves.starts.bits(), *index))
        .unwrap_or(width);
    if !width.is_multiple_of(stride) {
        return None;
    }
    let planes = [
        leaves.default.bits(),
        leaves.default.x_mask(),
        leaves.default.z_mask(),
    ];
    for index in stride..width {
        if bit(leaves.starts.bits(), index) != index.is_multiple_of(stride)
            || planes
                .iter()
                .any(|plane| bit(plane, index) != bit(plane, index % stride))
        {
            return None;
        }
    }
    let slice = |plane: &[u64]| {
        let mut limbs = vec![0u64; stride.div_ceil(64) as usize];
        for index in 0..stride {
            if bit(plane, index) {
                limbs[(index / 64) as usize] |= 1u64 << (index % 64);
            }
        }
        limbs
    };
    IrConst::packed(
        slice(planes[0]),
        slice(planes[1]),
        slice(planes[2]),
        stride,
        false,
        None,
    )
    .ok()
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
                        native_bindings: Vec::new(),
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
                native_bindings: Vec::new(),
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

/// A pattern test that always matches (a wildcard over a value without a
/// packed payload); its effects are the native bindings it carries.
fn native_match_all(
    match_kind: IrPatternMatchKind,
    native_bindings: Vec<IrNativeBinding>,
) -> IrExpr {
    IrExpr::new(
        IrExprKind::Pattern(Box::new(IrPatternExpr {
            value: Box::new(const_bits_expr(1, true)),
            constant: None,
            binding: None,
            match_kind,
            checks: Vec::new(),
            native_bindings,
        })),
        1,
        false,
        None,
    )
}

impl Codegen<'_> {
    /// Pattern tests whose source has no packed payload (SIM-007): a tagged
    /// union variable with string, real or handle members, or a whole
    /// string, real or handle value. `None` leaves other sources to the
    /// packed pattern lowering. The source is read in place when the test
    /// runs; a tagged member's tag is tested before its payload (SV 12.6).
    pub(in super::super) fn lower_native_pattern(
        &mut self,
        scope_path: &str,
        source: NodeId,
        pattern_id: NodeId,
        match_kind: IrPatternMatchKind,
    ) -> Result<Option<IrExpr>, String> {
        let info = self.db.conditional_pattern(pattern_id).ok_or_else(|| {
            format!("conditional predicate pattern metadata is missing in `{scope_path}`")
        })?;
        if let Some(root) = self.native_tagged_root(source) {
            return match info.kind {
                ConditionalPatternKind::Wildcard => {
                    Ok(Some(native_match_all(match_kind, Vec::new())))
                }
                ConditionalPatternKind::Tagged => self
                    .lower_native_tagged_pattern(scope_path, root, &info, match_kind)
                    .map(Some),
                _ => Err(format!(
                    "this pattern on a tagged union with string, real or handle members in `{scope_path}` is not supported"
                )),
            };
        }
        let Some(descriptor) = self.query_descriptor(source).cloned() else {
            return Ok(None);
        };
        if !matches!(
            descriptor.shape,
            TypeShape::String | TypeShape::Real { .. } | TypeShape::Opaque { .. }
        ) {
            return Ok(None);
        }
        match info.kind {
            ConditionalPatternKind::Wildcard => Ok(Some(native_match_all(match_kind, Vec::new()))),
            ConditionalPatternKind::Binding => {
                let target = info.binding.ok_or_else(|| {
                    format!("conditional predicate binding has no declaration in `{scope_path}`")
                })?;
                let binding = match &descriptor.shape {
                    TypeShape::String => IrNativeBinding::String {
                        local: self.pattern_string_local(target),
                        value: self.lower_string(scope_path, source)?,
                    },
                    TypeShape::Real { .. } => IrNativeBinding::Value {
                        lhs: self.lower_lhs(scope_path, target)?,
                        value: self.lower_expr(scope_path, source)?,
                    },
                    _ => {
                        return Err(format!(
                            "a handle pattern variable in `{scope_path}` is not supported"
                        ))
                    }
                };
                Ok(Some(native_match_all(match_kind, vec![binding])))
            }
            _ => Ok(None),
        }
    }

    fn lower_native_tagged_pattern(
        &mut self,
        scope_path: &str,
        root: super::super::collection::native_values::NativeTaggedRoot,
        info: &crate::core::db::ConditionalPatternInfo,
        match_kind: IrPatternMatchKind,
    ) -> Result<IrExpr, String> {
        let member_id = info.tagged_member.ok_or_else(|| {
            format!("conditional tagged pattern has no resolved union member in `{scope_path}`")
        })?;
        let member_name = self.db.node(member_id).name.clone();
        let parts =
            self.native_tagged_pattern(scope_path, root, &member_name, info.value_pattern)?;
        let payload = match parts.payload {
            Some(payload) => Some(self.db.conditional_pattern(payload).ok_or_else(|| {
                format!("tagged pattern payload metadata is missing in `{scope_path}`")
            })?),
            None => None,
        };
        let tag_check = |offset: u32| -> Result<Option<IrPatternCheck>, String> {
            if parts.tag.is_none() {
                return Ok(None);
            }
            let expected = IrConst::packed(
                vec![u64::from(parts.index)],
                vec![0],
                vec![0],
                parts.tag_bits,
                false,
                None,
            )
            .map_err(|error| error.to_string())?;
            Ok(Some(IrPatternCheck {
                offset,
                width: parts.tag_bits,
                signed: false,
                two_state: false,
                // The enclosing case mode applies to tag bits (SV 12.6.1).
                exact: false,
                constant: Some(Box::new(IrExpr::new(
                    IrExprKind::Const(expected),
                    parts.tag_bits,
                    false,
                    None,
                ))),
                binding: None,
            }))
        };
        let tag_value = parts
            .tag
            .clone()
            .unwrap_or_else(|| const_bits_expr(1, true));
        let pattern =
            |value: IrExpr, checks: Vec<IrPatternCheck>, bindings: Vec<IrNativeBinding>| {
                IrExpr::new(
                    IrExprKind::Pattern(Box::new(IrPatternExpr {
                        value: Box::new(value),
                        constant: None,
                        binding: None,
                        match_kind,
                        checks,
                        native_bindings: bindings,
                    })),
                    1,
                    false,
                    None,
                )
            };
        let Some(payload_info) = payload else {
            return Ok(pattern(
                tag_value,
                tag_check(0)?.into_iter().collect(),
                Vec::new(),
            ));
        };
        match payload_info.kind {
            ConditionalPatternKind::Wildcard => Ok(pattern(
                tag_value,
                tag_check(0)?.into_iter().collect(),
                Vec::new(),
            )),
            ConditionalPatternKind::Binding => {
                let target = payload_info.binding.ok_or_else(|| {
                    format!("tagged pattern binding has no declaration in `{scope_path}`")
                })?;
                let binding = match parts.value.clone() {
                    Some(IrNativeLeafExpr::String(value)) => IrNativeBinding::String {
                        local: self.pattern_string_local(target),
                        value,
                    },
                    Some(IrNativeLeafExpr::Packed(value) | IrNativeLeafExpr::Real(value)) => {
                        IrNativeBinding::Value {
                            lhs: self.lower_lhs(scope_path, target)?,
                            value,
                        }
                    }
                    _ => {
                        return Err(format!(
                            "binding tagged member `{member_name}` to a pattern variable in `{scope_path}` is not supported for its type"
                        ))
                    }
                };
                Ok(pattern(
                    tag_value,
                    tag_check(0)?.into_iter().collect(),
                    vec![binding],
                ))
            }
            ConditionalPatternKind::Constant
            | ConditionalPatternKind::Structure
            | ConditionalPatternKind::Tagged => {
                // A packed member's payload checks run on {tag, member}.
                let Some(IrNativeLeafExpr::Packed(member)) = parts.value.clone() else {
                    return Err(format!(
                        "tagged member `{member_name}` payload pattern in `{scope_path}` requires a packed member"
                    ));
                };
                let member_width = member.width;
                let mut checks: Vec<IrPatternCheck> =
                    tag_check(member_width)?.into_iter().collect();
                let value = match parts.tag.clone() {
                    Some(tag) => IrExpr::new(
                        IrExprKind::Concat {
                            parts: vec![tag, member],
                        },
                        parts.tag_bits + member_width,
                        false,
                        None,
                    ),
                    None => member,
                };
                let mut active = HashSet::new();
                self.lower_pattern_component(
                    scope_path,
                    parts.payload.ok_or("tagged payload disappeared")?,
                    &parts.descriptor,
                    0,
                    &mut checks,
                    &mut active,
                    match_kind,
                )?;
                Ok(pattern(value, checks, Vec::new()))
            }
            _ => Err(format!(
                "unsupported tagged payload pattern in `{scope_path}`"
            )),
        }
    }
}
