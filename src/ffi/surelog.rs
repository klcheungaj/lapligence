use std::ffi::{CStr, CString};
use std::marker::PhantomData;
use std::os::raw::{c_char, c_int, c_uint, c_void};

// ── Raw FFI ──────────────────────────────────────────────────────────────────

#[link(name = "surelog_c_wrapper", kind = "static")]
unsafe extern "C" {
    // SymbolTable
    fn sl_create_symbol_table() -> *mut c_void;
    fn sl_free_symbol_table(st: *mut c_void);

    // ErrorContainer
    fn sl_create_error_container(st: *mut c_void) -> *mut c_void;
    fn sl_free_error_container(ec: *mut c_void);
    fn sl_errors_print_messages(ec: *mut c_void, mute: c_int);
    fn sl_errors_get_fatal_count(ec: *mut c_void) -> c_uint;
    fn sl_errors_get_syntax_count(ec: *mut c_void) -> c_uint;
    fn sl_errors_get_error_count(ec: *mut c_void) -> c_uint;
    fn sl_errors_get_count(ec: *mut c_void) -> c_uint;
    fn sl_errors_get_item(ec: *mut c_void, index: c_uint, out: *mut SLDiag) -> c_int;

    // CommandLineParser
    fn sl_create_command_line_parser(ec: *mut c_void, st: *mut c_void) -> *mut c_void;
    fn sl_free_command_line_parser(clp: *mut c_void);
    fn sl_clp_no_python(clp: *mut c_void);
    fn sl_clp_set_parse(clp: *mut c_void);
    fn sl_clp_set_write_pp_output(clp: *mut c_void);
    fn sl_clp_set_compile(clp: *mut c_void);
    fn sl_clp_set_elaborate(clp: *mut c_void);
    fn sl_clp_set_elab_uhdm(clp: *mut c_void);
    fn sl_clp_set_mute_stdout(clp: *mut c_void);
    fn sl_clp_parse_command_line(
        clp: *mut c_void,
        argc: c_int,
        argv: *const *const c_char,
    ) -> c_int;
    fn sl_clp_help(clp: *mut c_void) -> c_int;
    fn sl_clp_mute_stdout(clp: *mut c_void) -> c_int;

    // Compiler
    fn sl_start_compiler(clp: *mut c_void) -> *mut c_void;
    fn sl_get_design(compiler: *mut c_void) -> *mut c_void;
    fn sl_get_uhdm_design(compiler: *mut c_void) -> *mut u32; // VpiHandle
    fn sl_uhdm_elaborate(vpi_design: *mut u32);
    fn sl_shutdown_compiler(compiler: *mut c_void);

    // Design
    fn sl_design_get_top_instance_count(design: *mut c_void) -> c_uint;
    fn sl_design_get_top_instance(design: *mut c_void, i: c_uint) -> *mut c_void;

    // ModuleInstance
    fn sl_instance_get_full_path_name(inst: *mut c_void) -> *mut c_char;
    fn sl_instance_get_file_path(inst: *mut c_void) -> *mut c_char;
    fn sl_instance_get_child_count(inst: *mut c_void) -> c_uint;
    fn sl_instance_get_child(inst: *mut c_void, i: c_uint) -> *mut c_void;

    // File Content
    fn sl_design_get_file_content_count(design: *mut c_void) -> c_uint;
    fn sl_design_get_file_content(design: *mut c_void, i: c_uint) -> *mut c_void;
    fn sl_file_content_get_path(fc: *mut c_void) -> *mut c_char;
    fn sl_file_content_get_file_id(fc: *mut c_void) -> c_uint;
    fn sl_file_content_get_node_count(fc: *mut c_void) -> c_uint;
    fn sl_file_content_get_node(fc: *mut c_void, index: c_uint, out: *mut SLRawNodeInfo) -> c_int;

    // Memory
    fn sl_free_string(s: *mut c_char);
}

