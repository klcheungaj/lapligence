//! Path-mode include directories double as definition search directories:
//! a top-only compile admits the `.v`/`.sv` files that declare its missing
//! definitions as library units.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use llg::core::compile::{self, CompilationUnitMode, CompileOpts, StartupErrorKind};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// A fresh temporary directory, removed when dropped.
struct TempRoot(PathBuf);

impl TempRoot {
    fn new() -> Self {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "llg-include-search-{}-{sequence}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temporary root");
        Self(path)
    }

    fn write(&self, relative: &str, text: &str) -> PathBuf {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().expect("parent")).expect("create fixture directory");
        fs::write(&path, text).expect("write fixture file");
        path
    }

    fn path(&self, relative: &str) -> String {
        self.0.join(relative).to_string_lossy().into_owned()
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn opts(root: &TempRoot, files: &[&str], include_dirs: &[&str]) -> CompileOpts {
    CompileOpts {
        files: files.iter().map(|file| root.path(file)).collect(),
        include_dirs: include_dirs.iter().map(|dir| root.path(dir)).collect(),
        ..CompileOpts::default()
    }
}

/// Handle-resolved spelling of a fixture file, as admitted sources are named.
fn admitted_name(path: &Path) -> String {
    llg::ffi::platform::canonicalize(path)
        .expect("canonical fixture path")
        .to_string_lossy()
        .into_owned()
}

fn file_names(out: &compile::CompileOut) -> Vec<String> {
    out.snapshot
        .files
        .iter()
        .map(|file| file.name.clone())
        .collect()
}

fn write_design(root: &TempRoot) {
    root.write(
        "tb.sv",
        "module tb;\n  import cfg_pkg::*;\n  wire [WIDTH-1:0] y;\n  child #(.W(WIDTH)) u_child(.y(y));\nendmodule\n",
    );
    // The file name differs from the module it declares; its include is
    // admitted like any other source's.
    root.write(
        "rtl/not_child_named.sv",
        "`include \"child_defs.svh\"\nmodule child #(parameter int W = 4) (output logic [W-1:0] y);\n  grand #(.VALUE(`CHILD_VALUE)) u_grand(.y(y));\nendmodule\n",
    );
    root.write("rtl/child_defs.svh", "`define CHILD_VALUE 42\n");
    root.write(
        "rtl/pkg.sv",
        "package cfg_pkg;\n  localparam int WIDTH = 8;\nendpackage\n",
    );
    // Declares nothing missing, so it is never admitted or elaborated.
    root.write(
        "rtl/unused.sv",
        "module unused_top; initial $display(\"unused\"); endmodule\n",
    );
    // Only `.v`/`.sv` files are candidates: scanning this one would make
    // `child` ambiguous.
    root.write("rtl/notes.txt", "module child; endmodule\n");
    root.write(
        "more/grand.sv",
        "module grand #(parameter int VALUE = 0) (output logic [7:0] y);\n  assign y = VALUE;\nendmodule\n",
    );
}

#[test]
fn top_only_source_finds_children_packages_and_grandchildren() {
    let root = TempRoot::new();
    write_design(&root);
    for mode in [CompilationUnitMode::Separate, CompilationUnitMode::Merged] {
        let out = compile::compile(&CompileOpts {
            compilation_unit_mode: mode,
            ..opts(&root, &["tb.sv"], &["rtl", "more"])
        })
        .expect("definition search admits the design");
        assert!(out.ok(), "{mode:?} diagnostics: {:?}", out.diagnostics);
        let files = file_names(&out);
        for found in [
            "rtl/not_child_named.sv",
            "rtl/child_defs.svh",
            "rtl/pkg.sv",
            "more/grand.sv",
        ] {
            let name = admitted_name(&root.0.join(found));
            assert!(files.contains(&name), "{found} not admitted: {files:?}");
        }
        let unused = admitted_name(&root.0.join("rtl/unused.sv"));
        assert!(
            !files.contains(&unused),
            "unrelated file admitted: {files:?}"
        );

        // Found files are library units: only `tb` is a top.
        let tops: Vec<_> = out
            .snapshot
            .instances
            .iter()
            .filter(|instance| instance.parent_id.is_none())
            .map(|instance| instance.definition_name.as_str())
            .collect();
        assert_eq!(tops, ["tb"]);
        let definitions: Vec<_> = out
            .snapshot
            .instances
            .iter()
            .map(|instance| instance.definition_name.as_str())
            .collect();
        assert!(definitions.contains(&"child"), "{definitions:?}");
        assert!(definitions.contains(&"grand"), "{definitions:?}");
    }
}

#[test]
fn ambiguous_definition_names_the_module_and_every_file() {
    let root = TempRoot::new();
    write_design(&root);
    let second = root.write(
        "more/also_child.v",
        "module child #(parameter int W = 4) (output logic [W-1:0] y); endmodule\n",
    );
    let first = root.0.join("rtl/not_child_named.sv");
    let error = compile::compile(&opts(&root, &["tb.sv"], &["rtl", "more"]))
        .expect_err("two files declare the missing definition");
    assert_eq!(error.kind(), StartupErrorKind::InvalidArgument);
    let message = error.to_string();
    assert!(message.contains("`child`"), "{message}");
    assert!(message.contains(&admitted_name(&first)), "{message}");
    assert!(message.contains(&admitted_name(&second)), "{message}");
}

#[test]
fn definitions_already_declared_are_not_ambiguous() {
    let root = TempRoot::new();
    write_design(&root);
    // Both include directories declare `unused_top`, but nothing references
    // it; and the given sources declare `child` themselves.
    root.write("more/unused_copy.sv", "module unused_top; endmodule\n");
    root.write(
        "top_with_child.sv",
        "module top_with_child; child u(); endmodule\nmodule child; endmodule\n",
    );
    let out = compile::compile(&opts(&root, &["top_with_child.sv"], &["rtl", "more"]))
        .expect("nothing is missing");
    assert!(out.ok(), "{:?}", out.diagnostics);
    assert_eq!(out.snapshot.files.len(), 1, "{:?}", file_names(&out));
}

#[test]
fn unknown_definition_keeps_the_frontend_diagnostic() {
    let root = TempRoot::new();
    write_design(&root);
    root.write("lonely.sv", "module lonely; nowhere u(); endmodule\n");
    let out = compile::compile(&opts(&root, &["lonely.sv"], &["rtl", "more"]))
        .expect("an unresolved name is left to Slang");
    assert!(!out.ok());
    assert!(
        out.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("unknown module 'nowhere'")),
        "{:?}",
        out.diagnostics
    );
}

