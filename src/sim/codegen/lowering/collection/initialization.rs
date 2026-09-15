//! Initialization.

use super::*;

impl<'a> Codegen<'a> {

    /// Fold every declaration initializer of a scalar VARIABLE whose init
    /// is attached to the variable (`logic l = 1'b0;`, `int x = 5;` —
    /// captured in [`Db::vars_init`]) into a constant and queue it for
    /// `main()`.  Called after the scope's parameters are collected so
    /// `P + 1`-style RHS refs resolve via `param_vals`.  Vars that are not
    /// collected as signals (function/block locals) carry no
    /// fill; their initializers are handled by their own paths.
    pub(super) fn collect_var_inits(&mut self, path: &str, inst: NodeId) -> Result<(), String> {
        self.inst = inst;
        for c in &self.node(inst).children {
            if !matches!(self.kind(*c), NodeKind::Var { .. }) {
                continue;
            }
            if self.db.virtual_interface_target(*c).is_some() {
                continue;
            }
            let init = match self.db.var_initializer(*c) {
                Some(init) => init,
                None => continue,
            };
            if let Some(aggregate) = self.unpacked_aggregates.get(c).cloned() {
                self.collect_unpacked_aggregate_decl_init(path, *c, init, &aggregate)?;
                continue;
            }
            let info = match self.signal_of(*c) {
                Some(info) => info.clone(),
                None => continue,
            };
            if let Some(layout) = self.db.aggregate_layout(*c) {
                if matches!(
                    layout.kind,
                    AggregateKind::PackedStruct | AggregateKind::PackedUnion
                ) && matches!(
                    self.kind(init),
                    NodeKind::Expr(ExprKind::Operation { op, .. })
                        if *op == Operation::AssignmentPattern
                ) {
                    let value = self.packed_aggregate_decl_init(path, init, layout, &info)?;
                    self.var_inits.push((info, value));
                    continue;
                }
            }
            let name = self.node(*c).name.clone();
            let initializer = self.lower_declaration_initializer(
                path,
                *c,
                init,
                IrInitTarget::Signal(info.ir),
                info.width,
                info.signed,
                info.two_state,
                info.real,
            );
            match initializer {
                Ok(initializer) => self.declaration_inits.push(initializer),
                Err(lowering_error) => match self.var_decl_init(path, &name, init) {
                    Ok(cconst) => self.var_inits.push((info, cconst)),
                    Err(_) => return Err(lowering_error),
                },
            }
        }
        Ok(())
    }

    fn packed_aggregate_decl_init(
        &self,
        path: &str,
        init: NodeId,
        layout: &crate::core::db::AggregateLayout,
        storage: &SignalInfo,
    ) -> Result<IrConst, String> {
        let value = self.packed_aggregate_decl_value(path, init, layout)?;
        let value = materialize_decl_cast_value(
            value,
            storage.width as usize,
            storage.signed,
            storage.two_state,
        );
        decl_value_to_const(Val::Bits(value))
    }

