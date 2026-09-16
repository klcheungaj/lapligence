//! Public façade for rendering validated simulator IR as C11.

mod assignments;
mod constants;
mod containers;
mod context;
mod error;
mod expressions;
mod model;
mod names;
mod objects;
mod owned;
mod stack;
mod statements;

/// Exclusive packed-width safeguard of the C runtime, not an IR restriction.
/// Keep aligned with `LLG_SUPPORTED_WIDTH_LIMIT` in `rt/llg_value.h`.
pub const LLG_WIDTH_LIMIT: u32 = 1 << 20;
/// Generated model/runtime ownership ABI. Keep aligned with llg_value.h.
pub const VALUE_ABI_VERSION: u32 = 3;
/// Largest width supported by this C backend.
pub const LLG_MAX_WIDTH: u32 = LLG_WIDTH_LIMIT - 1;

/// The legacy fragment APIs cannot represent ordered ownership setup/cleanup.
/// Whole-model emission uses `owned::Frame` and rejects unmigrated features.
pub(crate) const OWNER_MIGRATION_DIAGNOSTIC: &str =
    "legacy expression/statement fragments cannot manage dynamic sv4_t owners; use whole-model emission through the structured ownership emitter";

pub(crate) fn require_owned_emission() -> Result<(), EmitError> {
    Err(EmitError::new(OWNER_MIGRATION_DIAGNOSTIC))
}

fn check_capacity(width: u128) -> Result<(), EmitError> {
    if width >= u128::from(LLG_WIDTH_LIMIT) {
        Err(EmitError::new(format!(
            "packed width {width} reaches the C runtime exclusive limit {LLG_WIDTH_LIMIT}"
        )))
    } else {
        Ok(())
    }
}

pub use context::{RCtx, RenderedExpr};
pub use error::EmitError;
pub use expressions::render_expr;
pub use model::render;
pub use statements::{render_pre_fn, render_stmt};

pub(crate) use expressions::array_guard;
pub(crate) use names::{
    escaped_char, event_global_name, global_name, ident, real_global_name, strip_lib,
};

/// Select the generated C entry point for one lowered call. Virtual methods
/// share a dispatch helper; ordinary and explicit-`super` calls retain the
/// concrete function symbol.
pub(crate) fn function_call_name(
    function: &crate::sim::ir::IrFunc,
    virtual_dispatch: bool,
) -> String {
    if virtual_dispatch {
        if let Some(slot) = function.virtual_slot {
            return format!("llg_class_dispatch_{slot}");
        }
    }
    function.c_name.clone()
}

#[cfg(test)]
mod tests;
