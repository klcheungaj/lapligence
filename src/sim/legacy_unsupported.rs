//! Legacy constructs `llg` rejects by design.
//!
//! User decisions 2026-10-08/09: procedural `assign`/`deassign` in every
//! form (any target, local or hierarchical; `force`/`release` and module
//! continuous assignments, including hierarchical ones, stay supported), MOS and
//! resistive switches, `trireg` charge storage, the optional charge and
//! delay-mode directives, `$dumpports*`, stochastic-queue forms beyond the
//! supported `$q_*` subset, PLA tasks, the legacy driver/pattern/scale/scope
//! inspection tasks and the PLI 1.0 TF/ACC interface are not simulated. Each
//! must stop the run before C generation with one source-located message,
//! never run as a no-op or fail generically. [`diagnostic`] is the single
//! message shape; [`scan`] finds the constructs the owned database exposes
//! directly, and lowering uses [`diagnostic`] for the forms it must classify
//! itself or that the scan cannot reach.

use super::semantic::SemanticModel;
use crate::core::db::{NetType, NodeKind, PrimitiveType, StmtKind};

/// Constructs sharing one documented rejection scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LegacyFamily {
    ProceduralAssign,
    MosSwitch,
    TriregCharge,
    ChargeDirective,
    ExtendedVcd,
    StochasticQueue,
    Pla,
    DriverInspection,
    PliTfAcc,
}

impl LegacyFamily {
    /// The family spelling shown in diagnostics and `docs/sim_features.md`.
    pub fn label(self) -> &'static str {
        match self {
            Self::ProceduralAssign => "legacy procedural assign/deassign form",
            Self::MosSwitch => "MOS and resistive switch primitives",
            Self::TriregCharge => "trireg charge storage",
            Self::ChargeDirective => "charge and delay-mode directives",
            Self::ExtendedVcd => "extended VCD port dumping",
            Self::StochasticQueue => "stochastic queue form",
            Self::Pla => "legacy PLA tasks",
            Self::DriverInspection => "legacy driver and scope inspection tasks",
            Self::PliTfAcc => "PLI 1.0 TF/ACC interface",
        }
    }
}

/// The stable rejection message: `<location>: unsupported: <construct>
/// (<family>) is not supported by llg`. `location` is `file:line:col`.
pub fn diagnostic(location: &str, construct: &str, family: LegacyFamily) -> String {
    format!(
        "{location}: unsupported: {construct} ({}) is not supported by llg",
        family.label()
    )
}

/// Whether every line of `text` has the [`diagnostic`] shape.
pub fn is_diagnostic_list(text: &str) -> bool {
    !text.is_empty()
        && text.lines().all(|line| {
            line.ends_with(") is not supported by llg") && line.contains(": unsupported: ")
        })
}

/// Whether `name` is one of the sixteen `$async|$sync $and|$nand|$or|$nor
/// $array|$plane` PLA tasks (IEEE 1364-2001 17.5).
pub fn is_pla_system_task(name: &str) -> bool {
    let mut parts = name.split('$');
    parts.next() == Some("")
        && matches!(parts.next(), Some("async" | "sync"))
        && matches!(parts.next(), Some("and" | "nand" | "or" | "nor"))
        && matches!(parts.next(), Some("array" | "plane"))
        && parts.next().is_none()
}

/// The family of a system task that has no simulator implementation by
/// design. `$q_*` is not listed: a supported subset exists.
pub fn unsupported_system_task_family(name: &str) -> Option<LegacyFamily> {
    if is_pla_system_task(name) {
        return Some(LegacyFamily::Pla);
    }
    match name {
        "$dumpports" | "$dumpportsoff" | "$dumpportson" | "$dumpportsall" | "$dumpportslimit"
        | "$dumpportsflush" => Some(LegacyFamily::ExtendedVcd),
        "$countdrivers" | "$getpattern" | "$scale" | "$scope" | "$showscopes" | "$showvars" => {
            Some(LegacyFamily::DriverInspection)
        }
        _ => None,
    }
}

fn is_mos(prim_type: PrimitiveType) -> bool {
    matches!(
        prim_type,
        PrimitiveType::Nmos
            | PrimitiveType::Pmos
            | PrimitiveType::Cmos
            | PrimitiveType::Rnmos
            | PrimitiveType::Rpmos
            | PrimitiveType::Rcmos
    )
}

