//! tower-lsp backend for the llg language server.
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use tokio::sync::Notify;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use crate::config::{self, ConfigError, LlgConfig};
use crate::dump;
use crate::features::{self, Analysis, SymbolIndex};
use crate::module_explorer;
use crate::request_cache::{
    next_analysis_epoch, CacheStats, MemoCache, RequestKey, RequestKind, NAVIGATION_CACHE_CAPACITY,
    TOKEN_CACHE_CAPACITY,
};
use crate::scheduler::{FireDecision, SchedulerState, TriggerDecision};
use crate::workspace::{self, RootDescriptor};
use llg::core::lint::LintConfig;

mod diagnostics;
mod scheduling;
mod staging;
mod state;

use diagnostics::*;
#[cfg(test)]
use scheduling::*;
use staging::*;
pub(crate) use staging::{emergency_shadow_cleanup, emergency_shadow_cleanup_and_exit};
pub(crate) use state::Backend;
#[cfg(test)]
use state::RootState;
use state::{BackendState, CommitOutcome, CompileResult, RescanResult, RootJob};

/// Trailing-edge quiet period for root re-analyses.  Bursts of didOpen/
/// didChange/didClose and watched-file events collapse into ONE scheduled
/// run per root; while a run executes, triggers only mark the root dirty and
/// exactly one debounced follow-up observes the latest state (see
/// `crate::scheduler`).
const RECOMPILE_DEBOUNCE: Duration = Duration::from_millis(300);
const WATCH_REGISTRATION_ID: &str = "llg-watched-files";
/// The client-to-server initialization protocol version this server accepts.
const CLIENT_PROTOCOL_VERSION: u32 = 1;
type RootKey = PathBuf;
type SharedText = Arc<String>;
type OpenDocuments = BTreeMap<PathBuf, SharedText>;
type OpenTokenDocument = (PathBuf, SharedText, Vec<String>);

/// Compiler-directive words mirrored from the shared macro scanner's
/// `DIRECTIVE_KEYWORDS` table.  The scanner also lists `__FILE__` and
/// `__LINE__` there so they are excluded from macro-usage reporting, but they
/// are predefined expression macros rather than whole-line directives and
/// therefore live in the separate table below.
const SEMANTIC_DIRECTIVE_KEYWORDS: &[&str] = &[
    "define",
    "undef",
    "undefineall",
    "ifdef",
    "ifndef",
    "elsif",
    "else",
    "endif",
    "include",
    "timescale",
    "resetall",
    "default_nettype",
    "line",
    "begin_keywords",
    "end_keywords",
    "celldefine",
    "endcelldefine",
    "pragma",
    "unconnected_drive",
    "nounconnected_drive",
    "accelerate",
    "noaccelerate",
    "default_decay_time",
    "default_trireg_strength",
    "delay_mode_distributed",
    "delay_mode_path",
    "delay_mode_unit",
    "delay_mode_zero",
];

/// Entries present in the shared scanner's directive-exclusion table that
/// must remain source text during isolated parsing.
const SEMANTIC_PREDEFINED_EXPRESSION_MACROS: &[&str] = &["__FILE__", "__LINE__"];

/// Set once the client's `shutdown` request has been handled; read by the
/// lifecycle interceptor in `main.rs` to pick the spec-mandated process exit
/// code (0 after `shutdown`, otherwise 1).
static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Whether the client already sent (and this server answered) `shutdown`.
pub(crate) fn shutdown_requested() -> bool {
    SHUTDOWN_REQUESTED.load(Ordering::SeqCst)
}

/// Record that the client's `shutdown` request arrived.  Called by the
/// lifecycle interceptor for the `params: null` wire shape too (whose request
/// never reaches [`LanguageServer::shutdown`] through tower-lsp 0.20).
pub(crate) fn mark_shutdown_requested() {
    SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
}

struct ModuleExplorerTaskResult {
    snapshot: module_explorer::ExplorerSnapshot,
    outcome: &'static str,
    error: Option<String>,
}

fn module_explorer_task_result(
    result: std::result::Result<module_explorer::ExplorerSnapshot, tokio::task::JoinError>,
) -> ModuleExplorerTaskResult {
    match result {
        Ok(snapshot) => ModuleExplorerTaskResult {
            snapshot,
            outcome: "ok",
            error: None,
        },
        Err(error) => ModuleExplorerTaskResult {
            snapshot: module_explorer::ExplorerSnapshot {
                modules: Vec::new(),
                roots: Vec::new(),
            },
            outcome: "error",
            error: Some(error.to_string()),
        },
    }
}
/// Parameters of the custom `llg/dumpTokens` request.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DumpTokensParams {
    uri: Url,
}

/// Parameters of the custom `llg/moduleExplorer` request.  An empty object
/// (or omitted/null params through the `Option` handler) requests every root.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModuleExplorerParams {
    /// Workspace-folder URI to select; omitted means all workspace roots.
    #[serde(alias = "rootUri")]
    workspace_uri: Option<Url>,
}

/// Result of the custom `llg/dumpTokens` request.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DumpTokensResult {
    lines: Vec<String>,
}

/// Failure shape of `llg/dumpTokens`: one `# error:` line, never an Err.
fn dump_tokens_error(reason: String) -> DumpTokensResult {
    DumpTokensResult {
        lines: vec![format!("# error: {reason}")],
    }
}

/// Parameters of the custom `llg/inactiveRanges` request.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InactiveRangesParams {
    uri: Url,
}

/// Result of the custom `llg/inactiveRanges` request: zero-based INCLUSIVE
/// line ranges, sorted and non-overlapping.
#[derive(Debug, serde::Serialize)]
pub(crate) struct InactiveRangesResult {
    ranges: Vec<crate::inactive_ranges::LineRange>,
}

/// Parameters of the server→client `llg/configChanged` notification.  The
/// payload is deliberately EMPTY — the signal itself is the contract; the
/// client refetches every config-derived view (inactive-region dimming)
/// instead of trusting a stale delta.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub(crate) struct ConfigChangedParams {}

/// Server→client notification type for [`ConfigChangedParams`].
pub(crate) enum LlgConfigChanged {}

impl tower_lsp::lsp_types::notification::Notification for LlgConfigChanged {
    type Params = ConfigChangedParams;
    const METHOD: &'static str = "llg/configChanged";
}

/// Empty server→client signal that a committed analysis changed the module
/// explorer data.  Clients refetch the complete snapshot, avoiding partial
/// update ordering problems when multiple roots commit close together.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub(crate) struct ModuleExplorerChangedParams {}

pub(crate) enum LlgModuleExplorerChanged {}

impl tower_lsp::lsp_types::notification::Notification for LlgModuleExplorerChanged {
    type Params = ModuleExplorerChangedParams;
    const METHOD: &'static str = "llg/moduleExplorerChanged";
}

/// Memoization key of one inactive-range computation: document URI, text
/// digest and effective `[compile] defines` — exactly the inputs of
/// [`crate::inactive_ranges::inactive_line_ranges`].
fn inactive_ranges_cache_key(uri: &str, text: &str, defines: &[String]) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    for define in defines {
        define.hash(&mut hasher);
    }
    format!("inactive-ranges|{uri}|{:016x}", hasher.finish())
}

/// Append one `# request-cache:` observability line to a dumpTokens payload,
/// immediately BEFORE the trailing `# analysis:` summary (which must stay the
/// last line — tests and tooling pin that position).
fn insert_cache_stats_line(lines: &mut Vec<String>, (stats, entries): (CacheStats, usize)) {
    let line = format!(
        "# request-cache: hits={} misses={} entries={}",
        stats.hits, stats.misses, entries
    );
    match lines
        .iter()
        .rposition(|line| line.starts_with("# analysis:"))
    {
        Some(position) => lines.insert(position, line),
        None => lines.push(line),
    }
}

