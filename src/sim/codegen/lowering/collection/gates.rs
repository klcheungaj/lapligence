//! Gates.

use super::*;
use crate::core::db::{UdpRow, UdpTable};
use crate::sim::ir::{
    IrUdpEdge, IrUdpInput, IrUdpNext, IrUdpOutput, IrUdpRow, IrUdpSequential, IrUdpSequentialRow,
    IrUdpTable,
};
use std::sync::Arc;

impl<'a> Codegen<'a> {
    // ── Structural gate primitives ─────────────────────────────────────────

    /// Lower one structural primitive ([`NodeKind::Gate`]) into comb processes
    /// shaped like continuous assignments. Every process evaluates at spawn,
    /// then re-evaluates whenever an input-terminal dependency changes. A
    /// multi-output `buf`/`not` gets one process per output so each output can
    /// retain an independent canonical driver contribution.
    ///
    /// Multi-input logic gates reduce their inputs left-to-right with the
    /// two-input runtime op; nand/nor/xnor negate after the full reduce.
    /// A gate delay `#D` schedules a captured inertial update while the
    /// process continues watching its inputs. UDPs use their owned table and
    /// retain the same structural driver identity as builtin gates; a
    /// sequential UDP instance first advances its own persistent state
    /// ([`IrStmt::UdpStep`]) and then publishes that state. Switches are
    /// rejected explicitly.
    pub(super) fn emit_gate(&mut self, inst: NodeId, path: &str, g: NodeId) -> Result<(), String> {
        let (class, prim_type, strength0, strength1, delay, terms, udp_table) = match self.kind(g) {
            NodeKind::Gate {
                class,
                prim_type,
                strength0,
                strength1,
                delay,
                terms,
                udp,
            } => (
                *class,
                *prim_type,
                *strength0,
                *strength1,
                *delay,
                Vec::clone(terms),
                udp.clone(),
            ),
            _ => unreachable!("non-gate node passed to emit_gate"),
        };
        let gname = self.node(g).name.clone();
        let shown = if gname.is_empty() {
            "gate"
        } else {
            gname.as_str()
        };
        match class {
            PrimClass::Gate => {}
            PrimClass::Switch => {
                return Err(format!(
                    "switch/transistor primitive `{shown}` in `{path}` is not \
                     supported"
                ))
            }
            PrimClass::Udp | PrimClass::Array => {}
        }
        let driver_strengths =
            gate_driver_strengths(prim_type, strength0, strength1, &format!("{path}.{shown}"))?;
        // Which builtin gate this is; UDP instances take the separate table
        // evaluator below and every other primitive kind remains explicit.
        let op = match prim_type {
            PrimitiveType::And => GateOp::Reduce(IrBinOp::BitAnd, false),
            PrimitiveType::Nand => GateOp::Reduce(IrBinOp::BitAnd, true),
            PrimitiveType::Or => GateOp::Reduce(IrBinOp::BitOr, false),
            PrimitiveType::Nor => GateOp::Reduce(IrBinOp::BitOr, true),
            PrimitiveType::Xor => GateOp::Reduce(IrBinOp::BitXor, false),
            PrimitiveType::Xnor => GateOp::Reduce(IrBinOp::BitXor, true),
            PrimitiveType::Buf => GateOp::Copy,
            PrimitiveType::Not => GateOp::Not,
            PrimitiveType::Bufif1 => GateOp::Enable {
                invert_out: false,
                active_high: true,
            },
            PrimitiveType::Bufif0 => GateOp::Enable {
                invert_out: false,
                active_high: false,
            },
            PrimitiveType::Notif1 => GateOp::Enable {
                invert_out: true,
                active_high: true,
            },
            PrimitiveType::Notif0 => GateOp::Enable {
                invert_out: true,
                active_high: false,
            },
            PrimitiveType::Pullup => GateOp::Pull(true),
            PrimitiveType::Pulldown => GateOp::Pull(false),
            PrimitiveType::Combinational | PrimitiveType::Sequential => GateOp::Udp,
            _ => {
                return Err(format!(
                    "primitive type {prim_type:?} of `{shown}` in `{path}` is not \
                     supported"
                ))
            }
        };
        // Terminal-count/direction validation per kind. Directions are
        // captured by the frontend from the primitive definition: `buf` and
        // `not` mark every terminal except the final input as an output.
        let out_positions: Vec<usize> = terms
            .iter()
            .enumerate()
            .filter(|(_, t)| t.direction == DbDirection::Output)
            .map(|(i, _)| i)
            .collect();
        let in_positions: Vec<usize> = terms
            .iter()
            .enumerate()
            .filter(|(_, t)| t.direction == DbDirection::Input)
            .map(|(i, _)| i)
            .collect();
        let shape_ok = match op {
            GateOp::Pull(_) => {
                terms.len() == 1 && out_positions.len() == 1 && in_positions.is_empty()
            }
            GateOp::Copy | GateOp::Not => {
                !out_positions.is_empty()
                    && in_positions.len() == 1
                    && out_positions.len() + in_positions.len() == terms.len()
            }
            GateOp::Enable { .. } => {
                terms.len() == 3 && out_positions.len() == 1 && in_positions.len() == 2
            }
            GateOp::Reduce(..) => {
                terms.len() >= 2
                    && out_positions.len() == 1
                    && !in_positions.is_empty()
                    && out_positions.len() + in_positions.len() == terms.len()
            }
            GateOp::Udp => udp_table.as_ref().is_some_and(|table| {
                usize::try_from(table.input_count).ok() == Some(in_positions.len())
                    && terms.len() == in_positions.len() + 1
                    && out_positions.len() == 1
                    && out_positions[0] < terms.len()
                    && out_positions[0] == 0
            }),
        };
        if !shape_ok {
            let what = match op {
                GateOp::Pull(_) => "pullup/pulldown instance takes exactly one output terminal",
                GateOp::Copy | GateOp::Not => {
                    "`buf`/`not` gates take one or more output terminals followed by \
                     exactly one input terminal"
                }
                GateOp::Enable { .. } => {
                    "enable gates take exactly one output, one data input and one \
                     enable input terminal"
                }
                GateOp::Reduce(..) => {
                    "logic gates take exactly one output terminal plus at least one \
                     input terminal"
                }
                GateOp::Udp if prim_type == PrimitiveType::Sequential => {
                    "sequential UDPs take one scalar output followed by scalar input terminals"
                }
                GateOp::Udp => {
                    "combinational UDPs take one scalar output followed by scalar input terminals"
                }
            };
            return Err(format!(
                "gate `{shown}` in `{path}`: {what} (found {} output(s) among {} \
                 terminals)",
                out_positions.len(),
                terms.len()
            ));
        }

        // Callee resolution in the LHS/inputs needs the owning instance.
        self.inst = inst;

        // Lower every terminal through the typed paths used by ordinary
        // assignments and expressions. This admits selects, constants and
        // hierarchical references while keeping real values and invalid
        // output expressions as precise gate diagnostics.
        let mut terminal_lhs: Vec<Option<IrLhs>> = vec![None; terms.len()];
        let mut terminal_exprs: Vec<Option<IrExpr>> = vec![None; terms.len()];
        let mut widths = vec![0u32; terms.len()];
        let mut sens = Vec::new();
        let is_udp = matches!(op, GateOp::Udp);
        let udp_kind = if prim_type == PrimitiveType::Sequential {
            "sequential"
        } else {
            "combinational"
        };
        for i in &in_positions {
            let expr = self.lower_expr(path, terms[*i].expr).map_err(|error| {
                format!(
                    "input terminal {} of gate `{shown}` in `{path}` is not a legal \
                     expression: {error}",
                    i
                )
            })?;
            if expr.is_real() {
                return Err(format!(
                    "real-valued input terminal {} of gate `{shown}` in `{path}` \
                     is not supported",
                    i
                ));
            }
            if expr.width() == 0 {
                return Err(format!(
                    "input terminal {} of gate `{shown}` in `{path}` has zero width",
                    i
                ));
            }
            if is_udp && expr.width() != 1 {
                return Err(format!(
                    "input terminal {} of {udp_kind} UDP `{shown}` in `{path}` must be scalar \
                     (at {})",
                    i,
                    self.source_location(g)
                ));
            }
            widths[*i] = expr.width();
            terminal_exprs[*i] = Some(expr);
            for dependency in self.collect_read_signals(path, terms[*i].expr)? {
                if !sens.contains(&dependency) {
                    sens.push(dependency);
                }
            }
        }
        for i in &out_positions {
            let lhs = self.lower_lhs(path, terms[*i].expr).map_err(|error| {
                format!(
                    "output terminal {} of gate `{shown}` in `{path}` is not a legal \
                     lvalue: {error}",
                    i
                )
            })?;
            let width = packed_lhs_width(&self.model, &lhs).ok_or_else(|| {
                format!(
                    "output terminal {} of gate `{shown}` in `{path}` must be a \
                     packed net or variable",
                    i
                )
            })?;
            if width == 0 {
                return Err(format!(
                    "output terminal {} of gate `{shown}` in `{path}` has zero width",
                    i
                ));
            }
            if is_udp && width != 1 {
                return Err(format!(
                    "output terminal {} of {udp_kind} UDP `{shown}` in `{path}` must be scalar \
                     (at {})",
                    i,
                    self.source_location(g)
                ));
            }
            widths[*i] = width;
            terminal_lhs[*i] = Some(lhs);
        }

        let scaled_delay = match delay {
            Some(de) => Some(self.driver_delay(path, g, de).map_err(|error| {
                format!(
                    "delay of `{shown}` in `{path}` at {}: {error}",
                    self.source_location(g)
                )
            })?),
            None => None,
        };
        // An SDF DEVICE entry annotates the gate: it reads writable delays
        // that start as its own (none: 0), so a delayed driver replaces an
        // undelayed one (IEEE 1800-2009 32.4.3).
        let scaled_delay = match (self.sdf_gate_slot(g), scaled_delay) {
            (Some(slot), delay) => {
                let initial = match delay {
                    None => IrTransitionDelay::uniform(0),
                    Some(IrInertialDelay::Constant(delay)) => delay,
                    Some(_) => {
                        return Err(format!(
                            "delay of `{shown}` in `{path}` at {}: an SDF DEVICE entry \
                             annotates this gate, whose delay is runtime-valued; llg annotates \
                             constant gate delays",
                            self.source_location(g)
                        ))
                    }
                };
                self.model.sdf_gate_delays[slot as usize] = initial;
                Some(IrInertialDelay::Annotated(slot))
            }
            (None, delay) => delay,
        };

        for (output_ordinal, out_pos) in out_positions.iter().enumerate() {
            let raw_lhs = terminal_lhs[*out_pos]
                .clone()
                .expect("gate output LHS lowered above");
            let output_width = widths[*out_pos];

            let mut lhs = raw_lhs.clone();
            let mut structural_terminal = None;
            let alias_bindings = self.alias_lvalue_bindings(g, terms[*out_pos].expr)?;
            let mut alias_terminals = HashMap::new();
            if let Some(bindings) = &alias_bindings {
                let groups = bindings
                    .iter()
                    .map(|(binding, _)| binding.group())
                    .collect::<HashSet<_>>();
                for group in groups {
                    // Number terminals within each resolved group. Alias
                    // concatenations can span several groups, so each group
                    // needs its own ordinal.
                    let alias_terminal = out_positions[..output_ordinal]
                        .iter()
                        .map(|previous| self.alias_lvalue_bindings(g, terms[*previous].expr))
                        .collect::<Result<Vec<_>, _>>()?
                        .into_iter()
                        .flatten()
                        .filter(|previous| {
                            previous.iter().any(|(binding, _)| binding.group() == group)
                        })
                        .count();
                    alias_terminals.insert(group, alias_terminal);
                    if alias_terminal == 0 {
                        if self.structural_driver_signal(g, group).is_none() {
                            return Err(format!(
                                "gate `{shown}` has no structural driver mapping for resolved net group {} at {}:{}:{}",
                                group,
                                self.node(g).file.as_deref().unwrap_or("<unknown>"),
                                self.node(g).line,
                                self.node(g).col,
                            ));
                        }
                    } else {
                        self.add_structural_driver_for_terminal(
                            group,
                            g,
                            driver_strengths,
                            alias_terminal,
                        )?;
                    }
                }
            } else if let Some(group) = self.structural_group_for_lhs(&raw_lhs) {
                // Primitive output terminals are numbered within each
                // resolved group. A gate may write one group before another
                // and then return to the first; using the global output
                // ordinal would give that later terminal a stale slot key.
                let terminal = out_positions[..output_ordinal]
                    .iter()
                    .filter(|previous| {
                        terminal_lhs[**previous]
                            .as_ref()
                            .and_then(|lhs| self.structural_group_for_lhs(lhs))
                            == Some(group)
                    })
                    .count();
                if terminal == 0 {
                    if self.structural_driver_signal(g, group).is_none() {
                        return Err(format!(
                            "gate `{shown}` has no structural driver mapping for resolved net \
                             group {} at {}:{}:{}",
                            group,
                            self.node(g).file.as_deref().unwrap_or("<unknown>"),
                            self.node(g).line,
                            self.node(g).col,
                        ));
                    }
                } else {
                    self.add_structural_driver_for_terminal(group, g, driver_strengths, terminal)?;
                }
                lhs = self.remap_structural_lhs_for_terminal(lhs, g, terminal);
                structural_terminal = Some((group, terminal));
                if let Some(unmapped) =
                    self.unmapped_structural_group_for_terminal(&raw_lhs, g, terminal)
                {
                    return Err(format!(
                        "gate `{shown}` has no structural driver mapping for resolved net group {unmapped} at {}:{}:{}",
                        self.node(g).file.as_deref().unwrap_or("<unknown>"),
                        self.node(g).line,
                        self.node(g).col,
                    ));
                }
            }

            // UDP inputs stay plain scalar expressions: the emitter reads each
            // one's state in place, so no per-evaluation input cell exists.
            let input_values = in_positions
                .iter()
                .map(|i| {
                    let expr = terminal_exprs[*i]
                        .clone()
                        .expect("gate input expression lowered above");
                    IrExpr::resize_to(expr, output_width, false)
                })
                .collect::<Vec<_>>();
            let mut enable_halves = None;
            let mut udp_step = None;
            let value = match op {
                GateOp::Pull(ones) => const_bits_expr(output_width, ones),
                // IEEE 1364-2001 Table 34: `buf` outputs x for a z input, so a
                // Z data bit must not pass through as an absent contribution.
                // `data | data` normalizes Z to X like the enable gates below.
                GateOp::Copy => {
                    let data = input_values
                        .into_iter()
                        .next()
                        .expect("buf/not shape checked");
                    bin_expr(IrBinOp::BitOr, data.clone(), data)
                }
                GateOp::Not => bitneg_full_width(
                    input_values
                        .into_iter()
                        .next()
                        .expect("buf/not shape checked"),
                ),
                GateOp::Reduce(bop, neg) => {
                    let mut values = input_values.into_iter();
                    let mut acc = values.next().expect("logic gate shape checked");
                    for next in values {
                        acc = bin_expr(bop, acc, next);
                    }
                    if neg {
                        bitneg_full_width(acc)
                    } else {
                        acc
                    }
                }
                GateOp::Enable {
                    invert_out,
                    active_high,
                } => {
                    let mut values = input_values.into_iter();
                    let data = values.next().expect("enable gate shape checked");
                    let en = IrExpr::resize_to(
                        values.next().expect("enable gate shape checked"),
                        1,
                        false,
                    );
                    // LRM 1364-1995 §7.4 Table 7-5: an ENABLED enable gate acts
                    // like `buf`/`not` (Tables 7-3/7-4), so a data Z must reach
                    // the output as X while known bits pass unchanged.  The mux
                    // is an identity/copy context that returns its chosen arm
                    // verbatim (`sv4_mux` with a known select), so the Z→X
                    // normalization is done explicitly with `data|data`.
                    // Correctness proof from llg_value.c `sv4_bitwise` (op = OR),
                    // both operands identical so every bit takes one rule:
                    // known 0 → `!ax && !ab && !bx && !bb` → 0; known 1 →
                    // `!ax && ab` → 1; a Z bit reads as `sv4_lsb_bit() == 3`,
                    // i.e. unknown, and falls to the `else o_x = 1` arm → X
                    // (Z behaves as X in expression ops, LRM 11.4.5); X likewise
                    // stays X.  Same operand ⇒ same width/signedness, resize is
                    // a no-op; opt's identity pass has no `a|a → a` rule, so the
                    // normalization survives optimization.
                    let data = bin_expr(IrBinOp::BitOr, data.clone(), data);
                    let data = if invert_out {
                        bitneg_full_width(data)
                    } else {
                        data
                    };
                    let enable_mux = |data: IrExpr, off: IrExpr| {
                        let (a, b) = if active_high {
                            (data, off)
                        } else {
                            (off, data)
                        };
                        IrExpr::new(
                            IrExprKind::Mux {
                                sel: Box::new(en.clone()),
                                a: Box::new(a),
                                b: Box::new(b),
                            },
                            output_width,
                            false,
                            None,
                        )
                    };
                    // An unknown enable gives L (0 or Z) or H (1 or Z), not X
                    // (IEEE 1364-2001 7.4 and 7.10.2). Without a gate delay,
                    // split the output into a strength0-only and a
                    // strength1-only contribution: the disabled arm drives the
                    // value that slot cannot carry, so an unknown enable
                    // merges a known data bit into one-sided X. Delayed gates
                    // keep one slot so each transition retains its single
                    // rise/fall/turn-off delay.
                    if scaled_delay.is_none()
                        && (alias_bindings.is_some() || structural_terminal.is_some())
                    {
                        enable_halves = Some((
                            enable_mux(data.clone(), const_bits_expr(output_width, true)),
                            enable_mux(data.clone(), const_bits_expr(output_width, false)),
                        ));
                    }
                    enable_mux(data, const_z_expr(output_width))
                }
                GateOp::Udp => {
                    let table = udp_table.as_ref().ok_or_else(|| {
                        format!("{udp_kind} UDP `{shown}` in `{path}` has no owned table")
                    })?;
                    let (value, step, start) =
                        self.udp_value(table, input_values, &format!("{path}.{shown}"))?;
                    udp_step = step;
                    self.delayed_driver_start = start;
                    value
                }
            };
            let body = if let (Some(bindings), Some((zero_side, one_side))) =
                (&alias_bindings, &enable_halves)
            {
                let mut groups = bindings
                    .iter()
                    .map(|(binding, _)| binding.group())
                    .collect::<Vec<_>>();
                groups.sort_unstable();
                groups.dedup();
                for group in groups {
                    self.split_enable_driver(g, group, driver_strengths)?;
                }
                let mut body = Vec::new();
                for (side, terminal) in [(zero_side, 0), (one_side, ENABLE_ONE_SIDE_TERMINAL)] {
                    for (driver, rhs) in
                        self.alias_driver_assignments(g, bindings, side, |_| terminal)?
                    {
                        body.push(IrStmt::Assign {
                            lhs: IrLhs::Whole(driver),
                            rhs,
                            nba: false,
                        });
                    }
                }
                body
            } else if let Some(bindings) = alias_bindings {
                let value_name = format!("_alias_gate_value_{}_{}", g.index(), output_ordinal);
                let value_read = IrExpr::new(
                    IrExprKind::LocalRead(value_name.clone()),
                    value.width(),
                    value.signed(),
                    None,
                );
                let assignments =
                    self.alias_driver_assignments(g, &bindings, &value_read, |group| {
                        alias_terminals.get(&group).copied().unwrap_or(0)
                    })?;
                self.publish_alias_drivers(
                    path,
                    g,
                    value_name,
                    value,
                    assignments,
                    scaled_delay.clone(),
                )?
            } else if let Some(delay) = scaled_delay.clone() {
                self.initialize_delayed_lhs(&lhs)?;
                vec![IrStmt::InertialAssign {
                    lhs,
                    rhs: value,
                    delay,
                }]
            } else if let (Some((group, 0)), Some((zero_side, one_side))) =
                (structural_terminal, enable_halves)
            {
                self.split_enable_driver(g, group, driver_strengths)?;
                let one_lhs =
                    self.remap_structural_lhs_for_terminal(raw_lhs, g, ENABLE_ONE_SIDE_TERMINAL);
                vec![
                    IrStmt::Assign {
                        lhs,
                        rhs: zero_side,
                        nba: false,
                    },
                    IrStmt::Assign {
                        lhs: one_lhs,
                        rhs: one_side,
                        nba: false,
                    },
                ]
            } else {
                vec![IrStmt::Assign {
                    lhs,
                    rhs: value,
                    nba: false,
                }]
            };
            // Delayed-driver initialization above used the UDP start value.
            self.delayed_driver_start = None;
            let mut body = body;
            if let Some(step) = udp_step {
                body.insert(0, step);
            }
            let shape = if sens.is_empty() {
                IrShape::RunOnce
            } else {
                IrShape::SensLoop {
                    reads: sens.clone(),
                }
            };
            let fn_name = self.new_fn_name(path, "gate");
            let origin = self.origin(g);
            self.model.processes.push(IrProcess::new_with_origin(
                fn_name,
                format!("{path}.{shown}[output{output_ordinal}]"),
                shape,
                Vec::new(),
                body,
                origin,
            ));
        }
        Ok(())
    }

