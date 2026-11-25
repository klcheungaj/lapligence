//! Owned lexical tokens and exact semantic bindings captured by Slang.

use crate::cancel::{CancelToken, Cancelled};
use crate::core::macros::MacroTable;
use crate::ffi::slang::{
    LexicalKind, LexicalRole, LexicalTargetRef, LexicalToken, SemanticDefinitionKind, SemanticKind,
    SemanticNode, Snapshot, SourceRange,
};
use std::collections::{HashMap, HashSet};
use std::convert::Infallible;

pub const TOKEN_SLANG_MODULE: i32 = -1;
pub const TOKEN_SLANG_INTERFACE: i32 = -2;
pub const TOKEN_SLANG_PROGRAM: i32 = -3;
pub const TOKEN_SLANG_PACKAGE: i32 = -4;
pub const TOKEN_SLANG_CLASS: i32 = -5;
pub const TOKEN_SLANG_STRUCT: i32 = -6;
pub const TOKEN_SLANG_UNION: i32 = -7;
pub const TOKEN_SLANG_ENUM: i32 = -8;
pub const TOKEN_SLANG_ENUM_MEMBER: i32 = -9;
pub const TOKEN_SLANG_TYPE_ALIAS: i32 = -10;
pub const TOKEN_SLANG_PARAMETER: i32 = -11;
pub const TOKEN_SLANG_PORT: i32 = -12;
pub const TOKEN_SLANG_VARIABLE: i32 = -13;
pub const TOKEN_SLANG_NET: i32 = -14;
pub const TOKEN_SLANG_FUNCTION: i32 = -15;
pub const TOKEN_SLANG_TASK: i32 = -16;
pub const TOKEN_SLANG_METHOD: i32 = -17;
pub const TOKEN_SLANG_MACRO: i32 = -18;
pub const TOKEN_SLANG_KEYWORD: i32 = -19;
pub const TOKEN_SLANG_STRING: i32 = -20;
pub const TOKEN_SLANG_NUMBER: i32 = -21;
pub const TOKEN_SLANG_OPERATOR: i32 = -22;
pub const TOKEN_SLANG_IDENTIFIER: i32 = -23;
pub const TOKEN_SLANG_PORT_CONNECTION_LABEL: i32 = -24;
pub const TOKEN_GENVAR_DECL: i32 = -25;
pub const TOKEN_GENVAR_REF: i32 = -26;
pub const TOKEN_SLANG_PARAMETER_CONNECTION_LABEL: i32 = -27;
/// System task or function, or built-in method, name (no declaration).
pub const TOKEN_SLANG_SYSTEM_SUBROUTINE: i32 = -28;
/// Reference bound to a declaration whose lexical kind is a plain identifier
/// (instances, modports, events). An unbound reference stays
/// [`TOKEN_SLANG_IDENTIFIER`].
pub const TOKEN_SLANG_BOUND_IDENTIFIER: i32 = -29;
/// Named statement block, labeled statement or generate block name: a
/// declaration (with [`TOKEN_DECLARATION_OFFSET`]), an end label, or a
/// reference bound to the block. Instance names are not blocks.
pub const TOKEN_SLANG_BLOCK: i32 = -30;
/// Keyword that is control flow only at this syntax position (a case item
/// `default`, an if/case `unique`/`unique0`/`priority`, a disable statement's
/// `disable`, a procedure, continuous-assignment, alias or procedural
/// continuous-assignment keyword). Every other keyword is
/// [`TOKEN_SLANG_KEYWORD`].
pub const TOKEN_SLANG_CONTROL_KEYWORD: i32 = -31;
/// Type parameter (`parameter type T`): a declaration (with
/// [`TOKEN_DECLARATION_OFFSET`]) or a reference bound to it. Value parameters
/// are [`TOKEN_SLANG_PARAMETER`].
pub const TOKEN_SLANG_TYPE_PARAMETER: i32 = -32;
/// Struct or union member or class property: a declaration (with
/// [`TOKEN_DECLARATION_OFFSET`]) or a reference bound to it.
pub const TOKEN_SLANG_FIELD: i32 = -33;
/// Formal argument of a `` `define ``: its name in the formal list (with
/// [`TOKEN_DECLARATION_OFFSET`]) or a body token that spells it. Always
/// carries [`TOKEN_MACRO_BODY_OFFSET`].
pub const TOKEN_SLANG_MACRO_FORMAL: i32 = -34;
/// Class method declared `static` (IEEE 1800-2009 8.10), a declaration (with
/// [`TOKEN_DECLARATION_OFFSET`]) or a reference bound to it. Other functions,
/// tasks and methods are [`TOKEN_SLANG_FUNCTION`].
pub const TOKEN_SLANG_STATIC_FUNCTION: i32 = -35;
/// Attribute name (IEEE 1800-2009 5.12), which no declaration introduces.
pub const TOKEN_SLANG_ATTRIBUTE: i32 = -36;
pub const TOKEN_DECLARATION_OFFSET: i32 = -1_000;
/// Added to the kind (declaration offset included) of a name written in a
/// `` `define `` directive: a body name bound from the macro's expansions, or
/// a formal argument. Such a token is text of the macro, not a declaration or
/// reference of the design, so name indexes skip it (its kind matches none of
/// theirs); [`token_class`] recovers the kind.
pub const TOKEN_MACRO_BODY_OFFSET: i32 = -10_000;
/// Added to the kind of a name that no source declares; see
/// [`TokenRule::Undeclared`]. Name indexes skip it too.
pub const TOKEN_UNDECLARED_OFFSET: i32 = -100_000;

