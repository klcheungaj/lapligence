//! semantic_tokens — LSP semantic token legend and encoding for UHDM VPI nodes.
//!
//! # Token-type index table
//!
//! | Index | LSP type          | VPI / synthetic types covered                           |
//! |-------|-------------------|---------------------------------------------------------|
//! |   0   | `namespace`       | `vpiModule`, `uhdmpackage`, `uhdmmodule_inst`           |
//! |   1   | `type`            | Typedef aliases                                         |
//! |   2   | `class`           | `uhdmclass_defn`                                        |
//! |   3   | `enum`            | `uhdmenum_typespec`                                     |
//! |   4   | `interface`       | `uhdminterface_inst`                                    |
//! |   5   | `struct`          | `uhdmstruct_typespec`, `uhdmunion_typespec`              |
//! |   6   | `typeParameter`   | (reserved)                                              |
//! |   7   | `parameter`       | `vpiParameter`, `vpiSpecParam`, `TOKEN_PORT_INPUT`      |
//! |   8   | `variable`        | nets, wires, regs, logic vars, output/inout ports,      |
//! |       |                   | `uhdmref_obj`, `uhdmref_var` (signal references)        |
//! |   9   | `property`        | `vpiParameter`, `uhdmparameter` (param definitions)     |
//! |  10   | `enumMember`      | `uhdmenum_const`                                        |
//! |  11   | `event`           | (reserved)                                              |
//! |  12   | `function`        | `vpiFunction`, `vpiTask`                                |
//! |  13   | `method`          | (reserved)                                              |
//! |  14   | `macro`           | (not available via VPI/UHDM interface)                  |
//! |  15   | `keyword`         | (reserved)                                              |
//! |  16   | `modifier`        | (reserved)                                              |
//! |  17   | `comment`         | (reserved)                                              |
//! |  18   | `string`          | (reserved)                                              |
//! |  19   | `number`          | (reserved)                                              |
//! |  20   | `regexp`          | (reserved)                                              |
//! |  21   | `operator`        | (reserved)                                              |
//!
//! # Token-modifier index table
//!
//! | Bit | LSP modifier      | Applied to                                              |
//! |-----|-------------------|---------------------------------------------------------|
//! |  0  | `declaration`     | module instantiation names                              |
//! |  1  | `definition`      | (reserved)                                              |
//! |  2  | `readonly`        | parameters, input ports                                 |
//! |  3  | `static`          | (reserved)                                              |
//! |  4  | `deprecated`      | (reserved)                                              |
//! |  5  | `abstract`        | (reserved)                                              |
//! |  6  | `async`           | (reserved)                                              |
//! |  7  | `modification`    | (reserved)                                              |
//! |  8  | `documentation`   | (reserved)                                              |
//! |  9  | `defaultLibrary`  | (reserved)                                              |
//! | 10  | `connectionLabel` | named PORT/PARAMETER connection labels (`.clk` in       |
//! |     |                   | `.clk(wa)`, `.W` in `m #(.W(4))`) — the label side of a |
//! |     |                   | instantiation connection, distinguishable from the      |
//! |     |                   | connected signal via `*.connectionLabel` theme rules.   |

#![allow(dead_code)]

use tower_lsp::lsp_types::{
    SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokens, SemanticTokensLegend,
};

use llg::core::vobject_types::*;
use llg::ffi::surelog::VObjectInfo;

// ── Legend index constants ────────────────────────────────────────────────────

const TT_NAMESPACE: u32 = 0;
const TT_TYPE: u32 = 1;
const TT_CLASS: u32 = 2;
const TT_ENUM: u32 = 3;
const TT_INTERFACE: u32 = 4;
const TT_STRUCT: u32 = 5;
const TT_TYPE_PARAMETER: u32 = 6;
const TT_PARAMETER: u32 = 7;
const TT_VARIABLE: u32 = 8;
const TT_PROPERTY: u32 = 9;
const TT_ENUM_MEMBER: u32 = 10;
const TT_EVENT: u32 = 11;
const TT_FUNCTION: u32 = 12;
const TT_METHOD: u32 = 13;
const TT_MACRO: u32 = 14;
const TT_KEYWORD: u32 = 15;
const TT_MODIFIER: u32 = 16;
const TT_COMMENT: u32 = 17;
const TT_STRING: u32 = 18;
const TT_NUMBER: u32 = 19;
const TT_REGEXP: u32 = 20;
const TT_OPERATOR: u32 = 21;

