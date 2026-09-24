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

/// Equivalent spellings share a table row. Keep uwire distinct in this pure
/// table even though the current executable-inout admission rejects it.
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

struct Entry {
    parent: usize,
    rank: u8,
    resolved: CollapsedType,
}

#[derive(Default)]
pub(super) struct NetCollapsePlan {
    points: HashMap<NetPoint, usize>,
    entries: Vec<Entry>,
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
            });
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

    pub(super) fn port(
        &mut self,
        internal: NetPoint,
        external: NetPoint,
    ) -> Result<Option<CollapseWarning>, String> {
        let (internal, external) = self.roots(internal, external)?;
        if internal == external {
            return Ok(None);
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
        let resolved = self.entries[winner].resolved.clone();
        let warning = choice.warns().then_some(CollapseWarning {
            internal: internal_kind,
            external: external_kind,
            selected: resolved.kind,
        });
        // Union rank controls storage topology only, never the selected type.
        self.merge(internal, external, resolved);
        Ok(warning)
    }
}

#[cfg(test)]
mod tests;
