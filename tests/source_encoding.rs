//! Source bytes that are not UTF-8: a comment keeps them with a warning,
//! anything else is a located error, and no byte moves a position.

use super::sim_harness;
use llg::core::compile::{self, CompileOpts, Diag, Severity};
use std::fs;
use std::path::Path;

fn write(directory: &Path, name: &str, bytes: &[u8]) -> String {
    let path = directory.join(name);
    fs::write(&path, bytes).expect("write source");
    path.to_string_lossy().into_owned()
}

fn compile(directory: &Path, bytes: &[u8]) -> Vec<Diag> {
    let top = write(directory, "top.sv", bytes);
    let opts = CompileOpts {
        files: vec![top],
        include_dirs: vec![directory.to_string_lossy().into_owned()],
        top: Some("tb".to_owned()),
        ..CompileOpts::default()
    };
    compile::compile(&opts)
        .expect("the file is read")
        .diagnostics
}

fn encoding(diagnostics: &[Diag]) -> Vec<(&Severity, u32, u32)> {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.message.contains("invalid UTF-8"))
        .map(|diagnostic| (&diagnostic.severity, diagnostic.line, diagnostic.col))
        .collect()
}

fn errors(diagnostics: &[Diag]) -> usize {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .count()
}

#[test]
fn a_line_comment_warns_and_keeps_positions() {
    let directory = sim_harness::TempDir::new("source-encoding").expect("temporary directory");
    let diagnostics = compile(
        directory.path(),
        b"// a \xbf b\nmodule tb; undeclared_t x; endmodule\n",
    );
    assert_eq!(encoding(&diagnostics), [(&Severity::Warning, 1, 6)]);
    let undeclared = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("undeclared_t"))
        .expect("a later error");
    assert_eq!((undeclared.line, undeclared.col), (2, 12));
}

#[test]
fn a_block_comment_warns_and_an_invalid_byte_after_it_on_the_line_keeps_its_column() {
    let directory = sim_harness::TempDir::new("source-encoding").expect("temporary directory");
    let diagnostics = compile(
        directory.path(),
        b"/* one\n \xff\xfe */ module tb; undeclared_t x; endmodule\n",
    );
    assert_eq!(encoding(&diagnostics), [(&Severity::Warning, 2, 2)]);
    let undeclared = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("undeclared_t"))
        .expect("a later error");
    assert_eq!((undeclared.line, undeclared.col), (2, 19));
}

#[test]
fn a_comment_with_a_quote_is_still_a_comment() {
    let directory = sim_harness::TempDir::new("source-encoding").expect("temporary directory");
    let diagnostics = compile(directory.path(), b"// \" \xbf \"\nmodule tb; endmodule\n");
    assert_eq!(errors(&diagnostics), 0);
    assert_eq!(encoding(&diagnostics), [(&Severity::Warning, 1, 6)]);
}

#[test]
fn a_string_literal_is_a_located_error() {
    let directory = sim_harness::TempDir::new("source-encoding").expect("temporary directory");
    let diagnostics = compile(
        directory.path(),
        b"module tb;\n  string s = \"a\xbf\";\nendmodule\n",
    );
    assert_eq!(encoding(&diagnostics), [(&Severity::Error, 2, 16)]);
}

#[test]
fn a_string_in_a_macro_body_is_a_located_error() {
    let directory = sim_harness::TempDir::new("source-encoding").expect("temporary directory");
    let diagnostics = compile(
        directory.path(),
        b"`define S \"\xbf\"\nmodule tb; string s = `S; endmodule\n",
    );
    assert!(errors(&diagnostics) > 0);
    assert!(
        encoding(&diagnostics).contains(&(&Severity::Error, 1, 12)),
        "{diagnostics:?}"
    );
}

#[test]
fn code_is_an_error() {
    let directory = sim_harness::TempDir::new("source-encoding").expect("temporary directory");
    let diagnostics = compile(directory.path(), b"module tb;\n  \xbf\nendmodule\n");
    assert!(errors(&diagnostics) > 0, "{diagnostics:?}");
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error && diagnostic.line == 2));
}

#[test]
fn an_escaped_identifier_is_an_error() {
    let directory = sim_harness::TempDir::new("source-encoding").expect("temporary directory");
    let diagnostics = compile(
        directory.path(),
        b"module tb;\n  int \\a\xbf ;\nendmodule\n",
    );
    assert!(errors(&diagnostics) > 0, "{diagnostics:?}");
}

#[test]
fn an_include_file_follows_the_same_rule() {
    let directory = sim_harness::TempDir::new("source-encoding").expect("temporary directory");
    write(directory.path(), "good.svh", b"// \xbf\n");
    let diagnostics = compile(
        directory.path(),
        b"`include \"good.svh\"\nmodule tb; endmodule\n",
    );
    assert_eq!(errors(&diagnostics), 0);
    let warning = encoding(&diagnostics);
    assert_eq!(warning, [(&Severity::Warning, 1, 4)]);
    assert!(diagnostics.iter().any(|diagnostic| diagnostic
        .file
        .as_deref()
        .is_some_and(|f| f.ends_with("good.svh"))));

    write(
        directory.path(),
        "bad.svh",
        b"parameter string P = \"\xbf\";\n",
    );
    let diagnostics = compile(
        directory.path(),
        b"module tb;\n`include \"bad.svh\"\nendmodule\n",
    );
    assert_eq!(encoding(&diagnostics), [(&Severity::Error, 1, 23)]);
}

#[test]
fn a_library_map_stays_rejected() {
    let directory = sim_harness::TempDir::new("source-encoding").expect("temporary directory");
    let map = write(
        directory.path(),
        "lib.map",
        b"// \xbf\nlibrary work *.sv;\n",
    );
    let opts = CompileOpts {
        library_map_files: vec![map],
        ..CompileOpts::default()
    };
    let error = compile::compile(&opts).expect_err("a map must be UTF-8");
    assert!(error.to_string().contains("not UTF-8"), "{error}");
}

#[test]
fn valid_utf8_is_unchanged() {
    let directory = sim_harness::TempDir::new("source-encoding").expect("temporary directory");
    let diagnostics = compile(
        directory.path(),
        "// caf\u{e9}\nmodule tb; string s = \"caf\u{e9}\"; endmodule\n".as_bytes(),
    );
    assert_eq!(errors(&diagnostics), 0);
    assert!(encoding(&diagnostics).is_empty());
    let read = compile::SourceText::from_bytes("caf\u{e9}".as_bytes().to_vec());
    assert_eq!((read.text.as_str(), read.raw), ("caf\u{e9}", None));
}

#[test]
fn text_keeps_every_offset_of_the_file() {
    let bytes = b"a\xbfb\xe2\x82\n\xf0\x9f\x98\x80\xff".to_vec();
    let read = compile::SourceText::from_bytes(bytes.clone());
    assert_eq!(read.text.len(), bytes.len());
    assert_eq!(read.text, "a?b??\n\u{1f600}?");
    assert_eq!(read.raw.as_deref(), Some(bytes.as_slice()));
}
