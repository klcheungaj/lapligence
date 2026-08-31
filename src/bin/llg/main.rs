//! Lapligence (llg) — Verilog/SystemVerilog Language Server
//!
//! Communicates with editors via the Language Server Protocol (LSP) over
//! stdin/stdout.  Parsing is delegated to Surelog via a C FFI wrapper.
//!
//! See `src/bin/llg_demo.rs` for the original command-line demo that
//! illustrates raw Surelog API usage.

// Replace musl's default allocator (and the system allocator on all targets)
// with mimalloc.  Because this is the final link point the override is also
// effective for all statically linked C/C++ code (Surelog, UHDM, ANTLR, …).
use mimalloc::MiMalloc;
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

mod config;
mod dump;
mod features;
mod inactive_ranges;
mod logging;
mod lsp;
mod module_explorer;
mod rename;
mod request_cache;
mod scheduler;
mod semantic_tokens;
mod workspace;

// Cross-scanner corpus pinning `core::macros` and `inactive_ranges` together;
// compiled only for tests.
#[cfg(test)]
mod conditional_conformance;

use std::task::{Context, Poll};
use std::time::Duration;

use tower::Service;
use tower_lsp::jsonrpc::{Request, Response};

/// Grace period between observing the LSP `exit` notification and terminating
/// the process: queued server-to-client messages flush through the transport
/// during this window.
const EXIT_GRACE: Duration = Duration::from_millis(300);

/// Service wrapper fixing two tower-lsp 0.20 transport gaps around the LSP
/// lifecycle notifications.
///
/// * `shutdown`: tower-lsp routes the built-in `shutdown` method through a
///   handler that takes NO parameters, so a client sending the conventional
///   `"params": null` (VS Code's languageclient does) gets a `-32602`
///   rejection and `LanguageServer::shutdown` NEVER RUNS — the internal
///   state flips, but this server's temp-tree cleanup would be skipped
///   entirely.  The wrapper marks the shutdown and schedules the same
///   deterministic shadow-base cleanup before delegating, so cleanup happens
///   for both the parameterless and the `params: null` wire shapes.
/// * `exit`: the serve loop only ends on stdin EOF or one further inbound
///   message after `exit`; a client that keeps stdin open hangs the runtime
///   forever.  Per the LSP specification the server must terminate once
///   `exit` arrives, so the wrapper schedules termination: a short grace
///   period, a final temp-tree sweep on the blocking pool, then
///   `std::process::exit` with the spec-mandated code (0 after `shutdown`,
///   otherwise 1).
struct LifecycleService<S> {
    inner: S,
}

impl<S> Service<Request> for LifecycleService<S>
where
    S: Service<Request, Response = Option<Response>> + Send + 'static,
    S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    S::Future: Send,
{
    type Response = Option<Response>;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        match req.method() {
            "shutdown" => schedule_shutdown(),
            "exit" => schedule_exit(),
            _ => {}
        }
        self.inner.call(req)
    }
}

/// Mark the shutdown and clean the temp tree concurrently with the delegated
/// request handling.  Idempotent: a well-formed `shutdown` also triggers
/// [`lsp`] cleanup inside `Backend::shutdown`, and every path serializes on
/// the staging lock.
fn schedule_shutdown() {
    lsp::mark_shutdown_requested();
    tokio::spawn(async move {
        // Blocking fs work stays off the async runtime; the staging lock is
        // taken inside the closure so cleanup stays ordered against jobs.
        let _ = tokio::task::spawn_blocking(lsp::emergency_shadow_cleanup).await;
    });
}

fn schedule_exit() {
    tokio::spawn(async move {
        tokio::time::sleep(EXIT_GRACE).await;
        // Final idempotent sweep: guarantees no `<tmp>/llg-{pid}-*` tree
        // survives even if a late job re-created one after the shutdown
        // cleanup.
        let _ = tokio::task::spawn_blocking(lsp::emergency_shadow_cleanup).await;
        let code = i32::from(!lsp::shutdown_requested());
        std::process::exit(code);
    });
}

// ── main ─────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    logging::init();

    // Offline diagnostic mode: dump every computed token/binding for a
    // project (or a single file) and exit before the LSP runtime starts.
    // stdout carries plain report lines here, not LSP framing.
    if let Some(code) = dump::run_from_env() {
        std::process::exit(code);
    }

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    // The builder form registers this server's custom requests next to the
    // built-in LanguageServer methods (see `Backend::dump_tokens` and
    // `Backend::inactive_ranges`); `finish` returns the same (service, socket)
    // pair as `LspService::new`.
    let (service, socket) = tower_lsp::LspService::build(lsp::Backend::new)
        .custom_method("llg/dumpTokens", lsp::Backend::dump_tokens)
        .custom_method("llg/inactiveRanges", lsp::Backend::inactive_ranges)
        .custom_method("llg/moduleExplorer", lsp::Backend::module_explorer)
        .finish();

    tower_lsp::Server::new(stdin, stdout, socket)
        .serve(LifecycleService { inner: service })
        .await;
}
