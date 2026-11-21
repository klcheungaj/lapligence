//! Static declaration-initializer ordering.
//!
//! SystemVerilog static initialization (SV §§6.8, 6.21, 10.5) runs before
//! any process starts. Collection discovers initializers out of source
//! order: initializers that call subroutines or read storage of
//! not-yet-collected scopes are lowered late, static subroutine locals are
//! found while lowering bodies, and descriptor-backed fixed arrays are
//! lowered after every array is collected. The schedule therefore sorts
//! initializers by scope and position (decision S22-D8): packages, the
//! compilation unit, then the design hierarchy in post-order; in each the
//! class static properties, then (in an instance) each generate block, then
//! the scope's own declarations. Each of these runs its constant
//! initializers first, then the others in declaration order, the class
//! static properties of one scope as a whole. An initializer reads whatever
//! the declarations it names hold when it runs, so a forward read sees the
//! default value of a later non-constant initializer. A static local of a
//! subroutine or procedure takes the place of its subroutine or procedure
//! among the scope's declarations; a call does not move it.

use super::*;

/// Elaboration-time system functions (IEEE 1800-2009 11.2.1, 20.5-20.9) that
/// keep a call-free initializer constant: conversions, data and array
/// queries, math and bit-vector functions.
const CONSTANT_SYSTEM_FUNCTIONS: &[&str] = &[
    "$signed",
    "$unsigned",
    "$rtoi",
    "$itor",
    "$realtobits",
    "$bitstoreal",
    "$shortrealtobits",
    "$bitstoshortreal",
    "$bits",
    "$clog2",
    "$ln",
    "$log10",
    "$exp",
    "$sqrt",
    "$pow",
    "$floor",
    "$ceil",
    "$sin",
    "$cos",
    "$tan",
    "$asin",
    "$acos",
    "$atan",
    "$atan2",
    "$hypot",
    "$sinh",
    "$cosh",
    "$tanh",
    "$asinh",
    "$acosh",
    "$atanh",
    "$dimensions",
    "$unpacked_dimensions",
    "$left",
    "$right",
    "$low",
    "$high",
    "$increment",
    "$size",
    "$countbits",
    "$countones",
    "$onehot",
    "$onehot0",
    "$isunknown",
];

/// Data and array queries (IEEE 1800-2009 20.6.2, 20.7) whose first operand
/// may name a variable: they read its type or current extent, not its value,
/// so the initializer stays a constant (industry-standard behaviour presets
/// it, also for a dynamic array or queue, with the extent it has then).
const QUERY_SYSTEM_FUNCTIONS: &[&str] = &[
    "$bits",
    "$dimensions",
    "$unpacked_dimensions",
    "$left",
    "$right",
    "$low",
    "$high",
    "$increment",
    "$size",
];

/// The scope whose initialization runs a static initializer (S22-D8).
#[derive(Clone, Copy)]
enum InitializerOwner {
    Package(NodeId),
    Unit,
    Instance(NodeId),
}

impl<'a> Codegen<'a> {
    /// Record the collection-time declaration order of one initialized
    /// declaration. The first reservation wins, so a deferred initializer
    /// keeps the slot of its declaration rather than the slot of its lowering.
    pub(in super::super) fn reserve_initializer_order(&mut self, declaration: NodeId) {
        let next = self.initializer_order.len();
        self.initializer_order
            .entry(declaration.index() as u32)
            .or_insert(next);
    }

    /// Remember the source expression of one lowered declaration initializer
    /// for the static-effect checks (`effects.rs`).
    pub(in super::super) fn record_initializer_source(
        &mut self,
        declaration: NodeId,
        initializer: NodeId,
    ) {
        self.reserve_initializer_order(declaration);
        self.initializer_sources
            .entry(declaration.index() as u32)
            .or_default()
            .push(initializer);
    }

    /// The scope that runs a static initializer (decision S22-D8) and the
    /// initializer's sub-rank there: a package or the compilation unit
    /// (class static properties 0, variables 1), or a design instance (class
    /// static properties 0, variables of its generate blocks 1, its own
    /// variables 2). A class property belongs to the scope that declares the
    /// class, a static local to the scope of its subroutine.
    fn initializer_owner(&self, declaration: NodeId) -> (InitializerOwner, usize) {
        let mut class = false;
        let mut generate = false;
        let mut current = self.node(declaration).parent;
        while let Some(node) = current {
            match self.kind(node) {
                NodeKind::ClassDef => {
                    class = true;
                    current = self
                        .db
                        .class_declaring_scope(node)
                        .or(self.node(node).parent);
                    continue;
                }
                NodeKind::ModuleInst { .. } => {
                    let sub = if class {
                        0
                    } else if generate {
                        1
                    } else {
                        2
                    };
                    return (InitializerOwner::Instance(node), sub);
                }
                NodeKind::Package => {
                    return (InitializerOwner::Package(node), usize::from(!class));
                }
                NodeKind::GenScope | NodeKind::GenScopeArray => generate = true,
                _ if self.is_compilation_unit(node) => break,
                _ => {}
            }
            current = self.node(node).parent;
        }
        (InitializerOwner::Unit, usize::from(!class))
    }

