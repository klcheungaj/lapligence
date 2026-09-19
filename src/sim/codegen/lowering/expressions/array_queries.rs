//! Array queries.

use super::*;

impl<'a> Codegen<'a> {
    pub(in super::super) fn query_descriptor(&self, node: NodeId) -> Option<&TypeDescriptor> {
        self.db
            .type_descriptor(node)
            .or_else(|| match self.kind(node) {
                NodeKind::Expr(ExprKind::Ref {
                    target: Some(target),
                }) => self.db.type_descriptor(*target),
                NodeKind::Expr(ExprKind::HierPath { refs, .. }) => refs
                    .iter()
                    .rev()
                    .flatten()
                    .find_map(|target| self.db.type_descriptor(*target)),
                _ => None,
            })
    }

    pub(super) fn query_dimensions_for(descriptor: &TypeDescriptor) -> Vec<IrArrayDimension> {
        match &descriptor.shape {
            TypeShape::PackedAtom { ranges } => {
                if ranges.is_empty() {
                    // IEEE 1800-2009 20.7 reports one dimension for every
                    // simple bit-vector type, including a 1-bit scalar.
                    descriptor
                        .info
                        .width
                        .map(|width| {
                            vec![IrArrayDimension {
                                left: Some(i128::from(width) - 1),
                                right: Some(0),
                            }]
                        })
                        .unwrap_or_default()
                } else {
                    ranges
                        .iter()
                        .map(|range| IrArrayDimension {
                            left: Some(range.left),
                            right: Some(range.right),
                        })
                        .collect()
                }
            }
            TypeShape::Aggregate(layout)
                if matches!(
                    layout.kind,
                    AggregateKind::PackedStruct | AggregateKind::PackedUnion
                ) =>
            {
                descriptor
                    .info
                    .width
                    .map(|width| {
                        vec![IrArrayDimension {
                            left: Some(i128::from(width) - 1),
                            right: Some(0),
                        }]
                    })
                    .unwrap_or_default()
            }
            TypeShape::FixedArray {
                dimensions,
                element,
            } => {
                let mut result = dimensions
                    .iter()
                    .map(|(left, right)| IrArrayDimension {
                        left: Some(i128::from(*left)),
                        right: Some(i128::from(*right)),
                    })
                    .collect::<Vec<_>>();
                result.extend(Self::query_dimensions_for(element));
                result
            }
            TypeShape::Container { element, .. } => {
                let mut result = vec![IrArrayDimension {
                    left: None,
                    right: None,
                }];
                result.extend(Self::query_dimensions_for(element));
                result
            }
            TypeShape::String => vec![IrArrayDimension {
                left: None,
                right: None,
            }],
            TypeShape::Real { .. } | TypeShape::Opaque { .. } | TypeShape::Aggregate(_) => {
                Vec::new()
            }
        }
    }

    pub(super) fn query_unpacked_dimensions_for(descriptor: &TypeDescriptor) -> u32 {
        match &descriptor.shape {
            TypeShape::FixedArray {
                dimensions,
                element,
            } => u32::try_from(dimensions.len())
                .unwrap_or(u32::MAX)
                .saturating_add(Self::query_unpacked_dimensions_for(element)),
            TypeShape::Container { element, .. } => {
                1_u32.saturating_add(Self::query_unpacked_dimensions_for(element))
            }
            _ => 0,
        }
    }

