//! Functions.

use super::*;

/// Passing mode of a lowered function/task formal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrFormalMode {
    Input,
    Output,
    Inout,
    Ref,
}

/// A formal argument of a lowered function/task.
#[derive(Clone, Debug, PartialEq)]
pub struct IrFormal {
    /// `true` for output/inout formals (passed as `sv4_t* o{idx}`); `false`
    /// for inputs (passed by value as `sv4_t a{idx}`).  Indices are the
    /// formal's declaration position.
    pub(in crate::sim) is_out: bool,
    pub(in crate::sim) mode: IrFormalMode,
    /// Recursive fixed-value shape; its declaration-order payload uses `width` bits.
    pub(in crate::sim) fixed_shape: Option<IrContainerElement>,
    /// Default fixed payload, preserving each unpacked leaf's state domain.
    pub(in crate::sim) fixed_default: Option<IrConst>,
    /// `true` only for a `const ref` formal.
    pub(in crate::sim) const_ref: bool,
    /// `true` only for a `ref static` formal.
    pub(in crate::sim) ref_static: bool,
    pub(in crate::sim) width: u32,
    pub(in crate::sim) signed: bool,
    pub(in crate::sim) two_state: bool,
    /// Native real formal; width/signedness are unused when set.
    pub(in crate::sim) real: bool,
    /// `true` for a `shortreal` formal. The value is rounded at the formal
    /// storage boundary, just like a shortreal signal.
    pub(in crate::sim) shortreal: bool,
    /// Non-integral native pointer formal; width/signedness are unused.
    pub(in crate::sim) chandle: bool,
    /// Named-event formal. Event handles are resolved by inline call lowering,
    /// not represented as packed values in the C ABI.
    pub(in crate::sim) event: bool,
    /// Native arbitrary-byte string formal. String formals use
    /// `llg_string_t` values/pointers rather than packed storage.
    pub(in crate::sim) string: bool,
}

/// Owned DPI-C linkage qualifiers attached to one imported subroutine.
///
/// The generated wrapper keeps this metadata separate from the internal
/// simulator calling convention.  `c_name` is emitted only after lowering has
/// validated the bounded scalar ABI, while `pure` and `context` remain
/// available to effect analysis and diagnostics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrDpiImport {
    pub(in crate::sim) c_name: String,
    pub(in crate::sim) context: bool,
    pub(in crate::sim) pure: bool,
}

impl IrDpiImport {
    pub fn c_name(&self) -> &str {
        &self.c_name
    }

    pub fn is_context(&self) -> bool {
        self.context
    }

    pub fn is_pure(&self) -> bool {
        self.pure
    }
}

impl IrFormal {
    pub fn new(is_out: bool, width: u32, signed: bool) -> Result<Self, IrValidationError> {
        validate_width("formal.width", width)?;
        Ok(Self {
            is_out,
            mode: if is_out {
                IrFormalMode::Output
            } else {
                IrFormalMode::Input
            },
            const_ref: false,
            ref_static: false,
            fixed_shape: None,
            fixed_default: None,
            width,
            signed,
            two_state: false,
            real: false,
            shortreal: false,
            chandle: false,
            event: false,
            string: false,
        })
    }

    pub fn is_out(&self) -> bool {
        self.is_out
    }
    pub fn mode(&self) -> IrFormalMode {
        self.mode
    }
    pub fn is_ref(&self) -> bool {
        self.mode == IrFormalMode::Ref
    }
    pub fn is_const_ref(&self) -> bool {
        self.is_ref() && self.const_ref
    }
    pub fn is_ref_static(&self) -> bool {
        self.is_ref() && self.ref_static
    }
    pub fn is_address(&self) -> bool {
        self.is_out || self.is_ref()
    }
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn signed(&self) -> bool {
        self.signed
    }
    pub fn is_event(&self) -> bool {
        self.event
    }
    pub fn is_string(&self) -> bool {
        self.string
    }
}

/// Persistent function/task local (`_l{n}` or `_i{site}_{n}`).
#[derive(Clone, Debug, PartialEq)]
pub struct IrLocal {
    pub(in crate::sim) fixed_default: Option<IrConst>,
    pub(in crate::sim) c_name: String,
    pub(in crate::sim) width: u32,
    pub(in crate::sim) signed: bool,
    pub(in crate::sim) two_state: bool,
    pub(in crate::sim) real: bool,
    pub(in crate::sim) shortreal: bool,
    /// Native string storage; width/signedness are unused when set.
    pub(in crate::sim) string: bool,
    /// Legacy inline initializer for IRs that model local storage directly;
    /// lowered declarations use [`IrInitialization`] so runtime values keep
    /// their declaration and scheduling metadata.
    pub(in crate::sim) initial: Option<IrExpr>,
}

