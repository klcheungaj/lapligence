//! C literals, four-state initializers, and assignment conversions.

use super::names::escaped_char;
use crate::sim::ir::IrConst;

pub(super) fn c_string_literal(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        out.push_str(&escaped_char(ch));
    }
    out.push('"');
    out
}

/// The Verilog `timescale string for a value in fs (e.g. 1_000_000 fs →
/// "1ns").
pub(crate) fn fs_to_timescale_str(v: u64) -> String {
    const PAIRS: [(u64, &str); 18] = [
        (100_000_000_000_000_000, "100s"),
        (10_000_000_000_000_000, "10s"),
        (1_000_000_000_000_000, "1s"),
        (100_000_000_000_000, "100ms"),
        (10_000_000_000_000, "10ms"),
        (1_000_000_000_000, "1ms"),
        (100_000_000_000, "100us"),
        (10_000_000_000, "10us"),
        (1_000_000_000, "1us"),
        (100_000_000, "100ns"),
        (10_000_000, "10ns"),
        (1_000_000, "1ns"),
        (100_000, "100ps"),
        (10_000, "10ps"),
        (1_000, "1ps"),
        (100, "100fs"),
        (10, "10fs"),
        (1, "1fs"),
    ];
    for (val, s) in PAIRS {
        if val == v {
            return s.to_string();
        }
    }
    format!("{v}fs")
}

pub(crate) fn emit_real_literal(value: f64) -> String {
    if value.is_nan() {
        "NAN".to_string()
    } else if value.is_infinite() {
        if value.is_sign_negative() {
            "-INFINITY".to_string()
        } else {
            "INFINITY".to_string()
        }
    } else {
        format!("{value:.17e}")
    }
}

/// Render a lowered constant: `SV4_INIT(...)` up to 64 packed bits,
/// `sv4_from_limbs((uint64_t[]){...}, ...)` beyond, and the bare double
/// literal for real constants.
pub(crate) fn emit_const(c: &IrConst) -> String {
    if let Some(value) = c.real {
        return emit_real_literal(value);
    }
    let signed = if c.signed { 1 } else { 0 };
    if c.width <= 64 {
        let b = c.bits.first().copied().unwrap_or(0);
        let x = c.x.first().copied().unwrap_or(0);
        let z = c.z.first().copied().unwrap_or(0);
        format!("SV4_INIT({b}ULL, {x}ULL, {z}ULL, {}, {signed})", c.width)
    } else {
        let limbs = c.width.div_ceil(64) as usize;
        let mut bs = vec![0u64; limbs];
        let mut xs = vec![0u64; limbs];
        let mut zs = vec![0u64; limbs];
        let nb = c.bits.len();
        let nx = c.x.len();
        let nz = c.z.len();
        bs[..nb].copy_from_slice(&c.bits[..nb]);
        xs[..nx].copy_from_slice(&c.x[..nx]);
        zs[..nz].copy_from_slice(&c.z[..nz]);
        let b = bs
            .iter()
            .map(|v| format!("{v}ULL"))
            .collect::<Vec<_>>()
            .join(", ");
        let x = xs
            .iter()
            .map(|v| format!("{v}ULL"))
            .collect::<Vec<_>>()
            .join(", ");
        let z = zs
            .iter()
            .map(|v| format!("{v}ULL"))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "sv4_from_limbs((uint64_t[]){{{b}}}, (uint64_t[]){{{x}}}, \
             (uint64_t[]){{{z}}}, {}, {signed})",
            c.width
        )
    }
}

/// Narrowest unsigned packed constant that [`periodic_constant`] splits into
/// one period and a repetition count. Below it the limb tables are at most 64
/// words per plane; at and above it a whole-array constant (such as the
/// default value of a dense unpacked array formal) would otherwise spell
/// every element's bits into `model.c`.
pub(crate) const PERIODIC_CONSTANT_MIN_BITS: u32 = 4096;

/// Fewest repetitions of its period for which a constant is emitted as one
/// period and a replication; the same count as `REPEAT_VALUE_MIN_COUNT`.
const PERIODIC_CONSTANT_MIN_REPEATS: u32 = 4;

/// Bits `start..start + length` (`length <= 64`) of one limb plane; limbs
/// past the plane's end read as zero.
fn plane_window(plane: &[u64], start: usize, length: usize) -> u64 {
    let word = |index: usize| plane.get(index).copied().unwrap_or(0);
    let (index, shift) = (start / 64, start % 64);
    let mut value = word(index) >> shift;
    if shift != 0 {
        value |= word(index + 1) << (64 - shift);
    }
    if length < 64 {
        value &= (1u64 << length) - 1;
    }
    value
}

