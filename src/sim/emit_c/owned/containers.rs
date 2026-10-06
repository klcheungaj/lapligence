//! Container storage operations with ordered, registered operand owners.
use super::native::{NativeKind, NativeValue};
use super::*;
mod expressions;
mod keys;
mod statements;
pub(in crate::sim::emit_c) use keys::key_adapters;

fn name(frame: &Frame<'_, '_>, index: usize) -> Result<String, String> {
    frame.container_name(index)
}

impl Frame<'_, '_> {
    /// C lvalue of container `index`: its global for model storage, or the
    /// binding of an activation container declared or bound in this frame.
    pub(super) fn container_name(&self, index: usize) -> Result<String, String> {
        let container = self
            .ctx
            .model
            .containers
            .get(index)
            .ok_or("container reference is out of bounds")?;
        if let Some((class, field)) = container.class_field {
            if let Some(receiver) = &container.receiver {
                // The property selected through an explicit handle (SIM-011),
                // re-read at each use.
                let receiver = self.plain_handle_code(receiver, 0)?;
                let (ty, _, _) = super::super::containers::activation_storage(container, "")?;
                return Ok(format!(
                    "(*({ty}*)llg_class_field({receiver}, {class}, {field}, 3)->value.handle)"
                ));
            }
            // Per-object storage of an instance property, reached through
            // the receiver of the enclosing method.
            if self
                .ctx
                .func
                .is_none_or(|function| function.receiver_class.is_none())
            {
                return Err("class container property used outside its class methods".to_owned());
            }
            let (ty, _, _) = super::super::containers::activation_storage(container, "")?;
            return Ok(format!(
                "(*({ty}*)llg_class_field(_this, {class}, {field}, 3)->value.handle)"
            ));
        }
        if !container.activation {
            return Ok(container.c_name.clone());
        }
        self.containers
            .get(&index)
            .cloned()
            .ok_or_else(|| "container used before its lexical declaration".to_owned())
    }

    /// C expression of a plain handle read (`IrChandleExpr::is_plain_receiver`)
    /// that emits no statements, so container names can embed it.
    pub(super) fn plain_handle_code(
        &self,
        value: &IrChandleExpr,
        depth: usize,
    ) -> Result<String, String> {
        if depth > 256 {
            return Err("cyclic or excessively deep class property receiver".to_owned());
        }
        let local = |name: &str| {
            self.native_bindings
                .iter()
                .rev()
                .find_map(|scope| scope.get(name))
                .filter(|binding| binding.kind == NativeKind::Chandle)
                .map(|binding| format!("*({})", binding.address))
                .ok_or_else(|| format!("class property receiver `{name}` has no handle storage"))
        };
        match value {
            IrChandleExpr::Read(index) => Ok(self.ctx.model.objects[*index].c_name.clone()),
            IrChandleExpr::Required { handle, site } => Ok(format!(
                "llg_class_require({}, {})",
                self.plain_handle_code(handle, depth + 1)?,
                c_string_literal(site)
            )),
            IrChandleExpr::LocalRead(name)
                if name == "_this" && self.ctx.func.is_some_and(|f| f.receiver_class.is_some()) =>
            {
                Ok("_this".to_owned())
            }
            IrChandleExpr::LocalRead(name) => {
                let Some(access) = self
                    .ctx
                    .model
                    .native_accesses
                    .iter()
                    .find(|access| &access.name == name)
                else {
                    return local(name);
                };
                match access.kind {
                    IrNativeAccessKind::ClassField { class, field }
                        if self.ctx.model.classes[class].fields[field].ty
                            == IrClassFieldType::Chandle =>
                    {
                        Ok(format!(
                            "llg_class_field({}, {class}, {field}, 3)->value.handle",
                            self.plain_handle_code(&access.receiver, depth + 1)?
                        ))
                    }
                    _ => Err(format!(
                        "class property receiver `{name}` is not a handle property"
                    )),
                }
            }
            IrChandleExpr::FormalRead(index) => {
                let formal = self
                    .ctx
                    .func
                    .and_then(|function| function.formals.get(*index))
                    .ok_or_else(|| "handle formal outside a function".to_owned())?;
                if formal.is_ref() {
                    Ok(format!("*r{index}"))
                } else if formal.is_out {
                    Ok(format!("*o{index}"))
                } else {
                    local(&format!("a{index}"))
                }
            }
            _ => Err("class property receiver is not a plain handle read".to_owned()),
        }
    }

