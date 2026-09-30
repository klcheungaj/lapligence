//! Typed, effect-free source shapes for ordered procedural assign batches.
use crate::sim::execution::{effects_for_statements, ExecutionEffect};
use crate::sim::ir::{IrExpr, IrExprKind, IrModel, IrStmt, IrType};

/// Minimum consecutive homogeneous assignments needed to amortize a table/loop.
pub(in crate::sim::emit_c) const PCA_BATCH_MIN_ASSIGNMENTS: usize = 4;

#[derive(Clone, Debug, PartialEq)]
pub(in crate::sim::emit_c) struct Shape {
    pub expression: IrExpr,
    pub source_type: IrType,
    pub target_type: IrType,
}

pub(in crate::sim::emit_c) struct Row {
    pub source: String,
    pub target: usize,
    pub enable: usize,
    pub binding: usize,
}

pub(in crate::sim::emit_c) struct Batch {
    pub name: String,
    pub shape: Shape,
    pub rows: Vec<Row>,
}

impl Batch {
    pub fn helper(&self) -> String {
        format!("{}_apply", self.name)
    }
}

/// Preserve every conversion and selection; abstract only the source identity.
/// This whitelist cannot call, mutate, suspend or evaluate a second source.
fn source_shape(expression: &mut IrExpr) -> Option<IrExpr> {
    match &mut expression.kind {
        IrExprKind::SigRead(_) | IrExprKind::LocalRead(_) => {
            let result = expression.clone();
            expression.kind = IrExprKind::SigRead(0);
            Some(result)
        }
        IrExprKind::PartSel { base: a, .. }
        | IrExprKind::CastToReal { a, .. }
        | IrExprKind::CastToPacked { a }
        | IrExprKind::Convert { a }
        | IrExprKind::Resize { a }
        | IrExprKind::ToTwoState { a } => source_shape(a),
        IrExprKind::BitSel { base, idx } if matches!(idx.kind, IrExprKind::Const(_)) => {
            source_shape(base)
        }
        IrExprKind::IdxPartSel { base, base_idx, .. }
            if matches!(base_idx.kind, IrExprKind::Const(_)) =>
        {
            source_shape(base)
        }
        _ => None,
    }
}

pub(in crate::sim::emit_c) fn assignment(
    model: &IrModel,
    statement: &IrStmt,
    resolve: impl FnOnce(&IrExpr) -> Option<(String, IrType)>,
) -> Option<(Shape, Row)> {
    let IrStmt::PcaAssign {
        sig,
        enable,
        site,
        value,
    } = statement
    else {
        return None;
    };
    let mut expression = value.clone();
    let (source, source_type) = resolve(&source_shape(&mut expression)?)?;
    if effects_for_statements(model, std::slice::from_ref(statement))
        .iter()
        .any(|effect| *effect != ExecutionEffect::ImmediateStore)
    {
        return None;
    }
    Some((
        Shape {
            expression,
            source_type,
            target_type: model.signal(*sig).ty,
        },
        Row {
            source,
            target: *sig,
            enable: *enable,
            binding: *site,
        },
    ))
}
