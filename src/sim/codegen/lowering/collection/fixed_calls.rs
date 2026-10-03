//! Lexical array-value activations avoid packed transport for large fixed returns.
use super::*;

impl Codegen<'_> {
    /// Integral fixed arrays beyond packed capacity use descriptor transport.
    /// Other oversized aggregates (records) keep their packed-limit diagnostic.
    fn descriptor_transport(&self, node: NodeId) -> bool {
        self.query_descriptor(node).is_some_and(|descriptor| {
            matches!(&descriptor.shape, TypeShape::FixedArray { element, .. }
                if Self::fixed_descriptor_width(element).is_some())
                && fixed_values::fixed_width_bits(descriptor)
                    .is_some_and(|width| width > u64::from(LLG_MAX_WIDTH))
        })
    }

    pub(in super::super) fn nonflatten_function(&self, node: NodeId) -> bool {
        self.descriptor_transport(node)
    }

    pub(in super::super) fn fixed_activation_array(
        &mut self,
        node: NodeId,
    ) -> Result<ArrayInfo, String> {
        let descriptor = self
            .query_descriptor(node)
            .ok_or("missing fixed activation descriptor")?;
        let TypeShape::FixedArray {
            dimensions,
            element,
        } = &descriptor.shape
        else {
            return Err("fixed activation requires an integral array".into());
        };
        let width = Self::fixed_descriptor_width(element)
            .ok_or("fixed activation element exceeds packed capacity")?;
        let total = fixed_values::fixed_array_cell_count(dimensions)?;
        let ir = self.model.arrays.len();
        let name = format!("_llg_fixed_{ir}");
        let info = ArrayInfo {
            global: name.clone(),
            elem_width: width,
            signed: element.info.signed,
            real: false,
            shortreal: false,
            is_net: false,
            dims: dimensions.clone(),
            init: None,
            ir,
        };
        self.model.arrays.push(crate::sim::ir::IrArray {
            activation: true,
            net_elements: Vec::new(),
            element_default: Self::fixed_descriptor_default(element),
            c_name: name,
            hdl_name: String::new(),
            elem_width: width,
            signed: element.info.signed,
            two_state: element.two_state,
            real: false,
            shortreal: false,
            dims: dimensions.clone(),
            total,
        });
        Ok(info)
    }

    fn fixed_call_locals(&self, node: NodeId, output: &mut Vec<NodeId>) {
        if matches!(
            self.kind(node),
            NodeKind::Array { .. } | NodeKind::Var { .. }
        ) && self.descriptor_transport(node)
        {
            output.push(node);
            return;
        }
        for child in &self.node(node).children {
            self.fixed_call_locals(*child, output);
        }
    }

    pub(in super::super) fn fixed_formal_array(&self, node: NodeId) -> Option<usize> {
        self.array_globals
            .get(&node)
            .filter(|_| self.descriptor_transport(node))
            .map(|array| array.ir)
    }

    pub(super) fn prepare_fixed_function(
        &mut self,
        function: NodeId,
        automatic: bool,
    ) -> Result<(), String> {
        let mut nodes = self
            .func_formals(function)
            .into_iter()
            .map(|(node, _)| node)
            .collect::<Vec<_>>();
        if self.nonflatten_function(function) {
            nodes.push(function);
        }
        if let Some(body) = self.func_body(function) {
            self.fixed_call_locals(body, &mut nodes);
        }
        for node in nodes {
            if !self.descriptor_transport(node) || self.array_globals.contains_key(&node) {
                continue;
            }
            let lifetime = if matches!(
                self.kind(node),
                NodeKind::FuncArg { .. } | NodeKind::FuncTask { .. }
            ) {
                automatic
            } else {
                self.db.variable_lifetime(node) == VariableLifetime::Automatic
            };
            let info = self.fixed_activation_array(node)?;
            self.model.arrays[info.ir].activation = lifetime;
            self.array_globals.insert(node, info);
        }
        Ok(())
    }

    pub(in super::super) fn lower_nonflatten_call(
        &mut self,
        path: &str,
        call: NodeId,
        destination: NodeId,
    ) -> Result<Option<IrStmt>, String> {
        let call = self.p30_unwrap_cast(call);
        let (name, callee) = match self.kind(call) {
            NodeKind::FuncCall { name, callee, .. } => (name.clone(), *callee),
            _ => return Ok(None),
        };
        let (function, _) = self.resolve_callee_env(self.inst, &name, false, callee)?;
        if !self.nonflatten_function(function) {
            return Ok(None);
        }
        let dst = self
            .array_of(destination)
            .ok_or("fixed function result requires an array destination")?
            .ir;
        let expression = self.lower_func_call_expr(path, call, &name, callee)?;
        let IrExprKind::CallFn(mut expression) = expression.kind else {
            return Err("fixed call did not lower to a typed call".into());
        };
        let output_count = self.model.funcs[expression.f]
            .formals
            .iter()
            .filter(|formal| formal.is_address())
            .count();
        expression
            .args
            .insert(output_count - 1, IrCallArg::FixedArray(dst));
        Ok(Some(IrStmt::Call(IrCall::new(
            expression.f,
            expression.args,
            expression.depth,
            Vec::new(),
            Vec::new(),
        ))))
    }
}
