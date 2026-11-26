//! Positions between the text an analysis compiled and the text in the editor.
//!
//! An analysis starts on a save, so it describes an older text while a document
//! has unsaved edits. A request asks it at the analysed position of the place
//! the user sees ([`Placement::analysed`]), and what it names leaves the server at
//! its place in the current texts or not at all ([`Places::location`]). Sent
//! diagnostics move the same way ([`BackendState::place_diagnostics`]).
//!
//! The analysed text of an open document ([`AnalysedTexts`]) is the document's
//! own text, shared with it, until an edit replaces that. So a document is
//! unedited exactly if both are one allocation, and only an edited one pays for
//! a second text and, on the first request that needs it, for a map
//! ([`TextPair`]).

use super::*;
use crate::position_map::PositionMap;

/// Bytes of analysed texts that the server keeps for edited documents, beside
/// their current texts. Of a document beyond it, requests answer as if every
/// place in it were gone until its next analysis. A root of the default size
/// limits stays below it.
pub(super) const ANALYSED_TEXT_BUDGET: usize = 16 << 20;

/// The texts of the open documents.
#[derive(Debug, Default)]
pub(super) struct OpenTexts {
    texts: BTreeMap<Url, SharedText>,
    /// The documents that are files, by path: the analyses name paths, and the
    /// spelling of a URI is the client's.
    uris: HashMap<PathBuf, Url>,
    /// The same by the identity of the file ([`input_identity`]): an analysis
    /// may read a document through another spelling (a symbolic link).
    identities: HashMap<PathBuf, Url>,
    /// The identities of the documents opened through a symbolic link, by
    /// path.
    aliased: HashMap<PathBuf, PathBuf>,
    /// The documents whose latest text exceeds the size limit. The text here is
    /// the last one that did not, which the user no longer sees.
    rejected: HashSet<Url>,
}

impl OpenTexts {
    pub(super) fn insert(&mut self, uri: Url, text: SharedText) {
        if let Some(path) = Backend::uri_to_path(&uri) {
            // A change keeps the identity of the document.
            if !self.texts.contains_key(&uri) {
                let identity = input_identity(&path);
                self.identities.insert(identity.clone(), uri.clone());
                if identity != path {
                    self.aliased.insert(path.clone(), identity);
                }
            }
            self.uris.insert(path, uri.clone());
        }
        self.rejected.remove(&uri);
        self.texts.insert(uri, text);
    }

    pub(super) fn remove(&mut self, uri: &Url) -> Option<SharedText> {
        if let Some(path) = Backend::uri_to_path(uri) {
            self.uris.remove(&path);
            let identity = self.aliased.remove(&path).unwrap_or(path);
            if self.identities.get(&identity) == Some(uri) {
                self.identities.remove(&identity);
            }
        }
        self.rejected.remove(uri);
        self.texts.remove(uri)
    }

    /// The URI of the open document that is the file an analysis read as
    /// `path`, whose identity is `identity` when it differs from `path`.
    pub(super) fn document(&self, path: &Path, identity: Option<&PathBuf>) -> Option<&Url> {
        self.uris
            .get(path)
            .or_else(|| self.identities.get(identity.map_or(path, PathBuf::as_path)))
    }

    /// The identity of open document `path` when it was opened through a
    /// symbolic link.
    pub(super) fn aliased(&self, path: &Path) -> Option<&PathBuf> {
        self.aliased.get(path)
    }