// ── C struct layout matching SL_VObjectInfo ─────────────────────────────────────────
//
// Must mirror the C header exactly.  The two padding arrays fill the gaps that
// the C ABI inserts between fields of different sizes.
//   offset 0 : u32  line
//   offset 4 : u16  col
//   offset 6 : [u8;2] padding
//   offset 8 : u32  end_line
//   offset 12: u16  end_col
//   offset 14: u16  type_id
//   offset 16: u32  file_id
//   offset 20: u32  parent_index
//   offset 24: u32  child_index
//   offset 28: u32  sibling_index
//   offset 32: *const c_char  type_name   (static string literal)
//   offset 40: *const c_char  symbol_name (SymbolTable intern; may be null)
//   total 48 bytes on 64-bit
#[repr(C)]
pub(crate) struct SLRawNodeInfo {
    pub line: u32,
    pub col: u16,
    pub _pad1: [u8; 2],
    pub end_line: u32,
    pub end_col: u16,
    pub type_id: u16,               // raw VObjectType discriminant (uint16_t)
    pub file_id: u32,               // RawPathId of originating source file
    pub parent_index: u32,          // NodeId of parent; 0 = no parent (InvalidRawNodeId)
    pub child_index: u32,           // first child NodeId; 0 = no child
    pub sibling_index: u32,         // next sibling NodeId; 0 = no sibling
    pub type_name: *const c_char,   // static string literal — never free
    pub symbol_name: *const c_char, // SymbolTable intern  — never free; may be null
}

// SAFETY: SLRawNodeInfo only contains plain data (integers + read-only C
// string pointers to static/interned storage).  It is never mutated after
// the FFI call that fills it, so Send + Sync hold.
unsafe impl Send for SLRawNodeInfo {}
unsafe impl Sync for SLRawNodeInfo {}

// ── Structured diagnostics (mirrors SL_Diag in surelog_c_api.h) ──────────────

/// C layout of `SL_Diag`.  `file` and `message` are malloc'd by the C++ side
/// and must be freed with `sl_free_string`.
#[repr(C)]
struct SLDiag {
    severity: c_int,
    file: *mut c_char,
    line: c_uint,
    col: c_uint,
    message: *mut c_char,
}

/// Diagnostic severity, mirroring `ErrorDefinition::ErrorSeverity`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Fatal,
    Syntax,
    Error,
    Warning,
    Note,
    Info,
}

/// One structured compile diagnostic, copied out of the session's
/// `ErrorContainer` so it stays valid after the session is dropped.
#[derive(Debug, Clone, PartialEq)]
pub struct Diag {
    pub severity: Severity,
    /// Source file path; `None` when Surelog has no path for the entry.
    pub file: Option<String>,
    /// 1-based source line; 0 when unknown.
    pub line: u32,
    /// 1-based source column; 0 when unknown.
    pub col: u32,
    /// Message text with the first `%s` placeholder already substituted.
    pub message: String,
}

fn severity_of(v: c_int) -> Severity {
    match v {
        0 => Severity::Fatal,
        1 => Severity::Syntax,
        2 => Severity::Error,
        3 => Severity::Warning,
        4 => Severity::Info,
        _ => Severity::Note,
    }
}

// ── Session ───────────────────────────────────────────────────────────────────

/// Owns all Surelog objects for one parse session.
///
/// Struct fields drop in **declaration order** (top to bottom), which is the
/// reverse of construction order — exactly the correct sequence:
/// `compiler → clp → errors → symbol_table`.
///
/// Callers obtain borrowed handles (`Design<'_>`, `ModuleInstance<'_>`, …)
/// from `&SurelogSession` methods.  The borrow checker ensures those handles
/// cannot outlive the session.
pub struct SurelogSession {
    // Drop order: compiler first, then clp, then errors, then symbol_table.
    compiler: *mut c_void,
    clp: *mut c_void,
    errors: *mut c_void,
    symbol_table: *mut c_void,
    /// `true` if `sl_start_compiler` was called successfully.
    compiled: bool,
}

impl Drop for SurelogSession {
    fn drop(&mut self) {
        unsafe {
            if self.compiled {
                sl_shutdown_compiler(self.compiler);
            }
            sl_free_command_line_parser(self.clp);
            sl_free_error_container(self.errors);
            sl_free_symbol_table(self.symbol_table);
        }
    }
}

impl SurelogSession {
    /// Build a session from a slice of string arguments (including the program
    /// name as `args[0]`).  `no_python` is set automatically; callers control
    /// the remaining flags through `args`.
    ///
    /// Returns `None` if any argument contains an interior null byte, or if
    /// the C++ side fails to initialise any object.
    pub fn new(args: &[&str]) -> Option<Self> {
        // Convert every argument; bail if any contains an interior null byte.
        let owned: Option<Vec<CString>> = args.iter().map(|s| CString::new(*s).ok()).collect();
        let owned = owned?;

        // SAFETY: the raw pointers are created here and handed to
        // `finish_session`, which owns them from that point on and frees them
        // on any failure.
        unsafe {
            let symbol_table = sl_create_symbol_table();
            if symbol_table.is_null() {
                return None;
            }
            let errors = sl_create_error_container(symbol_table);
            if errors.is_null() {
                sl_free_symbol_table(symbol_table);
                return None;
            }
            let clp = sl_create_command_line_parser(errors, symbol_table);
            if clp.is_null() {
                sl_free_error_container(errors);
                sl_free_symbol_table(symbol_table);
                return None;
            }
            sl_clp_no_python(clp);
            finish_session(symbol_table, errors, clp, &owned)
        }
    }

