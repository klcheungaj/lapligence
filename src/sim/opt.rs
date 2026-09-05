//! Public façade for conservative simulator-IR optimization passes.

mod passes;

pub use passes::{run, OptConfig};
