//! Descriptor-backed native aggregate subroutine storage (SIM-003).
//!
//! An unpacked record without a packed payload (it has string, chandle or
//! real leaves, or is wider than packed transport) is one runtime value with
//! its own recursive descriptor when it is a subroutine formal, result or
//! local. Module records keep their declaration-owned leaf storage; values
//! cross between the two representations leaf by leaf, never through packed
//! vectors or `IrFixedValue`.
use super::super::expressions::AggregateSelection;
use super::aggregates::NATIVE_TAG_MEMBER;
use super::*;
use crate::sim::ir::{
    is_class_handle_kind, native_item_count, validate_native_type, IrNativeAccessKind,
    IrNativeLeafExpr, IrNativeLeafValue, IrNativeValue, IrObjectQuery,
};

mod conditionals;
mod elements;
mod member_select;
mod tagged;
pub(in crate::sim::codegen) use tagged::NativeTaggedRoot;

/// Largest number of leaves of one native record. Leaf-wise transfers to and
/// from flattened module storage emit one operation per leaf, so larger
/// records are rejected rather than expanded.
pub(in super::super) const NATIVE_VALUE_MAX_LEAVES: usize = 4096;

/// A native-result call: call node, callee name, resolved callee and function.
type NativeResultCallee = (NodeId, String, Option<NodeId>, NodeId);

/// One side of a leaf-wise native transfer.
#[derive(Clone)]
pub(in super::super) enum NativeEndpoint {
    /// Descriptor-backed value with an item-path prefix.
    Value {
        value: usize,
        prefix: Vec<AggregatePathPart>,
    },
    /// Declaration-owned module record leaves below a path prefix.
    Module(Box<AggregateSelection>),
}

/// Scalar leaf values of an input operand and the source storage of its
/// container members, in companion order.
type NativeInputLeaves = (Vec<IrNativeLeafValue>, Vec<usize>);
/// Every leaf value of a record operand with its path, in declaration order.
type LeafReads = Vec<(Vec<AggregatePathPart>, LeafValue)>;

/// One captured leaf value of a transfer.
#[derive(Clone)]
enum LeafValue {
    Packed(IrExpr),
    Real(IrExpr),
    String(IrStringExpr),
    Chandle(IrChandleExpr),
    /// A queue, dynamic or associative member's own storage (SIM-007),
    /// read in place: transfers copy it after every scalar leaf is captured.
    Container(usize),
}

/// Diagnostic for a container member where only scalar leaves can travel.
const CONTAINER_LEAF_UNSUPPORTED: &str =
    "a record with a queue, dynamic or associative member is not supported in this context";

impl LeafValue {
    fn into_leaf_expr(self) -> Result<IrNativeLeafExpr, String> {
        Ok(match self {
            LeafValue::Packed(value) => IrNativeLeafExpr::Packed(value),
            LeafValue::Real(value) => IrNativeLeafExpr::Real(value),
            LeafValue::String(value) => IrNativeLeafExpr::String(value),
            LeafValue::Chandle(value) => IrNativeLeafExpr::Chandle(value),
            LeafValue::Container(_) => return Err(CONTAINER_LEAF_UNSUPPORTED.to_owned()),
        })
    }
}

/// Evaluate `value` once into the local `name` and return its read.
fn capture_leaf(value: LeafValue, name: String, captures: &mut Vec<IrStmt>) -> LeafValue {
    match value {
        LeafValue::Container(container) => LeafValue::Container(container),
        LeafValue::String(value) => {
            captures.push(IrStmt::DeclString {
                name: name.clone(),
                init: Some(value),
            });
            LeafValue::String(IrStringExpr::LocalRead(name))
        }
        LeafValue::Chandle(value) => {
            captures.push(IrStmt::Object(Box::new(IrObjectStmt::ChandleDeclareLocal(
                name.clone(),
                Some(value),
            ))));
            LeafValue::Chandle(IrChandleExpr::LocalRead(name))
        }
        LeafValue::Packed(value) | LeafValue::Real(value) => {
            let (width, signed) = (value.width, value.signed);
            captures.push(IrStmt::DeclLocal {
                name: name.clone(),
                width,
                signed,
                two_state: false,
                init: Some(Box::new(value)),
            });
            let read = IrExpr::new(IrExprKind::LocalRead(name), width, signed, None);
            if width == 0 {
                LeafValue::Real(read)
            } else {
                LeafValue::Packed(read)
            }
        }
    }
}

pub(super) fn leaf_ty(element: &IrContainerElement) -> Result<IrClassFieldType, String> {
    match element {
        IrContainerElement::Packed {
            width,
            signed,
            two_state,
        } => Ok(IrClassFieldType::Packed {
            width: *width,
            signed: *signed,
            two_state: *two_state,
        }),
        IrContainerElement::Real { shortreal } => Ok(IrClassFieldType::Real {
            shortreal: *shortreal,
        }),
        IrContainerElement::String => Ok(IrClassFieldType::String),
        IrContainerElement::Chandle => Ok(IrClassFieldType::Chandle),
        IrContainerElement::Opaque { kind, .. } if is_class_handle_kind(kind) => {
            Ok(IrClassFieldType::Chandle)
        }
        _ => Err("native record leaf has no scalar representation".to_owned()),
    }
}

/// Scalar leaves and container members of one native layout.
#[derive(Default)]
pub(super) struct NativeLeaves {
    pub(super) scalars: Vec<NativeLeaf>,
    pub(super) containers: Vec<NativeContainerLeaf>,
}

fn collect_native_leaves(
    descriptor: &TypeDescriptor,
    element: &IrContainerElement,
    path: &mut Vec<AggregatePathPart>,
    items: &mut Vec<u32>,
    output: &mut NativeLeaves,
) -> Result<(), String> {
    let leaves = &mut output.scalars;
    match (element, &descriptor.shape) {
        (IrContainerElement::Aggregate { members, .. }, TypeShape::Aggregate(layout)) => {
            if members.len() != layout.members.len() {
                return Err("native record layout disagrees with its descriptor".to_owned());
            }
            for (index, (member, item)) in layout.members.iter().zip(members).enumerate() {
                if member.initializer.is_some() {
                    return Err(format!(
                        "member default of `{}` in native subroutine storage is not supported",
                        member.name
                    ));
                }
                path.push(AggregatePathPart::Member(member.name.clone()));
                items.push(u32::try_from(index).map_err(|_| "native record is too wide")?);
                collect_native_leaves(&member.descriptor, &item.element, path, items, output)?;
                items.pop();
                path.pop();
            }
            Ok(())
        }
        (
            IrContainerElement::FixedArray {
                dimensions,
                element: item,
            },
            TypeShape::FixedArray {
                element: item_descriptor,
                ..
            },
        ) => {
            let count = native_item_count(element).ok_or("native fixed array is too large")?;
            if count > NATIVE_VALUE_MAX_LEAVES as u64 {
                return Err(format!(
                    "native record array member has {count} elements; at most {NATIVE_VALUE_MAX_LEAVES} leaves are supported"
                ));
            }
            for flat in 0..count {
                // Left dimensions vary slowest; each index is the declared one.
                let mut remainder = flat;
                let mut indices = vec![0i32; dimensions.len()];
                for (slot, (left, right)) in dimensions.iter().enumerate().rev() {
                    let extent = i64::from(*left).abs_diff(i64::from(*right)) + 1;
                    let offset = i64::try_from(remainder % extent).map_err(|_| "index")?;
                    remainder /= extent;
                    let declared = if left <= right {
                        i64::from(*left) + offset
                    } else {
                        i64::from(*left) - offset
                    };
                    indices[slot] = i32::try_from(declared).map_err(|_| "native index overflow")?;
                }
                let depth = path.len();
                path.extend(indices.into_iter().map(AggregatePathPart::Index));
                items.push(u32::try_from(flat).map_err(|_| "native array is too large")?);
                collect_native_leaves(item_descriptor, item, path, items, output)?;
                items.pop();
                path.truncate(depth);
            }
            Ok(())
        }
        (
            IrContainerElement::Container { .. },
            TypeShape::Container {
                element: item,
                array,
                ..
            },
        ) => {
            let name = path
                .iter()
                .map(|part| match part {
                    AggregatePathPart::Member(name) => format!(".{name}"),
                    AggregatePathPart::Index(index) => format!("[{index}]"),
                })
                .collect::<String>();
            output.containers.push(NativeContainerLeaf {
                path: path.clone(),
                element: lower_container_element(item)?,
                kind: super::aggregates::ir_container_kind(array, &name, &descriptor.name)?,
            });
            Ok(())
        }
        (leaf, _) => {
            leaves.push(NativeLeaf {
                path: path.clone(),
                items: items.clone(),
                ty: leaf_ty(leaf)?,
            });
            if leaves.len() > NATIVE_VALUE_MAX_LEAVES {
                return Err(format!(
                    "native record has more than {NATIVE_VALUE_MAX_LEAVES} leaves"
                ));
            }
            Ok(())
        }
    }
}

/// The native value type of a tagged union with string, real, handle or
/// container members: a record of its tag (four-state, so an unassigned
/// union has no active member) and each non-void member's own storage, the
/// same leaves as module storage (`collect_native_tagged_union`).
fn native_tagged_element(
    descriptor: &TypeDescriptor,
    layout: &AggregateLayout,
) -> Result<IrContainerElement, String> {
    let tag_bits = layout.tag_bits().ok_or("tagged union has no members")?;
    let mut members = Vec::new();
    if tag_bits > 0 {
        members.push(IrContainerMember {
            name: NATIVE_TAG_MEMBER.to_owned(),
            element: Box::new(IrContainerElement::Packed {
                width: tag_bits,
                signed: false,
                two_state: false,
            }),
        });
    }
    for member in layout
        .members
        .iter()
        .filter(|member| !is_void_member(member))
    {
        members.push(IrContainerMember {
            name: member.name.clone(),
            element: Box::new(lower_container_element(&member.descriptor)?),
        });
    }
    Ok(IrContainerElement::Aggregate {
        type_id: descriptor.id.0,
        members,
    })
}

