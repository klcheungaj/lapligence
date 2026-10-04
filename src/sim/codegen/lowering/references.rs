//! References.

use super::*;
use crate::sim::ir::IrPackedSelect;

impl<'a> Codegen<'a> {
    /// Resolve module references to canonical storage with selection steps
    /// relative to each immediate value, preserving intermediate bounds.
    pub(in super::super) fn reference_lhs(&self, lhs: IrLhs) -> Result<IrLhs, String> {
        fn resolve(
            cg: &Codegen<'_>,
            lhs: IrLhs,
            seen: &mut HashSet<usize>,
        ) -> Result<IrLhs, String> {
            match lhs {
                IrLhs::PackedSelect {
                    target,
                    steps,
                    signed,
                    two_state,
                } => {
                    let target = normalize_projection(resolve(cg, *target, seen)?)?;
                    if let IrLhs::PackedSelect {
                        target,
                        steps: mut prefix,
                        two_state: parent_state,
                        ..
                    } = target
                    {
                        prefix.extend(steps);
                        Ok(IrLhs::PackedSelect {
                            target,
                            steps: prefix,
                            signed,
                            two_state: two_state || parent_state,
                        })
                    } else {
                        Ok(IrLhs::PackedSelect {
                            target: Box::new(target),
                            steps,
                            signed,
                            two_state,
                        })
                    }
                }
                IrLhs::TaggedSelect {
                    target,
                    mut steps,
                    signed,
                    two_state,
                    location,
                } => {
                    let target = normalize_projection(resolve(cg, *target, seen)?)?;
                    match target {
                        IrLhs::TaggedSelect {
                            target,
                            steps: mut prefix,
                            two_state: parent_state,
                            ..
                        } => {
                            prefix.append(&mut steps);
                            Ok(IrLhs::TaggedSelect {
                                target,
                                steps: prefix,
                                signed,
                                two_state: two_state || parent_state,
                                location,
                            })
                        }
                        IrLhs::PackedSelect {
                            target,
                            steps: prefix,
                            two_state: parent_state,
                            ..
                        } => {
                            let mut combined = prefix
                                .into_iter()
                                .map(|selection| crate::sim::ir::IrTaggedSelectStep {
                                    selection,
                                    guard: None,
                                    two_state: false,
                                })
                                .collect::<Vec<_>>();
                            combined.append(&mut steps);
                            Ok(IrLhs::TaggedSelect {
                                target,
                                steps: combined,
                                signed,
                                two_state: two_state || parent_state,
                                location,
                            })
                        }
                        target => Ok(IrLhs::TaggedSelect {
                            target: Box::new(target),
                            steps,
                            signed,
                            two_state,
                            location,
                        }),
                    }
                }
                IrLhs::Whole(index) => {
                    let Some(target) = cg.reference_signals.get(&index).cloned() else {
                        return Ok(IrLhs::Whole(index));
                    };
                    if !seen.insert(index) {
                        return Err("cyclic reference port storage".to_owned());
                    }
                    let resolved = resolve(cg, target, seen).and_then(normalize_projection);
                    seen.remove(&index);
                    resolved
                }
                IrLhs::Bit(index, expression, two_state) => {
                    let base = resolve(cg, IrLhs::Whole(index), seen)?;
                    compose_reference_bit(base, *expression, two_state)
                }
                IrLhs::Part(index, left, right, two_state) => {
                    let base = resolve(cg, IrLhs::Whole(index), seen)?;
                    compose_reference_part(base, left, right, two_state)
                }
                IrLhs::IdxPart(index, base, width, selected_width, negative, two_state) => {
                    let target = normalize_projection(resolve(cg, IrLhs::Whole(index), seen)?)?;
                    match target {
                        IrLhs::PackedSelect {
                            target,
                            mut steps,
                            two_state: state,
                            ..
                        } => {
                            let base = indexed_projection(*base, selected_width, negative)?.base;
                            steps.push(crate::sim::ir::IrPackedSelect {
                                base,
                                width: selected_width,
                            });
                            Ok(IrLhs::PackedSelect {
                                target,
                                steps,
                                signed: false,
                                two_state: two_state || state,
                            })
                        }
                        IrLhs::TaggedSelect {
                            target,
                            mut steps,
                            two_state: state,
                            location,
                            ..
                        } => {
                            let base = indexed_projection(*base, selected_width, negative)?.base;
                            steps.push(crate::sim::ir::IrTaggedSelectStep {
                                selection: crate::sim::ir::IrPackedSelect {
                                    base,
                                    width: selected_width,
                                },
                                two_state,
                                guard: None,
                            });
                            Ok(IrLhs::TaggedSelect {
                                target,
                                steps,
                                signed: false,
                                two_state: two_state || state,
                                location,
                            })
                        }
                        IrLhs::Whole(index) => Ok(IrLhs::IdxPart(
                            index,
                            base,
                            width,
                            selected_width,
                            negative,
                            two_state,
                        )),
                        IrLhs::ArrayElem {
                            arr,
                            indices,
                            elem_sel: IrElemSel::Whole,
                        } => Ok(IrLhs::ArrayElem {
                            arr,
                            indices,
                            elem_sel: IrElemSel::Indexed {
                                base,
                                width: selected_width,
                                negative,
                            },
                        }),
                        _ => Err(
                            "nested indexed selection through a reference port is not supported"
                                .to_owned(),
                        ),
                    }
                }
                IrLhs::ArrayElem {
                    arr,
                    indices,
                    elem_sel,
                } => {
                    let arr = cg.reference_array(arr);
                    let array = &cg.model.arrays[arr];
                    let info = ArrayInfo {
                        global: array.c_name.clone(),
                        elem_width: array.elem_width,
                        signed: array.signed,
                        real: array.real,
                        shortreal: array.shortreal,
                        is_net: true,
                        dims: array.dims.clone(),
                        init: None,
                        ir: arr,
                    };
                    if let Some(index) = Codegen::array_constant_linear_index(&info, &indices) {
                        if let Some((_, signal)) = array
                            .net_elements
                            .iter()
                            .find(|(element, _)| *element == index)
                        {
                            return Ok(match elem_sel {
                                IrElemSel::Whole => IrLhs::Whole(*signal),
                                IrElemSel::Bit(index) => IrLhs::Bit(*signal, index, false),
                                IrElemSel::Part(left, right) => {
                                    IrLhs::Part(*signal, left, right, false)
                                }
                                IrElemSel::Indexed {
                                    base,
                                    width,
                                    negative,
                                } => IrLhs::IdxPart(
                                    *signal,
                                    base,
                                    Box::new(lhs_integer_expr(i128::from(width))),
                                    width,
                                    negative,
                                    false,
                                ),
                                IrElemSel::PackedChain(steps) => IrLhs::PackedSelect {
                                    target: Box::new(IrLhs::Whole(*signal)),
                                    steps,
                                    signed: false,
                                    two_state: false,
                                },
                            });
                        }
                    }
                    Ok(IrLhs::ArrayElem {
                        arr,
                        indices,
                        elem_sel,
                    })
                }
                IrLhs::Stream {
                    parts,
                    width,
                    slice,
                    direction,
                } => Ok(IrLhs::Stream {
                    parts: parts
                        .into_iter()
                        .map(|(part, part_width)| Ok((resolve(cg, part, seen)?, part_width)))
                        .collect::<Result<Vec<_>, String>>()?,
                    width,
                    slice,
                    direction,
                }),
                other => Ok(other),
            }
        }

        fn normalize_projection(lhs: IrLhs) -> Result<IrLhs, String> {
            let (target, steps, two_state) = match lhs {
                IrLhs::Bit(index, base, two_state) => (
                    IrLhs::Whole(index),
                    vec![IrPackedSelect {
                        base: *base,
                        width: 1,
                    }],
                    two_state,
                ),
                IrLhs::Part(index, left, right, two_state) => (
                    IrLhs::Whole(index),
                    vec![IrPackedSelect {
                        base: lhs_integer_expr(i128::from(left.min(right))),
                        width: u32::try_from(left.abs_diff(right) + 1)
                            .map_err(|_| "reference part width overflow")?,
                    }],
                    two_state,
                ),
                IrLhs::IdxPart(index, base, _, width, negative, two_state) => (
                    IrLhs::Whole(index),
                    vec![indexed_projection(*base, width, negative)?],
                    two_state,
                ),
                IrLhs::ArrayElem {
                    arr,
                    indices,
                    elem_sel,
                } => {
                    let steps = match elem_sel {
                        IrElemSel::Whole => {
                            return Ok(IrLhs::ArrayElem {
                                arr,
                                indices,
                                elem_sel: IrElemSel::Whole,
                            })
                        }
                        IrElemSel::Bit(base) => vec![IrPackedSelect {
                            base: *base,
                            width: 1,
                        }],
                        IrElemSel::Part(left, right) => vec![IrPackedSelect {
                            base: lhs_integer_expr(i128::from(left.min(right))),
                            width: u32::try_from(left.abs_diff(right) + 1)
                                .map_err(|_| "reference part width overflow")?,
                        }],
                        IrElemSel::Indexed {
                            base,
                            width,
                            negative,
                        } => {
                            vec![indexed_projection(*base, width, negative)?]
                        }
                        IrElemSel::PackedChain(steps) => steps,
                    };
                    (
                        IrLhs::ArrayElem {
                            arr,
                            indices,
                            elem_sel: IrElemSel::Whole,
                        },
                        steps,
                        false,
                    )
                }
                other => return Ok(other),
            };
            Ok(IrLhs::PackedSelect {
                target: Box::new(target),
                steps,
                signed: false,
                two_state,
            })
        }

        fn compose_reference_bit(
            base: IrLhs,
            expression: IrExpr,
            two_state: bool,
        ) -> Result<IrLhs, String> {
            match normalize_projection(base)? {
                IrLhs::PackedSelect {
                    target,
                    mut steps,
                    two_state: state,
                    ..
                } => {
                    steps.push(crate::sim::ir::IrPackedSelect {
                        base: expression,
                        width: 1,
                    });
                    Ok(IrLhs::PackedSelect {
                        target,
                        steps,
                        signed: false,
                        two_state: two_state || state,
                    })
                }
                IrLhs::TaggedSelect {
                    target,
                    mut steps,
                    two_state: state,
                    location,
                    ..
                } => {
                    steps.push(crate::sim::ir::IrTaggedSelectStep {
                        selection: crate::sim::ir::IrPackedSelect {
                            base: expression,
                            width: 1,
                        },
                        two_state,
                        guard: None,
                    });
                    Ok(IrLhs::TaggedSelect {
                        target,
                        steps,
                        signed: false,
                        two_state: two_state || state,
                        location,
                    })
                }
                IrLhs::Whole(index) => Ok(IrLhs::Bit(index, Box::new(expression), two_state)),
                IrLhs::ArrayElem {
                    arr,
                    indices,
                    elem_sel: IrElemSel::Whole,
                } => Ok(IrLhs::ArrayElem {
                    arr,
                    indices,
                    elem_sel: IrElemSel::Bit(Box::new(expression)),
                }),
                _ => Err(
                    "nested bit selection through a selected reference port is not supported"
                        .to_owned(),
                ),
            }
        }

        fn compose_reference_part(
            base: IrLhs,
            left: i64,
            right: i64,
            two_state: bool,
        ) -> Result<IrLhs, String> {
            match normalize_projection(base)? {
                IrLhs::PackedSelect {
                    target,
                    mut steps,
                    two_state: state,
                    ..
                } => {
                    let width = u32::try_from(left.abs_diff(right) + 1)
                        .map_err(|_| "reference part width overflow")?;
                    steps.push(crate::sim::ir::IrPackedSelect {
                        base: lhs_integer_expr(i128::from(left.min(right))),
                        width,
                    });
                    Ok(IrLhs::PackedSelect {
                        target,
                        steps,
                        signed: false,
                        two_state: two_state || state,
                    })
                }
                IrLhs::TaggedSelect {
                    target,
                    mut steps,
                    two_state: state,
                    location,
                    ..
                } => {
                    let width = u32::try_from(left.abs_diff(right) + 1)
                        .map_err(|_| "reference part width overflow")?;
                    steps.push(crate::sim::ir::IrTaggedSelectStep {
                        selection: crate::sim::ir::IrPackedSelect {
                            base: lhs_integer_expr(i128::from(left.min(right))),
                            width,
                        },
                        two_state,
                        guard: None,
                    });
                    Ok(IrLhs::TaggedSelect {
                        target,
                        steps,
                        signed: false,
                        two_state: two_state || state,
                        location,
                    })
                }
                IrLhs::Whole(index) => Ok(IrLhs::Part(index, left, right, two_state)),
                IrLhs::ArrayElem {
                    arr,
                    indices,
                    elem_sel: IrElemSel::Whole,
                } => Ok(IrLhs::ArrayElem {
                    arr,
                    indices,
                    elem_sel: IrElemSel::Part(left, right),
                }),
                _ => Err(
                    "nested part selection through a selected reference port is not supported"
                        .to_owned(),
                ),
            }
        }

        resolve(self, lhs, &mut HashSet::new())
    }

