/// Strip an optional frontend library prefix from a top-instance name.
pub(crate) fn strip_lib(name: &str) -> String {
    match name.split_once('@') {
        Some((_, rest)) if !rest.is_empty() => rest.to_string(),
        _ => name.to_string(),
    }
}

/// Encode a source name as a collision-free C identifier fragment.
///
/// Ordinary HDL identifiers retain their readable spelling.  Names containing
/// punctuation (escaped identifiers, generate labels, and library-qualified
/// paths) use a reserved, byte-wise hexadecimal form.  Simple source names
/// beginning with that reserved prefix are encoded too, keeping the two forms
/// disjoint instead of merely relying on punctuation replacement.
pub(crate) fn ident(value: &str) -> String {
    const RESERVED: &str = "__llg_ident_";
    let is_simple = |value: &str| {
        !value.is_empty()
            && !value.starts_with(RESERVED)
            && value.bytes().enumerate().all(|(index, byte)| {
                (byte.is_ascii_alphanumeric() && (index != 0 || !byte.is_ascii_digit()))
                    || byte == b'_'
            })
    };
    let simple = is_simple(value);
    if simple {
        return value.to_owned();
    }

    let tag = if value.bytes().enumerate().all(|(index, byte)| {
        (byte.is_ascii_alphanumeric() && (index != 0 || !byte.is_ascii_digit())) || byte == b'_'
    }) {
        's'
    } else {
        'e'
    };
    let mut encoded = format!("{RESERVED}{tag}_");
    for byte in value.as_bytes() {
        encoded.push_str(&format!("{byte:02X}"));
    }
    encoded
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

#[cfg(test)]
mod tests {
    use super::ident;

    #[test]
    fn identifier_encoding_keeps_colliding_hdl_names_distinct() {
        let plain = ident("a_b");
        let hyphen = ident("a-b");
        let dot = ident("a.b");
        let dollar = ident("a$b");
        assert_eq!(plain, "a_b");
        assert_ne!(plain, hyphen);
        assert_ne!(plain, dot);
        assert_ne!(plain, dollar);
        assert_ne!(hyphen, dot);
        assert_ne!(hyphen, dollar);
        assert_ne!(dot, dollar);
    }

    #[test]
    fn identifier_encoding_is_safe_for_reserved_prefix_names() {
        let encoded = ident("a-b");
        let reserved = ident("__llg_ident_e_61_2D");
        assert!(encoded
            .bytes()
            .all(|byte| { byte.is_ascii_alphanumeric() || byte == b'_' }));
        assert_ne!(encoded, reserved);
    }
}