    // ── Errors ────────────────────────────────────────────────────────────────

    pub fn fatal_count(&self) -> u32 {
        unsafe { sl_errors_get_fatal_count(self.errors) }
    }
    pub fn syntax_count(&self) -> u32 {
        unsafe { sl_errors_get_syntax_count(self.errors) }
    }
    pub fn error_count(&self) -> u32 {
        unsafe { sl_errors_get_error_count(self.errors) }
    }

    /// Print (or suppress) all accumulated error messages.
    pub fn print_messages(&self, mute: bool) {
        unsafe {
            sl_errors_print_messages(self.errors, mute as c_int);
        }
    }

    /// Collect structured diagnostics from the session's ErrorContainer.
    ///
    /// Every entry is copied into an owned `Diag`, so the returned vector
    /// remains valid after the session is dropped.
    pub fn diagnostics(&self) -> Vec<Diag> {
        let n = unsafe { sl_errors_get_count(self.errors) };
        let mut out = Vec::with_capacity(n as usize);
        for i in 0..n {
            // Zero-initialise so padding bytes are defined and nothing is read
            // as a pointer if the C++ side fails to fill `raw`.
            let mut raw: SLDiag = unsafe { std::mem::zeroed() };
            let ok = unsafe { sl_errors_get_item(self.errors, i, &mut raw) };
            if ok == 0 {
                continue;
            }
            // SAFETY: sl_errors_get_item returned 1, so raw.file and
            // raw.message are malloc'd NUL-terminated strings (or NULL).
            // take_owned_c_string copies them and frees the C buffers.
            let file = take_owned_c_string(raw.file);
            let message = take_owned_c_string(raw.message);
            let file = if file.is_empty() { None } else { Some(file) };
            out.push(Diag {
                severity: severity_of(raw.severity),
                file,
                line: raw.line,
                col: raw.col,
                message,
            });
        }
        out
    }

    // ── Design access ─────────────────────────────────────────────────────────

    /// Returns a non-owning `Design` handle whose lifetime is tied to `&self`.
    /// Automatically ensures the compiler is still alive.
    pub fn design(&self) -> Option<Design<'_>> {
        let ptr = unsafe { sl_get_design(self.compiler) };
        if ptr.is_null() {
            None
        } else {
            Some(Design(ptr, PhantomData))
        }
    }

    /// Returns the raw UHDM VPI handle.  Valid until `self` is dropped.
    pub fn uhdm_design(&self) -> Option<*mut u32> {
        let ptr = unsafe { sl_get_uhdm_design(self.compiler) };
        if ptr.is_null() {
            None
        } else {
            Some(ptr)
        }
    }
}

// ── Session construction helpers ──────────────────────────────────────────────

/// Shared tail of session construction: parse the collected arguments, start
/// the compiler and assemble the `SurelogSession`.
///
/// On any failure every C++ object is freed and `None` is returned, so the
/// caller never observes leaked handles.
///
/// # Safety
/// `symbol_table`, `errors` and `clp` must be the three objects created
/// together by the caller (in dependency order) and are owned by this
/// function afterwards; `args` must contain only valid NUL-terminated C
/// strings (as produced by `CString`).
unsafe fn finish_session(
    symbol_table: *mut c_void,
    errors: *mut c_void,
    clp: *mut c_void,
    args: &[CString],
) -> Option<SurelogSession> {
    let c_argv: Vec<*const c_char> = args.iter().map(|s| s.as_ptr()).collect();
    let argc = args.len() as c_int;

    // SAFETY: `c_argv` points into `args`, alive for the full extent of this
    // call; all pointers are valid null-terminated C strings.
    let ok = sl_clp_parse_command_line(clp, argc, c_argv.as_ptr());
    if ok == 0 {
        sl_free_command_line_parser(clp);
        sl_free_error_container(errors);
        sl_free_symbol_table(symbol_table);
        return None;
    }
    let compiler = sl_start_compiler(clp);
    if compiler.is_null() {
        sl_free_command_line_parser(clp);
        sl_free_error_container(errors);
        sl_free_symbol_table(symbol_table);
        return None;
    }
    Some(SurelogSession {
        compiler,
        clp,
        errors,
        symbol_table,
        compiled: true,
    })
}

