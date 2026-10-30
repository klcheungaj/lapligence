//! Vpi.

/// Object kinds admitted to the generated model's VPI catalog (IEEE
/// 1800-2009 clause 37, Annex K/M object types).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IrVpiObjectKind {
    #[default]
    Module,
    /// An interface instance (`vpiInterface`).
    Interface,
    /// A program instance (`vpiProgram`, 37.9).
    Program,
    /// A package or the compilation unit `$unit` (`vpiPackage`, 37.10);
    /// [`IrVpiObject::qualifier`] marks `$unit` (`vpiUnit`).
    Package,
    /// A clocking block (`vpiClockingBlock`, 37.44): [`IrVpiObject::detail`]
    /// holds its default input and output edges, [`IrVpiObject::qualifier`]
    /// marks a global clocking, [`IrVpiObject::code`] the hidden entry whose
    /// children are its clocking event and default skews.
    ClockingBlock,
    /// A clocking io declaration (`vpiClockingIODecl`, 37.44): its
    /// direction, edges and skews as a clocking block's, `conns[0]` the
    /// expression it names (`vpiExpr`).
    ClockingIoDecl,
    /// A generate block instance (`vpiGenScope`), e.g. `g[0]` (SV 27.4).
    GenScope,
    /// A named `begin ... end` block (`vpiNamedBegin`).
    NamedBegin,
    /// A named `fork ... join` block (`vpiNamedFork`).
    NamedFork,
    /// An unnamed begin or fork that declares items, a `for` that declares
    /// its loop variables or a `foreach` (37.12 details 1-3): a scope named
    /// `unnamed$$_<n>` whose [`IrVpiObject::type_code`] is its statement's
    /// type (decision A19-D27).
    StatementScope,
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
    /// An unpacked structure or union variable whose members keep their own
    /// storage (`vpiStructVar`/`vpiUnionVar` by [`IrVpiVarKind`]); it has no
    /// value of its own (IEEE 1800-2009 37.24 detail 1).
    Record,
    /// A named event (`vpiNamedEvent`, 37.25) backed by [`IrVpiObject::event`].
    NamedEvent,
    /// An `initial`, `always` or `final` procedure (37.58); unnamed.
    Process,
    /// A continuous assignment (37.43); unnamed, `conns` holds lhs and rhs.
    ContAssign,
    /// A task or function declaration (37.37); a scope.
    TaskFunc,
    /// A gate, switch or UDP instance (37.31); `conns` holds its terminals.
    Primitive,
    /// A module, interface or primitive instance array (37.11); `dims`
    /// holds its declared ranges, outermost first.
    InstanceArray,
    /// A modport of an interface instance (37.7).
    Modport,
    /// A module path declaration (37.35); unnamed for lookup by name, its
    /// `name` the reported `<scope>.<in>$<out>`; `conns` holds its
    /// terminals and condition, [`IrVpiObject::timing`] its delays.
    ModPath,
    /// A timing check (37.36); unnamed, `conns` holds its arguments and
    /// terminal conditions, [`IrVpiObject::timing`] its limits.
    Tchk,
    /// A string, chandle or class variable, or a dynamic array, queue or
    /// associative array (37.17, 37.29): [`IrVpiObject::storage`] names its
    /// storage and [`IrVpiObject::shape`] its value shape.
    Dynamic,
    /// A generate-loop scope array (37.80): its generate scopes name it as
    /// their `port_target`, its own `port_target` names its genvar.
    GenScopeArray,
    /// A genvar (37.80); it has no value of its own.
    GenVar,
    /// An io declaration (37.13) of a module port, a task or function
    /// argument or a modport port, keyed under its scope with a hidden
    /// `io` part; [`IrVpiObject::port_index`] is its declaration order.
    IoDecl,
}

