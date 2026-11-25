//! Pure analysis-scheduling state machine for per-root recompiles.
//!
//! The backend decides which events call for an analysis (`handlers/inputs.rs`)
//! and collapses bursts of them with three cooperating rules:
//!
//! * **Delay before a run**: the first trigger of an idle root arms one timer;
//!   further triggers join it and do not restart it.  The job built when the
//!   timer fires snapshots the LATEST state, so a burst produces exactly one
//!   run over the freshest inputs.
//! * **Latest-wins around a running job**: a trigger never queues a job behind
//!   the running one.  It marks the root dirty and cancels the running job,
//!   whose inputs it made old; when that job ends, a dirty root schedules
//!   exactly ONE follow-up run (again delayed), which observes the newest
//!   inputs.
//! * **Progress under continuous triggers**: after
//!   [`MAX_CONSECUTIVE_CANCELLATIONS`] cancelled runs in a row the next one is
//!   left to finish, so that a user who saves more often than an analysis
//!   takes still gets results.
//!
//! Pure state only: no clocks, no tasks, no I/O — the caller owns timers and
//! jobs, which keeps every transition unit-testable.

/// Runs of one root that may be cancelled in a row. The run after them is not
/// cancelled.
pub const MAX_CONSECUTIVE_CANCELLATIONS: u32 = 3;

/// Decision returned by [`SchedulerState::trigger`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerDecision {
    /// The root was idle: arm one debounce timer.
    ArmTimer,
    /// A job is running on older inputs: cancel it. The root is marked dirty.
    CancelRunning,
    /// A timer is already pending, or a job is running that is being cancelled
    /// or must finish (the root is marked dirty): do nothing.
    Coalesce,
}

/// Decision returned by [`SchedulerState::timer_fired`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FireDecision {
    /// The caller won the Pending→Running transition: build and run exactly
    /// one job from the latest state.
    StartJob,
    /// Spurious fire (no timer was pending): do nothing.
    Ignore,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SchedulerState {
    /// A debounce timer is armed; the next `timer_fired` starts the run.
    pending: bool,
    /// A compile job is in flight; new triggers only mark the root dirty.
    running: bool,
    /// A trigger arrived while `running`; exactly one follow-up run is owed.
    dirty: bool,
    /// Runs that ended cancelled since the last one that did not.
    cancellations: u32,
}