/// Whether a token names a declaration. Highlighting and go-to-definition
/// read the same tokens and [`Bindings`], so this is the one rule for both: an
/// identifier has a token exactly when it has a definition, and the
/// [`Undeclared`](TokenRule::Undeclared) names are the complete list of
/// exceptions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenRule {
    /// A keyword, literal, operator or punctuation, or the scope names `$unit`
    /// and `$root`: no name of a declaration. The path of a resolved
    /// `` `include `` is the only one that has a definition, the included file.
    Lexical,
    /// A name that does not resolve to one kind of declaration: an unknown
    /// name, one used before its declaration, an undefined macro. It has
    /// neither a semantic token nor a definition.
    Unresolved,
    /// A highlighted name that no source declares, so it has no definition:
    ///
    /// * a system task or function and a built-in method
    ///   ([`TOKEN_SLANG_SYSTEM_SUBROUTINE`]);
    /// * a name the language declares: the `std` package, its classes
    ///   (`process`, `semaphore`, `mailbox`) and their members, the implicit
    ///   iterator `item`;
    /// * an attribute name ([`TOKEN_SLANG_ATTRIBUTE`]);
    /// * a macro that a `[compile] defines` entry or the language defines, the
    ///   operand of `` `ifdef ``, `` `ifndef ``, `` `elsif `` or `` `undef ``
    ///   that is not defined there, and the operand words of the other
    ///   directives (`` `pragma ``).
    Undeclared,
    /// A name that declares itself. Its definition is itself, except that an
    /// instance name is bound to the definition it instantiates.
    Declaration,
    /// A name bound to at least one declaration, all of one kind.
    Reference,
}

/// A token kind without its offsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenClass {
    /// One of the `TOKEN_*` kinds.
    pub kind: i32,
    pub declaration: bool,
    /// Written in a `` `define `` directive.
    pub macro_body: bool,
    pub undeclared: bool,
}

impl TokenClass {
    pub fn rule(self) -> TokenRule {
        match self.kind {
            TOKEN_SLANG_KEYWORD
            | TOKEN_SLANG_CONTROL_KEYWORD
            | TOKEN_SLANG_STRING
            | TOKEN_SLANG_NUMBER
            | TOKEN_SLANG_OPERATOR
            | 0 => TokenRule::Lexical,
            _ if self.declaration => TokenRule::Declaration,
            TOKEN_SLANG_SYSTEM_SUBROUTINE | TOKEN_SLANG_ATTRIBUTE => TokenRule::Undeclared,
            _ if self.undeclared => TokenRule::Undeclared,
            TOKEN_SLANG_IDENTIFIER => TokenRule::Unresolved,
            _ => TokenRule::Reference,
        }
    }
}

/// Split a token kind into its base kind and offsets.
pub fn token_class(kind: i32) -> TokenClass {
    // Every smaller offset and base kind lies above half of an offset.
    let mut kind = kind;
    let mut take = |offset: i32| {
        let present = kind <= offset / 2;
        if present {
            kind -= offset;
        }
        present
    };
    let undeclared = take(TOKEN_UNDECLARED_OFFSET);
    let macro_body = take(TOKEN_MACRO_BODY_OFFSET);
    let declaration = take(TOKEN_DECLARATION_OFFSET);
    TokenClass {
        kind,
        declaration: declaration || kind == TOKEN_GENVAR_DECL,
        macro_body,
        undeclared,
    }
}

/// Source-positioned lexical item. Coordinates are one-based UTF-16 to retain
/// the existing feature/index representation; Slang byte ranges are converted
/// exactly once while the admitted source buffers are available.
#[derive(Debug, Clone)]
pub struct TokenInfo {
    pub line: u32,
    pub col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub kind: i32,
    pub name: Option<String>,
    pub file: String,
}

