//! The model executable's export table and its import library (Windows).
//!
//! A Windows DLL resolves every imported symbol when it is linked, from an
//! import library that names the module defining it. ELF and Mach-O shared
//! libraries leave the `svdpi.h`, `vpi_*` and DPI-C export calls of a
//! companion to the loader, which finds them in the model executable; a DLL
//! needs an import library for the model executable instead. The generated
//! project therefore writes [`DEF_FILE`], a module-definition file listing
//! what a companion may call: the `svdpi.h` routines llg provides, the VPI
//! routines of `vpi_user.h` and the design's `export "DPI-C"` entries.
//!
//! - MSVC links `sim.exe` with `/DEF:sim.def`, so the executable exports
//!   exactly these names.
//! - The [`IMPORT_TARGET`] target runs `lib /DEF:sim.def /NAME:sim.exe`,
//!   which writes [`IMPORT_LIBRARY`] without linking the model. The model
//!   links the companion's own import library, so the two import libraries
//!   must exist before either binary: building this one from the
//!   definition file breaks the cycle.
//! - A DLL linked against that import library binds its calls to the module
//!   named `sim.exe`. The loader matches an already loaded module by base
//!   name, so the calls reach the running model, as LLVM's plugins reach
//!   `opt.exe`. The executable must keep that name.
//!
//! The user's C code stays plain Annex H code: `svdpi.h` declares the
//! routines `__declspec(dllimport)` under MSVC, which an import library
//! satisfies, and plain `extern` declarations of exports and `vpi_*` link to
//! the import library's thunks.

use std::collections::BTreeSet;

/// Module-definition file written beside every generated model.
pub(super) const DEF_FILE: &str = "sim.def";

/// CMake target that writes the model's import library (MSVC only).
pub const IMPORT_TARGET: &str = "llg_import_library";

/// The import library [`IMPORT_TARGET`] writes, relative to the build tree.
/// It is not `sim.lib`, which the executable's own link writes there.
pub(super) const IMPORT_LIBRARY: &str = "llg_sim_import.lib";

/// Text the non-MSVC [`IMPORT_TARGET`] prints before it fails.
pub(super) const UNSUPPORTED_MARKER: &str = "llg-import-library-unsupported";

/// Prefix of the comment the emitter writes before each DPI-C export entry
/// (`emit_c/model/dpi_exports.rs`).
const EXPORT_MARKER: &str = "/* llg DPI-C export: c_name=";

/// The `svdpi.h` routines the runtime defines (`llg_dpi.c`). Every other
/// routine `svdpi.h` declares is in `dpi_link::UNPROVIDED_ROUTINES`; a unit
/// test keeps the two lists and the runtime in step, because an exported
/// name the executable does not define fails every MSVC model link.
pub(super) const PROVIDED_SVDPI_ROUTINES: &[&str] = &[
    "svAckDisabledState",
    "svDimensions",
    "svDpiVersion",
    "svGetArrElemPtr",
    "svGetArrElemPtr1",
    "svGetArrElemPtr2",
    "svGetArrElemPtr3",
    "svGetArrayPtr",
    "svGetBitArrElem",
    "svGetBitArrElem1",
    "svGetBitArrElem1VecVal",
    "svGetBitArrElem2",
    "svGetBitArrElem2VecVal",
    "svGetBitArrElem3",
    "svGetBitArrElem3VecVal",
    "svGetBitArrElemVecVal",
    "svGetBitselBit",
    "svGetBitselLogic",
    "svGetCallerInfo",
    "svGetLogicArrElem",
    "svGetLogicArrElem1",
    "svGetLogicArrElem1VecVal",
    "svGetLogicArrElem2",
    "svGetLogicArrElem2VecVal",
    "svGetLogicArrElem3",
    "svGetLogicArrElem3VecVal",
    "svGetLogicArrElemVecVal",
    "svGetNameFromScope",
    "svGetPartselBit",
    "svGetPartselLogic",
    "svGetScope",
    "svGetScopeFromName",
    "svGetUserData",
    "svHigh",
    "svIncrement",
    "svIsDisabledState",
    "svLeft",
    "svLow",
    "svPutBitArrElem",
    "svPutBitArrElem1",
    "svPutBitArrElem1VecVal",
    "svPutBitArrElem2",
    "svPutBitArrElem2VecVal",
    "svPutBitArrElem3",
    "svPutBitArrElem3VecVal",
    "svPutBitArrElemVecVal",
    "svPutBitselBit",
    "svPutBitselLogic",
    "svPutLogicArrElem",
    "svPutLogicArrElem1",
    "svPutLogicArrElem1VecVal",
    "svPutLogicArrElem2",
    "svPutLogicArrElem2VecVal",
    "svPutLogicArrElem3",
    "svPutLogicArrElem3VecVal",
    "svPutLogicArrElemVecVal",
    "svPutPartselBit",
    "svPutPartselLogic",
    "svPutUserData",
    "svRight",
    "svSetScope",
    "svSize",
    "svSizeOfArray",
];

