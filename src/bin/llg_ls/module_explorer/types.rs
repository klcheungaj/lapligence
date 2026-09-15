//! Types.

use super::*;

pub(super) fn module_id(root_id: &str, module: &ModuleDef) -> String {
    format!(
        "module:{root_id}:{}:{}:{}:{}:{}:{}",
        clean_name(&module.name),
        module.line,
        module.col,
        module.end_line,
        module.end_col,
        source_identity(module.file.as_deref()),
    )
}

pub(super) fn instance_id(root_id: &str, instance: &InstanceModel) -> String {
    let hierarchy = clean_name(&instance.full_name);
    let hierarchy = if hierarchy.is_empty() {
        instance.name.as_str()
    } else {
        hierarchy
    };
    format!("instance:{root_id}:{hierarchy}")
}

pub(super) fn generate_scope_id(root_id: &str, parent_id: &str, scope: &GenScopeModel) -> String {
    let hierarchy = clean_name(&scope.full_name);
    if hierarchy.is_empty() {
        format!("{parent_id}:generate:{}", scope.name)
    } else {
        format!("generate:{root_id}:{hierarchy}")
    }
}

pub(super) fn find_definition_id(definitions: &[(String, String)], name: &str) -> Option<String> {
    let name = clean_name(name);
    definitions
        .iter()
        .find(|(candidate, _)| candidate == name)
        .map(|(_, id)| id.clone())
}

pub(super) fn same_name(left: &str, right: &str) -> bool {
    clean_name(left) == clean_name(right)
}

pub(super) fn clean_name(name: &str) -> &str {
    name.split_once('@').map(|(_, rest)| rest).unwrap_or(name)
}

fn source_identity(file: Option<&str>) -> String {
    file.unwrap_or("<unknown>").to_owned()
}

pub(super) fn source_uri(file: Option<&str>) -> Option<String> {
    let file = file?;
    Url::from_file_path(file)
        .ok()
        .map(|uri| uri.to_string())
        .or_else(|| Some(file.to_owned()))
}

pub(super) fn source_range(line: u32, col: u32, end_line: u32, end_col: u32) -> Option<ExplorerRange> {
    if line == 0 || col == 0 {
        return None;
    }
    Some(ExplorerRange {
        start_line: line.saturating_sub(1),
        start_character: col.saturating_sub(1),
        end_line: end_line.max(line).saturating_sub(1),
        end_character: end_col.max(col).saturating_sub(1),
    })
}

pub(super) fn instance_range(instance: &InstanceModel) -> Option<ExplorerRange> {
    let displayed_type = if instance.def_name.is_empty() {
        instance.name.as_str()
    } else {
        clean_name(&instance.def_name)
    };
    let end_col = instance
        .col
        .saturating_add(displayed_type.chars().count() as u32);
    source_range(instance.line, instance.col, instance.line, end_col)
}

fn explorer_type(ty: &TypeInfo) -> ExplorerType {
    explorer_type_with_context(ty, None, None, None)
}

pub(super) fn explorer_type_with_context(
    ty: &TypeInfo,
    symbolic_display: Option<&str>,
    display_shape: Option<ModuleGraphTypeShape>,
    concrete_ranges: Option<&[Option<ModuleGraphPackedRange>]>,
) -> ExplorerType {
    ExplorerType {
        kind: ty.kind.clone(),
        width: ty.width,
        signed: ty.signed,
        type_name: ty.type_name.clone(),
        display_type: resolved_type_display(ty, symbolic_display, display_shape, concrete_ranges),
    }
}

pub(super) fn direction(direction: Direction) -> String {
    match direction {
        Direction::Input => "input",
        Direction::Output => "output",
        Direction::Inout => "inout",
        Direction::None => "none",
    }
    .to_owned()
}

pub(super) fn port(port: &PortModel) -> ExplorerPort {
    ExplorerPort {
        name: port.name.clone(),
        direction: direction(port.direction),
        ty: explorer_type(&port.ty),
        detail: None,
        location: None,
    }
}

pub(super) fn signal(signal: &SignalModel) -> ExplorerSignal {
    ExplorerSignal {
        name: signal.name.clone(),
        kind: signal.kind.clone(),
        ty: explorer_type(&signal.ty),
        detail: None,
        location: None,
    }
}

pub(super) fn parameter(parameter: &ParamModel) -> ExplorerParameter {
    ExplorerParameter {
        name: parameter.name.clone(),
        ty: explorer_type(&parameter.ty),
        value: parameter.value.as_ref().map(Val::format_verilog),
        local: parameter.local,
        detail: None,
        location: None,
    }
}

