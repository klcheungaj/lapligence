//! Captured lvalues and ordered, borrowing runtime writes.

use super::*;

pub(super) enum Selection {
    Bit(String),
    Part(i64, i64),
    Indexed(Value, u32, bool),
}

pub(super) struct Target {
    pub binding: Binding,
    pub valid: String,
    pub selection: Option<Selection>,
    pub width: u32,
    pub signed: bool,
    pub net: Option<(String, usize)>,
    pub sequence_local: bool,
    pub reference: Option<String>,
}

impl<'a, 'm> Frame<'a, 'm> {
    fn index(&mut self, expr: &IrExpr) -> Result<String, String> {
        let value = self.expression(expr)?;
        let result = self.scalar("uint64_t", format!("sv4_to_index({})", value.code));
        self.discard(value);
        Ok(result)
    }

    fn array_cell(&mut self, array: usize, indices: &[IrExpr]) -> Result<(Binding, String), String> {
        let array = self.ctx.model.array(array);
        if indices.len() != array.dims.len() { return Err("array index rank mismatch".to_owned()); }
        let mut conditions = Vec::new();
        let mut offsets = Vec::new();
        // Capture indices in source order. Invalid arithmetic uses offset zero;
        // even forming an out-of-bounds C pointer is avoided below.
        for (expr, (left, right)) in indices.iter().zip(&array.dims) {
            let value = self.expression(expr)?;
            let number = self.name("index");
            self.line(format!("int64_t {number} = 0;"));
            let valid = self.scalar("int", format!(
                "(sv4_to_index_i64({}, &{number}) && {number} >= {}LL && {number} <= {}LL)",
                value.code, left.min(right), left.max(right)));
            let offset = if left >= right { format!("({left}LL - {number})") }
                else { format!("({number} - {left}LL)") };
            offsets.push((format!("({valid} ? (uint64_t){offset} : 0ULL)"),
                (i64::from(*left) - i64::from(*right)).unsigned_abs() + 1));
            conditions.push(valid);
            self.discard(value);
        }
        let mut linear = "0ULL".to_owned();
        for (offset, extent) in offsets { linear = format!("(({linear}) * {extent}ULL + {offset})"); }
        let valid = if conditions.is_empty() { "1".to_owned() } else { conditions.join(" && ") };
        let pointer = self.scalar(if array.real { "double*" } else { "sv4_t*" },
            format!("({valid}) ? &{}[{linear}] : NULL", array.c_name));
        Ok((Binding { address: pointer, width: if array.real { 0 } else { array.elem_width },
            signed: array.signed, two_state: array.two_state, shortreal: array.shortreal,
            automatic: false }, valid))
    }

    fn selection(&mut self, selection: &IrElemSel) -> Result<Option<Selection>, String> {
        Ok(match selection {
            IrElemSel::Whole => None,
            IrElemSel::Bit(index) => Some(Selection::Bit(self.index(index)?)),
            IrElemSel::Part(left, right) => Some(Selection::Part(*left, *right)),
            IrElemSel::Indexed { base, width, negative } =>
                Some(Selection::Indexed(self.expression(base)?, *width, *negative)),
        })
    }

    pub(super) fn target(&mut self, lhs: &IrLhs) -> Result<Target, String> {
        let (binding, valid, selection, net) = match lhs {
            IrLhs::Whole(index) | IrLhs::Bit(index, ..) | IrLhs::Part(index, ..) | IrLhs::IdxPart(index, ..) => {
                let signal = self.ctx.model.signal(*index);
                if !signal.net_alias.is_empty() { return Err(pending("true-net-alias writes")); }
                let net = signal.net_driver.map(|(group, slot)| (self.ctx.model.net_group(group).c_name.clone(), slot));
                let address = if let Some((name, slot)) = &net { format!("{name}.drivers[{slot}]") }
                    else { format!("&{}", signal.c_name) };
                let mut binding = Binding { address, width: signal.ty.width(), signed: signal.ty.signed(),
                    two_state: signal.ty.two_state(), shortreal: matches!(signal.ty, IrType::Real { shortreal: true }), automatic: false };
                let selection = match lhs {
                    IrLhs::Bit(_, index, two_state) => {
                        binding.two_state |= *two_state;
                        Some(Selection::Bit(self.index(index)?))
                    }
                    IrLhs::Part(_, left, right, two_state) => {
                        binding.two_state |= *two_state;
                        Some(Selection::Part(*left, *right))
                    }
                    IrLhs::IdxPart(_, base, _, width, negative, two_state) => {
                        binding.two_state |= *two_state;
                        Some(Selection::Indexed(self.expression(base)?, *width, *negative))
                    }
                    _ => None,
                };
                (binding, "1".to_owned(), selection, net)
            }
            IrLhs::WholeRef { addr, width, signed, two_state, shortreal } => {
                let mut binding = self.address(addr)?;
                if binding.width != *width { return Err("local target storage width mismatch".to_owned()); }
                binding.signed = *signed;
                binding.two_state |= *two_state;
                binding.shortreal |= *shortreal;
                (binding, "1".to_owned(), None, None)
            }
            IrLhs::ArrayElem { arr, indices, elem_sel } => {
                let (binding, valid) = self.array_cell(*arr, indices)?;
                let selection = self.selection(elem_sel)?;
                (binding, valid, selection, None)
            }
            IrLhs::Ref { addr, width, signed, two_state, const_ref, bit } => {
                if *const_ref { return Err("cannot write a const reference".to_owned()); }
                let address = self.reference_address(addr)?;
                let selection = bit.as_ref().map(|index| self.index(index).map(Selection::Bit)).transpose()?;
                return Ok(Target {
                    binding: Binding { address: "NULL".to_owned(), width: *width, signed: *signed, two_state: *two_state, shortreal: false, automatic: true },
                    valid: "1".to_owned(), width: if selection.is_some() { 1 } else { *width },
                    signed: if selection.is_some() { false } else { *signed }, selection,
                    net: None, sequence_local: false, reference: Some(address),
                });
            }
            IrLhs::Stream { .. } => return Err(pending("streaming lvalues")),
        };
        let (width, signed) = match &selection {
            None => (binding.width, binding.signed),
            Some(Selection::Bit(_)) => (1, false),
            Some(Selection::Part(left, right)) => ((left.abs_diff(*right) + 1) as u32, false),
            Some(Selection::Indexed(_, width, _)) => (*width, false),
        };
        let sequence_local = matches!(lhs, IrLhs::WholeRef { addr, .. } if self.sequence_addresses.contains_key(addr));
        Ok(Target { binding, valid, selection, width, signed, net, sequence_local, reference: None })
    }

