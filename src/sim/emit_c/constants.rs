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
