//! Coverpoints wider than 64 bits (IEEE 1800-2009 19.5) on the 64-bit
//! runtime keys.
//!
//! The bins of such a coverpoint name finitely many value ranges, so its
//! values fall into classes: maximal ranges that every item holds entirely
//! or not at all. Each class gets one runtime key, except that classes
//! inside the items of `[]` and `[N]` bins (and `[]` transition bins) keep
//! one key per value, so the runtime can count and distribute their values
//! as it does at 64 bits. Keys keep the value order, so every key range the
//! runtime builds from an item is the item's values. The runtime maps a
//! sampled value to its key through the class table (`llg_cov_hit_wide`).
//!
//! A coverpoint without bins declarations has automatic bins capped by
//! `auto_bin_max` (19.5.2); the runtime computes their index from the whole
//! value, so it needs no table.
use super::{CoverageBuild, WideItems};
use crate::core::db::{CoverpointInfo, ExprKind, NodeId, NodeKind, Operation};
use crate::core::elab::{Bit, Value};
use crate::sim::codegen::lowering::Codegen;
use crate::sim::ir::{IrCoverAutomatic, IrCoverBound, IrCoverItem, IrCoverWide, IrCoverpoint};

/// Widths up to this many bits fit the 128-bit keys of the compile-time
/// tables and the two runtime words a sampled value is read from.
pub(super) const MAX_WIDE_POINT_BITS: u32 = 128;
/// The most values one item of a `[]` or `[N]` bin of a wide coverpoint
/// may hold: each keeps its own runtime key.
pub(super) const MAX_WIDE_EXACT_VALUES: u128 = 1 << 32;

/// A constant value of any width: sign and magnitude, or beyond 128 bits
/// (`huge`), where only its sign matters to a coverpoint of at most 128.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct WideNum {
    neg: bool,
    mag: u128,
    huge: bool,
}

impl WideNum {
    fn new(neg: bool, mag: u128, huge: bool) -> Self {
        Self {
            neg: neg && (mag != 0 || huge),
            mag,
            huge,
        }
    }

    /// The numeric value of a known constant (two's complement when signed).
    pub(super) fn of(value: &Value) -> Self {
        let negative = value.signed && value.bits.first() == Some(&Bit::One);
        if negative {
            match value.to_i128() {
                Some(number) => Self::new(true, number.unsigned_abs(), false),
                None => Self::new(true, 0, true),
            }
        } else {
            match value.to_u128() {
                Some(number) => Self::new(false, number, false),
                None => Self::new(false, 0, true),
            }
        }
    }

    /// The smallest and largest value of a `width`-bit type.
    pub(super) fn type_range(width: u32, signed: bool) -> (Self, Self) {
        let all = u128::MAX >> (u128::BITS - width);
        if signed {
            let half = 1u128 << (width - 1);
            (
                Self::new(true, half, false),
                Self::new(false, half - 1, false),
            )
        } else {
            (Self::new(false, 0, false), Self::new(false, all, false))
        }
    }

    /// The runtime ordering: huge negatives, negatives, non-negatives,
    /// huge positives.
    fn rank(self) -> (i8, u128) {
        match (self.neg, self.huge) {
            (true, true) => (-2, 0),
            (true, false) => (-1, u128::MAX - self.mag),
            (false, false) => (0, self.mag),
            (false, true) => (1, 0),
        }
    }

    /// The key of a value inside the `width`-bit type: its bits, with the
    /// sign bit flipped when signed, so keys follow the numeric order.
    pub(super) fn key(self, width: u32, signed: bool) -> u128 {
        let all = u128::MAX >> (u128::BITS - width);
        let raw = if self.neg {
            0u128.wrapping_sub(self.mag) & all
        } else {
            self.mag & all
        };
        if signed {
            raw ^ (1u128 << (width - 1))
        } else {
            raw
        }
    }
}

impl PartialOrd for WideNum {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for WideNum {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.rank().cmp(&other.rank())
    }
}

/// The classes of one wide coverpoint: class `i` holds the keys from
/// `bounds[i]` up to the next bound and starts at runtime key `bases[i]`;
/// an exact class has one runtime key per value.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct WideMap {
    pub(super) bounds: Vec<u128>,
    pub(super) bases: Vec<u64>,
    pub(super) exact: Vec<bool>,
}

