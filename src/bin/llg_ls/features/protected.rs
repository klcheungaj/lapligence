//! Protected (encrypted) source as opaque regions (IEEE 1800-2009 clause 34).
//!
//! The frontend skips every key, digest and data block of a decryption
//! envelope and parses the text around it; the capture reports each envelope
//! with the declaration that holds it ([`ProtectedEnvelope`]). Editing a
//! design that uses encrypted IP must not light up errors the user cannot
//! fix, so the analysis treats the protected text as a black box:
//!
//! - The frontend's per-block "cannot be decrypted" warnings are replaced by
//!   one hint per decryption envelope.
//! - A declaration whose text holds an envelope is protected. Its plaintext
//!   header (name, parameters, ports) stays an ordinary definition; Slang
//!   analysis findings and lint findings inside it are dropped because the
//!   hidden text may use or drive anything it declares.
//! - Errors caused by the hidden text become hints that say so: the missing
//!   end keyword of a declaration that the envelope finishes, a hierarchical
//!   name that continues into an instance of a protected definition, and,
//!   when an envelope lies outside every declaration (whole definitions are
//!   protected), an unknown module that such an envelope may define.
//!
//! Encryption envelopes (`pragma protect begin`/`end`) hold plaintext and need
//! no special handling. Everything here reads the owned snapshot.

use super::*;
use llg::ffi::slang::{
    DiagnosticProvider, DiagnosticSeverity as SlangSeverity, DiagnosticSubsystem, LexicalKind,
    ProtectedEnvelope, ProtectedEnvelopeKind, SemanticKind, Snapshot, SourceRange,
};

/// Diagnostic code of the per-envelope hint.
pub(crate) const PROTECTED_ENVELOPE_CODE: &str = "llg.protected-envelope";

/// What the analysis does with one frontend diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Disposition {
    Keep,
    Drop,
    /// Report as a hint with the reason appended.
    Hint(HintReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HintReason {
    MissingEnd,
    HiddenMember,
    OpaqueDefinition,
}

impl HintReason {
    fn text(self) -> &'static str {
        match self {
            Self::MissingEnd => "the end of this declaration is inside protected source",
            Self::HiddenMember => "the name continues into protected source",
            Self::OpaqueDefinition => "the definition may be inside protected source",
        }
    }
}

/// The message of a diagnostic reported as a hint for `reason`.
pub(super) fn hint_message(message: &str, reason: HintReason) -> String {
    format!("{message} ({})", reason.text())
}

/// Protected regions of one snapshot.
pub(super) struct ProtectedSource<'a> {
    snapshot: &'a Snapshot,
    /// Decryption envelopes, in snapshot order (by file and start).
    envelopes: Vec<&'a ProtectedEnvelope>,
    /// Name ranges of the protected declarations, with the end offset of
    /// their last envelope.
    protected_scopes: HashMap<(u64, u64, u64), u64>,
    /// Names of protected definitions.
    protected_names: HashSet<&'a str>,
    /// Some decryption envelope lies outside every declaration.
    opaque: bool,
}

impl<'a> ProtectedSource<'a> {
    pub(super) fn new(snapshot: &'a Snapshot) -> Self {
        let envelopes: Vec<_> = snapshot
            .protected_envelopes
            .iter()
            .filter(|envelope| envelope.kind == ProtectedEnvelopeKind::Decryption)
            .collect();
        let mut protected_scopes = HashMap::new();
        let mut opaque = false;
        for envelope in &envelopes {
            match envelope.scope {
                Some(scope) => {
                    let end = protected_scopes
                        .entry(range_key(scope))
                        .or_insert(envelope.range.end);
                    *end = (*end).max(envelope.range.end);
                }
                None => opaque = true,
            }
        }
        let protected_names = if protected_scopes.is_empty() {
            HashSet::new()
        } else {
            snapshot
                .semantic_nodes
                .iter()
                .filter(|node| node.kind == SemanticKind::Definition)
                .filter(|node| {
                    node.range
                        .is_some_and(|range| protected_scopes.contains_key(&range_key(range)))
                })
                .map(|node| &*node.name)
                .collect()
        };
        Self {
            snapshot,
            envelopes,
            protected_scopes,
            protected_names,
            opaque,
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.envelopes.is_empty()
    }

    /// One disposition per `snapshot.diagnostics` entry.
    pub(super) fn dispositions(&self) -> Vec<Disposition> {
        if self.is_empty() {
            return vec![Disposition::Keep; self.snapshot.diagnostics.len()];
        }
        let mut hierarchy = None;
        self.snapshot
            .diagnostics
            .iter()
            .map(|diagnostic| {
                if compile::is_protected_envelope_warning(diagnostic) {
                    return Disposition::Drop;
                }
                let Some(primary) = diagnostic.primary else {
                    return Disposition::Keep;
                };
                if diagnostic.provider == DiagnosticProvider::Analysis {
                    return if self.inside_protected_scope(primary) {
                        Disposition::Drop
                    } else {
                        Disposition::Keep
                    };
                }
                if !matches!(
                    diagnostic.severity,
                    SlangSeverity::Error | SlangSeverity::Fatal
                ) {
                    return Disposition::Keep;
                }
                match (diagnostic.subsystem, diagnostic.name.as_str()) {
                    (DiagnosticSubsystem::Parser, "ExpectedToken")
                        if self.only_protected_text_after(primary) =>
                    {
                        Disposition::Hint(HintReason::MissingEnd)
                    }
                    (_, "UnknownModule") if self.opaque => {
                        Disposition::Hint(HintReason::OpaqueDefinition)
                    }
                    (_, "CouldNotResolveHierarchicalPath") => {
                        let hierarchy =
                            hierarchy.get_or_insert_with(|| Hierarchy::new(self.snapshot));
                        if self.path_enters_protected(hierarchy, primary) {
                            Disposition::Hint(HintReason::HiddenMember)
                        } else {
                            Disposition::Keep
                        }
                    }
                    _ => Disposition::Keep,
                }
            })
            .collect()
    }

    /// Whether `range` lies in a protected declaration, from its name to the
    /// end of its last envelope.
    fn inside_protected_scope(&self, range: SourceRange) -> bool {
        self.protected_scopes
            .iter()
            .any(|(&(file, start, _), &end)| {
                file == range.file_id && start <= range.start && range.start <= end
            })
    }

    /// Byte extents of the protected declarations, per file id.
    pub(super) fn protected_extents(&self) -> impl Iterator<Item = (u64, u64, u64)> + '_ {
        self.protected_scopes
            .iter()
            .map(|(&(file, start, _), &end)| (file, start, end))
    }

