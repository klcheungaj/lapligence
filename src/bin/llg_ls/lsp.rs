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

struct RootState {
    descriptor: RootDescriptor,
    shadow: ShadowPaths,
    /// Discovered `.v`/`.sv` compilation units (normalized absolute paths).
    discovered: Vec<PathBuf>,
    /// Exact resolved include dependencies (any extension), used to extend the
    /// watched-file set after a successful analysis.
    include_deps: BTreeSet<PathBuf>,
    lint_config: LintConfig,
    /// Parse/validation errors from the most recent config load (empty when
    /// the config is valid or missing).  Used to publish a diagnostic against
    /// the TOML URI even when a last-valid config is retained.
    config_errors: Vec<ConfigError>,
    /// Non-fatal config-load warnings (e.g. configured source/include
    /// directories that do not exist).  Published once per load as WARNING
    /// diagnostics against the TOML URI; discovery rescans stay silent about
    /// them so they never repeat on every scan.
    config_warnings: Vec<String>,
    last_good: Option<Arc<Analysis>>,
    /// Identity of the `last_good` snapshot for the request memoization keys
    /// (`request_cache`).  Stamped with a fresh process-global value exactly
    /// where a commit replaces or clears `last_good`, so any input change that
    /// flows through an analysis commit invalidates memoized results
    /// structurally; reads happen together with `last_good` under the state
    /// lock.
    analysis_epoch: u64,
    /// Diagnostics published for this root's documents (open or closed).
    diagnostics: BTreeMap<Url, Vec<Diagnostic>>,
    /// Digest of the last primary payload sent per URI by this root, so
    /// unchanged diagnostics are not re-sent after every commit.  Cleared for
    /// URIs this root no longer publishes.
    published_digests: BTreeMap<Url, String>,
    /// Diagnostics from the latest commit keyed by REAL file path, including
    /// closed files.  Feeds the shared-file aggregation (which must publish
    /// unions for tracked-but-closed external files too).
    all_diagnostics: BTreeMap<PathBuf, Vec<Diagnostic>>,
    generation: u64,
    /// Parent correlation for the next run.  A coalesced trigger replaces
    /// this only when it carries a real request/notification ID; startup and
    /// internal reschedules intentionally leave it absent.
    pending_parent_id: Option<u64>,
    /// Debounce + latest-wins coalescing for this root's analysis runs (see
    /// [`SchedulerState`]); mutated only under the backend state lock.
    scheduler: SchedulerState,
}
struct BackendState {
    roots: BTreeMap<RootKey, RootState>,
    documents: BTreeMap<Url, SharedText>,
    dynamic_watched_files: bool,
    watchers_registered: bool,
    /// Digest of the watcher options sent with the last successful
    /// registration, so repeated valid commits coalesce into one registration
    /// when nothing relevant changed.
    registered_watchers_digest: Option<String>,
    initialized: bool,
    shutting_down: bool,
    pending_logs: Vec<String>,
    merged: Option<Arc<Analysis>>,
    initial_pending: BTreeSet<RootKey>,
    ready_sent: bool,
    /// Reverse include-dependency index: resolved dep path → dependent roots.
    /// Rebuilt during commits and workspace-folder changes; watched-file
    /// events for arbitrary-extension deps schedule every dependent root.
    dep_dependents: BTreeMap<PathBuf, BTreeSet<RootKey>>,
    /// Digest of the last aggregated shared-file publication per URI, so
    /// unchanged unions are not re-sent after every commit.
    published_shared: BTreeMap<Url, String>,
    /// Monotonic job-generation counter; bumped only while creating a job
    /// under the state lock (which is exactly when staleness is decided).
    next_generation: u64,
}
struct RootJob {
    key: RootKey,
    generation: u64,
    /// Correlation ID of the request/notification that armed or refreshed
    /// this coalesced run.  `None` is expected for startup/internal work;
    /// coalesced triggers retain only the latest meaningful parent.
    parent_id: Option<u64>,
    shadow: ShadowPaths,
    files: Vec<PathBuf>,
    /// Include dependencies from the last committed result.  A size-limit
    /// preflight must retain these watchers while the last-good snapshot is
    /// still being served.
    previous_include_deps: BTreeSet<PathBuf>,
    open_documents: OpenDocuments,
    lint_config: LintConfig,
    config: LlgConfig,
}
struct CompileResult {
    analysis: Option<Analysis>,
    files: Vec<(PathBuf, String)>,
    include_deps: BTreeSet<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum InputSizeLimitKind {
    PerFile,
    Total,
    Unreadable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InputSizeLimit {
    path: PathBuf,
    measured_bytes: u64,
    configured_limit: u64,
    kind: InputSizeLimitKind,
    total_bytes: Option<u64>,
}

impl InputSizeLimit {
    fn message(&self) -> String {
        match self.kind {
            InputSizeLimitKind::PerFile => format!(
                "input-size-limit: {} measured {} bytes, exceeding max_file_bytes={} (per-file budget)",
                self.path.display(),
                self.measured_bytes,
                self.configured_limit
            ),
            InputSizeLimitKind::Total => format!(
                "input-size-limit: {} measured {} bytes; total unique input size is {} bytes, exceeding max_total_input_bytes={} (total budget)",
                self.path.display(),
                self.measured_bytes,
                self.total_bytes.unwrap_or(self.measured_bytes),
                self.configured_limit
            ),
            InputSizeLimitKind::Unreadable => format!(
                "input-snapshot: {} could not be read as a bounded UTF-8 snapshot after observing {} bytes; compile rejected to preserve max_file_bytes={}",
                self.path.display(),
                self.measured_bytes,
                self.configured_limit
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InputBudgetFailure {
    limit: InputSizeLimit,
    /// Resolved dependencies seen before (and including) the offending input.
    /// Keeping these paths lets a failed job preserve the last-good watch set.
    include_deps: BTreeSet<PathBuf>,
}

/// Exact text snapshots captured during input-budget admission.  The lexical
/// path and canonical identity are both retained so a file can be staged from
/// the admitted bytes even if its on-disk spelling or symlink changes before
/// staging begins.
#[derive(Debug, Default)]
struct InputSnapshots {
    by_path: BTreeMap<PathBuf, Arc<String>>,
    by_identity: BTreeMap<PathBuf, Arc<String>>,
}

impl InputSnapshots {
    fn insert(&mut self, path: &Path, text: SharedText) {
        self.by_path.insert(path.to_path_buf(), Arc::clone(&text));
        self.by_identity.insert(input_identity(path), text);
    }

    fn text(&self, path: &Path) -> Option<&str> {
        self.by_path
            .get(path)
            .map(|text| text.as_str())
            .or_else(|| {
                self.by_identity
                    .get(&input_identity(path))
                    .map(|text| text.as_str())
            })
    }
}

#[derive(Debug, Default)]
struct InputBudget {
    snapshots: InputSnapshots,
}

fn open_input_size_limit(path: &Path, text: &str, max_file_bytes: u64) -> Option<InputSizeLimit> {
    let measured_bytes = text.len() as u64;
    (measured_bytes > max_file_bytes).then(|| InputSizeLimit {
        path: path.to_path_buf(),
        measured_bytes,
        configured_limit: max_file_bytes,
        kind: InputSizeLimitKind::PerFile,
        total_bytes: None,
    })
}

/// Apply the open-buffer admission check used by the isolated semantic-token
/// handler.  Keeping the tuple/options plumbing here makes the helper test
/// exercise the same decision that must happen before cache-key construction
/// and single-flight admission.
fn open_token_size_limit(
    open_document: Option<&(PathBuf, SharedText, Vec<String>)>,
    max_file_bytes: Option<u64>,
) -> Option<InputSizeLimit> {
    let (Some((path, text, _)), Some(max_file_bytes)) = (open_document, max_file_bytes) else {
        return None;
    };
    open_input_size_limit(path, text.as_str(), max_file_bytes)
}

/// Bound the number of open-document parses that can be retained in the
/// single-flight table.  Entries live while the detached coordinator owns the
/// parse and through cache publication, then are removed on success/failure;
/// absent keys are refused and degraded to the fallback while the table is
/// saturated.
const OPEN_TOKEN_FLIGHT_CAPACITY: usize = 32;
type OpenTokenResult = std::result::Result<SemanticTokens, String>;
const STALE_OPEN_TOKEN_ERROR: &str = "semantic-token request is no longer current";

/// Check a captured open-buffer revision without retaining the backend state
/// lock across any staging or frontend work.  The request can only use the
/// isolated path while the same text is still the document's current
/// snapshot; shutdown also makes an otherwise matching revision ineligible.
fn open_document_is_current(
    state: &Arc<Mutex<BackendState>>,
    uri: &Url,
    captured_text: &str,
) -> bool {
    if shutdown_requested() {
        return false;
    }
    let state = state.lock().unwrap_or_else(|error| error.into_inner());
    !state.shutting_down
        && !shutdown_requested()
        && state
            .documents
            .get(uri)
            .is_some_and(|current_text| current_text.as_str() == captured_text)
}

struct OpenTokenFlight {
    notify: Notify,
    result: Mutex<Option<OpenTokenResult>>,
}

impl OpenTokenFlight {
    fn new() -> Self {
        Self {
            notify: Notify::new(),
            result: Mutex::new(None),
        }
    }

    fn result(&self) -> Option<OpenTokenResult> {
        self.result
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    async fn wait(&self) -> OpenTokenResult {
        loop {
            // Register before checking the result.  This ordering prevents a
            // leader's notify from landing between the check and await.
            let notified = self.notify.notified();
            if let Some(result) = self.result() {
                return result;
            }
            notified.await;
        }
    }
}

struct OpenTokenFlightRegistry {
    flights: Mutex<HashMap<String, Arc<OpenTokenFlight>>>,
}

enum OpenTokenFlightLease {
    Leader(OpenTokenFlightLeader),
    Follower(Arc<OpenTokenFlight>),
    /// The bounded registry is saturated.  This request is refused a flight
    /// and must use the already-available fallback instead of starting
    /// another blocking parse.
    Saturated,
}

struct OpenTokenFlightLeader {
    registry: Arc<OpenTokenFlightRegistry>,
    key: String,
    flight: Arc<OpenTokenFlight>,
    completed: bool,
    detached: bool,
}

/// Completion owner for a detached open-document parse.  It is deliberately
/// independent of the request future: cancelling the request only drops its
/// waiter, while this owner remains in the blocking task until the result has
/// been published and all followers have been woken.
struct OpenTokenFlightCoordinator {
    registry: Arc<OpenTokenFlightRegistry>,
    key: String,
    flight: Arc<OpenTokenFlight>,
    completed: bool,
}

impl OpenTokenFlightRegistry {
    fn new() -> Self {
        Self {
            flights: Mutex::new(HashMap::new()),
        }
    }

    fn acquire(self: &Arc<Self>, key: String) -> OpenTokenFlightLease {
        let mut flights = self
            .flights
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(flight) = flights.get(&key) {
            return OpenTokenFlightLease::Follower(Arc::clone(flight));
        }
        if flights.len() >= OPEN_TOKEN_FLIGHT_CAPACITY {
            return OpenTokenFlightLease::Saturated;
        }
        let flight = Arc::new(OpenTokenFlight::new());
        flights.insert(key.clone(), Arc::clone(&flight));
        OpenTokenFlightLease::Leader(OpenTokenFlightLeader {
            registry: Arc::clone(self),
            key,
            flight,
            completed: false,
            detached: false,
        })
    }

    fn remove(&self, key: &str, flight: &Arc<OpenTokenFlight>) {
        let mut flights = self
            .flights
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if flights
            .get(key)
            .is_some_and(|current| Arc::ptr_eq(current, flight))
        {
            flights.remove(key);
        }
    }

    fn len(&self) -> usize {
        self.flights
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .len()
    }
}

impl OpenTokenFlightLeader {
    fn finish(&mut self, result: OpenTokenResult) {
        OpenTokenFlightCoordinator {
            registry: Arc::clone(&self.registry),
            key: self.key.clone(),
            flight: Arc::clone(&self.flight),
            completed: false,
        }
        .finish(result);
        self.completed = true;
    }

    /// Transfer completion ownership to a detached task before the request
    /// reaches its first await.  The leader's Drop implementation therefore
    /// cannot remove the flight while the parse is still running.
    fn detach(&mut self) -> OpenTokenFlightCoordinator {
        self.detached = true;
        OpenTokenFlightCoordinator {
            registry: Arc::clone(&self.registry),
            key: self.key.clone(),
            flight: Arc::clone(&self.flight),
            completed: false,
        }
    }
}

impl Drop for OpenTokenFlightLeader {
    fn drop(&mut self) {
        if self.completed || self.detached {
            return;
        }
        // This only covers cancellation before detachment (for example, a
        // panic between acquisition and task submission).  Once detached,
        // the coordinator owns completion and the request may be cancelled
        // without exposing a second parser for the same key.
        self.finish(Err("semantic-token leader cancelled".to_owned()));
    }
}

impl OpenTokenFlightCoordinator {
    fn finish(&mut self, result: OpenTokenResult) {
        {
            let mut stored = self
                .flight
                .result
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            *stored = Some(result);
        }
        // The caller must publish any successful cache result before invoking
        // this method.  Keeping removal here makes the registry entry cover
        // the entire compute→publish interval.
        self.registry.remove(&self.key, &self.flight);
        self.flight.notify.notify_waiters();
        self.completed = true;
    }
}

impl Drop for OpenTokenFlightCoordinator {
    fn drop(&mut self) {
        if !self.completed {
            self.finish(Err("semantic-token coordinator cancelled".to_owned()));
        }
    }
}

/// Result of committing a compile result into the backend state.
#[derive(Default)]
struct CommitOutcome {
    publications: Vec<(Url, Vec<Diagnostic>)>,
    ready: bool,
    /// Whether the committed analysis carries servable feature data (see
    /// [`Analysis::has_feature_data`]).  Watcher re-registration triggers
    /// once per such commit so newly resolved include deps get watched —
    /// including commits from roots that never reach strict validity.
    valid_commit: bool,
    /// Whether this commit has a resolved include set that should be offered
    /// to dynamic watchers, even when the analysis itself is Fatal (for
    /// example, a size-limit rejection retaining the last-good snapshot).
    watchers_refresh: bool,
    /// Whether the committed feature model changed the module explorer
    /// snapshot.  Diagnostics-only/fatal commits retain the existing model
    /// and do not cause clients to refetch it.
    module_explorer_changed: bool,
}
struct RescanResult {
    discovered: BTreeMap<RootKey, BTreeSet<PathBuf>>,
    warnings: Vec<String>,
}

/// One client-supplied config-file override: a workspace root and the path to
/// its effective `llg.toml`.
#[derive(Debug, Clone)]
struct ClientConfigFile {
    workspace_uri: Url,
    path: PathBuf,
}

/// The parsed client-to-server initialization payload:
///
/// ```json
/// { "llg": { "protocolVersion": 1, "configFiles": [
///     { "workspaceUri": "file:///project-a", "path": "/project-a/llg.toml" }
/// ] } }
/// ```
///
/// The client passes paths, never TOML contents.
#[derive(Debug, Clone, Default)]
struct ClientInit {
    protocol_version: u32,
    config_files: Vec<ClientConfigFile>,
}

pub struct Backend {
    client: Client,
    state: Arc<Mutex<BackendState>>,
    /// Request memoization for read-only queries.  Keys carry the analysis
    /// epoch, so entries never outlive the snapshot they were computed from;
    /// access bypasses the serialized lifecycle queue exactly like the
    /// underlying handlers do.
    definition_cache: MemoCache<RequestKey, Option<Location>>,
    hover_cache: MemoCache<RequestKey, Option<Hover>>,
    references_cache: MemoCache<RequestKey, Vec<Location>>,
    /// Open-buffer isolated token streams keyed on (uri, buffer text,
    /// effective `-D` defines) — the exact inputs of the request-local
    /// parse-only run.
    open_token_cache: Arc<MemoCache<String, SemanticTokens>>,
    /// In-flight keyed single-flight coordination for open-buffer semantic
    /// token misses.  This is separate from the result cache so cache changes
    /// owned by another module do not affect the bounded wait lifecycle.
    open_token_flights: Arc<OpenTokenFlightRegistry>,
    /// Inactive-range results keyed on (uri, text digest, effective
    /// `[compile] defines`) — the exact inputs of the pure lexical scan.
    inactive_cache: MemoCache<String, Vec<crate::inactive_ranges::LineRange>>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            state: Arc::new(Mutex::new(BackendState {
                roots: BTreeMap::new(),
                documents: BTreeMap::new(),
                dynamic_watched_files: false,
                watchers_registered: false,
                registered_watchers_digest: None,
                initialized: false,
                shutting_down: false,
                pending_logs: Vec::new(),
                merged: None,
                initial_pending: BTreeSet::new(),
                ready_sent: false,
                dep_dependents: BTreeMap::new(),
                published_shared: BTreeMap::new(),
                next_generation: 0,
            })),
            definition_cache: MemoCache::new(NAVIGATION_CACHE_CAPACITY),
            hover_cache: MemoCache::new(NAVIGATION_CACHE_CAPACITY),
            references_cache: MemoCache::new(NAVIGATION_CACHE_CAPACITY),
            open_token_cache: Arc::new(MemoCache::new(TOKEN_CACHE_CAPACITY)),
            open_token_flights: Arc::new(OpenTokenFlightRegistry::new()),
            inactive_cache: MemoCache::new(NAVIGATION_CACHE_CAPACITY),
        }
    }

    /// Aggregate hit/miss counters across all memoization stores; surfaced via
    /// `llg/dumpTokens` so tests and users can observe warm short-circuits.
    fn cache_stats(&self) -> (CacheStats, usize) {
        let mut total = CacheStats::default();
        for stats in [
            self.definition_cache.stats(),
            self.hover_cache.stats(),
            self.references_cache.stats(),
        ] {
            total.add(&stats);
        }
        total.add(&self.open_token_cache.stats());
        total.add(&self.inactive_cache.stats());
        let entries = self.definition_cache.len()
            + self.hover_cache.len()
            + self.references_cache.len()
            + self.open_token_cache.len()
            + self.inactive_cache.len();
        (total, entries)
    }

    /// Shared-file hover annotation applied AFTER the memoized value is
    /// fetched, so the cached payload stays owner-independent.
    fn annotate_hover(&self, mut hover: Option<Hover>, owner_name: Option<&str>) -> Option<Hover> {
        if let (Some(hover), Some(owner_name)) = (hover.as_mut(), owner_name) {
            annotate_shared_hover(hover, owner_name);
        }
        hover
    }

    /// Shadow→real URI mapping for a batch of memoized or fresh locations.
    fn map_locations(&self, locations: &[Location]) -> Vec<Location> {
        let state = self.lock_state();
        locations
            .iter()
            .map(|value| Self::map_location(&state, value.clone()))
            .collect()
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, BackendState> {
        self.state.lock().unwrap_or_else(|error| error.into_inner())
    }

    fn uri_to_path(uri: &Url) -> Option<PathBuf> {
        if uri.scheme() != "file" {
            return None;
        }
        uri.to_file_path()
            .ok()
            .and_then(|path| workspace::normalize_absolute_path(&path))
    }

    fn path_to_uri(path: &Path) -> Option<Url> {
        Url::from_file_path(path).ok()
    }

    /// Capture only a request's document identity for logs.  Source text and
    /// request payloads never enter lifecycle records.
    fn log_uri_identity(uri: &Url) -> String {
        crate::logging::bounded_field(uri.as_str())
    }

    fn roots_from_initialize(params: &InitializeParams) -> Vec<(PathBuf, String)> {
        let mut roots = BTreeMap::new();
        if let Some(folders) = &params.workspace_folders {
            for folder in folders {
                if let Some(path) = Self::uri_to_path(&folder.uri) {
                    roots.entry(path).or_insert_with(|| folder.name.clone());
                }
            }
        }
        if roots.is_empty() {
            if let Some(uri) = &params.root_uri {
                if let Some(path) = Self::uri_to_path(uri) {
                    roots.insert(path, uri.to_string());
                }
            }
        }
        if roots.is_empty() {
            if let Ok(path) = std::env::current_dir() {
                if let Some(path) = workspace::normalize_absolute_path(&path) {
                    roots.insert(path, "cwd".to_owned());
                }
            }
        }
        roots.into_iter().collect()
    }

    /// Parse the client initialization options payload.
    fn parse_client_init(settings: &LSPAny) -> (ClientInit, Vec<String>) {
        let mut init = ClientInit::default();
        let mut warnings = Vec::new();
        let Some(object) = settings.as_object() else {
            return (init, warnings);
        };
        let Some(llg) = object.get("llg").and_then(LSPAny::as_object) else {
            return (init, warnings);
        };
        match llg.get("protocolVersion") {
            Some(version) => match version.as_i64() {
                Some(value) => {
                    init.protocol_version = value.max(0) as u32;
                    if init.protocol_version != CLIENT_PROTOCOL_VERSION {
                        warnings.push(format!(
                            "unsupported llg protocolVersion {} (expected {CLIENT_PROTOCOL_VERSION})",
                            init.protocol_version
                        ));
                    }
                }
                None => warnings.push(format!(
                    "invalid llg.protocolVersion (expected {CLIENT_PROTOCOL_VERSION})"
                )),
            },
            None => warnings.push(format!(
                "missing llg.protocolVersion (expected {CLIENT_PROTOCOL_VERSION})"
            )),
        }
        if let Some(files) = llg.get("configFiles").and_then(LSPAny::as_array) {
            for file in files {
                let Some(file_object) = file.as_object() else {
                    warnings.push("llg.configFiles entries must be objects".to_owned());
                    continue;
                };
                let uri = file_object
                    .get("workspaceUri")
                    .and_then(LSPAny::as_str)
                    .and_then(|value| Url::parse(value).ok());
                let path = file_object
                    .get("path")
                    .and_then(LSPAny::as_str)
                    .map(PathBuf::from);
                match (uri, path) {
                    (Some(workspace_uri), Some(path)) => {
                        init.config_files.push(ClientConfigFile {
                            workspace_uri,
                            path,
                        });
                    }
                    _ => warnings.push(
                        "llg.configFiles entries need workspaceUri and path strings".to_owned(),
                    ),
                }
            }
        }
        (init, warnings)
    }

    /// Config-path override for a workspace root, if the client supplied one.
    fn config_override_for<'a>(init: &'a ClientInit, root: &Path) -> Option<&'a ClientConfigFile> {
        init.config_files.iter().find(|file| {
            Self::uri_to_path(&file.workspace_uri)
                .as_deref()
                .is_some_and(|path| path == root)
        })
    }

    /// Load a root's `llg.toml` (at the effective path) and return the
    /// last-valid config plus any parse errors and non-fatal warnings.  A
    /// missing config is a normal result with `config: None` and no errors.
    fn load_root_config(
        _root: &Path,
        config_path: &Path,
    ) -> (Option<Arc<LlgConfig>>, Vec<ConfigError>, Vec<ConfigError>) {
        match config::load_config_file(config_path) {
            Ok(load) => (load.config.map(Arc::new), load.errors, load.warnings),
            Err(error) => (
                None,
                vec![ConfigError::new(format!(
                    "failed to read {}: {error}",
                    config_path.display()
                ))],
                Vec::new(),
            ),
        }
    }

    /// Build a root state, loading its config from the effective path.
    fn root_state(
        path: PathBuf,
        id: String,
        config_path: PathBuf,
    ) -> (RootState, Vec<ConfigError>) {
        let (config, errors, warnings) = Self::load_root_config(&path, &config_path);
        let descriptor = RootDescriptor::from_absolute(&path)
            .expect("normalized workspace root")
            .with_id(id)
            .with_config_path(config_path)
            .with_config(config.clone());
        let lint_config = config
            .as_deref()
            .map(|cfg| cfg.lint.clone())
            .unwrap_or_default();
        (
            RootState {
                descriptor,
                shadow: ShadowPaths::new(),
                discovered: Vec::new(),
                include_deps: BTreeSet::new(),
                lint_config,
                config_errors: errors.clone(),
                config_warnings: warnings
                    .iter()
                    .map(|warning| warning.message.clone())
                    .collect(),
                last_good: None,
                diagnostics: BTreeMap::new(),
                published_digests: BTreeMap::new(),
                all_diagnostics: BTreeMap::new(),
                generation: 0,
                pending_parent_id: None,
                scheduler: SchedulerState::default(),
                analysis_epoch: 0,
            },
            errors,
        )
    }

    /// Publish config-parse diagnostics for every root to its TOML URI.
    ///
    /// Parse errors surface as ERROR; non-fatal load warnings (missing
    /// configured directories) surface as WARNING.  The full list is
    /// republished per URI, so warnings never accumulate across loads.
    async fn publish_config_diagnostics(&self) {
        let started = std::time::Instant::now();
        let root_count = self.lock_state().roots.len();
        crate::llg_debug!(
            "event=publish_config_diagnostics.begin roots={}",
            root_count
        );
        let publications: Vec<(Url, Vec<Diagnostic>)> = {
            let state = self.lock_state();
            state
                .roots
                .values()
                .filter_map(|root| {
                    let uri = Self::path_to_uri(&root.descriptor.config_path)?;
                    let diagnostics = if !root.config_errors.is_empty() {
                        vec![Diagnostic {
                            range: Range {
                                start: Position::new(0, 0),
                                end: Position::new(0, 1),
                            },
                            severity: Some(DiagnosticSeverity::ERROR),
                            code: Some(NumberOrString::String("llg-config".to_owned())),
                            code_description: None,
                            source: Some("llg".to_owned()),
                            message: format!(
                                "invalid {}: see server log",
                                root.descriptor.config_path.display()
                            ),
                            related_information: None,
                            tags: None,
                            data: None,
                        }]
                    } else {
                        root.config_warnings
                            .iter()
                            .map(|warning| Diagnostic {
                                range: Range {
                                    start: Position::new(0, 0),
                                    end: Position::new(0, 1),
                                },
                                severity: Some(DiagnosticSeverity::WARNING),
                                code: Some(NumberOrString::String("llg-config".to_owned())),
                                code_description: None,
                                source: Some("llg".to_owned()),
                                message: warning.clone(),
                                related_information: None,
                                tags: None,
                                data: None,
                            })
                            .collect()
                    };
                    Some((uri, diagnostics))
                })
                .collect()
        };
        for (uri, diagnostics) in publications {
            let diagnostic_count = diagnostics.len();
            self.client
                .publish_diagnostics(uri.clone(), diagnostics, None)
                .await;
            crate::llg_trace!(
                "event=publish_config_diagnostics.item.end outcome=ok uri={} diagnostics={}",
                uri,
                diagnostic_count
            );
        }
        crate::llg_debug!(
            "event=publish_config_diagnostics.end outcome=ok roots={} elapsed_us={}",
            root_count,
            started.elapsed().as_micros()
        );
    }

    async fn rescan(&self) -> BTreeSet<RootKey> {
        let started = std::time::Instant::now();
        let (descriptors, previous) = {
            let state = self.lock_state();
            (
                state
                    .roots
                    .iter()
                    .map(|(key, root)| (key.clone(), root.descriptor.clone()))
                    .collect::<BTreeMap<_, _>>(),
                state
                    .roots
                    .iter()
                    .map(|(key, root)| (key.clone(), root.discovered.clone()))
                    .collect::<BTreeMap<_, _>>(),
            )
        };
        crate::llg_debug!(
            "event=workspace.discovery.begin roots={} previous_file_sets={}",
            descriptors.len(),
            previous.len()
        );
        let discovery_descriptors = descriptors.clone();
        let discovery = tokio::task::spawn_blocking(move || {
            Self::discover_snapshot(discovery_descriptors.values().cloned().collect())
        })
        .await;
        let Ok(RescanResult {
            discovered: mut groups,
            warnings,
        }) = discovery
        else {
            crate::llg_debug!(
                "event=workspace.discovery.end outcome=error roots={} elapsed_us={}",
                descriptors.len(),
                started.elapsed().as_micros()
            );
            self.lock_state()
                .pending_logs
                .push("workspace discovery task failed".to_owned());
            return BTreeSet::new();
        };

        let mut state = self.lock_state();
        let current_descriptors: BTreeMap<RootKey, RootDescriptor> = state
            .roots
            .iter()
            .map(|(key, root)| (key.clone(), root.descriptor.clone()))
            .collect();
        if current_descriptors != descriptors {
            crate::llg_debug!(
                "event=workspace.discovery.end outcome=stale roots={} elapsed_us={}",
                descriptors.len(),
                started.elapsed().as_micros()
            );
            return BTreeSet::new();
        }
        let warning_count = warnings.len();
        state.pending_logs.extend(warnings);
        let mut changed = BTreeSet::new();
        for (key, root) in &mut state.roots {
            root.discovered = groups.remove(key).unwrap_or_default().into_iter().collect();
            if previous.get(key) != Some(&root.discovered) {
                changed.insert(key.clone());
            }
        }
        let discovered_files: usize = state.roots.values().map(|root| root.discovered.len()).sum();
        crate::llg_debug!(
            "event=workspace.discovery.end outcome=ok roots={} discovered_files={} changed_roots={} warnings={} elapsed_us={}",
            state.roots.len(),
            discovered_files,
            changed.len(),
            warning_count,
            started.elapsed().as_micros()
        );
        changed
    }

    fn discover_snapshot(descriptors: Vec<RootDescriptor>) -> RescanResult {
        let started = std::time::Instant::now();
        let mut groups: BTreeMap<RootKey, BTreeSet<PathBuf>> = descriptors
            .iter()
            .map(|descriptor| (descriptor.root.clone(), BTreeSet::new()))
            .collect();
        let mut warnings = Vec::new();
        for descriptor in &descriptors {
            crate::llg_debug!(
                "event=workspace.discovery.root.begin root={} source_dirs={}",
                descriptor.root.display(),
                descriptor.effective_config().sources.directories.len()
            );
            match workspace::discover_units(descriptor) {
                Ok(files) => {
                    crate::llg_debug!(
                        "event=workspace.discovery.root.end outcome=ok root={} files={}",
                        descriptor.root.display(),
                        files.len()
                    );
                    groups
                        .entry(descriptor.root.clone())
                        .or_default()
                        .extend(files);
                }
                // A configured source/include directory that does not exist
                // yet is reported ONCE as a config-load warning against the
                // TOML URI; repeating it on every scan would be noise.
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                    ) => {}
                Err(error) => {
                    crate::llg_debug!(
                        "event=workspace.discovery.root.end outcome=error root={} error={}",
                        descriptor.root.display(),
                        error
                    );
                    warnings.push(format!(
                        "workspace discovery failed for {}: {error}",
                        descriptor.root.display()
                    ));
                }
            }
        }
        crate::llg_debug!(
            "event=workspace.discovery.task.end outcome=ok roots={} discovered_files={} warnings={} elapsed_us={}",
            descriptors.len(),
            groups.values().map(BTreeSet::len).sum::<usize>(),
            warnings.len(),
            started.elapsed().as_micros()
        );
        RescanResult {
            discovered: groups,
            warnings,
        }
    }

    // ── jobs ────────────────────────────────────────────────────────────────

    /// Relative path of `path` within its deepest configured source
    /// directory.
    ///
    /// Discovery evaluated include/exclude globs against SOURCE-DIR-relative
    /// paths (`discover_files(dir, …)`), while ownership computes paths
    /// relative to the deepest INCLUDE dir; re-checking an open buffer's
    /// filters must use the same base as discovery or restrictive globs
    /// (`src/**`) wrongly reject files under nested include dirs.
    fn source_relative_path(config: &LlgConfig, path: &Path) -> Option<PathBuf> {
        let mut best: Option<(usize, PathBuf)> = None;
        for dir in &config.sources.directories {
            let Ok(stripped) = path.strip_prefix(dir) else {
                continue;
            };
            let Some(relative) = workspace::normalize_relative_path(stripped) else {
                continue;
            };
            let depth = dir.components().count();
            if best
                .as_ref()
                .is_none_or(|(best_depth, _)| depth > *best_depth)
            {
                best = Some((depth, relative));
            }
        }
        best.map(|(_, relative)| relative)
    }

    /// Snapshot the latest inputs of each named root into exactly one job per
    /// root, assigning fresh generations.  Static over the shared state so
    /// spawned debounce tasks (which cannot borrow `&Backend`) can call it.
    fn make_jobs(state: &Arc<Mutex<BackendState>>, keys: Vec<RootKey>) -> Vec<RootJob> {
        let mut state = state.lock().unwrap_or_else(|error| error.into_inner());
        Self::purge_oversized_documents(&mut state);
        let documents = state.documents.clone();
        let descriptors: Vec<_> = state
            .roots
            .values()
            .map(|root| root.descriptor.clone())
            .collect();
        let mut jobs = Vec::new();
        let mut seen = BTreeSet::new();
        for key in keys {
            if !seen.insert(key.clone()) {
                continue;
            }
            state.next_generation += 1;
            let generation = state.next_generation;
            let Some(root) = state.roots.get_mut(&key) else {
                continue;
            };
            // Longest-root ownership is authoritative: a discovered file
            // whose structural owner is a different (e.g. nested) root must
            // not be compiled by this root, even when discovery filters would
            // include it.
            let mut files: BTreeSet<PathBuf> = root
                .discovered
                .iter()
                .filter(|file| {
                    workspace::owning_root_unfiltered(file, &descriptors)
                        .is_some_and(|owner| owner.root == root.descriptor.root)
                })
                .cloned()
                .collect();
            let config = root.descriptor.effective_config();
            let mut open_documents = BTreeMap::new();
            for (uri, text) in &documents {
                let Some(path) = Self::uri_to_path(uri) else {
                    continue;
                };
                let Some(owner) = workspace::owning_root_unfiltered(&path, &descriptors) else {
                    continue;
                };
                if owner.root != root.descriptor.root {
                    continue;
                }
                open_documents.insert(path.clone(), Arc::clone(text));
                if config::is_compilation_unit(&path) {
                    // Same relative base as discovery time (source-dir-
                    // relative), falling back to the ownership-relative path
                    // for open units outside every source directory.
                    let relative = Self::source_relative_path(&config, &path)
                        .unwrap_or_else(|| owner.relative_path.clone());
                    if root.descriptor.allows_file(&relative) {
                        files.insert(path.clone());
                    }
                }
            }
            root.generation = generation;
            let parent_id = root.pending_parent_id.take();
            let files: Vec<PathBuf> = files.into_iter().collect();
            crate::llg_debug!(
                "event=root_job.snapshot root={} generation={} files={} open_documents={} previous_include_deps={} defines={} param_overrides={} include_dirs={} parent_id={:?}",
                root.descriptor.root.display(),
                generation,
                files.len(),
                open_documents.len(),
                root.include_deps.len(),
                config.compile.defines.len(),
                config.compile.param_overrides.len(),
                config.compile.include_dirs.len(),
                parent_id
            );
            jobs.push(RootJob {
                key,
                generation,
                parent_id,
                shadow: root.shadow.clone(),
                files,
                previous_include_deps: root.include_deps.clone(),
                open_documents,
                lint_config: root.lint_config.clone(),
                config,
            });
        }
        jobs
    }

    fn schedule_all(&self, parent_id: Option<u64>) {
        let keys = self.lock_state().roots.keys().cloned().collect();
        self.schedule_roots_with_parent(keys, parent_id);
    }

    /// Request a re-analysis of the given roots.
    ///
    /// Every root coalesces its own triggers (see [`SchedulerState`]): the
    /// first trigger arms ONE trailing-edge debounce timer, further triggers
    /// during the quiet period are absorbed by it, and triggers landing while
    /// a job runs only mark the root dirty so completion owes exactly one
    /// follow-up run built from the latest state.
    /// Request a re-analysis while retaining the originating lifecycle ID
    /// through the scheduler.  The scheduler still coalesces triggers; when
    /// several meaningful notifications arrive, only the latest parent is
    /// attached to the eventual run.  Internal/startup calls intentionally
    /// pass `None` and are logged as having no originating request.
    fn schedule_roots_with_parent(&self, keys: Vec<RootKey>, parent_id: Option<u64>) {
        let mut armed = Vec::new();
        let requested = keys.len();
        let mut coalesced = 0usize;
        {
            let mut state = self.lock_state();
            for key in keys {
                if let Some(root) = state.roots.get_mut(&key) {
                    if let Some(parent_id) = parent_id {
                        root.pending_parent_id = Some(parent_id);
                    }
                    match root.scheduler.trigger() {
                        TriggerDecision::ArmTimer => armed.push(key),
                        TriggerDecision::Coalesce => coalesced += 1,
                    }
                }
            }
        }
        crate::llg_debug!(
            "event=scheduler.trigger requested={} armed={} coalesced={} parent_id={:?}",
            requested,
            armed.len(),
            coalesced,
            parent_id
        );
        for key in armed {
            spawn_debounced_run(self.client.clone(), Arc::clone(&self.state), key);
        }
    }

    /// Release a root's running slot after its claimed run finished.  Returns
    /// `true` when triggers arrived during the run and one debounced
    /// follow-up must be armed now.  A vanished or shutting-down root never
    /// owes a follow-up.
    fn finish_root_run(state: &Arc<Mutex<BackendState>>, key: &RootKey) -> bool {
        let mut guard = state.lock().unwrap_or_else(|error| error.into_inner());
        if guard.shutting_down {
            return false;
        }
        guard
            .roots
            .get_mut(key)
            .is_some_and(|root| root.scheduler.job_finished())
    }

    fn job_current(state: &Arc<Mutex<BackendState>>, job: &RootJob) -> bool {
        let state = state.lock().unwrap_or_else(|error| error.into_inner());
        state
            .roots
            .get(&job.key)
            .is_some_and(|root| !state.shutting_down && root.generation == job.generation)
    }

    fn compile_job(state: &Arc<Mutex<BackendState>>, job: RootJob) -> Option<CompileResult> {
        let root_identity = job.key.to_string_lossy().into_owned();
        let job_started = std::time::Instant::now();
        let mut job_span = crate::logging::LifecycleSpan::analysis_with_parent(
            "root-job",
            || root_identity.clone(),
            job.generation,
            job.files.len(),
            job.parent_id,
        );
        crate::llg_debug!(
            "event=root_job.frontend.begin root={} generation={} parent_id={:?} files={} open_documents={} previous_include_deps={}",
            root_identity,
            job.generation,
            job.parent_id,
            job.files.len(),
            job.open_documents.len(),
            job.previous_include_deps.len()
        );
        let mut staging_span = Some(crate::logging::LifecycleSpan::phase_with_parent(
            "analysis.shadow_staging_preflight",
            || root_identity.clone(),
            job.generation,
            job.files.len(),
            Some(job_span.id()),
        ));
        if !Self::job_current(state, &job) {
            staging_span.as_mut().unwrap().outcome("stale");
            crate::llg_debug!(
                "event=root_job.frontend.end outcome=stale root={} generation={} elapsed_us={}",
                root_identity,
                job.generation,
                job_started.elapsed().as_micros()
            );
            return None;
        }
        let _shadow_guard = shadow_staging_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if !Self::job_current(state, &job) {
            staging_span.as_mut().unwrap().outcome("stale");
            crate::llg_debug!(
                "event=root_job.frontend.end outcome=stale-after-lock root={} generation={} elapsed_us={}",
                root_identity,
                job.generation,
                job_started.elapsed().as_micros()
            );
            return None;
        }
        let budget_started = std::time::Instant::now();
        let mut budget_span = crate::logging::LifecycleSpan::phase_with_parent(
            "analysis.input_budget",
            || root_identity.clone(),
            job.generation,
            job.files.len(),
            Some(job_span.id()),
        );
        crate::llg_debug!(
            "event=analysis.input_budget.begin root={} generation={} files={} open_documents={} max_file_bytes={} max_total_input_bytes={}",
            root_identity,
            job.generation,
            job.files.len(),
            job.open_documents.len(),
            job.config.analysis.max_file_bytes,
            job.config.analysis.max_total_input_bytes
        );
        let input_snapshots = match enforce_input_budget(
            &job.config,
            &job.files,
            &job.open_documents,
        ) {
            Ok(budget) => {
                let snapshot_files = budget.snapshots.by_path.len();
                let snapshot_identities = budget.snapshots.by_identity.len();
                budget_span.complete("admitted", snapshot_files);
                crate::llg_debug!(
                    "event=analysis.input_budget.end outcome=admitted root={} generation={} snapshots={} identities={} elapsed_us={}",
                    root_identity,
                    job.generation,
                    snapshot_files,
                    snapshot_identities,
                    budget_started.elapsed().as_micros()
                );
                budget.snapshots
            }
            Err(failure) => {
                let message = failure.limit.message();
                let diagnostic_file = failure.limit.path.to_string_lossy().into_owned();
                let measured_bytes = failure.limit.measured_bytes;
                let total_bytes = failure.limit.total_bytes;
                let failure_kind = match &failure.limit.kind {
                    InputSizeLimitKind::PerFile => "per-file",
                    InputSizeLimitKind::Total => "total",
                    InputSizeLimitKind::Unreadable => "unreadable",
                };
                let include_deps_seen = failure.include_deps.len();
                crate::llg_debug!(
                    "event=analysis.input_budget.end outcome=rejected root={} generation={} kind={} path={} measured_bytes={} total_bytes={:?} include_deps={} elapsed_us={}",
                    root_identity,
                    job.generation,
                    failure_kind,
                    diagnostic_file,
                    measured_bytes,
                    total_bytes,
                    include_deps_seen,
                    budget_started.elapsed().as_micros()
                );
                let mut analysis = Analysis::fatal_preflight(message);
                if let Some(diagnostic) = analysis.diagnostics.first_mut() {
                    diagnostic.file = Some(diagnostic_file);
                }
                let mut include_deps = job.previous_include_deps;
                include_deps.extend(failure.include_deps);
                budget_span.complete("rejected", analysis.diagnostics.len());
                staging_span.as_mut().unwrap().outcome("error");
                job_span.complete("error", analysis.diagnostics.len());
                crate::llg_debug!(
                    "event=root_job.frontend.end outcome=input-budget-rejected root={} generation={} diagnostics={} include_deps={} elapsed_us={}",
                    root_identity,
                    job.generation,
                    analysis.diagnostics.len(),
                    include_deps.len(),
                    job_started.elapsed().as_micros()
                );
                return Some(CompileResult {
                    analysis: Some(analysis),
                    files: compile_result_files(&job.files),
                    include_deps,
                });
            }
        };
        job.shadow.cleanup();
        // Reset the shared analysis scratch area: jobs are serialized behind
        // the shadow-staging lock, so the previous job's Surelog artifacts
        // (`slpp_all/`, logs, caches) can be removed wholesale here.  The
        // analysis itself parks the process CWD inside this directory (see
        // `features::analysis_scratch_dir`) so Surelog's side-effects never
        // land in the process CWD or any project/external tree.
        clean_analysis_scratch();
        crate::llg_debug!(
            "event=analysis.shadow_cleanup.end outcome=ok root={} generation={} elapsed_us={}",
            root_identity,
            job.generation,
            job_started.elapsed().as_micros()
        );
        let stage_started = std::time::Instant::now();
        let mut stage_span = crate::logging::LifecycleSpan::phase_with_parent(
            "analysis.shadow_stage_inputs",
            || root_identity.clone(),
            job.generation,
            job.files.len(),
            Some(job_span.id()),
        );
        crate::llg_debug!(
            "event=analysis.shadow_stage_inputs.begin root={} generation={} source_files={}",
            root_identity,
            job.generation,
            job.files.len()
        );
        let mut files = Vec::new();
        let mut staged_files = 0usize;
        let mut real_files = 0usize;
        let mut skipped_files = 0usize;
        for real in &job.files {
            let real = real.clone();
            let compiled = match prepared_input_text(&real, &job.open_documents, &input_snapshots) {
                Some(text) => {
                    let staged = match job.shadow.stage(&real, text) {
                        Ok(staged) => staged,
                        Err(error) => {
                            let message = format!(
                                "input-staging: failed to stage {} from its admitted bounded snapshot; compile rejected to preserve the input budget: {error}",
                                real.display()
                            );
                            crate::llg_debug!(
                                "event=analysis.shadow_stage_inputs.end outcome=error root={} generation={} path={} error={} elapsed_us={}",
                                root_identity,
                                job.generation,
                                real.display(),
                                error,
                                stage_started.elapsed().as_micros()
                            );
                            let mut analysis = Analysis::fatal_preflight(message);
                            if let Some(diagnostic) = analysis.diagnostics.first_mut() {
                                diagnostic.file = Some(real.to_string_lossy().into_owned());
                            }
                            stage_span.complete("error", analysis.diagnostics.len());
                            staging_span.as_mut().unwrap().outcome("error");
                            job_span.complete("error", analysis.diagnostics.len());
                            job.shadow.cleanup();
                            crate::llg_debug!(
                                "event=root_job.frontend.end outcome=input-staging-rejected root={} generation={} diagnostics={} elapsed_us={}",
                                root_identity,
                                job.generation,
                                analysis.diagnostics.len(),
                                job_started.elapsed().as_micros()
                            );
                            return Some(CompileResult {
                                analysis: Some(analysis),
                                files: compile_result_files(&job.files),
                                include_deps: job.previous_include_deps,
                            });
                        }
                    };
                    if staged != real {
                        staged_files += 1;
                    } else {
                        real_files += 1;
                    }
                    staged
                }
                None => {
                    let message = format!(
                        "input-snapshot: root compilation unit {} has no admitted bounded snapshot; compile rejected to preserve the input budget",
                        real.display()
                    );
                    crate::llg_debug!(
                        "event=analysis.shadow_stage_inputs.file outcome=rejected root={} generation={} path={} reason=input-snapshot",
                        root_identity,
                        job.generation,
                        real.display()
                    );
                    let mut analysis = Analysis::fatal_preflight(message);
                    if let Some(diagnostic) = analysis.diagnostics.first_mut() {
                        diagnostic.file = Some(real.to_string_lossy().into_owned());
                    }
                    stage_span.complete("error", analysis.diagnostics.len());
                    staging_span.as_mut().unwrap().outcome("error");
                    job_span.complete("error", analysis.diagnostics.len());
                    job.shadow.cleanup();
                    crate::llg_debug!(
                        "event=root_job.frontend.end outcome=input-snapshot-rejected root={} generation={} diagnostics={} elapsed_us={}",
                        root_identity,
                        job.generation,
                        analysis.diagnostics.len(),
                        job_started.elapsed().as_micros()
                    );
                    return Some(CompileResult {
                        analysis: Some(analysis),
                        files: compile_result_files(&job.files),
                        include_deps: job.previous_include_deps,
                    });
                }
            };
            let Some(compiled) = compiled.to_str().map(str::to_owned) else {
                skipped_files += 1;
                continue;
            };
            files.push((real, compiled));
        }
        files.sort_by(|left, right| left.0.cmp(&right.0));
        files.dedup_by(|left, right| left.0 == right.0);
        stage_span.complete("ok", files.len());
        crate::llg_debug!(
            "event=analysis.shadow_stage_inputs.end outcome=ok root={} generation={} compiled_files={} staged_files={} real_files={} skipped_files={} elapsed_us={}",
            root_identity,
            job.generation,
            files.len(),
            staged_files,
            real_files,
            skipped_files,
            stage_started.elapsed().as_micros()
        );
        drop(stage_span);
        if !Self::job_current(state, &job) {
            staging_span.as_mut().unwrap().outcome("stale");
            crate::llg_debug!(
                "event=root_job.frontend.end outcome=stale-after-input-stage root={} generation={} elapsed_us={}",
                root_identity,
                job.generation,
                job_started.elapsed().as_micros()
            );
            return None;
        }
        let mut include_deps = BTreeSet::new();
        let include_preflight_started = std::time::Instant::now();
        let mut include_preflight_span = crate::logging::LifecycleSpan::phase_with_parent(
            "analysis.include_resolution_preflight",
            || root_identity.clone(),
            job.generation,
            files.len(),
            Some(job_span.id()),
        );
        crate::llg_debug!(
            "event=analysis.include_resolution_preflight.begin root={} generation={} files={} snapshots={}",
            root_identity,
            job.generation,
            files.len(),
            input_snapshots.by_path.len()
        );
        let include_preflight = if files.is_empty() {
            include_preflight_span.complete("empty", 0);
            crate::llg_debug!(
                "event=analysis.include_resolution_preflight.end outcome=empty root={} generation={} resolved=0 elapsed_us={}",
                root_identity,
                job.generation,
                include_preflight_started.elapsed().as_micros()
            );
            None
        } else {
            let result = preflight_include_isolation(
                &job.config,
                &files,
                &job.open_documents,
                &input_snapshots,
            );
            let outcome = if result.is_some() { "rejected" } else { "ok" };
            include_preflight_span.complete(outcome, usize::from(result.is_none()));
            crate::llg_debug!(
                "event=analysis.include_resolution_preflight.end outcome={} root={} generation={} resolved={} elapsed_us={}",
                outcome,
                root_identity,
                job.generation,
                usize::from(result.is_none()),
                include_preflight_started.elapsed().as_micros()
            );
            result
        };
        drop(include_preflight_span);
        let analysis = if files.is_empty() {
            None
        } else if let Some((file, message)) = include_preflight {
            // A preflight rejection happens before the dependency-stage pass
            // can produce a new complete set.  Keep the prior watched set
            // while the last-good analysis remains servable; this also keeps
            // an unreadable discovered include observable for a later fix.
            include_deps = job.previous_include_deps.clone();
            staging_span.as_mut().unwrap().outcome("error");
            let mut analysis = Analysis::fatal_preflight(message);
            if let Some(diagnostic) = analysis.diagnostics.first_mut() {
                // `fatal_preflight` deliberately has no source file.  Attach
                // this root-local preflight failure to the same real/shadow
                // path used by the compile result so it can be published while
                // the retained last-good snapshot remains available.
                diagnostic.file = Some(file);
            }
            Some(analysis)
        } else {
            if !Self::job_current(state, &job) {
                staging_span.as_mut().unwrap().outcome("stale");
                crate::llg_debug!(
                    "event=root_job.frontend.end outcome=stale-before-include-stage root={} generation={} elapsed_us={}",
                    root_identity,
                    job.generation,
                    job_started.elapsed().as_micros()
                );
                return None;
            }
            let include_stage_started = std::time::Instant::now();
            let mut include_stage_span = crate::logging::LifecycleSpan::phase_with_parent(
                "analysis.include_stage",
                || root_identity.clone(),
                job.generation,
                files.len(),
                Some(job_span.id()),
            );
            crate::llg_debug!(
                "event=analysis.include_stage.begin root={} generation={} files={}",
                root_identity,
                job.generation,
                files.len()
            );
            let staged_deps = match stage_include_tree(
                &job.config,
                &files,
                &job.open_documents,
                &input_snapshots,
                &job.shadow,
            ) {
                Ok(deps) => deps,
                Err(failure) => {
                    include_stage_span.complete("error", failure.dependencies.len());
                    crate::llg_debug!(
                        "event=analysis.include_stage.end outcome=error root={} generation={} path={} include_deps={} elapsed_us={} error_kind=input-staging",
                        root_identity,
                        job.generation,
                        failure.path.display(),
                        failure.dependencies.len(),
                        include_stage_started.elapsed().as_micros()
                    );
                    let mut analysis = Analysis::fatal_preflight(failure.message);
                    if let Some(diagnostic) = analysis.diagnostics.first_mut() {
                        diagnostic.file = Some(failure.path.to_string_lossy().into_owned());
                    }
                    let mut include_deps = job.previous_include_deps;
                    include_deps.extend(failure.dependencies);
                    staging_span.as_mut().unwrap().outcome("error");
                    job_span.complete("error", analysis.diagnostics.len());
                    job.shadow.cleanup();
                    crate::llg_debug!(
                        "event=root_job.frontend.end outcome=input-include-staging-rejected root={} generation={} diagnostics={} include_deps={} elapsed_us={}",
                        root_identity,
                        job.generation,
                        analysis.diagnostics.len(),
                        include_deps.len(),
                        job_started.elapsed().as_micros()
                    );
                    return Some(CompileResult {
                        analysis: Some(analysis),
                        files: compile_result_files(&job.files),
                        include_deps,
                    });
                }
            };
            include_deps = staged_deps;
            include_stage_span.complete("ok", include_deps.len());
            crate::llg_debug!(
                "event=analysis.include_stage.end outcome=ok root={} generation={} include_deps={} elapsed_us={}",
                root_identity,
                job.generation,
                include_deps.len(),
                include_stage_started.elapsed().as_micros()
            );
            drop(include_stage_span);
            // The LSP compile path is isolated to admitted shadow inputs.
            // Literal includes were staged above; omitting live `-I` paths
            // also makes macro-generated/dynamic includes fail closed instead
            // of allowing Surelog to read an unmeasured project file.
            let opts = config::compile_opts_isolated(
                &job.config,
                files.iter().map(|(_, path)| path.clone()).collect(),
                job.shadow.base(),
            );
            if !Self::job_current(state, &job) {
                staging_span.as_mut().unwrap().outcome("stale");
                crate::llg_debug!(
                    "event=root_job.frontend.end outcome=stale-before-analysis root={} generation={} elapsed_us={}",
                    root_identity,
                    job.generation,
                    job_started.elapsed().as_micros()
                );
                return None;
            }
            if let Some(mut span) = staging_span.take() {
                span.complete("ok", files.len() + include_deps.len());
            }
            let analysis = features::analyze_with_config_context_parent(
                &opts,
                &job.lint_config,
                &root_identity,
                job.generation,
                Some(job_span.id()),
            );
            crate::llg_debug!(
                "event=root_job.analysis.end outcome={:?} root={} generation={} diagnostics={} lint={} token_files={} declarations={} references={} elapsed_us={}",
                analysis.outcome,
                root_identity,
                job.generation,
                analysis.diagnostics.len(),
                analysis.lint.len(),
                analysis.tokens.len(),
                analysis.index.decls.len(),
                analysis.index.refs.len(),
                job_started.elapsed().as_micros()
            );
            Some(analysis)
        };
        if !matches!(
            analysis.as_ref().map(|a| a.outcome),
            Some(features::AnalysisOutcome::Fatal)
        ) {
            if let Some(span) = staging_span.as_mut() {
                span.complete("ok", files.len() + include_deps.len());
            }
        }
        let job_outcome = match analysis.as_ref().map(|analysis| analysis.outcome) {
            None => "empty",
            Some(features::AnalysisOutcome::Valid) => "ok",
            Some(
                features::AnalysisOutcome::Fatal
                | features::AnalysisOutcome::Parse
                | features::AnalysisOutcome::Compile,
            ) => "error",
        };
        let diagnostic_count = analysis
            .as_ref()
            .map_or(0, |analysis| analysis.diagnostics.len());
        let lint_count = analysis.as_ref().map_or(0, |analysis| analysis.lint.len());
        job_span.complete(job_outcome, diagnostic_count + lint_count);
        crate::llg_debug!(
            "event=root_job.frontend.end outcome={} root={} generation={} diagnostics={} lint={} files={} include_deps={} elapsed_us={}",
            job_outcome,
            root_identity,
            job.generation,
            diagnostic_count,
            lint_count,
            files.len(),
            include_deps.len(),
            job_started.elapsed().as_micros()
        );
        Some(CompileResult {
            analysis,
            files,
            include_deps,
        })
    }

    fn commit_job(
        state: &Arc<Mutex<BackendState>>,
        key: &RootKey,
        generation: u64,
        result: CompileResult,
    ) -> CommitOutcome {
        let mut state = state.lock().unwrap_or_else(|error| error.into_inner());
        if state.shutting_down {
            return CommitOutcome::default();
        }
        // Open-document set used ONLY to pick where fileless diagnostics
        // attach: an open compilation unit is preferred so unsaved buffers
        // surface them; otherwise the first compiled file receives them.
        let currently_open: BTreeSet<PathBuf> = state
            .documents
            .keys()
            .filter_map(Self::uri_to_path)
            .collect();
        // Files tracked by OTHER roots; this root's own contribution is added
        // right after its include-dependency refresh below.  A path tracked
        // by more than one root belongs to the shared-file aggregation slice,
        // which publishes its union (labeled on conflict) and owns its
        // clearing — the per-root publication map here must stay out of its
        // way or the union would be swallowed by same-commit publications.
        let mut tracker_counts: BTreeMap<PathBuf, usize> = BTreeMap::new();
        for (other_key, other) in state.roots.iter() {
            if other_key == key {
                continue;
            }
            for file in other.discovered.iter().chain(&other.include_deps) {
                *tracker_counts.entry(file.clone()).or_default() += 1;
            }
        }
        let Some(root) = state.roots.get_mut(key) else {
            return CommitOutcome::default();
        };
        if root.generation != generation {
            crate::llg_debug!(
                "job commit stale root={} generation={} current={}",
                key.display(),
                generation,
                root.generation
            );
            return CommitOutcome::default();
        }
        let usable = result
            .analysis
            .as_ref()
            .is_some_and(Analysis::has_feature_data);
        let module_explorer_changed = match (root.last_good.as_ref(), result.analysis.as_ref()) {
            (Some(previous), Some(next)) if usable => {
                previous.model != next.model
                    || previous.module_graph != next.module_graph
                    || previous.configured_top != next.configured_top
            }
            (None, Some(_)) if usable => true,
            (Some(_), None) => true,
            _ => false,
        };
        let mut result = result;
        let watchers_refresh = usable || !result.include_deps.is_empty();
        root.include_deps = result.include_deps.clone();
        for file in root.discovered.iter().chain(&root.include_deps) {
            *tracker_counts.entry(file.clone()).or_default() += 1;
        }
        let shared_files: BTreeSet<PathBuf> = tracker_counts
            .into_iter()
            .filter(|(_, count)| *count > 1)
            .map(|(file, _)| file)
            .collect();
        let diagnostic_file = result
            .files
            .iter()
            .find(|(real, _)| currently_open.contains(real))
            .or_else(|| result.files.first())
            .map(|(_, compiled)| compiled.clone());
        if let (Some(analysis), Some(diagnostic_file)) = (result.analysis.as_mut(), diagnostic_file)
        {
            attach_fileless_diagnostics(analysis, &diagnostic_file);
        }
        let diagnostics_by_path = result
            .analysis
            .as_ref()
            .map(features::lsp_diagnostics)
            .unwrap_or_default();
        // Full per-real-path diagnostics map (open or closed): feeds both
        // the project-wide publication map and the shared-file aggregation.
        let mut all: BTreeMap<PathBuf, Vec<Diagnostic>> = BTreeMap::new();
        for (path, diagnostics) in diagnostics_by_path {
            let path = PathBuf::from(path);
            let real = root.shadow.real_path(&path).unwrap_or(path);
            all.insert(real, diagnostics);
        }
        // Project-wide publication map: every compilation unit is published,
        // open or closed (clients render Problems-panel entries for closed
        // files too).  Units without findings get an empty list so stale
        // client-side errors are cleared; the union with the previous key set
        // below clears URIs that left the analysis entirely.  Multi-root
        // tracked (shared) paths are excluded: the aggregation slice owns
        // them end to end.
        let mut current = BTreeMap::new();
        for (real, _) in &result.files {
            if shared_files.contains(real) {
                continue;
            }
            if let Some(uri) = Self::path_to_uri(real) {
                current.insert(uri, all.get(real).cloned().unwrap_or_default());
            }
        }
        // Analyzed-but-not-compiled files (e.g. include headers) publish when
        // the analysis produced findings for them.
        for (real, diagnostics) in &all {
            if shared_files.contains(real) {
                continue;
            }
            if let Some(uri) = Self::path_to_uri(real) {
                current.entry(uri).or_insert_with(|| diagnostics.clone());
            }
        }
        root.all_diagnostics = all;
        // Feature-serving gate: an analysis replaces the retained snapshot
        // when it carries servable navigation data — best-effort for
        // Parse/Compile outcomes (Surelog still elaborated the surviving
        // files), never for Fatal ones.  An analysis without feature data
        // neither replaces nor clears the snapshot: before the first
        // servable commit the root keeps serving None.  Watcher
        // re-registration follows the SAME predicate so roots that are not
        // strictly valid still register their resolved include dependencies.
        if result.analysis.is_none() {
            root.last_good = None;
            // The served snapshot changed (it disappeared): memoized results
            // keyed on the previous epoch become unreachable.
            root.analysis_epoch = next_analysis_epoch();
        } else if usable {
            root.last_good = result.analysis.map(|mut analysis| {
                // Undefined-macro hovers name the config file that produced
                // the checked `[compile] defines`; attach it at commit time
                // so the memoized payloads carry their own provenance.
                analysis.attach_macro_config_note(
                    root.descriptor
                        .config_path
                        .file_name()
                        .unwrap_or_else(|| root.descriptor.config_path.as_os_str())
                        .to_string_lossy(),
                );
                Arc::new(analysis)
            });
            root.analysis_epoch = next_analysis_epoch();
        }
        let mut keys: BTreeSet<Url> = root.diagnostics.keys().cloned().collect();
        keys.extend(current.keys().cloned());
        // A URI that just became multi-root tracked is dropped silently here:
        // the aggregation slice publishes its union in this very commit, so a
        // primary clearing payload would only race it.
        keys.retain(|uri| Self::uri_to_path(uri).is_none_or(|path| !shared_files.contains(&path)));
        root.diagnostics = current;
        // Republish only URIs whose payload changed since this root's last
        // commit; disappeared URIs clear through the union above (empty
        // payload).
        let mut publications: Vec<(Url, Vec<Diagnostic>)> = Vec::new();
        for uri in keys {
            let diagnostics = root.diagnostics.get(&uri).cloned().unwrap_or_default();
            let digest = diagnostics_digest(&diagnostics);
            if root.published_digests.get(&uri).map(String::as_str) == Some(digest.as_str()) {
                continue;
            }
            root.published_digests.insert(uri.clone(), digest);
            publications.push((uri, diagnostics));
        }
        let live: BTreeSet<Url> = root.diagnostics.keys().cloned().collect();
        root.published_digests.retain(|uri, _| live.contains(uri));
        let ready = state.initial_pending.remove(key)
            && state.initial_pending.is_empty()
            && !state.ready_sent;
        if ready {
            state.ready_sent = true;
        }
        crate::llg_debug!(
            "initial root commit root={} pending={} ready={}",
            key.display(),
            state.initial_pending.len(),
            ready
        );
        Self::rebuild_merged(&mut state);
        rebuild_dep_dependents(&mut state);
        let published_uris: BTreeSet<Url> =
            publications.iter().map(|(uri, _)| uri.clone()).collect();
        let shared_publications = aggregate_shared_publications(&mut state, &published_uris);
        CommitOutcome {
            publications: publications
                .into_iter()
                .chain(shared_publications)
                .collect(),
            ready,
            valid_commit: usable,
            watchers_refresh,
            module_explorer_changed,
        }
    }

    fn rebuild_merged(state: &mut BackendState) {
        let indexes: Vec<_> = state
            .roots
            .values()
            .filter_map(|root| root.last_good.as_ref().map(|analysis| &analysis.index))
            .collect();
        if indexes.is_empty() {
            state.merged = None;
            crate::llg_debug!("merged index cleared roots={}", state.roots.len());
        } else {
            let mut merged = features::empty_analysis();
            merged.index = SymbolIndex::merge(indexes);
            crate::llg_debug!(
                "merged index rebuilt roots={} declarations={}",
                state.roots.len(),
                merged.index.decls.len()
            );
            state.merged = Some(Arc::new(merged));
        }
    }

    fn source_root(&self, path: &Path) -> Option<RootKey> {
        let state = self.lock_state();
        let roots: Vec<_> = state
            .roots
            .values()
            .map(|root| root.descriptor.clone())
            .collect();
        workspace::owning_root_unfiltered(path, &roots).map(|owner| owner.root)
    }

    /// Return the positive per-file limit applicable to an open document.
    /// Before initialization (or for a URI that is not owned by a root), use
    /// the conservative built-in limit so admission never fails open.
    fn document_max_file_bytes(state: &BackendState, uri: &Url) -> u64 {
        let Some(path) = Self::uri_to_path(uri) else {
            return config::DEFAULT_MAX_FILE_BYTES;
        };
        let descriptors: Vec<_> = state
            .roots
            .values()
            .map(|root| root.descriptor.clone())
            .collect();
        workspace::owning_root_unfiltered(&path, &descriptors)
            .and_then(|owner| state.roots.get(&owner.root))
            .map(|root| root.descriptor.effective_config().analysis.max_file_bytes)
            .filter(|limit| *limit > 0)
            .unwrap_or(config::DEFAULT_MAX_FILE_BYTES)
    }

    /// Admit one complete client buffer without copying its contents.  The
    /// size check runs before the text is wrapped in an `Arc` or stored.  A
    /// rejected change leaves the previous admitted buffer in place, so a
    /// later job can never observe the rejected text.
    fn admit_document_text(
        state: &mut BackendState,
        uri: Url,
        text: String,
        replace_unchanged: bool,
    ) -> std::result::Result<bool, InputSizeLimit> {
        let max_file_bytes = Self::document_max_file_bytes(state, &uri);
        let path = Self::uri_to_path(&uri).unwrap_or_else(|| PathBuf::from(uri.as_str()));
        if let Some(limit) = open_input_size_limit(&path, &text, max_file_bytes) {
            return Err(limit);
        }

        let changed = did_change_is_new_content(
            state.documents.get(&uri).map(|current| current.as_str()),
            &text,
        );
        if changed || replace_unchanged {
            state.documents.insert(uri, Arc::new(text));
        }
        Ok(changed)
    }

    /// Drop buffers admitted under an older, larger bound before constructing
    /// any job snapshot.  This covers config initialization/reload lowering a
    /// limit after a buffer was already open, without copying its text.
    fn purge_oversized_documents(state: &mut BackendState) {
        let rejected: Vec<_> = state
            .documents
            .iter()
            .filter_map(|(uri, text)| {
                let max_file_bytes = Self::document_max_file_bytes(state, uri);
                (text.len() as u64 > max_file_bytes)
                    .then(|| (uri.clone(), text.len() as u64, max_file_bytes))
            })
            .collect();
        for (uri, measured_bytes, max_file_bytes) in rejected {
            if state.documents.remove(&uri).is_some() {
                crate::llg_debug!(
                    "event=document.admission outcome=rejected reason=too-large uri={} bytes={} max_file_bytes={}",
                    crate::logging::bounded_field(uri.as_str()),
                    measured_bytes,
                    max_file_bytes
                );
            }
        }
    }

    fn config_root(&self, path: &Path) -> Option<RootKey> {
        let path = workspace::normalize_absolute_path(path)?;
        let state = self.lock_state();
        state
            .roots
            .values()
            .filter(|root| {
                workspace::normalize_absolute_path(&root.descriptor.config_path).as_deref()
                    == Some(path.as_path())
            })
            .max_by_key(|root| root.descriptor.config_path.components().count())
            .map(|root| root.descriptor.root.clone())
    }

    /// Whether `path` equals any root's effective config path.
    ///
    /// Roots may point at arbitrary config files via initialization overrides
    /// (`custom.toml`, …), so events whose basename is not `llg.toml` must
    /// still reload that root when the normalized path matches.
    fn is_effective_config_path(&self, path: &Path) -> bool {
        let roots: Vec<_> = self
            .lock_state()
            .roots
            .values()
            .map(|root| root.descriptor.clone())
            .collect();
        workspace::is_effective_config_path(path, &roots)
    }

    fn mark_ready_if_empty(&self) -> bool {
        let mut state = self.lock_state();
        if !state.ready_sent && state.initial_pending.is_empty() {
            state.ready_sent = true;
            true
        } else {
            false
        }
    }

    fn root_context<'a>(
        state: &'a BackendState,
        uri: &Url,
    ) -> Option<(&'a RootState, PathBuf, Vec<String>)> {
        let real = Self::uri_to_path(uri)?;
        let roots: Vec<_> = state
            .roots
            .values()
            .map(|root| root.descriptor.clone())
            .collect();
        let owner = workspace::owner_for_path(&real, &roots)?;
        let root = state.roots.get(&owner.root)?;
        let mut paths = Vec::new();
        if let Some(path) = root
            .shadow
            .shadow_path(&real)
            .and_then(|path| path.to_str().map(str::to_owned))
        {
            paths.push(path);
        }
        if let Some(path) = real.to_str() {
            if !paths.iter().any(|candidate| candidate == path) {
                paths.push(path.to_owned());
            }
        }
        Some((root, real, paths))
    }

    fn map_location(state: &BackendState, mut location: Location) -> Location {
        let Ok(path) = location.uri.to_file_path() else {
            return location;
        };
        for root in state.roots.values() {
            if let Some(real) = root.shadow.real_path(&path) {
                if let Ok(uri) = Url::from_file_path(real) {
                    location.uri = uri;
                }
                break;
            }
        }
        location
    }

    /// Remap the URIs of a plain `changes`-shaped edit from the shadow tree
    /// back to the real files (same mapping as [`Backend::map_location`]).
    fn map_workspace_edit_uris(state: &BackendState, mut edit: WorkspaceEdit) -> WorkspaceEdit {
        if let Some(changes) = edit.changes.take() {
            edit.changes = Some(
                changes
                    .into_iter()
                    .map(|(uri, edits)| {
                        let mapped = Self::map_location(
                            state,
                            Location {
                                uri,
                                range: Range::default(),
                            },
                        )
                        .uri;
                        (mapped, edits)
                    })
                    .collect(),
            );
        }
        edit
    }

    async fn flush_logs(&self) {
        let logs = {
            let mut state = self.lock_state();
            std::mem::take(&mut state.pending_logs)
        };
        for log in logs {
            self.client.log_message(MessageType::WARNING, log).await;
        }
    }

    /// Every root whose resolved include-dependency set tracks `path`
    /// (arbitrary extension).  Uses the dependency reverse index rebuilt at
    /// commit time, plus a direct membership scan so a just-committed dep is
    /// never missed.
    fn dependent_roots_for_dep(&self, path: &Path) -> BTreeSet<RootKey> {
        let Some(path) = workspace::normalize_absolute_path(path) else {
            return BTreeSet::new();
        };
        let state = self.lock_state();
        let mut roots: BTreeSet<RootKey> =
            state.dep_dependents.get(&path).cloned().unwrap_or_default();
        for (key, root) in &state.roots {
            if root.include_deps.contains(&path) {
                roots.insert(key.clone());
            }
        }
        roots
    }

    // ── watched files ───────────────────────────────────────────────────────

    /// (Re)register the dynamic watched-file watchers with the client.
    ///
    /// Reuses the same registration id; when the watcher set is unchanged
    /// since the last successful registration the update is coalesced away so
    /// repeated valid commits do not spam registrations.
    async fn register_watchers(&self) {
        register_watchers_with(&self.client, &self.state).await;
    }

    // ── config reload ───────────────────────────────────────────────────────

    /// Reload a root's `llg.toml`, retaining the last-valid config when the
    /// new file is malformed.  Returns `true` when the effective config
    /// (or its path) changed so callers can rescan/schedule.
    fn reload_root_config(&self, key: &RootKey, config_path: &Path) -> bool {
        let (config, errors, warnings) = Self::load_root_config(key, config_path);
        let mut state = self.lock_state();
        let changed =
            reload_root_config_in_state(&mut state, key, config_path, config, &errors, &warnings);
        changed
    }

    /// Custom request `llg/dumpTokens`: serve the same formatted token-dump
    /// rows as the CLI's `--dump-tokens`, filtered to one document, plus the
    /// trailing `# analysis:` summary line.
    ///
    /// Failures (unknown document, no servable analysis yet) answer Ok with a
    /// single `# error: <reason>` line so clients always have something to
    /// print instead of surfacing a JSON-RPC error.
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
        let snapshot = tokio::task::spawn_blocking(move || {
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
        .await
        .unwrap_or_else(|error| {
            crate::llg_debug!("module explorer snapshot task failed: {error}");
            module_explorer::ExplorerSnapshot {
                modules: Vec::new(),
                roots: Vec::new(),
            }
        });
        crate::llg_debug!(
            "event=module_explorer.snapshot.end outcome=ok modules={} roots={} truncated_modules={}",
            snapshot.modules.len(),
            snapshot.roots.len(),
            snapshot
                .modules
                .iter()
                .filter(|module| module.is_budget_truncated)
                .count()
        );
        request.complete("ok", snapshot.modules.len() + snapshot.roots.len());
        Ok(snapshot)
    }

    /// Custom request `llg/inactiveRanges`: the zero-based inclusive line
    /// ranges a preprocessor would SKIP for one document under the owner
    /// root's effective `[compile] defines`, so editors can dim skipped
    /// conditional-compilation branches.
    ///
    /// Serves COMMITTED state only — the staged open-buffer text (same store
    /// `semantic_tokens/full` reads) or the on-disk source otherwise.  The
    /// computation is a pure lexical scan (`inactive_ranges`): no Surelog
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

/// Free-standing watcher registration used by both the backend methods and
/// spawned commit tasks (which cannot borrow `&Backend`).
///
/// See [`Backend::register_watchers`] for the coalescing contract.
async fn register_watchers_with(client: &Client, state: &Arc<Mutex<BackendState>>) {
    let started = std::time::Instant::now();
    let dynamic = {
        let state = state.lock().unwrap_or_else(|error| error.into_inner());
        state.dynamic_watched_files
    };
    crate::llg_debug!("event=watchers.registration.begin dynamic={}", dynamic);
    if !dynamic {
        crate::llg_debug!(
            "event=watchers.registration.end outcome=disabled elapsed_us={}",
            started.elapsed().as_micros()
        );
        return;
    }
    let options = {
        let state = state.lock().unwrap_or_else(|error| error.into_inner());
        watcher_options_from_state(&state)
    };
    let digest = options.to_string();
    let digest_bytes = digest.len();
    {
        let state = state.lock().unwrap_or_else(|error| error.into_inner());
        if state.watchers_registered
            && state.registered_watchers_digest.as_deref() == Some(digest.as_str())
        {
            crate::llg_debug!(
                "event=watchers.registration.end outcome=unchanged digest_bytes={} elapsed_us={}",
                digest.len(),
                started.elapsed().as_micros()
            );
            return;
        }
    }
    let was_registered = {
        let state = state.lock().unwrap_or_else(|error| error.into_inner());
        state.watchers_registered
    };
    if was_registered {
        let unregistration = Unregistration {
            id: WATCH_REGISTRATION_ID.to_owned(),
            method: "workspace/didChangeWatchedFiles".to_owned(),
        };
        if let Err(error) = client.unregister_capability(vec![unregistration]).await {
            crate::llg_debug!(
                "event=watchers.unregistration.end outcome=error error={}",
                error
            );
            client
                .log_message(
                    MessageType::WARNING,
                    format!("watched-file unregistration failed: {error}"),
                )
                .await;
        } else {
            crate::llg_trace!("event=watchers.unregistration.end outcome=ok");
        }
    }
    {
        let mut state = state.lock().unwrap_or_else(|error| error.into_inner());
        state.watchers_registered = true;
        state.registered_watchers_digest = Some(digest);
    }
    if let Err(error) = client
        .register_capability(vec![Registration {
            id: WATCH_REGISTRATION_ID.to_owned(),
            method: "workspace/didChangeWatchedFiles".to_owned(),
            register_options: Some(options),
        }])
        .await
    {
        {
            let mut state = state.lock().unwrap_or_else(|error| error.into_inner());
            state.registered_watchers_digest = None;
        }
        client
            .log_message(
                MessageType::WARNING,
                format!("watched-file registration failed: {error}"),
            )
            .await;
        crate::llg_debug!(
            "event=watchers.registration.end outcome=error digest_bytes={} elapsed_us={} error={}",
            digest_bytes,
            started.elapsed().as_micros(),
            error
        );
    } else {
        crate::llg_debug!(
            "event=watchers.registration.end outcome=ok digest_bytes={} elapsed_us={}",
            digest_bytes,
            started.elapsed().as_micros()
        );
    }
}

/// Debounced analysis task for ONE armed root (free-standing because spawned
/// commit tasks re-arm follow-ups without a `&Backend`).
///
/// After the trailing-edge quiet period it claims the root's running slot
/// (`Pending→Running`), builds exactly one job from the latest state, runs
/// and commits it; when triggers arrived during the run the root is dirty and
/// this function re-arms itself once.  While the run executes no further job
/// can be created for the root — new triggers only set the dirty flag — so a
/// burst of N events yields at most two runs instead of N racing jobs.
fn spawn_debounced_run(client: Client, state: Arc<Mutex<BackendState>>, key: RootKey) {
    tokio::spawn(async move {
        let timer_started = std::time::Instant::now();
        crate::llg_debug!(
            "event=scheduler.debounce.begin root={} delay_ms={}",
            key.display(),
            RECOMPILE_DEBOUNCE.as_millis()
        );
        // Async timer: unlike the previous per-trigger blocking sleep this
        // occupies no thread while waiting out the quiet period.
        tokio::time::sleep(RECOMPILE_DEBOUNCE).await;
        let fire = {
            let mut guard = state.lock().unwrap_or_else(|error| error.into_inner());
            if guard.shutting_down {
                if let Some(root) = guard.roots.get_mut(&key) {
                    root.scheduler.cancel_pending();
                }
                FireDecision::Ignore
            } else {
                guard
                    .roots
                    .get_mut(&key)
                    .map_or(FireDecision::Ignore, |root| root.scheduler.timer_fired())
            }
        };
        if fire != FireDecision::StartJob {
            crate::llg_debug!(
                "event=scheduler.debounce.end outcome=cancelled root={} decision={:?} elapsed_us={}",
                key.display(),
                fire,
                timer_started.elapsed().as_micros()
            );
            return;
        }
        crate::llg_debug!(
            "event=scheduler.debounce.end outcome=start_job root={} decision={:?} elapsed_us={}",
            key.display(),
            fire,
            timer_started.elapsed().as_micros()
        );
        let mut jobs = Backend::make_jobs(&state, vec![key.clone()]);
        let Some(job) = jobs.pop() else {
            // The root disappeared between the two locks; its scheduler died
            // with the RootState.
            return;
        };
        let job_started = std::time::Instant::now();
        let job_parent_id = job.parent_id;
        let job_file_count = job.files.len();
        let mut end_to_end_span = crate::logging::LifecycleSpan::analysis_with_parent(
            "root-job.end_to_end",
            || key.to_string_lossy().into_owned(),
            job.generation,
            job_file_count,
            job_parent_id,
        );
        crate::llg_debug!(
            "event=root_job.begin root={} generation={} files={} parent_id={:?}",
            job.key.display(),
            job.generation,
            job.files.len(),
            job.parent_id
        );
        let generation = job.generation;
        crate::llg_debug!(
            "event=root_job.compile.begin phase=job_compiling job compiling root={} generation={}",
            key.display(),
            generation
        );
        let compile_state = Arc::clone(&state);
        let result =
            tokio::task::spawn_blocking(move || Backend::compile_job(&compile_state, job)).await;
        let Ok(Some(result)) = result else {
            end_to_end_span.outcome("stale-or-aborted");
            crate::llg_debug!(
                "event=root_job.end outcome=stale-or-aborted root={} generation={} elapsed_us={}",
                key.display(),
                generation,
                job_started.elapsed().as_micros()
            );
            // Aborted mid-flight (stale/shutdown): still release the slot so
            // a dirty marker from this window is honored.
            if Backend::finish_root_run(&state, &key) {
                spawn_debounced_run(client, state, key);
            }
            return;
        };
        crate::llg_debug!(
            "event=root_job.compile.end outcome={} root={} generation={} diagnostics={} lint={} compiled_files={} include_deps={} elapsed_us={}",
            result
                .analysis
                .as_ref()
                .map_or("empty", |analysis| match analysis.outcome {
                    features::AnalysisOutcome::Valid => "ok",
                    features::AnalysisOutcome::Fatal
                    | features::AnalysisOutcome::Parse
                    | features::AnalysisOutcome::Compile => "error",
                }),
            key.display(),
            generation,
            result
                .analysis
                .as_ref()
                .map_or(0, |analysis| analysis.diagnostics.len()),
            result
                .analysis
                .as_ref()
                .map_or(0, |analysis| analysis.lint.len()),
            result.files.len(),
            result.include_deps.len(),
            job_started.elapsed().as_micros()
        );
        let outcome = Backend::commit_job(&state, &key, generation, result);
        let ready = outcome.ready;
        crate::llg_debug!(
            "event=root_job.commit.end outcome=ok root={} generation={} publications={} ready={} valid_commit={} watchers_refresh={} module_explorer_changed={}",
            key.display(),
            generation,
            outcome.publications.len(),
            ready,
            outcome.valid_commit,
            outcome.watchers_refresh,
            outcome.module_explorer_changed
        );
        for (uri, diagnostics) in &outcome.publications {
            let lint_count = diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.source.as_deref() == Some("llg-lint"))
                .count();
            crate::llg_trace!(
                "publishing diagnostics uri={} lint_rules={} total={}",
                uri,
                lint_count,
                diagnostics.len()
            );
            client
                .publish_diagnostics(uri.clone(), diagnostics.clone(), None)
                .await;
            crate::llg_trace!(
                "event=publish_diagnostics.end outcome=ok uri={} lint_rules={} total={}",
                uri,
                lint_count,
                diagnostics.len()
            );
        }
        if outcome.module_explorer_changed {
            client
                .send_notification::<LlgModuleExplorerChanged>(ModuleExplorerChangedParams {})
                .await;
        }
        if outcome.valid_commit || outcome.watchers_refresh {
            // Re-register watchers now that resolved include dependencies are
            // known; unchanged watcher sets are coalesced away.
            register_watchers_with(&client, &state).await;
        }
        if ready {
            client
                .log_message(
                    MessageType::INFO,
                    "llg Verilog/SystemVerilog language server ready",
                )
                .await;
        }
        // Latest-wins coalescing: at most ONE debounced follow-up run when
        // triggers landed while this job executed.
        if Backend::finish_root_run(&state, &key) {
            end_to_end_span.complete("ok-follow-up", outcome.publications.len());
            crate::llg_debug!(
                "event=root_job.end outcome=ok-follow-up root={} generation={} publications={} elapsed_us={}",
                key.display(),
                generation,
                outcome.publications.len(),
                job_started.elapsed().as_micros()
            );
            spawn_debounced_run(client, state, key);
        } else {
            end_to_end_span.complete("ok", outcome.publications.len());
            crate::llg_debug!(
                "event=root_job.end outcome=ok root={} generation={} publications={} elapsed_us={}",
                key.display(),
                generation,
                outcome.publications.len(),
                job_started.elapsed().as_micros()
            );
        }
    });
}

/// Pure state update backing [`Backend::reload_root_config`].
///
/// Retains the last-valid config when the reload is invalid.  Parse errors and
/// non-fatal warnings are appended to `state.pending_logs`.
fn reload_root_config_in_state(
    state: &mut BackendState,
    key: &RootKey,
    config_path: &Path,
    config: Option<Arc<LlgConfig>>,
    errors: &[ConfigError],
    warnings: &[ConfigError],
) -> bool {
    let Some(root) = state.roots.get_mut(key) else {
        return false;
    };
    let previous_config = root.descriptor.config.clone();
    let previous_path = root.descriptor.config_path.clone();
    root.descriptor = root
        .descriptor
        .clone()
        .with_config_path(config_path.to_path_buf());
    // Retain the last-valid config when the reload is invalid.
    if config.is_some() {
        root.descriptor = root.descriptor.clone().with_config(config.clone());
        if let Some(config) = config.as_deref() {
            root.lint_config = config.lint.clone();
        }
    }
    for error in errors {
        state
            .pending_logs
            .push(format!("{}: {}", config_path.display(), error.message));
    }
    for warning in warnings {
        state
            .pending_logs
            .push(format!("{}: {}", config_path.display(), warning.message));
    }
    root.config_errors = errors.to_vec();
    // Warnings are republished wholesale per load; the publish step replaces
    // the previous diagnostic list, so identical reloads do not accumulate.
    root.config_warnings = warnings
        .iter()
        .map(|warning| warning.message.clone())
        .collect();
    previous_path != config_path || previous_config.as_ref().map(Arc::as_ref) != config.as_deref()
}

#[derive(Debug, Clone)]
pub struct ShadowPaths {
    base: PathBuf,
    staged: Arc<Mutex<BTreeSet<PathBuf>>>,
}
impl ShadowPaths {
    pub fn new() -> Self {
        Self {
            base: features::process_shadow_base(),
            staged: Arc::new(Mutex::new(BTreeSet::new())),
        }
    }
    pub fn base(&self) -> &Path {
        &self.base
    }
    pub fn shadow_path(&self, real: &Path) -> Option<PathBuf> {
        real.is_absolute()
            .then(|| features::shadow_path(real, &self.base))
    }
    pub fn real_path(&self, shadow: &Path) -> Option<PathBuf> {
        features::real_path(shadow, &self.base)
    }
    pub fn stage(&self, real: &Path, text: &str) -> std::io::Result<PathBuf> {
        let shadow = self.shadow_path(real).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "absolute shadow path required",
            )
        })?;
        if let Some(parent) = shadow.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&shadow, text)?;
        self.staged
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(real.to_path_buf());
        Ok(shadow)
    }
    pub fn remove(&self, real: &Path) {
        if let Some(path) = self.shadow_path(real) {
            let _ = std::fs::remove_file(path);
            self.staged
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .remove(real);
        }
    }
    pub fn cleanup(&self) {
        let staged = std::mem::take(
            &mut *self
                .staged
                .lock()
                .unwrap_or_else(|error| error.into_inner()),
        );
        for real in staged {
            if let Some(path) = self.shadow_path(&real) {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

static NEXT_SEMANTIC_STAGE_ID: AtomicU64 = AtomicU64::new(0);

/// One request-local copy of an open buffer under the private process shadow
/// base.  The directory is unique so semantic parsing cannot overwrite the
/// project-analysis shadow copy of the same document.
struct SemanticStage {
    directory: PathBuf,
    path: PathBuf,
}

impl SemanticStage {
    fn new(real: &Path, text: &str, defines: &[String]) -> std::io::Result<Self> {
        let id = NEXT_SEMANTIC_STAGE_ID.fetch_add(1, Ordering::Relaxed);
        let directory = features::process_shadow_base()
            .join("semantic")
            .join(format!("{}-{id}", std::process::id()));
        std::fs::create_dir_all(&directory)?;
        let file_name = real
            .file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new("document.sv"));
        let path = directory.join(file_name);
        let parse_text = mask_semantic_preprocessor_directives(text, defines);
        if let Err(error) = std::fs::write(&path, parse_text) {
            let _ = std::fs::remove_dir_all(&directory);
            return Err(error);
        }
        Ok(Self { directory, path })
    }
}

/// Replace standalone compiler-directive lines with spaces while preserving
/// every newline and character column. Surelog's `-parseonly` mode bypasses
/// preprocessing and otherwise diagnoses valid directives such as
/// `` `include`` as parser syntax errors. Semantic-token collection is
/// intentionally source-local, so masking the directives both avoids that
/// false error and guarantees that includes are not consumed.
fn mask_semantic_preprocessor_directives(text: &str, defines: &[String]) -> String {
    const DIRECTIVES: &[&str] = &[
        "celldefine",
        "default_nettype",
        "define",
        "else",
        "elsif",
        "endcelldefine",
        "endif",
        "ifdef",
        "ifndef",
        "include",
        "line",
        "nounconnected_drive",
        "pragma",
        "resetall",
        "timescale",
        "unconnected_drive",
        "undef",
        "undefineall",
    ];

    let inactive_defines = defines
        .iter()
        .map(|define| define.strip_prefix("-D").unwrap_or(define).to_owned())
        .collect::<Vec<_>>();
    let inactive = crate::inactive_ranges::inactive_line_ranges(text, &inactive_defines);
    let mut inactive_index = 0usize;
    let mut output = String::with_capacity(text.len());
    let mut continuation = false;
    let mut in_block_comment = false;
    for (line_index, line) in text.split_inclusive('\n').enumerate() {
        let body = line.strip_suffix('\n').unwrap_or(line);
        while inactive
            .get(inactive_index)
            .is_some_and(|range| range.end_line < line_index as u32)
        {
            inactive_index += 1;
        }
        let line_is_inactive = inactive.get(inactive_index).is_some_and(|range| {
            range.start_line <= line_index as u32 && line_index as u32 <= range.end_line
        });
        let starts_directive =
            semantic_directive_outside_comment(body, &mut in_block_comment, DIRECTIVES);
        let directive = continuation || starts_directive;
        continuation = directive && body.trim_end().ends_with('\\');
        if directive || line_is_inactive {
            output.extend(body.chars().map(|ch| if ch == '\t' { '\t' } else { ' ' }));
        } else {
            output.push_str(body);
        }
        if line.ends_with('\n') {
            output.push('\n');
        }
    }
    output
}

/// Whether the first non-whitespace, non-comment token on this line is one
/// of the directives masked for isolated semantic parsing. The block-comment
/// state crosses lines, and quoted/comment text never starts a directive.
fn semantic_directive_outside_comment(
    line: &str,
    in_block_comment: &mut bool,
    directives: &[&str],
) -> bool {
    let bytes = line.as_bytes();
    let mut index = 0usize;
    let mut saw_code = false;
    let mut in_string = false;
    let mut escaped = false;
    let mut directive = false;

    while index < bytes.len() {
        if *in_block_comment {
            if bytes[index..].starts_with(b"*/") {
                *in_block_comment = false;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if in_string {
            if escaped {
                escaped = false;
            } else if bytes[index] == b'\\' {
                escaped = true;
            } else if bytes[index] == b'"' {
                in_string = false;
            }
            index += 1;
            continue;
        }
        if bytes[index..].starts_with(b"//") {
            break;
        }
        if bytes[index..].starts_with(b"/*") {
            *in_block_comment = true;
            index += 2;
            continue;
        }
        if bytes[index].is_ascii_whitespace() {
            index += 1;
            continue;
        }
        if !saw_code && bytes[index] == b'`' {
            let start = index + 1;
            let mut end = start;
            while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
                end += 1;
            }
            directive = directives.contains(&&line[start..end]);
        }
        saw_code = true;
        if bytes[index] == b'"' {
            in_string = true;
        }
        index += 1;
    }
    directive
}

impl Drop for SemanticStage {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

fn empty_semantic_tokens() -> SemanticTokens {
    SemanticTokens {
        result_id: None,
        data: Vec::new(),
    }
}

fn cached_semantic_tokens(analysis: Option<&Analysis>, paths: &[String]) -> SemanticTokens {
    analysis
        .and_then(|analysis| {
            paths
                .iter()
                .map(|path| features::semantic_tokens_for(analysis, path))
                .find(|tokens| !tokens.data.is_empty())
        })
        .unwrap_or_else(empty_semantic_tokens)
}

fn compute_semantic_tokens(
    analysis: Option<Arc<Analysis>>,
    paths: Vec<String>,
    open_document: Option<OpenTokenDocument>,
    current: impl Fn() -> bool,
    parent_id: Option<u64>,
) -> (Option<OpenTokenResult>, SemanticTokens) {
    let cached = cached_semantic_tokens(analysis.as_deref(), &paths);
    let fresh = open_document.map(|(real, text, defines)| {
        let result = compute_open_document_semantic_tokens(real, text, defines, current, parent_id);
        if let Err(error) = &result {
            crate::llg_debug!(
                "semantic tokens: isolated collection failed before producing tokens: {error}"
            );
        }
        result
    });
    (fresh, cached)
}

fn compute_open_document_semantic_tokens(
    real: PathBuf,
    text: SharedText,
    defines: Vec<String>,
    current: impl Fn() -> bool,
    parent_id: Option<u64>,
) -> OpenTokenResult {
    open_document_semantic_tokens_if_current(&real, &text, &defines, current, parent_id)
}

/// Memoization key of one open-buffer isolated token stream: document URI,
/// buffer text digest and effective `-D` defines.  These are exactly the
/// inputs of the request-local `-parseonly` run, so any edit or defines
/// hot-reload produces a different key while an unchanged repeat hits.
fn open_token_cache_key(uri: &str, text: &str, defines: &[String]) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    for define in defines {
        define.hash(&mut hasher);
    }
    format!("open-tokens|{uri}|{:016x}", hasher.finish())
}

fn open_document_semantic_tokens_if_current(
    real: &Path,
    text: &str,
    defines: &[String],
    current: impl Fn() -> bool,
    parent_id: Option<u64>,
) -> std::result::Result<SemanticTokens, String> {
    let started = std::time::Instant::now();
    crate::llg_debug!(
        "event=semantic_tokens.open_parse.begin file={} bytes={} defines={} parent_id={:?}",
        real.display(),
        text.len(),
        defines.len(),
        parent_id
    );
    // Reject an obsolete request before it waits for the staging lock.  The
    // check after acquiring the lock closes the race with didChange while the
    // request was waiting; both checks happen before any stage or frontend
    // work is admitted.
    if !current() {
        crate::llg_debug!(
            "event=semantic_tokens.open_parse.end outcome=stale-before-staging file={} elapsed_us={}",
            real.display(),
            started.elapsed().as_micros()
        );
        return Err(STALE_OPEN_TOKEN_ERROR.to_owned());
    }
    // Project jobs acquire these locks in the same order.  Holding the
    // staging lock through parse and cleanup also prevents shutdown from
    // deleting the process shadow base while Surelog reads this copy.
    let staging_wait_started = std::time::Instant::now();
    crate::llg_debug!(
        "event=semantic_tokens.staging_lock.begin file={} parent_id={:?}",
        real.display(),
        parent_id
    );
    let _staging = shadow_staging_lock()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    crate::llg_debug!(
        "event=semantic_tokens.staging_lock.end outcome=acquired file={} elapsed_us={}",
        real.display(),
        staging_wait_started.elapsed().as_micros()
    );
    if !current() {
        crate::llg_debug!(
            "event=semantic_tokens.open_parse.end outcome=stale-after-staging file={} elapsed_us={}",
            real.display(),
            started.elapsed().as_micros()
        );
        return Err(STALE_OPEN_TOKEN_ERROR.to_owned());
    }
    let stage_started = std::time::Instant::now();
    let stage = match SemanticStage::new(real, text, defines) {
        Ok(stage) => stage,
        Err(error) => {
            let error = format!("failed to stage open document: {error}");
            crate::llg_debug!(
                "event=semantic_tokens.open_stage.end outcome=error file={} elapsed_us={} error={}",
                real.display(),
                stage_started.elapsed().as_micros(),
                error
            );
            crate::llg_debug!(
                "event=semantic_tokens.open_parse.end outcome=error file={} token_count=0 elapsed_us={}",
                real.display(),
                started.elapsed().as_micros()
            );
            return Err(error);
        }
    };
    crate::llg_debug!(
        "event=semantic_tokens.open_stage.end outcome=ok file={} elapsed_us={}",
        real.display(),
        stage_started.elapsed().as_micros()
    );
    clean_analysis_scratch();
    crate::llg_trace!(
        "event=semantic_tokens.open_stage.cleanup outcome=ok file={}",
        real.display()
    );
    let result = stage
        .path
        .to_str()
        .ok_or_else(|| "staged semantic source path is not UTF-8".to_owned())
        .and_then(|path| {
            // A revision can change while the request-local stage is being
            // written.  Check again immediately before entering Surelog so a
            // stale buffer cannot start the expensive parse.
            if !current() {
                crate::llg_debug!(
                    "event=semantic_tokens.open_parse.frontend outcome=stale-before-surelog file={} elapsed_us={}",
                    real.display(),
                    started.elapsed().as_micros()
                );
                Err(STALE_OPEN_TOKEN_ERROR.to_owned())
            } else {
                features::semantic_tokens_for_open_document_with_parent(path, defines, parent_id)
            }
        });
    clean_analysis_scratch();
    crate::llg_debug!(
        "event=semantic_tokens.open_parse.end outcome={} file={} token_count={} elapsed_us={}",
        if result.is_ok() { "ok" } else { "error" },
        real.display(),
        result.as_ref().map_or(0, |tokens| tokens.data.len()),
        started.elapsed().as_micros()
    );
    result
}

fn select_semantic_tokens(
    fresh: Option<std::result::Result<SemanticTokens, String>>,
    cached: SemanticTokens,
    buffer_is_current: bool,
) -> SemanticTokens {
    // The isolated parse returns an authoritative empty result when the
    // current buffer has a syntax error.  The cache is used only when
    // staging/session work (or the blocking task) fails before producing a
    // result; a successful empty result must therefore remain authoritative.
    if buffer_is_current {
        fresh.and_then(std::result::Result::ok).unwrap_or(cached)
    } else {
        cached
    }
}

/// Whether a full-text `didChange` actually carries new content.  Identical
/// full-text changes (same document text re-sent, e.g. by editor save/format
/// flows) must not reschedule the root: nothing the analysis reads changed.
fn did_change_is_new_content(current: Option<&str>, next: &str) -> bool {
    match current {
        Some(text) => text != next,
        None => true,
    }
}

fn shadow_staging_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Last-resort removal of the whole per-process shadow base, run on the LSP
/// `exit` path (see `main.rs`).  Idempotent; the staging lock is acquired
/// here so the sweep never races a running compile job.
pub(crate) fn emergency_shadow_cleanup() {
    let _staging = shadow_staging_lock()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    features::cleanup_process_shadow();
}

/// Remove every root's staged files plus the entire process shadow base.
///
/// The staging lock is acquired INSIDE this function (not by callers) so it
/// can run on `tokio::task::spawn_blocking` while keeping its ordering
/// guarantee against compile jobs.
fn cleanup_shadow_state_blocking(shadows: Vec<ShadowPaths>) {
    let _staging = shadow_staging_lock()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    for shadow in &shadows {
        shadow.cleanup();
    }
    // Deterministic whole-base removal: also drops the analysis scratch
    // directory and anything staged outside a root's tracked set.
    features::cleanup_process_shadow();
}

/// Empty the shared analysis scratch directory (keeping the directory
/// itself, which may be a running job's CWD).
///
/// Surelog artifacts (`slpp_all/`, logs, caches) accumulate there across
/// jobs; jobs are serialized behind the shadow-staging lock, so removing the
/// contents between jobs cannot race a running analysis.
fn clean_analysis_scratch() {
    let scratch = features::analysis_scratch_dir();
    let Ok(entries) = std::fs::read_dir(&scratch) else {
        return;
    };
    for entry in entries.filter_map(|entry| entry.ok()) {
        let path = entry.path();
        if path.is_dir() {
            let _ = std::fs::remove_dir_all(&path);
        } else {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// Rebuild the include-dependency reverse index (dep path → dependent roots)
/// from every root's resolved deps.
fn rebuild_dep_dependents(state: &mut BackendState) {
    let mut map: BTreeMap<PathBuf, BTreeSet<RootKey>> = BTreeMap::new();
    for (key, root) in &state.roots {
        for dep in &root.include_deps {
            map.entry(dep.clone()).or_default().insert(key.clone());
        }
    }
    state.dep_dependents = map;
}

// ── Shared-file aggregation ──────────────────────────────────────────────────

/// Number of roots whose discovery or resolved include-dep sets track `path`.
fn shared_tracker_count(state: &BackendState, path: &Path) -> usize {
    state
        .roots
        .values()
        .filter(|root| {
            root.discovered.iter().any(|file| file == path) || root.include_deps.contains(path)
        })
        .count()
}

/// Annotate a hover served from a file tracked by multiple roots so users can
/// tell which root's configuration produced configuration-dependent sections
/// (parameter values, define-derived text).
fn annotate_shared_hover(hover: &mut Hover, root_name: &str) {
    if let HoverContents::Markup(markup) = &mut hover.contents {
        markup.value.push_str(&format!(
            "\n\n---\n[{root_name}] parameter/define values follow this root's configuration"
        ));
    }
}

/// Hashable position key (`lsp_types::Position` is not `Hash`).
type PosKey = (u32, u32);

fn pos_key(position: Position) -> PosKey {
    (position.line, position.character)
}

type DiagKey = ((PosKey, PosKey), u8, Option<NumberOrString>, String);

/// Wire severity number for digest/duplicate keys (`None` → 0).
fn severity_tag(severity: Option<DiagnosticSeverity>) -> u8 {
    match severity {
        Some(DiagnosticSeverity::ERROR) => 1,
        Some(DiagnosticSeverity::WARNING) => 2,
        Some(DiagnosticSeverity::INFORMATION) => 3,
        Some(DiagnosticSeverity::HINT) => 4,
        _ => 0,
    }
}

/// Stable identity of a diagnostic for exact-duplicate detection:
/// same range/severity/code/message.
fn diagnostic_key(diagnostic: &Diagnostic) -> DiagKey {
    (
        (
            pos_key(diagnostic.range.start),
            pos_key(diagnostic.range.end),
        ),
        severity_tag(diagnostic.severity),
        diagnostic.code.clone(),
        diagnostic.message.clone(),
    )
}

fn diagnostics_digest(diagnostics: &[Diagnostic]) -> String {
    let mut digest = String::new();
    for diagnostic in diagnostics {
        digest.push_str(&format!("{:?};", diagnostic_key(diagnostic)));
    }
    digest
}

/// Merge one shared file's diagnostics from every tracking root into the
/// published union.
///
/// Exact duplicates (same range/severity/code/message) appear once.  When
/// distinct findings share a location, non-owner-root copies carry a
/// `[<root-name>]` message suffix identifying which root's configuration
/// produced them.
fn merge_shared_file_diagnostics(
    mut entries: Vec<(RootKey, bool, Diagnostic)>,
    names: &BTreeMap<RootKey, String>,
) -> Vec<Diagnostic> {
    // Owner-first ordering gives the unlabeled copy to the owner when exact
    // duplicates cancel out.
    entries.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    let mut kept: Vec<(RootKey, bool, Diagnostic)> = Vec::new();
    let mut seen: HashSet<DiagKey> = HashSet::new();
    for (key, owner, diagnostic) in entries {
        if seen.insert(diagnostic_key(&diagnostic)) {
            kept.push((key, owner, diagnostic));
        }
    }
    // Conflict labeling: distinct findings sharing one location get a root
    // label on every non-owner copy.
    let mut by_range: HashMap<(PosKey, PosKey), Vec<usize>> = HashMap::new();
    for (index, (_, _, diagnostic)) in kept.iter().enumerate() {
        by_range
            .entry((
                pos_key(diagnostic.range.start),
                pos_key(diagnostic.range.end),
            ))
            .or_default()
            .push(index);
    }
    for indices in by_range.values() {
        if indices.len() < 2 {
            continue;
        }
        let conflicting = indices
            .iter()
            .map(|index| diagnostic_key(&kept[*index].2))
            .collect::<HashSet<_>>()
            .len()
            > 1;
        if !conflicting {
            continue;
        }
        for index in indices {
            let (key, owner, diagnostic) = &mut kept[*index];
            if *owner {
                continue;
            }
            let name = names
                .get(key)
                .cloned()
                .unwrap_or_else(|| key.display().to_string());
            diagnostic.message = format!("{} [{name}]", diagnostic.message);
        }
    }
    kept.into_iter()
        .map(|(_, _, diagnostic)| diagnostic)
        .collect()
}

/// Compute the diagnostic union for every file tracked by more than one root
/// and publish the unions that changed since the last commit.
///
/// v1 model ("owner root wins"): each shared file is analyzed only by its
/// single longest-prefix owner root, so the union usually is just the owner's
/// diagnostics — but it is published for tracked files even when they are not
/// open.  Should multiple roots ever hold results for the same file (e.g.
/// right after an ownership transfer), identical findings collapse to one and
/// conflicting ones are labeled per root.
///
/// Suppression edges handled here (review P1-2): a file whose underlying
/// state changed must never stay hidden behind an unchanged digest.
///
/// * A file that leaves the shared set because a folder was removed keeps a
///   previously published union; the SURVIVING root's view is republished
///   (owner-wins), falling back to clearing when the survivor has no
///   diagnostics.
/// * A still-shared file whose diagnostics vanished everywhere (fixed or
///   cleared by a reload) gets its stale union cleared instead of keeping the
///   old errors at the client.
fn aggregate_shared_publications(
    state: &mut BackendState,
    skip_uris: &BTreeSet<Url>,
) -> Vec<(Url, Vec<Diagnostic>)> {
    if skip_uris.is_empty() && state.roots.len() < 2 && state.published_shared.is_empty() {
        return Vec::new();
    }
    let names: BTreeMap<RootKey, String> = state
        .roots
        .iter()
        .map(|(key, root)| (key.clone(), root.descriptor.id.clone()))
        .collect();
    let descriptors: Vec<RootDescriptor> = state
        .roots
        .values()
        .map(|root| root.descriptor.clone())
        .collect();
    let mut trackers: BTreeMap<PathBuf, Vec<RootKey>> = BTreeMap::new();
    for (key, root) in &state.roots {
        for file in root.discovered.iter().chain(&root.include_deps) {
            trackers.entry(file.clone()).or_default().push(key.clone());
        }
    }
    let mut computed: BTreeMap<Url, Vec<Diagnostic>> = BTreeMap::new();
    for (file, keys) in trackers {
        let Ok(uri) = Url::from_file_path(&file) else {
            continue;
        };
        // Only files with a live publication need republication decisions;
        // never-shared single-root files keep their normal (unpublished)
        // closed-file behavior.
        let was_published = state.published_shared.contains_key(&uri);
        if keys.len() < 2 && !was_published {
            continue;
        }
        let owner = workspace::owning_root_unfiltered(&file, &descriptors).map(|o| o.root);
        let mut entries = Vec::new();
        for key in &keys {
            let Some(root) = state.roots.get(key) else {
                continue;
            };
            let Some(diagnostics) = root.all_diagnostics.get(&file) else {
                continue;
            };
            let is_owner = owner.as_ref().is_some_and(|owner| owner == key);
            for diagnostic in diagnostics {
                entries.push((key.clone(), is_owner, diagnostic.clone()));
            }
        }
        // Empty result for a still-tracked file: clear a previously
        // published union (the diagnostics were fixed/cleared elsewhere);
        // never-published files stay unpublished.
        let merged = if entries.is_empty() {
            was_published.then(Vec::new)
        } else {
            Some(merge_shared_file_diagnostics(entries, &names))
        };
        if let Some(diagnostics) = merged {
            computed.insert(uri, diagnostics);
        }
    }
    let mut publications = Vec::new();
    for (uri, diagnostics) in &computed {
        let digest = diagnostics_digest(diagnostics);
        if state.published_shared.get(uri).map(String::as_str) == Some(digest.as_str()) {
            continue;
        }
        state.published_shared.insert(uri.clone(), digest);
        if !skip_uris.contains(uri) {
            publications.push((uri.clone(), diagnostics.clone()));
        }
    }
    // Unions that vanished entirely (folder removed with no surviving
    // tracker, file dropped from discovery): clear them.
    let stale: Vec<Url> = state
        .published_shared
        .keys()
        .filter(|uri| !computed.contains_key(uri))
        .cloned()
        .collect();
    for uri in stale {
        state.published_shared.remove(&uri);
        publications.push((uri, Vec::new()));
    }
    publications
}

/// Build the dynamic watched-file options from a backend state snapshot.
///
/// Watchers cover each root's effective `llg.toml`, `.v`/`.sv` units under
/// every configured source directory, and the exact resolved include
/// dependencies (any extension).
fn watcher_options_from_state(state: &BackendState) -> LSPAny {
    let watcher = |pattern: &str| {
        let mut object = LSPObject::new();
        object.insert("globPattern".to_owned(), LSPAny::String(pattern.to_owned()));
        LSPAny::Object(object)
    };
    let mut watchers: Vec<LSPAny> = Vec::new();
    for root in state.roots.values() {
        // The effective config file (exact path).
        if let Some(path) = root.descriptor.config_path.to_str() {
            watchers.push(watcher(path));
        }
        // `.v`/`.sv` units under each configured source directory.
        let cfg = root.descriptor.effective_config();
        for dir in &cfg.sources.directories {
            if let Some(dir) = dir.to_str() {
                watchers.push(watcher(&format!("{dir}/**/*.v")));
                watchers.push(watcher(&format!("{dir}/**/*.sv")));
            }
        }
        // Exact resolved include dependencies, regardless of extension.
        for dep in &root.include_deps {
            if let Some(dep) = dep.to_str() {
                watchers.push(watcher(dep));
            }
        }
    }
    let mut options = LSPObject::new();
    options.insert("watchers".to_owned(), LSPAny::Array(watchers));
    LSPAny::Object(options)
}

fn explicit_include_targets(source: &str) -> Vec<String> {
    const DIRECTIVE: &[u8] = b"`include";

    let bytes = source.as_bytes();
    let mut targets = Vec::new();
    let mut index = 0;
    let mut in_block_comment = false;
    let mut in_line_comment = false;
    while index < bytes.len() {
        if in_block_comment {
            if bytes[index..].starts_with(b"*/") {
                in_block_comment = false;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if in_line_comment {
            if bytes[index] == b'\n' {
                in_line_comment = false;
            }
            index += 1;
            continue;
        }

        if bytes[index..].starts_with(b"//") {
            in_line_comment = true;
            index += 2;
            continue;
        }
        if bytes[index..].starts_with(b"/*") {
            in_block_comment = true;
            index += 2;
            continue;
        }
        if bytes[index] == b'"' {
            index += 1;
            let mut escaped = false;
            while index < bytes.len() {
                if escaped {
                    escaped = false;
                    index += 1;
                } else if bytes[index] == b'\\' {
                    escaped = true;
                    index += 1;
                } else if bytes[index] == b'"' {
                    index += 1;
                    break;
                } else {
                    index += 1;
                }
            }
            continue;
        }

        if bytes[index..].starts_with(DIRECTIVE) {
            let after = index + DIRECTIVE.len();
            if after == bytes.len()
                || !(bytes[after].is_ascii_alphanumeric() || matches!(bytes[after], b'_' | b'$'))
            {
                let mut target_start = after;
                while target_start < bytes.len() && bytes[target_start].is_ascii_whitespace() {
                    target_start += 1;
                }
                if target_start < bytes.len() && bytes[target_start] == b'"' {
                    let mut target_end = target_start + 1;
                    let mut escaped = false;
                    while target_end < bytes.len() {
                        if escaped {
                            escaped = false;
                        } else if bytes[target_end] == b'\\' {
                            escaped = true;
                        } else if bytes[target_end] == b'"' {
                            targets.push(
                                String::from_utf8_lossy(&bytes[target_start + 1..target_end])
                                    .into_owned(),
                            );
                            index = target_end + 1;
                            break;
                        }
                        target_end += 1;
                    }
                    if index != target_end + 1 {
                        index = target_end;
                    }
                    continue;
                }
            }
        }
        index += 1;
    }
    targets
}

/// Check both the lexical path and the resolved filesystem target against a
/// set of allowed directories.  The lexical check intentionally runs first so
/// a `..` escape is rejected even when a symlink happens to point back into an
/// allowed directory.
fn canonical_include_target(
    allowed: &[PathBuf],
    resolved: &Path,
) -> std::result::Result<Option<PathBuf>, ()> {
    if !is_under_any(allowed, resolved) {
        return Err(());
    }

    let canonical_allowed: Vec<PathBuf> = allowed
        .iter()
        .map(|dir| std::fs::canonicalize(dir).unwrap_or_else(|_| dir.clone()))
        .collect();
    match std::fs::symlink_metadata(resolved) {
        Ok(_) => {
            let canonical = std::fs::canonicalize(resolved).map_err(|_| ())?;
            if is_under_any(&canonical_allowed, &canonical) {
                Ok(Some(canonical))
            } else {
                Err(())
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            // A missing include is left to Surelog to diagnose, but an
            // existing symlinked parent must not hide an outside resolution.
            let mut parent = resolved.parent();
            while let Some(candidate) = parent {
                if let Ok(canonical) = std::fs::canonicalize(candidate) {
                    if !is_under_any(&canonical_allowed, &canonical) {
                        return Err(());
                    }
                    break;
                }
                parent = candidate.parent();
            }
            Ok(None)
        }
        Err(_) => Ok(None),
    }
}

fn is_under_any(dirs: &[PathBuf], path: &Path) -> bool {
    dirs.iter()
        .any(|dir| workspace::root_relative_path(dir, path).is_some())
}

fn open_document_value<'a>(
    path: &Path,
    open_documents: &'a OpenDocuments,
) -> Option<&'a SharedText> {
    open_documents.get(path).or_else(|| {
        std::fs::canonicalize(path)
            .ok()
            .and_then(|canonical| open_documents.get(&canonical))
    })
}

fn open_document_text<'a>(path: &Path, open_documents: &'a OpenDocuments) -> Option<&'a str> {
    open_document_value(path, open_documents).map(|text| text.as_str())
}

/// Return the byte length that can be measured without reading an input.
/// Open UTF-8 buffers are authoritative; closed files use metadata so an
/// over-limit input is rejected before any unbounded source read or staging.
fn measured_input_bytes(path: &Path, open_documents: &OpenDocuments) -> Option<u64> {
    open_document_text(path, open_documents)
        .map(|text| text.len() as u64)
        .or_else(|| std::fs::metadata(path).ok().map(|metadata| metadata.len()))
}

fn input_identity(path: &Path) -> PathBuf {
    std::fs::canonicalize(path)
        .ok()
        .or_else(|| workspace::normalize_absolute_path(path))
        .unwrap_or_else(|| path.to_path_buf())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IncludeResolutionError {
    Unauthorized,
    SnapshotUnavailable,
}

/// Resolve a literal include using the same search order as the `-I` arguments
/// handed to Surelog: an absolute target is used as-is; a relative target is
/// tried beside the including file first, then in each configured source or
/// explicit include directory.  Each candidate goes through the existing
/// lexical and symlink containment policy before it is accepted.
fn resolve_include_target(
    allowed: &[PathBuf],
    source: &Path,
    target: &str,
    open_documents: &OpenDocuments,
    snapshots: &InputSnapshots,
) -> std::result::Result<Option<PathBuf>, ()> {
    resolve_include_target_with_policy(allowed, source, target, open_documents, snapshots, false)
        .map_err(|_| ())
}

fn resolve_admitted_include_target(
    allowed: &[PathBuf],
    source: &Path,
    target: &str,
    open_documents: &OpenDocuments,
    snapshots: &InputSnapshots,
) -> std::result::Result<Option<PathBuf>, IncludeResolutionError> {
    resolve_include_target_with_policy(allowed, source, target, open_documents, snapshots, true)
}

fn resolve_include_target_with_policy(
    allowed: &[PathBuf],
    source: &Path,
    target: &str,
    open_documents: &OpenDocuments,
    snapshots: &InputSnapshots,
    require_snapshot: bool,
) -> std::result::Result<Option<PathBuf>, IncludeResolutionError> {
    let target = Path::new(target);
    let mut candidates = Vec::new();
    if target.is_absolute() {
        candidates.push(target.to_owned());
    } else {
        candidates.push(
            source
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .join(target),
        );
        candidates.extend(allowed.iter().map(|directory| directory.join(target)));
    }

    let mut seen = HashSet::new();
    for candidate in candidates {
        let Some(candidate) = workspace::normalize_absolute_path(&candidate) else {
            continue;
        };
        if !seen.insert(candidate.clone()) {
            continue;
        }
        match canonical_include_target(allowed, &candidate)
            .map_err(|_| IncludeResolutionError::Unauthorized)?
        {
            Some(_) => {
                if require_snapshot
                    && prepared_input_text(&candidate, open_documents, snapshots).is_none()
                {
                    return Err(IncludeResolutionError::SnapshotUnavailable);
                }
                return Ok(Some(candidate));
            }
            // An open buffer can supply a file which does not exist on disk.
            // It is still subject to the same lexical/symlink policy above;
            // only the filesystem-existence part of resolution is replaced by
            // the authoritative open text.
            None if open_document_text(&candidate, open_documents).is_some()
                || snapshots.text(&candidate).is_some() =>
            {
                return Ok(Some(candidate));
            }
            None => continue,
        }
    }
    Ok(None)
}

/// Read a closed input with a bounded exact read.  The extra byte makes a
/// file that grows after metadata measurement fail admission instead of
/// allowing a later staging read to exceed the configured budget.
fn read_closed_input_snapshot(
    path: &Path,
    max_file_bytes: u64,
) -> std::result::Result<String, InputSizeLimit> {
    let metadata_bytes = match std::fs::metadata(path) {
        Ok(metadata) => {
            let measured_bytes = metadata.len();
            if measured_bytes > max_file_bytes {
                return Err(InputSizeLimit {
                    path: path.to_path_buf(),
                    measured_bytes,
                    configured_limit: max_file_bytes,
                    kind: InputSizeLimitKind::PerFile,
                    total_bytes: None,
                });
            }
            Some(measured_bytes)
        }
        Err(_) => None,
    };
    let file = std::fs::File::open(path).map_err(|_| InputSizeLimit {
        path: path.to_path_buf(),
        measured_bytes: metadata_bytes.unwrap_or_default(),
        configured_limit: max_file_bytes,
        kind: InputSizeLimitKind::Unreadable,
        total_bytes: None,
    })?;
    let mut bytes = Vec::new();
    let read_limit = max_file_bytes.saturating_add(1);
    file.take(read_limit)
        .read_to_end(&mut bytes)
        .map_err(|_| InputSizeLimit {
            path: path.to_path_buf(),
            measured_bytes: bytes.len() as u64,
            configured_limit: max_file_bytes,
            kind: InputSizeLimitKind::Unreadable,
            total_bytes: None,
        })?;
    let measured_bytes = bytes.len() as u64;
    if measured_bytes > max_file_bytes {
        return Err(InputSizeLimit {
            path: path.to_path_buf(),
            measured_bytes,
            configured_limit: max_file_bytes,
            kind: InputSizeLimitKind::PerFile,
            total_bytes: None,
        });
    }
    String::from_utf8(bytes).map_err(|error| InputSizeLimit {
        path: path.to_path_buf(),
        measured_bytes: error.as_bytes().len() as u64,
        configured_limit: max_file_bytes,
        kind: InputSizeLimitKind::Unreadable,
        total_bytes: None,
    })
}

/// Return the exact admitted text, preferring the captured snapshot over any
/// live document text.  Root compilation units must always have an admitted
/// snapshot before they reach staging or Surelog.
fn prepared_input_text<'a>(
    path: &Path,
    open_documents: &'a OpenDocuments,
    snapshots: &'a InputSnapshots,
) -> Option<&'a str> {
    snapshots
        .text(path)
        .or_else(|| open_document_text(path, open_documents))
}

/// Check root compilation units and their resolved literal include graph
/// before staging.  Every existing/open input is measured once by canonical
/// identity; include cycles and alternate spellings therefore cannot inflate
/// the total budget.  Readable closed inputs are retained as exact snapshots
/// for the subsequent isolation and staging passes.  Every discovered root
/// must yield a bounded UTF-8 snapshot; an unreadable or missing root is
/// rejected before it can reach Surelog on its live real path.
fn enforce_input_budget(
    config: &LlgConfig,
    files: &[PathBuf],
    open_documents: &OpenDocuments,
) -> std::result::Result<InputBudget, InputBudgetFailure> {
    let allowed = config::include_dirs(config);
    let mut pending: VecDeque<PathBuf> = files.iter().cloned().collect();
    let mut visited = HashSet::new();
    let mut include_deps = BTreeSet::new();
    let mut total_bytes = 0u64;
    let mut snapshots = InputSnapshots::default();

    while let Some(source) = pending.pop_front() {
        if !visited.insert(input_identity(&source)) {
            continue;
        }

        let measured = measured_input_bytes(&source, open_documents);
        let path = source.clone();
        if measured.is_some_and(|measured_bytes| measured_bytes > config.analysis.max_file_bytes) {
            let measured_bytes = measured.expect("measured bytes just checked");
            return Err(InputBudgetFailure {
                limit: InputSizeLimit {
                    path,
                    measured_bytes,
                    configured_limit: config.analysis.max_file_bytes,
                    kind: InputSizeLimitKind::PerFile,
                    total_bytes: None,
                },
                include_deps,
            });
        }

        let text = if let Some(text) = open_document_value(&source, open_documents) {
            Arc::clone(text)
        } else {
            match read_closed_input_snapshot(&source, config.analysis.max_file_bytes) {
                Ok(text) => Arc::new(text),
                Err(limit) => {
                    return Err(InputBudgetFailure {
                        limit,
                        include_deps,
                    });
                }
            }
        };
        // Closed files are accounted from metadata as the non-reading
        // measurement.  If the bounded snapshot observed growth after that
        // metadata read, retain the larger exact byte count so the total
        // budget cannot be bypassed by a file that grew during admission.
        let measured_bytes = measured
            .unwrap_or_else(|| text.len() as u64)
            .max(text.len() as u64);
        if measured_bytes > config.analysis.max_file_bytes {
            return Err(InputBudgetFailure {
                limit: InputSizeLimit {
                    path,
                    measured_bytes,
                    configured_limit: config.analysis.max_file_bytes,
                    kind: InputSizeLimitKind::PerFile,
                    total_bytes: None,
                },
                include_deps,
            });
        }

        let Some(next_total) = total_bytes.checked_add(measured_bytes) else {
            return Err(InputBudgetFailure {
                limit: InputSizeLimit {
                    path,
                    measured_bytes,
                    configured_limit: config.analysis.max_total_input_bytes,
                    kind: InputSizeLimitKind::Total,
                    total_bytes: Some(u64::MAX),
                },
                include_deps,
            });
        };
        if next_total > config.analysis.max_total_input_bytes {
            return Err(InputBudgetFailure {
                limit: InputSizeLimit {
                    path,
                    measured_bytes,
                    configured_limit: config.analysis.max_total_input_bytes,
                    kind: InputSizeLimitKind::Total,
                    total_bytes: Some(next_total),
                },
                include_deps,
            });
        }
        total_bytes = next_total;

        snapshots.insert(&source, Arc::clone(&text));
        for target in explicit_include_targets(text.as_str()) {
            let Ok(Some(resolved)) =
                resolve_include_target(&allowed, &source, &target, open_documents, &snapshots)
            else {
                continue;
            };
            if measured_input_bytes(&resolved, open_documents).is_some() {
                include_deps.insert(resolved.clone());
                pending.push_back(resolved);
            }
        }
    }

    Ok(InputBudget { snapshots })
}

fn compile_result_files(paths: &[PathBuf]) -> Vec<(PathBuf, String)> {
    paths
        .iter()
        .map(|real| (real.clone(), real.to_string_lossy().into_owned()))
        .collect()
}

fn attach_fileless_diagnostics(analysis: &mut Analysis, compiled_path: &str) {
    for diagnostic in &mut analysis.diagnostics {
        if diagnostic.file.is_none() {
            diagnostic.file = Some(compiled_path.to_owned());
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IncludeStageFailure {
    path: PathBuf,
    message: String,
    dependencies: BTreeSet<PathBuf>,
}

/// Stage every resolved include dependency (from disk or an open buffer) into
/// the shadow tree so relative includes and unsaved headers resolve.  Returns
/// the set of resolved include dependency real paths (any extension).
fn stage_include_tree(
    config: &LlgConfig,
    files: &[(PathBuf, String)],
    open_documents: &OpenDocuments,
    snapshots: &InputSnapshots,
    shadow: &ShadowPaths,
) -> std::result::Result<BTreeSet<PathBuf>, IncludeStageFailure> {
    let allowed = config::include_dirs(config);
    let mut pending: Vec<PathBuf> = files.iter().map(|(real, _)| real.clone()).collect();
    let mut visited = HashSet::new();
    let mut deps = BTreeSet::new();
    while let Some(source) = pending.pop() {
        let identity = std::fs::canonicalize(&source).unwrap_or_else(|_| source.clone());
        if !visited.insert(identity) {
            continue;
        }
        let Some(text) = prepared_input_text(&source, open_documents, snapshots) else {
            return Err(IncludeStageFailure {
                path: source.clone(),
                message: format!(
                    "input-staging: input {} has no admitted bounded snapshot; compile rejected to preserve the input budget",
                    source.display()
                ),
                dependencies: deps,
            });
        };
        for target in explicit_include_targets(text) {
            let resolved = match resolve_admitted_include_target(
                &allowed,
                &source,
                &target,
                open_documents,
                snapshots,
            ) {
                Ok(Some(resolved)) => resolved,
                Ok(None) => continue,
                Err(IncludeResolutionError::Unauthorized) => {
                    return Err(IncludeStageFailure {
                        path: source.clone(),
                        message: format!(
                            "input-staging: SystemVerilog include target {target:?} in {} escapes configured source/include directories",
                            source.display()
                        ),
                        dependencies: deps,
                    });
                }
                Err(IncludeResolutionError::SnapshotUnavailable) => {
                    return Err(IncludeStageFailure {
                        path: source.clone(),
                        message: format!(
                            "input-staging: resolved include target {target:?} in {} has no admitted bounded snapshot; compile rejected to preserve the input budget",
                            source.display()
                        ),
                        dependencies: deps,
                    });
                }
            };
            let Some(nested_text) = prepared_input_text(&resolved, open_documents, snapshots)
            else {
                return Err(IncludeStageFailure {
                    path: resolved,
                    message: format!(
                        "input-staging: resolved include target {target:?} in {} has no admitted bounded snapshot; compile rejected to preserve the input budget",
                        source.display()
                    ),
                    dependencies: deps,
                });
            };
            // A missing on-disk file can still be supplied by an open buffer.
            // Existing files are copied too, so relative includes continue to
            // resolve from the shadow tree rather than the real source tree.
            deps.insert(resolved.clone());
            if let Err(error) = shadow.stage(&resolved, nested_text) {
                return Err(IncludeStageFailure {
                    path: resolved,
                    message: format!(
                        "input-staging: failed to stage include from its admitted bounded snapshot; compile rejected to preserve the input budget: {error}"
                    ),
                    dependencies: deps,
                });
            }
            pending.push(resolved);
        }
    }
    Ok(deps)
}

/// Reject include targets that escape every configured source/include
/// directory of the owning root.  Returns `(diagnostic_file, message)` for the
/// first offending include.
fn preflight_include_isolation(
    config: &LlgConfig,
    files: &[(PathBuf, String)],
    open_documents: &OpenDocuments,
    snapshots: &InputSnapshots,
) -> Option<(String, String)> {
    let allowed = config::include_dirs(config);
    let mut pending: Vec<(PathBuf, String)> = files
        .iter()
        .map(|(real, compiled)| (real.clone(), compiled.clone()))
        .collect();
    let mut visited = HashSet::new();
    while let Some((source, diagnostic_file)) = pending.pop() {
        let identity = std::fs::canonicalize(&source).unwrap_or_else(|_| source.clone());
        if !visited.insert(identity) {
            continue;
        }
        let Some(text) = prepared_input_text(&source, open_documents, snapshots) else {
            return Some((
                diagnostic_file,
                format!(
                    "input-snapshot: input {} has no admitted bounded snapshot; compile rejected to preserve the input budget",
                    source.display()
                ),
            ));
        };
        for target in explicit_include_targets(text) {
            let resolved = match resolve_admitted_include_target(
                &allowed,
                &source,
                &target,
                open_documents,
                snapshots,
            ) {
                Ok(Some(resolved)) => resolved,
                Ok(None) => continue,
                Err(IncludeResolutionError::Unauthorized) => {
                    return Some((
                        diagnostic_file.clone(),
                        format!(
                            "SystemVerilog include target {target:?} in {} escapes configured source/include directories (allowed: {})",
                            source.display(),
                            allowed
                                .iter()
                                .map(|dir| dir.display().to_string())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                    ));
                }
                Err(IncludeResolutionError::SnapshotUnavailable) => {
                    return Some((
                        diagnostic_file.clone(),
                        format!(
                            "input-snapshot: resolved include target {target:?} in {} has no admitted bounded snapshot; compile rejected to preserve the input budget",
                            source.display()
                        ),
                    ));
                }
            };
            if prepared_input_text(&resolved, open_documents, snapshots).is_some() {
                pending.push((resolved, diagnostic_file.clone()));
            }
        }
    }
    None
}

impl Backend {
    /// Start an isolated open-document parse whose completion is detached
    /// from the request future.  The blocking closure owns the coordinator,
    /// so request cancellation cannot remove the flight while Surelog is
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
        let root = Self::uri_to_path(&uri).and_then(|path| self.source_root(&path));
        if let Some(root) = &root {
            notification.set_root(|| root.to_string_lossy().into_owned());
        }
        let (initialized, admission) = {
            let mut state = self.lock_state();
            let admission =
                Self::admit_document_text(&mut state, uri.clone(), params.text_document.text, true);
            (state.initialized, admission)
        };
        match admission {
            Ok(_) => {
                if initialized {
                    if let Some(root) = root {
                        self.schedule_roots_with_parent(vec![root], Some(notification.id()));
                    }
                }
                notification.complete(if initialized { "scheduled" } else { "deferred" }, 1);
            }
            Err(limit) => {
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
                let defines: Vec<String> = root
                    .descriptor
                    .effective_config()
                    .compile
                    .defines
                    .into_iter()
                    .map(|define| format!("-D{define}"))
                    .collect();
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
        let fallback_analysis = analysis.clone();
        let fallback_paths = paths.clone();

        // The request released BackendState after capturing this text.  Do
        // not even inspect/serve an open-buffer cache entry for an obsolete
        // revision; it must fall back to the committed snapshot without
        // acquiring a flight or starting isolated work.
        if let Some(captured_text) = captured_text.as_deref() {
            if !open_document_is_current(&self.state, &uri, captured_text) {
                let tokens = cached_semantic_tokens(fallback_analysis.as_deref(), &fallback_paths);
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
                    let tokens =
                        cached_semantic_tokens(fallback_analysis.as_deref(), &fallback_paths);
                    request.complete("stale", tokens.data.len());
                    return Ok(Some(SemanticTokensResult::Tokens(tokens)));
                }
                crate::llg_debug!("semantic tokens rejected: {}", limit.message());
                let tokens = cached_semantic_tokens(fallback_analysis.as_deref(), &fallback_paths);
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
                    let tokens =
                        cached_semantic_tokens(fallback_analysis.as_deref(), &fallback_paths);
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
                let tokens = cached_semantic_tokens(fallback_analysis.as_deref(), &fallback_paths);
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
                    cached_semantic_tokens(cached_analysis.as_deref(), &cached_paths)
                })
                .await
                .unwrap_or_else(|_| {
                    cached_semantic_tokens(fallback_analysis.as_deref(), &fallback_paths)
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
                            cached_semantic_tokens(fallback_analysis.as_deref(), &fallback_paths),
                            "stale",
                        )
                    } else {
                        leader.finish(Ok(tokens.clone()));
                        let cached_analysis = fallback_analysis.clone();
                        let cached_paths = fallback_paths.clone();
                        let cached = tokio::task::spawn_blocking(move || {
                            cached_semantic_tokens(cached_analysis.as_deref(), &cached_paths)
                        })
                        .await
                        .unwrap_or_else(|_| {
                            cached_semantic_tokens(fallback_analysis.as_deref(), &fallback_paths)
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
                                    )
                                })
                                .await
                                .unwrap_or_else(|_| {
                                    cached_semantic_tokens(
                                        fallback_analysis.as_deref(),
                                        &fallback_paths,
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
                    cached_semantic_tokens(fallback_analysis.as_deref(), &fallback_paths),
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
                        cached_semantic_tokens(fallback_analysis.as_deref(), &fallback_paths),
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
                Err(tower_lsp::jsonrpc::Error::invalid_params(message))
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
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    /// The process-global shadow base is shared by every test in this binary.
    /// Tests that stage files and call `cleanup_process_shadow` must be
    /// serialized so one test's cleanup cannot delete another test's staged
    /// files.
    use crate::features::TEST_PROCESS_SHADOW_LOCK as SHADOW_TESTS_LOCK;

    fn temp_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "llg_lsp_{name}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ))
    }

    /// A config whose only allowed dirs are `source_dirs` (used by the
    /// include-preflight tests).
    fn config_with_dirs(root: &Path, source_dirs: Vec<PathBuf>) -> LlgConfig {
        let mut config = config::default_config(root);
        config.sources.directories = source_dirs;
        config
    }

    fn config_with_budget(
        root: &Path,
        source_dirs: Vec<PathBuf>,
        max_file_bytes: u64,
        max_total_input_bytes: u64,
    ) -> LlgConfig {
        let mut config = config_with_dirs(root, source_dirs);
        config.analysis = config::AnalysisConfig {
            max_file_bytes,
            max_total_input_bytes,
        };
        config
    }

    fn empty_backend_state() -> BackendState {
        BackendState {
            roots: BTreeMap::new(),
            documents: BTreeMap::new(),
            dynamic_watched_files: false,
            watchers_registered: false,
            registered_watchers_digest: None,
            initialized: false,
            shutting_down: false,
            pending_logs: Vec::new(),
            merged: None,
            initial_pending: BTreeSet::new(),
            ready_sent: false,
            dep_dependents: BTreeMap::new(),
            published_shared: BTreeMap::new(),
            next_generation: 0,
        }
    }

    #[test]
    fn did_change_suppresses_only_identical_full_text() {
        let tracked = "module a;\nendmodule\n".to_owned();
        assert!(did_change_is_new_content(None, "anything"), "first change");
        assert!(
            did_change_is_new_content(Some(tracked.as_str()), "module b;\nendmodule\n"),
            "real edit"
        );
        assert!(
            !did_change_is_new_content(Some(tracked.as_str()), "module a;\nendmodule\n"),
            "identical full-text re-send must not reschedule"
        );
    }

    #[test]
    fn document_admission_rejects_before_storage_and_reuses_shared_text() {
        let uri = Url::parse("file:///tmp/llg-admission.sv").expect("document URI");
        let mut state = empty_backend_state();
        let oversized = "x".repeat(config::DEFAULT_MAX_FILE_BYTES as usize + 1);
        let rejection = Backend::admit_document_text(&mut state, uri.clone(), oversized, true)
            .expect_err("uninitialized documents use the conservative default limit");
        assert_eq!(rejection.kind, InputSizeLimitKind::PerFile);
        assert!(
            state.documents.is_empty(),
            "rejected text is never retained"
        );

        let accepted = "module m; endmodule\n".to_owned();
        assert!(
            Backend::admit_document_text(&mut state, uri.clone(), accepted, true)
                .expect("admit document")
        );
        let stored = state
            .documents
            .get(&uri)
            .expect("stored admitted text")
            .clone();
        assert!(!Backend::admit_document_text(
            &mut state,
            uri.clone(),
            "module m; endmodule\n".to_owned(),
            false,
        )
        .expect("identical didChange"));
        assert!(Arc::ptr_eq(
            &stored,
            state
                .documents
                .get(&uri)
                .expect("unchanged text remains shared")
        ));
    }

    #[test]
    fn admitted_snapshot_indexes_share_the_same_text_allocation() {
        let path = PathBuf::from("/tmp/llg-shared-snapshot.sv");
        let text = Arc::new("module m; endmodule\n".to_owned());
        let mut snapshots = InputSnapshots::default();
        snapshots.insert(&path, Arc::clone(&text));

        assert!(Arc::ptr_eq(
            &text,
            snapshots.by_path.get(&path).expect("path snapshot")
        ));
        assert!(Arc::ptr_eq(
            &text,
            snapshots
                .by_identity
                .get(&input_identity(&path))
                .expect("identity snapshot")
        ));
    }

    #[test]
    fn open_token_cache_key_tracks_text_and_defines() {
        let defines_a = vec!["-DWIDTH=8".to_owned()];
        let defines_b = vec!["-DWIDTH=16".to_owned()];
        let key = open_token_cache_key("file:///p.sv", "module m;\nendmodule", &defines_a);
        assert_eq!(
            key,
            open_token_cache_key("file:///p.sv", "module m;\nendmodule", &defines_a),
            "unchanged inputs repeat the key"
        );
        assert_ne!(
            key,
            open_token_cache_key("file:///p.sv", "module n;\nendmodule", &defines_a),
            "buffer edit changes the key"
        );
        assert_ne!(
            key,
            open_token_cache_key("file:///p.sv", "module m;\nendmodule", &defines_b),
            "defines hot-reload changes the key"
        );
        assert_ne!(
            key,
            open_token_cache_key("file:///q.sv", "module m;\nendmodule", &defines_a),
            "uri is part of the key"
        );
    }

    #[test]
    fn oversized_open_token_buffer_is_rejected_before_flight_admission() {
        let path = PathBuf::from("/tmp/llg-open-too-large.sv");
        let open_document = (path.clone(), Arc::new("12345".to_owned()), Vec::new());
        let limit =
            open_token_size_limit(Some(&open_document), Some(4)).expect("over-limit buffer");
        assert_eq!(limit.measured_bytes, 5);
        assert!(limit.message().contains("max_file_bytes=4"));

        // The semantic-token handler returns on this decision before it
        // constructs a cache key or calls `OpenTokenFlightRegistry::acquire`.
        let registry = Arc::new(OpenTokenFlightRegistry::new());
        assert_eq!(registry.len(), 0);
        let boundary_document = (path, Arc::new("1234".to_owned()), Vec::new());
        assert!(open_token_size_limit(Some(&boundary_document), Some(4)).is_none());
        assert!(open_token_size_limit(None, Some(4)).is_none());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn closed_request_snapshot_is_bounded_and_reports_unreadable_inputs() {
        let root = temp_root("request_snapshot");
        std::fs::create_dir_all(&root).expect("create request snapshot root");
        let source = root.join("request.sv");
        std::fs::write(&source, b"12345").expect("write request source");

        let failure =
            read_closed_input_snapshot(&source, 4).expect_err("snapshot exceeds max file bytes");
        assert_eq!(failure.kind, InputSizeLimitKind::PerFile);
        assert_eq!(failure.measured_bytes, 5);

        std::fs::write(&source, [0xff, 0xfe]).expect("write invalid UTF-8");
        let failure =
            read_closed_input_snapshot(&source, 4).expect_err("invalid UTF-8 is not a snapshot");
        assert_eq!(failure.kind, InputSizeLimitKind::Unreadable);
        assert!(failure.message().contains("input-snapshot"));

        std::fs::remove_file(&source).expect("remove request source");
        let failure =
            read_closed_input_snapshot(&source, 4).expect_err("missing disk input is unavailable");
        assert_eq!(failure.kind, InputSizeLimitKind::Unreadable);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn input_budget_allows_exact_file_boundary_and_rejects_over_limit() {
        let root = temp_root("budget_boundary");
        std::fs::create_dir_all(&root).expect("create budget root");
        let source = root.join("top.sv");
        std::fs::write(&source, b"1234").expect("write boundary source");
        let config = config_with_budget(&root, vec![root.clone()], 4, 4);

        assert!(
            enforce_input_budget(&config, std::slice::from_ref(&source), &BTreeMap::new()).is_ok()
        );

        std::fs::write(&source, b"12345").expect("write over-limit source");
        let failure =
            enforce_input_budget(&config, std::slice::from_ref(&source), &BTreeMap::new())
                .expect_err("one byte over the per-file boundary");
        assert_eq!(failure.limit.kind, InputSizeLimitKind::PerFile);
        assert_eq!(failure.limit.measured_bytes, 5);
        assert!(failure
            .limit
            .message()
            .contains(&source.display().to_string()));
        assert!(failure.limit.message().contains("max_file_bytes=4"));
        assert!(failure.limit.message().contains("per-file budget"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn input_budget_rejects_a_missing_root_before_frontend_admission() {
        let root = temp_root("budget_missing_root");
        std::fs::create_dir_all(&root).expect("create budget root");
        let source = root.join("missing.sv");
        let config = config_with_budget(&root, vec![root.clone()], 4, 4);

        let failure =
            enforce_input_budget(&config, std::slice::from_ref(&source), &BTreeMap::new())
                .expect_err("a missing root cannot be passed to Surelog on its live path");
        assert_eq!(failure.limit.kind, InputSizeLimitKind::Unreadable);
        assert_eq!(failure.limit.path, source);
        assert!(failure.limit.message().contains("input-snapshot"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn open_text_wins_over_larger_disk_metadata_for_budgeting() {
        let root = temp_root("budget_open_wins");
        std::fs::create_dir_all(&root).expect("create budget root");
        let source = root.join("top.sv");
        std::fs::write(&source, b"on-disk text is longer").expect("write disk source");
        let config = config_with_budget(&root, vec![root.clone()], 4, 4);
        let open = BTreeMap::from([(source.clone(), Arc::new("four".to_owned()))]);

        assert!(enforce_input_budget(&config, std::slice::from_ref(&source), &open).is_ok());
        let over = BTreeMap::from([(source.clone(), Arc::new("five!".to_owned()))]);
        let failure = enforce_input_budget(&config, std::slice::from_ref(&source), &over)
            .expect_err("open UTF-8 text exceeds the limit");
        assert_eq!(failure.limit.measured_bytes, 5);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn input_budget_counts_unique_canonical_include_paths_once_through_cycles() {
        let root = temp_root("budget_cycle");
        std::fs::create_dir_all(&root).expect("create budget root");
        let top = root.join("top.sv");
        let first = root.join("first.inc");
        let second = root.join("second.inc");
        std::fs::write(&top, b"`include \"first.inc\"\n").expect("write top");
        std::fs::write(&first, b"`include \"second.inc\"\n").expect("write first");
        std::fs::write(&second, b"`include \"./first.inc\"\n").expect("write second");
        let total = [&top, &first, &second]
            .iter()
            .map(|path| std::fs::metadata(path).expect("input metadata").len())
            .sum();
        let config = config_with_budget(&root, vec![root.clone()], total, total);

        assert!(
            enforce_input_budget(&config, std::slice::from_ref(&top), &BTreeMap::new()).is_ok()
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn input_budget_rejects_total_boundary_with_path_and_configured_limit() {
        let root = temp_root("budget_total");
        std::fs::create_dir_all(&root).expect("create budget root");
        let top = root.join("top.sv");
        let include = root.join("payload.inc");
        std::fs::write(&top, b"`include \"payload.inc\"\n").expect("write top");
        std::fs::write(&include, b"payload").expect("write include");
        let top_bytes = std::fs::metadata(&top).expect("top metadata").len();
        let include_bytes = std::fs::metadata(&include).expect("include metadata").len();
        let limit = top_bytes + include_bytes - 1;
        let config = config_with_budget(
            &root,
            vec![root.clone()],
            top_bytes.max(include_bytes),
            limit,
        );

        let failure = enforce_input_budget(&config, std::slice::from_ref(&top), &BTreeMap::new())
            .expect_err("total input budget is exceeded");
        assert_eq!(failure.limit.kind, InputSizeLimitKind::Total);
        assert_eq!(failure.limit.total_bytes, Some(top_bytes + include_bytes));
        assert!(failure
            .limit
            .message()
            .contains(&include.display().to_string()));
        assert!(failure
            .limit
            .message()
            .contains(&format!("max_total_input_bytes={limit}")));
        assert!(failure.limit.message().contains("total budget"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn input_budget_resolves_compile_include_dir_and_counts_it() {
        let _guard = SHADOW_TESTS_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let root = temp_root("budget_include_dir");
        let source_dir = root.join("src");
        let include_dir = root.join("include");
        std::fs::create_dir_all(&source_dir).expect("create source dir");
        std::fs::create_dir_all(&include_dir).expect("create include dir");
        let top = source_dir.join("top.sv");
        let header = include_dir.join("search-only.inc");
        let top_text = "`include \"search-only.inc\"\nmodule top; endmodule\n";
        let header_text = "// include-search-only\n";
        std::fs::write(&top, top_text).expect("write top");
        std::fs::write(&header, header_text).expect("write header");
        let top_bytes = top_text.len() as u64;
        let header_bytes = header_text.len() as u64;

        let mut config = config_with_budget(
            &root,
            vec![source_dir.clone()],
            top_bytes.max(header_bytes),
            top_bytes + header_bytes,
        );
        config.compile.include_dirs = vec![include_dir.clone()];
        let files = vec![(top.clone(), top.to_string_lossy().into_owned())];
        let budget = enforce_input_budget(&config, std::slice::from_ref(&top), &BTreeMap::new())
            .expect("include-search directory input is admitted");
        assert_eq!(
            prepared_input_text(&header, &BTreeMap::new(), &budget.snapshots),
            Some(header_text)
        );
        assert_eq!(
            resolve_include_target(
                &config::include_dirs(&config),
                &top,
                "search-only.inc",
                &BTreeMap::new(),
                &budget.snapshots,
            )
            .expect("resolve include")
            .as_deref(),
            Some(header.as_path())
        );
        assert!(
            preflight_include_isolation(&config, &files, &BTreeMap::new(), &budget.snapshots)
                .is_none()
        );

        let shadow = ShadowPaths::new();
        shadow.stage(&top, top_text).expect("stage root");
        let deps = stage_include_tree(
            &config,
            &files,
            &BTreeMap::new(),
            &budget.snapshots,
            &shadow,
        )
        .expect("stage admitted include dependency");
        assert!(
            deps.contains(&header),
            "include dependency is watched/staged"
        );
        let staged_header = shadow.shadow_path(&header).expect("staged header path");
        assert_eq!(
            std::fs::read_to_string(staged_header).expect("read staged header"),
            header_text
        );
        shadow.cleanup();
        features::cleanup_process_shadow();

        config.analysis.max_total_input_bytes = top_bytes + header_bytes - 1;
        let failure = enforce_input_budget(&config, std::slice::from_ref(&top), &BTreeMap::new())
            .expect_err("include-search directory input exceeds total budget");
        assert_eq!(failure.limit.kind, InputSizeLimitKind::Total);
        assert_eq!(failure.limit.path, header);
        assert!(failure.include_deps.contains(&header));
        assert!(failure.limit.message().contains("total budget"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn admitted_snapshots_are_reused_after_disk_and_open_text_changes() {
        let _guard = SHADOW_TESTS_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let root = temp_root("budget_snapshot");
        std::fs::create_dir_all(&root).expect("create snapshot root");
        let top = root.join("top.sv");
        let header = root.join("payload.inc");
        let original_top = "`include \"payload.inc\"\nmodule original; endmodule\n";
        let original_header = "// admitted header\n";
        std::fs::write(&top, original_top).expect("write original top");
        std::fs::write(&header, original_header).expect("write original header");
        let top_bytes = original_top.len() as u64;
        let header_bytes = original_header.len() as u64;
        let config = config_with_budget(
            &root,
            vec![root.clone()],
            top_bytes.max(header_bytes),
            top_bytes + header_bytes,
        );
        let files = vec![(top.clone(), top.to_string_lossy().into_owned())];
        let budget = enforce_input_budget(&config, std::slice::from_ref(&top), &BTreeMap::new())
            .expect("admit original inputs");

        // If staging rereads disk, it would see both this changed root (with
        // no include) and an over-limit header.  The admitted graph must use
        // the exact bounded snapshots instead.
        std::fs::write(&top, "module changed; endmodule\n").expect("change top after admission");
        std::fs::write(
            &header,
            "x".repeat((top_bytes.max(header_bytes) + 32) as usize),
        )
        .expect("grow header after admission");

        let changed_open_documents = BTreeMap::from([
            (
                top.clone(),
                Arc::new("module open_changed; endmodule\n".to_owned()),
            ),
            (
                header.clone(),
                Arc::new("// changed open header\n".to_owned()),
            ),
        ]);
        let shadow = ShadowPaths::new();
        let admitted_top = prepared_input_text(&top, &changed_open_documents, &budget.snapshots)
            .expect("admitted top snapshot");
        assert_eq!(admitted_top, original_top);
        shadow
            .stage(&top, admitted_top)
            .expect("stage admitted top");
        let deps = stage_include_tree(
            &config,
            &files,
            &changed_open_documents,
            &budget.snapshots,
            &shadow,
        )
        .expect("stage admitted include dependency");
        assert!(deps.contains(&header));
        assert_eq!(
            std::fs::read_to_string(shadow.shadow_path(&top).expect("staged top"))
                .expect("read staged top"),
            original_top
        );
        assert_eq!(
            std::fs::read_to_string(shadow.shadow_path(&header).expect("staged header"))
                .expect("read staged header"),
            original_header
        );
        shadow.cleanup();
        features::cleanup_process_shadow();
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn oversized_compile_job_publishes_limit_and_retains_last_good_snapshot() {
        let root = temp_root("budget_last_good");
        std::fs::create_dir_all(&root).expect("create budget root");
        let source = root.join("top.sv");
        std::fs::write(&source, b"12345").expect("write over-limit source");
        let config = config_with_budget(&root, vec![root.clone()], 4, 16);
        let descriptor = RootDescriptor::from_absolute(&root)
            .expect("root descriptor")
            .with_config(Some(Arc::new(config.clone())));
        let previous = Arc::new(features::empty_analysis());
        let mut roots = BTreeMap::new();
        roots.insert(
            root.clone(),
            RootState {
                descriptor,
                shadow: ShadowPaths::new(),
                discovered: vec![source.clone()],
                include_deps: BTreeSet::new(),
                lint_config: LintConfig::default(),
                config_errors: Vec::new(),
                config_warnings: Vec::new(),
                last_good: Some(Arc::clone(&previous)),
                diagnostics: BTreeMap::new(),
                published_digests: BTreeMap::new(),
                all_diagnostics: BTreeMap::new(),
                generation: 1,
                pending_parent_id: None,
                scheduler: SchedulerState::default(),
                analysis_epoch: 0,
            },
        );
        let state = Arc::new(Mutex::new(BackendState {
            roots,
            documents: BTreeMap::new(),
            dynamic_watched_files: false,
            watchers_registered: false,
            registered_watchers_digest: None,
            initialized: true,
            shutting_down: false,
            pending_logs: Vec::new(),
            merged: None,
            initial_pending: BTreeSet::new(),
            ready_sent: true,
            dep_dependents: BTreeMap::new(),
            published_shared: BTreeMap::new(),
            next_generation: 0,
        }));
        let job = RootJob {
            key: root.clone(),
            generation: 1,
            parent_id: None,
            shadow: ShadowPaths::new(),
            files: vec![source.clone()],
            previous_include_deps: BTreeSet::new(),
            open_documents: BTreeMap::new(),
            lint_config: LintConfig::default(),
            config,
        };

        let result = Backend::compile_job(&state, job).expect("size-limit result");
        let message = result
            .analysis
            .as_ref()
            .expect("fatal size-limit analysis")
            .diagnostics[0]
            .message
            .clone();
        assert!(message.contains("input-size-limit"));
        assert!(message.contains(&source.display().to_string()));
        let outcome = Backend::commit_job(&state, &root, 1, result);
        assert!(!outcome.valid_commit);
        assert!(outcome.publications.iter().any(|(uri, diagnostics)| {
            uri == &Url::from_file_path(&source).expect("source URI")
                && diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("input-size-limit"))
        }));
        let state = state.lock().unwrap_or_else(|error| error.into_inner());
        assert!(Arc::ptr_eq(
            state
                .roots
                .get(&root)
                .expect("root state")
                .last_good
                .as_ref()
                .expect("retained last-good snapshot"),
            &previous
        ));
        assert_eq!(
            state.roots.get(&root).expect("root state").discovered,
            vec![source]
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn open_token_freshness_barrier_rejects_stale_revision_before_admission() {
        let uri = Url::parse("file:///tmp/llg-stale-open.sv").expect("document URI");
        let state = Arc::new(Mutex::new(BackendState {
            roots: BTreeMap::new(),
            documents: BTreeMap::from([(uri.clone(), Arc::new("current".to_owned()))]),
            dynamic_watched_files: false,
            watchers_registered: false,
            registered_watchers_digest: None,
            initialized: true,
            shutting_down: false,
            pending_logs: Vec::new(),
            merged: None,
            initial_pending: BTreeSet::new(),
            ready_sent: true,
            dep_dependents: BTreeMap::new(),
            published_shared: BTreeMap::new(),
            next_generation: 0,
        }));
        let registry = Arc::new(OpenTokenFlightRegistry::new());

        assert!(
            !open_document_is_current(&state, &uri, "captured-before-edit"),
            "an obsolete captured revision must fail the admission barrier"
        );
        if open_document_is_current(&state, &uri, "captured-before-edit") {
            let _ = registry.acquire("stale-revision".to_owned());
        }
        assert_eq!(
            registry.len(),
            0,
            "a stale revision must not acquire an open-token flight"
        );

        assert!(
            open_document_is_current(&state, &uri, "current"),
            "the current revision remains eligible for isolated tokens"
        );
        let leader = match registry.acquire("current-revision".to_owned()) {
            OpenTokenFlightLease::Leader(leader) => leader,
            _ => panic!("the current revision should be admitted when capacity is available"),
        };
        assert_eq!(registry.len(), 1);
        drop(leader);
        assert_eq!(registry.len(), 0);

        state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .shutting_down = true;
        assert!(
            !open_document_is_current(&state, &uri, "current"),
            "shutdown makes an otherwise matching revision ineligible"
        );
    }

    #[test]
    fn open_token_flights_join_and_reclaim() {
        let registry = Arc::new(OpenTokenFlightRegistry::new());
        let leader = match registry.acquire("same-input".to_owned()) {
            OpenTokenFlightLease::Leader(leader) => leader,
            _ => panic!("first request must lead the flight"),
        };
        let leader_flight = Arc::clone(&leader.flight);
        let follower = match registry.acquire("same-input".to_owned()) {
            OpenTokenFlightLease::Follower(flight) => flight,
            _ => panic!("identical request must join the existing flight"),
        };
        assert!(Arc::ptr_eq(&leader_flight, &follower));
        assert_eq!(registry.len(), 1);

        drop(leader);
        assert!(leader_flight.result().is_some_and(|result| result.is_err()));
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn open_token_flights_refuse_overflow_without_a_second_parse() {
        let registry = Arc::new(OpenTokenFlightRegistry::new());
        let mut leaders = Vec::new();
        let parses_started = AtomicUsize::new(0);
        for index in 0..OPEN_TOKEN_FLIGHT_CAPACITY {
            match registry.acquire(format!("input-{index}")) {
                OpenTokenFlightLease::Leader(leader) => {
                    // An admitted leader is the only lease that can start a
                    // detached parse.  Keep each leader alive to hold the
                    // registry at its capacity while exercising overflow.
                    parses_started.fetch_add(1, Ordering::Relaxed);
                    leaders.push(leader);
                }
                _ => panic!("flight registry capacity should admit each bounded key"),
            }
        }
        assert_eq!(registry.len(), OPEN_TOKEN_FLIGHT_CAPACITY);
        assert!(matches!(
            registry.acquire("input-0".to_owned()),
            OpenTokenFlightLease::Follower(_)
        ));
        assert!(matches!(
            registry.acquire("overflow".to_owned()),
            OpenTokenFlightLease::Saturated
        ));
        assert_eq!(
            parses_started.load(Ordering::Relaxed),
            OPEN_TOKEN_FLIGHT_CAPACITY,
            "a saturated overflow key must not launch a second parse"
        );

        drop(leaders);
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn detached_token_coordinator_survives_leader_cancellation() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime");
        runtime.block_on(async {
            let registry = Arc::new(OpenTokenFlightRegistry::new());
            let mut leader = match registry.acquire("cancelled-request".to_owned()) {
                OpenTokenFlightLease::Leader(leader) => leader,
                _ => panic!("first request must lead the flight"),
            };
            let flight = Arc::clone(&leader.flight);
            let coordinator = leader.detach();
            // The request future may be cancelled immediately after it
            // submits the detached blocking parse.  Its Drop must not wake
            // followers early or remove this still-running flight.
            drop(leader);

            let (started_tx, started_rx) = std::sync::mpsc::channel();
            let (release_tx, release_rx) = std::sync::mpsc::channel();
            let parse_count = Arc::new(AtomicUsize::new(0));
            let parse_count_worker = Arc::clone(&parse_count);
            let worker = tokio::task::spawn_blocking(move || {
                parse_count_worker.fetch_add(1, Ordering::Relaxed);
                started_tx.send(()).expect("worker started");
                release_rx.recv().expect("worker release");
                let mut coordinator = coordinator;
                coordinator.finish(Ok(empty_semantic_tokens()));
            });
            // Dropping the join handle models request cancellation.  The
            // blocking closure still owns the coordinator and must finish.
            drop(worker);
            started_rx.recv().expect("detached worker started");

            let mut followers = Vec::new();
            for _ in 0..8 {
                match registry.acquire("cancelled-request".to_owned()) {
                    OpenTokenFlightLease::Follower(follower) => followers.push(follower),
                    _ => panic!("identical requests must join the detached flight"),
                }
            }
            assert_eq!(registry.len(), 1);
            assert_eq!(parse_count.load(Ordering::Relaxed), 1);
            release_tx.send(()).expect("release worker");
            for follower in followers {
                assert!(follower.wait().await.is_ok());
            }
            assert!(flight.result().is_some_and(|result| result.is_ok()));
            assert_eq!(registry.len(), 0);
        });
    }

    #[test]
    fn token_flight_stays_joinable_through_cache_publication() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime");
        runtime.block_on(async {
            let registry = Arc::new(OpenTokenFlightRegistry::new());
            let mut leader = match registry.acquire("publication-race".to_owned()) {
                OpenTokenFlightLease::Leader(leader) => leader,
                _ => panic!("first request must lead the flight"),
            };
            let coordinator = leader.detach();
            drop(leader);
            let published = Arc::new(MemoCache::new(1));
            let published_worker = Arc::clone(&published);
            let (published_tx, published_rx) = std::sync::mpsc::channel();
            let (release_tx, release_rx) = std::sync::mpsc::channel();
            let worker = tokio::task::spawn_blocking(move || {
                published_worker.put("publication-race".to_owned(), empty_semantic_tokens());
                published_tx.send(()).expect("cache publication");
                release_rx.recv().expect("coordinator release");
                let mut coordinator = coordinator;
                coordinator.finish(Ok(empty_semantic_tokens()));
            });
            drop(worker);
            published_rx.recv().expect("published before completion");
            assert!(published.get(&"publication-race".to_owned()).is_some());
            // A request racing the publication boundary still joins the
            // existing flight; it cannot become a second leader between the
            // cache write and coordinator completion.
            let follower = match registry.acquire("publication-race".to_owned()) {
                OpenTokenFlightLease::Follower(follower) => follower,
                _ => panic!("flight must remain registered through publication"),
            };
            assert_eq!(registry.len(), 1);
            release_tx.send(()).expect("release coordinator");
            assert!(follower.wait().await.is_ok());
            assert_eq!(registry.len(), 0);
        });
    }

    #[test]
    fn inactive_ranges_cache_key_tracks_text_and_defines() {
        let defines_a = vec!["FOO".to_owned()];
        let defines_b = vec!["BAR".to_owned()];
        let key = inactive_ranges_cache_key("file:///p.sv", "`ifdef FOO\n`endif", &defines_a);
        assert_eq!(
            key,
            inactive_ranges_cache_key("file:///p.sv", "`ifdef FOO\n`endif", &defines_a),
            "unchanged inputs repeat the key"
        );
        assert_ne!(
            key,
            inactive_ranges_cache_key("file:///p.sv", "`ifdef BAR\n`endif", &defines_a),
            "buffer edit changes the key"
        );
        assert_ne!(
            key,
            inactive_ranges_cache_key("file:///p.sv", "`ifdef FOO\n`endif", &defines_b),
            "config defines hot-reload changes the key"
        );
        assert_ne!(
            key,
            inactive_ranges_cache_key("file:///q.sv", "`ifdef FOO\n`endif", &defines_a),
            "uri is part of the key"
        );
    }

    #[test]
    fn include_preflight_allows_configured_dirs_and_rejects_escapes() {
        let root = temp_root("preflight");
        let source_dir = root.join("src");
        std::fs::create_dir_all(&source_dir).expect("create include graph");
        let allowed_dir = root.join("shared");
        std::fs::create_dir_all(&allowed_dir).expect("create allowed include dir");
        let top = source_dir.join("top.sv");
        let allowed_include = allowed_dir.join("allowed.svh");
        std::fs::write(
            &top,
            "`include \"../shared/allowed.svh\"\nmodule top; endmodule\n",
        )
        .expect("write top");
        std::fs::write(&allowed_include, "// allowed\n").expect("write allowed include");

        let config = config_with_dirs(&root, vec![source_dir.clone(), allowed_dir.clone()]);
        let files = vec![(top.clone(), top.to_string_lossy().into_owned())];
        let budget = enforce_input_budget(&config, std::slice::from_ref(&top), &BTreeMap::new())
            .expect("admit configured include");
        assert!(
            preflight_include_isolation(&config, &files, &BTreeMap::new(), &budget.snapshots)
                .is_none()
        );

        // Escape to a directory outside the configured set.
        std::fs::write(
            &top,
            "`include \"../../outside.svh\"\nmodule top; endmodule\n",
        )
        .expect("write escape");
        let budget = enforce_input_budget(&config, std::slice::from_ref(&top), &BTreeMap::new())
            .expect("admit escape for isolation preflight");
        let failure =
            preflight_include_isolation(&config, &files, &BTreeMap::new(), &budget.snapshots);
        assert!(failure.is_some_and(|(_, message)| message.contains("outside.svh")));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn include_preflight_visits_filtered_files_and_cycles() {
        let root = temp_root("preflight_cycle");
        let source_dir = root.join("src");
        std::fs::create_dir_all(&source_dir).expect("create include graph");
        let top = source_dir.join("top.sv");
        let filtered = source_dir.join("filtered.inc");
        let nested = source_dir.join("nested.txt");
        std::fs::write(&top, "`include \"filtered.inc\"\nmodule top; endmodule\n")
            .expect("write top");
        std::fs::write(&filtered, "`include \"nested.txt\"\n").expect("write filtered");
        std::fs::write(&nested, "`include \"filtered.inc\"\n").expect("write nested");

        let config = config_with_dirs(&root, vec![source_dir.clone()]);
        let files = vec![(top.clone(), top.to_string_lossy().into_owned())];
        let budget = enforce_input_budget(&config, std::slice::from_ref(&top), &BTreeMap::new())
            .expect("admit include cycle");
        assert!(
            preflight_include_isolation(&config, &files, &BTreeMap::new(), &budget.snapshots)
                .is_none()
        );

        std::fs::write(&nested, "`include \"../../outside.svh\"\n").expect("write escape");
        let budget = enforce_input_budget(&config, std::slice::from_ref(&top), &BTreeMap::new())
            .expect("admit escape for isolation preflight");
        let failure =
            preflight_include_isolation(&config, &files, &BTreeMap::new(), &budget.snapshots);
        assert!(failure.is_some_and(|(_, message)| message.contains("outside.svh")));
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn include_preflight_rejects_outside_symlink_targets() {
        use std::os::unix::fs::symlink;

        let root = temp_root("preflight_symlink");
        let source_dir = root.join("src");
        std::fs::create_dir_all(&source_dir).expect("create symlink graph");
        let outside = root.with_file_name(format!(
            "{}_outside.inc",
            root.file_name()
                .expect("temporary root name")
                .to_string_lossy()
        ));
        let top = source_dir.join("top.sv");
        let link = source_dir.join("link.inc");
        std::fs::write(&outside, "// outside\n").expect("write outside include");
        std::fs::write(&top, "`include \"link.inc\"\nmodule top; endmodule\n").expect("write top");
        symlink(&outside, &link).expect("create outside include symlink");

        let config = config_with_dirs(&root, vec![source_dir.clone()]);
        let files = vec![(top.clone(), top.to_string_lossy().into_owned())];
        let budget = enforce_input_budget(&config, std::slice::from_ref(&top), &BTreeMap::new())
            .expect("admit symlink for isolation preflight");
        let failure =
            preflight_include_isolation(&config, &files, &BTreeMap::new(), &budget.snapshots);
        assert!(failure.is_some_and(|(_, message)| message.contains("link.inc")));
        let _ = std::fs::remove_file(outside);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn commit_job_publishes_fileless_diagnostics_on_an_open_compiled_file() {
        use llg::ffi::surelog::{Diag, Severity};

        let root_path = PathBuf::from("/tmp/llg_lsp_fileless_diagnostic");
        let real = root_path.join("top.sv");
        let compiled = real.to_string_lossy().into_owned();
        let uri = Url::from_file_path(&real).expect("real document URI");
        let descriptor = RootDescriptor::from_absolute(&root_path).expect("root descriptor");
        let mut roots = BTreeMap::new();
        roots.insert(
            root_path.clone(),
            RootState {
                descriptor,
                shadow: ShadowPaths::new(),
                discovered: vec![real.clone()],
                include_deps: BTreeSet::new(),
                lint_config: LintConfig::default(),
                config_errors: Vec::new(),
                config_warnings: Vec::new(),
                last_good: None,
                diagnostics: BTreeMap::new(),
                published_digests: BTreeMap::new(),
                all_diagnostics: BTreeMap::new(),
                generation: 1,
                pending_parent_id: None,
                scheduler: SchedulerState::default(),
                analysis_epoch: 0,
            },
        );
        let state = Arc::new(Mutex::new(BackendState {
            roots,
            documents: BTreeMap::from([(uri.clone(), Arc::new(String::new()))]),
            dynamic_watched_files: false,
            watchers_registered: false,
            registered_watchers_digest: None,
            initialized: true,
            shutting_down: false,
            pending_logs: Vec::new(),
            merged: None,
            initial_pending: BTreeSet::new(),
            ready_sent: true,
            dep_dependents: BTreeMap::new(),
            published_shared: BTreeMap::new(),
            next_generation: 0,
        }));
        let mut analysis = features::empty_analysis();
        analysis.diagnostics.push(Diag {
            severity: Severity::Error,
            file: None,
            line: 0,
            col: 0,
            message: "UHDM database build failed: synthetic failure".to_owned(),
        });
        let result = CompileResult {
            analysis: Some(analysis),
            files: vec![(real.clone(), compiled)],
            include_deps: BTreeSet::new(),
        };

        let outcome = Backend::commit_job(&state, &root_path, 1, result);
        let (_, diagnostics) = outcome
            .publications
            .into_iter()
            .find(|(candidate, _)| candidate == &uri)
            .expect("fileless diagnostic publication");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.message == "UHDM database build failed: synthetic failure"
        }));
    }

    /// An analysis whose outcome is Parse/Compile but that carries servable
    /// data (Surelog still elaborated the surviving files) must populate
    /// `last_good` and count as a valid commit for watcher registration; an
    /// analysis WITHOUT feature data retains the previous snapshot; an absent
    /// analysis clears it.
    #[test]
    fn commit_job_serves_features_from_partial_analyses() {
        use crate::features::AnalysisOutcome;
        use llg::core::model::{DesignModel, ModuleDef};

        fn served_analysis(outcome: AnalysisOutcome) -> Analysis {
            let model = DesignModel {
                design_name: "top".to_owned(),
                top_instances: Vec::new(),
                modules: vec![ModuleDef {
                    name: "m".to_owned(),
                    file: Some("/tmp/llg_lsp_partial_feature/top.sv".to_owned()),
                    line: 1,
                    col: 8,
                    end_line: 1,
                    end_col: 9,
                }],
                packages: Vec::new(),
                classes: Vec::new(),
            };
            Analysis::new_with_outcome(
                outcome,
                Vec::new(),
                model,
                Vec::new(),
                Vec::new(),
                HashMap::new(),
                features::ConnectionInputs::default(),
            )
        }

        fn last_good_is_some(state: &BackendState, root: &RootKey) -> bool {
            state.roots.get(root).expect("root").last_good.is_some()
        }

        let root_path = PathBuf::from("/tmp/llg_lsp_partial_feature");
        let real = root_path.join("top.sv");
        let descriptor = RootDescriptor::from_absolute(&root_path).expect("root descriptor");
        let mut roots = BTreeMap::new();
        roots.insert(
            root_path.clone(),
            RootState {
                descriptor,
                shadow: ShadowPaths::new(),
                discovered: vec![real.clone()],
                include_deps: BTreeSet::new(),
                lint_config: LintConfig::default(),
                config_errors: Vec::new(),
                config_warnings: Vec::new(),
                last_good: None,
                diagnostics: BTreeMap::new(),
                published_digests: BTreeMap::new(),
                all_diagnostics: BTreeMap::new(),
                generation: 1,
                pending_parent_id: None,
                scheduler: SchedulerState::default(),
                analysis_epoch: 0,
            },
        );
        let state = Arc::new(Mutex::new(BackendState {
            roots,
            documents: BTreeMap::new(),
            dynamic_watched_files: false,
            watchers_registered: false,
            registered_watchers_digest: None,
            initialized: true,
            shutting_down: false,
            pending_logs: Vec::new(),
            merged: None,
            initial_pending: BTreeSet::new(),
            ready_sent: true,
            dep_dependents: BTreeMap::new(),
            published_shared: BTreeMap::new(),
            next_generation: 0,
        }));
        let commit = |generation: u64, analysis: Option<Analysis>| {
            state
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .roots
                .get_mut(&root_path)
                .unwrap()
                .generation = generation;
            Backend::commit_job(
                &state,
                &root_path,
                generation,
                CompileResult {
                    analysis,
                    files: vec![(real.clone(), real.to_string_lossy().into_owned())],
                    include_deps: BTreeSet::new(),
                },
            )
        };

        // A Parse-outcome analysis WITH feature data becomes servable.
        let outcome = commit(1, Some(served_analysis(AnalysisOutcome::Parse)));
        assert!(outcome.valid_commit, "partial analysis is a usable commit");
        assert!(
            outcome.module_explorer_changed,
            "first servable model must notify the explorer"
        );
        assert!(
            last_good_is_some(
                &state.lock().unwrap_or_else(|error| error.into_inner()),
                &root_path
            ),
            "servable partial analysis must populate last_good"
        );

        // A Compile outcome with the same data still serves.
        let outcome = commit(2, Some(served_analysis(AnalysisOutcome::Compile)));
        assert!(outcome.valid_commit);
        assert!(
            !outcome.module_explorer_changed,
            "an unchanged committed model must not notify the explorer"
        );

        // An unservable analysis (Fatal preflight shape: no data) RETAINS the
        // previous snapshot and does not re-register watchers.
        let outcome = commit(
            3,
            Some(Analysis::fatal_preflight("include escapes workspace")),
        );
        assert!(!outcome.valid_commit);
        assert!(
            !outcome.module_explorer_changed,
            "a fatal analysis retains the explorer snapshot"
        );
        assert!(
            last_good_is_some(
                &state.lock().unwrap_or_else(|error| error.into_inner()),
                &root_path
            ),
            "unservable analysis must retain the last-good snapshot"
        );

        // An absent analysis (empty project) clears it.
        let outcome = commit(4, None);
        assert!(
            outcome.module_explorer_changed,
            "clearing the committed model must notify the explorer"
        );
        assert!(!last_good_is_some(
            &state.lock().unwrap_or_else(|error| error.into_inner()),
            &root_path
        ));
    }

    #[test]
    fn module_explorer_changed_notification_uses_the_stable_method_name() {
        assert_eq!(
            <LlgModuleExplorerChanged as tower_lsp::lsp_types::notification::Notification>::METHOD,
            "llg/moduleExplorerChanged"
        );
    }

    #[test]
    fn commit_job_publishes_closed_files_and_suppresses_unchanged_payloads() {
        use llg::ffi::surelog::{Diag, Severity};

        let root_path = PathBuf::from("/tmp/llg_lsp_project_wide_diagnostics");
        let open_real = root_path.join("open.sv");
        let closed_real = root_path.join("closed.sv");
        let open_uri = Url::from_file_path(&open_real).expect("open document URI");
        let closed_uri = Url::from_file_path(&closed_real).expect("closed document URI");
        let descriptor = RootDescriptor::from_absolute(&root_path).expect("root descriptor");
        let mut roots = BTreeMap::new();
        roots.insert(
            root_path.clone(),
            RootState {
                descriptor,
                shadow: ShadowPaths::new(),
                discovered: vec![open_real.clone(), closed_real.clone()],
                include_deps: BTreeSet::new(),
                lint_config: LintConfig::default(),
                config_errors: Vec::new(),
                config_warnings: Vec::new(),
                last_good: None,
                diagnostics: BTreeMap::new(),
                published_digests: BTreeMap::new(),
                all_diagnostics: BTreeMap::new(),
                generation: 1,
                pending_parent_id: None,
                scheduler: SchedulerState::default(),
                analysis_epoch: 0,
            },
        );
        let state = Arc::new(Mutex::new(BackendState {
            roots,
            // Only open.sv is open; closed.sv is never opened by the client.
            documents: BTreeMap::from([(open_uri.clone(), Arc::new(String::new()))]),
            dynamic_watched_files: false,
            watchers_registered: false,
            registered_watchers_digest: None,
            initialized: true,
            shutting_down: false,
            pending_logs: Vec::new(),
            merged: None,
            initial_pending: BTreeSet::new(),
            ready_sent: true,
            dep_dependents: BTreeMap::new(),
            published_shared: BTreeMap::new(),
            next_generation: 0,
        }));

        let files = vec![
            (open_real.clone(), open_real.to_string_lossy().into_owned()),
            (
                closed_real.clone(),
                closed_real.to_string_lossy().into_owned(),
            ),
        ];
        let error_analysis = || {
            let mut analysis = features::empty_analysis();
            for (file, message) in [
                (open_real.to_string_lossy().into_owned(), "error in open.sv"),
                (
                    closed_real.to_string_lossy().into_owned(),
                    "error in closed.sv",
                ),
            ] {
                analysis.diagnostics.push(Diag {
                    severity: Severity::Error,
                    file: Some(file),
                    line: 1,
                    col: 1,
                    message: message.to_owned(),
                });
            }
            analysis
        };

        let payload_for = |outcome: &CommitOutcome, uri: &Url| -> Option<Vec<Diagnostic>> {
            outcome
                .publications
                .iter()
                .find(|(candidate, _)| candidate == uri)
                .map(|(_, diagnostics)| diagnostics.clone())
        };

        // First commit publishes project-wide: BOTH compilation units get a
        // publication, including the never-opened one.
        let outcome = Backend::commit_job(
            &state,
            &root_path,
            1,
            CompileResult {
                analysis: Some(error_analysis()),
                files: files.clone(),
                include_deps: BTreeSet::new(),
            },
        );
        assert_eq!(
            payload_for(&outcome, &closed_uri)
                .expect("closed file published")
                .len(),
            1
        );
        assert_eq!(
            payload_for(&outcome, &open_uri)
                .expect("open file published")
                .len(),
            1
        );

        // Identical re-commit: unchanged payloads are suppressed.
        state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .roots
            .get_mut(&root_path)
            .unwrap()
            .generation = 2;
        let outcome = Backend::commit_job(
            &state,
            &root_path,
            2,
            CompileResult {
                analysis: Some(error_analysis()),
                files,
                include_deps: BTreeSet::new(),
            },
        );
        assert!(
            outcome.publications.is_empty(),
            "unchanged payloads must be suppressed: {:?}",
            outcome.publications
        );

        // Fixing the NEVER-OPENED file clears exactly its URI; the unchanged
        // open-file payload stays suppressed.
        let mut fixed_analysis = features::empty_analysis();
        fixed_analysis.diagnostics.push(Diag {
            severity: Severity::Error,
            file: Some(open_real.to_string_lossy().into_owned()),
            line: 1,
            col: 1,
            message: "error in open.sv".to_owned(),
        });
        state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .roots
            .get_mut(&root_path)
            .unwrap()
            .generation = 3;
        let outcome = Backend::commit_job(
            &state,
            &root_path,
            3,
            CompileResult {
                analysis: Some(fixed_analysis),
                files: vec![
                    (open_real.clone(), open_real.to_string_lossy().into_owned()),
                    (
                        closed_real.clone(),
                        closed_real.to_string_lossy().into_owned(),
                    ),
                ],
                include_deps: BTreeSet::new(),
            },
        );
        assert_eq!(
            payload_for(&outcome, &closed_uri).expect("closed URI cleared via union"),
            Vec::<Diagnostic>::new()
        );
        assert!(
            !outcome.publications.iter().any(|(uri, _)| uri == &open_uri),
            "unchanged open-file payload must be suppressed"
        );
    }

    #[test]
    fn include_staging_prefers_open_text_and_handles_missing_cycles() {
        let _guard = SHADOW_TESTS_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let root = temp_root("shadow_include");
        std::fs::create_dir_all(&root).expect("create shadow include root");
        let top = root.join("top.sv");
        let header = root.join("defs.inc");
        std::fs::write(&top, "`include \"defs.inc\"\nmodule top; endmodule\n").expect("write top");
        std::fs::write(&header, "`include \"defs.inc\"\n// disk text\n").expect("write header");
        let mut open = BTreeMap::new();
        open.insert(
            top.clone(),
            Arc::new("`include \"defs.inc\"\nmodule unsaved_top; endmodule\n".to_owned()),
        );
        open.insert(
            header.clone(),
            Arc::new("`include \"missing.inc\"\n// open text\n".to_owned()),
        );
        let shadow = ShadowPaths::new();
        let files = vec![(top.clone(), top.to_string_lossy().into_owned())];
        let config = config_with_dirs(&root, vec![root.clone()]);
        let budget = enforce_input_budget(&config, std::slice::from_ref(&top), &open)
            .expect("admit open include graph");
        assert!(preflight_include_isolation(&config, &files, &open, &budget.snapshots).is_none());
        shadow
            .stage(&top, open[&top].as_str())
            .expect("stage open top");
        let deps = stage_include_tree(&config, &files, &open, &budget.snapshots, &shadow)
            .expect("stage admitted include dependency");

        let staged_top = shadow.shadow_path(&top).expect("staged top");
        let staged_header = shadow.shadow_path(&header).expect("staged header");
        assert_eq!(
            std::fs::read_to_string(staged_top).expect("read staged top"),
            open[&top].as_str()
        );
        assert_eq!(
            std::fs::read_to_string(staged_header).expect("read staged header"),
            open[&header].as_str()
        );
        assert!(deps.contains(&header));
        assert!(!shadow
            .shadow_path(&root.join("missing.inc"))
            .is_some_and(|path| path.exists()));
        shadow.cleanup();
        features::cleanup_process_shadow();
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn include_staging_failure_rejects_instead_of_using_live_path() {
        let _guard = SHADOW_TESTS_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let root = temp_root("shadow_include_failure");
        std::fs::create_dir_all(&root).expect("create include failure root");
        let top = root.join("top.sv");
        let header = root.join("defs.inc");
        std::fs::write(&top, "\x60include \"defs.inc\"\nmodule top; endmodule\n")
            .expect("write top");
        std::fs::write(&header, "// bounded header\n").expect("write header");
        let config = config_with_dirs(&root, vec![root.clone()]);
        let files = vec![(top.clone(), top.to_string_lossy().into_owned())];
        let budget = enforce_input_budget(&config, std::slice::from_ref(&top), &BTreeMap::new())
            .expect("admit include graph");

        let blocker = root.join("shadow-blocker");
        std::fs::write(&blocker, "not a directory").expect("write shadow blocker");
        let shadow = ShadowPaths {
            base: blocker,
            staged: Arc::new(Mutex::new(BTreeSet::new())),
        };
        let failure = stage_include_tree(
            &config,
            &files,
            &BTreeMap::new(),
            &budget.snapshots,
            &shadow,
        )
        .expect_err("a staging I/O failure must reject the include stage");
        assert_eq!(failure.path, header);
        assert!(failure.message.contains("input-staging"));
        assert!(failure.message.contains("admitted bounded snapshot"));
        assert!(failure.dependencies.contains(&header));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn shadow_paths_stage_and_round_trip() {
        let _guard = SHADOW_TESTS_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let root = temp_root("shadow_roundtrip");
        std::fs::create_dir_all(&root).expect("create shadow root");
        let shadow = ShadowPaths::new();
        let real = root.join("src").join("my file.sv");
        let staged = shadow
            .stage(&real, "module top; endmodule\n")
            .expect("stage buffer");
        assert_eq!(staged, shadow.shadow_path(&real).expect("shadow path"));
        assert_eq!(shadow.real_path(&staged), Some(real.clone()));
        assert_eq!(
            std::fs::read_to_string(&staged).expect("read staged file"),
            "module top; endmodule\n"
        );
        assert!(shadow.shadow_path(Path::new("relative.sv")).is_none());
        assert!(shadow.real_path(&real).is_none());
        shadow.remove(&real);
        assert!(!staged.exists());
        shadow.cleanup();
        features::cleanup_process_shadow();
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn staging_never_touches_the_project_tree() {
        let _guard = SHADOW_TESTS_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let root = temp_root("readonly");
        let source_dir = root.join("rtl");
        std::fs::create_dir_all(&source_dir).expect("create project tree");
        let source = source_dir.join("top.sv");
        let text = "module top; endmodule\n";
        std::fs::write(&source, text).expect("write project source");

        let shadow = ShadowPaths::new();
        let staged = shadow.stage(&source, text).expect("stage");
        assert!(staged.starts_with(&features::process_shadow_base()));
        assert_eq!(
            std::fs::read_to_string(&source).expect("project source unchanged"),
            text
        );
        assert!(
            !root.join("target").exists(),
            "project tree gained a target dir"
        );
        shadow.cleanup();
        features::cleanup_process_shadow();
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn semantic_staging_guard_does_not_recreate_shadow_state_after_shutdown() {
        // Arrange
        let _guard = SHADOW_TESTS_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        features::cleanup_process_shadow();
        let base = features::process_shadow_base();
        let source = temp_root("semantic_shutdown_guard").join("top.sv");
        assert!(!base.exists(), "precondition: shadow base is absent");

        // Act
        let result = open_document_semantic_tokens_if_current(
            &source,
            "module top; endmodule\n",
            &[],
            || false,
            None,
        );

        // Assert
        assert!(result.is_err(), "stale semantic work must be rejected");
        assert!(
            !base.exists(),
            "rejected semantic work recreated the process shadow base"
        );
    }

    #[test]
    fn semantic_parse_masks_directives_without_shifting_source_positions() {
        // Arrange
        let source = "  `include \"defs.svh\"\nmodule top;\n`define VALUE \\\n+  1\nlogic `WIDTH data;\nendmodule\n";

        // Act
        let masked = mask_semantic_preprocessor_directives(source, &[]);
        let source_lines = source.lines().collect::<Vec<_>>();
        let masked_lines = masked.lines().collect::<Vec<_>>();

        // Assert
        assert_eq!(masked_lines.len(), source_lines.len());
        assert!(masked_lines[0].trim().is_empty());
        assert_eq!(masked_lines[1], "module top;");
        assert!(masked_lines[2].trim().is_empty());
        assert!(masked_lines[3].trim().is_empty());
        assert_eq!(masked_lines[4], "logic `WIDTH data;");
        assert_eq!(masked_lines[5], "endmodule");
        for (original, replacement) in source_lines.iter().zip(masked_lines) {
            assert_eq!(original.chars().count(), replacement.chars().count());
        }
    }

    #[test]
    fn semantic_parse_masks_inactive_conditional_branches() {
        // Arrange: the inactive branch is deliberately incomplete and would
        // create a false syntax error if both branch bodies reached Surelog.
        let source = "`ifdef ACTIVE\nmodule top;\n`else\nmodule broken(\n`endif\nendmodule\n";

        // Act
        let masked = mask_semantic_preprocessor_directives(source, &["-DACTIVE=1".to_owned()]);
        let lines = masked.lines().collect::<Vec<_>>();

        // Assert
        assert!(lines[0].trim().is_empty());
        assert_eq!(lines[1], "module top;");
        assert!(lines[2].trim().is_empty());
        assert!(lines[3].trim().is_empty());
        assert!(lines[4].trim().is_empty());
        assert_eq!(lines[5], "endmodule");
        for (original, replacement) in source.lines().zip(lines) {
            assert_eq!(original.chars().count(), replacement.chars().count());
        }
    }

    #[test]
    fn semantic_parse_does_not_mask_directives_inside_block_comments() {
        // Arrange: erasing the apparent directive line would also erase the
        // closing delimiter and turn valid source into an unterminated comment.
        let source = "/* documentation\n`include \"not-real.svh\" */\nmodule top;\nendmodule\n";

        // Act
        let masked = mask_semantic_preprocessor_directives(source, &[]);

        // Assert
        assert_eq!(masked, source);
    }

    #[test]
    fn stale_open_buffer_semantic_result_uses_cached_snapshot() {
        // Arrange
        let fresh = SemanticTokens {
            result_id: None,
            data: vec![SemanticToken {
                delta_line: 7,
                delta_start: 0,
                length: 1,
                token_type: 0,
                token_modifiers_bitset: 0,
            }],
        };
        let cached = SemanticTokens {
            result_id: None,
            data: vec![SemanticToken {
                delta_line: 1,
                delta_start: 0,
                length: 1,
                token_type: 0,
                token_modifiers_bitset: 0,
            }],
        };

        // Act
        let selected = select_semantic_tokens(Some(Ok(fresh)), cached, false);

        // Assert
        assert_eq!(selected.data[0].delta_line, 1);
    }

    #[test]
    fn current_empty_open_buffer_semantic_result_is_authoritative() {
        // Arrange
        let cached = SemanticTokens {
            result_id: None,
            data: vec![SemanticToken {
                delta_line: 1,
                delta_start: 0,
                length: 1,
                token_type: 0,
                token_modifiers_bitset: 0,
            }],
        };

        // Act
        let selected = select_semantic_tokens(Some(Ok(empty_semantic_tokens())), cached, true);

        // Assert
        assert!(selected.data.is_empty());
    }

    #[test]
    fn client_init_parses_default_and_overridden_config_paths() {
        let number_one = serde_json::Number::from(1);
        let mut llg = LSPObject::new();
        llg.insert(
            "protocolVersion".to_owned(),
            LSPAny::Number(number_one.clone()),
        );
        let mut options = LSPObject::new();
        options.insert("llg".to_owned(), LSPAny::Object(llg));
        let (init, warnings) = Backend::parse_client_init(&LSPAny::Object(options));
        assert!(warnings.is_empty(), "warnings: {warnings:?}");
        assert_eq!(init.protocol_version, 1);
        assert!(init.config_files.is_empty());

        let mut config_file = LSPObject::new();
        config_file.insert(
            "workspaceUri".to_owned(),
            LSPAny::String("file:///project-a".to_owned()),
        );
        config_file.insert(
            "path".to_owned(),
            LSPAny::String("/project-a/custom.toml".to_owned()),
        );
        let mut llg = LSPObject::new();
        llg.insert("protocolVersion".to_owned(), LSPAny::Number(number_one));
        llg.insert(
            "configFiles".to_owned(),
            LSPAny::Array(vec![LSPAny::Object(config_file)]),
        );
        let mut options = LSPObject::new();
        options.insert("llg".to_owned(), LSPAny::Object(llg));
        let (init, warnings) = Backend::parse_client_init(&LSPAny::Object(options));
        assert!(warnings.is_empty(), "warnings: {warnings:?}");
        assert_eq!(init.config_files.len(), 1);
        assert_eq!(
            init.config_files[0].path,
            PathBuf::from("/project-a/custom.toml")
        );
    }

    #[test]
    fn watcher_options_cover_config_sources_and_include_deps() {
        let root = temp_root("watchers");
        std::fs::create_dir_all(&root).expect("create root");
        let config = config::default_config(&root);
        let descriptor = RootDescriptor::from_absolute(&root)
            .expect("descriptor")
            .with_config(Some(Arc::new(config)));
        let mut state = BackendState {
            roots: BTreeMap::new(),
            documents: BTreeMap::new(),
            dynamic_watched_files: true,
            watchers_registered: false,
            registered_watchers_digest: None,
            initialized: true,
            shutting_down: false,
            pending_logs: Vec::new(),
            merged: None,
            initial_pending: BTreeSet::new(),
            ready_sent: true,
            dep_dependents: BTreeMap::new(),
            published_shared: BTreeMap::new(),
            next_generation: 0,
        };
        state.roots.insert(
            root.clone(),
            RootState {
                descriptor,
                shadow: ShadowPaths::new(),
                discovered: vec![root.join("top.sv")],
                include_deps: BTreeSet::from([root.join("shared").join("defs.inc")]),
                lint_config: LintConfig::default(),
                config_errors: Vec::new(),
                config_warnings: Vec::new(),
                last_good: None,
                diagnostics: BTreeMap::new(),
                published_digests: BTreeMap::new(),
                all_diagnostics: BTreeMap::new(),
                generation: 0,
                pending_parent_id: None,
                scheduler: SchedulerState::default(),
                analysis_epoch: 0,
            },
        );
        let options = watcher_options_from_state(&state);
        let text = options.to_string();
        assert!(text.contains(&root.join(config::CONFIG_FILE).display().to_string()));
        assert!(text.contains("**/*.v"));
        assert!(text.contains("defs.inc"));
    }

    #[test]
    fn reload_keeps_last_valid_config_on_malformed_reload() {
        let _guard = SHADOW_TESTS_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let root = temp_root("reload");
        std::fs::create_dir_all(&root).expect("create root");
        let config_path = root.join(config::CONFIG_FILE);
        std::fs::write(
            &config_path,
            "schema_version = 1\n[sources]\ndirectories = [\".\"]\n",
        )
        .expect("write valid config");
        let (config, errors, _warnings) = Backend::load_root_config(&root, &config_path);
        assert!(errors.is_empty());
        let config = config.expect("valid config");

        let mut state = BackendState {
            roots: BTreeMap::new(),
            documents: BTreeMap::new(),
            dynamic_watched_files: false,
            watchers_registered: false,
            registered_watchers_digest: None,
            initialized: true,
            shutting_down: false,
            pending_logs: Vec::new(),
            merged: None,
            initial_pending: BTreeSet::new(),
            ready_sent: true,
            dep_dependents: BTreeMap::new(),
            published_shared: BTreeMap::new(),
            next_generation: 0,
        };
        state.roots.insert(
            root.clone(),
            RootState {
                descriptor: RootDescriptor::from_absolute(&root)
                    .expect("descriptor")
                    .with_config(Some(config.clone())),
                shadow: ShadowPaths::new(),
                discovered: Vec::new(),
                include_deps: BTreeSet::new(),
                lint_config: LintConfig::default(),
                config_errors: Vec::new(),
                config_warnings: Vec::new(),
                last_good: None,
                diagnostics: BTreeMap::new(),
                published_digests: BTreeMap::new(),
                all_diagnostics: BTreeMap::new(),
                generation: 0,
                pending_parent_id: None,
                scheduler: SchedulerState::default(),
                analysis_epoch: 0,
            },
        );
        // A malformed reload must retain the last-valid config.
        std::fs::write(&config_path, "schema_version = 1\n[sources\n").expect("write bad config");
        let (reloaded, reload_errors, reload_warnings) =
            Backend::load_root_config(&root, &config_path);
        assert!(
            !reload_errors.is_empty(),
            "malformed config must report errors"
        );
        let changed = reload_root_config_in_state(
            &mut state,
            &root,
            &config_path,
            reloaded,
            &reload_errors,
            &reload_warnings,
        );
        assert!(changed, "config state changed after malformed reload");
        let retained = state.roots.get(&root).expect("root");
        assert!(
            retained.descriptor.config.is_some(),
            "malformed config must retain the last-valid config"
        );
        assert!(
            !retained.config_errors.is_empty(),
            "malformed config must record load errors for TOML-URI diagnostics"
        );
        features::cleanup_process_shadow();
        let _ = std::fs::remove_dir_all(root);
    }

    fn llg_init_object(entries: Vec<(&str, LSPAny)>) -> LSPAny {
        let mut llg = LSPObject::new();
        for (key, value) in entries {
            llg.insert(key.to_owned(), value);
        }
        let mut options = LSPObject::new();
        options.insert("llg".to_owned(), LSPAny::Object(llg));
        LSPAny::Object(options)
    }

    #[test]
    fn client_init_warns_on_missing_or_invalid_protocol_version() {
        let (init, warnings) = Backend::parse_client_init(&llg_init_object(vec![(
            "configFiles",
            LSPAny::Array(vec![]),
        )]));
        assert_eq!(init.protocol_version, 0);
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("missing llg.protocolVersion")
                    && warning.contains("expected 1")),
            "missing protocolVersion must warn: {warnings:?}"
        );

        let (_, warnings) = Backend::parse_client_init(&llg_init_object(vec![(
            "protocolVersion",
            LSPAny::String("1".to_owned()),
        )]));
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("invalid llg.protocolVersion")),
            "non-numeric protocolVersion must warn: {warnings:?}"
        );

        // Unsupported numeric versions keep the existing warning behavior.
        let (_, warnings) = Backend::parse_client_init(&llg_init_object(vec![(
            "protocolVersion",
            LSPAny::Number(serde_json::Number::from(2)),
        )]));
        assert!(
            warnings.iter().any(
                |warning| warning.contains("unsupported llg protocolVersion 2")
                    && warning.contains("expected 1")
            ),
            "unsupported version must warn with expectation: {warnings:?}"
        );
    }

    #[test]
    fn dep_dependents_maps_every_dependent_root() {
        let root_a = PathBuf::from("/tmp/dep_map_a");
        let root_b = PathBuf::from("/tmp/dep_map_b");
        let shared = PathBuf::from("/tmp/shared/defs.svh");
        let only_a = PathBuf::from("/tmp/a/only.inc");
        let mut state = BackendState {
            roots: BTreeMap::new(),
            documents: BTreeMap::new(),
            dynamic_watched_files: false,
            watchers_registered: false,
            registered_watchers_digest: None,
            initialized: true,
            shutting_down: false,
            pending_logs: Vec::new(),
            merged: None,
            initial_pending: BTreeSet::new(),
            ready_sent: true,
            dep_dependents: BTreeMap::new(),
            published_shared: BTreeMap::new(),
            next_generation: 0,
        };
        state.roots.insert(
            root_a.clone(),
            RootState {
                descriptor: RootDescriptor::from_absolute(&root_a).expect("descriptor a"),
                shadow: ShadowPaths::new(),
                discovered: Vec::new(),
                include_deps: BTreeSet::from([shared.clone(), only_a.clone()]),
                lint_config: LintConfig::default(),
                config_errors: Vec::new(),
                config_warnings: Vec::new(),
                last_good: None,
                diagnostics: BTreeMap::new(),
                published_digests: BTreeMap::new(),
                all_diagnostics: BTreeMap::new(),
                generation: 0,
                pending_parent_id: None,
                scheduler: SchedulerState::default(),
                analysis_epoch: 0,
            },
        );
        state.roots.insert(
            root_b.clone(),
            RootState {
                descriptor: RootDescriptor::from_absolute(&root_b).expect("descriptor b"),
                shadow: ShadowPaths::new(),
                discovered: Vec::new(),
                include_deps: BTreeSet::from([shared.clone()]),
                lint_config: LintConfig::default(),
                config_errors: Vec::new(),
                config_warnings: Vec::new(),
                last_good: None,
                diagnostics: BTreeMap::new(),
                published_digests: BTreeMap::new(),
                all_diagnostics: BTreeMap::new(),
                generation: 0,
                pending_parent_id: None,
                scheduler: SchedulerState::default(),
                analysis_epoch: 0,
            },
        );
        rebuild_dep_dependents(&mut state);

        assert_eq!(
            state.dep_dependents.get(&shared),
            Some(&BTreeSet::from([root_a.clone(), root_b.clone()])),
            "a dep shared by two roots must map to BOTH roots"
        );
        assert_eq!(
            state.dep_dependents.get(&only_a),
            Some(&BTreeSet::from([root_a.clone()])),
            "an exclusive dep maps to its own root only"
        );
        assert!(!state
            .dep_dependents
            .contains_key(Path::new("/tmp/untracked.sv")));
    }

    fn labeled_diagnostic(range_start: (u32, u32), severity: u8, message: &str) -> Diagnostic {
        Diagnostic {
            range: Range {
                start: Position::new(range_start.0, range_start.1),
                end: Position::new(range_start.0, range_start.1 + 5),
            },
            severity: match severity {
                1 => Some(DiagnosticSeverity::ERROR),
                2 => Some(DiagnosticSeverity::WARNING),
                3 => Some(DiagnosticSeverity::INFORMATION),
                _ => None,
            },
            code: Some(NumberOrString::String("width-mismatch".to_owned())),
            code_description: None,
            source: Some("llg-lint".to_owned()),
            message: message.to_owned(),
            related_information: None,
            tags: None,
            data: None,
        }
    }

    #[test]
    fn shared_diagnostics_dedupe_exact_and_label_conflicts() {
        let names = BTreeMap::from([
            (PathBuf::from("/w/root-b"), "root-b".to_owned()),
            (PathBuf::from("/w/root-a"), "root-a".to_owned()),
        ]);
        let owner_key = PathBuf::from("/w/root-b");
        let non_owner_key = PathBuf::from("/w/root-a");

        // Identical finding from both roots: appears exactly ONCE, unlabeled.
        // Differing severity at the same location: both survive, the
        // non-owner copy is labeled [root-a].
        let entries = vec![
            (
                owner_key.clone(),
                true,
                labeled_diagnostic((4, 2), 2, "width mismatch"),
            ),
            (
                non_owner_key.clone(),
                false,
                labeled_diagnostic((4, 2), 2, "width mismatch"),
            ),
            (
                owner_key,
                true,
                labeled_diagnostic((9, 0), 2, "width mismatch"),
            ),
            (
                non_owner_key,
                false,
                labeled_diagnostic((9, 0), 3, "width mismatch"),
            ),
        ];
        let merged = merge_shared_file_diagnostics(entries, &names);
        let at_4: Vec<_> = merged
            .iter()
            .filter(|diagnostic| diagnostic.range.start.line == 4)
            .collect();
        assert_eq!(at_4.len(), 1, "exact duplicates must collapse: {merged:?}");
        assert_eq!(
            at_4[0].message, "width mismatch",
            "owner copies stay unlabeled"
        );
        let at_9: Vec<_> = merged
            .iter()
            .filter(|diagnostic| diagnostic.range.start.line == 9)
            .collect();
        assert_eq!(
            at_9.len(),
            2,
            "conflicting findings both survive: {merged:?}"
        );
        assert!(
            at_9.iter()
                .any(|diagnostic| diagnostic.message == "width mismatch"),
            "owner copy unlabeled: {merged:?}"
        );
        assert!(
            at_9.iter()
                .any(|diagnostic| diagnostic.message == "width mismatch [root-a]"),
            "non-owner copy must carry the documented [root-name] label: {merged:?}"
        );
    }

    #[test]
    fn shared_hover_is_annotated_with_root_name() {
        let mut hover = Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: "```systemverilog\nlocalparam int W = 8\n```".to_owned(),
            }),
            range: None,
        };
        annotate_shared_hover(&mut hover, "root-a");
        let HoverContents::Markup(markup) = &hover.contents else {
            panic!("markup hover expected");
        };
        assert!(markup.value.contains("[root-a] parameter/define values"));
        assert!(markup.value.starts_with("```systemverilog"));
    }

    fn two_root_state_with_shared_file(
        shared: &Path,
        diag_a: Diagnostic,
        diag_b: Diagnostic,
    ) -> BackendState {
        let root_a = PathBuf::from("/tmp/llg_agg_a");
        let root_b = PathBuf::from("/tmp/llg_agg_b");
        let descriptor = |path: &Path| RootDescriptor::from_absolute(path).expect("descriptor");
        let mut state = BackendState {
            roots: BTreeMap::new(),
            documents: BTreeMap::new(),
            dynamic_watched_files: false,
            watchers_registered: false,
            registered_watchers_digest: None,
            initialized: true,
            shutting_down: false,
            pending_logs: Vec::new(),
            merged: None,
            initial_pending: BTreeSet::new(),
            ready_sent: true,
            dep_dependents: BTreeMap::new(),
            published_shared: BTreeMap::new(),
            next_generation: 0,
        };
        state.roots.insert(
            root_a.clone(),
            RootState {
                descriptor: descriptor(&root_a),
                shadow: ShadowPaths::new(),
                discovered: Vec::new(),
                include_deps: BTreeSet::from([shared.to_path_buf()]),
                lint_config: LintConfig::default(),
                config_errors: Vec::new(),
                config_warnings: Vec::new(),
                last_good: None,
                diagnostics: BTreeMap::new(),
                published_digests: BTreeMap::new(),
                all_diagnostics: BTreeMap::from([(shared.to_path_buf(), vec![diag_a])]),
                generation: 1,
                pending_parent_id: None,
                scheduler: SchedulerState::default(),
                analysis_epoch: 0,
            },
        );
        state.roots.insert(
            root_b.clone(),
            RootState {
                descriptor: descriptor(&root_b),
                shadow: ShadowPaths::new(),
                discovered: Vec::new(),
                include_deps: BTreeSet::from([shared.to_path_buf()]),
                lint_config: LintConfig::default(),
                config_errors: Vec::new(),
                config_warnings: Vec::new(),
                last_good: None,
                diagnostics: BTreeMap::new(),
                published_digests: BTreeMap::new(),
                all_diagnostics: BTreeMap::from([(shared.to_path_buf(), vec![diag_b])]),
                generation: 1,
                pending_parent_id: None,
                scheduler: SchedulerState::default(),
                analysis_epoch: 0,
            },
        );
        rebuild_dep_dependents(&mut state);
        state
    }

    /// Regression (review P1-2): a changed underlying state must never stay
    /// hidden behind an unchanged/stale digest.  When a folder removal leaves
    /// one surviving tracker for a formerly shared file, that survivor's
    /// diagnostics are republished instead of the stale union (or a blind
    /// clear).
    #[test]
    fn aggregate_republishes_surviving_root_after_folder_removal() {
        let shared = PathBuf::from("/tmp/llg_agg_a/src/shared.svh");
        let shared_uri = Url::from_file_path(&shared).expect("shared uri");
        let distinct_b = labeled_diagnostic((9, 0), 2, "width mismatch b");
        let mut state = two_root_state_with_shared_file(
            &shared,
            labeled_diagnostic((4, 2), 2, "width mismatch"),
            distinct_b,
        );

        // First pass publishes the multi-root union.
        let first = aggregate_shared_publications(&mut state, &BTreeSet::new());
        let union = first
            .iter()
            .find(|(uri, _)| uri == &shared_uri)
            .map(|(_, diagnostics)| diagnostics)
            .expect("union publication");
        assert_eq!(union.len(), 2, "distinct findings both survive: {union:?}");

        // Folder removal: root-b disappears; only root-a still tracks the
        // file.  The union is stale now and must be replaced by the
        // survivor's view.
        state.roots.remove(&PathBuf::from("/tmp/llg_agg_b"));
        rebuild_dep_dependents(&mut state);
        let second = aggregate_shared_publications(&mut state, &BTreeSet::new());
        let survivor: Vec<_> = second
            .iter()
            .filter(|(uri, _)| uri == &shared_uri)
            .collect();
        assert_eq!(survivor.len(), 1, "exactly one republication: {second:?}");
        assert!(!survivor[0].1.is_empty(), "the surviving root's diagnostics must be republished instead of clearing the stale union");
        let stored = diagnostics_digest(&survivor[0].1);
        assert_eq!(
            state.published_shared.get(&shared_uri).map(String::as_str),
            Some(stored.as_str()),
            "publication bookkeeping must track the survivor's digest"
        );
    }

    /// Regression (review P1-2): when every tracking root's diagnostics
    /// vanish (fixed or cleared by a reload), the previously published union
    /// must be cleared at the client rather than suppressed behind its old
    /// digest.
    #[test]
    fn aggregate_clears_stale_union_when_diagnostics_vanish_everywhere() {
        let shared = PathBuf::from("/tmp/llg_agg_a/src/shared.svh");
        let shared_uri = Url::from_file_path(&shared).expect("shared uri");
        let mut state = two_root_state_with_shared_file(
            &shared,
            labeled_diagnostic((4, 2), 2, "width mismatch"),
            labeled_diagnostic((9, 0), 2, "width mismatch b"),
        );

        let first = aggregate_shared_publications(&mut state, &BTreeSet::new());
        assert!(
            first
                .iter()
                .any(|(uri, diagnostics)| uri == &shared_uri && !diagnostics.is_empty()),
            "precondition: union published"
        );

        // A later valid commit produced no findings anywhere; the file is
        // still tracked by both roots but closed in the editor.
        for root in state.roots.values_mut() {
            root.all_diagnostics.clear();
        }
        let second = aggregate_shared_publications(&mut state, &BTreeSet::new());
        let cleared = second
            .iter()
            .find(|(uri, _)| uri == &shared_uri)
            .map(|(_, diagnostics)| diagnostics)
            .expect("stale union must be republished as empty");
        assert!(
            cleared.is_empty(),
            "cleared diagnostics must reach the client: {cleard:?}",
            cleard = cleared
        );
    }

    #[test]
    fn source_relative_path_uses_deepest_source_directory_base() {
        let root = temp_root("source_relative");
        std::fs::create_dir_all(&root).expect("create root");
        let mut config = config::default_config(&root);
        config.sources.directories = vec![root.clone(), root.join("src")];
        let nested = root.join("src").join("nested.sv");

        // Discovery evaluated globs relative to `src/`, so the filter check
        // must see `nested.sv`, not `<root>-relative` components.
        assert_eq!(
            Backend::source_relative_path(&config, &nested),
            Some(PathBuf::from("nested.sv"))
        );
        let outside = root.join("other.sv");
        assert_eq!(
            Backend::source_relative_path(&config, &outside),
            Some(PathBuf::from("other.sv")),
            "files under shallower source dirs resolve against them"
        );
        assert_eq!(
            Backend::source_relative_path(&config, &temp_root("elsewhere")),
            None
        );
    }

    /// The exact cleanup routine `Backend::shutdown` runs must remove the
    /// whole process shadow base — staged buffers AND the analysis scratch
    /// directory included (review A1).
    #[test]
    fn shutdown_cleanup_removes_process_shadow_base() {
        let _guard = SHADOW_TESTS_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let root = temp_root("shutdown_cleanup");
        std::fs::create_dir_all(&root).expect("create root");
        let shadow = ShadowPaths::new();
        let real = root.join("top.sv");
        shadow
            .stage(&real, "module top; endmodule\n")
            .expect("stage buffer");
        // The analysis parks Surelog artifacts here; recreate it like a job
        // would so cleanup has more than staged files to remove.
        std::fs::create_dir_all(features::analysis_scratch_dir()).expect("scratch dir");

        let base = features::process_shadow_base();
        assert!(base.exists(), "precondition: shadow base exists");
        cleanup_shadow_state_blocking(vec![shadow]);
        assert!(
            !base.exists(),
            "shutdown cleanup must remove the whole process shadow base"
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
