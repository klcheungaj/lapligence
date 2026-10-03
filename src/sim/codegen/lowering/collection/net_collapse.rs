//! Directional port-collapse policy, independent of storage and driver allocation.

use super::{AliasBit, NetType, NodeId};
use std::collections::HashMap;

/// Whole connections need one entry, not one allocation per packed bit. A net
/// promoted to the selected/alias path uses bit entries everywhere instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum NetPoint {
    Whole(NodeId),
    Bit(AliasBit),
    ArrayBit {
        owner: NodeId,
        element: u64,
        bit: u32,
    },
}

impl AliasBit {
    pub(super) fn point(self) -> NetPoint {
        match self {
            Self::Net { net, bit } => NetPoint::Bit(Self::Net { net, bit }),
            Self::Array {
                owner,
                element,
                bit,
            } => NetPoint::ArrayBit {
                owner,
                element,
                bit,
            },
        }
    }
}

impl NetPoint {
    pub(super) fn owner(self) -> NodeId {
        match self {
            Self::Whole(owner) | Self::ArrayBit { owner, .. } => owner,
            Self::Bit(bit) => bit.owner(),
        }
    }
}

/// Equivalent spellings share a table row. Keep uwire distinct: it resolves
/// like a wire but carries the single-driver rule (SV 6.6.2).
pub(super) fn canonical_net_type(kind: NetType) -> Option<NetType> {
    match kind {
        NetType::Wire | NetType::Tri | NetType::Logic => Some(NetType::Wire),
        NetType::Wand | NetType::TriAnd => Some(NetType::Wand),
        NetType::Wor | NetType::TriOr => Some(NetType::Wor),
        NetType::TriReg
        | NetType::Tri0
        | NetType::Tri1
        | NetType::Uwire
        | NetType::Supply0
        | NetType::Supply1 => Some(kind),
        NetType::None | NetType::Reg | NetType::Unsupported => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PortChoice {
    External,
    Internal,
    ExternalWarn,
    InternalWarn,
}

impl PortChoice {
    pub(super) fn internal(self) -> bool {
        matches!(self, Self::Internal | Self::InternalWarn)
    }

    pub(super) fn warns(self) -> bool {
        matches!(self, Self::ExternalWarn | Self::InternalWarn)
    }
}

/// Column order of Table 23-1, the documented tie-break between types that
/// do not dominate each other.
fn table_order(kind: NetType) -> usize {
    [
        NetType::Wire,
        NetType::Wand,
        NetType::Wor,
        NetType::TriReg,
        NetType::Tri0,
        NetType::Tri1,
        NetType::Uwire,
        NetType::Supply0,
        NetType::Supply1,
    ]
    .iter()
    .position(|candidate| canonical_net_type(kind) == Some(*candidate))
    .unwrap_or(usize::MAX)
}

/// `winner` beats `loser` on either side of a port without a warning.
fn dominates(winner: NetType, loser: NetType) -> bool {
    port_choice(winner, loser) == Some(PortChoice::Internal)
        && port_choice(loser, winner) == Some(PortChoice::External)
}

/// IEEE 1800-2009 Table 23-1 (1364-2001 Table 45 without the uwire row/column).
/// Rows are internal; columns are external. The choice also selects the delay
/// owner, including an absent delay, not just the runtime truth table.
pub(super) fn port_choice(internal: NetType, external: NetType) -> Option<PortChoice> {
    fn index(kind: NetType) -> Option<usize> {
        Some(match canonical_net_type(kind)? {
            NetType::Wire => 0,
            NetType::Wand => 1,
            NetType::Wor => 2,
            NetType::TriReg => 3,
            NetType::Tri0 => 4,
            NetType::Tri1 => 5,
            NetType::Uwire => 6,
            NetType::Supply0 => 7,
            NetType::Supply1 => 8,
            _ => return None,
        })
    }
    use PortChoice::{External as E, ExternalWarn as EW, Internal as I, InternalWarn as IW};
    const TABLE: [[PortChoice; 9]; 9] = [
        [E, E, E, E, E, E, E, E, E],
        [I, E, EW, EW, EW, EW, EW, E, E],
        [I, EW, E, EW, EW, EW, EW, E, E],
        [I, EW, EW, E, E, E, EW, E, E],
        [I, EW, EW, I, E, EW, EW, E, E],
        [I, EW, EW, I, EW, E, EW, E, E],
        [I, IW, IW, IW, IW, IW, E, E, E],
        [I, I, I, I, I, I, I, E, EW],
        [I, I, I, I, I, I, I, EW, E],
    ];
    Some(TABLE[index(internal)?][index(external)?])
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CollapsedType {
    pub(super) kind: NetType,
    /// True aliases have no external side: retain their existing delay-conflict
    /// checks. A port selects only the winning component's declaration(s).
    pub(super) delay_members: Vec<NodeId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct CollapseWarning {
    pub(super) internal: NetType,
    pub(super) external: NetType,
    pub(super) selected: NetType,
}

/// A same-depth batch whose winning edges name mutually non-dominating types.
/// Table 23-1 defines pairs only, so the plan picks the first maximal type in
/// table column order, independently of declaration order, and reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CollapseTie {
    /// Index of the first batch edge in the tied component.
    pub(super) edge: usize,
    pub(super) candidates: Vec<NetType>,
    pub(super) selected: NetType,
}

/// Per-edge pairwise warnings (edge index) and same-depth ties of one batch.
pub(super) type BatchOutcome = (Vec<(usize, CollapseWarning)>, Vec<CollapseTie>);

struct Entry {
    parent: usize,
    rank: u8,
    resolved: CollapsedType,
    /// Some declaration in the component is a uwire (SV 6.6.2 single driver).
    uwire: bool,
}

#[derive(Default)]
pub(super) struct NetCollapsePlan {
    points: HashMap<NetPoint, usize>,
    entries: Vec<Entry>,
    any_uwire: bool,
}

impl NetCollapsePlan {
    pub(super) fn insert(&mut self, point: NetPoint, kind: NetType) -> Result<(), String> {
        let kind = canonical_net_type(kind).ok_or("port collapse requires a net type")?;
        if !self.points.contains_key(&point) {
            let index = self.entries.len();
            self.points.insert(point, index);
            self.entries.push(Entry {
                parent: index,
                rank: 0,
                resolved: CollapsedType {
                    kind,
                    delay_members: vec![point.owner()],
                },
                uwire: kind == NetType::Uwire,
            });
            self.any_uwire |= kind == NetType::Uwire;
        }
        Ok(())
    }

    fn root(&self, mut index: usize) -> usize {
        while self.entries[index].parent != index {
            index = self.entries[index].parent;
        }
        index
    }

    pub(super) fn component(&self, point: NetPoint) -> Option<usize> {
        self.points.get(&point).map(|index| self.root(*index))
    }

    pub(super) fn resolved(&self, point: NetPoint) -> Option<&CollapsedType> {
        Some(&self.entries[self.component(point)?].resolved)
    }

    /// Whether the plan has any uwire declaration at all.
    pub(super) fn any_uwire(&self) -> bool {
        self.any_uwire
    }

    /// Whether any declaration collapsed into this point's net is a uwire.
    pub(super) fn contains_uwire(&self, point: NetPoint) -> bool {
        self.component(point)
            .is_some_and(|root| self.entries[root].uwire)
    }

    fn roots(&self, first: NetPoint, second: NetPoint) -> Result<(usize, usize), String> {
        Ok((
            self.component(first)
                .ok_or("missing first port-collapse endpoint")?,
            self.component(second)
                .ok_or("missing second port-collapse endpoint")?,
        ))
    }

    fn merge(&mut self, mut first: usize, mut second: usize, resolved: CollapsedType) {
        if self.entries[first].rank < self.entries[second].rank {
            std::mem::swap(&mut first, &mut second);
        }
        if self.entries[first].rank == self.entries[second].rank {
            self.entries[first].rank += 1;
        }
        self.entries[second].parent = first;
        self.entries[second].resolved.delay_members.clear();
        self.entries[first].resolved = resolved;
        self.entries[first].uwire |= self.entries[second].uwire;
    }

    /// Call for source alias statements before introducing port edges. Port
    /// compatibility must never legalize an invalid alias declaration.
    pub(super) fn alias(&mut self, first: NetPoint, second: NetPoint) -> Result<(), String> {
        let (first, second) = self.roots(first, second)?;
        let mut resolved = self.entries[first].resolved.clone();
        if resolved.kind != self.entries[second].resolved.kind {
            return Err(
                "alias members have incompatible net types (ports use different rules)".into(),
            );
        }
        if first != second {
            resolved
                .delay_members
                .extend(&self.entries[second].resolved.delay_members);
            resolved.delay_members.sort_by_key(|node| node.index());
            resolved.delay_members.dedup();
            self.merge(first, second, resolved);
        }
        Ok(())
    }

    /// Collapse every port edge of one hierarchy depth as one batch. Each
    /// edge compares the types its endpoints had before the batch, so the
    /// order of sibling instances or ports cannot change the result. Winning
    /// types of one merged component reduce to the types no other winner
    /// dominates (Table 23-1 strict dominance, independent of sides); a
    /// remaining warning-only tie selects the first in table column order.
    /// The delay owners are every winning declaration of the selected type.
    /// Returns the per-edge pairwise warnings and any ties.
    pub(super) fn port_batch(
        &mut self,
        edges: &[(NetPoint, NetPoint)],
    ) -> Result<BatchOutcome, String> {
        // Edge winners against the pre-batch component types.
        let mut winners = Vec::with_capacity(edges.len());
        let mut warnings = Vec::new();
        for (index, (internal, external)) in edges.iter().enumerate() {
            let (internal, external) = self.roots(*internal, *external)?;
            if internal == external {
                winners.push(None);
                continue;
            }
            let internal_kind = self.entries[internal].resolved.kind;
            let external_kind = self.entries[external].resolved.kind;
            let choice = port_choice(internal_kind, external_kind)
                .ok_or("unsupported net type in port collapse")?;
            let winner = if choice.internal() {
                internal
            } else {
                external
            };
            if choice.warns() {
                warnings.push((
                    index,
                    CollapseWarning {
                        internal: internal_kind,
                        external: external_kind,
                        selected: self.entries[winner].resolved.kind,
                    },
                ));
            }
            winners.push(Some((internal, external, winner)));
        }
        // Batch-local union of the touched pre-batch roots.
        let mut local: HashMap<usize, usize> = HashMap::new();
        fn find(local: &mut HashMap<usize, usize>, mut node: usize) -> usize {
            let mut path = Vec::new();
            while let Some(&parent) = local.get(&node) {
                if parent == node {
                    break;
                }
                path.push(node);
                node = parent;
            }
            for member in path {
                local.insert(member, node);
            }
            node
        }
        for (internal, external, _) in winners.iter().flatten() {
            local.entry(*internal).or_insert(*internal);
            local.entry(*external).or_insert(*external);
            let (a, b) = (find(&mut local, *internal), find(&mut local, *external));
            if a != b {
                // Deterministic: the smaller entry index becomes the local root.
                let (low, high) = (a.min(b), a.max(b));
                local.insert(high, low);
            }
        }
        // Per merged component: its first edge, all roots and winning roots.
        let mut components: HashMap<usize, (usize, Vec<usize>, Vec<usize>)> = HashMap::new();
        for (index, edge) in winners.iter().enumerate() {
            let Some((internal, external, winner)) = *edge else {
                continue;
            };
            let root = find(&mut local, internal);
            let component = components
                .entry(root)
                .or_insert_with(|| (index, Vec::new(), Vec::new()));
            component.1.extend([internal, external]);
            component.2.push(winner);
        }
        let mut components = components.into_values().collect::<Vec<_>>();
        components.sort_by_key(|component| component.0);
        let mut ties = Vec::new();
        for (edge, mut roots, mut winning) in components {
            roots.sort_unstable();
            roots.dedup();
            winning.sort_unstable();
            winning.dedup();
            let mut kinds = winning
                .iter()
                .map(|root| self.entries[*root].resolved.kind)
                .collect::<Vec<_>>();
            kinds.sort_by_key(|kind| table_order(*kind));
            kinds.dedup();
            let maximal = kinds
                .iter()
                .copied()
                .filter(|kind| !kinds.iter().any(|other| dominates(*other, *kind)))
                .collect::<Vec<_>>();
            let selected = *maximal.first().ok_or("port collapse batch has no winner")?;
            if maximal.len() > 1 {
                ties.push(CollapseTie {
                    edge,
                    candidates: maximal.clone(),
                    selected,
                });
            }
            let mut delay_members = winning
                .iter()
                .filter(|root| self.entries[**root].resolved.kind == selected)
                .flat_map(|root| self.entries[*root].resolved.delay_members.iter().copied())
                .collect::<Vec<_>>();
            delay_members.sort_by_key(|node| node.index());
            delay_members.dedup();
            let resolved = CollapsedType {
                kind: selected,
                delay_members,
            };
            let mut merged = roots[0];
            for other in roots.into_iter().skip(1) {
                let (first, second) = (self.root(merged), self.root(other));
                if first != second {
                    self.merge(first, second, resolved.clone());
                }
                merged = self.root(first);
            }
            let root = self.root(merged);
            self.entries[root].resolved = resolved;
        }
        Ok((warnings, ties))
    }

    /// One port edge as its own batch (unit-test convenience).
    #[cfg(test)]
    pub(super) fn port(
        &mut self,
        internal: NetPoint,
        external: NetPoint,
    ) -> Result<Option<CollapseWarning>, String> {
        let (warnings, _) = self.port_batch(&[(internal, external)])?;
        Ok(warnings.into_iter().next().map(|(_, warning)| warning))
    }
}

#[cfg(test)]
mod tests;