impl IrVpiObjectKind {
    pub const fn c_type(self) -> &'static str {
        match self {
            Self::Module => "vpiModule",
            Self::Interface => "vpiInterface",
            Self::Program => "vpiProgram",
            Self::Package => "vpiPackage",
            Self::ClockingBlock => "vpiClockingBlock",
            Self::ClockingIoDecl => "vpiClockingIODecl",
            Self::GenScope => "vpiGenScope",
            Self::NamedBegin => "vpiNamedBegin",
            Self::NamedFork => "vpiNamedFork",
            Self::StatementScope => "vpiBegin",
            Self::Net => "vpiNet",
            Self::Reg => "vpiReg",
            Self::RealVar => "vpiRealVar",
            Self::RegArray => "vpiRegArray",
            Self::NetArray => "vpiNetArray",
            Self::Parameter => "vpiParameter",
            Self::Port => "vpiPort",
            Self::Record => "vpiStructVar",
            Self::NamedEvent => "vpiNamedEvent",
            Self::Process => "vpiInitial",
            Self::ContAssign => "vpiContAssign",
            Self::TaskFunc => "vpiTask",
            Self::Primitive => "vpiGate",
            Self::InstanceArray => "vpiModuleArray",
            Self::Modport => "vpiModport",
            Self::ModPath => "vpiModPath",
            Self::Tchk => "vpiTchk",
            Self::Dynamic => "vpiUndefined",
            Self::GenScopeArray => "vpiGenScopeArray",
            Self::GenVar => "vpiGenVar",
            Self::IoDecl => "vpiIODecl",
        }
    }

    /// Scopes own other catalog objects and have no value.
    pub const fn is_scope(self) -> bool {
        matches!(
            self,
            Self::Module
                | Self::Interface
                | Self::Program
                | Self::Package
                | Self::ClockingBlock
                | Self::GenScope
                | Self::NamedBegin
                | Self::NamedFork
                | Self::StatementScope
                | Self::TaskFunc
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
    /// A `ref` task or function argument (`vpiRef`).
    Ref,
    #[default]
    None,
}

impl IrVpiDirection {
    pub const fn c_code(self) -> &'static str {
        match self {
            Self::Input => "vpiInput",
            Self::Output => "vpiOutput",
            Self::Inout => "vpiInout",
            Self::Ref => "vpiRef",
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
    /// Port order within its module, from 0; the declaration order of a
    /// clocking io declaration in its block.
    pub(in crate::sim) port_index: u32,
    /// Full name of the object a port's low connection refers to; of the
    /// default clocking block of a module, interface or program instance.
    pub(in crate::sim) port_target: Option<String>,
    pub(in crate::sim) constant: Option<IrVpiConstant>,
    /// Index into [`super::IrModel::vpi_typespecs`] (`vpiTypespec`), or
    /// `None` for a type llg exposes no typespec for.
    pub(in crate::sim) typespec: Option<usize>,
    /// A member of the structure or union variable named by the full-name
    /// prefix (`vpiStructUnionMember`).
    pub(in crate::sim) member: bool,
    /// The value is the low `width` bits of a wider signal: a member of an
    /// unpacked union, which shares one storage extent.
    pub(in crate::sim) slice: bool,
    /// The exact `vpiType` code name when [`IrVpiObjectKind::c_type`] is
    /// only the family default (`vpiAlways`, `vpiFunction`, `vpiUdp`, ...).
    pub(in crate::sim) type_code: Option<&'static str>,
    /// Kind-specific property code name: `vpiAlwaysType` of a process,
    /// `vpiPrimType` of a primitive, `vpiFuncType` of a function,
    /// `vpiPortType` of a port, element `vpiType` of an instance array,
    /// the `vpiInputEdge | vpiOutputEdge << 8` of a clocking block or
    /// clocking io declaration.
    pub(in crate::sim) detail: Option<&'static str>,
    /// Index into [`super::IrModel::events`] of a named event.
    pub(in crate::sim) event: Option<usize>,
    /// Expression connections: the high connection of a port, the lhs and
    /// rhs of a continuous assignment, the terminals of a primitive, the
    /// `default disable iff` expression of an instance, the expression of a
    /// clocking io declaration.
    pub(in crate::sim) conns: Vec<IrVpiConn>,
    /// The object has no name (processes, continuous assignments, unnamed
    /// primitives): name lookup never finds it.
    pub(in crate::sim) unnamed: bool,
    /// Declared ranges of an instance array, outermost first.
    pub(in crate::sim) dims: Vec<(i64, i64)>,
    /// `vpiNetDeclAssign` of a continuous assignment, `vpiAutomatic` of a
    /// task or function, `vpiUnit` of a package, a global clocking block.
    pub(in crate::sim) qualifier: bool,
    /// Runtime activation identity `(declaration, instance)` of a named
    /// block, task or function: what `disable` names (cbDisable).
    pub(in crate::sim) activation: Option<(u32, u32)>,
    /// Delays of a module path, timing check, primitive or continuous
    /// assignment (`vpi_get_delays`/`vpi_put_delays`).
    pub(in crate::sim) timing: Option<IrVpiTiming>,
    /// Storage of a [`IrVpiObjectKind::Dynamic`] object.
    pub(in crate::sim) storage: Option<super::IrVpiStorage>,
    /// Shape index (into [`super::IrVpiDynamic::shapes`]) of a dynamic object.
    pub(in crate::sim) shape: Option<usize>,
    /// Index into [`super::IrModel::vpi_code`]: the `vpiStmt` of a process,
    /// task or function, or the statement object of a named block.
    pub(in crate::sim) code: Option<u32>,
    /// Definition and configuration properties of a module or interface
    /// instance (37.10).
    pub(in crate::sim) instance: Option<Box<IrVpiInstance>>,
    /// `vpiStrength0` and `vpiStrength1` code names of a primitive or
    /// continuous assignment (37.31, 37.43).
    pub(in crate::sim) strength: Option<(&'static str, &'static str)>,
    /// `vpiImplicitDecl` of a net (37.16).
    pub(in crate::sim) implicit: bool,
    /// `vpiExplicitScalared`/`vpiExplicitVectored` of a net (37.16),
    /// `vpiExplicitName`/`vpiConnByName` of a port (37.14).
    pub(in crate::sim) qualifiers: IrVpiQualifiers,
}

/// Declaration keywords and port naming of a net or port row (IEEE
/// 1800-2009 37.14, 37.16).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IrVpiQualifiers {
    pub(in crate::sim) scalared: bool,
    pub(in crate::sim) vectored: bool,
    pub(in crate::sim) explicit_name: bool,
    pub(in crate::sim) by_name: bool,
}

/// The definition and configuration properties of an instance (IEEE
/// 1800-2009 37.10, 33.7): code names are Annex K constant names.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IrVpiInstance {
    /// `vpiDefFile`, `vpiDefLineNo`: where the definition is declared.
    pub(in crate::sim) def_file: Option<String>,
    pub(in crate::sim) def_line: u32,
    /// `vpiLibrary`: the library the definition was compiled into.
    pub(in crate::sim) library: String,
    /// `vpiConfig`: `library.config` of the controlling configuration.
    pub(in crate::sim) config: Option<String>,
    /// `vpiCell`: the name of the cell bound to the instance.
    pub(in crate::sim) cell: String,
    /// `vpiCellInstance`.
    pub(in crate::sim) cell_define: bool,
    /// `vpiAutomatic`: the definition's default lifetime is automatic.
    pub(in crate::sim) automatic: bool,
    /// `vpiDefNetType` (`vpiNone` for `` `default_nettype none``).
    pub(in crate::sim) net_type: &'static str,
    /// `vpiUnconnDrive`: `vpiHighZ`, `vpiPull0` or `vpiPull1`.
    pub(in crate::sim) unconnected_drive: &'static str,
}

