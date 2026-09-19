//! Strict edition capabilities over the owned frontend snapshot.
//!
//! The vendored frontend bridges 2001 -> 2005 and 2009 -> 2017. Keyword
//! remapping alone is not an edition boundary. Keep builtin/name, token, and
//! kind-specific checks together, shared by CLI and navigation compilations.
//! Tables describe admission, not executable simulator support. Unknown system
//! names require an explicit prototype; they are never silently treated as a
//! standard builtin of an older edition.
use super::{one_based_utf16_position, Diag, LanguageEdition, Severity, Snapshot};
use crate::ffi::slang::{
    LexicalKind, LexicalRole, SemanticKind, SemanticOperation, SourceRange,
    CLASS_INTERFACE, SEMANTIC_ASSERTION_FINAL, SEMANTIC_STMT_CONCURRENT_ASSERT,
    SEMANTIC_STMT_CONCURRENT_ASSUME, SEMANTIC_STMT_CONCURRENT_COVER,
    SEMANTIC_STMT_CONCURRENT_EXPECT, SEMANTIC_STMT_IMMEDIATE_ASSERT,
    SEMANTIC_STMT_IMMEDIATE_ASSUME, SEMANTIC_STMT_IMMEDIATE_COVER,
};
use std::collections::{HashMap, HashSet};