    /// Bind a constant selection of a plain concatenation target, such as a
    /// member of an unpacked record whose leaves are separate signals, to
    /// the one leaf that holds it. Reads, writes, writer ownership and
    /// sensitivity then use that leaf's canonical storage instead of an
    /// unnamed projection of the whole record.
    pub(in super::super) fn collapse_concat_reference(&self, lhs: IrLhs) -> IrLhs {
        let IrLhs::PackedSelect {
            target,
            steps,
            two_state,
            ..
        } = &lhs
        else {
            return lhs;
        };
        let IrLhs::Stream {
            parts,
            slice: 1,
            direction: IrStreamDirection::LeftToRight,
            ..
        } = target.as_ref()
        else {
            return lhs;
        };
        let mut lsb = 0u64;
        for step in steps {
            let Some(base) =
                Self::ir_constant_i128(&step.base).and_then(|base| u64::try_from(base).ok())
            else {
                return lhs;
            };
            lsb = lsb.saturating_add(base);
        }
        let Some(width) = steps.last().map(|step| u64::from(step.width)) else {
            return lhs;
        };
        // Concatenation parts are listed MSB first; offsets count from the LSB.
        let mut offset = 0u64;
        for (part, part_width) in parts.iter().rev() {
            let part_width = u64::from(*part_width);
            if lsb >= offset && lsb.saturating_add(width) <= offset + part_width {
                let (index, low, state) = match part {
                    IrLhs::Whole(index) => (*index, lsb - offset, *two_state),
                    IrLhs::Part(index, left, right, state) => {
                        let Ok(base) = u64::try_from((*left).min(*right)) else {
                            return lhs;
                        };
                        (*index, base + lsb - offset, *state || *two_state)
                    }
                    _ => return lhs,
                };
                let full = self
                    .model
                    .signals
                    .get(index)
                    .is_some_and(|signal| u64::from(signal.ty.width()) == width);
                if low == 0 && full {
                    return IrLhs::Whole(index);
                }
                let (Ok(left), Ok(right)) = (i64::try_from(low + width - 1), i64::try_from(low))
                else {
                    return lhs;
                };
                return IrLhs::Part(index, left, right, state);
            }
            offset += part_width;
        }
        lhs
    }