pub(super) fn port_with_source(
    port: &PortModel,
    source: Option<&crate::features::ModuleGraphPort>,
    concrete_ranges: Option<&[Option<ModuleGraphPackedRange>]>,
) -> ExplorerPort {
    let ty = if port.ty.kind == "other" {
        source.map_or(&port.ty, |source| &source.ty)
    } else {
        &port.ty
    };
    ExplorerPort {
        name: port.name.clone(),
        direction: direction(port.direction),
        ty: explorer_type_with_context(
            ty,
            source.and_then(|source| source.display_type.as_deref()),
            Some(source.map_or(ModuleGraphTypeShape::default(), |source| {
                source.display_shape
            })),
            concrete_ranges,
        ),
        // Elaborated contents deliberately keep their source text in
        // `displayType`; the legacy detail field stays declaration-fallback
        // text so older clients can distinguish the two sources.
        detail: None,
        location: source
            .and_then(|source| source.location.as_ref())
            .and_then(explorer_location),
    }
}

pub(super) fn signal_with_source(
    signal: &SignalModel,
    source: Option<&crate::features::ModuleGraphSignal>,
    concrete_ranges: Option<&[Option<ModuleGraphPackedRange>]>,
) -> ExplorerSignal {
    let kind = if signal.kind == "net" {
        source
            .map(|source| source.kind.as_str())
            .unwrap_or(signal.kind.as_str())
    } else {
        signal.kind.as_str()
    };
    let ty = if signal.ty.kind == "other" {
        source.map_or(&signal.ty, |source| &source.ty)
    } else {
        &signal.ty
    };
    ExplorerSignal {
        name: signal.name.clone(),
        kind: kind.to_owned(),
        ty: explorer_type_with_context(
            ty,
            source.and_then(|source| source.display_type.as_deref()),
            Some(source.map_or(ModuleGraphTypeShape::default(), |source| {
                source.display_shape
            })),
            concrete_ranges,
        ),
        detail: None,
        location: source
            .and_then(|source| source.location.as_ref())
            .and_then(explorer_location),
    }
}

pub(super) fn parameter_with_source(
    parameter: &ParamModel,
    source: Option<&crate::features::ModuleGraphParameter>,
    concrete_ranges: Option<&[Option<ModuleGraphPackedRange>]>,
) -> ExplorerParameter {
    let ty = if parameter.ty.kind == "other" {
        source.map_or(&parameter.ty, |source| &source.ty)
    } else {
        &parameter.ty
    };
    ExplorerParameter {
        name: parameter.name.clone(),
        ty: explorer_type_with_context(
            ty,
            source.and_then(|source| source.display_type.as_deref()),
            Some(source.map_or(ModuleGraphTypeShape::default(), |source| {
                source.display_shape
            })),
            concrete_ranges,
        ),
        value: parameter.value.as_ref().map(Val::format_verilog),
        local: parameter.local,
        detail: None,
        location: source
            .and_then(|source| source.location.as_ref())
            .and_then(explorer_location),
    }
}

pub(super) fn explorer_location(location: &ModuleGraphLocation) -> Option<ExplorerLocation> {
    Some(ExplorerLocation {
        uri: source_uri(Some(location.file.as_str()))?,
        range: source_range(
            location.line,
            location.col,
            location.end_line,
            location.end_col,
        )?,
    })
}

pub(super) fn resolved_type_display(
    ty: &TypeInfo,
    symbolic_display: Option<&str>,
    display_shape: Option<ModuleGraphTypeShape>,
    concrete_ranges: Option<&[Option<ModuleGraphPackedRange>]>,
) -> Option<String> {
    let fallback = symbolic_display.map(normalize_type_display);
    if let Some(source) = fallback {
        if source.contains('[') {
            return Some(render_concrete_type_display(
                &source,
                display_shape,
                concrete_ranges,
            ));
        }
        // A scalar source spelling is authoritative for named typedefs and
        // source qualifiers such as `wire`.  Do not replace it with the
        // underlying elaborated TypeInfo (`logic`, for example).
        return Some(source);
    }
    (ty.kind != "other").then(|| ty.render())
}

