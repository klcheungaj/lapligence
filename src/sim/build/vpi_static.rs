//! Detection of a statically linked VPI application.
//!
//! A VPI application normally loads at run time from `LLG_VPI_PLUGIN` (a
//! Windows DLL links against the model's import library, see
//! `import_library.rs`). It can instead be linked into the model as a static
//! archive with `--dpi-lib` (the same route as DPI-C companions). The model
//! must then run that archive's `vlog_startup_routines` itself: the build
//! defines `LLG_VPI_STATIC_STARTUP` for the generated model when an archive
//! defines the table.
//!
//! GNU/System V, BSD and MSVC COFF `.lib` archives share the `ar` container.
//! The scan reads the archive's symbol index, which lists only the global
//! symbols its members define: the System V/MSVC first linker member `/`,
//! the GNU 64-bit `/SYM64/` and the BSD `__.SYMDEF` tables. A member that is
//! a COFF short import object belongs to an import library of a DLL, which is
//! a run-time plugin and not a static application. An archive without any
//! index (no `ranlib`) falls back to a whole-identifier byte scan, which
//! cannot tell a definition from a reference; such an archive then fails at
//! link time with an unresolved symbol. Shared libraries are never selected.

use std::io::Read;
use std::path::Path;

const STARTUP_TABLE: &[u8] = b"vlog_startup_routines";

/// `ar` archives (GNU, BSD and the COFF `.lib` format) start with this magic.
const ARCHIVE_MAGIC: &[u8] = b"!<arch>\n";
const HEADER_LEN: usize = 60;

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// The table name, or the name with the single leading `_` that Mach-O and
/// 32-bit x86 COFF add to C symbols.
fn is_startup_symbol(name: &[u8]) -> bool {
    name == STARTUP_TABLE || name.strip_prefix(b"_") == Some(STARTUP_TABLE)
}

/// Whether `bytes` contain the table name as a whole identifier, allowing
/// the Mach-O `_` and 32-bit COFF `_` decorations.
fn names_startup_table(bytes: &[u8]) -> bool {
    let mut start = 0;
    while let Some(offset) = bytes[start..]
        .windows(STARTUP_TABLE.len())
        .position(|window| window == STARTUP_TABLE)
    {
        let at = start + offset;
        let end = at + STARTUP_TABLE.len();
        let before = bytes[..at]
            .iter()
            .rev()
            .take_while(|byte| is_identifier_byte(**byte))
            .count();
        let decorated = before == 0 || (before == 1 && bytes[at - 1] == b'_');
        let after_ok = bytes.get(end).is_none_or(|byte| !is_identifier_byte(*byte));
        if decorated && after_ok {
            return true;
        }
        start = at + 1;
    }
    false
}

/// One archive member: its header offset, raw name field and data.
struct Member<'a> {
    offset: usize,
    name: &'a [u8],
    data: &'a [u8],
}

fn parse_decimal(field: &[u8]) -> Option<usize> {
    std::str::from_utf8(field).ok()?.trim().parse().ok()
}

/// Members in file order; stops at the first malformed header.
fn members(bytes: &[u8]) -> Vec<Member<'_>> {
    let mut out = Vec::new();
    let mut offset = ARCHIVE_MAGIC.len();
    while offset + HEADER_LEN <= bytes.len() {
        let header = &bytes[offset..offset + HEADER_LEN];
        if &header[58..60] != b"`\n" {
            break;
        }
        let Some(size) = parse_decimal(&header[48..58]) else {
            break;
        };
        let start = offset + HEADER_LEN;
        let Some(end) = start.checked_add(size).filter(|end| *end <= bytes.len()) else {
            break;
        };
        out.push(Member {
            offset,
            name: &header[..16],
            data: &bytes[start..end],
        });
        offset = end + (size & 1);
    }
    out
}

/// Read one index word of `width` bytes at `at`.
fn word(bytes: &[u8], at: usize, width: usize, big_endian: bool) -> Option<usize> {
    let raw = bytes.get(at..at.checked_add(width)?)?;
    let mut value: u64 = 0;
    for index in 0..width {
        let byte = if big_endian {
            raw[index]
        } else {
            raw[width - 1 - index]
        };
        value = (value << 8) | u64::from(byte);
    }
    usize::try_from(value).ok()
}