#[derive(Debug)]
pub struct FileTokens {
    pub path: String,
    pub nodes: Vec<TokenInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclTarget {
    pub name: String,
    pub kind: String,
    pub file: String,
    pub line0: u32,
    pub col0: u32,
    pub via_label: bool,
    pub via_connection: bool,
}

impl DeclTarget {
    pub fn from_decl_coords(
        name: &str,
        kind: &str,
        file: &str,
        line1: u32,
        col1: u32,
    ) -> Option<Self> {
        if name.is_empty() || kind.is_empty() || file.is_empty() || line1 == 0 || col1 == 0 {
            return None;
        }
        Some(Self {
            name: name.to_owned(),
            kind: kind.to_owned(),
            file: file.to_owned(),
            line0: line1 - 1,
            col0: col1 - 1,
            via_label: false,
            via_connection: false,
        })
    }
}

/// Position of one token: its file and 0-based line and UTF-16 column.
pub type TokenKey = (String, u32, u32);
pub type RefBindings = HashMap<TokenKey, DeclTarget>;
pub type DeclDetails = HashMap<TokenKey, String>;
pub type ParseDeclPositions = HashSet<TokenKey>;

/// [`DeclTarget::kind`] of a `` `define `` name.
pub const TARGET_MACRO: &str = "macro";
/// [`DeclTarget::kind`] of a `` `define `` formal argument.
pub const TARGET_MACRO_FORMAL: &str = "macro formal";
/// [`DeclTarget::kind`] of an included file: its start.
pub const TARGET_FILE: &str = "file";

/// The targets of every bound token, by the token's position. Definition
/// requests read both tables; a token has an entry in at most one.
#[derive(Debug, Default)]
pub struct Bindings {
    /// Names of the design bound to exactly one declaration. Hover,
    /// references and rename read these.
    pub references: RefBindings,
    /// Tokens that have a definition but are no reference of the design:
    /// names in `` `define `` text, macro names and `` `include `` paths,
    /// instance names (bound to the definition they instantiate), and names
    /// bound to several declarations.
    pub navigation: HashMap<TokenKey, Vec<DeclTarget>>,
}

/// What a token's definition is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Definition<'a> {
    /// The token declares the name.
    Itself,
    /// At least one target.
    Targets(&'a [DeclTarget]),
}

impl Bindings {
    /// The targets bound at a token's 0-based position.
    pub fn targets(&self, file: &str, line0: u32, col0: u32) -> &[DeclTarget] {
        let key = (file.to_owned(), line0, col0);
        match self.references.get(&key) {
            Some(target) => std::slice::from_ref(target),
            None => self.navigation.get(&key).map_or(&[], Vec::as_slice),
        }
    }

    /// The definition of `token`, a token of the projection these bindings
    /// came with ([`TokenRule`]).
    pub fn definition(&self, token: &TokenInfo) -> Option<Definition<'_>> {
        let targets = self.targets(
            &token.file,
            token.line.checked_sub(1)?,
            token.col.checked_sub(1)?,
        );
        if !targets.is_empty() {
            return Some(Definition::Targets(targets));
        }
        (token_class(token.kind).rule() == TokenRule::Declaration).then_some(Definition::Itself)
    }
}

/// The token at a 0-based position of one file's tokens, which are sorted by
/// position: the token that covers it or ends there. A name that ends there
/// goes before the punctuation that starts there.
pub fn token_at(nodes: &[TokenInfo], line0: u32, col0: u32) -> Option<&TokenInfo> {
    let position = (line0.checked_add(1)?, col0.checked_add(1)?);
    let after = nodes.partition_point(|node| (node.line, node.col) <= position);
    let (token, before) = nodes[..after].split_last()?;
    if (token.line, token.col) == position && token_class(token.kind).rule() == TokenRule::Lexical {
        let name = before
            .last()
            .filter(|before| (before.end_line, before.end_col) == position);
        if name.is_some() {
            return name;
        }
    }
    ((token.end_line, token.end_col) >= position).then_some(token)
}

/// Everything one owned Slang snapshot says about the names written in its
/// sources, in LSP coordinates.
#[derive(Debug, Default)]
pub struct Projection {
    /// Tokens by file, sorted by position with one token per position.
    pub tokens: Vec<FileTokens>,
    pub bindings: Bindings,
    pub macros: MacroTable,
    pub details: DeclDetails,
    /// Positions (1-based) of the design's declaration tokens.
    pub declarations: ParseDeclPositions,
}

/// The tokens of `file`, or of every admitted file.
pub fn from_slang_snapshot(snapshot: &Snapshot, file: Option<&str>) -> Vec<FileTokens> {
    project_slang(snapshot, file, &[]).tokens
}

/// Project tokens, bindings and macros from one owned Slang snapshot compiled
/// with `defines`. With `file`, only that file's tokens are projected; their
/// targets may lie in any admitted file.
pub fn project_slang(snapshot: &Snapshot, file: Option<&str>, defines: &[String]) -> Projection {
    match project_steps(snapshot, file, defines, || Ok::<_, Infallible>(())) {
        Ok(projection) => projection,
        Err(never) => match never {},
    }
}

/// [`project_slang`], unless `cancel` stops it.
pub fn project_slang_unless_cancelled(
    snapshot: &Snapshot,
    file: Option<&str>,
    defines: &[String],
    cancel: &CancelToken,
) -> Result<Projection, Cancelled> {
    let mut poller = cancel.poller();
    project_steps(snapshot, file, defines, || poller.step())
}

