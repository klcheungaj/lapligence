//! Input and output net ports merged like inout connections (decision A21-D4).
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
//! (`port_sinks.rs`, A21-D2) or its link. A port also keeps its link when a
//! merge would change more than the nets' resolution: either side is not a
//! net of a type the merge supports (a variable, a `uwire` or `trireg`
//! net), the sides differ in width or packed type (the link converts),
//! either side has a net delay or takes part in a net alias, the port has a
//! drive strength, a member write the resolved group rejects reaches either
//! side, the connection is not to a whole net, the design reads SDF, or
//! either instance has module paths or timing checks. A model that keeps
//! the VPI catalog merges too: both names stay nets of the one group, so
//! `vpi_handle_by_name` of either shows the merged value and drivers, and a
//! put or force through either name reaches both, as for an inout port.

use super::*;

/// One input or output port that may merge: its actual and formal nets and
/// the net it drives.
struct MergeCandidate {
    port: NodeId,
    actual: NodeId,
    formal: NodeId,
    sink: NodeId,
}

impl Codegen<'_> {
    /// The input and output ports whose nets merge into one inout-style
    /// group, in node order. `parent` holds the whole-net inout unions and
    /// `alias_nets` the nets connected bit by bit, both before any merge.
    pub(super) fn merged_net_ports(
        &self,
        nodes: &[NodeId],
        writers: &NetWriterIndex,
        parent: &HashMap<NodeId, NodeId>,
        alias_nets: &HashSet<NodeId>,
    ) -> Vec<NodeId> {
        if !self.merge_ports || self.sdf.is_some() {
            return Vec::new();
        }
        let specify = self.specify_instances();
        let mut candidates = Vec::new();
        for port in nodes {
            if let Some(candidate) =
                self.merge_candidate(*port, nodes, writers, alias_nets, &specify)
            {
                candidates.push(candidate);
            }
        }
        if candidates.is_empty() {
            return Vec::new();
        }
        // A candidate merges when its sink would differ from its source; a
        // merge then puts both of its nets into one group, so a candidate
        // whose sink is one of them merges too (the sink now has the other
        // side's drivers). The worklist visits every candidate once.
        let mut by_sink: HashMap<NodeId, Vec<usize>> = HashMap::new();
        for (index, candidate) in candidates.iter().enumerate() {
            by_sink.entry(candidate.sink).or_default().push(index);
        }
        let mut merged = vec![false; candidates.len()];
        let mut work = Vec::new();
        for (index, candidate) in candidates.iter().enumerate() {
            if parent.contains_key(&candidate.sink)
                || self.sink_differs_from_source(candidate, nodes, writers)
            {
                merged[index] = true;
                work.push(index);
            }
        }
        while let Some(index) = work.pop() {
            let candidate = &candidates[index];
            for net in [candidate.actual, candidate.formal] {
                for next in by_sink.get(&net).into_iter().flatten() {
                    if !merged[*next] {
                        merged[*next] = true;
                        work.push(*next);
                    }
                }
            }
        }
        candidates
            .iter()
            .zip(merged)
            .filter_map(|(candidate, merged)| merged.then_some(candidate.port))
            .collect()
    }

    /// A whole input or output connection between two nets that a merge
    /// keeps exact (module docs), or `None`.
    fn merge_candidate(
        &self,
        port: NodeId,
        nodes: &[NodeId],
        writers: &NetWriterIndex,
        alias_nets: &HashSet<NodeId>,
        specify: &HashSet<NodeId>,
    ) -> Option<MergeCandidate> {
        let (direction, actual, formal) = self.whole_port_connection(port)?;
        let sink = match direction {
            DbDirection::Input => formal,
            DbDirection::Output => actual,
            _ => return None,
        };
        let NodeKind::Port {
            strength0,
            strength1,
            low,
            ..
        } = self.kind(port)
        else {
            return None;
        };
        if actual == formal
            || [port, actual]
                .iter()
                .filter_map(|node| self.owning_inst(*node))
                .any(|instance| specify.contains(&instance))
            || !self.mergeable_port_net(actual, alias_nets)
            || !self.mergeable_port_net(formal, alias_nets)
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
        if self
            .unsupported_member_write(nodes, writers, &[actual, formal])
            .is_some()
        {
            return None;
        }
        Some(MergeCandidate {
            port,
            actual,
            formal,
            sink,
        })
    }

    /// A plain net of a type the merged group supports, outside any net
    /// alias, without a net delay.
    fn mergeable_port_net(&self, net: NodeId, alias_nets: &HashSet<NodeId>) -> bool {
        matches!(
            self.kind(net),
            NodeKind::Net {
                net_type: NetType::Wire
                    | NetType::Tri
                    | NetType::Logic
                    | NetType::Wand
                    | NetType::TriAnd
                    | NetType::Wor
                    | NetType::TriOr
                    | NetType::Tri0
                    | NetType::Tri1
                    | NetType::Supply0
                    | NetType::Supply1,
                ..
            }
        ) && !alias_nets.contains(&net)
            && self.db.net_delay(net).is_none()
    }

    /// Whether the merged net would differ from the sink's source: the sink
    /// has a type other than `wire`/`tri`/`logic` or a driver besides the
    /// port. A driver scan that fails keeps the link, so the group that the
    /// sink gets anyway reports the error.
    fn sink_differs_from_source(
        &self,
        candidate: &MergeCandidate,
        nodes: &[NodeId],
        writers: &NetWriterIndex,
    ) -> bool {
        if !matches!(
            self.kind(candidate.sink),
            NodeKind::Net {
                net_type: NetType::Wire | NetType::Tri | NetType::Logic,
                ..
            }
        ) {
            return true;
        }
        self.structural_site_sources(nodes, writers, &[candidate.sink])
            .is_ok_and(|sources| sources.iter().any(|(source, _)| *source != candidate.port))
    }
}