    /// Give an undelayed enable gate a second driver slot in `group`: the
    /// primary slot keeps only strength0 and the new slot only strength1.
    fn split_enable_driver(
        &mut self,
        gate: NodeId,
        group: usize,
        (strength0, strength1): (u8, u8),
    ) -> Result<(), String> {
        let primary = self
            .structural_driver_signal(gate, group)
            .and_then(|signal| self.model.signals.get(signal))
            .and_then(|signal| signal.net_driver)
            .ok_or_else(|| "enable gate has no primary structural driver".to_owned())?;
        self.model.net_groups[primary.0].driver_strengths[primary.1] = (strength0, 0);
        self.add_structural_driver_for_terminal(
            group,
            gate,
            (0, strength1),
            ENABLE_ONE_SIDE_TERMINAL,
        )?;
        Ok(())
    }
}

/// Terminal key of the strength1-only slot of a split enable gate. Enable
/// gates have one output, which always uses terminal 0.
const ENABLE_ONE_SIDE_TERMINAL: usize = 1;

impl Codegen<'_> {
    /// Retain the definition once and lower one instance's evaluation.
    ///
    /// A combinational instance evaluates its rows against the scalar inputs.
    /// A sequential instance gets its own persistent state: the returned
    /// [`IrStmt::UdpStep`] advances it and the value reads it. The third
    /// result is the start value a delayed output driver takes: an
    /// `initial` value is not delayed by the instance delay (IEEE 1364-2001
    /// 8.5, Figure 30), while without one the output starts X.
    fn udp_value(
        &mut self,
        table: &Arc<UdpTable>,
        inputs: Vec<IrExpr>,
        instance: &str,
    ) -> Result<(IrExpr, Option<IrStmt>, Option<bool>), String> {
        let key = Arc::as_ptr(table) as usize;
        let index = match self.udp_declaration_indices.get(&key) {
            Some(index) => *index,
            None => {
                let definition = udp_definition(table)?;
                let index = match self.udp_table_indices.get(&definition) {
                    Some(index) => *index,
                    None => {
                        let index = self.model.udp_tables.len();
                        self.model.udp_tables.push(definition.clone());
                        self.udp_table_indices.insert(definition, index);
                        index
                    }
                };
                self.udp_declaration_indices.insert(key, index);
                index
            }
        };
        let definition = &self.model.udp_tables[index];
        if definition.input_count != inputs.len() {
            return Err(format!(
                "UDP `{}` instance `{instance}` has {} input terminals but its definition at {} \
                 has {}",
                definition.name,
                inputs.len(),
                table.location,
                definition.input_count
            ));
        }
        let Some(sequential) = &definition.sequential else {
            return Ok((
                IrExpr::new(
                    IrExprKind::UdpEval {
                        table: index,
                        inputs,
                    },
                    1,
                    false,
                    None,
                ),
                None,
                None,
            ));
        };
        let start = match sequential.initial {
            IrUdpOutput::Zero => Some(false),
            IrUdpOutput::One => Some(true),
            IrUdpOutput::Unknown => None,
        };
        let state = self.model.udp_states.len();
        self.model.udp_states.push(index);
        Ok((
            IrExpr::new(IrExprKind::UdpState(state), 1, false, None),
            Some(IrStmt::UdpStep { state, inputs }),
            start,
        ))
    }
}