impl SchedulerState {
    /// True when no timer is pending, no job runs and nothing is owed.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_idle(&self) -> bool {
        !self.pending && !self.running && !self.dirty
    }

    /// Record one analysis trigger.
    pub fn trigger(&mut self) -> TriggerDecision {
        if self.running {
            // Never queue behind a running job: remember only that newer
            // inputs exist.  The follow-up run reads the latest state.
            let first = !std::mem::replace(&mut self.dirty, true);
            return if first && self.cancellations < MAX_CONSECUTIVE_CANCELLATIONS {
                TriggerDecision::CancelRunning
            } else {
                TriggerDecision::Coalesce
            };
        }
        if self.pending {
            // The armed timer already covers this burst.
            return TriggerDecision::Coalesce;
        }
        self.pending = true;
        TriggerDecision::ArmTimer
    }

    /// The debounce timer elapsed.
    pub fn timer_fired(&mut self) -> FireDecision {
        if !self.pending || self.running {
            return FireDecision::Ignore;
        }
        self.pending = false;
        self.running = true;
        FireDecision::StartJob
    }

    /// Roll back an armed timer without starting a run (the root disappeared
    /// or the server is shutting down).
    pub fn cancel_pending(&mut self) {
        self.pending = false;
    }

    /// The running job ended, with its result or because it was cancelled.
    /// Returns `true` when the caller must arm one follow-up debounce timer
    /// because triggers arrived during the run.
    pub fn job_finished(&mut self, cancelled: bool) -> bool {
        if !self.running {
            return false;
        }
        self.running = false;
        self.cancellations = if cancelled { self.cancellations + 1 } else { 0 };
        if self.dirty {
            self.dirty = false;
            self.pending = true;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::{FireDecision, SchedulerState, TriggerDecision, MAX_CONSECUTIVE_CANCELLATIONS};

    #[test]
    fn idle_trigger_arms_exactly_one_timer() {
        let mut scheduler = SchedulerState::default();
        assert!(scheduler.is_idle());
        assert_eq!(scheduler.trigger(), TriggerDecision::ArmTimer);
        for _ in 0..10 {
            assert_eq!(scheduler.trigger(), TriggerDecision::Coalesce);
        }
        assert!(!scheduler.is_idle());
    }

    #[test]
    fn idle_state_does_not_generate_recurring_work() {
        let mut scheduler = SchedulerState::default();

        // Timer/task completion callbacks may race with cancellation or root
        // removal at the integration boundary. Replaying either callback
        // while idle must remain a no-op: the state machine cannot re-arm
        // itself and therefore cannot create an idle wakeup loop.
        for _ in 0..100 {
            assert_eq!(scheduler.timer_fired(), FireDecision::Ignore);
            assert!(!scheduler.job_finished(false));
            scheduler.cancel_pending();
            assert!(scheduler.is_idle());
        }
    }

    #[test]
    fn burst_of_ten_triggers_while_running_produces_one_followup_run() {
        let mut scheduler = SchedulerState::default();
        assert_eq!(scheduler.trigger(), TriggerDecision::ArmTimer);
        assert_eq!(scheduler.timer_fired(), FireDecision::StartJob);
        // Ten triggers arrive while the job runs: the first cancels it, none
        // may enqueue work.
        assert_eq!(scheduler.trigger(), TriggerDecision::CancelRunning);
        for _ in 0..9 {
            assert_eq!(scheduler.trigger(), TriggerDecision::Coalesce);
        }
        // Its end owes exactly one debounced follow-up.
        assert!(scheduler.job_finished(true));
        assert!(!scheduler.is_idle());
        // The follow-up fires and, with no further triggers, settles idle.
        assert_eq!(scheduler.trigger(), TriggerDecision::Coalesce);
        assert_eq!(scheduler.timer_fired(), FireDecision::StartJob);
        assert!(!scheduler.job_finished(false));
        assert!(scheduler.is_idle());
    }

    #[test]
    fn burst_triggers_produce_exactly_one_run() {
        let mut scheduler = SchedulerState::default();
        let mut timers_armed = 0;
        let mut jobs_started = 0;

        for _ in 0..100 {
            if scheduler.trigger() == TriggerDecision::ArmTimer {
                timers_armed += 1;
            }
        }
        if scheduler.timer_fired() == FireDecision::StartJob {
            jobs_started += 1;
        }
        assert!(!scheduler.job_finished(false));

        assert_eq!(timers_armed, 1);
        assert_eq!(jobs_started, 1);
        assert!(scheduler.is_idle());
    }

    #[test]
    fn job_without_triggers_settles_idle() {
        let mut scheduler = SchedulerState::default();
        scheduler.trigger();
        assert_eq!(scheduler.timer_fired(), FireDecision::StartJob);
        assert!(!scheduler.is_idle());
        assert!(!scheduler.job_finished(false));
        assert!(scheduler.is_idle());
    }

    #[test]
    fn sustained_storms_converge_to_one_followup_per_storm() {
        let mut scheduler = SchedulerState::default();
        scheduler.trigger();
        assert_eq!(scheduler.timer_fired(), FireDecision::StartJob);
        let mut runs = 1;
        // Three edit storms whose ten triggers each land while a job is
        // still executing: every storm may owe exactly ONE follow-up run,
        // and a pending follow-up absorbs any straggler trigger.
        for _ in 0..3 {
            assert_eq!(scheduler.trigger(), TriggerDecision::CancelRunning);
            for _ in 0..9 {
                assert_eq!(scheduler.trigger(), TriggerDecision::Coalesce);
            }
            // The job was done before it noticed.
            if scheduler.job_finished(false) {
                runs += 1;
                assert_eq!(scheduler.trigger(), TriggerDecision::Coalesce);
                assert_eq!(scheduler.timer_fired(), FireDecision::StartJob);
            }
        }
        assert_eq!(runs, 4, "each storm owes exactly one follow-up run");
        // Quiescence: settle the tail run.
        while scheduler.job_finished(false) {
            assert_eq!(scheduler.trigger(), TriggerDecision::Coalesce);
            assert_eq!(scheduler.timer_fired(), FireDecision::StartJob);
        }
        assert!(scheduler.is_idle());
    }

    #[test]
    fn triggers_during_the_debounce_window_coalesce_into_the_armed_timer() {
        let mut scheduler = SchedulerState::default();
        assert_eq!(scheduler.trigger(), TriggerDecision::ArmTimer);
        // Triggers landing before the timer fires cost nothing extra and are
        // picked up by the single job built at fire time (latest state).
        for _ in 0..9 {
            assert_eq!(scheduler.trigger(), TriggerDecision::Coalesce);
        }
        assert_eq!(scheduler.timer_fired(), FireDecision::StartJob);
        assert!(!scheduler.job_finished(false));
        scheduler.job_finished(false);
        assert!(scheduler.is_idle());
    }

    #[test]
    fn spurious_timer_fire_is_ignored() {
        let mut scheduler = SchedulerState::default();
        assert_eq!(scheduler.timer_fired(), FireDecision::Ignore);
        scheduler.trigger();
        scheduler.cancel_pending();
        assert_eq!(scheduler.timer_fired(), FireDecision::Ignore);
        assert!(scheduler.is_idle());
    }

    #[test]
    fn cancelled_timer_can_rearm_on_next_trigger() {
        let mut scheduler = SchedulerState::default();
        assert_eq!(scheduler.trigger(), TriggerDecision::ArmTimer);
        scheduler.cancel_pending();
        assert!(scheduler.is_idle());
        assert_eq!(scheduler.trigger(), TriggerDecision::ArmTimer);
        assert_eq!(scheduler.timer_fired(), FireDecision::StartJob);
    }

    #[test]
    fn job_finished_without_a_run_is_ignored() {
        let mut scheduler = SchedulerState::default();
        assert!(!scheduler.job_finished(false));
        scheduler.trigger();
        assert!(!scheduler.job_finished(false));
        assert_eq!(scheduler.trigger(), TriggerDecision::Coalesce);
    }

    #[test]
    fn dirty_flag_is_cleared_by_the_rescheduled_run() {
        let mut scheduler = SchedulerState::default();
        scheduler.trigger();
        assert_eq!(scheduler.timer_fired(), FireDecision::StartJob);
        assert_eq!(scheduler.trigger(), TriggerDecision::CancelRunning);
        assert!(scheduler.job_finished(true));
        assert!(!scheduler.is_idle());
        // The owed follow-up starts and finishes without owing another.
        assert_eq!(scheduler.timer_fired(), FireDecision::StartJob);
        assert!(!scheduler.job_finished(false));
        assert!(scheduler.is_idle());
    }

    /// One run that a trigger meets halfway; whether it was cancelled.
    fn interrupted_run(scheduler: &mut SchedulerState) -> bool {
        assert_eq!(scheduler.timer_fired(), FireDecision::StartJob);
        let cancelled = scheduler.trigger() == TriggerDecision::CancelRunning;
        assert!(scheduler.job_finished(cancelled));
        cancelled
    }

    #[test]
    fn continuous_triggers_let_every_run_after_the_allowed_cancellations_finish() {
        let mut scheduler = SchedulerState::default();
        assert_eq!(scheduler.trigger(), TriggerDecision::ArmTimer);
        for _ in 0..3 {
            for _ in 0..MAX_CONSECUTIVE_CANCELLATIONS {
                assert!(interrupted_run(&mut scheduler));
            }
            assert!(!interrupted_run(&mut scheduler));
        }
    }

    #[test]
    fn a_finished_run_resets_the_cancellation_count() {
        let mut scheduler = SchedulerState::default();
        scheduler.trigger();
        for _ in 1..MAX_CONSECUTIVE_CANCELLATIONS {
            assert!(interrupted_run(&mut scheduler));
        }
        assert_eq!(scheduler.timer_fired(), FireDecision::StartJob);
        assert!(!scheduler.job_finished(false));

        scheduler.trigger();
        for _ in 0..MAX_CONSECUTIVE_CANCELLATIONS {
            assert!(interrupted_run(&mut scheduler));
        }
        assert!(!interrupted_run(&mut scheduler));
    }
}