impl Backend {
    /// Start an isolated open-document parse whose completion is detached
    /// from the request future.  The blocking closure owns the coordinator,
    /// so request cancellation cannot remove the flight while Slang is
    /// still running.  Successful cache publication happens before the
    /// coordinator removes the registry entry and wakes followers.
    fn spawn_open_token_coordinator(
        &self,
        mut coordinator: OpenTokenFlightCoordinator,
        key: String,
        uri: Url,
        captured_text: SharedText,
        open_document: OpenTokenDocument,
        parent_id: Option<u64>,
    ) -> Arc<OpenTokenFlight> {
        let flight = Arc::clone(&coordinator.flight);
        let cache = Arc::clone(&self.open_token_cache);
        let state = Arc::clone(&self.state);
        let coordinator_started = std::time::Instant::now();
        let file_for_log = crate::logging::enabled(crate::logging::Level::Debug)
            .then(|| open_document.0.display().to_string());
        let text_bytes = open_document.1.len();
        let define_count = open_document.2.len();
        let detached = tokio::task::spawn_blocking(move || {
            crate::llg_debug!(
                "event=semantic_tokens.coordinator.begin file={} bytes={} defines={} parent_id={:?}",
                file_for_log.as_deref().unwrap_or("-"),
                text_bytes,
                define_count,
                parent_id
            );
            let result = compute_open_document_semantic_tokens(
                open_document.0,
                open_document.1,
                open_document.2,
                {
                    let state = Arc::clone(&state);
                    let uri = uri.clone();
                    let captured_text = captured_text.clone();
                    move || open_document_is_current(&state, &uri, captured_text.as_str())
                },
                parent_id,
            );
            // Keep this post-compute check: a revision may change after the
            // final pre-frontend check, so stale results must never enter the
            // request cache.
            let buffer_is_current = open_document_is_current(&state, &uri, &captured_text);
            let token_count = result.as_ref().map_or(0, |tokens| tokens.data.len());
            let mut cache_published = false;
            if let Ok(tokens) = &result {
                if buffer_is_current {
                    cache.put(key, tokens.clone());
                    cache_published = true;
                }
            }
            crate::llg_debug!(
                "event=semantic_tokens.coordinator.publish outcome={} current={} cache_published={} token_count={} elapsed_us={}",
                if result.is_ok() { "ok" } else { "error" },
                buffer_is_current,
                cache_published,
                token_count,
                coordinator_started.elapsed().as_micros()
            );
            coordinator.finish(result);
            crate::llg_debug!(
                "event=semantic_tokens.coordinator.end outcome=complete current={} cache_published={} token_count={} elapsed_us={}",
                buffer_is_current,
                cache_published,
                token_count,
                coordinator_started.elapsed().as_micros()
            );
        });
        // The coordinator is deliberately detached.  Dropping this handle
        // does not abort a started `spawn_blocking` task, and the coordinator
        // remains responsible for waking followers on completion or panic.
        drop(detached);
        flight
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        let mut request =
            crate::logging::LifecycleSpan::request("initialize", || "workspace".to_owned());
        let settings = params
            .initialization_options
            .clone()
            .unwrap_or(LSPAny::Null);
        let (client_init, init_warnings) = Self::parse_client_init(&settings);
        let dynamic = params
            .capabilities
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.did_change_watched_files.as_ref())
            .and_then(|watched| watched.dynamic_registration)
            .unwrap_or(false);
        let mut roots = BTreeMap::new();
        let mut warnings = init_warnings;
        for (path, id) in Self::roots_from_initialize(&params) {
            let config_path = Self::config_override_for(&client_init, &path)
                .map(|file| file.path.clone())
                .unwrap_or_else(|| path.join(config::CONFIG_FILE));
            let (root, root_warnings) = Self::root_state(path.clone(), id, config_path);
            warnings.extend(root_warnings.iter().map(|error| error.message.clone()));
            roots.insert(path, root);
        }
        let root_count = roots.len();
        {
            let mut state = self.lock_state();
            state.roots = roots;
            state.dynamic_watched_files = dynamic;
            state.pending_logs.extend(warnings);
        }
        request.set_root(|| format!("roots={root_count}"));
        let _ = self.rescan().await;
        let result = InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Right(RenameOptions {
                    prepare_provider: Some(true),
                    work_done_progress_options: WorkDoneProgressOptions::default(),
                })),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![".".into(), ":".into(), "\u{60}".into()]),
                    ..Default::default()
                }),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            legend: crate::semantic_tokens::legend(),
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                            range: None,
                            ..Default::default()
                        },
                    ),
                ),
                workspace: Some(WorkspaceServerCapabilities {
                    workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                        supported: Some(true),
                        change_notifications: Some(OneOf::Left(true)),
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "Lapligence".to_owned(),
                version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            }),
        };
        request.complete("ok", root_count);
        Ok(result)
    }

    async fn initialized(&self, _: InitializedParams) {
        let mut notification =
            crate::logging::LifecycleSpan::notification("initialized", || "workspace".to_owned());
        let (dynamic, root_count, pending_count) = {
            let mut state = self.lock_state();
            state.initialized = true;
            state.initial_pending = state.roots.keys().cloned().collect();
            state.ready_sent = false;
            crate::llg_debug!(
                "initialized roots={} initial_pending={}",
                state.roots.len(),
                state.initial_pending.len()
            );
            (
                state.dynamic_watched_files,
                state.roots.len(),
                state.initial_pending.len(),
            )
        };
        notification.set_root(|| format!("roots={root_count}"));
        self.flush_logs().await;
        self.publish_config_diagnostics().await;
        if dynamic {
            self.register_watchers().await;
        }
        self.schedule_all(Some(notification.id()));
        self.flush_logs().await;
        if self.mark_ready_if_empty() {
            self.client
                .log_message(
                    MessageType::INFO,
                    "llg Verilog/SystemVerilog language server ready",
                )
                .await;
        }
        notification.complete("ok", pending_count);
    }

    async fn did_change_workspace_folders(&self, params: DidChangeWorkspaceFoldersParams) {
        let added_count = params.event.added.len();
        let removed_count = params.event.removed.len();
        let mut notification = crate::logging::LifecycleSpan::notification(
            "workspace/didChangeWorkspaceFolders",
            || format!("added={added_count} removed={removed_count}"),
        );
        notification.set_root(|| "workspace".to_owned());
        let mut added_keys = BTreeSet::new();
        let removed_shadows = {
            let mut state = self.lock_state();
            let mut removed_shadows = Vec::new();
            for folder in &params.event.removed {
                if let Some(path) = Self::uri_to_path(&folder.uri) {
                    state.initial_pending.remove(&path);
                    if let Some(root) = state.roots.remove(&path) {
                        removed_shadows.push(root.shadow);
                    }
                }
            }
            rebuild_dep_dependents(&mut state);
            Self::rebuild_merged(&mut state);
            removed_shadows
        };
        if !removed_shadows.is_empty() {
            // Serialize against compile jobs so a staged file is not deleted
            // while a blocking compile still reads it.  Recursive deletion is
            // unbounded blocking work (review P1-1): run on the blocking pool
            // with the staging lock acquired inside the closure.
            let _ = tokio::task::spawn_blocking(move || {
                let _staging = shadow_staging_lock()
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                for shadow in &removed_shadows {
                    shadow.cleanup();
                }
            })
            .await;
        }
        let mut warnings = Vec::new();
        let mut additions = Vec::new();
        for folder in &params.event.added {
            let Some(path) = Self::uri_to_path(&folder.uri) else {
                continue;
            };
            let already_present = self.lock_state().roots.contains_key(&path);
            if already_present {
                continue;
            }
            let config_path = path.join(config::CONFIG_FILE);
            let (root, root_warnings) =
                Self::root_state(path.clone(), folder.name.clone(), config_path);
            warnings.extend(root_warnings.iter().map(|error| error.message.clone()));
            additions.push((path, root));
        }
        {
            let mut state = self.lock_state();
            for (path, root) in additions {
                if state.roots.contains_key(&path) {
                    continue;
                }
                if !state.ready_sent {
                    state.initial_pending.insert(path.clone());
                }
                added_keys.insert(path.clone());
                state.roots.insert(path, root);
            }
            state.pending_logs.extend(warnings);
        }
        let mut schedule = self.rescan().await;
        schedule.extend(added_keys);
        if !params.event.removed.is_empty() {
            // Longest-root ownership may have changed for remaining roots even
            // when their discovered sets did not; recompile them all.
            let remaining: BTreeSet<RootKey> = {
                let state = self.lock_state();
                state.roots.keys().cloned().collect()
            };
            schedule.extend(remaining);
        }
        let schedule: Vec<_> = schedule.into_iter().collect();
        let schedule_count = schedule.len();
        self.schedule_roots_with_parent(schedule, Some(notification.id()));
        self.register_watchers().await;
        self.publish_config_diagnostics().await;
        self.flush_logs().await;
        notification.complete("ok", schedule_count);
    }

    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        let event_count = params.changes.len();
        let mut notification =
            crate::logging::LifecycleSpan::notification("workspace/didChangeWatchedFiles", || {
                format!("events={event_count}")
            });
        notification.set_root(|| "workspace".to_owned());
        let mut config_roots = BTreeSet::new();
        let mut source_roots = BTreeSet::new();
        let mut dep_roots = BTreeSet::new();
        for event in params.changes {
            let Some(path) = Self::uri_to_path(&event.uri) else {
                continue;
            };
            // The server never reacts to its own shadow/staging writes: the
            // compile/commit path stages open buffers and include deps into a
            // private tree under the OS temp dir, and those copies can fall
            // under a registered watcher glob (e.g. a TMPDIR that resolves into
            // the workspace, or a source/include dir covering the temp dir).
            // Reacting would reschedule the root from every staged write,
            // re-staging on the next run and looping forever at idle.  Skip
            // them before any dep/source/config routing; the check is robust
            // to symlinked temp dirs.
            if workspace::is_shadow_tree_path(&path) {
                continue;
            }
            // Tracked include dependencies are routed FIRST: resolved deps of
            // any extension must refresh their dependent roots even when the
            // path classifies as `Other` (.inc, .mem, ...).
            dep_roots.extend(self.dependent_roots_for_dep(&path));
            match workspace::classify_source_file_event(&path) {
                workspace::SourceFileEvent::Source(_) => {
                    if let Some(root) = self.source_root(&path) {
                        source_roots.insert(root);
                    }
                }
                workspace::SourceFileEvent::Config => {
                    if let Some(root) = self.config_root(&path) {
                        config_roots.insert(root);
                    }
                }
                workspace::SourceFileEvent::ShadowTree | workspace::SourceFileEvent::Other => {
                    // Override configs may carry any basename (e.g.
                    // `custom.toml` via `llg.configFiles`); route events
                    // whose normalized absolute path equals some root's
                    // effective config path so reload-without-restart keeps
                    // working for overridden roots (review P2-12).
                    if self.is_effective_config_path(&path) {
                        if let Some(root) = self.config_root(&path) {
                            config_roots.insert(root);
                        }
                    }
                }
            }
        }
        let mut reloaded = BTreeSet::new();
        for root in &config_roots {
            let config_path = {
                let state = self.lock_state();
                state
                    .roots
                    .get(root)
                    .map(|root| root.descriptor.config_path.clone())
            };
            if let Some(config_path) = config_path {
                if self.reload_root_config(root, &config_path) {
                    reloaded.insert(root.clone());
                }
            }
        }
        if !config_roots.is_empty() {
            self.publish_config_diagnostics().await;
        }
        if !reloaded.is_empty() {
            // An effective llg.toml reload changed backend state (e.g.
            // `[compile] defines`): tell the client so config-derived client
            // views — inactive-region dimming above all — refetch instead of
            // staying stale until the next buffer event.  Diagnostics need no
            // signal (the server republishes them itself); the notification
            // exists for data the client pulls on demand.
            self.client
                .send_notification::<LlgConfigChanged>(ConfigChangedParams {})
                .await;
            let mut state = self.lock_state();
            let notified_paths = reloaded
                .iter()
                .filter_map(|key| state.roots.get(key))
                .map(|root| root.descriptor.config_path.clone())
                .collect::<Vec<_>>();
            for path in notified_paths {
                state.pending_logs.push(format!(
                    "config reload notified the client: {}",
                    path.display()
                ));
            }
        }
        if !source_roots.is_empty() || !config_roots.is_empty() {
            // Source/config events can change discovery; rescan first.
            let mut schedule = self.rescan().await;
            schedule.extend(source_roots);
            schedule.extend(config_roots);
            schedule.extend(dep_roots);
            let schedule: Vec<_> = schedule.into_iter().collect();
            let schedule_count = schedule.len();
            self.schedule_roots_with_parent(schedule, Some(notification.id()));
            self.register_watchers().await;
            self.flush_logs().await;
            notification.complete("ok", schedule_count);
        } else if !dep_roots.is_empty() {
            // Dependency-only events do not change discovery.
            let schedule: Vec<_> = dep_roots.into_iter().collect();
            let schedule_count = schedule.len();
            self.schedule_roots_with_parent(schedule, Some(notification.id()));
            self.flush_logs().await;
            notification.complete("ok", schedule_count);
        } else {
            notification.complete("no-op", 0);
        }
    }

    async fn did_change_configuration(&self, _params: DidChangeConfigurationParams) {
        let mut notification =
            crate::logging::LifecycleSpan::notification("workspace/didChangeConfiguration", || {
                "workspace".to_owned()
            });
        // Configuration lives in each root's `llg.toml`; the LSP
        // configuration surface is intentionally unused (the client sends
        // config *paths* in initializationOptions, never settings).
        notification.complete("ignored", 0);
    }

    async fn shutdown(&self) -> Result<()> {
        let mut request =
            crate::logging::LifecycleSpan::request("shutdown", || "workspace".to_owned());
        mark_shutdown_requested();
        let shadows = {
            let mut state = self.lock_state();
            state.shutting_down = true;
            state
                .roots
                .values()
                .map(|root| root.shadow.clone())
                .collect::<Vec<_>>()
        };
        // Deterministically remove the process shadow base.  Recursive temp
        // deletion plus the tmpdir scans are unbounded blocking work (review
        // P1-1): run them on the blocking pool, with the staging lock taken
        // INSIDE the closure so cleanup stays ordered against compile jobs
        // (a job holding the lock finishes staging before files vanish; a
        // job that starts after it observes `shutting_down` and bails).
        //
        // NOTE: this handler only runs for a parameterless shutdown request;
        // the conventional `"params": null` shape is rejected by tower-lsp
        // before it reaches here.  The lifecycle interceptor in main.rs
        // performs the same cleanup for that shape, and both paths are
        // idempotent.
        let shadow_count = shadows.len();
        let cleanup =
            tokio::task::spawn_blocking(move || cleanup_shadow_state_blocking(shadows)).await;
        request.complete(if cleanup.is_ok() { "ok" } else { "error" }, shadow_count);
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let mut notification =
            crate::logging::LifecycleSpan::notification("textDocument/didOpen", || {
                Self::log_uri_identity(&params.text_document.uri)
            });
        let uri = params.text_document.uri;
        let path = Self::uri_to_path(&uri);
        let (root, root_added) = match path.as_deref() {
            Some(path) => self.ensure_source_root(path),
            None => (None, false),
        };
        if let Some(root) = &root {
            notification.set_root(|| root.to_string_lossy().into_owned());
        }
        let (initialized, admission, cached_diagnostics) = {
            let mut state = self.lock_state();
            let admission =
                Self::admit_document_text(&mut state, uri.clone(), params.text_document.text, true);
            let cached_diagnostics = match (root.as_ref(), path.as_ref()) {
                (Some(root), Some(path)) => state
                    .roots
                    .get(root)
                    .and_then(|root| root.all_diagnostics.get(path).cloned()),
                _ => None,
            };
            // Opening a document starts a new client-side lifecycle for this
            // URI. Ensure the following root commit republishes its current
            // diagnostics even when the payload matches the closed-file
            // snapshot published during initialization.
            if let Some(root) = root.as_ref().and_then(|root| state.roots.get_mut(root)) {
                root.published_digests.remove(&uri);
            }
            state.published_shared.remove(&uri);
            (state.initialized, admission, cached_diagnostics)
        };
        // Store the didOpen buffer before any await so a concurrent change
        // cannot be overwritten by the older open text. Discovery itself is
        // filesystem-only; job snapshotting below adds admitted unsaved units.
        if root_added {
            let _ = self.rescan().await;
            self.register_watchers().await;
            self.publish_config_diagnostics().await;
            self.flush_logs().await;
        }
        match admission {
            Ok(_) => {
                if initialized {
                    // A client can ignore project-wide diagnostics published
                    // before it opens a document. Replay the retained payload
                    // (or an authoritative empty one) at didOpen; later root
                    // commits still use digest suppression.
                    self.client
                        .publish_diagnostics(
                            uri.clone(),
                            cached_diagnostics.unwrap_or_default(),
                            None,
                        )
                        .await;
                    if let Some(root) = root {
                        self.schedule_roots_with_parent(vec![root], Some(notification.id()));
                    }
                }
                notification.complete(if initialized { "scheduled" } else { "deferred" }, 1);
            }
            Err(limit) => {
                limit.log_rejection();
                crate::llg_debug!(
                    "event=document.admission outcome=rejected reason=too-large uri={} bytes={} max_file_bytes={}",
                    crate::logging::bounded_field(uri.as_str()),
                    limit.measured_bytes,
                    limit.configured_limit
                );
                notification.complete("rejected-too-large", 0);
            }
        }
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let mut notification =
            crate::logging::LifecycleSpan::notification("textDocument/didChange", || {
                Self::log_uri_identity(&params.text_document.uri)
            });
        let uri = params.text_document.uri;
        let root = Self::uri_to_path(&uri).and_then(|path| self.source_root(&path));
        if let Some(root) = &root {
            notification.set_root(|| root.to_string_lossy().into_owned());
        }
        let (initialized, admission) = {
            let mut state = self.lock_state();
            let admission = params
                .content_changes
                .into_iter()
                .last()
                .map_or(Ok(false), |change| {
                    Self::admit_document_text(&mut state, uri.clone(), change.text, false)
                });
            (state.initialized, admission)
        };
        let changed = match admission {
            Ok(changed) => changed,
            Err(limit) => {
                limit.log_rejection();
                crate::llg_debug!(
                    "event=document.admission outcome=rejected reason=too-large uri={} bytes={} max_file_bytes={}",
                    crate::logging::bounded_field(uri.as_str()),
                    limit.measured_bytes,
                    limit.configured_limit
                );
                notification.complete("rejected-too-large", 0);
                return;
            }
        };
        // Identical full-text changes carry no information: scheduling a run
        // for them would only churn the analysis (and with it the request
        // memoization epochs) without changing any input.
        if initialized && changed {
            if let Some(root) = root {
                self.schedule_roots_with_parent(vec![root], Some(notification.id()));
            }
        }
        notification.complete(
            if initialized && changed {
                "scheduled"
            } else if changed {
                "deferred"
            } else {
                "unchanged"
            },
            usize::from(changed),
        );
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let mut notification =
            crate::logging::LifecycleSpan::notification("textDocument/didClose", || {
                Self::log_uri_identity(&params.text_document.uri)
            });
        let uri = params.text_document.uri;
        let root = Self::uri_to_path(&uri).and_then(|path| self.source_root(&path));
        let real = Self::uri_to_path(&uri);
        if let Some(root) = &root {
            notification.set_root(|| root.to_string_lossy().into_owned());
        }
        let (initialized, shadow) = {
            let mut state = self.lock_state();
            state.documents.remove(&uri);
            // Forget the last aggregated union so a still-tracked shared file
            // is republished from the post-close recompile.
            state.published_shared.remove(&uri);
            let shadow = root
                .as_ref()
                .and_then(|key| state.roots.get(key))
                .map(|root| root.shadow.clone());
            (state.initialized, shadow)
        };
        if let (Some(shadow), Some(real)) = (shadow, real.as_ref()) {
            let _staging = shadow_staging_lock()
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            shadow.remove(real);
        }
        // No immediate clear here: diagnostics are published project-wide,
        // so the closed URI keeps its last known findings until the
        // rescheduled recompile below refreshes them from on-disk state
        // (unchanged payloads are suppressed by digest).
        if initialized {
            if let Some(root) = root {
                self.schedule_roots_with_parent(vec![root], Some(notification.id()));
            }
        }
        notification.complete(if initialized { "scheduled" } else { "deferred" }, 1);
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let mut request =
            crate::logging::LifecycleSpan::request("textDocument/semanticTokens/full", || {
                Self::log_uri_identity(&params.text_document.uri)
            });
        let uri = params.text_document.uri;
        let (analysis, paths, open_document, max_file_bytes, shared_owner) = {
            let state = self.lock_state();
            let Some((root, real, paths)) = Self::root_context(&state, &uri) else {
                return Ok(None);
            };
            request.set_root(|| root.descriptor.id.clone());
            let analysis = root.last_good.clone();
            let open_document = state.documents.get(&uri).cloned().map(|text| {
                let defines = root.descriptor.effective_config().compile.defines;
                (real.clone(), text, defines)
            });
            let max_file_bytes = open_document
                .as_ref()
                .map(|_| root.descriptor.effective_config().analysis.max_file_bytes);
            // Semantic-token policy: the owner root wins.  In v1 only the
            // owner holds token data for a shared file, so a stream comparison
            // is not possible; log the ownership decision instead.
            let shared = shared_tracker_count(&state, &real) > 1;
            let owner_name = root.descriptor.id.clone();
            (
                analysis,
                paths,
                open_document,
                max_file_bytes,
                shared.then_some((owner_name, real)),
            )
        };
        if let Some((owner_name, real)) = &shared_owner {
            crate::llg_trace!(
                "semantic tokens: owner root {owner_name} wins for shared file {}",
                real.display()
            );
        }
        let captured_text = open_document.as_ref().map(|(_, text, _)| text.clone());
        let allow_filename_fallback = captured_text.is_none();
        let fallback_analysis = analysis.clone();
        let fallback_paths = paths.clone();

        // The request released BackendState after capturing this text.  Do
        // not even inspect/serve an open-buffer cache entry for an obsolete
        // revision; it must fall back to the committed snapshot without
        // acquiring a flight or starting isolated work.
        if let Some(captured_text) = captured_text.as_deref() {
            if !open_document_is_current(&self.state, &uri, captured_text) {
                let tokens = cached_semantic_tokens(
                    fallback_analysis.as_deref(),
                    &fallback_paths,
                    allow_filename_fallback,
                );
                request.complete("stale", tokens.data.len());
                return Ok(Some(SemanticTokensResult::Tokens(tokens)));
            }
        }
        if let Some(limit) = open_token_size_limit(open_document.as_ref(), max_file_bytes) {
            // Recheck after measuring so a concurrent edit cannot turn a
            // stale captured revision into a size-limit result.  This is
            // deliberately before key construction, flight admission,
            // staging, and spawn_blocking.
            if let Some((_, text, _)) = open_document.as_ref() {
                if !open_document_is_current(&self.state, &uri, text) {
                    let tokens = cached_semantic_tokens(
                        fallback_analysis.as_deref(),
                        &fallback_paths,
                        allow_filename_fallback,
                    );
                    request.complete("stale", tokens.data.len());
                    return Ok(Some(SemanticTokensResult::Tokens(tokens)));
                }
                limit.log_rejection();
                crate::llg_debug!("semantic tokens rejected: {}", limit.message());
                let tokens = cached_semantic_tokens(
                    fallback_analysis.as_deref(),
                    &fallback_paths,
                    allow_filename_fallback,
                );
                request.complete("too-large", tokens.data.len());
                return Ok(Some(SemanticTokensResult::Tokens(tokens)));
            }
        }
        // Memoize the request-local parse: the isolated stream is a pure
        // function of (buffer text, -D defines), so a repeat request with
        // unchanged inputs short-circuits the whole stage+parse pipeline.
        // A config hot reload that changes `compile.defines` changes the key;
        // other config edits do not affect a single-file `-parseonly` run.
        let token_key = open_document
            .as_ref()
            .map(|(_, text, defines)| open_token_cache_key(uri.as_str(), text, defines));
        let request_id = request.id();
        let started = std::time::Instant::now();
        if let Some(key) = &token_key {
            if let Some(tokens) = self.open_token_cache.get(key) {
                // didChange can race between the initial barrier and this
                // cache read.  Validate again immediately before serving it.
                let cache_is_current = captured_text
                    .as_deref()
                    .is_some_and(|text| open_document_is_current(&self.state, &uri, text));
                if !cache_is_current {
                    let tokens = cached_semantic_tokens(
                        fallback_analysis.as_deref(),
                        &fallback_paths,
                        allow_filename_fallback,
                    );
                    request.complete("stale", tokens.data.len());
                    return Ok(Some(SemanticTokensResult::Tokens(tokens)));
                }
                crate::llg_trace!(
                    "semantic tokens: served from request cache elapsed_us={} uri={}",
                    started.elapsed().as_micros(),
                    uri
                );
                request.complete("cache-hit", tokens.data.len());
                return Ok(Some(SemanticTokensResult::Tokens(tokens)));
            }
        }

        // Repeat the barrier after the cache miss and immediately before
        // flight admission.  A stale revision therefore cannot consume a
        // bounded flight slot merely because an edit landed during cache
        // lookup.
        if let Some(captured_text) = captured_text.as_deref() {
            if !open_document_is_current(&self.state, &uri, captured_text) {
                let tokens = cached_semantic_tokens(
                    fallback_analysis.as_deref(),
                    &fallback_paths,
                    allow_filename_fallback,
                );
                request.complete("stale", tokens.data.len());
                return Ok(Some(SemanticTokensResult::Tokens(tokens)));
            }
        }
        let lease = token_key
            .as_ref()
            .map(|key| self.open_token_flights.acquire(key.clone()));
        if crate::logging::enabled(crate::logging::Level::Trace) {
            let lease_kind = match lease.as_ref() {
                Some(OpenTokenFlightLease::Leader(_)) => "leader",
                Some(OpenTokenFlightLease::Follower(_)) => "follower",
                Some(OpenTokenFlightLease::Saturated) => "saturated",
                None => "no-flight",
            };
            crate::llg_trace!(
                "event=semantic_tokens.flight_admission outcome={} uri={} key_present={} active_flights={}",
                lease_kind,
                uri,
                token_key.is_some(),
                self.open_token_flights.len()
            );
        }
        let (fresh, cached, outcome) = match lease {
            Some(OpenTokenFlightLease::Follower(flight)) => {
                crate::llg_trace!(
                    "event=semantic_tokens.flight_wait.begin outcome=follower uri={}",
                    uri
                );
                let fresh = Some(flight.wait().await);
                crate::llg_trace!(
                    "event=semantic_tokens.flight_wait.end outcome=ready uri={} fresh_ok={}",
                    uri,
                    fresh.as_ref().is_some_and(|result| result.is_ok())
                );
                let cached_analysis = fallback_analysis.clone();
                let cached_paths = fallback_paths.clone();
                let cached = tokio::task::spawn_blocking(move || {
                    cached_semantic_tokens(
                        cached_analysis.as_deref(),
                        &cached_paths,
                        allow_filename_fallback,
                    )
                })
                .await
                .unwrap_or_else(|_| {
                    cached_semantic_tokens(
                        fallback_analysis.as_deref(),
                        &fallback_paths,
                        allow_filename_fallback,
                    )
                });
                (fresh, cached, "single-flight")
            }
            Some(OpenTokenFlightLease::Leader(mut leader)) => {
                crate::llg_debug!(
                    "event=semantic_tokens.flight_leader.begin uri={} request_id={}",
                    uri,
                    request_id
                );
                let key = token_key
                    .as_ref()
                    .expect("a leader always has an open-document cache key");
                // A request can miss the cache, then race a coordinator that
                // publishes and removes its flight before this request calls
                // `acquire`.  Recheck after becoming leader; if the result is
                // already cached, complete this short-lived flight from the
                // cache instead of launching a duplicate parse.
                if let Some(tokens) = self.open_token_cache.get(key) {
                    crate::llg_trace!(
                        "event=semantic_tokens.flight_leader.end outcome=cache-race uri={} token_count={}",
                        uri,
                        tokens.data.len()
                    );
                    let cache_is_current = captured_text
                        .as_deref()
                        .is_some_and(|text| open_document_is_current(&self.state, &uri, text));
                    if !cache_is_current {
                        let error = STALE_OPEN_TOKEN_ERROR.to_owned();
                        leader.finish(Err(error.clone()));
                        (
                            Some(Err(error)),
                            cached_semantic_tokens(
                                fallback_analysis.as_deref(),
                                &fallback_paths,
                                allow_filename_fallback,
                            ),
                            "stale",
                        )
                    } else {
                        leader.finish(Ok(tokens.clone()));
                        let cached_analysis = fallback_analysis.clone();
                        let cached_paths = fallback_paths.clone();
                        let cached = tokio::task::spawn_blocking(move || {
                            cached_semantic_tokens(
                                cached_analysis.as_deref(),
                                &cached_paths,
                                allow_filename_fallback,
                            )
                        })
                        .await
                        .unwrap_or_else(|_| {
                            cached_semantic_tokens(
                                fallback_analysis.as_deref(),
                                &fallback_paths,
                                allow_filename_fallback,
                            )
                        });
                        (Some(Ok(tokens)), cached, "cache-race")
                    }
                } else {
                    match open_document {
                        Some(open_document) => {
                            let captured_text = captured_text
                                .clone()
                                .expect("open-document inputs always carry captured text");
                            if !open_document_is_current(&self.state, &uri, &captured_text) {
                                let error = STALE_OPEN_TOKEN_ERROR.to_owned();
                                leader.finish(Err(error.clone()));
                                (
                                    Some(Err(error)),
                                    cached_semantic_tokens(
                                        fallback_analysis.as_deref(),
                                        &fallback_paths,
                                        allow_filename_fallback,
                                    ),
                                    "stale",
                                )
                            } else {
                                let coordinator = leader.detach();
                                crate::llg_debug!(
                            "event=semantic_tokens.flight_leader.detach outcome=compute uri={} request_id={}",
                            uri,
                            request_id
                        );
                                let flight = self.spawn_open_token_coordinator(
                                    coordinator,
                                    key.clone(),
                                    uri.clone(),
                                    captured_text,
                                    open_document,
                                    Some(request_id),
                                );
                                let fresh = Some(flight.wait().await);
                                let cached_analysis = fallback_analysis.clone();
                                let cached_paths = fallback_paths.clone();
                                let cached = tokio::task::spawn_blocking(move || {
                                    cached_semantic_tokens(
                                        cached_analysis.as_deref(),
                                        &cached_paths,
                                        allow_filename_fallback,
                                    )
                                })
                                .await
                                .unwrap_or_else(|_| {
                                    cached_semantic_tokens(
                                        fallback_analysis.as_deref(),
                                        &fallback_paths,
                                        allow_filename_fallback,
                                    )
                                });
                                (fresh, cached, "computed")
                            }
                        }
                        None => {
                            let error =
                                "open-document semantic-token inputs disappeared".to_owned();
                            leader.finish(Err(error.clone()));
                            (
                                Some(Err(error)),
                                cached_semantic_tokens(
                                    fallback_analysis.as_deref(),
                                    &fallback_paths,
                                    allow_filename_fallback,
                                ),
                                "no-document",
                            )
                        }
                    }
                }
            }
            Some(OpenTokenFlightLease::Saturated) => {
                // Saturation is a deliberate refusal: bypassing the bounded
                // registry here would recreate the unbounded spawn_blocking
                // queue this guard is meant to prevent.  Serve the project
                // snapshot (or an empty authoritative-safe result) inline.
                crate::llg_debug!(
                    "event=semantic_tokens.flight_admission.end outcome=saturated uri={} active_flights={}",
                    uri,
                    self.open_token_flights.len()
                );
                (
                    None,
                    cached_semantic_tokens(
                        fallback_analysis.as_deref(),
                        &fallback_paths,
                        allow_filename_fallback,
                    ),
                    "saturated",
                )
            }
            None => {
                crate::llg_trace!(
                    "event=semantic_tokens.flight_admission.end outcome=project-analysis uri={}",
                    uri
                );
                let current_state = Arc::clone(&self.state);
                let current_uri = uri.clone();
                let current_text = captured_text.clone();
                let task = tokio::task::spawn_blocking(move || {
                    compute_semantic_tokens(
                        analysis,
                        paths,
                        open_document,
                        move || match current_text.as_deref() {
                            Some(text) => {
                                open_document_is_current(&current_state, &current_uri, text)
                            }
                            None => !shutdown_requested(),
                        },
                        Some(request_id),
                    )
                });
                let result = task.await.unwrap_or_else(|error| {
                    crate::llg_debug!("semantic tokens: blocking task failed: {error}");
                    (
                        None,
                        cached_semantic_tokens(
                            fallback_analysis.as_deref(),
                            &fallback_paths,
                            allow_filename_fallback,
                        ),
                    )
                });
                (result.0, result.1, "computed")
            }
        };
        let buffer_is_current = captured_text
            .as_deref()
            .is_some_and(|text| open_document_is_current(&self.state, &uri, text));
        let outcome = if captured_text.is_some() && !buffer_is_current {
            "stale"
        } else {
            outcome
        };
        // Only successful fresh streams are memoized: failures fall back to
        // the project snapshot and must be retried on the next request.
        if let Some(Ok(fresh_tokens)) = &fresh {
            if buffer_is_current {
                if let Some(key) = &token_key {
                    self.open_token_cache.put(key.clone(), fresh_tokens.clone());
                }
            }
        }
        crate::llg_trace!(
            "semantic tokens: computed elapsed_us={} uri={}",
            started.elapsed().as_micros(),
            uri
        );
        let tokens = select_semantic_tokens(fresh, cached, buffer_is_current);
        request.complete(outcome, tokens.data.len());
        Ok(Some(SemanticTokensResult::Tokens(tokens)))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let mut request = crate::logging::LifecycleSpan::request("textDocument/hover", || {
            Self::log_uri_identity(&params.text_document_position_params.text_document.uri)
        });
        let (analysis, epoch, paths, position, shared_owner) = {
            let state = self.lock_state();
            let Some((root, real, paths)) = Self::root_context(
                &state,
                &params.text_document_position_params.text_document.uri,
            ) else {
                return Ok(None);
            };
            request.set_root(|| root.descriptor.id.clone());
            let Some(analysis) = root.last_good.clone() else {
                request.complete("no-analysis", 0);
                return Ok(None);
            };
            // Shared files: label configuration-dependent hover sections with
            // the owner root so users know which config produced them.
            let shared_owner =
                (shared_tracker_count(&state, &real) > 1).then(|| root.descriptor.id.clone());
            (
                analysis,
                root.analysis_epoch,
                paths,
                params.text_document_position_params.position,
                shared_owner,
            )
        };
        let key = RequestKey::new(
            RequestKind::Hover,
            params
                .text_document_position_params
                .text_document
                .uri
                .as_str(),
            position.line,
            position.character,
            epoch,
        );
        let started = std::time::Instant::now();
        if let Some(cached) = self.hover_cache.get(&key) {
            crate::llg_trace!(
                "hover: served from request cache elapsed_us={} uri={} line={} col={}",
                started.elapsed().as_micros(),
                key.uri,
                key.line,
                key.character
            );
            let value = self.annotate_hover(cached, shared_owner.as_deref());
            request.complete(
                if value.is_some() {
                    "cache-hit"
                } else {
                    "no-data"
                },
                1,
            );
            return Ok(value);
        }
        let value = tokio::task::spawn_blocking(move || {
            paths.into_iter().find_map(|path| {
                features::hover_at(&analysis, &path, position.line, position.character)
            })
        })
        .await
        .ok()
        .flatten();
        self.hover_cache.put(key.clone(), value.clone());
        crate::llg_trace!(
            "hover: computed elapsed_us={} uri={} line={} col={}",
            started.elapsed().as_micros(),
            key.uri,
            key.line,
            key.character
        );
        let value = self.annotate_hover(value, shared_owner.as_deref());
        request.complete(if value.is_some() { "ok" } else { "no-data" }, 1);
        Ok(value)
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let mut request = crate::logging::LifecycleSpan::request("textDocument/definition", || {
            Self::log_uri_identity(&params.text_document_position_params.text_document.uri)
        });
        let text_document = params.text_document_position_params.text_document;
        let position = params.text_document_position_params.position;
        let (analysis, epoch, paths) = {
            let state = self.lock_state();
            let Some((root, _, paths)) = Self::root_context(&state, &text_document.uri) else {
                return Ok(None);
            };
            request.set_root(|| root.descriptor.id.clone());
            let Some(analysis) = root.last_good.clone() else {
                request.complete("no-analysis", 0);
                return Ok(None);
            };
            (analysis, root.analysis_epoch, paths)
        };
        let key = RequestKey::new(
            RequestKind::Definition,
            text_document.uri.as_str(),
            position.line,
            position.character,
            epoch,
        );
        let started = std::time::Instant::now();
        if let Some(cached) = self.definition_cache.get(&key) {
            crate::llg_trace!(
                "definition: served from request cache elapsed_us={} uri={} line={} col={}",
                started.elapsed().as_micros(),
                key.uri,
                key.line,
                key.character
            );
            let value = cached.map(|value| {
                GotoDefinitionResponse::Scalar(Self::map_location(&self.lock_state(), value))
            });
            request.complete(
                if value.is_some() {
                    "cache-hit"
                } else {
                    "no-data"
                },
                1,
            );
            return Ok(value);
        }
        let value = tokio::task::spawn_blocking(move || {
            paths.into_iter().find_map(|path| {
                features::definition_at(&analysis, &path, position.line, position.character)
            })
        })
        .await
        .ok()
        .flatten();
        self.definition_cache.put(key.clone(), value.clone());
        crate::llg_trace!(
            "definition: computed elapsed_us={} uri={} line={} col={}",
            started.elapsed().as_micros(),
            key.uri,
            key.line,
            key.character
        );
        let value = value.map(|value| {
            GotoDefinitionResponse::Scalar(Self::map_location(&self.lock_state(), value))
        });
        request.complete(if value.is_some() { "ok" } else { "no-data" }, 1);
        Ok(value)
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let mut request = crate::logging::LifecycleSpan::request("textDocument/references", || {
            Self::log_uri_identity(&params.text_document_position.text_document.uri)
        });
        let (analysis, epoch, paths, position) = {
            let state = self.lock_state();
            let Some((root, _, paths)) =
                Self::root_context(&state, &params.text_document_position.text_document.uri)
            else {
                return Ok(None);
            };
            request.set_root(|| root.descriptor.id.clone());
            let Some(analysis) = root.last_good.clone() else {
                return Ok(Some(Vec::new()));
            };
            (
                analysis,
                root.analysis_epoch,
                paths,
                params.text_document_position.position,
            )
        };
        let include_declaration = params.context.include_declaration;
        let key = RequestKey::new(
            RequestKind::References {
                include_declaration,
            },
            params.text_document_position.text_document.uri.as_str(),
            position.line,
            position.character,
            epoch,
        );
        let started = std::time::Instant::now();
        if let Some(cached) = self.references_cache.get(&key) {
            crate::llg_trace!(
                "references: served from request cache elapsed_us={} uri={} line={} col={}",
                started.elapsed().as_micros(),
                key.uri,
                key.line,
                key.character
            );
            let value = self.map_locations(&cached);
            request.complete("cache-hit", value.len());
            return Ok(Some(value));
        }
        let values = tokio::task::spawn_blocking(move || {
            paths
                .into_iter()
                .map(|path| {
                    features::references_at_with_options(
                        &analysis,
                        &path,
                        position.line,
                        position.character,
                        include_declaration,
                    )
                })
                .find(|values| !values.is_empty())
                .unwrap_or_default()
        })
        .await
        .unwrap_or_default();
        self.references_cache.put(key.clone(), values.clone());
        crate::llg_trace!(
            "references: computed elapsed_us={} uri={} line={} col={}",
            started.elapsed().as_micros(),
            key.uri,
            key.line,
            key.character
        );
        let value = self.map_locations(&values);
        request.complete("ok", value.len());
        Ok(Some(value))
    }

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        let mut request =
            crate::logging::LifecycleSpan::request("textDocument/prepareRename", || {
                Self::log_uri_identity(&params.text_document.uri)
            });
        let (analysis, paths, position) = {
            let state = self.lock_state();
            let Some((root, _, paths)) = Self::root_context(&state, &params.text_document.uri)
            else {
                return Ok(None);
            };
            request.set_root(|| root.descriptor.id.clone());
            let Some(analysis) = root.last_good.clone() else {
                return Ok(None);
            };
            (analysis, paths, params.position)
        };
        let value = tokio::task::spawn_blocking(move || {
            paths.into_iter().find_map(|path| {
                crate::rename::prepare_rename(&analysis, &path, position.line, position.character)
            })
        })
        .await
        .ok()
        .flatten();
        let value = value.map(
            |(range, placeholder)| PrepareRenameResponse::RangeWithPlaceholder {
                range,
                placeholder,
            },
        );
        request.complete(
            if value.is_some() { "ok" } else { "no-data" },
            usize::from(value.is_some()),
        );
        Ok(value)
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let mut request = crate::logging::LifecycleSpan::request("textDocument/rename", || {
            Self::log_uri_identity(&params.text_document_position.text_document.uri)
        });
        let (analysis, paths, position, new_name) = {
            let state = self.lock_state();
            let Some((root, _, paths)) =
                Self::root_context(&state, &params.text_document_position.text_document.uri)
            else {
                return Ok(None);
            };
            request.set_root(|| root.descriptor.id.clone());
            let Some(analysis) = root.last_good.clone() else {
                return Ok(None);
            };
            (
                analysis,
                paths,
                params.text_document_position.position,
                params.new_name,
            )
        };
        // First candidate wins: an invalid name is a request-level error
        // independent of the shadow/real path choice; `Ok(None)` falls through
        // to the next candidate path like the other navigation handlers.
        let value = tokio::task::spawn_blocking(move || {
            for path in paths {
                match crate::rename::rename(
                    &analysis,
                    &path,
                    position.line,
                    position.character,
                    &new_name,
                ) {
                    Ok(Some(edit)) => return Some(Ok(Some(edit))),
                    Ok(None) => {}
                    Err(message) => return Some(Err(message)),
                }
            }
            None
        })
        .await
        .ok()
        .flatten();
        match value {
            Some(Ok(None)) | None => {
                request.complete("no-data", 0);
                Ok(None)
            }
            Some(Err(message)) => {
                request.complete("error", 0);
                Err(tower_lsp::jsonrpc::Error::invalid_params(
                    message.to_string(),
                ))
            }
            Some(Ok(Some(edit))) => {
                let state = self.lock_state();
                let edit = Backend::map_workspace_edit_uris(&state, edit);
                request.complete("ok", 1);
                Ok(Some(edit))
            }
        }
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let mut request =
            crate::logging::LifecycleSpan::request("textDocument/documentSymbol", || {
                Self::log_uri_identity(&params.text_document.uri)
            });
        let (analysis, paths) = {
            let state = self.lock_state();
            let Some((root, _, paths)) = Self::root_context(&state, &params.text_document.uri)
            else {
                return Ok(None);
            };
            request.set_root(|| root.descriptor.id.clone());
            let Some(analysis) = root.last_good.clone() else {
                return Ok(None);
            };
            (analysis, paths)
        };
        let symbols = tokio::task::spawn_blocking(move || {
            paths
                .into_iter()
                .map(|path| features::document_symbols(&analysis, &path))
                .find(|symbols| !symbols.is_empty())
                .unwrap_or_default()
        })
        .await
        .unwrap_or_default();
        if symbols.is_empty() {
            request.complete("no-data", 0);
            Ok(None)
        } else {
            let count = symbols.len();
            request.complete("ok", count);
            Ok(Some(DocumentSymbolResponse::Nested(symbols)))
        }
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        let mut request =
            crate::logging::LifecycleSpan::request("workspace/symbol", || "workspace".to_owned());
        request.set_root(|| "workspace".to_owned());
        let analysis = {
            let state = self.lock_state();
            state.merged.clone()
        };
        let Some(analysis) = analysis else {
            request.complete("no-data", 0);
            return Ok(Some(Vec::new()));
        };
        let query = params.query;
        let query_len = query.len();
        let mut values =
            tokio::task::spawn_blocking(move || features::workspace_symbols(&analysis, &query))
                .await
                .unwrap_or_default();
        crate::llg_trace!(
            "workspace symbol query_len={} result_count={}",
            query_len,
            values.len()
        );
        let state = self.lock_state();
        for value in &mut values {
            value.location = Self::map_location(&state, value.location.clone());
        }
        request.complete("ok", values.len());
        Ok(Some(values))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let mut request = crate::logging::LifecycleSpan::request("textDocument/completion", || {
            Self::log_uri_identity(&params.text_document_position.text_document.uri)
        });
        let uri = &params.text_document_position.text_document.uri;
        enum CompletionSource {
            Open(String),
            Disk(PathBuf, u64),
            TooLarge(InputSizeLimit),
        }
        let (analysis, paths, position, source) = {
            let state = self.lock_state();
            let Some((root, real, paths)) = Self::root_context(&state, uri) else {
                return Ok(None);
            };
            request.set_root(|| root.descriptor.id.clone());
            let Some(analysis) = root.last_good.clone() else {
                return Ok(None);
            };
            let max_file_bytes = root.descriptor.effective_config().analysis.max_file_bytes;
            let source = match state.documents.get(uri) {
                Some(text) => open_input_size_limit(&real, text, max_file_bytes).map_or_else(
                    || CompletionSource::Open(text.to_string()),
                    CompletionSource::TooLarge,
                ),
                None => CompletionSource::Disk(real, max_file_bytes),
            };
            (
                analysis,
                paths,
                params.text_document_position.position,
                source,
            )
        };
        let source_too_large = matches!(&source, CompletionSource::TooLarge(_));
        let values = tokio::task::spawn_blocking(move || {
            let text = match source {
                CompletionSource::Open(text) => text,
                CompletionSource::Disk(path, max_file_bytes) => {
                    match read_closed_input_snapshot(&path, max_file_bytes) {
                        Ok(text) => text,
                        Err(error) => {
                            error.log_rejection();
                            crate::llg_debug!(
                                "event=completion.source outcome=unavailable path={} message={}",
                                error.path.display(),
                                error.message()
                            );
                            return Vec::new();
                        }
                    }
                }
                CompletionSource::TooLarge(limit) => {
                    limit.log_rejection();
                    crate::llg_debug!(
                        "event=completion.source outcome=too-large path={} message={}",
                        limit.path.display(),
                        limit.message()
                    );
                    return Vec::new();
                }
            };
            let line = text
                .lines()
                .nth(position.line as usize)
                .unwrap_or("")
                .to_owned();
            paths
                .into_iter()
                .map(|path| {
                    features::completion_at(
                        &analysis,
                        &path,
                        position.line,
                        position.character,
                        &line,
                    )
                })
                .find(|values| !values.is_empty())
                .unwrap_or_default()
        })
        .await
        .unwrap_or_default();
        if values.is_empty() {
            request.complete(
                if source_too_large {
                    "too-large"
                } else {
                    "no-data"
                },
                0,
            );
            Ok(None)
        } else {
            let count = values.len();
            request.complete("ok", count);
            Ok(Some(CompletionResponse::Array(values)))
        }
    }
}

