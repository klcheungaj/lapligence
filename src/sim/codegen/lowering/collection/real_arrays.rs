//! Real fixed-array subroutine storage (SIM-005).
//!
//! Formals, results and locals whose type is a fixed unpacked array of `real`
//! or `shortreal` elements keep numeric `double` cells: a lexical activation
//! buffer for automatic storage and a model global for static storage. Their
//! elements use the ordinary real array element paths. They never become
//! packed vectors and never enter the integral fixed-value descriptor ABI,
//! so no real value is ever reinterpreted as a bit pattern.
use super::*;

impl Codegen<'_> {
    /// Declared dimensions (all ranks, outermost first) and shortreal flag of
    /// a fixed unpacked array whose leaves are `real`/`shortreal`.
    pub(in super::super) fn real_array_shape(
        &self,
        node: NodeId,
    ) -> Option<(Vec<(i32, i32)>, bool)> {
        let mut descriptor = self.query_descriptor(node)?;
        let mut dims = Vec::new();
        loop {
            match &descriptor.shape {
                TypeShape::FixedArray {
                    dimensions,
                    element,
                } => {
                    dims.extend(dimensions.iter().copied());
                    descriptor = element;
                }
                TypeShape::Real { shortreal } => {
                    return (!dims.is_empty()).then_some((dims, *shortreal));
                }
                _ => return None,
            }
        }
    }

    /// Whether `function` returns a real fixed array (a trailing output formal).
    pub(in super::super) fn real_array_return(&self, function: NodeId) -> bool {
        matches!(self.kind(function), NodeKind::FuncTask { ret: Some(_), .. })
            && self.real_array_shape(function).is_some()
    }

    /// Real-array storage bound to a subroutine formal or result node.
    pub(in super::super) fn real_formal_array(&self, node: NodeId) -> Option<usize> {
        matches!(
            self.kind(node),
            NodeKind::FuncArg { .. } | NodeKind::FuncTask { .. }
        )
        .then(|| self.array_globals.get(&node))
        .flatten()
        .filter(|array| self.model.arrays[array.ir].real)
        .map(|array| array.ir)
    }

    fn real_subroutine_array(&mut self, node: NodeId, automatic: bool) -> Result<(), String> {
        if self.array_globals.contains_key(&node) {
            return Ok(());
        }
        let Some((dims, shortreal)) = self.real_array_shape(node) else {
            return Ok(());
        };
        let total = fixed_values::fixed_array_cell_count(&dims)
            .map_err(|error| format!("real array `{}`: {error}", self.node(node).name))?;
        let ir = self.model.arrays.len();
        // Activation buffers are addressed through the frame; static storage
        // is a model global so coroutine bodies address it directly.
        let global = if automatic {
            format!("_llg_real_{ir}")
        } else {
            format!("S_llg_real_{ir}")
        };
        self.model.arrays.push(crate::sim::ir::IrArray {
            activation: automatic,
            net: None,
            net_elements: Vec::new(),
            element_default: None,
            element_uninitialized: None,
            c_name: global.clone(),
            hdl_name: String::new(),
            elem_width: 0,
            signed: false,
            two_state: false,
            real: true,
            shortreal,
            dims: dims.clone(),
            total,
        });
        self.array_globals.insert(
            node,
            ArrayInfo {
                global,
                elem_width: 0,
                signed: false,
                real: true,
                shortreal,
                is_net: false,
                dims,
                init: None,
                ir,
            },
        );
        Ok(())
    }

    /// A fresh lexical real array with the shape of `like`, declared by the
    /// caller with `FixedArrayDeclare` (call results, temporaries).
    pub(in super::super) fn real_array_temporary_like(&mut self, like: usize) -> usize {
        let mut array = self.model.arrays[like].clone();
        let ir = self.model.arrays.len();
        array.activation = true;
        array.c_name = format!("_llg_real_{ir}");
        self.model.arrays.push(array);
        ir
    }

    fn real_array_locals(&self, node: NodeId, output: &mut Vec<NodeId>) {
        if matches!(
            self.kind(node),
            NodeKind::Array { .. } | NodeKind::Var { .. }
        ) && self.real_array_shape(node).is_some()
        {
            output.push(node);
            return;
        }
        for child in &self.node(node).children {
            self.real_array_locals(*child, output);
        }
    }

    /// Allocate real-array storage for the formals, result and locals of one
    /// subroutine before its signature and body are lowered.
    pub(in super::super) fn prepare_real_array_function(
        &mut self,
        function: NodeId,
        automatic: bool,
    ) -> Result<(), String> {
        let formals = self
            .func_formals(function)
            .into_iter()
            .map(|(node, _)| node)
            .collect::<Vec<_>>();
        for formal in formals {
            self.real_subroutine_array(formal, automatic)?;
        }
        if self.real_array_return(function) {
            self.real_subroutine_array(function, automatic)?;
        }
        let mut locals = Vec::new();
        if let Some(body) = self.func_body(function) {
            self.real_array_locals(body, &mut locals);
        }
        for local in locals {
            let automatic = match self.db.variable_lifetime(local) {
                VariableLifetime::Automatic => true,
                VariableLifetime::Static => false,
                VariableLifetime::Unavailable => {
                    return Err(format!(
                        "resolved lifetime is unavailable for real array local `{}`",
                        self.node(local).name
                    ))
                }
            };
            self.real_subroutine_array(local, automatic)?;
        }
        Ok(())
    }

    /// Operand for a real-array formal. Whole real storage is passed as
    /// cells (copied for inputs/outputs, aliased for `ref`); any other input
    /// expression is lowered to its declaration-order element values.
    pub(in super::super) fn real_array_argument(
        &mut self,
        path: &str,
        formal: NodeId,
        actual: NodeId,
    ) -> Result<IrCallArg, String> {
        let shape = self
            .real_formal_array(formal)
            .ok_or("real-array formal has no storage")?;
        let (total, shortreal) = {
            let array = &self.model.arrays[shape];
            (array.total, array.shortreal)
        };
        let input = matches!(
            self.kind(formal),
            NodeKind::FuncArg {
                direction: DbDirection::Input,
                ..
            }
        );
        if let Some(info) = self.array_of(actual) {
            let array = &self.model.arrays[info.ir];
            if array.real && array.total == total && array.shortreal == shortreal {
                return Ok(IrCallArg::RealArray(info.ir));
            }
        }
        if !input {
            return Err(format!(
                "real-array output, inout or ref actual in `{path}` must be a whole real array variable"
            ));
        }
        if let Some((array, call)) = self.real_array_result_call(path, actual)? {
            return Ok(IrCallArg::RealArrayCall {
                array,
                call: Box::new(call),
            });
        }
        let dims = self.model.arrays[shape].dims.clone();
        let values = self.real_array_values(path, actual, &dims)?;
        if values.len() as u64 != total {
            return Err(format!(
                "real-array operand in `{path}` has {} elements but its formal has {total}",
                values.len()
            ));
        }
        Ok(IrCallArg::RealArrayValues(values))
    }

    /// A call of a real-array-result function, lowered as a statement call
    /// whose trailing output operand is a fresh lexical result array. The
    /// caller declares that array before the call.
    pub(in super::super) fn real_array_result_call(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<(usize, IrCall)>, String> {
        let node = self.p30_unwrap_cast(node);
        let (name, callee) = match self.kind(node) {
            NodeKind::FuncCall { name, callee, .. } => (name.clone(), *callee),
            _ => return Ok(None),
        };
        let (function, _) = self.resolve_callee_env(self.inst, &name, false, callee)?;
        let Some(result) = self
            .real_array_return(function)
            .then(|| self.real_formal_array(function))
            .flatten()
        else {
            return Ok(None);
        };
        let temporary = self.real_array_temporary_like(result);
        let expression = self.lower_func_call_expr(path, node, &name, callee)?;
        let IrExprKind::CallFn(mut call) = expression.kind else {
            return Err("real-array call did not lower to a typed call".into());
        };
        let outputs = self.model.funcs[call.f]
            .formals
            .iter()
            .filter(|formal| formal.is_address())
            .count();
        call.args
            .insert(outputs - 1, IrCallArg::RealArray(temporary));
        Ok(Some((
            temporary,
            IrCall::new(call.f, call.args, call.depth, Vec::new(), Vec::new()),
        )))
    }
}
