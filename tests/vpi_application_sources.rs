//! Portability scan of the checked-in VPI application C sources.
//!
//! IEEE 1800-2009 38.2: "The error status shall be reset by any VPI routine
//! call except vpi_chk_error()." In C11 (6.5p3, 6.5.2.2p10) the calls in the
//! arguments of one call, or in the operands of one operator, are
//! indeterminately sequenced: their order is unspecified. `vpi_printf("%s %d", vpi_get_str(vpiType, h),
//! vpi_chk_error(NULL))` therefore reports the status of the call before the
//! statement where the compiler evaluates the arguments from right to left
//! (GCC on x86_64) and 0 where it evaluates them from left to right (GCC on
//! AArch64, Clang everywhere): CI on fd95a315 failed A19-D7 and A19-D8 on
//! every arm64 runner that way. Read the status into a variable first.

use std::path::Path;

/// One unsequenced use of `vpi_chk_error`: the byte offset of the
/// `vpi_chk_error` name and the VPI routine evaluated in an unspecified order
/// with it.
#[derive(Debug, PartialEq, Eq)]
struct Finding {
    offset: usize,
    other: String,
}

const CHECK: &str = "vpi_chk_error";

/// `text` with comments, string literals and character literals replaced by
/// spaces of the same length (newlines kept), so offsets and line numbers
/// stay valid and only code is scanned.
fn code_only(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let mut out = bytes.to_vec();
    let mut at = 0;
    let blank = |out: &mut Vec<u8>, from: usize, to: usize| {
        for byte in &mut out[from..to] {
            if *byte != b'\n' {
                *byte = b' ';
            }
        }
    };
    while at < bytes.len() {
        match bytes[at] {
            b'/' if bytes.get(at + 1) == Some(&b'/') => {
                let end = text[at..].find('\n').map_or(bytes.len(), |end| at + end);
                blank(&mut out, at, end);
                at = end;
            }
            b'/' if bytes.get(at + 1) == Some(&b'*') => {
                let end = text[at + 2..]
                    .find("*/")
                    .map_or(bytes.len(), |end| at + 2 + end + 2);
                blank(&mut out, at, end);
                at = end;
            }
            quote @ (b'"' | b'\'') => {
                let mut end = at + 1;
                while end < bytes.len() && bytes[end] != quote && bytes[end] != b'\n' {
                    end += if bytes[end] == b'\\' { 2 } else { 1 };
                }
                let end = (end + 1).min(bytes.len());
                blank(&mut out, at + 1, end.saturating_sub(1).max(at + 1));
                at = end;
            }
            _ => at += 1,
        }
    }
    out
}

fn is_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Keywords whose parenthesized expression is a full expression.
const CONTROL: &[&[u8]] = &[b"if", b"while", b"for", b"switch"];

/// The end of the identifier that starts at `start`.
fn name_end(code: &[u8], start: usize) -> usize {
    let mut end = start;
    while end < code.len() && is_word(code[end]) {
        end += 1;
    }
    end
}

/// The offset of the bracket that closes the one at `open`, or the end.
fn closing(code: &[u8], open: usize) -> usize {
    let mut depth = 0usize;
    for (at, byte) in code.iter().enumerate().skip(open) {
        match byte {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                depth -= 1;
                if depth == 0 {
                    return at;
                }
            }
            _ => {}
        }
    }
    code.len()
}

/// The VPI routine calls (`vpi_*(`) in `code[from..to]`: name and offset.
fn vpi_calls(code: &[u8], from: usize, to: usize) -> Vec<(String, usize)> {
    let mut calls = Vec::new();
    let mut at = from;
    while at + 4 <= to {
        if code[at..].starts_with(b"vpi_") && (at == 0 || !is_word(code[at - 1])) {
            let mut end = at;
            while end < to && is_word(code[end]) {
                end += 1;
            }
            let mut open = end;
            while open < to && code[open].is_ascii_whitespace() {
                open += 1;
            }
            if open < to && code[open] == b'(' {
                calls.push((String::from_utf8_lossy(&code[at..end]).into_owned(), at));
            }
            at = end;
        } else {
            at += 1;
        }
    }
    calls
}