    /// The identities of open documents `paths` that were opened through a
    /// symbolic link.
    pub(super) fn aliased_identities<'a>(
        &'a self,
        paths: impl Iterator<Item = &'a PathBuf>,
    ) -> HashSet<&'a PathBuf> {
        paths.filter_map(|path| self.aliased.get(path)).collect()
    }

    /// The last text of document `uri` within the size limit.
    pub(super) fn get(&self, uri: &Url) -> Option<&SharedText> {
        self.texts.get(uri)
    }

    pub(super) fn contains_key(&self, uri: &Url) -> bool {
        self.texts.contains_key(uri)
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = (&Url, &SharedText)> {
        self.texts.iter()
    }

    pub(super) fn keys(&self) -> impl Iterator<Item = &Url> {
        self.texts.keys()
    }

    /// Note that document `uri` has a text that is not here.
    pub(super) fn reject(&mut self, uri: &Url) {
        if self.texts.contains_key(uri) {
            self.rejected.insert(uri.clone());
        }
    }

    /// Whether document `uri` has a text that is not here.
    pub(super) fn is_rejected(&self, uri: &Url) -> bool {
        self.rejected.contains(uri)
    }

    /// The text of document `uri` that the user sees.
    pub(super) fn admitted(&self, uri: &Url) -> Option<&SharedText> {
        self.texts.get(uri).filter(|_| !self.is_rejected(uri))
    }

    pub(super) fn of_file(&self, path: &Path) -> Option<&SharedText> {
        self.texts.get(self.uris.get(path)?)
    }

    /// The open files.
    pub(super) fn files(&self) -> impl Iterator<Item = (&PathBuf, &SharedText)> {
        self.uris
            .iter()
            .filter_map(|(path, uri)| Some((path, self.texts.get(uri)?)))
    }
}

impl FromIterator<(Url, SharedText)> for OpenTexts {
    fn from_iter<T: IntoIterator<Item = (Url, SharedText)>>(texts: T) -> Self {
        let mut open = Self::default();
        for (uri, text) in texts {
            open.insert(uri, text);
        }
        open
    }
}

/// The texts of open documents that the `last_good` analysis of a root
/// compiled, by path.
pub(super) type AnalysedTexts = HashMap<PathBuf, SharedText>;

/// What the requests of a root are answered from.
#[derive(Clone)]
pub(super) struct Served {
    pub(super) analysis: Arc<Analysis>,
    pub(super) epoch: u64,
    pub(super) texts: Arc<AnalysedTexts>,
}

/// An analysed text and the one that replaced it, with the map between them
/// once a request needed it.
#[derive(Debug)]
pub(super) struct TextPair {
    analysed: SharedText,
    current: SharedText,
    map: OnceLock<PositionMap>,
}

impl TextPair {
    pub(super) fn map(&self) -> &PositionMap {
        self.map.get_or_init(|| {
            let started = std::time::Instant::now();
            let map = PositionMap::new(&self.analysed, Arc::clone(&self.current));
            crate::llg_debug!(
                "event=positions.map analysed_bytes={} current_bytes={} elapsed_us={}",
                self.analysed.len(),
                self.current.len(),
                started.elapsed().as_micros()
            );
            if !map.is_complete() {
                crate::llg_info!(
                    "event=positions.map outcome=capped analysed_bytes={} current_bytes={}: places among the edited lines are unknown until the next analysis",
                    self.analysed.len(),
                    self.current.len()
                );
            }
            map
        })
    }
}

/// How the positions of an analysis relate to those of a document now.
#[derive(Debug, Clone)]
pub(super) enum Placement {
    /// They are the same: the document holds the analysed text or is closed.
    Same,
    Edited(Arc<TextPair>),
    /// The server does not have the text that the analysis compiled, if any.
    Unknown,
}

impl Placement {
    pub(super) fn is_same(&self) -> bool {
        matches!(self, Self::Same)
    }

    /// Where the characters of an analysed `range` are now.
    pub(super) fn current(&self, range: Range) -> Option<Range> {
        match self {
            Self::Same => Some(range),
            Self::Edited(pair) => pair.map().current(range),
            Self::Unknown => None,
        }
    }

    /// Where the analysed identifier `name` is now.
    fn current_name(&self, range: Range, name: &str) -> Option<Range> {
        let place = self.current(range)?;
        let spelled = match self {
            Self::Edited(pair) => pair
                .map()
                .line(place.start.line)
                .is_some_and(|line| features::spells_identifier(line, place.start.character, name)),
            _ => true,
        };
        spelled.then_some(place)
    }