    /// Whether a decryption envelope follows `range` in its file and every
    /// source token from there to the end of the file is inside an envelope:
    /// the text the parser expected is protected.
    fn only_protected_text_after(&self, range: SourceRange) -> bool {
        let in_file = |envelope: &&&ProtectedEnvelope| envelope.range.file_id == range.file_id;
        if !self
            .envelopes
            .iter()
            .filter(in_file)
            .any(|envelope| envelope.range.start >= range.start)
        {
            return false;
        }
        self.snapshot.lexical_tokens.iter().all(|token| {
            let Some(token_range) = token.range else {
                return true;
            };
            token_range.file_id != range.file_id
                || token_range.start < range.start
                || token.is_missing
                || token.is_directive
                || token.is_skipped
                || token_range.start == token_range.end
                || self.envelopes.iter().filter(in_file).any(|envelope| {
                    envelope.range.start <= token_range.start
                        && token_range.end <= envelope.range.end
                })
        })
    }

    /// Whether the hierarchical name whose lookup failed after `range` (the
    /// frontend reports the position after the last resolved element) names
    /// an instance of a protected definition, or of an unknown definition
    /// that an opaque envelope may hold.
    fn path_enters_protected(&self, hierarchy: &Hierarchy<'a>, range: SourceRange) -> bool {
        let Some(path) = hierarchy.path_before(range) else {
            return false;
        };
        let Some(scope) = hierarchy.enclosing_definition(range) else {
            return false;
        };
        let mut names = path.iter();
        let Some(first) = names.next() else {
            return false;
        };
        let mut definition = match hierarchy.child(scope, first) {
            Some(child) => child,
            None if hierarchy.definitions.contains(first) => first,
            None => return false,
        };
        for name in names {
            match hierarchy.child(definition, name) {
                Some(child) => definition = child,
                None => return false,
            }
        }
        self.protected_names.contains(definition)
            || (self.opaque && !hierarchy.definitions.contains(definition))
    }

    /// One hint per decryption envelope at its opening directive.
    pub(super) fn hints(&self, texts: &HashMap<&str, &str>) -> Vec<(String, LspDiagnostic)> {
        let files: HashMap<_, _> = self
            .snapshot
            .files
            .iter()
            .map(|file| (file.id, file.name.as_str()))
            .collect();
        self.envelopes
            .iter()
            .filter_map(|envelope| {
                let path = *files.get(&envelope.range.file_id)?;
                let text = *texts.get(path)?;
                let start = usize::try_from(envelope.range.start).ok()?;
                let line_end = text
                    .get(start..)?
                    .find(['\r', '\n'])
                    .map_or(text.len(), |offset| start + offset);
                let range = Range::new(
                    super::slang::byte_position(text, envelope.range.start),
                    super::slang::byte_position(text, line_end as u64),
                );
                let mut message = format!(
                    "protected envelope ({}): the encrypted content is not available and is \
                     treated as a black box",
                    compile::envelope_directive(envelope)
                );
                if envelope.scope.is_none() {
                    message.push_str("; definitions inside it are unknown");
                }
                if envelope.unterminated {
                    message.push_str("; the envelope is not closed before the end of the file");
                }
                Some((
                    path.to_owned(),
                    LspDiagnostic {
                        range,
                        severity: Some(DiagnosticSeverity::HINT),
                        code: Some(NumberOrString::String(PROTECTED_ENVELOPE_CODE.to_owned())),
                        code_description: None,
                        source: Some("llg".to_owned()),
                        message,
                        related_information: None,
                        tags: None,
                        data: None,
                    },
                ))
            })
            .collect()
    }
}