    /// The post-order key of a design instance: the child positions on its
    /// path from the root, closed by `usize::MAX`, so an instance's
    /// descendants sort before it and siblings keep declaration order.
    fn instance_post_order(
        &self,
        instance: NodeId,
        positions: &mut HashMap<NodeId, HashMap<NodeId, usize>>,
    ) -> Vec<usize> {
        let mut path = vec![usize::MAX];
        let mut current = instance;
        while let Some(parent) = self.node(current).parent {
            let children = positions.entry(parent).or_insert_with(|| {
                self.node(parent)
                    .children
                    .iter()
                    .enumerate()
                    .map(|(position, child)| (*child, position))
                    .collect()
            });
            path.push(children.get(&current).copied().unwrap_or(0));
            current = parent;
        }
        path.reverse();
        path
    }

    /// The declaring scope of an initialized declaration and its position
    /// path there (decision S22-D8). A declaration nested in a subroutine or
    /// procedure (a static local, also in a block of it) belongs to the
    /// scope that holds the subroutine or procedure, at the position of that
    /// subroutine or procedure, then its position inside; any other
    /// declaration belongs to its parent at its own position.
    fn initializer_anchor(
        &self,
        declaration: NodeId,
        positions: &mut HashMap<NodeId, HashMap<NodeId, usize>>,
    ) -> (Option<NodeId>, Vec<usize>) {
        let parent = self.node(declaration).parent;
        let mut chain = vec![declaration];
        let mut nested = false;
        let mut scope = parent;
        while let Some(node) = scope {
            match self.kind(node) {
                NodeKind::ModuleInst { .. }
                | NodeKind::Package
                | NodeKind::ClassDef
                | NodeKind::GenScope
                | NodeKind::GenScopeArray => break,
                _ if self.is_compilation_unit(node) => break,
                NodeKind::FuncTask { .. } | NodeKind::Process { .. } | NodeKind::Stmt(_) => {
                    nested = true
                }
                _ => {}
            }
            chain.push(node);
            scope = self.node(node).parent;
        }
        if !nested {
            chain.truncate(1);
            scope = parent;
        }
        let mut path = Vec::with_capacity(chain.len());
        let mut holder = scope;
        for node in chain.iter().rev() {
            let position = holder
                .map(|holder| {
                    let children = positions.entry(holder).or_insert_with(|| {
                        self.node(holder)
                            .children
                            .iter()
                            .enumerate()
                            .map(|(position, child)| (*child, position))
                            .collect()
                    });
                    children.get(node).copied().unwrap_or(0)
                })
                .unwrap_or(0);
            path.push(position);
            holder = Some(*node);
        }
        (scope, path)
    }

    /// Whether a declaration's initializer is a constant (decision S22-D8):
    /// it runs with its scope's constants, before the scope's other
    /// initializers. Industry-standard behaviour presets such a value, so a
    /// subroutine called by an earlier initializer of the scope already reads
    /// it; it reads no variable value and has no effect, so running it first
    /// is equivalent.
    fn has_constant_initializer(&self, declaration: NodeId) -> bool {
        self.db
            .var_initializer(declaration)
            .or_else(|| {
                self.db
                    .array_meta(declaration)
                    .and_then(|meta| meta.initializer())
            })
            .is_some_and(|initializer| self.is_constant_initializer(initializer))
    }

    /// Whether an initializer is a constant expression without subroutine
    /// calls: literals, parameters and enumeration constants combined by
    /// operators, casts, selections, patterns and the constant system
    /// functions of `CONSTANT_SYSTEM_FUNCTIONS`, reading no variable value.
    /// The first operand of a `QUERY_SYSTEM_FUNCTIONS` query may name a
    /// variable.
    fn is_constant_initializer(&self, initializer: NodeId) -> bool {
        // Each node carries whether it lies in a query's first operand.
        let mut stack = vec![(initializer, false)];
        while let Some((current, queried)) = stack.pop() {
            let constant = match self.kind(current) {
                NodeKind::Expr(ExprKind::Ref {
                    target: Some(target),
                }) => {
                    queried
                        || matches!(
                            self.kind(*target),
                            NodeKind::Param { .. }
                                | NodeKind::ParamAssign { .. }
                                | NodeKind::EnumConst { .. }
                                | NodeKind::Genvar { .. }
                        )
                }
                NodeKind::Expr(ExprKind::HierPath { .. }) => queried,
                NodeKind::Expr(ExprKind::Operation { op, assignment, .. }) => {
                    !*assignment
                        && !matches!(
                            op,
                            Operation::Assignment
                                | Operation::PostIncrement
                                | Operation::PreIncrement
                                | Operation::PostDecrement
                                | Operation::PreDecrement
                        )
                }
                NodeKind::Expr(
                    ExprKind::Constant { .. }
                    | ExprKind::Conditional { .. }
                    | ExprKind::Cast { .. }
                    | ExprKind::DataType
                    | ExprKind::Unbounded
                    | ExprKind::BitSelect { .. }
                    | ExprKind::PartSelect { .. }
                    | ExprKind::IndexedPartSelect { .. }
                    | ExprKind::ArraySelect { .. }
                    | ExprKind::MemberSelect { .. },
                ) => true,
                // An assignment-pattern item: its key and value are checked
                // like any operand.
                NodeKind::Expr(ExprKind::TaggedPattern {
                    index_key, value, ..
                }) => {
                    stack.extend(index_key.iter().chain(value).map(|item| (*item, queried)));
                    true
                }
                NodeKind::SysCall { name } if QUERY_SYSTEM_FUNCTIONS.contains(&name.as_str()) => {
                    let operands = &self.node(current).children;
                    stack.extend(operands.first().map(|operand| (*operand, true)));
                    stack.extend(operands.iter().skip(1).map(|operand| (*operand, queried)));
                    continue;
                }
                NodeKind::SysCall { name } => CONSTANT_SYSTEM_FUNCTIONS.contains(&name.as_str()),
                _ => false,
            };
            if !constant {
                return false;
            }
            stack.extend(
                self.node(current)
                    .children
                    .iter()
                    .map(|child| (*child, queried)),
            );
        }
        true
    }