    /// Where an analysed stretch of text is now that began with a place which
    /// is `start` now.
    pub(super) fn current_stretch(&self, range: Range, start: Range) -> Range {
        let end = match self {
            Self::Edited(pair) => pair.map().current_end(range.end),
            _ => Some(range.end),
        };
        Range::new(start.start, start.end.max(end.unwrap_or(start.end)))
    }

    /// The analysed position that a request at `position` of the document asks
    /// about: in an edited document one at which the analysis has an identifier
    /// that the document has here as well.
    pub(super) fn analysed(
        &self,
        analysis: &Analysis,
        path: &str,
        position: Position,
    ) -> Option<Position> {
        match self {
            Self::Same => Some(position),
            Self::Unknown => None,
            Self::Edited(pair) => pair.map().analysed(position).find(|place| {
                features::identifier_at(analysis, path, place.line, place.character)
                    .and_then(|token| {
                        let range = features::token_range(token);
                        self.current_name(range, token.name.as_deref()?)
                    })
                    .is_some_and(|name| name.start <= position && position <= name.end)
            }),
        }
    }
}

impl RootState {
    pub(super) fn served(&self) -> Option<Served> {
        Some(Served {
            analysis: Arc::clone(self.last_good.as_ref()?),
            epoch: self.analysis_epoch,
            texts: Arc::clone(&self.analysed_texts),
        })
    }

    /// Whether `last_good` compiled a text with this fingerprint as `path`.
    pub(super) fn compiled(&self, path: &Path, text: features::SourceFingerprint) -> bool {
        match (&self.last_good, path.to_str()) {
            (Some(analysis), Some(path)) => analysis.compiled_from(path, text),
            _ => false,
        }
    }
}

impl BackendState {
    /// The URI of the open document that is the file which an analysis names
    /// `path`.
    pub(super) fn document_of_file(&self, path: &Path) -> Option<&Url> {
        let identity = self
            .roots
            .values()
            .find_map(|root| root.input_aliases.get(path));
        self.documents.document(path, identity)
    }

    /// The name by which `analysis`, the last of root `root`, read the file of
    /// open document `path`, when that is another spelling of it: either went
    /// through a symbolic link.
    fn analysed_name(&self, root: &RootState, analysis: &Analysis, path: &Path) -> Option<String> {
        let identity = self.documents.aliased(path).map_or(path, PathBuf::as_path);
        let aliases = root
            .input_aliases
            .iter()
            .filter(|(_, target)| target.as_path() == identity)
            .map(|(name, _)| name.as_path());
        std::iter::once(identity)
            .chain(aliases)
            .filter(|name| *name != path)
            .find(|name| analysis.compiled_length(name).is_some())
            .and_then(Path::to_str)
            .map(str::to_owned)
    }

    /// How the positions that `texts` belong to relate to those of the file
    /// `path` now.
    pub(super) fn placement(&mut self, texts: &AnalysedTexts, path: &Path) -> Placement {
        let Some(uri) = self.documents.uris.get(path) else {
            return Placement::Same;
        };
        let (Some(analysed), Some(current)) = (texts.get(path), self.documents.admitted(uri))
        else {
            return Placement::Unknown;
        };
        if Arc::ptr_eq(analysed, current) {
            return Placement::Same;
        }
        let pairs = match self.text_pairs.get_mut(path) {
            Some(pairs) => pairs,
            None => self.text_pairs.entry(path.to_path_buf()).or_default(),
        };
        let known = pairs.iter().find(|pair| {
            Arc::ptr_eq(&pair.analysed, analysed) && Arc::ptr_eq(&pair.current, current)
        });
        let pair = match known {
            Some(pair) => Arc::clone(pair),
            None => {
                let pair = Arc::new(TextPair {
                    analysed: Arc::clone(analysed),
                    current: Arc::clone(current),
                    map: OnceLock::new(),
                });
                pairs.push(Arc::clone(&pair));
                pair
            }
        };
        Placement::Edited(pair)
    }

