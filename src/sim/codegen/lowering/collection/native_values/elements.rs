//! Record and fixed-array elements of descriptor-backed containers (SIM-006).
//!
//! A record element stays one runtime value inside its container. Members are
//! read and written in place through element-item accesses whose locator is
//! re-evaluated at every use; whole elements move between a container and any
//! record endpoint through a lexical native temporary of the element type, so
//! module records, native roots, patterns and calls share the SIM-003
//! transfers. Copies are deep except for handles (SV 7.5-7.10, 8.4).
use super::*;
use crate::sim::ir::IrValueSlot;

/// How one whole element of a container is selected.
#[derive(Clone)]
enum ElementSelector {
    /// Integral indices through nested containers, outermost first.
    Indices(Vec<NodeId>),
    /// Key of a string-indexed associative array.
    Key(NodeId),
}

/// One record or fixed-array element of container storage.
#[derive(Clone)]
struct ElementSelection {
    container: usize,
    selector: ElementSelector,
}

impl ElementSelection {
    fn depth(&self) -> usize {
        match &self.selector {
            ElementSelector::Indices(indices) => indices.len(),
            ElementSelector::Key(_) => 1,
        }
    }
}

fn record_element(element: &IrContainerElement) -> bool {
    matches!(
        element,
        IrContainerElement::Aggregate { .. } | IrContainerElement::FixedArray { .. }
    )
}