fn range_key(range: SourceRange) -> (u64, u64, u64) {
    (range.file_id, range.start, range.end)
}

/// Definition names, the instances each definition declares and the source
/// tokens, for resolving the prefix of a failed hierarchical name.
struct Hierarchy<'a> {
    snapshot: &'a Snapshot,
    definitions: HashSet<&'a str>,
    /// Definition name starts per file, ascending.
    definition_starts: HashMap<u64, Vec<(u64, &'a str)>>,
    /// (owning definition, instance name) -> instantiated definition.
    children: HashMap<(&'a str, &'a str), &'a str>,
}

impl<'a> Hierarchy<'a> {
    fn new(snapshot: &'a Snapshot) -> Self {
        let by_id: HashMap<_, _> = snapshot
            .semantic_nodes
            .iter()
            .map(|node| (node.id, node))
            .collect();
        let mut definitions = HashSet::new();
        let mut definition_starts: HashMap<u64, Vec<(u64, &str)>> = HashMap::new();
        let mut children = HashMap::new();
        for node in &snapshot.semantic_nodes {
            match node.kind {
                SemanticKind::Definition if !node.name.is_empty() => {
                    definitions.insert(&*node.name);
                    if let Some(range) = node.range {
                        definition_starts
                            .entry(range.file_id)
                            .or_default()
                            .push((range.start, &*node.name));
                    }
                }
                SemanticKind::Instance
                    if !node.is_top()
                        && !node.name.is_empty()
                        && !node.definition_name.is_empty() =>
                {
                    // The owner is the nearest enclosing instance body or
                    // definition; generate scopes in between are not names
                    // this resolver follows, so their instances are skipped.
                    let Some(parent) = node.parent_id().and_then(|id| by_id.get(&id)) else {
                        continue;
                    };
                    let owner = match parent.kind {
                        SemanticKind::Definition => &*parent.name,
                        SemanticKind::Instance => &*parent.definition_name,
                        _ => continue,
                    };
                    if !owner.is_empty() {
                        children.insert((owner, &*node.name), &*node.definition_name);
                    }
                }
                _ => {}
            }
        }
        for starts in definition_starts.values_mut() {
            starts.sort_unstable();
        }
        Self {
            snapshot,
            definitions,
            definition_starts,
            children,
        }
    }

    fn child(&self, definition: &str, name: &str) -> Option<&'a str> {
        self.children.get(&(definition, name)).copied()
    }

    /// The definition whose name is the last one declared before `range` in
    /// its file.
    fn enclosing_definition(&self, range: SourceRange) -> Option<&'a str> {
        let starts = self.definition_starts.get(&range.file_id)?;
        let index = starts.partition_point(|(start, _)| *start <= range.start);
        index.checked_sub(1).map(|index| starts[index].1)
    }

    /// The written names of a hierarchical name that end at `range.start`
    /// (`a.b[1].c`), outermost first. `None` when the text there is not such
    /// a name or comes from a macro expansion.
    fn path_before(&self, range: SourceRange) -> Option<Vec<&'a str>> {
        let tokens: Vec<_> = self
            .snapshot
            .lexical_tokens
            .iter()
            .filter(|token| {
                !token.is_directive
                    && !token.is_skipped
                    && !token.is_missing
                    && token
                        .range
                        .is_some_and(|token_range| token_range.file_id == range.file_id)
            })
            .collect();
        let mut index = tokens
            .iter()
            .position(|token| token.range.is_some_and(|r| r.end == range.start))?;
        let mut names = Vec::new();
        loop {
            // Skip constant selects of instance arrays.
            while tokens[index].text == "]" {
                let mut depth = 0_usize;
                loop {
                    match tokens[index].text.as_str() {
                        "]" => depth += 1,
                        "[" => depth -= 1,
                        _ => {}
                    }
                    if depth == 0 {
                        break;
                    }
                    index = index.checked_sub(1)?;
                }
                index = index.checked_sub(1)?;
            }
            let token = tokens[index];
            if token.is_macro_expansion
                || matches!(
                    token.kind,
                    LexicalKind::Keyword
                        | LexicalKind::Operator
                        | LexicalKind::Number
                        | LexicalKind::String
                        | LexicalKind::SystemSubroutine
                        | LexicalKind::Macro
                        | LexicalKind::MacroFormal
                )
            {
                return None;
            }
            names.push(token.text.as_str());
            match index.checked_sub(1).map(|previous| tokens[previous]) {
                Some(dot) if dot.text == "." && !dot.is_macro_expansion => {
                    index = index.checked_sub(2)?;
                }
                _ => break,
            }
        }
        names.reverse();
        Some(names)
    }
}
