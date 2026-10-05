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

/// Element storage of a fixed streaming target selected by a `with` range.
enum FixedElement {
    /// A model array: each element is an ordinary guarded array cell.
    Array(usize),
    /// A one-dimensional array represented as one packed lvalue whose left
    /// declared element occupies the most significant bits.
    Image {
        target: Box<IrLhs>,
        bounds: (i32, i32),
        two_state: bool,
    },
}

/// C scalars naming one resolved fixed `with` selection.
struct FixedSelection {
    left: String,
    right: String,
    count: String,
    in_bounds: String,
    segment_width: String,
}

/// One staged destination write of a streaming assignment.
enum StreamWrite {
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
        element: FixedElement,
        segment: Value,
        left: String,
        right: String,
        count: String,
        segment_width: String,
        element_width: u32,
        in_bounds: String,
    },
}

impl Frame<'_, '_> {
    /// Snapshot the RHS, then evaluate and publish components from left to right.
    /// A with-selector can read a value unpacked by an earlier blocking
    /// component. A nonblocking form evaluates every selector at issue and
    /// queues each write; lowering rejects selectors that read an earlier
    /// target of the same assignment.
    pub(super) fn stream_assignment(
        &mut self,
        source: &IrExpr,
        slice: u32,
        direction: IrStreamDirection,
        targets: &[IrStreamTarget],
        nba: bool,
    ) -> Result<(), String> {
        let cancellation_mark = self.cancellation_mark();
        if slice == 0 || targets.is_empty() {
            return Err("invalid streaming assignment shape".to_owned());
        }
        let containers = targets
            .iter()
            .filter(|target| matches!(target, IrStreamTarget::Container { .. }))
            .count();
        if containers > 1 {
            return Err("streaming assignment supports at most one resizable target".to_owned());
        }
        if nba && containers != 0 {
            return Err("nonblocking streaming assignment cannot resize a container".to_owned());
        }
        let value = self.expression(source)?;
        if value.width == 0 {
            return Err("streaming source must be packed".to_owned());
        }
        let width = value.width;
        let right_to_left = direction == IrStreamDirection::RightToLeft;
        let mut owners = Vec::new();
        // A `<<` unpack reorders only the bits it consumes from the left of
        // the source (SV 11.4.14.3), so their count must be known first:
        // every fixed selector is resolved before any target is written.
        // Lowering rejects a `<<` selector that reads an earlier target. A
        // resizable target consumes the remaining source either way.
        let mut resolved = Vec::new();
        let (value, cursor) = if right_to_left && containers == 0 {
            let mut total = String::from("(uint64_t)0");
            for target in targets {
                let selection = match target {
                    IrStreamTarget::Packed { width, .. } => {
                        total.push_str(&format!(" + {width}u"));
                        None
                    }
                    IrStreamTarget::FixedSelector { array, selector } => {
                        let array = self.ctx.model.array(*array).clone();
                        let bounds = array.dims.first().copied().ok_or_else(|| {
                            "fixed-array streaming target has no dimensions".to_owned()
                        })?;
                        Some(self.resolve_fixed_selector(
                            &mut owners,
                            selector,
                            bounds,
                            array.elem_width,
                        )?)
                    }
                    IrStreamTarget::FixedImageSelector {
                        bounds,
                        element_width,
                        selector,
                        ..
                    } => Some(self.resolve_fixed_selector(
                        &mut owners,
                        selector,
                        *bounds,
                        *element_width,
                    )?),
                    IrStreamTarget::Container { .. } => {
                        return Err("resizable target in a resolved stream".to_owned())
                    }
                };
                if let Some(selection) = &selection {
                    total.push_str(&format!(" + {}", selection.segment_width));
                }
                resolved.push(selection);
            }
            let total = self.scalar("uint64_t", total);
            let code = format!(
                "llg_stream_unpack_source({}, {total}, {slice}, 1)",
                value.code
            );
            let value = self.replace(value, code, width, false);
            (value, self.scalar("int64_t", format!("(int64_t){total}")))
        } else {
            let code = format!(
                "sv4_unstream({}, {slice}, {})",
                value.code,
                u8::from(right_to_left)
            );
            let value = self.replace(value, code, width, false);
            // The descriptor's width is authoritative for a resizable source.
            let cursor = self.scalar("int64_t", format!("(int64_t)llg_sv4_width({})", value.code));
            (value, cursor)
        };
        let mut resolved = resolved.into_iter();
        for (position, target) in targets.iter().enumerate() {
            let selection = resolved.next().flatten();
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
                            .map(|(target, value)| StreamWrite::Packed(target, value)),
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
                    let select = crate::sim::emit_c::destinations::assign(
                        &format!("&{}", piece.code),
                        &format!(
                            "sv4_part_select({}, {cursor} - 1, {cursor} - {segment_width})",
                            value.code
                        ),
                    );
                    self.line(format!("if ({segment_width}) {select}"));
                    self.line(format!("{cursor} -= {segment_width};"));
                    let function = match container.kind {
                        IrContainerKind::Dynamic => "llg_dyn_unstream_assign",
                        IrContainerKind::Queue { .. } => "llg_queue_unstream_assign",
                        IrContainerKind::Associative { .. } => {
                            return Err("associative arrays are not streaming targets".to_owned())
                        }
                    };
                    writes.push(StreamWrite::Container {
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
                    let selection = match selection {
                        Some(selection) => selection,
                        None => self.resolve_fixed_selector(
                            &mut owners,
                            selector,
                            array.dims[0],
                            array.elem_width,
                        )?,
                    };
                    writes.push(self.stage_fixed_selection(
                        &value,
                        &cursor,
                        selection,
                        array.elem_width,
                        FixedElement::Array(array_index),
                    ));
                }
                IrStreamTarget::FixedImageSelector {
                    target,
                    bounds,
                    element_width,
                    two_state,
                    selector,
                    two_state_runs,
                } => {
                    let selection = match selection {
                        Some(selection) => selection,
                        None => self.resolve_fixed_selector(
                            &mut owners,
                            selector,
                            *bounds,
                            *element_width,
                        )?,
                    };
                    let write = self.stage_fixed_selection(
                        &value,
                        &cursor,
                        selection,
                        *element_width,
                        FixedElement::Image {
                            target: target.clone(),
                            bounds: *bounds,
                            two_state: *two_state && two_state_runs.is_empty(),
                        },
                    );
                    if let (StreamWrite::FixedSelector { segment, .. }, false) =
                        (&write, two_state_runs.is_empty())
                    {
                        // Mixed-domain elements convert member-wise before the
                        // packed element writes, which then keep every state.
                        let runs = self.name("two_state_runs");
                        let values = two_state_runs
                            .iter()
                            .map(|(lsb, width)| format!("{lsb}u, {width}u"))
                            .collect::<Vec<_>>()
                            .join(", ");
                        self.line(format!("static const uint32_t {runs}[] = {{ {values} }};"));
                        self.line(format!(
                            "if (llg_sv4_width({})) llg_stream_segment_two_state(&{}, {element_width}u, {runs}, {}u);",
                            segment.code,
                            segment.code,
                            two_state_runs.len()
                        ));
                    }
                    writes.push(write);
                }
            }
            for write in writes {
                match write {
                    StreamWrite::Packed(target, value) => {
                        self.store(&target, value, nba, "0")?;
                        self.release_target(target);
                    }
                    StreamWrite::Container {
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
                    StreamWrite::FixedSelector {
                        element,
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
                        // only in-bounds elements are written.
                        self.line(format!("if (!{in_bounds}) {{"));
                        self.line("fputs(\"llg: fixed streaming target selector is unknown, empty, or outside declared bounds\\n\", stderr);");
                        self.line("llg_rt_mark_failed();");
                        self.line("}");
                        match element {
                            FixedElement::Image {
                                target,
                                bounds: (declared_left, declared_right),
                                ..
                            } if !nba => {
                                // One read-modify-write keeps a blocking store
                                // linear in the image width.
                                self.line(format!("if ({count}) {{"));
                                let target = self.target(&target)?;
                                let image = self.read_target(&target);
                                self.line(format!(
                                    "llg_fixed_image_stream_scatter(&{}, {}, {declared_left}LL, {declared_right}LL, {element_width}u, {left}, {right}, {count});",
                                    image.code, segment.code
                                ));
                                self.store(&target, image, false, "0")?;
                                self.release_target(target);
                                self.line("}");
                            }
                            element => self.fixed_selector_elements(
                                element,
                                &segment,
                                &left,
                                &right,
                                &count,
                                &segment_width,
                                element_width,
                                nba,
                            )?,
                        }
                        self.discard(segment);
                    }
                }
                self.cancellation_check_covering(cancellation_mark)?;
            }
        }
        self.discard(value);
        for value in owners {
            self.discard(value);
        }
        Ok(())
    }

    /// Evaluate a fixed target's `with` selector once and resolve its
    /// logical range, element count, bounds check and selected width. The
    /// selector bounds are owned by `owners` until the statement completes.
    fn resolve_fixed_selector(
        &mut self,
        owners: &mut Vec<Value>,
        selector: &IrStreamSelector,
        (declared_left, declared_right): (i32, i32),
        element_width: u32,
    ) -> Result<FixedSelection, String> {
        let (kind, first, second) =
            super::containers::stream_selector(self, owners, Some(selector))?;
        let left = self.scalar("int64_t", "0".to_owned());
        let right = self.scalar("int64_t", "0".to_owned());
        let count = self.scalar("size_t", "0".to_owned());
        self.line(format!(
            "llg_fixed_stream_bounds({kind}, {first}, {second}, {declared_left}, {declared_right}, &{left}, &{right}, &{count});"
        ));
        let in_bounds = self.scalar(
            "int",
            format!(
                "llg_fixed_stream_target_in_bounds({declared_left}, {declared_right}, {left}, {right}, {count})"
            ),
        );
        // The runtime helper validates the selected extent against the
        // supported width limit and returns the actual selected bit count,
        // not the index width.
        let segment_width = self.scalar(
            "uint32_t",
            format!("llg_fixed_stream_width({kind}, {first}, {second}, {element_width})"),
        );
        Ok(FixedSelection {
            left,
            right,
            count,
            in_bounds,
            segment_width,
        })
    }

    /// Take a resolved selection's segment from the current stream position.
    fn stage_fixed_selection(
        &mut self,
        value: &Value,
        cursor: &str,
        selection: FixedSelection,
        element_width: u32,
        element: FixedElement,
    ) -> StreamWrite {
        let FixedSelection {
            left,
            right,
            count,
            in_bounds,
            segment_width,
        } = selection;
        self.line(format!(
            "llg_stream_require_bits({cursor}, {segment_width});"
        ));
        let segment = self.reserve(crate::sim::emit_c::LLG_MAX_WIDTH, false);
        let select = crate::sim::emit_c::destinations::assign(
            &format!("&{}", segment.code),
            &format!(
                "sv4_part_select({}, {cursor} - 1, {cursor} - {segment_width})",
                value.code
            ),
        );
        self.line(format!("if ({segment_width}) {select}"));
        self.line(format!("{cursor} -= {segment_width};"));
        StreamWrite::FixedSelector {
            element,
            segment,
            left,
            right,
            count,
            segment_width,
            element_width,
            in_bounds,
        }
    }

    /// Store each in-bounds selected element of a fixed streaming target, in
    /// stream order. A nonblocking form queues one update per element with
    /// its issue-time coordinates.
    #[allow(clippy::too_many_arguments)]
    fn fixed_selector_elements(
        &mut self,
        element: FixedElement,
        segment: &Value,
        left: &str,
        right: &str,
        count: &str,
        segment_width: &str,
        element_width: u32,
        nba: bool,
    ) -> Result<(), String> {
        let (offset, offset_declaration) = self.loop_variable("size_t", "fs_offset");
        self.line(format!(
            "for ({offset_declaration} = 0; {offset} < {count}; ++{offset}) {{"
        ));
        let logical = format!("llg_fixed_stream_index_at({left}, {right}, {offset})");
        // An image element has no guarded storage of its own: skip indices
        // outside the bounds before computing its bit position.
        let (coordinate, closing) = match &element {
            FixedElement::Array(_) => (logical, ""),
            FixedElement::Image {
                bounds: (declared_left, declared_right),
                ..
            } => {
                let index = self.scalar("int64_t", logical);
                self.line(format!(
                    "if (llg_fixed_stream_storage_offset({declared_left}LL, {declared_right}LL, {index}) >= 0) {{"
                ));
                (
                    format!(
                        "llg_fixed_image_element_lsb({declared_left}LL, {declared_right}LL, {index}, {element_width}u)"
                    ),
                    "}",
                )
            }
        };
        // This is an owning packed value, not a C scalar. Keep it in the
        // frame's registered temporary slots so cancellation also releases
        // the original (the captured address clones it).
        let index = self.value(format!("sv4_from_i64({coordinate}, 64)"), 64, true);
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
        let coordinate = IrExpr::new(IrExprKind::LocalRead(index_name.clone()), 64, true, None);
        let lhs = match element {
            FixedElement::Array(array) => IrLhs::ArrayElem {
                arr: array,
                indices: vec![coordinate],
                elem_sel: IrElemSel::Whole,
            },
            FixedElement::Image {
                target, two_state, ..
            } => IrLhs::fixed_image_element(&target, coordinate, element_width, two_state),
        };
        let target = self.target(&lhs)?;
        let unit_high = format!("((int64_t){segment_width} - (int64_t){offset} * {element_width})");
        let piece = self.value(
            format!(
                "sv4_part_select({}, ({unit_high}) - 1, ({unit_high}) - {element_width})",
                segment.code
            ),
            element_width,
            false,
        );
        self.store(&target, piece, nba, "0")?;
        self.release_target(target);
        self.discard(index);
        self.bindings
            .last_mut()
            .expect("frame always has a binding scope")
            .remove(&index_name);
        if !closing.is_empty() {
            self.line(closing);
        }
        self.line("}");
        Ok(())
    }

    /// Materialize a fixed-array runtime `with` source as one packed value in
    /// declared stream order. The selector bounds are evaluated once and owned
    /// until the runtime helper returns; the result carries the actual
    /// selected width rather than the selector expressions' storage widths.
    /// Descriptor storage is read through its cells; indices outside the
    /// bounds stream the element's default-uninitialized value.
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
        let fallback = info.element_fallback();
        let fallback = self.expression(&IrExpr::new(
            IrExprKind::Const(fallback),
            info.elem_width,
            false,
            None,
        ))?;
        let mut owners = Vec::new();
        let (kind, first, second) =
            super::containers::stream_selector(self, &mut owners, Some(selector))?;
        let code = if info.sparse() {
            format!(
                "llg_fixed_array_stream_source({}, {declared_left}LL, {declared_right}LL, {}, {}, {kind}, {first}, {second})",
                self.fixed_array_address(array)?,
                info.elem_width,
                fallback.code
            )
        } else {
            format!(
                "llg_fixed_stream_source({}, {declared_left}LL, {declared_right}LL, {}, {}, {kind}, {first}, {second})",
                info.c_name, info.elem_width, fallback.code
            )
        };
        let result = self.value(code, expression.width, expression.signed);
        self.discard(fallback);
        for value in owners {
            self.discard(value);
        }
        Ok(result)
    }

    /// Materialize a `with` selection of a one-dimensional fixed array that
    /// is represented by its whole packed image. The image, fallback and
    /// selector bounds are each evaluated once, in that order.
    pub(super) fn fixed_image_stream_source(
        &mut self,
        image: &IrExpr,
        (declared_left, declared_right): (i32, i32),
        element_width: u32,
        fallback: &IrExpr,
        selector: &IrStreamSelector,
        expression: &IrExpr,
    ) -> Result<Value, String> {
        let image = self.expression(image)?;
        let fallback = self.expression(fallback)?;
        let mut owners = Vec::new();
        let (kind, first, second) =
            super::containers::stream_selector(self, &mut owners, Some(selector))?;
        let result = self.value(
            format!(
                "llg_fixed_image_stream_source({}, {declared_left}LL, {declared_right}LL, {element_width}, {}, {kind}, {first}, {second})",
                image.code, fallback.code
            ),
            expression.width,
            false,
        );
        self.discard(image);
        self.discard(fallback);
        for value in owners {
            self.discard(value);
        }
        Ok(result)
    }
}
