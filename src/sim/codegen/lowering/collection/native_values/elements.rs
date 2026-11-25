//! Record and fixed-array elements of descriptor-backed containers (SIM-006).
//!
//! A record element stays one runtime value inside its container. Members are
//! read and written in place through element-item accesses whose locator is
//! re-evaluated at every use; whole elements move between a container and any
//! record endpoint through a lexical native temporary of the element type, so
//! module records, native roots, patterns and calls share the SIM-003
//! transfers. Copies are deep except for handles (SV 7.5-7.10, 8.4).
use super::*;
use crate::sim::ir::{
    IrElementItem, IrElementItemValue, IrInsideArray, IrValueItemRoot, IrValueSlot,
};

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

/// The item positions of `path` below a value of type `element`, which
/// becomes the type reached; `None` when the path crosses a union, selects a
/// row of a multidimensional array or an index outside one.
fn member_items(
    element: &mut IrContainerElement,
    mut path: &[AggregatePathPart],
) -> Option<Vec<u32>> {
    let mut items = Vec::new();
    while !path.is_empty() {
        let mut reached = std::mem::replace(element, IrContainerElement::String);
        let next = match &mut reached {
            // A member of a tagged union needs a check of its tag.
            IrContainerElement::Aggregate { members, .. }
                if members
                    .first()
                    .is_some_and(|member| member.name != NATIVE_TAG_MEMBER) =>
            {
                let AggregatePathPart::Member(name) = &path[0] else {
                    return None;
                };
                let position = members.iter().position(|member| member.name == *name)?;
                items.push(u32::try_from(position).ok()?);
                path = &path[1..];
                &mut members[position].element
            }
            IrContainerElement::FixedArray {
                dimensions,
                element,
            } => {
                let mut flat = 0u64;
                for (slot, (left, right)) in dimensions.iter().enumerate() {
                    let Some(AggregatePathPart::Index(index)) = path.get(slot) else {
                        return None;
                    };
                    let (low, high) = (*left.min(right), *left.max(right));
                    if *index < low || *index > high {
                        return None;
                    }
                    let extent = u64::from(left.abs_diff(*right)) + 1;
                    flat = flat.checked_mul(extent)? + u64::from(index.abs_diff(*left));
                }
                items.push(u32::try_from(flat).ok()?);
                path = &path[dimensions.len()..];
                element
            }
            _ => return None,
        };
        *element = std::mem::replace(next, IrContainerElement::String);
    }
    Some(items)
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

/// The statements around staged uses: before and after.
type StagingStatements = (Vec<IrStmt>, Vec<IrStmt>);

/// A value evaluated before its statement's nested array is bound.
#[derive(Clone)]
pub(in crate::sim::codegen) enum HoistedValue {
    Packed(String, u32, bool),
    String(String),
    Handle(String),
    /// A lexical native value of the element's record type.
    Record(usize),
}

/// What a staging walks again once a bound associative array exposes the
/// level below it.
#[derive(Clone, Copy)]
enum StageRoot {
    Statement(NodeId),
    Expression(NodeId),
    Initializer(NodeId),
}

/// One queue or dynamic-array member of a container record element that a
/// statement names, and whether the statement can change it.
struct StagedMember {
    element: NodeId,
    path: Vec<AggregatePathPart>,
    mutated: bool,
}

/// One queue, dynamic or associative array nested in a container element
/// (`qq[i]`, `sq[k]`) that a statement names whole, and whether the statement
/// can change it.
struct StagedNested {
    nodes: Vec<NodeId>,
    selection: ElementSelection,
    mutated: bool,
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

/// Whether each scalar leaf of a record stands alone: no union shares storage
/// or carries a tag above it.
fn leaf_record(descriptor: &TypeDescriptor) -> bool {
    let TypeShape::Aggregate(layout) = &descriptor.shape else {
        return true;
    };
    match layout.kind {
        AggregateKind::UnpackedStruct => layout
            .members
            .iter()
            .all(|member| leaf_record(&member.descriptor)),
        AggregateKind::PackedStruct | AggregateKind::PackedUnion => true,
        AggregateKind::UnpackedUnion | AggregateKind::TaggedUnion => false,
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
        if self.model.containers.is_empty() {
            return None;
        }
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

    /// `da[k][j]`, `aa[s][j]`, `q[k].a[j]` flattened: the frontend imports a
    /// select below a whole record or fixed-array element of container
    /// storage as one select of every index, and keeps the select of the
    /// element itself as a child. Returns that child, its selection and the
    /// indices left below it, outermost first.
    pub(in crate::sim::codegen) fn element_cell_select(
        &self,
        node: NodeId,
    ) -> Option<(NodeId, ElementSelection, Vec<NodeId>)> {
        let NodeKind::Expr(ExprKind::ArraySelect { base, indices }) = self.kind(node) else {
            return None;
        };
        let mut current = node;
        loop {
            let NodeKind::Expr(ExprKind::ArraySelect { indices: inner, .. }) = self.kind(current)
            else {
                return None;
            };
            // Of the indices only the last is a child: the others belong to
            // the selects below.
            let index = inner.last();
            crate::work::count(crate::work::Work::SelectIndices, 1);
            // The element select shares the flattened select's base; a
            // member array below a record element (`q[i].a[j]`) does not.
            let (child, consumed) =
                self.node(current)
                    .children
                    .iter()
                    .find_map(|child| match self.kind(*child) {
                        NodeKind::Expr(ExprKind::ArraySelect {
                            base: child_base,
                            indices,
                        }) if child_base == base && index != Some(child) => {
                            Some((*child, indices.len()))
                        }
                        NodeKind::Expr(ExprKind::BitSelect {
                            base: child_base, ..
                        }) if child_base == base && index != Some(child) => Some((*child, 1)),
                        _ => None,
                    })?;
            if let Some(selection) = self.record_element_of(child) {
                let trailing = indices.get(consumed..)?.to_vec();
                return (!trailing.is_empty()).then_some((child, selection, trailing));
            }
            current = child;
        }
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
            .unordered()
            .filter(|(_, info)| info.ir == typed)
            .map(|(node, _)| *node)
            .min_by_key(|node| node.index())
            .or_else(|| {
                self.subroutine_containers
                    .unordered()
                    .filter(|(_, ir)| **ir == typed)
                    .map(|((_, node), _)| *node)
                    .min_by_key(|node| node.index())
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
        validate_native_type(&element, format_args!("container element")).map_err(|error| {
            format!(
                "record elements of container `{}` are not supported: {error}",
                descriptor.name
            )
        })?;
        let mut leaves = NativeLeaves {
            columns: self.record_column_layout(&descriptor),
            ..NativeLeaves::default()
        };
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
            fixed: leaves.fixed,
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
            marker: false,
            ref_rooted: false,
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

    /// The array that `node` names as an element of container storage, or as
    /// a member below one: a nested queue, dynamic or associative array or a
    /// fixed-size array. An invalid index locates an empty or default array
    /// (SV 7.4.6).
    pub(in crate::sim::codegen) fn lower_element_array(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrInsideArray>, String> {
        let whole = self
            .nested_element_selection(node)
            .or_else(|| self.record_element_of(node));
        let (selection, member) = match whole {
            Some(selection) => (selection, Vec::new()),
            None => match self.element_member_parts(node) {
                Some((_, selection, member)) => (selection, member),
                None => return Ok(None),
            },
        };
        let Some(mut array) = self.container_element_type(selection.container, selection.depth())
        else {
            return Ok(None);
        };
        let Some(items) = member_items(&mut array, &member) else {
            return Ok(None);
        };
        if !matches!(
            array,
            IrContainerElement::Container { .. } | IrContainerElement::FixedArray { .. }
        ) {
            return Ok(None);
        }
        let IrValueSlot::Element { indices, key } = self.lower_element_slot(path, &selection)?
        else {
            unreachable!("element selections lower to element slots");
        };
        Ok(Some(IrInsideArray::Value {
            element: IrChandleExpr::ContainerElement {
                container: selection.container,
                indices,
                key: key.map(Box::new),
                write: false,
            },
            items,
        }))
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
        if let Some(HoistedValue::Record(temporary)) = self.hoisted_value(rhs) {
            return Ok(IrStmt::Container(Box::new(IrContainerStmt::SetValue {
                container,
                slot,
                value: *temporary,
            })));
        }
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
        if let Some(statement) = self.lower_element_from_row(path, &selection, rhs)? {
            return Ok(Some(statement));
        }
        if let Some(statement) = self.element_items_assignment(path, &selection, rhs)? {
            return Ok(Some(statement));
        }
        let slot = self.lower_element_slot(path, &selection)?;
        self.record_into_slot(path, selection.container, selection.depth(), slot, lhs, rhs)
            .map(Some)
    }

    /// `a[i] = v` from a record variable of the element's own type, leaf by
    /// leaf in place: no temporary and no string copy. `None` when a leaf
    /// needs more than a store: a dynamic array is the one kind whose element
    /// write creates nothing (`q[$+1]` appends, a missing key inserts), and
    /// members that are containers, fixed arrays or unions move as a whole.
    fn element_items_assignment(
        &mut self,
        path: &str,
        selection: &ElementSelection,
        rhs: NodeId,
    ) -> Result<Option<IrStmt>, String> {
        let container = selection.container;
        if self.element_nba_target.is_some()
            || self.model.containers[container].kind != IrContainerKind::Dynamic
            || !selection.side_effect_free(self)
        {
            return Ok(None);
        }
        let (_, descriptor, leaves) = self.element_leaves(container, selection.depth())?;
        if leaves.columns
            || !leaves.containers.is_empty()
            || !leaves.fixed.is_empty()
            || !leaf_record(&descriptor)
        {
            return Ok(None);
        }
        let Ok(Some((source, source_type))) = self.native_endpoint(rhs) else {
            return Ok(None);
        };
        if source_type != descriptor {
            return Ok(None);
        }
        let sources = self.endpoint_leaves(&source)?;
        let paired = leaves
            .scalars
            .iter()
            .map(|leaf| sources.iter().find(|(path, _)| *path == leaf.path))
            .collect::<Option<Vec<_>>>();
        let Some(paired) = paired.filter(|paired| paired.len() == sources.len()) else {
            return Ok(None);
        };
        let slot = self.lower_element_slot(path, selection)?;
        let mut items = Vec::with_capacity(paired.len());
        for (leaf, (_, source)) in leaves.scalars.iter().zip(paired) {
            let value = match self.endpoint_leaf_read(source)? {
                LeafValue::Packed(value) => IrElementItemValue::Packed(value),
                LeafValue::Real(value) => IrElementItemValue::Real(value),
                LeafValue::String(value) => IrElementItemValue::String(value),
                LeafValue::Chandle(value) => IrElementItemValue::Chandle(value),
                _ => {
                    return Err(format!(
                        "record element leaf has no scalar source in `{path}`"
                    ))
                }
            };
            items.push(IrElementItem {
                path: leaf.items.clone(),
                value,
            });
        }
        Ok(Some(IrStmt::Container(Box::new(
            IrContainerStmt::SetElementItems {
                container,
                slot,
                items,
            },
        ))))
    }

    /// The cell count and leaf of a fixed-array element of container
    /// storage whose cells are integral or real; `None` for other elements.
    fn fixed_element_row(&self, selection: &ElementSelection) -> Option<(u64, IrContainerElement)> {
        let IrContainerElement::FixedArray {
            dimensions,
            element,
        } = self.container_element_type(selection.container, selection.depth())?
        else {
            return None;
        };
        if !matches!(
            *element,
            IrContainerElement::Packed { .. } | IrContainerElement::Real { .. }
        ) {
            return None;
        }
        let total = dimensions
            .iter()
            .map(|(left, right)| i64::from(*left).abs_diff(i64::from(*right)) + 1)
            .product();
        Some((total, *element))
    }

    /// A whole unpacked-array element of string, handle or record cells
    /// copied to or from an array variable (`sr = sd[k]`, `sd[k] = sr`):
    /// the rows move only between arrays of integral or real cells.
    pub(in crate::sim::codegen) fn reject_native_row_copy(
        &self,
        path: &str,
        element: NodeId,
    ) -> Result<(), String> {
        let native = self.record_element_of(element).is_some_and(|selection| {
            matches!(
                self.container_element_type(selection.container, selection.depth()),
                Some(IrContainerElement::FixedArray { .. })
            ) && self.fixed_element_row(&selection).is_none()
        });
        if native {
            return Err(crate::sim::codegen::error::located(
                &self.source_location(element),
                format!(
                    "copying an unpacked-array element of strings, handles or records of a queue, dynamic or associative array to or from an array variable in `{path}` is not supported; copy it cell by cell"
                ),
            ));
        }
        Ok(())
    }

    /// The whole fixed-array variable `node` names, when its cells have the
    /// shape of a row of `total` cells of `leaf` (SV 7.6 requires
    /// equivalent element types and equal counts).
    fn row_variable(
        &mut self,
        path: &str,
        node: NodeId,
        total: u64,
        leaf: &IrContainerElement,
    ) -> Result<Option<usize>, String> {
        let Ok(view) = self.fixed_memory_view(path, node) else {
            return Ok(None);
        };
        let storage = &self.model.arrays[view.array];
        if view.origin != 0
            || !view.selectors.is_empty()
            || view.sliced
            || view.total != storage.total
        {
            return Err(crate::sim::codegen::error::located(
                &self.source_location(node),
                format!(
                    "copying a row or slice of a fixed-size array to or from an unpacked-array element of a queue, dynamic or associative array in `{path}` is not supported; copy a whole array variable"
                ),
            ));
        }
        let matches = match leaf {
            IrContainerElement::Packed { width, .. } => {
                !storage.real && !storage.is_net() && storage.elem_width == *width
            }
            IrContainerElement::Real { .. } => storage.real,
            _ => false,
        };
        if !matches || storage.total != total {
            return Err(crate::sim::codegen::error::located(
                &self.source_location(node),
                format!("array and unpacked-array element in `{path}` have different shapes"),
            ));
        }
        Ok(Some(view.array))
    }

    /// `row = da[k]`: every cell of an unpacked-array element of container
    /// storage, copied into the whole fixed-array variable `lhs` in one
    /// statement (SV 7.6). A missing element reads the Table 7-1 default.
    pub(in crate::sim::codegen) fn lower_row_from_element(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
        nba: bool,
    ) -> Result<Option<IrStmt>, String> {
        let source = self.p30_unwrap_cast(rhs);
        let Some(selection) = self.record_element_of(source) else {
            return Ok(None);
        };
        let Some((total, leaf)) = self.fixed_element_row(&selection) else {
            return Ok(None);
        };
        let Some(array) = self.row_variable(path, lhs, total, &leaf)? else {
            return Ok(None);
        };
        let root = self.element_row_root(path, &selection, false)?;
        Ok(Some(if self.model.arrays[array].real {
            IrStmt::Container(Box::new(IrContainerStmt::RealColumnAssign {
                array,
                source: None,
                root: Some(root),
                items: Vec::new(),
                nba,
            }))
        } else {
            IrStmt::FixedValueAssign {
                dst: self.fixed_view_at(array, &[]),
                src: Box::new(crate::sim::ir::IrFixedValue::ValueItems {
                    root,
                    items: Vec::new(),
                    array,
                }),
                nba,
            }
        }))
    }

    /// `da[k] = row`: the whole fixed-array variable `rhs` stored into the
    /// unpacked-array element `selection` selects, cell for cell in one
    /// statement (SV 7.6).
    fn lower_element_from_row(
        &mut self,
        path: &str,
        selection: &ElementSelection,
        rhs: NodeId,
    ) -> Result<Option<IrStmt>, String> {
        if self.element_nba_target.is_some() {
            return Ok(None);
        }
        let source = self.p30_unwrap_cast(rhs);
        let Some((total, leaf)) = self.fixed_element_row(selection) else {
            if matches!(
                self.container_element_type(selection.container, selection.depth()),
                Some(IrContainerElement::FixedArray { .. })
            ) && self.container_of(source).is_some()
            {
                return Err(crate::sim::codegen::error::located(
                    &self.source_location(source),
                    format!(
                        "copying an unpacked-array element of strings, handles or records of a queue, dynamic or associative array to or from an array variable in `{path}` is not supported; copy it cell by cell"
                    ),
                ));
            }
            return Ok(None);
        };
        let Some(array) = self.row_variable(path, source, total, &leaf)? else {
            return Ok(None);
        };
        let root = self.element_row_root(path, selection, true)?;
        Ok(Some(IrStmt::Container(Box::new(
            IrContainerStmt::FixedToValueItem {
                array,
                root,
                items: Vec::new(),
            },
        ))))
    }

    /// The whole fixed-array variable `source` into native value `value`
    /// of an unpacked-array type with integral or real cells.
    pub(in crate::sim::codegen) fn native_row_from_array(
        &mut self,
        path: &str,
        value: usize,
        source: NodeId,
    ) -> Result<Option<IrStmt>, String> {
        let ty = self.model.native_values[value].ty;
        let IrContainerElement::FixedArray {
            dimensions,
            element,
        } = self.model.native_types[ty].clone()
        else {
            return Ok(None);
        };
        if !matches!(
            *element,
            IrContainerElement::Packed { .. } | IrContainerElement::Real { .. }
        ) {
            return Ok(None);
        }
        let total = dimensions
            .iter()
            .map(|(left, right)| i64::from(*left).abs_diff(i64::from(*right)) + 1)
            .product();
        let Some(array) = self.row_variable(path, source, total, &element)? else {
            return Ok(None);
        };
        Ok(Some(IrStmt::Container(Box::new(
            IrContainerStmt::FixedToValueItem {
                array,
                root: IrValueItemRoot::Value(value),
                items: Vec::new(),
            },
        ))))
    }

    /// The locator of a whole element as a transfer root; a `write` locator
    /// creates a missing associative entry and publishes the store.
    fn element_row_root(
        &mut self,
        path: &str,
        selection: &ElementSelection,
        write: bool,
    ) -> Result<IrValueItemRoot, String> {
        let IrValueSlot::Element { indices, key } = self.lower_element_slot(path, selection)?
        else {
            unreachable!("element selections lower to element slots");
        };
        Ok(IrValueItemRoot::Element(IrChandleExpr::ContainerElement {
            container: selection.container,
            indices,
            key: key.map(Box::new),
            write,
        }))
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
        let (element, prefix) = match self.element_subrecord(source) {
            Some((element, _, prefix, _)) => (element, prefix),
            None => (source, Vec::new()),
        };
        let Some((temporary, mut statements)) = self.element_copy(path, element)? else {
            return Ok(None);
        };
        let endpoint = NativeEndpoint::Value {
            value: temporary,
            prefix,
        };
        statements.push(self.native_transfer(path, target, &endpoint, nba)?);
        Ok(Some(IrStmt::Block(statements)))
    }

    /// Copy a whole record element, a sub-record of one or the record removed
    /// by a pop into the columns of `value`; `None` for other sources.
    pub(in crate::sim::codegen::lowering) fn element_into_record_value(
        &mut self,
        path: &str,
        value: &RecordValue,
        source: NodeId,
        nba: bool,
    ) -> Result<Option<IrStmt>, String> {
        let (element, prefix) = match self.element_subrecord(source) {
            Some((element, _, prefix, _)) => (element, prefix),
            None => (source, Vec::new()),
        };
        let Some((temporary, mut statements)) = self.element_copy(path, element)? else {
            return Ok(None);
        };
        let endpoint = NativeEndpoint::Value {
            value: temporary,
            prefix,
        };
        statements.push(self.endpoint_to_record_value(path, value, &endpoint, nba)?);
        Ok(Some(IrStmt::Block(statements)))
    }

    /// The element, selection and member path of `node` when it names a whole
    /// sub-record or member array inside a record element.
    fn element_subrecord(
        &self,
        node: NodeId,
    ) -> Option<(
        NodeId,
        ElementSelection,
        Vec<AggregatePathPart>,
        TypeDescriptor,
    )> {
        let (element, selection, member) = self.element_member_parts(node)?;
        let descriptor = self.query_descriptor(node)?.clone();
        matches!(
            &descriptor.shape,
            TypeShape::FixedArray { .. }
                | TypeShape::Aggregate(AggregateLayout {
                    kind: AggregateKind::UnpackedStruct,
                    ..
                })
        )
        .then_some((element, selection, member, descriptor))
    }

    /// `q[i].m = v` and `v = q[i].m` for a sub-record: the element moves
    /// through a temporary, whose member is an ordinary record endpoint. The
    /// indices of a written element are evaluated once.
    pub(super) fn element_subrecord_assignment(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
        nba: bool,
    ) -> Result<Option<IrStmt>, String> {
        if let Some((element, selection, prefix, descriptor)) = self.element_subrecord(lhs) {
            if nba {
                return self
                    .lower_element_member_nba(path, lhs, rhs, None)
                    .map(Some);
            }
            let mut statements = Vec::new();
            let slot = self.element_nba_slot(path, &selection, &mut statements)?;
            let (container, depth) = (selection.container, selection.depth());
            let temporary = self.element_temporary(container, depth, element)?;
            statements.push(IrStmt::NativeValueDeclare(temporary));
            statements.push(IrStmt::Container(Box::new(IrContainerStmt::GetValue {
                container,
                slot: slot.clone(),
                value: temporary,
            })));
            statements.extend(self.element_items_to_companions(temporary)?);
            let target = NativeEndpoint::Value {
                value: temporary,
                prefix,
            };
            statements.push(self.native_assign_into(path, &target, &descriptor, rhs, false)?);
            statements.extend(self.companions_to_element_items(temporary)?);
            statements.push(IrStmt::Container(Box::new(IrContainerStmt::SetValue {
                container,
                slot,
                value: temporary,
            })));
            return Ok(Some(IrStmt::Block(statements)));
        }
        let source = self.p30_unwrap_cast(rhs);
        if self.element_subrecord(source).is_none() {
            return Ok(None);
        }
        let Some((target, _)) = self.native_endpoint(lhs)? else {
            return Ok(None);
        };
        self.container_record_into(path, &target, source, nba)
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
            || !matches!(
                self.kind(destination),
                NodeKind::Expr(ExprKind::Ref { .. }) | NodeKind::Var { .. }
            )
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
                self.db.array_select_path(node).is_some_and(|(owner, _)| {
                    self.record_element_of(owner).is_some() || self.names_element_member(owner)
                }) || self.element_cell_select(node).is_some()
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
                let members = parts
                    .iter()
                    .skip(1)
                    .filter(|part| !part.is_empty())
                    .map(|part| AggregatePathPart::Member(part.clone()));
                // The first step may itself be an element of a member array
                // of a record element (`q[i].a[1].n`).
                if let Some(selection) = self.record_element_of(first) {
                    let path = members.collect::<Vec<_>>();
                    return (!path.is_empty()).then_some((first, selection, path));
                }
                let (element, selection, mut path) = self.element_member_parts(first)?;
                path.extend(members);
                Some((element, selection, path))
            }
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                if let Some((element, selection, trailing)) = self.element_cell_select(node) {
                    let path = trailing
                        .iter()
                        .map(|index| {
                            Some(AggregatePathPart::Index(
                                i32::try_from(self.eval_bound_i128(*index).ok()?).ok()?,
                            ))
                        })
                        .collect::<Option<Vec<_>>>()?;
                    return Some((element, selection, path));
                }
                // A member array is named by a detached array node; the
                // captured select path gives its owner: the record element,
                // or an element of a member array of it (`q[0].e[1].a[2]`).
                let (element, selection, mut path) = match self.db.array_select_path(node) {
                    Some((owner, members)) => {
                        let (element, selection, mut path) = match self.record_element_of(owner) {
                            Some(selection) => (owner, selection, Vec::new()),
                            None => self.element_member_parts(owner)?,
                        };
                        path.extend(members.iter().cloned().map(AggregatePathPart::Member));
                        (element, selection, path)
                    }
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
        if let Some(leaf) = fixed_item_leaf(&leaves.fixed, &path) {
            return Ok(Some((selection, leaf)));
        }
        if leaves
            .fixed
            .iter()
            .any(|leaf| leaf.path.starts_with(&path) || path.starts_with(&leaf.path))
        {
            return Ok(None);
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
        let mut scalars = leaves
            .scalars
            .into_iter()
            .filter(|leaf| leaf.path.starts_with(&prefix))
            .collect::<Vec<_>>();
        for fixed in &leaves.fixed {
            if fixed.path.starts_with(&prefix) || prefix.starts_with(&fixed.path) {
                scalars.extend(
                    fixed_leaf_items(fixed)?
                        .into_iter()
                        .filter(|leaf| leaf.path.starts_with(&prefix)),
                );
            }
        }
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
        // The owner is the record element or an element of a member array
        // of it (`q[0].e[1].a[2][0]`).
        let (selection, mut path) = match self.record_element_of(owner) {
            Some(selection) => (selection, Vec::new()),
            None => match self.element_member_parts(owner) {
                Some((_, selection, path)) => (selection, path),
                None => return Ok(None),
            },
        };
        path.extend(members.iter().cloned().map(AggregatePathPart::Member));
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
        if !leaves.fixed.is_empty() {
            return Ok(None);
        }
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
        if leaves.containers.is_empty() && leaves.fixed.is_empty() {
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
            // An associative array has no direct element path either.
            if self.selects_associative(&selection) {
                if let ElementSelector::Indices(indices) = &selection.selector {
                    for index in indices.clone() {
                        self.collect_staged_members(index, false, out, nested);
                    }
                }
                nested.push((node, selection, mutated));
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
                                ExprKind::ArraySelect { .. }
                                    | ExprKind::BitSelect { .. }
                                    | ExprKind::HierPath { .. }
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
                // method runs on its alias (SV 7.5.1, 7.10.2, 7.12).
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
            // A selector that calls a function in a lazy operand must not run
            // when the operand is skipped: the operand stages it.
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::LogicalAnd | Operation::LogicalOr | Operation::Conditional,
                operands,
                ..
            }) => {
                let lazy = operands.get(1..).unwrap_or_default().to_vec();
                for child in children {
                    if !lazy.contains(&child) {
                        self.collect_staged_members(child, false, out, nested);
                        continue;
                    }
                    // What an operand does not name the operands in it do not.
                    let outer = self.no_called_selectors.get();
                    if !outer && self.names_called_selector(child) {
                        self.deferred_selectors.set(true);
                        continue;
                    }
                    self.no_called_selectors.set(true);
                    self.collect_staged_members(child, false, out, nested);
                    self.no_called_selectors.set(outer);
                }
            }
            // A member passed to an output, inout or ref formal is written
            // after the call. A nested element passed whole to an
            // input formal is read once.
            NodeKind::FuncCall {
                callee, is_task, ..
            } => {
                let is_task = *is_task;
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
                    // formal: the call copies the alias in and out. A `ref`
                    // formal takes only an associative alias; the actual of
                    // any other rejects where the call is lowered.
                    let by_reference = direction == Some(DbDirection::Ref);
                    if matches!(direction, Some(DbDirection::Output | DbDirection::Inout))
                        || by_reference
                    {
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
                            let live =
                                by_reference && is_task && !self.selects_associative(&selection);
                            if let (ElementSelector::Indices(indices), false) =
                                (&selection.selector, live)
                            {
                                for index in indices.clone() {
                                    self.collect_staged_members(index, false, out, nested);
                                }
                                nested.push((actual, selection, true));
                                continue;
                            }
                        }
                    }
                    // An input actual is only read: the callee may change
                    // the array through another name.
                    let changes = direction != Some(DbDirection::Input);
                    self.collect_staged_members(child, changes, out, nested);
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
            // A nested element printed whole (`%p`) is read through its
            // alias. A report formatted after its statement (`$strobe`,
            // `$monitor`) has no statement to bind one.
            NodeKind::SysCall { name }
                if crate::sim::codegen::lowering::statements::immediate_display_task(name) =>
            {
                for child in children {
                    if !self.stage_whole_read(child, out, nested) {
                        self.collect_staged_members(child, false, out, nested);
                    }
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
    /// `x = q[i].m[j]`, `q[i].m = v`) in lexical containers: each is an
    /// alias of the member's own storage, bound before the statement, so
    /// container operations work on it unchanged, and a member the statement
    /// can change publishes to its container after it. Returns the
    /// statements to run before and after `h`; the caller ends the staging
    /// with `end_element_member_staging`.
    pub(in crate::sim::codegen) fn stage_element_members(
        &mut self,
        path: &str,
        h: NodeId,
    ) -> Result<(Vec<IrStmt>, Vec<IrStmt>), String> {
        let mut members = Vec::new();
        let mut nested = Vec::new();
        self.collect_statement_members(h, &mut members, &mut nested);
        if matches!(
            self.kind(h),
            NodeKind::Stmt(StmtKind::Assign {
                blocking: false,
                ..
            })
        ) && (members.iter().any(|member| member.mutated)
            || nested.iter().any(|(_, _, mutated)| *mutated))
        {
            return Err(format!(
                "nonblocking assignment to a queue or dynamic-array member of a container record element in `{path}` is not supported"
            ));
        }
        let (mut before, mut after) = self.build_element_staging(path, members, nested)?;
        self.stage_below_aliases(path, &mut before, &mut after, StageRoot::Statement(h))?;
        Ok((before, after))
    }

    /// Collect what statement `h` stages.
    fn collect_statement_members(
        &self,
        h: NodeId,
        members: &mut Vec<StagedMember>,
        nested: &mut Vec<(NodeId, ElementSelection, bool)>,
    ) {
        match self.kind(h) {
            NodeKind::Stmt(StmtKind::Assign { blocking, .. }) => {
                let blocking = *blocking;
                let children = self.node(h).children.clone();
                for (position, child) in children.into_iter().enumerate() {
                    // A hoisted value already ran with its own staging.
                    if position == 1 && self.hoisted_source == Some(child) {
                        continue;
                    }
                    // A nonblocking write of a whole member is queued from
                    // the source, without an alias of the target.
                    if position == 0 && !blocking && self.element_container_member(child) {
                        continue;
                    }
                    // A nested element assigned whole (`row = qq[i]`) is read
                    // from its alias.
                    if position == 1 && self.stage_whole_read(child, members, nested) {
                        continue;
                    }
                    self.collect_staged_members(child, position == 0, members, nested);
                }
            }
            _ => self.collect_staged_members(h, false, members, nested),
        }
    }

    /// Bind the nested elements reached through a bound array
    /// (`a3["a"]["b"][1]`, `aq["k"][2].push_back(x)`): each level only
    /// resolves once the level above it has its alias.
    fn stage_below_aliases(
        &mut self,
        path: &str,
        before: &mut Vec<IrStmt>,
        after: &mut Vec<IrStmt>,
        root: StageRoot,
    ) -> Result<(), String> {
        loop {
            if self.staged_nested_elements.is_empty() {
                return Ok(());
            }
            let (mut members, mut nested) = (Vec::new(), Vec::new());
            match root {
                StageRoot::Statement(h) => {
                    self.collect_statement_members(h, &mut members, &mut nested);
                }
                StageRoot::Expression(node) => {
                    self.collect_staged_members(node, false, &mut members, &mut nested)
                }
                StageRoot::Initializer(node) => {
                    if !self.stage_whole_read(node, &mut members, &mut nested) {
                        self.collect_staged_members(node, false, &mut members, &mut nested);
                    }
                }
            }
            nested.retain(|(node, _, _)| self.staged_nested_element(*node).is_none());
            if nested.is_empty() {
                return Ok(());
            }
            self.stage_nested_elements(path, nested, before, after)?;
        }
    }

    /// Stage the members and nested elements a container initializer reads;
    /// nothing is published. The caller ends the staging with
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
        let (mut before, mut after) = self.build_element_staging(path, members, nested)?;
        self.stage_below_aliases(path, &mut before, &mut after, StageRoot::Initializer(node))?;
        debug_assert!(after.is_empty(), "an initializer writes back nothing");
        Ok(before)
    }

    /// Whether the declaration initializer `initializer` reads an element of
    /// a nested container (`nas["x"][1]`) or a container member of an element,
    /// which must be staged around it (see [`Self::stage_initializer`]),
    /// outside any other staging.
    pub(in crate::sim::codegen) fn initializer_stages(&self, initializer: NodeId) -> bool {
        if !self.element_staging_possible
            || !self.staged_element_members.is_empty()
            || !self.staged_nested_elements.is_empty()
        {
            return false;
        }
        let (mut members, mut nested) = (Vec::new(), Vec::new());
        if !self.stage_whole_read(initializer, &mut members, &mut nested) {
            self.collect_staged_members(initializer, false, &mut members, &mut nested);
        }
        !members.is_empty() || !nested.is_empty()
    }

    /// Whether `declaration` is an unpacked record or union whose initializer
    /// is a whole nested container element ([`Self::initializer_stages`]).
    pub(in crate::sim::codegen) fn record_initializer_stages(
        &self,
        declaration: NodeId,
        initializer: NodeId,
    ) -> bool {
        matches!(
            self.query_descriptor(declaration).map(|descriptor| &descriptor.shape),
            Some(TypeShape::Aggregate(layout))
                if matches!(layout.kind, AggregateKind::UnpackedStruct | AggregateKind::UnpackedUnion)
        ) && self.initializer_stages(initializer)
    }

    /// The statements `lower` gives for a record declaration initializer
    /// `initializer`, with the nested container elements and element members
    /// it reads staged around them as an assignment statement stages its
    /// source (`rec_t z = nas["x"][1];`). Without anything to stage, or inside
    /// another staging, the statements are `lower`'s alone.
    pub(in crate::sim::codegen) fn lower_staged_record_initializer(
        &mut self,
        path: &str,
        initializer: NodeId,
        lower: impl FnOnce(&mut Self) -> Result<Vec<IrStmt>, String>,
    ) -> Result<Vec<IrStmt>, String> {
        if !self.initializer_stages(initializer) {
            return lower(self);
        }
        let lowered = self
            .stage_initializer(path, initializer)
            .and_then(|before| Ok((before, lower(self)?)));
        self.end_element_member_staging();
        let (mut before, body) = lowered?;
        if before.is_empty() {
            return Ok(body);
        }
        before.extend(body);
        Ok(vec![IrStmt::Block(before)])
    }

    /// Stage the queue and dynamic-array members of container record
    /// elements (and queues or dynamic arrays nested in container elements)
    /// that the expression `node` reads: a condition, loop bound,
    /// declaration initializer or the value of a timed assignment. Each is
    /// bound before every evaluation and, when the expression changes it,
    /// published after it. Returns those statements, or `None` when
    /// `node` names none. The caller lowers `node` and then ends the staging
    /// with `end_element_member_staging`.
    pub(in crate::sim::codegen) fn stage_expression_members(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<StagingStatements>, String> {
        if !self.element_staging_possible {
            return Ok(None);
        }
        // Inside a staged statement only a selector it left to its lazy
        // operand is staged here.
        let inner =
            !self.staged_element_members.is_empty() || !self.staged_nested_elements.is_empty();
        if inner && !self.deferred_selectors.get() {
            return Ok(None);
        }
        let (mut members, mut nested) = (Vec::new(), Vec::new());
        self.collect_staged_members(node, false, &mut members, &mut nested);
        if inner {
            members.clear();
            nested.retain(|(element, _, _)| {
                self.staged_nested_element(*element).is_none() && !self.side_effect_free(*element)
            });
        }
        if members.is_empty() && nested.is_empty() {
            return Ok(None);
        }
        // A skipped operand must not locate its element as a write target
        // (that would create a missing associative entry), locate a missing
        // array or call a selector: each operand stages its own.
        if (members.iter().any(|member| member.mutated)
            || nested.iter().any(|(element, selection, mutated)| {
                *mutated || self.selects_associative(selection) || !self.side_effect_free(*element)
            }))
            && self.lazily_evaluated_operands(node)
        {
            return Ok(None);
        }
        let outer: std::collections::HashSet<NodeId> =
            self.staged_nested_elements.keys().copied().collect();
        let (mut before, mut after) = self.build_element_staging(path, members, nested)?;
        self.stage_below_aliases(path, &mut before, &mut after, StageRoot::Expression(node))?;
        if inner {
            let added = self
                .staged_nested_elements
                .keys()
                .copied()
                .filter(|node| !outer.contains(node))
                .collect();
            self.inner_stagings.push(added);
        }
        Ok(Some((before, after)))
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

    /// The statements binding `members` and `nested` and the ones publishing
    /// the changed ones.
    fn build_element_staging(
        &mut self,
        path: &str,
        members: Vec<StagedMember>,
        nested: Vec<(NodeId, ElementSelection, bool)>,
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
            // The member is used in place: the element is located once.
            before.push(IrStmt::Container(Box::new(IrContainerStmt::BindElement {
                root: locator(self, member.mutated)?,
                items: leaf.items.clone(),
                container: ir,
            })));
            if member.mutated {
                after.push(IrStmt::Container(Box::new(IrContainerStmt::TouchContents(
                    selection.container,
                ))));
            }
        }
        self.stage_nested_elements(path, nested, &mut before, &mut after)?;
        Ok((before, after))
    }

    /// Bind the queues, dynamic and associative arrays nested in container
    /// elements that a statement names whole (`qq[i].push_back(x)`,
    /// `sq[k] = src`, `x = sq[k][j]`) to lexical aliases of the nested type,
    /// as `stage_element_members` does for record members: the statement
    /// works on the element's own storage, and a changed element publishes
    /// to its container after the statement. Each element is located once,
    /// so its selector may have side effects and two aliases may name one
    /// element. A changed element is located as a write target, which
    /// creates a missing associative entry (SV 7.8.6); an invalid index
    /// binds an empty array that is dropped (SV 7.4.6, 7.10.1).
    fn stage_nested_elements(
        &mut self,
        path: &str,
        nested: Vec<(NodeId, ElementSelection, bool)>,
        before: &mut Vec<IrStmt>,
        after: &mut Vec<IrStmt>,
    ) -> Result<(), String> {
        let mut groups: Vec<StagedNested> = Vec::new();
        for (node, selection, mutated) in nested {
            if let Some(group) = groups
                .iter_mut()
                .find(|group| self.same_element_selection(&group.selection, &selection))
            {
                group.nodes.push(node);
                group.mutated |= mutated;
                continue;
            }
            groups.push(StagedNested {
                nodes: vec![node],
                selection,
                mutated,
            });
        }
        for group in groups {
            let selection = &group.selection;
            let depth = selection.depth();
            let nested = self.container_element_type(selection.container, depth);
            let Some((IrContainerElement::Container { element, .. }, kind)) = nested
                .as_ref()
                .and_then(|nested| Some((nested.clone(), nested.nested_kind()?)))
            else {
                return Err(format!(
                    "nested container element in `{path}` does not select a nested container"
                ));
            };
            // The nested type gives the element type of record operations
            // on the alias.
            let descriptor = self
                .container_element_descriptor(selection.container, depth)
                .or_else(|| self.query_descriptor(group.nodes[0]).cloned());
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
            // The array is used in place: a copy would cost its size at
            // every statement and lose a change made through another name.
            before.push(IrStmt::Container(Box::new(IrContainerStmt::BindElement {
                root: locator(self, group.mutated)?,
                items: Vec::new(),
                container: ir,
            })));
            if group.mutated {
                after.push(IrStmt::Container(Box::new(IrContainerStmt::TouchContents(
                    selection.container,
                ))));
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

    /// Whether `node` names a nested container element whole.
    pub(in crate::sim::codegen) fn is_nested_element(&self, node: NodeId) -> bool {
        self.nested_element_selection(node).is_some()
    }

    /// Whether `node` holds a nested array selected by a call.
    fn names_called_selector(&self, node: NodeId) -> bool {
        crate::work::count(crate::work::Work::SelectorNodes, 1);
        (self.nested_element_selection(node).is_some() && !self.side_effect_free(node))
            || self
                .node(node)
                .children
                .iter()
                .any(|child| self.names_called_selector(*child))
    }

    /// Evaluate the value of a plain blocking write through a nested associative
    /// array before the array is bound, so that a call in the value that
    /// deletes the element is seen (the write then creates it again). Returns
    /// the statements to run first and registers the value; the caller ends
    /// it with `end_hoisted_source`.
    pub(in crate::sim::codegen) fn hoist_assignment_source(
        &mut self,
        path: &str,
        h: NodeId,
    ) -> Result<Vec<IrStmt>, String> {
        let NodeKind::Stmt(StmtKind::Assign {
            blocking: true,
            op: Operation::Assignment,
            delay: None,
        }) = self.kind(h)
        else {
            return Ok(Vec::new());
        };
        let [lhs, rhs] = self.node(h).children.as_slice() else {
            return Ok(Vec::new());
        };
        let (lhs, rhs) = (*lhs, *rhs);
        if self.side_effect_free(rhs) {
            return Ok(Vec::new());
        }
        let (mut members, mut nested) = (Vec::new(), Vec::new());
        self.collect_staged_members(lhs, true, &mut members, &mut nested);
        if nested.is_empty() {
            return Ok(Vec::new());
        }
        let name = format!("_llg_rhs{}", rhs.0);
        let mut statements = Vec::new();
        let value = match self
            .query_descriptor(lhs)
            .map(|descriptor| &descriptor.shape)
        {
            // A string member keeps the element it was bound to when its
            // value is a conditional, a conversion or a system call.
            Some(TypeShape::String) if self.string_member_binds_first(lhs, rhs) => {
                return Ok(Vec::new());
            }
            Some(TypeShape::String) => {
                statements.push(IrStmt::DeclString {
                    name: name.clone(),
                    init: Some(self.lower_string(path, rhs)?),
                });
                HoistedValue::String(name)
            }
            Some(TypeShape::Opaque { .. }) => {
                let value = self.lower_chandle(path, rhs)?;
                statements.push(IrStmt::Object(Box::new(IrObjectStmt::ChandleDeclareLocal(
                    name.clone(),
                    Some(value),
                ))));
                HoistedValue::Handle(name)
            }
            // Only a whole element takes the value; a sub-record is written
            // through the element's bound copy.
            Some(TypeShape::Aggregate(_))
                if self
                    .selection_record_depth(&nested)
                    .is_some_and(|(container, depth)| {
                        matches!(self.kind(lhs), NodeKind::Expr(ExprKind::ArraySelect { .. }))
                            && self
                                .container_element_descriptor(container, depth)
                                .zip(self.query_descriptor(lhs))
                                .is_some_and(|(element, target)| element.id == target.id)
                    }) =>
            {
                let Some((container, depth)) = self.selection_record_depth(&nested) else {
                    return Ok(Vec::new());
                };
                let temporary = self.element_temporary(container, depth, rhs)?;
                statements.extend(self.record_value_into(path, temporary, rhs)?);
                HoistedValue::Record(temporary)
            }
            Some(TypeShape::Aggregate(_)) if self.selection_holds_packed(&nested) => {
                let value = self.lower_expr(path, rhs)?;
                let (width, signed) = (value.width, value.signed);
                statements.push(IrStmt::DeclLocal {
                    name: name.clone(),
                    width,
                    signed,
                    two_state: false,
                    init: Some(Box::new(value)),
                });
                HoistedValue::Packed(name, width, signed)
            }
            Some(TypeShape::Aggregate(_) | TypeShape::FixedArray { .. }) => {
                return Ok(Vec::new());
            }
            Some(TypeShape::Container { .. }) | None => return Ok(Vec::new()),
            // A packed or real member keeps the element it was bound to,
            // except a real value an implicit conversion made.
            Some(shape)
                if matches!(
                    self.kind(lhs),
                    NodeKind::Expr(ExprKind::MemberSelect { .. } | ExprKind::HierPath { .. })
                ) && !(matches!(shape, TypeShape::Real { .. })
                    && self.db.is_implicit_conversion(rhs)) =>
            {
                return Ok(Vec::new());
            }
            Some(_) => {
                let value = self.lower_expr(path, rhs)?;
                let (width, signed) = (value.width, value.signed);
                statements.push(IrStmt::DeclLocal {
                    name: name.clone(),
                    width,
                    signed,
                    two_state: false,
                    init: Some(Box::new(value)),
                });
                HoistedValue::Packed(name, width, signed)
            }
        };
        self.hoisted_values.insert(rhs, value.clone());
        self.hoisted_values.insert(self.p30_unwrap_cast(rhs), value);
        self.hoisted_source = Some(rhs);
        Ok(statements)
    }

    fn string_member_binds_first(&self, lhs: NodeId, rhs: NodeId) -> bool {
        let member = matches!(
            self.kind(lhs),
            NodeKind::Expr(ExprKind::MemberSelect { .. } | ExprKind::HierPath { .. })
        );
        let mut source = rhs;
        while let NodeKind::Expr(ExprKind::Cast { operand, .. }) = self.kind(source) {
            if !self.db.is_implicit_conversion(source) {
                return member;
            }
            source = *operand;
        }
        member
            && matches!(
                self.kind(source),
                NodeKind::SysCall { .. }
                    | NodeKind::Expr(
                        ExprKind::Conditional { .. }
                            | ExprKind::Operation {
                                op: Operation::Conditional,
                                ..
                            }
                    )
            )
    }

    /// Container and depth of the record element reached through the first
    /// of `nested`, when it holds records.
    fn selection_record_depth(
        &self,
        nested: &[(NodeId, ElementSelection, bool)],
    ) -> Option<(usize, usize)> {
        let (_, selection, _) = nested.first()?;
        let mut depth = selection.depth();
        while matches!(
            self.container_element_type(selection.container, depth),
            Some(IrContainerElement::Container { .. })
        ) {
            depth += 1;
        }
        self.element_leaves(selection.container, depth)
            .is_ok()
            .then_some((selection.container, depth))
    }

    /// Whether the first of `nested` holds packed elements.
    fn selection_holds_packed(&self, nested: &[(NodeId, ElementSelection, bool)]) -> bool {
        let Some((_, selection, _)) = nested.first() else {
            return false;
        };
        let mut depth = selection.depth();
        while matches!(
            self.container_element_type(selection.container, depth),
            Some(IrContainerElement::Container { .. })
        ) {
            depth += 1;
        }
        matches!(
            self.container_element_type(selection.container, depth),
            Some(IrContainerElement::Packed { .. })
        )
    }

    /// Declare record `temporary` and build `rhs` into it, with the staging
    /// the value needs for itself.
    fn record_value_into(
        &mut self,
        path: &str,
        temporary: usize,
        rhs: NodeId,
    ) -> Result<Vec<IrStmt>, String> {
        let descriptor = self.native_layout_of_value(temporary)?.descriptor.clone();
        let endpoint = NativeEndpoint::Value {
            value: temporary,
            prefix: Vec::new(),
        };
        let staged = self.stage_expression_members(path, rhs)?;
        let fill = self.native_assign_into(path, &endpoint, &descriptor, rhs, false);
        let (before, after) = match staged {
            Some(statements) => {
                self.end_element_member_staging();
                statements
            }
            None => Default::default(),
        };
        let mut statements = vec![IrStmt::NativeValueDeclare(temporary)];
        statements.extend(before);
        statements.push(fill?);
        statements.extend(after);
        statements.extend(self.companions_to_element_items(temporary)?);
        Ok(statements)
    }

    /// The hoisted value of `node`, if any.
    pub(in crate::sim::codegen) fn hoisted_value(&self, node: NodeId) -> Option<&HoistedValue> {
        if self.hoisted_values.is_empty() {
            return None;
        }
        self.hoisted_values.get(&node)
    }

    /// Forget the value `hoist_assignment_source` registered.
    pub(in crate::sim::codegen) fn end_hoisted_source(&mut self) {
        if let Some(rhs) = self.hoisted_source.take() {
            let inner = self.p30_unwrap_cast(rhs);
            for node in [rhs, inner] {
                self.hoisted_values.remove(&node);
            }
        }
    }

    /// Whether `node` names a queue or dynamic array nested in an element:
    /// an alias a statement binds, which no `ref` formal takes
    /// (docs/known_issues.md).
    pub(in crate::sim::codegen) fn names_nested_sequence(&self, node: NodeId) -> bool {
        if let Some(staged) = self.staged_nested_element(node) {
            return !matches!(
                self.model.containers[staged].kind,
                IrContainerKind::Associative { .. }
            );
        }
        self.nested_element_selection(node)
            .is_some_and(|selection| !self.selects_associative(&selection))
    }

    /// Whether `selection` names an associative array, which has no element
    /// path of its own: every use binds it.
    fn selects_associative(&self, selection: &ElementSelection) -> bool {
        matches!(
            self.container_element_type(selection.container, selection.depth()),
            Some(IrContainerElement::Container { key: Some(_), .. })
        )
    }

    /// A queue, dynamic or associative array nested in an element of
    /// container storage (`qq[i]`, `qd[i][j]`, `sq[k]`), named whole.
    fn nested_element_selection(&self, node: NodeId) -> Option<ElementSelection> {
        if self.model.containers.is_empty() {
            return None;
        }
        let nested = |element: Option<IrContainerElement>| {
            matches!(element, Some(IrContainerElement::Container { .. }))
        };
        // `a3["a"]["b"]`: a key of a bound nested array.
        if let Some((staged, tail)) = self.select_below_staged_element(node) {
            if let (
                [key],
                IrContainerKind::Associative {
                    key: IrAssocKey::String,
                },
            ) = (tail.as_slice(), &self.model.containers[staged].kind)
            {
                return nested(Some(self.model.containers[staged].element.clone())).then_some(
                    ElementSelection {
                        container: staged,
                        selector: ElementSelector::Key(*key),
                    },
                );
            }
        }
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

    /// The lexical alias of the nested element `node` names.
    pub(in crate::sim::codegen) fn staged_nested_element(&self, node: NodeId) -> Option<usize> {
        if self.staged_nested_elements.is_empty() {
            return None;
        }
        self.staged_nested_elements.get(&node).copied()
    }

    /// End the staging that `stage_element_members` opened.
    pub(in crate::sim::codegen) fn end_element_member_staging(&mut self) {
        if let Some(added) = self.inner_stagings.pop() {
            for node in added {
                self.staged_nested_elements.remove(&node);
            }
            return;
        }
        self.deferred_selectors.set(false);
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
        let symbol = match self.element_leaf_symbol(path, node, true)? {
            Some(symbol) => Some(symbol),
            None => self.native_member_select_target(path, node)?,
        };
        let Some((name, ty)) = symbol else {
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