/// Builder for a Surelog compile session.
///
/// Sets Surelog's pipeline flags (parse/compile/elaborate/elab-uhdm), appends
/// command-line arguments (files, `-top`, defines, include dirs, …), then
/// `build()`s the session.  Flags mirror Surelog's CLI options.
///
/// Ownership: the builder owns the raw Surelog objects created in `new()`.
/// `build(self)` hands them to the returned `SurelogSession`, which frees them
/// in the correct order (`compiler → clp → errors → symbol_table`). Dropping
/// an unused builder releases its parser, error container and symbol table.
pub struct SessionBuilder {
    symbol_table: *mut c_void,
    errors: *mut c_void,
    clp: *mut c_void,
    args: Vec<CString>,
}

impl Drop for SessionBuilder {
    fn drop(&mut self) {
        // SAFETY: non-null pointers are owned by this builder. build()
        // replaces transferred pointers with null before finish_session
        // takes ownership, preventing double-free on either success or error.
        unsafe {
            if !self.clp.is_null() {
                sl_free_command_line_parser(self.clp);
            }
            if !self.errors.is_null() {
                sl_free_error_container(self.errors);
            }
            if !self.symbol_table.is_null() {
                sl_free_symbol_table(self.symbol_table);
            }
        }
    }
}

impl SessionBuilder {
    /// Create the symbol table, error container and command-line parser, and
    /// disable Python support.  `None` if the C++ side fails to initialise
    /// any object.
    pub fn new() -> Option<SessionBuilder> {
        // SAFETY: the pointers are created here and owned by the builder;
        // they are freed by `build()` on failure or by the resulting session.
        unsafe {
            let symbol_table = sl_create_symbol_table();
            if symbol_table.is_null() {
                return None;
            }
            let errors = sl_create_error_container(symbol_table);
            if errors.is_null() {
                sl_free_symbol_table(symbol_table);
                return None;
            }
            let clp = sl_create_command_line_parser(errors, symbol_table);
            if clp.is_null() {
                sl_free_error_container(errors);
                sl_free_symbol_table(symbol_table);
                return None;
            }
            sl_clp_no_python(clp);
            Some(SessionBuilder {
                symbol_table,
                errors,
                clp,
                args: Vec::new(),
            })
        }
    }

    /// Enable parsing.  Surelog enables this by default; kept for symmetry
    /// with the CLI flags.
    pub fn set_parse(&mut self) -> &mut Self {
        unsafe {
            sl_clp_set_parse(self.clp);
        }
        self
    }

    /// Write preprocessed output files.
    ///
    /// Surelog's `-parse` CLI flag enables this implicitly, and the full
    /// parse→compile→elaborate pipeline requires it (without it the design is
    /// left empty).  Always enable it when driving the pipeline via setters.
    pub fn set_write_pp_output(&mut self) -> &mut Self {
        unsafe {
            sl_clp_set_write_pp_output(self.clp);
        }
        self
    }

    /// Enable compilation of the parsed design.
    pub fn set_compile(&mut self) -> &mut Self {
        unsafe {
            sl_clp_set_compile(self.clp);
        }
        self
    }

    /// Enable elaboration of the design.
    pub fn set_elaborate(&mut self) -> &mut Self {
        unsafe {
            sl_clp_set_elaborate(self.clp);
        }
        self
    }

    /// Enable UHDM full elaboration: ref binding and per-instance
    /// uniquification.
    pub fn set_elab_uhdm(&mut self) -> &mut Self {
        unsafe {
            sl_clp_set_elab_uhdm(self.clp);
        }
        self
    }

    /// Suppress Surelog's own stdout diagnostics.
    pub fn set_mute_stdout(&mut self) -> &mut Self {
        unsafe {
            sl_clp_set_mute_stdout(self.clp);
        }
        self
    }

    /// Append a command-line argument (a file path, `-top`, `-D…`,
    /// `+incdir+…`, …).  Returns `false` and does not append if `arg`
    /// contains an interior NUL byte.
    pub fn add_arg(&mut self, arg: &str) -> bool {
        match CString::new(arg) {
            Ok(c) => {
                self.args.push(c);
                true
            }
            Err(_) => false,
        }
    }

    /// Parse the collected arguments and start the compiler.
    ///
    /// On success the returned session owns all C++ objects; on failure
    /// everything is freed and `None` is returned.
    pub fn build(mut self) -> Option<SurelogSession> {
        // argv[0] is the program name, which Surelog's parser expects.
        let program = CString::new("llg").ok()?;
        self.args.insert(0, program);

        let symbol_table = std::mem::replace(&mut self.symbol_table, std::ptr::null_mut());
        let errors = std::mem::replace(&mut self.errors, std::ptr::null_mut());
        let clp = std::mem::replace(&mut self.clp, std::ptr::null_mut());
        let args = std::mem::take(&mut self.args);
        // SAFETY: the three pointers were created in `new()` and move here;
        // `finish_session` owns them and frees them on failure.
        unsafe { finish_session(symbol_table, errors, clp, &args) }
    }
}

