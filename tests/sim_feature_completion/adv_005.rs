//! ADV-005: bidirectional pass-switch networks (`tran`, `tranif0`,
//! `tranif1`, `rtran`, `rtranif0`, `rtranif1`). The fixtures' expected values
//! and `%v` strengths are derived in the fixture readme from IEEE 1800-2009
//! 4.9.5, 28.8 and 28.12-28.15 (IEEE 1364-2001 5.6.5, 7.6 and 7.10-7.13); they
//! run in both editions, both optimizer modes and every value backend. The
//! A01 test compares generated random networks with an independent oracle that
//! enumerates every conducting/nonconducting combination of the switches with
//! an unknown control.
use super::{sim_cli, sim_harness};

const SUITE: &str = "feature_completion/adv_005";

const EDITIONS: [&str; 2] = ["v2001", "sv2009"];

// ── Fixtures: strengths, unknown controls, loops, force, delays, ports ───────

#[test]
fn tran_keeps_and_rtran_reduces_strength() {
    for edition in EDITIONS {
        sim_cli::run_case_backend_parity(
            SUITE,
            "strengths",
            include_str!("../fixtures/sim/feature_completion/adv_005/strengths.out"),
            &["--edition", edition],
            &[],
        );
    }
}

#[test]
fn a_node_passes_only_what_survives_its_own_resolution() {
    for edition in EDITIONS {
        sim_cli::run_case_backend_parity(
            SUITE,
            "blocking",
            include_str!("../fixtures/sim/feature_completion/adv_005/blocking.out"),
            &["--edition", edition],
            &[],
        );
    }
}

#[test]
fn unknown_controls_give_the_union_of_both_switch_states() {
    for edition in EDITIONS {
        sim_cli::run_case_backend_parity(
            SUITE,
            "unknown_controls",
            include_str!("../fixtures/sim/feature_completion/adv_005/unknown_controls.out"),
            &["--edition", edition],
            &[],
        );
    }
}

#[test]
fn loops_and_opposing_drivers_resolve_once() {
    for edition in EDITIONS {
        sim_cli::run_case_backend_parity(
            SUITE,
            "loops",
            include_str!("../fixtures/sim/feature_completion/adv_005/loops.out"),
            &["--edition", edition],
            &[],
        );
    }
}

#[test]
fn a_forced_node_is_a_fixed_strong_source() {
    for edition in EDITIONS {
        sim_cli::run_case_backend_parity(
            SUITE,
            "force",
            include_str!("../fixtures/sim/feature_completion/adv_005/force.out"),
            &["--edition", edition],
            &[],
        );
    }
}

#[test]
fn tranif_delays_delay_the_switch_state() {
    for edition in EDITIONS {
        sim_cli::run_case_backend_parity(
            SUITE,
            "delays",
            include_str!("../fixtures/sim/feature_completion/adv_005/delays.out"),
            &["--edition", edition],
            &[],
        );
    }
}

#[test]
fn ports_connect_without_reduction_and_net_delays_stay_local() {
    for edition in EDITIONS {
        sim_cli::run_case_backend_parity(
            SUITE,
            "ports",
            include_str!("../fixtures/sim/feature_completion/adv_005/ports.out"),
            &["--edition", edition],
            &[],
        );
    }
}

#[test]
fn switch_arrays_take_one_bit_per_instance() {
    for edition in EDITIONS {
        sim_cli::run_case_backend_parity(
            SUITE,
            "arrays",
            include_str!("../fixtures/sim/feature_completion/adv_005/arrays.out"),
            &["--edition", edition],
            &[],
        );
    }
}

#[test]
fn gates_drive_and_read_switch_networks() {
    for edition in EDITIONS {
        sim_cli::run_case_backend_parity(
            SUITE,
            "gates",
            include_str!("../fixtures/sim/feature_completion/adv_005/gates.out"),
            &["--edition", edition],
            &[],
        );
    }
}

#[test]
fn only_changed_nodes_wake_their_waiters() {
    for edition in EDITIONS {
        sim_cli::run_case_backend_parity(
            SUITE,
            "events",
            include_str!("../fixtures/sim/feature_completion/adv_005/events.out"),
            &["--edition", edition],
            &[],
        );
    }
}

