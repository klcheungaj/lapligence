//! Zero-delay continuous-driver self-feedback.
//!
//! A continuous assignment is re-evaluated whenever an operand changes
//! (IEEE 1364-2005 6.1.2, IEEE 1800-2009 10.3.2), including a change caused
//! by its own publication. A sensitivity loop arms its wait only after the
//! body has published, so a driver whose target is also one of its operands
//! would miss that change. Such sites are found statically; only they
//! snapshot the self-read operands and repeat the body while a snapshot
//! differs. The repeat is an ordinary `while` loop, so a nonconvergent
//! feedback cycle stops at the existing per-process zero-time step limit.

use super::*;

impl Codegen<'_> {
    /// Wrap a zero-delay continuous-driver body when the driver can change
    /// one of its own operands. Other drivers are returned unchanged.
    pub(super) fn wrap_continuous_self_feedback(
        &mut self,
        source: NodeId,
        lhs: NodeId,
        reads: &[IrDependency],
        body: Vec<IrStmt>,
    ) -> Result<Vec<IrStmt>, String> {
        if reads.is_empty() {
            return Ok(body);
        }
        let mut writes = HashSet::new();
        self.add_process_lhs_write(lhs, &mut writes);
        if writes.is_empty() {
            return Ok(body);
        }
        self.ensure_feedback_storage();
        // Different names of one true-net alias network share canonical
        // groups, so a write through one view changes a read of another.
        let mut write_groups = HashSet::new();
        for write in &writes {
            self.dependency_alias_groups(write, &mut write_groups);
        }
        let mut feedback = Vec::new();
        for read in reads {
            let direct = writes.iter().any(|write| self.same_storage(read, write));
            let aliased = !direct && !write_groups.is_empty() && {
                let mut groups = HashSet::new();
                self.dependency_alias_groups(read, &mut groups);
                groups.iter().any(|group| write_groups.contains(group))
            };
            if direct || aliased {
                feedback.push(read.clone());
            }
        }
        if feedback.is_empty() {
            return Ok(body);
        }

        let mut snapshots = Vec::new();
        let mut differences = Vec::new();
        for dependency in &feedback {
            self.feedback_snapshot(source, dependency, &mut snapshots, &mut differences);
        }
        let mut changed: Option<IrExpr> = None;
        for differs in differences {
            changed = Some(match changed {
                None => differs,
                Some(previous) => IrExpr::new(
                    IrExprKind::Bin {
                        op: IrBinOp::BitOr,
                        a: Box::new(previous),
                        b: Box::new(differs),
                    },
                    1,
                    false,
                    None,
                ),
            });
        }
        let Some(changed) = changed else {
            return Ok(body);
        };
        let again = format!("_llg_ca_again_{}", source.index());
        let again_read = IrExpr::new(IrExprKind::LocalRead(again.clone()), 1, false, None);
        let mut loop_body = snapshots;
        loop_body.extend(body);
        loop_body.push(IrStmt::Assign {
            lhs: IrLhs::WholeRef {
                addr: format!("&{again}"),
                width: 1,
                signed: false,
                two_state: false,
                shortreal: false,
            },
            rhs: changed,
            nba: false,
        });
        Ok(vec![
            IrStmt::DeclLocal {
                name: again,
                width: 1,
                signed: false,
                two_state: false,
                init: Some(Box::new(IrExpr::resize_to(lhs_integer_expr(1), 1, false))),
            },
            IrStmt::While {
                cond: again_read,
                body: loop_body,
            },
        ])
    }

    fn ensure_feedback_storage(&mut self) {
        if self.feedback_storage.is_some() {
            return;
        }
        // Dependencies name an alias view's visible cell, a resolved group
        // cell or plain storage (`signal_dependency_name`).
        let mut storage: HashMap<String, (usize, Vec<usize>)> = HashMap::new();
        for (index, signal) in self.model.signals.iter().enumerate() {
            if !matches!(signal.ty, IrType::Packed { .. }) {
                continue;
            }
            let entry = storage
                .entry(self.signal_dependency_name(index))
                .or_insert_with(|| (index, Vec::new()));
            for binding in &signal.net_alias {
                if !entry.1.contains(&binding.group()) {
                    entry.1.push(binding.group());
                }
            }
        }
        self.feedback_storage = Some(storage);
    }

    fn dependency_alias_groups(&self, dependency: &IrDependency, out: &mut HashSet<usize>) {
        let signal_groups = |signal: usize, out: &mut HashSet<usize>| {
            if let Some(signal) = self.model.signals.get(signal) {
                out.extend(signal.net_alias.iter().map(|binding| binding.group()));
            }
        };
        match dependency {
            IrDependency::Scalar(name) => {
                if let Some((_, groups)) = self
                    .feedback_storage
                    .as_ref()
                    .and_then(|storage| storage.get(name))
                {
                    out.extend(groups.iter().copied());
                }
            }
            IrDependency::PackedRange { storage, .. } => self.dependency_alias_groups(storage, out),
            IrDependency::ArrayElement { array, index } => {
                if let Some(array) = self.model.arrays.get(*array) {
                    for (element, signal) in &array.net_elements {
                        if element == index {
                            signal_groups(*signal, out);
                        }
                    }
                }
            }
            IrDependency::ArrayContents(array) => {
                if let Some(array) = self.model.arrays.get(*array) {
                    for (_, signal) in &array.net_elements {
                        signal_groups(*signal, out);
                    }
                }
            }
            IrDependency::Real(_)
            | IrDependency::ContainerContents(_)
            | IrDependency::ContainerShape(_)
            | IrDependency::Object(_)
            | IrDependency::SharedCell { .. }
            | IrDependency::RefFormal { .. }
            | IrDependency::NativeAccess(_) => {}
        }
    }

    /// Read the current value of one packed dependency.
    fn feedback_read(&self, dependency: &IrDependency) -> Option<IrExpr> {
        match dependency {
            IrDependency::Scalar(name) => {
                let (signal, _) = self.feedback_storage.as_ref()?.get(name)?;
                let IrType::Packed { width, signed, .. } = self.model.signals[*signal].ty else {
                    return None;
                };
                Some(IrExpr::new(
                    IrExprKind::SigRead(*signal),
                    width,
                    signed,
                    None,
                ))
            }
            IrDependency::PackedRange {
                storage,
                lsb,
                width,
            } => {
                let base = self.feedback_read(storage)?;
                let left = i64::from(*lsb) + i64::from(*width) - 1;
                Some(IrExpr::new(
                    IrExprKind::PartSel {
                        base: Box::new(base),
                        left,
                        right: i64::from(*lsb),
                    },
                    *width,
                    false,
                    None,
                ))
            }
            IrDependency::ArrayElement { array, index } => {
                let storage = self.model.arrays.get(*array)?;
                if storage.real {
                    return None;
                }
                // Invert the declaration-order linearization used by the
                // dependency collector into declared coordinates.
                let mut remaining = *index;
                let mut coordinates = vec![0i128; storage.dims.len()];
                for (slot, (left, right)) in storage.dims.iter().enumerate().rev() {
                    let extent = (i64::from(*left) - i64::from(*right)).unsigned_abs() + 1;
                    let offset = i128::from(remaining % extent);
                    remaining /= extent;
                    coordinates[slot] = if left >= right {
                        i128::from(*left) - offset
                    } else {
                        i128::from(*left) + offset
                    };
                }
                Some(IrExpr::new(
                    IrExprKind::ArrayRead {
                        arr: *array,
                        indices: coordinates.into_iter().map(lhs_integer_expr).collect(),
                        elem_sel: IrElemSel::Whole,
                    },
                    storage.elem_width,
                    storage.signed,
                    None,
                ))
            }
            _ => None,
        }
    }

    /// Append statements that snapshot one dependency, and one-bit
    /// expressions that are true when it no longer matches its snapshot.
    /// Real and native storage are skipped and keep the plain sensitivity
    /// behaviour.
    fn feedback_snapshot(
        &mut self,
        source: NodeId,
        dependency: &IrDependency,
        statements: &mut Vec<IrStmt>,
        differences: &mut Vec<IrExpr>,
    ) {
        if let IrDependency::ArrayContents(array) = dependency {
            let Some(storage) = self.model.arrays.get(*array) else {
                return;
            };
            if storage.real {
                return;
            }
            if !storage.sparse() {
                // Dense storage has no descriptor copy; its bounded cells
                // snapshot individually.
                for index in 0..storage.total {
                    self.feedback_snapshot(
                        source,
                        &IrDependency::ArrayElement {
                            array: *array,
                            index,
                        },
                        statements,
                        differences,
                    );
                }
                return;
            }
            // Descriptor operands snapshot into activation storage; the copy
            // and four-state comparison are descriptor operations.
            let mut snapshot = storage.clone();
            let ir = self.model.arrays.len();
            snapshot.activation = true;
            snapshot.c_name = format!("_llg_fixed_{ir}");
            snapshot.hdl_name = String::new();
            self.model.arrays.push(snapshot);
            statements.push(IrStmt::FixedArrayDeclare(ir));
            statements.push(IrStmt::FixedArrayCopy {
                dst: ir,
                src: *array,
                nba: false,
                slice: 0,
            });
            differences.push(IrExpr::new(
                IrExprKind::FixedArrayCompare {
                    left: ir,
                    right: *array,
                    case: true,
                    negate: true,
                },
                1,
                false,
                None,
            ));
            return;
        }
        let Some(read) = self.feedback_read(dependency) else {
            return;
        };
        let name = format!("_llg_ca_before_{}_{}", source.index(), differences.len());
        let (width, signed) = (read.width(), read.signed());
        statements.push(IrStmt::DeclLocal {
            name: name.clone(),
            width,
            signed,
            two_state: false,
            init: Some(Box::new(read.clone())),
        });
        differences.push(IrExpr::new(
            IrExprKind::Bin {
                op: IrBinOp::CaseNeq,
                a: Box::new(IrExpr::new(
                    IrExprKind::LocalRead(name),
                    width,
                    signed,
                    None,
                )),
                b: Box::new(read),
            },
            1,
            false,
            None,
        ));
    }
}
