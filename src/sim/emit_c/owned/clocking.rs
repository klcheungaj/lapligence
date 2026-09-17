//! Clocking operations capture operands before registration and borrow them
//! during the runtime's copy/retain boundary, including selected write masks.
use super::*;

impl Frame<'_, '_> {
    pub(super) fn canonical_signal(&self, index: usize) -> String {
        let signal = self.ctx.model.signal(index);
        if signal.net_alias.is_empty() { format!("&{}", signal.c_name) }
        else { format!("&llg_net_alias_{index}.visible") }
    }

    pub(super) fn event_repeat(&mut self, repeat: Option<&IrExpr>) -> Result<String, String> {
        let Some(repeat) = repeat else { return Ok("1ULL".to_owned()); };
        let value = self.expression(repeat)?;
        if value.width == 0 { return Err("event repeat count must be integral".to_owned()); }
        let count = self.scalar("uint64_t", format!("llg_repeat_count({})", value.code));
        self.discard(value);
        Ok(count)
    }

    pub(super) fn clocking_sources(&mut self, specs: &[(IrWaitSrc, IrEdge)]) -> Result<String, String> {
        if specs.is_empty() { return Err("clocking operation requires an associated event".to_owned()); }
        let mut entries = Vec::new();
        for (source, edge) in specs {
            let edge = match edge { IrEdge::Any => "LLG_EV_ANY", IrEdge::Posedge => "LLG_EV_POSEDGE", IrEdge::Negedge => "LLG_EV_NEGEDGE" };
            entries.push(match source {
                IrWaitSrc::Sig(name) => {
                    let binding = self.lookup(name).ok_or_else(|| format!("unknown clocking source {name}"))?;
                    if binding.width == 0 || binding.automatic { return Err("clocking source must be persistent packed storage".to_owned()); }
                    format!("{{ .sig = {}, .kind = {edge}, .ev = NULL }}", binding.address)
                }
                IrWaitSrc::Event(event) => format!("{{ .sig = NULL, .kind = {edge}, .ev = {} }}", self.event_address(event)?),
                _ => return Err("clocking operation requires a simple signal or named event".to_owned()),
            });
        }
        let name = self.name("clocking_sources");
        self.line(format!("llg_wait_src_t {name}[] = {{ {} }};", entries.join(", ")));
        Ok(name)
    }

    pub(super) fn clocking_drive(&mut self, lhs: &IrLhs, rhs: &IrExpr,
        ticks: &IrDelay, specs: &[(IrWaitSrc, IrEdge)]) -> Result<(), String> {
        let value = self.expression(rhs)?;
        let target = self.target(lhs)?;
        if target.sequence_local || (target.binding.automatic && target.width == 0) {
            return Err("clocking drive requires stable target storage".to_owned());
        }
        let value = self.convert(value, target.width, target.signed, target.binding.two_state, target.binding.shortreal);
        let ticks = self.delay(ticks)?;
        let sources = self.clocking_sources(specs)?;
        let tail = format!("{ticks}, {sources}, {}", specs.len());
        self.line(format!("if ({}) {{", target.valid));
        if target.width == 0 {
            self.line(format!("llg_clocking_nba_d_sync_after({}, {}, {tail});", target.binding.address, value.code));
        } else if let Some(selection) = &target.selection {
            // Unselected bits are not an issue-time read. The mask merges into
            // the destination at commit, preserving overlapping NBA order.
            let updated = self.value(format!("sv4_zero({}, 0)", target.binding.width), target.binding.width, false);
            let mask = self.value(format!("sv4_zero({}, 0)", target.binding.width), target.binding.width, false);
            let ones = self.value(format!("sv4_fill(1, {}, 0)", target.width), target.width, false);
            self.set_selected(selection, &updated.code, &value.code);
            self.set_selected(selection, &mask.code, &ones.code);
            self.line(if let Some((net, slot)) = &target.net {
                format!("llg_clocking_nba_net_sync_masked_after(&{net}, {slot}, {}, {}, {tail});", updated.code, mask.code)
            } else {
                format!("llg_clocking_nba_sync_masked_after({}, {}, {}, {tail});", target.binding.address, updated.code, mask.code)
            });
            self.discard(ones); self.discard(mask); self.discard(updated);
        } else {
            self.line(if let Some((net, slot)) = &target.net {
                format!("llg_clocking_nba_net_sync_after(&{net}, {slot}, {}, {tail});", value.code)
            } else { format!("llg_clocking_nba_sync_after({}, {}, {tail});", target.binding.address, value.code) });
        }
        self.line("}");
        self.discard(value); self.release_target(target);
        Ok(())
    }
}
