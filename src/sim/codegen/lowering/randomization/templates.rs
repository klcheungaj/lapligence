//! Problems with units (SIM-030): random arrays, `foreach`, array methods,
//! `inside` an array and objects reached through `rand` handles.
//!
//! Such a problem is a template that the solver expands per object, array
//! element and loop iteration (`llg_constraint.h`, `constraints/expand.c`).
//! Unit 0 is the randomized object (or the scope of `std::randomize`);
//! every other unit is one class that an object reached through a `rand`
//! handle can have (18.4: such objects are randomized with their parent,
//! recursively). Variables, arrays and handle slots are problem-wide, one
//! per property, so a property has the same index in every unit.
//!
//! Constraint expressions still use the ordinary expression lowering:
//! before an expression is lowered, the forms the solver expands (array
//! elements, `size()`, reductions, `inside` an array, another object's
//! variable or null test, `item` and `item.index`) become template nodes,
//! and the expression lowering returns their placeholders for those nodes
//! (see `Codegen::random_placeholder`). The generated code feeds every
//! instance the runtime registers (variables, blocks, leaves, arrays,
//! handles), solves, then commits and runs `post_randomize` for every
//! random instance.

use super::problems::{placeholders_in, CallPlan, LeafPlan, ProblemBuilder};
use super::sites::{OrderSide, RandSelection, VarPlan};
use super::*;
use crate::core::db::{ConstraintBlockInfo, RandQualifier, VariableLifetime};
use crate::sim::ir::{
    IrContainerElement, IrContainerExpr, IrContainerKind, IrContainerStmt, IrInsideItem,
    IrNativeAccessKind, IrRandArray, IrRandArrayKind, IrRandOp, IrRandQuery, IrRandSlot,
    IrRandStmt, IrRandUnit, IrSysFunc, RAND_NONE, RAND_ORDER_ARRAY, RAND_ORDER_ELEM,
    RAND_SLOT_ARRAY,
};

/// Which arrays and handles of unit 0 are random (18.11).
#[derive(Clone)]
enum RootMode {
    /// Every `rand` property whose `rand_mode` is on.
    Default,
    /// Exactly these declarations (`randomize(list)`, `std::randomize`).
    List(Vec<NodeId>),
    /// `randomize(null)`.
    Checker,
}

/// One unit: a class layout (or the `std::randomize` scope).
struct UnitInfo {
    class: Option<NodeId>,
    chain: Vec<NodeId>,
    /// The object the unit's expressions read: the randomized object for
    /// unit 0, a local the instance walk assigns for the others.
    object: Option<IrChandleExpr>,
    /// Units after 0: their variables, block enables and hooks (unit 0 is
    /// fed by the ordinary problem code).
    vars: Vec<VarPlan>,
    blocks: Vec<IrExpr>,
    block_first: u32,
    block_count: u32,
    leaf_first: u32,
    leaf_count: u32,
    pre: Option<IrStmt>,
    post: Option<IrStmt>,
}

/// One array property (or scope array) of the problem.
struct ArrayInfo {
    decl: NodeId,
    /// Declaring class of an instance property; `None` for storage of the
    /// randomize call's scope (unit 0 only).
    owner: Option<NodeId>,
    /// The property's storage (aliased per object) or the scope container.
    container: usize,
    kind: IrRandArrayKind,
    left: i32,
    right: i32,
    width: u32,
    signed: bool,
    two_state: bool,
    /// The element type of a random array with illegal bit patterns.
    domain: Option<TypeDescriptor>,
}

/// One class handle property, or handle array, the problem follows.
struct SlotInfo {
    decl: NodeId,
    /// Declaring class of an instance property; `None` for an array of the
    /// randomize call's scope (unit 0 only).
    owner: Option<NodeId>,
    /// The handle's declared class.
    target: NodeId,
    /// Where a scalar handle lives.
    field: Option<(usize, usize)>,
    array: Option<SlotArray>,
}

/// The storage of a handle array.
struct SlotArray {
    container: usize,
    kind: IrRandArrayKind,
    left: i32,
    right: i32,
    /// The size-only array of a `rand` dynamic array or queue: 18.4 resizes
    /// it, and loops run over the solved size.
    size: Option<u32>,
}

/// A select of a template multidimensional fixed array.
struct ViewSelect {
    /// The expression naming the whole array.
    root: NodeId,
    array: u32,
    reference: u32,
    dimensions: Vec<(i32, i32)>,
    /// Translated leading indices.
    prefix: Vec<u32>,
}

/// Units, arrays and slots of one problem with units.
pub(super) struct UnitState {
    /// Prefix of the generated locals.
    tag: String,
    root: RootMode,
    units: Vec<UnitInfo>,
    unit_of_class: HashMap<NodeId, u32>,
    arrays: Vec<ArrayInfo>,
    array_of: HashMap<NodeId, u32>,
    slots: Vec<SlotInfo>,
    slot_of: HashMap<NodeId, u32>,
    /// Non-random packed properties that constraints read through a handle
    /// array element: inactive variables of their objects.
    state_vars: Vec<NodeId>,
    /// Array expressions a template form reads (not their storage).
    consumed: HashSet<NodeId>,
    /// Template placeholders created for, and read by, the constraint
    /// expression being lowered: a placeholder the lowering did not read
    /// means a form was lowered some other way.
    created: Vec<String>,
    used: HashSet<String>,
}

impl UnitState {
    pub(super) fn root_object(&self) -> Option<IrChandleExpr> {
        self.units[0].object.clone()
    }

    /// Leaves of unit 0 (the problem's first leaves).
    pub(super) fn root_leaf_count(&self) -> usize {
        self.units[0].leaf_count as usize
    }

    fn unit_has_slot(&self, unit: usize, slot: &SlotInfo) -> bool {
        match slot.owner {
            Some(owner) => self.units[unit].chain.contains(&owner),
            None => unit == 0,
        }
    }

    fn unit_has_array(&self, unit: usize, array: &ArrayInfo) -> bool {
        match array.owner {
            Some(owner) => self.units[unit].chain.contains(&owner),
            None => unit == 0,
        }
    }
}

/// Generated code of a problem with units (see `Codegen::unit_statements`).
pub(super) struct UnitCode {
    /// Feeds every unit after unit 0's variables.
    pub(super) walk: Vec<IrStmt>,
    /// Commits arrays and the other instances' variables.
    pub(super) commit: Vec<IrStmt>,
    /// The other instances' `post_randomize` calls.
    pub(super) posts: Vec<IrStmt>,
    /// The handle local of each unit after unit 0.
    pub(super) handles: HashMap<u32, String>,
}

/// One round over the calls of `TCall` nodes: every ready one reads its
/// arguments, names its object and sets its result.
pub(super) fn expanded_calls(
    problem: usize,
    tag: &str,
    calls: Vec<(u32, CallPlan)>,
    handles: &HashMap<u32, String>,
) -> Vec<IrStmt> {
    let counter = format!("{tag}_x");
    let site = format!("{tag}_xs");
    let index = || local_read(&counter, 32, true);
    let mut cases = Vec::with_capacity(calls.len());
    for (call, plan) in calls {
        let mut value = plan.template;
        let args = plan.expanded.unwrap_or_default();
        crate::sim::opt::walk_expr_mut(&mut value, &mut |expr: &mut IrExpr| {
            let IrExprKind::LocalRead(name) = &expr.kind else {
                return;
            };
            let Some(position) = args.iter().position(|arg| arg.placeholder == *name) else {
                return;
            };
            let arg = &args[position];
            *expr = query(
                problem,
                IrRandQuery::CallArg(position as u32),
                Some(index()),
                None,
                arg.width,
                arg.signed,
            );
        });
        let mut body = Vec::new();
        if let Some(handle) = handles.get(&plan.unit) {
            body.push(IrStmt::Object(Box::new(IrObjectStmt::ChandleAssignLocal(
                handle.clone(),
                IrChandleExpr::RandInstance {
                    problem,
                    inst: Box::new(query(
                        problem,
                        IrRandQuery::CallInst,
                        Some(index()),
                        None,
                        32,
                        true,
                    )),
                },
            ))));
        }
        body.push(IrStmt::Randomize(Box::new(IrRandStmt::ISetCallResult {
            problem,
            call: index(),
            site: call,
            value,
        })));
        cases.push((call, body));
    }
    let mut ready = vec![assign32(
        &site,
        query(
            problem,
            IrRandQuery::CallSite,
            Some(index()),
            None,
            32,
            true,
        ),
    )];
    ready.extend(dispatch(&site, cases));
    vec![
        decl32(&counter, int_const(0)),
        decl32(&site, int_const(0)),
        counted(
            &counter,
            query(problem, IrRandQuery::CallCount, None, None, 32, true),
            vec![when(
                query(
                    problem,
                    IrRandQuery::CallReady,
                    Some(index()),
                    None,
                    1,
                    false,
                ),
                ready,
            )],
        ),
    ]
}

fn state(builder: &mut ProblemBuilder) -> &mut UnitState {
    builder
        .units
        .as_deref_mut()
        .expect("a problem with units has a unit state")
}

fn int_const(value: i64) -> IrExpr {
    packed_const((value as u64) & 0xffff_ffff, 32, true)
}

/// `a op b` on two 32-bit signed operands.
fn bin32(op: IrBinOp, a: IrExpr, b: IrExpr) -> IrExpr {
    let (width, signed) = match op {
        IrBinOp::Eq | IrBinOp::Neq | IrBinOp::Lt => (1, false),
        _ => (32, true),
    };
    IrExpr::new(
        IrExprKind::Bin {
            op,
            a: Box::new(a),
            b: Box::new(b),
        },
        width,
        signed,
        None,
    )
}

fn local_lhs(name: &str) -> IrLhs {
    IrLhs::WholeRef {
        addr: format!("&{name}"),
        width: 32,
        signed: true,
        two_state: true,
        shortreal: false,
    }
}

fn decl32(name: &str, init: IrExpr) -> IrStmt {
    IrStmt::DeclLocal {
        name: name.to_owned(),
        width: 32,
        signed: true,
        init: Some(Box::new(init)),
        two_state: true,
    }
}

fn assign32(name: &str, value: IrExpr) -> IrStmt {
    IrStmt::Assign {
        lhs: local_lhs(name),
        rhs: value,
        nba: false,
    }
}

/// `for (counter = 0; counter < count; counter++) body`.
fn counted(counter: &str, count: IrExpr, body: Vec<IrStmt>) -> IrStmt {
    let read = || local_read(counter, 32, true);
    IrStmt::For {
        init: vec![assign32(counter, int_const(0))],
        cond: bin32(IrBinOp::Lt, read(), count),
        incr: vec![assign32(counter, bin32(IrBinOp::Add, read(), int_const(1)))],
        body,
    }
}

fn when(cond: IrExpr, then_: Vec<IrStmt>) -> IrStmt {
    IrStmt::If {
        cond,
        then_,
        els: None,
        check: IrUniquePriorityCheck::None,
    }
}

/// `if (value == 0) bodies[0] else if (value == 1) ...` over `(value, body)`.
fn dispatch(selector: &str, cases: Vec<(u32, Vec<IrStmt>)>) -> Option<IrStmt> {
    let mut chain: Option<IrStmt> = None;
    for (value, body) in cases.into_iter().rev() {
        chain = Some(IrStmt::If {
            cond: bin32(
                IrBinOp::Eq,
                local_read(selector, 32, true),
                int_const(i64::from(value)),
            ),
            then_: body,
            els: chain.map(|statement| vec![statement]),
            check: IrUniquePriorityCheck::None,
        });
    }
    chain
}

fn query(
    problem: usize,
    query: IrRandQuery,
    inst: Option<IrExpr>,
    index: Option<IrExpr>,
    width: u32,
    signed: bool,
) -> IrExpr {
    IrExpr::new(
        IrExprKind::SysFunc(Box::new(IrSysFunc::RandInst {
            problem,
            query,
            inst: inst.map(Box::new),
            index: index.map(Box::new),
        })),
        width,
        signed,
        None,
    )
}

fn container_read(operation: IrContainerExpr, width: u32, signed: bool) -> IrExpr {
    IrExpr::new(
        IrExprKind::Container(Box::new(operation)),
        width,
        signed,
        None,
    )
}

