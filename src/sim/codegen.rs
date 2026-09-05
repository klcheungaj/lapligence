//! Public façade for elaborated-database to simulator-IR/C lowering.
//!
//! The lowering implementation is isolated in [`lowering`]. Keeping this
//! façade small makes the supported library boundary explicit while the
//! implementation remains free to organize UHDM collection and IR lowering.

mod error;
mod lowering;
mod timescale;

pub use error::CodegenError;
pub use lowering::{generate, generate_from_db_with_opts, generate_with_opts, GeneratedModel};
