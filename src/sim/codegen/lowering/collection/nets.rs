//! Nets.

use super::*;

impl<'a> Codegen<'a> {
    // ── Collapsed inout-net groups ────────────────────────────────────────

    /// Collapse inout-port net pairs (parent high connection + child low
    /// connection) into one resolved simulated net per connected set
    /// (LRM §23.3.3.7), run after [`collect_design`](Self::collect_design)
    /// and before any emission.
    ///
    /// Every grouped member's `SignalInfo` is redirected to the shared
    /// `llg_net_t`'s `resolved` cell and tagged with its driver slot, so
    /// reads/writes/sensitivity all use the resolution cell automatically.
    /// Groups with anything the runtime cannot resolve (non-net members,
    /// mixed widths, unsupported net types, dynamic/NBA/task-actual writes)
    /// reject code generation rather than disconnecting the net group.
    fn net_propagation_delay_for_members(
        &mut self,
        members: &[NodeId],
        shown: &str,
    ) -> Result<Option<crate::sim::ir::IrTransitionDelay>, String> {
        let mut selected = None;
        for &member in members {
            let Some(delay) = self.db.net_delay(member) else {
                continue;
            };
            let previous_inst = self.inst;
            if let Some(instance) = self.owning_inst(member) {
                self.inst = instance;
            }
            let result = self.driver_delay_ticks(member, delay);
            self.inst = previous_inst;
            let converted = result.map_err(|error| {
                format!(
                    "net propagation delay for `{shown}` at {}:{}:{}: {error}",
                    self.node(member).file.as_deref().unwrap_or("<unknown>"),
                    self.node(member).line,
                    self.node(member).col,
                )
            })?;
            if let Some(existing) = selected {
                if existing != converted {
                    return Err(format!(
                        "net propagation delay for `{shown}` has conflicting member delays at {}:{}:{}",
                        self.node(member).file.as_deref().unwrap_or("<unknown>"),
                        self.node(member).line,
                        self.node(member).col,
                    ));
                }
            } else {
                selected = Some(converted);
            }
        }
        Ok(selected)
    }

    fn alias_error(&self, alias: NodeId, message: &str) -> String {
        format!(
            "net alias `{}` {message} at {}:{}:{}",
            self.display_name(alias),
            self.node(alias).file.as_deref().unwrap_or("<unknown>"),
            self.node(alias).line,
            self.node(alias).col,
        )
    }

    fn alias_base_net(&self, alias: NodeId, expression: NodeId) -> Result<NodeId, String> {
        match self.kind(expression) {
            NodeKind::Net { .. } => Ok(expression),
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => self.alias_base_net(alias, *target),
            NodeKind::Expr(ExprKind::HierPath { .. }) => self
                .hier_path_signal(expression)
                .and_then(|info| {
                    self.sig_globals.iter().find_map(|(target, candidate)| {
                        (candidate.ir == info.ir).then_some(*target)
                    })
                })
                .ok_or_else(|| self.alias_error(alias, "has an unresolved hierarchical net")),
            NodeKind::Expr(ExprKind::Cast { operand, .. }) => self.alias_base_net(alias, *operand),
            NodeKind::Expr(ExprKind::Ref { target: None }) => {
                Err(self.alias_error(alias, "has an unresolved net expression"))
            }
            _ => Err(self.alias_error(alias, "does not reference a plain net")),
        }
    }

    fn alias_bit(&self, alias: NodeId, net: NodeId, label: i128) -> Result<AliasBit, String> {
        let bit = self
            .packed_relative_bound(net, label)?
            .try_into()
            .map_err(|_| {
                self.alias_error(alias, "has a packed bit index outside the runtime range")
            })?;
        let width = match self.kind(net) {
            NodeKind::Net { ty, .. } => self
                .signal_of(net)
                .map_or(ty.width.unwrap_or(1), |signal| signal.width),
            _ => return Err(self.alias_error(alias, "does not reference a plain net")),
        };
        if bit >= width {
            return Err(self.alias_error(alias, "has a packed bit index outside its net"));
        }
        Ok(AliasBit { net, bit })
    }

