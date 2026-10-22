//! Runtime tasks which borrow packed operands or consume a published literal path.
use super::stores::Target;
use super::*;

impl Frame<'_, '_> {
    pub(super) fn descriptor(&mut self, expression: &IrExpr) -> Result<String, String> {
        let value = self.expression(expression)?;
        let descriptor = self.scalar("uint32_t", format!("llg_file_descriptor({})", value.code));
        self.discard(value);
        Ok(descriptor)
    }

    pub(super) fn file_control(
        &mut self,
        operation: IrFileOp,
        expression: Option<&IrExpr>,
    ) -> Result<(), String> {
        let descriptor = expression.map(|value| self.descriptor(value)).transpose()?;
        self.line(match (operation, descriptor) {
            (IrFileOp::Close, Some(value)) => format!("llg_file_close({value});"),
            (IrFileOp::Rewind, Some(value)) => format!("llg_file_rewind({value});"),
            (IrFileOp::Flush, Some(value)) => format!("llg_file_flush({value}, 0);"),
            (IrFileOp::Flush, None) => "llg_file_flush(0, 1);".to_owned(),
            _ => return Err("file control task is missing its descriptor".to_owned()),
        });
        Ok(())
    }

    pub(super) fn memory_task(&mut self, statement: &IrStmt) -> Result<(), String> {
        let IrStmt::Memory {
            write,
            path,
            text,
            view,
            radix,
            addressing,
            start,
            finish,
        } = statement
        else {
            return Err("expected a memory task".to_owned());
        };
        let array = self.ctx.model.array(view.array);
        if array.real || view.dims.is_empty() || view.dims.len() != view.strides.len() {
            return Err("memory task requires a fixed packed memory view".to_owned());
        }
        let (runtime, memory) = if array.sparse() {
            (
                if *write {
                    "llg_fixed_memory_write_view"
                } else {
                    "llg_fixed_memory_read_view"
                },
                self.fixed_array_address(view.array)?,
            )
        } else {
            (
                if *write {
                    "llg_memory_write_view"
                } else {
                    "llg_memory_read_view"
                },
                array.c_name.clone(),
            )
        };
        let element = MemoryElement {
            cells: array.total,
            width: array.elem_width,
            signed: array.signed,
            two_state: array.two_state,
        };
        self.memory_call(MemoryCall {
            runtime,
            memory,
            rows: None,
            element,
            path,
            text: text.as_ref(),
            origin: view.origin,
            selectors: &view.selectors,
            dims: &view.dims,
            strides: &view.strides,
            total: view.total,
            radix: *radix,
            addressing: *addressing,
            start: start.as_deref(),
            finish: finish.as_deref(),
        })
    }

