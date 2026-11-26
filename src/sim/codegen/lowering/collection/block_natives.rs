//! Strings, chandles, class handles and virtual interfaces declared in
//! procedural blocks.
//!
//! Each declaration of each instance or generate scope owns one model
//! object, exactly like a module-scope string or handle, so reads, writes,
//! methods, calls, comparisons and change markers (`@(h)`, always_comb
//! sensitivity) apply unchanged. Static declarations keep their value and
//! initialize once in the static schedule (SV 6.21, 10.5); automatic ones
//! are reset to their default (empty string, null) and initialized at every
//! entry of their block. When a fork could keep two activations live (see
//! `concurrent_block_activation`), an automatic declaration is instead a
//! local of each activation, which fork branches share.
use super::*;
use crate::sim::ir::IrObject;

impl Codegen<'_> {
    /// The object kind of a block declaration with string, chandle,
    /// virtual-interface or user class-handle storage. The built-in
    /// semaphore, mailbox and process classes keep their own paths.
    pub(super) fn block_native_type(&self, declaration: NodeId) -> Option<IrObjectType> {
        let NodeKind::Var { ty } = self.kind(declaration) else {
            return None;
        };
        match ty.kind.as_str() {
            "string" => Some(IrObjectType::String),
            "chandle" | "virtual_interface" => Some(IrObjectType::Chandle),
            "class"
                if !matches!(ty.type_name.as_deref(), Some("process" | "semaphore"))
                    && !self.is_mailbox_expr("", declaration) =>
            {
                Some(IrObjectType::Chandle)
            }
            _ => None,
        }
    }

    /// Give one block declaration its model object. `statement` is its
    /// declaration statement, which carries the lexical position.
    pub(in super::super) fn collect_block_native(
        &mut self,
        path: &str,
        declaration: NodeId,
        statement: NodeId,
    ) -> Result<(), String> {
        if let Some(owner) = self.block_natives.get(&declaration) {
            if owner != path {
                return Err(format!(
                    "procedural-block variable `{}` is shared by `{owner}` and `{path}`",
                    self.node(declaration).name
                ));
            }
            return Ok(());
        }
        let ty = self.block_native_type(declaration).ok_or_else(|| {
            format!(
                "procedural-block variable `{}` in `{path}` has no string or handle type",
                self.node(declaration).name
            )
        })?;
        let name = self.node(declaration).name.clone();
        match self.db.variable_lifetime(declaration) {
            VariableLifetime::Unavailable => {
                return Err(format!(
                    "resolved lifetime is unavailable for procedural-block variable `{name}` in `{path}`"
                ))
            }
            VariableLifetime::Automatic
                if self.concurrent_block_activation(statement, declaration) =>
            {
                // One model object cannot hold two live activations: the
                // variable is a local of the process (or branch) activation
                // that declares it, which fork branches share like an
                // integral automatic.
                let local = format!("_lbn{}", declaration.index());
                match ty {
                    IrObjectType::String => self.proc_string_locals.insert(declaration, local),
                    _ => self.proc_semaphore_locals.insert(declaration, local),
                };
                self.block_natives.insert(declaration, path.to_owned());
                self.block_activation_locals.insert(declaration, statement);
                return Ok(());
            }
            VariableLifetime::Automatic => {}
            VariableLifetime::Static => {
                if let Some(initializer) = self.db.var_initializer(declaration) {
                    self.reserve_initializer_order(declaration);
                    self.block_native_initializers
                        .push((declaration, initializer, path.to_owned()));
                }
            }
        }
        let scope = self.block_storage_scope(path, declaration);
        let index = self.model.objects.len();
        self.model.objects.push(IrObject {
            c_name: self.c_name("O", &scope, &[&name]),
            ty,
            initial: None,
        });
        // Only the declaration identity resolves this object: a scope name
        // entry would make the block variable visible to the whole scope.
        self.object_globals.insert(declaration, index);
        self.block_natives.insert(declaration, path.to_owned());
        Ok(())
    }

    /// A named event declared in a procedural block: one synchronization
    /// object per declaration and instance, like a module-scope event. An
    /// event holds no value, so automatic declarations need no reset at
    /// entry; one that can be live in two activations is a local object of
    /// each activation instead.
    pub(super) fn collect_block_event(
        &mut self,
        path: &str,
        declaration: NodeId,
        statement: NodeId,
    ) -> Result<(), String> {
        if self.event_globals.contains_key(&declaration) {
            return Ok(());
        }
        if self.local_event_handles.contains_key(&declaration) {
            return Ok(());
        }
        let name = self.node(declaration).name.clone();
        if self.db.variable_lifetime(declaration) == VariableLifetime::Automatic
            && self.concurrent_block_activation(statement, declaration)
        {
            // Like a subroutine event: a fresh synchronization object at
            // each entry of the declaring activation, which a fork captures
            // by identity.
            self.local_event_handles
                .insert(declaration, format!("_llg_event_{}", declaration.index()));
            self.block_activation_locals.insert(declaration, statement);
            return Ok(());
        }
        let scope = self.block_storage_scope(path, declaration);
        let global = self.event_global_name(&scope, &name);
        if self.db.variable_lifetime(declaration) == VariableLifetime::Automatic {
            let info = self.new_event_info(global);
            self.event_globals.insert(declaration, info);
        } else {
            self.collect_event_global(path, declaration, global);
        }
        Ok(())
    }

    pub(in super::super) fn is_block_native(&self, declaration: NodeId) -> bool {
        self.block_natives.contains_key(&declaration)
    }

    /// The statements of a procedural-block string or handle declaration,
    /// or `None` for any other declaration. Static storage initializes in
    /// the static schedule; automatic storage is reset and initialized at
    /// each entry.
    pub(in super::super) fn lower_block_native_declaration(
        &mut self,
        path: &str,
        declaration: NodeId,
    ) -> Result<Option<Vec<IrStmt>>, String> {
        if !self.is_block_native(declaration) {
            return Ok(None);
        }
        if self.db.variable_lifetime(declaration) != VariableLifetime::Automatic {
            return Ok(Some(Vec::new()));
        }
        let initializer = self.db.var_initializer(declaration);
        let Some(&object) = self.object_globals.get(&declaration) else {
            return self
                .block_native_local_declaration(path, declaration, initializer)
                .map(Some);
        };
        let mut statements = Vec::with_capacity(2);
        // A whole initializer replaces the value; it sees the default only
        // when it reads the variable itself.
        if initializer.is_none_or(|value| self.references_declaration(value, declaration)) {
            statements.push(IrStmt::Object(Box::new(
                match self.model.objects[object].ty {
                    IrObjectType::String => {
                        IrObjectStmt::StringAssign(object, IrStringExpr::Literal(Vec::new()))
                    }
                    _ => IrObjectStmt::ChandleAssign(object, IrChandleExpr::Null),
                },
            )));
        }
        if let Some(initializer) = initializer {
            statements.push(self.block_native_assignment(path, declaration, initializer)?);
        }
        Ok(Some(statements))
    }

    /// The declaration of a block string or handle that lives in its
    /// activation (see `collect_block_native`): a fresh local at each entry,
    /// in a shared frame when a fork branch uses it (SV 6.21, 9.3.2).
    fn block_native_local_declaration(
        &mut self,
        path: &str,
        declaration: NodeId,
        initializer: Option<NodeId>,
    ) -> Result<Vec<IrStmt>, String> {
        if let Some(name) = self.proc_string_local_name(declaration).map(str::to_owned) {
            let init = initializer
                .map(|initializer| self.lower_string(path, initializer))
                .transpose()?;
            if self.fork_shared(declaration) {
                self.shared_locals.insert(declaration);
                let observed = self.native_observed(declaration, self.inst);
                return Ok(vec![IrStmt::SharedString {
                    name,
                    init,
                    observed,
                }]);
            }
            return Ok(vec![IrStmt::DeclString { name, init }]);
        }
        let name = self
            .proc_semaphore_local_name(declaration)
            .map(str::to_owned)
            .ok_or_else(|| {
                format!(
                    "procedural-block variable `{}` in `{path}` has no storage",
                    self.node(declaration).name
                )
            })?;
        let init = initializer
            .map(|initializer| self.lower_chandle(path, initializer))
            .transpose()?;
        Ok(vec![IrStmt::Object(Box::new(
            if self.fork_shared(declaration) {
                self.shared_locals.insert(declaration);
                IrObjectStmt::ChandleDeclareShared(name, init)
            } else {
                IrObjectStmt::ChandleDeclareLocal(name, init)
            },
        ))])
    }

    fn block_native_assignment(
        &mut self,
        path: &str,
        declaration: NodeId,
        initializer: NodeId,
    ) -> Result<IrStmt, String> {
        self.lower_object_assignment(path, declaration, initializer, true, Operation::Assignment)?
            .ok_or_else(|| {
                format!(
                    "initializer of procedural-block variable `{}` in `{path}` has no assignment",
                    self.node(declaration).name
                )
            })
    }

    /// Static block string and handle initializers run once in the static
    /// initialization schedule, after every body has lowered.
    pub(in super::super) fn emit_block_native_initializers(&mut self) -> Result<(), String> {
        let initializers = std::mem::take(&mut self.block_native_initializers);
        for (declaration, initializer, path) in initializers {
            let inst = self.initializer_environment(declaration).ok_or_else(|| {
                format!(
                    "initializer for `{}` has no owning instance",
                    self.node(declaration).name
                )
            })?;
            self.inst = inst;
            self.depth_arg = "0".into();
            let statement = self.block_native_assignment(&path, declaration, initializer)?;
            self.record_initializer_source(declaration, initializer);
            let identity = self.declaration_identity(declaration)?;
            self.declaration_statements.push((identity, statement));
        }
        Ok(())
    }
}