// IEEE 1364-2001 Clauses 15/17/18 and Annex C, plus $feof (17.2.4).
// Optional standard facilities remain admitted; this does not promise runtime
// implementation of every optional facility.
const VERILOG_SYSTEM_NAMES: &[&str] = &[
    "$async$and$array",
    "$async$and$plane",
    "$async$nand$array",
    "$async$nand$plane",
    "$async$nor$array",
    "$async$nor$plane",
    "$async$or$array",
    "$async$or$plane",
    "$bitstoreal",
    "$countdrivers",
    "$display",
    "$displayb",
    "$displayh",
    "$displayo",
    "$dist_chi_square",
    "$dist_erlang",
    "$dist_exponential",
    "$dist_normal",
    "$dist_poisson",
    "$dist_t",
    "$dist_uniform",
    "$dumpall",
    "$dumpfile",
    "$dumpflush",
    "$dumplimit",
    "$dumpoff",
    "$dumpon",
    "$dumpports",
    "$dumpportsall",
    "$dumpportsflush",
    "$dumpportslimit",
    "$dumpportsoff",
    "$dumpportson",
    "$dumpvars",
    "$fclose",
    "$fdisplay",
    "$fdisplayb",
    "$fdisplayh",
    "$fdisplayo",
    "$feof",
    "$ferror",
    "$fflush",
    "$fgetc",
    "$fgets",
    "$finish",
    "$fmonitor",
    "$fmonitorb",
    "$fmonitorh",
    "$fmonitoro",
    "$fopen",
    "$fread",
    "$fscanf",
    "$fseek",
    "$fstrobe",
    "$fstrobeb",
    "$fstrobeh",
    "$fstrobeo",
    "$ftell",
    "$fullskew",
    "$fwrite",
    "$fwriteb",
    "$fwriteh",
    "$fwriteo",
    "$getpattern",
    "$hold",
    "$incsave",
    "$input",
    "$itor",
    "$key",
    "$list",
    "$log",
    "$monitor",
    "$monitorb",
    "$monitorh",
    "$monitoro",
    "$monitoroff",
    "$monitoron",
    "$nochange",
    "$nokey",
    "$nolog",
    "$period",
    "$printtimescale",
    "$q_add",
    "$q_exam",
    "$q_full",
    "$q_initialize",
    "$q_remove",
    "$random",
    "$readmemb",
    "$readmemh",
    "$realtime",
    "$realtobits",
    "$recovery",
    "$recrem",
    "$removal",
    "$reset",
    "$reset_count",
    "$reset_value",
    "$restart",
    "$rewind",
    "$rtoi",
    "$save",
    "$scale",
    "$scope",
    "$sdf_annotate",
    "$setup",
    "$setuphold",
    "$sformat",
    "$showscopes",
    "$showvars",
    "$signed",
    "$skew",
    "$sreadmemb",
    "$sreadmemh",
    "$sscanf",
    "$stime",
    "$stop",
    "$strobe",
    "$strobeb",
    "$strobeh",
    "$strobeo",
    "$swrite",
    "$swriteb",
    "$swriteh",
    "$swriteo",
    "$sync$and$array",
    "$sync$and$plane",
    "$sync$nand$array",
    "$sync$nand$plane",
    "$sync$nor$array",
    "$sync$nor$plane",
    "$sync$or$array",
    "$sync$or$plane",
    "$test$plusargs",
    "$time",
    "$timeformat",
    "$timeskew",
    "$ungetc",
    "$unsigned",
    "$value$plusargs",
    "$width",
    "$write",
    "$writeb",
    "$writeh",
    "$writeo",
];
// IEEE 1800-2009 Clauses 18, 20, 21 and checker inference (17).
// $root / $unit / $global_clock are language names, not callable routines.
const SYSTEMVERILOG_SYSTEM_NAMES: &[&str] = &[
    "$acos", "$acosh", "$asin", "$asinh", "$assertfailoff",
    "$assertfailon", "$assertkill", "$assertnonvacuouson", "$assertoff", "$asserton",
    "$assertpassoff", "$assertpasson", "$assertvacuousoff", "$atan", "$atan2",
    "$atanh", "$bits", "$bitstoshortreal", "$cast", "$ceil",
    "$changed", "$changed_gclk", "$changing_gclk", "$clog2", "$cos",
    "$cosh", "$countones", "$coverage_control", "$coverage_get", "$coverage_get_max",
    "$coverage_merge", "$coverage_save", "$dimensions", "$error", "$exit",
    "$exp", "$falling_gclk", "$fatal", "$fell", "$fell_gclk",
    "$floor", "$future_gclk", "$get_coverage", "$global_clock", "$high",
    "$hypot", "$increment", "$inferred_clock", "$inferred_disable", "$info",
    "$isunbounded", "$isunknown", "$left", "$ln", "$load_coverage_db",
    "$log10", "$low", "$onehot", "$onehot0", "$past",
    "$past_gclk", "$pow", "$right", "$rising_gclk", "$root",
    "$rose", "$rose_gclk", "$sampled", "$set_coverage_db_name", "$sformatf",
    "$shortrealtobits", "$sin", "$sinh", "$size", "$sqrt",
    "$stable", "$stable_gclk", "$steady_gclk", "$system", "$tan",
    "$tanh", "$typename", "$unit", "$unpacked_dimensions", "$urandom",
    "$urandom_range", "$warning", "$writememb", "$writememh",
];
// Keyword inventories: supplied LRMs, Annex B; spellings cross-checked against
// the vendored LexerFacts version tables. A spelling is only checked when the
// frontend classified the token as a keyword. Old-edition identifiers named
// "logic", "soft", etc. therefore remain valid identifiers.
const VERILOG_KEYWORDS: &[&str] = &[
    "always", "and", "assign", "automatic", "begin",
    "buf", "bufif0", "bufif1", "case", "casex",
    "casez", "cell", "cmos", "config", "deassign",
    "default", "defparam", "design", "disable", "edge",
    "else", "end", "endcase", "endconfig", "endfunction",
    "endgenerate", "endmodule", "endprimitive", "endspecify", "endtable",
    "endtask", "event", "for", "force", "forever",
    "fork", "function", "generate", "genvar", "highz0",
    "highz1", "if", "ifnone", "incdir", "include",
    "initial", "inout", "input", "instance", "integer",
    "join", "large", "liblist", "library", "localparam",
    "macromodule", "medium", "module", "nand", "negedge",
    "nmos", "nor", "noshowcancelled", "not", "notif0",
    "notif1", "or", "output", "parameter", "pmos",
    "posedge", "primitive", "pull0", "pull1", "pulldown",
    "pullup", "pulsestyle_ondetect", "pulsestyle_onevent", "rcmos", "real",
    "realtime", "reg", "release", "repeat", "rnmos",
    "rpmos", "rtran", "rtranif0", "rtranif1", "scalared",
    "showcancelled", "signed", "small", "specify", "specparam",
    "strong0", "strong1", "supply0", "supply1", "table",
    "task", "time", "tran", "tranif0", "tranif1",
    "tri", "tri0", "tri1", "triand", "trior",
    "trireg", "unsigned", "use", "vectored", "wait",
    "wand", "weak0", "weak1", "while", "wire",
    "wor", "xnor", "xor",
];
const SYSTEMVERILOG_KEYWORDS: &[&str] = &[
    "1step", "accept_on", "alias", "always_comb", "always_ff", "always_latch",
    "assert", "assume", "before", "bind", "bins",
    "binsof", "bit", "break", "byte", "chandle",
    "checker", "class", "clocking", "const", "constraint",
    "context", "continue", "cover", "covergroup", "coverpoint",
    "cross", "dist", "do", "endchecker", "endclass",
    "endclocking", "endgroup", "endinterface", "endpackage", "endprogram",
    "endproperty", "endsequence", "enum", "eventually", "expect",
    "export", "extends", "extern", "final", "first_match",
    "foreach", "forkjoin", "global", "iff", "ignore_bins",
    "illegal_bins", "implies", "import", "inside", "int",
    "interface", "intersect", "join_any", "join_none", "let",
    "local", "logic", "longint", "matches", "modport",
    "new", "nexttime", "null", "package", "packed",
    "priority", "program", "property", "protected", "pure",
    "rand", "randc", "randcase", "randsequence", "ref",
    "reject_on", "restrict", "return", "s_always", "s_eventually",
    "s_nexttime", "s_until", "s_until_with", "sequence", "shortint",
    "shortreal", "solve", "static", "string", "strong",
    "struct", "super", "sync_accept_on", "sync_reject_on", "tagged",
    "this", "throughout", "timeprecision", "timeunit", "type",
    "typedef", "union", "unique", "unique0", "until",
    "until_with", "untyped", "uwire", "var", "virtual",
    "void", "wait_order", "weak", "wildcard", "with",
    "within",
];

