//! Static declaration-initializer ordering.
//!
//! SystemVerilog static initialization (SV §§6.21, 10.5) runs before any
//! process starts, and a declaration must precede its simple references.
//! Collection discovers initializers out of source order: initializers that
//! call subroutines or read storage of not-yet-collected scopes are lowered
//! late, static subroutine locals are found while lowering bodies, and
//! descriptor-backed fixed arrays are lowered after every array is collected.
//! The schedule therefore orders initializers by the static declarations
//! their values read, directly or through called functions, and otherwise
//! by scope (packages, the compilation unit, then the design hierarchy in
//! post-order; class statics first in each),
//! the order in which collection first reached each declaring scope, and
//! declaration order within the scope.

use super::*;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// The scope whose initialization runs a static initializer (S22-D8).
#[derive(Clone, Copy)]
enum InitializerOwner {
    Package(NodeId),
    Unit,
    Instance(NodeId),
}

/// Dependency graph over initializer entries followed by pass-through
/// subroutine and multi-entry declaration nodes. `dependencies[node]` lists nodes that must complete
/// before `node`; only entries (`node < priorities.len()`) are emitted.
pub(super) struct InitializerGraph {
    /// Base order of each entry; smaller runs first among ready entries.
    pub(super) priorities: Vec<usize>,
    pub(super) dependencies: Vec<Vec<usize>>,
}

impl InitializerGraph {
    /// Topologically order the strongly connected components, choosing the
    /// ready component with the earliest-declared entry first, so independent
    /// initializers keep declaration order. A dependency cycle (possible only
    /// through recursive helpers or forward reads the frontend accepts) runs
    /// its entries in declaration order, as plain declaration order would.
    pub(super) fn schedule(&self) -> Vec<usize> {
        let entries = self.priorities.len();
        let components = self.components();
        let count = components.iter().max().map_or(0, |last| last + 1);
        let mut members = vec![Vec::new(); count];
        for (node, component) in components.iter().enumerate() {
            if node < entries {
                members[*component].push(node);
            }
        }
        for list in &mut members {
            list.sort_by_key(|entry| (self.priorities[*entry], *entry));
        }
        // Components without entries are pass-through nodes; they sort
        // before every entry once ready and never delay one.
        let key = |component: usize| {
            members[component]
                .first()
                .map_or((0, 0, component), |entry| {
                    (1, self.priorities[*entry], *entry)
                })
        };
        let mut pending = vec![0usize; count];
        let mut dependents = vec![Vec::new(); count];
        for (node, dependencies) in self.dependencies.iter().enumerate() {
            for dependency in dependencies {
                let (from, to) = (components[*dependency], components[node]);
                if from != to {
                    pending[to] += 1;
                    dependents[from].push(to);
                }
            }
        }
        let mut ready: BinaryHeap<_> = (0..count)
            .filter(|component| pending[*component] == 0)
            .map(|component| Reverse(key(component)))
            .collect();
        let mut order = Vec::with_capacity(entries);
        while let Some(Reverse((class, _, id))) = ready.pop() {
            let component = if class == 0 { id } else { components[id] };
            order.extend(members[component].iter().copied());
            for dependent in &dependents[component] {
                pending[*dependent] -= 1;
                if pending[*dependent] == 0 {
                    ready.push(Reverse(key(*dependent)));
                }
            }
        }
        order
    }

    /// Iterative Tarjan strongly connected components; returns each node's
    /// component index. Edges run from a node to its dependencies.
    fn components(&self) -> Vec<usize> {
        const UNVISITED: usize = usize::MAX;
        let nodes = self.dependencies.len();
        let mut index = vec![UNVISITED; nodes];
        let mut low = vec![0usize; nodes];
        let mut on_stack = vec![false; nodes];
        let mut component = vec![UNVISITED; nodes];
        let mut stack = Vec::new();
        let mut next_index = 0usize;
        let mut next_component = 0usize;
        for root in 0..nodes {
            if index[root] != UNVISITED {
                continue;
            }
            let mut frames = vec![(root, 0usize)];
            index[root] = next_index;
            low[root] = next_index;
            next_index += 1;
            stack.push(root);
            on_stack[root] = true;
            while let Some((node, edge)) = frames.last_mut() {
                let node = *node;
                if let Some(next) = self.dependencies[node].get(*edge).copied() {
                    *edge += 1;
                    if index[next] == UNVISITED {
                        index[next] = next_index;
                        low[next] = next_index;
                        next_index += 1;
                        stack.push(next);
                        on_stack[next] = true;
                        frames.push((next, 0));
                    } else if on_stack[next] {
                        low[node] = low[node].min(index[next]);
                    }
                    continue;
                }
                frames.pop();
                if let Some((parent, _)) = frames.last() {
                    low[*parent] = low[*parent].min(low[node]);
                }
                if low[node] == index[node] {
                    while let Some(member) = stack.pop() {
                        on_stack[member] = false;
                        component[member] = next_component;
                        if member == node {
                            break;
                        }
                    }
                    next_component += 1;
                }
            }
        }
        component
    }
}

