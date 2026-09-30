/// Strip an optional frontend library prefix from a top-instance name.
pub(crate) fn strip_lib(name: &str) -> String {
    match name.split_once('@') {
        Some((_, rest)) if !rest.is_empty() => rest.to_string(),
        _ => name.to_string(),
    }
}

/// Historical spelling of scope components in display paths and diagnostics.
/// This is not a C-name builder; source components are kept separately for that.
pub(crate) fn display_ident(value: &str) -> String {
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

const ENCODED_PREFIX: &str = "cI_";

/// Maximum length of a complete internal C identifier, including derived
/// frame/descriptor suffixes. Foreign DPI symbols are outside this policy.
pub(crate) const MAX_C_IDENTIFIER_LEN: usize = 128;

fn simple(value: &str) -> bool {
    !value.is_empty()
        && ![
            ENCODED_PREFIX,
            "__llg_ident_",
            "llg_",
            "sv4_",
            "LLG_",
            "SV4_",
            "G_",
            "D_",
            "E_",
            "O_",
            "p_",
            "f_",
            "fn_",
            "g_",
            "__",
        ]
        .iter()
        .any(|prefix| value.starts_with(prefix))
        && !value
            .as_bytes()
            .get(1)
            .is_some_and(|byte| value.starts_with('_') && byte.is_ascii_uppercase())
        && value.bytes().enumerate().all(|(index, byte)| {
            byte == b'_' || (byte.is_ascii_alphanumeric() && (index != 0 || !byte.is_ascii_digit()))
        })
}

fn append_component(out: &mut String, value: &str) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for byte in value.bytes() {
        match byte {
            b'Z' => out.push_str("ZZ"),
            b'_' => out.push_str("Zu"),
            b'.' => out.push_str("Zd"),
            b'[' => out.push_str("Zl"),
            b']' => out.push_str("Zr"),
            b'a'..=b'z' | b'A'..=b'Y' | b'0'..=b'9' => out.push(char::from(byte)),
            _ => {
                out.push_str("Zx");
                out.push(char::from(HEX[usize::from(byte >> 4)]));
                out.push(char::from(HEX[usize::from(byte & 15)]));
            }
        }
    }
}

/// Encode one source name, exactly once, as an injective C fragment.
///
/// Plain identifiers keep their spelling except protected namespaces/forms.
/// Encoded names start with `cI_`, which source names cannot pass through.
/// Inside that namespace `Z` introduces prefix-free tokens: `ZZ` is a literal
/// Z, `Zu/Zd/Zl/Zr` are punctuation, and `ZxHH` is one other UTF-8 byte. Thus no
/// source spelling can impersonate an escape. Length is bounded on complete
/// model symbols by `bound_identifiers`, rather than by a collision-prone hash.
pub(crate) fn ident(value: &str) -> String {
    if simple(value) {
        return value.to_owned();
    }
    let mut out = ENCODED_PREFIX.to_owned();
    append_component(&mut out, value);
    out
}

/// Encode a source function fragment in a fixed generated namespace. Source
/// descriptor/frame spellings must not impersonate that function's derived
/// symbols, even when there is no hierarchy tuple to protect their boundary.
pub(crate) fn function_ident(value: &str) -> String {
    if !value.contains("_desc") && !value.ends_with("_frame_t") {
        return ident(value);
    }
    let mut out = ENCODED_PREFIX.to_owned();
    append_component(&mut out, value);
    out
}

/// Compose raw source components without encoding an encoded fragment again.
///
/// A multi-component path uses token `Zp`, which never occurs at a token
/// boundary inside an encoded source component: a source Z is always `ZZ`.
/// Scanning the prefix-free tokens (rather than splitting substrings such as
/// the `Zp` inside `ZZp`) therefore recovers the exact component sequence,
/// including empty components and dots inside escaped identifiers. Single
/// components retain `ident`'s readable form; multi-component paths always
/// occupy the encoded namespace, so `a.b` cannot collide with `a_b`.
pub(crate) fn path_ident(components: &[&str]) -> String {
    if let [component] = components {
        return ident(component);
    }
    encoded_components(components)
}

fn encoded_components(components: &[&str]) -> String {
    if components.is_empty() {
        return format!("{ENCODED_PREFIX}Ze");
    }
    let mut out = ENCODED_PREFIX.to_owned();
    for (index, component) in components.iter().enumerate() {
        if index != 0 {
            out.push_str("Zp");
        }
        append_component(&mut out, component);
    }
    out
}

/// Name storage/functions from source components in a generated namespace.
/// The readable two-component form has exactly one boundary: the first
/// component has no underscore. Reserve the encoded namespace and derived
/// descriptor/frame/array suffixes too. Encoded payloads escape underscores,
/// so appending a generated underscore suffix cannot impersonate a source
/// component. All other tuples use the reversible path encoding.
pub(crate) fn scoped_name(prefix: &str, components: &[&str]) -> String {
    let suffix = match components {
        [scope, name]
            if simple(scope)
                && !scope.contains('_')
                && *scope != ENCODED_PREFIX.trim_end_matches('_')
                && simple(name)
                && !name.contains("__")
                && !name.contains("_desc")
                && !name.ends_with("_frame_t") =>
        {
            format!("{scope}_{name}")
        }
        _ => encoded_components(components),
    };
    format!("{prefix}_{suffix}")
}

