//! Capture every streaming destination before publishing any write. The RHS and
//! unpacked pieces remain registered owners if a selector calls or suspends.
use super::*;
use super::stores::Target;

impl Frame<'_, '_> {
    pub(super) fn prepare_assignment(&mut self, lhs: &IrLhs, value: Value) -> Result<Vec<(Target, Value)>, String> {
        let IrLhs::Stream { parts, width, slice, direction } = lhs else {
            return Ok(vec![(self.target(lhs)?, value)]);
        };
        let mut remaining = *width;
        if *width == 0 || *slice == 0 || parts.is_empty() { return Err("invalid streaming assignment shape".to_owned()); }
        let value = self.convert(value, *width, false, false, false);
        let code = format!("sv4_unstream({}, {slice}, {})", value.code,
            u8::from(*direction == IrStreamDirection::RightToLeft));
        let value = self.replace(value, code, *width, false);
        let mut writes = Vec::new();
        for (part, part_width) in parts {
            remaining = remaining.checked_sub(*part_width).ok_or_else(|| "streaming target width overflow".to_owned())?;
            if *part_width == 0 { return Err("empty streaming target part".to_owned()); }
            let high = remaining + part_width - 1;
            let piece = self.value(format!("sv4_part_select({}, {high}ULL, {remaining}ULL)", value.code), *part_width, false);
            writes.extend(self.prepare_assignment(part, piece)?);
        }
        if remaining != 0 { return Err("streaming target widths do not cover the source".to_owned()); }
        self.discard(value);
        Ok(writes)
    }
}


impl Frame<'_, '_> {
    /// Materialize the complete source and every selector/address before the
    /// first write. Container commits cannot change a later target expression.
    pub(super) fn stream_assignment(&mut self, source: &IrExpr, slice: u32,
        direction: IrStreamDirection, targets: &[IrStreamTarget],
    ) -> Result<(), String> {
        enum Write {
            Packed(Target, Value),
            Container { name: String, function: &'static str, value: Value,
                kind: i32, first: String, second: String, width: String },
        }
        if slice == 0 || targets.is_empty() { return Err("invalid streaming assignment shape".to_owned()); }
        if targets.iter().filter(|target| matches!(target, IrStreamTarget::Container { .. })).count() > 1 {
            return Err("streaming assignment supports at most one resizable target".to_owned());
        }
        let value = self.expression(source)?;
        if value.width == 0 { return Err("streaming source must be packed".to_owned()); }
        let width = value.width;
        let code = format!("sv4_unstream({}, {slice}, {})", value.code, u8::from(direction == IrStreamDirection::RightToLeft));
        let value = self.replace(value, code, width, false);
        // The descriptor's width is authoritative for a resizable source.
        let cursor = self.scalar("int64_t", format!("(int64_t){}.width", value.code));
        let mut owners = Vec::new();
        let mut writes = Vec::new();
        for (position, target) in targets.iter().enumerate() {
            match target {
                IrStreamTarget::Packed { lhs, width } => {
                    let piece = self.value(format!("sv4_part_select({}, {cursor} - 1, {cursor} - {width})", value.code), *width, false);
                    writes.extend(self.prepare_assignment(lhs, piece)?.into_iter().map(|(target, value)| Write::Packed(target, value)));
                    self.line(format!("{cursor} -= {width};"));
                }
                IrStreamTarget::Container { container, selector } => {
                    let container = self.ctx.model.containers[*container].clone();
                    let Some((element_width, _, _)) = container.element.packed() else {
                        return Err("streaming target requires packed elements".to_owned());
                    };
                    let trailing = targets[position + 1..].iter().try_fold(0u32, |sum, target| {
                        let IrStreamTarget::Packed { width, .. } = target else { return Err("multiple resizable streaming targets".to_owned()); };
                        sum.checked_add(*width).ok_or_else(|| "streaming target width overflow".to_owned())
                    })?;
                    let (kind, first, second) = super::containers::stream_selector(self, &mut owners, selector.as_ref())?;
                    let segment_width = if selector.is_some() {
                        format!("llg_stream_selector_width({kind}, {first}, {second}, {element_width})")
                    } else { format!("({cursor} > {trailing} ? (uint32_t)({cursor} - {trailing}) : 0)") };
                    let segment_width = self.scalar("uint32_t", segment_width);
                    let piece = self.reserve(width, false);
                    self.line(format!("if ({segment_width}) sv4_replace(&{}, sv4_part_select({}, {cursor} - 1, {cursor} - {segment_width}));", piece.code, value.code));
                    self.line(format!("{cursor} -= {segment_width};"));
                    let function = match container.kind {
                        IrContainerKind::Dynamic => "llg_dyn_unstream_assign",
                        IrContainerKind::Queue { .. } => "llg_queue_unstream_assign",
                        IrContainerKind::Associative { .. } => return Err("associative arrays are not streaming targets".to_owned()),
                    };
                    writes.push(Write::Container { name: container.c_name, function, value: piece, kind, first, second, width: segment_width });
                }
            }
        }
        self.discard(value);
        for write in writes {
            match write {
                Write::Packed(target, value) => { self.store(&target, value, false, "0")?; self.release_target(target); }
                Write::Container { name, function, value, kind, first, second, width } => {
                    // An unselected empty segment empties the destination. A
                    // zero-width explicit selection is inert.
                    self.line(format!("if ({width} || {kind} == 0) {function}(&{name}, {}, 1, 0, {kind}, {first}, {second});", value.code));
                    self.discard(value);
                }
            }
            self.cancellation_check()?;
        }
        for value in owners { self.discard(value); }
        Ok(())
    }
}