    /// Create an empty container with the storage type of `index`, owned by
    /// the current lexical value scope, and return its C lvalue. Activation
    /// containers and call-boundary copies use this storage.
    pub(super) fn new_container(&mut self, index: usize) -> Result<String, String> {
        self.new_container_in(index, None)
    }

    /// Create activation container `index` owned by the lexical value scope,
    /// or by slot `slot` of the shared activation frame `frame`, whose next
    /// two slots hold its contents and shape change markers (other processes
    /// sharing the container wait on them).
    pub(super) fn new_container_in(
        &mut self,
        index: usize,
        frame: Option<(&str, u32)>,
    ) -> Result<String, String> {
        let container = self
            .ctx
            .model
            .containers
            .get(index)
            .ok_or("container declaration is out of bounds")?;
        let (ty, _, destroy) = super::super::containers::activation_storage(container, "")?;
        let pointer = self.scalar(
            &format!("{ty}*"),
            match frame {
                Some((frame, slot)) => format!(
                    "({ty}*)llg_frame_capture_object({frame}, {slot}u, sizeof({ty}), {destroy})"
                ),
                None => format!(
                    "({ty}*)llg_value_scope_object(llg_value_scope_begin_object(sizeof({ty}), {destroy}))"
                ),
            },
        );
        let target = format!("(*{pointer})");
        let (_, init, _) = super::super::containers::activation_storage(container, &target)?;
        for line in init.lines() {
            self.line(line.trim());
        }
        if let Some((frame, slot)) = frame {
            self.line(format!(
                "{target}.contents_dependency = llg_frame_value_address({frame}, {}u);",
                slot + 1
            ));
            self.line(format!(
                "{target}.shape_dependency = llg_frame_value_address({frame}, {}u);",
                slot + 2
            ));
            self.line(format!("{target}.notify = llg_dependency_notify;"));
        }
        if let Some(size) = container.initial_size {
            // A fixed-array view starts with its declared default elements.
            let function = if container.element.is_packed() {
                "llg_dyn_new"
            } else {
                "llg_dyn_value_new"
            };
            self.line(format!(
                "{{ sv4_t size = sv4_from_u64({size}ULL, 64, 0); {function}(&{target}, size, NULL); sv4_destroy(&size); }}"
            ));
        }
        Ok(target)
    }
}

