use super::*;
use crate::core::compile::{
    admit_in_memory_library_maps, admit_library_maps, admit_library_maps_with_targets,
    collect_in_memory_library_maps, CompileOpts, LanguageEdition, MAX_LIBRARY_MAP_WORK,
};
use crate::ffi::platform::CanonicalPath as _;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

fn in_memory(text: &str) -> Result<Vec<LibrarySource>, StartupError> {
    let maps = [OwnedSource::include("virtual/root.map", text)];
    let mut sources = vec![OwnedSource::compilation_unit("virtual/rtl/cell.sv", "body")];
    let mut libraries = Vec::new();
    let mut count = 2;
    let mut bytes = u64::MAX;
    let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
    admit_in_memory_library_maps(
        &maps,
        &mut sources,
        &mut libraries,
        &mut count,
        &mut bytes,
        8,
        &mut work,
    )?;
    assert!(sources.is_empty());
    assert_eq!(
        count, 2,
        "moving an admitted buffer must not charge a second source"
    );
    assert_eq!(u64::MAX - bytes, libraries[0].library.len() as u64);
    Ok(libraries)
}

#[test]
fn specificity_depends_on_the_final_component() {
    use LibrarySpecificity::{Directory, ExplicitFile, WildcardFile};
    for (pattern, rank) in [
        ("rtl/", Directory),
        (r"rtl\", Directory),
        ("rtl/*.sv", WildcardFile),
        ("rtl/cell.s?", WildcardFile),
        ("*/cell.sv", ExplicitFile),
        ("**/cell.sv", ExplicitFile),
        (r"rtl\cell.sv", ExplicitFile),
    ] {
        assert_eq!(LibrarySpecificity::of_pattern(pattern), rank, "{pattern}");
    }
}

#[test]
fn all_specificity_orders_resolve_before_ambiguity() {
    let entries = [
        "library lower rtl/;",
        "library broad rtl/*.sv;",
        "library chosen */cell.sv;",
    ];
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let text = order.map(|index| entries[index]).join("\n");
        let sources = in_memory(&text).expect("specificity should resolve every permutation");
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].library, "chosen");
    }
    for text in [
        "library A rtl/*.sv; library B rtl/*.sv; library chosen rtl/cell.sv;",
        "library chosen rtl/cell.sv; library B rtl/*.sv; library A rtl/*.sv;",
    ] {
        assert_eq!(in_memory(text).unwrap()[0].library, "chosen");
    }
}

#[test]
fn equal_rank_cross_library_ties_reject_but_same_library_repeats_do_not() {
    for text in [
        "library A rtl/*.sv; library B rtl/*.sv;",
        "library A rtl/cell.sv; library B */cell.sv;",
    ] {
        let error = in_memory(text).expect_err("unresolved winning-rank tie");
        assert_eq!(error.kind(), StartupErrorKind::InvalidArgument);
        assert!(error.contains("ambiguous library mapping"));
        assert!(error.contains("virtual/rtl/cell.sv"));
    }
    let sources = in_memory("library chosen rtl/*.sv, rtl/*.sv; library chosen rtl/;").unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].library, "chosen");
}

#[test]
fn explicit_admitted_library_assignment_overrides_maps_without_recharging() {
    let maps = [OwnedSource::include(
        "root.map",
        "library A *.sv; library B cell.sv;",
    )];
    let mut sources = Vec::new();
    let mut libraries = vec![LibrarySource::new("cell.sv", "overlay", "explicit")];
    let mut count = 2;
    let mut bytes = 0;
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
    .expect("maps cannot override an explicit admitted library assignment");
    assert_eq!(
        libraries,
        vec![LibrarySource::new("cell.sv", "overlay", "explicit")]
    );
    assert_eq!(bytes, 0);
}