#[test]
fn unsupported_switch_forms_are_located_errors() {
    for edition in EDITIONS {
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_mos",
            "neg_mos.sv:5:8: unsupported: `nmos` primitive (MOS and resistive switch \
             primitives) is not supported by llg",
            &["--edition", edition],
        );
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_wired",
            "neg_wired.sv:6:8: terminal `w` is a wired-logic (wand, wor, triand or trior) \
             net; wired resolution across a switch network is not simulated",
            &["--edition", edition],
        );
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_wide_terminal",
            "neg_wide_terminal.sv:5:8: terminal `p` is 2 bits wide; a terminal is a scalar \
             net or a bit select (IEEE 1800-2009 28.8)",
            &["--edition", edition],
        );
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_array_width",
            "neg_array_width.sv:4:8: terminal `p` is 3 bits wide for an array of 2 switches; \
             it must be 1 bit or one bit per switch (IEEE 1800-2009 28.3.6)",
            &["--edition", edition],
        );
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_tran_delay",
            "neg_tran_delay.sv:4:8 delays are not allowed on 'tran' gates",
            &["--edition", edition],
        );
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_variable_terminal",
            "neg_variable_terminal.sv:5:11 'r' cannot be connected to 'inout' port",
            &["--edition", edition],
        );
    }
}

// ── A01: random networks against an exhaustive enumeration oracle ───────────

/// Strength levels of the IEEE 1800-2009 28.12 scale: 7 supply, 6 strong,
/// 5 pull, 3 weak (the drive strengths), down to 1 small; -1 is none.
const NONE: i8 = -1;

/// Table 28-8 for `rtran*`; otherwise supply becomes strong (28.13).
fn reduce(level: i8, resistive: bool) -> i8 {
    if resistive {
        [0, 1, 1, 2, 2, 3, 5, 5][level as usize]
    } else if level == 7 {
        6
    } else {
        level
    }
}

/// Strongest known 0, known 1, possible 0, possible 1 levels of a node.
type Levels = [i8; 4];

/// The signed strength range (28.12.2) of a node's levels: a possible level
/// survives only if it is not weaker than the opposite known level.
fn hull(levels: Levels) -> (i8, i8) {
    let [k0, k1, p0, p1] = levels;
    let high = if p1 >= 0 && p1 >= k0 {
        p1
    } else if k0 >= 0 {
        -k0
    } else {
        0
    };
    let low = if p0 >= 0 && p0 >= k1 {
        -p0
    } else if k1 >= 0 {
        k1
    } else {
        0
    };
    (low, high)
}

/// One network state with every switch either on or off: the strongest
/// signal of each value reaching each node along a simple path, where a
/// signal leaves a node only if no stronger opposite known level holds it
/// there (decision A5-D2). Levels are tried strongest first, so a node's
/// blocking level is final before weaker signals are routed.
fn solve_definite(own: &[Levels], switches: &[(usize, usize, bool, u8)]) -> Vec<(i8, i8)> {
    let mut adjacent = vec![Vec::new(); own.len()];
    for &(a, b, resistive, state) in switches {
        if state == 1 {
            adjacent[a].push((b, resistive));
            adjacent[b].push((a, resistive));
        }
    }
    let mut levels = own.to_vec();
    for level in (1..=7).rev() {
        for (source, source_levels) in own.iter().enumerate() {
            for value in 0..2 {
                for possible in [false, true] {
                    let index = value + if possible { 2 } else { 0 };
                    let start = source_levels[index];
                    if start < level {
                        continue;
                    }
                    let mut stack = vec![(source, start, vec![source])];
                    while let Some((node, strength, path)) = stack.pop() {
                        if strength < level {
                            continue;
                        }
                        if strength == level && levels[node][index] < strength {
                            levels[node][index] = strength;
                            if !possible && levels[node][2 + value] < strength {
                                levels[node][2 + value] = strength;
                            }
                        }
                        if levels[node][1 - value] > strength {
                            continue;
                        }
                        for &(next, resistive) in &adjacent[node] {
                            if !path.contains(&next) {
                                let mut extended = path.clone();
                                extended.push(next);
                                stack.push((next, reduce(strength, resistive), extended));
                            }
                        }
                    }
                }
            }
        }
    }
    levels.into_iter().map(hull).collect()
}

/// 4.9.5: solve every conducting/nonconducting combination of the unknown
/// switches; each node takes the union of its ranges.
fn solve_enumerated(own: &[Levels], switches: &[(usize, usize, bool, u8)]) -> Vec<(i8, i8)> {
    let unknown: Vec<usize> = (0..switches.len())
        .filter(|&index| switches[index].3 == 2)
        .collect();
    let mut union: Option<Vec<(i8, i8)>> = None;
    for combination in 0..1u32 << unknown.len() {
        let mut fixed = switches.to_vec();
        for (bit, &index) in unknown.iter().enumerate() {
            fixed[index].3 = ((combination >> bit) & 1) as u8;
        }
        let result = solve_definite(own, &fixed);
        union = Some(match union {
            None => result,
            Some(previous) => previous
                .into_iter()
                .zip(result)
                .map(|((low, high), (l, h))| (low.min(l), high.max(h)))
                .collect(),
        });
    }
    union.unwrap_or_default()
}

