//! Gates.

use super::*;

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
    /// process continues watching its inputs. Unsupported primitives
    /// (switches and UDPs) are rejected with explicit errors here at lowering
    /// time; terminal legality is checked by the typed LHS/expression
    /// lowerers.
    pub(super) fn emit_gate(&mut self, inst: NodeId, path: &str, g: NodeId) -> Result<(), String> {
        let (class, prim_type, strength0, strength1, delay, terms) = match self.kind(g) {
            NodeKind::Gate {
                class,
                prim_type,
                strength0,
                strength1,
                delay,
                terms,
            } => (
                *class,
                *prim_type,
                *strength0,
                *strength1,
                *delay,
                Vec::clone(terms),
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
            PrimClass::Udp => {
                return Err(format!(
                    "user-defined primitive instance `{shown}` in `{path}` is not \
                     supported"
                ))
            }
            PrimClass::Array => {}
        }
        let driver_strengths =
            gate_driver_strengths(prim_type, strength0, strength1, &format!("{path}.{shown}"))?;
        // Which builtin gate this is; everything outside the supported set
        // (switch/transistor prim types, sequential/combinational UDP types)
        // is rejected.  UDP instances never reach this point (their class was
        // rejected above); the prim-type reject covers unknown/other kinds.
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
            widths[*i] = width;
            terminal_lhs[*i] = Some(lhs);
        }

        let scaled_delay = match delay {
            Some(de) => Some(self.driver_delay_ticks(g, de)?),
            None => None,
        };

        for (output_ordinal, out_pos) in out_positions.iter().enumerate() {
            let raw_lhs = terminal_lhs[*out_pos]
                .clone()
                .expect("gate output LHS lowered above");
            let output_width = widths[*out_pos];

            let mut lhs = raw_lhs.clone();
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

            let mut input_values = Vec::with_capacity(in_positions.len());
            for i in &in_positions {
                let expr = terminal_exprs[*i]
                    .clone()
                    .expect("gate input expression lowered above");
                input_values.push(IrExpr::resize_to(expr, output_width, false));
            }
            let value = match op {
                GateOp::Pull(ones) => const_bits_expr(output_width, ones),
                GateOp::Copy => input_values
                    .into_iter()
                    .next()
                    .expect("buf/not shape checked"),
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
                    let z = const_z_expr(output_width);
                    let (a, b) = if active_high { (data, z) } else { (z, data) };
                    IrExpr::new(
                        IrExprKind::Mux {
                            sel: Box::new(en),
                            a: Box::new(a),
                            b: Box::new(b),
                        },
                        output_width,
                        false,
                        None,
                    )
                }
            };
            let body = if let Some(bindings) = alias_bindings {
                let value_name = format!("_alias_gate_value_{}_{}", g.index(), output_ordinal);
                let value_read = IrExpr::new(
                    IrExprKind::LocalRead(value_name.clone()),
                    value.width(),
                    value.signed(),
                    None,
                );
                let mut body = vec![IrStmt::DeclLocal {
                    name: value_name,
                    width: value.width(),
                    signed: value.signed(),
                    init: Some(Box::new(value)),
                    two_state: false,
                }];
                for (driver, value) in
                    self.alias_driver_assignments(g, &bindings, &value_read, |group| {
                        alias_terminals.get(&group).copied().unwrap_or(0)
                    })?
                {
                    let lhs = IrLhs::Whole(driver);
                    if let Some(delay) = scaled_delay {
                        self.initialize_delayed_driver(driver)?;
                        body.push(IrStmt::InertialAssign {
                            lhs,
                            rhs: value,
                            delay,
                        });
                    } else {
                        body.push(IrStmt::Assign {
                            lhs,
                            rhs: value,
                            nba: false,
                        });
                    }
                }
                body
            } else if let Some(delay) = scaled_delay {
                if let IrLhs::Whole(index) = &lhs {
                    self.initialize_delayed_driver(*index)?;
                }
                vec![IrStmt::InertialAssign {
                    lhs,
                    rhs: value,
                    delay,
                }]
            } else {
                vec![IrStmt::Assign {
                    lhs,
                    rhs: value,
                    nba: false,
                }]
            };
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
}
