//! tower-lsp backend for the llg language server.
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use tokio::sync::Notify;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use crate::config::{self, ConfigError, LlgConfig};
use crate::dump;
use crate::features::{self, Analysis};
use crate::module_explorer;
use crate::request_cache::{
    next_analysis_epoch, CacheStats, MemoCache, RequestKey, RequestKind, NAVIGATION_CACHE_CAPACITY,
    TOKEN_CACHE_CAPACITY,
};
use crate::scheduler::{FireDecision, SchedulerState, TriggerDecision};
use crate::workspace::{self, RootDescriptor};
use llg::cancel::CancelToken;
use llg::core::compile::CompileOpts;
use llg::core::lint::LintConfig;
use llg::profile::Stage;

mod diagnostics;
mod identifiers;
mod inputs;
mod positions;
mod scheduling;
mod staging;
mod state;

use diagnostics::*;
use inputs::FileWrite;
use positions::{Places, Query, Served};
#[cfg(test)]
use scheduling::*;
use staging::*;
pub(crate) use staging::{emergency_shadow_cleanup, emergency_shadow_cleanup_and_exit};
pub(crate) use state::Backend;
use state::{
    stream_digest, BackendState, CommitOutcome, CompileResult, DiscoveryOutcome, JobEnd,
    RescanResult, RootJob, RootState,
};

/// Delay between the first trigger of an idle root and its analysis. Later
/// triggers join the run and do not restart the delay, so a burst of events
/// (a workspace-wide save, a checkout) costs one run (see `crate::scheduler`).
const RECOMPILE_DEBOUNCE: Duration = Duration::from_millis(300);
const WATCH_REGISTRATION_ID: &str = "llg-watched-files";
/// The client-to-server initialization protocol version this server accepts.
const CLIENT_PROTOCOL_VERSION: u32 = 1;
type RootKey = PathBuf;
type SharedText = Arc<String>;
type OpenDocuments = BTreeMap<PathBuf, SharedText>;
/// Set once the client's `shutdown` request has been handled; read by the
/// lifecycle interceptor in `transport.rs` to pick the spec-mandated process
/// exit code (0 after `shutdown`, otherwise 1).
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
    cancel_analyses();
}

/// The token of the isolated parses, and the parent of every analysis' token.
fn server_token() -> &'static CancelToken {
    static TOKEN: OnceLock<CancelToken> = OnceLock::new();
    TOKEN.get_or_init(CancelToken::new)
}

