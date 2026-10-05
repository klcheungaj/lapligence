//! Dispatch.

use super::*;

impl<'a> Codegen<'a> {
    fn fixed_array_parameter_target(&self, node: NodeId) -> Option<NodeId> {
        let target = match self.kind(node) {
            NodeKind::Param { .. } => node,
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => *target,
            _ => return None,
        };
        matches!(self.kind(target), NodeKind::Param { .. }).then_some(target)
    }

    fn fixed_array_parameter_initializer(&self, parameter: NodeId) -> Option<NodeId> {
        // Slang's instantiated Param node owns the effective initializer,
        // including an instance override. Never reconstruct that value from
        // the parameter's declaration or scalar `Param::value` field.
        self.node(parameter).children.iter().copied().find(|child| {
            matches!(
                self.kind(*child),
                NodeKind::Expr(ExprKind::Operation {
                    op: Operation::AssignmentPattern | Operation::MultiAssignmentPattern,
                    ..
                })
            )
        })
    }

    /// Lower a fixed unpacked array, structure or union parameter as a value.
    fn lower_fixed_array_parameter_value(
        &mut self,
        path: &str,
        parameter: NodeId,
    ) -> Result<Option<IrExpr>, String> {
        if !matches!(
            self.query_descriptor(parameter)
                .map(|descriptor| &descriptor.shape),
            Some(
                TypeShape::FixedArray { .. }
                    | TypeShape::Aggregate(crate::core::db::AggregateLayout {
                        kind: AggregateKind::UnpackedStruct | AggregateKind::UnpackedUnion,
                        ..
                    })
            )
        ) {
            return Ok(None);
        }
        if let Some(initializer) = self.fixed_array_parameter_initializer(parameter) {
            return self.fixed_pattern_value(path, initializer);
        }
        // Another constant fixed-value initializer, such as a conditional of
        // other parameters, uses the ordinary fixed-value expression path;
        // its runtime merge is the same element rule as SV 11.4.11 folding.
        // An override expression belongs to the parent scope, so only the
        // parameter's own declaration initializer is lowered here.
        if self.db.parameter_is_overridden(parameter) {
            return Ok(None);
        }
        let Some(initializer) = self
            .node(parameter)
            .children
            .iter()
            .copied()
            .find(|child| matches!(self.kind(*child), NodeKind::Expr(_)))
        else {
            return Ok(None);
        };
        self.lower_expr(path, initializer).map(Some)
    }

    fn lower_subroutine_localparam_value(
        &self,
        scope_path: &str,
        parameter: NodeId,
    ) -> Result<Option<IrExpr>, String> {
        let NodeKind::Param {
            value, local: true, ..
        } = self.kind(parameter)
        else {
            return Ok(None);
        };
        let mut parent = self.node(parameter).parent;
        let mut in_subroutine = false;
        while let Some(scope) = parent {
            match self.kind(scope) {
                NodeKind::FuncTask { .. } => {
                    in_subroutine = true;
                    break;
                }
                NodeKind::ModuleInst { .. } | NodeKind::GenScope => break,
                _ => parent = self.node(scope).parent,
            }
        }
        if !in_subroutine {
            return Ok(None);
        }
        let Some(parent) = self.node(parameter).parent else {
            return Ok(None);
        };
        let value = self.collected_parameter_value(parent, parameter, value.as_ref())?;
        let Some(value) = value else {
            return Ok(None);
        };
        match value {
            Val::Bits(value) => {
                let constant = val_to_const(&value)?;
                Ok(Some(IrExpr::new(
                    IrExprKind::Const(constant.clone()),
                    constant.width,
                    constant.signed,
                    None,
                )))
            }
            Val::Real(value) => Ok(Some(real_literal_expr(value))),
            Val::Str(value) => match self.kind(parameter) {
                NodeKind::Param { ty, .. } if ty.kind != "string" => match ty.width {
                    Some(width) => {
                        let constant = string_to_const(&value)?;
                        let expr = IrExpr::new(
                            IrExprKind::Const(constant.clone()),
                            constant.width,
                            constant.signed,
                            None,
                        );
                        Ok(Some(IrExpr::convert_to(expr, width, ty.signed)))
                    }
                    None => Err(format!(
                        "string parameter `{}` used as a value is not supported in `{scope_path}`",
                        self.node(parameter).name
                    )),
                },
                _ => Err(format!(
                    "string parameter `{}` used as a value is not supported in `{scope_path}`",
                    self.node(parameter).name
                )),
            },
        }
    }

    fn lower_fixed_array_parameter_element(
        &mut self,
        path: &str,
        base: NodeId,
        indices: &[NodeId],
    ) -> Result<Option<IrExpr>, String> {
        let Some(parameter) = self.fixed_array_parameter_target(base) else {
            return Ok(None);
        };
        let Some(descriptor) = self.query_descriptor(parameter).cloned() else {
            return Ok(None);
        };
        let TypeShape::FixedArray {
            dimensions,
            element,
        } = &descriptor.shape
        else {
            return Ok(None);
        };
        if indices.len() != dimensions.len() {
            return Ok(None);
        }
        let Some(width) = Self::fixed_descriptor_width(element) else {
            return Ok(None);
        };
        if width == 0 {
            return Ok(None);
        }
        let Some(mut initializer) = self.fixed_array_parameter_initializer(parameter) else {
            return Ok(None);
        };

        let constant_indices = indices
            .iter()
            .map(|index| self.eval_bound_i128(*index))
            .collect::<Result<Vec<_>, _>>();
        let constant_indices = match constant_indices {
            Ok(indices) => indices,
            Err(_) if dimensions.len() == 1 => {
                let (left_bound, right_bound) = dimensions[0];
                let Some(array_value) = self.fixed_pattern_value(path, initializer)? else {
                    return Ok(None);
                };
                let index = self.lower_expr(path, indices[0])?;
                if index.is_real() || index.width == 0 {
                    return Err(format!(
                        "fixed-array parameter index must be integral in `{path}`"
                    ));
                }
                let base_width = index.width.max(32);
                let scale_width = u32::BITS - (width - 1).leading_zeros();
                let arithmetic_width = base_width
                    .checked_add(scale_width)
                    .and_then(|width| width.checked_add(2))
                    .filter(|width| *width <= LLG_MAX_WIDTH)
                    .ok_or_else(|| {
                        "fixed-array parameter index arithmetic exceeds the supported width"
                            .to_owned()
                    })?;
                let index = IrExpr::convert_to(index, arithmetic_width, true);
                let left_expr = IrExpr::convert_to(
                    lhs_integer_expr(i128::from(left_bound)),
                    arithmetic_width,
                    true,
                );
                let ordinal = if left_bound >= right_bound {
                    bin_expr(IrBinOp::Sub, left_expr, index)
                } else {
                    bin_expr(IrBinOp::Sub, index, left_expr)
                };
                let count = i128::from(i64::from(left_bound).abs_diff(i64::from(right_bound)) + 1);
                let remaining = bin_expr(
                    IrBinOp::Sub,
                    IrExpr::convert_to(lhs_integer_expr(count - 1), arithmetic_width, true),
                    ordinal,
                );
                let bit_offset = bin_expr(
                    IrBinOp::Mul,
                    remaining,
                    IrExpr::convert_to(lhs_integer_expr(i128::from(width)), arithmetic_width, true),
                );
                let selected = IrExpr::new(
                    IrExprKind::IdxPartSel {
                        base: Box::new(array_value),
                        base_idx: Box::new(bit_offset),
                        width_expr: Box::new(lhs_integer_expr(i128::from(width))),
                        neg: false,
                    },
                    width,
                    false,
                    None,
                );
                return ir_to_storage(selected, width, element.info.signed, element.two_state)
                    .map(Some);
            }
            Err(_) => return Ok(None),
        };

        for (&index, &(left, right)) in constant_indices.iter().zip(dimensions) {
            let index = i32::try_from(index)
                .map_err(|_| format!("fixed-array parameter index does not fit i32 in `{path}`"))?;
            let offset = if left >= right {
                i64::from(left) - i64::from(index)
            } else {
                i64::from(index) - i64::from(left)
            };
            let count = i64::from(left).abs_diff(i64::from(right)) + 1;
            let offset = usize::try_from(offset)
                .ok()
                .filter(|offset| u64::try_from(*offset).is_ok_and(|offset| offset < count))
                .ok_or_else(|| {
                    format!(
                        "fixed-array parameter index {index} is outside [{left}:{right}] in `{path}`"
                    )
                })?;
            let values = self.p30_pattern_level(path, initializer, (left, right))?;
            initializer = *values.get(offset).ok_or_else(|| {
                format!("fixed-array parameter initializer is missing index {index} in `{path}`")
            })?;
        }

        let value = self.lower_expr(path, initializer)?;
        ir_to_storage(value, width, element.info.signed, element.two_state).map(Some)
    }

