//! Constrained randomization (IEEE 1800-2009 clause 18): problem tables and
//! the solver-session primitives. Lowering arranges the surrounding control
//! flow (dynamic-class cases, hooks, the step loop and the commit); each
//! primitive here is one call into `llg_constraint.h` or its scheduler glue.
use super::*;

fn problem_name(problem: usize) -> String {
    format!("llg_rand_problem_{problem}")
}

/// The site's persistent session (created on first use).
fn session(problem: usize) -> String {
    format!("llg_rand_session_{problem}()")
}

/// Static tables and session accessors of every randomize problem.
pub(in crate::sim::emit_c) fn tables(model: &IrModel, out: &mut String) {
    for (index, problem) in model.rand_problems.iter().enumerate() {
        let name = problem_name(index);
        let mut args = problem.args.clone();
        let mut call_first = Vec::with_capacity(problem.calls.len());
        for call in &problem.calls {
            call_first.push(args.len());
            args.extend(&call.args);
        }
        let array = |out: &mut String, ty: &str, suffix: &str, items: Vec<String>| -> String {
            if items.is_empty() {
                return "NULL".to_owned();
            }
            out.push_str(&format!(
                "static const {ty} {name}_{suffix}[] = {{\n    {}\n}};\n",
                items.join(",\n    ")
            ));
            format!("{name}_{suffix}")
        };
        let nodes = array(
            out,
            "llg_cr_node_t",
            "nodes",
            problem
                .nodes
                .iter()
                .map(|node| {
                    format!(
                        "{{{}u, {}u, 0u, {}u, {}u, {}u, {}u}}",
                        node.op.code(),
                        u8::from(node.signed),
                        node.width,
                        node.a,
                        node.b,
                        node.c
                    )
                })
                .collect(),
        );
        let arg_table = array(
            out,
            "uint32_t",
            "args",
            args.iter().map(|arg| format!("{arg}u")).collect(),
        );
        let consts = array(
            out,
            "uint64_t",
            "consts",
            problem
                .consts
                .iter()
                .map(|word| format!("UINT64_C({word})"))
                .collect(),
        );
        let vars = array(
            out,
            "llg_cr_var_t",
            "vars",
            problem
                .vars
                .iter()
                .map(|var| {
                    format!(
                        "{{{}, {}u, {}u, {}, {}u}}",
                        c_string_literal(&var.name),
                        var.width,
                        u8::from(var.signed),
                        var.domain
                            .map_or_else(|| "LLG_CR_NONE".to_owned(), |node| format!("{node}u")),
                        u8::from(var.cyclic)
                    )
                })
                .collect(),
        );
        let blocks = array(
            out,
            "llg_cr_block_t",
            "blocks",
            problem
                .blocks
                .iter()
                .map(|block| format!("{{{}, {}u}}", c_string_literal(&block.name), block.root))
                .collect(),
        );
        let leaves = array(
            out,
            "llg_cr_leaf_t",
            "leaves",
            problem
                .leaves
                .iter()
                .map(|leaf| {
                    format!(
                        "{{{}, {}u, {}u}}",
                        c_string_literal(&leaf.name),
                        leaf.width,
                        u8::from(leaf.signed)
                    )
                })
                .collect(),
        );
        let calls = array(
            out,
            "llg_cr_call_t",
            "calls",
            problem
                .calls
                .iter()
                .zip(&call_first)
                .map(|(call, first)| {
                    format!(
                        "{{{}, {}u, {}u, {first}u, {}u}}",
                        c_string_literal(&call.name),
                        call.width,
                        u8::from(call.signed),
                        call.args.len()
                    )
                })
                .collect(),
        );
        let optional =
            |node: Option<u32>| node.map_or_else(|| "LLG_CR_NONE".to_owned(), |n| format!("{n}u"));
        let mut item_rows = Vec::new();
        let mut dist_rows = Vec::with_capacity(problem.dists.len());
        for dist in &problem.dists {
            dist_rows.push(format!(
                "{{{}u, {}, {}u, {}u}}",
                dist.expr,
                optional(dist.guard),
                item_rows.len(),
                dist.items.len()
            ));
            item_rows.extend(dist.items.iter().map(|item| {
                format!(
                    "{{{}u, {}u, {}u, {}, {}u, {}u}}",
                    item.matches,
                    item.lo,
                    item.hi,
                    optional(item.weight),
                    u8::from(item.per_range),
                    u8::from(item.signed)
                )
            }));
        }
        let item_count = item_rows.len();
        let dists = array(out, "llg_cr_dist_t", "dists", dist_rows);
        let dist_items = array(out, "llg_cr_dist_item_t", "dist_items", item_rows);
        let orders = array(
            out,
            "llg_cr_order_t",
            "orders",
            problem
                .orders
                .iter()
                .map(|order| format!("{{{}u, {}u, {}u}}", order.block, order.before, order.after))
                .collect(),
        );
        // Problems with units (SIM-030): class layouts, arrays and slots.
        let mut unit_items: Vec<u32> = Vec::new();
        let mut unit_rows = Vec::with_capacity(problem.units.len());
        for unit in &problem.units {
            unit_rows.push(format!(
                "{{{}, {}u, {}u, {}u, {}u, {}u, {}u, {}u, {}u}}",
                c_string_literal(&unit.name),
                unit_items.len(),
                unit.vars.len(),
                unit.arrays.len(),
                unit.slots.len(),
                unit.block_first,
                unit.block_count,
                unit.leaf_first,
                unit.leaf_count
            ));
            unit_items.extend(unit.vars.iter().chain(&unit.arrays).chain(&unit.slots));
        }
        let unit_item_count = unit_items.len();
        let units = array(out, "llg_cr_unit_t", "units", unit_rows);
        let unit_item_table = array(
            out,
            "uint32_t",
            "unit_items",
            unit_items.iter().map(|item| format!("{item}u")).collect(),
        );
        let rand_arrays = array(
            out,
            "llg_cr_array_t",
            "arrays",
            problem
                .arrays
                .iter()
                .map(|array| {
                    format!(
                        "{{{}, {}u, {}u, {}u, {}, {}}}",
                        c_string_literal(&array.name),
                        array.elem_width,
                        u8::from(array.elem_signed),
                        array.kind as u8,
                        array.left,
                        array.right
                    )
                })
                .collect(),
        );
        let slots = array(
            out,
            "llg_cr_slot_t",
            "slots",
            problem
                .slots
                .iter()
                .map(|slot| {
                    format!(
                        "{{{}, {}u, {}u, {}, {}}}",
                        c_string_literal(&slot.name),
                        slot.kind as u8,
                        u8::from(slot.is_array),
                        slot.left,
                        slot.right
                    )
                })
                .collect(),
        );
        out.push_str(&format!(
            "static const llg_cr_problem_t {name} = {{\n    {}, {nodes}, {}u, {arg_table}, {}u, {consts}, {}u,\n    {vars}, {}u, {blocks}, {}u, {leaves}, {}u, {calls}, {}u,\n    {dists}, {}u, {dist_items}, {item_count}u, {orders}, {}u,\n    NULL, 0u, NULL, 0u, {units}, {}u, {unit_item_table}, {unit_item_count}u,\n    {rand_arrays}, {}u, {slots}, {}u\n}};\n",
            c_string_literal(&problem.name),
            problem.nodes.len(),
            args.len(),
            problem.consts.len(),
            problem.vars.len(),
            problem.blocks.len(),
            problem.leaves.len(),
            problem.calls.len(),
            problem.dists.len(),
            problem.orders.len(),
            problem.units.len(),
            problem.arrays.len(),
            problem.slots.len(),
        ));
        out.push_str(&format!(
            "static llg_cr_session_t* llg_rand_slot_{index};\nstatic llg_cr_session_t* llg_rand_session_{index}(void) {{\n    return llg_cr_session_get(&{name}, &llg_rand_slot_{index});\n}}\n"
        ));
    }
}

