//! Force descriptors borrow only persistent targets; evaluators publish owners.
use super::stores::Selection;
use super::*;
use crate::IdSet;

impl Frame<'_, '_> {
    fn force_parts(
        &mut self,
        lhs: &IrLhs,
        offset: u32,
        parts: &mut Vec<String>,
    ) -> Result<(), String> {
        if let IrLhs::Stream {
            parts: children,
            width,
            ..
        } = lhs
        {
            let mut cursor = *width;
            for (child, width) in children {
                cursor = cursor
                    .checked_sub(*width)
                    .ok_or_else(|| "force stream width mismatch".to_owned())?;
                self.force_parts(
                    child,
                    offset
                        .checked_add(cursor)
                        .ok_or_else(|| "force stream offset overflow".to_owned())?,
                    parts,
                )?;
            }
            return Ok(());
        }
        if let IrLhs::Whole(index) | IrLhs::Bit(index, ..) | IrLhs::Part(index, ..) = lhs {
            let signal = self.ctx.model.signal(*index);
            if !signal.net_alias.is_empty() {
                let selected: Vec<(u32, u32)> = match lhs {
                    IrLhs::Whole(_) => signal
                        .net_alias
                        .iter()
                        .map(|b| (b.signal_bit(), b.signal_bit()))
                        .collect(),
                    IrLhs::Bit(_, expression, _) => vec![(
                        constant_alias_bit(expression).ok_or_else(|| {
                            "selected force on a net alias requires a constant index".to_owned()
                        })?,
                        0,
                    )],
                    IrLhs::Part(_, left, right, _) => {
                        let width = u32::try_from(left.abs_diff(*right) + 1)
                            .map_err(|_| "force width overflow".to_owned())?;
                        let step = if left <= right { 1 } else { -1 };
                        (0..width)
                            .map(|i| {
                                Ok((
                                    u32::try_from(left + i64::from(i) * step).map_err(|_| {
                                        "selected force on a net alias has an unmapped bit"
                                            .to_owned()
                                    })?,
                                    width - 1 - i,
                                ))
                            })
                            .collect::<Result<Vec<_>, String>>()?
                    }
                    _ => unreachable!(),
                };
                let two_state = signal.ty.two_state()
                    || match lhs {
                        IrLhs::Bit(_, _, state) | IrLhs::Part(_, _, _, state) => *state,
                        _ => false,
                    };
                let mut seen = IdSet::new();
                for (bit, source) in selected {
                    let binding = signal
                        .net_alias
                        .iter()
                        .find(|b| b.signal_bit() == bit)
                        .ok_or_else(|| {
                            "selected force on a net alias has an unmapped bit".to_owned()
                        })?;
                    if !seen.insert((binding.group(), binding.group_bit())) {
                        continue;
                    }
                    let group = &self.ctx.model.net_group(binding.group()).c_name;
                    let bit = binding.group_bit();
                    let source = offset
                        .checked_add(source)
                        .ok_or_else(|| "force offset overflow".to_owned())?;
                    parts.push(format!(
                        "{{ &{group}.resolved, &{group}, {bit}, {bit}, 1, {source}, {} }}",
                        u8::from(two_state)
                    ));
                }
                return Ok(());
            }
        }
        let (address, net, left, right, width, two_state) = match lhs {
            IrLhs::Whole(index) | IrLhs::Bit(index, ..) | IrLhs::Part(index, ..) => {
                let signal = self.ctx.model.signal(*index);
                if !signal.net_alias.is_empty() {
                    return Err(pending("true-net-alias force descriptors"));
                }
                if signal.ty.width() == 0 {
                    return Err("real force cannot be part of a packed force".to_owned());
                }
                let (address, net) = if let Some((group, _)) = signal.net_driver {
                    let name = &self.ctx.model.net_group(group).c_name;
                    (format!("&{name}.resolved"), format!("&{name}"))
                } else {
                    (format!("&{}", signal.c_name), "NULL".to_owned())
                };
                let (left, right, width, selected_two_state) = match lhs {
                    IrLhs::Bit(_, expression, two_state) => {
                        let value = self.expression(expression)?;
                        let index = self.scalar("int64_t", format!("sv4_to_i64({})", value.code));
                        self.discard(value);
                        (index.clone(), index, 1, *two_state)
                    }
                    IrLhs::Part(_, left, right, two_state) => (
                        format!("{left}LL"),
                        format!("{right}LL"),
                        (left.abs_diff(*right) + 1) as u32,
                        *two_state,
                    ),
                    _ => (
                        (signal.ty.width() - 1).to_string(),
                        "0".to_owned(),
                        signal.ty.width(),
                        false,
                    ),
                };
                (
                    address,
                    net,
                    left,
                    right,
                    width,
                    u8::from(signal.ty.two_state() || selected_two_state).to_string(),
                )
            }
            IrLhs::WholeRef {
                addr,
                width,
                two_state,
                ..
            } => {
                let binding = self.address(addr)?;
                if binding.automatic || *width == 0 {
                    return Err(pending("automatic or real indirect force targets"));
                }
                (
                    binding.address,
                    "NULL".to_owned(),
                    (width - 1).to_string(),
                    "0".to_owned(),
                    *width,
                    u8::from(*two_state).to_string(),
                )
            }
            // The cell the indices select when the statement executes, or
            // NULL for an invalid index (the runtime skips that part).
            IrLhs::ArrayElem {
                arr,
                indices,
                elem_sel,
            } => {
                match elem_sel {
                    IrElemSel::Whole => {
                        let (binding, _) = self.array_cell(*arr, indices, true)?;
                        if binding.width == 0 {
                            return Err("real force cannot be part of a packed force".to_owned());
                        }
                        (
                            binding.address,
                            "NULL".to_owned(),
                            format!("{}LL", binding.width - 1),
                            "0LL".to_owned(),
                            binding.width,
                            u8::from(binding.two_state).to_string(),
                        )
                    }
                    // A member of an element held as one packed image.
                    IrElemSel::PackedChain(steps) => {
                        let element = IrLhs::ArrayElem {
                            arr: *arr,
                            indices: indices.clone(),
                            elem_sel: IrElemSel::Whole,
                        };
                        self.force_selected_part(&element, steps, false)?
                    }
                    _ => return Err(pending("this array element force target descriptor")),
                }
            }
            // A member of an element held as one packed image, or an element
            // of a member array of a record held as one signal per leaf
            // (below `LLG_RECORD_COLUMN_MIN_CELLS` cells), found when the
            // statement executes.
            IrLhs::PackedSelect {
                target,
                steps,
                two_state,
                ..
            } => self.force_selected_part(target, steps, *two_state)?,
            _ => return Err(pending("this force target descriptor")),
        };
        parts.push(format!(
            "{{ {address}, {net}, {left}, {right}, {width}u, {offset}u, {two_state} }}"
        ));
        Ok(())
    }

