//! Values.

use super::*;

impl Validator<'_> {

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
}
