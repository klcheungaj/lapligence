/// Strip an optional frontend library prefix from a top-instance name.
pub(crate) fn strip_lib(name: &str) -> String {
    match name.split_once('@') {
        Some((_, rest)) if !rest.is_empty() => rest.to_string(),
        _ => name.to_string(),
    }
}

/// Sanitize a string for use as a C identifier fragment.
pub(crate) fn ident(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect()
}

pub(crate) fn global_name(path: &str, name: &str) -> String {
    format!("G_{}_{}", ident(path), ident(name))
}

pub(crate) fn real_global_name(path: &str, name: &str) -> String {
    format!("D_{}_{}", ident(path), ident(name))
}

pub(crate) fn event_global_name(path: &str, name: &str) -> String {
    format!("E_{}_{}", ident(path), ident(name))
}

pub(crate) fn escaped_char(character: char) -> String {
    match character {
        '"' => "\\\"".to_string(),
        '\\' => "\\\\".to_string(),
        '\n' => "\\n".to_string(),
        '\t' => "\\t".to_string(),
        '\r' => "\\r".to_string(),
        character if character.is_ascii_control() => format!("\\{:03o}", character as u32),
        _ => character.to_string(),
    }
}
