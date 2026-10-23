use super::*;
use crate::sim::sdf::{
    resolve, SdfLimits, SdfPortDirection, SdfScope, SdfScopeKind, SdfTargetPort,
};
use crate::sim::sdf::{SdfTargetCondition, SdfTargetEdge, SdfTargetPath, SdfTerminal};

fn number(text: &str) -> SdfNumber {
    SdfNumber::parse(text).expect("number")
}

fn terminal(name: &str) -> SdfTerminal {
    SdfTerminal {
        name: name.to_owned(),
        range: None,
    }
}

/// `top` (1 ps precision) with a cell `u` of type `C` (`precision_fs`)
/// declaring `(a => y)` and a gate `g` driving output `y`.
fn targets(precision_fs: u64) -> SdfTargets {
    let mut targets = SdfTargets::new();
    let mut top = SdfScope::new("top".into(), None, SdfScopeKind::Module, "top".into());
    top.precision_fs = 1_000;
    let top = targets.add_scope(None, top);
    let mut cell = SdfScope::new("u".into(), None, SdfScopeKind::Module, "C".into());
    cell.precision_fs = precision_fs;
    for (name, direction) in [
        ("a", SdfPortDirection::Input),
        ("y", SdfPortDirection::Output),
    ] {
        cell.ports.push(SdfTargetPort {
            name: name.into(),
            direction,
            range: None,
        });
    }
    cell.paths.push(SdfTargetPath {
        origin: String::new(),
        inputs: vec![terminal("a")],
        outputs: vec![terminal("y")],
        edge: SdfTargetEdge::None,
        condition: SdfTargetCondition::None,
        rows: Vec::new(),
    });
    targets.add_scope(Some(top), cell);
    // `v` of type `D` has no paths; its gate `g` drives output `y`.
    let mut cell = SdfScope::new("v".into(), None, SdfScopeKind::Module, "D".into());
    cell.precision_fs = precision_fs;
    cell.ports.push(SdfTargetPort {
        name: "y".into(),
        direction: SdfPortDirection::Output,
        range: None,
    });
    let cell = targets.add_scope(Some(top), cell);
    let mut gate = SdfScope::new("g".into(), None, SdfScopeKind::Primitive, "and".into());
    gate.precision_fs = precision_fs;
    gate.drives.push(0);
    targets.add_scope(Some(cell), gate);
    targets
}

/// Value `body` (DELAY entries of CELL `u`) with `options`; ticks are
/// femtoseconds.
fn valued(body: &str, precision_fs: u64, options: &SdfCallOptions) -> SdfAnnotation {
    valued_in("C", "u", body, precision_fs, options)
}

fn valued_in(
    celltype: &str,
    instance: &str,
    body: &str,
    precision_fs: u64,
    options: &SdfCallOptions,
) -> SdfAnnotation {
    let targets = targets(precision_fs);
    let text = format!(
        "(DELAYFILE (SDFVERSION \"3.0\") (TIMESCALE 1ns) (CELL (CELLTYPE \"{celltype}\") \
         (INSTANCE {instance}) (DELAY {body})))"
    );
    let plan = resolve(text.as_bytes(), &targets, 0, SdfLimits::default()).expect("resolves");
    apply(
        &plan,
        &targets,
        options,
        SdfTimebase {
            design_precision_fs: 1,
        },
    )
}

