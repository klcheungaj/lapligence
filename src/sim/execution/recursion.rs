//! Recursion analysis of synchronous (non-suspending) subprograms.
//!
//! A plain C function call per SystemVerilog call would make native stack use
//! proportional to the dynamic recursion depth. Every subprogram in a cyclic
//! strongly connected component of the synchronous call graph is therefore
//! also emitted as a stackless coroutine, and calls between members of one
//! component use the chain arena (`LLG_CO_CALL_ARENA`). Calls entering a
//! component from outside go through an ordinary C function that runs the
//! coroutine on a synchronous driver (`llg_co_sync_*`), so the native stack
//! grows by one driver per component on a call path, which the acyclic
//! condensation bounds statically.
//!
//! The graph has one node per subprogram that has a body and is neither
//! suspendable (suspendable recursion already uses the arena, D17) nor
//! inline-expanded. Dynamic dispatch contributes an edge to every possible
//! implementation: class virtual calls to every method with the same slot and
//! virtual-interface calls to every instance implementation. Re-entry through
//! foreign DPI code is invisible here and keeps native recursion.

use std::collections::{BTreeMap, BTreeSet};

use crate::sim::ir::IrModel;

use super::analysis::strongly_connected_components;
use super::{direct_call_targets, CallTarget};

/// Cyclic components of the synchronous call graph.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct RecursionAnalysis {
    /// Component index of each recursive subprogram, by function index.
    components: Vec<Option<usize>>,
    /// Class virtual slots called recursively from a component member.
    dispatch_slots: BTreeSet<usize>,
    /// Virtual-interface `(interface, method)` pairs called recursively.
    interface_methods: BTreeSet<(usize, usize)>,
}

impl RecursionAnalysis {
    pub(super) fn analyze(ir: &IrModel, suspendable: &BTreeSet<usize>) -> Self {
        let nodes = ir
            .funcs
            .iter()
            .enumerate()
            .filter(|(index, function)| {
                !function.is_inline_expanded()
                    && function.dpi_import().is_none()
                    && !suspendable.contains(index)
            })
            .map(|(index, _)| index)
            .collect::<BTreeSet<_>>();
        let mut targets = BTreeMap::new();
        let mut graph = BTreeMap::new();
        for &function in &nodes {
            let calls = direct_call_targets(ir, &ir.funcs[function].body);
            let callees = calls
                .iter()
                .flat_map(|target| target.functions(ir))
                .filter(|callee| nodes.contains(callee))
                .collect::<BTreeSet<_>>();
            graph.insert(function, callees);
            targets.insert(function, calls);
        }
        let mut components = vec![None; ir.funcs.len()];
        let mut cyclic = 0usize;
        for members in strongly_connected_components(&nodes, &graph) {
            let recursive = members.len() > 1
                || members
                    .first()
                    .is_some_and(|member| graph.get(member).is_some_and(|c| c.contains(member)));
            if recursive {
                for member in members {
                    components[member] = Some(cyclic);
                }
                cyclic += 1;
            }
        }
        let mut analysis = Self {
            components,
            ..Self::default()
        };
        for (function, calls) in &targets {
            for target in calls {
                if !analysis.recursive_call(ir, *function, target) {
                    continue;
                }
                match *target {
                    CallTarget::Static(_) => {}
                    CallTarget::Virtual(method) => {
                        if let Some(slot) = ir.funcs[method].virtual_slot {
                            analysis.dispatch_slots.insert(slot);
                        }
                    }
                    CallTarget::Interface(interface, method) => {
                        analysis.interface_methods.insert((interface, method));
                    }
                }
            }
        }
        analysis
    }

    pub(super) fn component(&self, function: usize) -> Option<usize> {
        self.components.get(function).copied().flatten()
    }

    /// Whether `caller` enters `target` through the chain arena: some
    /// implementation of the target belongs to the caller's own component.
    pub(super) fn recursive_call(&self, ir: &IrModel, caller: usize, target: &CallTarget) -> bool {
        let Some(component) = self.component(caller) else {
            return false;
        };
        target
            .functions(ir)
            .into_iter()
            .any(|callee| self.component(callee) == Some(component))
    }

    pub(super) fn dispatch_slots(&self) -> &BTreeSet<usize> {
        &self.dispatch_slots
    }

    pub(super) fn interface_methods(&self) -> &BTreeSet<(usize, usize)> {
        &self.interface_methods
    }
}

