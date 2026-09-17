//! Container storage operations with ordered, registered operand owners.
use super::*;
use super::native::{NativeKind, NativeValue};
mod expressions;
mod statements;
mod keys;
pub(in crate::sim::emit_c) use keys::key_adapters;

fn name<'a>(ctx: &'a RCtx<'_>, index: usize) -> &'a str { &ctx.model.containers[index].c_name }

fn operand(frame: &mut Frame<'_, '_>, values: &mut Vec<Value>, expression: &IrExpr) -> Result<super::super::context::RenderedExpr, String> {
    let value = frame.expression(expression)?;
    let result = super::super::context::RenderedExpr { code: value.code.clone(), width: value.width, signed: value.signed, fill: value.fill };
    values.push(value);
    Ok(result)
}
fn zero_operand(frame: &mut Frame<'_, '_>, values: &mut Vec<Value>) -> String {
    let value = frame.value("sv4_from_u64(0, 32, 1)".to_owned(), 32, true);
    let code = value.code.clone(); values.push(value); code
}
fn key_operand(frame: &mut Frame<'_, '_>, strings: &mut Vec<NativeValue>, text: &IrStringExpr) -> Result<String, String> {
    let value = frame.string(text)?;
    let code = value.code(); strings.push(value); Ok(code)
}
fn text_operand(frame: &mut Frame<'_, '_>, strings: &mut Vec<NativeValue>, text: &IrStringExpr) -> Result<String, String> {
    let value = frame.string(text)?;
    let code = value.take_string(); strings.push(value); Ok(code)
}
fn indices(frame: &mut Frame<'_, '_>, values: &mut Vec<Value>, items: &[IrExpr]) -> Result<String, String> {
    let mut codes = Vec::new();
    for item in items { codes.push(operand(frame, values, item)?.code); }
    Ok(if codes.is_empty() { "NULL".to_owned() } else { format!("(const sv4_t[]){{ {} }}", codes.join(", ")) })
}
pub(super) fn stream_selector(frame: &mut Frame<'_, '_>, values: &mut Vec<Value>, selector: Option<&IrStreamSelector>) -> Result<(i32, String, String), String> {
    Ok(match selector {
        None => (0, zero_operand(frame, values), zero_operand(frame, values)),
        Some(IrStreamSelector::Index(index)) => (1, operand(frame, values, index)?.code, zero_operand(frame, values)),
        Some(IrStreamSelector::Range { left, right }) => (2, operand(frame, values, left)?.code, operand(frame, values, right)?.code),
        Some(IrStreamSelector::Indexed { base, width, negative }) => (if *negative { 4 } else { 3 }, operand(frame, values, base)?.code, operand(frame, values, width)?.code),
    })
}
impl Frame<'_, '_> {
    pub(super) fn container_indices(&mut self, expressions: &[IrExpr]) -> Result<(String, Vec<Value>), String> {
        let mut values = Vec::new();
        let codes = indices(self, &mut values, expressions)?;
        Ok((codes, values))
    }
    pub(super) fn container_expression(&mut self, operation: &IrContainerExpr, expression: &IrExpr) -> Result<Value, String> {
        if self.read_only_callback && matches!(operation,
            IrContainerExpr::QueuePopFront(_) | IrContainerExpr::QueuePopBack(_)
            | IrContainerExpr::AssocTraverse { .. } | IrContainerExpr::AssocTraverseString { .. }
            | IrContainerExpr::AssocTraverseStringLocal { .. }) {
            return Err(pending("mutating container query in a read-only callback"));
        }
        if let IrContainerExpr::QueueFront(index) | IrContainerExpr::QueueBack(index)
            | IrContainerExpr::QueuePopFront(index) | IrContainerExpr::QueuePopBack(index) = operation {
            if !self.ctx.model.containers[*index].element.is_packed() {
                return Err(pending("non-packed queue endpoint/pop expressions"));
            }
        }
        if let IrContainerExpr::QueuePopFront(index) | IrContainerExpr::QueuePopBack(index) = operation {
            let result = self.reserve(expression.width, expression.signed);
            let side = if matches!(operation, IrContainerExpr::QueuePopFront(_)) { "front" } else { "back" };
            self.line(format!("llg_queue_pop_{side}_into(&{}, &{});", self.ctx.model.containers[*index].c_name, result.code));
            return Ok(result);
        }
        let mut values = Vec::new(); let mut strings = Vec::new();
        let code = expressions::render(self, operation, &mut values, &mut strings)?;
        let result = self.value(code, expression.width, expression.signed);
        for value in values { self.discard(value); }
        for value in strings { self.native_discard(value); }
        Ok(result)
    }
    pub(super) fn container_statement(&mut self, operation: &IrContainerStmt) -> Result<(), String> {
        let mut values = Vec::new(); let mut strings = Vec::new();
        let code = statements::render(self, operation, &mut values, &mut strings)?;
        self.line(code);
        for value in values { self.discard(value); }
        for value in strings { self.native_discard(value); }
        Ok(())
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
