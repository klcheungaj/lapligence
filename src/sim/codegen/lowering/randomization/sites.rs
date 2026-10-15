//! Randomize call sites: `obj.randomize()` (18.6.1) with inline constraints
//! (18.7), variable lists and `null` (18.11), and `std::randomize` (18.12).

use super::problems::{
    has_placeholder, placeholders_in, DistSpec, ProblemBuilder, PLACEHOLDER_PREFIX,
};
use super::*;
use crate::sim::ir::{IrRandOp, IrSysFunc, RAND_STEP_NEED_CALLS};

/// Which variables a class randomize call randomizes (18.11).
pub(super) enum RandSelection {
    /// Every active `rand` property (`rand_mode`).
    Default,
    /// Exactly these properties, whatever their qualifier or mode.
    List(Vec<NodeId>),
    /// `randomize(null)`: none; the call only checks the constraints.
    Checker,
}

/// How generated code reads and writes one candidate variable.
pub(super) struct VarPlan {
    pub(super) index: u32,
    pub(super) width: u32,
    pub(super) signed: bool,
    pub(super) two_state: bool,
    pub(super) current: IrExpr,
    pub(super) lhs: IrLhs,
    pub(super) active: IrExpr,
    /// Storage key a constraint must not read directly (see
    /// `check_direct_reads`).
    pub(super) direct: DirectRead,
    /// The cycle state of a `randc` variable (18.4.2, S30-D3).
    pub(super) cyclic: Option<CyclicPlan>,
}

/// Where a `randc` variable's cycle state lives (a hidden field or signal).
pub(super) struct CyclicPlan {
    pub(super) read: IrExpr,
    pub(super) target: IrLhs,
}

#[derive(PartialEq)]
pub(super) enum DirectRead {
    Field(usize, usize),
    Signal(usize),
    Local(String),
    Other,
}

/// One problem ready to be fed: its builder plus the blocks' enable values.
struct ProblemPlan {
    builder: ProblemBuilder,
    vars: Vec<VarPlan>,
    blocks: Vec<IrExpr>,
}

