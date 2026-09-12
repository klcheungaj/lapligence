//! Owned lexical tokens and exact semantic bindings captured by Slang.

use crate::ffi::slang::{LexicalKind, LexicalRole, SemanticKind, Snapshot, SourceRange};
use std::collections::{BTreeMap, HashMap, HashSet};

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
pub const TOKEN_DECLARATION_OFFSET: i32 = -1_000;

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

pub type RefBindings = HashMap<(String, u32, u32), DeclTarget>;
/// Reference or connection-label positions for which Slang did not provide
/// one unique declaration identity. Consumers must not resolve these by name.
pub type UnresolvedBindings = HashSet<(String, u32, u32)>;
pub type DeclDetails = HashMap<(String, u32, u32), String>;
pub type ParseDeclPositions = HashSet<(String, u32, u32)>;

#[derive(Debug, Default)]
pub struct ParseGenvarFacts {
    pub bindings: RefBindings,
}

/// Convert Slang's bounded lexical table into the representation consumed by
/// the language-server feature index.
pub fn from_slang_snapshot(snapshot: &Snapshot, sources: &[(&str, &str)]) -> Vec<FileTokens> {
    project_slang(snapshot, sources).0
}

/// Project tokens, exact reference bindings, declaration details, and the
/// declaration-position set from one owned Slang snapshot.
pub fn project_slang(
    snapshot: &Snapshot,
    sources: &[(&str, &str)],
) -> (
    Vec<FileTokens>,
    RefBindings,
    UnresolvedBindings,
    DeclDetails,
    ParseDeclPositions,
) {
    let files: HashMap<_, _> = snapshot
        .files
        .iter()
        .map(|file| (file.id, file.name.as_str()))
        .collect();
    let texts: HashMap<_, _> = sources
        .iter()
        .map(|&(file, text)| (file, SourcePositions::new(text)))
        .collect();
    let semantic: HashMap<_, _> = snapshot
        .semantic_nodes
        .iter()
        .map(|node| (node.id, node))
        .collect();
    let mut grouped: BTreeMap<String, Vec<TokenInfo>> = BTreeMap::new();
    let mut bindings = RefBindings::new();
    let mut unresolved_bindings = UnresolvedBindings::new();
    let mut details = DeclDetails::new();
    let mut declarations = ParseDeclPositions::new();

    for token in &snapshot.lexical_tokens {
        if token.is_missing || token.is_skipped {
            continue;
        }
        let Some(range) = token.range else {
            continue;
        };
        let Some((file, _, line, col, end_line, end_col)) = source_position(range, &files, &texts)
        else {
            continue;
        };
        let role_kind = if token.kind == LexicalKind::Genvar {
            if token.role == LexicalRole::Declaration {
                TOKEN_GENVAR_DECL
            } else {
                TOKEN_GENVAR_REF
            }
        } else if token.role == LexicalRole::ConnectionLabel {
            match token
                .semantic_id
                .and_then(|id| semantic.get(&id).map(|node| node.kind))
            {
                Some(SemanticKind::Parameter) => TOKEN_SLANG_PARAMETER_CONNECTION_LABEL,
                None if token.kind == LexicalKind::Parameter => {
                    TOKEN_SLANG_PARAMETER_CONNECTION_LABEL
                }
                _ => TOKEN_SLANG_PORT_CONNECTION_LABEL,
            }
        } else {
            let kind = lexical_kind(token.kind);
            if token.role == LexicalRole::Declaration {
                kind + TOKEN_DECLARATION_OFFSET
            } else {
                kind
            }
        };
        let node = TokenInfo {
            line,
            col,
            end_line,
            end_col,
            kind: role_kind,
            name: (!token.text.is_empty()).then(|| token.text.clone()),
            file: file.to_owned(),
        };
        if token.role == LexicalRole::Declaration {
            declarations.insert((file.to_owned(), line, col));
        }

        let semantic_node = token.semantic_id.and_then(|id| semantic.get(&id).copied());
        let instance_definition = semantic_node
            .filter(|node| {
                token.role == LexicalRole::Declaration && node.kind == SemanticKind::Instance
            })
            .and_then(|node| node.target_id)
            .and_then(|id| semantic.get(&id).copied())
            .filter(|node| node.kind == SemanticKind::Definition);
        if matches!(
            token.role,
            LexicalRole::Reference | LexicalRole::ConnectionLabel | LexicalRole::ConnectionActual
        ) || instance_definition.is_some()
        {
            let key = (file.to_owned(), line - 1, col - 1);
            if let Some(target) = instance_definition
                .or(semantic_node)
                .and_then(|node| decl_target(node, &files, &texts))
            {
                let mut target = target;
                if token.kind == LexicalKind::Genvar {
                    target.kind = "genvar".to_owned();
                }
                target.via_label = token.role == LexicalRole::ConnectionLabel;
                target.via_connection = token.role == LexicalRole::ConnectionActual;
                if !unresolved_bindings.contains(&key) {
                    match bindings.get(&key) {
                        None => {
                            bindings.insert(key, target);
                        }
                        Some(previous) if previous == &target => {}
                        Some(_) => {
                            bindings.remove(&key);
                            unresolved_bindings.insert(key);
                        }
                    }
                }
            } else {
                bindings.remove(&key);
                unresolved_bindings.insert(key);
            }
        }
        grouped.entry(file.to_owned()).or_default().push(node);
    }

    for node in &snapshot.semantic_nodes {
        let Some(range) = node.range else {
            continue;
        };
        let Some((file, _, line, col, _, _)) = source_position(range, &files, &texts) else {
            continue;
        };
        let detail = declaration_detail(node, snapshot);
        if !detail.is_empty() {
            details
                .entry((file.to_owned(), line, col))
                .or_insert(detail);
        }
    }

    let tokens = grouped
        .into_iter()
        .map(|(path, nodes)| FileTokens { path, nodes })
        .collect();
    (tokens, bindings, unresolved_bindings, details, declarations)
}

