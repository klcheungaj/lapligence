//! Pure analysis-scheduling state machine for per-root recompiles.
//!
//! The backend collapses bursts of analysis triggers (didOpen/didChange/
//! didClose, watched-file events, config reloads, workspace-folder changes)
//! with two cooperating rules:
//!
//! * **Trailing-edge debounce** — while a root is idle, the first trigger arms
//!   one timer; further triggers during the quiet period coalesce into it.
//!   The job built when the timer fires snapshots the LATEST state, so a
//!   burst produces exactly one run over the freshest inputs.
//! * **Latest-wins coalescing around a running job** — while a compile is in
//!   flight, triggers NEVER queue a new job; they only mark the root dirty.
//!   When the job finishes, a dirty root schedules exactly ONE follow-up run
//!   (again debounced), which observes the newest inputs.  Continuous edits
//!   therefore converge: every completed run is followed by at most one more,
//!   and any pause lets the final state catch up.
//!
//! Pure state only: no clocks, no tasks, no I/O — the caller owns timers and
//! jobs, which keeps every transition unit-testable.

/// Decision returned by [`SchedulerState::trigger`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerDecision {
    /// The root was idle: arm one debounce timer.
    ArmTimer,
    /// A timer is already pending or a job is running (the root is marked
    /// dirty): do nothing.
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
            self.dirty = true;
            return TriggerDecision::Coalesce;
        }
        if self.pending {
            // Trailing edge: the armed timer already covers this burst.
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

    /// The running job finished.  Returns `true` when the caller must arm one
    /// follow-up debounce timer because triggers arrived during the run.
    pub fn job_finished(&mut self) -> bool {
        if !self.running {
            return false;
        }
        self.running = false;
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
    use super::{FireDecision, SchedulerState, TriggerDecision};

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
            assert!(!scheduler.job_finished());
            scheduler.cancel_pending();
            assert!(scheduler.is_idle());
        }
    }

    #[test]
    fn burst_of_ten_triggers_while_running_produces_one_followup_run() {
        let mut scheduler = SchedulerState::default();
        assert_eq!(scheduler.trigger(), TriggerDecision::ArmTimer);
        assert_eq!(scheduler.timer_fired(), FireDecision::StartJob);
        // Ten triggers arrive while the job runs: none may enqueue work.
        for _ in 0..10 {
            assert_eq!(scheduler.trigger(), TriggerDecision::Coalesce);
        }
        // Completion owes exactly one debounced follow-up.
        assert!(scheduler.job_finished());
        assert!(!scheduler.is_idle());
        // The follow-up fires and, with no further triggers, settles idle.
        assert_eq!(scheduler.trigger(), TriggerDecision::Coalesce);
        assert_eq!(scheduler.timer_fired(), FireDecision::StartJob);
        assert!(!scheduler.job_finished());
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
        assert!(!scheduler.job_finished());

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
        assert!(!scheduler.job_finished());
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
            for _ in 0..10 {
                assert_eq!(scheduler.trigger(), TriggerDecision::Coalesce);
            }
            if scheduler.job_finished() {
                runs += 1;
                assert_eq!(scheduler.trigger(), TriggerDecision::Coalesce);
                assert_eq!(scheduler.timer_fired(), FireDecision::StartJob);
            }
        }
        assert_eq!(runs, 4, "each storm owes exactly one follow-up run");
        // Quiescence: settle the tail run.
        while scheduler.job_finished() {
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
        assert!(!scheduler.job_finished());
        scheduler.job_finished();
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
        assert!(!scheduler.job_finished());
        scheduler.trigger();
        assert!(!scheduler.job_finished());
        assert_eq!(scheduler.trigger(), TriggerDecision::Coalesce);
    }

    #[test]
    fn dirty_flag_is_cleared_by_the_rescheduled_run() {
        let mut scheduler = SchedulerState::default();
        scheduler.trigger();
        assert_eq!(scheduler.timer_fired(), FireDecision::StartJob);
        scheduler.trigger();
        assert!(scheduler.job_finished());
        assert!(!scheduler.is_idle());
        // The owed follow-up starts and finishes without owing another.
        assert_eq!(scheduler.timer_fired(), FireDecision::StartJob);
        assert!(!scheduler.job_finished());
        assert!(scheduler.is_idle());
    }
}