    pub(in super::super) fn reference_lhs_type(&self, lhs: &IrLhs) -> Option<IrType> {
        match lhs {
            IrLhs::PackedSelect {
                target,
                steps,
                signed,
                two_state,
            } => Some(IrType::Packed {
                width: steps.last()?.width,
                signed: *signed,
                two_state: *two_state || self.reference_lhs_type(target)?.two_state(),
            }),
            IrLhs::TaggedSelect {
                target,
                steps,
                signed,
                two_state,
                ..
            } => Some(IrType::Packed {
                width: steps.last()?.selection.width,
                signed: *signed,
                two_state: *two_state || self.reference_lhs_type(target)?.two_state(),
            }),
            IrLhs::Whole(index) => self.model.signals.get(*index).map(|signal| signal.ty),
            IrLhs::Bit(index, ..) => self.model.signals.get(*index).map(|signal| IrType::Packed {
                width: 1,
                signed: false,
                two_state: signal.ty.two_state(),
            }),
            IrLhs::Part(index, left, right, two_state) => self
                .model
                .signals
                .get(*index)
                .is_some()
                .then_some(IrType::Packed {
                    width: left.abs_diff(*right) as u32 + 1,
                    signed: false,
                    two_state: *two_state,
                }),
            IrLhs::IdxPart(index, _, _, width, _, two_state) => self
                .model
                .signals
                .get(*index)
                .is_some()
                .then_some(IrType::Packed {
                    width: *width,
                    signed: false,
                    two_state: *two_state,
                }),
            IrLhs::ArrayElem { arr, elem_sel, .. } => {
                let array = self.model.arrays.get(self.reference_array(*arr))?;
                match elem_sel {
                    IrElemSel::Whole => Some(if array.real {
                        IrType::Real {
                            shortreal: array.shortreal,
                        }
                    } else {
                        IrType::Packed {
                            width: array.elem_width,
                            signed: array.signed,
                            two_state: array.two_state,
                        }
                    }),
                    IrElemSel::Part(left, right) => Some(IrType::Packed {
                        width: left.abs_diff(*right) as u32 + 1,
                        signed: false,
                        two_state: array.two_state,
                    }),
                    IrElemSel::Bit(_) => Some(IrType::Packed {
                        width: 1,
                        signed: false,
                        two_state: array.two_state,
                    }),
                    IrElemSel::Indexed { width, .. } => Some(IrType::Packed {
                        width: *width,
                        signed: false,
                        two_state: array.two_state,
                    }),
                    IrElemSel::PackedChain(steps) => Some(IrType::Packed {
                        width: steps.last()?.width,
                        signed: false,
                        two_state: array.two_state,
                    }),
                }
            }
            IrLhs::WholeRef {
                width,
                signed,
                two_state,
                ..
            }
            | IrLhs::Ref {
                width,
                signed,
                two_state,
                ..
            } => Some(IrType::Packed {
                width: *width,
                signed: *signed,
                two_state: *two_state,
            }),
            IrLhs::Stream { width, .. } => Some(IrType::Packed {
                width: *width,
                signed: false,
                two_state: false,
            }),
        }
    }