impl Backend {
    /// Serve one document's formatted token dump from the committed snapshot.
    pub(crate) async fn dump_tokens(&self, params: DumpTokensParams) -> Result<DumpTokensResult> {
        let mut request = crate::logging::LifecycleSpan::request("llg/dumpTokens", || {
            Self::log_uri_identity(&params.uri)
        });
        let resolved = {
            let state = self.lock_state();
            match Self::root_context(&state, &params.uri) {
                None => Err("unknown document".to_owned()),
                Some((root, _real, candidates)) => {
                    request.set_root(|| root.descriptor.id.clone());
                    match root.last_good.clone() {
                        None => Err("no analysis available yet for this document".to_owned()),
                        Some(analysis) => Ok((analysis, candidates, root.descriptor.root.clone())),
                    }
                }
            }
        };
        let (analysis, candidates, display_root) = match resolved {
            Ok(parts) => parts,
            Err(reason) => {
                request.complete("no-data", 1);
                return Ok(dump_tokens_error(reason));
            }
        };
        let mut lines = tokio::task::spawn_blocking(move || {
            let rows = dump::collect_rows_for(&analysis, &display_root, &candidates);
            let mut lines = dump::format_rows(
                &format!("root={}", display_root.display()),
                &rows,
                &analysis,
            );
            // The CLI-only report header stays out of the wire response: the
            // contract is the formatted rows plus the trailing summary line.
            let _ = lines.remove(0);
            lines
        })
        .await
        .unwrap_or_else(|_| vec!["# error: dump task failed".to_owned()]);
        insert_cache_stats_line(&mut lines, self.cache_stats());
        request.complete("ok", lines.len());
        Ok(DumpTokensResult { lines })
    }

