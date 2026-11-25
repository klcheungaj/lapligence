//! Counts of the work that stages do, for tests of how it grows with a design.
//!
//! Each thread keeps its own counts, in unit tests only; elsewhere [`count`]
//! does nothing.

/// A kind of work.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Work {
    /// Bytes of rendered bodies normalized for instance sharing.
    NormalizedBytes,
    /// Statements weighed for outlining.
    OutlineWeighing,
    /// Scopes passed on the way outwards from a node.
    EnclosingSteps,
    /// Nodes of lazy operands passed for a selector that a call computes.
    SelectorNodes,
    /// Subroutine bodies entered to follow the calls in them.
    CallBodies,
    /// Nodes of fork branches passed for what they name outside.
    BranchNodes,
}

/// Note `amount` of `work` done by this thread.
#[inline(always)]
pub(crate) fn count(work: Work, amount: usize) {
    #[cfg(test)]
    COUNTS.with_borrow_mut(|counts| *counts.entry(work).or_default() += amount);
    #[cfg(not(test))]
    let _ = (work, amount);
}

#[cfg(test)]
thread_local! {
    static COUNTS: std::cell::RefCell<std::collections::BTreeMap<Work, usize>> =
        const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
}

/// How much `work` this thread does in `run`.
#[cfg(test)]
pub(crate) fn counted(work: Work, run: impl FnOnce()) -> usize {
    let read = || COUNTS.with_borrow(|counts| counts.get(&work).copied().unwrap_or(0));
    let before = read();
    run();
    read() - before
}
