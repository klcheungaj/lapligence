//! Public façade for rendering validated simulator IR as C11.

mod constants;
mod context;
mod error;
mod expressions;
mod model;
mod names;
mod statements;

pub use context::{RCtx, RenderedExpr};
pub use error::EmitError;
pub use expressions::render_expr;
pub use model::render;
pub use statements::{render_pre_fn, render_stmt};

pub(crate) use names::{
    escaped_char, event_global_name, global_name, ident, real_global_name, strip_lib,
};

#[cfg(test)]
mod tests;
