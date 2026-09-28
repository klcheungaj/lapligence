//! Structural checks for explicit coroutine storage in generated C.
#![allow(dead_code)]

/// Check address-taking and overlaid-block access in every coroutine function.
pub(crate) fn lint_generated_coroutine_c(source: &str) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    for (name, body) in coroutine_bodies(source) {
        lint_coroutine_body(&name, body, &mut errors);
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn coroutine_bodies(source: &str) -> Vec<(String, &str)> {
    let mut bodies = Vec::new();
    let mut search = 0;
    while let Some(relative) = source[search..].find("static void ") {
        let start = search + relative;
        let Some(open_relative) = source[start..].find('{') else {
            break;
        };
        let open = start + open_relative;
        if source[start..open].contains(';') {
            search = open + 1;
            continue;
        }
        let Some(close) = matching_brace(source, open) else {
            break;
        };
        let signature = &source[start..open];
        let body = &source[open + 1..close];
        if signature.contains("_frame_t* F") || body.contains("llg_proc_co_frame(self)") {
            let name_start = start + "static void ".len();
            let name_end = source[name_start..open]
                .find('(')
                .map(|offset| name_start + offset)
                .unwrap_or(open);
            bodies.push((source[name_start..name_end].trim().to_owned(), body));
        }
        search = close + 1;
    }
    bodies
}

fn matching_brace(source: &str, open: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut depth = 0usize;
    let mut index = open;
    let mut quoted = None;
    let mut escaped = false;
    let mut block_comment = false;
    while index < bytes.len() {
        if block_comment {
            if bytes[index..].starts_with(b"*/") {
                block_comment = false;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if let Some(quote) = quoted {
            let byte = bytes[index];
            index += 1;
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == quote {
                quoted = None;
            }
            continue;
        }
        if bytes[index..].starts_with(b"//") {
            index = source[index..]
                .find('\n')
                .map(|offset| index + offset + 1)
                .unwrap_or(bytes.len());
            continue;
        }
        if bytes[index..].starts_with(b"/*") {
            block_comment = true;
            index += 2;
            continue;
        }
        match bytes[index] {
            b'\'' | b'"' => quoted = Some(bytes[index]),
            b'{' => depth += 1,
            b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

fn lint_coroutine_body(name: &str, body: &str, errors: &mut Vec<String>) {
    let bytes = body.as_bytes();
    let mut index = 0;
    let mut line = 1usize;
    let mut quoted = None;
    let mut escaped = false;
    let mut block_comment = false;
    let mut brace_kinds = Vec::new();
    let mut active_blocks = Vec::new();
    let mut next_block = 1usize;
    let mut segment_start = 0usize;
    let mut paren_depth = 0usize;

    while index < bytes.len() {
        if bytes[index] == b'\n' {
            line += 1;
        }
        if block_comment {
            if bytes[index..].starts_with(b"*/") {
                block_comment = false;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if let Some(quote) = quoted {
            let byte = bytes[index];
            index += 1;
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == quote {
                quoted = None;
            }
            continue;
        }
        if bytes[index..].starts_with(b"//") {
            index = body[index..]
                .find('\n')
                .map(|offset| index + offset)
                .unwrap_or(bytes.len());
            continue;
        }
        if bytes[index..].starts_with(b"/*") {
            block_comment = true;
            index += 2;
            continue;
        }
        match bytes[index] {
            b'\'' | b'"' => {
                quoted = Some(bytes[index]);
                index += 1;
            }
            b'(' | b'[' => {
                paren_depth += 1;
                index += 1;
            }
            b')' | b']' => {
                paren_depth = paren_depth.saturating_sub(1);
                index += 1;
            }
            b';' if paren_depth == 0 => {
                segment_start = index + 1;
                index += 1;
            }
            b'{' => {
                let inside_initializer = brace_kinds.last() == Some(&false);
                let segment = body[segment_start..index].trim();
                // The recursion guard is assembled around `Frame::body` and
                // therefore is not part of the frame-layout block numbering.
                let external_recursion_guard = segment.starts_with("if (F->depth >= 256)");
                let structural = !inside_initializer
                    && !external_recursion_guard
                    && is_structural_block_open(segment);
                brace_kinds.push(structural);
                if structural {
                    active_blocks.push(next_block);
                    next_block += 1;
                }
                segment_start = index + 1;
                index += 1;
            }
            b'}' => {
                match brace_kinds.pop() {
                    Some(true) => {
                        active_blocks.pop();
                    }
                    Some(false) => {}
                    None => errors.push(format!("{name}:{line}: unmatched closing brace")),
                }
                segment_start = index + 1;
                index += 1;
            }
            b'F' if body[index..].starts_with("F->") => {
                let end = frame_access_end(body, index);
                let access = &body[index..end];
                for block in overlay_blocks(access) {
                    if !active_blocks.contains(&block) {
                        errors.push(format!(
                            "{name}:{line}: `{access}` references inactive overlaid block b{block}"
                        ));
                    }
                }
                index = end;
            }
            b'&' if !bytes[index..].starts_with(b"&&")
                && !bytes[index..].starts_with(b"&=")
                && is_unary_address(body, index) =>
            {
                if let Err(reason) = allowed_address_operand(body, index + 1) {
                    errors.push(format!("{name}:{line}: {reason}"));
                }
                index += 1;
            }
            _ => index += 1,
        }
    }
    if !brace_kinds.is_empty() {
        errors.push(format!(
            "{name}: generated body has {} unclosed brace(s)",
            brace_kinds.len()
        ));
    }
}

fn is_unary_address(body: &str, index: usize) -> bool {
    let previous = body[..index]
        .bytes()
        .rev()
        .find(|byte| !byte.is_ascii_whitespace());
    previous.is_none_or(|byte| {
        matches!(
            byte,
            b'(' | b'[' | b'{' | b',' | b'=' | b'!' | b':' | b';' | b'?'
        )
    })
}

fn is_structural_block_open(segment: &str) -> bool {
    if segment.is_empty() {
        return true;
    }
    let segment = segment.trim_start_matches('}').trim_start();
    ["if", "else", "for", "while", "switch", "do"]
        .into_iter()
        .any(|keyword| {
            segment == keyword
                || segment.strip_prefix(keyword).is_some_and(|rest| {
                    rest.as_bytes()
                        .first()
                        .is_some_and(|byte| matches!(byte, b' ' | b'('))
                })
        })
}

fn frame_access_end(body: &str, start: usize) -> usize {
    let bytes = body.as_bytes();
    let mut index = start + 3;
    while index < bytes.len()
        && (bytes[index] == b'_' || bytes[index] == b'.' || bytes[index].is_ascii_alphanumeric())
    {
        index += 1;
    }
    index
}

fn overlay_blocks(access: &str) -> Vec<usize> {
    access
        .split('.')
        .filter_map(|part| part.strip_prefix('b')?.parse().ok())
        .collect()
}

/// Allowed address operands are deliberately narrow:
///
/// - explicit `F->...` frame members (including a parenthesized access),
/// - generated globals/statics (`G_`, `S_`, `O_`, `D_`, `E_`, `g_`, function
///   symbols, descriptors and `llg_` catalogs),
/// - compound literals passed directly inside generated runtime calls.
///
/// `_llg_array_cell_` and `_lsN_` are macro-internal lvalue selectors whose
/// declarations and uses are contained by one `do { ... } while (0)` expansion;
/// `_llg_inertial_` and `_llg_ret_` are function-static state rather than stack
/// storage.
fn allowed_address_operand(body: &str, operand_start: usize) -> Result<(), String> {
    let rest = body[operand_start..].trim_start();
    let operand = rest
        .trim_start_matches(|ch: char| ch.is_ascii_whitespace() || matches!(ch, '(' | '*' | '&'));
    if operand.starts_with("F->") {
        return Ok(());
    }
    if rest.starts_with('(') && looks_like_compound_literal(rest) {
        return Ok(());
    }
    let ident_end = operand
        .bytes()
        .take_while(|byte| *byte == b'_' || byte.is_ascii_alphanumeric())
        .count();
    let ident = &operand[..ident_end];
    if ident.starts_with("G_")
        || ident.starts_with("S_")
        || ident.starts_with("O_")
        || ident.starts_with("D_")
        || ident.starts_with("E_")
        || ident.starts_with("g_")
        || ident.starts_with("llg_")
        || ident.starts_with("fn_")
        || ident.starts_with("p_")
        || ident.starts_with("_llg_array_cell_")
        || ident.starts_with("_llg_inertial_")
        || ident.starts_with("_llg_ret_")
        || ident.starts_with("_ls")
        || ident.ends_with("_desc")
    {
        return Ok(());
    }
    Err(format!(
        "address operand `{}` is not explicit coroutine-frame storage",
        rest.lines().next().unwrap_or(rest).trim()
    ))
}

fn looks_like_compound_literal(rest: &str) -> bool {
    let Some(close) = rest.find(')') else {
        return false;
    };
    rest[close + 1..].trim_start().starts_with('{')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_nested_frame_access_and_documented_address_forms() {
        let c = r#"
static void fn_ok(fn_ok_frame_t* F) {
    {
        F->u0.b1.value = 1;
        runtime(&F->u0.b1.value, &(item_t){ 0 }, &G_signal, &fn_ok_desc);
    }
}
"#;
        assert_eq!(lint_generated_coroutine_c(c), Ok(()));
    }

    #[test]
    fn rejects_address_of_c_local() {
        let c = r#"
static void fn_bad(fn_bad_frame_t* F) {
    int local = 0;
    runtime(&local);
}
"#;
        let errors = lint_generated_coroutine_c(c).unwrap_err();
        assert!(errors[0].contains("address operand `local"), "{errors:?}");
    }

    #[test]
    fn rejects_access_after_overlay_block_closes() {
        let c = r#"
static void fn_bad(fn_bad_frame_t* F) {
    { F->u0.b1.value = 1; }
    consume(F->u0.b1.value);
}
"#;
        let errors = lint_generated_coroutine_c(c).unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.contains("inactive overlaid block b1")));
    }

    #[test]
    fn ignores_plain_functions_and_final_processes() {
        let c = r#"
static void fn_plain(int* value) { consume(&value); }
static void p_final(llg_proc_t* self) { int local; consume(&local); }
"#;
        assert_eq!(lint_generated_coroutine_c(c), Ok(()));
    }
}
