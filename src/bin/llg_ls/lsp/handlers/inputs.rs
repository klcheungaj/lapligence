//! What the newest analysis of a root read, and which events call for another.
//!
//! The newest analysis is the running one, else the last that completed. An
//! event starts an analysis only when that one read something else than one
//! started now would, so that the several notifications of one write (a save,
//! the file watcher's event) start one analysis, and opening or closing an
//! unchanged file starts none. A run that is still to start, after the delay or
//! after the running one ends, reads the newest state anyway.

use super::*;
use features::SourceFingerprint;
use llg::cancel::CancelToken;

/// The files one analysis of a root read.
#[derive(Debug, Default)]
pub(super) struct AnalysisInputs {
    units: BTreeSet<PathBuf>,
    /// The texts of the open documents of the root, whether the analysis read
    /// them or not, and of the units and includes it read from the disk.
    texts: HashMap<PathBuf, SourceFingerprint>,
}

impl AnalysisInputs {
    pub(super) fn new(units: &[PathBuf], open: &OpenDocuments) -> Self {
        Self {
            units: units.iter().cloned().collect(),
            texts: open
                .iter()
                .map(|(path, text)| (path.clone(), SourceFingerprint::of(text)))
                .collect(),
        }
    }
}

/// The texts of `snapshots` that were read from the disk.
pub(super) fn closed_texts(
    snapshots: &InputSnapshots,
    open: &OpenDocuments,
) -> Vec<(PathBuf, SourceFingerprint)> {
    snapshots
        .by_path
        .iter()
        .filter(|(path, _)| !open.contains_key(*path))
        .map(|(path, text)| (path.clone(), SourceFingerprint::of(text)))
        .collect()
}

/// A file that was written, or whose document was closed, and the roots whose
/// analysis may read it.
pub(super) struct FileWrite {
    pub(super) path: PathBuf,
    /// It did not exist before.
    pub(super) created: bool,
    pub(super) readers: BTreeSet<RootKey>,
}

/// The analysis that a root's running job holds. Dropping it, as the removal of
/// the root does, cancels the analysis.
#[derive(Debug)]
pub(super) struct RunningAnalysis {
    cancel: CancelToken,
    inputs: AnalysisInputs,
    /// The frontend reads `inputs`, which now include the closed files.
    admitted: bool,
}

impl RunningAnalysis {
    pub(super) fn new(cancel: CancelToken, inputs: AnalysisInputs) -> Self {
        Self {
            cancel,
            inputs,
            admitted: false,
        }
    }

    pub(super) fn cancel(&self) {
        self.cancel.cancel();
    }

    /// The job admitted these texts beside the open documents.
    pub(super) fn admit(&mut self, closed: Vec<(PathBuf, SourceFingerprint)>) {
        self.inputs.texts.extend(closed);
        self.admitted = true;
    }

    /// What the finished analysis read. Inputs that were rejected before the
    /// frontend read them may be accepted as they are the next time.
    pub(super) fn finish(mut self) -> Option<AnalysisInputs> {
        self.admitted.then(|| std::mem::take(&mut self.inputs))
    }
}

impl Drop for RunningAnalysis {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

impl RootState {
    /// What the newest analysis read; `None` before the first one.
    fn newest_inputs(&self) -> Option<&AnalysisInputs> {
        self.running
            .as_ref()
            .map(|running| &running.inputs)
            .or(self.analysed.as_ref())
    }

    /// Whether `path` is a compilation unit of this root, as a discovered file
    /// or as a document that is `open`. Longest-root ownership is
    /// authoritative: a file whose structural owner is another (say, nested)
    /// root is not compiled by this one, even when its discovery filters would
    /// include it.
    fn is_unit(
        &self,
        descriptors: &[RootDescriptor],
        config: &LlgConfig,
        path: &Path,
        open: bool,
    ) -> bool {
        let Some(owner) = workspace::owning_root_unfiltered(path, descriptors)
            .filter(|owner| owner.root == self.descriptor.root)
        else {
            return false;
        };
        if open && config::is_compilation_unit(path) {
            // Same relative base as discovery time (source-dir-relative),
            // falling back to the ownership-relative path for open units
            // outside every source directory.
            let relative =
                Backend::source_relative_path(config, path).unwrap_or(owner.relative_path);
            if self.descriptor.allows_file(&relative) {
                return true;
            }
        }
        self.discovered.iter().any(|file| file == path)
    }

    /// The compilation units of this root while `open` holds its open documents.
    pub(super) fn units(
        &self,
        descriptors: &[RootDescriptor],
        config: &LlgConfig,
        open: &OpenDocuments,
    ) -> BTreeSet<PathBuf> {
        let discovered = self.discovered.iter().filter(|file| {
            workspace::owning_root_unfiltered(file, descriptors)
                .is_some_and(|owner| owner.root == self.descriptor.root)
        });
        let open = open
            .keys()
            .filter(|path| self.is_unit(descriptors, config, path, true));
        discovered.chain(open).cloned().collect()
    }

    /// Whether the newest analysis read `text` in open document `path`, in the
    /// role the document has now.
    fn covers(
        &self,
        descriptors: &[RootDescriptor],
        config: &LlgConfig,
        path: &Path,
        text: &str,
    ) -> bool {
        self.newest_inputs().is_some_and(|newest| {
            newest.texts.get(path) == Some(&SourceFingerprint::of(text))
                && newest.units.contains(path) == self.is_unit(descriptors, config, path, true)
        })
    }
}

impl BackendState {
    pub(super) fn descriptors(&self) -> Vec<RootDescriptor> {
        self.roots
            .values()
            .map(|root| root.descriptor.clone())
            .collect()
    }