impl WideMap {
    /// The classes of the item key ranges `(lo, hi, exact)` of a
    /// `width`-bit coverpoint. `None` when the runtime keys would not fit
    /// in 64 bits.
    pub(super) fn build(width: u32, ranges: &[(u128, u128, bool)]) -> Option<Self> {
        let max = u128::MAX >> (u128::BITS - width);
        let mut bounds = vec![0u128];
        for &(lo, hi, _) in ranges {
            bounds.push(lo);
            if hi < max {
                bounds.push(hi + 1);
            }
        }
        bounds.sort_unstable();
        bounds.dedup();
        let class = |key: u128| bounds.partition_point(|bound| *bound <= key) - 1;
        let mut exact = vec![false; bounds.len()];
        for &(lo, hi, wanted) in ranges {
            if wanted {
                for flag in &mut exact[class(lo)..=class(hi)] {
                    *flag = true;
                }
            }
        }
        let mut bases = Vec::with_capacity(bounds.len());
        let mut next = 0u64;
        for (index, bound) in bounds.iter().enumerate() {
            bases.push(next);
            let size = if exact[index] {
                let end = bounds.get(index + 1).map_or(max, |next| next - 1);
                u64::try_from(end - bound + 1).ok()?
            } else {
                1
            };
            next = next.checked_add(size)?;
        }
        Some(Self {
            bounds,
            bases,
            exact,
        })
    }

    /// The runtime key of a value key.
    pub(super) fn key(&self, key: u128) -> u64 {
        let class = self.bounds.partition_point(|bound| *bound <= key) - 1;
        let offset = if self.exact[class] {
            (key - self.bounds[class]) as u64
        } else {
            0
        };
        self.bases[class] + offset
    }
}