// ── Non-owning handle types ───────────────────────────────────────────────────
//
// Lifetime `'s` is the lifetime of the *session* (or parent handle) that owns
// the underlying C++ object.  Callers never write these lifetimes explicitly —
// they are inferred from `&SurelogSession` (or `&Design`, `&ModuleInstance`).

/// Non-owning handle to a Surelog `Design`.  Valid for `'s`.
pub struct Design<'s>(*mut c_void, PhantomData<&'s ()>);

/// Non-owning handle to a `ModuleInstance`.  Valid for `'s`.
pub struct ModuleInstance<'s>(*mut c_void, PhantomData<&'s ()>);

/// Non-owning handle to a `FileContent`.  Valid for `'s`.
pub struct FileContent<'s>(*mut c_void, PhantomData<&'s ()>);

impl<'s> Design<'s> {
    /// Raw pointer for interop with other FFI modules (e.g. VPI).
    pub fn raw(&self) -> *mut c_void {
        self.0
    }

    pub fn top_instance_count(&self) -> u32 {
        unsafe { sl_design_get_top_instance_count(self.0) }
    }

    pub fn top_instance(&self, i: u32) -> Option<ModuleInstance<'s>> {
        let ptr = unsafe { sl_design_get_top_instance(self.0, i) };
        if ptr.is_null() {
            None
        } else {
            Some(ModuleInstance(ptr, PhantomData))
        }
    }

    pub fn file_content_count(&self) -> u32 {
        unsafe { sl_design_get_file_content_count(self.0) }
    }

    pub fn file_content(&self, i: u32) -> Option<FileContent<'s>> {
        let ptr = unsafe { sl_design_get_file_content(self.0, i) };
        if ptr.is_null() {
            None
        } else {
            Some(FileContent(ptr, PhantomData))
        }
    }
}

impl<'s> ModuleInstance<'s> {
    /// Raw pointer for interop with other FFI modules.
    pub fn raw(&self) -> *mut c_void {
        self.0
    }

    pub fn full_path_name(&self) -> String {
        take_owned_c_string(unsafe { sl_instance_get_full_path_name(self.0) })
    }

    pub fn file_path(&self) -> String {
        take_owned_c_string(unsafe { sl_instance_get_file_path(self.0) })
    }

    pub fn child_count(&self) -> u32 {
        unsafe { sl_instance_get_child_count(self.0) }
    }

    pub fn child(&self, i: u32) -> Option<ModuleInstance<'s>> {
        let ptr = unsafe { sl_instance_get_child(self.0, i) };
        if ptr.is_null() {
            None
        } else {
            Some(ModuleInstance(ptr, PhantomData))
        }
    }
}

impl<'s> FileContent<'s> {
    /// Raw pointer for interop with other FFI modules.
    pub fn raw(&self) -> *mut c_void {
        self.0
    }

    pub fn path(&self) -> String {
        take_owned_c_string(unsafe { sl_file_content_get_path(self.0) })
    }

    pub fn file_id(&self) -> u32 {
        unsafe { sl_file_content_get_file_id(self.0) }
    }

    pub fn node_count(&self) -> u32 {
        unsafe { sl_file_content_get_node_count(self.0) }
    }

    /// Return the parse-tree node at `index`, or `None` if out of range.
    ///
    /// The returned `ParseNode` owns its strings; no pointers into the
    /// C++ session are retained after the call.
    pub fn get_node(&self, index: u32) -> Option<ParseNode> {
        // Zero-initialise so padding bytes are defined (avoid UB in
        // assume_init even though padding is not read by Rust).
        let mut raw: SLRawNodeInfo = unsafe { std::mem::zeroed() };
        let ok = unsafe { sl_file_content_get_node(self.0, index, &mut raw as *mut _) };
        if ok == 0 {
            return None;
        }
        // SAFETY: sl_file_content_get_node returned 1, so all fields are
        // valid.  type_name points to a static string; symbol_name points
        // into the SymbolTable intern pool (valid for compiler lifetime) or
        // is null.  We copy both into owned Strings immediately.
        let symbol_name = if raw.symbol_name.is_null() {
            None
        } else {
            Some(
                unsafe { CStr::from_ptr(raw.symbol_name) }
                    .to_string_lossy()
                    .into_owned(),
            )
        };
        Some(ParseNode {
            line: raw.line,
            col: raw.col,
            end_line: raw.end_line,
            end_col: raw.end_col,
            type_id: raw.type_id,
            file_id: raw.file_id,
            parent_index: raw.parent_index,
            child_index: raw.child_index,
            sibling_index: raw.sibling_index,
            symbol_name,
        })
    }
}