    /// Custom request `llg/moduleExplorer`: return all committed module
    /// definitions and all elaborated top instances across the workspace.
    ///
    /// The optional root filter is useful for clients displaying one
    /// workspace folder at a time; omitting it returns a deterministic
    /// multi-root snapshot.  The request only reads retained `Analysis`
    /// values.  In particular, it never starts a compile or reads a project
    /// file, and a syntax-fallback analysis still contributes its parsed
    /// module definitions even though it has no instance tree.
    pub(crate) async fn module_explorer(
        &self,
        params: Option<ModuleExplorerParams>,
    ) -> Result<module_explorer::ExplorerSnapshot> {
        let mut request = crate::logging::LifecycleSpan::request("llg/moduleExplorer", || {
            params
                .as_ref()
                .and_then(|params| params.workspace_uri.as_ref())
                .map(Self::log_uri_identity)
                .unwrap_or_else(|| "workspace:*".to_owned())
        });
        let root_filter = params
            .and_then(|params| params.workspace_uri)
            .and_then(|uri| Self::uri_to_path(&uri));
        if let Some(root_filter) = &root_filter {
            request.set_root(|| root_filter.to_string_lossy().into_owned());
        } else {
            request.set_root(|| "workspace".to_owned());
        }
        let roots = {
            let state = self.lock_state();
            state
                .roots
                .iter()
                .filter(|(key, _)| match root_filter.as_ref() {
                    Some(filter) => *key == filter,
                    None => true,
                })
                .filter_map(|(key, root)| {
                    let analysis = root.last_good.clone()?;
                    Some((
                        key.to_string_lossy().into_owned(),
                        root.shadow.clone(),
                        analysis,
                    ))
                })
                .collect::<Vec<_>>()
        };
        let task_result = tokio::task::spawn_blocking(move || {
            // The request is one serialized response even when it combines
            // several workspace roots. Share the hierarchy budget across all
            // snapshots before merging them; allocating one budget per root
            // would allow the merged JSON to exceed the server-side cap.
            let mut budget = module_explorer::new_response_budget();
            budget.prepare_workspaces();
            let workspace_count = roots.len();
            let snapshots = roots.into_iter().enumerate().map(
                |(workspace_index, (root_id, shadow, analysis))| {
                    budget.begin_workspace(workspace_count - workspace_index);
                    let mut snapshot = module_explorer::snapshot_analysis_with_budget(
                        &root_id,
                        &analysis,
                        |path| shadow.real_path(path).or_else(|| Some(path.to_path_buf())),
                        &mut budget,
                    );
                    module_explorer::remap_uris(&mut snapshot, |path| {
                        shadow.real_path(path).or_else(|| Some(path.to_path_buf()))
                    });
                    snapshot
                },
            );
            module_explorer::merge(snapshots)
        })
        .await;
        let ModuleExplorerTaskResult {
            snapshot,
            outcome,
            error,
        } = module_explorer_task_result(task_result);
        let cardinality = snapshot.modules.len() + snapshot.roots.len();
        let truncated_modules = snapshot
            .modules
            .iter()
            .filter(|module| module.is_budget_truncated)
            .count();
        if let Some(error) = error {
            crate::llg_error!(
                "event=module_explorer.snapshot.end outcome=error error_kind=task error={} modules={} roots={} truncated_modules={}",
                error,
                snapshot.modules.len(),
                snapshot.roots.len(),
                truncated_modules
            );
        } else {
            crate::llg_debug!(
                "event=module_explorer.snapshot.end outcome=ok modules={} roots={} truncated_modules={}",
                snapshot.modules.len(),
                snapshot.roots.len(),
                truncated_modules
            );
        }
        request.complete(outcome, cardinality);
        Ok(snapshot)
    }

