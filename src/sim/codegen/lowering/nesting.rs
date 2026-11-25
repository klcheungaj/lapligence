//! Which nodes hold which: ancestor queries that do not walk the parent links.

use crate::core::db::{Db, NodeId};

/// The nodes in an order that puts a node and everything inside it next to
/// each other.
pub(super) struct Nesting {
    /// Position of each node.
    first: Vec<u32>,
    /// Last position inside each node.
    last: Vec<u32>,
}

impl Nesting {
    pub(super) fn new(db: &Db) -> Self {
        const NONE: u32 = u32::MAX;
        let count = db.node_count();
        // The nodes inside each node, as a list through `next`.
        let mut inside = vec![NONE; count];
        let mut next = vec![NONE; count];
        let mut roots = Vec::new();
        for node in db.node_ids() {
            match db.node(node).parent {
                Some(parent) => {
                    next[node.index()] = inside[parent.index()];
                    inside[parent.index()] = node.index() as u32;
                }
                None => roots.push(node.index() as u32),
            }
        }
        let mut first = vec![0; count];
        let mut last = vec![0; count];
        let mut position = 0;
        let mut open = Vec::new();
        for root in roots {
            open.push(root);
            while let Some(&node) = open.last() {
                let index = node as usize;
                if first[index] == 0 {
                    position += 1;
                    first[index] = position;
                }
                match inside[index] {
                    NONE => {
                        last[index] = position;
                        open.pop();
                    }
                    nested => {
                        inside[index] = next[nested as usize];
                        open.push(nested);
                    }
                }
            }
        }
        Self { first, last }
    }

    /// Whether `inner` is `outer` or inside it.
    pub(super) fn contains(&self, outer: NodeId, inner: NodeId) -> bool {
        let at = self.first[inner.index()];
        self.first[outer.index()] <= at && at <= self.last[outer.index()]
    }
}

/// Nodes with something recorded about each, for walks from a node outwards
/// through those that hold it.
pub(super) struct Enclosing<T> {
    /// Ordered as in the [`Nesting`].
    entries: Vec<Entry<T>>,
}

struct Entry<T> {
    node: NodeId,
    value: T,
    /// The next entry outwards.
    outer: Option<u32>,
    /// How many entries are outwards of this one.
    depth: u32,
    /// One of them, chosen so that the entries passed outwards by jumps and
    /// single steps grow with the logarithm of those between.
    jump: u32,
}

impl<T> Enclosing<T> {
    pub(super) fn new(nesting: &Nesting, mut nodes: Vec<(NodeId, T)>) -> Self {
        nodes.sort_by_key(|(node, _)| nesting.first[node.index()]);
        let mut open = Vec::<u32>::new();
        let mut entries: Vec<Entry<T>> = Vec::with_capacity(nodes.len());
        for (node, value) in nodes {
            while open
                .last()
                .is_some_and(|outer| !nesting.contains(entries[*outer as usize].node, node))
            {
                open.pop();
            }
            let outer = open.last().copied();
            let (depth, jump) = match outer {
                None => (0, entries.len() as u32),
                Some(outer) => {
                    let near = &entries[outer as usize];
                    let middle = &entries[near.jump as usize];
                    let far = &entries[middle.jump as usize];
                    let equal = near.depth - middle.depth == middle.depth - far.depth;
                    (near.depth + 1, if equal { middle.jump } else { outer })
                }
            };
            open.push(entries.len() as u32);
            entries.push(Entry {
                node,
                value,
                outer,
                depth,
                jump,
            });
        }
        Self { entries }
    }

    /// The entries of the nodes that hold `node`, the innermost first.
    pub(super) fn around<'s>(
        &'s self,
        nesting: &'s Nesting,
        node: NodeId,
    ) -> impl Iterator<Item = (NodeId, &'s T)> + 's {
        let at = nesting.first[node.index()];
        let before = self
            .entries
            .partition_point(|entry| nesting.first[entry.node.index()] < at);
        // The entry before `node` holds it or ends before it, and of those
        // around that entry the outer ones hold what the inner ones do.
        let mut next = before.checked_sub(1);
        while let Some(index) = next {
            crate::work::count(crate::work::Work::EnclosingSteps, 1);
            let entry = &self.entries[index];
            if nesting.contains(entry.node, node) {
                break;
            }
            let jump = entry.jump as usize;
            next = if jump != index && !nesting.contains(self.entries[jump].node, node) {
                Some(jump)
            } else {
                entry.outer.map(|outer| outer as usize)
            };
        }
        std::iter::from_fn(move || {
            let entry = &self.entries[next?];
            next = entry.outer.map(|outer| outer as usize);
            Some((entry.node, &entry.value))
        })
    }
}