#[test]
fn mapping_metadata_budget_fails_before_moving_sources() {
    let maps = [OwnedSource::include("root.map", "library chosen cell.sv;")];
    let mut sources = vec![OwnedSource::compilation_unit("cell.sv", "body")];
    let original = sources.clone();
    let mut libraries = Vec::new();
    let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
    let error = admit_in_memory_library_maps(
        &maps,
        &mut sources,
        &mut libraries,
        &mut 2,
        &mut 5,
        2,
        &mut work,
    )
    .expect_err("six library-name bytes do not fit a five-byte remainder");
    assert_eq!(error.kind(), StartupErrorKind::LimitExceeded);
    assert_eq!(sources, original);
    assert!(libraries.is_empty());
}

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "llg-map-specificity-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir_all(path.join("rtl")).expect("temporary source directory");
        std::fs::write(path.join("rtl/cell.sv"), "body").expect("source bytes");
        // Disk admission names files by their resolved handle path, while
        // in-memory maps resolve lexically; macOS reports /var/... as
        // /private/var/... and Windows expands 8.3 short names, so use the
        // resolved spelling for both.
        let path = path.canonical().expect("canonical temporary directory");
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn filesystem_maps_share_precedence_and_charge_each_source_once() {
    let directory = Directory::new();
    for text in [
        "library lower rtl/; library broad rtl/*.sv; library chosen */cell.sv;",
        "library chosen */cell.sv; library broad rtl/*.sv; library lower rtl/;",
        "library A rtl/*.sv; library B rtl/*.sv; library chosen rtl/cell.sv;",
        "library chosen rtl/*.sv, rtl/*.sv; library chosen rtl/;",
    ] {
        let map = directory.0.join("root.map");
        std::fs::write(&map, text).unwrap();
        let opts = CompileOpts {
            library_map_files: vec![map.to_string_lossy().into_owned()],
            ..Default::default()
        };
        let mut identities = HashSet::new();
        let mut libraries = Vec::new();
        let mut count = 0;
        let mut remaining = u64::MAX;
        let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
        admit_library_maps(
            &opts,
            &mut identities,
            &mut libraries,
            &mut count,
            &mut remaining,
            &mut work,
        )
        .expect("ranked disk admission");
        assert_eq!(libraries.len(), 1);
        assert_eq!(libraries[0].library, "chosen");
        assert_eq!(libraries[0].text, "body");
        assert_eq!(
            count, 2,
            "one map and one source are admitted for all matching patterns"
        );
    }
}

#[test]
fn disk_and_logical_maps_resolve_one_joint_candidate_set() {
    let directory = Directory::new();
    let map = directory.0.join("disk.map");
    std::fs::write(&map, "library A rtl/*.sv; library B rtl/*.sv;").unwrap();
    let opts = CompileOpts {
        library_map_files: vec![map.to_string_lossy().into_owned()],
        ..Default::default()
    };
    let logical_name = directory
        .0
        .join("memory.map")
        .to_string_lossy()
        .into_owned();
    let logical_maps = [OwnedSource::include(
        logical_name,
        "library chosen rtl/cell.sv;",
    )];
    let mut identities = HashSet::new();
    let mut targets = HashMap::new();
    let mut sources = Vec::new();
    let mut libraries = Vec::new();
    let mut count = 1;
    let mut remaining = u64::MAX;
    let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
    let mut buffers = LibraryMapBuffers::new(&mut sources, &mut libraries, &mut work).unwrap();
    admit_library_maps_with_targets(
        &opts,
        &mut Vec::new(),
        &mut identities,
        &mut buffers,
        &mut count,
        &mut remaining,
        &mut work,
        &mut targets,
    )
    .expect("defer disk-map tie");
    collect_in_memory_library_maps(
        &logical_maps,
        &[],
        LanguageEdition::SystemVerilog2009,
        &[],
        &mut Vec::new(),
        &mut buffers,
        count,
        8,
        &mut work,
    )
    .unwrap();
    buffers
        .finish(&mut remaining, &mut work)
        .expect("logical explicit filename resolves tie");
    assert_eq!(libraries.len(), 1);
    assert_eq!(libraries[0].library, "chosen");
}

