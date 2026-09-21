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
        self.array_globals
            .iter()
            .find_map(|(owner, info)| (info.ir == array).then_some(*owner))
            .ok_or_else(|| "port-connected net array has no owned declaration".into())
    }

    fn insert_collapse_point(&self, plan: &mut NetCollapsePlan, point: NetPoint) -> Result<(), String> {
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
                        NetPoint::ArrayBit { owner, element, bit },
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
        for owner in bit_nets {
            let width = self
                .signal_of(*owner)
                .ok_or("port-collapse bit endpoint has no packed storage")?
                .width;
            let kind = self.collapse_net_type(*owner)?;
            for bit in 0..width {
                plan.insert(NetPoint::Bit(AliasBit::Net { net: *owner, bit }), kind)?;
            }
        }
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
        // Parent connections precede descendant connections. Ties retain the
        // owned design traversal order (ports/siblings in declaration order).
        // The LRM table defines pairs, not a unique winner for an arbitrary
        // multiway warning-only conflict; this order makes that choice stable.
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
            .collect::<Vec<_>>();
        ports.sort_by_key(|port| {
            let mut depth = 0usize;
            let mut owner = self.owning_inst(*port);
            while let Some(instance) = owner {
                depth += 1;
                owner = self.owning_inst(instance);
            }
            depth
        });
        let mut warnings = HashSet::new();
        for port in ports {
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
                // The bundled frontend rejects uwire inouts. Keep that bound
                // if an alternate owned database reaches lowering, without
                // rejecting a same-type uwire alias that has no port edges.
                if self.collapse_net_type(internal.owner())? == NetType::Uwire
                    || self.collapse_net_type(external.owner())? == NetType::Uwire
                {
                    return Err(format!(
                        "inout port `{}`: uwire endpoints remain unsupported at {}:{}:{}",
                        self.display_name(port),
                        self.node(port).file.as_deref().unwrap_or("<unknown>"),
                        self.node(port).line, self.node(port).col,
                    ));
                }
                self.insert_collapse_point(&mut plan, internal)?;
                self.insert_collapse_point(&mut plan, external)?;
                // Diagnose a source-level warning cell even when another path
                // already joined the endpoints or changed an effective type.
                let internal_kind = self.collapse_net_type(internal.owner())?;
                let external_kind = self.collapse_net_type(external.owner())?;
                let choice = port_choice(internal_kind, external_kind)
                    .ok_or("inout endpoint has no net-type table entry")?;
                if choice.warns() {
                    let warning = CollapseWarning {
                        internal: internal_kind,
                        external: external_kind,
                        selected: if choice.internal() { internal_kind } else { external_kind },
                    };
                    self.warn_port_collapse(port, warning, &mut warnings);
                }
                if let Some(warning) = plan.port(internal, external)? {
                    self.warn_port_collapse(port, warning, &mut warnings);
                }
            }
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