    fn fixed_parameter_member_target(
        &self,
        node: NodeId,
    ) -> Option<(NodeId, Vec<AggregatePathPart>)> {
        let (parts, refs) = match self.kind(node) {
            NodeKind::Expr(ExprKind::HierPath { parts, refs }) if parts.len() > 1 => (parts, refs),
            _ => return None,
        };
        let first = refs.first().copied().flatten()?;
        let (parameter, indices) = match self.kind(first) {
            NodeKind::Param { .. } => (first, &[][..]),
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => (
                self.fixed_array_parameter_target(*base)?,
                indices.as_slice(),
            ),
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) if matches!(self.kind(*target), NodeKind::Param { .. }) => (*target, &[][..]),
            _ => return None,
        };
        let descriptor = self.query_descriptor(parameter)?;
        match &descriptor.shape {
            TypeShape::FixedArray { .. } => {}
            TypeShape::Aggregate(layout)
                if matches!(
                    layout.kind,
                    AggregateKind::UnpackedStruct | AggregateKind::UnpackedUnion
                ) => {}
            _ => return None,
        }
        let mut path = Vec::with_capacity(indices.len() + parts.len() - 1);
        for index in indices {
            let index = i32::try_from(self.eval_bound_i128(*index).ok()?).ok()?;
            path.push(AggregatePathPart::Index(index));
        }
        path.extend(parts.iter().skip(1).cloned().map(AggregatePathPart::Member));
        (!path.is_empty()).then_some((parameter, path))
    }

    fn lower_fixed_parameter_member_read(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrExpr>, String> {
        let Some((parameter, aggregate_path)) = self.fixed_parameter_member_target(node) else {
            return Ok(None);
        };
        let Some(descriptor) = self.query_descriptor(parameter).cloned() else {
            return Ok(None);
        };
        let Some((member, offset)) = Self::fixed_descriptor_path(&descriptor, &aggregate_path)
        else {
            return Ok(None);
        };
        let Some(width) = Self::fixed_descriptor_width(&member) else {
            return Ok(None);
        };
        if width == 0 {
            return Ok(None);
        }
        let Some(value) = self.lower_fixed_array_parameter_value(path, parameter)? else {
            return Ok(None);
        };
        let Some(end) = offset.checked_add(width).filter(|end| *end <= value.width) else {
            return Err(format!(
                "fixed parameter member projection exceeds its aggregate value in `{path}`"
            ));
        };
        let selected = IrExpr::new(
            IrExprKind::PartSel {
                base: Box::new(value),
                left: i64::from(end - 1),
                right: i64::from(offset),
            },
            width,
            member.info.signed,
            None,
        );
        ir_to_storage(selected, width, member.info.signed, member.two_state).map(Some)
    }

    fn lower_fixed_parameter_member_array_element(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrExpr>, String> {
        let indices = match self.kind(node) {
            NodeKind::Expr(ExprKind::ArraySelect { indices, .. }) => indices,
            _ => return Ok(None),
        };
        // The owned path retains the exact aggregate owner even when Slang
        // represents its member array as a detached synthetic Array node.
        let Some((root, members)) = self.db.array_select_path(node) else {
            return Ok(None);
        };
        let Some(parameter) = self.fixed_array_parameter_target(root) else {
            return Ok(None);
        };
        let Some(descriptor) = self.query_descriptor(parameter).cloned() else {
            return Ok(None);
        };
        if !matches!(
            &descriptor.shape,
            TypeShape::Aggregate(layout)
                if matches!(layout.kind, AggregateKind::UnpackedStruct | AggregateKind::UnpackedUnion)
        ) {
            return Ok(None);
        }
        let mut aggregate_path = members
            .iter()
            .cloned()
            .map(AggregatePathPart::Member)
            .collect::<Vec<_>>();
        for index in indices {
            let Ok(index) = self.eval_bound_i128(*index) else {
                return Ok(None);
            };
            let Ok(index) = i32::try_from(index) else {
                return Ok(None);
            };
            aggregate_path.push(AggregatePathPart::Index(index));
        }
        let Some((element, offset)) = Self::fixed_descriptor_path(&descriptor, &aggregate_path)
        else {
            return Ok(None);
        };
        let Some(width) = Self::fixed_descriptor_width(&element) else {
            return Ok(None);
        };
        if width == 0 {
            return Ok(None);
        }
        let Some(value) = self.lower_fixed_array_parameter_value(path, parameter)? else {
            return Ok(None);
        };
        let Some(end) = offset.checked_add(width).filter(|end| *end <= value.width) else {
            return Err(format!(
                "fixed parameter member array projection exceeds its aggregate value in `{path}`"
            ));
        };
        let selected = IrExpr::new(
            IrExprKind::PartSel {
                base: Box::new(value),
                left: i64::from(end - 1),
                right: i64::from(offset),
            },
            width,
            element.info.signed,
            None,
        );
        ir_to_storage(selected, width, element.info.signed, element.two_state).map(Some)
    }

    fn lower_fixed_array_parameter_partial(
        &mut self,
        path: &str,
        base: NodeId,
        indices: &[NodeId],
    ) -> Result<Option<IrExpr>, String> {
        let Some(parameter) = self.fixed_array_parameter_target(base) else {
            return Ok(None);
        };
        let Some(descriptor) = self.query_descriptor(parameter).cloned() else {
            return Ok(None);
        };
        let TypeShape::FixedArray { dimensions, .. } = &descriptor.shape else {
            return Ok(None);
        };
        if indices.is_empty() || indices.len() >= dimensions.len() {
            return Ok(None);
        }
        let Some(mut initializer) = self.fixed_array_parameter_initializer(parameter) else {
            return Ok(None);
        };
        for (index_node, &(left, right)) in indices.iter().zip(dimensions) {
            let Ok(index) = self.eval_bound_i128(*index_node) else {
                return Ok(None);
            };
            let index = i32::try_from(index)
                .map_err(|_| format!("fixed-array parameter index does not fit i32 in `{path}`"))?;
            let offset = if left >= right {
                i64::from(left) - i64::from(index)
            } else {
                i64::from(index) - i64::from(left)
            };
            let count = i64::from(left).abs_diff(i64::from(right)) + 1;
            let Some(offset) = usize::try_from(offset)
                .ok()
                .filter(|offset| u64::try_from(*offset).is_ok_and(|offset| offset < count))
            else {
                return Ok(None);
            };
            let values = self.p30_pattern_level(path, initializer, (left, right))?;
            let Some(value) = values.get(offset) else {
                return Ok(None);
            };
            initializer = *value;
        }
        self.fixed_pattern_value(path, initializer)
    }

    fn lower_tagged_union_constructor(
        &mut self,
        scope_path: &str,
        node: NodeId,
        member_name: &str,
        value_node: Option<NodeId>,
    ) -> Result<IrExpr, String> {
        let descriptor = self
            .query_descriptor(node)
            .cloned()
            .ok_or_else(|| format!("tagged union constructor has no type in `{scope_path}`"))?;
        let TypeShape::Aggregate(layout) = &descriptor.shape else {
            return Err(format!(
                "tagged union constructor has a non-aggregate type in `{scope_path}`"
            ));
        };
        if layout.kind != AggregateKind::TaggedUnion {
            return Err(format!(
                "tagged constructor targets a non-tagged union in `{scope_path}`"
            ));
        }
        let member_index = layout
            .members
            .iter()
            .position(|member| member.name == member_name)
            .ok_or_else(|| {
                format!("tagged union has no member `{member_name}` in `{scope_path}`")
            })?;
        let member = &layout.members[member_index];
        let tag_width = layout
            .tag_bits()
            .ok_or_else(|| format!("tagged union tag width overflows in `{scope_path}`"))?;
        if tag_width == 0 && layout.members.len() != 1 {
            return Err(format!(
                "tagged union `{}` has no representable tag bits in `{scope_path}`",
                descriptor.name
            ));
        }
        let payload_width = layout
            .payload_bits()
            .ok_or_else(|| format!("tagged union payload width overflows in `{scope_path}`"))?;
        let total_width = tag_width
            .checked_add(payload_width)
            .ok_or_else(|| format!("tagged union width overflows in `{scope_path}`"))?;
        // Unpacked tagged unions have no frontend packed width; their storage
        // uses the same finite tag-plus-payload layout as a packed union.
        if descriptor
            .info
            .width
            .filter(|width| *width != 0)
            .or_else(|| Self::fixed_descriptor_width(&descriptor))
            != Some(total_width)
        {
            return Err(format!(
                "tagged union `{}` is not a fixed packed value in `{scope_path}`",
                descriptor.name
            ));
        }
        if total_width == 0 {
            return Err(format!(
                "tagged union `{}` has zero storage width in `{scope_path}`",
                descriptor.name
            ));
        }
        let member_width = if matches!(
            &member.descriptor.shape,
            TypeShape::Opaque { kind } if kind == "Void"
        ) {
            0
        } else {
            Self::fixed_descriptor_width(&member.descriptor).ok_or_else(|| {
                format!(
                    "tagged union member `{member_name}` has no fixed packed width in `{scope_path}`"
                )
            })?
        };
        if member_width > payload_width {
            return Err(format!(
                "tagged union member `{member_name}` exceeds payload width in `{scope_path}`"
            ));
        }
        let fill = if descriptor.two_state { 0 } else { 2 };
        let payload = match value_node {
            Some(_value_node) if member_width == 0 => {
                return Err(format!(
                    "void tagged union member `{member_name}` has a value in `{scope_path}`"
                ));
            }
            Some(value_node) => {
                let value = ir_to_storage(
                    self.lower_expr(scope_path, value_node)?,
                    member_width,
                    member.descriptor.info.signed,
                    member.descriptor.two_state,
                )?;
                let padding = payload_width - member_width;
                if padding == 0 {
                    value
                } else {
                    IrExpr::new(
                        IrExprKind::Concat {
                            parts: vec![
                                IrExpr::new(IrExprKind::Fill(fill), padding, false, Some(fill)),
                                value,
                            ],
                        },
                        payload_width,
                        false,
                        None,
                    )
                }
            }
            None if payload_width == 0 => {
                return Ok(IrExpr::new(
                    IrExprKind::Const(
                        IrConst::packed(
                            vec![u64::try_from(member_index).map_err(|_| {
                                format!("tagged union member index overflows in `{scope_path}`")
                            })?],
                            vec![0],
                            vec![0],
                            tag_width,
                            false,
                            None,
                        )
                        .map_err(|error| error.to_string())?,
                    ),
                    total_width,
                    descriptor.info.signed,
                    None,
                ))
            }
            None => IrExpr::new(IrExprKind::Fill(fill), payload_width, false, Some(fill)),
        };
        if tag_width == 0 {
            return Ok(payload);
        }
        let tag = IrConst::packed(
            vec![u64::try_from(member_index)
                .map_err(|_| format!("tagged union member index overflows in `{scope_path}`"))?],
            vec![0],
            vec![0],
            tag_width,
            false,
            None,
        )
        .map_err(|error| error.to_string())?;
        Ok(IrExpr::new(
            IrExprKind::Concat {
                parts: vec![
                    IrExpr::new(IrExprKind::Const(tag), tag_width, false, None),
                    payload,
                ],
            },
            total_width,
            descriptor.info.signed,
            None,
        ))
    }

    fn packed_parameter_projection(
        &self,
        parameter: NodeId,
        member: &PackedMember,
        lsb: u32,
        width: u32,
        signed: bool,
    ) -> Result<IrExpr, String> {
        let Some(Val::Bits(value)) = self.param_vals.get(&parameter) else {
            return Err(format!(
                "packed parameter `{}` has no integral value for member read",
                self.node(parameter).name
            ));
        };
        let lsb = usize::try_from(lsb)
            .map_err(|_| format!("packed member `{}` offset does not fit usize", member.name))?;
        let width = usize::try_from(width)
            .map_err(|_| format!("packed member `{}` width does not fit usize", member.name))?;
        let end = lsb
            .checked_add(width)
            .ok_or_else(|| format!("packed member `{}` range overflows", member.name))?;
        if width == 0 || end > value.width() {
            return Err(format!(
                "packed member `{}` exceeds parameter `{}` width",
                member.name,
                self.node(parameter).name
            ));
        }
        let start = value.width() - end;
        let selected_bits = value
            .bits
            .get(start..start + width)
            .ok_or_else(|| format!("packed member `{}` is outside parameter value", member.name))?;
        let mut projected = value.clone();
        projected.bits = selected_bits.to_vec();
        projected.signed = signed;
        projected.fill = None;
        let constant = val_to_const(&projected)?;
        let selected = IrExpr::new(
            IrExprKind::Const(constant.clone()),
            constant.width,
            constant.signed,
            constant.fill,
        );
        Ok(if member.two_state {
            IrExpr::to_two_state(selected)
        } else {
            selected
        })
    }

    fn packed_parameter_member_read(&self, node: NodeId) -> Result<Option<IrExpr>, String> {
        let Some((parameter, member)) = self.packed_parameter_member_info(node) else {
            return Ok(None);
        };
        Ok(Some(self.packed_parameter_projection(
            parameter,
            &member,
            member.lsb,
            member.width,
            member.signed,
        )?))
    }

    fn packed_parameter_member_select_read(
        &self,
        base: NodeId,
        indices: &[NodeId],
    ) -> Result<Option<IrExpr>, String> {
        let Some((parameter, member, lsb, width)) =
            self.packed_parameter_member_select_info(base, indices)?
        else {
            return Ok(None);
        };
        Ok(Some(self.packed_parameter_projection(
            parameter, &member, lsb, width, false,
        )?))
    }

    fn packed_parameter_member_dynamic_select_read(
        &mut self,
        path: &str,
        base: NodeId,
        indices: &[NodeId],
    ) -> Result<Option<IrExpr>, String> {
        let Some((parameter, member, base_idx, width)) =
            self.packed_parameter_member_dynamic_select_info(path, base, indices)?
        else {
            return Ok(None);
        };
        // Restrict the selector base to the member itself so an invalid index
        // cannot read neighboring fields from the containing packed value.
        let value = self.packed_parameter_projection(
            parameter,
            &member,
            member.lsb,
            member.width,
            member.signed,
        )?;
        let selected = IrExpr::new(
            IrExprKind::IdxPartSel {
                base: Box::new(value),
                base_idx: Box::new(base_idx),
                width_expr: Box::new(lhs_integer_expr(i128::from(width))),
                neg: false,
            },
            width,
            false,
            None,
        );
        Ok(Some(if member.two_state {
            IrExpr::to_two_state(selected)
        } else {
            selected
        }))
    }

    /// Render context for the IR built so far (the enclosing function, when
    /// any, resolves formal reads).
    #[allow(dead_code)] // retained for fragment callers pending the lowering migration
    pub(in super::super) fn render_ctx(&self) -> RCtx<'_> {
        RCtx {
            value_backend: crate::sim::value_backend::ValueBackend::Legacy,
            model: &self.model,
            func: self.cur_fn_ir.map(|i| &self.model.funcs[i]),
            sampled: false,
            activation_label: None,
            constants: None,
        }
    }

    /// Render a lowered expression to its C code.
    #[allow(dead_code)] // retained for fragment callers pending the lowering migration
    pub(in super::super) fn render_ir_code(&self, ir: &IrExpr) -> Result<String, String> {
        let ctx = self.render_ctx();
        Ok(render_expr(&ctx, ir)?.code)
    }

    /// Lower an expression node decision-for-decision like the pre-IR
    /// emitter: same widths, signednesses, fills, and error strings.
    pub(in super::super) fn lower_expr(
        &mut self,
        scope_path: &str,
        h: NodeId,
    ) -> Result<IrExpr, String> {
        if let Some(value) = self.overload_current_read(scope_path, h) {
            return value;
        }
        if self.packed_element_member_select(h).is_some() {
            if let Some(value) = self.packed_value_read_ir(scope_path, h)? {
                return Ok(value);
            }
        }
        let partial_array_select = match self.kind(h) {
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                Some((*base, indices.clone()))
            }
            _ => None,
        };
        if let Some((base, indices)) = partial_array_select {
            if let Some(value) =
                self.lower_fixed_array_parameter_partial(scope_path, base, &indices)?
            {
                return Ok(value);
            }
        }
        if let Some(value) = self.fixed_activation_read(scope_path, h)? {
            return Ok(value);
        }
        if let Some(value) = self.fixed_pattern_value(scope_path, h)? {
            return Ok(value);
        }
        if !matches!(self.kind(h), NodeKind::Expr(ExprKind::Cast { .. }))
            && (self.array_of(h).is_some() || self.unpacked_aggregate_info(h).is_some())
            && self.packed_member_info(h).is_none()
        {
            if let Some(value) = self.lower_bitstream_source(scope_path, h)? {
                return Ok(value);
            }
        }
        if let Some(value) = self.packed_formal_read(scope_path, h)? {
            return Ok(value);
        }
        if let Some(value) = self.lower_fixed_array_reduction(scope_path, h)? {
            return Ok(value);
        }
        if let Some(value) = self.lower_container_query(scope_path, h)? {
            return Ok(value);
        }
        if let Some(value) = self.lower_object_query(scope_path, h)? {
            return Ok(value);
        }
        if let Some(value) = self.virtual_interface_member_expr(scope_path, h)? {
            return Ok(value);
        }
        if let Some(value) = self.class_field_expr(scope_path, h)? {
            return Ok(value);
        }
        if let Some(value) = self.lower_enum_method(scope_path, h)? {
            return Ok(value);
        }
        if let NodeKind::MethodCall { name, .. } = self.kind(h) {
            if self.virtual_interface_method_info(h)?.is_some() {
                return self.lower_func_call_expr(scope_path, h, name, None);
            }
        }
        if matches!(self.kind(h), NodeKind::MethodCall { .. }) && self.is_class_method_call(h) {
            let (name, callee) = match self.kind(h) {
                NodeKind::MethodCall { name, callee, .. } => (name.clone(), *callee),
                _ => return Err("malformed class method call".to_owned()),
            };
            let receiver = self.class_method_receiver(h)?;
            let mut value = self.lower_func_call_expr(scope_path, h, &name, callee)?;
            if let IrExprKind::CallFn(call) = &mut value.kind {
                call.receiver = receiver;
                call.virtual_dispatch = self.class_method_virtual_dispatch(h);
            }
            return Ok(value);
        }
        match self.kind(h) {
            NodeKind::Expr(ExprKind::TaggedUnion { member, value }) => {
                self.lower_tagged_union_constructor(scope_path, h, member, *value)
            }
            NodeKind::Expr(ExprKind::Conditional {
                predicate,
                if_true,
                if_false,
            }) => self.lower_predicate_conditional(scope_path, predicate, *if_true, *if_false),
            NodeKind::Expr(ExprKind::Constant { .. }) => {
                if let Some(comparison) = self.recover_folded_real_parameter_comparison(h) {
                    return Ok(comparison);
                }
                let c = self.const_of_node(h)?;
                Ok(IrExpr::new(
                    IrExprKind::Const(c.clone()),
                    c.width,
                    c.signed,
                    c.fill,
                ))
            }
            NodeKind::EnumConst { value } => match value {
                Some(Val::Bits(v)) => {
                    let c = val_to_const(v)?;
                    Ok(IrExpr::new(
                        IrExprKind::Const(c.clone()),
                        c.width,
                        c.signed,
                        None,
                    ))
                }
                Some(Val::Real(v)) => Ok(real_literal_expr(*v)),
                Some(Val::Str(_)) => Err("string enum constant in expression".to_string()),
                None => Err("enum constant without value in expression".to_string()),
            },
            NodeKind::Expr(ExprKind::ScopeRef { target }) => {
                if let Some(info) = self.sampled_signal_of(*target) {
                    self.signal_read_expr(info)
                } else if let Some(info) = self.clocking_var_read_source_info(*target)? {
                    self.signal_read_expr(info)
                } else {
                    Err(format!(
                        "clocking block scope `{}` is not a value in `{scope_path}`",
                        self.node(*target).name
                    ))
                }
            }
            NodeKind::Expr(ExprKind::Ref { target }) => self.lower_ref_expr(scope_path, h, *target),
            NodeKind::Expr(
                ExprKind::BitSelect { .. }
                | ExprKind::PartSelect { .. }
                | ExprKind::IndexedPartSelect { .. }
                | ExprKind::ArraySelect { .. },
            ) if self.is_modport_select_root(h) => {
                let (base, steps) = self.modport_select_step(scope_path, h)?.ok_or_else(|| {
                    format!("unsupported modport port selection in `{scope_path}`")
                })?;
                let value = self.lower_expr(scope_path, base)?;
                Ok(steps.into_iter().fold(value, packed_step_read))
            }
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                if let Some(value) =
                    self.packed_parameter_member_dynamic_select_read(scope_path, *base, &[*index])?
                {
                    return Ok(value);
                }
                if let Some(value) = self.packed_element_read_ir(scope_path, h)? {
                    return Ok(value);
                }
                if let Some(value) = self.packed_value_read_ir(scope_path, h)? {
                    return Ok(value);
                }
                self.ensure_clocking_readable(*base)?;
                if let Some(ai) = self.array_of(*base).cloned() {
                    if ai.real {
                        return Err(format!(
                            "select on a real array element in `{scope_path}` is not supported"
                        ));
                    }
                    if ai.dims.len() != 1 {
                        return Err(format!(
                            "array slice access (`{}[...]` on a {}-dimensional array) \
                             is not supported in `{scope_path}`",
                            self.node(*base).name,
                            ai.dims.len()
                        ));
                    }
                    let ie = self.lower_expr(scope_path, *index)?;
                    return Ok(IrExpr::new(
                        IrExprKind::ArrayRead {
                            arr: self.reference_array(ai.ir),
                            indices: vec![ie],
                            elem_sel: IrElemSel::Whole,
                        },
                        ai.elem_width,
                        ai.signed,
                        None,
                    ));
                }
                if let Some((info, member, lsb, width)) =
                    self.packed_member_select_info(*base, &[*index])?
                {
                    let selected = IrExpr::new(
                        IrExprKind::PartSel {
                            base: Box::new(self.signal_read_expr(&info)?),
                            left: i64::from(lsb) + i64::from(width) - 1,
                            right: i64::from(lsb),
                        },
                        width,
                        false,
                        None,
                    );
                    return Ok(if member.two_state {
                        IrExpr::to_two_state(selected)
                    } else {
                        selected
                    });
                }
                if let Some(value) = self.packed_parameter_member_select_read(*base, &[*index])? {
                    return Ok(value);
                }
                if let Some((info, lsb, width)) = self.packed_select_info(*base, &[*index])? {
                    if width > 1 {
                        let right = i64::from(lsb);
                        return Ok(IrExpr::new(
                            IrExprKind::PartSel {
                                base: Box::new(self.signal_read_expr(&info)?),
                                left: right + i64::from(width) - 1,
                                right,
                            },
                            width,
                            false,
                            None,
                        ));
                    }
                }
                let base_value = self.lower_expr(scope_path, *base)?;
                if base_value.is_real() {
                    return Err(format!(
                        "select on real-valued signal in `{scope_path}` is not supported"
                    ));
                }
                let ie = self.lower_member_select_index(scope_path, *base, *index)?;
                Ok(IrExpr::new(
                    IrExprKind::BitSel {
                        base: Box::new(base_value),
                        idx: Box::new(ie),
                    },
                    1,
                    false,
                    None,
                ))
            }
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                if let Some(value) =
                    self.packed_parameter_member_dynamic_select_read(scope_path, *base, indices)?
                {
                    return Ok(value);
                }
                if let Some(value) = self.packed_element_read_ir(scope_path, h)? {
                    return Ok(value);
                }
                if let Some(value) = self.packed_value_read_ir(scope_path, h)? {
                    return Ok(value);
                }
                self.ensure_clocking_readable(*base)?;
                if let Some((_target, _kind, member_info)) = self.unpacked_member_info(h) {
                    let member = member_info.member;
                    let signal = member_info.signal.ok_or_else(|| {
                        if member_info.object.is_some() {
                            format!(
                                "string aggregate member `{}` must be used in a string context",
                                member.name
                            )
                        } else {
                            format!("aggregate member `{}` has no scalar storage", member.name)
                        }
                    })?;
                    let value = if signal.real {
                        self.signal_read_expr(&signal)?
                    } else {
                        IrExpr::resize_to(
                            self.signal_read_expr(&signal)?,
                            member.ty.width.ok_or_else(|| {
                                format!("unpacked member `{}` has unresolved width", member.name)
                            })?,
                            member.ty.signed,
                        )
                    };
                    let value = self.guard_tagged_member_read(h, &signal, value)?;
                    return Ok(if member.two_state && !signal.real {
                        IrExpr::to_two_state(value)
                    } else {
                        value
                    });
                }
                if let Some((info, member, lsb, width)) =
                    self.packed_member_select_info(*base, indices)?
                {
                    let selected = IrExpr::new(
                        IrExprKind::PartSel {
                            base: Box::new(self.signal_read_expr(&info)?),
                            left: i64::from(lsb) + i64::from(width) - 1,
                            right: i64::from(lsb),
                        },
                        width,
                        false,
                        None,
                    );
                    return Ok(if member.two_state {
                        IrExpr::to_two_state(selected)
                    } else {
                        selected
                    });
                }
                if let Some(value) = self.packed_parameter_member_select_read(*base, indices)? {
                    return Ok(value);
                }
                if let Some(value) =
                    self.lower_fixed_parameter_member_array_element(scope_path, h)?
                {
                    return Ok(value);
                }
                if let Some(value) =
                    self.lower_fixed_array_parameter_element(scope_path, *base, indices)?
                {
                    return Ok(value);
                }
                if let Some((info, lsb, width)) = self.packed_select_info(*base, indices)? {
                    let right = i64::from(lsb);
                    return Ok(IrExpr::new(
                        IrExprKind::PartSel {
                            base: Box::new(self.signal_read_expr(&info)?),
                            left: right + i64::from(width) - 1,
                            right,
                        },
                        width,
                        false,
                        None,
                    ));
                }
                let ai = self
                    .record_column_select(h)
                    .map(|(column, _)| column)
                    .or_else(|| self.array_of(*base))
                    .cloned()
                    .ok_or_else(|| {
                    format!(
                        "cannot resolve array base of select `{}` in `{scope_path}` (base kind: {:?})",
                        self.node(*base).name,
                        self.kind(*base)
                    )
                })?;
                let ndims = ai.dims.len();
                if indices.len() == ndims {
                    let ies = indices
                        .iter()
                        .map(|i| self.lower_expr(scope_path, *i))
                        .collect::<Result<Vec<_>, _>>()?;
                    let element = IrExpr::new(
                        IrExprKind::ArrayRead {
                            arr: self.reference_array(ai.ir),
                            indices: ies,
                            elem_sel: IrElemSel::Whole,
                        },
                        ai.elem_width,
                        ai.signed,
                        None,
                    );
                    if let Some(guard) = self.record_select_guard(h)? {
                        // A member array of a column-layout tagged union
                        // reads only while its member is active.
                        let (tag, guard) = (guard.tag_read, guard.guard);
                        let tag_width = tag.width;
                        return Ok(IrExpr::new(
                            IrExprKind::TaggedSelect {
                                base: Box::new(IrExpr::new(
                                    IrExprKind::Concat {
                                        parts: vec![tag, element],
                                    },
                                    tag_width + ai.elem_width,
                                    false,
                                    None,
                                )),
                                steps: vec![crate::sim::ir::IrTaggedSelectStep {
                                    selection: crate::sim::ir::IrPackedSelect {
                                        base: lhs_integer_expr(0),
                                        width: ai.elem_width,
                                    },
                                    two_state: false,
                                    guard: Some(guard),
                                }],
                                location: self.source_location(h),
                            },
                            ai.elem_width,
                            ai.signed,
                            None,
                        ));
                    }
                    return Ok(element);
                }
                if indices.len() == ndims + 1 {
                    if ai.real {
                        return Err(format!(
                            "select on a real array element in `{scope_path}` is not supported"
                        ));
                    }
                    let last = *indices.last().expect("non-empty indices");
                    let ies = indices[..ndims]
                        .iter()
                        .map(|i| self.lower_expr(scope_path, *i))
                        .collect::<Result<Vec<_>, _>>()?;
                    let (elem_sel, width) = match self.kind(last) {
                        NodeKind::Expr(ExprKind::PartSelect { left, right, .. }) => {
                            let l =
                                self.packed_relative_bound(*base, self.eval_bound_i128(*left)?)?;
                            let r =
                                self.packed_relative_bound(*base, self.eval_bound_i128(*right)?)?;
                            let (l, r, width) =
                                checked_select_bounds(l, r, "array-element part select")?;
                            (IrElemSel::Part(l, r), width)
                        }
                        NodeKind::Expr(ExprKind::IndexedPartSelect {
                            base_expr,
                            width_expr,
                            neg,
                            ..
                        }) => {
                            let width = self.indexed_part_select_width(*width_expr, scope_path)?;
                            (
                                IrElemSel::Indexed {
                                    base: Box::new(
                                        self.lower_packed_index(scope_path, *base, *base_expr)?,
                                    ),
                                    width,
                                    negative: *neg ^ self.packed_range_ascending(*base),
                                },
                                width,
                            )
                        }
                        _ => {
                            let ie = self.lower_packed_index(scope_path, *base, last)?;
                            (IrElemSel::Bit(Box::new(ie)), 1)
                        }
                    };
                    return Ok(IrExpr::new(
                        IrExprKind::ArrayRead {
                            arr: self.reference_array(ai.ir),
                            indices: ies,
                            elem_sel,
                        },
                        width,
                        false,
                        None,
                    ));
                }
                Err(format!(
                    "array `{}` in `{scope_path}`: {}-level select on a \
                     {}-dimensional array is not supported",
                    self.node(*base).name,
                    indices.len(),
                    ndims
                ))
            }
            NodeKind::Expr(ExprKind::PartSelect { base, left, right }) => {
                if let Some(value) = self.packed_element_read_ir(scope_path, h)? {
                    return Ok(value);
                }
                if let Some(value) = self.packed_value_read_ir(scope_path, h)? {
                    return Ok(value);
                }
                self.ensure_clocking_readable(*base)?;
                let base_value = self.lower_expr(scope_path, *base)?;
                if base_value.is_real() {
                    return Err(format!(
                        "select on real-valued signal in `{scope_path}` is not supported"
                    ));
                }
                if let Some((info, member, lsb, width)) = self.packed_member_range_info(
                    *base,
                    self.eval_bound_i128(*left)?,
                    self.eval_bound_i128(*right)?,
                )? {
                    let selected = IrExpr::new(
                        IrExprKind::PartSel {
                            base: Box::new(self.signal_read_expr(&info)?),
                            left: i64::from(lsb) + i64::from(width) - 1,
                            right: i64::from(lsb),
                        },
                        width,
                        false,
                        None,
                    );
                    return Ok(if member.two_state {
                        IrExpr::to_two_state(selected)
                    } else {
                        selected
                    });
                }
                let mut l = self.eval_bound_i128(*left)?;
                let mut r = self.eval_bound_i128(*right)?;
                if let Some((_, member)) = self.packed_member_info(*base) {
                    l = i128::from(self.aggregate_member_relative_bound(
                        &member.name,
                        &member.packed_ranges,
                        l,
                    )?);
                    r = i128::from(self.aggregate_member_relative_bound(
                        &member.name,
                        &member.packed_ranges,
                        r,
                    )?);
                } else {
                    l = self.packed_relative_bound(*base, l)?;
                    r = self.packed_relative_bound(*base, r)?;
                }
                let (l, r, width) = checked_select_bounds(l, r, "part select")?;
                Ok(IrExpr::new(
                    IrExprKind::PartSel {
                        base: Box::new(base_value),
                        left: l,
                        right: r,
                    },
                    width,
                    false,
                    None,
                ))
            }
            NodeKind::Expr(ExprKind::IndexedPartSelect {
                base,
                base_expr,
                width_expr,
                neg,
            }) => {
                if let Some(value) = self.packed_element_read_ir(scope_path, h)? {
                    return Ok(value);
                }
                if let Some(value) = self.packed_value_read_ir(scope_path, h)? {
                    return Ok(value);
                }
                self.ensure_clocking_readable(*base)?;
                let base_value = self.lower_expr(scope_path, *base)?;
                if base_value.is_real() {
                    return Err(format!(
                        "select on real-valued signal in `{scope_path}` is not supported"
                    ));
                }
                let be = self.lower_packed_index(scope_path, *base, *base_expr)?;
                let we = self.lower_expr(scope_path, *width_expr)?;
                let width = self.indexed_part_select_width(*width_expr, scope_path)?;
                Ok(IrExpr::new(
                    IrExprKind::IdxPartSel {
                        base: Box::new(base_value),
                        base_idx: Box::new(be),
                        width_expr: Box::new(we),
                        neg: *neg ^ self.packed_range_ascending(*base),
                    },
                    width,
                    false,
                    None,
                ))
            }
            NodeKind::Expr(ExprKind::Streaming {
                direction,
                slice_size,
                streams,
            }) => {
                if streams.is_empty() {
                    return Err(format!("empty streaming concatenation in `{scope_path}`"));
                }
                let mut parts = Vec::with_capacity(streams.len());
                for stream in streams {
                    let value =
                        self.lower_stream_operand(scope_path, stream.value, stream.with_expr)?;
                    if value.is_real() {
                        return Err(format!(
                            "streaming concatenation of real value in `{scope_path}` is not supported"
                        ));
                    }
                    parts.push(value);
                }
                let runtime_sized = parts.iter().any(|part| part.width == LLG_MAX_WIDTH);
                let width = if runtime_sized {
                    Some(LLG_MAX_WIDTH)
                } else {
                    parts.iter().try_fold(0u32, |width, part| {
                        width
                            .checked_add(part.width)
                            .filter(|width| *width <= LLG_MAX_WIDTH)
                    })
                }
                .ok_or_else(|| {
                    format!("streaming concatenation in `{scope_path}` has an oversized operand")
                })?;
                let value = if let [value] = parts.as_slice() {
                    value.clone()
                } else {
                    IrExpr::new(IrExprKind::Concat { parts }, width, false, None)
                };
                let slice = if *slice_size == 0 {
                    1
                } else {
                    (*slice_size).min(u64::from(width)) as u32
                };
                Ok(IrExpr::new(
                    IrExprKind::Stream {
                        value: Box::new(value),
                        slice,
                        direction: match direction {
                            DbStreamingDirection::LeftToRight => IrStreamDirection::LeftToRight,
                            DbStreamingDirection::RightToLeft => IrStreamDirection::RightToLeft,
                        },
                    },
                    width,
                    false,
                    None,
                ))
            }
            NodeKind::Expr(ExprKind::Operation {
                op,
                reordered,
                assignment,
                operands,
            }) if !*reordered
                && !*assignment
                && matches!(op, Operation::LogicalAnd | Operation::LogicalOr)
                && operands.len() == 2 =>
            {
                self.lower_logical_chain(scope_path, *op, operands)
            }
            NodeKind::Expr(ExprKind::Operation {
                op,
                reordered,
                assignment,
                operands,
            }) => self.lower_operation(scope_path, *op, *reordered, *assignment, operands),
            NodeKind::Expr(ExprKind::Cast {
                operand,
                ty,
                size_cast,
                size_cast_expr,
                cast_kind_known,
                two_state,
                propagated,
            }) => {
                if !cast_kind_known {
                    return Err(format!(
                        "cast kind cannot be determined without admitted source or semantic type metadata in `{scope_path}`"
                    ));
                }
                if matches!(ty.kind.as_str(), "real" | "shortreal") {
                    let v = self.lower_expr(scope_path, *operand)?;
                    return Ok(IrExpr::new(
                        IrExprKind::CastToReal {
                            a: Box::new(v),
                            shortreal: ty.kind == "shortreal",
                        },
                        REAL_EXPR_WIDTH,
                        true,
                        None,
                    ));
                }
                let bitstream_source = self.lower_bitstream_source(scope_path, *operand)?;
                let source_value = match &bitstream_source {
                    Some(value) => value.clone(),
                    None => self.lower_expr(scope_path, *operand)?,
                };
                let target_width = size_cast_expr
                    .as_deref()
                    .and_then(|expression| self.source_size_cast_width(expression))
                    .or_else(|| self.fixed_value_width(h))
                    .or(ty.width);
                let (w, s) = match (target_width, ty.signed) {
                    (Some(w), s) => (w, if *size_cast { source_value.signed } else { s }),
                    (None, _) => {
                        return Err(format!(
                            "cast with unsized target type `{}` in `{scope_path}`",
                            ty.kind
                        ))
                    }
                };
                let v = source_value;
                if w > LLG_MAX_WIDTH {
                    return Err(format!(
                        "cast target in `{scope_path}` is {w} bits wide; the v1 \
                         runtime maximum supported width is {LLG_MAX_WIDTH}"
                    ));
                }
                // Context propagation extends using the target signedness
                // (§11.8.2); assignment and explicit casts use the source (§11.8.3).
                let v = if *propagated {
                    let source_width = v.width;
                    IrExpr::resize_to(v, source_width, s)
                } else {
                    v
                };
                // Slang wraps a streaming source in a conversion to its
                // assignment target. A wider fixed target receives the stream
                // left-aligned and zero-filled on the right (SV 11.4.14); an
                // explicit bit-stream cast of a stream has equal widths.
                let streaming_operand = matches!(
                    self.kind(*operand),
                    NodeKind::Expr(ExprKind::Streaming { .. })
                );
                // A runtime `with`/container stream learns its width only when
                // evaluated, so the runtime aligns it (and rejects an oversize
                // stream) against the fixed target.
                if bitstream_source.is_none()
                    && streaming_operand
                    && !v.is_real()
                    && v.width == LLG_MAX_WIDTH
                    && w < LLG_MAX_WIDTH
                {
                    let aligned =
                        IrExpr::new(IrExprKind::StreamToFixed { a: Box::new(v) }, w, s, None);
                    let aligned = if *two_state || is_two_state_kind(&ty.kind) {
                        IrExpr::to_two_state(aligned)
                    } else {
                        aligned
                    };
                    return self.convert_fixed_payload(h, aligned);
                }
                if bitstream_source.is_none() && streaming_operand && !v.is_real() && v.width < w {
                    let stream_width = v.width;
                    let aligned = IrExpr::new(
                        IrExprKind::Concat {
                            parts: vec![v, const_zero_expr(w - stream_width)],
                        },
                        w,
                        false,
                        None,
                    );
                    let aligned = if *two_state || is_two_state_kind(&ty.kind) {
                        IrExpr::to_two_state(aligned)
                    } else {
                        aligned
                    };
                    return self.convert_fixed_payload(h, IrExpr::resize_to(aligned, w, s));
                }
                if let Some(source) = bitstream_source {
                    if source.width != w {
                        return Err(format!(
                            "bit-stream cast source is {} bits but target is {} bits in `{scope_path}`",
                            source.width, w
                        ));
                    }
                    let value = IrExpr::new(
                        IrExprKind::BitStreamCast {
                            a: Box::new(source),
                            source_width: w,
                            target_two_state: *two_state || is_two_state_kind(&ty.kind),
                        },
                        w,
                        s,
                        None,
                    );
                    return self.convert_fixed_payload(h, value);
                }
                // Value-preserving conversion (LRM 1800-2009 §6.24.1: the
                // cast yields the value a variable of the cast type holds
                // after the assignment — extension follows the SOURCE's
                // signedness, so int'(8'hFF) is 255, not -1).
                let value = ir_to_explicit_cast_storage(
                    v,
                    w,
                    s,
                    *two_state || is_two_state_kind(&ty.kind),
                )?;
                self.convert_fixed_payload(h, value)
            }
            NodeKind::SysCall { name } => self.lower_sys_func_expr(scope_path, name, h),
            NodeKind::FuncCall {
                name,
                is_task,
                callee,
                ..
            } => {
                if *is_task {
                    return Err(format!(
                        "task call `{name}` used as an expression in `{scope_path}`"
                    ));
                }
                let mut value = self.lower_func_call_expr(scope_path, h, name, *callee)?;
                if let IrExprKind::CallFn(call) = &mut value.kind {
                    call.virtual_dispatch = self.class_method_virtual_dispatch(h);
                }
                Ok(value)
            }
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
                ..
            } if name == "triggered" => {
                let target = self.event_target_of(*receiver).ok_or_else(|| {
                    format!(
                        "sequence `.triggered` status is not supported for an unresolved receiver in `{scope_path}`"
                    )
                })?;
                let event = self.event_ref_of(&target, scope_path)?;
                Ok(IrExpr::new(
                    IrExprKind::EventTriggered(event),
                    1,
                    false,
                    None,
                ))
            }
            NodeKind::MethodCall { name, .. } if name == "matched" => Err(format!(
                "sequence `.matched` status is not supported in `{scope_path}`"
            )),
            NodeKind::MethodCall { name, .. } if name == "triggered" => Err(format!(
                "sequence `.triggered` status is not supported in `{scope_path}`"
            )),
            NodeKind::Expr(ExprKind::HierPath { .. }) => {
                if let Some((_target, _kind, member_info)) = self.unpacked_member_info(h) {
                    let member = member_info.member;
                    let signal = member_info.signal.ok_or_else(|| {
                        if member_info.object.is_some() {
                            format!(
                                "string aggregate member `{}` must be used in a string context",
                                member.name
                            )
                        } else {
                            format!("aggregate member `{}` has no scalar storage", member.name)
                        }
                    })?;
                    let member_value = if signal.real {
                        self.signal_read_expr(&signal)?
                    } else {
                        IrExpr::resize_to(
                            self.signal_read_expr(&signal)?,
                            member.ty.width.ok_or_else(|| {
                                format!("unpacked member `{}` has unresolved width", member.name)
                            })?,
                            member.ty.signed,
                        )
                    };
                    let member_value = self.guard_tagged_member_read(h, &signal, member_value)?;
                    return Ok(if member.two_state && !member_value.is_real() {
                        IrExpr::to_two_state(member_value)
                    } else {
                        member_value
                    });
                }
                if let Some((info, member)) = self.packed_member_info(h) {
                    let base = self.signal_read_expr(&info)?;
                    let member_value = IrExpr::new(
                        IrExprKind::PartSel {
                            base: Box::new(base),
                            left: i64::from(member.lsb + member.width - 1),
                            right: i64::from(member.lsb),
                        },
                        member.width,
                        false,
                        None,
                    );
                    let member_value = self.guard_tagged_member_read(h, &info, member_value)?;
                    let selected = IrExpr::resize_to(member_value, member.width, member.signed);
                    return Ok(if member.two_state {
                        IrExpr::to_two_state(selected)
                    } else {
                        selected
                    });
                }
                if let Some(value) = self.packed_parameter_member_read(h)? {
                    return Ok(value);
                }
                if let Some(value) = self.lower_fixed_parameter_member_read(scope_path, h)? {
                    return Ok(value);
                }
                // Interface and ordinary hierarchical members both resolve
                // to their concrete owned storage identity.
                if let Some(info) = self.hier_path_signal(h) {
                    return self.signal_read_expr(info);
                }
                Err(format!(
                    "hierarchical reference `{}` is not supported (in `{scope_path}`): {:?}",
                    self.node(h).name,
                    self.kind(h)
                ))
            }
            other => Err(format!(
                "unsupported expression in `{scope_path}` (node kind {other:?})"
            )),
        }
    }

    fn recover_folded_real_parameter_comparison(&self, node: NodeId) -> Option<IrExpr> {
        let NodeKind::Expr(ExprKind::Constant {
            source: ConstantSource::Exact(source),
            size: 1,
            ..
        }) = self.kind(node)
        else {
            return None;
        };
        let (left, op, right) = [
            ("!=", IrBinOp::Neq),
            ("==", IrBinOp::Eq),
            ("<=", IrBinOp::Le),
            (">=", IrBinOp::Ge),
            ("<", IrBinOp::Lt),
            (">", IrBinOp::Gt),
        ]
        .into_iter()
        .find_map(|(token, op)| {
            source
                .split_once(token)
                .map(|(left, right)| (left.trim(), op, right.trim()))
        })?;

        let mut scope = self.node(node).parent;
        let mut lexical_scopes = Vec::new();
        let scope = loop {
            let candidate = scope?;
            if matches!(
                self.kind(candidate),
                NodeKind::ModuleInst { .. } | NodeKind::GenScope
            ) {
                break candidate;
            }
            lexical_scopes.push(candidate);
            scope = self.node(candidate).parent;
        };
        let real_parameter = |name: &str| {
            let shadowed = lexical_scopes.iter().any(|scope| {
                self.node(*scope).children.iter().any(|declaration| {
                    self.node(*declaration).name == name
                        && matches!(
                            self.kind(*declaration),
                            NodeKind::Var { .. }
                                | NodeKind::Array { .. }
                                | NodeKind::Param { .. }
                                | NodeKind::FuncArg { .. }
                        )
                })
            });
            if shadowed {
                return None;
            }
            self.node(scope).children.iter().find_map(|parameter| {
                (self.node(*parameter).name == name)
                    .then(|| self.param_vals.get(parameter))
                    .flatten()
                    .and_then(|value| match value {
                        Val::Real(value) => Some(*value),
                        Val::Bits(_) | Val::Str(_) => None,
                    })
            })
        };
        let reverse = |op| match op {
            IrBinOp::Lt => IrBinOp::Gt,
            IrBinOp::Le => IrBinOp::Ge,
            IrBinOp::Gt => IrBinOp::Lt,
            IrBinOp::Ge => IrBinOp::Le,
            other => other,
        };
        if let (Some(parameter), Some(literal)) =
            (real_parameter(left), parse_decimal_real_literal(right))
        {
            return Some(cmp_expr_ir(
                op,
                real_literal_expr(parameter),
                real_literal_expr(literal),
            ));
        }
        if let (Some(literal), Some(parameter)) =
            (parse_decimal_real_literal(left), real_parameter(right))
        {
            return Some(cmp_expr_ir(
                reverse(op),
                real_literal_expr(parameter),
                real_literal_expr(literal),
            ));
        }
        None
    }

    fn lower_ref_expr(
        &mut self,
        scope_path: &str,
        r: NodeId,
        target: Option<NodeId>,
    ) -> Result<IrExpr, String> {
        if let Some(iterator) = self.container_iterator {
            if target == Some(iterator.node) {
                return Ok(IrExpr::new(
                    IrExprKind::LocalRead("__llg_method_item".to_owned()),
                    iterator.item_width,
                    iterator.item_signed,
                    None,
                ));
            }
        }
        if let Some(target) = target {
            if let Some(binding) = self.assertion_local_binding(target)? {
                self.ensure_assertion_local_initializer(scope_path, target, binding)?;
                return Ok(IrExpr::new(
                    IrExprKind::LocalRead(format!(
                        "llg_sequence_local_read(data, {}u)",
                        binding.slot
                    )),
                    binding.width,
                    binding.signed,
                    None,
                ));
            }
        }
        if let Some(captured) = self
            .capture_target(r)
            .or_else(|| target.filter(|target| self.capture_locals.contains_key(target)))
        {
            let binding = self
                .capture_binding(captured)
                .expect("capture target must have a binding");
            return Ok(IrExpr::new(
                IrExprKind::LocalRead(Codegen::capture_local_name(binding.storage)),
                binding.local.width,
                binding.local.signed,
                None,
            ));
        }
        if self.lexical_proc_string_local(r).is_some() {
            return Err(format!(
                "string procedural local `{}` cannot be used as a packed expression in `{scope_path}`",
                self.node(r).name
            ));
        }
        if let Some((_, info)) = self.lexical_proc_local(r) {
            if let Some(signal) = &info.static_signal {
                return self.signal_read_expr(signal);
            }
            return Ok(IrExpr::new(
                IrExprKind::LocalRead(info.c_name.clone()),
                info.width,
                info.signed,
                None,
            ));
        }
        if let Some(expression) = target.and_then(|t| self.db.modport_port_expression(t)) {
            return self.lower_expr(scope_path, expression);
        }
        if let Some(t) = target {
            let t = self.canonical_func_target(t).unwrap_or(t);
            if let Some(binding) = self.capture_binding(t) {
                return Ok(IrExpr::new(
                    IrExprKind::LocalRead(Codegen::capture_local_name(binding.storage)),
                    binding.local.width,
                    binding.local.signed,
                    None,
                ));
            }
            if self.unpacked_aggregates.contains_key(&t) {
                return Err(format!(
                    "whole unpacked aggregate `{}` is not supported in scalar expression `{scope_path}`",
                    self.node(t).name
                ));
            }
            if let Some(info) = self.sampled_signal_of(t) {
                return self.signal_read_expr(info);
            }
            if let Some(info) = self.clocking_var_read_source_info(t)? {
                return self.signal_read_expr(info);
            }
            if let Some(info) = self.signal_of(t) {
                return self.signal_read_expr(info);
            }
            // Static subroutine formals have persistent model storage. A
            // hierarchical reference from outside the task/function body
            // resolves to the same formal declaration node, but there is no
            // active `FuncCtx` from which to read it.
            if let Some(info) = self.static_formals.get(&(self.inst, t)) {
                return self.signal_read_expr(info);
            }
            if !self.proc_local_is_shadowed(r) {
                if let Some(info) = self.proc_local_info(t) {
                    if let Some(signal) = &info.static_signal {
                        return self.signal_read_expr(signal);
                    }
                    return Ok(IrExpr::new(
                        IrExprKind::LocalRead(info.c_name.clone()),
                        info.width,
                        info.signed,
                        None,
                    ));
                }
            }
            // Function/task body reads: formals, locals and the return
            // variable (by arena node).
            if let Some(f) = &self.func {
                if let Some(storage) = f.persistent.get(&t) {
                    return self.signal_read_expr(storage);
                }
                if let Some(ir) = f.arg_ir.get(&t) {
                    return Ok(ir.clone());
                }
                if let Some((cname, w, s, _, _shortreal)) = f.locals.get(&t) {
                    return Ok(IrExpr::new(
                        IrExprKind::LocalRead(cname.clone()),
                        *w,
                        *s,
                        None,
                    ));
                }
                if f.ret_node == Some(t) {
                    if let Some(rctx) = &f.ret {
                        return Ok(IrExpr::new(
                            IrExprKind::LocalRead(rctx.c_name.clone()),
                            rctx.width,
                            rctx.signed,
                            None,
                        ));
                    }
                }
            }
            if let Some(v) = self.param_vals.get(&t) {
                return match v {
                    Val::Bits(b) => {
                        let c = val_to_const(b)?;
                        Ok(IrExpr::new(
                            IrExprKind::Const(c.clone()),
                            c.width,
                            c.signed,
                            None,
                        ))
                    }
                    Val::Real(value) => Ok(real_literal_expr(*value)),
                    Val::Str(value) => match self.kind(t) {
                        NodeKind::Param { ty, .. } if ty.kind != "string" => match ty.width {
                            Some(width) => {
                                let c = string_to_const(value)?;
                                let expr = IrExpr::new(
                                    IrExprKind::Const(c.clone()),
                                    c.width,
                                    c.signed,
                                    None,
                                );
                                Ok(IrExpr::convert_to(expr, width, ty.signed))
                            }
                            None => Err(format!(
                                "string parameter `{}` used as a value is not supported",
                                self.node(t).name
                            )),
                        },
                        _ => Err(format!(
                            "string parameter `{}` used as a value is not supported",
                            self.node(t).name
                        )),
                    },
                };
            }
            if let Some(value) = self.lower_fixed_array_parameter_value(scope_path, t)? {
                return Ok(value);
            }
            if let Some(value) = self.lower_subroutine_localparam_value(scope_path, t)? {
                return Ok(value);
            }
            if let NodeKind::EnumConst { value } = self.kind(t) {
                return enum_value_expr(value.as_ref(), &self.node(t).name);
            }
            return Err(format!(
                "cannot resolve bound expression reference `{}` in `{scope_path}`",
                self.node(r).name
            ));
        }
        // Unbound enum references can still arise in the flat definition
        // view. Any captured target identity must resolve above.
        let name = self.node(r).name.clone();
        if !name.is_empty() {
            // io_decls are not indexed, so formals resolve by name.
            if let Some(f) = &self.func {
                for io in sorted_node_ids(&f.arg_ir) {
                    if self.node(io).name == name {
                        return Ok(f.arg_ir[&io].clone());
                    }
                }
                for node in sorted_node_ids(&f.locals) {
                    let (cname, w, s, _, _shortreal) = &f.locals[&node];
                    if self.node(node).name == name {
                        if let Some(storage) = f.persistent.get(&node) {
                            return self.signal_read_expr(storage);
                        }
                        return Ok(IrExpr::new(
                            IrExprKind::LocalRead(cname.clone()),
                            *w,
                            *s,
                            None,
                        ));
                    }
                }
                if let Some(rctx) = &f.ret {
                    if rctx
                        .node
                        .map(|n| self.node(n).name == name)
                        .unwrap_or(false)
                    {
                        return Ok(IrExpr::new(
                            IrExprKind::LocalRead(rctx.c_name.clone()),
                            rctx.width,
                            rctx.signed,
                            None,
                        ));
                    }
                }
            }
            if let Some(info) = self
                .scope_sig_names
                .get(scope_path)
                .and_then(|m| m.get(&name))
            {
                return self.signal_read_expr(info);
            }
            // Some unqualified module-local enum uses can lack a resolved
            // target. Resolve those only against the current
            // instance's matching flat module definition and only when the
            // enumerator name is unique there.
            let def_name = match self.kind(self.inst) {
                NodeKind::ModuleInst { def_name, .. } => strip_lib(def_name),
                _ => String::new(),
            };
            let mut matches = self
                .db
                .flat_modules()
                .iter()
                .filter(|module| match self.kind(**module) {
                    NodeKind::ModuleInst {
                        def_name: candidate,
                        ..
                    } => strip_lib(candidate) == def_name,
                    _ => false,
                })
                .flat_map(|module| self.node(*module).children.iter())
                .filter_map(|candidate| match self.kind(*candidate) {
                    NodeKind::EnumConst { value } if self.node(*candidate).name == name => {
                        Some((value.as_ref(), self.node(*candidate).name.as_str()))
                    }
                    _ => None,
                });
            if let Some((value, enum_name)) = matches.next() {
                if matches.next().is_none() {
                    return enum_value_expr(value, enum_name);
                }
            }
        }
        Err(format!(
            "cannot resolve expression reference `{name}` in `{scope_path}`"
        ))
    }

    /// A plain constant node (`ExprKind::Constant`); used where the old code
    /// called `read_const` directly on a handle.
    pub(in super::super) fn const_of_node(&self, node: NodeId) -> Result<IrConst, String> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Constant {
                value,
                size,
                const_type,
                source,
                time_scale,
                ..
            }) => {
                let mut c = if let Some(fill) = self.source_fill_literal(node) {
                    IrConst {
                        bits: vec![(fill == 1) as u64],
                        x: vec![(fill == 2) as u64],
                        z: vec![(fill == 3) as u64],
                        width: 1,
                        signed: false,
                        real: None,
                        fill: Some(fill),
                    }
                } else {
                    read_const_from(value, *size)?
                };
                if *const_type == ConstantType::Time && self.round_time_literals {
                    let raw = c.real_value().ok_or_else(|| {
                        format!(
                            "time literal at {}:{}:{} has no real value",
                            self.node(node).file.as_deref().unwrap_or("<unknown>"),
                            self.node(node).line,
                            self.node(node).col
                        )
                    })?;
                    c = IrConst::real(self.rounded_time_literal(node, raw, source, *time_scale)?);
                }
                let (signed, literal_width) = self.signed_based_literal_info(node);
                if signed {
                    if let Some(width) = literal_width {
                        if width < c.width {
                            c = read_const_from(value, width as i32)?;
                        }
                    }
                    c.signed = true;
                }
                Ok(c)
            }
            _ => Err("unsupported constant value format".to_string()),
        }
    }

    /// Read the same bounded slice chain the lvalue path builds for packed
    /// index / part / bit / indexed-part selections beneath a fixed-array
    /// element. Returns `None` when the select is not such a composition, so
    /// the ordinary packed and array-select arms keep owning it.
    fn packed_element_read_ir(
        &mut self,
        scope_path: &str,
        node: NodeId,
    ) -> Result<Option<IrExpr>, String> {
        let Some(IrLhs::ArrayElem {
            arr,
            indices,
            elem_sel,
        }) = self.packed_element_lhs_ir(scope_path, node)?
        else {
            return Ok(None);
        };
        let width = match &elem_sel {
            IrElemSel::Part(left, right) => left
                .abs_diff(*right)
                .checked_add(1)
                .and_then(|width| u32::try_from(width).ok()),
            IrElemSel::Indexed { width, .. } => Some(*width),
            IrElemSel::PackedChain(steps) => steps.last().map(|step| step.width),
            _ => return Ok(None),
        }
        .ok_or_else(|| format!("packed element select width overflows in `{scope_path}`"))?;
        Ok(Some(IrExpr::new(
            IrExprKind::ArrayRead {
                arr,
                indices,
                elem_sel,
            },
            width,
            false,
            None,
        )))
    }

    pub(in super::super) fn indexed_part_select_width(
        &self,
        node: NodeId,
        scope_path: &str,
    ) -> Result<u32, String> {
        let value = self.eval_bound_i128(node).map_err(|_| {
            format!("indexed part-select width must be a constant in `{scope_path}`")
        })?;
        let width = u32::try_from(value)
            .map_err(|_| format!("indexed part-select width must be positive in `{scope_path}`"))?;
        if width == 0 {
            return Err(format!(
                "indexed part-select width must be positive in `{scope_path}`"
            ));
        }
        if width > LLG_MAX_WIDTH {
            return Err(format!(
                "indexed part-select width {width} exceeds maximum {LLG_MAX_WIDTH} in `{scope_path}`"
            ));
        }
        Ok(width)
    }
}

/// Read one packed step of a value: a constant part-select when the step
/// offset is constant, otherwise an indexed part-select from the LSB.
fn packed_step_read(value: IrExpr, step: crate::sim::ir::IrPackedSelect) -> IrExpr {
    let width = step.width;
    if let IrExprKind::Const(offset) = &step.base.kind {
        let known = !offset.signed
            && offset.real.is_none()
            && offset.x.iter().chain(&offset.z).all(|word| *word == 0)
            && offset.bits.iter().skip(1).all(|word| *word == 0);
        if let Some(right) = known
            .then(|| offset.bits.first().copied().unwrap_or(0))
            .and_then(|right| i64::try_from(right).ok())
        {
            return IrExpr::new(
                IrExprKind::PartSel {
                    base: Box::new(value),
                    left: right + i64::from(width) - 1,
                    right,
                },
                width,
                false,
                None,
            );
        }
    }
    IrExpr::new(
        IrExprKind::IdxPartSel {
            base: Box::new(value),
            base_idx: Box::new(step.base),
            width_expr: Box::new(lhs_integer_expr(i128::from(width))),
            neg: false,
        },
        width,
        false,
        None,
    )
}
