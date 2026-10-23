//! Values.

use super::*;

impl Validator<'_> {
    pub(super) fn validate_storage_default(
        &self,
        value: &IrConst,
        width: u32,
        signed: bool,
        path: &str,
    ) -> ValidationResult {
        if width == 0 || value.width != width || value.signed != signed || value.real.is_some() {
            return self.fail(path, "fixed storage default has the wrong type");
        }
        self.validate_expr(
            &IrExpr::new(IrExprKind::Const(value.clone()), width, signed, None),
            &[],
            path,
        )
    }

    pub(super) fn validate_type(&self, ty: &IrType, path: &str) -> ValidationResult {
        if let IrType::Packed { width, .. } = ty {
            self.validate_width(*width, path)?;
        }
        Ok(())
    }

    pub(super) fn validate_width(&self, width: u32, path: &str) -> ValidationResult {
        if width == 0 {
            return self.fail(path, "packed width must be nonzero");
        }
        self.max_width
            .set(self.max_width.get().max(u128::from(width)));
        Ok(())
    }

    pub(super) fn validate_const(&self, value: &IrConst, path: &str) -> ValidationResult {
        if value.real.is_some() {
            if value.width != 0 {
                return self.fail(path, "real constant has a non-zero packed width");
            }
            if value.fill.is_some() {
                return self.fail(path, "real constant carries a packed fill marker");
            }
            return Ok(());
        }
        self.validate_width(value.width, &format!("{path}.width"))?;
        if value.fill.is_some_and(|fill| fill > 3) {
            return self.fail(format!("{path}.fill"), "fill marker must be in 0..=3");
        }
        let limbs = value.width.div_ceil(64) as usize;
        for (name, values) in [("bits", &value.bits), ("x", &value.x), ("z", &value.z)] {
            if values.len() > limbs {
                return self.fail(
                    format!("{path}.{name}"),
                    format!("{} limbs exceed the {limbs}-limb width", values.len()),
                );
            }
        }
        for idx in 0..limbs {
            let x = value.x.get(idx).copied().unwrap_or(0);
            let z = value.z.get(idx).copied().unwrap_or(0);
            if x & z != 0 {
                return self.fail(format!("{path}.x"), "X and Z masks overlap");
            }
        }
        let tail = value.width % 64;
        if tail != 0 {
            let outside = !((1u64 << tail) - 1);
            for (name, values) in [("bits", &value.bits), ("x", &value.x), ("z", &value.z)] {
                if values.get(limbs - 1).copied().unwrap_or(0) & outside != 0 {
                    return self.fail(
                        format!("{path}.{name}"),
                        "high limb contains bits outside the declared width",
                    );
                }
            }
        }
        Ok(())
    }

    /// One period of an aggregate conditional merge: a concrete packed
    /// default whose width divides `payload`, and a two-state leaf-start
    /// mask of that width with bit 0 set.
    pub(super) fn validate_leaf_merge(
        &self,
        leaves: &IrLeafMerge,
        payload: u32,
        path: &str,
    ) -> ValidationResult {
        let (default, starts) = (&leaves.default, &leaves.starts);
        self.validate_const(default, &format!("{path}.default"))?;
        self.validate_const(starts, &format!("{path}.starts"))?;
        if default.real.is_some()
            || default.fill.is_some()
            || default.width == 0
            || !payload.is_multiple_of(default.width)
        {
            return self.fail(
                format!("{path}.default"),
                "leaf merge default must be a concrete packed period whose width divides the payload",
            );
        }
        if starts.real.is_some()
            || starts.fill.is_some()
            || starts.width != default.width
            || starts.x.iter().chain(&starts.z).any(|limb| *limb != 0)
            || starts.bits.first().is_none_or(|limb| limb & 1 == 0)
        {
            return self.fail(
                format!("{path}.starts"),
                "leaf merge starts must be a known mask of the period width with bit 0 set",
            );
        }
        Ok(())
    }
}
