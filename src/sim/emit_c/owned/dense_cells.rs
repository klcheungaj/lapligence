//! One runtime call per run of consecutive dense array cells.
//!
//! Lowering expresses a whole unpacked-array value element by element: a
//! concatenation of constant-index reads, and a block of constant-index
//! element stores taking slices of one captured value. Emitted one access at
//! a time, both make `model.c` and its compile time grow with the element
//! count. A run of consecutive cells of one dense array is instead one call
//! over the cell range, so the C stays proportional to the aggregate's shape.
use super::*;

/// Shorter runs keep their per-element code.
const DENSE_CELL_RUN_MIN: usize = LLG_DENSE_CELL_RUN_MIN;

/// A constant-index whole-cell access of directly addressed dense storage.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Cell {
    array: usize,
    offset: u64,
}

/// What one element store writes.
#[derive(Clone, Copy, PartialEq)]
enum StoreSource<'a> {
    /// Bits `left` down to `left - width + 1` of a captured value, cleared of
    /// X/Z when `two_state`.
    Slice {
        base: &'a IrExpr,
        left: i64,
        two_state: bool,
    },
    /// One constant, the same for every cell of the run.
    Fill(&'a IrExpr),
}

#[derive(Clone, Copy)]
struct Store<'a> {
    cell: Cell,
    nba: bool,
    source: StoreSource<'a>,
}

/// The index an `sv4_to_index_i64` of this constant yields, if it is known.
fn constant_index(index: &IrExpr) -> Option<i64> {
    let IrExprKind::Const(constant) = &index.kind else {
        return None;
    };
    let width = constant.width();
    if index.fill.is_some()
        || constant.fill().is_some()
        || index.width != width
        || index.signed != constant.signed()
        || width == 0
        || width > 64
        || constant
            .x_mask()
            .iter()
            .chain(constant.z_mask())
            .any(|word| *word != 0)
    {
        return None;
    }
    let mask = if width == 64 {
        u64::MAX
    } else {
        (1u64 << width) - 1
    };
    let bits = constant.bits().first().copied().unwrap_or(0) & mask;
    if constant.signed() && (bits >> (width - 1)) & 1 == 1 {
        Some((bits | !mask) as i64)
    } else {
        i64::try_from(bits).ok()
    }
}

/// A captured operand: evaluating it once or once per element is the same.
fn captured(expression: &IrExpr) -> bool {
    match &expression.kind {
        IrExprKind::LocalRead(_) => true,
        IrExprKind::Convert { a } | IrExprKind::Resize { a } => captured(a),
        _ => false,
    }
}

/// Concatenation operands with nested concatenations spliced in: a nested
/// concatenation contributes exactly its operands' bits.
fn concat_leaves<'a>(parts: &'a [IrExpr], leaves: &mut Vec<&'a IrExpr>) {
    for part in parts {
        match &part.kind {
            IrExprKind::Concat { parts } if part.fill.is_none() => concat_leaves(parts, leaves),
            _ => leaves.push(part),
        }
    }
}

/// Leaf targets of a left-to-right streaming target with their widths. Such
/// a stream reorders nothing, so a nested one splices in its own parts.
fn stream_leaves<'a>(parts: &'a [(IrLhs, u32)], leaves: &mut Vec<(&'a IrLhs, u32)>) {
    for (part, width) in parts {
        match part {
            IrLhs::Stream {
                parts,
                direction: IrStreamDirection::LeftToRight,
                ..
            } => stream_leaves(parts, leaves),
            _ => leaves.push((part, *width)),
        }
    }
}

impl Frame<'_, '_> {
    fn dense_cell(&self, array: usize, indices: &[IrExpr], selection: &IrElemSel) -> Option<Cell> {
        let info = self.ctx.model.array(array);
        if !matches!(selection, IrElemSel::Whole)
            || info.sparse()
            || info.real
            || info.activation
            || info.elem_width == 0
            || indices.len() != info.dims.len()
        {
            return None;
        }
        // The same row-major linearization as `array_cell`.
        let mut offset = 0u64;
        for (index, &(left, right)) in indices.iter().zip(&info.dims) {
            let index = constant_index(index)?;
            if index < i64::from(left.min(right)) || index > i64::from(left.max(right)) {
                return None;
            }
            let position = i64::from(left).abs_diff(index);
            let extent = i64::from(left).abs_diff(i64::from(right)) + 1;
            offset = offset.checked_mul(extent)?.checked_add(position)?;
        }
        Some(Cell { array, offset })
    }

    fn dense_cell_read(&self, part: &IrExpr) -> Option<Cell> {
        let IrExprKind::ArrayRead {
            arr,
            indices,
            elem_sel,
        } = &part.kind
        else {
            return None;
        };
        let cell = self.dense_cell(*arr, indices, elem_sel)?;
        (part.fill.is_none() && part.width == self.ctx.model.array(*arr).elem_width).then_some(cell)
    }