/// `%v` of a signed strength range (IEEE 1800-2009 21.2.1.5): one mnemonic
/// for one level, otherwise the two levels as digits.
fn strength_text((low, high): (i8, i8)) -> String {
    const NAMES: [&str; 8] = ["Hi", "Sm", "Me", "We", "La", "Pu", "St", "Su"];
    let (value, first, second) = if low == 0 && high == 0 {
        return "HiZ".to_owned();
    } else if high < 0 {
        ('0', -low, -high)
    } else if low > 0 {
        ('1', high, low)
    } else if high == 0 {
        ('L', -low, -low)
    } else if low == 0 {
        ('H', high, high)
    } else {
        ('X', -low, high)
    };
    if first == second {
        format!("{}{value}", NAMES[first as usize])
    } else {
        format!("{first}{second}{value}")
    }
}

/// Deterministic xorshift generator; the networks are fixed by the seed.
struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }
}

const DRIVE: [(i8, &str); 4] = [(7, "supply"), (6, "strong"), (5, "pull"), (3, "weak")];
const VALUES: [char; 4] = ['0', '1', 'x', 'z'];

struct Driver {
    node: usize,
    strength0: usize,
    strength1: usize,
}

struct Switch {
    a: usize,
    b: usize,
    resistive: bool,
    controlled: bool,
    /// For a controlled switch, whether a 1 control conducts (`tranif1`).
    on_one: bool,
}

struct Network {
    nodes: usize,
    drivers: Vec<Driver>,
    switches: Vec<Switch>,
}

impl Network {
    fn random(random: &mut Random) -> Self {
        let nodes = 2 + random.below(5);
        let drivers = (0..random.below(nodes + 2))
            .map(|_| Driver {
                node: random.below(nodes),
                strength0: random.below(DRIVE.len()),
                strength1: random.below(DRIVE.len()),
            })
            .collect();
        let switches = (0..1 + random.below(8))
            .map(|_| Switch {
                a: random.below(nodes),
                b: random.below(nodes),
                resistive: random.below(10) < 3,
                controlled: random.below(4) != 0,
                on_one: random.below(2) == 0,
            })
            .collect();
        Self {
            nodes,
            drivers,
            switches,
        }
    }

    /// Own levels of every node for the given driver values (28.12.1).
    fn own(&self, values: &[char]) -> Vec<Levels> {
        let mut own = vec![[NONE; 4]; self.nodes];
        for (driver, value) in self.drivers.iter().zip(values) {
            let levels = &mut own[driver.node];
            let s0 = DRIVE[driver.strength0].0;
            let s1 = DRIVE[driver.strength1].0;
            match value {
                '0' => {
                    levels[0] = levels[0].max(s0);
                    levels[2] = levels[2].max(s0);
                }
                '1' => {
                    levels[1] = levels[1].max(s1);
                    levels[3] = levels[3].max(s1);
                }
                'x' => {
                    levels[2] = levels[2].max(s0);
                    levels[3] = levels[3].max(s1);
                }
                _ => {}
            }
        }
        own
    }

    /// Switch states for the given control values: 1 on, 0 off, 2 unknown.
    fn states(&self, controls: &[char]) -> Vec<(usize, usize, bool, u8)> {
        self.switches
            .iter()
            .zip(controls)
            .map(|(switch, control)| {
                let state = match (switch.controlled, control) {
                    (false, _) => 1,
                    (true, 'x' | 'z') => 2,
                    (true, '1') => u8::from(switch.on_one),
                    (true, _) => u8::from(!switch.on_one),
                };
                (switch.a, switch.b, switch.resistive, state)
            })
            .collect()
    }
}

