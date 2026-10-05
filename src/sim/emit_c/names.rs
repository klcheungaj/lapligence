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
pub(crate) const MAX_C_IDENTIFIER_LEN: usize = 32;

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
    std::iter::from_fn(move || next_identifier(bytes, &mut index))
}

/// Advance `index` past the next identifier of `bytes` and return its span.
/// The scan reads only bytes at or after `index`, so a caller may rewrite the
/// bytes before it between calls.
fn next_identifier(bytes: &[u8], index: &mut usize) -> Option<std::ops::Range<usize>> {
    while *index < bytes.len() {
        let byte = bytes[*index];
        if matches!(byte, b'"' | b'\'') {
            *index += 1;
            while *index < bytes.len() && bytes[*index] != byte {
                *index += if bytes[*index] == b'\\' { 2 } else { 1 };
            }
            *index = (*index + 1).min(bytes.len());
        } else if byte == b'/' && bytes.get(*index + 1) == Some(&b'*') {
            *index = bytes[*index + 2..]
                .windows(2)
                .position(|window| window == b"*/")
                .map_or(bytes.len(), |offset| *index + 2 + offset + 2);
        } else if byte == b'/' && bytes.get(*index + 1) == Some(&b'/') {
            *index = bytes[*index..]
                .iter()
                .position(|byte| *byte == b'\n')
                .map_or(bytes.len(), |offset| *index + offset);
        } else if byte == b'_' || byte.is_ascii_alphanumeric() {
            let start = *index;
            while *index < bytes.len()
                && (bytes[*index] == b'_' || bytes[*index].is_ascii_alphanumeric())
            {
                *index += 1;
            }
            if !byte.is_ascii_digit() {
                return Some(start..*index);
            }
        } else {
            *index += 1;
        }
    }
    None
}

/// Rewrite identifiers in emitted C without touching user-visible text.
pub(super) fn rewrite_identifiers<S: AsRef<str>>(
    text: &str,
    mut rename: impl FnMut(&str) -> Option<S>,
) -> String {
    let mut out = String::with_capacity(text.len());
    let mut copied = 0;
    for span in identifier_spans(text) {
        if let Some(replacement) = rename(&text[span.clone()]) {
            out.push_str(&text[copied..span.start]);
            out.push_str(replacement.as_ref());
            copied = span.end;
        }
    }
    out.push_str(&text[copied..]);
    out
}

/// Rewrite identifiers like [`rewrite_identifiers`], reusing the buffer of
/// `text` when no replacement is longer than the identifier it replaces.
///
/// The whole model text is the largest single allocation of C emission, so
/// shortening its identifiers in place avoids holding a second copy.
pub(super) fn rewrite_identifiers_in_place(
    text: String,
    renamed: &std::collections::HashMap<String, String>,
) -> String {
    if renamed
        .iter()
        .any(|(original, replacement)| replacement.len() > original.len())
    {
        return rewrite_identifiers(&text, |name| renamed.get(name).map(String::as_str));
    }
    let mut bytes = text.into_bytes();
    let (mut read, mut write, mut copied) = (0, 0, 0);
    while let Some(span) = next_identifier(&bytes, &mut read) {
        // Identifiers are ASCII, so every span is valid UTF-8.
        let replacement = std::str::from_utf8(&bytes[span.clone()])
            .ok()
            .and_then(|name| renamed.get(name));
        if let Some(replacement) = replacement {
            bytes.copy_within(copied..span.start, write);
            write += span.start - copied;
            bytes[write..write + replacement.len()].copy_from_slice(replacement.as_bytes());
            write += replacement.len();
            copied = span.end;
        }
    }
    let end = bytes.len();
    bytes.copy_within(copied..end, write);
    bytes.truncate(write + end - copied);
    // Only ASCII identifiers were replaced by ASCII text, so the bytes remain
    // valid UTF-8; the conversion is checked rather than assumed.
    let mut text = String::from_utf8(bytes)
        .unwrap_or_else(|error| String::from_utf8_lossy(error.as_bytes()).into_owned());
    text.shrink_to_fit();
    text
}

