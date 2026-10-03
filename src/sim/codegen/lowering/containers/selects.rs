//! Packed selects of resizable-container elements.

use super::collection::Select;
use super::*;

/// Lowered element address: integral indices, or one string key.
#[derive(Clone)]
enum ElementKeys {
    Integral(Vec<IrExpr>),
    String(IrStringExpr),
}

/// A bit, part or indexed part-select rooted in one packed element of a
/// dynamic array, queue or associative array.
struct ContainerElementSelect {
    container: usize,
    /// Container indices that name the element, outermost first. A
    /// string-keyed associative array has exactly one string key.
    indices: Vec<NodeId>,
    /// Packed selects from the element outward. Each records the node whose
    /// packed type it selects, so intermediate bounds are retained.
    selects: Vec<(NodeId, Select)>,
    element: IrContainerElement,
}

impl Codegen<'_> {
    /// Number of container indices that reach a non-container element.
    pub(super) fn container_index_depth(&self, container: usize) -> usize {
        let mut depth = 1;
        let mut element = &self.model.containers[container].element;
        while let IrContainerElement::Container { element: next, .. } = element {
            depth += 1;
            element = next;
        }
        depth
    }

    /// Resolve a packed select of a packed container element, or `None` when
    /// `node` is not one (including a whole-element access).
    fn container_element_select(&self, node: NodeId) -> Option<ContainerElementSelect> {
        let mut current = node;
        let mut outer = Vec::new();
        let (container, indices) = loop {
            let element = self.container_element_path(current).or_else(|| {
                self.associative_string_element(current)
                    .map(|(container, key)| (container, vec![key]))
            });
            if let Some((container, indices)) = element {
                if outer.is_empty() {
                    return None;
                }
                break (container, indices);
            }
            match self.kind(current) {
                NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                    if let Some(container) = self.container_of(*base) {
                        // A select flattened across the element boundary:
                        // `d[i][b]` arrives as one select with indices beyond
                        // the container depth.
                        let depth = self.container_index_depth(container.ir);
                        if indices.len() <= depth {
                            return None;
                        }
                        outer.push((*base, Select::Elements(indices[depth..].to_vec())));
                        break (container.ir, indices[..depth].to_vec());
                    }
                    outer.push((*base, Select::Elements(indices.clone())));
                    current = *base;
                }
                NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                    outer.push((*base, Select::Elements(vec![*index])));
                    current = *base;
                }
                NodeKind::Expr(ExprKind::PartSelect { base, left, right }) => {
                    outer.push((*base, Select::Part(*left, *right)));
                    current = *base;
                }
                NodeKind::Expr(ExprKind::IndexedPartSelect {
                    base,
                    base_expr,
                    width_expr,
                    neg,
                }) => {
                    outer.push((*base, Select::Indexed(*base_expr, *width_expr, *neg)));
                    current = *base;
                }
                _ => return None,
            }
        };
        let element = self.container_element_type(container, indices.len())?;
        if !element.is_packed() {
            return None;
        }
        outer.reverse();
        Some(ContainerElementSelect {
            container,
            indices,
            selects: outer,
            element,
        })
    }

    fn is_string_keyed(&self, container: usize) -> bool {
        matches!(
            self.model.containers[container].kind,
            IrContainerKind::Associative {
                key: IrAssocKey::String
            }
        )
    }

    fn lower_element_keys(
        &mut self,
        path: &str,
        container: usize,
        indices: Vec<NodeId>,
    ) -> Result<ElementKeys, String> {
        if self.is_string_keyed(container) {
            let [key] = indices[..] else {
                return Err(format!(
                    "string-keyed associative element in `{path}` takes one key"
                ));
            };
            return Ok(ElementKeys::String(self.lower_string(path, key)?));
        }
        Ok(ElementKeys::Integral(
            self.lower_container_path_indices(path, container, indices)?,
        ))
    }

    fn container_element_read(
        container: usize,
        element: &IrContainerElement,
        keys: ElementKeys,
    ) -> IrExpr {
        let operation = match keys {
            ElementKeys::String(key) => IrContainerExpr::GetString { container, key },
            ElementKeys::Integral(indices) => match <[IrExpr; 1]>::try_from(indices) {
                Ok([index]) => IrContainerExpr::Get {
                    container,
                    index: Box::new(index),
                },
                Err(indices) => IrContainerExpr::GetNested { container, indices },
            },
        };
        IrExpr::new(
            IrExprKind::Container(Box::new(operation)),
            element.width(),
            element.signed(),
            None,
        )
    }

    fn container_select_steps(
        &mut self,
        path: &str,
        select: ContainerElementSelect,
    ) -> Result<Vec<crate::sim::ir::IrPackedSelect>, String> {
        let mut width = select.element.width();
        let mut steps = Vec::new();
        for (base, packed) in select.selects {
            self.packed_selection_steps(path, base, packed, &mut width, &mut steps)?;
        }
        Ok(steps)
    }

    /// Read a select flattened into one container select (`a[k][3]`). Selects
    /// written around a whole-element select are read by the generic packed
    /// select lowering over the element value.
    pub(super) fn lower_container_flattened_select(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrExpr>, String> {
        if !matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::ArraySelect { .. })
        ) {
            return Ok(None);
        }
        let Some(select) = self.container_element_select(node) else {
            return Ok(None);
        };
        let keys = self.lower_element_keys(path, select.container, select.indices.clone())?;
        let mut value = Self::container_element_read(select.container, &select.element, keys);
        for step in self.container_select_steps(path, select)? {
            value = super::collection::packed_formals::packed_step_read(value, step);
        }
        Ok(Some(value))
    }

    /// Lower `c[i][sel] = rhs` as one read/modify/write of the element.
    ///
    /// Container indices and the right-hand side are each evaluated once,
    /// before the element is read, so a function in the right-hand side that
    /// updates the same element is not overwritten by a stale copy. The
    /// write-back reuses the whole-element store: an invalid dynamic-array
    /// or queue index ignores the write, `q[$+1]` appends, and a missing
    /// associative key is created from the value a read returns (the array's
    /// default) before the selected bits are replaced.
    pub(super) fn lower_container_select_assignment(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
        blocking: bool,
        op: Operation,
    ) -> Result<Option<IrStmt>, String> {
        let Some(select) = self.container_element_select(lhs) else {
            return Ok(None);
        };
        if !blocking {
            return Err(format!(
                "nonblocking assignment to resizable container element in `{path}` is illegal"
            ));
        }
        if op != Operation::Assignment {
            return Err(format!(
                "compound assignment to resizable container element in `{path}` is not supported"
            ));
        }
        let container = select.container;
        let element = select.element.clone();
        let mut block = Vec::new();
        let keys = match self.lower_element_keys(path, container, select.indices.clone())? {
            ElementKeys::Integral(indices) => ElementKeys::Integral(
                indices
                    .into_iter()
                    .enumerate()
                    .map(|(position, index)| {
                        let name = format!("_csi{}_{position}", lhs.0);
                        let read = IrExpr::new(
                            IrExprKind::LocalRead(name.clone()),
                            index.width,
                            index.signed,
                            None,
                        );
                        block.push(IrStmt::DeclLocal {
                            name,
                            width: index.width,
                            signed: index.signed,
                            two_state: false,
                            init: Some(Box::new(index)),
                        });
                        read
                    })
                    .collect(),
            ),
            ElementKeys::String(key) => {
                let name = format!("_csk{}", lhs.0);
                block.push(IrStmt::DeclString {
                    name: name.clone(),
                    init: Some(key),
                });
                ElementKeys::String(IrStringExpr::LocalRead(name))
            }
        };
        let steps = self.container_select_steps(path, select)?;
        let element_name = format!("_cse{}", lhs.0);
        let target = IrLhs::PackedSelect {
            target: Box::new(IrLhs::WholeRef {
                addr: format!("&{element_name}"),
                width: element.width(),
                signed: element.signed(),
                two_state: false,
                shortreal: false,
            }),
            steps,
            signed: false,
            two_state: false,
        };
        let value = self.lower_expr(path, rhs)?;
        let value = apply_lhs_assignment_context(&self.model, &target, value);
        let selected_width = packed_lhs_width(&self.model, &target)
            .ok_or_else(|| format!("container element select in `{path}` has no width"))?;
        let value = ir_to_storage(value, selected_width, false, false)?;
        let value_name = format!("_csv{}", lhs.0);
        block.push(IrStmt::DeclLocal {
            name: value_name.clone(),
            width: selected_width,
            signed: false,
            two_state: false,
            init: Some(Box::new(value)),
        });
        let store_keys = keys.clone();
        block.push(IrStmt::DeclLocal {
            name: element_name.clone(),
            width: element.width(),
            signed: element.signed(),
            two_state: false,
            init: Some(Box::new(Self::container_element_read(
                container, &element, keys,
            ))),
        });
        block.push(IrStmt::Assign {
            lhs: target,
            rhs: IrExpr::new(
                IrExprKind::LocalRead(value_name),
                selected_width,
                false,
                None,
            ),
            nba: false,
        });
        let value = IrExpr::new(
            IrExprKind::LocalRead(element_name),
            element.width(),
            element.signed(),
            None,
        );
        block.push(IrStmt::Container(match store_keys {
            ElementKeys::String(key) => IrContainerStmt::SetString {
                container,
                key,
                value,
            },
            ElementKeys::Integral(indices) => match <[IrExpr; 1]>::try_from(indices) {
                Ok([index]) => IrContainerStmt::Set {
                    container,
                    index,
                    value,
                },
                Err(indices) => IrContainerStmt::SetNested {
                    container,
                    indices,
                    value,
                },
            },
        }));
        Ok(Some(IrStmt::Block(block)))
    }
}