#[test]
fn random_networks_match_the_enumeration_oracle() {
    const NETWORKS: usize = 160;
    const PHASES: usize = 4;
    const MAX_UNKNOWN: usize = 5;
    let mut random = Random(0x0adf_0050_5eed_1234);
    let networks: Vec<Network> = (0..NETWORKS)
        .map(|_| Network::random(&mut random))
        .collect();
    // Driver and control values of every phase; at most MAX_UNKNOWN unknown
    // controls per network keep the enumeration small.
    let mut phases: Vec<Vec<(Vec<char>, Vec<char>)>> = Vec::new();
    for _ in 0..PHASES {
        phases.push(
            networks
                .iter()
                .map(|network| {
                    let values = (0..network.drivers.len())
                        .map(|_| VALUES[random.below(VALUES.len())])
                        .collect();
                    let mut unknown = 0;
                    let controls = network
                        .switches
                        .iter()
                        .map(|_| {
                            let control = VALUES[random.below(VALUES.len())];
                            if matches!(control, 'x' | 'z') {
                                unknown += 1;
                                if unknown > MAX_UNKNOWN {
                                    return '1';
                                }
                            }
                            control
                        })
                        .collect();
                    (values, controls)
                })
                .collect(),
        );
    }

    let mut source = String::from("`timescale 1ns/1ns\nmodule tb;\n");
    for (k, network) in networks.iter().enumerate() {
        let nodes = (0..network.nodes)
            .map(|i| format!("n{k}_{i}"))
            .collect::<Vec<_>>();
        source.push_str(&format!("  wire {};\n", nodes.join(", ")));
        for (d, driver) in network.drivers.iter().enumerate() {
            source.push_str(&format!(
                "  reg r{k}_{d};\n  assign ({}0, {}1) n{k}_{} = r{k}_{d};\n",
                DRIVE[driver.strength0].1, DRIVE[driver.strength1].1, driver.node
            ));
        }
        for (s, switch) in network.switches.iter().enumerate() {
            let kind = match (switch.resistive, switch.controlled, switch.on_one) {
                (false, false, _) => "tran",
                (true, false, _) => "rtran",
                (false, true, true) => "tranif1",
                (false, true, false) => "tranif0",
                (true, true, true) => "rtranif1",
                (true, true, false) => "rtranif0",
            };
            if switch.controlled {
                source.push_str(&format!(
                    "  reg c{k}_{s};\n  {kind} s{k}_{s}(n{k}_{}, n{k}_{}, c{k}_{s});\n",
                    switch.a, switch.b
                ));
            } else {
                source.push_str(&format!(
                    "  {kind} s{k}_{s}(n{k}_{}, n{k}_{});\n",
                    switch.a, switch.b
                ));
            }
        }
    }
    source.push_str("  initial begin\n");
    let mut expected = String::new();
    for (phase, settings) in phases.iter().enumerate() {
        for (k, (network, (values, controls))) in networks.iter().zip(settings).enumerate() {
            for (d, value) in values.iter().enumerate() {
                source.push_str(&format!("    r{k}_{d} = 1'b{value};\n"));
            }
            for (s, control) in controls.iter().enumerate() {
                if network.switches[s].controlled {
                    source.push_str(&format!("    c{k}_{s} = 1'b{control};\n"));
                }
            }
        }
        source.push_str("    #1;\n");
        for (k, (network, (values, controls))) in networks.iter().zip(settings).enumerate() {
            let nodes = (0..network.nodes)
                .map(|i| format!("n{k}_{i}"))
                .collect::<Vec<_>>();
            source.push_str(&format!(
                "    $display(\"{phase} {k}{}\", {});\n",
                " %v".repeat(network.nodes),
                nodes.join(", ")
            ));
            let result = solve_enumerated(&network.own(values), &network.states(controls));
            expected.push_str(&format!("{phase} {k}"));
            for range in result {
                expected.push(' ');
                expected.push_str(&strength_text(range));
            }
            expected.push('\n');
        }
    }
    source.push_str("    $finish;\n  end\nendmodule\n");

    for (label, options) in [
        ("opt", llg::sim::opt::OptConfig::default()),
        ("no-opt", llg::sim::opt::OptConfig::none()),
    ] {
        let run = sim_harness::run_generated_sim_with_files_opts(
            &source,
            "tb",
            &format!("adv005_oracle_{label}"),
            &[],
            &options,
        )
        .unwrap_or_else(|error| panic!("{label}: {error}\n{source}"));
        for (line, (actual, wanted)) in run.stdout.lines().zip(expected.lines()).enumerate() {
            assert_eq!(actual, wanted, "{label}: line {}", line + 1);
        }
        assert_eq!(
            run.stdout.lines().count(),
            expected.lines().count(),
            "{label}: {}",
            run.stdout
        );
    }
}

/// A model without switches emits no switch tables or start call, so it is
/// unchanged by the switch support.
#[test]
fn a_model_without_switches_has_no_switch_runtime() {
    let run = sim_harness::run_generated_sim(
        "module tb; wire a, b; reg r; assign a = r; buf (b, a);\n\
         initial begin r = 1; #1 $display(\"%v %v\", a, b); $finish; end endmodule\n",
        "tb",
        "adv005_switchless",
    )
    .expect("switchless model");
    assert_eq!(run.stdout, "St1 St1\n");
    assert!(!run.model_c.contains("llg_switch"), "{}", run.model_c);
}
