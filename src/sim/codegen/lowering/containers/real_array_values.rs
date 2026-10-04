//! Declaration-order element values of real fixed-array expressions (SIM-005).
//!
//! Real arrays have no packed payload, so a real-array operand that is not
//! whole storage is lowered to one numeric expression per element. These
//! values are evaluated in order where they are used (call operands), so no
//! snapshot statements are needed; operands whose selectors would require a
//! snapshot are rejected rather than evaluated more than once.
use super::fixed_arrays::P30PatternSource;
use super::*;
use crate::sim::ir::{IrFixedArrayOrderMethod, IrRealArrayOrder};

fn fixed_values_cell_count(dims: &[(i32, i32)]) -> Result<u64, String> {
    dims.iter().try_fold(1u64, |count, (left, right)| {
        count
            .checked_mul(i64::from(*left).abs_diff(i64::from(*right)) + 1)
            .ok_or_else(|| "real-array element count overflows".to_owned())
    })
}

/// Largest real fixed array lowered to per-element values (equality,
/// conditionals and non-storage call operands). Whole-storage copies at call
/// boundaries are block copies and are not limited by this tunable.
pub(in super::super) const REAL_ARRAY_ELEMENTWISE_LIMIT: u64 = 4096;

impl Codegen<'_> {
    /// `==`/`!=` over real fixed arrays: numeric element equality in
    /// declaration order (SV 7.4.6, 11.4.5); never a bit-pattern compare.
    pub(in super::super) fn lower_real_array_equality(
        &mut self,
        path: &str,
        operation: Operation,
        operands: &[NodeId],
    ) -> Result<Option<IrExpr>, String> {
        if !matches!(
            operation,
            Operation::Equal | Operation::NotEqual | Operation::CaseEqual | Operation::CaseNotEqual
        ) || operands.len() != 2
        {
            return Ok(None);
        }
        let Some((dims, _)) = self.real_array_shape(operands[0]) else {
            return Ok(None);
        };
        if matches!(operation, Operation::CaseEqual | Operation::CaseNotEqual) {
            return Err(format!(
                "case equality on a real operand in `{path}` is illegal (SV 11.3.1, Table 11-1)"
            ));
        }
        let left = self.real_array_values(path, operands[0], &dims)?;
        let right = self.real_array_values(path, operands[1], &dims)?;
        if left.len() != right.len() {
            return Err(format!(
                "real-array equality operands disagree in size in `{path}`"
            ));
        }
        let equal = left
            .into_iter()
            .zip(right)
            .map(|(a, b)| {
                IrExpr::new(
                    IrExprKind::Bin {
                        op: IrBinOp::Eq,
                        a: Box::new(a),
                        b: Box::new(b),
                    },
                    1,
                    false,
                    None,
                )
            })
            .reduce(|all, next| {
                IrExpr::new(
                    IrExprKind::Bin {
                        op: IrBinOp::LogAnd,
                        a: Box::new(all),
                        b: Box::new(next),
                    },
                    1,
                    false,
                    None,
                )
            })
            .ok_or_else(|| format!("real-array equality has no elements in `{path}`"))?;
        Ok(Some(if operation == Operation::NotEqual {
            IrExpr::new(
                IrExprKind::Un {
                    op: IrUnOp::LogNot,
                    a: Box::new(equal),
                },
                1,
                false,
                None,
            )
        } else {
            equal
        }))
    }

    /// Declaration-order element values of a real fixed-array expression
    /// with target dimensions `dims`: storage views (whole arrays, rows,
    /// slices with constant selectors) and assignment patterns.
    pub(in super::super) fn real_array_values(
        &mut self,
        path: &str,
        node: NodeId,
        dims: &[(i32, i32)],
    ) -> Result<Vec<IrExpr>, String> {
        let count = fixed_values_cell_count(dims)?;
        if count > REAL_ARRAY_ELEMENTWISE_LIMIT {
            return Err(format!(
                "real-array operand in `{path}` has {count} elements; element-wise real-array \
                 expressions support at most {REAL_ARRAY_ELEMENTWISE_LIMIT}"
            ));
        }
        let mut captures = Vec::new();
        let mut captured = HashMap::new();
        if let Some(view) = self.p30_array_view(path, node, &mut captures, &mut captured)? {
            if !view.array.real {
                return Ok(Vec::new());
            }
            // Row selectors are captured as locals for snapshot-based
            // assignments. Here each value is read where it is used, so a
            // selector that is a constant or a plain variable read is
            // substituted directly; anything else could repeat side effects.
            let mut selectors = HashMap::new();
            for capture in captures {
                let IrStmt::DeclLocal {
                    name,
                    init: Some(init),
                    ..
                } = capture
                else {
                    return Err(format!(
                        "real-array operand in `{path}` has an unsupported row selector"
                    ));
                };
                if !matches!(
                    init.kind(),
                    IrExprKind::Const(_)
                        | IrExprKind::SigRead(_)
                        | IrExprKind::LocalRead(_)
                        | IrExprKind::FormalRead(_)
                ) {
                    return Err(format!(
                        "real-array operand in `{path}` needs a constant or variable row selector"
                    ));
                }
                selectors.insert(name, *init);
            }
            let arr = self.reference_array(view.array.ir);
            return Ok(view
                .coordinates
                .into_iter()
                .map(|indices| {
                    indices
                        .into_iter()
                        .map(|index| match index.kind() {
                            IrExprKind::LocalRead(name) => {
                                selectors.get(name).cloned().unwrap_or(index)
                            }
                            _ => index,
                        })
                        .collect::<Vec<_>>()
                })
                .map(|indices| {
                    IrExpr::new(
                        IrExprKind::ArrayRead {
                            arr,
                            indices,
                            elem_sel: IrElemSel::Whole,
                        },
                        0,
                        false,
                        None,
                    )
                })
                .collect());
        }
        let node = self.p30_unwrap_cast(node);
        if self.assignment_pattern_operands(path, node)?.is_none() {
            return Err(format!(
                "real-array operand in `{path}` must be a real array variable, row or assignment pattern"
            ));
        }
        let mut values = Vec::new();
        let mut items: HashMap<NodeId, Vec<IrExpr>> = HashMap::new();
        for source in self.p30_pattern_values(path, node, dims)? {
            match source {
                P30PatternSource::Leaf(leaf) => {
                    let value = self.lower_expr(path, leaf)?;
                    // Integral pattern items convert numerically (SV 6.12.2).
                    values.push(if value.is_real() {
                        value
                    } else {
                        IrExpr::new(
                            IrExprKind::CastToReal {
                                a: Box::new(value),
                                shortreal: false,
                            },
                            0,
                            false,
                            None,
                        )
                    });
                }
                P30PatternSource::Element { node, index, dims } => {
                    let item = match items.entry(node) {
                        std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
                        std::collections::hash_map::Entry::Vacant(entry) => {
                            entry.insert(self.real_array_values(path, node, &dims)?)
                        }
                    };
                    values.push(item.get(index).cloned().ok_or_else(|| {
                        format!("real-array pattern item is missing element {index} in `{path}`")
                    })?);
                }
            }
        }
        Ok(values)
    }

    /// Element values of a real-array assignment source that has no storage
    /// view: a real-array function result (called once into a lexical
    /// result array) or a conditional over real arrays. Statements that must
    /// run before the destination is written are appended to `captures`.
    pub(in super::super) fn p30_real_source_values(
        &mut self,
        path: &str,
        rhs: NodeId,
        dims: &[(i32, i32)],
        captures: &mut Vec<IrStmt>,
    ) -> Result<Option<Vec<IrExpr>>, String> {
        let rhs = self.p30_unwrap_cast(rhs);
        if let Some((temporary, call)) = self.real_array_result_call(path, rhs)? {
            captures.push(IrStmt::FixedArrayDeclare(temporary));
            captures.push(IrStmt::Call(Box::new(call)));
            let arr = self.reference_array(temporary);
            let dims = self.model.arrays[temporary].dims.clone();
            return Ok(Some(
                super::super::expressions::inside_array_index_vectors(&dims)
                    .into_iter()
                    .map(|indices| {
                        IrExpr::new(
                            IrExprKind::ArrayRead {
                                arr,
                                indices: indices
                                    .into_iter()
                                    .map(|index| lhs_integer_expr(i128::from(index)))
                                    .collect(),
                                elem_sel: IrElemSel::Whole,
                            },
                            0,
                            false,
                            None,
                        )
                    })
                    .collect(),
            ));
        }
        let operands = match self.kind(rhs) {
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Conditional,
                operands,
                ..
            }) if operands.len() == 3 => operands.clone(),
            _ => return Ok(None),
        };
        if self.real_array_shape(operands[1]).is_none() {
            return Ok(None);
        }
        // Each arm must be readable without side effects, because an
        // ambiguous predicate merges both arms element by element.
        let first = self.real_array_values(path, operands[1], dims)?;
        let second = self.real_array_values(path, operands[2], dims)?;
        if first.len() != second.len() {
            return Err(format!(
                "real-array conditional arms disagree in size in `{path}`"
            ));
        }
        let predicate = self.lower_expr(path, operands[0])?;
        let predicate = if predicate.is_real() {
            predicate
        } else {
            IrExpr::new(
                IrExprKind::Un {
                    op: IrUnOp::RedOr,
                    a: Box::new(predicate),
                },
                1,
                false,
                None,
            )
        };
        let selector = format!("_real_mux_sel_{}", rhs.0);
        captures.push(IrStmt::DeclLocal {
            name: selector.clone(),
            width: predicate.width,
            signed: false,
            init: Some(Box::new(predicate.clone())),
            two_state: false,
        });
        let selector = IrExpr::new(
            IrExprKind::LocalRead(selector),
            predicate.width,
            false,
            None,
        );
        // SV 11.4.11: a known predicate selects one arm; an ambiguous one keeps
        // numerically equal elements and yields the default 0.0 otherwise.
        let unknown = if selector.is_real() {
            None
        } else {
            Some(IrExpr::new(
                IrExprKind::SysFunc(Box::new(IrSysFunc::BitQuery {
                    kind: IrBitQuery::IsUnknown,
                    arg: Box::new(selector.clone()),
                })),
                1,
                false,
                None,
            ))
        };
        let zero = IrExpr::new(IrExprKind::Const(IrConst::real(0.0)), 0, false, None);
        let mut values = Vec::with_capacity(first.len());
        for (index, (a, b)) in first.into_iter().zip(second).enumerate() {
            let known = IrExpr::new(
                IrExprKind::Mux {
                    sel: Box::new(selector.clone()),
                    a: Box::new(a.clone()),
                    b: Box::new(b.clone()),
                },
                0,
                false,
                None,
            );
            let value = match &unknown {
                None => known,
                Some(unknown) => {
                    let equal = IrExpr::new(
                        IrExprKind::Bin {
                            op: IrBinOp::Eq,
                            a: Box::new(a.clone()),
                            b: Box::new(b),
                        },
                        1,
                        false,
                        None,
                    );
                    let merged = IrExpr::new(
                        IrExprKind::Mux {
                            sel: Box::new(equal),
                            a: Box::new(a),
                            b: Box::new(zero.clone()),
                        },
                        0,
                        false,
                        None,
                    );
                    IrExpr::new(
                        IrExprKind::Mux {
                            sel: Box::new(unknown.clone()),
                            a: Box::new(merged),
                            b: Box::new(known),
                        },
                        0,
                        false,
                        None,
                    )
                }
            };
            let name = format!("_real_mux_{}_{index}", rhs.0);
            captures.push(IrStmt::DeclLocal {
                name: name.clone(),
                width: 0,
                signed: false,
                init: Some(Box::new(value)),
                two_state: false,
            });
            values.push(IrExpr::new(IrExprKind::LocalRead(name), 0, false, None));
        }
        Ok(Some(values))
    }

    /// `reverse`, `sort` and `rsort` of stored real cells (a whole real
    /// array or a selected row) as one in-place numeric reorder.
    pub(in super::super) fn lower_real_array_order(
        &mut self,
        path: &str,
        call: NodeId,
        receiver: NodeId,
        method: &str,
    ) -> Result<Option<IrStmt>, String> {
        if self.real_array_shape(receiver).is_none() {
            return Ok(None);
        }
        let Some((_, cells)) = self.fixed_array_cells(path, receiver)? else {
            return Err(format!(
                "array method `{method}` in `{path}` requires stored real array cells"
            ));
        };
        if !self.model.arrays[cells.array].real {
            return Ok(None);
        }
        if self.db.method_call_has_with_clause(call) {
            return Err(format!(
                "array method `{method}` with a `with` clause over real elements in `{path}` is not supported"
            ));
        }
        let method = match method {
            "reverse" => IrFixedArrayOrderMethod::Reverse,
            "sort" => IrFixedArrayOrderMethod::Sort,
            _ => IrFixedArrayOrderMethod::RSort,
        };
        Ok(Some(IrStmt::RealArrayOrder(Box::new(IrRealArrayOrder {
            cells,
            method,
        }))))
    }
}