fn listed(table: &[&str], name: &str) -> bool {
    table.binary_search(&name).is_ok()
}

fn system_name_allowed(name: &str, edition: LanguageEdition, extensions: &HashSet<&str>) -> bool {
    extensions.contains(name)
        || listed(VERILOG_SYSTEM_NAMES, name)
        || (edition == LanguageEdition::SystemVerilog2009
            && listed(SYSTEMVERILOG_SYSTEM_NAMES, name))
}

fn keyword_allowed(name: &str, edition: LanguageEdition) -> bool {
    listed(VERILOG_KEYWORDS, name)
        || (edition == LanguageEdition::SystemVerilog2009
            && listed(SYSTEMVERILOG_KEYWORDS, name))
}

/// Native prototype parsing has already succeeded. Match the same first-dollar
/// name that UserDefinedSubroutine::create parses; do not whitelist substrings
/// in arbitrary source comments, strings, includes, or an unregistered call.
fn extension_name(prototype: &str) -> Option<&str> {
    let tail = &prototype[prototype.find('$')?..];
    let len = tail.bytes().take_while(|c| c.is_ascii_alphanumeric() || matches!(*c, b'_' | b'$')).count();
    (len > 1).then_some(&tail[..len])
}

fn immediate(subkind: u32) -> bool {
    matches!(subkind, SEMANTIC_STMT_IMMEDIATE_ASSERT | SEMANTIC_STMT_IMMEDIATE_ASSUME
        | SEMANTIC_STMT_IMMEDIATE_COVER)
}