fn declaration_detail(node: &crate::ffi::slang::SemanticNode, snapshot: &Snapshot) -> String {
    let ty = node
        .type_id
        .and_then(|id| snapshot.types.iter().find(|ty| ty.id == id))
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
            let direction = if node.is_input {
                "input"
            } else if node.is_output {
                "output"
            } else if node.is_inout {
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
            let keyword = if node.is_local {
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
            let keyword = if node.is_task { "task" } else { "function" };
            match (node.is_task, ty) {
                (false, Some(ty)) => format!("{keyword} {ty} {name}"),
                _ => format!("{keyword} {name}"),
            }
        }
        SemanticKind::EnumConstant => format!("enum constant {name}"),
        _ if !name.is_empty() => format!("{} {name}", semantic_kind(node.kind)),
        _ => String::new(),
    }
}

fn decl_target(
    node: &crate::ffi::slang::SemanticNode,
    files: &HashMap<u64, &str>,
    texts: &HashMap<&str, SourcePositions<'_>>,
) -> Option<DeclTarget> {
    use crate::ffi::slang::SemanticDefinitionKind;
    let range = node.range?;
    let (file, _, line, col, _, _) = source_position(range, files, texts)?;
    let kind = match node.definition_kind {
        Some(SemanticDefinitionKind::Module) => "module",
        Some(SemanticDefinitionKind::Interface) => "interface",
        Some(SemanticDefinitionKind::Program) => "program",
        None => semantic_kind(node.kind),
    };
    DeclTarget::from_decl_coords(&node.name, kind, file, line, col)
}

fn source_position<'a>(
    range: SourceRange,
    files: &HashMap<u64, &'a str>,
    texts: &HashMap<&'a str, SourcePositions<'a>>,
) -> Option<(&'a str, &'a str, u32, u32, u32, u32)> {
    let file = *files.get(&range.file_id)?;
    let positions = texts.get(file)?;
    let (line, col) = positions.position(range.start);
    let (end_line, end_col) = positions.position(range.end.max(range.start));
    Some((file, positions.text, line, col, end_line, end_col))
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