/// The shortest exact period of a wide unsigned packed constant, as that
/// period's constant and the number of repetitions, when the constant is at
/// least [`PERIODIC_CONSTANT_MIN_BITS`] wide and repeats its period at least
/// `PERIODIC_CONSTANT_MIN_REPEATS` times. Replicating the period
/// (`{count{period}}`, LSB-first) reproduces the constant exactly in all
/// three planes.
pub(crate) fn periodic_constant(c: &IrConst) -> Option<(IrConst, u32)> {
    if c.real.is_some() || c.signed || c.width < PERIODIC_CONSTANT_MIN_BITS {
        return None;
    }
    let width = c.width as usize;
    let planes = [&c.bits, &c.x, &c.z];
    let repeats = |period: usize| {
        // Periodic with `period` exactly when every bit equals the bit one
        // period below it.
        (period..width).step_by(64).all(|start| {
            let length = (width - start).min(64);
            planes.iter().all(|plane| {
                plane_window(plane, start, length) == plane_window(plane, start - period, length)
            })
        })
    };
    let max_period = width / PERIODIC_CONSTANT_MIN_REPEATS as usize;
    let period =
        (1..=max_period).find(|&period| width.is_multiple_of(period) && repeats(period))?;
    let pattern = |plane: &[u64]| {
        (0..period.div_ceil(64))
            .map(|word| plane_window(plane, word * 64, (period - word * 64).min(64)))
            .collect::<Vec<_>>()
    };
    let constant = IrConst {
        bits: pattern(&c.bits),
        x: pattern(&c.x),
        z: pattern(&c.z),
        width: u32::try_from(period).ok()?,
        signed: false,
        real: None,
        fill: None,
    };
    Some((constant, u32::try_from(width / period).ok()?))
}

/// Move the limb arrays of a multi-limb [`emit_const`] constructor to
/// file-scope `static const` tables named `{name}_bits`/`_x`/`_z`, appending
/// their definitions to `tables`, and return the constructor that reads them.
/// All-zero X/Z planes become `NULL` (zero-filled by `sv4_from_limbs`).
///
/// Compound-literal limb arrays have automatic storage, so a startup function
/// initializing many wide constants would otherwise hold every array in its
/// native frame at once (several hundred KiB for one maximum-width constant).
/// Any other constructor is returned unchanged.
pub(crate) fn hoist_const_limbs(constructor: &str, name: &str, tables: &mut String) -> String {
    let Some(rest) = constructor.strip_prefix("sv4_from_limbs(") else {
        return constructor.to_owned();
    };
    let mut planes = Vec::with_capacity(3);
    let mut rest = rest;
    for _ in 0..3 {
        let Some(after) = rest.strip_prefix("(uint64_t[]){") else {
            return constructor.to_owned();
        };
        let Some(end) = after.find('}') else {
            return constructor.to_owned();
        };
        planes.push(&after[..end]);
        rest = after[end + 1..].trim_start_matches(',').trim_start();
    }
    let Some(shape) = rest.strip_suffix(')') else {
        return constructor.to_owned();
    };
    let mut arguments = Vec::with_capacity(3);
    for (plane, suffix) in planes.iter().zip(["bits", "x", "z"]) {
        let zero = plane
            .split(',')
            .all(|limb| limb.trim() == "0ULL" || limb.trim().is_empty());
        if zero && suffix != "bits" {
            arguments.push("NULL".to_owned());
            continue;
        }
        let table = format!("{name}_{suffix}");
        tables.push_str(&format!("static const uint64_t {table}[] = {{{plane}}};\n"));
        arguments.push(table);
    }
    format!("sv4_from_limbs({}, {shape})", arguments.join(", "))
}

/// Render a constant converted to an explicit `(width, signed)` vector target:
/// real payloads go through `sv4_from_real` (at most 64 bits), everything else
/// through the value-preserving `sv4_cast` keyed on the constant's own
/// signedness (LRM §10.7 assignment padding).
#[allow(dead_code)] // legacy emitter helper retained until the owned-emission migration removes it
pub(crate) fn emit_const_for_vector(
    c: &IrConst,
    width: u32,
    signed: bool,
) -> Result<String, String> {
    Ok(match c.real {
        Some(value) => format!(
            "sv4_from_real({}, {}, {})",
            emit_real_literal(value),
            width,
            signed as u8
        ),
        None => format!("sv4_cast({}, {}, {})", emit_const(c), width, signed as u8),
    })
}