fn operand(
    frame: &mut Frame<'_, '_>,
    values: &mut Vec<Value>,
    expression: &IrExpr,
) -> Result<super::super::context::RenderedExpr, String> {
    let value = frame.expression(expression)?;
    let result = super::super::context::RenderedExpr {
        code: value.code.clone(),
        width: value.width,
        signed: value.signed,
        fill: value.fill,
    };
    values.push(value);
    Ok(result)
}
fn zero_operand(frame: &mut Frame<'_, '_>, values: &mut Vec<Value>) -> String {
    let value = frame.value("sv4_from_u64(0, 32, 1)".to_owned(), 32, true);
    let code = value.code.clone();
    values.push(value);
    code
}
fn key_operand(
    frame: &mut Frame<'_, '_>,
    strings: &mut Vec<NativeValue>,
    text: &IrStringExpr,
) -> Result<String, String> {
    let value = frame.string(text)?;
    let code = value.code();
    strings.push(value);
    Ok(code)
}
fn text_operand(
    frame: &mut Frame<'_, '_>,
    strings: &mut Vec<NativeValue>,
    text: &IrStringExpr,
) -> Result<String, String> {
    let value = frame.string(text)?;
    let code = value.take_string();
    strings.push(value);
    Ok(code)
}
fn indices(
    frame: &mut Frame<'_, '_>,
    values: &mut Vec<Value>,
    items: &[IrExpr],
) -> Result<String, String> {
    let mut codes = Vec::new();
    for item in items {
        codes.push(operand(frame, values, item)?.code);
    }
    Ok(if codes.is_empty() {
        "NULL".to_owned()
    } else {
        format!("(const sv4_t[]){{ {} }}", codes.join(", "))
    })
}
pub(super) fn stream_selector(
    frame: &mut Frame<'_, '_>,
    values: &mut Vec<Value>,
    selector: Option<&IrStreamSelector>,
) -> Result<(i32, String, String), String> {
    Ok(match selector {
        None => (0, zero_operand(frame, values), zero_operand(frame, values)),
        Some(IrStreamSelector::Index(index)) => (
            1,
            operand(frame, values, index)?.code,
            zero_operand(frame, values),
        ),
        Some(IrStreamSelector::Range { left, right }) => (
            2,
            operand(frame, values, left)?.code,
            operand(frame, values, right)?.code,
        ),
        Some(IrStreamSelector::Indexed {
            base,
            width,
            negative,
        }) => (
            if *negative { 4 } else { 3 },
            operand(frame, values, base)?.code,
            operand(frame, values, width)?.code,
        ),
    })
}
impl Frame<'_, '_> {
    pub(super) fn container_indices(
        &mut self,
        expressions: &[IrExpr],
    ) -> Result<(String, Vec<Value>), String> {
        let mut values = Vec::new();
        let codes = indices(self, &mut values, expressions)?;
        Ok((codes, values))
    }
    pub(super) fn container_expression(
        &mut self,
        operation: &IrContainerExpr,
        expression: &IrExpr,
    ) -> Result<Value, String> {
        if self.read_only_callback
            && matches!(
                operation,
                IrContainerExpr::QueuePopFront(_)
                    | IrContainerExpr::QueuePopBack(_)
                    | IrContainerExpr::AssocTraverse { .. }
                    | IrContainerExpr::AssocTraverseString { .. }
                    | IrContainerExpr::AssocTraverseStringLocal { .. }
            )
        {
            return Err(pending("mutating container query in a read-only callback"));
        }
        if let IrContainerExpr::QueuePopFront(index) | IrContainerExpr::QueuePopBack(index) =
            operation
        {
            if self.ctx.model.containers[*index].element.is_real() {
                let back = matches!(operation, IrContainerExpr::QueuePopBack(_));
                let name = self.container_name(*index)?;
                return Ok(self.value(
                    format!("llg_queue_value_pop_real(&{name}, {})", i32::from(back)),
                    0,
                    false,
                ));
            }
        }
        if let IrContainerExpr::QueueFront(index)
        | IrContainerExpr::QueueBack(index)
        | IrContainerExpr::QueuePopFront(index)
        | IrContainerExpr::QueuePopBack(index) = operation
        {
            if !self.ctx.model.containers[*index].element.is_packed() {
                return Err(pending("non-packed queue endpoint/pop expressions"));
            }
        }
        if let IrContainerExpr::QueuePopFront(index) | IrContainerExpr::QueuePopBack(index) =
            operation
        {
            let result = self.reserve(expression.width, expression.signed);
            let side = if matches!(operation, IrContainerExpr::QueuePopFront(_)) {
                "front"
            } else {
                "back"
            };
            let name = self.container_name(*index)?;
            self.line(format!(
                "llg_queue_pop_{side}_into(&{name}, &{});",
                result.code
            ));
            return Ok(result);
        }
        let mut values = Vec::new();
        let mut strings = Vec::new();
        let code = expressions::render(self, operation, &mut values, &mut strings)?;
        let result = self.value(code, expression.width, expression.signed);
        for value in values {
            self.discard(value);
        }
        for value in strings {
            self.native_discard(value);
        }
        Ok(result)
    }
    pub(super) fn container_statement(
        &mut self,
        operation: &IrContainerStmt,
    ) -> Result<(), String> {
        if let IrContainerStmt::Declare(container) = operation {
            if !self.ctx.model.containers[*container].activation {
                return Err("only activation containers are declared lexically".to_owned());
            }
            let target = self.new_container(*container)?;
            self.containers.insert(*container, target);
            return Ok(());
        }
        if let IrContainerStmt::SharedDeclare(container) = operation {
            if !self.ctx.model.containers[*container].activation {
                return Err("only activation containers are declared lexically".to_owned());
            }
            let owner = self.scalar(
                "llg_frame_t**",
                "(llg_frame_t**)llg_value_scope_object(llg_value_scope_begin_object(sizeof(llg_frame_t*), llg_owned_frame_drop))"
                    .to_owned(),
            );
            self.line(format!("*{owner} = llg_frame_new(3ULL);"));
            let target = self.new_container_in(*container, Some((&format!("*{owner}"), 0)))?;
            self.containers.insert(*container, target);
            self.shared_cells.insert(
                crate::sim::ir::shared_container_capture_name(*container),
                (format!("(*{owner})"), 0),
            );
            return Ok(());
        }
        let mut values = Vec::new();
        let mut strings = Vec::new();
        let code = statements::render(self, operation, &mut values, &mut strings)?;
        self.line(code);
        for value in values {
            self.discard(value);
        }
        for value in strings {
            self.native_discard(value);
        }
        Ok(())
    }
}
/// Runtime whole-container copy for the storage type of `container`.
pub(super) fn copy_function(container: &crate::sim::ir::IrContainer) -> &'static str {
    match (container.element.is_packed(), &container.kind) {
        (false, IrContainerKind::Dynamic) => "llg_dyn_value_copy",
        (false, IrContainerKind::Queue { .. }) => "llg_queue_value_copy",
        (false, IrContainerKind::Associative { .. }) => "llg_assoc_value_copy",
        (true, IrContainerKind::Dynamic) => "llg_dyn_copy",
        (true, IrContainerKind::Queue { .. }) => "llg_queue_copy",
        (true, IrContainerKind::Associative { .. }) => "llg_assoc_copy",
    }
}

