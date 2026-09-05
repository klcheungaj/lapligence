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

fn semantic_token_file(path: &Path, line: u32) -> llg::core::tokens::FileTokens {
    let path = path.to_string_lossy().into_owned();
    llg::core::tokens::FileTokens {
        path: path.clone(),
        nodes: vec![llg::ffi::surelog::VObjectInfo {
            line,
            col: 1,
            end_line: line,
            end_col: 2,
            vpi_type: llg::ffi::vpi::vpiModule,
            name: Some("m".to_owned()),
            file: path,
        }],
    }
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
fn standalone_root_prefers_nearest_config_and_rejects_headers() {
    let root = temp_root("standalone_root");
    let nested = root.join("rtl").join("blocks");
    std::fs::create_dir_all(&nested).expect("create standalone test directories");
    std::fs::write(root.join(config::CONFIG_FILE), "schema_version = 1\n")
        .expect("write standalone root config");

    assert_eq!(
        Backend::standalone_root(&nested.join("unit.sv")),
        Some(root.clone())
    );
    assert_eq!(Backend::standalone_root(&nested.join("defs.svh")), None);

    std::fs::remove_dir_all(root).expect("remove standalone root fixture");
}

#[test]
fn standalone_root_without_config_uses_source_parent() {
    let root = temp_root("standalone_parent");
    let nested = root.join("rtl");
    std::fs::create_dir_all(&nested).expect("create standalone source parent");

    assert_eq!(
        Backend::standalone_root(&nested.join("unit.v")),
        Some(nested)
    );

    std::fs::remove_dir_all(root).expect("remove standalone parent fixture");
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
fn cached_semantic_tokens_syntax_error_blocks_real_shadow_alias_fallbacks() {
    // Arrange
    let real = PathBuf::from("/tmp/llg-lsp-token-alias/src/thing.sv");
    let shadow = PathBuf::from("/tmp/llg-lsp-token-alias-shadow/src/thing.sv");
    let paths = vec![
        shadow.to_string_lossy().into_owned(),
        real.to_string_lossy().into_owned(),
    ];
    let mut analysis = features::empty_analysis();
    analysis.tokens.push(semantic_token_file(&shadow, 4));
    analysis.diagnostics.push(llg::ffi::surelog::Diag {
        severity: llg::ffi::surelog::Severity::Syntax,
        file: Some(real.to_string_lossy().into_owned()),
        line: 4,
        col: 1,
        message: "incomplete declaration".to_owned(),
    });

    // Act
    let cached = cached_semantic_tokens(Some(&analysis), &paths, false);

    // Assert: the real-path diagnostic suppresses the shadow-path token
    // before any alternate-path lookup can serve it.
    assert!(cached.data.is_empty());

    // Repeat with the diagnostic and token aliases reversed.  Both
    // aliases are relevant to the same open document.
    analysis.tokens.clear();
    analysis.tokens.push(semantic_token_file(&real, 4));
    analysis.diagnostics[0].file = Some(shadow.to_string_lossy().into_owned());
    let cached = cached_semantic_tokens(Some(&analysis), &paths, false);
    assert!(cached.data.is_empty());
}

#[test]
fn cached_semantic_tokens_checks_canonical_aliases_before_fallback() {
    // Arrange
    let root = temp_root("token_canonical_alias");
    let source_dir = root.join("src");
    std::fs::create_dir_all(&source_dir).expect("create canonical alias root");
    let real = source_dir.join("thing.sv");
    let lexical_alias = source_dir.join(".").join("thing.sv");
    std::fs::write(&real, "module m; endmodule\n").expect("write canonical alias source");
    let mut analysis = features::empty_analysis();
    analysis.tokens.push(semantic_token_file(&real, 4));
    analysis.diagnostics.push(llg::ffi::surelog::Diag {
        severity: llg::ffi::surelog::Severity::Syntax,
        file: Some(lexical_alias.to_string_lossy().into_owned()),
        line: 4,
        col: 1,
        message: "incomplete declaration".to_owned(),
    });

    // Act
    let cached = cached_semantic_tokens(
        Some(&analysis),
        &[real.to_string_lossy().into_owned()],
        false,
    );

    // Assert
    assert!(cached.data.is_empty());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cached_semantic_tokens_isolates_same_basename_files_and_preserves_closed_fallbacks() {
    // Arrange
    let left = PathBuf::from("/tmp/llg-lsp-token-left/dup.sv");
    let right = PathBuf::from("/tmp/llg-lsp-token-right/dup.sv");
    let mut analysis = features::empty_analysis();
    analysis.tokens.push(semantic_token_file(&left, 2));
    analysis.tokens.push(semantic_token_file(&right, 20));
    analysis.diagnostics.push(llg::ffi::surelog::Diag {
        severity: llg::ffi::surelog::Severity::Syntax,
        file: Some(left.to_string_lossy().into_owned()),
        line: 2,
        col: 1,
        message: "incomplete declaration".to_owned(),
    });

    // Act / Assert: the invalid left file is empty, while the valid
    // right file's exact path still returns its own token stream.
    let left_tokens = cached_semantic_tokens(
        Some(&analysis),
        &[left.to_string_lossy().into_owned()],
        false,
    );
    assert!(left_tokens.data.is_empty());
    let right_tokens = cached_semantic_tokens(
        Some(&analysis),
        &[right.to_string_lossy().into_owned()],
        false,
    );
    assert_eq!(right_tokens.data.len(), 1);
    assert_eq!(right_tokens.data[0].delta_line, 19);

    // An open-buffer fallback never guesses from another same-basename
    // file.  The closed/project compatibility path retains the historic
    // basename lookup when no exact entry exists.
    analysis.diagnostics.clear();
    let missing = PathBuf::from("/tmp/llg-lsp-token-missing/dup.sv");
    let open_fallback = cached_semantic_tokens(
        Some(&analysis),
        &[missing.to_string_lossy().into_owned()],
        false,
    );
    assert!(open_fallback.data.is_empty());
    let closed_fallback = cached_semantic_tokens(
        Some(&analysis),
        &[missing.to_string_lossy().into_owned()],
        true,
    );
    assert_eq!(closed_fallback.data.len(), 1);
}

#[test]
fn oversized_open_token_buffer_is_rejected_before_flight_admission() {
    let path = PathBuf::from("/tmp/llg-open-too-large.sv");
    let open_document = (path.clone(), Arc::new("12345".to_owned()), Vec::new());
    let limit = open_token_size_limit(Some(&open_document), Some(4)).expect("over-limit buffer");
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

    assert!(enforce_input_budget(&config, std::slice::from_ref(&source), &BTreeMap::new()).is_ok());

    std::fs::write(&source, b"12345").expect("write over-limit source");
    let failure = enforce_input_budget(&config, std::slice::from_ref(&source), &BTreeMap::new())
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

    let failure = enforce_input_budget(&config, std::slice::from_ref(&source), &BTreeMap::new())
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

    assert!(enforce_input_budget(&config, std::slice::from_ref(&top), &BTreeMap::new()).is_ok());
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
        preflight_include_isolation(&config, &files, &BTreeMap::new(), &budget.snapshots).is_none()
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
        preflight_include_isolation(&config, &files, &BTreeMap::new(), &budget.snapshots).is_none()
    );

    // Escape to a directory outside the configured set.
    std::fs::write(
        &top,
        "`include \"../../outside.svh\"\nmodule top; endmodule\n",
    )
    .expect("write escape");
    let budget = enforce_input_budget(&config, std::slice::from_ref(&top), &BTreeMap::new())
        .expect("admit escape for isolation preflight");
    let failure = preflight_include_isolation(&config, &files, &BTreeMap::new(), &budget.snapshots);
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
    std::fs::write(&top, "`include \"filtered.inc\"\nmodule top; endmodule\n").expect("write top");
    std::fs::write(&filtered, "`include \"nested.txt\"\n").expect("write filtered");
    std::fs::write(&nested, "`include \"filtered.inc\"\n").expect("write nested");

    let config = config_with_dirs(&root, vec![source_dir.clone()]);
    let files = vec![(top.clone(), top.to_string_lossy().into_owned())];
    let budget = enforce_input_budget(&config, std::slice::from_ref(&top), &BTreeMap::new())
        .expect("admit include cycle");
    assert!(
        preflight_include_isolation(&config, &files, &BTreeMap::new(), &budget.snapshots).is_none()
    );

    std::fs::write(&nested, "`include \"../../outside.svh\"\n").expect("write escape");
    let budget = enforce_input_budget(&config, std::slice::from_ref(&top), &BTreeMap::new())
        .expect("admit escape for isolation preflight");
    let failure = preflight_include_isolation(&config, &files, &BTreeMap::new(), &budget.snapshots);
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
    let failure = preflight_include_isolation(&config, &files, &BTreeMap::new(), &budget.snapshots);
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

#[tokio::test]
async fn module_explorer_task_failure_uses_error_outcome_and_empty_snapshot() {
    // Arrange: a panic in the blocking worker is surfaced as a JoinError.
    let task = tokio::task::spawn_blocking(|| -> module_explorer::ExplorerSnapshot {
        panic!("intentional module-explorer task failure");
    });

    // Act
    let result = module_explorer_task_result(task.await);

    // Assert: task failure is not indistinguishable from a valid empty
    // workspace, while the fallback remains protocol-safe.
    assert_eq!(result.outcome, "error");
    assert!(
        result.error.is_some(),
        "JoinError must be retained for logging"
    );
    assert!(result.snapshot.modules.is_empty());
    assert!(result.snapshot.roots.is_empty());

    let empty_workspace = module_explorer_task_result(Ok(module_explorer::ExplorerSnapshot {
        modules: Vec::new(),
        roots: Vec::new(),
    }));
    assert_eq!(empty_workspace.outcome, "ok");
    assert!(empty_workspace.error.is_none());
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
    std::fs::write(&top, "\x60include \"defs.inc\"\nmodule top; endmodule\n").expect("write top");
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
    assert!(staged.starts_with(features::process_shadow_base()));
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
fn semantic_parse_masks_begin_keywords_and_delay_directives() {
    // Arrange: these compiler directives are valid in a preprocessed
    // source file but are not accepted by the request-local parse-only
    // path. The continuation also must not leak into the parser.
    let source = concat!(
        "  `begin_keywords \"1800-2012\" \\\r\n",
        "    \"1800-2017\"\r\n",
        "`end_keywords\r\n",
        "`accelerate\r\n",
        "`noaccelerate\r\n",
        "`default_decay_time 0\r\n",
        "`default_trireg_strength (strong1, strong0)\r\n",
        "`delay_mode_distributed\r\n",
        "`delay_mode_path\r\n",
        "`delay_mode_unit\r\n",
        "`delay_mode_zero\r\n",
        "module top;\r\n",
        "endmodule\r\n",
    );

    // Act
    let masked = mask_semantic_preprocessor_directives(source, &[]);
    let lines = masked.lines().collect::<Vec<_>>();

    // Assert
    for line in &lines[..11] {
        assert!(line.trim().is_empty(), "directive was not masked: {line:?}");
    }
    assert_eq!(lines[11], "module top;");
    assert_eq!(lines[12], "endmodule");
    assert_eq!(source.len(), masked.len(), "CRLF bytes must be retained");
    assert_eq!(
        source.encode_utf16().count(),
        masked.encode_utf16().count(),
        "masked text must retain UTF-16 positions"
    );
    assert_eq!(
        source.matches("\r\n").count(),
        masked.matches("\r\n").count()
    );
}

#[test]
fn semantic_parse_accepts_masked_compiler_directives() {
    // Arrange
    let _guard = SHADOW_TESTS_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let root = temp_root("semantic_directives");
    std::fs::create_dir_all(&root).expect("create semantic test root");
    let path = root.join("directives.sv");
    let source = concat!(
        "`begin_keywords \"1800-2012\"\n",
        "`end_keywords\n",
        "`accelerate\n",
        "`default_decay_time 0\n",
        "`delay_mode_distributed\n",
        "`delay_mode_path\n",
        "`delay_mode_unit\n",
        "`delay_mode_zero\n",
        "module top;\n",
        "endmodule\n",
    );
    let masked = mask_semantic_preprocessor_directives(source, &[]);
    std::fs::write(&path, masked).expect("write masked semantic source");

    // Act
    let tokens =
        features::semantic_tokens_for_open_document(path.to_str().expect("UTF-8 test path"), &[]);
    features::cleanup_process_shadow();
    let _ = std::fs::remove_dir_all(root);

    // Assert: the valid module survives parse-only collection, so the
    // directive lines did not become false syntax errors.
    let tokens = tokens.expect("masked compiler directives must parse");
    assert!(!tokens.data.is_empty(), "module tokens should be collected");
}

#[test]
fn semantic_parse_masks_every_shared_directive_keyword() {
    // Arrange / Act / Assert: exercise the local classifier against the
    // complete compiler-directive keyword set mirrored from
    // core::macros. The scanner-only predefined expression macros are
    // covered separately because they must remain parseable source.
    for keyword in SEMANTIC_DIRECTIVE_KEYWORDS {
        let source = format!("  `{keyword} argument\r\nmodule top;\r\nendmodule\r\n");
        let masked = mask_semantic_preprocessor_directives(&source, &[]);
        let first_line = masked.lines().next().expect("directive line");

        assert!(
            first_line.trim().is_empty(),
            "shared directive `{keyword}` was not masked: {first_line:?}"
        );
        assert_eq!(
            source.len(),
            masked.len(),
            "byte positions changed for `{keyword}`"
        );
        assert_eq!(
            source.encode_utf16().count(),
            masked.encode_utf16().count(),
            "UTF-16 positions changed for `{keyword}`"
        );
        assert_eq!(
            source.matches("\r\n").count(),
            masked.matches("\r\n").count(),
            "line endings changed for `{keyword}`"
        );
        assert!(masked.contains("module top;"));
    }
}

#[test]
fn semantic_parse_keeps_predefined_expression_macros_in_multiline_localparams() {
    // Arrange
    assert_eq!(
        SEMANTIC_PREDEFINED_EXPRESSION_MACROS,
        &["__FILE__", "__LINE__"]
    );
    for keyword in SEMANTIC_PREDEFINED_EXPRESSION_MACROS {
        assert!(
            !SEMANTIC_DIRECTIVE_KEYWORDS.contains(keyword),
            "predefined expression macro `{keyword}` must not be a whole-line directive"
        );
    }
    let source = concat!(
        "module top;\n",
        "  localparam string source_file =\n",
        "    `__FILE__;\n",
        "  localparam int source_line =\n",
        "    `__LINE__;\n",
        "  `include \"not-consumed.svh\"\n",
        "endmodule\n",
    );

    // Act
    let masked = mask_semantic_preprocessor_directives(source, &[]);
    let source_lines = source.lines().collect::<Vec<_>>();
    let masked_lines = masked.lines().collect::<Vec<_>>();

    // Assert: macro-use lines retain their source text, while the actual
    // compiler directive remains masked.
    assert_eq!(masked_lines[2], source_lines[2]);
    assert_eq!(masked_lines[4], source_lines[4]);
    assert!(masked_lines[5].trim().is_empty());
    assert_eq!(source.len(), masked.len());
    assert_eq!(source.encode_utf16().count(), masked.encode_utf16().count());

    let prefixed = "`__FILE__SUFFIX `__LINE__WIDTH\n";
    assert_eq!(
        normalize_semantic_expression_macros_for_parse(prefixed),
        prefixed,
        "longer user macro identifiers must not match predefined names by prefix"
    );

    // Also exercise the isolated parser over the staged source.  The
    // request-local copy keeps the macro lines nonblank while replacing
    // only their backtick for Surelog's raw parse-only grammar.
    let _guard = SHADOW_TESTS_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let path = temp_root("semantic_predefined_expression_macros").join("predefined.sv");
    let stage = SemanticStage::new(&path, source, &[]).expect("stage semantic macro source");
    let tokens = features::semantic_tokens_for_open_document(
        stage.path.to_str().expect("UTF-8 staged path"),
        &[],
    );
    drop(stage);
    features::cleanup_process_shadow();
    let tokens = tokens.expect("predefined expression macros must parse in isolation");
    assert!(!tokens.data.is_empty(), "module tokens should be collected");
}

#[test]
fn semantic_parse_masks_only_directives_outside_strings_and_comments() {
    // Arrange
    let source = concat!(
        "localparam string text = \"`begin_keywords\";\r\n",
        "// `delay_mode_zero\r\n",
        "/* `accelerate\r\n",
        "   `end_keywords */\r\n",
        "  `delay_mode_zero\r\n",
        "module top;\r\n",
        "endmodule\r\n",
    );

    // Act
    let masked = mask_semantic_preprocessor_directives(source, &[]);
    let source_lines = source.lines().collect::<Vec<_>>();
    let masked_lines = masked.lines().collect::<Vec<_>>();

    // Assert
    assert_eq!(&masked_lines[..4], &source_lines[..4]);
    assert!(masked_lines[4].trim().is_empty());
    assert_eq!(&masked_lines[5..], &source_lines[5..]);
}

#[test]
fn semantic_parse_masks_inactive_unicode_lines_without_changing_utf16_positions() {
    // Arrange: the inactive body contains a supplementary character, so
    // scalar-count preservation alone would move an LSP UTF-16 column.
    let source = concat!(
        "`ifdef UNUSED\r\n",
        "😀 must not reach parse-only\r\n",
        "`endif\r\n",
        "module top;\r\n",
        "endmodule\r\n",
    );

    // Act
    let masked = mask_semantic_preprocessor_directives(source, &[]);

    // Assert
    assert_eq!(
        source.encode_utf16().count(),
        masked.encode_utf16().count(),
        "inactive masking must preserve UTF-16 coordinates"
    );
    assert_eq!(
        source.matches("\r\n").count(),
        masked.matches("\r\n").count()
    );
    assert_eq!(
        masked.lines().nth(3),
        Some("module top;"),
        "active source after an inactive branch must remain unchanged"
    );
    assert_eq!(masked.lines().nth(4), Some("endmodule"));
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
    let (reloaded, reload_errors, reload_warnings) = Backend::load_root_config(&root, &config_path);
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
    assert!(
        !survivor[0].1.is_empty(),
        "the surviving root's diagnostics must be republished instead of clearing the stale union"
    );
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
