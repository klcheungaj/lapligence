//! Constants.

use super::*;

/// A concrete constant value: LSB-indexed 64-bit limbs (bit `i` lives in
/// `bits[i / 64]` at position `i % 64`, matching the runtime `sv4_t` layout),
/// with X and Z bits kept in separate limb arrays.  `real` carries the payload
/// of a real literal (packed limbs are then unused), and `fill` marks a bare
/// unsized fill literal (0/1/2=x/3=z).
#[derive(Clone, Debug)]
pub struct IrConst {
    pub(in crate::sim) bits: Vec<u64>,
    pub(in crate::sim) x: Vec<u64>,
    pub(in crate::sim) z: Vec<u64>,
    pub(in crate::sim) width: u32,
    pub(in crate::sim) signed: bool,
    pub(in crate::sim) real: Option<f64>,
    pub(in crate::sim) fill: Option<u8>,
}

impl IrConst {
    /// Construct and validate a packed four-state constant.
    pub fn packed(
        bits: Vec<u64>,
        x: Vec<u64>,
        z: Vec<u64>,
        width: u32,
        signed: bool,
        fill: Option<u8>,
    ) -> Result<Self, IrValidationError> {
        validate_width("const.width", width)?;
        if fill.is_some_and(|value| value > 3) {
            return Err(IrValidationError::new(
                "const.fill",
                "fill marker must be in 0..=3",
            ));
        }
        let limbs = width.div_ceil(64) as usize;
        for (name, values) in [("bits", &bits), ("x", &x), ("z", &z)] {
            if values.len() > limbs {
                return Err(IrValidationError::new(
                    format!("const.{name}"),
                    format!("{} limbs exceed the {limbs}-limb width", values.len()),
                ));
            }
        }
        for index in 0..limbs {
            if x.get(index).copied().unwrap_or(0) & z.get(index).copied().unwrap_or(0) != 0 {
                return Err(IrValidationError::new("const.x", "X and Z masks overlap"));
            }
        }
        if !width.is_multiple_of(64) {
            let outside = !((1u64 << (width % 64)) - 1);
            for (name, values) in [("bits", &bits), ("x", &x), ("z", &z)] {
                if values.get(limbs - 1).copied().unwrap_or(0) & outside != 0 {
                    return Err(IrValidationError::new(
                        format!("const.{name}"),
                        "high limb contains bits outside the declared width",
                    ));
                }
            }
        }
        Ok(Self {
            bits,
            x,
            z,
            width,
            signed,
            real: None,
            fill,
        })
    }

    /// Construct a real constant.
    pub fn real(value: f64) -> Self {
        Self {
            bits: Vec::new(),
            x: Vec::new(),
            z: Vec::new(),
            width: 0,
            signed: false,
            real: Some(value),
            fill: None,
        }
    }

    pub fn bits(&self) -> &[u64] {
        &self.bits
    }
    pub fn x_mask(&self) -> &[u64] {
        &self.x
    }
    pub fn z_mask(&self) -> &[u64] {
        &self.z
    }
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn signed(&self) -> bool {
        self.signed
    }
    pub fn real_value(&self) -> Option<f64> {
        self.real
    }
    pub fn fill(&self) -> Option<u8> {
        self.fill
    }
}

impl PartialEq for IrConst {
    fn eq(&self, other: &Self) -> bool {
        self.bits == other.bits
            && self.x == other.x
            && self.z == other.z
            && self.width == other.width
            && self.signed == other.signed
            // NaN != NaN would make structurally identical NaN consts unequal.
            && match (self.real, other.real) {
                (Some(a), Some(b)) => a.to_bits() == b.to_bits(),
                (None, None) => true,
                _ => false,
            }
            && self.fill == other.fill
    }
}
