//! Explicit diagnostics for DPI-C symbols a model link could not resolve.
//!
//! The C linker is the only component that sees every user library, so an
//! import missing from all `--dpi-lib` inputs surfaces as a link failure.
//! This module recognizes the undefined-symbol reports of the GNU, LLVM,
//! Apple and Microsoft linkers and names the DPI-C imports (from the
//! generated prototypes) and the `svdpi.h` routines that llg does not
//! provide, so the failure is not a bare toolchain transcript.

use std::collections::BTreeSet;
use std::path::Path;

/// Prefix of the comment the emitter writes before each import prototype.
const IMPORT_MARKER: &str = "/* llg DPI-C import: c_name=";

/// `svdpi.h` routines outside the implemented open-array, canonical and
/// version set: scope, user-data, caller, disable and time services
/// (context imports and exports, SIM-041) and the deprecated SV3.1a
/// representation (H.13, which an implementation need not provide).
const UNPROVIDED_ROUTINES: &[&str] = &[
    "svGetScope",
    "svSetScope",
    "svGetNameFromScope",
    "svGetScopeFromName",
    "svPutUserData",
    "svGetUserData",
    "svGetCallerInfo",
    "svIsDisabledState",
    "svAckDisabledState",
    "svGetTime",
    "svGetTimeUnit",
    "svGetTimePrecision",
    "svSizeOfBitPackedArr",
    "svSizeOfLogicPackedArr",
    "svPutBitVec32",
    "svPutLogicVec32",
    "svGetBitVec32",
    "svGetLogicVec32",
    "svGetSelectBit",
    "svGetSelectLogic",
    "svPutSelectBit",
    "svPutSelectLogic",
    "svGetPartSelectBit",
    "svGetBits",
    "svGet32Bits",
    "svGet64Bits",
    "svGetPartSelectLogic",
    "svPutPartSelectBit",
    "svPutPartSelectLogic",
    "svPutBitArrElemVec32",
    "svPutBitArrElem1Vec32",
    "svPutBitArrElem2Vec32",
    "svPutBitArrElem3Vec32",
    "svPutLogicArrElemVec32",
    "svPutLogicArrElem1Vec32",
    "svPutLogicArrElem2Vec32",
    "svPutLogicArrElem3Vec32",
    "svGetBitArrElemVec32",
    "svGetBitArrElem1Vec32",
    "svGetBitArrElem2Vec32",
    "svGetBitArrElem3Vec32",
    "svGetLogicArrElemVec32",
    "svGetLogicArrElem1Vec32",
    "svGetLogicArrElem2Vec32",
    "svGetLogicArrElem3Vec32",
];

/// Undefined DPI-C symbols named by a failed model link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MissingDpiSymbols {
    /// Imported C functions that no linked library defines.
    pub imports: Vec<String>,
    /// `svdpi.h` routines a user library calls that llg does not provide.
    pub routines: Vec<String>,
}

/// C linkage names of the imports declared by the generated sources.
pub(super) fn declared_imports(out_dir: &Path) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let Ok(entries) = std::fs::read_dir(out_dir) else {
        return names;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("c") {
            continue;
        }
        let Ok(source) = std::fs::read_to_string(&path) else {
            continue;
        };
        for line in source.lines() {
            if let Some(rest) = line.strip_prefix(IMPORT_MARKER) {
                if let Some(name) = rest.split_whitespace().next() {
                    names.insert(name.to_owned());
                }
            }
        }
    }
    names
}

/// Symbols reported undefined by a linker transcript.
fn undefined_symbols(output: &str) -> BTreeSet<String> {
    let mut symbols = BTreeSet::new();
    let identifier = |text: &str| -> Option<String> {
        let name: String = text
            .chars()
            .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
            .collect();
        (!name.is_empty()).then_some(name)
    };
    for line in output.lines() {
        // GNU ld / gold / mold: undefined reference to `name'
        if let Some(index) = line.find("undefined reference to ") {
            let rest = line[index + "undefined reference to ".len()..]
                .trim_start_matches(['`', '\'', '"']);
            symbols.extend(identifier(rest));
        }
        // LLVM lld: undefined symbol: name
        if let Some(index) = line.find("undefined symbol: ") {
            symbols.extend(identifier(&line[index + "undefined symbol: ".len()..]));
        }
        // Apple ld: "_name", referenced from:
        if line.contains("referenced from") {
            if let Some(rest) = line.trim_start().strip_prefix('"') {
                symbols.extend(identifier(rest.trim_start_matches('_')));
            }
        }
        // MSVC link: unresolved external symbol name (or _name on x86)
        if let Some(index) = line.find("unresolved external symbol ") {
            let rest = &line[index + "unresolved external symbol ".len()..];
            symbols.extend(identifier(rest.trim_start_matches(['_', '"'])));
            symbols.extend(identifier(rest.trim_start_matches('"')));
        }
    }
    symbols
}

/// Classify a failed link: `None` when it reports no DPI-related symbol.
pub(super) fn missing_dpi_symbols(out_dir: &Path, output: &str) -> Option<MissingDpiSymbols> {
    let undefined = undefined_symbols(output);
    if undefined.is_empty() {
        return None;
    }
    let declared = declared_imports(out_dir);
    let imports = undefined
        .iter()
        .filter(|symbol| declared.contains(*symbol))
        .cloned()
        .collect::<Vec<_>>();
    let routines = undefined
        .iter()
        .filter(|symbol| UNPROVIDED_ROUTINES.contains(&symbol.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    (!imports.is_empty() || !routines.is_empty()).then_some(MissingDpiSymbols { imports, routines })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linker_dialects_report_their_undefined_symbols() {
        let gnu = "model.c:(.text+0xdb): undefined reference to `nowhere'";
        let lld = "ld.lld: error: undefined symbol: nowhere";
        let apple = "  \"_nowhere\", referenced from:";
        let msvc = "model.c.obj : error LNK2019: unresolved external symbol nowhere referenced in function p";
        for transcript in [gnu, lld, apple, msvc] {
            assert!(
                undefined_symbols(transcript).contains("nowhere"),
                "{transcript}"
            );
        }
        assert!(undefined_symbols("collect2: error: ld returned 1 exit status").is_empty());
    }

    #[test]
    fn only_declared_imports_and_unprovided_routines_are_named() {
        let directory =
            std::env::temp_dir().join(format!("llg-dpi-link-{}-{}", std::process::id(), line!()));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("model.c"),
            "/* llg DPI-C import: c_name=nowhere context=0 pure=0; x */\nextern int nowhere(void);\n",
        )
        .unwrap();
        let transcript = "undefined reference to `nowhere'\nundefined reference to `svGetScope'\nundefined reference to `helper'";
        let missing = missing_dpi_symbols(&directory, transcript).unwrap();
        assert_eq!(missing.imports, vec!["nowhere".to_owned()]);
        assert_eq!(missing.routines, vec!["svGetScope".to_owned()]);
        assert!(missing_dpi_symbols(&directory, "undefined reference to `helper'").is_none());
        std::fs::remove_dir_all(&directory).unwrap();
    }
}
