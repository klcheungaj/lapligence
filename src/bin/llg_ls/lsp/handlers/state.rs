//! Owned backend and per-root state.

use super::*;

pub(super) struct RootState {
    pub(super) descriptor: RootDescriptor,
    pub(super) shadow: ShadowPaths,
    /// Discovered `.v`/`.sv` compilation units (normalized absolute paths).
    pub(super) discovered: Vec<PathBuf>,
    /// Exact resolved include dependencies (any extension), used to extend the
    /// watched-file set after a successful analysis.
    pub(super) include_deps: BTreeSet<PathBuf>,
    pub(super) lint_config: LintConfig,
    /// Parse/validation errors from the most recent config load (empty when
    /// the config is valid or missing).  Used to publish a diagnostic against
    /// the TOML URI even when a last-valid config is retained.
    pub(super) config_errors: Vec<ConfigError>,
    /// Non-fatal config-load warnings (e.g. configured source/include
    /// directories that do not exist).  Published once per load as WARNING
    /// diagnostics against the TOML URI; discovery rescans stay silent about
    /// them so they never repeat on every scan.
    pub(super) config_warnings: Vec<String>,
    pub(super) last_good: Option<Arc<Analysis>>,
    /// Identity of the `last_good` snapshot for the request memoization keys
    /// (`request_cache`).  Stamped with a fresh process-global value exactly
    /// where a commit replaces or clears `last_good`, so any input change that
    /// flows through an analysis commit invalidates memoized results
    /// structurally; reads happen together with `last_good` under the state
    /// lock.
    pub(super) analysis_epoch: u64,
    /// Diagnostics published for this root's documents (open or closed).
    pub(super) diagnostics: BTreeMap<Url, Vec<Diagnostic>>,
    /// Digest of the last primary payload sent per URI by this root, so
    /// unchanged diagnostics are not re-sent after every commit.  Cleared for
    /// URIs this root no longer publishes.
    pub(super) published_digests: BTreeMap<Url, String>,
    /// Diagnostics from the latest commit keyed by REAL file path, including
    /// closed files.  Feeds the shared-file aggregation (which must publish
    /// unions for tracked-but-closed external files too).
    pub(super) all_diagnostics: BTreeMap<PathBuf, Vec<Diagnostic>>,
    pub(super) generation: u64,
    /// Parent correlation for the next run.  A coalesced trigger replaces
    /// this only when it carries a real request/notification ID; startup and
    /// internal reschedules intentionally leave it absent.
    pub(super) pending_parent_id: Option<u64>,
    /// Debounce + latest-wins coalescing for this root's analysis runs (see
    /// [`SchedulerState`]); mutated only under the backend state lock.
    pub(super) scheduler: SchedulerState,
}
pub(super) struct BackendState {
    pub(super) roots: BTreeMap<RootKey, RootState>,
    pub(super) documents: BTreeMap<Url, SharedText>,
    pub(super) dynamic_watched_files: bool,
    pub(super) watchers_registered: bool,
    /// Digest of the watcher options sent with the last successful
    /// registration, so repeated valid commits coalesce into one registration
    /// when nothing relevant changed.
    pub(super) registered_watchers_digest: Option<String>,
    pub(super) initialized: bool,
    pub(super) shutting_down: bool,
    pub(super) pending_logs: Vec<String>,
    pub(super) merged: Option<Arc<Analysis>>,
    pub(super) initial_pending: BTreeSet<RootKey>,
    pub(super) ready_sent: bool,
    /// Reverse include-dependency index: resolved dep path → dependent roots.
    /// Rebuilt during commits and workspace-folder changes; watched-file
    /// events for arbitrary-extension deps schedule every dependent root.
    pub(super) dep_dependents: BTreeMap<PathBuf, BTreeSet<RootKey>>,
    /// Digest of the last aggregated shared-file publication per URI, so
    /// unchanged unions are not re-sent after every commit.
    pub(super) published_shared: BTreeMap<Url, String>,
    /// Monotonic job-generation counter; bumped only while creating a job
    /// under the state lock (which is exactly when staleness is decided).
    pub(super) next_generation: u64,
}
pub(super) struct RootJob {
    pub(super) key: RootKey,
    pub(super) generation: u64,
    /// Correlation ID of the request/notification that armed or refreshed
    /// this coalesced run.  `None` is expected for startup/internal work;
    /// coalesced triggers retain only the latest meaningful parent.
    pub(super) parent_id: Option<u64>,
    pub(super) shadow: ShadowPaths,
    pub(super) files: Vec<PathBuf>,
    /// Include dependencies from the last committed result.  A size-limit
    /// preflight must retain these watchers while the last-good snapshot is
    /// still being served.
    pub(super) previous_include_deps: BTreeSet<PathBuf>,
    pub(super) open_documents: OpenDocuments,
    pub(super) lint_config: LintConfig,
    pub(super) config: LlgConfig,
}
pub(super) struct CompileResult {
    pub(super) analysis: Option<Analysis>,
    pub(super) files: Vec<(PathBuf, String)>,
    pub(super) include_deps: BTreeSet<PathBuf>,
}

