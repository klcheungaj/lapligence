//! Strings, chandles, class handles and virtual interfaces declared in
//! procedural blocks.
//!
//! Each declaration of each instance or generate scope owns one model
//! object, exactly like a module-scope string or handle, so reads, writes,
//! methods, calls, comparisons and change markers (`@(h)`, always_comb
//! sensitivity) apply unchanged. Static declarations keep their value and
//! initialize once in the static schedule (SV 6.21, 10.5); automatic ones
//! are reset to their default (empty string, null) and initialized at every
//! entry of their block, and reject when a fork could keep two activations
//! live (see `concurrent_block_activation`).
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
    pub(super) fn collect_block_native(
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
                return Err(format!(
                    "automatic variable `{name}` in `{path}` can be live in two activations through a `join_any`/`join_none` fork that runs again; this is not supported"
                ))
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
    /// entry; two live activations would share one object and reject.
    pub(super) fn collect_block_event(
        &mut self,
        path: &str,
        declaration: NodeId,
        statement: NodeId,
    ) -> Result<(), String> {
        if self.event_globals.contains_key(&declaration) {
            return Ok(());
        }
        let name = self.node(declaration).name.clone();
        if self.db.var_initializer(declaration).is_some() {
            return Err(format!(
                "procedural-block event `{name}` in `{path}` with an initializer is not supported"
            ));
        }
        if self.db.variable_lifetime(declaration) == VariableLifetime::Automatic
            && self.concurrent_block_activation(statement, declaration)
        {
            return Err(format!(
                "automatic event `{name}` in `{path}` can be live in two activations through a `join_any`/`join_none` fork that runs again; this is not supported"
            ));
        }
        let scope = self.block_storage_scope(path, declaration);
        let info = self.new_event_info(self.event_global_name(&scope, &name));
        self.event_globals.insert(declaration, info);
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
        let object = self.object_globals[&declaration];
        let initializer = self.db.var_initializer(declaration);
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
            let inst = self.owner_instance(declaration).ok_or_else(|| {
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