#[test]
fn disk_map_uses_existing_cli_source_bytes_instead_of_reading_again() {
    let directory = Directory::new();
    let name = directory.0.join("rtl/cell.sv").canonical().unwrap();
    let map = directory.0.join("root.map");
    std::fs::write(&map, "library chosen rtl/*.sv;").unwrap();
    let opts = CompileOpts {
        library_map_files: vec![map.to_string_lossy().into_owned()],
        ..Default::default()
    };
    let mut sources = vec![OwnedSource::compilation_unit(
        name.to_string_lossy(),
        "retained bytes",
    )];
    let mut libraries = Vec::new();
    let mut identities = HashSet::from([name]);
    let mut targets = HashMap::new();
    let mut count = 1;
    let mut remaining = u64::MAX;
    let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
    let mut buffers = LibraryMapBuffers::new(&mut sources, &mut libraries, &mut work).unwrap();
    admit_library_maps_with_targets(
        &opts,
        &mut Vec::new(),
        &mut identities,
        &mut buffers,
        &mut count,
        &mut remaining,
        &mut work,
        &mut targets,
    )
    .unwrap();
    buffers.finish(&mut remaining, &mut work).unwrap();
    assert_eq!(libraries[0].text, "retained bytes");
    assert_eq!(count, 2, "the existing source plus its map input");
    assert!(sources.is_empty());
}

#[test]
fn unrecorded_buffers_fail_closed_before_publication() {
    let mut sources = Vec::new();
    let mut libraries = Vec::new();
    let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
    let buffers = LibraryMapBuffers::new(&mut sources, &mut libraries, &mut work).unwrap();
    buffers
        .sources
        .push(OwnedSource::compilation_unit("unrecorded.sv", "body"));
    let mut remaining = u64::MAX;
    let error = buffers
        .finish(&mut remaining, &mut work)
        .expect_err("missing candidate record");
    assert_eq!(error.kind(), StartupErrorKind::Internal);
    assert!(libraries.is_empty());
    assert_eq!(sources.len(), 1);
}

#[test]
fn filesystem_map_configurations_are_registered_before_mapping_later_files() {
    let directory = Directory::new();
    let first = directory.0.join("first.map");
    let second = directory.0.join("second.map");
    let configuration = "config chosen; design work.top; endconfig";
    std::fs::write(&first, "include second.map; library configs second.map;").unwrap();
    std::fs::write(&second, configuration).unwrap();
    let opts = CompileOpts {
        library_map_files: vec![first.to_string_lossy().into_owned()],
        ..Default::default()
    };
    let mut sources = Vec::new();
    let mut libraries = Vec::new();
    let mut identities = HashSet::new();
    let mut targets = HashMap::new();
    let mut count = 0;
    let mut remaining = u64::MAX;
    let mut work = LibraryMapWorkBudget::new(MAX_LIBRARY_MAP_WORK);
    let mut buffers = LibraryMapBuffers::new(&mut sources, &mut libraries, &mut work).unwrap();
    admit_library_maps_with_targets(
        &opts,
        &mut Vec::new(),
        &mut identities,
        &mut buffers,
        &mut count,
        &mut remaining,
        &mut work,
        &mut targets,
    )
    .expect("admit each map once before mapping its configuration source");
    let originals = buffers.finish(&mut remaining, &mut work).unwrap();
    assert_eq!(
        count, 2,
        "one root map and one included map, not a third source read"
    );
    assert!(sources.is_empty());
    assert_eq!(libraries.len(), 1);
    assert_eq!(libraries[0].library, "configs");
    assert_eq!(libraries[0].text, configuration);
    assert_eq!(originals.len(), 1);
    assert_eq!(originals[0].text, configuration);
}