    /// Make `text` the text of open document `uri`. One that an analysis
    /// compiled is shared with it again.
    pub(super) fn set_document(&mut self, uri: Url, text: String) {
        let path = Backend::uri_to_path(&uri);
        let analysed = path.as_ref().and_then(|path| {
            self.roots
                .values()
                .filter_map(|root| root.analysed_texts.get(path))
                .find(|analysed| ***analysed == text)
                .map(Arc::clone)
        });
        let text = analysed.unwrap_or_else(|| Arc::new(text));
        self.documents.insert(uri, Arc::clone(&text));
        let Some(path) = path else {
            return;
        };
        self.text_pairs.remove(&path);
        let (mut identity, mut fingerprint) = (None, None);
        for root in self.roots.values_mut() {
            if root.analysed_texts.contains_key(&path) {
                continue;
            }
            let Some(analysis) = &root.last_good else {
                continue;
            };
            // Slang may have read the file by another path.
            let length = analysis.compiled_length(&path).or_else(|| {
                let identity = identity.get_or_insert_with(|| input_identity(&path));
                analysis.compiled_length(identity)
            });
            if length != Some(text.len()) {
                continue;
            }
            let fingerprint =
                *fingerprint.get_or_insert_with(|| features::SourceFingerprint::of(&text));
            if root.compiled(&path, fingerprint) {
                Arc::make_mut(&mut root.analysed_texts).insert(path.clone(), Arc::clone(&text));
            }
        }
        self.enforce_text_budget(&path);
    }

    /// Forget open document `uri`.
    pub(super) fn remove_document(&mut self, uri: &Url) -> Option<SharedText> {
        let text = self.documents.remove(uri)?;
        self.unsaved_names.remove(uri);
        if let Some(path) = Backend::uri_to_path(uri) {
            self.text_pairs.remove(&path);
            for root in self.roots.values_mut() {
                if root.analysed_texts.contains_key(&path) {
                    Arc::make_mut(&mut root.analysed_texts).remove(&path);
                }
            }
        }
        Some(text)
    }

    /// Note the names that the isolated parse of open document `uri` found.
    pub(super) fn set_unsaved_names(&mut self, uri: Url, names: Vec<features::DeclaredName>) {
        if self.documents.contains_key(&uri) {
            self.unsaved_names.insert(uri, Arc::new(names));
        }
    }

    /// Note the texts of the open documents that the new `last_good` analysis
    /// of root `key` compiled, of those it read. Equal texts of a document are
    /// one allocation, whichever root read them.
    pub(super) fn set_analysed_texts(
        &mut self,
        key: &RootKey,
        mut read: BTreeMap<PathBuf, SharedText>,
    ) {
        read.retain(|path, _| self.documents.of_file(path).is_some());
        let mut analysed = AnalysedTexts::with_capacity(read.len());
        for (path, text) in read {
            let others = self
                .roots
                .iter()
                .filter(|(other, _)| *other != key)
                .filter_map(|(_, root)| root.analysed_texts.get(&path));
            let known = self
                .documents
                .of_file(&path)
                .into_iter()
                .chain(others)
                .find(|known| Arc::ptr_eq(known, &text) || **known == text);
            let text = known.map_or(text, Arc::clone);
            analysed.insert(path, text);
        }
        let paths: Vec<PathBuf> = analysed.keys().cloned().collect();
        if let Some(root) = self.roots.get_mut(key) {
            root.analysed_texts = Arc::new(analysed);
        }
        self.text_pairs.clear();
        for path in paths {
            self.enforce_text_budget(&path);
        }
        // Every name of an unedited document is the analysis' own.
        let (documents, roots) = (&self.documents, &self.roots);
        self.unsaved_names.retain(|uri, _| {
            let path = Backend::uri_to_path(uri);
            let text = documents.get(uri);
            !path.zip(text).is_some_and(|(path, text)| {
                roots
                    .values()
                    .filter_map(|root| root.analysed_texts.get(&path))
                    .any(|analysed| Arc::ptr_eq(analysed, text))
            })
        });
    }

