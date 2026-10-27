//! The constraint problem of a free checker variable set (IEEE 1800-2009
//! 17.7.2): the active free variables are its random variables and each
//! enforced assume statement contributes one constraint over them.

use super::problems::ProblemBuilder;
use super::sites::{DirectRead, ProblemPlan, VarPlan};
use super::*;
use crate::sim::ir::{IrSampledCall, IrSampledFunc, IrSysFunc};

/// One active free variable of a set.
pub(in crate::sim::codegen) struct FreeSolveVar {
    pub(in crate::sim::codegen) signal: usize,
    pub(in crate::sim::codegen) name: String,
    pub(in crate::sim::codegen) width: u32,
    pub(in crate::sim::codegen) signed: bool,
    pub(in crate::sim::codegen) two_state: bool,
    /// Whether the solve may assign it (an initialized variable keeps its
    /// initial value in the first time step).
    pub(in crate::sim::codegen) active: IrExpr,
}

impl Codegen<'_> {
    /// The statements of one solve: feed the set's problem, solve it and
    /// commit a solution. An unsuccessful attempt leaves the variables
    /// unchanged, which 17.7.2 allows ("any values may be chosen for the
    /// active variables"). Each constraint is an assertion expression over
    /// signals; reads of the set's variables become random variables and
    /// every other signal read keeps its assertion meaning: its sampled
    /// value, or the current value of another active free variable.
    /// Returns the statements and, per constraint, whether the solver can
    /// express it (an inexpressible one is left to the assumption check).
    pub(in crate::sim::codegen) fn free_variable_solve(
        &mut self,
        tag: &str,
        site: &str,
        vars: &[FreeSolveVar],
        constraints: &[IrExpr],
    ) -> Result<(Vec<IrStmt>, Vec<bool>), String> {
        self.model.random.threads = true;
        let mut builder = ProblemBuilder::new(site.to_owned());
        let mut placeholders = HashMap::new();
        let mut plans = Vec::new();
        for var in vars {
            let (index, placeholder) = builder.add_var(var.name.clone(), var.width, var.signed);
            placeholders.insert(var.signal, local_read(&placeholder, var.width, var.signed));
            let current = IrExpr::new(IrExprKind::SigRead(var.signal), var.width, var.signed, None);
            plans.push(VarPlan {
                index,
                width: var.width,
                signed: var.signed,
                two_state: var.two_state,
                current,
                lhs: IrLhs::Whole(var.signal),
                active: var.active.clone(),
                direct: DirectRead::Signal(var.signal),
                cyclic: None,
            });
        }
        let mut blocks = Vec::new();
        let mut enforced = Vec::with_capacity(constraints.len());
        for (index, constraint) in constraints.iter().enumerate() {
            let mut expression = constraint.clone();
            crate::sim::opt::walk_expr_mut(&mut expression, &mut |node| {
                let IrExprKind::SigRead(signal) = node.kind else {
                    return;
                };
                *node = match placeholders.get(&signal) {
                    Some(placeholder) => placeholder.clone(),
                    None => IrExpr::new(
                        IrExprKind::SysFunc(Box::new(IrSysFunc::Sampled(IrSampledCall::new(
                            IrSampledFunc::Sampled,
                            node.clone(),
                            None,
                            0,
                        )))),
                        node.width,
                        node.signed,
                        node.fill,
                    ),
                };
            });
            builder.item = format!("{site} (assume {index})");
            // A form the solver cannot express adds no constraint: the
            // solve may then miss a solution, which 17.7.2 permits. Nodes it
            // already added stay unreferenced by any block.
            match builder.expr(&expression) {
                Ok(root) => {
                    let root = builder.truth(root);
                    builder.add_block(format!("assume {index}"), root);
                    blocks.push(packed_const(1, 1, false));
                    enforced.push(true);
                }
                Err(_) => enforced.push(false),
            }
        }
        let result = format!("{tag}_r");
        let mut statements = vec![IrStmt::DeclLocal {
            name: result.clone(),
            width: 32,
            signed: true,
            init: Some(Box::new(packed_const(0, 32, true))),
            two_state: true,
        }];
        let plan = ProblemPlan {
            builder,
            vars: plans,
            blocks,
        };
        statements.push(IrStmt::Block(self.feed_problem_tagged(
            tag,
            plan,
            None,
            &result,
            site,
            Vec::new(),
        )?));
        Ok((vec![IrStmt::Block(statements)], enforced))
    }
}
