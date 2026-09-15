//! Methods.

use super::*;

impl Codegen<'_> {

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
        if self.is_process_expr(path, receiver)
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
                "kill" => Ok(IrStmt::Object(IrObjectStmt::ProcessControl {
                    op: IrProcessControl::Kill,
                    target,
                })),
                "suspend" => Ok(IrStmt::Object(IrObjectStmt::ProcessControl {
                    op: IrProcessControl::Suspend,
                    target,
                })),
                "resume" => Ok(IrStmt::Object(IrObjectStmt::ProcessControl {
                    op: IrProcessControl::Resume,
                    target,
                })),
                "await" => Ok(IrStmt::Object(IrObjectStmt::ProcessAwait(target))),
                _ => Err(format!("unsupported process method: {name}")),
            };
        }
        if self.is_process_rng_receiver(receiver) {
            let args = self.node(node).children.get(1..).unwrap_or_default();
            return match (name.as_str(), args) {
                ("srandom", [seed]) => {
                    let seed = self.lower_expr(path, *seed)?;
                    if seed.is_real() {
                        return Err(format!("srandom seed must be integral in {path}"));
                    }
                    Ok(IrStmt::RandomSeed {
                        seed: IrExpr::convert_to(seed, 32, false),
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
                "put" => Ok(IrStmt::Object(IrObjectStmt::SemaphorePut(receiver, keys))),
                "get" => Ok(IrStmt::Object(IrObjectStmt::SemaphoreGet(receiver, keys))),
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
        Ok(IrStmt::Object(operation))
    }
}
