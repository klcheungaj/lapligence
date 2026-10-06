//! Record and fixed-array elements of descriptor-backed containers (SIM-006).
//!
//! A record element stays one runtime value inside its container. Members are
//! read and written in place through element-item accesses whose locator is
//! re-evaluated at every use; whole elements move between a container and any
//! record endpoint through a lexical native temporary of the element type, so
//! module records, native roots, patterns and calls share the SIM-003
//! transfers. Copies are deep except for handles (SV 7.5-7.10, 8.4).
use super::*;
use crate::sim::ir::{IrValueItemRoot, IrValueSlot};

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
pub(super) struct ElementSelection {
    pub(super) container: usize,
    selector: ElementSelector,
}

impl ElementSelection {
    pub(super) fn depth(&self) -> usize {
        match &self.selector {
            ElementSelector::Indices(indices) => indices.len(),
            ElementSelector::Key(_) => 1,
        }
    }
}

/// Queue and dynamic-array methods that change their receiver
/// (SV 7.5.1, 7.10.2, 7.12.2).
const MUTATING_CONTAINER_METHODS: &[&str] = &[
    "push_front",
    "push_back",
    "pop_front",
    "pop_back",
    "insert",
    "delete",
    "sort",
    "rsort",
    "reverse",
    "shuffle",
];

/// One queue or dynamic-array member of a container record element that a
/// statement names, and whether the statement can change it.
struct StagedMember {
    element: NodeId,
    path: Vec<AggregatePathPart>,
    mutated: bool,
}

fn unstaged_member_error(path: &[AggregatePathPart]) -> String {
    format!(
        "queue or dynamic-array member `{}` of a container record element is only supported in assignment, call and system-task statements and whole-element copies",
        aggregate_path_suffix(path)
    )
}