/// Report each `vpi_chk_error` call in `code[from..to]` together with a VPI
/// call it is unsequenced against. `parts` are the spans evaluated in an
/// unspecified order relative to each other (the arguments of one call, or
/// the operands between two sequence points of one expression).
fn check_unsequenced(code: &[u8], parts: &[(usize, usize)], findings: &mut Vec<Finding>) {
    for (index, &(from, to)) in parts.iter().enumerate() {
        for (name, offset) in vpi_calls(code, from, to) {
            if name != CHECK {
                continue;
            }
            let other = parts
                .iter()
                .enumerate()
                .filter(|(other, _)| *other != index)
                .flat_map(|(_, &(from, to))| vpi_calls(code, from, to))
                .find(|(name, _)| name != CHECK);
            if let Some((other, _)) = other {
                findings.push(Finding { offset, other });
            }
        }
    }
}

/// Scan the expression `code[from..to]`. Its depth-0 items between two
/// sequencing operators (`&&`, `||`, `?`, `:`, `,`) are unsequenced against
/// each other, where an item is a call or a parenthesized group; a call's
/// arguments are unsequenced against each other. A call is sequenced after
/// its own arguments, so an enclosing `vpi_printf` is not reported.
fn scan_expression(code: &[u8], from: usize, to: usize, findings: &mut Vec<Finding>) {
    let mut segment: Vec<(usize, usize)> = Vec::new();
    let mut at = from;
    while at < to {
        let byte = code[at];
        let pair = code.get(at..at + 2);
        if pair == Some(b"&&") || pair == Some(b"||") {
            check_unsequenced(code, &segment, findings);
            segment.clear();
            at += 2;
            continue;
        }
        if matches!(byte, b'?' | b':' | b',' | b';') {
            check_unsequenced(code, &segment, findings);
            segment.clear();
            at += 1;
            continue;
        }
        if matches!(byte, b'(' | b'[' | b'{') {
            let close = closing(code, at).min(to);
            // Back up over a callee name so the item covers the whole call.
            let mut start = at;
            while start > from && code[start - 1].is_ascii_whitespace() {
                start -= 1;
            }
            let named = start > from && is_word(code[start - 1]);
            while start > from && is_word(code[start - 1]) {
                start -= 1;
            }
            let start = if named { start } else { at };
            if named && CONTROL.contains(&&code[start..name_end(code, start)]) {
                // A controlling expression is a full expression: it is
                // sequenced before the statement it controls.
                check_unsequenced(code, &segment, findings);
                segment.clear();
                scan_expression(code, at + 1, close, findings);
                at = close + 1;
                continue;
            }
            segment.push((start, (close + 1).min(to)));
            let mut arguments = Vec::new();
            let mut argument = at + 1;
            let mut inner = at + 1;
            while inner < close {
                match code[inner] {
                    b'(' | b'[' | b'{' => inner = closing(code, inner) + 1,
                    b',' => {
                        arguments.push((argument, inner));
                        argument = inner + 1;
                        inner += 1;
                    }
                    _ => inner += 1,
                }
            }
            arguments.push((argument, close));
            if named && byte == b'(' {
                check_unsequenced(code, &arguments, findings);
                for &(from, to) in &arguments {
                    scan_expression(code, from, to, findings);
                }
            } else {
                scan_expression(code, at + 1, close, findings);
            }
            at = close + 1;
            continue;
        }
        at += 1;
    }
    check_unsequenced(code, &segment, findings);
}

/// Every unsequenced `vpi_chk_error` in the C source `text`.
fn unsequenced_status_reads(text: &str) -> Vec<Finding> {
    let code = code_only(text);
    let mut findings = Vec::new();
    let mut start = 0;
    let mut at = 0;
    while at < code.len() {
        match code[at] {
            // A statement or a block boundary is a sequence point.
            b';' | b'{' | b'}' => {
                scan_expression(&code, start, at, &mut findings);
                start = at + 1;
            }
            b'(' | b'[' => {
                // `for (;;)` headers and nested blocks in a call stay whole.
                let close = closing(&code, at);
                if code[at..close.min(code.len())].contains(&b'{') {
                    at += 1;
                    continue;
                }
                at = close;
            }
            _ => {}
        }
        at += 1;
    }
    scan_expression(&code, start, code.len(), &mut findings);
    findings.sort_by_key(|finding| finding.offset);
    findings.dedup_by_key(|finding| finding.offset);
    findings
}

