//! Numeric delays and waits on stable storage. Stack descriptors only borrow.
use super::*;
use crate::sim::execution::ScheduleRegion;

/// Binding name of an event formal's activation-owned handle.
pub(super) fn event_formal_binding(index: usize) -> String {
    format!("_event_formal_{index}")
}

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
            IrEventRef::Handle(handle) => {
                // A container element stores only the object identity; give
                // the runtime a statement-lifetime handle naming it. Waits and
                // triggers resolve the object before this handle goes away.
                let object = self.chandle(handle)?;
                let event = self.scalar(
                    "llg_event_t",
                    format!("(llg_event_t){{ (llg_event_object_t*)({object}) }}"),
                );
                format!("&{event}")
            }
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
                let entries = values
                    .iter()
                    .map(|value| value.code.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                let args =
                    self.declare_array_init("sv4_t", "event_indices", values.len(), &entries);
                let address = self.scalar("llg_event_t*", format!(
                    "llg_event_array_select({name}__elements, {count}ULL, {name}__left, {name}__right, {args}, {})",
                    values.len()));
                for value in values {
                    self.discard(value);
                }
                address
            }
            IrEventRef::Formal(index) => self
                .event_bindings
                .iter()
                .rev()
                .find_map(|scope| scope.get(&event_formal_binding(*index)))
                .cloned()
                .ok_or_else(|| pending("unbound event formal handle"))?,
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
            IrDependency::RefFormal { index, real: true } => format!("{{ .real = r{index} }}"),
            IrDependency::RefFormal { index, real: false } => {
                format!("{{ .sig = llg_ref_dependency(r{index}) }}")
            }
            IrDependency::Scalar(name)
            | IrDependency::Real(name)
            | IrDependency::SharedCell { local: name, .. } => {
                let binding = self.resolve_lookup(name)?;
                format!(
                    "{{ .{} = {} }}",
                    if binding.width == 0 { "real" } else { "sig" },
                    binding.address
                )
            }
            IrDependency::ArrayElement { array, index } => {
                let array = self.ctx.model.array(*array);
                if array.sparse() {
                    format!("{{ .sig = {} }}", array.cell_address(&index.to_string()))
                } else if array.real {
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
                        let array = self.ctx.model.array(*array);
                        let name = &array.c_name;
                        if array.sparse() {
                            let cell = array.cell_address(&index.to_string());
                            (cell.clone(), cell)
                        } else {
                            (
                                format!("&{name}_llg_element_deps[{index}]"),
                                format!("&{name}[{index}]"),
                            )
                        }
                    }
                    _ => return Err(pending("this packed-range dependency")),
                };
                format!(
                    "{{ .sig = {trigger}, .value = {value}, .lsb = {lsb}u, .width = {width}u }}"
                )
            }
            // An activation container's markers exist only when it is shared
            // with fork branches, the only other processes that can change it.
            IrDependency::ContainerContents(index) | IrDependency::ContainerShape(index)
                if !self.ctx.model.containers[*index].is_global_storage() =>
            {
                let container = self.container_name(*index)?;
                let field = if matches!(dependency, IrDependency::ContainerShape(_)) {
                    "shape_dependency"
                } else {
                    "contents_dependency"
                };
                format!("{{ .sig = llg_dependency_or_never({container}.{field}) }}")
            }
            IrDependency::ContainerContents(index) => format!(
                "{{ .sig = &{}_llg_contents_dep }}",
                self.ctx.model.containers[*index].c_name
            ),
            IrDependency::ContainerShape(index) => format!(
                "{{ .sig = &{}_llg_shape_dep }}",
                self.ctx.model.containers[*index].c_name
            ),
            IrDependency::NativeAccess(name) => self.native_access_dependency(name)?,
            IrDependency::Object(index) => {
                let object = &self.ctx.model.objects[*index];
                if !object.ty.has_change_marker() {
                    return Err(pending("semaphore object dependencies"));
                }
                format!("{{ .sig = &{}_llg_dep }}", object.c_name)
            }
        })
    }

    /// The storage a native access selects when the wait arms. The receiver
    /// renders quietly: a null handle at any step selects never-changing
    /// storage instead of reporting an access error.
    fn native_access_dependency(&mut self, name: &str) -> Result<String, String> {
        let access = self
            .ctx
            .model
            .native_accesses
            .iter()
            .find(|access| access.name == name)
            .cloned()
            .ok_or_else(|| pending(&format!("unresolved native access dependency {name}")))?;
        let previous = std::mem::replace(&mut self.quiet_receivers, true);
        let receiver = self.chandle(&access.receiver);
        self.quiet_receivers = previous;
        let receiver = receiver?;
        Ok(match access.kind {
            IrNativeAccessKind::ClassField { class, field } => {
                match self.ctx.model.classes[class].fields[field].ty {
                    // A handle property: the marker its object toggles when
                    // one of its handle properties is rebound.
                    IrClassFieldType::Chandle => {
                        format!("{{ .sig = llg_class_handle_dependency({receiver}) }}")
                    }
                    IrClassFieldType::Real { .. } => format!(
                        "{{ .real = llg_class_real_dependency({receiver}, {class}, {field}) }}"
                    ),
                    // A string property: its own marker, allocated on the
                    // first wait that observes it.
                    IrClassFieldType::String => format!(
                        "{{ .sig = llg_class_string_dependency({receiver}, {class}, {field}) }}"
                    ),
                    _ => format!(
                        "{{ .sig = llg_class_packed_dependency({receiver}, {class}, {field}) }}"
                    ),
                }
            }
            IrNativeAccessKind::InterfaceMember { interface, member }
                if self.ctx.model.virtual_interfaces[interface].members[member].real =>
            {
                format!(
                    "{{ .real = llg_vif_real_member_quiet({receiver}, {interface}, {member}) }}"
                )
            }
            IrNativeAccessKind::InterfaceMember { interface, member } => format!(
                "{{ .sig = llg_dependency_or_never(llg_vif_member_dependency({receiver}, {interface}, {member})) }}"
            ),
            IrNativeAccessKind::ValueItem { .. } | IrNativeAccessKind::ElementItem { .. } => {
                return Err(pending("native value item wait dependencies"))
            }
        })
    }

    pub(super) fn wait_any(
        &mut self,
        sens: &[IrDependency],
        region: Option<ScheduleRegion>,
        operation: SuspensionOperation,
    ) -> Result<(), String> {
        let cancellation_mark = self.cancellation_mark();
        if let Some(region) = region {
            self.line(format!(
                "llg_wait_resume_in_region({});",
                region.runtime_symbol()
            ));
        }
        if sens.is_empty() {
            self.await_arm(operation, "llg_arm_any(self, NULL, 0)")?;
        } else {
            let values = sens
                .iter()
                .map(|item| self.dependency(item))
                .collect::<Result<Vec<_>, _>>()?;
            let array = self.arm_array(
                "llg_wait_dependency_t",
                "dependencies",
                values.len(),
                &values.join(", "),
            );
            // Event controls (explicit, implicit or a process trigger) are
            // resensitized by resume(); a wait condition keeps its wake.
            let arm = if operation == SuspensionOperation::ConditionWait {
                "llg_arm_any_dependencies"
            } else {
                "llg_arm_event_dependencies"
            };
            self.await_arm(operation, format!("{arm}(self, {array}, {})", sens.len()))?;
        }
        self.cancellation_check_covering(cancellation_mark)
    }
}

