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
            event: false,
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
                    if let Some(cells) = self.element_mode_cells(child) {
                        let name = format!("_llg_rand_cells_{}", ident(&self.node(child).name));
                        let cells_bit =
                            self.new_hidden_state(class_index, &name, is_static, cells, fields);
                        self.rand_cell_modes.insert(child, (cells_bit, cells));
                    }
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
            .chain(
                self.rand_cell_modes
                    .values()
                    .map(|(bit, width)| (bit, *width)),
            )
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
                NodeKind::Expr(ExprKind::ScopeRef { target }) => Some(*target),
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
                    let lhs = self.mode_target(&bit, receiver.clone())?;
                    let off = self.mode_off_value(path, *state)?;
                    let cells = self.rand_cell_modes.get(&target).cloned();
                    let mut statements = Vec::new();
                    if let Some((cells, width)) = cells {
                        // The whole array sets every element's mode.
                        let lhs = self.hidden_state_target(&cells, width, receiver)?;
                        let zero = packed_const(0, width, false);
                        statements.push(IrStmt::Assign {
                            lhs,
                            rhs: bin_packed(
                                IrBinOp::Sub,
                                zero,
                                IrExpr::resize_to(off.clone(), width, false),
                            ),
                            nba: false,
                        });
                    }
                    statements.push(IrStmt::Assign {
                        lhs,
                        rhs: off,
                        nba: false,
                    });
                    Ok(Some(ModeCall::Statements(statements)))
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
        if let Some(property) = receiver.filter(|_| !constraint) {
            if let Some(selection) = self.handle_property_selection(property) {
                return self
                    .lower_handle_variable_mode(path, node, selection, *state)
                    .map(|statements| Some(ModeCall::Statements(statements)));
            }
        }
        let (handle, static_class) = match receiver {
            Some(receiver) => (
                self.lower_chandle(path, receiver)?,
                self.class_node_for_type_expr(receiver),
            ),
            None => (
                self.method_receiver().ok_or_else(|| {
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

    /// The class handle property a `rand_mode` receiver names, directly or
    /// by one element select: 18.8 "If the random variable is an object
    /// handle, only the mode of the variable is changed".
    fn handle_property_selection(&self, receiver: NodeId) -> Option<HandleSelection> {
        let (base, element) = match self.kind(receiver) {
            NodeKind::Expr(ExprKind::BitSelect { base, .. }) => (*base, true),
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) if indices.len() == 1 => {
                (*base, true)
            }
            _ => (receiver, false),
        };
        let decl = match self.kind(base) {
            NodeKind::Expr(ExprKind::Ref { target }) => (*target)?,
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => refs.last().copied().flatten()?,
            // An element select keeps its declaration as the base.
            NodeKind::Array { .. } => base,
            _ => return None,
        };
        let parent = self.node(decl).parent?;
        self.db.class_metadata(parent)?;
        let handle = if element {
            self.is_handle_array(decl)
        } else {
            self.is_handle_property(decl)
        };
        handle.then_some(HandleSelection {
            receiver,
            decl,
            element,
        })
    }

    /// The hidden off bits one element of `rand` handle array `decl` needs:
    /// its element count, when the design calls `rand_mode` on an element of
    /// a fixed array. Every other design keeps its class layouts.
    fn element_mode_cells(&mut self, decl: NodeId) -> Option<u32> {
        if !self.is_handle_array(decl) {
            return None;
        }
        let selected = match &self.rand_mode_elements {
            Some(selected) => selected,
            None => {
                let selected = (0..self.db.node_count())
                    .map(NodeId::from_index)
                    .filter_map(|node| match self.kind(node) {
                        NodeKind::MethodCall {
                            name,
                            receiver: Some(receiver),
                            ..
                        } if name == "rand_mode" => self.handle_property_selection(*receiver),
                        _ => None,
                    })
                    .filter(|selection| selection.element)
                    .map(|selection| selection.decl)
                    .collect();
                self.rand_mode_elements.insert(selected)
            }
        };
        if !selected.contains(&decl) {
            return None;
        }
        match &self.db.type_descriptor(decl)?.shape {
            TypeShape::FixedArray { dimensions, .. } if dimensions.len() == 1 => {
                let (left, right) = dimensions[0];
                u32::try_from(i64::from(left).abs_diff(i64::from(right)) + 1).ok()
            }
            _ => None,
        }
    }

    /// `rand_mode(s)` of a class handle property or of one element of a
    /// handle array: a variable of the class, so an object it refers to
    /// keeps its modes (18.8). A property that is not `rand` has no mode and
    /// the call has no effect.
    fn lower_handle_variable_mode(
        &mut self,
        path: &str,
        node: NodeId,
        selection: HandleSelection,
        state: NodeId,
    ) -> Result<Vec<IrStmt>, String> {
        let HandleSelection {
            receiver,
            decl,
            element,
        } = selection;
        let Some(bit) = self.rand_mode_bits.get(&decl).cloned() else {
            return Ok(Vec::new());
        };
        let object = match bit {
            ModeBit::Signal(_) => None,
            ModeBit::Field(..) => {
                // A select keeps its declaration as the base; its path
                // expression child names the object.
                let named = element
                    .then(|| {
                        self.node(receiver).children.iter().copied().find(|child| {
                            matches!(
                                self.kind(*child),
                                NodeKind::Expr(
                                    ExprKind::HierPath { .. } | ExprKind::MemberSelect { .. }
                                )
                            )
                        })
                    })
                    .flatten()
                    .unwrap_or(receiver);
                Some(self.class_receiver_for(path, named, decl)?)
            }
        };
        let off = self.mode_off_value(path, state)?;
        if !element {
            let lhs = self.mode_target(&bit, object)?;
            return Ok(vec![IrStmt::Assign {
                lhs,
                rhs: off,
                nba: false,
            }]);
        }
        let Some((cells, width)) = self.rand_cell_modes.get(&decl).cloned() else {
            return Err(self.rand_error(
                node,
                path,
                "`rand_mode` of an element of a dynamic array or queue of class handles is not supported",
            ));
        };
        let IrChandleExpr::ContainerGet { index, .. } = self.lower_chandle(path, receiver)? else {
            return Err(self.rand_error(
                node,
                path,
                "`rand_mode` of this element is not supported",
            ));
        };
        let local = format!("_llg_mode_{}", node.index());
        let (cell, value) = (format!("{local}_c"), format!("{local}_v"));
        let index_width = index.width;
        let index_signed = index.signed;
        let read_cell = || {
            IrExpr::new(
                IrExprKind::LocalRead(cell.clone()),
                index_width,
                index_signed,
                None,
            )
        };
        let read_off = || IrExpr::new(IrExprKind::LocalRead(value.clone()), 1, false, None);
        let at =
            |bit: IrExpr| bin_packed(IrBinOp::Shl, bit, IrExpr::resize_to(read_cell(), 32, false));
        let mask = at(packed_const(1, width, false));
        let kept = bin_packed(
            IrBinOp::BitAnd,
            self.hidden_state_read(&cells, width, object.clone())?,
            bit_not(mask),
        );
        let updated = bin_packed(
            IrBinOp::BitOr,
            kept,
            at(IrExpr::resize_to(read_off(), width, false)),
        );
        // A selector outside the array names no element: no effect.
        let cond = IrExpr::new(
            IrExprKind::Bin {
                op: IrBinOp::Lt,
                a: Box::new(IrExpr::resize_to(read_cell(), 32, false)),
                b: Box::new(packed_const(u64::from(width), 32, false)),
            },
            1,
            false,
            None,
        );
        let base_lhs = self.mode_target(&bit, object.clone())?;
        let base_kept = bin_packed(
            IrBinOp::BitAnd,
            self.mode_read(&bit, object.clone())?,
            read_off(),
        );
        let lhs = self.hidden_state_target(&cells, width, object)?;
        Ok(vec![IrStmt::Block(vec![
            IrStmt::DeclLocal {
                name: cell.clone(),
                width: index_width,
                signed: index_signed,
                init: Some(index),
                two_state: false,
            },
            IrStmt::DeclLocal {
                name: value.clone(),
                width: 1,
                signed: false,
                init: Some(Box::new(off)),
                two_state: true,
            },
            IrStmt::If {
                cond,
                then_: vec![
                    IrStmt::Assign {
                        lhs,
                        rhs: updated,
                        nba: false,
                    },
                    // One active element makes the array active again; the
                    // others keep their own bits.
                    IrStmt::Assign {
                        lhs: base_lhs,
                        rhs: base_kept,
                        nba: false,
                    },
                ],
                els: None,
                check: crate::sim::ir::IrUniquePriorityCheck::None,
            },
        ])])
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

/// A `rand_mode` receiver naming a class handle variable.
struct HandleSelection {
    receiver: NodeId,
    decl: NodeId,
    element: bool,
}

/// An unsigned packed operation at the width of `a`.
fn bin_packed(op: IrBinOp, a: IrExpr, b: IrExpr) -> IrExpr {
    let width = a.width;
    IrExpr::new(
        IrExprKind::Bin {
            op,
            a: Box::new(a),
            b: Box::new(b),
        },
        width,
        false,
        None,
    )
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