pub(crate) fn global_name(path: &str, name: &str) -> String {
    scoped_name("G", &[path, name])
}

pub(crate) fn real_global_name(path: &str, name: &str) -> String {
    scoped_name("D", &[path, name])
}

pub(crate) fn event_global_name(path: &str, name: &str) -> String {
    scoped_name("E", &[path, name])
}

/// Identifier spans in emitter-produced C, excluding literals and comments.
pub(super) fn identifier_spans(text: &str) -> impl Iterator<Item = std::ops::Range<usize>> + '_ {
    let bytes = text.as_bytes();
    let mut index = 0;
    std::iter::from_fn(move || {
        while index < bytes.len() {
            let byte = bytes[index];
            if matches!(byte, b'"' | b'\'') {
                index += 1;
                while index < bytes.len() && bytes[index] != byte {
                    index += if bytes[index] == b'\\' { 2 } else { 1 };
                }
                index = (index + 1).min(bytes.len());
            } else if byte == b'/' && bytes.get(index + 1) == Some(&b'*') {
                index = text[index + 2..]
                    .find("*/")
                    .map_or(bytes.len(), |offset| index + 2 + offset + 2);
            } else if byte == b'/' && bytes.get(index + 1) == Some(&b'/') {
                index = text[index..]
                    .find('\n')
                    .map_or(bytes.len(), |offset| index + offset);
            } else if byte == b'_' || byte.is_ascii_alphanumeric() {
                let start = index;
                while index < bytes.len()
                    && (bytes[index] == b'_' || bytes[index].is_ascii_alphanumeric())
                {
                    index += 1;
                }
                if !byte.is_ascii_digit() {
                    return Some(start..index);
                }
            } else {
                index += 1;
            }
        }
        None
    })
}

/// Rewrite identifiers in emitted C without touching user-visible text.
pub(super) fn rewrite_identifiers(
    text: &str,
    mut rename: impl FnMut(&str) -> Option<String>,
) -> String {
    let mut out = String::with_capacity(text.len());
    let mut copied = 0;
    for span in identifier_spans(text) {
        if let Some(replacement) = rename(&text[span.clone()]) {
            out.push_str(&text[copied..span.start]);
            out.push_str(&replacement);
            copied = span.end;
        }
    }
    out.push_str(&text[copied..]);
    out
}