    /// Order initializer entries identified by their declaration identities
    /// (decision S22-D8). Returns entry indices in execution order: packages
    /// first, each in declaration order, then the compilation unit, then the
    /// design hierarchy in post-order (an instance after the instances below
    /// it). In each scope the class static properties run first, the
    /// constant ones of every class before the others; then, in an instance,
    /// each generate block; then the scope's own declarations. A generate
    /// block and the scope's own declarations each run their constant
    /// initializers first, then the others in declaration order, a static
    /// local at the place of its subroutine or procedure. Ties keep the
    /// declaring scopes in the order collection first reached them.
    /// Collection reaches the kinds of one scope (patterns, handles,
    /// strings, containers, records) in separate passes, so its order alone
    /// does not follow the source; the position among the scope's members
    /// does. The entries of one declaration (unpacked aggregate leaves) keep
    /// their order.
    pub(super) fn initializer_schedule(&self, declarations: &[u32]) -> Vec<usize> {
        let unordered = self.initializer_order.len();
        let order_of = |declaration: &u32| {
            self.initializer_order
                .get(declaration)
                .copied()
                .unwrap_or(unordered)
        };
        let mut positions: HashMap<NodeId, HashMap<NodeId, usize>> = HashMap::new();
        let anchors = declarations
            .iter()
            .map(|declaration| {
                self.initializer_anchor(NodeId::from_index(*declaration as usize), &mut positions)
            })
            .collect::<Vec<_>>();
        // A scope's slot comes from its own declarations; a static local
        // names its scope's slot only when the scope has no other entry, so
        // static locals never move the scopes' landed order.
        let mut scope_order: HashMap<Option<NodeId>, usize> = HashMap::new();
        for ((scope, path), declaration) in anchors.iter().zip(declarations) {
            if path.len() == 1 {
                let order = order_of(declaration);
                scope_order
                    .entry(*scope)
                    .and_modify(|first| *first = (*first).min(order))
                    .or_insert(order);
            }
        }
        let mut nested_order: HashMap<Option<NodeId>, usize> = HashMap::new();
        for ((scope, path), declaration) in anchors.iter().zip(declarations) {
            if path.len() > 1 && !scope_order.contains_key(scope) {
                let order = order_of(declaration);
                nested_order
                    .entry(*scope)
                    .and_modify(|first| *first = (*first).min(order))
                    .or_insert(order);
            }
        }
        scope_order.extend(nested_order);
        let mut instance_keys: HashMap<NodeId, Vec<usize>> = HashMap::new();
        let mut constants: HashMap<u32, bool> = HashMap::new();
        let keys = declarations
            .iter()
            .zip(anchors)
            .map(|(declaration, (scope, position))| {
                let node = NodeId::from_index(*declaration as usize);
                let (owner, sub) = self.initializer_owner(node);
                let (group, index, path) = match owner {
                    InitializerOwner::Package(package) => (0, package.index(), Vec::new()),
                    InitializerOwner::Unit => (1, 0, Vec::new()),
                    InitializerOwner::Instance(instance) => {
                        let path = match instance_keys.get(&instance) {
                            Some(path) => path.clone(),
                            None => {
                                let path = self.instance_post_order(instance, &mut positions);
                                instance_keys.insert(instance, path.clone());
                                path
                            }
                        };
                        (2, 0, path)
                    }
                };
                let constant = *constants
                    .entry(*declaration)
                    .or_insert_with(|| self.has_constant_initializer(node));
                // Sub-rank 0 is the class static properties of the scope:
                // their constants run before every class's other ones.
                let class_variable = sub == 0 && !constant;
                (
                    group,
                    index,
                    path,
                    sub,
                    class_variable,
                    scope_order.get(&scope).copied().unwrap_or(unordered),
                    !constant,
                    position,
                    order_of(declaration),
                )
            })
            .collect::<Vec<_>>();
        let mut order = (0..declarations.len()).collect::<Vec<_>>();
        // A stable sort: the entries of one declaration keep their order.
        order.sort_by(|left, right| keys[*left].cmp(&keys[*right]));
        order
    }
}
