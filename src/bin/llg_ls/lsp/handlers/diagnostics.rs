//! Diagnostic identity, publication suppression, and shared-file merging.

use super::*;

/// Rebuild the reverse map from include dependency to dependent roots.
pub(super) fn rebuild_dep_dependents(state: &mut BackendState) {
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
pub(super) fn shared_tracker_count(state: &BackendState, path: &Path) -> usize {
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
pub(super) fn annotate_shared_hover(hover: &mut Hover, root_name: &str) {
    if let HoverContents::Markup(markup) = &mut hover.contents {
        markup.value.push_str(&format!(
            "\n\n---\n[{root_name}] parameter/define values follow this root's configuration"
        ));
    }
}

/// Hashable position key (`lsp_types::Position` is not `Hash`).
pub(super) type PosKey = (u32, u32);

pub(super) fn pos_key(position: Position) -> PosKey {
    (position.line, position.character)
}

pub(super) type DiagKey = ((PosKey, PosKey), u8, Option<NumberOrString>, String);

/// Wire severity number for digest/duplicate keys (`None` → 0).
pub(super) fn severity_tag(severity: Option<DiagnosticSeverity>) -> u8 {
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
pub(super) fn diagnostic_key(diagnostic: &Diagnostic) -> DiagKey {
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

pub(super) fn diagnostics_digest(diagnostics: &[Diagnostic]) -> String {
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
pub(super) fn merge_shared_file_diagnostics(
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
pub(super) fn aggregate_shared_publications(
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
impl Backend {
    /// Publish each root's config errors and warnings against its TOML URI.
    pub(super) async fn publish_config_diagnostics(&self) {
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

    pub(super) fn commit_job(
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
}
