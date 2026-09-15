//! Initialization.

use super::*;

impl<'a> Codegen<'a> {
    pub(in super::super) fn emit_array_initializers(&mut self) -> Result<(), String> {
        let initializers = std::mem::take(&mut self.array_initializers);
        let mut processes = Vec::with_capacity(initializers.len());
        for (index, (array, initializer)) in initializers.into_iter().enumerate() {
            let inst = self.owning_inst(array).ok_or_else(|| {
                format!(
                    "fixed-array initializer for `{}` has no owning instance",
                    self.node(array).name
                )
            })?;
            self.inst = inst;
            let path = self.instance_path_of(inst);
            let body = self
                .lower_p30_fixed_array_assignment(
                    &path,
                    array,
                    initializer,
                    true,
                    Operation::Assignment,
                )?
                .ok_or_else(|| {
                    format!(
                        "fixed-array initializer for `{}` in `{path}` has no array target",
                        self.node(array).name
                    )
                })?;
            processes.push(IrProcess::new_with_origin(
                self.new_fn_name(&path, "array_init"),
                format!("{path}.array_initializer.{index}"),
                IrShape::RunOnce,
                Vec::new(),
                vec![body],
                self.origin(array),
            ));
        }
        self.model.processes.splice(0..0, processes);
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
