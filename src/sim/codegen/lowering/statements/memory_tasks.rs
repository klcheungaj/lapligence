//! Memory file tasks: `$readmemb`, `$readmemh`, `$writememb`, `$writememh`.

use super::*;
use crate::sim::ir::{
    IrContainerElement, IrContainerKind, IrMemoryAccess, IrMemoryImage, IrMemoryStorage,
};

#[derive(Clone, Copy)]
enum MemorySliceNodes {
    Range {
        left: NodeId,
        right: NodeId,
    },
    Indexed {
        base: NodeId,
        width: NodeId,
        descending: bool,
    },
}

/// A selected memory view: see [`IrMemoryView`] for the meaning of each field.
struct MemoryGeometry {
    origin: u64,
    selectors: Vec<IrMemorySelector>,
    sliced: bool,
    dims: Vec<(i32, i32)>,
    strides: Vec<u64>,
    total: u64,
}

/// Lowered operands shared by both memory storage forms.
struct MemoryOperands {
    radix: IrMemoryRadix,
    addressing: IrMemoryAddressingPolicy,
    enum_values: Option<Vec<IrConst>>,
    start: Option<IrExpr>,
    finish: Option<IrExpr>,
}

impl EmitCtx<'_, '_> {
    pub(super) fn lower_memory_task(
        &mut self,
        name: &str,
        args: &[NodeId],
    ) -> Result<IrStmt, String> {
        if !(2..=4).contains(&args.len()) {
            return Err(format!(
                "{name} requires two to four arguments in `{}`",
                self.path
            ));
        }
        let write = matches!(name, "$writememb" | "$writememh");
        if write && self.cg.db.edition() == LanguageEdition::Verilog2001 {
            return Err(format!(
                "{name} is a SystemVerilog memory task and is not available in Verilog-2001 in `{}`",
                self.path
            ));
        }
        if !self.memory_has_model_storage(name, args[1]) {
            return self.lower_memory_access_task(name, args, write);
        }
        let path = self.cg.lower_string(&self.path, args[0])?;
        let view = self.lower_memory_view(name, args[1])?;
        let array = self.cg.model.arrays.get(view.array).ok_or_else(|| {
            format!(
                "{name} requires a fixed packed memory view in `{}`",
                self.path
            )
        })?;
        let array_rank = array.dims.len();
        if array.real {
            return Err(format!(
                "{name} does not support real or shortreal memory elements in `{}`",
                self.path
            ));
        }
        let element = self.memory_element_descriptor(args[1]).ok_or_else(|| {
            format!(
                "{name} requires packed integral or packed-struct memory elements in `{}`",
                self.path
            )
        })?;
        let operands = self.lower_memory_operands(name, args, &element, write)?;
        self.check_memory_edition(name, array_rank, view.sliced)?;
        Ok(IrStmt::Memory {
            write,
            path,
            view: Box::new(view),
            radix: operands.radix,
            addressing: operands.addressing,
            enum_values: operands.enum_values,
            start: operands.start.map(Box::new),
            finish: operands.finish.map(Box::new),
        })
    }

    /// Whether `node` names a view of model array storage (module, package,
    /// static-procedural descriptor or record-column cells), which the
    /// runtime addresses in place.
    fn memory_has_model_storage(&self, name: &str, node: NodeId) -> bool {
        self.memory_view_base(name, node).is_ok_and(|(base, _, _)| {
            self.cg.record_column_select(base).is_some() || self.cg.array_of(base).is_some()
        })
    }

