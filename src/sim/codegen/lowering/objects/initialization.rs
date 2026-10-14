//! Initialization.

use super::*;

impl Codegen<'_> {
    /// The environment a declaration initializer lowers in: its module,
    /// interface, package or `$unit` instance, or the nearest class for a
    /// static class property or a static local of a class method.
    pub(in super::super) fn initializer_environment(&self, declaration: NodeId) -> Option<NodeId> {
        let mut current = Some(declaration);
        while let Some(id) = current {
            if self.class_nodes.contains_key(&id) {
                return Some(id);
            }
            if matches!(self.kind(id), NodeKind::ModuleInst { .. })
                || self.is_runtime_environment(id)
            {
                return Some(id);
            }
            current = self.node(id).parent;
        }
        None
    }

    /// Class-handle, virtual-interface, semaphore and mailbox declaration
    /// initializers. In SystemVerilog they join the static initialization
    /// schedule (SV 6.8, 6.21, 10.5): each runs once before any process
    /// starts, after the static declarations its value reads and before the
    /// declarations that read it, whether the declaration belongs to a
    /// module, interface, package, `$unit` or a subroutine. Verilog-2001
    /// keeps them as run-once processes ahead of user processes.
    fn schedule_handle_initializer(
        &mut self,
        kind: &str,
        object_node: NodeId,
        initializer: NodeId,
        path: &str,
        lower: &mut dyn FnMut(&mut Self, &str) -> Result<IrObjectStmt, String>,
        processes: &mut Vec<IrProcess>,
    ) -> Result<(), String> {
        let owner = self.initializer_environment(object_node).ok_or_else(|| {
            format!(
                "{kind} initializer for `{}` has no owning module, interface, package or class",
                self.node(object_node).full_name
            )
        })?;
        self.inst = owner;
        self.depth_arg = "0".into();
        let statement = IrStmt::Object(Box::new(lower(self, path)?));
        if self.db.edition() == LanguageEdition::SystemVerilog2009 {
            self.record_initializer_source(object_node, initializer);
            let identity = self.declaration_identity(object_node)?;
            self.declaration_statements.push((identity, statement));
            // A constructed object seeds from its declaring instance's
            // initialization RNG, as when the initializer was a process of
            // that instance (SV 18.14.1, S28-D4); class-scope properties
            // have no instance and keep the root stream.
            if !self.class_nodes.contains_key(&owner) {
                self.initializer_random_scopes
                    .insert(identity, format!("{}.initializer", self.source_path(path)));
            }
            return Ok(());
        }
        let index = processes.len();
        let c_name = format!("p_{}_{kind}_init_{index}", self.c_path_ident(path));
        let label = format!("{}.{kind}_initializer.{index}", self.source_path(path));
        processes.push(IrProcess::new_with_origin(
            c_name,
            label,
            IrShape::RunOnce,
            Vec::new(),
            vec![statement],
            self.origin(object_node),
        ));
        Ok(())
    }

    pub(in super::super) fn emit_class_object_initializers(&mut self) -> Result<(), String> {
        let initializers = std::mem::take(&mut self.class_object_initializers);
        let mut processes = Vec::new();
        for (object_node, object, initializer, path) in initializers {
            self.schedule_handle_initializer(
                "class",
                object_node,
                initializer,
                &path,
                &mut |cg, path| {
                    Ok(IrObjectStmt::ChandleAssign(
                        object,
                        cg.lower_chandle(path, initializer)?,
                    ))
                },
                &mut processes,
            )?;
        }
        self.model.processes.splice(0..0, processes);
        Ok(())
    }

    /// Semaphore constructors: `new(...)` is a runtime operation and cannot
    /// appear in a C static initializer.
    pub(in super::super) fn emit_semaphore_initializers(&mut self) -> Result<(), String> {
        let initializers = std::mem::take(&mut self.semaphore_initializers);
        let mut processes = Vec::new();
        for (object_node, object, initializer, path) in initializers {
            self.schedule_handle_initializer(
                "semaphore",
                object_node,
                initializer,
                &path,
                &mut |cg, path| {
                    Ok(IrObjectStmt::ChandleAssign(
                        object,
                        cg.lower_chandle(path, initializer)?,
                    ))
                },
                &mut processes,
            )?;
        }
        self.model.processes.splice(0..0, processes);
        Ok(())
    }

    /// Mailbox constructors build into the runtime-owned mailbox registry.
    pub(in super::super) fn emit_mailbox_object_initializers(&mut self) -> Result<(), String> {
        let initializers = std::mem::take(&mut self.mailbox_object_initializers);
        let mut processes = Vec::new();
        for (object_node, object, initializer, path) in initializers {
            self.schedule_handle_initializer(
                "mailbox",
                object_node,
                initializer,
                &path,
                &mut |cg, path| {
                    let element = cg.mailbox_element_for_decl(object_node);
                    Ok(IrObjectStmt::MailboxAssign(
                        object,
                        cg.lower_mailbox_expr(path, initializer, element)?,
                    ))
                },
                &mut processes,
            )?;
        }
        self.model.processes.splice(0..0, processes);
        Ok(())
    }

    /// Module, package and `$unit` string values that call a subroutine,
    /// lowered once every prototype exists and run at their declaration's
    /// slot of the static schedule.
    pub(in super::super) fn emit_string_object_initializers(&mut self) -> Result<(), String> {
        for (declaration, object, initializer, path) in
            std::mem::take(&mut self.string_object_initializers)
        {
            let owner = self.initializer_environment(declaration).ok_or_else(|| {
                format!(
                    "string initializer for `{}` has no owning module, interface, package or class",
                    self.node(declaration).full_name
                )
            })?;
            self.inst = owner;
            self.depth_arg = "0".into();
            let value = self.lower_string(&path, initializer)?;
            self.model.objects[object].initial = Some(value);
            self.record_initializer_source(declaration, initializer);
            let identity = self.declaration_identity(declaration)?;
            self.object_initializers.push((object, identity));
        }
        Ok(())
    }

    /// Static subroutine records with native members: one record assignment
    /// at the declaration's slot of the static schedule.
    pub(in super::super) fn emit_native_record_initializers(&mut self) -> Result<(), String> {
        for (declaration, value, initializer, path) in
            std::mem::take(&mut self.native_record_initializers)
        {
            let owner = self.initializer_environment(declaration).ok_or_else(|| {
                format!(
                    "record initializer for `{}` has no owning module, interface, package or class",
                    self.node(declaration).full_name
                )
            })?;
            self.inst = owner;
            self.depth_arg = "0".into();
            // Subroutine native locals are bound only while their body
            // lowers; bind this declaration to its persistent value again.
            let previous = self.native_roots.insert(declaration, value);
            let statement = self.lower_native_value_assignment(
                &path,
                declaration,
                initializer,
                false,
                Operation::Assignment,
            );
            match previous {
                Some(previous) => self.native_roots.insert(declaration, previous),
                None => self.native_roots.remove(&declaration),
            };
            let statement = statement?.ok_or_else(|| {
                format!(
                    "initializer of static record `{}` in `{path}` has no record assignment",
                    self.node(declaration).name
                )
            })?;
            self.record_initializer_source(declaration, initializer);
            let identity = self.declaration_identity(declaration)?;
            self.declaration_statements.push((identity, statement));
        }
        Ok(())
    }
}