/// [`project_slang`], with `step` called for every token and node.
fn project_steps<E>(
    snapshot: &Snapshot,
    file: Option<&str>,
    defines: &[String],
    mut step: impl FnMut() -> Result<(), E>,
) -> Result<Projection, E> {
    let sources = SnapshotSources::new(snapshot);
    let macros = MacroTable::from_slang(snapshot, &sources, defines);
    let written = WrittenNames::new(snapshot);
    let mut projector = Projector {
        snapshot,
        sources: &sources,
        macros: &macros,
        written: &written,
        bindings: Bindings::default(),
        declarations: ParseDeclPositions::new(),
    };
    let mut tokens: Vec<FileTokens> = Vec::new();
    let mut file_id = None;
    let lexical = &snapshot.lexical_tokens;
    let mut start = 0;
    while start < lexical.len() {
        step()?;
        // A header of several trees has one token per tree at each range.
        let range = lexical[start].range;
        let mut end = start + 1;
        while end < lexical.len() && lexical[end].range == range {
            end += 1;
        }
        let group = start..end;
        start = end;
        let Some(range) = range else {
            continue;
        };
        let Some(source) = sources.file(range.file_id) else {
            continue;
        };
        if file.is_some_and(|file| file != source.name) {
            continue;
        }
        let Some(token) = projector.project(group, range, source) else {
            continue;
        };
        if file_id != Some(range.file_id) {
            file_id = Some(range.file_id);
            tokens.push(FileTokens {
                path: source.name.to_owned(),
                nodes: Vec::new(),
            });
        }
        if let Some(file) = tokens.last_mut() {
            file.nodes.push(token);
        }
    }
    let Projector {
        bindings,
        declarations,
        ..
    } = projector;

    let mut details = DeclDetails::new();
    for node in &snapshot.semantic_nodes {
        step()?;
        let Some((source, line, col)) = written.range(node).and_then(|range| sources.start(range))
        else {
            continue;
        };
        if file.is_some_and(|file| file != source) {
            continue;
        }
        let detail = declaration_detail(node, snapshot);
        if !detail.is_empty() {
            details
                .entry((source.to_owned(), line, col))
                .or_insert(detail);
        }
    }
    Ok(Projection {
        tokens,
        bindings,
        macros,
        details,
        declarations,
    })
}

/// One token's kind and what it names, before its copies in other syntax
/// trees are merged.
struct Resolved {
    kind: i32,
    targets: Vec<DeclTarget>,
    /// The targets are references of the design.
    reference: bool,
}

/// Where the names that are declared in macro arguments are written. The range
/// of their nodes is the macro use that the declaration is expanded from.
struct WrittenNames<'a>(HashMap<(SourceRange, &'a str), SourceRange>);

impl<'a> WrittenNames<'a> {
    fn new(snapshot: &'a Snapshot) -> Self {
        let names = snapshot
            .lexical_tokens
            .iter()
            .filter(|token| token.is_macro_argument && token.role == LexicalRole::Declaration);
        Self(
            names
                .filter_map(|token| {
                    let node = semantic_node(snapshot, token.semantic_id?)?;
                    Some(((node.range?, node.name.as_str()), token.range?))
                })
                .collect(),
        )
    }

    /// The range of the name that declares `node`.
    fn range(&self, node: &SemanticNode) -> Option<SourceRange> {
        let range = node.range?;
        let written = self.0.get(&(range, node.name.as_str())).copied();
        Some(written.unwrap_or(range))
    }
}

struct Projector<'a> {
    snapshot: &'a Snapshot,
    sources: &'a SnapshotSources<'a>,
    macros: &'a MacroTable,
    written: &'a WrittenNames<'a>,
    bindings: Bindings,
    declarations: ParseDeclPositions,
}

