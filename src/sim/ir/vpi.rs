//! Vpi.

/// Object kinds admitted to the generated model's bounded VPI catalog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrVpiObjectKind {
    Module,
    Net,
    Reg,
    RealVar,
    RegArray,
}

impl IrVpiObjectKind {
    pub const fn c_type(self) -> &'static str {
        match self {
            Self::Module => "vpiModule",
            Self::Net => "vpiNet",
            Self::Reg => "vpiReg",
            Self::RealVar => "vpiRealVar",
            Self::RegArray => "vpiRegArray",
        }
    }
}

/// Owned source and storage metadata for one generated VPI object.
///
/// Paths use the same unit-separator hierarchy encoding as [`super::IrSignal`].
/// `signal`/`array` refer to stable model-table indices; the emitter resolves
/// those indices to C storage only after optimization has completed.
#[derive(Clone, Debug, PartialEq)]
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
}

/// Type-only descriptor for one source-level VPI call site.  It is kept
/// separate from [`IrVpiObject`] because compiletf/sizetf receives argument
/// shapes before any runtime value exists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrVpiCompileCall {
    pub(in crate::sim) time_unit_fs: u64,
    pub(in crate::sim) name: String,
    pub(in crate::sim) args: Vec<IrVpiCompileArg>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IrVpiCompileArg {
    pub(in crate::sim) width: u32,
    pub(in crate::sim) signed: bool,
    pub(in crate::sim) real: bool,
}

impl IrVpiCompileCall {
    pub fn new(name: String, args: Vec<IrVpiCompileArg>) -> Self {
        Self {
            name,
            args,
            time_unit_fs: 0,
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
            time_unit_fs: 0,
            full_name,
            name,
            definition_name: Some(definition_name),
            file,
            line,
            kind: IrVpiObjectKind::Module,
            width: 0,
            signed: false,
            real: false,
            net: false,
            signal: None,
            array: None,
        }
    }
}
