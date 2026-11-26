//! The identifier table of one document, which semantic tokens and
//! go-to-definition both read.

use super::positions::{Placement, Places, Served};
use super::*;

/// The table that answers requests for the current revision of a document.
#[derive(Clone)]
pub(super) enum DocumentTable {
    /// The committed analysis: of a closed document, of an open one whose
    /// text it compiled, and of an open one whose isolated parse was refused
    /// or failed.
    Committed,
    /// The isolated parse of an open document's uncommitted text.
    Isolated(Arc<features::DocumentIdentifiers>),
}

/// An isolated parse, and how it was obtained.
pub(super) enum IsolatedParse {
    Ended(Option<Arc<features::DocumentIdentifiers>>, &'static str),
    Running(Arc<OpenTokenFlight>, &'static str),
}

/// The table of one request, and how it was obtained.
pub(super) struct DocumentRequest {
    pub(super) table: DocumentTable,
    /// The committed analysis, which also gives an isolated parse its positions
    /// in other documents.
    served: Option<Served>,
    /// How the committed analysis' positions in the document moved.
    placement: Placement,
    /// The document's path and the epoch of the committed analysis.
    pub(super) path: String,
    pub(super) epoch: u64,
    pub(super) outcome: &'static str,
    root: RootKey,
    open: bool,
}

impl DocumentRequest {
    /// Answer from the isolated parse `outcome` describes, when it gave a
    /// table. An edit that landed while the parse ran (not `current`) makes
    /// its table the one of an obsolete revision; the client asks again for
    /// the new one.
    fn settle(
        &mut self,
        isolated: Option<Arc<features::DocumentIdentifiers>>,
        outcome: &'static str,
        current: bool,
    ) {
        self.outcome = if current { outcome } else { "stale" };
        if let Some(identifiers) = isolated.filter(|_| current) {
            self.table = DocumentTable::Isolated(identifiers);
        }
    }

    pub(super) fn semantic_tokens(&self) -> SemanticTokens {
        match (&self.table, &self.served) {
            (DocumentTable::Isolated(identifiers), _) => identifiers.semantic_tokens(),
            (DocumentTable::Committed, Some(served)) if self.placement.is_same() => {
                features::semantic_tokens_for(&served.analysis, &self.path)
            }
            (DocumentTable::Committed, Some(served)) => {
                features::placed_semantic_tokens_for(&served.analysis, &self.path, |range| {
                    self.placement.current(range)
                })
            }
            (DocumentTable::Committed, None) => empty_semantic_tokens(),
        }
    }

    /// The definitions of the name at `position` that the current texts have.
    pub(super) fn definitions_at(
        &self,
        state: &Arc<Mutex<BackendState>>,
        position: Position,
    ) -> Vec<Location> {
        let (definitions, places) = match (&self.table, &self.served) {
            (DocumentTable::Isolated(identifiers), served) => {
                let places = served.as_ref().map(|served| {
                    let mut places = Places::new(state, served);
                    // Its own targets are those of the text that was parsed.
                    if let Ok(uri) = Url::from_file_path(&self.path) {
                        places.hold_in(uri);
                    }
                    places
                });
                let definitions = identifiers.definitions_at(position.line, position.character);
                (definitions, places)
            }
            (DocumentTable::Committed, Some(served)) => {
                let Some(place) = self
                    .placement
                    .analysed(&served.analysis, &self.path, position)
                else {
                    return Vec::new();
                };
                let definitions = features::definitions_at(
                    &served.analysis,
                    &self.path,
                    place.line,
                    place.character,
                );
                (definitions, Some(Places::new(state, served)))
            }
            (DocumentTable::Committed, None) => return Vec::new(),
        };
        let Some(mut places) = places else {
            return definitions
                .into_iter()
                .map(|definition| definition.location)
                .collect();
        };
        let locations: Vec<Location> = definitions
            .iter()
            .filter_map(|definition| {
                places.location(definition.name.as_deref(), &definition.location)
            })
            .collect();
        places.log("textDocument/definition", locations.len());
        locations
    }
}

impl Backend {
    /// The table that answers a request for `uri` now. `None` for a document
    /// that no root owns.
    pub(super) async fn document_request(
        &self,
        uri: &Url,
        serves_tokens: bool,
        span: &mut crate::logging::LifecycleSpan,
    ) -> Option<DocumentRequest> {
        let (mut request, document, max_file_bytes) = {
            let mut state = self.lock_state();
            let (root, real, paths) = Self::root_context(&state, uri)?;
            let path = paths.into_iter().next()?;
            let root_key = root.descriptor.root.clone();
            span.set_root(|| root.descriptor.id.clone());
            let served = root.served();
            let epoch = root.analysis_epoch;
            let document = root.open_document(&state.documents, uri, &real);
            let max_file_bytes = root.descriptor.effective_config().analysis.max_file_bytes;
            // The owner root serves a file that several roots track.
            if crate::logging::enabled(crate::logging::Level::Trace)
                && shared_tracker_count(&state, &real) > 1
            {
                crate::llg_trace!(
                    "identifiers: owner root {} wins for shared file {}",
                    root.descriptor.id,
                    real.display()
                );
            }
            // Recorded under the same lock as the captured inputs, so a
            // commit that lands while this request computes sees them and
            // asks the client to re-request a stream that is already old.
            if serves_tokens && state.semantic_tokens_refresh {
                if let Some(root) = state.roots.get_mut(&root_key) {
                    match &document {
                        Some(document) => root.served_tokens.record_open(
                            uri,
                            &document.defines,
                            &document.includes,
                        ),
                        None => root.served_tokens.record_closed(),
                    }
                }
            }
            let placement = match &served {
                Some(served) => state.placement(&served.texts, &real),
                None => Placement::Unknown,
            };
            let request = DocumentRequest {
                table: DocumentTable::Committed,
                served,
                placement,
                path,
                epoch,
                outcome: "committed",
                root: root_key,
                open: document.is_some(),
            };
            (request, document, max_file_bytes)
        };
        let Some(document) = document else {
            return Some(request);
        };
        if request.placement.is_same() {
            return Some(request);
        }
        let text = Arc::clone(&document.text);
        let (isolated, outcome) = self
            .isolated_identifiers(uri, document, max_file_bytes, span.id())
            .await;
        let current = open_document_is_current(&self.state, uri, &text);
        request.settle(isolated, outcome, current);
        if let (false, Some(served)) = (current, &request.served) {
            let path = Path::new(&request.path);
            request.placement = self.lock_state().placement(&served.texts, path);
        }
        Some(request)
    }

    /// Stop the isolated parses of document `uri` that answer for an older text.
    pub(super) fn cancel_superseded_parses(&self, uri: &Url) {
        let current = self.lock_state().documents.get(uri).map(Arc::clone);
        self.open_token_flights
            .cancel_superseded(uri, current.as_deref().map(String::as_str));
    }

    /// Record the stream served for an open document.
    pub(super) fn record_served_stream(
        &self,
        uri: &Url,
        request: &DocumentRequest,
        tokens: &SemanticTokens,
    ) {
        if !request.open {
            return;
        }
        let digest = stream_digest(tokens);
        let mut state = self.lock_state();
        if let Some(root) = state.roots.get_mut(&request.root) {
            // A commit since the request captured its inputs decided with the
            // stream unknown, and the record is gone.
            if root.analysis_epoch == request.epoch {
                root.served_tokens.record_stream(uri, digest);
            }
        }
    }

    /// Whether the analysis compiled what document `uri` holds now: its open
    /// text, or else its file.
    pub(super) fn compiled_current_text(
        &self,
        served: &Served,
        uri: &Url,
        max_file_bytes: u64,
    ) -> bool {
        let Some(path) = Self::uri_to_path(uri) else {
            return false;
        };
        {
            let mut state = self.lock_state();
            if state.documents.contains_key(uri) {
                return state.placement(&served.texts, &path).is_same();
            }
        }
        let Ok(read) = read_closed_input_snapshot(&path, max_file_bytes) else {
            return false;
        };
        path.to_str().is_some_and(|path| {
            served
                .analysis
                .compiled_from(path, features::SourceFingerprint::of(&read.text))
        })
    }

    /// The identifiers of an open document from its isolated parse. `None` when
    /// the parse is refused or failed.
    async fn isolated_identifiers(
        &self,
        uri: &Url,
        document: OpenTokenDocument,
        max_file_bytes: u64,
        request_id: u64,
    ) -> (Option<Arc<features::DocumentIdentifiers>>, &'static str) {
        match self.isolated_parse(uri, document, max_file_bytes, request_id) {
            IsolatedParse::Ended(identifiers, outcome) => (identifiers, outcome),
            IsolatedParse::Running(flight, outcome) => (flight.wait().await.ok(), outcome),
        }
    }

    /// The isolated parse of an open document, memoized: it is a pure
    /// function of the buffer text, the `[compile] defines` and the committed
    /// include context (named by the epoch). A repeat request with unchanged
    /// inputs costs one lookup, and concurrent requests share one parse.
    pub(super) fn isolated_parse(
        &self,
        uri: &Url,
        document: OpenTokenDocument,
        max_file_bytes: u64,
        request_id: u64,
    ) -> IsolatedParse {
        // Checked before key construction, flight admission and
        // spawn_blocking: an obsolete revision must not consume a bounded
        // flight slot, and an oversized one must not be parsed.
        if !open_document_is_current(&self.state, uri, &document.text) {
            return IsolatedParse::Ended(None, "stale");
        }
        if let Some(limit) = open_input_size_limit(&document.real, &document.text, max_file_bytes) {
            limit.log_rejection();
            crate::llg_debug!("isolated parse rejected: {}", limit.message());
            return IsolatedParse::Ended(None, "too-large");
        }
        let key = open_token_cache_key(
            uri.as_str(),
            &document.text,
            &document.defines,
            document.epoch,
        );
        if let Some(result) = self.open_token_cache.get(&key) {
            return IsolatedParse::Ended(result.ok(), "cache-hit");
        }
        let lease = self
            .open_token_flights
            .acquire(key.clone(), uri, &document.text);
        if crate::logging::enabled(crate::logging::Level::Trace) {
            let lease_kind = match &lease {
                OpenTokenFlightLease::Leader(_) => "leader",
                OpenTokenFlightLease::Follower(_) => "follower",
                OpenTokenFlightLease::Saturated => "saturated",
            };
            crate::llg_trace!(
                "event=semantic_tokens.flight_admission outcome={} uri={} active_flights={}",
                lease_kind,
                uri,
                self.open_token_flights.len()
            );
        }
        match lease {
            OpenTokenFlightLease::Follower(flight) => {
                IsolatedParse::Running(flight, "single-flight")
            }
            OpenTokenFlightLease::Leader(mut leader) => {
                // A request can miss the cache, then race a coordinator that
                // publishes and removes its flight before this request calls
                // `acquire`; complete this flight from the cache then.
                if let Some(result) = self.open_token_cache.get(&key) {
                    leader.finish(result.clone());
                    return IsolatedParse::Ended(result.ok(), "cache-race");
                }
                let flight = self.spawn_open_token_coordinator(
                    leader.detach(),
                    key,
                    uri.clone(),
                    document,
                    Some(request_id),
                );
                IsolatedParse::Running(flight, "computed")
            }
            // Saturation is a deliberate refusal: bypassing the bounded
            // registry here would recreate the unbounded spawn_blocking queue
            // this guard is meant to prevent.
            OpenTokenFlightLease::Saturated => IsolatedParse::Ended(None, "saturated"),
        }
    }

    fn spawn_open_token_coordinator(
        &self,
        mut coordinator: OpenTokenFlightCoordinator,
        key: String,
        uri: Url,
        document: OpenTokenDocument,
        parent_id: Option<u64>,
    ) -> Arc<OpenTokenFlight> {
        let flight = Arc::clone(&coordinator.flight);
        let cache = Arc::clone(&self.open_token_cache);
        let state = Arc::clone(&self.state);
        let coordinator_started = std::time::Instant::now();
        let detached = tokio::task::spawn_blocking(move || {
            crate::llg_debug!(
                "event=semantic_tokens.coordinator.begin file={} bytes={} defines={} parent_id={:?}",
                document.real.display(),
                document.text.len(),
                document.defines.len(),
                parent_id
            );
            let current = || open_document_is_current(&state, &uri, &document.text);
            let cancel = &coordinator.flight.cancel;
            let result =
                open_document_identifiers_if_current(&document, current, cancel, parent_id);
            // Keep this post-compute check: a revision may change after the
            // final pre-frontend check, so stale results must never enter the
            // request cache. A frontend refusal is as much a function of the
            // inputs as a table is.
            let buffer_is_current = current();
            let publish = buffer_is_current && !matches!(&result, Err(error) if error.transient);
            if publish {
                cache.put(key, result.clone());
                if let Ok(identifiers) = &result {
                    let names = identifiers.declared_names();
                    state
                        .lock()
                        .unwrap_or_else(|error| error.into_inner())
                        .set_unsaved_names(uri, names);
                }
            }
            crate::llg_debug!(
                "event=semantic_tokens.coordinator.end outcome={} current={} cache_published={} elapsed_us={}",
                if result.is_ok() { "ok" } else { "error" },
                buffer_is_current,
                publish,
                coordinator_started.elapsed().as_micros()
            );
            coordinator.finish(result);
        });
        // The coordinator is deliberately detached.  Dropping this handle
        // does not abort a started `spawn_blocking` task, and the coordinator
        // remains responsible for waking followers on completion or panic.
        drop(detached);
        flight
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn declaration(line: u32) -> llg::core::tokens::TokenInfo {
        llg::core::tokens::TokenInfo {
            line,
            col: 1,
            end_line: line,
            end_col: 2,
            kind: llg::core::tokens::TOKEN_SLANG_MODULE
                + llg::core::tokens::TOKEN_DECLARATION_OFFSET,
            name: Some("m".to_owned()),
            file: "/r/a.sv".to_owned(),
        }
    }

    /// A request whose committed table declares `m` on line 1.
    fn committed() -> DocumentRequest {
        let mut analysis = features::empty_analysis();
        analysis.tokens.push(llg::core::tokens::FileTokens {
            path: "/r/a.sv".to_owned(),
            nodes: vec![declaration(1)],
        });
        DocumentRequest {
            table: DocumentTable::Committed,
            served: Some(Served {
                analysis: Arc::new(analysis),
                epoch: 1,
                texts: Arc::default(),
            }),
            placement: Placement::Same,
            path: "/r/a.sv".to_owned(),
            epoch: 1,
            outcome: "committed",
            root: RootKey::from("/r"),
            open: true,
        }
    }

    /// The lines of the tokens and of the definitions at columns 0 of lines 0
    /// to 9, which come from one table.
    fn answers(request: &DocumentRequest) -> (Vec<u32>, Vec<u32>) {
        let mut line = 0;
        let tokens = request
            .semantic_tokens()
            .data
            .iter()
            .map(|token| {
                line += token.delta_line;
                line
            })
            .collect();
        let definitions = (0..10)
            .filter(|line| {
                let position = Position::new(*line, 0);
                !request.definitions_at(&Arc::default(), position).is_empty()
            })
            .collect();
        (tokens, definitions)
    }

    #[test]
    fn a_current_isolated_table_answers_even_when_empty() {
        for (lines, expected) in [(vec![7], vec![6]), (Vec::new(), Vec::new())] {
            let mut request = committed();
            let identifiers = features::DocumentIdentifiers::new(
                lines.into_iter().map(declaration).collect(),
                Default::default(),
            );
            request.settle(Some(Arc::new(identifiers)), "computed", true);
            assert_eq!(request.outcome, "computed");
            assert_eq!(answers(&request), (expected.clone(), expected));
        }
    }

    #[test]
    fn a_stale_or_refused_isolated_parse_answers_from_the_committed_table() {
        let identifiers =
            features::DocumentIdentifiers::new(vec![declaration(7)], Default::default());
        for (isolated, outcome, current, served) in [
            (Some(Arc::new(identifiers)), "computed", false, "stale"),
            (None, "too-large", true, "too-large"),
        ] {
            let mut request = committed();
            request.settle(isolated, outcome, current);
            assert_eq!(request.outcome, served);
            assert_eq!(answers(&request), (vec![0], vec![0]));
        }
    }
}
