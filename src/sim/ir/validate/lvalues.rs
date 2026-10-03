//! Lvalues.

use super::*;

impl Validator<'_> {
    pub(super) fn validate_elem_sel(
        &self,
        sel: &IrElemSel,
        formals: &[IrFormal],
        path: &str,
    ) -> ValidationResult {
        match sel {
            IrElemSel::PackedChain(steps) => {
                if steps.is_empty() {
                    return self.fail(path, "packed selection chain must not be empty");
                }
                for (index, step) in steps.iter().enumerate() {
                    let path = format!("{path}.steps[{index}]");
                    self.validate_width(step.width, &format!("{path}.width"))?;
                    if step.base.is_real() {
                        return self
                            .fail(format!("{path}.base"), "packed selector must be integral");
                    }
                    self.validate_expr(&step.base, formals, &format!("{path}.base"))?;
                }
                Ok(())
            }
            IrElemSel::Whole => Ok(()),
            IrElemSel::Part(left, right) => self.validate_select_width(*left, *right, path),
            IrElemSel::Bit(expr) => self.validate_expr(expr, formals, path),
            IrElemSel::Indexed { base, width, .. } => {
                self.validate_expr(base, formals, &format!("{path}.base"))?;
                self.validate_width(*width, &format!("{path}.width"))
            }
        }
    }

    pub(super) fn validate_select_width(
        &self,
        left: i64,
        right: i64,
        _path: &str,
    ) -> ValidationResult {
        let width = (i128::from(left) - i128::from(right)).unsigned_abs() + 1;
        self.max_width.set(self.max_width.get().max(width));
        Ok(())
    }

    pub(super) fn lhs_packed_width(&self, lhs: &IrLhs) -> Option<u32> {
        let width = match lhs {
            IrLhs::PackedSelect { steps, .. } => steps.last()?.width,
            IrLhs::TaggedSelect { steps, .. } => steps.last()?.selection.width,
            IrLhs::Whole(signal) => self.model.signals.get(*signal)?.ty.width(),
            IrLhs::WholeRef { width, .. }
            | IrLhs::Ref { width, .. }
            | IrLhs::Stream { width, .. } => *width,
            IrLhs::Bit(..) => 1,
            IrLhs::Part(_, left, right, _) => ((left - right).abs() + 1) as u32,
            IrLhs::IdxPart(_, _, _, width, _, _) => *width,
            IrLhs::ArrayElem { arr, elem_sel, .. } => match elem_sel {
                IrElemSel::Whole => self.model.arrays.get(*arr)?.elem_width,
                IrElemSel::Part(left, right) => ((left - right).abs() + 1) as u32,
                IrElemSel::Bit(_) => 1,
                IrElemSel::Indexed { width, .. } => *width,
                IrElemSel::PackedChain(steps) => steps.last().map_or(0, |step| step.width),
            },
        };
        (width != 0).then_some(width)
    }

    pub(super) fn validate_stochastic_output(
        &self,
        lhs: &IrLhs,
        formals: &[IrFormal],
        path: &str,
    ) -> ValidationResult {
        self.validate_lhs(lhs, formals, path)?;
        match lhs {
            IrLhs::Whole(signal) => {
                let signal = &self.model.signals[*signal];
                if signal.net_driver.is_some() || !matches!(signal.ty, IrType::Packed { .. }) {
                    return self.fail(
                        path,
                        "stochastic queue output must be whole packed variable storage",
                    );
                }
            }
            IrLhs::WholeRef { width, .. } if *width != 0 => {}
            _ => {
                return self.fail(
                    path,
                    "stochastic queue output must be whole packed variable storage",
                )
            }
        }
        Ok(())
    }

    pub(super) fn validate_lhs(
        &self,
        lhs: &IrLhs,
        formals: &[IrFormal],
        path: &str,
    ) -> ValidationResult {
        match lhs {
            IrLhs::PackedSelect { target, steps, .. } => {
                self.validate_lhs(target, formals, &format!("{path}.target"))?;
                if self.lhs_packed_width(target).is_none()
                    || !matches!(
                        target.as_ref(),
                        IrLhs::Whole(_)
                            | IrLhs::WholeRef { width: 1.., .. }
                            | IrLhs::Ref { bit: None, .. }
                            | IrLhs::ArrayElem {
                                elem_sel: IrElemSel::Whole,
                                ..
                            }
                            | IrLhs::Stream { .. }
                    )
                {
                    return self.fail(
                        path,
                        "packed activation select requires an unselected packed root",
                    );
                }
                self.validate_elem_sel(&IrElemSel::PackedChain(steps.clone()), formals, path)?;
            }
            IrLhs::TaggedSelect { target, steps, .. } => {
                self.validate_lhs(target, formals, &format!("{path}.target"))?;
                if self.lhs_packed_width(target).is_none()
                    || !matches!(
                        target.as_ref(),
                        IrLhs::Whole(_)
                            | IrLhs::WholeRef { width: 1.., .. }
                            | IrLhs::Ref { bit: None, .. }
                            | IrLhs::ArrayElem {
                                elem_sel: IrElemSel::Whole,
                                ..
                            }
                            | IrLhs::Stream { .. }
                    )
                {
                    return self.fail(
                        path,
                        "tagged member select requires an unselected packed root",
                    );
                }
                if steps.is_empty() {
                    return self.fail(path, "tagged member select has no projection steps");
                }
                let mut parent_width = self.lhs_packed_width(target).expect("checked root");
                for (index, step) in steps.iter().enumerate() {
                    let step_path = format!("{path}.steps[{index}]");
                    self.validate_expr(
                        &step.selection.base,
                        formals,
                        &format!("{step_path}.base"),
                    )?;
                    self.validate_width(step.selection.width, &format!("{step_path}.width"))?;
                    if step.selection.base.is_real() || step.selection.width > parent_width {
                        return self.fail(&step_path, "invalid packed tagged-member projection");
                    }
                    if let Some(guard) = &step.guard {
                        if guard.tag_width == 0 || guard.tag_width > parent_width {
                            return self
                                .fail(&step_path, "tagged-union guard width exceeds its receiver");
                        }
                        if guard.member_name.is_empty()
                            || (guard.tag_width < 32
                                && guard.member_index >= (1u32 << guard.tag_width))
                        {
                            return self.fail(&step_path, "invalid tagged-union guard metadata");
                        }
                    }
                    parent_width = step.selection.width;
                }
            }
            IrLhs::Whole(signal) | IrLhs::Part(signal, ..) => {
                if *signal >= self.model.signals.len() {
                    return self.fail(path, format!("signal index {signal} is out of bounds"));
                }
                if let IrLhs::Part(_, left, right, _) = lhs {
                    self.validate_select_width(*left, *right, path)?;
                }
            }
            IrLhs::WholeRef { width, .. } => {
                if *width != 0 {
                    self.validate_width(*width, path)?;
                }
            }
            IrLhs::Ref {
                addr,
                width,
                const_ref,
                bit,
                signed,
                ..
            } => {
                if addr.is_empty() {
                    return self.fail(path, "reference descriptor address must not be empty");
                }
                if *const_ref {
                    return self.fail(path, "const reference cannot be an assignment target");
                }
                self.validate_width(*width, path)?;
                if let Some(index) = bit {
                    if *width != 1 || *signed {
                        return self.fail(path, "reference bit select must be one unsigned bit");
                    }
                    self.validate_expr(index, formals, &format!("{path}.index"))?;
                }
            }
            IrLhs::Bit(signal, index, _) => {
                if *signal >= self.model.signals.len() {
                    return self.fail(path, format!("signal index {signal} is out of bounds"));
                }
                self.validate_expr(index, formals, &format!("{path}.index"))?;
            }
            IrLhs::IdxPart(signal, base, width_expr, width, _, _) => {
                if *signal >= self.model.signals.len() {
                    return self.fail(path, format!("signal index {signal} is out of bounds"));
                }
                self.validate_expr(base, formals, &format!("{path}.base"))?;
                self.validate_expr(width_expr, formals, &format!("{path}.width_expr"))?;
                self.validate_width(*width, &format!("{path}.selected_width"))?;
            }
            IrLhs::ArrayElem {
                arr,
                indices,
                elem_sel,
            } => {
                self.validate_fixed_activation(*arr, path)?;
                let array = self.model.arrays.get(*arr).ok_or_else(|| {
                    IrValidationError::new(path, format!("array index {arr} is out of bounds"))
                })?;
                if indices.len() != array.dims.len() {
                    return self.fail(path, "array LHS index count does not match its dimensions");
                }
                for (idx, index) in indices.iter().enumerate() {
                    self.validate_expr(index, formals, &format!("{path}.indices[{idx}]"))?;
                }
                self.validate_elem_sel(elem_sel, formals, &format!("{path}.elem_sel"))?;
                if array.real && matches!(elem_sel, IrElemSel::PackedChain(_)) {
                    return self.fail(
                        path,
                        "packed selection chain requires packed array elements",
                    );
                }
            }
            IrLhs::Stream {
                parts,
                width,
                slice,
                ..
            } => {
                self.validate_width(*width, &format!("{path}.width"))?;
                if *slice == 0 || *slice > *width {
                    return self.fail(path, "streaming LHS slice must be in 1..=its packed width");
                }
                if parts.is_empty() {
                    return self.fail(path, "streaming LHS must contain at least one target");
                }
                let mut total = 0u32;
                for (index, (part, part_width)) in parts.iter().enumerate() {
                    self.validate_width(*part_width, &format!("{path}.parts[{index}].width"))?;
                    self.validate_lhs(part, formals, &format!("{path}.parts[{index}]"))?;
                    if self.lhs_packed_width(part) != Some(*part_width) {
                        return self.fail(
                            format!("{path}.parts[{index}].width"),
                            "streaming LHS part width disagrees with its target",
                        );
                    }
                    total = total.checked_add(*part_width).ok_or_else(|| {
                        IrValidationError::new(path, "streaming LHS width sum overflows u32")
                    })?;
                }
                if total != *width {
                    return self.fail(
                        path,
                        format!(
                            "streaming LHS part widths sum to {total}, not declared width {width}"
                        ),
                    );
                }
            }
        }
        Ok(())
    }
}