#[test]
fn given_sources_are_not_admitted_twice() {
    let root = TempRoot::new();
    write_design(&root);
    // The child file is both a given source and a file in the search
    // directory; its canonical identity admits it once.
    let out = compile::compile(&opts(
        &root,
        &["tb.sv", "rtl/not_child_named.sv"],
        &["rtl", "more"],
    ))
    .expect("given child plus searched grandchild");
    assert!(out.ok(), "{:?}", out.diagnostics);
    let files = file_names(&out);
    let child = admitted_name(&root.0.join("rtl/not_child_named.sv"));
    assert_eq!(files.iter().filter(|name| **name == child).count(), 1);
    assert!(files.contains(&admitted_name(&root.0.join("more/grand.sv"))));
}

#[test]
fn in_memory_compiles_do_not_search_include_directories() {
    let root = TempRoot::new();
    write_design(&root);
    let out = compile::compile_sources(
        &[compile::OwnedSource::compilation_unit(
            "tb_mem.sv",
            "module tb_mem; grand u(); endmodule\n",
        )],
        &CompileOpts {
            include_dirs: vec![root.path("more")],
            ..CompileOpts::default()
        },
    )
    .expect("in-memory compile");
    assert!(
        !out.ok(),
        "in-memory sources must not read search directories"
    );
}
