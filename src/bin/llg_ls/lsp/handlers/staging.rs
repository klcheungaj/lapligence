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

/// One request-local open-buffer transformation guarded by a unique private
/// directory so cleanup stays isolated from project-analysis shadow state.
pub(super) struct SemanticStage {
    pub(super) directory: PathBuf,
    pub(super) source: String,
}

impl SemanticStage {
    pub(super) fn new(_real: &Path, text: &str, defines: &[String]) -> std::io::Result<Self> {
        let id = NEXT_SEMANTIC_STAGE_ID.fetch_add(1, Ordering::Relaxed);
        let directory = features::process_shadow_base()
            .join("semantic")
            .join(format!("{}-{id}", std::process::id()));
        std::fs::create_dir_all(&directory)?;
        Ok(Self {
            directory,
            source: mask_semantic_preprocessor_directives(text, defines),
        })
    }
}

/// Replace standalone compiler-directive lines with spaces while preserving
/// every newline and UTF-16 source column. Slang's `-parseonly` mode bypasses
/// preprocessing and otherwise diagnoses valid directives such as
/// `` `include`` as parser syntax errors. Semantic-token collection is
/// intentionally source-local, so masking the directives both avoids that
/// false error and guarantees that includes are not consumed.
pub(super) fn mask_semantic_preprocessor_directives(text: &str, defines: &[String]) -> String {
    let inactive = crate::inactive_ranges::inactive_line_ranges(text, defines);
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
        let starts_directive = semantic_directive_outside_comment(body, &mut in_block_comment);
        let directive = continuation || starts_directive;
        continuation = directive && body.trim_end().ends_with('\\');
        if directive || line_is_inactive {
            output.push_str(&mask_semantic_source_line(body));
        } else {
            output.push_str(body);
        }
        if line.ends_with('\n') {
            output.push('\n');
        }
    }
    output
}

/// Mask one source line without changing its line ending or LSP column
/// accounting.  Source positions use UTF-16 code units, so a supplementary
/// character becomes two spaces; tabs remain tabs just as they did before
/// masking.  `\n` is normally handled by the caller, but preserving it here
/// keeps this helper safe for direct use too.
pub(super) fn mask_semantic_source_line(line: &str) -> String {
    let mut masked = String::with_capacity(line.len());
    for ch in line.chars() {
        match ch {
            '\r' | '\n' | '\t' => masked.push(ch),
            _ => {
                for _ in 0..ch.len_utf16() {
                    masked.push(' ');
                }
            }
        }
    }
    masked
}

/// Whether the first non-whitespace, non-comment token on this line is one
/// of the directives masked for isolated semantic parsing. The block-comment
/// state crosses lines, and quoted/comment text never starts a directive.
pub(super) fn semantic_directive_outside_comment(line: &str, in_block_comment: &mut bool) -> bool {
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
            let keyword = &line[start..end];
            directive = SEMANTIC_DIRECTIVE_KEYWORDS.contains(&keyword)
                && !SEMANTIC_PREDEFINED_EXPRESSION_MACROS.contains(&keyword);
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

    // Check every exact/canonical alias before looking at any token list.  A
    // project analysis for an open buffer normally contains the shadow path,
    // while its diagnostic may carry the real path (or vice versa).  Looking
    // up one candidate at a time would let the other alias, or a same-named
    // file found by the compatibility fallback, resurrect partial tokens.
    if paths
        .iter()
        .any(|path| cached_semantic_syntax_diagnostic_matches(analysis, path))
    {
        return empty_semantic_tokens();
    }

    // Prefer exact/canonical token paths.  Once an exact path entry exists,
    // an empty stream is authoritative for that file and must not fall
    // through to an unrelated same-basename file.
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
    // an isolated request's cache fallback.  Do not serve a partial stream
    // from a fallback file that is itself syntax-invalid.
    for path in paths {
        let Some(file_name) = Path::new(path).file_name() else {
            continue;
        };
        for file_tokens in &analysis.tokens {
            if Path::new(&file_tokens.path).file_name() != Some(file_name)
                || cached_semantic_syntax_diagnostic_matches(analysis, &file_tokens.path)
            {
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
    match (std::fs::canonicalize(left), std::fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

pub(super) fn cached_semantic_syntax_diagnostic_matches(analysis: &Analysis, path: &str) -> bool {
    analysis.diagnostics.iter().any(|diagnostic| {
        matches!(diagnostic.severity, llg::core::compile::Severity::Syntax)
            && diagnostic
                .file
                .as_deref()
                .is_some_and(|diagnostic_file| cached_semantic_file_matches(diagnostic_file, path))
    })
}

pub(super) fn compute_semantic_tokens(
    analysis: Option<Arc<Analysis>>,
    paths: Vec<String>,
    open_document: Option<OpenTokenDocument>,
    current: impl Fn() -> bool,
    parent_id: Option<u64>,
) -> (Option<OpenTokenResult>, SemanticTokens) {
    let cached = cached_semantic_tokens(analysis.as_deref(), &paths, open_document.is_none());
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

pub(super) fn compute_open_document_semantic_tokens(
    real: PathBuf,
    text: SharedText,
    defines: Vec<String>,
    current: impl Fn() -> bool,
    parent_id: Option<u64>,
) -> OpenTokenResult {
    open_document_semantic_tokens_if_current(&real, &text, &defines, current, parent_id)
}

/// Memoization key of one open-buffer isolated token stream: document URI,
/// buffer text digest and effective configured defines. These are exactly the
/// inputs of the request-local `-parseonly` run, so any edit or defines
/// hot-reload produces a different key while an unchanged repeat hits.
pub(super) fn open_token_cache_key(uri: &str, text: &str, defines: &[String]) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    for define in defines {
        define.hash(&mut hasher);
    }
    format!("open-tokens|{uri}|{:016x}", hasher.finish())
}

pub(super) fn open_document_semantic_tokens_if_current(
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
                // path. Preserve the real document identity in its snapshot;
                // the private stage only serializes cleanup and masking.
                features::semantic_tokens_for_open_document_with_parent(
                    path,
                    defines,
                    Some(&stage.source),
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
            // A missing include is left to Slang to diagnose, but an
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

pub(super) fn is_under_any(dirs: &[PathBuf], path: &Path) -> bool {
    dirs.iter()
        .any(|dir| workspace::root_relative_path(dir, path).is_some())
}

pub(super) fn open_document_value<'a>(
    path: &Path,
    open_documents: &'a OpenDocuments,
) -> Option<&'a SharedText> {
    open_documents.get(path).or_else(|| {
        std::fs::canonicalize(path)
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
    std::fs::canonicalize(path)
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
