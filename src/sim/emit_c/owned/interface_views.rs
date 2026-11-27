//! Views of unpacked interface members bound through virtual-interface
//! handles (`IrStmt::InterfaceBind`, SV 25.9).
use super::*;
use crate::sim::ir::{IrInterfaceBinding, IrInterfaceStorage};

impl Frame<'_, '_> {
    /// Bind a view for the statements after the binding: the storage slot
    /// of the instance the handle binds now, read without reporting. A
    /// bound array is named by its typed base pointer, a bound container by
    /// its lvalue, as lexical storage is.
    pub(super) fn interface_bind(&mut self, binding: &IrInterfaceBinding) -> Result<(), String> {
        // A handle that a use reads again must name storage, not a value
        // the binding computed.
        let again = binding
            .refresh
            .then(|| self.reread_handle(&binding.handle))
            .flatten();
        let handle = self.chandle(&binding.handle)?;
        let storage = self.scalar(
            "llg_vif_storage_t*",
            format!(
                "llg_vif_storage_quiet({handle}, {}, {})",
                binding.interface, binding.slot
            ),
        );
        // The storage may go unread, as where a statement names the member
        // only for its constant shape (the header of a `foreach` over a fixed
        // array, an array query); the C compiler must not warn about it.
        self.line(format!("(void){storage};"));
        let site = c_string_literal(&binding.site);
        // Where a failed access cannot leave (a callback), a use through an
        // unbound view reports and reads the storage of the first instance,
        // or the template of a descriptor without instances, which has the
        // view's shape, while the run ends.
        if self.failed_access_exit().is_none() {
            let fallback = match self.ctx.model.virtual_interfaces[binding.interface]
                .instances
                .first()
            {
                Some(instance) => format!("&{}.storage[{}]", instance.c_name, binding.slot),
                None => format!(
                    "&{}[{}]",
                    crate::sim::emit_c::model::template_storage_name(binding.interface),
                    binding.slot
                ),
            };
            let base = format!("llg_vif_valid({storage}, {fallback}, {site})");
            return self.bind_view(binding.view, &base, None);
        }
        // A use checks the binding before its line runs; a use bound again
        // also binds where its operand is evaluated, keeping the storage it
        // checked when the handle no longer binds an instance (the failed
        // access has been reported and the run ends at the next check).
        let (check, base) = match &again {
            Some(handle) => (
                format!(
                    "llg_vif_bound(({storage} = llg_vif_storage_quiet({handle}, {}, {})), {site})",
                    binding.interface, binding.slot
                ),
                format!(
                    "llg_vif_rebind(&{storage}, {handle}, {}, {}, {site})",
                    binding.interface, binding.slot
                ),
            ),
            None => (format!("llg_vif_bound({storage}, {site})"), storage),
        };
        self.bind_view(binding.view, &base, Some(check))
    }

    /// Name view `view` by the storage pointer `base`; each use runs
    /// `check` first.
    fn bind_view(
        &mut self,
        view: IrInterfaceStorage,
        base: &str,
        check: Option<String>,
    ) -> Result<(), String> {
        match view {
            IrInterfaceStorage::Array(index) => {
                // A typed accessor rather than a cast keeps the base of an
                // element address a runtime call, which coroutine frames may
                // take addresses through.
                let array = self.ctx.model.array(index);
                let accessor = if array.sparse() {
                    "llg_vif_sparse"
                } else if array.real {
                    "llg_vif_real"
                } else {
                    "llg_vif_dense"
                };
                self.fixed_arrays
                    .insert(index, format!("{accessor}({base})"));
            }
            IrInterfaceStorage::Container(index) => {
                let (ty, _, _) = super::super::containers::activation_storage(
                    &self.ctx.model.containers[index],
                    "",
                )?;
                self.containers
                    .insert(index, format!("(*({ty}*){base}->base)"));
            }
        }
        match check {
            Some(check) => self.interface_views.insert(view, check),
            None => self.interface_views.remove(&view),
        };
        Ok(())
    }

    /// The C expression that reads handle `handle` again where it is
    /// evaluated, when the handle is storage (a variable, a formal, a
    /// property, a nested instance of one); `None` for a computed handle
    /// (an element of an array of handles, a call), which the binding
    /// evaluates once.
    fn reread_handle(&self, handle: &IrChandleExpr) -> Option<String> {
        match handle {
            IrChandleExpr::VirtualInterfaceNested {
                handle,
                interface,
                slot,
                ..
            } => Some(format!(
                "llg_vif_nested_quiet({}, {interface}, {slot})",
                self.reread_handle(handle)?
            )),
            IrChandleExpr::InterfaceInstance { .. } => None,
            _ => self.plain_handle_code(handle, 0).ok(),
        }
    }

    /// Note a use of view `view`, if it is one: a view bound through a null
    /// handle reports at the use and ends the run before the line that
    /// uses it (decision S99-D8). An operand that is not evaluated reports
    /// nothing.
    pub(super) fn use_interface_view(&self, view: IrInterfaceStorage) {
        if self.quiet_receivers {
            return;
        }
        let Some(check) = self.interface_views.get(&view) else {
            return;
        };
        let mut checks = self.container_receivers.borrow_mut();
        if !checks.contains(check) {
            checks.push(check.clone());
        }
    }
}
