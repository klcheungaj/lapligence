//! Stdio transport, process lifecycle, and server startup.

use crate::{dump, logging, lsp};

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
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
///   otherwise 1).  The main future retains the task if the transport returns
///   early because stdin reached EOF, so runtime shutdown cannot cancel it.
struct LifecycleService<S> {
    inner: S,
    exit: ExitCoordinator,
}

#[derive(Clone)]
struct ExitCoordinator {
    state: Arc<ExitState>,
}

struct ExitState {
    requested: AtomicBool,
    task: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl ExitCoordinator {
    fn new() -> Self {
        Self {
            state: Arc::new(ExitState {
                requested: AtomicBool::new(false),
                task: Mutex::new(None),
            }),
        }
    }

    fn request(&self) {
        // LSP clients should send one exit notification, but accepting a
        // duplicate must not create two independent process terminators.
        if self.state.requested.swap(true, Ordering::SeqCst) {
            return;
        }

        let task = tokio::spawn(async {
            tokio::time::sleep(EXIT_GRACE).await;
            let code = i32::from(!lsp::shutdown_requested());
            let cleanup = tokio::task::spawn_blocking(move || -> ! {
                lsp::emergency_shadow_cleanup_and_exit(code)
            })
            .await;
            match cleanup {
                Ok(never) => match never {},
                Err(error) => {
                    // A blocking task normally cannot be cancelled after it
                    // starts. If the pool rejects it before that point, keep
                    // the same cleanup/exit guarantee on this runtime thread.
                    crate::llg_error!("event=exit.cleanup outcome=task-error error={}", error);
                    lsp::emergency_shadow_cleanup_and_exit(code);
                }
            }
        });
        *self
            .state
            .task
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(task);
    }

    fn requested(&self) -> bool {
        self.state.requested.load(Ordering::SeqCst)
    }

    /// Keep the runtime alive until the scheduled process terminator has run.
    /// This is needed when `serve` returns because stdin closed immediately
    /// after the exit notification.
    async fn finish_after_serve(&self) -> ! {
        let task = self
            .state
            .task
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        if let Some(task) = task {
            match task.await {
                Ok(()) => {
                    // The scheduled task only returns if its blocking cleanup
                    // unexpectedly failed; retry below before returning from
                    // main so exit cannot silently become a normal EOF.
                    crate::llg_error!("event=exit.cleanup outcome=returned-before-exit");
                }
                Err(error) => {
                    // A panic or cancellation is likewise repaired by the
                    // single fallback below. No lock is held across await.
                    crate::llg_error!("event=exit.cleanup outcome=join-error error={}", error);
                }
            }
        }

        let code = i32::from(!lsp::shutdown_requested());
        match tokio::task::spawn_blocking(move || -> ! {
            lsp::emergency_shadow_cleanup_and_exit(code)
        })
        .await
        {
            Ok(never) => match never {},
            Err(error) => {
                // Preserve the lock-through-exit guarantee even if the
                // fallback could not be submitted to the blocking pool.
                crate::llg_error!(
                    "event=exit.cleanup outcome=fallback-task-error error={}",
                    error
                );
                lsp::emergency_shadow_cleanup_and_exit(code);
            }
        }
    }
}

impl<S> Service<Request> for LifecycleService<S>
where
    S: Service<Request, Response = Option<Response>> + Send + 'static,
    S::Error: Into<Box<dyn std::error::Error + Send + Sync>> + 'static,
    S::Future: Send + 'static,
{
    type Response = Option<Response>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Option<Response>, S::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let method = logging::bounded_field(req.method());
        let request_id = req
            .id()
            .map(ToString::to_string)
            .map(|id| logging::bounded_field(&id))
            .unwrap_or_else(|| "-".to_owned());
        let has_params = req.params().is_some();
        crate::llg_trace!(
            "event=transport.receive method={} id={} has_params={}",
            method,
            request_id,
            has_params
        );
        match req.method() {
            "shutdown" if req.id().is_some() => schedule_shutdown(),
            "exit" => schedule_exit(&self.exit),
            _ => {}
        }
        let future = self.inner.call(req);
        Box::pin(async move {
            let result = future.await;
            let outcome = match &result {
                Ok(Some(_)) => "response",
                Ok(None) => "notification",
                Err(_) => "error",
            };
            crate::llg_trace!(
                "event=transport.dispatch.end method={} id={} outcome={}",
                method,
                request_id,
                outcome
            );
            result
        })
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

fn schedule_exit(exit: &ExitCoordinator) {
    exit.request();
}

fn memory_log(level: llg::memory_limit::LogLevel, message: std::fmt::Arguments<'_>) {
    let level = match level {
        llg::memory_limit::LogLevel::Debug => logging::Level::Debug,
        llg::memory_limit::LogLevel::Info => logging::Level::Info,
        llg::memory_limit::LogLevel::Warn => logging::Level::Warn,
    };
    logging::write(level, message);
}

pub(crate) async fn run() -> i32 {
    logging::init();
    let _ = logging::set_memory_sampler(llg::memory_limit::current_physical_bytes);
    // Keep this guard in scope for the entire server lifetime.  It owns both
    // the optional native limit and the independent std watchdog thread.
    let memory_report = llg::memory_limit::install_with_logger(memory_log);
    let _memory_guard = memory_report.guard;

    // Offline diagnostic mode: dump every computed token/binding for a
    // project (or a single file) and exit before the LSP runtime starts.
    // stdout carries plain report lines here, not LSP framing.
    if let Some(code) = dump::run_from_env() {
        return code;
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

    let exit = ExitCoordinator::new();
    tower_lsp::Server::new(stdin, stdout, socket)
        .serve(LifecycleService {
            inner: service,
            exit: exit.clone(),
        })
        .await;
    if exit.requested() {
        exit.finish_after_serve().await;
    }
    0
}
