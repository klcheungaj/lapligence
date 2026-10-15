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
                        "{{{}, {}u, {}u, {}}}",
                        c_string_literal(&var.name),
                        var.width,
                        u8::from(var.signed),
                        var.domain
                            .map_or_else(|| "LLG_CR_NONE".to_owned(), |node| format!("{node}u"))
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
        out.push_str(&format!(
            "static const llg_cr_problem_t {name} = {{\n    {}, {nodes}, {}u, {arg_table}, {}u, {consts}, {}u,\n    {vars}, {}u, {blocks}, {}u, {leaves}, {}u, {calls}, {}u\n}};\n",
            c_string_literal(&problem.name),
            problem.nodes.len(),
            args.len(),
            problem.consts.len(),
            problem.vars.len(),
            problem.blocks.len(),
            problem.leaves.len(),
            problem.calls.len(),
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
            IrRandStmt::SetCallResult { call, value, .. } => {
                let width = self.ctx.model.rand_problems[problem].calls[*call as usize].width;
                let value = self.expression(value)?;
                self.line(format!(
                    "llg_cr_set_call_result_sv4({s}, {call}u, {}, {width}u);",
                    value.code
                ));
                self.discard(value);
            }
        }
        Ok(())
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
            _ => Err("not a randomize primitive".to_owned()),
        }
    }
}