const TM_DECLARATION: u32 = 0;
const TM_DEFINITION: u32 = 1;
const TM_READONLY: u32 = 2;
const TM_STATIC: u32 = 3;
const TM_DEPRECATED: u32 = 4;
const TM_ABSTRACT: u32 = 5;
const TM_ASYNC: u32 = 6;
const TM_MODIFICATION: u32 = 7;
const TM_DOCUMENTATION: u32 = 8;
const TM_DEFAULT_LIBRARY: u32 = 9;
const TM_CONNECTION_LABEL: u32 = 10;

/// Custom legend modifier marking the LABEL side of a module-instantiation
/// connection (`.clk` in `.clk(wa)`, `.W` in `m #(.W(4))`).  A modifier — not
/// a new token type — so stock themes keep the labels' existing appearance
/// and one `*.connectionLabel` rule recolors both label flavors.
const TM_CONNECTION_LABEL_NAME: SemanticTokenModifier =
    SemanticTokenModifier::new("connectionLabel");
// ── Public API ────────────────────────────────────────────────────────────────

/// Build the semantic token legend to be reported in `InitializeResult`.
pub fn legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: vec![
            SemanticTokenType::NAMESPACE,
            SemanticTokenType::TYPE,
            SemanticTokenType::CLASS,
            SemanticTokenType::ENUM,
            SemanticTokenType::INTERFACE,
            SemanticTokenType::STRUCT,
            SemanticTokenType::TYPE_PARAMETER,
            SemanticTokenType::PARAMETER,
            SemanticTokenType::VARIABLE,
            SemanticTokenType::PROPERTY,
            SemanticTokenType::ENUM_MEMBER,
            SemanticTokenType::EVENT,
            SemanticTokenType::FUNCTION,
            SemanticTokenType::METHOD,
            SemanticTokenType::MACRO,
            SemanticTokenType::KEYWORD,
            SemanticTokenType::MODIFIER,
            SemanticTokenType::COMMENT,
            SemanticTokenType::STRING,
            SemanticTokenType::NUMBER,
            SemanticTokenType::REGEXP,
            SemanticTokenType::OPERATOR,
        ],
        token_modifiers: vec![
            SemanticTokenModifier::DECLARATION,
            SemanticTokenModifier::DEFINITION,
            SemanticTokenModifier::READONLY,
            SemanticTokenModifier::STATIC,
            SemanticTokenModifier::DEPRECATED,
            SemanticTokenModifier::ABSTRACT,
            SemanticTokenModifier::ASYNC,
            SemanticTokenModifier::MODIFICATION,
            SemanticTokenModifier::DOCUMENTATION,
            SemanticTokenModifier::DEFAULT_LIBRARY,
            TM_CONNECTION_LABEL_NAME,
        ],
    }
}

