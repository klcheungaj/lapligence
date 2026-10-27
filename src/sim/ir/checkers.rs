//! Free checker variables (IEEE 1800-2009 17.7.2).

/// The assume-based randomization of one checker instance's active free
/// variables. The runtime calls the [`super::IrPreFn::DeferredAssertion`]
/// callback `solve`, which has no captures, before the Observed region: in
/// every time step when `assumes` is empty (implicitly clocked variables),
/// otherwise at every clock event of one of the assume statements named by
/// identity in `assumes`, which form the instance's assume set. While the
/// callback runs, [`super::IrRuntimeQuery::CheckerAssume`] reads the events
/// of the assumes by their position in `assumes`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrFreeSet {
    /// Label of a process of the checker instance (`tb.u.free`): the set
    /// draws from that instance's random stream (18.14.1).
    pub(in crate::sim) label: String,
    pub(in crate::sim) solve: String,
    pub(in crate::sim) assumes: Vec<u64>,
}

impl IrFreeSet {
    pub(in crate::sim) fn new(label: String, solve: String, assumes: Vec<u64>) -> Self {
        Self {
            label,
            solve,
            assumes,
        }
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn solve(&self) -> &str {
        &self.solve
    }

    pub fn assumes(&self) -> &[u64] {
        &self.assumes
    }
}