fn is_void_member(member: &AggregateMember) -> bool {
    matches!(&member.descriptor.shape, TypeShape::Opaque { kind } if kind == "Void")
}

/// Leaves of a whole native value: a tagged union's tag and members by name
/// (void members have no storage), or a record's members recursively.
fn collect_native_root_leaves(
    descriptor: &TypeDescriptor,
    element: &IrContainerElement,
    output: &mut NativeLeaves,
) -> Result<(), String> {
    let (TypeShape::Aggregate(layout), IrContainerElement::Aggregate { members, .. }) =
        (&descriptor.shape, element)
    else {
        return collect_native_leaves(
            descriptor,
            element,
            &mut Vec::new(),
            &mut Vec::new(),
            output,
        );
    };
    if layout.kind != AggregateKind::TaggedUnion {
        return collect_native_leaves(
            descriptor,
            element,
            &mut Vec::new(),
            &mut Vec::new(),
            output,
        );
    }
    let tag_bits = layout.tag_bits().ok_or("tagged union has no members")?;
    let mut items = members.iter();
    let mut position = 0u32;
    if tag_bits > 0 {
        items.next();
        output.scalars.push(NativeLeaf {
            path: vec![AggregatePathPart::Member(NATIVE_TAG_MEMBER.to_owned())],
            items: vec![0],
            ty: IrClassFieldType::Packed {
                width: tag_bits,
                signed: false,
                two_state: false,
            },
        });
        position = 1;
    }
    for (member, item) in layout
        .members
        .iter()
        .filter(|member| !is_void_member(member))
        .zip(items)
    {
        collect_native_leaves(
            &member.descriptor,
            &item.element,
            &mut vec![AggregatePathPart::Member(member.name.clone())],
            &mut vec![position],
            output,
        )?;
        position += 1;
    }
    Ok(())
}

/// Whether a record reaches a built-in semaphore, mailbox or process handle,
/// which keep their own object kinds rather than plain identity leaves.
fn has_builtin_class_leaf(descriptor: &TypeDescriptor) -> bool {
    match &descriptor.shape {
        TypeShape::Opaque { kind } => {
            kind == "Class"
                && matches!(
                    descriptor.name.as_str(),
                    "semaphore" | "mailbox" | "process"
                )
        }
        TypeShape::Aggregate(layout) => layout
            .members
            .iter()
            .any(|member| has_builtin_class_leaf(&member.descriptor)),
        TypeShape::FixedArray { element, .. } | TypeShape::Container { element, .. } => {
            has_builtin_class_leaf(element)
        }
        _ => false,
    }
}

