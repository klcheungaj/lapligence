//! Typed user-defined primitive tables: combinational truth tables and
//! sequential state/transition tables.

/// One UDP input field, represented as a mask over 0, 1 and X.
/// Runtime Z inputs are normalized to X before matching. The table symbols
/// are `0 1 x b ?`; the two remaining masks only describe the transition
/// ends of the `p` and `n` shorthands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum IrUdpInput {
    Zero = 1,
    One = 2,
    Unknown = 4,
    Binary = 3,
    Any = 7,
    ZeroOrUnknown = 5,
    OneOrUnknown = 6,
}

impl IrUdpInput {
    /// Whether scalar state `state` (0, 1 or 2 for X) is in this field.
    pub fn matches(self, state: u8) -> bool {
        state < 3 && (self as u8) & (1 << state) != 0
    }
}

/// A combinational UDP output; Z and state-retention symbols are excluded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum IrUdpOutput {
    Zero = 0,
    One = 1,
    Unknown = 2,
}

/// One source-order row in a combinational UDP definition.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IrUdpRow {
    pub inputs: Vec<IrUdpInput>,
    pub output: IrUdpOutput,
}

/// An owned definition shared by all of its scalar and array instances.
/// Combinational rows retain source order: the first match wins and no
/// match returns X. A sequential definition keeps `rows` empty and describes
/// its behavior in `sequential`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IrUdpTable {
    pub name: String,
    pub input_count: usize,
    pub rows: Vec<IrUdpRow>,
    pub sequential: Option<Box<IrUdpSequential>>,
}

/// State-holding behavior of a sequential UDP (IEEE 1364-2001 8.3-8.8,
/// IEEE 1800-2009 29.5-29.10).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IrUdpSequential {
    /// Output state when simulation begins: the `initial` value, else X.
    pub initial: IrUdpOutput,
    /// Source-order rows; level rows have no `edge`.
    pub rows: Vec<IrUdpSequentialRow>,
}

/// The transition field of an edge row on input `input`. The row's level
/// field for that input is the value the input changes to; `from` is the
/// value it changes from. A transition matches only when the value changes,
/// so `*` is `(??)`, `p` is `(01)(0x)(x1)` = from `{0,x}` to `{1,x}`, and
/// `n` is from `{1,x}` to `{0,x}`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IrUdpEdge {
    pub input: usize,
    pub from: IrUdpInput,
}

/// Next state of a sequential row: an output value or `-` (no change).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IrUdpNext {
    Output(IrUdpOutput),
    Hold,
}

/// One sequential table entry.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IrUdpSequentialRow {
    /// Level field per input; for the edge input, the value changed to.
    pub inputs: Vec<IrUdpInput>,
    pub edge: Option<IrUdpEdge>,
    /// Current-state field.
    pub state: IrUdpInput,
    pub next: IrUdpNext,
}

impl IrUdpSequential {
    /// Inputs that some row names with a transition, in port order. A change
    /// of any other input can only match level rows.
    pub fn edge_inputs(&self, input_count: usize) -> Vec<usize> {
        let mut edge = vec![false; input_count];
        for transition in self.rows.iter().filter_map(|row| row.edge) {
            if let Some(flag) = edge.get_mut(transition.input) {
                *flag = true;
            }
        }
        (0..input_count).filter(|input| edge[*input]).collect()
    }

    /// Next state after input `changed` moves from `old` to its value in
    /// `inputs` (states 0, 1, 2 = X; the caller asks only when the value
    /// changed) while the output holds `state`. Edge rows are processed
    /// first and level rows after them, and the level row decides when both
    /// match (1364-2001 8.7): a matching level row wins, then a matching edge
    /// row on `changed`; an unmatched combination gives X (8.1.4). Within a
    /// kind the first matching row decides: Slang rejects entries that
    /// overlap with different outputs (`UdpDupDiffOutput`).
    pub fn next_state(&self, inputs: &[u8], changed: usize, old: u8, state: u8) -> u8 {
        let levels = |row: &IrUdpSequentialRow| {
            row.state.matches(state)
                && row
                    .inputs
                    .iter()
                    .zip(inputs)
                    .all(|(field, value)| field.matches(*value))
        };
        let resolve = |row: &IrUdpSequentialRow| match row.next {
            IrUdpNext::Output(output) => output as u8,
            IrUdpNext::Hold => state,
        };
        if let Some(row) = self
            .rows
            .iter()
            .find(|row| row.edge.is_none() && levels(row))
        {
            return resolve(row);
        }
        self.rows
            .iter()
            .find(|row| {
                row.edge
                    .is_some_and(|edge| edge.input == changed && edge.from.matches(old))
                    && levels(row)
            })
            .map_or(IrUdpOutput::Unknown as u8, resolve)
    }
}
