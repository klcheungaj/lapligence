//! Call-scoped packed borrows. Retained expression results remain owners.
use super::*;

pub(super) fn stable_expression(expr: &IrExpr) -> bool {
    match &expr.kind {
        IrExprKind::Const(_)
        | IrExprKind::Fill(_)
        | IrExprKind::SigRead(_)
        | IrExprKind::LocalRead(_)
        | IrExprKind::FormalRead(_) => true,
        IrExprKind::Bin { a, b, .. } | IrExprKind::RealBin { a, b, .. } => {
            stable_expression(a) && stable_expression(b)
        }
        IrExprKind::Un { a, .. }
        | IrExprKind::RealUn { a, .. }
        | IrExprKind::Convert { a }
        | IrExprKind::Resize { a }
        | IrExprKind::CastToPacked { a }
        | IrExprKind::CastToReal { a, .. }
        | IrExprKind::ToTwoState { a }
        | IrExprKind::PartSel { base: a, .. } => stable_expression(a),
        IrExprKind::BitSel { base, idx } => stable_expression(base) && stable_expression(idx),
        IrExprKind::IdxPartSel { base, base_idx, .. } => {
            stable_expression(base) && stable_expression(base_idx)
        }
        _ => false,
    }
}

impl Frame<'_, '_> {
    pub(super) fn reserve_reused(&self, value: &Value, width: u32, signed: bool) -> Value {
        Value {
            code: value.code.clone(),
            width,
            signed,
            fill: None,
            slot: value.slot,
            borrowed_address: None,
        }
    }

    fn borrow(code: String, width: u32, signed: bool) -> Value {
        Value {
            borrowed_address: Some(format!("&{code}")),
            code,
            width,
            signed,
            fill: None,
            slot: None,
        }
    }

    fn borrow_binding(binding: &Binding) -> Value {
        Value {
            code: format!("*({})", binding.address),
            width: binding.width,
            signed: binding.signed,
            fill: None,
            slot: None,
            borrowed_address: Some(binding.address.clone()),
        }
    }

    pub(super) fn own(&mut self, value: Value) -> Value {
        if value.width == 0 || value.slot.is_some() {
            return value;
        }
        let address = value
            .borrowed_address
            .as_ref()
            .cloned()
            .unwrap_or_else(|| format!("&({})", value.code));
        let mut owned = self.value(format!("sv4_clone({address})"), value.width, value.signed);
        owned.fill = value.fill;
        owned
    }

    pub(super) fn constant(&mut self, constant: &IrConst, borrow: bool) -> Value {
        self.constant_value(
            emit_const(constant),
            constant.width,
            constant.signed,
            borrow,
        )
    }

    pub(super) fn constant_value(
        &mut self,
        constructor: String,
        width: u32,
        signed: bool,
        borrow: bool,
    ) -> Value {
        if width > u64::BITS {
            if let Some(pool) = self.ctx.constants {
                let code = pool.intern(constructor, width, signed);
                let value = Self::borrow(code, width, signed);
                return if borrow { value } else { self.own(value) };
            }
        }
        self.value(constructor, width, signed)
    }

    pub(super) fn packed_fill(
        &mut self,
        fill: u8,
        width: u32,
        signed: bool,
        borrow: bool,
    ) -> Value {
        if self.ctx.constants.is_none() {
            return self.value(
                format!("sv4_fill({fill}, {width}, {})", u8::from(signed)),
                width,
                signed,
            );
        }
        let count = width.div_ceil(64) as usize;
        let mut words = vec![u64::MAX; count];
        if let Some(top) = words.last_mut() {
            if !width.is_multiple_of(64) {
                *top >>= 64 - width % 64;
            }
        }
        let zero = vec![0; count];
        let constant = IrConst {
            bits: if fill == 1 {
                words.clone()
            } else {
                zero.clone()
            },
            x: if fill == 2 {
                words.clone()
            } else {
                zero.clone()
            },
            z: if fill == 3 { words } else { zero },
            width,
            signed,
            real: None,
            fill: None,
        };
        self.constant(&constant, borrow)
    }

    /// The caller must consume the returned borrow before any later effect or yield.
    pub(super) fn operand(&mut self, expr: &IrExpr) -> Result<Value, String> {
        match &expr.kind {
            IrExprKind::Const(constant) => {
                let mut value = self.constant(constant, true);
                value.fill = constant.fill;
                Ok(value)
            }
            IrExprKind::Fill(fill) => {
                let mut value = self.packed_fill(*fill, expr.width, expr.signed, true);
                value.fill = Some(*fill);
                Ok(value)
            }
            IrExprKind::SigRead(index)
                if !self.sampled_reads
                    && self.callback_signal_overrides.is_empty()
                    && self.ctx.model.signal(*index).net_alias.is_empty()
                    && expr.width != 0 =>
            {
                Ok(Self::borrow(
                    self.ctx.model.signal(*index).c_name.clone(),
                    expr.width,
                    expr.signed,
                ))
            }
            IrExprKind::LocalRead(name) if !self.item_callback && expr.width != 0 => {
                let binding = self.resolve_lookup(name)?;
                Ok(Self::borrow_binding(&binding))
            }
            IrExprKind::FormalRead(index) if expr.width != 0 => {
                let binding = self
                    .formal_overrides
                    .last()
                    .and_then(|bindings| bindings.get(*index))
                    .cloned()
                    .or_else(|| {
                        let formal = self.ctx.func?.formals.get(*index)?;
                        (!formal.is_address())
                            .then(|| self.lookup(&format!("a{index}")))
                            .flatten()
                    });
                if let Some(binding) = binding {
                    Ok(Self::borrow_binding(&binding))
                } else {
                    self.expression(expr)
                }
            }
            IrExprKind::Convert { a }
            | IrExprKind::Resize { a }
            | IrExprKind::CastToPacked { a }
                if expr.width != 0
                    && expr.width == a.width
                    && expr.signed == a.signed
                    && a.fill.is_none() =>
            {
                self.operand(a)
            }
            _ => self.expression(expr),
        }
    }
}
