//! Unpacked array concatenations (SV 10.10).

use super::*;
use crate::sim::codegen::lowering::collection::lower_container_element;

/// One item of an unpacked array concatenation.
pub(in crate::sim::codegen::lowering) struct ConcatItem {
    pub(in crate::sim::codegen::lowering) node: NodeId,
    /// Whether the item stands for the elements of an array, not for one
    /// element.
    pub(in crate::sim::codegen::lowering) array: bool,
}

impl Codegen<'_> {
    /// The items of `node` when it is an unpacked array concatenation. The
    /// frontend gives one the type of its target; a concatenation inside it
    /// is a vector or string concatenation (SV 10.10.3).
    pub(in crate::sim::codegen::lowering) fn unpacked_concat_items(
        &self,
        node: NodeId,
    ) -> Option<Vec<ConcatItem>> {
        let node = self.p30_unwrap_cast(node);
        let NodeKind::Expr(ExprKind::Operation {
            op: Operation::Concat,
            operands,
            reordered,
            ..
        }) = self.kind(node)
        else {
            return None;
        };
        let rank = self.unpacked_rank(node);
        if rank == 0 {
            return None;
        }
        let mut items = operands
            .iter()
            .map(|operand| ConcatItem {
                node: *operand,
                array: self.unpacked_rank(*operand) >= rank,
            })
            .collect::<Vec<_>>();
        if *reordered {
            items.reverse();
        }
        Some(items)
    }

    fn unpacked_rank(&self, node: NodeId) -> u32 {
        self.query_descriptor(node)
            .map_or(0, Self::query_unpacked_dimensions_for)
    }

    /// `{a, q, b}`: element values and arrays into a queue, a dynamic array
    /// or a fixed view. Element values are evaluated before the target
    /// changes; each run of them fills a temporary queue, as does an array
    /// that is not a queue.
    pub(super) fn lower_container_concat_into(
        &mut self,
        path: &str,
        dst: usize,
        rhs: NodeId,
    ) -> Result<Option<IrStmt>, String> {
        let Some(mut items) = self.unpacked_concat_items(rhs) else {
            return Ok(None);
        };
        if items.is_empty()
            || matches!(
                self.model.containers[dst].kind,
                IrContainerKind::Associative { .. }
            )
        {
            return Ok(None);
        }
        // A multidimensional fixed view stores its cells: each of its rows
        // is a run of them.
        let cells = self.fixed_view_dims.contains_key(&dst);
        if cells {
            for item in &mut items {
                item.array = true;
            }
        }
        let count = self.concat_count(&items, cells);
        let fixed = self
            .is_fixed_view(dst)
            .then(|| self.model.containers[dst].initial_size)
            .flatten();
        if let (Some(size), Some(count)) = (fixed, count) {
            if size != count {
                return Err(format!(
                    "unpacked array concatenation in `{path}` has {count} elements; its target has {size}"
                ));
            }
        }
        if !items.iter().any(|item| item.array) {
            let values = items.iter().map(|item| item.node).collect();
            return self
                .lower_container_source_values(path, dst, values)
                .map(Some);
        }
        let mut statements = Vec::new();
        let mut sources = Vec::new();
        let mut run = Vec::new();
        for item in items {
            if !item.array {
                run.push(item.node);
                continue;
            }
            if !run.is_empty() {
                sources.push(self.concat_run_source(
                    path,
                    dst,
                    std::mem::take(&mut run),
                    &mut statements,
                )?);
            }
            // A member of a record element read after an effect of the
            // statement is bound here, after the items before it (decision
            // EO-D4).
            if let Some(bindings) = self.take_placed_interface_bindings(item.node) {
                statements.extend(bindings);
            }
            let mut array = self
                .concat_array_source(path, dst, item.node, cells, &mut statements)
                .map_err(|error| self.locate_error(item.node, error))?;
            sources.append(&mut array);
        }
        if !run.is_empty() {
            sources.push(self.concat_run_source(path, dst, run, &mut statements)?);
        }
        if fixed.is_none()
            && matches!(
                self.model.containers[dst].kind,
                IrContainerKind::Queue { .. }
            )
        {
            statements.push(IrStmt::Container(Box::new(IrContainerStmt::QueueAssign {
                container: dst,
                sources,
            })));
            return Ok(Some(IrStmt::Block(statements)));
        }
        let queue = self.concat_queue(dst, &mut statements);
        statements.push(IrStmt::Container(Box::new(IrContainerStmt::QueueAssign {
            container: queue,
            sources,
        })));
        // A fixed view keeps its size: the queue has as many elements.
        let copy = IrStmt::Container(Box::new(IrContainerStmt::Copy { dst, src: queue }));
        statements.push(match fixed {
            Some(size) if count.is_none() => self.when_queue_has(path, rhs, queue, size, copy),
            _ => copy,
        });
        Ok(Some(IrStmt::Block(statements)))
    }

    /// The element count of a concatenation, when its items have fixed sizes.
    fn concat_count(&self, items: &[ConcatItem], cells: bool) -> Option<u64> {
        items.iter().try_fold(0u64, |count, item| {
            if !item.array {
                return count.checked_add(1);
            }
            let TypeShape::FixedArray { dimensions, .. } = &self.query_descriptor(item.node)?.shape
            else {
                return None;
            };
            let extent =
                |(left, right): &(i32, i32)| u64::from(left.abs_diff(*right)).checked_add(1);
            let elements = if cells {
                dimensions
                    .iter()
                    .try_fold(1u64, |cells, range| cells.checked_mul(extent(range)?))?
            } else {
                extent(dimensions.first()?)?
            };
            count.checked_add(elements)
        })
    }

    /// `statement` when `queue` holds `size` elements; another size is an
    /// error (SV 10.10).
    fn when_queue_has(
        &self,
        path: &str,
        rhs: NodeId,
        queue: usize,
        size: u64,
        statement: IrStmt,
    ) -> IrStmt {
        let has = IrExpr::new(
            IrExprKind::Bin {
                op: IrBinOp::Eq,
                a: Box::new(IrExpr::new(
                    IrExprKind::Container(Box::new(IrContainerExpr::Size(queue))),
                    32,
                    true,
                    None,
                )),
                b: Box::new(pattern_key_expr(i128::from(size), 32, true, false)),
            },
            1,
            false,
            None,
        );
        IrStmt::If {
            cond: has,
            then_: vec![statement],
            els: Some(vec![IrStmt::Severity {
                level: crate::sim::ir::IrSeverityLevel::Fatal,
                fmt: "\"fixed unpacked-array assignment size mismatch\"".to_owned(),
                args: Vec::new(),
                scope: path.to_owned(),
                location: self.source_location(rhs),
                fatal_finish_number: Some(0),
                origin: crate::sim::ir::IrSeverityOrigin::RuntimeFailure,
                skip_finals: false,
            }]),
            check: IrUniquePriorityCheck::None,
        }
    }

    /// A temporary queue, declared, of elements of container `like`.
    pub(super) fn concat_queue(&mut self, like: usize, statements: &mut Vec<IrStmt>) -> usize {
        let queue = self.concat_queue_of(self.model.containers[like].element.clone(), statements);
        // Record elements take their descriptor from the target's type.
        self.container_types_like.insert(queue, like);
        queue
    }

    pub(super) fn concat_queue_of(
        &mut self,
        element: IrContainerElement,
        statements: &mut Vec<IrStmt>,
    ) -> usize {
        let queue = self.model.containers.len();
        self.model.containers.push(IrContainer {
            c_name: format!("S_llg_container_{queue}"),
            element,
            kind: IrContainerKind::Queue {
                maximum_elements: None,
            },
            initial_size: None,
            activation: true,
            class_field: None,
            receiver: None,
        });
        statements.push(IrStmt::Container(Box::new(IrContainerStmt::Declare(queue))));
        queue
    }

    /// A temporary queue holding one run of concatenation element values.
    fn concat_run_source(
        &mut self,
        path: &str,
        dst: usize,
        values: Vec<NodeId>,
        statements: &mut Vec<IrStmt>,
    ) -> Result<IrQueueSource, String> {
        let queue = self.concat_queue(dst, statements);
        statements.push(self.lower_container_source_values(path, queue, values)?);
        Ok(IrQueueSource::Whole(queue))
    }

    /// The elements, or with `cells` the cells, of array item `item`.
    fn concat_array_source(
        &mut self,
        path: &str,
        dst: usize,
        item: NodeId,
        cells: bool,
        statements: &mut Vec<IrStmt>,
    ) -> Result<Vec<IrQueueSource>, String> {
        let mut descriptor = self
            .query_descriptor(item)
            .cloned()
            .ok_or_else(|| format!("unpacked array concatenation item in `{path}` has no type"))?;
        let fixed_item = matches!(descriptor.shape, TypeShape::FixedArray { .. });
        loop {
            descriptor = match descriptor.shape {
                TypeShape::FixedArray {
                    dimensions,
                    element,
                } if cells || dimensions.len() == 1 => *element,
                TypeShape::FixedArray {
                    dimensions,
                    element,
                } => TypeDescriptor {
                    shape: TypeShape::FixedArray {
                        dimensions: dimensions[1..].to_vec(),
                        element,
                    },
                    ..descriptor
                },
                TypeShape::Container { element, .. } => *element,
                _ => break,
            };
            if !cells {
                break;
            }
        }
        let element = lower_container_element(&descriptor)?;
        let packed_cells = fixed_item && element.is_packed();
        let target = &self.model.containers[dst].element;
        if !target.copies_from(&element)
            && !(element.is_object_handle() && target.is_object_handle())
        {
            return Err(format!(
                "unpacked array concatenation item in `{path}` is an array whose elements are not of the element type of the target"
            ));
        }
        if let Some(sources) = self.lower_queue_sources(path, item)? {
            return Ok(sources);
        }
        let source = if let Some(source) = self.container_of(self.p30_unwrap_cast(item)) {
            source.ir
        } else if let Some((view, start, count)) = self.fixed_view_slice(path, item)? {
            let rows = self.selection_temporary(view, item, count);
            self.model.containers[rows].initial_size = None;
            self.fixed_view_ranges.remove(&rows);
            self.fixed_view_dims.remove(&rows);
            statements.push(IrStmt::Container(Box::new(IrContainerStmt::Declare(rows))));
            statements.push(self.lower_slice_into(path, rows, view, start, count)?);
            rows
        } else if packed_cells {
            // Packed cells of a fixed array come out of its bits.
            let queue = self.concat_queue_of(element, statements);
            statements.push(self.lower_bitstream_into_container(
                path,
                item,
                &ContainerInfo { ir: queue },
            )?);
            return Ok(vec![IrQueueSource::Whole(queue)]);
        } else if let Some(value) = self.container_valued_temporary(item)? {
            statements.push(IrStmt::Container(Box::new(IrContainerStmt::Declare(value))));
            statements.push(self.lower_container_into(path, item, value, item)?);
            value
        } else {
            let queue = self.concat_queue_of(element, statements);
            statements.push(self.lower_container_into(path, item, queue, item)?);
            return Ok(vec![IrQueueSource::Whole(queue)]);
        };
        if matches!(
            self.model.containers[source].kind,
            IrContainerKind::Queue { .. }
        ) && !self.is_fixed_view(source)
        {
            return Ok(vec![IrQueueSource::Whole(source)]);
        }
        let queue = self.concat_queue_of(element, statements);
        statements.push(IrStmt::Container(Box::new(IrContainerStmt::Copy {
            dst: queue,
            src: source,
        })));
        Ok(vec![IrQueueSource::Whole(queue)])
    }
}