impl Projector<'_> {
    /// Project the tokens at one range, one per syntax tree that holds it.
    /// Copies that disagree on the kind are unresolved.
    fn project(
        &mut self,
        group: std::ops::Range<usize>,
        range: SourceRange,
        source: &SnapshotFile<'_>,
    ) -> Option<TokenInfo> {
        let (line, col) = source.positions.position(range.start);
        let mut merged: Option<Resolved> = None;
        let mut text = "";
        for index in group {
            let token = &self.snapshot.lexical_tokens[index];
            // Macro body tokens all span their use site, where the `` `NAME ``
            // usage token stands; projecting them would paint the body's first
            // token over the usage. Argument tokens keep their written range.
            if token.is_missing
                || token.is_skipped
                || (token.is_macro_expansion && !token.is_macro_argument)
            {
                continue;
            }
            text = &token.text;
            let resolved = self.resolve(index, token, source, line, col);
            merged = Some(match merged {
                None => resolved,
                Some(previous) => merge_copies(previous, resolved),
            });
        }
        let Resolved {
            kind,
            mut targets,
            reference,
        } = merged?;
        let class = token_class(kind);
        if !targets.is_empty() {
            let key = (source.name.to_owned(), line - 1, col - 1);
            if reference && targets.len() == 1 {
                self.bindings
                    .references
                    .extend(targets.pop().map(|target| (key, target)));
            } else {
                self.bindings.navigation.insert(key, targets);
            }
        }
        if class.declaration && !class.macro_body && class.kind != TOKEN_SLANG_MACRO {
            self.declarations
                .insert((source.name.to_owned(), line, col));
        }
        let (end_line, end_col) = source.positions.position(range.end.max(range.start));
        Some(TokenInfo {
            line,
            col,
            end_line,
            end_col,
            kind,
            name: (!text.is_empty()).then(|| text.to_owned()),
            file: source.name.to_owned(),
        })
    }

    /// The kind and targets of one token ([`TokenRule`]).
    fn resolve(
        &self,
        index: usize,
        token: &LexicalToken,
        source: &SnapshotFile<'_>,
        line: u32,
        col: u32,
    ) -> Resolved {
        let lexical = |kind| Resolved {
            kind,
            targets: Vec::new(),
            reference: false,
        };
        match token.kind {
            LexicalKind::Unknown
            | LexicalKind::Number
            | LexicalKind::Operator
            | LexicalKind::SystemSubroutine => return lexical(lexical_kind(token.kind)),
            LexicalKind::Keyword if token.is_control_flow => {
                return lexical(TOKEN_SLANG_CONTROL_KEYWORD)
            }
            LexicalKind::Keyword => return lexical(TOKEN_SLANG_KEYWORD),
            LexicalKind::String => {
                return Resolved {
                    kind: TOKEN_SLANG_STRING,
                    targets: self.include_targets(token),
                    reference: false,
                }
            }
            LexicalKind::Macro => return self.resolve_macro(token, source, line, col),
            _ => {}
        }
        let macro_body = token.is_directive;
        let offset = if macro_body {
            TOKEN_MACRO_BODY_OFFSET
        } else {
            0
        };
        if token.kind == LexicalKind::Attribute {
            return lexical(TOKEN_SLANG_ATTRIBUTE + offset);
        }
        let node = token
            .semantic_id
            .and_then(|id| semantic_node(self.snapshot, id));
        let mut targets: Vec<DeclTarget> = Vec::new();
        if token.role == LexicalRole::Declaration {
            let mut kind = if token.kind == LexicalKind::Genvar {
                TOKEN_GENVAR_DECL
            } else {
                name_kind(token, node) + TOKEN_DECLARATION_OFFSET
            };
            kind += offset;
            // An instance name leads to the definition it instantiates.
            let definition = node
                .filter(|node| !macro_body && is_instance(node))
                .and_then(|node| node.target_id())
                .and_then(|id| semantic_node(self.snapshot, id))
                .filter(|node| node.kind == SemanticKind::Definition);
            targets.extend(definition.and_then(|node| self.decl_target(node)));
            return Resolved {
                kind,
                targets,
                reference: false,
            };
        }
        if token.role != LexicalRole::None {
            for target in self.lexical_targets(index) {
                let target = match target {
                    LexicalTargetRef::Semantic(id) => {
                        semantic_node(self.snapshot, id).and_then(|node| self.decl_target(node))
                    }
                    LexicalTargetRef::Token(index) => self.token_target(index),
                };
                targets.extend(target.filter(|target| !targets.contains(target)));
            }
            if targets.is_empty() {
                targets.extend(node.and_then(|node| self.decl_target(node)));
            }
            for target in &mut targets {
                if token.kind == LexicalKind::Genvar {
                    target.kind = "genvar".to_owned();
                }
                target.via_label = token.role == LexicalRole::ConnectionLabel;
                target.via_connection = token.role == LexicalRole::ConnectionActual;
            }
        }
        if targets.is_empty() {
            // A node without a source range is a name of the language; the
            // capture sends the `std` package itself without a node.
            let language = match node {
                Some(node) => node.range.is_none(),
                None => token.kind == LexicalKind::Package && token.role == LexicalRole::None,
            };
            let kind = if language {
                reference_kind(token, node) + TOKEN_UNDECLARED_OFFSET
            } else {
                TOKEN_SLANG_IDENTIFIER
            };
            return lexical(kind + offset);
        }
        Resolved {
            kind: reference_kind(token, node) + offset,
            targets,
            reference: !macro_body,
        }
    }

    /// A macro name: the name in a `` `define ``, a use, or an operand word of
    /// another directive.
    fn resolve_macro(
        &self,
        token: &LexicalToken,
        source: &SnapshotFile<'_>,
        line: u32,
        col: u32,
    ) -> Resolved {
        let site = self.macros.site_at(source.name, line - 1, col - 1);
        let targets = match site {
            Some(site) if token.role != LexicalRole::Declaration => {
                self.macros.targets(site).collect()
            }
            _ => Vec::new(),
        };
        let kind = match token.role {
            LexicalRole::Declaration => TOKEN_SLANG_MACRO + TOKEN_DECLARATION_OFFSET,
            _ if !targets.is_empty() => TOKEN_SLANG_MACRO,
            LexicalRole::Reference if !site.is_some_and(|site| site.predefined) => {
                TOKEN_SLANG_IDENTIFIER
            }
            _ => TOKEN_SLANG_MACRO + TOKEN_UNDECLARED_OFFSET,
        };
        Resolved {
            kind,
            targets,
            reference: false,
        }
    }

    fn include_targets(&self, token: &LexicalToken) -> Vec<DeclTarget> {
        let Some(range) = token.range.filter(|_| token.is_directive) else {
            return Vec::new();
        };
        let key = |range: SourceRange| (range.file_id, range.start, range.end);
        let includes = &self.snapshot.includes;
        let first = includes.partition_point(|include| key(include.operand) < key(range));
        includes[first..]
            .iter()
            .take_while(|include| include.operand == range)
            .filter_map(|include| self.sources.file(include.file_id))
            .map(|file| DeclTarget {
                name: token.text.clone(),
                kind: TARGET_FILE.to_owned(),
                file: file.name.to_owned(),
                line0: 0,
                col0: 0,
                via_label: false,
                via_connection: false,
            })
            .collect()
    }

    fn lexical_targets(&self, index: usize) -> impl Iterator<Item = LexicalTargetRef> + '_ {
        let targets = &self.snapshot.lexical_targets;
        let first = targets.partition_point(|target| target.token < index);
        targets[first..]
            .iter()
            .take_while(move |target| target.token == index)
            .map(|target| target.target)
    }

    /// A declaration token as a target.
    fn token_target(&self, index: usize) -> Option<DeclTarget> {
        let token = self.snapshot.lexical_tokens.get(index)?;
        let (file, line, col) = self.sources.start(token.range?)?;
        let kind = match token.kind {
            LexicalKind::MacroFormal => TARGET_MACRO_FORMAL,
            _ => "scope",
        };
        DeclTarget::from_decl_coords(&token.text, kind, file, line, col)
    }

    fn decl_target(&self, node: &SemanticNode) -> Option<DeclTarget> {
        let (file, line, col) = self.sources.start(self.written.range(node)?)?;
        // An instance that is not elaborated has the kind of its definition.
        let described = is_unelaborated_instance(node)
            .then(|| node.target_id())
            .flatten()
            .and_then(|id| semantic_node(self.snapshot, id))
            .unwrap_or(node);
        let kind = match described.definition_kind {
            Some(SemanticDefinitionKind::Module) => "module",
            Some(SemanticDefinitionKind::Interface) => "interface",
            Some(SemanticDefinitionKind::Program) => "program",
            None => semantic_kind(node.kind),
        };
        DeclTarget::from_decl_coords(&node.name, kind, file, line, col)
    }
}

