//! Lexical array-value activations avoid packed transport for large fixed returns.
use super::*;

impl Codegen<'_> {
    pub(in super::super) fn nonflatten_function(&self, node: NodeId) -> bool {
        self.query_descriptor(node).is_some_and(|descriptor| {
            let TypeShape::FixedArray { .. } = &descriptor.shape else {
                return false;
            };
            fixed_values::fixed_width_bits(descriptor)
                .is_some_and(|width| width > u64::from(LLG_MAX_WIDTH))
        })
    }

    fn fixed_activation_array(&mut self, node: NodeId) -> Result<ArrayInfo, String> {
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
        ) && self
            .query_descriptor(node)
            .is_some_and(|descriptor| matches!(descriptor.shape, TypeShape::FixedArray { .. }))
        {
            output.push(node);
            return;
        }
        for child in &self.node(node).children {
            self.fixed_call_locals(*child, output);
        }
    }

    pub(in super::super) fn lower_nonflatten_call(
        &mut self,
        path: &str,
        call: NodeId,
        destination: NodeId,
    ) -> Result<Option<IrStmt>, String> {
        let call = self.p30_unwrap_cast(call);
        let NodeKind::FuncCall { name, callee, .. } = self.kind(call) else {
            return Ok(None);
        };
        let (function, instance) = self.resolve_callee_env(self.inst, name, false, *callee)?;
        if !self.nonflatten_function(function) {
            return Ok(None);
        }
        if self.nonflatten_calls.contains(&function) {
            return Err(
                "recursive non-flattened fixed-value calls require descriptor call transport"
                    .into(),
            );
        }
        if !matches!(
            self.kind(function),
            NodeKind::FuncTask {
                automatic: true,
                is_task: false,
                ..
            }
        ) {
            return Err("non-flattened fixed-value call requires an automatic function".into());
        }
        let body = self
            .func_body(function)
            .ok_or("fixed-value function has no body")?;
        if self.node_has_stack_backed_subroutine_nba(body, function, true) {
            return Err(
                "nonblocking assignment in fixed-value function targets automatic storage".into(),
            );
        }
        let formals = self.func_formals(function);
        let bound = self.bind_call_args(self.inst, &formals, &self.call_argument_nodes(call))?;
        let mut before = Vec::new();
        let mut arrays = Vec::new();
        let mut context = FuncCtx {
            name: self.node(function).name.clone(),
            ret_node: Some(function),
            def_node: Some(function),
            ..Default::default()
        };
        let mut arguments = vec![None; formals.len()];
        for (index, ((formal, output), actual)) in formals.iter().zip(&bound).enumerate() {
            if *output
                || matches!(
                    self.kind(*formal),
                    NodeKind::FuncArg {
                        direction: DbDirection::Ref,
                        ..
                    }
                )
            {
                return Err("non-flattened value function output/reference formal requires descriptor call transport".into());
            }
            if self
                .query_descriptor(*formal)
                .is_some_and(|descriptor| matches!(descriptor.shape, TypeShape::FixedArray { .. }))
            {
                let source = self
                    .array_of(self.p30_unwrap_cast(actual.expr))
                    .cloned()
                    .ok_or("fixed-value input must name fixed array storage")?;
                let info = self.fixed_activation_array(*formal)?;
                before.push(IrStmt::FixedArrayDeclare(info.ir));
                before.push(IrStmt::FixedArrayCopy {
                    dst: info.ir,
                    src: source.ir,
                    nba: false,
                    slice: 0,
                });
                arrays.push((*formal, info));
            } else {
                let value = self.lower_bound_arg(path, &formals, &bound, index, &mut arguments)?;
                let name = self.new_fn_name(path, "fixed_argument");
                let read = IrExpr::new(
                    IrExprKind::LocalRead(name.clone()),
                    value.width,
                    value.signed,
                    None,
                );
                arguments[index] = Some(read.clone());
                context.arg_ir.insert(*formal, read);
                context.arg_write.insert(*formal, format!("&{name}"));
                before.push(IrStmt::DeclLocal {
                    name,
                    width: value.width,
                    signed: value.signed,
                    two_state: self.db.is_two_state_type(*formal),
                    init: Some(Box::new(value)),
                });
            }
        }
        let result = self.fixed_activation_array(function)?;
        before.push(IrStmt::FixedArrayDeclare(result.ir));
        arrays.push((function, result.clone()));
        let mut local_nodes = Vec::new();
        self.fixed_call_locals(body, &mut local_nodes);
        for node in local_nodes {
            if self.db.variable_lifetime(node) != VariableLifetime::Automatic {
                return Err("non-flattened static function-local array requires persistent descriptor storage".into());
            }
            arrays.push((node, self.fixed_activation_array(node)?));
        }
        let mappings = arrays
            .into_iter()
            .map(|(node, info)| (node, self.array_globals.insert(node, info)))
            .collect::<Vec<_>>();
        let saved = (self.func.take(), self.inst, self.depth_arg.clone());
        self.nonflatten_calls.push(function);
        let lowered = (|| {
            let mut chandle = HashMap::new();
            let mut process = HashMap::new();
            let mut sequence = 0;
            self.collect_func_locals(
                body,
                &mut context.locals,
                &mut chandle,
                &mut process,
                &mut sequence,
                &format!("_fixed_call_{}", result.ir),
            )?;
            if !chandle.is_empty() || !process.is_empty() {
                return Err("native locals in a non-flattened fixed function require native activation transport".into());
            }
            let done = self.new_fn_name(path, "fixed_return");
            let inline = InlineCtx {
                done_label: done.clone(),
                chain: Vec::new(),
                used: false,
            };
            let mut emitter = EmitCtx::new(
                self,
                path.to_owned(),
                instance,
                "0",
                Some(context),
                Some(inline),
                false,
            );
            let mut statements = emitter.lower_stmt(body)?;
            if emitter.saw_wait {
                return Err("fixed-value function cannot suspend".into());
            }
            if emitter.inline.as_ref().is_some_and(|inline| inline.used) {
                statements.push(IrStmt::Label(done));
            }
            before.extend(statements);
            let dst = emitter
                .cg
                .array_of(destination)
                .ok_or("fixed function result requires an array destination")?
                .ir;
            before.push(IrStmt::FixedArrayCopy {
                dst,
                src: result.ir,
                nba: false,
                slice: 0,
            });
            Ok(IrStmt::Block(before))
        })();
        self.nonflatten_calls.pop();
        self.func = saved.0;
        self.inst = saved.1;
        self.depth_arg = saved.2;
        for (node, previous) in mappings {
            if let Some(previous) = previous {
                self.array_globals.insert(node, previous);
            } else {
                self.array_globals.remove(&node);
            }
        }
        lowered.map(Some)
    }
}