#[derive(Default)]
pub(super) struct CommitOutcome {
    pub(super) publications: Vec<(Url, Vec<Diagnostic>)>,
    pub(super) ready: bool,
    /// Whether the committed analysis carries servable feature data (see
    /// [`Analysis::has_feature_data`]).  Watcher re-registration triggers
    /// once per such commit so newly resolved include deps get watched —
    /// including commits from roots that never reach strict validity.
    pub(super) valid_commit: bool,
    /// Whether this commit has a resolved include set that should be offered
    /// to dynamic watchers, even when the analysis itself is Fatal (for
    /// example, a size-limit rejection retaining the last-good snapshot).
    pub(super) watchers_refresh: bool,
    /// Whether the committed feature model changed the module explorer
    /// snapshot.  Diagnostics-only/fatal commits retain the existing model
    /// and do not cause clients to refetch it.
    pub(super) module_explorer_changed: bool,
}
pub(super) struct RescanResult {
    pub(super) discovered: BTreeMap<RootKey, BTreeSet<PathBuf>>,
    pub(super) warnings: Vec<String>,
}

/// One client-supplied config-file override: a workspace root and the path to
/// its effective `llg.toml`.
#[derive(Debug, Clone)]
pub(super) struct ClientConfigFile {
    pub(super) workspace_uri: Url,
    pub(super) path: PathBuf,
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
pub(super) struct ClientInit {
    pub(super) protocol_version: u32,
    pub(super) config_files: Vec<ClientConfigFile>,
}

pub(crate) struct Backend {
    pub(super) client: Client,
    pub(super) state: Arc<Mutex<BackendState>>,
    /// Request memoization for read-only queries.  Keys carry the analysis
    /// epoch, so entries never outlive the snapshot they were computed from;
    /// access bypasses the serialized lifecycle queue exactly like the
    /// underlying handlers do.
    pub(super) definition_cache: MemoCache<RequestKey, Option<Location>>,
    pub(super) hover_cache: MemoCache<RequestKey, Option<Hover>>,
    pub(super) references_cache: MemoCache<RequestKey, Vec<Location>>,
    /// Open-buffer isolated token streams keyed on (uri, buffer text,
    /// effective `-D` defines) — the exact inputs of the request-local
    /// parse-only run.
    pub(super) open_token_cache: Arc<MemoCache<String, SemanticTokens>>,
    /// In-flight keyed single-flight coordination for open-buffer semantic
    /// token misses.  This is separate from the result cache so cache changes
    /// owned by another module do not affect the bounded wait lifecycle.
    pub(super) open_token_flights: Arc<OpenTokenFlightRegistry>,
    /// Inactive-range results keyed on (uri, text digest, effective
    /// `[compile] defines`) — the exact inputs of the pure lexical scan.
    pub(super) inactive_cache: MemoCache<String, Vec<crate::inactive_ranges::LineRange>>,
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
    pub(super) fn cache_stats(&self) -> (CacheStats, usize) {
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
    pub(super) fn annotate_hover(
        &self,
        mut hover: Option<Hover>,
        owner_name: Option<&str>,
    ) -> Option<Hover> {
        if let (Some(hover), Some(owner_name)) = (hover.as_mut(), owner_name) {
            annotate_shared_hover(hover, owner_name);
        }
        hover
    }

    /// Shadow→real URI mapping for a batch of memoized or fresh locations.
    pub(super) fn map_locations(&self, locations: &[Location]) -> Vec<Location> {
        let state = self.lock_state();
        locations
            .iter()
            .map(|value| Self::map_location(&state, value.clone()))
            .collect()
    }

    pub(super) fn lock_state(&self) -> std::sync::MutexGuard<'_, BackendState> {
        self.state.lock().unwrap_or_else(|error| error.into_inner())
    }

    pub(super) fn uri_to_path(uri: &Url) -> Option<PathBuf> {
        if uri.scheme() != "file" {
            return None;
        }
        uri.to_file_path()
            .ok()
            .and_then(|path| workspace::normalize_absolute_path(&path))
    }

    pub(super) fn path_to_uri(path: &Path) -> Option<Url> {
        Url::from_file_path(path).ok()
    }

    /// Capture only a request's document identity for logs.  Source text and
    /// request payloads never enter lifecycle records.
    pub(super) fn log_uri_identity(uri: &Url) -> String {
        crate::logging::bounded_field(uri.as_str())
    }

    pub(super) fn roots_from_initialize(params: &InitializeParams) -> Vec<(PathBuf, String)> {
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
        roots.into_iter().collect()
    }

