//! Coalesce canonical electrical bits only while connectivity stays affine.
use super::net_collapse::NetCollapsePlan;
use super::*;

type ElectricalRun = Vec<Vec<AliasBit>>;
type Connections = HashMap<AliasBit, Vec<(NodeId, usize, u32)>>;
/// A member view: a net, or one cell of a net array.
type OwnerKey = (u32, u8, u64);
/// One structural source terminal.
type SourceKey = (NodeId, usize);

/// Bound on a shared run's driver slots times its width (see
/// [`Codegen::partition_electrical_bits`]). Every member and source of a
/// group owns one full-width contribution slot, and a write that spans the
/// group resolves every active slot, so a run that admits new members bit by
/// bit stops growing at this many slot bits (2^22 is about 1 MiB of slot
/// planes). A bit-level group splits beyond it, which is only slower.
pub(in crate::sim::codegen) const MAX_SHARED_RUN_SLOT_BITS: u64 = 1 << 22;

/// The owners and source terminals of one column, each at one offset.
struct ColumnKeys {
    owners: HashMap<OwnerKey, u32>,
    sources: HashMap<SourceKey, u32>,
    forced: Vec<NodeId>,
}

/// The owners and sources a shared run has seen, its last column, and its
/// slot count (distinct owners plus distinct source terminals).
struct SharedRun {
    owners: HashSet<OwnerKey>,
    sources: HashSet<SourceKey>,
    last: ColumnKeys,
    width: u64,
    shared: bool,
}

impl SharedRun {
    fn slots(&self) -> u64 {
        (self.owners.len() + self.sources.len()) as u64
    }

    fn push(&mut self, column: ColumnKeys) {
        self.owners.extend(column.owners.keys().copied());
        self.sources.extend(column.sources.keys().copied());
        self.last = column;
        self.width += 1;
    }
}

