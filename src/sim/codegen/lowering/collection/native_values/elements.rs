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
pub(in crate::sim::codegen) struct ElementSelection {
    pub(in crate::sim::codegen) container: usize,
    selector: ElementSelector,
}

impl ElementSelection {
    /// Whether evaluating the selection changes nothing.
    pub(super) fn side_effect_free(&self, codegen: &Codegen<'_>) -> bool {
        match &self.selector {
            ElementSelector::Indices(indices) => {
                indices.iter().all(|index| codegen.side_effect_free(*index))
            }
            ElementSelector::Key(key) => codegen.side_effect_free(*key),
        }
    }

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

/// One queue or dynamic array nested in a container element (`qq[i]`,
/// `sq[k]`) that a statement names whole, and whether the statement can
/// change it.
struct StagedNested {
    nodes: Vec<NodeId>,
    selection: ElementSelection,
    mutated: bool,
    /// Every name of the element is the target of a plain `=`, so its old
    /// value is never read.
    overwritten: bool,
}

fn unstaged_member_error(path: &[AggregatePathPart]) -> String {
    format!(
        "queue or dynamic-array member `{}` of a container record element is only supported in assignment, call and system-task statements and whole-element copies",
        aggregate_path_suffix(path)
    )
}

/// The value of a fresh element-leaf access named `name`.
pub(super) fn element_leaf_value(name: String, ty: IrClassFieldType) -> LeafValue {
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
        let staged = self.staged_container_types.get(&typed).cloned();
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
            });
        let mut descriptor = match staged {
            Some(descriptor) => descriptor,
            None => self.db.type_descriptor(declaration?)?.clone(),
        };
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
    pub(super) fn element_items_to_companions(&self, value: usize) -> Result<Vec<IrStmt>, String> {
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
    pub(super) fn companions_to_element_items(&self, value: usize) -> Result<Vec<IrStmt>, String> {
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
            class_field: None,
            receiver: None,
        });
        self.native_value_layouts.insert(index, site);
        Ok(index)
    }

    pub(super) fn lower_element_slot(
        &mut self,
        path: &str,
        selection: &ElementSelection,
    ) -> Result<IrValueSlot, String> {
        if let Some(slot) = &self.element_nba_target {
            return Ok(slot.clone());
        }
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

    /// Copy a whole record element, or the record removed by a pop, into
    /// `destination`, a whole fixed record held as one packed value (a
    /// procedural-block local): the element is copied once into a temporary
    /// and its leaves are concatenated in the record's packed layout. `None`
    /// for other sources or destinations.
    pub(super) fn container_record_to_fixed(
        &mut self,
        path: &str,
        destination: NodeId,
        source: NodeId,
        nba: bool,
    ) -> Result<Option<IrStmt>, String> {
        let source = self.p30_unwrap_cast(source);
        if !self.is_container_record(source)
            || !matches!(self.kind(destination), NodeKind::Expr(ExprKind::Ref { .. }))
        {
            return Ok(None);
        }
        let Some(descriptor) = self.query_descriptor(destination).cloned() else {
            return Ok(None);
        };
        let Some(width) = Self::fixed_descriptor_width(&descriptor) else {
            return Ok(None);
        };
        if self.fixed_value_width(destination) != Some(width) {
            return Ok(None);
        }
        let Some((temporary, mut statements)) = self.element_copy(path, source)? else {
            return Ok(None);
        };
        let Some(mut write) =
            self.native_fixed_destination(path, destination, &descriptor, temporary)?
        else {
            return Err(format!(
                "record element read into `{}` in `{path}` needs integral record members",
                self.node(destination).name
            ));
        };
        if let IrStmt::Assign { nba: write_nba, .. } = &mut write {
            *write_nba = nba;
        }
        statements.push(write);
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

    /// Whether `node` names a member of a record element of container
    /// storage, or a select inside one, with constant or run-time indices.
    pub(in crate::sim::codegen) fn names_element_member(&self, node: NodeId) -> bool {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::HierPath { parts, refs }) => {
                parts.iter().skip(1).any(|part| !part.is_empty())
                    && refs
                        .first()
                        .copied()
                        .flatten()
                        .is_some_and(|first| self.record_element_of(first).is_some())
            }
            NodeKind::Expr(ExprKind::ArraySelect { base, .. }) => {
                self.db
                    .array_select_path(node)
                    .is_some_and(|(owner, _)| self.record_element_of(owner).is_some())
                    || self.names_element_member(*base)
            }
            NodeKind::Expr(
                ExprKind::BitSelect { base, .. }
                | ExprKind::PartSelect { base, .. }
                | ExprKind::IndexedPartSelect { base, .. },
            ) => self.names_element_member(*base),
            _ => false,
        }
    }

    /// Whether `node` is a member path rooted at a selected array element
    /// (`recs[i].n`), which names no signal of its own.
    pub(in crate::sim::codegen) fn selected_member_path(&self, node: NodeId) -> bool {
        let NodeKind::Expr(ExprKind::HierPath { refs, .. }) = self.kind(node) else {
            return false;
        };
        refs.first().copied().flatten().is_some_and(|first| {
            matches!(
                self.kind(first),
                NodeKind::Expr(ExprKind::ArraySelect { .. } | ExprKind::BitSelect { .. })
            )
        })
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

    /// The element and leaf a nonblocking write of `node` stores to: the
    /// leaf itself or the packed leaf a select chain is inside; `None` for a
    /// whole sub-record or member array.
    pub(in crate::sim::codegen) fn element_nba_leaf(
        &self,
        node: NodeId,
    ) -> Result<Option<(ElementSelection, NativeLeaf)>, String> {
        if let Some((_, selection, leaf, _)) = self.element_leaf_select(node)? {
            return Ok(Some((selection, leaf)));
        }
        // A whole container member takes the sub-record path.
        if self.element_container_member(node) {
            return Ok(None);
        }
        if let Some(found) = self.element_leaf(node)? {
            return Ok(Some(found));
        }
        match self.kind(node) {
            NodeKind::Expr(
                ExprKind::BitSelect { base, .. }
                | ExprKind::PartSelect { base, .. }
                | ExprKind::IndexedPartSelect { base, .. },
            ) => self.element_nba_leaf(*base),
            NodeKind::Expr(ExprKind::ArraySelect { base, .. })
                if self.db.array_select_path(node).is_none() =>
            {
                self.element_nba_leaf(*base)
            }
            _ => Ok(None),
        }
    }

    /// Whether `node` names a whole container member of a record element.
    fn element_container_member(&self, node: NodeId) -> bool {
        self.element_member_path(node)
            .is_some_and(|(selection, member)| {
                self.element_leaves(selection.container, selection.depth())
                    .is_ok_and(|(_, _, leaves)| {
                        leaves.containers.iter().any(|leaf| leaf.path == member)
                    })
            })
    }

    /// The selection, member path and scalar and container leaves of a whole
    /// sub-record, member array or container member of a record element.
    #[allow(clippy::type_complexity)]
    pub(in crate::sim::codegen) fn element_member_leaves(
        &self,
        path: &str,
        node: NodeId,
    ) -> Result<
        (
            ElementSelection,
            Vec<AggregatePathPart>,
            Vec<NativeLeaf>,
            Vec<NativeContainerLeaf>,
        ),
        String,
    > {
        let not_supported = || {
            format!(
                "a nonblocking assignment to this selection of a record element in `{path}` is not supported; assign its members or the whole element"
            )
        };
        let (selection, prefix) = self.element_member_path(node).ok_or_else(not_supported)?;
        let (_, _, leaves) = self.element_leaves(selection.container, selection.depth())?;
        let scalars = leaves
            .scalars
            .into_iter()
            .filter(|leaf| leaf.path.starts_with(&prefix))
            .collect::<Vec<_>>();
        let containers = leaves
            .containers
            .into_iter()
            .filter(|leaf| leaf.path.starts_with(&prefix))
            .collect::<Vec<_>>();
        if scalars.is_empty() && containers.is_empty() {
            return Err(not_supported());
        }
        Ok((selection, prefix, scalars, containers))
    }

    /// A native temporary of the element type and the descriptor of the
    /// member at `prefix`.
    pub(in crate::sim::codegen) fn element_nba_temporary(
        &mut self,
        selection: ElementSelection,
        source: NodeId,
        prefix: &[AggregatePathPart],
    ) -> Result<(usize, TypeDescriptor), String> {
        let temporary = self.element_temporary(selection.container, selection.depth(), source)?;
        let layout = self.native_layout_of_value(temporary)?;
        let descriptor = Self::descriptor_at_path(&layout.descriptor, prefix)
            .ok_or("record element member has no type")?;
        Ok((temporary, descriptor))
    }

    /// A read access to `leaf` that shares a member NBA's captured `slot`.
    pub(in crate::sim::codegen) fn element_nba_access(
        &mut self,
        path: &str,
        selection: &ElementSelection,
        slot: &IrValueSlot,
        leaf: &NativeLeaf,
    ) -> Result<String, String> {
        self.element_nba_target = Some(slot.clone());
        let name = self.element_access(path, selection, leaf, false);
        self.element_nba_target = None;
        name
    }

    /// The element slot of a member NBA with its run-time indices captured
    /// once, in locals declared by `statements`: the locator and the typed
    /// access share them.
    pub(in crate::sim::codegen) fn element_nba_slot(
        &mut self,
        path: &str,
        selection: &ElementSelection,
        statements: &mut Vec<IrStmt>,
    ) -> Result<IrValueSlot, String> {
        let mut slot = self.lower_element_slot(path, selection)?;
        let IrValueSlot::Element { indices, .. } = &mut slot else {
            unreachable!("element selections lower to element slots");
        };
        for index in indices.iter_mut() {
            if matches!(index.kind(), IrExprKind::Const(_)) {
                continue;
            }
            let name = format!("_llg_nba_index_{}", self.native_copy_sequence);
            self.native_copy_sequence += 1;
            let (width, signed) = (index.width, index.signed);
            statements.push(IrStmt::DeclLocal {
                name: name.clone(),
                width,
                signed,
                two_state: false,
                init: Some(Box::new(index.clone())),
            });
            *index = IrExpr::new(IrExprKind::LocalRead(name), width, signed, None);
        }
        Ok(slot)
    }

    /// Whether `node` names a whole packed leaf of a container record
    /// element.
    pub(in crate::sim::codegen) fn element_packed_leaf(&self, node: NodeId) -> bool {
        self.names_element_member(node)
            && matches!(
                self.element_leaf_kind(node),
                Some(IrClassFieldType::Packed { .. })
            )
    }

    /// `recs[i].a[1][k]`: the frontend flattens a bit of a packed element
    /// of a member array into one select. Returns the array node, the
    /// packed leaf and the indices that select inside it.
    #[allow(clippy::type_complexity)]
    fn element_leaf_select(
        &self,
        node: NodeId,
    ) -> Result<Option<(NodeId, ElementSelection, NativeLeaf, Vec<NodeId>)>, String> {
        let NodeKind::Expr(ExprKind::ArraySelect { base, indices }) = self.kind(node) else {
            return Ok(None);
        };
        let Some((owner, members)) = self.db.array_select_path(node) else {
            return Ok(None);
        };
        let Some(selection) = self.record_element_of(owner) else {
            return Ok(None);
        };
        let mut path: Vec<_> = members
            .iter()
            .cloned()
            .map(AggregatePathPart::Member)
            .collect();
        let (_, _, leaves) = self.element_leaves(selection.container, selection.depth())?;
        for position in 0..=indices.len() {
            if let Some(leaf) = leaves.scalars.iter().find(|leaf| leaf.path == path) {
                let trailing = indices[position..].to_vec();
                return Ok((!trailing.is_empty()
                    && matches!(leaf.ty, IrClassFieldType::Packed { .. }))
                .then(|| (*base, selection, leaf.clone(), trailing)));
            }
            let Some(index) = indices
                .get(position)
                .and_then(|index| self.eval_bound_i128(*index).ok())
                .and_then(|index| i32::try_from(index).ok())
            else {
                return Ok(None);
            };
            path.push(AggregatePathPart::Index(index));
        }
        Ok(None)
    }

    /// Packed steps of the selects inside a leaf named by
    /// [`Self::element_leaf_select`], with the leaf's access and width.
    pub(in crate::sim::codegen) fn element_leaf_select_steps(
        &mut self,
        path: &str,
        node: NodeId,
        write: bool,
    ) -> Result<
        Option<(
            String,
            IrClassFieldType,
            Vec<crate::sim::ir::IrPackedSelect>,
        )>,
        String,
    > {
        let Some((array, selection, leaf, trailing)) = self.element_leaf_select(node)? else {
            return Ok(None);
        };
        let IrClassFieldType::Packed { width, .. } = leaf.ty else {
            return Ok(None);
        };
        let name = self.element_access(path, &selection, &leaf, write)?;
        let mut width = width;
        let steps = self.packed_steps_of_elements(path, array, trailing, &mut width)?;
        Ok(Some((name, leaf.ty, steps)))
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
    fn collect_staged_members(
        &self,
        node: NodeId,
        mutated: bool,
        out: &mut Vec<StagedMember>,
        nested: &mut Vec<(NodeId, ElementSelection, bool)>,
    ) {
        // A queue or dynamic array nested in an element of a string-keyed
        // array has no other access form; it is staged wherever it is named.
        if let Some(selection) = self.nested_element_selection(node) {
            if let ElementSelector::Key(key) = selection.selector {
                nested.push((node, selection, mutated));
                self.collect_staged_members(key, false, out, nested);
                return;
            }
        }
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
                    self.collect_staged_members(index, false, out, nested);
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
                let indices = match self.kind(node) {
                    NodeKind::Expr(ExprKind::ArraySelect { indices, .. }) => indices.clone(),
                    _ => Vec::new(),
                };
                for child in children {
                    // A flattened select keeps its outer select as a child.
                    let chain = !indices.contains(&child)
                        && matches!(
                            self.kind(child),
                            NodeKind::Expr(
                                ExprKind::ArraySelect { .. } | ExprKind::BitSelect { .. }
                            )
                        );
                    self.collect_staged_members(child, child == base || chain, out, nested);
                }
            }
            // A member of an element of the staged container (`a[1].q[0].s`).
            NodeKind::Expr(ExprKind::HierPath { .. }) if mutated => {
                for child in children {
                    self.collect_staged_members(child, true, out, nested);
                }
            }
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
                ..
            } => {
                let changes = MUTATING_CONTAINER_METHODS.contains(&name.as_str());
                let receiver = *receiver;
                // A nested element's `size()` has a direct query; any other
                // method runs on a staged copy (SV 7.5.1, 7.10.2, 7.12).
                let staged_receiver = (name != "size")
                    .then(|| self.nested_element_selection(receiver))
                    .flatten();
                // A nested element pushed or inserted whole is read once.
                let pushes = matches!(name.as_str(), "push_back" | "push_front" | "insert");
                for child in children {
                    if pushes && child != receiver && self.stage_whole_read(child, out, nested) {
                        continue;
                    }
                    if child == receiver {
                        if let Some(selection) = &staged_receiver {
                            if let ElementSelector::Indices(indices) = &selection.selector {
                                for index in indices.clone() {
                                    self.collect_staged_members(index, false, out, nested);
                                }
                                nested.push((receiver, selection.clone(), changes));
                                continue;
                            }
                        }
                    }
                    self.collect_staged_members(child, child == receiver && changes, out, nested);
                }
            }
            // A member passed to a subroutine may be an output, inout or
            // ref actual, so it is copied back after the call. A nested
            // element passed whole to an input formal is read once.
            NodeKind::FuncCall { callee, .. } => {
                let directions: Vec<DbDirection> = callee
                    .map(|callee| {
                        self.node(callee)
                            .children
                            .iter()
                            .filter_map(|formal| match self.kind(*formal) {
                                NodeKind::FuncArg { direction, .. } => Some(*direction),
                                _ => None,
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let positional = directions.len() == children.len();
                for (position, child) in children.into_iter().enumerate() {
                    let direction = positional.then(|| directions[position]);
                    if direction == Some(DbDirection::Input)
                        && self.stage_whole_read(child, out, nested)
                    {
                        continue;
                    }
                    // A nested element passed whole to an output or inout
                    // formal: a copy goes in and comes back after the call.
                    if matches!(direction, Some(DbDirection::Output | DbDirection::Inout)) {
                        // The frontend wraps the actual in an assignment.
                        let actual = match self.kind(child) {
                            NodeKind::Expr(ExprKind::Operation {
                                op: Operation::Assignment,
                                operands,
                                ..
                            }) => operands.first().copied().unwrap_or(child),
                            _ => child,
                        };
                        if let Some(selection) = self.nested_element_selection(actual) {
                            if let ElementSelector::Indices(indices) = &selection.selector {
                                for index in indices.clone() {
                                    self.collect_staged_members(index, false, out, nested);
                                }
                                nested.push((actual, selection, true));
                                continue;
                            }
                        }
                    }
                    self.collect_staged_members(child, true, out, nested);
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
                    self.collect_staged_members(child, position == 0, out, nested);
                }
            }
            _ => {
                for child in children {
                    self.collect_staged_members(child, false, out, nested);
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
        let mut nested = Vec::new();
        // The target of a plain blocking `=`, whose old value is not read.
        let mut overwritten = None;
        match self.kind(h) {
            NodeKind::Stmt(StmtKind::Assign { blocking, op, .. }) => {
                let blocking = *blocking;
                let children = self.node(h).children.clone();
                if blocking && *op == Operation::Assignment {
                    overwritten = children.first().copied();
                }
                for (position, child) in children.into_iter().enumerate() {
                    // A nonblocking write of a whole member is queued from
                    // the source, without a staged copy of the target.
                    if position == 0 && !blocking && self.element_container_member(child) {
                        continue;
                    }
                    // A nested element assigned whole (`row = qq[i]`) is read
                    // from a staged copy.
                    if position == 1 && self.stage_whole_read(child, &mut members, &mut nested) {
                        continue;
                    }
                    self.collect_staged_members(child, position == 0, &mut members, &mut nested);
                }
                if !blocking
                    && (members.iter().any(|member| member.mutated)
                        || nested.iter().any(|(_, _, mutated)| *mutated))
                {
                    return Err(format!(
                        "nonblocking assignment to a queue or dynamic-array member of a container record element in `{path}` is not supported"
                    ));
                }
            }
            _ => self.collect_staged_members(h, false, &mut members, &mut nested),
        }
        self.build_element_staging(path, members, nested, overwritten)
    }

    /// Stage the members and nested elements a container initializer reads;
    /// nothing is written back. The caller ends the staging with
    /// `end_element_member_staging`.
    pub(in crate::sim::codegen) fn stage_initializer(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Vec<IrStmt>, String> {
        let (mut members, mut nested) = (Vec::new(), Vec::new());
        if !self.stage_whole_read(node, &mut members, &mut nested) {
            self.collect_staged_members(node, false, &mut members, &mut nested);
        }
        let (before, after) = self.build_element_staging(path, members, nested, None)?;
        debug_assert!(after.is_empty(), "an initializer writes back nothing");
        Ok(before)
    }

    /// Stage the queue and dynamic-array members of container record
    /// elements (and queues or dynamic arrays nested in container elements)
    /// that the expression `node` reads: a condition, loop bound,
    /// declaration initializer or the value of a timed assignment. Each is
    /// copied out before every evaluation and, when the expression changes
    /// it, copied back after it. Returns those statements, or `None` when
    /// `node` names none. The caller lowers `node` and then ends the staging
    /// with `end_element_member_staging`.
    pub(in crate::sim::codegen) fn stage_expression_members(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<(Vec<IrStmt>, Vec<IrStmt>)>, String> {
        if !self.element_staging_possible
            || !self.staged_element_members.is_empty()
            || !self.staged_nested_elements.is_empty()
        {
            return Ok(None);
        }
        let (mut members, mut nested) = (Vec::new(), Vec::new());
        self.collect_staged_members(node, false, &mut members, &mut nested);
        if members.is_empty() && nested.is_empty() {
            return Ok(None);
        }
        // A skipped operand must not write its element back (that would
        // create a missing associative entry): each operand stages its own.
        if (members.iter().any(|member| member.mutated)
            || nested.iter().any(|(_, _, mutated)| *mutated))
            && self.lazily_evaluated_operands(node)
        {
            return Ok(None);
        }
        self.build_element_staging(path, members, nested, None)
            .map(Some)
    }

    fn lazily_evaluated_operands(&self, node: NodeId) -> bool {
        matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::LogicalAnd | Operation::LogicalOr | Operation::Conditional,
                ..
            })
        )
    }

    /// The copy-out statements of `members` and `nested` and the write-back
    /// statements of the changed ones.
    fn build_element_staging(
        &mut self,
        path: &str,
        members: Vec<StagedMember>,
        nested: Vec<(NodeId, ElementSelection, bool)>,
        overwritten: Option<NodeId>,
    ) -> Result<(Vec<IrStmt>, Vec<IrStmt>), String> {
        let (mut before, mut after) = (Vec::new(), Vec::new());
        for member in members {
            let selection = self
                .record_element_of(member.element)
                .ok_or("staged element member has no element")?;
            let (_, record, leaves) =
                self.element_leaves(selection.container, selection.depth())?;
            let leaf = leaves
                .containers
                .into_iter()
                .find(|leaf| leaf.path == member.path)
                .ok_or("staged element member has no container")?;
            let typed = Self::descriptor_at_path(&record, &member.path);
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
                receiver: None,
            });
            if let Some(typed) = typed {
                self.staged_container_types.insert(ir, typed);
            }
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
        self.stage_nested_elements(path, nested, overwritten, &mut before, &mut after)?;
        Ok((before, after))
    }

    /// Stage the queues and dynamic arrays nested in container elements that
    /// a statement names whole (`qq[i].push_back(x)`, `sq[k] = src`,
    /// `x = sq[k][j]`) in lexical containers of the nested type, as
    /// `stage_element_members` does for record members: names of one element
    /// share one copy, which is written back when the statement can change
    /// it. The element is located again for the write-back, which creates a
    /// missing associative entry (SV 7.8.6) and discards a write to an
    /// invalid index (SV 7.4.6, 7.10.1).
    fn stage_nested_elements(
        &mut self,
        path: &str,
        nested: Vec<(NodeId, ElementSelection, bool)>,
        overwritten: Option<NodeId>,
        before: &mut Vec<IrStmt>,
        after: &mut Vec<IrStmt>,
    ) -> Result<(), String> {
        let mut groups: Vec<StagedNested> = Vec::new();
        for (node, selection, mutated) in nested {
            if !self.side_effect_free(node) {
                return Err(format!(
                    "a nested container element used whole in `{path}` must be selected without side effects"
                ));
            }
            let target = Some(node) == overwritten;
            if let Some(group) = groups
                .iter_mut()
                .find(|group| self.same_element_selection(&group.selection, &selection))
            {
                group.nodes.push(node);
                group.mutated |= mutated;
                group.overwritten &= target;
                continue;
            }
            groups.push(StagedNested {
                nodes: vec![node],
                selection,
                mutated,
                overwritten: target,
            });
        }
        // Two changed copies of possibly one element would lose a change.
        for (position, group) in groups.iter().enumerate() {
            if group.mutated
                && groups.iter().enumerate().any(|(other, staged)| {
                    other != position
                        && staged.mutated
                        && staged.selection.container == group.selection.container
                        && !self.distinct_element_selections(&group.selection, &staged.selection)
                })
            {
                return Err(format!(
                    "a statement in `{path}` changes two nested container elements of one container; change them in separate statements"
                ));
            }
        }
        for group in groups {
            let selection = &group.selection;
            let depth = selection.depth();
            let Some(IrContainerElement::Container { kind, element, .. }) =
                self.container_element_type(selection.container, depth)
            else {
                return Err(format!(
                    "nested container element in `{path}` does not select a queue or dynamic array"
                ));
            };
            // The nested type gives a queue's bound and the element type of
            // record operations on the staged copy.
            let descriptor = self
                .container_element_descriptor(selection.container, depth)
                .or_else(|| self.query_descriptor(group.nodes[0]).cloned());
            let kind = if kind == "Queue" {
                let Some(TypeShape::Container {
                    array: ArrayKind::Queue { maximum_elements },
                    ..
                }) = descriptor.as_ref().map(|descriptor| &descriptor.shape)
                else {
                    return Err(format!(
                        "nested queue element in `{path}` has no queue type descriptor"
                    ));
                };
                IrContainerKind::Queue {
                    maximum_elements: *maximum_elements,
                }
            } else {
                IrContainerKind::Dynamic
            };
            let ir = self.model.containers.len();
            self.model.containers.push(IrContainer {
                c_name: format!("S_llg_container_{ir}"),
                element: *element,
                kind,
                initial_size: None,
                activation: true,
                class_field: None,
                receiver: None,
            });
            if let Some(descriptor) = descriptor {
                self.staged_container_types.insert(ir, descriptor);
            }
            for node in &group.nodes {
                self.staged_nested_elements.insert(*node, ir);
            }
            let locator = |this: &mut Self, write: bool| -> Result<IrValueItemRoot, String> {
                let IrValueSlot::Element { indices, key } =
                    this.lower_element_slot(path, selection)?
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
            // A changed element is located as a write target throughout, so
            // a missing associative entry is created without a read warning.
            if !(group.overwritten && group.mutated) {
                before.push(IrStmt::Container(Box::new(
                    IrContainerStmt::ValueItemToContainer {
                        root: locator(self, group.mutated)?,
                        items: Vec::new(),
                        container: ir,
                    },
                )));
            }
            if group.mutated {
                after.push(IrStmt::Container(Box::new(
                    IrContainerStmt::ContainerToValueItem {
                        container: ir,
                        root: locator(self, true)?,
                        items: Vec::new(),
                    },
                )));
            }
        }
        Ok(())
    }

    /// Stage `node` when it reads a queue or dynamic array nested in an
    /// element of integral-indexed container storage whole; string-keyed
    /// elements are staged wherever they are named.
    fn stage_whole_read(
        &self,
        node: NodeId,
        out: &mut Vec<StagedMember>,
        nested: &mut Vec<(NodeId, ElementSelection, bool)>,
    ) -> bool {
        let node = self.p30_unwrap_cast(node);
        let Some(selection) = self.nested_element_selection(node) else {
            return false;
        };
        let ElementSelector::Indices(indices) = &selection.selector else {
            return false;
        };
        for index in indices.clone() {
            self.collect_staged_members(index, false, out, nested);
        }
        nested.push((node, selection, false));
        true
    }

    /// A queue or dynamic array nested in an element of container storage
    /// (`qq[i]`, `qd[i][j]`, `sq[k]`), named whole.
    fn nested_element_selection(&self, node: NodeId) -> Option<ElementSelection> {
        let nested = |element: Option<IrContainerElement>| {
            matches!(element, Some(IrContainerElement::Container { kind, .. })
                if !kind.to_ascii_lowercase().contains("assoc"))
        };
        // `sq[k][i]` selects inside the element; its child `sq[k]` names it.
        let inside = matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::ArraySelect { indices, .. }) if indices.len() > 1
        );
        if let Some((container, key)) = self.associative_string_element(node).filter(|_| !inside) {
            return nested(Some(self.model.containers[container].element.clone())).then_some(
                ElementSelection {
                    container,
                    selector: ElementSelector::Key(key),
                },
            );
        }
        let (container, indices) = self.container_element_path(node)?;
        nested(self.container_element_type(container, indices.len())).then_some(ElementSelection {
            container,
            selector: ElementSelector::Indices(indices),
        })
    }

    /// Whether two selections name one element whenever they are evaluated
    /// together: the same container and, level by level, the same variable
    /// or equal constants.
    fn same_element_selection(&self, a: &ElementSelection, b: &ElementSelection) -> bool {
        let same = |x: NodeId, y: NodeId| {
            x == y
                || match (self.kind(x), self.kind(y)) {
                    (
                        NodeKind::Expr(ExprKind::Ref { target: Some(left) }),
                        NodeKind::Expr(ExprKind::Ref {
                            target: Some(right),
                        }),
                    ) => left == right,
                    (
                        NodeKind::Expr(ExprKind::Constant { value: left, .. }),
                        NodeKind::Expr(ExprKind::Constant { value: right, .. }),
                    ) => left == right,
                    _ => false,
                }
        };
        a.container == b.container
            && match (&a.selector, &b.selector) {
                (ElementSelector::Key(x), ElementSelector::Key(y)) => same(*x, *y),
                (ElementSelector::Indices(x), ElementSelector::Indices(y)) => {
                    x.len() == y.len() && x.iter().zip(y).all(|(x, y)| same(*x, *y))
                }
                _ => false,
            }
    }

    /// Whether two selections can never name one element: at some level
    /// both are constants of different values.
    fn distinct_element_selections(&self, a: &ElementSelection, b: &ElementSelection) -> bool {
        let differ = |x: NodeId, y: NodeId| {
            matches!(
                (
                    self.kind(self.p30_unwrap_cast(x)),
                    self.kind(self.p30_unwrap_cast(y))
                ),
                (
                    NodeKind::Expr(ExprKind::Constant { value: left, .. }),
                    NodeKind::Expr(ExprKind::Constant { value: right, .. }),
                ) if left != right
            )
        };
        match (&a.selector, &b.selector) {
            (ElementSelector::Key(x), ElementSelector::Key(y)) => differ(*x, *y),
            (ElementSelector::Indices(x), ElementSelector::Indices(y)) => {
                x.iter().zip(y).any(|(x, y)| differ(*x, *y))
            }
            _ => false,
        }
    }

    /// The lexical container staging the nested element `node` names.
    pub(in crate::sim::codegen) fn staged_nested_element(&self, node: NodeId) -> Option<usize> {
        if self.staged_nested_elements.is_empty() {
            return None;
        }
        self.staged_nested_elements.get(&node).copied()
    }

    /// End the staging that `stage_element_members` opened.
    pub(in crate::sim::codegen) fn end_element_member_staging(&mut self) {
        self.staged_element_members.clear();
        self.staged_nested_elements.clear();
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
        // The access of a queued write only types it: nothing changes yet.
        let write = write && self.element_nba_target.is_none();
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
