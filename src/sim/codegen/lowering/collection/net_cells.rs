//! Cell-interval classification of net arrays.
//!
//! The bit-level alias graph, type plan and partitioner cost memory and time
//! per bit, and every published cell costs an electrical group, a declared
//! view and generated C. Only *connected* cells need that: cells that a
//! structural driver, a true alias, a selected inout connection or a
//! force/release target reaches, plus every whole-array inout peer of such a
//! cell. Every other cell, with all of its whole-array inout peers, is an
//! undriven net: its value is the constant fill of its effective net type and
//! it never changes, so it becomes part of an `IrNetCellRun` instead.
//!
//! Effective types still follow the dissimilar-port collapse rules. Undriven
//! classes with the same shape (member arrays, relative cell offsets and
//! connecting ports) collapse identically, so only one representative class
//! per shape enters the type plan. That keeps the plan, its warnings and its
//! delay checks proportional to distinct connection shapes, not cells.
use super::net_collapse::{NetCollapsePlan, NetPoint};
use super::*;
use crate::sim::ir::{IrNetArray, IrNetCellRun};

#[cfg(test)]
mod tests;

struct NetCellArray {
    owner: NodeId,
    ir: usize,
    total: u64,
    /// First global cell index.
    base: usize,
}

/// One whole-array inout cell pairing: port, formal and actual global cells.
type CellEdge = (NodeId, usize, usize);

/// Shape of one undriven class relative to its first member: member arrays
/// with cell offsets, then edges as (port, formal member, actual member).
type ClassShape = (Vec<(usize, i128)>, Vec<(NodeId, usize, usize)>);

#[derive(Default)]
pub(super) struct NetCellPlan {
    arrays: Vec<NetCellArray>,
    by_ir: HashMap<usize, usize>,
    /// Per global cell: true when it takes the bit-level path.
    connected: Vec<bool>,
    /// Per global cell: undriven class, or `u32::MAX` for connected cells.
    class_of: Vec<u32>,
    /// Per undriven class: representative class of the same shape.
    representative: Vec<u32>,
    /// Per undriven class: its first member and member count.
    class_first: Vec<usize>,
    class_members: Vec<usize>,
    /// Global cells of representative classes; they enter the type plan.
    type_cells: HashSet<usize>,
}

impl NetCellPlan {
    fn global(&self, ir: usize, element: u64) -> Option<usize> {
        let array = &self.arrays[*self.by_ir.get(&ir)?];
        (element < array.total).then(|| array.base + element as usize)
    }

    fn locate(&self, global: usize) -> (&NetCellArray, u64) {
        let position = self
            .arrays
            .partition_point(|array| array.base <= global)
            .saturating_sub(1);
        let array = &self.arrays[position];
        (array, (global - array.base) as u64)
    }

    /// Whether `(ir, element)` takes the bit-level electrical path. Cells of
    /// arrays this plan does not own are always connected.
    pub(super) fn connected(&self, ir: usize, element: u64) -> bool {
        self.global(ir, element)
            .is_none_or(|global| self.connected[global])
    }

    /// Whether a cell's bits enter the port type plan: connected cells and
    /// the members of representative undriven classes.
    pub(super) fn in_type_plan(&self, ir: usize, element: u64) -> bool {
        self.global(ir, element)
            .is_none_or(|global| self.connected[global] || self.type_cells.contains(&global))
    }

    /// Representative undriven cells whose declarations are not otherwise
    /// type-plan points (no port edge reaches them).
    pub(super) fn representative_points(&self) -> Vec<(NodeId, u64)> {
        let mut cells = self.type_cells.iter().copied().collect::<Vec<_>>();
        cells.sort_unstable();
        cells
            .into_iter()
            .map(|global| {
                let (array, element) = self.locate(global);
                (array.owner, element)
            })
            .collect()
    }
}

fn find(parent: &mut [u32], mut cell: usize) -> usize {
    while parent[cell] as usize != cell {
        let next = parent[parent[cell] as usize];
        parent[cell] = next;
        cell = next as usize;
    }
    cell
}