/// The value of a fresh element-leaf access named `name`.
fn element_leaf_value(name: String, ty: IrClassFieldType) -> LeafValue {
    match ty {
        IrClassFieldType::String => LeafValue::String(IrStringExpr::LocalRead(name)),
        IrClassFieldType::Chandle => LeafValue::Chandle(IrChandleExpr::LocalRead(name)),
        IrClassFieldType::Packed { width, signed, .. } => LeafValue::Packed(IrExpr::new(
            IrExprKind::LocalRead(name),
            width,
            signed,
            None,
        )),
        IrClassFieldType::Real { .. } => {
            LeafValue::Real(IrExpr::new(IrExprKind::LocalRead(name), 0, false, None))
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
    pub(super) fn record_element_of(&self, node: NodeId) -> Option<ElementSelection> {
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
    pub(super) fn container_element_descriptor(
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
    /// container's record element; container members are listed apart.
    fn element_leaves(
        &self,
        container: usize,
        depth: usize,
    ) -> Result<(IrContainerElement, TypeDescriptor, NativeLeaves), String> {
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
        let mut leaves = NativeLeaves::default();
        collect_native_root_leaves(&descriptor, &element, &mut leaves)
            .map_err(|error| format!("{error} (container element `{}`)", descriptor.name))?;
        // A container element owns a queue or dynamic-array member as a
        // nested dynamic array inside its value; the runtime has no nested
        // associative form.
        if let Some(leaf) = leaves
            .containers
            .iter()
            .find(|leaf| matches!(leaf.kind, IrContainerKind::Associative { .. }))
        {
            return Err(format!(
                "record elements of a queue, dynamic, associative or fixed array with associative array member `{}` are not supported: the element value has no nested associative form",
                aggregate_path_suffix(&leaf.path)
            ));
        }
        Ok((element, descriptor, leaves))
    }

    /// Scalar leaves of a container's record element.
    fn element_scalar_leaves(
        &self,
        container: usize,
        depth: usize,
    ) -> Result<Vec<NativeLeaf>, String> {
        Ok(self.element_leaves(container, depth)?.2.scalars)
    }

    /// Move every queue or dynamic-array member of `value`, just read from
    /// container storage, from its nested slot into its companion.
    fn element_items_to_companions(&self, value: usize) -> Result<Vec<IrStmt>, String> {
        let layout = self.native_layout_of_value(value)?;
        Ok(layout
            .containers
            .iter()
            .zip(&self.model.native_values[value].companions)
            .map(|(leaf, companion)| {
                IrStmt::Container(Box::new(IrContainerStmt::ValueItemToContainer {
                    root: IrValueItemRoot::Value(value),
                    items: leaf.items.clone(),
                    container: *companion,
                }))
            })
            .collect())
    }

    /// Copy every companion container of `value` into its nested slot
    /// before the value is stored into container storage.
    fn companions_to_element_items(&self, value: usize) -> Result<Vec<IrStmt>, String> {
        let layout = self.native_layout_of_value(value)?;
        Ok(layout
            .containers
            .iter()
            .zip(&self.model.native_values[value].companions)
            .map(|(leaf, companion)| {
                IrStmt::Container(Box::new(IrContainerStmt::ContainerToValueItem {
                    container: *companion,
                    root: IrValueItemRoot::Value(value),
                    items: leaf.items.clone(),
                }))
            })
            .collect())
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
        // Queue and dynamic-array members travel in companion containers,
        // like a native record's (`element_items_to_companions`).
        let layout = NativeLayout {
            ty,
            descriptor,
            leaves: leaves.scalars,
            containers: leaves.containers,
        };
        let companions = self.native_companions(&layout, true);
        self.native_layouts.insert(site, layout);
        let index = self.model.native_values.len();
        self.model.native_values.push(IrNativeValue {
            c_name: format!("S_llg_native_{index}"),
            ty,
            activation: true,
            companions,
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
        let mut statements = vec![IrStmt::NativeValueDeclare(temporary), fill];
        statements.extend(self.companions_to_element_items(temporary)?);
        statements.push(IrStmt::Container(Box::new(IrContainerStmt::SetValue {
            container,
            slot,
            value: temporary,
        })));
        Ok(IrStmt::Block(statements))
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
        let mut statements = self.companions_to_element_items(value)?;
        statements.push(IrStmt::Container(Box::new(IrContainerStmt::SetValue {
            container: selection.container,
            slot,
            value,
        })));
        Ok(Some(IrStmt::Block(statements)))
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
        nba: bool,
    ) -> Result<Option<IrStmt>, String> {
        let Some((temporary, mut statements)) = self.element_copy(path, source)? else {
            return Ok(None);
        };
        let endpoint = NativeEndpoint::Value {
            value: temporary,
            prefix: Vec::new(),
        };
        statements.push(self.native_transfer(path, target, &endpoint, nba)?);
        Ok(Some(IrStmt::Block(statements)))
    }

    /// Statements that copy a whole record element, or the record removed by
    /// a pop, into a fresh lexical temporary of the element type, with its
    /// queue and dynamic-array members in the temporary's companions;
    /// `None` for other sources.
    fn element_copy(
        &mut self,
        path: &str,
        source: NodeId,
    ) -> Result<Option<(usize, Vec<IrStmt>)>, String> {
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
        let mut statements = vec![
            IrStmt::NativeValueDeclare(temporary),
            IrStmt::Container(Box::new(IrContainerStmt::GetValue {
                container,
                slot,
                value: temporary,
            })),
        ];
        statements.extend(self.element_items_to_companions(temporary)?);
        Ok(Some((temporary, statements)))
    }

    /// The member/index path from a record element to `node`, for constant
    /// member and member-array selections (`q[i].m`, `q[i].a[2].s`).
    fn element_member_path(
        &self,
        node: NodeId,
    ) -> Option<(ElementSelection, Vec<AggregatePathPart>)> {
        self.element_member_parts(node)
            .map(|(_, selection, path)| (selection, path))
    }

    /// `element_member_path` with the node naming the whole element.
    pub(super) fn element_member_parts(
        &self,
        node: NodeId,
    ) -> Option<(NodeId, ElementSelection, Vec<AggregatePathPart>)> {
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
                (!path.is_empty()).then_some((first, selection, path))
            }
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                // A member array is named by a detached array node; the
                // captured select path gives its element owner.
                let (element, selection, mut path) = match self.db.array_select_path(node) {
                    Some((owner, members)) => (
                        owner,
                        self.record_element_of(owner)?,
                        members
                            .iter()
                            .cloned()
                            .map(AggregatePathPart::Member)
                            .collect(),
                    ),
                    None => self.element_member_parts(*base)?,
                };
                for index in indices {
                    path.push(AggregatePathPart::Index(
                        i32::try_from(self.eval_bound_i128(*index).ok()?).ok()?,
                    ));
                }
                Some((element, selection, path))
            }
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                let (element, selection, mut path) = self.element_member_parts(*base)?;
                path.push(AggregatePathPart::Index(
                    i32::try_from(self.eval_bound_i128(*index).ok()?).ok()?,
                ));
                Some((element, selection, path))
            }
            _ => None,
        }
    }

    /// A fresh read of the leaf at `path` of the whole record element
    /// `element`; `None` when the element has no such leaf.
    pub(super) fn element_path_read(
        &mut self,
        path: &str,
        element: NodeId,
        leaf_path: &[AggregatePathPart],
    ) -> Result<Option<LeafValue>, String> {
        let Some(selection) = self.record_element_of(element) else {
            return Ok(None);
        };
        let leaves = self.element_scalar_leaves(selection.container, selection.depth())?;
        let Some(leaf) = leaves.into_iter().find(|leaf| leaf.path == leaf_path) else {
            return Ok(None);
        };
        let name = self.element_access(path, &selection, &leaf, false)?;
        Ok(Some(element_leaf_value(name, leaf.ty)))
    }

    /// The record element leaf named by `node`; `None` for a sub-record, a
    /// selection inside a packed leaf, or anything that is not an element
    /// member.
    fn element_leaf(&self, node: NodeId) -> Result<Option<(ElementSelection, NativeLeaf)>, String> {
        let Some((selection, path)) = self.element_member_path(node) else {
            return Ok(None);
        };
        let (_, _, leaves) = self.element_leaves(selection.container, selection.depth())?;
        if let Some(leaf) = leaves
            .containers
            .iter()
            .find(|leaf| path.starts_with(&leaf.path))
        {
            // A staged member is an ordinary container for the statement.
            let staged = self
                .element_member_parts(node)
                .is_some_and(|(element, _, _)| {
                    self.staged_element_members
                        .contains_key(&(element, aggregate_path_suffix(&leaf.path)))
                });
            if staged {
                return Ok(None);
            }
            return Err(unstaged_member_error(&leaf.path));
        }
        let leaves = leaves.scalars;
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
        let name = self.element_access(path, &selection, &leaf, write)?;
        Ok(Some((name, leaf.ty)))
    }

    /// Every leaf of the whole record element named by `node`, read in
    /// declaration order with paths relative to the element; `None` when
    /// `node` is not a whole record element of container storage. Each read
    /// re-evaluates the element locator, so it must be free of side effects.
    pub(super) fn container_record_leaf_reads(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<LeafReads>, String> {
        let Some(selection) = self.record_element_of(node) else {
            return Ok(None);
        };
        if !self.side_effect_free(node) {
            return Err(format!(
                "a container record element compared in `{path}` must be selected without side effects"
            ));
        }
        let (_, _, leaves) = self.element_leaves(selection.container, selection.depth())?;
        if let Some(leaf) = leaves.containers.first() {
            return Err(format!(
                "a container record element with queue or dynamic-array member `{}` cannot be read member by member in `{path}`",
                aggregate_path_suffix(&leaf.path)
            ));
        }
        let leaves = leaves.scalars;
        let mut reads = Vec::with_capacity(leaves.len());
        for leaf in leaves {
            let name = self.element_access(path, &selection, &leaf, false)?;
            reads.push((leaf.path, element_leaf_value(name, leaf.ty)));
        }
        Ok(Some(reads))
    }

    /// Every leaf of a whole record element with queue or dynamic-array
    /// members, read from a copy that `setup` makes in a lexical temporary
    /// (its container members become the temporary's companions); `None`
    /// for other operands.
    pub(super) fn container_record_staged_reads(
        &mut self,
        path: &str,
        node: NodeId,
        setup: &mut Vec<IrStmt>,
    ) -> Result<Option<LeafReads>, String> {
        let Some(selection) = self.record_element_of(node) else {
            return Ok(None);
        };
        let (_, _, leaves) = self.element_leaves(selection.container, selection.depth())?;
        if leaves.containers.is_empty() {
            return Ok(None);
        }
        // The copy evaluates the element's locator once.
        let (temporary, copy) = self
            .element_copy(path, node)?
            .ok_or("container record element has no element copy")?;
        setup.extend(copy);
        let target = NativeEndpoint::Value {
            value: temporary,
            prefix: Vec::new(),
        };
        let mut reads = Vec::new();
        for (leaf_path, leaf) in self.endpoint_leaves(&target)? {
            reads.push((leaf_path, self.endpoint_leaf_read(&leaf)?));
        }
        Ok(Some(reads))
    }

    /// The element and queue or dynamic-array member path that `node` names:
    /// the member itself (`q[i].m`, a method receiver or whole operand) or a
    /// select of it (`q[i].m[j]`, whose frontend base is the member's
    /// declaration and whose select path names the element).
    fn element_member_container_parts(
        &self,
        node: NodeId,
    ) -> Option<(NodeId, Vec<AggregatePathPart>)> {
        let select = matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::ArraySelect { .. })
        )
        .then(|| self.db.array_select_path(node))
        .flatten();
        let (element, path) = match select {
            Some((owner, members)) => (
                owner,
                members
                    .iter()
                    .cloned()
                    .map(AggregatePathPart::Member)
                    .collect(),
            ),
            None => {
                let (element, _, path) = self.element_member_parts(node)?;
                (element, path)
            }
        };
        let selection = self.record_element_of(element)?;
        let (_, _, leaves) = self
            .element_leaves(selection.container, selection.depth())
            .ok()?;
        leaves
            .containers
            .iter()
            .any(|leaf| leaf.path == path)
            .then_some((element, path))
    }

    /// The diagnostic for a queue or dynamic-array member of a container
    /// record element (or a method call on one) used where no statement
    /// staged it.
    pub(in crate::sim::codegen) fn unstaged_element_member(&self, node: NodeId) -> Option<String> {
        let node = match self.kind(node) {
            NodeKind::MethodCall {
                receiver: Some(receiver),
                ..
            } => *receiver,
            _ => node,
        };
        self.element_member_container_parts(node)
            .map(|(_, path)| unstaged_member_error(&path))
    }

    /// The staged container of the element member that `node` names.
    pub(in crate::sim::codegen) fn staged_element_member(&self, node: NodeId) -> Option<usize> {
        if self.staged_element_members.is_empty() {
            return None;
        }
        let (element, _, path) = self.element_member_parts(node)?;
        self.staged_element_members
            .get(&(element, aggregate_path_suffix(&path)))
            .copied()
    }

    /// The staged container of member `members` of element `owner`, for a
    /// select whose frontend base is the member's declaration.
    pub(in crate::sim::codegen) fn staged_owner_member(
        &self,
        owner: NodeId,
        members: &[String],
    ) -> Option<usize> {
        if self.staged_element_members.is_empty() {
            return None;
        }
        let path = members
            .iter()
            .cloned()
            .map(AggregatePathPart::Member)
            .collect::<Vec<_>>();
        self.staged_element_members
            .get(&(owner, aggregate_path_suffix(&path)))
            .copied()
    }

    /// Collect the element member containers below `node`; `mutated` marks
    /// a write position (an assignment target or an increment operand).
    fn collect_staged_members(&self, node: NodeId, mutated: bool, out: &mut Vec<StagedMember>) {
        if let Some((element, path)) = self.element_member_container_parts(node) {
            if let Some(member) = out
                .iter_mut()
                .find(|member| member.element == element && member.path == path)
            {
                member.mutated |= mutated;
            } else {
                out.push(StagedMember {
                    element,
                    path,
                    mutated,
                });
            }
            // A select's indices are read before the member is written.
            if matches!(
                self.kind(node),
                NodeKind::Expr(ExprKind::ArraySelect { .. })
            ) {
                let NodeKind::Expr(ExprKind::ArraySelect { indices, .. }) = self.kind(node) else {
                    unreachable!("checked above");
                };
                for index in indices.clone() {
                    self.collect_staged_members(index, false, out);
                }
            }
            return;
        }
        let children = self.node(node).children.clone();
        match self.kind(node) {
            // The target's base chain is written; its indices are read.
            NodeKind::Expr(
                ExprKind::ArraySelect { base, .. }
                | ExprKind::BitSelect { base, .. }
                | ExprKind::PartSelect { base, .. }
                | ExprKind::IndexedPartSelect { base, .. },
            ) if mutated => {
                let base = *base;
                for child in children {
                    self.collect_staged_members(child, child == base, out);
                }
            }
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
                ..
            } => {
                let changes = MUTATING_CONTAINER_METHODS.contains(&name.as_str());
                let receiver = *receiver;
                for child in children {
                    self.collect_staged_members(child, child == receiver && changes, out);
                }
            }
            // A member passed to a subroutine may be an output, inout or
            // ref actual, so it is copied back after the call.
            NodeKind::FuncCall { .. } => {
                for child in children {
                    self.collect_staged_members(child, true, out);
                }
            }
            NodeKind::Expr(ExprKind::Operation {
                op:
                    Operation::Assignment
                    | Operation::PostIncrement
                    | Operation::PreIncrement
                    | Operation::PostDecrement
                    | Operation::PreDecrement,
                ..
            }) => {
                for (position, child) in children.into_iter().enumerate() {
                    self.collect_staged_members(child, position == 0, out);
                }
            }
            _ => {
                for child in children {
                    self.collect_staged_members(child, false, out);
                }
            }
        }
    }

    /// Stage the queue and dynamic-array members of container record
    /// elements that statement `h` names (`q[i].m.push_back(x)`,
    /// `x = q[i].m[j]`, `q[i].m = v`) in lexical containers: each member is
    /// copied out of its element before the statement and, when the
    /// statement can change it, copied back after it, so container
    /// operations work on it unchanged. Returns the statements to run before
    /// and after `h`; the caller ends the staging with
    /// `end_element_member_staging`. A container element keeps such a member
    /// as a nested dynamic array, which no container operation addresses.
    pub(in crate::sim::codegen) fn stage_element_members(
        &mut self,
        path: &str,
        h: NodeId,
    ) -> Result<(Vec<IrStmt>, Vec<IrStmt>), String> {
        let mut members = Vec::new();
        match self.kind(h) {
            NodeKind::Stmt(StmtKind::Assign { blocking, .. }) => {
                let blocking = *blocking;
                let children = self.node(h).children.clone();
                for (position, child) in children.into_iter().enumerate() {
                    self.collect_staged_members(child, position == 0, &mut members);
                }
                if !blocking && members.iter().any(|member| member.mutated) {
                    return Err(format!(
                        "nonblocking assignment to a queue or dynamic-array member of a container record element in `{path}` is not supported"
                    ));
                }
            }
            _ => self.collect_staged_members(h, false, &mut members),
        }
        let (mut before, mut after) = (Vec::new(), Vec::new());
        for member in members {
            let selection = self
                .record_element_of(member.element)
                .ok_or("staged element member has no element")?;
            let (_, _, leaves) = self.element_leaves(selection.container, selection.depth())?;
            let leaf = leaves
                .containers
                .into_iter()
                .find(|leaf| leaf.path == member.path)
                .ok_or("staged element member has no container")?;
            // The element is located again to copy the member back.
            if !self.side_effect_free(member.element) {
                return Err(format!(
                    "a container record element whose queue or dynamic-array member is used in `{path}` must be selected without side effects"
                ));
            }
            let ir = self.model.containers.len();
            self.model.containers.push(IrContainer {
                c_name: format!("S_llg_container_{ir}"),
                element: leaf.element.clone(),
                kind: leaf.kind.clone(),
                initial_size: None,
                activation: true,
                class_field: None,
            });
            self.staged_element_members
                .insert((member.element, aggregate_path_suffix(&member.path)), ir);
            let locator = |this: &mut Self, write: bool| -> Result<IrValueItemRoot, String> {
                let IrValueSlot::Element { indices, key } =
                    this.lower_element_slot(path, &selection)?
                else {
                    unreachable!("element selections lower to element slots");
                };
                Ok(IrValueItemRoot::Element(IrChandleExpr::ContainerElement {
                    container: selection.container,
                    indices,
                    key: key.map(Box::new),
                    write,
                }))
            };
            before.push(IrStmt::Container(Box::new(IrContainerStmt::Declare(ir))));
            before.push(IrStmt::Container(Box::new(
                IrContainerStmt::ValueItemToContainer {
                    root: locator(self, false)?,
                    items: leaf.items.clone(),
                    container: ir,
                },
            )));
            if member.mutated {
                after.push(IrStmt::Container(Box::new(
                    IrContainerStmt::ContainerToValueItem {
                        container: ir,
                        root: locator(self, true)?,
                        items: leaf.items.clone(),
                    },
                )));
            }
        }
        Ok((before, after))
    }

    /// End the staging that `stage_element_members` opened.
    pub(in crate::sim::codegen) fn end_element_member_staging(&mut self) {
        self.staged_element_members.clear();
    }

    /// A fresh access to one leaf of a selected record element.
    fn element_access(
        &mut self,
        path: &str,
        selection: &ElementSelection,
        leaf: &NativeLeaf,
        write: bool,
    ) -> Result<String, String> {
        let IrValueSlot::Element { indices, key } = self.lower_element_slot(path, selection)?
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
                item_path: leaf.items.clone(),
                function: self.cur_fn_ir,
            });
        Ok(name)
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
