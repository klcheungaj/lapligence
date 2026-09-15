//! Queries.

use super::*;

impl Codegen<'_> {
    pub(in super::super) fn lower_object_query(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrExpr>, String> {
        let query = match self.kind(node) {
            NodeKind::Expr(ExprKind::Operation { op, operands, .. })
                if *op == Operation::LogicalNot
                    && operands.len() == 1
                    && self.is_mailbox_expr(path, operands[0]) =>
            {
                (
                    IrObjectQuery::MailboxEq(
                        self.lower_mailbox_expr(
                            path,
                            operands[0],
                            self.mailbox_element_for_decl(operands[0]),
                        )?,
                        IrMailboxExpr::Null,
                    ),
                    1,
                    false,
                )
            }
            NodeKind::Expr(ExprKind::Operation { op, operands, .. })
                if *op == Operation::LogicalNot
                    && operands.len() == 1
                    && self.is_chandle_expr(path, operands[0]) =>
            {
                (
                    IrObjectQuery::ChandleEq(
                        self.lower_chandle(path, operands[0])?,
                        IrChandleExpr::Null,
                    ),
                    1,
                    false,
                )
            }
            NodeKind::Expr(ExprKind::BitSelect { base, index })
                if self.is_string_expr(path, *base) =>
            {
                let (base, index) = (*base, *index);
                (
                    IrObjectQuery::StringGetc(
                        self.lower_string(path, base)?,
                        Box::new(self.object_int_argument(path, index, 32)?),
                    ),
                    8,
                    true,
                )
            }
            NodeKind::Expr(ExprKind::Cast { operand, ty, .. })
                if self.is_string_expr(path, *operand) && ty.kind != "string" =>
            {
                let operand = *operand;
                let ty = ty.clone();
                let width = ty
                    .width
                    .ok_or("string cast requires resolved integral target width")?;
                (
                    IrObjectQuery::StringPacked(self.lower_string(path, operand)?),
                    width,
                    ty.signed,
                )
            }
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
                ..
            } if name == "try_get" && self.is_semaphore_expr(path, *receiver) => {
                let receiver = *receiver;
                let args = self.node(node).children.get(1..).unwrap_or_default();
                let keys = match args {
                    [] => lhs_integer_expr(1),
                    [value] => self.semaphore_key_argument(path, *value)?,
                    _ => {
                        return Err(format!(
                            "semaphore try_get takes zero or one key-count argument in `{path}`"
                        ))
                    }
                };
                (
                    IrObjectQuery::SemaphoreTryGet(self.lower_chandle(path, receiver)?, keys),
                    32,
                    true,
                )
            }
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
                ..
            } if name == "status" && self.is_process_expr(path, *receiver) => {
                let receiver = *receiver;
                let args = self.node(node).children.get(1..).unwrap_or_default();
                if !args.is_empty() {
                    return Err(format!("process status takes no arguments in `{path}`"));
                }
                return Ok(Some(object_query(
                    IrObjectQuery::ProcessStatus(self.lower_process(path, receiver)?),
                    32,
                    false,
                )));
            }
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
                ..
            } if self.is_mailbox_expr(path, *receiver) => {
                let name = name.clone();
                let receiver = *receiver;
                let args = self.node(node).children.get(1..).unwrap_or_default();
                let mailbox = self.lower_chandle(path, receiver)?;
                let value = match (name.as_str(), args) {
                    ("num", []) => {
                        return Ok(Some(object_query(
                            IrObjectQuery::MailboxNum(mailbox),
                            32,
                            true,
                        )));
                    }
                    ("try_put", [value]) => IrObjectQuery::MailboxTryPut {
                        mailbox,
                        value: self.lower_mailbox_value(path, *value)?,
                    },
                    ("try_get" | "try_peek", [target]) => IrObjectQuery::MailboxTryGet {
                        mailbox,
                        target: self.lower_mailbox_target(path, *target)?,
                        peek: name == "try_peek",
                    },
                    ("num" | "try_put" | "try_get" | "try_peek", _) => {
                        return Err(format!(
                            "mailbox method `{name}` has the wrong argument count in `{path}`"
                        ));
                    }
                    _ => return Ok(None),
                };
                return Ok(Some(object_query(value, 32, true)));
            }
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
                ..
            } if self.is_string_expr(path, *receiver) => {
                let name = name.clone();
                let receiver = *receiver;
                let args = self.node(node).children[1..].to_vec();
                let value = self.lower_string(path, receiver)?;
                match (name.as_str(), args.as_slice()) {
                    ("len", []) => (IrObjectQuery::StringLen(value), 32, true),
                    ("atoreal", []) => (IrObjectQuery::StringAtoreal(value), REAL_EXPR_WIDTH, true),
                    ("getc", [index]) => (
                        IrObjectQuery::StringGetc(
                            value,
                            Box::new(self.object_int_argument(path, *index, 32)?),
                        ),
                        8,
                        true,
                    ),
                    ("compare" | "icompare", [other]) => (
                        IrObjectQuery::StringCompare(
                            value,
                            self.lower_string(path, *other)?,
                            name == "icompare",
                        ),
                        32,
                        true,
                    ),
                    ("atoi" | "atohex" | "atooct" | "atobin", []) => (
                        IrObjectQuery::StringAtoi(
                            value,
                            match name.as_str() {
                                "atohex" => 16,
                                "atooct" => 8,
                                "atobin" => 2,
                                _ => 10,
                            },
                        ),
                        32,
                        true,
                    ),
                    _ => {
                        return Err(format!(
                            "unsupported string value method or argument count: {name}"
                        ))
                    }
                }
            }
            NodeKind::Expr(ExprKind::Operation { op, operands, .. })
                if operands.len() == 2 && *op != Operation::Inside =>
            {
                let op = *op;
                let (a, b) = (operands[0], operands[1]);
                // `null` is shared by process, semaphore, class, and chandle
                // types. Let the non-null operand select the equality domain.
                let is_process = [a, b].iter().any(|node| {
                    !matches!(
                        self.kind(*node),
                        NodeKind::Expr(ExprKind::Constant {
                            const_type: ConstantType::Null,
                            ..
                        })
                    ) && self.is_process_expr(path, *node)
                });
                if is_process {
                    if !matches!(
                        op,
                        Operation::Equal
                            | Operation::NotEqual
                            | Operation::CaseEqual
                            | Operation::CaseNotEqual
                    ) {
                        return Err("operator is not valid for process handle".to_owned());
                    }
                    let value = object_query(
                        IrObjectQuery::ProcessEq(
                            self.lower_process(path, a)?,
                            self.lower_process(path, b)?,
                        ),
                        1,
                        false,
                    );
                    return Ok(Some(
                        if matches!(op, Operation::NotEqual | Operation::CaseNotEqual) {
                            IrExpr::new(
                                IrExprKind::Un {
                                    op: IrUnOp::LogNot,
                                    a: Box::new(value),
                                },
                                1,
                                false,
                                None,
                            )
                        } else {
                            value
                        },
                    ));
                }
                let is_mailbox = [a, b].iter().any(|node| self.is_mailbox_expr(path, *node));
                if is_mailbox {
                    if matches!(op, Operation::LogicalAnd | Operation::LogicalOr) {
                        let a = self.lower_boolean_expr(path, a)?;
                        let b = self.lower_boolean_expr(path, b)?;
                        return Ok(Some(IrExpr::new(
                            IrExprKind::Bin {
                                op: if op == Operation::LogicalAnd {
                                    IrBinOp::LogAnd
                                } else {
                                    IrBinOp::LogOr
                                },
                                a: Box::new(a),
                                b: Box::new(b),
                            },
                            1,
                            false,
                            None,
                        )));
                    }
                    if !matches!(
                        op,
                        Operation::Equal
                            | Operation::NotEqual
                            | Operation::CaseEqual
                            | Operation::CaseNotEqual
                    ) {
                        return Err("operator is not valid for mailbox handle".to_owned());
                    }
                    let is_null = |node: NodeId| {
                        matches!(
                            self.kind(node),
                            NodeKind::Expr(ExprKind::Constant {
                                const_type: ConstantType::Null,
                                ..
                            })
                        )
                    };
                    if ![a, b]
                        .iter()
                        .all(|node| self.is_mailbox_expr(path, *node) || is_null(*node))
                    {
                        return Err(
                            "mailbox equality requires another mailbox handle or null".to_owned()
                        );
                    }
                    let value = object_query(
                        IrObjectQuery::MailboxEq(
                            self.lower_mailbox_expr(path, a, self.mailbox_element_for_decl(a))?,
                            self.lower_mailbox_expr(path, b, self.mailbox_element_for_decl(b))?,
                        ),
                        1,
                        false,
                    );
                    return Ok(Some(
                        if matches!(op, Operation::NotEqual | Operation::CaseNotEqual) {
                            IrExpr::new(
                                IrExprKind::Un {
                                    op: IrUnOp::LogNot,
                                    a: Box::new(value),
                                },
                                1,
                                false,
                                None,
                            )
                        } else {
                            value
                        },
                    ));
                }
                let is_chandle = [a, b].iter().any(|node| self.is_chandle_expr(path, *node));
                if is_chandle {
                    if matches!(op, Operation::LogicalAnd | Operation::LogicalOr) {
                        let a = self.lower_boolean_expr(path, a)?;
                        let b = self.lower_boolean_expr(path, b)?;
                        return Ok(Some(IrExpr::new(
                            IrExprKind::Bin {
                                op: if op == Operation::LogicalAnd {
                                    IrBinOp::LogAnd
                                } else {
                                    IrBinOp::LogOr
                                },
                                a: Box::new(a),
                                b: Box::new(b),
                            },
                            1,
                            false,
                            None,
                        )));
                    }
                    if !matches!(
                        op,
                        Operation::Equal
                            | Operation::NotEqual
                            | Operation::CaseEqual
                            | Operation::CaseNotEqual
                    ) {
                        return Err("operator is not valid for chandle".to_owned());
                    }
                    let value = object_query(
                        IrObjectQuery::ChandleEq(
                            self.lower_chandle(path, a)?,
                            self.lower_chandle(path, b)?,
                        ),
                        1,
                        false,
                    );
                    return Ok(Some(
                        if matches!(op, Operation::NotEqual | Operation::CaseNotEqual) {
                            IrExpr::new(
                                IrExprKind::Un {
                                    op: IrUnOp::LogNot,
                                    a: Box::new(value),
                                },
                                1,
                                false,
                                None,
                            )
                        } else {
                            value
                        },
                    ));
                }
                if !self.is_string_expr(path, a) && !self.is_string_expr(path, b) {
                    return Ok(None);
                }
                let bin = match op {
                    Operation::Equal => IrBinOp::Eq,
                    Operation::NotEqual => IrBinOp::Neq,
                    Operation::Less => IrBinOp::Lt,
                    Operation::LessEqual => IrBinOp::Le,
                    Operation::Greater => IrBinOp::Gt,
                    Operation::GreaterEqual => IrBinOp::Ge,
                    _ => return Err("operator is not valid for string".to_owned()),
                };
                let compare = object_query(
                    IrObjectQuery::StringCompare(
                        self.lower_string(path, a)?,
                        self.lower_string(path, b)?,
                        false,
                    ),
                    32,
                    true,
                );
                let zero = IrExpr::new(
                    IrExprKind::Const(
                        IrConst::packed(vec![0], vec![], vec![], 32, true, None)
                            .map_err(|e| e.to_string())?,
                    ),
                    32,
                    true,
                    None,
                );
                return Ok(Some(IrExpr::new(
                    IrExprKind::Bin {
                        op: bin,
                        a: Box::new(compare),
                        b: Box::new(zero),
                    },
                    1,
                    false,
                    None,
                )));
            }
            _ => return Ok(None),
        };
        Ok(Some(object_query(query.0, query.1, query.2)))
    }
}
