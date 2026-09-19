//! Capture fixed-value copyouts and stream an owned RHS in language order.
use super::stores::Target;
use super::*;

pub(super) enum CapturedAssignment {
    Target(Box<Target>),
    Stream {
        parts: Vec<(CapturedAssignment, u32)>,
        width: u32,
        slice: u32,
        direction: IrStreamDirection,
    },
}

impl Frame<'_, '_> {
    pub(super) fn capture_assignment(&mut self, lhs: &IrLhs) -> Result<CapturedAssignment, String> {
        if let IrLhs::Stream {
            parts,
            width,
            slice,
            direction,
        } = lhs
        {
            let parts = parts
                .iter()
                .map(|(part, width)| Ok((self.capture_assignment(part)?, *width)))
                .collect::<Result<Vec<_>, String>>()?;
            Ok(CapturedAssignment::Stream {
                parts,
                width: *width,
                slice: *slice,
                direction: *direction,
            })
        } else {
            Ok(CapturedAssignment::Target(Box::new(self.target(lhs)?)))
        }
    }

    pub(super) fn prepare_assignment(
        &mut self,
        lhs: &IrLhs,
        value: Value,
    ) -> Result<Vec<(Target, Value)>, String> {
        let target = self.capture_assignment(lhs)?;
        self.prepare_captured_assignment(target, value)
    }

    pub(super) fn prepare_captured_assignment(
        &mut self,
        target: CapturedAssignment,
        value: Value,
    ) -> Result<Vec<(Target, Value)>, String> {
        let (parts, width, slice, direction) = match target {
            CapturedAssignment::Target(target) => return Ok(vec![(*target, value)]),
            CapturedAssignment::Stream {
                parts,
                width,
                slice,
                direction,
            } => (parts, width, slice, direction),
        };
        if width == 0 || slice == 0 || parts.is_empty() {
            return Err("invalid streaming assignment shape".to_owned());
        }
        let mut remaining = width;
        let value = self.convert(value, width, false, false, false);
        let code = format!(
            "sv4_unstream({}, {slice}, {})",
            value.code,
            u8::from(direction == IrStreamDirection::RightToLeft)
        );
        let value = self.replace(value, code, width, false);
        let mut writes = Vec::new();
        for (part, part_width) in parts {
            remaining = remaining
                .checked_sub(part_width)
                .ok_or("streaming target width overflow")?;
            if part_width == 0 {
                return Err("empty streaming target part".to_owned());
            }
            let high = remaining + part_width - 1;
            let piece = self.value(
                format!("sv4_part_select({}, {high}ULL, {remaining}ULL)", value.code),
                part_width,
                false,
            );
            writes.extend(self.prepare_captured_assignment(part, piece)?);
        }
        if remaining != 0 {
            return Err("streaming target widths do not cover the source".to_owned());
        }
        self.discard(value);
        Ok(writes)
    }
}

