//! Fixed assignment patterns and casts capture each source value once.
use super::fixed_values::{fixed_path_descriptor, fixed_width, two_state};
use super::*;
use crate::sim::ir::IrPackedSelect;

impl Codegen<'_> {
    /// Apply unpacked leaf state domains after a bit-stream cast. The helper
    /// receives one evaluated payload, so splitting it never repeats effects.
    pub(in super::super) fn convert_fixed_payload(
        &mut self,
        node: NodeId,
        value: IrExpr,
    ) -> Result<IrExpr, String> {
        let Some(descriptor) = self.query_descriptor(node).cloned() else {
            return Ok(value);
        };
        if !matches!(
            &descriptor.shape,
            TypeShape::FixedArray { .. }
                | TypeShape::Aggregate(crate::core::db::AggregateLayout {
                    kind: AggregateKind::UnpackedStruct,
                    ..
                })
        ) {
            return Ok(value);
        }
        let width = fixed_width(&descriptor).ok_or("fixed conversion type has no width")?;
        fn convert(
            descriptor: &TypeDescriptor,
            source: &IrExpr,
            offset: u32,
        ) -> Result<IrExpr, String> {
            let width = fixed_width(descriptor).ok_or("fixed conversion member has no width")?;
            let mut values = Vec::new();
            match &descriptor.shape {
                TypeShape::Aggregate(layout) if layout.kind == AggregateKind::UnpackedStruct => {
                    let mut cursor = offset + width;
                    for member in &layout.members {
                        cursor -=
                            fixed_width(&member.descriptor).ok_or("fixed member has no width")?;
                        values.push(convert(&member.descriptor, source, cursor)?);
                    }
                }
                TypeShape::FixedArray { element, .. } => {
                    let stride = fixed_width(element).ok_or("fixed element has no width")?;
                    for index in (0..width / stride).rev() {
                        values.push(convert(element, source, offset + index * stride)?);
                    }
                }
                _ => {
                    let selected = super::packed_formals::packed_step_read(
                        source.clone(),
                        IrPackedSelect {
                            base: lhs_integer_expr(i128::from(offset)),
                            width,
                        },
                    );
                    return ir_to_storage(
                        selected,
                        width,
                        descriptor.info.signed,
                        two_state(descriptor),
                    );
                }
            }
            Codegen::join_bitstream_parts("fixed conversion", values)
        }
        let name = format!("_llg_fixed_cast_{}", descriptor.id.0);
        let index = if let Some(index) = self
            .model
            .funcs
            .iter()
            .position(|function| function.c_name == name)
        {
            index
        } else {
            let source = IrExpr::new(IrExprKind::FormalRead(0), width, false, None);
            let result = convert(&descriptor, &source, 0)?;
            let formal = crate::sim::ir::IrFormal::new(false, width, false)
                .map_err(|error| error.to_string())?;
            let index = self.model.funcs.len();
            self.model.funcs.push(crate::sim::ir::IrFunc::new(
                name,
                Some(IrType::Packed {
                    width,
                    signed: descriptor.info.signed,
                    two_state: two_state(&descriptor),
                }),
                vec![formal],
                Vec::new(),
                Vec::new(),
                vec![IrStmt::Return {
                    value: Some(Box::new(result)),
                }],
            ));
            index
        };
        Ok(IrExpr::new(
            IrExprKind::CallFn(Box::new(IrCallExpr::new(
                index,
                vec![IrCallArg::Val(IrExpr::resize_to(value, width, false))],
                parse_depth(&self.depth_arg),
                false,
            ))),
            width,
            descriptor.info.signed,
            None,
        ))
    }

    pub(in super::super) fn fixed_pattern_value(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrExpr>, String> {
        if !matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::AssignmentPattern,
                ..
            })
        ) {
            return Ok(None);
        }
        let Some(descriptor) = self.query_descriptor(node).cloned() else {
            return Ok(None);
        };
        if let TypeShape::Aggregate(layout) = &descriptor.shape {
            if matches!(
                layout.kind,
                AggregateKind::PackedStruct | AggregateKind::PackedUnion
            ) {
                return self
                    .lower_packed_aggregate_pattern_value(path, node, layout)
                    .map(Some);
            }
        }
        if fixed_width(&descriptor).is_none()
            || !matches!(
                &descriptor.shape,
                TypeShape::FixedArray { .. }
                    | TypeShape::Aggregate(crate::core::db::AggregateLayout {
                        kind: AggregateKind::UnpackedStruct | AggregateKind::UnpackedUnion,
                        ..
                    })
            )
        {
            return Ok(None);
        }
        let width = fixed_width(&descriptor).ok_or("fixed pattern has no width")?;
        let mut leaves = Vec::new();
        self.aggregate_descriptor_pattern_values(path, node, &descriptor, &[], &mut leaves)?;
        let mut source_widths: HashMap<NodeId, u32> = HashMap::new();
        for (member_path, source) in &leaves {
            let (member, _) = fixed_path_descriptor(&descriptor, member_path)
                .ok_or("fixed pattern path is invalid")?;
            let width = fixed_width(&member).ok_or("fixed pattern member has no width")?;
            source_widths
                .entry(*source)
                .and_modify(|current| *current = (*current).max(width))
                .or_insert(width);
        }
        let mut sources = HashMap::new();
        let mut formals = Vec::new();
        let mut arguments = Vec::new();
        let mut parts = Vec::new();
        for (member_path, source) in leaves {
            let (member, offset) = fixed_path_descriptor(&descriptor, &member_path)
                .ok_or("fixed pattern path is invalid")?;
            let member_width = fixed_width(&member).ok_or("fixed pattern member has no width")?;
            let index = if let Some(index) = sources.get(&source) {
                *index
            } else {
                let value = self.lower_expr(path, source)?;
                let value = if value.fill.is_some() {
                    let signed = value.signed;
                    ir_to_storage(value, source_widths[&source], signed, false)?
                } else {
                    value
                };
                let index = formals.len();
                formals.push(
                    crate::sim::ir::IrFormal::new(false, value.width, value.signed)
                        .map_err(|error| error.to_string())?,
                );
                arguments.push(IrCallArg::Val(value));
                sources.insert(source, index);
                index
            };
            let formal = &formals[index];
            let value = IrExpr::new(
                IrExprKind::FormalRead(index),
                formal.width,
                formal.signed,
                None,
            );
            parts.push((
                offset,
                ir_to_storage(value, member_width, member.info.signed, two_state(&member))?,
            ));
        }
        parts.sort_by_key(|part| std::cmp::Reverse(part.0));
        let mut cursor = width;
        let mut values = Vec::new();
        for (offset, value) in parts {
            let end = offset
                .checked_add(value.width)
                .ok_or("fixed pattern offset overflow")?;
            if end > cursor {
                return Err("overlapping fixed pattern members".into());
            }
            if end < cursor {
                values.push(IrExpr::new(
                    IrExprKind::Const(val_to_const(&elab::Value::from_bits(
                        vec![Bit::X; (cursor - end) as usize],
                        false,
                    ))?),
                    cursor - end,
                    false,
                    None,
                ));
            }
            values.push(value);
            cursor = offset;
        }
        if cursor != 0 {
            values.push(IrExpr::new(
                IrExprKind::Const(val_to_const(&elab::Value::from_bits(
                    vec![Bit::X; cursor as usize],
                    false,
                ))?),
                cursor,
                false,
                None,
            ));
        }
        let result = Self::join_bitstream_parts(path, values)?;
        let name = format!("_llg_fixed_pattern_{}_{}", self.inst.index(), node.index());
        let index = if let Some(index) = self
            .model
            .funcs
            .iter()
            .position(|function| function.c_name == name)
        {
            index
        } else {
            let index = self.model.funcs.len();
            self.model.funcs.push(crate::sim::ir::IrFunc::new(
                name,
                Some(IrType::Packed {
                    width,
                    signed: descriptor.info.signed,
                    two_state: two_state(&descriptor),
                }),
                formals,
                Vec::new(),
                Vec::new(),
                vec![IrStmt::Return {
                    value: Some(Box::new(result)),
                }],
            ));
            index
        };
        Ok(Some(IrExpr::new(
            IrExprKind::CallFn(Box::new(IrCallExpr::new(
                index,
                arguments,
                parse_depth(&self.depth_arg),
                false,
            ))),
            width,
            descriptor.info.signed,
            None,
        )))
    }
}