/// The twelve delays (`-` for unchanged) and limit masks of the only op.
fn delays(annotation: &SdfAnnotation) -> String {
    let [op] = annotation.ops.as_slice() else {
        panic!("one op: {annotation:?}");
    };
    let values = annotation.values_of(op);
    let mut next = values.iter();
    let mut text = (0..12)
        .map(|index| {
            if op.masks.delay & (1 << index) != 0 {
                (next.next().expect("value") / 1_000).to_string()
            } else {
                "-".to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    if op.masks.derive != 0 {
        text.push_str(&format!(" derive {:03x}", op.masks.derive));
    }
    if op.masks.reject | op.masks.error != 0 {
        let rest = next
            .map(|value| (value / 1_000).to_string())
            .collect::<Vec<_>>();
        text.push_str(&format!(
            " limits {:03x}/{:03x} {}",
            op.masks.reject,
            op.masks.error,
            rest.join(" ")
        ));
    }
    text
}

fn path(body: &str) -> String {
    delays(&valued(
        &format!("(ABSOLUTE (IOPATH a y {body}))"),
        1_000,
        &SdfCallOptions::default(),
    ))
}

/// IEEE 1800-2009 Table 32-4, in transition order 01 10 0z z1 1z z0 0x x1
/// 1x x0 xz zx; delays in picoseconds.
#[test]
fn delay_lists_expand_by_table_32_4() {
    assert_eq!(
        path("(5)"),
        "5000 5000 5000 5000 5000 5000 5000 5000 5000 5000 5000 5000 derive fff"
    );
    assert_eq!(
        path("(3) (5)"),
        "3000 5000 3000 3000 5000 5000 3000 3000 5000 5000 5000 3000 derive fff"
    );
    assert_eq!(
        path("(3) (5) (9)"),
        "3000 5000 9000 3000 9000 5000 3000 3000 5000 5000 9000 3000 derive fff"
    );
    assert_eq!(
        path("(5) (12) (17) (10) (6) (22)"),
        "5000 12000 17000 10000 6000 22000 5000 10000 6000 22000 17000 10000 derive fff"
    );
    assert_eq!(
        path("(1) (2) (3) (4) (5) (6) (7) (8) (9) (10) (11) (12)"),
        "1000 2000 3000 4000 5000 6000 7000 8000 9000 10000 11000 12000 derive fff"
    );
    // Four values are six with trailing nulls: 1->z, z->0 and the x
    // transitions derived from them keep their values.
    assert_eq!(
        path("(1) (2) (3) (4)"),
        "1000 2000 3000 4000 - - 1000 4000 - - - - derive 0cf"
    );
    // A null delval keeps its transitions.
    assert_eq!(
        path("() (7)"),
        "- 7000 - - 7000 7000 - - 7000 7000 - - derive 332"
    );
}

/// 32.7 and OVI SDF 3.0 3-18: `(d r e)` gives both limits, `(d r)` one for
/// both, `(d () ())` keeps them.
#[test]
fn delvals_carry_pulse_limits() {
    assert_eq!(
        path("((10) (4) (8))"),
        "10000 10000 10000 10000 10000 10000 10000 10000 10000 10000 10000 10000 limits fff/fff \
         4000 4000 4000 4000 4000 4000 4000 4000 4000 4000 4000 4000 8000 8000 8000 8000 8000 \
         8000 8000 8000 8000 8000 8000 8000"
    );
    let annotation = valued(
        "(ABSOLUTE (IOPATH a y ((10) () ())))",
        1_000,
        &SdfCallOptions::default(),
    );
    let op = annotation.ops[0];
    assert_eq!(
        (op.masks.delay, op.masks.reject, op.masks.derive),
        (0xfff, 0, 0)
    );
}

#[test]
fn members_and_scale_factors_follow_tables_32_5_and_32_6() {
    let rise = |options: SdfCallOptions| {
        let annotation = valued("(ABSOLUTE (IOPATH a y (1:2:3)))", 1_000, &options);
        // Picoseconds.
        annotation.values[0] / 1_000
    };
    let options = |mtm, scale: &str, scale_type| SdfCallOptions {
        mtm,
        scale: parse_scale_factors(scale).expect("scale factors"),
        scale_type,
        ..SdfCallOptions::default()
    };
    use SdfMtm::*;
    use SdfScaleType::*;
    assert_eq!(rise(options(ToolControl, "1:1:1", FromMtm)), 2000);
    assert_eq!(rise(options(Minimum, "1:1:1", FromMtm)), 1000);
    assert_eq!(rise(options(Maximum, "1:1:1", FromMtm)), 3000);
    assert_eq!(rise(options(Minimum, "2:3:4", FromMtm)), 2000);
    assert_eq!(rise(options(Typical, "2:3:4", FromMtm)), 6000);
    assert_eq!(rise(options(Maximum, "2:3:4", FromMtm)), 12000);
    // FROM_MINIMUM: every member is the minimum, scaled by its own factor.
    assert_eq!(rise(options(Typical, "2:3:4", FromMinimum)), 3000);
    assert_eq!(rise(options(Typical, "2:3:4", FromMaximum)), 9000);
    assert_eq!(rise(options(Maximum, "2:3:4", FromTypical)), 8000);
    assert_eq!(rise(options(Maximum, "0.5:0.5:0.5", FromMinimum)), 500);
    let tool_max = SdfCallOptions {
        tool_member: 2,
        ..SdfCallOptions::default()
    };
    assert_eq!(rise(tool_max), 3000);
    // An absent member leaves the transition unchanged.
    let annotation = valued(
        "(ABSOLUTE (IOPATH a y (1::3)))",
        1_000,
        &SdfCallOptions::default(),
    );
    assert!(annotation.ops.is_empty());
}

/// Values round once, to the precision of the module they land in, half
/// away from zero; out of range is one precision unit with a warning.
#[test]
fn values_round_to_the_target_precision() {
    let rise = |value: &str, precision_fs: u64| {
        let annotation = valued(
            &format!("(INCREMENT (IOPATH a y ({value})))"),
            precision_fs,
            &SdfCallOptions::default(),
        );
        (annotation.values[0], annotation.warnings.len())
    };
    assert_eq!(rise("1.2345", 1_000), (1_235_000, 0));
    assert_eq!(rise("2.4", 1_000_000), (2_000_000, 0));
    assert_eq!(rise("2.5", 1_000_000), (3_000_000, 0));
    assert_eq!(rise("-2.5", 1_000_000), (-3_000_000, 0));
    assert_eq!(rise("1.25", 100_000), (1_300_000, 0));
    assert_eq!(rise("1e999", 1_000), (1_000, 1));
    assert_eq!(rise("1e-999", 1_000), (0, 0));
}

#[test]
fn primitive_delays_follow_table_28_9() {
    let gate = |body: &str, turnoff| {
        let options = SdfCallOptions {
            turnoff,
            ..SdfCallOptions::default()
        };
        let annotation = valued_in(
            "D",
            "v",
            &format!("(ABSOLUTE (DEVICE {body}))"),
            1_000,
            &options,
        );
        assert_eq!(annotation.ops.len(), 1, "{body}: {annotation:?}");
        let op = annotation.ops[0];
        assert!(matches!(op.target, SdfAppliedTarget::Primitive { .. }));
        (
            op.masks.delay,
            annotation
                .values_of(&op)
                .iter()
                .map(|v| v / 1_000_000)
                .collect::<Vec<_>>(),
        )
    };
    // `v` has no path, so DEVICE reaches the gate driving its output: rise,
    // fall and turn-off (Table 28-9 with SDF lists of one, two and three).
    assert_eq!(gate("(3)", SdfTurnoff::FromFile), (0b111, vec![3, 3, 3]));
    assert_eq!(
        gate("(3) (5)", SdfTurnoff::FromFile),
        (0b111, vec![3, 5, 3])
    );
    assert_eq!(
        gate("(3) (5) (7)", SdfTurnoff::FromFile),
        (0b111, vec![3, 5, 7])
    );
    // Six values: the first three are the gate's rise, fall and turn-off.
    assert_eq!(
        gate("(3) (5) (7) (9) (11) (13)", SdfTurnoff::FromFile),
        (0b111, vec![3, 5, 7])
    );
    assert_eq!(gate("(3) (5)", SdfTurnoff::Minimum), (0b111, vec![3, 5, 3]));
    assert_eq!(gate("(3) (5)", SdfTurnoff::Maximum), (0b111, vec![3, 5, 5]));
    assert_eq!(gate("(3) (7)", SdfTurnoff::Average), (0b111, vec![3, 7, 5]));
    // A null fall keeps the gate's fall delay and the turn-off delay
    // derived from it.
    assert_eq!(gate("(3) ()", SdfTurnoff::FromFile).0, 0b001);
}

#[test]
fn config_files_set_the_defaults() {
    let config = parse_config(
        "// comment\nMTM = maximum;\nSCALE_FACTORS = 2:2:2 ; /* c */ SCALE_TYPE=FROM_MTM;\n\
         INTERCONNECT_MIPD = AVERAGE;\nTURNOFF_DELAY = MAXIMUM;\n",
    )
    .expect("config");
    assert_eq!(config.mtm, Some(SdfMtm::Maximum));
    assert_eq!(config.scale, Some([number("2"); 3]));
    assert_eq!(config.scale_type, Some(SdfScaleType::FromMtm));
    assert_eq!(config.mipd, Some(SdfMipdPolicy::Average));
    assert_eq!(config.turnoff, Some(SdfTurnoff::Maximum));
    assert_eq!(
        parse_config("MTM = MAXIMUM;\nFOO = 1;\n")
            .expect_err("unknown")
            .to_string(),
        "2: unknown statement `FOO`; llg reads MTM, SCALE_FACTORS, SCALE_TYPE, \
         INTERCONNECT_MIPD and TURNOFF_DELAY"
    );
    assert_eq!(
        parse_config("\nMTM = SOMETIMES;")
            .expect_err("bad")
            .to_string(),
        "2: invalid MTM value `SOMETIMES`"
    );
    assert_eq!(
        parse_config("MTM = MAXIMUM").expect_err("open").to_string(),
        "1: `MTM = MAXIMUM` has no closing `;`"
    );
    assert_eq!(parse_scale_factors("2:3"), None);
    assert_eq!(parse_scale_factors("a:b:c"), None);
    assert_eq!(parse_scale_factors("-1:1:1"), None);
    assert_eq!(SdfMtm::parse("tool_control"), Some(SdfMtm::ToolControl));
    assert_eq!(SdfScaleType::parse("FOO"), None);
}