/// The delays or limits of a catalog object and the runtime values that
/// hold them (IEEE 1800-2009 38.10, 38.32).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IrVpiTiming {
    /// The model values back-annotation writes, each with the index of the
    /// timing check limit it holds (0 for paths and primitives): the path
    /// rows of a module path, the check rows or combined pairs of each
    /// limit of a timing check, the delay slot of a primitive.
    pub(in crate::sim) targets: Vec<(u32, super::IrSdfTarget)>,
    /// The values as elaborated, in design-precision ticks: the twelve
    /// transition delays of a path, the limits of a check, the rise, fall
    /// and turn-off delays of a primitive or continuous assignment.
    pub(in crate::sim) declared: Vec<i64>,
    /// How many delays or limits the source gives (0: none).
    pub(in crate::sim) count: u32,
    /// `vpiPolarity` and `vpiDataPolarity` code names of a path.
    pub(in crate::sim) polarity: &'static str,
    pub(in crate::sim) data_polarity: &'static str,
    /// `vpiModPathHasIfNone`: the path is an `ifnone` path.
    pub(in crate::sim) has_ifnone: bool,
}

/// What a connection of a module path or timing check stands for
/// (IEEE 1800-2009 37.35, 37.36).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IrVpiConnRole {
    /// Any other structure connection.
    #[default]
    Connection,
    PathIn,
    PathOut,
    PathData,
    /// The condition of a state-dependent path.
    Condition,
    TchkRef,
    TchkData,
    /// Another timing check argument (a limit or flag).
    Argument,
    Notifier,
    /// The `&&&` condition of a timing check terminal.
    TermCondition,
}

