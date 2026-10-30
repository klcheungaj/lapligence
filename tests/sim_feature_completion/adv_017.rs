//! ADV-017: assertion and coverage native API object lifecycles (IEEE
//! 1800-2009 Clauses 39 and 40). Each fixture's C application is loaded as
//! a run-time plugin (`LLG_VPI_PLUGIN`, a DLL linked against the model
//! import library on Windows); `lifecycle` also links as a static archive
//! (`--dpi-lib`, every host). Expected outputs are derived by hand in the
//! fixture readme.
use super::sim_cli;
use super::sim_harness;

const SUITE: &str = "feature_completion/adv_017";

fn plugin(directory: &sim_harness::TempDir, stem: &str) -> Option<String> {
    super::sim_040::plugin_library(SUITE, directory, stem, &[])
}

const LIFECYCLE: &[&str] = &[
    "--define-system-task",
    "task $attach()",
    "--define-system-task",
    "task $ctl(input integer n)",
    "--define-system-task",
    "task $report()",
];

/// A01/A02: attempt callbacks on multiclock, future-global, checker-owned
/// and cover assertions agree with the HDL action counters and the 40.5.3
/// counters; a kill and a disabled pass action are told apart; a removed
/// record runs no more.
#[test]
fn assertion_callbacks_correlate_with_hdl_and_coverage_counters() {
    let directory = sim_harness::TempDir::new("adv017-life").expect("temporary directory");
    let Some(library) = plugin(&directory, "lifecycle") else {
        return;
    };
    sim_cli::run_case_backend_parity(
        SUITE,
        "lifecycle",
        include_str!("../fixtures/sim/feature_completion/adv_017/lifecycle.out"),
        LIFECYCLE,
        &[("LLG_VPI_PLUGIN", &library)],
    );
}

#[test]
fn assertion_callbacks_link_from_a_static_archive() {
    let directory = sim_harness::TempDir::new("adv017-static").expect("temporary directory");
    let archive = super::sim_041::static_archive(SUITE, &directory, "lifecycle", &[]);
    let mut args = LIFECYCLE.to_vec();
    args.extend(["--dpi-lib", archive.as_str()]);
    sim_cli::run_case_backend_parity(
        SUITE,
        "lifecycle",
        include_str!("../fixtures/sim/feature_completion/adv_017/lifecycle.out"),
        &args,
        &[],
    );
}

/// A03: every 39.5 operation on concurrent, immediate, deferred and
/// sub-instance assertions, the callbacks it raises, and the specified
/// errors (step callbacks, wrong handles, after vpiAssertionSysEnd).
#[test]
fn vpi_control_drives_assertions_and_raises_their_callbacks() {
    let directory = sim_harness::TempDir::new("adv017-ctl").expect("temporary directory");
    let Some(library) = plugin(&directory, "control") else {
        return;
    };
    sim_cli::run_case_backend_parity(
        SUITE,
        "control",
        include_str!("../fixtures/sim/feature_completion/adv_017/control.out"),
        &[
            "--define-system-task",
            "task $setup()",
            "--define-system-task",
            "task $ctl(input integer n)",
            "--define-system-task",
            "task $report()",
        ],
        &[("LLG_VPI_PLUGIN", &library)],
    );
}

/// 39.3 static information of concurrent, immediate, deferred, unnamed and
/// checker-owned assertions; a callback that kills its own attempt and one
/// that removes itself.
#[test]
fn static_assertion_information_and_reentrant_callbacks() {
    let directory = sim_harness::TempDir::new("adv017-obj").expect("temporary directory");
    let Some(library) = plugin(&directory, "objects") else {
        return;
    };
    sim_cli::run_case_backend_parity(
        SUITE,
        "objects",
        include_str!("../fixtures/sim/feature_completion/adv_017/objects.out"),
        &[
            "--define-system-task",
            "task $objects()",
            "--define-system-task",
            "task $summary()",
        ],
        &[("LLG_VPI_PLUGIN", &library)],
    );
}

/// A01/A03: assertion relations (37.45-37.51), sequence and property
/// instances and declarations (37.47-37.50), failExpr and the callbacks of
/// an instance that is an assertion's whole property (39.4.2, A17-D4 and
/// A17-D5); a nested instance, a declaration body and a formal default are
/// specified errors.
#[test]
fn assertion_relations_reach_clocks_conditions_properties_and_instances() {
    let directory = sim_harness::TempDir::new("adv017-rel").expect("temporary directory");
    let Some(library) = plugin(&directory, "relations") else {
        return;
    };
    sim_cli::run_case_backend_parity(
        SUITE,
        "relations",
        include_str!("../fixtures/sim/feature_completion/adv_017/relations.out"),
        &["--vpi", "--define-system-task", "task $relations()"],
        &[("LLG_VPI_PLUGIN", &library)],
    );
}

/// FSM state coverage (IEEE 1800-2009 40.2.2, 40.3.2, 40.4): every pragma
/// form of 40.4.1-40.4.7 (separate and inline enumeration names, block and
/// one-line comments, a part-select, a concatenation, an ANSI port, a
/// continuously assigned net, localparam states) and the `$coverage_*`
/// functions for SV_COV_FSM_STATE, with stop, start, reset, save and merge
/// (decisions A17-D11 and A17-D12).
#[test]
fn fsm_state_coverage_follows_the_pragmas_and_coverage_functions() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "fsm_coverage",
        include_str!("../fixtures/sim/feature_completion/adv_017/fsm_coverage.out"),
        &[],
        &[],
    );
}

