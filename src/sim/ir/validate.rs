use std::cell::Cell;
use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use super::containers::{container_kind, validate_stream_selector};
use super::*;
use crate::sim::emit_c::LLG_MAX_WIDTH;

mod calls;
mod dependencies;
mod events;
mod expressions;
mod file_input;
mod initialization;
mod lvalues;
mod statements;
mod tables;
mod values;

/// A structural IR invariant violation detected between simulator phases.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrValidationError {
    path: String,
    detail: String,
}

impl IrValidationError {
    pub(in crate::sim) fn new(path: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            detail: detail.into(),
        }
    }

    /// Location of the invalid value within the IR model.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Human-readable description of the violated invariant.
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl fmt::Display for IrValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid simulator IR at {}: {}", self.path, self.detail)
    }
}

impl Error for IrValidationError {}

type ValidationResult = Result<(), IrValidationError>;

fn lhs_is_real(model: &IrModel, lhs: &IrLhs) -> bool {
    match lhs {
        IrLhs::Whole(index) => model
            .signals
            .get(*index)
            .is_some_and(|signal| matches!(signal.ty, IrType::Real { .. })),
        IrLhs::WholeRef { width, .. } => *width == 0,
        IrLhs::ArrayElem {
            arr,
            elem_sel: IrElemSel::Whole,
            ..
        } => model.arrays.get(*arr).is_some_and(|array| array.real),
        _ => false,
    }
}

fn lhs_signed(model: &IrModel, lhs: &IrLhs) -> Option<bool> {
    match lhs {
        IrLhs::Whole(index) => model.signals.get(*index).map(|signal| signal.ty.signed()),
        IrLhs::WholeRef { width, signed, .. } | IrLhs::Ref { width, signed, .. } => {
            (*width != 0).then_some(*signed)
        }
        IrLhs::PackedSelect { signed, .. } => Some(*signed),
        IrLhs::Bit(..) | IrLhs::Part(..) | IrLhs::IdxPart(..) | IrLhs::Stream { .. } => Some(false),
        IrLhs::ArrayElem { arr, elem_sel, .. } => {
            let array = model.arrays.get(*arr)?;
            match elem_sel {
                IrElemSel::Whole => (!array.real).then_some(array.signed),
                IrElemSel::Part(..)
                | IrElemSel::Bit(..)
                | IrElemSel::Indexed { .. }
                | IrElemSel::PackedChain(_) => Some(false),
            }
        }
    }
}

fn lhs_two_state(model: &IrModel, lhs: &IrLhs) -> Option<bool> {
    match lhs {
        IrLhs::PackedSelect {
            target, two_state, ..
        } => lhs_two_state(model, target).map(|state| state || *two_state),
        IrLhs::Whole(index) => model
            .signals
            .get(*index)
            .map(|signal| signal.ty.two_state()),
        IrLhs::WholeRef {
            width, two_state, ..
        }
        | IrLhs::Ref {
            width, two_state, ..
        } => (*width != 0).then_some(*two_state),
        IrLhs::Bit(index, ..) | IrLhs::Part(index, ..) | IrLhs::IdxPart(index, ..) => model
            .signals
            .get(*index)
            .map(|signal| signal.ty.two_state()),
        IrLhs::ArrayElem { arr, .. } => model.arrays.get(*arr).map(|array| array.two_state),
        IrLhs::Stream { .. } => Some(false),
    }
}

fn validate_container_element(element: &IrContainerElement, path: &str) -> ValidationResult {
    match element {
        IrContainerElement::Packed { width, .. } if *width == 0 => {
            Err(IrValidationError::new(path, "packed width must be nonzero"))
        }
        IrContainerElement::Packed { .. }
        | IrContainerElement::Real { .. }
        | IrContainerElement::String
        | IrContainerElement::Chandle
        | IrContainerElement::Event
        | IrContainerElement::Opaque { .. } => Ok(()),
        IrContainerElement::Aggregate { members, .. }
        | IrContainerElement::Union { members, .. } => {
            for (index, member) in members.iter().enumerate() {
                if member.name.is_empty() {
                    return Err(IrValidationError::new(
                        format!("{path}.members[{index}]"),
                        "aggregate member name must not be empty",
                    ));
                }
                validate_container_element(&member.element, &format!("{path}.members[{index}]"))?;
            }
            Ok(())
        }
        IrContainerElement::FixedArray {
            dimensions,
            element,
        } => {
            if dimensions.is_empty() {
                return Err(IrValidationError::new(
                    path,
                    "fixed-array element has no dimensions",
                ));
            }
            let mut count = 1u128;
            for (left, right) in dimensions {
                let extent = (i64::from(*left) - i64::from(*right))
                    .unsigned_abs()
                    .checked_add(1)
                    .ok_or_else(|| {
                        IrValidationError::new(path, "fixed-array dimension extent overflows")
                    })?;
                count = count.checked_mul(u128::from(extent)).ok_or_else(|| {
                    IrValidationError::new(path, "fixed-array element count overflows")
                })?;
            }
            let _ = count;
            validate_container_element(element, &format!("{path}.element"))
        }
        IrContainerElement::Container { element, .. } => {
            validate_container_element(element, &format!("{path}.element"))
        }
    }
}