    /// Memory file task over a packed image or a container (see
    /// `IrMemoryAccess`). An image root is resolved once, before the path.
    pub(super) fn memory_access_task(&mut self, task: &IrMemoryAccess) -> Result<(), String> {
        if task.dims.len() != task.strides.len() {
            return Err("memory task requires a fixed packed memory view".to_owned());
        }
        let mut after = MemoryImageAfter::None;
        let mut rows = None;
        let (runtime, memory, cells) = match &task.storage {
            IrMemoryStorage::Image(image) => {
                let cells = task
                    .cells()
                    .ok_or_else(|| "memory image cell count overflows".to_owned())?;
                let width = cells
                    .checked_mul(u64::from(task.element_width))
                    .and_then(|width| u32::try_from(width).ok())
                    .ok_or_else(|| "memory image width overflows".to_owned())?;
                let runtime = if task.write {
                    "llg_image_memory_write_view"
                } else {
                    "llg_image_memory_read_view"
                };
                let memory = match image {
                    IrMemoryImage::Value(value) => {
                        let value = self.operand(value)?;
                        if value.width != width {
                            return Err("memory image width disagrees with its cells".to_owned());
                        }
                        let value = if value.borrowed_address.is_some() {
                            value
                        } else {
                            self.own(value)
                        };
                        let memory = memory_image(&value.address(), 0, false);
                        after = MemoryImageAfter::Discard(value);
                        memory
                    }
                    IrMemoryImage::Target(lhs) => {
                        if let Some((root, offset)) = self.memory_image_in_place(lhs, width) {
                            let target = self.target(root)?;
                            let memory = memory_image(&target.binding.address, offset, true);
                            after = MemoryImageAfter::Release(target);
                            memory
                        } else {
                            let target = self.target(lhs)?;
                            if target.width != width {
                                return Err(
                                    "memory image width disagrees with its cells".to_owned()
                                );
                            }
                            let value = self.read_target(&target);
                            let value = self.convert(value, width, false, false, false);
                            let value = self.own(value);
                            let memory = memory_image(&value.address(), 0, false);
                            after = MemoryImageAfter::Store(target, value);
                            memory
                        }
                    }
                };
                (runtime, memory, cells)
            }
            IrMemoryStorage::Container(container)
                if matches!(
                    self.ctx.model.containers[*container].element,
                    IrContainerElement::FixedArray { .. }
                ) =>
            {
                let model_container = &self.ctx.model.containers[*container];
                let IrContainerElement::FixedArray { dimensions, .. } = &model_container.element
                else {
                    unreachable!("guarded by the match arm");
                };
                let row_cells = dimensions
                    .iter()
                    .try_fold(1u64, |total, (left, right)| {
                        total.checked_mul(u64::from(left.abs_diff(*right)) + 1)
                    })
                    .ok_or_else(|| "memory row cell count overflows".to_owned())?;
                let runtime = match (&model_container.kind, task.write) {
                    (IrContainerKind::Dynamic, false) => "llg_dyn_rows_memory_read_view",
                    (IrContainerKind::Dynamic, true) => "llg_dyn_rows_memory_write_view",
                    (IrContainerKind::Queue { .. }, false) => "llg_queue_rows_memory_read_view",
                    (IrContainerKind::Queue { .. }, true) => "llg_queue_rows_memory_write_view",
                    (IrContainerKind::Associative { .. }, false) => {
                        "llg_assoc_rows_memory_read_view"
                    }
                    (IrContainerKind::Associative { .. }, true) => {
                        "llg_assoc_rows_memory_write_view"
                    }
                };
                if task
                    .selectors
                    .first()
                    .is_none_or(|row| row.kind != IrMemorySelectorKind::Row)
                {
                    return Err("a memory over container rows needs one element".to_owned());
                }
                rows = Some(row_cells);
                let memory = format!("&{}", self.container_name(*container)?);
                (runtime, memory, row_cells)
            }
            IrMemoryStorage::Container(container) => {
                let runtime = match (&self.ctx.model.containers[*container].kind, task.write) {
                    (IrContainerKind::Dynamic, false) => "llg_dyn_memory_read_view",
                    (IrContainerKind::Dynamic, true) => "llg_dyn_memory_write_view",
                    (IrContainerKind::Queue { .. }, false) => "llg_queue_memory_read_view",
                    (IrContainerKind::Queue { .. }, true) => "llg_queue_memory_write_view",
                    (IrContainerKind::Associative { .. }, false) => "llg_assoc_memory_read_view",
                    (IrContainerKind::Associative { .. }, true) => "llg_assoc_memory_write_view",
                };
                let memory = format!("&{}", self.container_name(*container)?);
                // Zero cells: the runtime takes the extent from the current size.
                (runtime, memory, task.cells().unwrap_or(0))
            }
        };
        self.memory_call(MemoryCall {
            runtime,
            memory,
            rows,
            element: MemoryElement {
                cells,
                width: task.element_width,
                signed: task.signed,
                two_state: task.two_state,
            },
            path: &task.path,
            text: task.text.as_ref(),
            origin: task.origin,
            selectors: &task.selectors,
            dims: &task.dims,
            strides: &task.strides,
            total: task.total,
            radix: task.radix,
            addressing: task.addressing,
            start: task.start.as_ref(),
            finish: task.finish.as_ref(),
        })?;
        match after {
            MemoryImageAfter::None => {}
            MemoryImageAfter::Discard(value) => self.discard(value),
            MemoryImageAfter::Release(target) => self.release_target(target),
            MemoryImageAfter::Store(target, value) => {
                // One blocking store of the whole copy, through the target
                // resolved before the load.
                self.store(&target, value, false, "0")?;
                self.release_target(target);
            }
        }
        Ok(())
    }