    pub(super) fn release_target(&mut self, target: Target) {
        if let Some(Selection::Indexed(base, ..)) = target.selection { self.discard(base); }
    }

    fn select_code(&self, target: &Target, source: &str) -> String {
        match &target.selection {
            None => format!("sv4_clone(&({source}))"),
            Some(Selection::Bit(index)) => format!("sv4_bit_select({source}, {index})"),
            Some(Selection::Part(left, right)) => format!("sv4_part_select({source}, {left}LL, {right}LL)"),
            Some(Selection::Indexed(base, width, neg)) => format!("sv4_idx_part_select_value({source}, {}, {width}, {})", base.code, u8::from(*neg)),
        }
    }

    pub(super) fn read_target(&mut self, target: &Target) -> Value {
        if let Some(reference) = &target.reference {
            let whole = self.value(format!("llg_ref_read({reference})"), target.binding.width, target.binding.signed);
            if target.selection.is_none() { return whole; }
            let code = self.select_code(target, &whole.code);
            return self.replace(whole, code, target.width, target.signed);
        }
        let code = if target.width == 0 { format!("({}) ? *({}) : 0.0", target.valid, target.binding.address) }
            else { format!("({}) ? {} : {}", target.valid,
                self.select_code(target, &format!("*({})", target.binding.address)),
                super::super::expressions::packed_default(target.width, target.signed, target.binding.two_state)) };
        self.value(code, target.width, target.signed)
    }

    pub(super) fn set_selected(&mut self, selection: &Selection, destination: &str, source: &str) {
        self.line(match selection {
            Selection::Bit(index) => format!("sv4_bit_select_set(&{destination}, {index}, {source});"),
            Selection::Part(left, right) => format!("sv4_part_select_set(&{destination}, {left}LL, {right}LL, {source});"),
            Selection::Indexed(base, width, neg) => format!("sv4_idx_part_select_set_value(&{destination}, {}, {width}, {}, {source});", base.code, u8::from(*neg)),
        });
    }

