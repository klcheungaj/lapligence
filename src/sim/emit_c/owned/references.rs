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
        let target = self.target(lhs)?;
        if target.width != width || width == 0 || target.net.is_some() {
            return Err("reference argument requires matching packed variable storage".to_owned());
        }
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