pub(super) fn edition_diagnostics(
    snapshot: &Snapshot,
    edition: LanguageEdition,
    prototypes: &[String],
) -> Vec<Diag> {
    let extensions: HashSet<_> = prototypes.iter().filter_map(|p| extension_name(p)).collect();
    let files: HashMap<_, _> = snapshot.files.iter().map(|f| (f.id, f)).collect();
    let mut violations: Vec<(Option<SourceRange>, String)> = Vec::new();
    for node in &snapshot.semantic_nodes {
        if node.kind == SemanticKind::SystemCall && node.name.starts_with('$')
            && !system_name_allowed(&node.name, edition, &extensions) {
            violations.push((node.range, node.name.clone()));
        }
        // Auxiliary tags are kind-scoped: an expression's numeric subtype is
        // not an assertion-statement tag, nor a streaming slice a final flag.
        if node.kind == SemanticKind::Statement {
            if immediate(node.subkind) && node.auxiliary & SEMANTIC_ASSERTION_FINAL != 0 {
                violations.push((node.range, "assert final".to_owned()));
            } else if edition == LanguageEdition::Verilog2001
                && (immediate(node.subkind) || matches!(node.subkind,
                    SEMANTIC_STMT_CONCURRENT_ASSERT | SEMANTIC_STMT_CONCURRENT_ASSUME
                    | SEMANTIC_STMT_CONCURRENT_COVER | SEMANTIC_STMT_CONCURRENT_EXPECT)) {
                violations.push((node.range, "SystemVerilog assertion".to_owned()));
            }
        }
        if node.kind == SemanticKind::Class && node.auxiliary & CLASS_INTERFACE != 0 {
            violations.push((node.range, "interface class".to_owned()));
        }
        if edition == LanguageEdition::Verilog2001 && node.kind == SemanticKind::Expression
            && matches!(node.operation,
                SemanticOperation::LogicalImplication | SemanticOperation::LogicalEquivalence
                | SemanticOperation::WildcardEqual | SemanticOperation::WildcardNotEqual
                | SemanticOperation::PreIncrement | SemanticOperation::PreDecrement
                | SemanticOperation::PostIncrement | SemanticOperation::PostDecrement
                | SemanticOperation::StreamLeft | SemanticOperation::StreamRight
                | SemanticOperation::Inside | SemanticOperation::AssignmentPattern
                | SemanticOperation::MultiAssignmentPattern) {
            violations.push((node.range, "SystemVerilog expression".to_owned()));
        }
    }
    // Navigation snapshots deliberately omit executable bodies. Use the
    // frontend's classified tokens as well, not a raw-source substring scan.
    // This covers macros, inactive branches, strings and escaped identifiers
    // without confusing those with actual language constructs.
    let mut tokens: Vec<_> = snapshot.lexical_tokens.iter()
        .filter(|t| !t.is_missing && !t.is_skipped && !t.is_directive && t.range.is_some())
        .filter(|t| t.kind != LexicalKind::Macro || t.is_macro_expansion)
        .collect();
    tokens.sort_by_key(|t| t.range.map(|r| (r.file_id, r.start, r.end)));
    for (i, token) in tokens.iter().enumerate() {
        let keyword = token.kind == LexicalKind::Keyword || token.role == LexicalRole::Keyword;
        if keyword && !keyword_allowed(&token.text, edition) {
            violations.push((token.range, token.text.clone()));
        }
        if !matches!(token.kind, LexicalKind::String | LexicalKind::Number | LexicalKind::Operator)
            && token.text.len() > 1 && token.text.starts_with('$')
            && !system_name_allowed(&token.text, edition, &extensions) {
            violations.push((token.range, token.text.clone()));
        }
        if edition == LanguageEdition::Verilog2001 && token.kind == LexicalKind::Operator
            && matches!(token.text.as_str(), "++" | "--" | "+=" | "-=" | "*=" | "/=" | "%="
                | "&=" | "|=" | "^=" | "<<=" | ">>=" | "<<<=" | ">>>="
                | "==?" | "!=?" | "##" | "::" | ".*" | "->>" | "<->") {
            violations.push((token.range, token.text.clone()));
        }
        let Some(next) = tokens.get(i + 1) else { continue; };
        if token.range.map(|r| r.file_id) != next.range.map(|r| r.file_id) { continue; }
        let next_keyword = next.kind == LexicalKind::Keyword || next.role == LexicalRole::Keyword;
        if keyword && next_keyword {
            if matches!(token.text.as_str(), "assert" | "assume" | "cover") && next.text == "final" {
                violations.push((token.range, "assert final".to_owned()));
            }
            if token.text == "interface" && next.text == "class" {
                violations.push((token.range, "interface class".to_owned()));
            }
        }
        if keyword && token.text == "unique" && next.text == "{" {
            violations.push((token.range, "uniqueness constraint".to_owned()));
        }
    }
    let mut seen = HashSet::new();
    violations.into_iter().filter_map(|(range, label)| {
        let key = (range.map(|r| (r.file_id, r.start)), label.clone());
        if !seen.insert(key) { return None; }
        let (file, line, col) = range.map_or((None, 0, 0), |range| {
            let Some(file) = files.get(&range.file_id) else { return (None, 0, 0); };
            let (line, col) = one_based_utf16_position(&file.text, range.start);
            (Some(file.name.clone()), line, col)
        });
        Some(Diag {
            severity: Severity::Error, file, line, col,
            message: format!("`{label}` is not available in IEEE {edition} and is rejected by the strict edition profile"),
        })
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_tables_are_sorted_unique_and_disjoint() {
        for table in [VERILOG_SYSTEM_NAMES, SYSTEMVERILOG_SYSTEM_NAMES, VERILOG_KEYWORDS, SYSTEMVERILOG_KEYWORDS] {
            assert!(table.windows(2).all(|w| w[0] < w[1]));
        }
        assert!(VERILOG_SYSTEM_NAMES.iter().all(|s| !listed(SYSTEMVERILOG_SYSTEM_NAMES, s)));
        assert!(VERILOG_KEYWORDS.iter().all(|s| !listed(SYSTEMVERILOG_KEYWORDS, s)));
    }

    #[test]
    fn system_name_policy_is_closed_without_silent_newer_builtins() {
        let none = HashSet::new();
        for name in ["$asserton", "$assertoff", "$assertkill", "$bits", "$urandom", "$writememh"] {
            assert!(!system_name_allowed(name, LanguageEdition::Verilog2001, &none));
            assert!(system_name_allowed(name, LanguageEdition::SystemVerilog2009, &none));
        }
        for name in ["$assertcontrol", "$countbits", "$static_assert", "$made_up"] {
            for edition in [LanguageEdition::Verilog2001, LanguageEdition::SystemVerilog2009] {
                assert!(!system_name_allowed(name, edition, &none));
            }
        }
        for name in ["$display", "$signed", "$readmemh", "$fscanf", "$test$plusargs", "$setup", "$fullskew"] {
            assert!(system_name_allowed(name, LanguageEdition::Verilog2001, &none));
        }
    }

    #[test]
    fn registered_extensions_are_explicit_not_standard_capabilities() {
        let name = extension_name("function int $custom(input int arg);").unwrap();
        assert_eq!(name, "$custom");
        let extensions = HashSet::from([name]);
        assert!(system_name_allowed(name, LanguageEdition::Verilog2001, &extensions));
        assert!(!system_name_allowed("$custom2", LanguageEdition::Verilog2001, &extensions));
        assert!(extension_name("task ordinary();").is_none());
    }

    #[test]
    fn global_keyword_profile_cannot_be_relaxed_by_begin_keywords() {
        assert!(!keyword_allowed("1step", LanguageEdition::Verilog2001));
        assert!(keyword_allowed("1step", LanguageEdition::SystemVerilog2009));
        assert!(!keyword_allowed("logic", LanguageEdition::Verilog2001));
        assert!(keyword_allowed("final", LanguageEdition::SystemVerilog2009));
        for name in ["soft", "interconnect", "nettype", "implements"] {
            assert!(!keyword_allowed(name, LanguageEdition::SystemVerilog2009));
        }
    }
}