impl Codegen<'_> {
    /// The native value type of a declaration, or `None` when the existing
    /// packed, fixed-array or leaf storage represents it.
    pub(in super::super) fn native_value_type(&self, node: NodeId) -> Option<IrContainerElement> {
        let descriptor = self.query_descriptor(node)?;
        let TypeShape::Aggregate(layout) = &descriptor.shape else {
            return None;
        };
        // A tagged union without a packed payload (SIM-007); wider fixed
        // unions use column layout.
        if layout.kind == AggregateKind::TaggedUnion {
            if Self::fixed_descriptor_width_bits(descriptor).is_some()
                || has_builtin_class_leaf(descriptor)
            {
                return None;
            }
            let element = native_tagged_element(descriptor, layout).ok()?;
            validate_native_type(&element, "native").ok()?;
            return Some(element);
        }
        // Records whose leaves are all integral are fixed values; beyond
        // packed capacity they use column layout (RTL-101), not native
        // storage. So do records with a member array above the dense
        // threshold, whose real, string and chandle members travel in a
        // native value of their own (RTL-101b).
        if layout.kind != AggregateKind::UnpackedStruct
            || Self::fixed_descriptor_width_bits(descriptor).is_some()
            || super::record_columns::record_column_layout_type(descriptor)
        {
            return None;
        }
        if has_builtin_class_leaf(descriptor) {
            return None;
        }
        let element = lower_container_element(descriptor).ok()?;
        validate_native_type(&element, "native").ok()?;
        Some(element)
    }

    /// Intern the type and leaf layout of a native declaration.
    pub(in super::super) fn native_layout(
        &mut self,
        node: NodeId,
    ) -> Result<Option<NativeLayout>, String> {
        if let Some(layout) = self.native_layouts.get(&node) {
            return Ok(Some(layout.clone()));
        }
        let Some(element) = self.native_value_type(node) else {
            return Ok(None);
        };
        let descriptor = self
            .query_descriptor(node)
            .cloned()
            .ok_or("native declaration has no type descriptor")?;
        let mut leaves = NativeLeaves::default();
        collect_native_root_leaves(&descriptor, &element, &mut leaves)
            .map_err(|error| format!("{error} (`{}`)", self.node(node).name))?;
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
        let layout = NativeLayout {
            ty,
            descriptor,
            leaves: leaves.scalars,
            containers: leaves.containers,
        };
        self.native_layouts.insert(node, layout.clone());
        Ok(Some(layout))
    }

    /// Storage for one native declaration of one instance. Automatic storage
    /// is a lexical activation; static storage persists for the model.
    fn native_storage_for(
        &mut self,
        inst: NodeId,
        node: NodeId,
        automatic: bool,
    ) -> Result<Option<usize>, String> {
        if let Some(value) = self.native_storage.get(&(inst, node)) {
            return Ok(Some(*value));
        }
        let Some(layout) = self.native_layout(node)? else {
            return Ok(None);
        };
        let companions = self.native_companions(&layout, automatic);
        let index = self.model.native_values.len();
        self.model.native_values.push(IrNativeValue {
            c_name: format!("S_llg_native_{index}"),
            ty: layout.ty,
            activation: automatic,
            companions,
        });
        self.native_storage.insert((inst, node), index);
        Ok(Some(index))
    }

    /// One companion container per container member of `layout`, with the
    /// value's lifetime: activation storage, or model storage for a static
    /// subroutine value.
    fn native_companions(&mut self, layout: &NativeLayout, activation: bool) -> Vec<usize> {
        layout
            .containers
            .iter()
            .map(|leaf| {
                let ir = self.model.containers.len();
                self.model.containers.push(crate::sim::ir::IrContainer {
                    c_name: format!("S_llg_container_{ir}"),
                    element: leaf.element.clone(),
                    kind: leaf.kind.clone(),
                    initial_size: None,
                    activation,
                    class_field: None,
                });
                ir
            })
            .collect()
    }

    /// The companion container of member `path` of native value `value`.
    pub(in super::super) fn native_companion(
        &self,
        value: usize,
        path: &[AggregatePathPart],
    ) -> Option<usize> {
        let layout = self.native_layout_of_value(value).ok()?;
        let position = layout
            .containers
            .iter()
            .position(|leaf| leaf.path == path)?;
        self.model.native_values[value]
            .companions
            .get(position)
            .copied()
    }

    /// A fresh lexical value of the same type as `node`, for caller-side
    /// transfers. The caller declares it with `NativeValueDeclare`.
    pub(in super::super) fn native_temporary(&mut self, node: NodeId) -> Result<usize, String> {
        let layout = self
            .native_layout(node)?
            .ok_or("native temporary requires a native record type")?;
        let companions = self.native_companions(&layout, true);
        let index = self.model.native_values.len();
        self.model.native_values.push(IrNativeValue {
            c_name: format!("S_llg_native_{index}"),
            ty: layout.ty,
            activation: true,
            companions,
        });
        self.native_value_layouts.insert(index, node);
        Ok(index)
    }

    /// A fresh lexical value with the type and layout of native value `like`.
    pub(in super::super) fn native_temporary_like(&mut self, like: usize) -> Result<usize, String> {
        let node = *self
            .native_value_layouts
            .get(&like)
            .ok_or("native value has no declaration layout")?;
        self.native_temporary(node)
    }

    /// The first activation native value or container of the enclosing
    /// subroutine that a fork branch references; branch processes cannot
    /// address it.
    pub(in super::super) fn native_activation_capture(&self, branch: NodeId) -> Option<String> {
        let mut pending = vec![branch];
        while let Some(node) = pending.pop() {
            let targets: Vec<NodeId> = match self.kind(node) {
                NodeKind::Expr(ExprKind::Ref {
                    target: Some(target),
                }) => vec![*target],
                NodeKind::Expr(ExprKind::HierPath { refs, .. }) => {
                    refs.iter().flatten().copied().collect()
                }
                _ => Vec::new(),
            };
            for target in targets {
                let activation = self
                    .native_roots
                    .get(&target)
                    .is_some_and(|value| self.model.native_values[*value].activation)
                    || self
                        .container_globals
                        .get(&target)
                        .is_some_and(|container| self.model.containers[container.ir].activation);
                if activation && !self.node_is_within(target, branch) {
                    return Some(self.node(target).name.clone());
                }
            }
            pending.extend(self.node(node).children.iter().copied());
        }
        None
    }

    /// Whether a function result uses descriptor-backed native storage.
    pub(in super::super) fn native_return(&self, function: NodeId) -> bool {
        matches!(self.kind(function), NodeKind::FuncTask { ret: Some(_), .. })
            && self.native_value_type(function).is_some()
    }

    fn native_function_nodes(&self, function: NodeId, output: &mut Vec<NodeId>) {
        output.extend(
            self.func_formals(function)
                .into_iter()
                .map(|(formal, _)| formal),
        );
        if self.native_return(function) {
            output.push(function);
        }
        if let Some(body) = self.func_body(function) {
            self.native_body_locals(body, output);
        }
    }

    fn native_body_locals(&self, node: NodeId, output: &mut Vec<NodeId>) {
        if matches!(self.kind(node), NodeKind::Var { .. }) && self.native_value_type(node).is_some()
        {
            if !self.is_body_local(node) {
                return;
            }
            output.push(node);
            return;
        }
        for child in &self.node(node).children {
            self.native_body_locals(*child, output);
        }
    }

    /// Allocate native storage for the formals, result and locals of one
    /// subroutine instance. Static subroutines keep persistent storage.
    pub(in super::super) fn prepare_native_function(
        &mut self,
        inst: NodeId,
        function: NodeId,
        automatic: bool,
    ) -> Result<(), String> {
        let mut nodes = Vec::new();
        self.native_function_nodes(function, &mut nodes);
        for node in nodes {
            let lifetime = match self.kind(node) {
                NodeKind::FuncArg { direction, .. } => {
                    if *direction == DbDirection::Ref && self.native_value_type(node).is_some() {
                        return Err(format!(
                            "ref formal `{}` of native record type is not supported",
                            self.node(node).name
                        ));
                    }
                    automatic
                }
                NodeKind::FuncTask { .. } => automatic,
                _ => self.db.variable_lifetime(node) == VariableLifetime::Automatic,
            };
            if let Some(value) = self.native_storage_for(inst, node, lifetime)? {
                self.native_value_layouts.insert(value, node);
            }
        }
        Ok(())
    }

    /// Bind the native declarations of the subroutine instance being lowered.
    pub(in super::super) fn bind_native_function(&mut self, inst: NodeId, function: NodeId) {
        let mut nodes = Vec::new();
        self.native_function_nodes(function, &mut nodes);
        self.native_roots = nodes
            .into_iter()
            .filter_map(|node| {
                self.native_storage
                    .get(&(inst, node))
                    .map(|value| (node, *value))
            })
            .collect();
    }

    /// Callee storage of a native formal or result in one instance.
    pub(in super::super) fn native_formal_storage(
        &self,
        inst: NodeId,
        node: NodeId,
    ) -> Option<usize> {
        self.native_storage.get(&(inst, node)).copied()
    }

    /// Whether `node` declares a native formal, result or local.
    pub(in super::super) fn is_native_declaration(&self, node: NodeId) -> bool {
        self.native_layouts.contains_key(&node)
    }

    fn native_layout_of_value(&self, value: usize) -> Result<&NativeLayout, String> {
        self.native_value_layouts
            .get(&value)
            .and_then(|node| self.native_layouts.get(node))
            .ok_or_else(|| "native value has no layout".to_owned())
    }

    /// Resolve an expression rooted at a bound native declaration to its
    /// value and constant member/index path. `Err` marks a selection rooted
    /// at a native value that has no constant path.
    pub(in super::super) fn native_path_of(
        &self,
        node: NodeId,
    ) -> Result<Option<(usize, Vec<AggregatePathPart>)>, String> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => Ok(self
                .native_roots
                .get(target)
                .map(|value| (*value, Vec::new()))),
            NodeKind::Var { .. } | NodeKind::FuncArg { .. } | NodeKind::FuncTask { .. } => Ok(self
                .native_roots
                .get(&node)
                .map(|value| (*value, Vec::new()))),
            NodeKind::Expr(ExprKind::HierPath { parts, refs }) => {
                let Some((index, value)) = refs.iter().enumerate().find_map(|(index, target)| {
                    target.and_then(|target| self.native_roots.get(&target).map(|v| (index, *v)))
                }) else {
                    return Ok(None);
                };
                Ok(Some((
                    value,
                    parts
                        .iter()
                        .skip(index + 1)
                        .cloned()
                        .map(AggregatePathPart::Member)
                        .collect(),
                )))
            }
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                // The frontend can name a record member array by a detached
                // array node; the captured select path then gives its owner.
                let rooted = match self.db.array_select_path(node) {
                    Some((owner, members)) => self.native_roots.get(&owner).map(|value| {
                        (
                            *value,
                            members
                                .iter()
                                .cloned()
                                .map(AggregatePathPart::Member)
                                .collect::<Vec<_>>(),
                        )
                    }),
                    None => self.native_path_of(*base)?,
                };
                let Some((value, mut path)) = rooted else {
                    return Ok(None);
                };
                // An element of a container member selects the container.
                if self.native_companion(value, &path).is_some() {
                    return Ok(None);
                }
                for index in indices {
                    let index = self
                        .eval_bound_i128(*index)
                        .ok()
                        .and_then(|index| i32::try_from(index).ok())
                        .ok_or_else(|| {
                            "runtime index into a native record array member is not supported"
                                .to_owned()
                        })?;
                    path.push(AggregatePathPart::Index(index));
                }
                Ok(Some((value, path)))
            }
            _ => Ok(None),
        }
    }

    /// The leaf selected by a native-rooted expression, if it is one leaf.
    pub(in super::super) fn native_leaf_of(
        &self,
        node: NodeId,
    ) -> Result<Option<(usize, NativeLeaf)>, String> {
        // Real, string and chandle members of a column-layout record value
        // are items of the record's native value.
        if let Some(leaf) = self.record_native_leaf(node) {
            return Ok(Some(leaf));
        }
        let Some((value, path)) = self.native_path_of(node)? else {
            return Ok(None);
        };
        let layout = self.native_layout_of_value(value)?;
        if let Some(leaf) = layout.leaves.iter().find(|leaf| leaf.path == path) {
            return Ok(Some((value, leaf.clone())));
        }
        if layout.leaves.iter().any(|leaf| {
            leaf.path.starts_with(&path)
                || (path.starts_with(&leaf.path)
                    && matches!(leaf.ty, IrClassFieldType::Packed { .. }))
        }) {
            // A sub-record, or a selection inside a packed leaf that the
            // fixed projections lower.
            return Ok(None);
        }
        Err(format!(
            "native record selection `{}` does not name a member",
            aggregate_path_suffix(&path)
        ))
    }

    /// The packed leaf whose path is a prefix of a native selection, with the
    /// remaining member/index path and the leaf type descriptor.
    #[allow(clippy::type_complexity)]
    pub(in super::super) fn native_packed_leaf_prefix(
        &self,
        node: NodeId,
    ) -> Result<Option<(usize, NativeLeaf, Vec<AggregatePathPart>, TypeDescriptor)>, String> {
        let Some((value, path)) = self.native_path_of(node)? else {
            return Ok(None);
        };
        let layout = self.native_layout_of_value(value)?;
        let Some(leaf) = layout.leaves.iter().find(|leaf| {
            path.starts_with(&leaf.path) && matches!(leaf.ty, IrClassFieldType::Packed { .. })
        }) else {
            return Ok(None);
        };
        let descriptor = Self::descriptor_at_path(&layout.descriptor, &leaf.path)
            .ok_or("native packed leaf has no type")?;
        Ok(Some((
            value,
            leaf.clone(),
            path[leaf.path.len()..].to_vec(),
            descriptor,
        )))
    }

    pub(in super::super) fn native_leaf_target_of(
        &mut self,
        value: usize,
        leaf: &NativeLeaf,
    ) -> IrLhs {
        self.native_leaf_lhs(value, leaf)
    }

    /// Read of the handle leaf at `path` below native value `value`.
    pub(in super::super) fn native_handle_leaf(
        &mut self,
        value: usize,
        path: &[AggregatePathPart],
    ) -> Result<Option<IrChandleExpr>, String> {
        let Some(leaf) = self
            .native_layout_of_value(value)?
            .leaves
            .iter()
            .find(|leaf| leaf.path == path && leaf.ty == IrClassFieldType::Chandle)
            .cloned()
        else {
            return Ok(None);
        };
        Ok(Some(IrChandleExpr::LocalRead(
            self.native_leaf_symbol(value, &leaf),
        )))
    }

    /// Model-level access name of one native leaf, shared by all uses.
    pub(in super::super) fn native_leaf_symbol(
        &mut self,
        value: usize,
        leaf: &NativeLeaf,
    ) -> String {
        if let Some(name) = self.native_leaf_symbols.get(&(value, leaf.items.clone())) {
            return name.clone();
        }
        let name = format!("_llg_access_{}", self.model.native_accesses.len());
        self.model
            .native_accesses
            .push(crate::sim::ir::IrNativeAccess {
                name: name.clone(),
                receiver: IrChandleExpr::Null,
                kind: IrNativeAccessKind::ValueItem { value, ty: leaf.ty },
                site: None,
                item_path: leaf.items.clone(),
                function: None,
            });
        self.native_leaf_symbols
            .insert((value, leaf.items.clone()), name.clone());
        name
    }

    fn native_leaf_read(&mut self, value: usize, leaf: &NativeLeaf) -> LeafValue {
        let name = self.native_leaf_symbol(value, leaf);
        match leaf.ty {
            IrClassFieldType::Packed { width, signed, .. } => LeafValue::Packed(IrExpr::new(
                IrExprKind::LocalRead(name),
                width,
                signed,
                None,
            )),
            IrClassFieldType::Real { .. } => {
                LeafValue::Real(IrExpr::new(IrExprKind::LocalRead(name), 0, false, None))
            }
            IrClassFieldType::String => LeafValue::String(IrStringExpr::LocalRead(name)),
            IrClassFieldType::Chandle => LeafValue::Chandle(IrChandleExpr::LocalRead(name)),
        }
    }

    fn native_leaf_lhs(&mut self, value: usize, leaf: &NativeLeaf) -> IrLhs {
        let name = self.native_leaf_symbol(value, leaf);
        let (width, signed, two_state, shortreal) = match leaf.ty {
            IrClassFieldType::Packed {
                width,
                signed,
                two_state,
            } => (width, signed, two_state, false),
            IrClassFieldType::Real { shortreal } => (0, false, false, shortreal),
            _ => (0, false, false, false),
        };
        IrLhs::WholeRef {
            addr: format!("&{name}"),
            width,
            signed,
            two_state,
            shortreal,
        }
    }

    /// Packed or real read of a native leaf; `None` for other expressions.
    pub(in super::super) fn native_leaf_expr(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrExpr>, String> {
        let Some((value, leaf)) = self.native_leaf_of(node)? else {
            return self.element_leaf_read(path, node);
        };
        Ok(match self.native_leaf_read(value, &leaf) {
            LeafValue::Packed(value) | LeafValue::Real(value) => Some(value),
            _ => None,
        })
    }

    /// Packed or real target of a native leaf; `None` for other lvalues.
    pub(in super::super) fn native_leaf_target(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrLhs>, String> {
        let Some((value, leaf)) = self.native_leaf_of(node)? else {
            return self.element_leaf_target(path, node);
        };
        Ok(matches!(
            leaf.ty,
            IrClassFieldType::Packed { .. } | IrClassFieldType::Real { .. }
        )
        .then(|| self.native_leaf_lhs(value, &leaf)))
    }

    /// Access name of a native string or chandle leaf, for reads and writes.
    pub(in super::super) fn native_object_leaf(
        &mut self,
        path: &str,
        node: NodeId,
        string: bool,
    ) -> Result<Option<String>, String> {
        let Some((value, leaf)) = self.native_leaf_of(node)? else {
            // Reads of element strings and handles resolve earlier (see
            // `lower_string`/`lower_chandle`); this path names write targets.
            let wanted = if string {
                IrClassFieldType::String
            } else {
                IrClassFieldType::Chandle
            };
            if self.element_leaf_kind(node) != Some(wanted) {
                return Ok(None);
            }
            return Ok(self
                .element_leaf_symbol(path, node, true)?
                .map(|(name, _)| name));
        };
        let matches = if string {
            leaf.ty == IrClassFieldType::String
        } else {
            leaf.ty == IrClassFieldType::Chandle
        };
        Ok(matches.then(|| self.native_leaf_symbol(value, &leaf)))
    }

    /// Kind test used by expression classification (no IR is created).
    pub(in super::super) fn native_leaf_kind(&self, node: NodeId) -> Option<IrClassFieldType> {
        self.native_leaf_of(node)
            .ok()
            .flatten()
            .map(|(_, leaf)| leaf.ty)
            .or_else(|| self.element_leaf_kind(node))
    }

    /// Whether an lvalue writes native subroutine storage.
    pub(in super::super) fn native_target(&self, node: NodeId) -> bool {
        matches!(self.native_path_of(node), Ok(Some(_)) | Err(_))
    }

    /// Whether an lvalue is a record (or record selection) with string or
    /// chandle leaves: a native root or a module record whose leaves are
    /// separate model objects rather than one packed signal.
    pub(in super::super) fn native_record_target(&self, node: NodeId) -> bool {
        if matches!(self.native_path_of(node), Ok(Some(_))) {
            return true;
        }
        self.resolve_unpacked_aggregate(node)
            .is_some_and(|selection| {
                selection
                    .storage
                    .leaves
                    .iter()
                    .any(|leaf| leaf.path.starts_with(&selection.prefix) && leaf.object.is_some())
            })
    }

    // ── Whole-value transfers ───────────────────────────────────────────

    /// A whole native value or native/module record selection.
    pub(in super::super) fn native_endpoint(
        &self,
        node: NodeId,
    ) -> Result<Option<(NativeEndpoint, TypeDescriptor)>, String> {
        if let Some((value, prefix)) = self.native_path_of(node)? {
            let layout = self.native_layout_of_value(value)?;
            // Only selections above the leaves are record values; a packed
            // struct or packed array member is one scalar leaf.
            let below =
                |path: &[AggregatePathPart]| path.len() > prefix.len() && path.starts_with(&prefix);
            if !layout.leaves.iter().any(|leaf| below(&leaf.path))
                && !layout.containers.iter().any(|leaf| below(&leaf.path))
            {
                return Ok(None);
            }
            let descriptor = Self::descriptor_at_path(&layout.descriptor, &prefix)
                .ok_or("native record selection has no type")?;
            return Ok(Some((NativeEndpoint::Value { value, prefix }, descriptor)));
        }
        Ok(self.resolve_unpacked_aggregate(node).map(|selection| {
            let descriptor = selection.descriptor.clone();
            (NativeEndpoint::Module(Box::new(selection)), descriptor)
        }))
    }

    fn endpoint_leaves(
        &self,
        endpoint: &NativeEndpoint,
    ) -> Result<Vec<(Vec<AggregatePathPart>, NativeEndpointLeaf)>, String> {
        match endpoint {
            // Scalar leaves, then container members, each in declaration
            // order, so both representations pair leaves alike.
            NativeEndpoint::Value { value, prefix } => {
                let layout = self.native_layout_of_value(*value)?;
                let mut leaves = layout
                    .leaves
                    .iter()
                    .filter(|leaf| leaf.path.starts_with(prefix))
                    .map(|leaf| {
                        (
                            leaf.path[prefix.len()..].to_vec(),
                            NativeEndpointLeaf::Value(*value, leaf.clone()),
                        )
                    })
                    .collect::<Vec<_>>();
                for (leaf, companion) in layout
                    .containers
                    .iter()
                    .zip(&self.model.native_values[*value].companions)
                {
                    if leaf.path.starts_with(prefix) {
                        leaves.push((
                            leaf.path[prefix.len()..].to_vec(),
                            NativeEndpointLeaf::Container(*companion),
                        ));
                    }
                }
                Ok(leaves)
            }
            NativeEndpoint::Module(selection) => {
                let selected = selection
                    .storage
                    .leaves
                    .iter()
                    .filter(|leaf| leaf.path.starts_with(&selection.prefix));
                let (containers, scalars): (Vec<_>, Vec<_>) =
                    selected.partition(|leaf| leaf.container.is_some());
                Ok(scalars
                    .into_iter()
                    .map(|leaf| {
                        (
                            leaf.path[selection.prefix.len()..].to_vec(),
                            NativeEndpointLeaf::Module(Box::new(leaf.clone())),
                        )
                    })
                    .chain(containers.into_iter().filter_map(|leaf| {
                        leaf.container.as_ref().map(|container| {
                            (
                                leaf.path[selection.prefix.len()..].to_vec(),
                                NativeEndpointLeaf::Container(container.ir),
                            )
                        })
                    }))
                    .collect())
            }
        }
    }

    fn endpoint_leaf_read(&mut self, leaf: &NativeEndpointLeaf) -> Result<LeafValue, String> {
        match leaf {
            NativeEndpointLeaf::Value(value, leaf) => Ok(self.native_leaf_read(*value, leaf)),
            NativeEndpointLeaf::Container(container) => Ok(LeafValue::Container(*container)),
            NativeEndpointLeaf::Module(leaf) => {
                if let Some(object) = leaf.object {
                    let object = self.reference_object(object);
                    return Ok(match self.model.objects[object].ty {
                        IrObjectType::String => LeafValue::String(IrStringExpr::Read(object)),
                        IrObjectType::Chandle | IrObjectType::Semaphore => {
                            LeafValue::Chandle(IrChandleExpr::Read(object))
                        }
                        IrObjectType::Process => {
                            return Err("process record members cannot be copied".to_owned())
                        }
                    });
                }
                let value = self.aggregate_leaf_read(leaf)?;
                Ok(if value.is_real() {
                    LeafValue::Real(value)
                } else {
                    LeafValue::Packed(value)
                })
            }
        }
    }

    /// Write one captured leaf. A nonblocking write queues the leaf; only
    /// module record leaves (persistent model storage) are nonblocking
    /// targets, because a native root's leaves move when the root is
    /// replaced.
    fn endpoint_leaf_write(
        &mut self,
        path: &str,
        leaf: &NativeEndpointLeaf,
        value: LeafValue,
        nba: bool,
    ) -> Result<IrStmt, String> {
        if let LeafValue::Container(source) = value {
            let NativeEndpointLeaf::Container(target) = leaf else {
                return Err(format!(
                    "native record transfer pairs a container member with a scalar leaf in `{path}`"
                ));
            };
            if nba {
                return Err(format!(
                    "nonblocking assignment of a record with a queue, dynamic or associative member is not supported in `{path}`"
                ));
            }
            return Ok(IrStmt::Container(Box::new(IrContainerStmt::Copy {
                dst: *target,
                src: source,
            })));
        }
        if matches!(leaf, NativeEndpointLeaf::Container(_)) {
            return Err(format!(
                "native record transfer pairs a container member with a scalar leaf in `{path}`"
            ));
        }
        if nba {
            let NativeEndpointLeaf::Module(leaf) = leaf else {
                return Err(format!(
                    "nonblocking assignment to native record subroutine storage in `{path}` is not supported"
                ));
            };
            return match value {
                LeafValue::Container(_) => Err(CONTAINER_LEAF_UNSUPPORTED.to_owned()),
                LeafValue::String(value) => self.object_leaf_nba(
                    path,
                    self.reference_object(leaf.object.ok_or("string leaf has no storage")?),
                    NativeNbaValue::String(value),
                ),
                LeafValue::Chandle(value) => self.object_leaf_nba(
                    path,
                    self.reference_object(leaf.object.ok_or("chandle leaf has no storage")?),
                    NativeNbaValue::Chandle(value),
                ),
                LeafValue::Packed(value) | LeafValue::Real(value) => {
                    let lhs = self.aggregate_leaf_lhs(leaf)?;
                    Ok(IrStmt::Assign {
                        rhs: apply_lhs_assignment_context(&self.model, &lhs, value),
                        lhs,
                        nba: true,
                    })
                }
            };
        }
        match (leaf, value) {
            (NativeEndpointLeaf::Container(_), _) | (_, LeafValue::Container(_)) => {
                Err(CONTAINER_LEAF_UNSUPPORTED.to_owned())
            }
            (NativeEndpointLeaf::Value(native, leaf), LeafValue::String(value)) => {
                let name = self.native_leaf_symbol(*native, leaf);
                Ok(IrStmt::Object(Box::new(IrObjectStmt::StringAssignLocal(
                    name, value,
                ))))
            }
            (NativeEndpointLeaf::Value(native, leaf), LeafValue::Chandle(value)) => {
                let name = self.native_leaf_symbol(*native, leaf);
                Ok(IrStmt::Object(Box::new(IrObjectStmt::ChandleAssignLocal(
                    name, value,
                ))))
            }
            (
                NativeEndpointLeaf::Value(native, leaf),
                LeafValue::Packed(value) | LeafValue::Real(value),
            ) => {
                let lhs = self.native_leaf_lhs(*native, leaf);
                Ok(IrStmt::Assign {
                    rhs: apply_lhs_assignment_context(&self.model, &lhs, value),
                    lhs,
                    nba: false,
                })
            }
            (NativeEndpointLeaf::Module(leaf), LeafValue::String(value)) => {
                let object =
                    self.reference_object(leaf.object.ok_or("string leaf has no storage")?);
                Ok(IrStmt::Object(Box::new(IrObjectStmt::StringAssign(
                    object, value,
                ))))
            }
            (NativeEndpointLeaf::Module(leaf), LeafValue::Chandle(value)) => {
                let object =
                    self.reference_object(leaf.object.ok_or("chandle leaf has no storage")?);
                Ok(IrStmt::Object(Box::new(IrObjectStmt::ChandleAssign(
                    object, value,
                ))))
            }
            (
                NativeEndpointLeaf::Module(leaf),
                LeafValue::Packed(value) | LeafValue::Real(value),
            ) => {
                let lhs = self.aggregate_leaf_lhs(leaf)?;
                Ok(IrStmt::Assign {
                    rhs: apply_lhs_assignment_context(&self.model, &lhs, value),
                    lhs,
                    nba: false,
                })
            }
        }
    }

    /// Copy `source` into `target` leaf by leaf. Every source leaf is
    /// captured before the first write, so overlapping selections copy the
    /// original value.
    pub(in super::super) fn native_transfer(
        &mut self,
        path: &str,
        target: &NativeEndpoint,
        source: &NativeEndpoint,
        nba: bool,
    ) -> Result<IrStmt, String> {
        if let (
            NativeEndpoint::Value {
                value: dst,
                prefix: dst_prefix,
            },
            NativeEndpoint::Value {
                value: src,
                prefix: src_prefix,
            },
        ) = (target, source)
        {
            if !nba
                && dst_prefix.is_empty()
                && src_prefix.is_empty()
                && self.model.native_values[*dst].ty == self.model.native_values[*src].ty
            {
                return Ok(IrStmt::NativeValueCopy {
                    dst: *dst,
                    src: *src,
                });
            }
        }
        let targets = self.endpoint_leaves(target)?;
        let sources = self.endpoint_leaves(source)?;
        if targets.len() != sources.len()
            || targets.is_empty()
            || targets
                .iter()
                .zip(&sources)
                .any(|((left, _), (right, _))| left != right)
        {
            return Err(format!(
                "native record transfer has incompatible leaf layouts in `{path}`"
            ));
        }
        let mut captures = Vec::with_capacity(sources.len());
        let mut writes = Vec::with_capacity(targets.len());
        for (position, ((_, target), (_, source))) in targets.iter().zip(&sources).enumerate() {
            let value = self.endpoint_leaf_read(source)?;
            let name = format!("_llg_native_copy_{}_{position}", self.native_copy_sequence);
            let captured = capture_leaf(value, name, &mut captures);
            writes.push(self.endpoint_leaf_write(path, target, captured, nba)?);
        }
        self.native_copy_sequence += 1;
        captures.extend(writes);
        Ok(IrStmt::Block(captures))
    }
}

impl Codegen<'_> {
    /// Capture one source expression as a typed leaf value.
    fn native_leaf_source(
        &mut self,
        path: &str,
        ty: IrClassFieldType,
        node: NodeId,
    ) -> Result<LeafValue, String> {
        Ok(match ty {
            IrClassFieldType::String => LeafValue::String(self.lower_string(path, node)?),
            IrClassFieldType::Chandle => LeafValue::Chandle(self.lower_chandle(path, node)?),
            // The leaf write applies the assignment conversion.
            IrClassFieldType::Real { .. } | IrClassFieldType::Packed { .. } => {
                let value = self.lower_expr(path, node)?;
                if value.is_real() {
                    LeafValue::Real(value)
                } else {
                    LeafValue::Packed(value)
                }
            }
        })
    }

    /// Assign a pattern to a native value: every leaf value is evaluated and
    /// captured before the first write.
    fn native_pattern_into(
        &mut self,
        path: &str,
        value: usize,
        prefix: &[AggregatePathPart],
        descriptor: &TypeDescriptor,
        rhs: NodeId,
    ) -> Result<Option<IrStmt>, String> {
        let pattern = self.unwrap_assignment_pattern_cast(rhs);
        if self.assignment_pattern_operands(path, pattern)?.is_none() {
            return Ok(None);
        }
        let mut values = Vec::new();
        self.aggregate_descriptor_pattern_values(path, rhs, descriptor, prefix, &mut values)?;
        let leaves = self.native_layout_of_value(value)?.leaves.clone();
        // Container members are written in place, after every scalar source
        // is captured; a pattern must not read another container member it
        // also writes (SV 10.9 reads every source before writing).
        let written: Vec<usize> = values
            .iter()
            .filter_map(|(member_path, _)| self.native_companion(value, member_path))
            .collect();
        let mut captures = Vec::new();
        let mut writes = Vec::new();
        for (position, (member_path, node)) in values.into_iter().enumerate() {
            if let Some(container) = self.native_companion(value, &member_path) {
                if let Some(source) = self.container_of(node) {
                    if source.ir != container && written.contains(&source.ir) {
                        return Err(format!(
                            "assignment pattern in `{path}` reads container member storage it also writes"
                        ));
                    }
                }
                writes.push(self.lower_container_into(path, node, container, node)?);
                continue;
            }
            let Some(leaf) = leaves.iter().find(|leaf| leaf.path == member_path) else {
                // A whole nested record or subarray value.
                let (source, _) = self.native_endpoint(node)?.ok_or_else(|| {
                    format!(
                        "native record pattern member `{}` in `{path}` has no record value",
                        aggregate_path_suffix(&member_path)
                    )
                })?;
                let target = NativeEndpoint::Value {
                    value,
                    prefix: member_path,
                };
                writes.push(self.native_transfer(path, &target, &source, false)?);
                continue;
            };
            let source = self.native_leaf_source(path, leaf.ty, node)?;
            let name = format!(
                "_llg_native_pattern_{}_{position}",
                self.native_copy_sequence
            );
            let captured = match source {
                LeafValue::Container(_) => return Err(CONTAINER_LEAF_UNSUPPORTED.to_owned()),
                LeafValue::String(source) => {
                    captures.push(IrStmt::DeclString {
                        name: name.clone(),
                        init: Some(source),
                    });
                    LeafValue::String(IrStringExpr::LocalRead(name))
                }
                LeafValue::Chandle(source) => {
                    captures.push(IrStmt::Object(Box::new(IrObjectStmt::ChandleDeclareLocal(
                        name.clone(),
                        Some(source),
                    ))));
                    LeafValue::Chandle(IrChandleExpr::LocalRead(name))
                }
                LeafValue::Packed(source) | LeafValue::Real(source) => {
                    let (width, signed) = (source.width, source.signed);
                    captures.push(IrStmt::DeclLocal {
                        name: name.clone(),
                        width,
                        signed,
                        two_state: false,
                        init: Some(Box::new(source)),
                    });
                    let read = IrExpr::new(IrExprKind::LocalRead(name), width, signed, None);
                    if width == 0 {
                        LeafValue::Real(read)
                    } else {
                        LeafValue::Packed(read)
                    }
                }
            };
            writes.push(self.endpoint_leaf_write(
                path,
                &NativeEndpointLeaf::Value(value, leaf.clone()),
                captured,
                false,
            )?);
        }
        self.native_copy_sequence += 1;
        captures.extend(writes);
        Ok(Some(IrStmt::Block(captures)))
    }

    /// The native-result function a (possibly cast) call node invokes.
    fn native_result_callee(&self, rhs: NodeId) -> Result<Option<NativeResultCallee>, String> {
        let call = self.p30_unwrap_cast(rhs);
        let (name, callee) = match self.kind(call) {
            NodeKind::FuncCall { name, callee, .. } | NodeKind::MethodCall { name, callee, .. } => {
                (name.clone(), *callee)
            }
            _ => return Ok(None),
        };
        let function = match self.kind(call) {
            NodeKind::FuncCall { .. } => {
                self.resolve_callee_env(self.inst, &name, false, callee)?.0
            }
            _ => match callee {
                Some(function) => function,
                None => return Ok(None),
            },
        };
        Ok(self
            .native_return(function)
            .then_some((call, name, callee, function)))
    }

    /// The typed statement call of a native-result function writing its
    /// result into `result` through the trailing output operand.
    fn native_result_call(
        &mut self,
        path: &str,
        call: NodeId,
        name: &str,
        callee: Option<NodeId>,
        result: usize,
    ) -> Result<IrCall, String> {
        let expression = self.lower_func_call_expr(path, call, name, callee)?;
        let IrExprKind::CallFn(expression) = expression.kind else {
            return Err("native result call did not lower to a typed call".into());
        };
        let mut args = expression.args;
        let outputs = self.model.funcs[expression.f]
            .formals
            .iter()
            .filter(|formal| formal.is_address())
            .count();
        args.insert(outputs - 1, IrCallArg::NativeValue(result));
        let mut lowered = IrCall::new(expression.f, args, expression.depth, Vec::new(), Vec::new());
        lowered.receiver = expression.receiver;
        lowered.virtual_dispatch = expression.virtual_dispatch;
        lowered.virtual_call = expression.virtual_call;
        Ok(lowered)
    }

    /// Store a call to a native-result function into `target`. A whole
    /// native value of the result type receives the result directly.
    fn native_call_into(
        &mut self,
        path: &str,
        rhs: NodeId,
        target: &NativeEndpoint,
        nba: bool,
    ) -> Result<Option<IrStmt>, String> {
        let Some((call, name, callee, function)) = self.native_result_callee(rhs)? else {
            return Ok(None);
        };
        let mut statements = Vec::new();
        let result_type = self
            .native_layout(function)?
            .ok_or("native result has no layout")?
            .ty;
        let direct = match target {
            NativeEndpoint::Value { value, prefix }
                if !nba
                    && prefix.is_empty()
                    && self.model.native_values[*value].ty == result_type =>
            {
                Some(*value)
            }
            _ => None,
        };
        let result = match direct {
            Some(value) => value,
            None => {
                let temporary = self.native_temporary(function)?;
                statements.push(IrStmt::NativeValueDeclare(temporary));
                temporary
            }
        };
        let saved = self.native_call_prelude.replace((Vec::new(), Vec::new()));
        let lowered = self.native_result_call(path, call, &name, callee, result);
        let (prelude, epilogue) =
            std::mem::replace(&mut self.native_call_prelude, saved).unwrap_or_default();
        let lowered = lowered?;
        statements.extend(prelude);
        statements.push(IrStmt::Call(Box::new(lowered)));
        statements.extend(epilogue);
        if direct.is_none() {
            statements.push(self.native_transfer(
                path,
                target,
                &NativeEndpoint::Value {
                    value: result,
                    prefix: Vec::new(),
                },
                nba,
            )?);
        }
        Ok(Some(IrStmt::Block(statements)))
    }

    /// An input operand evaluated by a native-result call at the operand
    /// itself, valid in any expression context.
    fn native_call_operand(
        &mut self,
        path: &str,
        layout: &NativeLayout,
        actual: NodeId,
    ) -> Result<Option<IrCallArg>, String> {
        let Some((call, name, callee, function)) = self.native_result_callee(actual)? else {
            return Ok(None);
        };
        let result = self.native_temporary(function)?;
        if self.model.native_values[result].ty != layout.ty {
            return Err(format!(
                "native record argument in `{path}` has a different record type"
            ));
        }
        let saved = self.native_call_prelude.take();
        let lowered = self.native_result_call(path, call, &name, callee, result);
        self.native_call_prelude = saved;
        Ok(Some(IrCallArg::NativeCall {
            value: result,
            call: Box::new(lowered?),
        }))
    }

    /// Assign any legal source to a native or module record endpoint.
    pub(in super::super) fn native_assign_into(
        &mut self,
        path: &str,
        target: &NativeEndpoint,
        descriptor: &TypeDescriptor,
        rhs: NodeId,
        nba: bool,
    ) -> Result<IrStmt, String> {
        if let Some(statement) = self.native_call_into(path, rhs, target, nba)? {
            return Ok(statement);
        }
        // `tagged m value` into a whole tagged union value (SIM-007).
        let root = match target {
            NativeEndpoint::Value { value, prefix } if prefix.is_empty() => {
                Some(NativeTaggedRoot::Value(*value))
            }
            NativeEndpoint::Module(selection) if selection.prefix.is_empty() => {
                Some(NativeTaggedRoot::Module(selection.root))
            }
            _ => None,
        };
        if let Some(root) = root {
            if let Some(statement) = self.lower_native_tagged_construct(path, root, rhs, nba)? {
                return Ok(statement);
            }
        }
        if let (false, NativeEndpoint::Value { value, prefix }) = (nba, target) {
            if let Some(statement) =
                self.native_pattern_into(path, *value, prefix, descriptor, rhs)?
            {
                return Ok(statement);
            }
        }
        let source = self.p30_unwrap_cast(rhs);
        if self.native_record_conditional(source) {
            return self.native_conditional_into(path, target, descriptor, source, nba);
        }
        if !nba {
            if let Some(statement) = self.container_record_into(path, target, source)? {
                return Ok(statement);
            }
        }
        let (source, _) = self
            .native_endpoint(source)?
            .ok_or_else(|| format!("native record assignment in `{path}` has no record source"))?;
        self.native_transfer(path, target, &source, nba)
    }

    /// Whether `node` is a call whose result is a native record.
    pub(in super::super) fn native_call_node(&self, node: NodeId) -> bool {
        let call = self.p30_unwrap_cast(node);
        match self.kind(call) {
            NodeKind::FuncCall { name, callee, .. } => self
                .resolve_callee_env(self.inst, name, false, *callee)
                .is_ok_and(|(function, _)| self.native_return(function)),
            NodeKind::MethodCall { callee, .. } => {
                callee.is_some_and(|function| self.native_return(function))
            }
            _ => false,
        }
    }

    /// Lower a record assignment that reads or writes native subroutine
    /// storage or a native-result call; other record assignments keep their
    /// leaf lowering.
    pub(in super::super) fn lower_native_value_assignment(
        &mut self,
        path: &str,
        lhs: NodeId,
        rhs: NodeId,
        nba: bool,
        op: Operation,
    ) -> Result<Option<IrStmt>, String> {
        let lhs_native = self.native_path_of(lhs)?.is_some();
        let rhs_native = self
            .native_path_of(self.p30_unwrap_cast(rhs))
            .ok()
            .flatten()
            .is_some()
            || self.native_call_node(rhs)
            || self.native_record_conditional(self.p30_unwrap_cast(rhs))
            || self.is_container_record(rhs);
        if !lhs_native && !rhs_native {
            return Ok(None);
        }
        let Some((target, descriptor)) = self.native_endpoint(lhs)? else {
            return Ok(None);
        };
        // Only module record targets are persistent leaf objects; native
        // roots are subroutine storage whose leaves move on replacement.
        if nba && lhs_native {
            return Err(if self.subroutine_auto_target(lhs) {
                format!(
                    "nonblocking assignment to an automatic native record in `{path}` is illegal (SV 6.21, 10.4.2)"
                )
            } else {
                format!(
                    "nonblocking assignment to static native record subroutine storage in `{path}` is not supported"
                )
            });
        }
        if op != Operation::Assignment {
            return Err(format!(
                "compound assignment of a native record value in `{path}` is not supported"
            ));
        }
        self.native_assign_into(path, &target, &descriptor, rhs, nba)
            .map(Some)
    }

    /// Caller operand of a native formal. A whole native value of the formal
    /// type passes directly (the callee receives its own copy); any other
    /// actual uses a lexical temporary filled before the call and, for
    /// outputs and inouts, copied back after it.
    pub(in super::super) fn native_call_argument(
        &mut self,
        path: &str,
        formal: NodeId,
        actual: NodeId,
        before: &mut Vec<IrStmt>,
        after: &mut Vec<IrStmt>,
    ) -> Result<IrCallArg, String> {
        let layout = self
            .native_layout(formal)?
            .ok_or("native formal has no layout")?;
        let direction = match self.kind(formal) {
            NodeKind::FuncArg { direction, .. } => *direction,
            _ => return Err("native formal is not a subroutine argument".to_owned()),
        };
        let source = self.p30_unwrap_cast(actual);
        if let Some((value, prefix)) = self.native_path_of(source)? {
            if prefix.is_empty() && self.model.native_values[value].ty == layout.ty {
                return Ok(IrCallArg::NativeValue(value));
            }
        }
        if direction == DbDirection::Input {
            if let Some(argument) = self.native_call_operand(path, &layout, actual)? {
                return Ok(argument);
            }
            if let Some((leaves, containers)) = self.native_input_leaves(path, &layout, actual)? {
                return Ok(IrCallArg::NativeLeaves {
                    ty: layout.ty,
                    leaves,
                    containers,
                });
            }
        }
        let temporary = self.native_temporary(formal)?;
        let endpoint = NativeEndpoint::Value {
            value: temporary,
            prefix: Vec::new(),
        };
        before.push(IrStmt::NativeValueDeclare(temporary));
        if matches!(direction, DbDirection::Input | DbDirection::Inout) {
            before.push(self.native_assign_into(
                path,
                &endpoint,
                &layout.descriptor,
                actual,
                false,
            )?);
        }
        if matches!(direction, DbDirection::Output | DbDirection::Inout) {
            if let Some(writeback) =
                self.container_record_writeback(path, actual, temporary, before)?
            {
                after.push(writeback);
                return Ok(IrCallArg::NativeValue(temporary));
            }
            let (target, _) = self.native_endpoint(actual)?.ok_or_else(|| {
                format!(
                    "output actual of native record formal `{}` in `{path}` is not a record variable",
                    self.node(formal).name
                )
            })?;
            after.push(self.native_transfer(path, &target, &endpoint, false)?);
        }
        Ok(IrCallArg::NativeValue(temporary))
    }

    /// Leaf values of an input actual that is a record selection or an
    /// assignment pattern, evaluated by the callee-value construction at the
    /// call itself. `None` for sources that need statement temporaries.
    fn native_input_leaves(
        &mut self,
        path: &str,
        layout: &NativeLayout,
        actual: NodeId,
    ) -> Result<Option<NativeInputLeaves>, String> {
        let mut leaves = Vec::new();
        let mut containers = Vec::new();
        if let NodeKind::Expr(ExprKind::TaggedUnion { member, value }) =
            self.kind(self.p30_unwrap_cast(actual))
        {
            let (member, value) = (member.clone(), *value);
            return self.native_tagged_input_leaves(path, layout, &member, value);
        }
        let pattern = self.unwrap_assignment_pattern_cast(actual);
        if self.assignment_pattern_operands(path, pattern)?.is_some() {
            // Pattern container members are built in a statement temporary.
            if !layout.containers.is_empty() {
                return Ok(None);
            }
            let mut values = Vec::new();
            self.aggregate_descriptor_pattern_values(
                path,
                actual,
                &layout.descriptor,
                &[],
                &mut values,
            )?;
            if !self.native_pattern_leaf_values(
                path,
                layout,
                values,
                &mut leaves,
                &mut containers,
            )? {
                return Ok(None);
            }
            return Ok(Some((leaves, containers)));
        }
        if self.native_call_node(actual) {
            return Ok(None);
        }
        let Some((source, _)) = self.native_endpoint(self.p30_unwrap_cast(actual))? else {
            return Ok(None);
        };
        self.native_endpoint_leaves(path, layout, &[], &source, &mut leaves, &mut containers)?;
        Ok(Some((leaves, containers)))
    }

    /// Leaf values for `(member path, source node)` pairs of a pattern or a
    /// tagged construction; `false` when a source needs a statement
    /// temporary.
    fn native_pattern_leaf_values(
        &mut self,
        path: &str,
        layout: &NativeLayout,
        values: Vec<(Vec<AggregatePathPart>, NodeId)>,
        leaves: &mut Vec<IrNativeLeafValue>,
        containers: &mut Vec<usize>,
    ) -> Result<bool, String> {
        for (member_path, node) in values {
            if let Some(leaf) = layout.leaves.iter().find(|leaf| leaf.path == member_path) {
                let value = self.native_leaf_source(path, leaf.ty, node)?;
                leaves.push(IrNativeLeafValue {
                    items: leaf.items.clone(),
                    value: value.into_leaf_expr()?,
                });
                continue;
            }
            let Some((source, _)) = self.native_endpoint(self.p30_unwrap_cast(node))? else {
                return Ok(false);
            };
            self.native_endpoint_leaves(path, layout, &member_path, &source, leaves, containers)?;
        }
        Ok(true)
    }

    /// Leaf values of `tagged member value` for a tagged union formal: the
    /// tag names `member` and the member's leaves take the value.
    fn native_tagged_input_leaves(
        &mut self,
        path: &str,
        layout: &NativeLayout,
        member: &str,
        value: Option<NodeId>,
    ) -> Result<Option<NativeInputLeaves>, String> {
        let TypeShape::Aggregate(union) = &layout.descriptor.shape else {
            return Ok(None);
        };
        if union.kind != AggregateKind::TaggedUnion {
            return Ok(None);
        }
        let index = union
            .members
            .iter()
            .position(|candidate| candidate.name == member)
            .ok_or_else(|| format!("tagged union has no member `{member}` in `{path}`"))?;
        let descriptor = union.members[index].descriptor.clone();
        let tag_bits = union
            .tag_bits()
            .ok_or_else(|| format!("tagged union in `{path}` has no tag"))?;
        let mut leaves = Vec::new();
        let mut containers = Vec::new();
        if let Some(tag) = layout
            .leaves
            .iter()
            .find(|leaf| leaf.path == [AggregatePathPart::Member(NATIVE_TAG_MEMBER.to_owned())])
        {
            let constant = IrConst::packed(
                vec![u64::try_from(index).map_err(|_| "tagged member index overflows")?],
                vec![0],
                vec![0],
                tag_bits,
                false,
                None,
            )
            .map_err(|error| error.to_string())?;
            leaves.push(IrNativeLeafValue {
                items: tag.items.clone(),
                value: IrNativeLeafExpr::Packed(IrExpr::new(
                    IrExprKind::Const(constant),
                    tag_bits,
                    false,
                    None,
                )),
            });
        }
        if let Some(value) = value {
            let prefix = vec![AggregatePathPart::Member(member.to_owned())];
            let mut values = Vec::new();
            let pattern = self.unwrap_assignment_pattern_cast(value);
            if layout.leaves.iter().any(|leaf| leaf.path == prefix)
                || self.assignment_pattern_operands(path, pattern)?.is_none()
            {
                values.push((prefix, value));
            } else if layout.containers.is_empty() {
                self.aggregate_descriptor_pattern_values(
                    path,
                    value,
                    &descriptor,
                    &prefix,
                    &mut values,
                )?;
            } else {
                return Ok(None);
            }
            if !self.native_pattern_leaf_values(
                path,
                layout,
                values,
                &mut leaves,
                &mut containers,
            )? {
                return Ok(None);
            }
        }
        Ok(Some((leaves, containers)))
    }

    /// Append reads of every leaf of `source` as values for the leaves of
    /// `layout` below `prefix`, matched by relative member/index path, and
    /// the source storage of its container members to `containers`.
    fn native_endpoint_leaves(
        &mut self,
        path: &str,
        layout: &NativeLayout,
        prefix: &[AggregatePathPart],
        source: &NativeEndpoint,
        leaves: &mut Vec<IrNativeLeafValue>,
        containers: &mut Vec<usize>,
    ) -> Result<(), String> {
        let (source_containers, sources): (Vec<_>, Vec<_>) = self
            .endpoint_leaves(source)?
            .into_iter()
            .partition(|(_, leaf)| matches!(leaf, NativeEndpointLeaf::Container(_)));
        let targets: Vec<_> = layout
            .leaves
            .iter()
            .filter(|leaf| leaf.path.starts_with(prefix))
            .collect();
        let target_containers: Vec<_> = layout
            .containers
            .iter()
            .filter(|leaf| leaf.path.starts_with(prefix))
            .collect();
        if targets.len() != sources.len()
            || target_containers.len() != source_containers.len()
            || targets
                .iter()
                .zip(&sources)
                .any(|(target, (relative, _))| target.path[prefix.len()..] != relative[..])
            || target_containers
                .iter()
                .zip(&source_containers)
                .any(|(target, (relative, _))| target.path[prefix.len()..] != relative[..])
        {
            return Err(format!(
                "native record argument has an incompatible leaf layout in `{path}`"
            ));
        }
        for (target, (_, source)) in targets.into_iter().zip(sources) {
            let value = self.endpoint_leaf_read(&source)?;
            leaves.push(IrNativeLeafValue {
                items: target.items.clone(),
                value: value.into_leaf_expr()?,
            });
        }
        containers.extend(
            source_containers
                .into_iter()
                .filter_map(|(_, leaf)| match leaf {
                    NativeEndpointLeaf::Container(container) => Some(container),
                    _ => None,
                }),
        );
        Ok(())
    }

    /// Every leaf of a record operand in declaration order: a native value,
    /// a module native record or a whole record element of a container.
    fn record_leaf_reads(&mut self, path: &str, node: NodeId) -> Result<Option<LeafReads>, String> {
        if let Some(reads) = self.container_record_leaf_reads(path, node)? {
            return Ok(Some(reads));
        }
        let Some((endpoint, _)) = self.native_endpoint(node)? else {
            return Ok(None);
        };
        let mut reads = Vec::new();
        for (leaf_path, leaf) in self.endpoint_leaves(&endpoint)? {
            reads.push((leaf_path, self.endpoint_leaf_read(&leaf)?));
        }
        Ok(Some(reads))
    }

    /// Member-wise `==`/`!=`/`===`/`!==` when either record operand is
    /// native subroutine storage. Packed leaves keep four-state comparison,
    /// reals compare numerically, strings by contents and chandles by
    /// identity; the conjunction propagates unknown packed results.
    pub(in super::super) fn lower_native_comparison(
        &mut self,
        path: &str,
        op: Operation,
        operands: &[NodeId],
    ) -> Result<Option<IrExpr>, String> {
        if !matches!(
            op,
            Operation::Equal | Operation::NotEqual | Operation::CaseEqual | Operation::CaseNotEqual
        ) {
            return Ok(None);
        }
        let [left, right] = operands else {
            return Ok(None);
        };
        let (left, right) = (self.p30_unwrap_cast(*left), self.p30_unwrap_cast(*right));
        let element = self.is_container_record(left) || self.is_container_record(right);
        if !element && self.native_path_of(left)?.is_none() && self.native_path_of(right)?.is_none()
        {
            return Ok(None);
        }
        let (left, right) = match (
            self.record_leaf_reads(path, left)?,
            self.record_leaf_reads(path, right)?,
        ) {
            (Some(left), Some(right)) => (left, right),
            // Scalar members of native values compare as scalars.
            (None, None) if !element => return Ok(None),
            _ => {
                return Err(format!(
                    "native record comparison in `{path}` needs record variables on both sides"
                ))
            }
        };
        if left.len() != right.len()
            || left.is_empty()
            || left.iter().zip(&right).any(|((a, _), (b, _))| a != b)
        {
            return Err(format!(
                "native record comparison in `{path}` has incompatible leaf layouts"
            ));
        }
        let case = matches!(op, Operation::CaseEqual | Operation::CaseNotEqual);
        let mut equality: Option<IrExpr> = None;
        for ((_, left), (_, right)) in left.into_iter().zip(right) {
            let leaf_equal = self.leaf_value_equality(path, left, right, case)?;
            equality = Some(match equality {
                Some(previous) => cmp_expr_ir(IrBinOp::LogAnd, previous, leaf_equal),
                None => leaf_equal,
            });
        }
        let equality = equality.ok_or("native record comparison has no leaves")?;
        Ok(Some(
            if matches!(op, Operation::NotEqual | Operation::CaseNotEqual) {
                IrExpr::new(
                    IrExprKind::Un {
                        op: IrUnOp::LogNot,
                        a: Box::new(equality),
                    },
                    1,
                    false,
                    None,
                )
            } else {
                equality
            },
        ))
    }

    /// One-bit equality of two scalar record leaves (SV 11.4.5): strings
    /// compare their bytes, chandles their pointers, packed and real leaves
    /// their values.
    pub(in super::super) fn native_leaf_equality(
        &mut self,
        path: &str,
        left: &NativeEndpointLeaf,
        right: &NativeEndpointLeaf,
        case: bool,
    ) -> Result<IrExpr, String> {
        let (left, right) = (
            self.endpoint_leaf_read(left)?,
            self.endpoint_leaf_read(right)?,
        );
        self.leaf_value_equality(path, left, right, case)
    }

    fn leaf_value_equality(
        &mut self,
        path: &str,
        left: LeafValue,
        right: LeafValue,
        case: bool,
    ) -> Result<IrExpr, String> {
        Ok(match (left, right) {
            (LeafValue::Container(a), LeafValue::Container(b)) => {
                if matches!(
                    self.model.containers[a].kind,
                    crate::sim::ir::IrContainerKind::Associative { .. }
                ) {
                    return Err(format!(
                        "equality of records with an associative array member is not supported in `{path}` (associative array equality is not supported)"
                    ));
                }
                // SV 7.2.2, 7.10: members compare element-wise.
                IrExpr::new(
                    IrExprKind::Container(Box::new(IrContainerExpr::Equal {
                        left: a,
                        right: b,
                        case,
                        negate: false,
                    })),
                    1,
                    false,
                    None,
                )
            }
            (LeafValue::Container(_), _) | (_, LeafValue::Container(_)) => {
                return Err(format!(
                "native record comparison in `{path}` pairs a container member with a scalar leaf"
            ))
            }
            (LeafValue::String(a), LeafValue::String(b)) => {
                let compare = IrExpr::new(
                    IrExprKind::ObjectQuery(Box::new(IrObjectQuery::StringCompare(a, b, false))),
                    32,
                    true,
                    None,
                );
                let zero = IrExpr::new(
                    IrExprKind::Const(
                        IrConst::packed(vec![0], vec![], vec![], 32, true, None)
                            .map_err(|error| error.to_string())?,
                    ),
                    32,
                    true,
                    None,
                );
                cmp_expr_ir(IrBinOp::Eq, compare, zero)
            }
            (LeafValue::Chandle(a), LeafValue::Chandle(b)) => IrExpr::new(
                IrExprKind::ObjectQuery(Box::new(IrObjectQuery::ChandleEq(a, b))),
                1,
                false,
                None,
            ),
            (
                LeafValue::Packed(a) | LeafValue::Real(a),
                LeafValue::Packed(b) | LeafValue::Real(b),
            ) => {
                if case && !a.is_real() && !b.is_real() {
                    cmp_expr_ir(IrBinOp::CaseEq, a, b)
                } else if case {
                    return Err(format!(
                        "case equality on real record member in `{path}` is not supported"
                    ));
                } else {
                    common_cmp_expr_ir(IrBinOp::Eq, a, b, path)?
                }
            }
            _ => {
                return Err(format!(
                    "native record comparison in `{path}` has mismatched member kinds"
                ))
            }
        })
    }

    /// The value of one scalar record leaf as a native leaf initializer.
    pub(in super::super) fn native_leaf_value(
        &mut self,
        leaf: &NativeEndpointLeaf,
    ) -> Result<IrNativeLeafExpr, String> {
        self.endpoint_leaf_read(leaf)?.into_leaf_expr()
    }

    /// Copy one scalar record leaf into another.
    pub(in super::super) fn native_leaf_copy(
        &mut self,
        path: &str,
        target: &NativeEndpointLeaf,
        source: &NativeEndpointLeaf,
        nba: bool,
    ) -> Result<IrStmt, String> {
        let value = self.endpoint_leaf_read(source)?;
        self.endpoint_leaf_write(path, target, value, nba)
    }

    /// Assign the value of expression `node` to one scalar record leaf.
    pub(in super::super) fn native_leaf_assign(
        &mut self,
        path: &str,
        target: &NativeEndpointLeaf,
        node: NodeId,
        nba: bool,
    ) -> Result<IrStmt, String> {
        let ty = self.endpoint_leaf_type(target)?;
        let value = self.native_leaf_source(path, ty, node)?;
        self.endpoint_leaf_write(path, target, value, nba)
    }

    /// Give one scalar record leaf its type's default-uninitialized value:
    /// an empty string, a null chandle, 0.0, or X (zero when two-state).
    pub(in super::super) fn native_leaf_reset(
        &mut self,
        path: &str,
        target: &NativeEndpointLeaf,
        nba: bool,
    ) -> Result<IrStmt, String> {
        let value = match self.endpoint_leaf_type(target)? {
            IrClassFieldType::String => LeafValue::String(IrStringExpr::Literal(Vec::new())),
            IrClassFieldType::Chandle => LeafValue::Chandle(IrChandleExpr::Null),
            IrClassFieldType::Real { .. } => LeafValue::Real(IrExpr::new(
                IrExprKind::Const(IrConst::real(0.0)),
                0,
                false,
                None,
            )),
            IrClassFieldType::Packed {
                width,
                signed,
                two_state,
            } => LeafValue::Packed(IrExpr::new(
                IrExprKind::Const(IrConst::integral_default(width, two_state)),
                width,
                signed,
                None,
            )),
        };
        self.endpoint_leaf_write(path, target, value, nba)
    }

    fn endpoint_leaf_type(&self, leaf: &NativeEndpointLeaf) -> Result<IrClassFieldType, String> {
        match leaf {
            NativeEndpointLeaf::Value(_, leaf) => Ok(leaf.ty),
            NativeEndpointLeaf::Container(_) => Err(CONTAINER_LEAF_UNSUPPORTED.to_owned()),
            NativeEndpointLeaf::Module(leaf) => {
                if let Some(object) = leaf.object {
                    return match self.model.objects[self.reference_object(object)].ty {
                        IrObjectType::String => Ok(IrClassFieldType::String),
                        IrObjectType::Chandle => Ok(IrClassFieldType::Chandle),
                        _ => Err("record member object is not a string or chandle".to_owned()),
                    };
                }
                let signal = leaf
                    .signal
                    .as_ref()
                    .ok_or("record member leaf has no scalar storage")?;
                Ok(if signal.real {
                    IrClassFieldType::Real {
                        shortreal: signal.shortreal,
                    }
                } else {
                    IrClassFieldType::Packed {
                        width: signal.width,
                        signed: signal.signed,
                        two_state: signal.two_state,
                    }
                })
            }
        }
    }

    /// Native operand of an expression call. Statement-level callers open a
    /// prelude for temporaries; elsewhere only native values pass directly.
    pub(in super::super) fn native_expression_argument(
        &mut self,
        path: &str,
        formal: NodeId,
        actual: NodeId,
    ) -> Result<IrCallArg, String> {
        let Some((mut before, mut after)) = self.native_call_prelude.take() else {
            let layout = self
                .native_layout(formal)?
                .ok_or("native formal has no layout")?;
            if let Some((value, prefix)) = self.native_path_of(self.p30_unwrap_cast(actual))? {
                if prefix.is_empty() && self.model.native_values[value].ty == layout.ty {
                    return Ok(IrCallArg::NativeValue(value));
                }
            }
            if matches!(
                self.kind(formal),
                NodeKind::FuncArg {
                    direction: DbDirection::Input,
                    ..
                }
            ) {
                if let Some(argument) = self.native_call_operand(path, &layout, actual)? {
                    return Ok(argument);
                }
                if let Some((leaves, containers)) =
                    self.native_input_leaves(path, &layout, actual)?
                {
                    return Ok(IrCallArg::NativeLeaves {
                        ty: layout.ty,
                        leaves,
                        containers,
                    });
                }
            }
            return Err(format!(
                "native record argument for `{}` in `{path}` must be a subroutine record variable unless the call is a whole statement or assignment",
                self.node(formal).name
            ));
        };
        let argument = self.native_call_argument(path, formal, actual, &mut before, &mut after);
        self.native_call_prelude = Some((before, after));
        argument
    }
}

/// One leaf of a transfer endpoint.
#[derive(Clone)]
pub(in super::super) enum NativeEndpointLeaf {
    Value(usize, NativeLeaf),
    Module(Box<AggregateMemberInfo>),
    /// A container member's storage: a native value's companion or a
    /// module record member's own container (SIM-007).
    Container(usize),
}