    /// The stable packed cell a load can address in place, with the bit
    /// offset of the image in it: a whole variable or local that is neither
    /// a net, a reference nor a sequence local, or a member of one selected
    /// by constant in-range steps. Every other root is copied.
    fn memory_image_in_place<'l>(&self, lhs: &'l IrLhs, width: u32) -> Option<(&'l IrLhs, u32)> {
        if self.read_only_callback {
            return None;
        }
        let (root, offset) = match lhs {
            IrLhs::PackedSelect { target, steps, .. } => {
                let mut available = self.memory_image_root_width(target)?;
                let mut offset = 0u32;
                for step in steps {
                    let IrExprKind::Const(constant) = step.base.kind() else {
                        return None;
                    };
                    let low = memory_image_constant(constant)?;
                    if low.checked_add(step.width)? > available {
                        return None;
                    }
                    offset = offset.checked_add(low)?;
                    available = step.width;
                }
                (target.as_ref(), offset)
            }
            _ => (lhs, 0),
        };
        let root_width = self.memory_image_root_width(root)?;
        (offset.checked_add(width)? <= root_width).then_some((root, offset))
    }

    fn memory_image_root_width(&self, lhs: &IrLhs) -> Option<u32> {
        match lhs {
            IrLhs::Whole(index) => {
                let signal = self.ctx.model.signal(*index);
                (signal.net_driver.is_none()
                    && signal.net_alias.is_empty()
                    && signal.ty.width() != 0)
                    .then(|| signal.ty.width())
            }
            IrLhs::WholeRef {
                addr,
                width,
                shortreal,
                ..
            } => (*width != 0 && !*shortreal && !self.sequence_addresses.contains_key(addr))
                .then_some(*width),
            _ => None,
        }
    }

    /// Evaluate the path, runtime selectors and bounds of a memory task in
    /// that order, then call `call.runtime` on `call.memory`.
    fn memory_call(&mut self, call: MemoryCall<'_>) -> Result<(), String> {
        let path = self.string(call.path)?;
        let mut selector_values = Vec::with_capacity(call.selectors.len());
        for selector in call.selectors {
            let value = self.expression(&selector.value)?;
            if value.width == 0 {
                return Err("memory selector must be a packed integer".to_owned());
            }
            selector_values.push((selector, value));
        }
        // A variable slice position or a runtime queue bound replaces the
        // first view dimension's placeholder bounds (and, for a resizable
        // array, its origin and size) when the task runs.
        let resizable = call.element.cells == 0;
        let mut slice_bounds = None;
        let mut row = None;
        let mut view_total = format!("{}ULL", call.total);
        let origin = if selector_values.is_empty() {
            format!("{}ULL", call.origin)
        } else {
            let origin = self.scalar("uint64_t", format!("{}ULL", call.origin));
            let mut range_left = None;
            for (index, (selector, value)) in selector_values.iter().enumerate() {
                match selector.kind {
                    IrMemorySelectorKind::Row => row = Some(value.code.clone()),
                    IrMemorySelectorKind::Index => {
                        self.memory_index_selector(&origin, index, selector, &value.code)
                    }
                    IrMemorySelectorKind::SliceBase { width, descending } => {
                        let (left, right) = *call
                            .dims
                            .first()
                            .ok_or_else(|| "memory slice has no view dimension".to_owned())?;
                        let bounds = self.memory_slice_base(
                            &origin,
                            selector,
                            &value.code,
                            (width, descending),
                            (left, right),
                            resizable,
                        );
                        if resizable {
                            view_total = self.memory_slice_total(&bounds);
                        }
                        slice_bounds = Some(bounds);
                    }
                    IrMemorySelectorKind::SliceLeft => range_left = Some(value.code.clone()),
                    IrMemorySelectorKind::SliceRight => {
                        let left = range_left.take().ok_or_else(|| {
                            "memory slice right bound has no left bound".to_owned()
                        })?;
                        let bounds = self.memory_slice_range(&origin, &left, &value.code);
                        view_total = self.memory_slice_total(&bounds);
                        slice_bounds = Some(bounds);
                    }
                }
            }
            origin
        };
        let start_value = call
            .start
            .map(|expression| self.expression(expression))
            .transpose()?;
        let finish_value = call
            .finish
            .map(|expression| self.expression(expression))
            .transpose()?;
        // `$sreadmem*` data follows the bounds in source order. Literal-only
        // data is a C string in the model; other data is evaluated into one
        // owned string that lives until the load returns.
        let text = match call.text {
            Some(IrStringExpr::Literal(bytes)) => {
                Some((format!("{}, {}u", c_byte_string(bytes), bytes.len()), None))
            }
            Some(expression) => {
                let value = self.string(expression)?;
                Some((
                    format!("({0})->data, ({0})->len", value.address),
                    Some(value),
                ))
            }
            None => None,
        };
        let first = start_value
            .as_ref()
            .map(|value| value.code.as_str())
            .unwrap_or("(sv4_t)SV4_EMPTY");
        let last = finish_value
            .as_ref()
            .map(|value| value.code.as_str())
            .unwrap_or("(sv4_t)SV4_EMPTY");
        let radix = match call.radix {
            IrMemoryRadix::Binary => 2,
            IrMemoryRadix::Hex => 16,
        };
        let addressing = match call.addressing {
            IrMemoryAddressingPolicy::Verilog2001 => 0,
            IrMemoryAddressingPolicy::SystemVerilog2009 => 1,
        };
        // C has no empty compound literal; a runtime-sized view passes NULL.
        let dimensions = if call.dims.is_empty() {
            "NULL".to_owned()
        } else {
            let mut bounds = call
                .dims
                .iter()
                .flat_map(|(left, right)| [left.to_string(), right.to_string()])
                .collect::<Vec<_>>();
            if let Some((left, right)) = slice_bounds {
                bounds[0] = left;
                bounds[1] = right;
            }
            format!("(const int32_t[]){{ {} }}", bounds.join(", "))
        };
        let strides = if call.strides.is_empty() {
            "NULL".to_owned()
        } else {
            format!(
                "(const uint64_t[]){{ {} }}",
                call.strides
                    .iter()
                    .map(u64::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        let element = call.element;
        // A row-container call names its row and cells per row where the
        // other entry points take the cell count.
        let storage = match call.rows {
            Some(row_cells) => format!(
                "{}, {}, {row_cells}ULL",
                call.memory,
                row.as_deref().unwrap_or("(sv4_t)SV4_EMPTY")
            ),
            None => format!("{}, {}ULL", call.memory, element.cells),
        };
        // The words of `$sreadmemb/$sreadmemh` are text in the model; the
        // runtime serves them to the same parser that reads a file for the
        // duration of this one call.
        if let Some((arguments, _)) = &text {
            self.line(format!("llg_memory_text_begin({arguments});"));
        }
        self.line(format!("{}({}, {storage}, {}u, {}, {}, {dimensions}, {}, {strides}, {origin}, {view_total}, {first}, {last}, {}, {}, {addressing}, {radix});",
            call.runtime, path.take_string(), element.width,
            u8::from(element.signed), u8::from(element.two_state), call.dims.len(),
            u8::from(call.start.is_some()), u8::from(call.finish.is_some())));
        if let Some((_, value)) = text {
            self.line("llg_memory_text_end();");
            if let Some(value) = value {
                self.native_discard(value);
            }
        }
        self.native_discard(path);
        if let Some(value) = start_value {
            self.discard(value);
        }
        if let Some(value) = finish_value {
            self.discard(value);
        }
        for (_, value) in selector_values {
            self.discard(value);
        }
        Ok(())
    }

    /// Add a runtime index of a higher dimension to `origin`; an unknown or
    /// out-of-range index makes the view invalid (`UINT64_MAX`), which the
    /// runtime reports without writing.
    fn memory_index_selector(
        &mut self,
        origin: &str,
        index: usize,
        selector: &IrMemorySelector,
        value: &str,
    ) {
        let selected = self.declare("int64_t", "memory_selector_index", "0".to_owned());
        let (left, right) = (selector.left, selector.right);
        let minimum = left.min(right);
        let maximum = left.max(right);
        let offset = if left >= right {
            format!("(uint64_t)((int64_t){left} - {selected})")
        } else {
            format!("(uint64_t)({selected} - (int64_t){left})")
        };
        let offset_name = self.name(&format!("memory_offset_{index}"));
        let (offset_access, offset_declaration) =
            self.declaration_target_named("uint64_t", &offset_name);
        self.line(format!(
            "if ({origin} != UINT64_MAX) {{ if (!sv4_to_index_i64({value}, &{selected}) || {selected} < {minimum} || {selected} > {maximum}) {{ {origin} = UINT64_MAX; }} else {{ {offset_declaration} = {offset}; if ({offset_access} > (UINT64_MAX - {origin}) / {}ULL) {{ {origin} = UINT64_MAX; }} else {{ {origin} += {offset_access} * {}ULL; }} }} }}",
            selector.stride,
            selector.stride
        ));
    }

    /// Resolve the variable position of an indexed slice once (decision
    /// S27-D19) and return the C names of the view's first-dimension bounds.
    /// A fixed dimension keeps its declared direction, adds the slice offset
    /// to `origin` and makes the view invalid when the slice is unknown or
    /// not inside the declared range. A dynamic array or queue gets the
    /// ascending bounds and origin, or `-1` bounds that the runtime rejects
    /// with the size check.
    fn memory_slice_base(
        &mut self,
        origin: &str,
        selector: &IrMemorySelector,
        value: &str,
        (width, descending): (u32, bool),
        placeholder: (i32, i32),
        resizable: bool,
    ) -> (String, String) {
        let span = u64::from(width) - 1;
        let slice_left = self.declare("int32_t", "memory_slice_left", placeholder.0.to_string());
        let slice_right = self.declare("int32_t", "memory_slice_right", placeholder.1.to_string());
        let base = self.declare("int64_t", "memory_slice_base", "0".to_owned());
        // `low > high` marks an unknown position or one outside the 32-bit
        // index range; a known position inside it keeps both bounds exact in
        // 64 bits.
        let low = self.declare("int64_t", "memory_slice_low", "1".to_owned());
        let high = self.declare("int64_t", "memory_slice_high", "0".to_owned());
        let first = if descending {
            format!("{base} - {span}LL")
        } else {
            base.clone()
        };
        self.line(format!(
            "if (sv4_to_index_i64({value}, &{base}) && {base} >= INT32_MIN && {base} <= INT32_MAX) {{ {low} = {first}; {high} = {low} + {span}LL; }}"
        ));
        if resizable {
            self.line(format!(
                "if ({low} > {high} || {low} < 0 || {high} > INT32_MAX) {{ {low} = -1; {high} = -1; }}"
            ));
            self.line(format!(
                "{slice_left} = (int32_t){low}; {slice_right} = (int32_t){high}; {origin} = {low} >= 0 ? (uint64_t){low} : 0u;"
            ));
            return (slice_left, slice_right);
        }
        let (left, right) = (selector.left, selector.right);
        let minimum = left.min(right);
        let maximum = left.max(right);
        let (view_left, view_right, offset) = if left >= right {
            (&high, &low, format!("(uint64_t)((int64_t){left} - {high})"))
        } else {
            (&low, &high, format!("(uint64_t)({low} - (int64_t){left})"))
        };
        let offset_name = self.name("memory_slice_offset");
        let (offset_access, offset_declaration) =
            self.declaration_target_named("uint64_t", &offset_name);
        self.line(format!(
            "if ({origin} != UINT64_MAX) {{ if ({low} > {high} || {low} < {minimum} || {high} > {maximum}) {{ {origin} = UINT64_MAX; }} else {{ {slice_left} = (int32_t){view_left}; {slice_right} = (int32_t){view_right}; {offset_declaration} = {offset}; if ({offset_access} > (UINT64_MAX - {origin}) / {}ULL) {{ {origin} = UINT64_MAX; }} else {{ {origin} += {offset_access} * {}ULL; }} }} }}",
            selector.stride,
            selector.stride
        ));
        (slice_left, slice_right)
    }

    /// Resolve the two bounds of a queue range slice, each evaluated once in
    /// order (decision S27-D20), into the view's ascending bounds and origin.
    /// An unknown bound gives `-1` bounds; reversed bounds stay reversed.
    /// The runtime rejects both against the current size.
    fn memory_slice_range(&mut self, origin: &str, left: &str, right: &str) -> (String, String) {
        let slice_left = self.declare("int32_t", "memory_slice_left", "-1".to_owned());
        let slice_right = self.declare("int32_t", "memory_slice_right", "-1".to_owned());
        let low = self.declare("int64_t", "memory_slice_low", "0".to_owned());
        let high = self.declare("int64_t", "memory_slice_high", "0".to_owned());
        self.line(format!(
            "if (sv4_to_index_i64({left}, &{low}) && sv4_to_index_i64({right}, &{high}) && {low} >= 0 && {low} <= INT32_MAX && {high} >= 0 && {high} <= INT32_MAX) {{ {slice_left} = (int32_t){low}; {slice_right} = (int32_t){high}; {origin} = (uint64_t){low}; }}"
        ));
        (slice_left, slice_right)
    }

    /// Element count of a resizable slice whose bounds resolve at run time;
    /// the runtime rejects an empty or invalid slice before reading it.
    fn memory_slice_total(&mut self, (left, right): &(String, String)) -> String {
        self.scalar(
            "uint64_t",
            format!(
                "({left} >= 0 && {left} <= {right}) ? (uint64_t)((int64_t){right} - {left}) + 1u : 1u"
            ),
        )
    }

    pub(super) fn monitor_task(&mut self, statement: &IrStmt) -> Result<(), String> {
        let IrStmt::MonitorSet {
            strobe,
            fmt,
            eval,
            n_args,
            reads,
            poll,
            scope,
            descriptor,
            ..
        } = statement
        else {
            return Err("expected a monitor task".to_owned());
        };
        let descriptor = descriptor
            .as_ref()
            .map(|value| self.descriptor(value))
            .transpose()?;
        let scope = c_string_literal(scope);
        let prefix = if descriptor.is_some() {
            "llg_file_"
        } else {
            "llg_"
        };
        let descriptor = descriptor
            .map(|value| format!("{value}, "))
            .unwrap_or_default();
        if *strobe {
            self.line(format!(
                "{prefix}strobe_typed({descriptor}{fmt}, {n_args}, {eval}, {scope});"
            ));
        } else {
            let mut pointers = Vec::new();
            for read in reads {
                let read = match read {
                    IrDependency::PackedRange { storage, .. } => storage.as_ref(),
                    other => other,
                };
                // The monitor keeps this address after its registering scope.
                // Only model/static storage is eligible, never an automatic.
                let (kind, address) = match read {
                    IrDependency::Scalar(name) | IrDependency::Real(name) => {
                        let binding = self
                            .lookup(name)
                            .ok_or_else(|| format!("unknown monitor dependency {name}"))?;
                        if binding.automatic {
                            return Err(pending("automatic monitor dependencies"));
                        }
                        (
                            if binding.width == 0 {
                                "LLG_FMT_REAL"
                            } else {
                                "LLG_FMT_PACKED"
                            },
                            binding.address,
                        )
                    }
                    IrDependency::ArrayContents(index) => (
                        "LLG_FMT_PACKED",
                        format!("&{}_llg_contents_dep", self.ctx.model.array(*index).c_name),
                    ),
                    IrDependency::ArrayElement { array, index } => {
                        let array = self.ctx.model.array(*array);
                        if array.sparse() {
                            ("LLG_FMT_PACKED", array.cell_address(&index.to_string()))
                        } else if array.real {
                            ("LLG_FMT_REAL", format!("&{}[{index}]", array.c_name))
                        } else {
                            (
                                "LLG_FMT_PACKED",
                                format!("&{}_llg_element_deps[{index}]", array.c_name),
                            )
                        }
                    }
                    IrDependency::ContainerContents(index)
                    | IrDependency::ContainerShape(index)
                        if !self.ctx.model.containers[*index].is_global_storage() =>
                    {
                        return Err(pending(
                            "monitoring resizable containers in subroutine storage",
                        ))
                    }
                    IrDependency::ContainerContents(index) => (
                        "LLG_FMT_PACKED",
                        format!(
                            "&{}_llg_contents_dep",
                            self.ctx.model.containers[*index].c_name
                        ),
                    ),
                    IrDependency::ContainerShape(index) => (
                        "LLG_FMT_PACKED",
                        format!(
                            "&{}_llg_shape_dep",
                            self.ctx.model.containers[*index].c_name
                        ),
                    ),
                    IrDependency::Object(index)
                        if self.ctx.model.objects[*index].ty.has_change_marker() =>
                    {
                        (
                            "LLG_FMT_PACKED",
                            format!("&{}_llg_dep", self.ctx.model.objects[*index].c_name),
                        )
                    }
                    _ => return Err(pending("this monitor dependency")),
                };
                pointers.push(format!("{{ {kind}, {address} }}"));
            }
            if *poll {
                pointers.push("{ LLG_MONITOR_READ_POLL, NULL }".to_owned());
            }
            let pointers_len = pointers.len();
            let pointer = if pointers.is_empty() {
                "NULL".to_owned()
            } else {
                self.declare_array_init(
                    "llg_display_read_t",
                    "monitor_reads",
                    pointers.len(),
                    &pointers.join(", "),
                )
            };
            self.line(format!("{prefix}monitor_with_typed_reads({descriptor}{fmt}, {n_args}, {eval}, {scope}, {pointer}, {});", pointers_len));
        }
        Ok(())
    }
}

/// Cell layout of the storage a memory file task addresses.
#[derive(Clone, Copy)]
struct MemoryElement {
    cells: u64,
    width: u32,
    signed: bool,
    two_state: bool,
}

/// What follows the runtime call of an image memory task.
enum MemoryImageAfter {
    None,
    /// A dump's evaluated root.
    Discard(Value),
    /// The in-place target of a load.
    Release(Target),
    /// The copy of a load's root and the target it is stored back to.
    Store(Target, Value),
}

/// An `llg_memory_image_t` compound literal for `storage`.
fn memory_image(storage: &str, offset: u32, publish: bool) -> String {
    format!(
        "&(llg_memory_image_t){{ (sv4_t*)({storage}), {offset}u, {} }}",
        u8::from(publish)
    )
}

/// A known, non-negative constant selector that fits a packed offset.
fn memory_image_constant(constant: &IrConst) -> Option<u32> {
    if constant.real.is_some()
        || constant.fill.is_some()
        || constant.x.iter().chain(&constant.z).any(|word| *word != 0)
        || constant.bits.iter().skip(1).any(|word| *word != 0)
    {
        return None;
    }
    let low = constant.bits.first().copied().unwrap_or(0);
    if constant.signed && constant.width != 0 && constant.width <= 64 {
        let sign = 1u64 << (constant.width - 1);
        if low & sign != 0 {
            return None;
        }
    }
    u32::try_from(low).ok()
}

/// One memory file task call: runtime entry point, storage and view.
struct MemoryCall<'a> {
    runtime: &'static str,
    memory: String,
    /// The cells per row of a container of fixed arrays (one element).
    rows: Option<u64>,
    element: MemoryElement,
    path: &'a IrStringExpr,
    /// The text a load reads instead of the file `path` names.
    text: Option<&'a IrStringExpr>,
    origin: u64,
    selectors: &'a [IrMemorySelector],
    dims: &'a [(i32, i32)],
    strides: &'a [u64],
    total: u64,
    radix: IrMemoryRadix,
    addressing: IrMemoryAddressingPolicy,
    start: Option<&'a IrExpr>,
    finish: Option<&'a IrExpr>,
}

/// A C string literal of `bytes`: octal escapes keep every byte exact and
/// cannot run into a following digit.
fn c_byte_string(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() + 2);
    out.push('"');
    for &byte in bytes {
        match byte {
            b' '..=b'~' if !matches!(byte, b'"' | b'\\' | b'?') => out.push(char::from(byte)),
            _ => out.push_str(&format!("\\{byte:03o}")),
        }
    }
    out.push('"');
    out
}
