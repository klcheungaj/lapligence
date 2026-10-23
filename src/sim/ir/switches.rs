//! Bidirectional pass-switch networks (IEEE 1364-2001 7.6, IEEE 1800-2009
//! 28.8, 28.13-28.14, 4.9.5).
//!
//! A node is one bit of one net group; a switch joins two nodes. The nodes
//! and switches of one statically connected network are numbered
//! contiguously ([`IrSwitchNetwork`]), so the runtime's per-node state of a
//! network is adjacent. A `tranif` sets its conduction state through
//! [`IrStmt::SwitchControl`]; `tran` and `rtran` are always on.
//!
//! [`IrStmt::SwitchControl`]: super::IrStmt::SwitchControl

/// One node: bit `bit` of net group `group`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IrSwitchNode {
    pub(in crate::sim) group: usize,
    pub(in crate::sim) bit: u32,
}

impl IrSwitchNode {
    pub fn new(group: usize, bit: u32) -> Self {
        Self { group, bit }
    }
    pub fn group(&self) -> usize {
        self.group
    }
    pub fn bit(&self) -> u32 {
        self.bit
    }
}

/// One switch between nodes `a` and `b` (indices into
/// [`IrSwitchNetwork::nodes`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrSwitch {
    pub(in crate::sim) a: u32,
    pub(in crate::sim) b: u32,
    /// `rtran`, `rtranif0`, `rtranif1`: Table 28-8 strength reduction.
    pub(in crate::sim) resistive: bool,
    /// `tranif0`, `tranif1`, `rtranif0`, `rtranif1`: the state follows
    /// [`IrStmt::SwitchControl`](super::IrStmt::SwitchControl).
    pub(in crate::sim) controlled: bool,
}

impl IrSwitch {
    pub fn new(a: u32, b: u32, resistive: bool, controlled: bool) -> Self {
        Self {
            a,
            b,
            resistive,
            controlled,
        }
    }
    pub fn a(&self) -> u32 {
        self.a
    }
    pub fn b(&self) -> u32 {
        self.b
    }
    pub fn resistive(&self) -> bool {
        self.resistive
    }
    pub fn controlled(&self) -> bool {
        self.controlled
    }
}

/// Every switch of a design. `components` holds the first node of each
/// statically connected network in ascending order: network `k` owns the
/// nodes from `components[k]` up to the next start, and every switch joins
/// two nodes of one network. Empty for a design without switches.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IrSwitchNetwork {
    pub(in crate::sim) nodes: Vec<IrSwitchNode>,
    pub(in crate::sim) switches: Vec<IrSwitch>,
    pub(in crate::sim) components: Vec<u32>,
}

impl IrSwitchNetwork {
    pub fn new(nodes: Vec<IrSwitchNode>, switches: Vec<IrSwitch>, components: Vec<u32>) -> Self {
        Self {
            nodes,
            switches,
            components,
        }
    }
    pub fn nodes(&self) -> &[IrSwitchNode] {
        &self.nodes
    }
    pub fn switches(&self) -> &[IrSwitch] {
        &self.switches
    }
    pub fn components(&self) -> &[u32] {
        &self.components
    }
    pub fn is_empty(&self) -> bool {
        self.switches.is_empty()
    }

    /// The network a node belongs to.
    pub fn component_of(&self, node: u32) -> Option<usize> {
        if node as usize >= self.nodes.len() {
            return None;
        }
        self.components
            .partition_point(|start| *start <= node)
            .checked_sub(1)
    }
}