fn owner_key(bit: AliasBit) -> OwnerKey {
    let key = bit.sort_key();
    (key.0, key.1, key.2)
}

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
    ///
    /// A run is also shared by columns whose members differ when at least one
    /// member continues (its offset advances by one) and every other member
    /// or source starts or ends there: each owner and source
    /// terminal covers one contiguous, affine range of the run. A vector net
    /// split bit by bit (a selected port or inout per bit, a true alias of
    /// its bits) then stays one group: a wide driver of it writes one slot
    /// instead of one per bit, and each per-bit member or driver owns a
    /// narrow contribution of the shared group. Such a run stops growing at
    /// [`MAX_SHARED_RUN_SLOT_BITS`].
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
            // A merged port is no driver: its nets are one group.
            if self.port_aliases.contains(source) {
                continue;
            }
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
                    DbDirection::Input => {
                        self.port_child_target(*source, *low).into_iter().collect()
                    }
                    DbDirection::Output => high_expr.or(*high).into_iter().collect(),
                    _ => Vec::new(),
                },
                NodeKind::Gate { .. } => self.gate_driven_terms(*source),
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
        let column_keys = |column: &[AliasBit]| -> Option<ColumnKeys> {
            let mut keys = ColumnKeys {
                owners: HashMap::new(),
                sources: HashMap::new(),
                forced: Vec::new(),
            };
            for bit in column {
                if opaque.contains(&bit.owner())
                    || keys.owners.insert(owner_key(*bit), bit.bit()).is_some()
                {
                    return None;
                }
                for (source, terminal, position) in connections.get(bit).into_iter().flatten() {
                    if keys
                        .sources
                        .insert((*source, *terminal), *position)
                        .is_some_and(|previous| previous != *position)
                    {
                        return None;
                    }
                }
                keys.forced
                    .extend(forced.get(bit).into_iter().flatten().copied());
            }
            keys.forced.sort_unstable_by_key(|node| node.index());
            keys.forced.dedup();
            Some(keys)
        };
        // Whether `next` may join a shared run ending in `run.last`.
        let shares =
            |run: &&mut SharedRun, previous: &[AliasBit], next: &[AliasBit], keys: &ColumnKeys| {
                if types.resolved(previous[0].point()) != types.resolved(next[0].point())
                    || run.last.forced != keys.forced
                {
                    return false;
                }
                let mut continuing = 0;
                for (owner, bit) in &keys.owners {
                    match run.last.owners.get(owner) {
                        Some(last) if last.checked_add(1) == Some(*bit) => continuing += 1,
                        Some(_) => return false,
                        None if run.owners.contains(owner) => return false,
                        None => {}
                    }
                }
                // Some member continues, and some member starts or ends here: a
                // boundary of drivers alone still splits the run.
                if continuing == 0
                    || (continuing == keys.owners.len() && continuing == run.last.owners.len())
                {
                    return false;
                }
                for (source, position) in &keys.sources {
                    match run.last.sources.get(source) {
                        Some(last) if last.checked_add(1) == Some(*position) => {}
                        Some(_) => return false,
                        None if run.sources.contains(source) => return false,
                        None => {}
                    }
                }
                let added = keys
                    .owners
                    .keys()
                    .filter(|owner| !run.owners.contains(owner))
                    .count()
                    + keys
                        .sources
                        .keys()
                        .filter(|source| !run.sources.contains(source))
                        .count();
                (run.slots() + added as u64).saturating_mul(run.width + 1)
                    <= self.shared_run_slot_bits
            };
        let mut runs: Vec<ElectricalRun> = Vec::new();
        // Keys of the last run, built when it first tries to share; `None`
        // inside once a column of it cannot share.
        let mut state: Option<Option<SharedRun>> = None;
        for column in columns {
            let Some(run) = runs.last_mut() else {
                runs.push(vec![column]);
                continue;
            };
            let previous = run.last().expect("runs are never empty");
            if adjacent(previous, &column) {
                match &mut state {
                    None | Some(None) => {
                        run.push(column);
                        continue;
                    }
                    Some(Some(shared)) => {
                        // A run that already shares keeps its slot bound.
                        let within = !shared.shared
                            || shared.slots().saturating_mul(shared.width + 1)
                                <= self.shared_run_slot_bits;
                        match column_keys(&column) {
                            Some(keys) if within => {
                                shared.push(keys);
                                run.push(column);
                                continue;
                            }
                            None if !shared.shared => {
                                state = Some(None);
                                run.push(column);
                                continue;
                            }
                            _ => {}
                        }
                    }
                }
            } else if let Some(keys) = column_keys(&column) {
                let built = state.get_or_insert_with(|| {
                    let mut built: Option<SharedRun> = None;
                    for column in run.iter() {
                        let keys = column_keys(column)?;
                        match &mut built {
                            None => {
                                built = Some(SharedRun {
                                    owners: keys.owners.keys().copied().collect(),
                                    sources: keys.sources.keys().copied().collect(),
                                    last: keys,
                                    width: 1,
                                    shared: false,
                                })
                            }
                            Some(built) => built.push(keys),
                        }
                    }
                    built
                });
                if let Some(shared) = built
                    .as_mut()
                    .filter(|shared| shares(shared, previous, &column, &keys))
                {
                    shared.push(keys);
                    shared.shared = true;
                    run.push(column);
                    continue;
                }
            }
            state = None;
            runs.push(vec![column]);
        }
        Ok(runs)
    }

    /// The sort key that orders a canonical column among the others: that of
    /// its bit on the widest member view (the lowest such bit on a tie), so
    /// the columns of a vector split bit by bit follow its bits and can
    /// share a run.
    pub(super) fn electrical_column_key(
        &self,
        column: &[AliasBit],
        widths: &mut HashMap<OwnerKey, u32>,
    ) -> (u32, u8, u64, u32) {
        let mut best: Option<(u32, (u32, u8, u64, u32))> = None;
        for bit in column {
            let width = *widths.entry(owner_key(*bit)).or_insert_with(|| match bit {
                AliasBit::Net { net, .. } => self.signal_of(*net).map_or(1, |signal| signal.width),
                AliasBit::Array { owner, .. } => self
                    .array_globals
                    .get(owner)
                    .map_or(1, |info| info.elem_width),
            });
            let key = bit.sort_key();
            if best.is_none_or(|(best_width, best_key)| {
                width > best_width || (width == best_width && key < best_key)
            }) {
                best = Some((width, key));
            }
        }
        best.map_or((u32::MAX, u8::MAX, u64::MAX, u32::MAX), |(_, key)| key)
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