impl IrVpiConnRole {
    pub const fn c_code(self) -> &'static str {
        match self {
            Self::Connection => "LLG_VPI_ROLE_CONNECTION",
            Self::PathIn => "LLG_VPI_ROLE_PATH_IN",
            Self::PathOut => "LLG_VPI_ROLE_PATH_OUT",
            Self::PathData => "LLG_VPI_ROLE_PATH_DATA",
            Self::Condition => "LLG_VPI_ROLE_CONDITION",
            Self::TchkRef => "LLG_VPI_ROLE_TCHK_REF",
            Self::TchkData => "LLG_VPI_ROLE_TCHK_DATA",
            Self::Argument => "LLG_VPI_ROLE_ARGUMENT",
            Self::Notifier => "LLG_VPI_ROLE_NOTIFIER",
            Self::TermCondition => "LLG_VPI_ROLE_TERM_COND",
        }
    }
}

/// One expression connection of a structure object: how the application
/// sees the expression (an object, a select, an element, a constant or an
/// opaque expression), with its self-determined width.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IrVpiConn {
    pub(in crate::sim) reference: IrVpiArgRef,
    pub(in crate::sim) width: u32,
    pub(in crate::sim) signed: bool,
    /// Terminal direction of a primitive terminal.
    pub(in crate::sim) direction: IrVpiDirection,
    /// Value digits (`0`/`1`/`x`/`z`, most significant first) of a constant.
    pub(in crate::sim) value: Option<String>,
    /// Role in a module path or timing check.
    pub(in crate::sim) role: IrVpiConnRole,
    /// `vpiEdge` bits of a path or timing check terminal.
    pub(in crate::sim) edge: u8,
    /// Index of the connection holding a timing check terminal's condition.
    pub(in crate::sim) condition: Option<u32>,
    /// Index into [`super::IrModel::vpi_code`] of the expression object of
    /// an [`IrVpiArgRef::Expression`] connection.
    pub(in crate::sim) code: Option<u32>,
}

