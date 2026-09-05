//! User-facing explanations of Surelog parser diagnostics.
//!
//! Presentation only: the original `Diag` stays intact for logging and failure
//! classification. No source reads are needed, including for unsaved LSP buffers.

use super::compile::{Diag, Severity};

/// Explain recognized parser messages without exposing ANTLR grammar names.
/// Suggestions describe possible causes, not a diagnosis inferred from a filename.
/// Unrecognized messages and non-syntax diagnostics pass through unchanged.
pub fn user_message(diagnostic: &Diag) -> String {
    if diagnostic.severity != Severity::Syntax {
        return diagnostic.message.clone();
    }
    let Some(message) = diagnostic.message.strip_prefix("Syntax error: ") else {
        return diagnostic.message.clone();
    };
    explain_syntax(message).unwrap_or_else(|| diagnostic.message.clone())
}

fn explain_syntax(message: &str) -> Option<String> {
    if let Some(input) = message.strip_prefix("no viable alternative at input ") {
        let input = unquote(input)?;
        if looks_like_instance(input) {
            return Some(format!(
                "Syntax error near `{input}`: this looks like a module instance. \
                 Check that it is inside a `module ... endmodule` block and that \
                 the preceding declaration is complete. If this file is an \
                 instantiation template, copy the instance into your module, \
                 replace its placeholders, and exclude the template from the \
                 project's source files. Later port-connection errors may be \
                 caused by this first error."
            ));
        }
        return Some(format!(
            "Syntax error {}: this does not form a valid Verilog/SystemVerilog \
             statement or declaration here. Check the surrounding code for a \
             missing expression, delimiter, or enclosing block. If an earlier \
             syntax error is reported, fix it first.",
            near(input)
        ));
    }
    if let Some(rest) = message.strip_prefix("missing ") {
        let (expected, input) = rest.rsplit_once(" at ")?;
        let input = unquote(input)?;
        let expected = expected_tokens(expected)?;
        return Some(format!(
            "Syntax error: expected {expected} before {}. Check for missing text or \
             punctuation immediately before this point; an earlier syntax error \
             can also cause this message.",
            token(input)
        ));
    }
    for prefix in ["extraneous input ", "mismatched input "] {
        if let Some(rest) = message.strip_prefix(prefix) {
            let (input, expected) = rest.rsplit_once(" expecting ")?;
            let input = unquote(input)?;
            let expected = expected_tokens(expected)?;
            let hint = if input == "." {
                "For a named port connection such as `.port(signal)`, check that \
                 it is inside an instance's parentheses and that the instance \
                 is inside a module. Fix any earlier syntax error first."
            } else {
                "Check for missing or misplaced text just before this point. \
                 Fix any earlier syntax error first."
            };
            return Some(format!(
                "Syntax error: unexpected {}; expected {expected}. {hint}",
                token(input)
            ));
        }
    }
    None
}

fn unquote(text: &str) -> Option<&str> {
    text.strip_prefix('\'')?.strip_suffix('\'')
}

fn token(text: &str) -> String {
    if text == "<EOF>" {
        "end of file".to_owned()
    } else {
        format!("`{text}`")
    }
}

fn near(text: &str) -> String {
    if text == "<EOF>" {
        "at end of file".to_owned()
    } else {
        format!("near {}", token(text))
    }
}

fn expected_tokens(text: &str) -> Option<String> {
    let text = text.trim();
    let set = text.strip_prefix('{').and_then(|s| s.strip_suffix('}'));
    let mut names = Vec::new();
    for item in set.unwrap_or(text).split(", ") {
        let name = match item {
            "Simple_identifier" | "Escaped_identifier" => "a name (identifier)".to_owned(),
            "DOLLAR_UNIT" => "`$unit`".to_owned(),
            "EOF" | "<EOF>" => "end of file".to_owned(),
            _ => token(unquote(item)?),
        };
        if !names.contains(&name) {
            names.push(name);
        }
    }
    // Large recovery sets describe several grammar rules, not a useful menu
    // of edits. Retain only broad categories supported by this particular set.
    if names.len() > 5 {
        let has_name = names.iter().any(|name| name == "a name (identifier)");
        let has_type = names
            .iter()
            .any(|name| name == "`logic`" || name == "`int`");
        return Some(match (has_name, has_type) {
            (true, true) => "a name, a data type, or another declaration or expression".to_owned(),
            (true, false) => "a name or another valid expression or declaration".to_owned(),
            _ => "a valid expression, declaration, or statement".to_owned(),
        });
    }
    Some(names.join(" or "))
}