/// One input field of a Slang-normalized row: a level symbol, a shorthand
/// edge or an explicit `(vw)` transition.
enum UdpField {
    Level(IrUdpInput),
    Edge { from: IrUdpInput, to: IrUdpInput },
}

fn udp_level(symbol: u8) -> Option<IrUdpInput> {
    Some(match symbol {
        b'0' => IrUdpInput::Zero,
        b'1' => IrUdpInput::One,
        b'x' => IrUdpInput::Unknown,
        b'b' => IrUdpInput::Binary,
        b'?' => IrUdpInput::Any,
        _ => return None,
    })
}

/// Split one row's inputs into fields; IEEE 1364-2001 Table 40 gives the
/// shorthand edges: `r` = (01), `f` = (10), `p` = (01)(0x)(x1), `n` =
/// (10)(1x)(x0) and `*` = (??). A transition always changes the value, so
/// `p` and `n` are the from/to products minus the unchanged (xx).
fn udp_fields(inputs: &str) -> Result<Vec<UdpField>, char> {
    let bytes = inputs.as_bytes();
    let mut fields = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let field = match bytes[index] {
            b'(' => {
                let (from, to, close) = (
                    bytes.get(index + 1),
                    bytes.get(index + 2),
                    bytes.get(index + 3),
                );
                index += 3;
                match (
                    from.copied().and_then(udp_level),
                    to.copied().and_then(udp_level),
                    close,
                ) {
                    (Some(from), Some(to), Some(b')')) => UdpField::Edge { from, to },
                    _ => return Err('('),
                }
            }
            b'r' => UdpField::Edge {
                from: IrUdpInput::Zero,
                to: IrUdpInput::One,
            },
            b'f' => UdpField::Edge {
                from: IrUdpInput::One,
                to: IrUdpInput::Zero,
            },
            b'p' => UdpField::Edge {
                from: IrUdpInput::ZeroOrUnknown,
                to: IrUdpInput::OneOrUnknown,
            },
            b'n' => UdpField::Edge {
                from: IrUdpInput::OneOrUnknown,
                to: IrUdpInput::ZeroOrUnknown,
            },
            b'*' => UdpField::Edge {
                from: IrUdpInput::Any,
                to: IrUdpInput::Any,
            },
            symbol => UdpField::Level(udp_level(symbol).ok_or(symbol as char)?),
        };
        fields.push(field);
        index += 1;
    }
    Ok(fields)
}