pub(crate) struct BoundedIdentifiers {
    pub source: String,
    pub symbols_tsv: String,
}

pub(super) fn runtime_identifiers() -> &'static std::collections::BTreeSet<&'static str> {
    static NAMES: std::sync::OnceLock<std::collections::BTreeSet<&'static str>> =
        std::sync::OnceLock::new();
    NAMES.get_or_init(|| {
        [
            include_str!("../rt/llg_compiler.h"),
            include_str!("../rt/llg_rt.h"),
            include_str!("../rt/llg_value.h"),
            include_str!("../rt/value/backend.h"),
            include_str!("../rt/value/bridge.h"),
            include_str!("../rt/value/consumer_bridge.h"),
            include_str!("../rt/value/destinations.h"),
            include_str!("../rt/llg_random.h"),
            include_str!("../rt/llg_rng.h"),
            include_str!("../rt/llg_co.h"),
            include_str!("../rt/llg_vpi.h"),
            include_str!("../rt/vpi_user.h"),
            include_str!("../rt/llg_container.h"),
            include_str!("../rt/llg_string.h"),
            include_str!("../rt/llg_wave.h"),
        ]
        .into_iter()
        .flat_map(|header| identifier_spans(header).map(move |span| &header[span]))
        .collect()
    })
}

fn registry_index(mut index: usize) -> String {
    const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut digits = Vec::new();
    loop {
        digits.push(char::from(DIGITS[index % DIGITS.len()]));
        index /= DIGITS.len();
        if index == 0 {
            return digits.into_iter().rev().collect();
        }
    }
}

fn bounded_name(name: &str, sequence: usize) -> String {
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
    let index = registry_index(sequence);
    let stem_len = MAX_C_IDENTIFIER_LEN - prefix.len() - suffix.len() - index.len() - 2;
    let stem = &name[prefix.len()..prefix.len() + stem_len];
    format!("{prefix}{stem}_h{index}{suffix}")
}