/// Static declarations and subroutines an initializer expression or
/// subroutine body reaches directly.
#[derive(Default)]
struct DirectReads {
    declarations: Vec<NodeId>,
    subroutines: Vec<NodeId>,
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
    /// for dependency ordering.
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

    /// The base order of each entry (decision S22-D8): packages first, each
    /// in declaration order, then the compilation unit, then the design
    /// hierarchy in post-order (an instance after the instances below it);
    /// in each scope class static properties, then (in an instance) the
    /// variables of its generate blocks, then its own variables. Ties keep
    /// the declaring scopes in the order collection first reached them and
    /// declaration order within the scope. Collection reaches the kinds of
    /// one scope (patterns, handles, strings, containers, records) in
    /// separate passes, so its order alone does not follow the source; the
    /// position among the scope's members does.
    fn initializer_priorities(&self, declarations: &[u32]) -> Vec<usize> {
        let unordered = self.initializer_order.len();
        let order_of = |declaration: &u32| {
            self.initializer_order
                .get(declaration)
                .copied()
                .unwrap_or(unordered)
        };
        let mut scope_order: HashMap<Option<NodeId>, usize> = HashMap::new();
        let mut positions: HashMap<NodeId, HashMap<NodeId, usize>> = HashMap::new();
        for declaration in declarations {
            let node = NodeId::from_index(*declaration as usize);
            let scope = self.node(node).parent;
            let order = order_of(declaration);
            scope_order
                .entry(scope)
                .and_modify(|first| *first = (*first).min(order))
                .or_insert(order);
            if let Some(scope) = scope {
                positions.entry(scope).or_insert_with(|| {
                    self.node(scope)
                        .children
                        .iter()
                        .enumerate()
                        .map(|(position, child)| (*child, position))
                        .collect()
                });
            }
        }
        let mut instance_keys: HashMap<NodeId, Vec<usize>> = HashMap::new();
        let keys = declarations
            .iter()
            .map(|declaration| {
                let node = NodeId::from_index(*declaration as usize);
                let scope = self.node(node).parent;
                let position = scope
                    .and_then(|scope| positions.get(&scope)?.get(&node).copied())
                    .unwrap_or(0);
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
                (
                    group,
                    index,
                    path,
                    sub,
                    scope_order.get(&scope).copied().unwrap_or(unordered),
                    position,
                    order_of(declaration),
                )
            })
            .collect::<Vec<_>>();
        let mut sorted = keys.clone();
        sorted.sort_unstable();
        sorted.dedup();
        keys.iter()
            .map(|key| sorted.binary_search(key).unwrap_or_else(|index| index))
            .collect()
    }

