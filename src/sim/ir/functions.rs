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
    /// Descriptor storage for a fixed value that exceeds packed transport.
    pub(in crate::sim) fixed_array: Option<usize>,
    /// Descriptor-backed native value bound to this formal in the callee
    /// (`llg_value_t*`); index into [`super::IrModel::native_values`].
    pub(in crate::sim) native_value: Option<usize>,
    /// Companion containers of `native_value` (see
    /// [`super::IrNativeValue::companions`]); each is one more `void*` C
    /// parameter right after the value's own, passed like a container formal.
    pub(in crate::sim) native_companions: Vec<usize>,
    /// Real fixed-array storage bound to this formal in the callee (a
    /// `double*` C parameter); index into [`super::IrModel::arrays`]. Real
    /// arrays keep numeric cells and never use the packed fixed-value ABI.
    pub(in crate::sim) real_array: Option<usize>,
    /// Resizable container bound to this formal in the callee (a `void*` C
    /// parameter naming caller-created storage of the same container type);
    /// index into [`super::IrModel::containers`]. The caller copies inputs
    /// in and outputs back, so the callee owns a private value.
    pub(in crate::sim) container: Option<usize>,
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
    /// Named-event formal. An input is a by-value `llg_event_t` naming the
    /// caller's event object (`IrCallArg::EventVal`, `IrEventRef::Formal`); an
    /// output, inout or ref event formal is resolved by inline call lowering.
    pub(in crate::sim) event: bool,
    /// Native arbitrary-byte string formal. String formals use
    /// `llg_string_t` values/pointers rather than packed storage.
    pub(in crate::sim) string: bool,
    /// For a packed or real output/inout formal that a detached fork branch
    /// shares: the local naming the shared frame cell that holds its value
    /// during the activation. The callee creates the cell from `o{idx}` at
    /// entry, outside the body's scopes, and copies it back at return. For a
    /// container formal that any fork branch names: its shared capture name;
    /// the callee copies the caller's container into a shared frame the same
    /// way.
    pub(in crate::sim) shared_local: Option<String>,
}

/// Owned DPI-C linkage qualifiers and foreign C-layer types attached to one
/// imported subroutine.
///
/// The generated wrapper keeps this metadata separate from the internal
/// simulator calling convention: each formal still travels as its internal
/// value (a packed payload for aggregates), and the thunk converts it to the
/// foreign representation in [`Self::formals`]. `pure` and `context` remain
/// available to effect analysis and diagnostics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrDpiImport {
    pub(in crate::sim) c_name: String,
    pub(in crate::sim) context: bool,
    pub(in crate::sim) pure: bool,
    /// Foreign type of each declared formal, in declaration order.
    pub(in crate::sim) formals: Vec<IrDpiType>,
    /// Foreign result type; `None` for a void function. An imported task
    /// returns the C `int` of the disable protocol (SV 35.5.4), which this
    /// does not record.
    pub(in crate::sim) ret: Option<IrDpiType>,
}

impl IrDpiImport {
    pub fn new(
        c_name: String,
        context: bool,
        pure: bool,
        formals: Vec<IrDpiType>,
        ret: Option<IrDpiType>,
    ) -> Self {
        Self {
            c_name,
            context,
            pure,
            formals,
            ret,
        }
    }

    pub fn c_name(&self) -> &str {
        &self.c_name
    }

    pub fn is_context(&self) -> bool {
        self.context
    }

    pub fn is_pure(&self) -> bool {
        self.pure
    }

    pub fn formals(&self) -> &[IrDpiType] {
        &self.formals
    }

    pub fn ret(&self) -> Option<&IrDpiType> {
        self.ret.as_ref()
    }

    /// Whether an open-array formal still has an unsized dimension: such an
    /// import is a template that each call specializes for its actuals.
    pub fn is_open_template(&self) -> bool {
        self.formals.iter().any(IrDpiType::is_unsized)
    }
}

/// The C-layer representation of one DPI value (SV Annex H.7).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum IrDpiType {
    /// Scalar `bit`: an `svBit` code.
    Bit,
    /// Scalar `logic`/`reg`: an `svLogic` code.
    Logic,
    /// `byte`, `shortint`, `int`, `longint` (also as an enumeration base):
    /// the C integer of `bytes` bytes.
    Int {
        bytes: u8,
        signed: bool,
    },
    Real,
    ShortReal,
    Chandle,
    String,
    /// Any other packed type, in canonical `svBitVecVal`/`svLogicVecVal`
    /// chunks (H.7.7).
    Vector {
        width: u32,
        logic: bool,
    },
    /// A sized unpacked array, dimensions slowest first with their declared
    /// ranges; C layout in natural order (H.7.6).
    Array {
        dims: Vec<(i32, i32)>,
        element: Box<IrDpiType>,
    },
    /// An unpacked structure with the C compiler's layout (H.7.8).
    Struct(Vec<IrDpiType>),
    /// An open-array formal passed by `svOpenArrayHandle` (H.12). A template
    /// marks an unsized unpacked dimension with `None`; a call
    /// specialization carries the actual's ranges.
    Open {
        dims: Vec<Option<(i32, i32)>>,
        element: Box<IrDpiType>,
    },
    /// The unsized packed dimension of a template open array; a call
    /// specialization replaces it with the actual's linearized `Vector`.
    OpenPacked {
        logic: bool,
    },
}