/// Convert one owned definition into its typed form. Slang has already
/// reported every illegal table (and stopped the run); these located errors
/// keep a frontend that kept a diagnosed row from reaching execution.
fn udp_definition(table: &UdpTable) -> Result<IrUdpTable, String> {
    let kind = if table.sequential {
        "sequential"
    } else {
        "combinational"
    };
    let input_count = usize::try_from(table.input_count).unwrap_or(usize::MAX);
    let row_error = |row: &UdpRow, what: String| {
        format!(
            "{kind} UDP `{}` table entry at {}: {what}",
            table.name, row.location
        )
    };
    let mut rows = Vec::new();
    let mut sequential_rows = Vec::new();
    for row in &table.rows {
        let fields = udp_fields(&row.inputs)
            .map_err(|symbol| row_error(row, format!("invalid input symbol `{symbol}`")))?;
        if fields.len() != input_count {
            return Err(row_error(
                row,
                format!("{} input fields, expected {input_count}", fields.len()),
            ));
        }
        let mut inputs = Vec::with_capacity(input_count);
        let mut edge = None;
        for (position, field) in fields.into_iter().enumerate() {
            match field {
                UdpField::Level(level) => inputs.push(level),
                UdpField::Edge { from, to } => {
                    if !table.sequential {
                        return Err(row_error(
                            row,
                            "an input transition is only allowed in a sequential UDP".to_owned(),
                        ));
                    }
                    if edge.is_some() {
                        return Err(row_error(
                            row,
                            "an entry may have at most one input transition".to_owned(),
                        ));
                    }
                    edge = Some(IrUdpEdge {
                        input: position,
                        from,
                    });
                    inputs.push(to);
                }
            }
        }
        let output = match row.output {
            b'0' => Some(IrUdpOutput::Zero),
            b'1' => Some(IrUdpOutput::One),
            b'x' => Some(IrUdpOutput::Unknown),
            b'-' => None,
            other => {
                return Err(row_error(
                    row,
                    format!("invalid output symbol `{}`", other as char),
                ))
            }
        };
        if !table.sequential {
            if row.state.is_some() {
                return Err(row_error(
                    row,
                    "a current-state field is only allowed in a sequential UDP".to_owned(),
                ));
            }
            let output = output.ok_or_else(|| {
                row_error(
                    row,
                    "`-` (no change) is only allowed in a sequential UDP".to_owned(),
                )
            })?;
            rows.push(IrUdpRow { inputs, output });
            continue;
        }
        let state = row
            .state
            .ok_or_else(|| {
                row_error(
                    row,
                    "a sequential entry needs a current-state field".to_owned(),
                )
            })
            .and_then(|state| {
                udp_level(state).ok_or_else(|| {
                    row_error(
                        row,
                        format!("invalid current-state symbol `{}`", state as char),
                    )
                })
            })?;
        sequential_rows.push(IrUdpSequentialRow {
            inputs,
            edge,
            state,
            next: output.map_or(IrUdpNext::Hold, IrUdpNext::Output),
        });
    }
    let sequential = if table.sequential {
        let initial = match table.initial {
            None | Some(b'x') => IrUdpOutput::Unknown,
            Some(b'0') => IrUdpOutput::Zero,
            Some(b'1') => IrUdpOutput::One,
            Some(other) => {
                return Err(format!(
                    "sequential UDP `{}` at {} has invalid initial value `{}`",
                    table.name, table.location, other as char
                ))
            }
        };
        Some(Box::new(IrUdpSequential {
            initial,
            rows: sequential_rows,
        }))
    } else {
        None
    };
    Ok(IrUdpTable {
        name: table.name.clone(),
        input_count,
        rows,
        sequential,
    })
}
