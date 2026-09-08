//! FFI layer — checked Rust APIs over native frontend and platform ABIs.

pub mod process_memory;
#[cfg(feature = "slang")]
pub mod slang;
pub mod surelog;
pub mod vpi;