    pub(super) fn store(&mut self, target: &Target, value: Value, nba: bool, ticks: &str) -> Result<(), String> {
        let binding = &target.binding;
        if nba && binding.width == 0 && binding.automatic {
            return Err(pending("queued writes to automatic real cells"));
        }
        // A read-only evaluator may only write its own automatic frame. These
        // writes are private calculations, so they copy directly instead of
        // passing through `llg_ba`, which notifies waiters and is suppressed in
        // read-only scheduling regions.
        if self.read_only_callback {
            if !binding.automatic
                || nba
                || target.reference.is_some()
                || target.net.is_some()
                || target.sequence_local
            {
                return Err(pending(
                    "side-effect-capable evaluator expressions: callback helper writes visible state",
                ));
            }
            let value = self.convert(
                value,
                target.width,
                target.signed,
                binding.two_state,
                binding.shortreal,
            );
            match (&target.selection, target.width) {
                (None, 0) => self.line(format!(
                    "if ({}) {{ *({}) = {}; }}",
                    target.valid, binding.address, value.code
                )),
                (None, _) => self.line(format!(
                    "if ({}) {{ sv4_move({}, &{}); }}",
                    target.valid, binding.address, value.code
                )),
                (Some(selection), _) => {
                    let updated = self.value(
                        format!("sv4_clone({})", binding.address),
                        binding.width,
                        binding.signed,
                    );
                    self.set_selected(selection, &updated.code, &value.code);
                    self.line(format!(
                        "if ({}) {{ sv4_move({}, &{}); }}",
                        target.valid, binding.address, updated.code
                    ));
                    self.discard(updated);
                }
            }
            self.discard(value);
            return Ok(());
        }
        let value = self.convert(value, target.width, target.signed, binding.two_state, binding.shortreal);
        self.line(format!("if ({}) {{", target.valid));
        if let Some(reference) = &target.reference {
            if nba { return Err(pending("nonblocking writes through reference formals")); }
            match &target.selection {
                None => self.line(format!("llg_ref_write({reference}, {});", value.code)),
                Some(Selection::Bit(index)) => {
                    // target() already captured the checked native index once.
                    self.line(format!("llg_ref_write_bit({reference}, {index}, {});", value.code));
                }
                _ => return Err("invalid reference sub-selection".to_owned()),
            }
        } else if target.sequence_local {
            if nba { return Err("sequence locals cannot be nonblocking targets".to_owned()); }
            self.line(format!("llg_sequence_local_write({}, {});", binding.address, value.code));
        } else if target.width == 0 {
            let call = if nba { format!("llg_nba_d_after({}, {}, {ticks});", binding.address, value.code) }
                else { format!("llg_ba_d({}, {});", binding.address, value.code) };
            self.line(call);
        } else if let Some(selection) = &target.selection {
            let updated = self.value(format!("sv4_clone({})", binding.address), binding.width, binding.signed);
            self.set_selected(selection, &updated.code, &value.code);
            if nba {
                let mask = self.value(format!("sv4_zero({}, 0)", binding.width), binding.width, false);
                let ones = self.value(format!("sv4_fill(1, {}, 0)", target.width), target.width, false);
                self.set_selected(selection, &mask.code, &ones.code);
                self.discard(ones);
                self.line(if let Some((name, slot)) = &target.net {
                    format!("llg_nba_net_masked_after(&{name}, {slot}, {}, {}, {ticks});", updated.code, mask.code)
                } else { format!("llg_nba_masked({}, {}, {}, {ticks});", binding.address, updated.code, mask.code) });
                self.discard(mask);
            } else {
                self.line(if let Some((name, slot)) = &target.net {
                    format!("llg_net_write(&{name}, {slot}, {});", updated.code)
                } else { format!("llg_ba({}, {});", binding.address, updated.code) });
            }
            self.discard(updated);
        } else {
            self.line(match (&target.net, nba) {
                (Some((name, slot)), false) => format!("llg_net_write(&{name}, {slot}, {});", value.code),
                (Some((name, slot)), true) => format!("llg_nba_net_after(&{name}, {slot}, {}, {ticks});", value.code),
                (None, false) => format!("llg_ba({}, {});", binding.address, value.code),
                (None, true) => format!("llg_nba_after({}, {}, {ticks});", binding.address, value.code),
            });
        }
        self.line("}");
        self.discard(value);
        self.cancellation_check()
    }

    pub(super) fn array_read(&mut self, array: usize, indices: &[IrExpr], selection: &IrElemSel, _expr: &IrExpr) -> Result<Value, String> {
        let lhs = IrLhs::ArrayElem { arr: array, indices: indices.to_vec(), elem_sel: selection.clone() };
        let target = self.target(&lhs)?;
        let value = self.read_target(&target);
        self.release_target(target);
        Ok(value)
    }

    pub(super) fn mutation(&mut self, mutation: &IrMutationExpr, expr: &IrExpr) -> Result<Value, String> {
        let target = self.target(&mutation.lhs)?;
        self.bindings.push(HashMap::new());
        let current = if mutation.reads_current || mutation.post {
            let value = self.read_target(&target);
            let value = self.convert(value, mutation.current_width, mutation.current_signed, false, false);
            self.bindings.last_mut().expect("mutation scope").insert("_llg_mut_current".to_owned(), Binding {
                address: format!("&{}", value.code), width: value.width, signed: value.signed,
                two_state: false, shortreal: false, automatic: true });
            Some(value)
        } else { None };
        let value = self.expression(&mutation.value)?;
        let value = self.convert(value, target.width, target.signed, target.binding.two_state, target.binding.shortreal);
        // A mutation's yielded value is a snapshot, not a borrow of the target.
        let result = if mutation.post {
            self.read_binding(&self.lookup("_llg_mut_current").expect("post mutation snapshot"))
        } else if value.width == 0 { self.value(value.code.clone(), 0, value.signed) }
        else { self.value(format!("sv4_clone(&{})", value.code), value.width, value.signed) };
        self.store(&target, value, false, "0")?;
        if let Some(current) = current { self.discard(current); }
        self.bindings.pop();
        self.release_target(target);
        Ok(self.convert(result, expr.width, expr.signed, false, false))
    }
}
