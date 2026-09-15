//! Dependencies.

use super::*;

impl Validator<'_> {
    pub(super) fn new(model: &IrModel) -> Validator<'_> {
        Validator {
            model,
            max_width: Cell::new(0),
            chandle_return: Cell::new(None),
            string_return: Cell::new(None),
        }
    }

    pub(super) fn valid_dependency(&self, dependency: &IrDependency) -> bool {
        match dependency {
            IrDependency::PackedRange { storage, lsb, width } => {
                let total = match storage.as_ref() {
                    IrDependency::Scalar(name) => self.model.signals.iter().enumerate().find_map(|(index, signal)| {
                        let alias = format!("llg_net_alias_{index}.visible");
                        (signal.c_name == *name || (!signal.net_alias.is_empty() && alias == *name)).then_some(signal.ty.width())
                    }),
                    IrDependency::ArrayElement { array, .. } => self.model.arrays.get(*array)
                        .filter(|array| !array.real).map(|array| array.elem_width),
                    _ => None,
                };
                self.valid_dependency(storage) && *width != 0 && total.is_some_and(|total|
                    lsb.checked_add(*width).is_some_and(|end| end <= total))
            }
            IrDependency::Scalar(name) => {
                let alias_index = name
                    .strip_prefix("llg_net_alias_")
                    .and_then(|name| name.strip_suffix(".visible"))
                    .and_then(|index| index.parse::<usize>().ok());
                self.model
                    .signals
                    .iter()
                    .enumerate()
                    .any(|(index, signal)| {
                        (signal.c_name == *name
                            || (alias_index == Some(index) && !signal.net_alias.is_empty()))
                            && !signal.omit
                            && signal.ty.width() != 0
                    })
            }
            IrDependency::Real(name) => self.model.signals.iter().any(|signal| {
                signal.c_name == *name && !signal.omit && matches!(signal.ty, IrType::Real { .. })
            }),
            IrDependency::ArrayElement { array, index } => self
                .model
                .arrays
                .get(*array)
                .is_some_and(|array| *index < array.total),
            IrDependency::ArrayContents(array) => *array < self.model.arrays.len(),
            IrDependency::ContainerContents(container)
            | IrDependency::ContainerShape(container) => *container < self.model.containers.len(),
            IrDependency::Object(object) => self
                .model
                .objects
                .get(*object)
                .is_some_and(|object| object.ty == crate::sim::ir::IrObjectType::String),
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
        }
        Ok(())
    }
}