    /// Move `diagnostics` that the last analysis of root `key` found in `file`
    /// to their places in the current texts, when they are sent: digests and
    /// unions compare them at the places of the analysed texts, which the
    /// client never sees. A range whose text an edit touched stays where the
    /// analysis had it: the client shows the finding at the place of the
    /// analysed text rather than not at all, and the next analysis replaces it.
    pub(super) fn place_diagnostics(
        &mut self,
        key: &RootKey,
        file: &Path,
        diagnostics: &mut [Diagnostic],
    ) {
        let texts = match self.roots.get(key) {
            Some(root) if root.diagnosed_analysed_texts => Arc::clone(&root.analysed_texts),
            _ => return,
        };
        let placement = self.placement(&texts, file);
        for diagnostic in diagnostics {
            if let Some(range) = placement.current(diagnostic.range) {
                diagnostic.range = range;
            }
            for related in diagnostic.related_information.iter_mut().flatten() {
                let Some(path) = Backend::uri_to_path(&related.location.uri) else {
                    continue;
                };
                let range = self
                    .placement(&texts, &path)
                    .current(related.location.range);
                if let Some(range) = range {
                    related.location.range = range;
                }
            }
        }
    }

    /// Drop the analysed texts of the document `path`, which was edited, when
    /// those of the edited documents exceed the budget.
    fn enforce_text_budget(&mut self, path: &Path) {
        let kept: usize = self
            .documents
            .files()
            .flat_map(|(path, current)| {
                self.roots
                    .values()
                    .filter_map(move |root| root.analysed_texts.get(path))
                    .filter(move |analysed| !Arc::ptr_eq(analysed, current))
            })
            .map(|analysed| analysed.len())
            .sum();
        if kept <= ANALYSED_TEXT_BUDGET {
            return;
        }
        let current = self.documents.of_file(path);
        for root in self.roots.values_mut() {
            let edited = root
                .analysed_texts
                .get(path)
                .is_some_and(|analysed| current.is_none_or(|text| !Arc::ptr_eq(analysed, text)));
            if edited {
                Arc::make_mut(&mut root.analysed_texts).remove(path);
                crate::llg_info!(
                    "event=positions.budget outcome=dropped root={} path={} bytes={} budget={}: positions in the edited document are unknown until its next analysis",
                    root.descriptor.root.display(),
                    path.display(),
                    kept,
                    ANALYSED_TEXT_BUDGET
                );
            }
        }
    }
}

/// What a request about one document is answered from.
pub(super) struct Query {
    /// `None` before the first analysis.
    pub(super) served: Option<Served>,
    pub(super) path: String,
    /// How the analysis' positions in the document moved.
    pub(super) placement: Placement,
    /// Whether no request of the root needs a map: the open documents hold the
    /// analysed texts.
    pub(super) settled: bool,
    pub(super) max_file_bytes: u64,
    pub(super) root: RootKey,
}

impl Query {
    /// [`Placement::analysed`] of the document.
    pub(super) async fn analysed(&self, position: Position) -> Option<Position> {
        match &self.placement {
            Placement::Same => Some(position),
            Placement::Unknown => None,
            Placement::Edited(_) => {
                let served = self.served.clone()?;
                let (placement, path) = (self.placement.clone(), self.path.clone());
                blocking("position", move || {
                    placement.analysed(&served.analysis, &path, position)
                })
                .await
                .flatten()
            }
        }
    }