/// Cancel every analysis and isolated parse, now and later: the server ends,
/// and its cleanup waits for the running ones.
pub(crate) fn cancel_analyses() {
    server_token().cancel();
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
        let announces_saves = params
            .capabilities
            .text_document
            .as_ref()
            .and_then(|document| document.synchronization.as_ref())
            .and_then(|synchronization| synchronization.did_save)
            .unwrap_or(false);
        let semantic_tokens_refresh = params
            .capabilities
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.semantic_tokens.as_ref())
            .and_then(|tokens| tokens.refresh_support)
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
            state.semantic_tokens_refresh = semantic_tokens_refresh;
            state.pending_logs.extend(warnings);
        }
        if !announces_saves {
            crate::llg_info!(
                "event=initialize.client did_save=false: an edited file is analysed when a watched-file event reports its write"
            );
        }
        request.set_root(|| format!("roots={root_count}"));
        let _ = self.rescan().await;
        let result = InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                            // The client's text is the one of its last change.
                            include_text: Some(false),
                        })),
                        ..Default::default()
                    },
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
                // SystemVerilog has no color literals. Answering
                // `textDocument/documentColor` with an empty list keeps VS
                // Code's default color decorators (on with
                // `editor.defaultColorDecorators: "auto"`) from showing a
                // color picker beside delays such as `#10`
                // (docs/semantic_tokens.md, ST-66).
                color_provider: Some(ColorProviderCapability::Simple(true)),
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
            scheduling::register_watchers_with(&self.client, &self.state).await;
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
        self.register_watchers();
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
        let mut writes = Vec::new();
        let mut sources = false;
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
            let mut readers = self.dependent_roots_for_dep(&path);
            match workspace::classify_source_file_event(&path) {
                workspace::SourceFileEvent::Source(_) => {
                    if let Some(root) = self.source_root(&path) {
                        sources = true;
                        readers.insert(root);
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
            if !readers.is_empty() {
                writes.push(FileWrite {
                    path,
                    created: event.typ == FileChangeType::CREATED,
                    readers,
                });
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
        let mut schedule = reloaded;
        if sources || !config_roots.is_empty() {
            // Source/config events can change discovery; rescan first.
            let rescanned = self.rescan().await;
            // The diagnostics of a config include that it selects no files.
            if !config_roots.is_empty() || !rescanned.is_empty() {
                self.publish_config_diagnostics().await;
            }
            let state = self.lock_state();
            schedule.extend(rescanned.into_iter().filter(|key| state.units_changed(key)));
        }
        schedule.extend(self.roots_outdated_by(writes).await);
        let schedule_count = schedule.len();
        if schedule_count != 0 {
            let schedule = schedule.into_iter().collect();
            self.schedule_roots_with_parent(schedule, Some(notification.id()));
        }
        if sources || !config_roots.is_empty() {
            self.register_watchers();
        }
        self.flush_logs().await;
        notification.complete(
            if schedule_count != 0 { "ok" } else { "no-op" },
            schedule_count,
        );
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
        let (initialized, admission, cached_diagnostics, ticket) = {
            let mut state = self.lock_state();
            let admission =
                Self::admit_document_text(&mut state, uri.clone(), params.text_document.text, true);
            let cached_diagnostics = match (root.as_ref(), path.as_ref()) {
                (Some(root), Some(path)) => retained_diagnostics(&state, root, path),
                _ => Vec::new(),
            };
            // Opening a document starts a new client-side lifecycle for this
            // URI. Ensure the following root commit republishes its current
            // diagnostics even when the payload matches the closed-file
            // snapshot published during initialization.
            if let Some(root) = root.as_ref().and_then(|root| state.roots.get_mut(root)) {
                root.published_digests.remove(&uri);
            }
            state.published_shared.remove(&uri);
            // The replay below can run after awaits; the ticket taken with
            // the payload keeps it from replacing a commit published meanwhile.
            let ticket = state.client_order.ticket();
            (state.initialized, admission, cached_diagnostics, ticket)
        };
        // Store the didOpen buffer before any await so a concurrent change
        // cannot be overwritten by the older open text. Discovery itself is
        // filesystem-only; job snapshotting below adds admitted unsaved units.
        if root_added {
            let _ = self.rescan().await;
            self.register_watchers();
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
                    publish_ordered(
                        &self.client,
                        &self.state,
                        ticket,
                        vec![(uri.clone(), cached_diagnostics)],
                    )
                    .await;
                }
                let schedule = root.filter(|root| {
                    initialized && (root_added || !self.lock_state().document_covered(root, &uri))
                });
                notification.complete(
                    match (initialized, &schedule) {
                        (false, _) => "deferred",
                        (true, Some(_)) => "scheduled",
                        (true, None) => "covered",
                    },
                    1,
                );
                if let Some(root) = schedule {
                    self.schedule_roots_with_parent(vec![root], Some(notification.id()));
                }
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
        let (admission, reload) = {
            let mut state = self.lock_state();
            let admission = params
                .content_changes
                .into_iter()
                .last()
                .map_or(Ok(false), |change| {
                    Self::admit_document_text(&mut state, uri.clone(), change.text, false)
                });
            let reload = root.filter(|root| {
                matches!(admission, Ok(true))
                    && state.take_reload(&uri)
                    && !state.document_covered(root, &uri)
            });
            (admission, reload)
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
        self.cancel_superseded_parses(&uri);
        // An edit is analysed when the user saves; until then the isolated
        // parse of the document answers for it (`identifiers.rs`), and the
        // analysis at the places that the edit left (`positions.rs`).
        notification.complete(
            match (changed, &reload) {
                (true, Some(_)) => "reloaded",
                (true, None) => "admitted",
                (false, _) => "unchanged",
            },
            usize::from(changed),
        );
        if let Some(root) = reload {
            self.schedule_roots_with_parent(vec![root], Some(notification.id()));
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let mut notification =
            crate::logging::LifecycleSpan::notification("textDocument/didSave", || {
                Self::log_uri_identity(&params.text_document.uri)
            });
        let uri = params.text_document.uri;
        let Some(path) = Self::uri_to_path(&uri) else {
            return;
        };
        let owner = self.source_root(&path);
        if let Some(root) = &owner {
            notification.set_root(|| root.to_string_lossy().into_owned());
        }
        let (initialized, open, mut schedule) = {
            let mut state = self.lock_state();
            state.reloads.remove(&uri);
            let schedule: BTreeSet<RootKey> = owner
                .iter()
                .filter(|root| !state.documents_covered(root))
                .cloned()
                .collect();
            let open = state.documents.contains_key(&uri);
            (state.initialized, open, schedule)
        };
        if !initialized {
            notification.complete("deferred", 0);
            return;
        }
        // At once: it also cancels the analysis of the older texts.
        let parent_id = Some(notification.id());
        self.schedule_roots_with_parent(schedule.iter().cloned().collect(), parent_id);
        // Other roots read their include dependencies from the disk.
        let mut readers = self.dependent_roots_for_dep(&path);
        if let Some(owner) = owner {
            if open {
                readers.remove(&owner);
            } else {
                readers.insert(owner);
            }
        }
        let write = FileWrite {
            path,
            created: false,
            readers,
        };
        let outdated = self.roots_outdated_by(vec![write]).await;
        self.schedule_roots_with_parent(outdated.iter().cloned().collect(), parent_id);
        schedule.extend(outdated);
        notification.complete(
            if schedule.is_empty() {
                "covered"
            } else {
                "scheduled"
            },
            schedule.len(),
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
            state.remove_document(&uri);
            state.reloads.remove(&uri);
            // Forget the last aggregated union so a still-tracked shared file
            // is republished from the post-close recompile.
            state.published_shared.remove(&uri);
            let shadow = root
                .as_ref()
                .and_then(|key| state.roots.get(key))
                .map(|root| root.shadow.clone());
            (state.initialized, shadow)
        };
        self.cancel_superseded_parses(&uri);
        if let (Some(shadow), Some(real)) = (shadow, real.clone()) {
            // Beside this message: the lock is held while an analysis runs.
            tokio::task::spawn_blocking(move || {
                let _staging = shadow_staging_lock()
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                shadow.remove(&real);
            });
        }
        // No immediate clear here: diagnostics are published project-wide,
        // so the closed URI keeps its last known findings until a recompile
        // refreshes them from on-disk state (unchanged payloads are
        // suppressed by digest).
        let schedule = match (initialized, root, real) {
            (true, Some(root), Some(path)) => {
                let write = FileWrite {
                    path,
                    created: false,
                    readers: BTreeSet::from([root]),
                };
                self.roots_outdated_by(vec![write]).await
            }
            _ => BTreeSet::new(),
        };
        notification.complete(
            match (initialized, schedule.is_empty()) {
                (false, _) => "deferred",
                (true, false) => "scheduled",
                (true, true) => "covered",
            },
            1,
        );
        let schedule = schedule.into_iter().collect();
        self.schedule_roots_with_parent(schedule, Some(notification.id()));
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
        let Some(document) = self.document_request(&uri, true, &mut request).await else {
            return Ok(None);
        };
        let (tokens, document) =
            match tokio::task::spawn_blocking(move || (document.semantic_tokens(), document)).await
            {
                Ok(result) => result,
                Err(error) => {
                    crate::llg_debug!("semantic tokens: blocking task failed: {error}");
                    return Ok(None);
                }
            };
        self.record_served_stream(&uri, &document, &tokens);
        request.complete(document.outcome, tokens.data.len());
        Ok(Some(SemanticTokensResult::Tokens(tokens)))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let mut request = crate::logging::LifecycleSpan::request("textDocument/hover", || {
            Self::log_uri_identity(&params.text_document_position_params.text_document.uri)
        });
        let params = params.text_document_position_params;
        let (uri, position) = (params.text_document.uri, params.position);
        let Some(query) = self.query(&uri, &mut request) else {
            return Ok(None);
        };
        let Some(served) = query.served.clone() else {
            request.complete("no-analysis", 0);
            return Ok(None);
        };
        // Shared files: label configuration-dependent hover sections with
        // the owner root so users know which config produced them.
        let shared_owner = query.shared_owner(&self.lock_state());
        let Some(analysed) = query.analysed(position).await else {
            request.complete("stale", 0);
            return Ok(None);
        };
        let key = RequestKey::new(
            RequestKind::Hover,
            uri.as_str(),
            analysed.line,
            analysed.character,
            served.epoch,
        );
        let started = std::time::Instant::now();
        let cached = self.hover_cache.get(&key);
        let hit = cached.is_some();
        let Query {
            path,
            placement,
            settled,
            ..
        } = query;
        let mut places = Places::new(&self.state, &served);
        let compute = move || {
            let text = cached.unwrap_or_else(|| {
                features::hover_text_at(&served.analysis, &path, analysed.line, analysed.character)
            });
            let value = text.as_ref().and_then(|text| {
                let range = placement.current(text.range())?;
                Some(text.render(range, |definition| places.line(definition)))
            });
            (text, value)
        };
        let (text, value) = if hit && settled {
            compute()
        } else {
            blocking("hover", compute).await.unwrap_or_default()
        };
        crate::llg_trace!(
            "hover: {} elapsed_us={} uri={} line={} col={}",
            if hit {
                "served from request cache"
            } else {
                "computed"
            },
            started.elapsed().as_micros(),
            key.uri,
            key.line,
            key.character
        );
        if !hit {
            self.hover_cache.put(key, text);
        }
        let value = self.annotate_hover(value, shared_owner.as_deref());
        request.complete(
            match (&value, hit) {
                (None, _) => "no-data",
                (Some(_), true) => "cache-hit",
                (Some(_), false) => "ok",
            },
            1,
        );
        Ok(value)
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let mut request = crate::logging::LifecycleSpan::request("textDocument/definition", || {
            Self::log_uri_identity(&params.text_document_position_params.text_document.uri)
        });
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let Some(document) = self.document_request(&uri, false, &mut request).await else {
            return Ok(None);
        };
        let outcome = document.outcome;
        let state = Arc::clone(&self.state);
        let mut locations = blocking("definition", move || {
            document.definitions_at(&state, position)
        })
        .await
        .unwrap_or_default();
        request.complete(outcome, locations.len());
        Ok(match locations.len() {
            0 => None,
            1 => locations.pop().map(GotoDefinitionResponse::Scalar),
            _ => Some(GotoDefinitionResponse::Array(locations)),
        })
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let mut request = crate::logging::LifecycleSpan::request("textDocument/references", || {
            Self::log_uri_identity(&params.text_document_position.text_document.uri)
        });
        let include_declaration = params.context.include_declaration;
        let params = params.text_document_position;
        let (uri, position) = (params.text_document.uri, params.position);
        let Some(query) = self.query(&uri, &mut request) else {
            return Ok(None);
        };
        let Some(served) = query.served.clone() else {
            return Ok(Some(Vec::new()));
        };
        let Some(analysed) = query.analysed(position).await else {
            request.complete("stale", 0);
            return Ok(Some(Vec::new()));
        };
        let key = RequestKey::new(
            RequestKind::References {
                include_declaration,
            },
            uri.as_str(),
            analysed.line,
            analysed.character,
            served.epoch,
        );
        let started = std::time::Instant::now();
        let cached = self.references_cache.get(&key);
        let hit = cached.is_some();
        let Query { path, settled, .. } = query;
        let mut places = Places::new(&self.state, &served);
        let compute = move || {
            let references = cached.unwrap_or_else(|| {
                features::references_at_with_options(
                    &served.analysis,
                    &path,
                    analysed.line,
                    analysed.character,
                    include_declaration,
                )
            });
            // They all spell the name of the identifier.
            let name =
                features::identifier_at(&served.analysis, &path, analysed.line, analysed.character)
                    .and_then(|token| token.name.as_deref())
                    .unwrap_or_default();
            let value: Vec<Location> = references
                .iter()
                .filter_map(|reference| places.location(Some(name), reference))
                .collect();
            places.log("textDocument/references", value.len());
            (references, value)
        };
        let (references, value) = if hit && settled {
            compute()
        } else {
            blocking("references", compute).await.unwrap_or_default()
        };
        crate::llg_trace!(
            "references: {} elapsed_us={} uri={} line={} col={}",
            if hit {
                "served from request cache"
            } else {
                "computed"
            },
            started.elapsed().as_micros(),
            key.uri,
            key.line,
            key.character
        );
        if !hit {
            self.references_cache.put(key, references);
        }
        request.complete(if hit { "cache-hit" } else { "ok" }, value.len());
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
        let Some(query) = self.query(&params.text_document.uri, &mut request) else {
            return Ok(None);
        };
        let Some(served) = query.served else {
            return Ok(None);
        };
        if !query.placement.is_same() {
            request.complete("stale", 0);
            return Err(content_modified());
        }
        let (path, position) = (query.path, params.position);
        let value = blocking("prepareRename", move || {
            crate::rename::prepare_rename(
                &served.analysis,
                &path,
                position.line,
                position.character,
            )
        })
        .await
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
        let uri = params.text_document_position.text_document.uri;
        let Some(query) = self.query(&uri, &mut request) else {
            return Ok(None);
        };
        let Some(served) = query.served else {
            return Ok(None);
        };
        if !query.placement.is_same() {
            request.complete("stale", 0);
            return Err(content_modified());
        }
        let (path, max_file_bytes) = (query.path, query.max_file_bytes);
        let position = params.text_document_position.position;
        let new_name = params.new_name;
        let compiled = Arc::clone(&served.analysis);
        let value = blocking("rename", move || {
            crate::rename::rename(
                &compiled,
                &path,
                position.line,
                position.character,
                &new_name,
            )
        })
        .await;
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
                let edit = Backend::map_workspace_edit_uris(&self.lock_state(), edit);
                // An edit at the position of an older text would change
                // something else.
                let current = edit
                    .changes
                    .iter()
                    .flat_map(HashMap::keys)
                    .all(|uri| self.compiled_current_text(&served, uri, max_file_bytes));
                if !current {
                    request.complete("stale", 0);
                    return Err(content_modified());
                }
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
        let Some(query) = self.query(&params.text_document.uri, &mut request) else {
            return Ok(None);
        };
        let Some(served) = query.served else {
            return Ok(None);
        };
        let Query {
            path, placement, ..
        } = query;
        let symbols = blocking("documentSymbol", move || {
            let symbols = features::document_symbols(&served.analysis, &path);
            positions::current_symbols(&placement, symbols)
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
        let roots: Vec<(Served, Places)> = {
            let state = self.lock_state();
            state
                .roots
                .values()
                .filter_map(RootState::served)
                .map(|served| {
                    let places = Places::new(&self.state, &served);
                    (served, places)
                })
                .collect()
        };
        if roots.is_empty() {
            request.complete("no-data", 0);
            return Ok(Some(Vec::new()));
        }
        let query = params.query;
        let query_len = query.len();
        let values = blocking("workspaceSymbol", move || {
            positions::workspace_symbols(roots, &query)
        })
        .await
        .unwrap_or_default();
        crate::llg_trace!(
            "workspace symbol query_len={} result_count={}",
            query_len,
            values.len()
        );
        request.complete("ok", values.len());
        Ok(Some(values))
    }

    /// Always an empty list, never null: VS Code skips its default color
    /// decorators only when a provider answers with an array.
    async fn document_color(&self, _: DocumentColorParams) -> Result<Vec<ColorInformation>> {
        Ok(Vec::new())
    }

    async fn color_presentation(
        &self,
        _: ColorPresentationParams,
    ) -> Result<Vec<ColorPresentation>> {
        Ok(Vec::new())
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let mut request = crate::logging::LifecycleSpan::request("textDocument/completion", || {
            Self::log_uri_identity(&params.text_document_position.text_document.uri)
        });
        let uri = &params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        enum CompletionSource {
            Open(SharedText),
            Disk(PathBuf, u64),
            TooLarge(InputSizeLimit),
        }
        let Some(query) = self.query(uri, &mut request) else {
            return Ok(None);
        };
        let Some(served) = query.served else {
            return Ok(None);
        };
        let max_file_bytes = query.max_file_bytes;
        let path = PathBuf::from(&query.path);
        let (source, unsaved) = {
            let state = self.lock_state();
            if state.documents.is_rejected(uri) {
                request.complete("too-large", 0);
                return Ok(None);
            }
            let source = match state.documents.get(uri) {
                Some(text) => open_input_size_limit(&path, text, max_file_bytes).map_or_else(
                    || CompletionSource::Open(Arc::clone(text)),
                    CompletionSource::TooLarge,
                ),
                None => CompletionSource::Disk(path.clone(), max_file_bytes),
            };
            let unsaved = (!query.placement.is_same())
                .then(|| {
                    let root = state.roots.get(&query.root)?;
                    root.open_document(&state.documents, uri, &path)
                })
                .flatten();
            (source, unsaved)
        };
        let unsaved = match unsaved {
            Some(document) => {
                // Not waited for: the names of the last one are offered.
                self.isolated_parse(uri, document, max_file_bytes, request.id());
                self.lock_state().unsaved_names.get(uri).cloned()
            }
            None => None,
        }
        .unwrap_or_default();
        let source_too_large = matches!(&source, CompletionSource::TooLarge(_));
        let values = blocking("completion", move || {
            let line = match source {
                CompletionSource::Open(text) => line_of(&text, position.line),
                CompletionSource::Disk(path, max_file_bytes) => {
                    match read_closed_input_snapshot(&path, max_file_bytes) {
                        Ok(read) => line_of(&read.text, position.line),
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
            features::completion_at(&served.analysis, &unsaved, position.character, &line)
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

/// A line of `text`, empty when it has none there.
fn line_of(text: &str, line: u32) -> String {
    llg::core::tokens::SourcePositions::new(text)
        .lines()
        .nth(line as usize)
        .unwrap_or_default()
        .to_owned()
}

/// The result of `work` on the blocking pool; `None` when it panicked.
async fn blocking<T: Send + 'static>(
    request: &'static str,
    work: impl FnOnce() -> T + Send + 'static,
) -> Option<T> {
    match tokio::task::spawn_blocking(work).await {
        Ok(value) => Some(value),
        Err(error) => {
            crate::llg_debug!("event=request.task outcome=error request={request} error={error}");
            None
        }
    }
}

/// The answer to a request whose document the committed analysis has not
/// compiled yet; its save starts the analysis that does.
fn content_modified() -> tower_lsp::jsonrpc::Error {
    tower_lsp::jsonrpc::Error {
        code: tower_lsp::jsonrpc::ErrorCode::ContentModified,
        message: "the current text is not analysed yet: save the edited files, without syntax \
                  errors, and try again"
            .into(),
        data: None,
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
                    match root.served() {
                        None => Err("no analysis available yet for this document".to_owned()),
                        Some(served) => Ok((served, candidates, root.descriptor.root.clone())),
                    }
                }
            }
        };
        let (served, candidates, display_root) = match resolved {
            Ok(parts) => parts,
            Err(reason) => {
                request.complete("no-data", 1);
                return Ok(dump_tokens_error(reason));
            }
        };
        let mut places = Places::new(&self.state, &served);
        let mut lines = blocking("dumpTokens", move || {
            let analysis = &served.analysis;
            let mut rows = dump::collect_rows_for(analysis, &display_root, &candidates);
            rows.retain_mut(|row| row.relocate(|file, name, start| places.name(file, name, start)));
            places.log("llg/dumpTokens", rows.len());
            let mut lines =
                dump::format_rows(&format!("root={}", display_root.display()), &rows, analysis);
            // The CLI-only report header stays out of the wire response: the
            // contract is the formatted rows plus the trailing summary line.
            let _ = lines.remove(0);
            lines
        })
        .await
        .unwrap_or_else(|| vec!["# error: dump task failed".to_owned()]);
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
                    let served = root.served()?;
                    let places = Places::new(&self.state, &served);
                    Some((
                        key.to_string_lossy().into_owned(),
                        root.shadow.clone(),
                        served.analysis,
                        places,
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
                |(workspace_index, (root_id, shadow, analysis, mut places))| {
                    budget.begin_workspace(workspace_count - workspace_index);
                    let mut snapshot = module_explorer::snapshot_analysis_with_budget(
                        &root_id,
                        &analysis,
                        |path| shadow.real_path(path).or_else(|| Some(path.to_path_buf())),
                        &mut budget,
                    );
                    module_explorer::visit_places(&mut snapshot, |uri, range| {
                        places.relocate(uri, range);
                    });
                    places.log("llg/moduleExplorer", snapshot.modules.len());
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
                Some(_) if state.documents.is_rejected(&params.uri) => None,
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
                    Ok(Ok(read)) => read.text,
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
