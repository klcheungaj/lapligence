//! Detection of PLI 1.0 TF/ACC use in user DPI/VPI libraries.
//!
//! `llg` ships no `veriuser.h` or `acc_user.h`, no `veriusertfs` registration
//! table and no `tf_*`/`acc_*` routines (user decision 2026-10-08). A library
//! that registers through `veriusertfs` or calls one of those routines could
//! only fail at load or run time with an unresolved symbol, so the build step
//! names the first such symbol instead.
//!
//! The scan reads the library's bytes for whole identifier runs, so it needs
//! no object-format parser and covers ELF, Mach-O, COFF and archive symbol
//! tables alike. It cannot tell a definition from a reference, and a library
//! that spells these names only in obfuscated or computed form is not
//! detected; those limits are documented in `docs/sim_features.md`.

use std::io::Read;
use std::path::Path;

/// `veriusertfs` is the PLI 1.0 registration table (IEEE 1364-2001 Annex F).
const REGISTRATION_TABLE: &str = "veriusertfs";

/// Routine names of IEEE 1364-2001 Clause 25 (`tf_*`) and Clause 26 (`acc_*`).
/// Matching whole names, not the prefixes, keeps unrelated user functions such
/// as `acc_total` from being mistaken for PLI routines.
const ROUTINES: &[&str] = &[
    "acc_append_delays",
    "acc_append_pulsere",
    "acc_close",
    "acc_collect",
    "acc_compare_handles",
    "acc_configure",
    "acc_count",
    "acc_fetch_attribute",
    "acc_fetch_attribute_int",
    "acc_fetch_attribute_str",
    "acc_fetch_defname",
    "acc_fetch_delays",
    "acc_fetch_direction",
    "acc_fetch_edge",
    "acc_fetch_fullname",
    "acc_fetch_fulltype",
    "acc_fetch_index",
    "acc_fetch_itfarg",
    "acc_fetch_itfarg_int",
    "acc_fetch_itfarg_str",
    "acc_fetch_location",
    "acc_fetch_name",
    "acc_fetch_paramtype",
    "acc_fetch_paramval",
    "acc_fetch_polarity",
    "acc_fetch_precision",
    "acc_fetch_pulsere",
    "acc_fetch_range",
    "acc_fetch_size",
    "acc_fetch_tfarg",
    "acc_fetch_tfarg_int",
    "acc_fetch_tfarg_str",
    "acc_fetch_timescale_info",
    "acc_fetch_type",
    "acc_fetch_type_str",
    "acc_fetch_value",
    "acc_free",
    "acc_handle_by_name",
    "acc_handle_calling_mod_m",
    "acc_handle_condition",
    "acc_handle_conn",
    "acc_handle_datapath",
    "acc_handle_hiconn",
    "acc_handle_interactive_scope",
    "acc_handle_itfarg",
    "acc_handle_loconn",
    "acc_handle_modpath",
    "acc_handle_notifier",
    "acc_handle_object",
    "acc_handle_parent",
    "acc_handle_path",
    "acc_handle_pathin",
    "acc_handle_pathout",
    "acc_handle_port",
    "acc_handle_scope",
    "acc_handle_simulated_net",
    "acc_handle_tchk",
    "acc_handle_tchkarg1",
    "acc_handle_tchkarg2",
    "acc_handle_terminal",
    "acc_handle_tfarg",
    "acc_handle_tfinst",
    "acc_initialize",
    "acc_next",
    "acc_next_bit",
    "acc_next_cell",
    "acc_next_cell_load",
    "acc_next_child",
    "acc_next_driver",
    "acc_next_hiconn",
    "acc_next_input",
    "acc_next_load",
    "acc_next_loconn",
    "acc_next_modpath",
    "acc_next_net",
    "acc_next_output",
    "acc_next_parameter",
    "acc_next_port",
    "acc_next_portout",
    "acc_next_primitive",
    "acc_next_scope",
    "acc_next_specparam",
    "acc_next_tchk",
    "acc_next_terminal",
    "acc_next_topmod",
    "acc_object_in_typelist",
    "acc_object_of_type",
    "acc_product_type",
    "acc_product_version",
    "acc_release_object",
    "acc_replace_delays",
    "acc_replace_pulsere",
    "acc_reset_buffer",
    "acc_set_interactive_scope",
    "acc_set_scope",
    "acc_set_value",
    "acc_vcl_add",
    "acc_vcl_delete",
    "acc_version",
    "tf_add_long",
    "tf_asynchoff",
    "tf_asynchon",
    "tf_clearalldelays",
    "tf_compare_double",
    "tf_compare_long",
    "tf_copypvc_flag",
    "tf_divide_long",
    "tf_dofinish",
    "tf_dostop",
    "tf_error",
    "tf_evaluatep",
    "tf_exprinfo",
    "tf_getcstringp",
    "tf_getinstance",
    "tf_getlongp",
    "tf_getlongtime",
    "tf_getnextlongtime",
    "tf_getp",
    "tf_getpchange",
    "tf_getrealp",
    "tf_getrealtime",
    "tf_gettime",
    "tf_gettimeprecision",
    "tf_gettimeunit",
    "tf_getworkarea",
    "tf_iasynchoff",
    "tf_iasynchon",
    "tf_iclearalldelays",
    "tf_icopypvc_flag",
    "tf_ievaluatep",
    "tf_igetcstringp",
    "tf_igetlongp",
    "tf_igetlongtime",
    "tf_igetp",
    "tf_igetpchange",
    "tf_igetrealp",
    "tf_igetrealtime",
    "tf_igettime",
    "tf_igettimeprecision",
    "tf_igettimeunit",
    "tf_igetworkarea",
    "tf_imovepvc_flag",
    "tf_inodeinfo",
    "tf_ipropagatep",
    "tf_iputlongp",
    "tf_iputp",
    "tf_iputrealp",
    "tf_irosynchronize",
    "tf_isetdelay",
    "tf_isetlongdelay",
    "tf_isetrealdelay",
    "tf_isetworkarea",
    "tf_isizep",
    "tf_ispname",
    "tf_istrdelputp",
    "tf_istrgetp",
    "tf_istrlongdelputp",
    "tf_istrrealdelputp",
    "tf_isynchronize",
    "tf_itestpvc_flag",
    "tf_itypep",
    "tf_long_to_real",
    "tf_longtime_tostr",
    "tf_message",
    "tf_mipname",
    "tf_movepvc_flag",
    "tf_multiply_long",
    "tf_nextinstance",
    "tf_nodeinfo",
    "tf_nump",
    "tf_propagatep",
    "tf_putlongp",
    "tf_putp",
    "tf_putrealp",
    "tf_read_restart",
    "tf_real_to_long",
    "tf_rosynchronize",
    "tf_scale_longdelay",
    "tf_scale_realdelay",
    "tf_setdelay",
    "tf_setlongdelay",
    "tf_setrealdelay",
    "tf_setworkarea",
    "tf_sizep",
    "tf_spname",
    "tf_strdelputp",
    "tf_strgetp",
    "tf_strlongdelputp",
    "tf_strrealdelputp",
    "tf_subtract_long",
    "tf_synchronize",
    "tf_testpvc_flag",
    "tf_text",
    "tf_typep",
    "tf_unscale_longdelay",
    "tf_unscale_realdelay",
    "tf_warning",
    "tf_write_save",
];

