//! Coalesce canonical electrical bits only while connectivity stays affine.
use super::net_collapse::NetCollapsePlan;
use super::*;

type ElectricalRun = Vec<Vec<AliasBit>>;
type Connections = HashMap<AliasBit, Vec<(NodeId, usize, u32)>>;

impl Codegen<'_> {
    pub(super) fn partition_target_bits(
        &self,
        source: NodeId,
        target: NodeId,
    ) -> Option<Vec<AliasBit>> {
        if let Some((array, _)) = self.array_net_target_parts(target) {
            if array.is_net && self.array_net_endpoint(target).is_none() {
                let owner = self
                    .array_globals
                    .iter()
                    .find_map(|(owner, info)| (info.ir == array.ir).then_some(*owner))?;
                return Some(
                    self.array_net_target_elements(target)
                        .ok()??
                        .into_iter()
                        .flat_map(|element| {
                            (0..array.elem_width).rev().map(move |bit| AliasBit::Array {
                                owner,
                                element,
                                bit,
                            })
                        })
                        .collect(),
                );
            }
        }
        self.alias_expression_bits(source, target).ok()
    }

    /// A run has the same member views, source sites and force targets at every
    /// position. Every member and source offset advances by one; permutations,
    /// repeated aliases and unrepresented projections stay at bit granularity.
    pub(super) fn partition_electrical_bits(
        &self,
        columns: Vec<Vec<AliasBit>>,
        nodes: &[NodeId],
        types: &NetCollapsePlan,
    ) -> Result<Vec<ElectricalRun>, String> {
        let mut connections = Connections::new();
        let mut forced: HashMap<AliasBit, Vec<NodeId>> = HashMap::new();
        let mut opaque = HashSet::new();
        for source in nodes {
            let targets = match self.kind(*source) {
                NodeKind::ContAssign { .. } => self
                    .node(*source)
                    .children
                    .first()
                    .copied()
                    .into_iter()
                    .collect::<Vec<_>>(),
                NodeKind::Port {
                    direction,
                    high,
                    high_expr,
                    low,
                    ..
                } => match direction {
                    DbDirection::Input => low.iter().copied().collect(),
                    DbDirection::Output => high_expr.or(*high).into_iter().collect(),
                    _ => Vec::new(),
                },
                NodeKind::Gate { terms, .. } => terms
                    .iter()
                    .filter(|term| {
                        matches!(term.direction, DbDirection::Output | DbDirection::Inout)
                    })
                    .map(|term| term.expr)
                    .collect(),
                NodeKind::Stmt(StmtKind::Force { lhs, .. } | StmtKind::Release { lhs }) => {
                    if let Some(bits) = self.partition_target_bits(*source, *lhs) {
                        for bit in bits {
                            forced.entry(bit).or_default().push(*source);
                        }
                    } else {
                        self.partition_opaque_owners(*lhs, &mut opaque);
                    }
                    Vec::new()
                }
                _ => Vec::new(),
            };
            for (terminal, target) in targets.into_iter().enumerate() {
                if let Some(bits) = self.partition_target_bits(*source, target) {
                    let width = bits.len() as u32;
                    for (position, bit) in bits.into_iter().enumerate() {
                        connections.entry(bit).or_default().push((
                            *source,
                            terminal,
                            width - 1 - position as u32,
                        ));
                    }
                } else {
                    self.partition_opaque_owners(target, &mut opaque);
                }
            }
        }
        let adjacent = |left: &[AliasBit], right: &[AliasBit]| -> bool {
            if left.len() != right.len() {
                return false;
            }
            left.iter().zip(right).all(|(left, right)| {
                let l = left.sort_key();
                let r = right.sort_key();
                if (l.0, l.1, l.2) != (r.0, r.1, r.2)
                    || left.bit().checked_add(1) != Some(right.bit())
                    || opaque.contains(&left.owner())
                    || types.resolved(left.point()) != types.resolved(right.point())
                    || forced.get(left) != forced.get(right)
                {
                    return false;
                }
                let l = connections.get(left).map(Vec::as_slice).unwrap_or(&[]);
                let r = connections.get(right).map(Vec::as_slice).unwrap_or(&[]);
                l.len() == r.len()
                    && l.iter()
                        .zip(r)
                        .all(|(l, r)| (l.0, l.1) == (r.0, r.1) && l.2.checked_add(1) == Some(r.2))
            })
        };
        let mut runs: Vec<ElectricalRun> = Vec::new();
        for column in columns {
            let extend = runs
                .last()
                .and_then(|run| run.last())
                .is_some_and(|previous| adjacent(previous, &column));
            if extend {
                runs.last_mut().expect("previous run exists").push(column);
            } else {
                runs.push(vec![column]);
            }
        }
        Ok(runs)
    }

    pub(super) fn partition_opaque_owners(&self, target: NodeId, owners: &mut HashSet<NodeId>) {
        if let Some(signal) = self.signal_of(target) {
            if let Some(owner) = self.sig_global_for_ir(signal.ir) {
                owners.insert(owner);
            }
        }
        if let Some((array, _)) = self.array_net_target_parts(target) {
            owners.extend(
                self.array_globals
                    .iter()
                    .filter_map(|(owner, info)| (info.ir == array.ir).then_some(*owner)),
            );
        }
        for child in &self.node(target).children {
            self.partition_opaque_owners(*child, owners);
        }
    }
}
