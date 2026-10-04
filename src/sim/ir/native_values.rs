//! Descriptor-backed native aggregate storage (SIM-003).
//!
//! Unpacked records and fixed arrays whose leaves include strings, chandles
//! or reals have no packed payload. Subroutine formals, results and locals of
//! such types hold one runtime `llg_value_t` described by a recursive type
//! from [`super::IrModel::native_types`]; leaves are addressed by item paths
//! through [`super::IrNativeAccessKind::ValueItem`]. These values are never
//! routed through `IrFixedValue`, whose element domain stays integral.
use super::*;

/// Deepest native type accepted, matching the runtime `LLG_VALUE_DESC_MAX_DEPTH`.
pub(in crate::sim) const LLG_MAX_NATIVE_TYPE_DEPTH: usize = 64;

/// Largest item count of one native aggregate or fixed-array level. Item
/// paths use `u32` indices and each item is one runtime `llg_value_t`.
pub(in crate::sim) const LLG_MAX_NATIVE_TYPE_ITEMS: u64 = 1 << 24;

/// One descriptor-backed native value slot.
#[derive(Clone, Debug, PartialEq)]
pub struct IrNativeValue {
    /// Global name of persistent storage; diagnostic only for activations.
    pub(in crate::sim) c_name: String,
    /// Index into [`super::IrModel::native_types`].
    pub(in crate::sim) ty: usize,
    /// Lexical activation storage created by [`super::IrStmt::NativeValueDeclare`]
    /// or bound to a subroutine formal; otherwise model-lifetime storage.
    pub(in crate::sim) activation: bool,
}

impl IrNativeValue {
    pub fn c_name(&self) -> &str {
        &self.c_name
    }
    pub fn ty(&self) -> usize {
        self.ty
    }
    pub fn is_activation(&self) -> bool {
        self.activation
    }
}

/// One scalar leaf value of a native record built at a call boundary.
#[derive(Clone, Debug, PartialEq)]
pub enum IrNativeLeafExpr {
    Packed(IrExpr),
    Real(IrExpr),
    String(IrStringExpr),
    Chandle(IrChandleExpr),
}

/// A leaf value and the item path it initializes.
#[derive(Clone, Debug, PartialEq)]
pub struct IrNativeLeafValue {
    pub(in crate::sim) items: Vec<u32>,
    pub(in crate::sim) value: IrNativeLeafExpr,
}

impl IrNativeLeafValue {
    pub(in crate::sim) fn expressions(&self, visit: &mut impl FnMut(&IrExpr)) {
        match &self.value {
            IrNativeLeafExpr::Packed(value) | IrNativeLeafExpr::Real(value) => visit(value),
            IrNativeLeafExpr::String(value) => value.expressions(visit),
            IrNativeLeafExpr::Chandle(value) => value.expressions(visit),
        }
    }

    pub(in crate::sim) fn expressions_mut(&mut self, visit: &mut impl FnMut(&mut IrExpr)) {
        match &mut self.value {
            IrNativeLeafExpr::Packed(value) | IrNativeLeafExpr::Real(value) => visit(value),
            IrNativeLeafExpr::String(value) => value.expressions_mut(visit),
            IrNativeLeafExpr::Chandle(value) => value.expressions_mut(visit),
        }
    }
}

/// Number of runtime items directly below one aggregate or fixed-array level.
pub(in crate::sim) fn native_item_count(element: &IrContainerElement) -> Option<u64> {
    match element {
        IrContainerElement::Aggregate { members, .. } => u64::try_from(members.len()).ok(),
        IrContainerElement::FixedArray { dimensions, .. } => {
            dimensions.iter().try_fold(1u64, |count, (left, right)| {
                count.checked_mul(
                    i64::from(*left)
                        .abs_diff(i64::from(*right))
                        .checked_add(1)?,
                )
            })
        }
        _ => None,
    }
}