// ── Surelog parse-tree node ───────────────────────────────────────────────────

/// A single parse-tree node from Surelog's `FileContent`.
///
/// Obtained by iterating `FileContent::get_node`.  Position fields are
/// **1-based** like their VPI equivalents.  `type_id` is the raw
/// `VObjectType` discriminant; map it using the `PARSE_OFFSET` constants in
/// `tokens.rs` to classify it for semantic-token purposes.
#[derive(Debug, Clone)]
pub struct ParseNode {
    /// Source line (1-based).
    pub line: u32,
    /// Source column (1-based; 0 if unknown).
    pub col: u16,
    /// End source line (1-based; 0 if unknown).
    pub end_line: u32,
    /// End source column (1-based; 0 if unknown).
    pub end_col: u16,
    /// Raw `VObjectType` discriminant.  Stable within a Surelog build but
    /// not guaranteed across major Surelog version bumps.
    pub type_id: u16,
    /// Raw `RawPathId` of the originating file; compare with
    /// `FileContent::file_id()` to detect `\`include`-injected nodes.
    pub file_id: u32,
    /// Index of the parent node in the `FileContent` node array.
    /// `0` means no parent (`InvalidRawNodeId`).
    pub parent_index: u32,
    /// First child node in the `FileContent` tree, or zero when absent.
    pub child_index: u32,
    /// Next sibling node in the `FileContent` tree, or zero when absent.
    pub sibling_index: u32,
    /// Identifier / symbol string for leaf nodes (e.g. macro names,
    /// identifier tokens).  `None` for grammar-rule interior nodes.
    pub symbol_name: Option<String>,
}

// ── VPI-based node description ───────────────────────────────────────────────

/// Describes a single design object obtained by walking the UHDM tree via the
/// VPI interface.
///
/// Position fields are **1-based** (as returned by `vpiLineNo` / `vpiColumnNo`);
/// callers that need 0-based LSP positions should subtract 1.
#[derive(Debug, Clone)]
pub struct VObjectInfo {
    /// Source line of the object (1-based, from `vpiLineNo`).
    pub line: u32,
    /// Source column of the object (1-based, from `vpiColumnNo`; 0 if unknown).
    pub col: u32,
    /// End source line (1-based, from `vpiEndLineNo`; 0 if unknown).
    pub end_line: u32,
    /// End source column (1-based, from `vpiEndColumnNo`; 0 if unknown).
    pub end_col: u32,
    /// Standard VPI or UHDM object type (e.g. `vpiModule`, `vpiNet`).
    /// Use `llg::ffi::vpi` constants to classify this value.  This replaces the
    /// previous internal Surelog `VObjectType` discriminant which was not stable
    /// across Surelog versions.
    pub vpi_type: i32,
    /// Object name from `vpi_get_str(vpiName, h)`; `None` for anonymous nodes.
    pub name: Option<String>,
    /// Source file path from `vpi_get_str(vpiFile, h)`.
    pub file: String,
}

// ── Internal helper ───────────────────────────────────────────────────────────

fn take_owned_c_string(ptr: *mut c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    unsafe {
        let s = CStr::from_ptr(ptr).to_string_lossy().into_owned();
        sl_free_string(ptr);
        s
    }
}

/// Runs UHDM elaboration on the design returned by `SurelogSession::uhdm_design`.
pub fn uhdm_elaborate(vpi_design: *mut u32) {
    unsafe {
        sl_uhdm_elaborate(vpi_design);
    }
}

// ── Legacy standalone handle types (used by bin tools) ───────────────────────
//
// These types expose the Surelog session objects individually for binary tools
// (`llg_demo`, `hellouhdm`) that need fine-grained control over each phase.
// The borrow checker enforces the correct dependency chain through lifetimes:
//   SymbolTable ← ErrorContainer ← CommandLineParser ← Compiler ← Design

pub struct SymbolTable(*mut c_void);
pub struct ErrorContainer<'st>(*mut c_void, PhantomData<&'st SymbolTable>);
pub struct CommandLineParser<'a>(*mut c_void, PhantomData<&'a ()>);
pub struct Compiler<'clp>(*mut c_void, PhantomData<&'clp ()>);