impl Frame<'_, '_> {
    pub(super) fn rand_statement(&mut self, operation: &IrRandStmt) -> Result<(), String> {
        let problem = operation.problem();
        let s = session(problem);
        match operation {
            IrRandStmt::Begin { .. } => self.line(format!("llg_cr_begin({s});")),
            IrRandStmt::SetVar {
                var, value, active, ..
            } => {
                let width = self.ctx.model.rand_problems[problem].vars[*var as usize].width;
                let active = self.condition(active)?;
                let value = self.expression(value)?;
                self.line(format!(
                    "llg_cr_set_var_sv4({s}, {var}u, {}, {width}u, {active});",
                    value.code
                ));
                self.discard(value);
            }
            IrRandStmt::SetBlock { block, enabled, .. } => {
                let enabled = self.condition(enabled)?;
                self.line(format!("llg_cr_set_block({s}, {block}u, {enabled});"));
            }
            IrRandStmt::SetLeaf { leaf, value, .. } => {
                let width = self.ctx.model.rand_problems[problem].leaves[*leaf as usize].width;
                match value {
                    Some(value) => {
                        let value = self.expression(value)?;
                        self.line(format!(
                            "llg_cr_set_leaf_sv4({s}, {leaf}u, {}, {width}u, 0);",
                            value.code
                        ));
                        self.discard(value);
                    }
                    None => self.line(format!(
                        "llg_cr_set_leaf({s}, {leaf}u, NULL, LLG_CR_INPUT_NULL);"
                    )),
                }
            }
            IrRandStmt::SetCyclic { var, state, .. } => {
                let width = crate::sim::ir::rand_cyclic_state_width(
                    self.ctx.model.rand_problems[problem].vars[*var as usize].width,
                );
                let state = self.expression(state)?;
                self.line(format!(
                    "llg_cr_set_cyclic_sv4({s}, {var}u, {}, {width}u);",
                    state.code
                ));
                self.discard(state);
            }
            IrRandStmt::SetCallResult { call, value, .. } => {
                let width = self.ctx.model.rand_problems[problem].calls[*call as usize].width;
                let value = self.expression(value)?;
                self.line(format!(
                    "llg_cr_set_call_result_sv4({s}, {call}u, {}, {width}u);",
                    value.code
                ));
                self.discard(value);
            }
            IrRandStmt::InstRoot { handle, .. } => {
                let handle = self.chandle(handle)?;
                self.line(format!("llg_cr_inst_root({s}, {handle});"));
            }
            IrRandStmt::ISetVar {
                inst,
                var,
                value,
                active,
                ..
            } => {
                let width = self.ctx.model.rand_problems[problem].vars[*var as usize].width;
                let inst = self.rand_index(inst)?;
                let active = self.condition(active)?;
                let value = self.expression(value)?;
                self.line(format!(
                    "llg_cr_iset_var_sv4({s}, {inst}, {var}u, {}, {width}u, {active});",
                    value.code
                ));
                self.discard(value);
            }
            IrRandStmt::ISetBlock {
                inst,
                block,
                enabled,
                ..
            } => {
                let inst = self.rand_index(inst)?;
                let enabled = self.condition(enabled)?;
                self.line(format!(
                    "llg_cr_iset_block({s}, {inst}, {block}u, {enabled});"
                ));
            }
            IrRandStmt::ISetLeaf {
                inst, leaf, value, ..
            } => {
                let width = self.ctx.model.rand_problems[problem].leaves[*leaf as usize].width;
                let inst = self.rand_index(inst)?;
                match value {
                    Some(value) => {
                        let value = self.expression(value)?;
                        self.line(format!(
                            "llg_cr_iset_leaf_sv4({s}, {inst}, {leaf}u, {}, {width}u, 0);",
                            value.code
                        ));
                        self.discard(value);
                    }
                    None => self.line(format!(
                        "llg_cr_iset_leaf({s}, {inst}, {leaf}u, NULL, LLG_CR_INPUT_NULL);"
                    )),
                }
            }
            IrRandStmt::ISetCyclic {
                inst, var, state, ..
            } => {
                let width = crate::sim::ir::rand_cyclic_state_width(
                    self.ctx.model.rand_problems[problem].vars[*var as usize].width,
                );
                let inst = self.rand_index(inst)?;
                let state = self.expression(state)?;
                self.line(format!(
                    "llg_cr_iset_cyclic_sv4({s}, {inst}, {var}u, {}, {width}u);",
                    state.code
                ));
                self.discard(state);
            }
            IrRandStmt::SetArray {
                inst,
                array,
                size,
                active,
                ..
            } => {
                let inst = self.rand_index(inst)?;
                let size = self.rand_index(size)?;
                let active = self.condition(active)?;
                self.line(format!(
                    "llg_cr_set_array({s}, {inst}, {array}u, {size}, {active});"
                ));
            }
            IrRandStmt::SetElem {
                inst,
                array,
                index,
                value,
                ..
            } => {
                let width =
                    self.ctx.model.rand_problems[problem].arrays[*array as usize].elem_width;
                let inst = self.rand_index(inst)?;
                let index = self.rand_index(index)?;
                let value = self.expression(value)?;
                self.line(format!(
                    "llg_cr_set_elem_sv4({s}, {inst}, {array}u, {index}, {}, {width}u);",
                    value.code
                ));
                self.discard(value);
            }
            IrRandStmt::Link {
                inst,
                slot,
                child,
                unit,
                random,
                ..
            } => {
                let inst = self.rand_index(inst)?;
                let random = self.condition(random)?;
                let child = self.chandle(child)?;
                self.line(format!(
                    "(void)llg_cr_link({s}, {inst}, {slot}u, 0u, {child}, {unit}u, {random});"
                ));
            }
        }
        Ok(())
    }