struct Validator<'model> {
    model: &'model IrModel,
    max_width: Cell<u128>,
    /// None outside a C function; otherwise whether that function returns chandle.
    chandle_return: Cell<Option<bool>>,
    string_return: Cell<Option<bool>>,
    function: Cell<Option<&'model IrFunc>>,
}

impl IrModel {
    /// Validate all cross-table references and representation invariants.
    ///
    /// Lowering and optimization must call this at their phase boundaries;
    /// the C emitter may then index the model tables without defensive checks.
    pub fn validate(&self) -> Result<(), IrValidationError> {
        self.packed_capacity().map(|_| ())
    }

    /// Validate the model and find its required packed storage capacity.
    /// Includes intermediate expressions, function locals, and initializers.
    pub fn packed_capacity(&self) -> Result<u128, IrValidationError> {
        let validator = Validator::new(self);
        validator.validate()?;
        Ok(validator.max_width.get())
    }

    /// Validate a detached expression against this model's index tables.
    pub fn validate_expr(
        &self,
        expression: &IrExpr,
        function: Option<&IrFunc>,
    ) -> Result<(), IrValidationError> {
        self.expression_capacity(expression, function).map(|_| ())
    }

    /// Validate a detached expression and report its widest packed shape.
    pub fn expression_capacity(
        &self,
        expression: &IrExpr,
        function: Option<&IrFunc>,
    ) -> Result<u128, IrValidationError> {
        let validator = Validator::new(self);
        validator.function.set(function);
        validator
            .chandle_return
            .set(function.map(|function| function.ret_chandle));
        validator
            .string_return
            .set(function.map(|function| function.ret_string));
        validator.validate_expr(expression, function.map_or(&[], IrFunc::formals), "expr")?;
        Ok(validator.max_width.get())
    }

    /// Validate a detached statement against this model's index tables.
    pub fn validate_stmt(
        &self,
        statement: &IrStmt,
        function: Option<&IrFunc>,
    ) -> Result<(), IrValidationError> {
        self.statement_capacity(statement, function).map(|_| ())
    }

    /// Validate a detached statement and report its widest packed shape.
    pub fn statement_capacity(
        &self,
        statement: &IrStmt,
        function: Option<&IrFunc>,
    ) -> Result<u128, IrValidationError> {
        let validator = Validator::new(self);
        validator.function.set(function);
        validator
            .chandle_return
            .set(function.map(|function| function.ret_chandle));
        validator
            .string_return
            .set(function.map(|function| function.ret_string));
        validator.validate_stmts(
            std::slice::from_ref(statement),
            function.map_or(&[], IrFunc::formals),
            "stmt",
        )?;
        Ok(validator.max_width.get())
    }

    /// Validate a detached helper function against this model's index tables.
    pub fn validate_pre_fn(
        &self,
        pre_fn: &IrPreFn,
        function: Option<&IrFunc>,
    ) -> Result<(), IrValidationError> {
        self.pre_fn_capacity(pre_fn, function).map(|_| ())
    }

    /// Validate a detached helper and report its widest packed shape.
    pub fn pre_fn_capacity(
        &self,
        pre_fn: &IrPreFn,
        function: Option<&IrFunc>,
    ) -> Result<u128, IrValidationError> {
        let validator = Validator::new(self);
        validator
            .chandle_return
            .set(function.map(|function| function.ret_chandle));
        validator
            .string_return
            .set(function.map(|function| function.ret_string));
        validator.validate_pre_fns(
            std::slice::from_ref(pre_fn),
            function.map_or(&[], IrFunc::formals),
            "pre_fn",
        )?;
        Ok(validator.max_width.get())
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod packed_selection_tests;