/// Merge two copies of one written token.
fn merge_copies(left: Resolved, right: Resolved) -> Resolved {
    let (left_class, right_class) = (token_class(left.kind), token_class(right.kind));
    if left_class.rule() == TokenRule::Unresolved {
        return right;
    }
    if right_class.rule() == TokenRule::Unresolved {
        return left;
    }
    // An argument of a macro is text of the directive in a tree that does not
    // expand it.
    if left_class.macro_body != right_class.macro_body {
        return if left_class.macro_body { right } else { left };
    }
    if left.kind != right.kind {
        return Resolved {
            kind: TOKEN_SLANG_IDENTIFIER,
            targets: Vec::new(),
            reference: false,
        };
    }
    let mut merged = left;
    for target in right.targets {
        if !merged.targets.contains(&target) {
            merged.targets.push(target);
        }
    }
    merged
}

/// An instance, also one that is not elaborated: of an unknown definition, or
/// in a scope that is not instantiated.
fn is_instance(node: &SemanticNode) -> bool {
    node.kind == SemanticKind::Instance || is_unelaborated_instance(node)
}

fn is_unelaborated_instance(node: &SemanticNode) -> bool {
    node.kind == SemanticKind::Unsupported && node.detail == "UninstantiatedDef"
}

fn semantic_node(snapshot: &Snapshot, id: u64) -> Option<&SemanticNode> {
    // Node ids are their indices.
    usize::try_from(id)
        .ok()
        .and_then(|index| snapshot.semantic_nodes.get(index))
}

/// The token kind of a name by its lexical kind.
fn name_kind(token: &LexicalToken, node: Option<&SemanticNode>) -> i32 {
    let static_method = matches!(token.kind, LexicalKind::Function | LexicalKind::Task)
        && node.is_some_and(|node| {
            node.kind == SemanticKind::Subroutine
                && node.auxiliary & crate::ffi::slang::SUBROUTINE_STATIC != 0
        });
    if static_method {
        TOKEN_SLANG_STATIC_FUNCTION
    } else {
        lexical_kind(token.kind)
    }
}

/// The token kind of a bound reference.
fn reference_kind(token: &LexicalToken, node: Option<&SemanticNode>) -> i32 {
    if token.kind == LexicalKind::Genvar {
        return TOKEN_GENVAR_REF;
    }
    if token.role == LexicalRole::ConnectionLabel {
        let parameter = match node {
            Some(node) => node.kind == SemanticKind::Parameter,
            None => matches!(
                token.kind,
                LexicalKind::Parameter | LexicalKind::TypeParameter
            ),
        };
        return if parameter {
            TOKEN_SLANG_PARAMETER_CONNECTION_LABEL
        } else {
            TOKEN_SLANG_PORT_CONNECTION_LABEL
        };
    }
    match name_kind(token, node) {
        TOKEN_SLANG_IDENTIFIER => TOKEN_SLANG_BOUND_IDENTIFIER,
        kind => kind,
    }
}