    /// The force descriptor fields of a packed selection inside an array
    /// element held as one packed image, or inside a record held as one
    /// signal per leaf. The selection must lie wholly inside the element or
    /// one leaf when the statement executes; otherwise (an invalid index, SV
    /// 7.4.6) the target is NULL and the runtime skips the part.
    fn force_selected_part(
        &mut self,
        target: &IrLhs,
        steps: &[IrPackedSelect],
        select_two_state: bool,
    ) -> Result<(String, String, String, String, u32, String), String> {
        // A member of an array element held as one packed image, at bits
        // the selection chooses inside the element.
        if let IrLhs::ArrayElem {
            arr,
            indices,
            elem_sel: IrElemSel::Whole,
        } = target
        {
            let (binding, _) = self.array_cell(*arr, indices, true)?;
            if binding.width == 0 {
                return Err("real force cannot be part of a packed force".to_owned());
            }
            let Some(Selection::PackedChain(plan, width)) =
                self.selection(&IrElemSel::PackedChain(steps.to_vec()), binding.width)?
            else {
                return Err("element member selection lost its plan".to_owned());
            };
            let cell = self.scalar(
                "sv4_t*",
                format!("{plan}.count == {width}u ? {} : NULL", binding.address),
            );
            return Ok((
                cell,
                "NULL".to_owned(),
                format!("(int64_t){plan}.storage_lsb + {}LL", width - 1),
                format!("(int64_t){plan}.storage_lsb"),
                width,
                u8::from(binding.two_state || select_two_state).to_string(),
            ));
        }
        let IrLhs::Stream {
            parts: leaves,
            width: stream_width,
            ..
        } = target
        else {
            return Err(pending("this selected force target descriptor"));
        };
        let mut segments = Vec::with_capacity(leaves.len());
        let mut cursor = *stream_width;
        for (leaf, leaf_width) in leaves {
            cursor = cursor
                .checked_sub(*leaf_width)
                .ok_or_else(|| "force record leaf widths exceed the record".to_owned())?;
            let (index, right, leaf_two_state) = match leaf {
                IrLhs::Whole(index) => (*index, 0i64, false),
                IrLhs::Part(index, left, right, two_state)
                    if left >= right && left.abs_diff(*right) + 1 == u64::from(*leaf_width) =>
                {
                    (*index, *right, *two_state)
                }
                _ => return Err(pending("this record leaf force target descriptor")),
            };
            let signal = self.ctx.model.signal(index);
            if signal.net_driver.is_some() || !signal.net_alias.is_empty() || signal.ty.width() == 0
            {
                return Err(pending("this record leaf force target descriptor"));
            }
            segments.push((
                cursor,
                *leaf_width,
                format!("&{}", signal.c_name),
                right,
                signal.ty.two_state() || leaf_two_state,
            ));
        }
        let Some(Selection::PackedChain(plan, width)) =
            self.selection(&IrElemSel::PackedChain(steps.to_vec()), *stream_width)?
        else {
            return Err("record leaf selection lost its plan".to_owned());
        };
        let cell = self.declare("sv4_t*", "force_leaf", "NULL".to_owned());
        let right = self.declare("int64_t", "force_right", "0".to_owned());
        let state = self.declare("int", "force_state", "0".to_owned());
        for (start, leaf_width, address, leaf_right, leaf_two_state) in segments {
            self.line(format!(
                "if ({plan}.count == {width}u && {plan}.storage_lsb >= {start}u && {plan}.storage_lsb - {start}u <= {}u) {{",
                leaf_width.saturating_sub(width)
            ));
            self.line(format!("{cell} = {address};"));
            self.line(format!(
                "{right} = {leaf_right}LL + (int64_t)({plan}.storage_lsb - {start}u);"
            ));
            self.line(format!("{state} = {};", u8::from(leaf_two_state)));
            self.line("}");
        }
        let two_state = if select_two_state {
            "1".to_owned()
        } else {
            state
        };
        Ok((
            cell,
            "NULL".to_owned(),
            format!("{right} + {}LL", width - 1),
            right,
            width,
            two_state,
        ))
    }

