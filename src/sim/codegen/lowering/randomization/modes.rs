//! `rand_mode` and `constraint_mode` state (IEEE 1800-2009 18.8, 18.9).
//!
//! Each `rand`/`randc` property and each constraint block owns one hidden
//! two-state off bit (0 = active, the default of a new object). Instance
//! members keep it in a hidden class property; static members in one shared
//! static signal (18.8: a static random variable's mode is shared by every
//! instance). A constraint block that overrides an inherited block of the
//! same name shares the inherited block's bit, so a mode set through a base
//! handle addresses the block that is in effect.

use super::*;

impl Codegen<'_> {
    fn new_mode_bit(
        &mut self,
        class_index: usize,
        name: &str,
        is_static: bool,
        fields: &mut Vec<IrClassField>,
    ) -> ModeBit {
        self.new_hidden_state(class_index, name, is_static, 1, fields)
    }

    /// A hidden two-state value of `width` bits per object, or one shared
    /// static signal for a static member; zero in a new object.
    fn new_hidden_state(
        &mut self,
        class_index: usize,
        name: &str,
        is_static: bool,
        width: u32,
        fields: &mut Vec<IrClassField>,
    ) -> ModeBit {
        if is_static {
            let ir = self.model.signals.len();
            let info = SignalInfo {
                global: format!("G_class_{class_index}_{name}"),
                width,
                signed: false,
                two_state: true,
                real: false,
                shortreal: false,
                net_driver: None,
                ir,
            };
            self.model.signals.push(IrSignal {
                fixed_default: None,
                c_name: info.global.clone(),
                hdl_name: None,
                ty: IrType::Packed {
                    width,
                    signed: false,
                    two_state: true,
                },
                net_driver: None,
                net_alias: Vec::new(),
                alias: None,
                omit: false,
            });
            self.signals.push(info.clone());
            return ModeBit::Signal(info);
        }
        let field_index = fields.len();
        fields.push(IrClassField {
            c_name: format!("f_{class_index}_{field_index}_{name}"),
            ty: IrClassFieldType::Packed {
                width,
                signed: false,
                two_state: true,
            },
            container: None,
            native_value: None,
        });
        ModeBit::Field(class_index, field_index)
    }

    /// The topmost class of `class`'s base chain declaring a constraint
    /// block named `name`, with that block.
    fn constraint_root(&self, class: NodeId, name: &str) -> Option<(NodeId, NodeId)> {
        let mut root = None;
        let mut current = Some(class);
        while let Some(node) = current {
            if let Some(block) = self.node(node).children.iter().copied().find(|child| {
                matches!(self.kind(*child), NodeKind::ConstraintBlock(_))
                    && self.node(*child).name == name
            }) {
                root = Some((node, block));
            }
            current = self.db.class_metadata(node).and_then(|meta| meta.base);
        }
        root
    }

    /// Allocate the off bits declared by `class` (called while its own
    /// fields are collected, before base layouts are flattened in).
    pub(in super::super) fn allocate_random_modes(
        &mut self,
        class: NodeId,
        class_index: usize,
        fields: &mut Vec<IrClassField>,
    ) {
        for child in self.node(class).children.clone() {
            match self.kind(child) {
                NodeKind::Var { .. } | NodeKind::Array { .. }
                    if self.db.rand_qualifier(child) != RandQualifier::None =>
                {
                    let name = format!("_llg_rand_off_{}", ident(&self.node(child).name));
                    let is_static = self.db.variable_lifetime(child) == VariableLifetime::Static;
                    let bit = self.new_mode_bit(class_index, &name, is_static, fields);
                    self.rand_mode_bits.insert(child, bit);
                    if self.db.rand_qualifier(child) == RandQualifier::RandC {
                        // The cycle state of a packed randc property of at
                        // most RAND_CYCLIC_MAX_WIDTH bits; other randc
                        // properties are rejected where they are randomized.
                        let width = self
                            .class_static_signals
                            .get(&child)
                            .map(|info| (info.width, info.real))
                            .or_else(|| {
                                let (_, field) = *self.class_fields.get(&child)?;
                                match fields.get(field)?.ty {
                                    IrClassFieldType::Packed { width, .. } => Some((width, false)),
                                    _ => None,
                                }
                            });
                        if let Some((width, false)) = width {
                            if width <= crate::sim::ir::RAND_CYCLIC_MAX_WIDTH {
                                let name = format!("_llg_randc_{}", ident(&self.node(child).name));
                                let state_width = crate::sim::ir::rand_cyclic_state_width(width);
                                let state = self.new_hidden_state(
                                    class_index,
                                    &name,
                                    is_static,
                                    state_width,
                                    fields,
                                );
                                self.randc_states.insert(child, (state, state_width));
                            }
                        }
                    }
                }
                NodeKind::ConstraintBlock(info) => {
                    let is_static = info.is_static;
                    let name = self.node(child).name.clone();
                    if self
                        .constraint_root(class, &name)
                        .is_some_and(|(_, root)| root == child)
                    {
                        let field = format!("_llg_cons_off_{}", ident(&name));
                        let bit = self.new_mode_bit(class_index, &field, is_static, fields);
                        self.constraint_mode_bits.insert(child, bit);
                    }
                }
                _ => {}
            }
        }
    }

    /// Give every overriding constraint block the bit of its root block.
    pub(in super::super) fn share_constraint_modes(&mut self) {
        for class in self.db.classes().to_vec() {
            for child in self.node(class).children.clone() {
                if !matches!(self.kind(child), NodeKind::ConstraintBlock(_))
                    || self.constraint_mode_bits.contains_key(&child)
                {
                    continue;
                }
                let name = self.node(child).name.clone();
                if let Some(bit) = self
                    .constraint_root(class, &name)
                    .and_then(|(_, root)| self.constraint_mode_bits.get(&root).cloned())
                {
                    self.constraint_mode_bits.insert(child, bit);
                }
            }
        }
    }

    /// Hidden off-bit and randc-state fields of class layout `class_index`,
    /// by field index, with their widths.
    pub(in super::super) fn hidden_mode_fields(&self, class_index: usize) -> Vec<(usize, u32)> {
        let mut fields = self
            .rand_mode_bits
            .values()
            .chain(self.constraint_mode_bits.values())
            .map(|bit| (bit, 1))
            .chain(self.randc_states.values().map(|(bit, width)| (bit, *width)))
            .filter_map(|(bit, width)| match bit {
                ModeBit::Field(class, field) if *class == class_index => Some((*field, width)),
                _ => None,
            })
            .collect::<Vec<_>>();
        fields.sort_unstable();
        fields.dedup();
        fields
    }

    /// The off bit's current value through `receiver`.
    pub(super) fn mode_read(
        &mut self,
        bit: &ModeBit,
        receiver: Option<IrChandleExpr>,
    ) -> Result<IrExpr, String> {
        match bit {
            ModeBit::Signal(info) => self.signal_read_expr(info),
            ModeBit::Field(class, field) => {
                let receiver = receiver
                    .ok_or_else(|| "random mode of an object without a handle".to_owned())?;
                let name = self.native_access_symbol(
                    receiver,
                    crate::sim::ir::IrNativeAccessKind::ClassField {
                        class: *class,
                        field: *field,
                    },
                );
                Ok(IrExpr::new(IrExprKind::LocalRead(name), 1, false, None))
            }
        }
    }

    /// Read of a hidden state of `width` bits (a randc cycle state).
    pub(super) fn hidden_state_read(
        &mut self,
        bit: &ModeBit,
        width: u32,
        receiver: Option<IrChandleExpr>,
    ) -> Result<IrExpr, String> {
        let mut read = self.mode_read(bit, receiver)?;
        read.width = width;
        Ok(read)
    }

    pub(super) fn hidden_state_target(
        &mut self,
        bit: &ModeBit,
        width: u32,
        receiver: Option<IrChandleExpr>,
    ) -> Result<IrLhs, String> {
        Ok(match self.mode_target(bit, receiver)? {
            IrLhs::WholeRef {
                addr,
                signed,
                two_state,
                shortreal,
                ..
            } => IrLhs::WholeRef {
                addr,
                width,
                signed,
                two_state,
                shortreal,
            },
            other => other,
        })
    }

    pub(super) fn mode_target(
        &mut self,
        bit: &ModeBit,
        receiver: Option<IrChandleExpr>,
    ) -> Result<IrLhs, String> {
        match bit {
            ModeBit::Signal(info) => Ok(IrLhs::Whole(info.ir)),
            ModeBit::Field(class, field) => {
                let receiver = receiver
                    .ok_or_else(|| "random mode of an object without a handle".to_owned())?;
                let name = self.native_access_symbol(
                    receiver,
                    crate::sim::ir::IrNativeAccessKind::ClassField {
                        class: *class,
                        field: *field,
                    },
                );
                Ok(IrLhs::WholeRef {
                    addr: format!("&{name}"),
                    width: 1,
                    signed: false,
                    two_state: true,
                    shortreal: false,
                })
            }
        }
    }

    /// `rand_mode`/`constraint_mode` as a task or function (18.8, 18.9);
    /// `None` when `node` is another call. A task form returns statements;
    /// a function form (`obj.v.rand_mode()`) a 32-bit `int` value.
    pub(super) fn lower_mode_call(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<ModeCall>, String> {
        let (name, receiver) = match self.kind(node) {
            NodeKind::SysCall { name } => (name.clone(), None),
            NodeKind::MethodCall { name, receiver, .. } => (name.clone(), *receiver),
            _ => return Ok(None),
        };
        let constraint = match name.as_str() {
            "rand_mode" => false,
            "constraint_mode" => true,
            _ => return Ok(None),
        };
        if let NodeKind::MethodCall {
            callee: Some(callee),
            ..
        } = self.kind(node)
        {
            // Slang rejects user methods of these names; a builtin has no
            // source position.
            if self.node(*callee).line != 0 {
                return Ok(None);
            }
        }
        let args = self.call_argument_nodes(node);
        if matches!(self.kind(node), NodeKind::SysCall { .. }) {
            // `[obj.]member.rand_mode(...)`: the first argument names the member.
            let Some((&member, rest)) = args.split_first() else {
                return Err(self.rand_error(node, path, &format!("`{name}` names no member")));
            };
            let target = match self.kind(member) {
                NodeKind::Expr(ExprKind::Ref { target }) => *target,
                NodeKind::Expr(ExprKind::HierPath { refs, .. }) => refs.last().copied().flatten(),
                _ => None,
            };
            let bits = if constraint {
                &self.constraint_mode_bits
            } else {
                &self.rand_mode_bits
            };
            let Some(bit) = target.and_then(|target| bits.get(&target).cloned()) else {
                return Err(self.rand_error(
                    node,
                    path,
                    &format!("`{name}` of this member is not supported (arrays and object members are planned for SIM-030)"),
                ));
            };
            let target = target.expect("mode bit has a declaration");
            let receiver = match bit {
                ModeBit::Signal(_) => None,
                ModeBit::Field(..) => Some(self.class_receiver_for(path, member, target)?),
            };
            return match rest {
                [] => {
                    let off = self.mode_read(&bit, receiver)?;
                    Ok(Some(ModeCall::Value(mode_value(off))))
                }
                [state] => {
                    let lhs = self.mode_target(&bit, receiver)?;
                    let off = self.mode_off_value(path, *state)?;
                    Ok(Some(ModeCall::Statements(vec![IrStmt::Assign {
                        lhs,
                        rhs: off,
                        nba: false,
                    }])))
                }
                _ => Err(self.rand_error(node, path, &format!("`{name}` takes one argument"))),
            };
        }
        // `obj.rand_mode(s)` / `obj.constraint_mode(s)`: every member of
        // the object's dynamic class.
        let [state] = args.as_slice() else {
            return Err(self.rand_error(
                node,
                path,
                &format!("`{name}` of a whole object is a task with one argument (18.8, 18.9)"),
            ));
        };
        let (handle, static_class) = match receiver {
            Some(receiver) => (
                self.lower_chandle(path, receiver)?,
                self.class_node_for_type_expr(receiver),
            ),
            None => (
                self.func
                    .as_ref()
                    .and_then(|function| function.class_receiver.clone())
                    .ok_or_else(|| {
                        self.rand_error(node, path, &format!("`{name}` has no object"))
                    })?,
                self.class_method_owner(node),
            ),
        };
        let static_class = static_class
            .ok_or_else(|| self.rand_error(node, path, "the object's class is not known"))?;
        let off = self.mode_off_value(path, *state)?;
        let local = format!("_llg_mode_{}", node.index());
        let site = self.source_site(&format!("{name}()"), path, node);
        let mut statements = vec![
            IrStmt::Object(Box::new(IrObjectStmt::ChandleDeclareLocal(
                format!("{local}_h"),
                Some(IrChandleExpr::Required {
                    handle: Box::new(handle),
                    site,
                }),
            ))),
            IrStmt::DeclLocal {
                name: format!("{local}_v"),
                width: 1,
                signed: false,
                init: Some(Box::new(off)),
                two_state: true,
            },
        ];
        let object = IrChandleExpr::LocalRead(format!("{local}_h"));
        let value = IrExpr::new(IrExprKind::LocalRead(format!("{local}_v")), 1, false, None);
        let cases = self.class_cases(static_class);
        let mut bodies = Vec::new();
        for class in &cases {
            let mut body = Vec::new();
            for member in self.class_chain(*class) {
                for child in self.node(member).children.clone() {
                    let bit = if constraint {
                        self.constraint_mode_bits.get(&child).cloned()
                    } else {
                        self.rand_mode_bits.get(&child).cloned()
                    };
                    if let Some(bit) = bit {
                        let lhs = self.mode_target(&bit, Some(object.clone()))?;
                        body.push(IrStmt::Assign {
                            lhs,
                            rhs: value.clone(),
                            nba: false,
                        });
                    }
                }
            }
            bodies.push(body);
        }
        statements.extend(self.class_case_chain(&local, &object, &cases, bodies));
        Ok(Some(ModeCall::Statements(vec![IrStmt::Block(statements)])))
    }

    /// The off bit a `rand_mode(s)`/`constraint_mode(s)` argument sets: the
    /// argument converted to `bit` (6.24), inverted.
    fn mode_off_value(&mut self, path: &str, state: NodeId) -> Result<IrExpr, String> {
        let state = self.lower_expr(path, state)?;
        let state = ir_to_storage(state, 1, false, true)?;
        Ok(IrExpr::new(
            IrExprKind::Un {
                op: IrUnOp::BitNeg,
                a: Box::new(state),
            },
            1,
            false,
            None,
        ))
    }
}

/// The `int` a function-form mode query returns: 1 when active.
fn mode_value(off: IrExpr) -> IrExpr {
    let on = IrExpr::new(
        IrExprKind::Un {
            op: IrUnOp::BitNeg,
            a: Box::new(off),
        },
        1,
        false,
        None,
    );
    IrExpr::convert_to(on, 32, true)
}

/// A lowered mode call.
pub(super) enum ModeCall {
    Statements(Vec<IrStmt>),
    Value(IrExpr),
}