/// The VPI routines `vpi_user.h` (the 41 of IEEE 1800-2009 Annex K) and
/// `sv_vpi_user.h` (`vpi_register_assertion_cb`, Annex M, and the
/// collection constructor `vpi_create` of IEEE P1800-2005 30.4.2) declare,
/// all defined by `llg_vpi.c`.
pub(super) const VPI_ROUTINES: &[&str] = &[
    "vpi_chk_error",
    "vpi_compare_objects",
    "vpi_control",
    "vpi_create",
    "vpi_flush",
    "vpi_free_object",
    "vpi_get",
    "vpi_get64",
    "vpi_get_cb_info",
    "vpi_get_data",
    "vpi_get_delays",
    "vpi_get_str",
    "vpi_get_systf_info",
    "vpi_get_time",
    "vpi_get_userdata",
    "vpi_get_value",
    "vpi_get_value_array",
    "vpi_get_vlog_info",
    "vpi_handle",
    "vpi_handle_by_index",
    "vpi_handle_by_multi_index",
    "vpi_handle_by_name",
    "vpi_handle_multi",
    "vpi_iterate",
    "vpi_mcd_close",
    "vpi_mcd_flush",
    "vpi_mcd_name",
    "vpi_mcd_open",
    "vpi_mcd_printf",
    "vpi_mcd_vprintf",
    "vpi_printf",
    "vpi_put_data",
    "vpi_put_delays",
    "vpi_put_userdata",
    "vpi_put_value",
    "vpi_put_value_array",
    "vpi_register_assertion_cb",
    "vpi_register_cb",
    "vpi_register_systf",
    "vpi_release_handle",
    "vpi_remove_cb",
    "vpi_scan",
    "vpi_vprintf",
];

/// MSVC-only project text: export the definition file's names from
/// `sim.exe` and add [`IMPORT_TARGET`]. Other toolchains get a target that
/// fails with [`UNSUPPORTED_MARKER`], because their shared libraries need
/// no import library. `CMAKE_AR` is `lib` (or `llvm-lib` for `clang-cl`).
pub(super) const CMAKE: &str = r#"if(MSVC)
  # sim.exe exports the routines and DPI-C exports a Windows DLL may call; see
  # src/sim/build/import_library.rs.
  target_link_options(sim PRIVATE "/DEF:${CMAKE_SOURCE_DIR}/sim.def")
  set(LLG_IMPORT_MACHINE "")
  if(CMAKE_C_COMPILER_ARCHITECTURE_ID)
    set(LLG_IMPORT_MACHINE "/MACHINE:${CMAKE_C_COMPILER_ARCHITECTURE_ID}")
  endif()
  add_custom_command(OUTPUT "${CMAKE_BINARY_DIR}/llg_sim_import.lib"
    COMMAND "${CMAKE_AR}" /nologo "/DEF:${CMAKE_SOURCE_DIR}/sim.def" /NAME:sim.exe ${LLG_IMPORT_MACHINE} "/OUT:${CMAKE_BINARY_DIR}/llg_sim_import.lib"
    DEPENDS "${CMAKE_SOURCE_DIR}/sim.def"
    VERBATIM)
  add_custom_target(llg_import_library DEPENDS "${CMAKE_BINARY_DIR}/llg_sim_import.lib")
else()
  add_custom_target(llg_import_library
    COMMAND "${CMAKE_COMMAND}" -E echo "llg-import-library-unsupported"
    COMMAND "${CMAKE_COMMAND}" -E false
    VERBATIM)
endif()
"#;

/// C names of the design's DPI-C exports, from the emitter's markers in the
/// generated sources `extra`.
pub(super) fn design_exports(extra: &[(&str, &str)]) -> BTreeSet<String> {
    extra
        .iter()
        .filter(|(name, _)| name.ends_with(".c"))
        .flat_map(|(_, source)| source.lines())
        .filter_map(|line| line.strip_prefix(EXPORT_MARKER))
        .filter_map(|rest| rest.split_whitespace().next())
        .map(str::to_owned)
        .collect()
}

