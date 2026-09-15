//! Streaming.

use super::*;

impl<'a> Codegen<'a> {
    /// Lower the selector attached to a streaming operand.  The array base is
    /// retained by the owning streaming node; only selector bounds become
    /// runtime expressions, so each bound is evaluated once by the C call.
    pub(in super::super) fn lower_stream_selector(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<IrStreamSelector, String> {
        let lower_integral = |this: &mut Self, bound: NodeId| -> Result<IrExpr, String> {
            let value = this.lower_expr(path, bound)?;
            if value.is_real() {
                return Err(format!(
                    "streaming `with` selector bound must be integral in `{path}`"
                ));
            }
            Ok(value)
        };
        match self.kind(node) {
            NodeKind::Expr(ExprKind::BitSelect { index, .. }) => {
                Ok(IrStreamSelector::Index(lower_integral(self, *index)?))
            }
            NodeKind::Expr(ExprKind::PartSelect { left, right, .. }) => {
                Ok(IrStreamSelector::Range {
                    left: lower_integral(self, *left)?,
                    right: lower_integral(self, *right)?,
                })
            }
            NodeKind::Expr(ExprKind::IndexedPartSelect {
                base_expr,
                width_expr,
                neg,
                ..
            }) => Ok(IrStreamSelector::Indexed {
                base: lower_integral(self, *base_expr)?,
                width: lower_integral(self, *width_expr)?,
                negative: *neg,
            }),
            _ => Err(format!("unsupported streaming `with` selector in `{path}`")),
        }
    }

    /// Return constant logical indices selected by a `with` expression.  A
    /// `None` result means at least one bound is runtime-valued and must stay
    /// in the typed runtime selector path.
    pub(in super::super) fn static_stream_selector_indices(
        &self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<Vec<i128>>, String> {
        let range = |left: i128, right: i128| -> Result<Vec<i128>, String> {
            let distance = left.abs_diff(right);
            let count = distance
                .checked_add(1)
                .ok_or_else(|| format!("streaming selector range overflows in `{path}`"))?;
            if count > u128::from(LLG_MAX_WIDTH) {
                return Err(format!(
                    "streaming selector in `{path}` exceeds the runtime maximum width"
                ));
            }
            let count = usize::try_from(count)
                .map_err(|_| format!("streaming selector is too large in `{path}`"))?;
            let descending = left > right;
            (0..count)
                .map(|offset| {
                    let offset = i128::try_from(offset)
                        .map_err(|_| format!("streaming selector overflows in `{path}`"))?;
                    if descending {
                        left.checked_sub(offset)
                            .ok_or_else(|| format!("streaming selector overflows in `{path}`"))
                    } else {
                        left.checked_add(offset)
                            .ok_or_else(|| format!("streaming selector overflows in `{path}`"))
                    }
                })
                .collect()
        };
        let constant = |this: &Self, bound: NodeId| this.eval_bound_i128(bound).ok();
        match self.kind(node) {
            NodeKind::Expr(ExprKind::BitSelect { index, .. }) => {
                Ok(constant(self, *index).map(|index| vec![index]))
            }
            NodeKind::Expr(ExprKind::PartSelect { left, right, .. }) => {
                let (Some(left), Some(right)) = (constant(self, *left), constant(self, *right))
                else {
                    return Ok(None);
                };
                range(left, right).map(Some)
            }
            NodeKind::Expr(ExprKind::IndexedPartSelect {
                base_expr,
                width_expr,
                neg,
                ..
            }) => {
                let (Some(base), Some(width)) =
                    (constant(self, *base_expr), constant(self, *width_expr))
                else {
                    return Ok(None);
                };
                if width <= 0 {
                    return Err(format!(
                        "streaming indexed selector width must be positive in `{path}`"
                    ));
                }
                let end = if *neg {
                    base.checked_sub(width - 1)
                } else {
                    base.checked_add(width - 1)
                }
                .ok_or_else(|| format!("streaming selector overflows in `{path}`"))?;
                range(base, end).map(Some)
            }
            _ => Err(format!("unsupported streaming `with` selector in `{path}`")),
        }
    }

    fn fixed_stream_parts(
        &self,
        path: &str,
        array: &ArrayInfo,
        selected: Option<&[i128]>,
    ) -> Result<Vec<IrExpr>, String> {
        if array.dims.is_empty() {
            return Err(format!(
                "fixed streaming array has no dimensions in `{path}`"
            ));
        }
        if array.real {
            return Err(format!(
                "real array streaming operand is not supported in `{path}`"
            ));
        }
        let indices = selected.map(|indices| indices.to_vec()).unwrap_or_else(|| {
            let (left, right) = array.dims[0];
            let step = if left <= right { 1 } else { -1 };
            let mut values = Vec::new();
            let mut index = i128::from(left);
            loop {
                values.push(index);
                if index == i128::from(right) {
                    break;
                }
                index += i128::from(step);
            }
            values
        });
        let rest = inside_array_index_vectors(&array.dims[1..]);
        let rest_count = u128::try_from(rest.len())
            .map_err(|_| format!("fixed streaming array is too large in `{path}`"))?;
        let total = u128::try_from(indices.len())
            .ok()
            .and_then(|count| count.checked_mul(rest_count))
            .ok_or_else(|| format!("fixed streaming array is too large in `{path}`"))?;
        let width = total
            .checked_mul(u128::from(array.elem_width))
            .ok_or_else(|| format!("fixed streaming array width overflows in `{path}`"))?;
        if width > u128::from(LLG_MAX_WIDTH) {
            return Err(format!(
                "fixed streaming array in `{path}` exceeds the runtime maximum width"
            ));
        }
        let mut parts = Vec::with_capacity(usize::try_from(total).unwrap_or(0));
        for index in indices {
            let prefix = lhs_integer_expr(index);
            for suffix in &rest {
                let mut coordinates = Vec::with_capacity(1 + suffix.len());
                coordinates.push(prefix.clone());
                coordinates.extend(
                    suffix
                        .iter()
                        .map(|value| lhs_integer_expr(i128::from(*value))),
                );
                parts.push(IrExpr::new(
                    IrExprKind::ArrayRead {
                        arr: self.reference_array(array.ir),
                        indices: coordinates,
                        elem_sel: IrElemSel::Whole,
                    },
                    array.elem_width,
                    array.signed,
                    None,
                ));
            }
        }
        Ok(parts)
    }

    fn lower_stream_container_static(
        &self,
        path: &str,
        container: usize,
        indices: &[i128],
    ) -> Result<IrExpr, String> {
        let info = self
            .model
            .containers
            .get(container)
            .ok_or_else(|| format!("streaming container index is out of bounds in `{path}`"))?;
        let (width, signed, _) = info
            .element
            .packed()
            .ok_or_else(|| format!("streaming container requires a packed element in `{path}`"))?;
        let parts = indices
            .iter()
            .map(|index| {
                IrExpr::new(
                    IrExprKind::Container(Box::new(IrContainerExpr::Get {
                        container,
                        index: Box::new(lhs_integer_expr(*index)),
                    })),
                    width,
                    signed,
                    None,
                )
            })
            .collect::<Vec<_>>();
        Self::join_bitstream_parts(path, parts)
    }

    pub(in super::super) fn lower_stream_operand(
        &mut self,
        path: &str,
        value_node: NodeId,
        with_node: Option<NodeId>,
    ) -> Result<IrExpr, String> {
        if let Some(container) = self.container_of(value_node) {
            let element = self
                .model
                .containers
                .get(container.ir)
                .ok_or_else(|| format!("streaming container is out of bounds in `{path}`"))?;
            if !matches!(
                element.kind,
                IrContainerKind::Dynamic | IrContainerKind::Queue { .. }
            ) {
                return Err(format!(
                    "associative arrays are not legal streaming operands in `{path}`"
                ));
            }
            let (_, _, _) = element.element.packed().ok_or_else(|| {
                format!("streaming container operand must have a packed element in `{path}`")
            })?;
            if let Some(with_node) = with_node {
                if let Some(indices) = self.static_stream_selector_indices(path, with_node)? {
                    return self.lower_stream_container_static(path, container.ir, &indices);
                }
                let selector = self.lower_stream_selector(path, with_node)?;
                return Ok(IrExpr::new(
                    IrExprKind::Container(Box::new(IrContainerExpr::Stream {
                        container: container.ir,
                        slice: 1,
                        direction: IrStreamDirection::LeftToRight,
                        selector: Some(selector),
                    })),
                    LLG_MAX_WIDTH,
                    false,
                    None,
                ));
            }
            return Ok(IrExpr::new(
                IrExprKind::Container(Box::new(IrContainerExpr::Stream {
                    container: container.ir,
                    slice: 1,
                    direction: IrStreamDirection::LeftToRight,
                    selector: None,
                })),
                LLG_MAX_WIDTH,
                false,
                None,
            ));
        }
        if let Some(array) = self.array_of(value_node).cloned() {
            let selected = match with_node {
                Some(with_node) => Some(
                    self.static_stream_selector_indices(path, with_node)?
                        .ok_or_else(|| {
                            format!(
                                "runtime `with` selector on a fixed array is not supported in `{path}`"
                    )
                        })?,
                ),
                None => None,
            };
            let parts = self.fixed_stream_parts(path, &array, selected.as_deref())?;
            return Self::join_bitstream_parts(path, parts);
        }
        if with_node.is_some() {
            return Err(format!(
                "streaming `with` selector requires a packed-element array in `{path}`"
            ));
        }
        if let Some(value) = self.lower_bitstream_source(path, value_node)? {
            return Ok(value);
        }
        self.lower_expr(path, value_node)
    }
}
