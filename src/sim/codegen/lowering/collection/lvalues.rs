//! Lvalues.

use super::*;

impl<'a> Codegen<'a> {
    /// Resolve a net/var/ref node to a global signal name (used by port
    /// links and event sensitivities).
    pub(in super::super) fn resolve_signal_id(
        &self,
        scope_path: &str,
        node: NodeId,
    ) -> Result<(String, SignalInfo), String> {
        let name = self.node(node).name.clone();
        match self.kind(node) {
            NodeKind::Net { .. } | NodeKind::Var { .. } => {
                if let Some(info) = self.signal_of(node) {
                    return Ok((info.global.clone(), info.clone()));
                }
            }
            NodeKind::Expr(ExprKind::Ref { target: Some(t) }) => {
                if let Some(info) = self.signal_of(*t) {
                    return Ok((info.global.clone(), info.clone()));
                }
            }
            NodeKind::Expr(ExprKind::HierPath { .. }) => {
                if let Some(info) = self.hier_path_signal(node) {
                    return Ok((info.global.clone(), info.clone()));
                }
            }
            _ => {}
        }
        Err(format!(
            "cannot resolve signal reference `{name}` in `{scope_path}`"
        ))
    }

    /// Resolve a select node's base arena node to its global signal.
    pub(in super::super) fn base_signal(
        &self,
        _scope_path: &str,
        base: NodeId,
    ) -> Result<(String, SignalInfo), String> {
        if let Some(target) = self
            .clocking_var_target(base)
            .or_else(|| self.db.is_clocking_var(base).then_some(base))
        {
            if let Some(info) = self.clocking_var_source_info(target) {
                return Ok((info.global.clone(), info.clone()));
            }
        }
        match self.kind(base) {
            NodeKind::Net { .. } | NodeKind::Var { .. } => {
                if let Some(info) = self.signal_of(base) {
                    return Ok((info.global.clone(), info.clone()));
                }
            }
            NodeKind::Expr(ExprKind::Ref { target: Some(t) }) => {
                if let Some(info) = self.signal_of(*t) {
                    return Ok((info.global.clone(), info.clone()));
                }
            }
            NodeKind::Expr(ExprKind::HierPath { .. }) => {
                if let Some(info) = self.hier_path_signal(base) {
                    return Ok((info.global.clone(), info.clone()));
                }
            }
            _ => {}
        }
        Err(format!(
            "cannot resolve base signal of select `{}`",
            self.node(base).name
        ))
    }

    /// Resolve a whole-signal assignment target by its bound declaration.
    fn resolve_lhs_target(
        &self,
        node: NodeId,
        target: Option<NodeId>,
    ) -> Result<(String, SignalInfo), String> {
        if let Some(t) = target {
            if let Some(clocking_target) = self
                .clocking_var_target(t)
                .or_else(|| self.db.is_clocking_var(t).then_some(t))
            {
                if let Some(info) = self.clocking_var_source_info(clocking_target) {
                    return Ok((info.global.clone(), info.clone()));
                }
            }
            if let Some(info) = self.signal_of(t) {
                return Ok((info.global.clone(), info.clone()));
            }
        }
        let name = self.node(node).name.clone();
        Err(format!("cannot resolve assignment target `{name}`"))
    }

    // ── LHS analysis ───────────────────────────────────────────────────────