fn inside_item_exprs(item: &IrInsideItem) -> Vec<&IrExpr> {
    match item {
        IrInsideItem::Value(value) => vec![value],
        IrInsideItem::Range { low, high } => vec![low, high],
        IrInsideItem::OpenRange { low, high } => low.iter().chain(high.iter()).collect(),
        _ => Vec::new(),
    }
}

impl Codegen<'_> {
    /// Whether `decl` is a class handle property.
    fn is_handle_property(&self, decl: NodeId) -> bool {
        matches!(self.kind(decl), NodeKind::Var { ty } if ty.kind == "class")
    }

    /// Whether `decl` is an unpacked array of class handles.
    fn is_handle_array(&self, decl: NodeId) -> bool {
        self.db.array_meta(decl).is_some()
            && self.array_element_kind(decl).as_deref() == Some("class")
    }

    /// The class of the elements of handle array `decl`.
    fn handle_array_class(&self, decl: NodeId) -> Option<NodeId> {
        match &self.db.type_descriptor(decl)?.shape {
            TypeShape::Container { element, .. } | TypeShape::FixedArray { element, .. } => {
                self.db.class_for_type(element.id)
            }
            _ => None,
        }
    }

    /// The unpacked array declaration an expression names (`a`, `this.a`,
    /// `h.a`).
    fn array_expr_decl(&self, expr: NodeId) -> Option<NodeId> {
        let decl = match self.kind(expr) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => *target,
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => refs.last().copied().flatten()?,
            _ => return None,
        };
        self.db.array_meta(decl).is_some().then_some(decl)
    }

    /// Whether a subtree of `node` names a declaration of `randoms`.
    fn db_reads(&self, node: NodeId, randoms: &HashSet<NodeId>) -> bool {
        let mut stack = vec![node];
        while let Some(current) = stack.pop() {
            match self.kind(current) {
                NodeKind::Expr(ExprKind::Ref {
                    target: Some(target),
                }) if randoms.contains(target) => return true,
                NodeKind::Expr(ExprKind::HierPath { refs, .. })
                    if refs.iter().flatten().any(|target| randoms.contains(target)) =>
                {
                    return true
                }
                _ => {}
            }
            stack.extend(self.node(current).children.iter().copied());
        }
        false
    }

    /// Whether constraint `root` needs a problem with units: a `foreach`,
    /// or an array element, `with` reduction or `inside` an array whose
    /// value depends on a random variable of `randoms`.
    pub(super) fn constraint_needs_units(&self, root: NodeId, randoms: &HashSet<NodeId>) -> bool {
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            match self.kind(node) {
                NodeKind::Constraint(ConstraintKind::Foreach { .. }) => return true,
                NodeKind::Expr(ExprKind::ArraySelect { base, indices })
                    if self.db.array_meta(*base).is_some()
                        && indices.iter().any(|index| self.db_reads(*index, randoms)) =>
                {
                    return true
                }
                NodeKind::MethodCall {
                    receiver: Some(receiver),
                    ..
                } if self.db.method_call_has_with_clause(node)
                    && self.array_expr_decl(*receiver).is_some()
                    && self
                        .node(node)
                        .children
                        .iter()
                        .any(|child| child != receiver && self.db_reads(*child, randoms)) =>
                {
                    return true
                }
                NodeKind::Expr(ExprKind::Operation {
                    op: Operation::Inside,
                    operands,
                    ..
                }) if operands.len() > 1
                    && operands[1..]
                        .iter()
                        .any(|item| self.array_expr_decl(*item).is_some())
                    && self.db_reads(operands[0], randoms) =>
                {
                    return true
                }
                _ => {}
            }
            stack.extend(self.node(node).children.iter().copied());
        }
        false
    }

    /// Whether a class randomize call needs a problem with units: a random
    /// array or `rand` handle, or a constraint form that does.
    pub(super) fn class_needs_units(
        &self,
        chain: &[NodeId],
        effective: &[(NodeId, ConstraintBlockInfo)],
        inline: Option<NodeId>,
        selection: &RandSelection,
    ) -> bool {
        let listed: &[NodeId] = match selection {
            RandSelection::List(declarations) => declarations,
            _ => &[],
        };
        let mut randoms = HashSet::new();
        for member in chain {
            for child in self.node(*member).children.iter().copied() {
                if !matches!(
                    self.kind(child),
                    NodeKind::Var { .. } | NodeKind::Array { .. }
                ) || (self.db.rand_qualifier(child) == RandQualifier::None
                    && !listed.contains(&child))
                {
                    continue;
                }
                if self.db.array_meta(child).is_some() || self.is_handle_property(child) {
                    return true;
                }
                randoms.insert(child);
            }
        }
        effective
            .iter()
            .filter(|(_, info)| !info.is_pure)
            .filter_map(|(_, info)| info.body)
            .chain(inline)
            .any(|body| self.constraint_needs_units(body, &randoms))
    }

    /// The unit state of a new problem; unit 0 is `class` (`None` for
    /// `std::randomize`) read through `object`.
    pub(super) fn unit_state(
        &self,
        tag: String,
        class: Option<NodeId>,
        object: Option<IrChandleExpr>,
        selection: &RandSelection,
    ) -> Box<UnitState> {
        let root = match selection {
            RandSelection::Default => RootMode::Default,
            RandSelection::List(declarations) => RootMode::List(declarations.clone()),
            RandSelection::Checker => RootMode::Checker,
        };
        Box::new(UnitState {
            tag,
            root,
            units: vec![UnitInfo {
                class,
                chain: class
                    .map(|class| self.class_chain(class))
                    .unwrap_or_default(),
                object,
                vars: Vec::new(),
                blocks: Vec::new(),
                block_first: 0,
                block_count: 0,
                leaf_first: 0,
                leaf_count: 0,
                pre: None,
                post: None,
            }],
            unit_of_class: HashMap::new(),
            arrays: Vec::new(),
            array_of: HashMap::new(),
            slots: Vec::new(),
            slot_of: HashMap::new(),
            state_vars: Vec::new(),
            consumed: HashSet::new(),
            created: Vec::new(),
            used: HashSet::new(),
        })
    }

    /// Register an array or handle property of the randomized object's
    /// class; `false` for any other property.
    pub(super) fn root_unit_property(
        &mut self,
        path: &str,
        node: NodeId,
        builder: &mut ProblemBuilder,
        child: NodeId,
        randomized: bool,
    ) -> Result<bool, String> {
        if self.is_handle_array(child) {
            self.unit_slot(path, node, builder, child, None)?;
            return Ok(true);
        }
        if self.db.array_meta(child).is_some() {
            if self.db.rand_qualifier(child) == RandQualifier::RandC {
                return Err(self.rand_error(
                    node,
                    path,
                    &format!(
                        "`randc` array `{}` is not supported (S30-D3)",
                        self.node(child).name
                    ),
                ));
            }
            let random = randomized || self.db.rand_qualifier(child) != RandQualifier::None;
            self.unit_array(path, node, builder, child, None, random)?;
            return Ok(true);
        }
        if self.is_handle_property(child) {
            self.unit_slot(path, node, builder, child, None)?;
            return Ok(true);
        }
        Ok(false)
    }

    /// Register `std::randomize` array argument `argument`.
    pub(super) fn scope_unit_array(
        &mut self,
        path: &str,
        builder: &mut ProblemBuilder,
        argument: NodeId,
        target: NodeId,
    ) -> Result<(), String> {
        self.unit_array(path, argument, builder, target, Some(argument), true)
            .map(|_| ())
    }

    /// The type kind of the elements of array `decl`.
    fn array_element_kind(&self, decl: NodeId) -> Option<String> {
        match self
            .db
            .type_descriptor(decl)
            .map(|descriptor| &descriptor.shape)
        {
            Some(TypeShape::Container { element, .. })
            | Some(TypeShape::FixedArray { element, .. }) => Some(element.info.kind.clone()),
            _ => None,
        }
    }

    /// The element type of array `decl` when it has illegal bit patterns
    /// (an enum, S29-D2), which random elements must avoid.
    fn array_element_domain(&self, decl: NodeId) -> Option<TypeDescriptor> {
        match self
            .db
            .type_descriptor(decl)
            .map(|descriptor| &descriptor.shape)
        {
            Some(TypeShape::Container { element, .. })
            | Some(TypeShape::FixedArray { element, .. })
                if element.info.kind == "enum" || aggregate_needs_domain(element) =>
            {
                Some((**element).clone())
            }
            _ => None,
        }
    }

    /// The problem-wide array of declaration `decl`, registering it.
    /// `expr` names it (for storage of the call's scope); `random` says
    /// whether it can be randomized.
    fn unit_array(
        &mut self,
        path: &str,
        at: NodeId,
        builder: &mut ProblemBuilder,
        decl: NodeId,
        expr: Option<NodeId>,
        random: bool,
    ) -> Result<u32, String> {
        if let Some(index) = state(builder).array_of.get(&decl) {
            return Ok(*index);
        }
        let name = self.node(decl).name.clone();
        let unsupported = |this: &Self, why: &str| {
            this.rand_error(
                at,
                path,
                &format!("array `{name}` is not supported in constraints: {why}"),
            )
        };
        self.db
            .array_meta(decl)
            .ok_or_else(|| unsupported(self, "it is not an unpacked array"))?;
        let class_owner = self
            .node(decl)
            .parent
            .filter(|parent| self.class_nodes.contains_key(parent));
        let is_static = self.db.variable_lifetime(decl) == VariableLifetime::Static;
        let (owner, container) = match class_owner {
            Some(class) if !is_static => (
                Some(class),
                self.container_globals.get(&decl).map(|info| info.ir),
            ),
            _ => {
                if class_owner.is_some() && random {
                    return Err(unsupported(self, "a static random array is not supported"));
                }
                (
                    None,
                    expr.and_then(|expr| self.container_of(expr))
                        .map(|info| info.ir)
                        .or_else(|| self.container_globals.get(&decl).map(|info| info.ir)),
                )
            }
        };
        let container = container.ok_or_else(|| {
            unsupported(
                self,
                "only class properties and dynamic arrays and queues of the calling scope are supported",
            )
        })?;
        let flat_cells = self.fixed_view_dims.get(&container).map(|dimensions| {
            dimensions
                .iter()
                .map(|(left, right)| (i64::from(*left) - i64::from(*right)).abs() + 1)
                .product::<i64>()
        });
        let storage = &self.model.containers[container];
        let IrContainerElement::Packed {
            width,
            signed,
            two_state,
        } = &storage.element
        else {
            return Err(unsupported(
                self,
                if self.array_element_kind(decl).as_deref() == Some("class") {
                    "arrays of class handles are not supported"
                } else {
                    "its elements are not packed integral values"
                },
            ));
        };
        let (width, signed, two_state) = (*width, *signed, *two_state);
        let (kind, left, right) = match &storage.kind {
            IrContainerKind::Queue { maximum_elements } => {
                // The size variable of a random bounded queue stays within
                // its bound: `right` holds the maximum element count.
                let bound = match (maximum_elements, random) {
                    (Some(maximum), true) => i32::try_from(*maximum)
                        .map_err(|_| unsupported(self, "its bound is too large"))?,
                    _ => 0,
                };
                (IrRandArrayKind::Queue, 0, bound)
            }
            IrContainerKind::Dynamic => {
                match (flat_cells, self.fixed_view_ranges.get(&container)) {
                    // The elements of a multidimensional array are one flat,
                    // row-major array; selects compute the flat position.
                    (Some(cells), _) => (
                        IrRandArrayKind::Fixed,
                        0,
                        i32::try_from(cells - 1)
                            .map_err(|_| unsupported(self, "it has too many elements"))?,
                    ),
                    (None, Some((left, right))) => (IrRandArrayKind::Fixed, *left, *right),
                    (None, None) => (IrRandArrayKind::Dynamic, 0, 0),
                }
            }
            IrContainerKind::Associative { .. } => {
                return Err(unsupported(self, "associative arrays are not supported"))
            }
        };
        let domain = if random {
            self.array_element_domain(decl)
        } else {
            None
        };
        if domain.as_ref().is_some_and(aggregate_domain_unsupported) {
            return Err(unsupported(
                self,
                "random elements of an untagged union with enum members are not supported",
            ));
        }
        let st = state(builder);
        let index = st.arrays.len() as u32;
        st.arrays.push(ArrayInfo {
            decl,
            owner,
            container,
            kind,
            left,
            right,
            width,
            signed,
            two_state,
            domain,
        });
        st.array_of.insert(decl, index);
        Ok(index)
    }

    /// The problem-wide slot of handle property or handle array `decl`;
    /// `expr` names a handle array of the call's scope.
    fn unit_slot(
        &mut self,
        path: &str,
        at: NodeId,
        builder: &mut ProblemBuilder,
        decl: NodeId,
        expr: Option<NodeId>,
    ) -> Result<u32, String> {
        if let Some(index) = state(builder).slot_of.get(&decl) {
            return Ok(*index);
        }
        let name = self.node(decl).name.clone();
        let is_array = self.is_handle_array(decl);
        let class_owner = self
            .node(decl)
            .parent
            .filter(|parent| self.class_nodes.contains_key(parent));
        let is_static = self.db.variable_lifetime(decl) == VariableLifetime::Static;
        if is_static && class_owner.is_some() && self.db.rand_qualifier(decl) != RandQualifier::None
        {
            return Err(self.rand_error(
                at,
                path,
                &format!("static `rand` class handle `{name}` is not supported"),
            ));
        }
        let target = if is_array {
            self.handle_array_class(decl)
        } else {
            self.class_node_for_type_expr(decl)
        };
        let missing = || format!("`rand` class handle `{name}` has no known class layout");
        let Some(target) = target else {
            return Err(self.rand_error(at, path, &missing()));
        };
        let (owner, field, array) = if is_array {
            let (owner, container) = match class_owner.filter(|_| !is_static) {
                Some(class) => (
                    Some(class),
                    self.container_globals.get(&decl).map(|info| info.ir),
                ),
                None => (
                    None,
                    expr.and_then(|expr| self.container_of(expr))
                        .map(|info| info.ir)
                        .or_else(|| self.container_globals.get(&decl).map(|info| info.ir)),
                ),
            };
            let Some(container) = container else {
                return Err(self.rand_error(at, path, &missing()));
            };
            if self.fixed_view_dims.contains_key(&container) {
                return Err(self.rand_error(
                    at,
                    path,
                    &format!("array `{name}` is not supported in constraints: it has more than one unpacked dimension"),
                ));
            }
            let (kind, left, right) = match &self.model.containers[container].kind {
                IrContainerKind::Queue { .. } => (IrRandArrayKind::Queue, 0, 0),
                IrContainerKind::Dynamic => match self.fixed_view_ranges.get(&container) {
                    Some((left, right)) => (IrRandArrayKind::Fixed, *left, *right),
                    None => (IrRandArrayKind::Dynamic, 0, 0),
                },
                IrContainerKind::Associative { .. } => {
                    return Err(self.rand_error(
                        at,
                        path,
                        &format!("array `{name}` is not supported in constraints: associative arrays are not supported"),
                    ))
                }
            };
            let rand = self.db.rand_qualifier(decl) != RandQualifier::None;
            let size = (rand && kind != IrRandArrayKind::Fixed).then(|| {
                let st = state(builder);
                st.arrays.push(ArrayInfo {
                    decl,
                    owner,
                    container,
                    kind: IrRandArrayKind::Size,
                    left: 0,
                    right: 0,
                    width: 1,
                    signed: false,
                    two_state: true,
                    domain: None,
                });
                (st.arrays.len() - 1) as u32
            });
            (
                owner,
                None,
                Some(SlotArray {
                    container,
                    kind,
                    left,
                    right,
                    size,
                }),
            )
        } else {
            let Some(field) = self.class_fields.get(&decl).copied() else {
                return Err(self.rand_error(at, path, &missing()));
            };
            (self.node(decl).parent, Some(field), None)
        };
        let st = state(builder);
        let index = st.slots.len() as u32;
        st.slots.push(SlotInfo {
            decl,
            owner,
            target,
            field,
            array,
        });
        st.slot_of.insert(decl, index);
        Ok(index)
    }

    /// The problem-wide variable of `rand` property `decl` of another
    /// object, allocating it before its unit is built.
    fn unit_var(
        &mut self,
        path: &str,
        at: NodeId,
        builder: &mut ProblemBuilder,
        decl: NodeId,
    ) -> Result<u32, String> {
        if let Some(index) = builder.decl_vars.get(&decl) {
            return Ok(*index);
        }
        let name = self.node(decl).name.clone();
        let packed = self
            .class_fields
            .get(&decl)
            .copied()
            .and_then(
                |(class, field)| match self.model.classes[class].fields[field].ty {
                    IrClassFieldType::Packed { width, signed, .. } => Some((width, signed)),
                    _ => None,
                },
            )
            .filter(|_| {
                !self.random_aggregate_unsupported(decl)
                    && !self.class_native_fields.contains_key(&decl)
            });
        let Some((width, signed)) = packed else {
            return Err(self.rand_error(
                at,
                path,
                &format!("random property `{name}` of another object is not a packed integral instance property"),
            ));
        };
        let (index, _) = builder.add_var(name, width, signed);
        builder.decl_vars.insert(decl, index);
        if self.db.rand_qualifier(decl) == RandQualifier::None {
            state(builder).state_vars.push(decl);
        }
        if let Some(metadata) = self.enum_metadata_for_expr(decl).cloned() {
            let values = metadata
                .members
                .iter()
                .map(|member| match &member.value {
                    Val::Bits(value) => val_to_const(value),
                    _ => Err("enum member has a non-integral value".to_owned()),
                })
                .collect::<Result<Vec<_>, _>>()?;
            builder.set_domain(index, &values)?;
        }
        self.aggregate_var_domain(builder, index, decl)?;
        if self.randc_states.contains_key(&decl) {
            builder.set_cyclic(index);
        }
        Ok(index)
    }

    /// The array expression an `ArraySelect` selects from (its `base` field
    /// names the declaration).
    fn select_base(&self, node: NodeId) -> Option<NodeId> {
        let NodeKind::Expr(ExprKind::ArraySelect { indices, .. }) = self.kind(node) else {
            return None;
        };
        self.node(node)
            .children
            .iter()
            .copied()
            .find(|child| !indices.contains(child))
    }

    /// Whether `item`, a step of a handle path, selects an element of a
    /// handle array, or is the iterator over one.
    fn element_step(&self, item: NodeId) -> bool {
        matches!(
            self.kind(item),
            NodeKind::Expr(ExprKind::ArraySelect { .. })
        ) || self
            .rand_lowering
            .as_ref()
            .is_some_and(|lowering| lowering.element_refs.contains_key(&item))
    }

    /// Whether handle path `handles` can be followed: every step is a
    /// `rand` handle, a handle the problem already follows or an element
    /// of a handle array that is `rand`, followed, or read at a loop index.
    fn slot_path_known(&self, builder: &ProblemBuilder, handles: &[NodeId]) -> bool {
        let unit = builder.unit as usize;
        let st = builder.units.as_deref().expect("unit state");
        for (position, item) in handles.iter().copied().enumerate() {
            if self
                .rand_lowering
                .as_ref()
                .is_some_and(|lowering| lowering.element_refs.contains_key(&item))
            {
                continue;
            }
            let decl = match self.kind(item) {
                NodeKind::Expr(ExprKind::ArraySelect { indices, .. }) => {
                    let [index] = indices.as_slice() else {
                        return false;
                    };
                    let Some(base) = self.select_base(item) else {
                        return false;
                    };
                    let Some(decl) = self
                        .array_expr_decl(base)
                        .filter(|decl| self.is_handle_array(*decl))
                    else {
                        return false;
                    };
                    if self.db.rand_qualifier(decl) == RandQualifier::None
                        && !st.slot_of.contains_key(&decl)
                        && !self.template_reads(*index)
                    {
                        return false;
                    }
                    let Some(decls) = self.path_decls(base) else {
                        return false;
                    };
                    let (_, prefix) = decls.split_last().expect("a path has a declaration");
                    if !self.slot_path_known(builder, prefix) {
                        return false;
                    }
                    if !prefix.is_empty() {
                        continue;
                    }
                    decl
                }
                _ => {
                    if !st.slot_of.contains_key(&item)
                        && !(self.is_handle_property(item)
                            && self.db.rand_qualifier(item) != RandQualifier::None)
                    {
                        return false;
                    }
                    item
                }
            };
            if position == 0 {
                let owner = self.node(decl).parent;
                let scope_array = self.is_handle_array(decl)
                    && !owner.is_some_and(|owner| self.class_nodes.contains_key(&owner));
                let in_unit = owner.is_some_and(|owner| st.units[unit].chain.contains(&owner));
                if !in_unit && !(scope_array && unit == 0) {
                    return false;
                }
            }
        }
        true
    }

    /// The template object of slot `slot`'s element at index node `index`
    /// of the object `reference` names.
    fn slot_element(
        &self,
        builder: &mut ProblemBuilder,
        slot: u32,
        reference: u32,
        index: u32,
    ) -> Result<u32, String> {
        let st = builder.units.as_deref().expect("unit state");
        let array = st.slots[slot as usize]
            .array
            .as_ref()
            .expect("an element of a handle array");
        let (kind, left, right) = (array.kind, array.left, array.right);
        let position = builder.position(kind, left, right, index)?;
        Ok(builder.template(IrRandOp::Ref, false, 1, [slot, reference, position]))
    }

    /// The template reference of handle path `handles` (the first a
    /// property of the current unit's class), or `None` when some step
    /// cannot be followed.
    fn slot_reference(
        &mut self,
        path: &str,
        at: NodeId,
        builder: &mut ProblemBuilder,
        handles: &[NodeId],
    ) -> Result<Option<u32>, String> {
        if !self.slot_path_known(builder, handles) {
            return Ok(None);
        }
        let mut reference = RAND_NONE;
        for item in handles.iter().copied() {
            if let Some(element) = self
                .rand_lowering
                .as_ref()
                .and_then(|lowering| lowering.element_refs.get(&item))
            {
                reference = *element;
                continue;
            }
            let NodeKind::Expr(ExprKind::ArraySelect { indices, .. }) = self.kind(item) else {
                let slot = self.unit_slot(path, at, builder, item, None)?;
                reference = builder.template(IrRandOp::Ref, false, 1, [slot, reference, RAND_NONE]);
                continue;
            };
            let index = indices[0];
            let Some(base) = self.select_base(item) else {
                return Ok(None);
            };
            let Some((array, parent)) = self.handle_array(path, builder, base)? else {
                return Ok(None);
            };
            state(builder).consumed.insert(base);
            let (index, lowered) = self.template_operand(path, builder, index)?;
            let loops_only = self.rand_lowering.as_ref().is_some_and(|lowering| {
                let bound = lowering
                    .bindings
                    .values()
                    .flat_map(placeholders_in)
                    .collect::<HashSet<_>>();
                placeholders_in(&lowered)
                    .iter()
                    .all(|name| bound.contains(name))
            });
            if !loops_only {
                return Err(self.rand_error(
                    item,
                    path,
                    "a handle array element in a constraint needs a constant, state or loop index (18.4)",
                ));
            }
            reference = self
                .slot_element(builder, array, parent, index)
                .map_err(|error| self.rand_error(item, path, &error))?;
        }
        Ok(Some(reference))
    }

    /// The slot and owner reference of the handle array `expr` names.
    fn handle_array(
        &mut self,
        path: &str,
        builder: &mut ProblemBuilder,
        expr: NodeId,
    ) -> Result<Option<(u32, u32)>, String> {
        let Some(decls) = self.path_decls(expr) else {
            return Ok(None);
        };
        let Some((&decl, handles)) = decls.split_last() else {
            return Ok(None);
        };
        if !self.is_handle_array(decl) {
            return Ok(None);
        }
        if handles.is_empty() {
            let unit = builder.unit as usize;
            let in_unit = self
                .node(decl)
                .parent
                .is_some_and(|owner| state(builder).units[unit].chain.contains(&owner));
            let scope = !self
                .node(decl)
                .parent
                .is_some_and(|owner| self.class_nodes.contains_key(&owner));
            if !in_unit && !(scope && unit == 0) {
                return Ok(None);
            }
        }
        let Some(reference) = self.slot_reference(path, expr, builder, handles)? else {
            return Ok(None);
        };
        let slot = self.unit_slot(path, expr, builder, decl, Some(expr))?;
        Ok(Some((slot, reference)))
    }

    /// The solver's array argument of handle array slot `slot`: its size
    /// array when randomize can resize it.
    fn slot_array_operand(builder: &ProblemBuilder, slot: u32) -> u32 {
        let st = builder.units.as_deref().expect("unit state");
        match st.slots[slot as usize]
            .array
            .as_ref()
            .and_then(|array| array.size)
        {
            Some(size) => size,
            None => slot | RAND_SLOT_ARRAY,
        }
    }

    /// The declarations of a path expression with a leading constraint
    /// `this` removed.
    fn path_decls(&self, expr: NodeId) -> Option<Vec<NodeId>> {
        let mut refs = match self.kind(expr) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => vec![*target],
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => {
                refs.iter().copied().collect::<Option<Vec<_>>>()?
            }
            NodeKind::Expr(ExprKind::ArraySelect { .. }) => vec![expr],
            _ => return None,
        };
        if refs.len() > 1
            && self.node(refs[0]).name == "this"
            && self.rand_this(refs[0])
            && self
                .rand_lowering
                .as_ref()
                .is_some_and(|lowering| lowering.receiver.is_some())
        {
            refs.remove(0);
        }
        Some(refs)
    }

    /// The array and template reference an array expression names, when it
    /// is (or, with `create`, becomes) a template array.
    fn template_array(
        &mut self,
        path: &str,
        builder: &mut ProblemBuilder,
        expr: NodeId,
        create: bool,
    ) -> Result<Option<(u32, u32)>, String> {
        let Some(decls) = self.path_decls(expr) else {
            return Ok(None);
        };
        let Some((&decl, handles)) = decls.split_last() else {
            return Ok(None);
        };
        if self.db.array_meta(decl).is_none() {
            return Ok(None);
        }
        if self.is_handle_array(decl) {
            let followed = state(builder).slot_of.contains_key(&decl)
                || self.db.rand_qualifier(decl) != RandQualifier::None;
            if !followed && !create {
                return Ok(None);
            }
            return Ok(self
                .handle_array(path, builder, expr)?
                .map(|(slot, reference)| (Self::slot_array_operand(builder, slot), reference)));
        }
        let known = state(builder).array_of.get(&decl).copied();
        let rand = self.db.rand_qualifier(decl) != RandQualifier::None;
        if known.is_none() && !rand && !create {
            return Ok(None);
        }
        let Some(reference) = self.slot_reference(path, expr, builder, handles)? else {
            return Ok(None);
        };
        if handles.is_empty() {
            let unit = builder.unit as usize;
            let class_owner = self
                .node(decl)
                .parent
                .filter(|parent| self.class_nodes.contains_key(parent));
            let instance = class_owner.is_some()
                && self.db.variable_lifetime(decl) != VariableLifetime::Static;
            if instance
                && !class_owner
                    .is_some_and(|owner| state(builder).units[unit].chain.contains(&owner))
            {
                return Ok(None);
            }
            if !instance && unit != 0 {
                return Err(self.rand_error(
                    expr,
                    path,
                    "an array of another scope in a constraint of an object reached through a `rand` handle is not supported",
                ));
            }
        }
        let index = match known {
            Some(index) => index,
            None => self.unit_array(path, expr, builder, decl, Some(expr), rand)?,
        };
        Ok(Some((index, reference)))
    }

    /// Whether `expr` names an array that is (or must be) a template array.
    fn template_array_known(&self, builder: &ProblemBuilder, expr: NodeId) -> bool {
        self.array_expr_decl(expr).is_some_and(|decl| {
            builder
                .units
                .as_ref()
                .is_some_and(|st| st.array_of.contains_key(&decl))
                || self.db.rand_qualifier(decl) != RandQualifier::None
        })
    }

    /// Whether the value of `node` depends on the solution: it reads a
    /// random variable, a loop index, a template form, a random array or a
    /// member of another random object.
    fn template_reads(&self, node: NodeId) -> bool {
        let Some(lowering) = self.rand_lowering.as_ref() else {
            return false;
        };
        let mut stack = vec![node];
        while let Some(current) = stack.pop() {
            if lowering.templates.contains_key(&current) {
                return true;
            }
            match self.kind(current) {
                NodeKind::Expr(ExprKind::Ref {
                    target: Some(target),
                }) => {
                    if lowering.placeholders.contains_key(target)
                        || lowering.bindings.contains_key(target)
                        || (self.db.array_meta(*target).is_some()
                            && self.db.rand_qualifier(*target) != RandQualifier::None)
                    {
                        return true;
                    }
                }
                NodeKind::Expr(ExprKind::HierPath { refs, .. }) => {
                    let refs = refs.iter().flatten().copied().collect::<Vec<_>>();
                    if refs
                        .last()
                        .is_some_and(|last| lowering.placeholders.contains_key(last))
                        || refs.split_last().is_some_and(|(_, handles)| {
                            handles.iter().any(|handle| {
                                self.is_handle_property(*handle)
                                    && self.db.rand_qualifier(*handle) != RandQualifier::None
                            })
                        })
                    {
                        return true;
                    }
                }
                _ => {}
            }
            stack.extend(self.node(current).children.iter().copied());
        }
        false
    }

    /// Placeholder of template node `template` for expression `node`.
    fn register_template(&mut self, builder: &mut ProblemBuilder, node: NodeId, template: u32) {
        let placeholder = builder.template_placeholder(template);
        if let IrExprKind::LocalRead(name) = &placeholder.kind {
            state(builder).created.push(name.clone());
        }
        if let Some(lowering) = self.rand_lowering.as_mut() {
            lowering.templates.insert(node, placeholder);
        }
    }

    /// Lower operand `node` of a template form (its own forms already
    /// registered) and translate it.
    fn template_operand(
        &mut self,
        path: &str,
        builder: &mut ProblemBuilder,
        node: NodeId,
    ) -> Result<(u32, IrExpr), String> {
        let lowered = self
            .lower_expr(path, node)
            .map_err(|error| self.rand_error(node, path, &format!("in a constraint: {error}")))?;
        state(builder).used.extend(placeholders_in(&lowered));
        let translated = builder
            .expr(&lowered)
            .map_err(|error| self.rand_error(node, path, &error))?;
        Ok((translated, lowered))
    }

    /// An element of `array` at the template loop index `iter`.
    fn loop_element(
        &self,
        builder: &mut ProblemBuilder,
        array: u32,
        reference: u32,
        iter: u32,
    ) -> Result<u32, String> {
        let st = builder.units.as_deref().expect("unit state");
        let info = &st.arrays[array as usize];
        let (kind, left, right, width, signed) =
            (info.kind, info.left, info.right, info.width, info.signed);
        let position = builder.position(kind, left, right, iter)?;
        Ok(builder.template(IrRandOp::AElem, signed, width, [array, position, reference]))
    }

    /// Turn the solver-expanded forms of constraint expression `node` into
    /// template nodes before it is lowered (problems with units only).
    pub(super) fn template_forms(
        &mut self,
        path: &str,
        builder: &mut ProblemBuilder,
        node: NodeId,
    ) -> Result<(), String> {
        let Some(st) = builder.units.as_deref_mut() else {
            return Ok(());
        };
        st.created.clear();
        st.used.clear();
        self.template_visit(path, builder, node)
    }

    fn template_visit(
        &mut self,
        path: &str,
        builder: &mut ProblemBuilder,
        node: NodeId,
    ) -> Result<(), String> {
        if self
            .rand_lowering
            .as_ref()
            .is_some_and(|lowering| lowering.templates.contains_key(&node))
        {
            return Ok(());
        }
        let method = match self.kind(node) {
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
                ..
            } => Some((name.clone(), *receiver)),
            _ => None,
        };
        if let Some((name, receiver)) = method {
            if self.template_method(path, builder, node, &name, receiver)? {
                return Ok(());
            }
        }
        for child in self.node(node).children.clone() {
            if matches!(
                self.kind(child),
                NodeKind::Expr(_)
                    | NodeKind::MethodCall { .. }
                    | NodeKind::SysCall { .. }
                    | NodeKind::FuncCall { .. }
            ) {
                self.template_visit(path, builder, child)?;
            }
        }
        match self.kind(node) {
            NodeKind::Expr(ExprKind::ArraySelect { indices, .. }) => {
                let indices = indices.clone();
                self.template_select(path, builder, node, &indices)
            }
            NodeKind::Expr(ExprKind::HierPath { .. }) => self.template_member(path, builder, node),
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Inside,
                operands,
                ..
            }) => {
                let operands = operands.clone();
                self.template_inside(path, builder, node, &operands)
            }
            NodeKind::Expr(ExprKind::Operation {
                op:
                    op @ (Operation::Equal
                    | Operation::NotEqual
                    | Operation::CaseEqual
                    | Operation::CaseNotEqual),
                operands,
                ..
            }) if operands.len() == 2 => {
                let negate = matches!(op, Operation::NotEqual | Operation::CaseNotEqual);
                let operands = [operands[0], operands[1]];
                self.template_null_test(path, builder, node, operands, negate)
            }
            _ => Ok(()),
        }
    }

    /// The array expression below a chain of selects (`h.m` of `h.m[i][j]`).
    fn select_root(&self, select: NodeId) -> Option<NodeId> {
        let mut node = select;
        while matches!(
            self.kind(node),
            NodeKind::Expr(ExprKind::ArraySelect { .. })
        ) {
            node = self.select_base(node)?;
        }
        Some(node)
    }

    /// `expr` as a select of a template multidimensional fixed array: the
    /// array itself or some leading indices of it.
    fn view_select(
        &mut self,
        path: &str,
        builder: &mut ProblemBuilder,
        expr: NodeId,
        create: bool,
    ) -> Result<Option<ViewSelect>, String> {
        let (root, indices) = match self.kind(expr) {
            NodeKind::Expr(ExprKind::ArraySelect { indices, .. }) => {
                let indices = indices.clone();
                let Some(root) = self.select_root(expr) else {
                    return Ok(None);
                };
                (root, indices)
            }
            _ => (expr, Vec::new()),
        };
        let Some(decl) = self.array_expr_decl(root) else {
            return Ok(None);
        };
        let multidimensional = self.db.array_meta(decl).is_some_and(|meta| {
            matches!(meta.kind(), crate::core::db::ArrayKind::Static) && meta.dimensions().len() > 1
        });
        if !multidimensional || self.is_handle_array(decl) {
            return Ok(None);
        }
        let Some((array, reference)) = self.template_array(path, builder, root, create)? else {
            return Ok(None);
        };
        let container = state(builder).arrays[array as usize].container;
        let Some(dimensions) = self.fixed_view_dims.get(&container).cloned() else {
            return Ok(None);
        };
        if indices.len() > dimensions.len() {
            return Ok(None);
        }
        let mut prefix = Vec::with_capacity(indices.len());
        for index in indices {
            prefix.push(self.template_operand(path, builder, index)?.0);
        }
        Ok(Some(ViewSelect {
            root,
            array,
            reference,
            dimensions,
            prefix,
        }))
    }

    /// The element of `view` at its leading indices, then `trailing`.
    fn view_element(
        &self,
        node: NodeId,
        path: &str,
        builder: &mut ProblemBuilder,
        view: &ViewSelect,
        trailing: &[u32],
    ) -> Result<u32, String> {
        let indices = [view.prefix.as_slice(), trailing].concat();
        let position = builder
            .view_position(&view.dimensions, &indices)
            .map_err(|error| self.rand_error(node, path, &error))?;
        let st = builder.units.as_deref().expect("unit state");
        let info = &st.arrays[view.array as usize];
        let (width, signed) = (info.width, info.signed);
        Ok(builder.template(
            IrRandOp::AElem,
            signed,
            width,
            [view.array, position, view.reference],
        ))
    }

    /// `a[index]` of a template array (S30-D11 for out-of-range indices).
    fn template_select(
        &mut self,
        path: &str,
        builder: &mut ProblemBuilder,
        node: NodeId,
        indices: &[NodeId],
    ) -> Result<(), String> {
        let Some(base) = self.select_base(node) else {
            return Ok(());
        };
        let random = self.template_reads(node);
        if let Some(view) = self.view_select(path, builder, node, random)? {
            // A row is read by the method, `foreach` or `inside` around it.
            if view.prefix.len() == view.dimensions.len() {
                let element = self.view_element(node, path, builder, &view, &[])?;
                self.register_template(builder, node, element);
                state(builder).consumed.insert(view.root);
            }
            return Ok(());
        }
        let [index] = indices else {
            if self.template_array_known(builder, base) {
                return Err(self.rand_error(
                    node,
                    path,
                    "selecting from a multidimensional random array is not supported in constraints",
                ));
            }
            return Ok(());
        };
        // An element of a handle array is an object: the path through it
        // or the null test around it reads the select.
        if self
            .array_expr_decl(base)
            .is_some_and(|decl| self.is_handle_array(decl))
        {
            return Ok(());
        }
        let random = self.template_reads(*index);
        let Some((array, reference)) = self.template_array(path, builder, base, random)? else {
            return Ok(());
        };
        let (index, _) = self.template_operand(path, builder, *index)?;
        let element = self
            .loop_element(builder, array, reference, index)
            .map_err(|error| self.rand_error(node, path, &error))?;
        self.register_template(builder, node, element);
        state(builder).consumed.insert(base);
        Ok(())
    }

    /// `h.x` through `rand` handles: variable `x` of the object `h` names.
    fn template_member(
        &mut self,
        path: &str,
        builder: &mut ProblemBuilder,
        node: NodeId,
    ) -> Result<(), String> {
        if self.template_element_member(path, builder, node)? {
            return Ok(());
        }
        let Some(decls) = self.path_decls(node) else {
            return Ok(());
        };
        let Some((&last, handles)) = decls.split_last() else {
            return Ok(());
        };
        let through_element = handles.iter().any(|handle| self.element_step(*handle));
        if handles.is_empty()
            || (self.db.rand_qualifier(last) == RandQualifier::None && !through_element)
            || self.db.array_meta(last).is_some()
            || self.is_handle_property(last)
            || !matches!(self.kind(last), NodeKind::Var { .. })
        {
            return Ok(());
        }
        let Some(reference) = self.slot_reference(path, node, builder, handles)? else {
            return Ok(());
        };
        if self.db.variable_lifetime(last) == VariableLifetime::Static {
            return Err(self.rand_error(
                node,
                path,
                "a static random property selected through a `rand` handle is not supported in constraints",
            ));
        }
        let var = self.unit_var(path, node, builder, last)?;
        let (width, signed) = {
            let var = &builder.problem.vars[var as usize];
            (var.width, var.signed)
        };
        let member = builder.template(IrRandOp::HVar, signed, width, [reference, var, 0]);
        self.register_template(builder, node, member);
        Ok(())
    }

    /// `a[i].m.n`: packed members of a template array element are its bits.
    fn template_element_member(
        &mut self,
        path: &str,
        builder: &mut ProblemBuilder,
        node: NodeId,
    ) -> Result<bool, String> {
        let NodeKind::Expr(ExprKind::HierPath { parts, refs }) = self.kind(node) else {
            return Ok(false);
        };
        let Some(Some(select)) = refs.first().copied() else {
            return Ok(false);
        };
        let parts = parts.clone();
        let placeholder = self
            .rand_lowering
            .as_ref()
            .and_then(|lowering| lowering.templates.get(&select));
        let Some(IrExprKind::LocalRead(name)) = placeholder.map(|expr| &expr.kind) else {
            return Ok(false);
        };
        let name = name.clone();
        let Some(mut base) = builder.template_node(&name) else {
            return Ok(false);
        };
        let root = self.select_root(select);
        let array = root.and_then(|root| self.array_expr_decl(root));
        let shape = array.and_then(|decl| self.db.type_descriptor(decl));
        let mut descriptor = match shape.map(|descriptor| &descriptor.shape) {
            Some(TypeShape::Container { element, .. })
            | Some(TypeShape::FixedArray { element, .. }) => (**element).clone(),
            _ => return Ok(false),
        };
        for part in parts.iter().skip(1) {
            let TypeShape::Aggregate(layout) = &descriptor.shape else {
                return Ok(false);
            };
            let Some(index) = layout
                .members
                .iter()
                .position(|member| member.name == *part)
            else {
                return Ok(false);
            };
            let width = |item: &TypeDescriptor| {
                Self::fixed_descriptor_width(item)
                    .ok_or_else(|| self.rand_error(node, path, "a member has no packed width"))
            };
            let member = layout.members[index].descriptor.clone();
            // Struct members run from the most significant bit; union
            // members share the low bits.
            let low = if layout.kind == AggregateKind::PackedStruct {
                let mut below = 0;
                for later in &layout.members[index + 1..] {
                    below += width(&later.descriptor)?;
                }
                below
            } else {
                0
            };
            let bits = width(&member)?;
            base = builder.template(IrRandOp::Slice, member.info.signed, bits, [base, low, 0]);
            descriptor = member;
        }
        state(builder).used.insert(name);
        self.register_template(builder, node, base);
        Ok(true)
    }

    /// `h == null` / `h != null` of a `rand` handle.
    fn template_null_test(
        &mut self,
        path: &str,
        builder: &mut ProblemBuilder,
        node: NodeId,
        operands: [NodeId; 2],
        negate: bool,
    ) -> Result<(), String> {
        let is_null = |this: &Self, operand: NodeId| {
            matches!(
                this.kind(operand),
                NodeKind::Expr(ExprKind::Constant {
                    const_type: ConstantType::Null,
                    ..
                })
            )
        };
        let handle = if is_null(self, operands[1]) {
            operands[0]
        } else if is_null(self, operands[0]) {
            operands[1]
        } else {
            return Ok(());
        };
        let Some(decls) = self.path_decls(handle) else {
            return Ok(());
        };
        if decls.is_empty() {
            return Ok(());
        }
        let Some(reference) = self.slot_reference(path, handle, builder, &decls)? else {
            return Ok(());
        };
        let test = builder.template(IrRandOp::IsNull, false, 1, [reference, 0, 0]);
        let test = if negate { builder.not(test) } else { test };
        self.register_template(builder, node, test);
        Ok(())
    }

    /// `value inside { ..., array, ... }` with a random value or array: the
    /// array items become reductions over their elements (11.4.13).
    fn template_inside(
        &mut self,
        path: &str,
        builder: &mut ProblemBuilder,
        node: NodeId,
        operands: &[NodeId],
    ) -> Result<(), String> {
        let Some((&value, items)) = operands.split_first() else {
            return Ok(());
        };
        let (arrays, others): (Vec<NodeId>, Vec<NodeId>) = items
            .iter()
            .copied()
            .partition(|item| self.array_expr_decl(*item).is_some());
        if arrays.is_empty()
            || !(self.template_reads(value)
                || arrays
                    .iter()
                    .any(|array| self.template_array_known(builder, *array)))
        {
            return Ok(());
        }
        let (value, _) = self.template_operand(path, builder, value)?;
        let mut any = None;
        if !others.is_empty() {
            let lowered = self
                .lower_inside_items(path, &others)
                .map_err(|error| self.rand_error(node, path, &error))?;
            let st = state(builder);
            for item in &lowered {
                for expr in inside_item_exprs(item) {
                    st.used.extend(placeholders_in(expr));
                }
            }
            any = Some(
                builder
                    .inside_node(value, &lowered)
                    .map_err(|error| self.rand_error(node, path, &error))?,
            );
        }
        for array_node in arrays {
            let Some((array, reference)) = self.template_array(path, builder, array_node, true)?
            else {
                return Err(self.rand_error(
                    array_node,
                    path,
                    "this array cannot be used with `inside` in a constraint",
                ));
            };
            state(builder).consumed.insert(array_node);
            let loop_index = builder.new_loop();
            let iter = builder.template(IrRandOp::Iter, true, 32, [loop_index, 0, 0]);
            let element = self
                .loop_element(builder, array, reference, iter)
                .map_err(|error| self.rand_error(node, path, &error))?;
            let body = builder.equal(value, element);
            let args = builder.push_args(&[
                array,
                body,
                reference,
                loop_index,
                u32::from(IrRandOp::LogOr.code()),
            ]);
            let reduce = builder.template(IrRandOp::Reduce, false, 1, [0, args, 0]);
            any = Some(match any {
                None => reduce,
                Some(previous) => builder.logical(IrRandOp::LogOr, previous, reduce),
            });
        }
        if let Some(result) = any {
            self.register_template(builder, node, result);
        }
        Ok(())
    }

    /// `item.index` calls of iterator `iterator` in `root`.
    fn index_calls(&self, root: NodeId, iterator: NodeId) -> Vec<NodeId> {
        let mut found = Vec::new();
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            if let NodeKind::SysCall { name } = self.kind(node) {
                if name == "index"
                    && self.node(node).children.first().is_some_and(|child| {
                        matches!(
                            self.kind(*child),
                            NodeKind::Expr(ExprKind::Ref { target: Some(target) })
                                if *target == iterator
                        )
                    })
                {
                    found.push(node);
                    continue;
                }
            }
            stack.extend(self.node(node).children.iter().copied());
        }
        found
    }

    /// `array.size()` and the reductions `sum`, `product`, `and`, `or` and
    /// `xor` (7.12.3) of a template array; `false` when `node` is not one.
    fn template_method(
        &mut self,
        path: &str,
        builder: &mut ProblemBuilder,
        node: NodeId,
        name: &str,
        receiver: NodeId,
    ) -> Result<bool, String> {
        let is_row = matches!(
            self.kind(receiver),
            NodeKind::Expr(ExprKind::ArraySelect { .. })
        ) && self.select_root(receiver).is_some_and(|root| {
            self.array_expr_decl(root).is_some_and(|decl| {
                self.db
                    .array_meta(decl)
                    .is_some_and(|meta| meta.dimensions().len() > 1)
            })
        });
        if self.array_expr_decl(receiver).is_none() && !is_row {
            return Ok(false);
        }
        let op = match name {
            "size" => None,
            "sum" => Some(IrRandOp::Add),
            "product" => Some(IrRandOp::Mul),
            "and" => Some(IrRandOp::And),
            "or" => Some(IrRandOp::Or),
            "xor" => Some(IrRandOp::Xor),
            _ => {
                if self.template_array_known(builder, receiver) {
                    return Err(self.rand_error(
                        node,
                        path,
                        &format!("array method `{name}` of a random array is not supported in constraints"),
                    ));
                }
                return Ok(false);
            }
        };
        let with = if self.db.method_call_has_with_clause(node) {
            self.node(node)
                .children
                .iter()
                .copied()
                .find(|child| *child != receiver)
        } else {
            None
        };
        let create = with.is_some_and(|with| self.template_reads(with));
        let view = self.view_select(path, builder, receiver, create)?;
        if view.is_none() && is_row {
            return Ok(false);
        }
        let selected = match &view {
            Some(view) => Some((view.array, view.reference)),
            None => self.template_array(path, builder, receiver, create)?,
        };
        let Some((array, reference)) = selected else {
            return Ok(false);
        };
        state(builder)
            .consumed
            .insert(view.as_ref().map_or(receiver, |view| view.root));
        // The dimension a method of a multidimensional array walks.
        let dimension = view
            .as_ref()
            .and_then(|view| view.dimensions.get(view.prefix.len()).copied());
        let Some(op) = op else {
            let size = match dimension {
                Some((left, right)) => {
                    let extent = (i64::from(left) - i64::from(right)).abs() + 1;
                    builder.constant_u64(extent as u64, 32, true)
                }
                None => builder.template(IrRandOp::ASize, true, 32, [array, 0, reference]),
            };
            self.register_template(builder, node, size);
            return Ok(true);
        };
        if view
            .as_ref()
            .is_some_and(|view| view.prefix.len() + 1 != view.dimensions.len())
        {
            return Err(self.rand_error(
                node,
                path,
                "a reduction of a multidimensional array in a constraint must walk its last dimension",
            ));
        }
        let loop_array = match dimension {
            Some((left, right)) => builder.range_loop(left, right),
            None => array,
        };
        let loop_index = builder.new_loop();
        let iter = builder.template(IrRandOp::Iter, true, 32, [loop_index, 0, 0]);
        let slot = self
            .array_expr_decl(receiver)
            .and_then(|decl| state(builder).slot_of.get(&decl).copied());
        if let Some(slot) = slot {
            let Some(with) = with else {
                return Err(self.rand_error(
                    node,
                    path,
                    "a reduction of a handle array needs a `with` clause",
                ));
            };
            let element = self
                .slot_element(builder, slot, reference, iter)
                .map_err(|error| self.rand_error(node, path, &error))?;
            let iterator = self
                .db
                .method_call_iterator(node)
                .ok_or_else(|| self.rand_error(node, path, "the `with` clause has no iterator"))?;
            let index = builder.template_placeholder(iter);
            let calls = self.index_calls(with, iterator);
            if let Some(lowering) = self.rand_lowering.as_mut() {
                lowering.element_refs.insert(iterator, element);
                for call in &calls {
                    lowering.templates.insert(*call, index.clone());
                }
            }
            let lowered = self
                .template_visit(path, builder, with)
                .and_then(|()| self.template_operand(path, builder, with));
            if let Some(lowering) = self.rand_lowering.as_mut() {
                lowering.element_refs.remove(&iterator);
            }
            let (body, lowered) = lowered?;
            if lowered.is_real() || lowered.width == 0 {
                return Err(self.rand_error(
                    with,
                    path,
                    "a real-valued `with` expression is not supported in constraints",
                ));
            }
            let args =
                builder.push_args(&[array, body, reference, loop_index, u32::from(op.code())]);
            let reduce = builder.template(
                IrRandOp::Reduce,
                lowered.signed,
                lowered.width,
                [0, args, 0],
            );
            self.register_template(builder, node, reduce);
            return Ok(true);
        }
        let element = match &view {
            Some(view) => self.view_element(node, path, builder, view, &[iter])?,
            None => self
                .loop_element(builder, array, reference, iter)
                .map_err(|error| self.rand_error(node, path, &error))?,
        };
        let (body, width, signed) = match with {
            None => {
                let (width, signed) = builder.node_type(element);
                (element, width, signed)
            }
            Some(with) => {
                let iterator = self.db.method_call_iterator(node).ok_or_else(|| {
                    self.rand_error(node, path, "the `with` clause has no iterator")
                })?;
                let item = builder.template_placeholder(element);
                let index = builder.template_placeholder(iter);
                let calls = self.index_calls(with, iterator);
                if let Some(lowering) = self.rand_lowering.as_mut() {
                    lowering.bindings.insert(iterator, item);
                    for call in &calls {
                        lowering.templates.insert(*call, index.clone());
                    }
                }
                let lowered = self
                    .template_visit(path, builder, with)
                    .and_then(|()| self.template_operand(path, builder, with));
                if let Some(lowering) = self.rand_lowering.as_mut() {
                    lowering.bindings.remove(&iterator);
                }
                let (body, lowered) = lowered?;
                if lowered.is_real() || lowered.width == 0 {
                    return Err(self.rand_error(
                        with,
                        path,
                        "a real-valued `with` expression is not supported in constraints",
                    ));
                }
                (body, lowered.width, lowered.signed)
            }
        };
        let args = builder.push_args(&[
            loop_array,
            body,
            reference,
            loop_index,
            u32::from(op.code()),
        ]);
        let reduce = builder.template(IrRandOp::Reduce, signed, width, [0, args, 0]);
        self.register_template(builder, node, reduce);
        Ok(true)
    }

    /// After constraint expression `node` is lowered: every template form
    /// was read through its placeholder, and no random array is read
    /// directly (it would be treated as state).
    pub(super) fn check_template_uses(
        &self,
        path: &str,
        node: NodeId,
        lowered: &IrExpr,
        builder: &mut ProblemBuilder,
    ) -> Result<(), String> {
        let Some(st) = builder.units.as_deref_mut() else {
            return Ok(());
        };
        st.used.extend(placeholders_in(lowered));
        let st: &UnitState = st;
        if st.created.iter().any(|name| !st.used.contains(name)) {
            return Err(self.rand_error(
                node,
                path,
                "this use of an array element, array method or another object's member is not supported in a constraint",
            ));
        }
        let random_decl = |decl: NodeId| {
            self.db.rand_qualifier(decl) != RandQualifier::None
                || matches!(&st.root, RootMode::List(listed) if listed.contains(&decl))
        };
        let mut stack = vec![node];
        while let Some(current) = stack.pop() {
            let decls = match self.kind(current) {
                NodeKind::Expr(ExprKind::Ref { .. } | ExprKind::HierPath { .. }) => {
                    self.path_decls(current)
                }
                _ => None,
            };
            if let Some((&last, handles)) = decls.as_ref().and_then(|decls| decls.split_last()) {
                if self.db.array_meta(last).is_some()
                    && random_decl(last)
                    && !st.consumed.contains(&current)
                    && handles.iter().all(|handle| st.slot_of.contains_key(handle))
                {
                    return Err(self.rand_error(
                        current,
                        path,
                        "a random array may be used in a constraint only through element selects, `size()`, `sum`/`product`/`and`/`or`/`xor`, `inside` and `foreach`",
                    ));
                }
            }
            stack.extend(self.node(current).children.iter().copied());
        }
        Ok(())
    }

    /// The array form of `solve...before` primary `primary`: a whole array
    /// (every element), an element at a constant index, or a size.
    pub(super) fn array_order_side(
        &mut self,
        path: &str,
        builder: &mut ProblemBuilder,
        primary: NodeId,
        first: bool,
    ) -> Result<OrderSide, String> {
        let size_receiver = match self.kind(primary) {
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
                ..
            } if name == "size" => Some(*receiver),
            _ => None,
        };
        if let Some(receiver) = size_receiver {
            if self
                .template_array(path, builder, receiver, false)?
                .is_none()
            {
                return Ok(OrderSide::Variable);
            }
            if first {
                return Ok(OrderSide::Nothing);
            }
            return Err(self.rand_error(
                primary,
                path,
                "a `solve...before` order that solves a variable before an array size is not supported: sizes are solved first (S30-D5)",
            ));
        }
        if self.array_expr_decl(primary).is_some() {
            let Some((array, reference)) = self.template_array(path, builder, primary, false)?
            else {
                return Ok(OrderSide::Variable);
            };
            if reference != RAND_NONE {
                return Ok(OrderSide::Variable);
            }
            return Ok(OrderSide::Array(RAND_ORDER_ARRAY | array));
        }
        let NodeKind::Expr(ExprKind::ArraySelect { indices, .. }) = self.kind(primary) else {
            return Ok(OrderSide::Variable);
        };
        let indices = indices.clone();
        let Some(root) = self.select_root(primary) else {
            return Ok(OrderSide::Variable);
        };
        let Some((array, reference)) = self.template_array(path, builder, root, false)? else {
            return Ok(OrderSide::Variable);
        };
        if reference != RAND_NONE {
            return Ok(OrderSide::Variable);
        }
        let info = &state(builder).arrays[array as usize];
        let (kind, left, right, container) = (info.kind, info.left, info.right, info.container);
        let dimensions = match self.fixed_view_dims.get(&container) {
            Some(dimensions) => dimensions.clone(),
            None if kind == IrRandArrayKind::Fixed => vec![(left, right)],
            None => vec![(0, i32::MAX)],
        };
        if dimensions.len() != indices.len() {
            return Ok(OrderSide::Variable);
        }
        let mut position = 0i64;
        for (index, (left, right)) in indices.iter().zip(dimensions) {
            let value = self.eval_bound_i128(*index).ok().and_then(|value| {
                let offset = if left <= right {
                    value - i128::from(left)
                } else {
                    i128::from(left) - value
                };
                let extent = (i128::from(left) - i128::from(right)).abs() + 1;
                (0..extent)
                    .contains(&offset)
                    .then(|| (offset as i64, extent as i64))
            });
            let Some((offset, extent)) = value else {
                return Err(self.rand_error(
                    *index,
                    path,
                    "a `solve...before` array element needs a constant index inside the array",
                ));
            };
            position = position
                .checked_mul(extent)
                .and_then(|scaled| scaled.checked_add(offset))
                .ok_or_else(|| self.rand_error(*index, path, "array position overflows"))?;
        }
        let args = builder.push_args(&[array, position as u32]);
        Ok(OrderSide::Array(RAND_ORDER_ELEM | args))
    }

    /// `foreach (array[i]) body` (18.5.8.1): the body holds for every
    /// element; `i` is the element's declared index.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn constraint_foreach(
        &mut self,
        path: &str,
        builder: &mut ProblemBuilder,
        vars: &[VarPlan],
        object: &IrChandleExpr,
        item: NodeId,
        array: NodeId,
        loop_vars: &[Option<NodeId>],
        body: NodeId,
        guard: Option<u32>,
    ) -> Result<u32, String> {
        if builder.units.is_none() {
            return Err(self.rand_error(
                item,
                path,
                "internal: `foreach` outside a problem with units",
            ));
        }
        if let Some(view) = self.view_select(path, builder, array, true)? {
            return self.view_foreach(
                path, builder, vars, object, item, view, loop_vars, body, guard,
            );
        }
        if loop_vars.len() != 1 {
            return Err(self.rand_error(
                item,
                path,
                "a `foreach` constraint over more than one dimension is not supported",
            ));
        }
        let Some((array_index, reference)) = self.template_array(path, builder, array, true)?
        else {
            return Err(self.rand_error(
                array,
                path,
                "`foreach` in a constraint needs a one-dimensional array of packed integral elements or class handles that is a property of the randomized objects or a dynamic array or queue of the calling scope",
            ));
        };
        state(builder).consumed.insert(array);
        let loop_index = builder.new_loop();
        let iter = builder.template(IrRandOp::Iter, true, 32, [loop_index, 0, 0]);
        let binding = builder.template_placeholder(iter);
        if let (Some(var), Some(lowering)) = (loop_vars[0], self.rand_lowering.as_mut()) {
            lowering.bindings.insert(var, binding);
        }
        let lowered = self.constraint_item(path, builder, vars, object, body, guard);
        if let (Some(var), Some(lowering)) = (loop_vars[0], self.rand_lowering.as_mut()) {
            lowering.bindings.remove(&var);
        }
        let body = builder.truth(lowered?);
        let args = builder.push_args(&[array_index, body, reference, loop_index]);
        Ok(builder.template(IrRandOp::Foreach, false, 1, [0, args, 0]))
    }

    /// `foreach (m[i, j]) body` over the dimensions of a multidimensional
    /// array below the indices `view` selected; an omitted loop variable
    /// adds no loop (18.5.8.1).
    #[allow(clippy::too_many_arguments)]
    fn view_foreach(
        &mut self,
        path: &str,
        builder: &mut ProblemBuilder,
        vars: &[VarPlan],
        object: &IrChandleExpr,
        item: NodeId,
        view: ViewSelect,
        loop_vars: &[Option<NodeId>],
        body: NodeId,
        guard: Option<u32>,
    ) -> Result<u32, String> {
        let free = &view.dimensions[view.prefix.len()..];
        if loop_vars.len() > free.len() {
            return Err(self.rand_error(
                item,
                path,
                "a `foreach` constraint over the packed dimensions of an array is not supported",
            ));
        }
        state(builder).consumed.insert(view.root);
        let mut loops = Vec::new();
        for (variable, (left, right)) in loop_vars.iter().zip(free) {
            let Some(variable) = variable else {
                continue;
            };
            let loop_index = builder.new_loop();
            let iter = builder.template(IrRandOp::Iter, true, 32, [loop_index, 0, 0]);
            let binding = builder.template_placeholder(iter);
            if let Some(lowering) = self.rand_lowering.as_mut() {
                lowering.bindings.insert(*variable, binding);
            }
            loops.push((*variable, loop_index, *left, *right));
        }
        let lowered = self.constraint_item(path, builder, vars, object, body, guard);
        for (variable, ..) in &loops {
            if let Some(lowering) = self.rand_lowering.as_mut() {
                lowering.bindings.remove(variable);
            }
        }
        let mut result = builder.truth(lowered?);
        for (_, loop_index, left, right) in loops.into_iter().rev() {
            let range = builder.range_loop(left, right);
            let args = builder.push_args(&[range, result, view.reference, loop_index]);
            result = builder.template(IrRandOp::Foreach, false, 1, [0, args, 0]);
        }
        Ok(result)
    }

    /// One block per random array of the current unit whose elements have
    /// illegal bit patterns: every element holds a legal value. It is
    /// enabled while the array is random, and returns the enables. Sizes
    /// stay unconstrained: a loop over an unsolved size is dropped.
    pub(super) fn array_domain_blocks(
        &mut self,
        path: &str,
        at: NodeId,
        builder: &mut ProblemBuilder,
        object: Option<&IrChandleExpr>,
    ) -> Result<Vec<IrExpr>, String> {
        let unit = builder.unit as usize;
        let st = state(builder);
        let root = st.root.clone();
        let arrays = st
            .arrays
            .iter()
            .enumerate()
            .filter(|(_, info)| info.domain.is_some() && st.unit_has_array(unit, info))
            .map(|(index, info)| (index as u32, info.decl, info.domain.clone()))
            .collect::<Vec<_>>();
        let mut enables = Vec::with_capacity(arrays.len());
        for (array, decl, domain) in arrays {
            let Some(domain) = domain else {
                continue;
            };
            let loop_index = builder.new_loop();
            let iter = builder.template(IrRandOp::Iter, true, 32, [loop_index, 0, 0]);
            let element = self
                .loop_element(builder, array, RAND_NONE, iter)
                .and_then(|element| self.value_domain(builder, element, &domain))
                .map_err(|error| self.rand_error(at, path, &error))?;
            let Some(legal) = element else {
                continue;
            };
            let args = builder.push_args(&[array, legal, RAND_NONE, loop_index]);
            let root_node = builder.template(IrRandOp::Foreach, false, 1, [0, args, 0]);
            let name = format!("{}_legal", self.node(decl).name);
            builder.add_block(name, root_node);
            enables.push(self.unit_active(&root, unit, decl, object)?);
        }
        Ok(enables)
    }

    /// Build the units of every class an object reached through a `rand`
    /// handle can have, after unit 0's constraints are translated.
    pub(super) fn lower_units(
        &mut self,
        path: &str,
        node: NodeId,
        builder: &mut ProblemBuilder,
        root_vars: &mut Vec<VarPlan>,
    ) -> Result<(), String> {
        let blocks = builder.problem.blocks.len() as u32;
        let leaves = builder.leaves.len() as u32;
        {
            let root = &mut state(builder).units[0];
            root.block_count = blocks;
            root.leaf_count = leaves;
        }
        let mut next = 0;
        loop {
            let target = {
                let st = state(builder);
                match st.slots.get(next) {
                    Some(slot) => slot.target,
                    None => break,
                }
            };
            next += 1;
            for class in self.class_cases(target) {
                if !state(builder).unit_of_class.contains_key(&class) {
                    self.build_unit(path, node, builder, class)?;
                }
            }
        }
        // Non-random properties read through handle array elements are fed
        // like variables, in every unit that has them, and stay inactive.
        for decl in std::mem::take(&mut state(builder).state_vars) {
            let owner = self.node(decl).parent;
            for unit in 0..state(builder).units.len() {
                let info = &state(builder).units[unit];
                if !owner.is_some_and(|owner| info.chain.contains(&owner)) {
                    continue;
                }
                let Some(object) = info.object.clone() else {
                    continue;
                };
                let Some((plan, _)) =
                    self.class_var_plan(builder, decl, &object, &RandSelection::Checker, false)?
                else {
                    return Err(self.rand_error(
                        decl,
                        path,
                        "a property read through a handle array element in a constraint must be a packed integral value",
                    ));
                };
                if unit == 0 {
                    root_vars.push(plan);
                } else {
                    state(builder).units[unit].vars.push(plan);
                }
            }
        }
        Ok(())
    }

    fn build_unit(
        &mut self,
        path: &str,
        node: NodeId,
        builder: &mut ProblemBuilder,
        class: NodeId,
    ) -> Result<(), String> {
        let chain = self.class_chain(class);
        let block_first = builder.problem.blocks.len() as u32;
        let leaf_first = builder.leaves.len() as u32;
        let (unit, object) = {
            let st = state(builder);
            let unit = st.units.len() as u32;
            let object = IrChandleExpr::LocalRead(format!("{}_u{unit}_h", st.tag));
            st.units.push(UnitInfo {
                class: Some(class),
                chain: chain.clone(),
                object: Some(object.clone()),
                vars: Vec::new(),
                blocks: Vec::new(),
                block_first,
                block_count: 0,
                leaf_first,
                leaf_count: 0,
                pre: None,
                post: None,
            });
            st.unit_of_class.insert(class, unit);
            (unit, object)
        };
        builder.begin_unit(unit);
        let mut vars = Vec::new();
        let mut placeholders = HashMap::new();
        for member in chain.iter().rev().copied() {
            for child in self.node(member).children.clone() {
                if !matches!(
                    self.kind(child),
                    NodeKind::Var { .. } | NodeKind::Array { .. }
                ) || self.db.rand_qualifier(child) == RandQualifier::None
                {
                    continue;
                }
                let name = self.node(child).name.clone();
                let member_name = self.node(member).name.clone();
                if self.db.variable_lifetime(child) == VariableLifetime::Static {
                    return Err(self.rand_error(
                        node,
                        path,
                        &format!("static random property `{name}` of class `{member_name}`, whose objects are randomized through a `rand` handle, is not supported"),
                    ));
                }
                if self.root_unit_property(path, node, builder, child, true)? {
                    continue;
                }
                if self.db.rand_qualifier(child) == RandQualifier::RandC
                    && !self.randc_states.contains_key(&child)
                {
                    return Err(self.rand_error(
                        node,
                        path,
                        &format!(
                            "`randc` property `{name}` of class `{member_name}` must be a packed integral value of at most {} bits (18.4.2 permits a limit; S30-D3)",
                            crate::sim::ir::RAND_CYCLIC_MAX_WIDTH
                        ),
                    ));
                }
                let Some(plan) =
                    self.class_var_plan(builder, child, &object, &RandSelection::Default, false)?
                else {
                    return Err(self.rand_error(
                        node,
                        path,
                        &format!("random property `{name}` of class `{member_name}` is not a packed integral value, a one-dimensional array of packed integral elements or a class handle, or is a structure with enum or tagged-union members"),
                    ));
                };
                if plan.0.cyclic.is_some() {
                    builder.set_cyclic(plan.0.index);
                }
                placeholders.insert(child, plan.1);
                vars.push(plan.0);
            }
        }
        let effective = self.effective_blocks(&chain);
        let classes = chain
            .iter()
            .filter_map(|member| self.class_nodes.get(member).copied())
            .collect::<HashSet<_>>();
        let saved = self.rand_lowering.replace(RandLowering::new(
            classes,
            Some(object.clone()),
            placeholders,
        ));
        let bound = self.bind_constraint_containers(&chain, &effective, None, &object);
        let lowered = self.lower_class_blocks(path, builder, &vars, &object, &effective, None);
        self.rand_lowering = saved;
        for node in bound {
            self.receiver_containers.remove(&node);
        }
        let mut blocks = lowered?;
        blocks.extend(self.array_domain_blocks(path, node, builder, Some(&object))?);
        let pre = self
            .randomize_hook(class, "pre_randomize")
            .map(|hook| self.hook_call(path, node, hook, &object))
            .transpose()?;
        let post = self
            .randomize_hook(class, "post_randomize")
            .map(|hook| self.hook_call(path, node, hook, &object))
            .transpose()?;
        let block_count = builder.problem.blocks.len() as u32 - block_first;
        let leaf_count = builder.leaves.len() as u32 - leaf_first;
        let info = &mut state(builder).units[unit as usize];
        info.vars = vars;
        info.blocks = blocks;
        info.block_count = block_count;
        info.leaf_count = leaf_count;
        info.pre = pre;
        info.post = post;
        Ok(())
    }

    /// Record the units, arrays and slots in the problem; `root_vars` are
    /// unit 0's instance variables.
    pub(super) fn finish_units(&self, builder: &mut ProblemBuilder, root_vars: Vec<u32>) {
        let st = builder.units.as_deref().expect("unit state");
        builder.problem.arrays = st
            .arrays
            .iter()
            .map(|array| IrRandArray {
                name: self.node(array.decl).name.clone(),
                elem_width: array.width,
                elem_signed: array.signed,
                kind: array.kind,
                left: array.left,
                right: array.right,
            })
            .collect();
        builder.problem.slots = st
            .slots
            .iter()
            .map(|slot| IrRandSlot {
                name: self.node(slot.decl).name.clone(),
                kind: slot
                    .array
                    .as_ref()
                    .map_or(IrRandArrayKind::Fixed, |array| array.kind),
                is_array: slot.array.is_some(),
                left: slot.array.as_ref().map_or(0, |array| array.left),
                right: slot.array.as_ref().map_or(0, |array| array.right),
            })
            .collect();
        builder.problem.units = st
            .units
            .iter()
            .enumerate()
            .map(|(index, unit)| IrRandUnit {
                name: unit
                    .class
                    .map(|class| self.node(class).name.clone())
                    .unwrap_or_else(|| "std::randomize".to_owned()),
                vars: if index == 0 {
                    root_vars.clone()
                } else {
                    unit.vars.iter().map(|var| var.index).collect()
                },
                arrays: (0..st.arrays.len() as u32)
                    .filter(|array| st.unit_has_array(index, &st.arrays[*array as usize]))
                    .collect(),
                slots: (0..st.slots.len() as u32)
                    .filter(|slot| st.unit_has_slot(index, &st.slots[*slot as usize]))
                    .collect(),
                block_first: unit.block_first,
                block_count: unit.block_count,
                leaf_first: unit.leaf_first,
                leaf_count: unit.leaf_count,
            })
            .collect();
    }

    /// Whether array or handle `decl` of an object of unit `unit` (read
    /// through `object`) takes part as random.
    fn unit_active(
        &mut self,
        root: &RootMode,
        unit: usize,
        decl: NodeId,
        object: Option<&IrChandleExpr>,
    ) -> Result<IrExpr, String> {
        let rand = self.db.rand_qualifier(decl) != RandQualifier::None;
        let mode = if unit == 0 { root } else { &RootMode::Default };
        match mode {
            RootMode::Default if rand => match self.rand_mode_bits.get(&decl).cloned() {
                Some(bit) => Ok(bit_not(self.mode_read(&bit, object.cloned())?)),
                None => Ok(packed_const(1, 1, false)),
            },
            RootMode::Default | RootMode::Checker => Ok(packed_const(0, 1, false)),
            RootMode::List(listed) => Ok(packed_const(u64::from(listed.contains(&decl)), 1, false)),
        }
    }

    /// The storage of `array` for an object read through `object`.
    fn unit_container(&mut self, array: &ArrayInfo, object: Option<&IrChandleExpr>) -> usize {
        match (array.owner, object) {
            (Some(_), Some(object)) => self.class_container_alias(array.container, object.clone()),
            _ => array.container,
        }
    }

    /// The commit of a `rand` handle array's size (18.4): the array grows
    /// with null elements or loses its last ones; a dynamic array keeps its
    /// elements through `new[size](array)` (7.5.1).
    fn resize_handle_array(
        &self,
        problem: usize,
        st: &UnitState,
        array_index: u32,
        container: usize,
        committed: IrExpr,
    ) -> Vec<IrStmt> {
        let size = || container_read(IrContainerExpr::Size(container), 32, true);
        let chosen = || {
            query(
                problem,
                IrRandQuery::ArraySize(array_index),
                Some(committed.clone()),
                None,
                32,
                true,
            )
        };
        let (slot, kind) = st
            .slots
            .iter()
            .enumerate()
            .find_map(|(index, slot)| {
                let array = slot.array.as_ref()?;
                (array.size == Some(array_index)).then_some((index as u32, array.kind))
            })
            .expect("a size array belongs to a handle array");
        let stmt = |statement: IrContainerStmt| IrStmt::Container(Box::new(statement));
        let mut body = vec![IrStmt::Randomize(Box::new(IrRandStmt::TrimSlot {
            problem,
            inst: committed.clone(),
            slot,
            size: chosen(),
        }))];
        if kind == IrRandArrayKind::Queue {
            body.push(IrStmt::While {
                cond: bin32(IrBinOp::Lt, chosen(), size()),
                body: vec![stmt(IrContainerStmt::DeleteIndex {
                    container,
                    index: bin32(IrBinOp::Sub, size(), int_const(1)),
                })],
            });
            body.push(IrStmt::While {
                cond: bin32(IrBinOp::Lt, size(), chosen()),
                body: vec![stmt(IrContainerStmt::QueuePushBackChandle {
                    container,
                    value: IrChandleExpr::Null,
                })],
            });
        } else {
            body.push(when(
                bin32(IrBinOp::Neq, chosen(), size()),
                vec![stmt(IrContainerStmt::DynamicNew {
                    container,
                    size: chosen(),
                    initializer: Some(container),
                })],
            ));
        }
        body
    }

    /// The instance walk that feeds every unit after unit 0's variables,
    /// the commit of arrays and other instances, and the other instances'
    /// `post_randomize` calls.
    pub(super) fn unit_statements(
        &mut self,
        problem: usize,
        tag: &str,
        st: UnitState,
        leaves: Vec<LeafPlan>,
    ) -> Result<UnitCode, String> {
        let rand = |statement: IrRandStmt| IrStmt::Randomize(Box::new(statement));
        let inst_local = format!("{tag}_i");
        let unit_local = format!("{tag}_u");
        let counter = format!("{tag}_k");
        let commit_local = format!("{tag}_c");
        let inst = || local_read(&inst_local, 32, true);
        let committed = || local_read(&commit_local, 32, true);
        let root_leaves = st.root_leaf_count() as u32;
        let mut walk = vec![
            decl32(&unit_local, int_const(0)),
            decl32(&counter, int_const(0)),
            decl32(&commit_local, int_const(0)),
            decl32(
                &inst_local,
                query(problem, IrRandQuery::NextInst, None, None, 32, true),
            ),
        ];
        for unit in &st.units[1..] {
            if let Some(IrChandleExpr::LocalRead(name)) = &unit.object {
                walk.push(IrStmt::Object(Box::new(IrObjectStmt::ChandleDeclareLocal(
                    name.clone(),
                    None,
                ))));
            }
        }
        let handles = st
            .units
            .iter()
            .enumerate()
            .skip(1)
            .filter_map(|(index, unit)| match &unit.object {
                Some(IrChandleExpr::LocalRead(name)) => Some((index as u32, name.clone())),
                _ => None,
            })
            .collect();
        let mut feeds = Vec::new();
        let mut commits = Vec::new();
        let mut posts = Vec::new();
        for (index, unit) in st.units.iter().enumerate() {
            let object = unit.object.as_ref();
            let point = |at: IrExpr| -> Vec<IrStmt> {
                match (index, object) {
                    (0, _) | (_, None) => Vec::new(),
                    (_, Some(object)) => {
                        let IrChandleExpr::LocalRead(name) = object else {
                            return Vec::new();
                        };
                        vec![IrStmt::Object(Box::new(IrObjectStmt::ChandleAssignLocal(
                            name.clone(),
                            IrChandleExpr::RandInstance {
                                problem,
                                inst: Box::new(at),
                            },
                        )))]
                    }
                }
            };
            let mut feed = point(inst());
            let mut commit = point(committed());
            if index > 0 {
                if let Some(pre) = &unit.pre {
                    feed.push(when(
                        query(problem, IrRandQuery::Pre, Some(inst()), None, 1, false),
                        vec![pre.clone()],
                    ));
                }
                if let Some(post) = &unit.post {
                    let mut body = point(committed());
                    body.push(post.clone());
                    posts.push((index as u32, body));
                }
                for var in &unit.vars {
                    feed.push(rand(IrRandStmt::ISetVar {
                        problem,
                        inst: inst(),
                        var: var.index,
                        value: var.current.clone(),
                        active: var.active.clone(),
                    }));
                    let value = query(
                        problem,
                        IrRandQuery::Value(var.index),
                        Some(committed()),
                        None,
                        var.width,
                        var.signed,
                    );
                    let value = if var.two_state {
                        IrExpr::to_two_state(value)
                    } else {
                        value
                    };
                    let mut then_ = vec![IrStmt::Assign {
                        lhs: var.lhs.clone(),
                        rhs: value,
                        nba: false,
                    }];
                    if let Some(cyclic) = &var.cyclic {
                        feed.push(rand(IrRandStmt::ISetCyclic {
                            problem,
                            inst: inst(),
                            var: var.index,
                            state: cyclic.read.clone(),
                        }));
                        then_.push(IrStmt::Assign {
                            lhs: cyclic.target.clone(),
                            rhs: query(
                                problem,
                                IrRandQuery::Cyclic(var.index),
                                Some(committed()),
                                None,
                                crate::sim::ir::rand_cyclic_state_width(var.width),
                                false,
                            ),
                            nba: false,
                        });
                    }
                    commit.push(when(
                        query(
                            problem,
                            IrRandQuery::Active(var.index),
                            Some(committed()),
                            None,
                            1,
                            false,
                        ),
                        then_,
                    ));
                }
                for (offset, enabled) in unit.blocks.iter().enumerate() {
                    feed.push(rand(IrRandStmt::ISetBlock {
                        problem,
                        inst: inst(),
                        block: unit.block_first + offset as u32,
                        enabled: enabled.clone(),
                    }));
                }
                for leaf in unit.leaf_first..unit.leaf_first + unit.leaf_count {
                    let template = leaves[(leaf - root_leaves) as usize].template.clone();
                    feed.push(self.guarded_leaf(problem, Some(&inst()), leaf, template));
                }
            }
            for (array_index, array) in st.arrays.iter().enumerate() {
                if !st.unit_has_array(index, array) {
                    continue;
                }
                let array_index = array_index as u32;
                let active = self.unit_active(&st.root, index, array.decl, object)?;
                let container = self.unit_container(array, object);
                let size = container_read(IrContainerExpr::Size(container), 32, true);
                feed.push(rand(IrRandStmt::SetArray {
                    problem,
                    inst: inst(),
                    array: array_index,
                    size: size.clone(),
                    active,
                }));
                if array.kind == IrRandArrayKind::Size {
                    let body =
                        self.resize_handle_array(problem, &st, array_index, container, committed());
                    commit.push(when(
                        query(
                            problem,
                            IrRandQuery::ArrayActive(array_index),
                            Some(committed()),
                            None,
                            1,
                            false,
                        ),
                        body,
                    ));
                    continue;
                }
                feed.push(counted(
                    &counter,
                    size,
                    vec![rand(IrRandStmt::SetElem {
                        problem,
                        inst: inst(),
                        array: array_index,
                        index: local_read(&counter, 32, true),
                        value: container_read(
                            IrContainerExpr::Get {
                                container,
                                index: Box::new(local_read(&counter, 32, true)),
                            },
                            array.width,
                            array.signed,
                        ),
                    })],
                ));
                let chosen = query(
                    problem,
                    IrRandQuery::ArraySize(array_index),
                    Some(committed()),
                    None,
                    32,
                    true,
                );
                let element = query(
                    problem,
                    IrRandQuery::Elem(array_index),
                    Some(committed()),
                    Some(local_read(&counter, 32, true)),
                    array.width,
                    array.signed,
                );
                let element = if array.two_state {
                    IrExpr::to_two_state(element)
                } else {
                    element
                };
                let container_stmt =
                    |statement: IrContainerStmt| IrStmt::Container(Box::new(statement));
                let store = match array.kind {
                    IrRandArrayKind::Queue => container_stmt(IrContainerStmt::QueuePushBack {
                        container,
                        value: element,
                    }),
                    _ => container_stmt(IrContainerStmt::Set {
                        container,
                        index: local_read(&counter, 32, true),
                        value: element,
                    }),
                };
                let mut body = match array.kind {
                    IrRandArrayKind::Dynamic => vec![container_stmt(IrContainerStmt::DynamicNew {
                        container,
                        size: chosen.clone(),
                        initializer: None,
                    })],
                    IrRandArrayKind::Queue => {
                        vec![container_stmt(IrContainerStmt::Delete(container))]
                    }
                    IrRandArrayKind::Fixed | IrRandArrayKind::Size => Vec::new(),
                };
                body.push(counted(&counter, chosen, vec![store]));
                commit.push(when(
                    query(
                        problem,
                        IrRandQuery::ArrayActive(array_index),
                        Some(committed()),
                        None,
                        1,
                        false,
                    ),
                    body,
                ));
            }
            for (slot_index, slot) in st.slots.iter().enumerate() {
                if !st.unit_has_slot(index, slot) {
                    continue;
                }
                let random = self.unit_active(&st.root, index, slot.decl, object)?;
                let cases = self.class_cases(slot.target);
                let links = |child: &IrChandleExpr, at: Option<IrExpr>| -> Vec<Vec<IrStmt>> {
                    cases
                        .iter()
                        .map(|case| {
                            vec![rand(IrRandStmt::Link {
                                problem,
                                inst: inst(),
                                slot: slot_index as u32,
                                index: at.clone(),
                                child: child.clone(),
                                unit: st.unit_of_class[case],
                                random: random.clone(),
                            })]
                        })
                        .collect()
                };
                let local = format!("{tag}_l{index}_{slot_index}");
                if let Some(array) = &slot.array {
                    let container = match (slot.owner, object) {
                        (Some(_), Some(object)) => {
                            self.class_container_alias(array.container, object.clone())
                        }
                        _ => array.container,
                    };
                    let size = container_read(IrContainerExpr::Size(container), 32, true);
                    feed.push(rand(IrRandStmt::SetSlotSize {
                        problem,
                        inst: inst(),
                        slot: slot_index as u32,
                        size: size.clone(),
                    }));
                    let at = local_read(&counter, 32, true);
                    let child = IrChandleExpr::ContainerGet {
                        container,
                        index: Box::new(at.clone()),
                    };
                    let bodies = links(&child, Some(at));
                    let body = self.class_case_chain(&local, &child, &cases, bodies);
                    feed.push(counted(&counter, size, body));
                    continue;
                }
                let (Some(object), Some((class, field))) = (object, slot.field) else {
                    continue;
                };
                let child = IrChandleExpr::LocalRead(self.native_access_symbol(
                    object.clone(),
                    IrNativeAccessKind::ClassField { class, field },
                ));
                let bodies = links(&child, None);
                feed.extend(self.class_case_chain(&local, &child, &cases, bodies));
            }
            if !feed.is_empty() {
                feeds.push((index as u32, feed));
            }
            if commit.len() > usize::from(index > 0) {
                commits.push((index as u32, commit));
            }
        }
        let unit_of = |at: IrExpr| {
            assign32(
                &unit_local,
                query(problem, IrRandQuery::Unit, Some(at), None, 32, true),
            )
        };
        let mut body = vec![unit_of(inst())];
        body.extend(dispatch(&unit_local, feeds));
        body.push(assign32(
            &inst_local,
            query(problem, IrRandQuery::NextInst, None, None, 32, true),
        ));
        walk.push(IrStmt::While {
            cond: bin32(IrBinOp::Neq, inst(), int_const(-1)),
            body,
        });
        let count = || query(problem, IrRandQuery::Count, None, None, 32, true);
        let random = || {
            query(
                problem,
                IrRandQuery::Random,
                Some(committed()),
                None,
                1,
                false,
            )
        };
        let mut commit = Vec::new();
        if let Some(chain) = dispatch(&unit_local, commits) {
            commit.push(counted(
                &commit_local,
                count(),
                vec![unit_of(committed()), when(random(), vec![chain])],
            ));
        }
        // post_randomize follows the pre_randomize order (S30-D7): the walk
        // counter `counter` steps through the visited instances.
        let mut post = Vec::new();
        if let Some(chain) = dispatch(&unit_local, posts) {
            post.push(counted(
                &counter,
                count(),
                vec![
                    assign32(
                        &commit_local,
                        query(
                            problem,
                            IrRandQuery::Visit,
                            Some(local_read(&counter, 32, true)),
                            None,
                            32,
                            true,
                        ),
                    ),
                    when(
                        bin32(IrBinOp::Neq, committed(), int_const(-1)),
                        vec![unit_of(committed()), when(random(), vec![chain])],
                    ),
                ],
            ));
        }
        Ok(UnitCode {
            walk,
            commit,
            posts: post,
            handles,
        })
    }
}