fn nul_string(bytes: &[u8], at: usize) -> Option<&[u8]> {
    let tail = bytes.get(at..)?;
    let end = tail
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(tail.len());
    Some(&tail[..end])
}

/// (symbol, member header offset) pairs of a System V/MSVC `/` (32-bit
/// big-endian) or GNU `/SYM64/` (64-bit big-endian) index.
fn sysv_symbols(data: &[u8], width: usize) -> Option<Vec<(&[u8], usize)>> {
    let count = word(data, 0, width, true)?;
    let names_at = count.checked_add(1)?.checked_mul(width)?;
    if names_at > data.len() {
        return None;
    }
    let mut symbols = Vec::with_capacity(count);
    let mut cursor = names_at;
    for index in 0..count {
        let offset = word(data, (index + 1) * width, width, true)?;
        let name = nul_string(data, cursor)?;
        cursor += name.len() + 1;
        symbols.push((name, offset));
    }
    Some(symbols)
}

/// (symbol, member header offset) pairs of a BSD `__.SYMDEF` index, written
/// in the host byte order (little-endian on every supported Apple target).
fn bsd_symbols(data: &[u8], width: usize) -> Option<Vec<(&[u8], usize)>> {
    let ranlib_bytes = word(data, 0, width, false)?;
    let strings_size_at = width.checked_add(ranlib_bytes)?;
    let strings_at = strings_size_at.checked_add(width)?;
    let strings_size = word(data, strings_size_at, width, false)?;
    let strings = data.get(strings_at..strings_at.checked_add(strings_size)?)?;
    let mut symbols = Vec::new();
    for entry in (0..ranlib_bytes / (2 * width)).map(|index| width + index * 2 * width) {
        let name = nul_string(strings, word(data, entry, width, false)?)?;
        symbols.push((name, word(data, entry + width, width, false)?));
    }
    Some(symbols)
}

fn trim_name(name: &[u8]) -> &[u8] {
    let end = name
        .iter()
        .rposition(|byte| *byte != b' ')
        .map_or(0, |at| at + 1);
    &name[..end]
}

/// A member's name and payload. A BSD `#1/<len>` name is stored at the start
/// of the data.
fn name_and_payload<'a>(member: &Member<'a>) -> (&'a [u8], &'a [u8]) {
    let field = trim_name(member.name);
    if let Some(length) = field.strip_prefix(b"#1/").and_then(parse_decimal) {
        let length = length.min(member.data.len());
        let name = &member.data[..length];
        let end = name
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(name.len());
        return (&name[..end], &member.data[length..]);
    }
    (field, member.data)
}

/// A COFF short import object (`IMPORT_OBJECT_HEADER`: Sig1 0, Sig2 0xFFFF)
/// describes one export of a DLL in an MSVC import library.
fn is_import_object(data: &[u8]) -> bool {
    data.len() >= 4 && data[..4] == [0, 0, 0xff, 0xff]
}

/// The archive's symbol index, if it has one.
fn symbol_index<'a>(members: &[Member<'a>]) -> Option<Vec<(&'a [u8], usize)>> {
    let first = members.first()?;
    match name_and_payload(first) {
        (b"/", data) => sysv_symbols(data, 4),
        (b"/SYM64/", data) => sysv_symbols(data, 8),
        (b"__.SYMDEF" | b"__.SYMDEF SORTED", data) => bsd_symbols(data, 4),
        (b"__.SYMDEF_64" | b"__.SYMDEF_64 SORTED", data) => bsd_symbols(data, 8),
        _ => None,
    }
}

/// Whether an archive image defines `vlog_startup_routines` in a member that
/// is linked statically.
fn archive_defines_startup(bytes: &[u8]) -> bool {
    if !bytes.starts_with(ARCHIVE_MAGIC) {
        return false;
    }
    let members = members(bytes);
    let Some(symbols) = symbol_index(&members) else {
        return names_startup_table(bytes);
    };
    symbols
        .iter()
        .filter(|(name, _)| is_startup_symbol(name))
        .any(|(_, offset)| {
            members
                .iter()
                .find(|member| member.offset == *offset)
                .is_some_and(|member| !is_import_object(name_and_payload(member).1))
        })
}

