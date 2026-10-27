//! Dependencies.

use super::*;

impl Validator<'_> {
    pub(super) fn new(model: &IrModel) -> Validator<'_> {
        Validator {
            model,
            max_width: Cell::new(0),
            reduction_bindings: RefCell::new(Vec::new()),
            fixed_activations: RefCell::new(Vec::new()),
            native_activations: RefCell::new(Vec::new()),
            chandle_return: Cell::new(None),
            string_return: Cell::new(None),
            function: Cell::new(None),
            signal_names: std::cell::OnceCell::new(),
            future_slots: Cell::new(None),
        }
    }

    /// Positions of the signals whose storage name is `name`, ascending.
    fn signals_named(&self, name: &str) -> &[usize] {
        let names = self.signal_names.get_or_init(|| {
            let mut names = std::collections::HashMap::<&str, Vec<usize>>::new();
            for (index, signal) in self.model.signals.iter().enumerate() {
                names.entry(signal.c_name.as_str()).or_default().push(index);
            }
            names
        });
        names.get(name).map_or(&[], Vec::as_slice)
    }

    /// The aliased signal whose alias-visible cell `name` spells exactly
    /// (`llg_net_alias_{index}.visible`).
    fn alias_signal(&self, name: &str) -> Option<usize> {
        let digits = name
            .strip_prefix("llg_net_alias_")?
            .strip_suffix(".visible")?;
        let index = digits.parse::<usize>().ok()?;
        (index.to_string() == digits
            && self
                .model
                .signals
                .get(index)
                .is_some_and(|signal| !signal.net_alias.is_empty()))
        .then_some(index)
    }

    pub(super) fn validate_fixed_activation(&self, index: usize, path: &str) -> ValidationResult {
        if self
            .model
            .arrays
            .get(index)
            .is_some_and(|array| array.activation)
            && !self.function.get().is_some_and(|function| {
                function.formals.iter().any(|formal| {
                    formal.fixed_array == Some(index) || formal.real_array == Some(index)
                })
            })
            && !self
                .fixed_activations
                .borrow()
                .iter()
                .any(|scope| scope.contains(&index))
        {
            return self.fail(path, "fixed activation is outside its declaration scope");
        }
        Ok(())
    }

    pub(super) fn valid_dependency(&self, dependency: &IrDependency) -> bool {
        match dependency {
            IrDependency::PackedRange {
                storage,
                lsb,
                width,
            } => {
                let total = match storage.as_ref() {
                    // The first signal named by its storage or alias name.
                    IrDependency::Scalar(name) => self
                        .signals_named(name)
                        .first()
                        .copied()
                        .into_iter()
                        .chain(self.alias_signal(name))
                        .min()
                        .map(|index| self.model.signals[index].ty.width()),
                    IrDependency::ArrayElement { array, .. } => self
                        .model
                        .arrays
                        .get(*array)
                        .filter(|array| !array.real)
                        .map(|array| array.elem_width),
                    _ => None,
                };
                self.valid_dependency(storage)
                    && *width != 0
                    && total.is_some_and(|total| {
                        lsb.checked_add(*width).is_some_and(|end| end <= total)
                    })
            }
            IrDependency::Scalar(name) => {
                let alias_index = name
                    .strip_prefix("llg_net_alias_")
                    .and_then(|name| name.strip_suffix(".visible"))
                    .and_then(|index| index.parse::<usize>().ok())
                    .filter(|index| {
                        self.model
                            .signals
                            .get(*index)
                            .is_some_and(|signal| !signal.net_alias.is_empty())
                    });
                self.signals_named(name)
                    .iter()
                    .copied()
                    .chain(alias_index)
                    .any(|index| {
                        let signal = &self.model.signals[index];
                        !signal.omit && signal.ty.width() != 0
                    })
            }
            IrDependency::Real(name) => self.signals_named(name).iter().any(|index| {
                let signal = &self.model.signals[*index];
                !signal.omit && matches!(signal.ty, IrType::Real { .. })
            }),
            IrDependency::ArrayElement { array, index } => self
                .model
                .arrays
                .get(*array)
                .is_some_and(|array| *index < array.total),
            IrDependency::ArrayContents(array) => *array < self.model.arrays.len(),
            IrDependency::ContainerContents(container)
            | IrDependency::ContainerShape(container) => *container < self.model.containers.len(),
            IrDependency::SharedCell { local, .. } | IrDependency::SharedNative { local, .. } => {
                !local.is_empty()
            }
            IrDependency::RefFormal { .. } | IrDependency::NativeRefFormal { .. } => true,
            IrDependency::NativeAccess(name) => self
                .model
                .native_accesses
                .iter()
                .find(|access| access.name == *name)
                .is_some_and(|access| match access.kind {
                    crate::sim::ir::IrNativeAccessKind::ClassField { class, field } => self
                        .model
                        .classes
                        .get(class)
                        .and_then(|class| class.fields.get(field))
                        .is_some_and(|field| {
                            field.container.is_none()
                                && field.native_value.is_none()
                                && matches!(
                                    field.ty,
                                    IrClassFieldType::Packed { .. }
                                        | IrClassFieldType::Real { .. }
                                        | IrClassFieldType::String
                                        | IrClassFieldType::Chandle
                                )
                        }),
                    crate::sim::ir::IrNativeAccessKind::InterfaceMember { .. } => true,
                    _ => false,
                }),
            IrDependency::Object(object) => self
                .model
                .objects
                .get(*object)
                .is_some_and(|object| object.ty.has_change_marker()),
            IrDependency::InterfaceInstance { .. } => {
                crate::sim::execution::valid_interface_instance(self.model, dependency)
            }
        }
    }

    pub(super) fn validate_event_ref(
        &self,
        event: &IrEventRef,
        formals: &[IrFormal],
        path: &str,
    ) -> ValidationResult {
        match event {
            IrEventRef::Null => Ok(()),
            IrEventRef::Static(index) => {
                let Some(descriptor) = self.model.events.get(*index) else {
                    return self.fail(path, "event index is out of bounds");
                };
                if descriptor.is_array() {
                    return self.fail(path, "event array descriptor cannot be used as a handle");
                }
                Ok(())
            }
            IrEventRef::Array { array, indices } => {
                let Some(descriptor) = self.model.events.get(*array) else {
                    return self.fail(path, "event array index is out of bounds");
                };
                let Some(dims) = descriptor.array_dims() else {
                    return self.fail(path, "event handle references a non-array descriptor");
                };
                if dims.len() != indices.len() {
                    return self.fail(path, "event array index rank does not match dimensions");
                }
                for element in descriptor.array_elements() {
                    let Some(handle) = self.model.events.get(*element) else {
                        return self.fail(path, "event array element index is out of bounds");
                    };
                    if handle.is_array() {
                        return self
                            .fail(path, "event array element cannot be an array descriptor");
                    }
                }
                for (index, expression) in indices.iter().enumerate() {
                    self.validate_expr(expression, formals, &format!("{path}.indices[{index}]"))?;
                }
                Ok(())
            }
            IrEventRef::Captured(name) => {
                if name.is_empty() {
                    return self.fail(path, "captured event handle name must not be empty");
                }
                Ok(())
            }
            IrEventRef::Handle(handle) => {
                handle
                    .validate(self.model, formals, self.chandle_return.get())
                    .map_err(|error| IrValidationError::new(path.to_owned(), error.to_string()))?;
                let mut indices = Vec::new();
                handle.expressions(&mut |expression| indices.push(expression.clone()));
                for (index, expression) in indices.iter().enumerate() {
                    self.validate_expr(expression, formals, &format!("{path}.handle[{index}]"))?;
                }
                Ok(())
            }
            IrEventRef::Formal(index) => {
                if formals.get(*index).is_some_and(|formal| formal.event) {
                    Ok(())
                } else {
                    self.fail(path, "event formal reference requires an event formal")
                }
            }
        }
    }

    pub(super) fn validate_plusarg_text(
        &self,
        text: &IrPlusArgText,
        formals: &[IrFormal],
        path: &str,
    ) -> ValidationResult {
        match text {
            IrPlusArgText::Literal(text) => {
                if text.contains('\0') {
                    return self.fail(path, "plusarg text contains NUL");
                }
            }
            IrPlusArgText::Dynamic(value) => {
                value
                    .validate(self.model, self.string_return.get())
                    .map_err(|error| IrValidationError::new(path, error.to_string()))?;
                let mut result = Ok(());
                value.expressions(&mut |expression| {
                    if result.is_ok() {
                        result = self.validate_expr(expression, formals, path);
                    }
                });
                result?;
            }
            IrPlusArgText::Packed(value) => {
                self.validate_expr(value, formals, path)?;
                if value.is_real() {
                    return self.fail(path, "packed scan text must be integral");
                }
            }
        }
        Ok(())
    }
}