    /// A memory file task over storage other than model array cells: the
    /// packed image of a subroutine or block local, `ref` formal or structure
    /// member, or a container (fixed-size class property, dynamic array or
    /// queue) addressed in place. For an image the root is lowered once (see
    /// [`IrMemoryImage`]); the emitter loads a stable packed cell in place and
    /// any other root through one copy stored back with a blocking
    /// assignment. The loading process does not suspend in between, so no
    /// other process can observe a partly loaded image (IEEE 1800-2009 4.7;
    /// IEEE 1364-2001 5.4.2).
    fn lower_memory_access_task(
        &mut self,
        name: &str,
        args: &[NodeId],
        write: bool,
    ) -> Result<IrStmt, String> {
        let target = args[1];
        let (root, selectors, slice) = self.memory_image_base(name, target)?;
        if let Some(container) = self.cg.container_of(root) {
            return self.lower_container_memory_task(
                name,
                args,
                write,
                root,
                &selectors,
                slice,
                container.ir,
            );
        }
        if self
            .cg
            .query_descriptor(root)
            .is_some_and(|descriptor| matches!(descriptor.shape, TypeShape::Container { .. }))
        {
            return Err(format!(
                "{name} requires a whole dynamic array or queue, not a selection of one, in `{}`",
                self.path
            ));
        }
        let Some(mut descriptor) = self.cg.query_descriptor(root).cloned() else {
            return Err(format!(
                "{name} requires a fixed packed memory view in `{}`",
                self.path
            ));
        };
        let mut root_dims = Vec::new();
        while let TypeShape::FixedArray {
            dimensions,
            element,
        } = descriptor.shape
        {
            root_dims.extend(dimensions);
            descriptor = *element;
        }
        if root_dims.is_empty() {
            return Err(format!(
                "{name} requires a fixed packed memory view in `{}`",
                self.path
            ));
        }
        let element = descriptor;
        if matches!(element.shape, TypeShape::Real { .. }) {
            return Err(format!(
                "{name} does not support real or shortreal memory elements in `{}`",
                self.path
            ));
        }
        let operands = self.lower_memory_operands(name, args, &element, write)?;
        let element_width = Codegen::fixed_descriptor_width(&element).ok_or_else(|| {
            format!(
                "{name} requires packed integral or packed-struct memory elements in `{}`",
                self.path
            )
        })?;
        let geometry = self.memory_geometry(name, &root_dims, &selectors, slice)?;
        self.check_memory_edition(name, root_dims.len(), geometry.sliced)?;
        let cells = Self::memory_extent_product(name, &root_dims, &self.path)?;
        let image_width = cells
            .checked_mul(u64::from(element_width))
            .filter(|width| *width <= u64::from(LLG_MAX_WIDTH))
            .and_then(|width| u32::try_from(width).ok())
            .ok_or_else(|| {
                format!(
                    "{name} memory view root exceeds the packed image capacity in `{}`",
                    self.path
                )
            })?;
        let no_image = || {
            format!(
                "{name} memory view has no packed image of its declared shape in `{}`",
                self.path
            )
        };
        // The root is lowered once, as a value for a dump and as an lvalue for
        // a load, so a selector with side effects in the root runs once.
        let image = if write {
            let value = self.cg.lower_expr(&self.path, root)?;
            if value.is_real() || value.width != image_width {
                return Err(no_image());
            }
            IrMemoryImage::Value(value)
        } else {
            let lhs = self.cg.lower_lhs(&self.path, root)?;
            if self.memory_image_lhs_width(&lhs) != Some(image_width) {
                return Err(no_image());
            }
            IrMemoryImage::Target(lhs)
        };
        let path = self.cg.lower_string(&self.path, args[0])?;
        Ok(IrStmt::MemoryAccess(Box::new(IrMemoryAccess {
            write,
            path,
            storage: IrMemoryStorage::Image(image),
            root_dims,
            element_width,
            signed: element.info.signed,
            two_state: element.two_state,
            origin: geometry.origin,
            selectors: geometry.selectors,
            sliced: geometry.sliced,
            dims: geometry.dims,
            strides: geometry.strides,
            total: geometry.total,
            radix: operands.radix,
            addressing: operands.addressing,
            enum_values: operands.enum_values,
            start: operands.start,
            finish: operands.finish,
        })))
    }