/// Whether the library at `path` is a static archive defining
/// `vlog_startup_routines`.
pub(super) fn archive_has_vpi_startup(path: &Path) -> std::io::Result<bool> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?.read_to_end(&mut bytes)?;
    Ok(archive_defines_startup(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One `ar` member with a 16-byte name field and even padding.
    fn member(name: &str, data: &[u8]) -> Vec<u8> {
        let mut out = format!(
            "{name:<16}{:<12}{:<6}{:<6}{:<8}{:<10}`\n",
            0,
            0,
            0,
            644,
            data.len()
        )
        .into_bytes();
        out.extend_from_slice(data);
        if data.len() % 2 == 1 {
            out.push(b'\n');
        }
        out
    }

    /// An archive whose index member is built by `index` from the header
    /// offsets of `objects`, which follow it.
    fn archive(
        index_name: &str,
        objects: &[(&str, Vec<u8>)],
        index: impl Fn(&[usize]) -> Vec<u8>,
    ) -> Vec<u8> {
        // The index size does not depend on the offsets it stores.
        let placeholder = index(&vec![0; objects.len()]);
        let mut offsets = Vec::new();
        let mut offset = ARCHIVE_MAGIC.len() + member(index_name, &placeholder).len();
        for (name, data) in objects {
            offsets.push(offset);
            offset += member(name, data).len();
        }
        let mut out = ARCHIVE_MAGIC.to_vec();
        out.extend(member(index_name, &index(&offsets)));
        for (name, data) in objects {
            out.extend(member(name, data));
        }
        out
    }

    /// A System V / MSVC first linker member: `symbols[i]` in member i.
    fn sysv_index<'a>(symbols: &'a [&'a str]) -> impl Fn(&[usize]) -> Vec<u8> + 'a {
        move |offsets| {
            let mut out = (symbols.len() as u32).to_be_bytes().to_vec();
            for offset in offsets {
                out.extend((*offset as u32).to_be_bytes());
            }
            for symbol in symbols {
                out.extend(symbol.as_bytes());
                out.push(0);
            }
            out
        }
    }

    fn bsd_index<'a>(symbols: &'a [&'a str]) -> impl Fn(&[usize]) -> Vec<u8> + 'a {
        move |offsets| {
            let mut strings = Vec::new();
            let mut entries = Vec::new();
            for (symbol, offset) in symbols.iter().zip(offsets) {
                entries.extend((strings.len() as u32).to_le_bytes());
                entries.extend((*offset as u32).to_le_bytes());
                strings.extend(symbol.as_bytes());
                strings.push(0);
            }
            let mut out = (entries.len() as u32).to_le_bytes().to_vec();
            out.extend(entries);
            out.extend((strings.len() as u32).to_le_bytes());
            out.extend(strings);
            out
        }
    }

    /// An object payload whose first bytes are not an import header.
    fn object(text: &str) -> Vec<u8> {
        let mut out = b"\x64\x86\x02\x00".to_vec();
        out.extend(text.as_bytes());
        out
    }

    #[test]
    fn whole_and_decorated_names_are_found() {
        assert!(names_startup_table(b"\0vlog_startup_routines\0"));
        assert!(names_startup_table(b"\0_vlog_startup_routines\0"));
        assert!(names_startup_table(b"vlog_startup_routines"));
    }

    #[test]
    fn longer_identifiers_are_not_matches() {
        assert!(!names_startup_table(b"\0my_vlog_startup_routines\0"));
        assert!(!names_startup_table(b"\0vlog_startup_routines2\0"));
        assert!(!names_startup_table(b"\0vlog_startup\0"));
    }

    #[test]
    fn gnu_and_msvc_first_linker_members_are_read() {
        let objects = [("app.o/", object("code")), ("other.o/", object("data"))];
        let gnu = archive(
            "/",
            &objects,
            sysv_index(&["vlog_startup_routines", "helper"]),
        );
        assert!(archive_defines_startup(&gnu));
        // 32-bit x86 COFF decorates C data symbols with one underscore.
        let x86 = archive(
            "/",
            &objects,
            sysv_index(&["_vlog_startup_routines", "_helper"]),
        );
        assert!(archive_defines_startup(&x86));
        let other = archive(
            "/",
            &objects,
            sysv_index(&["my_vlog_startup_routines", "x"]),
        );
        assert!(!archive_defines_startup(&other));
    }

    #[test]
    fn an_index_ignores_references_in_member_bytes() {
        // The member mentions the table (an undefined reference), but the
        // index does not list it as a definition.
        let objects = [("user.obj/", object("\0vlog_startup_routines\0"))];
        let bytes = archive("/", &objects, sysv_index(&["user_main"]));
        assert!(!archive_defines_startup(&bytes));
    }

    #[test]
    fn msvc_import_libraries_are_dynamic() {
        // IMPORT_OBJECT_HEADER: Sig1 0, Sig2 0xFFFF, then version/machine.
        let import = b"\0\0\xff\xff\0\0\x64\x86vlog_startup_routines\0app.dll\0".to_vec();
        let objects = [("app.dll/", import)];
        let data = archive("/", &objects, sysv_index(&["__imp_vlog_startup_routines"]));
        assert!(!archive_defines_startup(&data));
        let thunk = archive("/", &objects, sysv_index(&["vlog_startup_routines"]));
        assert!(!archive_defines_startup(&thunk));
    }

    #[test]
    fn gnu_64_bit_and_bsd_indexes_are_read() {
        let objects = [("app.o/", object("code"))];
        let wide = archive("/SYM64/", &objects, |offsets: &[usize]| {
            let mut out = 1u64.to_be_bytes().to_vec();
            out.extend((offsets[0] as u64).to_be_bytes());
            out.extend(b"vlog_startup_routines\0");
            out
        });
        assert!(archive_defines_startup(&wide));
        let bsd = archive(
            "__.SYMDEF",
            &objects,
            bsd_index(&["_vlog_startup_routines"]),
        );
        assert!(archive_defines_startup(&bsd));
        let none = archive("__.SYMDEF", &objects, bsd_index(&["_main"]));
        assert!(!archive_defines_startup(&none));
    }

    #[test]
    fn bsd_long_name_index_is_read() {
        let objects = [("app.o", object("code"))];
        let bytes = archive("#1/20", &objects, |offsets: &[usize]| {
            let mut out = b"__.SYMDEF SORTED\0\0\0\0".to_vec();
            out.extend(bsd_index(&["_vlog_startup_routines"])(offsets));
            out
        });
        assert!(archive_defines_startup(&bytes));
    }

    #[test]
    fn an_archive_without_index_falls_back_to_the_byte_scan() {
        let mut bytes = ARCHIVE_MAGIC.to_vec();
        bytes.extend(member("app.o/", &object("\0vlog_startup_routines\0")));
        assert!(archive_defines_startup(&bytes));
        let mut bytes = ARCHIVE_MAGIC.to_vec();
        bytes.extend(member("app.o/", &object("\0main\0")));
        assert!(!archive_defines_startup(&bytes));
    }

    #[test]
    fn malformed_indexes_do_not_panic() {
        let mut bytes = ARCHIVE_MAGIC.to_vec();
        bytes.extend(member("/", b"\xff\xff\xff\xff"));
        assert!(!archive_defines_startup(&bytes));
        let mut bytes = ARCHIVE_MAGIC.to_vec();
        bytes.extend(member("__.SYMDEF", b"\xf0\xff\xff\xff\0\0"));
        assert!(!archive_defines_startup(&bytes));
        assert!(!archive_defines_startup(b"!<arch>\n/  truncated"));
    }

    #[test]
    fn only_archives_select_the_static_route() {
        let directory =
            std::env::temp_dir().join(format!("llg-vpi-static-{}-{}", std::process::id(), line!()));
        std::fs::create_dir_all(&directory).expect("temporary directory");
        let archive_path = directory.join("libapp.a");
        let shared = directory.join("libapp.so");
        let objects = [("app.o/", object("code"))];
        std::fs::write(
            &archive_path,
            archive("/", &objects, sysv_index(&["vlog_startup_routines"])),
        )
        .expect("archive");
        std::fs::write(&shared, b"\x7fELF vlog_startup_routines\0").expect("shared");
        assert!(archive_has_vpi_startup(&archive_path).expect("read archive"));
        assert!(!archive_has_vpi_startup(&shared).expect("read shared"));
        std::fs::remove_dir_all(&directory).expect("remove temporary directory");
    }
}
