//! Initialization.

use super::*;

/// Scheduling phase for a declaration initializer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrInitPhase {
    /// SystemVerilog static initialization, before ordinary processes spawn.
    BeforeProcesses,
    /// Verilog declaration initialization, represented as an active process.
    ActiveRegion,
}

/// Storage targeted by a typed declaration initializer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IrInitTarget {
    /// A model signal (module or synthesized static storage).
    Signal(usize),
    /// A persistent local belonging to one lowered function.
    StaticLocal { function: usize, name: String },
}

/// One declaration initializer with its semantic identity and scheduling
/// metadata.  The source origin carries the declaration's source range (or a
/// truthful synthetic origin when the frontend omitted source provenance).
#[derive(Clone, Debug, PartialEq)]
pub struct IrInitialization {
    pub(in crate::sim) declaration: u32,
    pub(in crate::sim) lifetime: StorageLifetime,
    pub(in crate::sim) phase: IrInitPhase,
    pub(in crate::sim) target: IrInitTarget,
    pub(in crate::sim) value: IrExpr,
    pub(in crate::sim) origin: crate::sim::semantic::Origin,
}

impl IrInitialization {
    pub fn new(
        declaration: u32,
        lifetime: StorageLifetime,
        phase: IrInitPhase,
        target: IrInitTarget,
        value: IrExpr,
        origin: crate::sim::semantic::Origin,
    ) -> Self {
        Self {
            declaration,
            lifetime,
            phase,
            target,
            value,
            origin,
        }
    }

    pub fn declaration(&self) -> u32 {
        self.declaration
    }

    pub fn lifetime(&self) -> StorageLifetime {
        self.lifetime
    }

    pub fn phase(&self) -> IrInitPhase {
        self.phase
    }

    pub fn target(&self) -> &IrInitTarget {
        &self.target
    }

    pub fn value(&self) -> &IrExpr {
        &self.value
    }

    pub fn origin(&self) -> &crate::sim::semantic::Origin {
        &self.origin
    }
}

/// One `main()` initialization step, applied before any process runs.
#[derive(Clone, Debug, PartialEq)]
pub enum IrInitStep {
    /// Fill an unpacked array with the variable type's X or two-state zero default.
    FillArrayX(usize),
    /// Fill an unpacked array with all-Z elements before net drivers execute.
    FillArrayZ(usize),
    /// Apply one declaration-initializer pattern element.
    SetArrayElem {
        arr: usize,
        index: u64,
        value: IrConst,
    },
    /// Fill a scalar net/var declaration initializer.
    SetScalar { sig: usize, value: IrConst },
    /// Register a source signal with the runtime's preponed sampling history.
    RegisterSampled(usize),
    /// Fill a collapsed-net member through its driver slot.
    WriteNet {
        group: usize,
        slot: usize,
        value: IrConst,
    },
    /// Apply a declaration initializer according to its recorded lifetime and
    /// edition-specific scheduling phase.
    Initialize(IrInitialization),
}
