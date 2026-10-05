//! Lowering for unpacked containers, fixed-array assignments, and reductions.

use super::*;
use crate::sim::ir::{
    IrChandleExpr, IrContainerElement, IrContainerMethod, IrContainerReduction, IrObjectType,
    IrQueueBound, IrQueueSource, IrStringExpr,
};

mod assignments;
mod callbacks;
mod fixed_arrays;
mod fixed_cells;
pub(in crate::sim::codegen) use fixed_cells::FIXED_CELL_UNROLL_LIMIT;
mod fixed_patterns;
mod fixed_receiver;
mod fixed_reductions;
mod indexing;
mod initialization;
mod methods;
mod patterns;
mod queries;
mod real_array_values;
mod selects;
mod streaming;
pub(super) use streaming::{StreamTargetMode, StreamTargetPlan};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum PatternAssignmentKind {
    Blocking,
    Nonblocking,
    Continuous,
}

impl<'a> Codegen<'a> {
    /// Return a pattern's semantic integral index, if it is an array-index
    /// key. Source spelling is diagnostic-only because valid keys can be
    /// arbitrary constant expressions.
    pub(super) fn assignment_pattern_index_key(
        &self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<i128>, String> {
        let NodeKind::Expr(ExprKind::TaggedPattern {
            index_key,
            index_value,
            ..
        }) = self.kind(node)
        else {
            return Ok(None);
        };
        if let Some(value) = index_value {
            return value.to_i128().map(Some).ok_or_else(|| {
                format!("assignment-pattern index key is not a known i128 value in `{path}`")
            });
        }
        index_key
            .as_ref()
            .map(|index| {
                self.eval_bound_i128(*index).map_err(|error| {
                    format!("assignment-pattern index key is not constant in `{path}`: {error}")
                })
            })
            .transpose()
    }
}

/// A fixed-array view represented by complete coordinates in logical
/// (declared left-to-right) order.
#[derive(Clone)]
struct P30ArrayView {
    array: ArrayInfo,
    coordinates: Vec<Vec<IrExpr>>,
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
