//! Typed combinational UDP truth tables.

/// One UDP input field, represented as a mask over 0, 1 and X.
/// Runtime Z inputs are normalized to X before matching.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum IrUdpInput {
    Zero = 1,
    One = 2,
    Unknown = 4,
    Binary = 3,
    Any = 7,
}

/// A combinational UDP output; Z and state-retention symbols are excluded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum IrUdpOutput {
    Zero = 0,
    One = 1,
    Unknown = 2,
}

/// One source-order row in a combinational UDP definition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrUdpRow {
    pub inputs: Vec<IrUdpInput>,
    pub output: IrUdpOutput,
}

/// An owned definition shared by all of its scalar and array instances.
/// Rows retain source order: the first match wins and no match returns X.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrUdpTable {
    pub name: String,
    pub input_count: usize,
    pub rows: Vec<IrUdpRow>,
}