#[cfg(test)]
mod tests {
    use crate::sim::execution::{CallTarget, ExecutionModel};
    use crate::sim::ir::{
        IrCall, IrCallExpr, IrDelay, IrDepth, IrExpr, IrExprKind, IrFunc, IrModel, IrModelParts,
        IrProcess, IrShape, IrStmt, IrType,
    };

    fn call(callee: usize) -> IrStmt {
        IrStmt::Call(Box::new(IrCall::new(
            callee,
            vec![],
            IrDepth::FUNC,
            vec![],
            vec![],
        )))
    }

    /// A value-returning function whose body is `return <call>` for each
    /// callee: the recursion happens inside expressions.
    fn function(index: usize, callees: &[usize]) -> IrFunc {
        let ty = IrType::Packed {
            width: 32,
            signed: true,
            two_state: true,
        };
        let mut body = callees
            .iter()
            .map(|callee| IrStmt::Return {
                value: Some(Box::new(IrExpr::new(
                    IrExprKind::CallFn(Box::new(IrCallExpr::new(
                        *callee,
                        vec![],
                        IrDepth::FUNC,
                        false,
                    ))),
                    32,
                    true,
                    None,
                ))),
            })
            .collect::<Vec<_>>();
        body.push(IrStmt::Return { value: None });
        IrFunc::new(format!("f{index}"), Some(ty), vec![], vec![], vec![], body)
    }

    fn task(index: usize, callees: &[usize], suspends: bool) -> IrFunc {
        let mut body = callees
            .iter()
            .map(|callee| call(*callee))
            .collect::<Vec<_>>();
        if suspends {
            body.push(IrStmt::Delay {
                ticks: IrDelay::Constant(1),
            });
        }
        IrFunc::new(format!("t{index}"), None, vec![], vec![], vec![], body)
    }

    fn lower(funcs: Vec<IrFunc>, roots: &[usize]) -> ExecutionModel {
        let body = roots
            .iter()
            .map(|callee| {
                IrStmt::Call(Box::new(IrCall::new(
                    *callee,
                    vec![],
                    IrDepth::PROC,
                    vec![],
                    vec![],
                )))
            })
            .collect();
        let process = IrProcess::new("p0".into(), "top.p".into(), IrShape::RunOnce, vec![], body);
        let ir = IrModel::from_parts(
            "recursion".into(),
            1,
            IrModelParts {
                funcs,
                processes: vec![process],
                spawns: vec!["p0".into()],
                ..IrModelParts::default()
            },
        )
        .unwrap();
        ExecutionModel::lower(ir).unwrap()
    }

    #[test]
    fn only_cyclic_synchronous_components_are_recursive() {
        // f0 calls itself; f1 <-> f2 are mutually recursive; f3 calls into
        // both cycles without being called back.
        let model = lower(
            vec![
                function(0, &[0]),
                function(1, &[2]),
                function(2, &[1]),
                function(3, &[0, 1]),
            ],
            &[3],
        );
        let analysis = model.analysis();
        let ir = model.ir();
        assert!(analysis.is_recursive_function(0));
        assert!(analysis.is_recursive_function(1));
        assert!(analysis.is_recursive_function(2));
        assert!(!analysis.is_recursive_function(3));
        assert_eq!(
            analysis.recursive_component(1),
            analysis.recursive_component(2)
        );
        assert_ne!(
            analysis.recursive_component(0),
            analysis.recursive_component(1)
        );
        assert!(analysis.is_recursive_call(ir, 0, &CallTarget::Static(0)));
        assert!(analysis.is_recursive_call(ir, 1, &CallTarget::Static(2)));
        // Entering a component from outside, or another component, stays a
        // plain call to the synchronous entry.
        assert!(!analysis.is_recursive_call(ir, 3, &CallTarget::Static(0)));
        assert!(!analysis.is_recursive_call(ir, 0, &CallTarget::Static(1)));
        assert!(!analysis.is_coroutine_function(0));
    }

    #[test]
    fn suspendable_recursion_keeps_the_existing_coroutine_path() {
        // t0 is a recursive timing task (D17 arena calls); t1 is a
        // non-suspending recursive task and t2 a non-recursive caller.
        let model = lower(
            vec![
                task(0, &[0], true),
                task(1, &[1], false),
                task(2, &[1], false),
            ],
            &[0, 2],
        );
        let analysis = model.analysis();
        assert!(analysis.is_coroutine_function(0));
        assert!(!analysis.is_recursive_function(0));
        assert!(analysis.is_recursive_function(1));
        assert!(!analysis.is_recursive_function(2));
        assert!(analysis.recursive_dispatch_slots().is_empty());
        assert!(analysis.recursive_interface_methods().is_empty());
    }
}
