//! Debounced analysis scheduling and dynamic watcher registration.

use super::*;

/// Free-standing watcher registration used by both the backend methods and
/// spawned commit tasks (which cannot borrow `&Backend`).
///
/// See [`Backend::register_watchers`] for the coalescing contract.
pub(super) async fn register_watchers_with(client: &Client, state: &Arc<Mutex<BackendState>>) {
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
pub(super) fn spawn_debounced_run(client: Client, state: Arc<Mutex<BackendState>>, key: RootKey) {
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
pub(super) fn reload_root_config_in_state(
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
/// Build dynamic watcher options from every root's config and resolved
/// include dependencies.
pub(super) fn watcher_options_from_state(state: &BackendState) -> LSPAny {
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
impl Backend {
    pub(super) async fn rescan(&self) -> BTreeSet<RootKey> {
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

    pub(super) fn discover_snapshot(descriptors: Vec<RootDescriptor>) -> RescanResult {
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
    pub(super) fn source_relative_path(config: &LlgConfig, path: &Path) -> Option<PathBuf> {
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
    pub(super) fn make_jobs(state: &Arc<Mutex<BackendState>>, keys: Vec<RootKey>) -> Vec<RootJob> {
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

    pub(super) fn schedule_all(&self, parent_id: Option<u64>) {
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
    pub(super) fn schedule_roots_with_parent(&self, keys: Vec<RootKey>, parent_id: Option<u64>) {
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
    pub(super) fn finish_root_run(state: &Arc<Mutex<BackendState>>, key: &RootKey) -> bool {
        let mut guard = state.lock().unwrap_or_else(|error| error.into_inner());
        if guard.shutting_down {
            return false;
        }
        guard
            .roots
            .get_mut(key)
            .is_some_and(|root| root.scheduler.job_finished())
    }

    pub(super) fn job_current(state: &Arc<Mutex<BackendState>>, job: &RootJob) -> bool {
        let state = state.lock().unwrap_or_else(|error| error.into_inner());
        state
            .roots
            .get(&job.key)
            .is_some_and(|root| !state.shutting_down && root.generation == job.generation)
    }

    pub(super) fn compile_job(
        state: &Arc<Mutex<BackendState>>,
        job: RootJob,
    ) -> Option<CompileResult> {
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

    pub(super) async fn register_watchers(&self) {
        register_watchers_with(&self.client, &self.state).await;
    }

    // ── config reload ───────────────────────────────────────────────────────

    /// Reload a root's `llg.toml`, retaining the last-valid config when the
    /// new file is malformed.  Returns `true` when the effective config
    /// (or its path) changed so callers can rescan/schedule.
    pub(super) fn reload_root_config(&self, key: &RootKey, config_path: &Path) -> bool {
        let (config, errors, warnings) = Self::load_root_config(key, config_path);
        let mut state = self.lock_state();

        reload_root_config_in_state(&mut state, key, config_path, config, &errors, &warnings)
    }
}