/// Resolve an item path to its leaf type, if every step is in range.
pub(in crate::sim) fn native_leaf_type<'a>(
    root: &'a IrContainerElement,
    path: &[u32],
) -> Option<&'a IrContainerElement> {
    path.iter().try_fold(root, |level, index| match level {
        IrContainerElement::Aggregate { members, .. } => members
            .get(usize::try_from(*index).ok()?)
            .map(|member| member.element.as_ref()),
        IrContainerElement::FixedArray { element, .. } => {
            (u64::from(*index) < native_item_count(level)?).then_some(element.as_ref())
        }
        _ => None,
    })
}

/// Validate one native value type: an unpacked aggregate or fixed array whose
/// leaves are packed, real, string or chandle values, with nonempty levels,
/// representable nominal identities and bounded nesting and item counts.
pub(in crate::sim) fn validate_native_type(
    element: &IrContainerElement,
    path: &str,
) -> Result<(), IrValidationError> {
    if !matches!(
        element,
        IrContainerElement::Aggregate { .. } | IrContainerElement::FixedArray { .. }
    ) {
        return Err(IrValidationError::new(
            path,
            "native value type must be an unpacked aggregate or fixed array",
        ));
    }
    validate_native_level(element, path, 0)
}

fn validate_native_level(
    element: &IrContainerElement,
    path: &str,
    depth: usize,
) -> Result<(), IrValidationError> {
    if depth >= LLG_MAX_NATIVE_TYPE_DEPTH {
        return Err(IrValidationError::new(
            path,
            format!("native value type nests deeper than {LLG_MAX_NATIVE_TYPE_DEPTH} levels"),
        ));
    }
    match element {
        IrContainerElement::Packed { width, .. } => {
            if *width == 0 || *width > crate::sim::emit_c::LLG_MAX_WIDTH {
                return Err(IrValidationError::new(
                    path,
                    "native packed leaf width is outside the packed value range",
                ));
            }
            Ok(())
        }
        IrContainerElement::Real { .. }
        | IrContainerElement::String
        | IrContainerElement::Chandle => Ok(()),
        IrContainerElement::Aggregate { type_id, members } => {
            if members.is_empty() {
                return Err(IrValidationError::new(
                    path,
                    "native aggregate has no members",
                ));
            }
            // The runtime reserves nominal identity zero; emission offsets by one.
            if type_id.checked_add(1).is_none() {
                return Err(IrValidationError::new(
                    path,
                    "native aggregate type identity is not representable",
                ));
            }
            if members.len() as u64 > LLG_MAX_NATIVE_TYPE_ITEMS {
                return Err(IrValidationError::new(
                    path,
                    "native aggregate has too many members",
                ));
            }
            for (index, member) in members.iter().enumerate() {
                validate_native_level(
                    &member.element,
                    &format!("{path}.members[{index}]"),
                    depth + 1,
                )?;
            }
            Ok(())
        }
        IrContainerElement::FixedArray {
            dimensions,
            element: item,
        } => {
            if dimensions.is_empty() {
                return Err(IrValidationError::new(
                    path,
                    "native fixed array has no dimensions",
                ));
            }
            let count = native_item_count(element).ok_or_else(|| {
                IrValidationError::new(path, "native fixed-array item count overflows")
            })?;
            if count > LLG_MAX_NATIVE_TYPE_ITEMS {
                return Err(IrValidationError::new(
                    path,
                    format!(
                        "native fixed array has {count} items; the limit is {LLG_MAX_NATIVE_TYPE_ITEMS}"
                    ),
                ));
            }
            validate_native_level(item, &format!("{path}.element"), depth + 1)
        }
        IrContainerElement::Union { .. }
        | IrContainerElement::Container { .. }
        | IrContainerElement::Event
        | IrContainerElement::Opaque { .. } => Err(IrValidationError::new(
            path,
            "native value leaf kind has no descriptor-backed subroutine layout",
        )),
    }
}