impl Codegen<'_> {
    /// `q[i]`, `q[i][j]` or `a["k"]` naming a whole record element.
    fn record_element_of(&self, node: NodeId) -> Option<ElementSelection> {
        let node = self.p30_unwrap_cast(node);
        if let Some((container, key)) = self.associative_string_element(node) {
            return record_element(&self.model.containers[container].element).then_some(
                ElementSelection {
                    container,
                    selector: ElementSelector::Key(key),
                },
            );
        }
        let (container, indices) = self.container_element_path(node)?;
        let element = self.container_element_type(container, indices.len())?;
        record_element(&element).then_some(ElementSelection {
            container,
            selector: ElementSelector::Indices(indices),
        })
    }

    /// Whether `node` names a whole record element of container storage or
    /// removes one from a queue of records.
    pub(in crate::sim::codegen) fn is_container_record(&self, node: NodeId) -> bool {
        self.record_element_of(node).is_some() || self.record_queue_pop(node).is_some()
    }

    /// A `pop_front()`/`pop_back()` of a queue of records.
    fn record_queue_pop(&self, node: NodeId) -> Option<(usize, bool)> {
        self.queue_pop_call(self.p30_unwrap_cast(node))
            .filter(|(container, _)| record_element(&self.model.containers[*container].element))
    }

    /// The record type descriptor of a container's element `depth` levels
    /// deep, from the container declaration.
    fn container_element_descriptor(
        &self,
        container: usize,
        depth: usize,
    ) -> Option<TypeDescriptor> {
        let mut typed = container;
        while let Some(like) = self.container_types_like.get(&typed) {
            typed = *like;
        }
        let declaration = self
            .container_globals
            .iter()
            .find(|(_, info)| info.ir == typed)
            .map(|(node, _)| *node)
            .or_else(|| {
                self.subroutine_containers
                    .iter()
                    .find(|(_, ir)| **ir == typed)
                    .map(|((_, node), _)| *node)
            })?;
        let mut descriptor = self.db.type_descriptor(declaration)?.clone();
        for level in 0..depth {
            descriptor = match descriptor.shape {
                TypeShape::Container { element, .. } => *element,
                // A one-dimensional fixed array of native elements is a
                // container view of its declaration (SIM-007).
                TypeShape::FixedArray { element, .. } if level == 0 => *element,
                _ => return None,
            };
        }
        Some(descriptor)
    }

    /// Element shape, type descriptor and declaration-order leaves of a
    /// container's record element.
    fn element_leaves(
        &self,
        container: usize,
        depth: usize,
    ) -> Result<(IrContainerElement, TypeDescriptor, Vec<NativeLeaf>), String> {
        let name = &self.model.containers[container].c_name;
        let element = self
            .container_element_type(container, depth)
            .ok_or_else(|| format!("container `{name}` has no element at depth {depth}"))?;
        let descriptor = self
            .container_element_descriptor(container, depth)
            .ok_or_else(|| format!("container `{name}` element has no type descriptor"))?;
        validate_native_type(&element, "container element").map_err(|error| {
            format!(
                "record elements of container `{}` are not supported: {error}",
                descriptor.name
            )
        })?;
        let mut leaves = Vec::new();
        collect_native_leaves(
            &descriptor,
            &element,
            &mut Vec::new(),
            &mut Vec::new(),
            &mut leaves,
        )
        .map_err(|error| format!("{error} (container element `{}`)", descriptor.name))?;
        Ok((element, descriptor, leaves))
    }

    /// A lexical native value of a container's element type for one
    /// transfer at `site`. The caller declares it with `NativeValueDeclare`.
    fn element_temporary(
        &mut self,
        container: usize,
        depth: usize,
        site: NodeId,
    ) -> Result<usize, String> {
        let (element, descriptor, leaves) = self.element_leaves(container, depth)?;
        let ty = match self
            .model
            .native_types
            .iter()
            .position(|existing| *existing == element)
        {
            Some(ty) => ty,
            None => {
                self.model.native_types.push(element);
                self.model.native_types.len() - 1
            }
        };
        // `site` is an expression, never a declaration that native storage
        // collection could mistake for a record variable.
        self.native_layouts.insert(
            site,
            NativeLayout {
                ty,
                descriptor,
                leaves,
            },
        );
        let index = self.model.native_values.len();
        self.model.native_values.push(IrNativeValue {
            c_name: format!("S_llg_native_{index}"),
            ty,
            activation: true,
        });
        self.native_value_layouts.insert(index, site);
        Ok(index)
    }

    fn lower_element_slot(
        &mut self,
        path: &str,
        selection: &ElementSelection,
    ) -> Result<IrValueSlot, String> {
        Ok(match &selection.selector {
            ElementSelector::Key(key) => IrValueSlot::Element {
                indices: Vec::new(),
                key: Some(self.lower_string(path, *key)?),
            },
            ElementSelector::Indices(indices) => IrValueSlot::Element {
                indices: self.lower_container_path_indices(
                    path,
                    selection.container,
                    indices.clone(),
                )?,
                key: None,
            },
        })
    }

    /// Store any record source into a container slot: the source fills a
    /// lexical temporary of the element type, which is then copied in.
    fn record_into_slot(
        &mut self,
        path: &str,
        container: usize,
        depth: usize,
        slot: IrValueSlot,
        site: NodeId,
        rhs: NodeId,
    ) -> Result<IrStmt, String> {
        let temporary = self.element_temporary(container, depth, site)?;
        let descriptor = self.native_layout_of_value(temporary)?.descriptor.clone();
        let endpoint = NativeEndpoint::Value {
            value: temporary,
            prefix: Vec::new(),
        };
        let fill = self.native_assign_into(path, &endpoint, &descriptor, rhs, false)?;
        Ok(IrStmt::Block(vec![
            IrStmt::NativeValueDeclare(temporary),
            fill,
            IrStmt::Container(Box::new(IrContainerStmt::SetValue {
                container,
                slot,
                value: temporary,
            })),
        ]))
    }

    /// Replace a container of record elements with `values` in order (an
    /// assignment pattern or unpacked concatenation). Every value is built
    /// into a fresh container first, so sources that read the destination
    /// see its old elements and the copy is independent (SV 10.9, 10.10).
    pub(in crate::sim::codegen) fn lower_container_record_values(
        &mut self,
        path: &str,
        container: usize,
        values: Vec<NodeId>,
    ) -> Result<IrStmt, String> {
        let count = u64::try_from(values.len()).map_err(|_| "pattern is too large")?;
        let temporary = self.container_temporary_like(container);
        let mut statements = vec![IrStmt::Container(Box::new(IrContainerStmt::Declare(
            temporary,
        )))];
        let queue = matches!(
            self.model.containers[container].kind,
            IrContainerKind::Queue { .. }
        );
        if !queue && self.model.containers[temporary].initial_size != Some(count) {
            statements.push(IrStmt::Container(Box::new(IrContainerStmt::DynamicNew {
                container: temporary,
                size: crate::sim::codegen::lowering::containers::pattern_key_expr(
                    i128::from(count),
                    64,
                    false,
                    true,
                ),
                initializer: None,
            })));
        }
        for (position, value) in values.into_iter().enumerate() {
            let slot = if queue {
                IrValueSlot::PushBack
            } else {
                IrValueSlot::Element {
                    indices: vec![crate::sim::codegen::lowering::containers::pattern_key_expr(
                        position as i128,
                        64,
                        false,
                        true,
                    )],
                    key: None,
                }
            };
            statements.push(self.record_into_slot(path, temporary, 1, slot, value, value)?);
        }
        statements.push(IrStmt::Container(Box::new(IrContainerStmt::Copy {
            dst: container,
            src: temporary,
        })));
        Ok(IrStmt::Block(statements))
    }

    /// Copy-out of a record output/inout formal into a container element
    /// actual (`f(q[i])`): the element's indices are evaluated before the
    /// call, and the value is stored after it returns.
    pub(in crate::sim::codegen) fn container_record_writeback(
        &mut self,
        path: &str,
        actual: NodeId,
        value: usize,
        before: &mut Vec<IrStmt>,
    ) -> Result<Option<IrStmt>, String> {
        let Some(selection) = self.record_element_of(actual) else {
            return Ok(None);
        };
        let slot = match self.lower_element_slot(path, &selection)? {
            IrValueSlot::Element { indices, key } => {
                let indices = indices
                    .into_iter()
                    .enumerate()
                    .map(|(position, index)| {
                        let name = format!("_llg_out_index_{}_{position}", actual.index());
                        let (width, signed) = (index.width, index.signed);
                        before.push(IrStmt::DeclLocal {
                            name: name.clone(),
                            width,
                            signed,
                            two_state: false,
                            init: Some(Box::new(index)),
                        });
                        IrExpr::new(IrExprKind::LocalRead(name), width, signed, None)
                    })
                    .collect();
                IrValueSlot::Element { indices, key }
            }
            slot => slot,
        };
        Ok(Some(IrStmt::Container(Box::new(
            IrContainerStmt::SetValue {
                container: selection.container,
                slot,
                value,
            },
        ))))
    }

    /// `q[i] = rhs` for a record element (SV 7.5-7.10 value semantics).
    pub(in crate::sim::codegen) fn lower_container_record_assignment(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
    ) -> Result<Option<IrStmt>, String> {
        let Some(selection) = self.record_element_of(lhs) else {
            return Ok(None);
        };
        let slot = self.lower_element_slot(path, &selection)?;
        self.record_into_slot(path, selection.container, selection.depth(), slot, lhs, rhs)
            .map(Some)
    }

    /// `push_front`, `push_back` or `insert` of a record into a queue.
    pub(in crate::sim::codegen) fn lower_container_record_push(
        &mut self,
        path: &str,
        container: usize,
        slot: IrValueSlot,
        value: NodeId,
    ) -> Result<IrStmt, String> {
        self.record_into_slot(path, container, 1, slot, value, value)
    }

    /// Copy a whole record element, or the record removed by a pop, into
    /// `target` (a native root or module record); `None` for other sources.
    pub(super) fn container_record_into(
        &mut self,
        path: &str,
        target: &NativeEndpoint,
        source: NodeId,
    ) -> Result<Option<IrStmt>, String> {
        let (container, depth, slot) = if let Some(selection) = self.record_element_of(source) {
            let slot = self.lower_element_slot(path, &selection)?;
            (selection.container, selection.depth(), slot)
        } else if let Some((container, back)) = self.record_queue_pop(source) {
            let slot = if back {
                IrValueSlot::PopBack
            } else {
                IrValueSlot::PopFront
            };
            (container, 1, slot)
        } else {
            return Ok(None);
        };
        let temporary = self.element_temporary(container, depth, source)?;
        let endpoint = NativeEndpoint::Value {
            value: temporary,
            prefix: Vec::new(),
        };
        let transfer = self.native_transfer(path, target, &endpoint, false)?;
        Ok(Some(IrStmt::Block(vec![
            IrStmt::NativeValueDeclare(temporary),
            IrStmt::Container(Box::new(IrContainerStmt::GetValue {
                container,
                slot,
                value: temporary,
            })),
            transfer,
        ])))
    }

    /// The member/index path from a record element to `node`, for constant
    /// member and member-array selections (`q[i].m`, `q[i].a[2].s`).
    fn element_member_path(
        &self,
        node: NodeId,
    ) -> Option<(ElementSelection, Vec<AggregatePathPart>)> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::HierPath { parts, refs }) => {
                let first = (*refs.first()?)?;
                let selection = self.record_element_of(first)?;
                let path = parts
                    .iter()
                    .skip(1)
                    .filter(|part| !part.is_empty())
                    .map(|part| AggregatePathPart::Member(part.clone()))
                    .collect::<Vec<_>>();
                (!path.is_empty()).then_some((selection, path))
            }
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                // A member array is named by a detached array node; the
                // captured select path gives its element owner.
                let (selection, mut path) = match self.db.array_select_path(node) {
                    Some((owner, members)) => (
                        self.record_element_of(owner)?,
                        members
                            .iter()
                            .cloned()
                            .map(AggregatePathPart::Member)
                            .collect(),
                    ),
                    None => self.element_member_path(*base)?,
                };
                for index in indices {
                    path.push(AggregatePathPart::Index(
                        i32::try_from(self.eval_bound_i128(*index).ok()?).ok()?,
                    ));
                }
                Some((selection, path))
            }
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                let (selection, mut path) = self.element_member_path(*base)?;
                path.push(AggregatePathPart::Index(
                    i32::try_from(self.eval_bound_i128(*index).ok()?).ok()?,
                ));
                Some((selection, path))
            }
            _ => None,
        }
    }

    /// The record element leaf named by `node`; `None` for a sub-record, a
    /// selection inside a packed leaf, or anything that is not an element
    /// member.
    fn element_leaf(&self, node: NodeId) -> Result<Option<(ElementSelection, NativeLeaf)>, String> {
        let Some((selection, path)) = self.element_member_path(node) else {
            return Ok(None);
        };
        let (_, _, leaves) = self.element_leaves(selection.container, selection.depth())?;
        if let Some(leaf) = leaves.iter().find(|leaf| leaf.path == path) {
            return Ok(Some((selection, leaf.clone())));
        }
        if leaves.iter().any(|leaf| {
            leaf.path.starts_with(&path)
                || (path.starts_with(&leaf.path)
                    && matches!(leaf.ty, IrClassFieldType::Packed { .. }))
        }) {
            return Ok(None);
        }
        Err(format!(
            "container element selection `{}` does not name a member",
            aggregate_path_suffix(&path)
        ))
    }

    /// Kind of the element leaf named by `node`, for classification.
    pub(in crate::sim::codegen) fn element_leaf_kind(
        &self,
        node: NodeId,
    ) -> Option<IrClassFieldType> {
        self.element_leaf(node)
            .ok()
            .flatten()
            .map(|(_, leaf)| leaf.ty)
    }

    /// A fresh access to the element leaf named by `node`. A `write` access
    /// creates a missing associative entry and publishes the store.
    pub(in crate::sim::codegen) fn element_leaf_symbol(
        &mut self,
        path: &str,
        node: NodeId,
        write: bool,
    ) -> Result<Option<(String, IrClassFieldType)>, String> {
        let Some((selection, leaf)) = self.element_leaf(node)? else {
            return Ok(None);
        };
        let IrValueSlot::Element { indices, key } = self.lower_element_slot(path, &selection)?
        else {
            unreachable!("element selections lower to element slots");
        };
        let name = format!("_llg_access_{}", self.model.native_accesses.len());
        self.model
            .native_accesses
            .push(crate::sim::ir::IrNativeAccess {
                name: name.clone(),
                receiver: IrChandleExpr::ContainerElement {
                    container: selection.container,
                    indices,
                    key: key.map(Box::new),
                    write,
                },
                kind: IrNativeAccessKind::ElementItem { ty: leaf.ty },
                site: None,
                item_path: leaf.items,
                function: self.cur_fn_ir,
            });
        Ok(Some((name, leaf.ty)))
    }

    /// Typed read of the element leaf named by `node`.
    pub(in crate::sim::codegen) fn element_leaf_read(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrExpr>, String> {
        let Some((name, ty)) = self.element_leaf_symbol(path, node, false)? else {
            return Ok(None);
        };
        Ok(match ty {
            IrClassFieldType::Packed { width, signed, .. } => Some(IrExpr::new(
                IrExprKind::LocalRead(name),
                width,
                signed,
                None,
            )),
            IrClassFieldType::Real { .. } => {
                Some(IrExpr::new(IrExprKind::LocalRead(name), 0, false, None))
            }
            _ => None,
        })
    }

    /// Packed or real target of the element leaf named by `node`.
    pub(in crate::sim::codegen) fn element_leaf_target(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrLhs>, String> {
        let Some((name, ty)) = self.element_leaf_symbol(path, node, true)? else {
            return Ok(None);
        };
        let (width, signed, two_state, shortreal) = match ty {
            IrClassFieldType::Packed {
                width,
                signed,
                two_state,
            } => (width, signed, two_state, false),
            IrClassFieldType::Real { shortreal } => (0, false, false, shortreal),
            _ => return Ok(None),
        };
        Ok(Some(IrLhs::WholeRef {
            addr: format!("&{name}"),
            width,
            signed,
            two_state,
            shortreal,
        }))
    }
}
