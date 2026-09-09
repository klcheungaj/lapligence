//! core::lint::rules — individual lint rules.
//!
//! Each rule lives in its own file and implements [`crate::core::lint::LintRule`];
//! shared analysis helpers live in [`analysis`].  Rules are registered in
//! [`crate::core::lint::LintRegistry::default_rules`] in a stable order.

pub mod analysis;
pub mod assignment_condition;
pub mod blocking_in_ff;
pub mod case_default;
pub mod casex_statement;
pub mod casez;
pub mod combloop;
pub mod comparison_width;
pub mod duplicate_case;
pub mod empty_sensitivity;
pub mod if_latch;
pub mod implicit_net;
pub mod latch;
pub mod mixed_assign;
pub mod multidriver;
pub mod nba_in_comb;
pub mod select_range;
pub mod sensitivity;
pub mod style;
pub mod unconnected_port;
pub mod undriven;
pub mod unused;
pub mod unused_param;
pub mod width;
pub mod xz_comparison;

pub use assignment_condition::AssignmentInConditionRule;
pub use blocking_in_ff::BlockingInFFRule;
pub use case_default::CaseDefaultMissingRule;
pub use casex_statement::CasexStatementRule;
pub use casez::CasezMisuseRule;
pub use combloop::CombinationalLoopRule;
pub use comparison_width::ComparisonWidthRule;
pub use duplicate_case::DuplicateCaseItemRule;
pub use empty_sensitivity::EmptyImplicitSensitivityRule;
pub use if_latch::IfLatchRule;
pub use implicit_net::ImplicitNetRule;
pub use latch::IncompleteCaseRule;
pub use mixed_assign::MixedAssignRule;
pub use multidriver::MultiDriverRule;
pub use nba_in_comb::NbaInCombRule;
pub use select_range::OutOfRangeSelectRule;
pub use sensitivity::IncompleteSensitivityListRule;
pub use style::StyleRule;
pub use unconnected_port::UnconnectedPortRule;
pub use undriven::UndrivenSignalRule;
pub use unused::UnusedSignalRule;
pub use unused_param::UnusedParameterRule;
pub use width::WidthMismatchRule;
pub use xz_comparison::XzLogicalEqualityRule;

/// The default rule set, in run order.
///
/// The original 16 rules keep their historical order. Newer rules are
/// appended so the run order of existing rules — and therefore the relative
/// order of their findings — stays stable across releases.
pub fn default_rules() -> Vec<Box<dyn crate::core::lint::LintRule>> {
    vec![
        Box::new(UnusedSignalRule),
        Box::new(WidthMismatchRule),
        Box::new(IncompleteCaseRule),
        Box::new(CombinationalLoopRule),
        Box::new(MultiDriverRule),
        Box::new(CasezMisuseRule),
        Box::new(IfLatchRule),
        Box::new(StyleRule::default()),
        Box::new(BlockingInFFRule),
        Box::new(NbaInCombRule),
        Box::new(UnusedParameterRule),
        Box::new(ImplicitNetRule),
        Box::new(CaseDefaultMissingRule),
        Box::new(ComparisonWidthRule),
        Box::new(UnconnectedPortRule),
        Box::new(MixedAssignRule),
        Box::new(UndrivenSignalRule),
        Box::new(IncompleteSensitivityListRule),
        Box::new(OutOfRangeSelectRule),
        Box::new(XzLogicalEqualityRule),
        Box::new(DuplicateCaseItemRule),
        Box::new(EmptyImplicitSensitivityRule),
        Box::new(AssignmentInConditionRule),
        Box::new(CasexStatementRule),
    ]
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::Mutex;

    use crate::core::compile;
    use crate::core::db::{Db, NodeId, NodeKind};
    use crate::core::lint::{lint, LintDiag};
    use crate::core::model::DesignModel;

    /// Serializes the remaining tests that mutate the process working directory.
    pub(crate) static CWD_LOCK: Mutex<()> = Mutex::new(());

    /// Compile an in-memory design and retain only owned semantic data.
    pub(crate) fn build_design(sv: &str, top: &str) -> (Db, DesignModel) {
        let out = compile::compile_checked(&compile::CompileOpts {
            sources: vec![compile::OwnedSource::compilation_unit("design.sv", sv)],
            top: (!top.is_empty()).then(|| top.to_owned()),
            ..Default::default()
        })
        .expect("design should compile");
        let db = Db::from_slang(&out.snapshot).expect("semantic capture");
        let model = DesignModel::from_db(&db);
        (db, model)
    }

    /// Compile `sv` and run every default lint rule over it.
    pub(crate) fn lint_design(sv: &str, top: &str) -> Vec<LintDiag> {
        let (db, model) = build_design(sv, top);
        lint(&db, &model)
    }

    /// The diagnostics produced by one rule.
    pub(crate) fn rule_diags<'a>(diags: &'a [LintDiag], rule: &str) -> Vec<&'a LintDiag> {
        diags.iter().filter(|d| d.rule == rule).collect()
    }

    /// The arena node of a named signal declared in the top instance.
    pub(crate) fn find_signal(db: &Db, name: &str) -> NodeId {
        for top in db.tops() {
            for c in &db.node(*top).children {
                if db.node(*c).name == name
                    && matches!(
                        db.node_kind(*c),
                        NodeKind::Net { .. } | NodeKind::Var { .. } | NodeKind::Array { .. }
                    )
                {
                    return *c;
                }
            }
        }
        panic!("signal {name} not found in the top instance");
    }
}