impl Drop for SymbolTable {
    fn drop(&mut self) {
        unsafe {
            sl_free_symbol_table(self.0);
        }
    }
}
impl Drop for ErrorContainer<'_> {
    fn drop(&mut self) {
        unsafe {
            sl_free_error_container(self.0);
        }
    }
}
impl Drop for CommandLineParser<'_> {
    fn drop(&mut self) {
        unsafe {
            sl_free_command_line_parser(self.0);
        }
    }
}
impl Drop for Compiler<'_> {
    fn drop(&mut self) {
        unsafe {
            sl_shutdown_compiler(self.0);
        }
    }
}

// ── Constructors (free functions — no owning parent yet) ─────────────────────

pub fn create_symbol_table() -> SymbolTable {
    SymbolTable(unsafe { sl_create_symbol_table() })
}
pub fn create_error_container<'st>(st: &'st SymbolTable) -> ErrorContainer<'st> {
    ErrorContainer(unsafe { sl_create_error_container(st.0) }, PhantomData)
}
pub fn create_command_line_parser<'a>(
    ec: &'a ErrorContainer<'_>,
    st: &'a SymbolTable,
) -> CommandLineParser<'a> {
    CommandLineParser(
        unsafe { sl_create_command_line_parser(ec.0, st.0) },
        PhantomData,
    )
}

// ── ErrorContainer methods ───────────────────────────────────────────────────

impl<'st> ErrorContainer<'st> {
    pub fn print_messages(&self, mute: c_int) {
        unsafe {
            sl_errors_print_messages(self.0, mute);
        }
    }
    pub fn fatal_count(&self) -> u32 {
        unsafe { sl_errors_get_fatal_count(self.0) }
    }
    pub fn syntax_count(&self) -> u32 {
        unsafe { sl_errors_get_syntax_count(self.0) }
    }
    pub fn error_count(&self) -> u32 {
        unsafe { sl_errors_get_error_count(self.0) }
    }
}

// ── CommandLineParser methods ────────────────────────────────────────────────

impl<'a> CommandLineParser<'a> {
    pub fn no_python(&self) {
        unsafe {
            sl_clp_no_python(self.0);
        }
    }
    pub fn set_parse(&self) {
        unsafe {
            sl_clp_set_parse(self.0);
        }
    }
    pub fn set_write_pp_output(&self) {
        unsafe {
            sl_clp_set_write_pp_output(self.0);
        }
    }
    pub fn set_compile(&self) {
        unsafe {
            sl_clp_set_compile(self.0);
        }
    }
    pub fn set_elaborate(&self) {
        unsafe {
            sl_clp_set_elaborate(self.0);
        }
    }
    /// Enable UHDM full elaboration: rebinds every `ref_obj`/`var_select`
    /// through `vpiActual` to the per-instance object, uniquifies typespecs,
    /// and resolves function/task calls.  Requires `set_elaborate` to have
    /// any effect.  This mirrors Surelog's `-elabuhdm` flag.
    pub fn set_elab_uhdm(&self) {
        unsafe {
            sl_clp_set_elab_uhdm(self.0);
        }
    }
    pub fn help(&self) -> bool {
        unsafe { sl_clp_help(self.0) != 0 }
    }
    pub fn mute_stdout(&self) -> c_int {
        unsafe { sl_clp_mute_stdout(self.0) }
    }

    /// Parse the command-line arguments from a string slice.
    ///
    /// Returns `false` if any argument contains an interior null byte, or if
    /// the underlying C++ parser reports failure.
    pub fn parse_command_line(&self, args: &[&str]) -> bool {
        use std::ffi::CString;
        let owned: Option<Vec<CString>> = args.iter().map(|s| CString::new(*s).ok()).collect();
        let owned = match owned {
            Some(v) => v,
            None => return false,
        };
        let c_argv: Vec<*const c_char> = owned.iter().map(|s| s.as_ptr()).collect();
        let argc = owned.len() as c_int;
        // SAFETY: `c_argv` points into `owned`, alive for the duration of this call.
        unsafe { sl_clp_parse_command_line(self.0, argc, c_argv.as_ptr()) != 0 }
    }

    /// Start the Surelog compiler. The returned `Compiler` borrows `self`,
    /// preventing the parser from being freed while the compiler is alive.
    /// Returns `None` when the C++ side cannot create a compiler.
    pub fn start_compiler(&self) -> Option<Compiler<'_>> {
        let ptr = unsafe { sl_start_compiler(self.0) };
        if ptr.is_null() {
            None
        } else {
            Some(Compiler(ptr, PhantomData))
        }
    }
}

