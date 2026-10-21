//! Explicit diagnostics for DPI-C symbols a model link could not resolve.
//!
//! The C linker is the only component that sees every user library, so an
//! import missing from all `--dpi-lib` inputs surfaces as a link failure.
//! This module recognizes the undefined-symbol reports of the GNU, LLVM,
//! Apple and Microsoft linkers and names the DPI-C imports (from the
//! generated prototypes) and the `svdpi.h` routines that llg does not
//! provide, so the failure is not a bare toolchain transcript.
//!
//! Apple's ld64 does not check the undefined symbols of a dylib it links
//! against: a DPI library built with `-undefined dynamic_lookup` leaves its
//! `svdpi.h` and export calls to dyld, which binds a missing one to an abort
//! that fires, without a message on stderr, when the model calls it (or, for
//! a call the run never makes, not at all). So before a build,
//! [`missing_mach_o_symbols`] reads each Mach-O library's symbol table and
//! names the unprovided routines and the undefined symbols it imports, as the
//! ELF link does.

use std::collections::BTreeSet;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Prefix of the comment the emitter writes before each import prototype.
const IMPORT_MARKER: &str = "/* llg DPI-C import: c_name=";

/// `svdpi.h` routines outside the implemented open-array, canonical,
/// version, scope, user-data, caller and disable set: the time services and
/// the deprecated SV3.1a representation (H.13, which an implementation need
/// not provide).
pub(super) const UNPROVIDED_ROUTINES: &[&str] = &[
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

/// Prefixes of symbols the generated model and runtime define themselves; an
/// undefined one is an llg defect, never attributed to a user library.
const RUNTIME_PREFIXES: &[&str] = &["llg_", "LLG_", "sv4_", "vpi_"];

/// Undefined DPI-C symbols named by a failed model link (or, for Mach-O
/// libraries, by their symbol tables before the build).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MissingDpiSymbols {
    /// Imported C functions that no linked library defines.
    pub imports: Vec<String>,
    /// `svdpi.h` routines a user library calls that llg does not provide.
    pub routines: Vec<String>,
    /// Other symbols a user library calls that nothing defines: with the
    /// model and runtime complete, these are exports the design does not
    /// declare (`export "DPI-C"`) and helpers missing from every library.
    pub unresolved: Vec<String>,
    /// Evidence from the Mach-O symbol tables; empty for a link failure,
    /// whose transcript is the evidence.
    pub output: String,
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

/// C linkage names of the imports declared by the generated model sources
/// `extra` (name, text), as [`declared_imports`] reads them from disk.
pub(super) fn declared_imports_in(extra: &[(&str, &str)]) -> BTreeSet<String> {
    extra
        .iter()
        .filter(|(name, _)| name.ends_with(".c"))
        .flat_map(|(_, source)| source.lines())
        .filter_map(|line| line.strip_prefix(IMPORT_MARKER))
        .filter_map(|rest| rest.split_whitespace().next())
        .map(str::to_owned)
        .collect()
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
/// `user_libraries` says whether `--dpi-lib` libraries took part; only then
/// is an otherwise unknown undefined symbol attributed to them.
pub(super) fn missing_dpi_symbols(
    out_dir: &Path,
    output: &str,
    user_libraries: bool,
) -> Option<MissingDpiSymbols> {
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
    let unresolved = if user_libraries {
        undefined
            .iter()
            .filter(|symbol| {
                !declared.contains(*symbol)
                    && !UNPROVIDED_ROUTINES.contains(&symbol.as_str())
                    && !RUNTIME_PREFIXES
                        .iter()
                        .any(|prefix| symbol.starts_with(prefix))
            })
            .cloned()
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    (!imports.is_empty() || !routines.is_empty() || !unresolved.is_empty()).then_some(
        MissingDpiSymbols {
            imports,
            routines,
            unresolved,
            output: String::new(),
        },
    )
}

/// What the Mach-O `--dpi-lib` libraries leave to dyld that nothing will
/// provide, found before the build because ld64 does not check it:
///
/// - `routines`: unprovided `svdpi.h` routines any Mach-O library imports;
/// - `unresolved`: other imports that neither `provided` (what the model
///   defines for its libraries: the `svdpi.h` and VPI routines, the design's
///   DPI-C exports and imports) nor any library defines, such as an export
///   the design does not declare. Only judged when every library is a
///   readable Mach-O file, because the definitions of another format (a
///   static archive, for example) are not read.
///
/// An import is one without a dependent dylib (library ordinal 0, 0xfe for
/// `-undefined dynamic_lookup`, or 0xff); a call into libSystem names it and
/// is never counted. Weak imports are skipped: dyld binds a missing one to
/// null. A relocatable object's undefined symbols (ordinal 0, libSystem's
/// included) are linked into the model, whose link checks them, so they
/// count only as routines. `None` when nothing is missing; otherwise `output` names each
/// library and the symbols it imports.
pub(super) fn missing_mach_o_symbols(
    libraries: &[impl AsRef<Path>],
    provided: &BTreeSet<String>,
) -> Option<MissingDpiSymbols> {
    let mut all_mach_o = true;
    let mut scanned = Vec::new();
    let mut definitions = BTreeSet::new();
    for path in libraries {
        let path = path.as_ref();
        let symbols = std::fs::File::open(path)
            .ok()
            .and_then(|mut file| mach_o_symbols(&mut file));
        match symbols {
            Some(symbols) => {
                definitions.extend(symbols.definitions);
                scanned.push((path, symbols.imports, symbols.relocatable));
            }
            None => all_mach_o = false,
        }
    }
    let mut routines = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut output = String::new();
    for (path, imports, relocatable) in &scanned {
        let mut named = Vec::new();
        for import in imports {
            if UNPROVIDED_ROUTINES.contains(&import.as_str()) {
                routines.insert(import.clone());
            } else if all_mach_o
                && !relocatable
                && !provided.contains(import)
                && !definitions.contains(import)
                && !RUNTIME_PREFIXES
                    .iter()
                    .any(|prefix| import.starts_with(prefix))
            {
                unresolved.insert(import.clone());
            } else {
                continue;
            }
            named.push(import.as_str());
        }
        if !named.is_empty() {
            output.push_str(&format!(
                "{} imports {} for dyld to resolve when the model runs; ld64 does not check \
                 a library's undefined symbols at link time\n",
                path.display(),
                named.join(", ")
            ));
        }
    }
    (!routines.is_empty() || !unresolved.is_empty()).then(|| MissingDpiSymbols {
        imports: Vec::new(),
        routines: routines.into_iter().collect(),
        unresolved: unresolved.into_iter().collect(),
        output,
    })
}

const MH_MAGIC_64: u32 = 0xfeed_facf;
/// `mach_header_64.filetype` of a relocatable object.
const MH_OBJECT: u32 = 0x1;
const FAT_MAGIC: u32 = 0xcafe_babe;
const FAT_MAGIC_64: u32 = 0xcafe_babf;
const LC_SYMTAB: u32 = 0x2;
/// `<mach-o/nlist.h>` type bits.
const N_STAB: u8 = 0xe0;
const N_TYPE: u8 = 0x0e;
const N_EXT: u8 = 0x01;
const N_UNDF: u8 = 0x0;
const N_ABS: u8 = 0x2;
const N_INDR: u8 = 0xa;
const N_SECT: u8 = 0xe;
/// `n_desc` bit of a weak import, which dyld binds to null when missing.
const N_WEAK_REF: u8 = 0x40;
/// Library ordinals 1 to 0xfd name a dependent dylib; 0 (own image or flat
/// namespace), 0xfe (`-undefined dynamic_lookup`) and 0xff (the main
/// executable) leave the lookup to the loading process.
const MAX_DEPENDENT_LIBRARY_ORDINAL: u16 = 0xfd;
/// Slices of a universal file; real ones have a handful. A larger count is
/// another format with the same magic (a Java class file).
const MAX_FAT_SLICES: u32 = 32;

/// External symbols of a Mach-O file, by name without the leading `_`.
#[derive(Debug, Default)]
struct MachOSymbols {
    /// Undefined, non-weak symbols left to the loading process.
    imports: BTreeSet<String>,
    /// Symbols the file defines for other images (section, absolute,
    /// indirect and common symbols).
    definitions: BTreeSet<String>,
    /// A slice is a relocatable object (`MH_OBJECT`): its undefined symbols
    /// carry no library ordinals, and ld64 resolves and checks them against
    /// everything the model links, libSystem included.
    relocatable: bool,
}

impl MachOSymbols {
    fn extend(&mut self, other: MachOSymbols) {
        self.imports.extend(other.imports);
        self.definitions.extend(other.definitions);
        self.relocatable |= other.relocatable;
    }
}

/// [`MachOSymbols`] of every 64-bit slice. `None` when the file is not a
/// 64-bit Mach-O or universal file or is malformed.
fn mach_o_symbols(file: &mut (impl Read + Seek)) -> Option<MachOSymbols> {
    let length = file.seek(SeekFrom::End(0)).ok()?;
    let magic = read_at(file, 0, 4, length)?;
    let mut imports = MachOSymbols::default();
    match u32::from_be_bytes(magic.clone().try_into().ok()?) {
        fat @ (FAT_MAGIC | FAT_MAGIC_64) => {
            let count = be_u32(&read_at(file, 4, 4, length)?, 0)?;
            if count > MAX_FAT_SLICES {
                return None;
            }
            let entry = if fat == FAT_MAGIC { 20 } else { 32 };
            let table = read_at(file, 8, entry * count as usize, length)?;
            for slice in table.chunks_exact(entry) {
                let offset = if fat == FAT_MAGIC {
                    u64::from(be_u32(slice, 8)?)
                } else {
                    u64::from_be_bytes(slice.get(8..16)?.try_into().ok()?)
                };
                // A 32-bit or malformed slice cannot run on a supported host.
                imports.extend(thin_symbols(file, offset, length).unwrap_or_default());
            }
        }
        _ if u32::from_le_bytes(magic.try_into().ok()?) == MH_MAGIC_64 => {
            imports = thin_symbols(file, 0, length)?;
        }
        _ => return None,
    }
    Some(imports)
}

/// [`mach_o_symbols`] for the little-endian 64-bit image at `base`.
fn thin_symbols(file: &mut (impl Read + Seek), base: u64, length: u64) -> Option<MachOSymbols> {
    let header = read_at(file, base, 32, length)?;
    if le_u32(&header, 0)? != MH_MAGIC_64 {
        return None;
    }
    let relocatable = le_u32(&header, 12)? == MH_OBJECT;
    let commands = read_at(file, base + 32, le_u32(&header, 20)? as usize, length)?;
    let mut at = 0usize;
    for _ in 0..le_u32(&header, 16)? {
        let command = le_u32(&commands, at)?;
        let size = le_u32(&commands, at + 4)? as usize;
        if command == LC_SYMTAB {
            let (symbols, count) = (le_u32(&commands, at + 8)?, le_u32(&commands, at + 12)?);
            let (strings, strings_size) =
                (le_u32(&commands, at + 16)?, le_u32(&commands, at + 20)?);
            let table = read_at(file, base + u64::from(symbols), 16 * count as usize, length)?;
            let names = read_at(
                file,
                base + u64::from(strings),
                strings_size as usize,
                length,
            )?;
            let mut found = MachOSymbols {
                relocatable,
                ..MachOSymbols::default()
            };
            for symbol in table.as_chunks::<16>().0 {
                let kind = symbol[4];
                let ordinal = u16::from_le_bytes([symbol[6], symbol[7]]) >> 8;
                let value = u64::from_le_bytes(symbol[8..16].try_into().ok()?);
                if kind & N_STAB != 0 || kind & N_EXT == 0 {
                    continue;
                }
                // An undefined external with a value is a common symbol.
                let set = match kind & N_TYPE {
                    N_UNDF if value != 0 => &mut found.definitions,
                    N_UNDF
                        if symbol[6] & N_WEAK_REF != 0
                            || (1..=MAX_DEPENDENT_LIBRARY_ORDINAL).contains(&ordinal) =>
                    {
                        continue;
                    }
                    N_UNDF => &mut found.imports,
                    N_SECT | N_ABS | N_INDR => &mut found.definitions,
                    _ => continue,
                };
                let start = le_u32(symbol, 0)? as usize;
                let name = names.get(start..)?;
                let name = &name[..name.iter().position(|&byte| byte == 0)?];
                if let Some(name) = name.strip_prefix(b"_") {
                    set.insert(String::from_utf8_lossy(name).into_owned());
                }
            }
            return Some(found);
        }
        if size < 8 {
            return None;
        }
        at += size;
    }
    Some(MachOSymbols {
        relocatable,
        ..MachOSymbols::default()
    })
}

/// `count` bytes at `offset`, refused when they would run past `length` (a
/// malformed header must not allocate more than the file holds).
fn read_at(
    file: &mut (impl Read + Seek),
    offset: u64,
    count: usize,
    length: u64,
) -> Option<Vec<u8>> {
    if offset.checked_add(count as u64)? > length {
        return None;
    }
    file.seek(SeekFrom::Start(offset)).ok()?;
    let mut bytes = vec![0; count];
    file.read_exact(&mut bytes).ok()?;
    Some(bytes)
}

fn le_u32(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn be_u32(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal 64-bit Mach-O image with one `LC_SYMTAB` holding `symbols`
    /// as (name with `_`, type, library ordinal, value).
    fn mach_o(symbols: &[(&str, u8, u16, u64)]) -> Vec<u8> {
        let described = symbols
            .iter()
            .map(|(name, kind, ordinal, value)| (*name, *kind, ordinal << 8, *value))
            .collect::<Vec<_>>();
        mach_o_described(&described)
    }

    /// [`mach_o`] with each symbol's whole `n_desc` (library ordinal in the
    /// high byte, flags such as `N_WEAK_REF` in the low byte).
    fn mach_o_described(symbols: &[(&str, u8, u16, u64)]) -> Vec<u8> {
        let mut strings = vec![0u8];
        let mut table = Vec::new();
        for (name, kind, desc, value) in symbols {
            table.extend_from_slice(&(strings.len() as u32).to_le_bytes());
            table.extend_from_slice(&[*kind, 0]);
            table.extend_from_slice(&desc.to_le_bytes());
            table.extend_from_slice(&value.to_le_bytes());
            strings.extend_from_slice(name.as_bytes());
            strings.push(0);
        }
        let symbols_at = 32 + 24;
        let strings_at = symbols_at + table.len();
        let mut image = Vec::new();
        for word in [MH_MAGIC_64, 0x0100_000c, 0, 6, 1, 24, 0x85, 0] {
            image.extend_from_slice(&word.to_le_bytes());
        }
        for word in [
            LC_SYMTAB,
            24,
            symbols_at as u32,
            symbols.len() as u32,
            strings_at as u32,
            strings.len() as u32,
        ] {
            image.extend_from_slice(&word.to_le_bytes());
        }
        image.extend_from_slice(&table);
        image.extend_from_slice(&strings);
        image
    }

    fn imports(bytes: &[u8]) -> Option<BTreeSet<String>> {
        mach_o_symbols(&mut std::io::Cursor::new(bytes)).map(|symbols| symbols.imports)
    }

    /// Only undefined externals left to the loading process count: not
    /// definitions, debugging entries, common symbols or imports bound to a
    /// dependent dylib (ordinal 1, for example libSystem).
    #[test]
    fn mach_o_imports_are_the_lookups_left_to_the_loading_process() {
        let image = mach_o(&[
            ("_svGetScope", N_EXT, 0xfe, 0),
            ("_svGetUserData", N_EXT, 0, 0),
            ("_svGetTime", N_EXT, 0xff, 0),
            ("_printf", N_EXT, 1, 0),
            ("_neg_scope", N_EXT | 0x0e, 1, 0x3f80),
            ("_common", N_EXT, 0, 8),
            ("_stab", 0x24, 0, 0),
            ("_local", 0, 0, 0),
        ]);
        let found = imports(&image).expect("a Mach-O image");
        assert_eq!(
            found.into_iter().collect::<Vec<_>>(),
            ["svGetScope", "svGetTime", "svGetUserData"]
        );
    }

    #[test]
    fn universal_files_report_every_slice_and_other_formats_none() {
        let first = mach_o(&[("_svGetScope", N_EXT, 0xfe, 0)]);
        let second = mach_o(&[("_svSetScope", N_EXT, 0xfe, 0)]);
        let mut fat = Vec::new();
        fat.extend_from_slice(&FAT_MAGIC.to_be_bytes());
        fat.extend_from_slice(&2u32.to_be_bytes());
        let first_at = 64u32;
        let second_at = first_at + first.len() as u32;
        for (offset, size) in [(first_at, first.len()), (second_at, second.len())] {
            for word in [0x0100_000c, 0, offset, size as u32, 0] {
                fat.extend_from_slice(&word.to_be_bytes());
            }
        }
        fat.resize(first_at as usize, 0);
        fat.extend_from_slice(&first);
        fat.extend_from_slice(&second);
        assert_eq!(
            imports(&fat).unwrap().into_iter().collect::<Vec<_>>(),
            ["svGetScope", "svSetScope"]
        );
        assert!(imports(b"\x7fELF\x02\x01\x01\0\0\0\0\0").is_none());
        assert!(imports(b"MZ").is_none());
        let mut truncated = first.clone();
        truncated.truncate(first.len() - 4);
        assert!(imports(&truncated).is_none());
        let mut huge = first;
        huge[20..24].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(imports(&huge).is_none());
    }

    /// A temporary directory holding `files` (name, contents).
    fn library_directory(line: u32, files: &[(&str, &[u8])]) -> std::path::PathBuf {
        let directory =
            std::env::temp_dir().join(format!("llg-dpi-macho-{}-{line}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        for (name, contents) in files {
            std::fs::write(directory.join(name), contents).unwrap();
        }
        directory
    }

    fn provided(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn only_unprovided_routines_of_a_mach_o_library_are_named() {
        let library = mach_o(&[
            ("_svGetTime", N_EXT, 0xfe, 0),
            ("_svGetArrayPtr", N_EXT, 0xfe, 0),
        ]);
        let directory = library_directory(line!(), &[("libneg.dylib", &library)]);
        let missing = missing_mach_o_symbols(
            &[directory.join("libneg.dylib")],
            &provided(&["svGetArrayPtr"]),
        )
        .expect("svGetTime is unprovided");
        assert_eq!(missing.routines, ["svGetTime"]);
        assert!(missing.imports.is_empty() && missing.unresolved.is_empty());
        assert!(
            missing
                .output
                .contains("libneg.dylib imports svGetTime for dyld"),
            "{}",
            missing.output
        );
        std::fs::remove_dir_all(&directory).unwrap();
    }

    /// The macOS form of an export the design does not declare (CI round 6,
    /// `sim_041::library_call_to_an_undeclared_export_is_named_at_link`):
    /// named unless the model or another library defines it. libSystem
    /// calls, weak imports and runtime names are never counted.
    #[test]
    fn undefined_imports_of_mach_o_libraries_are_named_unless_something_defines_them() {
        const LOOKUP: u16 = 0xfe << 8;
        let errors = mach_o_described(&[
            ("_err_scope", N_EXT | N_SECT, 0, 0x3f80),
            ("_err_inc", N_EXT, LOOKUP, 0),
            ("_err_kill", N_EXT, LOOKUP, 0),
            ("_err_tick", N_EXT, 0, 0),
            ("_helper", N_EXT, 0xff << 8, 0),
            ("_svSetScope", N_EXT, LOOKUP, 0),
            ("_printf", N_EXT, 1 << 8, 0),
            ("_maybe", N_EXT, LOOKUP | u16::from(N_WEAK_REF), 0),
            ("_llg_internal", N_EXT, LOOKUP, 0),
        ]);
        let helpers = mach_o(&[("_helper", N_EXT | N_SECT, 0, 0x10)]);
        let directory = library_directory(
            line!(),
            &[
                ("liberrors.dylib", &errors),
                ("libhelpers.dylib", &helpers),
                ("libother.a", b"!<arch>\n"),
            ],
        );
        let model = provided(&["err_inc", "svSetScope"]);
        let missing = missing_mach_o_symbols(
            &[
                directory.join("liberrors.dylib"),
                directory.join("libhelpers.dylib"),
            ],
            &model,
        )
        .expect("err_kill and err_tick are undefined");
        assert_eq!(missing.unresolved, ["err_kill", "err_tick"]);
        assert!(missing.routines.is_empty() && missing.imports.is_empty());
        assert!(
            missing
                .output
                .contains("liberrors.dylib imports err_kill, err_tick for dyld"),
            "{}",
            missing.output
        );
        // Without the library that defines `helper`, it is named too.
        let alone = missing_mach_o_symbols(&[directory.join("liberrors.dylib")], &model).unwrap();
        assert_eq!(alone.unresolved, ["err_kill", "err_tick", "helper"]);
        // A library whose definitions are not read (another format, a
        // missing file) withholds the judgement; routines are still named.
        for other in ["libother.a", "absent.dylib"] {
            assert!(
                missing_mach_o_symbols(
                    &[directory.join("liberrors.dylib"), directory.join(other)],
                    &model
                )
                .is_none(),
                "{other}"
            );
        }
        let elf = directory.join("libneg.so");
        std::fs::write(&elf, b"\x7fELF svGetTime").unwrap();
        assert!(missing_mach_o_symbols(&[&elf], &model).is_none());
        // A relocatable object (`MH_OBJECT`) leaves every undefined symbol
        // at ordinal 0, libc's too; the model link checks them, so only an
        // unprovided routine is named before it.
        let mut object = mach_o(&[
            ("_printf", N_EXT, 0, 0),
            ("_err_kill", N_EXT, 0, 0),
            ("_svGetTime", N_EXT, 0, 0),
        ]);
        object[12..16].copy_from_slice(&MH_OBJECT.to_le_bytes());
        let object_path = directory.join("errors.o");
        std::fs::write(&object_path, &object).unwrap();
        let missing = missing_mach_o_symbols(&[&object_path], &model).unwrap();
        assert_eq!(missing.routines, ["svGetTime"]);
        assert!(missing.unresolved.is_empty(), "{missing:?}");
        std::fs::remove_dir_all(&directory).unwrap();
    }

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
        let transcript = "undefined reference to `nowhere'\nundefined reference to `svGetTime'\nundefined reference to `helper'";
        let missing = missing_dpi_symbols(&directory, transcript, false).unwrap();
        assert_eq!(missing.imports, vec!["nowhere".to_owned()]);
        assert_eq!(missing.routines, vec!["svGetTime".to_owned()]);
        assert!(missing.unresolved.is_empty());
        assert!(
            missing_dpi_symbols(&directory, "undefined reference to `helper'", false).is_none()
        );
        // With user libraries linked, an unknown symbol is one of theirs
        // (for example an export the design does not declare); runtime
        // symbols stay unattributed.
        let missing = missing_dpi_symbols(&directory, transcript, true).unwrap();
        assert_eq!(missing.unresolved, vec!["helper".to_owned()]);
        assert!(missing_dpi_symbols(&directory, "undefined reference to `llg_x'", true).is_none());
        std::fs::remove_dir_all(&directory).unwrap();
    }
}
