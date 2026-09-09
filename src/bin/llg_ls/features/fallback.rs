//! Auxiliary source-level facts used when assembling the feature index.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConnKind {
    Port,
    Param,
}

#[derive(Debug, Clone)]
pub(crate) struct NamedPortConn {
    pub file: String,
    pub label: (u32, u32),
    pub label_name: String,
    pub actual: Option<(u32, u32)>,
    pub actual_name: Option<String>,
    pub inst_type: Option<String>,
    pub kind: ConnKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParseEnumDecl {
    pub(super) name: String,
    pub(super) file: String,
    pub(super) line1: u32,
    pub(super) col1: u32,
    pub(super) scope: Option<String>,
}

#[derive(Default)]
pub(crate) struct ConnectionInputs {
    pub parse_decls: Option<ParseDeclPositions>,
    pub pairs: Vec<NamedPortConn>,
    pub fallback_bindings: RefBindings,
    pub parse_enum_decls: Vec<ParseEnumDecl>,
    pub parse_enum_bindings: RefBindings,
    pub parse_enum_ref_positions: HashSet<(String, u32, u32)>,
    pub unresolved_enum_refs: HashSet<(String, u32, u32)>,
    /// Slang reference positions with no unique semantic declaration.
    pub unresolved_bindings: tokens::UnresolvedBindings,
    pub parse_enum_tokens: Vec<TokenInfo>,
}