    pub(in super::super) fn reference_lhs_is_variable(&self, lhs: &IrLhs) -> bool {
        match lhs {
            IrLhs::Whole(index)
            | IrLhs::Bit(index, ..)
            | IrLhs::Part(index, ..)
            | IrLhs::IdxPart(index, ..) => {
                self.model.signals.get(*index).is_some_and(|signal| {
                    signal.net_driver.is_none() && signal.net_alias.is_empty()
                })
            }
            IrLhs::ArrayElem { arr, .. } => {
                let array = self.reference_array(*arr);
                self.arrays
                    .iter()
                    .find(|info| info.ir == array)
                    .is_some_and(|info| !info.is_net)
            }
            IrLhs::PackedSelect { target, .. } | IrLhs::TaggedSelect { target, .. } => {
                self.reference_lhs_is_variable(target)
            }
            IrLhs::Stream { parts, .. } => parts
                .iter()
                .all(|(part, _)| self.reference_lhs_is_variable(part)),
            IrLhs::WholeRef { .. } | IrLhs::Ref { .. } => false,
        }
    }

    pub(in super::super) fn reference_actual_is_variable(&self, node: NodeId) -> bool {
        match self.kind(node) {
            NodeKind::Var { .. } => true,
            NodeKind::Array { .. } => self
                .db
                .array_meta(node)
                .is_some_and(|meta| matches!(meta.kind(), ArrayKind::Static)),
            NodeKind::Expr(ExprKind::Ref { target }) => {
                target.is_some_and(|target| self.reference_actual_is_variable(target))
            }
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => refs
                .iter()
                .rev()
                .flatten()
                .copied()
                .any(|target| self.reference_actual_is_variable(target)),
            NodeKind::Expr(
                ExprKind::BitSelect { base, .. }
                | ExprKind::PartSelect { base, .. }
                | ExprKind::IndexedPartSelect { base, .. }
                | ExprKind::ArraySelect { base, .. },
            ) => self.reference_actual_is_variable(*base),
            _ => false,
        }
    }