/// Render canonical captured packed ranges while retaining source unpacked
/// dimensions. The source string is only a normalized spelling fallback; no
/// request-time expression evaluation is performed here.
fn render_concrete_type_display(
    source: &str,
    display_shape: Option<ModuleGraphTypeShape>,
    concrete_ranges: Option<&[Option<ModuleGraphPackedRange>]>,
) -> String {
    let spans = bracket_spans(source);
    if spans.is_empty() {
        return source.to_owned();
    }

    let (packed_count, unpacked_count) = display_shape
        .filter(|shape| {
            shape
                .packed_dimensions
                .saturating_add(shape.unpacked_dimensions)
                > 0
        })
        .map_or_else(
            || {
                // Before shape metadata was added, graph entries only had a
                // flat list of packed dimensions. Treat all source brackets
                // as packed for that legacy representation.
                (spans.len(), 0)
            },
            |shape| {
                let packed = shape.packed_dimensions.min(spans.len());
                let remaining = spans.len().saturating_sub(packed);
                (packed, shape.unpacked_dimensions.min(remaining))
            },
        );
    let source_base = remove_bracket_dimensions(source).trim().to_owned();
    let mut display = source_base;

    for (index, (start, end, expression)) in spans.iter().take(packed_count).enumerate() {
        let concrete = concrete_ranges
            .and_then(|ranges| ranges.get(index))
            .and_then(|range| range.as_ref())
            .map(|range| format!("[{}:{}]", range.left, range.right));
        let dimension =
            concrete.unwrap_or_else(|| format!("[{}]", normalize_symbolic_expression(expression)));
        // `start`/`end` are intentionally used only to document that the
        // source span is retained per dimension; the normalized expression is
        // already owned by `bracket_spans` and is UTF-8 safe.
        let _ = (start, end);
        append_display_part(&mut display, &dimension);
    }

    let first_unpacked = packed_count;
    let last_unpacked = first_unpacked
        .saturating_add(unpacked_count)
        .min(spans.len());
    for (start, end, _) in &spans[first_unpacked..last_unpacked] {
        if let Some(dimension) = source.get(*start..*end) {
            append_display_part(&mut display, &normalize_type_display(dimension));
        }
    }
    display
}

fn append_display_part(display: &mut String, part: &str) {
    if !display.is_empty() {
        display.push(' ');
    }
    display.push_str(part);
}

pub(super) fn normalize_type_display(text: &str) -> String {
    if contains_comment(text) {
        return text.trim().to_owned();
    }

    let spans = bracket_spans(text);
    if spans.is_empty() {
        return text.split_whitespace().collect::<Vec<_>>().join(" ");
    }

    let mut normalized = String::new();
    let mut cursor = 0;
    for (start, end, expression) in spans {
        let prefix = text[cursor..start]
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if !prefix.is_empty() {
            append_display_part(&mut normalized, &prefix);
        }
        append_display_part(
            &mut normalized,
            &format!("[{}]", normalize_symbolic_expression(&expression)),
        );
        cursor = end;
    }
    let suffix = text[cursor..]
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if !suffix.is_empty() {
        append_display_part(&mut normalized, &suffix);
    }
    normalized
}

pub(super) fn normalize_symbolic_expression(expression: &str) -> String {
    if contains_comment(expression) {
        return expression.trim().to_owned();
    }

    let chars = expression.chars().collect::<Vec<_>>();
    let mut normalized = String::new();
    let mut pending_space = false;
    let mut pending_after_escaped_identifier = false;
    let mut escaped_identifier = false;
    let mut in_string = false;
    let mut escaped_string_character = false;

    for (index, character) in chars.iter().copied().enumerate() {
        if in_string {
            normalized.push(character);
            if escaped_string_character {
                escaped_string_character = false;
            } else if character == '\\' {
                escaped_string_character = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }

        // SystemVerilog escaped identifiers start with `\\` and end at the
        // first whitespace. Keep that terminating separator even when the
        // following token is punctuation.
        if escaped_identifier {
            if character.is_whitespace() {
                pending_space = true;
                pending_after_escaped_identifier = true;
                escaped_identifier = false;
            } else {
                normalized.push(character);
            }
            continue;
        }

        if character.is_whitespace() {
            pending_space = true;
            continue;
        }

        if pending_space {
            if should_retain_symbolic_separator(
                &normalized,
                &chars,
                index,
                pending_after_escaped_identifier,
            ) && !normalized.ends_with(' ')
            {
                normalized.push(' ');
            }
            pending_space = false;
            pending_after_escaped_identifier = false;
        }

        normalized.push(character);
        if character == '\\' {
            escaped_identifier = true;
        } else if character == '"' {
            in_string = true;
            escaped_string_character = false;
        }
    }

    if pending_space && pending_after_escaped_identifier && !normalized.ends_with(' ') {
        normalized.push(' ');
    }
    normalized
}

fn is_word_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_' || character == '$'
}

fn should_retain_symbolic_separator(
    normalized: &str,
    chars: &[char],
    next_index: usize,
    after_escaped_identifier: bool,
) -> bool {
    if after_escaped_identifier {
        return true;
    }

    let Some(previous) = normalized.chars().last() else {
        return false;
    };
    let Some(next) = chars.get(next_index).copied() else {
        return false;
    };

    if is_word_character(previous) && is_word_character(next) {
        return true;
    }

    let previous_word = normalized
        .rsplit(|character: char| !is_word_character(character))
        .next()
        .filter(|word| !word.is_empty());
    let next_word = next_symbolic_word(chars, next_index);
    if previous_word.is_some_and(is_dimension_keyword)
        || next_word.as_deref().is_some_and(is_dimension_keyword)
    {
        return true;
    }

    // A backslash starts a new escaped identifier. Keep a separator before
    // it as well; otherwise a preceding identifier could join the escaped
    // token.
    if next == '\\' {
        return true;
    }

    // Do not merge two operators into a different token (`+ +` → `++`,
    // `/ *` → `/*`, `: :` → `::`, and so on).
    if operator_pair_requires_separator(previous, next) {
        return true;
    }

    // Compact the known punctuation alphabet, while conservatively retaining
    // a separator around an unfamiliar character.
    !is_known_symbolic_character(previous) || !is_known_symbolic_character(next)
}

