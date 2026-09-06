//! tower-lsp backend facade for the llg language server.
//!
//! The implementation is split by responsibility under `lsp/`; this module
//! keeps the surface consumed by `transport.rs` internal to the binary.

mod handlers;

pub(crate) use handlers::{
    emergency_shadow_cleanup, emergency_shadow_cleanup_and_exit, mark_shutdown_requested,
    shutdown_requested, Backend,
};