#[test]
fn unsequenced_status_scan_finds_the_argument_and_operand_shapes() {
    let names = |text: &str| -> Vec<String> {
        unsequenced_status_reads(text)
            .into_iter()
            .map(|finding| finding.other)
            .collect()
    };
    // CI on fd95a315: A19-D7 and A19-D8 printed 0 on AArch64.
    assert_eq!(
        names(
            "vpi_printf(\"%s %s(%d)\\n\", text(vpi_get_str(vpiType, h)), text(name),\n\
             (int)vpi_chk_error(NULL));"
        ),
        ["vpi_get_str"]
    );
    assert_eq!(
        names("int level = vpi_get(vpiSize, h) + vpi_chk_error(NULL);"),
        ["vpi_get"]
    );
    assert_eq!(
        names("if (f(vpi_handle(vpiParent, h), vpi_chk_error(0))) {}"),
        ["vpi_handle"]
    );
    // Sequenced: the status is read first, the call encloses the read, or a
    // logical operator or a statement boundary orders the calls.
    for text in [
        "int level = vpi_chk_error(NULL);\n\
         vpi_printf(\"%s %d\", vpi_get_str(vpiType, h), level);",
        "vpi_printf(\"level=%d\\n\", (int)vpi_chk_error(NULL));",
        "if (vpi_put_value(h, &v, NULL, vpiNoDelay) || vpi_chk_error(NULL)) fail(\"put\");",
        "ok = vpi_get(vpiType, e) == vpiUndefined && vpi_chk_error(NULL) == vpiError;",
        "x = vpi_handle_by_index(a, 3) == NULL && vpi_chk_error(NULL) == vpiError ? 1 : 0;",
        "// vpi_printf(\"%d\", vpi_get(vpiSize, h), vpi_chk_error(NULL));\n",
        "vpi_printf(\"vpi_get(vpiSize, h), vpi_chk_error(NULL)\");",
        "for (h = vpi_scan(it); h; h = vpi_scan(it)) { n += !vpi_chk_error(NULL); }",
        "if (vpi_chk_error(NULL)) vpi_printf(\"failed\\n\");",
        "while (vpi_chk_error(NULL) == 0 && (h = vpi_scan(it))) vpi_release_handle(h);",
    ] {
        assert!(names(text).is_empty(), "{text}");
    }
}

/// Every checked-in C source under `tests/` that reads the VPI error status:
/// no `vpi_chk_error` call may be evaluated in an unspecified order with
/// another VPI routine call.
#[test]
fn vpi_applications_read_the_error_status_before_the_next_vpi_call() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut pending = vec![root.join("tests")];
    let mut findings = Vec::new();
    let mut scanned = 0usize;
    while let Some(directory) = pending.pop() {
        let entries = std::fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()));
        for entry in entries {
            let path = entry.expect("read directory entry").path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("c") {
                continue;
            }
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            if !text.contains(CHECK) {
                continue;
            }
            scanned += 1;
            for finding in unsequenced_status_reads(&text) {
                let line = text[..finding.offset].matches('\n').count() + 1;
                findings.push(format!(
                    "{}:{line}: vpi_chk_error unsequenced with {}",
                    path.strip_prefix(root).unwrap_or(&path).display(),
                    finding.other
                ));
            }
        }
    }
    assert!(scanned > 0, "no VPI application sources found");
    findings.sort();
    assert!(
        findings.is_empty(),
        "store vpi_chk_error(NULL) in a variable before the next VPI call \
         (argument order is unspecified in C; AArch64 and Clang evaluate left to right):\n{}",
        findings.join("\n")
    );
}
