//! Input and output net ports merged like inout connections (decisions
//! A21-D4, A21-D5).
//!
//! A port connects a source to a sink by a continuous assignment (23.3.3),
//! and 23.3.3.7 permits merging the nets on both sides into one simulated net
//! of the type Table 23-1 selects. A link resolves each side separately, so
//! it differs from that net whenever the sink is not simply a second name of
//! its source: the sink has drivers of its own (an input net the child drives
//! is coerced to inout, 23.3.3.1; an output actual has another driver) or a
//! type that is not dominated by every other type (a `tri0`, `wand` or
//! `supply0` formal, a `tri1` actual). Such a port joins the inout union, so
//! both nets become one resolved group holding both sides' drivers, typed
//! by the shared type plan, and the port emits no link.
//!
//! A sink that is a `wire`/`tri`/`logic` net only its port drives is the
//! source under another name; it stays with the cheaper storage alias
//! (`port_sinks.rs`, A21-D2) or its link.
//!
//! A port whose actual is a constant bit or part select of nets, a net-array
//! element (or a constant select of one) or a concatenation of those merges
//! bit by bit (A21-D5): no storage alias can name part of a net, so its
//! formal joins the bit-level union that selected inout actuals use, MSB to
//! MSB. It merges even where a copy would show the same values one update
//! later: the merged bits share one group with the rest of their vector
//! (`net_partition.rs`), which simulates faster than the copies, and each
//! merged formal is a one-run net alias view, a table row without storage of
//! its own (`IrSignal::net_view_part`), which is smaller than the copy's net
//! and link (A21-D5 trade-off). A merged net carries bit-level bindings
//! (`IrSignal::net_alias`), which a storage alias cannot share, so a whole
//! port with such a net on either side merges into it too instead of keeping
//! its link.
//!
//! A port also keeps its link when a merge would change more than the nets'
//! resolution: either side is not a net of a type the merge supports (a
//! variable, a `uwire` or `trireg` net), the sides differ in width or
//! packed type or the actual converts (the link converts), either side has a
//! net delay or a net takes part in a net alias, the port has a drive
//! strength, a member write the resolved group rejects reaches either side,
//! a select is not constant, a selected net-array cell has no other
//! connection, the design reads SDF, or either instance has module paths or
//! timing checks. A model that keeps the VPI catalog merges too: both names
//! stay nets of the one group, so `vpi_handle_by_name` of either shows the
//! merged value and drivers, and a put or force through either name reaches
//! both, as for an inout port.

use super::super::net_cells::NetCellPlan;
use super::*;

/// One input or output port that may merge.
struct MergeCandidate {
    port: NodeId,
    connection: MergeConnection,
}

enum MergeConnection {
    /// A whole connection of two nets of one packed type.
    Whole {
        actual: NodeId,
        formal: NodeId,
        sink: NodeId,
    },
    /// A selected actual: its bits paired MSB to MSB with the formal's, and
    /// every net either side names (the formal first; net arrays excluded).
    Selected {
        output: bool,
        bits: Vec<(AliasBit, AliasBit)>,
        nets: Vec<NodeId>,
    },
}

/// One merged port with a selected actual: (actual, formal) bit pairs.
pub(super) struct SelectedPortMerge {
    pub(super) port: NodeId,
    pub(super) bits: Vec<(AliasBit, AliasBit)>,
}

/// The ports [`Codegen::merged_net_ports`] merges, in node order.
#[derive(Default)]
pub(super) struct PortMerges {
    /// Whole connections, joined by the whole-net inout union.
    pub(super) whole: Vec<NodeId>,
    /// Selected actuals, joined by the bit-level union.
    pub(super) selected: Vec<SelectedPortMerge>,
}

/// The facts of `build_net_groups` the merge decision reads.
pub(super) struct MergeContext<'n> {
    pub(super) nodes: &'n [NodeId],
    pub(super) writers: &'n NetWriterIndex,
    /// The whole-net inout unions before any merge.
    pub(super) parent: &'n HashMap<NodeId, NodeId>,
    /// Nets that take part in a true net alias.
    pub(super) true_aliases: &'n HashSet<NodeId>,
    /// Nets already connected bit by bit (true aliases, selected inouts).
    pub(super) bit_nets: &'n HashSet<NodeId>,
    pub(super) cells: &'n NetCellPlan,
}

