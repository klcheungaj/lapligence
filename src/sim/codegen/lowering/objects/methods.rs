//! Methods.

use super::*;

impl Codegen<'_> {
    /// The built-in random-stream method `node` calls when it is a class's
    /// `srandom`, `get_randstate` or `set_randstate` (SV 18.13.3-18.13.5).
    /// Slang adds these to every class and rejects a member declaring one of
    /// their names (InvalidMethodOverride), so a class method with such a
    /// name is always the built-in.
    pub(in super::super) fn object_random_method(&self, node: NodeId) -> Option<&'static str> {
        let (name, callee) = match self.kind(node) {
            NodeKind::MethodCall {
                name,
                callee: Some(callee),
                ..
            }
            | NodeKind::FuncCall {
                name,
                callee: Some(callee),
                ..
            } => (name.as_str(), *callee),
            _ => return None,
        };
        let method = ["srandom", "get_randstate", "set_randstate"]
            .into_iter()
            .find(|method| *method == name)?;
        self.class_method_owner(callee)?;
        Some(method)
    }

    /// The object whose stream a built-in random method call addresses: its
    /// explicit receiver or the enclosing method's `this`. Marks the model as
    /// keeping object streams.
    fn object_random_target(&mut self, path: &str, node: NodeId) -> Result<IrChandleExpr, String> {
        let target = self
            .class_method_receiver(path, node)?
            .ok_or_else(|| format!("built-in random method without an object in `{path}`"))?;
        self.model.random.threads = true;
        self.model.random.objects = true;
        Ok(target)
    }

    /// Lower `h.srandom(seed)` or `h.set_randstate(state)` (SV 18.13.3,
    /// 18.13.5) on a class object; `None` when `node` is another call.
    pub(in super::super) fn lower_object_random_statement(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrStmt>, String> {
        let Some(method) = self.object_random_method(node) else {
            return Ok(None);
        };
        let args = self.call_argument_nodes(node);
        let op = match (method, args.as_slice()) {
            // `int seed`: real seeds round and X/Z bits read as 0 (6.11.2).
            ("srandom", [seed]) => {
                let seed = self.lower_expr(path, *seed)?;
                IrProcessRandom::Seed(ir_to_storage(seed, 32, false, true)?)
            }
            ("set_randstate", [state]) => {
                IrProcessRandom::SetState(self.lower_string(path, *state)?)
            }
            ("get_randstate", _) => {
                return Err(format!(
                    "`get_randstate` result discarded as a statement in `{path}` is not supported"
                ))
            }
            _ => return Err(format!("{method} requires exactly one argument in {path}")),
        };
        let target = self.object_random_target(path, node)?;
        Ok(Some(IrStmt::Object(Box::new(IrObjectStmt::ObjectRandom {
            target,
            op,
        }))))
    }

    /// Lower `h.get_randstate()` (SV 18.13.4) on a class object; `None` when
    /// `node` is another call.
    pub(in super::super) fn lower_object_random_state(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrStringExpr>, String> {
        if self.object_random_method(node) != Some("get_randstate") {
            return Ok(None);
        }
        if !self.call_argument_nodes(node).is_empty() {
            return Err(format!("get_randstate takes no arguments in {path}"));
        }
        let target = self.object_random_target(path, node)?;
        Ok(Some(IrStringExpr::ObjectRandState(Box::new(target))))
    }

    pub(in super::super) fn lower_object_method(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<IrStmt, String> {
        let (name, receiver) = match self.kind(node) {
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
                ..
            } => (name.clone(), *receiver),
            _ => return Err("object method has no receiver".to_owned()),
        };
        if self.is_process_value(path, receiver)
            && matches!(name.as_str(), "kill" | "suspend" | "resume" | "await")
        {
            let args = self.node(node).children.get(1..).unwrap_or_default();
            if !args.is_empty() {
                return Err(format!(
                    "process method `{name}` takes no arguments in `{path}`"
                ));
            }
            if matches!(name.as_str(), "suspend" | "await")
                && self.func.as_ref().is_some_and(|function| !function.is_task)
            {
                return Err(format!(
                    "process method `{name}` inside a function body in `{path}` is not supported"
                ));
            }
            let target = self.lower_process(path, receiver)?;
            return match name.as_str() {
                "kill" => Ok(IrStmt::Object(Box::new(IrObjectStmt::ProcessControl {
                    op: IrProcessControl::Kill,
                    target,
                }))),
                "suspend" => Ok(IrStmt::Object(Box::new(IrObjectStmt::ProcessControl {
                    op: IrProcessControl::Suspend,
                    target,
                }))),
                "resume" => Ok(IrStmt::Object(Box::new(IrObjectStmt::ProcessControl {
                    op: IrProcessControl::Resume,
                    target,
                }))),
                "await" => Ok(IrStmt::Object(Box::new(IrObjectStmt::ProcessAwait(target)))),
                _ => Err(format!("unsupported process method: {name}")),
            };
        }
        if self.is_process_rng_receiver(receiver) {
            self.model.random.threads = true;
            let args = self.node(node).children.get(1..).unwrap_or_default();
            return match (name.as_str(), args) {
                // `int seed`: real seeds round and X/Z bits read as 0 (6.11.2).
                ("srandom", [seed]) => {
                    let seed = self.lower_expr(path, *seed)?;
                    Ok(IrStmt::RandomSeed {
                        seed: ir_to_storage(seed, 32, false, true)?,
                    })
                }
                ("set_randstate", [state]) => Ok(IrStmt::RandomStateSet {
                    state: self.lower_string(path, *state)?,
                }),
                ("srandom", _) | ("set_randstate", _) => Err(format!(
                    "{} requires exactly one argument in {}",
                    name, path
                )),
                _ => Err(format!("unsupported process random method: {name}")),
            };
        }
        if self.is_process_value(path, receiver)
            && matches!(name.as_str(), "srandom" | "set_randstate")
        {
            // Any other handle seeds or restores the stream of the process it
            // names, which need not be the caller (SV 18.14).
            self.model.random.threads = true;
            let args = self.node(node).children.get(1..).unwrap_or_default();
            let [argument] = args else {
                return Err(format!("{name} requires exactly one argument in {path}"));
            };
            let op = if name == "srandom" {
                let seed = self.lower_expr(path, *argument)?;
                IrProcessRandom::Seed(ir_to_storage(seed, 32, false, true)?)
            } else {
                IrProcessRandom::SetState(self.lower_string(path, *argument)?)
            };
            let target = self.lower_process(path, receiver)?;
            return Ok(IrStmt::Object(Box::new(IrObjectStmt::ProcessRandom {
                target,
                op,
            })));
        }
        if self.is_semaphore_expr(path, receiver) {
            let args = self.node(node).children.get(1..).unwrap_or_default();
            let keys = match args {
                [] => lhs_integer_expr(1),
                [value] => self.semaphore_key_argument(path, *value)?,
                _ => {
                    return Err(format!(
                        "semaphore method `{name}` takes zero or one key-count argument in `{path}`"
                    ))
                }
            };
            let receiver = self.lower_chandle(path, receiver)?;
            return match name.as_str() {
                "put" => Ok(IrStmt::Object(Box::new(IrObjectStmt::SemaphorePut(
                    receiver, keys,
                )))),
                "get" => Ok(IrStmt::Object(Box::new(IrObjectStmt::SemaphoreGet(
                    receiver, keys,
                )))),
                _ => Err(format!("unsupported semaphore method: {name}")),
            };
        }
        let index = self.object_of(path, receiver);
        let local = if index.is_none() {
            let target = match self.kind(receiver) {
                NodeKind::Expr(ExprKind::Ref {
                    target: Some(target),
                }) => Some(*target),
                _ => Some(receiver),
            };
            self.func
                .as_ref()
                .and_then(|function| {
                    target
                        .and_then(|target| function.string_write.get(&target).cloned())
                        .or_else(|| {
                            function
                                .string_write
                                .iter()
                                .find(|(target, _)| {
                                    self.node(**target).name == self.node(receiver).name
                                })
                                .map(|(_, value)| value.clone())
                        })
                })
                .or_else(|| {
                    self.lexical_proc_string_local(receiver)
                        .map(|(_, name)| name.to_owned())
                })
        } else {
            None
        };
        if index.is_none() && local.is_none() {
            let const_ref = self.func.as_ref().is_some_and(|function| {
                let target = match self.kind(receiver) {
                    NodeKind::Expr(ExprKind::Ref {
                        target: Some(target),
                    }) => Some(*target),
                    _ => Some(receiver),
                };
                target.is_some_and(|target| {
                    function.string_read.contains_key(&target)
                        && !function.string_write.contains_key(&target)
                })
            });
            if const_ref {
                return Err("cannot mutate a const-ref string formal".to_owned());
            }
            if let Some(property) = self.foreign_class_container(receiver) {
                return Err(format!(
                    "class container property `{property}` in `{path}` selected through a handle is supported only in procedural statements (SIM-011)"
                ));
            }
            return Err("unsupported object method receiver".to_owned());
        }
        if let Some(index) = index {
            if self.model.objects[index].ty != IrObjectType::String {
                return Err("chandle has no built-in methods".to_owned());
            }
        }
        let args = self.node(node).children[1..].to_vec();
        let operation = match (name.as_str(), args.as_slice()) {
            ("realtoa", [value]) => {
                let value = self.lower_expr(path, *value)?;
                let value = if value.is_real() {
                    value
                } else {
                    IrExpr::new(
                        IrExprKind::CastToReal {
                            a: Box::new(value),
                            shortreal: false,
                        },
                        REAL_EXPR_WIDTH,
                        true,
                        None,
                    )
                };
                match index {
                    Some(index) => IrObjectStmt::StringRealtoa(index, value),
                    None => IrObjectStmt::StringRealtoaLocal(local.clone().unwrap(), value),
                }
            }
            ("putc", [position, value]) => match index {
                Some(index) => IrObjectStmt::StringPutc(
                    index,
                    self.object_int_argument(path, *position, 32)?,
                    self.object_int_argument(path, *value, 8)?,
                ),
                None => IrObjectStmt::StringPutcLocal(
                    local.clone().unwrap(),
                    self.object_int_argument(path, *position, 32)?,
                    self.object_int_argument(path, *value, 8)?,
                ),
            },
            ("itoa" | "hextoa" | "octtoa" | "bintoa", [value]) => {
                let value = ir_to_storage(self.lower_expr(path, *value)?, 32, true, false)?;
                let base = match name.as_str() {
                    "hextoa" => 16,
                    "octtoa" => 8,
                    "bintoa" => 2,
                    _ => 10,
                };
                match index {
                    Some(index) => IrObjectStmt::StringItoa(index, value, base),
                    None => IrObjectStmt::StringItoaLocal(local.clone().unwrap(), value, base),
                }
            }
            _ => {
                return Err(format!(
                    "unsupported string statement method or argument count: {name}"
                ))
            }
        };
        Ok(IrStmt::Object(Box::new(operation)))
    }
}
