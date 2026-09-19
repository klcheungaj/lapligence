//! Numeric delays and waits on stable storage. Stack descriptors only borrow.
use super::*;
use crate::sim::execution::ScheduleRegion;

impl Frame<'_, '_> {
    pub(super) fn delay(&mut self, ticks: &IrDelay) -> Result<String, String> {
        Ok(match ticks {
            IrDelay::Constant(ticks) => format!("{ticks}ULL"),
            IrDelay::Runtime {
                value,
                unit_ticks,
                precision_ticks,
            } => {
                let value = self.expression(value)?;
                let code = if value.width == 0 {
                    format!(
                        "sv4_real_delay_ticks({}, {unit_ticks}ULL, {precision_ticks}ULL)",
                        value.code
                    )
                } else {
                    format!("sv4_delay_ticks({}, {unit_ticks}ULL)", value.code)
                };
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
            IrEventRef::Array { array, indices } => {
                let descriptor = self.ctx.model.event(*array);
                let dims = descriptor
                    .array_dims()
                    .ok_or_else(|| "event handle is not an array".to_owned())?;
                if dims.len() != indices.len() || indices.is_empty() {
                    return Err("event array index rank does not match its dimensions".to_owned());
                }
                let name = descriptor.c_name().to_owned();
                let count = descriptor.array_elements().len();
                let mut values = Vec::new();
                for index in indices {
                    values.push(self.expression(index)?);
                }
                let args = self.name("event_indices");
                self.line(format!(
                    "const sv4_t {args}[] = {{ {} }}; /* borrowed descriptors */",
                    values
                        .iter()
                        .map(|value| value.code.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
                let address = self.scalar("llg_event_t*", format!(
                    "llg_event_array_select({name}__elements, {count}ULL, {name}__left, {name}__right, {args}, {})",
                    values.len()));
                for value in values {
                    self.discard(value);
                }
                address
            }
            IrEventRef::Captured(name) => self
                .event_bindings
                .iter()
                .rev()
                .find_map(|scope| scope.get(name))
                .cloned()
                .ok_or_else(|| pending(&format!("unresolved captured event handle {name}")))?,
        })
    }

    pub(super) fn dependency(&mut self, dependency: &IrDependency) -> Result<String, String> {
        Ok(match dependency {
            IrDependency::Scalar(name) | IrDependency::Real(name) => {
                let binding = self.resolve_lookup(name)?;
                format!(
                    "{{ .{} = {} }}",
                    if binding.width == 0 { "real" } else { "sig" },
                    binding.address
                )
            }
            IrDependency::ArrayElement { array, index } => {
                let array = self.ctx.model.array(*array);
                if array.real {
                    format!("{{ .real = &{}[{index}] }}", array.c_name)
                } else {
                    format!("{{ .sig = &{}_llg_element_deps[{index}] }}", array.c_name)
                }
            }
            IrDependency::ArrayContents(array) => format!(
                "{{ .sig = &{}_llg_contents_dep }}",
                self.ctx.model.array(*array).c_name
            ),
            IrDependency::PackedRange {
                storage,
                lsb,
                width,
            } => {
                let (trigger, value) = match storage.as_ref() {
                    IrDependency::Scalar(name) => {
                        let binding = self.resolve_lookup(name)?;
                        (binding.address.clone(), binding.address)
                    }
                    IrDependency::ArrayElement { array, index } => {
                        let name = &self.ctx.model.array(*array).c_name;
                        (
                            format!("&{name}_llg_element_deps[{index}]"),
                            format!("&{name}[{index}]"),
                        )
                    }
                    _ => return Err(pending("this packed-range dependency")),
                };
                format!(
                    "{{ .sig = {trigger}, .value = {value}, .lsb = {lsb}u, .width = {width}u }}"
                )
            }
            IrDependency::ContainerContents(index) => format!(
                "{{ .sig = &{}_llg_contents_dep }}",
                self.ctx.model.containers[*index].c_name
            ),
            IrDependency::ContainerShape(index) => format!(
                "{{ .sig = &{}_llg_shape_dep }}",
                self.ctx.model.containers[*index].c_name
            ),
            IrDependency::Object(index) => {
                let object = &self.ctx.model.objects[*index];
                if object.ty != IrObjectType::String {
                    return Err(pending("non-string object dependencies"));
                }
                format!("{{ .sig = &{}_llg_dep }}", object.c_name)
            }
        })
    }

    pub(super) fn wait_any(
        &mut self,
        sens: &[IrDependency],
        region: Option<ScheduleRegion>,
    ) -> Result<(), String> {
        if let Some(region) = region {
            self.line(format!(
                "llg_wait_resume_in_region({});",
                region.runtime_symbol()
            ));
        }
        if sens.is_empty() {
            self.line("llg_wait_any(NULL, 0);");
        } else {
            let values = sens
                .iter()
                .map(|item| self.dependency(item))
                .collect::<Result<Vec<_>, _>>()?;
            let array = self.name("dependencies");
            self.line(format!(
                "llg_wait_dependency_t {array}[] = {{ {} }};",
                values.join(", ")
            ));
            self.line(format!(
                "llg_wait_any_dependencies({array}, {});",
                sens.len()
            ));
        }
        self.cancellation_check()
    }
}
