//! Expression and assignment-target lowering into typed simulator IR.

use super::collection::aggregate_path_suffix;
use super::objects::object_query;
use super::*;
use crate::sim::ir::{
    IrArrayDimension, IrArrayQuery, IrArrayQueryKind, IrArrayQueryTarget, IrBinOp, IrChandleExpr,
    IrConst, IrContainerExpr, IrContainerKind, IrFileInput, IrFileInputTarget, IrFileReadTarget,
    IrInsideItem, IrObjectQuery, IrObjectStmt, IrObjectType, IrPlusArgTarget, IrPlusArgText,
    IrStreamSelector, IrStringExpr, IrStringInsideItem, IrVpiCompileArg, IrVpiCompileCall,
};

mod aggregates;
mod array_queries;
mod casts;
mod dispatch;
mod external_input;
mod membership;
mod operations;
mod streaming;
mod system_functions;

fn inside_array_index_vectors(dims: &[(i32, i32)]) -> Vec<Vec<i32>> {
    fn visit(
        dims: &[(i32, i32)],
        dimension: usize,
        current: &mut Vec<i32>,
        values: &mut Vec<Vec<i32>>,
    ) {
        if dimension == dims.len() {
            values.push(current.clone());
            return;
        }
        let (left, right) = dims[dimension];
        let step = if left <= right { 1 } else { -1 };
        let mut index = left;
        loop {
            current.push(index);
            visit(dims, dimension + 1, current, values);
            current.pop();
            if index == right {
                break;
            }
            index = index.saturating_add(step);
        }
    }

    let mut values = Vec::new();
    visit(dims, 0, &mut Vec::new(), &mut values);
    values
}

fn validate_plusarg_format(format: &str, scope_path: &str) -> Result<(), String> {
    let bytes = format.as_bytes();
    if bytes.contains(&0) {
        return Err(format!(
            "$value$plusargs format contains NUL in `{scope_path}`"
        ));
    }
    let mut conversion = false;
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'%' {
            index += 1;
            continue;
        }
        index += 1;
        let Some(&specifier) = bytes.get(index) else {
            return Err(format!(
                "$value$plusargs format ends with `%` in `{scope_path}`"
            ));
        };
        let mut specifier = specifier;
        if specifier == b'%' {
            index += 1;
            continue;
        }
        if specifier == b'0' {
            index += 1;
            let Some(&next) = bytes.get(index) else {
                return Err(format!(
                    "$value$plusargs format ends with `%0` in `{scope_path}`"
                ));
            };
            specifier = next;
        }
        if conversion {
            return Err(format!(
                "$value$plusargs format has more than one conversion in `{scope_path}`"
            ));
        }
        if !matches!(
            specifier.to_ascii_lowercase(),
            b'd' | b'h' | b'x' | b'o' | b'b' | b'f' | b'e' | b'g' | b's'
        ) {
            return Err(format!(
                "$value$plusargs format has unsupported conversion `%{}` in `{scope_path}`",
                char::from(specifier)
            ));
        }
        conversion = true;
        index += 1;
    }
    if !conversion {
        return Err(format!(
            "$value$plusargs format requires one conversion in `{scope_path}`"
        ));
    }
    Ok(())
}

/// Materialize an explicit cast's target width before the value enters any
/// enclosing assignment context. An unbased unsized fill is contextual only
/// until this boundary (IEEE 1800-2009 §6.24.1); retaining its marker would
/// incorrectly refill a wider destination instead of extending the cast value.
fn ir_to_explicit_cast_storage(
    value: IrExpr,
    width: u32,
    signed: bool,
    two_state: bool,
) -> Result<IrExpr, String> {
    let Some(fill) = value.fill else {
        return ir_to_storage(value, width, signed, two_state);
    };
    let limbs = (width as usize).div_ceil(64);
    let mut materialized = vec![u64::MAX; limbs];
    if let Some(last) = materialized.last_mut() {
        let tail = width % 64;
        if tail != 0 {
            *last = (1u64 << tail) - 1;
        }
    }
    let zeros = vec![0; limbs];
    let (bits, x, z) = match (fill, two_state) {
        (2 | 3, true) => (zeros.clone(), zeros.clone(), zeros),
        (0, _) => (zeros.clone(), zeros.clone(), zeros),
        (1, _) => (materialized, zeros.clone(), zeros),
        (2, _) => (zeros.clone(), materialized, zeros),
        (3, _) => (zeros.clone(), zeros, materialized),
        _ => return Err(format!("invalid explicit-cast fill value {fill}")),
    };
    let constant =
        IrConst::packed(bits, x, z, width, signed, None).map_err(|error| error.to_string())?;
    Ok(IrExpr::new(
        IrExprKind::Const(constant),
        width,
        signed,
        None,
    ))
}

fn parse_decimal_real_literal(token: &str) -> Option<f64> {
    let bytes = token.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'_'
            && (!index
                .checked_sub(1)
                .and_then(|previous| bytes.get(previous))
                .is_some_and(u8::is_ascii_digit)
                || !bytes.get(index + 1).is_some_and(u8::is_ascii_digit))
        {
            return None;
        }
    }
    let normalized = token.replace('_', "");
    let bytes = normalized.as_bytes();
    let mut index = usize::from(matches!(bytes.first(), Some(b'+' | b'-')));
    let integer_start = index;
    while bytes.get(index).is_some_and(u8::is_ascii_digit) {
        index += 1;
    }
    if index == integer_start {
        return None;
    }
    if bytes.get(index) == Some(&b'.') {
        index += 1;
        let fraction_start = index;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        if index == fraction_start {
            return None;
        }
    }
    if matches!(bytes.get(index), Some(b'e' | b'E')) {
        index += 1;
        if matches!(bytes.get(index), Some(b'+' | b'-')) {
            index += 1;
        }
        let exponent_start = index;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        if index == exponent_start {
            return None;
        }
    }
    if index != bytes.len() {
        return None;
    }
    normalized
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

#[cfg(test)]
mod cast_tests;
