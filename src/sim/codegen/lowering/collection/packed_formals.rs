//! Packed members retain their activation root, never a synthetic model signal.

use super::packed_elements::Select;
use super::*;
use crate::sim::ir::IrPackedSelect;

struct Projection {
    root: NodeId,
    member: PackedMember,
    steps: Vec<IrPackedSelect>,
    selected: bool,
}

impl<'a> Codegen<'a> {
    fn packed_formal_projection(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<Projection>, String> {
        if self.func.is_none() {
            return Ok(None);
        }
        let mut current = node;
        let mut selectors = Vec::new();
        let (root, member) = loop {
            match self.kind(current) {
                NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                    selectors.push((*base, Select::Elements(vec![*index])));
                    current = *base;
                }
                NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                    selectors.push((*base, Select::Elements(indices.clone())));
                    current = *base;
                }
                NodeKind::Expr(ExprKind::PartSelect { base, left, right }) => {
                    selectors.push((*base, Select::Part(*left, *right)));
                    current = *base;
                }
                NodeKind::Expr(ExprKind::IndexedPartSelect {
                    base,
                    base_expr,
                    width_expr,
                    neg,
                }) => {
                    selectors.push((*base, Select::Indexed(*base_expr, *width_expr, *neg)));
                    current = *base;
                }
                NodeKind::Expr(ExprKind::HierPath { parts, refs }) => {
                    let function = self.func.as_ref().expect("checked function context");
                    let Some((position, root)) = refs.iter().enumerate().find_map(|(i, root)| {
                        let root = (*root)?;
                        let root = self.canonical_func_target(root).unwrap_or(root);
                        (function.arg_ir.contains_key(&root) || function.ret_node == Some(root))
                            .then_some((i, root))
                    }) else {
                        return Ok(None);
                    };
                    let Some(member) = self.packed_member_layout(root, &parts[position + 1..])
                    else {
                        return Ok(None);
                    };
                    break (root, member);
                }
                _ => return Ok(None),
            }
        };
        let mut steps = vec![IrPackedSelect {
            base: lhs_integer_expr(i128::from(member.lsb)),
            width: member.width,
        }];
        let selected = !selectors.is_empty();
        let mut width = member.width;
        for (base, select) in selectors.into_iter().rev() {
            self.packed_selection_steps(path, base, select, &mut width, &mut steps)?;
        }
        Ok(Some(Projection {
            root,
            member,
            steps,
            selected,
        }))
    }

    pub(in super::super) fn packed_formal_read(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrExpr>, String> {
        let Some(projection) = self.packed_formal_projection(path, node)? else {
            return Ok(None);
        };
        let function = self.func.as_ref().expect("projection has an activation");
        let mut value = if let Some(value) = function.arg_ir.get(&projection.root) {
            value.clone()
        } else {
            let ret = function
                .ret
                .as_ref()
                .ok_or_else(|| "packed return has no storage".to_owned())?;
            IrExpr::new(
                IrExprKind::LocalRead(ret.c_name.clone()),
                ret.width,
                ret.signed,
                None,
            )
        };
        for (index, step) in projection.steps.into_iter().enumerate() {
            value = packed_step_read(value, step);
            // A two-state member of a four-state union converts when that
            // member is read, before a later out-of-bounds select can add X.
            if index == 0 && projection.member.two_state {
                value = IrExpr::to_two_state(value);
            }
        }
        if !projection.selected {
            let width = value.width;
            value = IrExpr::resize_to(value, width, projection.member.signed);
        }
        Ok(Some(value))
    }

    pub(in super::super) fn packed_formal_lhs(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrLhs>, String> {
        let Some(projection) = self.packed_formal_projection(path, node)? else {
            return Ok(None);
        };
        let function = self.func.as_ref().expect("projection has an activation");
        if function.const_refs.contains(&projection.root) {
            return Err(format!(
                "cannot write through const ref `{}` in `{path}`",
                self.node(projection.root).name
            ));
        }
        let target = self.func_write_target(projection.root, "").ok_or_else(|| {
            format!(
                "packed formal `{}` has no writable activation in `{path}`",
                self.node(projection.root).name
            )
        })?;
        Ok(Some(IrLhs::PackedSelect {
            target: Box::new(self.lhs_to_ir(target)?),
            steps: projection.steps,
            signed: !projection.selected && projection.member.signed,
            two_state: projection.member.two_state,
        }))
    }
}

pub(in super::super) fn packed_step_read(base: IrExpr, step: IrPackedSelect) -> IrExpr {
    IrExpr::new(
        IrExprKind::IdxPartSel {
            base: Box::new(base),
            base_idx: Box::new(step.base),
            width_expr: Box::new(lhs_integer_expr(i128::from(step.width))),
            neg: false,
        },
        step.width,
        false,
        None,
    )
}