/// Encode `nodes` into the delta-encoded `SemanticTokens` format required by
/// `textDocument/semanticTokens/full`.
///
/// Each node is classified via its stable VPI type constant (`node.vpi_type`).
/// Nodes with no recognised type, no valid position, or no name are silently
/// skipped.  The LSP protocol requires every semantic token to fit on a single
/// line; multi-line objects (e.g. a full module body) are emitted at their
/// start line with a length equal to the object's name.
///
/// Nodes are assumed to already be filtered to the target file (each
/// `FileTokens` entry from `tokens::collect_vpi_tokens` contains only one
/// file's objects).
pub fn encode(nodes: &[VObjectInfo]) -> SemanticTokens {
    // Phase 1: collect (line0, col0, length, token_type) tuples.
    let mut items: Vec<(u32, u32, u32, u32, u32)> = Vec::with_capacity(nodes.len());

    for node in nodes {
        if node.line == 0 {
            continue;
        }

        let Some(tt) = token_type_for(node.vpi_type) else {
            continue;
        };

        // Token length: prefer the object name length (most reliable across
        // all UHDM object types) then fall back to the single-line span.  The
        // LSP positions and lengths use UTF-16 code units, so keep names in
        // that same unit instead of UTF-8 bytes or Unicode scalar values.
        let length = if let Some(name) = &node.name {
            if name.is_empty() {
                continue;
            }
            name.encode_utf16().count() as u32
        } else if node.end_line == node.line
            && node.end_col > 0
            && node.end_col >= node.col
            && node.col > 0
        {
            node.end_col - node.col
        } else {
            continue; // no usable length information
        };

        // Convert from VPI's 1-based (line, col) to LSP's 0-based.
        let line0 = node.line.saturating_sub(1);
        let col0 = node.col.saturating_sub(1);

        items.push((line0, col0, length, tt.0, tt.1));
    }

    // Phase 2: sort by (line, col) and dedup VObjectType at identical positions.
    // Use a stable sort so that VPI-originated VObjectType (which appear first in
    // the merged list from `merge_file_VObjectType`) take priority over parse-tree
    // VObjectType at the same source position when deduplicating.
    items.sort_by_key(|&(l, c, _, _, _)| (l, c));
    items.dedup_by_key(|i| (i.0, i.1));

    // Phase 3: delta-encode.
    let mut data: Vec<SemanticToken> = Vec::with_capacity(items.len());
    let mut prev_line = 0u32;
    let mut prev_col = 0u32;

    for (line, col, length, token_type, token_modifiers) in items {
        let delta_line = line - prev_line;
        let delta_start = if delta_line == 0 { col - prev_col } else { col };

        data.push(SemanticToken {
            delta_line,
            delta_start,
            length,
            token_type,
            token_modifiers_bitset: token_modifiers,
        });

        prev_line = line;
        prev_col = col;
    }

    SemanticTokens {
        result_id: None,
        data,
    }
}

// ── Mapping from VPI type constant → token type ───────────────────────────────