    fn collect_unpacked_aggregate_decl_init(
        &mut self,
        path: &str,
        object: NodeId,
        init: NodeId,
        aggregate: &UnpackedAggregateInfo,
    ) -> Result<(), String> {
        let layout = self.db.aggregate_layout(object).ok_or_else(|| {
            format!(
                "unpacked aggregate `{}` in `{path}` has no captured layout",
                self.node(object).name
            )
        })?;
        let mut values = Vec::new();
        self.aggregate_pattern_leaf_values(path, init, layout, &[], &mut values)?;
        for (member_path, value_node) in values {
            let member = aggregate
                .leaves
                .iter()
                .find(|leaf| leaf.path == member_path)
                .ok_or_else(|| {
                    format!(
                        "aggregate initializer path `{}` has no storage in `{path}`",
                        aggregate_path_suffix(&member_path)
                    )
                })?;
            if let Some(index) = member.object {
                match self.model.objects[index].ty {
                    crate::sim::ir::IrObjectType::String => {
                        self.model.objects[index].initial =
                            Some(self.lower_string(path, value_node)?);
                    }
                    crate::sim::ir::IrObjectType::Chandle => {
                        if self.lower_chandle(path, value_node)? != IrChandleExpr::Null {
                            return Err(format!(
                                "chandle aggregate initializer `{}` must be null",
                                aggregate_path_suffix(&member_path)
                            ));
                        }
                    }
                    crate::sim::ir::IrObjectType::Semaphore => {
                        return Err(format!(
                            "semaphore aggregate initializer `{}` is not supported",
                            aggregate_path_suffix(&member_path)
                        ));
                    }
                    crate::sim::ir::IrObjectType::Process => {
                        return Err(format!(
                            "process aggregate initializer `{}` is not supported",
                            aggregate_path_suffix(&member_path)
                        ));
                    }
                }
                continue;
            }
            let signal = member.signal.as_ref().ok_or_else(|| {
                format!(
                    "aggregate initializer path `{}` has no scalar storage in `{path}`",
                    aggregate_path_suffix(&member_path)
                )
            })?;
            if signal.real {
                let value = match self.eval_decl_value(value_node) {
                    Ok(value) => value,
                    Err(_) => {
                        let initializer = self.lower_declaration_initializer(
                            path,
                            object,
                            value_node,
                            IrInitTarget::Signal(signal.ir),
                            signal.width,
                            signal.signed,
                            signal.two_state,
                            true,
                        )?;
                        self.declaration_inits.push(initializer);
                        continue;
                    }
                };
                let value = match value {
                    Val::Real(value) => Val::Real(value),
                    Val::Bits(value) => {
                        Val::Real(elab::ieee_bits_to_real(&value).ok_or_else(|| {
                            "real aggregate initializer is not an IEEE value".to_owned()
                        })?)
                    }
                    Val::Str(_) => {
                        return Err(format!(
                            "string value cannot initialize real aggregate member `{}` in `{path}`",
                            aggregate_path_suffix(&member_path)
                        ))
                    }
                };
                self.var_inits
                    .push((signal.clone(), decl_value_to_const(value)?));
                continue;
            }
            let width = member.member.ty.width.ok_or_else(|| {
                format!(
                    "unpacked member `{}` has unresolved width in `{path}`",
                    aggregate_path_suffix(&member_path)
                )
            })?;
            match self.aggregate_member_decl_value(path, value_node, &member.member, width) {
                Ok(value) => {
                    let value = value.cast(signal.width as usize, signal.signed);
                    self.var_inits
                        .push((signal.clone(), decl_value_to_const(Val::Bits(value))?));
                }
                Err(_) => {
                    let initializer = self.lower_declaration_initializer(
                        path,
                        object,
                        value_node,
                        IrInitTarget::Signal(signal.ir),
                        signal.width,
                        signal.signed,
                        signal.two_state,
                        false,
                    )?;
                    self.declaration_inits.push(initializer);
                }
            }
        }
        Ok(())
    }

    fn aggregate_member_decl_value(
        &self,
        path: &str,
        value_node: NodeId,
        member: &AggregateMember,
        width: u32,
    ) -> Result<elab::Value, String> {
        if let Some(layout) = member.aggregate_layout() {
            if matches!(
                self.kind(value_node),
                NodeKind::Expr(ExprKind::Operation { op, .. })
                    if *op == Operation::AssignmentPattern
            ) {
                if !matches!(
                    layout.kind,
                    AggregateKind::PackedStruct | AggregateKind::PackedUnion
                ) {
                    return Err(format!(
                        "nested unpacked aggregate member `{}` in `{path}` is not supported",
                        member.name
                    ));
                }
                let value = self.packed_aggregate_decl_value(path, value_node, layout)?;
                return Ok(materialize_decl_cast_value(
                    value,
                    width as usize,
                    member.ty.signed,
                    member.two_state,
                ));
            }
        }
        if let Some(fill) = self.source_fill_literal(value_node) {
            let fill = match fill {
                0 => Bit::Zero,
                1 => Bit::One,
                2 => Bit::X,
                3 => Bit::Z,
                _ => return Err(format!("invalid aggregate fill value {fill} in `{path}`")),
            };
            return Ok(materialize_decl_cast_value(
                elab::Value {
                    bits: vec![fill],
                    signed: false,
                    fill: Some(fill),
                },
                width as usize,
                member.ty.signed,
                member.two_state,
            ));
        }
        let value = match self.eval_decl_value(value_node)? {
            Val::Bits(value) => value,
            Val::Real(value) => elab::real_to_bits(value, width as usize, member.ty.signed),
            Val::Str(_) => {
                return Err(format!(
                    "string value for aggregate member `{}` is not supported",
                    member.name
                ))
            }
        };
        Ok(materialize_decl_cast_value(
            value,
            width as usize,
            member.ty.signed,
            member.two_state,
        ))
    }

