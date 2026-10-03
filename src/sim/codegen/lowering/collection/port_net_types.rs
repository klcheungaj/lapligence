//! Owned-declaration type/delay selection for whole and selected inout networks.

use super::net_collapse::{
    canonical_net_type, port_choice, CollapseWarning, NetCollapsePlan, NetPoint,
};
use super::*;

impl Codegen<'_> {
    fn collapse_net_type(&self, owner: NodeId) -> Result<NetType, String> {
        let kind = match self.kind(owner) {
            NodeKind::Net { net_type, .. } => Some(*net_type),
            NodeKind::Array { .. } => self.db.array_meta(owner).and_then(|meta| meta.net_type()),
            _ => None,
        };
        kind.and_then(canonical_net_type)
            .filter(|kind| Self::ir_net_kind(*kind).is_some())
            .ok_or_else(|| {
                format!(
                    "inout endpoint `{}` is not a supported net at {}:{}:{}",
                    self.display_name(owner),
                    self.node(owner).file.as_deref().unwrap_or("<unknown>"),
                    self.node(owner).line,
                    self.node(owner).col,
                )
            })
    }

    fn array_net_owner(&self, array: usize) -> Result<NodeId, String> {
        sorted_node_ids(&self.array_globals)
            .into_iter()
            .find(|owner| self.array_globals[owner].ir == array)
            .ok_or_else(|| "port-connected net array has no owned declaration".into())
    }

    fn insert_collapse_point(
        &self,
        plan: &mut NetCollapsePlan,
        point: NetPoint,
    ) -> Result<(), String> {
        plan.insert(point, self.collapse_net_type(point.owner())?)
    }

    fn inout_type_points(
        &self,
        port: NodeId,
        high: Option<NodeId>,
        low: NodeId,
        actual: Option<NodeId>,
        bit_nets: &HashSet<NodeId>,
    ) -> Result<Vec<(NetPoint, NetPoint)>, String> {
        if let Some(pairs) = actual
            .map(|actual| self.net_array_inout_pairs(port, actual, low))
            .transpose()?
            .flatten()
        {
            return Ok(pairs
                .into_iter()
                .flat_map(|((formal, formal_cell, width), (actual, actual_cell))| {
                    (0..width).map(move |bit| {
                        (
                            NetPoint::ArrayBit {
                                owner: formal,
                                element: formal_cell,
                                bit,
                            },
                            NetPoint::ArrayBit {
                                owner: actual,
                                element: actual_cell,
                                bit,
                            },
                        )
                    })
                })
                .collect());
        }
        if let Some(((array, element), actual_bits)) = actual
            .map(|actual| self.array_net_selection(actual))
            .transpose()?
            .flatten()
        {
            let owner = self.array_net_owner(array)?;
            let formal = self.alias_expression_bits(port, low)?;
            if formal.len() != actual_bits.len() {
                return Err("array inout type plan has different endpoint widths".into());
            }
            return Ok(formal
                .into_iter()
                .zip(actual_bits)
                .map(|(internal, bit)| {
                    (
                        NetPoint::Bit(internal),
                        NetPoint::ArrayBit {
                            owner,
                            element,
                            bit,
                        },
                    )
                })
                .collect());
        }
        if high.is_none()
            || high.is_some_and(|high| bit_nets.contains(&high))
            || bit_nets.contains(&low)
        {
            let formal = self.alias_expression_bits(port, low)?;
            let actual = actual
                .or(high)
                .ok_or("inout type plan has no actual expression")?;
            let actual = self.alias_expression_bits(port, actual)?;
            if formal.len() != actual.len() {
                return Err("selected inout type plan has different endpoint widths".into());
            }
            return Ok(formal
                .into_iter()
                .zip(actual)
                .map(|(internal, external)| (NetPoint::Bit(internal), NetPoint::Bit(external)))
                .collect());
        }
        let high = high.ok_or("whole inout type plan has no external declaration")?;
        Ok(vec![(NetPoint::Whole(low), NetPoint::Whole(high))])
    }

    /// Storage unions deliberately have no notion of internal/external. Replay
    /// the owned connections separately so union rank and hash iteration cannot
    /// choose a resolver or make a dominated declaration's delay survive.
    pub(super) fn build_port_net_type_plan(
        &mut self,
        nodes: &[NodeId],
        bit_nets: &HashSet<NodeId>,
        alias_bits: &HashSet<AliasBit>,
    ) -> Result<NetCollapsePlan, String> {
        let mut plan = NetCollapsePlan::default();
        // Include unconnected bits of a partially collapsed vector as singleton
        // declarations. They must keep their own type/default and delay.
        for owner in sorted_node_set(bit_nets) {
            let width = self
                .signal_of(owner)
                .ok_or("port-collapse bit endpoint has no packed storage")?
                .width;
            let kind = self.collapse_net_type(owner)?;
            for bit in 0..width {
                plan.insert(NetPoint::Bit(AliasBit::Net { net: owner, bit }), kind)?;
            }
        }
        let mut alias_bits = alias_bits.iter().copied().collect::<Vec<_>>();
        alias_bits.sort_by_key(|bit| bit.sort_key());
        for bit in alias_bits {
            plan.insert(bit.point(), self.collapse_net_type(bit.owner())?)?;
        }
        // Alias legality is checked against declarations, before any port can
        // change an effective type. Aliases have no external-side dominance.
        for node in nodes {
            let NodeKind::NetAlias { nets } = self.kind(*node) else {
                continue;
            };
            let mut expressions = nets
                .iter()
                .map(|expression| self.alias_expression_bits(*node, *expression));
            let first = expressions
                .next()
                .transpose()?
                .ok_or("net alias has no expressions")?;
            for other in expressions {
                let other = other?;
                if other.len() != first.len() {
                    return Err(self.alias_error(*node, "contains different widths"));
                }
                for (first, other) in first.iter().zip(other) {
                    plan.alias(first.point(), other.point())
                        .map_err(|error| self.alias_error(*node, &error))?;
                }
            }
        }
        // Parent connections precede descendant connections: each depth is
        // one batch whose result is independent of instance and port order
        // (`NetCollapsePlan::port_batch`).
        let mut ports = nodes
            .iter()
            .copied()
            .filter(|node| {
                matches!(
                    self.kind(*node),
                    NodeKind::Port {
                        direction: DbDirection::Inout,
                        high,
                        high_expr,
                        low: Some(_),
                        ..
                    } if high.is_some() || high_expr.is_some()
                )
            })
            .map(|port| {
                let mut depth = 0usize;
                let mut owner = self.owning_inst(port);
                while let Some(instance) = owner {
                    depth += 1;
                    owner = self.owning_inst(instance);
                }
                (depth, port)
            })
            .collect::<Vec<_>>();
        ports.sort_by_key(|(depth, _)| *depth);
        let mut warnings = HashSet::new();
        let mut level_start = 0;
        while level_start < ports.len() {
            let depth = ports[level_start].0;
            let level_end = ports[level_start..]
                .iter()
                .position(|(other, _)| *other != depth)
                .map_or(ports.len(), |offset| level_start + offset);
            let mut edges = Vec::new();
            let mut edge_ports = Vec::new();
            for &(_, port) in &ports[level_start..level_end] {
                let NodeKind::Port {
                    high,
                    low: Some(low),
                    high_expr,
                    ..
                } = self.kind(port)
                else {
                    return Err("inout type plan lost its port endpoints".into());
                };
                let points = self.inout_type_points(port, *high, *low, *high_expr, bit_nets)?;
                for (internal, external) in points {
                    self.insert_collapse_point(&mut plan, internal)?;
                    self.insert_collapse_point(&mut plan, external)?;
                    // Diagnose a source-level warning cell even when another
                    // path already joined the endpoints or changed a type.
                    let internal_kind = self.collapse_net_type(internal.owner())?;
                    let external_kind = self.collapse_net_type(external.owner())?;
                    let choice = port_choice(internal_kind, external_kind)
                        .ok_or("inout endpoint has no net-type table entry")?;
                    if choice.warns() {
                        let warning = CollapseWarning {
                            internal: internal_kind,
                            external: external_kind,
                            selected: if choice.internal() {
                                internal_kind
                            } else {
                                external_kind
                            },
                        };
                        self.warn_port_collapse(port, warning, &mut warnings);
                    }
                    edges.push((internal, external));
                    edge_ports.push(port);
                }
            }
            let (edge_warnings, ties) = plan.port_batch(&edges)?;
            for (edge, warning) in edge_warnings {
                self.warn_port_collapse(edge_ports[edge], warning, &mut warnings);
            }
            let mut tied = HashSet::new();
            for tie in ties {
                let port = edge_ports[tie.edge];
                if tied.insert((port, tie.candidates.clone())) {
                    self.warnings.push(format!(
                        "dissimilar inout port `{}`: same-depth connections choose {:?} among non-dominating {:?} by column order (IEEE 1800-2009 Table 23-1) at {}:{}:{}",
                        self.display_name(port), tie.selected, tie.candidates,
                        self.node(port).file.as_deref().unwrap_or("<unknown>"),
                        self.node(port).line, self.node(port).col,
                    ));
                }
            }
            level_start = level_end;
        }
        Ok(plan)
    }

    fn warn_port_collapse(
        &mut self,
        port: NodeId,
        warning: CollapseWarning,
        seen: &mut HashSet<(NodeId, CollapseWarning)>,
    ) {
        if seen.insert((port, warning)) {
            self.warnings.push(format!(
                "dissimilar inout port `{}`: internal {:?}, external {:?}; table choice {:?} (IEEE 1800-2009 Table 23-1) at {}:{}:{}",
                self.display_name(port), warning.internal, warning.external, warning.selected,
                self.node(port).file.as_deref().unwrap_or("<unknown>"),
                self.node(port).line, self.node(port).col,
            ));
        }
    }
}