impl Frame<'_, '_> {
    /// Emit a read-only scheduler query. Both queries are plain runtime reads
    /// with no ownership transfer, so they are legal wherever a value is.
    pub(super) fn runtime_query(&mut self, query: &IrRuntimeQuery) -> Result<Value, String> {
        Ok(match query {
            IrRuntimeQuery::WaitRefreshed => self.value(
                "sv4_from_u64(llg_wait_refreshed(self) ? 1ULL : 0ULL, 1, 0)".to_owned(),
                1,
                false,
            ),
            IrRuntimeQuery::ExpectOutcome(identity) => self.value(
                format!(
                    "sv4_from_u64((uint64_t)llg_assertion_expect_outcome({identity}ULL), 2, 0)"
                ),
                2,
                false,
            ),
            IrRuntimeQuery::EventTriggerCount(event) => {
                let event = self.event_address(&IrEventRef::Static(*event))?;
                self.value(
                    format!("sv4_from_u64(llg_event_trigger_count({event}), 64, 0)"),
                    64,
                    false,
                )
            }
            IrRuntimeQuery::ForceSourceActive(index) => {
                let signal = self.ctx.model.signal(*index);
                if !signal.net_alias.is_empty() {
                    return Err("a force source cannot be a net alias".to_owned());
                }
                let arguments = if matches!(signal.ty, IrType::Real { .. }) {
                    format!("NULL, &{}", signal.c_name)
                } else {
                    format!("&{}, NULL", signal.c_name)
                };
                self.value(
                    format!(
                        "sv4_from_u64(llg_force_source_active({arguments}) ? 1ULL : 0ULL, 1, 0)"
                    ),
                    1,
                    false,
                )
            }
        })
    }
}