    pub(in super::super) fn reference_array(&self, mut array: usize) -> usize {
        let mut seen = HashSet::new();
        while let Some(next) = self.reference_arrays.get(&array).copied() {
            if !seen.insert(array) {
                break;
            }
            array = next;
        }
        array
    }

    pub(in super::super) fn reference_object(&self, mut object: usize) -> usize {
        let mut seen = HashSet::new();
        while let Some(next) = self.reference_objects.get(&object).copied() {
            if !seen.insert(object) {
                break;
            }
            object = next;
        }
        object
    }

    /// Read a signal through its canonical reference target.  Selected
    /// targets are represented as ordinary typed IR selections, so reads and
    /// writes share the same four-state behavior and notification path.
    pub(in super::super) fn signal_read_expr(&self, info: &SignalInfo) -> Result<IrExpr, String> {
        let target = self.reference_lhs(IrLhs::Whole(info.ir))?;
        self.reference_target_read(target)
    }

    fn reference_target_read(&self, target: IrLhs) -> Result<IrExpr, String> {
        let read_signal = |index: usize| {
            let signal = self.model.signal(index);
            let (width, signed) = match signal.ty {
                IrType::Real { .. } => (0, false),
                IrType::Packed { width, signed, .. } => (width, signed),
            };
            IrExpr::new(IrExprKind::SigRead(index), width, signed, None)
        };
        match target {
            IrLhs::Whole(index) => Ok(read_signal(index)),
            IrLhs::Bit(index, expression, _) => Ok(IrExpr::new(
                IrExprKind::BitSel {
                    base: Box::new(read_signal(index)),
                    idx: expression,
                },
                1,
                false,
                None,
            )),
            IrLhs::Part(index, left, right, _) => Ok(IrExpr::new(
                IrExprKind::PartSel {
                    base: Box::new(read_signal(index)),
                    left,
                    right,
                },
                left.abs_diff(right) as u32 + 1,
                false,
                None,
            )),
            IrLhs::IdxPart(index, base, width, selected_width, negative, _) => Ok(IrExpr::new(
                IrExprKind::IdxPartSel {
                    base: Box::new(read_signal(index)),
                    base_idx: base,
                    width_expr: width,
                    neg: negative,
                },
                selected_width,
                false,
                None,
            )),
            IrLhs::ArrayElem {
                arr,
                indices,
                elem_sel,
            } => {
                let array = self.model.array(self.reference_array(arr));
                let width = match &elem_sel {
                    IrElemSel::Whole => array.elem_width,
                    IrElemSel::Part(left, right) => left.abs_diff(*right) as u32 + 1,
                    IrElemSel::Bit(_) => 1,
                    IrElemSel::Indexed { width, .. } => *width,
                    IrElemSel::PackedChain(steps) => steps.last().map_or(0, |step| step.width),
                };
                let signed = matches!(elem_sel, IrElemSel::Whole) && array.signed;
                Ok(IrExpr::new(
                    IrExprKind::ArrayRead {
                        arr: self.reference_array(arr),
                        indices,
                        elem_sel,
                    },
                    width,
                    signed,
                    None,
                ))
            }
            IrLhs::PackedSelect {
                target,
                steps,
                signed,
                two_state,
            } => {
                let mut value = self.reference_target_read(*target)?;
                for step in steps {
                    value = super::collection::packed_formals::packed_step_read(value, step);
                }
                let width = value.width;
                ir_to_storage(value, width, signed, two_state)
            }
            IrLhs::TaggedSelect {
                target,
                steps,
                signed,
                two_state,
                location,
            } => {
                let base = self.reference_target_read(*target)?;
                let width = steps.last().map_or(0, |step| step.selection.width);
                let value = IrExpr::new(
                    IrExprKind::TaggedSelect {
                        base: Box::new(base),
                        steps,
                        location,
                    },
                    width,
                    signed,
                    None,
                );
                ir_to_storage(value, width, signed, two_state)
            }
            IrLhs::Stream { parts, .. } => {
                let values = parts
                    .into_iter()
                    .map(|(part, _)| self.reference_target_read(part))
                    .collect::<Result<Vec<_>, _>>()?;
                Self::join_bitstream_parts("reference port", values)
            }
            IrLhs::WholeRef { .. } | IrLhs::Ref { .. } => {
                Err("reference port target is not a scalar readable storage".to_owned())
            }
        }
    }