fn looks_like_instance(input: &str) -> bool {
    let Some(head) = input.trim_end().strip_suffix('(') else {
        return false;
    };
    let words: Vec<_> = head.split_whitespace().collect();
    words.len() == 2
        && !matches!(
            words[0],
            "module"
                | "macromodule"
                | "interface"
                | "program"
                | "function"
                | "task"
                | "class"
                | "if"
                | "else"
                | "while"
                | "for"
                | "repeat"
                | "wait"
                | "always"
                | "initial"
                | "assign"
                | "return"
                | "wire"
                | "reg"
                | "logic"
                | "bit"
                | "int"
                | "integer"
                | "real"
                | "input"
                | "output"
                | "inout"
                | "parameter"
                | "localparam"
        )
        && words.iter().all(|word| simple_identifier(word))
}

fn simple_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diagnostic(message: &str) -> Diag {
        Diag {
            severity: Severity::Syntax,
            file: Some("/project/source.v".to_owned()),
            line: 2,
            col: 5,
            message: message.to_owned(),
        }
    }

    #[test]
    fn instance_error_explains_template_usage_without_claiming_a_certain_cause() {
        let raw =
            diagnostic("Syntax error: no viable alternative at input 'edb_top edb_top_inst ('");
        let original = raw.clone();
        let message = user_message(&raw);
        assert!(message.contains("looks like a module instance"));
        assert!(message.contains("`module ... endmodule`"));
        assert!(message.contains("If this file is an instantiation template"));
        assert!(message.contains("replace its placeholders"));
        assert_eq!(raw, original);
    }

    #[test]
    fn recovery_errors_use_readable_names_and_context() {
        for (input, expected) in [
            ("extraneous input '.' expecting {'virtual', 'type', 'enum', 'struct', 'union', 'string', 'chandle', 'event', 'byte', 'shortint', 'int', 'longint', 'integer', 'time', 'bit', 'logic', 'reg', 'shortreal', 'real', 'realtime', DOLLAR_UNIT, 'this', 'randomize', 'sample', Escaped_identifier, Simple_identifier}", "a name, a data type"),
            ("no viable alternative at input '('", "missing expression"),
            ("missing {'this', 'randomize', 'sample', Escaped_identifier, Simple_identifier} at ','", "a name (identifier)"),
            ("extraneous input '.' expecting {'this', 'randomize', 'sample', Escaped_identifier, Simple_identifier}", "named port connection"),
            ("missing ';' at '('", "expected `;` before `(`"),
            ("mismatched input '<EOF>' expecting 'endmodule'", "unexpected end of file; expected `endmodule`"),
            ("missing Simple_identifier at ','", "expected a name (identifier) before `,`"),
            ("missing ')' at '<EOF>'", "expected `)` before end of file"),
            ("no viable alternative at input '4'b001x'", "`4'b001x`"),
        ] {
            let message = user_message(&diagnostic(&format!("Syntax error: {input}")));
            assert!(message.contains(expected), "{message}");
            for jargon in ["viable alternative", "extraneous input", "Simple_identifier", "Escaped_identifier", "DOLLAR_UNIT", "<EOF>"] {
                assert!(!message.contains(jargon), "{message}");
            }
        }
    }

    #[test]
    fn unrelated_and_unrecognized_messages_are_preserved() {
        for message in [
            "Undefined module: edb_top",
            "Syntax error: an unfamiliar parser error",
            "Syntax error: missing something",
            "Syntax error: mismatched input '.' expecting UNKNOWN_TOKEN",
        ] {
            assert_eq!(user_message(&diagnostic(message)), message);
        }
        let mut raw = diagnostic("Syntax error: no viable alternative at input 'a b ('");
        raw.severity = Severity::Warning;
        assert_eq!(user_message(&raw), raw.message);
    }

    #[test]
    fn module_declaration_does_not_get_instance_template_advice() {
        for input in ["module top (", "logic signal (", "assign value ("] {
            let message = user_message(&diagnostic(&format!(
                "Syntax error: no viable alternative at input '{input}'"
            )));
            assert!(!message.contains("template"), "{message}");
        }
    }
}
