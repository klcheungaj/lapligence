//! Initialization.

use super::*;

impl<'a> Codegen<'a> {
    pub(in super::super) fn emit_array_initializers(&mut self) -> Result<(), String> {
        let initializers = std::mem::take(&mut self.array_initializers);
        let mut descriptor_processes = Vec::new();
        for (declaration, initializer) in initializers {
            // Package and `$unit` declarations own their storage directly.
            let inst = self.owner_instance(declaration).ok_or_else(|| {
                format!(
                    "fixed initializer for `{}` has no owning instance",
                    self.node(declaration).name
                )
            })?;
            self.inst = inst;
            self.depth_arg = "0".into();
            let path = self.instance_path_of(inst);
            if self
                .array_globals
                .get(&declaration)
                .is_some_and(|array| self.model.arrays[array.ir].sparse())
            {
                // Descriptor storage initializes through the same typed pattern,
                // view and call transport as assignments; packed transport would
                // impose the packed width limit and constant expansion would scale
                // generated source with the element count.
                let body = self
                    .lower_p30_fixed_array_assignment(
                        &path,
                        declaration,
                        initializer,
                        true,
                        Operation::Assignment,
                    )?
                    .ok_or("descriptor initializer has no array assignment")?;
                // SystemVerilog static initialization precedes every process
                // (SV §10.5) and joins the declaration schedule; Verilog keeps
                // its active-region initialization process.
                if self.db.edition() == LanguageEdition::SystemVerilog2009 {
                    self.record_initializer_source(declaration, initializer);
                    let identity = self.declaration_identity(declaration)?;
                    self.declaration_statements.push((identity, body));
                    continue;
                }
                let index = descriptor_processes.len();
                descriptor_processes.push(IrProcess::new_with_origin(
                    format!(
                        "p_{}_descriptor_init_{index}",
                        ident(&self.model.design_name)
                    ),
                    format!("{}.descriptor_initializer.{index}", self.model.design_name),
                    IrShape::RunOnce,
                    Vec::new(),
                    vec![body],
                    self.origin(declaration),
                ));
                continue;
            }
            let target = self
                .fixed_storage_lhs(&path, declaration)?
                .ok_or("fixed initializer has no persistent target")?;
            let width = self
                .fixed_value_width(declaration)
                .ok_or("fixed initializer has no payload width")?;
            let initialization = self.lower_declaration_initializer(
                &path,
                declaration,
                initializer,
                IrInitTarget::Fixed(Box::new(target)),
                width,
                false,
                false,
                false,
            )?;
            self.declaration_inits.push(initialization);
        }
        self.model.processes.splice(0..0, descriptor_processes);
        self.emit_record_initializers()
    }

    /// Static column-layout record initializers run column by column in the
    /// static initialization schedule (SV 6.21, 10.5), never as one packed
    /// payload.
    fn emit_record_initializers(&mut self) -> Result<(), String> {
        let initializers = std::mem::take(&mut self.record_initializers);
        for (declaration, initializer) in initializers {
            let name = self.node(declaration).name.clone();
            let inst = self
                .owner_instance(declaration)
                .ok_or_else(|| format!("record initializer for `{name}` has no owning instance"))?;
            self.inst = inst;
            self.depth_arg = "0".into();
            let path = self.instance_path_of(inst);
            if self.db.edition() != LanguageEdition::SystemVerilog2009 {
                return Err(format!(
                    "initializer of column-layout record `{name}` in `{path}` requires SystemVerilog"
                ));
            }
            let value = self.record_declaration_value(declaration).ok_or_else(|| {
                format!("column-layout record `{name}` in `{path}` has no columns")
            })?;
            let body = self.lower_record_initializer(&path, &value, initializer)?;
            self.record_initializer_source(declaration, initializer);
            let identity = self.declaration_identity(declaration)?;
            self.declaration_statements
                .push((identity, IrStmt::Block(body)));
        }
        Ok(())
    }

    pub(in super::super) fn emit_container_initializers(&mut self) -> Result<(), String> {
        let initializers = std::mem::take(&mut self.container_initializers);
        let mut processes = Vec::with_capacity(initializers.len());
        for (index, (owner, container)) in initializers.into_iter().enumerate() {
            let initializer = self
                .db
                .array_meta(owner)
                .and_then(|meta| meta.initializer())
                .ok_or_else(|| {
                    format!(
                        "container initializer for `{}` disappeared before lowering",
                        self.node(owner).full_name()
                    )
                })?;
            let descriptor = self.db.type_descriptor(owner).cloned();
            let path = self.node(owner).full_name();
            let body =
                self.lower_container_pattern(path, container, initializer, descriptor.as_ref())?;
            let name = format!(
                "p_{}_container_init_{index}",
                ident(&self.model.design_name)
            );
            let label = format!("{}.container_initializer.{index}", self.model.design_name);
            processes.push(IrProcess::new_with_origin(
                name,
                label,
                IrShape::RunOnce,
                Vec::new(),
                vec![body],
                self.origin(owner),
            ));
        }
        self.model.processes.splice(0..0, processes);
        Ok(())
    }
}
