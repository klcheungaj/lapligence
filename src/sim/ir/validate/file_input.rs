//! File input.

use super::*;

impl Validator<'_> {
    pub(super) fn validate_file_input(
        &self,
        input: &IrFileInput,
        formals: &[IrFormal],
        path: &str,
    ) -> ValidationResult {
        let validate_descriptor = |value: &IrExpr, name: &str| {
            self.validate_expr(value, formals, &format!("{path}.{name}"))?;
            if value.is_real() {
                return self.fail(
                    format!("{path}.{name}"),
                    "file descriptor must be a packed expression",
                );
            }
            Ok(())
        };
        let validate_target = |target: &IrFileInputTarget, target_path: &str| {
            match target {
                IrFileInputTarget::Packed {
                    lhs,
                    width,
                    signed,
                    two_state,
                } => {
                    self.validate_lhs(lhs, formals, target_path)?;
                    if *width == 0
                        || self.lhs_packed_width(lhs) != Some(*width)
                        || lhs_signed(self.model, lhs) != Some(*signed)
                        || lhs_two_state(self.model, lhs) != Some(*two_state)
                    {
                        return self.fail(
                            target_path,
                            "file scan packed target type disagrees with its lvalue",
                        );
                    }
                }
                IrFileInputTarget::Real { lhs, .. } => {
                    self.validate_lhs(lhs, formals, target_path)?;
                    if !lhs_is_real(self.model, lhs) {
                        return self.fail(target_path, "file scan real target is not real storage");
                    }
                }
                IrFileInputTarget::String { address } => {
                    if address.is_empty() {
                        return self.fail(
                            target_path,
                            "file scan string target address must not be empty",
                        );
                    }
                }
                IrFileInputTarget::Element {
                    read,
                    width,
                    signed,
                    two_state,
                } => {
                    self.validate_expr(read, formals, target_path)?;
                    let container = match read.kind() {
                        IrExprKind::Container(operation) => match operation.as_ref() {
                            IrContainerExpr::Get { container, .. }
                            | IrContainerExpr::GetString { container, .. } => {
                                self.model.containers.get(*container)
                            }
                            _ => None,
                        },
                        _ => None,
                    };
                    let matches = container.is_some_and(|container| {
                        container.element
                            == IrContainerElement::Packed {
                                width: *width,
                                signed: *signed,
                                two_state: *two_state,
                            }
                    });
                    if *width == 0 || !matches {
                        return self.fail(
                            target_path,
                            "file scan element target must read a packed container element of its type",
                        );
                    }
                }
            }
            Ok(())
        };
        let validate_read_target = |target: &IrFileReadTarget, target_path: &str| {
            match target {
                IrFileReadTarget::Packed {
                    lhs,
                    width,
                    signed,
                    two_state,
                } => {
                    self.validate_lhs(lhs, formals, target_path)?;
                    if *width == 0
                        || self.lhs_packed_width(lhs) != Some(*width)
                        || lhs_signed(self.model, lhs) != Some(*signed)
                        || lhs_two_state(self.model, lhs) != Some(*two_state)
                    {
                        return self.fail(
                            target_path,
                            "file read packed target type disagrees with its lvalue",
                        );
                    }
                }
                IrFileReadTarget::Array { array } => {
                    let Some(array) = self.model.arrays.get(*array) else {
                        return self.fail(target_path, "file read array index is out of bounds");
                    };
                    if array.real || array.elem_width == 0 || array.total == 0 {
                        return self
                            .fail(target_path, "file read array must contain packed elements");
                    }
                }
                IrFileReadTarget::Element {
                    read,
                    width,
                    signed,
                    two_state,
                } => validate_target(
                    &IrFileInputTarget::Element {
                        read: read.clone(),
                        width: *width,
                        signed: *signed,
                        two_state: *two_state,
                    },
                    target_path,
                )?,
                IrFileReadTarget::Container { container } => {
                    let packed = self
                        .model
                        .containers
                        .get(*container)
                        .is_some_and(|container| {
                            matches!(container.element, IrContainerElement::Packed { .. })
                                && matches!(
                                    container.kind,
                                    IrContainerKind::Dynamic | IrContainerKind::Queue { .. }
                                )
                        });
                    if !packed {
                        return self.fail(
                            target_path,
                            "file read container must be a packed dynamic array or queue",
                        );
                    }
                }
            }
            Ok(())
        };
        match input {
            IrFileInput::Getc { descriptor } => validate_descriptor(descriptor, "descriptor")?,
            IrFileInput::Ungetc {
                character,
                descriptor,
            } => {
                validate_descriptor(character, "character")?;
                validate_descriptor(descriptor, "descriptor")?;
            }
            IrFileInput::Gets { descriptor, target } => {
                validate_descriptor(descriptor, "descriptor")?;
                validate_target(target, &format!("{path}.target"))?;
                if matches!(target, IrFileInputTarget::Real { .. }) {
                    return self.fail(path, "file line input target cannot be real storage");
                }
            }
            IrFileInput::ScanFile {
                descriptor,
                format,
                targets,
                ..
            } => {
                validate_descriptor(descriptor, "descriptor")?;
                self.validate_plusarg_text(format, formals, &format!("{path}.format"))?;
                for (index, target) in targets.iter().enumerate() {
                    validate_target(target, &format!("{path}.targets[{index}]"))?;
                }
            }
            IrFileInput::ScanString {
                source,
                format,
                targets,
                ..
            } => {
                self.validate_plusarg_text(source, formals, &format!("{path}.source"))?;
                self.validate_plusarg_text(format, formals, &format!("{path}.format"))?;
                for (index, target) in targets.iter().enumerate() {
                    validate_target(target, &format!("{path}.targets[{index}]"))?;
                }
            }
            IrFileInput::Read {
                descriptor,
                target,
                start,
                count,
            } => {
                validate_descriptor(descriptor, "descriptor")?;
                validate_read_target(target, &format!("{path}.target"))?;
                for (name, value) in [("start", start), ("count", count)] {
                    if let Some(value) = value {
                        self.validate_expr(value, formals, &format!("{path}.{name}"))?;
                        if value.is_real() {
                            return self.fail(
                                format!("{path}.{name}"),
                                "file read bounds must be packed expressions",
                            );
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