    fn packed_aggregate_decl_value(
        &self,
        path: &str,
        init: NodeId,
        layout: &crate::core::db::AggregateLayout,
    ) -> Result<elab::Value, String> {
        let values = self.aggregate_pattern_values(path, init, layout)?;
        let mut members = Vec::with_capacity(values.len());
        for (member_index, value_node) in values {
            let member = layout.members.get(member_index).ok_or_else(|| {
                format!("aggregate initializer member index {member_index} is out of bounds")
            })?;
            let width = member.ty.width.ok_or_else(|| {
                format!(
                    "packed member `{}` has unresolved width in `{path}`",
                    member.name
                )
            })?;
            members.push(self.aggregate_member_decl_value(path, value_node, member, width)?);
        }
        if layout.kind == AggregateKind::PackedUnion {
            members
                .into_iter()
                .next()
                .ok_or_else(|| format!("packed union assignment pattern is empty in `{path}`"))
        } else {
            Ok(elab::concat(&members))
        }
    }

    pub(in super::super) fn aggregate_pattern_values(
        &self,
        path: &str,
        init: NodeId,
        layout: &crate::core::db::AggregateLayout,
    ) -> Result<Vec<(usize, NodeId)>, String> {
        let NodeKind::Expr(ExprKind::Operation {
            op,
            operands,
            reordered,
            ..
        }) = self.kind(init)
        else {
            return Err(format!(
                "declaration initializer for aggregate in `{path}` is not an assignment pattern"
            ));
        };
        if *op != Operation::AssignmentPattern {
            return Err(format!(
                "declaration initializer for aggregate in `{path}` is not an assignment pattern"
            ));
        }
        let mut operands = operands.clone();
        if *reordered {
            operands.reverse();
        }
        let tagged = operands.iter().any(|operand| {
            matches!(
                self.kind(*operand),
                NodeKind::Expr(ExprKind::TaggedPattern { .. })
            )
        });
        let is_union = matches!(
            layout.kind,
            AggregateKind::PackedUnion | AggregateKind::UnpackedUnion
        );
        if !tagged {
            let expected = if is_union { 1 } else { layout.members.len() };
            if operands.len() != expected {
                return Err(format!(
                    "aggregate assignment pattern in `{path}` has {} positional values; expected {expected}",
                    operands.len()
                ));
            }
            return Ok(operands.into_iter().enumerate().collect());
        }
        if operands.iter().any(|operand| {
            !matches!(
                self.kind(*operand),
                NodeKind::Expr(ExprKind::TaggedPattern { .. })
            )
        }) {
            if !is_union && operands.len() == layout.members.len() {
                // The elaborated snapshot replaces resolved member/default
                // keys with their values but retains resolved type keys as
                // tagged operands, all in declaration order.
                return operands
                    .into_iter()
                    .enumerate()
                    .map(|(index, operand)| match self.kind(operand) {
                        NodeKind::Expr(ExprKind::TaggedPattern {
                            value: Some(value),
                            ..
                        }) => Ok((index, *value)),
                        NodeKind::Expr(ExprKind::TaggedPattern { .. }) => Err(format!(
                            "flattened aggregate assignment pattern operand {index} has no value in `{path}`"
                        )),
                        _ => Ok((index, operand)),
                    })
                    .collect();
            }
            return Err(format!(
                "mixed positional and keyed aggregate assignment pattern in `{path}` is not supported"
            ));
        }

        let mut explicit = vec![None; layout.members.len()];
        let mut type_values: Vec<(String, AssignmentPatternKeyType, NodeId)> = Vec::new();
        let mut default = None;
        for operand in operands {
            let NodeKind::Expr(ExprKind::TaggedPattern {
                key,
                key_type,
                value,
            }) = self.kind(operand)
            else {
                continue;
            };
            let key = key.as_deref().ok_or_else(|| {
                format!("aggregate assignment pattern key is unavailable in `{path}`")
            })?;
            let value = value.ok_or_else(|| {
                format!("aggregate assignment pattern key `{key}` has no value in `{path}`")
            })?;
            if key == "default" {
                if default.replace(value).is_some() {
                    return Err(format!(
                        "duplicate default key in aggregate assignment pattern in `{path}`"
                    ));
                }
                continue;
            }
            if let Some(index) = layout.members.iter().position(|member| member.name == key) {
                if explicit[index].replace(value).is_some() {
                    return Err(format!(
                        "duplicate aggregate member key `{key}` in `{path}`"
                    ));
                }
                continue;
            }
            let Some(key_type) = key_type else {
                return Err(format!(
                    "aggregate assignment pattern key `{key}` has no matching member or type in `{path}`"
                ));
            };
            if !layout
                .members
                .iter()
                .any(|member| aggregate_member_matches_type_key(member, key, Some(key_type)))
            {
                return Err(format!(
                    "aggregate assignment pattern key `{key}` has no matching member or type in `{path}`"
                ));
            }
            if type_values
                .iter()
                .any(|(_, previous, _)| pattern_key_types_equal(previous, key_type))
            {
                return Err(format!("duplicate aggregate type key `{key}` in `{path}`"));
            }
            type_values.push((key.to_owned(), key_type.clone(), value));
        }

        let resolved = layout
            .members
            .iter()
            .enumerate()
            .filter_map(|(index, member)| {
                explicit[index]
                    .or_else(|| {
                        type_values.iter().rev().find_map(|(key, key_type, value)| {
                            aggregate_member_matches_type_key(member, key, Some(key_type))
                                .then_some(*value)
                        })
                    })
                    .or(default)
                    .map(|value| (index, value))
            })
            .collect::<Vec<_>>();
        if is_union {
            if resolved.len() != 1 {
                return Err(format!(
                    "untagged union assignment pattern in `{path}` must select exactly one member"
                ));
            }
        } else if resolved.len() != layout.members.len() {
            let missing = layout
                .members
                .iter()
                .enumerate()
                .find(|(index, _)| !resolved.iter().any(|(set, _)| set == index))
                .map(|(_, member)| member.name.as_str())
                .unwrap_or("<unknown>");
            return Err(format!(
                "aggregate assignment pattern in `{path}` does not cover member `{missing}`"
            ));
        }
        Ok(resolved)
    }

