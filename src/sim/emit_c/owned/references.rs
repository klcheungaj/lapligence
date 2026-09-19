//! Reference descriptors borrow variables and retain movable queue cells.
use super::stores::Selection;
use super::*;

impl Frame<'_, '_> {
    pub(super) fn reference_address(&self, address: &str) -> Result<String, String> {
        // Forwarded references are canonical formal descriptors (rN). There
        // are no executable, string-keyed reference bindings in this frame.
        if let Some(function) = self.ctx.func {
            for (index, formal) in function.formals.iter().enumerate() {
                if formal.is_ref()
                    && !formal.string
                    && !formal.chandle
                    && address == format!("r{index}")
                {
                    return Ok(address.to_owned());
                }
            }
        }
        Err(pending(&format!("unbound typed reference {address}")))
    }

    pub(super) fn reference_argument(
        &mut self,
        lhs: &IrLhs,
        read: &IrExpr,
        width: u32,
        signed: bool,
        two_state: bool,
    ) -> Result<String, String> {
        // Call markers own these descriptors until invocation/copyout finishes.
        self.reference_argument_with_scopes(lhs, read, width, signed, two_state, &mut Vec::new())
    }

    pub(super) fn reference_argument_with_scopes(
        &mut self,
        lhs: &IrLhs,
        read: &IrExpr,
        width: u32,
        signed: bool,
        two_state: bool,
        scopes: &mut Vec<String>,
    ) -> Result<String, String> {
        if let IrLhs::Stream {
            parts,
            width: total,
            slice: 1,
            direction: IrStreamDirection::LeftToRight,
        } = lhs
        {
            if *total != width || parts.is_empty() {
                return Err("composite reference shape mismatch".into());
            }
            let count = parts.len();
            let allocation = self.scalar(
                "llg_value_scope_t*",
                format!("llg_value_scope_begin_object(sizeof(llg_ref_t*) * {count}, NULL)"),
            );
            scopes.push(allocation.clone());
            let table = self.scalar(
                "llg_ref_t**",
                format!("(llg_ref_t**)llg_value_scope_object({allocation})"),
            );
            for (index, (part, part_width)) in parts.iter().enumerate() {
                let pointer = self.reference_argument_with_scopes(
                    part,
                    read,
                    *part_width,
                    false,
                    false,
                    scopes,
                )?;
                self.line(format!("{table}[{index}] = {pointer};"));
            }
            let allocation = self.scalar(
                "llg_value_scope_t*",
                "llg_value_scope_begin_object(sizeof(llg_ref_composite_t), NULL)".into(),
            );
            scopes.push(allocation.clone());
            let composite = self.scalar(
                "llg_ref_composite_t*",
                format!("(llg_ref_composite_t*)llg_value_scope_object({allocation})"),
            );
            self.line(format!(
                "*{composite} = (llg_ref_composite_t){{ .count = {count}, .parts = {table} }};"
            ));
            let allocation = self.scalar(
                "llg_value_scope_t*",
                "llg_value_scope_begin_object(sizeof(llg_ref_t), NULL)".into(),
            );
            scopes.push(allocation.clone());
            let pointer = self.scalar(
                "llg_ref_t*",
                format!("(llg_ref_t*)llg_value_scope_object({allocation})"),
            );
            self.line(format!("*{pointer} = (llg_ref_t){{ .kind = LLG_REF_COMPOSITE, .width = {width}, .is_signed = {}, .two_state = {}, .retained = {composite} }};", u8::from(signed), u8::from(two_state)));
            return Ok(pointer);
        }
        if let IrLhs::PackedSelect { target, steps, .. } = lhs {
            let root_width = match target.as_ref() {
                IrLhs::Whole(index) => self.ctx.model.signals[*index].ty.width(),
                IrLhs::WholeRef { width, .. }
                | IrLhs::Ref { width, .. }
                | IrLhs::Stream { width, .. } => *width,
                IrLhs::ArrayElem { arr, .. } => self.ctx.model.arrays[*arr].elem_width,
                _ => return Err("reference view requires a whole storage root".into()),
            };
            let parent = self
                .reference_argument_with_scopes(target, read, root_width, false, false, scopes)?;
            let Some(Selection::PackedChain(plan, _)) =
                self.selection(&IrElemSel::PackedChain(steps.clone()), root_width)?
            else {
                return Err("reference view has no selection plan".into());
            };
            let allocation = self.scalar(
                "llg_value_scope_t*",
                "llg_value_scope_begin_object(sizeof(llg_ref_view_t), NULL)".into(),
            );
            scopes.push(allocation.clone());
            let view = self.scalar(
                "llg_ref_view_t*",
                format!("(llg_ref_view_t*)llg_value_scope_object({allocation})"),
            );
            self.line(format!(
                "*{view} = (llg_ref_view_t){{ .parent = {parent}, .plan = {plan} }};"
            ));
            let allocation = self.scalar(
                "llg_value_scope_t*",
                "llg_value_scope_begin_object(sizeof(llg_ref_t), NULL)".into(),
            );
            scopes.push(allocation.clone());
            let pointer = self.scalar(
                "llg_ref_t*",
                format!("(llg_ref_t*)llg_value_scope_object({allocation})"),
            );
            self.line(format!("*{pointer} = (llg_ref_t){{ .kind = LLG_REF_VIEW, .width = {width}, .is_signed = {}, .two_state = {}, .retained = {view} }};", u8::from(signed), u8::from(two_state)));
            return Ok(pointer);
        }
        if let IrExprKind::Container(expression) = read.kind() {
            if let IrContainerExpr::Get { container, index } = expression.as_ref() {
                let container = self.ctx.model.containers[*container].clone();
                if matches!(container.kind, IrContainerKind::Queue { .. })
                    && container.element.is_packed()
                {
                    let index = self.expression(index)?;
                    let pointer = self.scalar(
                        "llg_ref_t*",
                        format!(
                            "llg_ref_queue(&{}, sv4_to_index({}))",
                            container.c_name, index.code
                        ),
                    );
                    self.discard(index);
                    return Ok(pointer);
                }
            }
        }
        if let IrLhs::Ref {
            addr, bit: None, ..
        } = lhs
        {
            return self.reference_address(addr);
        }
        let mut target = self.target(lhs)?;
        scopes.append(&mut target.reference_scopes);
        if target.width != width || width == 0 || target.net.is_some() {
            return Err("reference argument requires matching packed variable storage".to_owned());
        }
        let two_state = two_state || target.binding.two_state;
        let selection = match &target.selection {
            None => ".kind = LLG_REF_WHOLE".to_owned(),
            Some(Selection::PackedChain(..)) => {
                return Err("a packed selection is not a legal ref actual".to_owned());
            }
            Some(Selection::Bit(index)) => format!(".kind = LLG_REF_BIT, .index = {index}"),
            Some(Selection::Part(left, right)) => format!(".kind = LLG_REF_PART, .left = {left}LL, .right = {right}LL"),
            Some(Selection::Indexed(base, width, negative)) => format!(".kind = LLG_REF_INDEXED, .index = sv4_to_index({}), .indexed_width = {width}, .indexed_negative = {}", base.code, u8::from(*negative)),
        };
        // The call's C activation survives coroutine suspension. A process/root
        // value scope owns the descriptor so early cancellation never escapes
        // with a pointer to an expired inner C block.
        let scope = self.scalar(
            "llg_value_scope_t*",
            "llg_value_scope_begin_object(sizeof(llg_ref_t), NULL)".to_owned(),
        );
        scopes.push(scope.clone());
        let pointer = self.scalar(
            "llg_ref_t*",
            format!("(llg_ref_t*)llg_value_scope_object({scope})"),
        );
        self.line(format!("*{pointer} = (llg_ref_t){{ .base = ({} ? {} : NULL), .width = {width}, .is_signed = {}, .two_state = {}, {selection} }};",
            target.valid, target.binding.address, u8::from(signed), u8::from(two_state)));
        self.release_target(target);
        Ok(pointer)
    }
}