    pub(super) fn force_task(
        &mut self,
        lhs: &IrLhs,
        evaluator: Option<&str>,
        reads: &[usize],
        dependencies: &[IrDependency],
    ) -> Result<(), String> {
        let mut read_values = Vec::new();
        for index in reads {
            let signal = self.ctx.model.signal(*index);
            let pointer = if signal.net_alias.is_empty() {
                format!("&{}", signal.c_name)
            } else {
                format!("&llg_net_alias_{index}.visible")
            };
            read_values.push(if signal.ty.width() == 0 {
                format!("{{ NULL, {pointer}, 1 }}")
            } else {
                format!("{{ {pointer}, NULL, 0 }}")
            });
        }
        // Array dependencies use the same change markers as event waits; the
        // runtime publishes them through the ordinary signal-write path.
        for dependency in dependencies {
            read_values.push(match dependency {
                IrDependency::ArrayElement { array, index } => {
                    let array = self.ctx.model.array(*array);
                    if array.sparse() {
                        format!("{{ {}, NULL, 0 }}", array.cell_address(&index.to_string()))
                    } else if array.real {
                        format!("{{ NULL, &{}[{index}], 1 }}", array.c_name)
                    } else {
                        format!("{{ &{}_llg_element_deps[{index}], NULL, 0 }}", array.c_name)
                    }
                }
                IrDependency::ArrayContents(array) => format!(
                    "{{ &{}_llg_contents_dep, NULL, 0 }}",
                    self.ctx.model.array(*array).c_name
                ),
                _ => return Err("force dependency has no fixed-array change marker".to_owned()),
            });
        }
        let read_count = read_values.len();
        let read_ptr = if read_values.is_empty() {
            "NULL".to_owned()
        } else {
            self.declare_array_init(
                "llg_force_read_t",
                "force_reads",
                read_values.len(),
                &read_values.join(", "),
            )
        };
        // A real array element: the cell the index selects at the statement,
        // and no force or release for an invalid index (SV 7.4.6).
        if let IrLhs::ArrayElem {
            arr,
            indices,
            elem_sel: IrElemSel::Whole,
        } = lhs
        {
            if self.ctx.model.array(*arr).real {
                let (binding, _) = self.array_cell(*arr, indices, true)?;
                let cell = binding.address;
                self.line(if let Some(evaluator) = evaluator {
                    format!("if ({cell}) llg_force_real({cell}, {evaluator}, {read_ptr}, {read_count});")
                } else {
                    format!("if ({cell}) llg_release_real({cell});")
                });
                return Ok(());
            }
        }
        if let IrLhs::Whole(index) = lhs {
            let signal = self.ctx.model.signal(*index);
            if signal.ty.width() == 0 {
                self.line(if let Some(evaluator) = evaluator {
                    format!(
                        "llg_force_real(&{}, {evaluator}, {read_ptr}, {read_count});",
                        signal.c_name
                    )
                } else {
                    format!("llg_release_real(&{});", signal.c_name)
                });
                return Ok(());
            }
        }
        let mut parts = Vec::new();
        self.force_parts(lhs, 0, &mut parts)?;
        if parts.is_empty() {
            return Err("force target has no packed parts".to_owned());
        }
        let name = self.declare_array_init(
            "llg_force_part_t",
            "force_parts",
            parts.len(),
            &parts.join(", "),
        );
        let (slice, reverse) = match lhs {
            IrLhs::Stream {
                slice, direction, ..
            } => (*slice, *direction == IrStreamDirection::RightToLeft),
            _ => (0, false),
        };
        self.line(if let Some(evaluator) = evaluator {
            format!(
                "llg_force_expr_parts({name}, {}, {slice}, {}, {evaluator}, {read_ptr}, {read_count});",
                parts.len(),
                u8::from(reverse)
            )
        } else {
            format!(
                "llg_release_parts({name}, {}, {slice}, {});",
                parts.len(),
                u8::from(reverse)
            )
        });
        Ok(())
    }
}

// Alias bit identity is elaborated; never render a selector twice.
fn constant_alias_bit(expr: &IrExpr) -> Option<u32> {
    let IrExprKind::Const(value) = expr.kind() else {
        return None;
    };
    if value.real_value().is_some()
        || value.fill().is_some()
        || value.x_mask().iter().any(|m| *m != 0)
        || value.z_mask().iter().any(|m| *m != 0)
        || value.bits().iter().skip(1).any(|m| *m != 0)
    {
        return None;
    }
    value.bits().first().copied()?.try_into().ok()
}
