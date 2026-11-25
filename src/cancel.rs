//! Stopping an analysis whose result is no longer wanted.
//!
//! The owner of an analysis keeps a [`CancelToken`] and gives a clone to the
//! stages it runs. A stage asks at intervals and, once the token is cancelled,
//! returns its error early, which releases what it built: there is no partial
//! result. The frontend asks through the C ABI (`LlgSlangCompileRequest`), the
//! stages after it through [`CancelToken::check`] and [`Poller`].

use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

/// Steps of a loop between two questions of a [`Poller`]: a step of the
/// costliest loops takes a few microseconds.
pub const POLL_INTERVAL: u32 = 2048;

/// The work stopped because its [`CancelToken`] was cancelled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cancelled;

impl fmt::Display for Cancelled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the analysis was cancelled")
    }
}

impl std::error::Error for Cancelled {}

/// A request to stop, shared by the owner of an analysis and its stages. The
/// default token is never cancelled.
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Option<Arc<State>>);

#[derive(Debug, Default)]
struct State {
    cancelled: AtomicBool,
    /// Cancels this token as well.
    parent: CancelToken,
    /// Counts the questions of a token made by [`CancelToken::at_poll`].
    polls: Option<PollCount>,
}

#[derive(Debug)]
struct PollCount {
    asked: AtomicU64,
    cancels_at: u64,
}

impl CancelToken {
    pub fn new() -> Self {
        Self(Some(Arc::default()))
    }

    /// A token that is also cancelled when this one is.
    pub fn child(&self) -> Self {
        Self(Some(Arc::new(State {
            parent: self.clone(),
            ..State::default()
        })))
    }

    /// A token that is cancelled by its `poll`th question, so that a test can
    /// stop a stage at each of its poll points.
    pub fn at_poll(poll: u64) -> Self {
        Self(Some(Arc::new(State {
            polls: Some(PollCount {
                asked: AtomicU64::new(0),
                cancels_at: poll,
            }),
            ..State::default()
        })))
    }

    /// Questions that a token made by [`Self::at_poll`] answered.
    pub fn polls(&self) -> u64 {
        self.0
            .as_ref()
            .and_then(|state| state.polls.as_ref())
            .map_or(0, |polls| polls.asked.load(Ordering::Relaxed))
    }

    /// Ask the work to stop. The default token ignores it.
    pub fn cancel(&self) {
        if let Some(state) = &self.0 {
            state.cancelled.store(true, Ordering::Relaxed);
        }
    }

    /// Whether the work should stop.
    #[inline]
    pub fn is_cancelled(&self) -> bool {
        self.0.as_deref().is_some_and(State::poll)
    }

    #[inline]
    pub fn check(&self) -> Result<(), Cancelled> {
        if self.is_cancelled() {
            Err(Cancelled)
        } else {
            Ok(())
        }
    }

    /// Whether any clone can cancel this token.
    pub(crate) fn can_cancel(&self) -> bool {
        self.0.is_some()
    }

    /// A [`Poller`] that asks this token at every [`POLL_INTERVAL`]th step.
    pub fn poller(&self) -> Poller<'_> {
        self.poller_every(POLL_INTERVAL)
    }

    /// A [`Poller`] that asks this token at every `interval`th step, for steps
    /// that take longer.
    pub fn poller_every(&self, interval: u32) -> Poller<'_> {
        Poller {
            token: self,
            interval,
            left: interval,
        }
    }
}

/// Handles of one token are equal.
impl PartialEq for CancelToken {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (None, None) => true,
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl Eq for CancelToken {}

impl State {
    fn poll(&self) -> bool {
        if let Some(polls) = &self.polls {
            if polls.asked.fetch_add(1, Ordering::Relaxed) + 1 == polls.cancels_at {
                self.cancelled.store(true, Ordering::Relaxed);
            }
        }
        self.cancelled.load(Ordering::Relaxed) || self.parent.is_cancelled()
    }
}

/// Asks a token at intervals of a loop's steps.
#[derive(Debug)]
pub struct Poller<'a> {
    token: &'a CancelToken,
    interval: u32,
    left: u32,
}

impl Poller<'_> {
    #[inline]
    pub fn step(&mut self) -> Result<(), Cancelled> {
        self.left -= 1;
        if self.left == 0 {
            self.left = self.interval;
            return self.token.check();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_is_cancelled_through_every_clone_and_the_default_never() {
        let token = CancelToken::new();
        let clone = token.clone();
        assert_eq!(clone.check(), Ok(()));
        token.cancel();
        assert_eq!(clone.check(), Err(Cancelled));
        assert_eq!(token, clone);
        assert_ne!(token, CancelToken::new());

        let never = CancelToken::default();
        never.cancel();
        assert!(!never.is_cancelled());
        assert_eq!(never, CancelToken::default());
    }

    #[test]
    fn a_parent_cancels_its_children_and_not_the_reverse() {
        let parent = CancelToken::new();
        let (first, second) = (parent.child(), parent.child());
        first.cancel();
        assert_eq!(
            [&parent, &first, &second].map(CancelToken::is_cancelled),
            [false, true, false]
        );
        parent.cancel();
        assert!(second.is_cancelled());
    }

    #[test]
    fn a_counting_token_cancels_at_its_poll() {
        let token = CancelToken::at_poll(3);
        assert_eq!(
            [(); 4].map(|()| token.is_cancelled()),
            [false, false, true, true]
        );
        assert_eq!(token.polls(), 4);
    }

    #[test]
    fn a_poller_asks_once_per_interval() {
        let token = CancelToken::at_poll(2);
        let mut poller = token.poller();
        for _ in 0..2 * POLL_INTERVAL - 1 {
            assert_eq!(poller.step(), Ok(()));
        }
        assert_eq!(poller.step(), Err(Cancelled));
        assert_eq!(token.polls(), 2);
    }
}