/// Per FSM of a saved database's FSM section: the visits of each state and
/// the observed transitions as (from, to, count), sorted.
type FsmCounts = (Vec<u64>, Vec<(u32, u32, u64)>);

fn fsm_section(bytes: &[u8]) -> Vec<FsmCounts> {
    let u32_at = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
    let u64_at = |at: usize| u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap());
    assert_eq!(&bytes[..8], b"LLGCOVDB");
    assert_eq!(u32_at(12), 4, "only an FSM section");
    let end = 48 + u64_at(40) as usize;
    let mut at = 56;
    let mut fsms = Vec::new();
    for _ in 0..u32_at(at) {
        at += 4;
        let states = u32_at(at) as usize;
        let visits = (0..states)
            .map(|state| u64_at(at + 4 + state * 8))
            .collect();
        at += 4 + states * 8;
        let transitions = (0..u32_at(at) as usize)
            .map(|row| {
                let row = at + 4 + row * 16;
                (u32_at(row), u32_at(row + 4), u64_at(row + 8))
            })
            .collect::<Vec<_>>();
        at += transitions.len() * 16;
        fsms.push((visits, transitions));
    }
    assert_eq!(at + 4, end, "section length");
    fsms
}

/// The FSM section `fsm_coverage` saves at 36 ns: per FSM in instance and
/// declaration order (c1, c2, u3, then u4's q, part, pair and w), visits
/// per state and the transitions between states (readme.md).
#[test]
fn fsm_coverage_database_holds_visits_and_transitions() {
    let (directory, outputs) =
        sim_cli::run_case_sequence(SUITE, "fsm_coverage", false, &[], &[], &[(&[], &[])]);
    assert!(
        outputs[0].status.success(),
        "{}",
        String::from_utf8_lossy(&outputs[0].stderr)
    );
    let bytes = std::fs::read(directory.path().join("fsm_coverage.db")).expect("fsm_coverage.db");
    let expected: Vec<FsmCounts> = vec![
        (vec![1, 0, 0, 0], vec![(2, 0, 1)]),
        (vec![2, 1, 1, 0], vec![(1, 2, 1), (2, 0, 1)]),
        (vec![1, 1, 1], vec![(0, 1, 1)]),
        (vec![1, 0, 1], vec![(0, 2, 1)]),
        (vec![1, 1], vec![]),
        (vec![1, 1], vec![]),
        (vec![1, 1], vec![(0, 1, 1)]),
    ];
    assert_eq!(fsm_section(&bytes), expected);
}

/// 40.5.3 FSM objects: vpiFsm from an instance, vpiFsmStates, the state
/// expression (a signal, a part-select, and the concatenation that has no
/// object), the coverage properties of FSMs and states, instance-level
/// vpi_control with save and merge, and a value-change callback sharing
/// the state vector's watch (decision A17-D13).
#[test]
fn fsm_objects_report_states_counters_and_controls() {
    let directory = sim_harness::TempDir::new("adv017-fsm").expect("temporary directory");
    let Some(library) = plugin(&directory, "fsm_vpi") else {
        return;
    };
    sim_cli::run_case_backend_parity(
        SUITE,
        "fsm_vpi",
        include_str!("../fixtures/sim/feature_completion/adv_017/fsm_vpi.out"),
        &[
            "--define-system-task",
            "task $fsm_objects()",
            "--define-system-task",
            "task $fsm_counts(input integer n)",
            "--define-system-task",
            "task $fsm_control()",
            "--define-system-task",
            "task $fsm_files()",
        ],
        &[("LLG_VPI_PLUGIN", &library)],
    );
}

/// A malformed, misplaced or unresolvable FSM pragma is an error, never an
/// FSM left silently uncovered (decision A17-D11).
#[test]
fn fsm_pragma_errors_are_reported() {
    sim_cli::reject_case(
        SUITE,
        "neg_fsm_signal",
        "neg_fsm_signal.sv:5:3: `cz` is not a signal of module `tb`: an FSM pragma belongs in the module that declares its state vector (IEEE 1800-2009 40.4.1)",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_fsm_states",
        "neg_fsm_states.sv:4:3: no `parameter /* tool enum E */` declaration lists the states of FSM `cs`; llg does not extract FSM states automatically (IEEE 1800-2009 40.4, 40.4.6)",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_fsm_malformed",
        "neg_fsm_malformed.sv:4:3: malformed FSM pragma: `enum` needs an enumeration name, found the end of the pragma",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_fsm_select",
        "neg_fsm_select.sv:5:3: FSM `field` part-select `r[9:8]` is not within the declared range of `r` in the declared direction (IEEE 1800-2009 40.4.2)",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_fsm_value",
        "neg_fsm_value.sv:5:3: FSM `cs` state `S4` is not a known value of the 2-bit state vector (IEEE 1800-2009 40.4.6)",
    );
}