    /// Flatten one legal alias lvalue to its logical MSB-to-LSB bit order.
    /// The order is the same order used by Slang when pairing alias ranges,
    /// so concatenated and selected expressions can share the same canonical
    /// bit union-find as whole-net aliases.
    fn alias_expression_bits(
        &self,
        alias: NodeId,
        expression: NodeId,
    ) -> Result<Vec<AliasBit>, String> {
        let aggregate_path = self.unpacked_path_for_expr(expression).or_else(|| {
            self.unpacked_aggregate_info(expression)
                .map(|(root, _)| (root, Vec::new()))
        });
        if let Some((net, path)) =
            aggregate_path.filter(|(root, _)| matches!(self.kind(*root), NodeKind::Net { .. }))
        {
            let descriptor = self
                .query_descriptor(net)
                .ok_or("aggregate net alias has no type")?;
            let (member, offset) = super::fixed_values::fixed_path_descriptor(descriptor, &path)
                .ok_or("aggregate net alias has no selected member")?;
            let width =
                Self::fixed_descriptor_width(&member).ok_or("aggregate net alias has no width")?;
            return Ok((0..width)
                .rev()
                .map(|bit| AliasBit {
                    net,
                    bit: offset + bit,
                })
                .collect());
        }
        match self.kind(expression) {
            NodeKind::Net { .. }
            | NodeKind::Expr(ExprKind::Ref { .. } | ExprKind::HierPath { .. }) => {
                let net = self.alias_base_net(alias, expression)?;
                let width = match self.kind(net) {
                    NodeKind::Net { ty, .. } => self
                        .signal_of(net)
                        .map_or(ty.width.unwrap_or(1), |signal| signal.width),
                    _ => unreachable!("alias base validated as a net"),
                };
                let (left, right) = self
                    .packed_range_for_base(net)
                    .map(|range| (range.left, range.right))
                    .unwrap_or((i128::from(width - 1), 0));
                let step = if left <= right { 1 } else { -1 };
                (0..width)
                    .map(|offset| {
                        let label = left + i128::from(offset) * step;
                        self.alias_bit(alias, net, label)
                    })
                    .collect()
            }
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                let net = self.alias_base_net(alias, *base)?;
                let label = self.eval_bound_i128(*index)?;
                Ok(vec![self.alias_bit(alias, net, label)?])
            }
            NodeKind::Expr(ExprKind::PartSelect { base, left, right }) => {
                let net = self.alias_base_net(alias, *base)?;
                let left = self.eval_bound_i128(*left)?;
                let right = self.eval_bound_i128(*right)?;
                let width = left
                    .checked_sub(right)
                    .or_else(|| right.checked_sub(left))
                    .and_then(|width| width.checked_add(1))
                    .ok_or_else(|| self.alias_error(alias, "has an overflowing part-select"))?;
                let width = u32::try_from(width).map_err(|_| {
                    self.alias_error(alias, "has a part-select wider than the runtime")
                })?;
                let step = if left <= right { 1 } else { -1 };
                (0..width)
                    .map(|offset| {
                        let label =
                            left.checked_add(i128::from(offset) * step).ok_or_else(|| {
                                self.alias_error(alias, "has an overflowing part-select")
                            })?;
                        self.alias_bit(alias, net, label)
                    })
                    .collect()
            }
            NodeKind::Expr(ExprKind::IndexedPartSelect {
                base,
                base_expr,
                width_expr,
                neg,
            }) => {
                let net = self.alias_base_net(alias, *base)?;
                let start = self.eval_bound_i128(*base_expr)?;
                let width = self.eval_bound_i128(*width_expr)?;
                let width = u32::try_from(width)
                    .ok()
                    .filter(|width| *width != 0)
                    .ok_or_else(|| {
                        self.alias_error(alias, "has an invalid indexed part-select width")
                    })?;
                let ascending = self.packed_range_ascending(net);
                let (start, step) = if ascending {
                    (start, if *neg { -1 } else { 1 })
                } else {
                    (
                        if *neg {
                            start
                        } else {
                            start
                                .checked_add(i128::from(width.saturating_sub(1)))
                                .ok_or_else(|| {
                                    self.alias_error(
                                        alias,
                                        "has an overflowing indexed part-select",
                                    )
                                })?
                        },
                        -1,
                    )
                };
                (0..width)
                    .map(|offset| {
                        let label =
                            start
                                .checked_add(i128::from(offset) * step)
                                .ok_or_else(|| {
                                    self.alias_error(
                                        alias,
                                        "has an overflowing indexed part-select",
                                    )
                                })?;
                        self.alias_bit(alias, net, label)
                    })
                    .collect()
            }
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Concat,
                reordered,
                operands,
                ..
            }) => {
                let mut result = Vec::new();
                let mut operands = operands.clone();
                if *reordered {
                    operands.reverse();
                }
                for operand in operands {
                    result.extend(self.alias_expression_bits(alias, operand)?);
                }
                Ok(result)
            }
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::MultiConcat,
                operands,
                ..
            }) => {
                let count = operands
                    .first()
                    .copied()
                    .ok_or_else(|| self.alias_error(alias, "has an empty replication"))?;
                let count = self.eval_bound_i128(count)?;
                let count = usize::try_from(count)
                    .map_err(|_| self.alias_error(alias, "has an invalid replication count"))?;
                let mut pattern = Vec::new();
                for operand in operands.iter().skip(1).copied() {
                    pattern.extend(self.alias_expression_bits(alias, operand)?);
                }
                if pattern.is_empty() {
                    return Err(self.alias_error(alias, "has an empty replication pattern"));
                }
                let total = pattern
                    .len()
                    .checked_mul(count)
                    .ok_or_else(|| self.alias_error(alias, "has an overflowing replication"))?;
                let mut result = Vec::with_capacity(total);
                for _ in 0..count {
                    result.extend(pattern.iter().copied());
                }
                Ok(result)
            }
            NodeKind::Expr(ExprKind::Cast { operand, .. }) => {
                self.alias_expression_bits(alias, *operand)
            }
            _ => Err(self.alias_error(alias, "contains an unsupported alias lvalue expression")),
        }
    }

    /// Determine whether an expression is rooted in a net that participates in
    /// a true alias. This lets ordinary assignments keep their existing
    /// lowering path while ensuring an unsupported alias lvalue is rejected
    /// instead of silently being treated as a raw signal write.
    fn expression_may_touch_alias(&self, expression: NodeId) -> bool {
        let signal_is_alias = |net: NodeId| {
            self.sig_globals
                .get(&net)
                .and_then(|info| self.model.signals.get(info.ir))
                .is_some_and(|signal| !signal.net_alias.is_empty())
        };
        match self.kind(expression) {
            NodeKind::Net { .. } => signal_is_alias(expression),
            NodeKind::Expr(ExprKind::Ref { target }) => {
                target.is_some_and(|target| self.expression_may_touch_alias(target))
            }
            NodeKind::Expr(ExprKind::HierPath { .. }) => {
                self.hier_path_signal(expression).is_some_and(|info| {
                    self.model
                        .signals
                        .get(info.ir)
                        .is_some_and(|signal| !signal.net_alias.is_empty())
                })
            }
            NodeKind::Expr(ExprKind::BitSelect { base, .. })
            | NodeKind::Expr(ExprKind::PartSelect { base, .. })
            | NodeKind::Expr(ExprKind::IndexedPartSelect { base, .. }) => {
                self.expression_may_touch_alias(*base)
            }
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Concat,
                operands,
                ..
            }) => operands
                .iter()
                .any(|operand| self.expression_may_touch_alias(*operand)),
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::MultiConcat,
                operands,
                ..
            }) => operands
                .iter()
                .skip(1)
                .any(|operand| self.expression_may_touch_alias(*operand)),
            NodeKind::Expr(ExprKind::Cast { operand, .. }) => {
                self.expression_may_touch_alias(*operand)
            }
            _ => false,
        }
    }

    /// Return the canonical group binding for every bit of a continuous
    /// assignment LHS when it targets a true-net alias.  A source may name a
    /// whole net, a constant select, or a concatenation of those forms.  The
    /// returned order is the assignment value order (MSB to LSB), matching
    /// [`alias_expression_bits`].
    pub(super) fn alias_lvalue_bindings(
        &self,
        source: NodeId,
        lhs: NodeId,
    ) -> Result<Option<Vec<(IrNetAliasBinding, u32)>>, String> {
        if let Some((array, element)) = self.array_net_endpoint(lhs) {
            if let Some((_, signal)) = self.model.arrays[array]
                .net_elements
                .iter()
                .find(|(index, _)| *index == element)
            {
                let signal = &self.model.signals[*signal];
                if !signal.net_alias.is_empty() {
                    let (_, bits) = self
                        .array_net_selection(lhs)?
                        .ok_or("net-array selection disappeared")?;
                    let mut result = Vec::new();
                    for (position, bit) in bits.into_iter().enumerate() {
                        let binding = signal
                            .net_alias
                            .iter()
                            .find(|binding| binding.signal_bit == bit)
                            .ok_or("net-array selected bit has no electrical binding")?;
                        result.push((
                            binding.clone(),
                            u32::try_from(position).map_err(|_| "net-array source bit overflow")?,
                        ));
                    }
                    return Ok(Some(result));
                }
            }
        }
        let bits = match self.alias_expression_bits(source, lhs) {
            Ok(bits) => bits,
            // Ordinary variable/net lvalues use the existing lowering path.
            // Only a successfully resolved alias-participating bit needs the
            // special source expansion below.
            Err(error) if self.expression_may_touch_alias(lhs) => return Err(error),
            Err(_) => return Ok(None),
        };
        let mut bindings = Vec::with_capacity(bits.len());
        let mut saw_alias = false;
        let mut saw_plain = false;
        for (rhs_bit, bit) in bits.into_iter().enumerate() {
            let Some(info) = self.sig_globals.get(&bit.net) else {
                saw_plain = true;
                continue;
            };
            let signal = &self.model.signals[info.ir];
            let mapped = signal
                .net_alias
                .iter()
                .filter(|binding| binding.signal_bit == bit.bit)
                .cloned()
                .collect::<Vec<_>>();
            if mapped.is_empty() {
                saw_plain = true;
            } else {
                saw_alias = true;
                bindings.extend(
                    mapped
                        .into_iter()
                        .map(|binding| (binding, u32::try_from(rhs_bit).unwrap_or(u32::MAX))),
                );
            }
        }
        if !saw_alias {
            return Ok(None);
        }
        if saw_plain {
            return Err(format!(
                "continuous assignment `{}` mixes aliased and ordinary net bits at {}:{}:{}",
                self.display_name(source),
                self.node(source).file.as_deref().unwrap_or("<unknown>"),
                self.node(source).line,
                self.node(source).col,
            ));
        }
        Ok(Some(bindings))
    }

    /// Build one source-specific contribution value for each canonical alias
    /// group touched by a continuous assignment.  Group values contain Z in
    /// untouched canonical bits, so one assignment site cannot accidentally
    /// drive bits selected by another site in the same alias network.
    pub(super) fn alias_driver_assignments<F: Fn(usize) -> usize>(
        &self,
        source: NodeId,
        bindings: &[(IrNetAliasBinding, u32)],
        rhs: &IrExpr,
        terminal_for_group: F,
    ) -> Result<Vec<(usize, IrExpr)>, String> {
        if rhs.is_real() {
            return Err(format!(
                "continuous assignment `{}` has a real RHS for a packed net alias at {}:{}:{}",
                self.display_name(source),
                self.node(source).file.as_deref().unwrap_or("<unknown>"),
                self.node(source).line,
                self.node(source).col,
            ));
        }
        if bindings.is_empty() || rhs.width() == 0 {
            return Err(format!(
                "continuous assignment `{}` has no alias bits at {}:{}:{}",
                self.display_name(source),
                self.node(source).file.as_deref().unwrap_or("<unknown>"),
                self.node(source).line,
                self.node(source).col,
            ));
        }
        let mut group_bits: HashMap<(usize, u32), u32> = HashMap::new();
        for (binding, rhs_bit) in bindings {
            if let Some(previous) =
                group_bits.insert((binding.group(), binding.group_bit()), *rhs_bit)
            {
                if previous != *rhs_bit {
                    return Err(format!(
                        "continuous assignment `{}` drives one aliased net bit from multiple RHS bits at {}:{}:{}",
                        self.display_name(source),
                        self.node(source).file.as_deref().unwrap_or("<unknown>"),
                        self.node(source).line,
                        self.node(source).col,
                    ));
                }
            }
        }
        let mut groups: HashMap<usize, Vec<(IrNetAliasBinding, u32)>> = HashMap::new();
        for (binding, rhs_bit) in bindings {
            groups
                .entry(binding.group())
                .or_default()
                .push((binding.clone(), *rhs_bit));
        }
        let mut group_ids = groups.keys().copied().collect::<Vec<_>>();
        group_ids.sort_unstable();
        let mut result = Vec::with_capacity(group_ids.len());
        for group in group_ids {
            let members = groups.remove(&group).expect("alias group collected above");
            let driver = self
                .structural_driver_signal_for_terminal(source, group, terminal_for_group(group))
                .ok_or_else(|| {
                    format!(
                        "continuous assignment `{}` has no structural driver mapping for alias group {} at {}:{}:{}",
                        self.display_name(source),
                        group,
                        self.node(source).file.as_deref().unwrap_or("<unknown>"),
                        self.node(source).line,
                        self.node(source).col,
                    )
                })?;
            let width = self.model.net_group(group).width;
            let mut parts = Vec::with_capacity(width as usize);
            for group_bit in (0..width).rev() {
                let Some((_, rhs_bit)) = members
                    .iter()
                    .find(|(binding, _)| binding.group_bit() == group_bit)
                else {
                    parts.push(const_z_expr(1));
                    continue;
                };
                if *rhs_bit >= rhs.width() {
                    return Err(format!(
                        "continuous assignment `{}` has an alias RHS bit outside its width at {}:{}:{}",
                        self.display_name(source),
                        self.node(source).file.as_deref().unwrap_or("<unknown>"),
                        self.node(source).line,
                        self.node(source).col,
                    ));
                }
                parts.push(IrExpr::new(
                    IrExprKind::BitSel {
                        base: Box::new(rhs.clone()),
                        idx: Box::new(lhs_integer_expr(i128::from(rhs.width() - 1 - *rhs_bit))),
                    },
                    1,
                    false,
                    None,
                ));
            }
            let value = if parts.len() == 1 {
                parts.pop().expect("one alias group bit")
            } else {
                IrExpr::new(IrExprKind::Concat { parts }, width, false, None)
            };
            result.push((driver, value));
        }
        Ok(result)
    }

    pub(in super::super) fn array_net_endpoint(&self, node: NodeId) -> Option<(usize, u64)> {
        let (base, indices) = match self.kind(node) {
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => (*base, indices.clone()),
            NodeKind::Expr(ExprKind::BitSelect { base, index })
                if self.array_of(*base).is_some() =>
            {
                (*base, vec![*index])
            }
            NodeKind::Expr(
                ExprKind::BitSelect { base, .. }
                | ExprKind::PartSelect { base, .. }
                | ExprKind::IndexedPartSelect { base, .. },
            ) => return self.array_net_endpoint(*base),
            _ => return None,
        };
        let array = self.array_of(base)?;
        if !array.is_net || indices.len() < array.dims.len() {
            return None;
        }
        let indices = indices[..array.dims.len()]
            .iter()
            .map(|index| self.eval_bound_i128(*index).ok().map(lhs_integer_expr))
            .collect::<Option<Vec<_>>>()?;
        Some((
            array.ir,
            Self::array_constant_linear_index(array, &indices)?,
        ))
    }

    pub(in super::super) fn build_net_groups(&mut self) -> Result<(), String> {
        let nodes = self.design_nodes();
        // Union-find over the parent/child nets of every inout port. True
        // aliases use a separate bit-level union-find below because one net
        // can contribute disjoint selected bits to different networks.
        let mut parent: HashMap<NodeId, NodeId> = HashMap::new();
        let mut rank: HashMap<NodeId, u8> = HashMap::new();
        let mut inout_ports: Vec<NodeId> = Vec::new();
        let mut array_ports = HashSet::new();
        let mut array_endpoints: HashMap<(usize, u64), Vec<Option<AliasBit>>> = HashMap::new();
        for (node, info) in &self.array_globals {
            if self.db.array_meta(*node).is_some_and(|meta| {
                matches!(
                    meta.net_type(),
                    Some(
                        NetType::Wand
                            | NetType::TriAnd
                            | NetType::Wor
                            | NetType::TriOr
                            | NetType::Tri0
                            | NetType::Tri1
                            | NetType::Supply0
                            | NetType::Supply1
                    )
                )
            }) {
                for element in 0..self.model.arrays[info.ir].total {
                    array_endpoints
                        .entry((info.ir, element))
                        .or_insert_with(|| vec![None; info.elem_width as usize]);
                }
            }
        }
        let mut alias_parent: HashMap<AliasBit, AliasBit> = HashMap::new();
        let mut alias_rank: HashMap<AliasBit, u8> = HashMap::new();
        let mut alias_bits: HashSet<AliasBit> = HashSet::new();
        let mut alias_nets: HashSet<NodeId> = HashSet::new();
        for id in &nodes {
            let NodeKind::NetAlias { nets } = self.kind(*id) else {
                continue;
            };
            if nets.len() < 2 {
                return Err(format!(
                    "net alias `{}` must contain at least two net expressions at {}:{}:{}",
                    self.display_name(*id),
                    self.node(*id).file.as_deref().unwrap_or("<unknown>"),
                    self.node(*id).line,
                    self.node(*id).col,
                ));
            }
            let expressions = nets
                .iter()
                .map(|expression| self.alias_expression_bits(*id, *expression))
                .collect::<Result<Vec<_>, _>>()?;
            let Some(first) = expressions.first() else {
                return Err(self.alias_error(*id, "has no net expressions"));
            };
            if first.is_empty() {
                return Err(self.alias_error(*id, "has an empty net expression"));
            }
            for expression in &expressions {
                if expression.len() != first.len() {
                    return Err(
                        self.alias_error(*id, "contains net expressions with different widths")
                    );
                }
                alias_bits.extend(expression.iter().copied());
                alias_nets.extend(expression.iter().map(|bit| bit.net));
            }
            for expression in expressions.iter().skip(1) {
                for (first, second) in first.iter().zip(expression) {
                    alias_union(&mut alias_parent, &mut alias_rank, *first, *second);
                }
            }
        }
        // Every bit of an alias-participating net gets an explicit mapping:
        // aliased bits use their shared root, while the remaining bits use a
        // singleton network so ordinary net drivers still resolve electrically.
        for net in &alias_nets {
            let width = match self.kind(*net) {
                NodeKind::Net { ty, .. } => self
                    .signal_of(*net)
                    .map_or(ty.width.unwrap_or(1), |signal| signal.width),
                _ => {
                    return Err(
                        self.alias_error(*nodes.first().unwrap_or(net), "does not reference a net")
                    )
                }
            };
            for bit in 0..width {
                let bit = AliasBit { net: *net, bit };
                alias_bits.insert(bit);
                alias_find(&mut alias_parent, bit);
            }
        }
        for id in &nodes {
            if let NodeKind::Port {
                direction: DbDirection::Inout,
                high: Some(h),
                low: Some(l),
                ..
            } = self.kind(*id)
            {
                if let NodeKind::Port {
                    high_expr: Some(actual),
                    ..
                } = self.kind(*id)
                {
                    if let Some((endpoint, actual_bits)) = self.array_net_selection(*actual)? {
                        let width = self.model.arrays[endpoint.0].elem_width;
                        let formal_bits = self.alias_expression_bits(*id, *l)?;
                        if actual_bits.len() != formal_bits.len() {
                            return Err(
                                "array inout selection width disagrees with its formal".into()
                            );
                        }
                        let peers = array_endpoints
                            .entry(endpoint)
                            .or_insert_with(|| vec![None; width as usize]);
                        for (physical, formal) in actual_bits.into_iter().zip(formal_bits) {
                            if let Some(previous) = peers[physical as usize] {
                                alias_union(&mut alias_parent, &mut alias_rank, previous, formal);
                            } else {
                                peers[physical as usize] = Some(formal);
                            }
                            alias_bits.insert(formal);
                            alias_nets.insert(formal.net);
                        }
                        inout_ports.push(*id);
                        array_ports.insert(*id);
                        continue;
                    }
                    let whole_actual = *actual == *h
                        || matches!(
                            self.kind(*actual),
                            NodeKind::Expr(ExprKind::Ref {
                                target: Some(target)
                            }) if *target == *h
                        )
                        || self
                            .hier_path_signal(*actual)
                            .zip(self.signal_of(*h))
                            .is_some_and(|(actual, target)| actual.ir == target.ir);
                    if !whole_actual {
                        let actual_bits = self.alias_expression_bits(*id, *actual)?;
                        let formal_bits = self.alias_expression_bits(*id, *l)?;
                        if actual_bits.len() != formal_bits.len() {
                            return Err(
                                "selected inout actual width disagrees with its formal".into()
                            );
                        }
                        for (actual, formal) in actual_bits.into_iter().zip(formal_bits) {
                            alias_bits.extend([actual, formal]);
                            alias_nets.extend([actual.net, formal.net]);
                            alias_union(&mut alias_parent, &mut alias_rank, actual, formal);
                        }
                        inout_ports.push(*id);
                        array_ports.insert(*id);
                        continue;
                    }
                }
                // A true alias that touches one side of an inout connection
                // must absorb the other side into the same bit-level union.
                // Otherwise the later whole-net inout collapse would create a
                // second resolved object and split the alias electrically.
                if alias_nets.contains(h) || alias_nets.contains(l) {
                    let high_width = match self.kind(*h) {
                        NodeKind::Net { ty, .. } => self
                            .signal_of(*h)
                            .map_or(ty.width.unwrap_or(1), |signal| signal.width),
                        _ => 1,
                    };
                    let low_width = match self.kind(*l) {
                        NodeKind::Net { ty, .. } => self
                            .signal_of(*l)
                            .map_or(ty.width.unwrap_or(1), |signal| signal.width),
                        _ => 1,
                    };
                    if high_width != low_width {
                        return Err(format!(
                            "inout port `{}` connects alias nets with different widths at {}:{}:{}",
                            self.node(*id).name,
                            self.node(*id).file.as_deref().unwrap_or("<unknown>"),
                            self.node(*id).line,
                            self.node(*id).col,
                        ));
                    }
                    alias_nets.insert(*h);
                    alias_nets.insert(*l);
                    for bit in 0..high_width {
                        let high = AliasBit { net: *h, bit };
                        let low = AliasBit { net: *l, bit };
                        alias_bits.insert(high);
                        alias_bits.insert(low);
                        alias_find(&mut alias_parent, high);
                        alias_find(&mut alias_parent, low);
                        alias_union(&mut alias_parent, &mut alias_rank, high, low);
                    }
                }
                inout_ports.push(*id);
                union(&mut parent, &mut rank, *h, *l);
            } else if let NodeKind::Port {
                direction: DbDirection::Inout,
                high: None,
                ..
            } = self.kind(*id)
            {
                // A top-level inout port has no parent-side connection; there
                // is nothing to collapse, so it stays a plain net (no link is
                // ever emitted for top-level ports).
            } else if let NodeKind::Port {
                direction: DbDirection::Inout,
                high: Some(_),
                low: None,
                ..
            } = self.kind(*id)
            {
                return Err(format!(
                    "inout port `{}` has no child-side connection at {}:{}:{}",
                    self.node(*id).name,
                    self.node(*id).file.as_deref().unwrap_or("<unknown>"),
                    self.node(*id).line,
                    self.node(*id).col,
                ));
            }
        }

        // Port traversal order must not split a whole-net connection from a
        // selected or explicitly aliased endpoint discovered later.
        let mut whole_components: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
        for member in parent.keys().copied().collect::<Vec<_>>() {
            let root = find(&mut parent, member);
            whole_components.entry(root).or_default().push(member);
        }
        for component in whole_components.values() {
            if !component.iter().any(|member| alias_nets.contains(member)) {
                continue;
            }
            let first = component[0];
            let width = self
                .signal_of(first)
                .ok_or("inout component has no packed storage")?
                .width;
            for member in component {
                if self
                    .signal_of(*member)
                    .is_none_or(|signal| signal.width != width)
                {
                    return Err("inout component contains incompatible storage widths".into());
                }
                alias_nets.insert(*member);
                for bit in 0..width {
                    let first = AliasBit { net: first, bit };
                    let other = AliasBit { net: *member, bit };
                    alias_bits.extend([first, other]);
                    alias_union(&mut alias_parent, &mut alias_rank, first, other);
                }
            }
        }
        for member in &alias_nets {
            let width = self
                .signal_of(*member)
                .ok_or("alias member has no packed storage")?
                .width;
            for bit in 0..width {
                let bit = AliasBit { net: *member, bit };
                alias_bits.insert(bit);
                alias_find(&mut alias_parent, bit);
            }
        }

        let mut alias_buckets: HashMap<AliasBit, Vec<AliasBit>> = HashMap::new();
        for bit in alias_bits {
            let root = alias_find(&mut alias_parent, bit);
            alias_buckets.entry(root).or_default().push(bit);
        }
        let mut alias_groups = alias_buckets.into_values().collect::<Vec<_>>();
        for bits in &mut alias_groups {
            bits.sort_by_key(|bit| (bit.net.0, bit.bit));
        }
        alias_groups.sort_by_key(|bits| {
            bits.first()
                .map(|bit| (bit.net.0, bit.bit))
                .unwrap_or((u32::MAX, u32::MAX))
        });
        for bits in alias_groups {
            let mut members = bits.iter().map(|bit| bit.net).collect::<Vec<_>>();
            members.sort_by_key(|id| id.0);
            members.dedup();
            let names = members
                .iter()
                .map(|member| self.display_name(*member))
                .collect::<Vec<_>>()
                .join(", ");
            let shown = format!("net alias group {{{names}}}");
            let first_net = members
                .first()
                .copied()
                .ok_or_else(|| "net alias group has no member nets".to_string())?;
            let first_ty = match self.kind(first_net) {
                NodeKind::Net { ty, .. } => ty.clone(),
                _ => return Err(self.alias_error(first_net, "does not reference a net")),
            };
            let kind = match self.kind(first_net) {
                NodeKind::Net { net_type, .. } => Self::ir_net_kind(*net_type),
                _ => None,
            }
            .ok_or_else(|| {
                format!(
                    "{shown}: member `{}` has an unsupported net type",
                    self.display_name(first_net)
                )
            })?;
            if members.iter().skip(1).any(|member| {
                !matches!(
                    self.kind(*member),
                    NodeKind::Net { net_type, .. }
                        if Self::ir_net_kind(*net_type) == Some(kind)
                )
            }) {
                return Err(format!(
                    "{shown}: members have incompatible net types at {}:{}:{}",
                    self.node(first_net).file.as_deref().unwrap_or("<unknown>"),
                    self.node(first_net).line,
                    self.node(first_net).col,
                ));
            }
            if members.len() > LLG_MAX_NET_DRIVERS {
                return Err(format!(
                    "{shown}: {} members exceed the runtime driver-count range at {}:{}:{}",
                    members.len(),
                    self.node(first_net).file.as_deref().unwrap_or("<unknown>"),
                    self.node(first_net).line,
                    self.node(first_net).col,
                ));
            }
            let name = format!("g_net_{}", self.model.net_groups.len());
            let gidx = self.model.net_groups.len();
            let propagation_delay = self.net_propagation_delay_for_members(&members, &shown)?;
            self.model.net_groups.push(crate::sim::ir::IrNetGroup {
                c_name: name.clone(),
                // One union-find root is one electrical bit, regardless of
                // how many source-net bits name that same identity.
                width: 1,
                signed: first_ty.signed,
                kind,
                n_drivers: members.len(),
                driver_strengths: vec![(6, 6); members.len()],
                propagation_delay,
            });
            for bit in &bits {
                let slot = members
                    .iter()
                    .position(|member| *member == bit.net)
                    .expect("alias group member list contains every alias bit net");
                let info = self.sig_globals.get(&bit.net).ok_or_else(|| {
                    format!(
                        "{shown}: member `{}` has no collected signal storage",
                        self.display_name(bit.net)
                    )
                })?;
                self.model.signals[info.ir]
                    .net_alias
                    .push(IrNetAliasBinding {
                        group: gidx,
                        slot,
                        signal_bit: bit.bit,
                        group_bit: 0,
                    });
            }
            let sources = self.structural_site_sources(&members)?;
            for (source, strengths) in sources {
                let signal = self.add_structural_driver(gidx, source, strengths)?;
                if matches!(self.kind(source), NodeKind::ContAssign { .. }) {
                    self.wired_driver_sites.insert(source, signal);
                }
            }
        }

        // Bucket every distinct member by its union root (a parent net shared
        // by several ports lands in one group).
        let mut members: HashSet<NodeId> = HashSet::new();
        for port in &inout_ports {
            if let NodeKind::Port {
                high: Some(h),
                low: Some(l),
                ..
            } = self.kind(*port)
            {
                if !array_ports.contains(port) && !alias_nets.contains(h) {
                    members.insert(*h);
                }
                if !alias_nets.contains(l) {
                    members.insert(*l);
                }
            }
        }
        let mut buckets: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
        for m in members {
            let r = find(&mut parent, m);
            buckets.entry(r).or_default().push(m);
        }
        let mut groups: Vec<Vec<NodeId>> = buckets.into_values().collect();
        for g in &mut groups {
            g.sort_by_key(|id| id.0);
        }
        groups.sort_by_key(|g| g[0].0);

        let mut member_slots: HashMap<NodeId, (String, usize)> = HashMap::new();
        let mut old_globals: HashMap<String, NodeId> = HashMap::new();
        for members in &groups {
            let names = members
                .iter()
                .map(|m| self.display_name(*m))
                .collect::<Vec<_>>()
                .join(", ");
            let joined = format!("inout-net group {{{names}}}");
            // 1. Members must be plain nets (vars/arrays cannot resolve).
            if let Some(bad) = members
                .iter()
                .find(|m| !matches!(self.kind(**m), NodeKind::Net { .. }))
            {
                return Err(format!(
                    "{joined}: member `{}` is not a net at {}:{}:{}",
                    self.display_name(*bad),
                    self.node(*bad).file.as_deref().unwrap_or("<unknown>"),
                    self.node(*bad).line,
                    self.node(*bad).col,
                ));
            }
            // 2. Widths must agree across the collapsed net.
            let first_ty = match self.kind(members[0]) {
                NodeKind::Net { ty, .. } => ty.clone(),
                _ => unreachable!("validated above"),
            };
            let width = self
                .signal_of(members[0])
                .map_or(first_ty.width.unwrap_or(1), |signal| signal.width);
            if let Some(bad) = members.iter().skip(1).find(|m| match self.kind(**m) {
                NodeKind::Net { ty, .. } => {
                    self.signal_of(**m)
                        .map_or(ty.width.unwrap_or(1), |signal| signal.width)
                        != width
                }
                _ => true,
            }) {
                return Err(format!(
                    "{joined}: member `{}` has a different width than `{}` at {}:{}:{}",
                    self.display_name(*bad),
                    self.display_name(members[0]),
                    self.node(*bad).file.as_deref().unwrap_or("<unknown>"),
                    self.node(*bad).line,
                    self.node(*bad).col,
                ));
            }
            // 3. Every member contributes to one resolution family. Net and
            // tri spellings share wire resolution, while wired and biased
            // spellings retain their own canonical resolver.
            let kind = match self.kind(members[0]) {
                NodeKind::Net { net_type, .. } => Self::ir_net_kind(*net_type),
                _ => None,
            };
            let Some(kind) = kind else {
                return Err(format!(
                    "{joined}: member `{}` has unsupported net type for an executable inout \
                     connection",
                    self.display_name(members[0])
                ));
            };
            if let Some(bad) = members
                .iter()
                .skip(1)
                .find(|member| match self.kind(**member) {
                    NodeKind::Net { net_type, .. } => Self::ir_net_kind(*net_type) != Some(kind),
                    _ => true,
                })
            {
                return Err(format!(
                    "{joined}: member `{}` has an incompatible net type at {}:{}:{}",
                    self.display_name(*bad),
                    self.node(*bad).file.as_deref().unwrap_or("<unknown>"),
                    self.node(*bad).line,
                    self.node(*bad).col,
                ));
            }
            // 4. Driver counts must fit the emitted runtime integer field.
            if members.len() > LLG_MAX_NET_DRIVERS {
                return Err(format!(
                    "{joined}: {}-member group exceeds the runtime driver-count range at {}:{}:{}",
                    members.len(),
                    self.node(members[0]).file.as_deref().unwrap_or("<unknown>"),
                    self.node(members[0]).line,
                    self.node(members[0]).col,
                ));
            }
            // 5. Constant packed selects are admitted as masked contributions
            //    to the member's dedicated slot. Dynamic selects, NBA writes
            //    and task `sv4_t*` actuals would bypass the resolution cell.
            if let Some(reason) = self.unsupported_member_write(members) {
                return Err(format!(
                    "{joined}: {reason} at {}:{}:{}",
                    self.node(members[0]).file.as_deref().unwrap_or("<unknown>"),
                    self.node(members[0]).line,
                    self.node(members[0]).col,
                ));
            }
            let propagation_delay = self.net_propagation_delay_for_members(members, &joined)?;

            // Group is valid: assign one driver slot per member (NodeId
            // order) and redirect every member's storage to the resolved cell.
            let name = format!("g_net_{}", self.model.net_groups.len());
            let gidx = self.model.net_groups.len();
            for (slot, m) in members.iter().enumerate() {
                let old_global = self.sig_globals.get(m).map(|i| i.global.clone());
                if let Some(info) = self.sig_globals.get_mut(m) {
                    info.global = format!("{name}.resolved");
                    info.net_driver = Some((name.clone(), slot));
                    if let Some(sig) = self.model.signals.get_mut(info.ir) {
                        sig.c_name = format!("{name}.resolved");
                        sig.net_driver = Some((gidx, slot));
                    }
                }
                if let Some(g) = old_global {
                    old_globals.insert(g, *m);
                }
                member_slots.insert(*m, (name.clone(), slot));
            }
            // Keep the deterministic emission Vec (`signals`) in sync with
            // the global map so `emit_signals` skips members.
            for info in &mut self.signals {
                if info.net_driver.is_none() {
                    if let Some(m) = old_globals.get(&info.global) {
                        if let Some((n, slot)) = member_slots.get(m) {
                            info.global = format!("{n}.resolved");
                            info.net_driver = Some((n.clone(), *slot));
                        }
                    }
                }
            }
            // Name fallbacks (refs resolved by name) must see the resolved
            // cell too.
            for map in self.scope_sig_names.values_mut() {
                for info in map.values_mut() {
                    if let Some(m) = old_globals.get(&info.global) {
                        if let Some((n, slot)) = member_slots.get(m) {
                            info.global = format!("{n}.resolved");
                            info.net_driver = Some((n.clone(), *slot));
                        }
                    }
                }
            }
            self.model.net_groups.push(crate::sim::ir::IrNetGroup {
                c_name: name.clone(),
                width,
                signed: first_ty.signed,
                kind,
                n_drivers: members.len(),
                driver_strengths: vec![(6, 6); members.len()],
                propagation_delay,
            });
            for member in members {
                let Some(signal) = self.sig_globals.get(member).map(|info| info.ir) else {
                    continue;
                };
                let id = self.record_structural_driver(signal)?;
                self.structural_driver_sites.insert((*member, gidx), id);
            }
            let sources = self.structural_site_sources(members)?;
            for (source, strengths) in sources {
                let signal = self.add_structural_driver(gidx, source, strengths)?;
                if matches!(self.kind(source), NodeKind::ContAssign { .. }) {
                    self.wired_driver_sites.insert(source, signal);
                }
            }
        }

        // Declaration initializers on grouped members (`wire bus = 8'hzz;`)
        // are applied through their driver slot instead of a direct write to
        // the resolved cell.
        let mut keep = Vec::new();
        for (info, c) in std::mem::take(&mut self.scalar_inits) {
            match old_globals
                .get(&info.global)
                .and_then(|m| member_slots.get(m))
            {
                Some((net, slot)) => self.net_inits.push((net.clone(), *slot, c)),
                None => keep.push((info, c)),
            }
        }
        self.scalar_inits = keep;
        self.build_wired_net_groups(&nodes)?;
        self.publish_array_net_cells(array_endpoints, &nodes)?;
        Ok(())
    }

    pub(super) fn add_structural_driver(
        &mut self,
        group: usize,
        source: NodeId,
        strengths: (u8, u8),
    ) -> Result<usize, String> {
        self.add_structural_driver_for_terminal(group, source, strengths, 0)
    }

    /// Add a canonical contribution slot for one primitive output terminal.
    /// Terminal zero uses the original source/group identity; later outputs
    /// get an independent slot when they share that resolved group.
    pub(super) fn add_structural_driver_for_terminal(
        &mut self,
        group: usize,
        source: NodeId,
        strengths: (u8, u8),
        terminal: usize,
    ) -> Result<usize, String> {
        let existing = if terminal == 0 {
            self.structural_driver_sites.get(&(source, group))
        } else {
            self.structural_driver_terminal_sites
                .get(&(source, group, terminal))
        };
        if let Some(signal) = existing {
            return self
                .structural_drivers
                .get(signal.0 as usize)
                .map(|record| record.signal)
                .ok_or_else(|| format!("structural driver {} has no recorded signal", signal.0));
        }
        let net = self
            .model
            .net_groups
            .get_mut(group)
            .ok_or_else(|| format!("structural driver references missing net group {group}"))?;
        let slot = net.n_drivers;
        if slot >= LLG_MAX_NET_DRIVERS {
            return Err(format!(
                "resolved net `{}` has more structural drivers than the runtime can represent",
                net.c_name
            ));
        }
        net.n_drivers += 1;
        net.driver_strengths.push(strengths);
        let c_name = net.c_name.clone();
        let width = net.width;
        let signed = net.signed;
        let signal = self.model.signals.len();
        self.model.signals.push(IrSignal {
            fixed_default: None,
            c_name: format!("{c_name}.resolved"),
            hdl_name: None,
            ty: IrType::Packed {
                width,
                signed,
                two_state: false,
            },
            net_driver: Some((group, slot)),
            net_alias: Vec::new(),
            alias: None,
            omit: false,
        });
        let id = self.record_structural_driver(signal)?;
        if terminal == 0 {
            self.structural_driver_sites.insert((source, group), id);
        } else {
            self.structural_driver_terminal_sites
                .insert((source, group, terminal), id);
        }
        Ok(signal)
    }

    fn record_structural_driver(&mut self, signal: usize) -> Result<DriverId, String> {
        let id = DriverId(
            u32::try_from(self.structural_drivers.len())
                .map_err(|_| "structural driver id space exhausted".to_string())?,
        );
        self.structural_drivers
            .push(StructuralDriverRecord { signal });
        Ok(id)
    }

    pub(super) fn structural_driver_signal(&self, source: NodeId, group: usize) -> Option<usize> {
        self.structural_driver_sites
            .get(&(source, group))
            .and_then(|id| self.structural_drivers.get(id.0 as usize))
            .map(|record| record.signal)
    }

    pub(super) fn structural_driver_signal_for_terminal(
        &self,
        source: NodeId,
        group: usize,
        terminal: usize,
    ) -> Option<usize> {
        if terminal == 0 {
            return self.structural_driver_signal(source, group);
        }
        self.structural_driver_terminal_sites
            .get(&(source, group, terminal))
            .and_then(|id| self.structural_drivers.get(id.0 as usize))
            .map(|record| record.signal)
    }

    pub(super) fn has_structural_driver(&self, source: NodeId) -> bool {
        self.structural_driver_sites
            .keys()
            .any(|(candidate, _)| *candidate == source)
    }

    /// Resolve the strength of an output-port link from the port metadata and
    /// its child-side net declaration.  Slang attaches a net declaration's
    /// drive strength to that child net (the port symbol itself has no
    /// independent drive-strength syntax), while a port can still carry
    /// explicit metadata in a future frontend snapshot.  Keep the fallback
    /// strong/strong so variable outputs and ordinary implicit nets retain
    /// the default structural-driver strength.
    pub(super) fn effective_port_driver_strengths(
        &self,
        port: NodeId,
        explicit0: Strength,
        explicit1: Strength,
        low: Option<NodeId>,
    ) -> Result<(u8, u8), String> {
        let (strength0, strength1) =
            if explicit0 == Strength::Unspecified && explicit1 == Strength::Unspecified {
                match low.map(|id| self.kind(id)) {
                    Some(NodeKind::Net {
                        strength0,
                        strength1,
                        ..
                    }) => (*strength0, *strength1),
                    _ => (explicit0, explicit1),
                }
            } else {
                (explicit0, explicit1)
            };
        port_driver_strengths(strength0, strength1, &self.display_name(port))
    }

    #[allow(clippy::type_complexity)]
    fn structural_site_sources(
        &self,
        members: &[NodeId],
    ) -> Result<Vec<(NodeId, (u8, u8))>, String> {
        let member_set: HashSet<NodeId> = members.iter().copied().collect();
        let mut sources: HashMap<NodeId, (u8, u8)> = HashMap::new();
        for id in self.design_nodes() {
            match self.kind(id) {
                NodeKind::ContAssign {
                    strength0,
                    strength1,
                    ..
                } => {
                    let Some(lhs) = self.node(id).children.first().copied() else {
                        continue;
                    };
                    if self.nested_member_target(lhs, &member_set).is_some() {
                        let width = self
                            .sig_globals
                            .get(&members[0])
                            .map(|info| info.width)
                            .unwrap_or(1);
                        let strengths = continuous_assignment_strengths_for_width(
                            *strength0,
                            *strength1,
                            &self.display_name(id),
                            width,
                        )?;
                        sources.insert(id, strengths);
                    }
                }
                NodeKind::Gate {
                    prim_type,
                    strength0,
                    strength1,
                    terms,
                    ..
                } => {
                    if terms.iter().any(|term| {
                        matches!(term.direction, DbDirection::Output | DbDirection::Inout)
                            && self.nested_member_target(term.expr, &member_set).is_some()
                    }) {
                        let strengths = gate_driver_strengths(
                            *prim_type,
                            *strength0,
                            *strength1,
                            &self.display_name(id),
                        )?;
                        sources.entry(id).or_insert(strengths);
                    }
                }
                NodeKind::Port {
                    direction,
                    high,
                    low,
                    high_expr,
                    strength0,
                    strength1,
                    ..
                } => {
                    let target = match direction {
                        DbDirection::Input => *low,
                        DbDirection::Output => high_expr.or(*high),
                        _ => None,
                    };
                    if target.is_some_and(|target| {
                        self.nested_member_target(target, &member_set).is_some()
                    }) {
                        let strengths =
                            self.effective_port_driver_strengths(id, *strength0, *strength1, *low)?;
                        sources.entry(id).or_insert(strengths);
                    }
                }
                _ => {}
            }
        }
        let mut sources = sources.into_iter().collect::<Vec<_>>();
        sources.sort_by_key(|(source, _)| source.index());
        Ok(sources)
    }

    pub(super) fn ir_net_kind(net_type: NetType) -> Option<crate::sim::ir::IrNetKind> {
        match net_type {
            NetType::Wire | NetType::Tri | NetType::Uwire | NetType::Logic => {
                Some(crate::sim::ir::IrNetKind::Wire)
            }
            NetType::Wand | NetType::TriAnd => Some(crate::sim::ir::IrNetKind::Wand),
            NetType::Wor | NetType::TriOr => Some(crate::sim::ir::IrNetKind::Wor),
            NetType::Tri0 => Some(crate::sim::ir::IrNetKind::Tri0),
            NetType::Tri1 => Some(crate::sim::ir::IrNetKind::Tri1),
            NetType::Supply0 => Some(crate::sim::ir::IrNetKind::Supply0),
            NetType::Supply1 => Some(crate::sim::ir::IrNetKind::Supply1),
            NetType::TriReg | NetType::Reg | NetType::None | NetType::Unsupported => None,
        }
    }

    /// Build one resolved group per standalone scalar wire, tri, or wired net.
    /// Unlike collapsed inout groups, driver identity belongs to the
    /// continuous-assignment site, not to the declaration: two `assign w =
    /// ...` statements must retain two contributions even though both target
    /// the same net object. Ordinary nets participating in module ports or
    /// interfaces stay on the existing link/inout path.
    fn build_wired_net_groups(&mut self, nodes: &[NodeId]) -> Result<(), String> {
        let grouped_members: HashSet<NodeId> = nodes
            .iter()
            .filter_map(|id| {
                self.sig_globals.get(id).and_then(|info| {
                    self.model.signals.get(info.ir).and_then(|signal| {
                        (signal.net_driver.is_some() || !signal.net_alias.is_empty()).then_some(*id)
                    })
                })
            })
            .collect();
        let standalone: Vec<(NodeId, crate::sim::ir::IrNetKind)> = nodes
            .iter()
            .filter_map(|id| match self.kind(*id) {
                NodeKind::Net { net_type, .. } => match net_type {
                    NetType::Wire | NetType::Tri | NetType::Uwire | NetType::Logic => {
                        let in_interface = self.node(*id).parent.is_some_and(|parent| {
                            matches!(
                                self.kind(parent),
                                NodeKind::ModuleInst {
                                    is_interface: true,
                                    ..
                                }
                            )
                        });
                        let already_collapsed = self
                            .sig_globals
                            .get(id)
                            .is_some_and(|info| self.model.signals[info.ir].net_driver.is_some());
                        (!in_interface && !already_collapsed && !grouped_members.contains(id))
                            .then_some((*id, crate::sim::ir::IrNetKind::Wire))
                    }
                    NetType::Wand | NetType::TriAnd => (!grouped_members.contains(id))
                        .then_some((*id, crate::sim::ir::IrNetKind::Wand)),
                    NetType::Wor | NetType::TriOr => (!grouped_members.contains(id))
                        .then_some((*id, crate::sim::ir::IrNetKind::Wor)),
                    NetType::Tri0 => (!grouped_members.contains(id))
                        .then_some((*id, crate::sim::ir::IrNetKind::Tri0)),
                    NetType::Tri1 => (!grouped_members.contains(id))
                        .then_some((*id, crate::sim::ir::IrNetKind::Tri1)),
                    NetType::Supply0 => (!grouped_members.contains(id))
                        .then_some((*id, crate::sim::ir::IrNetKind::Supply0)),
                    NetType::Supply1 => (!grouped_members.contains(id))
                        .then_some((*id, crate::sim::ir::IrNetKind::Supply1)),
                    _ => None,
                },
                _ => None,
            })
            .collect();

        for (net, kind) in standalone {
            let shown = self.display_name(net);
            let member_set = HashSet::from([net]);
            let mut sites: HashMap<NodeId, (u8, u8)> = HashMap::new();
            for id in nodes {
                match self.kind(*id) {
                    NodeKind::ContAssign {
                        strength0,
                        strength1,
                        ..
                    } => {
                        let Some(lhs) = self.node(*id).children.first().copied() else {
                            continue;
                        };
                        // A hierarchical LHS that the owned database resolved to
                        // this net is a real driver identity and is admitted
                        // through the HierPath site below. The source-text
                        // fallback only rejects an unresolved top-self path
                        // (no owned target) so it cannot silently drop a driver.
                        if matches!(self.kind(lhs), NodeKind::Expr(ExprKind::HierPath { .. }))
                            && self.hier_path_signal(lhs).is_none()
                            && self.cont_assign_source_has_hier_lhs(*id, net)
                        {
                            return Err(format!(
                                "hierarchical continuous assignment to wired net `{shown}` is not supported"
                            ));
                        }
                        match self.member_write_kind(lhs, &member_set) {
                            MemberWrite::None => {
                                if matches!(
                                    self.kind(lhs),
                                    NodeKind::Expr(ExprKind::HierPath { .. })
                                ) && self.member_write_base(lhs, &member_set).is_some()
                                {
                                    let width = self
                                        .sig_globals
                                        .get(&net)
                                        .map(|info| info.width)
                                        .unwrap_or(1);
                                    let strengths = continuous_assignment_strengths_for_width(
                                        *strength0, *strength1, &shown, width,
                                    )?;
                                    sites.insert(*id, strengths);
                                } else if self.nested_member_target(lhs, &member_set).is_some() {
                                    let width = self
                                        .sig_globals
                                        .get(&net)
                                        .map(|info| info.width)
                                        .unwrap_or(1);
                                    let strengths = continuous_assignment_strengths_for_width(
                                        *strength0, *strength1, &shown, width,
                                    )?;
                                    sites.insert(*id, strengths);
                                }
                            }
                            MemberWrite::Whole => {
                                let width = self
                                    .sig_globals
                                    .get(&net)
                                    .map(|info| info.width)
                                    .unwrap_or(1);
                                sites.insert(
                                    *id,
                                    continuous_assignment_strengths_for_width(
                                        *strength0, *strength1, &shown, width,
                                    )?,
                                );
                            }
                            MemberWrite::Select => {
                                let width = self
                                    .sig_globals
                                    .get(&net)
                                    .map(|info| info.width)
                                    .unwrap_or(1);
                                let strengths = continuous_assignment_strengths_for_width(
                                    *strength0, *strength1, &shown, width,
                                )?;
                                sites.insert(*id, strengths);
                            }
                        }
                    }
                    NodeKind::Stmt(StmtKind::Assign { blocking, .. }) => {
                        let Some(lhs) = self.node(*id).children.first().copied() else {
                            continue;
                        };
                        if self.nested_member_target(lhs, &member_set).is_some() {
                            let form = if *blocking { "blocking" } else { "nonblocking" };
                            return Err(format!(
                                "procedural {form} assignment to wired net `{shown}` is not supported"
                            ));
                        }
                    }
                    NodeKind::Stmt(StmtKind::ProcContAssign { lhs, .. }) => {
                        if self.nested_member_target(*lhs, &member_set).is_some() {
                            if matches!(kind, crate::sim::ir::IrNetKind::Wire) {
                                return Err(format!(
                                    "procedural continuous assignment targets variables only; resolved net `{shown}` is not supported"
                                ));
                            }
                            return Err(format!(
                                "procedural continuous assignment to wired net `{shown}` is not supported"
                            ));
                        }
                    }
                    NodeKind::Stmt(StmtKind::Deassign { lhs }) => {
                        if self.nested_member_target(*lhs, &member_set).is_some() {
                            return Err(format!(
                                "procedural deassign of wired net `{shown}` is not supported"
                            ));
                        }
                    }
                    NodeKind::Stmt(StmtKind::Force { .. })
                    | NodeKind::Stmt(StmtKind::Release { .. }) => {}
                    NodeKind::Gate {
                        prim_type,
                        strength0,
                        strength1,
                        terms,
                        ..
                    } => {
                        if terms.iter().any(|term| {
                            matches!(term.direction, DbDirection::Output | DbDirection::Inout)
                                && self.nested_member_target(term.expr, &member_set).is_some()
                        }) {
                            let strengths = gate_driver_strengths(
                                *prim_type,
                                *strength0,
                                *strength1,
                                &self.display_name(*id),
                            )?;
                            sites.insert(*id, strengths);
                        }
                    }
                    NodeKind::Port {
                        direction,
                        high,
                        low,
                        high_expr,
                        strength0,
                        strength1,
                        ..
                    } => {
                        let target = match direction {
                            DbDirection::Input => *low,
                            DbDirection::Output => high_expr.or(*high),
                            _ => None,
                        };
                        if target.is_some_and(|target| {
                            self.nested_member_target(target, &member_set).is_some()
                        }) {
                            sites.insert(
                                *id,
                                self.effective_port_driver_strengths(
                                    *id, *strength0, *strength1, *low,
                                )?,
                            );
                        }
                    }
                    NodeKind::FuncCall { .. }
                        if self.task_actual_member_write(*id, &member_set).is_some() =>
                    {
                        return Err(format!(
                            "function/task output/inout driving wired net `{shown}` is not supported"
                        ));
                    }
                    _ => {}
                }
            }
            let mut sites = sites.into_iter().collect::<Vec<_>>();
            sites.sort_by_key(|(id, _)| id.0);
            if sites.len() > LLG_MAX_NET_DRIVERS {
                return Err(format!(
                    "wired net `{shown}` has {} continuous driver sites, exceeding the runtime driver-count range",
                    sites.len()
                ));
            }

            let info = self
                .sig_globals
                .get(&net)
                .cloned()
                .ok_or_else(|| format!("wired net `{shown}` has no lowered scalar storage"))?;
            if info.real {
                return Err(format!("real-valued wired net `{shown}` is not supported"));
            }
            let propagation_delay =
                self.net_propagation_delay_for_members(std::slice::from_ref(&net), &shown)?;
            let name = format!("g_net_{}", self.model.net_groups.len());
            let group = self.model.net_groups.len();
            let initial_drivers = usize::from(sites.is_empty());
            self.model.net_groups.push(crate::sim::ir::IrNetGroup {
                c_name: name.clone(),
                width: info.width,
                signed: info.signed,
                kind,
                n_drivers: initial_drivers,
                driver_strengths: vec![(6, 6); initial_drivers],
                propagation_delay,
            });

            let old_global = info.global;
            let resolved = format!("{name}.resolved");
            if let Some(mapped) = self.sig_globals.get_mut(&net) {
                mapped.global = resolved.clone();
                mapped.net_driver = Some((name.clone(), 0));
            }
            if let Some(signal) = self.model.signals.get_mut(info.ir) {
                signal.c_name = resolved.clone();
                signal.net_driver = Some((group, 0));
            }
            for signal in &mut self.signals {
                if signal.global == old_global {
                    signal.global = resolved.clone();
                    signal.net_driver = Some((name.clone(), 0));
                }
            }
            for names in self.scope_sig_names.values_mut() {
                for signal in names.values_mut() {
                    if signal.global == old_global {
                        signal.global = resolved.clone();
                        signal.net_driver = Some((name.clone(), 0));
                    }
                }
            }

            if sites.is_empty() {
                let id = self.record_structural_driver(info.ir)?;
                self.structural_driver_sites.insert((net, group), id);
            }

            for (source, strengths) in sites {
                let signal = self.add_structural_driver(group, source, strengths)?;
                if matches!(self.kind(source), NodeKind::ContAssign { .. }) {
                    self.wired_driver_sites.insert(source, signal);
                }
            }
        }
        Ok(())
    }

    /// The reason a candidate inout-net group cannot be supported, from a
    /// design-wide scan of every write targeting its members. Constant packed
    /// selected continuous drivers are admitted; procedural and dynamic
    /// writes still fail closed.
    fn unsupported_member_write(&self, members: &[NodeId]) -> Option<String> {
        let member_set: HashSet<NodeId> = members.iter().copied().collect();
        for id in self.design_nodes() {
            match self.kind(id) {
                NodeKind::ContAssign { .. } => {
                    let Some(lhs) = self.node(id).children.first().copied() else {
                        continue;
                    };
                    match self.member_write_kind(lhs, &member_set) {
                        MemberWrite::None | MemberWrite::Whole => {}
                        MemberWrite::Select => {
                            if !self.net_lvalue_selects_are_constant(lhs) {
                                return Some(format!(
                                    "dynamic bit/part/select LHS on member `{}`",
                                    self.display_name(
                                        self.member_write_base(lhs, &member_set).unwrap_or(lhs)
                                    )
                                ));
                            }
                        }
                    }
                }
                NodeKind::Stmt(StmtKind::Assign { blocking: true, .. }) => {
                    let Some(lhs) = self.node(id).children.first().copied() else {
                        continue;
                    };
                    match self.member_write_kind(lhs, &member_set) {
                        MemberWrite::None | MemberWrite::Whole => {}
                        MemberWrite::Select => {
                            return Some(format!(
                                "bit/part/select LHS on member `{}`",
                                self.display_name(
                                    self.member_write_base(lhs, &member_set).unwrap_or(lhs)
                                )
                            ))
                        }
                    }
                }
                NodeKind::Stmt(StmtKind::Assign {
                    blocking: false, ..
                }) => {
                    let Some(lhs) = self.node(id).children.first().copied() else {
                        continue;
                    };
                    if let Some(member) = self.member_write_base(lhs, &member_set) {
                        return Some(format!(
                            "nonblocking assignment to member `{}`",
                            self.display_name(member)
                        ));
                    }
                }
                NodeKind::FuncCall { is_task: true, .. } => {
                    if let Some(reason) = self.task_actual_member_write(id, &member_set) {
                        return Some(reason);
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// How an assignment LHS touches a member set: not at all, as a whole
    /// signal, or through a packed select. Dynamic select validation is kept
    /// separate so the same classifier can be used by all driver scans.
    fn member_write_kind(&self, lhs: NodeId, member_set: &HashSet<NodeId>) -> MemberWrite {
        match self.kind(lhs) {
            NodeKind::Net { .. } if member_set.contains(&lhs) => MemberWrite::Whole,
            NodeKind::Expr(ExprKind::Ref { target }) => match target {
                Some(t) if member_set.contains(t) => MemberWrite::Whole,
                _ => MemberWrite::None,
            },
            NodeKind::Expr(
                ExprKind::BitSelect { .. }
                | ExprKind::PartSelect { .. }
                | ExprKind::IndexedPartSelect { .. }
                | ExprKind::ArraySelect { .. },
            ) => {
                if self.member_write_base(lhs, member_set).is_some() {
                    MemberWrite::Select
                } else {
                    MemberWrite::None
                }
            }
            _ => MemberWrite::None,
        }
    }

    /// The member (if any) a select chain or ref ultimately writes to.
    fn member_write_base(&self, node: NodeId, member_set: &HashSet<NodeId>) -> Option<NodeId> {
        match self.kind(node) {
            NodeKind::Net { .. } if member_set.contains(&node) => Some(node),
            NodeKind::Expr(ExprKind::Ref { target }) => target.as_ref().and_then(|target| {
                if member_set.contains(target) {
                    Some(*target)
                } else {
                    self.member_write_base(*target, member_set)
                }
            }),
            NodeKind::Expr(ExprKind::HierPath { .. }) => {
                let signal = self.hier_path_signal(node)?;
                member_set.iter().find_map(|member| {
                    self.signal_of(*member)
                        .filter(|candidate| candidate.ir == signal.ir)
                        .map(|_| *member)
                })
            }
            NodeKind::Expr(
                ExprKind::BitSelect { base, .. }
                | ExprKind::PartSelect { base, .. }
                | ExprKind::IndexedPartSelect { base, .. }
                | ExprKind::ArraySelect { base, .. },
            ) => self.member_write_base(*base, member_set),
            _ => None,
        }
    }

    /// Find a wired member anywhere inside an LHS-shaped expression.  This
    /// closes fail-closed checks for concatenations and other compound actuals
    /// that the supported direct/ref/select classifier intentionally ignores.
    fn nested_member_target(&self, node: NodeId, member_set: &HashSet<NodeId>) -> Option<NodeId> {
        self.member_write_base(node, member_set).or_else(|| {
            self.node(node)
                .children
                .iter()
                .find_map(|child| self.nested_member_target(*child, member_set))
        })
    }

    /// Whether a task call binds an output/inout formal to a member: those
    /// actuals become `sv4_t*` parameters in the emitted C and would write
    /// through the resolved cell, bypassing resolution.
    fn task_actual_member_write(
        &self,
        call: NodeId,
        member_set: &HashSet<NodeId>,
    ) -> Option<String> {
        let (name, is_task, callee) = match self.kind(call) {
            NodeKind::FuncCall {
                name,
                is_task,
                callee,
                ..
            } => (name.clone(), *is_task, *callee),
            _ => return None,
        };
        let inst = self.owning_inst(call)?;
        let (ft, callee_inst) = self.resolve_callee_env(inst, &name, is_task, callee).ok()?;
        let (_, _, formals) = self.func_info(ft, callee_inst).ok()?;
        let args: Vec<NodeId> = self.node(call).children.clone();
        for (idx, (io, is_out)) in formals.iter().enumerate() {
            if !*is_out {
                continue;
            }
            if let Some(arg) = args.get(idx) {
                if let Some(m) = self.nested_member_target(*arg, member_set) {
                    return Some(format!(
                        "task output/inout actual `{}` on member `{}`",
                        self.node(*io).name,
                        self.display_name(m)
                    ));
                }
            }
        }
        None
    }
}