    fn query_target(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<(IrArrayQueryTarget, Vec<IrArrayDimension>), String> {
        let descriptor = self.query_descriptor(node).cloned().ok_or_else(|| {
            format!("array query argument has no owned type metadata in `{path}`")
        })?;
        let dimensions = Self::query_dimensions_for(&descriptor);
        if dimensions.is_empty() {
            return Err(format!(
                "array query argument in `{path}` has no queryable dimensions"
            ));
        }
        if let Some(container) = self.container_of(node) {
            return Ok((
                IrArrayQueryTarget::Container {
                    container: container.ir,
                    dimensions: dimensions.clone(),
                },
                dimensions,
            ));
        }
        if self
            .object_of(path, node)
            .is_some_and(|index| self.model.objects[index].ty == IrObjectType::String)
        {
            return Ok((
                IrArrayQueryTarget::String {
                    value: self.lower_string(path, node)?,
                    dimensions: dimensions.clone(),
                },
                dimensions,
            ));
        }
        Ok((
            IrArrayQueryTarget::Static {
                dimensions: dimensions.clone(),
            },
            dimensions,
        ))
    }

    pub(super) fn query_integer(value: i128) -> IrExpr {
        IrExpr::resize_to(lhs_integer_expr(value), 32, true)
    }

    fn static_array_query(kind: IrArrayQueryKind, dimension: IrArrayDimension) -> Option<i128> {
        let (Some(left), Some(right)) = (dimension.left, dimension.right) else {
            return None;
        };
        match kind {
            IrArrayQueryKind::Left => Some(left),
            IrArrayQueryKind::Right => Some(right),
            IrArrayQueryKind::Low => Some(left.min(right)),
            IrArrayQueryKind::High => Some(left.max(right)),
            IrArrayQueryKind::Increment => Some(if left >= right { 1 } else { -1 }),
            IrArrayQueryKind::Size => left
                .checked_sub(right)
                .and_then(|extent| extent.unsigned_abs().checked_add(1))
                .and_then(|size| i128::try_from(size).ok()),
        }
    }

    pub(super) fn lower_array_query(
        &mut self,
        path: &str,
        name: &str,
        args: &[NodeId],
    ) -> Result<IrExpr, String> {
        let [first, rest @ ..] = args else {
            return Err(format!("{name} requires one or two arguments in `{path}`"));
        };
        if rest.len() > 1 {
            return Err(format!("{name} requires one or two arguments in `{path}`"));
        }
        let (target, dimensions) = self.query_target(path, *first)?;
        let dimension_node = rest.first().copied();
        let known_dimension = dimension_node.and_then(|node| self.eval_bound_i128(node).ok());
        if let Some(index) = known_dimension {
            if index < 1
                || usize::try_from(index)
                    .ok()
                    .is_none_or(|index| index > dimensions.len())
            {
                return Err(format!(
                    "{name} dimension {index} is outside the queryable range in `{path}`"
                ));
            }
        }
        let nested_runtime_dimension = dimensions
            .iter()
            .skip(1)
            .any(|dimension| dimension.left.is_none() || dimension.right.is_none());
        if nested_runtime_dimension && dimension_node.is_some() && known_dimension != Some(1) {
            return Err(format!(
                "{name} cannot select a nested runtime dimension in `{path}`"
            ));
        }
        let selected = known_dimension
            .and_then(|index| usize::try_from(index - 1).ok())
            .and_then(|index| dimensions.get(index).copied());
        if let Some(selected) = selected {
            if let Some(value) = Self::static_array_query(
                match name {
                    "$left" => IrArrayQueryKind::Left,
                    "$right" => IrArrayQueryKind::Right,
                    "$low" => IrArrayQueryKind::Low,
                    "$high" => IrArrayQueryKind::High,
                    "$increment" => IrArrayQueryKind::Increment,
                    "$size" => IrArrayQueryKind::Size,
                    _ => unreachable!(),
                },
                selected,
            ) {
                if selected.left.is_some() {
                    return Ok(Self::query_integer(value));
                }
            }
        }
        let kind = match name {
            "$left" => IrArrayQueryKind::Left,
            "$right" => IrArrayQueryKind::Right,
            "$low" => IrArrayQueryKind::Low,
            "$high" => IrArrayQueryKind::High,
            "$increment" => IrArrayQueryKind::Increment,
            "$size" => IrArrayQueryKind::Size,
            _ => unreachable!(),
        };
        let dimension = dimension_node
            .map(|node| self.lower_expr(path, node).map(Box::new))
            .transpose()?;
        let query = IrArrayQuery {
            kind,
            target,
            dimension,
        };
        let (width, signed) = query.result_type(&self.model);
        Ok(IrExpr::new(
            IrExprKind::ObjectQuery(Box::new(IrObjectQuery::ArrayQuery(query))),
            width,
            signed,
            None,
        ))
    }
}