    /// The arena node of the array an LHS `Ref` resolves to, or `None`.
    fn ref_array_target(&self, node: NodeId) -> Option<NodeId> {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target: Some(t) })
                if matches!(self.kind(*t), NodeKind::Array { .. }) =>
            {
                Some(*t)
            }
            _ => None,
        }
    }

    /// The array a net-declaration continuous assignment initializes (its
    /// LHS resolves to an `Array` node), or `None` for other assignments.
    pub(super) fn cont_assign_array_target(&self, ca: NodeId) -> Option<NodeId> {
        self.node(ca)
            .children
            .first()
            .copied()
            .and_then(|lhs| self.ref_array_target(lhs))
    }

    fn cont_assign_decl_target(&self, ca: NodeId) -> Option<NodeId> {
        let lhs = self.node(ca).children.first().copied()?;
        match self.kind(lhs) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            NodeKind::Net { .. } | NodeKind::Var { .. } | NodeKind::Array { .. } => Some(lhs),
            _ => None,
        }
    }

    pub(super) fn collect_aggregate_cont_assign_init(
        &mut self,
        path: &str,
        ca: NodeId,
    ) -> Result<bool, String> {
        if !matches!(self.net_decl_target(ca), NetDeclTarget::Variable) {
            return Ok(false);
        }
        let Some(target) = self.cont_assign_decl_target(ca) else {
            return Ok(false);
        };
        let Some(rhs) = self.node(ca).children.get(1).copied() else {
            return Ok(false);
        };
        if !matches!(
            self.kind(rhs),
            NodeKind::Expr(ExprKind::Operation { op, .. })
                if *op == Operation::AssignmentPattern
        ) {
            return Ok(false);
        }
        if let Some(aggregate) = self.unpacked_aggregates.get(&target).cloned() {
            self.collect_unpacked_aggregate_decl_init(path, target, rhs, &aggregate)?;
            return Ok(true);
        }
        let Some(layout) = self.db.aggregate_layout(target).cloned() else {
            return Ok(false);
        };
        if !matches!(
            layout.kind,
            AggregateKind::PackedStruct | AggregateKind::PackedUnion
        ) {
            return Ok(false);
        }
        let Some(info) = self.signal_of(target).cloned() else {
            return Ok(false);
        };
        let value = self.packed_aggregate_decl_init(path, rhs, &layout, &info)?;
        self.scalar_inits.push((info, value));
        Ok(true)
    }

    /// Classify the declaration object on the LHS of a net-declaration
    /// assignment. `wire`, `tri`, and SV `logic` nets are true continuous
    /// drivers; `reg` and variable objects retain declaration-initializer
    /// behavior. Unpacked arrays stay on the dedicated initializer path.
    pub(super) fn net_decl_target(&self, ca: NodeId) -> NetDeclTarget {
        let Some(lhs) = self.node(ca).children.first().copied() else {
            return NetDeclTarget::Unknown;
        };
        let target = match self.kind(lhs) {
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            NodeKind::Net { .. } | NodeKind::Var { .. } | NodeKind::Array { .. } => Some(lhs),
            _ => None,
        };
        match target.map(|target| self.kind(target)) {
            Some(NodeKind::Array { .. }) => NetDeclTarget::Array,
            Some(NodeKind::Var { .. }) => NetDeclTarget::Variable,
            Some(NodeKind::Net { net_type, .. }) => match *net_type {
                NetType::None => NetDeclTarget::Variable,
                NetType::Wire
                | NetType::Tri
                | NetType::Uwire
                | NetType::Logic
                | NetType::Wand
                | NetType::TriAnd
                | NetType::Wor
                | NetType::TriOr
                | NetType::Tri0
                | NetType::Tri1
                | NetType::Supply0
                | NetType::Supply1 => NetDeclTarget::TrueNet,
                NetType::Reg => NetDeclTarget::Variable,
                other => NetDeclTarget::UnsupportedNet(other),
            },
            _ => NetDeclTarget::Unknown,
        }
    }

    /// The declaration-initializer constants of a net-declaration
    /// continuous assignment whose LHS resolves to an unpacked array
    /// (`reg [7:0] m [0:3] = '{…}`), or `None` when the assignment is not an
    /// array initializer.
    pub(super) fn cont_assign_array_init(
        &self,
        path: &str,
        ca: NodeId,
    ) -> Result<Option<(NodeId, Vec<IrConst>)>, String> {
        let target = match self.cont_assign_array_target(ca) {
            Some(t) => t,
            None => return Ok(None),
        };
        if !matches!(self.kind(target), NodeKind::Array { .. }) {
            return Ok(None);
        }
        let name = self.node(target).name.clone();
        let rhs = self
            .node(ca)
            .children
            .get(1)
            .copied()
            .ok_or_else(|| format!("array initializer for `{name}` in `{path}` without RHS"))?;
        let vals = match self.array_init_consts(path, &name, rhs) {
            Ok(values) => values,
            Err(_) => return Ok(None),
        };
        Ok(Some((target, vals)))
    }

    pub(super) fn collect_scalar_decl_init(&mut self, path: &str, ca: NodeId) -> Result<(), String> {
        let target = self.cont_assign_decl_target(ca).ok_or_else(|| {
            format!("variable declaration initializer in `{path}` has no scalar target")
        })?;
        let info = self.signal_of(target).cloned().ok_or_else(|| {
            format!(
                "variable declaration initializer for `{}` in `{path}` has no storage",
                self.node(target).name
            )
        })?;
        let initializer = self.node(ca).children.get(1).copied().ok_or_else(|| {
            format!(
                "variable declaration initializer for `{}` in `{path}` has no RHS",
                self.node(target).name
            )
        })?;
        if let Some(inst) = self.owning_inst(ca) {
            self.inst = inst;
        }
        let lowered = self.lower_declaration_initializer(
            path,
            target,
            initializer,
            IrInitTarget::Signal(info.ir),
            info.width,
            info.signed,
            info.two_state,
            info.real,
        );
        match lowered {
            Ok(initializer) => self.declaration_inits.push(initializer),
            Err(lowering_error) => {
                let value = self.scalar_decl_init(path, ca)?.ok_or(lowering_error)?.1;
                self.scalar_inits.push((info, value));
            }
        }
        self.scalar_init_ca.insert(ca);
        Ok(())
    }

    /// The declaration-initializer constant of a net-declaration assignment whose LHS
    /// is a scalar variable-like object (`reg y = 0`). The caller classifies
    /// the target first; true nets never enter this constant-only path.
    fn scalar_decl_init(
        &self,
        path: &str,
        ca: NodeId,
    ) -> Result<Option<(SignalInfo, IrConst)>, String> {
        let lhs = match self.node(ca).children.first() {
            Some(l) => *l,
            None => return Ok(None),
        };
        let rhs = match self.node(ca).children.get(1) {
            Some(r) => *r,
            None => return Ok(None),
        };
        // Whole-signal LHS only: `resolve_signal_id` rejects selects and
        // arrays (the latter are registered as refs to `Array` nodes, which
        // carry no `SignalInfo`).
        let (_, info) = match self.resolve_signal_id(path, lhs) {
            Ok(g) => g,
            Err(_) => return Ok(None),
        };
        // The RHS is a constant expression after elaboration; try a plain
        // constant first, then constant-foldable operations/params.  Anything
        // non-constant falls through to the emission error path.
        let c = match self.const_of_node(rhs) {
            Ok(c) => c,
            Err(_) => match self.eval_decl_value(rhs) {
                Ok(v) => decl_value_to_const(v)?,
                Err(_) => return Ok(None),
            },
        };
        Ok(Some((info, c)))
    }

    /// The declaration-initializer constant of a scalar VARIABLE whose init
    /// is attached to the variable (`logic l = 1'b0;`, `int x = 5;`). The RHS
    /// is a constant expression after elaboration: try a plain constant
    /// first, then constant-foldable operations/params via `eval_bits` (which
    /// resolves parameter references through `param_vals`).  Anything
    /// non-constant is rejected because variable initializers must be constant
    /// expressions.
    pub(in super::super) fn var_decl_init(
        &self,
        path: &str,
        name: &str,
        init: NodeId,
    ) -> Result<IrConst, String> {
        match self.const_of_node(init) {
            Ok(c) => Ok(c),
            Err(_) => match self.eval_decl_value(init) {
                Ok(v) => decl_value_to_const(v),
                Err(_) => Err(format!(
                    "variable initializer is not a constant expression in `{name}` in `{path}`"
                )),
            },
        }
    }

    /// The constant operands of an assignment-pattern (`'{…}`) initializer
    /// expression, in linear-index order.
    pub(super) fn array_init_consts(
        &self,
        path: &str,
        name: &str,
        init: NodeId,
    ) -> Result<Vec<IrConst>, String> {
        let operands: Vec<NodeId> = match self.kind(init) {
            NodeKind::Expr(ExprKind::Operation { op, operands, .. })
                if *op == Operation::AssignmentPattern =>
            {
                operands.clone()
            }
            other => {
                return Err(format!(
                    "array `{name}` in `{path}` has an unsupported declaration \
                     initializer: {other:?}"
                ))
            }
        };
        operands
            .iter()
            .map(|operand| {
                self.const_of_node(*operand)
                    .or_else(|_| self.eval_decl_value(*operand).and_then(decl_value_to_const))
                    .map_err(|_| {
                        format!(
                            "array `{name}` in `{path}`: initializer element is not a \
                             supported constant expression ({:?})",
                            self.kind(*operand)
                        )
                    })
            })
            .collect()
    }
}