impl Codegen<'_> {
    /// Classify every net-array cell before any bit-level structure exists.
    pub(super) fn plan_net_cells(&self, nodes: &[NodeId]) -> Result<NetCellPlan, String> {
        let mut plan = NetCellPlan::default();
        let mut cells = 0usize;
        for owner in sorted_node_ids(&self.array_globals) {
            let info = &self.array_globals[&owner];
            if plan.by_ir.contains_key(&info.ir)
                || self
                    .db
                    .array_meta(owner)
                    .and_then(|meta| meta.net_type())
                    .and_then(Self::ir_net_kind)
                    .is_none()
            {
                continue;
            }
            let total = self.model.arrays[info.ir].total;
            plan.by_ir.insert(info.ir, plan.arrays.len());
            plan.arrays.push(NetCellArray {
                owner,
                ir: info.ir,
                total,
                base: cells,
            });
            cells = usize::try_from(total)
                .ok()
                .and_then(|total| cells.checked_add(total))
                .filter(|cells| u32::try_from(*cells).is_ok())
                .ok_or("net-array cell count exceeds the electrical cell index range")?;
        }
        plan.connected = vec![false; cells];
        if cells == 0 {
            return Ok(plan);
        }
        let mut edges: Vec<CellEdge> = Vec::new();
        self.mark_connected_net_cells(nodes, &mut plan, &mut edges)?;

        // Whole-array inout peers share one electrical network per cell.
        let mut parent = Vec::new();
        if !edges.is_empty() {
            parent = (0..cells as u32).collect::<Vec<_>>();
            for &(_, formal, actual) in &edges {
                let (formal, actual) = (find(&mut parent, formal), find(&mut parent, actual));
                if formal != actual {
                    parent[formal.max(actual)] = formal.min(actual) as u32;
                }
            }
            let mut root_connected = vec![false; cells];
            for cell in 0..cells {
                if plan.connected[cell] {
                    root_connected[find(&mut parent, cell)] = true;
                }
            }
            for cell in 0..cells {
                plan.connected[cell] = root_connected[find(&mut parent, cell)];
            }
        }

        // Number undriven classes by their first member; list members in
        // ascending global order (compressed rows).
        plan.class_of = vec![u32::MAX; cells];
        let mut root_class: HashMap<usize, u32> = HashMap::new();
        for cell in 0..cells {
            if plan.connected[cell] {
                continue;
            }
            let class = if parent.is_empty() {
                plan.class_first.len() as u32
            } else {
                let next = plan.class_first.len() as u32;
                *root_class.entry(find(&mut parent, cell)).or_insert(next)
            };
            if class as usize == plan.class_first.len() {
                plan.class_first.push(cell);
                plan.class_members.push(0);
            }
            plan.class_members[class as usize] += 1;
            plan.class_of[cell] = class;
        }
        let classes = plan.class_first.len();
        let mut offsets = Vec::with_capacity(classes + 1);
        offsets.push(0usize);
        for count in &plan.class_members {
            offsets.push(offsets.last().copied().unwrap_or(0) + count);
        }
        let mut fill = offsets.clone();
        let mut members = vec![0usize; offsets[classes]];
        for cell in 0..cells {
            let class = plan.class_of[cell];
            if class != u32::MAX {
                members[fill[class as usize]] = cell;
                fill[class as usize] += 1;
            }
        }
        let mut class_edges = edges
            .iter()
            .filter(|(_, formal, _)| !plan.connected[*formal])
            .map(|edge| (plan.class_of[edge.1], *edge))
            .collect::<Vec<_>>();
        class_edges.sort_unstable_by_key(|(class, (port, formal, actual))| {
            (*class, port.index(), *formal, *actual)
        });

        let mut shapes: HashMap<ClassShape, u32> = HashMap::new();
        plan.representative = Vec::with_capacity(classes);
        let mut next_edge = 0usize;
        for class in 0..classes {
            let list = &members[offsets[class]..offsets[class + 1]];
            let (_, anchor) = plan.locate(list[0]);
            let anchor = i128::from(anchor);
            let shape_members = list
                .iter()
                .map(|cell| {
                    let (array, element) = plan.locate(*cell);
                    (plan.by_ir[&array.ir], i128::from(element) - anchor)
                })
                .collect::<Vec<_>>();
            let position = |cell: usize| list.binary_search(&cell).unwrap_or(usize::MAX);
            let mut shape_edges = Vec::new();
            while next_edge < class_edges.len() && class_edges[next_edge].0 as usize == class {
                let (_, (port, formal, actual)) = class_edges[next_edge];
                shape_edges.push((port, position(formal), position(actual)));
                next_edge += 1;
            }
            let representative = *shapes
                .entry((shape_members, shape_edges))
                .or_insert(class as u32);
            if representative == class as u32 {
                plan.type_cells.extend(list.iter().copied());
            }
            plan.representative.push(representative);
        }
        Ok(plan)
    }

    fn mark_connected_net_cells(
        &self,
        nodes: &[NodeId],
        plan: &mut NetCellPlan,
        edges: &mut Vec<CellEdge>,
    ) -> Result<(), String> {
        let mark_bits = |plan: &mut NetCellPlan, bits: &[AliasBit]| {
            for bit in bits {
                if let AliasBit::Array { owner, element, .. } = bit {
                    if let Some(global) = self
                        .array_globals
                        .get(owner)
                        .and_then(|info| plan.global(info.ir, *element))
                    {
                        plan.connected[global] = true;
                    }
                }
            }
        };
        let mark_cells = |plan: &mut NetCellPlan, cells: &[(usize, u64)]| {
            for (ir, element) in cells {
                if let Some(global) = plan.global(*ir, *element) {
                    plan.connected[global] = true;
                }
            }
        };
        // An unmappable target conservatively connects every cell of the
        // net arrays it names.
        let mark_opaque = |plan: &mut NetCellPlan, target: NodeId| {
            let mut owners = HashSet::new();
            self.partition_opaque_owners(target, &mut owners);
            for owner in owners {
                if let Some(&position) = self
                    .array_globals
                    .get(&owner)
                    .and_then(|info| plan.by_ir.get(&info.ir))
                {
                    let array = &plan.arrays[position];
                    let (base, total) = (array.base, array.total as usize);
                    plan.connected[base..base + total].fill(true);
                }
            }
        };
        for id in nodes {
            let targets = match self.kind(*id) {
                NodeKind::NetAlias { nets } => {
                    for expression in nets {
                        if let Ok(bits) = self.alias_expression_bits(*id, *expression) {
                            mark_bits(plan, &bits);
                        }
                    }
                    continue;
                }
                NodeKind::Port {
                    direction: DbDirection::Inout,
                    low: Some(low),
                    high_expr,
                    ..
                } => {
                    let Some(actual) = high_expr else {
                        continue;
                    };
                    if let Some(pairs) = self.net_array_inout_pairs(*id, *actual, *low)? {
                        for ((formal, formal_cell, _), (actual, actual_cell)) in pairs {
                            let formal = self
                                .array_globals
                                .get(&formal)
                                .and_then(|info| plan.global(info.ir, formal_cell));
                            let actual = self
                                .array_globals
                                .get(&actual)
                                .and_then(|info| plan.global(info.ir, actual_cell));
                            match (formal, actual) {
                                (Some(formal), Some(actual)) => edges.push((*id, formal, actual)),
                                (formal, actual) => {
                                    for cell in formal.into_iter().chain(actual) {
                                        plan.connected[cell] = true;
                                    }
                                }
                            }
                        }
                        continue;
                    }
                    if let Some((endpoint, _)) = self.array_net_selection(*actual)? {
                        mark_cells(plan, &[endpoint]);
                    } else if let Ok(bits) = self.alias_expression_bits(*id, *actual) {
                        mark_bits(plan, &bits);
                    }
                    if let Ok(bits) = self.alias_expression_bits(*id, *low) {
                        mark_bits(plan, &bits);
                    }
                    continue;
                }
                NodeKind::Stmt(StmtKind::Force { lhs, .. } | StmtKind::Release { lhs }) => {
                    vec![*lhs]
                }
                NodeKind::ContAssign { .. } | NodeKind::Gate { .. } | NodeKind::Port { .. } => {
                    self.structural_source_targets(*id)
                }
                _ => continue,
            };
            for target in targets {
                let mut cells = Vec::new();
                let patterned = self.continuous_net_array_cells(target, &mut cells).is_ok();
                mark_cells(plan, &cells);
                match self.partition_target_bits(*id, target) {
                    Some(bits) => mark_bits(plan, &bits),
                    None if !patterned || cells.is_empty() => mark_opaque(plan, target),
                    None => {}
                }
            }
        }
        Ok(())
    }

    /// Record every undriven net-array cell as a constant run of its
    /// effective net type, checking each class shape's collapse result once.
    pub(super) fn publish_undriven_net_cells(
        &mut self,
        plan: &NetCellPlan,
        type_plan: &NetCollapsePlan,
    ) -> Result<(), String> {
        let mut kinds: HashMap<u32, crate::sim::ir::IrNetKind> = HashMap::new();
        for class in 0..plan.representative.len() {
            let representative = plan.representative[class];
            if kinds.contains_key(&representative) {
                continue;
            }
            let first = plan.class_first[representative as usize];
            let (array, element) = plan.locate(first);
            let (owner, members) = (array.owner, plan.class_members[representative as usize]);
            let shown = format!("net-array cell {}[{element}]", self.display_name(owner));
            let resolved = type_plan
                .resolved(NetPoint::ArrayBit {
                    owner,
                    element,
                    bit: 0,
                })
                .ok_or("undriven net-array cell has no type-collapse plan")?
                .clone();
            let kind = Self::ir_net_kind(resolved.kind)
                .ok_or("undriven net-array cell has an unsupported effective net type")?;
            if members > LLG_MAX_NET_DRIVERS {
                return Err(format!(
                    "{shown}: {members} members exceed the runtime driver-count range at {}:{}:{}",
                    self.node(owner).file.as_deref().unwrap_or("<unknown>"),
                    self.node(owner).line,
                    self.node(owner).col,
                ));
            }
            // An undriven net never transitions; the delay is still checked
            // so conflicting collapsed declarations reject as before.
            self.net_propagation_delay_for_members(&resolved.delay_members, &shown)?;
            kinds.insert(representative, kind);
        }
        for array in &plan.arrays {
            let mut runs: Vec<IrNetCellRun> = Vec::new();
            for element in 0..array.total {
                let class = plan.class_of[array.base + element as usize];
                if class == u32::MAX {
                    continue;
                }
                let kind = kinds[&plan.representative[class as usize]];
                match runs.last_mut() {
                    Some(run) if run.kind == kind && run.first + run.count == element => {
                        run.count += 1;
                    }
                    _ => runs.push(IrNetCellRun {
                        first: element,
                        count: 1,
                        kind,
                    }),
                }
            }
            self.model.arrays[array.ir].net = Some(IrNetArray {
                constant_cells: runs,
            });
        }
        Ok(())
    }

    /// Give one undriven cell its own electrical group and declared view,
    /// for consumers that observe electrical state (a `%v` strength view).
    /// The cell stays undriven: one all-Z slot resolves exactly like the
    /// constant run it leaves.
    pub(super) fn materialize_undriven_net_cell(
        &mut self,
        array: usize,
        element: u64,
    ) -> Result<Option<usize>, String> {
        let Some(net) = self
            .model
            .arrays
            .get_mut(array)
            .and_then(|a| a.net.as_mut())
        else {
            return Ok(None);
        };
        let position = net
            .constant_cells
            .partition_point(|run| run.first + run.count <= element);
        let Some(run) = net
            .constant_cells
            .get(position)
            .copied()
            .filter(|run| run.first <= element)
        else {
            return Ok(None);
        };
        let mut replacement = Vec::with_capacity(2);
        if run.first < element {
            replacement.push(IrNetCellRun {
                count: element - run.first,
                ..run
            });
        }
        if element + 1 < run.first + run.count {
            replacement.push(IrNetCellRun {
                first: element + 1,
                count: run.first + run.count - element - 1,
                kind: run.kind,
            });
        }
        net.constant_cells
            .splice(position..position + 1, replacement);
        let storage = &self.model.arrays[array];
        let (width, signed) = (storage.elem_width, storage.signed);
        let group = self.model.net_groups.len();
        let net_group = crate::sim::ir::IrNetGroup::new(
            format!("g_array_net_{array}_{element}_0"),
            width,
            signed,
            run.kind,
            1,
        )
        .map_err(|error| error.to_string())?;
        self.model.net_groups.push(net_group);
        let index = self.model.signals.len();
        let global = format!("_llg_array_cell_{array}_{element}");
        let mut signal = IrSignal::new(
            global.clone(),
            None,
            IrType::Packed {
                width,
                signed,
                two_state: false,
            },
            None,
        )
        .map_err(|error| error.to_string())?;
        signal.net_alias = (0..width)
            .map(|bit| IrNetAliasBinding {
                group,
                slot: 0,
                signal_bit: bit,
                group_bit: bit,
            })
            .collect();
        self.model.signals.push(signal);
        self.signals.push(SignalInfo {
            global,
            width,
            signed,
            two_state: false,
            real: false,
            shortreal: false,
            net_driver: None,
            ir: index,
        });
        let elements = &mut self.model.arrays[array].net_elements;
        let at = elements.partition_point(|(cell, _)| *cell < element);
        elements.insert(at, (element, index));
        Ok(Some(index))
    }
}