impl IrDpiType {
    /// Whether a dimension or packed width is still unsized.
    pub fn is_unsized(&self) -> bool {
        match self {
            Self::OpenPacked { .. } => true,
            Self::Open { dims, element } => {
                dims.iter().any(Option::is_none) || element.is_unsized()
            }
            Self::Array { element, .. } => element.is_unsized(),
            Self::Struct(members) => members.iter().any(Self::is_unsized),
            _ => false,
        }
    }

    /// Bits of this value in the internal packed payload, or `None` for a
    /// type outside the payload (real, string, chandle) or still unsized.
    pub fn payload_width(&self) -> Option<u64> {
        match self {
            Self::Bit | Self::Logic => Some(1),
            Self::Int { bytes, .. } => Some(u64::from(*bytes) * 8),
            Self::Vector { width, .. } => Some(u64::from(*width)),
            Self::Array { dims, element } => {
                let mut total = element.payload_width()?;
                for (left, right) in dims {
                    total = total.checked_mul(dimension_size(*left, *right))?;
                }
                Some(total)
            }
            Self::Open { dims, element } => {
                let mut total = element.payload_width()?;
                for dim in dims {
                    let (left, right) = (*dim)?;
                    total = total.checked_mul(dimension_size(left, right))?;
                }
                Some(total)
            }
            Self::Struct(members) => members
                .iter()
                .try_fold(0u64, |sum, member| sum.checked_add(member.payload_width()?)),
            Self::Real
            | Self::ShortReal
            | Self::Chandle
            | Self::String
            | Self::OpenPacked { .. } => None,
        }
    }
}

/// Element count of one `[left:right]` dimension.
pub(in crate::sim) fn dimension_size(left: i32, right: i32) -> u64 {
    (i64::from(left) - i64::from(right)).unsigned_abs() + 1
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
            fixed_array: None,
            native_value: None,
            native_companions: Vec::new(),
            real_array: None,
            container: None,
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
            shared_local: None,
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
    pub fn native_value(&self) -> Option<usize> {
        self.native_value
    }
    pub fn real_array(&self) -> Option<usize> {
        self.real_array
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
    pub(in crate::sim) origin: crate::sim::semantic::Origin,
    pub(in crate::sim) return_default: Option<IrConst>,
    /// Optional model signal that owns a statically allocated numeric result
    /// when the result is also targeted by a continuous assignment.
    pub(in crate::sim) return_signal: Option<usize>,
    pub(in crate::sim) c_name: String,
    /// Runtime diagnostic spelling, independent of the internal C symbol.
    pub(in crate::sim) diagnostic_name: Option<String>,
    /// The retained definition is a lowering template whose body is expanded
    /// into each caller. It is not an independently callable execution frame.
    pub(in crate::sim) inline_expanded: bool,
    /// Automatic subprograms use fresh C locals per call; static subprograms
    /// retain their return/local storage across calls.
    pub(in crate::sim) automatic: bool,
    /// `true` for a task and `false` for a function, including a void
    /// function. Return storage alone cannot distinguish those two cases.
    pub(in crate::sim) is_task: bool,
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
    /// Indices of native record `ref` formals that this specialization binds
    /// to declaration-owned record storage at lowering (SIM-008). Calls pass
    /// [`super::IrCallArg::NativeRefBound`] for them and the body never uses
    /// their native values.
    pub(in crate::sim) bound_native_refs: Vec<usize>,
    /// Compiler-generated copies from static by-value input formals into
    /// their persistent storage. A read-only callback materializes these
    /// copies in its private formal bindings instead of publishing them to
    /// model storage.
    pub(in crate::sim) callback_private_formal_copies: Vec<(usize, usize)>,
    /// True when a static function's result can be recomputed in fresh
    /// callback storage without observing its persistent result from an
    /// earlier call. The lowering proof requires a read-free result and a
    /// result value established on every path.
    pub(in crate::sim) callback_return_independent: bool,
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
        let is_task = ret.is_none();
        Self {
            origin: crate::sim::semantic::Origin::Synthetic {
                reason: "constructed function".to_owned(),
            },
            c_name,
            inline_expanded: false,
            diagnostic_name: None,
            automatic: true,
            is_task,
            return_default: None,
            return_signal: None,
            ret_chandle: false,
            ret_string: false,
            ret,
            dpi: None,
            receiver_class: None,
            virtual_slot: None,
            formals,
            bound_native_refs: Vec::new(),
            callback_private_formal_copies: Vec::new(),
            callback_return_independent: false,
            locals,
            pre_fns,
            body,
        }
    }

    pub fn origin(&self) -> &crate::sim::semantic::Origin {
        &self.origin
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
    /// Source function label for runtime diagnostics and coroutine metadata.
    /// Functions without source provenance use an unnamed label, never a C symbol.
    pub fn diagnostic_name(&self) -> &str {
        self.diagnostic_name
            .as_deref()
            .unwrap_or("unnamed function")
    }
    /// Whether calls expand this definition into their owning execution body.
    pub fn is_inline_expanded(&self) -> bool {
        self.inline_expanded
    }
    pub fn is_automatic(&self) -> bool {
        self.automatic
    }
    pub fn is_task(&self) -> bool {
        self.is_task
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
