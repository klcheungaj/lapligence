use super::*;
use crate::core::compile::{
    admit_in_memory_library_maps, parse_library_map, LibrarySource, OwnedSource,
    MAX_LIBRARY_MAP_WORK,
};

#[test]
fn projection_keeps_configuration_bytes_and_source_offsets() {
    let text = "// UTF-8: é\r\nlibrary cells rtl/*.v;\r\nconfig cfg;\n design work.top;\nendconfig : cfg\ninclude more.map;\nconfig second; design work.other; endconfig\n";
    let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
    let map = parse_library_map(text, &mut work).unwrap();
    assert_eq!(map.includes, ["more.map"]);
    assert_eq!(map.entries[0].patterns, ["rtl/*.v"]);
    let projected = map.configuration.unwrap();
    assert_eq!(projected.len(), text.len());
    assert_eq!(projected.find("config cfg"), text.find("config cfg"));
    assert_eq!(projected.find("config second"), text.find("config second"));
    assert!(projected.contains("endconfig : cfg"));
    assert!(!projected.contains("library cells"));
    for (offset, byte) in text.bytes().enumerate() {
        if matches!(byte, b'\r' | b'\n') {
            assert_eq!(projected.as_bytes()[offset], byte);
        }
    }
}

#[test]
fn config_lexing_does_not_treat_keywords_in_paths_comments_or_strings_as_declarations() {
    let text = concat!(
        "library L config, \\\\server\\share\\endconfig, \"endconfig\";\n",
        "config \\endconfig ;\n",
        " // endconfig\n localparam A = \"endconfig \\\" still quoted\";\n",
        " design work.top; /* endconfig */\n",
        " instance top.\\endconfig use L.cell;\n",
        "endconfig : \\endconfig \n library M other.v;\n",
    );
    let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
    let map = parse_library_map(text, &mut work).unwrap();
    assert_eq!(map.entries.len(), 2);
    assert_eq!(map.entries[0].patterns[0], "config");
    assert!(map
        .configuration
        .unwrap()
        .contains("endconfig : \\endconfig"));
}

#[test]
fn malformed_map_configurations_fail_directly_and_budgeted_projection_fails_closed() {
    for (text, expected) in [
        ("config c; design work.top;", "missing endconfig"),
        ("config c; /* endconfig", "unterminated comment"),
        (
            "config c; localparam S = \"endconfig",
            "unterminated string",
        ),
        ("endconfig", "unexpected library map token"),
        ("include other.map", "terminating semicolon"),
    ] {
        let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
        let error = parse_library_map(text, &mut work).unwrap_err();
        assert_eq!(error.kind(), StartupErrorKind::InvalidArgument);
        assert!(error.contains(expected), "{error}");
    }
    let mut work = LibraryMapWorkBudget::with_allocation_limit(MAX_LIBRARY_MAP_WORK, 0);
    let error = project("config c; endconfig", &[0..18], &mut work).unwrap_err();
    assert_eq!(error.kind(), StartupErrorKind::LimitExceeded);
    assert!(error.contains("allocation budget"));
}

#[test]
fn in_memory_configs_are_compilation_units_without_extra_source_admission() {
    let text = "library L cell.v;\nconfig cfg; design work.top; default liblist L; endconfig\n";
    let maps = [OwnedSource::include("virtual/root.map", text)];
    let mut sources = vec![OwnedSource::compilation_unit(
        "virtual/cell.v",
        "module cell; endmodule",
    )];
    let mut libraries = Vec::new();
    let mut count = 2;
    let mut bytes = 32;
    let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
    admit_in_memory_library_maps(
        &maps,
        &mut sources,
        &mut libraries,
        &mut count,
        &mut bytes,
        2,
        &mut work,
    )
    .unwrap();
    assert_eq!(count, 2, "the map was already counted as one input");
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].name, "virtual/root.map");
    assert!(sources[0].is_compilation_unit);
    assert_eq!(sources[0].text.len(), text.len());
    assert!(sources[0].text.contains("config cfg;"));
    assert_eq!(libraries[0].library, "L");
    assert_eq!(
        bytes, 31,
        "only the mapped library-name metadata is new input"
    );
}

#[test]
fn configs_in_later_maps_can_be_mapped_and_explicit_library_choices_survive() {
    for explicit in [false, true] {
        let first = "library chosen second.map;";
        let second = "config c; design work.top; endconfig";
        let maps = [
            OwnedSource::include("first.map", first),
            OwnedSource::include("second.map", second),
        ];
        let mut sources = Vec::new();
        let mut libraries = if explicit {
            vec![LibrarySource::new("second.map", second, "manual")]
        } else {
            Vec::new()
        };
        let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
        admit_in_memory_library_maps(
            &maps,
            &mut sources,
            &mut libraries,
            &mut 3,
            &mut 100,
            4,
            &mut work,
        )
        .unwrap();
        assert!(sources.is_empty());
        assert_eq!(libraries.len(), 1);
        assert_eq!(
            libraries[0].library,
            if explicit { "manual" } else { "chosen" }
        );
        assert_eq!(libraries[0].text, second);
    }
}

#[test]
fn conflicting_same_name_source_is_not_overwritten_by_a_map() {
    let maps = [OwnedSource::include(
        "same.map",
        "config c; design work.top; endconfig",
    )];
    let mut sources = vec![OwnedSource::compilation_unit(
        "same.map",
        "module different; endmodule",
    )];
    let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
    let error = admit_in_memory_library_maps(
        &maps,
        &mut sources,
        &mut Vec::new(),
        &mut 2,
        &mut 100,
        3,
        &mut work,
    )
    .unwrap_err();
    assert!(error.contains("configuration conflicts with admitted source"));
    assert_eq!(sources[0].text, "module different; endmodule");
}

#[test]
fn restored_map_text_keeps_same_line_utf16_positions_and_unknown_sources_fail_closed() {
    let text = "/* \u{e9}\u{1f600} */ config/*gap*/ c; design work.top; endconfig\n";
    let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
    let projected = parse_library_map(text, &mut work)
        .unwrap()
        .configuration
        .unwrap();
    let offset = text.find("design").unwrap() as u64;
    let expected = crate::core::compile::one_based_utf16_position(text, offset);
    assert_ne!(
        crate::core::compile::one_based_utf16_position(&projected, offset),
        expected
    );
    let mut files = vec![crate::ffi::slang::File {
        id: 0,
        name: "unicode.map".to_owned(),
        byte_len: text.len() as u64,
        text: projected,
    }];
    restore_source_text(
        &mut files,
        vec![OwnedSource::include("unicode.map", text)],
        &mut work,
    )
    .expect("restore the owned original without changing offsets");
    assert_eq!(files[0].text, text);
    assert_eq!(
        crate::core::compile::one_based_utf16_position(&files[0].text, offset),
        expected
    );
    let error = restore_source_text(
        &mut files,
        vec![OwnedSource::include("missing.map", text)],
        &mut work,
    )
    .unwrap_err();
    assert_eq!(error.kind(), StartupErrorKind::Internal);
}

#[test]
fn duplicate_logical_maps_cannot_silently_discard_a_different_configuration() {
    let maps = [
        OwnedSource::include("same.map", "config a; design work.top; endconfig"),
        OwnedSource::include("./same.map", "config b; design work.top; endconfig"),
    ];
    let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
    let error = admit_in_memory_library_maps(
        &maps,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut 2,
        &mut 1000,
        4,
        &mut work,
    )
    .unwrap_err();
    assert!(error.contains("conflicting in-memory library map contents"));
}