/// Bound complete internal symbols using a deterministic per-model registry.
///
/// A finite-length stateless hash cannot be injective on arbitrary source
/// names. Instead sort the oversized identifiers, assign distinct indices,
/// and skip every candidate already present anywhere in this model (including
/// external names). This makes the finite model's substitution injective and
/// reproducible. Run after all derived names exist; retain namespace prefixes
/// and frame/descriptor suffixes needed by generated-C validation. Foreign
/// C names are explicit ABI contracts and must remain byte-for-byte exact.
pub(super) fn bound_identifiers(
    source: String,
    external: &std::collections::BTreeSet<&str>,
) -> String {
    if !identifier_spans(&source)
        .any(|span| span.len() > MAX_C_IDENTIFIER_LEN && !external.contains(&source[span]))
    {
        return source;
    }
    let occupied = identifier_spans(&source)
        .map(|span| &source[span])
        .collect::<std::collections::BTreeSet<_>>();
    let mut renamed = std::collections::BTreeMap::new();
    let mut sequence = 0usize;
    for name in &occupied {
        if name.len() <= MAX_C_IDENTIFIER_LEN || external.contains(name) {
            continue;
        }
        let prefix = [
            "fn_", "G_", "D_", "E_", "O_", "p_", "f_", "g_", "llg_", "_llg_",
        ]
        .into_iter()
        .find(|prefix| name.starts_with(prefix))
        .unwrap_or("");
        let suffix = ["_frame_t", "_desc"]
            .into_iter()
            .find(|suffix| name.ends_with(suffix))
            .unwrap_or("");
        let replacement = loop {
            let candidate = format!("{prefix}{ENCODED_PREFIX}h{sequence}{suffix}");
            sequence += 1;
            if !occupied.contains(candidate.as_str()) && !external.contains(candidate.as_str()) {
                break candidate;
            }
        };
        renamed.insert(*name, replacement);
    }
    rewrite_identifiers(&source, |name| renamed.get(name).cloned())
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
    use super::*;
    use std::collections::BTreeSet;

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

    #[test]
    fn adversarial_source_names_are_injective_and_standard_c() {
        let names = [
            "a",
            "a.b",
            "a_b",
            "a__b",
            "aZdb",
            "aZZdb",
            "aZpb",
            "aZlbZr",
            "a[1].b",
            "a.1.b",
            "aZx2Eb",
            "cI_aZdb",
            "cI_",
            "__llg_ident_e_612E62",
            "__UPPER",
            "_Upper",
            "llg_model_start",
            "sv4_t",
            "G_a",
            "D_a",
            "E_a",
            "O_a",
            "p_a",
            "f_a",
            "fn_a",
            "g_a",
            "LLG_MODEL_NO_MAIN",
            "SV4_EMPTY",
            "0a",
            "",
            "é",
            "\\bus+index ",
            "bus+index",
            "bus-index",
            "bus@index",
        ];
        let encoded = names.into_iter().map(ident).collect::<BTreeSet<_>>();
        assert_eq!(encoded.len(), names.len());
        for name in encoded {
            assert!(!name.starts_with("__"));
            assert!(!name.as_bytes()[0].is_ascii_digit());
            assert!(name
                .bytes()
                .all(|byte| byte == b'_' || byte.is_ascii_alphanumeric()));
        }
        for name in ["plain", "a_b", "a__b", "withZ", "_lower"] {
            assert_eq!(ident(name), name);
        }
    }

    #[test]
    fn source_components_are_composed_once_with_distinct_boundaries() {
        assert_eq!(
            path_ident(&["pca_sites", "sites[0]", "v"]),
            "cI_pcaZusitesZpsitesZl0ZrZpv"
        );
        let paths: &[&[&str]] = &[
            &["a", "b"],
            &["a.b"],
            &["a_b"],
            &["a__b"],
            &["aZpb"],
            &["a", "1", "b"],
            &["a[1]", "b"],
            &["a", "b.c"],
            &["a.b", "c"],
            &[""],
            &[],
            &["", ""],
            &["cI_aZpb"],
        ];
        let encoded = paths
            .iter()
            .map(|parts| path_ident(parts))
            .collect::<BTreeSet<_>>();
        assert_eq!(encoded.len(), paths.len());
        assert_ne!(global_name("a_b", "c"), global_name("a", "b_c"));
        assert_ne!(
            scoped_name("G", &["a", "b", "c"]),
            scoped_name("G", &["a.b", "c"])
        );
        assert_eq!(global_name("tb", "plain_name"), "G_tb_plain_name");
        assert_ne!(
            function_ident("leaf_desc_sites"),
            format!("{}_desc_sites", function_ident("leaf"))
        );
        assert_ne!(
            function_ident("leaf_frame_t"),
            format!("{}_frame_t", function_ident("leaf"))
        );
        assert_ne!(global_name("cI", "aZdbZpc"), global_name("a.b", "c"));
        assert_ne!(
            scoped_name("fn", &["tb", "leaf_desc"]),
            format!("{}_desc", scoped_name("fn", &["tb", "leaf"]))
        );
        assert_ne!(
            scoped_name("fn", &["tb", "leaf_desc_sites"]),
            format!("{}_desc_sites", scoped_name("fn", &["tb", "leaf"]))
        );
        assert_ne!(
            scoped_name("E", &["tb", "wake__elements"]),
            format!("{}__elements", scoped_name("E", &["tb", "wake"]))
        );
    }

    #[test]
    fn length_fallback_is_unique_deterministic_and_preserves_literals_and_abi() {
        let long = format!("p_{}", "scope_".repeat(MAX_C_IDENTIFIER_LEN));
        let other = format!("{long}_other");
        let foreign = format!("foreign_{}", "x".repeat(MAX_C_IDENTIFIER_LEN));
        let source = format!(
            "int p_cI_h0; int {long}; int {other}; extern int {foreign}(void);\n\
             use({long}, {other}, {foreign}());\n\
             char *s = \"{long}\"; /* {other} */ // {long}\n\
             char c = 'Z'; unsigned n = 123{};\n",
            "U".repeat(MAX_C_IDENTIFIER_LEN + 1)
        );
        let external = BTreeSet::from([foreign.as_str()]);
        let bounded = bound_identifiers(source.clone(), &external);
        assert_eq!(bounded, bound_identifiers(source, &external));
        assert!(bounded.contains("int p_cI_h0; int p_cI_h1; int p_cI_h2;"));
        assert!(bounded.contains("use(p_cI_h1, p_cI_h2,"));
        assert!(bounded.contains(&format!("\"{long}\"")));
        assert!(bounded.contains(&format!("/* {other} */ // {long}")));
        assert!(bounded.contains(&format!("extern int {foreign}(void)")));
        for span in identifier_spans(&bounded) {
            let name = &bounded[span];
            assert!(name.len() <= MAX_C_IDENTIFIER_LEN || name == foreign);
        }
    }

    #[test]
    fn complete_symbol_bound_includes_derived_names_at_the_boundary() {
        let name = format!("p_{}", "a".repeat(MAX_C_IDENTIFIER_LEN - 2));
        let source = format!("int {name}; int {name}_desc; int {name}_frame_t;");
        let bounded = bound_identifiers(source, &BTreeSet::new());
        assert!(bounded.contains(&format!("int {name};")));
        assert!(bounded.contains("p_cI_h0_desc"));
        assert!(bounded.contains("p_cI_h1_frame_t"));
        assert!(identifier_spans(&bounded).all(|span| span.len() <= MAX_C_IDENTIFIER_LEN));
    }
}
