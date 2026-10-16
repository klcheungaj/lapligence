//! Vpi.

/// Object kinds admitted to the generated model's VPI catalog (IEEE
/// 1800-2009 clause 37, Annex K/M object types).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IrVpiObjectKind {
    #[default]
    Module,
    /// An interface instance (`vpiInterface`).
    Interface,
    /// A generate block instance (`vpiGenScope`), e.g. `g[0]` (SV 27.4).
    GenScope,
    /// A named `begin ... end` block (`vpiNamedBegin`).
    NamedBegin,
    /// A named `fork ... join` block (`vpiNamedFork`).
    NamedFork,
    Net,
    /// A packed variable; [`IrVpiVarKind`] selects its exact object type.
    Reg,
    RealVar,
    /// An unpacked array of variables (`vpiRegArray`, `vpiArrayVar`).
    RegArray,
    /// An unpacked array of nets (`vpiNetArray`, `vpiArrayNet`).
    NetArray,
    Parameter,
    Port,
}

impl IrVpiObjectKind {
    pub const fn c_type(self) -> &'static str {
        match self {
            Self::Module => "vpiModule",
            Self::Interface => "vpiInterface",
            Self::GenScope => "vpiGenScope",
            Self::NamedBegin => "vpiNamedBegin",
            Self::NamedFork => "vpiNamedFork",
            Self::Net => "vpiNet",
            Self::Reg => "vpiReg",
            Self::RealVar => "vpiRealVar",
            Self::RegArray => "vpiRegArray",
            Self::NetArray => "vpiNetArray",
            Self::Parameter => "vpiParameter",
            Self::Port => "vpiPort",
        }
    }

    /// Scopes own other catalog objects and have no value.
    pub const fn is_scope(self) -> bool {
        matches!(
            self,
            Self::Module | Self::Interface | Self::GenScope | Self::NamedBegin | Self::NamedFork
        )
    }
}

/// Declared data type of a variable, array element or parameter, which
/// decides its exact VPI object type (Annex M `vpiIntVar`, `vpiBitVar`, ...).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IrVpiVarKind {
    #[default]
    Logic,
    Bit,
    Int,
    Integer,
    ShortInt,
    LongInt,
    Byte,
    Time,
    Real,
    ShortReal,
    Enum,
    Struct,
    Union,
    /// A logic/bit variable with more than one packed dimension.
    PackedArray,
}

impl IrVpiVarKind {
    pub fn from_type_kind(kind: &str, packed_dimensions: usize) -> Self {
        match kind {
            "bit" if packed_dimensions > 1 => Self::PackedArray,
            "bit" => Self::Bit,
            "int" => Self::Int,
            "integer" => Self::Integer,
            "shortint" => Self::ShortInt,
            "longint" => Self::LongInt,
            "byte" => Self::Byte,
            "time" => Self::Time,
            "real" | "real_array" => Self::Real,
            "shortreal" | "shortreal_array" => Self::ShortReal,
            "enum" => Self::Enum,
            "struct" => Self::Struct,
            "union" => Self::Union,
            _ if packed_dimensions > 1 => Self::PackedArray,
            _ => Self::Logic,
        }
    }

    /// The `vpiType` of a variable of this kind.
    pub const fn c_type(self) -> &'static str {
        match self {
            Self::Logic => "vpiReg",
            Self::Bit => "vpiBitVar",
            Self::Int => "vpiIntVar",
            Self::Integer => "vpiIntegerVar",
            Self::ShortInt => "vpiShortIntVar",
            Self::LongInt => "vpiLongIntVar",
            Self::Byte => "vpiByteVar",
            Self::Time => "vpiTimeVar",
            Self::Real => "vpiRealVar",
            Self::ShortReal => "vpiShortRealVar",
            Self::Enum => "vpiEnumVar",
            Self::Struct => "vpiStructVar",
            Self::Union => "vpiUnionVar",
            Self::PackedArray => "vpiPackedArrayVar",
        }
    }
}

/// Port direction, in `vpiDirection` codes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IrVpiDirection {
    Input,
    Output,
    Inout,
    #[default]
    None,
}

impl IrVpiDirection {
    pub const fn c_code(self) -> &'static str {
        match self {
            Self::Input => "vpiInput",
            Self::Output => "vpiOutput",
            Self::Inout => "vpiInout",
            Self::None => "vpiNoDirection",
        }
    }
}

/// Elaborated value of a parameter object.
#[derive(Clone, Debug, PartialEq)]
pub enum IrVpiConstant {
    /// Four-state digits, most significant first (`0`, `1`, `x`, `z`).
    Bits(String),
    Real(f64),
    Text(String),
}

