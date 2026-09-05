//! Shared model and expression context for C rendering.

use crate::sim::ir::{IrFunc, IrModel};

// ── Expression rendering ──────────────────────────────────────────────────────

/// A rendered expression: C code plus its self-determined width/signedness.
/// `width == 0` marks a real (double) value; `fill` mirrors the IR node's
/// unsized-fill marker for assignment/call/return conversions.
pub struct RenderedExpr {
    pub code: String,
    pub width: u32,
    pub signed: bool,
    pub fill: Option<u8>,
}

/// Render context: the model tables plus the enclosing C function when
/// rendering a function body (formal reads resolve through it).
pub struct RCtx<'m> {
    pub model: &'m IrModel,
    pub func: Option<&'m IrFunc>,
}
