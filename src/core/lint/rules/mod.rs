//! core::lint::rules — individual lint rules.
//!
//! Each rule lives in its own file and implements [`crate::core::lint::LintRule`];
//! shared analysis helpers live in [`analysis`].  Rules are registered in
//! [`crate::core::lint::LintRegistry::default_rules`] in a stable order.

pub mod analysis;
pub mod blocking_in_ff;
pub mod case_default;
pub mod casez;
pub mod combloop;
pub mod comparison_width;
pub mod if_latch;
pub mod implicit_net;
pub mod latch;
pub mod mixed_assign;
pub mod multidriver;
pub mod nba_in_comb;
pub mod style;
pub mod unconnected_port;
pub mod unused;
pub mod unused_param;
pub mod width;

pub use blocking_in_ff::BlockingInFFRule;
pub use case_default::CaseDefaultMissingRule;
pub use casez::CasezMisuseRule;
pub use combloop::CombinationalLoopRule;
pub use comparison_width::ComparisonWidthRule;
pub use if_latch::IfLatchRule;
pub use implicit_net::ImplicitNetRule;
pub use latch::IncompleteCaseRule;
pub use mixed_assign::MixedAssignRule;
pub use multidriver::MultiDriverRule;
pub use nba_in_comb::NbaInCombRule;
pub use style::StyleRule;
pub use unconnected_port::UnconnectedPortRule;
pub use unused::UnusedSignalRule;
pub use unused_param::UnusedParameterRule;
pub use width::WidthMismatchRule;

/// The default rule set, in run order.
///
/// The original 14 rules keep their historical order; newer rules are
/// appended at the end (`unconnected-port`, then `mixed-assignments`) so the
/// run order of existing rules — and therefore the relative order of their
/// findings — stays stable across releases.
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
    ]
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::Mutex;

    use crate::core::compile;
    use crate::core::db::{Db, NodeId, NodeKind};
    use crate::core::lint::{lint, LintDiag};
    use crate::core::model::DesignModel;

    /// Serializes every Surelog-touching lib test (they share one process and
    /// Surelog writes `slpp_all/` into the CWD).
    pub(crate) static SURELOG_LOCK: Mutex<()> = Mutex::new(());

    /// Restores the process CWD and removes the temp dir even when the body
    /// panics, so a failing test cannot strand other tests in a deleted CWD.
    struct TempDirGuard {
        dir: std::path::PathBuf,
        orig: std::path::PathBuf,
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.orig);
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    /// Compile `sv` in a fresh temp dir (the CWD is moved there while the
    /// compile runs), build the db + model, and return them.
    pub(crate) fn build_design(sv: &str, top: &str) -> (Db, DesignModel) {
        let _guard = SURELOG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("llg_lint_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let orig_cwd = std::env::current_dir().expect("current dir");
        let _restore = TempDirGuard {
            dir: dir.clone(),
            orig: orig_cwd,
        };
        std::env::set_current_dir(&dir).expect("chdir to temp dir");
        let file = dir.join("design.sv");
        std::fs::write(&file, sv).expect("write design");
        let out = compile::compile(&compile::CompileOpts {
            files: vec![file.to_string_lossy().into_owned()],
            top: if top.is_empty() {
                None
            } else {
                Some(top.to_string())
            },
            ..Default::default()
        })
        .expect("compile should start");
        assert!(out.ok(), "compile must succeed: {:?}", out.diagnostics);
        let design = out.uhdm_design().expect("no uhdm design handle");
        let db = Db::build(design).expect("db build");
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
        for top in &db.tops {
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