impl Frame<'_, '_> {
    /// Snapshot the RHS, then evaluate and publish components from left to right.
    /// A with-selector can read a value unpacked by an earlier component.
    pub(super) fn stream_assignment(
        &mut self,
        source: &IrExpr,
        slice: u32,
        direction: IrStreamDirection,
        targets: &[IrStreamTarget],
    ) -> Result<(), String> {
        enum Write {
            Packed(Target, Value),
            Container {
                name: String,
                function: &'static str,
                value: Value,
                kind: i32,
                first: String,
                second: String,
                width: String,
            },
            FixedSelector {
                array: usize,
                segment: Value,
                left: String,
                right: String,
                count: String,
                segment_width: String,
                element_width: u32,
                in_bounds: String,
            },
        }
        if slice == 0 || targets.is_empty() {
            return Err("invalid streaming assignment shape".to_owned());
        }
        if targets
            .iter()
            .filter(|target| matches!(target, IrStreamTarget::Container { .. }))
            .count()
            > 1
        {
            return Err("streaming assignment supports at most one resizable target".to_owned());
        }
        let value = self.expression(source)?;
        if value.width == 0 {
            return Err("streaming source must be packed".to_owned());
        }
        let width = value.width;
        let code = format!(
            "sv4_unstream({}, {slice}, {})",
            value.code,
            u8::from(direction == IrStreamDirection::RightToLeft)
        );
        let value = self.replace(value, code, width, false);
        // The descriptor's width is authoritative for a resizable source.
        let cursor = self.scalar("int64_t", format!("(int64_t){}.width", value.code));
        let mut owners = Vec::new();
        for (position, target) in targets.iter().enumerate() {
            let mut writes = Vec::new();
            match target {
                IrStreamTarget::Packed { lhs, width } => {
                    self.line(format!("llg_stream_require_bits({cursor}, {width});"));
                    let piece = self.value(
                        format!(
                            "sv4_part_select({}, {cursor} - 1, {cursor} - {width})",
                            value.code
                        ),
                        *width,
                        false,
                    );
                    writes.extend(
                        self.prepare_assignment(lhs, piece)?
                            .into_iter()
                            .map(|(target, value)| Write::Packed(target, value)),
                    );
                    self.line(format!("{cursor} -= {width};"));
                }
                IrStreamTarget::Container {
                    container,
                    selector,
                } => {
                    let container = self.ctx.model.containers[*container].clone();
                    let Some((element_width, _, _)) = container.element.packed() else {
                        return Err("streaming target requires packed elements".to_owned());
                    };
                    let trailing =
                        targets[position + 1..]
                            .iter()
                            .try_fold(0u32, |sum, target| {
                                let IrStreamTarget::Packed { width, .. } = target else {
                                    return Err("multiple resizable streaming targets".to_owned());
                                };
                                sum.checked_add(*width)
                                    .ok_or_else(|| "streaming target width overflow".to_owned())
                            })?;
                    let (kind, first, second) =
                        super::containers::stream_selector(self, &mut owners, selector.as_ref())?;
                    let segment_width = if selector.is_some() {
                        format!(
                            "llg_stream_selector_width({kind}, {first}, {second}, {element_width})"
                        )
                    } else {
                        format!("({cursor} > {trailing} ? (uint32_t)({cursor} - {trailing}) : 0)")
                    };
                    let segment_width = self.scalar("uint32_t", segment_width);
                    self.line(format!(
                        "llg_stream_require_bits({cursor}, {segment_width});"
                    ));
                    let piece = self.reserve(width, false);
                    self.line(format!("if ({segment_width}) sv4_replace(&{}, sv4_part_select({}, {cursor} - 1, {cursor} - {segment_width}));", piece.code, value.code));
                    self.line(format!("{cursor} -= {segment_width};"));
                    let function = match container.kind {
                        IrContainerKind::Dynamic => "llg_dyn_unstream_assign",
                        IrContainerKind::Queue { .. } => "llg_queue_unstream_assign",
                        IrContainerKind::Associative { .. } => {
                            return Err("associative arrays are not streaming targets".to_owned())
                        }
                    };
                    writes.push(Write::Container {
                        name: container.c_name,
                        function,
                        value: piece,
                        kind,
                        first,
                        second,
                        width: segment_width,
                    });
                }
                IrStreamTarget::FixedSelector { array, selector } => {
                    let array_index = *array;
                    let array = self.ctx.model.array(array_index).clone();
                    if array.real || array.dims.len() != 1 || array.elem_width == 0 {
                        return Err(
                            "fixed-array streaming target requires a packed one-dimensional array"
                                .to_owned(),
                        );
                    }
                    let element_width = array.elem_width;
                    let (kind, first, second) =
                        super::containers::stream_selector(self, &mut owners, Some(selector))?;
                    let left = self.scalar("int64_t", "0".to_owned());
                    let right = self.scalar("int64_t", "0".to_owned());
                    let count = self.scalar("size_t", "0".to_owned());
                    self.line(format!(
                        "llg_fixed_stream_bounds({kind}, {first}, {second}, &{left}, &{right}, &{count});"
                    ));
                    let (declared_left, declared_right) = array.dims[0];
                    let in_bounds = self.scalar(
                        "int",
                        format!(
                            "llg_fixed_stream_target_in_bounds({declared_left}, {declared_right}, {left}, {right}, {count})"
                        ),
                    );
                    // The runtime helper validates the selected extent
                    // against the supported width limit and returns the
                    // actual selected bit count, not the index width.
                    let segment_width = self.scalar(
                        "uint32_t",
                        format!(
                            "llg_fixed_stream_width({kind}, {first}, {second}, {element_width})"
                        ),
                    );
                    self.line(format!(
                        "llg_stream_require_bits({cursor}, {segment_width});"
                    ));
                    let segment = self.reserve(crate::sim::emit_c::LLG_MAX_WIDTH, false);
                    self.line(format!(
                        "if ({segment_width}) sv4_replace(&{}, sv4_part_select({}, {cursor} - 1, {cursor} - {segment_width}));",
                        segment.code, value.code
                    ));
                    self.line(format!("{cursor} -= {segment_width};"));
                    writes.push(Write::FixedSelector {
                        array: array_index,
                        segment,
                        left,
                        right,
                        count,
                        segment_width,
                        element_width,
                        in_bounds,
                    });
                }
            }
            for write in writes {
                match write {
                    Write::Packed(target, value) => {
                        self.store(&target, value, false, "0")?;
                        self.release_target(target);
                    }
                    Write::Container {
                        name,
                        function,
                        value,
                        kind,
                        first,
                        second,
                        width,
                    } => {
                        // An unselected empty segment empties the destination. A
                        // zero-width explicit selection is inert.
                        self.line(format!("if ({width} || {kind} == 0) {function}(&{name}, {}, 1, 0, {kind}, {first}, {second});", value.code));
                        self.discard(value);
                    }
                    Write::FixedSelector {
                        array,
                        segment,
                        left,
                        right,
                        count,
                        segment_width,
                        element_width,
                        in_bounds,
                    } => {
                        // SV 11.4.14.4 requires the valid portion of an out-of-
                        // range fixed destination to be unpacked as well as an
                        // error. Mark failure before stores can invoke callbacks;
                        // array-element stores already guard invalid indices.
                        self.line(format!("if (!{in_bounds}) {{"));
                        self.line("fputs(\"llg: fixed streaming target selector is unknown, empty, or outside declared bounds\\n\", stderr);");
                        self.line("llg_rt_mark_failed();");
                        self.line("}");
                        let array_index = array;
                        let offset = self.name("fs_offset");
                        self.line(format!(
                            "for (size_t {offset} = 0; {offset} < {count}; ++{offset}) {{"
                        ));
                        // This is an owning packed value, not a C scalar. Keep it
                        // in the frame's registered temporary slots so cancellation
                        // also releases the original (the captured address clones it).
                        let index = self.value(
                            format!(
                            "sv4_from_i64(llg_fixed_stream_index_at({left}, {right}, {offset}), 64)"
                        ),
                            64,
                            true,
                        );
                        let index_name = self.name("fs_index");
                        self.bindings
                            .last_mut()
                            .expect("frame always has a binding scope")
                            .insert(
                                index_name.clone(),
                                Binding {
                                    address: format!("&{}", index.code),
                                    width: 64,
                                    signed: true,
                                    two_state: true,
                                    shortreal: false,
                                    automatic: true,
                                },
                            );
                        let lhs = IrLhs::ArrayElem {
                            arr: array_index,
                            indices: vec![IrExpr::new(
                                IrExprKind::LocalRead(index_name.clone()),
                                64,
                                true,
                                None,
                            )],
                            elem_sel: IrElemSel::Whole,
                        };
                        let target = self.target(&lhs)?;
                        let unit_high = format!(
                            "((int64_t){segment_width} - (int64_t){offset} * {element_width})"
                        );
                        let piece = self.value(
                        format!(
                            "sv4_part_select({}, ({unit_high}) - 1, ({unit_high}) - {element_width})",
                            segment.code
                        ),
                        element_width,
                        false,
                    );
                        self.store(&target, piece, false, "0")?;
                        self.release_target(target);
                        self.discard(index);
                        self.bindings
                            .last_mut()
                            .expect("frame always has a binding scope")
                            .remove(&index_name);
                        self.line("}");
                        self.discard(segment);
                    }
                }
                self.cancellation_check()?;
            }
        }
        self.discard(value);
        for value in owners {
            self.discard(value);
        }
        Ok(())
    }

    /// Materialize a fixed-array runtime `with` source as one packed value in
    /// declared stream order. The selector bounds are evaluated once and owned
    /// until the runtime helper returns; the result carries the actual
    /// selected width rather than the selector expressions' storage widths.
    pub(super) fn fixed_stream_source(
        &mut self,
        array: usize,
        selector: &IrStreamSelector,
        expression: &IrExpr,
    ) -> Result<Value, String> {
        let info = self.ctx.model.array(array).clone();
        if info.real || info.dims.len() != 1 || info.elem_width == 0 {
            return Err(
                "fixed-array streaming source requires a packed one-dimensional array".to_owned(),
            );
        }
        let (declared_left, declared_right) = info.dims[0];
        let mut owners = Vec::new();
        let (kind, first, second) =
            super::containers::stream_selector(self, &mut owners, Some(selector))?;
        let code = format!(
            "llg_fixed_stream_source({}, {declared_left}LL, {declared_right}LL, {}, {}, {kind}, {first}, {second})",
            info.c_name,
            info.elem_width,
            u8::from(info.two_state)
        );
        let result = self.value(code, expression.width, expression.signed);
        for value in owners {
            self.discard(value);
        }
        Ok(result)
    }
}
