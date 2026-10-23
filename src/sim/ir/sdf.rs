//! SDF back-annotation (IEEE 1800-2009 32, IEEE 1364-2001 16): the valued
//! operations of each `$sdf_annotate` call, read and resolved when the model
//! is generated and applied by the runtime when the call executes
//! ([`super::IrStmt::SdfAnnotate`]) or, for `--sdf`, before simulation
//! starts. Pending events keep their scheduled times; later events use the
//! new values.

/// The op adds its values to the current ones (INCREMENT); otherwise they
/// replace them (ABSOLUTE).
pub const SDF_INCREMENT: u8 = 1;
/// The op's limits are percentages of the path delay, in millionths of a
/// percent (PATHPULSEPERCENT).
pub const SDF_PERCENT: u8 = 2;

/// What one op changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrSdfTarget {
    /// Path `path` of destination `destination` of module path set `set`:
    /// twelve transition delays and their pulse limits.
    Path {
        set: u32,
        destination: u32,
        path: u32,
    },
    /// Check `check` of timing check set `set`: its `limit` or, with
    /// `second`, its `limit2`.
    Check { set: u32, check: u32, second: bool },
    /// The limit before (`after` false: setup, removal) or after the
    /// reference event of combined check `pair` of timing check set `set`,
    /// whose delayed signal delays the runtime solves again (31.9.1).
    Pair { set: u32, pair: u32, after: bool },
    /// Rise, fall and turn-off delays of annotated gate `slot`
    /// ([`super::IrModel::sdf_gate_delays`]).
    Gate { slot: u32 },
    /// The interconnect delays into load `load` ([`IrSdfLoad`]) from source
    /// entry `entry`; entry 0 is every source (PORT, NETDELAY, or an
    /// INTERCONNECT whose source was not found).
    Load { load: u32, entry: u32 },
}

/// One load port bit with interconnect delays: path `path` of destination
/// `destination` of module path set `set`, an interconnect path
/// ([`super::IrModulePath::interconnect`]) whose delays combine the
/// annotated delays of its `entries` sources by the annotation's
/// INTERCONNECT_MIPD policy (entry 0: from every source).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IrSdfLoad {
    pub set: u32,
    pub destination: u32,
    pub path: u32,
    pub entries: u32,
}

/// INTERCONNECT_MIPD policy of an op, in bits 3 and 4 of its flags: the
/// largest, smallest or average annotated source delay, or the last one
/// annotated.
pub const SDF_MIPD_SHIFT: u8 = 3;

/// One op: a target, the delay, reject and error masks of the transitions
/// it gives (bit `t` for transition `t`; one bit for a check limit, three
/// for a gate), the transitions whose limits derive from the new delay by
/// the pulse percentages, and its first value in
/// [`IrSdfAnnotation::values`] (delays, then rejects, then errors).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IrSdfOp {
    pub target: IrSdfTarget,
    pub flags: u8,
    pub delay: u16,
    pub reject: u16,
    pub error: u16,
    pub derive: u16,
    pub first: u32,
}

impl IrSdfOp {
    pub fn value_count(&self) -> usize {
        (self.delay.count_ones() + self.reject.count_ones() + self.error.count_ones()) as usize
    }
}

/// The valued ops of one call (or one `--sdf` file) in file order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IrSdfAnnotation {
    /// The file name, for the generated comment.
    pub file: String,
    pub ops: Vec<IrSdfOp>,
    /// Design-precision ticks (check limits may be negative).
    pub values: Vec<i64>,
}