    /// A packed instance, size or element index as a C `uint32_t`.
    fn rand_index(&mut self, value: &IrExpr) -> Result<String, String> {
        let value = self.expression(value)?;
        let index = self.scalar("uint32_t", format!("(uint32_t)sv4_to_u64({})", value.code));
        self.discard(value);
        Ok(index)
    }

    /// The object of instance `inst` (a problem with units).
    pub(super) fn rand_instance(
        &mut self,
        problem: usize,
        inst: &IrExpr,
    ) -> Result<String, String> {
        let inst = self.rand_index(inst)?;
        Ok(format!("llg_cr_inst_handle({}, {inst})", session(problem)))
    }

    pub(super) fn rand_function(
        &mut self,
        function: &IrSysFunc,
        expr: &IrExpr,
    ) -> Result<Value, String> {
        match function {
            IrSysFunc::RandStep {
                problem,
                stream,
                site,
            } => {
                let stream = match stream {
                    Some(handle) => {
                        let handle = self.chandle(handle)?;
                        format!("llg_class_rng({handle}, {})", c_string_literal(site))
                    }
                    None => "NULL".to_owned(),
                };
                Ok(self.value(
                    format!(
                        "sv4_from_i64(llg_randomize_step(&{}, {}, {stream}, {}), 32)",
                        problem_name(*problem),
                        session(*problem),
                        c_string_literal(site)
                    ),
                    32,
                    true,
                ))
            }
            IrSysFunc::RandCallReady { problem, call } => Ok(self.value(
                format!(
                    "sv4_from_u64((uint64_t)llg_cr_call_ready({}, {call}u), 1, 0)",
                    session(*problem)
                ),
                1,
                false,
            )),
            IrSysFunc::RandValue { problem, var } => Ok(self.value(
                format!(
                    "llg_cr_value_sv4({}, {var}u, {}u, {})",
                    session(*problem),
                    expr.width,
                    u8::from(expr.signed)
                ),
                expr.width,
                expr.signed,
            )),
            IrSysFunc::RandCyclic { problem, var } => Ok(self.value(
                format!(
                    "llg_cr_cyclic_sv4({}, {var}u, {}u)",
                    session(*problem),
                    expr.width
                ),
                expr.width,
                false,
            )),
            IrSysFunc::RandInst {
                problem,
                query,
                inst,
                index,
            } => {
                use crate::sim::ir::IrRandQuery;
                let s = session(*problem);
                let inst = match inst {
                    Some(inst) => self.rand_index(inst)?,
                    None => String::new(),
                };
                let flag = |call: String| format!("sv4_from_u64((uint64_t)({call} != 0), 1, 0)");
                let code = match query {
                    IrRandQuery::NextInst => {
                        format!("sv4_from_i64((int64_t)(int32_t)llg_cr_next_inst({s}), 32)")
                    }
                    IrRandQuery::Count => {
                        format!("sv4_from_i64((int64_t)llg_cr_inst_count({s}), 32)")
                    }
                    IrRandQuery::Pre => flag(format!("llg_cr_inst_pre({s}, {inst})")),
                    IrRandQuery::Unit => {
                        format!("sv4_from_i64((int64_t)llg_cr_inst_unit({s}, {inst}), 32)")
                    }
                    IrRandQuery::Random => flag(format!("llg_cr_inst_random({s}, {inst})")),
                    IrRandQuery::Value(var) => format!(
                        "llg_cr_ivalue_sv4({s}, {inst}, {var}u, {}u, {})",
                        expr.width,
                        u8::from(expr.signed)
                    ),
                    IrRandQuery::Active(var) => {
                        flag(format!("llg_cr_iactive({s}, {inst}, {var}u)"))
                    }
                    IrRandQuery::Cyclic(var) => {
                        format!("llg_cr_icyclic_sv4({s}, {inst}, {var}u, {}u)", expr.width)
                    }
                    IrRandQuery::ArraySize(array) => format!(
                        "sv4_from_i64((int64_t)llg_cr_array_size({s}, {inst}, {array}u), 32)"
                    ),
                    IrRandQuery::ArrayActive(array) => {
                        flag(format!("llg_cr_array_active({s}, {inst}, {array}u)"))
                    }
                    IrRandQuery::Elem(array) => {
                        let index = match index {
                            Some(index) => self.rand_index(index)?,
                            None => return Err("element query without an index".to_owned()),
                        };
                        format!(
                            "llg_cr_elem_value_sv4({s}, {inst}, {array}u, {index}, {}u, {})",
                            expr.width,
                            u8::from(expr.signed)
                        )
                    }
                };
                Ok(self.value(code, expr.width, expr.signed))
            }
            _ => Err("not a randomize primitive".to_owned()),
        }
    }
}