    /// Order initializer entries identified by their declaration identities.
    /// Returns entry indices in execution order.
    pub(super) fn initializer_schedule(&self, declarations: &[u32]) -> Vec<usize> {
        let mut entries_of: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
        for (entry, declaration) in declarations.iter().enumerate() {
            entries_of.entry(*declaration).or_default().push(entry);
        }
        let priorities = self.initializer_priorities(declarations);
        let mut dependencies: Vec<Vec<usize>> = vec![Vec::new(); declarations.len()];
        // A declaration with several entries (unpacked aggregate leaves) is
        // reached through one pass-through hub, so readers add one edge each
        // instead of one per leaf.
        let mut declaration_nodes: HashMap<u32, usize> = HashMap::new();
        for (declaration, entries) in &entries_of {
            let node = match entries.as_slice() {
                [entry] => *entry,
                _ => {
                    dependencies.push(entries.clone());
                    dependencies.len() - 1
                }
            };
            declaration_nodes.insert(*declaration, node);
        }
        let mut subroutine_nodes: HashMap<NodeId, usize> = HashMap::new();
        let mut worklist: Vec<(NodeId, usize)> = Vec::new();
        let mut add_reads = |node: usize,
                             own: Option<u32>,
                             reads: DirectReads,
                             dependencies: &mut Vec<Vec<usize>>,
                             worklist: &mut Vec<(NodeId, usize)>| {
            for declaration in reads.declarations {
                let identity = declaration.index() as u32;
                if Some(identity) == own {
                    continue;
                }
                if let Some(target) = declaration_nodes.get(&identity) {
                    dependencies[node].push(*target);
                }
            }
            for subroutine in reads.subroutines {
                let target = *subroutine_nodes.entry(subroutine).or_insert_with(|| {
                    dependencies.push(Vec::new());
                    worklist.push((subroutine, dependencies.len() - 1));
                    dependencies.len() - 1
                });
                dependencies[node].push(target);
            }
        };
        for (entry, declaration) in declarations.iter().enumerate() {
            let mut reads = DirectReads::default();
            for source in self
                .initializer_sources
                .get(declaration)
                .into_iter()
                .flatten()
            {
                self.collect_direct_reads(*source, false, &mut reads);
            }
            add_reads(
                entry,
                Some(*declaration),
                reads,
                &mut dependencies,
                &mut worklist,
            );
        }
        while let Some((subroutine, node)) = worklist.pop() {
            let mut reads = DirectReads::default();
            self.collect_direct_reads(subroutine, true, &mut reads);
            add_reads(node, None, reads, &mut dependencies, &mut worklist);
        }
        for list in &mut dependencies {
            list.sort_unstable();
            list.dedup();
        }
        InitializerGraph {
            priorities,
            dependencies,
        }
        .schedule()
    }

    /// Collect direct declaration reads and subroutine calls below `node`.
    /// Inside a subroutine body every local declaration is also a read, so
    /// a static local initializes before any initializer calls its owner,
    /// and automatic local initializers execute as part of the call.
    fn collect_direct_reads(&self, node: NodeId, in_subroutine: bool, reads: &mut DirectReads) {
        let mut stack = vec![node];
        while let Some(current) = stack.pop() {
            match self.kind(current) {
                NodeKind::Expr(ExprKind::Ref {
                    target: Some(target),
                }) => reads.declarations.push(*target),
                NodeKind::Expr(ExprKind::HierPath { refs, .. }) => {
                    reads.declarations.extend(refs.iter().flatten().copied());
                }
                NodeKind::Expr(ExprKind::AssertionInstance { body, bindings, .. }) => {
                    stack.push(*body);
                    stack.extend(bindings.iter().map(|binding| binding.actual));
                }
                NodeKind::FuncCall {
                    name,
                    is_task,
                    callee,
                    ..
                } => {
                    let resolved = callee.or_else(|| {
                        let instance = self.owner_instance(current)?;
                        self.resolve_callee_env(instance, name, *is_task, None)
                            .ok()
                            .map(|(subroutine, _)| subroutine)
                    });
                    reads.subroutines.extend(resolved);
                }
                NodeKind::Var { .. } | NodeKind::Array { .. } if in_subroutine => {
                    reads.declarations.push(current);
                    if let Some(initializer) = self.db.var_initializer(current) {
                        stack.push(initializer);
                    }
                }
                _ => {}
            }
            stack.extend(self.node(current).children.iter().copied());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::InitializerGraph;

    #[test]
    fn independent_initializers_keep_declaration_order() {
        let graph = InitializerGraph {
            priorities: vec![2, 0, 1],
            dependencies: vec![Vec::new(); 3],
        };
        assert_eq!(graph.schedule(), vec![1, 2, 0]);
    }

    #[test]
    fn readers_follow_late_lowered_writers_through_subroutines() {
        // Entry 0 (`int b = a + 1`) reads entry 1 (`int a = f()`, lowered
        // late); entry 2 calls subroutine node 3, which reads entry 1.
        let graph = InitializerGraph {
            priorities: vec![1, 0, 2],
            dependencies: vec![vec![1], vec![], vec![3], vec![1]],
        };
        assert_eq!(graph.schedule(), vec![1, 0, 2]);
        let late = InitializerGraph {
            priorities: vec![0, 5, 1],
            dependencies: vec![vec![1], vec![], vec![3], vec![1]],
        };
        assert_eq!(late.schedule(), vec![1, 0, 2]);
    }

    #[test]
    fn cycles_break_at_the_earliest_declaration() {
        // Entries 0 and 1 reach each other through recursive subroutines 3
        // and 4; entry 2 is independent and declared last.
        let graph = InitializerGraph {
            priorities: vec![1, 0, 2],
            dependencies: vec![vec![3], vec![4], vec![], vec![4, 1], vec![3, 0]],
        };
        assert_eq!(graph.schedule(), vec![1, 0, 2]);
    }
}