impl IrLocal {
    pub fn new(c_name: String, width: u32, signed: bool) -> Result<Self, IrValidationError> {
        validate_width("local.width", width)?;
        Ok(Self {
            c_name,
            width,
            signed,
            two_state: false,
            real: false,
            shortreal: false,
            string: false,
            initial: None,
            fixed_default: None,
        })
    }

    pub fn c_name(&self) -> &str {
        &self.c_name
    }
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn signed(&self) -> bool {
        self.signed
    }
}

/// A lowered function or delay-free task: a static C function with a
/// recursion-depth guard.
#[derive(Clone, Debug, PartialEq)]
pub struct IrFunc {
    pub(in crate::sim) return_default: Option<IrConst>,
    pub(in crate::sim) c_name: String,
    /// Automatic subprograms use fresh C locals per call; static subprograms
    /// retain their return/local storage across calls.
    pub(in crate::sim) automatic: bool,
    /// Distinguishes a chandle-returning function from a void function/task.
    pub(in crate::sim) ret_chandle: bool,
    /// Automatic function returning an owned SystemVerilog string.
    pub(in crate::sim) ret_string: bool,
    /// Return type; `None` for tasks and void functions.
    pub(in crate::sim) ret: Option<IrType>,
    /// Foreign DPI-C import contract. Imported functions use the same
    /// internal call ABI as ordinary subroutines and are rendered as thunks.
    pub(in crate::sim) dpi: Option<IrDpiImport>,
    /// Class-method functions receive one hidden `void *` receiver before
    /// their ordinary formals. `None` denotes a module/package subprogram.
    pub(in crate::sim) receiver_class: Option<usize>,
    /// Stable slot assigned to virtual methods in one inheritance family.
    pub(in crate::sim) virtual_slot: Option<usize>,
    pub(in crate::sim) formals: Vec<IrFormal>,
    /// Resolved-static locals in emission order (node-id sorted at lowering).
    /// Resolved-automatic locals remain declaration-site [`IrStmt::DeclLocal`]
    /// operations so nested block reentry recreates them correctly.
    pub(in crate::sim) locals: Vec<IrLocal>,
    pub(in crate::sim) pre_fns: Vec<IrPreFn>,
    pub(in crate::sim) body: Vec<IrStmt>,
}

impl IrFunc {
    /// Create a function/task staging value. Formal/local constructors check
    /// widths; the containing model checks body and call references.
    pub fn new(
        c_name: String,
        ret: Option<IrType>,
        formals: Vec<IrFormal>,
        locals: Vec<IrLocal>,
        pre_fns: Vec<IrPreFn>,
        body: Vec<IrStmt>,
    ) -> Self {
        Self {
            c_name,
            automatic: true,
            return_default: None,
            ret_chandle: false,
            ret_string: false,
            ret,
            dpi: None,
            receiver_class: None,
            virtual_slot: None,
            formals,
            locals,
            pre_fns,
            body,
        }
    }

    /// The all-X return initializer used by the recursion guard
    /// (`sv4_x(w, s)`), empty for void functions/tasks.
    pub fn ret_x(&self) -> String {
        match self.ret {
            Some(IrType::Packed {
                width,
                signed,
                two_state,
            }) => {
                if two_state {
                    format!("sv4_from_u64(0, {width}, {})", signed as u8)
                } else {
                    format!("sv4_x({width}, {})", signed as u8)
                }
            }
            Some(IrType::Real { .. }) => "0.0".to_string(),
            None => String::new(),
        }
    }

    pub fn c_name(&self) -> &str {
        &self.c_name
    }
    pub fn is_automatic(&self) -> bool {
        self.automatic
    }
    pub fn ret(&self) -> Option<IrType> {
        self.ret
    }
    pub fn dpi_import(&self) -> Option<&IrDpiImport> {
        self.dpi.as_ref()
    }
    pub fn formals(&self) -> &[IrFormal] {
        &self.formals
    }
    pub fn locals(&self) -> &[IrLocal] {
        &self.locals
    }
    pub fn pre_fns(&self) -> &[IrPreFn] {
        &self.pre_fns
    }
    pub fn body(&self) -> &[IrStmt] {
        &self.body
    }
}
