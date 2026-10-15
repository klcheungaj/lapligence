//! Detection of a statically linked VPI application.
//!
//! A VPI application normally loads at run time from `LLG_VPI_PLUGIN`. On
//! Windows a DLL cannot import `vpi_*` from the model executable, so the
//! application is linked into the model as a static archive with
//! `--dpi-lib` instead (the same route as DPI-C companions). The model must
//! then run that archive's `vlog_startup_routines` itself: the build defines
//! `LLG_VPI_STATIC_STARTUP` for the generated model when an archive names
//! the table.
//!
//! The scan reads the archive's bytes for the whole identifier, so it covers
//! the GNU/BSD `ar` and COFF archive symbol tables without an object-format
//! parser. Only archives are considered: a shared library is a run-time
//! plugin and keeps loading through `LLG_VPI_PLUGIN`. Like the PLI 1.0 scan
//! it cannot tell a definition from a reference; an archive that only
//! references the table fails at link time with an unresolved symbol.

use std::io::Read;
use std::path::Path;

const STARTUP_TABLE: &[u8] = b"vlog_startup_routines";

/// `ar` archives (GNU, BSD and the COFF `.lib` format) start with this magic.
const ARCHIVE_MAGIC: &[u8] = b"!<arch>\n";

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
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

/// Whether the library at `path` is a static archive naming
/// `vlog_startup_routines`.
pub(super) fn archive_has_vpi_startup(path: &Path) -> std::io::Result<bool> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?.read_to_end(&mut bytes)?;
    Ok(bytes.starts_with(ARCHIVE_MAGIC) && names_startup_table(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn only_archives_select_the_static_route() {
        let directory =
            std::env::temp_dir().join(format!("llg-vpi-static-{}-{}", std::process::id(), line!()));
        std::fs::create_dir_all(&directory).expect("temporary directory");
        let archive = directory.join("libapp.a");
        let shared = directory.join("libapp.so");
        std::fs::write(&archive, b"!<arch>\n/ vlog_startup_routines\0").expect("archive");
        std::fs::write(&shared, b"\x7fELF vlog_startup_routines\0").expect("shared");
        assert!(archive_has_vpi_startup(&archive).expect("read archive"));
        assert!(!archive_has_vpi_startup(&shared).expect("read shared"));
        std::fs::remove_dir_all(&directory).expect("remove temporary directory");
    }
}