fn declaration_detail(node: &crate::ffi::slang::SemanticNode, snapshot: &Snapshot) -> String {
    let ty = node
        .type_id()
        .and_then(|id| snapshot.types.get(usize::try_from(id).ok()?))
        .map(|ty| ty.display_name.as_str())
        .filter(|name| !name.is_empty());
    let name = node.name.as_str();
    match node.kind {
        SemanticKind::Definition => match node.definition_kind {
            Some(crate::ffi::slang::SemanticDefinitionKind::Module) => format!("module {name}"),
            Some(crate::ffi::slang::SemanticDefinitionKind::Interface) => {
                format!("interface {name}")
            }
            Some(crate::ffi::slang::SemanticDefinitionKind::Program) => format!("program {name}"),
            None => format!("definition {name}"),
        },
        SemanticKind::Package => format!("package {name}"),
        SemanticKind::Class => format!("class {name}"),
        SemanticKind::Port => {
            let direction = if node.is_input() {
                "input"
            } else if node.is_output() {
                "output"
            } else if node.is_inout() {
                "inout"
            } else {
                "port"
            };
            match ty {
                Some(ty) => format!("{direction} {ty} {name}"),
                None => format!("{direction} {name}"),
            }
        }
        SemanticKind::Parameter => {
            let keyword = if node.is_local() {
                "localparam"
            } else {
                "parameter"
            };
            match ty {
                Some(ty) => format!("{keyword} {ty} {name}"),
                None => format!("{keyword} {name}"),
            }
        }
        SemanticKind::Variable if node.subkind == 229 => format!("genvar {name}"),
        SemanticKind::Net | SemanticKind::Variable | SemanticKind::Argument => match ty {
            Some(ty) => format!("{ty} {name}"),
            None => format!("{} {name}", semantic_kind(node.kind)),
        },
        SemanticKind::Subroutine => {
            let keyword = if node.is_task() { "task" } else { "function" };
            match (node.is_task(), ty) {
                (false, Some(ty)) => format!("{keyword} {ty} {name}"),
                _ => format!("{keyword} {name}"),
            }
        }
        SemanticKind::EnumConstant => format!("enum constant {name}"),
        // An instance of a definition Slang could not resolve (an unknown
        // module is a black box): describe it like an instance, by the
        // written definition name.
        _ if is_unelaborated_instance(node)
            && !name.is_empty()
            && !node.definition_name.is_empty() =>
        {
            format!("{} {name}", node.definition_name)
        }
        _ if !name.is_empty() => format!("{} {name}", semantic_kind(node.kind)),
        _ => String::new(),
    }
}

/// One admitted file of a snapshot.
pub(crate) struct SnapshotFile<'a> {
    pub name: &'a str,
    pub positions: SourcePositions<'a>,
}

/// The admitted files of a snapshot by id.
pub(crate) struct SnapshotSources<'a>(HashMap<u64, SnapshotFile<'a>>);

impl<'a> SnapshotSources<'a> {
    pub fn new(snapshot: &'a Snapshot) -> Self {
        Self(
            snapshot
                .files
                .iter()
                .map(|file| {
                    (
                        file.id,
                        SnapshotFile {
                            name: file.name.as_str(),
                            positions: SourcePositions::new(&file.text),
                        },
                    )
                })
                .collect(),
        )
    }

    pub fn file(&self, id: u64) -> Option<&SnapshotFile<'a>> {
        self.0.get(&id)
    }

    /// The file and one-based position of a range's start.
    pub fn start(&self, range: SourceRange) -> Option<(&'a str, u32, u32)> {
        let file = self.file(range.file_id)?;
        let (line, col) = file.positions.position(range.start);
        Some((file.name, line, col))
    }
}

/// Borrowed source text with a newline index for repeated byte-to-UTF-16 lookups.
pub struct SourcePositions<'a> {
    text: &'a str,
    line_starts: Vec<usize>,
}

impl<'a> SourcePositions<'a> {
    /// Index CR, LF, and CRLF line endings without copying the source text.
    pub fn new(text: &'a str) -> Self {
        let mut line_starts = vec![0];
        let mut bytes = text.bytes().enumerate().peekable();
        while let Some((offset, byte)) = bytes.next() {
            match byte {
                b'\r' if bytes.peek().is_some_and(|(_, next)| *next == b'\n') => {
                    bytes.next();
                    line_starts.push(offset + 2);
                }
                b'\r' | b'\n' => line_starts.push(offset + 1),
                _ => {}
            }
        }
        Self { text, line_starts }
    }

    /// The lines without their line endings: one more than there are endings.
    pub fn lines(&self) -> impl Iterator<Item = &'a str> + '_ {
        let ends = self.line_starts[1..].iter().copied();
        self.line_starts
            .iter()
            .zip(ends.chain([self.text.len()]))
            .map(|(&start, end)| self.text[start..end].trim_end_matches(['\r', '\n']))
    }

    /// The text of `range`, empty when it is not inside the source.
    pub fn text(&self, range: SourceRange) -> &'a str {
        let bound = |offset| usize::try_from(offset).unwrap_or(usize::MAX);
        self.text
            .get(bound(range.start)..bound(range.end))
            .unwrap_or_default()
    }

    /// Return one-based line and UTF-16 column, clamping to a character boundary.
    pub fn position(&self, offset: u64) -> (u32, u32) {
        let mut end = usize::try_from(offset)
            .unwrap_or(usize::MAX)
            .min(self.text.len());
        while !self.text.is_char_boundary(end) {
            end -= 1;
        }
        if end > 0
            && self.text.as_bytes().get(end) == Some(&b'\n')
            && self.text.as_bytes()[end - 1] == b'\r'
        {
            end += 1;
        }
        let line = self.line_starts.partition_point(|&start| start <= end) - 1;
        let col = self.text[self.line_starts[line]..end]
            .encode_utf16()
            .count();
        (line as u32 + 1, col as u32 + 1)
    }
}

