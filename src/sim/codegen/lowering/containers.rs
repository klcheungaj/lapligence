//! Lowering for resizable unpacked containers.

use super::*;
use crate::sim::ir::{
    IrChandleExpr, IrContainerElement, IrContainerMethod, IrContainerReduction, IrObjectType,
    IrQueueBound, IrQueueSource, IrStringExpr,
};

mod assignments;
mod callbacks;
mod fixed_arrays;
mod indexing;
mod initialization;
mod methods;
mod patterns;
mod queries;
mod streaming;


/// A fixed-array view represented by complete coordinates in logical
/// (declared left-to-right) order.
#[derive(Clone)]
struct P30ArrayView {
    array: ArrayInfo,
    coordinates: Vec<Vec<IrExpr>>,
}

fn parse_pattern_i128(key: &str) -> Option<i128> {
    let key = key.trim();
    let key = key
        .strip_prefix('[')
        .and_then(|key| key.strip_suffix(']'))
        .unwrap_or(key)
        .trim()
        .replace('_', "");
    if let Some((width, literal)) = key.split_once('\'') {
        let _ = width.parse::<u32>().ok()?;
        let (base, digits) = literal.split_at(1);
        let radix = match base {
            "b" | "B" => 2,
            "o" | "O" => 8,
            "d" | "D" => 10,
            "h" | "H" => 16,
            _ => return None,
        };
        let sign = digits.starts_with('-');
        let digits = digits.trim_start_matches('-');
        let value = i128::from_str_radix(digits, radix).ok()?;
        return Some(if sign { -value } else { value });
    }
    key.parse::<i128>().ok()
}

fn parse_pattern_string_key(key: &str) -> Option<Vec<u8>> {
    let key = key.trim();
    let key = key.strip_prefix('"')?.strip_suffix('"')?;
    decode_verilog_string(key).ok()
}

fn pattern_key_expr(index: i128, width: u32, signed: bool, _two_state: bool) -> IrExpr {
    let limbs = width.div_ceil(64) as usize;
    let raw = index as u128;
    let mut bits = vec![0; limbs];
    if let Some(low) = bits.get_mut(0) {
        *low = raw as u64;
    }
    if let Some(high) = bits.get_mut(1) {
        *high = (raw >> 64) as u64;
    }
    if !width.is_multiple_of(64) {
        if let Some(high) = bits.last_mut() {
            *high &= (1u64 << (width % 64)) - 1;
        }
    }
    IrExpr::new(
        IrExprKind::Const(IrConst {
            bits,
            x: vec![0; limbs],
            z: vec![0; limbs],
            width,
            signed,
            real: None,
            fill: None,
        }),
        width,
        signed,
        None,
    )
}