    /// Packed width of a load's root lvalue; `None` for a real cell.
    fn memory_image_lhs_width(&self, lhs: &IrLhs) -> Option<u32> {
        let width = match lhs {
            IrLhs::Whole(signal) => self.cg.model.signals.get(*signal)?.ty.width(),
            IrLhs::WholeRef { width, .. }
            | IrLhs::Ref { width, .. }
            | IrLhs::Stream { width, .. }
            | IrLhs::IdxPart(_, _, _, width, _, _) => *width,
            IrLhs::PackedSelect { steps, .. } => steps.last()?.width,
            IrLhs::TaggedSelect { steps, .. } => steps.last()?.selection.width,
            IrLhs::Bit(..) => 1,
            IrLhs::Part(_, left, right, _) => u32::try_from(left.abs_diff(*right) + 1).ok()?,
            IrLhs::ArrayElem { arr, elem_sel, .. } => match elem_sel {
                IrElemSel::Whole => self.cg.model.arrays.get(*arr)?.elem_width,
                IrElemSel::Part(left, right) => u32::try_from(left.abs_diff(*right) + 1).ok()?,
                IrElemSel::Bit(_) => 1,
                IrElemSel::Indexed { width, .. } => *width,
                IrElemSel::PackedChain(steps) => steps.last()?.width,
            },
        };
        (width != 0).then_some(width)
    }

    /// A memory file task addressing container elements in place.
    #[allow(clippy::too_many_arguments)]
    fn lower_container_memory_task(
        &mut self,
        name: &str,
        args: &[NodeId],
        write: bool,
        root: NodeId,
        selectors: &[NodeId],
        slice: Option<MemorySliceNodes>,
        container: usize,
    ) -> Result<IrStmt, String> {
        let (kind, initial_size, element_layout) = {
            let container = &self.cg.model.containers[container];
            (
                container.kind.clone(),
                container.initial_size,
                container.element.clone(),
            )
        };
        if matches!(kind, IrContainerKind::Associative { .. }) {
            return Err(format!(
                "{name} does not support associative arrays in `{}`",
                self.path
            ));
        }
        let packed_elements = || {
            format!(
                "{name} requires packed integral or packed-struct memory elements in `{}`",
                self.path
            )
        };
        let IrContainerElement::Packed {
            width,
            signed,
            two_state,
        } = element_layout
        else {
            return Err(packed_elements());
        };
        let Some(mut descriptor) = self.cg.query_descriptor(root).cloned() else {
            return Err(packed_elements());
        };
        let mut root_dims = Vec::new();
        loop {
            match descriptor.shape {
                TypeShape::FixedArray {
                    dimensions,
                    element,
                } => {
                    root_dims.extend(dimensions);
                    descriptor = *element;
                }
                TypeShape::Container { element, .. } if root_dims.is_empty() => {
                    descriptor = *element;
                    break;
                }
                _ => break,
            }
        }
        let element = descriptor;
        if Codegen::fixed_descriptor_width(&element) != Some(width) {
            return Err(packed_elements());
        }
        let operands = self.lower_memory_operands(name, args, &element, write)?;
        let geometry = if root_dims.is_empty() {
            if !selectors.is_empty() {
                return Err(format!(
                    "{name} requires a whole dynamic array or queue or a slice of one, not an element, in `{}`",
                    self.path
                ));
            }
            match slice {
                Some(slice) => self.resizable_memory_slice(name, slice)?,
                None => MemoryGeometry {
                    origin: 0,
                    selectors: Vec::new(),
                    sliced: false,
                    dims: Vec::new(),
                    strides: Vec::new(),
                    total: 0,
                },
            }
        } else {
            let cells = Self::memory_extent_product(name, &root_dims, &self.path)?;
            if initial_size != Some(cells) {
                return Err(format!(
                    "{name} fixed memory property storage disagrees with its declaration in `{}`",
                    self.path
                ));
            }
            self.memory_geometry(name, &root_dims, selectors, slice)?
        };
        self.check_memory_edition(name, root_dims.len(), geometry.sliced)?;
        let path = self.cg.lower_string(&self.path, args[0])?;
        Ok(IrStmt::MemoryAccess(Box::new(IrMemoryAccess {
            write,
            path,
            storage: IrMemoryStorage::Container(container),
            root_dims,
            element_width: width,
            signed,
            two_state,
            origin: geometry.origin,
            selectors: geometry.selectors,
            sliced: geometry.sliced,
            dims: geometry.dims,
            strides: geometry.strides,
            total: geometry.total,
            radix: operands.radix,
            addressing: operands.addressing,
            enum_values: operands.enum_values,
            start: operands.start,
            finish: operands.finish,
        })))
    }

