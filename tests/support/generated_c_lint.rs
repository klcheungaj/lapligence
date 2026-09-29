//! Structural checks for explicit coroutine storage in generated C.
#![allow(dead_code)]

use std::collections::{BTreeSet, HashMap};

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

/// Reject `$` anywhere outside string/character literals and comments.
/// Standard C11 identifiers are `[A-Za-z_][A-Za-z0-9_]*`; `$` is only a
/// compiler extension, so generated identifiers must never contain it.
pub(crate) fn lint_standard_identifiers(source: &str) -> Result<(), Vec<String>> {
    let bytes = source.as_bytes();
    let mut errors = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            quote @ (b'"' | b'\'') => {
                index += 1;
                while index < bytes.len() && bytes[index] != quote {
                    index += if bytes[index] == b'\\' { 2 } else { 1 };
                }
                index += 1;
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                index = source[index + 2..]
                    .find("*/")
                    .map_or(bytes.len(), |offset| index + 2 + offset + 2);
            }
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                index = source[index..]
                    .find('\n')
                    .map_or(bytes.len(), |offset| index + offset);
            }
            b'$' => {
                let line = source[..index].matches('\n').count() + 1;
                let start = source[..index].rfind('\n').map_or(0, |offset| offset + 1);
                let end = source[index..]
                    .find('\n')
                    .map_or(source.len(), |offset| index + offset);
                errors.push(format!(
                    "line {line}: `$` outside a literal is not a standard C identifier character: {}",
                    source[start..end].trim()
                ));
                index += 1;
            }
            _ => index += 1,
        }
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
    const PREFIX: &str = "static llg_co_status_t ";
    while let Some(relative) = source[search..].find(PREFIX) {
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
        if signature.contains("llg_co_frame_t* co, llg_co_chain_t* ch")
            && body.contains("_frame_t* F = (")
        {
            let name_start = start + PREFIX.len();
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
    let facts = body_facts(body);
    let bytes = body.as_bytes();
    let mut index = 0;
    let mut line = 1usize;
    let mut quoted = None;
    let mut escaped = false;
    let mut block_comment = false;
    let mut brace_kinds = Vec::new();
    let mut active_blocks = vec![0usize];
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
                    if !address_of_narrowed_local(body, index, index + 1, &facts) {
                        errors.push(format!("{name}:{line}: {reason}"));
                    }
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

struct BodyFacts {
    resume_blocks: BTreeSet<usize>,
    first_identifiers: HashMap<String, (usize, usize)>,
}

fn body_facts(body: &str) -> BodyFacts {
    let bytes = body.as_bytes();
    let mut resume_blocks = BTreeSet::new();
    let mut first_identifiers = HashMap::new();
    let mut brace_kinds = Vec::new();
    let mut active_blocks = vec![0usize];
    let mut next_block = 1usize;
    let mut segment_start = 0usize;
    let mut paren_depth = 0usize;
    let mut index = 0usize;
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
        if is_resume_macro(&body[index..]) {
            resume_blocks.extend(active_blocks.iter().copied());
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
                if brace_kinds.pop() == Some(true) {
                    active_blocks.pop();
                }
                segment_start = index + 1;
                index += 1;
            }
            byte if byte == b'_' || byte.is_ascii_alphabetic() => {
                let start = index;
                index += 1;
                while index < bytes.len()
                    && (bytes[index] == b'_' || bytes[index].is_ascii_alphanumeric())
                {
                    index += 1;
                }
                first_identifiers
                    .entry(body[start..index].to_owned())
                    .or_insert((start, *active_blocks.last().unwrap_or(&0)));
            }
            _ => index += 1,
        }
    }
    BodyFacts {
        resume_blocks,
        first_identifiers,
    }
}

fn is_resume_macro(rest: &str) -> bool {
    [
        "LLG_CO_AWAIT(",
        "LLG_CO_SUSPEND(",
        "LLG_CO_CALL(",
        "LLG_CO_CALL_ANCHOR(",
        "LLG_CO_CALL_ARENA(",
    ]
    .into_iter()
    .any(|prefix| rest.starts_with(prefix))
}

fn address_of_narrowed_local(
    body: &str,
    address: usize,
    operand_start: usize,
    facts: &BodyFacts,
) -> bool {
    let operand = body[operand_start..].trim_start();
    let operand = operand
        .trim_start_matches(|ch: char| ch.is_ascii_whitespace() || matches!(ch, '(' | '*' | '&'));
    let ident_len = operand
        .bytes()
        .take_while(|byte| *byte == b'_' || byte.is_ascii_alphanumeric())
        .count();
    if ident_len == 0 {
        return false;
    }
    let ident = &operand[..ident_len];
    let Some(&(declaration, block)) = facts.first_identifiers.get(ident) else {
        return false;
    };
    declaration < address
        && looks_like_local_declaration(body, declaration)
        && !facts.resume_blocks.contains(&block)
}