/// The module-definition file for the model sources `extra`: one `EXPORTS`
/// section with the provided `svdpi.h` routines, the VPI routines and the
/// design's DPI-C exports, each once.
pub(super) fn module_definition(extra: &[(&str, &str)]) -> String {
    let mut names: BTreeSet<String> = PROVIDED_SVDPI_ROUTINES
        .iter()
        .chain(VPI_ROUTINES)
        .map(|name| (*name).to_owned())
        .collect();
    names.extend(design_exports(extra));
    let mut text = String::from(
        "; Generated by llg: the names a Windows DLL may import from sim.exe\n\
         ; (svdpi.h routines, VPI routines and the design's DPI-C exports).\n\
         EXPORTS\n",
    );
    for name in names {
        text.push_str("    ");
        text.push_str(&name);
        text.push('\n');
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whether `source` has a non-static definition of the C function `name`
    /// starting at the beginning of a line.
    fn defines(source: &str, name: &str) -> bool {
        let call = format!("{name}(");
        source.lines().any(|line| {
            !line.starts_with(char::is_whitespace)
                && !line.starts_with("static")
                && !line.trim_end().ends_with(';')
                && line
                    .split(&['*', ' '][..])
                    .any(|word| word.starts_with(&call))
        })
    }

    /// Routine names an `XXTERN` declaration of `svdpi.h` introduces.
    fn declared_svdpi_routines() -> BTreeSet<String> {
        let header = crate::sim::rt::svdpi_header();
        let mut names = BTreeSet::new();
        for statement in header.split(';') {
            let statement = statement.trim();
            let Some(rest) = statement
                .lines()
                .map(str::trim)
                .find(|line| line.starts_with("XXTERN"))
            else {
                continue;
            };
            if rest.contains("typedef") {
                continue;
            }
            if let Some(open) = rest.find('(') {
                let name = rest[..open]
                    .rsplit(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                    .next()
                    .unwrap_or_default();
                if name.starts_with("sv") {
                    names.insert(name.to_owned());
                }
            }
        }
        names
    }

    #[test]
    fn exported_routines_are_exactly_the_ones_the_runtime_defines() {
        let (vpi_header, runtime) = crate::sim::rt::vpi_sources();
        for name in PROVIDED_SVDPI_ROUTINES.iter().chain(VPI_ROUTINES) {
            assert!(defines(runtime, name), "runtime does not define {name}");
        }
        for name in super::super::dpi_link::UNPROVIDED_ROUTINES {
            assert!(!defines(runtime, name), "{name} is provided now");
            assert!(!PROVIDED_SVDPI_ROUTINES.contains(name), "{name}");
        }
        let declared = declared_svdpi_routines();
        assert!(declared.len() > 90, "svdpi.h parse found {declared:?}");
        for name in &declared {
            assert!(
                PROVIDED_SVDPI_ROUTINES.contains(&name.as_str())
                    || super::super::dpi_link::UNPROVIDED_ROUTINES.contains(&name.as_str()),
                "svdpi.h routine {name} is neither exported nor named unprovided"
            );
        }
        for name in PROVIDED_SVDPI_ROUTINES {
            assert!(declared.contains(*name), "{name} is not in svdpi.h");
        }
        let declared_vpi: BTreeSet<&str> = vpi_header
            .lines()
            .chain(crate::sim::rt::sv_vpi_header().lines())
            .filter(|line| !line.starts_with(char::is_whitespace) && !line.starts_with('#'))
            .filter_map(|line| line.split('(').next())
            .filter_map(|head| head.rsplit(&['*', ' '][..]).next())
            .filter(|name| name.starts_with("vpi_"))
            .collect();
        assert_eq!(
            declared_vpi,
            VPI_ROUTINES.iter().copied().collect::<BTreeSet<_>>()
        );
    }

    #[test]
    fn definition_lists_routines_and_design_exports_once() {
        let model = "/* llg DPI-C export: c_name=ex_add */\nint ex_add(int a);\n\
                     /* llg DPI-C export: c_name=ex_add */\n\
                     /* llg DPI-C export: c_name=ex_tick */\nint ex_tick(void);\n";
        let extra = [
            ("model.c", model),
            ("notes.h", "/* llg DPI-C export: c_name=no */"),
        ];
        assert_eq!(
            design_exports(&extra).into_iter().collect::<Vec<_>>(),
            ["ex_add", "ex_tick"]
        );
        let text = module_definition(&extra);
        let lines: Vec<&str> = text.lines().collect();
        let start = lines.iter().position(|line| *line == "EXPORTS").unwrap();
        let names: Vec<&str> = lines[start + 1..].iter().map(|line| line.trim()).collect();
        assert_eq!(
            names.len(),
            PROVIDED_SVDPI_ROUTINES.len() + VPI_ROUTINES.len() + 2
        );
        for name in ["ex_add", "ex_tick", "svGetScope", "vpi_register_cb"] {
            assert_eq!(names.iter().filter(|n| **n == name).count(), 1, "{name}");
            assert!(lines[start + 1..].contains(&format!("    {name}").as_str()));
        }
        assert!(!names.contains(&"no"));
        assert!(!names.contains(&"svGetTime"));
        assert!(lines[..start].iter().all(|line| line.starts_with(';')));
    }

    #[test]
    fn project_text_exports_and_builds_the_import_library_only_for_msvc() {
        let (msvc, other) = CMAKE.split_once("else()").unwrap();
        assert!(msvc.starts_with("if(MSVC)"));
        assert!(msvc.contains(&format!("\"/DEF:${{CMAKE_SOURCE_DIR}}/{DEF_FILE}\"")));
        assert!(msvc.contains("target_link_options(sim PRIVATE"));
        assert!(msvc.contains("/NAME:sim.exe"));
        assert!(msvc.contains("/MACHINE:${CMAKE_C_COMPILER_ARCHITECTURE_ID}"));
        assert!(msvc.contains(&format!("${{CMAKE_BINARY_DIR}}/{IMPORT_LIBRARY}")));
        assert!(msvc.contains(&format!("add_custom_target({IMPORT_TARGET} DEPENDS")));
        assert!(other.contains(&format!("add_custom_target({IMPORT_TARGET}")));
        assert!(other.contains(UNSUPPORTED_MARKER));
        assert!(other.contains("-E false"));
        assert!(!other.contains("/DEF:"));
    }
}