// ── Compiler methods ─────────────────────────────────────────────────────────

impl<'clp> Compiler<'clp> {
    /// Return a non-owning `Design` handle.  Valid for as long as `self` is
    /// borrowed — the borrow checker prevents `self` from being dropped while
    /// the returned handle is in use.
    pub fn get_design(&self) -> Option<Design<'_>> {
        let ptr = unsafe { sl_get_design(self.0) };
        if ptr.is_null() {
            None
        } else {
            Some(Design(ptr, PhantomData))
        }
    }

    /// Return the raw UHDM VPI handle.  The pointer is valid until `self` is
    /// dropped; callers must not use it after that point.
    pub fn get_uhdm_design(&self) -> Option<*mut u32> {
        let ptr = unsafe { sl_get_uhdm_design(self.0) };
        if ptr.is_null() {
            None
        } else {
            Some(ptr)
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a `SurelogSession` from a list of extra string arguments.
    /// The program name `"llg-test"` is prepended automatically.
    fn make_session(extra_args: &[&str]) -> Option<SurelogSession> {
        let mut all_args = vec!["llg-test"];
        all_args.extend_from_slice(extra_args);
        SurelogSession::new(&all_args)
    }

    /// Verify that `Design` cannot outlive the `SurelogSession` it was obtained
    /// from.  This test exists to document the borrow relationship — if it ever
    /// fails to compile, the lifetime tracking has been broken.
    ///
    /// The relevant invariant is enforced at compile time; there is no runtime
    /// assertion to make.  The test body just exercises the borrowing pattern.
    ///
    /// Marked `#[ignore]` because it requires the Surelog C++ library to be
    /// linked, which is not available in unit-test builds without a full CMake
    /// build first.
    #[test]
    #[ignore = "requires linked Surelog C++ library"]
    fn design_lifetime_tied_to_session() {
        let session = make_session(&["-parse"]);
        let session = match session {
            Some(s) => s,
            None => return, // Surelog not available; skip.
        };
        // `design` borrows `session`; the borrow checker prevents `session`
        // from being dropped while `design` is alive.
        let design = session.design();
        // Both are still in scope here — OK.
        let _ = design.as_ref().map(|d| d.top_instance_count());
        drop(design);
        drop(session);
        // Reversed drop order is now safe because `design` is gone first.
    }

    /// Verify that `FileContent` and `ModuleInstance` lifetimes are also tied
    /// to the session through the `Design` borrow.
    #[test]
    #[ignore = "requires linked Surelog C++ library"]
    fn file_content_lifetime_tied_to_design() {
        let session = make_session(&["-parse"]);
        let session = match session {
            Some(s) => s,
            None => return,
        };
        let design = session.design();
        if let Some(ref d) = design {
            let count = d.file_content_count();
            for i in 0..count {
                // `fc` borrows `d` which borrows `session` — none can be
                // dropped out of order.
                if let Some(fc) = d.file_content(i) {
                    let _path = fc.path();
                    let _id = fc.file_id();
                    let _node_count = fc.node_count();
                    // Node-level access is now performed via the VPI interface;
                    // see `tokens::collect_vpi_tokens`.
                }
            }
        }
    }

    /// Verify that `ModuleInstance` children share the session lifetime.
    #[test]
    #[ignore = "requires linked Surelog C++ library"]
    fn module_instance_lifetime_tied_to_design() {
        let session = make_session(&["-parse"]);
        let session = match session {
            Some(s) => s,
            None => return,
        };
        if let Some(ref d) = session.design() {
            for i in 0..d.top_instance_count() {
                if let Some(top) = d.top_instance(i) {
                    for j in 0..top.child_count() {
                        let _ = top.child(j);
                    }
                }
            }
        }
    }

    // ── Compile-fail style checks (enforced by rustc, not by running) ─────────
    //
    // The following commented-out snippets would not compile. They are kept as
    // documentation of what the borrow checker prevents.
    //
    // ```compile_fail
    // fn design_outlives_session() {
    //     let design;
    //     {
    //         let session = unsafe { make_session(&["-parse"]) }.unwrap();
    //         design = session.design(); // ERROR: `session` does not live long enough
    //     }
    //     let _ = design; // `session` already dropped here
    // }
    // ```
    //
    // ```compile_fail
    // fn file_content_outlives_design() {
    //     let fc;
    //     let session = unsafe { make_session(&["-parse"]) }.unwrap();
    //     {
    //         let design = session.design().unwrap();
    //         fc = design.file_content(0); // ERROR: `design` does not live long enough
    //     }
    //     let _ = fc;
    // }
    // ```
}
