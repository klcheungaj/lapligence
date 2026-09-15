//! Initialization.

use super::*;

impl Codegen<'_> {

    /// Lower class-valued declaration initializers into run-once processes.
    /// They are inserted before user processes so allocation, property
    /// defaults, and constructors have completed at time zero.
    pub(in super::super) fn emit_class_object_initializers(&mut self) -> Result<(), String> {
        let initializers = std::mem::take(&mut self.class_object_initializers);
        let mut processes = Vec::with_capacity(initializers.len());
        for (index, (object_node, object, initializer, path)) in
            initializers.into_iter().enumerate()
        {
            let owner = self.owning_inst(object_node).ok_or_else(|| {
                format!(
                    "class initializer for `{}` has no owning module instance",
                    self.node(object_node).full_name
                )
            })?;
            self.inst = owner;
            let value = self.lower_chandle(&path, initializer)?;
            let c_name = format!("p_{}_class_init_{index}", ident(&path));
            let label = format!("{path}.class_initializer.{index}");
            processes.push(IrProcess::new_with_origin(
                c_name,
                label,
                IrShape::RunOnce,
                Vec::new(),
                vec![IrStmt::Object(IrObjectStmt::ChandleAssign(object, value))],
                self.origin(object_node),
            ));
        }
        self.model.processes.splice(0..0, processes);
        Ok(())
    }

    /// Construct semaphore objects from declaration initializers in a small
    /// run-once process.  `new(...)` is a runtime operation and cannot appear
    /// in a C static initializer; running these before user processes preserves
    /// the time-zero object construction order.
    pub(in super::super) fn emit_semaphore_initializers(&mut self) -> Result<(), String> {
        let initializers = std::mem::take(&mut self.semaphore_initializers);
        let mut processes = Vec::with_capacity(initializers.len());
        for (index, (object_node, object, initializer, path)) in
            initializers.into_iter().enumerate()
        {
            let owner = self.owning_inst(object_node).ok_or_else(|| {
                format!(
                    "semaphore initializer for `{}` has no owning module instance",
                    self.node(object_node).full_name
                )
            })?;
            self.inst = owner;
            let value = self.lower_chandle(&path, initializer)?;
            let c_name = format!("p_{}_semaphore_init_{index}", ident(&path));
            let label = format!("{path}.semaphore_initializer.{index}");
            processes.push(IrProcess::new_with_origin(
                c_name,
                label,
                IrShape::RunOnce,
                Vec::new(),
                vec![IrStmt::Object(IrObjectStmt::ChandleAssign(object, value))],
                self.origin(object_node),
            ));
        }
        self.model.processes.splice(0..0, processes);
        Ok(())
    }

    /// Mailbox constructors are deferred until the model's process table is
    /// otherwise complete, matching class initializer ordering while keeping
    /// construction in the runtime-owned mailbox registry.
    pub(in super::super) fn emit_mailbox_object_initializers(&mut self) -> Result<(), String> {
        let initializers = std::mem::take(&mut self.mailbox_object_initializers);
        let mut processes = Vec::with_capacity(initializers.len());
        for (index, (object_node, object, initializer, path)) in
            initializers.into_iter().enumerate()
        {
            let owner = self.owning_inst(object_node).ok_or_else(|| {
                format!(
                    "mailbox initializer for {} has no owning module instance",
                    self.node(object_node).full_name
                )
            })?;
            self.inst = owner;
            let value = self.lower_mailbox_expr(
                &path,
                initializer,
                self.mailbox_element_for_decl(object_node),
            )?;
            let c_name = format!("p_{}_mailbox_init_{index}", ident(&path));
            let label = format!("{path}.mailbox_initializer.{index}");
            processes.push(IrProcess::new_with_origin(
                c_name,
                label,
                IrShape::RunOnce,
                Vec::new(),
                vec![IrStmt::Object(IrObjectStmt::MailboxAssign(object, value))],
                self.origin(object_node),
            ));
        }
        self.model.processes.splice(0..0, processes);
        Ok(())
    }
}
