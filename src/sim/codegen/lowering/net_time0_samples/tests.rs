use super::*;
use crate::sim::ir::{IrBinOp, IrNetGroup, IrProcess, IrShape, IrSignal};

fn literal(bits: u64, x: u64, z: u64, width: u32) -> IrConst {
    IrConst {
        bits: vec![bits],
        x: vec![x],
        z: vec![z],
        width,
        signed: false,
        real: None,
        fill: None,
    }
}

fn constant(bits: u64, z: u64, width: u32) -> IrExpr {
    IrExpr::new(
        IrExprKind::Const(literal(bits, 0, z, width)),
        width,
        false,
        None,
    )
}

fn read(signal: usize, width: u32) -> IrExpr {
    IrExpr::new(IrExprKind::SigRead(signal), width, false, None)
}

struct Fixture {
    model: IrModel,
    continuous: HashSet<String>,
}

impl Fixture {
    fn new() -> Self {
        Self {
            model: IrModel::new("net_time0_samples".to_owned(), 1).unwrap(),
            continuous: HashSet::new(),
        }
    }

    /// A 4-bit wire with `drivers` slots; returns the first slot signal.
    fn net(&mut self, drivers: usize) -> usize {
        let group = self.model.net_groups.len();
        self.model.net_groups.push(
            IrNetGroup::new(format!("g_net_{group}"), 4, false, IrNetKind::Wire, drivers).unwrap(),
        );
        let first = self.model.signals.len();
        for slot in 0..drivers {
            self.model.signals.push(
                IrSignal::new(
                    format!("g_net_{group}.resolved"),
                    Some(format!("tb.n{group}_{slot}")),
                    IrType::Packed {
                        width: 4,
                        signed: false,
                        two_state: false,
                    },
                    Some((group, slot)),
                )
                .unwrap(),
            );
        }
        self.model
            .init_steps
            .push(IrInitStep::RegisterSampledValue(first));
        first
    }

    fn variable(&mut self, initial: Option<IrConst>) -> usize {
        let signal = self.model.signals.len();
        self.model.signals.push(
            IrSignal::new(
                format!("G_v{signal}"),
                Some(format!("tb.v{signal}")),
                IrType::Packed {
                    width: 4,
                    signed: false,
                    two_state: false,
                },
                None,
            )
            .unwrap(),
        );
        if let Some(value) = initial {
            self.model
                .init_steps
                .push(IrInitStep::SetScalar { sig: signal, value });
        }
        signal
    }

    fn drive(&mut self, lhs: IrLhs, rhs: IrExpr, continuous: bool) {
        let name = format!("p_{}", self.model.processes.len());
        self.model.processes.push(IrProcess::new(
            name.clone(),
            "tb.assign".to_owned(),
            IrShape::RunOnce,
            Vec::new(),
            vec![IrStmt::Assign {
                lhs,
                rhs,
                nba: false,
            }],
        ));
        if continuous {
            self.continuous.insert(name);
        }
    }

    fn starts(mut self) -> HashMap<usize, IrConst> {
        assign_net_time0_samples(&mut self.model, &self.continuous);
        self.model
            .init_steps
            .iter()
            .filter_map(|step| match step {
                IrInitStep::SampledStart { sig, value } => Some((*sig, value.clone())),
                _ => None,
            })
            .collect()
    }
}

#[test]
fn single_continuous_drivers_fold_over_initial_values_through_chains() {
    let mut fixture = Fixture::new();
    let constant_net = fixture.net(1);
    fixture.drive(IrLhs::Whole(constant_net), constant(9, 0, 4), true);
    let variable = fixture.variable(Some(literal(3, 0, 0, 4)));
    let first = fixture.net(1);
    fixture.drive(IrLhs::Whole(first), read(variable, 4), true);
    let second = fixture.net(1);
    let sum = IrExpr::new(
        IrExprKind::Bin {
            op: IrBinOp::Add,
            a: Box::new(read(first, 4)),
            b: Box::new(constant(2, 0, 4)),
        },
        4,
        false,
        None,
    );
    fixture.drive(IrLhs::Whole(second), sum, true);
    let uninitialized = fixture.variable(None);
    let unknown = fixture.net(1);
    fixture.drive(IrLhs::Whole(unknown), read(uninitialized, 4), true);
    let part = fixture.net(1);
    fixture.drive(IrLhs::Part(part, 1, 0, false), constant(2, 0, 2), true);
    let starts = fixture.starts();
    assert_eq!(starts[&constant_net], literal(9, 0, 0, 4));
    assert_eq!(starts[&first], literal(3, 0, 0, 4));
    assert_eq!(starts[&second], literal(5, 0, 0, 4));
    assert_eq!(starts[&unknown], literal(0, 0xf, 0, 4));
    // Undriven bits keep z.
    assert_eq!(starts[&part], literal(2, 0, 0xc, 4));
}

#[test]
fn nets_without_one_folding_continuous_driver_keep_their_start() {
    let mut fixture = Fixture::new();
    // Two driver sites.
    let wired = fixture.net(2);
    fixture.drive(IrLhs::Whole(wired), constant(1, 0, 4), true);
    fixture.drive(IrLhs::Whole(wired + 1), constant(1, 0, 4), true);
    // A gate or other non-continuous writer.
    let gate = fixture.net(1);
    fixture.drive(IrLhs::Whole(gate), constant(1, 0, 4), false);
    // A computed z is an X to z change (T0-D4).
    let enable = fixture.variable(Some(literal(0, 0, 0, 4)));
    let computed = fixture.net(1);
    let select = IrExpr::new(
        IrExprKind::Mux {
            sel: Box::new(read(enable, 4)),
            a: Box::new(constant(1, 0, 4)),
            b: Box::new(constant(0, 0xf, 4)),
        },
        4,
        false,
        None,
    );
    fixture.drive(IrLhs::Whole(computed), select, true);
    // A constant z driver presents z.
    let floating = fixture.net(1);
    fixture.drive(IrLhs::Whole(floating), constant(0, 0xf, 4), true);
    // A cycle never folds.
    let loop_a = fixture.net(1);
    let loop_b = fixture.net(1);
    fixture.drive(IrLhs::Whole(loop_a), read(loop_b, 4), true);
    fixture.drive(IrLhs::Whole(loop_b), read(loop_a, 4), true);
    let starts = fixture.starts();
    for net in [wired, gate, computed, loop_a, loop_b] {
        assert!(!starts.contains_key(&net), "net {net}");
    }
    assert_eq!(starts[&floating], literal(0, 0, 0xf, 4));
}

#[test]
fn models_that_never_sample_get_no_start_values() {
    let mut fixture = Fixture::new();
    let net = fixture.net(1);
    fixture.model.init_steps.clear();
    fixture.drive(IrLhs::Whole(net), constant(9, 0, 4), true);
    assert!(fixture.starts().is_empty());
}
