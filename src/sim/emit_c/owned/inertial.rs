//! Inertial drivers retain their own snapshots; generated operands are borrowed.
use super::*;

impl Frame<'_, '_> {
    pub(super) fn inertial_assign(
        &mut self,
        lhs: &IrLhs,
        rhs: &IrExpr,
        delay: &IrInertialDelay,
    ) -> Result<(), String> {
        if !matches!(
            lhs,
            IrLhs::Whole(_)
                | IrLhs::Bit(..)
                | IrLhs::Part(..)
                | IrLhs::IdxPart(..)
                | IrLhs::ArrayElem { .. }
        ) {
            return Err("inertial update requires a persistent packed target".to_owned());
        }
        let value = self.expression(rhs)?;
        let target = self.target(lhs)?;
        if target.binding.width == 0 {
            return Err("inertial update requires a packed target".to_owned());
        }
        let value = self.convert(
            value,
            target.width,
            target.signed,
            target.binding.two_state,
            false,
        );
        // One handle per source site AND array element. Selecting a new element
        // must not cancel a pending update to the previously selected element.
        let handle = if let IrLhs::ArrayElem { arr, .. } = lhs {
            let array = self.ctx.model.array(*arr);
            if array.sparse() {
                let driver = self.site_static("const char", "inertial", None);
                format!(
                    "llg_fixed_array_inertial({}, &{driver})",
                    target.binding.address
                )
            } else {
                let total = array.total;
                let c_name = array.c_name.clone();
                let driver = self.site_static("llg_inertial_t*", "inertial", Some(total));
                format!("&{driver}[{} - {c_name}]", target.binding.address)
            }
        } else {
            let driver = self.site_static("llg_inertial_t*", "inertial", None);
            format!("&{driver}")
        };
        let delays = match delay {
            IrInertialDelay::Constant(delay) => format!(
                "{}ULL, {}ULL, {}ULL",
                delay.rise, delay.fall, delay.turn_off
            ),
            IrInertialDelay::Annotated(slot) => {
                let base = 3 * *slot;
                format!(
                    "llg_sdf_gate_delays[{base}], llg_sdf_gate_delays[{}], llg_sdf_gate_delays[{}]",
                    base + 1,
                    base + 2
                )
            }
            // Runtime values are evaluated each time the site schedules.
            IrInertialDelay::Runtime(delay) => {
                let rise = self.delay(&delay.rise)?;
                let fall = match &delay.fall {
                    Some(fall) => self.delay(fall)?,
                    None => rise.clone(),
                };
                let turn_off = match (&delay.turn_off, &delay.fall) {
                    (Some(turn_off), _) => self.delay(turn_off)?,
                    (None, Some(_)) => format!("({rise} < {fall} ? {rise} : {fall})"),
                    (None, None) => rise.clone(),
                };
                format!("{rise}, {fall}, {turn_off}")
            }
        };
        self.line(format!("if ({}) {{", target.valid));
        if let Some(selection) = &target.selection {
            let updated = self.value(
                format!("sv4_clone({})", target.binding.address),
                target.binding.width,
                target.binding.signed,
            );
            let mask = self.value(
                format!("sv4_zero({}, 0)", target.binding.width),
                target.binding.width,
                false,
            );
            let ones = self.value(
                format!("sv4_fill(1, {}, 0)", target.width),
                target.width,
                false,
            );
            self.set_selected(selection, &updated.code, &value.code);
            self.set_selected(selection, &mask.code, &ones.code);
            self.line(if let Some((net, slot)) = &target.net {
                format!(
                    "llg_inertial_selected_net({handle}, &{net}, {slot}, {}, {}, {delays});",
                    updated.code, mask.code
                )
            } else {
                format!(
                    "llg_inertial_selected_assign({handle}, {}, {}, {}, {delays});",
                    target.binding.address, updated.code, mask.code
                )
            });
            self.discard(ones);
            self.discard(mask);
            self.discard(updated);
        } else {
            self.line(if let Some((net, slot)) = &target.net {
                format!(
                    "llg_inertial_net({handle}, &{net}, {slot}, {}, {delays});",
                    value.code
                )
            } else {
                format!(
                    "llg_inertial_assign({handle}, {}, {}, {delays});",
                    target.binding.address, value.code
                )
            });
        }
        self.line("}");
        self.discard(value);
        self.release_target(target);
        Ok(())
    }

    /// Store a runtime-valued net delay in the net's propagation fields;
    /// the net's later value changes use it.
    pub(super) fn set_net_delay(
        &mut self,
        group: usize,
        delay: &IrRuntimeTransitionDelay,
    ) -> Result<(), String> {
        let rise = self.delay(&delay.rise)?;
        let fall = match &delay.fall {
            Some(fall) => self.delay(fall)?,
            None => rise.clone(),
        };
        let turn_off = match (&delay.turn_off, &delay.fall) {
            (Some(turn_off), _) => self.delay(turn_off)?,
            (None, Some(_)) => format!("({rise} < {fall} ? {rise} : {fall})"),
            (None, None) => rise.clone(),
        };
        let net = self.ctx.model.net_group(group);
        let (name, starts_unknown) = (net.c_name.clone(), net.starts_unknown);
        self.line(format!("{name}.propagation_rise = {rise};"));
        self.line(format!("{name}.propagation_fall = {fall};"));
        self.line(format!("{name}.propagation_turn_off = {turn_off};"));
        // The first store lets the X start propagate (S2-D10); a later one
        // finds the net settled or pending, which a resolution keeps.
        if starts_unknown {
            self.line(format!("llg_net_resolve(&{name});"));
        }
        Ok(())
    }
}