    pub(in super::super) fn signal_dependency_name(&self, index: usize) -> String {
        let signal = self.model.signal(index);
        if signal.net_alias.is_empty() {
            signal.c_name.clone()
        } else {
            // Alias-visible storage is refreshed by the runtime whenever any
            // canonical group bit changes. Dependencies therefore point at
            // the descriptor's visible cell rather than retained raw storage.
            format!("llg_net_alias_{index}.visible")
        }
    }

    pub(in super::super) fn reference_dependency(&self, info: &SignalInfo) -> IrDependency {
        let target = self
            .reference_lhs(IrLhs::Whole(info.ir))
            .unwrap_or(IrLhs::Whole(info.ir));
        self.reference_target_dependency(target, &info.global)
    }

    fn reference_target_dependency(&self, target: IrLhs, fallback: &str) -> IrDependency {
        match target {
            IrLhs::Whole(index) => match self.model.signal(index).ty {
                IrType::Real { .. } => IrDependency::real(self.model.signal(index).c_name.clone()),
                IrType::Packed { .. } => IrDependency::scalar(self.signal_dependency_name(index)),
            },
            IrLhs::Bit(index, ..) | IrLhs::Part(index, ..) | IrLhs::IdxPart(index, ..) => {
                IrDependency::scalar(self.signal_dependency_name(index))
            }
            IrLhs::ArrayElem { arr, indices, .. } => {
                let arr = self.reference_array(arr);
                let info = self.arrays.iter().find(|candidate| candidate.ir == arr);
                let linear = info.and_then(|candidate| {
                    let expressions = indices;
                    Self::array_constant_linear_index(candidate, &expressions)
                });
                linear.map_or(IrDependency::ArrayContents(arr), |index| {
                    IrDependency::ArrayElement { array: arr, index }
                })
            }
            IrLhs::PackedSelect { target, steps, .. } => {
                let mut storage = self.reference_target_dependency(*target, fallback);
                for step in steps {
                    let Some(lsb) = Self::ir_constant_i128(&step.base)
                        .and_then(|value| u32::try_from(value).ok())
                    else {
                        break;
                    };
                    storage = IrDependency::PackedRange {
                        storage: Box::new(storage),
                        lsb,
                        width: step.width,
                    };
                }
                storage
            }
            // The active-tag guard reads discriminator bits outside the
            // selected payload, so retain a dependency on the complete root.
            IrLhs::TaggedSelect { target, .. } => {
                self.reference_target_dependency(*target, fallback)
            }
            IrLhs::WholeRef { .. } | IrLhs::Ref { .. } | IrLhs::Stream { .. } => {
                IrDependency::scalar(fallback.to_owned())
            }
        }
    }
}

/// Packed step for an indexed part-select `[base +: width]` or
/// `[base -: width]` whose base is already a storage-relative bit offset.
pub(super) fn indexed_projection(
    base: IrExpr,
    width: u32,
    negative: bool,
) -> Result<IrPackedSelect, String> {
    let base = if negative {
        let offset = lhs_integer_expr(i128::from(width) - 1);
        let arithmetic_width = base
            .width
            .max(offset.width)
            .checked_add(1)
            .filter(|width| *width <= LLG_MAX_WIDTH)
            .ok_or_else(|| {
                "reference selector arithmetic exceeds the supported limit".to_owned()
            })?;
        bin_expr(
            IrBinOp::Sub,
            IrExpr::convert_to(base, arithmetic_width, true),
            IrExpr::convert_to(offset, arithmetic_width, true),
        )
    } else {
        base
    };
    Ok(IrPackedSelect { base, width })
}