impl Codegen<'_> {
    /// Lower a randomize call; returns its statements and its `int` result.
    pub(in super::super) fn lower_randomize(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<(Vec<IrStmt>, IrExpr), String> {
        let call = *self
            .db
            .randomize_call(node)
            .ok_or_else(|| "not a randomize call".to_owned())?;
        let result = format!("_llg_rand_{}_r", node.index());
        let declaration = IrStmt::DeclLocal {
            name: result.clone(),
            width: 32,
            signed: true,
            init: Some(Box::new(packed_const(0, 32, true))),
            two_state: true,
        };
        let body = match call.form {
            RandomizeForm::Class => self.lower_class_randomize(path, node, call.inline, &result)?,
            RandomizeForm::Scope => self.lower_scope_randomize(path, node, call.inline, &result)?,
        };
        // The result outlives the block: the statement reads it.
        Ok((
            vec![declaration, IrStmt::Block(body)],
            local_read(&result, 32, true),
        ))
    }

    /// The call's arguments: children other than the receiver and the
    /// inline constraint block.
    fn randomize_arguments(&self, node: NodeId, inline: Option<NodeId>) -> Vec<NodeId> {
        let receiver = match self.kind(node) {
            NodeKind::MethodCall { receiver, .. } => *receiver,
            _ => None,
        };
        self.node(node)
            .children
            .iter()
            .copied()
            .filter(|child| Some(*child) != receiver && Some(*child) != inline)
            .map(|child| match self.kind(child) {
                // std::randomize arguments are output-like assignments.
                NodeKind::Expr(ExprKind::Operation {
                    op: Operation::Assignment,
                    operands,
                    ..
                }) if operands.len() == 2
                    && matches!(self.kind(operands[1]), NodeKind::Expr(ExprKind::Other)) =>
                {
                    operands[0]
                }
                _ => child,
            })
            .collect()
    }

    fn lower_class_randomize(
        &mut self,
        path: &str,
        node: NodeId,
        inline: Option<NodeId>,
        result: &str,
    ) -> Result<Vec<IrStmt>, String> {
        let receiver = match self.kind(node) {
            NodeKind::MethodCall { receiver, .. } => *receiver,
            _ => None,
        };
        if let Some(receiver) = receiver {
            if matches!(
                self.kind(receiver),
                NodeKind::Expr(ExprKind::NewClass {
                    is_super_class: true,
                    ..
                })
            ) || self.node(receiver).name == "super"
            {
                return Err(self.rand_error(node, path, "`super.randomize()` is not supported"));
            }
        }
        let caller_receiver = self
            .func
            .as_ref()
            .and_then(|function| function.class_receiver.clone());
        let (handle, static_class) = match receiver {
            Some(receiver) => (
                self.lower_chandle(path, receiver)?,
                self.class_node_for_type_expr(receiver),
            ),
            None => (
                caller_receiver.clone().ok_or_else(|| {
                    self.rand_error(
                        node,
                        path,
                        "randomize() outside a class method has no object",
                    )
                })?,
                self.class_method_owner(node),
            ),
        };
        let static_class = static_class.ok_or_else(|| {
            self.rand_error(node, path, "the randomized object's class is not known")
        })?;
        let selection = match self.randomize_arguments(node, inline).as_slice() {
            [] => RandSelection::Default,
            [only]
                if matches!(
                    self.kind(*only),
                    NodeKind::Expr(ExprKind::Constant {
                        const_type: ConstantType::Null,
                        ..
                    })
                ) =>
            {
                RandSelection::Checker
            }
            arguments => {
                let mut declarations = Vec::new();
                for argument in arguments {
                    let target = match self.kind(*argument) {
                        NodeKind::Expr(ExprKind::Ref {
                            target: Some(target),
                        }) => Some(*target),
                        NodeKind::Expr(ExprKind::HierPath { refs, .. }) => match refs.as_slice() {
                            [Some(this), Some(target)] if self.node(*this).name == "this" => {
                                Some(*target)
                            }
                            _ => None,
                        },
                        _ => None,
                    };
                    let Some(target) = target else {
                        return Err(self.rand_error(
                            *argument,
                            path,
                            "a randomize() argument must name a property of the object (selects and members are planned for SIM-030)",
                        ));
                    };
                    declarations.push(target);
                }
                RandSelection::List(declarations)
            }
        };
        // Inline constraints of a call on another object of the caller's
        // own class hierarchy could name the caller's properties through
        // `local::`, which the captured names do not distinguish.
        if inline.is_some() && receiver.is_some() && caller_receiver.is_some() {
            if let Some(caller_class) = self.class_method_owner(node) {
                let overlap = self.class_chain(caller_class).contains(&static_class)
                    || self.class_chain(static_class).contains(&caller_class);
                if overlap {
                    return Err(self.rand_error(
                        node,
                        path,
                        "inline constraints on another object of the calling method's class hierarchy are not supported (`local::` resolution)",
                    ));
                }
            }
        }
        self.model.random.threads = true;
        self.model.random.objects = true;
        let local = format!("_llg_rand_{}", node.index());
        let site = self.source_site("randomize()", path, node);
        let object_name = format!("{local}_h");
        let object = IrChandleExpr::LocalRead(object_name.clone());
        let mut statements = vec![IrStmt::Object(Box::new(IrObjectStmt::ChandleDeclareLocal(
            object_name,
            Some(IrChandleExpr::Required {
                handle: Box::new(handle),
                site: site.clone(),
            }),
        )))];
        let cases = self.class_cases(static_class);
        let mut bodies = Vec::new();
        for class in &cases {
            bodies.push(self.class_randomize_case(
                path, node, *class, &object, &selection, inline, result, &site,
            )?);
        }
        statements.extend(self.class_case_chain(&local, &object, &cases, bodies));
        Ok(statements)
    }

    /// The user-defined `pre_randomize`/`post_randomize` in effect for an
    /// object of `class` (the most derived definition, 18.6.2).
    pub(super) fn randomize_hook(&self, class: NodeId, name: &str) -> Option<NodeId> {
        self.class_chain(class).into_iter().find_map(|member| {
            self.class_method_nodes(member).into_iter().find(|method| {
                // Slang's implicit hooks have a body but no source position.
                self.node(*method).name == name
                    && self.node(*method).line != 0
                    && self.db.subroutine_body(*method).is_some()
            })
        })
    }

    pub(super) fn hook_call(
        &mut self,
        path: &str,
        node: NodeId,
        hook: NodeId,
        object: &IrChandleExpr,
    ) -> Result<IrStmt, String> {
        let f = self
            .func_meta
            .get(&hook)
            .map(|meta| meta.ir)
            .ok_or_else(|| {
                self.rand_error(
                    node,
                    path,
                    &format!("`{}` has no lowered body", self.node(hook).name),
                )
            })?;
        if !self.model.funcs[f].formals.is_empty() {
            return Err(self.rand_error(
                node,
                path,
                &format!("`{}` must take no arguments (18.6.2)", self.node(hook).name),
            ));
        }
        Ok(IrStmt::Call(Box::new(IrCall {
            f,
            args: Vec::new(),
            depth: parse_depth(&self.depth_arg),
            receiver: Some(object.clone()),
            virtual_dispatch: false,
            virtual_call: None,
            temps: Vec::new(),
            copyouts: Vec::new(),
            dpi_site: None,
        })))
    }

    #[allow(clippy::too_many_arguments)]
    fn class_randomize_case(
        &mut self,
        path: &str,
        node: NodeId,
        class: NodeId,
        object: &IrChandleExpr,
        selection: &RandSelection,
        inline: Option<NodeId>,
        result: &str,
        site: &str,
    ) -> Result<Vec<IrStmt>, String> {
        let chain = self.class_chain(class);
        let class_name = self.node(class).name.clone();
        let mut builder = ProblemBuilder::new(format!("{site} ({class_name})"));
        let mut vars = Vec::new();
        let mut placeholders = HashMap::new();
        let mut listed = match selection {
            RandSelection::List(declarations) => declarations.clone(),
            _ => Vec::new(),
        };
        let effective = self.effective_blocks(&chain);
        // Arrays, objects reached through `rand` handles and loops make the
        // problem a template with units (templates.rs); other problems keep
        // the flat form.
        if self.class_needs_units(&chain, &effective, inline, selection) {
            let tag = format!(
                "_llg_rand_{}_{}",
                node.index(),
                self.class_nodes.get(&class).copied().unwrap_or(0)
            );
            builder.units =
                Some(self.unit_state(tag, Some(class), Some(object.clone()), selection));
        }
        for member in chain.iter().rev().copied() {
            for child in self.node(member).children.clone() {
                // Any property kind (scalars, arrays, handles) may be random.
                if !matches!(
                    self.kind(child),
                    NodeKind::Var { .. } | NodeKind::Array { .. }
                ) {
                    continue;
                }
                let qualifier = self.db.rand_qualifier(child);
                let in_list = listed.contains(&child);
                listed.retain(|declaration| *declaration != child);
                if qualifier == RandQualifier::None && !in_list {
                    continue;
                }
                let randomized = match selection {
                    RandSelection::Default => true,
                    RandSelection::List(_) => in_list,
                    RandSelection::Checker => false,
                };
                let name = self.node(child).name.clone();
                if builder.units.is_some()
                    && self.root_unit_property(path, node, &mut builder, child, randomized)?
                {
                    continue;
                }
                if qualifier == RandQualifier::RandC
                    && randomized
                    && !self.randc_states.contains_key(&child)
                {
                    return Err(self.rand_error(
                        node,
                        path,
                        &format!(
                            "`randc` property `{name}` of class `{}` must be a packed integral value of at most {} bits (18.4.2 permits a limit; S30-D3)",
                            self.node(member).name,
                            crate::sim::ir::RAND_CYCLIC_MAX_WIDTH
                        ),
                    ));
                }
                let Some(plan) =
                    self.class_var_plan(&mut builder, child, object, selection, in_list)?
                else {
                    if randomized {
                        return Err(self.rand_error(
                            node,
                            path,
                            &format!("random property `{name}` of class `{}` is not a packed integral value, a one-dimensional array of packed integral elements or a class handle, or is a structure with enum or tagged-union members", self.node(member).name),
                        ));
                    }
                    continue;
                };
                if plan.0.cyclic.is_some() {
                    builder.set_cyclic(plan.0.index);
                }
                placeholders.insert(child, plan.1);
                vars.push(plan.0);
            }
        }
        if let Some(missing) = listed.first() {
            return Err(self.rand_error(
                node,
                path,
                &format!(
                    "randomize() argument `{}` is not a property of class `{class_name}`",
                    self.node(*missing).name
                ),
            ));
        }
        let classes = chain
            .iter()
            .filter_map(|member| self.class_nodes.get(member).copied())
            .collect::<HashSet<_>>();
        let saved = self.rand_lowering.replace(RandLowering::new(
            classes,
            Some(object.clone()),
            placeholders,
        ));
        let lowered =
            self.lower_class_blocks(path, &mut builder, &vars, object, &effective, inline);
        self.rand_lowering = saved;
        let blocks = lowered?;
        if builder.units.is_some() {
            // Instance variables of the object are unit 0's; static ones
            // are global.
            let own = vars
                .iter()
                .filter(|var| !matches!(var.direct, DirectRead::Signal(_)))
                .map(|var| var.index)
                .collect::<Vec<_>>();
            self.lower_units(path, node, &mut builder)?;
            self.finish_units(&mut builder, own);
        }
        let pre = self.randomize_hook(class, "pre_randomize");
        let post = self.randomize_hook(class, "post_randomize");
        let pre = pre
            .map(|hook| self.hook_call(path, node, hook, object))
            .transpose()?;
        let post = post
            .map(|hook| self.hook_call(path, node, hook, object))
            .transpose()?;
        let plan = ProblemPlan {
            builder,
            vars,
            blocks,
        };
        let mut statements = Vec::new();
        statements.extend(pre);
        statements.extend(self.feed_problem(
            node,
            plan,
            Some(object.clone()),
            result,
            site,
            post.into_iter().collect(),
        )?);
        Ok(statements)
    }

    /// Effective constraint blocks of an object whose class has `chain`
    /// (most derived first): a derived block replaces an inherited block of
    /// the same name (18.5.2). Root-first order keeps block numbering
    /// stable across classes.
    pub(super) fn effective_blocks(
        &self,
        chain: &[NodeId],
    ) -> Vec<(NodeId, crate::core::db::ConstraintBlockInfo)> {
        let mut seen = HashSet::new();
        let mut effective = Vec::new();
        for member in chain {
            for child in self.node(*member).children.clone() {
                if let NodeKind::ConstraintBlock(info) = self.kind(child) {
                    if seen.insert(self.node(child).name.clone()) {
                        effective.push((child, *info));
                    }
                }
            }
        }
        effective.reverse();
        effective
    }

    /// The candidate-variable plan of class property `child`, or `None`
    /// when it is not packed integral storage.
    pub(super) fn class_var_plan(
        &mut self,
        builder: &mut ProblemBuilder,
        child: NodeId,
        object: &IrChandleExpr,
        selection: &RandSelection,
        in_list: bool,
    ) -> Result<Option<(VarPlan, IrExpr)>, String> {
        if self.db.array_meta(child).is_some()
            || self.class_native_fields.contains_key(&child)
            || self.random_aggregate_unsupported(child)
            || matches!(self.kind(child), NodeKind::Var { ty } if is_handle_kind(&ty.kind) || ty.kind == "string" || is_real_kind(&ty.kind))
        {
            return Ok(None);
        }
        let (width, signed, two_state, current, lhs, direct) =
            if let Some(info) = self.class_static_signals.get(&child).cloned() {
                if info.real {
                    return Ok(None);
                }
                (
                    info.width,
                    info.signed,
                    info.two_state,
                    self.signal_read_expr(&info)?,
                    IrLhs::Whole(info.ir),
                    DirectRead::Signal(info.ir),
                )
            } else if let Some((class, field)) = self.class_fields.get(&child).copied() {
                let IrClassFieldType::Packed {
                    width,
                    signed,
                    two_state,
                } = self.model.classes[class].fields[field].ty
                else {
                    return Ok(None);
                };
                let read = self.native_access_symbol(
                    object.clone(),
                    crate::sim::ir::IrNativeAccessKind::ClassField { class, field },
                );
                let write = self.native_access_symbol(
                    object.clone(),
                    crate::sim::ir::IrNativeAccessKind::ClassField { class, field },
                );
                (
                    width,
                    signed,
                    two_state,
                    local_read(&read, width, signed),
                    IrLhs::WholeRef {
                        addr: format!("&{write}"),
                        width,
                        signed,
                        two_state,
                        shortreal: false,
                    },
                    DirectRead::Field(class, field),
                )
            } else {
                return Ok(None);
            };
        let active = match selection {
            RandSelection::Default => match self.rand_mode_bits.get(&child).cloned() {
                Some(bit) => bit_not(self.mode_read(&bit, Some(object.clone()))?),
                None => packed_const(0, 1, false),
            },
            RandSelection::List(_) => packed_const(u64::from(in_list), 1, false),
            RandSelection::Checker => packed_const(0, 1, false),
        };
        let cyclic = match self.randc_states.get(&child).cloned() {
            Some((state, state_width)) => {
                let receiver = Some(object.clone());
                Some(CyclicPlan {
                    read: self.hidden_state_read(&state, state_width, receiver.clone())?,
                    target: self.hidden_state_target(&state, state_width, receiver)?,
                })
            }
            None => None,
        };
        let name = self.node(child).name.clone();
        // Problems with units share one variable per property.
        if let Some(index) = builder.decl_vars.get(&child).copied() {
            return Ok(Some((
                VarPlan {
                    index,
                    width,
                    signed,
                    two_state,
                    current,
                    lhs,
                    active,
                    direct,
                    cyclic,
                },
                local_read(&format!("{PLACEHOLDER_PREFIX}{index}"), width, signed),
            )));
        }
        let (index, placeholder) = builder.add_var(name, width, signed);
        builder.decl_vars.insert(child, index);
        if let Some(metadata) = self.enum_metadata_for_expr(child).cloned() {
            let values = metadata
                .members
                .iter()
                .map(|member| match &member.value {
                    Val::Bits(value) => val_to_const(value),
                    _ => Err("enum member has a non-integral value".to_owned()),
                })
                .collect::<Result<Vec<_>, _>>()?;
            builder.set_domain(index, &values)?;
        }
        Ok(Some((
            VarPlan {
                index,
                width,
                signed,
                two_state,
                current,
                lhs,
                active,
                direct,
                cyclic,
            },
            local_read(&placeholder, width, signed),
        )))
    }

    pub(super) fn lower_class_blocks(
        &mut self,
        path: &str,
        builder: &mut ProblemBuilder,
        vars: &[VarPlan],
        object: &IrChandleExpr,
        effective: &[(NodeId, crate::core::db::ConstraintBlockInfo)],
        inline: Option<NodeId>,
    ) -> Result<Vec<IrExpr>, String> {
        let mut blocks = Vec::new();
        for (block, info) in effective {
            if info.is_pure {
                continue;
            }
            let root = match info.body {
                Some(body) => self.constraint_item(path, builder, vars, object, body, None)?,
                // A prototype without a body is an empty constraint (18.5.1).
                None => builder.constant_u64(1, 1, false),
            };
            let root = builder.truth(root);
            builder.add_block(self.node(*block).name.clone(), root);
            let enabled = match self.constraint_mode_bits.get(block).cloned() {
                Some(bit) => bit_not(self.mode_read(&bit, Some(object.clone()))?),
                None => packed_const(1, 1, false),
            };
            blocks.push(enabled);
        }
        if let Some(inline) = inline {
            let root = self.constraint_item(path, builder, vars, object, inline, None)?;
            let root = builder.truth(root);
            builder.add_block("with".to_owned(), root);
            blocks.push(packed_const(1, 1, false));
        }
        Ok(blocks)
    }

    /// One constraint item (18.5) as a solver node. `guard` is the
    /// conjunction of the enclosing implication and `if` conditions, which
    /// scopes the weights of a `dist` (S30-D1).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn constraint_item(
        &mut self,
        path: &str,
        builder: &mut ProblemBuilder,
        vars: &[VarPlan],
        object: &IrChandleExpr,
        item: NodeId,
        guard: Option<u32>,
    ) -> Result<u32, String> {
        let kind = match self.kind(item) {
            NodeKind::Constraint(kind) => kind.clone(),
            _ => return Err(self.rand_error(item, path, "unsupported constraint item")),
        };
        let within = |builder: &mut ProblemBuilder, condition: u32| match guard {
            Some(guard) => builder.logical(IrRandOp::LogAnd, guard, condition),
            None => builder.truth(condition),
        };
        match kind {
            ConstraintKind::List { items } => {
                let mut nodes = Vec::with_capacity(items.len());
                for item in items {
                    let node = self.constraint_item(path, builder, vars, object, item, guard)?;
                    nodes.push(builder.truth(node));
                }
                Ok(builder.all(&nodes))
            }
            ConstraintKind::Expr { expr, soft } => {
                if soft {
                    return Err(self.rand_error(
                        item,
                        path,
                        "`soft` constraints are not part of IEEE 1800-2009",
                    ));
                }
                if let NodeKind::Expr(ExprKind::Dist { .. }) = self.kind(expr) {
                    return self.constraint_dist(path, builder, vars, object, expr, guard);
                }
                self.constraint_expr(path, builder, vars, object, expr)
            }
            ConstraintKind::Implication { predicate, body } => {
                let predicate = self.constraint_expr(path, builder, vars, object, predicate)?;
                let inner = within(builder, predicate);
                let body = self.constraint_item(path, builder, vars, object, body, Some(inner))?;
                Ok(builder.logical(IrRandOp::Implies, predicate, body))
            }
            ConstraintKind::Conditional {
                predicate,
                then_body,
                else_body,
            } => {
                let predicate = self.constraint_expr(path, builder, vars, object, predicate)?;
                let inner = within(builder, predicate);
                let then_body =
                    self.constraint_item(path, builder, vars, object, then_body, Some(inner))?;
                let then_node = builder.logical(IrRandOp::Implies, predicate, then_body);
                match else_body {
                    None => Ok(then_node),
                    Some(else_body) => {
                        let negated = builder.not(predicate);
                        let inner = within(builder, negated);
                        let else_body = self.constraint_item(
                            path,
                            builder,
                            vars,
                            object,
                            else_body,
                            Some(inner),
                        )?;
                        let else_node = builder.logical(IrRandOp::Implies, negated, else_body);
                        Ok(builder.logical(IrRandOp::LogAnd, then_node, else_node))
                    }
                }
            }
            ConstraintKind::SolveBefore { solve, before } => {
                let solve = self.solve_before_vars(path, builder, &solve)?;
                let after = self.solve_before_vars(path, builder, &before)?;
                for first in &solve {
                    for second in &after {
                        builder.add_order(*first, *second);
                    }
                }
                // An ordering is not a value constraint (18.5.9).
                Ok(builder.constant_u64(1, 1, false))
            }
            ConstraintKind::Foreach {
                array,
                vars: loop_vars,
                body,
            } => self.constraint_foreach(
                path, builder, vars, object, item, array, &loop_vars, body, guard,
            ),
            ConstraintKind::Uniqueness { .. } | ConstraintKind::DisableSoft { .. } => {
                Err(self.rand_error(item, path, "this constraint is not part of IEEE 1800-2009"))
            }
        }
    }

    /// The random variables a `solve...before` list names (18.5.9: rand
    /// variables only, never randc).
    fn solve_before_vars(
        &mut self,
        path: &str,
        builder: &ProblemBuilder,
        primaries: &[NodeId],
    ) -> Result<Vec<u32>, String> {
        let mut vars = Vec::with_capacity(primaries.len());
        for primary in primaries {
            let lowered = self
                .lower_expr(path, *primary)
                .map_err(|error| self.rand_error(*primary, path, &error))?;
            let Some(var) = builder.placeholder_var(&lowered) else {
                return Err(self.rand_error(
                    *primary,
                    path,
                    "a `solve...before` list may name only random variables (18.5.9)",
                ));
            };
            if builder.is_cyclic(var) {
                return Err(self.rand_error(
                    *primary,
                    path,
                    "`randc` variables are not allowed in `solve...before` (18.5.9)",
                ));
            }
            vars.push(var);
        }
        Ok(vars)
    }

    /// `expression dist { ... }` (18.5.4).
    fn constraint_dist(
        &mut self,
        path: &str,
        builder: &mut ProblemBuilder,
        vars: &[VarPlan],
        object: &IrChandleExpr,
        expr: NodeId,
        guard: Option<u32>,
    ) -> Result<u32, String> {
        let (left, items, default_weight) = match self.kind(expr) {
            NodeKind::Expr(ExprKind::Dist {
                left,
                items,
                default_weight,
            }) => (*left, items.clone(), *default_weight),
            _ => return Err(self.rand_error(expr, path, "not a dist expression")),
        };
        if default_weight {
            return Err(self.rand_error(
                expr,
                path,
                "a `default` dist item is not part of IEEE 1800-2009",
            ));
        }
        self.template_forms(path, builder, left)?;
        let value = self
            .lower_expr(path, left)
            .map_err(|error| self.rand_error(left, path, &format!("in a constraint: {error}")))?;
        self.check_template_uses(path, expr, &value, builder)?;
        self.check_direct_reads(path, left, &value, vars, object)?;
        let names = placeholders_in(&value);
        if names.is_empty() {
            return Err(self.rand_error(
                left,
                path,
                "a dist expression must contain a random variable (18.5.4)",
            ));
        }
        if names.iter().any(|name| {
            builder
                .placeholder_var(&local_read(name, 1, false))
                .is_some_and(|var| builder.is_cyclic(var))
        }) {
            return Err(self.rand_error(
                left,
                path,
                "a dist shall not be applied to `randc` variables (18.5.4)",
            ));
        }
        let mut specs = Vec::with_capacity(items.len());
        for item in &items {
            let mut lowered = self
                .lower_inside_items(path, std::slice::from_ref(&item.value))
                .map_err(|error| self.rand_error(item.value, path, &error))?;
            if lowered.len() != 1 {
                return Err(self.rand_error(
                    item.value,
                    path,
                    "a dist item must be a value or a range (18.5.4)",
                ));
            }
            let weight = item
                .weight
                .map(|weight| {
                    self.lower_expr(path, weight)
                        .map_err(|error| self.rand_error(weight, path, &error))
                })
                .transpose()?;
            specs.push(DistSpec {
                item: lowered.remove(0),
                weight,
                per_range: item.per_range,
            });
        }
        builder.item = self.source_site("in the constraint", path, expr);
        builder
            .dist(&value, &specs, guard)
            .map_err(|error| self.rand_error(expr, path, &error))
    }

    fn constraint_expr(
        &mut self,
        path: &str,
        builder: &mut ProblemBuilder,
        vars: &[VarPlan],
        object: &IrChandleExpr,
        expr: NodeId,
    ) -> Result<u32, String> {
        match self.kind(expr) {
            NodeKind::Expr(ExprKind::Other) => {
                return Err(self.rand_error(expr, path, "unsupported constraint expression"))
            }
            NodeKind::Expr(ExprKind::Dist { .. }) => {
                return Err(self.rand_error(
                    expr,
                    path,
                    "a dist expression may appear only as a constraint (18.5.4)",
                ))
            }
            _ => {}
        }
        self.template_forms(path, builder, expr)?;
        let lowered = self
            .lower_expr(path, expr)
            .map_err(|error| self.rand_error(expr, path, &format!("in a constraint: {error}")))?;
        self.check_template_uses(path, expr, &lowered, builder)?;
        self.check_direct_reads(path, expr, &lowered, vars, object)?;
        // Run-time diagnostics name the constraint like other run-time
        // sites: instance path, line and column.
        builder.item = self.source_site("in the constraint", path, expr);
        builder
            .expr(&lowered)
            .map_err(|error| self.rand_error(expr, path, &error))
    }

    /// A constraint form that read a random variable's storage instead of
    /// its placeholder would silently treat it as state (A03): reject it.
    fn check_direct_reads(
        &self,
        path: &str,
        node: NodeId,
        lowered: &IrExpr,
        vars: &[VarPlan],
        object: &IrChandleExpr,
    ) -> Result<(), String> {
        let mut copy = lowered.clone();
        let mut direct = None;
        crate::sim::opt::walk_expr_mut(&mut copy, &mut |expr| {
            let key = match &expr.kind {
                IrExprKind::SigRead(signal) => DirectRead::Signal(*signal),
                IrExprKind::LocalRead(name) if !name.starts_with(PLACEHOLDER_PREFIX) => {
                    match name
                        .strip_prefix("_llg_access_")
                        .and_then(|index| index.parse::<usize>().ok())
                        .and_then(|index| self.model.native_accesses.get(index))
                    {
                        Some(access) => match access.kind {
                            crate::sim::ir::IrNativeAccessKind::ClassField { class, field }
                                if access.receiver == *object =>
                            {
                                DirectRead::Field(class, field)
                            }
                            _ => DirectRead::Other,
                        },
                        None => DirectRead::Local(name.clone()),
                    }
                }
                _ => DirectRead::Other,
            };
            if key != DirectRead::Other && vars.iter().any(|var| var.direct == key) {
                direct = Some(());
            }
        });
        match direct {
            Some(()) => Err(self.rand_error(
                node,
                path,
                "this use of a random variable is not supported in a constraint",
            )),
            None => Ok(()),
        }
    }

    /// `std::randomize(v...) [with {...}]` (18.12): the arguments are the
    /// random variables; the calling thread's stream supplies the draws.
    fn lower_scope_randomize(
        &mut self,
        path: &str,
        node: NodeId,
        inline: Option<NodeId>,
        result: &str,
    ) -> Result<Vec<IrStmt>, String> {
        self.model.random.threads = true;
        let site = self.source_site("std::randomize()", path, node);
        let mut builder = ProblemBuilder::new(site.clone());
        let mut vars = Vec::new();
        let mut placeholders = HashMap::new();
        let mut arguments = Vec::new();
        for argument in self.randomize_arguments(node, inline) {
            let target = match self.kind(argument) {
                NodeKind::Expr(ExprKind::Ref {
                    target: Some(target),
                }) => *target,
                _ => {
                    return Err(self.rand_error(
                        argument,
                        path,
                        "a std::randomize() argument must be a variable name (a select or member is not supported)",
                    ))
                }
            };
            if arguments.iter().any(|(_, named)| *named == target) {
                return Err(self.rand_error(
                    argument,
                    path,
                    "a std::randomize() argument is named twice",
                ));
            }
            arguments.push((argument, target));
        }
        let targets = arguments
            .iter()
            .map(|(_, target)| *target)
            .collect::<Vec<_>>();
        let randoms = targets.iter().copied().collect::<HashSet<_>>();
        if targets
            .iter()
            .any(|target| self.db.array_meta(*target).is_some())
            || inline.is_some_and(|inline| self.constraint_needs_units(inline, &randoms))
        {
            builder.units = Some(self.unit_state(
                format!("_llg_rand_{}_s", node.index()),
                None,
                None,
                &RandSelection::List(targets.clone()),
            ));
        }
        for (argument, target) in arguments {
            if self.db.array_meta(target).is_some() && builder.units.is_some() {
                self.scope_unit_array(path, &mut builder, argument, target)?;
                continue;
            }
            if self.random_aggregate_unsupported(target) {
                return Err(self.rand_error(
                    argument,
                    path,
                    "std::randomize() of a structure with enum or tagged-union members is not supported",
                ));
            }
            let current = self
                .lower_expr(path, argument)
                .map_err(|error| self.rand_error(argument, path, &error))?;
            if current.is_real() || current.width == 0 || current.fill.is_some() {
                return Err(self.rand_error(
                    argument,
                    path,
                    "a std::randomize() argument must be a packed integral variable",
                ));
            }
            let lhs = self
                .lower_lhs(path, argument)
                .map_err(|error| self.rand_error(argument, path, &error))?;
            let (width, signed) = (current.width, current.signed);
            let two_state = self.db.is_two_state_type(target)
                || matches!(self.kind(target), NodeKind::Var { ty } if is_two_state_kind(&ty.kind));
            let direct = match &current.kind {
                IrExprKind::SigRead(signal) => DirectRead::Signal(*signal),
                IrExprKind::LocalRead(name) => match name
                    .strip_prefix("_llg_access_")
                    .and_then(|index| index.parse::<usize>().ok())
                    .and_then(|index| self.model.native_accesses.get(index))
                {
                    Some(access) => match access.kind {
                        crate::sim::ir::IrNativeAccessKind::ClassField { class, field } => {
                            DirectRead::Field(class, field)
                        }
                        _ => DirectRead::Other,
                    },
                    None => DirectRead::Local(name.clone()),
                },
                _ => DirectRead::Other,
            };
            let (index, placeholder) =
                builder.add_var(self.node(target).name.clone(), width, signed);
            if let Some(metadata) = self.enum_metadata_for_expr(target).cloned() {
                let values = metadata
                    .members
                    .iter()
                    .map(|member| match &member.value {
                        Val::Bits(value) => val_to_const(value),
                        _ => Err("enum member has a non-integral value".to_owned()),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                builder.set_domain(index, &values)?;
            }
            placeholders.insert(target, local_read(&placeholder, width, signed));
            vars.push(VarPlan {
                index,
                width,
                signed,
                two_state,
                current,
                lhs,
                active: packed_const(1, 1, false),
                direct,
                cyclic: None,
            });
        }
        let mut blocks = Vec::new();
        if let Some(inline) = inline {
            let receiver = self
                .func
                .as_ref()
                .and_then(|function| function.class_receiver.clone())
                .unwrap_or(IrChandleExpr::Null);
            let saved =
                self.rand_lowering
                    .replace(RandLowering::new(HashSet::new(), None, placeholders));
            let lowered = self
                .constraint_item(path, &mut builder, &vars, &receiver, inline, None)
                .map(|root| builder.truth(root));
            self.rand_lowering = saved;
            let root = lowered?;
            builder.add_block("with".to_owned(), root);
            blocks.push(packed_const(1, 1, false));
        }
        if builder.units.is_some() {
            // std::randomize arguments are global variables.
            self.lower_units(path, node, &mut builder)?;
            self.finish_units(&mut builder, Vec::new());
        }
        let plan = ProblemPlan {
            builder,
            vars,
            blocks,
        };
        self.feed_problem(node, plan, None, result, &site, Vec::new())
    }

    /// Feed, solve and commit one problem; `result` receives the outcome.
    fn feed_problem(
        &mut self,
        node: NodeId,
        plan: ProblemPlan,
        stream: Option<IrChandleExpr>,
        result: &str,
        site: &str,
        after_commit: Vec<IrStmt>,
    ) -> Result<Vec<IrStmt>, String> {
        let ProblemPlan {
            builder,
            vars,
            blocks,
        } = plan;
        let problem = self.model.rand_problems.len();
        let mut leaves = builder.leaves;
        let calls = builder.calls;
        let units = builder.units;
        self.model.rand_problems.push(builder.problem);
        let tag = format!("_llg_rand_{}_{problem}", node.index());
        let rand = |statement: IrRandStmt| IrStmt::Randomize(Box::new(statement));
        let mut statements = vec![rand(IrRandStmt::Begin { problem })];
        // Problems with units: the instance walk feeds the other units'
        // leaves; it runs after unit 0 is fed.
        let code = match units {
            Some(state) => {
                if let Some(handle) = state.root_object() {
                    statements.push(rand(IrRandStmt::InstRoot { problem, handle }));
                }
                let first = state.root_leaf_count();
                let rest = leaves.split_off(first);
                self.unit_statements(problem, &tag, *state, rest)?
            }
            None => super::templates::UnitCode {
                walk: Vec::new(),
                commit: Vec::new(),
                posts: Vec::new(),
            },
        };
        for var in &vars {
            let active = format!("{tag}_a{}", var.index);
            statements.push(IrStmt::DeclLocal {
                name: active.clone(),
                width: 1,
                signed: false,
                init: Some(Box::new(var.active.clone())),
                two_state: true,
            });
            statements.push(rand(IrRandStmt::SetVar {
                problem,
                var: var.index,
                value: var.current.clone(),
                active: local_read(&active, 1, false),
            }));
            if let Some(cyclic) = &var.cyclic {
                statements.push(rand(IrRandStmt::SetCyclic {
                    problem,
                    var: var.index,
                    state: cyclic.read.clone(),
                }));
            }
        }
        for (index, enabled) in blocks.into_iter().enumerate() {
            statements.push(rand(IrRandStmt::SetBlock {
                problem,
                block: index as u32,
                enabled,
            }));
        }
        for (index, leaf) in leaves.into_iter().enumerate() {
            statements.push(self.guarded_leaf(problem, None, index as u32, leaf.template));
        }
        statements.extend(code.walk);
        let step = || {
            IrExpr::new(
                IrExprKind::SysFunc(Box::new(IrSysFunc::RandStep {
                    problem,
                    stream: stream.clone().map(Box::new),
                    site: site.to_owned(),
                })),
                32,
                true,
                None,
            )
        };
        let status = format!("{tag}_s");
        let status_lhs = IrLhs::WholeRef {
            addr: format!("&{status}"),
            width: 32,
            signed: true,
            two_state: true,
            shortreal: false,
        };
        statements.push(IrStmt::DeclLocal {
            name: status.clone(),
            width: 32,
            signed: true,
            init: Some(Box::new(step())),
            two_state: true,
        });
        if !calls.is_empty() {
            let mut body = Vec::new();
            for (index, call) in calls.into_iter().enumerate() {
                let mut template = call.template;
                let mut substitution = |expr: &mut IrExpr| {
                    let IrExprKind::LocalRead(name) = &expr.kind else {
                        return;
                    };
                    let Some(var) = name
                        .strip_prefix(PLACEHOLDER_PREFIX)
                        .and_then(|index| index.parse::<u32>().ok())
                        .and_then(|index| vars.iter().find(|var| var.index == index))
                    else {
                        return;
                    };
                    let value = IrExpr::new(
                        IrExprKind::SysFunc(Box::new(IrSysFunc::RandValue {
                            problem,
                            var: var.index,
                        })),
                        var.width,
                        var.signed,
                        None,
                    );
                    *expr = IrExpr::new(
                        IrExprKind::Mux {
                            sel: Box::new(local_read(&format!("{tag}_a{}", var.index), 1, false)),
                            a: Box::new(value),
                            b: Box::new(var.current.clone()),
                        },
                        var.width,
                        var.signed,
                        None,
                    );
                };
                crate::sim::opt::walk_expr_mut(&mut template, &mut substitution);
                debug_assert!(!has_placeholder(&template) && placeholders_in(&template).is_empty());
                body.push(IrStmt::If {
                    cond: IrExpr::new(
                        IrExprKind::SysFunc(Box::new(IrSysFunc::RandCallReady {
                            problem,
                            call: index as u32,
                        })),
                        1,
                        false,
                        None,
                    ),
                    then_: vec![rand(IrRandStmt::SetCallResult {
                        problem,
                        call: index as u32,
                        value: template,
                    })],
                    els: None,
                    check: IrUniquePriorityCheck::None,
                });
            }
            body.push(IrStmt::Assign {
                lhs: status_lhs.clone(),
                rhs: step(),
                nba: false,
            });
            statements.push(IrStmt::While {
                cond: IrExpr::new(
                    IrExprKind::Bin {
                        op: IrBinOp::Eq,
                        a: Box::new(local_read(&status, 32, true)),
                        b: Box::new(packed_const(RAND_STEP_NEED_CALLS as u64, 32, true)),
                    },
                    1,
                    false,
                    None,
                ),
                body,
            });
        }
        statements.push(IrStmt::Assign {
            lhs: IrLhs::WholeRef {
                addr: format!("&{result}"),
                width: 32,
                signed: true,
                two_state: true,
                shortreal: false,
            },
            rhs: local_read(&status, 32, true),
            nba: false,
        });
        let mut commit = Vec::new();
        for var in &vars {
            let value = IrExpr::new(
                IrExprKind::SysFunc(Box::new(IrSysFunc::RandValue {
                    problem,
                    var: var.index,
                })),
                var.width,
                var.signed,
                None,
            );
            let value = if var.two_state {
                IrExpr::to_two_state(value)
            } else {
                value
            };
            let mut then_ = vec![IrStmt::Assign {
                lhs: var.lhs.clone(),
                rhs: value,
                nba: false,
            }];
            if let Some(cyclic) = &var.cyclic {
                // The cycle advances only with a committed solution, so a
                // failed randomize leaves it unchanged (S30-D3).
                let width = crate::sim::ir::rand_cyclic_state_width(var.width);
                then_.push(IrStmt::Assign {
                    lhs: cyclic.target.clone(),
                    rhs: IrExpr::new(
                        IrExprKind::SysFunc(Box::new(IrSysFunc::RandCyclic {
                            problem,
                            var: var.index,
                        })),
                        width,
                        false,
                        None,
                    ),
                    nba: false,
                });
            }
            commit.push(IrStmt::If {
                cond: local_read(&format!("{tag}_a{}", var.index), 1, false),
                then_,
                els: None,
                check: IrUniquePriorityCheck::None,
            });
        }
        // Arrays and other instances commit after unit 0's variables; the
        // randomized object's post_randomize runs first, then the other
        // random objects' in registration order (S30-D7).
        commit.extend(code.commit);
        commit.extend(after_commit);
        commit.extend(code.posts);
        statements.push(IrStmt::If {
            cond: local_read(result, 32, true),
            then_: commit,
            els: None,
            check: IrUniquePriorityCheck::None,
        });
        Ok(statements)
    }

    /// Set state leaf `leaf`; a handle the value is read through that is
    /// null is reported to the solver instead of being dereferenced, so a
    /// guard can still eliminate the constraint (18.5.12).
    pub(super) fn guarded_leaf(
        &self,
        problem: usize,
        inst: Option<&IrExpr>,
        leaf: u32,
        template: IrExpr,
    ) -> IrStmt {
        let mut handles = Vec::new();
        let mut copy = template.clone();
        crate::sim::opt::walk_expr_mut(&mut copy, &mut |expr| {
            if let IrExprKind::LocalRead(name) = &expr.kind {
                self.collect_required_handles(name, &mut handles);
            }
        });
        let set = |value: Option<IrExpr>| {
            IrStmt::Randomize(Box::new(match inst {
                Some(inst) => IrRandStmt::ISetLeaf {
                    problem,
                    inst: inst.clone(),
                    leaf,
                    value,
                },
                None => IrRandStmt::SetLeaf {
                    problem,
                    leaf,
                    value,
                },
            }))
        };
        let mut statement = set(Some(template));
        for handle in handles.into_iter().rev() {
            statement = IrStmt::If {
                cond: super::super::objects::object_query(
                    IrObjectQuery::ChandleEq(handle, IrChandleExpr::Null),
                    1,
                    false,
                ),
                then_: vec![set(None)],
                els: Some(vec![statement]),
                check: IrUniquePriorityCheck::None,
            };
        }
        statement
    }

    /// The null-checked handles a native access named `name` dereferences,
    /// outermost first.
    fn collect_required_handles(&self, name: &str, handles: &mut Vec<IrChandleExpr>) {
        let Some(access) = name
            .strip_prefix("_llg_access_")
            .and_then(|index| index.parse::<usize>().ok())
            .and_then(|index| self.model.native_accesses.get(index))
        else {
            return;
        };
        if let IrChandleExpr::Required { handle, .. } = &access.receiver {
            if let IrChandleExpr::LocalRead(inner) = handle.as_ref() {
                self.collect_required_handles(inner, handles);
            }
            if !handles.contains(handle) {
                handles.push((**handle).clone());
            }
        }
    }
}
