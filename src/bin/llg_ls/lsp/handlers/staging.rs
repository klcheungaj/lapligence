//! Private source staging, bounded input admission, and include isolation.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum InputSizeLimitKind {
    PerFile,
    Total,
    Unreadable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct InputSizeLimit {
    pub(super) path: PathBuf,
    pub(super) measured_bytes: u64,
    pub(super) configured_limit: u64,
    pub(super) kind: InputSizeLimitKind,
    pub(super) total_bytes: Option<u64>,
}

impl InputSizeLimit {
    pub(super) fn message(&self) -> String {
        let message = match self.kind {
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
        };
        if self.kind == InputSizeLimitKind::Unreadable {
            message
        } else {
            format!("{message}. {}", config::SOURCE_SIZE_LIMIT_GUIDANCE)
        }
    }

    pub(super) fn log_rejection(&self) {
        if self.kind != InputSizeLimitKind::Unreadable {
            crate::llg_error!(
                "event=analysis.input_limit outcome=rejected kind={:?} path={} measured_bytes={} total_bytes={:?} configured_limit={} advice={}",
                self.kind,
                crate::logging::bounded_field(&self.path.to_string_lossy()),
                self.measured_bytes, self.total_bytes, self.configured_limit,
                config::SOURCE_SIZE_LIMIT_GUIDANCE
            );
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct InputBudgetFailure {
    pub(super) limit: InputSizeLimit,
    /// Resolved dependencies seen before (and including) the offending input.
    /// Keeping these paths lets a failed job preserve the last-good watch set.
    pub(super) include_deps: BTreeSet<PathBuf>,
}

/// Exact text snapshots captured during input-budget admission.  The lexical
/// path and canonical identity are both retained so a file can be staged from
/// the admitted bytes even if its on-disk spelling or symlink changes before
/// staging begins.
#[derive(Debug, Default)]
pub(super) struct InputSnapshots {
    pub(super) by_path: BTreeMap<PathBuf, Arc<String>>,
    pub(super) by_identity: BTreeMap<PathBuf, Arc<String>>,
}

impl InputSnapshots {
    pub(super) fn insert(&mut self, path: &Path, text: SharedText) {
        self.by_path.insert(path.to_path_buf(), Arc::clone(&text));
        self.by_identity.insert(input_identity(path), text);
    }

    /// The shared admitted text of `path`, by spelling or identity.
    pub(super) fn shared(&self, path: &Path) -> Option<&SharedText> {
        self.by_path
            .get(path)
            .or_else(|| self.by_identity.get(&input_identity(path)))
    }

    pub(super) fn text(&self, path: &Path) -> Option<&str> {
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
pub(super) struct InputBudget {
    pub(super) snapshots: InputSnapshots,
}

pub(super) fn open_input_size_limit(
    path: &Path,
    text: &str,
    max_file_bytes: u64,
) -> Option<InputSizeLimit> {
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
pub(super) fn open_token_size_limit(
    open_document: Option<&OpenTokenDocument>,
    max_file_bytes: Option<u64>,
) -> Option<InputSizeLimit> {
    let (Some(document), Some(max_file_bytes)) = (open_document, max_file_bytes) else {
        return None;
    };
    open_input_size_limit(&document.real, document.text.as_str(), max_file_bytes)
}

/// The inputs of one isolated open-buffer token parse, captured under the
/// backend state lock: the real path, the admitted buffer revision, the
/// effective configured defines and the include context of the root's
/// committed analysis together with that analysis' epoch.
#[derive(Clone)]
pub(super) struct OpenTokenDocument {
    pub(super) real: PathBuf,
    pub(super) text: SharedText,
    pub(super) defines: Vec<String>,
    pub(super) includes: Arc<features::IncludeContext>,
    pub(super) epoch: u64,
}

/// Bound the number of open-document parses that can be retained in the
/// single-flight table.  Entries live while the detached coordinator owns the
/// parse and through cache publication, then are removed on success/failure;
/// absent keys are refused and degraded to the fallback while the table is
/// saturated.
pub(super) const OPEN_TOKEN_FLIGHT_CAPACITY: usize = 32;
type OpenTokenResult = std::result::Result<SemanticTokens, String>;
pub(super) const STALE_OPEN_TOKEN_ERROR: &str = "semantic-token request is no longer current";

/// Check a captured open-buffer revision without retaining the backend state
/// lock across any staging or frontend work.  The request can only use the
/// isolated path while the same text is still the document's current
/// snapshot; shutdown also makes an otherwise matching revision ineligible.
pub(super) fn open_document_is_current(
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

pub(super) struct OpenTokenFlight {
    pub(super) notify: Notify,
    pub(super) result: Mutex<Option<OpenTokenResult>>,
}

impl OpenTokenFlight {
    pub(super) fn new() -> Self {
        Self {
            notify: Notify::new(),
            result: Mutex::new(None),
        }
    }

    pub(super) fn result(&self) -> Option<OpenTokenResult> {
        self.result
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    pub(super) async fn wait(&self) -> OpenTokenResult {
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

pub(super) struct OpenTokenFlightRegistry {
    pub(super) flights: Mutex<HashMap<String, Arc<OpenTokenFlight>>>,
}

pub(super) enum OpenTokenFlightLease {
    Leader(OpenTokenFlightLeader),
    Follower(Arc<OpenTokenFlight>),
    /// The bounded registry is saturated.  This request is refused a flight
    /// and must use the already-available fallback instead of starting
    /// another blocking parse.
    Saturated,
}

pub(super) struct OpenTokenFlightLeader {
    pub(super) registry: Arc<OpenTokenFlightRegistry>,
    pub(super) key: String,
    pub(super) flight: Arc<OpenTokenFlight>,
    pub(super) completed: bool,
    pub(super) detached: bool,
}

/// Completion owner for a detached open-document parse.  It is deliberately
/// independent of the request future: cancelling the request only drops its
/// waiter, while this owner remains in the blocking task until the result has
/// been published and all followers have been woken.
pub(super) struct OpenTokenFlightCoordinator {
    pub(super) registry: Arc<OpenTokenFlightRegistry>,
    pub(super) key: String,
    pub(super) flight: Arc<OpenTokenFlight>,
    pub(super) completed: bool,
}

impl OpenTokenFlightRegistry {
    pub(super) fn new() -> Self {
        Self {
            flights: Mutex::new(HashMap::new()),
        }
    }

    pub(super) fn acquire(self: &Arc<Self>, key: String) -> OpenTokenFlightLease {
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

    pub(super) fn remove(&self, key: &str, flight: &Arc<OpenTokenFlight>) {
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

    pub(super) fn len(&self) -> usize {
        self.flights
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .len()
    }
}

impl OpenTokenFlightLeader {
    pub(super) fn finish(&mut self, result: OpenTokenResult) {
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
    pub(super) fn detach(&mut self) -> OpenTokenFlightCoordinator {
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
    pub(super) fn finish(&mut self, result: OpenTokenResult) {
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

/// Per-root mapping between real files and private staged copies.
#[derive(Debug, Clone)]
pub(super) struct ShadowPaths {
    pub(super) base: PathBuf,
    pub(super) staged: Arc<Mutex<BTreeSet<PathBuf>>>,
}
impl ShadowPaths {
    pub fn new() -> Self {
        Self {
            base: features::process_shadow_base(),
            staged: Arc::new(Mutex::new(BTreeSet::new())),
        }
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

/// One request-local isolated parse guarded by a unique private directory so
/// cleanup stays isolated from project-analysis shadow state. Slang receives
/// the buffer text unchanged: the preprocessor runs with the configured
/// defines and the committed include context, so directives, macro usages and
/// inactive branches are handled exactly as in the project compile.
pub(super) struct SemanticStage {
    pub(super) directory: PathBuf,
}

impl SemanticStage {
    pub(super) fn new() -> std::io::Result<Self> {
        let id = NEXT_SEMANTIC_STAGE_ID.fetch_add(1, Ordering::Relaxed);
        let directory = features::process_shadow_base()
            .join("semantic")
            .join(format!("{}-{id}", std::process::id()));
        std::fs::create_dir_all(&directory)?;
        Ok(Self { directory })
    }
}

impl Drop for SemanticStage {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

pub(super) fn empty_semantic_tokens() -> SemanticTokens {
    SemanticTokens {
        result_id: None,
        data: Vec::new(),
    }
}

pub(super) fn cached_semantic_tokens(
    analysis: Option<&Analysis>,
    paths: &[String],
    allow_filename_fallback: bool,
) -> SemanticTokens {
    let Some(analysis) = analysis else {
        return empty_semantic_tokens();
    };

    // Prefer exact/canonical token paths.  Once an exact path entry exists,
    // an empty stream is authoritative for that file and must not fall
    // through to an unrelated same-basename file.  A file with syntax errors
    // keeps the tokens the frontend recovered (per-token degradation).
    let mut found_exact = false;
    for path in paths {
        for file_tokens in &analysis.tokens {
            if !cached_semantic_file_matches(&file_tokens.path, path) {
                continue;
            }
            found_exact = true;
            let tokens = crate::semantic_tokens::encode(&file_tokens.nodes);
            if !tokens.data.is_empty() {
                return tokens;
            }
        }
    }
    if found_exact || !allow_filename_fallback {
        return empty_semantic_tokens();
    }

    // Preserve the historical basename lookup for closed/project requests,
    // where Slang can report a compatible path spelling.  It is disabled
    // for open buffers so two files with the same basename cannot contaminate
    // an isolated request's cache fallback.
    for path in paths {
        let Some(file_name) = Path::new(path).file_name() else {
            continue;
        };
        for file_tokens in &analysis.tokens {
            if Path::new(&file_tokens.path).file_name() != Some(file_name) {
                continue;
            }
            let tokens = crate::semantic_tokens::encode(&file_tokens.nodes);
            if !tokens.data.is_empty() {
                return tokens;
            }
        }
    }
    empty_semantic_tokens()
}

pub(super) fn cached_semantic_file_matches(left: &str, right: &str) -> bool {
    if Path::new(left) == Path::new(right) {
        return true;
    }
    match (
        llg::ffi::platform::canonicalize(left),
        llg::ffi::platform::canonicalize(right),
    ) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

pub(super) fn compute_semantic_tokens(
    analysis: Option<Arc<Analysis>>,
    paths: Vec<String>,
    open_document: Option<OpenTokenDocument>,
    current: impl Fn() -> bool,
    parent_id: Option<u64>,
) -> (Option<OpenTokenResult>, SemanticTokens) {
    let cached = cached_semantic_tokens(analysis.as_deref(), &paths, open_document.is_none());
    let fresh = open_document.map(|document| {
        let result = compute_open_document_semantic_tokens(document, current, parent_id);
        if let Err(error) = &result {
            crate::llg_debug!(
                "semantic tokens: isolated collection failed before producing tokens: {error}"
            );
        }
        result
    });
    (fresh, cached)
}

pub(super) fn compute_open_document_semantic_tokens(
    document: OpenTokenDocument,
    current: impl Fn() -> bool,
    parent_id: Option<u64>,
) -> OpenTokenResult {
    open_document_semantic_tokens_if_current(
        &document.real,
        &document.text,
        &document.defines,
        &document.includes,
        current,
        parent_id,
    )
}

/// Memoization key of one open-buffer isolated token stream: document URI,
/// buffer text digest, effective configured defines and the epoch of the
/// committed analysis whose include context the parse reads. These are
/// exactly the inputs of the request-local run, so any edit, defines
/// hot-reload or snapshot replacement produces a different key while an
/// unchanged repeat hits.
pub(super) fn open_token_cache_key(
    uri: &str,
    text: &str,
    defines: &[String],
    epoch: u64,
) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    for define in defines {
        define.hash(&mut hasher);
    }
    format!("open-tokens|{uri}|{epoch}|{:016x}", hasher.finish())
}

pub(super) fn open_document_semantic_tokens_if_current(
    real: &Path,
    text: &str,
    defines: &[String],
    includes: &features::IncludeContext,
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
    // deleting the process shadow base while Slang reads this copy.
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
    // Held until the parse and scratch cleanup finish; dropping it removes the
    // private directory.
    let _stage = match SemanticStage::new() {
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
    let result = real
        .to_str()
        .ok_or_else(|| "semantic source path is not UTF-8".to_owned())
        .and_then(|path| {
            // A revision can change while the request-local stage is being
            // written.  Check again immediately before entering Slang so a
            // stale buffer cannot start the expensive parse.
            if !current() {
                crate::llg_debug!(
                    "event=semantic_tokens.open_parse.frontend outcome=stale-before-slang file={} elapsed_us={}",
                    real.display(),
                    started.elapsed().as_micros()
                );
                Err(STALE_OPEN_TOKEN_ERROR.to_owned())
            } else {
                // Slang consumes the admitted buffer and never reads this
                // path. Preserve the real document identity in its snapshot,
                // so includes resolve beside the real file exactly as in the
                // project compile; the private stage only serializes cleanup.
                features::semantic_tokens_for_open_document_with_parent(
                    path,
                    defines,
                    Some(text),
                    includes,
                    parent_id,
                )
                    .map_err(|error| error.to_string())
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

pub(super) fn select_semantic_tokens(
    fresh: Option<std::result::Result<SemanticTokens, String>>,
    cached: SemanticTokens,
    buffer_is_current: bool,
) -> SemanticTokens {
    // A successful isolated parse is authoritative for the current buffer,
    // including the partial stream of a buffer with syntax errors and an
    // empty stream.  The cache is used only when staging/session work (or
    // the blocking task) fails before producing a result.
    if buffer_is_current {
        fresh.and_then(std::result::Result::ok).unwrap_or(cached)
    } else {
        cached
    }
}

/// Whether a full-text `didChange` actually carries new content.  Identical
/// full-text changes (same document text re-sent, e.g. by editor save/format
/// flows) must not reschedule the root: nothing the analysis reads changed.
pub(super) fn did_change_is_new_content(current: Option<&str>, next: &str) -> bool {
    match current {
        Some(text) => text != next,
        None => true,
    }
}

pub(super) fn shadow_staging_lock() -> &'static Mutex<()> {
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

/// Perform the final shadow sweep and terminate while still holding the
/// staging lock. Keeping the lock through process exit closes the small race
/// where a queued analysis could recreate the shadow base after cleanup but
/// before [`std::process::exit`].
pub(crate) fn emergency_shadow_cleanup_and_exit(code: i32) -> ! {
    let _staging = shadow_staging_lock()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    features::cleanup_process_shadow();
    std::process::exit(code)
}

/// Remove every root's staged files plus the entire process shadow base.
///
/// The staging lock is acquired INSIDE this function (not by callers) so it
/// can run on `tokio::task::spawn_blocking` while keeping its ordering
/// guarantee against compile jobs.
pub(super) fn cleanup_shadow_state_blocking(shadows: Vec<ShadowPaths>) {
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
/// Private mirrors and transient analysis artifacts accumulate there across
/// jobs; jobs are serialized behind the shadow-staging lock, so removing the
/// contents between jobs cannot race a running analysis.
pub(super) fn clean_analysis_scratch() {
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

/// Collect literal include targets while ignoring comments.
pub(super) fn explicit_include_targets(source: &str) -> Vec<String> {
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
pub(super) fn canonical_include_target(
    allowed: &[PathBuf],
    resolved: &Path,
) -> std::result::Result<Option<PathBuf>, ()> {
    if !is_under_any(allowed, resolved) {
        return Err(());
    }

    let canonical_allowed: Vec<PathBuf> = allowed
        .iter()
        .map(|dir| llg::ffi::platform::canonicalize(dir).unwrap_or_else(|_| dir.clone()))
        .collect();
    match std::fs::symlink_metadata(resolved) {
        Ok(_) => {
            let canonical = llg::ffi::platform::canonicalize(resolved).map_err(|_| ())?;
            if is_under_any(&canonical_allowed, &canonical) {
                Ok(Some(canonical))
            } else {
                Err(())
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            // A missing include is left to Slang to diagnose, but an
            // existing symlinked parent must not hide an outside resolution.
            let mut parent = resolved.parent();
            while let Some(candidate) = parent {
                if let Ok(canonical) = llg::ffi::platform::canonicalize(candidate) {
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

pub(super) fn is_under_any(dirs: &[PathBuf], path: &Path) -> bool {
    dirs.iter()
        .any(|dir| workspace::root_relative_path(dir, path).is_some())
}

pub(super) fn open_document_value<'a>(
    path: &Path,
    open_documents: &'a OpenDocuments,
) -> Option<&'a SharedText> {
    open_documents.get(path).or_else(|| {
        llg::ffi::platform::canonicalize(path)
            .ok()
            .and_then(|canonical| open_documents.get(&canonical))
    })
}

pub(super) fn open_document_text<'a>(
    path: &Path,
    open_documents: &'a OpenDocuments,
) -> Option<&'a str> {
    open_document_value(path, open_documents).map(|text| text.as_str())
}

/// Return the byte length that can be measured without reading an input.
/// Open UTF-8 buffers are authoritative; closed files use metadata so an
/// over-limit input is rejected before any unbounded source read or staging.
pub(super) fn measured_input_bytes(path: &Path, open_documents: &OpenDocuments) -> Option<u64> {
    open_document_text(path, open_documents)
        .map(|text| text.len() as u64)
        .or_else(|| std::fs::metadata(path).ok().map(|metadata| metadata.len()))
}

pub(super) fn input_identity(path: &Path) -> PathBuf {
    llg::ffi::platform::canonicalize(path)
        .ok()
        .or_else(|| workspace::normalize_absolute_path(path))
        .unwrap_or_else(|| path.to_path_buf())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum IncludeResolutionError {
    Unauthorized,
    SnapshotUnavailable,
}

/// Resolve a literal include using the same search order as the `-I` arguments
/// handed to Slang: an absolute target is used as-is; a relative target is
/// tried beside the including file first, then in each configured source or
/// explicit include directory.  Each candidate goes through the existing
/// lexical and symlink containment policy before it is accepted.
pub(super) fn resolve_include_target(
    allowed: &[PathBuf],
    source: &Path,
    target: &str,
    open_documents: &OpenDocuments,
    snapshots: &InputSnapshots,
) -> std::result::Result<Option<PathBuf>, ()> {
    resolve_include_target_with_policy(allowed, source, target, open_documents, snapshots, false)
        .map_err(|_| ())
}

pub(super) fn resolve_admitted_include_target(
    allowed: &[PathBuf],
    source: &Path,
    target: &str,
    open_documents: &OpenDocuments,
    snapshots: &InputSnapshots,
) -> std::result::Result<Option<PathBuf>, IncludeResolutionError> {
    resolve_include_target_with_policy(allowed, source, target, open_documents, snapshots, true)
}

pub(super) fn resolve_include_target_with_policy(
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
pub(super) fn read_closed_input_snapshot(
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
/// snapshot before they reach staging or Slang.
pub(super) fn prepared_input_text<'a>(
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
/// rejected before it can reach Slang on its live real path.
pub(super) fn enforce_input_budget(
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

pub(super) fn compile_result_files(paths: &[PathBuf]) -> Vec<(PathBuf, String)> {
    paths
        .iter()
        .map(|real| (real.clone(), real.to_string_lossy().into_owned()))
        .collect()
}

pub(super) fn attach_fileless_diagnostics(analysis: &mut Analysis, compiled_path: &str) {
    for diagnostic in &mut analysis.diagnostics {
        if diagnostic.file.is_none() {
            diagnostic.file = Some(compiled_path.to_owned());
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct IncludeStageFailure {
    pub(super) path: PathBuf,
    pub(super) message: String,
    pub(super) dependencies: BTreeSet<PathBuf>,
}

/// Stage every resolved include dependency (from disk or an open buffer) into
/// the shadow tree so relative includes and unsaved headers resolve.  Returns
/// the set of resolved include dependency real paths (any extension).
pub(super) fn stage_include_tree(
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
        let identity = llg::ffi::platform::canonicalize(&source).unwrap_or_else(|_| source.clone());
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
pub(super) fn preflight_include_isolation(
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
        let identity = llg::ffi::platform::canonicalize(&source).unwrap_or_else(|_| source.clone());
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