/// Owned source and storage metadata for one generated VPI object.
///
/// Paths use the same unit-separator hierarchy encoding as [`super::IrSignal`],
/// with generate scopes named per IEEE 1800-2009 27.4 (`g[0]`).
/// `signal`/`array` refer to stable model-table indices; the emitter resolves
/// those indices to C storage only after optimization has completed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IrVpiObject {
    pub(in crate::sim) time_unit_fs: u64,
    pub(in crate::sim) full_name: String,
    pub(in crate::sim) name: String,
    pub(in crate::sim) definition_name: Option<String>,
    pub(in crate::sim) file: Option<String>,
    pub(in crate::sim) line: u32,
    pub(in crate::sim) kind: IrVpiObjectKind,
    pub(in crate::sim) width: u32,
    pub(in crate::sim) signed: bool,
    pub(in crate::sim) real: bool,
    pub(in crate::sim) net: bool,
    pub(in crate::sim) signal: Option<usize>,
    pub(in crate::sim) array: Option<usize>,
    /// Variable/element data type (variables, arrays and parameters).
    pub(in crate::sim) var_kind: IrVpiVarKind,
    /// Declared `[left:right]` of the outermost packed dimension; `None`
    /// for a value without a declared packed range (`real`, a scalar).
    pub(in crate::sim) packed_range: Option<(i64, i64)>,
    /// `vpiNetType` code name of a net or net array (`vpiWire`, ...).
    pub(in crate::sim) net_type: Option<&'static str>,
    pub(in crate::sim) two_state: bool,
    pub(in crate::sim) top: bool,
    pub(in crate::sim) local_param: bool,
    pub(in crate::sim) direction: IrVpiDirection,
    /// Port order within its module, from 0.
    pub(in crate::sim) port_index: u32,
    /// Full name of the object a port's low connection refers to.
    pub(in crate::sim) port_target: Option<String>,
    pub(in crate::sim) constant: Option<IrVpiConstant>,
}

/// How a VPI call argument is visible to the application: as the object it
/// names (writable through `vpi_put_value`) or as a value snapshot.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum IrVpiArgRef {
    /// Any other expression: a read-only value snapshot (`vpiOperation`).
    #[default]
    Expression,
    /// A literal: `vpiConstant` with this `vpiConstType` code name.
    Constant(&'static str),
    /// A whole catalog object, by full name.
    Object(String),
    /// A constant bit-select (`width == 1`) or part-select of a catalog
    /// object: storage offset of the least significant bit and width.
    Select {
        object: String,
        lsb: u32,
        width: u32,
        left: i64,
        right: i64,
    },
    /// A constant-index element of a catalog array (flat row-major index).
    Element { object: String, index: u64 },
}

/// Type-only descriptor for one source-level VPI call site.  It is kept
/// separate from [`IrVpiObject`] because compiletf/sizetf receives argument
/// shapes before any runtime value exists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrVpiCompileCall {
    pub(in crate::sim) time_unit_fs: u64,
    pub(in crate::sim) name: String,
    pub(in crate::sim) args: Vec<IrVpiCompileArg>,
    /// Full name of the scope containing the call (`vpiScope`).
    pub(in crate::sim) scope: Option<String>,
    pub(in crate::sim) file: Option<String>,
    pub(in crate::sim) line: u32,
}

/// A built-in system task or function the design calls and llg runs
/// natively, with its first call site. A VPI registration under the same name
/// would override it (IEEE 1800-2009 36.3.2), which llg does not support, so
/// the generated startup stops naming the call (decision S99-D6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrVpiBuiltinCall {
    pub(in crate::sim) name: String,
    pub(in crate::sim) file: Option<String>,
    pub(in crate::sim) line: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IrVpiCompileArg {
    pub(in crate::sim) width: u32,
    pub(in crate::sim) signed: bool,
    pub(in crate::sim) real: bool,
    pub(in crate::sim) reference: IrVpiArgRef,
}

impl IrVpiCompileCall {
    pub fn new(name: String, args: Vec<IrVpiCompileArg>) -> Self {
        Self {
            name,
            args,
            time_unit_fs: 0,
            scope: None,
            file: None,
            line: 0,
        }
    }
}

impl IrVpiObject {
    pub fn module(
        full_name: String,
        name: String,
        definition_name: String,
        file: Option<String>,
        line: u32,
    ) -> Self {
        Self {
            full_name,
            name,
            definition_name: Some(definition_name),
            file,
            line,
            kind: IrVpiObjectKind::Module,
            ..Self::default()
        }
    }
}