    /// The open documents that `root` owns.
    pub(super) fn open_documents(
        &self,
        root: &RootState,
        descriptors: &[RootDescriptor],
    ) -> OpenDocuments {
        self.documents
            .iter()
            .filter_map(|(uri, text)| {
                let path = Backend::uri_to_path(uri)?;
                let owner = workspace::owning_root_unfiltered(&path, descriptors)?;
                (owner.root == root.descriptor.root).then(|| (path, Arc::clone(text)))
            })
            .collect()
    }

    /// Whether the newest analysis of root `key` read open document `uri`,
    /// which the root owns, as it is now.
    pub(super) fn document_covered(&self, key: &RootKey, uri: &Url) -> bool {
        let Some(root) = self.roots.get(key) else {
            return true;
        };
        let (Some(text), Some(path)) = (self.documents.get(uri), Backend::uri_to_path(uri)) else {
            return true;
        };
        root.covers(
            &self.descriptors(),
            &root.descriptor.effective_config(),
            &path,
            text,
        )
    }

    /// Whether the newest analysis of root `key` read every open document of
    /// the root as it is now.
    pub(super) fn documents_covered(&self, key: &RootKey) -> bool {
        let Some(root) = self.roots.get(key) else {
            return true;
        };
        let descriptors = self.descriptors();
        let config = root.descriptor.effective_config();
        self.open_documents(root, &descriptors)
            .iter()
            .all(|(path, text)| root.covers(&descriptors, &config, path, text))
    }

    /// Whether the newest analysis of root `key` is out of date now that file
    /// `path`, which is not an open document of the root, holds `disk`. One
    /// that appeared (`created`) may satisfy an include.
    fn file_outdates(
        &self,
        key: &RootKey,
        path: &Path,
        disk: Option<SourceFingerprint>,
        created: bool,
    ) -> bool {
        let Some(root) = self.roots.get(key) else {
            return false;
        };
        let Some(newest) = root.newest_inputs() else {
            return true;
        };
        let was_unit = newest.units.contains(path);
        let config = root.descriptor.effective_config();
        was_unit != root.is_unit(&self.descriptors(), &config, path, false)
            || match newest.texts.get(path) {
                Some(read) => Some(*read) != disk,
                // The analysis that names the file admitted no text of it.
                None => created || was_unit || root.include_deps.contains(path),
            }
    }

    /// Whether the write of `path`, which left `disk` there, calls for another
    /// analysis of root `key`. The text of an open document of the root is the
    /// client's, which takes a write over with a change notification.
    fn write_outdates(
        &mut self,
        key: &RootKey,
        path: &Path,
        disk: Option<SourceFingerprint>,
        created: bool,
    ) -> bool {
        let owns = workspace::owning_root_unfiltered(path, &self.descriptors())
            .is_some_and(|owner| owner.root == *key);
        let document = self
            .documents
            .iter()
            .find(|(uri, _)| owns && Backend::uri_to_path(uri).as_deref() == Some(path))
            .map(|(uri, text)| (uri.clone(), SourceFingerprint::of(text)));
        match document {
            None => self.file_outdates(key, path, disk, created),
            Some((uri, text)) if Some(text) == disk => !self.document_covered(key, &uri),
            Some((uri, _)) => {
                if let Some(disk) = disk {
                    self.reloads.insert(uri, disk);
                }
                false
            }
        }
    }

    /// Whether the text of `uri` is what a watched-file event found on the disk
    /// while the document held another; forgets the event then.
    pub(super) fn take_reload(&mut self, uri: &Url) -> bool {
        let reloaded = match (self.reloads.get(uri), self.documents.get(uri)) {
            (Some(disk), Some(text)) => *disk == SourceFingerprint::of(text),
            _ => false,
        };
        if reloaded {
            self.reloads.remove(uri);
        }
        reloaded
    }

    /// Whether the units of root `key` are others than those the newest
    /// analysis compiled.
    pub(super) fn units_changed(&self, key: &RootKey) -> bool {
        let Some(root) = self.roots.get(key) else {
            return false;
        };
        let Some(newest) = root.newest_inputs() else {
            return true;
        };
        let descriptors = self.descriptors();
        let config = root.descriptor.effective_config();
        let open = self.open_documents(root, &descriptors);
        newest.units != root.units(&descriptors, &config, &open)
    }
}

impl Backend {
    /// The roots that call for another analysis after `writes`.
    pub(super) async fn roots_outdated_by(&self, writes: Vec<FileWrite>) -> BTreeSet<RootKey> {
        let state = Arc::clone(&self.state);
        tokio::task::spawn_blocking(move || {
            let mut outdated = BTreeSet::new();
            for FileWrite {
                path,
                created,
                mut readers,
            } in writes
            {
                let max_file_bytes = {
                    let state = state.lock().unwrap_or_else(|error| error.into_inner());
                    let open = state
                        .documents
                        .keys()
                        .any(|uri| Backend::uri_to_path(uri).as_ref() == Some(&path));
                    // The write of an open document is noted in any case.
                    readers.retain(|key| open || !outdated.contains(key));
                    readers
                        .iter()
                        .filter_map(|key| state.roots.get(key))
                        .map(|root| root.descriptor.effective_config().analysis.max_file_bytes)
                        .max()
                };
                let Some(max_file_bytes) = max_file_bytes else {
                    continue;
                };
                let disk = read_closed_input_snapshot(&path, max_file_bytes)
                    .ok()
                    .map(|read| SourceFingerprint::of(&read.text));
                let mut state = state.lock().unwrap_or_else(|error| error.into_inner());
                for key in readers {
                    if state.write_outdates(&key, &path, disk, created) {
                        outdated.insert(key);
                    }
                }
            }
            outdated
        })
        .await
        .unwrap_or_default()
    }
}