    /// A constant slice of a dynamic array or queue, addressed in place like a
    /// fixed-array slice: its indices keep the container's ascending
    /// `[0:size-1]` direction whatever the slice spelling, and the runtime
    /// checks them against the current size (decision S27-D14).
    fn resizable_memory_slice(
        &self,
        name: &str,
        slice: MemorySliceNodes,
    ) -> Result<MemoryGeometry, String> {
        let (left, right) = self.memory_slice_bounds(name, slice)?;
        let (low, high) = (left.min(right), left.max(right));
        let low = i32::try_from(low).ok().filter(|low| *low >= 0);
        let high = i32::try_from(high).ok();
        let (Some(low), Some(high)) = (low, high) else {
            return Err(format!(
                "{name} memory slice bound is outside every dynamic array or queue index in `{}`",
                self.path
            ));
        };
        Ok(MemoryGeometry {
            origin: u64::from(low.unsigned_abs()),
            selectors: Vec::new(),
            sliced: true,
            dims: vec![(low, high)],
            strides: vec![1],
            total: u64::from(high.abs_diff(low)) + 1,
        })
    }

    /// Element checks, start/finish bounds, enum members, radix and default
    /// addressing shared by both storage forms.
    fn lower_memory_operands(
        &mut self,
        name: &str,
        args: &[NodeId],
        element: &TypeDescriptor,
        write: bool,
    ) -> Result<MemoryOperands, String> {
        if !matches!(
            element.shape,
            TypeShape::PackedAtom { .. }
                | TypeShape::Aggregate(AggregateLayout {
                    kind: AggregateKind::PackedStruct | AggregateKind::PackedUnion,
                    ..
                })
        ) {
            return Err(format!(
                "{name} requires packed integral or packed-struct memory elements in `{}`",
                self.path
            ));
        }
        let mut lower_bound = |node| {
            let value = self.cg.lower_expr(&self.path, node)?;
            if value.is_real() {
                return Err(format!(
                    "{name} start/finish bounds must be packed integers in `{}`",
                    self.path
                ));
            }
            Ok(value)
        };
        let start = args.get(2).copied().map(&mut lower_bound).transpose()?;
        let finish = args.get(3).copied().map(&mut lower_bound).transpose()?;
        let enum_values = if write || element.info.kind != "enum" {
            None
        } else {
            let metadata = self.cg.db.enum_type_metadata(element.id).ok_or_else(|| {
                format!(
                    "{name} enum memory element has no owned enum metadata in `{}`",
                    self.path
                )
            })?;
            Some(
                metadata
                    .members
                    .iter()
                    .map(|member| match &member.value {
                        Val::Bits(value) => val_to_const(value),
                        _ => Err(format!(
                            "{name} enum member `{}` has a non-integral value in `{}`",
                            member.name, self.path
                        )),
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            )
        };
        let radix = match name {
            "$readmemb" | "$writememb" => IrMemoryRadix::Binary,
            "$readmemh" | "$writememh" => IrMemoryRadix::Hex,
            _ => unreachable!(),
        };
        let addressing = match self.cg.db.edition() {
            LanguageEdition::Verilog2001 => IrMemoryAddressingPolicy::Verilog2001,
            LanguageEdition::SystemVerilog2009 => IrMemoryAddressingPolicy::SystemVerilog2009,
        };
        Ok(MemoryOperands {
            radix,
            addressing,
            enum_values,
            start,
            finish,
        })
    }

    fn check_memory_edition(&self, name: &str, rank: usize, sliced: bool) -> Result<(), String> {
        if rank > 1 && self.cg.db.edition() == LanguageEdition::Verilog2001 {
            return Err(format!(
                "{name} multidimensional memory views require SystemVerilog-2009 in `{}`",
                self.path
            ));
        }
        if sliced && self.cg.db.edition() == LanguageEdition::Verilog2001 {
            return Err(format!(
                "{name} memory slices require SystemVerilog-2009 in `{}`",
                self.path
            ));
        }
        Ok(())
    }

    /// Split an image-represented memory view into its root fixed array and
    /// the selectors and optional slice applied to it, outermost first.
    fn memory_image_base(
        &self,
        name: &str,
        node: NodeId,
    ) -> Result<(NodeId, Vec<NodeId>, Option<MemorySliceNodes>), String> {
        let fixed_base = |base: NodeId| {
            self.cg
                .query_descriptor(base)
                .is_some_and(|descriptor| matches!(descriptor.shape, TypeShape::FixedArray { .. }))
        };
        // A slice of a whole dynamic array or queue addresses it in place.
        let sliceable_base =
            |base: NodeId| fixed_base(base) || self.cg.container_of(base).is_some();
        let no_selector_after_slice = |slice: &Option<MemorySliceNodes>| {
            if slice.is_some() {
                Err(format!(
                    "{name} memory slice cannot be followed by another selector in `{}`",
                    self.path
                ))
            } else {
                Ok(())
            }
        };
        let single_slice = |slice: &Option<MemorySliceNodes>| {
            if slice.is_some() {
                Err(format!(
                    "{name} memory view contains more than one slice in `{}`",
                    self.path
                ))
            } else {
                Ok(())
            }
        };
        match self.cg.kind(node) {
            NodeKind::Expr(ExprKind::BitSelect { base, index }) if fixed_base(*base) => {
                let (root, mut selectors, slice) = self.memory_image_base(name, *base)?;
                no_selector_after_slice(&slice)?;
                selectors.push(*index);
                Ok((root, selectors, None))
            }
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) if fixed_base(*base) => {
                let (root, mut selectors, slice) = self.memory_image_base(name, *base)?;
                no_selector_after_slice(&slice)?;
                selectors.extend(indices.iter().copied());
                Ok((root, selectors, None))
            }
            NodeKind::Expr(ExprKind::PartSelect { base, left, right }) if sliceable_base(*base) => {
                let (root, selectors, slice) = self.memory_image_base(name, *base)?;
                single_slice(&slice)?;
                Ok((
                    root,
                    selectors,
                    Some(MemorySliceNodes::Range {
                        left: *left,
                        right: *right,
                    }),
                ))
            }
            NodeKind::Expr(ExprKind::IndexedPartSelect {
                base,
                base_expr,
                width_expr,
                neg,
            }) if sliceable_base(*base) => {
                let (root, selectors, slice) = self.memory_image_base(name, *base)?;
                single_slice(&slice)?;
                Ok((
                    root,
                    selectors,
                    Some(MemorySliceNodes::Indexed {
                        base: *base_expr,
                        width: *width_expr,
                        descending: *neg,
                    }),
                ))
            }
            // The frontend presents the memory argument as the target of an
            // implicit assignment.
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Assignment,
                operands,
                ..
            }) if !operands.is_empty() => self.memory_image_base(name, operands[0]),
            _ => Ok((node, Vec::new(), None)),
        }
    }

    /// Capture a fixed memory view. SystemVerilog permits constant or runtime
    /// higher-dimension indices and a constant slice on the lowest specified
    /// dimension. At least one unpacked dimension remains addressable.
    pub(in super::super) fn lower_memory_view(
        &mut self,
        name: &str,
        node: NodeId,
    ) -> Result<IrMemoryView, String> {
        let (base, selectors, slice) = self.memory_view_base(name, node)?;
        let array = self
            .cg
            .record_column_select(base)
            .map(|(column, _)| column)
            .or_else(|| self.cg.array_of(base))
            .cloned()
            .ok_or_else(|| {
                if self.cg.container_of(node).is_some() {
                    format!(
                    "{name} does not support dynamic arrays, queues, or associative arrays in `{}`",
                    self.path
                )
                } else {
                    format!(
                        "{name} requires a fixed packed memory view in `{}`",
                        self.path
                    )
                }
            })?;
        if array.is_net {
            return Err(format!(
                "{name} requires a variable memory, not a net, in `{}`",
                self.path
            ));
        }
        let geometry = self.memory_geometry(name, &array.dims, &selectors, slice)?;
        Ok(IrMemoryView {
            array: array.ir,
            origin: geometry.origin,
            selectors: geometry.selectors,
            sliced: geometry.sliced,
            dims: geometry.dims,
            strides: geometry.strides,
            total: geometry.total,
        })
    }

    /// Selected view of a source with unpacked dimensions `source_dims`:
    /// constant selectors fold into the origin, runtime selectors are lowered
    /// once, and an optional constant slice of the next dimension keeps the
    /// source declaration's direction. At least one dimension remains.
    fn memory_geometry(
        &mut self,
        name: &str,
        source_dims: &[(i32, i32)],
        selectors: &[NodeId],
        slice: Option<MemorySliceNodes>,
    ) -> Result<MemoryGeometry, String> {
        if selectors.len() >= source_dims.len() {
            return Err(format!(
                "{name} memory view must retain at least one unpacked dimension in `{}`",
                self.path
            ));
        }
        let source_strides = Self::memory_dimension_strides(name, source_dims, &self.path)?;
        let mut origin = 0u64;
        let mut runtime_selectors = Vec::new();
        for (dimension, selector) in selectors.iter().enumerate() {
            let (left, right) = source_dims[dimension];
            if let Ok(value) = self.cg.eval_bound_i128(*selector) {
                let value = i32::try_from(value).map_err(|_| {
                    format!(
                        "{name} memory view selector is outside the supported index range in `{}`",
                        self.path
                    )
                })?;
                Self::validate_memory_index(name, dimension, value, (left, right), &self.path)?;
                let offset = Self::memory_dimension_offset(left, right, value);
                origin = origin
                    .checked_add(offset.checked_mul(source_strides[dimension]).ok_or_else(
                        || format!("{name} memory view offset overflow in `{}`", self.path),
                    )?)
                    .ok_or_else(|| {
                        format!("{name} memory view offset overflow in `{}`", self.path)
                    })?;
            } else {
                let value = self.cg.lower_expr(&self.path, *selector)?;
                if value.is_real() || value.width == 0 {
                    return Err(format!(
                        "{name} memory view selectors must be packed integral indices in `{}`",
                        self.path
                    ));
                }
                runtime_selectors.push(IrMemorySelector {
                    dimension,
                    left,
                    right,
                    stride: source_strides[dimension],
                    value,
                });
            }
        }
        let sliced = slice.is_some();
        let dims = if let Some(slice) = slice {
            let dimension = selectors.len();
            let (left, right) = source_dims[dimension];
            let (slice_left, slice_right) = self.memory_slice_bounds(name, slice)?;
            let slice_left = i32::try_from(slice_left).map_err(|_| {
                format!(
                    "{name} memory slice bound is outside the supported index range in `{}`",
                    self.path
                )
            })?;
            let slice_right = i32::try_from(slice_right).map_err(|_| {
                format!(
                    "{name} memory slice bound is outside the supported index range in `{}`",
                    self.path
                )
            })?;
            Self::validate_memory_index(name, dimension, slice_left, (left, right), &self.path)?;
            Self::validate_memory_index(name, dimension, slice_right, (left, right), &self.path)?;
            // A memory file addresses array indices. Keep the selected
            // interval in the source declaration's physical direction so
            // ascending/descending slice spelling cannot reverse file order.
            let (view_left, view_right) = if left >= right {
                (slice_left.max(slice_right), slice_left.min(slice_right))
            } else {
                (slice_left.min(slice_right), slice_left.max(slice_right))
            };
            let offset = Self::memory_dimension_offset(left, right, view_left);
            origin =
                origin
                    .checked_add(offset.checked_mul(source_strides[dimension]).ok_or_else(
                        || format!("{name} memory view offset overflow in `{}`", self.path),
                    )?)
                    .ok_or_else(|| {
                        format!("{name} memory view offset overflow in `{}`", self.path)
                    })?;
            let mut selected = vec![(view_left, view_right)];
            selected.extend_from_slice(&source_dims[dimension + 1..]);
            selected
        } else {
            source_dims[selectors.len()..].to_vec()
        };
        if dims.is_empty() {
            return Err(format!(
                "{name} memory view must retain at least one unpacked dimension in `{}`",
                self.path
            ));
        }
        let dimensions_start = source_dims.len() - dims.len();
        let strides = source_strides[dimensions_start..].to_vec();
        let total = Self::memory_extent_product(name, &dims, &self.path)?;
        Ok(MemoryGeometry {
            origin,
            selectors: runtime_selectors,
            sliced,
            dims,
            strides,
            total,
        })
    }

    fn memory_extent_product(name: &str, dims: &[(i32, i32)], path: &str) -> Result<u64, String> {
        dims.iter().try_fold(1u64, |total, &(left, right)| {
            let extent = (i64::from(left) - i64::from(right)).unsigned_abs() + 1;
            total
                .checked_mul(extent)
                .ok_or_else(|| format!("{name} memory view extent overflow in `{path}`"))
        })
    }

    fn memory_view_base(
        &self,
        name: &str,
        node: NodeId,
    ) -> Result<(NodeId, Vec<NodeId>, Option<MemorySliceNodes>), String> {
        if self.cg.array_of(node).is_some() {
            return Ok((node, Vec::new(), None));
        }
        match self.cg.kind(node) {
            NodeKind::Array { .. } => Ok((node, Vec::new(), None)),
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => {
                if self.cg.array_of(*target).is_some() {
                    Ok((*target, Vec::new(), None))
                } else {
                    self.memory_view_base(name, *target)
                }
            }
            NodeKind::Expr(ExprKind::BitSelect { base, index })
                if self.cg.query_descriptor(*base).is_some_and(|descriptor| {
                    matches!(descriptor.shape, TypeShape::FixedArray { .. })
                }) =>
            {
                let (root, mut selectors, slice) = self.memory_view_base(name, *base)?;
                if slice.is_some() {
                    return Err("fixed slice cannot be followed by an index".into());
                }
                selectors.push(*index);
                Ok((root, selectors, None))
            }
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                if let Some((_, indices)) = self.cg.record_column_select(node) {
                    // The select itself locates its record column.
                    return Ok((node, indices.to_vec(), None));
                }
                let (root, mut selectors, slice) = self.memory_view_base(name, *base)?;
                if slice.is_some() {
                    return Err(format!(
                        "{name} memory slice cannot be followed by another selector in `{}`",
                        self.path
                    ));
                }
                selectors.extend(indices.iter().copied());
                Ok((root, selectors, None))
            }
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => {
                let target = refs
                    .first()
                    .copied()
                    .flatten()
                    .or_else(|| refs.last().copied().flatten());
                target
                    .and_then(|target| self.cg.array_of(target).map(|_| (target, Vec::new(), None)))
                    .ok_or_else(|| {
                        format!(
                            "{} does not resolve to a fixed memory view in `{}`",
                            self.cg.node(node).name,
                            self.path
                        )
                    })
            }
            NodeKind::Expr(ExprKind::PartSelect { base, left, right })
                if self.cg.query_descriptor(*base).is_some_and(|descriptor| {
                    matches!(descriptor.shape, TypeShape::FixedArray { .. })
                }) =>
            {
                let (root, selectors, prior_slice) = self.memory_view_base(name, *base)?;
                if prior_slice.is_some() {
                    return Err(format!(
                        "{name} memory view contains more than one slice in `{}`",
                        self.path
                    ));
                }
                Ok((
                    root,
                    selectors,
                    Some(MemorySliceNodes::Range {
                        left: *left,
                        right: *right,
                    }),
                ))
            }
            NodeKind::Expr(ExprKind::IndexedPartSelect {
                base,
                base_expr,
                width_expr,
                neg,
            }) if self.cg.query_descriptor(*base).is_some_and(|descriptor| {
                matches!(descriptor.shape, TypeShape::FixedArray { .. })
            }) =>
            {
                let (root, selectors, prior_slice) = self.memory_view_base(name, *base)?;
                if prior_slice.is_some() {
                    return Err(format!(
                        "{name} memory view contains more than one slice in `{}`",
                        self.path
                    ));
                }
                Ok((
                    root,
                    selectors,
                    Some(MemorySliceNodes::Indexed {
                        base: *base_expr,
                        width: *width_expr,
                        descending: *neg,
                    }),
                ))
            }
            NodeKind::Expr(ExprKind::Cast { operand, .. }) => self.memory_view_base(name, *operand),
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::Assignment,
                operands,
                ..
            }) => operands.first().copied().map_or_else(
                || {
                    Err(format!(
                        "{} requires a fixed memory view in `{}`",
                        self.cg.node(node).name,
                        self.path
                    ))
                },
                |operand| self.memory_view_base(name, operand),
            ),
            _ => Err(format!(
                "{} requires a fixed memory view in `{}`",
                self.cg.node(node).name,
                self.path
            )),
        }
    }

    fn memory_slice_bounds(
        &self,
        name: &str,
        slice: MemorySliceNodes,
    ) -> Result<(i128, i128), String> {
        match slice {
            MemorySliceNodes::Range { left, right } => Ok((
                self.cg.eval_bound_i128(left).map_err(|_| {
                    format!(
                        "{name} memory slice bounds must be constant integers in `{}`",
                        self.path
                    )
                })?,
                self.cg.eval_bound_i128(right).map_err(|_| {
                    format!(
                        "{name} memory slice bounds must be constant integers in `{}`",
                        self.path
                    )
                })?,
            )),
            MemorySliceNodes::Indexed {
                base,
                width,
                descending,
            } => {
                let base = self.cg.eval_bound_i128(base).map_err(|_| {
                    format!(
                        "{name} memory slice bounds must be constant integers in `{}`",
                        self.path
                    )
                })?;
                let width = self.cg.eval_bound_i128(width).map_err(|_| {
                    format!(
                        "{name} indexed memory slice width must be constant in `{}`",
                        self.path
                    )
                })?;
                if width <= 0 {
                    return Err(format!(
                        "{name} indexed memory slice width must be positive in `{}`",
                        self.path
                    ));
                }
                let extension = width.checked_sub(1).ok_or_else(|| {
                    format!(
                        "{name} indexed memory slice width overflows in `{}`",
                        self.path
                    )
                })?;
                if descending {
                    Ok((
                        base,
                        base.checked_sub(extension).ok_or_else(|| {
                            format!(
                                "{name} indexed memory slice bound overflows in `{}`",
                                self.path
                            )
                        })?,
                    ))
                } else {
                    Ok((
                        base.checked_add(extension).ok_or_else(|| {
                            format!(
                                "{name} indexed memory slice bound overflows in `{}`",
                                self.path
                            )
                        })?,
                        base,
                    ))
                }
            }
        }
    }

    fn memory_dimension_strides(
        name: &str,
        dims: &[(i32, i32)],
        path: &str,
    ) -> Result<Vec<u64>, String> {
        let mut strides = vec![1u64; dims.len()];
        for dimension in (0..dims.len().saturating_sub(1)).rev() {
            let (left, right) = dims[dimension + 1];
            let extent = (i64::from(left) - i64::from(right)).unsigned_abs() + 1;
            strides[dimension] = strides[dimension + 1]
                .checked_mul(extent)
                .ok_or_else(|| format!("{name} memory view stride overflow in `{path}`"))?;
        }
        Ok(strides)
    }

    fn memory_dimension_offset(left: i32, right: i32, index: i32) -> u64 {
        if left >= right {
            (i64::from(left) - i64::from(index)) as u64
        } else {
            (i64::from(index) - i64::from(left)) as u64
        }
    }

    fn validate_memory_index(
        name: &str,
        dimension: usize,
        index: i32,
        bounds: (i32, i32),
        path: &str,
    ) -> Result<(), String> {
        let (left, right) = bounds;
        if index < left.min(right) || index > left.max(right) {
            return Err(format!(
                "{name} memory view selector {index} is outside dimension {dimension} in `{path}`"
            ));
        }
        Ok(())
    }

    fn memory_element_descriptor(&self, node: NodeId) -> Option<TypeDescriptor> {
        let (base, _, _) = self.memory_view_base("memory task", node).ok()?;
        let mut descriptor = self
            .cg
            .query_descriptor(node)
            .or_else(|| self.cg.query_descriptor(base))?
            .clone();
        loop {
            match descriptor.shape {
                TypeShape::FixedArray { element, .. } => descriptor = *element,
                _ => return Some(descriptor),
            }
        }
    }
}