fn lexical_kind(kind: LexicalKind) -> i32 {
    match kind {
        LexicalKind::Unknown => 0,
        LexicalKind::Module => TOKEN_SLANG_MODULE,
        LexicalKind::Interface => TOKEN_SLANG_INTERFACE,
        LexicalKind::Program => TOKEN_SLANG_PROGRAM,
        LexicalKind::Package => TOKEN_SLANG_PACKAGE,
        LexicalKind::Class => TOKEN_SLANG_CLASS,
        LexicalKind::Struct => TOKEN_SLANG_STRUCT,
        LexicalKind::Union => TOKEN_SLANG_UNION,
        LexicalKind::Enum => TOKEN_SLANG_ENUM,
        LexicalKind::EnumMember => TOKEN_SLANG_ENUM_MEMBER,
        LexicalKind::TypeAlias => TOKEN_SLANG_TYPE_ALIAS,
        LexicalKind::Parameter => TOKEN_SLANG_PARAMETER,
        LexicalKind::Port => TOKEN_SLANG_PORT,
        LexicalKind::Variable => TOKEN_SLANG_VARIABLE,
        LexicalKind::Net => TOKEN_SLANG_NET,
        LexicalKind::Function => TOKEN_SLANG_FUNCTION,
        LexicalKind::Task => TOKEN_SLANG_TASK,
        LexicalKind::Method => TOKEN_SLANG_METHOD,
        LexicalKind::Macro => TOKEN_SLANG_MACRO,
        LexicalKind::Keyword => TOKEN_SLANG_KEYWORD,
        LexicalKind::String => TOKEN_SLANG_STRING,
        LexicalKind::Number => TOKEN_SLANG_NUMBER,
        LexicalKind::Operator => TOKEN_SLANG_OPERATOR,
        LexicalKind::Identifier => TOKEN_SLANG_IDENTIFIER,
        LexicalKind::Genvar => TOKEN_GENVAR_REF,
        LexicalKind::SystemSubroutine => TOKEN_SLANG_SYSTEM_SUBROUTINE,
        LexicalKind::Block => TOKEN_SLANG_BLOCK,
        LexicalKind::TypeParameter => TOKEN_SLANG_TYPE_PARAMETER,
        LexicalKind::Field => TOKEN_SLANG_FIELD,
        LexicalKind::MacroFormal => TOKEN_SLANG_MACRO_FORMAL,
        LexicalKind::Attribute => TOKEN_SLANG_ATTRIBUTE,
    }
}

pub fn token_base_kind(kind: i32) -> (i32, bool) {
    if kind <= TOKEN_DECLARATION_OFFSET {
        (kind - TOKEN_DECLARATION_OFFSET, true)
    } else {
        (kind, false)
    }
}

fn semantic_kind(kind: SemanticKind) -> &'static str {
    match kind {
        SemanticKind::Instance => "instance",
        SemanticKind::Package => "package",
        SemanticKind::Class => "class",
        SemanticKind::GenerateScope | SemanticKind::Scope => "scope",
        SemanticKind::Port => "port",
        SemanticKind::Modport => "modport",
        SemanticKind::InterfaceConnection => "interface connection",
        SemanticKind::Net => "net",
        SemanticKind::NetAlias => "net alias",
        SemanticKind::Variable | SemanticKind::Array => "variable",
        SemanticKind::NamedEvent => "event",
        SemanticKind::Parameter => "parameter",
        SemanticKind::Process => "process",
        SemanticKind::ContinuousAssign => "continuous assignment",
        SemanticKind::TimingControl => "timing control",
        SemanticKind::AssertionExpr => "assertion expression",
        SemanticKind::ConstraintBlock => "constraint block",
        SemanticKind::Constraint => "constraint",
        SemanticKind::Coverage => "coverage",
        SemanticKind::Production => "production",
        SemanticKind::Specify => "specify item",
        SemanticKind::Primitive => "primitive",
        SemanticKind::Subroutine => "subroutine",
        SemanticKind::Argument => "argument",
        SemanticKind::Statement => "statement",
        SemanticKind::Expression => "expression",
        SemanticKind::SystemCall => "system call",
        SemanticKind::MethodCall => "method call",
        SemanticKind::FunctionCall => "function call",
        SemanticKind::EnumConstant => "enum member",
        SemanticKind::Definition => "definition",
        SemanticKind::Unsupported => "unsupported",
    }
}

#[cfg(test)]
mod tests {
    use super::SourcePositions;

    #[test]
    fn source_positions_handle_utf16_newlines_and_clamped_offsets() {
        let text = "a😀\r\nb\rc\nd";
        let positions = SourcePositions::new(text);
        for (offset, expected) in [
            (0, (1, 1)),
            (1, (1, 2)),
            (2, (1, 2)),
            (5, (1, 4)),
            (6, (2, 1)),
            (7, (2, 1)),
            (8, (2, 2)),
            (9, (3, 1)),
            (11, (4, 1)),
            (u64::MAX, (4, 2)),
        ] {
            assert_eq!(positions.position(offset), expected, "offset={offset}");
        }
    }
}