    /// Length of the run of consecutive cell reads that starts `parts`.
    fn dense_read_run(&self, parts: &[&IrExpr]) -> usize {
        let Some(first) = parts.first().and_then(|part| self.dense_cell_read(part)) else {
            return 0;
        };
        let mut length = 1;
        for part in &parts[1..] {
            let expected = Cell {
                array: first.array,
                offset: first.offset + length as u64,
            };
            if self.dense_cell_read(part) != Some(expected) {
                break;
            }
            length += 1;
        }
        length
    }

    /// Concatenation operands to emit with cell runs, or `None` when no run
    /// reaches `DENSE_CELL_RUN_MIN` and the ordinary emission applies.
    /// Sampled reads keep per-element emission.
    pub(super) fn dense_concat_leaves<'a>(&self, parts: &'a [IrExpr]) -> Option<Vec<&'a IrExpr>> {
        if self.sampled_reads {
            return None;
        }
        let mut leaves = Vec::new();
        concat_leaves(parts, &mut leaves);
        let mut rest = leaves.as_slice();
        while !rest.is_empty() {
            let run = self.dense_read_run(rest);
            if run >= DENSE_CELL_RUN_MIN {
                return Some(leaves);
            }
            rest = &rest[run.max(1)..];
        }
        None
    }

    /// Emit concatenation operands, reading each cell run with one call.
    pub(super) fn concat_dense_leaves(&mut self, leaves: &[&IrExpr]) -> Result<Value, String> {
        let mut result: Option<Value> = None;
        let mut rest = leaves;
        while let Some(part) = rest.first() {
            let run = self.dense_read_run(rest);
            if run >= DENSE_CELL_RUN_MIN {
                let first = self
                    .dense_cell_read(part)
                    .ok_or_else(|| "dense cell run lost its first cell".to_owned())?;
                let info = self.ctx.model.array(first.array);
                let width = u32::try_from(run)
                    .ok()
                    .and_then(|count| info.elem_width.checked_mul(count))
                    .ok_or_else(|| "dense cell image width overflow".to_owned())?;
                let cells = info.cell_address(&format!("{}ULL", first.offset));
                let image = self.value(
                    format!("llg_dense_cells_image({cells}, {run}ULL)"),
                    width,
                    false,
                );
                result = Some(match result {
                    None => image,
                    Some(prefix) => {
                        let width = prefix
                            .width
                            .checked_add(width)
                            .ok_or_else(|| "concatenation width overflow".to_owned())?;
                        let code = format!("sv4_concat({}, {})", prefix.code, image.code);
                        let joined = self.replace(prefix, code, width, false);
                        self.discard(image);
                        joined
                    }
                });
                rest = &rest[run..];
                continue;
            }
            result = Some(match result {
                None => self.expression(part)?,
                Some(prefix) => {
                    let value = self.operand(part)?;
                    let width = prefix
                        .width
                        .checked_add(value.width)
                        .ok_or_else(|| "concatenation width overflow".to_owned())?;
                    let code = format!("sv4_concat({}, {})", prefix.code, value.code);
                    let joined = self.replace(prefix, code, width, false);
                    self.discard(value);
                    joined
                }
            });
            rest = &rest[1..];
        }
        let mut result = result.ok_or_else(|| "empty concatenation".to_owned())?;
        // Even a one-element concatenation is unsigned and self-determined.
        self.line(format!("llg_sv4_set_signed(&{}, 0);", result.code));
        result.signed = false;
        result.fill = None;
        Ok(result)
    }

    fn dense_cell_store<'a>(&self, statement: &'a IrStmt) -> Option<Store<'a>> {
        if self.read_only_callback {
            return None;
        }
        let IrStmt::Assign {
            lhs:
                IrLhs::ArrayElem {
                    arr,
                    indices,
                    elem_sel,
                },
            rhs,
            nba,
        } = statement
        else {
            return None;
        };
        let info = self.ctx.model.array(*arr);
        if info.is_net() {
            return None;
        }
        let cell = self.dense_cell(*arr, indices, elem_sel)?;
        if matches!(rhs.kind, IrExprKind::Const(_)) {
            return Some(Store {
                cell,
                nba: *nba,
                source: StoreSource::Fill(rhs),
            });
        }
        // Same-width conversions change only the sign, which the store's
        // cast to the cell sets anyway; a two-state conversion clears X/Z.
        let width = info.elem_width;
        let mut value = rhs;
        let mut two_state = false;
        loop {
            if value.width != width || value.fill.is_some() {
                return None;
            }
            match &value.kind {
                IrExprKind::ToTwoState { a } => {
                    two_state = true;
                    value = a;
                }
                IrExprKind::Convert { a } | IrExprKind::Resize { a } => value = a,
                IrExprKind::PartSel { base, left, right } => {
                    let in_range = *right >= 0
                        && left.checked_sub(*right) == Some(i64::from(width) - 1)
                        && *left < i64::from(base.width);
                    return (in_range && base.fill.is_none() && captured(base)).then_some(Store {
                        cell,
                        nba: *nba,
                        source: StoreSource::Slice {
                            base,
                            left: *left,
                            two_state,
                        },
                    });
                }
                _ => return None,
            }
        }
    }

    /// Emit a run of whole-cell stores into consecutive cells of one dense
    /// array as one call, returning the number of statements consumed (zero
    /// when no run reaches `DENSE_CELL_RUN_MIN`).
    pub(super) fn dense_cell_stores(&mut self, statements: &[IrStmt]) -> Result<usize, String> {
        let Some(first) = statements
            .first()
            .and_then(|statement| self.dense_cell_store(statement))
        else {
            return Ok(0);
        };
        let width = i64::from(self.ctx.model.array(first.cell.array).elem_width);
        let mut count = 1usize;
        for statement in &statements[1..] {
            let Some(next) = self.dense_cell_store(statement) else {
                break;
            };
            let step = count as i64;
            let source = match first.source {
                StoreSource::Slice {
                    base,
                    left,
                    two_state,
                } => StoreSource::Slice {
                    base,
                    left: left - step * width,
                    two_state,
                },
                fill => fill,
            };
            let expected = Cell {
                array: first.cell.array,
                offset: first.cell.offset + count as u64,
            };
            if next.cell != expected || next.nba != first.nba || next.source != source {
                break;
            }
            count += 1;
        }
        if count < DENSE_CELL_RUN_MIN {
            return Ok(0);
        }
        let cancellation_mark = self.cancellation_mark();
        let info = self.ctx.model.array(first.cell.array);
        let (cell_width, signed, two_state) = (info.elem_width, info.signed, info.two_state);
        let cells = info.cell_address(&format!("{}ULL", first.cell.offset));
        let nba = u8::from(first.nba);
        match first.source {
            StoreSource::Slice {
                base,
                left,
                two_state: cleared,
            } => {
                let source = self.expression(base)?;
                self.line(format!(
                    "llg_dense_cells_store({cells}, {count}ULL, {}, {left}LL, {cell_width}u, {}, {}, {nba});",
                    source.address(),
                    u8::from(signed),
                    u8::from(two_state || cleared)
                ));
                self.discard(source);
            }
            StoreSource::Fill(constant) => {
                let value = self.expression(constant)?;
                let value = self.convert(value, cell_width, signed, two_state, false);
                self.line(format!(
                    "llg_dense_cells_fill({cells}, {count}ULL, {}, {nba});",
                    value.address()
                ));
                self.discard(value);
            }
        }
        self.cancellation_check_covering(cancellation_mark)?;
        Ok(count)
    }

    /// The cell run a left-to-right streaming target covers when its leaves
    /// are all consecutive whole cells of one dense array (a whole-array
    /// copy-out): its first cell and cell count.
    fn dense_stream_run(&self, lhs: &IrLhs) -> Option<(Cell, usize)> {
        let IrLhs::Stream {
            parts,
            width,
            direction: IrStreamDirection::LeftToRight,
            ..
        } = lhs
        else {
            return None;
        };
        if self.read_only_callback {
            return None;
        }
        let mut leaves = Vec::new();
        stream_leaves(parts, &mut leaves);
        let mut first: Option<Cell> = None;
        for (position, (leaf, leaf_width)) in leaves.iter().enumerate() {
            let IrLhs::ArrayElem {
                arr,
                indices,
                elem_sel,
            } = leaf
            else {
                return None;
            };
            let info = self.ctx.model.array(*arr);
            let cell = self.dense_cell(*arr, indices, elem_sel)?;
            if info.is_net() || *leaf_width != info.elem_width {
                return None;
            }
            let start = *first.get_or_insert(cell);
            let expected = Cell {
                array: start.array,
                offset: start.offset + position as u64,
            };
            if cell != expected {
                return None;
            }
        }
        let first = first?;
        let count = leaves.len();
        let covered = u64::from(self.ctx.model.array(first.array).elem_width) * count as u64;
        (count >= DENSE_CELL_RUN_MIN && covered == u64::from(*width)).then_some((first, count))
    }

    /// Store `value` (an owner of at least the run's width) into the cell
    /// run `lhs` covers, as one range call, and consume it. Returns the
    /// value unchanged, emitting nothing, when `lhs` is not such a run.
    pub(super) fn dense_stream_store(
        &mut self,
        lhs: &IrLhs,
        value: Value,
        nba: bool,
    ) -> Result<Option<Value>, String> {
        let Some((first, count)) = self.dense_stream_run(lhs) else {
            return Ok(Some(value));
        };
        let info = self.ctx.model.array(first.array);
        let (cell_width, signed, two_state) = (info.elem_width, info.signed, info.two_state);
        let width = cell_width * u32::try_from(count).map_err(|_| "dense run too long")?;
        let cells = info.cell_address(&format!("{}ULL", first.offset));
        // The stream takes the value's rightmost `width` bits, unsigned.
        let value = self.convert(value, width, false, false, false);
        self.line(format!(
            "llg_dense_cells_store({cells}, {count}ULL, {}, {}LL, {cell_width}u, {}, {}, {});",
            value.address(),
            width - 1,
            u8::from(signed),
            u8::from(two_state),
            u8::from(nba)
        ));
        self.discard(value);
        Ok(None)
    }
}