    /// The id of the root that answers for a document which several roots
    /// track.
    pub(super) fn shared_owner(&self, state: &BackendState) -> Option<String> {
        let root = state.roots.get(&self.root)?;
        (shared_tracker_count(state, Path::new(&self.path)) > 1).then(|| root.descriptor.id.clone())
    }
}

impl Backend {
    /// What requests about `uri` are answered from now. `None` for a document
    /// that no root owns.
    pub(super) fn query(
        &self,
        uri: &Url,
        request: &mut crate::logging::LifecycleSpan,
    ) -> Option<Query> {
        let mut state = self.lock_state();
        let (root, real, paths) = Self::root_context(&state, uri)?;
        request.set_root(|| root.descriptor.id.clone());
        let path = paths.into_iter().next()?;
        let max_file_bytes = root.descriptor.effective_config().analysis.max_file_bytes;
        let key = root.descriptor.root.clone();
        let served = root.served();
        // The analysis answers by its own name of the file.
        let path = served
            .as_ref()
            .and_then(|served| state.analysed_name(root, &served.analysis, &real))
            .unwrap_or(path);
        let placement = match &served {
            Some(served) => state.placement(&served.texts, &real),
            None => Placement::Unknown,
        };
        let settled = served.as_ref().is_none_or(|served| {
            served.texts.iter().all(|(path, analysed)| {
                let current = state.documents.of_file(path);
                current.is_some_and(|current| Arc::ptr_eq(current, analysed))
            })
        });
        Some(Query {
            served,
            path,
            placement,
            settled,
            max_file_bytes,
            root: key,
        })
    }
}

/// `symbols` of a document at their places in its current text. A symbol whose
/// name is gone leaves those it contained.
pub(super) fn current_symbols(
    placement: &Placement,
    symbols: Vec<DocumentSymbol>,
) -> Vec<DocumentSymbol> {
    if placement.is_same() {
        return symbols;
    }
    let mut placed = Vec::with_capacity(symbols.len());
    for mut symbol in symbols {
        let children = current_symbols(placement, symbol.children.take().unwrap_or_default());
        match placement.current(symbol.selection_range) {
            Some(name) => {
                symbol.range = placement.current_stretch(symbol.range, name);
                symbol.selection_range = name;
                symbol.children = (!children.is_empty()).then_some(children);
                placed.push(symbol);
            }
            None => placed.extend(children),
        }
    }
    placed
}

/// The symbols that match `query`, once per place.
pub(super) fn workspace_symbols(
    roots: Vec<(Served, Places)>,
    query: &str,
) -> Vec<SymbolInformation> {
    let mut seen = HashSet::new();
    let mut symbols = Vec::new();
    for (served, mut places) in roots {
        for mut symbol in features::workspace_symbols(&served.analysis, query) {
            let Some(location) = places.location(Some(&symbol.name), &symbol.location) else {
                continue;
            };
            let start = pos_key(location.range.start);
            if seen.insert((location.uri.clone(), start)) {
                symbol.location = location;
                symbols.push(symbol);
            }
        }
        places.log("workspace/symbol", symbols.len());
    }
    symbols
}

/// Maps what the analysis of one request names to the current texts.
pub(super) struct Places {
    state: Arc<Mutex<BackendState>>,
    served: Served,
    /// By the URI that the analysis names: the one the client knows, and how
    /// positions in it moved.
    files: HashMap<Url, (Url, Placement)>,
    dropped: usize,
}

impl Places {
    pub(super) fn new(state: &Arc<Mutex<BackendState>>, served: &Served) -> Self {
        Self {
            state: Arc::clone(state),
            served: served.clone(),
            files: HashMap::new(),
            dropped: 0,
        }
    }

    /// What is named in `uri` is named in its current text.
    pub(super) fn hold_in(&mut self, uri: Url) {
        self.files.insert(uri.clone(), (uri, Placement::Same));
    }