#[allow(dead_code)] // legacy emitter helper retained until the owned-emission migration removes it
pub(crate) fn emit_const_for_real(c: &IrConst) -> String {
    match c.real {
        Some(value) => emit_real_literal(value),
        None => format!("sv4_to_real({})", emit_const(c)),
    }
}

/// Round a rendered real expression through C `float` for shortreal targets.
pub(crate) fn round_shortreal(code: String, shortreal: bool) -> String {
    if shortreal {
        format!("((double)(float)({code}))")
    } else {
        code
    }
}

/// All-X constant expression for a `w`-bit global signal initializer,
/// mirroring the runtime's `sv4_x(w, 0)`: X bits fill the width's limbs,
/// limbs beyond the width are zero.  A brace initializer (not a function
/// call) so the generated C stays a valid static initializer.
#[allow(dead_code)] // legacy emitter helper retained until the owned-emission migration removes it
pub(crate) fn emit_all_x_init(width: u32, signed: bool) -> String {
    let nlimbs = (width as usize).div_ceil(64);
    let mut xz = Vec::with_capacity(nlimbs);
    for i in 0..nlimbs {
        xz.push(if i < nlimbs {
            if i == nlimbs - 1 && !width.is_multiple_of(64) {
                (1u64 << (width % 64)) - 1
            } else {
                u64::MAX
            }
        } else {
            0
        });
    }
    let x = xz
        .iter()
        .map(|v| format!("{v}ULL"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{{ {{ 0 }}, {{ {x} }}, {{ 0 }}, {width}, {} }}",
        signed as u8
    )
}

/// All-Z constant expression for a `w`-bit global initializer, mirroring
/// [`emit_all_x_init`] (the `SV4_Z` macro clamps to 64 bits, so wide nets use
/// this brace initializer to stay a valid static initializer).
#[allow(dead_code)] // legacy emitter helper retained until the owned-emission migration removes it
pub(crate) fn emit_all_z_init(width: u32) -> String {
    let nlimbs = (width as usize).div_ceil(64);
    let mut zz = Vec::with_capacity(nlimbs);
    for i in 0..nlimbs {
        zz.push(if i < nlimbs {
            if i == nlimbs - 1 && !width.is_multiple_of(64) {
                (1u64 << (width % 64)) - 1
            } else {
                u64::MAX
            }
        } else {
            0
        });
    }
    let z = zz
        .iter()
        .map(|v| format!("{v}ULL"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{{ {{ 0 }}, {{ 0 }}, {{ {z} }}, {width}, 0 }}")
}

/// All-zero/all-one known value for a file-scope resolved-net initializer.
#[allow(dead_code)] // legacy emitter helper retained until the owned-emission migration removes it
pub(crate) fn emit_all_known_init(width: u32, signed: bool, ones: bool) -> String {
    let nlimbs = (width as usize).div_ceil(64);
    let mut bits = Vec::with_capacity(nlimbs);
    for i in 0..nlimbs {
        bits.push(if ones && i < nlimbs {
            if i == nlimbs - 1 && !width.is_multiple_of(64) {
                (1u64 << (width % 64)) - 1
            } else {
                u64::MAX
            }
        } else {
            0
        });
    }
    let bits = bits
        .iter()
        .map(|value| format!("{value}ULL"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{{ {{ {bits} }}, {{ 0 }}, {{ 0 }}, {width}, {} }}",
        signed as u8
    )
}

/// Packed owners shared by call-scoped constant borrows in one generated model.
#[derive(Default)]
pub struct PackedConstants {
    entries: std::cell::RefCell<std::collections::BTreeMap<String, (usize, u32, bool)>>,
}

impl PackedConstants {
    pub(super) fn intern(&self, constructor: String, width: u32, signed: bool) -> String {
        let mut entries = self.entries.borrow_mut();
        let next = entries.len();
        let index = entries
            .entry(constructor)
            .or_insert((next, width, signed))
            .0;
        format!("llg_constant_{index}")
    }

    pub(super) fn operands(&self) -> Vec<(String, u32, bool)> {
        self.entries
            .borrow()
            .values()
            .map(|(index, width, signed)| (format!("llg_constant_{index}"), *width, *signed))
            .collect()
    }

    pub(super) fn declarations(&self) -> String {
        let mut out = String::new();
        for index in 0..self.entries.borrow().len() {
            out.push_str(&format!("static sv4_t llg_constant_{index} = SV4_EMPTY;\n"));
        }
        out.push_str("static void llg_model_constants_init(void);\nstatic void llg_model_constants_destroy(void);\n");
        out
    }

    pub(super) fn lifecycle(&self) -> String {
        let entries = self.entries.borrow();
        let mut ordered = entries.iter().collect::<Vec<_>>();
        ordered.sort_by_key(|(_, (index, _, _))| *index);
        let mut tables = String::new();
        let mut init = String::new();
        for (constructor, (index, _, _)) in &ordered {
            let constructor =
                hoist_const_limbs(constructor, &format!("llg_constant_{index}"), &mut tables);
            init.push_str(&format!(
                "    {}\n",
                super::destinations::assign(&format!("&llg_constant_{index}"), &constructor)
            ));
        }
        let mut out = tables;
        out.push_str("static void llg_model_constants_init(void) {\n");
        out.push_str(&init);
        out.push_str("}\nstatic void llg_model_constants_destroy(void) {\n");
        for (_, (index, _, _)) in &ordered {
            out.push_str(&format!("    sv4_destroy(&llg_constant_{index});\n"));
        }
        out.push_str("}\n");
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn constant(width: u32, signed: bool, bit: impl Fn(usize) -> (bool, bool, bool)) -> IrConst {
        let limbs = width.div_ceil(64) as usize;
        let mut planes = [vec![0u64; limbs], vec![0u64; limbs], vec![0u64; limbs]];
        for index in 0..width as usize {
            let (b, x, z) = bit(index);
            for (plane, set) in planes.iter_mut().zip([b, x, z]) {
                if set {
                    plane[index / 64] |= 1 << (index % 64);
                }
            }
        }
        let [bits, x, z] = planes;
        IrConst {
            bits,
            x,
            z,
            width,
            signed,
            real: None,
            fill: None,
        }
    }

    /// `{count{period}}`, least significant repetition first.
    fn replicate(period: &IrConst, count: u32) -> IrConst {
        let width = period.width as usize;
        constant(period.width * count, false, |index| {
            let bit =
                |plane: &[u64]| plane[(index % width) / 64] >> ((index % width) % 64) & 1 == 1;
            (bit(&period.bits), bit(&period.x), bit(&period.z))
        })
    }

    #[test]
    fn element_periodic_constants_split_into_one_period() {
        // 4,096 elements of a 36-bit record: 4 X bits over a 32-bit zero,
        // with one Z bit in the zero, as a dense array's default value.
        let record = |index: usize| {
            let bit = index % 36;
            (bit == 7, bit >= 32, bit == 9)
        };
        let value = constant(36 * 4096, false, record);
        let (period, count) = periodic_constant(&value).expect("periodic");
        assert_eq!((period.width, count), (36, 4096));
        assert_eq!(replicate(&period, count), value);

        let ones = constant(PERIODIC_CONSTANT_MIN_BITS, false, |_| (true, false, false));
        let (period, count) = periodic_constant(&ones).expect("uniform");
        assert_eq!((period.width, count), (1, PERIODIC_CONSTANT_MIN_BITS));
        assert_eq!(replicate(&period, count), ones);

        // A 1,100-bit period over 4,400 bits crosses limb boundaries.
        let wide = constant(4400, false, |index| {
            ((index % 1100) % 3 == 0, index % 1100 == 5, false)
        });
        let (period, count) = periodic_constant(&wide).expect("wide period");
        assert_eq!((period.width, count), (1100, 4));
        assert_eq!(replicate(&period, count), wide);
    }

    #[test]
    fn other_constants_keep_their_tables() {
        let record = |index: usize| (index % 36 == 7, false, false);
        // Below the width threshold, signed, or too few repetitions.
        assert!(
            periodic_constant(&constant(PERIODIC_CONSTANT_MIN_BITS - 1, false, |_| (
                true, false, false
            )))
            .is_none()
        );
        assert!(periodic_constant(&constant(36 * 4096, true, record)).is_none());
        assert!(periodic_constant(&constant(4400, false, |index| (
            index % 1467 == 0,
            false,
            false
        )))
        .is_none());
        // One differing bit in the last repetition, in each plane.
        for plane in 0..3 {
            let value = constant(36 * 4096, false, |index| {
                let (b, x, z) = record(index);
                let last = index == 36 * 4096 - 2;
                (
                    b ^ (last && plane == 0),
                    x ^ (last && plane == 1),
                    z ^ (last && plane == 2),
                )
            });
            assert!(periodic_constant(&value).is_none(), "plane {plane}");
        }
    }
}
