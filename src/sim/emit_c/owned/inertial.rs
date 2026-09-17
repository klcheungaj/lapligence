//! Inertial drivers retain their own snapshots; generated operands are borrowed.
use super::*;

impl Frame<'_, '_> {
    pub(super) fn inertial_assign(&mut self, lhs: &IrLhs, rhs: &IrExpr, delay: IrTransitionDelay) -> Result<(), String> {
        if !matches!(lhs, IrLhs::Whole(_) | IrLhs::Bit(..) | IrLhs::Part(..)
            | IrLhs::IdxPart(..) | IrLhs::ArrayElem { .. }) {
            return Err("inertial update requires a persistent packed target".to_owned());
        }
        let value = self.expression(rhs)?;
        let target = self.target(lhs)?;
        if target.binding.width == 0 { return Err("inertial update requires a packed target".to_owned()); }
        let value = self.convert(value, target.width, target.signed, target.binding.two_state, false);
        let driver = self.name("inertial");
        // One handle per source site AND array element. Selecting a new element
        // must not cancel a pending update to the previously selected element.
        let handle = if let IrLhs::ArrayElem { arr, .. } = lhs {
            let array = self.ctx.model.array(*arr);
            self.line(format!("static llg_inertial_t* {driver}[{}] = {{0}};", array.total));
            format!("&{driver}[{} - {}]", target.binding.address, array.c_name)
        } else {
            self.line(format!("static llg_inertial_t* {driver} = NULL;"));
            format!("&{driver}")
        };
        let delays = format!("{}ULL, {}ULL, {}ULL", delay.rise, delay.fall, delay.turn_off);
        self.line(format!("if ({}) {{", target.valid));
        if let Some(selection) = &target.selection {
            let updated = self.value(format!("sv4_clone({})", target.binding.address), target.binding.width, target.binding.signed);
            let mask = self.value(format!("sv4_zero({}, 0)", target.binding.width), target.binding.width, false);
            let ones = self.value(format!("sv4_fill(1, {}, 0)", target.width), target.width, false);
            self.set_selected(selection, &updated.code, &value.code);
            self.set_selected(selection, &mask.code, &ones.code);
            self.line(if let Some((net, slot)) = &target.net {
                format!("llg_inertial_selected_net({handle}, &{net}, {slot}, {}, {}, {delays});", updated.code, mask.code)
            } else {
                format!("llg_inertial_selected_assign({handle}, {}, {}, {}, {delays});", target.binding.address, updated.code, mask.code)
            });
            self.discard(ones); self.discard(mask); self.discard(updated);
        } else {
            self.line(if let Some((net, slot)) = &target.net {
                format!("llg_inertial_net({handle}, &{net}, {slot}, {}, {delays});", value.code)
            } else { format!("llg_inertial_assign({handle}, {}, {}, {delays});", target.binding.address, value.code) });
        }
        self.line("}");
        self.discard(value); self.release_target(target);
        Ok(())
    }
}