    /// The URI that the client knows for one of the analysis, and how positions
    /// in it moved.
    fn file(&mut self, uri: &Url) -> &(Url, Placement) {
        if !self.files.contains_key(uri) {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            let known = Backend::map_uri(&state, uri);
            let placement = match Backend::uri_to_path(&known) {
                Some(path) => state.placement(&self.served.texts, &path),
                None => Placement::Same,
            };
            drop(state);
            self.files.insert(uri.clone(), (known, placement));
        }
        &self.files[uri]
    }

    /// Where the identifier `name` is now that the analysis has at `location`.
    /// A location without a name is the start of a file.
    pub(super) fn location(&mut self, name: Option<&str>, location: &Location) -> Option<Location> {
        let (uri, placement) = self.file(&location.uri);
        let range = match name {
            Some(name) => placement.current_name(location.range, name),
            None => Some(location.range),
        };
        let location = range.map(|range| Location::new(uri.clone(), range));
        self.dropped += usize::from(location.is_none());
        location
    }

    /// Where the identifier `name` starts now that the analysis has at `start`
    /// of `file`.
    pub(super) fn name(&mut self, file: &str, name: &str, start: Position) -> Option<Position> {
        let Ok(uri) = Url::from_file_path(file) else {
            // No document has that name.
            return Some(start);
        };
        let nodes = features::tokens_of(&self.served.analysis, file);
        let range = features::identifier_range(nodes, start, name);
        let place = self.location(Some(name), &Location::new(uri, range))?;
        Some(place.range.start)
    }

    /// The current line of a declaration.
    pub(super) fn line(&mut self, target: &llg::core::tokens::DeclTarget) -> Option<u32> {
        let nodes = features::tokens_of(&self.served.analysis, &target.file);
        let target = features::target_location(target, nodes);
        let location = self.location(target.name.as_deref(), &target.location)?;
        Some(location.range.start.line)
    }

    /// Name the file and range of a module explorer entry as the client knows
    /// them. The range begins with a name and may go on after it; it is gone
    /// with the name.
    pub(super) fn relocate(
        &mut self,
        uri: &mut Option<String>,
        range: &mut Option<module_explorer::ExplorerRange>,
    ) {
        let Some(named) = uri.as_deref().and_then(|uri| Url::parse(uri).ok()) else {
            return;
        };
        let (known, placement) = self.file(&named).clone();
        if let Some(place) = range.take() {
            *range = match &placement {
                Placement::Same => Some(place),
                Placement::Edited(_) => self.stretch(&named, &placement, place),
                Placement::Unknown => None,
            };
            self.dropped += usize::from(range.is_none());
        }
        *uri = Some(known.into());
    }

    /// Where the analysed `stretch` of text is now that begins with a name.
    fn stretch(
        &self,
        uri: &Url,
        placement: &Placement,
        stretch: module_explorer::ExplorerRange,
    ) -> Option<module_explorer::ExplorerRange> {
        let start = Position::new(stretch.start_line, stretch.start_character);
        let path = Backend::uri_to_path(uri)?;
        let token = features::identifier_at(
            &self.served.analysis,
            path.to_str()?,
            start.line,
            start.character,
        )?;
        let name = features::token_range(token);
        let mut place = placement.current_name(name, token.name.as_deref()?)?;
        let end = Position::new(stretch.end_line, stretch.end_character);
        if end <= name.end {
            place.end.character -= name.end.character - end.character;
        } else {
            place = placement.current_stretch(Range::new(start, end), place);
        }
        Some(module_explorer::ExplorerRange {
            start_line: place.start.line,
            start_character: place.start.character,
            end_line: place.end.line,
            end_character: place.end.character,
        })
    }

    /// Log how many results besides the `kept` ones had no place any more.
    pub(super) fn log(&self, request: &str, kept: usize) {
        if self.dropped != 0 {
            crate::llg_debug!(
                "event=positions.dropped request={request} kept={kept} dropped={}",
                self.dropped
            );
        }
    }
}

#[cfg(test)]
mod tests;