fn prefix(kind: &IrContainerKind) -> &'static str {
    match kind {
        IrContainerKind::Dynamic => "llg_dyn",
        IrContainerKind::Queue { .. } => "llg_queue",
        IrContainerKind::Associative { .. } => "llg_assoc",
    }
}

fn method_code(method: IrContainerMethod) -> &'static str {
    match method {
        IrContainerMethod::Find => "LLG_CONTAINER_METHOD_FIND",
        IrContainerMethod::FindIndex => "LLG_CONTAINER_METHOD_FIND_INDEX",
        IrContainerMethod::FindFirst => "LLG_CONTAINER_METHOD_FIND_FIRST",
        IrContainerMethod::FindFirstIndex => "LLG_CONTAINER_METHOD_FIND_FIRST_INDEX",
        IrContainerMethod::FindLast => "LLG_CONTAINER_METHOD_FIND_LAST",
        IrContainerMethod::FindLastIndex => "LLG_CONTAINER_METHOD_FIND_LAST_INDEX",
        IrContainerMethod::Min => "LLG_CONTAINER_METHOD_MIN",
        IrContainerMethod::Max => "LLG_CONTAINER_METHOD_MAX",
        IrContainerMethod::Unique => "LLG_CONTAINER_METHOD_UNIQUE",
        IrContainerMethod::UniqueIndex => "LLG_CONTAINER_METHOD_UNIQUE_INDEX",
        IrContainerMethod::Sort => "LLG_CONTAINER_METHOD_SORT",
        IrContainerMethod::RSort => "LLG_CONTAINER_METHOD_RSORT",
        IrContainerMethod::Reverse => "LLG_CONTAINER_METHOD_REVERSE",
        IrContainerMethod::Shuffle => "LLG_CONTAINER_METHOD_SHUFFLE",
    }
}