/// One expression or statement object (IEEE 1800-2009 37.54-37.74) of a
/// model generated for VPI applications. The children of an entry are the
/// contiguous entries `first..first + count`; each child's `role` names the
/// relation that reaches it from its parent (`vpiOperand`, `vpiLhs`,
/// `vpiStmt`, ...). Code names are the Annex K/M constant names the
/// emitter writes verbatim.
#[derive(Clone, Debug, PartialEq)]
pub struct IrVpiCode {
    pub(in crate::sim) type_code: &'static str,
    /// `vpiOpType`, `vpiConstType`, `vpiCaseType` or
    /// `vpiIndexedPartSelectType`, or `"0"`.
    pub(in crate::sim) subtype: &'static str,
    /// The relation from the parent, or `"0"` for a root.
    pub(in crate::sim) role: &'static str,
    /// `vpiQualifier` of an `if`, `if-else` or `case`, or `"0"`.
    pub(in crate::sim) qualifier: &'static str,
    /// [`IrVpiCode::SIGNED`] and the other flags, as the runtime's
    /// `LLG_VPI_CODE_*`.
    pub(in crate::sim) flags: u32,
    pub(in crate::sim) width: u32,
    /// Index into [`super::IrModel::vpi_code_files`].
    pub(in crate::sim) file: Option<u32>,
    pub(in crate::sim) line: u32,
    /// Index into [`super::IrModel::vpi_objects`] of the catalog object the
    /// entry stands for ([`IrVpiCode::ALIAS`]) or names.
    pub(in crate::sim) object: Option<usize>,
    /// Index into [`super::IrModel::vpi_objects`] of the enclosing scope.
    pub(in crate::sim) scope: Option<usize>,
    pub(in crate::sim) first: u32,
    pub(in crate::sim) count: u32,
    /// Constant digits (`0`/`1`/`x`/`z`, most significant first), string
    /// or real text; the name of a call, a reference or a label.
    pub(in crate::sim) text: Option<String>,
    /// Declared `(left, right)` of the dimension a select indexes.
    pub(in crate::sim) range: Option<(i64, i64)>,
    /// Index into [`super::IrModel::vpi_typespecs`] of a cast's type, of
    /// the value of a select or call, or of a formal's declared type.
    pub(in crate::sim) typespec: Option<usize>,
    /// The `(column, end line, end column)` of an assertion object (37.45
    /// location): where its source starts on `line` and where it ends, the
    /// end column being that of its last character.
    pub(in crate::sim) span: Option<(u32, u32, u32)>,
}

impl IrVpiCode {
    pub const SIGNED: u32 = 0x01;
    pub const REAL: u32 = 0x02;
    /// An implicit conversion: evaluated, never an object of its own.
    pub const HIDDEN: u32 = 0x04;
    /// An expression or statement llg does not decompose: no children and
    /// no value.
    pub const OPAQUE: u32 = 0x08;
    /// Stands for catalog object `object` (a reference, a named block).
    pub const ALIAS: u32 = 0x10;
    pub const BLOCKING: u32 = 0x20;
    pub const CONSTANT: u32 = 0x40;
    pub const USER: u32 = 0x80;
    pub const LOCALS: u32 = 0x100;
    pub const TWO_STATE: u32 = 0x200;
    /// A constant whose text is the characters of a string.
    pub const STRING: u32 = 0x400;
    /// An assertion whose clocking event is inferred, not written in it
    /// (37.46 `vpiIsClockInferred`).
    pub const INFERRED: u32 = 0x800;
    /// Stands for the entry `first` names (an instance's declaration); no
    /// children.
    pub const LINK: u32 = 0x1000;
    /// An unbased unsized literal (`'0`, `'1`, `'x`, `'z`): decompiles as
    /// written.
    pub const FILL: u32 = 0x2000;
    /// The `LLG_VPI_CODE_*` names of `flags`, lowest bit first.
    pub const FLAG_NAMES: [&'static str; 14] = [
        "LLG_VPI_CODE_SIGNED",
        "LLG_VPI_CODE_REAL",
        "LLG_VPI_CODE_HIDDEN",
        "LLG_VPI_CODE_OPAQUE",
        "LLG_VPI_CODE_ALIAS",
        "LLG_VPI_CODE_BLOCKING",
        "LLG_VPI_CODE_CONSTANT",
        "LLG_VPI_CODE_USER",
        "LLG_VPI_CODE_LOCALS",
        "LLG_VPI_CODE_TWO_STATE",
        "LLG_VPI_CODE_STRING",
        "LLG_VPI_CODE_INFERRED",
        "LLG_VPI_CODE_LINK",
        "LLG_VPI_CODE_FILL",
    ];

    pub fn new(type_code: &'static str) -> Self {
        Self {
            type_code,
            subtype: "0",
            role: "0",
            qualifier: "0",
            flags: 0,
            width: 0,
            file: None,
            line: 0,
            object: None,
            scope: None,
            first: 0,
            count: 0,
            text: None,
            range: None,
            typespec: None,
            span: None,
        }
    }
}

