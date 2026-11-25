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
    /// Ordered as in the [`Nesting`]; with each node the index of the next
    /// entry outwards.
    entries: Vec<(NodeId, Option<u32>, T)>,
}

impl<T> Enclosing<T> {
    pub(super) fn new(nesting: &Nesting, mut nodes: Vec<(NodeId, T)>) -> Self {
        nodes.sort_by_key(|(node, _)| nesting.first[node.index()]);
        let mut open = Vec::<u32>::new();
        let mut entries: Vec<(NodeId, Option<u32>, T)> = Vec::with_capacity(nodes.len());
        for (node, value) in nodes {
            while open
                .last()
                .is_some_and(|outer| !nesting.contains(entries[*outer as usize].0, node))
            {
                open.pop();
            }
            let outer = open.last().copied();
            open.push(entries.len() as u32);
            entries.push((node, outer, value));
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
            .partition_point(|(entry, ..)| nesting.first[entry.index()] < at);
        let mut next = before.checked_sub(1);
        std::iter::from_fn(move || loop {
            crate::work::count(crate::work::Work::EnclosingSteps, 1);
            let (entry, outer, value) = &self.entries[next?];
            next = outer.map(|outer| outer as usize);
            if nesting.contains(*entry, node) {
                return Some((*entry, value));
            }
        })
    }
}
