//! File-scope symbols of the generated model.
//!
//! Every storage object, net, event, interface instance, subroutine, process
//! and callback is a file-scope object or function of `model.c` under its
//! C name, so the names of all of them are one name space. An activation's
//! array or native value is declared in its frame, and a signal that aliases
//! storage or drives a net uses the symbol of that storage or net.

use super::*;

impl Validator<'_> {
    /// No two file-scope symbols of the model have one name.
    pub(super) fn validate_symbols(&self) -> ValidationResult {
        let model: &IrModel = self.model.model;
        let mut names = HashSet::new();
        let mut unique = |table: &str, index: usize, kind: &str, name| {
            if names.insert(name) {
                return Ok(());
            }
            self.fail(
                format_args!("{table}[{index}].c_name"),
                format!("{kind} name `{name}` is not unique"),
            )
        };
        for (index, container) in model.containers.iter().enumerate() {
            unique("containers", index, "container", container.c_name.as_str())?;
        }
        for (index, object) in model.objects.iter().enumerate() {
            unique("objects", index, "object", &object.c_name)?;
        }
        for (index, signal) in model.signals.iter().enumerate() {
            if signal.alias.is_none() && signal.net_driver.is_none() && !signal.omit {
                unique("signals", index, "active storage", &signal.c_name)?;
            }
        }
        for (index, group) in model.net_groups.iter().enumerate() {
            unique("net_groups", index, "net", &group.c_name)?;
        }
        for (index, array) in model.arrays.iter().enumerate() {
            if array.is_model_storage() {
                unique("arrays", index, "static array storage", &array.c_name)?;
            }
        }
        for (index, value) in model.native_values.iter().enumerate() {
            if !value.activation {
                unique("native_values", index, "native value", &value.c_name)?;
            }
        }
        for (index, event) in model.events.iter().enumerate() {
            unique("events", index, "event", &event.c_name)?;
        }
        for interface in &model.virtual_interfaces {
            for (index, instance) in interface.instances.iter().enumerate() {
                unique("instances", index, "interface instance", &instance.c_name)?;
            }
        }
        for (index, func) in model.funcs.iter().enumerate() {
            unique("funcs", index, "function", &func.c_name)?;
        }
        for (index, process) in model.processes.iter().enumerate() {
            unique("processes", index, "process", &process.c_name)?;
        }
        let callbacks = (model.funcs.iter().flat_map(|func| &func.pre_fns))
            .chain(model.processes.iter().flat_map(|process| &process.pre_fns));
        for (index, callback) in callbacks.enumerate() {
            unique("pre_fns", index, "callback", callback.c_name())?;
        }
        Ok(())
    }
}