impl Codegen<'_> {
    /// The automatic bins of a coverpoint wider than 64 bits (19.5.2):
    /// capped by `auto_bin_max`, with no values removed.
    pub(super) fn coverage_wide_automatic(
        &self,
        group_name: &str,
        point: NodeId,
        info: &CoverpointInfo,
        width: u32,
        signed: bool,
    ) -> Result<IrCoverAutomatic, String> {
        let point_name = self.coverpoint_name(point);
        if let Some(bin) = info.bins.first() {
            return Err(self.coverage_error(
                *bin,
                format!(
                    "ignore_bins and illegal_bins of coverpoint `{point_name}` in covergroup `{group_name}`, which is {width} bits wide and has automatic bins: llg removes values from automatic bins only for coverpoints of at most 64 bits (llg limitation)"
                ),
            ));
        }
        if self.coverpoint_enum(info, width, signed).is_some() {
            return Err(self.coverage_error(
                point,
                format!(
                    "enum coverpoint `{point_name}` of covergroup `{group_name}` is {width} bits wide; llg supports enum coverpoints of at most 64 bits (llg limitation)"
                ),
            ));
        }
        Ok(IrCoverAutomatic::Auto)
    }

    /// One bins item of a coverpoint wider than 64 bits, resolved by 19.5.6
    /// like [`Self::coverage_value_item`]: a placeholder key naming its
    /// range while the coverpoint's own bins lower, its runtime keys once
    /// the classes are known.
    pub(super) fn coverage_wide_item(
        &mut self,
        bin_name: &str,
        item: NodeId,
        wildcard: bool,
        coverpoint: &mut IrCoverpoint,
        build: &mut CoverageBuild,
    ) -> Result<(), String> {
        let Some(wide) = build.wide.as_ref() else {
            return Err(self.coverage_error(item, "wide coverpoint item without its type"));
        };
        let (width, signed) = (wide.width, wide.signed);
        let (low_node, high_node) = match self.kind(item) {
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::List,
                operands,
                ..
            }) if operands.len() == 2 => (operands[0], operands[1]),
            _ => (item, item),
        };
        let argument = |cg: &Codegen<'_>| {
            cg.coverage_error(
                item,
                format!(
                    "bins `{bin_name}` of a coverpoint wider than 64 bits reads a covergroup argument; llg reads bins values from arguments only for coverpoints of at most 64 bits (llg limitation)"
                ),
            )
        };
        // 19.5.3: a wildcard singleton is the set of values its X, Z and ?
        // bits leave open.
        if wildcard && low_node == high_node {
            let node = self.coverage_bin_operand(item, width, signed);
            let value = self.eval_bits(node).map_err(|_| argument(self))?;
            for (lo, hi) in self.coverage_wide_wildcard(bin_name, item, &value, width, signed)? {
                self.coverage_wide_push(bin_name, item, lo, hi, coverpoint, build)?;
            }
            return Ok(());
        }
        let (min, max) = WideNum::type_range(width, signed);
        let bound =
            |cg: &Codegen<'_>, node: NodeId, upper: bool| -> Result<Option<WideNum>, String> {
                let node = cg.coverage_bin_operand(node, width, signed);
                if matches!(cg.kind(node), NodeKind::Expr(ExprKind::Unbounded)) {
                    return Ok(Some(if upper { max } else { min }));
                }
                let mut value = cg.eval_bits(node).map_err(|_| argument(cg))?;
                // A wildcard range bound: its open bits at their extreme.
                if wildcard {
                    let fill = if upper { Bit::One } else { Bit::Zero };
                    for bit in &mut value.bits {
                        if matches!(bit, Bit::X | Bit::Z) {
                            *bit = fill;
                        }
                    }
                }
                Ok((!value.is_unknown()).then(|| WideNum::of(&value)))
            };
        let low = bound(self, low_node, false)?;
        let high = bound(self, high_node, true)?;
        let (Some(low), Some(high)) = (low, high) else {
            self.coverage_warning(
                item,
                format!("bins `{bin_name}` value has X or Z bits and is ignored (IEEE 1800-2009 19.5.6)"),
            );
            return Ok(());
        };
        let (low_in, high_in) = (low.max(min), high.min(max));
        if low > high || low_in > high_in {
            self.coverage_warning(
                item,
                format!("bins `{bin_name}` value is outside the coverpoint type and is ignored (IEEE 1800-2009 19.5.6)"),
            );
            return Ok(());
        }
        if low_in != low || high_in != high {
            self.coverage_warning(
                item,
                format!("bins `{bin_name}` range is limited to the values of the coverpoint type (IEEE 1800-2009 19.5.6)"),
            );
        }
        let (lo, hi) = (low_in.key(width, signed), high_in.key(width, signed));
        self.coverage_wide_push(bin_name, item, lo, hi, coverpoint, build)
    }

    /// One key range of a wide coverpoint's item.
    fn coverage_wide_push(
        &self,
        bin_name: &str,
        item: NodeId,
        lo: u128,
        hi: u128,
        coverpoint: &mut IrCoverpoint,
        build: &mut CoverageBuild,
    ) -> Result<(), String> {
        let exact = build.wide.as_ref().is_some_and(|wide| wide.exact);
        if exact && hi - lo >= MAX_WIDE_EXACT_VALUES {
            return Err(self.coverage_error(
                item,
                format!(
                    "bins `{bin_name}` of a coverpoint wider than 64 bits has an item of more than 2^32 values, each a bin or counted in a fixed number of bins; llg supports this only for coverpoints of at most 64 bits (llg limitation)"
                ),
            ));
        }
        let Some(wide) = build.wide.as_mut() else {
            return Ok(());
        };
        let (lo, hi) = match &wide.map {
            Some(map) => (map.key(lo), map.key(hi)),
            None => {
                wide.ranges.push((lo, hi, exact));
                let index = (wide.ranges.len() - 1) as u64;
                (index, index)
            }
        };
        coverpoint.items.push(IrCoverItem {
            lo: IrCoverBound::Key(lo),
            hi: IrCoverBound::Key(hi),
            site: None,
        });
        Ok(())
    }

    /// The key ranges of a wildcard value of a wide coverpoint (19.5.3,
    /// 19.5.6), as `wildcard_ranges` gives them at 64 bits.
    fn coverage_wide_wildcard(
        &mut self,
        bin_name: &str,
        item: NodeId,
        value: &Value,
        width: u32,
        signed: bool,
    ) -> Result<Vec<(u128, u128)>, String> {
        let mask = u128::MAX >> (u128::BITS - width);
        let source = value.width();
        let extension = if value.signed && source > 0 {
            value.bit_lsb(source - 1)
        } else {
            Bit::Zero
        };
        let (mut fixed, mut care) = (0u128, 0u128);
        let mut limited = false;
        for index in 0..source.max(width as usize) {
            let bit = if index < source {
                value.bit_lsb(index)
            } else {
                extension
            };
            if index >= width as usize {
                match bit {
                    Bit::One => {
                        self.coverage_warning(
                            item,
                            format!("bins `{bin_name}` value is outside the coverpoint type and is ignored (IEEE 1800-2009 19.5.6)"),
                        );
                        return Ok(Vec::new());
                    }
                    Bit::X | Bit::Z => limited = true,
                    Bit::Zero => {}
                }
                continue;
            }
            match bit {
                Bit::One => {
                    fixed |= 1 << index;
                    care |= 1 << index;
                }
                Bit::Zero => care |= 1 << index,
                Bit::X | Bit::Z => {}
            }
        }
        if limited {
            self.coverage_warning(
                item,
                format!("bins `{bin_name}` range is limited to the values of the coverpoint type (IEEE 1800-2009 19.5.6)"),
            );
        }
        let open = !care & mask;
        if open == mask {
            return Ok(vec![(0, mask)]);
        }
        let low_block = (1u128 << care.trailing_zeros()) - 1;
        let high_open = open & !low_block;
        if u64::from(high_open.count_ones()) > super::WILDCARD_RANGE_LIMIT.trailing_zeros().into() {
            return Err(self.coverage_error(
                item,
                format!(
                    "wildcard bins `{bin_name}` value matches more than {} separate value ranges, which llg does not support",
                    super::WILDCARD_RANGE_LIMIT
                ),
            ));
        }
        let sign = if signed { 1u128 << (width - 1) } else { 0 };
        let mut ranges = Vec::new();
        let mut subset = 0u128;
        loop {
            let base = fixed | subset;
            ranges.push((base ^ sign, (base | low_block) ^ sign));
            subset = subset.wrapping_sub(high_open) & high_open;
            if subset == 0 {
                break;
            }
        }
        ranges.sort_unstable();
        Ok(ranges)
    }

    /// The classes of a wide coverpoint from its items' ranges; every
    /// placeholder key of its value and transition bins becomes a runtime
    /// key.
    pub(super) fn coverage_wide_keys(
        &self,
        group_name: &str,
        point: NodeId,
        items: WideItems,
        coverpoint: &mut IrCoverpoint,
        transition_bins: &mut [super::transitions::BinSpec],
    ) -> Result<(), String> {
        if coverpoint.automatic == IrCoverAutomatic::Auto {
            coverpoint.wide = Some(IrCoverWide {
                width: items.width,
                signed: items.signed,
                bounds: Vec::new(),
                bases: Vec::new(),
                exact: Vec::new(),
            });
            return Ok(());
        }
        let map = WideMap::build(items.width, &items.ranges).ok_or_else(|| {
            self.coverage_error(
                point,
                format!(
                    "the bins of coverpoint `{}` in covergroup `{group_name}` need more than 2^64 run-time keys (llg limitation)",
                    coverpoint.name
                ),
            )
        })?;
        let ranges = &items.ranges;
        let key = |bound: IrCoverBound, high: bool| match bound {
            IrCoverBound::Key(index) => {
                let range = ranges[index as usize];
                IrCoverBound::Key(map.key(if high { range.1 } else { range.0 }))
            }
            other => other,
        };
        for item in &mut coverpoint.items {
            item.lo = key(item.lo, false);
            item.hi = key(item.hi, true);
        }
        for step in transition_bins
            .iter_mut()
            .flat_map(|bin| bin.sets.iter_mut())
            .flatten()
        {
            for range in &mut step.ranges {
                let entry = ranges[range.0 as usize];
                *range = (map.key(entry.0), map.key(entry.1));
            }
            for item in &mut step.items {
                item.lo = key(item.lo, false);
                item.hi = key(item.hi, true);
            }
        }
        coverpoint.wide = Some(IrCoverWide {
            width: items.width,
            signed: items.signed,
            bounds: map.bounds,
            bases: map.bases,
            exact: map.exact,
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(bits: &str, signed: bool) -> Value {
        Value {
            bits: bits
                .chars()
                .map(|bit| if bit == '1' { Bit::One } else { Bit::Zero })
                .collect(),
            signed,
            fill: None,
        }
    }

    #[test]
    fn numbers_order_and_key_like_the_type() {
        let minus_one = WideNum::of(&value("11111111", true));
        let three = WideNum::of(&value("011", false));
        assert!(minus_one < three);
        let (min, max) = WideNum::type_range(71, true);
        assert!(min < minus_one && three < max);
        // Signed 71 bits: -1 is just below the flipped sign bit, 3 above it.
        assert_eq!(minus_one.key(71, true), (1u128 << 70) - 1);
        assert_eq!(three.key(71, true), (1u128 << 70) + 3);
        assert_eq!(WideNum::type_range(128, false).1.key(128, false), u128::MAX);
        let wide = value(&format!("1{}", "0".repeat(140)), false);
        assert!(WideNum::of(&wide) > WideNum::type_range(128, false).1);
    }

    #[test]
    fn classes_keep_order_and_exact_values() {
        // A scalar bin over [2^69, 2^70), one over {2^70} and an exact
        // array over [5, 8].
        let ranges = [
            (1u128 << 69, (1u128 << 70) - 1, false),
            (1u128 << 70, 1u128 << 70, false),
            (5, 8, true),
        ];
        let map = WideMap::build(71, &ranges).unwrap();
        assert_eq!(map.bounds, [0, 5, 9, 1 << 69, 1 << 70, (1 << 70) + 1]);
        assert_eq!(map.bases, [0, 1, 5, 6, 7, 8]);
        assert_eq!(map.key(4), 0);
        assert_eq!(map.key(7), 3);
        assert_eq!(map.key((1 << 69) + 12345), 6);
        assert_eq!(map.key(1 << 70), 7);
        assert_eq!(map.key((1 << 71) - 1), 8);
    }
}