/// Map a stable VPI / UHDM object type to the corresponding LSP token-type
/// index, or return `None` if the object should not produce a semantic token.
///
/// VPI type constants (e.g. `vpiModule = 32`, `vpiNet = 36`) are defined by
/// the IEEE 1800-2017 standard and the UHDM extension set; they do not change
/// across Surelog versions.
fn token_type_for(vpi_type: i32) -> Option<(u32, u32)> {
    use llg::ffi::vpi;

    let token_type: Option<u32>;
    let mut token_modifiers: u32;
    match vpi_type {
        // ── Scope / namespace types ───────────────────────────────────────────
        // module definition
        vpi::vpiModule => token_type = Some(TT_NAMESPACE),
        // module instantiation (synthetic type set by VObjectType.rs walker)
        vpi::uhdmmodule_inst => token_type = Some(TT_NAMESPACE),
        // package definitions
        vpi::uhdmpackage => token_type = Some(TT_NAMESPACE),

        // ── Interface ─────────────────────────────────────────────────────────
        // UHDM represents interface definitions as interface_inst objects.
        vpi::uhdminterface_inst => token_type = Some(TT_INTERFACE),

        // ── Class definitions ─────────────────────────────────────────────────
        vpi::uhdmclass_defn => token_type = Some(TT_CLASS),

        // ── Enum typespecs and constants ──────────────────────────────────────
        vpi::uhdmenum_typespec => token_type = Some(TT_ENUM),
        vpi::uhdmenum_const => token_type = Some(TT_ENUM_MEMBER),

        // ── Struct / union typespecs ──────────────────────────────────────────
        vpi::uhdmstruct_typespec | vpi::uhdmunion_typespec => token_type = Some(TT_STRUCT),

        // ── Functions and tasks ───────────────────────────────────────────────
        vpi::vpiFunction | vpi::vpiTask | vpi::uhdmfunction | vpi::uhdmtask => {
            token_type = Some(TT_FUNCTION)
        }

        // ── Parameters ────────────────────────────────────────────────────────
        vpi::vpiParameter | vpi::vpiSpecParam | vpi::uhdmparameter => {
            token_type = Some(TT_PROPERTY)
        }

        // ── Ports — direction-specific synthetic types ─────────────────────────
        // Input ports: treated as read-only, shown like parameters.
        vpi::TOKEN_PORT_INPUT => token_type = Some(TT_PARAMETER),
        // Output and inout ports: writable, shown as variables.
        vpi::TOKEN_PORT_OUTPUT | vpi::TOKEN_PORT_INOUT => token_type = Some(TT_VARIABLE),
        // Generic port fallback (direction unknown).
        vpi::vpiPort | vpi::vpiPortBit => token_type = Some(TT_VARIABLE),

        // ── Nets — by net type ────────────────────────────────────────────────
        // wire / logic_net / tri / supply / …
        vpi::vpiNet | vpi::vpiNetBit | vpi::uhdmlogic_net | vpi::uhdmnet => {
            token_type = Some(TT_VARIABLE)
        }

        // ── Registers ─────────────────────────────────────────────────────────
        vpi::vpiReg | vpi::vpiRegBit => token_type = Some(TT_VARIABLE),

        // ── Standard VPI variable types ───────────────────────────────────────
        vpi::vpiIntegerVar | vpi::vpiRealVar | vpi::vpiTimeVar => token_type = Some(TT_VARIABLE),

        // ── UHDM-specific variable types ──────────────────────────────────────
        // vpiLogicVar (3009) is the UHDM extension selector; uhdmlogic_var
        // (2213) is the UHDM object type returned by vpi_get(vpiType, h).
        vpi::vpiLogicVar
        | vpi::uhdmlogic_var
        | vpi::uhdmint_var
        | vpi::uhdmreal_var
        | vpi::uhdmbit_var
        | vpi::uhdmbyte_var
        | vpi::uhdmshort_int_var
        | vpi::uhdmlong_int_var => token_type = Some(TT_VARIABLE),

        // ── Signal references (uses in expressions) ───────────────────────────
        // uhdmref_obj covers references to nets, regs, ports, and variables.
        // uhdmref_var covers variable-specific references (UHDM extension).
        vpi::uhdmref_obj | vpi::uhdmref_var | vpi::vpiRefObj => token_type = Some(TT_VARIABLE),

        // ── Named-connection labels (LSP-internal synthetic types) ───────────
        // The label keeps the base type it historically rendered with — port
        // labels as `function` (the pre-modifier classification), override
        // labels as `property` — so stock themes show today's colors; the
        // `connectionLabel` modifier is what themes style to distinguish
        // labels from connected signals.
        vpi::TOKEN_PORT_CONN_LABEL => token_type = Some(TT_FUNCTION),
        vpi::TOKEN_PARAM_CONN_LABEL => token_type = Some(TT_PROPERTY),

        // ── Parse-tree keyword VObjectType (PARSE_OFFSET = 100_000) ───────────────
        //
        // These are Surelog VObjectType discriminants shifted by PARSE_OFFSET.
        // They cover constructs that are absent from the UHDM model.

        // Macro usages: `MY_MACRO, `MY_MACRO(...)
        t if t == VObjectTypeShifted::ppMacroInstanceNoArgs
            || t == VObjectTypeShifted::ppMacroInstanceWithArgs
            || t == VObjectTypeShifted::ppMacro_definition =>
        {
            token_type = Some(TT_MACRO)
        }

        // Scope / module-boundary keywords
        t if t == VObjectTypeShifted::paModule_keyword
            || t == VObjectTypeShifted::paMODULE
            || t == VObjectTypeShifted::paENDMODULE
            || t == VObjectTypeShifted::paPACKAGE
            || t == VObjectTypeShifted::paENDPACKAGE
            || t == VObjectTypeShifted::paINTERFACE
            || t == VObjectTypeShifted::paENDINTERFACE
            || t == VObjectTypeShifted::paCLASS
            || t == VObjectTypeShifted::paENDCLASS
            || t == VObjectTypeShifted::paPROGRAM
            || t == VObjectTypeShifted::paENDPROGRAM
            || t == VObjectTypeShifted::paFUNCTION
            || t == VObjectTypeShifted::paENDFUNCTION
            || t == VObjectTypeShifted::paTASK
            || t == VObjectTypeShifted::paENDTASK
            || t == VObjectTypeShifted::paGENERATE
            || t == VObjectTypeShifted::paENDGENERATE =>
        {
            token_type = Some(TT_KEYWORD)
        }

        // Port direction keywords
        t if t == VObjectTypeShifted::paINPUT => token_type = Some(TT_MODIFIER),
        t if t == VObjectTypeShifted::paOUTPUT => token_type = Some(TT_MODIFIER),
        t if t == VObjectTypeShifted::paINOUT => token_type = Some(TT_MODIFIER),

        // Net-type keywords
        t if t == VObjectTypeShifted::paWIRE
            || t == VObjectTypeShifted::paWAND
            || t == VObjectTypeShifted::paWOR
            || t == VObjectTypeShifted::paUWIRE
            || t == VObjectTypeShifted::paTRI
            || t == VObjectTypeShifted::paTRI0
            || t == VObjectTypeShifted::paTRI1
            || t == VObjectTypeShifted::paTRIAND
            || t == VObjectTypeShifted::paTRIOR
            || t == VObjectTypeShifted::paTRIREG
            || t == VObjectTypeShifted::paSUPPLY0
            || t == VObjectTypeShifted::paSUPPLY1
            || t == VObjectTypeShifted::paREG =>
        {
            token_type = Some(TT_KEYWORD)
        }

        // Data-type keywords
        t if t == VObjectTypeShifted::paLOGIC
            || t == VObjectTypeShifted::paBIT
            || t == VObjectTypeShifted::paBYTE
            || t == VObjectTypeShifted::paSHORTINT
            || t == VObjectTypeShifted::paINT
            || t == VObjectTypeShifted::paLONGINT
            || t == VObjectTypeShifted::paINTEGER
            || t == VObjectTypeShifted::paREAL
            || t == VObjectTypeShifted::paSHORTREAL
            || t == VObjectTypeShifted::paREALTIME
            || t == VObjectTypeShifted::paTIME
            || t == VObjectTypeShifted::paCHANDLE
            || t == VObjectTypeShifted::paSTRING
            || t == VObjectTypeShifted::paVOID
            || t == VObjectTypeShifted::paGENVAR
            || t == VObjectTypeShifted::paENUM
            || t == VObjectTypeShifted::paSTRUCT
            || t == VObjectTypeShifted::paUNION
            || t == VObjectTypeShifted::paTYPEDEF
            || t == VObjectTypeShifted::paTYPE =>
        {
            token_type = Some(TT_KEYWORD)
        }

        // Qualifier / modifier keywords
        t if t == VObjectTypeShifted::paPARAMETER
            || t == VObjectTypeShifted::paLOCALPARAM
            || t == VObjectTypeShifted::paDEFPARAM =>
        {
            token_type = Some(TT_KEYWORD)
        }

        t if t == VObjectTypeShifted::paSTATIC
            || t == VObjectTypeShifted::paAUTOMATIC
            || t == VObjectTypeShifted::paVIRTUAL
            || t == VObjectTypeShifted::paEXTENDS
            || t == VObjectTypeShifted::paIMPLEMENTS =>
        {
            token_type = Some(TT_MODIFIER)
        }

        t if t == VObjectTypeShifted::paAssignment_pattern_key
            || t == VObjectTypeShifted::paStructure_pattern_key =>
        {
            token_type = Some(TT_KEYWORD)
        }

        t if t == VObjectTypeShifted::ppComment => token_type = Some(TT_COMMENT),

        // Typedef declaration names (parse-tree origin).
        vpi::TOKEN_TYPEDEF_NAME => token_type = Some(TT_TYPE),

        _ => token_type = None,
    }

    match vpi_type {
        // Parameters and input ports are read-only.  The parameter-override
        // label keeps its historical `readonly` base-modifier too.
        vpi::vpiParameter
        | vpi::vpiSpecParam
        | vpi::uhdmparameter
        | vpi::TOKEN_PORT_INPUT
        | vpi::TOKEN_PARAM_CONN_LABEL => {
            token_modifiers = 1 << TM_READONLY;
        }
        // Module instantiation names are declarations of a new named instance.
        vpi::uhdmmodule_inst => {
            token_modifiers = 1 << TM_DECLARATION;
        }
        // parameter / localparam keywords are always read-only.
        t if t == VObjectTypeShifted::paPARAMETER || t == VObjectTypeShifted::paLOCALPARAM => {
            token_modifiers = 1 << TM_READONLY;
        }
        _ => token_modifiers = 0,
    }

    // Connection labels always carry the `connectionLabel` modifier, on top
    // of whatever base-type modifiers applied above.
    if matches!(
        vpi_type,
        vpi::TOKEN_PORT_CONN_LABEL | vpi::TOKEN_PARAM_CONN_LABEL
    ) {
        token_modifiers |= 1 << TM_CONNECTION_LABEL;
    }

    return token_type.map(|tt| (tt, token_modifiers));
}