    /// Select the root for an opened compilation unit that is not owned by a
    /// configured workspace. A nearby `llg.toml` defines the project;
    /// otherwise the file's parent is the smallest useful standalone root.
    /// Header files remain include-only and never create projects themselves.
    pub(super) fn standalone_root(path: &Path) -> Option<PathBuf> {
        if !config::is_compilation_unit(path) {
            return None;
        }
        let parent = path.parent()?;
        for ancestor in parent.ancestors() {
            if ancestor.join(config::CONFIG_FILE).is_file() {
                return workspace::normalize_absolute_path(ancestor);
            }
        }
        workspace::normalize_absolute_path(parent)
    }

    /// Ensure an opened source has an owning root. Standalone roots are
    /// retained for the server session so later edits, sibling opens and
    /// diagnostics use the same analysis and scheduler lifecycle.
    pub(super) fn ensure_source_root(&self, path: &Path) -> (Option<RootKey>, bool) {
        if let Some(root) = self.source_root(path) {
            return (Some(root), false);
        }
        let Some(root_path) = Self::standalone_root(path) else {
            return (None, false);
        };
        let config_path = root_path.join(config::CONFIG_FILE);
        let id = format!("standalone:{}", root_path.display());
        let (root, warnings) = Self::root_state(root_path.clone(), id, config_path);

        let root = {
            let mut state = self.lock_state();
            let descriptors: Vec<_> = state
                .roots
                .values()
                .map(|root| root.descriptor.clone())
                .collect();
            if let Some(owner) = workspace::owning_root_unfiltered(path, &descriptors) {
                return (Some(owner.root), false);
            }
            if state.roots.contains_key(&root_path) {
                return (Some(root_path), false);
            }
            state
                .pending_logs
                .extend(warnings.iter().map(|warning| warning.message.clone()));
            state.roots.insert(root_path.clone(), root);
            rebuild_dep_dependents(&mut state);
            Self::rebuild_merged(&mut state);
            root_path
        };
        (Some(root), true)
    }

    /// Parse the client initialization options payload.
    pub(super) fn parse_client_init(settings: &LSPAny) -> (ClientInit, Vec<String>) {
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
    pub(super) fn config_override_for<'a>(
        init: &'a ClientInit,
        root: &Path,
    ) -> Option<&'a ClientConfigFile> {
        init.config_files.iter().find(|file| {
            Self::uri_to_path(&file.workspace_uri)
                .as_deref()
                .is_some_and(|path| path == root)
        })
    }

    /// Load a root's `llg.toml` (at the effective path) and return the
    /// last-valid config plus any parse errors and non-fatal warnings.  A
    /// missing config is a normal result with `config: None` and no errors.
    pub(super) fn load_root_config(
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
    pub(super) fn root_state(
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

    /// Rebuild the merged symbol index from every retained root snapshot.
    pub(super) fn rebuild_merged(state: &mut BackendState) {
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

    pub(super) fn source_root(&self, path: &Path) -> Option<RootKey> {
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
    pub(super) fn document_max_file_bytes(state: &BackendState, uri: &Url) -> u64 {
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
    pub(super) fn admit_document_text(
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
    pub(super) fn purge_oversized_documents(state: &mut BackendState) {
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
                crate::llg_error!(
                    "event=document.admission outcome=rejected reason=too-large uri={} bytes={} max_file_bytes={} advice={}",
                    crate::logging::bounded_field(uri.as_str()),
                    measured_bytes,
                    max_file_bytes,
                    config::SOURCE_SIZE_LIMIT_GUIDANCE
                );
            }
        }
    }

    pub(super) fn config_root(&self, path: &Path) -> Option<RootKey> {
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
    pub(super) fn is_effective_config_path(&self, path: &Path) -> bool {
        let roots: Vec<_> = self
            .lock_state()
            .roots
            .values()
            .map(|root| root.descriptor.clone())
            .collect();
        workspace::is_effective_config_path(path, &roots)
    }

    pub(super) fn mark_ready_if_empty(&self) -> bool {
        let mut state = self.lock_state();
        if !state.ready_sent && state.initial_pending.is_empty() {
            state.ready_sent = true;
            true
        } else {
            false
        }
    }

    pub(super) fn root_context<'a>(
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
        // Slang compiles admitted buffers under their real source identity.
        // The staging tree is only an I/O isolation detail and must not enter
        // feature lookup: trying its path first can activate filename fallback
        // and select a different same-named file before the exact path is read.
        let paths = real
            .to_str()
            .map(|path| vec![path.to_owned()])
            .unwrap_or_default();
        Some((root, real, paths))
    }

    pub(super) fn map_location(state: &BackendState, mut location: Location) -> Location {
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
    pub(super) fn map_workspace_edit_uris(
        state: &BackendState,
        mut edit: WorkspaceEdit,
    ) -> WorkspaceEdit {
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

    pub(super) async fn flush_logs(&self) {
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
    pub(super) fn dependent_roots_for_dep(&self, path: &Path) -> BTreeSet<RootKey> {
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
}
