//! Ordered PCA loops using registered temporaries and model-local typed helpers.
use super::super::statements::pca_batches::{
    assignment, Batch, Row, Shape, PCA_BATCH_MIN_ASSIGNMENTS,
};
use super::*;

impl Frame<'_, '_> {
    pub(super) fn statements(&mut self, mut statements: &[IrStmt]) -> Result<(), String> {
        while let Some(statement) = statements.first() {
            let mut consumed = self.net_batch(statements)?;
            if consumed == 0
                && self.pca_owner.is_some()
                && !self.sampled_reads
                && self.formal_overrides.is_empty()
            {
                if let Some((shape, first)) = self.pca_assignment(statement) {
                    let mut rows = vec![first];
                    for statement in &statements[1..] {
                        let Some((next, row)) = self.pca_assignment(statement) else {
                            break;
                        };
                        if next != shape {
                            break;
                        }
                        rows.push(row);
                    }
                    if rows.len() >= PCA_BATCH_MIN_ASSIGNMENTS {
                        consumed = rows.len();
                        self.pca_batch(shape, rows)?;
                    }
                }
            }
            if consumed == 0 {
                self.statement(statement)?;
                consumed = 1;
            }
            statements = &statements[consumed..];
        }
        Ok(())
    }

    fn pca_assignment(&self, statement: &IrStmt) -> Option<(Shape, Row)> {
        assignment(self.ctx.model, statement, |source| match &source.kind {
            IrExprKind::SigRead(index) => {
                let signal = self.ctx.model.signal(*index);
                // Alias reads require a resolver, rather than a storage clone.
                signal
                    .net_alias
                    .is_empty()
                    .then(|| (format!("&{}", signal.c_name), signal.ty))
            }
            IrExprKind::LocalRead(name) => {
                let binding = self.lookup(name)?;
                if binding.automatic {
                    return None;
                }
                let ty = if binding.width == 0 {
                    IrType::Real {
                        shortreal: binding.shortreal,
                    }
                } else {
                    IrType::Packed {
                        width: binding.width,
                        signed: binding.signed,
                        two_state: binding.two_state,
                    }
                };
                Some((binding.address, ty))
            }
            _ => None,
        })
    }

    fn pca_batch(&mut self, shape: Shape, rows: Vec<Row>) -> Result<(), String> {
        let batch = Batch {
            name: format!(
                "llg_pca_rows_{}_{}",
                self.pca_owner.as_ref().expect("PCA owner"),
                self.pca_batches.len()
            ),
            shape,
            rows,
        };
        let (_, slots) = helper_body(&batch.shape)?;
        // Use the caller's registered scope, including constant index owners,
        // so synchronous publication retains the original temporary lifetime.
        let start = (0..=self.slots.len())
            .find(|start| {
                (0..slots).all(|offset| !self.slots.get(start + offset).copied().unwrap_or(false))
            })
            .ok_or_else(|| "no empty PCA temporary range".to_owned())?;
        self.slots
            .resize(self.slots.len().max(start + slots), false);
        self.slots[start..start + slots].fill(true);
        let temporaries = if slots == 0 {
            "NULL".to_owned()
        } else {
            format!("_llg_t + {start}")
        };
        self.line("{");
        let index = self.declare("size_t", "pca_i", "0".to_owned());
        self.line(format!(
            "for (; {index} < {}; ++{index}) {{",
            batch.rows.len()
        ));
        self.line(format!(
            "{}({temporaries}, &{}[{index}]);",
            batch.helper(),
            batch.name
        ));
        self.line("}");
        self.line("}");
        self.slots[start..start + slots].fill(false);
        self.pca_batches.push(batch);
        Ok(())
    }
}

pub(in crate::sim::emit_c) fn helper_body(shape: &Shape) -> Result<(String, usize), String> {
    let mut model = IrModel::new("pca_helper".to_owned(), 1).map_err(|error| error.to_string())?;
    model.signals.push(
        IrSignal::new("llg_source".to_owned(), None, shape.source_type, None)
            .map_err(|error| error.to_string())?,
    );
    let ctx = RCtx {
        model: &model,
        func: None,
        sampled: false,
        activation_label: None,
    };
    let mut frame = Frame::new(&ctx);
    frame
        .callback_signal_overrides
        .push(HashMap::from([(0, 0)]));
    frame.formal_overrides.push(vec![Binding {
        address: "row->source".to_owned(),
        width: shape.source_type.width(),
        signed: shape.source_type.signed(),
        two_state: shape.source_type.two_state(),
        shortreal: matches!(shape.source_type, IrType::Real { shortreal: true }),
        automatic: false,
    }]);
    let value = frame.expression(&shape.expression)?;
    let value = frame.convert(
        value,
        shape.target_type.width(),
        shape.target_type.signed(),
        shape.target_type.two_state(),
        matches!(shape.target_type, IrType::Real { shortreal: true }),
    );
    let suffix = if shape.target_type.width() == 0 {
        "_d"
    } else {
        ""
    };
    frame.line(format!(
        "llg_pca_assign{suffix}(row->target, row->enable, row->binding, {});",
        value.code
    ));
    frame.discard(value);
    Ok((frame.body().to_owned(), frame.slots.len()))
}