/// Bound complete internal symbols using a deterministic per-model registry.
///
/// Sort original names, assign base-36 indices and skip occupied, previously
/// assigned and external names. Leading characters retain hierarchy context
/// without decoding the reversible source-name encoding. Reserve header tokens
/// too: a smaller internal cap must never rename a runtime API or macro.
/// Run after all derived names exist; preserve namespace and frame/descriptor
/// suffixes. The sidecar lists every substitution, sorted by shortened name.
pub(super) fn bound_identifiers(
    source: String,
    external: &std::collections::BTreeSet<&str>,
) -> BoundedIdentifiers {
    let runtime = runtime_identifiers();
    let is_external = |name: &str| external.contains(name) || runtime.contains(name);
    if !identifier_spans(&source)
        .any(|span| span.len() > MAX_C_IDENTIFIER_LEN && !is_external(&source[span]))
    {
        return BoundedIdentifiers {
            source,
            symbols_tsv: String::new(),
        };
    }
    // Every replacement is exactly `MAX_C_IDENTIFIER_LEN` long (see
    // `bounded_name`), so only occupied identifiers of that length can
    // collide with one.
    let mut occupied = std::collections::HashSet::new();
    let mut oversized = std::collections::HashSet::new();
    for span in identifier_spans(&source) {
        let name = &source[span];
        if name.len() == MAX_C_IDENTIFIER_LEN {
            occupied.insert(name);
        } else if name.len() > MAX_C_IDENTIFIER_LEN && !is_external(name) {
            oversized.insert(name);
        }
    }
    let mut oversized = oversized.into_iter().collect::<Vec<_>>();
    oversized.sort_unstable();
    let mut symbols = std::collections::BTreeMap::new();
    let mut sequence = 0usize;
    for name in oversized {
        let replacement = loop {
            let candidate = bounded_name(name, sequence);
            sequence += 1;
            if !occupied.contains(candidate.as_str())
                && !is_external(candidate.as_str())
                && !symbols.contains_key(&candidate)
            {
                break candidate;
            }
        };
        symbols.insert(replacement, name);
    }
    let mut symbols_tsv = String::new();
    let mut renamed = std::collections::HashMap::with_capacity(symbols.len());
    for (short, original) in symbols {
        symbols_tsv.push_str(&short);
        symbols_tsv.push('\t');
        symbols_tsv.push_str(original);
        symbols_tsv.push('\n');
        renamed.insert(original.to_owned(), short);
    }
    drop(occupied);
    BoundedIdentifiers {
        source: rewrite_identifiers_in_place(source, &renamed),
        symbols_tsv,
    }
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
        let stem = "readable".repeat(MAX_C_IDENTIFIER_LEN);
        let long = format!("p_{stem}");
        let other = format!("{long}_other");
        let foreign = format!("foreign_{}", "x".repeat(MAX_C_IDENTIFIER_LEN));
        let occupied = format!("p_{}_h0", &stem[..MAX_C_IDENTIFIER_LEN - 5]);
        let external_candidate = format!("p_{}_h1", &stem[..MAX_C_IDENTIFIER_LEN - 5]);
        let source = format!(
            "int {occupied}; int {long}; int {other}; extern int {foreign}(void);\n\
             use({long}, {other}, {foreign}());\n\
             char *s = \"{long}\"; /* {other} */ // {long}\n\
             char c = 'Z'; unsigned n = 123{};\n",
            "U".repeat(MAX_C_IDENTIFIER_LEN + 1)
        );
        let external = BTreeSet::from([foreign.as_str(), external_candidate.as_str()]);
        let bounded = bound_identifiers(source.clone(), &external);
        let repeated = bound_identifiers(source.clone(), &external);
        assert_eq!(bounded.source, repeated.source);
        assert_eq!(bounded.symbols_tsv, repeated.symbols_tsv);
        let rows = bounded
            .symbols_tsv
            .lines()
            .map(|row| row.split_once('\t').unwrap())
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[0],
            (
                format!("p_{}_h2", &stem[..MAX_C_IDENTIFIER_LEN - 5]).as_str(),
                long.as_str()
            )
        );
        assert_eq!(
            rows[1],
            (
                format!("p_{}_h3", &stem[..MAX_C_IDENTIFIER_LEN - 5]).as_str(),
                other.as_str()
            )
        );
        assert!(bounded.source.contains(&format!("int {occupied};")));
        assert!(bounded.source.contains(&format!("\"{long}\"")));
        assert!(bounded.source.contains(&format!("/* {other} */ // {long}")));
        assert!(bounded
            .source
            .contains(&format!("extern int {foreign}(void)")));
        let reverse = rows
            .iter()
            .map(|(short, original)| (*short, *original))
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(
            rewrite_identifiers(&bounded.source, |name| reverse
                .get(name)
                .map(|original| (*original).to_owned())),
            source
        );
        for span in identifier_spans(&bounded.source) {
            let name = &bounded.source[span];
            assert!(name.len() <= MAX_C_IDENTIFIER_LEN || name == foreign);
        }
    }

    #[test]
    fn in_place_rewrite_matches_the_copying_rewrite() {
        let text = "int long_name_a; /* long_name_a */ char *s = \"long_name_a \\\" x\";\n\
                    char c = '\\''; long_name_b(long_name_a, 12long_name_a); // long_name_b\n\
                    é long_name_b_tail long_name_b";
        let shorter = std::collections::HashMap::from([
            ("long_name_a".to_owned(), "a".to_owned()),
            ("long_name_b".to_owned(), "bb".to_owned()),
        ]);
        let longer = std::collections::HashMap::from([
            ("long_name_a".to_owned(), "a".to_owned()),
            ("long_name_b".to_owned(), "much_longer_name_b".to_owned()),
        ]);
        for renamed in [&shorter, &longer, &std::collections::HashMap::new()] {
            let expected = rewrite_identifiers(text, |name| renamed.get(name).map(String::as_str));
            assert_eq!(
                rewrite_identifiers_in_place(text.to_owned(), renamed),
                expected
            );
        }
        let in_place = rewrite_identifiers_in_place(text.to_owned(), &shorter);
        assert!(in_place.starts_with("int a; /* long_name_a */ char *s = \"long_name_a "));
        assert!(in_place.ends_with("é long_name_b_tail bb"));
        assert!(in_place.contains("bb(a, 12long_name_a); // long_name_b\n"));
        // An unterminated trailing comment or literal keeps the remaining text.
        for tail in ["/* long_name_a", "\"long_name_a", "// long_name_a"] {
            let text = format!("long_name_a {tail}");
            assert_eq!(
                rewrite_identifiers_in_place(text.clone(), &shorter),
                format!("a {tail}")
            );
        }
    }

    #[test]
    fn complete_symbol_bound_keeps_prefix_stem_and_derived_suffixes() {
        for prefix in [
            "fn_", "G_", "D_", "E_", "O_", "p_", "f_", "g_", "llg_", "_llg_", "",
        ] {
            for suffix in ["", "_desc", "_frame_t"] {
                let name = format!(
                    "{prefix}hierarchy_{}{suffix}",
                    "a".repeat(MAX_C_IDENTIFIER_LEN)
                );
                let bounded =
                    bound_identifiers(format!("int {name}; use({name});"), &BTreeSet::new());
                let (short, original) = bounded.symbols_tsv.trim_end().split_once('\t').unwrap();
                assert_eq!(original, name);
                assert_eq!(short.len(), MAX_C_IDENTIFIER_LEN);
                assert!(short.starts_with(&format!("{prefix}hier")));
                assert!(short.ends_with(&format!("_h0{suffix}")));
                assert_eq!(bounded.source, format!("int {short}; use({short});"));
            }
        }
        let name = format!("p_{}", "a".repeat(MAX_C_IDENTIFIER_LEN - 2));
        let bounded = bound_identifiers(
            format!("int {name}; int {name}_desc; int {name}_frame_t;"),
            &BTreeSet::new(),
        );
        assert!(bounded.source.contains(&format!("int {name};")));
        assert_eq!(bounded.symbols_tsv.lines().count(), 2);
        assert!(identifier_spans(&bounded.source).all(|span| span.len() <= MAX_C_IDENTIFIER_LEN));
    }

    #[test]
    fn registry_indices_cross_digit_boundaries_without_collisions() {
        let source = (0..1400)
            .map(|index| {
                format!(
                    "int G_{}_{index};\n",
                    "shared_stem".repeat(MAX_C_IDENTIFIER_LEN)
                )
            })
            .collect::<String>();
        let bounded = bound_identifiers(source, &BTreeSet::new());
        let names = identifier_spans(&bounded.source)
            .map(|span| &bounded.source[span])
            .filter(|name| *name != "int")
            .collect::<BTreeSet<_>>();
        assert_eq!(names.len(), 1400);
        assert_eq!(bounded.symbols_tsv.lines().count(), 1400);
        assert!(names
            .iter()
            .all(|name| name.len() == MAX_C_IDENTIFIER_LEN && name.starts_with("G_shared_stem")));
        assert_eq!(registry_index(35), "z");
        assert_eq!(registry_index(36), "10");
        assert_eq!(registry_index(1296), "100");
    }

    #[test]
    fn runtime_header_identifiers_are_reserved() {
        let source =
            "llg_rt_init_with_args_and_precision(0, 0, 0); LLG_CONTAINER_METHOD_FIND_LAST_INDEX;";
        let bounded = bound_identifiers(source.to_owned(), &BTreeSet::new());
        assert_eq!(bounded.source, source);
        assert!(bounded.symbols_tsv.is_empty());
    }
}