    /// Custom request `llg/inactiveRanges`: the zero-based inclusive line
    /// ranges a preprocessor would SKIP for one document under the owner
    /// root's effective `[compile] defines`, so editors can dim skipped
    /// conditional-compilation branches.
    ///
    /// Serves COMMITTED state only — the staged open-buffer text (same store
    /// `semantic_tokens/full` reads) or the on-disk source otherwise.  The
    /// computation is a pure lexical scan (`inactive_ranges`): no Slang
    /// work, no parsing, no diagnostics, no snapshot mutation.  Unknown or
    /// unowned documents answer an EMPTY range list; nothing here fails.
    pub(crate) async fn inactive_ranges(
        &self,
        params: InactiveRangesParams,
    ) -> Result<InactiveRangesResult> {
        let mut request = crate::logging::LifecycleSpan::request("llg/inactiveRanges", || {
            Self::log_uri_identity(&params.uri)
        });
        enum Source {
            Open(String),
            Disk(PathBuf, u64),
            TooLarge(InputSizeLimit),
        }
        let resolved = {
            let state = self.lock_state();
            match Self::root_context(&state, &params.uri) {
                None => None,
                Some((root, real, _candidates)) => {
                    request.set_root(|| root.descriptor.id.clone());
                    let defines = root.descriptor.effective_config().compile.defines.clone();
                    let max_file_bytes = root.descriptor.effective_config().analysis.max_file_bytes;
                    let source = match state.documents.get(&params.uri) {
                        Some(text) => open_input_size_limit(&real, text, max_file_bytes)
                            .map_or_else(|| Source::Open(text.to_string()), Source::TooLarge),
                        None => Source::Disk(real, max_file_bytes),
                    };
                    Some((source, defines))
                }
            }
        };
        let Some((source, defines)) = resolved else {
            request.complete("no-data", 0);
            return Ok(InactiveRangesResult { ranges: Vec::new() });
        };
        let text = match source {
            Source::Open(text) => text,
            Source::TooLarge(limit) => {
                limit.log_rejection();
                crate::llg_debug!(
                    "event=llg.inactive_ranges.source outcome=too-large path={} message={}",
                    limit.path.display(),
                    limit.message()
                );
                request.complete("too-large", 0);
                return Ok(InactiveRangesResult { ranges: Vec::new() });
            }
            Source::Disk(path, max_file_bytes) => {
                match tokio::task::spawn_blocking(move || {
                    read_closed_input_snapshot(&path, max_file_bytes)
                })
                .await
                {
                    Ok(Ok(text)) => text,
                    Ok(Err(limit)) => {
                        limit.log_rejection();
                        crate::llg_debug!(
                            "event=llg.inactive_ranges.source outcome=unavailable path={} message={}",
                            limit.path.display(),
                            limit.message()
                        );
                        request.complete("no-data", 0);
                        return Ok(InactiveRangesResult { ranges: Vec::new() });
                    }
                    Err(error) => {
                        crate::llg_debug!(
                            "event=llg.inactive_ranges.source outcome=task-error error={}",
                            error
                        );
                        request.complete("no-data", 0);
                        return Ok(InactiveRangesResult { ranges: Vec::new() });
                    }
                }
            }
        };
        // Memoize exactly like the isolated open-token stream: the ranges are
        // a pure function of (text, -D defines), so the key hashes both and
        // needs NO analysis epoch — identical didChange re-sends and repeat
        // requests short-circuit while edits or defines hot reloads miss.
        let key = inactive_ranges_cache_key(params.uri.as_str(), &text, &defines);
        if let Some(ranges) = self.inactive_cache.get(&key) {
            request.complete("cache-hit", ranges.len());
            return Ok(InactiveRangesResult { ranges });
        }
        let ranges = tokio::task::spawn_blocking(move || {
            crate::inactive_ranges::inactive_line_ranges(&text, &defines)
        })
        .await
        .unwrap_or_default();
        self.inactive_cache.put(key, ranges.clone());
        request.complete("ok", ranges.len());
        Ok(InactiveRangesResult { ranges })
    }
}

#[cfg(test)]
mod tests;
