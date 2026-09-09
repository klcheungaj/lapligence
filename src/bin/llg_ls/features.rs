//! features — LSP feature implementations over the shared compile/model layer.
//!
//! Cached feature queries are pure: they take a shared [`Analysis`] plus a
//! position and return LSP payloads.  [`semantic_tokens_for_open_document`]
//! is the narrow exception: it performs one blocking, isolated parse-only run
//! over an admitted open buffer. No `Client` and no async here — the
//! tower-lsp backend (`lsp.rs` and `lsp/`) owns caching, staging, scheduling,
//! and request handling.
//!
//! # Position conventions
//!
//! * [`Diag`] positions are **1-based**; 0 means
//!   "unknown".  [`lsp_diagnostics`] converts them to 0-based LSP ranges.
//! * `TokenInfo` / `FileTokens` positions are **1-based** (as returned by
//!   Slang lexical snapshot); [`semantic_tokens_for`] converts them.
//! * Every other public feature function (`hover_at`, `definition_at`,
//!   `references_at`, `document_symbols`, `completion_at`) takes **0-based**
//!   line/column values, matching the LSP wire format directly, and converts
//!   internally when comparing against the 1-based token data.
//! * [`Analysis`] is fully owned and `Send`.

use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, Diagnostic as LspDiagnostic, DiagnosticSeverity,
    DocumentSymbol, Hover, HoverContents, LSPAny, LSPObject, Location, MarkupContent, MarkupKind,
    NumberOrString, Position, Range, SemanticTokens, SymbolInformation, SymbolKind, Url,
};

use crate::semantic_tokens;
use llg::core::compile::{self, CompileOpts};
use llg::core::compile::{Diag, Severity};
use llg::core::elab::Val;
use llg::core::lint::{self, LintConfig, LintDiag, LintRegistry, LintSeverity, RuleConfig};
use llg::core::macros;
use llg::core::model::{
    ClassDef, ClassFieldDef, DesignModel, Direction, EnumConstDef, FuncArgDef, FuncDef,
    InstanceModel, ModuleDef, PackageDef, ParamModel, PortModel, SignalModel, SymKind, TypeInfo,
};
use llg::core::tokens::{self, DeclTarget, FileTokens, ParseDeclPositions, RefBindings, TokenInfo};

// ── Analysis ──────────────────────────────────────────────────────────────────

mod analysis;
mod fallback;
mod requests;
mod slang;
mod source_graph;
mod symbol_index;

pub(crate) use analysis::*;
pub(crate) use fallback::*;
pub(crate) use requests::*;
pub(crate) use slang::*;
pub(crate) use source_graph::*;
pub(crate) use symbol_index::*;

#[cfg(test)]
mod tests;