fn looks_like_local_declaration(body: &str, identifier: usize) -> bool {
    body[..identifier]
        .bytes()
        .rev()
        .find(|byte| !byte.is_ascii_whitespace())
        .is_some_and(|byte| byte == b'*' || byte == b'_' || byte.is_ascii_alphanumeric())
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
/// `_llg_array_cell_<array>_<element>` and `_ls<instance>_<node>` are generated
/// global lvalue selectors. `_llg_inertial_<site>` and `_llg_ret_<index>` are
/// function-static state; `_llg_inertial_index` is a macro-internal temporary
/// contained by one `do { ... } while (0)` expansion. Numeric components must
/// match the generator exactly; lookalike prefixes are not accepted.
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
    if ["G_", "S_", "O_", "D_", "E_", "g_", "llg_", "fn_", "p_"]
        .into_iter()
        .any(|prefix| has_generated_payload(ident, prefix))
        || numbered_identifier(ident, "_llg_array_cell_", 2)
        || numbered_identifier(ident, "_llg_inertial_", 1)
        || ident == "_llg_inertial_index"
        || numbered_identifier(ident, "_llg_ret_", 1)
        || numbered_identifier(ident, "_ls", 2)
    {
        return Ok(());
    }
    Err(format!(
        "address operand `{}` is not explicit coroutine-frame storage",
        rest.lines().next().unwrap_or(rest).trim()
    ))
}

fn has_generated_payload(ident: &str, prefix: &str) -> bool {
    ident
        .strip_prefix(prefix)
        .is_some_and(|payload| !payload.is_empty())
}

fn numbered_identifier(ident: &str, prefix: &str, components: usize) -> bool {
    let Some(suffix) = ident.strip_prefix(prefix) else {
        return false;
    };
    let mut parts = suffix.split('_');
    (0..components).all(|_| {
        parts
            .next()
            .is_some_and(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
    }) && parts.next().is_none()
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
static llg_co_status_t fn_ok(llg_co_frame_t* co, llg_co_chain_t* ch) {
    fn_ok_frame_t* F = (fn_ok_frame_t*)co;
    {
        F->u0.b1.value = 1;
        runtime(&F->u0.b1.value, &(item_t){ 0 }, &G_signal, &fn_ok_desc,
                &_ls2_26, &_llg_array_cell_1_2, &_llg_inertial_index,
                &_llg_inertial_5, &_llg_ret_4);
    }
}
"#;
        assert_eq!(lint_generated_coroutine_c(c), Ok(()));
    }

    #[test]
    fn rejects_lookalikes_of_generated_static_names() {
        let c = r#"
static llg_co_status_t fn_bad(llg_co_frame_t* co, llg_co_chain_t* ch) {
    fn_bad_frame_t* F = (fn_bad_frame_t*)co;
    runtime(&_lslocal, &_ls2_bad, &_llg_array_cell_left_0,
            &_llg_inertial_value, &_llg_ret_state, &G_, &llg_);
}
"#;
        let errors = lint_generated_coroutine_c(c).unwrap_err();
        assert_eq!(errors.len(), 7, "{errors:?}");
    }

    #[test]
    fn accepts_address_of_local_in_resume_free_scope() {
        let c = r#"
static llg_co_status_t fn_ok(llg_co_frame_t* co, llg_co_chain_t* ch) {
    fn_ok_frame_t* F = (fn_ok_frame_t*)co;
    int* local = 0;
    runtime(&(*local));
}
"#;
        assert_eq!(lint_generated_coroutine_c(c), Ok(()));
    }

    #[test]
    fn rejects_address_of_c_local_in_scope_with_resume() {
        let c = r#"
static llg_co_status_t fn_bad(llg_co_frame_t* co, llg_co_chain_t* ch) {
    fn_bad_frame_t* F = (fn_bad_frame_t*)co;
    int local = 0;
    runtime(&local);
    LLG_CO_AWAIT(co, ch, 1, arm());
}
"#;
        let errors = lint_generated_coroutine_c(c).unwrap_err();
        assert!(errors[0].contains("address operand `local"), "{errors:?}");
    }

    #[test]
    fn rejects_access_after_overlay_block_closes() {
        let c = r#"
static llg_co_status_t fn_bad(llg_co_frame_t* co, llg_co_chain_t* ch) {
    fn_bad_frame_t* F = (fn_bad_frame_t*)co;
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

    #[test]
    fn rejects_dollar_in_generated_identifiers() {
        let c = "sv4_t G_tb_pca$0_en = SV4_EMPTY;\nsv4_t G_ok = SV4_EMPTY;\n";
        let errors = lint_standard_identifiers(c).unwrap_err();
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(errors[0].starts_with("line 1:"), "{errors:?}");
    }

    #[test]
    fn allows_dollar_in_literals_and_comments() {
        let c = "/* $display */ // $finish\nconst char* s = \"$bits \\\" $x\"; int c = '$';\n";
        assert_eq!(lint_standard_identifiers(c), Ok(()));
    }
}