fn next_symbolic_word(chars: &[char], start: usize) -> Option<String> {
    let mut index = start;
    while chars
        .get(index)
        .is_some_and(|character| character.is_whitespace())
    {
        index += 1;
    }
    let word_start = index;
    while chars
        .get(index)
        .is_some_and(|character| is_word_character(*character))
    {
        index += 1;
    }
    (index > word_start).then(|| chars[word_start..index].iter().collect())
}

fn is_dimension_keyword(word: &str) -> bool {
    word.eq_ignore_ascii_case("inside")
}

fn is_known_symbolic_character(character: char) -> bool {
    is_word_character(character)
        || matches!(
            character,
            '"' | '\''
                | '['
                | ']'
                | '{'
                | '}'
                | '('
                | ')'
                | ','
                | ':'
                | ';'
                | '.'
                | '?'
                | '+'
                | '-'
                | '*'
                | '/'
                | '%'
                | '&'
                | '|'
                | '^'
                | '~'
                | '!'
                | '='
                | '<'
                | '>'
        )
}

fn operator_pair_requires_separator(previous: char, next: char) -> bool {
    matches!(
        (previous, next),
        ('+', '+' | '=')
            | ('-', '-' | '=' | '>' | ':')
            | ('*', '*' | '=' | '/' | '>')
            | ('/', '/' | '*' | '=')
            | ('%', '=')
            | ('&', '&' | '=')
            | ('|', '|' | '=' | '-' | '>')
            | ('^', '^' | '=' | '~')
            | ('~', '^' | '=')
            | ('!', '!' | '=')
            | ('=', '=' | '<' | '>')
            | ('<', '<' | '=' | '>')
            | ('>', '>' | '=')
            | ('?', '?')
            | (':', ':' | '=' | '+' | '-')
            | ('.', '.' | '*')
    )
}

fn contains_comment(text: &str) -> bool {
    let mut escaped_identifier = false;
    let mut in_string = false;
    let mut escaped_string_character = false;

    for (index, character) in text.char_indices() {
        if in_string {
            if escaped_string_character {
                escaped_string_character = false;
            } else if character == '\\' {
                escaped_string_character = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }
        if escaped_identifier {
            if character.is_whitespace() {
                escaped_identifier = false;
            }
            continue;
        }
        if character == '"' {
            in_string = true;
            escaped_string_character = false;
            continue;
        }
        if character == '\\' {
            escaped_identifier = true;
            continue;
        }
        if character == '/' && text[index..].starts_with("//") {
            return true;
        }
        if character == '/' && text[index..].starts_with("/*") {
            return true;
        }
    }
    false
}

pub(super) fn bracket_spans(text: &str) -> Vec<(usize, usize, String)> {
    let mut spans = Vec::new();
    let mut start = None;
    let mut depth = 0usize;
    let mut escaped_identifier = false;
    let mut in_string = false;
    let mut escaped_string_character = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    for (index, character) in text.char_indices() {
        if in_line_comment {
            if matches!(character, '\n' | '\r') {
                in_line_comment = false;
            }
            continue;
        }
        if in_block_comment {
            if character == '*' && text[index..].starts_with("*/") {
                in_block_comment = false;
            }
            continue;
        }
        if in_string {
            if escaped_string_character {
                escaped_string_character = false;
            } else if character == '\\' {
                escaped_string_character = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }
        if escaped_identifier {
            if character.is_whitespace() {
                escaped_identifier = false;
            }
            continue;
        }
        if character == '"' {
            in_string = true;
            escaped_string_character = false;
            continue;
        }
        if character == '\\' {
            escaped_identifier = true;
            continue;
        }
        if character == '/' && text[index..].starts_with("//") {
            in_line_comment = true;
            continue;
        }
        if character == '/' && text[index..].starts_with("/*") {
            in_block_comment = true;
            continue;
        }
        match character {
            '[' if depth == 0 => {
                start = Some(index);
                depth = 1;
            }
            '[' if depth > 0 => depth += 1,
            ']' if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    if let Some(start) = start.take() {
                        let expression = &text[start + 1..index];
                        spans.push((start, index + character.len_utf8(), expression.to_owned()));
                    }
                }
            }
            _ => {}
        }
    }
    spans
}