impl Codegen<'_> {
    /// The input and output ports whose nets merge into one inout-style
    /// group (module docs).
    pub(super) fn merged_net_ports(&self, context: &MergeContext<'_>) -> PortMerges {
        if !self.merge_ports || self.sdf.is_some() {
            return PortMerges::default();
        }
        let specify = self.specify_instances();
        let mut candidates = Vec::new();
        for port in context.nodes {
            if let Some(candidate) = self.merge_candidate(*port, context, &specify) {
                candidates.push(candidate);
            }
        }
        if candidates.is_empty() {
            return PortMerges::default();
        }
        let merged = Self::propagate_merges(&candidates, context, |candidate| {
            match &candidate.connection {
                // A merged selected port is never slower or larger than its
                // copy (A21-D5 trade-off) and is exactly one simulated net.
                MergeConnection::Selected { .. } => true,
                MergeConnection::Whole { sink, .. } => {
                    context.parent.contains_key(sink)
                        || self.sink_differs_from_source(*sink, candidate.port, context)
                }
            }
        });
        let mut merges = PortMerges::default();
        for (candidate, merged) in candidates.into_iter().zip(merged) {
            if !merged {
                continue;
            }
            match candidate.connection {
                MergeConnection::Whole { .. } => merges.whole.push(candidate.port),
                MergeConnection::Selected { bits, .. } => merges.selected.push(SelectedPortMerge {
                    port: candidate.port,
                    bits,
                }),
            }
        }
        merges
    }

    /// Close the seed merges over the connections a merge changes; each
    /// candidate and net is queued at most once.
    ///
    /// - A merge puts the nets of both sides into one group, so a whole
    ///   candidate whose sink is one of them merges too (the sink now has
    ///   the other side's drivers).
    /// - A net connected bit by bit (a true alias, a selected inout or a
    ///   selected merge, and every net whole-connected to one) carries
    ///   bit-level bindings that a storage alias cannot share, so every
    ///   whole candidate with such a net on either side merges into it.
    fn propagate_merges(
        candidates: &[MergeCandidate],
        context: &MergeContext<'_>,
        seed: impl Fn(&MergeCandidate) -> bool,
    ) -> Vec<bool> {
        let mut by_sink: HashMap<NodeId, Vec<usize>> = HashMap::new();
        let mut by_net: HashMap<NodeId, Vec<usize>> = HashMap::new();
        for (index, candidate) in candidates.iter().enumerate() {
            if let MergeConnection::Whole {
                actual,
                formal,
                sink,
            } = &candidate.connection
            {
                by_sink.entry(*sink).or_default().push(index);
                by_net.entry(*actual).or_default().push(index);
                by_net.entry(*formal).or_default().push(index);
            } else if let MergeConnection::Selected { output, nets, .. } = &candidate.connection {
                // The sink: the formal of an input, the actual nets of an
                // output.
                let sinks = if *output { &nets[1..] } else { &nets[..1] };
                for net in sinks {
                    by_sink.entry(*net).or_default().push(index);
                }
            }
        }
        // Whole-net connections: inout unions now, whole merges as they
        // happen. Bit-level membership spreads along them.
        let mut links: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
        for (child, root) in context.parent {
            if child != root {
                links.entry(*child).or_default().push(*root);
                links.entry(*root).or_default().push(*child);
            }
        }
        let mut merged = vec![false; candidates.len()];
        let mut work = Vec::new();
        let mut bit_level: HashSet<NodeId> = HashSet::new();
        let mut bit_work: Vec<NodeId> = Vec::new();
        for net in sorted_node_set(context.bit_nets) {
            if bit_level.insert(net) {
                bit_work.push(net);
            }
        }
        for (index, candidate) in candidates.iter().enumerate() {
            if seed(candidate) {
                merged[index] = true;
                work.push(index);
            }
        }
        let merge = |index: usize, merged: &mut [bool], work: &mut Vec<usize>| {
            if !merged[index] {
                merged[index] = true;
                work.push(index);
            }
        };
        loop {
            if let Some(net) = bit_work.pop() {
                for next in links.get(&net).into_iter().flatten() {
                    if bit_level.insert(*next) {
                        bit_work.push(*next);
                    }
                }
                for index in by_net.get(&net).into_iter().flatten() {
                    merge(*index, &mut merged, &mut work);
                }
                continue;
            }
            let Some(index) = work.pop() else {
                break;
            };
            let sides = match &candidates[index].connection {
                MergeConnection::Whole { actual, formal, .. } => {
                    links.entry(*actual).or_default().push(*formal);
                    links.entry(*formal).or_default().push(*actual);
                    if bit_level.contains(actual) || bit_level.contains(formal) {
                        for net in [*actual, *formal] {
                            if bit_level.insert(net) {
                                bit_work.push(net);
                            }
                        }
                    }
                    vec![*actual, *formal]
                }
                MergeConnection::Selected { nets, .. } => {
                    for net in nets {
                        if bit_level.insert(*net) {
                            bit_work.push(*net);
                        }
                    }
                    nets.clone()
                }
            };
            for net in sides {
                for next in by_sink.get(&net).into_iter().flatten() {
                    merge(*next, &mut merged, &mut work);
                }
            }
        }
        merged
    }

    /// An input or output connection between nets that a merge keeps exact
    /// (module docs), or `None`.
    fn merge_candidate(
        &self,
        port: NodeId,
        context: &MergeContext<'_>,
        specify: &HashSet<NodeId>,
    ) -> Option<MergeCandidate> {
        let NodeKind::Port {
            direction,
            strength0,
            strength1,
            low,
            ..
        } = self.kind(port)
        else {
            return None;
        };
        if !matches!(direction, DbDirection::Input | DbDirection::Output)
            || self
                .owning_inst(port)
                .is_some_and(|instance| specify.contains(&instance))
        {
            return None;
        }
        let connection = match self.whole_port_connection(port) {
            Some((direction, actual, formal)) => {
                let sink = match direction {
                    DbDirection::Input => formal,
                    _ => actual,
                };
                self.whole_merge_connection(actual, formal, sink, context, specify)?
            }
            None => self.selected_merge_connection(
                port,
                matches!(direction, DbDirection::Output),
                context,
                specify,
            )?,
        };
        // A link with a strength of its own drives with it; the merged net
        // has no such driver.
        if self
            .effective_port_driver_strengths(port, *strength0, *strength1, *low)
            .ok()
            != Some((6, 6))
        {
            return None;
        }
        // Writes a resolved group rejects stay on their own net's group.
        let nets = match &connection {
            MergeConnection::Whole { actual, formal, .. } => vec![*actual, *formal],
            MergeConnection::Selected { nets, .. } => nets.clone(),
        };
        if self
            .unsupported_member_write(context.nodes, context.writers, &nets)
            .is_some()
        {
            return None;
        }
        Some(MergeCandidate { port, connection })
    }

    fn whole_merge_connection(
        &self,
        actual: NodeId,
        formal: NodeId,
        sink: NodeId,
        context: &MergeContext<'_>,
        specify: &HashSet<NodeId>,
    ) -> Option<MergeConnection> {
        if actual == formal
            || self
                .owning_inst(actual)
                .is_some_and(|instance| specify.contains(&instance))
            || !self.mergeable_port_net(actual, context.true_aliases)
            || !self.mergeable_port_net(formal, context.true_aliases)
        {
            return None;
        }
        let storage = |net: NodeId| {
            self.signal_of(net)
                .and_then(|info| self.model.signals.get(info.ir))
                .map(|signal| &signal.ty)
        };
        match (storage(actual), storage(formal)) {
            (Some(outer @ IrType::Packed { .. }), Some(inner)) if outer == inner => {}
            _ => return None,
        }
        Some(MergeConnection::Whole {
            actual,
            formal,
            sink,
        })
    }

    /// A port whose actual selects constant bits of nets or net-array cells
    /// (or concatenates such selections) and whose formal is a whole net of
    /// the same width, or `None`.
    fn selected_merge_connection(
        &self,
        port: NodeId,
        output: bool,
        context: &MergeContext<'_>,
        specify: &HashSet<NodeId>,
    ) -> Option<MergeConnection> {
        let NodeKind::Port {
            high,
            low: Some(formal),
            high_expr,
            ..
        } = self.kind(port)
        else {
            return None;
        };
        let actual = high_expr.or(*high)?;
        if self
            .node(port)
            .children
            .iter()
            .any(|child| matches!(self.kind(*child), NodeKind::IfaceConn { .. }))
            || !self.db.multi_port_components(port).is_empty()
            || self.db.multi_port_of(port).is_some()
            || !self.plain_net_selection(actual)
            || !self.mergeable_port_net(*formal, context.true_aliases)
            || !matches!(
                self.signal_of(*formal)
                    .and_then(|info| self.model.signals.get(info.ir))
                    .map(|signal| &signal.ty),
                Some(IrType::Packed { .. })
            )
        {
            return None;
        }
        let actual_bits = self.alias_expression_bits(port, actual).ok()?;
        let formal_bits = self.alias_expression_bits(port, *formal).ok()?;
        if actual_bits.is_empty() || actual_bits.len() != formal_bits.len() {
            return None;
        }
        let mut nets = vec![*formal];
        let mut owners = HashSet::new();
        for bit in &actual_bits {
            match *bit {
                AliasBit::Net { net, .. } => {
                    if net == *formal {
                        return None;
                    }
                    if owners.insert(net) {
                        if !self.mergeable_port_net(net, context.true_aliases) {
                            return None;
                        }
                        nets.push(net);
                    }
                }
                AliasBit::Array { owner, element, .. } => {
                    // A cell the inventory left unconnected (see
                    // `input_merge_cells`) is a constant run outside the
                    // bit-level graph.
                    if !self.mergeable_port_array(owner)
                        || !self
                            .array_globals
                            .get(&owner)
                            .is_some_and(|info| context.cells.connected(info.ir, element))
                    {
                        return None;
                    }
                    owners.insert(owner);
                }
            }
        }
        if owners
            .iter()
            .filter_map(|owner| self.owning_inst(*owner))
            .any(|instance| specify.contains(&instance))
        {
            return None;
        }
        Some(MergeConnection::Selected {
            output,
            bits: actual_bits.into_iter().zip(formal_bits).collect(),
            nets,
        })
    }

    /// The net-array bits an input port reads that the net-cell inventory
    /// connects so a merge can reach them: bits of a selected actual whose
    /// array has a type that dominates a `wire` formal (`tri0`, `tri1`,
    /// `supply0`, wired types), which always merge. A `wire`/`tri`/`logic`
    /// cell nothing else connects is a constant run; its port keeps the copy.
    pub(in super::super) fn input_merge_cells(&self, port: NodeId) -> Vec<AliasBit> {
        let NodeKind::Port {
            direction: DbDirection::Input,
            high,
            low: Some(_),
            high_expr,
            ..
        } = self.kind(port)
        else {
            return Vec::new();
        };
        let Some(actual) = high_expr.or(*high) else {
            return Vec::new();
        };
        if !self.merge_ports
            || self.sdf.is_some()
            || self.whole_port_connection(port).is_some()
            || !self.plain_net_selection(actual)
        {
            return Vec::new();
        }
        let mut bits = self.alias_expression_bits(port, actual).unwrap_or_default();
        bits.retain(|bit| match bit {
            AliasBit::Array { owner, .. } => self
                .db
                .array_meta(*owner)
                .and_then(|meta| meta.net_type())
                .is_some_and(|kind| {
                    Self::mergeable_net_type(kind)
                        && !matches!(kind, NetType::Wire | NetType::Tri | NetType::Logic)
                }),
            AliasBit::Net { .. } => false,
        });
        bits
    }

    /// Whether `expression` names only nets and net-array cells through
    /// selects and concatenations, without a conversion: the forms the
    /// bit-level union pairs bit by bit. Whether every select is constant
    /// is checked when the bits are resolved.
    fn plain_net_selection(&self, expression: NodeId) -> bool {
        match self.kind(expression) {
            NodeKind::Net { .. }
            | NodeKind::Array { .. }
            | NodeKind::Expr(ExprKind::Ref { .. } | ExprKind::HierPath { .. }) => true,
            NodeKind::Expr(
                ExprKind::BitSelect { base, .. }
                | ExprKind::PartSelect { base, .. }
                | ExprKind::IndexedPartSelect { base, .. }
                | ExprKind::ArraySelect { base, .. },
            ) => self.plain_net_selection(*base),
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Concat,
                operands,
                ..
            }) => operands
                .iter()
                .all(|operand| self.plain_net_selection(*operand)),
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::MultiConcat,
                operands,
                ..
            }) => operands
                .iter()
                .skip(1)
                .all(|operand| self.plain_net_selection(*operand)),
            _ => false,
        }
    }

    /// A plain net of a type the merged group supports, outside any net
    /// alias, without a net delay.
    fn mergeable_port_net(&self, net: NodeId, true_aliases: &HashSet<NodeId>) -> bool {
        matches!(
            self.kind(net),
            NodeKind::Net { net_type, .. } if Self::mergeable_net_type(*net_type)
        ) && !true_aliases.contains(&net)
            && self.db.net_delay(net).is_none()
    }

    /// A net array of a type the merged group supports, without a net delay.
    fn mergeable_port_array(&self, owner: NodeId) -> bool {
        self.db
            .array_meta(owner)
            .and_then(|meta| meta.net_type())
            .is_some_and(Self::mergeable_net_type)
            && self.db.net_delay(owner).is_none()
    }

    fn mergeable_net_type(net_type: NetType) -> bool {
        matches!(
            net_type,
            NetType::Wire
                | NetType::Tri
                | NetType::Logic
                | NetType::Wand
                | NetType::TriAnd
                | NetType::Wor
                | NetType::TriOr
                | NetType::Tri0
                | NetType::Tri1
                | NetType::Supply0
                | NetType::Supply1
        )
    }

    /// Whether the merged net would differ from the sink's source: the sink
    /// has a type other than `wire`/`tri`/`logic` or a driver besides the
    /// port. A driver scan that fails keeps the link, so the group that the
    /// sink gets anyway reports the error.
    fn sink_differs_from_source(
        &self,
        sink: NodeId,
        port: NodeId,
        context: &MergeContext<'_>,
    ) -> bool {
        if !matches!(
            self.kind(sink),
            NodeKind::Net {
                net_type: NetType::Wire | NetType::Tri | NetType::Logic,
                ..
            }
        ) {
            return true;
        }
        self.structural_site_sources(context.nodes, context.writers, &[sink])
            .is_ok_and(|sources| sources.iter().any(|(source, _)| *source != port))
    }
}