/// Every reachable instance of the families the owned database exposes
/// directly, as stable diagnostics sorted by source position. Nodes that
/// elaboration discards (an inactive generate branch, an uninstantiated
/// module) are not reported.
pub fn scan(model: &SemanticModel<'_>) -> Vec<String> {
    let db = model.db();
    let reachable = model.simulation_reachability();
    let mut found: Vec<((String, u32, u32), String)> = Vec::new();
    for id in db.node_ids() {
        if !reachable[id.index()] {
            continue;
        }
        let (construct, family) = match db.node_kind(id) {
            NodeKind::Gate { prim_type, .. } if is_mos(*prim_type) => (
                format!("`{}` primitive", format!("{prim_type:?}").to_lowercase()),
                LegacyFamily::MosSwitch,
            ),
            NodeKind::Net {
                net_type: NetType::TriReg,
                ..
            } => (
                format!("`trireg` net `{}`", db.node(id).name()),
                LegacyFamily::TriregCharge,
            ),
            NodeKind::Array { .. }
                if db.array_meta(id).and_then(|meta| meta.net_type()) == Some(NetType::TriReg) =>
            {
                (
                    format!("`trireg` net array `{}`", db.node(id).name()),
                    LegacyFamily::TriregCharge,
                )
            }
            NodeKind::Stmt(StmtKind::ProcContAssign { .. }) => (
                "procedural `assign`".to_owned(),
                LegacyFamily::ProceduralAssign,
            ),
            NodeKind::Stmt(StmtKind::Deassign { .. }) => (
                "procedural `deassign`".to_owned(),
                LegacyFamily::ProceduralAssign,
            ),
            NodeKind::SysCall { name } => match unsupported_system_task_family(name) {
                Some(family) => (format!("system task `{name}`"), family),
                None => continue,
            },
            _ => continue,
        };
        let node = db.node(id);
        let location = model
            .origin_of(id)
            .and_then(|origin| model.origin(origin))
            .map_or_else(|| "<unknown source>".to_owned(), |origin| origin.location());
        found.push((
            (
                node.file().map(str::to_owned).unwrap_or_default(),
                node.line(),
                node.column(),
            ),
            diagnostic(&location, &construct, family),
        ));
    }
    for directive in db.legacy_directives() {
        found.push((
            (directive.file.clone(), directive.line, directive.column),
            diagnostic(
                &format!("{}:{}:{}", directive.file, directive.line, directive.column),
                &format!("directive `` `{}``", directive.name),
                LegacyFamily::ChargeDirective,
            ),
        ));
    }
    found.sort();
    found.dedup();
    found.into_iter().map(|(_, message)| message).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_has_one_stable_shape() {
        assert_eq!(
            diagnostic("a.v:3:8", "`nmos` primitive", LegacyFamily::MosSwitch),
            "a.v:3:8: unsupported: `nmos` primitive (MOS and resistive switch primitives) is not supported by llg"
        );
    }

    #[test]
    fn diagnostic_lists_are_recognised_line_by_line() {
        let one = diagnostic("a.v:1:2", "`trireg` net `t`", LegacyFamily::TriregCharge);
        assert!(is_diagnostic_list(&one));
        assert!(is_diagnostic_list(&format!("{one}\n{one}")));
        assert!(!is_diagnostic_list(""));
        assert!(!is_diagnostic_list(&format!(
            "{one}\nunsupported expression"
        )));
        assert!(!is_diagnostic_list("unsupported system task $x in `tb`"));
    }

    #[test]
    fn system_task_families_cover_exactly_the_rejected_names() {
        for name in [
            "$dumpports",
            "$dumpportsoff",
            "$dumpportson",
            "$dumpportsall",
            "$dumpportslimit",
            "$dumpportsflush",
        ] {
            assert_eq!(
                unsupported_system_task_family(name),
                Some(LegacyFamily::ExtendedVcd)
            );
        }
        for name in [
            "$countdrivers",
            "$getpattern",
            "$scale",
            "$scope",
            "$showscopes",
            "$showvars",
        ] {
            assert_eq!(
                unsupported_system_task_family(name),
                Some(LegacyFamily::DriverInspection)
            );
        }
        for timing in ["async", "sync"] {
            for gate in ["and", "nand", "or", "nor"] {
                for form in ["array", "plane"] {
                    assert_eq!(
                        unsupported_system_task_family(&format!("${timing}${gate}${form}")),
                        Some(LegacyFamily::Pla)
                    );
                }
            }
        }
        for name in [
            "$display",
            "$dumpvars",
            "$dumpfile",
            "$q_initialize",
            "$q_full",
            "$async$and$other",
            "$sync$and$array$extra",
            "$scale_x",
        ] {
            assert_eq!(unsupported_system_task_family(name), None, "{name}");
        }
    }
}
