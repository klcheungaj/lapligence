//! Counts of the work that stages do, for tests of how it grows with a design.
//!
//! Each thread keeps its own counts, in unit tests only; elsewhere [`count`]
//! does nothing.

macro_rules! kinds {
    ($($(#[$doc:meta])* $kind:ident,)*) => {
        /// A kind of work.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub(crate) enum Work {
            $($(#[$doc])* $kind,)*
        }

        #[cfg(test)]
        impl Work {
            const ALL: &[Work] = &[$(Work::$kind),*];
        }
    };
}

kinds! {
    /// Lookups of a database node.
    NodeReads,
    /// Nodes entered in the lists of all nodes.
    ListedNodes,
    /// Bytes of source text indexed for line numbers.
    SourceIndexBytes,
    /// Line starts compared with a source offset.
    SourceLineSteps,
    /// Descriptions of nodes that messages name.
    NodeDescriptions,
    /// Statements that a walk for the effects of a body visits.
    EffectStatements,
    /// Bytes of rendered bodies normalized for instance sharing.
    NormalizedBytes,
    /// Bytes of rendered C searched for names.
    NameScanBytes,
    /// Names looked up among the fields of a coroutine frame that are none.
    FieldMisses,
    /// Statements weighed for outlining.
    OutlineWeighing,
    /// Scopes passed on the way outwards from a node.
    EnclosingSteps,
    /// Nodes of lazy operands passed for a selector that a call computes.
    SelectorNodes,
    /// Scopes tried as the one that declares a record.
    AggregateScopes,
    /// Indices compared to find the part of a select that is an element.
    SelectIndices,
    /// Operands passed to tell whether a string operation has a real one.
    StringOperands,
    /// Subroutine bodies entered to follow the calls in them.
    CallBodies,
    /// Nodes of fork branches passed for what they name outside.
    BranchNodes,
}

/// Note `amount` of `work` done by this thread.
#[inline(always)]
pub(crate) fn count(work: Work, amount: usize) {
    #[cfg(test)]
    COUNTS.with(|counts| counts[work as usize].set(counts[work as usize].get() + amount));
    #[cfg(not(test))]
    let _ = (work, amount);
}

#[cfg(test)]
thread_local! {
    static COUNTS: [std::cell::Cell<usize>; Work::ALL.len()] =
        const { [const { std::cell::Cell::new(0) }; Work::ALL.len()] };
}

/// How much `work` this thread does in `run`.
#[cfg(test)]
pub(crate) fn counted(work: Work, run: impl FnOnce()) -> usize {
    let all = all_counted(run);
    all[work as usize].1
}

/// How much work of each kind this thread does in `run`.
#[cfg(test)]
pub(crate) fn all_counted(run: impl FnOnce()) -> Vec<(Work, usize)> {
    let read = |work: &Work| COUNTS.with(|counts| counts[*work as usize].get());
    let before = Work::ALL.iter().map(read).collect::<Vec<_>>();
    run();
    Work::ALL
        .iter()
        .zip(before)
        .map(|(work, before)| (*work, read(work) - before))
        .collect()
}