#[cfg(test)]
mod tests {
    use super::*;
    use llg::core::tokens::{self, FileTokens};
    use llg::ffi::vpi;

    /// Decode an encoded stream back to absolute (line0, col0) positions.
    fn decoded_positions(data: &[SemanticToken]) -> Vec<(u32, u32)> {
        let mut out = Vec::new();
        let mut line = 0;
        let mut col = 0;
        for token in data {
            line += token.delta_line;
            col = if token.delta_line == 0 {
                col + token.delta_start
            } else {
                token.delta_start
            };
            out.push((line, col));
        }
        out
    }

    fn sym_string(legend: &SemanticTokensLegend, ty: u32, mods: u32) -> String {
        let base = legend.token_types[ty as usize].as_str().to_owned();
        let modifiers: Vec<String> = legend
            .token_modifiers
            .iter()
            .enumerate()
            .filter(|(bit, _)| mods & (1 << bit) != 0)
            .map(|(_, name)| name.as_str().to_owned())
            .collect();
        if modifiers.is_empty() {
            base
        } else {
            format!("{base}/{}", modifiers.join("+"))
        }
    }

    #[test]
    fn connection_labels_encode_with_the_connection_label_modifier() {
        // Arrange: one port-connection label and one parameter-override label,
        // plus a plain connected-signal reference for contrast.
        let nodes = vec![
            VObjectInfo {
                line: 5,
                col: 4,
                end_line: 5,
                end_col: 7,
                vpi_type: vpi::TOKEN_PORT_CONN_LABEL,
                name: Some("clk".to_owned()),
                file: "/x/top.sv".to_owned(),
            },
            VObjectInfo {
                line: 5,
                col: 9,
                end_line: 5,
                end_col: 11,
                vpi_type: vpi::uhdmref_obj,
                name: Some("wa".to_owned()),
                file: "/x/top.sv".to_owned(),
            },
            VObjectInfo {
                line: 6,
                col: 4,
                end_line: 6,
                end_col: 5,
                vpi_type: vpi::TOKEN_PARAM_CONN_LABEL,
                name: Some("W".to_owned()),
                file: "/x/top.sv".to_owned(),
            },
        ];
        let legend = legend();
        let connection_label_bit = legend
            .token_modifiers
            .iter()
            .position(|modifier| *modifier == TM_CONNECTION_LABEL_NAME)
            .expect("connectionLabel modifier in legend") as u32;

        // Act
        let encoded = encode(&nodes);

        // Assert
        let positions = decoded_positions(&encoded.data);
        assert_eq!(positions, vec![(4, 3), (4, 8), (5, 3)]);
        assert_eq!(
            sym_string(
                &legend,
                encoded.data[0].token_type,
                encoded.data[0].token_modifiers_bitset
            ),
            "function/connectionLabel"
        );
        // The connected signal carries NO connection-label modifier.
        assert_eq!(
            sym_string(
                &legend,
                encoded.data[1].token_type,
                encoded.data[1].token_modifiers_bitset
            ),
            "variable"
        );
        assert_eq!(
            encoded.data[1].token_modifiers_bitset & (1 << connection_label_bit),
            0,
            "connected signal must not carry connectionLabel"
        );
        assert_eq!(
            sym_string(
                &legend,
                encoded.data[2].token_type,
                encoded.data[2].token_modifiers_bitset
            ),
            "property/readonly+connectionLabel"
        );
    }

    #[test]
    fn supplemental_module_keyword_keeps_utf16_prefix_position_and_length() {
        // Arrange
        let file = "/virtual/unicode.sv";
        let mut parsed = Vec::new();
        tokens::supplement_source_local_module_tokens(
            file,
            "/* 😀 */ module Recovered; endmodule\n",
            &mut parsed,
        );
        let FileTokens { nodes, .. } = &parsed[0];
        let keyword_index = legend()
            .token_types
            .iter()
            .position(|token_type| *token_type == SemanticTokenType::KEYWORD)
            .expect("keyword token type") as u32;

        // Act
        let encoded = encode(nodes);
        let mut line = 0;
        let mut col = 0;
        let keyword = encoded.data.iter().find_map(|token| {
            line += token.delta_line;
            col = if token.delta_line == 0 {
                col + token.delta_start
            } else {
                token.delta_start
            };
            (token.token_type == keyword_index).then_some((line, col, token.length))
        });

        // Assert
        assert_eq!(
            keyword,
            Some((0, 9, "module".encode_utf16().count() as u32))
        );
    }
}
