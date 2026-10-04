//! Descriptor-backed native values (SIM-003).

use super::*;

fn leaf_matches(element: &IrContainerElement, ty: IrClassFieldType) -> bool {
    match (element, ty) {
        (
            IrContainerElement::Packed {
                width,
                signed,
                two_state,
            },
            IrClassFieldType::Packed {
                width: leaf_width,
                signed: leaf_signed,
                two_state: leaf_two_state,
            },
        ) => *width == leaf_width && *signed == leaf_signed && *two_state == leaf_two_state,
        (IrContainerElement::Real { shortreal }, IrClassFieldType::Real { shortreal: leaf }) => {
            *shortreal == leaf
        }
        (IrContainerElement::String, IrClassFieldType::String)
        | (IrContainerElement::Chandle, IrClassFieldType::Chandle) => true,
        _ => false,
    }
}

impl Validator<'_> {
    /// Types, storage names and type references of every native value.
    pub(super) fn validate_native_tables(&self) -> ValidationResult {
        for (index, ty) in self.model.native_types.iter().enumerate() {
            validate_native_type(ty, &format!("native_types[{index}]"))?;
        }
        let mut names = HashSet::new();
        for (index, value) in self.model.native_values.iter().enumerate() {
            let path = format!("native_values[{index}]");
            if value.ty >= self.model.native_types.len() {
                return self.fail(path, "native value type is out of bounds");
            }
            if !value.activation && (value.c_name.is_empty() || !names.insert(&value.c_name)) {
                return self.fail(path, "persistent native value needs a unique storage name");
            }
        }
        for (index, function) in self.model.funcs.iter().enumerate() {
            for (formal_index, formal) in function.formals.iter().enumerate() {
                let Some(value) = formal.native_value else {
                    continue;
                };
                let path = format!("funcs[{index}].formals[{formal_index}]");
                if value >= self.model.native_values.len() {
                    return self.fail(path, "native formal storage is out of bounds");
                }
                if formal.is_ref()
                    || formal.fixed_array.is_some()
                    || formal.string
                    || formal.chandle
                    || formal.real
                    || formal.event
                    || formal.width != 0
                {
                    return self.fail(path, "native formal has a conflicting value ABI");
                }
            }
        }
        Ok(())
    }

    /// A native value used here must be persistent, a formal of the function
    /// being validated or declared in an enclosing lexical scope.
    pub(super) fn validate_native_value_use(&self, index: usize, path: &str) -> ValidationResult {
        let Some(value) = self.model.native_values.get(index) else {
            return self.fail(path, "native value reference is out of bounds");
        };
        if value.activation
            && !self.function.get().is_some_and(|function| {
                function
                    .formals
                    .iter()
                    .any(|formal| formal.native_value == Some(index))
            })
            && !self
                .native_activations
                .borrow()
                .iter()
                .any(|scope| scope.contains(&index))
        {
            return self.fail(path, "native value is used outside its declaration scope");
        }
        Ok(())
    }

    /// Leaf values of a call-built native value: distinct paths selecting
    /// leaves whose scalar kind matches the value.
    pub(super) fn validate_native_leaf_values(
        &self,
        ty: usize,
        leaves: &[IrNativeLeafValue],
        formals: &[IrFormal],
        path: &str,
    ) -> ValidationResult {
        let Some(root) = self.model.native_types.get(ty) else {
            return self.fail(path, "native leaf operand type is out of bounds");
        };
        let mut seen = HashSet::new();
        for (index, leaf) in leaves.iter().enumerate() {
            let leaf_path = format!("{path}.leaves[{index}]");
            if !seen.insert(leaf.items.as_slice()) {
                return self.fail(leaf_path, "native leaf is initialized twice");
            }
            let matches = match (native_leaf_type(root, &leaf.items), &leaf.value) {
                (
                    Some(IrContainerElement::Packed { .. } | IrContainerElement::Real { .. }),
                    IrNativeLeafExpr::Packed(value) | IrNativeLeafExpr::Real(value),
                ) => {
                    self.validate_expr(value, formals, &leaf_path)?;
                    true
                }
                (Some(IrContainerElement::String), IrNativeLeafExpr::String(value)) => {
                    value.validate(self.model, self.string_return.get())?;
                    true
                }
                (Some(IrContainerElement::Chandle), IrNativeLeafExpr::Chandle(value)) => {
                    value.validate(self.model, formals, self.chandle_return.get())?;
                    true
                }
                _ => false,
            };
            if !matches {
                return self.fail(leaf_path, "native leaf value disagrees with its type");
            }
        }
        Ok(())
    }

    /// The item path of a value leaf must select a leaf of the declared type.
    /// Lexical scope is checked where the leaf is read or written, because
    /// accesses are model-level names resolved in the using frame.
    pub(super) fn validate_native_leaf(
        &self,
        value: usize,
        item_path: &[u32],
        ty: IrClassFieldType,
        path: &str,
    ) -> ValidationResult {
        let Some(storage) = self.model.native_values.get(value) else {
            return self.fail(path, "native value item references missing storage");
        };
        let Some(root) = self.model.native_types.get(storage.ty) else {
            return self.fail(path, "native value item references a missing type");
        };
        if item_path.is_empty()
            || !native_leaf_type(root, item_path).is_some_and(|leaf| leaf_matches(leaf, ty))
        {
            return self.fail(
                path,
                "native value item path does not select a leaf of that type",
            );
        }
        Ok(())
    }
}
