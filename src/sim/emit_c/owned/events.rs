//! Numeric delays and waits on stable storage. Stack descriptors only borrow.
use super::*;
use crate::sim::execution::ScheduleRegion;

impl Frame<'_, '_> {
    pub(super) fn delay(&mut self, ticks: &IrDelay) -> Result<String, String> {
        Ok(match ticks {
            IrDelay::Constant(ticks) => format!("{ticks}ULL"),
            IrDelay::Runtime { value, unit_ticks, precision_ticks } => {
                let value = self.expression(value)?;
                let code = if value.width == 0 {
                    format!("sv4_real_delay_ticks({}, {unit_ticks}ULL, {precision_ticks}ULL)", value.code)
                } else { format!("sv4_delay_ticks({}, {unit_ticks}ULL)", value.code) };
                let scalar = self.scalar("uint64_t", code);
                self.discard(value);
                scalar
            }
        })
    }

    pub(super) fn event_address(&mut self, event: &IrEventRef) -> Result<String, String> {
        Ok(match event {
            IrEventRef::Static(index) => format!("&{}", self.ctx.model.event(*index).c_name()),
            IrEventRef::Null => "NULL".to_owned(),
            _ => return Err(pending("event-array or captured event handles")),
        })
    }

    fn dependency(&self, dependency: &IrDependency) -> Result<String, String> {
        Ok(match dependency {
            IrDependency::Scalar(name) | IrDependency::Real(name) => {
                let binding = self.lookup(name).ok_or_else(|| format!("unknown dependency {name}"))?;
                format!("{{ .{} = {} }}", if binding.width == 0 { "real" } else { "sig" }, binding.address)
            }
            IrDependency::ArrayElement { array, index } => {
                let array = self.ctx.model.array(*array);
                if array.real { format!("{{ .real = &{}[{index}] }}", array.c_name) }
                else { format!("{{ .sig = &{}_llg_element_deps[{index}] }}", array.c_name) }
            }
            IrDependency::ArrayContents(array) => format!("{{ .sig = &{}_llg_contents_dep }}", self.ctx.model.array(*array).c_name),
            IrDependency::PackedRange { storage, lsb, width } => {
                let (trigger, value) = match storage.as_ref() {
                    IrDependency::Scalar(name) => {
                        let binding = self.lookup(name).ok_or_else(|| format!("unknown dependency {name}"))?;
                        (binding.address.clone(), binding.address)
                    }
                    IrDependency::ArrayElement { array, index } => {
                        let name = &self.ctx.model.array(*array).c_name;
                        (format!("&{name}_llg_element_deps[{index}]"), format!("&{name}[{index}]"))
                    }
                    _ => return Err(pending("this packed-range dependency")),
                };
                format!("{{ .sig = {trigger}, .value = {value}, .lsb = {lsb}u, .width = {width}u }}")
            }
            _ => return Err(pending("container/object dependencies")),
        })
    }

    pub(super) fn wait_any(&mut self, sens: &[IrDependency], region: Option<ScheduleRegion>) -> Result<(), String> {
        if let Some(region) = region { self.line(format!("llg_wait_resume_in_region({});", region.runtime_symbol())); }
        if sens.is_empty() { self.line("llg_wait_any(NULL, 0);"); }
        else {
            let values = sens.iter().map(|item| self.dependency(item)).collect::<Result<Vec<_>, _>>()?;
            let array = self.name("dependencies");
            self.line(format!("llg_wait_dependency_t {array}[] = {{ {} }};", values.join(", ")));
            self.line(format!("llg_wait_any_dependencies({array}, {});", sens.len()));
        }
        Ok(())
    }

    pub(super) fn wait_events(&mut self, specs: &[(IrWaitSrc, IrEdge)]) -> Result<(), String> {
        if specs.is_empty() { self.line("llg_wait_expressions(NULL, 0);"); return Ok(()); }
        let mut values = Vec::new();
        for (source, edge) in specs {
            let edge = match edge { IrEdge::Any => "LLG_EV_ANY", IrEdge::Posedge => "LLG_EV_POSEDGE", IrEdge::Negedge => "LLG_EV_NEGEDGE" };
            let (sig, real, event) = match source {
                IrWaitSrc::Sig(name) | IrWaitSrc::Real(name) => {
                    let binding = self.lookup(name).ok_or_else(|| pending("opaque wait address"))?;
                    if binding.width == 0 { ("NULL".to_owned(), binding.address, "NULL".to_owned()) }
                    else { (binding.address, "NULL".to_owned(), "NULL".to_owned()) }
                }
                IrWaitSrc::Event(event) => ("NULL".to_owned(), "NULL".to_owned(), self.event_address(event)?),
                _ => return Err(pending("evaluated/filtered event callbacks")),
            };
            let is_real = u8::from(real != "NULL");
            values.push(format!("{{ .sig = {sig}, .real_sig = {real}, .kind = {edge}, .event = {event}, .real = {is_real} }}"));
        }
        let name = self.name("events");
        self.line(format!("llg_expr_event_spec_t {name}[] = {{ {} }};", values.join(", ")));
        self.line(format!("llg_wait_expressions({name}, {});", specs.len()));
        Ok(())
    }
}