    fn array_element_lhs(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<ArrayElemLhs>, String> {
        let base = match self.kind(node) {
            NodeKind::Expr(
                ExprKind::BitSelect { base, .. } | ExprKind::ArraySelect { base, .. },
            ) => *base,
            _ => return Ok(None),
        };
        if self.array_of(base).is_none() {
            return Ok(None);
        }
        match self.analyze_lhs(path, node)? {
            Lhs::ArrayElem(element) if matches!(element.elem_sel, ElemSel::Whole) => {
                Ok(Some(element))
            }
            _ => Err(format!(
                "nested selects of an array element are not supported in `{path}`"
            )),
        }
    }

    pub(in super::super) fn fixed_stream_lhs_parts(
        &mut self,
        path: &str,
        value: NodeId,
        with_node: Option<NodeId>,
    ) -> Result<Option<Vec<Lhs>>, String> {
        let Some(array) = self.array_of(value).cloned() else {
            return Ok(None);
        };
        if array.real {
            return Err(format!(
                "real array streaming assignment target is not supported in `{path}`"
            ));
        }
        let selected = match with_node {
            Some(with_node) => self
                .static_stream_selector_indices(path, with_node)?
                .ok_or_else(|| {
                    format!(
                        "runtime `with` selector on a fixed streaming target is not supported in `{path}`"
                    )
                })?,
            None => {
                let (left, right) = array.dims.first().copied().ok_or_else(|| {
                    format!("fixed streaming target has no dimensions in `{path}`")
                })?;
                let step = if left <= right { 1 } else { -1 };
                let mut values = Vec::new();
                let mut index = i128::from(left);
                loop {
                    values.push(index);
                    if index == i128::from(right) {
                        break;
                    }
                    index += i128::from(step);
                }
                values
            }
        };
        let rest = port_array_index_vectors(&array.dims[1..]);
        let mut parts = Vec::new();
        for index in selected {
            for suffix in &rest {
                let mut indices = Vec::with_capacity(1 + suffix.len());
                indices.push(lhs_integer_expr(index));
                indices.extend(
                    suffix
                        .iter()
                        .map(|value| lhs_integer_expr(i128::from(*value))),
                );
                parts.push(Lhs::ArrayElem(ArrayElemLhs {
                    arr: array.clone(),
                    indices,
                    elem_sel: ElemSel::Whole,
                }));
            }
        }
        if parts.is_empty() {
            return Err(format!("empty fixed streaming target in `{path}`"));
        }
        Ok(Some(parts))
    }

    pub(in super::super) fn analyze_lhs(&mut self, path: &str, lhs: NodeId) -> Result<Lhs, String> {
        if let Some(target) = self.packed_formal_lhs(path, lhs)? {
            return Ok(Lhs::Canonical(target));
        }
        if let Some(target) = self
            .clocking_var_target(lhs)
            .or_else(|| self.db.is_clocking_var(lhs).then_some(lhs))
        {
            let info = self
                .clocking_var_source_info(target)
                .cloned()
                .ok_or_else(|| {
                    format!(
                        "clocking member `{}` has no writable source in `{path}`",
                        self.node(target).name
                    )
                })?;
            return Ok(Lhs::Whole(info));
        }
        match self.kind(lhs) {
            NodeKind::Expr(ExprKind::Streaming {
                direction,
                slice_size,
                streams,
            }) => {
                if streams.is_empty() {
                    return Err(format!("empty streaming assignment target in `{path}`"));
                }
                let streams = streams.clone();
                let mut parts = Vec::new();
                for stream in streams {
                    if let Some(fixed_parts) =
                        self.fixed_stream_lhs_parts(path, stream.value, stream.with_expr)?
                    {
                        parts.extend(fixed_parts);
                        continue;
                    }
                    match self.kind(stream.value) {
                        NodeKind::Expr(ExprKind::Operation {
                            op: Operation::Concat,
                            reordered,
                            operands,
                            ..
                        }) => {
                            let mut operands = operands.clone();
                            if *reordered {
                                operands.reverse();
                            }
                            for operand in operands {
                                parts.push(self.analyze_lhs(path, operand)?);
                            }
                        }
                        _ => parts.push(self.analyze_lhs(path, stream.value)?),
                    }
                }
                Ok(Lhs::Stream {
                    parts,
                    slice: (*slice_size != 0).then_some(u128::from(*slice_size)),
                    direction: match direction {
                        DbStreamingDirection::LeftToRight => IrStreamDirection::LeftToRight,
                        DbStreamingDirection::RightToLeft => IrStreamDirection::RightToLeft,
                    },
                })
            }
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Concat,
                reordered,
                operands,
                ..
            }) => {
                if operands.is_empty() {
                    return Err(format!("empty concatenation assignment target in `{path}`"));
                }
                let mut operands = operands.clone();
                if *reordered {
                    operands.reverse();
                }
                let parts = operands
                    .into_iter()
                    .map(|operand| self.analyze_lhs(path, operand))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Lhs::Stream {
                    parts,
                    slice: Some(1),
                    direction: IrStreamDirection::LeftToRight,
                })
            }
            NodeKind::Var { .. } => {
                if let Some(info) = self.proc_local_info(lhs) {
                    if let Some(signal) = &info.static_signal {
                        return Ok(Lhs::Whole(signal.clone()));
                    }
                    return Ok(Lhs::WholeRef {
                        addr: format!("&{}", info.c_name),
                        width: info.width,
                        signed: info.signed,
                        two_state: info.two_state,
                        shortreal: false,
                    });
                }
                let name = self.node(lhs).name.clone();
                self.func_write_target(lhs, &name).ok_or_else(|| {
                    if self.is_const_ref_target(lhs, &name) {
                        format!("cannot write through const ref `{name}` in `{path}`")
                    } else {
                        format!("cannot resolve procedural variable `{name}` in `{path}`")
                    }
                })
            }
            NodeKind::Expr(ExprKind::Ref { target }) => {
                if let Some((_, info)) = self.lexical_proc_local(lhs) {
                    if let Some(signal) = &info.static_signal {
                        return Ok(Lhs::Whole(signal.clone()));
                    }
                    return Ok(Lhs::WholeRef {
                        addr: format!("&{}", info.c_name),
                        width: info.width,
                        signed: info.signed,
                        two_state: info.two_state,
                        shortreal: false,
                    });
                }
                if let Some(t) = *target {
                    if let Some(info) = self.signal_of(t) {
                        return Ok(Lhs::Whole(info.clone()));
                    }
                    if !self.proc_local_is_shadowed(lhs) {
                        if let Some(info) = self.proc_local_info(t) {
                            if let Some(signal) = &info.static_signal {
                                return Ok(Lhs::Whole(signal.clone()));
                            }
                            return Ok(Lhs::WholeRef {
                                addr: format!("&{}", info.c_name),
                                width: info.width,
                                signed: info.signed,
                                two_state: info.two_state,
                                shortreal: false,
                            });
                        }
                    }
                    // Function/task body writes: output/inout formals, locals
                    // and the return variable (by arena node).
                    if let Some(lh) = self.func_write_target(t, "") {
                        return Ok(lh);
                    }
                    if self.is_const_ref_target(t, &self.node(lhs).name) {
                        return Err(format!(
                            "cannot write through const ref `{}` in `{path}`",
                            self.node(lhs).name
                        ));
                    }
                }
                let name = self.node(lhs).name.clone();
                if target.is_none() && !name.is_empty() {
                    // io_decls are not indexed, so formals resolve by name.
                    if let Some(lh) = self.func_write_target(NodeId(0), &name) {
                        return Ok(lh);
                    }
                    if self.is_const_ref_target(NodeId(0), &name) {
                        return Err(format!(
                            "cannot write through const ref `{name}` in `{path}`"
                        ));
                    }
                }
                let (name, info) = self.resolve_lhs_target(lhs, *target)?;
                Ok(Lhs::Whole(SignalInfo {
                    global: name,
                    ..info
                }))
            }
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                if let Some(lhs) = self.packed_element_lhs_ir(path, lhs)? {
                    return Ok(Lhs::Canonical(lhs));
                }
                let target = match self.kind(*base) {
                    NodeKind::Expr(ExprKind::Ref {
                        target: Some(target),
                    }) => *target,
                    _ => *base,
                };
                if let Some(Lhs::Ref {
                    addr,
                    two_state,
                    const_ref,
                    ..
                }) = self.func_write_target(target, "")
                {
                    let index = self.lower_packed_index(path, *base, *index)?;
                    return Ok(Lhs::Canonical(IrLhs::Ref {
                        addr,
                        width: 1,
                        signed: false,
                        two_state,
                        const_ref,
                        bit: Some(Box::new(index)),
                    }));
                }
                if let Some(mut element) = self.array_element_lhs(path, *base)? {
                    if element.arr.real {
                        return Err(format!(
                            "select on a real array element in `{path}` is not supported"
                        ));
                    }
                    element.elem_sel = ElemSel::Bit(self.lower_packed_index(path, *base, *index)?);
                    return Ok(Lhs::ArrayElem(element));
                }
                if let Some((info, member, lsb, width)) =
                    self.packed_member_select_info(*base, &[*index])?
                {
                    return Ok(Lhs::Part(
                        info,
                        i128::from(lsb) + i128::from(width) - 1,
                        i128::from(lsb),
                        member.two_state,
                    ));
                }
                if let Some((info, member)) = self.packed_member_info(*base) {
                    let index = self.eval_bound_i128(*index)?;
                    let relative = self.aggregate_member_relative_bound(
                        &member.name,
                        &member.packed_ranges,
                        index,
                    )?;
                    return Ok(Lhs::Bit(
                        info,
                        lhs_integer_expr(i128::from(member.lsb) + i128::from(relative)),
                        member.two_state,
                    ));
                }
                if let Some(ai) = self.array_of(*base).cloned() {
                    if ai.dims.len() != 1 {
                        return Err(format!(
                            "array slice access (`{}[...]` on a {}-dimensional array) \
                             is not supported in `{path}`",
                            self.node(*base).name,
                            ai.dims.len()
                        ));
                    }
                    let index = self.lower_expr(path, *index)?;
                    return Ok(Lhs::ArrayElem(ArrayElemLhs {
                        arr: ai,
                        indices: vec![index],
                        elem_sel: ElemSel::Whole,
                    }));
                }
                if let Some((info, lsb, width)) = self.packed_select_info(*base, &[*index])? {
                    if width > 1 {
                        let right = i128::from(lsb);
                        let two_state = info.two_state;
                        return Ok(Lhs::Part(
                            info,
                            right + i128::from(width) - 1,
                            right,
                            two_state,
                        ));
                    }
                }
                let (_, info) = self.base_signal(path, *base)?;
                if info.real {
                    return Err(format!(
                        "select on real-valued signal in `{path}` is not supported"
                    ));
                }
                let index = self.lower_packed_index(path, *base, *index)?;
                let two_state = info.two_state;
                Ok(Lhs::Bit(info, index, two_state))
            }
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                if let Some(lhs) = self.packed_element_lhs_ir(path, lhs)? {
                    return Ok(Lhs::Canonical(lhs));
                }
                if let Some((_target, _kind, member_info)) = self.unpacked_member_info(lhs) {
                    let member = member_info.member;
                    let info = member_info.signal.ok_or_else(|| {
                        format!(
                            "aggregate member `{}` is not a packed assignment target",
                            member.name
                        )
                    })?;
                    if info.real {
                        return Ok(Lhs::Whole(info));
                    }
                    let width = member.ty.width.ok_or_else(|| {
                        format!("unpacked member `{}` has unresolved width", member.name)
                    })?;
                    return Ok(Lhs::Part(info, i128::from(width - 1), 0, member.two_state));
                }
                if let Some((info, member, lsb, width)) =
                    self.packed_member_select_info(*base, indices)?
                {
                    return Ok(Lhs::Part(
                        info,
                        i128::from(lsb) + i128::from(width) - 1,
                        i128::from(lsb),
                        member.two_state,
                    ));
                }
                if let Some((info, lsb, width)) = self.packed_select_info(*base, indices)? {
                    let right = i128::from(lsb);
                    let two_state = info.two_state;
                    return Ok(Lhs::Part(
                        info,
                        right + i128::from(width) - 1,
                        right,
                        two_state,
                    ));
                }
                let ai = self.array_of(*base).cloned().ok_or_else(|| {
                    format!(
                        "cannot resolve array base of select `{}` in `{path}`",
                        self.node(*base).name,
                    )
                })?;
                let ndims = ai.dims.len();
                if indices.len() == ndims {
                    let ies = indices
                        .iter()
                        .map(|i| self.lower_expr(path, *i))
                        .collect::<Result<Vec<_>, _>>()?;
                    return Ok(Lhs::ArrayElem(ArrayElemLhs {
                        arr: ai,
                        indices: ies,
                        elem_sel: ElemSel::Whole,
                    }));
                }
                if indices.len() == ndims + 1 {
                    if ai.real {
                        return Err(format!(
                            "select on a real array element in `{path}` is not supported"
                        ));
                    }
                    let last = *indices.last().expect("non-empty indices");
                    let ies = indices[..ndims]
                        .iter()
                        .map(|i| self.lower_expr(path, *i))
                        .collect::<Result<Vec<_>, _>>()?;
                    let elem_sel = match self.kind(last) {
                        NodeKind::Expr(ExprKind::PartSelect { left, right, .. }) => {
                            let l =
                                self.packed_relative_bound(*base, self.eval_bound_i128(*left)?)?;
                            let r =
                                self.packed_relative_bound(*base, self.eval_bound_i128(*right)?)?;
                            ElemSel::Part(l, r)
                        }
                        NodeKind::Expr(ExprKind::IndexedPartSelect {
                            base_expr,
                            width_expr,
                            neg,
                            ..
                        }) => {
                            let width = self.indexed_part_select_width(*width_expr, path)?;
                            ElemSel::Indexed(
                                self.lower_packed_index(path, *base, *base_expr)?,
                                width,
                                *neg ^ self.packed_range_ascending(*base),
                            )
                        }
                        _ => ElemSel::Bit(self.lower_packed_index(path, *base, last)?),
                    };
                    return Ok(Lhs::ArrayElem(ArrayElemLhs {
                        arr: ai,
                        indices: ies,
                        elem_sel,
                    }));
                }
                Err(format!(
                    "array `{}` in `{path}`: {}-level select on a {}-dimensional \
                     array is not supported",
                    self.node(*base).name,
                    indices.len(),
                    ndims
                ))
            }
            NodeKind::Expr(ExprKind::PartSelect { base, left, right }) => {
                if let Some(lhs) = self.packed_element_lhs_ir(path, lhs)? {
                    return Ok(Lhs::Canonical(lhs));
                }
                if let Some((info, member, lsb, width)) = self.packed_member_range_info(
                    *base,
                    self.eval_bound_i128(*left)?,
                    self.eval_bound_i128(*right)?,
                )? {
                    return Ok(Lhs::Part(
                        info,
                        i128::from(lsb) + i128::from(width) - 1,
                        i128::from(lsb),
                        member.two_state,
                    ));
                }
                if let Some(mut element) = self.array_element_lhs(path, *base)? {
                    if element.arr.real {
                        return Err(format!(
                            "select on a real array element in `{path}` is not supported"
                        ));
                    }
                    element.elem_sel = ElemSel::Part(
                        self.packed_relative_bound(*base, self.eval_bound_i128(*left)?)?,
                        self.packed_relative_bound(*base, self.eval_bound_i128(*right)?)?,
                    );
                    return Ok(Lhs::ArrayElem(element));
                }
                if let Some((info, member)) = self.packed_member_info(*base) {
                    let left = self.aggregate_member_relative_bound(
                        &member.name,
                        &member.packed_ranges,
                        self.eval_bound_i128(*left)?,
                    )?;
                    let right = self.aggregate_member_relative_bound(
                        &member.name,
                        &member.packed_ranges,
                        self.eval_bound_i128(*right)?,
                    )?;
                    return Ok(Lhs::Part(
                        info,
                        i128::from(member.lsb) + i128::from(left),
                        i128::from(member.lsb) + i128::from(right),
                        member.two_state,
                    ));
                }
                let (_, info) = self.base_signal(path, *base)?;
                if info.real {
                    return Err(format!(
                        "select on real-valued signal in `{path}` is not supported"
                    ));
                }
                let (l, r) = (
                    self.packed_relative_bound(*base, self.eval_bound_i128(*left)?)?,
                    self.packed_relative_bound(*base, self.eval_bound_i128(*right)?)?,
                );
                let two_state = info.two_state;
                Ok(Lhs::Part(info, l, r, two_state))
            }
            NodeKind::Expr(ExprKind::IndexedPartSelect {
                base,
                base_expr,
                width_expr,
                neg,
            }) => {
                if let Some(lhs) = self.packed_element_lhs_ir(path, lhs)? {
                    return Ok(Lhs::Canonical(lhs));
                }
                if let Some(mut element) = self.array_element_lhs(path, *base)? {
                    if element.arr.real {
                        return Err(format!(
                            "select on a real array element in `{path}` is not supported"
                        ));
                    }
                    let width = self.indexed_part_select_width(*width_expr, path)?;
                    element.elem_sel = ElemSel::Indexed(
                        self.lower_packed_index(path, *base, *base_expr)?,
                        width,
                        *neg ^ self.packed_range_ascending(*base),
                    );
                    return Ok(Lhs::ArrayElem(element));
                }
                let (_, info) = self.base_signal(path, *base)?;
                if info.real {
                    return Err(format!(
                        "select on real-valued signal in `{path}` is not supported"
                    ));
                }
                let width = self.indexed_part_select_width(*width_expr, path)?;
                let ascending = self.packed_range_ascending(*base);
                let base = self.lower_packed_index(path, *base, *base_expr)?;
                let width_expr = self.lower_expr(path, *width_expr)?;
                let two_state = info.two_state;
                Ok(Lhs::IdxPart(
                    info,
                    base,
                    width_expr,
                    width,
                    *neg ^ ascending,
                    two_state,
                ))
            }
            NodeKind::Expr(ExprKind::HierPath { .. }) => {
                if let Some((_target, _kind, member_info)) = self.unpacked_member_info(lhs) {
                    let member = member_info.member;
                    let member_width = member.ty.width.ok_or_else(|| {
                        format!("unpacked member `{}` has unresolved width", member.name)
                    })?;
                    let info = member_info.signal.ok_or_else(|| {
                        format!(
                            "aggregate member `{}` is not a packed assignment target",
                            member.name
                        )
                    })?;
                    let two_state = member.two_state;
                    return Ok(if info.real {
                        Lhs::Whole(info)
                    } else {
                        Lhs::Part(info, i128::from(member_width - 1), 0, two_state)
                    });
                }
                if let Some((info, member)) = self.packed_member_info(lhs) {
                    let two_state = member.two_state;
                    return Ok(Lhs::Part(
                        info,
                        i128::from(member.lsb + member.width - 1),
                        i128::from(member.lsb),
                        two_state,
                    ));
                }
                // A whole-signal hierarchical WRITE (`m.data`, `tb.dut.sig`,
                // …) lowers to the resolved target signal's global, so
                // `llg_ba`/`llg_nba` (and the collapsed inout-net driver
                // path in `assign_statement`) apply unchanged. Selects are
                // represented by their own typed expression nodes and lower
                // through the corresponding arms above.
                let info = self.hier_path_signal(lhs).cloned().ok_or_else(|| {
                    format!(
                        "cannot resolve hierarchical assignment LHS `{}` in \
                             `{path}` (only plain per-instance signals are \
                             supported)",
                        self.node(lhs).name
                    )
                })?;
                Ok(Lhs::Whole(info))
            }
            _ => Err("unsupported assignment LHS".to_string()),
        }
    }
}