/// `vpiType` of a typespec (IEEE 1800-2009 37.23, Annex M).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum IrVpiTypespecKind {
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
    String,
    Chandle,
    Enum,
    Struct,
    Union,
    /// A packed array of structures, unions or enums.
    PackedArray,
    /// An unpacked array (`vpiArrayTypespec`).
    Array,
}

impl IrVpiTypespecKind {
    pub const fn c_type(self) -> &'static str {
        match self {
            Self::Logic => "vpiLogicTypespec",
            Self::Bit => "vpiBitTypespec",
            Self::Int => "vpiIntTypespec",
            Self::Integer => "vpiIntegerTypespec",
            Self::ShortInt => "vpiShortIntTypespec",
            Self::LongInt => "vpiLongIntTypespec",
            Self::Byte => "vpiByteTypespec",
            Self::Time => "vpiTimeTypespec",
            Self::Real => "vpiRealTypespec",
            Self::ShortReal => "vpiShortRealTypespec",
            Self::String => "vpiStringTypespec",
            Self::Chandle => "vpiChandleTypespec",
            Self::Enum => "vpiEnumTypespec",
            Self::Struct => "vpiStructTypespec",
            Self::Union => "vpiUnionTypespec",
            Self::PackedArray => "vpiPackedArrayTypespec",
            Self::Array => "vpiArrayTypespec",
        }
    }

    /// The `vpiType` of a variable (or structure member) of this type.
    pub const fn var_type(self) -> &'static str {
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
            Self::String => "vpiStringVar",
            Self::Chandle => "vpiChandleVar",
            Self::Enum => "vpiEnumVar",
            Self::Struct => "vpiStructVar",
            Self::Union => "vpiUnionVar",
            Self::PackedArray => "vpiPackedArrayVar",
            Self::Array => "vpiRegArray",
        }
    }
}

/// One typespec of the VPI catalog. Typespecs are interned: equal entries
/// share one index, so `vpi_compare_objects` sees one object per type.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct IrVpiTypespec {
    pub(in crate::sim) kind: IrVpiTypespecKind,
    /// Typedef name (`vpiName`); `None` for a type without one.
    pub(in crate::sim) name: Option<String>,
    /// `vpiTypedefAlias`: the typespec of the typedef or built-in type this
    /// typedef renames.
    pub(in crate::sim) alias: Option<usize>,
    /// `vpiElemTypespec`: this type without its leftmost dimension.
    pub(in crate::sim) element: Option<usize>,
    /// `vpiBaseTypespec` of an enum.
    pub(in crate::sim) base: Option<usize>,
    /// Bits of a fixed packed value, or of a fixed unpacked image; 0 for
    /// other types.
    pub(in crate::sim) width: u32,
    pub(in crate::sim) signed: bool,
    pub(in crate::sim) two_state: bool,
    /// `vpiPacked` of a structure or union; `None` when unknown (a tagged
    /// union).
    pub(in crate::sim) packed: Option<bool>,
    pub(in crate::sim) tagged: bool,
    /// `(left, right)` of each dimension at this level, leftmost first:
    /// packed ranges of a logic/bit typespec, unpacked ranges of an array.
    pub(in crate::sim) ranges: Vec<(i64, i64)>,
    pub(in crate::sim) members: Vec<IrVpiTypespecMember>,
    /// Enum constants: name and value digits, most significant first.
    pub(in crate::sim) enum_consts: Vec<(String, String)>,
}

/// One `vpiTypespecMember` of a structure or union typespec.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IrVpiTypespecMember {
    pub(in crate::sim) name: String,
    pub(in crate::sim) typespec: usize,
    /// Least significant bit of the member in its container's packed value
    /// (or fixed unpacked image), when the container has one.
    pub(in crate::sim) lsb: Option<u32>,
    pub(in crate::sim) width: u32,
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
    /// Index into [`super::IrModel::vpi_code`] of the argument's expression
    /// object, which answers its `vpiOpType` and `vpiOperand` relations.
    pub(in crate::sim) code: Option<u32>,
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
