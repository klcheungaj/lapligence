use std::cell::Cell;
use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use super::containers::{container_kind, validate_stream_selector};
use super::*;
use crate::sim::emit_c::LLG_MAX_WIDTH;

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
        IrLhs::Bit(..) | IrLhs::Part(..) | IrLhs::IdxPart(..) | IrLhs::Stream { .. } => Some(false),
        IrLhs::ArrayElem { arr, elem_sel, .. } => {
            let array = model.arrays.get(*arr)?;
            match elem_sel {
                IrElemSel::Whole => (!array.real).then_some(array.signed),
                IrElemSel::Part(..) | IrElemSel::Bit(..) | IrElemSel::Indexed { .. } => Some(false),
            }
        }
    }
}

fn lhs_two_state(model: &IrModel, lhs: &IrLhs) -> Option<bool> {
    match lhs {
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
        IrContainerElement::Aggregate { members, .. } => {
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

impl Validator<'_> {
    fn new(model: &IrModel) -> Validator<'_> {
        Validator {
            model,
            max_width: Cell::new(0),
            chandle_return: Cell::new(None),
            string_return: Cell::new(None),
        }
    }

    fn valid_dependency(&self, dependency: &IrDependency) -> bool {
        match dependency {
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

    fn validate_event_ref(
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

    fn validate_plusarg_text(
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

    fn validate(&self) -> ValidationResult {
        if self.model.precision_fs == 0 {
            return self.fail("precision_fs", "scheduler precision must be non-zero");
        }

        let mut storage_names = HashSet::new();
        for (idx, container) in self.model.containers.iter().enumerate() {
            if !storage_names.insert(container.c_name.as_str()) {
                return self.fail("containers", "duplicate container storage name");
            }
            validate_container_element(&container.element, &format!("containers[{idx}].element"))?;
            if !container.element.is_packed()
                && !matches!(
                    container.kind,
                    IrContainerKind::Dynamic
                        | IrContainerKind::Queue { .. }
                        | IrContainerKind::Associative { .. }
                )
            {
                return self.fail(
                    format!("containers[{idx}].element"),
                    "non-packed elements require a descriptor-backed container",
                );
            }
            if let IrContainerKind::Associative {
                key: IrAssocKey::Integral { width, .. },
            } = container.kind
            {
                self.validate_width(width, &format!("containers[{idx}].key"))?;
            }
            if let Some(size) = container.initial_size {
                if size == 0 {
                    return self.fail(
                        format!("containers[{idx}].initial_size"),
                        "initial container size must be non-zero",
                    );
                }
                if !matches!(container.kind, IrContainerKind::Dynamic) {
                    return self.fail(
                        format!("containers[{idx}].initial_size"),
                        "only dynamic containers may have an initial size",
                    );
                }
            }
        }
        for object in &self.model.objects {
            if !storage_names.insert(object.c_name.as_str()) {
                return self.fail(
                    "objects",
                    format!("duplicate object storage name `{}`", object.c_name),
                );
            }
            if let Some(initial) = &object.initial {
                if object.ty != IrObjectType::String {
                    return self.fail("objects", "non-string initializer");
                }
                initial.validate(self.model, None)?;
                let mut result = Ok(());
                initial.expressions(&mut |expr| {
                    result = result
                        .clone()
                        .and_then(|_| self.validate_expr(expr, &[], "object.initial"));
                });
                result?;
            }
        }
        for (idx, signal) in self.model.signals.iter().enumerate() {
            let path = format!("signals[{idx}]");
            self.validate_type(&signal.ty, &format!("{path}.ty"))?;
            if !signal.net_alias.is_empty() {
                if !matches!(signal.ty, IrType::Packed { .. }) {
                    return self.fail(
                        format!("{path}.net_alias"),
                        "true-net alias bindings require packed signal storage",
                    );
                }
                if signal.alias.is_some() {
                    return self.fail(
                        format!("{path}.net_alias"),
                        "true-net alias storage cannot also be a variable alias",
                    );
                }
                if signal.net_driver.is_some() {
                    return self.fail(
                        format!("{path}.net_alias"),
                        "true-net alias storage cannot also be an inout driver",
                    );
                }
                let mut signal_bits = HashSet::new();
                for (binding_idx, binding) in signal.net_alias.iter().enumerate() {
                    let binding_path = format!("{path}.net_alias[{binding_idx}]");
                    let group = self.model.net_groups.get(binding.group).ok_or_else(|| {
                        IrValidationError::new(
                            format!("{binding_path}.group"),
                            format!("net-group index {} is out of bounds", binding.group),
                        )
                    })?;
                    if binding.slot >= group.n_drivers {
                        return self.fail(
                            format!("{binding_path}.slot"),
                            format!(
                                "driver slot {} is out of bounds for group {}",
                                binding.slot, binding.group
                            ),
                        );
                    }
                    if binding.group_bit >= group.width {
                        return self.fail(
                            format!("{binding_path}.group_bit"),
                            format!(
                                "group bit {} is out of bounds for group {}",
                                binding.group_bit, binding.group
                            ),
                        );
                    }
                    if binding.signal_bit >= signal.ty.width() {
                        return self.fail(
                            format!("{binding_path}.signal_bit"),
                            format!(
                                "signal bit {} is out of bounds for signal width {}",
                                binding.signal_bit,
                                signal.ty.width()
                            ),
                        );
                    }
                    if !signal_bits.insert(binding.signal_bit) {
                        return self.fail(
                            format!("{binding_path}.signal_bit"),
                            "signal bit has more than one canonical alias binding",
                        );
                    }
                }
            }
            if signal.alias.is_some() && signal.net_driver.is_some() {
                return self.fail(
                    format!("{path}.alias"),
                    "net driver cannot also be a variable alias",
                );
            }
            if let Some((group, slot)) = signal.net_driver {
                let net = self.model.net_groups.get(group).ok_or_else(|| {
                    IrValidationError::new(
                        format!("{path}.net_driver"),
                        format!("net-group index {group} is out of bounds"),
                    )
                })?;
                if slot >= net.n_drivers {
                    return self.fail(
                        format!("{path}.net_driver"),
                        format!("driver slot {slot} is out of bounds for group {group}"),
                    );
                }
                if signal.ty.width() != net.width || signal.ty.signed() != net.signed {
                    return self.fail(
                        format!("{path}.net_driver"),
                        "signal type does not match its net group",
                    );
                }
            } else if let Some(target) = signal.alias {
                let canonical = self.model.signals.get(target).ok_or_else(|| {
                    IrValidationError::new(format!("{path}.alias"), "alias target is out of bounds")
                })?;
                if target == idx
                    || canonical.alias.is_some()
                    || canonical.net_driver.is_some()
                    || canonical.ty != signal.ty
                    || canonical.c_name != signal.c_name
                    || (!signal.omit && canonical.omit)
                {
                    return self.fail(
                        format!("{path}.alias"),
                        "alias must name matching canonical variable storage",
                    );
                }
            } else if !signal.omit && !storage_names.insert(signal.c_name.as_str()) {
                return self.fail(
                    format!("{path}.c_name"),
                    format!("active storage name `{}` is not unique", signal.c_name),
                );
            }
        }

        for (idx, group) in self.model.net_groups.iter().enumerate() {
            let path = format!("net_groups[{idx}]");
            self.validate_width(group.width, &format!("{path}.width"))?;
            if group.n_drivers == 0 {
                return self.fail(format!("{path}.n_drivers"), "net group has no drivers");
            }
            if group.n_drivers > LLG_MAX_NET_DRIVERS {
                return self.fail(
                    format!("{path}.n_drivers"),
                    format!("net group exceeds {LLG_MAX_NET_DRIVERS} drivers"),
                );
            }
            if group.driver_strengths.len() != group.n_drivers {
                return self.fail(
                    format!("{path}.driver_strengths"),
                    "net group strength count does not match its driver count",
                );
            }
            if let Some((strength_idx, _)) = group
                .driver_strengths
                .iter()
                .enumerate()
                .find(|(_, (strength0, strength1))| *strength0 > 7 || *strength1 > 7)
            {
                return self.fail(
                    format!("{path}.driver_strengths[{strength_idx}]"),
                    "net driver strength is outside the IEEE 1800 strength scale",
                );
            }
        }

        for (idx, assertion) in self.model.assertions.iter().enumerate() {
            let path = format!("assertions[{idx}]");
            let Some(clock) = self.model.signals.get(assertion.clock_signal) else {
                return self.fail(
                    format!("{path}.clock_signal"),
                    "assertion clock signal index is out of bounds",
                );
            };
            if clock.omit || clock.ty.width() == 0 {
                return self.fail(
                    format!("{path}.clock_signal"),
                    "assertion clock must be an active packed signal",
                );
            }
            if let Some(disable) = assertion.disable_signal {
                let Some(signal) = self.model.signals.get(disable) else {
                    return self.fail(
                        format!("{path}.disable_signal"),
                        "assertion disable signal index is out of bounds",
                    );
                };
                if signal.omit || signal.ty.width() == 0 {
                    return self.fail(
                        format!("{path}.disable_signal"),
                        "assertion disable must be an active packed signal",
                    );
                }
            }
            if let Some(antecedent) = &assertion.antecedent {
                self.validate_expr(antecedent, &[], &format!("{path}.antecedent"))?;
                if antecedent.is_real() {
                    return self.fail(
                        format!("{path}.antecedent"),
                        "assertion antecedent must be packed",
                    );
                }
            }
            self.validate_expr(&assertion.consequent, &[], &format!("{path}.consequent"))?;
            if assertion.consequent.is_real() {
                return self.fail(
                    format!("{path}.consequent"),
                    "assertion consequent must be packed",
                );
            }
        }

        for (idx, domain) in self.model.sampled_domains.iter().enumerate() {
            let path = format!("sampled_domains[{idx}]");
            let Some(clock) = self.model.signals.get(domain.clock_signal) else {
                return self.fail(
                    format!("{path}.clock_signal"),
                    "sampled clock signal index is out of bounds",
                );
            };
            if clock.omit || clock.ty.width() == 0 {
                return self.fail(
                    format!("{path}.clock_signal"),
                    "sampled clock must be an active packed signal",
                );
            }
            self.validate_expr(&domain.sample, &[], &format!("{path}.sample"))?;
            if domain.sample.is_real() {
                return self.fail(format!("{path}.sample"), "sampled value must be packed");
            }
            if let Some(gate) = &domain.gate {
                self.validate_expr(gate, &[], &format!("{path}.gate"))?;
                if gate.is_real() {
                    return self.fail(format!("{path}.gate"), "sampled gate must be packed");
                }
            }
        }

        for (idx, array) in self.model.arrays.iter().enumerate() {
            let path = format!("arrays[{idx}]");
            if array.real {
                if array.elem_width != 0 {
                    return self.fail(
                        format!("{path}.elem_width"),
                        "real array elements must use zero width",
                    );
                }
            } else {
                self.validate_width(array.elem_width, &format!("{path}.elem_width"))?;
            }
            if array.dims.is_empty() {
                return self.fail(format!("{path}.dims"), "array has no dimensions");
            }
            let mut total = 1u64;
            for (dim_idx, (left, right)) in array.dims.iter().copied().enumerate() {
                let extent = (i64::from(left) - i64::from(right)).unsigned_abs() + 1;
                total = total.checked_mul(extent).ok_or_else(|| {
                    IrValidationError::new(
                        format!("{path}.dims[{dim_idx}]"),
                        "dimension product overflows u64",
                    )
                })?;
            }
            if total != array.total {
                return self.fail(
                    format!("{path}.total"),
                    format!(
                        "stored total {} does not match dimension product {total}",
                        array.total
                    ),
                );
            }
        }

        for (idx, func) in self.model.funcs.iter().enumerate() {
            self.chandle_return.set(Some(func.ret_chandle));
            self.string_return.set(Some(func.ret_string));
            let path = format!("funcs[{idx}]");
            if (func.ret_chandle || func.ret_string) && func.ret.is_some()
                || func.ret_chandle && func.ret_string
            {
                return self.fail(&path, "function has incompatible return types");
            }
            if let Some(ret) = &func.ret {
                self.validate_type(ret, &format!("{path}.ret"))?;
            }
            for (formal_idx, formal) in func.formals.iter().enumerate() {
                if formal.const_ref && !formal.is_ref() {
                    return self.fail(
                        format!("{path}.formals[{formal_idx}].const_ref"),
                        "const qualification requires ref mode",
                    );
                }
                if formal.ref_static && !formal.is_ref() {
                    return self.fail(
                        format!("{path}.formals[{formal_idx}].ref_static"),
                        "ref static qualification requires ref mode",
                    );
                }
                if !formal.chandle && !formal.event && !formal.real && !formal.string {
                    self.validate_width(
                        formal.width,
                        &format!("{path}.formals[{formal_idx}].width"),
                    )?;
                } else if formal.real && formal.width != 0 {
                    return self.fail(
                        format!("{path}.formals[{formal_idx}].width"),
                        "real formal must use zero width",
                    );
                }
            }
            for (local_idx, local) in func.locals.iter().enumerate() {
                let local_path = format!("{path}.locals[{local_idx}]");
                if !local.real && !local.string {
                    self.validate_width(local.width, &format!("{local_path}.width"))?;
                } else if local.width != 0 {
                    return self.fail(
                        format!("{local_path}.width"),
                        "real local must use zero width",
                    );
                }
                if let Some(initial) = &local.initial {
                    self.validate_expr(initial, &func.formals, &format!("{local_path}.initial"))?;
                    if initial.is_real() != local.real
                        || initial.width != local.width
                        || initial.signed != local.signed
                    {
                        return self.fail(
                            format!("{local_path}.initial"),
                            "local initializer type disagrees with its declaration",
                        );
                    }
                }
            }
            self.validate_pre_fns(&func.pre_fns, &func.formals, &path)?;
            self.validate_stmts(&func.body, &func.formals, &format!("{path}.body"))?;
        }

        self.chandle_return.set(None);
        self.string_return.set(None);
        for (idx, process) in self.model.processes.iter().enumerate() {
            let path = format!("processes[{idx}]");
            for (write_idx, write) in process.writes.iter().enumerate() {
                if !self.valid_dependency(write) {
                    return self.fail(
                        format!("{path}.writes[{write_idx}]"),
                        "process write must name active storage",
                    );
                }
            }
            self.validate_pre_fns(&process.pre_fns, &[], &path)?;
            self.validate_stmts(&process.body, &[], &format!("{path}.body"))?;
        }

        for (idx, step) in self.model.init_steps.iter().enumerate() {
            self.validate_init_step(step, &format!("init_steps[{idx}]"))?;
        }

        self.validate_spawns(&self.model.spawns, "spawns")?;
        self.validate_spawns(&self.model.final_spawns, "final_spawns")?;
        let normal: HashSet<&str> = self.model.spawns.iter().map(String::as_str).collect();
        if let Some(name) = self
            .model
            .final_spawns
            .iter()
            .find(|name| normal.contains(name.as_str()))
        {
            return self.fail(
                "final_spawns",
                format!("process `{name}` is registered as both normal and final"),
            );
        }
        Ok(())
    }

    fn validate_type(&self, ty: &IrType, path: &str) -> ValidationResult {
        if let IrType::Packed { width, .. } = ty {
            self.validate_width(*width, path)?;
        }
        Ok(())
    }

    fn validate_width(&self, width: u32, path: &str) -> ValidationResult {
        if width == 0 {
            return self.fail(path, "packed width must be nonzero");
        }
        self.max_width
            .set(self.max_width.get().max(u128::from(width)));
        Ok(())
    }

    fn validate_const(&self, value: &IrConst, path: &str) -> ValidationResult {
        if value.real.is_some() {
            if value.width != 0 {
                return self.fail(path, "real constant has a non-zero packed width");
            }
            if value.fill.is_some() {
                return self.fail(path, "real constant carries a packed fill marker");
            }
            return Ok(());
        }
        self.validate_width(value.width, &format!("{path}.width"))?;
        if value.fill.is_some_and(|fill| fill > 3) {
            return self.fail(format!("{path}.fill"), "fill marker must be in 0..=3");
        }
        let limbs = value.width.div_ceil(64) as usize;
        for (name, values) in [("bits", &value.bits), ("x", &value.x), ("z", &value.z)] {
            if values.len() > limbs {
                return self.fail(
                    format!("{path}.{name}"),
                    format!("{} limbs exceed the {limbs}-limb width", values.len()),
                );
            }
        }
        for idx in 0..limbs {
            let x = value.x.get(idx).copied().unwrap_or(0);
            let z = value.z.get(idx).copied().unwrap_or(0);
            if x & z != 0 {
                return self.fail(format!("{path}.x"), "X and Z masks overlap");
            }
        }
        let tail = value.width % 64;
        if tail != 0 {
            let outside = !((1u64 << tail) - 1);
            for (name, values) in [("bits", &value.bits), ("x", &value.x), ("z", &value.z)] {
                if values.get(limbs - 1).copied().unwrap_or(0) & outside != 0 {
                    return self.fail(
                        format!("{path}.{name}"),
                        "high limb contains bits outside the declared width",
                    );
                }
            }
        }
        Ok(())
    }

    fn validate_expr(&self, expr: &IrExpr, formals: &[IrFormal], path: &str) -> ValidationResult {
        self.max_width
            .set(self.max_width.get().max(u128::from(expr.width)));
        if expr.fill.is_some_and(|fill| fill > 3) {
            return self.fail(format!("{path}.fill"), "fill marker must be in 0..=3");
        }
        if expr.width == 0 && expr.fill.is_some() {
            return self.fail(
                format!("{path}.fill"),
                "real expression carries a packed fill marker",
            );
        }
        match &expr.kind {
            IrExprKind::Container(operation) => {
                operation.validate(self.model, self.string_return.get())?;
                if (expr.width == 0
                    && !matches!(
                        operation.as_ref(),
                        IrContainerExpr::GetReal { .. } | IrContainerExpr::GetStringReal { .. }
                    ))
                    || expr.fill.is_some()
                {
                    return self.fail(path, "container expression must produce a packed value");
                }
                let expected = match operation.as_ref() {
                    IrContainerExpr::Stream { .. } => (expr.width, expr.signed),
                    IrContainerExpr::Size(_)
                    | IrContainerExpr::AssocTraverse { .. }
                    | IrContainerExpr::AssocTraverseString { .. }
                    | IrContainerExpr::AssocTraverseStringLocal { .. } => (32, true),
                    IrContainerExpr::Exists { .. } | IrContainerExpr::ExistsString { .. } => {
                        (32, true)
                    }
                    IrContainerExpr::ReduceWith {
                        result_width,
                        result_signed,
                        ..
                    } => (*result_width, *result_signed),
                    IrContainerExpr::Get { container, .. }
                    | IrContainerExpr::GetReal { container, .. }
                    | IrContainerExpr::GetString { container, .. }
                    | IrContainerExpr::GetStringReal { container, .. }
                    | IrContainerExpr::Reduce { container, .. }
                    | IrContainerExpr::QueueFront(container)
                    | IrContainerExpr::QueueBack(container)
                    | IrContainerExpr::QueuePopFront(container)
                    | IrContainerExpr::QueuePopBack(container) => {
                        let ty = &self.model.containers[*container].element;
                        (ty.width(), ty.signed())
                    }
                    IrContainerExpr::GetNested { .. } => (expr.width, expr.signed),
                    IrContainerExpr::GetNestedReal { .. } => (0, false),
                };
                if (expr.width, expr.signed) != expected {
                    return self.fail(path, "container result type disagrees with expression type");
                }
                let mut result = Ok(());
                operation.expressions(&mut |child| {
                    result = result
                        .clone()
                        .and_then(|_| self.validate_expr(child, formals, path));
                });
                result?;
            }
            IrExprKind::ObjectQuery(query) => {
                query.validate(
                    self.model,
                    formals,
                    self.chandle_return.get(),
                    self.string_return.get(),
                )?;
                let expected = match query.as_ref() {
                    IrObjectQuery::ChandleEq(..) => Some((1, false)),
                    IrObjectQuery::SemaphoreTryGet(..) => Some((32, true)),
                    IrObjectQuery::ProcessEq(..) => Some((1, false)),
                    IrObjectQuery::StringGetc(..) => Some((8, true)),
                    IrObjectQuery::StringAtoreal(..) => Some((0, true)),
                    IrObjectQuery::StringInside { .. } => Some((1, false)),
                    IrObjectQuery::StringPacked(..) => None,
                    IrObjectQuery::ProcessStatus(..) => Some((32, false)),
                    IrObjectQuery::ArrayQuery(query) => Some(query.result_type(self.model)),
                    _ => Some((32, true)),
                };
                if expected.is_some_and(|ty| ty != (expr.width, expr.signed)) {
                    return self.fail(path, "object query result type mismatch");
                }
                if (expr.width == 0 && !matches!(query.as_ref(), IrObjectQuery::StringAtoreal(_)))
                    || expr.fill.is_some()
                {
                    return self.fail(path, "object query has an invalid result shape");
                }
                let mut result = Ok(());
                query.expressions(&mut |child| {
                    result = result.clone().and_then(|_| {
                        let format_value =
                            |value: &IrStringExpr| matches!(value, IrStringExpr::Format { .. });
                        let allows_real = match query.as_ref() {
                            IrObjectQuery::StringLen(value)
                            | IrObjectQuery::StringAtoi(value, _)
                            | IrObjectQuery::StringAtoreal(value)
                            | IrObjectQuery::StringPacked(value) => format_value(value),
                            IrObjectQuery::StringGetc(value, index) => {
                                format_value(value) && !std::ptr::eq(child, index.as_ref())
                            }
                            IrObjectQuery::StringCompare(a, b, _) => {
                                format_value(a) || format_value(b)
                            }
                            IrObjectQuery::StringInside { value, items } => {
                                format_value(value)
                                    || items.iter().any(|item| match item {
                                        IrStringInsideItem::Value(value) => format_value(value),
                                        IrStringInsideItem::Range { low, high } => {
                                            format_value(low) || format_value(high)
                                        }
                                    })
                            }
                            _ => false,
                        };
                        if child.is_real() && !allows_real {
                            self.fail(path, "object query requires packed operands")
                        } else {
                            self.validate_expr(child, formals, path)
                        }
                    });
                });
                result?;
            }
            IrExprKind::EnumMethod(query) => {
                if query.members.is_empty() {
                    return self.fail(path, "enum method query has no declared members");
                }
                if query.default.is_real() {
                    return self.fail(path, "enum method default must be packed");
                }
                let expected = match query.method {
                    IrEnumMethod::Num => (32, true),
                    IrEnumMethod::First
                    | IrEnumMethod::Last
                    | IrEnumMethod::Next
                    | IrEnumMethod::Prev => (query.default.width, query.default.signed),
                };
                if (expr.width, expr.signed) != expected {
                    return self.fail(path, "enum method result type disagrees with query type");
                }
                if expr.fill.is_some() {
                    return self.fail(path, "enum method result carries a fill marker");
                }
                if matches!(query.method, IrEnumMethod::Next | IrEnumMethod::Prev) {
                    if query.receiver.is_none() {
                        return self.fail(path, "enum navigation query has no receiver");
                    }
                    if query.step.is_none() {
                        return self.fail(path, "enum navigation query has no step");
                    }
                } else {
                    if query.receiver.is_some() {
                        return self.fail(path, "type-only enum query carries a receiver");
                    }
                    if query.step.is_some() {
                        return self.fail(path, "type-only enum query carries a step");
                    }
                }
                let mut result = Ok(());
                query.expressions(&mut |child| {
                    result = result.clone().and_then(|_| {
                        if child.is_real() {
                            self.fail(path, "enum method requires packed operands")
                        } else {
                            self.validate_expr(child, formals, path)
                        }
                    });
                });
                result?;
                for (index, member) in query.members.iter().enumerate() {
                    if member.value.is_real()
                        || member.value.width != query.default.width
                        || member.value.signed != query.default.signed
                    {
                        return self.fail(
                            format!("{path}.members[{index}]"),
                            "enum member value type disagrees with enum type",
                        );
                    }
                }
            }
            IrExprKind::Const(value) => {
                self.validate_const(value, &format!("{path}.const"))?;
                if value.width != expr.width
                    || value.signed != expr.signed
                    || value.fill != expr.fill
                {
                    return self.fail(
                        path,
                        format!(
                            "constant payload type disagrees with expression type: payload width={} signed={} fill={:?}, expression width={} signed={} fill={:?}",
                            value.width,
                            value.signed,
                            value.fill,
                            expr.width,
                            expr.signed,
                            expr.fill
                        ),
                    );
                }
            }
            IrExprKind::SigRead(idx) => {
                let signal = self.model.signals.get(*idx).ok_or_else(|| {
                    IrValidationError::new(path, format!("signal index {idx} is out of bounds"))
                })?;
                if matches!(signal.ty, IrType::Real { .. }) != expr.is_real()
                    || signal.ty.width() != expr.width
                    || signal.ty.signed() != expr.signed
                {
                    return self.fail(path, "signal type disagrees with expression type");
                }
            }
            IrExprKind::LocalRead(_) => {}
            IrExprKind::FormalRead(idx) => {
                let formal = formals.get(*idx).ok_or_else(|| {
                    IrValidationError::new(path, format!("formal index {idx} is out of bounds"))
                })?;
                if formal.chandle
                    || formal.event
                    || formal.real != expr.is_real()
                    || formal.width != expr.width
                    || formal.signed != expr.signed
                {
                    return self.fail(path, "formal type disagrees with expression type");
                }
            }
            IrExprKind::CallFn(call) => {
                self.validate_call_expr(call, formals, path)?;
                let callee = &self.model.funcs[call.f];
                match callee.ret {
                    Some(ret) => {
                        if call.void_x {
                            return self.fail(path, "value-returning call is marked as void");
                        }
                        if matches!(ret, IrType::Real { .. }) != expr.is_real()
                            || ret.width() != expr.width
                            || ret.signed() != expr.signed
                        {
                            return self
                                .fail(path, "callee return type disagrees with expression type");
                        }
                    }
                    None => {
                        if !call.void_x {
                            return self.fail(path, "void call used as a value is not marked as X");
                        }
                        if expr.width != 1 || expr.signed || expr.fill.is_some() {
                            return self
                                .fail(path, "void-call fallback must be a 1-bit unsigned X");
                        }
                    }
                }
            }
            IrExprKind::EventTriggered(event) => {
                self.validate_event_ref(event, formals, path)?;
                if expr.width != 1 || expr.signed || expr.fill.is_some() {
                    return self.fail(
                        path,
                        "event triggered property must be a 1-bit unsigned value",
                    );
                }
            }
            IrExprKind::Mutation(mutation) => {
                self.validate_lhs(&mutation.lhs, formals, &format!("{path}.lhs"))?;
                self.validate_expr(&mutation.value, formals, &format!("{path}.value"))?;
                if mutation.current_width != 0 {
                    self.validate_width(mutation.current_width, &format!("{path}.current_width"))?;
                }
            }
            IrExprKind::DynamicCast(cast) => {
                if expr.width != 1 || expr.signed || expr.fill.is_some() {
                    return self.fail(path, "$cast status must be a 1-bit unsigned value");
                }
                if cast.class_target.is_some()
                    || cast.class_source.is_some()
                    || cast.class_expected.is_some()
                {
                    if cast.class_target.is_none()
                        || cast.class_source.is_none()
                        || cast.class_expected.is_none()
                        || cast.target_width != 0
                        || !cast.valid_values.is_empty()
                    {
                        return self.fail(path, "class $cast metadata is incomplete");
                    }
                    if !matches!(cast.lhs, IrLhs::WholeRef { width: 0, .. }) {
                        return self.fail(path, "class $cast target is not a handle slot");
                    }
                    if cast.class_target.as_deref().is_none_or(str::is_empty) {
                        return self.fail(path, "class $cast target address is empty");
                    }
                    let Some(expected) = cast.class_expected else {
                        return self.fail(path, "class $cast target type is missing");
                    };
                    if expected >= self.model.classes.len() {
                        return self.fail(path, "class $cast target type is out of bounds");
                    }
                    let Some(source) = cast.class_source.as_ref() else {
                        return self.fail(path, "class $cast source is missing");
                    };
                    source.validate(self.model, formals, self.chandle_return.get())?;
                    return Ok(());
                }
                self.validate_lhs(&cast.lhs, formals, &format!("{path}.lhs"))?;
                let lhs_width = self.lhs_packed_width(&cast.lhs);
                if cast.target_width == 0 {
                    if lhs_width.is_some() {
                        return self.fail(path, "$cast real target metadata disagrees with LHS");
                    }
                } else {
                    self.validate_width(cast.target_width, &format!("{path}.target_width"))?;
                    if lhs_width != Some(cast.target_width) {
                        return self.fail(path, "$cast target width disagrees with LHS");
                    }
                }
                self.validate_expr(&cast.rhs, formals, &format!("{path}.rhs"))?;
                for (idx, value) in cast.valid_values.iter().enumerate() {
                    if cast.target_width == 0
                        || value.width != cast.target_width
                        || value.signed != cast.target_signed
                    {
                        return self.fail(
                            format!("{path}.valid_values[{idx}]"),
                            "$cast enum member shape disagrees with target",
                        );
                    }
                    self.validate_expr(value, formals, &format!("{path}.valid_values[{idx}]"))?;
                }
            }
            IrExprKind::BitStreamCast {
                a,
                source_width,
                target_two_state: _,
            } => {
                if expr.width == 0 || *source_width == 0 || *source_width != expr.width {
                    return self.fail(
                        path,
                        "fixed bit-stream cast requires equal nonzero source and target widths",
                    );
                }
                self.validate_width(*source_width, &format!("{path}.source_width"))?;
                self.validate_expr(a, formals, &format!("{path}.a"))?;
                if a.width != *source_width || a.is_real() {
                    return self.fail(path, "bit-stream source width disagrees with metadata");
                }
            }
            IrExprKind::Bin { a, b, .. } | IrExprKind::RealBin { a, b, .. } => {
                self.validate_expr(a, formals, &format!("{path}.a"))?;
                self.validate_expr(b, formals, &format!("{path}.b"))?;
            }
            IrExprKind::Un { a, .. }
            | IrExprKind::RealUn { a, .. }
            | IrExprKind::CastToReal { a, .. }
            | IrExprKind::CastToPacked { a }
            | IrExprKind::Resize { a }
            | IrExprKind::Convert { a }
            | IrExprKind::ToTwoState { a } => {
                self.validate_expr(a, formals, &format!("{path}.a"))?;
            }
            IrExprKind::Mux { sel, a, b } => {
                self.validate_expr(sel, formals, &format!("{path}.sel"))?;
                self.validate_expr(a, formals, &format!("{path}.a"))?;
                self.validate_expr(b, formals, &format!("{path}.b"))?;
            }
            IrExprKind::Concat { parts } | IrExprKind::Replicate { parts, .. } => {
                if parts.is_empty() {
                    return self.fail(path, "concatenation requires at least one operand");
                }
                let sum: u128 = parts.iter().map(|part| u128::from(part.width)).sum();
                let runtime_sized = parts.iter().any(|part| part.width == LLG_MAX_WIDTH);
                if runtime_sized {
                    if !matches!(expr.kind, IrExprKind::Concat { .. })
                        || expr.width != LLG_MAX_WIDTH
                    {
                        return self.fail(
                            path,
                            "runtime-sized concatenation must retain model capacity width",
                        );
                    }
                } else {
                    let expected = match &expr.kind {
                        IrExprKind::Replicate { count, .. } => sum * u128::from(*count),
                        _ => sum,
                    };
                    if expected != u128::from(expr.width) {
                        return self.fail(path, format!("concatenation/replication width {} disagrees with derived width {expected}", expr.width));
                    }
                }
                for (idx, part) in parts.iter().enumerate() {
                    self.validate_expr(part, formals, &format!("{path}.parts[{idx}]"))?;
                }
            }
            IrExprKind::Stream { value, slice, .. } => {
                if *slice == 0 {
                    return self.fail(path, "streaming slice size must be positive");
                }
                self.validate_expr(value, formals, &format!("{path}.value"))?;
                if value.is_real() || expr.width != value.width || expr.signed {
                    return self.fail(
                        path,
                        "streaming expression must preserve packed width and produce unsigned data",
                    );
                }
            }
            IrExprKind::Inside { value, items } => {
                if items.is_empty() {
                    return self.fail(path, "inside expression requires at least one set item");
                }
                if expr.width != 1 || expr.signed {
                    return self.fail(path, "inside expression requires a 1-bit unsigned result");
                }
                self.validate_expr(value, formals, &format!("{path}.value"))?;
                for (idx, item) in items.iter().enumerate() {
                    match item {
                        IrInsideItem::Value(item) => {
                            self.validate_expr(item, formals, &format!("{path}.items[{idx}]"))?;
                        }
                        IrInsideItem::Range { low, high } => {
                            self.validate_expr(low, formals, &format!("{path}.items[{idx}].low"))?;
                            self.validate_expr(
                                high,
                                formals,
                                &format!("{path}.items[{idx}].high"),
                            )?;
                        }
                        IrInsideItem::OpenRange { low, high } => {
                            if low.is_none() && high.is_none() {
                                return self.fail(
                                    format!("{path}.items[{idx}]"),
                                    "inside open range requires an endpoint",
                                );
                            }
                            if let Some(low) = low {
                                self.validate_expr(
                                    low,
                                    formals,
                                    &format!("{path}.items[{idx}].low"),
                                )?;
                            }
                            if let Some(high) = high {
                                self.validate_expr(
                                    high,
                                    formals,
                                    &format!("{path}.items[{idx}].high"),
                                )?;
                            }
                        }
                        IrInsideItem::Container { container } => {
                            let Some(container_model) = self.model.containers.get(*container)
                            else {
                                return self.fail(
                                    format!("{path}.items[{idx}]"),
                                    "inside container index is out of bounds",
                                );
                            };
                            if !container_model.element.is_packed() {
                                return self.fail(
                                    format!("{path}.items[{idx}]"),
                                    "inside container element must be packed",
                                );
                            }
                        }
                    }
                }
            }
            IrExprKind::BitSel { base, idx } => {
                self.validate_expr(base, formals, &format!("{path}.base"))?;
                self.validate_expr(idx, formals, &format!("{path}.idx"))?;
            }
            IrExprKind::PartSel { base, left, right } => {
                self.validate_expr(base, formals, &format!("{path}.base"))?;
                self.validate_select_width(*left, *right, path)?;
            }
            IrExprKind::IdxPartSel {
                base,
                base_idx,
                width_expr,
                ..
            } => {
                self.validate_expr(base, formals, &format!("{path}.base"))?;
                self.validate_expr(base_idx, formals, &format!("{path}.base_idx"))?;
                self.validate_expr(width_expr, formals, &format!("{path}.width_expr"))?;
            }
            IrExprKind::ArrayRead {
                arr,
                indices,
                elem_sel,
            } => {
                let array = self.model.arrays.get(*arr).ok_or_else(|| {
                    IrValidationError::new(path, format!("array index {arr} is out of bounds"))
                })?;
                if indices.len() != array.dims.len() {
                    return self.fail(
                        path,
                        format!(
                            "array read has {} indices for {} dimensions",
                            indices.len(),
                            array.dims.len()
                        ),
                    );
                }
                for (idx, index) in indices.iter().enumerate() {
                    self.validate_expr(index, formals, &format!("{path}.indices[{idx}]"))?;
                }
                self.validate_elem_sel(elem_sel, formals, &format!("{path}.elem_sel"))?;
            }
            IrExprKind::Fill(fill) => {
                if *fill > 3 {
                    return self.fail(path, "fill value must be in 0..=3");
                }
            }
            IrExprKind::Verbatim { width, signed, .. } => {
                if *width != expr.width || *signed != expr.signed {
                    return self.fail(path, "verbatim payload type disagrees with expression type");
                }
            }
            IrExprKind::SysFunc(sys) => match sys {
                IrSysFunc::TestPlusArgs { pattern } => {
                    self.validate_plusarg_text(pattern, formals, &format!("{path}.pattern"))?;
                    if (expr.width, expr.signed) != (32, true) {
                        return self.fail(path, "invalid $test$plusargs expression");
                    }
                }
                IrSysFunc::ValuePlusArgs { format, target } => {
                    self.validate_plusarg_text(format, formals, &format!("{path}.format"))?;
                    if (expr.width, expr.signed) != (32, true) {
                        return self.fail(path, "invalid $value$plusargs expression");
                    }
                    match target {
                        IrPlusArgTarget::Packed {
                            lhs,
                            width,
                            signed,
                            two_state,
                        } => {
                            self.validate_lhs(lhs, formals, &format!("{path}.target"))?;
                            if *width == 0
                                || self.lhs_packed_width(lhs) != Some(*width)
                                || lhs_signed(self.model, lhs) != Some(*signed)
                                || lhs_two_state(self.model, lhs) != Some(*two_state)
                            {
                                return self.fail(
                                    format!("{path}.target"),
                                    "plusarg packed target type disagrees with its lvalue",
                                );
                            }
                        }
                        IrPlusArgTarget::Real { lhs, .. } => {
                            self.validate_lhs(lhs, formals, &format!("{path}.target"))?;
                            if !lhs_is_real(self.model, lhs) {
                                return self.fail(
                                    format!("{path}.target"),
                                    "plusarg real target is not real storage",
                                );
                            }
                        }
                        IrPlusArgTarget::String { address } => {
                            if address.is_empty() {
                                return self.fail(
                                    format!("{path}.target"),
                                    "plusarg string target address must not be empty",
                                );
                            }
                        }
                    }
                }
                IrSysFunc::System(command) => {
                    if let Some(command) = command {
                        command.validate(self.model, self.string_return.get())?;
                        let mut result = Ok(());
                        command.expressions(&mut |child| {
                            result = result
                                .clone()
                                .and_then(|_| self.validate_expr(child, formals, path));
                        });
                        result?;
                    }
                    if expr.width != 32 || !expr.signed {
                        return self.fail(path, "$system requires a signed int result");
                    }
                }
                IrSysFunc::LegacyRandom { kind, seed, args } => {
                    if args.len() != kind.arity() || (expr.width, expr.signed) != (32, true) {
                        return self.fail(
                            path,
                            "legacy random function requires its declared arity and signed int result",
                        );
                    }
                    match (kind, seed) {
                        (IrRandomFunc::Random, None) => {}
                        (_, Some(seed)) => {
                            self.validate_lhs(seed, formals, &format!("{path}.seed"))?;
                            if self.lhs_packed_width(seed).is_none() {
                                return self.fail(
                                    format!("{path}.seed"),
                                    "legacy random seed must be packed storage",
                                );
                            }
                        }
                        (_, None) => {
                            return self.fail(
                                format!("{path}.seed"),
                                "distribution function requires a seed",
                            )
                        }
                    }
                    for (index, arg) in args.iter().enumerate() {
                        if arg.is_real() {
                            return self.fail(
                                format!("{path}.args[{index}]"),
                                "legacy random parameters must be packed",
                            );
                        }
                        self.validate_expr(arg, formals, &format!("{path}.args[{index}]"))?;
                    }
                }
                IrSysFunc::Urandom { seed } => {
                    if let Some(seed) = seed {
                        self.validate_expr(seed, formals, &format!("{path}.seed"))?;
                        if seed.is_real() || (seed.width, seed.signed) != (32, false) {
                            return self
                                .fail(path, "$urandom seed must be a 32-bit unsigned value");
                        }
                    }
                    if (expr.width, expr.signed) != (32, false) {
                        return self.fail(path, "$urandom requires a 32-bit unsigned result");
                    }
                }
                IrSysFunc::UrandomRange { max, min } => {
                    self.validate_expr(max, formals, &format!("{path}.max"))?;
                    if (max.width, max.signed) != (32, false) {
                        return self.fail(
                            path,
                            "$urandom_range maximum must be a 32-bit unsigned value",
                        );
                    }
                    if let Some(min) = min {
                        self.validate_expr(min, formals, &format!("{path}.min"))?;
                        if (min.width, min.signed) != (32, false) {
                            return self.fail(
                                path,
                                "$urandom_range minimum must be a 32-bit unsigned value",
                            );
                        }
                    }
                    if (expr.width, expr.signed) != (32, false) {
                        return self.fail(path, "$urandom_range requires a 32-bit unsigned result");
                    }
                }
                IrSysFunc::Math { kind, args } => {
                    if args.len() != kind.arity() || !expr.is_real() {
                        return self.fail(
                            path,
                            "math function requires its declared arity and a real result",
                        );
                    }
                    for (index, arg) in args.iter().enumerate() {
                        self.validate_expr(arg, formals, &format!("{path}.args[{index}]"))?;
                    }
                }
                IrSysFunc::Realtime {
                    precision_fs,
                    unit_fs,
                } => {
                    if *precision_fs == 0 || *unit_fs == 0 || !expr.is_real() {
                        return self.fail(
                            path,
                            "realtime requires nonzero scaling units and a real result",
                        );
                    }
                }
                IrSysFunc::Rtoi(arg) => {
                    self.validate_expr(arg, formals, &format!("{path}.arg"))?;
                    if expr.width != 32 || !expr.signed {
                        return self.fail(path, "$rtoi requires a signed int result");
                    }
                }
                IrSysFunc::Itor(arg) => {
                    self.validate_expr(arg, formals, &format!("{path}.arg"))?;
                    if arg.is_real() || !expr.is_real() {
                        return self.fail(path, "$itor requires a packed argument and real result");
                    }
                }
                IrSysFunc::RealToBits(arg) => {
                    self.validate_expr(arg, formals, &format!("{path}.arg"))?;
                    if expr.width != 64 || expr.signed {
                        return self.fail(path, "$realtobits requires a 64-bit unsigned result");
                    }
                }
                IrSysFunc::BitsToReal(arg) => {
                    self.validate_expr(arg, formals, &format!("{path}.arg"))?;
                    if arg.is_real() || arg.width != 64 || !expr.is_real() {
                        return self.fail(
                            path,
                            "$bitstoreal requires a 64-bit packed argument and real result",
                        );
                    }
                }
                IrSysFunc::ShortRealToBits(arg) => {
                    self.validate_expr(arg, formals, &format!("{path}.arg"))?;
                    if expr.width != 32 || expr.signed {
                        return self
                            .fail(path, "$shortrealtobits requires a 32-bit unsigned result");
                    }
                }
                IrSysFunc::BitsToShortReal(arg) => {
                    self.validate_expr(arg, formals, &format!("{path}.arg"))?;
                    if arg.is_real() || arg.width != 32 || !expr.is_real() {
                        return self.fail(
                            path,
                            "$bitstoshortreal requires a 32-bit packed argument and real result",
                        );
                    }
                }
                IrSysFunc::BitQuery { kind, arg } => {
                    self.validate_expr(arg, formals, &format!("{path}.arg"))?;
                    if arg.is_real() || (expr.width, expr.signed) != kind.result_type() {
                        return self.fail(
                            path,
                            "bit query requires a packed argument and its declared result type",
                        );
                    }
                }
                IrSysFunc::Sampled(call) => {
                    self.validate_expr(&call.argument, formals, &format!("{path}.argument"))?;
                    if call.argument.is_real() {
                        return self.fail(path, "sampled-value argument must be packed");
                    }
                    match call.kind {
                        crate::sim::ir::IrSampledFunc::Sampled => {
                            if call.domain.is_some() || call.ticks != 0 {
                                return self.fail(path, "$sampled cannot carry a history domain");
                            }
                            if (expr.width, expr.signed)
                                != (call.argument.width, call.argument.signed)
                            {
                                return self
                                    .fail(path, "$sampled result shape disagrees with argument");
                            }
                        }
                        crate::sim::ir::IrSampledFunc::Past => {
                            let Some(domain) = call.domain else {
                                return self.fail(path, "$past requires a sampled domain");
                            };
                            if domain >= self.model.sampled_domains.len() || call.ticks == 0 {
                                return self.fail(path, "$past history metadata is invalid");
                            }
                            if (expr.width, expr.signed)
                                != (call.argument.width, call.argument.signed)
                            {
                                return self
                                    .fail(path, "$past result shape disagrees with argument");
                            }
                        }
                        IrSampledFunc::Rose
                        | IrSampledFunc::Fell
                        | IrSampledFunc::Stable
                        | IrSampledFunc::Changed => {
                            if call.domain.is_none() || call.ticks != 0 {
                                return self.fail(path, "sampled status requires a valid domain");
                            }
                            if expr.width != 1 || expr.signed {
                                return self
                                    .fail(path, "sampled status result must be one-bit unsigned");
                            }
                        }
                    }
                }
                IrSysFunc::Clog2(arg) | IrSysFunc::Bits(arg) => {
                    self.validate_expr(arg, formals, &format!("{path}.arg"))?;
                }
                IrSysFunc::Time {
                    precision_fs,
                    unit_fs,
                    kind,
                } => {
                    if *precision_fs == 0 || *unit_fs == 0 {
                        return self.fail(path, "time scaling units must be non-zero");
                    }
                    if expr.width != kind.width() {
                        return self.fail(path, "time expression width disagrees with its kind");
                    }
                }
                IrSysFunc::QFull { q_id, status } => {
                    self.validate_expr(q_id, formals, &format!("{path}.q_id"))?;
                    if q_id.is_real() || expr.width != 32 || !expr.signed {
                        return self
                            .fail(path, "$q_full requires a packed q_id and signed int result");
                    }
                    self.validate_stochastic_output(status, formals, &format!("{path}.status"))?;
                }
                IrSysFunc::FileOpen {
                    path: file_path,
                    mode,
                } => {
                    file_path.validate(self.model, self.string_return.get())?;
                    if let Some(mode) = mode {
                        mode.validate(self.model, self.string_return.get())?;
                    }
                    if expr.width != 32 || !expr.signed {
                        return self.fail(path, "$fopen requires a signed 32-bit result");
                    }
                }
                IrSysFunc::FileTell(descriptor) => {
                    self.validate_expr(descriptor, formals, &format!("{path}.descriptor"))?;
                    if descriptor.is_real() || expr.width != 64 || !expr.signed {
                        return self.fail(path, "$ftell requires a signed 64-bit result");
                    }
                }
                IrSysFunc::FileSeek {
                    descriptor,
                    offset,
                    operation,
                } => {
                    for (name, value) in [
                        ("descriptor", descriptor),
                        ("offset", offset),
                        ("operation", operation),
                    ] {
                        self.validate_expr(value, formals, &format!("{path}.{name}"))?;
                        if value.is_real() {
                            return self.fail(path, "$fseek requires packed arguments");
                        }
                    }
                    if expr.width != 32 || !expr.signed {
                        return self.fail(path, "$fseek requires a signed 32-bit result");
                    }
                }
                IrSysFunc::FileError {
                    descriptor,
                    message,
                } => {
                    self.validate_expr(descriptor, formals, &format!("{path}.descriptor"))?;
                    if descriptor.is_real() || expr.width != 32 || !expr.signed {
                        return self.fail(path, "$ferror requires a signed 32-bit result");
                    }
                    if message.as_deref().is_some_and(str::is_empty) {
                        return self.fail(path, "$ferror message address must not be empty");
                    }
                }
                IrSysFunc::FileEof(descriptor) => {
                    self.validate_expr(descriptor, formals, &format!("{path}.descriptor"))?;
                    if descriptor.is_real() || expr.width != 32 || !expr.signed {
                        return self.fail(path, "$feof requires a signed 32-bit result");
                    }
                }
                IrSysFunc::FileInput(input) => {
                    if expr.width != 32 || !expr.signed {
                        return self
                            .fail(path, "file input functions require a signed 32-bit result");
                    }
                    self.validate_file_input(input, formals, path)?;
                }
            },
        }
        Ok(())
    }

    fn validate_file_input(
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
            } => {
                source.validate(self.model, self.string_return.get())?;
                let mut result = Ok(());
                source.expressions(&mut |child| {
                    result = result.clone().and_then(|_| {
                        self.validate_expr(child, formals, &format!("{path}.source"))
                    });
                });
                result?;
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

    fn validate_call_expr(
        &self,
        call: &IrCallExpr,
        formals: &[IrFormal],
        path: &str,
    ) -> ValidationResult {
        self.validate_call_target(call.f, &call.args, formals, path, false)?;
        let callee = &self.model.funcs[call.f];
        if let Some(virtual_call) = &call.virtual_call {
            self.validate_virtual_call(call.f, virtual_call, formals, path)?;
            if call.receiver.is_some() {
                return self.fail(path, "virtual-interface call cannot carry a class receiver");
            }
        } else if callee.receiver_class.is_some() {
            let receiver = call
                .receiver
                .as_ref()
                .ok_or_else(|| IrValidationError::new(path, "class method call has no receiver"))?;
            receiver.validate(self.model, formals, self.chandle_return.get())?;
        } else if call.receiver.is_some() {
            return self.fail(path, "non-method call cannot carry a receiver");
        }
        for (idx, arg) in call.args.iter().enumerate() {
            if let IrCallArg::OutTemp {
                init,
                writeback,
                storage_lhs,
                storage_read,
                selector_inits,
                ..
            } = arg
            {
                if let Some(init) = init {
                    self.validate_expr(init, formals, &format!("{path}.args[{idx}].init"))?;
                }
                self.validate_lhs(writeback, formals, &format!("{path}.args[{idx}].writeback"))?;
                if let Some(storage_lhs) = storage_lhs {
                    self.validate_lhs(
                        storage_lhs,
                        formals,
                        &format!("{path}.args[{idx}].storage_lhs"),
                    )?;
                }
                if let Some(storage_read) = storage_read {
                    self.validate_expr(
                        storage_read,
                        formals,
                        &format!("{path}.args[{idx}].storage_read"),
                    )?;
                }
                for (selector_idx, (_, width, signed, two_state, init)) in
                    selector_inits.iter().enumerate()
                {
                    if *width == 0 {
                        return self.fail(
                            format!("{path}.args[{idx}].selector_inits[{selector_idx}]"),
                            "selector initializer must be packed",
                        );
                    }
                    self.validate_width(
                        *width,
                        &format!("{path}.args[{idx}].selector_inits[{selector_idx}].width"),
                    )?;
                    if *two_state && init.is_real() {
                        return self.fail(
                            format!("{path}.args[{idx}].selector_inits[{selector_idx}]"),
                            "two-state selector initializer cannot be real",
                        );
                    }
                    if init.width != *width || init.signed != *signed {
                        return self.fail(
                            format!("{path}.args[{idx}].selector_inits[{selector_idx}]"),
                            "selector initializer type disagrees with its capture",
                        );
                    }
                    self.validate_expr(
                        init,
                        formals,
                        &format!("{path}.args[{idx}].selector_inits[{selector_idx}].init"),
                    )?;
                }
            }
        }
        Ok(())
    }

    fn validate_virtual_call(
        &self,
        function: usize,
        call: &crate::sim::ir::IrVirtualCall,
        formals: &[IrFormal],
        path: &str,
    ) -> ValidationResult {
        let interface = self
            .model
            .virtual_interfaces
            .get(call.interface)
            .ok_or_else(|| {
                IrValidationError::new(
                    path,
                    format!(
                        "virtual-interface descriptor {} is out of bounds",
                        call.interface
                    ),
                )
            })?;
        let method = interface.methods.get(call.method).ok_or_else(|| {
            IrValidationError::new(
                path,
                format!("virtual-interface method {} is out of bounds", call.method),
            )
        })?;
        if method.function != function {
            return self.fail(
                path,
                "virtual-interface method function disagrees with call target",
            );
        }
        call.receiver
            .validate(self.model, formals, self.chandle_return.get())
    }

    fn validate_call_target(
        &self,
        function: usize,
        args: &[IrCallArg],
        formals: &[IrFormal],
        path: &str,
        allow_object_return: bool,
    ) -> ValidationResult {
        let callee = self.model.funcs.get(function).ok_or_else(|| {
            IrValidationError::new(path, format!("function index {function} is out of bounds"))
        })?;
        if (!allow_object_return && (callee.ret_chandle || callee.ret_string))
            || callee.formals.iter().any(|formal| formal.event)
        {
            return self.fail(path, "non-integral subprogram requires its typed call path");
        }
        if args.len() != callee.formals.len() {
            return self.fail(
                path,
                format!(
                    "call has {} arguments for {} formals",
                    args.len(),
                    callee.formals.len()
                ),
            );
        }
        let parameter_order = callee
            .formals
            .iter()
            .filter(|formal| formal.is_address())
            .chain(callee.formals.iter().filter(|formal| !formal.is_address()));
        for (idx, (arg, formal)) in args.iter().zip(parameter_order).enumerate() {
            let arg_path = format!("{path}.args[{idx}]");
            match arg {
                IrCallArg::Val(_) if formal.is_address() => {
                    return self.fail(arg_path, "address formal requires an address argument");
                }
                IrCallArg::Val(expr) => {
                    if formal.chandle {
                        return self
                            .fail(arg_path, "chandle formal requires a typed pointer value");
                    }
                    self.validate_expr(expr, formals, &arg_path)?;
                    if formal.real != expr.is_real()
                        || expr.width != formal.width
                        || expr.signed != formal.signed
                    {
                        return self
                            .fail(arg_path, "input argument type disagrees with its formal");
                    }
                }
                IrCallArg::StringVal(value) => {
                    if formal.string && !formal.is_address() {
                        value.validate(self.model, self.string_return.get())?;
                    } else {
                        return self.fail(arg_path, "string value requires a string input formal");
                    }
                }
                IrCallArg::StringOutAddr(addr) => {
                    if !formal.string || !formal.is_out || formal.is_ref() {
                        return self.fail(arg_path, "string address requires output/inout formal");
                    }
                    if addr.is_empty() {
                        return self.fail(arg_path, "string output address must not be empty");
                    }
                }
                IrCallArg::StringRefAddr { addr, const_ref } => {
                    if !formal.string || !formal.is_ref() {
                        return self
                            .fail(arg_path, "string reference requires a string ref formal");
                    }
                    if addr.is_empty() || (*const_ref && !formal.const_ref) {
                        return self.fail(arg_path, "invalid string reference descriptor");
                    }
                }
                IrCallArg::ChandleVal(value) => {
                    if formal.chandle && !formal.is_address() {
                        value.validate(self.model, formals, None).map_err(|error| {
                            IrValidationError::new(arg_path.clone(), error.to_string())
                        })?;
                    } else {
                        return self.fail(arg_path, "typed chandle value requires an input formal");
                    }
                }
                IrCallArg::ChandleAddr(addr) => {
                    if !formal.chandle || !formal.is_out || formal.is_ref() {
                        return self.fail(arg_path, "typed chandle address requires output/inout");
                    }
                    if addr.is_empty() {
                        return self.fail(arg_path, "chandle output address must not be empty");
                    }
                }
                IrCallArg::ChandleRefAddr(addr) => {
                    if !formal.chandle || !formal.is_ref() {
                        return self.fail(arg_path, "typed chandle reference requires ref formal");
                    }
                    if addr.is_empty() {
                        return self.fail(arg_path, "chandle reference address must not be empty");
                    }
                }
                IrCallArg::RefAddr {
                    addr,
                    width,
                    signed,
                    two_state,
                    const_ref,
                    lhs,
                    read,
                } => {
                    if !formal.is_ref() {
                        return self
                            .fail(arg_path, "output/inout formal requires an output address");
                    }
                    if addr.is_empty() {
                        return self.fail(arg_path, "reference address must not be empty");
                    }
                    if *width != formal.width
                        || *signed != formal.signed
                        || *two_state != formal.two_state
                    {
                        return self.fail(
                            arg_path,
                            "reference argument type disagrees with its formal",
                        );
                    }
                    if *const_ref && !formal.const_ref {
                        return self.fail(
                            arg_path,
                            "const reference cannot bind to a writable ref formal",
                        );
                    }
                    self.validate_ref_actual_lhs(lhs, formals, &format!("{arg_path}.lhs"))?;
                    self.validate_expr(read, formals, &format!("{arg_path}.read"))?;
                    if read.width != *width || read.signed != *signed {
                        return self.fail(
                            format!("{arg_path}.read"),
                            "reference read type disagrees with its descriptor",
                        );
                    }
                }
                IrCallArg::OutAddr(_) | IrCallArg::OutTemp { .. } if formal.is_ref() => {
                    return self.fail(arg_path, "ref formal requires a reference descriptor");
                }
                IrCallArg::OutAddr(_) | IrCallArg::OutTemp { .. } if !formal.is_address() => {
                    return self.fail(arg_path, "input formal requires a value argument");
                }
                IrCallArg::OutTemp {
                    storage_addr: Some(addr),
                    ..
                } if addr.is_empty() => {
                    return self.fail(
                        arg_path,
                        "persistent output storage address must not be empty",
                    );
                }
                IrCallArg::OutTemp {
                    init: Some(init), ..
                } => {
                    if formal.real != init.is_real()
                        || init.width != formal.width
                        || init.signed != formal.signed
                    {
                        return self.fail(
                            format!("{arg_path}.init"),
                            "output/inout temp initializer type disagrees with its formal",
                        );
                    }
                }
                IrCallArg::StringOutTemp {
                    name,
                    init,
                    writeback,
                    storage_addr,
                    storage_read,
                } => {
                    if !formal.string || !formal.is_out || formal.is_ref() {
                        return self.fail(arg_path, "string temp requires output/inout formal");
                    }
                    if name.is_empty() || writeback.is_empty() {
                        return self.fail(arg_path, "string output temp has empty storage");
                    }
                    if let Some(init) = init {
                        init.validate(self.model, self.string_return.get())?;
                    }
                    if let Some(addr) = storage_addr {
                        if addr.is_empty() {
                            return self
                                .fail(arg_path, "string persistent storage address is empty");
                        }
                    }
                    if let Some(read) = storage_read {
                        read.validate(self.model, self.string_return.get())?;
                    }
                }
                IrCallArg::OutAddr(_) | IrCallArg::OutTemp { init: None, .. } => {}
            }
        }
        Ok(())
    }

    fn validate_ref_actual_lhs(
        &self,
        lhs: &IrLhs,
        formals: &[IrFormal],
        path: &str,
    ) -> ValidationResult {
        match lhs {
            IrLhs::Ref { addr, width, .. } => {
                if addr.is_empty() {
                    return self.fail(path, "reference descriptor address must not be empty");
                }
                self.validate_width(*width, path)
            }
            _ => self.validate_lhs(lhs, formals, path),
        }
    }

    fn validate_elem_sel(
        &self,
        sel: &IrElemSel,
        formals: &[IrFormal],
        path: &str,
    ) -> ValidationResult {
        match sel {
            IrElemSel::Whole => Ok(()),
            IrElemSel::Part(left, right) => self.validate_select_width(*left, *right, path),
            IrElemSel::Bit(expr) => self.validate_expr(expr, formals, path),
            IrElemSel::Indexed { base, width, .. } => {
                self.validate_expr(base, formals, &format!("{path}.base"))?;
                self.validate_width(*width, &format!("{path}.width"))
            }
        }
    }

    fn validate_select_width(&self, left: i64, right: i64, _path: &str) -> ValidationResult {
        let width = (i128::from(left) - i128::from(right)).unsigned_abs() + 1;
        self.max_width.set(self.max_width.get().max(width));
        Ok(())
    }

    fn lhs_packed_width(&self, lhs: &IrLhs) -> Option<u32> {
        let width = match lhs {
            IrLhs::Whole(signal) => self.model.signals.get(*signal)?.ty.width(),
            IrLhs::WholeRef { width, .. }
            | IrLhs::Ref { width, .. }
            | IrLhs::Stream { width, .. } => *width,
            IrLhs::Bit(..) => 1,
            IrLhs::Part(_, left, right, _) => ((left - right).abs() + 1) as u32,
            IrLhs::IdxPart(_, _, _, width, _, _) => *width,
            IrLhs::ArrayElem { arr, elem_sel, .. } => match elem_sel {
                IrElemSel::Whole => self.model.arrays.get(*arr)?.elem_width,
                IrElemSel::Part(left, right) => ((left - right).abs() + 1) as u32,
                IrElemSel::Bit(_) => 1,
                IrElemSel::Indexed { width, .. } => *width,
            },
        };
        (width != 0).then_some(width)
    }

    fn validate_stochastic_output(
        &self,
        lhs: &IrLhs,
        formals: &[IrFormal],
        path: &str,
    ) -> ValidationResult {
        self.validate_lhs(lhs, formals, path)?;
        match lhs {
            IrLhs::Whole(signal) => {
                let signal = &self.model.signals[*signal];
                if signal.net_driver.is_some() || !matches!(signal.ty, IrType::Packed { .. }) {
                    return self.fail(
                        path,
                        "stochastic queue output must be whole packed variable storage",
                    );
                }
            }
            IrLhs::WholeRef { width, .. } if *width != 0 => {}
            _ => {
                return self.fail(
                    path,
                    "stochastic queue output must be whole packed variable storage",
                )
            }
        }
        Ok(())
    }

    fn validate_lhs(&self, lhs: &IrLhs, formals: &[IrFormal], path: &str) -> ValidationResult {
        match lhs {
            IrLhs::Whole(signal) | IrLhs::Part(signal, ..) => {
                if *signal >= self.model.signals.len() {
                    return self.fail(path, format!("signal index {signal} is out of bounds"));
                }
                if let IrLhs::Part(_, left, right, _) = lhs {
                    self.validate_select_width(*left, *right, path)?;
                }
            }
            IrLhs::WholeRef { width, .. } => {
                if *width != 0 {
                    self.validate_width(*width, path)?;
                }
            }
            IrLhs::Ref {
                addr,
                width,
                const_ref,
                ..
            } => {
                if addr.is_empty() {
                    return self.fail(path, "reference descriptor address must not be empty");
                }
                if *const_ref {
                    return self.fail(path, "const reference cannot be an assignment target");
                }
                self.validate_width(*width, path)?;
            }
            IrLhs::Bit(signal, index, _) => {
                if *signal >= self.model.signals.len() {
                    return self.fail(path, format!("signal index {signal} is out of bounds"));
                }
                self.validate_expr(index, formals, &format!("{path}.index"))?;
            }
            IrLhs::IdxPart(signal, base, width_expr, width, _, _) => {
                if *signal >= self.model.signals.len() {
                    return self.fail(path, format!("signal index {signal} is out of bounds"));
                }
                self.validate_expr(base, formals, &format!("{path}.base"))?;
                self.validate_expr(width_expr, formals, &format!("{path}.width_expr"))?;
                self.validate_width(*width, &format!("{path}.selected_width"))?;
            }
            IrLhs::ArrayElem {
                arr,
                indices,
                elem_sel,
            } => {
                let array = self.model.arrays.get(*arr).ok_or_else(|| {
                    IrValidationError::new(path, format!("array index {arr} is out of bounds"))
                })?;
                if indices.len() != array.dims.len() {
                    return self.fail(path, "array LHS index count does not match its dimensions");
                }
                for (idx, index) in indices.iter().enumerate() {
                    self.validate_expr(index, formals, &format!("{path}.indices[{idx}]"))?;
                }
                self.validate_elem_sel(elem_sel, formals, &format!("{path}.elem_sel"))?;
            }
            IrLhs::Stream {
                parts,
                width,
                slice,
                ..
            } => {
                self.validate_width(*width, &format!("{path}.width"))?;
                if *slice == 0 || *slice > *width {
                    return self.fail(path, "streaming LHS slice must be in 1..=its packed width");
                }
                if parts.is_empty() {
                    return self.fail(path, "streaming LHS must contain at least one target");
                }
                let mut total = 0u32;
                for (index, (part, part_width)) in parts.iter().enumerate() {
                    self.validate_width(*part_width, &format!("{path}.parts[{index}].width"))?;
                    self.validate_lhs(part, formals, &format!("{path}.parts[{index}]"))?;
                    if self.lhs_packed_width(part) != Some(*part_width) {
                        return self.fail(
                            format!("{path}.parts[{index}].width"),
                            "streaming LHS part width disagrees with its target",
                        );
                    }
                    total = total.checked_add(*part_width).ok_or_else(|| {
                        IrValidationError::new(path, "streaming LHS width sum overflows u32")
                    })?;
                }
                if total != *width {
                    return self.fail(
                        path,
                        format!(
                            "streaming LHS part widths sum to {total}, not declared width {width}"
                        ),
                    );
                }
            }
        }
        Ok(())
    }

    fn validate_stmts(
        &self,
        stmts: &[IrStmt],
        formals: &[IrFormal],
        path: &str,
    ) -> ValidationResult {
        for (idx, stmt) in stmts.iter().enumerate() {
            self.validate_stmt(stmt, formals, &format!("{path}[{idx}]"))?;
        }
        Ok(())
    }

    fn validate_event_assignment_specs(
        &self,
        specs: &[(IrWaitSrc, IrEdge)],
        formals: &[IrFormal],
        path: &str,
    ) -> ValidationResult {
        for (idx, (source, edge)) in specs.iter().enumerate() {
            if let IrWaitSrc::Event(event) | IrWaitSrc::FilteredEvent { event, .. } = source {
                self.validate_event_ref(event, formals, &format!("{path}[{idx}].event"))?;
            }
            let helpers: Vec<(&str, bool)> = match source {
                IrWaitSrc::Evaluated {
                    eval, condition, ..
                } => std::iter::once((eval.as_str(), false))
                    .chain(condition.as_deref().map(|condition| (condition, false)))
                    .collect(),
                IrWaitSrc::EvaluatedReal {
                    eval, condition, ..
                } => std::iter::once((eval.as_str(), true))
                    .chain(condition.as_deref().map(|condition| (condition, false)))
                    .collect(),
                IrWaitSrc::FilteredEvent { condition, .. } => {
                    vec![(condition.as_str(), false)]
                }
                _ => Vec::new(),
            };
            if matches!(source, IrWaitSrc::Real(_)) && *edge != IrEdge::Any {
                return self.fail(
                    format!("{path}[{idx}]"),
                    "real event sources only support any-change controls",
                );
            }
            for (helper, real) in helpers {
                let valid = self
                    .model
                    .processes
                    .iter()
                    .flat_map(|process| &process.pre_fns)
                    .chain(
                        self.model
                            .funcs
                            .iter()
                            .flat_map(|function| &function.pre_fns),
                    )
                    .any(|pre| {
                        if real {
                            matches!(
                                pre,
                                IrPreFn::RealEval { c_name, value, .. }
                                    if c_name == helper && value.is_real()
                            )
                        } else {
                            matches!(
                                pre,
                                IrPreFn::MonEval { c_name, args, .. }
                                    if c_name == helper && args.len() == 1 && !args[0].is_real()
                            )
                        }
                    });
                if !valid {
                    return self.fail(
                        format!("{path}[{idx}]"),
                        "event evaluator helper has an invalid value type",
                    );
                }
            }
            if let IrWaitSrc::Real(name) = source {
                if !self.valid_dependency(&IrDependency::real(name)) {
                    return self.fail(
                        format!("{path}[{idx}]"),
                        "real event source must name active real storage",
                    );
                }
            }
            if let IrWaitSrc::Evaluated { reads, .. } | IrWaitSrc::EvaluatedReal { reads, .. } =
                source
            {
                for read in reads {
                    if !self.valid_dependency(read) {
                        return self.fail(
                            format!("{path}[{idx}]"),
                            "event dependency must name active storage",
                        );
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_stmt(&self, stmt: &IrStmt, formals: &[IrFormal], path: &str) -> ValidationResult {
        if let IrStmt::Delay { ticks }
        | IrStmt::DelayedAssign { ticks, .. }
        | IrStmt::ClockingDrive { ticks, .. }
        | IrStmt::DelayedStringAssign { ticks, .. } = stmt
        {
            if let IrDelay::Runtime {
                value,
                unit_ticks,
                precision_ticks,
            } = ticks
            {
                if *unit_ticks == 0 || *precision_ticks == 0 || unit_ticks % precision_ticks != 0 {
                    return self.fail(path, "runtime delay requires valid integral time scaling");
                }
                self.validate_expr(value, formals, &format!("{path}.delay"))?;
            }
        }
        match stmt {
            IrStmt::System(command) => {
                if let Some(command) = command {
                    command.validate(self.model, self.string_return.get())?;
                    let mut result = Ok(());
                    command.expressions(&mut |child| {
                        result = result
                            .clone()
                            .and_then(|_| self.validate_expr(child, formals, path));
                    });
                    result?;
                }
            }
            IrStmt::RandomSeed { seed } => {
                self.validate_expr(seed, formals, &format!("{path}.seed"))?;
                if seed.is_real() || (seed.width, seed.signed) != (32, false) {
                    return self.fail(path, "random seed must be a 32-bit unsigned value");
                }
            }
            IrStmt::RandomStateSet { state } => {
                state.validate(self.model, self.string_return.get())?;
                let mut result = Ok(());
                state.expressions(&mut |child| {
                    result = result
                        .clone()
                        .and_then(|_| self.validate_expr(child, formals, path));
                });
                result?;
            }
            IrStmt::Memory {
                path: file,
                array,
                start,
                finish,
                ..
            } => {
                file.validate(self.model, self.string_return.get())?;
                let mut result = Ok(());
                file.expressions(&mut |child| {
                    result = result
                        .clone()
                        .and_then(|_| self.validate_expr(child, formals, path));
                });
                result?;
                let Some(array) = self.model.arrays.get(*array) else {
                    return self.fail(path, "memory task array index is out of bounds");
                };
                if array.real {
                    return self.fail(path, "memory task does not support real arrays");
                }
                if array.dims.len() != 1 {
                    return self.fail(path, "memory task requires a one-dimensional array");
                }
                for (name, bound) in [("start", start), ("finish", finish)] {
                    if let Some(bound) = bound {
                        self.validate_expr(bound, formals, &format!("{path}.{name}"))?;
                        if bound.is_real() {
                            return self.fail(
                                format!("{path}.{name}"),
                                "memory task bound must be a packed integer",
                            );
                        }
                    }
                }
            }
            IrStmt::Container(operation) => {
                operation.validate(self.model, self.string_return.get())?;
                let mut result = Ok(());
                operation.expressions(&mut |child| {
                    result = result
                        .clone()
                        .and_then(|_| self.validate_expr(child, formals, path));
                });
                result?;
            }
            IrStmt::StreamAssign {
                source,
                slice,
                targets,
                ..
            } => {
                if *slice == 0 {
                    return self.fail(path, "streaming assignment slice size must be positive");
                }
                if source.is_real() {
                    return self.fail(path, "streaming assignment source must be packed");
                }
                if targets.is_empty() {
                    return self.fail(path, "streaming assignment requires a target");
                }
                self.validate_expr(source, formals, &format!("{path}.source"))?;
                let mut dynamic_targets = 0usize;
                for (index, target) in targets.iter().enumerate() {
                    match target {
                        IrStreamTarget::Packed { lhs, width } => {
                            self.validate_width(*width, &format!("{path}.targets[{index}].width"))?;
                            self.validate_lhs(
                                lhs,
                                formals,
                                &format!("{path}.targets[{index}].lhs"),
                            )?;
                            if self.lhs_packed_width(lhs) != Some(*width) {
                                return self.fail(
                                    format!("{path}.targets[{index}].width"),
                                    "streaming target width disagrees with its lvalue",
                                );
                            }
                        }
                        IrStreamTarget::Container {
                            container,
                            selector,
                        } => {
                            dynamic_targets += 1;
                            if dynamic_targets > 1 {
                                return self.fail(
                                    format!("{path}.targets[{index}]"),
                                    "streaming assignment supports at most one resizable target",
                                );
                            }
                            let container = container_kind(self.model, *container, None)?;
                            if !matches!(
                                container.kind,
                                IrContainerKind::Dynamic | IrContainerKind::Queue { .. }
                            ) || !container.element.is_packed()
                            {
                                return self.fail(
                                    format!("{path}.targets[{index}]"),
                                    "streaming target requires a packed dynamic array or queue",
                                );
                            }
                            if let Some(selector) = selector {
                                validate_stream_selector(selector).map_err(|error| {
                                    IrValidationError::new(
                                        format!("{path}.targets[{index}].selector"),
                                        error.detail(),
                                    )
                                })?;
                                match selector {
                                    IrStreamSelector::Index(bound) => self.validate_expr(
                                        bound,
                                        formals,
                                        &format!("{path}.targets[{index}].selector.index"),
                                    )?,
                                    IrStreamSelector::Range { left, right } => {
                                        self.validate_expr(
                                            left,
                                            formals,
                                            &format!("{path}.targets[{index}].selector.left"),
                                        )?;
                                        self.validate_expr(
                                            right,
                                            formals,
                                            &format!("{path}.targets[{index}].selector.right"),
                                        )?;
                                    }
                                    IrStreamSelector::Indexed { base, width, .. } => {
                                        self.validate_expr(
                                            base,
                                            formals,
                                            &format!("{path}.targets[{index}].selector.base"),
                                        )?;
                                        self.validate_expr(
                                            width,
                                            formals,
                                            &format!("{path}.targets[{index}].selector.width"),
                                        )?;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            IrStmt::Object(operation) => {
                operation.validate(
                    self.model,
                    formals,
                    self.chandle_return.get(),
                    self.string_return.get(),
                )?;
                let mut result = Ok(());
                operation.expressions(&mut |child| {
                    result = result.clone().and_then(|_| {
                        let string_value = matches!(
                            operation,
                            IrObjectStmt::StringPrint(..)
                                | IrObjectStmt::StringAssign(..)
                                | IrObjectStmt::StringAssignLocal(..)
                        );
                        if child.is_real()
                            && !string_value
                            && !matches!(operation, IrObjectStmt::StringRealtoa(..))
                        {
                            self.fail(path, "object statement requires packed operands")
                        } else {
                            self.validate_expr(child, formals, path)
                        }
                    });
                });
                result?;
            }
            IrStmt::PlusArg(expression) => {
                self.validate_expr(expression, formals, &format!("{path}.expression"))?;
            }
            IrStmt::Stochastic(operation) => match operation.as_ref() {
                IrStochasticStmt::Initialize {
                    q_id,
                    q_type,
                    max_length,
                    status,
                } => {
                    self.validate_expr(q_id, formals, &format!("{path}.q_id"))?;
                    self.validate_expr(q_type, formals, &format!("{path}.q_type"))?;
                    self.validate_expr(max_length, formals, &format!("{path}.max_length"))?;
                    self.validate_stochastic_output(status, formals, &format!("{path}.status"))?;
                    if q_id.is_real() || q_type.is_real() || max_length.is_real() {
                        return self.fail(path, "stochastic queue inputs must be packed integers");
                    }
                }
                IrStochasticStmt::Add {
                    q_id,
                    job_id,
                    inform_id,
                    status,
                } => {
                    self.validate_expr(q_id, formals, &format!("{path}.q_id"))?;
                    self.validate_expr(job_id, formals, &format!("{path}.job_id"))?;
                    self.validate_expr(inform_id, formals, &format!("{path}.inform_id"))?;
                    self.validate_stochastic_output(status, formals, &format!("{path}.status"))?;
                    if q_id.is_real() || job_id.is_real() || inform_id.is_real() {
                        return self.fail(path, "stochastic queue inputs must be packed integers");
                    }
                }
                IrStochasticStmt::Remove {
                    q_id,
                    job_id,
                    inform_id,
                    status,
                } => {
                    self.validate_expr(q_id, formals, &format!("{path}.q_id"))?;
                    self.validate_stochastic_output(job_id, formals, &format!("{path}.job_id"))?;
                    self.validate_stochastic_output(
                        inform_id,
                        formals,
                        &format!("{path}.inform_id"),
                    )?;
                    self.validate_stochastic_output(status, formals, &format!("{path}.status"))?;
                    if q_id.is_real() {
                        return self.fail(path, "stochastic queue inputs must be packed integers");
                    }
                }
                IrStochasticStmt::Exam {
                    q_id,
                    stat_code,
                    stat_value,
                    status,
                } => {
                    self.validate_expr(q_id, formals, &format!("{path}.q_id"))?;
                    self.validate_expr(stat_code, formals, &format!("{path}.stat_code"))?;
                    self.validate_stochastic_output(
                        stat_value,
                        formals,
                        &format!("{path}.stat_value"),
                    )?;
                    self.validate_stochastic_output(status, formals, &format!("{path}.status"))?;
                    if q_id.is_real() || stat_code.is_real() {
                        return self.fail(path, "stochastic queue inputs must be packed integers");
                    }
                }
            },
            IrStmt::Block(body) | IrStmt::Forever { body } => {
                self.validate_stmts(body, formals, &format!("{path}.body"))?;
            }
            IrStmt::ActivationScope { body, exit, .. } => {
                if exit.is_empty() || exit.starts_with("_llg_exec_") {
                    return self.fail(path, "activation scope has an invalid exit label");
                }
                self.validate_stmts(body, formals, &format!("{path}.body"))?;
            }
            IrStmt::DeclLocal {
                width,
                init,
                two_state,
                ..
            } => {
                if *width == 0 {
                    // Width zero is the IR representation for a real
                    // capture.  Unlike packed locals, a real capture must
                    // always have an explicitly typed initializer: allowing
                    // an omitted initializer would leave the generated C
                    // local uninitialized and would make its first capture
                    // depend on stack contents.
                    if *two_state || init.as_ref().is_none_or(|value| !value.is_real()) {
                        return self.fail(path, "real local capture requires a real initializer");
                    }
                } else {
                    self.validate_width(*width, &format!("{path}.width"))?;
                }
                if let Some(init) = init {
                    self.validate_expr(init, formals, &format!("{path}.init"))?;
                }
            }
            IrStmt::DeclString { name, init } => {
                if name.is_empty() {
                    return self.fail(path, "string local name must not be empty");
                }
                if let Some(init) = init {
                    init.validate(self.model, self.string_return.get())?;
                }
            }
            IrStmt::DelayedStringAssign { target, rhs, .. } => {
                if target.is_empty() {
                    return self.fail(path, "delayed string target must not be empty");
                }
                rhs.validate(self.model, self.string_return.get())?;
            }
            IrStmt::ClockingSample { source, sample, .. } => {
                let Some(source_signal) = self.model.signals.get(*source) else {
                    return self.fail(
                        path,
                        format!("clocking source index {source} is out of bounds"),
                    );
                };
                let Some(sample_signal) = self.model.signals.get(*sample) else {
                    return self.fail(
                        path,
                        format!("clocking sample index {sample} is out of bounds"),
                    );
                };
                if !matches!(source_signal.ty, IrType::Packed { .. })
                    || !matches!(sample_signal.ty, IrType::Packed { .. })
                    || source_signal.ty.width() != sample_signal.ty.width()
                {
                    return self.fail(
                        path,
                        "clocking sample source and destination must be matching packed signals",
                    );
                }
            }
            IrStmt::ClockingDrive {
                lhs, rhs, specs, ..
            } => {
                if specs.is_empty() {
                    return self.fail(path, "clocking drive requires an associated event");
                }
                for (index, (source, _)) in specs.iter().enumerate() {
                    match source {
                        IrWaitSrc::Sig(name) => {
                            if !self.valid_dependency(&IrDependency::scalar(name)) {
                                return self.fail(
                                    format!("{path}.specs[{index}]"),
                                    "clocking drive event must name active packed storage",
                                );
                            }
                        }
                        IrWaitSrc::Event(event) => self.validate_event_ref(
                            event,
                            formals,
                            &format!("{path}.specs[{index}].event"),
                        )?,
                        _ => {
                            return self.fail(
                                format!("{path}.specs[{index}]"),
                                "clocking drive event must be a signal or named event",
                            )
                        }
                    }
                }
                fn persistent(lhs: &IrLhs) -> bool {
                    match lhs {
                        IrLhs::WholeRef { .. } | IrLhs::Ref { .. } => false,
                        IrLhs::Stream { parts, .. } => {
                            parts.iter().all(|(part, _)| persistent(part))
                        }
                        _ => true,
                    }
                }
                if !persistent(lhs) {
                    return self.fail(path, "clocking drive requires persistent target storage");
                }
                self.validate_lhs(lhs, formals, &format!("{path}.lhs"))?;
                self.validate_expr(rhs, formals, &format!("{path}.rhs"))?;
            }
            IrStmt::Assign { lhs, rhs, .. }
            | IrStmt::DelayedAssign { lhs, rhs, .. }
            | IrStmt::InertialAssign { lhs, rhs, .. } => {
                if matches!(stmt, IrStmt::InertialAssign { .. }) {
                    let packed_driver = match lhs {
                        IrLhs::Whole(index)
                        | IrLhs::Bit(index, ..)
                        | IrLhs::Part(index, ..)
                        | IrLhs::IdxPart(index, ..) => self
                            .model
                            .signals
                            .get(*index)
                            .is_some_and(|signal| matches!(signal.ty, IrType::Packed { .. })),
                        IrLhs::ArrayElem { arr, .. } => {
                            self.model.arrays.get(*arr).is_some_and(|array| !array.real)
                        }
                        _ => false,
                    };
                    if !packed_driver || rhs.is_real() {
                        return self.fail(
                            path,
                            "inertial update requires a persistent packed target and packed value",
                        );
                    }
                }
                if matches!(stmt, IrStmt::DelayedAssign { .. }) {
                    fn persistent(lhs: &IrLhs) -> bool {
                        match lhs {
                            IrLhs::WholeRef { .. } | IrLhs::Ref { .. } => false,
                            IrLhs::Stream { parts, .. } => {
                                parts.iter().all(|(part, _)| persistent(part))
                            }
                            _ => true,
                        }
                    }
                    if !persistent(lhs) {
                        return self.fail(path, "delayed NBA requires persistent target storage");
                    }
                }
                self.validate_lhs(lhs, formals, &format!("{path}.lhs"))?;
                self.validate_expr(rhs, formals, &format!("{path}.rhs"))?;
            }
            IrStmt::EventAssign { target, source } => {
                self.validate_event_ref(target, formals, &format!("{path}.target"))?;
                if let Some(source) = source {
                    self.validate_event_ref(source, formals, &format!("{path}.source"))?;
                }
            }
            IrStmt::EventCapture { name, source } => {
                if name.is_empty() {
                    return self.fail(path, "captured event handle name must not be empty");
                }
                self.validate_event_ref(source, formals, &format!("{path}.source"))?;
            }
            IrStmt::PcaAssign {
                sig, enable, value, ..
            }
            | IrStmt::PcaDrive {
                sig, enable, value, ..
            } => {
                let Some(target) = self.model.signals.get(*sig) else {
                    return self.fail(path, format!("signal index {sig} is out of bounds"));
                };
                let Some(enable_signal) = self.model.signals.get(*enable) else {
                    return self.fail(path, format!("enable index {enable} is out of bounds"));
                };
                if !matches!(enable_signal.ty, IrType::Packed { width: 1, .. }) {
                    return self.fail(path, "procedural continuous enable must be one packed bit");
                }
                match target.ty {
                    IrType::Packed { .. }
                        if value.is_real() || value.width != target.ty.width() =>
                    {
                        return self.fail(
                            path,
                            "procedural continuous value must match its packed target width",
                        );
                    }
                    IrType::Packed { .. } | IrType::Real { .. } => {}
                }
                self.validate_expr(value, formals, &format!("{path}.value"))?;
            }
            IrStmt::PcaDeassign { sig } => {
                if *sig >= self.model.signals.len() {
                    return self.fail(path, format!("signal index {sig} is out of bounds"));
                }
            }
            IrStmt::If {
                cond, then_, els, ..
            } => {
                self.validate_expr(cond, formals, &format!("{path}.cond"))?;
                self.validate_stmts(then_, formals, &format!("{path}.then"))?;
                if let Some(els) = els {
                    self.validate_stmts(els, formals, &format!("{path}.else"))?;
                }
            }
            IrStmt::While { cond, body }
            | IrStmt::Repeat { count: cond, body }
            | IrStmt::WaitCond { cond, body, .. } => {
                self.validate_expr(cond, formals, &format!("{path}.cond"))?;
                self.validate_stmts(body, formals, &format!("{path}.body"))?;
            }
            IrStmt::WaitEventTriggered { event, body } => {
                self.validate_event_ref(event, formals, &format!("{path}.event"))?;
                self.validate_stmts(body, formals, &format!("{path}.body"))?;
            }
            IrStmt::WaitOrder {
                events,
                success,
                failure,
            } => {
                if events.is_empty() {
                    return self.fail(path, "wait_order requires at least one event");
                }
                for (index, event) in events.iter().enumerate() {
                    self.validate_event_ref(event, formals, &format!("{path}.events[{index}]"))?;
                }
                self.validate_stmts(success, formals, &format!("{path}.success"))?;
                self.validate_stmts(failure, formals, &format!("{path}.failure"))?;
            }
            IrStmt::For {
                init,
                cond,
                incr,
                body,
            } => {
                self.validate_stmts(init, formals, &format!("{path}.init"))?;
                self.validate_expr(cond, formals, &format!("{path}.cond"))?;
                self.validate_stmts(incr, formals, &format!("{path}.incr"))?;
                self.validate_stmts(body, formals, &format!("{path}.body"))?;
            }
            IrStmt::Case { sel, items, .. } => {
                self.validate_expr(sel, formals, &format!("{path}.sel"))?;
                for (item_idx, item) in items.iter().enumerate() {
                    for (expr_idx, expr) in item.exprs.iter().enumerate() {
                        self.validate_expr(
                            expr,
                            formals,
                            &format!("{path}.items[{item_idx}].exprs[{expr_idx}]"),
                        )?;
                    }
                    self.validate_stmts(
                        &item.body,
                        formals,
                        &format!("{path}.items[{item_idx}].body"),
                    )?;
                }
            }
            IrStmt::WaitEvents { specs } => {
                for (idx, (source, edge)) in specs.iter().enumerate() {
                    if let IrWaitSrc::Event(event) | IrWaitSrc::FilteredEvent { event, .. } = source
                    {
                        self.validate_event_ref(
                            event,
                            formals,
                            &format!("{path}.specs[{idx}].event"),
                        )?;
                    }
                    let helpers: Vec<(&str, bool)> = match source {
                        IrWaitSrc::Evaluated {
                            eval, condition, ..
                        } => std::iter::once((eval.as_str(), false))
                            .chain(condition.as_deref().map(|condition| (condition, false)))
                            .collect(),
                        IrWaitSrc::EvaluatedReal {
                            eval, condition, ..
                        } => std::iter::once((eval.as_str(), true))
                            .chain(condition.as_deref().map(|condition| (condition, false)))
                            .collect(),
                        IrWaitSrc::FilteredEvent { condition, .. } => {
                            vec![(condition.as_str(), false)]
                        }
                        _ => Vec::new(),
                    };
                    if matches!(source, IrWaitSrc::Real(_)) && *edge != IrEdge::Any {
                        return self.fail(
                            format!("{path}.specs[{idx}]"),
                            "real event sources only support any-change controls",
                        );
                    }
                    for (helper, real) in helpers {
                        let valid = self.model.processes.iter().flat_map(|process| &process.pre_fns)
                            .chain(self.model.funcs.iter().flat_map(|function| &function.pre_fns))
                            .any(|pre| if real {
                                matches!(pre, IrPreFn::RealEval { c_name, value, .. } if c_name == helper && value.is_real())
                            } else {
                                matches!(pre, IrPreFn::MonEval { c_name, args, .. } if c_name == helper && args.len() == 1 && !args[0].is_real())
                            });
                        if !valid {
                            return self.fail(
                                format!("{path}.specs[{idx}]"),
                                "event evaluator helper has an invalid value type",
                            );
                        }
                    }
                    if let IrWaitSrc::Real(name) = source {
                        if !self.valid_dependency(&IrDependency::real(name)) {
                            return self.fail(
                                format!("{path}.specs[{idx}]"),
                                "real event source must name active real storage",
                            );
                        }
                    }
                    if let IrWaitSrc::Evaluated { reads, .. }
                    | IrWaitSrc::EvaluatedReal { reads, .. } = source
                    {
                        for read in reads {
                            if !self.valid_dependency(read) {
                                return self.fail(
                                    format!("{path}.specs[{idx}]"),
                                    "event dependency must name active storage",
                                );
                            }
                        }
                    }
                }
            }
            IrStmt::ClockingCycleWait { count, specs } => {
                if specs.is_empty() {
                    return self.fail(path, "clocking cycle wait requires at least one event");
                }
                self.validate_expr(count, formals, &format!("{path}.count"))?;
                if count.is_real() {
                    return self.fail(path, "clocking cycle wait count must be packed");
                }
                for (index, (source, _)) in specs.iter().enumerate() {
                    match source {
                        IrWaitSrc::Sig(name) => {
                            if !self.valid_dependency(&IrDependency::scalar(name)) {
                                return self.fail(
                                    format!("{path}.specs[{index}]"),
                                    "clocking cycle signal must name active packed storage",
                                );
                            }
                        }
                        IrWaitSrc::Event(event) => self.validate_event_ref(
                            event,
                            formals,
                            &format!("{path}.specs[{index}].event"),
                        )?,
                        _ => {
                            return self.fail(
                                format!("{path}.specs[{index}]"),
                                "clocking cycle wait source must be a signal or named event",
                            )
                        }
                    }
                }
            }
            IrStmt::EventTrigger { ev } => {
                self.validate_event_ref(ev, formals, path)?;
            }
            IrStmt::NonblockingEventTrigger { ev, ticks } => {
                self.validate_event_ref(ev, formals, path)?;
                if let Some(IrDelay::Runtime { value, .. }) = ticks {
                    self.validate_expr(value, formals, &format!("{path}.ticks"))?;
                }
            }
            IrStmt::NonblockingEventTriggerWhen { ev, specs, repeat } => {
                self.validate_event_ref(ev, formals, path)?;
                if let Some(repeat) = repeat {
                    self.validate_expr(repeat, formals, &format!("{path}.repeat"))?;
                    if repeat.is_real() {
                        return self.fail(path, "repeat count must be packed");
                    }
                }
                for (idx, (source, edge)) in specs.iter().enumerate() {
                    if let IrWaitSrc::Event(event) | IrWaitSrc::FilteredEvent { event, .. } = source
                    {
                        self.validate_event_ref(
                            event,
                            formals,
                            &format!("{path}.specs[{idx}].event"),
                        )?;
                    }
                    let helpers: Vec<(&str, bool)> = match source {
                        IrWaitSrc::Evaluated {
                            eval, condition, ..
                        } => std::iter::once((eval.as_str(), false))
                            .chain(condition.as_deref().map(|condition| (condition, false)))
                            .collect(),
                        IrWaitSrc::EvaluatedReal {
                            eval, condition, ..
                        } => std::iter::once((eval.as_str(), true))
                            .chain(condition.as_deref().map(|condition| (condition, false)))
                            .collect(),
                        IrWaitSrc::FilteredEvent { condition, .. } => {
                            vec![(condition.as_str(), false)]
                        }
                        _ => Vec::new(),
                    };
                    if matches!(source, IrWaitSrc::Real(_)) && *edge != IrEdge::Any {
                        return self.fail(
                            format!("{path}.specs[{idx}]"),
                            "real event sources only support any-change controls",
                        );
                    }
                    for (helper, real) in helpers {
                        let valid = self.model.processes.iter().flat_map(|process| &process.pre_fns)
                            .chain(self.model.funcs.iter().flat_map(|function| &function.pre_fns))
                            .any(|pre| if real {
                                matches!(pre, IrPreFn::RealEval { c_name, value, .. } if c_name == helper && value.is_real())
                            } else {
                                matches!(pre, IrPreFn::MonEval { c_name, args, .. } if c_name == helper && args.len() == 1 && !args[0].is_real())
                            });
                        if !valid {
                            return self.fail(
                                format!("{path}.specs[{idx}]"),
                                "event evaluator helper has an invalid value type",
                            );
                        }
                    }
                    if let IrWaitSrc::Real(name) = source {
                        if !self.valid_dependency(&IrDependency::real(name)) {
                            return self.fail(
                                format!("{path}.specs[{idx}]"),
                                "real event source must name active real storage",
                            );
                        }
                    }
                    if let IrWaitSrc::Evaluated { reads, .. }
                    | IrWaitSrc::EvaluatedReal { reads, .. } = source
                    {
                        for read in reads {
                            if !self.valid_dependency(read) {
                                return self.fail(
                                    format!("{path}.specs[{idx}]"),
                                    "event dependency must name active storage",
                                );
                            }
                        }
                    }
                }
            }
            IrStmt::NonblockingEventAssignWhen {
                lhs,
                rhs,
                specs,
                repeat,
                action,
                frame,
                captures,
            } => {
                self.validate_lhs(lhs, formals, &format!("{path}.lhs"))?;
                self.validate_expr(rhs, formals, &format!("{path}.rhs"))?;
                if let Some(repeat) = repeat {
                    self.validate_expr(repeat, formals, &format!("{path}.repeat"))?;
                    if repeat.is_real() {
                        return self.fail(path, "repeat count must be packed");
                    }
                }
                self.validate_event_assignment_specs(specs, formals, &format!("{path}.specs"))?;
                let valid_action = self
                    .model
                    .processes
                    .iter()
                    .flat_map(|process| &process.pre_fns)
                    .chain(
                        self.model
                            .funcs
                            .iter()
                            .flat_map(|function| &function.pre_fns),
                    )
                    .any(|pre| {
                        matches!(
                            pre,
                            IrPreFn::EventAssign {
                                c_name,
                                frame: action_frame,
                                ..
                            } if c_name == action && action_frame == frame
                        )
                    });
                if !valid_action {
                    return self.fail(path, "event assignment callback is not declared");
                }
                let mut slots = HashSet::new();
                for (capture_idx, capture) in captures.iter().enumerate() {
                    if capture.storage().frame() != *frame
                        || !slots.insert(capture.storage().slot())
                    {
                        return self.fail(
                            format!("{path}.captures[{capture_idx}]"),
                            "event assignment captures must use unique slots in their frame",
                        );
                    }
                    self.validate_expr(
                        capture.initial(),
                        formals,
                        &format!("{path}.captures[{capture_idx}].initial"),
                    )?;
                }
            }
            IrStmt::Fork { branches, .. } => {
                let names: HashSet<&str> = branches.iter().map(|(name, _)| name.as_str()).collect();
                if names.len() != branches.len() {
                    return self.fail(path, "fork branch function names are not unique");
                }
            }
            IrStmt::CapturedFork { branches, .. } => {
                let names: HashSet<&str> = branches.iter().map(IrCapturedBranch::c_name).collect();
                if names.len() != branches.len() {
                    return self.fail(path, "captured fork branch function names are not unique");
                }
                for (branch_idx, branch) in branches.iter().enumerate() {
                    let mut slots = HashSet::new();
                    for (capture_idx, capture) in branch.captures().iter().enumerate() {
                        let capture_path =
                            format!("{path}.branches[{branch_idx}].captures[{capture_idx}]");
                        if capture.storage().frame() != branch.frame() {
                            return self.fail(
                                format!("{capture_path}.storage"),
                                "capture storage belongs to a different frame",
                            );
                        }
                        if capture.storage().slot() as usize >= branch.captures().len()
                            || !slots.insert(capture.storage().slot())
                        {
                            return self.fail(
                                format!("{capture_path}.storage.slot"),
                                "capture storage slot is invalid or duplicated",
                            );
                        }
                        self.validate_expr(
                            capture.initial(),
                            formals,
                            &format!("{capture_path}.initial"),
                        )?;
                    }
                }
            }
            IrStmt::Force {
                lhs, value, reads, ..
            } => {
                self.validate_lhs(lhs, formals, &format!("{path}.lhs"))?;
                self.validate_expr(value, formals, &format!("{path}.value"))?;
                let target_width = self.lhs_packed_width(lhs);
                let target_real = matches!(
                    lhs,
                    IrLhs::Whole(index)
                        if self
                            .model
                            .signals
                            .get(*index)
                            .is_some_and(|signal| matches!(signal.ty, IrType::Real { .. }))
                );
                if target_real != value.is_real()
                    || (!value.is_real() && target_width != Some(value.width))
                {
                    return self.fail(
                        format!("{path}.value"),
                        "force value shape does not match its target",
                    );
                }
                for (index, read) in reads.iter().enumerate() {
                    let Some(signal) = self.model.signals.get(*read) else {
                        return self.fail(
                            format!("{path}.reads[{index}]"),
                            "force dependency signal index is out of bounds",
                        );
                    };
                    if signal.omit
                        || (signal.ty.width() == 0 && !matches!(signal.ty, IrType::Real { .. }))
                    {
                        return self.fail(
                            format!("{path}.reads[{index}]"),
                            "force dependency does not name active scalar storage",
                        );
                    }
                }
            }
            IrStmt::Release { lhs } => {
                self.validate_lhs(lhs, formals, &format!("{path}.lhs"))?;
            }
            IrStmt::Display { args, .. } => {
                for (idx, (expr, _)) in args.iter().enumerate() {
                    self.validate_expr(expr, formals, &format!("{path}.args[{idx}]"))?;
                }
            }
            IrStmt::DisplayTyped {
                args,
                scope,
                descriptor,
                ..
            } => {
                if scope.is_empty() {
                    return self.fail(path, "typed display scope must not be empty");
                }
                if let Some(descriptor) = descriptor {
                    self.validate_expr(descriptor, formals, &format!("{path}.descriptor"))?;
                    if descriptor.is_real() {
                        return self.fail(path, "file display descriptor cannot be real");
                    }
                }
                for (idx, arg) in args.iter().enumerate() {
                    arg.validate(
                        self.model,
                        self.string_return.get(),
                        &format!("{path}.args[{idx}]"),
                    )?;
                    let mut result = Ok(());
                    arg.expressions(&mut |expression| {
                        result = result
                            .clone()
                            .and_then(|_| self.validate_expr(expression, formals, path));
                    });
                    result?;
                }
            }
            IrStmt::Severity {
                level,
                args,
                scope,
                location,
                fatal_finish_number,
                ..
            } => {
                if scope.is_empty() {
                    return self.fail(path, "severity scope must not be empty");
                }
                if location.is_empty() {
                    return self.fail(path, "severity source location must not be empty");
                }
                if level.is_fatal() {
                    let Some(finish_number) = fatal_finish_number else {
                        return self.fail(path, "fatal severity must carry a finish number");
                    };
                    if *finish_number > 2 {
                        return self.fail(path, "fatal severity finish number must be 0, 1, or 2");
                    }
                } else if fatal_finish_number.is_some() {
                    return self.fail(path, "non-fatal severity must not carry a finish number");
                }
                for (idx, arg) in args.iter().enumerate() {
                    arg.validate(
                        self.model,
                        self.string_return.get(),
                        &format!("{path}.args[{idx}]"),
                    )?;
                    let mut result = Ok(());
                    arg.expressions(&mut |expression| {
                        result = result
                            .clone()
                            .and_then(|_| self.validate_expr(expression, formals, path));
                    });
                    result?;
                }
            }
            IrStmt::MonitorSet { descriptor, .. } => {
                if let Some(descriptor) = descriptor {
                    self.validate_expr(descriptor, formals, &format!("{path}.descriptor"))?;
                    if descriptor.is_real() {
                        return self.fail(path, "file monitor descriptor cannot be real");
                    }
                }
            }
            IrStmt::FileControl { descriptor, op } => {
                if descriptor.is_none() && !matches!(op, crate::sim::ir::IrFileOp::Flush) {
                    return self.fail(path, "only file flush accepts an omitted descriptor");
                }
                if let Some(descriptor) = descriptor {
                    self.validate_expr(descriptor, formals, &format!("{path}.descriptor"))?;
                    if descriptor.is_real() {
                        return self.fail(path, "file control descriptor cannot be real");
                    }
                }
            }
            IrStmt::ImmediateAssertion {
                condition,
                if_true,
                if_false,
                location,
                ..
            } => {
                if location.is_empty() {
                    return self.fail(path, "assertion source location must not be empty");
                }
                self.validate_expr(condition, formals, &format!("{path}.condition"))?;
                if let Some(if_true) = if_true {
                    self.validate_stmts(if_true, formals, &format!("{path}.if_true"))?;
                }
                if let Some(if_false) = if_false {
                    self.validate_stmts(if_false, formals, &format!("{path}.if_false"))?;
                }
            }
            IrStmt::DeferredImmediateAssertion {
                condition,
                if_true,
                if_false,
                location,
                ..
            } => {
                if location.is_empty() {
                    return self.fail(path, "assertion source location must not be empty");
                }
                self.validate_expr(condition, formals, &format!("{path}.condition"))?;
                for (arm, arm_name) in [(if_true, "if_true"), (if_false, "if_false")] {
                    let Some(arm) = arm else { continue };
                    if arm.c_name().is_empty() {
                        return self.fail(
                            format!("{path}.{arm_name}"),
                            "deferred assertion callback name must not be empty",
                        );
                    }
                    let callback = self
                        .model
                        .processes
                        .iter()
                        .flat_map(|process| &process.pre_fns)
                        .chain(
                            self.model
                                .funcs
                                .iter()
                                .flat_map(|function| &function.pre_fns),
                        )
                        .find(|pre| {
                            matches!(
                                pre,
                                IrPreFn::DeferredAssertion {
                                    c_name,
                                    frame,
                                    ..
                                } if c_name == arm.c_name()
                                    && *frame == arm.frame()
                            )
                        });
                    if callback.is_none() {
                        return self.fail(
                            format!("{path}.{arm_name}"),
                            "deferred assertion callback is not declared",
                        );
                    }
                }
            }
            IrStmt::WaveLimit(expr) => {
                self.validate_expr(expr, formals, &format!("{path}.limit"))?;
            }
            IrStmt::PrintTimescale {
                unit_fs,
                precision_fs,
                ..
            } => {
                if *unit_fs == 0 || *precision_fs == 0 {
                    return self.fail(path, "timescale units must be non-zero");
                }
            }
            IrStmt::TimeFormat {
                units,
                precision,
                suffix,
                minimum_field_width,
            } => {
                for (label, value) in [
                    ("units", units),
                    ("precision", precision),
                    ("minimum field width", minimum_field_width),
                ] {
                    self.validate_expr(value, formals, &format!("{path}.{label}"))?;
                    if value.is_real() {
                        return self.fail(
                            format!("{path}.{label}"),
                            "timeformat argument must be packed",
                        );
                    }
                }
                suffix.validate(self.model, self.string_return.get())?;
            }
            IrStmt::Call(call) => {
                self.validate_call_target(call.f, &call.args, formals, path, true)?;
                let callee = &self.model.funcs[call.f];
                if let Some(virtual_call) = &call.virtual_call {
                    self.validate_virtual_call(call.f, virtual_call, formals, path)?;
                    if call.receiver.is_some() {
                        return self
                            .fail(path, "virtual-interface call cannot carry a class receiver");
                    }
                } else if callee.receiver_class.is_some() {
                    let receiver = call.receiver.as_ref().ok_or_else(|| {
                        IrValidationError::new(path, "class method call has no receiver")
                    })?;
                    receiver.validate(self.model, formals, self.chandle_return.get())?;
                } else if call.receiver.is_some() {
                    return self.fail(path, "non-method call cannot carry a receiver");
                }
                for (idx, (_, formal, init)) in call.temps.iter().enumerate() {
                    let formal_ty = callee.formals.get(*formal).ok_or_else(|| {
                        IrValidationError::new(
                            format!("{path}.temps[{idx}]"),
                            format!("formal index {formal} is out of bounds"),
                        )
                    })?;
                    if !formal_ty.is_out {
                        return self.fail(
                            format!("{path}.temps[{idx}]"),
                            "call temp refers to an input formal",
                        );
                    }
                    if let Some(init) = init {
                        self.validate_expr(init, formals, &format!("{path}.temps[{idx}].init"))?;
                        if init.width != formal_ty.width || init.signed != formal_ty.signed {
                            return self.fail(
                                format!("{path}.temps[{idx}].init"),
                                "call temp initializer type disagrees with its formal",
                            );
                        }
                    }
                }
                for (idx, (lhs, _, width, _)) in call.copyouts.iter().enumerate() {
                    if *width == 0 {
                        let is_real_target = match lhs {
                            IrLhs::Whole(signal) => self
                                .model
                                .signals
                                .get(*signal)
                                .is_some_and(|signal| matches!(signal.ty, IrType::Real { .. })),
                            IrLhs::WholeRef { width, .. } => *width == 0,
                            _ => false,
                        };
                        if !is_real_target {
                            return self.fail(
                                format!("{path}.copyouts[{idx}].width"),
                                "zero-width call copyout requires a real target",
                            );
                        }
                    } else {
                        self.validate_width(*width, &format!("{path}.copyouts[{idx}].width"))?;
                    }
                    self.validate_lhs(lhs, formals, &format!("{path}.copyouts[{idx}].lhs"))?;
                }
            }
            IrStmt::Return { value } => {
                if let Some(value) = value {
                    if self.string_return.get() == Some(true)
                        || self.chandle_return.get() == Some(true)
                    {
                        return self.fail(
                            path,
                            "non-integral return requires its typed return storage",
                        );
                    }
                    self.validate_expr(value, formals, &format!("{path}.value"))?;
                }
            }
            IrStmt::Delay { .. }
            | IrStmt::WaitAny { .. }
            | IrStmt::WaitFork
            | IrStmt::DisableFork
            | IrStmt::DisableTarget { .. }
            | IrStmt::MonitorEnable(_)
            | IrStmt::WaveFile(_)
            | IrStmt::WaveDumpVars(_)
            | IrStmt::WaveOn
            | IrStmt::WaveOff
            | IrStmt::WaveDumpAll
            | IrStmt::WaveFlush
            | IrStmt::Finish
            | IrStmt::FinishControl { .. }
            | IrStmt::ProgramExit
            | IrStmt::StopControl { .. }
            | IrStmt::Label(_)
            | IrStmt::Goto(_)
            | IrStmt::Nop => {}
        }
        Ok(())
    }

    fn validate_pre_fns(
        &self,
        pre_fns: &[IrPreFn],
        formals: &[IrFormal],
        path: &str,
    ) -> ValidationResult {
        let saved_return = self.chandle_return.replace(None);
        let saved_string_return = self.string_return.replace(None);
        let branch_formals = &[];
        for (idx, pre_fn) in pre_fns.iter().enumerate() {
            match pre_fn {
                IrPreFn::Branch { body, .. } => {
                    self.validate_stmts(
                        body,
                        branch_formals,
                        &format!("{path}.pre_fns[{idx}].body"),
                    )?;
                }
                IrPreFn::CapturedBranch { captures, body, .. } => {
                    for (capture_idx, capture) in captures.iter().enumerate() {
                        self.validate_expr(
                            capture.initial(),
                            formals,
                            &format!("{path}.pre_fns[{idx}].captures[{capture_idx}].initial"),
                        )?;
                    }
                    self.validate_stmts(
                        body,
                        branch_formals,
                        &format!("{path}.pre_fns[{idx}].body"),
                    )?;
                }
                IrPreFn::MonEval { args, context, .. } => {
                    for (arg_idx, arg) in args.iter().enumerate() {
                        self.validate_expr(
                            arg,
                            branch_formals,
                            &format!("{path}.pre_fns[{idx}].args[{arg_idx}]"),
                        )?;
                    }
                    if let Some(context) = context {
                        for (capture_idx, capture) in context.captures().iter().enumerate() {
                            self.validate_expr(
                                capture.initial(),
                                formals,
                                &format!(
                                    "{path}.pre_fns[{idx}].context.captures[{capture_idx}].initial"
                                ),
                            )?;
                        }
                    }
                }
                IrPreFn::EventAssign {
                    frame: pre_frame,
                    captures,
                    lhs,
                    rhs,
                    ..
                } => {
                    let mut slots = HashSet::new();
                    for (capture_idx, capture) in captures.iter().enumerate() {
                        if capture.storage().frame() != *pre_frame
                            || !slots.insert(capture.storage().slot())
                        {
                            return self.fail(
                                format!("{path}.pre_fns[{idx}].captures[{capture_idx}]"),
                                "event assignment captures must use unique slots in their frame",
                            );
                        }
                        self.validate_expr(
                            capture.initial(),
                            formals,
                            &format!("{path}.pre_fns[{idx}].captures[{capture_idx}].initial"),
                        )?;
                    }
                    self.validate_lhs(lhs, branch_formals, &format!("{path}.pre_fns[{idx}].lhs"))?;
                    self.validate_expr(rhs, branch_formals, &format!("{path}.pre_fns[{idx}].rhs"))?;
                }
                IrPreFn::DeferredAssertion {
                    frame: pre_frame,
                    captures,
                    body,
                    ..
                } => {
                    let mut slots = HashSet::new();
                    for (capture_idx, capture) in captures.iter().enumerate() {
                        if capture.storage().frame() != *pre_frame
                            || !slots.insert(capture.storage().slot())
                        {
                            return self.fail(
                                format!("{path}.pre_fns[{idx}].captures[{capture_idx}]"),
                                "deferred assertion captures must use unique slots in their frame",
                            );
                        }
                        self.validate_expr(
                            capture.initial(),
                            formals,
                            &format!("{path}.pre_fns[{idx}].captures[{capture_idx}].initial"),
                        )?;
                    }
                    self.validate_stmts(
                        body,
                        branch_formals,
                        &format!("{path}.pre_fns[{idx}].body"),
                    )?;
                }
                IrPreFn::DisplayEval { args, .. } => {
                    for (arg_idx, arg) in args.iter().enumerate() {
                        arg.validate(
                            self.model,
                            self.string_return.get(),
                            &format!("{path}.pre_fns[{idx}].args[{arg_idx}]"),
                        )?;
                        let mut result = Ok(());
                        arg.expressions(&mut |expression| {
                            result = result.clone().and_then(|_| {
                                self.validate_expr(
                                    expression,
                                    branch_formals,
                                    &format!("{path}.pre_fns[{idx}].args[{arg_idx}]"),
                                )
                            });
                        });
                        result?;
                    }
                }
                IrPreFn::RealEval { value, context, .. } => {
                    if !value.is_real() {
                        return self.fail(
                            format!("{path}.pre_fns[{idx}].value"),
                            "real event evaluator requires a real expression",
                        );
                    }
                    self.validate_expr(
                        value,
                        branch_formals,
                        &format!("{path}.pre_fns[{idx}].value"),
                    )?;
                    if let Some(context) = context {
                        for (capture_idx, capture) in context.captures().iter().enumerate() {
                            self.validate_expr(
                                capture.initial(),
                                formals,
                                &format!(
                                    "{path}.pre_fns[{idx}].context.captures[{capture_idx}].initial"
                                ),
                            )?;
                        }
                    }
                }
                IrPreFn::ForceEval { value, real, .. } => {
                    if *real != value.is_real() {
                        return self.fail(
                            format!("{path}.pre_fns[{idx}].value"),
                            "force evaluator real flag disagrees with its value",
                        );
                    }
                    self.validate_expr(
                        value,
                        branch_formals,
                        &format!("{path}.pre_fns[{idx}].value"),
                    )?;
                }
            }
        }
        self.chandle_return.set(saved_return);
        self.string_return.set(saved_string_return);
        Ok(())
    }

    fn validate_init_step(&self, step: &IrInitStep, path: &str) -> ValidationResult {
        match step {
            IrInitStep::FillArrayX(array)
            | IrInitStep::FillArrayZ(array)
            | IrInitStep::SetArrayElem { arr: array, .. } => {
                if *array >= self.model.arrays.len() {
                    return self.fail(path, format!("array index {array} is out of bounds"));
                }
            }
            IrInitStep::SetScalar { sig, .. } => {
                if *sig >= self.model.signals.len() {
                    return self.fail(path, format!("signal index {sig} is out of bounds"));
                }
            }
            IrInitStep::RegisterSampled(sig) => {
                let Some(signal) = self.model.signals.get(*sig) else {
                    return self.fail(path, format!("sampled signal index {sig} is out of bounds"));
                };
                if !matches!(signal.ty, IrType::Packed { .. }) {
                    return self.fail(path, "sampled source must be a packed signal");
                }
            }
            IrInitStep::Initialize(initialization) => match &initialization.target {
                IrInitTarget::Signal(signal) => {
                    if *signal >= self.model.signals.len() {
                        return self.fail(path, format!("signal index {signal} is out of bounds"));
                    }
                }
                IrInitTarget::StaticLocal { function, name } => {
                    let function_ref = self.model.funcs.get(*function).ok_or_else(|| {
                        IrValidationError::new(
                            path,
                            format!("function index {function} is out of bounds"),
                        )
                    })?;
                    if !function_ref
                        .locals
                        .iter()
                        .any(|local| local.c_name() == name)
                    {
                        return self.fail(
                            path,
                            format!("static local `{name}` is not present in function {function}"),
                        );
                    }
                }
            },
            IrInitStep::WriteNet { group, slot, .. } => {
                let net = self.model.net_groups.get(*group).ok_or_else(|| {
                    IrValidationError::new(
                        path,
                        format!("net-group index {group} is out of bounds"),
                    )
                })?;
                if *slot >= net.n_drivers {
                    return self.fail(path, format!("driver slot {slot} is out of bounds"));
                }
            }
        }
        match step {
            IrInitStep::SetArrayElem { value, .. }
            | IrInitStep::SetScalar { value, .. }
            | IrInitStep::WriteNet { value, .. } => self.validate_const(value, path),
            IrInitStep::FillArrayX(_) => Ok(()),
            IrInitStep::RegisterSampled(_) => Ok(()),
            IrInitStep::FillArrayZ(array) => {
                if self.model.arrays[*array].two_state {
                    self.fail(path, "Z initialization requires four-state array elements")
                } else {
                    Ok(())
                }
            }
            IrInitStep::Initialize(initialization) => {
                if initialization.lifetime != StorageLifetime::Static {
                    return self.fail(
                        path,
                        "declaration initialization must target static storage",
                    );
                }
                match &initialization.target {
                    IrInitTarget::Signal(signal) => {
                        let ty = self.model.signal(*signal).ty;
                        match ty {
                            IrType::Real { .. } => {
                                if !initialization.value.is_real() {
                                    return self.fail(
                                        path,
                                        "real declaration initializer must produce a real value",
                                    );
                                }
                            }
                            IrType::Packed { width, signed, .. } => {
                                if initialization.value.is_real()
                                    || initialization.value.width != width
                                    || initialization.value.signed != signed
                                {
                                    return self.fail(
                                        path,
                                        "signal declaration initializer type disagrees with its target",
                                    );
                                }
                            }
                        }
                    }
                    IrInitTarget::StaticLocal { function, name } => {
                        let local = self
                            .model
                            .func(*function)
                            .locals
                            .iter()
                            .find(|local| local.c_name() == name);
                        let local = local.ok_or_else(|| {
                            IrValidationError::new(
                                path,
                                format!(
                                    "static local `{name}` is not present in function {function}"
                                ),
                            )
                        })?;
                        if initialization.value.is_real() != local.real
                            || initialization.value.width != local.width()
                            || initialization.value.signed != local.signed()
                        {
                            return self.fail(
                                path,
                                "static local initializer type disagrees with its target",
                            );
                        }
                    }
                }
                self.validate_expr(&initialization.value, &[], &format!("{path}.value"))
            }
        }
    }

    fn validate_spawns(&self, spawns: &[String], path: &str) -> ValidationResult {
        let processes: HashSet<&str> = self
            .model
            .processes
            .iter()
            .map(|process| process.c_name.as_str())
            .collect();
        let mut seen = HashSet::new();
        for (idx, spawn) in spawns.iter().enumerate() {
            if !processes.contains(spawn.as_str()) {
                return self.fail(
                    format!("{path}[{idx}]"),
                    format!("process `{spawn}` does not exist"),
                );
            }
            if !seen.insert(spawn.as_str()) {
                return self.fail(format!("{path}[{idx}]"), "duplicate process registration");
            }
        }
        Ok(())
    }

    fn fail<T>(
        &self,
        path: impl Into<String>,
        detail: impl Into<String>,
    ) -> Result<T, IrValidationError> {
        Err(IrValidationError::new(path, detail))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_model() -> IrModel {
        let mut model = IrModel::new("top".to_string(), 1).unwrap();
        model.signals = vec![IrSignal {
            c_name: "sig".to_string(),
            hdl_name: Some("sig".to_string()),
            ty: IrType::Packed {
                width: 1,
                signed: false,
                two_state: false,
            },
            net_driver: None,
            net_alias: Vec::new(),
            alias: None,
            omit: false,
        }];
        model
    }

    fn packed_const(value: u64, width: u32) -> IrExpr {
        IrExpr::new(
            IrExprKind::Const(
                IrConst::packed(vec![value], vec![], vec![], width, false, None).unwrap(),
            ),
            width,
            false,
            None,
        )
    }

    #[test]
    fn rejects_out_of_bounds_signal_reference() {
        let mut model = valid_model();
        model.processes.push(IrProcess {
            c_name: "proc".to_string(),
            label: "top.initial".to_string(),
            kind: IrProcessKind::Synthetic,
            shape: IrShape::RunOnce,
            writes: Vec::new(),
            pre_fns: Vec::new(),
            body: vec![IrStmt::Release {
                lhs: IrLhs::Whole(1),
            }],
            program: false,
            origin: crate::sim::semantic::Origin::Synthetic {
                reason: "validation fixture".to_owned(),
            },
        });
        model.spawns.push("proc".to_string());

        let error = model
            .validate()
            .expect_err("invalid signal index must fail");
        assert_eq!(error.path(), "processes[0].body[0].lhs");
        assert!(error.detail().contains("signal index 1"));
    }

    #[test]
    fn rejects_array_total_that_disagrees_with_dimensions() {
        let mut model = valid_model();
        model.arrays.push(IrArray {
            c_name: "memory".to_string(),
            hdl_name: "memory".to_string(),
            elem_width: 8,
            signed: false,
            two_state: false,
            real: false,
            shortreal: false,
            dims: vec![(3, 0), (1, 0)],
            total: 7,
        });

        let error = model.validate().expect_err("invalid array total must fail");
        assert_eq!(error.path(), "arrays[0].total");
        assert!(error.detail().contains("dimension product 8"));
    }

    #[test]
    fn accepts_a_minimal_well_formed_model() {
        valid_model().validate().expect("minimal model is valid");
    }

    #[test]
    fn runtime_delays_validate_scaling_and_include_expression_capacity() {
        let model = valid_model();
        let delay = |unit_ticks, precision_ticks| IrStmt::Delay {
            ticks: IrDelay::Runtime {
                value: Box::new(packed_const(1, 129)),
                unit_ticks,
                precision_ticks,
            },
        };
        assert_eq!(
            model.statement_capacity(&delay(1000, 100), None).unwrap(),
            129
        );
        for (unit, precision) in [(0, 1), (1, 0), (3, 2), (1, 10)] {
            assert!(model.validate_stmt(&delay(unit, precision), None).is_err());
        }
    }

    #[test]
    fn real_local_captures_require_real_initializers() {
        let model = valid_model();
        let local = |init| IrStmt::DeclLocal {
            name: "capture".into(),
            width: 0,
            signed: true,
            two_state: false,
            init,
        };
        assert!(model.validate_stmt(&local(None), None).is_err());
        assert!(model
            .validate_stmt(&local(Some(Box::new(packed_const(1, 32)))), None)
            .is_err());
        let real = IrExpr::new(IrExprKind::Const(IrConst::real(1.25)), 0, false, None);
        model
            .validate_stmt(&local(Some(Box::new(real))), None)
            .unwrap();
    }

    #[test]
    fn delayed_nba_rejects_unproven_pointer_lifetimes() {
        let statement = IrStmt::DelayedAssign {
            lhs: IrLhs::WholeRef {
                addr: "&local".into(),
                width: 1,
                signed: false,
                two_state: false,
                shortreal: false,
            },
            rhs: packed_const(1, 1),
            ticks: IrDelay::Constant(2),
        };
        let error = valid_model().validate_stmt(&statement, None).unwrap_err();
        assert!(error.detail().contains("persistent"));
    }

    #[test]
    fn inertial_updates_require_persistent_packed_drivers() {
        let model = valid_model();
        let statement = |lhs, rhs| IrStmt::InertialAssign {
            lhs,
            rhs,
            delay: IrTransitionDelay::uniform(2),
        };
        assert_eq!(
            model
                .statement_capacity(&statement(IrLhs::Whole(0), packed_const(1, 129)), None,)
                .unwrap(),
            129
        );
        for lhs in [
            IrLhs::Whole(1),
            IrLhs::WholeRef {
                addr: "&local".into(),
                width: 1,
                signed: false,
                two_state: false,
                shortreal: false,
            },
        ] {
            assert!(model
                .validate_stmt(&statement(lhs, packed_const(1, 1)), None)
                .is_err());
        }
        let real = IrExpr::new(IrExprKind::Const(IrConst::real(1.0)), 0, false, None);
        assert!(model
            .validate_stmt(&statement(IrLhs::Whole(0), real), None)
            .is_err());
    }

    #[test]
    fn math_and_realtime_require_valid_shapes_and_units() {
        let model = valid_model();
        let math = |args, width| {
            IrExpr::new(
                IrExprKind::SysFunc(IrSysFunc::Math {
                    kind: IrMathFunc::Pow,
                    args,
                }),
                width,
                true,
                None,
            )
        };
        model
            .validate_expr(&math(vec![packed_const(2, 2), packed_const(3, 2)], 0), None)
            .unwrap();
        assert!(model
            .validate_expr(&math(vec![packed_const(2, 2)], 0), None)
            .is_err());
        assert!(model
            .validate_expr(
                &math(vec![packed_const(2, 2), packed_const(3, 2)], 32),
                None
            )
            .is_err());
        let time = IrExpr::new(
            IrExprKind::SysFunc(IrSysFunc::Realtime {
                precision_fs: 1,
                unit_fs: 0,
            }),
            0,
            true,
            None,
        );
        assert!(model.validate_expr(&time, None).is_err());
    }

    #[test]
    fn variable_aliases_require_matching_canonical_storage() {
        let mut model = valid_model();
        let mut alias = model.signals[0].clone();
        alias.hdl_name = Some("child.sig".into());
        alias.alias = Some(0);
        model.signals.push(alias);
        model.validate().expect("alias shares canonical storage");

        for target in [1, 2] {
            model.signals[1].alias = Some(target);
            assert!(model.validate().unwrap_err().path().ends_with(".alias"));
        }
        model.signals[1].alias = Some(0);
        model.signals[0].omit = true;
        assert!(model.validate().is_err(), "live alias retains its target");
        model.signals[0].omit = false;
        model.signals[1].c_name = "other".into();
        assert!(model.validate().is_err(), "alias uses the same C storage");
    }

    #[test]
    fn evaluated_waits_require_valid_helpers_and_dependencies() {
        let mut model = valid_model();
        let process = IrProcess::new(
            "proc".into(),
            "top.initial".into(),
            IrShape::RunOnce,
            vec![IrPreFn::MonEval {
                c_name: "eval".into(),
                args: vec![packed_const(1, 1)],
                context: None,
                item: false,
            }],
            vec![IrStmt::WaitEvents {
                specs: vec![(
                    IrWaitSrc::Evaluated {
                        eval: "eval".into(),
                        condition: None,
                        reads: vec!["sig".into()],
                    },
                    IrEdge::Any,
                )],
            }],
        );
        model.processes.push(process);
        model.spawns.push("proc".into());
        model.validate().expect("valid expression wait");
        model.processes[0].pre_fns.clear();
        assert!(model.validate().unwrap_err().detail().contains("evaluator"));
        model.processes[0].pre_fns.push(IrPreFn::MonEval {
            c_name: "eval".into(),
            args: vec![packed_const(1, 1)],
            context: None,
            item: false,
        });
        model.signals[0].omit = true;
        assert!(model
            .validate()
            .unwrap_err()
            .detail()
            .contains("dependency"));
    }

    #[test]
    fn z_array_initialization_requires_valid_four_state_storage() {
        let mut model = valid_model();
        model.init_steps.push(IrInitStep::FillArrayZ(0));
        assert!(model
            .validate()
            .unwrap_err()
            .detail()
            .contains("array index"));
        model
            .arrays
            .push(IrArray::new("array".into(), "array".into(), 65, false, vec![(0, 1)]).unwrap());
        model.validate().expect("four-state Z initialization");
        model.arrays[0].two_state = true;
        assert!(model
            .validate()
            .unwrap_err()
            .detail()
            .contains("four-state"));
    }

    #[test]
    fn indexed_lhs_selected_width_contributes_to_capacity() {
        let statement = IrStmt::Assign {
            lhs: IrLhs::IdxPart(
                0,
                packed_const(0, 32),
                packed_const(96, 32),
                96,
                false,
                false,
            ),
            rhs: packed_const(1, 1),
            nba: false,
        };

        assert_eq!(
            valid_model().statement_capacity(&statement, None).unwrap(),
            96
        );
    }

    #[test]
    fn streaming_lhs_explicit_width_contributes_to_capacity() {
        let statement = IrStmt::Assign {
            lhs: IrLhs::Stream {
                parts: vec![
                    (IrLhs::Part(0, 63, 0, false), 64),
                    (IrLhs::Part(0, 31, 0, false), 32),
                ],
                width: 96,
                slice: 8,
                direction: IrStreamDirection::RightToLeft,
            },
            rhs: packed_const(1, 1),
            nba: false,
        };

        assert_eq!(
            valid_model().statement_capacity(&statement, None).unwrap(),
            96
        );
    }

    #[test]
    fn indexed_lhs_preserves_wide_base_expression_capacity() {
        let part = || packed_const(0, 32);
        let base = IrExpr::new(
            IrExprKind::Concat {
                parts: vec![part(), part(), part()],
            },
            96,
            false,
            None,
        );
        let statement = IrStmt::Assign {
            lhs: IrLhs::IdxPart(0, base, packed_const(8, 32), 8, false, false),
            rhs: packed_const(1, 1),
            nba: false,
        };

        assert_eq!(
            valid_model().statement_capacity(&statement, None).unwrap(),
            96
        );
    }

    #[test]
    fn streaming_and_inside_children_contribute_to_capacity() {
        let stream = IrExpr::new(
            IrExprKind::Stream {
                value: Box::new(packed_const(0xa5, 128)),
                slice: 8,
                direction: IrStreamDirection::RightToLeft,
            },
            128,
            false,
            None,
        );
        assert_eq!(
            valid_model().expression_capacity(&stream, None).unwrap(),
            128
        );

        let inside = IrExpr::new(
            IrExprKind::Inside {
                value: Box::new(packed_const(1, 512)),
                items: vec![
                    IrInsideItem::Value(packed_const(1, 8)),
                    IrInsideItem::Range {
                        low: packed_const(0, 32),
                        high: packed_const(3, 32),
                    },
                ],
            },
            1,
            false,
            None,
        );
        assert_eq!(
            valid_model().expression_capacity(&inside, None).unwrap(),
            512
        );
    }

    #[test]
    fn constructors_reject_invalid_local_invariants() {
        assert!(IrModel::new("top".to_string(), 0).is_err());
        assert!(IrType::packed(0, false).is_err());
        assert!(IrExpr::try_new(IrExprKind::Fill(4), 1, false, Some(4)).is_err());
        assert!(IrArray::new("a".into(), "a".into(), 8, false, Vec::new()).is_err());
        assert!(IrNetGroup::new("n".into(), 1, false, IrNetKind::Wire, 0).is_err());
        assert!(IrNetGroup::new(
            "n".into(),
            1,
            false,
            IrNetKind::Wire,
            LLG_MAX_NET_DRIVERS + 1,
        )
        .is_err());
    }

    #[test]
    fn packed_constructor_checks_the_declared_high_limb() {
        let value = IrConst::packed(vec![u64::MAX, 1], vec![], vec![], 65, false, None)
            .expect("bit 64 is inside a 65-bit value");
        assert_eq!(value.bits(), &[u64::MAX, 1]);

        let error = IrConst::packed(vec![0, 2], vec![], vec![], 65, false, None)
            .expect_err("bit 65 lies outside a 65-bit value");
        assert_eq!(error.path(), "const.bits");
    }

    #[test]
    fn function_local_initializer_uses_formals_and_contributes_to_capacity() {
        let formal = IrFormal::new(false, 512, false).expect("valid formal");
        let formal_read = IrExpr::new(IrExprKind::FormalRead(0), 512, false, None);
        let mut local = IrLocal::new("local".to_string(), 8, false).expect("valid local");
        local.initial = Some(IrExpr::resize_to(formal_read, 8, false));
        let function = IrFunc::new(
            "f".to_string(),
            None,
            vec![formal],
            vec![local],
            Vec::new(),
            Vec::new(),
        );
        let model = IrModel::from_parts(
            "top".to_string(),
            1,
            IrModelParts {
                funcs: vec![function],
                ..IrModelParts::default()
            },
        )
        .expect("a typed local initializer may read its function formal");

        assert_eq!(model.packed_capacity().unwrap(), 512);
    }

    #[test]
    fn public_parts_reject_local_initializer_with_wrong_type() {
        let formal = IrFormal::new(false, 8, false).expect("valid formal");
        let mut local = IrLocal::new("local".to_string(), 8, true).expect("valid local");
        local.initial = Some(IrExpr::new(IrExprKind::FormalRead(0), 8, false, None));
        let function = IrFunc::new(
            "f".to_string(),
            None,
            vec![formal],
            vec![local],
            Vec::new(),
            Vec::new(),
        );

        let error = IrModel::from_parts(
            "top".to_string(),
            1,
            IrModelParts {
                funcs: vec![function],
                ..IrModelParts::default()
            },
        )
        .expect_err("a local initializer must have the declaration's exact packed type");
        assert_eq!(error.path(), "funcs[0].locals[0].initial");
        assert!(error.detail().contains("initializer type"));
    }

    #[test]
    fn public_parts_build_a_nonempty_valid_model() {
        let signal = IrSignal::new(
            "sig".to_string(),
            Some("top.sig".to_string()),
            IrType::packed(1, false).unwrap(),
            None,
        )
        .unwrap();
        let model = IrModel::from_parts(
            "top".to_string(),
            1,
            IrModelParts {
                signals: vec![signal],
                ..IrModelParts::default()
            },
        )
        .expect("a nonempty valid model must be constructible through the public API");
        assert_eq!(model.signals().len(), 1);
        assert_eq!(model.signal(0).hdl_name(), Some("top.sig"));
    }

    #[test]
    fn public_parts_reject_invalid_cross_table_references() {
        let process = IrProcess::new(
            "proc".to_string(),
            "top.initial".to_string(),
            IrShape::RunOnce,
            Vec::new(),
            vec![IrStmt::Release {
                lhs: IrLhs::Whole(0),
            }],
        );
        let error = IrModel::from_parts(
            "top".to_string(),
            1,
            IrModelParts {
                processes: vec![process],
                spawns: vec!["proc".to_string()],
                ..IrModelParts::default()
            },
        )
        .expect_err("a model cannot reference a missing signal");
        assert_eq!(error.path(), "processes[0].body[0].lhs");
    }

    #[test]
    fn detached_nested_expression_checks_exact_formal_type() {
        let context = IrFunc::new(
            "context".to_string(),
            None,
            vec![IrFormal::new(false, 8, true).unwrap()],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        let expression = IrExpr::new(
            IrExprKind::Bin {
                op: IrBinOp::Add,
                a: Box::new(IrExpr::new(IrExprKind::FormalRead(0), 4, false, None)),
                b: Box::new(IrExpr::new(
                    IrExprKind::Const(
                        IrConst::packed(vec![1], vec![], vec![], 4, false, None).unwrap(),
                    ),
                    4,
                    false,
                    None,
                )),
            },
            4,
            false,
            None,
        );

        let error = valid_model()
            .validate_expr(&expression, Some(&context))
            .expect_err("a nested formal read must carry the formal's exact type");
        assert_eq!(error.path(), "expr.a");
        assert!(error.detail().contains("formal type"));
    }

    #[test]
    fn call_arguments_follow_output_then_input_parameter_order() {
        let callee = IrFunc::new(
            "callee".to_string(),
            Some(IrType::packed(1, false).unwrap()),
            vec![
                IrFormal::new(false, 8, false).unwrap(),
                IrFormal::new(true, 16, true).unwrap(),
            ],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        let model = IrModel::from_parts(
            "top".to_string(),
            1,
            IrModelParts {
                funcs: vec![callee],
                ..IrModelParts::default()
            },
        )
        .unwrap();
        let input = IrExpr::new(
            IrExprKind::Const(IrConst::packed(vec![7], vec![], vec![], 8, false, None).unwrap()),
            8,
            false,
            None,
        );
        let valid = IrExpr::new(
            IrExprKind::CallFn(Box::new(IrCallExpr::new(
                0,
                vec![
                    IrCallArg::OutAddr("&out".to_string()),
                    IrCallArg::Val(input.clone()),
                ],
                IrDepth::PROC,
                false,
            ))),
            1,
            false,
            None,
        );
        model
            .validate_expr(&valid, None)
            .expect("C-order output then input arguments are valid");

        let invalid = IrExpr::new(
            IrExprKind::CallFn(Box::new(IrCallExpr::new(
                0,
                vec![
                    IrCallArg::Val(input),
                    IrCallArg::OutAddr("&out".to_string()),
                ],
                IrDepth::PROC,
                false,
            ))),
            1,
            false,
            None,
        );
        let error = model
            .validate_expr(&invalid, None)
            .expect_err("argument variants must agree with formal directions");
        assert_eq!(error.path(), "expr.args[0]");
        assert!(error.detail().contains("address argument"));
    }

    #[test]
    fn string_real_queries_require_real_result_metadata() {
        let model = valid_model();
        let query = IrObjectQuery::StringAtoreal(IrStringExpr::Literal(b"1.5".to_vec()));
        let expression = |width, signed| {
            IrExpr::new(
                IrExprKind::ObjectQuery(Box::new(query.clone())),
                width,
                signed,
                None,
            )
        };
        model.validate_expr(&expression(0, true), None).unwrap();
        assert!(model.validate_expr(&expression(32, true), None).is_err());
        assert!(model.validate_expr(&expression(0, false), None).is_err());
        let packed_query = IrExpr::new(
            IrExprKind::ObjectQuery(Box::new(IrObjectQuery::StringLen(IrStringExpr::Literal(
                vec![],
            )))),
            0,
            true,
            None,
        );
        assert!(model.validate_expr(&packed_query, None).is_err());
    }

    #[test]
    fn string_realtoa_requires_real_argument_and_string_storage() {
        let mut model = valid_model();
        model.objects.push(IrObject {
            c_name: "text".to_owned(),
            ty: IrObjectType::String,
            initial: None,
        });
        let real = IrExpr::new(IrExprKind::Const(IrConst::real(1.5)), 0, false, None);
        model
            .validate_stmt(
                &IrStmt::Object(IrObjectStmt::StringRealtoa(0, real.clone())),
                None,
            )
            .unwrap();
        assert!(model
            .validate_stmt(
                &IrStmt::Object(IrObjectStmt::StringRealtoa(0, packed_const(1, 32))),
                None
            )
            .is_err());
        assert!(model
            .validate_stmt(
                &IrStmt::Object(IrObjectStmt::StringItoa(0, real.clone(), 10)),
                None
            )
            .is_err());
        model.objects[0].ty = IrObjectType::Chandle;
        assert!(model
            .validate_stmt(&IrStmt::Object(IrObjectStmt::StringRealtoa(0, real)), None)
            .is_err());
    }

    #[test]
    fn string_return_storage_requires_its_function_context() {
        let model = valid_model();
        let value = IrStringExpr::LocalRead("_ret".to_owned());
        let statement = IrStmt::Object(IrObjectStmt::StringAssignLocal("_ret".to_owned(), value));
        assert!(model.validate_stmt(&statement, None).is_err());
        let mut function =
            IrFunc::new("string_fn".to_owned(), None, vec![], vec![], vec![], vec![]);
        assert!(model.validate_stmt(&statement, Some(&function)).is_err());
        function.ret_string = true;
        model.validate_stmt(&statement, Some(&function)).unwrap();
        let helper = IrPreFn::Branch {
            c_name: "helper".to_owned(),
            body: vec![statement],
        };
        assert!(model.validate_pre_fn(&helper, Some(&function)).is_err());
    }

    #[test]
    fn string_calls_validate_packed_arguments_and_depth_context() {
        let mut model = valid_model();
        let mut function = IrFunc::new(
            "string_fn".to_owned(),
            None,
            vec![IrFormal::new(false, 128, false).unwrap()],
            vec![],
            vec![],
            vec![],
        );
        function.ret_string = true;
        model.funcs.push(function.clone());
        let statement = |width, depth| {
            IrStmt::Object(IrObjectStmt::StringPrint(IrStringExpr::Call {
                function: 0,
                args: vec![packed_const(1, width)],
                depth,
            }))
        };
        assert_eq!(
            model
                .statement_capacity(&statement(128, IrDepth::PROC), None)
                .unwrap(),
            128
        );
        assert!(model
            .validate_stmt(&statement(64, IrDepth::PROC), None)
            .is_err());
        assert!(model
            .validate_stmt(&statement(128, IrDepth::FUNC), None)
            .is_err());
        model
            .validate_stmt(&statement(128, IrDepth::FUNC), Some(&function))
            .unwrap();
    }
}
