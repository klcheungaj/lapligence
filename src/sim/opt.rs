//! Public façade for conservative simulator-IR optimization passes.

mod passes;

pub use passes::OptConfig;

use crate::sim::execution::ExecutionModel;
use crate::sim::ir::IrValidationError;

/// Optimize executable operations, then rebuild and validate their scheduling
/// effect summaries before the backend sees the model.
pub fn run(model: &mut ExecutionModel, cfg: &OptConfig) -> Result<(), IrValidationError> {
    passes::run_execution(model, cfg);
    model.refresh_effects()
}

#[cfg(test)]
pub(crate) use passes::run_ir;