/// The longest PLI 1.0 symbol, plus the platform decoration stripped below.
const MAX_RUN: usize = 64;

/// Whether `name`, as written in a symbol table, is a PLI 1.0 TF/ACC symbol.
/// Mach-O prepends `_`; COFF import thunks prepend `__imp_` (and `_` on
/// 32-bit x86).
fn is_legacy_pli_symbol(name: &str) -> bool {
    let name = name.strip_prefix("__imp_").unwrap_or(name);
    let name = name.strip_prefix('_').unwrap_or(name);
    name == REGISTRATION_TABLE
        || ((name.starts_with("tf_") || name.starts_with("acc_"))
            && ROUTINES.binary_search(&name).is_ok())
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Identifier run being accumulated across read chunks. A run longer than
/// [`MAX_RUN`] cannot be a PLI 1.0 symbol and is dropped, not stored.
#[derive(Default)]
struct Run {
    bytes: Vec<u8>,
    overlong: bool,
}

impl Run {
    /// Close the current run and return it when it is a PLI 1.0 symbol.
    fn finish(&mut self) -> Option<String> {
        let found = (!self.overlong && !self.bytes.is_empty())
            .then(|| std::str::from_utf8(&self.bytes).ok())
            .flatten()
            .filter(|name| is_legacy_pli_symbol(name))
            .map(str::to_owned);
        self.bytes.clear();
        self.overlong = false;
        found
    }

    fn feed(&mut self, chunk: &[u8]) -> Option<String> {
        for byte in chunk {
            if is_identifier_byte(*byte) {
                if self.bytes.len() < MAX_RUN {
                    self.bytes.push(*byte);
                } else {
                    self.overlong = true;
                }
            } else if let Some(found) = self.finish() {
                return Some(found);
            }
        }
        None
    }
}

/// The first PLI 1.0 TF/ACC symbol named by the library at `path`.
pub(super) fn find_legacy_pli_symbol(path: &Path) -> std::io::Result<Option<String>> {
    let mut file = std::fs::File::open(path)?;
    let mut buffer = vec![0u8; 1 << 20];
    let mut run = Run::default();
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            return Ok(run.finish());
        }
        if let Some(found) = run.feed(&buffer[..read]) {
            return Ok(Some(found));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routine_table_is_sorted_for_binary_search() {
        assert!(ROUTINES.windows(2).all(|pair| pair[0] < pair[1]));
    }

    fn scan(bytes: &[u8]) -> Option<String> {
        let dir = std::env::temp_dir().join(format!("llg-legacy-pli-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = dir.join(format!(
            "{}.bin",
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::write(&path, bytes).unwrap();
        let found = find_legacy_pli_symbol(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        found
    }

    #[test]
    fn registration_table_and_routines_are_found_in_symbol_strings() {
        assert_eq!(scan(b"\0veriusertfs\0").as_deref(), Some("veriusertfs"));
        assert_eq!(scan(b"\0_tf_getp\0").as_deref(), Some("_tf_getp"));
        assert_eq!(
            scan(b"x\0__imp_acc_next_net@8\0").as_deref(),
            Some("__imp_acc_next_net")
        );
        assert_eq!(
            scan(b"junk acc_fetch_value@@V1 junk").as_deref(),
            Some("acc_fetch_value")
        );
    }

    #[test]
    fn unrelated_names_and_substrings_are_not_reported() {
        for text in [
            &b"\0acc_total\0tf_helper\0"[..],
            b"\0my_tf_getp\0",
            b"\0tf_getpx\0",
            b"\0vpi_register_systf\0veriusertfs_not\0",
            b"",
        ] {
            assert_eq!(scan(text), None, "{}", String::from_utf8_lossy(text));
        }
    }

    #[test]
    fn names_spanning_read_chunks_are_found() {
        let mut bytes = vec![b'.'; (1 << 20) - 4];
        bytes.extend_from_slice(b"tf_getp");
        bytes.push(0);
        assert_eq!(scan(&bytes).as_deref(), Some("tf_getp"));
        let mut tail = vec![b'.'; (1 << 20) - 5];
        tail.extend_from_slice(b"veriusertfs");
        assert_eq!(scan(&tail).as_deref(), Some("veriusertfs"));
    }
}
